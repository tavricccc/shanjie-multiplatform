Import-Module "$PSScriptRoot/test-support.psm1" -ErrorAction Stop

function Get-CoreTestNames([string]$ProjectRoot) {
    @('core') + @(Get-ChildItem "$ProjectRoot/upstream/shanjie/core/tests" -Filter *.rs |
        Where-Object BaseName -ne 'learn_store' | Sort-Object BaseName | ForEach-Object BaseName)
}

function Get-CoreSourceFingerprint([string]$ProjectRoot) {
    $paths=@('UPSTREAM.json','MODEL-SOURCES.json','.cargo/config.toml',
        'upstream/shanjie/core/src','upstream/shanjie/core/tests','upstream/shanjie/core/Cargo.toml')
    $paths+=@(& git -C $ProjectRoot -c core.quotepath=false ls-files -- scripts platform)
    Get-ContentFingerprint -Root $ProjectRoot -Paths $paths
}

function Get-CoreShardPlan([string[]]$Names) {
    # Run 45828667360, seconds. Remaining suites use that run's measured costs.
    $costs=@{engine_cap=505.7;engine_mixed=381.6;engine_share=304.9;engine_lm=132.6;engine_replay=121.0;engine_newword=2.5;
        engine_auto_english=138.7;engine_pool_recur=129.5;engine_learn=121.8;engine_digits=109.5;engine_english=85.7;
        engine_pack=76.7;engine_colloq=65.8;engine_fuzzy=64.3;engine_tone3=62.8;engine_rerank=60.3;core=59.6;
        engine_colloquial=55.1;engine=52.3;reading_decode=40.3;colloquial=0.3;engine_demote=6.2;engine_digits_off=4.4;
        engine_english_next=5.7;engine_fuzzy_rerank=6.2;engine_option=22.2;engine_predict=14.7;engine_recur=7.4;
        engine_repick=3.5;engine_scheme=0.8;engine_vertical=3.1;kn_tiny=0.3;pack_oov=0.3;reading_mass=0.9;
        rerank_protect=7.3;tinylm_tiny=0.3;tri_tiny=0.3}
    $shards=@(
        @{id='cap';tests=@('engine_cap');estimated_seconds=505.7},
        @{id='mixed';tests=@('engine_mixed');estimated_seconds=381.6},
        @{id='share';tests=@('engine_share');estimated_seconds=304.9},
        @{id='latency';tests=@('engine_lm','engine_replay','engine_newword');estimated_seconds=256.1}
    )
    $reserved=@($shards | ForEach-Object tests)
    foreach($name in $Names | Where-Object {$_ -notin $reserved} | Sort-Object @{Expression={if($costs.ContainsKey($_)){$costs[$_]}else{5}};Descending=$true},@{Expression={$_}}){
        $shard=$shards | Where-Object id -ne 'cap' | Sort-Object estimated_seconds,id | Select-Object -First 1
        $shard.tests+=@($name)
        $shard.estimated_seconds+=if($costs.ContainsKey($name)){$costs[$name]}else{5}
    }
    Assert-CoreShardPlan $Names $shards
    $shards
}

function Assert-CoreShardPlan([string[]]$Names,[object[]]$Shards) {
    $assigned=@($Shards | ForEach-Object tests)
    $expected=[Collections.Generic.HashSet[string]]::new([string[]]$Names,[StringComparer]::Ordinal)
    if($Names.Count -ne 37 -or $expected.Count -ne $Names.Count -or $assigned.Count -ne $Names.Count -or !$expected.SetEquals([string[]]$assigned)){
        throw 'All 37 upstream core test binaries must be assigned exactly once.'
    }
    if(($Shards.id -join ',') -cne 'cap,mixed,share,latency' -or ($Shards[0].tests -join ',') -cne 'engine_cap') {throw 'Invalid core shard identities or dedicated cap shard.'}
    foreach($name in 'engine_lm','engine_replay','engine_newword'){
        if($name -notin $Shards[3].tests){throw 'Latency suites must share their dedicated sequential runner.'}
    }
}

