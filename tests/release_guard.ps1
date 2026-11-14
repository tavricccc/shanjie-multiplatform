#Requires -Version 7
param([Parameter(Mandatory)][string]$ResultRoot)
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
Import-Module "$root/scripts/release-support.psm1" -ErrorAction Stop
Import-Module "$root/scripts/test-support.psm1" -ErrorAction Stop
Import-Module "$root/scripts/test-plan.psm1" -ErrorAction Stop
$fixture=Join-Path ([IO.Path]::GetFullPath($ResultRoot)) 'source'
$null=New-Item -ItemType Directory -Path "$fixture/windows","$fixture/ui" -Force
[IO.File]::WriteAllText("$fixture/.gitignore","/build/`n/dist/`n")
[IO.File]::WriteAllText("$fixture/RELEASE-CHANNEL.json",'{"version":"0.0.1"}')
[IO.File]::WriteAllText("$fixture/windows/Cargo.toml","[package]`nname = `"shanjie-windows`"`nversion = `"0.0.1`"`n")
[IO.File]::WriteAllText("$fixture/windows/Cargo.lock","[[package]]`nname = `"shanjie-windows`"`nversion = `"0.0.1`"`n")
[IO.File]::WriteAllText("$fixture/ui/Shanjie.UI.csproj",'<Project><PropertyGroup><Version>0.0.1</Version></PropertyGroup></Project>')
[IO.File]::WriteAllText("$fixture/source.txt",'accepted source')
foreach($path in 'UPSTREAM.json','README.md','Changelog.md','profiles/test.ini','UPSTREAM-CHANGES.md','docs/test.md','MODEL-SOURCES.json','MODEL-CATALOG.json','安裝說明.txt'){
    $file=Join-Path $fixture $path
    $null=New-Item -ItemType Directory -Path (Split-Path $file -Parent) -Force
    [IO.File]::WriteAllText($file,"synthetic source: $path")
}
foreach($name in 'install.ps1','uninstall.ps1','install.cmd','uninstall.cmd','profiles.ps1','apply-profile.ps1','apply-windows.cmd','apply-shanjie.cmd','settings.cmd','重新選擇鍵盤.cmd'){
    $null=New-Item -ItemType Directory -Path "$fixture/scripts" -Force
    [IO.File]::WriteAllText("$fixture/scripts/$name","synthetic script: $name")
}
& git -C $fixture init -b test/release-guard *> $null
& git -C $fixture config core.autocrlf false
& git -C $fixture add -- .
& git -C $fixture -c user.name=ReleaseGuardTest -c user.email=release-guard@example.invalid -c commit.gpgsign=false -c core.hooksPath=NUL commit -m 'test: seed isolated release guard fixture' *> $null
if($LASTEXITCODE){throw 'Could not initialize the isolated guard fixture.'}
$identity=Get-ReleaseIdentity $fixture
$candidateId=[guid]::NewGuid().ToString('N')
$candidateRoot=Join-Path $fixture "build/release-candidates/$candidateId"
foreach($path in 'dist/x64/Shanjie.dll','dist/x86/Shanjie.dll','dist/tools/shanjie-core.exe','dist/tools/shanjie-word-helper.exe','dist/ui/Shanjie.UI.exe','dist/data/model','dist/LICENSES/license','dist/setup/安裝善解.exe','dist/setup/shanjie-pack.exe',
    'windows/target/x86_64-pc-windows-msvc/release/shanjie_windows.lib','windows/target/i686-pc-windows-msvc/release/shanjie_windows.lib'){
    $file=Join-Path $fixture $path
    $null=New-Item -ItemType Directory -Path (Split-Path $file -Parent) -Force
    [IO.File]::WriteAllText($file,"synthetic guard fixture: $path")
}
# Cargo output is an ignored build artifact in the real project too.
Add-Content "$fixture/.gitignore" '/windows/target/'
& git -C $fixture add .gitignore
& git -C $fixture -c user.name=ReleaseGuardTest -c user.email=release-guard@example.invalid -c commit.gpgsign=false -c core.hooksPath=NUL commit -m 'test: ignore native fixture outputs' *> $null
$identity=Get-ReleaseIdentity $fixture
$rejected=$false
[IO.File]::WriteAllText("$fixture/source.txt",'dirty source cannot be sealed')
try{$null=New-ReleaseSnapshot $fixture ([guid]::NewGuid().ToString('N')) $identity}catch{$rejected=$true}
finally{[IO.File]::WriteAllText("$fixture/source.txt",'accepted source')}
if(!$rejected){throw 'A dirty worktree must not produce a commit-bound snapshot.'}
$snapshot=New-ReleaseSnapshot $fixture $candidateId $identity
if($snapshot.source.commit -cne $identity.commit -or $snapshot.source.version -cne $identity.version){throw 'Snapshot must bind both product architectures to the clean commit/version.'}
$installer=Join-Path $candidateRoot 'Shanjie-Windows-Setup.exe'
$receiptRoot=Join-Path $fixture 'build/reports/release-fixture'
$null=New-Item -ItemType Directory -Path $receiptRoot -Force
$ctest=(Get-Command ctest -CommandType Application | Select-Object -First 1).Source
$cmake=(Get-Command cmake -CommandType Application | Select-Object -First 1).Source.Replace('\','/')
$reports=@();$selected=@()
foreach($arch in 'x64','x86'){
    $preset="windows-$arch"
    $binary=Join-Path $fixture "build/cmake/$preset"
    $null=New-Item -ItemType Directory -Path $binary -Force
    $name=if($arch -eq 'x64'){'full.english.caps.x64'}else{'release.fixture.x86'}
    [IO.File]::WriteAllText((Join-Path $binary 'CTestTestfile.cmake'),"add_test($name `"$cmake`" `"-E`" `"true`")`nset_tests_properties($name PROPERTIES LABELS `"release`")`n")
    if($arch -eq 'x64'){
        Add-Content (Join-Path $binary 'CTestTestfile.cmake') "add_test(release.package `"$cmake`" `"-E`" `"true`")`nset_tests_properties(release.package PROPERTIES LABELS `"release;package`")"
    }
    $junit=Join-Path $receiptRoot "$preset.xml"
    $log=Join-Path $receiptRoot "$preset.log"
    & $ctest --test-dir $binary -C Release --output-junit $junit --output-log $log -R "^$([regex]::Escape($name))`$" *> $null
    if($LASTEXITCODE){throw 'Synthetic CTest fixture failed.'}
    $reports+=[pscustomobject]@{preset=$preset;attempt=1;junit=$junit;log=$log;snapshot_id=$snapshot.id;snapshot_sha256=$snapshot.sha256}
    $selected+=[pscustomobject]@{preset=$preset;name=$name}
}
$caseRows=@($selected|ForEach-Object{[pscustomobject]@{name=$_.name;preset=$_.preset;selected=$true;reason='synthetic guard fixture'}})
$combinations=@(Get-HarnessCasePlan $caseRows)
foreach($combination in $combinations){$combination.status='passed';$combination.attempts=@(@{attempt=1;status='passed'})}
$permitPath=Join-Path $receiptRoot 'packaging-permit.json'
$prepack=[ordered]@{schema=4;level='RELEASE';status='tests-passed';ci_safe=$false;ctest=$reports;results=@(Read-CtestResults $reports);selection=@{selected=$selected};subcases=$combinations;release=@{architectures=@('x64','x86');candidate_id=$candidateId;snapshot_sha256=$snapshot.sha256}}
function Reject-Permit([string]$Name,$Record){
    $rejected=$false
    try{$null=New-ReleasePackagingPermit $fixture $permitPath $Record}catch{$rejected=$true}
    if(!$rejected -or (Test-Path -LiteralPath $permitPath) -or (Test-Path -LiteralPath $installer)){throw "Invalid tests authorized packaging: $Name"}
}
$incomplete=($prepack|ConvertTo-Json -Depth 18)|ConvertFrom-Json -AsHashtable
$incomplete.ctest=@($incomplete.ctest[0]);$incomplete.results=@($incomplete.results|Where-Object preset -eq 'windows-x64');$incomplete.selection.selected=@($incomplete.selection.selected|Where-Object preset -eq 'windows-x64')
Reject-Permit 'x64 alone, x86 has not run' $incomplete
$failed=($prepack|ConvertTo-Json -Depth 18)|ConvertFrom-Json -AsHashtable
$failed.results[1].status='failed'
Reject-Permit 'x86 failed' $failed
$missingSubcase=($prepack|ConvertTo-Json -Depth 18)|ConvertFrom-Json -AsHashtable
$missingSubcase.subcases=@()
Reject-Permit 'passing parent without complete assertions' $missingSubcase
$permit=New-ReleasePackagingPermit $fixture $permitPath $prepack
# A later ordinary build changes dist. The candidate must still copy the tested
# snapshot, not pick up the new DLL from this shared worktree output.
[IO.File]::AppendAllText("$fixture/dist/x86/Shanjie.dll",'unrelated later build')
Copy-ReleaseFiles $snapshot.runtime_root "$candidateRoot/package"
Assert-ReleasePackageCopy $snapshot "$candidateRoot/package"
if((Get-FileHash "$candidateRoot/package/x86/Shanjie.dll").Hash -ceq (Get-FileHash "$fixture/dist/x86/Shanjie.dll").Hash){throw 'Candidate used live dist instead of the tested snapshot.'}
$null=New-Item -ItemType Directory -Path $candidateRoot -Force
[IO.File]::WriteAllText($installer,'synthetic installer; no Windows executable was run')
$hash=(Get-FileHash -LiteralPath $installer).Hash.ToLowerInvariant()
@{schema=2;id=$candidateId;source=$identity;integrity_verified=$true;snapshot_id=$candidateId;snapshot_sha256=$snapshot.sha256;packaging_permit_sha256=$permit.sha256;installer_sha256=$hash}|ConvertTo-Json -Depth 5|Set-Content "$candidateRoot/candidate.json" -Encoding utf8NoBOM
$packageJunit=Join-Path $receiptRoot 'windows-x64-package.xml'
$packageLog=Join-Path $receiptRoot 'windows-x64-package.log'
& $ctest --test-dir "$fixture/build/cmake/windows-x64" -C Release -R '^release\.package$' --output-junit $packageJunit --output-log $packageLog *> $null
if($LASTEXITCODE){throw 'Synthetic deferred package CTest failed.'}
$reports+=[pscustomobject]@{preset='windows-x64';attempt=1;junit=$packageJunit;log=$packageLog;snapshot_id=$snapshot.id;snapshot_sha256=$snapshot.sha256}
$selected+=[pscustomobject]@{preset='windows-x64';name='release.package'}
$record=[ordered]@{schema=4;level='RELEASE';status='passed';commit=$identity.commit;version=$identity.version;worktree_dirty=$false;ci_safe=$false;ctest=$reports;results=@(Read-CtestResults $reports);selection=@{selected=$selected};subcases=$combinations;release=@{source_start=$identity;source_end=$identity;architectures=@('x64','x86');candidate_id=$candidateId;snapshot_sha256=$snapshot.sha256;packaging_permit=$permitPath;artifacts=(Get-ReleaseArtifacts $fixture $candidateId $reports $permitPath)}}
$reportPath=Join-Path $receiptRoot 'report.json'
$original=$record|ConvertTo-Json -Depth 15
[IO.File]::WriteAllText($reportPath,$original)
$proof=Assert-ReleaseAcceptance $fixture $reportPath
if(!$proof.local_release_verified -or $proof.sha256 -cne $hash){throw 'A complete matching receipt should be accepted.'}
$junitTime=(Get-Item -LiteralPath $reports[0].junit).LastWriteTimeUtc
$null=Complete-ReleaseArtifact $fixture $reportPath
$marker=Get-Content "$fixture/dist/release.json" -Raw|ConvertFrom-Json
if(!$marker.local_release_verified -or $marker.installer_sha256 -cne $hash -or (Get-FileHash "$fixture/dist/Shanjie-Windows-Setup.exe").Hash.ToLowerInvariant() -cne $hash){throw 'Promotion must publish the exact accepted candidate and marker.'}
$null=Complete-ReleaseArtifact $fixture
if((Get-Item -LiteralPath $reports[0].junit).LastWriteTimeUtc -ne $junitTime){throw 'Receipt reuse must not rerun tests.'}

function Assert-Rejected([string]$Name,[scriptblock]$Change) {
    $changed=$original|ConvertFrom-Json -AsHashtable
    & $Change $changed
    [IO.File]::WriteAllText($reportPath,($changed|ConvertTo-Json -Depth 15))
    $rejected=$false
    try{$null=Complete-ReleaseArtifact $fixture $reportPath}catch{$rejected=$true}
    finally{[IO.File]::WriteAllText($reportPath,$original)}
    if(!$rejected){throw "Guard accepted invalid receipt: $Name"}
}
Assert-Rejected 'old receipt without a snapshot' {param($r) $r.schema=3;$r.release.Remove('snapshot_sha256')}
Assert-Rejected 'wrong snapshot identity' {param($r) $r.release.snapshot_sha256='0'*64}
Assert-Rejected 'missing dual-architecture packaging permit' {param($r) $r.release.packaging_permit=$null}
Assert-Rejected 'wrong level' {param($r) $r.level='FULL'}
Assert-Rejected 'unfinished' {param($r) $r.status='running'}
Assert-Rejected 'CI-safe subset' {param($r) $r.ci_safe=$true}
Assert-Rejected 'stale commit' {param($r) $r.commit='0000000000000000000000000000000000000000'}
Assert-Rejected 'stale version' {param($r) $r.version='0.0.0'}
Assert-Rejected 'missing architecture' {param($r) $r.release.architectures=@('x64')}
Assert-Rejected 'missing result' {param($r) $r.results=@($r.results[0])}
Assert-Rejected 'JUnit recorded against another snapshot' {param($r) $r.ctest[1].snapshot_id='0'*32}
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
foreach($file in @($installer,"$candidateRoot/package/x86/Shanjie.dll","$($snapshot.runtime_root)/x64/Shanjie.dll","$($snapshot.runtime_root)/x86/Shanjie.dll","$($snapshot.runtime_root)/tools/shanjie-core.exe","$($snapshot.runtime_root)/data/model","$($snapshot.root)/native/x86/shanjie_windows.lib","$($snapshot.root)/snapshot.json")){
    $readOnly=(Get-Item -LiteralPath $file).IsReadOnly
    (Get-Item -LiteralPath $file).IsReadOnly=$false
    $bytes=[IO.File]::ReadAllBytes($file)
    try{[IO.File]::AppendAllText($file,'changed');Assert-Rejected 'changed artifact' {param($r)}}
    finally{[IO.File]::WriteAllBytes($file,$bytes);(Get-Item -LiteralPath $file).IsReadOnly=$readOnly}
}
$null=Assert-ReleaseAcceptance $fixture $reportPath
'PASS release guard: synthetic CTest evidence, dual-architecture gate, frozen test/package inputs, hash changes, receipt reuse and stale/dirty/skipped/incomplete rejection; no Windows product acceptance executed'
