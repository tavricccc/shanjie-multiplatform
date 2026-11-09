#Requires -Version 7
param([string]$ResultRoot,[string]$DllRoot,[ValidateSet('x64','x86')][string[]]$Architectures=@('x64','x86'),[string[]]$Modes,[ValidateSet(1,2)][int[]]$Keyboards=@(1,2),[string]$HarnessPath,[string]$PayloadRoot)
$ErrorActionPreference='Stop'
Import-Module "$PSScriptRoot/test-support.psm1" -ErrorAction Stop
. "$PSScriptRoot/msvc-command.ps1"
$root=Split-Path $PSScriptRoot -Parent
$null=Initialize-TestTimings -ProjectRoot $root -Name 'submission'
if(!$ResultRoot){$ResultRoot=Join-Path $root ('build/submission-'+[guid]::NewGuid().ToString('N'))}
$ResultRoot=[IO.Path]::GetFullPath($ResultRoot)
New-Item -ItemType Directory -Path $ResultRoot -Force | Out-Null
if($PayloadRoot){
 $DllRoot=[IO.Path]::GetFullPath($PayloadRoot)
 foreach($arch in $Architectures){if(!(Test-Path -LiteralPath "$DllRoot/$arch/Shanjie.dll")){throw "Validated CTest payload is missing $arch/Shanjie.dll."}}
 foreach($name in @('data','tools')){if(!(Test-Path -LiteralPath "$DllRoot/$name")){throw "Validated CTest payload is missing $name."}}
 if(!(Test-Path -LiteralPath "$DllRoot/installed.json")){throw 'Validated CTest payload is missing installed.json.'}
}else{
 if(!$DllRoot){$DllRoot=Join-Path $root 'dist'}
 $DllRoot=[IO.Path]::GetFullPath($DllRoot)
 $payload = Join-Path $ResultRoot 'payload'
 foreach ($arch in $Architectures) {
  New-Item -ItemType Directory -Path "$payload/$arch" -Force | Out-Null
  Copy-Item "$DllRoot/$arch/Shanjie.dll" "$payload/$arch" -Force
 }
 foreach ($name in 'data','tools') { New-Item -ItemType Junction -Path "$payload/$name" -Target "$DllRoot/$name" | Out-Null }
 # Test both layouts independently of the user's installed keyboard selection.
 Set-Content "$payload/installed.json" '{"version":"test","keyboards":3}' -Encoding utf8NoBOM
 $DllRoot = $payload
}
$taskProfileRoot=Join-Path $ResultRoot 'profile'
New-Item -ItemType Directory -Path "$taskProfileRoot/Shanjie" -Force | Out-Null
$content=(Get-Content "$root/profiles/windows.ini" -Raw) -replace '(?m)^mixed_input=.*','mixed_input=1' -replace '(?m)^upstream_english=.*','upstream_english=0' -replace '(?m)^learning=.*','learning=0' -replace '(?m)^prediction=.*','prediction=1' -replace '(?m)^caps_lock_english=.*','caps_lock_english=0' -replace '(?m)^shift_side=.*','shift_side=2' -replace '(?m)^escape=.*','escape=clear'
$saved=@{};foreach($name in 'LOCALAPPDATA','SHANJIE_HEADLESS','SHANJIE_TEST_PROFILE','SHANJIE_CORE_LOCAL'){$saved[$name]=[Environment]::GetEnvironmentVariable($name,'Process')}
$results=@()
try {
 $env:LOCALAPPDATA=$taskProfileRoot;$env:SHANJIE_HEADLESS='1';$env:SHANJIE_TEST_PROFILE='1';$env:SHANJIE_CORE_LOCAL='1'
 foreach($arch in $Architectures) {
  $out=Join-Path $ResultRoot $arch;New-Item -ItemType Directory -Path $out -Force | Out-Null
  if ($HarnessPath) {
   if ($Architectures.Count -ne 1) { throw 'A prebuilt submission harness can only be used for one architecture per invocation.' }
   $exe=[IO.Path]::GetFullPath($HarnessPath)
   if (!(Test-Path -LiteralPath $exe)) { throw "Submission harness is missing: $exe" }
  } else {
   $exe=Join-Path $out 'submission.exe'
   $command=@"
@echo off
$(Get-MsvcSetup $arch)
if errorlevel 1 exit /b 1
cd /d "$out"
cl /nologo /std:c++20 /EHsc /O2 /MT /utf-8 /DUNICODE /D_UNICODE /DNOMINMAX /I"$root/tests/support" /I"$root/upstream/shanjie/core/include" "$root/tests/submission.cpp" /link /OUT:"$exe" ole32.lib oleaut32.lib uuid.lib user32.lib gdi32.lib
"@
   Set-Content "$out/compile.cmd" $command -Encoding utf8NoBOM
   Invoke-Timed -Name "Compile submission harness ($arch)" -Category 'build.cpp.harness' -Details @{architecture=$arch;source='tests/submission.cpp'} -Action { & cmd.exe /d /c "$out/compile.cmd" | Set-Content "$out/compile.log";if($LASTEXITCODE){throw "$arch submission harness build failed; see compile.log"} }
  }
  foreach($keyboard in $Keyboards) {
   $prefix=if($keyboard -eq 1){'SU3'}else{'NE3'}
   $cases=@(@('commit','ADB','adb'),@('commit','XYZ','xyz'),@('commit',$prefix,'你'),@('commit',($prefix+'ADB'),'你adb'),@('commit',('A'*96),('a'*96)),@('cancel','ADB',''),@('focus','ADB','adb'),@('toggle','ADB','adb'),@('refresh','ADB','adb'),@('return','ADB','adb'),@('cancel-generation','ADB',''),@('deactivate','ADB','adb'),@('refresh-twice','ADB','adb'),@('busy','A','adb'),@('busy-epoch','A','adb'),@('busy-cancel','A',''),@('busy-focus','A','a'),@('busy-expire','A','adb'),@('busy-pick',$prefix,'妳'),@('busy-mouse',$prefix,'妳'),@('retire-write-fail','ADB','adb'),@('retire-request-fail','ADB','adb'),@('retire-escape','ADB','adb'),@('retire-deactivate','ADB','adb'),@('retire-stale','ADB','adb'))
   if($keyboard -eq 1) {
    # Mix bits: 1 raw-key mixing, 2 upstream English; one choice, so never both.
    foreach($mix in 0,1,2) {
     $cases+=,@('input-digits','9 U. ','哎呦',$mix)
     $cases+=,@('input-digits','GK42U/4','設定',$mix)
     $cases+=,@('input-digits','9 ⎋GK42U/4','設定',$mix)
     if(!($mix -band 2)) { $cases+=,@('input-digits','0D4','看',$mix) }
     if($mix) { $cases+=,@('input-digits','13 ','13 ',$mix) }
     if($mix -band 1) {
      foreach($digits in '13','14','16','17','13467','R3','R4','R6','R7','R3467') {
       $cases+=,@('input-digits',$digits,$digits.ToLowerInvariant(),$mix)
      }
      $cases+=,@('input-digits','GK42U⌫U/4','設定',$mix)
     }
     if($mix -band 2) {
      foreach($digits in '3.14','0.5','2026/10/15') {$cases+=,@('input-digits',$digits,$digits,$mix)}
     }
    }
   }
   foreach($case in $cases) {
    $testContent=$content -replace '(?m)^keyboard=.*',('keyboard='+$keyboard)
    if($case.Count -gt 3) {
     $testContent=$testContent -replace '(?m)^mixed_input=.*',"mixed_input=$([int](($case[3]-band 1)-ne 0))" -replace '(?m)^upstream_english=.*',"upstream_english=$([int](($case[3]-band 2)-ne 0))"
    }
    if($case[0] -like 'busy*'){$testContent=$testContent -replace '(?m)^learning=.*','learning=1'}
    if($Modes -and $case[0] -notin $Modes){continue}
    $name="keyboard$keyboard-$($case[0])-$($case[1].Length)-$($results.Count)"
    $taskProfileRoot=Join-Path $out "$name/profile"
    New-Item -ItemType Directory -Path "$taskProfileRoot/Shanjie" -Force | Out-Null
    $env:LOCALAPPDATA=$taskProfileRoot
    Set-Content "$taskProfileRoot/Shanjie/profile.ini" $testContent -Encoding utf8NoBOM
    $start=[Diagnostics.ProcessStartInfo]::new($exe);$start.UseShellExecute=$false;$start.CreateNoWindow=$true;$start.RedirectStandardOutput=$true;$start.RedirectStandardError=$true
    $start.StandardOutputEncoding=[Text.UTF8Encoding]::new($false)
    foreach($arg in @("$DllRoot/$arch/Shanjie.dll")+$case[0..2]){$start.ArgumentList.Add($arg)}
    $caseStage=Start-TestStage -Name "Submission case $arch/$name" -Category 'submission.case'
    try {
     $process=[Diagnostics.Process]::Start($start);$stdout=$process.StandardOutput.ReadToEndAsync();$stderr=$process.StandardError.ReadToEndAsync()
     if(!$process.WaitForExit(60000)){$process.Kill();throw 'Owned submission harness timed out'}
     $log=$stdout.GetAwaiter().GetResult();$errorLog=$stderr.GetAwaiter().GetResult();$code=$process.ExitCode;$process.Dispose()
     Set-Content "$out/$name.log" ($log+$errorLog) -Encoding utf8NoBOM
     if($code -or $log -notmatch 'PASS mode='){throw "$arch $name failed ($code): $log $errorLog"}
     Complete-TestStage -Stage $caseStage -Details @{architecture=$arch;keyboard=$keyboard;mode=$case[0];mix=if($case.Count -gt 3){$case[3]}else{1}}
    } catch { Complete-TestStage -Stage $caseStage -Status 'failed' -ErrorMessage $_.Exception.Message;throw }
    $results += [pscustomobject]@{architecture=$arch;keyboard=$keyboard;mode=$case[0];mix=if($case.Count -gt 3){$case[3]}else{1};keys=$case[1];expected=$case[2];passed=$true}
   }
  }
 }
 $results | ConvertTo-Json -Depth 4 | Set-Content "$ResultRoot/results.json" -Encoding utf8NoBOM
 "Passed $($results.Count) submission and delayed-context cases: $ResultRoot"
} finally {foreach($name in $saved.Keys){[Environment]::SetEnvironmentVariable($name,$saved[$name],'Process')}}
