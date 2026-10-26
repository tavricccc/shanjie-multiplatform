#Requires -Version 7
param([Parameter(Mandatory)][string]$CorePath)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$engine = Join-Path $CorePath 'src/engine.rs'
$content = Get-Content $engine -Raw
foreach ($contract in @('pub fn new_with_personal(', 'pub fn load_lm(', 'pub fn load_tinylm(')) {
    if ([regex]::Matches($content,[regex]::Escape($contract)).Count -ne 1) { throw "Resident contract changed: $contract" }
}
$content = $content.Replace('pub fn new_with_personal(', 'fn new_with_personal_uncached(').Replace('pub fn load_lm(', 'fn load_lm_uncached(').Replace('pub fn load_tinylm(', 'fn load_tinylm_uncached(')
$content += "`n" + (Get-Content "$root/platform/resident_windows.rs" -Raw)
Set-Content $engine $content -Encoding utf8NoBOM
Add-Content (Join-Path $CorePath 'src/ffi.rs') @'

#[no_mangle]
pub extern "C" fn shanjie_windows_resource_loads(model: u32) -> u64 {
    crate::engine::windows_resource_loads(model != 0)
}
'@
Add-Content $engine @'

pub fn windows_resource_loads(model: bool) -> u64 {
    if model { WINDOWS_MODEL_LOADS.load(Ordering::Relaxed) } else { WINDOWS_LEX_LOADS.load(Ordering::Relaxed) }
}
'@
