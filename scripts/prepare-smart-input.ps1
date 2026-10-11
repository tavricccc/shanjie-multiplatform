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
Replace-SmartContract '    abbr: bool,' "    abbr: bool,`n    smart_zhuyin: bool,`n    mixed_input: bool,`n    mixed_raw: String,`n    mixed_latin: bool,"
Replace-SmartContract '            abbr: false,' "            abbr: false,`n            smart_zhuyin: false,`n            mixed_input: false,`n            mixed_raw: String::new(),`n            mixed_latin: false,"
Replace-SmartContract '    pub fn key(&mut self, k: Key) -> Result<Output, EngineError> {' '    fn key_original(&mut self, k: Key) -> Result<Output, EngineError> {'
Replace-SmartContract '        self.left.clear();' "        self.mixed_raw.clear();`n        self.mixed_latin = false;`n        self.left.clear();"
Replace-SmartContract '        text.push_str(&self.pending());' '        text.push_str(&if self.mixed_latin { self.mixed_raw.clone() } else { self.pending() });'
Replace-SmartContract 'if self.pend[col].is_some() && self.abbr_active() {' 'if (self.pend[col].is_some() || (self.smart_zhuyin && self.pend[col + 1..].iter().any(Option::is_some))) && self.abbr_active() {'
$content += "`n" + (Get-Content "$root/platform/smart_input_windows.rs" -Raw)
Set-Content $file $content -Encoding utf8NoBOM
Copy-Item "$root/data/english-common.txt" "$CorePath/src/english-common.txt" -Force
Add-Content "$CorePath/src/ffi.rs" ("`n" + (Get-Content "$root/platform/smart_input_windows_ffi.rs" -Raw))
