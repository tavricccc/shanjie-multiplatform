// Windows selection learns are provisional until the composition is confirmed.
#[derive(Clone, Default)]
struct WindowsLearningPrefix {
    left: String,
    syls: Vec<String>,
    fixed: Vec<Fixed>,
    path: Vec<PathWord>,
    display: String,
}

impl WindowsLearningPrefix {
    fn append(&mut self, engine: &Engine, display: &str) {
        let offset = self.syls.len();
        self.fixed.extend(engine.fixed.iter().cloned().map(|mut f| {
            f.start += offset;
            f.end += offset;
            f
        }));
        self.syls.extend_from_slice(&engine.syls);
        self.path.extend_from_slice(&engine.path);
        self.display.push_str(display);
    }
}

impl Engine {
    fn restore_windows_selection(&mut self) -> bool {
        self.learner.windows_rollback_preview()
    }

    fn preview_windows_selection(&mut self) {
        if !self.windows_selection_learning || !self.learning || self.syls.is_empty() { return; }
        self.restore_windows_selection();
        self.learner.windows_begin_preview();
        self.learn_windows_composition(&self.display.clone());
    }

    fn learn_windows_composition(&mut self, display: &str) {
        let Some(mut whole) = self.windows_learning_prefix.clone() else {
            self.learn_commit_original(display);
            return;
        };
        whole.append(self, display);
        std::mem::swap(&mut self.left, &mut whole.left);
        std::mem::swap(&mut self.syls, &mut whole.syls);
        std::mem::swap(&mut self.fixed, &mut whole.fixed);
        std::mem::swap(&mut self.path, &mut whole.path);
        std::mem::swap(&mut self.display, &mut whole.display);
        self.learn_commit_original(&self.display.clone());
        std::mem::swap(&mut self.left, &mut whole.left);
        std::mem::swap(&mut self.syls, &mut whole.syls);
        std::mem::swap(&mut self.fixed, &mut whole.fixed);
        std::mem::swap(&mut self.path, &mut whole.path);
        std::mem::swap(&mut self.display, &mut whole.display);
    }

    fn learn_commit(&mut self, display: &str) {
        if self.windows_partial_commit { return; }
        self.restore_windows_selection();
        self.learn_windows_composition(display);
    }

    fn choose(&mut self, idx: usize) -> Result<(), EngineError> {
        if self.restore_windows_selection() { self.refresh()?; }
        self.choose_without_selection_learning(idx)?;
        self.preview_windows_selection();
        Ok(())
    }

    fn apply_pred(&mut self, p: Pred) -> Result<Output, EngineError> {
        if self.restore_windows_selection() { self.refresh()?; }
        let out = self.apply_pred_without_selection_learning(p)?;
        self.preview_windows_selection();
        Ok(out)
    }
}

#[cfg(test)]
mod windows_selection_learning_tests {
    use super::*;

    fn engine() -> Engine {
        // Learning uses the LM path, which the no-model decoder does not construct.
        static MODEL: std::sync::OnceLock<(Arc<Lexicon>, Arc<Lm>, Arc<CappedLexicon>)> = std::sync::OnceLock::new();
        let (lex, lm, capped) = MODEL.get_or_init(|| {
            let lex = Arc::new(Lexicon::parse("ㄈㄥ 風 -0.1\nㄈㄥ 楓 -2\nㄓ 之 -0.1\nㄍㄨˇ 谷 -0.1\nㄈㄥ-ㄓ-ㄍㄨˇ 風之谷 -7.041\n").unwrap());
            let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
            let lm = Arc::new(Lm::load(&root.join("data/lm/bigram.sjlm")).unwrap());
            let capped = Arc::new(CappedLexicon::new(lex.clone(), "", &lm, None).unwrap());
            (lex, lm, capped)
        });
        let mut e = Engine::with_lexicon(lex.clone(), Layout::Standard);
        e.set_lm(lm.clone(), capped.clone());
        e.set_windows_input_options(1 << 21);
        e.set_candidate_anchor(true);
        e.set_prediction(false).unwrap();
        e.set_learning(true);
        e.set_today(Some(20_000));
        e
    }

    fn type_keys(e: &mut Engine, keys: &str) {
        for ch in keys.chars() { e.key(if ch == ' ' { Key::new(KeyKind::Space) } else { Key::ch(ch, 0) }).unwrap(); }
    }

    fn open_maple(e: &mut Engine) -> usize {
        e.key(Key::new(KeyKind::Home)).unwrap();
        e.key(Key::new(KeyKind::Down)).unwrap();
        e.cands.as_ref().unwrap().list.iter().position(|(w, _)| w == "楓").unwrap()
    }

