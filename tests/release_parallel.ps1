#Requires -Version 7
param([Parameter(Mandatory)][string]$FixtureRoot,[Parameter(Mandatory)]$Prepack)
$ErrorActionPreference='Stop'
$project=Split-Path $PSScriptRoot -Parent
Import-Module "$project/scripts/release-support.psm1"
Import-Module "$project/scripts/core-shards.psm1"
Import-Module "$project/scripts/test-support.psm1"
$names=@(Get-CoreTestNames $project)
$plans=@(Get-CoreShardPlan $names)
Assert-CoreShardPlan $names $plans
$badPlans=($plans | ConvertTo-Json -Depth 8) | ConvertFrom-Json -AsHashtable
$badPlans[1].tests+=@('engine_cap')
$rejected=$false
try{Assert-CoreShardPlan $names $badPlans}catch{$rejected=$true}
if(!$rejected){throw 'Duplicate core shard assignment was accepted.'}
$badPlans=($plans | ConvertTo-Json -Depth 8) | ConvertFrom-Json -AsHashtable
$badPlans[1].tests=@($badPlans[1].tests | Select-Object -Skip 1)
$rejected=$false
try{Assert-CoreShardPlan $names $badPlans}catch{$rejected=$true}
if(!$rejected){throw 'Omitted core shard assignment was accepted.'}

# Extend the existing isolated synthetic release-guard repository. These .cmd
# stand-ins exercise process execution and receipts, never the Windows product.
$paths=@('upstream/shanjie/core/src/lib.rs','upstream/shanjie/core/Cargo.toml','.cargo/config.toml')+
    @($names | Where-Object {$_ -ne 'core'} | ForEach-Object {"upstream/shanjie/core/tests/$_.rs"})
