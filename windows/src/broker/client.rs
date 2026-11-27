use super::*;
use crate::pipe_io::Pipe;
use std::sync::atomic::{AtomicU64, Ordering};
pub const COLD: Duration = Duration::from_secs(30);
pub const REQUEST: Duration = Duration::from_millis(750);
pub const RECOVERY: Duration = Duration::from_millis(1500);
static NEXT: AtomicU64 = AtomicU64::new(0);
fn identity() -> String {
    format!(
        "{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )
}
fn classify(error: &io::Error) -> Failure {
    if error.kind() == io::ErrorKind::TimedOut {
        Failure::Timeout
    } else {
        Failure::Disconnected
    }
}
pub struct Client {
    pipe: Option<Pipe>,
    data: PathBuf,
    config: Config,
    id: String,
    sequence: u64,
    journal: Vec<Op>,
    settings: Vec<Op>,
    outstanding: Option<Op>,
    pub pending: bool,
    pub failure: Option<Failure>,
    #[cfg(feature = "rerank-timing")]
    pub last_rerank_job_id: Option<u64>,
}
impl Client {
    pub fn open(data: &Path, config: Config, source: Option<String>) -> Option<Self> {
        Self::try_open(data, config, source).map_err(|e| {
            crate::runtime_log::io("client.open", &e);
            crate::bridge::failure(format!("core open: kind={:?} win32={:?}",e.kind(),e.raw_os_error()));
        }).ok()
    }
    pub fn id(&self) -> &str { &self.id }
    fn try_open(data: &Path, config: Config, source: Option<String>) -> io::Result<Self> {
        let deadline = Instant::now() + COLD;
        let pipe = connect(data, true, deadline).map_err(crate::runtime_log::at("client.connect"))?;
        let id = identity();
        pipe.send(
            &Request::Open {
                id: id.clone(),
                config: config.clone(),
                source,
                replay: Vec::new(),
                sequence: 0,
            },
            deadline,
        )
        .map_err(crate::runtime_log::at("client.send_open"))?;
        if !pipe.receive::<Reply>(deadline).map_err(crate::runtime_log::at("client.receive_open"))?.ok {
            return Err(io::Error::other("Core refused session"));
        }
        Ok(Self {
            pipe: Some(pipe),
            data: data.into(),
            config,
            id,
            sequence: 0,
            journal: Vec::new(),
            settings: Vec::new(),
            outstanding: None,
            pending: false,
            failure: None,
            #[cfg(feature = "rerank-timing")]
            last_rerank_job_id: None,
        })
    }
    fn reconnect(&mut self, uncertain: bool, deadline: Instant) -> io::Result<()> {
        self.pipe = None;
        let pipe = connect(&self.data, true, deadline)?;
        pipe.send(&Request::Resume(self.id.clone()), deadline)?;
        let resumed = pipe.receive::<Reply>(deadline)?;
        if !resumed.found {
            if uncertain {
                self.failure = Some(Failure::Ambiguous);
                return Err(io::Error::other(
                    "Core result is unknown; composition retained",
                ));
            }
            let mut replay = self.settings.clone();
            replay.extend(self.journal.clone());
            pipe.send(
                &Request::Open {
                    id: self.id.clone(),
                    config: self.config.clone(),
                    source: None,
                    replay,
                    sequence: self.sequence,
                },
                deadline,
            )?;
            if !pipe.receive::<Reply>(deadline)?.ok {
                return Err(io::Error::other("Core recovery failed"));
            }
        }
        self.pipe = Some(pipe);
        Ok(())
    }
    pub fn perform(&mut self, op: Op) -> Option<Snapshot> {
        if self.failure == Some(Failure::Ambiguous) {
            return None;
        }
        // Privacy is idempotent, so an unanswered one is completed before a different operation.
        if let Some(old @ Op::Privacy(_)) = self.outstanding.clone().filter(|old| old != &op) {
            self.perform(old)?;
        }
        // A rerank left unanswered goes first: the core may have applied it, and its sequence comes before this one.
        if self.outstanding == Some(Op::Rerank) && op != Op::Rerank {
            self.perform(Op::Rerank)?;
        }
        if self.outstanding.as_ref().is_some_and(|old| old != &op) {
            self.failure = Some(Failure::Ambiguous);
            return None;
        }
        self.failure = None;
        let uncertain = self.key_outstanding();
        if self.pipe.is_none() {
            if let Err(error) = self.reconnect(uncertain, Instant::now() + RECOVERY) {
                crate::runtime_log::io("client.reconnect", &error);
                if self.failure.is_none() {
                    self.failure = Some(classify(&error));
                }
                return None;
            }
        }
        let request = Request::Apply {
            sequence: self.sequence + 1,
            op: op.clone(),
        };
        let deadline = Instant::now() + REQUEST;
        // Retain the same operation and sequence until its result is acknowledged.
        self.outstanding = Some(op.clone());
        let pipe = self.pipe.as_ref().unwrap();
        let result = match pipe.send_tracked(&request, deadline) {
            Ok(()) => pipe.receive::<Reply>(deadline),
            Err(error) => {
                crate::runtime_log::io("client.send_apply", &error.error);
                if !error.possible {
                    self.outstanding = None;
                }
                Err(error.error)
            }
        };
        let result = match result {
            Ok(result) => result,
            Err(error) => {
                crate::runtime_log::io("client.receive_apply", &error);
                self.pipe = None;
                self.failure = Some(classify(&error));
                return None;
            }
        };
        if !result.ok {
            self.failure = result.failure.or(Some(Failure::Failed));
            // Busy means the server explicitly did not process the operation.
            if self.failure == Some(Failure::Busy) {
                self.outstanding = None;
            }
            return None;
        }
        if result.sequence != self.sequence + 1 {
            self.failure = Some(Failure::Ambiguous);
            return None;
        }
        self.sequence = result.sequence;
        self.outstanding = None;
        self.pending = result.pending;
        #[cfg(feature = "rerank-timing")]
        {
            self.last_rerank_job_id = result.rerank_job_id;
        }
        match &op {
            Op::Options(_) | Op::HalfPunct(_) | Op::Mixed(_) | Op::English(_) | Op::Tolerance(_)
            | Op::MixedEnglishWriting(_) | Op::Prediction(_) | Op::Demote(_) | Op::Privacy(_) => {
                self.settings
                    .retain(|old| std::mem::discriminant(old) != std::mem::discriminant(&op));
                self.settings.push(op);
            }
            Op::Context(_) => {
                self.journal.retain(|old| !matches!(old, Op::Context(_)));
                self.journal.push(op);
            }
            _ => {
                if result
                    .snapshot
                    .as_ref()
                    .is_some_and(|s| s.preedit.is_empty())
                {
                    self.journal.clear();
                } else {
                    self.journal.push(op);
                }
            }
        }
        result.snapshot
    }
}
impl Client {
    /// An operation whose result is unknown and which cannot simply be sent again.
    pub fn key_outstanding(&self) -> bool {
        self.outstanding.as_ref().is_some_and(|op| !matches!(op, Op::Privacy(_)))
    }
    /// Whether the core finished reranking the last output. Unsequenced, so it changes nothing there; an unanswered
    /// `Op::Rerank` counts as ready, to be sent again.
    pub fn rerank_ready(&mut self) -> bool {
        if self.outstanding.is_some() || self.failure.is_some() {
            return self.outstanding == Some(Op::Rerank) && self.failure != Some(Failure::Ambiguous);
        }
        let Some(pipe) = self.pipe.as_ref() else {
            return false;
        };
        let deadline = Instant::now() + Duration::from_millis(200);
        #[cfg(feature = "rerank-timing")]
        let (poll_seq, poll_job, poll_t0) = {
            let seq = crate::rerank_timing::next_poll_seq();
            let job = self.last_rerank_job_id.unwrap_or(0);
            let t0 = crate::rerank_timing::qpc_now();
            crate::rerank_timing::record_span(
                crate::rerank_timing::ROLE_TIP,
                crate::rerank_timing::EV_POLL_START,
                t0,
                0,
                seq,
                job,
                0,
                0,
                0,
                0,
            );
            (seq, job, t0)
        };
        match pipe.send(&Request::RerankReady, deadline).and_then(|()| pipe.receive::<Reply>(deadline)) {
            Ok(reply) => {
                #[cfg(feature = "rerank-timing")]
                {
                    let t1 = crate::rerank_timing::qpc_now();
                    let dur_us = crate::rerank_timing::qpc_delta_us(poll_t0, t1);
                    let code = if reply.rerank_ready {
                        crate::rerank_timing::POLL_READY
                    } else {
                        crate::rerank_timing::POLL_NOT_READY
                    };
                    crate::rerank_timing::record_span(
                        crate::rerank_timing::ROLE_TIP,
                        crate::rerank_timing::EV_POLL_END,
                        poll_t0,
                        t1,
                        poll_seq,
                        poll_job,
                        dur_us,
                        0,
                        reply.rerank_ready as u8,
                        code,
                    );
                }
                reply.rerank_ready
            }
            Err(error) => {
                #[cfg(feature = "rerank-timing")]
                {
                    let t1 = crate::rerank_timing::qpc_now();
                    let dur_us = crate::rerank_timing::qpc_delta_us(poll_t0, t1);
                    let code = if error.kind() == io::ErrorKind::TimedOut {
                        crate::rerank_timing::POLL_TIMEOUT
                    } else {
                        crate::rerank_timing::POLL_IO_ERROR
                    };
                    crate::rerank_timing::record_span(
                        crate::rerank_timing::ROLE_TIP,
                        crate::rerank_timing::EV_POLL_END,
                        poll_t0,
                        t1,
                        poll_seq,
                        poll_job,
                        dur_us,
                        0,
                        0,
                        code,
                    );
                }
                // A late answer would be read as the next request's; the next perform reconnects.
                crate::runtime_log::io("client.rerank_ready", &error);
                self.pipe = None;
                false
            }
        }
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        if let Some(pipe) = self.pipe.as_ref() {
            let _ = pipe.send(&Request::Close, Instant::now() + Duration::from_millis(50));
        }
    }
}
