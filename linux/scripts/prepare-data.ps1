#Requires -Version 7
# The Linux runtime data: the same files as the Windows package's data folder (scripts/build.ps1 and
# scripts/prepare-models.ps1), so both platforms decode with the same lexicon, mapped model and word lists.
param([string]$Output = 'build/linux/data')
$ErrorActionPreference = 'Stop'
$root = Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
if ([IO.Path]::IsPathRooted($Output) -or $Output.Split('/') -contains '..') { throw 'Output must be a relative path inside the repository.' }
$target = [IO.Path]::GetFullPath((Join-Path $root $Output))
$stage = "$target.stage"
if (Test-Path -LiteralPath $stage) { Remove-Item -LiteralPath $stage -Recurse -Force }
New-Item -ItemType Directory -Path $stage -Force | Out-Null
& "$root/scripts/prepare-models.ps1" -Output "$Output.stage" | Out-Null
& "$root/scripts/prepare-data.ps1" -Output "$Output.stage"
if (Test-Path -LiteralPath $target) { Remove-Item -LiteralPath $target -Recurse -Force }
Move-Item -LiteralPath $stage -Destination $target
Write-Output "prepared $Output"
