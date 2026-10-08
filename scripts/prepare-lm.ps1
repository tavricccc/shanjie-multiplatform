#Requires -Version 7
param([Parameter(Mandatory)][string]$CorePath)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
Copy-Item "$root/platform/mapped_array.rs" "$CorePath/src/mapped_array.rs" -Force
Add-Content "$CorePath/src/lib.rs" "`nmod mapped_array;`n"
$lm = Join-Path $CorePath 'src/lm.rs'
$content = Get-Content $lm -Raw
foreach ($field in @('pool','voff','uni','ctx_id','ctx_total','ctx_back','off','nxt','cnt','cls','emit','pc')) {
    $pattern = "(?m)^    ${field}: Vec<([^>]+)>,"
    if ([regex]::Matches($content,$pattern).Count -ne 1) { throw "LM field contract changed: $field" }
    $content = [regex]::Replace($content,$pattern,"    ${field}: crate::mapped_array::Array<`$1>,")
    if ($field -in @('cls','emit','pc')) { continue }
    $pattern = "(?m)^            $field,"
    if ([regex]::Matches($content,$pattern).Count -ne 1) { throw "LM parser contract changed: $field" }
    $content = [regex]::Replace($content,$pattern,"            ${field}: $field.into(),")
}
$old = 'lm.classes = Some(Classes { k3, mu, cls, emit, pc });'
if (!$content.Contains($old)) { throw 'Word-class parser contract changed.' }
$content = $content.Replace($old,'lm.classes = Some(Classes { k3, mu, cls: cls.into(), emit: emit.into(), pc: pc.into() });')
$signature = 'pub fn load(path: &Path) -> Result<Lm, LmError> {'
if (!$content.Contains($signature)) { throw 'LM load contract changed.' }
$content = $content.Replace($signature,$signature + "`n        if path.extension().is_some_and(|ext| ext == `"winmap`") { return Self::load_windows_map(path); }")
$content += "`n" + (Get-Content "$root/platform/mapped_lm.rs" -Raw)
Set-Content $lm $content -Encoding utf8NoBOM
Copy-Item "$root/platform/map_model.rs" "$CorePath/src/map_model.rs" -Force
Add-Content "$CorePath/Cargo.toml" "`n[[bin]]`nname = `"map-model`"`npath = `"src/map_model.rs`"`n"
