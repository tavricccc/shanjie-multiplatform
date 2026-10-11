// Windows composition policy. Chinese candidates/scoring remain upstream.
fn english_word(word: &str) -> bool {
    static WORDS: std::sync::OnceLock<Vec<&'static str>> = std::sync::OnceLock::new();
    WORDS.get_or_init(|| include_str!("english-common.txt").lines().collect()).binary_search(&word).is_ok()
}

#[cfg(test)]
mod windows_smart_tests {
    use super::*;
    fn engine() -> Engine {
        let lex = Lexicon::parse("ㄋㄧˇ 你 -0.1\nㄏㄠˇ 好 -0.1\nㄧˇ 以 -0.2\nㄋㄧˇ-ㄏㄠˇ 你好 -0.1\n").unwrap();
        let mut e = Engine::with_lexicon(Arc::new(lex), Layout::Standard);
        e.set_smart_input(true, false);
        e
    }
    fn type_keys(e: &mut Engine, keys: &str) -> Output {
        let mut out = e.view(true, String::new());
        for ch in keys.chars() { out = e.key(if ch == ' ' { Key::new(KeyKind::Space) } else { Key::ch(ch, 0) }).unwrap(); }
        out
    }
    #[test]
    fn mixed_boundaries_and_lifecycle() {
        let mut e = engine();
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
        assert_eq!(e.key(Key::new(KeyKind::Enter)).unwrap().commit, "hello world");
        e.set_smart_input(false, false);
        assert!(!type_keys(&mut e, "hello").preedit.contains("hello"));
    }
}

impl Engine {
    pub fn set_smart_input(&mut self, mixed: bool, zhuyin: bool) {
        self.mixed_input = mixed;
        self.smart_zhuyin = zhuyin;
        self.abbr = zhuyin;
    }

    // A normal syllable must follow initial -> medial -> final without replacement.
    // This prevents an English word from winning over an explicitly typed Chinese syllable.
    fn mixed_reading(&self, raw: &str) -> Option<String> {
        let mut last = None;
        let mut reading = String::new();
        for ch in raw.chars() {
            let (column, symbol) = self.layout.symbol_of(ch)?;
            if last.is_some_and(|previous| previous >= column) { return None; }
            last = Some(column);
            reading.push(symbol);
        }
        (!reading.is_empty()).then_some(reading)
    }

    fn mixed_text(&mut self, text: &str) -> Result<Output, EngineError> {
        self.clear_pend();
        self.clear_pred();
        self.cands = None;
        self.insert_token(format!("{PUNCT_PREFIX}{text}"), Some(text.to_owned()))
    }

    fn finish_latin(&mut self, key: Key) -> Result<Output, EngineError> {
        let raw = std::mem::take(&mut self.mixed_raw);
        self.mixed_latin = false;
        // At a tone boundary prefer the longest English prefix that leaves a real Chinese syllable.
        let tone = if key.kind == KeyKind::Space { Some(0) }
            else if key.kind == KeyKind::Char && key.modifiers == 0 { self.layout.tone_of(key.ch) }
            else { None };
        if let Some(tone) = tone {
            for split in (3..raw.len().min(33)).rev() {
                let (word, tail) = raw.split_at(split);
                if !english_word(&word.to_ascii_lowercase()) { continue; }
                if let Some(mut reading) = self.mixed_reading(tail) {
                    reading.push_str(TONE_MARKS[tone]);
                    if self.lex.entries(&[reading]).is_empty() { continue; }
                    let prefix = self.mixed_text(word)?.commit;
                    for ch in tail.chars() { self.key_original(Key::ch(ch, 0))?; }
                    let mut out = self.key_original(key)?;
                    out.commit = prefix + &out.commit;
                    return Ok(out);
                }
            }
        }
        let mut out = self.mixed_text(&raw)?;
        if key.kind == KeyKind::Space {
            let prefix = out.commit;
            out = self.mixed_text(" ")?;
            out.commit = prefix + &out.commit;
        } else {
            let prefix = out.commit;
            out = self.key_original(key)?;
            out.commit = prefix + &out.commit;
        }
        Ok(out)
    }

    pub fn key(&mut self, key: Key) -> Result<Output, EngineError> {
        if self.mixed_latin {
            if key.modifiers & !MOD_SHIFT != 0 { return self.passthrough(String::new()); }
            if key.kind == KeyKind::Char && key.ch.is_ascii_alphabetic() {
                self.mixed_raw.push(if key.modifiers == MOD_SHIFT { key.ch.to_ascii_uppercase() } else { key.ch });
                return self.handled();
            }
            match key.kind {
                KeyKind::Backspace => {
                    self.mixed_raw.pop();
                    if self.mixed_raw.is_empty() { self.mixed_latin = false; }
                    return self.handled();
                }
                KeyKind::Esc => {
                    self.mixed_raw.clear(); self.mixed_latin = false;
                    return self.handled();
                }
                KeyKind::Down | KeyKind::Tab => {
                    let raw = std::mem::take(&mut self.mixed_raw);
                    self.mixed_latin = false;
                    for ch in raw.chars() { self.key_original(Key::ch(ch.to_ascii_lowercase(), 0))?; }
                    return self.smart_key(Key::new(KeyKind::Down));
                }
                _ => return self.finish_latin(key),
            }
        }
        if self.mixed_input && key.kind == KeyKind::Char && key.modifiers == MOD_SHIFT
            && key.ch.is_ascii_alphabetic() && self.pending().is_empty() && self.cands.is_none() {
            self.mixed_raw = key.ch.to_ascii_uppercase().to_string();
            self.mixed_latin = true;
            self.clear_pred();
            return self.handled();
        }
        let tracking = self.mixed_input && self.cands.is_none() && self.pred_sel.is_none()
            && key.kind == KeyKind::Char && key.modifiers == 0 && key.ch.is_ascii_alphabetic();
        if tracking {
            if self.pend.iter().all(Option::is_none) { self.mixed_raw.clear(); }
            if self.mixed_raw.len() < 64 { self.mixed_raw.push(key.ch); }
            if english_word(&self.mixed_raw) && self.mixed_reading(&self.mixed_raw).is_none() {
                self.clear_pend(); self.clear_pred();
                self.mixed_latin = true;
                return self.handled();
            }
        } else if key.kind == KeyKind::Backspace {
            self.mixed_raw.pop();
        } else { self.mixed_raw.clear(); }
        self.smart_key(key)
    }

    fn smart_key(&mut self, key: Key) -> Result<Output, EngineError> {
        if self.smart_zhuyin && key.modifiers == 0 && !self.pending().is_empty()
            && self.cands.is_none() && !self.pred.is_empty() {
            match key.kind {
                KeyKind::Space | KeyKind::Enter => {
                    let prefix = self.choose_pred(self.pred_sel.unwrap_or(0))?.commit;
                    self.mixed_raw.clear();
                    let mut out = if key.kind == KeyKind::Enter { self.key_original(key)? } else { self.handled()? };
                    out.commit = prefix + &out.commit;
                    return Ok(out);
                }
                KeyKind::Down if self.pred_sel.is_none() => {
                    self.pred_sel = Some(0);
                    return self.handled();
                }
                _ => {}
            }
        }
        self.key_original(key)
    }
}
