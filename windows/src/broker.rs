//! Per-user, per-session resident core. The pipe transports composition state;
//! it never writes keystrokes to disk. Immutable data is shared inside the core.
use crate::engine::{Engine, Failure, Snapshot};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    ffi::c_void,
    fs::File,
    io::{self, Read, Write},
    os::windows::io::{AsRawHandle, FromRawHandle},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
type Raw = *mut c_void;
#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateNamedPipeW(
        name: *const u16,
        access: u32,
        mode: u32,
        instances: u32,
        out: u32,
        input: u32,
        timeout: u32,
        security: *const Security,
    ) -> Raw;
    fn ConnectNamedPipe(pipe: Raw, overlapped: Raw) -> i32;
    fn GetNamedPipeServerProcessId(pipe: Raw, pid: *mut u32) -> i32;
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> Raw;
    fn QueryFullProcessImageNameW(
        process: Raw,
        flags: u32,
        path: *mut u16,
        length: *mut u32,
    ) -> i32;
    fn GetCurrentProcess() -> Raw;
    fn GetCurrentProcessId() -> u32;
    fn ProcessIdToSessionId(pid: u32, session: *mut u32) -> i32;
    fn CloseHandle(handle: Raw) -> i32;
    fn LocalFree(memory: Raw) -> Raw;
}
#[link(name = "advapi32")]
unsafe extern "system" {
    fn OpenProcessToken(process: Raw, access: u32, token: *mut Raw) -> i32;
    fn GetTokenInformation(
        token: Raw,
        class: u32,
        buffer: Raw,
        length: u32,
        needed: *mut u32,
    ) -> i32;
    fn ConvertSidToStringSidW(sid: Raw, text: *mut *mut u16) -> i32;
    fn ConvertStringSecurityDescriptorToSecurityDescriptorW(
        text: *const u16,
        revision: u32,
        descriptor: *mut Raw,
        size: *mut u32,
    ) -> i32;
}
#[repr(C)]
struct Security {
    length: u32,
    descriptor: Raw,
    inherit: i32,
}
pub(crate) fn user_sid() -> io::Result<String> {
    unsafe {
        let mut token = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), 8, &mut token) == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut needed = 0;
        GetTokenInformation(token, 1, std::ptr::null_mut(), 0, &mut needed);
        // usize alignment is required by TOKEN_USER's embedded SID pointer.
        let mut buf = vec![0usize; (needed as usize + 7) / std::mem::size_of::<usize>()];
        let ok = GetTokenInformation(
            token,
            1,
            buf.as_mut_ptr().cast(),
            (buf.len() * std::mem::size_of::<usize>()) as u32,
            &mut needed,
        );
        CloseHandle(token);
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        let sid = *(buf.as_ptr() as *const Raw);
        let mut text = std::ptr::null_mut();
        if ConvertSidToStringSidW(sid, &mut text) == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut n = 0;
        while *text.add(n) != 0 {
            n += 1;
        }
        let result = String::from_utf16_lossy(std::slice::from_raw_parts(text, n));
        LocalFree(text.cast());
        Ok(result)
    }
}
/// Sandboxed WinUI/Store hosts cannot spawn or inspect the medium-integrity core.
pub(crate) fn app_container() -> bool {
    unsafe {
        let mut token = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), 8, &mut token) == 0 {
            return false;
        }
        let mut value = 0u32;
        let mut needed = 0;
        // TokenIsAppContainer = 29.
        let ok = GetTokenInformation(token, 29, (&mut value as *mut u32).cast(), 4, &mut needed);
        CloseHandle(token);
        ok != 0 && value != 0
    }
}
/// An elevated host (installer dialog, admin console) must not start the core: the core
/// would inherit the admin token and every ordinary app would then talk to it.
pub(crate) fn elevated() -> bool {
    unsafe {
        let mut token = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), 8, &mut token) == 0 {
            return false;
        }
        let mut value = 0u32;
        let mut needed = 0;
        // TokenElevation = 20.
        let ok = GetTokenInformation(token, 20, (&mut value as *mut u32).cast(), 4, &mut needed);
        CloseHandle(token);
        ok != 0 && value != 0
    }
}
fn executable(data: &Path) -> PathBuf {
    data.parent().unwrap_or(data).join("tools/shanjie-core.exe")
}
fn endpoint(data: &Path) -> io::Result<String> {
    let exe = executable(data).canonicalize()?;
    let mut session = 0;
    unsafe {
        if ProcessIdToSessionId(GetCurrentProcessId(), &mut session) == 0 {
            return Err(io::Error::last_os_error());
        }
    }
    let mut hash = Sha256::new();
    hash.update(exe.to_string_lossy().to_lowercase());
    // Replacing a binary in an existing output directory must not reuse an old core.
    hash.update(std::fs::read(&exe)?);
    hash.update(crate::profile::folder().to_string_lossy().to_lowercase());
    Ok(format!(
        "\\\\.\\pipe\\Shanjie-{}-{session}-{:x}",
        user_sid()?,
        hash.finalize()
    ))
}
fn write<T: Serialize>(pipe: &mut File, value: &T) -> io::Result<()> {
    let bytes = serde_json::to_vec(value)?;
    if bytes.len() > 1024 * 1024 {
        return Err(io::Error::other("Core message too large"));
    }
    pipe.write_all(&(bytes.len() as u32).to_le_bytes())?;
    pipe.write_all(&bytes)?;
    pipe.flush()
}
fn read<T: DeserializeOwned>(pipe: &mut File) -> io::Result<T> {
    let mut size = [0; 4];
    pipe.read_exact(&mut size)?;
    let size = u32::from_le_bytes(size) as usize;
    if size > 1024 * 1024 {
        return Err(io::Error::other("Core message too large"));
    }
    let mut bytes = vec![0; size];
    pipe.read_exact(&mut bytes)?;
    Ok(serde_json::from_slice(&bytes)?)
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct Config {
    pub layout: u32,
    pub ranking: u32,
    pub right: bool,
    pub learning: bool,
    pub big5: bool,
    pub dictionary: String,
    pub prediction: bool,
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub enum Op {
    Key {
        kind: u32,
        ch: u32,
        modifiers: u32,
        latin: Option<char>,
    },
    Reset(bool),
    Pick(u32),
    Associate(String),
    Reading,
    Prediction(bool),
    Demote(bool),
    Punctuation(String),
    Cycle,
    Space,
    Context(String),
    Options(u32),
    Mixed(bool),
    English(u32),
}
#[derive(Serialize, Deserialize)]
enum Request {
    Open {
        id: String,
        config: Config,
        replay: Vec<Op>,
        sequence: u64,
    },
    Resume(String),
    Apply {
        sequence: u64,
        op: Op,
    },
    Close,
    Status,
    Stop,
    Profile,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Reply {
    pub snapshot: Option<Snapshot>,
    pub pending: bool,
    pub ok: bool,
    pub pid: u32,
    pub lexicon_loads: u64,
    pub model_loads: u64,
    pub sequence: u64,
    pub found: bool,
    pub failure: Option<Failure>,
    /// Profile text for sandboxed hosts, which cannot read the user's folder.
    #[serde(default)]
    pub profile: Option<String>,
}
fn reply(engine: Option<&Engine>, snapshot: Option<Snapshot>, ok: bool) -> Reply {
    Reply {
        snapshot,
        pending: engine.is_some_and(Engine::mixed_pending),
        ok,
        pid: std::process::id(),
        lexicon_loads: shanjie_core::ffi::shanjie_windows_resource_loads(0),
        model_loads: shanjie_core::ffi::shanjie_windows_resource_loads(1),
        sequence: 0,
        found: false,
        failure: if ok { None } else { Some(Failure::Failed) },
        profile: None,
    }
}
fn connect(data: &Path, start: bool, deadline: Instant) -> io::Result<crate::pipe_io::Pipe> {
    use std::os::windows::fs::OpenOptionsExt;
    let name = endpoint(data)?;
    let expected = executable(data).canonicalize()?;
    let mut launched = false;
    let mut full_since: Option<Instant> = None;
    loop {
        match std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(0x40000000)
            .open(&name)
        {
            Ok(file) => unsafe {
                let mut pid = 0;
                if GetNamedPipeServerProcessId(file.as_raw_handle(), &mut pid) == 0 {
                    return Err(io::Error::last_os_error());
                }
                let process = OpenProcess(0x1000, 0, pid);
                if process.is_null() {
                    // The core's default DACL excludes AppContainers; the pipe DACL still
                    // limits servers to this user, so sandboxed hosts trust the endpoint.
                    if app_container() {
                        return Ok(crate::pipe_io::Pipe(file));
                    }
                    return Err(io::Error::last_os_error());
                }
                let mut path = vec![0u16; 32768];
                let mut len = path.len() as u32;
                let ok = QueryFullProcessImageNameW(process, 0, path.as_mut_ptr(), &mut len);
                CloseHandle(process);
                if ok == 0
                    || PathBuf::from(String::from_utf16_lossy(&path[..len as usize]))
                        .canonicalize()?
                        != expected
                {
                    return Err(io::Error::other("Unexpected core server"));
                }
                return Ok(crate::pipe_io::Pipe(file));
            },
            Err(error) => {
                if error.raw_os_error() == Some(231) {
                    let since = *full_since.get_or_insert_with(Instant::now);
                    if since.elapsed() >= Duration::from_secs(1) {
                        return Err(error);
                    }
                }
                if ![Some(2), Some(231)].contains(&error.raw_os_error())
                    || Instant::now() >= deadline
                {
                    return Err(error);
                }
                if start
                    && !launched
                    && error.raw_os_error() == Some(2)
                    && !app_container()
                    && !elevated()
                {
                    start_core(&expected)?;
                    launched = true;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
}
/// After an update: stop this session's cores from retired versions and start the current
/// one, so sandboxed hosts (which cannot start a core) find it. Runs unelevated.
pub fn retire_and_start(data: &Path) -> io::Result<()> {
    if elevated() {
        return Err(io::Error::other("Elevated process cannot start the user core"));
    }
    let current = executable(data).canonicalize()?;
    let mut session = 0;
    unsafe {
        if ProcessIdToSessionId(GetCurrentProcessId(), &mut session) == 0 {
            return Err(io::Error::last_os_error());
        }
    }
    let prefix = format!("Shanjie-{}-{session}-", user_sid()?);
    let ours = endpoint(data)?;
    for entry in std::fs::read_dir(r"\\.\pipe\")?.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = format!(r"\\.\pipe\{name}");
        if !name.starts_with(&prefix) || path == ours {
            continue;
        }
        let _ = stop_retired(&path, &current);
    }
    let deadline = Instant::now() + Duration::from_secs(5);
    connect(data, true, deadline).map(|_| ())
}
fn stop_retired(path: &str, current: &Path) -> io::Result<()> {
    use std::os::windows::fs::OpenOptionsExt;
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(0x40000000)
        .open(path)?;
    let image = unsafe {
        let mut pid = 0;
        if GetNamedPipeServerProcessId(file.as_raw_handle(), &mut pid) == 0 {
            return Err(io::Error::last_os_error());
        }
        let process = OpenProcess(0x1000, 0, pid);
        if process.is_null() {
            return Err(io::Error::last_os_error());
        }
        let mut buffer = vec![0u16; 32768];
        let mut len = buffer.len() as u32;
        let ok = QueryFullProcessImageNameW(process, 0, buffer.as_mut_ptr(), &mut len);
        CloseHandle(process);
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        PathBuf::from(String::from_utf16_lossy(&buffer[..len as usize]))
    };
    // Only Shanjie cores, never the current one or an unrelated server on a similar name.
    if image.file_name().is_none_or(|n| !n.eq_ignore_ascii_case("shanjie-core.exe"))
        || image.canonicalize().ok().as_deref() == Some(current)
    {
        return Ok(());
    }
    let pipe = crate::pipe_io::Pipe(file);
    let deadline = Instant::now() + Duration::from_secs(1);
    pipe.send(&Request::Stop, deadline)?;
    let _ = pipe.receive::<Reply>(deadline);
    Ok(())
}
/// Sandboxed hosts read the profile through the already running core.
pub(crate) fn remote_profile() -> Option<String> {
    let data = crate::tsf::binary()?.join("data");
    let deadline = Instant::now() + Duration::from_millis(500);
    let pipe = connect(&data, false, deadline).ok()?;
    pipe.send(&Request::Profile, deadline).ok()?;
    pipe.receive::<Reply>(deadline).ok()?.profile
}
fn start_core(executable: &Path) -> io::Result<()> {
    use windows::{
        core::{PCWSTR, PWSTR},
        Win32::System::Threading::*,
    };
    let application = crate::wide(&executable.to_string_lossy());
    let mut command = crate::wide(&format!("\"{}\" --serve", executable.display()));
    // The long-lived core must not inherit the caller's directory (an installer run from a
    // package folder would otherwise keep that folder locked for as long as the core lives).
    let directory = crate::wide(
        executable
            .parent()
            .unwrap_or(executable)
            .to_string_lossy()
            .trim_start_matches(r"\?\"),
    );
    let startup = STARTUPINFOW {
        cb: std::mem::size_of::<STARTUPINFOW>() as u32,
        ..Default::default()
    };
    let mut process = PROCESS_INFORMATION::default();
    unsafe {
        // Never inherit an application's handles, console or capture pipes.
        CreateProcessW(
            PCWSTR(application.as_ptr()),
            Some(PWSTR(command.as_mut_ptr())),
            None,
            None,
            false,
            CREATE_NO_WINDOW,
            None,
            PCWSTR(directory.as_ptr()),
            &startup,
            &mut process,
        )
        .map_err(|e| io::Error::other(e.to_string()))?;
        CloseHandle(process.hThread.0);
        CloseHandle(process.hProcess.0);
    }
    Ok(())
}
mod client;
mod session;
pub mod verification;
pub use client::Client;
pub fn control(data: &Path, stop: bool) -> io::Result<Reply> {
    let deadline = Instant::now() + Duration::from_secs(1);
    let pipe = connect(data, false, deadline)?;
    pipe.send(
        &if stop { Request::Stop } else { Request::Status },
        deadline,
    )?;
    pipe.receive(deadline)
}
pub fn serve(data: &Path) -> io::Result<()> {
    let endpoint = endpoint(data)?;
    use windows::{
        core::PCWSTR,
        Win32::{
            Foundation::{WAIT_ABANDONED, WAIT_OBJECT_0},
            System::Threading::{CreateMutexW, ReleaseMutex, WaitForSingleObject},
        },
    };
    let owner_name = crate::wide(&format!(
        "Local\\ShanjieCoreOwner-{:x}",
        Sha256::digest(endpoint.as_bytes())
    ));
    let owner = unsafe { CreateMutexW(None, false, PCWSTR(owner_name.as_ptr())) }
        .map_err(|e| io::Error::other(e.to_string()))?;
    let acquired = unsafe { WaitForSingleObject(owner, 0) };
    if acquired != WAIT_OBJECT_0 && acquired != WAIT_ABANDONED {
        unsafe {
            CloseHandle(owner.0);
        }
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "Core already running",
        ));
    }
    struct Owner(windows::Win32::Foundation::HANDLE);
    impl Drop for Owner {
        fn drop(&mut self) {
            unsafe {
                let _ = ReleaseMutex(self.0);
                CloseHandle(self.0 .0);
            }
        }
    }
    let _owner = Owner(owner);
    let name = crate::wide(&endpoint);
    let descriptor = crate::wide(&format!("D:P(A;;GA;;;{})(A;;GRGW;;;AC)(A;;GRGW;;;S-1-15-2-2)S:(ML;;NW;;;LW)", user_sid()?));
    let mut sd = std::ptr::null_mut();
    unsafe {
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(
            descriptor.as_ptr(),
            1,
            &mut sd,
            std::ptr::null_mut(),
        ) == 0
        {
            return Err(io::Error::last_os_error());
        }
    }
    struct Descriptor(Raw);
    impl Drop for Descriptor {
        fn drop(&mut self) {
            unsafe {
                LocalFree(self.0);
            }
        }
    }
    let _descriptor = Descriptor(sd);
    let security = Security {
        length: std::mem::size_of::<Security>() as u32,
        descriptor: sd,
        inherit: 0,
    };
    let create = |first| unsafe {
        let raw = CreateNamedPipeW(
            name.as_ptr(),
            3 | if first { 0x80000 } else { 0 },
            8,
            64,
            65536,
            65536,
            5000,
            &security,
        );
        if raw as isize == -1 {
            Err(io::Error::last_os_error())
        } else {
            Ok(File::from_raw_handle(raw))
        }
    };
    // Own the endpoint before warming, so simultaneous first keys start one core.
    let mut pipe = create(true)?;
    let p = crate::profile::load(&crate::profile::path())
        .map_err(|e| io::Error::other(e.to_string()))?;
    let keyboard = p.number("input", "keyboard", 0);
    let layout = crate::installation::layout(keyboard, 0, crate::installation::available());
    let warm = Engine::open_local(
        data,
        layout,
        (p.get("engine", "ranking", "chat") == "formal") as u32,
        false,
        false,
        false,
        p.get("input", "dictionary", "dictionary.tsv"),
        p.flag("engine", "prediction", false),
    )
    .ok_or_else(|| io::Error::other("Unable to preload core resources"))?;
    let _warm = warm; // Keep resources resident even with no input clients.
    let sessions: session::Sessions = Default::default();
    loop {
        let connected = unsafe {
            if ConnectNamedPipe(pipe.as_raw_handle(), std::ptr::null_mut()) == 0
                && io::Error::last_os_error().raw_os_error() != Some(535)
            {
                false
            } else {
                true
            }
        };
        let data = data.to_owned();
        let clients = sessions.clone();
        // Serve the accepted client before trying to allocate another listener.
        // A full instance pool must not terminate existing sessions or the core.
        if connected {
            let _ = std::thread::Builder::new()
                .name("Shanjie core session".into())
                .spawn(move || session::run(pipe, data, clients));
        } else {
            drop(pipe);
        }
        let mut backoff = 20;
        pipe = loop {
            match create(false) {
                Ok(next) => break next,
                Err(_) => {
                    std::thread::sleep(Duration::from_millis(backoff));
                    backoff = (backoff * 2).min(500);
                }
            }
        };
    }
}
