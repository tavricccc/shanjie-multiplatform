#Requires -Version 7
param([string]$Commit='HEAD',[string]$BaselineCommit,[string]$OutputRoot=$env:SHANJIE_BENCHMARK_ROOT,[switch]$KeepWorktrees)
$ErrorActionPreference='Stop'
if(!$IsWindows){throw 'This benchmark requires a Windows host with MSVC, Windows SDK, Rust, PowerShell 7, and .NET 10.'}
$root=Split-Path $PSScriptRoot -Parent
$resolved=& git -C $root rev-parse --verify "$Commit^{commit}"
if($LASTEXITCODE){throw "Cannot resolve benchmark commit: $Commit"}
$head=(& git -C $root rev-parse --verify 'HEAD^{commit}').Trim()
if($head -ne $resolved){throw "Benchmark commit $resolved must be the checked-out HEAD $head; check out the intended commit first."}
$dirty=@(& git -C $root status --porcelain --untracked-files=normal)
if($LASTEXITCODE -or $dirty.Count){throw 'Benchmark requires a clean worktree so detached runs use the exact checked-out source and submodule pins.'}
if(!$BaselineCommit){
    # After syncing newer product changes, prefer the matching pre-CTest main
    # ancestor; the original migration parent may no longer have the same product.
    $mainBase=& git -C $root merge-base $resolved origin/main 2>$null
    if(!$LASTEXITCODE){
        & git -C $root cat-file -e "${mainBase}:CMakeLists.txt" 2>$null
        if($LASTEXITCODE){$BaselineCommit=$mainBase}
    }
}
if(!$BaselineCommit){
    $migration=& git -C $root log -1 --diff-filter=A --format=%H -- CMakeLists.txt
    if(!$migration){throw 'Specify -BaselineCommit for the version before the verification migration.'}
    $BaselineCommit="$migration^"
}
$baseline=& git -C $root rev-parse --verify "$BaselineCommit^{commit}"
if($LASTEXITCODE){throw "Cannot resolve baseline commit: $BaselineCommit"}
$productPaths=@('windows','shared','platform','ui','profiles','data','models','assets','UPSTREAM.json','MODEL-SOURCES.json','MODEL-CATALOG.json','upstream/shanjie')
$productChanges=@(& git -C $root diff --name-only $baseline $resolved -- @productPaths)
if($LASTEXITCODE -or $productChanges.Count){throw "Baseline and candidate product inputs differ; choose a comparable baseline before claiming a verification speedup: $($productChanges -join ', ')"}
$runId=[guid]::NewGuid().ToString('N')
$benchmarkParent=if($OutputRoot){[IO.Path]::GetFullPath($OutputRoot)}else{Join-Path $root 'build/benchmark'}
$benchmarkRoot=Join-Path $benchmarkParent $runId
$legacyRoot=Join-Path $benchmarkRoot 'legacy'
$baselineRoot=Join-Path $benchmarkRoot 'baseline'
$ctestRoot=Join-Path $benchmarkRoot 'ctest'
$artifactsRoot=Join-Path $benchmarkRoot 'artifacts'
New-Item -ItemType Directory -Path $artifactsRoot -Force|Out-Null
$gitAdded=@()
$measurements=[ordered]@{}
$summary=$null
$benchmarkFailure=$null
$comparisonPath=Join-Path $benchmarkRoot 'comparison.json'
$previousThreads=$env:RUST_TEST_THREADS
# The pre-migration identity assertion shares a four-entry process cache with
# other Rust tests. Use the same serial intra-binary setting for all six runs.
$env:RUST_TEST_THREADS='1'

