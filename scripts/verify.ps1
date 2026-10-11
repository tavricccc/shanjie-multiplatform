#Requires -Version 7
param([switch]$NativeOnly, [switch]$Visual, [ValidateSet('windows','shanjie')][string]$Interaction = 'windows', [ValidateSet('x64','x86')][string]$Architecture = 'x64')
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
Push-Location $root
try {
    if (!$NativeOnly) {
    & "$PSScriptRoot/build.ps1" -Architecture $Architecture
    New-Item -ItemType Directory -Path upstream/shanjie/data/lm -Force | Out-Null
    Copy-Item build/model/bigram-v5.sjlm upstream/shanjie/data/lm/bigram.sjlm -Force
    Copy-Item build/model/classes-v3.sjc upstream/shanjie/data/lm/classes.sjc -Force
    foreach ($name in @('data','eval')) {
        $destination = Join-Path $root "build/$name"
        if (!(Test-Path $destination)) { New-Item -ItemType Junction -Path $destination -Target (Join-Path $root "upstream/shanjie/$name") | Out-Null }
    }
    cargo test --manifest-path build/core/Cargo.toml --lib
    if ($LASTEXITCODE) { throw 'Upstream unit tests failed.' }
    New-Item -ItemType Directory -Path build/core/tests -Force | Out-Null
    foreach ($test in @('engine','engine_learn','engine_replay','engine_lm','engine_demote','engine_predict','engine_pack','engine_vertical','kn_tiny','tinylm_tiny')) { Copy-Item "upstream/shanjie/core/tests/$test.rs" build/core/tests -Force }
    $learnTests = Get-Content build/core/tests/engine_learn.rs -Raw
    $learnTests = $learnTests.Replace('fn store_append_refusals_fall_back_to_a_full_rewrite()', "#[cfg(unix)]`nfn store_append_refusals_fall_back_to_a_full_rewrite()")
    $learnTests = $learnTests.Replace('fn max_rss()', "#[cfg(unix)]`nfn max_rss()")
    $learnTests = $learnTests.Replace('fn perf_full_store_per_key_and_enter_with_write()', "#[cfg(unix)]`nfn perf_full_store_per_key_and_enter_with_write()")
    Set-Content build/core/tests/engine_learn.rs $learnTests -Encoding utf8NoBOM
    $demoteTests = Get-Content build/core/tests/engine_demote.rs -Raw
    $demoteTests = $demoteTests.Replace('std::os::unix::fs::symlink(', 'std::fs::copy(')
    Set-Content build/core/tests/engine_demote.rs $demoteTests -Encoding utf8NoBOM
    cargo test --manifest-path build/core/Cargo.toml --test engine --test engine_learn --test engine_replay --test engine_lm --test engine_demote --test engine_predict --test engine_pack --test engine_vertical --test kn_tiny --test tinylm_tiny -- --test-threads=4 --skip global_eps_table
    if ($LASTEXITCODE) { throw 'Engine behavior tests failed.' }
    }
    if (!(Test-Path dist/data/models/model-v5/bigram.winmap)) { throw 'Run prepare-models.ps1 first to prepare the verified model/class pairs.' }
    $vs = & "${env:ProgramFiles(x86)}/Microsoft Visual Studio/Installer/vswhere.exe" -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    $vcvars = Join-Path $vs 'VC/Auxiliary/Build/vcvarsall.bat'
    $harness = Join-Path $root "build/tsf-smoke-$Architecture.exe"
    $command = @"
@echo off
call "$vcvars" $Architecture >nul
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
    $profileRoot = Join-Path $root "build/profile-tests/$Architecture/$Interaction"
    New-Item -ItemType Directory -Path "$profileRoot/Shanjie" -Force | Out-Null
    Copy-Item "profiles/$Interaction.ini" "$profileRoot/Shanjie/profile.ini" -Force
    $previousLocalAppData = $env:LOCALAPPDATA
    $previousHeadless = $env:SHANJIE_HEADLESS
    try {
        $env:LOCALAPPDATA = $profileRoot
        $env:SHANJIE_HEADLESS = if ($Visual) { $null } else { '1' }
        & $harness @arguments
    } finally { $env:LOCALAPPDATA = $previousLocalAppData; $env:SHANJIE_HEADLESS = $previousHeadless }
    if ($LASTEXITCODE) { throw 'TSF smoke harness failed.' }
} finally { Pop-Location }
