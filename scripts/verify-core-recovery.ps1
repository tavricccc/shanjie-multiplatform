#Requires -Version 7
param([string]$ResultRoot,[ValidateSet('x64','x86')][string[]]$Architectures=@('x64','x86'),[string[]]$Cases,[string]$RuntimeRoot)
$ErrorActionPreference='Stop';$root=Split-Path $PSScriptRoot -Parent
if(!$RuntimeRoot){$RuntimeRoot=Join-Path $root 'dist'}
if(!$ResultRoot){$ResultRoot=Join-Path $root ('build/core-recovery-'+[guid]::NewGuid().ToString('N'))}
$ResultRoot=[IO.Path]::GetFullPath($ResultRoot)
New-Item -ItemType Directory -Path $ResultRoot -Force | Out-Null
$saved=@{};foreach($name in 'LOCALAPPDATA','SHANJIE_HEADLESS','SHANJIE_TEST_PROFILE','SHANJIE_CORE_LOCAL','SHANJIE_TEST_FAULT','SHANJIE_TEST_FAULT_SEQUENCE','SHANJIE_TEST_FAULT_OP'){$saved[$name]=[Environment]::GetEnvironmentVariable($name,'Process')}
function Invoke-Owned([string]$Executable,[string[]]$Arguments,[switch]$AllowFailure) {
 $start=[Diagnostics.ProcessStartInfo]::new($Executable);$start.UseShellExecute=$false;$start.CreateNoWindow=$true;$start.RedirectStandardOutput=$true;$start.RedirectStandardError=$true;$start.StandardOutputEncoding=[Text.UTF8Encoding]::new($false)
 foreach($argument in $Arguments){$start.ArgumentList.Add($argument)}
 $process=[Diagnostics.Process]::Start($start);$stdout=$process.StandardOutput.ReadToEndAsync();$stderr=$process.StandardError.ReadToEndAsync()
 if(!$process.WaitForExit(60000)){$process.Kill($true);throw 'Owned core check timed out'}
 $log=$stdout.GetAwaiter().GetResult()+$stderr.GetAwaiter().GetResult();$code=$process.ExitCode;$process.Dispose()
 if($code -and !$AllowFailure){throw "Core check failed ($code): $log"};$log
}
$results=@()
try {
 $env:SHANJIE_HEADLESS='1';$env:SHANJIE_TEST_PROFILE='1';$env:SHANJIE_CORE_LOCAL=$null
 foreach($arch in $Architectures) {
  $target=if($arch -eq 'x64'){'x86_64-pc-windows-msvc'}else{'i686-pc-windows-msvc'}
  $exe="$root/windows/target/$target/release/shanjie-reliability-check.exe"
  $checks=@(@('pipe',''),@('fault','drop-reply'),@('fault','before-apply'),@('fault','after-apply'),@('busy',''),@('restart',''),@('ambiguous','drop-reply'),@('rerank','drop-reply'),@('rerank','before-apply'),@('rerank','after-apply'),@('rerank-unknown','drop-reply'),@('outstanding','drop-reply'),@('capacity',''))
  foreach($check in 'learning-status','privacy'){if($check -in $Cases){$checks+=@(@($check,'local'),@($check,'pipe'))}}
  if('learning-status-ui' -in $Cases){$checks+=,@('learning-status-ui','')}
  if('pack-switch' -in $Cases){$checks+=@(@('pack-switch','sync-local'),@('pack-switch','background-local'),@('pack-switch','sync-pipe'),@('pack-switch','background-pipe'))}
  foreach($case in $checks) {
   if($Cases -and $case[0] -notin $Cases){continue}
   $label="$arch-$($case[0])-$($case[1])";$folder=Join-Path $ResultRoot $label
   if(Test-Path -LiteralPath $folder){throw 'Use a fresh result directory to isolate this core'}
   New-Item -ItemType Directory -Path "$folder/profile/Shanjie" -Force | Out-Null
   $isPackSwitch=$case[0] -eq 'pack-switch'
   if($isPackSwitch){
    # The resident process preloads its profile. Give both strategies the same starting pack.
    (Get-Content "$root/profiles/windows.ini" -Raw) -replace '(?m)^acg_pack=.*','acg_pack=0' | Set-Content "$folder/profile/Shanjie/profile.ini" -Encoding utf8NoBOM
   }else{Copy-Item "$root/profiles/windows.ini" "$folder/profile/Shanjie/profile.ini"}
   $env:SHANJIE_CORE_LOCAL=if($isPackSwitch -and $case[1] -like '*-local'){'1'}else{$null}
   $env:LOCALAPPDATA="$folder/profile";$env:SHANJIE_TEST_FAULT=if($isPackSwitch){$null}else{$case[1]};$env:SHANJIE_TEST_FAULT_OP=if($case[0] -like 'rerank*'){'rerank'}else{$null};$env:SHANJIE_TEST_FAULT_SEQUENCE=if($case[0] -eq 'ambiguous'){'8'}else{'5'}
   try {
    $arguments=@("$RuntimeRoot/data",$case[0]);if($isPackSwitch){$arguments+=($case[1] -split '-')[0]}elseif($case[0] -in @('learning-status','privacy')){$arguments+=$case[1];$env:SHANJIE_TEST_FAULT=$null}elseif($case[0] -eq 'learning-status-ui'){$env:SHANJIE_TEST_FAULT=$null}
    $log=Invoke-Owned $exe $arguments
    Set-Content "$folder/result.log" $log -Encoding utf8NoBOM
    if($log -notmatch 'PASS '){throw "Missing verification result: $label"}
    $results += [pscustomobject]@{architecture=$arch;case=$case[0];fault=$case[1];passed=$true}
   } catch {Set-Content "$folder/failure.log" $_.ToString() -Encoding utf8NoBOM;throw} finally {if(!$env:SHANJIE_CORE_LOCAL){Invoke-Owned "$RuntimeRoot/tools/shanjie-core.exe" @('--stop') -AllowFailure | Out-Null}}
  }
 }
 $results | ConvertTo-Json | Set-Content "$ResultRoot/results.json" -Encoding utf8NoBOM
 "Passed $($results.Count) deadline, delivery, contention and capacity checks: $ResultRoot"
} finally {foreach($name in $saved.Keys){[Environment]::SetEnvironmentVariable($name,$saved[$name],'Process')}}
