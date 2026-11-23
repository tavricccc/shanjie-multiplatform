#Requires -Version 7
param([Parameter(Mandatory)][string]$CorePath)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$engine = Join-Path $CorePath 'src/engine.rs'
$content = (Get-Content $engine -Raw).Replace("`r`n","`n")
function Replace-Contract([string]$old, [string]$replacement, [int]$count = 1) {
    $old = $old.Replace("`r`n","`n")
    $replacement = $replacement.Replace("`r`n","`n")
    if ([regex]::Matches($script:content,[regex]::Escape($old)).Count -ne $count) {
        throw "Upstream candidate anchor contract changed: $old"
    }
    $script:content = $script:content.Replace($old,$replacement)
}
Replace-Contract "    layout: Layout,`n    syls: Vec<String>,`n    cursor: usize," "    layout: Layout,`n    syls: Vec<String>,`n    cursor: usize,`n    candidate_from_right: bool,`n    windows_punctuation_candidates: bool,`n    candidate_grid_columns: usize,"
Replace-Contract '            cursor: 0,' "            cursor: 0,`n            candidate_from_right: false,`n            windows_punctuation_candidates: false,`n            candidate_grid_columns: PAGE_SIZE,"
Replace-Contract '    dalt: Option<DigitAlt>,' "    dalt: Option<DigitAlt>,`n    digit_zhuyin: Option<(usize, char)>,"
Replace-Contract 'dalt: None }' 'dalt: None, digit_zhuyin: None }'
Replace-Contract 'let (len, sel, cols) = (c.list.len(), c.sel, PAGE_SIZE);' 'let (len, sel, cols) = (c.list.len(), c.sel, self.candidate_grid_columns);'
Replace-Contract 'let columns = if c.expanded { PAGE_SIZE as u32 } else { 0 };' 'let columns = if c.expanded { self.candidate_grid_columns as u32 } else { 0 };'
# The viewport and number labels retain nine-item groups, independently of the visual row width.
Replace-Contract 'let idx = row * cols + d;' 'let idx = sel / PAGE_SIZE * PAGE_SIZE + d;'
Replace-Contract '    fn open_candidates(&mut self, rows: Vec<Pred>) {' '    fn open_candidates_left(&mut self, rows: Vec<Pred>) {'
Replace-Contract 'fn variant_move(&self, list: &mut Vec<(String, usize)>, rows: &mut Vec<Pred>) {' 'fn variant_move(&self, list: &mut Vec<(String, usize)>, rows: &mut Vec<Pred>, right: bool) {'
Replace-Contract 'self.variant_move(&mut list, &mut rows);' 'self.variant_move(&mut list, &mut rows, false);' 2
Replace-Contract 'e.variant_move(&mut list, &mut rows);' 'e.variant_move(&mut list, &mut rows, false);'
Replace-Contract 'let k = if a == 0 { self.syls.get(..l) } else { self.syls.get(a.checked_sub(l)?..a) };' 'let k = if right { self.syls.get(a..a + l) } else if a == 0 { self.syls.get(..l) } else { self.syls.get(a.checked_sub(l)?..a) };'
Replace-Contract 'self.syls[rows[il].start..a]' 'self.syls[rows[il].start..if right { self.syls.len() } else { a }]'
$leftDigitOld = @'
            if let Some(d) = digit_of(&self.syls[touching]) {
                // docs/spec/english-mixed.md §7.4 (Down): the row of the digit context first, then the digit and its full-width form, then (docs/spec/english-mixed.md §7.7)
                // the syllable the last digit tokens could have been. The punctuation table is not asked.
                let mut list: Vec<(String, usize)> = rows.iter().map(|p| (p.word.clone(), p.reading.len())).collect();
                list.push((d.to_string(), 1));
                list.push((digit_layer::full_width(d).to_string(), 1));
                let mut c = Cands::new(Vec::new(), self.cand_vertical, self.key_scheme == 1);
                if let Some((n, syl, word)) = self.digit_syllable_alt(touching + 1) {
                    list.push((word, 1));
                    c.dalt = Some(DigitAlt::Syllable { n, syl });
                }
                c.list = list;
                c.rows = rows;
                self.cands = Some(c);
                return;
            }
'@
$leftDigitNew = @'
            if let Some(d) = digit_of(&self.syls[touching]) {
                self.open_digit_candidates(d, touching, self.digit_zhuyin_symbol(d), rows);
                return;
            }
'@
Replace-Contract $leftDigitOld $leftDigitNew
Replace-Contract '        if c.sel < c.rows.len() {' "        if c.sel < c.rows.len() {`n            // Prediction rows belong to the end of the composition, before the right anchor moved left.`n            let cursor = self.cursor;`n            if self.candidate_from_right { self.cursor = self.syls.len(); }"
Replace-Contract '_ if after_digit_token => c.list[c.rows.len()..].to_vec(),' '_ if after_digit_token || c.digit_zhuyin.is_some() => c.list[c.rows.len()..].to_vec(),'
Replace-Contract '            c.rows = std::mem::take(&mut self.pred);' "            self.cursor = cursor;`n            c.rows = std::mem::take(&mut self.pred);"
Replace-Contract '            (c.list, c.alt, c.rows, c.mixed) = self.window_list(std::mem::take(&mut c.rows));' @'
            if self.candidate_from_right && c.digit_zhuyin.is_none() && c.mixed.is_none() {
                (c.list, c.alt, c.rows) = self.window_list_right(std::mem::take(&mut c.rows));
            } else {
                (c.list, c.alt, c.rows, c.mixed) = self.window_list(std::mem::take(&mut c.rows));
            }