function New-CoreTestBundle([string]$ProjectRoot,[string]$Destination,[object[]]$Binaries,[string]$Resources,[string]$Models,[string]$SourceKey,[string[]]$SkipArguments) {
    if(Test-Path -LiteralPath $Destination){throw 'Core bundles cannot overwrite existing artifacts.'}
    $tree=Join-Path $ProjectRoot 'build/test/core'
    foreach($path in 'src','tests','Cargo.toml','Cargo.lock'){Copy-ReleaseFiles "$tree/$path" "$Destination/test/core/$path"}
    foreach($path in 'eval','docs'){Copy-ReleaseFiles "$Resources/$path" "$Destination/test/$path"}
    foreach($item in Get-ChildItem "$Resources/data" | Where-Object Name -ne 'lm'){Copy-ReleaseFiles $item.FullName "$Destination/test/data/$($item.Name)"}
    Copy-ReleaseFiles $Models "$Destination/test/data/lm"
    $rows=@(foreach($binary in $Binaries | Sort-Object Name){
        $relative='test/core/target/release/deps/'+[IO.Path]::GetFileName($binary.Path)
        Copy-ReleaseFiles $binary.Path "$Destination/$relative"
        $listing=@(& $binary.Path --list @SkipArguments)
        if($LASTEXITCODE){throw "Cannot enumerate core assertions: $($binary.Name)"}
        $cases=@($listing | Where-Object {$_ -match ': test$'} | ForEach-Object {$_ -replace ': test$',''})
        if(!$cases.Count){throw "Empty core executable: $($binary.Name)"}
        @{name=$binary.Name;executable=$relative;cases=$cases}
    })
    # rustc's runtime dependencies, if the selected toolchain emitted any.
    foreach($directory in $Binaries.Path | ForEach-Object {Split-Path $_ -Parent} | Sort-Object -Unique){
        foreach($file in Get-ChildItem -LiteralPath $directory -Filter *.dll){Copy-ReleaseFiles $file.FullName "$Destination/test/core/target/release/deps/$($file.Name)"}
    }
    $names=Get-CoreTestNames $ProjectRoot
    if(@($rows).Count -ne $names.Count -or @(Compare-Object $names @($rows.name)).Count){throw 'Cargo did not prepare the complete upstream core inventory.'}
    $record=[ordered]@{schema=1;commit=(& git -C $ProjectRoot rev-parse HEAD);source_fingerprint=(Get-CoreSourceFingerprint $ProjectRoot);
        toolchain=$env:SHANJIE_RELEASE_TOOLCHAIN_IDENTITY;
        source_key=$SourceKey;manifest_directory=$tree;skip_arguments=$SkipArguments;binaries=$rows;shards=@(Get-CoreShardPlan $names);
        files=(Get-RelativeFileEntries -Root $Destination -Paths @('test') -RequirePaths)}
    $record | ConvertTo-Json -Depth 12 | Set-Content "$Destination/bundle.json" -Encoding utf8NoBOM
}

function Assert-CoreTestBundle([string]$ProjectRoot,[string]$BundleRoot) {
    $record=Get-Content "$BundleRoot/bundle.json" -Raw | ConvertFrom-Json -AsHashtable
    if($record.schema -ne 1 -or $record.commit -cne (& git -C $ProjectRoot rev-parse HEAD) -or
        $record.source_fingerprint -cne (Get-CoreSourceFingerprint $ProjectRoot)){throw 'Core test artifact has stale or inconsistent source fingerprints.'}
    Assert-CoreShardPlan (Get-CoreTestNames $ProjectRoot) $record.shards
    $files=Get-RelativeFileEntries -Root $BundleRoot -Paths @('test') -RequirePaths
    Assert-ReleaseFileMap $record.files $files 'Core executables, models, fixtures and generated sources'
    foreach($binary in $record.binaries){
        if(!$record.files.ContainsKey($binary.executable) -or !$binary.cases.Count){throw 'Core executable inventory is incomplete.'}
    }
    if($record.binaries.Count -ne 37 -or @(Compare-Object (Get-CoreTestNames $ProjectRoot) @($record.binaries.name)).Count){throw 'Core binary inventory differs from the pinned upstream suite.'}
    $record
}

