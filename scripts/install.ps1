#Requires -Version 7
#Requires -RunAsAdministrator
$ErrorActionPreference = 'Stop'
& (Join-Path $PSScriptRoot 'tools/shanjie-manager.exe') install
if ($LASTEXITCODE) { throw '善解安裝未完成。' }
