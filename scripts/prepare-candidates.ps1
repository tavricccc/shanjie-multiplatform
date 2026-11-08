#Requires -Version 7
param([Parameter(Mandatory)][string]$CorePath)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$engine = Join-Path $CorePath 'src/engine.rs'
$content = Get-Content $engine -Raw
function Replace-Contract([string]$old, [string]$replacement, [int]$count = 1) {
    if ([regex]::Matches($script:content,[regex]::Escape($old)).Count -ne $count) {
        throw "Upstream candidate anchor contract changed: $old"
    }
    $script:content = $script:content.Replace($old,$replacement)
}
Replace-Contract '    cursor: usize,' "    cursor: usize,`n    candidate_from_right: bool,`n    windows_punctuation_candidates: bool,`n    candidate_grid_columns: usize,"
Replace-Contract '            cursor: 0,' "            cursor: 0,`n            candidate_from_right: false,`n            windows_punctuation_candidates: false,`n            candidate_grid_columns: PAGE_SIZE,"
Replace-Contract 'let (len, sel, cols) = (c.list.len(), c.sel, PAGE_SIZE);' 'let (len, sel, cols) = (c.list.len(), c.sel, self.candidate_grid_columns);'
Replace-Contract 'let columns = if c.expanded { PAGE_SIZE as u32 } else { 0 };' 'let columns = if c.expanded { self.candidate_grid_columns as u32 } else { 0 };'
# The viewport and number labels retain nine-item groups, independently of the visual row width.
Replace-Contract 'let idx = row * cols + d;' 'let idx = sel / PAGE_SIZE * PAGE_SIZE + d;'
Replace-Contract '    fn open_candidates(&mut self, rows: Vec<Pred>) {' '    fn open_candidates_left(&mut self, rows: Vec<Pred>) {'
Replace-Contract 'let alts = typed.chars().next().and_then(|c| self.punct.get(&c)).cloned().unwrap_or_default();' 'let alts = self.windows_punctuation_alternatives(&typed);'
Replace-Contract '        self.punct = map;' "        self.punct = map;`n        self.windows_punctuation_candidates = false;"
Replace-Contract 'let (start, end) = if self.cursor == 0 { (0, l) } else { (self.cursor - l, self.cursor) };' 'let (start, end) = self.candidate_span(l);' 2
Replace-Contract '        self.fixed.push(Fixed { start, end, word, pre, key, pick_key, zh, frozen: false, split_off: false, displaced_new });' "        self.fixed.push(Fixed { start, end, word, pre, key, pick_key, zh, frozen: false, split_off: false, displaced_new });`n        if self.candidate_from_right { self.cursor = end; }"
Replace-Contract '        capped.predict_index(&lm);' '        if self.predict_on { capped.predict_index(&lm); }'
$content += "`n" + (Get-Content "$root/platform/candidates_windows.rs" -Raw)
$content += "`n" + (Get-Content "$root/platform/input_windows.rs" -Raw)
Set-Content $engine $content -Encoding utf8NoBOM
Add-Content (Join-Path $CorePath 'src/ffi.rs') ("`n" + (Get-Content "$root/platform/candidates_windows_ffi.rs" -Raw))
Add-Content (Join-Path $CorePath 'src/ffi.rs') ("`n" + (Get-Content "$root/platform/input_windows_ffi.rs" -Raw))
