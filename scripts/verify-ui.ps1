#Requires -Version 7
param([switch]$Benchmark)
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
cl /nologo /std:c++20 /EHsc /O2 /MT /W4 /utf-8 /DUNICODE /D_UNICODE /DNOMINMAX /I"$root/tests/support" /I"$root/upstream/shanjie/core/include" "$root/tests/candidate_host_smoke.cpp" "$root/tests/support/candidate_host.cpp" "$root/tests/support/engine.cpp" "$root/windows/target/$(if ($architecture -eq 'x64') {'x86_64'} else {'i686'})-pc-windows-msvc/release/shanjie_windows.lib" /link /OUT:"$root/build/candidate-host-$architecture.exe" ole32.lib user32.lib gdi32.lib d2d1.lib dwrite.lib dwmapi.lib windowsapp.lib psapi.lib advapi32.lib bcrypt.lib ntdll.lib ws2_32.lib userenv.lib
"@
    Set-Content "$root/build/native-$architecture/ui-smoke.cmd" $command -Encoding utf8NoBOM
    & cmd.exe /d /c "$root/build/native-$architecture/ui-smoke.cmd"
    if ($LASTEXITCODE) { throw 'Candidate host harness build failed.' }
    & "$root/build/candidate-host-$architecture.exe" "$root/dist/$architecture/Shanjie.dll" "$root/build/native-review/$architecture"
    if ($LASTEXITCODE) { throw "$architecture WinUI IPC failed." }
}
# A saved downloaded model survives closing and reopening the settings: separate processes, isolated profile, a package the scan really validates.
$modelOutput = Join-Path $root ('build/ui-model-startup/' + [guid]::NewGuid().ToString('N'))
$modelFolder = Join-Path $modelOutput 'profile/Shanjie/Models/fixture-model'
New-Item -ItemType Directory -Path $modelFolder -Force | Out-Null
Copy-Item "$root/dist/data/models/builtin/bigram.winmap","$root/dist/data/models/builtin/classes.sjc" $modelFolder
$assets = (Get-Content "$root/MODEL-SOURCES.json" -Raw | ConvertFrom-Json).builtin.assets
$bigram = $assets | Where-Object role -eq 'bigram'; $classes = $assets | Where-Object role -eq 'classes'
@{ id = 'fixture-model'; name = 'Fixture model'; model_release = $bigram.release; classes_release = $classes.release
   model_sha256 = $bigram.sha256; classes_sha256 = $classes.sha256; model_size = $bigram.size; classes_size = $classes.size
   map_sha256 = (Get-FileHash "$modelFolder/bigram.winmap" -Algorithm SHA256).Hash.ToLowerInvariant() } | ConvertTo-Json | Set-Content "$modelFolder/model.json" -Encoding utf8NoBOM
(Get-Content "$root/profiles/windows.ini" -Raw) -replace '(?m)^model_id=.*','model_id=fixture-model' | Set-Content "$modelOutput/profile/Shanjie/profile.ini" -Encoding utf8NoBOM
foreach ($phase in 'first-open','reopen') {
    $process = Start-Process -FilePath "$root/dist/ui/Shanjie.UI.exe" -ArgumentList '--verify-model-startup',('"' + $modelOutput + '"'),$phase -WindowStyle Hidden -PassThru
    $process.WaitForExit()
    $result = Get-Content "$modelOutput/result-$phase.txt" -Raw
    if ($result -ne 'PASS') { throw "Saved model after reopening: $result" }
}
Remove-Item -LiteralPath $modelOutput -Recurse -Force
Write-Host 'Saved model kept across settings restarts.'
$output = Join-Path $root 'build/ui-review-final'
$process = Start-Process -FilePath "$root/dist/ui/Shanjie.UI.exe" -ArgumentList '--verify-ui',('"' + $output + '"') -WindowStyle Hidden -PassThru
$process.WaitForExit()
if ((Get-Content "$output/result.txt" -Raw) -ne 'PASS') { throw 'WinUI loading, profile persistence or visual capture failed.' }
Write-Host "WinUI profile/capture checks passed: $output"
# Setup and uninstall pages, rendered offscreen with sample progress; nothing is installed.
$setupOutput = Join-Path $root 'build/ui-review-setup'
$process = Start-Process -FilePath "$root/dist/ui/Shanjie.UI.exe" -ArgumentList '--verify-setup',('"' + $setupOutput + '"') -WindowStyle Hidden -PassThru
$process.WaitForExit()
if ((Get-Content "$setupOutput/result.txt" -Raw) -ne 'PASS') { throw 'WinUI setup pages failed.' }
Write-Host "WinUI setup page checks passed: $setupOutput"
