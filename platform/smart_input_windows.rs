// Windows composition policy. Chinese candidates/scoring remain upstream.
fn english_word(word: &str) -> bool {
    static WORDS: std::sync::OnceLock<Vec<&'static str>> = std::sync::OnceLock::new();
    WORDS
        .get_or_init(|| include_str!("english-words.txt").lines().collect())
        .binary_search(&word)
        .is_ok()
}

#[cfg(test)]
mod windows_smart_tests {
    use super::*;
    #[test]
    fn upstream_english_preserves_windows_caps_and_shift_display() {
        let mut e=engine();e.set_mixed_input(false);
        let folder=std::env::temp_dir().join(format!("shanjie-caps-{}",std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();let path=folder.join("english.txt");
        std::fs::write(&path,"hello\nrust\n").unwrap();e.load_english(&path).unwrap();e.set_upstream_english(true,true).unwrap();
        for ch in "rust".chars(){e.key_with_latin(Key::ch(ch,0),Some(ch.to_ascii_uppercase())).unwrap();}
        assert_eq!(e.key(Key::new(KeyKind::Enter)).unwrap().commit,"RUST");
        let out=e.key_with_latin(Key::ch('r',MOD_SHIFT),Some('r')).unwrap();
        assert_eq!(out.preedit,"r");assert_eq!(out.english_run_utf16_len,1);
        e.key_with_latin(Key::ch('u',0),Some('U')).unwrap();
        assert_eq!(e.key(Key::new(KeyKind::Enter)).unwrap().commit,"rU");
        for ch in "su".chars(){e.key_with_latin(Key::ch(ch,0),Some(ch.to_ascii_uppercase())).unwrap();}
        e.key(Key::ch('3',0)).unwrap();assert_eq!(e.key(Key::new(KeyKind::Enter)).unwrap().commit,"你");
        e.layout=Layout::Eten;e.set_mixed_input(true);e.set_upstream_english(true,true).unwrap();
        assert!(!e.english&&e.mixed_input,"unsupported upstream English never disables Eten Windows mixed input");
    }
    #[test]
    fn symbols_kept_behind_a_reading_stay_before_an_english_run() {
        let mut e=engine();e.set_mixed_input(false);
        let folder=std::env::temp_dir().join(format!("shanjie-run-order-{}",std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();let path=folder.join("english.txt");
        std::fs::write(&path,"hello\n").unwrap();e.load_english(&path).unwrap();e.set_upstream_english(true,true).unwrap();
        e.set_windows_input_options(1 << 22);
        e.key(Key::ch('s',0)).unwrap();
        assert_eq!(e.key(Key::ch('\\',MOD_CONTROL)).unwrap().preedit,"ㄋ、","the kept unfinished reading");
        assert_eq!(e.key(Key::ch('a',MOD_SHIFT)).unwrap().preedit,"、A");
        assert_eq!(e.key(Key::new(KeyKind::Enter)).unwrap().commit,"、A");
        std::fs::remove_dir_all(&folder).ok();
    }
    #[test]
    fn incomplete_reading_keeps_unconvertible_tone_digits() {
        let lex=Arc::new(Lexicon::parse("ㄋㄧˇ 你 -0.1\nㄐㄧˇ 己 -0.1\nㄅㄚˋ 爸 -0.1\nㄓˊ 直 -0.1\n").unwrap());
        for english in [false,true] {
            let mut e=Engine::with_lexicon(lex.clone(),Layout::Standard);
            e.set_mixed_input(true);e.set_upstream_english(english,true).unwrap();
            for digits in ["13","14","16","17","13467","1234567890",
                "r3","r4","r6","r7","r3467","q3","w3","e3","s3","d3","f3","a3","z3","x3","c3","v3","r3a"] {
                assert_eq!(type_keys(&mut e,digits).preedit,digits,"english={english}");
                assert_eq!(e.reset(ResetMode::Commit).commit,digits);
            }
            assert_eq!(type_keys(&mut e,"13su3").preedit,"13你");e.reset(ResetMode::Discard);
            assert_eq!(type_keys(&mut e,"r3su3").preedit,"r3你");e.reset(ResetMode::Discard);
            type_keys(&mut e,"r3");e.key(Key::new(KeyKind::Backspace)).unwrap();
            assert_eq!(e.key(Key::new(KeyKind::Enter)).unwrap().commit,"r");
            type_keys(&mut e,"r4");e.reset(ResetMode::Discard);
            assert_eq!(type_keys(&mut e,"ru3").preedit,"己");e.reset(ResetMode::Discard);
            assert_eq!(type_keys(&mut e,"184").preedit,"爸","a complete valid syllable still converts");e.reset(ResetMode::Discard);
            assert_eq!(type_keys(&mut e,"su3").preedit,"你");
        }
    }
    fn engine() -> Engine {
        let lex =
            Lexicon::parse("ㄋㄧˇ 你 -0.1\nㄏㄠˇ 好 -0.1\nㄧˇ 以 -0.2\nㄋㄧˇ-ㄏㄠˇ 你好 -0.1\n")
                .unwrap();
        let mut e = Engine::with_lexicon(Arc::new(lex), Layout::Standard);
        e.set_mixed_input(true);
        e
    }
    fn type_keys(e: &mut Engine, keys: &str) -> Output {
        let mut out = e.view(true, String::new());
        for ch in keys.chars() {
            out = e
                .key(if ch == ' ' {
                    Key::new(KeyKind::Space)
                } else {
                    Key::ch(ch, 0)
                })
                .unwrap();
        }
        out
    }
    #[test]
    fn optional_duplicate_key_tolerance_keeps_original_selectable() {
        let lex=Arc::new(Lexicon::parse("ㄏㄨㄟˊ 回 -0.1\nㄐㄧㄚ 家 -0.1\nㄏㄨㄟˊ-ㄐㄧㄚ 回家 -0.01\n").unwrap());
        for layout in [Layout::Standard,Layout::Eten] {
            let word_keys:String="ㄏㄨㄟ".chars().map(|s|layout.key_of_symbol(s).unwrap()).collect();
            let tone=layout.key_of_tone('ˊ').unwrap();
            let home_keys:String="ㄐㄧㄚ".chars().map(|s|layout.key_of_symbol(s).unwrap()).collect();
            let correct=format!("{word_keys}{tone}{home_keys} ");
            let extra=format!("{}{word_keys}{tone}{home_keys} ",word_keys.chars().next().unwrap());
            for enabled in [false,true] {
                let mut e=Engine::with_lexicon(lex.clone(),layout); e.set_mixed_input(true); e.set_mixed_tolerance(enabled);
                assert_eq!(type_keys(&mut e,&correct).preedit,"回家"); e.reset(ResetMode::Discard);
                let out=type_keys(&mut e,&extra);
                assert_eq!(out.preedit,if enabled {"回家".into()} else {format!("{}回家",word_keys.chars().next().unwrap())});
                if enabled {
                    let shown=e.key(Key::new(KeyKind::Down)).unwrap();
                    let index=shown.candidates.iter().position(|s|s==&extra).unwrap();
                    assert_eq!(e.pick(index).unwrap().unwrap().preedit,extra);
                    assert_eq!(e.key(Key::new(KeyKind::Enter)).unwrap().commit,extra);
                }
            }
            let mut e=Engine::with_lexicon(lex.clone(),layout); e.set_mixed_input(true); e.set_mixed_tolerance(true);
            for english in ["call", "hello", "ABC", "SSD123"] {
                for ch in english.chars() { e.key_with_latin(Key::ch(ch.to_ascii_lowercase(),0),Some(ch)).unwrap(); }
                assert_eq!(e.view(true,String::new()).preedit,english); e.reset(ResetMode::Discard);
            }
        }
    }
    #[test]
    fn mixed_boundaries_and_lifecycle() {
        let mut e = engine();
        assert_eq!(type_keys(&mut e, "gj").preedit, "gj");
        assert_eq!(e.key(Key::new(KeyKind::Backspace)).unwrap().preedit, "g");
        assert_eq!(e.key(Key::ch('j', MOD_SHIFT)).unwrap().preedit, "gJ");
        assert_eq!(e.reset(ResetMode::Commit).commit, "gJ");
        assert_eq!(type_keys(&mut e, "su3hello").preedit, "你hello");
        assert_eq!(type_keys(&mut e, "su3").preedit, "你hello你");
        assert_eq!(e.key(Key::new(KeyKind::Enter)).unwrap().commit, "你hello你");
        assert_eq!(type_keys(&mut e, "callsu3").preedit, "call你");
        assert_eq!(e.reset(ResetMode::Commit).commit, "call你");
        type_keys(&mut e, "hello");
        assert_eq!(e.key(Key::new(KeyKind::Backspace)).unwrap().preedit, "hell");
        assert_eq!(e.reset(ResetMode::Commit).commit, "hell");
        type_keys(&mut e, "hello");
        assert!(e.key(Key::new(KeyKind::Esc)).unwrap().preedit.is_empty());
        assert_eq!(type_keys(&mut e, "hello world").preedit, "hello world");
        assert_eq!(
            e.key(Key::new(KeyKind::Enter)).unwrap().commit,
            "hello world"
        );
        type_keys(&mut e, "hello ");
        e.key(Key::new(KeyKind::Backspace)).unwrap();
        assert_eq!(e.key(Key::new(KeyKind::Backspace)).unwrap().preedit, "hell");
        e.reset(ResetMode::Discard);
        e.set_mixed_input(false);
        assert!(!type_keys(&mut e, "hello").preedit.contains("hello"));
    }

    #[test]
    fn upstream_english_runs_beside_the_windows_policy() {
        let mut e = engine();
        let dir = std::env::temp_dir().join("shanjie-upstream-english-test");
        std::fs::create_dir_all(&dir).unwrap();
        let list = dir.join("english.txt");
        std::fs::write(&list, "rust
hello
").unwrap();
        e.load_english(&list).unwrap();
        e.set_upstream_english(true, true).unwrap();
        assert!(e.mixed_input && e.english, "both stay on");
        // Shift+letter starts upstream's English run; the Windows raw state stays out of it.
        let out = e.key(Key::ch('r', MOD_SHIFT)).unwrap();
        assert_eq!(out.english_run_utf16_len, 1);
        assert!(e.mixed_raw.is_empty());
        // While the run is open upstream owns punctuation: it ends the run into a token.
        let out = e.key(Key::ch(',', 0)).unwrap();
        assert_eq!(out.english_run_utf16_len, 0);
        e.reset(ResetMode::Discard);
        // Idle again, the Windows policy maps the symbol as before.
        assert_eq!(e.key(Key::ch('[', 0)).unwrap().preedit, "「");
        e.reset(ResetMode::Discard);
        // The Windows order rule still turns non-Zhuyin keys into English.
        assert_eq!(type_keys(&mut e, "gj").preedit, "gj");
        e.reset(ResetMode::Discard);
        // Switching upstream off leaves only the Windows policy.
        e.set_upstream_english(false, true).unwrap();
        assert!(!e.english && e.mixed_input);
        assert_eq!(type_keys(&mut e, "gj").preedit, "gj");
    }

    #[test]
    fn zhuyin_order_does_not_matter_unless_it_looks_english() {
        let lex = Arc::new(Lexicon::parse("ㄎㄢˋ 看 -0.1\nㄓㄨㄥ 中 -0.1\n").unwrap());
        for mixed in [false, true] {
            let mut e = Engine::with_lexicon(lex.clone(), Layout::Standard);
            e.set_mixed_input(mixed);
            for keys in ["d04", "0d4"] {
                assert_eq!(type_keys(&mut e, keys).preedit, "看", "mixed={mixed} {keys}");
                e.reset(ResetMode::Discard);
            }
            // Final, medial, initial (/j5) still reaches 中 when the run starts with a symbol.
            assert_eq!(type_keys(&mut e, "/j5 ").preedit, "中", "mixed={mixed}");
            e.reset(ResetMode::Discard);
        }
    }

    #[test]
    fn english_before_chinese_keeps_the_word_and_spaces() {
        let lex = Lexicon::parse("ㄋㄧˇ 你 -0.1\nㄏㄠˇ 好 -0.1\nㄓㄜˋ 這 -0.1\nㄕˋ 是 -0.1\nㄓㄨㄥ 中 -0.1\nㄨㄣˊ 文 -0.1\n").unwrap();
        let mut e = Engine::with_lexicon(Arc::new(lex), Layout::Standard);
        e.set_mixed_input(true);
        for spaces in ["", " "] {
            assert_eq!(type_keys(&mut e, "su3cl35k4g4english").preedit, "你好這是english");
            type_keys(&mut e, spaces);
            let out = type_keys(&mut e, "5j/ jp6");
            assert_eq!(out.preedit, format!("你好這是english{spaces}中文"));
            assert_eq!(e.reset(ResetMode::Commit).commit, out.preedit);
        }
        type_keys(&mut e, "su3english");
        for _ in 0..2 { assert!(e.insert_literal_space().unwrap().commit.is_empty()); }
        assert_eq!(type_keys(&mut e, "5j/ jp6").preedit, "你english  中文");
        assert_eq!(e.reset(ResetMode::Commit).commit, "你english  中文");
    }
}

impl Engine {
    pub fn insert_literal_space(&mut self) -> Result<Output, EngineError> {
        let raw = std::mem::take(&mut self.mixed_raw);
        self.mixed_latin = false;
        let prefix = self.mixed_text(&raw)?.commit;
        let mut out = self.mixed_text(" ")?;
        out.commit = prefix + &out.commit;
        Ok(out)
    }
    pub fn is_mixed_pending(&self) -> bool {
        self.mixed_latin || !self.mixed_raw.is_empty()
    }
    pub fn set_mixed_input(&mut self, mixed: bool) {
        self.mixed_input = mixed;
    }
    pub fn set_mixed_tolerance(&mut self, enabled: bool) { self.mixed_tolerance = enabled; }

    // Conservative first stage: one adjacent repeated key, a complete syllable,
    // an explicit tone, and one unambiguous dictionary reading. Never touch host text.
    fn corrected_mixed_keys(&self, raw: &str, tone: usize) -> Option<String> {
        if !self.mixed_tolerance || !(2..=4).contains(&raw.len())
            || !raw.is_ascii() || raw.chars().any(|c| c.is_ascii_uppercase())
            || english_word(raw) { return None; }
        let mut choices = std::collections::BTreeSet::new();
        for index in 1..raw.len() {
            if raw.as_bytes()[index] != raw.as_bytes()[index-1] { continue; }
            let keys = format!("{}{}", &raw[..index], &raw[index+1..]);
            if let Some(mut reading) = self.mixed_reading_order(&keys, false) {
                reading.push_str(TONE_MARKS[tone]);
                if !self.lex.entries(&[reading]).is_empty() { choices.insert(keys); }
            }
        }
        if choices.len() == 1 { choices.into_iter().next() } else { None }
    }
    /// Upstream English (zhen-mixed/zhen-inline) runs beside the Windows mixed policy. `on` needs
    /// `load_english` for the word list; `inline` shows the typed letters instead of Zhuyin.
    pub fn set_upstream_english(&mut self, on: bool, inline: bool) -> Result<(), EngineError> {
        // Upstream's English composer supports Standard; Eten keeps Windows mixed input.
        let on=on && self.layout==Layout::Standard;
        self.set_english(on)?;
        self.set_english_inline(inline)?;
        Ok(())
    }
    fn upstream_english_busy(&self) -> bool {
        !self.run.is_empty()
            || self.cands.as_ref().is_some_and(|c| c.english)
            || self.english_inline_shown()
    }
    pub fn is_mixed_hint(&self) -> bool {
        self.mixed_input && !self.mixed_latin && !self.mixed_raw.is_empty() && !self.pending().is_empty()
    }

    // Each column (initial, medial, final) appears at most once. A run starting with an
    // English letter must also follow initial -> medial -> final, so an English word does
    // not win over typed text; one starting with a digit or symbol (e.g. 0d4 = ㄎㄢˋ) is not
    // English and may be typed in any order. An explicit tone also confirms
    // full reordered alphabetic syllables in finish_latin.
    fn mixed_reading(&self, raw: &str) -> Option<String> {
        let ordered = raw.chars().next().is_some_and(|c| c.is_ascii_alphabetic());
        self.mixed_reading_order(raw, ordered)
    }

    fn mixed_reading_order(&self, raw: &str, ordered: bool) -> Option<String> {
        let mut last = None;
        let mut columns: [Option<char>; 3] = [None; 3];
        for ch in raw.chars() {
            let (column, symbol) = self.layout.symbol_of(ch.to_ascii_lowercase())?;
            if columns[column].is_some() || ordered && last.is_some_and(|previous| previous >= column) {
                return None;
            }
            last = Some(column);
            columns[column] = Some(symbol);
        }
        let reading: String = columns.iter().flatten().collect();
        (!reading.is_empty()).then_some(reading)
    }

    fn mixed_text(&mut self, text: &str) -> Result<Output, EngineError> {
        self.clear_pend();
        self.clear_pred();
        self.cands = None;
        let mut out = self.handled()?;
        let mut committed = String::new();
        for ch in text.chars() {
            out = self.insert_token(format!("{PUNCT_PREFIX}{ch}"), Some(ch.to_string()), None)?;
            committed.push_str(&out.commit);
        }
        out.commit = committed;
        Ok(out)
    }

    fn finish_latin(&mut self, key: Key) -> Result<Output, EngineError> {
        let raw = std::mem::take(&mut self.mixed_raw);
        self.mixed_latin = false;
        // Once a run is literal digits, later tone-number keys remain digits.
        if !raw.is_empty() && raw.bytes().all(|c| c.is_ascii_digit())
            && key.kind==KeyKind::Char && key.modifiers==0 && key.ch.is_ascii_digit() {
            self.mixed_raw=raw;self.mixed_raw.push(key.ch);self.mixed_latin=true;
            return self.handled();
        }
        // Keep every literal prefix, including names absent from the English
        // list. Only the valid trailing syllable is allowed to become Chinese.
        let tone = if key.kind == KeyKind::Space {
            Some(0)
        } else if key.kind == KeyKind::Char && key.modifiers == 0 {
            self.layout.tone_of(key.ch)
        } else {
            None
        };
        if let Some(tone) =
            tone.filter(|_| key.kind != KeyKind::Space || !english_word(&raw.to_ascii_lowercase()))
        {
            if let Some(corrected) = self.corrected_mixed_keys(&raw, tone) {
                self.clear_pend(); self.clear_pred();
                for ch in corrected.chars() { self.key_original(Key::ch(ch, 0))?; }
                let out = self.key_original(key)?;
                let ending = if key.kind == KeyKind::Space { ' ' } else { key.ch };
                if let Some(index) = self.cursor.checked_sub(1).filter(|&i| i < self.syls.len()) {
                    self.typed_literals.insert(index, format!("{raw}{ending}"));
                }
                return Ok(out);
            }
            // A tone explicitly confirms a complete syllable before the suffix heuristic.
            if key.kind != KeyKind::Space {
                if let Some(mut reading) = self.mixed_reading_order(&raw, false) {
                    reading.push_str(TONE_MARKS[tone]);
                    if !self.lex.entries(&[reading]).is_empty() {
                        self.clear_pend();
                        for ch in raw.chars() { self.key_original(Key::ch(ch.to_ascii_lowercase(), 0))?; }
                        let out = self.key_original(key)?;
                        if let Some(index) = self.cursor.checked_sub(1).filter(|&i| i < self.syls.len()) {
                            self.typed_literals.insert(index, format!("{raw}{}", key.ch));
                        }
                        return Ok(out);
                    }
                }
            }
            for split in raw.len().saturating_sub(3).max(1)..raw.len() {
                let (word, tail) = raw.split_at(split);
                if key.kind == KeyKind::Space
                    && !english_word(&word.to_ascii_lowercase())
                    && tail.chars().all(|ch| ch.is_ascii_alphabetic())
                {
                    continue;
                }
                if let Some(mut reading) = self.mixed_reading(tail) {
                    reading.push_str(TONE_MARKS[tone]);
                    if self.lex.entries(&[reading]).is_empty() {
                        continue;
                    }
                    let prefix = self.mixed_text(word)?.commit;
                    for ch in tail.chars() {
                        self.key_original(Key::ch(ch.to_ascii_lowercase(), 0))?;
                    }
                    let mut out = self.key_original(key)?;
                    let ending = if key.kind == KeyKind::Space { ' ' } else { key.ch };
                    if let Some(index) = self.cursor.checked_sub(1).filter(|&i| i < self.syls.len()) {
                        self.typed_literals.insert(index, format!("{tail}{ending}"));
                    }
                    out.commit = prefix + &out.commit;
                    return Ok(out);
                }
            }
        }
        if key.kind == KeyKind::Char && key.modifiers == 0 && key.ch.is_ascii_digit() {
            self.mixed_raw = raw;
            self.mixed_raw.push(key.ch);
            self.mixed_latin = true;
            return self.handled();
        }
        let mut out = self.mixed_text(&raw)?;
        if key.kind == KeyKind::Space
            || (key.kind == KeyKind::Char && key.modifiers == 0 && key.ch.is_ascii_punctuation())
        {
            let prefix = out.commit;
            out = self.mixed_text(&if key.kind == KeyKind::Space {
                " ".into()
            } else {
                key.ch.to_string()
            })?;
            out.commit = prefix + &out.commit;
        } else {
            let prefix = out.commit;
            out = self.key_original(key)?;
            out.commit = prefix + &out.commit;
        }
        Ok(out)
    }

    pub fn key(&mut self, key: Key) -> Result<Output, EngineError> {
        // An open digit run (upstream digits.md A.3.4) sees every key first, as in upstream's dispatch.
        // The Windows punctuation, literal and mixed policies would otherwise keep its tail as an
        // unfinished reading (1 then Ctrl+\ showing ㄅ、 instead of 1、).
        if self.drun.is_some() {
            return self.key_original(key);
        }
        if self.shadow_on() && self.run.is_empty() && key.kind==KeyKind::Char && key.modifiers==MOD_SHIFT && key.ch.is_ascii_alphabetic() {
            // Symbols the Windows policy kept behind an unfinished reading go in front of the run: upstream's
            // run replaces that reading, and the symbols were typed before the letter.
            let mut commit=String::new();
            if !self.pending_literal.is_empty() {
                let text=std::mem::take(&mut self.pending_literal);
                self.clear_pend();
                commit=self.insert_windows_literal(&text)?.commit;
            }
            let mut out=self.start_run(self.latin_character(key))?;
            out.commit=commit+&out.commit;
            return Ok(self.mixed_view(out));
        }
        // While an upstream English run, English window or inline letters are on screen, upstream
        // owns the key (punctuation ends a run, Backspace takes a letter); otherwise the Windows
        // policy runs first and upstream English only sees what it leaves.
        if self.english && self.upstream_english_busy() {
            if key.kind==KeyKind::Char && key.ch.is_ascii_alphabetic() {
                let actual=self.latin_character(key);
                let mut key=key;key.modifiers=if actual.is_ascii_uppercase(){MOD_SHIFT}else{0};
                return self.key_original(key);
            }
            return self.key_original(key);
        }
        if key.kind == KeyKind::Enter && key.modifiers == 0 {
            if let Some(out) = self.commit_through_cursor()? { return Ok(self.mixed_view(out)); }
        }
        let out = self.windows_key(key)?;
        if self.drun.is_some() {
            // The key upstream let through opened a digit run: the run owns these keys now.
            self.mixed_raw.clear();
            self.mixed_latin = false;
            self.windows_raw.clear();
        }
        Ok(self.mixed_view(out))
    }

    pub fn key_with_latin(&mut self, key: Key, display: Option<char>) -> Result<Output, EngineError> {
        self.mixed_display_char = display.filter(|ch| ch.is_ascii_alphabetic());
        let result = self.key(key);
        self.mixed_display_char = None;
        result
    }

    fn latin_character(&self, key: Key) -> char {
        self.mixed_display_char.unwrap_or_else(|| {
            if key.modifiers == MOD_SHIFT { key.ch.to_ascii_uppercase() } else { key.ch }
        })
    }

    fn mixed_view(&self, mut out: Output) -> Output {
        if self.is_mixed_hint() {
            out.candidates = vec![self.pending(), self.mixed_raw.clone()];
            out.selected = None;
            out.columns = 0;
            out.first = 0;
            out.total = 1;
        }
        out
    }

    fn mixed_key(&mut self, key: Key) -> Result<Output, EngineError> {
        if self.mixed_latin {
            if key.modifiers & !MOD_SHIFT != 0 {
                return self.passthrough(String::new());
            }
            if key.kind == KeyKind::Char && key.ch.is_ascii_alphabetic() {
                self.mixed_raw.push(self.latin_character(key));
                return self.handled();
            }
            if key.kind == KeyKind::Char && key.modifiers == 0
                && self.layout.symbol_of(key.ch).is_some()
            {
                let next = format!("{}{}", self.mixed_raw, key.ch);
                // Zhuyin also uses digits and punctuation (e.g. 5j/ = ㄓㄨㄥ).
                // Retain a possible suffix until its tone arrives.
                if (1..next.len()).any(|split| self.mixed_reading(&next[split..]).is_some()) {
                    self.mixed_raw = next;
                    return self.handled();
                }
            }
            match key.kind {
                KeyKind::Backspace => {
                    self.mixed_raw.pop();
                    if self.mixed_raw.is_empty() {
                        self.mixed_latin = false;
                    }
                    return self.handled();
                }
                KeyKind::Esc => {
                    self.mixed_raw.clear();
                    self.mixed_latin = false;
                    return self.handled();
                }
                KeyKind::Down | KeyKind::Tab => {
                    let raw = std::mem::take(&mut self.mixed_raw);
                    self.mixed_latin = false;
                    for ch in raw.chars() {
                        self.key_original(Key::ch(ch.to_ascii_lowercase(), 0))?;
                    }
                    return self.key_original(Key::new(KeyKind::Down));
                }
                _ => return self.finish_latin(key),
            }
        }
        // With upstream English on, Shift+letter starts its English run instead.
        if self.mixed_input && !self.english
            && key.kind == KeyKind::Char
            && key.modifiers == MOD_SHIFT
            && key.ch.is_ascii_alphabetic()
            && (self.pending().is_empty() || !self.mixed_raw.is_empty())
            && self.cands.is_none()
        {
            self.mixed_raw.push(self.latin_character(key));
            self.mixed_latin = true;
            self.clear_pend();
            self.clear_pred();
            return self.handled();
        }
        if self.mixed_input && !self.mixed_raw.is_empty() && key.modifiers == 0 {
            match key.kind {
                KeyKind::Enter => {
                    let raw = std::mem::take(&mut self.mixed_raw);
                    let prefix = self.mixed_text(&raw)?.commit;
                    let mut out = self.key_original(key)?;
                    out.commit = prefix + &out.commit;
                    return Ok(out);
                }
                KeyKind::Backspace => {
                    self.mixed_raw.pop();
                    self.clear_pend();
                    self.clear_pred();
                    for ch in self.mixed_raw.clone().chars() { self.key_original(Key::ch(ch.to_ascii_lowercase(), 0))?; }
                    return self.handled();
                }
                KeyKind::Down | KeyKind::Tab => {
                    self.mixed_raw.clear();
                    return self.key_original(key);
                }
                _ => {}
            }
        }
        let tracking = self.mixed_input
            && self.cands.is_none()
            && self.pred_sel.is_none()
            && key.kind == KeyKind::Char
            && key.modifiers == 0
            && self.layout.symbol_of(key.ch).is_some();
        if tracking {
            if self.pend.iter().all(Option::is_none) {
                self.mixed_raw.clear();
            }
            self.mixed_raw.push(self.latin_character(key));
            if self.mixed_reading(&self.mixed_raw).is_none() {
                self.clear_pend();
                self.clear_pred();
                self.mixed_latin = true;
                return self.handled();
            }
        } else if key.kind == KeyKind::Backspace {
            self.mixed_raw.pop();
        } else {
            let previous = std::mem::take(&mut self.mixed_raw);
            // Incomplete readings such as 1 (ㄅ) and r (ㄐ) cannot take a tone.
            // Keep the entire literal run and digit instead of losing the tone key.
            if !previous.is_empty() && key.kind==KeyKind::Char
                && key.modifiers==0 && key.ch.is_ascii_digit() {
                if let Some(tone)=self.layout.tone_of(key.ch) {
                    let complete=self.mixed_reading_order(&previous,false).is_some_and(|mut reading| {
                        reading.push_str(TONE_MARKS[tone]);!self.lex.entries(&[reading]).is_empty()
                    });
                    if !complete {
                        self.clear_pend();self.clear_pred();self.mixed_raw=previous;
                        self.mixed_raw.push(key.ch);self.mixed_latin=true;return self.handled();
                    }
                }
            }
            let out = self.key_original(key)?;
            if !previous.is_empty() && !self.pending().is_empty()
                && (key.kind == KeyKind::Space || (key.kind == KeyKind::Char && self.layout.tone_of(key.ch).is_some())) {
                self.mixed_raw = previous;
                return Ok(self.view(out.handled, out.commit));
            }
            return Ok(out);
        }
        self.key_original(key)
    }
}
