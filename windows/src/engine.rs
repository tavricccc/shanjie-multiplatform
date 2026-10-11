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

#[derive(Clone, Default)]
pub struct Snapshot {
    pub handled: bool,
    pub associated: bool,
    pub prediction: bool,
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
    mutex: Option<HANDLE>,
    folder: PathBuf,
    epoch: String,
    learning_stamp: Option<(u64, u64)>,
    learning_loaded: bool,
}
impl Default for Engine {
    fn default() -> Self {
        Self {
            handle: ptr::null_mut(),
            mutex: None,
            folder: PathBuf::new(),
            epoch: String::new(),
            learning_stamp: None,
            learning_loaded: false,
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
                prediction: shanjie_windows_is_prediction_row(self.handle) != 0,
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
        self.operation(|h, o| unsafe { shanjie_engine_key(h, k, o) })
    }
    pub fn reset(&mut self, discard: bool) -> Option<Snapshot> {
        // Upstream reset clears composition without learning or persistence.
        // Focus loss must not wait for another App's learning writer.
        self.invoke(|h, o| unsafe { shanjie_engine_reset(h, discard as u32, o) })
    }
}
