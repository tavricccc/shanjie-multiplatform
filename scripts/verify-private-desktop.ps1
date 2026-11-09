#Requires -Version 7
param([Parameter(Mandatory)][string]$Payload,[Parameter(Mandatory)][string]$Harness,[Parameter(Mandatory)][string]$Output,[string[]]$Cases=@('shift-right','shift-left','shift-unknown','shift-proxy-enter','shift-enter',
 'handoff-same','handoff-timeout','handoff-generation','handoff-context','handoff-different','cmd-commit',
 'word-deferred','word-natural','word-stale','word-readonly','word-password','word-invalid','word-empty'))
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
Import-Module "$PSScriptRoot/test-support.psm1" -ErrorAction Stop
$null=Initialize-TestTimings -ProjectRoot $root -Name 'private-desktop'
if(Test-Path -LiteralPath $Output){throw 'Use a fresh fixture output directory.'}
New-Item -ItemType Directory -Path $Output -Force|Out-Null
$Payload=[IO.Path]::GetFullPath($Payload)
$results=@()
$ownedServers=[Collections.Generic.List[object]]::new()
try {
 foreach($scenario in $Cases) {
  $caseStage=Start-TestStage -Name "Private-desktop TSF case $scenario" -Category 'tsf.private-desktop'
  $caseCompleted=$false
  try {
  $fixture=[IO.Path]::GetFullPath((Join-Path $Output $scenario))
  New-Item -ItemType Directory -Path "$fixture/Shanjie" -Force|Out-Null
  $profile=Get-Content -LiteralPath "$root/profiles/windows.ini" -Raw -Encoding utf8
  $profile=$profile -replace '(?m)^learning=1','learning=0' -replace '(?m)^mixed_input=0','mixed_input=1' -replace '(?m)^upstream_english=.*','upstream_english=0'
  if($scenario -eq 'cancel-theme-dark'){$profile=$profile -replace '(?m)^theme=0','theme=2'}
  if($scenario -eq 'cancel-theme-light'){$profile=$profile -replace '(?m)^theme=0','theme=1'}
  Set-Content -LiteralPath "$fixture/Shanjie/profile.ini" -Value $profile -Encoding utf8NoBOM
  $start=[Diagnostics.ProcessStartInfo]::new([IO.Path]::GetFullPath($Harness))
  $start.UseShellExecute=$false;$start.CreateNoWindow=$true
  $start.RedirectStandardOutput=$true;$start.RedirectStandardError=$true
  $start.Environment['LOCALAPPDATA']=$fixture
  New-Item -ItemType Directory -Path "$fixture/temp" -Force|Out-Null
  $start.Environment['TEMP']="$fixture/temp";$start.Environment['TMP']="$fixture/temp"
  foreach($key in @('SHANJIE_TEST_PROFILE','SHANJIE_HEADLESS','SHANJIE_CORE_LOCAL')){$start.Environment[$key]='1'}
  $start.Environment['SHANJIE_TEST_ROOT']=$fixture
  $arch=if($Harness -match 'x86'){'x86'}else{'x64'}
  $start.ArgumentList.Add("$Payload/$arch/Shanjie.dll");$start.ArgumentList.Add($scenario)
  $server=$null;$child=$null
   $serverStart=[Diagnostics.ProcessStartInfo]::new("$Payload/tools/shanjie-core.exe")
   $serverStart.UseShellExecute=$false;$serverStart.CreateNoWindow=$true
   foreach($key in @('LOCALAPPDATA','SHANJIE_TEST_PROFILE','SHANJIE_TEST_ROOT','SHANJIE_HEADLESS')){$serverStart.Environment[$key]=$start.Environment[$key]}
   $serverStart.ArgumentList.Add('--serve');$server=[Diagnostics.Process]::Start($serverStart);$ownedServers.Add($server)
   $probeEnvironment=@{}
   foreach($key in @('LOCALAPPDATA','SHANJIE_TEST_PROFILE','SHANJIE_TEST_ROOT')){$probeEnvironment[$key]=[Environment]::GetEnvironmentVariable($key);[Environment]::SetEnvironmentVariable($key,$start.Environment[$key])}
   try{Wait-TestCoreReady -CorePath "$Payload/tools/shanjie-core.exe" -OutputDirectory $fixture | Out-Null}
   finally{foreach($key in $probeEnvironment.Keys){[Environment]::SetEnvironmentVariable($key,$probeEnvironment[$key])}}
  $child=[Diagnostics.Process]::Start($start)
  $stdout=$child.StandardOutput.ReadToEndAsync();$stderr=$child.StandardError.ReadToEndAsync()
  if(!$child.WaitForExit(25000)){
   try{$child.Kill();$child.WaitForExit(5000)|Out-Null}catch{}
   throw "Owned fixture timed out: $scenario"
  }
  $log=$stdout.GetAwaiter().GetResult()+$stderr.GetAwaiter().GetResult();$code=$child.ExitCode;$child.Dispose()
  $child=$null
  Set-Content -LiteralPath "$fixture/result.log" -Value $log -Encoding utf8NoBOM
  $results+=[pscustomobject]@{scenario=$scenario;exit=$code}
  Write-Output $log.Trim()
  if($server){if(!$server.HasExited){$server.Kill();if(!$server.WaitForExit(5000)){throw 'Owned fixture core did not exit.'}};$server.Dispose();[void]$ownedServers.Remove($server)}
  if($code){throw "Compatibility fixture failed: $scenario ($code)"}
  Complete-TestStage -Stage $caseStage -Details @{scenario=$scenario;architecture=$arch;profile=$fixture;explicit_core=$true}
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
