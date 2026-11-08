#Requires -Version 7
param([string]$ResultRoot,[ValidateSet('x64','x86')][string[]]$Architectures=@('x64','x86'),[string[]]$Scenarios)
$ErrorActionPreference='Stop';$root=Split-Path $PSScriptRoot -Parent
. "$PSScriptRoot/msvc-command.ps1"
if(!$ResultRoot){$ResultRoot=Join-Path $root ('build/english-keys-'+[guid]::NewGuid().ToString('N'))}
$ResultRoot=[IO.Path]::GetFullPath($ResultRoot);New-Item -ItemType Directory $ResultRoot -Force|Out-Null
$payload=Join-Path $ResultRoot 'payload';New-Item -ItemType Directory $payload -Force|Out-Null
foreach($arch in $Architectures){New-Item -ItemType Directory "$payload/$arch" -Force|Out-Null;Copy-Item "$root/dist/$arch/Shanjie.dll" "$payload/$arch" -Force}
foreach($name in 'data','tools'){New-Item -ItemType Junction -Path "$payload/$name" -Target "$root/dist/$name"|Out-Null}
Set-Content "$payload/installed.json" '{"version":"test","keyboards":3}' -Encoding utf8NoBOM
$saved=@{};foreach($name in 'LOCALAPPDATA','SHANJIE_TEST_PROFILE','SHANJIE_HEADLESS','SHANJIE_CORE_LOCAL','SHANJIE_TEST_PINYIN','SHANJIE_TEST_ETEN'){$saved[$name]=[Environment]::GetEnvironmentVariable($name)}
# Mode bits: 1 raw-key mixing, 2 upstream English; the mixing mode is one choice, so never both.
$cases=@();foreach($mode in 1,2){foreach($scenario in 'caps','caps-zh'){$cases+=,@($mode,$scenario,1,1)}}
foreach($scenario in 'shift','shift-caps'){$cases+=,@(2,$scenario,1,1)}
$cases+=,@(1,'eten',2,1);$cases+=,@(0,'partial',1,1);$cases+=,@(0,'whole',1,0);$cases+=,@(0,'candidate',1,1)
foreach($keyboard in 1,3,4){$cases+=,@(0,'backquote',$keyboard,1)}
$cases+=,@(0,'backquote-disabled',1,1)
foreach($mode in 0,1,2){$cases+=,@($mode,'numpad',1,1)}
$cases+=,@(0,'numpad',2,1)
foreach($mode in 0,1,2){$cases+=,@($mode,'standalone-tones',1,1)}
$cases+=,@(2,'standalone-tones-off',1,1);$cases+=,@(2,'standalone-tones-shanjie',1,1)
$cases+=,@(0,'fullwidth',1,1);$cases+=,@(0,'fullwidth-english',1,1);$cases+=,@(0,'fullwidth-disabled',1,1)
try{
 $env:SHANJIE_TEST_PROFILE='1';$env:SHANJIE_HEADLESS='1';$env:SHANJIE_CORE_LOCAL=$null
 foreach($arch in $Architectures){
  $out=Join-Path $ResultRoot $arch;New-Item -ItemType Directory $out -Force|Out-Null
  $command=@"
@echo off
$(Get-MsvcSetup $arch)
if errorlevel 1 exit /b 1
cl /nologo /std:c++20 /EHsc /O2 /MT /W4 /utf-8 /DUNICODE /D_UNICODE /DNOMINMAX /I"$root/tests/support" "$root/tests/english_keys.cpp" /Fo"$out/features.obj" /Fe"$out/features.exe" /link ole32.lib oleaut32.lib uuid.lib user32.lib gdi32.lib
"@
  Set-Content "$out/compile.cmd" $command -Encoding utf8NoBOM;& cmd.exe /d /c "$out/compile.cmd" *> "$out/compile.log";if($LASTEXITCODE){throw 'Feature harness compilation failed'}
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
   $worker=Start-Process "$payload/tools/shanjie-core.exe" -ArgumentList '--serve' -WindowStyle Hidden -PassThru
   try{& "$out/features.exe" "$payload/$arch/Shanjie.dll" $case[1] *> "$folder/result.log";if($LASTEXITCODE){throw "Feature failed: $arch $($case -join ',') ($folder/result.log)"};Write-Output "PASS $arch $($case -join ',')"}
   finally{$stop=Start-Process "$payload/tools/shanjie-core.exe" -ArgumentList '--stop' -WindowStyle Hidden -PassThru;$stop.WaitForExit(5000)|Out-Null;$worker.WaitForExit(5000)|Out-Null}
  }
 }
}finally{foreach($name in $saved.Keys){[Environment]::SetEnvironmentVariable($name,$saved[$name])}}
