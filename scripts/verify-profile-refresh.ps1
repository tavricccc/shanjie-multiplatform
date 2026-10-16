#Requires -Version 7
param([ValidateSet('x64','x86')][string[]]$Architectures=@('x64','x86'),[ValidateSet('none','before-apply','after-apply','drop-reply','hold-state')][string[]]$Faults=@('none','before-apply','after-apply','drop-reply'),[ValidateSet('normal','queued','latest','escape','focus','deadline')][string[]]$Scenarios=@('normal'),[string]$ResultRoot)
$ErrorActionPreference='Stop';$root=Split-Path $PSScriptRoot -Parent
if(!$ResultRoot){$ResultRoot=Join-Path $root ('build/profile-refresh-'+[guid]::NewGuid().ToString('N'))}
$ResultRoot=[IO.Path]::GetFullPath($ResultRoot);New-Item -ItemType Directory $ResultRoot -Force|Out-Null
$saved=@{};foreach($name in 'LOCALAPPDATA','SHANJIE_TEST_PROFILE','SHANJIE_HEADLESS','SHANJIE_CORE_LOCAL','SHANJIE_TEST_FAULT','SHANJIE_TEST_FAULT_OP'){$saved[$name]=[Environment]::GetEnvironmentVariable($name)}
$vs=& "${env:ProgramFiles(x86)}/Microsoft Visual Studio/Installer/vswhere.exe" -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
try {
 $env:SHANJIE_CORE_LOCAL=$null
 foreach($arch in $Architectures){
    $out=Join-Path $ResultRoot $arch;New-Item -ItemType Directory $out -Force|Out-Null
    $command=@"
@echo off
call "$vs/VC/Auxiliary/Build/vcvarsall.bat" $arch >nul
if errorlevel 1 exit /b 1
cl /nologo /std:c++20 /EHsc /O2 /MT /W4 /utf-8 /DUNICODE /D_UNICODE /DNOMINMAX /I"$root/tests/support" "$root/tests/profile_refresh.cpp" /Fo"$out/refresh.obj" /Fe"$out/refresh.exe" /link ole32.lib oleaut32.lib uuid.lib user32.lib gdi32.lib
"@
    Set-Content "$out/compile.cmd" $command -Encoding utf8NoBOM;& cmd.exe /d /c "$out/compile.cmd" *> "$out/compile.log";if($LASTEXITCODE){throw 'Refresh harness compilation failed'}
    foreach($fault in $Faults){foreach($scenario in $Scenarios){
      $folder=Join-Path $out "$fault-$scenario";if(Test-Path $folder){throw 'Use a fresh result directory for fault isolation'};New-Item -ItemType Directory "$folder/profile/Shanjie" -Force|Out-Null
      $text=(Get-Content "$root/profiles/windows.ini" -Raw) -replace '(?m)^learning=.*','learning=0' -replace '(?m)^keyboard=.*','keyboard=1'
      Set-Content "$folder/profile/Shanjie/profile.ini" $text -Encoding utf8NoBOM
      $env:LOCALAPPDATA="$folder/profile";$env:SHANJIE_TEST_PROFILE='1';$env:SHANJIE_HEADLESS='1'
      $env:SHANJIE_TEST_FAULT=if($fault -eq 'none'){$null}else{$fault};$env:SHANJIE_TEST_FAULT_OP='prediction'
      $worker=Start-Process "$root/dist/tools/shanjie-core.exe" -ArgumentList '--serve' -WindowStyle Hidden -PassThru
      try {& "$out/refresh.exe" "$root/dist/$arch/Shanjie.dll" $scenario *> "$folder/result.log";if($LASTEXITCODE){throw "Refresh check failed: $arch $fault $scenario (see $folder/result.log)"};Write-Output "PASS $arch $fault $scenario"}
      finally {$stop=Start-Process "$root/dist/tools/shanjie-core.exe" -ArgumentList '--stop' -WindowStyle Hidden -PassThru;$stop.WaitForExit(5000)|Out-Null;$worker.WaitForExit(5000)|Out-Null}
    }}
 }
}finally{foreach($name in $saved.Keys){[Environment]::SetEnvironmentVariable($name,$saved[$name])}}
