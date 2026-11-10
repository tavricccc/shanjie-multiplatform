#Requires -Version 7
param([switch]$NativeOnly, [switch]$UseExistingBuild, [switch]$Visual, [switch]$CoreOnly, [ValidateSet('windows','shanjie')][string]$Interaction = 'windows', [ValidateSet('x64','x86')][string]$Architecture = 'x64',
    # Empty runs every upstream core test (scripts/test-core.ps1).
    [string[]]$CoreTests)
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot/msvc-command.ps1"
. "$PSScriptRoot/preserve-output.ps1"
$root = Split-Path $PSScriptRoot -Parent
Push-Location $root
try {
    & "$PSScriptRoot/verify-upstream-data.ps1"
    if (!$NativeOnly) {
        if (!$UseExistingBuild) { & "$PSScriptRoot/build.ps1" -Architecture $Architecture }
        & "$PSScriptRoot/test-core.ps1" -Tests $CoreTests -UseExistingCore
    }
    if($CoreOnly){return}
    if (!(Test-Path dist/data/models/builtin/bigram.winmap)) { throw 'Run prepare-models.ps1 first to prepare the built-in model set.' }
    & "$PSScriptRoot/verify-upstream-data.ps1" -Dist
    # The unit tests that read the built data and model (dist/data); plain `cargo test --lib` ignores them.
    # Both TIP architectures use the same x64 resident core. Run its data checks
    # once in the x64 pass; an x86 MSVC environment cannot link x64 Rust tests.
    # One thread avoids evicting each other's entries from the shared model cache.
    if ($Architecture -eq 'x64') {
        & cargo test --manifest-path windows/Cargo.toml --lib --target x86_64-pc-windows-msvc -- --ignored mixed_regression --test-threads=1
        if ($LASTEXITCODE) { throw 'The mixed-input and digit regressions failed against the built data.' }
    }
    $harness = Join-Path $root "build/tsf-smoke-$Architecture.exe"
    $command = @"
@echo off
$(Get-MsvcSetup $Architecture)
if errorlevel 1 exit /b 1
cd /d "$root/build/native-$Architecture"
cl /nologo /std:c++20 /EHsc /O2 /MT /W4 /utf-8 /DUNICODE /D_UNICODE /DNOMINMAX /I"$root/tests/support" "$root/tests/tsf_smoke.cpp" /link /OUT:"$harness" ole32.lib oleaut32.lib uuid.lib user32.lib gdi32.lib
"@
    Set-Content "build/native-$Architecture/verify.cmd" $command -Encoding utf8NoBOM
    & cmd.exe /d /c (Join-Path $root "build/native-$Architecture/verify.cmd")
    if ($LASTEXITCODE) { throw 'TSF smoke harness build failed.' }
    $arguments = @("$root/dist/$Architecture/Shanjie.dll")
    if ($Visual) { $arguments += '--visual' }
    elseif ($Interaction -eq 'shanjie') { $arguments += '--shanjie' }
    $profileRoot = Join-Path $root ("build/profile-tests/$Architecture/$Interaction/" + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path "$profileRoot/Shanjie" -Force | Out-Null
    Copy-Item "profiles/$Interaction.ini" "$profileRoot/Shanjie/profile.ini" -Force
    # Force Standard and the chat profile (the harness is no chat program, so automatic would pick formal);
    # desktop active profile and prior learning must not change fixtures.
    $profileText = (Get-Content "$profileRoot/Shanjie/profile.ini" -Raw) -replace '(?m)^keyboard=.*','keyboard=1' -replace '(?m)^ranking=.*','ranking=chat'
    # The harness sets the mode and the right-arrow action it tests itself.
    if ($Interaction -eq 'windows') { $profileText = $profileText -replace '(?m)^upstream_english=.*','upstream_english=0' -replace '(?m)^right_action=.*','right_action=none' }
    Set-Content "$profileRoot/Shanjie/profile.ini" $profileText -Encoding utf8NoBOM
    $previousLocalAppData = $env:LOCALAPPDATA
    $previousHeadless = $env:SHANJIE_HEADLESS
    $previousTestProfile = $env:SHANJIE_TEST_PROFILE
    try {
        $env:LOCALAPPDATA = $profileRoot
        $env:SHANJIE_TEST_PROFILE = '1'
        $env:SHANJIE_HEADLESS = if ($Visual) { $null } else { '1' }
        $fixtureCore = Start-Process "$root/dist/tools/shanjie-core.exe" -ArgumentList '--serve' -WindowStyle Hidden -PassThru
        & $harness @arguments
        $harnessExit = $LASTEXITCODE
        if (!$harnessExit -and !$Visual) {
            # Make both layouts available to this portable DLL without changing machine registration.
            $installation = Join-Path $root 'dist/installed.json'
            $savedInstallation = if (Test-Path -LiteralPath $installation) { [IO.File]::ReadAllBytes($installation) } else { $null }
            try {
                [IO.File]::WriteAllText($installation,'{"version":"verification-fixture","keyboards":3}')
                & $harness "$root/dist/$Architecture/Shanjie.dll" --zhuyin-only
                $harnessExit = $LASTEXITCODE
            } finally {
                if ($null -ne $savedInstallation) { [IO.File]::WriteAllBytes($installation,$savedInstallation) }
                else { Preserve-Output $installation }
            }
        }
    } finally {
        $stop = Start-Process "$root/dist/tools/shanjie-core.exe" -ArgumentList '--stop' -WindowStyle Hidden -PassThru
        $stop.WaitForExit(5000) | Out-Null
        if ($fixtureCore) { $fixtureCore.WaitForExit(5000) | Out-Null }
        $env:LOCALAPPDATA = $previousLocalAppData; $env:SHANJIE_HEADLESS = $previousHeadless; $env:SHANJIE_TEST_PROFILE = $previousTestProfile
    }
    if ($harnessExit) { throw 'TSF smoke harness failed.' }
    if (!$Visual) {
        $cold = Join-Path $root "build/cold-tip/$Architecture"
        New-Item -ItemType Directory -Path $cold -Force | Out-Null
        Copy-Item "$root/dist/$Architecture/Shanjie.dll" "$cold/Shanjie.dll" -Force
        try {
            $env:LOCALAPPDATA = $profileRoot
            $env:SHANJIE_TEST_PROFILE = '1'
            # Keep the native channel enabled: it must remain hidden and avoid
            # renderer initialization while no candidate is displayed.
            $env:SHANJIE_HEADLESS = $null
            & $harness "$cold/Shanjie.dll" --cold-start
            if ($LASTEXITCODE) { throw 'Model-free TSF activation/shortcut check failed.' }
        } finally { $env:LOCALAPPDATA = $previousLocalAppData; $env:SHANJIE_HEADLESS = $previousHeadless; $env:SHANJIE_TEST_PROFILE = $previousTestProfile }
        & "$PSScriptRoot/verify-regressions.ps1" -Architecture $Architecture
    }
} finally { Pop-Location }
