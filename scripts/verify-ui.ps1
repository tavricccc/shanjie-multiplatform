#Requires -Version 7
param([switch]$Benchmark,[ValidateSet('x64','x86')][string[]]$Architecture=@('x64','x86'),[switch]$CandidateHostOnly,[switch]$SkipCandidateHost,[string]$HarnessPath,[string]$OutputRoot)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
Import-Module "$PSScriptRoot/test-support.psm1" -ErrorAction Stop
$null=Initialize-TestTimings -ProjectRoot $root -Name 'verify-ui'
$buildRoot=[IO.Path]::GetFullPath((Join-Path $root 'build'))
$uiRoot=if($OutputRoot){[IO.Path]::GetFullPath($OutputRoot)}else{Join-Path $buildRoot ('ui-verification-'+[guid]::NewGuid().ToString('N'))}
if(!$uiRoot.StartsWith($buildRoot+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)){throw "Refusing a UI fixture outside build/: $uiRoot"}
$null=New-Item -ItemType Directory -Path $uiRoot -Force
$vs = & "${env:ProgramFiles(x86)}/Microsoft Visual Studio/Installer/vswhere.exe" -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
$vcvars = Join-Path $vs 'VC/Auxiliary/Build/vcvarsall.bat'
if (!$SkipCandidateHost) { foreach ($architecture in $Architecture) {
    $candidateHarness = if ($HarnessPath) { [IO.Path]::GetFullPath($HarnessPath) } else { "${root}/build/candidate-host-$architecture.exe" }
    if ($HarnessPath -and $Architecture.Count -ne 1) { throw 'A prebuilt candidate-host harness can only be used for one architecture per invocation.' }
    if ($HarnessPath -and !(Test-Path -LiteralPath $candidateHarness)) { throw "Candidate-host harness is missing: $candidateHarness" }
    if (!$HarnessPath) {
    $command = @"
@echo off
call "$vcvars" $architecture >nul
if errorlevel 1 exit /b 1
cd /d "$root/build/native-$architecture"
cl /nologo /std:c++20 /EHsc /O2 /MT /W4 /utf-8 /DUNICODE /D_UNICODE /DNOMINMAX /I"$root/tests/support" /I"$root/upstream/shanjie/core/include" "$root/tests/candidate_host_smoke.cpp" "$root/tests/support/candidate_host.cpp" "$root/tests/support/engine.cpp" "$root/windows/target/$(if ($architecture -eq 'x64') {'x86_64'} else {'i686'})-pc-windows-msvc/release/shanjie_windows.lib" /link /OUT:"$root/build/candidate-host-$architecture.exe" ole32.lib user32.lib gdi32.lib d2d1.lib dwrite.lib dwmapi.lib windowsapp.lib psapi.lib advapi32.lib bcrypt.lib ntdll.lib ws2_32.lib userenv.lib
"@
    Set-Content "$root/build/native-$architecture/ui-smoke.cmd" $command -Encoding utf8NoBOM
    Invoke-Timed -Name "Compile candidate-host harness ($architecture)" -Category 'build.cpp.harness' -Details @{architecture=$architecture;source='tests/candidate_host_smoke.cpp'} -Action {
        & cmd.exe /d /c "$root/build/native-$architecture/ui-smoke.cmd"
        if ($LASTEXITCODE) { throw 'Candidate host harness build failed.' }
    }
    }
    Invoke-Timed -Name "Candidate-host WinUI IPC ($architecture)" -Category 'ui.candidate-host' -Details @{architecture=$architecture} -Action {
        & $candidateHarness "$root/dist/$architecture/Shanjie.dll" "$root/build/native-review/$architecture"
        if ($LASTEXITCODE) { throw "$architecture WinUI IPC failed." }
    }
} }
if ($CandidateHostOnly) { return }
# A saved downloaded model survives closing and reopening the settings: separate processes, isolated profile, a package the scan really validates.
# A WinUI process can load the installed input method, which then starts its own core on the test profile.
# A core's pipe name covers its executable and profile, so this stop reaches only that core, never the user's.
function Get-OutsideCores { Get-CimInstance Win32_Process -Filter "Name='shanjie-core.exe'" | Where-Object { $_.ExecutablePath -notlike "$root*" } }
$coresBefore = @(Get-OutsideCores | ForEach-Object ProcessId)
function Stop-CoresOnProfile([string]$profileRoot) {
    $saved = $env:LOCALAPPDATA, $env:SHANJIE_TEST_PROFILE
    $env:LOCALAPPDATA = [IO.Path]::GetFullPath($profileRoot); $env:SHANJIE_TEST_PROFILE = '1'
    try {
        Get-OutsideCores | Where-Object ProcessId -notin $coresBefore | ForEach-Object ExecutablePath | Sort-Object -Unique | ForEach-Object {
            Start-Process -FilePath $_ -ArgumentList '--stop' -WindowStyle Hidden -Wait
        }
    } finally { $env:LOCALAPPDATA, $env:SHANJIE_TEST_PROFILE = $saved }
    Start-Sleep -Milliseconds 500
}
$modelOutput = Join-Path $uiRoot ('model-startup/' + [guid]::NewGuid().ToString('N'))
$modelFolder = Join-Path $modelOutput 'profile/Shanjie/Models/fixture-model'
New-Item -ItemType Directory -Path $modelFolder -Force | Out-Null
Copy-Item "$root/dist/data/models/builtin/bigram.winmap","$root/dist/data/models/builtin/classes.sjc" $modelFolder
$assets = (Get-Content "$root/MODEL-SOURCES.json" -Raw | ConvertFrom-Json).builtin.assets
$bigram = $assets | Where-Object role -eq 'bigram'; $classes = $assets | Where-Object role -eq 'classes'
@{ id = 'fixture-model'; name = 'Fixture model'; model_release = $bigram.release; classes_release = $classes.release
   model_sha256 = $bigram.sha256; classes_sha256 = $classes.sha256; model_size = $bigram.size; classes_size = $classes.size
   map_sha256 = (Get-FileHash "$modelFolder/bigram.winmap" -Algorithm SHA256).Hash.ToLowerInvariant() } | ConvertTo-Json | Set-Content "$modelFolder/model.json" -Encoding utf8NoBOM
