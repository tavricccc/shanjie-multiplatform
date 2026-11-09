#Requires -Version 7
param([string]$Commit='HEAD',[switch]$KeepWorktrees)
$ErrorActionPreference='Stop'
if(!$IsWindows){throw 'This benchmark requires a Windows host with MSVC, Windows SDK, Rust, PowerShell 7, and .NET 10.'}
$root=Split-Path $PSScriptRoot -Parent
$resolved=& git -C $root rev-parse --verify "$Commit^{commit}"
if($LASTEXITCODE){throw "Cannot resolve benchmark commit: $Commit"}
$head=(& git -C $root rev-parse --verify 'HEAD^{commit}').Trim()
if($head -ne $resolved){throw "Benchmark commit $resolved must be the checked-out HEAD $head; check out the intended commit first."}
$dirty=@(& git -C $root status --porcelain --untracked-files=normal)
if($LASTEXITCODE -or $dirty.Count){throw 'Benchmark requires a clean worktree so both detached runs use the exact checked-out source and submodule pins.'}
$runId=[guid]::NewGuid().ToString('N')
$benchmarkRoot=Join-Path $root "build/benchmark/$runId"
$legacyRoot=Join-Path $benchmarkRoot 'legacy'
$ctestRoot=Join-Path $benchmarkRoot 'ctest'
$artifactsRoot=Join-Path $benchmarkRoot 'artifacts'
New-Item -ItemType Directory -Path $artifactsRoot -Force|Out-Null
$gitAdded=@()
$measurements=[ordered]@{}
function Get-Report([string]$Worktree,[string]$ExpectedLevel) {
    $files=@(Get-ChildItem -LiteralPath (Join-Path $Worktree 'build/reports') -Filter report.json -File -Recurse -ErrorAction SilentlyContinue)
    foreach($file in $files | Sort-Object LastWriteTimeUtc -Descending){
        try{$report=Get-Content -LiteralPath $file.FullName -Raw|ConvertFrom-Json}catch{continue}
        if(($ExpectedLevel -eq 'legacy' -and $report.level -eq 'legacy-verify') -or ($report.level -eq $ExpectedLevel)){return [pscustomobject]@{Path=$file.FullName;Report=$report}}
    }
    throw "No $ExpectedLevel timing report was produced in $Worktree."
}
function Invoke-Run([string]$Name,[string]$Worktree,[ValidateSet('legacy','full')][string]$Flow,[string]$RunKind) {
    $workBuild=Join-Path $Worktree 'build'
    $null=New-Item -ItemType Directory -Path $workBuild -Force
    $log=Join-Path $workBuild "benchmark-$Flow-$RunKind.log"
    $runArtifacts=Join-Path $artifactsRoot $Name
    $artifactWorktree=Join-Path $runArtifacts 'worktree'
    $null=New-Item -ItemType Directory -Path $artifactWorktree -Force
    Push-Location $Worktree
    $watch=[Diagnostics.Stopwatch]::StartNew()
    $exit=1
    try {
        if($Flow -eq 'legacy') { & pwsh -NoLogo -NoProfile -File scripts/verify.ps1 *> $log }
        else { & pwsh -NoLogo -NoProfile -File scripts/test.ps1 -Level FULL -BaseRef $resolved *> $log }
        $exit=$LASTEXITCODE
    } finally { $watch.Stop();Pop-Location }
    $level=if($Flow -eq 'legacy'){'legacy'}else{'FULL'}
    $artifactBuild=Join-Path $artifactWorktree 'build'
    $null=New-Item -ItemType Directory -Path $artifactBuild -Force
    $sourceReports=Join-Path $workBuild 'reports'
    if(Test-Path -LiteralPath $sourceReports){Copy-Item -LiteralPath $sourceReports -Destination (Join-Path $artifactBuild 'reports') -Recurse -Force}
    $artifactLog=Join-Path $artifactBuild (Split-Path $log -Leaf)
    if(Test-Path -LiteralPath $log){Copy-Item -LiteralPath $log -Destination $artifactLog -Force}
    $report=$null
    try { $report=Get-Report $Worktree $level } catch { if(!$exit){throw} }
    if($report){
        $relativeReport=[IO.Path]::GetRelativePath($Worktree,$report.Path)
        $escapedSource=$Worktree.Replace('\','\\')
        $escapedDestination=$artifactWorktree.Replace('\','\\')
        foreach($jsonFile in Get-ChildItem -LiteralPath (Join-Path $artifactWorktree 'build/reports') -Filter '*.json' -File -Recurse -ErrorAction SilentlyContinue){
            $jsonText=[IO.File]::ReadAllText($jsonFile.FullName)
            [IO.File]::WriteAllText($jsonFile.FullName,$jsonText.Replace($escapedSource,$escapedDestination),[Text.UTF8Encoding]::new($false))
        }
        $report=Get-Report $artifactWorktree $level
    }
    if($exit){throw "$Name failed with exit $exit; archived log: $artifactLog"}
    if(!$report){throw "$Name completed without a timing report; archived log: $artifactLog"}
    $measurements[$Name]=[pscustomobject]@{flow=$Flow;cache_state=$RunKind;wall_seconds=[Math]::Round($watch.Elapsed.TotalSeconds,3);report_path=$report.Path;log_path=$artifactLog;report=$report.Report}
}
try {
    $worktrees=[ordered]@{legacy=$legacyRoot;ctest=$ctestRoot}
    foreach($name in $worktrees.Keys){
        $worktree=$worktrees[$name]
        & git -C $root worktree add --detach $worktree $resolved *> (Join-Path $artifactsRoot "$name-worktree.log")
        if($LASTEXITCODE){throw "Could not create $name benchmark worktree."}
        $gitAdded+=,$worktree
        & git -C $worktree submodule update --init --recursive *> (Join-Path $artifactsRoot "$name-submodules.log")
        if($LASTEXITCODE){throw "Could not initialize $name pinned submodule."}
    }
    Invoke-Run 'legacy_cold' $legacyRoot legacy cold
    Invoke-Run 'legacy_warm' $legacyRoot legacy warm
    Invoke-Run 'ctest_cold' $ctestRoot full cold
    Invoke-Run 'ctest_warm' $ctestRoot full warm

    $legacyCold=[double]$measurements.legacy_cold.wall_seconds
    $ctestCold=[double]$measurements.ctest_cold.wall_seconds
    $legacyWarm=[double]$measurements.legacy_warm.wall_seconds
    $ctestWarm=[double]$measurements.ctest_warm.wall_seconds
    $coldSaved=$legacyCold-$ctestCold
    $warmSaved=$legacyWarm-$ctestWarm
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
        upstream_core_test_binaries=(Get-TimingCount -Report $measurements.legacy_cold.report -Category 'core.test')
        english_input_cases=(Get-TimingCount -Report $measurements.legacy_cold.report -Category 'english.keys')
        submission_input_cases=(Get-TimingCount -Report $measurements.legacy_cold.report -Category 'submission.case')
        private_desktop_cases=(Get-TimingCount -Report $measurements.legacy_cold.report -Category 'tsf.private-desktop')
        add_word_cases=(Get-TimingCount -Report $measurements.legacy_cold.report -Category 'word.add')
        tsf_standard_smokes=(Get-TimingCount -Report $measurements.legacy_cold.report -Category 'tsf.smoke')
        tsf_cold_smokes=(Get-TimingCount -Report $measurements.legacy_cold.report -Category 'tsf.cold-start')
    }
    $ctestCaseCounts=[ordered]@{
        upstream_core_test_binaries=(Get-TimingCount -Report $measurements.ctest_cold.report -Category 'core.test')
        english_input_cases=(Get-TimingCount -Report $measurements.ctest_cold.report -Category 'english.keys')
        submission_input_cases=(Get-TimingCount -Report $measurements.ctest_cold.report -Category 'submission.case')
        private_desktop_cases=(Get-TimingCount -Report $measurements.ctest_cold.report -Category 'tsf.private-desktop')
        add_word_cases=(Get-TimingCount -Report $measurements.ctest_cold.report -Category 'word.add')
        tsf_standard_smokes=(Get-TimingCount -Report $measurements.ctest_cold.report -Category 'tsf.smoke')
        tsf_cold_smokes=(Get-TimingCount -Report $measurements.ctest_cold.report -Category 'tsf.cold-start')
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
    $summary=[ordered]@{
        schema=1
        commit=$resolved
        status='measured'
        comparable_scope='Legacy verify.ps1 and CTest FULL x64; both include shared/windows unit tests, all upstream core test binaries, mixed-input regressions, data validation, TSF smoke, add-word, English-key, submission, and each private-desktop case.'
        coverage=[ordered]@{
            legacy_entry='scripts/verify.ps1 (compatibility path; all original assertions retained)'
            ctest_case_count=[int]$ctestRows.Count
            ctest_case_names=@($ctestRows | ForEach-Object name)
            ctest_group_counts=$ctestGroupCounts
            observed_case_counts=[ordered]@{legacy_verify=$legacyCaseCounts;ctest_full=$ctestCaseCounts}
            preserved_suite_list=@('upstream data static and built-output checks','shared and windows Cargo --lib','upstream core library and every integration-test binary, with latency suites serialized','ignored mixed-input and digit regressions','TSF standard and cold activation','add-word popup','all 19 English-key scenarios across their original mode/layout combinations','all 21 submission mode groups plus the input-digit matrix','all 18 private TSF compatibility cases','all 15 popup lifecycle private-desktop cases')
        }
        measurements=[ordered]@{
            legacy_cold=[ordered]@{wall_seconds=$legacyCold;report=$measurements.legacy_cold.report_path;log=$measurements.legacy_cold.log_path}
            legacy_warm=[ordered]@{wall_seconds=$legacyWarm;report=$measurements.legacy_warm.report_path;log=$measurements.legacy_warm.log_path}
            ctest_cold=[ordered]@{wall_seconds=$ctestCold;report=$measurements.ctest_cold.report_path;log=$measurements.ctest_cold.log_path}
            ctest_warm=[ordered]@{wall_seconds=$ctestWarm;report=$measurements.ctest_warm.report_path;log=$measurements.ctest_warm.log_path}
        }
        comparison=[ordered]@{
            cold_saved_seconds=[Math]::Round($coldSaved,3)
            cold_speedup_percent=if($legacyCold){[Math]::Round(100*$coldSaved/$legacyCold,2)}else{$null}
            warm_saved_seconds=[Math]::Round($warmSaved,3)
            warm_speedup_percent=if($legacyWarm){[Math]::Round(100*$warmSaved/$legacyWarm,2)}else{$null}
            ctest_management_only='Not isolated: CTest centralizes scheduling and reporting; it is not counted as a runtime optimization by itself.'
            incremental_and_cache='Compare cold versus warm runs and build.cpp.harness, build.rust.release, build.winui.publish, models.*, and data.* stage events in the referenced reports.'
            parallelism='The existing memory-limited upstream core scheduler remains in both flows. Independent CTest cases use resource locks; effects are included in end-to-end wall time but not isolated by an ablation run.'
            selective_affected='Not part of this equal-scope FULL comparison. Each AFFECTED report records its own selected rows and wall time; compare only with a FULL run for the full-suite baseline.'
            model='Cold and warm mean fresh versus reused project build/test outputs; shared NuGet/Cargo download caches and the machine file cache may already be warm.'
        }
        category_sums=[ordered]@{
            legacy_cold=@($measurements.legacy_cold.report.timings | Group-Object category | ForEach-Object { [pscustomobject]@{category=$_.Name;sum_seconds=[Math]::Round((($_.Group | Measure-Object duration_ms -Sum).Sum/1000),3)} })
            legacy_warm=@($measurements.legacy_warm.report.timings | Group-Object category | ForEach-Object { [pscustomobject]@{category=$_.Name;sum_seconds=[Math]::Round((($_.Group | Measure-Object duration_ms -Sum).Sum/1000),3)} })
            ctest_cold=@($measurements.ctest_cold.report.timings | Group-Object category | ForEach-Object { [pscustomobject]@{category=$_.Name;sum_seconds=[Math]::Round((($_.Group | Measure-Object duration_ms -Sum).Sum/1000),3)} })
            ctest_warm=@($measurements.ctest_warm.report.timings | Group-Object category | ForEach-Object { [pscustomobject]@{category=$_.Name;sum_seconds=[Math]::Round((($_.Group | Measure-Object duration_ms -Sum).Sum/1000),3)} })
        }
        generated_utc=[DateTimeOffset]::UtcNow.ToString('O')
    }
    $comparisonPath=Join-Path $benchmarkRoot 'comparison.json'
    [IO.File]::WriteAllText($comparisonPath,($summary|ConvertTo-Json -Depth 16),[Text.UTF8Encoding]::new($false))
    Write-Host "Benchmark report: $comparisonPath"
    Write-Host ('Cold: {0:F3}s -> {1:F3}s; saved {2:F3}s ({3:F2}%).' -f $legacyCold,$ctestCold,$coldSaved,$summary.comparison.cold_speedup_percent)
    Write-Host ('Warm: {0:F3}s -> {1:F3}s; saved {2:F3}s ({3:F2}%).' -f $legacyWarm,$ctestWarm,$warmSaved,$summary.comparison.warm_speedup_percent)
} finally {
    if(!$KeepWorktrees){foreach($path in $gitAdded){& git -C $root worktree remove --force $path *> $null}}
    if($gitAdded.Count){& git -C $root worktree prune}
}
