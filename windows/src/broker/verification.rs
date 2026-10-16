//! Isolated checks invoked by the verification executable, never by input flow.
use super::*;
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
    Config {
        classes: 5,
        layout: 0,
        ranking: 0,
        right: true,
        learning,
        big5: false,
        dictionary: "dictionary.tsv".into(),
        prediction: false,
    }
}
fn key(ch: char) -> Op {
    Op::Key {
        kind: 1,
        ch: ch as u32,
        modifiers: 0,
        latin: None,
    }
}
pub fn request_fault(data: &Path) {
    let mode = std::env::var("SHANJIE_TEST_FAULT").unwrap();
    let mut client = Client::open(data, config(false)).unwrap();
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
pub fn busy_learning(data: &Path) {
    use windows::{
        core::PCWSTR,
        Win32::{Foundation::*, System::Threading::*},
    };
    let mut client = Client::open(data, config(true)).unwrap();
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
    let mut client = Client::open(data, config(true)).unwrap();
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
        clients.push(Client::open(data, config(false)).expect("slot available"));
    }
    assert!(Client::open(data, config(false)).is_none());
    assert!(clients[0].perform(Op::Mixed(true)).is_some());
    assert_eq!(clients[0].perform(key('a')).unwrap().preedit, "a");
    clients.pop();
    std::thread::sleep(Duration::from_millis(550));
    let mut replacement =
        Client::open(data, config(false)).expect("released slot accepts replacement");
    assert!(replacement.perform(Op::Mixed(true)).is_some());
    assert_eq!(replacement.perform(key('b')).unwrap().preedit, "b");
    println!("PASS 64 sessions, excess refusal, existing-session liveness and capacity recovery");
}
pub fn restart(data: &Path) {
    let mut client = Client::open(data, config(false)).unwrap();
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
