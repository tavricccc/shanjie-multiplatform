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
Replace-SmartContract '    abbr: bool,' "    abbr: bool,`n    mixed_input: bool,`n    mixed_raw: String,`n    mixed_latin: bool,`n    mixed_display_char: Option<char>,"
Replace-SmartContract '            abbr: false,' "            abbr: false,`n            mixed_input: false,`n            mixed_raw: String::new(),`n            mixed_latin: false,`n            mixed_display_char: None,"
Replace-SmartContract '    pub fn key(&mut self, k: Key) -> Result<Output, EngineError> {' '    fn key_original(&mut self, k: Key) -> Result<Output, EngineError> {'
Replace-SmartContract '        self.left.clear();' "        self.mixed_raw.clear();`n        self.mixed_latin = false;`n        self.left.clear();"
Replace-SmartContract '        text.push_str(&self.pending());' '        text.push_str(&if self.mixed_input && !self.mixed_raw.is_empty() { self.mixed_raw.clone() } else { self.pending() });'
Replace-SmartContract '        Output { handled, commit, preedit, cursor_utf16, candidates, selected, columns, first, total, vertical }' '        self.mixed_view(Output { handled, commit, preedit, cursor_utf16, candidates, selected, columns, first, total, vertical })'
Replace-SmartContract '    pub fn pick(&mut self, index: usize) -> Result<Option<Output>, EngineError> {' "    pub fn pick(&mut self, index: usize) -> Result<Option<Output>, EngineError> {`n        if self.is_mixed_hint() { return Ok(None); }"
$content += "`n" + (Get-Content "$root/platform/smart_input_windows.rs" -Raw)
Set-Content $file $content -Encoding utf8NoBOM
Copy-Item "$root/data/english-common.txt" "$CorePath/src/english-common.txt" -Force
Add-Content "$CorePath/src/ffi.rs" ("`n" + (Get-Content "$root/platform/smart_input_windows_ffi.rs" -Raw))
