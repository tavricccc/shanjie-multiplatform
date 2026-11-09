#Requires -Version 7
param([string]$ResultRoot,[ValidateSet('x64','x86')][string[]]$Architectures=@('x64','x86'),[string[]]$Scenarios,[string]$HarnessPath,[string]$PayloadRoot)
$ErrorActionPreference='Stop';$root=Split-Path $PSScriptRoot -Parent
Import-Module "$PSScriptRoot/test-support.psm1" -ErrorAction Stop
Import-Module "$PSScriptRoot/test-plan.psm1" -ErrorAction Stop
. "$PSScriptRoot/msvc-command.ps1"
$null=Initialize-TestTimings -ProjectRoot $root -Name 'english-keys'
if(!$ResultRoot){$ResultRoot=Join-Path $root ('build/english-keys-'+[guid]::NewGuid().ToString('N'))}
$ResultRoot=[IO.Path]::GetFullPath($ResultRoot);New-Item -ItemType Directory $ResultRoot -Force|Out-Null
$payload=if($PayloadRoot){[IO.Path]::GetFullPath($PayloadRoot)}else{Join-Path $ResultRoot 'payload'}
if($PayloadRoot){
 foreach($arch in $Architectures){if(!(Test-Path -LiteralPath "$payload/$arch/Shanjie.dll")){throw "Validated CTest payload is missing $arch/Shanjie.dll."}}
 foreach($name in @('data','tools')){if(!(Test-Path -LiteralPath "$payload/$name")){throw "Validated CTest payload is missing $name."}}
 if(!(Test-Path -LiteralPath "$payload/installed.json")){throw 'Validated CTest payload is missing installed.json.'}
}else{
 New-Item -ItemType Directory $payload -Force|Out-Null
 foreach($arch in $Architectures){New-Item -ItemType Directory "$payload/$arch" -Force|Out-Null;Copy-Item "$root/dist/$arch/Shanjie.dll" "$payload/$arch" -Force}
 foreach($name in 'data','tools'){New-Item -ItemType Junction -Path "$payload/$name" -Target "$root/dist/$name"|Out-Null}
 Set-Content "$payload/installed.json" '{"version":"test","keyboards":3}' -Encoding utf8NoBOM
}
$saved=@{};foreach($name in 'LOCALAPPDATA','SHANJIE_TEST_PROFILE','SHANJIE_HEADLESS','SHANJIE_CORE_LOCAL','SHANJIE_TEST_PINYIN','SHANJIE_TEST_ETEN'){$saved[$name]=[Environment]::GetEnvironmentVariable($name)}
$cases=Get-EnglishKeyCases
try{
 $env:SHANJIE_TEST_PROFILE='1';$env:SHANJIE_HEADLESS='1';$env:SHANJIE_CORE_LOCAL=$null
 foreach($arch in $Architectures){
  $out=Join-Path $ResultRoot $arch;New-Item -ItemType Directory $out -Force|Out-Null
  if ($HarnessPath) {
   if ($Architectures.Count -ne 1) { throw 'A prebuilt English harness can only be used for one architecture per invocation.' }
   $exe=[IO.Path]::GetFullPath($HarnessPath)
   if (!(Test-Path -LiteralPath $exe)) { throw "English-key harness is missing: $exe" }
  } else {
   $exe=Join-Path $out 'features.exe'
   $command=@"
@echo off
$(Get-MsvcSetup $arch)
if errorlevel 1 exit /b 1
cl /nologo /std:c++20 /EHsc /O2 /MT /W4 /utf-8 /DUNICODE /D_UNICODE /DNOMINMAX /I"$root/tests/support" "$root/tests/english_keys.cpp" /Fo"$out/features.obj" /Fe"$exe" /link ole32.lib oleaut32.lib uuid.lib user32.lib gdi32.lib
"@
   Set-Content "$out/compile.cmd" $command -Encoding utf8NoBOM
   Invoke-Timed -Name "Compile English-key harness ($arch)" -Category 'build.cpp.harness' -Details @{architecture=$arch;source='tests/english_keys.cpp'} -Action { & cmd.exe /d /c "$out/compile.cmd" *> "$out/compile.log";if($LASTEXITCODE){throw 'Feature harness compilation failed'} }
  }
  foreach($case in $cases){
   if($Scenarios -and $case[1] -notin $Scenarios){continue}
   $folder=Join-Path $out "mode$($case[0])-layout$($case[2])-$($case[1])";if(Test-Path $folder){throw 'Use a fresh result directory'}
   New-Item -ItemType Directory "$folder/profile/Shanjie" -Force|Out-Null
   $text=(Get-Content "$root/profiles/windows.ini" -Raw) -replace '(?m)^learning=.*','learning=0' -replace '(?m)^keyboard=.*',"keyboard=$($case[2])"
   $text=$text -replace '(?m)^mixed_input=.*',"mixed_input=$([int](($case[0]-band 1)-ne 0))" -replace '(?m)^enter_to_cursor=.*',"enter_to_cursor=$($case[3])"
   $text=$text -replace '(?m)^upstream_english=.*',"upstream_english=$([int](($case[0]-band 2)-ne 0))" -replace '(?m)^caps_lock_english=.*','caps_lock_english=0' -replace '(?m)^right_action=.*','right_action=none'
   $text=$text -replace '(?m)^standalone_tones=.*',"standalone_tones=$([int]($case[1] -in 'standalone-tones','standalone-tones-shanjie'))"
   if($case[1] -eq 'standalone-tones-shanjie'){$text=$text -replace '(?m)^interaction=.*','interaction=shanjie'}
   if($case[1] -eq 'backquote-disabled'){$text=$text -replace '(?m)^backquote_symbols=.*','backquote_symbols=0'}
   $env:SHANJIE_TEST_PINYIN=if($case[2] -ge 3){'1'}else{$null}
   $env:SHANJIE_TEST_ETEN=if($case[2] -eq 2){'1'}else{$null}
   if($case[1] -eq 'fullwidth-english'){$text=$text -replace '(?m)^chinese=.*','chinese=0'}
   if($case[1] -eq 'fullwidth-disabled'){$text=$text -replace '(?m)^shift_space_toggle=.*','shift_space_toggle=0'}
   Set-Content "$folder/profile/Shanjie/profile.ini" $text -Encoding utf8NoBOM;$env:LOCALAPPDATA="$folder/profile"
   $start=Start-TestStage -Name "Start English-key core ($arch $($case[1]))" -Category 'core.lifecycle'
   $worker=$null
   try{$worker=Start-Process "$payload/tools/shanjie-core.exe" -ArgumentList '--serve' -WindowStyle Hidden -PassThru;Complete-TestStage -Stage $start -Details @{architecture=$arch;profile="$folder/profile";pid=$worker.Id}}
   catch{Complete-TestStage -Stage $start -Status 'failed' -Details @{architecture=$arch;profile="$folder/profile"} -ErrorMessage $_.Exception.Message;throw}
   try{
    Wait-TestCoreReady -CorePath "$payload/tools/shanjie-core.exe" -OutputDirectory $folder | Out-Null
    Invoke-Timed -Name "English-key regression ($arch $($case[1]))" -Category 'english.keys' -Details @{architecture=$arch;scenario=$case[1];mode=$case[0];keyboard=$case[2];enter_to_cursor=$case[3]} -Action { & $exe "$payload/$arch/Shanjie.dll" $case[1] *> "$folder/result.log";if($LASTEXITCODE){throw "Feature failed: $arch $($case -join ',') ($folder/result.log)"} }
    Write-Output "PASS $arch $($case -join ',')"
   } finally {
    $stopStage=Start-TestStage -Name "Stop English-key core ($arch $($case[1]))" -Category 'core.lifecycle'
    $stop=$null;$stopExited=$false;$stopCode=$null;$stopError=$null
    try{$stop=Start-Process "$payload/tools/shanjie-core.exe" -ArgumentList '--stop' -WindowStyle Hidden -PassThru;$stopExited=$stop.WaitForExit(5000);$stopCode=if($stopExited){$stop.ExitCode}else{$null}}
    catch{$stopError=$_.Exception.Message}
    if($stop -and !$stopExited){try{$stop.Kill();$stop.WaitForExit(5000)|Out-Null}catch{}}
    $workerExited=$worker.WaitForExit(5000)
    if(!$workerExited){try{$worker.Kill();$worker.WaitForExit(5000)|Out-Null}catch{}}
    if($stopError -or !$stopExited -or $stopCode -ne 0 -or !$workerExited){$message='An isolated English-key core did not stop cleanly within five seconds.';Complete-TestStage -Stage $stopStage -Status 'failed' -Details @{architecture=$arch;pid=$worker.Id;stop_exit_code=$stopCode;worker_force_stopped=(!$workerExited)} -ErrorMessage ($stopError ?? $message);throw $message}
    Complete-TestStage -Stage $stopStage -Details @{architecture=$arch;pid=$worker.Id;stop_exit_code=$stopCode}
   }
  }
 }
}finally{foreach($name in $saved.Keys){[Environment]::SetEnvironmentVariable($name,$saved[$name])}}
