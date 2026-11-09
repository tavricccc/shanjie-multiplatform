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
$cacheInputs = @('UPSTREAM.json','MODEL-SOURCES.json','platform','data/english-words.txt','scripts/test-support.psm1') + $preparationScripts
$toolVersions = @()
foreach ($tool in 'rustc','cargo','python','python3') {
    $command = Get-Command $tool -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($command) { $toolVersions += "$tool=$((& $command.Source --version 2>&1 | Select-Object -First 1).ToString().Trim())" }
}
$cacheValues = @($pin.commit,$englishHash)+$toolVersions
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
function New-CoreOverlay {
foreach ($relative in $cachePaths) {
    $old = Join-Path $output $relative
    if (Test-Path -LiteralPath $old) { Preserve-Output $old }
}
New-Item -ItemType Directory -Path $output -Force | Out-Null
Copy-Item (Join-Path $upstream 'core/src') $output -Recurse -Force
Copy-Item (Join-Path $upstream 'core/Cargo.toml') $output -Force
Add-Content (Join-Path $output 'Cargo.toml') "`n[workspace]`n"
$source = Get-Content (Join-Path $upstream 'core/src/learn_store.rs') -Raw
$constants = $source.Substring($source.IndexOf('pub const FILE:'), $source.IndexOf('// <fcntl.h>') - $source.IndexOf('pub const FILE:'))
$shared = $source.Substring($source.IndexOf('/// Latest accepted day'), $source.IndexOf('impl LearnStore {') - $source.IndexOf('/// Latest accepted day'))
$imports = @'
use crate::learn::{context_key, is_full_context, is_han, is_single, Record, GLOBAL, NEWWORD, NEWWORD_MAX, NEWWORD_MIN, NEXT_PREFIX, SENTINEL};
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
$learn = Join-Path $output 'src/learn.rs'
$content = Get-Content $learn -Raw
$content = $content.Replace('pub fn local_day() -> i64 {', "#[cfg(not(windows))]`npub fn local_day() -> i64 {")
$content += "`n#[cfg(windows)]`n" + (Get-Content (Join-Path $root 'platform/clock_windows.rs') -Raw)
Set-Content $learn $content -Encoding utf8NoBOM
& "$PSScriptRoot/prepare-lm.ps1" -CorePath $output
& "$PSScriptRoot/prepare-trigram.ps1" -CorePath $output
& "$PSScriptRoot/prepare-candidates.ps1" -CorePath $output
& "$PSScriptRoot/prepare-keys.ps1" -CorePath $output
& "$PSScriptRoot/prepare-smart-input.ps1" -CorePath $output
& "$PSScriptRoot/prepare-selection-learning.ps1" -CorePath $output
& "$PSScriptRoot/prepare-resident.ps1" -CorePath $output
& "$PSScriptRoot/prepare-memory.ps1" -CorePath $output
# Keep the registered chat cells for SR7 bundles and the separately shipped SR8 model.
$colloquial = Join-Path $output 'src/colloquial.rs'
$content = Get-Content $colloquial -Raw
$anchor = 'profile == Profile::Chat && model.file_sha256() == PAIRED_TINYLM_SHA256'
if (!$content.Contains($anchor)) { throw 'Upstream colloquial model pairing changed.' }
$content = $content.Replace($anchor, 'profile == Profile::Chat && (model.file_sha256() == PAIRED_TINYLM_SHA256 || model.file_sha256() == "356a37beede1a754cb6e963a52d0f20ae6574cbcd332d0b1bfc9686c0a97c5de")')
Set-Content $colloquial $content -Encoding utf8NoBOM
}

try {
    New-CoreOverlay
    Save-ContentCache -Root $output -ManifestPath $cacheManifest -Key $key -Paths $cachePaths
    Complete-TestStage -Stage $stage -Status 'passed' -Details @{ cache='miss'; upstream_commit=$pin.commit; source_files=(Get-RelativeFileEntries -Root $output -Paths $cachePaths).Count }
} catch {
    Complete-TestStage -Stage $stage -Status 'failed' -ErrorMessage $_.Exception.Message -Details @{ cache='miss'; upstream_commit=$pin.commit }
    throw
}
