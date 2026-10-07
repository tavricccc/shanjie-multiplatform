#Requires -Version 7
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$vs = & "${env:ProgramFiles(x86)}/Microsoft Visual Studio/Installer/vswhere.exe" -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
$vcvars = Join-Path $vs 'VC/Auxiliary/Build/vcvarsall.bat'
foreach ($architecture in @('x64','x86')) {
    $command = @"
@echo off
call "$vcvars" $architecture >nul
if errorlevel 1 exit /b 1
cd /d "$root/build/native-$architecture"
cl /nologo /std:c++20 /EHsc /O2 /MT /W4 /utf-8 /DUNICODE /D_UNICODE /DNOMINMAX /I"$root/src" /I"$root/upstream/shanjie/core/include" "$root/tests/candidate_host_smoke.cpp" "$root/src/candidate_host.cpp" /link /OUT:"$root/build/candidate-host-$architecture.exe" user32.lib dwmapi.lib
"@
    Set-Content "$root/build/native-$architecture/ui-smoke.cmd" $command -Encoding utf8NoBOM
    & cmd.exe /d /c "$root/build/native-$architecture/ui-smoke.cmd"
    if ($LASTEXITCODE) { throw 'Candidate host harness build failed.' }
    & "$root/build/candidate-host-$architecture.exe" "$root/dist/$architecture/Shanjie.dll"
    if ($LASTEXITCODE) { throw "$architecture WinUI IPC failed." }
}
$output = Join-Path $root 'build/ui-review-final'
Start-Process -FilePath "$root/dist/ui/Shanjie.UI.exe" -ArgumentList '--verify-ui',('"' + $output + '"') -WindowStyle Hidden -Wait
if ((Get-Content "$output/result.txt" -Raw) -ne 'PASS') { throw 'WinUI loading, profile persistence or visual capture failed.' }
Write-Host "WinUI profile/capture checks passed: $output"
