//! Per-user, per-session resident core. The pipe transports composition state;
//! it never writes keystrokes to disk. Immutable data is shared inside the core.
use crate::engine::{Engine, Snapshot};
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
fn user_sid() -> io::Result<String> {
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
#[derive(Clone, Serialize, Deserialize)]
pub struct Config {
    pub layout: u32,
    pub ranking: u32,
    pub right: bool,
    pub learning: bool,
    pub big5: bool,
    pub dictionary: String,
    pub prediction: bool,
}
#[derive(Clone, Serialize, Deserialize)]
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
}
#[derive(Serialize, Deserialize)]
enum Request {
    Open(Config, Vec<Op>),
    Apply(Op),
    Status,
    Stop,
}
#[derive(Serialize, Deserialize)]
pub struct Reply {
    pub snapshot: Option<Snapshot>,
    pub pending: bool,
    pub ok: bool,
    pub pid: u32,
    pub lexicon_loads: u64,
    pub model_loads: u64,
}
fn reply(engine: Option<&Engine>, snapshot: Option<Snapshot>, ok: bool) -> Reply {
    Reply {
        snapshot,
        pending: engine.is_some_and(Engine::mixed_pending),
        ok,
        pid: std::process::id(),
        lexicon_loads: shanjie_core::ffi::shanjie_windows_resource_loads(0),
        model_loads: shanjie_core::ffi::shanjie_windows_resource_loads(1),
    }
}
fn connect(data: &Path, start: bool) -> io::Result<File> {
    let name = endpoint(data)?;
    let expected = executable(data).canonicalize()?;
    let deadline = Instant::now() + Duration::from_secs(if start { 30 } else { 1 });
    let mut launched = false;
    loop {
        match std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&name)
        {
            Ok(file) => unsafe {
                let mut pid = 0;
                if GetNamedPipeServerProcessId(file.as_raw_handle(), &mut pid) == 0 {
                    return Err(io::Error::last_os_error());
                }
                let process = OpenProcess(0x1000, 0, pid);
                if process.is_null() {
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
                return Ok(file);
            },
            Err(error) => {
                if ![Some(2), Some(231)].contains(&error.raw_os_error())
                    || Instant::now() >= deadline
                {
                    return Err(error);
                }
                if start && !launched {
                    start_core(&expected)?;
                    launched = true;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
}
fn start_core(executable: &Path) -> io::Result<()> {
    use windows::{
        core::{PCWSTR, PWSTR},
        Win32::System::Threading::*,
    };
    let application = crate::wide(&executable.to_string_lossy());
    let mut command = crate::wide(&format!("\"{}\" --serve", executable.display()));
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
            PCWSTR::null(),
            &startup,
            &mut process,
        )
        .map_err(|e| io::Error::other(e.to_string()))?;
        CloseHandle(process.hThread.0);
        CloseHandle(process.hProcess.0);
    }
    Ok(())
}
pub struct Client {
    pipe: File,
    data: PathBuf,
    config: Config,
    journal: Vec<Op>,
    settings: Vec<Op>,
    pub pending: bool,
}
impl Client {
    pub fn open(data: &Path, config: Config) -> Option<Self> {
        let mut pipe = connect(data, true).ok()?;
        write(&mut pipe, &Request::Open(config.clone(), Vec::new())).ok()?;
        if !read::<Reply>(&mut pipe).ok()?.ok {
            return None;
        }
        Some(Self {
            pipe,
            data: data.into(),
            config,
            journal: Vec::new(),
            settings: Vec::new(),
            pending: false,
        })
    }
    fn request(&mut self, op: Op) -> io::Result<Reply> {
        write(&mut self.pipe, &Request::Apply(op))?;
        read(&mut self.pipe)
    }
    pub fn perform(&mut self, op: Op) -> Option<Snapshot> {
        let result = self
            .request(op.clone())
            .or_else(|_| {
                self.pipe = connect(&self.data, true)?;
                let mut replay = self.settings.clone();
                replay.extend(self.journal.clone());
                write(&mut self.pipe, &Request::Open(self.config.clone(), replay))?;
                if !read::<Reply>(&mut self.pipe)?.ok {
                    return Err(io::Error::other("Core recovery failed"));
                }
                self.request(op.clone())
            })
            .ok()?;
        if !result.ok {
            return None;
        }
        self.pending = result.pending;
        match &op {
            Op::Options(_) | Op::Mixed(_) | Op::Prediction(_) | Op::Demote(_) => {
                self.settings
                    .retain(|old| std::mem::discriminant(old) != std::mem::discriminant(&op));
                self.settings.push(op);
            }
            Op::Context(_) => {
                self.journal.retain(|old| !matches!(old, Op::Context(_)));
                self.journal.push(op);
            }
            _ => {
                if result
                    .snapshot
                    .as_ref()
                    .is_some_and(|s| s.preedit.is_empty())
                {
                    self.journal.clear();
                } else {
                    self.journal.push(op);
                }
            }
        }
        result.snapshot
    }
}
pub fn control(data: &Path, stop: bool) -> io::Result<Reply> {
    let mut pipe = connect(data, false)?;
    write(
        &mut pipe,
        &if stop { Request::Stop } else { Request::Status },
    )?;
    read(&mut pipe)
}
fn session(mut pipe: File, data: PathBuf) {
    let mut engine: Option<Engine> = None;
    while let Ok(request) = read::<Request>(&mut pipe) {
        let response = match request {
            Request::Open(config, replay) => {
                if Path::new(&config.dictionary).components().count() != 1
                    || !config.dictionary.ends_with(".tsv")
                {
                    reply(None, None, false)
                } else {
                    engine = Engine::open_local(
                        &data,
                        config.layout,
                        config.ranking,
                        config.right,
                        config.learning,
                        config.big5,
                        &config.dictionary,
                        config.prediction,
                    );
                    let ok = engine.as_mut().is_some_and(|e| e.replay(&replay));
                    reply(engine.as_ref(), None, ok)
                }
            }
            Request::Apply(op) => {
                let snapshot = engine.as_mut().and_then(|e| e.perform(op));
                let ok = snapshot.is_some();
                reply(engine.as_ref(), snapshot, ok)
            }
            Request::Status => reply(None, None, true),
            Request::Stop => {
                let _ = write(&mut pipe, &reply(None, None, true));
                std::process::exit(0);
            }
        };
        if write(&mut pipe, &response).is_err() {
            break;
        }
    }
}
pub fn serve(data: &Path) -> io::Result<()> {
    let name = crate::wide(&endpoint(data)?);
    let descriptor = crate::wide(&format!("D:P(A;;GA;;;{})S:(ML;;NW;;;LW)", user_sid()?));
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
    let layout = if keyboard == 2 { 1 } else { 0 };
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
    loop {
        unsafe {
            if ConnectNamedPipe(pipe.as_raw_handle(), std::ptr::null_mut()) == 0
                && io::Error::last_os_error().raw_os_error() != Some(535)
            {
                return Err(io::Error::last_os_error());
            }
        }
        let next = create(false)?;
        let data = data.to_owned();
        std::thread::spawn(move || session(pipe, data));
        pipe = next;
    }
}
