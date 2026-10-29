//! Bounded failure metadata only. Callers pass static stage names, never text,
//! paths, request payloads, selections, dictionary entries or error messages.
use std::{collections::HashMap, io::{self, Write}, sync::{Mutex, OnceLock}, time::{Duration, Instant}};
static RECENT: OnceLock<Mutex<HashMap<&'static str, Instant>>> = OnceLock::new();
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
