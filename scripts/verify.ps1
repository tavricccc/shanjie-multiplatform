#Requires -Version 7
param([switch]$NativeOnly, [switch]$Visual, [ValidateSet('windows','shanjie')][string]$Interaction = 'windows', [ValidateSet('x64','x86')][string]$Architecture = 'x64',
    [string[]]$CoreTests = @('engine','engine_learn','engine_replay','engine_lm','engine_demote','engine_predict','engine_pack','engine_vertical','kn_tiny','tinylm_tiny'))
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
Push-Location $root
try {
    if (!$NativeOnly) {
    & "$PSScriptRoot/build.ps1" -Architecture $Architecture
    New-Item -ItemType Directory -Path upstream/shanjie/data/lm -Force | Out-Null
    Copy-Item build/model/bigram-v6.sjlm upstream/shanjie/data/lm/bigram.sjlm -Force
    Copy-Item build/model/classes-v5.sjc upstream/shanjie/data/lm/classes.sjc -Force
    # Upstream goldens assume the kn-v1 side file next to the model; used for verification only, not packaged.
    if (!(Test-Path build/model/kn-v1.sjkn) -or (Get-FileHash build/model/kn-v1.sjkn).Hash.ToLowerInvariant() -ne 'cbefbced072f7e9d40dbfc3d2fad27c29f82e44bf801800dfb17b4eec962597e') {
        & "$PSScriptRoot/download-asset.ps1" -Url 'https://github.com/Nanako0129/shanjie/releases/download/kn-v1/kn.sjkn' -Destination (Join-Path $root 'build/model/kn-v1.sjkn') -Size 1414052 -Sha256 'cbefbced072f7e9d40dbfc3d2fad27c29f82e44bf801800dfb17b4eec962597e'
    }
    Copy-Item build/model/kn-v1.sjkn upstream/shanjie/data/lm/kn.sjkn -Force
    foreach ($name in @('data','eval')) {
        $destination = Join-Path $root "build/$name"
        if (!(Test-Path $destination)) { New-Item -ItemType Junction -Path $destination -Target (Join-Path $root "upstream/shanjie/$name") | Out-Null }
    }
    # Upstream's lib tests link model files with unix symlinks; copy them on Windows (generated tree only).
    $ffi = Get-Content build/core/src/ffi.rs -Raw
    Set-Content build/core/src/ffi.rs $ffi.Replace('std::os::unix::fs::symlink(', 'std::fs::copy(') -Encoding utf8NoBOM
    cargo test --release --manifest-path build/core/Cargo.toml --lib -- --skip english_codes_through_the_c_abi --skip child_english
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
    $packTests = Get-Content build/core/tests/engine_pack.rs -Raw
    Set-Content build/core/tests/engine_pack.rs $packTests.Replace('std::os::unix::fs::symlink(', 'std::fs::copy(') -Encoding utf8NoBOM
    $knTests = (Get-Content build/core/tests/kn_tiny.rs -Raw).Replace("`r`n","`n")
    $symlink = @'
    std::os::unix::fs::symlink(tmp.join("missing.sjkn"), tmp.join("kn.sjkn")).unwrap();
    assert_eq!(Lm::load(&m).err(), Some(LmError::KnSide), "a dangling kn.sjkn symlink is not 'absent'");
    assert!(Lm::load_with(&m, true, false).is_ok());
    std::fs::remove_file(tmp.join("kn.sjkn")).unwrap();
'@
    if (!$knTests.Contains($symlink)) { throw 'Upstream KN symlink fixture changed.' }
    $knTests = $knTests.Replace($symlink,"    #[cfg(unix)] {`n$symlink`n    }")
    $knTests = $knTests.Replace('let want = String::from_utf8(bytes("decode.tsv")).unwrap();', 'let want = String::from_utf8(bytes("decode.tsv")).unwrap().replace("\r\n", "\n");')
    Set-Content build/core/tests/kn_tiny.rs $knTests -Encoding utf8NoBOM
    $testArguments = foreach ($test in $CoreTests) { '--test'; $test }
    cargo test --release --manifest-path build/core/Cargo.toml @testArguments -- --test-threads=4 --skip global_eps_table
    if ($LASTEXITCODE) { throw 'Engine behavior tests failed.' }
    }
    if (!(Test-Path dist/data/models/model-v6/bigram.winmap)) { throw 'Run prepare-models.ps1 first to prepare the verified model/class pairs.' }
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
    $profileRoot = Join-Path $root ("build/profile-tests/$Architecture/$Interaction/" + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path "$profileRoot/Shanjie" -Force | Out-Null
    Copy-Item "profiles/$Interaction.ini" "$profileRoot/Shanjie/profile.ini" -Force
    # Force Standard; desktop active profile and prior learning must not change fixtures.
    $profileText = (Get-Content "$profileRoot/Shanjie/profile.ini" -Raw) -replace '(?m)^keyboard=.*','keyboard=1'
    Set-Content "$profileRoot/Shanjie/profile.ini" $profileText -Encoding utf8NoBOM
    $previousLocalAppData = $env:LOCALAPPDATA
    $previousHeadless = $env:SHANJIE_HEADLESS
    try {
        $env:LOCALAPPDATA = $profileRoot
        $env:SHANJIE_HEADLESS = if ($Visual) { $null } else { '1' }
        & $harness @arguments
    } finally { $env:LOCALAPPDATA = $previousLocalAppData; $env:SHANJIE_HEADLESS = $previousHeadless }
    if ($LASTEXITCODE) { throw 'TSF smoke harness failed.' }
    if (!$Visual) {
        $cold = Join-Path $root "build/cold-tip/$Architecture"
        New-Item -ItemType Directory -Path $cold -Force | Out-Null
        Copy-Item "$root/dist/$Architecture/Shanjie.dll" "$cold/Shanjie.dll" -Force
        try {
            $env:LOCALAPPDATA = $profileRoot
            # Keep the native channel enabled: it must remain hidden and avoid
            # renderer initialization while no candidate is displayed.
            $env:SHANJIE_HEADLESS = $null
            & $harness "$cold/Shanjie.dll" --cold-start
            if ($LASTEXITCODE) { throw 'Model-free TSF activation/shortcut check failed.' }
        } finally { $env:LOCALAPPDATA = $previousLocalAppData; $env:SHANJIE_HEADLESS = $previousHeadless }
    }
} finally { Pop-Location }
