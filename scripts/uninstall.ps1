#Requires -Version 7
#Requires -RunAsAdministrator
$ErrorActionPreference = 'Stop'
if (![Environment]::Is64BitProcess) { throw '請用 64 位元 PowerShell 7 執行移除。' }
$failures = @()
$target = [IO.Path]::GetFullPath((Join-Path $env:ProgramFiles 'Shanjie'))
if (!(Test-Path -LiteralPath $target)) {
    Write-Host '善解安裝目錄不存在，沒有需要移除的程式檔案。'
    return
}
if ((Get-Item -LiteralPath $target -Force).Attributes -band [IO.FileAttributes]::ReparsePoint) {
    throw '安裝目錄是連結或 junction，請先確認實際位置；未執行移除。'
}
foreach ($arch in @('x64','x86')) {
    $system = if ($arch -eq 'x64') { 'System32' } else { 'SysWOW64' }
    $dll = Join-Path $target "$arch/Shanjie.dll"
    $registration = Start-Process "$env:WINDIR/$system/regsvr32.exe" -ArgumentList '/s','/u',('"' + $dll + '"') -Wait -PassThru -WindowStyle Hidden
    if ($registration.ExitCode) { $failures += $arch }
}
if ($failures.Count) { throw "TSF unregistration failed: $($failures -join ', ')" }
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class ShanjieRemoval {
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    public static extern bool MoveFileEx(string existing, string replacement, int flags);
}
'@
# Delete files first, then directories. Loaded IME DLL/model files stay locked
# until their host exits; Windows removes those at the next restart.
$pending = 0
$items = @(Get-ChildItem -LiteralPath $target -Recurse -Force | Sort-Object { $_.FullName.Length } -Descending)
$paths = @($items.FullName) + @($target)
foreach ($path in $paths) {
    $full = [IO.Path]::GetFullPath($path)
    if ($full -ne $target -and !$full.StartsWith($target + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
        throw "移除路徑超出安裝目錄：$full"
    }
    try {
        if ([IO.Directory]::Exists($full)) { [IO.Directory]::Delete($full, $false) }
        else { [IO.File]::Delete($full) }
    } catch {
        if (![ShanjieRemoval]::MoveFileEx($full, $null, 4)) {
            throw "無法刪除或安排重新啟動後刪除：$full（Win32 $([Runtime.InteropServices.Marshal]::GetLastWin32Error())）"
        }
        $pending++
    }
}
if ($pending) {
    Write-Host "已取消輸入法註冊；$pending 個使用中的檔案或目錄已安排重新啟動時刪除。請重新啟動 Windows 完成乾淨移除。"
} else {
    Write-Host '已取消輸入法註冊並刪除整個安裝目錄。請登出再登入以更新輸入法清單。'
}
