//! The core, in process. The IBus engine process is itself resident (one per login, shared by every
//! input context), so Linux needs only the local half of windows/src/engine.rs. Learning keeps the
//! Windows rules: one lock per profile folder held around each operation, `learning.epoch` to see a
//! clear from the settings app, and the learning file's stamp to see another writer.
use shanjie_core::ffi::*;
use shanjie_shell::protocol::{Failure, Op, Snapshot};
use shanjie_linux::lock;
use std::{
    ffi::{CStr, CString},
    fs::File,
    path::PathBuf,
    ptr,
    time::{Duration, UNIX_EPOCH},
};

/// Same bound as the Windows learning mutex wait.
const LOCK_WAIT: Duration = Duration::from_millis(250);

#[derive(Clone, PartialEq, Eq)]
pub struct Config {
    pub data: PathBuf,
    pub folder: PathBuf,
    /// The resolved main model; its side files sit beside it.
    pub model: PathBuf,
    pub engine: shanjie_shell::protocol::Config,
}

pub struct Engine {
    handle: *mut ShanjieEngine,
    lock: Option<File>,
    folder: PathBuf,
    epoch: String,
    learning_stamp: Option<(u128, u64)>,
    learning_loaded: bool,
    failure: Option<Failure>,
}

// The core owns its allocations and mappings and has no thread-affine state; an Engine moves
// between threads but is never shared (every session keeps it behind its own lock).
unsafe impl Send for Engine {}
unsafe impl Sync for Engine {}

impl Drop for Engine {
    fn drop(&mut self) {
        unsafe { shanjie_engine_free(self.handle) };
    }
}

