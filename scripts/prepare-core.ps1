#Requires -Version 7
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
Import-Module "$PSScriptRoot/test-support.psm1" -ErrorAction Stop
. "$PSScriptRoot/preserve-output.ps1"
$upstream = Join-Path $root 'upstream/shanjie'
$pin = Get-Content (Join-Path $root 'UPSTREAM.json') -Raw | ConvertFrom-Json
$actual = git -C $upstream rev-parse HEAD
if ($actual -ne $pin.commit) { throw 'Upstream pin changed; review UPSTREAM.json before building.' }
if (git -C $upstream status --porcelain) { throw 'Upstream checkout must be clean.' }
$output = Join-Path $root 'build/core'
New-Item -ItemType Directory -Path $output -Force | Out-Null
$null = Initialize-TestTimings -ProjectRoot $root -Name 'prepare-core'
$null = & "$PSScriptRoot/prepare-english.ps1"
$english = Join-Path $root 'build/english-words.txt'
$englishHash = (Get-FileHash -LiteralPath $english -Algorithm SHA256).Hash.ToLowerInvariant()
$preparationScripts = @(Get-ChildItem -LiteralPath $PSScriptRoot -Filter 'prepare-*.ps1' -File | ForEach-Object { [IO.Path]::GetRelativePath($root,$_.FullName) })
$cacheInputs = @('UPSTREAM.json','MODEL-SOURCES.json','platform') + $preparationScripts
$cacheValues = @($pin.commit,$englishHash)
$key = Get-ContentFingerprint -Root $root -Paths $cacheInputs -Values $cacheValues
$cacheManifest = Join-Path $output '.prepare-cache.json'
$cachePaths = @('src','Cargo.toml')
$cacheCheck = Start-TestStage -Name 'Validate prepared core cache' -Category 'core.cache'
try {
    $cacheHit = Test-ContentCache -Root $output -ManifestPath $cacheManifest -Key $key -Paths $cachePaths
    Complete-TestStage -Stage $cacheCheck -Details @{cache=if($cacheHit){'hit'}else{'miss'};upstream_commit=$pin.commit}
} catch {
    Complete-TestStage -Stage $cacheCheck -Status 'failed' -Details @{cache='miss';upstream_commit=$pin.commit} -ErrorMessage $_.Exception.Message
    throw
}
if ($cacheHit) {
    Write-Host "Prepared core cache hit: upstream $($pin.commit), source tree verified."
    return
}

$stage = Start-TestStage -Name 'Generate Windows core overlay' -Category 'core.prepare'
$previousParent=$env:SHANJIE_TEST_PARENT_STAGE_ID
$env:SHANJIE_TEST_PARENT_STAGE_ID=$stage.Id
try {
foreach ($relative in $cachePaths) {
    $old = Join-Path $output $relative
    if (Test-Path -LiteralPath $old) { Preserve-Output $old }
}
New-Item -ItemType Directory -Path $output -Force | Out-Null
Copy-Item (Join-Path $upstream 'core/src') $output -Recurse -Force
Copy-Item (Join-Path $upstream 'core/Cargo.toml') $output -Force
Add-Content (Join-Path $output 'Cargo.toml') "`n[workspace]`n"
$source = (Get-Content (Join-Path $upstream 'core/src/learn_store.rs') -Raw).Replace("`r`n","`n")
$constants = $source.Substring($source.IndexOf('pub const FILE:'), $source.IndexOf('// <fcntl.h>') - $source.IndexOf('pub const FILE:'))
$shared = $source.Substring($source.IndexOf('/// Latest accepted day'), $source.IndexOf("#[cfg(unix)]`nimpl LearnStore {") - $source.IndexOf('/// Latest accepted day'))
$imports = @'
use crate::learn::{context_key, is_han, is_single, single_key_ok, FINE_SEP, Record, GLOBAL, NEWWORD, NEWWORD_MAX, NEWWORD_MIN, NEXT_PREFIX, SENTINEL};
use std::fmt::Write as _;
use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
'@
$backend = Get-Content (Join-Path $root 'platform/learning_windows.rs') -Raw
Set-Content (Join-Path $output 'src/learn_store_windows.rs') ($imports + "`n" + $constants + $shared + $backend) -Encoding utf8NoBOM
$lib = Join-Path $output 'src/lib.rs'
$content = Get-Content $lib -Raw
if (!$content.Contains('pub mod learn_store;')) { throw 'Upstream module contract changed.' }
$content = $content.Replace('pub mod learn_store;', "#[cfg(not(windows))]`npub mod learn_store;`n#[cfg(windows)]`n#[path = `"learn_store_windows.rs`"]`npub mod learn_store;")
Set-Content $lib $content -Encoding utf8NoBOM
& "$PSScriptRoot/prepare-lm.ps1" -CorePath $output
& "$PSScriptRoot/prepare-candidates.ps1" -CorePath $output
& "$PSScriptRoot/prepare-keys.ps1" -CorePath $output
& "$PSScriptRoot/prepare-smart-input.ps1" -CorePath $output
& "$PSScriptRoot/prepare-selection-learning.ps1" -CorePath $output
& "$PSScriptRoot/prepare-resident.ps1" -CorePath $output
& "$PSScriptRoot/prepare-memory.ps1" -CorePath $output
Save-ContentCache -Root $output -ManifestPath $cacheManifest -Key $key -Paths $cachePaths
    Complete-TestStage -Stage $stage -Status 'passed' -Details @{ cache='miss'; upstream_commit=$pin.commit }
} catch {
    Complete-TestStage -Stage $stage -Status 'failed' -ErrorMessage $_.Exception.Message -Details @{ cache='miss'; upstream_commit=$pin.commit }
    throw
} finally {
    $env:SHANJIE_TEST_PARENT_STAGE_ID=$previousParent
}
