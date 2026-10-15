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

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Snapshot {
    pub handled: bool,
    pub associated: bool,
    pub prediction: bool,
    pub reading_hint: bool,
    #[serde(default)]
    pub phonetic_pending: bool,
    pub commit: String,
    pub preedit: String,
    pub cursor: u32,
    pub columns: u32,
    pub first: u32,
    pub total: u32,
    pub selected: Option<usize>,
    pub candidates: Vec<String>,
}
pub struct Engine {
    pub handle: *mut ShanjieEngine,
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Failure {
    Busy,
    Disconnected,
    Timeout,
    Ambiguous,
    Failed,
}
// The core owns its allocations/mappings and has no thread-affine UI or COM
// objects. Move exclusive ownership only; never share an Engine concurrently.
// The learning mutex is acquired and released within each operation.
unsafe impl Send for Engine {}
impl Default for Engine {
    fn default() -> Self {
        Self {
            handle: ptr::null_mut(),
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
    pub fn open(
        data: &Path,
        layout: u32,
        ranking: u32,
        right: bool,
        learning: bool,
        big5: bool,
        dictionary: &str,
        prediction: bool,
    ) -> Option<Self> {
        if std::env::var_os("SHANJIE_CORE_LOCAL").is_some() {
            return Self::open_local(
                data, layout, ranking, right, learning, big5, dictionary, prediction,
            );
        }
        let config = crate::broker::Config {
            layout,
            ranking,
            right,
            learning,
            big5,
            dictionary: dictionary.into(),
            prediction,
        };
        let remote = crate::broker::Client::open(data, config)?;
        let mut engine = Self::default();
        engine.remote = Some(remote);
        Some(engine)
    }
    pub fn open_local(
        data: &Path,
        layout: u32,
        ranking: u32,
        right: bool,
        learning: bool,
        big5: bool,
        dictionary: &str,
        prediction: bool,
    ) -> Option<Self> {
        let mut e = Self::default();
        let folder = crate::profile::folder();
        let personal = folder.join(dictionary);
        if dictionary != "dictionary.tsv" && !personal.exists() {
            return None;
        }
        let dictionary = if personal.exists() && personal.metadata().ok()?.len() != 0 {
            Some(CString::new(personal.to_str()?).ok()?)
        } else {
            None
        };
        unsafe {
            if shanjie_windows_engine_new(
                CString::new(data.to_str()?).ok()?.as_ptr(),
                dictionary.as_ref().map_or(ptr::null(), |d| d.as_ptr()),
                layout,
                &mut e.handle,
            ) != 0
            {
                return None;
            }
            e.operation(|h, o| shanjie_engine_set_prediction(h, prediction as u32, o))?;
            let lm = CString::new(data.join("models/model-v5/bigram.winmap").to_str()?).ok()?;
            if shanjie_engine_load_lm(e.handle, lm.as_ptr()) != 0 {
                return None;
            }
            e.operation(|h, o| shanjie_engine_set_profile(h, ranking, o))?;
            if shanjie_windows_set_candidate_anchor(e.handle, right as u32) != 0
                || shanjie_windows_set_candidate_charset(e.handle, big5 as u32) != 0
            {
                return None;
            }
            if learning {
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
            let out = Snapshot {
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
