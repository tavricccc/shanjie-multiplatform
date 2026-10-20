//! Progress lines for the WinUI setup window. The unelevated window owns the pipe; the
//! elevated installer only writes JSON lines to it, one per step, and a final result.
use super::*;
use std::{io::Write, os::windows::fs::OpenOptionsExt, sync::Mutex};

const PREFIX: &str = r"\\.\pipe\ShanjieSetup-";
/// SECURITY_IDENTIFICATION: the window's pipe server must never impersonate this elevated
/// client. Rust adds SECURITY_SQOS_PRESENT itself.
const IDENTIFICATION: u32 = 0x0001_0000;
static SINK: Mutex<Option<fs::File>> = Mutex::new(None);

pub fn valid(name: &str) -> bool {
    name.strip_prefix(PREFIX).is_some_and(|id| {
        (8..=64).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_hexdigit())
    })
}
pub fn connect(name: &str) -> Result<()> {
    if !valid(name) {
        return Err("安裝進度管道名稱不合法。".into());
    }
    let file = fs::OpenOptions::new()
        .write(true)
        .security_qos_flags(IDENTIFICATION)
        .open(name)?;
    *SINK.lock().unwrap() = Some(file);
    Ok(())
}
pub fn connected() -> bool {
    SINK.lock().unwrap().is_some()
}
fn send(value: serde_json::Value) {
    // Progress is advisory: a closed window must not fail or roll back the install.
    let mut sink = SINK.lock().unwrap();
    if let Some(file) = sink.as_mut() {
        let mut line = value.to_string();
        line.push('\n');
        if file.write_all(line.as_bytes()).and_then(|_| file.flush()).is_err() {
            *sink = None;
        }
    }
}
pub fn step(stage: &str, status: &str, message: &str) {
    send(serde_json::json!({"stage":stage,"status":status,"message":message}));
}
pub fn count(stage: &str, done: usize, total: usize) {
    send(serde_json::json!({"stage":stage,"status":"running","done":done,"total":total}));
}
/// `status` is succeeded, cancelled or failed; the window shows `message` as the outcome.
pub fn finish(status: &str, message: &str) {
    step("Result", status, message);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Read, os::windows::io::FromRawHandle};
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreateNamedPipeW(
            name: *const u16,
            open: u32,
            mode: u32,
            instances: u32,
            out: u32,
            input: u32,
            timeout: u32,
            security: *const std::ffi::c_void,
        ) -> *mut std::ffi::c_void;
    }
    #[test]
    fn only_setup_pipes_with_hex_ids_are_accepted() {
        assert!(valid(r"\\.\pipe\ShanjieSetup-0123abcdEF"));
        for name in [
            r"\\.\pipe\ShanjieSetup-",
            r"\\.\pipe\ShanjieSetup-1234",
            r"\\.\pipe\ShanjieSetup-0123abcd\..\other",
            r"\\.\pipe\Other-0123abcdef",
            r"C:\Windows\System32\drivers\etc\hosts",
            r"\\server\pipe\ShanjieSetup-0123abcdef",
        ] {
            assert!(!valid(name), "{name}");
            assert!(connect(name).is_err(), "{name}");
        }
    }
    #[test]
    fn steps_arrive_as_json_lines() {
        let name = format!("{PREFIX}{:x}{:08x}", std::process::id(), 0x5eed_u32);
        let wide = crate::wide(&name);
        // PIPE_ACCESS_INBOUND, byte mode, one instance.
        let raw = unsafe {
            CreateNamedPipeW(wide.as_ptr(), 1, 0, 1, 4096, 4096, 0, std::ptr::null())
        };
        assert!(!raw.is_null() && raw as isize != -1);
        let mut server = unsafe { fs::File::from_raw_handle(raw) };
        connect(&name).unwrap();
        step("Copy", "started", "");
        count("Copy", 3, 9);
        finish("succeeded", "已安裝");
        *SINK.lock().unwrap() = None;
        let mut text = String::new();
        server.read_to_string(&mut text).unwrap();
        let lines: Vec<serde_json::Value> =
            text.lines().map(|l| serde_json::from_str(l).unwrap()).collect();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[1]["done"], 3);
        assert_eq!(lines[2]["stage"], "Result");
        assert_eq!(lines[2]["message"], "已安裝");
    }
}