function Invoke-CoreTestShard([string]$ProjectRoot,$Snapshot,[string]$Shard,[string]$OutputDirectory,[string[]]$CiTests) {
    $bundleRoot=Join-Path $Snapshot.root 'core-tests'
    $bundle=Assert-CoreTestBundle $ProjectRoot $bundleRoot
    # Upstream uses compile-time env!(CARGO_MANIFEST_DIR). Actions checkouts have
    # the same path; refuse a relocated artifact rather than reading other data.
    $tree=Join-Path $ProjectRoot 'build/test/core'
    if([IO.Path]::GetFullPath($bundle.manifest_directory) -cne [IO.Path]::GetFullPath($tree)){throw 'Core binary compile-time fixture path differs from this checkout.'}
    $plan=if($CiTests){
        if($env:GITHUB_ACTIONS -ne 'true' -or !$Snapshot.files.ContainsKey('ci-plan.json')){throw 'Dynamic Core shards require the sealed CI plan.'}
        @(@{id=$Shard;tests=$CiTests})
    }else{@($bundle.shards | Where-Object id -ceq $Shard)}
    $plan=@($plan)
    if($plan.Count -ne 1 -or (Test-Path "$OutputDirectory/receipt.json")){throw 'Unknown shard or already executed shard receipt.'}
    Copy-ReleaseFiles "$bundleRoot/test" "$ProjectRoot/build/test"
    $null=New-Item -ItemType Directory -Path $OutputDirectory -Force
    $environment=@{}
    if($CiTests){
        & python "$PSScriptRoot/release_evidence.py" environment --output "$OutputDirectory/environment.json"
        if($LASTEXITCODE){throw 'Cannot identify the Core test execution environment.'}
        $environment=Get-Content "$OutputDirectory/environment.json" -Raw | ConvertFrom-Json -AsHashtable
    }
    $rows=[Collections.Generic.List[object]]::new()
    $status='failed'
    try {
        foreach($name in $plan[0].tests){
            $binary=$bundle.binaries | Where-Object name -ceq $name
            $relative=$binary.executable.Substring('test/'.Length)
            $exe=Join-Path "$ProjectRoot/build/test" $relative
            $token=$name -replace '[^\w-]','_'
            $stdout=Join-Path $OutputDirectory "$token.txt"
            $stderr="$stdout.err"
            $threads=if($name -in 'engine_lm','engine_replay','engine_newword'){1}else{[Environment]::ProcessorCount}
            $env:CARGO_MANIFEST_DIR=$tree
            $arguments=@($bundle.skip_arguments)+@("--test-threads=$threads")
            $watch=[Diagnostics.Stopwatch]::StartNew()
            $process=Start-Process -FilePath $exe -ArgumentList $arguments -WorkingDirectory $tree -WindowStyle Hidden -PassThru -RedirectStandardOutput $stdout -RedirectStandardError $stderr
            try {
                if(!$process.WaitForExit(3600000)){$process.Kill($true);$process.WaitForExit();throw "Core shard test timed out: $name"}
                $exit=$process.ExitCode
            } finally {$process.Dispose()}
            $log=[IO.File]::ReadAllText($stdout)
            $summary=[regex]::Match($log,'test result: ok\. (\d+) passed; 0 failed; (\d+) ignored;')
            $passed=($exit -eq 0 -and $summary.Success -and ([int]$summary.Groups[1].Value+[int]$summary.Groups[2].Value) -eq $binary.cases.Count)
            $rows.Add(@{name=$name;status=if($passed){'passed'}else{'failed'};exit_code=$exit;executed=$true;reused=$false;
                evidence_mode='executed';
                duration_seconds=$watch.Elapsed.TotalSeconds;threads=$threads;processes=1;arguments=$arguments;executable_sha256=$bundle.files[$binary.executable];
                stdout=[IO.Path]::GetFileName($stdout);stderr=[IO.Path]::GetFileName($stderr);stdout_sha256=(Get-FileHash $stdout).Hash.ToLowerInvariant();stderr_sha256=(Get-FileHash $stderr).Hash.ToLowerInvariant()})
            Write-Host ("{0} {1} ({2:F1}s, {3} threads)" -f $rows[-1].status,$name,$watch.Elapsed.TotalSeconds,$threads)
            if(!$passed){Write-Host $log;Get-Content $stderr;throw "Core shard failed: $name"}
        }
        $status='passed'
    } finally {
        @{schema=if($CiTests){6}else{1};shard=$Shard;status=$status;commit=$Snapshot.source.commit;snapshot_id=$Snapshot.id;snapshot_sha256=$Snapshot.sha256;
            bundle_sha256=(Get-FileHash "$bundleRoot/bundle.json").Hash.ToLowerInvariant();run_id=$env:GITHUB_RUN_ID;run_attempt=$env:GITHUB_RUN_ATTEMPT;
            environment=$environment;
            results=@($rows)} | ConvertTo-Json -Depth 10 | Set-Content "$OutputDirectory/receipt.json" -Encoding utf8NoBOM
    }
}