'@
$chooseOld = @'
        if let Some(alt) = c.dalt.take().filter(|_| idx + 1 == c.list.len() && idx >= c.rows.len()) {
            let word = c.list[idx].0.clone();
            let commit = self.choose_digit_alt(alt, word)?;
            if !commit.is_empty() {
                self.commit_out = Some(commit);
            }
            return Ok(());
        }
'@
$chooseNew = @'
        // The Zhuyin alternative follows the half-width and full-width forms.
        if let Some((target, sym)) = c.digit_zhuyin.take().filter(|_| idx == c.rows.len() + 2) {
            let commit = self.choose_digit_zhuyin(target, sym)?;
            if !commit.is_empty() {
                self.commit_out = Some(commit);
            }
            return Ok(());
        }
        if let Some(alt) = c.dalt.take().filter(|_| idx + 1 == c.list.len() && idx >= c.rows.len()) {
            let word = c.list[idx].0.clone();
            if self.candidate_from_right && c.mixed.is_none() && matches!(&alt, DigitAlt::Digits(_)) {
                self.cursor += 1; // upstream restores the syllable before its cursor
            }
            let commit = self.choose_digit_alt(alt, word)?;
            if !commit.is_empty() {
                self.commit_out = Some(commit);
            }
            return Ok(());
        }
        let row_span = c.rows.get(idx).filter(|_| self.candidate_from_right).map(|p| (p.start, self.syls.len()));
        if row_span.is_some() {
            self.cursor = self.syls.len();
        }
'@
Replace-Contract $chooseOld $chooseNew
Replace-Contract 'let alts = typed.chars().next().and_then(|c| self.punct.get(&c)).cloned().unwrap_or_default();' 'let alts = self.windows_punctuation_alternatives(&typed);'
Replace-Contract '        self.punct = map;' "        self.punct = map;`n        self.windows_punctuation_candidates = false;"
Replace-Contract 'let (start, end) = if self.cursor == 0 { (0, l) } else { (self.cursor - l, self.cursor) };' 'let (start, end) = if c.mixed.is_some() { if self.cursor == 0 { (0, l) } else { (self.cursor - l, self.cursor) } } else { self.candidate_span(l) };' 2
Replace-Contract @'
        let (start, end) = if c.mixed.is_some() { if self.cursor == 0 { (0, l) } else { (self.cursor - l, self.cursor) } } else { self.candidate_span(l) };
        // New words of the path as shown, under the readings of their records: read before the span is rewritten below (docs/spec/learning.md §8.5, bookkeeping).
'@ @'
        let (start, end) = row_span.unwrap_or_else(|| if c.mixed.is_some() { if self.cursor == 0 { (0, l) } else { (self.cursor - l, self.cursor) } } else { self.candidate_span(l) });
        // New words of the path as shown, under the readings of their records: read before the span is rewritten below (docs/spec/learning.md §8.5, bookkeeping).
'@
Replace-Contract '        self.fixed.push(Fixed { start, end, word, pre, key, pick_keys, zh, frozen: false, split_off: false, displaced_new });' "        self.fixed.push(Fixed { start, end, word, pre, key, pick_keys, zh, frozen: false, split_off: false, displaced_new });`n        if self.candidate_from_right { self.cursor = end; }"
Replace-Contract '        capped.predict_index(&lm);' '        if self.predict_on { capped.predict_index(&lm); }'
$content += "`n" + (Get-Content "$root/platform/candidates_windows.rs" -Raw)
$content += "`n" + (Get-Content "$root/platform/input_windows.rs" -Raw)
Set-Content $engine $content -Encoding utf8NoBOM
$digitLayerFile = Join-Path $CorePath 'src/engine/digit_layer.rs'
$digitLayerContent = (Get-Content $digitLayerFile -Raw).Replace("`r`n","`n")
function Replace-DigitLayerCandidate([string]$old, [string]$replacement) {
    $old = $old.Replace("`r`n","`n")
    $replacement = $replacement.Replace("`r`n","`n")
    if ([regex]::Matches($script:digitLayerContent,[regex]::Escape($old)).Count -ne 1) {
        throw "Upstream digit layer candidate contract changed: $old"
    }
    $script:digitLayerContent = $script:digitLayerContent.Replace($old,$replacement)
}
Replace-DigitLayerCandidate '    Syllable { n: usize, syl: String },' '    Syllable { start: usize, n: usize, syl: String },'
$digitAltOld = @'
            DigitAlt::Syllable { n, syl } => {
                for _ in 0..n {
                    self.cursor = self.cursor.saturating_sub(1);
                    self.remove_syllable(self.cursor)?;
                }
                // Like the Chinese item of an English token's list: the word is fixed, nothing is learned.
                Ok(self.insert_token(syl, Some(word), None)?.commit)
            }
'@
$digitAltNew = @'
            DigitAlt::Syllable { start, n, syl } => {
                for i in (start..start + n).rev() {
                    self.remove_syllable(i)?;
                }
                self.cursor = start;
                // Like the Chinese item of an English token's list: the word is fixed, nothing is learned.
                Ok(self.insert_token(syl, Some(word), None)?.commit)
            }
'@
Replace-DigitLayerCandidate $digitAltOld $digitAltNew
Set-Content $digitLayerFile $digitLayerContent -Encoding utf8NoBOM
Add-Content (Join-Path $CorePath 'src/ffi.rs') ("`n" + (Get-Content "$root/platform/candidates_windows_ffi.rs" -Raw))
Add-Content (Join-Path $CorePath 'src/ffi.rs') ("`n" + (Get-Content "$root/platform/input_windows_ffi.rs" -Raw))
