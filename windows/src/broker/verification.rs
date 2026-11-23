//! Isolated checks invoked by the verification executable, never by input flow.
use super::*;
pub fn profile_root() {
    let expected=crate::profile::user_folder();
    let fixture=std::env::var_os("LOCALAPPDATA").unwrap();
    std::env::remove_var("SHANJIE_TEST_PROFILE");
    let actual=crate::profile::folder();
    assert_eq!(actual,expected,"inherited LOCALAPPDATA and USERPROFILE cannot redirect real data");
    assert_ne!(actual,PathBuf::from(fixture).join("Shanjie"));
    println!("PASS trusted user folder ignores inherited packaged/systemprofile environment (no user-data access)");
}
pub fn log_failure(data:&Path) {
    for _ in 0..20 {assert!(Client::open(data, config(false), None).is_none());}
    println!("PASS failed opens retain failure stage; throttled metadata log written");
}
/// Read-only probe of the exact identity code with an existing Store token.
/// No injection, ACL changes, target messages or user-data reads/writes.
pub fn restricted_identity(data:&Path,pid:u32) {unsafe{
    use windows::Win32::{Foundation::*,Security::*,System::Threading::*};
    let process=windows::Win32::System::Threading::OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION,false,pid).unwrap();
    let mut image=[0u16;32768];let mut length=image.len() as u32;
    windows::Win32::System::Threading::QueryFullProcessImageNameW(process,PROCESS_NAME_WIN32,windows::core::PWSTR(image.as_mut_ptr()),&mut length).unwrap();
    let image=PathBuf::from(String::from_utf16_lossy(&image[..length as usize]));
    assert!(["WinStore.App.exe","ShellExperienceHost.exe"].iter().any(|name|image.file_name().unwrap().eq_ignore_ascii_case(name)));
    let mut token=HANDLE::default();windows::Win32::System::Threading::OpenProcessToken(process,TOKEN_QUERY|TOKEN_DUPLICATE|TOKEN_IMPERSONATE,&mut token).unwrap();
    let mut ac=0u32;let mut needed=0;windows::Win32::Security::GetTokenInformation(token,TokenIsAppContainer,Some((&mut ac as *mut u32).cast()),4,&mut needed).unwrap();assert_eq!(ac,1);
    struct Handles(HANDLE,HANDLE);impl Drop for Handles{fn drop(&mut self){unsafe{let _=windows::Win32::Foundation::CloseHandle(self.0);let _=windows::Win32::Foundation::CloseHandle(self.1);}}}
    let _handles=Handles(process,token);
    let expected_identity=identity::path(&executable(data)).unwrap();
    let expected_contents=Sha256::digest(std::fs::read(executable(data)).unwrap());
    let expected_folder=crate::profile::user_folder();
    ImpersonateLoggedOnUser(token).unwrap();
    struct Revert;impl Drop for Revert{fn drop(&mut self){unsafe{RevertToSelf().unwrap();}}}
    let _revert=Revert;
    let old=executable(data).canonicalize();
    // Impersonation cannot query this desktop probe's process token as though
    // it belonged to the package. Probe the exact file-identity/read operations
    // separately; full IPC/TSF acceptance still requires the installed host.
    let mut file=File::open(executable(data)).expect("restricted file read");
    let actual=identity::file_path(&file).expect("restricted NT identity");
    let mut contents=Vec::new();file.read_to_end(&mut contents).unwrap();
    use windows::Win32::{UI::Shell::*,System::Com::CoTaskMemFree};
    let folder=SHGetKnownFolderPath(&FOLDERID_LocalAppData,KF_FLAG_NO_PACKAGE_REDIRECTION|KF_FLAG_DONT_VERIFY,Some(token)).unwrap();
    let actual_folder=PathBuf::from(folder.to_string().unwrap()).join("Shanjie");CoTaskMemFree(Some(folder.0.cast()));
    assert_eq!(actual_folder,expected_folder,"known folder is stable under AppContainer token");
    assert_eq!(actual,expected_identity,"restricted and normal tokens resolve identical file identity");
    assert_eq!(Sha256::digest(contents),expected_contents);
    assert_eq!(old.unwrap_err().raw_os_error(),Some(5),"control still reproduces DOS failure");
    println!("PASS AppContainer token: DOS canonicalize denied; NT identity identical; content hash identical; explicit-token known folder identical");
}}
use crate::pipe_io::Pipe;
use std::os::windows::fs::OpenOptionsExt;
pub fn start_fixture(data:&Path) {
    assert_eq!(std::env::var("SHANJIE_TEST_PROFILE").as_deref(),Ok("1"));
    if control(data,false).is_ok() {return;}
    start_core(&executable(data)).expect("start isolated fixture core");
    let deadline=Instant::now()+Duration::from_secs(10);
    while control(data,false).is_err() {
        assert!(Instant::now()<deadline,"fixture core did not become ready");
        std::thread::sleep(Duration::from_millis(20));
    }
}
pub fn pipe_deadlines() {
    for writing in [false, true] {
        let name = crate::wide(&format!(
            "\\\\.\\pipe\\ShanjieDeadlineTest-{}-{}",
            std::process::id(),
            writing
        ));
        let text = crate::wide(&format!(
            "D:P(A;;GA;;;{})S:(ML;;NW;;;LW)",
            user_sid().unwrap()
        ));
        let mut descriptor = std::ptr::null_mut();
        assert_ne!(
            unsafe {
                ConvertStringSecurityDescriptorToSecurityDescriptorW(
                    text.as_ptr(),
                    1,
                    &mut descriptor,
                    std::ptr::null_mut(),
                )
            },
            0
        );
        let security = Security {
            length: std::mem::size_of::<Security>() as u32,
            descriptor,
            inherit: 0,
        };
        let raw = unsafe { CreateNamedPipeW(name.as_ptr(), 3, 0, 1, 64, 64, 100, &security) };
        unsafe {
            LocalFree(descriptor);
        }
        assert_ne!(raw as isize, -1);
        let server = unsafe { File::from_raw_handle(raw) };
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(0x40000000)
            .open(String::from_utf16_lossy(&name[..name.len() - 1]))
            .unwrap();
        unsafe {
            ConnectNamedPipe(raw, std::ptr::null_mut());
        }
        let pipe = Pipe(file);
        let start = Instant::now();
        let deadline = start + Duration::from_millis(50);
        let result = if writing {
            pipe.send(&vec![1u8; 65536], deadline)
        } else {
            pipe.read(&mut [0; 4], deadline)
        };
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::TimedOut);
        assert!(start.elapsed() < Duration::from_secs(1));
        drop(pipe);
        drop(server);
    }
    println!("PASS connected read and blocked write cancellation");
}
fn config(learning: bool) -> Config {
    Config { learning, prediction: false, ..Config::default() }
}
fn key(ch: char) -> Op {
    Op::Key {
        kind: 1,
        ch: ch as u32,
        modifiers: 0,
        latin: None,
    }
}

