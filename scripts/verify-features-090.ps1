#Requires -Version 7
param([ValidateSet('x64','x86')][string[]]$Architectures=@('x64','x86'))
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
$result=Join-Path $root 'build/release-0.9.0'
$vs=& "${env:ProgramFiles(x86)}/Microsoft Visual Studio/Installer/vswhere.exe" -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
foreach($arch in $Architectures) {
    $out=Join-Path $result "tsf-$arch"
    New-Item -ItemType Directory "$out/profile/Shanjie" -Force | Out-Null
    $text=(Get-Content "$root/profiles/windows.ini" -Raw) -replace '(?m)^learning=.*','learning=0'
    Set-Content "$out/profile/Shanjie/profile.ini" $text -Encoding utf8NoBOM
    $exe=Join-Path $out 'features.exe'
    $command=@"
@echo off
call "$vs/VC/Auxiliary/Build/vcvarsall.bat" $arch >nul
if errorlevel 1 exit /b 1
cl /nologo /std:c++20 /EHsc /O2 /MT /W4 /utf-8 /DUNICODE /D_UNICODE /DNOMINMAX /I"$root/tests/support" "$root/tests/features_090.cpp" /Fo"$out/features.obj" /Fe"$exe" /link ole32.lib oleaut32.lib uuid.lib user32.lib gdi32.lib advapi32.lib
"@
    Set-Content "$out/compile.cmd" $command -Encoding utf8NoBOM
    & cmd.exe /d /c "$out/compile.cmd" *> "$out/compile.log"
    if($LASTEXITCODE){throw "0.9.0 TSF harness compile failed: $arch"}
    $previous=$env:LOCALAPPDATA; $previousTest=$env:SHANJIE_TEST_PROFILE; $previousHeadless=$env:SHANJIE_HEADLESS
    try {
        $env:LOCALAPPDATA="$out/profile"; $env:SHANJIE_TEST_PROFILE='1'; $env:SHANJIE_HEADLESS='1'
        # Elevated test hosts deliberately cannot auto-start a user core. Start
        # the isolated fixture explicitly; its endpoint includes this profile.
        $worker=Start-Process -FilePath "$root/dist/tools/shanjie-core.exe" -ArgumentList '--serve' -WindowStyle Hidden -RedirectStandardError "$out/core-error.log" -RedirectStandardOutput "$out/core-output.log" -PassThru
        Start-Sleep -Milliseconds 500
        & $exe "$root/dist/$arch/Shanjie.dll" $out *> "$out/result.log"
        if($LASTEXITCODE){throw "0.9.0 TSF regression failed: $arch, exit $LASTEXITCODE"}
    } finally {
        $stop=Start-Process -FilePath "$root/dist/tools/shanjie-core.exe" -ArgumentList '--stop' -WindowStyle Hidden -PassThru
        $stop.WaitForExit(5000) | Out-Null
        if($worker){ $worker.WaitForExit(5000) | Out-Null }
        $env:LOCALAPPDATA=$previous; $env:SHANJIE_TEST_PROFILE=$previousTest; $env:SHANJIE_HEADLESS=$previousHeadless
    }
}
Write-Host 'PASS 0.9.0 offscreen TSF regression'
