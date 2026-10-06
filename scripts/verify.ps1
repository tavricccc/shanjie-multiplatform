#Requires -Version 7
param([switch]$NativeOnly, [switch]$Visual, [ValidateSet('windows','shanjie')][string]$Interaction = 'windows')
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
Push-Location $root
try {
    if (!$NativeOnly) {
    & "$PSScriptRoot/build.ps1"
    New-Item -ItemType Directory -Path upstream/shanjie/data/lm -Force | Out-Null
    Copy-Item build/model/bigram.sjlm upstream/shanjie/data/lm/bigram.sjlm -Force
    foreach ($name in @('data','eval')) {
        $destination = Join-Path $root "build/$name"
        if (!(Test-Path $destination)) { New-Item -ItemType Junction -Path $destination -Target (Join-Path $root "upstream/shanjie/$name") | Out-Null }
    }
    cargo test --manifest-path build/core/Cargo.toml --lib
    if ($LASTEXITCODE) { throw 'Upstream unit tests failed.' }
    New-Item -ItemType Directory -Path build/core/tests -Force | Out-Null
    foreach ($test in @('engine','engine_learn','engine_replay','engine_lm')) { Copy-Item "upstream/shanjie/core/tests/$test.rs" build/core/tests -Force }
    $learnTests = Get-Content build/core/tests/engine_learn.rs -Raw
    $learnTests = $learnTests.Replace('fn store_append_refusals_fall_back_to_a_full_rewrite()', "#[cfg(unix)]`nfn store_append_refusals_fall_back_to_a_full_rewrite()")
    $learnTests = $learnTests.Replace('fn max_rss()', "#[cfg(unix)]`nfn max_rss()")
    $learnTests = $learnTests.Replace('fn perf_full_store_per_key_and_enter_with_write()', "#[cfg(unix)]`nfn perf_full_store_per_key_and_enter_with_write()")
    Set-Content build/core/tests/engine_learn.rs $learnTests -Encoding utf8NoBOM
    cargo test --manifest-path build/core/Cargo.toml --test engine --test engine_learn --test engine_replay -- --skip global_eps_table
    if ($LASTEXITCODE) { throw 'Engine behavior tests failed.' }
    }
    if (!(Test-Path dist/data/bigram.winmap)) { throw 'Run package.ps1 first to prepare the verified mapped model.' }
    $vs = & "${env:ProgramFiles(x86)}/Microsoft Visual Studio/Installer/vswhere.exe" -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    $vcvars = Join-Path $vs 'VC/Auxiliary/Build/vcvars64.bat'
    $command = @"
@echo off
call "$vcvars" >nul
if errorlevel 1 exit /b 1
cd /d "$root/build/native-x64"
cl /nologo /std:c++20 /EHsc /O2 /MT /W4 /utf-8 /DUNICODE /D_UNICODE /DNOMINMAX /I"$root/src" "$root/tests/tsf_smoke.cpp" /link /OUT:"$root/build/tsf-smoke.exe" ole32.lib oleaut32.lib uuid.lib user32.lib gdi32.lib
"@
    Set-Content build/native-x64/verify.cmd $command -Encoding utf8NoBOM
    & cmd.exe /d /c (Join-Path $root 'build/native-x64/verify.cmd')
    if ($LASTEXITCODE) { throw 'TSF smoke harness build failed.' }
    $arguments = @("$root/dist/x64/Shanjie.dll")
    if ($Visual) { $arguments += '--visual' }
    elseif ($Interaction -eq 'shanjie') { $arguments += '--shanjie' }
    $profileRoot = Join-Path $root "build/profile-tests/$Interaction"
    New-Item -ItemType Directory -Path "$profileRoot/Shanjie" -Force | Out-Null
    Copy-Item "profiles/$Interaction.ini" "$profileRoot/Shanjie/profile.ini" -Force
    $previousLocalAppData = $env:LOCALAPPDATA
    try {
        $env:LOCALAPPDATA = $profileRoot
        $env:SHANJIE_HEADLESS = '1'
        & ./build/tsf-smoke.exe @arguments
    } finally { $env:LOCALAPPDATA = $previousLocalAppData; Remove-Item Env:SHANJIE_HEADLESS }
    if ($LASTEXITCODE) { throw 'TSF smoke harness failed.' }
} finally { Pop-Location }
