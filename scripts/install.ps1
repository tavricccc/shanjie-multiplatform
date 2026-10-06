#Requires -Version 7
#Requires -RunAsAdministrator
$ErrorActionPreference = 'Stop'
if (![Environment]::Is64BitProcess) { throw '請用 64 位元 PowerShell 7 執行安裝。' }
foreach ($line in Get-Content (Join-Path $PSScriptRoot 'SHA256SUMS.txt')) {
    $hash, $relative = $line -split '  ', 2
    if ((Get-FileHash (Join-Path $PSScriptRoot $relative) -Algorithm SHA256).Hash.ToLowerInvariant() -ne $hash) {
        throw "安裝包檔案校驗失敗：$relative"
    }
}
$target = Join-Path $env:ProgramFiles 'Shanjie'
New-Item -ItemType Directory -Path $target -Force | Out-Null
Copy-Item "$PSScriptRoot/*" $target -Recurse -Force
foreach ($arch in @('x64','x86')) {
    $system = if ($arch -eq 'x64') { 'System32' } else { 'SysWOW64' }
    $dll = Join-Path $target "$arch/Shanjie.dll"
    $registration = Start-Process "$env:WINDIR/$system/regsvr32.exe" -ArgumentList '/s',('"' + $dll + '"') -Wait -PassThru -WindowStyle Hidden
    if ($registration.ExitCode) {
        & (Join-Path $target 'uninstall.ps1')
        throw "TSF $arch registration failed: $($registration.ExitCode)"
    }
}
& (Join-Path $target 'profiles.ps1')
Write-Host '已註冊善解。請登出再登入，於 Windows 輸入法清單選擇善解注音。'
