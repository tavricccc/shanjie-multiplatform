#Requires -Version 7
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$vs = & "${env:ProgramFiles(x86)}/Microsoft Visual Studio/Installer/vswhere.exe" -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
$vcvars = Join-Path $vs 'VC/Auxiliary/Build/vcvars64.bat'
$command = @"
@echo off
call "$vcvars" >nul
if errorlevel 1 exit /b 1
cd /d "$root/build/native-x64"
cl /nologo /std:c++20 /EHsc /O2 /MT /utf-8 /DUNICODE /D_UNICODE /DNOMINMAX /I"$root/src" /I"$root/upstream/shanjie/core/include" "$root/tests/engine_perf.cpp" "$root/src/engine.cpp" "$root/build/core/target/x86_64-pc-windows-msvc/release/core.lib" /link /OUT:"$root/build/engine-perf.exe" user32.lib shell32.lib advapi32.lib bcrypt.lib ntdll.lib ws2_32.lib userenv.lib psapi.lib
"@
Set-Content "$root/build/native-x64/engine-perf.cmd" $command -Encoding utf8NoBOM
& cmd.exe /d /c "$root/build/native-x64/engine-perf.cmd"
if ($LASTEXITCODE) { throw 'Engine benchmark build failed.' }
$savedLocal = $env:LOCALAPPDATA
try {
    $env:LOCALAPPDATA = "$root/build/perf-profile"
    foreach ($model in @(1,2)) {
        & "$root/build/engine-perf.exe" "$root/dist/data" $model
        if ($LASTEXITCODE) { throw "Model $model benchmark failed." }
    }
} finally { $env:LOCALAPPDATA = $savedLocal }
