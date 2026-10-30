#Requires -Version 7
# The real-TSF regressions on owned windows and private desktops, against the built dist/ of one architecture:
# Shift and the add-word popup, English, Caps Lock and Enter keys, focus handoff and add-word rejection, popup lifecycle.
# No registration, foreground activation or desktop input; each case gets its own profile under build/.
param([ValidateSet('x64','x86')][string]$Architecture='x64',[string]$ResultRoot)
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
if(!$ResultRoot){$ResultRoot=Join-Path $root ('build/regressions-'+[guid]::NewGuid().ToString('N'))}
$ResultRoot=[IO.Path]::GetFullPath($ResultRoot)
if(Test-Path -LiteralPath $ResultRoot){throw 'Use a fresh result directory.'}
& "$PSScriptRoot/verify-add-word.ps1" -Architectures $Architecture -ResultRoot "$ResultRoot/add-word"
& "$PSScriptRoot/verify-english-keys.ps1" -Architectures $Architecture -ResultRoot "$ResultRoot/english-keys"
# The private-desktop harnesses read an uninstalled payload laid out like the installed folder.
$payload=[IO.Path]::GetFullPath("$ResultRoot/payload")
New-Item -ItemType Directory "$payload/$Architecture" -Force|Out-Null
Copy-Item "$root/dist/$Architecture/Shanjie.dll" "$payload/$Architecture" -Force
foreach($name in 'data','tools'){New-Item -ItemType Junction -Path "$payload/$name" -Target "$root/dist/$name"|Out-Null}
Set-Content "$payload/installed.json" '{"version":"test","keyboards":3}' -Encoding utf8NoBOM
$vs=& "${env:ProgramFiles(x86)}/Microsoft Visual Studio/Installer/vswhere.exe" -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
$lifecycle=@('cancel-command','cancel-deactivate','cancel-double','cancel-escape','cancel-focus-notification','cancel-keyboard-reading',
    'cancel-late','cancel-mouse','cancel-outside','cancel-owner-destroy','cancel-reentry','cancel-save','cancel-stale-notify',
    'cancel-theme-dark','cancel-theme-light')
# Every private desktop starts its own core from the payload; none of them may outlive the run.
try{
foreach($suite in @(@('tsf_compat',$null),@('popup_lifecycle',$lifecycle))){
    $name=$suite[0];$out="$ResultRoot/$name";New-Item -ItemType Directory $out -Force|Out-Null
    $harness="$out/$name-$Architecture.exe"
    $command=@"
@echo off
call "$vs/VC/Auxiliary/Build/vcvarsall.bat" $Architecture >nul
if errorlevel 1 exit /b 1
cl /nologo /std:c++20 /EHsc /O2 /MT /W4 /utf-8 /DUNICODE /D_UNICODE /DNOMINMAX /I"$root/tests/support" "$root/tests/$name.cpp" /Fo"$out/$name.obj" /Fe"$harness" /link ole32.lib oleaut32.lib uuid.lib user32.lib gdi32.lib advapi32.lib
"@
    Set-Content "$out/compile.cmd" $command -Encoding utf8NoBOM
    & cmd.exe /d /c "$out/compile.cmd" *> "$out/compile.log"
    if($LASTEXITCODE){throw "$name harness compile failed: $Architecture ($out/compile.log)"}
    $cases=if($suite[1]){@{Cases=$suite[1]}}else{@{}}
    & "$PSScriptRoot/verify-private-desktop.ps1" -Payload $payload -Harness $harness -Output "$out/cases" @cases
}
}finally{
    Get-Process shanjie-core -ErrorAction SilentlyContinue|Where-Object{$_.Path -and $_.Path.StartsWith($payload,'OrdinalIgnoreCase')}|Stop-Process -Force -ErrorAction SilentlyContinue
}
Write-Host "PASS real-TSF regressions ($Architecture)"
