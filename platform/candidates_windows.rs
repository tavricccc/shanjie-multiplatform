// Appended inside engine.rs in the generated Windows build only.
impl Engine {
    pub fn set_candidate_anchor(&mut self, right: bool) {
        self.candidate_from_right = right;
        self.cands = None;
        self.clear_pred();
    }

    fn candidate_span(&self, length: usize) -> (usize, usize) {
        if self.candidate_from_right {
            (self.cursor, self.cursor + length)
        } else if self.cursor == 0 {
            (0, length)
        } else {
            (self.cursor - length, self.cursor)
        }
    }

    fn open_candidates(&mut self, rows: Vec<Pred>) {
        if !self.candidate_from_right {
            self.open_candidates_left(rows);
            self.decorate_windows_candidates();
            return;
        }
        if self.syls.is_empty() { return; }
        // At the end, select the final character and put the caret on its left.
        self.cursor = self.cursor.min(self.syls.len() - 1);
        let start = self.cursor;
        if self.is_punct(start) {
            let typed = self.syls[start][PUNCT_PREFIX.len()..].to_string();
            let alternatives = typed.chars().next().and_then(|c| self.punct.get(&c)).cloned().unwrap_or_default();
            let mut seen = HashSet::new();
            let list = std::iter::once(typed).chain(alternatives)
                .filter(|word| seen.insert(word.clone())).map(|word| (word, 1)).collect();
            self.cands = self.make_candidates(list);
            return;
        }
        let available = (start..self.syls.len()).take_while(|&i| !self.is_punct(i)).count();
        let mut seen = HashSet::new();
        let mut list = Vec::new();
        let today = if self.learner.has_new_words() { self.today() } else { 0 };
        for length in (1..=self.lex.max_len.max(NEWWORD_MAX).min(available)).rev() {
            let key = &self.syls[start..start + length];
            let lexicon = self.lex.entries(key);
            if (NEWWORD_MIN..=NEWWORD_MAX).contains(&length) {
                for word in self.learner.new_words_all(key, today) {
                    if !lexicon.iter().any(|(w, _)| *w == word) && seen.insert(word) {
                        list.push((word.to_string(), length));
                    }
                }
            }
            for (word, _) in lexicon {
                if seen.insert(word) { list.push((word.to_string(), length)); }
            }
        }
        let mut alt = HashMap::new();
        for length in (1..=self.lex.max_len.min(available)).rev() {
            for (word, reading) in self.lex.tolerant_words(&self.syls[start..start + length], self.fuzzy) {
                if seen.insert(word) {
                    alt.insert(list.len(), reading);
                    list.push((word.to_string(), length));
                }
            }
        }
        self.cands = self.make_candidates_with(list, alt);
    }

    pub fn cycle_windows_candidate(&mut self) -> Result<Output,EngineError> {
        let cursor = self.cursor; self.clear_pred(); self.open_candidates(Vec::new());
        let Some(cands) = &self.cands else { self.cursor = cursor; return self.handled(); };
        let shown: Vec<char> = self.display.chars().collect();
        let current = cands.list.iter().position(|(word,length)| {
            let (start,end) = self.candidate_span(*length);
            let before = (0..start).map(|i| self.token_width(i)).sum::<usize>();
            let after = (0..end).map(|i| self.token_width(i)).sum::<usize>();
            shown[before..after].iter().collect::<String>() == *word
        });
        let next = (1..=cands.list.len()).map(|offset| (current.unwrap_or(cands.list.len()-1)+offset) % cands.list.len())
            .find(|i| !cands.literals.contains_key(i));
        let Some(next) = next else { self.cands = None; self.cursor = cursor; return self.handled(); };
        self.choose(next)?; self.cursor = cursor;
        self.handled()
    }
}

