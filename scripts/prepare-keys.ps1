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
Replace-KeyContract '    candidate_from_right: bool,' "    candidate_from_right: bool,`n    shift_punctuation: u32,`n    plain_punctuation: u32,`n    mcbopomofo_punctuation: bool,`n    half_punct_mask: u32,`n    standalone_tones: bool,`n    windows_digit_zhuyin: bool,"
Replace-KeyContract '            candidate_from_right: false,' "            candidate_from_right: false,`n            shift_punctuation: 0x1fff,`n            plain_punctuation: 0x3f,`n            mcbopomofo_punctuation: false,`n            half_punct_mask: 0,`n            standalone_tones: true,`n            windows_digit_zhuyin: false,"
Replace-KeyContract 'SHIFT_PUNCT.iter().find(|(c, _)| *c == k.ch).map(|(_, p)| *p)' 'self.windows_shift_punct(k.ch)'
Replace-KeyContract 'PLAIN_PUNCT.iter().find(|(c, _)| *c == k.ch).map(|(_, p)| *p)' 'self.windows_plain_punct(k.ch)'
Replace-KeyContract 'if let (KeyKind::Char, Some(t)) = (k.kind, tone) {' 'if let (KeyKind::Char, Some(t), true) = (k.kind, tone, self.standalone_tones) {'
$old = @'
            } else {
                self.nothing(old); // the key does nothing
'@
$new = @'
            } else if is_char && ((m == MOD_SHIFT && self.windows_shift_punct(k.ch).is_none()
                    && (self.shift_punctuation == 0 || SHIFT_PUNCT.iter().any(|(ch,_)| *ch == k.ch)))
                || (plain && self.windows_plain_punct(k.ch).is_none() && PLAIN_PUNCT.iter().any(|(ch,_)| *ch == k.ch))) {
                let commit = self.take_commit();
                return self.passthrough(commit);
            } else {
                self.nothing(old); // the key does nothing
'@
Replace-KeyContract $old $new
$halfBefore = @'
        // Key scheme 1, K8-K10 and K24a
'@
$halfAfter = @'
        if let Some(mark) = self.windows_half_punct(k) {
            return self.insert_windows_punctuation(&mark.to_string());
        }
        // Key scheme 1, K8-K10 and K24a
'@
Replace-KeyContract $halfBefore $halfAfter
$content += "`n" + (Get-Content "$root/platform/key_options_windows.rs" -Raw)
$content += "`npub(crate) mod half_punctuation {`n" + (Get-Content "$root/platform/half_punctuation.rs" -Raw) + "`n}`n"
Set-Content $engine $content -Encoding utf8NoBOM
Add-Content "$CorePath/src/ffi.rs" ("`n" + (Get-Content "$root/platform/key_options_windows_ffi.rs" -Raw))
