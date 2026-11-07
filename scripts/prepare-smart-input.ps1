#Requires -Version 7
param([Parameter(Mandatory)][string]$CorePath)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$file = Join-Path $CorePath 'src/engine.rs'
$content = (Get-Content $file -Raw).Replace("`r`n","`n")
function Replace-SmartContract([string]$old,[string]$new) {
    if ([regex]::Matches($script:content,[regex]::Escape($old)).Count -ne 1) { throw "Upstream smart-input contract changed: $old" }
    $script:content = $script:content.Replace($old,$new)
}
Replace-SmartContract '    abbr: bool,' "    abbr: bool,`n    mixed_input: bool,`n    mixed_raw: String,`n    mixed_latin: bool,`n    mixed_explicit_english: bool,`n    mixed_shifted_end: usize,`n    run_shifted: Vec<bool>,`n    mixed_display_char: Option<char>,"
Replace-SmartContract '            abbr: false,' "            abbr: false,`n            mixed_input: false,`n            mixed_raw: String::new(),`n            mixed_latin: false,`n            mixed_explicit_english: false,`n            mixed_shifted_end: 0,`n            run_shifted: Vec::new(),`n            mixed_display_char: None,"
Replace-SmartContract '    pub fn key(&mut self, k: Key) -> Result<Output, EngineError> {' '    fn key_original(&mut self, k: Key) -> Result<Output, EngineError> {'
Replace-SmartContract '            self.shadow_key[col] = Some(Self::shadow_entry(&self.shadow_key, ch));' '            self.shadow_key[col] = Some(Self::shadow_entry(&self.shadow_key, self.mixed_display_char.filter(|display| display.eq_ignore_ascii_case(&ch)).unwrap_or(ch)));'
Replace-SmartContract 'return self.revert_keys(Revert::Overwrite, k.ch, col, sym);' 'return self.revert_keys(Revert::Overwrite, self.mixed_display_char.unwrap_or(k.ch), col, sym);'
Replace-SmartContract 'return self.revert_keys(Revert::Order, k.ch, col, sym);' 'return self.revert_keys(Revert::Order, self.mixed_display_char.unwrap_or(k.ch), col, sym);'
Replace-SmartContract "        self.invalidate();`n        self.left.clear();" "        self.invalidate();`n        self.mixed_raw.clear();`n        self.mixed_explicit_english = false;`n        self.mixed_shifted_end = 0;`n        self.run_shifted.clear();`n        self.mixed_latin = false;`n        self.left.clear();"
Replace-SmartContract "        } else {`n            self.pending()`n        };" "        } else if self.mixed_input && !self.mixed_raw.is_empty() {`n            self.mixed_raw.clone()`n        } else {`n            self.pending()`n        };"
Replace-SmartContract '        text.push_str(&unfinished);' '        text.push_str(&unfinished); text.push_str(&self.pending_literal);'
Replace-SmartContract '        Output { handled, commit, preedit, cursor_utf16, candidates, selected, columns, first, total, vertical, english_run_utf16_start, english_run_utf16_len }' '        self.mixed_view(Output { handled, commit, preedit, cursor_utf16, candidates, selected, columns, first, total, vertical, english_run_utf16_start, english_run_utf16_len })'
Replace-SmartContract '    pub fn pick(&mut self, index: usize) -> Result<Option<Output>, EngineError> {' "    pub fn pick(&mut self, index: usize) -> Result<Option<Output>, EngineError> {`n        if self.is_mixed_hint() { return Ok(None); }"
# A lone digit key shows as the digit; its keys stay buffered, so later keys can still form Zhuyin.
Replace-SmartContract '        text.push_str(&unfinished); text.push_str(&self.pending_literal);' '        match self.lone_digit() { Some(digit) => text.push(digit), None => text.push_str(&unfinished) } text.push_str(&self.pending_literal);'
$content += "`n" + (Get-Content "$root/platform/smart_input_windows.rs" -Raw)
$content += "`n" + (Get-Content "$root/platform/composition_windows.rs" -Raw)
Replace-SmartContract '    mixed_input: bool,' "    mixed_input: bool,`n    mixed_tolerance: bool,`n    enter_to_cursor: bool,`n    punctuation_keeps_reading: bool,`n    lone_digit_literal: bool,`n    tones_over_digits: bool,`n    space_inputs_zhuyin: bool,`n    windows_raw: String,`n    pending_literal: String,`n    typed_literals: HashMap<usize,String>,"
Replace-SmartContract '            mixed_input: false,' "            mixed_input: false,`n            mixed_tolerance: false,`n            enter_to_cursor: false,`n            punctuation_keeps_reading: false,`n            lone_digit_literal: false,`n            tones_over_digits: false,`n            space_inputs_zhuyin: true,`n            windows_raw: String::new(),`n            pending_literal: String::new(),`n            typed_literals: HashMap::new(),"
Replace-SmartContract "        self.mixed_latin = false;`n        self.left.clear();" "        self.mixed_latin = false;`n        self.windows_raw.clear();`n        self.pending_literal.clear();`n        self.typed_literals.clear();`n        self.left.clear();"
Replace-SmartContract '        self.syls.insert(c, reading);' "        self.typed_literals = std::mem::take(&mut self.typed_literals).into_iter().map(|(i,text)| (if i >= c { i+1 } else { i },text)).collect();`n        self.syls.insert(c, reading);"
Replace-SmartContract '        self.syls.remove(i);' "        self.typed_literals = std::mem::take(&mut self.typed_literals).into_iter().filter(|(index,_)| *index != i).map(|(index,text)| (if index > i { index-1 } else { index },text)).collect();`n        self.syls.remove(i);"
Replace-SmartContract '        self.syls.splice(start..end, reading);' "        self.typed_literals = std::mem::take(&mut self.typed_literals).into_iter().filter(|(i,_)| *i < start || *i >= end).map(|(i,text)| (if i >= end { i - (end-start) + m } else { i },text)).collect();`n        self.syls.splice(start..end, reading);"
Replace-SmartContract '    fn choose(&mut self, idx: usize) -> Result<(), EngineError> {' "    fn choose(&mut self, idx: usize) -> Result<(), EngineError> {`n        if self.choose_windows_literal(idx)? { return Ok(()); }"
Replace-SmartContract '    fn forget_highlighted(&mut self) -> Result<(), EngineError> {' "    fn forget_highlighted(&mut self) -> Result<(), EngineError> {`n        if self.cands.as_ref().is_some_and(|c| c.literals.contains_key(&c.sel)) { return Ok(()); }"
Replace-SmartContract '    list: Vec<(String, usize)>,' "    list: Vec<(String, usize)>,`n    literals: HashMap<usize,WindowsLiteralCandidate>,"
Replace-SmartContract 'Cands { list, sel: 0, expanded: false, top: 0, vertical, first: 0, short: None, alt: HashMap::new(), zh: None, rows: Vec::new(), dalt: None }' 'Cands { list, literals: HashMap::new(), sel: 0, expanded: false, top: 0, vertical, first: 0, short: None, alt: HashMap::new(), zh: None, rows: Vec::new(), dalt: None }'
Set-Content $file $content -Encoding utf8NoBOM
$digitsFile = Join-Path $CorePath 'src/digits.rs'
$digitsContent = (Get-Content $digitsFile -Raw).Replace("`r`n","`n")
function Replace-DigitsContract([string]$old,[string]$new) {
    if ([regex]::Matches($script:digitsContent,[regex]::Escape($old)).Count -ne 1) { throw "Upstream digits smart-input contract changed: $old" }
    $script:digitsContent = $script:digitsContent.Replace($old,$new)
}
$mixedLegal = @'
        let s: String = keys.iter().map(|&k| symbol_of_key(k)).collect::<Option<String>>()?;
        self.prefixes.contains(&s).then(|| units_of(&s))
    }

    pub fn tail_units_mixed(&self, keys: &[char]) -> Option<Vec<Unit>> {
        fn ordered_cols(keys: &[char]) -> Option<[Option<char>; 3]> {
            let mut cols = [None; 3];
            for &k in keys {
                let (col, sym) = Layout::Standard.symbol_of(k)?;
                if cols[col].is_some() {
                    return None;
                }
                cols[col] = Some(sym);
            }
            Some(cols)
        }
        let last = *keys.last()?;
        if last == ' ' || tone_mark_of_key(last).is_some() {
            let body = &keys[..keys.len() - 1];
            if body.is_empty() {
                return None;
            }
            let cols = ordered_cols(body)?;
            let mut s: String = cols.into_iter().flatten().collect();
            let mark = tone_mark_of_key(last);
            let mut full = s.clone();
            full.extend(mark);
            if !self.han.contains(&full) {
                return None;
            }
            s.push(mark.unwrap_or(' '));
            return Some(units_of(&s));
        }
        let cols = ordered_cols(keys)?;
        let s: String = cols.into_iter().flatten().collect();
        if self.prefixes.contains(&s) {
            return Some(units_of(&s));
        }
        if let [Some(c0), None, Some(c2)] = cols {
            for m in ['ㄧ', 'ㄨ', 'ㄩ'] {
                let bridged = format!("{c0}{m}{c2}");
                if self.prefixes.contains(&bridged) {
                    return Some(units_of(&bridged));
                }
            }
        }
        None
    }
