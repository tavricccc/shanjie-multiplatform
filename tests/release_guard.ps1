#Requires -Version 7
param([Parameter(Mandatory)][string]$ResultRoot)
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
Import-Module "$root/scripts/release-support.psm1" -ErrorAction Stop
Import-Module "$root/scripts/test-support.psm1" -ErrorAction Stop
Import-Module "$root/scripts/test-plan.psm1" -ErrorAction Stop
$fixture=Join-Path $ResultRoot 'source'
$null=New-Item -ItemType Directory -Path "$fixture/windows","$fixture/ui" -Force
[IO.File]::WriteAllText("$fixture/.gitignore","/build/`n/dist/`n")
[IO.File]::WriteAllText("$fixture/RELEASE-CHANNEL.json",'{"version":"0.0.1"}')
[IO.File]::WriteAllText("$fixture/windows/Cargo.toml","[package]`nname = `"shanjie-windows`"`nversion = `"0.0.1`"`n")
[IO.File]::WriteAllText("$fixture/windows/Cargo.lock","[[package]]`nname = `"shanjie-windows`"`nversion = `"0.0.1`"`n")
[IO.File]::WriteAllText("$fixture/ui/Shanjie.UI.csproj",'<Project><PropertyGroup><Version>0.0.1</Version></PropertyGroup></Project>')
[IO.File]::WriteAllText("$fixture/source.txt",'accepted source')
& git -C $fixture init -b test/release-guard *> $null
& git -C $fixture config core.autocrlf false
& git -C $fixture add -- .gitignore RELEASE-CHANNEL.json windows/Cargo.toml windows/Cargo.lock ui/Shanjie.UI.csproj source.txt
& git -C $fixture -c user.name=ReleaseGuardTest -c user.email=release-guard@example.invalid -c commit.gpgsign=false -c core.hooksPath=NUL commit -m 'test: seed isolated release guard fixture' *> $null
if($LASTEXITCODE){throw 'Could not initialize the isolated guard fixture.'}
$identity=Get-ReleaseIdentity $fixture
$candidateId=[guid]::NewGuid().ToString('N')
$candidateRoot=Join-Path $fixture "build/release-candidates/$candidateId"
foreach($path in 'dist/x64/Shanjie.dll','dist/x86/Shanjie.dll','dist/tools/core.exe','dist/ui/Shanjie.UI.exe','dist/data/model','dist/LICENSES/license','dist/setup/stub',"build/release-candidates/$candidateId/Shanjie-Windows-Setup.exe"){
    $file=Join-Path $fixture $path
    $null=New-Item -ItemType Directory -Path (Split-Path $file -Parent) -Force
    [IO.File]::WriteAllText($file,"synthetic guard fixture: $path")
}
$installer=Join-Path $candidateRoot 'Shanjie-Windows-Setup.exe'
$hash=(Get-FileHash -LiteralPath $installer).Hash.ToLowerInvariant()
@{id=$candidateId;source=$identity;integrity_verified=$true;installer_sha256=$hash}|ConvertTo-Json -Depth 5|Set-Content "$candidateRoot/candidate.json" -Encoding utf8NoBOM
$receiptRoot=Join-Path $fixture 'build/reports/release-fixture'
$null=New-Item -ItemType Directory -Path $receiptRoot -Force
$ctest=(Get-Command ctest -CommandType Application).Source
$cmake=(Get-Command cmake -CommandType Application).Source.Replace('\','/')
$reports=@();$selected=@()
foreach($arch in 'x64','x86'){
    $preset="windows-$arch"
    $binary=Join-Path $fixture "build/cmake/$preset"
    $null=New-Item -ItemType Directory -Path $binary -Force
    $name=if($arch -eq 'x64'){'full.english.caps.x64'}else{'release.fixture.x86'}
    [IO.File]::WriteAllText((Join-Path $binary 'CTestTestfile.cmake'),"add_test($name `"$cmake`" `"-E`" `"true`")`nset_tests_properties($name PROPERTIES LABELS `"release`")`n")
    $junit=Join-Path $receiptRoot "$preset.xml"
    $log=Join-Path $receiptRoot "$preset.log"
    & $ctest --test-dir $binary -C Release --output-junit $junit --output-log $log *> $null
    if($LASTEXITCODE){throw 'Synthetic CTest fixture failed.'}
    $reports+=[pscustomobject]@{preset=$preset;attempt=1;junit=$junit;log=$log}
    $selected+=[pscustomobject]@{preset=$preset;name=$name}
}
$caseRows=@($selected|ForEach-Object{[pscustomobject]@{name=$_.name;preset=$_.preset;selected=$true;reason='synthetic guard fixture'}})
$combinations=@(Get-HarnessCasePlan $caseRows)
foreach($combination in $combinations){$combination.status='passed';$combination.attempts=@(@{attempt=1;status='passed'})}
$record=[ordered]@{schema=3;level='RELEASE';status='passed';commit=$identity.commit;version=$identity.version;worktree_dirty=$false;ci_safe=$false;ctest=$reports;results=@(Read-CtestResults $reports);selection=@{selected=$selected};subcases=$combinations;release=@{source_start=$identity;source_end=$identity;architectures=@('x64','x86');candidate_id=$candidateId;artifacts=(Get-ReleaseArtifacts $fixture $candidateId $reports)}}
$reportPath=Join-Path $receiptRoot 'report.json'
$original=$record|ConvertTo-Json -Depth 15
[IO.File]::WriteAllText($reportPath,$original)
$proof=Assert-ReleaseAcceptance $fixture $reportPath
if(!$proof.local_release_verified -or $proof.sha256 -cne $hash){throw 'A complete matching receipt should be accepted.'}
$junitTime=(Get-Item -LiteralPath $reports[0].junit).LastWriteTimeUtc
$null=Assert-ReleaseAcceptance $fixture $reportPath
if((Get-Item -LiteralPath $reports[0].junit).LastWriteTimeUtc -ne $junitTime){throw 'Receipt reuse must not rerun tests.'}

function Assert-Rejected([string]$Name,[scriptblock]$Change) {
    $changed=$original|ConvertFrom-Json -AsHashtable
    & $Change $changed
    [IO.File]::WriteAllText($reportPath,($changed|ConvertTo-Json -Depth 15))
    $rejected=$false
    try{$null=Assert-ReleaseAcceptance $fixture $reportPath}catch{$rejected=$true}
    finally{[IO.File]::WriteAllText($reportPath,$original)}
    if(!$rejected){throw "Guard accepted invalid receipt: $Name"}
}
Assert-Rejected 'wrong level' {param($r) $r.level='FULL'}
Assert-Rejected 'unfinished' {param($r) $r.status='running'}
Assert-Rejected 'CI-safe subset' {param($r) $r.ci_safe=$true}
Assert-Rejected 'stale commit' {param($r) $r.commit='0000000000000000000000000000000000000000'}
Assert-Rejected 'stale version' {param($r) $r.version='0.0.0'}
Assert-Rejected 'missing architecture' {param($r) $r.release.architectures=@('x64')}
Assert-Rejected 'missing result' {param($r) $r.results=@($r.results[0])}
Assert-Rejected 'skipped result' {param($r) $r.results[0].status='not-run'}
Assert-Rejected 'status without evidence' {param($r) $r.ctest=@();$r.results=@()}
Assert-Rejected 'missing combination' {param($r) $r.subcases=@($r.subcases[0])}
Assert-Rejected 'different combination parameters' {param($r) $r.subcases[0].parameters.keyboard=2}
Assert-Rejected 'combination without execution' {param($r) $r.subcases[0].attempts=@()}
$junitOriginal=[IO.File]::ReadAllText($reports[0].junit)
try {
    [xml]$xml=$junitOriginal
    $skip=$xml.CreateElement('skipped');$skip.SetAttribute('message','not executed');$null=$xml.SelectSingleNode('//testcase').AppendChild($skip)
    $xml.Save($reports[0].junit)
    Assert-Rejected 'skipped JUnit despite passed JSON' {param($r)}
}finally{[IO.File]::WriteAllText($reports[0].junit,$junitOriginal)}
[IO.File]::WriteAllText("$fixture/source.txt",'changed source')
try{Assert-Rejected 'dirty source' {param($r)}}finally{[IO.File]::WriteAllText("$fixture/source.txt",'accepted source')}
foreach($file in @($installer,"$fixture/dist/x86/Shanjie.dll")){
    $bytes=[IO.File]::ReadAllBytes($file)
    try{[IO.File]::AppendAllText($file,'changed');Assert-Rejected 'changed artifact' {param($r)}}
    finally{[IO.File]::WriteAllBytes($file,$bytes)}
}
$null=Assert-ReleaseAcceptance $fixture $reportPath
'PASS local release guard: real synthetic CTest evidence, reuse, stale/dirty/skipped/incomplete receipts, architectures and artifact changes'
