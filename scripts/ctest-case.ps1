#Requires -Version 7
param(
    [Parameter(Mandatory)][ValidateSet('infrastructure','upstream-data','prepare-core','shared-unit','windows-unit','core-suite','mixed-regression','dist-data','runtime-payload','tsf-smoke-standard','tsf-smoke-cold','add-word','english-keys','submission','private-tsf','private-popup','candidate-host','mode-smoke','interaction','ui-verify','ui-startup','ui-disabled','ui-models','package')][string]$CaseId,
    [ValidateSet('x64','x86')][string]$Architecture='x64',
    [ValidateSet('windows','shanjie')][string]$Interaction='windows',
    [string]$Scenario,
    [string]$Mode,
    [int[]]$Keyboards=@(1,2),
    [string]$HarnessPath,
    [string]$PayloadRoot,
    [string]$TestName
)
$ErrorActionPreference = 'Stop'
$env:SHANJIE_CTEST_TEST_NAME=$TestName
$root = Split-Path $PSScriptRoot -Parent
Import-Module "$PSScriptRoot/test-support.psm1" -ErrorAction Stop
$timingDirectory = Initialize-TestTimings -ProjectRoot $root -Name "ctest-$CaseId"
$runId = if ($env:SHANJIE_TEST_RUN_ID) { $env:SHANJIE_TEST_RUN_ID } else { [guid]::NewGuid().ToString('N') }
$caseToken = (($CaseId,$Architecture,$Scenario,$Mode) -join '-') -replace '[^A-Za-z0-9_.-]','_'
$resultRoot = Join-Path $root "build/reports/ctest-$runId/cases/$caseToken-$([guid]::NewGuid().ToString('N').Substring(0,8))"
New-Item -ItemType Directory -Path $resultRoot -Force | Out-Null
$stage = Start-TestStage -Name "CTest $CaseId $Scenario $Mode" -Category "ctest.$CaseId"
$previousParent=$env:SHANJIE_TEST_PARENT_STAGE_ID
$env:SHANJIE_TEST_PARENT_STAGE_ID=$stage.Id
$status = 'passed'
$failure = $null
$snapshot=$null
$runtimeRoot=Join-Path $root 'dist'
$uiRuntimeRoot=$null
try {
    if($env:SHANJIE_RELEASE_SNAPSHOT_ID){
        Import-Module "$PSScriptRoot/release-support.psm1" -ErrorAction Stop
        $snapshot=Get-ReleaseSnapshot $root $env:SHANJIE_RELEASE_SNAPSHOT_ID $env:SHANJIE_RELEASE_SNAPSHOT_SHA256
        $runtimeRoot=$snapshot.runtime_root
        if($PayloadRoot){$PayloadRoot="$PayloadRoot-$($snapshot.id)"}
        if($CaseId -in 'ui-verify','ui-startup','ui-disabled','ui-models'){
            # WinUI temporarily changes installed.json; only a private fixture may
            # be mutable. Its DLL/tools/UI bytes are checked against the seal.
            $uiRuntimeRoot=Join-Path $resultRoot 'runtime'
            foreach($name in 'x64','x86','ui','tools'){Copy-ReleaseFiles "$runtimeRoot/$name" "$uiRuntimeRoot/$name"}
            $null=New-Item -ItemType Junction -Path "$uiRuntimeRoot/data" -Target "$runtimeRoot/data"
            Assert-ReleasePayload $snapshot $uiRuntimeRoot @('x64','x86','ui','tools')
        }
    }
    $uiArguments=@{RuntimeRoot=if($uiRuntimeRoot){$uiRuntimeRoot}else{$runtimeRoot}}
    Push-Location $root
    try {
        switch ($CaseId) {
            'infrastructure' { & "$root/tests/test_infrastructure.ps1" -ResultRoot $resultRoot }
            'upstream-data' { & "$PSScriptRoot/verify-upstream-data.ps1" }
            'prepare-core' { & "$PSScriptRoot/prepare-core.ps1" }
            'shared-unit' {
                Invoke-Timed -Name 'Shared Rust unit tests' -Category 'rust.unit.shared' -Details @{target='x86_64-pc-windows-msvc'} -Action {
                    cargo test --manifest-path shared/Cargo.toml --lib --target x86_64-pc-windows-msvc
                    if ($LASTEXITCODE) { throw 'Shared Rust unit tests failed.' }
                }
            }
            'windows-unit' {
                $target = if ($Architecture -eq 'x64') { 'x86_64-pc-windows-msvc' } else { 'i686-pc-windows-msvc' }
                # Installer journal and progress tests share a process-wide sink.
                Invoke-Timed -Name "Windows Rust unit tests ($Architecture)" -Category 'rust.unit.windows' -Details @{target=$target;test_threads=1} -Action {
                    cargo test --manifest-path windows/Cargo.toml --lib --target $target -- --test-threads=1
                    if ($LASTEXITCODE) { throw "Windows Rust unit tests failed for $Architecture." }
                }
            }
            'core-suite' { & "$PSScriptRoot/test-core.ps1" -UseExistingCore -UseExistingModels -RuntimeRoot $runtimeRoot -Rerun:($env:SHANJIE_TEST_LEVEL -ne 'AFFECTED') }
            'mixed-regression' { & "$PSScriptRoot/test-mixed.ps1" -Architecture $Architecture -DataRoot "$runtimeRoot/data" }
            'dist-data' { & "$PSScriptRoot/verify-upstream-data.ps1" -Dist -RuntimeRoot $runtimeRoot }
            'runtime-payload' {
                if (!$PayloadRoot) { throw 'CTest runtime payload path was not configured.' }
                & "$PSScriptRoot/prepare-test-payload.ps1" -Architecture $Architecture -PayloadRoot $PayloadRoot -RuntimeRoot $runtimeRoot
            }
            'tsf-smoke-standard' {
                & "$PSScriptRoot/verify.ps1" -SmokeOnly -NativeOnly -SmokeCase standard -Interaction $Interaction -Architecture $Architecture -TsfHarnessPath $HarnessPath -ResultRoot $resultRoot -PayloadRoot $PayloadRoot
            }
            'tsf-smoke-cold' {
                & "$PSScriptRoot/verify.ps1" -SmokeOnly -NativeOnly -SmokeCase cold -Interaction windows -Architecture $Architecture -TsfHarnessPath $HarnessPath -ResultRoot $resultRoot -PayloadRoot $PayloadRoot
            }
            'add-word' {
                & "$PSScriptRoot/verify-add-word.ps1" -Architectures $Architecture -ResultRoot $resultRoot -HarnessPath $HarnessPath -PayloadRoot $PayloadRoot
            }
            'english-keys' {
                if (!$Scenario) { throw 'English-key CTest rows require a scenario name.' }
                & "$PSScriptRoot/verify-english-keys.ps1" -Architectures $Architecture -Scenarios $Scenario -ResultRoot $resultRoot -HarnessPath $HarnessPath -PayloadRoot $PayloadRoot
            }
            'submission' {
                if (!$Mode) { throw 'Submission CTest rows require a mode name.' }
                & "$PSScriptRoot/verify-submission.ps1" -Architectures $Architecture -Modes $Mode -Keyboards $Keyboards -ResultRoot $resultRoot -HarnessPath $HarnessPath -PayloadRoot $PayloadRoot
            }
            'private-tsf' {
                if (!$Scenario -or !$HarnessPath -or !$PayloadRoot) { throw 'Private TSF test row is missing its case, harness, or payload.' }
                & "$PSScriptRoot/verify-private-desktop.ps1" -Payload $PayloadRoot -Harness $HarnessPath -Output (Join-Path $resultRoot 'case') -Cases $Scenario
            }
            'private-popup' {
                if (!$Scenario -or !$HarnessPath -or !$PayloadRoot) { throw 'Private popup test row is missing its case, harness, or payload.' }
                & "$PSScriptRoot/verify-private-desktop.ps1" -Payload $PayloadRoot -Harness $HarnessPath -Output (Join-Path $resultRoot 'case') -Cases $Scenario
            }
            'mode-smoke' {
                if (!$HarnessPath) { throw 'Mode smoke CTest row requires its CMake-built executable.' }
                & "$PSScriptRoot/verify-mode.ps1" -RuntimeRoot $runtimeRoot -Architectures $Architecture -HarnessPath $HarnessPath -CaptureRoot (Join-Path $resultRoot 'capture')
            }
            'interaction' {
                if (!$HarnessPath) { throw 'Interaction CTest row requires its CMake-built executable.' }
                & "$PSScriptRoot/verify-interaction.ps1" -RuntimeRoot $runtimeRoot -HarnessPath $HarnessPath -ResultRoot (Join-Path $resultRoot 'fixture')
            }
            'candidate-host' {
                if (!$HarnessPath) { throw 'Candidate-host CTest row requires its CMake-built executable.' }
                & "$PSScriptRoot/verify-ui.ps1" @uiArguments -Architecture $Architecture -CandidateHostOnly -HarnessPath $HarnessPath
            }
            'ui-verify' { & "$PSScriptRoot/verify-ui.ps1" @uiArguments -SkipCandidateHost -Scenario general -OutputRoot (Join-Path $resultRoot 'ui') }
            'ui-startup' { & "$PSScriptRoot/verify-ui.ps1" @uiArguments -SkipCandidateHost -Scenario startup -Phases $Scenario -OutputRoot (Join-Path $resultRoot 'ui') }
            'ui-disabled' { & "$PSScriptRoot/verify-ui.ps1" @uiArguments -SkipCandidateHost -Scenario disabled -Keyboards $Keyboards -OutputRoot (Join-Path $resultRoot 'ui') }
            'package' {
                if(!$snapshot){throw 'RELEASE packaging requires the sealed snapshot and both architectures test evidence; use package.ps1 for a development candidate.'}
                & "$PSScriptRoot/package.ps1" -UseExistingBuild -CandidateId $runId -SnapshotHash $snapshot.sha256 -PackagingPermitPath $env:SHANJIE_RELEASE_PACKAGING_PERMIT
            }
            'ui-models' {
                $fixtures=Join-Path $resultRoot 'fixtures'
                & python "$root/tests/model_sources_fixture.py" $fixtures
                if($LASTEXITCODE){throw 'Model-source fixture preparation failed.'}
                $output=Join-Path $resultRoot 'ui-models'
                $profile=Join-Path $output 'profile'
                $null=New-Item -ItemType Directory -Path $profile -Force
                $start=[Diagnostics.ProcessStartInfo]::new("$($uiArguments.RuntimeRoot)/ui/Shanjie.UI.exe")
                $start.UseShellExecute=$false;$start.CreateNoWindow=$true;$start.WindowStyle=[Diagnostics.ProcessWindowStyle]::Hidden
                $start.WorkingDirectory="$($uiArguments.RuntimeRoot)/ui"
                $start.Environment['LOCALAPPDATA']=$profile
                $start.Environment['SHANJIE_TEST_PROFILE']='1'
                $start.Environment['SHANJIE_TEST_ROOT']=$profile
                $start.Environment['SHANJIE_CORE_LOCAL']='1'
                foreach($argument in @('--verify-model-v2',$output,$fixtures)){$start.ArgumentList.Add($argument)}
                $process=[Diagnostics.Process]::Start($start)
                try {
                    if(!$process.WaitForExit(180000)){$process.Kill($true);$process.WaitForExit();throw 'Model-manager UI verification timed out.'}
                    if($process.ExitCode -or (Get-Content "$output/result.txt" -Raw) -ne 'PASS'){throw "Model-manager verification failed: $output/result.txt"}
                } finally {$process.Dispose()}
            }
        }
    } finally { Pop-Location }
    if($uiRuntimeRoot){Assert-ReleasePayload $snapshot $uiRuntimeRoot @('x64','x86','ui','tools')}
} catch {
    $status = 'failed'
    $failure = $_.Exception.Message
    throw
} finally {
    $env:SHANJIE_TEST_PARENT_STAGE_ID=$previousParent
    Complete-TestStage -Stage $stage -Status $status -Details @{ architecture=$Architecture; scenario=$Scenario; mode=$Mode; result_root=$resultRoot } -ErrorMessage $failure
}