foreach($path in $paths){
    $file=Join-Path $FixtureRoot $path
    $null=New-Item -ItemType Directory -Path (Split-Path $file -Parent) -Force
    [IO.File]::WriteAllText($file,'synthetic source')
}
& git -C $FixtureRoot add -- .cargo upstream
& git -C $FixtureRoot -c user.name=ReleaseGuardTest -c user.email=release-guard@example.invalid -c commit.gpgsign=false -c core.hooksPath=NUL commit --allow-empty -m 'test: seed parallel core fixture' *> $null
if($LASTEXITCODE){throw 'Cannot commit parallel release fixture.'}
$identity=Get-ReleaseIdentity $FixtureRoot
$bundle=Join-Path $FixtureRoot 'build/parallel-core-fixture'
$tree=Join-Path $FixtureRoot 'build/test/core'
$resources=Join-Path $FixtureRoot 'build/parallel-fixture-resources'
$models=Join-Path $FixtureRoot 'build/parallel-fixture-models'
# Match Cargo's target-qualified layout rather than the old hard-coded directory.
$compiled=Join-Path $FixtureRoot 'build/core-executables/x86_64-pc-windows-msvc/release/deps'
$null=New-Item -ItemType Directory -Path "$tree/src","$tree/tests","$resources/data","$resources/eval","$resources/docs",$models,$compiled -Force
foreach($file in 'Cargo.toml','Cargo.lock','src/lib.rs','tests/fixture.rs'){[IO.File]::WriteAllText("$tree/$file",'synthetic generated core input')}
[IO.File]::WriteAllText("$resources/data/table",'synthetic dictionary')
[IO.File]::WriteAllText("$resources/eval/table",'synthetic evaluation data')
[IO.File]::WriteAllText("$resources/docs/spec",'synthetic specification')
[IO.File]::WriteAllText("$models/model",'synthetic model')
[IO.File]::WriteAllText("$compiled/core-test-fixture.dll",'synthetic runtime dependency')
$binaries=@(foreach($name in $names){
    $exe=Join-Path $compiled "$name.cmd"
    $output=if($name -eq 'core'){
        "echo running 3 tests`r`necho test sample ... ok`r`necho test expected_panic - should panic ... ok`r`necho test optional_case ... ignored, synthetic optional test`r`necho test result: ok. 2 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out`r`n"
    }else{"echo running 1 test`r`necho test sample ... ok`r`necho test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`r`n"}
    $listing="echo sample: test`r`n"+$(if($name -eq 'core'){"echo expected_panic: test`r`necho optional_case: test`r`n"})
    [IO.File]::WriteAllText($exe,"@echo off`r`nif `"%1`"==`"--list`" goto list`r`nif `"%SHANJIE_SYNTHETIC_CORE_FAIL%`"==`"$name`" exit /b 1`r`n${output}exit /b 0`r`n:list`r`n${listing}exit /b 0`r`n")
    [pscustomobject]@{Name=$name;Path=$exe}
})
New-CoreTestBundle $FixtureRoot $bundle $binaries $resources $models ('a'*64) @()
$prepared=Assert-CoreTestBundle $FixtureRoot $bundle
if($prepared.binaries.Count -ne 37 -or !$prepared.files.ContainsKey('test/core/target/release/deps/core-test-fixture.dll') -or
    (Get-FileHash "$bundle/test/core/target/release/deps/core-test-fixture.dll").Hash -cne (Get-FileHash "$compiled/core-test-fixture.dll").Hash){throw 'Prepared bundle lost Cargo-discovered executables or runtime dependencies.'}
foreach($triple in 'x86_64-pc-windows-msvc','i686-pc-windows-msvc'){
    [IO.File]::WriteAllText("$FixtureRoot/windows/target/$triple/release/shanjie-reliability-check.exe",'synthetic probe')
}
$tokens=$null;$parseErrors=$null
$driver=[Management.Automation.Language.Parser]::ParseFile("$project/scripts/release-pipeline.ps1",[ref]$tokens,[ref]$parseErrors)
$packCommand=$driver.Find({param($node) $node -is [Management.Automation.Language.CommandAst] -and $node.Extent.Text.Contains('"$parallel/build-$Architecture.tar"')},$true)
$packAction=[scriptblock]::Create($packCommand.Extent.Text.Replace('$PSScriptRoot','$project/scripts'))
$root=$FixtureRoot;$parallel=Join-Path $FixtureRoot 'build/parallel'
$null=New-Item -ItemType Directory -Path $parallel -Force
foreach($Architecture in 'x64','x86'){
    $triple=if($Architecture -eq 'x64'){'x86_64-pc-windows-msvc'}else{'i686-pc-windows-msvc'}
    $paths=@(if($Architecture -eq 'x64'){'dist'}else{'dist/x86'})+@("windows/target/$triple/release/shanjie_windows.lib","windows/target/$triple/release/shanjie-reliability-check.exe")
    [IO.File]::WriteAllText("$parallel/build-$Architecture.json",'synthetic build receipt')
    & $packAction
    if($LASTEXITCODE){throw "Actual $Architecture archive command failed."}
    $restored=Join-Path $parallel "restored-$Architecture"
    & python "$project/scripts/release_artifacts.py" unpack "$parallel/build-$Architecture.tar" --root $restored
    if($LASTEXITCODE){throw 'Build artifact extraction failed.'}
    $archiveInputs=$paths+@("build/parallel/build-$Architecture.json")
    $expectedFiles=Get-RelativeFileEntries -Root $FixtureRoot -Paths $archiveInputs -RequirePaths
    $restoredFiles=Get-RelativeFileEntries -Root $restored -Paths $archiveInputs -RequirePaths
    Assert-ReleaseFileMap $expectedFiles $restoredFiles "Transported $Architecture build artifacts"
}
$snapshot=New-ReleaseSnapshot $FixtureRoot ([guid]::NewGuid().ToString('N')) $identity $bundle
$receiptRoot=Join-Path $FixtureRoot "build/reports/parallel-fixture-$($snapshot.id)"
$coreEvidence=Join-Path $receiptRoot 'core'
foreach($plan in $plans){Invoke-CoreTestShard $FixtureRoot $snapshot $plan.id "$coreEvidence/$($plan.id)"}
Assert-CoreShardEvidence $FixtureRoot $snapshot $coreEvidence
# Rehash the intentionally changed log in each fixture receipt so these cases
# exercise assertion parsing after stream integrity has already been checked.
$shareReceiptPath=Join-Path $coreEvidence 'share/receipt.json'
$shareReceiptOriginal=[IO.File]::ReadAllText($shareReceiptPath)
$shareReceipt=$shareReceiptOriginal | ConvertFrom-Json -AsHashtable
$coreResult=$shareReceipt.results | Where-Object name -eq 'core'
$coreLogPath=Join-Path (Split-Path $shareReceiptPath -Parent) $coreResult.stdout
$coreLogOriginal=[IO.File]::ReadAllText($coreLogPath)
foreach($invalidLog in @(
    $coreLogOriginal.Replace("test expected_panic - should panic ... ok`r`n",''),
    $coreLogOriginal.Replace('test expected_panic - should panic ... ok','test expected_panic - should panic ... FAILED'),
    ($coreLogOriginal+"test expected_panic - should panic ... ok`r`n")
)){
    try{
        [IO.File]::WriteAllText($coreLogPath,$invalidLog)
        $coreResult.stdout_sha256=(Get-FileHash $coreLogPath).Hash.ToLowerInvariant()
        $shareReceipt | ConvertTo-Json -Depth 12 | Set-Content $shareReceiptPath -Encoding utf8NoBOM
        $rejected=$false
        try{Assert-CoreShardEvidence $FixtureRoot $snapshot $coreEvidence *> $null}catch{$rejected=$true}
        if(!$rejected){throw 'Missing, failed or duplicated expected-panic assertions must reject shard evidence.'}
    }finally{
        [IO.File]::WriteAllText($coreLogPath,$coreLogOriginal)
        [IO.File]::WriteAllText($shareReceiptPath,$shareReceiptOriginal)
    }
}
$previousFailure=$env:SHANJIE_SYNTHETIC_CORE_FAIL
$env:SHANJIE_SYNTHETIC_CORE_FAIL='engine_mixed'
$failedRoot=Join-Path $receiptRoot 'failed-process'
$rejected=$false
try{Invoke-CoreTestShard $FixtureRoot $snapshot mixed $failedRoot}catch{$rejected=$true}
finally{$env:SHANJIE_SYNTHETIC_CORE_FAIL=$previousFailure}
$failedReceipt=Get-Content "$failedRoot/receipt.json" -Raw | ConvertFrom-Json
if(!$rejected -or $failedReceipt.status -ne 'failed' -or $failedReceipt.results.Count -ne 1){throw 'Actual shard failure did not stop before the next binary.'}
$receipts=@(Get-ChildItem $coreEvidence -Recurse -Filter receipt.json)
$firstPath=$receipts[0].FullName
$original=[IO.File]::ReadAllText($firstPath)
function Reject-Core([string]$Name,[scriptblock]$Change){
    $record=$original | ConvertFrom-Json -AsHashtable
    & $Change $record
    $record | ConvertTo-Json -Depth 12 | Set-Content $firstPath -Encoding utf8NoBOM
    $denied=$false
    try{Assert-CoreShardEvidence $FixtureRoot $snapshot $coreEvidence *> $null}catch{$denied=$true}
    finally{[IO.File]::WriteAllText($firstPath,$original)}
    if(!$denied){throw "Invalid core evidence was accepted: $Name"}
}
Reject-Core 'failed shard' {param($r) $r.status='failed'}
Reject-Core 'one omitted test' {param($r) $r.results=@()}
Reject-Core 'duplicate test' {param($r) $r.results+=@($r.results[0])}
Reject-Core 'snapshot SHA-256 mismatch' {param($r) $r.snapshot_sha256='0'*64}
Reject-Core 'old source' {param($r) $r.commit='0'*40}
Reject-Core 'old Actions run' {param($r) $r.run_id='different-run'}
Reject-Core 'test result reuse' {param($r) $r.results[0].reused=$true}
Reject-Core 'changed binary fingerprint' {param($r) $r.results[0].executable_sha256='0'*64}
$firstReceipt=$original | ConvertFrom-Json
$testLog=Join-Path $receipts[0].DirectoryName $firstReceipt.results[0].stdout
$originalLog=[IO.File]::ReadAllText($testLog)
try{
    [IO.File]::AppendAllText($testLog,'changed log')
    $rejected=$false
    try{Assert-CoreShardEvidence $FixtureRoot $snapshot $coreEvidence *> $null}catch{$rejected=$true}
    if(!$rejected){throw 'Changed core test log was accepted.'}
}finally{[IO.File]::WriteAllText($testLog,$originalLog)}
Move-Item -LiteralPath $firstPath -Destination "$firstPath.missing"
$rejected=$false
try{Assert-CoreShardEvidence $FixtureRoot $snapshot $coreEvidence *> $null}catch{$rejected=$true}
finally{Move-Item -LiteralPath "$firstPath.missing" -Destination $firstPath}
if(!$rejected){throw 'A missing shard artifact was accepted.'}
$latencyReceipt=Get-Content "$coreEvidence/latency/receipt.json" -Raw | ConvertFrom-Json
if(@($latencyReceipt.results | Where-Object { $_.name -in 'engine_lm','engine_replay','engine_newword' -and ($_.threads -ne 1 -or $_.processes -ne 1) }).Count){throw 'Latency isolation was lost.'}
$model="$($snapshot.root)/core-tests/test/data/lm/model"
(Get-Item $model).IsReadOnly=$false
try{
    [IO.File]::AppendAllText($model,'changed')
    $rejected=$false
    try{$null=Get-ReleaseSnapshot $FixtureRoot $snapshot.id $snapshot.sha256 -VerifyFiles}catch{$rejected=$true}
    if(!$rejected){throw 'Changed sealed core model was accepted.'}
}finally{[IO.File]::WriteAllText($model,'synthetic model');(Get-Item $model).IsReadOnly=$true}

# Real CTest writes the logical core-suite JUnit only after a delegate has
# validated the complete shard evidence; no passed XML is manufactured.
$runner=Join-Path $FixtureRoot 'build/parallel-aggregate.ps1'
@"
`$ErrorActionPreference='Stop'
Import-Module '$project/scripts/release-support.psm1'
Import-Module '$project/scripts/core-shards.psm1'
`$snapshot=Get-ReleaseSnapshot '$FixtureRoot' '$($snapshot.id)' '$($snapshot.sha256)' -VerifyFiles
Assert-CoreShardEvidence '$FixtureRoot' `$snapshot '$coreEvidence'
"@ | Set-Content $runner -Encoding utf8NoBOM
$pwsh=(Get-Command pwsh -CommandType Application | Select-Object -First 1).Source.Replace('\','/')
$runnerPath=$runner.Replace('\','/')
Add-Content "$FixtureRoot/build/cmake/windows-x64/CTestTestfile.cmake" "add_test(full.core-suite `"$pwsh`" -NoProfile -File `"$runnerPath`")`nset_tests_properties(full.core-suite PROPERTIES LABELS `"full;core`")"
$junit=Join-Path $receiptRoot 'core.xml';$log=Join-Path $receiptRoot 'core.log'
& ctest --test-dir "$FixtureRoot/build/cmake/windows-x64" -C Release -R '^full[.]core-suite$' --output-junit $junit --output-log $log *> $null
if($LASTEXITCODE){throw 'Real core receipt aggregation delegate failed.'}
$record=($Prepack | ConvertTo-Json -Depth 20) | ConvertFrom-Json -AsHashtable
$record.release.candidate_id=$snapshot.id;$record.release.snapshot_sha256=$snapshot.sha256
$record.ctest=@(foreach($old in $record.ctest){
    $folder=Join-Path $receiptRoot $old.preset
    $null=New-Item -ItemType Directory -Path $folder -Force
    Copy-Item $old.junit "$folder/ctest.xml"
    Copy-Item $old.log "$folder/ctest.log"
    @{preset=$old.preset;attempt=1;junit="$folder/ctest.xml";log="$folder/ctest.log";snapshot_id=$snapshot.id;snapshot_sha256=$snapshot.sha256}
})+@(@{preset='windows-x64';attempt=1;junit=$junit;log=$log;snapshot_id=$snapshot.id;snapshot_sha256=$snapshot.sha256})
$record.selection.selected+=@(@{preset='windows-x64';name='full.core-suite'})
$record.results=@(Read-CtestResults $record.ctest)
$verification=@(foreach($arch in 'x64','x86'){
    $folder=Join-Path $receiptRoot "windows-$arch"
    @{architecture=$arch;root=$folder;status='passed';commit=$identity.commit;snapshot_id=$snapshot.id;snapshot_sha256=$snapshot.sha256;
        run_id=$env:GITHUB_RUN_ID;files=(Get-RelativeFileEntries -Root $folder -Paths @('.') -RequirePaths)}
})
$record.parallel=@{run_id=$env:GITHUB_RUN_ID;core_evidence=$coreEvidence;verification=$verification}
$permitPath=Join-Path $receiptRoot 'permit.json'
function Reject-ParallelPermit([string]$Name,[scriptblock]$Change){
    $bad=($record | ConvertTo-Json -Depth 20) | ConvertFrom-Json -AsHashtable
    & $Change $bad
    $rejected=$false
    try{$null=New-ReleasePackagingPermit $FixtureRoot $permitPath $bad}catch{$rejected=$true}
    if(!$rejected -or (Test-Path $permitPath)){throw "Parallel failure authorized packaging: $Name"}
}
Reject-ParallelPermit 'failed x86' {param($r) ($r.results | Where-Object preset -eq windows-x86).status='failed'}
Reject-ParallelPermit 'missing x86 evidence' {param($r) $r.parallel.verification=@($r.parallel.verification[0])}
Reject-ParallelPermit 'wrong verification snapshot' {param($r) $r.parallel.verification[1].snapshot_sha256='0'*64}
Reject-ParallelPermit 'missing core results despite passing logical JUnit' {param($r) $r.parallel.core_evidence="$receiptRoot/missing"}
Reject-ParallelPermit 'parallel evidence cannot be stripped' {param($r) $r.Remove('parallel')}
$permit=New-ReleasePackagingPermit $FixtureRoot $permitPath $record
$rejected=$false
try{$null=Assert-ReleasePackagingPermit $FixtureRoot $permit.path ('0'*32) $snapshot.sha256}catch{$rejected=$true}
if(!$rejected){throw 'Package using another snapshot was accepted.'}
$null=Assert-ReleasePackagingPermit $FixtureRoot $permit.path $snapshot.id $snapshot.sha256
'PASS parallel release guards: 37 executed synthetic binaries, four exact shards, latency isolation, real CTest aggregation, fingerprint/log/snapshot checks and packaging failure gates; no product acceptance executed'
