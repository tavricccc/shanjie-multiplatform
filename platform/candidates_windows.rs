// Appended inside engine.rs in the generated Windows build only.
impl Engine {
    fn windows_punctuation_alternatives(&self, typed: &str) -> Vec<String> {
        let mut chars = typed.chars();
        let Some(mark) = chars.next() else { return Vec::new() };
        if chars.next().is_some() { return Vec::new(); }
        let mut alternatives = self.punct.get(&mark).cloned().unwrap_or_default();
        if !self.windows_punctuation_candidates { return alternatives; }
        // Include width variants and related marks even when the upstream table
        // has no entry for the typed glyph (notably the Windows ［／］ keys).
        for family in ["［「『【《〈〔〖（｛[({", "］」』】》〉〕〗）｝])}",
            "，,、", "。．.⋯…", "；：;:", "／＼/\\、÷", "‘’＇'", "“”＂\"",
            "！!", "？?", "％%‰", "＋+±", "－-—–―─→←", "＊*×", "＝=≠≈≒",
            "＄$€£¥￥", "＠@", "＃#", "＆&", "＿_﹏￣", "～~〜", "＾^︿︽↑",
            "｜|︱↑↓↕←→↔↖↗↘↙‖¦", "＜<〈《‹«←≤", "＞>〉》›»→≥",
            "｀`ˋˊ´", "·‧・•"] {
            if family.contains(mark) {
                for symbol in family.chars() {
                    alternatives.push(symbol.to_string());
                    if let Some(upstream) = self.punct.get(&symbol) {
                        alternatives.extend(upstream.iter().cloned());
                    }
                }
            }
        }
        alternatives
    }

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
            let alternatives = self.windows_punctuation_alternatives(&typed);
            let mut seen = HashSet::new();
            let list = std::iter::once(typed).chain(alternatives)
                .filter(|word| seen.insert(word.clone())).map(|word| (word, 1)).collect();
            self.cands = self.make_candidates(list);
            return;
        }
        let available = (start..self.syls.len()).take_while(|&i| !self.is_punct(i)).count();
        let mut seen = HashSet::new();
        let mut list = Vec::new();
        for length in (1..=self.lex.max_len.min(available)).rev() {
            for (word, _) in self.lex.entries(&self.syls[start..start + length]) {
                if seen.insert(word) { list.push((word.to_string(), length)); }
            }
        }
        self.cands = self.make_candidates(list);
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
    fn punctuation_families_are_editable_from_both_candidate_anchors() {
        for right in [false, true] {
            let mut e = sample(right);
            e.set_windows_input_options(0);
            for (typed, replacement) in [("[", "「"), ("]", "」"), ("［", "「"), ("］", "」"),
                ("（", "『"), ("）", "』"), ("「", "［"), ("／", "＼"), ("；", "："),
                ("％", "%"), ("！", "!"), ("＂", "“"), ("|", "↑"), ("｜", "↓"),
                ("｜", "↕"), ("<", "≤"), (">", "≥"), ("＾", "↑"), ("－", "→"),
                ("＿", "﹏"), ("｀", "`"), ("·", "・")] {
                e.reset(ResetMode::Discard);
                let out = e.insert_windows_punctuation(typed).unwrap();
                assert!(out.commit.is_empty());
                let mut out = e.key(Key::new(KeyKind::Down)).unwrap();
                assert_eq!(out.candidates[0], typed);
                let mut unique = HashSet::new();
                for (word, _) in &e.cands.as_ref().unwrap().list { assert!(unique.insert(word.clone())); }
                while !out.candidates.iter().any(|word| word == replacement) {
                    assert!((out.first as usize + out.candidates.len()) < out.total as usize);
                    out = e.key(Key::new(KeyKind::Space)).unwrap();
                }
                let index = out.candidates.iter().position(|word| word == replacement).unwrap();
                let out = e.pick(index).unwrap().unwrap();
                assert_eq!(out.preedit, replacement);
                assert_eq!(e.key(Key::new(KeyKind::Enter)).unwrap().commit, replacement);
            }
            assert!(e.set_punctuation("，\t、"));
            e.insert_windows_punctuation("，").unwrap();
            assert_eq!(e.key(Key::new(KeyKind::Down)).unwrap().candidates, vec!["，", "、"]);
        }
    }
}
