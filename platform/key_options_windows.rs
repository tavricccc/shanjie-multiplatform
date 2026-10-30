// Key options only; LM arithmetic remains upstream.
impl Engine {
    /// bit 0: standalone tones; bits 1-13: Shift; bits 14-19: plain; bit 20: disable space symbols;
    /// bit 21: Enter commits only up to the cursor; bit 22: punctuation keeps an unfinished reading.
    pub fn set_windows_input_options(&mut self, flags: u32) {
        self.standalone_tones = flags & 1 != 0;
        self.shift_punctuation = (flags >> 1) & 0x1fff;
        self.plain_punctuation = (flags >> 14) & 0x3f;
        self.space_inputs_zhuyin = flags & (1 << 20) == 0;
        self.enter_to_cursor = flags & (1 << 21) != 0;
        self.punctuation_keeps_reading = flags & (1 << 22) != 0;
    }
    fn windows_shift_punct(&self, ch: char) -> bool {
        SHIFT_PUNCT.iter().position(|(c, _)| *c == ch).is_some_and(|i| self.shift_punctuation & (1 << i) != 0)
    }
    fn windows_plain_punct(&self, ch: char) -> bool {
        PLAIN_PUNCT.iter().position(|(c, _)| *c == ch).is_some_and(|i| self.plain_punctuation & (1 << i) != 0)
    }
    pub fn insert_windows_punctuation(&mut self, mark: &str) -> Result<Output,EngineError> {
        self.insert_windows_literal(mark)
    }
}