    fn weight(e: &Engine) -> Option<f64> {
        e.learner.new_word_weight(&["ㄈㄥ".into(), "ㄓ".into(), "ㄍㄨˇ".into()], "楓之谷", 20_000)
    }

    #[test]
    fn number_enter_and_mouse_picks_learn_once_across_partial_enters() {
        for source in 0..3 {
            let mut e = engine();
            type_keys(&mut e, "z/ 5 ej3");
            let index = open_maple(&mut e);
            match source {
                0 => { e.key(Key::ch(char::from(b'1' + index as u8), 0)).unwrap(); }
                1 => {
                    for _ in 0..index { e.key(Key::new(KeyKind::Right)).unwrap(); }
                    e.key(Key::new(KeyKind::Enter)).unwrap();
                }
                _ => { e.pick(index).unwrap().unwrap(); }
            }
            assert_eq!(weight(&e), Some(1.0));
            let out = e.key(Key::new(KeyKind::Enter)).unwrap();
            assert_eq!((out.commit.as_str(), out.preedit.as_str()), ("楓", "之谷"));
            assert_eq!(weight(&e), Some(1.0));
            e.key(Key::new(KeyKind::End)).unwrap();
            assert_eq!(e.key(Key::new(KeyKind::Enter)).unwrap().commit, "之谷");
            assert_eq!(weight(&e), Some(1.0));
        }
    }

    #[test]
    fn typing_after_a_single_character_pick_keeps_it_in_new_word_learning() {
        let mut e = engine();
        type_keys(&mut e, "z/ ");
        let index = open_maple(&mut e);
        e.pick(index).unwrap();
        type_keys(&mut e, "5 ej3");
        assert_eq!(e.key(Key::new(KeyKind::Enter)).unwrap().commit, "楓之谷");
        assert_eq!(weight(&e), Some(1.0));
    }

    #[test]
    fn repicking_deleting_cancelling_and_disabling_learning_undo_the_preview() {
        for action in 0..4 {
            let mut e = engine();
            type_keys(&mut e, "z/ 5 ej3");
            let index = open_maple(&mut e);
            e.pick(index).unwrap();
            assert_eq!(weight(&e), Some(1.0));
            match action {
                0 => {
                    e.key(Key::new(KeyKind::Home)).unwrap();
                    e.key(Key::new(KeyKind::Down)).unwrap();
                    let index = e.cands.as_ref().unwrap().list.iter().position(|(w, n)| w == "風" && *n == 1).unwrap();
                    e.pick(index).unwrap();
                    e.key(Key::new(KeyKind::End)).unwrap();
                    e.key(Key::new(KeyKind::Enter)).unwrap();
                }
                1 => { e.key(Key::new(KeyKind::Backspace)).unwrap(); }
                2 => { e.key(Key::new(KeyKind::Enter)).unwrap(); e.reset(ResetMode::Discard); }
                _ => { e.set_learning(false); e.reset(ResetMode::Commit); }
            }
            assert_eq!(weight(&e), None, "action={action}");
        }
    }

    #[test]
    fn repeated_partial_commits_bound_the_learning_prefix_and_finalize_old_choices() {
        let mut e = engine();
        e.left = "這是".into();
        type_keys(&mut e, "z/ 5 ej3");
        let index = open_maple(&mut e);
        e.pick(index).unwrap();
        for _ in 0..MAX_SYLLABLES * 2 {
            e.key(Key::new(KeyKind::Home)).unwrap();
            e.key(Key::new(KeyKind::Right)).unwrap();
            e.key(Key::new(KeyKind::Enter)).unwrap();
            assert!(e.windows_learning_prefix.as_ref().map_or(0, |p| p.syls.len()) <= MAX_SYLLABLES);
            type_keys(&mut e, "5 ");
        }
        assert!(!e.learner.records().is_empty(), "already submitted choices must be finalized at the bound");
        e.reset(ResetMode::Discard);
        assert!(e.windows_learning_prefix.is_none());
        assert!(!e.learner.windows_preview_active());
    }

    #[test]
    fn only_confirmed_compositions_persist_selection_learning() {
        let dir = std::env::temp_dir().join(format!("shanjie-selection-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut e = engine();
        e.learning_open(&dir).unwrap();
        type_keys(&mut e, "z/ 5 ej3");
        let index = open_maple(&mut e);
        e.pick(index).unwrap();
        let mut other = engine();
        other.learning_open(&dir).unwrap();
        assert_eq!(weight(&other), None);
        assert_eq!(e.reset(ResetMode::Commit).commit, "楓之谷");
        other.learning_open(&dir).unwrap();
        assert_eq!(weight(&other), Some(1.0));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
