//! Public miniature lexicon; no installed data or user settings.
use shanjie_core::{engine::{Engine,Layout,Key,KeyKind,ResetMode},Lexicon};
use std::sync::Arc;
#[test]fn incomplete_initial_digits_preserve_literals_and_valid_zhuyin(){
    let lex=Arc::new(Lexicon::parse("ㄋㄧˇ 你 -0.1\nㄐㄧˇ 己 -0.1\nㄅㄚˋ 爸 -0.1\nㄨㄛˇ 我 -0.1\nㄎㄢˋ 看 -0.1\n").unwrap());
    for english in [false,true]{
        let mut e=Engine::with_lexicon(lex.clone(),Layout::Standard);
        e.set_mixed_input(true);e.set_upstream_english(english,true).unwrap();
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
