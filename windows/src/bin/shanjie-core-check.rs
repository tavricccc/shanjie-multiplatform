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
/// One key on the resident core and on a local engine that reranks before returning: the remote one waits until the core
/// says its rerank is done, then asks for it. Returns whether a rerank was applied.
fn rerank_step(remote: &mut broker::Client, local: &mut Engine, ch: char) -> bool {
    let op = Op::Key { kind: 1, ch: ch as u32, modifiers: 0, latin: None };
    let want = local.perform(op.clone()).unwrap();
    // The first request after a core restart finds the old pipe gone; the next one reconnects and replays.
    let mut got = (0..4)
        .find_map(|_| {
            let got = remote.perform(op.clone());
            assert_ne!(remote.failure, Some(shanjie_windows::engine::Failure::Ambiguous));
            if got.is_none() {
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            got
        })
        .expect("key did not recover within bounded attempts");
    assert_eq!(got.commit, want.commit);
    let reranked = got.rerank;
    if reranked {
        let until = Instant::now() + std::time::Duration::from_secs(10);
        while !remote.rerank_ready() {
            assert!(Instant::now() < until, "rerank did not finish");
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let ranked = remote.perform(Op::Rerank).unwrap();
        assert!(ranked.commit.is_empty() && !ranked.rerank);
        got = ranked;
    }
    assert_eq!((got.preedit, got.candidates, got.selected), (want.preedit, want.candidates, want.selected), "background rerank");
    reranked
}
/// Opening a core in this process replaces the panic hook with a silent one, so this one is set again after that.
fn report_panics() {
    std::panic::set_hook(Box::new(|info| {
        if let Some(folder)=std::env::var_os("LOCALAPPDATA") {
            let _=std::fs::write(PathBuf::from(folder).join("core-check-panic.txt"),info.to_string());
        }
        eprintln!("{info}");
    }));
}
fn main() {
    let data = PathBuf::from(std::env::args().nth(1).expect("data directory"));
    assert!(
        std::env::var_os("SHANJIE_TEST_PROFILE").is_some(),
        "Use an isolated test profile"
    );
    report_panics();
    let config = Config { prediction: false, ..Config::default() };
    let open = || Engine::open(&data, &config).unwrap();
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
    // The core reranks after replying, as upstream runs the job off the main thread; a restart replays the reranks.
    let mut local = Engine::open_local(&data, &config).expect("local engine");
    report_panics();
    let mut remote = broker::Client::open(&data, config.clone()).unwrap();
    let mut reranked = 0;
    for ch in "su3cl".chars() {
        reranked += rerank_step(&mut remote, &mut local, ch) as usize;
    }
    stop_fixture(&data);
    start_fixture(&data);
    reranked += rerank_step(&mut remote, &mut local, '3') as usize;
    assert!(reranked > 0, "the built-in model must rerank");
    assert_eq!(remote.perform(Op::Reset(false)).unwrap().commit, local.reset(false).unwrap().commit);
    drop((local, remote));
    let status = broker::control(&data, false).unwrap();
    assert_eq!(status.lexicon_loads, 1);
    assert_eq!(status.model_loads, 1);
    // verify-update.ps1 installs the public model-v6-classes-v4 bundle as a user model.
    for model in ["model-v6-classes-v4", shanjie_shell::profile::BUILTIN] {
        let mut alternate=broker::Client::open(&data,Config { model:model.into(),..config.clone() }).expect("selected model opens");
        for ch in "su3".chars() {
            alternate.perform(Op::Key {kind:1,ch:ch as u32,modifiers:0,latin:None}).unwrap();
        }
        assert_eq!(alternate.perform(Op::Reset(false)).unwrap().commit,"你");
    }
    assert_eq!(broker::control(&data,false).unwrap().model_loads,2,"the user model was loaded beside the built-in one");
    println!("PASS built-in and user model through resident IPC");
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
        serde_json::json!({"first_open_ms":first_ms,"checks": ["separate_sessions","separate_processes","caps_display","restart_replay","background_rerank","concurrent_start"],"status":status})
    );
}
