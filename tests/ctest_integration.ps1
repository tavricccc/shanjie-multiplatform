#Requires -Version 7
# Self-check Windows verification plumbing with file fixtures; no product processes or SDK tools.
param([string]$ResultRoot)
$ErrorActionPreference='Stop'
$project=Split-Path $PSScriptRoot -Parent
Import-Module "$project/scripts/test-plan.psm1" -ErrorAction Stop
Import-Module "$project/scripts/test-support.psm1" -ErrorAction Stop
. "$project/scripts/preserve-output.ps1"
if(!$ResultRoot){$ResultRoot=Join-Path $project ('build/ctest-integration-check-'+[guid]::NewGuid().ToString('N'))}
if(Test-Path -LiteralPath $ResultRoot){throw 'Use a fresh CTest integration fixture directory.'}
$null=New-Item -ItemType Directory -Path $ResultRoot -Force
$checks=0
function Check([bool]$Condition,[string]$Name){$script:checks++;if(!$Condition){throw "FAIL $Name"}}

$cmake=Get-Content "$project/CMakeLists.txt" -Raw
# Account for every verification entry using actual CMake case IDs and dispatch
# calls. A new standalone script cannot silently sit outside the maintained plan.
# This checks registration/policy, not semantic coverage of arbitrary branches.
function Get-CtestDelegatedScripts([string]$CmakeText,$DispatcherAst) {
    $ids=@([regex]::Matches($CmakeText,'shanjie_case\(\s*(?:"[^"]+"|[^\s)]+)\s+([a-z-]+)(?:\s|\))')|ForEach-Object{$_.Groups[1].Value}|Sort-Object -Unique)
    $switch=$DispatcherAst.Find({param($n)$n -is [Management.Automation.Language.SwitchStatementAst] -and $n.Condition.Extent.Text -eq '$CaseId'},$true)
    if(!$switch){throw 'CTest dispatcher case switch is missing.'}
    @(foreach($clause in $switch.Clauses){
        $id=$clause.Item1.Extent.Text.Trim("'"+'"')
        if($id -notin $ids){continue}
        foreach($command in $clause.Item2.FindAll({param($n)$n -is [Management.Automation.Language.CommandAst]},$true)){
            if($command.CommandElements[0].Extent.Text -match '[/\\]([^/\\"'']+\.ps1)["'']$'){$Matches[1]}
        }
    })|Sort-Object -Unique
}
function Assert-TestEntryAccounting([string[]]$Entries,[string[]]$Delegates,[object[]]$Exceptions) {
    foreach($rule in $Exceptions){
        if($rule.script -notin $Entries -or !$rule.scope -or $rule.category -notin 'dispatcher','wrapper','helper','conditional','required-extra'){throw "Unexplained or stale verification exception: $($rule.script)"}
    }
    foreach($entry in $Entries){
        $exception=@($Exceptions|Where-Object script -eq $entry)
        if($exception.Count -gt 1 -or ($entry -notin $Delegates -and !$exception.Count)){throw "Verification entry has no registered CTest delegate or maintained exception: $entry"}
        if($entry -in $Delegates -and $exception.Count){throw "CTest delegate must not be hidden behind an exception: $entry"}
    }
}
$dispatchTokens=$null;$dispatchErrors=$null
$dispatcher=[Management.Automation.Language.Parser]::ParseFile("$project/scripts/ctest-case.ps1",[ref]$dispatchTokens,[ref]$dispatchErrors)
if($dispatchErrors){throw 'CTest dispatcher could not be parsed.'}
$entries=@(Get-ChildItem "$project/scripts" -File|Where-Object Name -Match '^(verify[^/]*|test[^/]*|measure[^/]*)\.ps1$'|ForEach-Object Name)
$delegates=@(Get-CtestDelegatedScripts $cmake $dispatcher)
$exceptions=@(Get-VerificationExceptions)
Assert-TestEntryAccounting $entries $delegates $exceptions
Check ($entries.Count -gt 0 -and $delegates.Count -gt 0) 'Every verification entry has an actual CTest route or explained standalone policy'
foreach($mutation in 'new-entry','removed-case','stale-exception'){
    $rejected=$false
    try{
        switch($mutation){
            'new-entry' {Assert-TestEntryAccounting ($entries+@('verify-unmapped.ps1')) $delegates $exceptions}
            'removed-case' {Assert-TestEntryAccounting $entries @(Get-CtestDelegatedScripts ($cmake.Replace(' english-keys ',' removed-english-keys ')) $dispatcher) $exceptions}
            'stale-exception' {Assert-TestEntryAccounting $entries $delegates ($exceptions+@(@{script='verify-removed.ps1';category='conditional';scope='stale fixture'}))}
        }
    }catch{$rejected=$true}
    Check $rejected "Inventory guard detects $mutation"
}

