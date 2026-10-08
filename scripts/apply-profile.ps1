#Requires -Version 7
param([Parameter(Mandatory)][string]$ProfileFile)
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot/profile-ini.ps1"
$source = (Resolve-Path -LiteralPath $ProfileFile).Path
$target = Join-Path $env:LOCALAPPDATA 'Shanjie'
New-Item -ItemType Directory -Path $target -Force | Out-Null
$destination = Join-Path $target 'profile.ini'
$profile = Read-ProfileIni ([IO.File]::ReadAllText($source))
# Presets share their files with WinUI; retain data, model and appearance choices.
if (Test-Path -LiteralPath $destination) {
    $previous = Read-ProfileIni ([IO.File]::ReadAllText($destination))
    $profile = Preserve-ProfileData $profile $previous
}
$content = Write-ProfileIni $profile
[IO.File]::WriteAllText("$destination.tmp", $content, [Text.UTF8Encoding]::new($false))
[IO.File]::Move("$destination.tmp", $destination, $true)
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
namespace Shanjie {
    public static class ProfileNotification {
        [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern uint RegisterWindowMessage(string name);
        [DllImport("user32.dll")] public static extern bool SendNotifyMessage(IntPtr window, uint message, UIntPtr wp, IntPtr lp);
        public static void Send() { SendNotifyMessage(new IntPtr(0xffff), RegisterWindowMessage("Shanjie.ProfileChanged.v1"), UIntPtr.Zero, IntPtr.Zero); }
    }
}
'@
[Shanjie.ProfileNotification]::Send()
Write-Host "已套用：$source"
Write-Host '已通知善解立即套用操作設定。模型、詞庫、學習與外觀已保留。'
