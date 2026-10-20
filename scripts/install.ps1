#Requires -Version 7
#Requires -RunAsAdministrator
param([ValidateSet('standard','eten')][string]$Keyboard,[switch]$ChooseKeyboard)
$ErrorActionPreference = 'Stop'
$arguments=@('install')
if($Keyboard){$arguments+=@('--keyboard',$Keyboard)}
if($ChooseKeyboard){$arguments+='--choose-keyboard'}
& (Join-Path $PSScriptRoot '../tools/shanjie-manager.exe') @arguments
if ($LASTEXITCODE) { throw '善解安裝未完成。' }
