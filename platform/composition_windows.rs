// Windows literal input and candidate actions. The upstream decoder stays unchanged.
#[derive(Clone)]
struct WindowsLiteralCandidate {
    text: String,
    start: usize,
    end: usize,
}

fn windows_reading_text(text: &str) -> bool {
    !text.is_empty() && text.chars().all(|c| ('ㄅ'..='ㄯ').contains(&c) || "ˊˇˋ˙".contains(c))
}

#[cfg(test)]
mod windows_composition_tests {
    use super::*;
    fn engine(mixed: bool, symbols: bool) -> Engine {
        let lex = Lexicon::parse("ㄅ ㄅ -0.5\nㄋㄧˇ 你 -0.1\nㄏㄠˇ 好 -0.1\nㄋㄧˇ-ㄏㄠˇ 你好 -0.1\nㄨㄛˇ 我 -0.1\nㄨㄛ 窩 -0.1\nㄨˇ 五 -0.1\nㄋㄧˋ 逆 -0.1\nㄎㄢˋ 看 -0.1\n").unwrap();
        let mut e = Engine::with_lexicon(Arc::new(lex), Layout::Standard);
        e.set_mixed_input(mixed);
        e.set_windows_input_options(if symbols { 0 } else { 1 << 20 });
        e
    }
    fn type_keys(e: &mut Engine, keys: &str) -> Output {
        let mut out = e.view(true,String::new());
        for ch in keys.chars() {
            out = e.key(if ch == ' ' { Key::new(KeyKind::Space) } else { Key::ch(ch,0) }).unwrap();
        }
        out
    }
    #[test]
    fn literal_digits_and_symbols_are_handled_without_committing() {
        for mixed in [false,true] {
            for keys in ["3", "4", "\\", "su3()", "su3\\"] {
                let mut e = engine(mixed,true);
                let out = type_keys(&mut e,keys);
                assert!(out.handled && out.commit.is_empty(),"{mixed} {keys}");
                assert_eq!(out.preedit,keys.replace("su3","你"));
                assert_eq!(e.key(Key::new(KeyKind::Enter)).unwrap().commit,out.preedit);
            }
        }
        let mut e = engine(true,true);
        assert_eq!(type_keys(&mut e,"abc()\\").preedit,"abc()\\");
    }
    #[test]
    fn punctuation_preserves_an_unfinished_reading() {
        for mixed in [false,true] {
            let mut e = engine(mixed,true);
            type_keys(&mut e,"su");
            let out = type_keys(&mut e,"\\()");
            assert!(out.commit.is_empty() && e.windows_phonetic_pending());
            e.key(Key::new(KeyKind::Backspace)).unwrap();
            assert_eq!(type_keys(&mut e,"3").preedit,"你\\(");
        }
    }
    #[test]
    fn space_symbols_off_restores_keys_but_preserves_first_tone() {
        for mixed in [false,true] {
            let mut e = engine(mixed,false);
            let out = type_keys(&mut e,"1 ");
            assert_eq!(out.preedit,"1 ");
            assert!(out.commit.is_empty() && !e.windows_phonetic_pending());
            assert_eq!(e.key(Key::new(KeyKind::Enter)).unwrap().commit,"1 ");
            assert_eq!(type_keys(&mut e,"ji ").preedit,"窩");
            e.reset(ResetMode::Discard);
            assert_eq!(type_keys(&mut e,"su31 ").preedit,"你1 ");
        }
        let mut e = engine(false,true);
        assert_eq!(type_keys(&mut e,"1 ").preedit,"ㄅ");
        let mut e = engine(false,false);
        assert_eq!(type_keys(&mut e,"q1 ").preedit,"1 ");
    }
    #[test]
    fn full_reordered_syllable_wins_and_original_is_selectable() {
        for keys in ["ji3","ij3"] {
            let mut e = engine(true,true);
            assert_eq!(type_keys(&mut e,keys).preedit,"我");
            let out = e.key(Key::new(KeyKind::Down)).unwrap();
            let index = out.candidates.iter().position(|s| s == keys).unwrap();
            let out = e.pick(index).unwrap().unwrap();
            assert_eq!(out.preedit,keys);
            assert!(out.commit.is_empty() && !e.windows_phonetic_pending());
        }
        let mut e = engine(true,true);
        assert_eq!(type_keys(&mut e,"us4").preedit,"逆");
        assert!(e.key(Key::new(KeyKind::Down)).unwrap().candidates.contains(&"us4".into()));
        e.reset(ResetMode::Discard);
        assert_eq!(type_keys(&mut e,"callsu3").preedit,"call你");
    }
    #[test]
    fn reading_candidate_replaces_only_its_word_and_is_not_a_pending_reading() {
        let mut e = engine(false,false);
        type_keys(&mut e,"su3cl3");
        let out = e.key(Key::new(KeyKind::Down)).unwrap();
        assert_eq!(out.candidates[0],"ㄋㄧˇㄏㄠˇ");
        assert_eq!(out.selected,Some(1));
        let out = e.pick(0).unwrap().unwrap();
        assert_eq!(out.preedit,"ㄋㄧˇㄏㄠˇ");
        assert!(out.commit.is_empty() && !e.windows_phonetic_pending());
        assert!(e.fixed.iter().all(|f| f.pre.is_none()));
        assert_eq!(type_keys(&mut e," ").preedit,"ㄋㄧˇㄏㄠˇ"); // Native space operation supplies literal space.
        let out = e.insert_literal_space().unwrap();
        assert_eq!(out.preedit,"ㄋㄧˇㄏㄠˇ ");
    }
    #[test]
    fn eten_reordering_and_raw_space_and_big5_reading_only() {
        let mut e = engine(true,false);
        e.layout = Layout::Eten;
        assert_eq!(type_keys(&mut e,"ox3").preedit,"我");
        e.reset(ResetMode::Discard);
        assert_eq!(type_keys(&mut e,"b ").preedit,"b ");
        let lex = Lexicon::parse("ㄐㄧˊ 𠮷 -0.1\n").unwrap();
        let mut e = Engine::with_lexicon(Arc::new(lex),Layout::Standard);
        e.set_windows_input_options(1 << 20);
        e.set_candidate_charset(true);
        type_keys(&mut e,"ru6");
        let out = e.key(Key::new(KeyKind::Down)).unwrap();
        assert_eq!(out.candidates,vec!["ㄐㄧˊ"]);
        assert_eq!(out.selected,Some(0));
        assert_eq!(e.pick(0).unwrap().unwrap().preedit,"ㄐㄧˊ");
    }
    #[test]
    fn reordered_syllable_at_composition_limit_keeps_the_commit() {
        let mut e = engine(true,true);
        for _ in 0..MAX_SYLLABLES-1 { type_keys(&mut e,"su3"); }
        let out = type_keys(&mut e,"ij3");
        assert!(out.commit.ends_with('我'));
        assert!(out.preedit.is_empty() && e.typed_literals.is_empty());
    }
    #[test]
    fn reading_candidate_near_composition_limit_keeps_surrounding_text() {
        let mut e = engine(false,false);
        for _ in 0..MAX_SYLLABLES-1 { type_keys(&mut e,"su3"); }
        e.key(Key::new(KeyKind::Down)).unwrap();
        let out = e.pick(0).unwrap().unwrap();
        assert_eq!(out.preedit,"你".repeat(MAX_SYLLABLES-2)+"ㄋㄧˇ");
        assert!(out.commit.is_empty());
        assert_eq!(e.key(Key::new(KeyKind::Enter)).unwrap().commit,out.preedit);
    }
}