function Get-RunCoverage([string]$Worktree,[datetime]$Started,[string]$Log,$Report,[string]$EvidenceRoot) {
    $ids=[Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    $texts=[Collections.Generic.List[string]]::new()
    if($Report -and $Report.level -eq 'FULL'){
        foreach($ctestReport in $Report.ctest){
            [xml]$xml=Get-Content -LiteralPath $ctestReport.junit -Raw
            foreach($output in $xml.SelectNodes('//testcase/system-out')){$texts.Add($output.InnerText)}
        }
    }else{$texts.Add([IO.File]::ReadAllText($Log))}
    foreach($body in $texts){
        $crate=$null
        foreach($line in $body -split '\r?\n'){
            if($line -match 'Running unittests .+[\\/](shanjie_shell|shanjie_windows)-[0-9a-f]+\.exe'){$crate=$Matches[1]}
            elseif($crate -and $line -match '^test (\S+) \.\.\. ok\s*$'){[void]$ids.Add("rust:$crate`:$($Matches[1])")}
            if($line -match '^TSF composition, caret, commit, cancel, candidates, and COM lifetime passed\s*$'){[void]$ids.Add('tsf:standard:x64:windows')}
            if($line -match '^PASS: activation and system shortcuts require no model or lexicon\s*$'){[void]$ids.Add('tsf:cold:x64')}
            if($line -match '^PASS: pure Zhuyin, literal Space, both layouts, both interactions and mode reload\s*$'){[void]$ids.Add('tsf:zhuyin-only:x64')}
            if($line -match '^Upstream data coverage ok: (.+)$'){[void]$ids.Add("data:$($Matches[1].Trim())")}
        }
    }
    $files=[Collections.Generic.List[object]]::new()
    $coreLogs=Join-Path $Worktree 'build/test/core/target/test-logs'
    foreach($file in Get-ChildItem -LiteralPath $coreLogs -Filter '*.txt' -File -ErrorAction SilentlyContinue | Where-Object LastWriteTimeUtc -GE $Started){
        $files.Add($file)
        foreach($line in Get-Content -LiteralPath $file.FullName){
            if($line -match '^test (\S+) \.\.\. ok\s*$'){[void]$ids.Add("core:$($file.BaseName):$($Matches[1])")}
        }
    }
    $caseRoots=@(Get-ChildItem -LiteralPath (Join-Path $Worktree 'build') -Directory -Filter 'regressions-*' -ErrorAction SilentlyContinue)+
        @(Get-ChildItem -LiteralPath (Join-Path $Worktree 'build/reports') -Directory -Filter 'ctest-*' -ErrorAction SilentlyContinue)
    foreach($caseRoot in $caseRoots){
        foreach($file in Get-ChildItem -LiteralPath $caseRoot.FullName -File -Recurse | Where-Object { $_.LastWriteTimeUtc -ge $Started -and $_.Name -in @('result.log','results.json') }){
            $files.Add($file)
            if($file.Name -eq 'result.log'){
                if($file.Directory.Name -match '^mode\d+-layout\d+-(.+)$'){
                    $scenario=$Matches[1]
                    $profile=@{}
                    foreach($line in Get-Content -LiteralPath (Join-Path $file.Directory.FullName 'profile/Shanjie/profile.ini')){if($line -match '^(\w+)=(.*)$'){$profile[$Matches[1]]=$Matches[2]}}
                    if((Get-Content -LiteralPath $file.FullName -Raw) -match 'PASS'){
                        $mode=[int]$profile.mixed_input+2*[int]$profile.upstream_english
                        [void]$ids.Add("english:x64:$scenario`:mode$mode`:keyboard$($profile.keyboard):enter$($profile.enter_to_cursor)")
                    }
                }elseif($file.Directory.Name -match '^x64-theme(\d+)$'){
                    $theme=$Matches[1]
                    if((Get-Content -LiteralPath $file.FullName -Raw) -match 'PASS'){[void]$ids.Add("word:x64:theme$theme")}
                }
            }else{
                foreach($case in @(Get-Content -LiteralPath $file.FullName -Raw | ConvertFrom-Json -AsHashtable)){
                    if($case.ContainsKey('keyboard') -and $case.passed){
                        $configuration=[ordered]@{architecture=$case.architecture;keyboard=$case.keyboard;mode=$case.mode;mix=$case.mix;keys=$case.keys;expected=$case.expected}
                        [void]$ids.Add('submission:'+($configuration|ConvertTo-Json -Compress))
                    }elseif($case.ContainsKey('scenario') -and $case.exit -eq 0){
                        $suite=if($file.FullName -match 'tsf_compat|private-tsf'){'compat'}else{'popup'}
                        [void]$ids.Add("private:x64:$suite`:$($case.scenario)")
                    }
                }
            }
        }
    }
    foreach($file in $files){
        $destination=Join-Path $EvidenceRoot ([IO.Path]::GetRelativePath($Worktree,$file.FullName))
        $null=New-Item -ItemType Directory -Path (Split-Path $destination -Parent) -Force
        Copy-Item -LiteralPath $file.FullName -Destination $destination -Force
    }
    return @($ids | Sort-Object)
}

function Get-Report([string]$Worktree,[string]$ExpectedLevel,[datetime]$Since=[datetime]::MinValue) {
    $files=@(Get-ChildItem -LiteralPath (Join-Path $Worktree 'build/reports') -Filter report.json -File -Recurse -ErrorAction SilentlyContinue | Where-Object LastWriteTimeUtc -GE $Since)
    foreach($file in $files | Sort-Object LastWriteTimeUtc -Descending){
        try{$report=Get-Content -LiteralPath $file.FullName -Raw|ConvertFrom-Json}catch{continue}
        if(($ExpectedLevel -eq 'legacy' -and $report.level -eq 'legacy-verify') -or ($report.level -eq $ExpectedLevel)){return [pscustomobject]@{Path=$file.FullName;Report=$report}}
    }
    throw "No $ExpectedLevel timing report was produced in $Worktree."
}
function Invoke-Run([string]$Name,[string]$Worktree,[ValidateSet('baseline','legacy','full')][string]$Flow,[string]$RunKind) {
    $workBuild=Join-Path $Worktree 'build'
    $null=New-Item -ItemType Directory -Path $workBuild -Force
    $log=Join-Path $workBuild "benchmark-$Flow-$RunKind.log"
    $runArtifacts=Join-Path $artifactsRoot $Name
    $artifactWorktree=Join-Path $runArtifacts 'worktree'
    $null=New-Item -ItemType Directory -Path $artifactWorktree -Force
    Push-Location $Worktree
    $started=[datetime]::UtcNow
    $watch=[Diagnostics.Stopwatch]::StartNew()
    $exit=1
    $timingEnvironment=@{}
    foreach($key in 'SHANJIE_TEST_RUN_ID','SHANJIE_TEST_TIMINGS_DIR','SHANJIE_TEST_PARENT_STAGE_ID'){$timingEnvironment[$key]=[Environment]::GetEnvironmentVariable($key);[Environment]::SetEnvironmentVariable($key,$null)}
    try {
        Write-Host "Benchmark $Name started; live log: $log"
        if($Flow -ne 'full') { & pwsh -NoLogo -NoProfile -File scripts/verify.ps1 *> $log }
        else { & pwsh -NoLogo -NoProfile -File scripts/test.ps1 -Level FULL -BaseRef $resolved *> $log }
        $exit=$LASTEXITCODE
    } finally { $watch.Stop();Pop-Location;foreach($key in $timingEnvironment.Keys){[Environment]::SetEnvironmentVariable($key,$timingEnvironment[$key])} }
    $level=if($Flow -eq 'legacy'){'legacy'}else{'FULL'}
    $artifactBuild=Join-Path $artifactWorktree 'build'
    $null=New-Item -ItemType Directory -Path $artifactBuild -Force
    $sourceReports=Join-Path $workBuild 'reports'
    if(Test-Path -LiteralPath $sourceReports){Copy-Item -LiteralPath $sourceReports -Destination (Join-Path $artifactBuild 'reports') -Recurse -Force}
    $artifactLog=Join-Path $artifactBuild (Split-Path $log -Leaf)
    if(Test-Path -LiteralPath $log){Copy-Item -LiteralPath $log -Destination $artifactLog -Force}
    $report=$null
    if($Flow -ne 'baseline'){try { $report=Get-Report $Worktree $level $started } catch { if(!$exit){throw} }}
    $coverage=@()
    $coverageError=$null
    if(!$exit){try{$coverage=@(Get-RunCoverage $Worktree $started $log $(if($report){$report.Report}else{$null}) (Join-Path $runArtifacts 'evidence'))}catch{$coverageError=$_.Exception.Message}}
    if($Flow -eq 'baseline'){
        $baselineReport=[ordered]@{level='baseline-verify';commit=$baseline;wall_seconds=$watch.Elapsed.TotalSeconds;timings=@();coverage_ids=$coverage;notes=@('Unmodified pre-migration scripts; no original JUnit or stage instrumentation.')}
        $baselinePath=Join-Path $artifactWorktree 'build/reports/baseline/report.json'
        $null=New-Item -ItemType Directory -Path (Split-Path $baselinePath -Parent) -Force
        [IO.File]::WriteAllText($baselinePath,($baselineReport|ConvertTo-Json -Depth 8),[Text.UTF8Encoding]::new($false))
        $report=[pscustomobject]@{Path=$baselinePath;Report=[pscustomobject]$baselineReport}
    }
    if($report -and $Flow -ne 'baseline'){
        $relativeReport=[IO.Path]::GetRelativePath($Worktree,$report.Path)
        $escapedSource=$Worktree.Replace('\','\\')
        $escapedDestination=$artifactWorktree.Replace('\','\\')
        foreach($jsonFile in Get-ChildItem -LiteralPath (Join-Path $artifactWorktree 'build/reports') -Filter '*.json' -File -Recurse -ErrorAction SilentlyContinue){
            $jsonText=[IO.File]::ReadAllText($jsonFile.FullName)
            [IO.File]::WriteAllText($jsonFile.FullName,$jsonText.Replace($escapedSource,$escapedDestination),[Text.UTF8Encoding]::new($false))
        }
        $archivedReportPath=Join-Path $artifactWorktree $relativeReport
        $report=[pscustomobject]@{Path=$archivedReportPath;Report=(Get-Content -LiteralPath $archivedReportPath -Raw|ConvertFrom-Json)}
    }
    $measurements[$Name]=[pscustomobject]@{flow=$Flow;cache_state=$RunKind;status=if($exit){'failed'}else{'passed'};wall_seconds=[Math]::Round($watch.Elapsed.TotalSeconds,3);coverage_ids=$coverage;report_path=if($report){$report.Path}else{$null};log_path=$artifactLog;report=if($report){$report.Report}else{$null}}
    if($exit){throw "$Name failed with exit $exit; archived log: $artifactLog"}
    if(!$report){throw "$Name completed without a timing report; archived log: $artifactLog"}
    if($coverageError){$measurements[$Name].status='not-comparable';throw "Coverage could not be read for $Name`: $coverageError"}
    foreach($prefix in 'rust:shanjie_shell:','rust:shanjie_windows:','core:','english:','submission:','private:x64:compat:','private:x64:popup:','word:','tsf:standard:','tsf:cold:','tsf:zhuyin-only:','data:'){
        if(!@($coverage | Where-Object { $_.StartsWith($prefix) }).Count){$measurements[$Name].status='not-comparable';throw "Coverage evidence is missing $prefix in $Name; the run is not comparable."}
    }
    Write-Host ("Benchmark {0}: {1:F3}s ({2:F2} minutes), {3} observed product cases." -f $Name,$watch.Elapsed.TotalSeconds,($watch.Elapsed.TotalSeconds/60),$coverage.Count)
}
try {
    $worktrees=[ordered]@{baseline=$baselineRoot;legacy=$legacyRoot;ctest=$ctestRoot}
    foreach($name in $worktrees.Keys){
        $worktree=$worktrees[$name]
        $workCommit=if($name -eq 'baseline'){$baseline}else{$resolved}
        & git -C $root worktree add --detach $worktree $workCommit *> (Join-Path $artifactsRoot "$name-worktree.log")
        if($LASTEXITCODE){throw "Could not create $name benchmark worktree."}
        $gitAdded+=,$worktree
        & git -C $worktree submodule update --init --recursive *> (Join-Path $artifactsRoot "$name-submodules.log")
        if($LASTEXITCODE){throw "Could not initialize $name pinned submodule."}
    }
    Invoke-Run 'baseline_cold' $baselineRoot baseline cold
    Invoke-Run 'baseline_warm' $baselineRoot baseline warm
    Invoke-Run 'legacy_cold' $legacyRoot legacy cold
    Invoke-Run 'legacy_warm' $legacyRoot legacy warm
    Invoke-Run 'ctest_cold' $ctestRoot full cold
    Invoke-Run 'ctest_warm' $ctestRoot full warm

    $expected=$measurements.baseline_cold.coverage_ids
    foreach($name in $measurements.Keys){
        $difference=@(Compare-Object $expected $measurements[$name].coverage_ids -CaseSensitive)
        if($difference.Count){
            $difference | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $artifactsRoot "$name-coverage-difference.json") -Encoding utf8NoBOM
            throw "Observed case IDs or configurations differ in $name; inspect coverage-difference.json before claiming a speedup."
        }
    }

    $legacyCold=[double]$measurements.legacy_cold.wall_seconds
    $ctestCold=[double]$measurements.ctest_cold.wall_seconds
    $legacyWarm=[double]$measurements.legacy_warm.wall_seconds
    $ctestWarm=[double]$measurements.ctest_warm.wall_seconds
    $coldSaved=$legacyCold-$ctestCold
    $warmSaved=$legacyWarm-$ctestWarm
    $baselineCold=[double]$measurements.baseline_cold.wall_seconds
    $baselineWarm=[double]$measurements.baseline_warm.wall_seconds
    $ctestRows=@($measurements.ctest_cold.report.selection.selected)
    function Get-TimingCount {
        param([Parameter(Mandatory)]$Report,[Parameter(Mandatory)][string]$Category)
        $matching = @($Report.timings | Where-Object { $_.category -eq $Category })
        return $matching.Count
    }
    function Get-CtestRowCount {
        param([Parameter(Mandatory)][string]$Pattern)
        $matching = @($ctestRows | Where-Object { $_.name -like $Pattern })
        return $matching.Count
    }
    $legacyCaseCounts=[ordered]@{
        shared_unit_suites=(Get-TimingCount -Report $measurements.legacy_cold.report -Category 'rust.unit.shared')
        windows_unit_suites=(Get-TimingCount -Report $measurements.legacy_cold.report -Category 'rust.unit.windows')
        mixed_regression_suites=(Get-TimingCount -Report $measurements.legacy_cold.report -Category 'rust.mixed-regression')
        upstream_core_test_binaries=(Get-TimingCount -Report $measurements.legacy_cold.report -Category 'core.test')
        english_input_cases=(Get-TimingCount -Report $measurements.legacy_cold.report -Category 'english.keys')
        submission_input_cases=(Get-TimingCount -Report $measurements.legacy_cold.report -Category 'submission.case')
        private_desktop_cases=(Get-TimingCount -Report $measurements.legacy_cold.report -Category 'tsf.private-desktop')
        add_word_cases=(Get-TimingCount -Report $measurements.legacy_cold.report -Category 'word.add')
        tsf_standard_smokes=(Get-TimingCount -Report $measurements.legacy_cold.report -Category 'tsf.smoke')
        tsf_cold_smokes=(Get-TimingCount -Report $measurements.legacy_cold.report -Category 'tsf.cold-start')
    }
    $ctestCaseCounts=[ordered]@{
        shared_unit_suites=(Get-TimingCount -Report $measurements.ctest_cold.report -Category 'rust.unit.shared')
        windows_unit_suites=(Get-TimingCount -Report $measurements.ctest_cold.report -Category 'rust.unit.windows')
        mixed_regression_suites=(Get-TimingCount -Report $measurements.ctest_cold.report -Category 'rust.mixed-regression')
        upstream_core_test_binaries=(Get-TimingCount -Report $measurements.ctest_cold.report -Category 'core.test')
        english_input_cases=(Get-TimingCount -Report $measurements.ctest_cold.report -Category 'english.keys')
        submission_input_cases=(Get-TimingCount -Report $measurements.ctest_cold.report -Category 'submission.case')
        private_desktop_cases=(Get-TimingCount -Report $measurements.ctest_cold.report -Category 'tsf.private-desktop')
        add_word_cases=(Get-TimingCount -Report $measurements.ctest_cold.report -Category 'word.add')
        tsf_standard_smokes=(Get-TimingCount -Report $measurements.ctest_cold.report -Category 'tsf.smoke')
        tsf_cold_smokes=(Get-TimingCount -Report $measurements.ctest_cold.report -Category 'tsf.cold-start')
    }
    foreach($category in $legacyCaseCounts.Keys){
        if(!$legacyCaseCounts[$category] -or $legacyCaseCounts[$category] -ne $ctestCaseCounts[$category]){
            throw "Benchmark coverage mismatch for ${category}: legacy=$($legacyCaseCounts[$category]), CTest=$($ctestCaseCounts[$category]). Timing improvement is not comparable."
        }
    }
    foreach($flow in 'legacy','ctest'){
        $coldCategories=@($measurements["${flow}_cold"].report.timings.category | Sort-Object -Unique)
        foreach($category in $coldCategories | Where-Object { $_ -in @('core.test','english.keys','submission.case','tsf.private-desktop','word.add','tsf.smoke','tsf.cold-start','rust.unit.shared','rust.unit.windows','rust.mixed-regression') }){
            if((Get-TimingCount $measurements["${flow}_cold"].report $category) -ne (Get-TimingCount $measurements["${flow}_warm"].report $category)){
                throw "Benchmark cold/warm coverage mismatch for $flow/$category."
            }
        }
    }
    $ctestGroupCounts=[ordered]@{
        fast=(Get-CtestRowCount 'fast.*')
        core_suite=(Get-CtestRowCount 'full.core-suite')
        mixed_regression=(Get-CtestRowCount '*.mixed-regression.*')
        data=(Get-CtestRowCount '*.dist-data')
        payload=(Get-CtestRowCount '*.payload')
        tsf_smoke=(Get-CtestRowCount '*.tsf-smoke.*')
        add_word=(Get-CtestRowCount '*.word.add.*')
        english=(Get-CtestRowCount '*.english.*')
        submission=(Get-CtestRowCount '*.submission.*')
        private_tsf=(Get-CtestRowCount '*.private.tsf-compat.*')
        private_popup=(Get-CtestRowCount '*.private.popup-lifecycle.*')
    }
    $categorySums=[ordered]@{}
    foreach($name in $measurements.Keys){
        $categorySums[$name]=@($measurements[$name].report.timings | Group-Object category | ForEach-Object {
            [pscustomobject]@{category=$_.Name;inclusive_seconds=[Math]::Round((($_.Group | Measure-Object duration_ms -Sum).Sum/1000),3);exclusive_seconds=[Math]::Round((($_.Group | Measure-Object exclusive_duration_ms -Sum).Sum/1000),3)}
        })
    }
    $summary=[ordered]@{
        schema=1
        commit=$resolved
        baseline_commit=$baseline
        status='measured'
        comparable_scope='Legacy verify.ps1 and CTest FULL x64; both include shared/windows unit tests, all upstream core test binaries, mixed-input regressions, data validation, TSF smoke, add-word, English-key, submission, and each private-desktop case.'
        coverage=[ordered]@{
            legacy_entry='scripts/verify.ps1 (compatibility path; all original assertions retained)'
            ctest_case_count=[int]$ctestRows.Count
            ctest_case_names=@($ctestRows | ForEach-Object name)
            ctest_group_counts=$ctestGroupCounts
            observed_case_counts=[ordered]@{legacy_verify=$legacyCaseCounts;ctest_full=$ctestCaseCounts}
            observed_ids=$expected
            exact_configuration_match=$true
            additional_checks='The candidate flows also run test-infrastructure self-tests, which are not part of the pre-migration product acceptance scope.'
            preserved_suite_list=@('upstream data static and built-output checks','shared and windows Cargo --lib','upstream core library and every integration-test binary, with latency suites serialized','ignored mixed-input and digit regressions','TSF standard, pure Zhuyin and cold activation','add-word popup','all 19 English-key scenarios across their original mode/layout combinations','all 21 submission mode groups plus the input-digit matrix','all 18 private TSF compatibility cases','all 15 popup lifecycle private-desktop cases')
        }
        measurements=[ordered]@{
            baseline_cold=[ordered]@{wall_seconds=$baselineCold;report=$measurements.baseline_cold.report_path;log=$measurements.baseline_cold.log_path}
            baseline_warm=[ordered]@{wall_seconds=$baselineWarm;report=$measurements.baseline_warm.report_path;log=$measurements.baseline_warm.log_path}
            legacy_cold=[ordered]@{wall_seconds=$legacyCold;report=$measurements.legacy_cold.report_path;log=$measurements.legacy_cold.log_path}
            legacy_warm=[ordered]@{wall_seconds=$legacyWarm;report=$measurements.legacy_warm.report_path;log=$measurements.legacy_warm.log_path}
            ctest_cold=[ordered]@{wall_seconds=$ctestCold;report=$measurements.ctest_cold.report_path;log=$measurements.ctest_cold.log_path}
            ctest_warm=[ordered]@{wall_seconds=$ctestWarm;report=$measurements.ctest_warm.report_path;log=$measurements.ctest_warm.log_path}
        }
        comparison=[ordered]@{
            baseline_cold_saved_seconds=[Math]::Round($baselineCold-$ctestCold,3)
            baseline_cold_speedup_percent=[Math]::Round(100*($baselineCold-$ctestCold)/$baselineCold,2)
            baseline_warm_saved_seconds=[Math]::Round($baselineWarm-$ctestWarm,3)
            baseline_warm_speedup_percent=[Math]::Round(100*($baselineWarm-$ctestWarm)/$baselineWarm,2)
            baseline_scope='Unmodified scripts from baseline_commit versus candidate CTest FULL; product sources, data, models, pins, and observed case/configuration IDs must match.'
            same_commit_scope='The legacy_* versus ctest_* fields compare the two candidate entry points and therefore share the candidate caching improvements.'
            cold_saved_seconds=[Math]::Round($coldSaved,3)
            cold_speedup_percent=if($legacyCold){[Math]::Round(100*$coldSaved/$legacyCold,2)}else{$null}
            warm_saved_seconds=[Math]::Round($warmSaved,3)
            warm_speedup_percent=if($legacyWarm){[Math]::Round(100*$warmSaved/$legacyWarm,2)}else{$null}
            ctest_management_only='Not isolated: CTest centralizes scheduling and reporting; it is not counted as a runtime optimization by itself.'
            incremental_and_cache='Compare cold versus warm runs and build.cpp.harness, build.rust.release, build.winui.publish, models.*, and data.* stage events in the referenced reports.'
            parallelism='The existing memory-limited upstream core scheduler remains in both flows. Independent CTest cases use resource locks; effects are included in end-to-end wall time but not isolated by an ablation run.'
            selective_affected='Not part of this equal-scope FULL comparison. Each AFFECTED report records its own selected rows and wall time; compare only with a FULL run for the full-suite baseline.'
            model='Cold and warm mean fresh versus reused project build/test outputs; shared NuGet/Cargo download caches and the machine file cache may already be warm.'
            rust_test_threads='RUST_TEST_THREADS=1 for all six runs to isolate the original process-wide cache identity assertion. Independent upstream test binaries still use the original memory-limited parallel scheduler.'
            category_model='Inclusive time contains child stages; exclusive time subtracts their interval union using parent IDs. Parallel work can exceed wall time. The uninstrumented baseline has no category breakdown.'
        }
        category_sums=$categorySums
        generated_utc=[DateTimeOffset]::UtcNow.ToString('O')
    }
    [IO.File]::WriteAllText($comparisonPath,($summary|ConvertTo-Json -Depth 16),[Text.UTF8Encoding]::new($false))
    Write-Host "Benchmark report: $comparisonPath"
    Write-Host ('Before migration cold/warm: {0:F2}/{1:F2} minutes; CTest cold/warm: {2:F2}/{3:F2} minutes.' -f ($baselineCold/60),($baselineWarm/60),($ctestCold/60),($ctestWarm/60))
    Write-Host ('Cold: {0:F3}s -> {1:F3}s; saved {2:F3}s ({3:F2}%).' -f $legacyCold,$ctestCold,$coldSaved,$summary.comparison.cold_speedup_percent)
    Write-Host ('Warm: {0:F3}s -> {1:F3}s; saved {2:F3}s ({3:F2}%).' -f $legacyWarm,$ctestWarm,$warmSaved,$summary.comparison.warm_speedup_percent)
} catch {
    $benchmarkFailure=$_.Exception.Message
    throw
} finally {
    if(!$summary){
        $partial=[ordered]@{
            schema=1
            commit=$resolved
            baseline_commit=$baseline
            status='failed'
            failure=$benchmarkFailure
            measurements=$measurements
            not_run=@('baseline_cold','baseline_warm','legacy_cold','legacy_warm','ctest_cold','ctest_warm' | Where-Object { $_ -notin $measurements.Keys } | ForEach-Object { [pscustomobject]@{name=$_;reason='Benchmark stopped before this run; see failure and archived logs.'} })
            comparison=[ordered]@{status='not-comparable';cold_saved_seconds=$null;warm_saved_seconds=$null}
            generated_utc=[DateTimeOffset]::UtcNow.ToString('O')
        }
        [IO.File]::WriteAllText($comparisonPath,($partial|ConvertTo-Json -Depth 20),[Text.UTF8Encoding]::new($false))
        Write-Host "Incomplete benchmark report: $comparisonPath"
    }
    $env:RUST_TEST_THREADS=$previousThreads
    if(!$KeepWorktrees){foreach($path in $gitAdded){
        $cleanupPath=[IO.Path]::GetFullPath($path)
        if([IO.Path]::GetDirectoryName($cleanupPath) -ne [IO.Path]::GetFullPath($benchmarkRoot)){throw 'Benchmark cleanup path must remain directly under its own run directory.'}
        & git -C $root -c core.longpaths=true worktree remove --force $cleanupPath *> $null
    }}
    if($gitAdded.Count){& git -C $root worktree prune}
}
