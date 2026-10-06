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
$version = (Get-FileHash "$PSScriptRoot/x64/Shanjie.dll").Hash.Substring(0,8) + (Get-FileHash "$PSScriptRoot/x86/Shanjie.dll").Hash.Substring(0,8)
$binaries = Join-Path $target "versions/$version"
foreach ($file in Get-ChildItem -LiteralPath $PSScriptRoot -File -Recurse) {
    $relative = [IO.Path]::GetRelativePath($PSScriptRoot,$file.FullName)
    $destination = if ($relative -match '^(x64|x86)[\\/]') { Join-Path $binaries $relative } else { Join-Path $target $relative }
    # Identical mapped data must not be overwritten while an application uses it.
    if ((Test-Path -LiteralPath $destination) -and (Get-FileHash -LiteralPath $destination).Hash -eq (Get-FileHash -LiteralPath $file.FullName).Hash) { continue }
    New-Item -ItemType Directory -Path (Split-Path $destination) -Force | Out-Null
    Copy-Item -LiteralPath $file.FullName -Destination $destination -Force
}
foreach ($arch in @('x64','x86')) {
    $system = if ($arch -eq 'x64') { 'System32' } else { 'SysWOW64' }
    $dll = Join-Path $binaries "$arch/Shanjie.dll"
    $registration = Start-Process "$env:WINDIR/$system/regsvr32.exe" -ArgumentList '/s',('"' + $dll + '"') -Wait -PassThru -WindowStyle Hidden
    if ($registration.ExitCode) {
        & (Join-Path $target 'uninstall.ps1')
        throw "TSF $arch registration failed: $($registration.ExitCode)"
    }
}
& (Join-Path $target 'profiles.ps1')
Write-Host '已註冊善解。請登出再登入，於 Windows 輸入法清單選擇善解注音。'
