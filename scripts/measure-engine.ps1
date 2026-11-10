#Requires -Version 7
param([switch]$Learning)
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot/msvc-command.ps1"
$root = Split-Path $PSScriptRoot -Parent
$command = @"
@echo off
$(Get-MsvcSetup x64)
if errorlevel 1 exit /b 1
cd /d "$root/build/native-x64"
cl /nologo /std:c++20 /EHsc /O2 /MT /utf-8 /DUNICODE /D_UNICODE /DNOMINMAX /I"$root/tests/support" /I"$root/upstream/shanjie/core/include" "$root/tests/engine_perf.cpp" "$root/tests/support/engine.cpp" "$root/windows/target/x86_64-pc-windows-msvc/release/shanjie_windows.lib" /link /OUT:"$root/build/engine-perf.exe" user32.lib shell32.lib advapi32.lib bcrypt.lib ntdll.lib ws2_32.lib userenv.lib psapi.lib ole32.lib oleaut32.lib gdi32.lib d2d1.lib dwrite.lib dwmapi.lib windowsapp.lib
"@
Invoke-HarnessBuild $command "$root/build/native-x64/engine-perf.cmd" "$root/build/engine-perf.exe"
if ($LASTEXITCODE) { throw 'Engine benchmark build failed.' }
$savedLocal = $env:LOCALAPPDATA
$savedTest = $env:SHANJIE_TEST_PROFILE
try {
    $env:SHANJIE_TEST_PROFILE = '1'
    $measurementProfile = if ($Learning) { "$root/build/perf-profile-learning" } else { "$root/build/perf-profile" }
    $env:LOCALAPPDATA = [IO.Path]::GetFullPath($measurementProfile)
    if ($Learning) {
        New-Item -ItemType Directory -Path "$env:LOCALAPPDATA/Shanjie" -Force | Out-Null
        $day = [int][Math]::Floor(([DateTime]::Today - [DateTime]::new(1970,1,1)).TotalDays)
        $rows = [Text.StringBuilder]::new("#shanjie-learning v1`n")
        foreach ($index in 0..7999) { [void]$rows.Append("字$([char](0x4e00+$index))`tㄋㄧˇ`t妳`t1`t$day`n") }
        [IO.File]::WriteAllText("$env:LOCALAPPDATA/Shanjie/learning.tsv",$rows.ToString())
    }
    $arguments = @("$root/dist/data")
    if ($Learning) { $arguments += '--learning' }
    & "$root/build/engine-perf.exe" @arguments
    if ($LASTEXITCODE) { throw 'Engine benchmark failed.' }
} finally {
    try {
        # Engine::free releases the session; the resident process has its own lifetime.
        # Keep the isolated profile selected while stopping only this measurement's core.
        $stop = Start-Process "$root/dist/tools/shanjie-core.exe" -ArgumentList '--stop' -WindowStyle Hidden -PassThru
        $null = $stop.Handle
        if (!$stop.WaitForExit(5000) -or $stop.ExitCode -ne 0) { throw 'Measurement core stop request failed.' }
    } finally { $env:LOCALAPPDATA = $savedLocal; $env:SHANJIE_TEST_PROFILE = $savedTest }
}
