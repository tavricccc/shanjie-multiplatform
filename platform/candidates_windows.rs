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
        alternatives.extend(Self::windows_symbol_extras(mark).chars().map(|symbol| symbol.to_string()));
        alternatives
    }

    fn windows_symbol_extras(mark: char) -> &'static str {
        match mark {
            '-' | '－' => "→←…⋯",
            '/' | '／' => "↗↙",
            '\\' | '＼' => "↖↘",
            '*' | '＊' => "※",
            _ => "",
        }
    }

    pub fn set_candidate_anchor(&mut self, right: bool) {
        self.candidate_from_right = right;
        self.cands = None;
        self.clear_pred();
    }

    fn candidate_span(&self, length: usize) -> (usize, usize) {
        if self.candidate_from_right && !self.cands.as_ref().is_some_and(|c| c.mixed.is_some()) {
            (self.cursor, self.cursor + length)
        } else if self.cursor == 0 {
            (0, length)
        } else {
            (self.cursor - length, self.cursor)
        }
    }

    /// Highlight one decoded word while editing inside a sentence. The candidate
    /// anchor decides which word touches the caret at a boundary.
    pub fn windows_active_segment(&self) -> Option<[u32; 2]> {
        if self.path.len() < 2 || self.cursor >= self.syls.len()
            || self.windows_phonetic_pending() || !self.pending_literal.is_empty()
            || !self.run.is_empty() || !self.mixed_raw.is_empty() || self.drun.is_some()
        {
            return None;
        }
        let touching = if self.candidate_from_right { self.cursor } else { self.cursor.saturating_sub(1) };
        let mut start = 0;
        let (mut start, mut end) = self.path.iter().find_map(|word| {
            let end = start + word.syllables;
            let span = (start, end);
            start = end;
            (touching < end).then_some(span)
        })?;
        if let Some(candidates) = &self.cands {
            // Prediction rows may insert new units, so they do not have an exact span in the current text.
            if let Some(literal) = candidates.literals.get(&candidates.sel) {
                (start, end) = (literal.start, literal.end);
            } else if let Some((_, length)) = candidates.list.get(candidates.sel).filter(|_| candidates.sel >= candidates.rows.len() && candidates.dalt.is_none()) {
                let right = self.candidate_from_right && candidates.mixed.is_none();
                let available = if right { self.syls.len() - self.cursor } else if self.cursor == 0 { self.syls.len() } else { self.cursor };
                if *length <= available { (start, end) = self.candidate_span(*length); }
            }
        }
        if start == 0 && end == self.syls.len() { return None; }
        let offset = |end| {
            let chars = (0..end).map(|i| self.token_width(i)).sum::<usize>();
            self.display.chars().take(chars).map(char::len_utf16).sum::<usize>() as u32
        };
        Some([offset(start), offset(end)])
    }

    fn open_candidates(&mut self, rows: Vec<Pred>) {
        // A mixed word spans its English token and the Chinese suffix at the end;
        // keep that upstream span even when ordinary candidates are anchored right.
        if !self.candidate_from_right || self.mixed_item().is_some() {
            self.open_candidates_left(rows);
            self.decorate_windows_candidates();
            return;
        }
        if self.syls.is_empty() { return; }
        // At the end, select the final character and put the caret on its left.
        self.cursor = self.cursor.min(self.syls.len() - 1);
        let start = self.cursor;
        if self.is_punct(start) {
            if is_space_unit(&self.syls[start]) { return; }
            if let Some(word) = english_of(&self.syls[start]) {
                let f = self.fixed.iter().find(|f| f.start == start);
                let mut list: Vec<(String, usize)> = Self::english_writings(f.map_or(word, |f| f.word.as_str()))
                    .into_iter().map(|w| (w, 1)).collect();
                let zh = f.and_then(|f| f.zh.clone())
                    .and_then(|syl| self.lex.entries(std::slice::from_ref(&syl)).first().map(|(w, _)| (w.to_string(), syl)));
                let mut c = Cands::new(Vec::new(), self.cand_vertical, self.key_scheme == 1);
                if let Some((word, syl)) = zh {
                    list.push((word, 1));
                    c.zh = Some(syl);
                }
                c.list = list;
                self.cands = Some(c);
                return;
            }
            if let Some((d, sym)) = digit_of(&self.syls[start])
                .and_then(|d| self.digit_zhuyin_symbol(d).map(|sym| (d, sym))) {
                self.open_digit_candidates(d, start, Some(sym), rows);
                return;
            }
            let typed = self.syls[start][PUNCT_PREFIX.len()..].to_string();
            let alternatives = self.windows_punctuation_alternatives(&typed);
            let mut seen = HashSet::new();
            let list = std::iter::once(typed).chain(alternatives)
                .filter(|word| seen.insert(word.clone())).map(|word| (word, 1)).collect();
            self.cands = self.make_candidates(list);
            return;
        }
        let (mut list, alt, rows) = self.window_list_right(rows);
        let digits = self.ld_now.take().filter(|_| !list.is_empty());
        if let Some(keys) = &digits { list.push((keys.clone(), 1)); }
        let short = self.ls_now.take().filter(|(i, _)| *i == start && !list.is_empty() && digits.is_none());
        if let Some((_, word)) = &short {
            list.push((word.to_ascii_lowercase(), 1));
            list.push((word.to_ascii_uppercase(), 1));
        }
        if !rows.is_empty() || digits.is_some() || short.is_some() {
            if !list.is_empty() {
                let mut c = Cands::new(list, self.cand_vertical, self.key_scheme == 1);
                c.alt = alt;
                c.rows = rows;
                c.dalt = digits.map(DigitAlt::Digits);
                c.short = short;
                self.cands = Some(c);
            }
        } else {
            self.cands = self.make_candidates_with(list, alt);
        }
    }

    // Also used after forgetting a prediction: row metadata and the right-hand
    // lexicon span must be rebuilt together, before any index-bearing tails.
    fn window_list_right(&self, mut rows: Vec<Pred>) -> (Vec<(String, usize)>, HashMap<usize, Vec<String>>, Vec<Pred>) {
        let start = self.cursor;
        let available = (start..self.syls.len()).take_while(|&i| !self.is_punct(i)).count();
        let mut seen: HashSet<String> = rows.iter().map(|p| p.word.clone()).collect();
        let mut list: Vec<_> = rows.iter().map(|p| (p.word.clone(), p.reading.len())).collect();
        if self.pend.iter().any(Option::is_some) {
            self.variant_move(&mut list, &mut rows, true);
            return (list, HashMap::new(), rows);
        }
        let today = if self.learner.has_new_words() { self.today() } else { 0 };
        for length in (1..=self.lex.max_len.max(NEWWORD_MAX).min(available)).rev() {
            let key = &self.syls[start..start + length];
            let lexicon = self.lex.entries_with(key, self.colloq_on());
            if (NEWWORD_MIN..=NEWWORD_MAX).contains(&length) {
                for word in self.learner.new_words_all(key, today) {
                    if !lexicon.iter().any(|(w, _)| *w == word) && seen.insert(word.to_string()) {
                        list.push((word.to_string(), length));
                    }
                }
            }
            for (word, _) in lexicon {
                if seen.insert(word.to_string()) { list.push((word.to_string(), length)); }
            }
        }
        self.variant_move(&mut list, &mut rows, true);
        let mut alt = HashMap::new();
        let t3 = self.t3();
        for length in (1..=self.lex.max_len.min(available)).rev() {
            for (word, reading) in self.lex.tolerant_words(&self.syls[start..start + length], self.fuzzy, &t3[start..start + length]) {
                if seen.insert(word.to_string()) {
                    alt.insert(list.len(), reading);
                    list.push((word.to_string(), length));
                }
            }
        }
        (list, alt, rows)
    }

    fn open_digit_candidates(&mut self, mark: char, target: usize, reading: Option<char>, rows: Vec<Pred>) {
        let mut list: Vec<_> = rows.iter().map(|p| (p.word.clone(), p.reading.len())).collect();
        list.extend([(mark.to_string(), 1), (digit_layer::full_width(mark).to_string(), 1)]);
        let mut cands = Cands::new(Vec::new(), self.cand_vertical, self.key_scheme == 1);
        if let Some(symbol) = reading {
            list.push((symbol.to_string(), 1));
            cands.digit_zhuyin = Some((target, symbol));
        }
        if self.windows_punctuation_candidates {
            list.extend(Self::windows_symbol_extras(mark).chars().map(|symbol| (symbol.to_string(), 1)));
        }
        if let Some((n, syl, word)) = self.digit_syllable_alt(target + 1) {
            list.push((word, 1));
            cands.dalt = Some(DigitAlt::Syllable { start: target + 1 - n, n, syl });
        }
        cands.list = list;
        cands.rows = rows;
        self.cands = Some(cands);
    }

    fn digit_zhuyin_symbol(&self, d: char) -> Option<char> {
        if !self.windows_digit_zhuyin || self.layout != Layout::Standard {
            return None;
        }
        Layout::Standard.symbol_of(d).map(|(_, sym)| sym)
            .or_else(|| Layout::Standard.key_tone_mark(d))
    }

    fn choose_digit_zhuyin(&mut self, target_index: usize, sym: char) -> Result<String, EngineError> {
        if target_index >= self.syls.len() {
            return Ok(String::new());
        }
        self.pred_dirty = true;
        self.remove_syllable(target_index)?;
        self.cursor = target_index;
        let reading = sym.to_string();
        let use_real_reading = Layout::Standard.key_of_symbol(sym).is_some()
            && !self.lex.entries(std::slice::from_ref(&reading)).is_empty()
            && self.lm.as_ref().is_none_or(|st| {
                st.capped.best_lp_delta(std::slice::from_ref(&reading), &reading).is_some()
            });
        let (syl, word) = if use_real_reading {
            (reading.clone(), Some(reading))
        } else {
            (format!("{PUNCT_PREFIX}{sym}"), Some(reading))
        };
        Ok(self.insert_token(syl, word, None)?.commit)
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
mod windows_history_candidates {
    use super::*;

    fn engine(right: bool) -> Engine {
        let lex = Lexicon::parse("ㄋㄧˇ 你 -0.1\nㄋㄧˇ 妳 -1\nㄏㄠˇ 好 -0.1\nㄏㄠˇ 郝 -1\nㄇㄚ˙ 嗎 -0.1\nㄋㄧˇ-ㄏㄠˇ 你好 -0.1\nㄋㄧˇ-ㄏㄠˇ 妳好 -1\nㄏㄠˇ-ㄇㄚ˙ 好嗎 -1\n").unwrap();
        let mut e = Engine::with_lexicon(Arc::new(lex), Layout::Standard);
        e.set_candidate_anchor(right);
        e.set_learning(true);
        for ch in "su3cl3".chars() { e.key(Key::ch(ch, 0)).unwrap(); }
        e
    }

    #[test]
    fn general_han_window_keeps_prediction_metadata_and_span() {
        for right in [false, true] {
            for (word, reading, start) in [
                ("好嗎", vec!["ㄏㄠˇ".into(), "ㄇㄚ˙".into()], 1),
                ("妳好", vec!["ㄋㄧˇ".into(), "ㄏㄠˇ".into()], 0),
            ] {
                let mut e = engine(right);
                e.pred = vec![Pred { word: word.into(), reading: reading.clone(), start, key: "前文".into(), weight: 2.0 }];
                let opened = e.key(Key::new(KeyKind::Down)).unwrap();
                assert_eq!(opened.candidates.first().map(String::as_str), Some(word));
                let row = &e.cands.as_ref().unwrap().rows[0];
                assert_eq!((&row.reading, row.start, row.key.as_str(), row.weight), (&reading, start, "前文", 2.0));
                let chosen = e.pick(0).unwrap().unwrap();
                assert_eq!(chosen.preedit, if start == 1 { "你好嗎" } else { "妳好" });
                let fixed = e.fixed.iter().find(|f| f.word == word).unwrap();
                assert_eq!((fixed.start, fixed.end), (start, start + reading.len()));
                if start == 1 { assert_eq!(fixed.key.as_deref(), Some("前文")); }
            }
        }
    }

    #[test]
    fn general_han_digit_alternative_is_available_without_raw_mixing() {
        for right in [false, true] {
            for mixed in [false, true] {
                let mut e = engine(right);
                e.set_mixed_input(mixed);
                // The digit layer leaves this marker on the syllable it just closed.
                // This deliberately opens a Han window, not the digit-token window.
                e.last_digits = Some("823".into());
                let opened = e.key(Key::new(KeyKind::Down)).unwrap();
                let index = opened.candidates.iter().position(|w| w == "823").expect("original digits in Han candidates");
                assert!(matches!(e.cands.as_ref().unwrap().dalt, Some(DigitAlt::Digits(_))));
                assert_eq!(e.pick(index).unwrap().unwrap().preedit, "你823");
                assert!(e.learner.records().is_empty());
            }
        }
    }

    #[test]
    fn forgetting_a_general_han_prediction_keeps_digit_tail() {
        let mut e = engine(true);
        e.set_learning(true);
        let reading = vec!["ㄏㄠˇ".into(), "ㄇㄚ˙".into()];
        e.learner.teach("前文", &reading, "", "好嗎", 20_000);
        e.pred = vec![Pred { word: "好嗎".into(), reading: reading.clone(), start: 1, key: "前文".into(), weight: 2.0 }];
        e.last_digits = Some("823".into());
        e.key(Key::new(KeyKind::Down)).unwrap();
        e.forget_highlighted().unwrap();
        let c = e.cands.as_ref().unwrap();
        assert!(c.list.iter().any(|(w, _)| w == "好"));
        assert_eq!(c.list.last().map(|x| x.0.as_str()), Some("823"));
        assert!(!e.learner.records().iter().any(|r| r.reading == reading && r.word == "好嗎"));
    }

    #[test]
    fn unfinished_right_syllable_keeps_only_the_prediction_rows_and_their_span() {
        let mut e=engine(true);
        e.key(Key::ch('a',0)).unwrap(); // unfinished ㄇ after 你好
        e.pred=vec![Pred{word:"好嗎".into(),reading:vec!["ㄏㄠˇ".into(),"ㄇㄚ˙".into()],start:1,key:"前文".into(),weight:2.0}];
        let opened=e.key(Key::new(KeyKind::Down)).unwrap();
        assert_eq!(opened.candidates,vec!["好嗎"]);
        assert_eq!(e.pick(0).unwrap().unwrap().preedit,"你好嗎");
        let fixed=e.fixed.iter().find(|f|f.word=="好嗎").unwrap();
        assert_eq!((fixed.start,fixed.end),(1,3));assert_eq!(fixed.key.as_deref(),Some("前文"));
    }

    #[test]
    fn right_prediction_rows_keep_variant_order_and_tolerant_indices() {
        let variants = crate::VariantOrder::parse("ㄍㄨㄥ\t公\t功\ttest\n").unwrap();
        let lex = Arc::new(Lexicon::parse_with_variants("ㄍㄨㄥ 公 -1\nㄍㄨㄥ 功 -2\nㄍㄨㄣ 棍 -3\n", None, Some(&variants)).unwrap());
        for weight in [0.0, 2.0] {
            let mut e = Engine::with_lexicon(lex.clone(), Layout::Standard);
            e.set_candidate_anchor(true);
            e.set_fuzzy(1).unwrap();
            for ch in "ej/".chars() { e.key(Key::ch(ch, 0)).unwrap(); }
            e.key(Key::new(KeyKind::Space)).unwrap();
            e.pred = vec![Pred { word: "公".into(), reading: vec!["ㄍㄨㄥ".into()], start: 0, key: "前文".into(), weight }];
            e.key(Key::new(KeyKind::Down)).unwrap();
            let c = e.cands.as_ref().unwrap();
            assert_eq!(c.list[0].0, if weight == 0.0 { "功" } else { "公" });
            assert_eq!(c.rows.len(), usize::from(weight > 0.0));
            let alt = c.list.iter().position(|(w, _)| w == "棍").unwrap();
            assert_eq!(c.alt[&alt], vec!["ㄍㄨㄣ"]);
            assert_eq!(e.pick(alt).unwrap().unwrap().preedit, "棍");
            assert_eq!(e.syls, vec!["ㄍㄨㄣ"]);
        }
    }
}

#[cfg(test)]
mod windows_grid_tests {
    use super::*;

    fn expanded() -> Engine {
        let data: String = (0..53).map(|i| format!("ㄋㄧˇ {} -{}\n", char::from_u32(0x4e00 + i).unwrap(), i + 1)).collect();
        let mut e = Engine::with_lexicon(Arc::new(Lexicon::parse(&data).unwrap()), Layout::Standard);
        e.set_windows_input_options(0);
        for ch in "su3".chars() { e.key(Key::ch(ch, 0)).unwrap(); }
        e.key(Key::new(KeyKind::Down)).unwrap();
        let out = e.key(Key::new(KeyKind::Down)).unwrap();
        assert_eq!(out.columns, 3);
        e
    }

    #[test]
    fn arrows_stay_in_the_visual_column_and_collapse_only_at_the_top() {
        let mut e = expanded();
        for (kind, expected) in [(KeyKind::Right, 1), (KeyKind::Down, 4), (KeyKind::Down, 7),
            (KeyKind::Up, 4), (KeyKind::Up, 1)] {
            e.key(Key::new(kind)).unwrap();
            assert_eq!(e.cands.as_ref().unwrap().sel, expected);
        }
        assert_eq!(e.key(Key::new(KeyKind::Up)).unwrap().columns, 0);
        let mut e = expanded();
        let c = e.cands.as_mut().unwrap();
        c.sel = c.list.len() - 1;
        let last = c.sel;
        c.scroll();
        e.key(Key::new(KeyKind::Down)).unwrap();
        assert_eq!(e.cands.as_ref().unwrap().sel, last);
    }

    #[test]
    fn scrolled_grid_keeps_number_keys_in_nine_item_groups() {
        for (selected, digit, target) in [(10, '1', 9), (10, '9', 17), (47, '2', 46)] {
            let mut e = expanded();
            let c = e.cands.as_mut().unwrap();
            c.sel = selected;
            let expected = c.list[target].0.clone();
            c.scroll();
            let out = e.view(true, String::new());
            assert_eq!(out.first % 9, 0);
            assert_eq!(out.first as usize + out.selected.unwrap(), selected);
            let out = e.key(Key::ch(digit, 0)).unwrap();
            assert_eq!(out.preedit, expected);
        }
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
                ("｜", "↕"), ("<", "≤"), (">", "≥"), ("＾", "↑"),
                ("-", "→"), ("-", "…"), ("－", "←"), ("－", "⋯"),
                ("/", "↗"), ("／", "↙"), ("\\", "↖"), ("＼", "↘"), ("*", "※"), ("＊", "※"),
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
    fn right_anchor_tolerant_choices_keep_their_lexicon_reading() {
        let lex = Lexicon::parse("ㄍㄨㄥ 公 -1\nㄍㄨㄣ 棍 -2\n").unwrap();
        let mut e = Engine::with_lexicon(Arc::new(lex), Layout::Standard);
        e.set_candidate_anchor(true);
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

    #[test]
    fn right_anchor_lists_and_picks_tone3_sandhi_candidates() {
        let lex = Arc::new(Lexicon::parse("ㄓㄚˊ 炸 -1\nㄓㄚˇ 眨 -1\nㄧㄢˇ 眼 -1\nㄓㄚˇ-ㄧㄢˇ 眨眼 -0.5\nㄨㄛˇ 我 -0.5\n").unwrap());
        let lm = digit_test_lm();
        let capped = Arc::new(CappedLexicon::new(lex.clone(), "", &lm, None).unwrap());
        let mut e = Engine::with_lexicon(lex, Layout::Standard);
        e.set_lm(lm, capped);
        e.set_candidate_anchor(true);
        for ch in "586u03".chars() { e.key(Key::ch(ch, 0)).unwrap(); }
        e.key(Key::new(KeyKind::Home)).unwrap();
        e.key(Key::new(KeyKind::Down)).unwrap();
        let idx = e.cands.as_ref().unwrap().list.iter().position(|(w, _)| w == "眨眼").unwrap();
        assert_eq!(e.cands.as_ref().unwrap().alt[&idx], vec!["ㄓㄚˇ", "ㄧㄢˇ"]);
        assert_eq!(e.pick(idx).unwrap().unwrap().preedit, "眨眼");
        assert_eq!(e.syls, vec!["ㄓㄚˇ", "ㄧㄢˇ"]);
        e.reset(ResetMode::Discard);
        for ch in "ji6".chars() { e.key(Key::ch(ch, 0)).unwrap(); }
        assert_eq!(e.view(true, String::new()).preedit, "我");
        e.key(Key::new(KeyKind::Down)).unwrap();
        let idx = e.cands.as_ref().unwrap().list.iter().position(|(w, _)| w == "我").unwrap();
        assert_eq!(e.cands.as_ref().unwrap().alt[&idx], vec!["ㄨㄛˇ"]);
        assert_eq!(e.pick(idx).unwrap().unwrap().preedit, "我");
        assert_eq!(e.syls, vec!["ㄨㄛˇ"]);
        e.reset(ResetMode::Discard);
        e.set_tone3(false).unwrap();
        for ch in "586u03".chars() { e.key(Key::ch(ch, 0)).unwrap(); }
        e.key(Key::new(KeyKind::Home)).unwrap();
        e.key(Key::new(KeyKind::Down)).unwrap();
        assert!(!e.cands.as_ref().unwrap().list.iter().any(|(w, _)| w == "眨眼"));
    }

    #[test]
    fn right_anchor_lists_and_picks_colloquial_sandhi_candidates() {
        let lex = Arc::new(
            Lexicon::parse_with_variants(
                "ㄖㄜˋ 熱 -1\nㄊㄥ 騰 -1\nㄌㄧˋ 厲 -1\nㄏㄞˋ 害 -1\nㄌㄧˋ-ㄏㄞˋ 厲害 -0.5\n",
                Some("ㄖㄜˋ-ㄊㄥ-ㄊㄥ\t熱騰騰\t-4.5\tcolloq\n"),
                None,
            )
            .unwrap(),
        );
        let lm = digit_test_lm();
        let capped = Arc::new(CappedLexicon::new(lex.clone(), "", &lm, None).unwrap());
        let mut e = Engine::with_lexicon(lex, Layout::Standard);
        e.set_lm(lm, capped);
        e.set_candidate_anchor(true);
        // Colloquial neutral reading: ㄌㄧˋ ㄏㄞ˙ -> 厲害, restored to lexicon reading on pick.
        for ch in "xu4c97".chars() { e.key(Key::ch(ch, 0)).unwrap(); }
        e.key(Key::new(KeyKind::Home)).unwrap();
        e.key(Key::new(KeyKind::Down)).unwrap();
        let idx = e.cands.as_ref().unwrap().list.iter().position(|(w, _)| w == "厲害").unwrap();
        assert_eq!(e.cands.as_ref().unwrap().alt[&idx], vec!["ㄌㄧˋ", "ㄏㄞˋ"]);
        assert_eq!(e.pick(idx).unwrap().unwrap().preedit, "厲害");
        assert_eq!(e.syls, vec!["ㄌㄧˋ", "ㄏㄞˋ"]);
        e.reset(ResetMode::Discard);
        // Colloquial reduplicated entry (`colloq` tag): ㄖㄜˋ ㄊㄥ ㄊㄥ -> 熱騰騰.
        for ch in "bk4".chars() { e.key(Key::ch(ch, 0)).unwrap(); }
        for _ in 0..2 {
            for ch in "w/".chars() { e.key(Key::ch(ch, 0)).unwrap(); }
            e.key(Key::new(KeyKind::Space)).unwrap();
        }
        e.key(Key::new(KeyKind::Home)).unwrap();
        e.key(Key::new(KeyKind::Down)).unwrap();
        let idx = e.cands.as_ref().unwrap().list.iter().position(|(w, _)| w == "熱騰騰").unwrap();
        assert_eq!(e.pick(idx).unwrap().unwrap().preedit, "熱騰騰");
        e.reset(ResetMode::Discard);
        // Turning sandhi off hides `colloq` rows in right-anchored candidates as well.
        e.set_tone3(false).unwrap();
        for ch in "bk4".chars() { e.key(Key::ch(ch, 0)).unwrap(); }
        for _ in 0..2 {
            for ch in "w/".chars() { e.key(Key::ch(ch, 0)).unwrap(); }
            e.key(Key::new(KeyKind::Space)).unwrap();
        }
        e.key(Key::new(KeyKind::Home)).unwrap();
        e.key(Key::new(KeyKind::Down)).unwrap();
        assert!(!e.cands.as_ref().unwrap().list.iter().any(|(w, _)| w == "熱騰騰"));
    }

    fn digit_test_lm() -> Arc<Lm> {
        static LM: std::sync::OnceLock<Arc<Lm>> = std::sync::OnceLock::new();
        LM.get_or_init(|| {
            let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
            Arc::new(Lm::load(&root.join("data/lm/bigram.sjlm")).unwrap())
        }).clone()
    }

    fn digit_test_table(favor_digits: bool, with_pred_words: bool) -> Arc<DigitTable> {
        let (t, n, ds, db) = if favor_digits {
            (1_000_000_000u64, 999_999_999u64, 999_999u64, 1_000_000u64)
        } else {
            (1000, 100, 5, 100)
        };
        let mut table = format!("T\t{t}\nN\t{n}\nS\t10\nD\t<s>\t{ds}\t{db}\n");
        if with_pred_words {
            table += "W\t年\t50\nW\t日\t40\nQ\t年\t50\nQ\t日\t40\nP\t<s>\t年\t50\nP\t<s>\t日\t40\n";
        }
        for k in 1..=9 { table += &format!("L\t{k}\t10\n"); }
        table += "L\t10+\t10\n";
        for c in "0123456789./-".chars() { table += &format!("C\t{c}\t10\n"); }
        Arc::new(DigitTable::parse(&table).unwrap())
    }

    fn setup_digit_engine(lex_data: &str, right: bool, favor_digits: bool, with_pred_words: bool, options: u32) -> Engine {
        let lex = Arc::new(Lexicon::parse(lex_data).unwrap());
        let lm = digit_test_lm();
        let capped = Arc::new(CappedLexicon::new(lex.clone(), "", &lm, None).unwrap());
        let mut e = Engine::with_lexicon(lex, Layout::Standard);
        e.set_lm(lm, capped);
        e.digit_table = Some(digit_test_table(favor_digits, with_pred_words));
        e.set_windows_input_options(options);
        e.set_candidate_anchor(right);
        e.set_prediction(with_pred_words).unwrap();
        e.set_upstream_english(true, true, true, true).unwrap();
        e
    }

    #[test]
    fn digit_keys_closed_as_han_keep_original_digits_in_the_han_window() {
        for right in [false, true] {
            for mixed in [false, true] {
                let mut e = setup_digit_engine("ㄚˋ 阿 -0.1\n", right, false, false, 0);
                e.set_mixed_input(mixed);
                for ch in "84".chars() { e.key(Key::ch(ch, 0)).unwrap(); }
                assert_eq!(e.syls, vec!["ㄚˋ"], "must exercise Han, not digit tokens");
                assert_eq!(e.view(true, String::new()).preedit, "阿");
                let opened = e.key(Key::new(KeyKind::Down)).unwrap();
                let index = opened.candidates.iter().position(|w| w == "84").unwrap();
                assert_eq!(e.pick(index).unwrap().unwrap().preedit, "84");
            }
        }
    }

    #[test]
    fn digit_candidates_offer_zhuyin_and_tones_from_both_anchors() {
        let lex = "ㄅ ㄅ -0.1\nㄅ 八 -1.5\nㄅㄚˋ 爸 -0.5\nㄉ ㄉ -0.1\nㄓ ㄓ -0.1\nㄚ ㄚ -0.1\nㄞ ㄞ -0.1\nㄢ ㄢ -0.1\nㄦ ㄦ -0.1\nㄡ ㄡ -0.1\nㄥ ㄥ -0.1\n";
        for right in [false, true] {
            let mut e = setup_digit_engine(lex, right, false, false, 1 << 23);
            for (digit, fw, sym) in [
                ('1', "１", "ㄅ"), ('2', "２", "ㄉ"), ('5', "５", "ㄓ"),
                ('8', "８", "ㄚ"), ('9', "９", "ㄞ"), ('0', "０", "ㄢ"),
            ] {
                e.reset(ResetMode::Discard);
                assert_eq!(e.key(Key::ch(digit, 0)).unwrap().preedit, digit.to_string());
                let out = e.key(Key::new(KeyKind::Down)).unwrap();
                assert_eq!(out.candidates, vec![digit.to_string(), fw.into(), sym.into()], "right={right} digit={digit}");
                let out = e.pick(2).unwrap().unwrap();
                assert_eq!(out.preedit, sym);
                assert!(!e.is_punct(0));
                assert_eq!(e.syls, vec![sym.to_string()]);
                if digit == '1' {
                    let list = e.key(Key::new(KeyKind::Down)).unwrap().candidates;
                    let ba = list.iter().position(|w| w == "八").unwrap();
                    assert_eq!(e.pick(ba).unwrap().unwrap().preedit, "八");
                }
            }
            for (tone_key, fw, mark) in [('3', "３", "ˇ"), ('4', "４", "ˋ"), ('6', "６", "ˊ"), ('7', "７", "˙")] {
                e.reset(ResetMode::Discard);
                e.key(Key::ch('1', 0)).unwrap();
                e.key(Key::ch(tone_key, 0)).unwrap();
                if right { e.key(Key::new(KeyKind::Left)).unwrap(); }
                let out = e.key(Key::new(KeyKind::Down)).unwrap();
                assert_eq!(out.candidates, vec![tone_key.to_string(), fw.into(), mark.into()], "right={right} tone={tone_key}");
                let out = e.pick(2).unwrap().unwrap();
                assert_eq!(out.preedit, format!("1{mark}"));
                assert!(e.is_punct(1));
                assert_eq!(e.key(Key::new(KeyKind::Enter)).unwrap().commit, format!("1{mark}"));
            }
            let mut e_tokens = setup_digit_engine(lex, right, true, false, 1 << 23);
            for (key_ch, fw, sym, extras) in [('-', "－", "ㄦ", "→←…⋯"), ('.', "．", "ㄡ", ""), ('/', "／", "ㄥ", "↗↙")] {
                e_tokens.reset(ResetMode::Discard);
                for ch in ['1', key_ch, '1'] { e_tokens.key(Key::ch(ch, 0)).unwrap(); }
                for _ in 0..(if right { 2 } else { 1 }) { e_tokens.key(Key::new(KeyKind::Left)).unwrap(); }
                let out = e_tokens.key(Key::new(KeyKind::Down)).unwrap();
                let mut expected = vec![key_ch.to_string(), fw.into(), sym.into()];
                expected.extend(extras.chars().map(|ch| ch.to_string()));
                assert_eq!(out.candidates, expected, "right={right} sym={key_ch}");
                let out = e_tokens.pick(2).unwrap().unwrap();
                assert_eq!(out.preedit, format!("1{sym}1"));
                assert!(!e_tokens.is_punct(1));
                assert_eq!(e_tokens.syls[1], sym);
            }

            e_tokens.reset(ResetMode::Discard);
            for ch in "1-1".chars() { e_tokens.key(Key::ch(ch, 0)).unwrap(); }
            assert_eq!(e_tokens.view(true, String::new()).preedit, "1-1");
            for _ in 0..(if right { 2 } else { 1 }) { e_tokens.key(Key::new(KeyKind::Left)).unwrap(); }
            let out = e_tokens.key(Key::new(KeyKind::Down)).unwrap();
            let index = out.candidates.iter().position(|word| word == "←").unwrap();
            assert_eq!(e_tokens.pick(index).unwrap().unwrap().preedit, "1←1");
        }
    }

    #[test]
    fn digit_syllable_alt_in_right_anchor_replaces_exact_span_without_corrupting_neighbors() {
        for right in [false, true] {
            let mut e = setup_digit_engine("ㄅㄚˋ 爸 -10.0\n", right, true, false, 1 << 23);
            for ch in "21849".chars() { e.key(Key::ch(ch, 0)).unwrap(); }
            assert_eq!(e.view(true, String::new()).preedit, "21849");
            for _ in 0..(if right { 2 } else { 1 }) { e.key(Key::new(KeyKind::Left)).unwrap(); }
            let out = e.key(Key::new(KeyKind::Down)).unwrap();
            assert_eq!(out.candidates, vec!["4", "４", "ˋ", "爸"]);
            assert_eq!(e.pick(2).unwrap().unwrap().preedit, "218ˋ9");

            e.reset(ResetMode::Discard);
            for ch in "21849".chars() { e.key(Key::ch(ch, 0)).unwrap(); }
            for _ in 0..(if right { 2 } else { 1 }) { e.key(Key::new(KeyKind::Left)).unwrap(); }
            let out = e.key(Key::new(KeyKind::Down)).unwrap();
            assert_eq!(out.candidates, vec!["4", "４", "ˋ", "爸"]);
            let out = e.pick(3).unwrap().unwrap();
            assert_eq!(out.preedit, "2爸9");
            assert_eq!(out.cursor_utf16, 2);
        }
    }

    #[test]
    fn digit_candidates_preserve_upstream_and_eten_behavior() {
        let lex = "ㄅ ㄅ -0.1\n";
        let l = Arc::new(Lexicon::parse(lex).unwrap());
        let lm = digit_test_lm();
        let capped = Arc::new(CappedLexicon::new(l.clone(), "", &lm, None).unwrap());
        let mut upstream = Engine::with_lexicon(l.clone(), Layout::Standard);
        upstream.set_lm(lm.clone(), capped.clone());
        upstream.digit_table = Some(digit_test_table(true, false));
        upstream.set_upstream_english(true, true, true, true).unwrap();
        for ch in "21".chars() { upstream.key(Key::ch(ch, 0)).unwrap(); }
        assert_eq!(upstream.key(Key::new(KeyKind::Down)).unwrap().candidates, vec!["1", "１"]);
        upstream.reset(ResetMode::Discard);
        upstream.set_candidate_anchor(true);
        upstream.insert_windows_literal("1").unwrap();
        assert_eq!(upstream.key(Key::new(KeyKind::Down)).unwrap().candidates, vec!["1"]);

        let mut eten = Engine::with_lexicon(l, Layout::Eten);
        eten.set_lm(lm, capped);
        eten.set_windows_input_options(1 << 23);
        assert!(eten.digit_zhuyin_symbol('1').is_none());
        eten.set_candidate_anchor(true);
        assert!(eten.set_punctuation("1\t①\n"));
        eten.insert_windows_literal("1").unwrap();
        assert_eq!(eten.key(Key::new(KeyKind::Down)).unwrap().candidates, vec!["1", "①"]);
    }

    #[test]
    fn digit_zhuyin_handles_prediction_forget_and_missing_lexicon_safely() {
        let mut missing = setup_digit_engine("ㄋㄧㄢˊ 年 -0.1\nㄖˋ 日 -0.1\n", true, false, false, 1 << 23);
        missing.key(Key::ch('1', 0)).unwrap();
        assert_eq!(missing.key(Key::new(KeyKind::Down)).unwrap().candidates, vec!["1", "１", "ㄅ"]);
        let out = missing.pick(2).unwrap().unwrap();
        assert_eq!(out.preedit, "ㄅ");
        assert!(missing.is_punct(0));

        for right in [false, true] {
            let mut e = setup_digit_engine("ㄋㄧㄢˊ 年 -0.1\nㄖˋ 日 -0.1\n", right, false, true, 1 << 23);
            e.key(Key::ch('1', 0)).unwrap();
            e.key(Key::new(KeyKind::Down)).unwrap();
            let before_rows = e.cands.as_ref().unwrap().rows.len();
            assert!(before_rows >= 1, "right={right}: digit-context predictions must remain selectable");
            assert_eq!(&e.cands.as_ref().unwrap().list[before_rows..], &[
                ("1".into(), 1), ("１".into(), 1), ("ㄅ".into(), 1)
            ]);
            e.key(Key { kind: KeyKind::Backspace, ch: '\0', modifiers: MOD_COMMAND }).unwrap();
            let after_rows = e.cands.as_ref().unwrap().rows.len();
            assert_eq!(after_rows, before_rows - 1);
            assert_eq!(&e.cands.as_ref().unwrap().list[after_rows..], &[
                ("1".into(), 1), ("１".into(), 1), ("ㄅ".into(), 1)
            ]);
            e.cands.as_mut().unwrap().sel = after_rows + 2;
            e.key(Key { kind: KeyKind::Backspace, ch: '\0', modifiers: MOD_COMMAND }).unwrap();
            let zhuyin_idx = e.cands.as_ref().unwrap().list.len() - 1;
            e.cands.as_mut().unwrap().sel = zhuyin_idx;
            let out = e.key(Key::new(KeyKind::Enter)).unwrap();
            assert_eq!(out.preedit, "ㄅ");

            let mut selected = setup_digit_engine("ㄋㄧㄢˊ 年 -0.1\nㄖˋ 日 -0.1\n", right, false, true, 1 << 23);
            selected.key(Key::ch('1', 0)).unwrap();
            selected.key(Key::new(KeyKind::Down)).unwrap();
            let word = selected.cands.as_ref().unwrap().rows[0].word.clone();
            assert_eq!(selected.pick(0).unwrap().unwrap().preedit, format!("1{word}"));
        }
    }

    #[test]
    fn digit_cycle_and_selection_learning_work_with_zhuyin_option() {
        let mut e = setup_digit_engine("ㄅ ㄅ -0.1\nㄅㄚ 巴 -0.5\n", true, true, false, 1 << 23);
        e.set_learning(true);
        e.key(Key::ch('1', 0)).unwrap();
        e.key(Key::ch('2', 0)).unwrap();
        e.key(Key::new(KeyKind::Home)).unwrap();
        let out = e.cycle_windows_candidate().unwrap();
        assert_eq!((out.preedit.as_str(), out.cursor_utf16), ("１2", 0));
        let out = e.cycle_windows_candidate().unwrap();
        assert_eq!((out.preedit.as_str(), out.cursor_utf16), ("ㄅ2", 0));
        assert_eq!(e.key(Key::new(KeyKind::Enter)).unwrap().commit, "ㄅ2");
        assert!(e.learner.records().is_empty());
    }
}
