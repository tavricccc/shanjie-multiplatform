#Requires -Version 7
param([string]$ResultRoot,[string]$DllRoot,[ValidateSet('x64','x86')][string[]]$Architectures=@('x64','x86'),[string[]]$Modes)
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
if(!$ResultRoot){$ResultRoot=Join-Path $root ('build/submission-'+[guid]::NewGuid().ToString('N'))}
if(!$DllRoot){$DllRoot=Join-Path $root 'dist'}
$ResultRoot=[IO.Path]::GetFullPath($ResultRoot)
$DllRoot=[IO.Path]::GetFullPath($DllRoot)
New-Item -ItemType Directory -Path $ResultRoot -Force | Out-Null
$taskProfileRoot=Join-Path $ResultRoot 'profile'
New-Item -ItemType Directory -Path "$taskProfileRoot/Shanjie" -Force | Out-Null
$content=(Get-Content "$root/profiles/windows.ini" -Raw) -replace '(?m)^mixed_input=.*','mixed_input=1' -replace '(?m)^learning=.*','learning=0' -replace '(?m)^prediction=.*','prediction=1' -replace '(?m)^caps_lock_english=.*','caps_lock_english=0' -replace '(?m)^shift_side=.*','shift_side=2' -replace '(?m)^escape=.*','escape=clear'
$saved=@{};foreach($name in 'LOCALAPPDATA','SHANJIE_HEADLESS','SHANJIE_TEST_PROFILE','SHANJIE_CORE_LOCAL'){$saved[$name]=[Environment]::GetEnvironmentVariable($name,'Process')}
$vs=& "${env:ProgramFiles(x86)}/Microsoft Visual Studio/Installer/vswhere.exe" -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
$results=@()
try {
 $env:LOCALAPPDATA=$taskProfileRoot;$env:SHANJIE_HEADLESS='1';$env:SHANJIE_TEST_PROFILE='1';$env:SHANJIE_CORE_LOCAL='1'
 foreach($arch in $Architectures) {
  $out=Join-Path $ResultRoot $arch;New-Item -ItemType Directory -Path $out -Force | Out-Null
  $exe=Join-Path $out 'submission.exe'
  $command=@"
@echo off
call "$vs/VC/Auxiliary/Build/vcvarsall.bat" $arch >nul
if errorlevel 1 exit /b 1
cd /d "$out"
cl /nologo /std:c++20 /EHsc /O2 /MT /utf-8 /DUNICODE /D_UNICODE /DNOMINMAX /I"$root/tests/support" /I"$root/upstream/shanjie/core/include" "$root/tests/submission.cpp" /link /OUT:"$exe" ole32.lib oleaut32.lib uuid.lib user32.lib gdi32.lib
"@
  Set-Content "$out/compile.cmd" $command -Encoding utf8NoBOM
  & cmd.exe /d /c "$out/compile.cmd" | Set-Content "$out/compile.log"
  if($LASTEXITCODE){throw "$arch submission harness build failed; see compile.log"}
  foreach($keyboard in 1,2) {
   $prefix=if($keyboard -eq 1){'SU3'}else{'NE3'}
   $cases=@(@('commit','ADB','adb'),@('commit','XYZ','xyz'),@('commit',$prefix,'你'),@('commit',($prefix+'ADB'),'你adb'),@('commit',('A'*96),('a'*96)),@('cancel','ADB',''),@('focus','ADB','adb'),@('toggle','ADB','adb'),@('refresh','ADB','adb'),@('return','ADB','adb'),@('cancel-generation','ADB',''),@('deactivate','ADB','adb'),@('refresh-twice','ADB','adb'),@('busy','A','adb'),@('busy-epoch','A','adb'),@('busy-cancel','A',''),@('busy-focus','A','a'),@('busy-expire','A','adb'),@('busy-pick',$prefix,'妳'),@('busy-mouse',$prefix,'妳'))
   foreach($case in $cases) {
    $testContent=$content -replace '(?m)^keyboard=.*',('keyboard='+$keyboard)
    if($case[0] -like 'busy*'){$testContent=$testContent -replace '(?m)^learning=.*','learning=1'}
    if($Modes -and $case[0] -notin $Modes){continue}
    $name="keyboard$keyboard-$($case[0])-$($case[1].Length)-$($results.Count)"
    $taskProfileRoot=Join-Path $out "$name/profile"
    New-Item -ItemType Directory -Path "$taskProfileRoot/Shanjie" -Force | Out-Null
    $env:LOCALAPPDATA=$taskProfileRoot
    Set-Content "$taskProfileRoot/Shanjie/profile.ini" $testContent -Encoding utf8NoBOM
    $start=[Diagnostics.ProcessStartInfo]::new($exe);$start.UseShellExecute=$false;$start.CreateNoWindow=$true;$start.RedirectStandardOutput=$true;$start.RedirectStandardError=$true
    $start.StandardOutputEncoding=[Text.UTF8Encoding]::new($false)
    foreach($arg in @("$DllRoot/$arch/Shanjie.dll")+$case){$start.ArgumentList.Add($arg)}
    $process=[Diagnostics.Process]::Start($start);$stdout=$process.StandardOutput.ReadToEndAsync();$stderr=$process.StandardError.ReadToEndAsync()
    if(!$process.WaitForExit(60000)){$process.Kill();throw 'Owned submission harness timed out'}
    $log=$stdout.GetAwaiter().GetResult();$errorLog=$stderr.GetAwaiter().GetResult();$code=$process.ExitCode;$process.Dispose()
    Set-Content "$out/$name.log" ($log+$errorLog) -Encoding utf8NoBOM
    if($code -or $log -notmatch 'PASS mode='){throw "$arch $name failed ($code): $log $errorLog"}
    $results += [pscustomobject]@{architecture=$arch;keyboard=$keyboard;mode=$case[0];keys=$case[1];expected=$case[2];passed=$true}
   }
  }
 }
 $results | ConvertTo-Json -Depth 4 | Set-Content "$ResultRoot/results.json" -Encoding utf8NoBOM
 "Passed $($results.Count) submission and delayed-context cases: $ResultRoot"
} finally {foreach($name in $saved.Keys){[Environment]::SetEnvironmentVariable($name,$saved[$name],'Process')}}