impl Engine {
    pub fn open(config: &Config) -> Option<Self> {
        let mut e = Self {
            handle: ptr::null_mut(),
            lock: None,
            folder: PathBuf::new(),
            epoch: String::new(),
            learning_stamp: None,
            learning_loaded: false,
            failure: None,
        };
        let (engine, model) = (&config.engine, &config.model);
        let personal = config.folder.join(&engine.dictionary);
        if engine.dictionary != "dictionary.tsv" && !personal.exists() {
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
                CString::new(config.data.to_str()?).ok()?.as_ptr(),
                dictionary.as_ref().map_or(ptr::null(), |d| d.as_ptr()),
                engine.layout,
                engine.packs(),
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
            if let Some(list) = config.data.join("english-words.txt").to_str().and_then(|p| CString::new(p).ok()) {
                shanjie_engine_load_english(e.handle, list.as_ptr());
            }
            // A new engine starts with the core's defaults, so every switch is sent each time.
            e.operation(|h, o| shanjie_engine_set_prediction(h, engine.prediction as u32, o))?;
            e.operation(|h, o| shanjie_engine_set_english_key_order(h, engine.english_key_order as u32, o))?;
            e.operation(|h, o| shanjie_engine_set_fuzzy(h, engine.fuzzy, o))?;
            e.operation(|h, o| shanjie_engine_set_rerank(h, engine.rerank as u32, o))?;
            e.operation(|h, o| shanjie_engine_set_rerank_level(h, engine.rerank_level, o))?;
            e.operation(|h, o| shanjie_engine_set_profile(h, engine.ranking, o))?;
            if shanjie_windows_set_candidate_anchor(e.handle, engine.right as u32) != 0
                || shanjie_windows_set_candidate_charset(e.handle, engine.big5 as u32) != 0
            {
                return None;
            }
            if engine.learning {
                e.lock = Some(lock::open(&config.folder, lock::LEARNING).ok()?);
                e.folder = config.folder.clone();
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
        let Some(lock) = self.lock.as_ref() else { return true };
        if !lock::acquire(lock, LOCK_WAIT) {
            self.failure = Some(Failure::Busy);
            return false;
        }
        let epoch = std::fs::read_to_string(self.folder.join("learning.epoch")).unwrap_or_default().trim().to_owned();
        unsafe {
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
            let folder = CString::new(self.folder.to_str().unwrap_or_default()).unwrap_or_default();
            if shanjie_engine_learning_open(self.handle, folder.as_ptr()) == 0 {
                self.learning_loaded = true;
                self.learning_stamp = self.store_stamp();
                true
            } else {
                lock::release(lock);
                false
            }
        }
    }

    fn store_stamp(&self) -> Option<(u128, u64)> {
        let meta = std::fs::metadata(self.folder.join("learning.tsv")).ok()?;
        let modified = meta.modified().ok()?.duration_since(UNIX_EPOCH).ok()?.as_nanos();
        Some((modified, meta.len()))
    }

    fn end(&mut self) {
        if let Some(lock) = self.lock.as_ref() {
            // Capture our own writes while holding the same lock; a peer write or clear is
            // detected before the next operation.
            self.learning_stamp = self.store_stamp();
            lock::release(lock);
        }
    }

    fn operation(&mut self, f: impl FnOnce(*mut ShanjieEngine, *mut *mut ShanjieOutput) -> i32) -> Option<Snapshot> {
        if self.handle.is_null() || !self.begin() {
            return None;
        }
        let out = self.invoke(f);
        self.end();
        out
    }

    fn invoke(&mut self, f: impl FnOnce(*mut ShanjieEngine, *mut *mut ShanjieOutput) -> i32) -> Option<Snapshot> {
        if self.handle.is_null() {
            return None;
        }
        let mut raw = ptr::null_mut();
        if f(self.handle, &mut raw) != 0 {
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
                prediction: shanjie_windows_is_prediction_row(self.handle) != 0 || shanjie_windows_is_mixed_hint(self.handle) != 0,
                reading_hint: shanjie_windows_is_mixed_hint(self.handle) != 0,
                phonetic_pending: shanjie_windows_phonetic_pending(self.handle) != 0,
                candidates: (0..r.candidate_count).map(|i| string(*r.candidates.add(i as usize))).collect(),
                associated: false,
            };
            shanjie_output_free(raw);
            // The engine process is the resident core; like Windows, a queued rerank runs here, between
            // keys. The upstream generation guard drops a stale result.
            let mut job = ptr::null_mut();
            if shanjie_engine_rerank_take(self.handle, &mut job) == 0 && !job.is_null() {
                let mut ranked = ptr::null_mut();
                if shanjie_rerank_run(job) == 0 && shanjie_engine_rerank_apply(self.handle, job, &mut ranked) == 0 && !ranked.is_null() {
                    let r = &*ranked;
                    out.preedit = string(r.preedit);
                    out.cursor = r.cursor_utf16;
                    out.candidates = (0..r.candidate_count).map(|i| string(*r.candidates.add(i as usize))).collect();
                    out.selected = usize::try_from(r.candidate_selected).ok();
                    out.columns = r.candidate_columns;
                    out.first = r.candidate_first;
                    out.total = r.candidate_total;
                    shanjie_output_free(ranked);
                }
                shanjie_rerank_free(job);
            }
            Some(out)
        }
    }

    pub fn key(&mut self, k: ShanjieKey) -> Option<Snapshot> {
        self.key_with_latin(k, None)
    }

    pub fn key_with_latin(&mut self, k: ShanjieKey, latin: Option<char>) -> Option<Snapshot> {
        self.perform(Op::Key { kind: k.kind, ch: k.ch, modifiers: k.modifiers, latin })
    }

    pub fn mixed_pending(&self) -> bool {
        unsafe { shanjie_windows_is_mixed_pending(self.handle) != 0 }
    }

    pub fn reset(&mut self, discard: bool) -> Option<Snapshot> {
        // Upstream reset clears the composition without learning or persistence.
        self.perform(Op::Reset(discard))
    }

    pub fn failure(&self) -> Option<Failure> {
        self.failure
    }

    pub fn perform(&mut self, op: Op) -> Option<Snapshot> {
        self.failure = None;
        let string = |s: String| CString::new(s).ok();
        let result = match op {
            Op::Key { kind, ch, modifiers, latin } => self.operation(|h, o| unsafe {
                shanjie_windows_key_with_latin(h, ShanjieKey { kind, ch, modifiers }, latin.map_or(0, |c| c as u32), o)
            }),
            Op::Reset(discard) => self.invoke(|h, o| unsafe { shanjie_engine_reset(h, discard as u32, o) }),
            Op::Pick(index) => self.operation(|h, o| unsafe { shanjie_engine_pick(h, index, o) }),
            Op::Associate(s) => {
                let s = string(s)?;
                self.operation(|h, o| unsafe { shanjie_windows_associate(h, s.as_ptr(), o) })
            }
            Op::Reading => self.operation(|h, o| unsafe { shanjie_windows_reading(h, 1, o) }),
            Op::Prediction(on) => self.operation(|h, o| unsafe { shanjie_engine_set_prediction(h, on as u32, o) }),
            Op::Demote(on) => self.operation(|h, o| unsafe { shanjie_engine_set_demote(h, on as u32, o) }),
            Op::Punctuation(s) => {
                let s = string(s)?;
                self.operation(|h, o| unsafe { shanjie_windows_punctuation(h, s.as_ptr(), o) })
            }
            Op::Cycle => self.operation(|h, o| unsafe { shanjie_windows_cycle_candidate(h, o) }),
            Op::Space => self.operation(|h, o| unsafe { shanjie_windows_insert_space(h, o) }),
            Op::Context(s) => {
                let s = string(s)?;
                (unsafe { shanjie_engine_set_left_context(self.handle, s.as_ptr()) } == 0).then(Snapshot::default)
            }
            Op::Options(flags) => (unsafe { shanjie_windows_set_input_options(self.handle, flags) } == 0).then(Snapshot::default),
            Op::Mixed(on) => (unsafe { shanjie_windows_set_mixed_input(self.handle, on as u32) } == 0).then(Snapshot::default),
            Op::Tolerance(on) => (unsafe { shanjie_windows_set_mixed_tolerance(self.handle, on as u32) } == 0).then(Snapshot::default),
            Op::English(on) => (unsafe { shanjie_windows_set_upstream_english(self.handle, on) } == 0).then(Snapshot::default),
        };
        if result.is_none() && self.failure.is_none() {
            self.failure = Some(Failure::Failed);
        }
        result
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
