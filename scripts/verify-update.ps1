#Requires -Version 7
param([string]$ResultRoot,[switch]$ExplicitCore)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
if (!$ResultRoot) { $ResultRoot = Join-Path $root ('build/update-checks-' + (Get-Date -Format yyyyMMdd-HHmmss)) }
New-Item -ItemType Directory -Path $ResultRoot -Force | Out-Null
$payload=Join-Path $ResultRoot 'payload'
New-Item -ItemType Directory -Path $payload -Force | Out-Null
foreach($architecture in 'x64','x86') {
    New-Item -ItemType Directory "$payload/$architecture" -Force | Out-Null
    Copy-Item "$root/dist/$architecture/Shanjie.dll" "$payload/$architecture" -Force
}
Copy-Item "$root/dist/data","$root/dist/tools" $payload -Recurse -Force
# A portable fixture must expose both layouts independently of the real install.
Set-Content "$payload/installed.json" '{"version":"test","keyboards":3}' -Encoding utf8NoBOM
$profile = Join-Path $ResultRoot 'profile'
New-Item -ItemType Directory -Path "$profile/Shanjie" -Force | Out-Null
$previousLocal = $env:LOCALAPPDATA
$previousHeadless = $env:SHANJIE_HEADLESS
$previousTest = $env:SHANJIE_TEST_PROFILE
$previousCore = $env:SHANJIE_CORE_LOCAL
$previousEten = $env:SHANJIE_TEST_ETEN
$previousExplicit = $env:SHANJIE_TEST_EXPLICIT_CORE
$vs = & "${env:ProgramFiles(x86)}/Microsoft Visual Studio/Installer/vswhere.exe" -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
$cases = @()
foreach ($setting in 0,1) { foreach ($caps in 0,1) { foreach ($shift in 0,1) {
    $cases += ,@("setting$setting-caps$caps-shift$shift",$caps,$shift,$setting,0,0)
} } }
$cases += ,@('caps-on-midword',0,0,0,1,0)
$cases += ,@('caps-off-midword',1,0,0,1,0)
$cases += ,@('chinese-prefix-caps-on',1,0,0,0,1)
$cases += ,@('punctuation-empty',0,0,0,0,0,'punctuation')
$cases += ,@('punctuation-composing',0,0,0,0,1,'punctuation')
$cases += ,@('caps-on-zhuyin',1,0,0,0,0,'chinese')
$cases += ,@('first-keys-immediate',1,0,0,0,0,'immediate')
$cases += ,@('switch-fields',0,0,0,0,0,'fields')
$results = @()
function Invoke-Check([string]$Executable, [string[]]$Arguments, [switch]$AllowFailure) {
    $start = [Diagnostics.ProcessStartInfo]::new($Executable)
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    $start.StandardOutputEncoding = [Text.UTF8Encoding]::new($false)
    $start.StandardErrorEncoding = [Text.UTF8Encoding]::new($false)
    foreach ($argument in $Arguments) { $start.ArgumentList.Add($argument) }
    $process = [Diagnostics.Process]::new()
    $process.StartInfo = $start
    $null = $process.Start()
    $stdout = $process.StandardOutput.ReadToEndAsync()
    $stderr = $process.StandardError.ReadToEndAsync()
    if (!$process.WaitForExit(60000)) { $process.Kill(); throw "Check timed out: $Executable" }
    if (!$stdout.Wait(5000) -or !$stderr.Wait(5000)) { $process.Dispose(); throw 'Owned check exited but its output pipe remained open.' }
    $text = $stdout.GetAwaiter().GetResult()
    $errorText = $stderr.GetAwaiter().GetResult()
    $code = $process.ExitCode
    $process.Dispose()
    if ($code -and !$AllowFailure) { throw "Check failed ($code): $text $errorText" }
    $text
}
try {
    $env:LOCALAPPDATA = $profile
    $env:SHANJIE_HEADLESS = '1'
    $env:SHANJIE_TEST_PROFILE = '1'
    $env:SHANJIE_TEST_EXPLICIT_CORE = if($ExplicitCore){'1'}else{$null}
    $env:SHANJIE_CORE_LOCAL = $null
    Copy-Item "$root/profiles/windows.ini" "$profile/Shanjie/profile.ini" -Force
    $content = Get-Content "$profile/Shanjie/profile.ini" -Raw
    $content = $content -replace '(?m)^mixed_input=.*','mixed_input=1' -replace '(?m)^learning=.*','learning=0' -replace '(?m)^caps_lock_english=.*','caps_lock_english=0' -replace '(?m)^keyboard=.*','keyboard=1'
    Set-Content "$profile/Shanjie/profile.ini" $content -Encoding utf8NoBOM
    $brokerLog = Invoke-Check "$root/windows/target/x86_64-pc-windows-msvc/release/shanjie-core-check.exe" @("$payload/data")
    Set-Content "$ResultRoot/resident.log" $brokerLog -Encoding utf8NoBOM
    foreach ($arch in 'x64','x86') {
        $output = Join-Path $ResultRoot $arch
        New-Item -ItemType Directory -Path $output -Force | Out-Null
        $command = @"
@echo off
call "$vs/VC/Auxiliary/Build/vcvarsall.bat" $arch >nul
if errorlevel 1 exit /b 1
cd /d "$output"
cl /nologo /std:c++20 /EHsc /O2 /MT /utf-8 /DUNICODE /D_UNICODE /DNOMINMAX /I"$root/tests/support" /I"$root/upstream/shanjie/core/include" "$root/tests/caps_lock.cpp" /link /OUT:"$output/input-probe.exe" ole32.lib oleaut32.lib uuid.lib user32.lib gdi32.lib
"@
        Set-Content "$output/compile.cmd" $command -Encoding utf8NoBOM
        & cmd.exe /d /c "$output/compile.cmd" | Out-File "$output/compile.log"
        if ($LASTEXITCODE) { throw "$arch regression probe build failed." }
        foreach ($keyboard in 1,2) {
          $env:SHANJIE_TEST_ETEN = if ($keyboard -eq 2) { '1' } else { $null }
          foreach ($case in $cases) {
            $modified = $content -replace '(?m)^caps_lock_english=.*',("caps_lock_english=" + $case[3])
            $modified = $modified -replace '(?m)^keyboard=.*',("keyboard=" + $keyboard)
            Set-Content "$profile/Shanjie/profile.ini" $modified -Encoding utf8NoBOM
            if ($case[0] -eq 'first-keys-immediate') {
                Invoke-Check "$payload/tools/shanjie-core.exe" @('--stop') | Out-Null
                Start-Sleep -Milliseconds 100
                # Explicit isolated worker also supports elevated test hosts;
                # the product still refuses elevated automatic core startup.
                if($ExplicitCore){$fixture=Start-Process "$payload/tools/shanjie-core.exe" -ArgumentList '--serve' -WindowStyle Hidden -PassThru}
            }
            $arguments = @("$payload/$arch/Shanjie.dll") + $case[1..($case.Count-1)]
            $log = Invoke-Check "$output/input-probe.exe" $arguments
            Set-Content "$output/keyboard$keyboard-$($case[0]).log" $log -Encoding utf8NoBOM
            $json = (($log -split "`r?`n") | Where-Object { $_ -like 'RESULT *' }) -replace '^RESULT ','' | ConvertFrom-Json
            if (!$json.matches) { throw "$arch $($case[0]) output mismatch: $log" }
            $results += [pscustomobject]@{architecture=$arch;keyboard=$keyboard;case=$case[0];result=$json}
          }
        }
    }
    $status = (Invoke-Check "$payload/tools/shanjie-core.exe" @('--status') | ConvertFrom-Json)
    $process = Get-Process -Id $status.pid
    $status | Add-Member -NotePropertyName working_set_bytes -NotePropertyValue $process.WorkingSet64
    $status | Add-Member -NotePropertyName private_bytes -NotePropertyValue $process.PrivateMemorySize64
    $results | ConvertTo-Json -Depth 8 | Set-Content "$ResultRoot/results.json" -Encoding utf8NoBOM
    $status | ConvertTo-Json | Set-Content "$ResultRoot/resident-status.json" -Encoding utf8NoBOM
    Write-Output "Passed $($results.Count) native cases. Evidence: $ResultRoot"
} finally {
    Invoke-Check "$payload/tools/shanjie-core.exe" @('--stop') -AllowFailure | Out-Null
    $env:LOCALAPPDATA=$previousLocal
    $env:SHANJIE_HEADLESS=$previousHeadless
    $env:SHANJIE_TEST_PROFILE=$previousTest
    $env:SHANJIE_CORE_LOCAL=$previousCore
    $env:SHANJIE_TEST_ETEN=$previousEten
    $env:SHANJIE_TEST_EXPLICIT_CORE=$previousExplicit
}
