use shanjie_core::ffi::*;
use std::{
    ffi::{CStr, CString},
    os::windows::fs::MetadataExt,
    path::{Path, PathBuf},
    ptr,
    sync::{Arc, Condvar, Mutex},
    time::{Duration, Instant},
};
use windows::{
    core::PCWSTR,
    Win32::{
        Foundation::{CloseHandle, HANDLE, WAIT_ABANDONED, WAIT_OBJECT_0},
        System::Threading::{
            CreateMutexW, GetCurrentThread, ReleaseMutex, SetThreadPriority, WaitForSingleObject,
            THREAD_PRIORITY_BELOW_NORMAL,
        },
    },
};

pub use shanjie_shell::protocol::{Failure, Snapshot};

#[derive(Clone, PartialEq, Eq)]
struct ModelStamp {
    path: PathBuf,
    files: [Option<(u64, u64)>; 7],
}
impl ModelStamp {
    fn read(path: PathBuf) -> Self {
        let stamp = |p: &Path| std::fs::metadata(p).ok().map(|m| (m.file_size(), m.last_write_time()));
        let [classes, kn, trigram, chat, tiny, colloquial] = ["classes.sjc", "kn.sjkn", "trigram.sjt3", "trigram-chat.sjt3", "tinylm.sjtl", "colloquial.sjlm"]
            .map(|name| stamp(&path.with_file_name(name)));
        let files = [stamp(&path), classes, kn, trigram, chat, tiny, colloquial];
        Self { path, files }
    }
}
pub(crate) struct SharedModel {
    handle: *mut ShanjieShared,
    model: ModelStamp,
}
// §5.4: only the immutable shared parts cross threads. No operation borrows the source engine.
unsafe impl Send for SharedModel {}
unsafe impl Sync for SharedModel {}
impl Drop for SharedModel {
    fn drop(&mut self) {
        unsafe { shanjie_shared_free(self.handle) }
    }
}
pub(crate) enum BuildSource {
    Local(SharedModel),
    Remote(String),
}
/// A rerank job. It owns its data (S7c-2a section 2.5), so it may outlive the engine; freed once nothing holds it.
struct Job(*mut ShanjieRerankJob);
// The core makes the job Send and Sync from its fields (ffi.rs F8); only the pointer needs saying so.
unsafe impl Send for Job {}
unsafe impl Sync for Job {}
impl Drop for Job {
    fn drop(&mut self) {
        unsafe { shanjie_rerank_free(self.0) }
    }
}
/// Upstream's rerank runner (s7c2b section 2.1): one job at a time off the request thread, and only the latest output's
/// job waits. `latest` counts outputs; a job is kept only while it belongs to the latest one.
#[derive(Default)]
struct Reranks {
    latest: u64,
    waiting: Option<(u64, Arc<Job>)>,
    running: Option<u64>,
    finished: Option<(u64, Arc<Job>, i32)>,
    stop: bool,
}
impl Reranks {
    fn busy(&self) -> bool {
        self.waiting.is_some() || self.running == Some(self.latest)
    }
    fn pending(&self) -> bool {
        self.busy() || self.finished.is_some()
    }
    fn discard(&mut self) {
        self.latest += 1;
        self.waiting = None;
        self.finished = None;
    }
}
type Runner = Arc<(Mutex<Reranks>, Condvar)>;
fn rerank_worker(runner: Runner) {
    unsafe {
        let _ = SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_BELOW_NORMAL);
    }
    let (slot, signal) = &*runner;
    let mut r = slot.lock().unwrap();
    loop {
        if r.stop {
            return;
        }
        let Some((id, job)) = r.waiting.take() else {
            r = signal.wait(r).unwrap();
            continue;
        };
        r.running = Some(id);
        drop(r);
        let code = unsafe { shanjie_rerank_run(job.0) };
        r = slot.lock().unwrap();
        r.running = None;
        if id == r.latest {
            r.finished = Some((id, job, code));
        }
        signal.notify_all();
    }
}
pub struct Engine {
    pub handle: *mut ShanjieEngine,
    model: Option<ModelStamp>,
    pub rerank_jobs_run:u64,
    pub last_rerank_ns: Option<u64>,
    /// Set in the resident core: jobs run there instead of before the reply.
    reranks: Option<Runner>,
    /// The last output, which `Op::Rerank` reranks.
    last: Snapshot,
    remote: Option<crate::broker::Client>,
    mutex: Option<HANDLE>,
    folder: PathBuf,
    epoch: String,
    learning_stamp: Option<(u64, u64)>,
    learning_loaded: bool,
    privacy_allowed: bool,
    replaying: bool,
    failure: Option<Failure>,
    attempt: Vec<(crate::broker::Op, Snapshot)>,
    attempt_cursor: Option<usize>,
}
// The core owns its allocations/mappings and has no thread-affine UI or COM
// objects. Move exclusive ownership only; never share an Engine concurrently.
// The learning mutex is acquired and released within each operation.
unsafe impl Send for Engine {}
impl Default for Engine {
    fn default() -> Self {
        Self {
            handle: ptr::null_mut(),
            model: None,
            rerank_jobs_run:0,
            last_rerank_ns: None,
            reranks: None,
            last: Snapshot::default(),
            remote: None,
            mutex: None,
            folder: PathBuf::new(),
            epoch: String::new(),
            learning_stamp: None,
            learning_loaded: false,
            privacy_allowed: true,
            replaying: false,
            failure: None,
            attempt: Vec::new(),
            attempt_cursor: None,
        }
    }
}
impl Drop for Engine {
    fn drop(&mut self) {
        if let Some(runner) = self.reranks.take() {
            // A job still running finishes on its own; nothing applies it.
            runner.0.lock().unwrap().stop = true;
            runner.1.notify_all();
        }
        unsafe {
            shanjie_engine_free(self.handle);
            if let Some(h) = self.mutex {
                let _ = CloseHandle(h);
            }
        }
    }
}
impl Engine {
    /// An engine in the resident core (SHANJIE_CORE_LOCAL: in this process instead).
    pub fn open(data: &Path, config: &crate::broker::Config) -> Option<Self> {
        Self::open_reusing(data, config, None)
    }
    pub(crate) fn open_reusing(data: &Path, config: &crate::broker::Config, source: Option<BuildSource>) -> Option<Self> {
        if std::env::var_os("SHANJIE_CORE_LOCAL").is_some() {
            return Self::open_local_reusing(data, config, match &source {
                Some(BuildSource::Local(shared)) => Some(shared), _ => None,
            });
        }
        let remote = crate::broker::Client::open(data, config.clone(), match source {
            Some(BuildSource::Remote(id)) => Some(id), _ => None,
        })?;
        let mut engine = Self::default();
        engine.remote = Some(remote);
        Some(engine)
    }
    pub(crate) fn build_source(&self) -> Option<BuildSource> {
        if let Some(client) = &self.remote {
            return Some(BuildSource::Remote(client.id().to_owned()));
        }
        self.share_local().map(BuildSource::Local)
    }
    pub(crate) fn share_local(&self) -> Option<SharedModel> {
        let model = self.model.as_ref()?.clone();
        let mut handle = ptr::null_mut();
        (unsafe { shanjie_engine_share(self.handle, &mut handle) } == 0)
            .then_some(SharedModel { handle, model })
    }
    /// Builds the engine in this process, in the order of upstream's Shell.build(): a failing side
    /// model only turns its feature off, as there.
    pub fn open_local(data: &Path, config: &crate::broker::Config) -> Option<Self> {
        Self::open_local_reusing(data, config, None)
    }
    pub(crate) fn open_local_reusing(data: &Path, config: &crate::broker::Config, source: Option<&SharedModel>) -> Option<Self> {
        let model = ModelStamp::read(crate::model_store::resolve(data, &config.model));
        let mut e = Self::default();
        let folder = crate::profile::folder();
        let personal = folder.join(&config.dictionary);
        if config.dictionary != "dictionary.tsv" && !personal.exists() {
            return None;
        }
        let personal = (personal.exists() && personal.metadata().ok()?.len() != 0).then_some(personal);
        unsafe {
            if let Some(source) = source.filter(|s| s.model == model && model.path.extension().is_some_and(|ext| ext == "winmap")) {
                e.handle = windows_engine_new_shared(&*source.handle, &model.path, personal.as_deref(), config.layout, config.packs())?;
            } else {
                let dictionary = match personal.as_deref() {
                    Some(path) => Some(CString::new(path.to_str()?).ok()?),
                    None => None,
                };
                if shanjie_windows_engine_new(
                    CString::new(data.to_str()?).ok()?.as_ptr(),
                    dictionary.as_ref().map_or(ptr::null(), |d| d.as_ptr()),
                    config.layout,
                    config.packs(),
                    &mut e.handle,
                ) != 0 {
                    return None;
                }
                let lm = CString::new(model.path.to_str()?).ok()?;
                if shanjie_engine_load_lm(e.handle, lm.as_ptr()) != 0 {
                    return None;
                }
                let side = |name: &str| CString::new(model.path.with_file_name(name).to_str()?).ok();
                // Missing or failed: the n-gram alone. 常談 only serves the small model's rerank.
                if side("tinylm.sjtl").is_some_and(|t| shanjie_engine_load_tinylm(e.handle, t.as_ptr()) == 0) {
                    if let Some(table) = side("colloquial.sjlm") {
                        shanjie_engine_load_colloquial(e.handle, table.as_ptr());
                    }
                }
            }
            // A new engine starts with the core's defaults, so every switch is sent each time.
            e.operation(|h, o| shanjie_engine_set_prediction(h, config.prediction as u32, o))?;
            e.operation(|h, o| shanjie_engine_set_fuzzy(h, config.fuzzy, o))?;
            e.operation(|h, o| shanjie_engine_set_tone3(h, config.tone3 as u32, o))?;
            e.operation(|h, o| shanjie_engine_set_rerank(h, config.rerank as u32, o))?;
            e.operation(|h, o| shanjie_engine_set_rerank_level(h, config.rerank_level, o))?;
            e.operation(|h, o| shanjie_engine_set_profile(h, config.ranking, o))?;
            if shanjie_windows_set_candidate_anchor(e.handle, config.right as u32) != 0 {
                return None;
            }
            if config.learning {
                let name = format!(
                    "Local\\ShanjieLearning-{}",
                    folder.to_str()?.to_uppercase().replace('\\', "_")
                );
                e.mutex =
                    Some(CreateMutexW(None, false, PCWSTR(crate::wide(&name).as_ptr())).ok()?);
                e.folder = folder;
                if !e.begin(250) {
                    return None;
                }
                let result = shanjie_engine_set_learning(e.handle, 1);
                e.end();
                if result != 0 {
                    return None;
                }
            }
        }
        e.model = Some(model);
        Some(e)
    }
    /// From now on a rerank job runs on its own thread after the reply, as upstream runs it off the main thread.
    pub fn rerank_in_background(&mut self) {
        let runner: Runner = Default::default();
        let worker = runner.clone();
        if std::thread::Builder::new()
            .name("Shanjie rerank".into())
            .spawn(move || rerank_worker(worker))
            .is_ok()
        {
            self.reranks = Some(runner);
        }
    }
    /// Whether `Op::Rerank` would answer at once: no job of the last output is waiting or running.
    pub fn rerank_ready(&mut self) -> bool {
        if let Some(remote) = self.remote.as_mut() {
            return remote.rerank_ready();
        }
        self.reranks.as_ref().is_none_or(|r| !r.0.lock().unwrap().busy())
    }
    /// The core's generation guard drops a result whose composition has changed since the job was taken.
    unsafe fn apply_rerank(&mut self, job: &Job, out: &mut Snapshot) {
        let string = |p| CStr::from_ptr(p).to_str().unwrap().to_owned();
        let mut ranked = ptr::null_mut();
        if shanjie_engine_rerank_apply(self.handle, job.0, &mut ranked) == 0 && !ranked.is_null() {
            let r = &*ranked;
            out.preedit = string(r.preedit);
            out.cursor = r.cursor_utf16;
            out.active_segment = windows_active_segment(&*self.handle);
            out.english_run_utf16_start = r.english_run_utf16_start;
            out.english_run_utf16_len = r.english_run_utf16_len;
            out.candidates = (0..r.candidate_count).map(|i| string(*r.candidates.add(i as usize))).collect();
            out.selected = usize::try_from(r.candidate_selected).ok();
            out.columns = r.candidate_columns;
            out.first = r.candidate_first;
            out.total = r.candidate_total;
            out.prediction = shanjie_windows_is_prediction_row(self.handle) != 0
                || shanjie_windows_is_mixed_hint(self.handle) != 0;
            out.reading_hint = shanjie_windows_is_mixed_hint(self.handle) != 0;
            out.phonetic_pending = shanjie_windows_phonetic_pending(self.handle) != 0;
            shanjie_output_free(ranked);
        }
    }
    fn rerank(&mut self, timeout: Option<Duration>) -> Option<Snapshot> {
        let finished = if let Some(runner) = &self.reranks {
            let (slot, signal) = runner.as_ref();
            let mut r = slot.lock().unwrap();
            if let Some(timeout) = timeout {
                r = signal.wait_timeout_while(r, timeout, |state| state.busy()).unwrap().0;
                if r.busy() {
                    return None;
                }
            } else {
                r = signal.wait_while(r, |state| state.busy()).unwrap();
            }
            let latest = r.latest;
            r.finished.take().filter(|f| f.0 == latest)
        } else {
            None
        };
        // Only the candidates can change; what the last output committed was already sent.
        let mut out = Snapshot { commit: String::new(), rerank: false, ..self.last.clone() };
        if let Some((_, job, 0)) = finished {
            self.rerank_jobs_run = self.rerank_jobs_run.saturating_add(1);
            unsafe { self.apply_rerank(&job, &mut out) };
        }
        self.last = out.clone();
        Some(out)
    }
    fn begin(&mut self, timeout: u32) -> bool {
        if !self.privacy_allowed || self.replaying { return true; }
        unsafe {
            let Some(m) = self.mutex else {
                return true;
            };
            let result = WaitForSingleObject(m, timeout);
            if result != WAIT_OBJECT_0 && result != WAIT_ABANDONED {
                self.failure = Some(if result == windows::Win32::Foundation::WAIT_TIMEOUT {
                    Failure::Busy
                } else {
                    Failure::Failed
                });
                return false;
            }
            let epoch = std::fs::read_to_string(self.folder.join("learning.epoch"))
                .unwrap_or_default()
                .trim()
                .to_owned();
            if epoch != self.epoch {
                shanjie_engine_set_learning(self.handle, 0);
                shanjie_engine_set_learning(self.handle, 1);
                self.epoch = epoch;
                self.learning_loaded = false;
            }
            let stamp = self.store_stamp();
            if self.learning_loaded && stamp == self.learning_stamp {
                return true;
            }
            if shanjie_engine_learning_open(
                self.handle,
                CString::new(self.folder.to_str().unwrap())
                    .unwrap()
                    .as_ptr(),
            ) == 0
            {
                self.learning_loaded = true;
                self.learning_stamp = self.store_stamp();
                true
            } else {
                let _ = ReleaseMutex(m);
                false
            }
        }
    }
    fn store_stamp(&self) -> Option<(u64, u64)> {
        std::fs::metadata(self.folder.join("learning.tsv"))
            .ok()
            .map(|m| (m.last_write_time(), m.file_size()))
    }
    fn end(&mut self) {
        if !self.privacy_allowed || self.replaying { return; }
        if let Some(h) = self.mutex {
            // Capture our own writes while holding the same cross-process lock.
            // A peer write or clear is detected before the next operation.
            self.learning_stamp = self.store_stamp();
            unsafe {
                let _ = ReleaseMutex(h);
            }
        }
    }
    pub fn operation(
        &mut self,
        f: impl FnOnce(*mut ShanjieEngine, *mut *mut ShanjieOutput) -> i32,
    ) -> Option<Snapshot> {
        if self.handle.is_null() || !self.begin(250) {
            return None;
        }
        let out = self.invoke(f);
        self.end();
        out
    }
    fn invoke(
        &mut self,
        f: impl FnOnce(*mut ShanjieEngine, *mut *mut ShanjieOutput) -> i32,
    ) -> Option<Snapshot> {
        if self.handle.is_null() {
            return None;
        }
        let mut raw = ptr::null_mut();
        let result = f(self.handle, &mut raw);
        if result != 0 {
            return None;
        }
        unsafe {
            let r = &*raw;
            let string = |p| CStr::from_ptr(p).to_str().unwrap().to_owned();
            let mut out = Snapshot {
                handled: r.handled != 0,
                commit: string(r.commit),
                preedit: string(r.preedit),
                cursor: r.cursor_utf16,
                active_segment: windows_active_segment(&*self.handle),
                english_run_utf16_start: r.english_run_utf16_start,
                english_run_utf16_len: r.english_run_utf16_len,
                selected: usize::try_from(r.candidate_selected).ok(),
                columns: r.candidate_columns,
                first: r.candidate_first,
                total: r.candidate_total,
                prediction: shanjie_windows_is_prediction_row(self.handle) != 0
                    || shanjie_windows_is_mixed_hint(self.handle) != 0,
                reading_hint: shanjie_windows_is_mixed_hint(self.handle) != 0,
                phonetic_pending: shanjie_windows_phonetic_pending(self.handle) != 0,
                candidates: (0..r.candidate_count)
                    .map(|i| string(*r.candidates.add(i as usize)))
                    .collect(),
                associated: false,
                rerank: false,
                learning_unavailable: self.learning_unavailable(),
            };
            shanjie_output_free(raw);
            self.last_rerank_ns = None;
            let mut job=ptr::null_mut();
            let take_rc=shanjie_engine_rerank_take(self.handle,&mut job);
            let job=(take_rc==0&&!job.is_null()).then(||Job(job));
            if let Some(runner)=self.reranks.as_ref() {
                // Only a new decode (new job) or an ended composition/reset invalidates the previous job.
                // Non-decoding keys (e.g. an unfinished syllable) keep an in-flight or finished job valid.
                let mut r=runner.0.lock().unwrap();
                if let Some(job)=job {
                    r.latest+=1;
                    r.finished=None;
                    r.waiting=Some((r.latest,Arc::new(job)));
                    runner.1.notify_all();
                } else if take_rc!=0||out.preedit.is_empty() {
                    r.discard();
                    runner.1.notify_all();
                }
                out.rerank=r.pending();
            } else if let Some(job)=job {
                // Tools without a runner rerank before returning; commit is preserved once.
                let started = Instant::now();
                if shanjie_rerank_run(job.0)==0 {
                    self.last_rerank_ns = Some(started.elapsed().as_nanos() as u64);
                    self.rerank_jobs_run=self.rerank_jobs_run.saturating_add(1);
                    self.apply_rerank(&job,&mut out);
                }
            }
            self.last=out.clone();
            Some(out)
        }
    }
    pub fn key(&mut self, k: ShanjieKey) -> Option<Snapshot> {
        self.key_with_latin(k, None)
    }
    pub fn is_open(&self) -> bool {
        self.remote.is_some() || !self.handle.is_null()
    }
    pub fn key_with_latin(&mut self, k: ShanjieKey, latin: Option<char>) -> Option<Snapshot> {
        self.perform(crate::broker::Op::Key {
            kind: k.kind,
            ch: k.ch,
            modifiers: k.modifiers,
            latin,
        })
    }
    pub fn mixed_pending(&self) -> bool {
        self.remote.as_ref().map_or_else(
            || unsafe { shanjie_windows_is_mixed_pending(self.handle) != 0 },
            |c| c.pending,
        )
    }
    pub fn reset(&mut self, discard: bool) -> Option<Snapshot> {
        // Commit confirms pending Windows selection learning; discard only drops it.
        self.perform(crate::broker::Op::Reset(discard))
    }
    pub fn perform(&mut self, op: crate::broker::Op) -> Option<Snapshot> {
        if let Some(at) = self.attempt_cursor {
            if let Some((old, result)) = self.attempt.get(at) {
                if old != &op {
                    self.failure = Some(Failure::Ambiguous);
                    return None;
                }
                self.attempt_cursor = Some(at + 1);
                return Some(result.clone());
            }
        }
        self.failure = None;
        let mut result = self.perform_uncached(op.clone());
        if result.is_some() {
            if let crate::broker::Op::Privacy(allowed) = &op { self.privacy_allowed = *allowed; }
            if self.remote.is_some() {
                self.last.learning_unavailable = result.as_ref().unwrap().learning_unavailable;
            } else {
                result.as_mut().unwrap().learning_unavailable = self.learning_unavailable();
            }
        }
        if let Some(remote) = self.remote.as_ref() {
            self.failure = remote.failure;
        }
        if result.is_none() && self.failure.is_none() {
            self.failure = Some(Failure::Failed);
        }
        if let (Some(at), Some(snapshot)) = (self.attempt_cursor, result.as_ref()) {
            self.attempt.push((op, snapshot.clone()));
            self.attempt_cursor = Some(at + 1);
        }
        result
    }
    pub fn failure(&self) -> Option<Failure> {
        self.failure
    }
    pub fn learning_unavailable(&self) -> bool {
        if !self.privacy_allowed { return false; }
        if self.remote.is_some() { return self.last.learning_unavailable; }
        let mut flags = 0;
        self.mutex.is_some() && unsafe {
            shanjie_engine_learning_status(self.handle, &mut flags) == 0 && flags & 1 != 0
        }
    }
    pub fn begin_attempt(&mut self) {
        self.attempt_cursor = Some(0);
        self.failure = None;
    }
    pub fn finish_attempt(&mut self, succeeded: bool) {
        self.attempt_cursor = None;
        if succeeded {
            self.attempt.clear();
            self.failure = None;
        }
    }
    pub fn recovery_pending(&self) -> bool {
        !self.attempt.is_empty() || self.failure.is_some_and(|f| f != Failure::Failed)
    }
    pub fn privacy_allowed(&self) -> bool { self.privacy_allowed }
    /// Local maintenance tools only; no snapshot text or learning records leave the engine.
    pub fn benchmark_settings(&self) -> serde_json::Value {
        let s = unsafe { windows_benchmark_settings(&*self.handle) }.expect("semantic model loaded");
        serde_json::json!({"tinylm_sha256":s.tinylm_sha256,"colloquial_enabled":s.colloquial_enabled,
            "cells":s.cells,"header_cells":s.header_cells,"levels":s.levels,"strength":s.strength,"profile":s.profile,
            "finite_fallback":"per-job fallback to header cells remains upstream"})
    }
    pub fn benchmark_idle(&self) -> bool {
        (unsafe { windows_benchmark_idle(&*self.handle) }) && !self.last.rerank && self.last.preedit.is_empty()
    }
    pub fn set_privacy_allowed(&mut self, allowed: bool) -> Option<()> {
        // An unresolved key belongs to the privacy state it started under; its retry must come first.
        if !self.attempt.is_empty() || self.remote.as_ref().is_some_and(|c| c.key_outstanding()) { return Some(()); }
        if allowed != self.privacy_allowed { self.perform(crate::broker::Op::Privacy(allowed))?; }
        Some(())
    }
    fn perform_uncached(&mut self, op: crate::broker::Op) -> Option<Snapshot> {
        if let Some(remote) = self.remote.as_mut() {
            return remote.perform(op);
        }
        use crate::broker::Op;
        let string = |s: String| CString::new(s).ok();
        match op {
            Op::Key {
                kind,
                ch,
                modifiers,
                latin,
            } => {
                if kind == 3 && modifiers == 0 && self.last.rerank && !self.last.preedit.is_empty()
                    && (self.last.candidates.is_empty() || self.last.prediction && self.last.selected.is_none())
                {
                    self.rerank(Some(Duration::from_millis(100)));
                }
                self.operation(|h, o| unsafe {
                shanjie_windows_key_with_latin(
                    h,
                    ShanjieKey {
                        kind,
                        ch,
                        modifiers,
                    },
                    latin.map_or(0, |c| c as u32),
                    o,
                )
                })
            }
            Op::Reset(discard) => {
                if discard {
                    self.invoke(|h, o| unsafe { shanjie_engine_reset(h, 1, o) })
                } else if self.begin(0) {
                    let out = self.invoke(|h, o| unsafe { shanjie_engine_reset(h, 0, o) });
                    self.end();
                    out
                } else if self.failure == Some(Failure::Busy) {
                    // Focus loss must send its text without waiting for another learning writer.
                    self.failure = None;
                    unsafe { shanjie_engine_set_learning(self.handle, 0); }
                    let out = self.invoke(|h, o| unsafe { shanjie_engine_reset(h, 0, o) });
                    unsafe { shanjie_engine_set_learning(self.handle, 1); }
                    out
                } else {
                    None
                }
            }
            Op::Pick(index) => self.operation(|h, o| unsafe { shanjie_engine_pick(h, index, o) }),
            Op::Associate(s) => {
                let s = string(s)?;
                self.operation(|h, o| unsafe { shanjie_windows_associate(h, s.as_ptr(), o) })
            }
            Op::Reading => self.operation(|h, o| unsafe { shanjie_windows_reading(h, 1, o) }),
            Op::Prediction(on) => {
                self.operation(|h, o| unsafe { shanjie_engine_set_prediction(h, on as u32, o) })
            }
            Op::Demote(on) => {
                self.operation(|h, o| unsafe { shanjie_engine_set_demote(h, on as u32, o) })
            }
            Op::Punctuation(s) => {
                let s = string(s)?;
                self.operation(|h, o| unsafe { shanjie_windows_punctuation(h, s.as_ptr(), o) })
            }
            Op::Cycle => self.operation(|h, o| unsafe { shanjie_windows_cycle_candidate(h, o) }),
            Op::Space => self.operation(|h, o| unsafe { shanjie_windows_insert_space(h, o) }),
            Op::Context(s) => {
                let s = string(if self.privacy_allowed { s } else { String::new() })?;
                (unsafe { shanjie_engine_set_left_context(self.handle, s.as_ptr()) } == 0)
                    .then(Snapshot::default)
            }
            Op::Privacy(_) if self.handle.is_null() => None,
            Op::Privacy(allowed) => {
                if !allowed {
                    unsafe { windows_pause_learning(&mut *self.handle); }
                    if let Some(runner) = &self.reranks { runner.0.lock().unwrap().discard(); }
                } else {
                    self.learning_loaded = false;
                    unsafe { shanjie_engine_set_learning(self.handle, (self.mutex.is_some() && !self.replaying) as u32); }
                }
                self.privacy_allowed = allowed;
                Some(Snapshot::default())
            }
            Op::Options(flags) => {
                (unsafe { shanjie_windows_set_input_options(self.handle, flags) } == 0)
                    .then(Snapshot::default)
            }
            Op::Mixed(on) => (unsafe { shanjie_windows_set_mixed_input(self.handle, on as u32) }
                == 0)
                .then(Snapshot::default),
            Op::Tolerance(on) => (unsafe { shanjie_windows_set_mixed_tolerance(self.handle, on as u32) }
                == 0).then(Snapshot::default),
            Op::English(on) => (unsafe { shanjie_windows_set_upstream_english(self.handle, on) }
                == 0)
                .then(Snapshot::default),
            Op::Rerank if !self.handle.is_null() => self.rerank(None),
            Op::Rerank => None,
        }
    }
    pub fn replay(&mut self, ops: &[crate::broker::Op]) -> bool {
        // Reconstruct only the uncommitted composition after a core restart.
        // Replay must never add the same learning record a second time.
        self.replaying = true;
        unsafe {
            shanjie_engine_set_learning(self.handle, 0);
        }
        let ok = ops.iter().all(|op| self.perform(op.clone()).is_some());
        self.replaying = false;
        unsafe {
            shanjie_engine_set_learning(self.handle, (self.mutex.is_some() && self.privacy_allowed) as u32);
        }
        self.learning_loaded = false;
        ok
    }
}
#[cfg(test)]
mod privacy_tests {
    use super::*;
    #[test]
    fn closed_engine_has_no_privacy_operation_until_it_has_a_core_owner() {
        let mut engine=Engine::default();
        assert!(engine.set_privacy_allowed(false).is_none());
        assert!(!engine.is_open());
    }
}

