// Key options only; LM arithmetic remains upstream.
impl Engine {
    /// bit 0: standalone tones; bits 1-13: SHIFT_PUNCT entries; bits 14-19: PLAIN_PUNCT entries.
    pub fn set_windows_input_options(&mut self, flags: u32) {
        self.standalone_tones = flags & 1 != 0;
        self.shift_punctuation = (flags >> 1) & 0x1fff;
        self.plain_punctuation = (flags >> 14) & 0x3f;
    }
    fn windows_shift_punct(&self, ch: char) -> bool {
        SHIFT_PUNCT.iter().position(|(c, _)| *c == ch).is_some_and(|i| self.shift_punctuation & (1 << i) != 0)
    }
    fn windows_plain_punct(&self, ch: char) -> bool {
        PLAIN_PUNCT.iter().position(|(c, _)| *c == ch).is_some_and(|i| self.plain_punctuation & (1 << i) != 0)
    }
    pub fn insert_windows_punctuation(&mut self, mark: &str) -> Result<Output,EngineError> {
        let committed = if !self.mixed_raw.is_empty() {
            let raw = std::mem::take(&mut self.mixed_raw);
            self.mixed_latin = false;
            self.mixed_text(&raw)?.commit
        } else { String::new() };
        self.clear_pend(); self.cands = None; self.clear_pred();
        let mut out = self.insert_token(format!("{PUNCT_PREFIX}{mark}"),Some(mark.to_owned()))?;
        out.commit = committed + &out.commit;
        Ok(out)
    }
}
