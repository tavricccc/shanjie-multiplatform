#Requires -Version 7
param([Parameter(Mandatory)][string]$CorePath)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
# Upstream maps the trigram table (and the 0c model) with mmap. Keep that code for Unix and give Windows the same
# `Mapped` type over the mapping the model arrays already use.
$file = Join-Path $CorePath 'src/trigram.rs'
$content = (Get-Content $file -Raw).Replace("`r`n","`n")
function Replace-Once([string]$old,[string]$new) {
    if ([regex]::Matches($script:content,[regex]::Escape($old)).Count -ne 1) { throw "Upstream mapped-file contract changed: $old" }
    $script:content = $script:content.Replace($old,$new)
}
Replace-Once "use std::ffi::c_void;`nuse std::ops::Deref;`nuse std::os::fd::AsRawFd;`nuse std::path::Path;`n" "use std::ops::Deref;`n"
Replace-Once '// <sys/mman.h>: PROT_READ' "#[cfg(unix)]`nmod unix_mapped {`nuse std::ffi::c_void;`nuse std::ops::Deref;`nuse std::os::fd::AsRawFd;`nuse std::path::Path;`n// <sys/mman.h>: PROT_READ"
Replace-Once "/// Where a table's bytes live" "}`n#[cfg(unix)]`npub(crate) use unix_mapped::Mapped;`n#[cfg(windows)]`npub(crate) use crate::mapped_windows::Mapped;`n`n/// Where a table's bytes live"
Set-Content $file $content -Encoding utf8NoBOM
Copy-Item "$root/platform/mapped_windows.rs" "$CorePath/src/mapped_windows.rs" -Force
Add-Content (Join-Path $CorePath 'src/lib.rs') "`n#[cfg(windows)]`nmod mapped_windows;`n"