impl shanjie_shell::input::Core for Engine {
    fn key(&mut self, key: ShanjieKey) -> Option<Snapshot> {
        Engine::key(self, key)
    }
    fn reset(&mut self, discard: bool) -> Option<Snapshot> {
        Engine::reset(self, discard)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enter_rerank_timeout_leaves_the_current_output_available() {
        let mut engine = Engine::default();
        engine.last = Snapshot { preedit: "原組字".into(), rerank: true, ..Default::default() };
        let runner = Arc::new((Mutex::new(Reranks { latest: 1, running: Some(1), ..Default::default() }), Condvar::new()));
        engine.reranks = Some(runner.clone());
        assert!(engine.rerank(Some(Duration::from_millis(1))).is_none());
        assert!(engine.last.rerank);
        assert_eq!(engine.last.preedit, "原組字");
        runner.0.lock().unwrap().latest += 1;
        let output = engine.rerank(Some(Duration::ZERO)).unwrap();
        assert!(!output.rerank);
        assert_eq!(output.preedit, "原組字");
    }

    #[test]
    fn reranks_pending_and_discard_track_inflight_and_finished_states() {
        let mut state = Reranks { latest: 1, running: Some(1), ..Default::default() };
        assert!(state.busy());
        assert!(state.pending());
        state.running = None;
        assert!(!state.busy());
        assert!(!state.pending());

        state.running = Some(1);
        state.discard();
        assert_eq!(state.latest, 2);
        assert!(!state.busy());
        assert!(!state.pending());
    }
}
