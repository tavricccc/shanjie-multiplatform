#Requires -Version 7
param(
    [switch]$NativeOnly,
    [switch]$UseExistingBuild,
    [switch]$Visual,
    [switch]$CoreOnly,
    [switch]$SmokeOnly,
    [ValidateSet('windows','shanjie')][string]$Interaction = 'windows',
    [ValidateSet('x64','x86')][string]$Architecture = 'x64',
    [ValidateSet('all','standard','cold')][string]$SmokeCase = 'all',
    [string[]]$CoreTests,
    [string]$TsfHarnessPath,
    [string]$ResultRoot,
    [string]$PayloadRoot
)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$runtimeRoot = if ($PayloadRoot) { [IO.Path]::GetFullPath($PayloadRoot) } else { [IO.Path]::GetFullPath((Join-Path $root 'dist')) }
Import-Module "$PSScriptRoot/test-support.psm1" -ErrorAction Stop
. "$PSScriptRoot/msvc-command.ps1"
$hadTimingDirectory = !!$env:SHANJIE_TEST_TIMINGS_DIR
$timingDirectory = Initialize-TestTimings -ProjectRoot $root -Name 'verify'
$whole = [Diagnostics.Stopwatch]::StartNew()
Push-Location $root
try {
    if (!$SmokeOnly) { Invoke-Timed -Name 'Test framework regressions' -Category 'test.infrastructure' -Action { & "$root/tests/test_infrastructure.ps1" } }
    if (!$SmokeOnly) { Invoke-Timed -Name 'Static upstream data coverage' -Category 'data.validate' -Action { & "$PSScriptRoot/verify-upstream-data.ps1" } }
    if (!$SmokeOnly -and !$NativeOnly) {
        if (!$UseExistingBuild) { Invoke-Timed -Name 'Windows product build' -Category 'build.product' -Details @{ architecture=$Architecture } -Action { & "$PSScriptRoot/build.ps1" -Architecture $Architecture } }
        Invoke-Timed -Name 'Shared Rust unit tests' -Category 'rust.unit.shared' -Details @{ target='x86_64-pc-windows-msvc' } -Action {
            & cargo test --manifest-path shared/Cargo.toml --lib --target x86_64-pc-windows-msvc
            if ($LASTEXITCODE) { throw 'Shared Rust unit tests failed.' }
        }
        Invoke-Timed -Name 'Windows Rust unit tests' -Category 'rust.unit.windows' -Details @{ target='x86_64-pc-windows-msvc';test_threads=1 } -Action {
            & cargo test --manifest-path windows/Cargo.toml --lib --target x86_64-pc-windows-msvc -- --test-threads=1
            if ($LASTEXITCODE) { throw 'Windows Rust unit tests failed.' }
        }
        Invoke-Timed -Name 'Upstream core regression suite' -Category 'core.suite' -Details @{ architecture=$Architecture } -Action {
            & "$PSScriptRoot/test-core.ps1" -Tests $CoreTests -UseExistingCore -UseExistingModels
        }
    }
    if ($CoreOnly) { return }
    if (!(Test-Path -LiteralPath 'dist/data/models/builtin/bigram.winmap')) { throw 'Run prepare-models.ps1 first to prepare the built-in model set.' }
    if (!$SmokeOnly) {
        Invoke-Timed -Name 'Validate built upstream data' -Category 'data.validate' -Details @{ output='dist/data' } -Action { & "$PSScriptRoot/verify-upstream-data.ps1" -Dist }
        # Unit tests that read built data and models; plain cargo test --lib intentionally ignores these.
        if($Architecture -eq 'x64'){& "$PSScriptRoot/test-mixed.ps1" -Architecture x64}
    }

    $harness = if ($TsfHarnessPath) { [IO.Path]::GetFullPath($TsfHarnessPath) } else { Join-Path $root "build/tsf-smoke-$Architecture.exe" }
    if ($TsfHarnessPath) {
        if (!(Test-Path -LiteralPath $harness)) { throw "CMake-built TSF smoke harness is missing: $harness" }
    } else {
        $command = @"
@echo off
$(Get-MsvcSetup $Architecture)
if errorlevel 1 exit /b 1
cd /d "$root/build/native-$Architecture"
cl /nologo /std:c++20 /EHsc /O2 /MT /W4 /utf-8 /DUNICODE /D_UNICODE /DNOMINMAX /I"$root/tests/support" "$root/tests/tsf_smoke.cpp" /link /OUT:"$harness" ole32.lib oleaut32.lib uuid.lib user32.lib gdi32.lib
"@
        Set-Content "build/native-$Architecture/verify.cmd" $command -Encoding utf8NoBOM
        Invoke-Timed -Name 'Compile TSF smoke harness' -Category 'build.cpp.harness' -Details @{ architecture=$Architecture; source='tests/tsf_smoke.cpp' } -Action {
            Invoke-HarnessBuild $command (Join-Path $root "build/native-$Architecture/verify.cmd") $harness
            if ($LASTEXITCODE) { throw 'TSF smoke harness build failed.' }
        }
    }

    if ($SmokeCase -in @('all','standard')) {
        $profileRoot = if ($ResultRoot) { Join-Path ([IO.Path]::GetFullPath($ResultRoot)) "standard-$Interaction" } else { Join-Path $root ("build/profile-tests/$Architecture/$Interaction/" + [guid]::NewGuid().ToString('N')) }
        New-Item -ItemType Directory -Path "$profileRoot/Shanjie" -Force | Out-Null
        Copy-Item "profiles/$Interaction.ini" "$profileRoot/Shanjie/profile.ini" -Force
        # Force Standard and the chat profile (this harness is not a chat program); the desktop profile and learning stay untouched.
        $profileText = (Get-Content "$profileRoot/Shanjie/profile.ini" -Raw) -replace '(?m)^keyboard=.*','keyboard=1' -replace '(?m)^ranking=.*','ranking=chat'
        if ($Interaction -eq 'windows') { $profileText = $profileText -replace '(?m)^upstream_english=.*','upstream_english=0' -replace '(?m)^right_action=.*','right_action=none' }
        Set-Content "$profileRoot/Shanjie/profile.ini" $profileText -Encoding utf8NoBOM
        $previousLocalAppData = $env:LOCALAPPDATA
        $previousHeadless = $env:SHANJIE_HEADLESS
        $previousTestProfile = $env:SHANJIE_TEST_PROFILE
        $fixtureCore = $null
        try {
            $env:LOCALAPPDATA = $profileRoot
            $env:SHANJIE_TEST_PROFILE = '1'
            $env:SHANJIE_HEADLESS = if ($Visual) { $null } else { '1' }
            $start = Start-TestStage -Name 'Start isolated core process' -Category 'core.lifecycle'
            try{$fixtureCore = Start-Process "$runtimeRoot/tools/shanjie-core.exe" -ArgumentList '--serve' -WindowStyle Hidden -PassThru;Complete-TestStage -Stage $start -Details @{ pid=$fixtureCore.Id; architecture=$Architecture; profile=$profileRoot }}
            catch{Complete-TestStage -Stage $start -Status 'failed' -Details @{ architecture=$Architecture; profile=$profileRoot } -ErrorMessage $_.Exception.Message;throw}
            Wait-TestCoreReady -CorePath "$runtimeRoot/tools/shanjie-core.exe" -OutputDirectory $profileRoot | Out-Null

            $arguments = @("$runtimeRoot/$Architecture/Shanjie.dll")
            if ($Visual) { $arguments += '--visual' }
            elseif ($Interaction -eq 'shanjie') { $arguments += '--shanjie' }
            Invoke-Timed -Name 'Run TSF smoke regression' -Category 'tsf.smoke' -Details @{ architecture=$Architecture; interaction=$Interaction } -Action {
                & $harness @arguments
                if ($LASTEXITCODE) { throw 'TSF smoke harness failed.' }
            }
            if(!$Visual){
                # CTest's isolated payload already enables both layouts; the legacy
                # dist entry uses a temporary fixture without changing registration.
                $installation=Join-Path $runtimeRoot 'installed.json'
                $savedInstallation=if(!$PayloadRoot -and (Test-Path -LiteralPath $installation)){[IO.File]::ReadAllBytes($installation)}else{$null}
                try {
                    if(!$PayloadRoot){[IO.File]::WriteAllText($installation,'{"version":"verification-fixture","keyboards":3}')}
                    Invoke-Timed -Name 'Run pure Zhuyin TSF regression' -Category 'tsf.zhuyin-only' -Details @{architecture=$Architecture} -Action {
                        & $harness "$runtimeRoot/$Architecture/Shanjie.dll" --zhuyin-only
                        if($LASTEXITCODE){throw 'Pure Zhuyin TSF harness failed.'}
                    }
                } finally {
                    if(!$PayloadRoot){
                        if($null -ne $savedInstallation){[IO.File]::WriteAllBytes($installation,$savedInstallation)}else{Remove-Item -LiteralPath $installation}
                    }
                }
            }
        } finally {
            try {
                if ($fixtureCore) {
                    $stopStage = Start-TestStage -Name 'Stop isolated core process' -Category 'core.lifecycle'
                    $stop=$null;$stopExited=$false;$stopCode=$null;$stopError=$null
                    try{$stop=Start-Process -FilePath "$runtimeRoot/tools/shanjie-core.exe" -ArgumentList '--stop' -WindowStyle Hidden -PassThru;$stopExited=$stop.WaitForExit(5000);$stopCode=if($stopExited){$stop.ExitCode}else{$null}}
                    catch{$stopError=$_.Exception.Message}
                    if($stop -and !$stopExited){try{$stop.Kill();$stop.WaitForExit(5000)|Out-Null}catch{}}
                    $fixtureExited=$fixtureCore.WaitForExit(5000)
                    if(!$fixtureExited){try{$fixtureCore.Kill();$fixtureCore.WaitForExit(5000)|Out-Null}catch{}}
                    if ($stopError -or !$stopExited -or $stopCode -ne 0 -or !$fixtureExited) { $message='Core stop command failed or timed out.';Complete-TestStage -Stage $stopStage -Status 'failed' -Details @{ pid=$fixtureCore.Id; stop_exit_code=$stopCode;worker_force_stopped=(!$fixtureExited) } -ErrorMessage ($stopError ?? $message); throw 'Isolated core stop failed.' }
                    Complete-TestStage -Stage $stopStage -Details @{ pid=$fixtureCore.Id; stop_exit_code=$stopCode }
                }
            } finally {
                $cleanup = Start-TestStage -Name 'Restore isolated TSF environment' -Category 'test.cleanup'
                $env:LOCALAPPDATA = $previousLocalAppData
                $env:SHANJIE_HEADLESS = $previousHeadless
                $env:SHANJIE_TEST_PROFILE = $previousTestProfile
                Complete-TestStage -Stage $cleanup -Details @{ profile=$profileRoot }
            }
        }
    }

    if ($SmokeCase -in @('all','cold') -and !$Visual) {
        $cold = if ($ResultRoot) { Join-Path ([IO.Path]::GetFullPath($ResultRoot)) "cold-module/$Architecture" } else { Join-Path $root "build/cold-tip/$Architecture" }
        New-Item -ItemType Directory -Path $cold -Force | Out-Null
        Copy-Item "$runtimeRoot/$Architecture/Shanjie.dll" "$cold/Shanjie.dll" -Force
        $coldProfile = if ($ResultRoot) { Join-Path ([IO.Path]::GetFullPath($ResultRoot)) 'cold' } else { $cold }
        New-Item -ItemType Directory -Path "$coldProfile/Shanjie" -Force | Out-Null
        $previousLocalAppData = $env:LOCALAPPDATA
        $previousHeadless = $env:SHANJIE_HEADLESS
        $previousTestProfile = $env:SHANJIE_TEST_PROFILE
        try {
            $env:LOCALAPPDATA = $coldProfile
            $env:SHANJIE_TEST_PROFILE = '1'
            # Keep the native channel enabled; it must remain hidden and avoid renderer initialization without candidates.
            $env:SHANJIE_HEADLESS = $null
            Invoke-Timed -Name 'Run model-free TSF activation and shortcut regression' -Category 'tsf.cold-start' -Details @{ architecture=$Architecture } -Action {
                & $harness "$cold/Shanjie.dll" --cold-start
                if ($LASTEXITCODE) { throw 'Model-free TSF activation/shortcut check failed.' }
            }
        } finally {
            $cleanup = Start-TestStage -Name 'Restore cold-start environment' -Category 'test.cleanup'
            $env:LOCALAPPDATA = $previousLocalAppData
            $env:SHANJIE_HEADLESS = $previousHeadless
            $env:SHANJIE_TEST_PROFILE = $previousTestProfile
            Complete-TestStage -Stage $cleanup -Details @{ profile=$coldProfile }
        }
    }
    if (!$SmokeOnly -and !$Visual -and $SmokeCase -eq 'all') {
        Invoke-Timed -Name 'TSF input, submission, and popup regressions' -Category 'tsf.regression-suite' -Details @{ architecture=$Architecture } -Action { & "$PSScriptRoot/verify-regressions.ps1" -Architecture $Architecture }
    }
} finally {
    Pop-Location
    if (!$hadTimingDirectory) {
        $whole.Stop()
        Export-TestTimingSummary -Directory $timingDirectory -Level 'legacy-verify' -WallSeconds $whole.Elapsed.TotalSeconds
    }
}