#[cfg(test)]
mod windows_candidates {
    use super::*;
    fn sample(right: bool) -> Engine {
        let lex = Lexicon::parse("ㄋㄧˇ 你 -0.1\nㄋㄧˇ 妳 -1\nㄏㄠˇ 好 -0.1\nㄏㄠˇ 郝 -1\nㄋㄧˇ-ㄏㄠˇ 你好 -0.1\n").unwrap();
        let mut engine = Engine::with_lexicon(Arc::new(lex), Layout::Standard);
        engine.set_candidate_anchor(right);
        for ch in "su3cl3".chars() { engine.key(Key::ch(ch, 0)).unwrap(); }
        engine
    }
    #[test]
    fn right_of_caret_and_end_boundary() {
        let mut engine = sample(true);
        let out = engine.key(Key::new(KeyKind::Down)).unwrap();
        assert_eq!(out.cursor_utf16, 1);
        assert_eq!(out.candidates, vec!["好", "郝"]);
        let out = engine.pick(1).unwrap().unwrap();
        assert_eq!(out.preedit, "你郝");
        assert_eq!(out.cursor_utf16, 2);
        engine.key(Key::new(KeyKind::Home)).unwrap();
        engine.key(Key::new(KeyKind::Right)).unwrap();
        assert_eq!(engine.key(Key::new(KeyKind::Down)).unwrap().candidates, vec!["好", "郝"]);
    }
    #[test]
    fn rightward_phrase_and_left_profile() {
        let mut engine = sample(true);
        engine.key(Key::new(KeyKind::Home)).unwrap();
        let out = engine.key(Key::new(KeyKind::Down)).unwrap();
        assert_eq!(out.candidates[0], "你好");
        assert_eq!(engine.pick(0).unwrap().unwrap().cursor_utf16, 2);
        let mut original = sample(false);
        original.key(Key::new(KeyKind::Left)).unwrap();
        assert_eq!(original.key(Key::new(KeyKind::Down)).unwrap().candidates, vec!["你", "妳"]);
    }

    #[test]
    fn right_anchor_lists_faded_new_words_and_forgets_them() {
        let mut e = sample(true);
        e.set_today(Some(20_000));
        let reading = e.syls.clone();
        e.learner.learn_new(&reading, "擬郝", 19_900);
        e.key(Key::new(KeyKind::Home)).unwrap();
        e.key(Key::new(KeyKind::Down)).unwrap();
        assert_eq!(e.cands.as_ref().unwrap().list[0], ("擬郝".into(), 2));
        let out = e.pick(0).unwrap().unwrap();
        assert_eq!(out.preedit, "擬郝");
        e.key(Key::new(KeyKind::Home)).unwrap();
        e.key(Key::new(KeyKind::Down)).unwrap();
        e.key(Key { kind: KeyKind::Backspace, ch: '\0', modifiers: MOD_COMMAND }).unwrap();
        assert!(e.learner.new_word_day(&reading, "擬郝").is_none());
        assert!(!e.cands.as_ref().unwrap().list.iter().any(|(w, _)| w == "擬郝"));
    }

    #[test]
    fn right_anchor_tolerant_choices_keep_their_lexicon_reading_after_filtering() {
        let lex = Lexicon::parse("ㄍㄨㄥ 公 -1\nㄍㄨㄣ 棍 -2\n").unwrap();
        let mut e = Engine::with_lexicon(Arc::new(lex), Layout::Standard);
        e.set_candidate_anchor(true);
        e.set_candidate_charset(true);
        e.set_fuzzy(1).unwrap(); // ㄣ／ㄥ
        for ch in "ej/ ".chars() {
            e.key(if ch == ' ' { Key::new(KeyKind::Space) } else { Key::ch(ch, 0) }).unwrap();
        }
        e.key(Key::new(KeyKind::Down)).unwrap();
        let idx = e.cands.as_ref().unwrap().list.iter().position(|(w, _)| w == "棍").unwrap();
        assert_eq!(e.cands.as_ref().unwrap().alt[&idx], vec!["ㄍㄨㄣ"]);
        assert_eq!(e.pick(idx).unwrap().unwrap().preedit, "棍");
        assert_eq!(e.syls, vec!["ㄍㄨㄣ"]);
    }
}
