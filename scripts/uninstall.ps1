#Requires -Version 7
#Requires -RunAsAdministrator
$ErrorActionPreference = 'Stop'
if (![Environment]::Is64BitProcess) { throw '請用 64 位元 PowerShell 7 執行移除。' }
$failures = @()
foreach ($arch in @('x64','x86')) {
    $system = if ($arch -eq 'x64') { 'System32' } else { 'SysWOW64' }
    $dll = Join-Path $PSScriptRoot "$arch/Shanjie.dll"
    $registration = Start-Process "$env:WINDIR/$system/regsvr32.exe" -ArgumentList '/s','/u',('"' + $dll + '"') -Wait -PassThru -WindowStyle Hidden
    if ($registration.ExitCode) { $failures += $arch }
}
if ($failures.Count) { throw "TSF unregistration failed: $($failures -join ', ')" }
Write-Host '已取消註冊。請登出再登入後刪除安裝資料夾。'
