#Requires -Version 7
#Requires -RunAsAdministrator
$ErrorActionPreference = 'Stop'
$dll = Join-Path $PSScriptRoot 'Shanjie.dll'
$registration = Start-Process "$env:WINDIR/System32/regsvr32.exe" -ArgumentList '/s','/u',('"' + $dll + '"') -Wait -PassThru -WindowStyle Hidden
if ($registration.ExitCode) { throw "TSF unregistration failed: $($registration.ExitCode)" }
Write-Host '已取消註冊。請登出再登入後刪除安裝資料夾。'
