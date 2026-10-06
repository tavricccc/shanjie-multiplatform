#Requires -Version 7
#Requires -RunAsAdministrator
$ErrorActionPreference = 'Stop'
$target = Join-Path $env:ProgramFiles 'Shanjie'
New-Item -ItemType Directory -Path $target -Force | Out-Null
Copy-Item "$PSScriptRoot/*" $target -Recurse -Force
$registration = Start-Process "$env:WINDIR/System32/regsvr32.exe" -ArgumentList '/s',('"' + (Join-Path $target 'Shanjie.dll') + '"') -Wait -PassThru -WindowStyle Hidden
if ($registration.ExitCode) { throw "TSF registration failed: $($registration.ExitCode)" }
Write-Host '已註冊善解。請登出再登入，於 Windows 輸入法清單選擇善解注音。'
