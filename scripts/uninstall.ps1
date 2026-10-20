#Requires -Version 7
#Requires -RunAsAdministrator
param([switch]$KeepUserData,[switch]$DeleteUserData)
$ErrorActionPreference = 'Stop'
if ($KeepUserData -and $DeleteUserData) { throw '請只選一種個人資料處理方式。' }
$arguments = @('uninstall')
if ($KeepUserData) { $arguments += '--keep-user-data' }
if ($DeleteUserData) { $arguments += '--delete-user-data' }
& (Join-Path $PSScriptRoot '../tools/shanjie-manager.exe') @arguments
if ($LASTEXITCODE) { throw '善解解除安裝未完成。' }