'@
Replace-DigitsContract "        let s: String = keys.iter().map(|&k| symbol_of_key(k)).collect::<Option<String>>()?;`n        self.prefixes.contains(&s).then(|| units_of(&s))`n    }" $mixedLegal.Replace("`r`n","`n").TrimEnd()
Replace-DigitsContract '    pub fn decide(&self, c: &str, emitted: &[char], buf: &[char], extra: Option<char>, prev_d: Option<usize>, closing: bool, complete: bool) -> Decision {' "    pub fn decide(&self, c: &str, emitted: &[char], buf: &[char], extra: Option<char>, prev_d: Option<usize>, closing: bool, complete: bool) -> Decision {`n        self.decide_mixed(c, emitted, buf, extra, prev_d, closing, complete, false)`n    }`n`n    #[allow(clippy::too_many_arguments)]`n    pub fn decide_mixed(&self, c: &str, emitted: &[char], buf: &[char], extra: Option<char>, prev_d: Option<usize>, closing: bool, complete: bool, mixed_order: bool) -> Decision {"
Replace-DigitsContract '                if let Some(units) = self.legal.tail_units(&tail) {' '                if let Some(units) = if mixed_order { self.legal.tail_units_mixed(&tail) } else { self.legal.tail_units(&tail) } {'
Set-Content $digitsFile $digitsContent -Encoding utf8NoBOM

