#Requires -Version 7
param([switch]$Benchmark, [string]$ResultRoot = (Join-Path ([IO.Path]::GetTempPath()) ('shanjie-ui-' + [guid]::NewGuid().ToString('N'))),
    [ValidateSet('x64','x86')][string[]]$Architectures = @('x64','x86'),[string[]]$ModelFixtures=@(),[switch]$SkipNative,[switch]$NativeOnly)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
. "$PSScriptRoot/msvc-command.ps1"
. "$PSScriptRoot/preserve-output.ps1"
New-Item -ItemType Directory -Path $ResultRoot -Force | Out-Null
if($SkipNative -and $NativeOnly){throw 'SkipNative and NativeOnly cannot be combined.'}
if(!$SkipNative){foreach ($architecture in $Architectures) {
    $command = @"
@echo off
$(Get-MsvcSetup $architecture)
if errorlevel 1 exit /b 1
cd /d "$root/build/native-$architecture"
cl /nologo /std:c++20 /EHsc /O2 /MT /W4 /utf-8 /DUNICODE /D_UNICODE /DNOMINMAX /I"$root/tests/support" /I"$root/upstream/shanjie/core/include" "$root/tests/candidate_host_smoke.cpp" "$root/tests/support/candidate_host.cpp" "$root/tests/support/engine.cpp" "$root/windows/target/$(if ($architecture -eq 'x64') {'x86_64'} else {'i686'})-pc-windows-msvc/release/shanjie_windows.lib" /link /OUT:"$root/build/candidate-host-$architecture.exe" ole32.lib user32.lib gdi32.lib d2d1.lib dwrite.lib dwmapi.lib windowsapp.lib psapi.lib advapi32.lib bcrypt.lib ntdll.lib ws2_32.lib userenv.lib
"@
    Set-Content "$root/build/native-$architecture/ui-smoke.cmd" $command -Encoding utf8NoBOM
    & cmd.exe /d /c "$root/build/native-$architecture/ui-smoke.cmd"
    if ($LASTEXITCODE) { throw 'Candidate host harness build failed.' }
    & "$root/build/candidate-host-$architecture.exe" "$root/dist/$architecture/Shanjie.dll" "$ResultRoot/native/$architecture"
    if ($LASTEXITCODE) { throw "$architecture WinUI IPC failed." }
}}
if($NativeOnly){return}
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
# Each launch must resolve this app's PRI even when this script inherits a packaged host's identity.
foreach ($phase in 'clean','reopen','different-directory') {
    $startupOutput = Join-Path $ResultRoot "startup-$phase"
    New-Item -ItemType Directory -Path "$startupOutput/profile/Shanjie" -Force | Out-Null
    if ($phase -ne 'clean') { Copy-Item "$root/profiles/windows.ini" "$startupOutput/profile/Shanjie/profile.ini" }
    $working = if ($phase -eq 'different-directory') { $ResultRoot } else { "$root/dist/ui" }
    try {
        $process = Start-Process -FilePath "$root/dist/ui/Shanjie.UI.exe" -ArgumentList '--verify-startup',('"' + $startupOutput + '"') -WorkingDirectory $working -WindowStyle Hidden -PassThru
        $process.WaitForExit()
        if ($process.ExitCode -ne 0 -or (Get-Content "$startupOutput/result.txt" -Raw) -ne 'PASS') { throw "Settings startup $phase failed: $startupOutput/result.txt" }
    } finally { Stop-CoresOnProfile "$startupOutput/profile" }
}
# Initial effective IsEnabled can already be false before controls subscribe to changes.
# Exercise a fresh process and reopening for raw Zhuyin, Pinyin and Eten configurations.
$installation = Join-Path $root 'dist/installed.json'
$savedInstallation = if (Test-Path -LiteralPath $installation) { [IO.File]::ReadAllBytes($installation) } else { $null }
try {
# Portable test metadata only; do not let the user's installed keyboard subset replace an Eten fixture.
[IO.File]::WriteAllText($installation,'{"version":"verification-fixture","keyboards":3}')
foreach ($keyboard in 1,3,2) {
    $disabledOutput = Join-Path $ResultRoot "disabled-keyboard-$keyboard"
    New-Item -ItemType Directory -Path "$disabledOutput/profile/Shanjie" -Force | Out-Null
    $fixture = Get-Content "$root/profiles/windows.ini" -Raw
    foreach ($entry in @{ keyboard=$keyboard; ranking='formal'; mixed_input=1; upstream_english=0; shift_toggle=0; add_word=0; demote=0; learning=0; space_after_commit=1 }.GetEnumerator()) {
        $fixture = $fixture -replace "(?m)^$($entry.Key)=.*", "$($entry.Key)=$($entry.Value)"
    }
    Set-Content "$disabledOutput/profile/Shanjie/profile.ini" $fixture -Encoding utf8NoBOM
    foreach ($phase in 'initial','reopen') {
        $process = Start-Process "$root/dist/ui/Shanjie.UI.exe" -ArgumentList '--verify-ui',('"' + $disabledOutput + '"'),'disabled-settings' -WindowStyle Hidden -PassThru
        $process.WaitForExit()
        if ((Get-Content "$disabledOutput/result.txt" -Raw) -ne 'PASS') { throw "Initial disabled UI ($keyboard/$phase) failed: $disabledOutput/result.txt" }
        if ($keyboard -ge 2 -and '鄰鍵打錯' -notin @(Get-Content "$disabledOutput/disabled-rows.txt")) { throw 'The nonstandard keyboard fixture fell back to Standard.' }
    }
    Stop-CoresOnProfile "$disabledOutput/profile"
}
} finally {
    if ($null -ne $savedInstallation) { [IO.File]::WriteAllBytes($installation,$savedInstallation) }
    else { Preserve-Output $installation }
}
$modelOutput = Join-Path $ResultRoot 'model-startup'
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
Stop-CoresOnProfile "$modelOutput/profile"
Write-Host 'Saved model kept across settings restarts.'
foreach($fixture in $ModelFixtures) {
    $receipt=Get-Content -LiteralPath (Join-Path $fixture 'model.json') -Raw|ConvertFrom-Json
    $id=$receipt.id
    if($id -notmatch '^bundle-[a-f0-9]{64}$'){throw 'Legacy fixture must preserve its immutable bundle ID.'}
    $legacyOutput=Join-Path $ResultRoot "legacy-$id"
    New-Item -ItemType Directory "$legacyOutput/profile/Shanjie/Models" -Force|Out-Null
    Copy-Item -LiteralPath $fixture -Destination "$legacyOutput/profile/Shanjie/Models/$id" -Recurse
    (Get-Content "$root/profiles/windows.ini" -Raw) -replace '(?m)^model_id=.*',"model_id=$id" | Set-Content "$legacyOutput/profile/Shanjie/profile.ini" -Encoding utf8NoBOM
    $savedId=$env:SHANJIE_MODEL_STARTUP_ID
    try {
        $env:SHANJIE_MODEL_STARTUP_ID=$id
        foreach($phase in 'first-open','reopen') {
            $process=Start-Process "$root/dist/ui/Shanjie.UI.exe" -ArgumentList '--verify-model-startup',('"'+$legacyOutput+'"'),$phase -WindowStyle Hidden -PassThru
            $process.WaitForExit()
            if((Get-Content "$legacyOutput/result-$phase.txt" -Raw)-ne 'PASS'){throw "Legacy model settings failed: $legacyOutput/result-$phase.txt"}
        }
        Stop-CoresOnProfile "$legacyOutput/profile"
    } finally {$env:SHANJIE_MODEL_STARTUP_ID=$savedId}
    Write-Host "Legacy model and appearance kept across settings restarts: $id"
}
# Quick Setup shows once: separate processes on one isolated profile, like reopening the settings and upgrading.
$quickOutput = Join-Path $ResultRoot 'quick-setup'
foreach ($phase in 'first-open','reopen','after-upgrade') {
    if ($phase -eq 'after-upgrade') { Set-Content "$quickOutput/profile/Shanjie/quick-setup.txt" 'shown 0.0.0-older' -Encoding utf8NoBOM }
    $process = Start-Process -FilePath "$root/dist/ui/Shanjie.UI.exe" -ArgumentList '--verify-quick-setup',('"' + $quickOutput + '"'),$phase -WindowStyle Hidden -PassThru
    $process.WaitForExit()
    $result = Get-Content "$quickOutput/result-$phase.txt" -Raw
    if ($result -ne 'PASS') { throw "Quick Setup $phase`: $result" }
}
Stop-CoresOnProfile "$quickOutput/profile"
Write-Host 'Quick Setup showed once across reopening and upgrading.'
# Quick Setup's decisions, saved files, keys, demos and screenshots (each step, mid-transition and the finish), on its own profile.
$quickReview = Join-Path $ResultRoot 'quick-setup-review'
$process = Start-Process -FilePath "$root/dist/ui/Shanjie.UI.exe" -ArgumentList '--verify-quick-setup',('"' + $quickReview + '"'),'flow' -WindowStyle Hidden -PassThru
$process.WaitForExit()
$result = Get-Content "$quickReview/result-flow.txt" -Raw
Stop-CoresOnProfile "$quickReview/profile"
if ($result -ne 'PASS') { throw "Quick Setup flow: $result" }
Write-Host "Quick Setup flow checks passed: $quickReview/quick-setup"
$output = Join-Path $ResultRoot 'final'
$process = Start-Process -FilePath "$root/dist/ui/Shanjie.UI.exe" -ArgumentList '--verify-ui',('"' + $output + '"') -WindowStyle Hidden -PassThru
$process.WaitForExit()
if ((Get-Content "$output/result.txt" -Raw) -ne 'PASS') { throw 'WinUI loading, profile persistence or visual capture failed.' }
Stop-CoresOnProfile "$output/profile"
Write-Host "WinUI profile/capture checks passed: $output"
# Setup and uninstall pages, rendered offscreen with sample progress; nothing is installed.
$setupOutput = Join-Path $ResultRoot 'setup'
$process = Start-Process -FilePath "$root/dist/ui/Shanjie.UI.exe" -ArgumentList '--verify-setup',('"' + $setupOutput + '"') -WindowStyle Hidden -PassThru
$process.WaitForExit()
if ((Get-Content "$setupOutput/result.txt" -Raw) -ne 'PASS') { throw 'WinUI setup pages failed.' }
Write-Host "WinUI setup page checks passed: $setupOutput"
