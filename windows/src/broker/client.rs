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
}
impl Client {
    pub fn open(data: &Path, config: Config) -> Option<Self> {
        let deadline = Instant::now() + COLD;
        let pipe = connect(data, true, deadline).ok()?;
        let id = identity();
        pipe.send(
            &Request::Open {
                id: id.clone(),
                config: config.clone(),
                replay: Vec::new(),
                sequence: 0,
            },
            deadline,
        )
        .ok()?;
        if !pipe.receive::<Reply>(deadline).ok()?.ok {
            return None;
        }
        Some(Self {
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
        if self.outstanding.as_ref().is_some_and(|old| old != &op) {
            self.failure = Some(Failure::Ambiguous);
            return None;
        }
        self.failure = None;
        let uncertain = self.outstanding.is_some();
        if self.pipe.is_none() {
            if let Err(error) = self.reconnect(uncertain, Instant::now() + RECOVERY) {
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
                if !error.possible {
                    self.outstanding = None;
                }
                Err(error.error)
            }
        };
        let result = match result {
            Ok(result) => result,
            Err(error) => {
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
        match &op {
            Op::Options(_) | Op::Mixed(_) | Op::English(_) | Op::Prediction(_) | Op::Demote(_) => {
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
impl Drop for Client {
    fn drop(&mut self) {
        if let Some(pipe) = self.pipe.as_ref() {
            let _ = pipe.send(&Request::Close, Instant::now() + Duration::from_millis(50));
        }
    }
}
