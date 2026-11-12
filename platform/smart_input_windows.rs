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
        e.set_upstream_english(true,true,true,true).unwrap();
        for ch in "rust".chars(){e.key_with_latin(Key::ch(ch,0),Some(ch.to_ascii_uppercase())).unwrap();}
        assert_eq!(e.key(Key::new(KeyKind::Enter)).unwrap().commit,"RUST");
        let out=e.key_with_latin(Key::ch('r',MOD_SHIFT),Some('r')).unwrap();
        assert_eq!(out.preedit,"r");assert_eq!(out.english_run_utf16_len,1);
        e.key_with_latin(Key::ch('u',0),Some('U')).unwrap();
        assert_eq!(e.key(Key::new(KeyKind::Enter)).unwrap().commit,"rU");
        for ch in "su".chars(){e.key_with_latin(Key::ch(ch,0),Some(ch.to_ascii_uppercase())).unwrap();}
        e.key(Key::ch('3',0)).unwrap();assert_eq!(e.key(Key::new(KeyKind::Enter)).unwrap().commit,"你");
        e.layout=Layout::Eten;e.set_upstream_english(true,true,true,true).unwrap();
        assert!(!e.english,"upstream English never turns on for Eten, which it does not support");
    }
    #[test]
    fn symbols_kept_behind_a_reading_stay_before_an_english_run() {
        let mut e=engine();e.set_mixed_input(false);
        e.set_upstream_english(true,true,true,true).unwrap();
        e.set_windows_input_options(1 << 22);
        e.key(Key::ch('s',0)).unwrap();
        assert_eq!(e.key(Key::ch('\\',MOD_CONTROL)).unwrap().preedit,"ㄋ、","the kept unfinished reading");
        assert_eq!(e.key(Key::ch('a',MOD_SHIFT)).unwrap().preedit,"、A");
        assert_eq!(e.key(Key::new(KeyKind::Enter)).unwrap().commit,"、A");
    }
    #[test]
    fn incomplete_reading_keeps_unconvertible_tone_digits() {
        let lex=Arc::new(Lexicon::parse("ㄋㄧˇ 你 -0.1\nㄐㄧˇ 己 -0.1\nㄅㄚˋ 爸 -0.1\nㄓˊ 直 -0.1\n").unwrap());
        {
            let mut e=Engine::with_lexicon(lex.clone(),Layout::Standard);
            e.set_mixed_input(true);
            for digits in ["13","14","16","17","13467","1234567890",
                "r3","r4","r6","r7","r3467","q3","w3","e3","s3","d3","f3","a3","z3","x3","c3","v3","r3a"] {
                assert_eq!(type_keys(&mut e,digits).preedit,digits);
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
    fn upstream_english_keeps_the_windows_punctuation_while_idle() {
        let mut e = engine();
        e.set_mixed_input(false);
        e.set_upstream_english(true, true, true, true).unwrap();
        // Shift+letter starts upstream's English run; the Windows raw state stays out of it.
        let out = e.key(Key::ch('r', MOD_SHIFT)).unwrap();
        assert_eq!(out.english_run_utf16_len, 1);
        assert!(e.mixed_raw.is_empty());
        // While the run is open upstream owns punctuation: a non-Zhuyin mark ends the run into a token.
        let out = e.key(Key::ch('[', 0)).unwrap();
        assert_eq!(out.english_run_utf16_len, 0);
        e.reset(ResetMode::Discard);
        // Idle again, the Windows policy maps the symbol as before.
        assert_eq!(e.key(Key::ch('[', 0)).unwrap().preedit, "「");
        e.reset(ResetMode::Discard);
        // Switching to raw-key mixing leaves only the Windows policy, whose order rule turns non-Zhuyin keys into English.
        e.set_upstream_english(false, true, true, true).unwrap();
        e.set_mixed_input(true);
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

    #[test]
    fn caps_lock_shifted_english_allows_following_chinese_without_breaking_models() {
        let lex = Arc::new(
            Lexicon::parse(
                "ㄏㄠˇ 好 -0.1\nㄩㄥˋ 用 -0.1\nㄇㄚ˙ 嗎 -0.1\nㄏㄠˇ-ㄩㄥˋ 好用 -0.05\nㄅㄚˋ 爸 -0.1\nㄚˋ 亞 -0.1\nㄋㄧˇ 你 -0.1\n",
            )
            .unwrap(),
        );
        for english in [false, true] {
            let mut e = Engine::with_lexicon(lex.clone(), Layout::Standard);
            e.set_mixed_input(!english);
            e.set_windows_input_options(0);
            e.set_upstream_english(english, true, true, true).unwrap();

            // Reordered readings belong to raw-key mixing; upstream mode keeps its ordered contract.
            let tails: &[&str] = if english { &["CL3M/4A87"] } else { &["CL3M/4A87", "LC3M/4A87"] };
            for tail in tails {
                for ch in "gemini".chars() {
                    e.key_with_latin(Key::ch(ch, MOD_SHIFT), Some(ch)).unwrap();
                }
                for ch in tail.chars() {
                    let lower = ch.to_ascii_lowercase();
                    let display = ch.is_ascii_alphabetic().then_some(ch);
                    e.key_with_latin(Key::ch(lower, 0), display).unwrap();
                }
                assert_eq!(
                    e.reset(ResetMode::Commit).commit,
                    if english { "Gemini好用嗎" } else { "gemini好用嗎" },
                    "english={english} tail={tail}"
                );
            }

            // Shifted prefix boundary never lets a trailing reading steal the last shifted letter
            // (e.g. Shift+"windows" ends in 's'=ㄋ, so following 'u'+'3' stays literal "windowsU3").
            for ch in "windows".chars() {
                e.key_with_latin(Key::ch(ch, MOD_SHIFT), Some(ch)).unwrap();
            }
            e.key_with_latin(Key::ch('u', 0), Some('U')).unwrap();
            e.key(Key::ch('3', 0)).unwrap();
            assert_eq!(
                e.reset(ResetMode::Commit).commit,
                "windowsU3",
                "english={english}"
            );

            // Model strings such as A84 keep digits literal after a shifted letter.
            e.key_with_latin(Key::ch('a', MOD_SHIFT), Some('A')).unwrap();
            e.key(Key::ch('8', 0)).unwrap();
            e.key(Key::ch('4', 0)).unwrap();
            assert_eq!(
                e.reset(ResetMode::Commit).commit,
                "A84",
                "english={english}"
            );
        }
    }
}

impl Engine {
    pub fn insert_literal_space(&mut self) -> Result<Output, EngineError> {
        let mut flushed = if self.drun.is_some() { self.digit_flush()?.0 } else { String::new() };
        if !self.run.is_empty() { flushed.push_str(&self.end_run()?); }
        let raw = std::mem::take(&mut self.mixed_raw);
        self.mixed_latin = false;
        self.mixed_explicit_english = false;
        let prefix = self.mixed_text(&raw)?.commit;
        let mut out = self.mixed_text(" ")?;
        out.commit = flushed + &prefix + &out.commit;
        Ok(out)
    }
    pub fn is_mixed_pending(&self) -> bool {
        self.drun.is_some() || self.mixed_latin || !self.mixed_raw.is_empty()
    }
    pub fn set_mixed_input(&mut self, mixed: bool) {
        self.mixed_input = mixed;
    }
    pub fn set_mixed_tolerance(&mut self, enabled: bool) { self.mixed_tolerance = enabled; }

    fn has_mixed_han_entry(&self, reading: &str, tone: usize) -> bool {
        let mut syllable = reading.to_string();
        syllable.push_str(TONE_MARKS[tone]);
        self.lex.entries(&[syllable]).iter().any(|(word, _)| !windows_reading_text(word))
    }

    fn replay_mixed_reading(&mut self, reading: &str, key: Key) -> Result<Output, EngineError> {
        self.drun = None;
        self.clear_pend();
        self.clear_pred();
        self.cands = None;
        for (i, symbol) in reading.chars().enumerate() {
            let ch = self.layout.key_of_symbol(symbol).expect("reading symbol belongs to active layout");
            let (col, sym) = self.layout.symbol_of(ch).expect("layout key maps back to symbol");
            if i == 0 {
                self.start_syllable(ch, col, sym);
            } else {
                self.shadow_push(ch, col);
                self.pend[col] = Some(sym);
                self.pred_dirty = true;
            }
        }
        self.key_original(key)
    }

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
            if let Some(reading) = self.mixed_reading_order(&keys, false) {
                if self.has_mixed_han_entry(&reading, tone) { choices.insert(reading); }
            }
        }
        if choices.len() == 1 { choices.into_iter().next() } else { None }
    }
    /// Upstream English (zhen-revert/zhen-reapply), the other mixing mode beside raw-key mixing (the profile turns on
    /// one of them): the three revert triggers and the reversal guard are upstream's switches.
    pub fn set_upstream_english(&mut self, on: bool, overwrite: bool, key_order: bool, guard: bool) -> Result<(), EngineError> {
        // Upstream's English composer supports Standard; Eten keeps Windows mixed input.
        let on=on && self.layout==Layout::Standard;
        self.set_english(on)?;
        self.set_english_overwrite(overwrite)?;
        self.set_english_key_order(key_order)?;
        self.set_english_reversal_guard(guard)?;
        Ok(())
    }
    fn upstream_english_busy(&self) -> bool {
        !self.run.is_empty() || self.cands.as_ref().is_some_and(|c| c.short.is_some() || c.zh.is_some())
    }
    pub fn is_mixed_hint(&self) -> bool {
        self.mixed_input && !self.mixed_latin && !self.mixed_raw.is_empty() && !self.pending().is_empty()
    }
    pub(crate) fn mixed_order_active(&self) -> bool {
        self.mixed_input || (self.windows_shifted_boundaries && self.english && self.english_reversal_guard)
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
        let explicit_english = std::mem::take(&mut self.mixed_explicit_english);
        let shifted_end = std::mem::take(&mut self.mixed_shifted_end).min(raw.len());
        self.mixed_latin = false;
        // Once a run is literal digits, later tone-number keys remain digits.
        if !raw.is_empty() && raw.bytes().all(|c| c.is_ascii_digit())
            && key.kind==KeyKind::Char && key.modifiers==0 && key.ch.is_ascii_digit() {
            self.mixed_raw=raw;self.mixed_raw.push(key.ch);self.mixed_latin=true;
            self.mixed_explicit_english = explicit_english;
            self.mixed_shifted_end = shifted_end;
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
        if let Some(tone) = tone.filter(|_| key.kind != KeyKind::Space || !explicit_english) {
            if shifted_end == 0 {
                if let Some(reading) = self.corrected_mixed_keys(&raw, tone) {
                    let out = self.replay_mixed_reading(&reading, key)?;
                    let ending = if key.kind == KeyKind::Space { ' ' } else { key.ch };
                    if let Some(index) = self.cursor.checked_sub(1).filter(|&i| i < self.syls.len()) {
                        self.typed_literals.insert(index, format!("{raw}{ending}"));
                    }
                    return Ok(out);
                }
                // A tone (including first-tone Space on an unshifted run) confirms a complete
                // reordered syllable before checking English-word literals or suffix splits.
                if let Some(reading) = self
                    .mixed_reading_order(&raw, false)
                    .filter(|r| self.has_mixed_han_entry(r, tone))
                {
                    let out = self.replay_mixed_reading(&reading, key)?;
                    let ending = if key.kind == KeyKind::Space { ' ' } else { key.ch };
                    if let Some(index) = self.cursor.checked_sub(1).filter(|&i| i < self.syls.len()) {
                        self.typed_literals.insert(index, format!("{raw}{ending}"));
                    }
                    return Ok(out);
                }
            }
            if (key.kind != KeyKind::Space || !english_word(&raw.to_ascii_lowercase()))
                && !raw.bytes().all(|c| c.is_ascii_digit() || c == b'.')
            {
                // Hard boundary: after an incomplete literal+tone fragment (e.g. w4 + u0r3) or an
                // explicit Shift-typed English prefix (e.g. Shift+"gemini" + CL3 under Caps Lock),
                // the trailing unshifted keys form an isolated syllable without consuming the prefix.
                let non_phonetic_end = raw
                    .char_indices()
                    .rfind(|(_, ch)| self.layout.symbol_of(ch.to_ascii_lowercase()).is_none())
                    .map_or(0, |(idx, ch)| idx + ch.len_utf8());
                let boundary = non_phonetic_end.max(shifted_end);
                if boundary > 0 && boundary < raw.len() {
                    let (word, tail) = raw.split_at(boundary);
                    let tail_chars = tail.chars().count();
                    let min_tail = if boundary == shifted_end && non_phonetic_end < shifted_end { 2 } else { 1 };
                    if (min_tail..=3).contains(&tail_chars)
                        && !(key.kind == KeyKind::Space && explicit_english)
                        && !tail.bytes().all(|c| c.is_ascii_digit())
                    {
                        if let Some(reading) = self
                            .mixed_reading_order(tail, false)
                            .filter(|r| self.has_mixed_han_entry(r, tone))
                        {
                            let prefix = self.mixed_text(word)?.commit;
                            let mut out = self.replay_mixed_reading(&reading, key)?;
                            let ending = if key.kind == KeyKind::Space { ' ' } else { key.ch };
                            if let Some(index) = self.cursor.checked_sub(1).filter(|&i| i < self.syls.len()) {
                                self.typed_literals.insert(index, format!("{tail}{ending}"));
                            }
                            out.commit = prefix + &out.commit;
                            return Ok(out);
                        }
                    }
                }
                let start_split = raw.len().saturating_sub(3).max(shifted_end).max(1);
                for split in start_split..raw.len() {
                    let (word, tail) = raw.split_at(split);
                    if shifted_end > 0 && split == shifted_end && tail.chars().count() < 2 {
                        continue;
                    }
                    if key.kind == KeyKind::Space
                        && (explicit_english
                            || (!english_word(&word.to_ascii_lowercase())
                                && tail.chars().all(|ch| ch.is_ascii_alphabetic())))
                    {
                        continue;
                    }
                    if let Some(reading) = self
                        .mixed_reading(tail)
                        .filter(|r| self.has_mixed_han_entry(r, tone))
                    {
                        let prefix = self.mixed_text(word)?.commit;
                        let mut out = self.replay_mixed_reading(&reading, key)?;
                        let ending = if key.kind == KeyKind::Space { ' ' } else { key.ch };
                        if let Some(index) = self.cursor.checked_sub(1).filter(|&i| i < self.syls.len()) {
                            self.typed_literals.insert(index, format!("{tail}{ending}"));
                        }
                        out.commit = prefix + &out.commit;
                        return Ok(out);
                    }
                }
                if key.kind != KeyKind::Space {
                    for split in (start_split..raw.len()).rev() {
                        let (word, tail) = raw.split_at(split);
                        if tail.chars().count() < 2 || !english_word(&word.to_ascii_lowercase()) {
                            continue;
                        }
                        if let Some(reading) = self
                            .mixed_reading_order(tail, false)
                            .filter(|r| self.has_mixed_han_entry(r, tone))
                        {
                            let prefix = self.mixed_text(word)?.commit;
                            let mut out = self.replay_mixed_reading(&reading, key)?;
                            if let Some(index) = self.cursor.checked_sub(1).filter(|&i| i < self.syls.len()) {
                                self.typed_literals.insert(index, format!("{tail}{}", key.ch));
                            }
                            out.commit = prefix + &out.commit;
                            return Ok(out);
                        }
                    }
                }
            }
        }
        if key.kind == KeyKind::Char && key.modifiers == 0 && key.ch.is_ascii_digit() {
            self.mixed_raw = raw;
            self.mixed_raw.push(key.ch);
            self.mixed_latin = true;
            self.mixed_explicit_english = explicit_english;
            self.mixed_shifted_end = shifted_end;
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

    /// The digit of a run that is one buffered key still shown as an unfinished reading (Windows option, upstream shows the reading).
    fn lone_digit(&self) -> Option<char> {
        let run = self.drun.as_ref()?;
        (self.lone_digit_literal && run.emitted.is_empty() && run.m == 0 && run.buf.len() == 1 && run.buf[0].is_ascii_digit())
            .then(|| run.buf[0])
    }

    fn sync_run_shifted(&mut self) {
        let len = self.run.chars().count();
        if len == 0 {
            self.run_shifted.clear();
        } else if self.run_shifted.len() > len {
            self.run_shifted.truncate(len);
        } else if self.run_shifted.len() < len {
            self.run_shifted.resize(len, false);
        }
    }

    fn windows_run_tone(&mut self, c: char, tone: usize) -> Result<Output, EngineError> {
        self.sync_run_shifted();
        let chars: Vec<char> = self.run.chars().collect();
        let mut tail: Vec<(usize, char)> = Vec::new();
        let mut above = 3;
        for (i, ch) in chars.iter().enumerate().rev() {
            if self.run_shifted[i] {
                break;
            }
            match Layout::Standard.symbol_of(ch.to_ascii_lowercase()) {
                Some((col, sym)) if col < above && tail.len() < 3 => {
                    tail.push((col, sym));
                    above = col;
                }
                _ => break,
            }
        }
        let syl: String = tail
            .iter()
            .rev()
            .map(|&(_, sym)| sym)
            .chain(TONE_MARKS[tone].chars())
            .collect();
        if tail.len() >= 2 && !self.lex.entries(std::slice::from_ref(&syl)).is_empty() {
            let split_at = chars.len() - tail.len();
            let head: String = chars[..split_at].iter().collect();
            let raw_tail: String = chars[split_at..].iter().collect();
            let run_ascii = std::mem::take(&mut self.run_ascii);
            self.run.clear();
            self.run_shifted.clear();
            self.run_zh = None;
            self.revert_point = None;
            self.split = true;
            let mut commit = if head.is_empty() {
                String::new()
            } else {
                let writing = if run_ascii || self.mixed_input { head.clone() } else { english_writing(&head) };
                self.put_english(format!("{PUNCT_PREFIX}{head}"), writing, None)?
            };
            self.pred_dirty = true;
            commit.push_str(&self.insert_token(syl, None, None)?.commit);
            if self.mixed_input && raw_tail.chars().any(|ch| ch.is_ascii_uppercase()) {
                if let Some(index) = self.cursor.checked_sub(1).filter(|&i| i < self.syls.len()) {
                    self.typed_literals.insert(index, format!("{raw_tail}{c}"));
                }
            }
            self.recompute_pred();
            return Ok(self.mixed_view(self.view(true, commit)));
        }
        if self.mixed_input && chars.len() >= 2 {
            let unshifted_start = self
                .run_shifted
                .iter()
                .rposition(|&shifted| shifted)
                .map_or(0, |idx| idx + 1);
            let min_split = chars.len().saturating_sub(3).max(unshifted_start);
            let max_split = chars.len() - 2;
            for split_at in min_split..=max_split {
                let head: String = chars[..split_at].iter().collect();
                if split_at > 0
                    && split_at != unshifted_start
                    && !english_word(&head.to_ascii_lowercase())
                {
                    continue;
                }
                let raw_tail: String = chars[split_at..].iter().collect();
                if let Some(reading) = self
                    .mixed_reading_order(&raw_tail, false)
                    .filter(|r| self.has_mixed_han_entry(r, tone))
                {
                    let full_syl = format!("{reading}{}", TONE_MARKS[tone]);
                    let run_ascii = std::mem::take(&mut self.run_ascii);
                    self.run.clear();
                    self.run_shifted.clear();
                    self.run_zh = None;
                    self.revert_point = None;
                    self.split = true;
                    let mut commit = if head.is_empty() {
                        String::new()
                    } else {
                        let writing = if run_ascii || self.mixed_input { head.clone() } else { english_writing(&head) };
                        self.put_english(format!("{PUNCT_PREFIX}{head}"), writing, None)?
                    };
                    self.pred_dirty = true;
                    commit.push_str(&self.insert_token(full_syl, None, None)?.commit);
                    if let Some(index) = self.cursor.checked_sub(1).filter(|&i| i < self.syls.len()) {
                        self.typed_literals.insert(index, format!("{raw_tail}{c}"));
                    }
                    self.recompute_pred();
                    return Ok(self.mixed_view(self.view(true, commit)));
                }
            }
        }
        let was_max = self.run.len() == ENGLISH_RUN_MAX;
        let commit = self.run_push(c)?;
        if was_max {
            self.run_shifted.clear();
        }
        self.run_shifted.push(false);
        Ok(self.mixed_view(self.view(true, commit)))
    }

    pub fn key(&mut self, key: Key) -> Result<Output, EngineError> {
        // The explicit upstream Microsoft scheme owns its key rules. A window
        // keeps the scheme it opened with even if the setting changes meanwhile.
        if self.cands.as_ref().map_or(self.key_scheme == 1, |c| c.ms) {
            return self.key_original(key);
        }
        // Literal tokens keep typed order and use the existing cursor, deletion and commit paths.
        // This precedes English/digit interpretation, including tone keys that otherwise start a digit run.
        if self.zhuyin_only && key.modifiers == 0 {
            match key.kind {
                KeyKind::Char => {
                    if let Some(symbol) = self.layout.symbol_of(key.ch).map(|(_, symbol)| symbol)
                        .or_else(|| self.layout.key_tone_mark(key.ch)) {
                        return self.insert_windows_literal(&symbol.to_string());
                    }
                }
                KeyKind::Space => return self.insert_windows_literal(" "),
                KeyKind::Up | KeyKind::Down => return self.handled(),
                _ => {}
            }
        }
        // An open digit run (upstream digits.md A.3.4) sees every key first, as in upstream's dispatch.
        // The Windows punctuation, literal and mixed policies would otherwise keep its tail as an
        // unfinished reading (1 then Ctrl+\ showing ㄅ、 instead of 1、).
        if self.drun.is_some() {
            // Commit, candidate opening and cursor movement preserve the displayed digit.
            // Its Zhuyin symbol remains selectable in the opened candidate list.
            if matches!(key.kind, KeyKind::Enter | KeyKind::Down | KeyKind::Left | KeyKind::Right | KeyKind::Home | KeyKind::End)
                && self.lone_digit().is_some() {
                if let Some(run) = self.drun.as_mut() { run.m = 1; }
                self.clear_pend();
            }
            let before_syls = self.syls.len();
            let out = self.key_original(key)?;
            // A closed syllable from the digit buffer (including reordered keys such as 823 = 打)
            // keeps its raw keys selectable from both candidate anchors when mixed input is on.
            if self.mixed_input && self.drun.is_none() && self.syls.len() > before_syls {
                if let (Some(keys), Some(index)) = (
                    self.last_digits.clone(),
                    self.cursor.checked_sub(1).filter(|&i| i < self.syls.len()),
                ) {
                    self.typed_literals.entry(index).or_insert(keys);
                }
            }
            // A non-digit Zhuyin key can end the digit run with a whole unfinished
            // reading (2 then u = ㄉㄧ, or 8 then w = ㄊㄚ). Hand all its keys back, in typed order;
            // tracking only the next key loses the initial at the following tone.
            if self.drun.is_none() && self.windows_phonetic_pending()
                && self.cands.is_none() && !self.upstream_english_busy()
            {
                self.windows_raw = self.shadow_text();
                if self.mixed_input {
                    self.mixed_raw = self.windows_raw.clone();
                    self.mixed_shifted_end = 0;
                }
            }
            return Ok(self.view(out.handled, out.commit));
        }
        // Upstream's digit run replaces rule 22a. In the Windows interaction the switch asks for the mark, so it wins on
        // an empty composition; the Shanjie interaction keeps upstream's digit run.
        if self.tones_over_digits && self.standalone_tones && self.digits_active()
            && key.kind == KeyKind::Char && key.modifiers == 0
            && self.syls.is_empty() && self.cands.is_none() && self.pred_sel.is_none()
            && !self.windows_phonetic_pending() && self.run.is_empty()
            && !self.mixed_latin && self.mixed_raw.is_empty() && !self.upstream_english_busy()
        {
            if let Some(tone) = self.layout.tone_of(key.ch) {
                let mark = TONE_MARKS[tone];
                let out = self.insert_token(format!("{PUNCT_PREFIX}{mark}"), Some(mark.to_string()), None)?;
                return Ok(self.mixed_view(out));
            }
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
            let run_len = self.run.chars().count();
            self.run_shifted.clear();
            if run_len > 0 {
                self.run_shifted.resize(run_len - 1, false);
                self.run_shifted.push(true);
            }
            out.commit=commit+&out.commit;
            return Ok(self.mixed_view(out));
        }
        // While an upstream English run or English window is on screen, upstream owns the key
        // (punctuation ends a run, Backspace takes a letter); otherwise the Windows punctuation and
        // literal policy runs first and upstream English only sees what it leaves.
        if self.english && self.upstream_english_busy() {
            self.sync_run_shifted();
            if (self.mixed_input || self.windows_shifted_boundaries)
                && !self.run.is_empty() && key.kind == KeyKind::Char && key.modifiers == 0 {
                if let Some(tone) = Layout::Standard.tone_of(key.ch) {
                    return self.windows_run_tone(key.ch, tone);
                }
            }
            if key.kind==KeyKind::Char && key.ch.is_ascii_alphabetic() {
                let was_shifted = key.modifiers == MOD_SHIFT;
                let accept_letter = !self.run.is_empty() && (key.modifiers == 0 || key.modifiers == MOD_SHIFT);
                let was_max = !self.run.is_empty() && self.run.len() == ENGLISH_RUN_MAX;
                let actual=self.latin_character(key);
                let mut key=key;key.modifiers=if actual.is_ascii_uppercase(){MOD_SHIFT}else{0};
                let out = self.key_original(key)?;
                if accept_letter && !self.run.is_empty() {
                    if was_max {
                        self.run_shifted.clear();
                    }
                    self.run_shifted.push(was_shifted);
                }
                self.sync_run_shifted();
                return Ok(out);
            }
            let was_max = !self.run.is_empty() && self.run.len() == ENGLISH_RUN_MAX;
            let append_unshifted = !self.run.is_empty()
                && key.kind == KeyKind::Char
                && key.modifiers == 0
                && Layout::Standard.symbol_of(key.ch).is_some();
            let out = self.key_original(key)?;
            if append_unshifted && !self.run.is_empty() {
                if was_max {
                    self.run_shifted.clear();
                }
                self.run_shifted.push(false);
            }
            self.sync_run_shifted();
            return Ok(out);
        }
        if key.kind == KeyKind::Enter && key.modifiers == 0 {
            if let Some(out) = self.commit_through_cursor()? { return Ok(self.mixed_view(out)); }
        }
        let mut out = self.windows_key(key)?;
        if self.drun.is_some() {
            // The key upstream let through opened a digit run: the run owns these keys now.
            self.mixed_raw.clear();
            self.mixed_latin = false;
            self.mixed_explicit_english = false;
            self.mixed_shifted_end = 0;
            self.windows_raw.clear();
            // windows_key built its output while the raw key still overlapped the
            // new digit buffer. Rebuild after transferring ownership, exactly once.
            out = self.view(out.handled, out.commit);
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
                if key.modifiers == MOD_SHIFT {
                    self.mixed_explicit_english = true;
                    self.mixed_shifted_end = self.mixed_raw.len();
                }
                return self.handled();
            }
            if key.kind == KeyKind::Char && key.modifiers == 0
                && self.layout.symbol_of(key.ch).is_some()
            {
                let next = format!("{}{}", self.mixed_raw, key.ch);
                // Zhuyin also uses digits and punctuation (e.g. 5j/ = ㄓㄨㄥ, u0r = ㄐㄧㄢ).
                // Retain a possible whole or trailing reading until its tone arrives.
                if (0..next.len()).any(|split| self.mixed_reading_order(&next[split..], false).is_some()) {
                    self.mixed_raw = next;
                    return self.handled();
                }
            }
            match key.kind {
                KeyKind::Backspace => {
                    self.mixed_raw.pop();
                    self.mixed_shifted_end = self.mixed_shifted_end.min(self.mixed_raw.len());
                    if self.mixed_raw.is_empty() {
                        self.mixed_latin = false;
                        self.mixed_explicit_english = false;
                    }
                    return self.handled();
                }
                KeyKind::Esc => {
                    self.mixed_raw.clear();
                    self.mixed_latin = false;
                    self.mixed_explicit_english = false;
                    self.mixed_shifted_end = 0;
                    return self.handled();
                }
                KeyKind::Down | KeyKind::Tab => {
                    let raw = std::mem::take(&mut self.mixed_raw);
                    self.mixed_latin = false;
                    self.mixed_explicit_english = false;
                    self.mixed_shifted_end = 0;
                    for ch in raw.chars() {
                        self.key_original(Key::ch(ch.to_ascii_lowercase(), 0))?;
                    }
                    return self.key_original(Key::new(KeyKind::Down));
                }
                _ => return self.finish_latin(key),
            }
        }
        if self.mixed_input
            && key.kind == KeyKind::Char
            && key.modifiers == MOD_SHIFT
            && key.ch.is_ascii_alphabetic()
            && (self.pending().is_empty() || !self.mixed_raw.is_empty())
            && self.cands.is_none()
        {
            self.mixed_raw.push(self.latin_character(key));
            self.mixed_latin = true;
            self.mixed_explicit_english = true;
            self.mixed_shifted_end = self.mixed_raw.len();
            self.clear_pend();
            self.clear_pred();
            return self.handled();
        }
        if self.mixed_input && !self.mixed_raw.is_empty() && key.modifiers == 0 {
            match key.kind {
                KeyKind::Enter => {
                    let raw = std::mem::take(&mut self.mixed_raw);
                    self.mixed_shifted_end = 0;
                    let prefix = self.mixed_text(&raw)?.commit;
                    let mut out = self.key_original(key)?;
                    out.commit = prefix + &out.commit;
                    return Ok(out);
                }
                KeyKind::Backspace => {
                    self.mixed_raw.pop();
                    self.mixed_shifted_end = self.mixed_shifted_end.min(self.mixed_raw.len());
                    self.clear_pend();
                    self.clear_pred();
                    for ch in self.mixed_raw.clone().chars() { self.key_original(Key::ch(ch.to_ascii_lowercase(), 0))?; }
                    return self.handled();
                }
                KeyKind::Down | KeyKind::Tab => {
                    self.mixed_raw.clear();
                    self.mixed_shifted_end = 0;
                    return self.key_original(key);
                }
                KeyKind::Left | KeyKind::Right | KeyKind::Home | KeyKind::End
                    if self.mixed_raw.bytes().all(|ch| ch.is_ascii_digit()) => {
                    // The view shows raw digits over an unfinished reading. Make those
                    // digits editable before handing navigation back to the core.
                    let prefix = self.insert_windows_literal("")?.commit;
                    let mut out = self.key_original(key)?;
                    out.commit = prefix + &out.commit;
                    return Ok(out);
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
                self.mixed_shifted_end = 0;
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
            self.mixed_shifted_end = self.mixed_shifted_end.min(self.mixed_raw.len());
        } else {
            let previous = std::mem::take(&mut self.mixed_raw);
            self.mixed_shifted_end = 0;
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
        let prev_guard = self.english_reversal_guard;
        if tracking {
            self.english_reversal_guard = true;
        }
        let res = self.key_original(key);
        self.english_reversal_guard = prev_guard;
        res
    }
}
