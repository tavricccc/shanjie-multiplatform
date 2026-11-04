//! Public fixtures only; no installed data or user settings.
use shanjie_core::{engine::{Engine,Layout,Key,KeyKind,ResetMode},Lexicon};
use std::sync::Arc;
#[test]fn incomplete_initial_digits_preserve_literals_and_valid_zhuyin(){
    let lex=Arc::new(Lexicon::parse("ㄋㄧˇ 你 -0.1\nㄐㄧˇ 己 -0.1\nㄅㄚˋ 爸 -0.1\nㄨㄛˇ 我 -0.1\nㄎㄢˋ 看 -0.1\n").unwrap());
    for english in [false,true]{
        let mut e=Engine::with_lexicon(lex.clone(),Layout::Standard);
        e.set_mixed_input(true);e.set_upstream_english(english,true,true,true).unwrap();
        for (keys,want) in [("r3","r3"),("r4","r4"),("r6","r6"),("r7","r7"),("r3467","r3467"),("r3a","r3a"),
            ("r3su3","r3你"),("q3","q3"),("w3","w3"),("e3","e3"),("13","13"),("14","14"),("16","16"),("17","17"),
            ("13467","13467"),("1234567890","1234567890"),("13su3","13你"),("su3","你"),("ru3","己"),("ji3","我"),("ij3","我"),
            ("d04","看"),("0d4","看"),("184","爸")]{
            for c in keys.chars(){assert!(e.key(Key::ch(c,0)).unwrap().handled);}
            assert_eq!(e.reset(ResetMode::Commit).commit,want,"mode {english}, {keys}");
            assert!(e.reset(ResetMode::Commit).commit.is_empty());
        }
        for c in "r3".chars(){e.key(Key::ch(c,0)).unwrap();}
        e.key(Key::new(KeyKind::Backspace)).unwrap();
        assert_eq!(e.key(Key::new(KeyKind::Enter)).unwrap().commit,"r");
        for c in "r4".chars(){e.key(Key::ch(c,0)).unwrap();}
        e.reset(ResetMode::Discard);
        for c in "su3".chars(){e.key(Key::ch(c,0)).unwrap();}
        assert_eq!(e.key(Key::new(KeyKind::Enter)).unwrap().commit,"你");
    }
}

#[test]
#[ignore = "Reads the built data and model in dist/data; scripts/verify.ps1 and the release job run it after the build"]
fn digit_runs_hand_back_complete_readings_to_windows_mixing() {
    let data=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../dist/data");
    let mut e=Engine::new(&data,Layout::Standard).unwrap();
    e.load_lm(&data.join("models/builtin/bigram.winmap")).unwrap();
    e.set_windows_input_options((1<<20)|(1<<21)|(1<<22));
    for (mixed,english) in [(true,true),(false,true),(true,false),(false,false)] {
        e.set_mixed_input(mixed);e.set_upstream_english(english,true,true,true).unwrap();
        if english {
            let out=e.key(Key::ch('9',0)).unwrap();
            assert!(e.digits_typed().is_some(),"the real digit table and model must be active");
            assert!(e.is_mixed_pending(),"Space must reach the open digit run");
            assert_eq!(out.preedit,"9","a buffered key is shown only once");
            e.reset(ResetMode::Discard);
        }
        for (keys,want) in [("9 u. ","哎呦"),("gk42u/4","設定"),
            ("13","13"),("14","14"),("16","16"),("17","17"),("13467","13467"),
            ("r3","r3"),("r4","r4"),("r6","r6"),("r7","r7"),("r3467","r3467")]
        {
            // Literal initial+digit handling is a Windows mixed policy.
            if (!mixed && keys.starts_with('r')) || (!mixed && !english && keys.bytes().all(|c| c.is_ascii_digit())) { continue; }
            let mut shown=String::new();
            for ch in keys.chars() {
                let key=if ch==' ' {Key::new(KeyKind::Space)} else {Key::ch(ch,0)};
                let out=e.key_with_latin(key,ch.is_ascii_alphabetic().then_some(ch)).unwrap();
                assert!(out.commit.is_empty(),"{mixed}/{english} {keys}: no early submission");
                shown=out.preedit;
            }
            assert_eq!(shown,want,"{mixed}/{english} {keys}");
            assert_eq!(e.key(Key::new(KeyKind::Enter)).unwrap().commit,want);
            assert!(e.reset(ResetMode::Commit).commit.is_empty());
        }
        for (keys,want) in [("9 u. ","哎呦"),("gk42u/4","設定")] {
            for ch in keys.chars() {
                let key=if ch==' ' {Key::new(KeyKind::Space)} else {Key::ch(ch,0)};
                e.key_with_latin(key,ch.is_ascii_alphabetic().then_some(ch.to_ascii_uppercase())).unwrap();
            }
            assert_eq!(e.key(Key::new(KeyKind::Enter)).unwrap().commit,want,"Caps display {mixed}/{english}");
        }
    }
}
