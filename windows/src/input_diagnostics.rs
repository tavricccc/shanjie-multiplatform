//! Opt-in, short-lived metadata capture. Never accepts document text as an event field.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    cell::RefCell,
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use windows::Win32::System::Threading::GetCurrentThreadId;

pub const BUILD: &str = match option_env!("SHANJIE_DIAGNOSTICS_BUILD") {
    Some(value) => value,
    None => "0.12.1-input-diagnostics-dev",
};
pub const MAX_DURATION_MS: u64 = 300_000;
const MAX_LOG_BYTES: u64 = 2 * 1024 * 1024;
static SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Session {
    pub schema: u32,
    pub id: String,
    pub mode: String,
    pub started_ms: u64,
    pub expires_ms: u64,
    pub active: bool,
}
impl Session {
    fn safe_location(&self) -> bool {
        self.schema == 1
            && self.id.len() == 32
            && self
                .id
                .bytes()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
            && matches!(self.mode.as_str(), "shift" | "excel" | "word")
    }
    pub fn valid(&self, now: u64) -> bool {
        self.safe_location()
            && self.active
            && self.started_ms <= now
            && now < self.expires_ms
            && self.expires_ms.saturating_sub(self.started_ms) <= MAX_DURATION_MS
    }
}
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
pub fn root() -> PathBuf {
    crate::profile::folder().join("InputDiagnostics")
}
pub fn reports() -> PathBuf {
    root().join("reports")
}
pub fn report(session: &Session) -> PathBuf {
    reports().join(&session.id)
}
pub fn load_at(root: &Path) -> Option<Session> {
    let file = fs::File::open(root.join("control.json")).ok()?;
    if file.metadata().ok()?.len() > 1024 {
        return None;
    }
    let mut bytes = Vec::new();
    file.take(1025).read_to_end(&mut bytes).ok()?;
    serde_json::from_slice(&bytes).ok()
}
fn write_control(session: &Session) -> std::io::Result<()> {
    use windows::{
        core::PCWSTR,
        Win32::Storage::FileSystem::{
            MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
        },
    };
    fs::create_dir_all(root())?;
    let temporary = root().join(format!("control-{}.tmp", std::process::id()));
    let mut file = fs::File::create(&temporary)?;
    file.write_all(&serde_json::to_vec(session)?)?;
    file.sync_all()?;
    drop(file);
    let source = crate::wide(&temporary.to_string_lossy());
    let target = crate::wide(&root().join("control.json").to_string_lossy());
    unsafe {
        MoveFileExW(
            PCWSTR(source.as_ptr()),
            PCWSTR(target.as_ptr()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    }
    .map_err(std::io::Error::other)
}
pub fn start(mode: &str) -> std::io::Result<Session> {
    if !matches!(mode, "shift" | "excel" | "word") {
        return Err(std::io::Error::other("未知的測試種類"));
    }
    if load_at(&root()).is_some_and(|s| s.valid(now_ms())) {
        return Err(std::io::Error::other("已有測試正在紀錄，請先停止。"));
    }
    use sha2::{Digest, Sha256};
    let now = now_ms();
    let entropy = format!("{now}-{}-{:?}", std::process::id(), Instant::now());
    let id = format!("{:x}", Sha256::digest(entropy.as_bytes()))[..32].to_owned();
    let session = Session {
        schema: 1,
        id,
        mode: mode.into(),
        started_ms: now,
        expires_ms: now + MAX_DURATION_MS,
        active: true,
    };
    fs::create_dir_all(report(&session))?;
    fs::write(
        report(&session).join("session.json"),
        serde_json::to_vec_pretty(&json!({
            "session":session,"diagnostics_build":BUILD,"base_version":env!("CARGO_PKG_VERSION"),
            "privacy":"No document text, clipboard, window titles, dictionaries or learned data",
            "keys":"Shift/Control/Alt/Win, PROCESS_KEY, 1/2/3, arrows, F2, Enter/Escape/Tab/Backspace; all other keys are OTHER"
        }))?,
    )?;
    write_control(&session)?;
    Ok(session)
}
pub fn stop() -> std::io::Result<Option<Session>> {
    let Some(mut session) = load_at(&root()) else {
        return Ok(None);
    };
    if !session.safe_location() {
        return Err(std::io::Error::other("無效的診斷控制檔"));
    }
    session.active = false;
    write_control(&session)?;
    // Give positive caches time to observe stop before presenting completed logs.
    std::thread::sleep(Duration::from_millis(150));
    fs::write(
        report(&session).join("stopped.json"),
        serde_json::to_vec(&json!({"stopped_ms":now_ms()}))?,
    )?;
    Ok(Some(session))
}
pub fn allowed(mode: &str, process: &str) -> bool {
    let process = process.to_ascii_lowercase();
    match mode {
        "shift" => matches!(
            process.as_str(),
            "ayugram.exe" | "explorer.exe" | "notepad.exe"
        ),
        "excel" => process == "excel.exe",
        "word" => matches!(process.as_str(), "ayugram.exe" | "explorer.exe" | "notepad.exe" | "excel.exe"),
        _ => false,
    }
}
pub fn key_name(key: usize) -> &'static str {
    match key {
        16 => "SHIFT",
        160 => "LEFT_SHIFT",
        161 => "RIGHT_SHIFT",
        17 | 162 | 163 => "CONTROL",
        18 | 164 | 165 => "ALT",
        91 | 92 => "WIN",
        229 => "PROCESS_KEY",
        49 => "1",
        50 => "2",
        51 => "3",
        37 => "LEFT",
        38 => "UP",
        39 => "RIGHT",
        40 => "DOWN",
        113 => "F2",
        13 => "ENTER",
        27 => "ESCAPE",
        9 => "TAB",
        8 => "BACKSPACE",
        _ => "OTHER",
    }
}
/// Classify transport flags without retaining arbitrary virtual keys or scan codes.
pub fn key_metadata(key: usize, flags: isize) -> Value {
    json!({"key":key_name(key),"is_process_key":key == 229,
        "scan_present":(flags >> 16) & 255 != 0,"extended":flags & (1 << 24) != 0,
        "previous_down":flags & (1 << 30) != 0,"transition_up":flags & (1isize << 31) != 0})
}
struct Capture {
    session: Session,
    deadline: Instant,
    path: PathBuf,
    bytes: u64,
}
#[derive(Default)]
struct Cache {
    checked: Option<Instant>,
    capture: Option<Capture>,
}
thread_local! { static CACHE: RefCell<Cache> = RefCell::new(Cache::default()); }

fn with_capture<T>(operation: impl FnOnce(&mut Capture) -> T) -> Option<T> {
    CACHE.with(|cache| {
        let mut cache = cache.try_borrow_mut().ok()?;
        if cache
            .checked
            .is_none_or(|t| t.elapsed() >= Duration::from_millis(100))
        {
            cache.checked = Some(Instant::now());
            let active = load_at(&root()).filter(|s| s.valid(now_ms()));
            match active {
                Some(session)
                    if cache
                        .capture
                        .as_ref()
                        .is_some_and(|c| c.session.id == session.id) => {}
                Some(session) => {
                    let process = std::env::current_exe()
                        .ok()?
                        .file_name()?
                        .to_string_lossy()
                        .into_owned();
                    if !allowed(&session.mode, &process) {
                        cache.capture = None;
                        return None;
                    }
                    let path = report(&session).join(format!(
                        "events-{}-{}-{}.jsonl",
                        process,
                        std::process::id(),
                        unsafe { GetCurrentThreadId() }
                    ));
                    let bytes = fs::metadata(&path).map_or(0, |m| m.len());
                    let deadline = Instant::now()
                        + Duration::from_millis(
                            session
                                .expires_ms
                                .saturating_sub(now_ms())
                                .min(MAX_DURATION_MS),
                        );
                    cache.capture = Some(Capture {
                        session,
                        deadline,
                        path,
                        bytes,
                    });
                }
                None => cache.capture = None,
            }
        }
        let capture = cache.capture.as_mut()?;
        if Instant::now() >= capture.deadline
            || !capture.session.valid(now_ms())
            || capture.bytes >= MAX_LOG_BYTES
        {
            return None;
        }
        Some(operation(capture))
    })
}
pub fn enabled() -> bool {
    with_capture(|_| ()).is_some()
}
/// Callers pass only a fixed event name and numeric/boolean metadata, never user strings.
pub fn emit(event: &'static str, fields: Value) {
    let _ = with_capture(|capture| {
        let entry = json!({"schema":1,"event":event,"time_ms":now_ms(),"sequence":SEQUENCE.fetch_add(1,Ordering::Relaxed),
            "pid":std::process::id(),"tid":unsafe{GetCurrentThreadId()},"architecture":std::env::consts::ARCH,
            "diagnostics_build":BUILD,"fields":fields});
        if let Ok(mut bytes) = serde_json::to_vec(&entry) {
            bytes.push(b'\n');
            if capture.bytes + bytes.len() as u64 > MAX_LOG_BYTES {
                return;
            }
            if let Ok(mut file) = OpenOptions::new()
                .append(true)
                .create(true)
                .open(&capture.path)
            {
                if file.write_all(&bytes).is_ok() {
                    capture.bytes += bytes.len() as u64;
                }
            }
        }
    });
}
#[no_mangle]
pub extern "C" fn sj_input_diagnostics_version() -> u32 {
    1
}

#[cfg(test)]
mod tests {
    use super::*;
    fn session() -> Session {
        Session {
            schema: 1,
            id: "0123456789abcdef0123456789abcdef".into(),
            mode: "shift".into(),
            started_ms: 100,
            expires_ms: 200,
            active: true,
        }
    }
    #[test]
    fn session_requires_explicit_valid_bounded_activation() {
        let s = session();
        assert!(s.valid(100));
        assert!(!s.valid(99));
        assert!(!s.valid(200));
        for alter in 0..6 {
            let mut bad = s.clone();
            match alter {
                0 => bad.active = false,
                1 => bad.id = "../outside".into(),
                2 => bad.expires_ms = 100 + MAX_DURATION_MS + 1,
                3 => bad.mode = "all".into(),
                4 => bad.schema = 2,
                _ => bad.id = "A".repeat(32),
            };
            assert!(!bad.valid(100));
        }
    }
    #[test]
    fn host_and_key_allowlists_do_not_capture_arbitrary_input() {
        assert!(allowed("shift", "AyuGram.exe"));
        assert!(allowed("shift", "explorer.exe"));
        assert!(allowed("excel", "EXCEL.EXE"));
        assert!(!allowed("shift", "EXCEL.EXE"));
        assert!(!allowed("excel", "notepad.exe"));
        assert!(!allowed("shift", "password-manager.exe"));
        for process in ["AyuGram.exe","explorer.exe","notepad.exe","EXCEL.EXE"] {
            assert!(allowed("word",process));
        }
        assert!(!allowed("word","password-manager.exe"));
        let mut word=session();word.mode="word".into();assert!(word.valid(100));
        for k in b'A'..=b'Z' {
            assert_eq!(key_name(k as usize), "OTHER");
        }
        assert_eq!(key_name(b'9' as usize), "OTHER");
        assert_eq!(key_name(161), "RIGHT_SHIFT");
        assert_eq!(key_name(229), "PROCESS_KEY");
    }
    #[test]
    fn message_metadata_redacts_unlisted_keys_and_scan_codes() {
        let first = key_metadata(b'A' as usize, 0x1e << 16);
        let second = key_metadata(b'Z' as usize, 0x2c << 16);
        assert_eq!(first, second);
        assert_eq!(first["key"], "OTHER");
        assert_eq!(first["scan_present"], true);
        let proxy = key_metadata(229, (1isize << 31) | (1 << 30));
        assert_eq!(proxy["is_process_key"], true);
        assert_eq!(proxy["transition_up"], true);
        assert_eq!(proxy["previous_down"], true);
        assert_eq!(proxy["scan_present"], false);
    }
    #[test]
    fn unknown_control_fields_are_rejected() {
        let mut v = serde_json::to_value(session()).unwrap();
        v["path"] = json!("C:/private");
        assert!(serde_json::from_value::<Session>(v).is_err());
    }
}
