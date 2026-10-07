#Requires -Version 7
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$vs = & "${env:ProgramFiles(x86)}/Microsoft Visual Studio/Installer/vswhere.exe" -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
$vcvars = Join-Path $vs 'VC/Auxiliary/Build/vcvarsall.bat'
$captures = "$root/build/mode-review"
New-Item -ItemType Directory -Path $captures -Force | Out-Null
foreach ($architecture in @('x64','x86')) {
    $command = @"
@echo off
call "$vcvars" $architecture >nul
if errorlevel 1 exit /b 1
cd /d "$root/build/native-$architecture"
cl /nologo /std:c++20 /EHsc /O2 /MT /W4 /utf-8 /DUNICODE /D_UNICODE /DNOMINMAX /I"$root/src" "$root/tests/mode_button_smoke.cpp" "$root/src/mode_button.cpp" "$root/src/mode_indicator.cpp" /link /OUT:"$root/build/mode-$architecture.exe" ole32.lib oleaut32.lib uuid.lib user32.lib gdi32.lib gdiplus.lib
"@
    $script = "$root/build/native-$architecture/mode-smoke.cmd"
    Set-Content $script $command -Encoding utf8NoBOM
    & cmd.exe /d /c $script
    if ($LASTEXITCODE) { throw 'Mode harness build failed.' }
    & "$root/build/mode-$architecture.exe" "$root/dist/$architecture/Shanjie.dll" $captures
    if ($LASTEXITCODE) { throw "$architecture mode surface failed." }
}