$digitLayerFile = Join-Path $CorePath 'src/engine/digit_layer.rs'
$digitLayerContent = (Get-Content $digitLayerFile -Raw).Replace("`r`n","`n")
function Replace-DigitLayerContract([string]$old,[string]$new,[int]$expected=1) {
    if ([regex]::Matches($script:digitLayerContent,[regex]::Escape($old)).Count -ne $expected) { throw "Upstream digit_layer contract changed: $old" }
    $script:digitLayerContent = $script:digitLayerContent.Replace($old,$new)
}
Replace-DigitLayerContract "    pub profile: Profile,`n}" "    pub profile: Profile,`n    pub mixed_order: bool,`n}"
Replace-DigitLayerContract "    pub fn scorer(&self) -> Scorer<'_> {`n        Scorer { table: &self.table, lm: &self.st.lm, idx: self.st.capped.predict_index(&self.st.lm), legal: &self.cache.legal, cfg: self.st.lm.scoring(self.profile) }`n    }" "    pub fn scorer(&self) -> Scorer<'_> {`n        Scorer { table: &self.table, lm: &self.st.lm, idx: self.st.capped.predict_index(&self.st.lm), legal: &self.cache.legal, cfg: self.st.lm.scoring(self.profile) }`n    }`n    #[allow(clippy::too_many_arguments)]`n    pub fn decide(&self, c: &str, emitted: &[char], buf: &[char], extra: Option<char>, prev_d: Option<usize>, closing: bool, complete: bool) -> Decision {`n        self.scorer().decide_mixed(c, emitted, buf, extra, prev_d, closing, complete, self.mixed_order)`n    }"
Replace-DigitLayerContract 'return Some(DigitEnv { st, table, cache: c, profile: self.profile });' 'return Some(DigitEnv { st, table, cache: c, profile: self.profile, mixed_order: self.mixed_input });'
Replace-DigitLayerContract 'Some(DigitEnv { st, table, cache, profile: self.profile })' 'Some(DigitEnv { st, table, cache, profile: self.profile, mixed_order: self.mixed_input })'
Replace-DigitLayerContract 'env.scorer().decide(' 'env.decide(' 5
Replace-DigitLayerContract 'let Some(mut syl) = body.iter().map(|&k| Layout::Standard.symbol_of(k).map(|x| x.1)).collect::<Option<String>>() else { continue };' 'let Some(mut syl) = (if self.mixed_input { self.mixed_reading_order(&body.iter().collect::<String>(), false) } else { body.iter().map(|&k| Layout::Standard.symbol_of(k).map(|x| x.1)).collect::<Option<String>>() }) else { continue };'
Set-Content $digitLayerFile $digitLayerContent -Encoding utf8NoBOM

& "$PSScriptRoot/prepare-english.ps1" | Out-Null
Copy-Item "$root/build/english-words.txt" "$CorePath/src/english-words.txt" -Force
Add-Content "$CorePath/src/ffi.rs" ("`n" + (Get-Content "$root/platform/smart_input_windows_ffi.rs" -Raw))
