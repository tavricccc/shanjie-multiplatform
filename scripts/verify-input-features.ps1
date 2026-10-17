#Requires -Version 7
param([string]$ResultRoot,[ValidateSet('x64','x86')][string[]]$Architectures=@('x64','x86'),[int[]]$CaseIndices,[switch]$ResidentCore)
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
if(!$ResultRoot){$ResultRoot=Join-Path $root ('build/input-features-'+[guid]::NewGuid().ToString('N'))}
$ResultRoot=[IO.Path]::GetFullPath($ResultRoot)
New-Item -ItemType Directory -Path $ResultRoot -Force | Out-Null
$payload=Join-Path $ResultRoot 'payload'
New-Item -ItemType Directory -Path $payload -Force | Out-Null
foreach($arch in $Architectures){
 New-Item -ItemType Directory -Path "$payload/$arch" -Force | Out-Null
 Copy-Item "$root/dist/$arch/Shanjie.dll" "$payload/$arch" -Force
}
Copy-Item "$root/dist/data","$root/dist/tools" $payload -Recurse -Force
# Portable fixture exposes both layouts without changing real TSF registration.
Set-Content "$payload/installed.json" '{"version":"test","keyboards":3}' -Encoding utf8NoBOM
$vs=& "${env:ProgramFiles(x86)}/Microsoft Visual Studio/Installer/vswhere.exe" -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
$cases=@()
foreach($mixed in 0,1) {
 foreach($item in @(@('3','3'),@('4','4'),@('\','\'),@('SU3()','你()'),@('SU3\','你\'),@('SU(3','你('))) {
  $cases += [pscustomobject]@{keys=$item[0];expected=$item[1];mixed=$mixed;symbols=1;keyboard=1;space=0;big5=0}
 }
 foreach($item in @(@('1 ','1 '),@('SU31 ','你1 '),@('JI ','窩'),@('JI3↓1','ㄨㄛˇ'),@('JI3↓1 ','ㄨㄛˇ '),@('SU3CL3⌂↓1','ㄋㄧˇㄏㄠˇ'))) {
  $cases += [pscustomobject]@{keys=$item[0];expected=$item[1];mixed=$mixed;symbols=0;keyboard=1;space=0;big5=0}
 }
}
foreach($item in @(@('IJ3','我'),@('IJ3↓2','ij3'),@('US4↓2','us4'),@('CALLSU3','call你'),@('0D4','看'),@('ABC()\','abc()\'))) {
 $cases += [pscustomobject]@{keys=$item[0];expected=$item[1];mixed=1;symbols=1;keyboard=1;space=0;big5=0}
}
foreach($item in @(@('OX3','我'),@('XO3↓1','ㄨㄛˇ'),@('B ','b '))) {
 $cases += [pscustomobject]@{keys=$item[0];expected=$item[1];mixed=1;symbols=0;keyboard=2;space=0;big5=0}
}
$cases += [pscustomobject]@{keys='1 ';expected='1 ';mixed=1;symbols=0;keyboard=1;space=1;big5=0}
$cases += [pscustomobject]@{keys='JI3↓1';expected='ㄨㄛˇ';mixed=0;symbols=0;keyboard=1;space=0;big5=1}
foreach($upstream in 0,1){foreach($tones in 0,1){
 foreach($digits in @('13','14','16','17','13467','1234567890')) {
  $cases += [pscustomobject]@{keys=$digits;expected=$digits;mixed=1;symbols=1;keyboard=1;space=0;big5=0;upstream=$upstream;standalone=$tones}
 }
 $cases += [pscustomobject]@{keys='13SU3';expected='13你';mixed=1;symbols=1;keyboard=1;space=0;big5=0;upstream=$upstream;standalone=$tones}
}}
$saved=@{};foreach($name in 'LOCALAPPDATA','SHANJIE_HEADLESS','SHANJIE_TEST_PROFILE','SHANJIE_CORE_LOCAL','SHANJIE_TEST_ETEN'){$saved[$name]=[Environment]::GetEnvironmentVariable($name,'Process')}
$results=@()
try {
 $env:SHANJIE_HEADLESS='1';$env:SHANJIE_TEST_PROFILE='1';$env:SHANJIE_CORE_LOCAL=if($ResidentCore){$null}else{'1'}
 foreach($arch in $Architectures) {
  $out=Join-Path $ResultRoot $arch;New-Item -ItemType Directory -Path $out -Force | Out-Null
  $exe=Join-Path $out 'input-features.exe'
  $command=@"
@echo off
call "$vs/VC/Auxiliary/Build/vcvarsall.bat" $arch >nul
if errorlevel 1 exit /b 1
cl /nologo /std:c++20 /EHsc /O2 /MT /utf-8 /DUNICODE /D_UNICODE /DNOMINMAX /I"$root/tests/support" /I"$root/upstream/shanjie/core/include" "$root/tests/submission.cpp" /Fo"$out/features.obj" /Fe"$exe" /link ole32.lib oleaut32.lib uuid.lib user32.lib gdi32.lib
"@
  Set-Content "$out/compile.cmd" $command -Encoding utf8NoBOM
  & cmd.exe /d /c "$out/compile.cmd" *> "$out/compile.log"
  if($LASTEXITCODE){throw "Feature harness build failed: $arch"}
  for($index=0;$index -lt $cases.Count;$index++) {
   if($CaseIndices -and $index -notin $CaseIndices){continue}
   $case=$cases[$index];$fixture=Join-Path $out "case$index"
   $env:SHANJIE_TEST_ETEN=if($case.keyboard -eq 2){'1'}else{$null}
   New-Item -ItemType Directory -Path "$fixture/Shanjie" -Force | Out-Null
   $text=Get-Content "$root/profiles/windows.ini" -Raw
   foreach($pair in @(@('keyboard',$case.keyboard),@('mixed_input',$case.mixed),@('space_inputs_zhuyin',$case.symbols),@('space_commits',$case.space),@('big5',$case.big5),@('learning',0),@('upstream_english',[int]$case.upstream),@('standalone_tones',[int]$case.standalone))) {
    $text=$text -replace ('(?m)^'+$pair[0]+'=.*'),($pair[0]+'='+$pair[1])
   }
   Set-Content "$fixture/Shanjie/profile.ini" $text -Encoding utf8NoBOM
   $env:LOCALAPPDATA=$fixture
   $mode=if($case.space){'input-space-commit'}else{'input-retain'}
   $start=[Diagnostics.ProcessStartInfo]::new($exe);$start.UseShellExecute=$false;$start.CreateNoWindow=$true
   $start.RedirectStandardOutput=$true;$start.RedirectStandardError=$true;$start.StandardOutputEncoding=[Text.UTF8Encoding]::new($false)
   foreach($arg in @("$payload/$arch/Shanjie.dll",$mode,$case.keys,$case.expected)){$start.ArgumentList.Add($arg)}
   $process=[Diagnostics.Process]::Start($start);$stdout=$process.StandardOutput.ReadToEndAsync();$stderr=$process.StandardError.ReadToEndAsync()
   if(!$process.WaitForExit(45000)){$process.Kill();throw "Owned feature harness timed out: $arch $index"}
   $log=$stdout.GetAwaiter().GetResult()+$stderr.GetAwaiter().GetResult();$code=$process.ExitCode;$process.Dispose()
   if($ResidentCore){& "$payload/tools/shanjie-core.exe" --stop *> "$out/core-stop$index.log"}
   Set-Content "$out/case$index.log" $log -Encoding utf8NoBOM
   if($code){throw "Feature check failed: $arch $index ($code), $($case.keys): $log"}
   $results += [pscustomobject]@{architecture=$arch;case=$index;configuration=$case;status='PASS'}
   Write-Output "$arch case $index PASS"
  }
 }
 $results | ConvertTo-Json -Depth 5 | Set-Content "$ResultRoot/results.json" -Encoding utf8NoBOM
 Write-Output "PASS: $($results.Count) isolated TSF feature cases"
} finally {
 foreach($name in $saved.Keys){[Environment]::SetEnvironmentVariable($name,$saved[$name],'Process')}
}
