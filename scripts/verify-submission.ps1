#Requires -Version 7
param([string]$ResultRoot,[string]$DllRoot,[ValidateSet('x64','x86')][string[]]$Architectures=@('x64','x86'),[string[]]$Modes,[ValidateSet(1,2)][int[]]$Keyboards=@(1,2),[string]$HarnessPath,[string]$PayloadRoot,[int]$TimeoutSeconds=0)
$ErrorActionPreference='Stop'
Import-Module "$PSScriptRoot/test-support.psm1" -ErrorAction Stop
Import-Module "$PSScriptRoot/test-plan.psm1" -ErrorAction Stop
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
  New-Item -ItemType Junction -Path "$payload/data" -Target "$DllRoot/data" | Out-Null
  Copy-TestTools -Source "$DllRoot/tools" -Destination "$payload/tools"
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
   Invoke-Timed -Name "Compile submission harness ($arch)" -Category 'build.cpp.harness' -Details @{architecture=$arch;source='tests/submission.cpp'} -Action { Invoke-HarnessBuild $command "$out/compile.cmd" $exe "$out/compile.log";if($LASTEXITCODE){throw "$arch submission harness build failed; see compile.log"} }
  }
  foreach($keyboard in $Keyboards) {
   $cases=Get-SubmissionCases $keyboard
   foreach($case in $cases) {
    $testContent=$content -replace '(?m)^keyboard=.*',('keyboard='+$keyboard)
    if($case.Count -gt 3) {
     $testContent=$testContent -replace '(?m)^mixed_input=.*',"mixed_input=$([int](($case[3]-band 1)-ne 0))" -replace '(?m)^upstream_english=.*',"upstream_english=$([int](($case[3]-band 2)-ne 0))"
    }
    if($case[0] -like 'busy*'){$testContent=$testContent -replace '(?m)^learning=.*','learning=1'}
    $interaction=if($case.Count -gt 4){$case[4]}else{'windows'}
    if($case[0] -eq 'async-keys'){
     $testContent=$testContent -replace '(?m)^interaction=.*',"interaction=$interaction" -replace '(?m)^right_action=.*','right_action=none'
     if($interaction -eq 'shanjie'){$testContent=$testContent -replace '(?m)^prediction=.*','prediction=0'}
    }
    if($case[0] -eq 'segment'){
     $testContent=$testContent -replace '(?m)^mixed_input=.*','mixed_input=0' -replace '(?m)^prediction=.*','prediction=0'
     if($keyboard -eq 2){$testContent=$testContent -replace '(?m)^anchor=.*','anchor=left'}
    }
    if($Modes -and $case[0] -notin $Modes){continue}
    $pipe=$case[0] -eq 'rebuild-pipe'
    $env:SHANJIE_CORE_LOCAL=if($pipe){$null}else{'1'}
    $name="keyboard$keyboard-$($case[0])-$($case[1].Length)-$($results.Count)"
    $taskProfileRoot=Join-Path $out "$name/profile"
    New-Item -ItemType Directory -Path "$taskProfileRoot/Shanjie" -Force | Out-Null
    $env:LOCALAPPDATA=$taskProfileRoot
    if($case[0] -like 'rebuild-*') {
     $testContent=$testContent -replace '(?m)^mixed_input=.*','mixed_input=0' -replace '(?m)^acg_pack=.*','acg_pack=0' -replace '(?m)^learning=.*','learning=1'
     if($testContent -match '(?m)^dictionary='){$testContent=$testContent -replace '(?m)^dictionary=.*','dictionary=personal.tsv'}else{$testContent=$testContent -replace '(?m)^\[input\]\r?$',"[input]`ndictionary=personal.tsv"}
     Set-Content "$taskProfileRoot/Shanjie/personal.tsv" 'ㄋㄧˇ 龘 -1' -Encoding utf8NoBOM
     $personalHash=(Get-FileHash "$taskProfileRoot/Shanjie/personal.tsv").Hash
     $date=[DateTimeOffset]::UtcNow.ToUnixTimeSeconds()/86400
     $learnedRow="測試`tㄋㄧˇ`t妳`t3`t$([int][Math]::Floor($date))"
     [IO.File]::WriteAllText("$taskProfileRoot/Shanjie/learning.tsv","#shanjie-learning v1`n$learnedRow`n")
    }
    Set-Content "$taskProfileRoot/Shanjie/profile.ini" $testContent -Encoding utf8NoBOM
    $start=[Diagnostics.ProcessStartInfo]::new($exe);$start.UseShellExecute=$false;$start.CreateNoWindow=$true;$start.RedirectStandardOutput=$true;$start.RedirectStandardError=$true
    $start.StandardOutputEncoding=[Text.UTF8Encoding]::new($false)
    $start.StandardErrorEncoding=$start.StandardOutputEncoding
    foreach($arg in @("$DllRoot/$arch/Shanjie.dll")+$case[0..2]){$start.ArgumentList.Add($arg)}
    $caseStage=Start-TestStage -Name "Submission case $arch/$name" -Category 'submission.case' -Details @{architecture=$arch;keyboard=$keyboard;mode=$case[0];mix=if($case.Count -gt 3){$case[3]}else{1};interaction=$interaction;keys=$case[1];expected=$case[2]}
    $timeoutMs=1000 * $(if($TimeoutSeconds -gt 0){$TimeoutSeconds}elseif($pipe){120}else{60})
    $process=$null;$worker=$null
    try {
      try {
      if($pipe){
       # The DLL does not auto-start a core from elevated hosts, including CI.
       $corePath="$DllRoot/tools/shanjie-core.exe"
       $worker=Invoke-Timed -Name 'Start isolated submission core' -Category 'core.lifecycle' -Action {
        Start-Process $corePath -ArgumentList '--serve' -WindowStyle Hidden -RedirectStandardOutput "$out/$name.core.out.log" -RedirectStandardError "$out/$name.core.err.log" -PassThru
       }
       Wait-TestCoreReady -CorePath $corePath -OutputDirectory (Join-Path $out $name) | Out-Null
      }
      $process=[Diagnostics.Process]::Start($start)
      $stdout=$process.StandardOutput.ReadToEndAsync()
      $stderr=$process.StandardError.ReadToEndAsync()
      $timedOut=!$process.WaitForExit($timeoutMs)
      if($timedOut){
       if(!$process.HasExited){$process.Kill($true)}
       $process.WaitForExit()
      }
      $log=$stdout.GetAwaiter().GetResult();$errorLog=$stderr.GetAwaiter().GetResult();$code=$process.ExitCode
      Set-Content "$out/$name.log" "$log`n--- stderr ---`n$errorLog" -Encoding utf8NoBOM
      if($timedOut){
       $lastStage=($log -split '\r?\n' -match '^STAGE ' | Select-Object -Last 1) ?? '<none>'
       $tail=($log -split '\r?\n' | Select-Object -Last 60) -join "`n"
       $timeoutLog="$out/$name.timeout.log"
       $detail="Owned submission harness timed out ($arch/$name after ${timeoutMs}ms). Last stage: $lastStage"
       Set-Content $timeoutLog "$detail`n--- stdout tail ---`n$tail`n--- stderr ---`n$errorLog" -Encoding utf8NoBOM
       throw "$detail See $timeoutLog"
      }
      if($code -or $log -notmatch 'PASS mode='){throw "$arch $name failed ($code): $log $errorLog"}
      if($worker){
       & $corePath --stop *> "$out/$name.core.stop.log"
       if($LASTEXITCODE -or !$worker.WaitForExit(5000)){throw 'Isolated submission core did not stop cleanly.'}
      }
      if($case[0] -like 'rebuild-*') {
       if((Get-FileHash "$taskProfileRoot/Shanjie/personal.tsv").Hash -ne $personalHash -or $learnedRow -notin (Get-Content "$taskProfileRoot/Shanjie/learning.tsv")) {throw 'Rebuild changed the personal dictionary or lost an existing learning row'}
      }
     } finally {
      if($process){$process.Dispose()}
      if($worker){
       if(!$worker.HasExited){$worker.Kill();$worker.WaitForExit()}
       $worker.Dispose()
      }
     }
     Complete-TestStage -Stage $caseStage
    } catch { Complete-TestStage -Stage $caseStage -Status 'failed' -ErrorMessage $_.Exception.Message;throw }
    $results += [pscustomobject]@{architecture=$arch;keyboard=$keyboard;mode=$case[0];mix=if($case.Count -gt 3){$case[3]}else{1};interaction=$interaction;keys=$case[1];expected=$case[2];passed=$true}
   }
  }
 }
 $results | ConvertTo-Json -Depth 4 | Set-Content "$ResultRoot/results.json" -Encoding utf8NoBOM
 "Passed $($results.Count) submission and delayed-context cases: $ResultRoot"
} finally {foreach($name in $saved.Keys){[Environment]::SetEnvironmentVariable($name,$saved[$name],'Process')}}
