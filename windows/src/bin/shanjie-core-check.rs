//! Isolated integration harness; never registers TSF or sends desktop input.
use shanjie_core::ffi::ShanjieKey;
use shanjie_windows::{
    broker::{self, Config, Op},
    engine::Engine,
};
use std::{path::PathBuf, time::Instant};
fn start_fixture(data:&std::path::Path) {
    if std::env::var("SHANJIE_TEST_EXPLICIT_CORE").as_deref()!=Ok("1") {return;}
    broker::verification::start_fixture(data);
}
fn stop_fixture(data:&std::path::Path) {
    use windows::Win32::{Foundation::{CloseHandle,WAIT_OBJECT_0},System::Threading::{OpenProcess,PROCESS_ACCESS_RIGHTS,WaitForSingleObject}};
    let pid=broker::control(data,false).unwrap().pid;
    let handle=unsafe{OpenProcess(PROCESS_ACCESS_RIGHTS(0x00100000),false,pid)}.unwrap();
    broker::control(data,true).unwrap();
    let result=unsafe{WaitForSingleObject(handle,5000)};
    unsafe{CloseHandle(handle).unwrap();}
    assert_eq!(result,WAIT_OBJECT_0,"fixture core must exit before restart");
}
fn main() {
    let data = PathBuf::from(std::env::args().nth(1).expect("data directory"));
    assert!(
        std::env::var_os("SHANJIE_TEST_PROFILE").is_some(),
        "Use an isolated test profile"
    );
    let default_hook=std::panic::take_hook();
    std::panic::set_hook(Box::new(move|info| {
        if let Some(folder)=std::env::var_os("LOCALAPPDATA") {
            let _=std::fs::write(PathBuf::from(folder).join("core-check-panic.txt"),info.to_string());
        }
        default_hook(info);
    }));
    let config = Config {
        model: String::new(),
        classes: 5,
        layout: 0,
        ranking: 0,
        right: true,
        learning: false,
        big5: false,
        dictionary: "dictionary.tsv".into(),
        prediction: false,
        acg: true,
    };
    let open = || Engine::open(&data, 0, 0, true, false, false, "dictionary.tsv", false).unwrap();
    let key = |e: &mut Engine, ch: char| {
        let key = ShanjieKey {
            kind: 1,
            ch: ch as u32,
            modifiers: 0,
        };
        for _ in 0..4 {
            if let Some(result) = e.key(key) {
                return result;
            }
            assert_ne!(
                e.failure(),
                Some(shanjie_windows::engine::Failure::Ambiguous)
            );
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        panic!("key did not recover within bounded attempts")
    };
    if std::env::args().nth(2).as_deref() == Some("--peer") {
        let mut e = open();
        for ch in "su3".chars() {
            key(&mut e, ch);
        }
        assert_eq!(e.reset(false).unwrap().commit, "你");
        println!("peer passed");
        return;
    }
    if std::env::args().nth(2).as_deref() == Some("--user-models") {
        start_fixture(&data);
        for id in ["model-v6-classes-v5","model-v6-classes-v4","model-v5-classes-v3","model-v6-classes-v5"] {
            let mut selected=broker::Client::open(&data,Config{model:id.into(),..config.clone()}).unwrap();
            for ch in "su3".chars(){selected.perform(Op::Key{kind:1,ch:ch as u32,modifiers:0,latin:None}).unwrap();}
            assert_eq!(selected.perform(Op::Reset(false)).unwrap().commit,"你");
        }
        let status=broker::control(&data,false).unwrap();assert_eq!(status.lexicon_loads,1);assert_eq!(status.model_loads,3);
        println!("PASS built-in and two user model ids through resident IPC; {}",serde_json::to_string(&status).unwrap());return;
    }
    if std::env::args().nth(2).as_deref() == Some("--models") {
        for classes in [5,4,5] {
            let mut selected=broker::Client::open(&data,Config { classes,..config.clone() }).unwrap();
            for ch in "su3".chars() {
                selected.perform(Op::Key {kind:1,ch:ch as u32,modifiers:0,latin:None}).unwrap();
            }
            assert_eq!(selected.perform(Op::Reset(false)).unwrap().commit,"你");
        }
        let status=broker::control(&data,false).unwrap();
        assert_eq!(status.lexicon_loads,1);
        assert_eq!(status.model_loads,2);
        println!("PASS model-v6/classes-v5 to classes-v4 and back through resident IPC; {}",serde_json::to_string(&status).unwrap());
        return;
    }
    start_fixture(&data);
    eprintln!("check: first session");
    let start = Instant::now();
    let mut a = open();
    let first_ms = start.elapsed().as_millis();
    key(&mut a, 's');
    key(&mut a, 'u');
    let mut b = open();
    b.perform(Op::Mixed(true)).unwrap();
    for (ch, display) in "english".chars().zip("ENGLISH".chars()) {
        b.key_with_latin(
            ShanjieKey {
                kind: 1,
                ch: ch as u32,
                modifiers: 0,
            },
            Some(display),
        )
        .unwrap();
    }
    assert_eq!(b.reset(false).unwrap().commit, "ENGLISH");
    let exe = std::env::current_exe().unwrap();
    assert!(std::process::Command::new(exe)
        .args([data.to_str().unwrap(), "--peer"])
        .status()
        .unwrap()
        .success());
    key(&mut a, '3');
    assert_eq!(a.reset(false).unwrap().commit, "你");
    // Reconnect and replay a partially typed syllable after a server exit.
    key(&mut a, 's');
    key(&mut a, 'u');
    stop_fixture(&data);
    start_fixture(&data);
    key(&mut a, '3');
    assert_eq!(a.reset(false).unwrap().commit, "你");
    let status = broker::control(&data, false).unwrap();
    assert_eq!(status.lexicon_loads, 1);
    assert_eq!(status.model_loads, 1);
    for classes in [4,5] {
        let mut alternate=broker::Client::open(&data,Config { classes,..config.clone() }).expect("model variant opens");
        for ch in "su3".chars() {
            alternate.perform(Op::Key {kind:1,ch:ch as u32,modifiers:0,latin:None}).unwrap();
        }
        assert_eq!(alternate.perform(Op::Reset(false)).unwrap().commit,"你");
    }
    assert_eq!(broker::control(&data,false).unwrap().model_loads,2,"both selected model assets were loaded");
    println!("PASS both model-v6 class variants through resident IPC");
    // Simultaneous first sessions after stopping the broker own one endpoint.
    drop(a);
    drop(b);
    stop_fixture(&data);
    let joins: Vec<_> = (0..4)
        .map(|_| {
            let data = data.clone();
            let config = config.clone();
            std::thread::spawn(move || {
                start_fixture(&data);
                let mut c = broker::Client::open(&data, config).unwrap();
                for ch in "su3".chars() {
                    c.perform(Op::Key {
                        kind: 1,
                        ch: ch as u32,
                        modifiers: 0,
                        latin: None,
                    })
                    .unwrap();
                }
                assert_eq!(c.perform(Op::Reset(false)).unwrap().commit, "你");
            })
        })
        .collect();
    for j in joins {
        j.join().unwrap();
    }
    let status = broker::control(&data, false).unwrap();
    assert_eq!(status.lexicon_loads, 1);
    assert_eq!(status.model_loads, 1);
    println!(
        "{}",
        serde_json::json!({"first_open_ms":first_ms,"checks": ["separate_sessions","separate_processes","caps_display","restart_replay","concurrent_start"],"status":status})
    );
}
