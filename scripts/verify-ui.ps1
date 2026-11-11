#Requires -Version 7
param([switch]$Benchmark,[Alias('Architectures')][ValidateSet('x64','x86')][string[]]$Architecture=@('x64','x86'),
    [Alias('NativeOnly')][switch]$CandidateHostOnly,[Alias('SkipNative')][switch]$SkipCandidateHost,[string]$HarnessPath,[Alias('ResultRoot')][string]$OutputRoot,
    [ValidateSet('all','general','startup','disabled')][string]$Scenario='all',[string[]]$Phases,[ValidateSet(1,2,3)][int[]]$Keyboards=@(1,3,2),[string[]]$ModelFixtures=@(),[switch]$Visual)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
Import-Module "$PSScriptRoot/test-support.psm1" -ErrorAction Stop
. "$PSScriptRoot/msvc-command.ps1"
$null=Initialize-TestTimings -ProjectRoot $root -Name 'verify-ui'
$buildRoot=[IO.Path]::GetFullPath((Join-Path $root 'build'))
$uiRoot=if($OutputRoot){[IO.Path]::GetFullPath($OutputRoot)}else{Join-Path $buildRoot ('ui-verification-'+[guid]::NewGuid().ToString('N'))}
if(!$uiRoot.StartsWith($buildRoot+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)){throw "Refusing a UI fixture outside build/: $uiRoot"}
$null=New-Item -ItemType Directory -Path $uiRoot -Force
if($SkipCandidateHost -and $CandidateHostOnly){throw 'SkipNative and NativeOnly cannot be combined.'}
$previousVisual=$env:SHANJIE_VERIFY_VISUAL
$env:SHANJIE_VERIFY_VISUAL=if($Visual){'1'}else{$null}
try {
if (!$SkipCandidateHost) { foreach ($architecture in $Architecture) {
    $candidateHarness = if ($HarnessPath) { [IO.Path]::GetFullPath($HarnessPath) } else { "${root}/build/candidate-host-$architecture.exe" }
    if ($HarnessPath -and $Architecture.Count -ne 1) { throw 'A prebuilt candidate-host harness can only be used for one architecture per invocation.' }
    if ($HarnessPath -and !(Test-Path -LiteralPath $candidateHarness)) { throw "Candidate-host harness is missing: $candidateHarness" }
    if (!$HarnessPath) {
    $command = @"
@echo off
$(Get-MsvcSetup $architecture)
if errorlevel 1 exit /b 1
cd /d "$root/build/native-$architecture"
cl /nologo /std:c++20 /EHsc /O2 /MT /W4 /utf-8 /DUNICODE /D_UNICODE /DNOMINMAX /I"$root/tests/support" /I"$root/upstream/shanjie/core/include" "$root/tests/candidate_host_smoke.cpp" "$root/tests/support/candidate_host.cpp" "$root/tests/support/engine.cpp" "$root/windows/target/$(if ($architecture -eq 'x64') {'x86_64'} else {'i686'})-pc-windows-msvc/release/shanjie_windows.lib" /link /OUT:"$root/build/candidate-host-$architecture.exe" ole32.lib user32.lib gdi32.lib d2d1.lib dwrite.lib dwmapi.lib windowsapp.lib psapi.lib advapi32.lib bcrypt.lib ntdll.lib ws2_32.lib userenv.lib
"@
    Set-Content "$root/build/native-$architecture/ui-smoke.cmd" $command -Encoding utf8NoBOM
    Invoke-Timed -Name "Compile candidate-host harness ($architecture)" -Category 'build.cpp.harness' -Details @{architecture=$architecture;source='tests/candidate_host_smoke.cpp'} -Action {
        Invoke-HarnessBuild $command "$root/build/native-$architecture/ui-smoke.cmd" $candidateHarness
        if ($LASTEXITCODE) { throw 'Candidate host harness build failed.' }
    }
    }
    Invoke-Timed -Name "Candidate-host WinUI IPC ($architecture)" -Category 'ui.candidate-host' -Details @{architecture=$architecture} -Action {
        & $candidateHarness "$root/dist/$architecture/Shanjie.dll" "$uiRoot/native/$architecture"
        if ($LASTEXITCODE) { throw "$architecture WinUI IPC failed." }
    }
} }
if ($CandidateHostOnly) { return }
# A saved downloaded model survives closing and reopening the settings: separate processes, isolated profile, a package the scan really validates.
# Set isolation before XAML starts: the OS may load an installed IME before
# OnLaunched sets its profile. Keep that IME's core in this process as well.
function Invoke-UiFixture([string[]]$Arguments,[string]$WorkingDirectory="$root/dist/ui",[string]$ModelId) {
    $start=[Diagnostics.ProcessStartInfo]::new("$root/dist/ui/Shanjie.UI.exe")
    $start.UseShellExecute=$false;$start.CreateNoWindow=$true;$start.WindowStyle=[Diagnostics.ProcessWindowStyle]::Hidden
    $profile=[IO.Path]::Combine($Arguments[1],'profile')
    $null=New-Item -ItemType Directory -Path $profile -Force
    $start.Environment['LOCALAPPDATA']=$profile
    $start.Environment['SHANJIE_TEST_PROFILE']='1'
    $start.Environment['SHANJIE_TEST_ROOT']=$profile
    $start.Environment['SHANJIE_CORE_LOCAL']='1'
    $start.WorkingDirectory=$WorkingDirectory
    $start.Environment.Remove('SHANJIE_MODEL_STARTUP_ID')|Out-Null
    if($ModelId){$start.Environment['SHANJIE_MODEL_STARTUP_ID']=$ModelId}
    foreach($argument in $Arguments){$start.ArgumentList.Add($argument)}
    $process=[Diagnostics.Process]::Start($start)
    try {
        if(!$process.WaitForExit(180000)){$process.Kill($true);$process.WaitForExit(5000)|Out-Null;throw "WinUI fixture timed out: $($Arguments[0])"}
        if($process.ExitCode){throw "WinUI fixture failed: $($Arguments[0]), exit $($process.ExitCode)"}
    } finally {$process.Dispose()}
}
if($Scenario -in 'all','startup'){
    $startupPhases=if($Phases){$Phases}else{@('clean','reopen','different-directory')}
    foreach($phase in $startupPhases){
        if($phase -notin 'clean','reopen','different-directory'){throw "Unknown startup phase: $phase"}
        $startupOutput=Join-Path $uiRoot "startup-$phase"
        $null=New-Item -ItemType Directory -Path "$startupOutput/profile/Shanjie" -Force
        if($phase -ne 'clean'){Copy-Item "$root/profiles/windows.ini" "$startupOutput/profile/Shanjie/profile.ini"}
        $working=if($phase -eq 'different-directory'){$uiRoot}else{"$root/dist/ui"}
        Invoke-Timed -Name "WinUI startup ($phase)" -Category 'ui.startup' -Details @{architecture='x64';phase=$phase} -Action {
            Invoke-UiFixture -Arguments @('--verify-startup',$startupOutput) -WorkingDirectory $working
            if((Get-Content "$startupOutput/result.txt" -Raw) -ne 'PASS'){throw "Settings startup $phase failed."}
        }
    }
    if($Scenario -eq 'startup'){return}
}
if($Scenario -in 'all','disabled'){
    $installation=Join-Path $root 'dist/installed.json'
    $savedInstallation=if(Test-Path -LiteralPath $installation){[IO.File]::ReadAllBytes($installation)}else{$null}
    try {
        # Only generated dist metadata changes; the interactive-ui CTest lock serializes these fixtures.
        [IO.File]::WriteAllText($installation,'{"version":"verification-fixture","keyboards":3}')
        foreach($keyboard in $Keyboards){
            $disabledOutput=Join-Path $uiRoot "disabled-keyboard-$keyboard"
            $null=New-Item -ItemType Directory -Path "$disabledOutput/profile/Shanjie" -Force
            $fixture=Get-Content "$root/profiles/windows.ini" -Raw
            foreach($entry in @{keyboard=$keyboard;ranking='formal';mixed_input=1;upstream_english=0;shift_toggle=0;add_word=0;demote=0;learning=0;space_after_commit=1}.GetEnumerator()){
                $fixture=$fixture -replace "(?m)^$($entry.Key)=.*","$($entry.Key)=$($entry.Value)"
            }
            Set-Content "$disabledOutput/profile/Shanjie/profile.ini" $fixture -Encoding utf8NoBOM
            foreach($phase in 'initial','reopen'){
                Invoke-Timed -Name "WinUI disabled controls ($keyboard/$phase)" -Category 'ui.disabled' -Details @{architecture='x64';keyboard=$keyboard;phase=$phase} -Action {
                    . "$PSScriptRoot/preserve-output.ps1"
                    Preserve-Output "$disabledOutput/result.txt"
                    Preserve-Output "$disabledOutput/disabled-rows.txt"
                    Invoke-UiFixture -Arguments @('--verify-ui',$disabledOutput,'disabled-settings')
                    if((Get-Content "$disabledOutput/result.txt" -Raw) -ne 'PASS'){throw "Initial disabled UI ($keyboard/$phase) failed."}
                    if($keyboard -ge 2 -and '鄰鍵打錯' -notin @(Get-Content "$disabledOutput/disabled-rows.txt")){throw 'The nonstandard keyboard fixture fell back to Standard.'}
                }
            }
        }
    } finally {
        if($null -ne $savedInstallation){[IO.File]::WriteAllBytes($installation,$savedInstallation)}
        else{. "$PSScriptRoot/preserve-output.ps1";Preserve-Output $installation}
    }
    if($Scenario -eq 'disabled'){return}
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
foreach ($phase in 'first-open','reopen') {
    Invoke-Timed -Name "WinUI saved model startup ($phase)" -Category 'ui.model-startup' -Details @{phase=$phase;profile="$modelOutput/profile"} -Action {
        Invoke-UiFixture @('--verify-model-startup',$modelOutput,$phase)
        $result = Get-Content "$modelOutput/result-$phase.txt" -Raw
        if ($result -ne 'PASS') { throw "Saved model after reopening: $result" }
    }
}
Write-Host 'Saved model kept across settings restarts.'
foreach($fixture in $ModelFixtures){
    $receipt=Get-Content -LiteralPath (Join-Path $fixture 'model.json') -Raw|ConvertFrom-Json
    $id=$receipt.id
    if($id -notmatch '^bundle-[a-f0-9]{64}$'){throw 'Legacy fixture must preserve its immutable bundle ID.'}
    $legacyOutput=Join-Path $uiRoot "legacy-$id"
    $null=New-Item -ItemType Directory -Path "$legacyOutput/profile/Shanjie/Models" -Force
    Copy-Item -LiteralPath $fixture -Destination "$legacyOutput/profile/Shanjie/Models/$id" -Recurse
    (Get-Content "$root/profiles/windows.ini" -Raw) -replace '(?m)^model_id=.*',"model_id=$id"|Set-Content "$legacyOutput/profile/Shanjie/profile.ini" -Encoding utf8NoBOM
    foreach($phase in 'first-open','reopen'){
        Invoke-Timed -Name "WinUI legacy model ($id/$phase)" -Category 'ui.model-startup' -Details @{phase=$phase;model_id=$id} -Action {
            Invoke-UiFixture -Arguments @('--verify-model-startup',$legacyOutput,$phase) -ModelId $id
            if((Get-Content "$legacyOutput/result-$phase.txt" -Raw) -ne 'PASS'){throw "Legacy model settings failed: $id/$phase"}
        }
    }
}
# Quick Setup shows once: separate processes on one isolated profile, like reopening the settings and upgrading.
$quickOutput = Join-Path $uiRoot ('quick-setup/' + [guid]::NewGuid().ToString('N'))
foreach ($phase in 'first-open','reopen','after-upgrade') {
    if ($phase -eq 'after-upgrade') { Set-Content "$quickOutput/profile/Shanjie/quick-setup.txt" 'shown 0.0.0-older' -Encoding utf8NoBOM }
    Invoke-Timed -Name "WinUI Quick Setup persistence ($phase)" -Category 'ui.quick-setup' -Details @{phase=$phase;profile="$quickOutput/profile"} -Action {
        Invoke-UiFixture @('--verify-quick-setup',$quickOutput,$phase)
        $result = Get-Content "$quickOutput/result-$phase.txt" -Raw
        if ($result -ne 'PASS') { throw "Quick Setup $phase`: $result" }
    }
}
Write-Host 'Quick Setup showed once across reopening and upgrading.'
# Quick Setup's decisions, saved files, keys, demos and screenshots (each step, mid-transition and the finish), on its own profile.
$quickReview = Join-Path $uiRoot 'quick-setup-flow'
Invoke-Timed -Name 'WinUI Quick Setup flow' -Category 'ui.quick-setup-flow' -Details @{profile="$quickReview/profile"} -Action {
    Invoke-UiFixture @('--verify-quick-setup',$quickReview,'flow')
    $result = Get-Content "$quickReview/result-flow.txt" -Raw
    if ($result -ne 'PASS') { throw "Quick Setup flow: $result" }
}
Write-Host "Quick Setup flow checks passed: $quickReview/quick-setup"
$output = Join-Path $uiRoot 'final-review'
Invoke-Timed -Name 'WinUI profile and visual verification' -Category 'ui.verify' -Details @{profile="$output/profile"} -Action {
    Invoke-UiFixture @('--verify-ui',$output)
    if ((Get-Content "$output/result.txt" -Raw) -ne 'PASS') { throw 'WinUI loading, profile persistence or visual capture failed.' }
}
Write-Host "WinUI profile/capture checks passed: $output"
# Setup and uninstall pages, rendered offscreen with sample progress; nothing is installed.
$setupOutput = Join-Path $uiRoot 'setup-review'
Invoke-Timed -Name 'WinUI setup pages' -Category 'ui.setup-pages' -Details @{output=$setupOutput} -Action {
    Invoke-UiFixture @('--verify-setup',$setupOutput)
    if ((Get-Content "$setupOutput/result.txt" -Raw) -ne 'PASS') { throw 'WinUI setup pages failed.' }
}
Write-Host "WinUI setup page checks passed: $setupOutput"
} finally {$env:SHANJIE_VERIFY_VISUAL=$previousVisual}
