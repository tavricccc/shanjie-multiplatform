#Requires -Version 7
param([Parameter(Mandatory)][string]$Payload,[Parameter(Mandatory)][string]$Harness,[Parameter(Mandatory)][string]$Output,[string[]]$Cases=@('shift-right','shift-left','shift-unknown','shift-proxy-enter','shift-enter',
 'handoff-same','handoff-timeout','handoff-generation','handoff-context','handoff-different','cmd-commit',
 'word-deferred','word-natural','word-stale','word-readonly','word-password','word-invalid','word-empty'),[switch]$ExplicitCore)
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
if(!('ShanjieFixtureProcess' -as [type])) { Add-Type -TypeDefinition @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class ShanjieFixtureProcess {
 [DllImport("kernel32.dll",SetLastError=true)] static extern IntPtr OpenProcess(uint access,bool inherit,uint pid);
 [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern bool QueryFullProcessImageName(IntPtr handle,uint flags,StringBuilder path,ref uint size);
 [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr handle);
 [DllImport("kernel32.dll",SetLastError=true)] static extern bool TerminateProcess(IntPtr handle,uint code);
 [DllImport("kernel32.dll")] static extern uint WaitForSingleObject(IntPtr handle,uint timeout);
 static string PathOf(IntPtr handle) {var path=new StringBuilder(32768);uint size=32768;return QueryFullProcessImageName(handle,0,path,ref size)?path.ToString():null;}
 public static string Read(uint pid) {var h=OpenProcess(0x1000,false,pid);if(h==IntPtr.Zero)return null;try{return PathOf(h);}finally{CloseHandle(h);}}
 public static bool Retire(uint pid,string expected) {
  // Query on the same handle used for termination, so PID reuse cannot target
  // a different executable. Never request termination access to other paths.
  if(!String.Equals(Read(pid),expected,StringComparison.OrdinalIgnoreCase))return false;
  var h=OpenProcess(0x101001,false,pid);if(h==IntPtr.Zero)throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error());
  try {if(!String.Equals(PathOf(h),expected,StringComparison.OrdinalIgnoreCase))return false;
   if(!TerminateProcess(h,0))throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error());
   if(WaitForSingleObject(h,5000)!=0)throw new Exception("Fixture helper did not exit");return true;
  } finally {CloseHandle(h);}
 }
}
'@ }
if(Test-Path -LiteralPath $Output){throw 'Use a fresh fixture output directory.'}
New-Item -ItemType Directory -Path $Output -Force|Out-Null
$Payload=[IO.Path]::GetFullPath($Payload)
$results=@()
try {
 foreach($scenario in $Cases) {
  $fixture=[IO.Path]::GetFullPath((Join-Path $Output $scenario))
  New-Item -ItemType Directory -Path "$fixture/Shanjie" -Force|Out-Null
  $profile=Get-Content -LiteralPath "$root/profiles/windows.ini" -Raw -Encoding utf8
  $profile=$profile -replace '(?m)^learning=1','learning=0' -replace '(?m)^mixed_input=0','mixed_input=1'
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
  $server=$null
  if($ExplicitCore){
   $serverStart=[Diagnostics.ProcessStartInfo]::new("$Payload/tools/shanjie-core.exe")
   $serverStart.UseShellExecute=$false;$serverStart.CreateNoWindow=$true
   foreach($key in @('LOCALAPPDATA','SHANJIE_TEST_PROFILE','SHANJIE_TEST_ROOT')){$serverStart.Environment[$key]=$start.Environment[$key]}
   $serverStart.ArgumentList.Add('--serve');$server=[Diagnostics.Process]::Start($serverStart)
   Start-Sleep -Milliseconds 200
   if($server.HasExited){throw "Private fixture core exited: $($server.ExitCode)"}
  }
  $child=[Diagnostics.Process]::Start($start)
  $stdout=$child.StandardOutput.ReadToEndAsync();$stderr=$child.StandardError.ReadToEndAsync()
  if(!$child.WaitForExit(25000)){$child.Kill();throw "Owned fixture timed out: $scenario"}
  $log=$stdout.GetAwaiter().GetResult()+$stderr.GetAwaiter().GetResult();$code=$child.ExitCode;$child.Dispose()
  Set-Content -LiteralPath "$fixture/result.log" -Value $log -Encoding utf8NoBOM
  $results+=[pscustomobject]@{scenario=$scenario;exit=$code}
  Write-Output $log.Trim()
  if($server){[ShanjieFixtureProcess]::Retire($server.Id,[IO.Path]::GetFullPath("$Payload/tools/shanjie-core.exe"))|Out-Null;$server.Dispose()}
  if($code){throw "Compatibility fixture failed: $scenario ($code)"}
 }
} finally {
 $results|ConvertTo-Json|Set-Content -LiteralPath "$Output/results.json" -Encoding utf8NoBOM
 # Retire only helpers launched from this never-installed private fixture payload.
 $core=[IO.Path]::GetFullPath("$Payload/tools/shanjie-core.exe")
 foreach($worker in Get-Process -Name shanjie-core -ErrorAction SilentlyContinue){
  if([ShanjieFixtureProcess]::Read($worker.Id) -eq $core){
   [ShanjieFixtureProcess]::Retire($worker.Id,$core)|Out-Null
  }
 }
}