impl Engine {
    pub fn windows_phonetic_pending(&self) -> bool {
        self.pend.iter().any(Option::is_some) || !self.pend_prev.is_empty()
    }

    pub fn insert_windows_literal(&mut self, text: &str) -> Result<Output, EngineError> {
        self.cands = None;
        self.clear_pred();
        if self.windows_phonetic_pending() && !self.mixed_latin {
            // Symbols typed during a reading follow it until that reading is completed.
            self.pending_literal.push_str(text);
            return self.handled();
        }
        let raw = std::mem::take(&mut self.mixed_raw);
        self.mixed_latin = false;
        let mut commit = String::new();
        for ch in raw.chars().chain(text.chars()) {
            let out = self.insert_token(format!("{PUNCT_PREFIX}{ch}"), Some(ch.to_string()), None)?;
            commit.push_str(&out.commit);
        }
        self.windows_raw.clear();
        Ok(self.view(true, commit))
    }

    fn windows_key(&mut self, key: Key) -> Result<Output, EngineError> {
        let plain = key.modifiers == 0;
        if plain && key.kind == KeyKind::Backspace && !self.pending_literal.is_empty() {
            self.pending_literal.pop();
            return self.handled();
        }
        if plain && key.kind == KeyKind::Esc {
            self.pending_literal.clear();
        }
        if key.kind == KeyKind::Char && (plain || key.modifiers == MOD_SHIFT) {
            let mapped = if plain && self.windows_plain_punct(key.ch) {
                PLAIN_PUNCT.iter().find(|(ch,_)| *ch == key.ch).map(|(_,mark)| *mark)
            } else if key.modifiers == MOD_SHIFT && self.windows_shift_punct(key.ch) {
                SHIFT_PUNCT.iter().find(|(ch,_)| *ch == key.ch).map(|(_,mark)| *mark)
            } else { None };
            // Some ETEN symbol keys also have a punctuation mapping: phonetics win.
            if let Some(mark) = mapped.filter(|_| !plain || self.layout.symbol_of(key.ch).is_none()) {
                return self.insert_windows_literal(&mark.to_string());
            }
            if plain && self.layout.symbol_of(key.ch).is_none()
                && ((key.ch.is_ascii_punctuation() && self.layout.tone_of(key.ch).is_none())
                    || (self.cands.is_none() && !self.windows_phonetic_pending()
                        && self.layout.tone_of(key.ch).is_some() && !self.standalone_tones
                        && !self.mixed_latin)) {
                return self.insert_windows_literal(&key.ch.to_string());
            }
        }
        let was_latin = self.mixed_latin;
        let original = if self.mixed_input { self.mixed_raw.clone() } else { self.windows_raw.clone() };
        if plain && key.kind == KeyKind::Space && !self.space_inputs_zhuyin
            && self.windows_phonetic_pending() && !was_latin && self.pend_prev.is_empty()
            && !self.lex.entries(&[self.pending()]).iter().any(|(word,_)| !windows_reading_text(word)) {
            self.clear_pend();
            self.mixed_raw.clear();
            let suffix = std::mem::take(&mut self.pending_literal);
            return self.insert_windows_literal(&(original + &suffix + " "));
        }
        if !self.mixed_input && self.cands.is_none() && self.pred_sel.is_none() && plain {
            if key.kind == KeyKind::Char && self.layout.symbol_of(key.ch).is_some() {
                if !self.windows_phonetic_pending() { self.windows_raw.clear(); }
                if !self.abbr_active() {
                    let column = self.layout.symbol_of(key.ch).unwrap().0;
                    self.windows_raw = self.windows_raw.chars()
                        .filter(|&ch| !self.layout.symbol_of(ch).is_some_and(|(col,_)| col == column)).collect();
                }
                self.windows_raw.push(key.ch);
            } else if key.kind == KeyKind::Backspace {
                if let Some(column) = (0..3).rev().find(|&i| self.pend[i].is_some()) {
                    let position = self.windows_raw.char_indices().rev()
                        .find(|(_,ch)| self.layout.symbol_of(*ch).is_some_and(|(col,_)| col == column))
                        .map(|(index,_)| index);
                    if let Some(index) = position { self.windows_raw.remove(index); }
                }
            }
        }
        let before = self.syls.len();
        let mut out = self.mixed_key(key)?;
        if !self.windows_phonetic_pending() {
            if self.mixed_input && !was_latin && !original.is_empty() && self.syls.len() > before
                && (key.kind == KeyKind::Space || (plain && self.layout.tone_of(key.ch).is_some())) {
                let ending = if key.kind == KeyKind::Space { ' ' } else { key.ch };
                self.typed_literals.insert(self.cursor - 1, format!("{original}{ending}"));
            }
            self.windows_raw.clear();
            let suffix = std::mem::take(&mut self.pending_literal);
            if !suffix.is_empty() {
                let commit = out.commit;
                out = self.insert_windows_literal(&suffix)?;
                out.commit = commit + &out.commit;
            }
        }
        Ok(out)
    }

    fn choose_windows_literal(&mut self, index: usize) -> Result<bool, EngineError> {
        let candidate = self.cands.as_ref().and_then(|c| c.literals.get(&index)).cloned();
        let Some(candidate) = candidate else { return Ok(false) };
        self.cands = None;
        self.clear_pred();
        self.clear_pend();
        self.mixed_raw.clear();
        self.mixed_latin = false;
        for index in (candidate.start..candidate.end).rev() { self.remove_syllable(index)?; }
        self.cursor = candidate.start;
        // One literal word replaces its original span. Expanding it into several
        // cells could reach MAX_SYLLABLES and silently discard an automatic commit.
        self.insert_token(format!("{PUNCT_PREFIX}{}",candidate.text),Some(candidate.text),None)?;
        Ok(true)
    }
}
