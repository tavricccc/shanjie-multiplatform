//! Bounded failure metadata only. Callers pass static stage names, never text,
//! paths, request payloads, selections, dictionary entries or error messages.
use std::{collections::HashMap, io::{self, Write}, sync::{Mutex, OnceLock}, time::{Duration, Instant}};
static RECENT: OnceLock<Mutex<HashMap<&'static str, Instant>>> = OnceLock::new();
/// A bounded lifecycle marker; no key codes, selected words or document text.
pub(crate) fn event(stage: &'static str) { io(stage, &io::Error::other("lifecycle")); }
pub(crate) fn io(stage: &'static str, error: &io::Error) {
    let mut recent = RECENT.get_or_init(Default::default).lock().unwrap_or_else(|e|e.into_inner());
    if recent.get(stage).is_some_and(|t|t.elapsed()<Duration::from_secs(30)) { return; }
    recent.insert(stage, Instant::now());
    let path = std::env::temp_dir().join("shanjie-tsf.log");
    if std::fs::metadata(&path).is_ok_and(|m|m.len()>=256*1024) { return; }
    if let Ok(mut file)=std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let ms=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0,|t|t.as_millis());
        let code=error.raw_os_error().unwrap_or(0);
        let hresult=if code>0 {0x80070000u32|(code as u32&0xffff)} else if code<0 {code as u32} else {0};
        let _=writeln!(file,"{} {ms} version={} stage={stage} kind={:?} win32={code} hresult={hresult:08x}",
            std::process::id(),env!("CARGO_PKG_VERSION"),error.kind());
    }
}
pub(crate) fn at(stage: &'static str) -> impl FnOnce(io::Error)->io::Error {
    move |error| { io(stage,&error); error }
}
/// ShellExecuteW returns a legacy SE_ERR_* code (<= 32) on failure, not a Win32 error.
/// Map it to the Win32 error with the same meaning so the log reads correctly.
pub(crate) fn shell_execute_error(result: isize) -> Option<io::Error> {
    let win32 = match result {
        33.. => return None,
        0 | 8 => 8,          // out of memory
        2 | 3 | 5 => result as i32, // file/path not found, access denied: same values
        11 => 193,           // ERROR_BAD_EXE_FORMAT
        26 => 32,            // SE_ERR_SHARE -> ERROR_SHARING_VIOLATION
        27 | 31 => 1155,     // incomplete/no association -> ERROR_NO_ASSOCIATION
        28 => 1460,          // DDE timeout -> ERROR_TIMEOUT
        32 => 1157,          // SE_ERR_DLLNOTFOUND -> ERROR_DLL_NOT_FOUND
        _ => 1156,           // other DDE failures -> ERROR_DDE_FAIL
    };
    Some(io::Error::from_raw_os_error(win32))
}
#[cfg(test)]
mod tests {
    #[test]
    fn shell_execute_codes_map_to_matching_win32_errors() {
        let code = |r| super::shell_execute_error(r).map(|e| e.raw_os_error().unwrap());
        assert_eq!(code(33), None);
        assert_eq!(code(4096), None);
        for (se, win32) in [(0, 8), (2, 2), (3, 3), (5, 5), (8, 8), (11, 193), (26, 32), (27, 1155), (28, 1460), (29, 1156), (30, 1156), (31, 1155), (32, 1157)] {
            assert_eq!(code(se), Some(win32), "SE_ERR {se}");
        }
    }
}