$english=Get-EnglishKeyCases
$scenarios=[regex]::Match($cmake,'(?m)^set\(english_scenarios ([^)]+)\)').Groups[1].Value -split '\s+'
$caseScenarios=@($english|ForEach-Object{$_[1]}|Sort-Object -Unique)
Check (@(Compare-Object $scenarios $caseScenarios).Count -eq 0) 'Every English scenario has a CTest row'
Check (@($english|Where-Object{$_[1] -eq 'digit-navigation'}).Count -eq 2) 'Both mixing modes retain digit-navigation coverage'
$rows=@(
    foreach($phase in 'clean','reopen','different-directory','glass-fallback'){[pscustomobject]@{name="release.ui.startup.$phase";preset='windows-x64';selected=$true;reason='fixture'}}
    foreach($keyboard in 1,3,2){[pscustomobject]@{name="release.ui.disabled.keyboard$keyboard";preset='windows-x64';selected=$true;reason='fixture'}}
)
$plan=@(Get-HarnessCasePlan $rows)
Check ($plan.Count -eq 10) 'Startup and initial/reopen UI checks produce ten subcases'
Check (@($plan|Where-Object status -ne 'not-run').Count -eq 0) 'Planning UI coverage never claims execution'
Check (@($plan.name|Sort-Object -Unique).Count -eq 10) 'UI subcase identities are unique'
foreach($case in $plan){Check ($case.parameters.architecture -eq 'x64') 'WinUI subcases keep their x64 host identity'}

