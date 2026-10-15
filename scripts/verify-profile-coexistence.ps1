#Requires -Version 7
param(
    [Parameter(Mandatory)][string]$OldPackage,
    [ValidateSet(5,6)][int]$OldSchema = 5,
    [ValidateSet('all','x64','x86')][string]$Architecture = 'all',
    [string]$Evidence
)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
if (!$Evidence) { $Evidence = Join-Path $root ('build/profile-coexistence-' + [guid]::NewGuid().ToString('N')) }
$Evidence = [IO.Path]::GetFullPath($Evidence)
if (!$Evidence.StartsWith([IO.Path]::GetFullPath($root) + [IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) { throw 'Evidence must stay in this workspace.' }
if (Test-Path -LiteralPath $Evidence) { throw 'Use a new evidence directory.' }
New-Item -ItemType Directory -Path $Evidence | Out-Null
$vs = & "${env:ProgramFiles(x86)}/Microsoft Visual Studio/Installer/vswhere.exe" -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (!$vs) { throw 'Visual Studio C++ tools required.' }
$architectures = if ($Architecture -eq 'all') { @('x64','x86') } else { @($Architecture) }
foreach ($arch in $architectures) {
    $output = Join-Path $Evidence $arch
    New-Item -ItemType Directory -Path $output | Out-Null
    $oldDll = [IO.Path]::GetFullPath((Join-Path $OldPackage "$arch/Shanjie.dll"))
    $newDll = Join-Path $root "dist/$arch/Shanjie.dll"
    if (!(Test-Path -LiteralPath $oldDll) -or !(Test-Path -LiteralPath $newDll)) { throw 'Both DLL releases are required.' }
    $vcvars = Join-Path $vs 'VC/Auxiliary/Build/vcvarsall.bat'
    $command = @"
@echo off
call "$vcvars" $arch >nul
if errorlevel 1 exit /b 1
cl /nologo /std:c++20 /EHsc /O2 /MT /W4 /utf-8 "$root/tests/profile_coexistence.cpp" /Fo"$output/probe.obj" /Fe"$output/probe.exe"
"@
    Set-Content -LiteralPath "$output/build.cmd" -Value $command -Encoding utf8NoBOM
    & cmd.exe /d /c "$output/build.cmd"
    if ($LASTEXITCODE) { throw 'Coexistence probe compile failed.' }
    & "$output/probe.exe" $oldDll $newDll "$output/profile" $OldSchema
    if ($LASTEXITCODE) { throw 'Coexistence probe failed.' }
    [pscustomobject]@{architecture=$arch;old_schema=$OldSchema;old_sha256=(Get-FileHash -LiteralPath $oldDll).Hash;new_sha256=(Get-FileHash -LiteralPath $newDll).Hash;status='PASS';read_pairs=64;broadcasts_disabled=$true} | ConvertTo-Json | Set-Content -LiteralPath "$output/result.json" -Encoding utf8NoBOM
}
