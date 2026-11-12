#Requires -Version 7
param([string]$HarnessPath,[string]$ResultRoot,[string]$RuntimeRoot)
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
if(!$RuntimeRoot){$RuntimeRoot=Join-Path $root 'dist'}
Import-Module "$PSScriptRoot/test-support.psm1" -ErrorAction Stop
. "$PSScriptRoot/msvc-command.ps1"
$null=Initialize-TestTimings -ProjectRoot $root -Name 'verify-interaction'
$buildRoot=[IO.Path]::GetFullPath((Join-Path $root 'build'))
$testRoot=if($ResultRoot){[IO.Path]::GetFullPath($ResultRoot)}else{Join-Path $buildRoot ('interaction-'+[guid]::NewGuid().ToString('N'))}
if(!$testRoot.StartsWith($buildRoot+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)){throw "Refusing an interaction fixture outside build/: $testRoot"}
if(Test-Path -LiteralPath (Join-Path $testRoot 'profile')){throw 'Use a fresh interaction fixture directory.'}
$null=New-Item -ItemType Directory -Path $testRoot -Force
if($HarnessPath){
    $exe=[IO.Path]::GetFullPath($HarnessPath)
    if(!(Test-Path -LiteralPath $exe)){throw "Interaction harness is missing: $exe"}
}else{
    $vs=& "${env:ProgramFiles(x86)}/Microsoft Visual Studio/Installer/vswhere.exe" -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    $vcvars=Join-Path $vs 'VC/Auxiliary/Build/vcvars64.bat'
    $exe="$root/build/windows-interaction.exe"
    $command=@"
@echo off
call "$vcvars" >nul
if errorlevel 1 exit /b 1
cd /d "$root/build/native-x64"
cl /nologo /std:c++20 /EHsc /O2 /MT /W4 /utf-8 /DUNICODE /D_UNICODE /DNOMINMAX /I"$root/tests/support" /I"$root/upstream/shanjie/core/include" "$root/tests/windows_interaction.cpp" "$root/tests/support/engine.cpp" "$root/tests/support/key_policy.cpp" "$root/tests/support/special_input.cpp" "$root/tests/support/associated_candidates.cpp" "$root/tests/support/pinyin_input.cpp" "$root/windows/target/x86_64-pc-windows-msvc/release/shanjie_windows.lib" /link /OUT:"$exe" ole32.lib user32.lib gdi32.lib d2d1.lib dwrite.lib dwmapi.lib windowsapp.lib advapi32.lib bcrypt.lib ntdll.lib ws2_32.lib userenv.lib
"@
    $script="$root/build/native-x64/interaction.cmd"
    Set-Content -LiteralPath $script -Value $command -Encoding utf8NoBOM
    Invoke-Timed -Name 'Compile Windows interaction harness (x64)' -Category 'build.cpp.harness' -Details @{architecture='x64';source='tests/windows_interaction.cpp'} -Action {
        & cmd.exe /d /c $script
        if($LASTEXITCODE){throw 'Interaction harness build failed.'}
    }
}
$previousLocal=$env:LOCALAPPDATA
$previousCore=$env:SHANJIE_CORE_LOCAL
$previousTest=$env:SHANJIE_TEST_PROFILE
try{
    $env:LOCALAPPDATA=Join-Path $testRoot 'profile'
    $null=New-Item -ItemType Directory -Path $env:LOCALAPPDATA -Force
    $env:SHANJIE_TEST_PROFILE='1'
    # Fixture lexicons use local core; resident IPC has a separate integration test.
    $env:SHANJIE_CORE_LOCAL='1'
    Invoke-Timed -Name 'Windows interaction regression (x64)' -Category 'interaction.engine' -Details @{architecture='x64';profile=$env:LOCALAPPDATA} -Action {
        & $exe "$RuntimeRoot/data"
        if($LASTEXITCODE){throw "Interaction checks failed: $LASTEXITCODE"}
    }
}finally{$env:LOCALAPPDATA=$previousLocal;$env:SHANJIE_CORE_LOCAL=$previousCore;$env:SHANJIE_TEST_PROFILE=$previousTest}
