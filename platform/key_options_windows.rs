// Key options only; LM arithmetic remains upstream.
impl Engine {
    pub fn set_windows_input_options(&mut self, flags: u32) {
        self.shift_punctuation = flags & 1 != 0;
        self.plain_punctuation = flags & 2 != 0;
        self.standalone_tones = flags & 4 != 0;
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
