//! Isolated integration harness; never registers TSF or sends desktop input.
use shanjie_core::ffi::ShanjieKey;
use shanjie_windows::{
    broker::{self, Config, Op},
    engine::Engine,
};
use std::{path::PathBuf, time::Instant};
fn main() {
    let data = PathBuf::from(std::env::args().nth(1).expect("data directory"));
    assert!(
        std::env::var_os("SHANJIE_TEST_PROFILE").is_some(),
        "Use an isolated test profile"
    );
    let config = Config {
        layout: 0,
        ranking: 0,
        right: true,
        learning: false,
        big5: false,
        dictionary: "dictionary.tsv".into(),
        prediction: false,
    };
    let open = || Engine::open(&data, 0, 0, true, false, false, "dictionary.tsv", false).unwrap();
    let key = |e: &mut Engine, ch: char| {
        e.key(ShanjieKey {
            kind: 1,
            ch: ch as u32,
            modifiers: 0,
        })
        .unwrap()
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
    broker::control(&data, true).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(100));
    key(&mut a, '3');
    assert_eq!(a.reset(false).unwrap().commit, "你");
    let status = broker::control(&data, false).unwrap();
    assert_eq!(status.lexicon_loads, 1);
    assert_eq!(status.model_loads, 1);
    // Simultaneous first sessions after stopping the broker own one endpoint.
    drop(a);
    drop(b);
    broker::control(&data, true).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(100));
    let joins: Vec<_> = (0..4)
        .map(|_| {
            let data = data.clone();
            let config = config.clone();
            std::thread::spawn(move || {
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