pub fn learning_storage_status(data: &Path, route: &str) {
    use std::os::windows::fs::OpenOptionsExt;
    let cfg = config(true);
    let mut e = if route == "local" { Engine::open_local(data, &cfg) } else { Engine::open(data, &cfg) }.unwrap();
    e.perform(Op::Options(crate::profile::load(&crate::profile::path()).unwrap().input_options())).unwrap();
    let select = |e: &mut Engine| {
        e.reset(true).unwrap();
        e.perform(Op::Context("他們".into())).unwrap();
        let mut shown = Snapshot::default();
        for ch in "su3".chars() { shown = e.perform(key(ch)).unwrap(); }
        let list = e.perform(Op::Key { kind: 10, ch: 0, modifiers: 0, latin: None }).unwrap();
        let i = list.candidates.iter().position(|w| w != &shown.preedit && w.chars().all(|c| ('\u{3400}'..='\u{9fff}').contains(&c))).unwrap();
        e.perform(Op::Pick(i as u32)).unwrap();
        e.reset(false).unwrap()
    };
    assert!(!select(&mut e).learning_unavailable);
    assert!(!select(&mut e).learning_unavailable); // first rewrite, then append
    let file = crate::profile::folder().join("learning.tsv");
    let bytes = std::fs::read(&file).unwrap();
    // Sharing permits reads, but blocks both append and replacement of the file.
    let lock = std::fs::OpenOptions::new().read(true).share_mode(1).open(&file).unwrap();
    let failed = select(&mut e);
    assert!(!failed.commit.is_empty() && failed.preedit.is_empty(), "saving cannot swallow text");
    assert!(failed.learning_unavailable, "append and fallback rewrite both failed");
    assert_eq!(std::fs::read(&file).unwrap(), bytes);
    let transmitted: Snapshot = serde_json::from_str(&serde_json::to_string(&failed).unwrap()).unwrap();
    assert!(transmitted.learning_unavailable);
    if route == "pipe" { assert!(control(data, false).unwrap().learning_unavailable); }
    drop(lock);
    assert!(!select(&mut e).learning_unavailable, "successful retry clears core status");
    if route == "pipe" { assert!(!control(data, false).unwrap().learning_unavailable); }
    crate::profile::clear_learning().unwrap();
    assert!(!select(&mut e).learning_unavailable, "peer epoch remains usable after clear");
    let mut off = Engine::open_local(data, &config(false)).unwrap();
    assert!(!select(&mut off).learning_unavailable);
    println!("PASS {route} learning failure, visible snapshot, successful retry, off and clear epoch");
}

