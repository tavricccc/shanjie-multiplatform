// Key options only; LM arithmetic remains upstream.
impl Engine {
    pub fn set_windows_input_options(&mut self, flags: u32) {
        self.shift_punctuation = flags & 1 != 0;
        self.plain_punctuation = flags & 2 != 0;
        self.standalone_tones = flags & 4 != 0;
    }
    pub fn insert_windows_punctuation(&mut self, mark: &str) -> Result<Output,EngineError> {
        self.pend = [None;3]; self.cands = None; self.clear_pred();
        self.insert_token(format!("{PUNCT_PREFIX}{mark}"),Some(mark.to_owned()))
    }
}
