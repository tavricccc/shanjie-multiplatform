#Requires -Version 7
param([Parameter(Mandatory)][ValidateSet('test','release')][string]$Profile)

$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$rust = (& rustc --version --verbose | Out-String).Trim()
if ($LASTEXITCODE) { throw 'Cannot identify rustc for the Cargo cache.' }
$cargo = (& cargo --version | Out-String).Trim()
if ($LASTEXITCODE) { throw 'Cannot identify Cargo for the cache.' }

. "$PSScriptRoot/msvc-command.ps1"
$probe = Join-Path $env:RUNNER_TEMP 'shanjie-msvc-cache-key.cmd'
$setup = Get-MsvcSetup x64
Set-Content -LiteralPath $probe -Encoding utf8NoBOM -Value @"
@echo off
$setup
if errorlevel 1 exit /b 1
set VCToolsVersion
if errorlevel 1 exit /b 1
set WindowsSDKVersion
"@
$native = & cmd.exe /d /c $probe
if ($LASTEXITCODE) { throw 'Cannot identify the MSVC tools and Windows SDK for the Cargo cache.' }

# Test inherits dev. Include the effective profile settings without tying the
# compatible restore prefix to package version changes in the manifest.
$profiles = & python -c 'import json, sys, tomllib; p = tomllib.load(open(sys.argv[1], "rb")).get("profile", {}); print(json.dumps({k: p.get(k, {}) for k in (("dev", "test") if sys.argv[2] == "test" else ("release",))}, sort_keys=True))' "$root/windows/Cargo.toml" $Profile
if ($LASTEXITCODE) { throw 'Cannot identify the Cargo compilation profile.' }
$overrides = Get-ChildItem Env: |
    Where-Object Name -Match '^CARGO_(PROFILE_(DEV|TEST|RELEASE)_|BUILD_RUSTFLAGS$|TARGET_.*_(RUSTFLAGS|LINKER)$)' |
    Sort-Object Name | ForEach-Object { "$($_.Name)=$($_.Value)" }
$parts = [ordered]@{
    toolchain = @($rust, $cargo, $env:RUSTFLAGS, $env:CARGO_ENCODED_RUSTFLAGS)
    native = @($native)
    config = @($profiles, (Get-FileHash -LiteralPath "$root/.cargo/config.toml").Hash) + @($overrides)
}
$digests = @{}
foreach ($part in $parts.GetEnumerator()) {
    $bytes = [Text.Encoding]::UTF8.GetBytes($part.Value -join "`n")
    $digests[$part.Key] = [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($bytes)).ToLowerInvariant()
}
"toolchain=$($digests.toolchain)" >> $env:GITHUB_OUTPUT
"native=$($digests.native)" >> $env:GITHUB_OUTPUT
$prefix = "cargo-$Profile-v2-$env:RUNNER_OS-$($digests.toolchain)-$($digests.native)-$($digests.config)"
"prefix=$prefix" >> $env:GITHUB_OUTPUT
if ($Profile -eq 'release') {
    $x64Config = (Get-FileHash -LiteralPath "$root/.cargo/windows-release-x64.toml").Hash.ToLowerInvariant()
    "x64_prefix=$prefix-$x64Config" >> $env:GITHUB_OUTPUT
}
"inputs=$env:SHANJIE_CARGO_CACHE_INPUTS" >> $env:GITHUB_OUTPUT
Write-Host $rust
Write-Host $cargo
$native | ForEach-Object { Write-Host $_ }
