use shanjie_core::ffi::*;
use std::{
    ffi::{CStr, CString},
    os::windows::fs::MetadataExt,
    path::{Path, PathBuf},
    ptr,
};
use windows::{
    core::PCWSTR,
    Win32::{
        Foundation::{CloseHandle, HANDLE, WAIT_ABANDONED, WAIT_OBJECT_0},
        System::Threading::{CreateMutexW, ReleaseMutex, WaitForSingleObject},
    },
};

pub use shanjie_shell::protocol::{Failure, Snapshot};
pub struct Engine {
    pub handle: *mut ShanjieEngine,
    pub rerank_jobs_run:u64,
    remote: Option<crate::broker::Client>,
    mutex: Option<HANDLE>,
    folder: PathBuf,
    epoch: String,
    learning_stamp: Option<(u64, u64)>,
    learning_loaded: bool,
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
            rerank_jobs_run:0,
            remote: None,
            mutex: None,
            folder: PathBuf::new(),
            epoch: String::new(),
            learning_stamp: None,
            learning_loaded: false,
            failure: None,
            attempt: Vec::new(),
            attempt_cursor: None,
        }
    }
}
impl Drop for Engine {
    fn drop(&mut self) {
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
        if std::env::var_os("SHANJIE_CORE_LOCAL").is_some() {
            return Self::open_local(data, config);
        }
        let remote = crate::broker::Client::open(data, config.clone())?;
        let mut engine = Self::default();
        engine.remote = Some(remote);
        Some(engine)
    }
    /// Builds the engine in this process, in the order of upstream's Shell.build(): a failing side
    /// model only turns its feature off, as there.
    pub fn open_local(data: &Path, config: &crate::broker::Config) -> Option<Self> {
        let model = crate::model_store::resolve(data, &config.model);
        let mut e = Self::default();
        let folder = crate::profile::folder();
        let personal = folder.join(&config.dictionary);
        if config.dictionary != "dictionary.tsv" && !personal.exists() {
            return None;
        }
        let dictionary = if personal.exists() && personal.metadata().ok()?.len() != 0 {
            Some(CString::new(personal.to_str()?).ok()?)
        } else {
            None
        };
        let side = |name: &str| CString::new(model.with_file_name(name).to_str()?).ok();
        unsafe {
            if shanjie_windows_engine_new(
                CString::new(data.to_str()?).ok()?.as_ptr(),
                dictionary.as_ref().map_or(ptr::null(), |d| d.as_ptr()),
                config.layout,
                config.packs(),
                &mut e.handle,
            ) != 0
            {
                return None;
            }
            let lm = CString::new(model.to_str()?).ok()?;
            if shanjie_engine_load_lm(e.handle, lm.as_ptr()) != 0 {
                return None;
            }
            // Missing or failed: the n-gram alone. 常談 only serves the small model's rerank.
            if side("tinylm.sjtl").is_some_and(|t| shanjie_engine_load_tinylm(e.handle, t.as_ptr()) == 0) {
                if let Some(table) = side("colloquial.sjlm") {
                    shanjie_engine_load_colloquial(e.handle, table.as_ptr());
                }
            }
            // A missing list leaves inline English inert instead of failing the engine.
            if let Some(list) = data.join("english-words.txt").to_str().and_then(|p| CString::new(p).ok()) {
                shanjie_engine_load_english(e.handle, list.as_ptr());
            }
            // A new engine starts with the core's defaults, so every switch is sent each time.
            e.operation(|h, o| shanjie_engine_set_prediction(h, config.prediction as u32, o))?;
            e.operation(|h, o| shanjie_engine_set_english_key_order(h, config.english_key_order as u32, o))?;
            e.operation(|h, o| shanjie_engine_set_fuzzy(h, config.fuzzy, o))?;
            e.operation(|h, o| shanjie_engine_set_rerank(h, config.rerank as u32, o))?;
            e.operation(|h, o| shanjie_engine_set_rerank_level(h, config.rerank_level, o))?;
            e.operation(|h, o| shanjie_engine_set_profile(h, config.ranking, o))?;
            if shanjie_windows_set_candidate_anchor(e.handle, config.right as u32) != 0
                || shanjie_windows_set_candidate_charset(e.handle, config.big5 as u32) != 0
            {
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
                if !e.begin() {
                    return None;
                }
                let result = shanjie_engine_set_learning(e.handle, 1);
                e.end();
                if result != 0 {
                    return None;
                }
            }
        }
        Some(e)
    }
    fn begin(&mut self) -> bool {
        unsafe {
            let Some(m) = self.mutex else {
                return true;
            };
            let result = WaitForSingleObject(m, 250);
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
        if self.handle.is_null() || !self.begin() {
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
            };
            shanjie_output_free(raw);
            // Serialized in the resident core, never on a keyboard hook. The upstream
            // generation guard prevents stale application; commit is preserved once.
            let mut job=ptr::null_mut();
            if shanjie_engine_rerank_take(self.handle,&mut job)==0&&!job.is_null(){
                if shanjie_rerank_run(job)==0 {
                    self.rerank_jobs_run=self.rerank_jobs_run.saturating_add(1);
                    let mut ranked=ptr::null_mut();
                    if shanjie_engine_rerank_apply(self.handle,job,&mut ranked)==0&&!ranked.is_null(){
                        let r=&*ranked;out.preedit=string(r.preedit);out.cursor=r.cursor_utf16;
                        out.candidates=(0..r.candidate_count).map(|i|string(*r.candidates.add(i as usize))).collect();
                        out.selected=usize::try_from(r.candidate_selected).ok();out.columns=r.candidate_columns;
                        out.first=r.candidate_first;out.total=r.candidate_total;
                        shanjie_output_free(ranked);
                    }
                }
                shanjie_rerank_free(job);
            }
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
        // Upstream reset clears composition without learning or persistence.
        // Focus loss must not wait for another App's learning writer.
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
        let result = self.perform_uncached(op.clone());
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
            } => self.operation(|h, o| unsafe {
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
            }),
            Op::Reset(discard) => {
                self.invoke(|h, o| unsafe { shanjie_engine_reset(h, discard as u32, o) })
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
                let s = string(s)?;
                (unsafe { shanjie_engine_set_left_context(self.handle, s.as_ptr()) } == 0)
                    .then(Snapshot::default)
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
        }
    }
    pub fn replay(&mut self, ops: &[crate::broker::Op]) -> bool {
        // Reconstruct only the uncommitted composition after a core restart.
        // Replay must never add the same learning record a second time.
        let learning = self.mutex.is_some();
        unsafe {
            shanjie_engine_set_learning(self.handle, 0);
        }
        let ok = ops.iter().all(|op| self.perform(op.clone()).is_some());
        unsafe {
            shanjie_engine_set_learning(self.handle, learning as u32);
        }
        self.learning_loaded = false;
        ok
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