function Assert-CoreShardEvidence([string]$ProjectRoot,$Snapshot,[string]$EvidenceRoot) {
    $bundleRoot=Join-Path $Snapshot.root 'core-tests'
    $bundle=Assert-CoreTestBundle $ProjectRoot $bundleRoot
    $receipts=@(Get-ChildItem -LiteralPath $EvidenceRoot -Recurse -Filter receipt.json)
    if($receipts.Count -ne $bundle.shards.Count){throw 'Missing or duplicate core shard artifacts.'}
    $seen=[Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach($plan in $bundle.shards){
        $matches=@($receipts | Where-Object {(Get-Content $_.FullName -Raw | ConvertFrom-Json).shard -ceq $plan.id})
        if($matches.Count -ne 1){throw "Missing or duplicate shard: $($plan.id)"}
        $receipt=Get-Content $matches[0].FullName -Raw | ConvertFrom-Json -AsHashtable
        if($receipt.schema -ne 1 -or $receipt.status -cne 'passed' -or $receipt.commit -cne $Snapshot.source.commit -or
            $receipt.snapshot_id -cne $Snapshot.id -or $receipt.snapshot_sha256 -cne $Snapshot.sha256 -or
            $receipt.bundle_sha256 -cne (Get-FileHash "$bundleRoot/bundle.json").Hash.ToLowerInvariant() -or
            $receipt.run_id -cne $env:GITHUB_RUN_ID){throw 'Core shard evidence is failed, stale or belongs to another snapshot/run.'}
        if($receipt.results.Count -ne $plan.tests.Count -or @(Compare-Object @($plan.tests) @($receipt.results.name)).Count){throw 'Core shard did not execute its exact test assignment.'}
        foreach($result in $receipt.results){
            $binary=$bundle.binaries | Where-Object name -ceq $result.name
            if(!$seen.Add($result.name) -or $result.status -cne 'passed' -or !$result.executed -or $result.reused -or $result.exit_code -ne 0 -or
                $result.executable_sha256 -cne $bundle.files[$binary.executable]){throw 'Missing, duplicated, reused or failed core test result.'}
            $threads=if($result.name -in 'engine_lm','engine_replay','engine_newword'){1}else{$result.threads}
            $arguments=@($bundle.skip_arguments)+@("--test-threads=$threads")
            if($result.processes -ne 1 -or $result.threads -lt 1 -or ($result.arguments -join "`n") -cne ($arguments -join "`n")){throw 'Core test arguments or latency isolation changed.'}
            foreach($stream in 'stdout','stderr'){
                $file=Join-Path $matches[0].DirectoryName $result[$stream]
                if([IO.Path]::GetFileName($result[$stream]) -cne $result[$stream] -or (Get-FileHash $file).Hash.ToLowerInvariant() -cne $result["${stream}_sha256"]){throw 'Core shard test log is missing or changed.'}
            }
            $log=Get-Content (Join-Path $matches[0].DirectoryName $result.stdout) -Raw
            $summary=[regex]::Match($log,'test result: ok\. (\d+) passed; 0 failed; (\d+) ignored;')
            if(!$summary.Success -or ([int]$summary.Groups[1].Value+[int]$summary.Groups[2].Value) -ne $binary.cases.Count){throw 'Core assertion log coverage is incomplete.'}
            foreach($case in $binary.cases){
                if([regex]::Matches($log,('(?m)^test '+[regex]::Escape($case)+'(?: - should panic)? \.\.\. (ok|ignored(?:, [^\r\n]*)?)\r?$')).Count -ne 1){throw "Missing or duplicated core assertion log: $($result.name)/$case"}
            }
            Write-Host ("Verified {0}/{1}: {2:F1}s, executable {3}" -f $plan.id,$result.name,$result.duration_seconds,$result.executable_sha256)
        }
    }
    if(!$seen.SetEquals([string[]](Get-CoreTestNames $ProjectRoot))){throw 'Upstream core suite coverage is incomplete.'}
    Write-Host 'PASS all 37 core test binaries: exact shard ownership, executed assertions, logs, source and snapshot hashes verified.'
}

Export-ModuleMember -Function Get-CoreTestNames,Get-CoreSourceFingerprint,Get-CoreShardPlan,Assert-CoreShardPlan,New-CoreTestBundle,Assert-CoreTestBundle,Invoke-CoreTestShard,Assert-CoreShardEvidence