(Get-Content "$root/profiles/windows.ini" -Raw) -replace '(?m)^model_id=.*','model_id=fixture-model' | Set-Content "$modelOutput/profile/Shanjie/profile.ini" -Encoding utf8NoBOM
try {
    foreach ($phase in 'first-open','reopen') {
        Invoke-Timed -Name "WinUI saved model startup ($phase)" -Category 'ui.model-startup' -Details @{phase=$phase;profile="$modelOutput/profile"} -Action {
            $process = Start-Process -FilePath "$root/dist/ui/Shanjie.UI.exe" -ArgumentList '--verify-model-startup',('"' + $modelOutput + '"'),$phase -WindowStyle Hidden -PassThru
            $process.WaitForExit()
            $result = Get-Content "$modelOutput/result-$phase.txt" -Raw
            if ($result -ne 'PASS') { throw "Saved model after reopening: $result" }
        }
    }
} finally {
    try { Stop-CoresOnProfile "$modelOutput/profile" } finally { if(Test-Path -LiteralPath $modelOutput){Remove-Item -LiteralPath $modelOutput -Recurse -Force} }
}
Write-Host 'Saved model kept across settings restarts.'
# Quick Setup shows once: separate processes on one isolated profile, like reopening the settings and upgrading.
$quickOutput = Join-Path $uiRoot ('quick-setup/' + [guid]::NewGuid().ToString('N'))
try {
    foreach ($phase in 'first-open','reopen','after-upgrade') {
        if ($phase -eq 'after-upgrade') { Set-Content "$quickOutput/profile/Shanjie/quick-setup.txt" 'shown 0.0.0-older' -Encoding utf8NoBOM }
        Invoke-Timed -Name "WinUI Quick Setup persistence ($phase)" -Category 'ui.quick-setup' -Details @{phase=$phase;profile="$quickOutput/profile"} -Action {
            $process = Start-Process -FilePath "$root/dist/ui/Shanjie.UI.exe" -ArgumentList '--verify-quick-setup',('"' + $quickOutput + '"'),$phase -WindowStyle Hidden -PassThru
            $process.WaitForExit()
            $result = Get-Content "$quickOutput/result-$phase.txt" -Raw
            if ($result -ne 'PASS') { throw "Quick Setup $phase`: $result" }
        }
    }
} finally {
    try { Stop-CoresOnProfile "$quickOutput/profile" } finally { if(Test-Path -LiteralPath $quickOutput){Remove-Item -LiteralPath $quickOutput -Recurse -Force} }
}
Write-Host 'Quick Setup showed once across reopening and upgrading.'
# Quick Setup's decisions, saved files, keys, demos and screenshots (each step, mid-transition and the finish), on its own profile.
$quickReview = Join-Path $uiRoot 'quick-setup-flow'
if (Test-Path $quickReview) { Remove-Item -LiteralPath $quickReview -Recurse -Force }
Invoke-Timed -Name 'WinUI Quick Setup flow' -Category 'ui.quick-setup-flow' -Details @{profile="$quickReview/profile"} -Action {
    try {
        $process = Start-Process -FilePath "$root/dist/ui/Shanjie.UI.exe" -ArgumentList '--verify-quick-setup',('"' + $quickReview + '"'),'flow' -WindowStyle Hidden -PassThru
        $process.WaitForExit()
        $result = Get-Content "$quickReview/result-flow.txt" -Raw
        if ($result -ne 'PASS') { throw "Quick Setup flow: $result" }
    } finally { Stop-CoresOnProfile "$quickReview/profile" }
}
Write-Host "Quick Setup flow checks passed: $quickReview/quick-setup"
$output = Join-Path $uiRoot 'final-review'
Invoke-Timed -Name 'WinUI profile and visual verification' -Category 'ui.verify' -Details @{profile="$output/profile"} -Action {
    try {
        $process = Start-Process -FilePath "$root/dist/ui/Shanjie.UI.exe" -ArgumentList '--verify-ui',('"' + $output + '"') -WindowStyle Hidden -PassThru
        $process.WaitForExit()
        if ((Get-Content "$output/result.txt" -Raw) -ne 'PASS') { throw 'WinUI loading, profile persistence or visual capture failed.' }
    } finally { Stop-CoresOnProfile "$output/profile" }
}
Write-Host "WinUI profile/capture checks passed: $output"
# Setup and uninstall pages, rendered offscreen with sample progress; nothing is installed.
$setupOutput = Join-Path $uiRoot 'setup-review'
Invoke-Timed -Name 'WinUI setup pages' -Category 'ui.setup-pages' -Details @{output=$setupOutput} -Action {
    $process = Start-Process -FilePath "$root/dist/ui/Shanjie.UI.exe" -ArgumentList '--verify-setup',('"' + $setupOutput + '"') -WindowStyle Hidden -PassThru
    $process.WaitForExit()
    if ((Get-Content "$setupOutput/result.txt" -Raw) -ne 'PASS') { throw 'WinUI setup pages failed.' }
}
Write-Host "WinUI setup page checks passed: $setupOutput"
