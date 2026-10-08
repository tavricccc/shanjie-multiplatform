#Requires -Version 7
param([switch]$Remove)
$ErrorActionPreference = 'Stop'
$arguments = @('profiles')
if ($Remove) { $arguments += '--remove' }
& (Join-Path $PSScriptRoot 'tools/shanjie-manager.exe') @arguments
if ($LASTEXITCODE) { throw 'Windows 輸入法清單更新未完成。' }
