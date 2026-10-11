#Requires -Version 7
param([switch]$Learning)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$vs = & "${env:ProgramFiles(x86)}/Microsoft Visual Studio/Installer/vswhere.exe" -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
$vcvars = Join-Path $vs 'VC/Auxiliary/Build/vcvars64.bat'
$command = @"
@echo off
call "$vcvars" >nul
if errorlevel 1 exit /b 1
cd /d "$root/build/native-x64"
cl /nologo /std:c++20 /EHsc /O2 /MT /utf-8 /DUNICODE /D_UNICODE /DNOMINMAX /I"$root/tests/support" /I"$root/upstream/shanjie/core/include" "$root/tests/engine_perf.cpp" "$root/tests/support/engine.cpp" "$root/windows/target/x86_64-pc-windows-msvc/release/shanjie_windows.lib" /link /OUT:"$root/build/engine-perf.exe" user32.lib shell32.lib advapi32.lib bcrypt.lib ntdll.lib ws2_32.lib userenv.lib psapi.lib ole32.lib oleaut32.lib gdi32.lib d2d1.lib dwrite.lib dwmapi.lib windowsapp.lib
"@
Set-Content "$root/build/native-x64/engine-perf.cmd" $command -Encoding utf8NoBOM
& cmd.exe /d /c "$root/build/native-x64/engine-perf.cmd"
if ($LASTEXITCODE) { throw 'Engine benchmark build failed.' }
$savedLocal = $env:LOCALAPPDATA
try {
    $env:LOCALAPPDATA = if ($Learning) { "$root/build/perf-profile-learning" } else { "$root/build/perf-profile" }
    if ($Learning) {
        New-Item -ItemType Directory -Path "$env:LOCALAPPDATA/Shanjie" -Force | Out-Null
        $day = [int][Math]::Floor(([DateTime]::Today - [DateTime]::new(1970,1,1)).TotalDays)
        $rows = [Text.StringBuilder]::new("#shanjie-learning v1`n")
        foreach ($index in 0..7999) { [void]$rows.Append("字$([char](0x4e00+$index))`tㄋㄧˇ`t妳`t1`t$day`n") }
        [IO.File]::WriteAllText("$env:LOCALAPPDATA/Shanjie/learning.tsv",$rows.ToString())
    }
    foreach ($model in @(5)) {
        $arguments = @("$root/dist/data",$model)
        if ($Learning) { $arguments += '--learning' }
        & "$root/build/engine-perf.exe" @arguments
        if ($LASTEXITCODE) { throw "Model $model benchmark failed." }
    }
} finally { $env:LOCALAPPDATA = $savedLocal }
