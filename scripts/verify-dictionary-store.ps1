#Requires -Version 7
param([string]$ResultRoot)
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
if(!$ResultRoot){$ResultRoot=Join-Path $root ('build/dictionary-store-'+[guid]::NewGuid().ToString('N'))}
$ResultRoot=[IO.Path]::GetFullPath($ResultRoot)
New-Item -ItemType Directory -Path $ResultRoot -Force | Out-Null
$vs=& "${env:ProgramFiles(x86)}/Microsoft Visual Studio/Installer/vswhere.exe" -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
$executables=@{}
foreach($arch in 'x64','x86') {
 $out=Join-Path $ResultRoot $arch;New-Item -ItemType Directory -Path $out -Force | Out-Null
 $exe=Join-Path $out 'dictionary-store.exe';$executables[$arch]=$exe
 $command=@"
@echo off
call "$vs/VC/Auxiliary/Build/vcvarsall.bat" $arch >nul
if errorlevel 1 exit /b 1
cd /d "$out"
cl /nologo /std:c++20 /EHsc /O2 /MT /utf-8 /DUNICODE /D_UNICODE "$root/tests/dictionary_store.cpp" /link /OUT:"$exe"
"@
 Set-Content "$out/compile.cmd" $command -Encoding utf8NoBOM
 & cmd.exe /d /c "$out/compile.cmd" | Set-Content "$out/compile.log"
 if($LASTEXITCODE){throw 'Dictionary ABI probe compilation failed'}
}
function Start-Probe([string]$Arch,[string]$Folder,[int]$Iterations) {
 $start=[Diagnostics.ProcessStartInfo]::new($executables[$Arch]);$start.UseShellExecute=$false;$start.CreateNoWindow=$true;$start.RedirectStandardOutput=$true;$start.RedirectStandardError=$true
 foreach($arg in @("$root/dist/$Arch/Shanjie.dll",$Folder,"$Iterations")){$start.ArgumentList.Add($arg)}
 $process=[Diagnostics.Process]::Start($start)
 [pscustomobject]@{process=$process;output=$process.StandardOutput.ReadToEndAsync();error=$process.StandardError.ReadToEndAsync()}
}
function Finish-Probe($Probe) {
 if(!$Probe.process.WaitForExit(15000)){$Probe.process.Kill();throw 'Owned dictionary probe timed out'}
 $output=$Probe.output.GetAwaiter().GetResult()+$Probe.error.GetAwaiter().GetResult();$code=$Probe.process.ExitCode;$Probe.process.Dispose()
 if($code -or $output -notmatch 'PASS '){throw "Dictionary ABI check failed ($code): $output"};$output
}
$folder=Join-Path $ResultRoot 'shared-dictionary';New-Item -ItemType Directory -Path $folder -Force | Out-Null
$first=Finish-Probe (Start-Probe x64 $folder 1)
$name=($first.Trim() -split ' ')[-1];$file=Join-Path $folder $name
if($name -ne ('dictionary-'+(Get-FileHash $file -Algorithm SHA256).Hash.ToLowerInvariant()+'.tsv')){throw 'Dictionary content hash mismatch'}
[IO.File]::WriteAllText($file,'partial')
$left=Start-Probe x64 $folder 100;$right=Start-Probe x86 $folder 100
$logs=@($first,(Finish-Probe $left),(Finish-Probe $right))
if($name -ne ('dictionary-'+(Get-FileHash $file -Algorithm SHA256).Hash.ToLowerInvariant()+'.tsv')){throw 'Repaired dictionary content hash mismatch'}
$corrupt=@(Get-ChildItem "$folder/.deleted_files" -File)
if($corrupt.Count -ne 1 -or [IO.File]::ReadAllText($corrupt[0].FullName) -ne 'partial'){throw 'Damaged dictionary was not preserved'}
$logs | Set-Content "$ResultRoot/results.txt" -Encoding utf8NoBOM
"Passed first publication, truncated-file repair, SHA-256 and 200 simultaneous x64/x86 ABI saves: $ResultRoot"
