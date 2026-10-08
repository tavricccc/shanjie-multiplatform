#Requires -Version 7
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$upstream = Join-Path $root 'upstream/shanjie'
$pin = Get-Content (Join-Path $root 'UPSTREAM.json') -Raw | ConvertFrom-Json
$actual = git -C $upstream rev-parse HEAD
if ($actual -ne $pin.commit) { throw 'Upstream pin changed; review UPSTREAM.json before building.' }
if (git -C $upstream status --porcelain) { throw 'Upstream checkout must be clean.' }
$output = Join-Path $root 'build/core'
New-Item -ItemType Directory -Path $output -Force | Out-Null
Copy-Item (Join-Path $upstream 'core/src') $output -Recurse -Force
Copy-Item (Join-Path $upstream 'core/Cargo.toml') $output -Force
Add-Content (Join-Path $output 'Cargo.toml') "`n[workspace]`n"
$source = Get-Content (Join-Path $upstream 'core/src/learn_store.rs') -Raw
$constants = $source.Substring($source.IndexOf('pub const FILE:'), $source.IndexOf('// <fcntl.h>') - $source.IndexOf('pub const FILE:'))
$shared = $source.Substring($source.IndexOf('/// Latest accepted day'), $source.IndexOf('impl LearnStore {') - $source.IndexOf('/// Latest accepted day'))
$imports = @'
use crate::learn::{context_key, is_single, Record, GLOBAL, SENTINEL};
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
& "$PSScriptRoot/prepare-candidates.ps1" -CorePath $output
