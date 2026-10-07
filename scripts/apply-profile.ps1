#Requires -Version 7
param([Parameter(Mandatory)][string]$ProfileFile)
$ErrorActionPreference = 'Stop'
$source = (Resolve-Path -LiteralPath $ProfileFile).Path
$target = Join-Path $env:LOCALAPPDATA 'Shanjie'
New-Item -ItemType Directory -Path $target -Force | Out-Null
$destination = Join-Path $target 'profile.ini'
$content = [IO.File]::ReadAllText($source)
# A preset changes interaction settings, while retaining the user's saved dictionary.
if (Test-Path -LiteralPath $destination) {
    $previous = [IO.File]::ReadAllText($destination)
    $inputSection = [regex]::Match($previous, '(?ms)^\[input\]\r?\n(.*?)(?=^\[|\z)').Groups[1].Value
    $dictionary = [regex]::Match($inputSection, '(?m)^dictionary=(.+)\r?$').Groups[1].Value.Trim()
    $newInput = [regex]::Match($content, '(?ms)^\[input\]\r?\n(.*?)(?=^\[|\z)').Groups[1].Value
    if ($dictionary -and $newInput -notmatch '(?m)^dictionary=') {
        $content = $content.Replace('[input]', "[input]`ndictionary=$dictionary")
    }
}
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
Write-Host '已通知善解立即套用設定。切換鍵盤或詞庫時會先提交目前組字。'
