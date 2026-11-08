//! Public fixtures only; no installed data or user settings.
use shanjie_core::{engine::{Engine,Layout,Key,KeyKind,ResetMode,MOD_SHIFT},lm::Profile,Lexicon};
use std::sync::Arc;
#[test]fn incomplete_initial_digits_preserve_literals_and_valid_zhuyin(){
    let lex=Arc::new(Lexicon::parse("ㄋㄧˇ 你 -0.1\nㄐㄧˇ 己 -0.1\nㄅㄚˋ 爸 -0.1\nㄨㄛˇ 我 -0.1\nㄎㄢˋ 看 -0.1\n").unwrap());
    {
        let mut e=Engine::with_lexicon(lex.clone(),Layout::Standard);
        e.set_mixed_input(true);
        for (keys,want) in [("r3","r3"),("r4","r4"),("r6","r6"),("r7","r7"),("r3467","r3467"),("r3a","r3a"),
            ("r3su3","r3你"),("q3","q3"),("w3","w3"),("e3","e3"),("13","13"),("14","14"),("16","16"),("17","17"),
            ("13467","13467"),("1234567890","1234567890"),("13su3","13你"),("su3","你"),("ru3","己"),("ji3","我"),("ij3","我"),
            ("d04","看"),("0d4","看"),("184","爸")]{
            for c in keys.chars(){assert!(e.key(Key::ch(c,0)).unwrap().handled);}
            assert_eq!(e.reset(ResetMode::Commit).commit,want,"{keys}");
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
fn first_tones_recover_whole_reordered_syllables_and_keep_mixed_boundaries() {
    let lex = Arc::new(
        Lexicon::parse(
            "ㄨㄛ 窩 -0.1\nㄐㄧㄥ 經 -0.1\nㄊㄚ 他 -0.1\nㄊㄞˋ 太 -0.1\nㄐㄧㄢˇ 簡 -0.1\nㄋㄧˇ 你 -0.1\nㄋㄧㄠˇ 鳥 -0.1\n",
        )
        .unwrap(),
    );
    let press = |e: &mut Engine, keys: &str| {
        let mut out = None;
        for ch in keys.chars() {
            let key = if ch == ' ' { Key::new(KeyKind::Space) } else { Key::ch(ch, 0) };
            out = Some(e.key_with_latin(key, ch.is_ascii_alphabetic().then_some(ch)).unwrap());
        }
        out.unwrap()
    };
    for symbols in [false, true] {
        let mut e = Engine::with_lexicon(lex.clone(), Layout::Standard);
        e.set_mixed_input(true);
        e.set_upstream_english(false, true, true, true).unwrap();
        e.set_windows_input_options(if symbols { 0 } else { 1 << 20 });

        for (keys, want) in [
            ("ij ", "窩"),
            ("ur/ ", "經"),
            ("8w ", "他"),
            ("9w4", "太"),
            ("u0r3", "簡"),
        ] {
            assert_eq!(press(&mut e, keys).preedit, want, "raw-key mixing symbols={symbols} {keys}");
            let down = e.key(Key::new(KeyKind::Down)).unwrap();
            let idx = down.candidates.iter().position(|c| c == keys).expect("typed literal selectable");
            assert_eq!(e.pick(idx).unwrap().unwrap().preedit, keys);
            e.reset(ResetMode::Discard);
        }

        for (keys, want) in [
            ("w4u0r3", "w4簡"),
            ("w48w ", "w4他"),
            ("r3us3", "r3你"),
            ("callsu3", "call你"),
            ("callus3", "call你"),
            ("the ", "the "),
            ("and ", "and "),
            ("not ", "not "),
            ("too ", "too "),
            ("see ", "see "),
            ("kanji ", "kanji "),
            ("input ", "input "),
            ("thorium ", "thorium "),
            ("ubuntu ", "ubuntu "),
        ] {
            assert_eq!(press(&mut e, keys).preedit, want, "raw-key mixing symbols={symbols} {keys}");
            e.reset(ResetMode::Discard);
        }
    }

}

#[test]
fn unshifted_reordered_first_tones_beat_english_words_and_keep_literal_candidates() {
    let std_lex = Arc::new(
        Lexicon::parse(
            "ㄕㄨㄛ 說 -0.1\nㄔㄨㄟ 吹 -0.1\nㄑㄧㄠ 敲 -0.1\nㄐㄧㄠ 教 -0.1\nㄐㄩㄣ 軍 -0.1\nㄨㄛ 窩 -0.1\n",
        )
        .unwrap(),
    );
    let press_latin = |e: &mut Engine, keys: &str, caps: bool, shift_first: bool, shift_all: bool| {
        let mut out = None;
        for (idx, ch) in keys.chars().enumerate() {
            if ch == ' ' {
                out = Some(e.key(Key::new(KeyKind::Space)).unwrap());
                continue;
            }
            let shifted = shift_all || (shift_first && idx == 0);
            let mods = if shifted { MOD_SHIFT } else { 0 };
            let lower = ch.to_ascii_lowercase();
            let display = if caps ^ shifted {
                lower.to_ascii_uppercase()
            } else {
                lower
            };
            out = Some(e.key_with_latin(Key::ch(lower, mods), Some(display)).unwrap());
        }
        out.unwrap()
    };

    for right_anchor in [false, true] {
        let mut e = Engine::with_lexicon(std_lex.clone(), Layout::Standard);
        e.set_mixed_input(true);
        e.set_mixed_tolerance(true);
        e.set_upstream_english(false, true, true, true).unwrap();
        e.set_candidate_anchor(right_anchor);

        for perm in ["gji ", "gij ", "jgi ", "jig ", "igj ", "ijg "] {
            let out = press_latin(&mut e, perm, false, false, false);
            assert_eq!(out.preedit, "說", "raw-key mixing right={right_anchor} {perm}");
            let down = e.key(Key::new(KeyKind::Down)).unwrap();
            let idx = down
                .candidates
                .iter()
                .position(|c| c == perm)
                .unwrap_or_else(|| panic!("missing literal {perm:?} in {:?}", down.candidates));
            assert_eq!(e.pick(idx).unwrap().unwrap().preedit, perm);
            e.reset(ResetMode::Discard);
        }

        for (keys, want) in [
            ("jot ", "吹"),
            ("flu ", "敲"),
            ("lur ", "教"),
            ("rpm ", "軍"),
        ] {
            let out = press_latin(&mut e, keys, false, false, false);
            assert_eq!(out.preedit, want, "raw-key mixing right={right_anchor} {keys}");
            let down = e.key(Key::new(KeyKind::Down)).unwrap();
            let idx = down.candidates.iter().position(|c| c == keys).expect("literal candidate");
            assert_eq!(e.pick(idx).unwrap().unwrap().preedit, keys);
            e.reset(ResetMode::Discard);
        }

        // Caps Lock without Shift still converts to Chinese while preserving uppercase literal candidate.
        let caps_out = press_latin(&mut e, "jig ", true, false, false);
        assert_eq!(caps_out.preedit, "說", "Caps Lock unshifted raw-key mixing right={right_anchor}");
        let caps_down = e.key(Key::new(KeyKind::Down)).unwrap();
        let caps_idx = caps_down
            .candidates
            .iter()
            .position(|c| c == "JIG ")
            .unwrap_or_else(|| panic!("missing uppercase literal in {:?}", caps_down.candidates));
        assert_eq!(e.pick(caps_idx).unwrap().unwrap().preedit, "JIG ");
        e.reset(ResetMode::Discard);

        // Explicit Shift keeps English (both normal Shift and Shift-lowercase under Caps Lock).
        let expected_space = " ";
        press_latin(&mut e, "jig ", false, true, false);
        assert_eq!(
            e.reset(ResetMode::Commit).commit,
            format!("Jig{expected_space}"),
            "Shift-first raw-key mixing"
        );
        press_latin(&mut e, "jig ", false, false, true);
        assert_eq!(
            e.reset(ResetMode::Commit).commit,
            format!("JIG{expected_space}"),
            "Shift-all raw-key mixing"
        );
        press_latin(&mut e, "jig ", true, false, true);
        assert_eq!(
            e.reset(ResetMode::Commit).commit,
            format!("jig{expected_space}"),
            "Caps+Shift lowercase raw-key mixing"
        );

        // Repeated-letter and suffix-trap English words stay English even with tolerance on.
        for word in ["too ", "see ", "kanji "] {
            let out = press_latin(&mut e, word, false, false, false);
            assert_eq!(out.preedit, word, "raw-key mixing right={right_anchor} {word}");
            e.reset(ResetMode::Discard);
        }
    }


    // Eten layout reordered first-tone words also convert and keep English literal candidates.
    let eten_lex = Arc::new(Lexicon::parse("ㄐㄧㄚ 家 -0.1\nㄊㄨㄛ 脫 -0.1\n").unwrap());
    for right_anchor in [false, true] {
        let mut e = Engine::with_lexicon(eten_lex.clone(), Layout::Eten);
        e.set_mixed_input(true);
        e.set_candidate_anchor(right_anchor);
        for (keys, want) in [("age ", "家"), ("tox ", "脫")] {
            let out = press_latin(&mut e, keys, false, false, false);
            assert_eq!(out.preedit, want, "eten right={right_anchor} {keys}");
            let down = e.key(Key::new(KeyKind::Down)).unwrap();
            let idx = down.candidates.iter().position(|c| c == keys).expect("eten literal candidate");
            assert_eq!(e.pick(idx).unwrap().unwrap().preedit, keys);
            e.reset(ResetMode::Discard);
        }
    }
}

#[test]
fn shifted_english_stays_literal_at_space_and_does_not_leak_to_next_reading() {
    let lex = Arc::new(
        Lexicon::parse("ㄨㄛ 窩 -0.1\nㄙㄠ 騷 -0.1\nㄇㄒ ㄇㄒ -0.9\n").unwrap(),
    );
    let mut e = Engine::with_lexicon(lex.clone(), Layout::Standard);
    e.set_mixed_input(true);
    e.set_upstream_english(false, true, true, true).unwrap();

    e.key_with_latin(Key::ch('l', MOD_SHIFT), Some('L')).unwrap();
    e.key_with_latin(Key::ch('s', 0), Some('s')).unwrap();
    e.key(Key::new(KeyKind::Space)).unwrap();
    for ch in "ij ".chars() {
        let key = if ch == ' ' { Key::new(KeyKind::Space) } else { Key::ch(ch, 0) };
        e.key_with_latin(key, ch.is_ascii_alphabetic().then_some(ch)).unwrap();
    }
    let expected = "Ls 窩";
    assert_eq!(e.reset(ResetMode::Commit).commit, expected, "raw-key mixing");

}

#[test]
#[ignore = "Reads the built data and model in dist/data; scripts/verify.ps1 and the release job run it after the build"]
fn digit_runs_hand_back_complete_readings_to_windows_mixing() {
    let data=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../dist/data");
    let mut e=Engine::new(&data,Layout::Standard).unwrap();
    e.load_lm(&data.join("models/builtin/bigram.winmap")).unwrap();
    e.set_windows_input_options((1<<20)|(1<<21)|(1<<22)|(1<<23));
    // The mixing mode is one choice: upstream English, raw-key mixing or off.
    for (mixed,english) in [(false,true),(true,false),(false,false)] {
        e.set_mixed_input(mixed);e.set_upstream_english(english,true,true,true).unwrap();
        if english {
            let out=e.key(Key::ch('9',0)).unwrap();
            assert!(e.digits_typed().is_some(),"the real digit table and model must be active");
            assert!(e.is_mixed_pending(),"Space must reach the open digit run");
            assert_eq!(out.preedit,"9","a buffered key is shown only once");
            e.reset(ResetMode::Discard);
            let press=|e:&mut Engine,keys:&str|{
                let mut out=None;
                for ch in keys.chars() {
                    let key=if ch==' ' {Key::new(KeyKind::Space)} else {Key::ch(ch,0)};
                    out=Some(e.key_with_latin(key,ch.is_ascii_alphabetic().then_some(ch)).unwrap());
                }
                out.unwrap()
            };
            // A lone digit shows as the digit, uncommitted, and Enter sends it; Space keeps composing.
            for digit in ["1","2","5","8","0"] {
                let shown=press(&mut e,digit);
                assert!(shown.commit.is_empty(),"{mixed} {digit}: not committed early");
                assert_eq!(shown.preedit,digit,"{mixed} {digit}");
                assert_eq!(e.key(Key::new(KeyKind::Enter)).unwrap().commit,digit,"{mixed} {digit} Enter");
                assert!(e.reset(ResetMode::Commit).commit.is_empty());
                let spaced=press(&mut e,&format!("{digit} "));
                assert!(spaced.commit.is_empty());
                // 5, 8 and 0 alone are whole syllables (ㄓ, ㄚ, ㄢ): Space converts them, as 9 Space does for 哎呦.
                if ["1","2"].contains(&digit) {assert_eq!(spaced.preedit,format!("{digit} "),"{mixed} {digit} Space");}
                e.reset(ResetMode::Discard);
                press(&mut e,digit);
                assert!(e.key(Key::new(KeyKind::Esc)).unwrap().commit.is_empty(),"{mixed} {digit} cancel");
                e.reset(ResetMode::Discard);
            }
            // The keys stay available: later keys still form readings and numbers, also behind a composition.
            for (keys,want) in [("184","爸"),("3.14","3.14"),("2026","2026"),("su31","你1")] {
                assert_eq!(press(&mut e,keys).preedit,want,"{mixed} {keys}");
                assert_eq!(e.key(Key::new(KeyKind::Enter)).unwrap().commit,want,"{mixed} {keys} Enter");
                assert!(e.reset(ResetMode::Commit).commit.is_empty());
            }
            press(&mut e,"1");
            assert!(!e.key(Key::new(KeyKind::Down)).unwrap().candidates.is_empty(),"Down still opens the list of the reading");
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

#[test]
#[ignore = "Reads the built data and model in dist/data; scripts/verify.ps1 and the release job run it after the build"]
fn sessions_on_one_model_share_the_digit_tables() {
    let data=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../dist/data");
    let model=data.join("models/builtin/bigram.winmap");
    let open=|packs:u32|{
        let mut e=Engine::new_with_personal(&data,Layout::Standard,None,packs).unwrap();
        e.load_lm(&model).unwrap();
        e
    };
    let (mut first,mut second)=(open(0),open(0));
    assert!(first.windows_shares_digit_cache(&second),"a session on the same lexicon and model reuses the tables");
    assert!(!first.windows_shares_digit_cache(&open(shanjie_core::engine::PACK_IT)),"another lexicon builds its own");
    for e in [&mut first,&mut second] {e.set_mixed_input(false);e.set_upstream_english(true,true,true,true).unwrap();}
    for ch in "2026su3".chars() {
        let (a,b)=(first.key(Key::ch(ch,0)).unwrap(),second.key(Key::ch(ch,0)).unwrap());
        assert_eq!((a.preedit,a.candidates),(b.preedit,b.candidates),"after {ch}");
    }
}

#[test]
#[ignore = "Reads the built data and model in dist/data; scripts/verify.ps1 and the release job run it after the build"]
fn shifted_caps_tail_refreshes_predictions_in_upstream_english_mode() {
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../dist/data");
    let mut e = Engine::new(&data, Layout::Standard).unwrap();
    e.load_lm(&data.join("models/builtin/bigram.winmap")).unwrap();
    e.set_windows_input_options(0);
    e.set_upstream_english(true, true, true, true).unwrap();
    e.set_prediction(true).unwrap();
    for ch in "gemini".chars() {
        e.key_with_latin(Key::ch(ch, shanjie_core::engine::MOD_SHIFT), Some(ch)).unwrap();
    }
    let mut out = None;
    for ch in "CL3".chars() {
        out = Some(e.key_with_latin(Key::ch(ch.to_ascii_lowercase(), 0), ch.is_ascii_alphabetic().then_some(ch)).unwrap());
    }
    let out = out.unwrap();
    assert_eq!(out.preedit, "gemini好");
    assert!(!out.candidates.is_empty(), "the restored Chinese syllable must refresh its prediction row");
}

#[test]
#[ignore = "Reads the built data and model in dist/data; scripts/verify.ps1 and the release job run it after the build"]
fn mixed_sentences_recover_reordered_syllables() {
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../dist/data");
    let mut e = Engine::new(&data, Layout::Standard).unwrap();
    e.load_lm(&data.join("models/builtin/bigram.winmap")).unwrap();
    let colloquial = data.join("models/builtin/colloquial.sjlm");
    if colloquial.exists() {
        e.load_colloquial(&colloquial).unwrap();
    }
    e.set_mixed_input(true);

    let press = |e: &mut Engine, keys: &str| {
        let mut out = None;
        for ch in keys.chars() {
            let key = if ch == ' ' { Key::new(KeyKind::Space) } else { Key::ch(ch, 0) };
            out = Some(e.key_with_latin(key, ch.is_ascii_alphabetic().then_some(ch)).unwrap());
        }
        out.unwrap()
    };

    e.set_upstream_english(false, true, true, true).unwrap();
    for profile in [Profile::Chat, Profile::Formal] {
        e.set_profile(profile).unwrap();
        for space_symbols in [false, true] {
            let base_opts = (1 << 21) | (1 << 22) | (1 << 23) | if space_symbols { 0 } else { 1 << 20 };
            e.set_windows_input_options(base_opts);

            let want_think_de = if matches!(profile, Profile::Chat) {
                "把他想得太簡單"
            } else {
                "把它想得太簡單"
            };
            let want_think_di = if matches!(profile, Profile::Chat) {
                "把他想的太簡單"
            } else {
                "把它想得太簡單"
            };

            for (keys, want) in [
                ("1838w vu;32k69w4u0r320 ", want_think_de),
                ("8138w ;uv3k269w4u0r320 ", want_think_de),
                ("1838w vu;32k79w4u0r320 ", want_think_di),
                ("1838w ;uv3k279w4u0r320 ", want_think_di),
                ("xu06vm4823n0 h4", "連續打三次"),
                ("w4u0r3", "w4簡"),
                ("ur/ ", "經"),
                ("jig ", "說"),
                ("su3jig kg6ka7", "你說什麼"),
                ("ji3urp wu0 jig 8jc4", "我今天說話"),
                ("flu ap6", "敲門"),
                ("jot z/ ", "吹風"),
                ("ul41j4ul4jt fm4y.3y.3u8 ", "要不要出去走走呀"),
                ("ul41j4ul4jt fm4.y3.y3u8 ", "要不要出去走走呀"),
                (
                    "1j4cl3u4n ji3urp wu0 ao6u.3g6ur0 tj3xu35k4ru04g4fu/6",
                    "不好意思我今天沒有時間處理這件事情",
                ),
                ("ji3urp wu0 1u3rul4a;61l4fu04", "我今天比較忙抱歉"),
                (
                    "xu3194u eo3ji35k4zp4jp6ru041j4ul4jwi u06g6u0r ",
                    "禮拜一給我這份文件不要拖延時間",
                ),
                (
                    "xu3914u eo3ji35k4zp4jp6ru041j4ul4jwi u06g6u0r ",
                    "禮拜一給我這份文件不要拖延時間",
                ),
            ] {
                let out = press(&mut e, keys);
                assert_eq!(
                    out.preedit, want,
                    "profile={profile:?} raw-key mixing space_symbols={space_symbols} keys={keys}"
                );
                assert_eq!(e.key(Key::new(KeyKind::Enter)).unwrap().commit, want);
            }

            for tail in ["CL3M/4A87", "LC3M/4A87"] {
                for ch in "gemini".chars() {
                    e.key_with_latin(Key::ch(ch, MOD_SHIFT), Some(ch)).unwrap();
                }
                for ch in tail.chars() {
                    let lower = ch.to_ascii_lowercase();
                    let display = ch.is_ascii_alphabetic().then_some(ch);
                    e.key_with_latin(Key::ch(lower, 0), display).unwrap();
                }
                let out = e.key(Key::new(KeyKind::Enter)).unwrap();
                assert_eq!(
                    out.commit, "gemini好用嗎",
                    "profile={profile:?} raw-key mixing space_symbols={space_symbols} tail={tail}"
                );
            }


        }
    }

}
