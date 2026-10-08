#Requires -Version 7
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$metadata = & cargo metadata --manifest-path "$root/windows/Cargo.toml" --format-version 1 --filter-platform x86_64-pc-windows-msvc --locked | ConvertFrom-Json
if ($LASTEXITCODE) { throw 'Rust dependency metadata failed.' }
$output = Join-Path $root 'dist/LICENSES/windows-rust'
New-Item -ItemType Directory -Path $output -Force | Out-Null
$notices = @('# Windows Rust 相依套件', '', '由 windows/Cargo.lock 的鎖定版本產生；授權全文在各套件子目錄。', '')
foreach ($dependency in $metadata.packages | Where-Object source | Sort-Object name) {
    $source = Split-Path $dependency.manifest_path -Parent
    $destination = Join-Path $output "$($dependency.name)-$($dependency.version)"
    New-Item -ItemType Directory -Path $destination -Force | Out-Null
    $licenses = @(Get-ChildItem -LiteralPath $source -File | Where-Object Name -Match '^(licen[cs]e|copying|copyright)')
    if ($dependency.license_file) { $licenses += Get-Item -LiteralPath (Join-Path $source $dependency.license_file) }
    if (!$licenses.Count) { throw "Missing license text: $($dependency.name)" }
    foreach ($license in $licenses | Sort-Object FullName -Unique) { Copy-Item -LiteralPath $license.FullName -Destination $destination -Force }
    $notices += "- $($dependency.name) $($dependency.version) — $($dependency.license) — $($dependency.repository)"
}
Set-Content (Join-Path $output 'README.md') $notices -Encoding utf8NoBOM
