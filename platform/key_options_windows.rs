// Key options only; LM arithmetic remains upstream.
impl Engine {
    /// bit 0: standalone tones; bits 1-13: Shift; bits 14-19: plain; bit 20: disable space symbols;
    /// bit 21: Enter commits only up to the cursor; bit 22: punctuation keeps an unfinished reading;
    /// bit 23: a lone digit shows as the digit; bit 24: standalone tones win over upstream digit runs.
    /// bit 25: output Zhuyin symbols without conversion; bit 26: McBopomofo punctuation layout.
    pub fn set_windows_input_options(&mut self, flags: u32) {
        self.windows_shifted_boundaries = true;
        self.windows_punctuation_candidates = true;
        self.windows_digit_zhuyin = true;
        self.candidate_grid_columns = 3;
        self.windows_selection_learning = true;
        self.standalone_tones = flags & 1 != 0;
        self.shift_punctuation = (flags >> 1) & 0x1fff;
        self.plain_punctuation = (flags >> 14) & 0x3f;
        self.space_inputs_zhuyin = flags & (1 << 20) == 0;
        self.enter_to_cursor = flags & (1 << 21) != 0;
        self.punctuation_keeps_reading = flags & (1 << 22) != 0;
        self.lone_digit_literal = flags & (1 << 23) != 0;
        self.tones_over_digits = flags & (1 << 24) != 0;
        self.zhuyin_only = flags & (1 << 25) != 0;
        self.mcbopomofo_punctuation = flags & (1 << 26) != 0;
    }
    pub(crate) fn set_windows_half_punct(&mut self, mask: u32) {
        self.half_punct_mask = mask;
    }

    fn windows_half_punct(&self, key: Key) -> Option<char> {
        if self.half_punct_mask == 0 || self.ascii || key.kind != KeyKind::Char {
            return None;
        }
        match key.modifiers {
            MOD_SHIFT => half_punctuation::selected(self.half_punct_mask, key.ch, true),
            0 if self.layout.symbol_of(key.ch).is_none() && self.layout.tone_of(key.ch).is_none() => {
                half_punctuation::selected(self.half_punct_mask, key.ch, false)
            }
            _ => None,
        }
    }

    fn windows_shift_punct(&self, ch: char) -> Option<char> {
        if self.mcbopomofo_punctuation && ('2'..='8').contains(&ch) {
            return "＠＃＄％︿＆＊".chars().nth(ch as usize - '2' as usize);
        }
        SHIFT_PUNCT.iter().enumerate().find_map(|(i, &(key, mark))| {
            (key == ch && self.shift_punctuation & (1 << i) != 0)
                .then_some(if self.mcbopomofo_punctuation && ch == '\'' { '；' } else { mark })
        })
    }
    fn windows_plain_punct(&self, ch: char) -> Option<char> {
        PLAIN_PUNCT.iter().enumerate().find_map(|(i, &(key, mark))| {
            (key == ch && self.plain_punctuation & (1 << i) != 0).then_some(
                match (self.mcbopomofo_punctuation, ch) {
                    (true, '\'') => '、',
                    (true, '\\') => '＼',
                    _ => mark,
                },
            )
        })
    }
    pub fn insert_windows_punctuation(&mut self, mark: &str) -> Result<Output,EngineError> {
        self.insert_windows_literal(mark)
    }
}
