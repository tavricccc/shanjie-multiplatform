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
Replace-Contract '    cursor: usize,' "    cursor: usize,`n    candidate_from_right: bool,`n    candidate_big5: bool,"
Replace-Contract '            cursor: 0,' "            cursor: 0,`n            candidate_from_right: false,`n            candidate_big5: false,"
Replace-Contract 'self.cands = Some(Cands::new(list));' 'self.cands = self.make_candidates(list);' 2
Replace-Contract '    fn open_candidates(&mut self) {' '    fn open_candidates_left(&mut self) {'
Replace-Contract 'let (start, end) = if self.cursor == 0 { (0, l) } else { (self.cursor - l, self.cursor) };' 'let (start, end) = self.candidate_span(l);' 2
Replace-Contract '        self.fixed.push(Fixed { start, end, word, pre });' "        self.fixed.push(Fixed { start, end, word, pre });`n        if self.candidate_from_right { self.cursor = end; }"
$content += "`n" + (Get-Content "$root/platform/candidates_windows.rs" -Raw)
$content += "`n" + (Get-Content "$root/platform/input_windows.rs" -Raw)
Set-Content $engine $content -Encoding utf8NoBOM
Add-Content (Join-Path $CorePath 'src/ffi.rs') ("`n" + (Get-Content "$root/platform/candidates_windows_ffi.rs" -Raw))
Add-Content (Join-Path $CorePath 'src/ffi.rs') ("`n" + (Get-Content "$root/platform/input_windows_ffi.rs" -Raw))
