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
cl /nologo /std:c++20 /EHsc /O2 /MT /W4 /utf-8 /DUNICODE /D_UNICODE /DNOMINMAX /I"$root/tests/support" /I"$root/upstream/shanjie/core/include" "$root/tests/windows_interaction.cpp" "$root/tests/support/engine.cpp" "$root/tests/support/key_policy.cpp" "$root/tests/support/special_input.cpp" "$root/tests/support/associated_candidates.cpp" "$root/tests/support/pinyin_input.cpp" "$root/windows/target/x86_64-pc-windows-msvc/release/shanjie_windows.lib" /link /OUT:"$root/build/windows-interaction.exe" ole32.lib user32.lib gdi32.lib d2d1.lib dwrite.lib dwmapi.lib windowsapp.lib advapi32.lib bcrypt.lib ntdll.lib ws2_32.lib userenv.lib
"@
Set-Content "$root/build/native-x64/interaction.cmd" $command -Encoding utf8NoBOM
& cmd.exe /d /c "$root/build/native-x64/interaction.cmd"
if ($LASTEXITCODE) { throw 'Interaction harness build failed.' }
$testRoot = Join-Path $root ('build/interaction-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory $testRoot -Force | Out-Null
$previous = $env:LOCALAPPDATA
try {
    $env:LOCALAPPDATA = $testRoot
    & "$root/build/windows-interaction.exe" "$root/dist/data"
    if ($LASTEXITCODE) { throw "Interaction checks failed: $LASTEXITCODE" }
} finally { $env:LOCALAPPDATA = $previous }