pub fn privacy_gate(data: &Path, route: &str) {
    let cfg = config(true);
    let mut e = if route == "local" { Engine::open_local(data, &cfg) } else { Engine::open(data, &cfg) }.unwrap();
    e.perform(Op::Options(crate::profile::load(&crate::profile::path()).unwrap().input_options())).unwrap();
    e.perform(Op::Context("隔離測試前文".into())).unwrap();
    for ch in "su3".chars() { e.perform(key(ch)).unwrap(); }
    let c = e.perform(Op::Key { kind: 10, ch: 0, modifiers: 0, latin: None }).unwrap();
    assert!(!c.candidates.is_empty());
    e.perform(Op::Pick(1)).unwrap();
    let file = crate::profile::folder().join("learning.tsv");
    let before = std::fs::read(&file).ok();
    e.set_privacy_allowed(false).unwrap();
    assert!(!e.privacy_allowed());
    e.perform(Op::Context("此內容不能送入解碼前文".into())).unwrap();
    let committed = e.reset(false).unwrap();
    assert!(!committed.commit.is_empty() && !committed.learning_unavailable);
    assert_eq!(std::fs::read(&file).ok(), before, "pending selection was not learned");
    if route == "local" { assert!(e.replay(&[Op::Privacy(false), key('s'), key('u'), key('3')])); }
    e.reset(false).unwrap();
    assert_eq!(std::fs::read(&file).ok(), before);
    e.set_privacy_allowed(true).unwrap();
    e.perform(Op::Context("普通前文".into())).unwrap();
    for ch in "su3".chars() { e.perform(key(ch)).unwrap(); }
    e.perform(Op::Key { kind: 10, ch: 0, modifiers: 0, latin: None }).unwrap();
    e.perform(Op::Pick(1)).unwrap();
    e.reset(false).unwrap();
    assert_ne!(std::fs::read(&file).ok(), before, "learning resumes after a normal document starts");
    let stable = std::fs::read(&file).unwrap();
    let mut off = Engine::open_local(data, &config(false)).unwrap();
    off.set_privacy_allowed(false).unwrap(); off.set_privacy_allowed(true).unwrap();
    for ch in "su3".chars() { off.perform(key(ch)).unwrap(); }
    off.perform(Op::Key { kind: 10, ch: 0, modifiers: 0, latin: None }).unwrap(); off.perform(Op::Pick(1)).unwrap(); off.reset(false).unwrap();
    assert_eq!(std::fs::read(file).unwrap(), stable, "privacy cannot enable a disabled user preference");
    println!("PASS {route} privacy gate drops pending learning and context; resume and explicit off preserved");
}
pub fn request_fault(data: &Path) {
    let mode = std::env::var("SHANJIE_TEST_FAULT").unwrap();
    let mut client = Client::open(data, config(false), None).unwrap();
    assert!(client.perform(Op::Mixed(true)).is_some());
    for ch in "adb".chars() {
        assert!(client.perform(key(ch)).is_some());
    }
    let enter = Op::Key {
        kind: 3,
        ch: 0,
        modifiers: 0,
        latin: None,
    };
    assert!(client.perform(enter.clone()).is_none());
    let result = client
        .perform(enter)
        .expect("same request resumes its cached or pending result");
    assert_eq!(result.commit, "adb");
    assert!(result.preedit.is_empty());
    assert_eq!(client.perform(Op::Reset(false)).unwrap().commit, "");
    println!("PASS {mode}: sequence result acknowledged once");
}
/// A rerank whose reply was lost or late must not stop the next key: it is completed first, under its own sequence.
pub fn rerank_lost(data: &Path) {
    let mode = std::env::var("SHANJIE_TEST_FAULT").unwrap();
    let typed = |client: &mut Client, keys: &str| {
        for ch in keys.chars() {
            assert!(client.perform(key(ch)).is_some());
        }
    };
    let mut client = Client::open(data, config(false), None).unwrap();
    typed(&mut client, "su3");
    assert!(client.perform(Op::Rerank).is_none(), "the fault must lose this reply");
    assert_ne!(client.failure, Some(Failure::Ambiguous));
    typed(&mut client, "cl3");
    let commit = client.perform(Op::Reset(false)).unwrap().commit;
    assert!(client.failure.is_none());
    assert_eq!(client.perform(Op::Reset(false)).unwrap().commit, "", "committed once");
    let mut control = Client::open(data, config(false), None).unwrap();
    typed(&mut control, "su3");
    control.perform(Op::Rerank).unwrap();
    typed(&mut control, "cl3");
    assert_eq!(commit, control.perform(Op::Reset(false)).unwrap().commit);
    println!("PASS {mode}: unanswered rerank completes before the next key, commit once");
}
/// When the core restarted, nobody can say whether the rerank ran: keep the guard, and a new session types normally.
pub fn rerank_unknown(data: &Path) {
    let mut client = Client::open(data, config(false), None).unwrap();
    for ch in "su3".chars() {
        assert!(client.perform(key(ch)).is_some());
    }
    assert!(client.perform(Op::Rerank).is_none(), "the fault must lose this reply");
    control(data, true).unwrap();
    std::thread::sleep(Duration::from_millis(100));
    for _ in 0..5 {
        assert!(client.perform(key('c')).is_none());
        if client.failure == Some(Failure::Ambiguous) {
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    assert_eq!(client.failure, Some(Failure::Ambiguous));
    let mut fresh = Client::open(data, config(false), None).unwrap();
    for ch in "su3".chars() {
        assert!(fresh.perform(key(ch)).is_some());
    }
    assert_eq!(fresh.perform(Op::Reset(false)).unwrap().commit, "你");
    println!("PASS rerank with an unknown result stays ambiguous; a new session recovers input");
}
/// Only a rerank is completed for the next operation; any other unanswered operation still blocks a different one.
pub fn outstanding_other(data: &Path) {
    let mut client = Client::open(data, config(false), None).unwrap();
    assert!(client.perform(Op::Mixed(true)).is_some());
    for ch in "adb".chars() {
        assert!(client.perform(key(ch)).is_some());
    }
    let enter = Op::Key { kind: 3, ch: 0, modifiers: 0, latin: None };
    assert!(client.perform(enter).is_none(), "the fault must lose this reply");
    assert!(client.perform(key('x')).is_none());
    assert_eq!(client.failure, Some(Failure::Ambiguous));
    println!("PASS a different unanswered operation stays guarded");
}
pub fn busy_learning(data: &Path) {
    use windows::{
        core::PCWSTR,
        Win32::{Foundation::*, System::Threading::*},
    };
    let mut client = Client::open(data, config(true), None).unwrap();
    client.perform(Op::Mixed(true)).unwrap();
    let folder = crate::profile::folder();
    let name = crate::wide(&format!(
        "Local\\ShanjieLearning-{}",
        folder.to_str().unwrap().to_uppercase().replace('\\', "_")
    ));
    let mutex = unsafe { CreateMutexW(None, false, PCWSTR(name.as_ptr())).unwrap() };
    assert_eq!(unsafe { WaitForSingleObject(mutex, 1000) }, WAIT_OBJECT_0);
    assert!(client.perform(key('a')).is_none());
    assert_eq!(client.failure, Some(Failure::Busy));
    unsafe {
        ReleaseMutex(mutex).unwrap();
        CloseHandle(mutex).unwrap();
    }
    assert_eq!(client.perform(key('a')).unwrap().preedit, "a");
    assert_eq!(client.perform(Op::Reset(false)).unwrap().commit, "a");
    println!("PASS learning contention preserves request order");
}
pub fn ambiguous(data: &Path) {
    let mut client = Client::open(data, config(true), None).unwrap();
    client.perform(Op::Context("他們".into())).unwrap();
    client.perform(Op::Mixed(true)).unwrap();
    let mut original=Snapshot::default();
    for ch in "su3".chars() {original=client.perform(key(ch)).unwrap();}
    let candidates = client
        .perform(Op::Key {
            kind: 10,
            ch: 0,
            modifiers: 0,
            latin: None,
        })
        .unwrap();
    let index = candidates
        .candidates
        .iter()
        .position(|word| word != &original.preedit && word.chars().all(|ch|('\u{3400}'..='\u{9fff}').contains(&ch)))
        .unwrap();
    client.perform(Op::Pick(index as u32)).unwrap();
    let enter = Op::Key {
        kind: 3,
        ch: 0,
        modifiers: 0,
        latin: None,
    };
    assert!(client.perform(enter.clone()).is_none());
    let file = crate::profile::folder().join("learning.tsv");
    let before = std::fs::read(&file).unwrap();
    control(data, true).unwrap();
    std::thread::sleep(Duration::from_millis(100));
    for _ in 0..5 {
        assert!(client.perform(enter.clone()).is_none());
        if client.failure == Some(Failure::Ambiguous) {
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    assert_eq!(client.failure, Some(Failure::Ambiguous));
    assert_eq!(std::fs::read(file).unwrap(), before);
    println!(
        "PASS lost result followed by restart is retained, never blindly replayed or learned twice"
    );
}
pub fn capacity(data: &Path) {
    let mut clients = Vec::new();
    for _ in 0..64 {
        clients.push(Client::open(data, config(false), None).expect("slot available"));
    }
    assert!(Client::open(data, config(false), None).is_none());
    assert!(clients[0].perform(Op::Mixed(true)).is_some());
    assert_eq!(clients[0].perform(key('a')).unwrap().preedit, "a");
    clients.pop();
    std::thread::sleep(Duration::from_millis(550));
    let mut replacement =
        Client::open(data, config(false), None).expect("released slot accepts replacement");
    assert!(replacement.perform(Op::Mixed(true)).is_some());
    assert_eq!(replacement.perform(key('b')).unwrap().preedit, "b");
    println!("PASS 64 sessions, excess refusal, existing-session liveness and capacity recovery");
}

/// Run each strategy in a fresh process/endpoint so the first pack change never reuses a lexicon
/// cached by the other strategy. Later turns also exercise eviction and reopening cached masks.
pub fn pack_switch(data: &Path, strategy: &str) {
    use crate::engine::{BuildSource, Engine};
    use windows::Win32::{Foundation::CloseHandle, System::{ProcessStatus::*, Threading::*}};

    let mut input = config(false);
    input.acg = false;
    let started = Instant::now();
    let mut engine = Engine::open(data, &input).expect("initial pack");
    let ms = |elapsed: Duration| elapsed.as_secs_f64() * 1000.0;
    let open_ms = ms(started.elapsed());
    let local = std::env::var_os("SHANJIE_CORE_LOCAL").is_some();
    let process = unsafe {
        if local { GetCurrentProcess() } else {
            OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, control(data, false).unwrap().pid).unwrap()
        }
    };
    let memory = || unsafe {
        let mut counters = PROCESS_MEMORY_COUNTERS_EX::default();
        let size = std::mem::size_of_val(&counters) as u32;
        counters.cb = size;
        GetProcessMemoryInfo(process, (&mut counters as *mut PROCESS_MEMORY_COUNTERS_EX).cast(), size).unwrap();
        counters
    };
    let opened = memory();
    let loads = || if local { shanjie_core::ffi::shanjie_windows_resource_loads(1) } else { control(data, false).unwrap().model_loads };
    let initial_loads = loads();
    let keys: Vec<_> = "su3cl3".chars().map(key).chain([Op::Key { kind: 3, ch: 0, modifiers: 0, latin: None }]).collect();
    let type_key = |engine: &mut Engine, i: usize, samples: &mut Vec<f64>| {
        let op = keys[i].clone();
        let start = Instant::now();
        let out = engine.perform(op).expect("old engine accepts a key");
        samples.push(ms(start.elapsed()));
        if i + 1 == keys.len() {
            assert_eq!(out.commit, "你好");
        } else {
            assert!(out.commit.is_empty());
        }
    };
    let mut idle = Vec::new();
    for _ in 0..30 { for i in 0..keys.len() { type_key(&mut engine, i, &mut idle); } }
    let mut busy = Vec::new();
    let mut builds = Vec::new();
    let mut private_mib = Vec::new();
    let mut closed = 0;
    let mut peak_dual_private = 0;
    for mask in [1, 2, 3, 4, 5, 6, 7, 0, 1, 2, 3, 4] {
        input.acg = mask & 1 != 0;
        input.quotes = mask & 2 != 0;
        input.it = mask & 4 == 0;
        let start = Instant::now();
        let replacement = if strategy == "background" {
            let source = engine.build_source().expect("working engine can share");
            let (data, input) = (data.to_owned(), input.clone());
            let worker = std::thread::spawn(move || {
                let result = Engine::open_reusing(&data, &input, Some(source)).expect("shared pack build");
                (result, start.elapsed())
            });
            let mut i = 0;
            while !worker.is_finished() {
                type_key(&mut engine, i, &mut busy);
                i = (i + 1) % keys.len();
                std::thread::sleep(Duration::from_millis(2));
            }
            if i != 0 { engine.perform(Op::Reset(true)).unwrap(); }
            let (new, elapsed) = worker.join().unwrap();
            builds.push(ms(elapsed));
            new
        } else {
            assert_eq!(strategy, "sync");
            let replacement = Engine::open(data, &input).expect("synchronous pack build");
            builds.push(ms(start.elapsed()));
            replacement
        };
        peak_dual_private = peak_dual_private.max(memory().PrivateUsage);
        let retired = engine.build_source();
        engine = replacement;
        if let Some(BuildSource::Remote(id)) = retired {
            let until = Instant::now() + Duration::from_secs(1);
            loop {
                let pipe = connect(data, false, until).unwrap();
                pipe.send(&Request::Resume(id.clone()), until).unwrap();
                if !pipe.receive::<Reply>(until).unwrap().found { break; }
                assert!(Instant::now() < until, "retired session was not released");
                std::thread::sleep(Duration::from_millis(5));
            }
            closed += 1;
        }
        private_mib.push(memory().PrivateUsage as f64 / 1048576.0);
    }
    let model_loads = loads() - initial_loads;
    if strategy == "background" {
        assert!(!busy.is_empty() && model_loads == 0, "same-model switches should reuse the map");
    }
    let quantiles = |samples: &mut [f64]| {
        samples.sort_by(|a, b| a.total_cmp(b));
        (!samples.is_empty()).then(|| [samples[(samples.len() - 1) / 2], samples[(samples.len() - 1) * 95 / 100]])
    };
    println!("PASS {}", serde_json::json!({
        "strategy": strategy, "transport": if local { "local" } else { "pipe" },
        "open_ms": open_ms, "build_ms": builds, "model_loads": model_loads, "retired_pipe_sessions": closed,
        "idle_samples": idle.len(), "idle_p50_p95_ms": quantiles(&mut idle),
        "building_samples": busy.len(), "building_p50_p95_ms": quantiles(&mut busy),
        "open_private_mib": opened.PrivateUsage as f64 / 1048576.0,
        "peak_dual_private_mib": peak_dual_private as f64 / 1048576.0,
        "peak_working_set_mib": memory().PeakWorkingSetSize as f64 / 1048576.0,
        "private_mib_after_each_swap": private_mib
    }));
    if !local { unsafe { CloseHandle(process).unwrap(); } }
}
pub fn restart(data: &Path) {
    let mut client = Client::open(data, config(false), None).unwrap();
    client.perform(Op::Mixed(true)).unwrap();
    for ch in "ad".chars() {
        client.perform(key(ch)).unwrap();
    }
    control(data, true).unwrap();
    std::thread::sleep(Duration::from_millis(100));
    assert!(client.perform(key('b')).is_none());
    let mut result = None;
    for _ in 0..4 {
        result = client.perform(key('b'));
        if result.is_some() {
            break;
        }
        assert_ne!(client.failure, Some(Failure::Ambiguous));
        std::thread::sleep(Duration::from_millis(200));
    }
    assert_eq!(result.unwrap().preedit, "adb");
    assert_eq!(client.perform(Op::Reset(false)).unwrap().commit, "adb");
    println!("PASS restart reconstructs only acknowledged uncommitted input");
}