# Exercise the actual level branches, so broad changes keep mapped UI checks in
# AFFECTED while the FULL/RELEASE inventories remain fixed across branch diffs.
$tokens=$null;$errors=$null
$driverAst=[System.Management.Automation.Language.Parser]::ParseFile("$project/scripts/test.ps1",[ref]$tokens,[ref]$errors)
foreach($Level in 'AFFECTED','FULL','RELEASE'){
    $requestedLabels=[Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    [void]$requestedLabels.Add('ui')
    $forceFull=$true;$skipTests=$false;$pathReasons=[ordered]@{};$notes=[Collections.Generic.List[string]]::new()
    $condition="`$Level -eq '$Level'";if($Level -eq 'AFFECTED'){$condition+=' -and !$skipTests'}
    $branch=$driverAst.Find({param($node) $node -is [System.Management.Automation.Language.IfStatementAst] -and $node.Clauses[0].Item1.Extent.Text -eq $condition -and $node.Extent.Text.Contains('$requestedLabels')},$false)
    . ([scriptblock]::Create($branch.Extent.Text))
    if($Level -eq 'AFFECTED'){Check ($requestedLabels.Contains('full') -and $requestedLabels.Contains('ui')) 'FULL expansion retains mapped UI cases'}
    elseif($Level -eq 'FULL'){Check ($requestedLabels.Count -eq 1 -and $requestedLabels.Contains('full')) 'FULL has a fixed inventory'}
    else{Check ($requestedLabels.Count -eq 2 -and $requestedLabels.Contains('full') -and $requestedLabels.Contains('release')) 'RELEASE has a fixed inventory'}
}

# Tool bytes are copied to a private executable path, and identical outputs keep their timestamps.
$source=Join-Path $ResultRoot 'tools-source'
$destination=Join-Path $ResultRoot 'tools-copy'
$null=New-Item -ItemType Directory -Path $source -Force
[IO.File]::WriteAllText("$source/shanjie-core.exe",'core fixture')
[IO.File]::WriteAllText("$source/old-tool.exe",'retired fixture')
Copy-TestTools $source $destination
Check (!((Get-Item $destination).Attributes -band [IO.FileAttributes]::ReparsePoint)) 'Fixture tools are real copies'
Check ((Get-FileHash "$source/shanjie-core.exe").Hash -eq (Get-FileHash "$destination/shanjie-core.exe").Hash) 'Private tool copy preserves bytes'
$stamp=[datetime]::SpecifyKind([datetime]'2001-01-01',[DateTimeKind]::Utc)
[IO.File]::SetLastWriteTimeUtc("$destination/shanjie-core.exe",$stamp)
Copy-TestTools $source $destination
Check ((Get-Item "$destination/shanjie-core.exe").LastWriteTimeUtc -eq $stamp) 'Identical private tools are not recopied'
[IO.File]::WriteAllText("$source/shanjie-core.exe",'changed core fixture')
Move-Item "$source/old-tool.exe" "$ResultRoot/retired-tool.exe"
Copy-TestTools $source $destination
Check ((Get-Content "$destination/shanjie-core.exe" -Raw) -eq 'changed core fixture') 'Changed tools invalidate the copy'
Check (!(Test-Path "$destination/old-tool.exe")) 'Retired tools leave the active fixture directory'

# A successful core result must invalidate on every input category, including the retained logs.
$resources=Join-Path $ResultRoot 'resources';$models=Join-Path $ResultRoot 'models'
$null=New-Item -ItemType Directory -Path "$resources/data","$resources/eval",$models -Force
[IO.File]::WriteAllText("$resources/data/input.txt",'data')
[IO.File]::WriteAllText("$resources/eval/input.txt",'eval')
[IO.File]::WriteAllText("$models/model.bin",'model')
$binary=Join-Path $ResultRoot 'test.exe';[IO.File]::WriteAllText($binary,'test executable')
$binaries=@([pscustomobject]@{Name='fixture';Path=$binary})
$arguments=@('--skip','unix-fixture')
$first=Get-CoreTestRunKey $binaries $resources $models $arguments 80
Check ($first -ceq (Get-CoreTestRunKey $binaries $resources $models $arguments 80)) 'Identical core inputs reuse one key'
foreach($file in @($binary,"$resources/data/input.txt","$resources/eval/input.txt","$models/model.bin")){
    $text=[IO.File]::ReadAllText($file)
    [IO.File]::AppendAllText($file,'changed')
    Check ($first -cne (Get-CoreTestRunKey $binaries $resources $models $arguments 80)) "Changed core input invalidates the key: $file"
    [IO.File]::WriteAllText($file,$text)
}
Check ($first -cne (Get-CoreTestRunKey $binaries $resources $models @('--skip','another-fixture') 80)) 'Changed arguments invalidate the core key'
Check ($first -cne (Get-CoreTestRunKey $binaries $resources $models $arguments 80 -SourceKey 'changed core source or toolchain')) 'Changed core source or toolchain invalidates the core key'
$logs=Join-Path $ResultRoot 'target/test-logs';$null=New-Item -ItemType Directory -Path $logs -Force
[IO.File]::WriteAllText("$logs/fixture.txt",'test fixture ... ok')
$receipt=Join-Path $ResultRoot 'passed.json'
Save-ContentCache -Root $ResultRoot -ManifestPath $receipt -Key $first -Paths @('target/test-logs')
Check (Test-ContentCache -Root $ResultRoot -ManifestPath $receipt -Key $first -Paths @('target/test-logs')) 'Core result reuse retains verified output evidence'
[IO.File]::WriteAllText("$logs/fixture.txt",'changed evidence')
Check (!(Test-ContentCache -Root $ResultRoot -ManifestPath $receipt -Key $first -Paths @('target/test-logs'))) 'Changed output evidence rejects core reuse'

$tokens=$null;$errors=$null
$ast=[System.Management.Automation.Language.Parser]::ParseFile("$project/scripts/test-core.ps1",[ref]$tokens,[ref]$errors)
foreach($fn in 'Sync-ChangedFiles','Get-CoreBaselineCosts','Read-CoreCostHistory','Get-OrderedCoreBinaries','Update-CoreCostHistory','Invoke-TestBinaries'){
    $node=$ast.Find({param($n) $n -is [System.Management.Automation.Language.FunctionDefinitionAst] -and $n.Name -eq $fn},$false)
    . ([scriptblock]::Create($node.Extent.Text))
}
$sourceTree=Join-Path $ResultRoot 'source';$output=Join-Path $ResultRoot 'tree'
$null=New-Item -ItemType Directory -Path $sourceTree,"$output/target" -Force
[IO.File]::WriteAllText("$sourceTree/core.rs",'source fixture')
[IO.File]::WriteAllText("$output/target/keep",'incremental output')
Check ((Sync-ChangedFiles $sourceTree $output) -eq 1) 'Changed-file sync writes the first source'
[IO.File]::SetLastWriteTimeUtc("$output/core.rs",$stamp)
Check ((Sync-ChangedFiles $sourceTree $output) -eq 0) 'Changed-file sync does not rewrite identical source'
Check ((Get-Item "$output/core.rs").LastWriteTimeUtc -eq $stamp) 'Unchanged source timestamps are preserved'
Check ((Get-Content "$output/target/keep" -Raw) -eq 'incremental output') 'Cargo target outputs survive source sync'

$baseline=Get-CoreBaselineCosts
$sampleLatency=@('engine_newword','engine_replay','engine_lm' | ForEach-Object {[pscustomobject]@{Name=$_}})
$orderedLatency=@(Get-OrderedCoreBinaries $sampleLatency @{} $baseline | ForEach-Object Name)
Check (($orderedLatency -join ',') -eq 'engine_lm,engine_replay,engine_newword') 'Cold-start latency suite prioritizes engine_lm then engine_replay before engine_newword'
$sampleParallel=@('short_case','engine_tone3','engine_auto_english','engine_cap','engine_share' | ForEach-Object {[pscustomobject]@{Name=$_}})
$orderedParallel=@(Get-OrderedCoreBinaries $sampleParallel @{} $baseline | ForEach-Object Name)
Check (($orderedParallel -join ',') -eq 'engine_cap,engine_share,engine_auto_english,engine_tone3,short_case') 'Cold-start parallel core ordering prioritizes heavy suites including engine_auto_english and engine_tone3'
$costFile=Join-Path $ResultRoot 'core-cost-data.json'
Update-CoreCostHistory $costFile @(
    [pscustomobject]@{Name='short_case';DurationSeconds=180.0;Failed=$false},
    [pscustomobject]@{Name='engine_tone3';DurationSeconds=12.0;Failed=$true},
    [pscustomobject]@{Name='engine_cap';DurationSeconds=90.0;Failed=$false}
)
$loadedHistory=Read-CoreCostHistory $costFile
$orderedDynamic=@(Get-OrderedCoreBinaries $sampleParallel $loadedHistory $baseline | ForEach-Object Name)
Check ($orderedDynamic[0] -eq 'engine_tone3' -and $orderedDynamic[1] -eq 'short_case' -and $orderedDynamic[2] -eq 'engine_share') 'Dynamic core cost history prioritizes previously failed suites and observed long durations'
Update-CoreCostHistory $costFile @([pscustomobject]@{Name='short_case';DurationSeconds=80.0;Failed=$false})
$smoothed=Read-CoreCostHistory $costFile
Check ([Math]::Abs([double]$smoothed['short_case']['avg_seconds'] - 150.0) -lt 0.01 -and $smoothed['short_case']['runs'] -eq 2) 'Core cost history updates smoothed moving averages across runs'
$corruptCost=Join-Path $ResultRoot 'corrupt-cost.json'
[IO.File]::WriteAllText($corruptCost,'{not-json')
Check ((Read-CoreCostHistory $corruptCost).Count -eq 0) 'Corrupted core cost history falls back safely to cold-start baselines'
$coreStopDir=Join-Path $ResultRoot 'core-stop-on-failure'
$null=New-Item -ItemType Directory -Path $coreStopDir -Force
$unstartedMarker=Join-Path $coreStopDir 'unstarted.ran'
$failCmd=Join-Path $coreStopDir 'fail_first.cmd'
$unstartedCmd=Join-Path $coreStopDir 'skip_second.cmd'
[IO.File]::WriteAllText($failCmd,"@echo off`r`necho simulated core failure`r`nexit /b 1`r`n")
[IO.File]::WriteAllText($unstartedCmd,"@echo off`r`necho ran > `"$unstartedMarker`"`r`nexit /b 0`r`n")
$stopMetrics=[Collections.Generic.List[object]]::new()
$stopThrew=$false
try {
    Invoke-TestBinaries @([pscustomobject]@{Name='fail_first';Path=$failCmd},[pscustomobject]@{Name='skip_second';Path=$unstartedCmd}) $coreStopDir @() 95 1 $stopMetrics -StopOnFailure
} catch { $stopThrew=$_.Exception.Message -like '*stopped early with 1 unstarted*' }
Check ($stopThrew -and !(Test-Path -LiteralPath $unstartedMarker) -and $stopMetrics.Count -eq 1 -and $stopMetrics[0].Failed) 'Core test runner stops unstarted binaries after first failure while recording failure metrics'
"PASS CTest integration: $checks checks; no Windows acceptance executed"

# Use the configured inventory when available (including the lightweight CI
# configure). This checks actual labels/fixtures without executing any test.
$inventoryDirectory=Join-Path $project 'build/cmake/windows-x64'
if((Test-Path "$inventoryDirectory/CTestTestfile.cmake") -and (Get-Command ctest -ErrorAction SilentlyContinue)){
    $raw=& ctest --test-dir $inventoryDirectory -C Release --show-only=json-v1
    if($LASTEXITCODE){throw 'Could not enumerate the configured CTest fixture'}
    $inventory=($raw|Out-String)|ConvertFrom-Json
    $tokens=$null;$errors=$null
    $driver=[Management.Automation.Language.Parser]::ParseFile("$project/scripts/test.ps1",[ref]$tokens,[ref]$errors)
    foreach($name in 'Get-TestLabels','Get-TestFixtureProperty','Get-SelectedTests'){
        $function=$driver.Find({param($n)$n -is [Management.Automation.Language.FunctionDefinitionAst] -and $n.Name -eq $name},$false)
        . ([scriptblock]::Create($function.Extent.Text))
    }
    $Level='AFFECTED';$CiSafe=$false;$skipTests=$false;$pathReasons=[ordered]@{}
    $scenarios=@(
        @('ui','ui/View.xaml.cs',16),@('add-word','windows/src/word_helper.rs',22),
        @('candidate','windows/src/candidate.rs',42),@('english','tests/english_keys.cpp',27),
        @('submission','tests/submission.cpp',32),@('platform','platform/input_windows.rs',57),
        @('shared','shared/src/lib.rs',111),@('shared-input','shared/src/input.rs',113),
        @('docs','README.md',0),@('unknown','tools/unknown.txt',111),@('ci-workflow','.github/workflows/ci.yml',5)
    )
    foreach($scenario in $scenarios){
        $policy=Select-VerificationPlan @($scenario[1])
        $selection=Get-SelectedTests $inventory @($policy.Labels) 'windows-x64'
        Check ($selection.Selected.Count -eq $scenario[2]) "Configured $($scenario[0]) selection includes exactly its required cases/fixtures"
        Check (@($selection.Selected.name|Sort-Object -Unique).Count -eq $selection.Selected.Count) "Configured $($scenario[0]) cases are selected once"
        Write-Host "PLAN $($scenario[0]): $($selection.Selected.Count) rows (fixtures included), FULL=$($policy.Full)"
    }
    # Stable behavioral requirements, checked against real selection and fixtures.
    # In particular, profile/template changes must not lose the Shanjie or WinUI
    # branches merely because the fixed FULL label contains neither of them.
    foreach($path in 'shared/src/input.rs','windows/src/tsf/keys.rs','shared/src/profile.rs','profiles/windows.ini','profiles/shanjie.ini'){
        $policy=Select-VerificationPlan @($path)
        $selected=Get-SelectedTests $inventory @($policy.Labels) 'windows-x64'
        $required=@('fast.rust.shared','fast.rust.windows','full.tsf-smoke.standard.x64','release.tsf-smoke.shanjie.x64','full.english.numpad.x64','full.english.fullwidth.x64','full.english.backquote.x64')
        if($path -match 'profile'){$required+=@('release.ui.verify','release.ui.models','release.ui.candidate.x64','release.ui.learning-health')}
        foreach($name in $required){Check ($name -in $selected.Selected.name) "$path retains required $name"}
    }
    $fixed=Get-SelectedTests $inventory @('ci') 'windows-x64'
    Assert-FixedCiSelection $fixed.Selected
    $full=Get-SelectedTests $inventory @('full') 'windows-x64'
    Check ($full.Selected.Count -eq 111 -and $inventory.tests.Count -eq 126 -and 'full.submission.async-keys.x64' -in $full.Selected.name -and 'full.submission.rebuild-local.x64' -in $full.Selected.name -and 'full.submission.rebuild-pipe.x64' -in $full.Selected.name -and 'full.submission.segment.x64' -in $full.Selected.name) 'Keep the FULL inventory and cover deferred edit locks, both rebuild modes and composition styling'
    foreach($name in 'full.core.rowstats','full.core.learning-status.x64','full.core.privacy.x64'){
        Check ($name -in $full.Selected.name) "FULL retains historical-gap regression $name"
    }
    foreach($scenario in 'modifiers-empty','modifiers-composing','modifiers-candidates','modifiers-english','range-refresh','privacy-normal','privacy-private','privacy-password','privacy-pin','privacy-native-password','privacy-disabled','privacy-transition'){
        Check ("full.private.tsf-compat.$scenario.x64" -in $full.Selected.name) "FULL retains native boundary $scenario"
    }
    $uiPolicy=Select-VerificationPlan @('ui/MainWindow.xaml.cs')
    $uiSelection=Get-SelectedTests $inventory @($uiPolicy.Labels) 'windows-x64'
    Check ('release.ui.learning-health' -in $uiSelection.Selected.name) 'UI selection exercises actual resident failure and recovery in WinUI'
    $costByName=@{}
    foreach($item in $inventory.tests){
        $costProp=$item.properties | Where-Object name -eq 'COST' | Select-Object -First 1
        if($costProp){$costByName[$item.name]=[double]$costProp.value}
    }
    Check ($costByName['full.core-suite'] -gt $costByName['fast.rust.windows'] -and $costByName['fast.rust.windows'] -gt $costByName['release.ui.verify'] -and $costByName['release.ui.verify'] -gt $costByName['full.submission.async-keys.x64'] -and $costByName['full.submission.async-keys.x64'] -gt $costByName['full.private.tsf-compat.shift-right.x64']) 'Configured CTest inventory preserves descending cold-start COST priorities for heavy suites'
    Write-Host 'PASS configured selection inventory: 11 count scenarios plus key/profile and historical-gap requirements, FULL 111, x64 inventory 126, fixed CI 5; no native execution'
}else{Write-Host 'Not run: configured CTest selection inventory (no configured x64 preset); pure policy checks remain available.'}
