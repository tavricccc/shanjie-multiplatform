#Requires -Version 7
param([Parameter(Mandatory)][string]$CorePath)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$engine = Join-Path $CorePath 'src/engine.rs'
$content = (Get-Content $engine -Raw).Replace("`r`n","`n")
function Replace-KeyContract([string]$old,[string]$replacement) {
    if ([regex]::Matches($script:content,[regex]::Escape($old)).Count -ne 1) { throw "Upstream key contract changed: $old" }
    $script:content = $script:content.Replace($old,$replacement)
}
Replace-KeyContract '    candidate_big5: bool,' "    candidate_big5: bool,`n    shift_punctuation: u32,`n    plain_punctuation: u32,`n    standalone_tones: bool,"
Replace-KeyContract '            candidate_big5: false,' "            candidate_big5: false,`n            shift_punctuation: 0x1fff,`n            plain_punctuation: 0x3f,`n            standalone_tones: true,"
Replace-KeyContract '} else if is_char && m == MOD_SHIFT {' '} else if is_char && m == MOD_SHIFT && self.windows_shift_punct(k.ch) {'
Replace-KeyContract '} else if is_char && m == 0 && self.layout.symbol_of(k.ch).is_none()' '} else if is_char && m == 0 && self.windows_plain_punct(k.ch) && self.layout.symbol_of(k.ch).is_none()'
Replace-KeyContract 'if let (KeyKind::Char, Some(t)) = (k.kind, tone) {' 'if let (KeyKind::Char, Some(t), true) = (k.kind, tone, self.standalone_tones) {'
$old = @'
            } else {
                self.pred = old; // the key does nothing
'@
$new = @'
            } else if is_char && ((m == MOD_SHIFT && !self.windows_shift_punct(k.ch)
                    && (self.shift_punctuation == 0 || SHIFT_PUNCT.iter().any(|(ch,_)| *ch == k.ch)))
                || (plain && !self.windows_plain_punct(k.ch) && PLAIN_PUNCT.iter().any(|(ch,_)| *ch == k.ch))) {
                let commit = self.take_commit();
                return self.passthrough(commit);
            } else {
                self.pred = old; // the key does nothing
'@
Replace-KeyContract $old $new
$content += "`n" + (Get-Content "$root/platform/key_options_windows.rs" -Raw)
Set-Content $engine $content -Encoding utf8NoBOM
Add-Content "$CorePath/src/ffi.rs" ("`n" + (Get-Content "$root/platform/key_options_windows_ffi.rs" -Raw))
