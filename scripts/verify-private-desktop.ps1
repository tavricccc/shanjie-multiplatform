#Requires -Version 7
param([Parameter(Mandatory)][string]$Payload,[Parameter(Mandatory)][string]$Harness,[Parameter(Mandatory)][string]$Output,[string[]]$Cases=@('shift-right','shift-left','shift-unknown','shift-proxy-enter','shift-enter','shift-retain-windows','shift-retain-shanjie','shift-finish-eten',
 'handoff-same','handoff-timeout','handoff-generation','handoff-context','handoff-different','cmd-commit',
 'status-writable-disjoint','status-writable-regions','status-readonly','status-readonly-disjoint',
 'word-deferred','word-natural','word-disjoint','word-stale','word-readonly','word-password','word-invalid','word-empty',
 'modifiers-empty','modifiers-composing','modifiers-candidates','modifiers-english','spacing-english','spacing-english-off','punctuation-mcbopomofo','range-refresh','privacy-normal','privacy-private','privacy-password','privacy-pin','privacy-native-password','privacy-disabled','privacy-transition'))
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
Import-Module "$PSScriptRoot/test-support.psm1" -ErrorAction Stop
$null=Initialize-TestTimings -ProjectRoot $root -Name 'private-desktop'
if(Test-Path -LiteralPath $Output){throw 'Use a fresh fixture output directory.'}
New-Item -ItemType Directory -Path $Output -Force|Out-Null
$Payload=[IO.Path]::GetFullPath($Payload)
$results=@()
$privacyCases=@($Cases|Where-Object{$_ -like 'privacy-*' -and $_ -notin @('privacy-control','privacy-disabled','privacy-transition')})
if($privacyCases.Count){$Cases=@('privacy-control')+@($Cases|Where-Object{$_ -ne 'privacy-control'})}
$ownedServers=[Collections.Generic.List[object]]::new()
try {
 foreach($scenario in $Cases) {
  $caseStage=Start-TestStage -Name "Private-desktop TSF case $scenario" -Category 'tsf.private-desktop'
  $caseCompleted=$false
  try {
  $fixture=[IO.Path]::GetFullPath((Join-Path $Output $scenario))
  New-Item -ItemType Directory -Path "$fixture/Shanjie" -Force|Out-Null
  $shiftCase=$scenario -in 'shift-retain-windows','shift-retain-shanjie','shift-finish-eten'
  $preset=if($scenario -eq 'shift-retain-shanjie'){'shanjie'}else{'windows'}
  $profile=Get-Content -LiteralPath "$root/profiles/$preset.ini" -Raw -Encoding utf8
  $profile=$profile -replace '(?m)^learning=1','learning=0' -replace '(?m)^mixed_input=.*',"mixed_input=$([int](!$shiftCase))" -replace '(?m)^upstream_english=.*','upstream_english=0'
  if($shiftCase){
   $keyboard=if($scenario -eq 'shift-finish-eten'){2}else{1}
   $profile=$profile -replace '(?m)^keyboard=.*',"keyboard=$keyboard"
  }
  if($scenario -eq 'shift-retain-shanjie'){$profile=$profile -replace '(?m)^cjk_spacing=0','cjk_spacing=1'}
  if($scenario -eq 'punctuation-mcbopomofo'){
   $profile=$profile -replace '(?m)^style=apple','style=mcbopomofo' -replace '(?m)^keyboard=.*','keyboard=1' -replace '(?m)^(shift_1|shift_quote|plain_quote|plain_backslash)=0','${1}=1'
  }
  if($scenario -like 'privacy-*'){$profile=$profile -replace '(?m)^learning=0','learning=1'}
  if($scenario -eq 'privacy-control'){$profile=$profile -replace '(?m)^context=1','context=0'}
  if($scenario -eq 'spacing-english'){$profile=$profile -replace '(?m)^cjk_spacing=0','cjk_spacing=1' -replace '(?m)^context=1','context=0'}
  if($scenario -eq 'modifiers-english'){$profile=$profile -replace '(?m)^upstream_english=0','upstream_english=1'}
  if($scenario -eq 'cancel-theme-dark'){$profile=$profile -replace '(?m)^theme=0','theme=2'}
  if($scenario -eq 'cancel-theme-light'){$profile=$profile -replace '(?m)^theme=0','theme=1'}
  Set-Content -LiteralPath "$fixture/Shanjie/profile.ini" -Value $profile -Encoding utf8NoBOM
  $start=[Diagnostics.ProcessStartInfo]::new([IO.Path]::GetFullPath($Harness))
  $start.UseShellExecute=$false;$start.CreateNoWindow=$true
  $start.RedirectStandardOutput=$true;$start.RedirectStandardError=$true
  $start.Environment['LOCALAPPDATA']=$fixture
  New-Item -ItemType Directory -Path "$fixture/temp" -Force|Out-Null
  $start.Environment['TEMP']="$fixture/temp";$start.Environment['TMP']="$fixture/temp"
  foreach($key in @('SHANJIE_TEST_PROFILE','SHANJIE_HEADLESS')){$start.Environment[$key]='1'}
  $residentCase=$scenario -in 'shift-retain-shanjie','punctuation-mcbopomofo'
  if($residentCase){[void]$start.Environment.Remove('SHANJIE_CORE_LOCAL')}else{$start.Environment['SHANJIE_CORE_LOCAL']='1'}
  $start.Environment['SHANJIE_TEST_ROOT']=$fixture
  $arch=if($Harness -match 'x86'){'x86'}else{'x64'}
  $start.ArgumentList.Add("$Payload/$arch/Shanjie.dll");$start.ArgumentList.Add($scenario)
  $server=$null;$child=$null
   if($residentCase -or $scenario -in 'word-deferred','word-natural','word-disjoint' -or $scenario -like 'cancel-*'){
   $serverStart=[Diagnostics.ProcessStartInfo]::new("$Payload/tools/shanjie-core.exe")
   $serverStart.UseShellExecute=$false;$serverStart.CreateNoWindow=$true
   foreach($key in @('LOCALAPPDATA','SHANJIE_TEST_PROFILE','SHANJIE_TEST_ROOT','SHANJIE_HEADLESS')){$serverStart.Environment[$key]=$start.Environment[$key]}
   $serverStart.ArgumentList.Add('--serve');$server=[Diagnostics.Process]::Start($serverStart);$ownedServers.Add($server)
   $probeEnvironment=@{}
   foreach($key in @('LOCALAPPDATA','SHANJIE_TEST_PROFILE','SHANJIE_TEST_ROOT')){$probeEnvironment[$key]=[Environment]::GetEnvironmentVariable($key);[Environment]::SetEnvironmentVariable($key,$start.Environment[$key])}
   try{Wait-TestCoreReady -CorePath "$Payload/tools/shanjie-core.exe" -OutputDirectory $fixture | Out-Null}
   finally{foreach($key in $probeEnvironment.Keys){[Environment]::SetEnvironmentVariable($key,$probeEnvironment[$key])}}
   }
  $child=[Diagnostics.Process]::Start($start)
  $stdout=$child.StandardOutput.ReadToEndAsync();$stderr=$child.StandardError.ReadToEndAsync()
  if(!$child.WaitForExit(25000)){
   try{$child.Kill();$child.WaitForExit(5000)|Out-Null}catch{}
   throw "Owned fixture timed out: $scenario"
  }
  $log=$stdout.GetAwaiter().GetResult()+$stderr.GetAwaiter().GetResult();$code=$child.ExitCode;$child.Dispose()
  $child=$null
  Set-Content -LiteralPath "$fixture/result.log" -Value $log -Encoding utf8NoBOM
  $prefixReads=if($log -match 'privacy prefix_reads=(\d+)'){[int]$Matches[1]}else{$null}
  $readsBeforeWrite=if($log -match 'prewrite_prefix_reads=(\d+)'){[int]$Matches[1]}else{$null}
  $results+=[pscustomobject]@{scenario=$scenario;exit=$code;prefix_reads=$prefixReads;prewrite_prefix_reads=$readsBeforeWrite}
  Write-Output $log.Trim()
  if($server){if(!$server.HasExited){$server.Kill();if(!$server.WaitForExit(5000)){throw 'Owned fixture core did not exit.'}};$server.Dispose();[void]$ownedServers.Remove($server)}
  if($code){throw "Compatibility fixture failed: $scenario ($code)"}
   Complete-TestStage -Stage $caseStage -Details @{scenario=$scenario;architecture=$arch;profile=$fixture;explicit_core=[bool]$server}
  $caseCompleted=$true
  } catch {
   if(!$caseCompleted){Complete-TestStage -Stage $caseStage -Status 'failed' -Details @{scenario=$scenario} -ErrorMessage $_.Exception.Message}
   throw
  } finally {
   if($child){
    try{if(!$child.HasExited){$child.Kill();$child.WaitForExit(5000)|Out-Null}}finally{$child.Dispose()}
   }
  }
 }
} finally {
 $results|ConvertTo-Json|Set-Content -LiteralPath "$Output/results.json" -Encoding utf8NoBOM
 # Retire only Process objects this invocation launched; never scan or stop a same-path user process.
 foreach($worker in $ownedServers){
  try{if(!$worker.HasExited){$worker.Kill();if(!$worker.WaitForExit(5000)){throw 'Owned fixture core did not exit.'}}}
  finally{$worker.Dispose()}
 }
}
