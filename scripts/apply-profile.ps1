#Requires -Version 7
param([Parameter(Mandatory)][string]$ProfileFile)
$ErrorActionPreference = 'Stop'
& (Join-Path $PSScriptRoot 'tools/shanjie-manager.exe') apply-profile (Resolve-Path -LiteralPath $ProfileFile).Path
if ($LASTEXITCODE) { throw '善解設定套用未完成。' }
