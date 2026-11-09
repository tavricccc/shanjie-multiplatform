#Requires -Version 7
param(
    [Parameter(Mandatory)][ValidateSet('CI','FAST','AFFECTED','FULL','RELEASE')][string]$Level,
    [string]$BaseRef='origin/main',
    [ValidateRange(1,32)][int]$Jobs=4,
    [switch]$RetryFailures,
    [switch]$CiSafe
)
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
Import-Module "$PSScriptRoot/test-support.psm1" -ErrorAction Stop
Import-Module "$PSScriptRoot/test-plan.psm1" -ErrorAction Stop
Import-Module "$PSScriptRoot/release-support.psm1" -ErrorAction Stop
$version=(Get-Content -LiteralPath "$root/RELEASE-CHANNEL.json" -Raw|ConvertFrom-Json).version
$sourceStart=$null;$sourceEnd=$null;$releaseData=$null
$runId=[guid]::NewGuid().ToString('N')
$reportRoot=Join-Path $root "build/reports/$($Level.ToLowerInvariant())-$((Get-Date).ToString('yyyyMMdd-HHmmss'))-$($runId.Substring(0,8))"
$eventsRoot=Join-Path $reportRoot 'events'
$null=New-Item -ItemType Directory -Path $eventsRoot -Force
$consoleLog=Join-Path $reportRoot 'console.log'
Start-Transcript -Path $consoleLog -Force | Out-Null
$previousRunId=$env:SHANJIE_TEST_RUN_ID
$previousTimingsDirectory=$env:SHANJIE_TEST_TIMINGS_DIR
$env:SHANJIE_TEST_RUN_ID=$runId
$env:SHANJIE_TEST_TIMINGS_DIR=$eventsRoot
$whole=[Diagnostics.Stopwatch]::StartNew()
$changedPaths=@()
$pathReasons=[ordered]@{}
$selectedByPreset=[ordered]@{}
$unselectedByPreset=[ordered]@{}
$inventoryByPreset=[ordered]@{}
$presets=@()
$testResults=@()
$ctestReports=@()
$overallStatus='passed'
$failureMessage=$null
$requestedLabels=[Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
$forceFull=$false
$skipTests=$false
$needsProduct=$false
$skipSettingsBuild=$false
$notes=[Collections.Generic.List[string]]::new()

function Add-Label([string]$Label,[string]$Reason) {
    [void]$requestedLabels.Add($Label)
    $pathReasons[$Label]=$Reason
}
function Get-ChangedPathList([string]$Ref) {
    $base=$Ref
    if (!(git -C $root rev-parse --verify --quiet $base 2>$null)) { return [pscustomobject]@{ Paths=@(); Error="Cannot resolve base ref '$base'." } }
    $items=[Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    $diff=@(git -C $root diff --name-only "$base...HEAD" 2>$null)
    if ($LASTEXITCODE) { return [pscustomobject]@{ Paths=@(); Error="Cannot compare HEAD with '$base'." } }
    foreach($path in $diff){if($path){[void]$items.Add($path.Replace('\','/'))}}
    foreach($path in @(git -C $root diff --name-only)){if($path){[void]$items.Add($path.Replace('\','/'))}}
    foreach($path in @(git -C $root diff --cached --name-only)){if($path){[void]$items.Add($path.Replace('\','/'))}}
    foreach($path in @(git -C $root ls-files --others --exclude-standard)){if($path){[void]$items.Add($path.Replace('\','/'))}}
    return [pscustomobject]@{ Paths=@($items | Sort-Object); Error=$null }
}
function Test-DocumentationOnly([string]$Path) {
    return ($Path -match '^(docs/|README(?:\..*)?$|AGENTS\.md$|Changelog\.md$|UPSTREAM-CHANGES\.md$)' -or $Path -match '\.(md|rst|adoc)$')
}
function Classify-ChangedPath([string]$Path) {
    if (Test-DocumentationOnly $Path) { $pathReasons[$Path]='Documentation-only change; no build or tests.'; return }
    $impact=Get-VerificationImpact $Path
    $pathReasons[$Path]=$impact.Reason
    if($impact.Full){$script:forceFull=$true}
    foreach($label in $impact.Labels){Add-Label $label "$Path`: $($impact.Reason)"}
}
function Get-CtestInventory([string]$BinaryDirectory) {
    $raw=& ctest --test-dir $BinaryDirectory -C Release --show-only=json-v1 2>&1
    if ($LASTEXITCODE) { throw "CTest inventory failed for $BinaryDirectory`: $raw" }
    $text=($raw | Out-String).Trim()
    return (ConvertFrom-Json -InputObject $text)
}
function Get-TestLabels($Test) {
    $property=$Test.properties | Where-Object name -eq 'LABELS' | Select-Object -First 1
    if (!$property) { return @() }
    return @($property.value | ForEach-Object { ([string]$_ -split ';') })
}
function Get-TestFixtureProperty($Test,[string]$PropertyName) {
    $property=$Test.properties | Where-Object name -eq $PropertyName | Select-Object -First 1
    if (!$property) { return @() }
    return @($property.value | ForEach-Object { ([string]$_ -split ';') })
}
function Get-SelectedTests($Inventory,[string[]]$Levels,[string]$Preset) {
    $selected=[Collections.Generic.List[object]]::new()
    $omitted=[Collections.Generic.List[object]]::new()
    foreach($test in $Inventory.tests) {
        $labels=Get-TestLabels $test
        $matches=@($labels | Where-Object { $_ -in $Levels })
        if($matches.Count -and $CiSafe -and 'ui' -in $labels -and 'ci-safe' -notin $labels) {
            $omitted.Add([pscustomobject]@{name=$test.name;labels=$labels;reason='Skipped in CI-safe mode: interactive WinUI/candidate-host checks require a local Windows verification session; CI still compiles the WinUI project.';preset=$Preset})
            continue
        }
        if ($matches.Count) {
            $reason=if($Level -in @('FAST','AFFECTED')) { ($matches | ForEach-Object { if($pathReasons.Contains($_)){$pathReasons[$_]}else{"Selected by $Level level."} }) -join ' ' } else { "$Level level selects the $($matches -join ', ') test group." }
            $selected.Add([pscustomobject]@{name=$test.name;labels=$labels;reason=$reason;preset=$Preset})
        } else {
            $reason=if($skipTests){'Documentation-only change; build and test not needed.'}elseif($Level -eq 'CI'){'Outside the fixed CI baseline; full product and desktop acceptance run locally or before release.'}elseif($Level -eq 'FAST'){'Outside the FAST smoke set.'}elseif($Level -eq 'AFFECTED'){'No selected label matches the changed modules.'}elseif($Level -eq 'FULL'){'Release-only check; outside FULL.'}elseif($Level -eq 'RELEASE' -and $Preset -eq 'windows-x86' -and $test.name -notlike 'release.*'){'x64 FULL suites are already run on the x64 preset; this x86 pass contains only x86 release checks.'}else{'Test is not selected for this architecture preset.'}
            $omitted.Add([pscustomobject]@{name=$test.name;labels=$labels;reason=$reason;preset=$Preset})
        }
    }
    $requiredFixtures=[Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach($test in $Inventory.tests | Where-Object name -In $selected.name) { foreach($fixture in (Get-TestFixtureProperty $test 'FIXTURES_REQUIRED')) { if($fixture){[void]$requiredFixtures.Add($fixture)} } }
    if($requiredFixtures.Count) {
        foreach($test in $Inventory.tests) {
            $setup=Get-TestFixtureProperty $test 'FIXTURES_SETUP'
            $missing=@($setup | Where-Object { $_ -in $requiredFixtures -and !($selected.name -contains $test.name) })
            if($missing.Count) {
                for($index=$omitted.Count-1;$index -ge 0;$index--) { if($omitted[$index].name -eq $test.name){$omitted.RemoveAt($index)} }
                $selected.Add([pscustomobject]@{name=$test.name;labels=(Get-TestLabels $test);reason="CTest fixture setup for $($missing -join ', ') is required by selected tests.";preset=$Preset})
            }
        }
    }
    return [pscustomobject]@{Selected=@($selected);Omitted=@($omitted)}
}
function Get-CmakeTargets([object[]]$Tests) {
    $targets=[Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach($test in $Tests) {
        $name=$test.name
        if($name -match '\.tsf-smoke\.') {[void]$targets.Add('shanjie_tsf_smoke')}
        if($name -match '\.word\.add\.') {[void]$targets.Add('shanjie_add_word_popup')}
        if($name -match '\.english\.') {[void]$targets.Add('shanjie_english_keys')}
        if($name -match '\.submission\.') {[void]$targets.Add('shanjie_submission')}
        if($name -match '\.private\.tsf-compat\.') {[void]$targets.Add('shanjie_tsf_compat')}
        if($name -match '\.private\.popup-lifecycle\.') {[void]$targets.Add('shanjie_popup_lifecycle')}
        if($name -match '\.ui\.candidate\.') {[void]$targets.Add('shanjie_candidate_host')}
        if($name -match '\.mode\.') {[void]$targets.Add('shanjie_mode_smoke')}
        if($name -match '\.interaction\.') {[void]$targets.Add('shanjie_windows_interaction')}
    }
    return @($targets)
}
function Invoke-CmakeBuild([string]$Preset,[object[]]$Tests) {
    $targets=@(Get-CmakeTargets $Tests)
    if (!$targets.Count) { return }
    $stage=Start-TestStage -Name "Incremental C++ harness build ($Preset)" -Category 'build.cpp.harness'
    $status='passed';$errorMessage=$null
    try {
        & cmake --build --preset $Preset --target @targets --config Release --parallel $Jobs
        if ($LASTEXITCODE) { throw "CMake harness build failed for $Preset." }
        Complete-TestStage -Stage $stage -Details @{preset=$Preset;targets=$targets;cache='CMake incremental build'}
    } catch { $status='failed';$errorMessage=$_.Exception.Message;Complete-TestStage -Stage $stage -Status $status -Details @{preset=$Preset;targets=$targets} -ErrorMessage $errorMessage;throw }
}
function Invoke-CtestSelection([string]$Preset,[string]$BinaryDirectory,[object[]]$Tests) {
    if (!$Tests.Count) { return }
    $names=@($Tests | ForEach-Object name)
    $regex='^(' + (($names | ForEach-Object { [regex]::Escape($_) }) -join '|') + ')$'
    $stage=Start-TestStage -Name "CTest execution ($Preset)" -Category 'ctest.run'
    $status='passed';$errorMessage=$null
    $previousAttempt=$env:SHANJIE_CTEST_ATTEMPT
    $previousParent=$env:SHANJIE_TEST_PARENT_STAGE_ID
    $env:SHANJIE_TEST_PARENT_STAGE_ID=$stage.Id
    try {
        for($attempt=1;$attempt -le $(if($RetryFailures){2}else{1});$attempt++) {
            $suffix=if($attempt -eq 1){''}else{'-retry'}
            $log=Join-Path $reportRoot "ctest-$Preset$suffix.log"
            $junit=Join-Path $reportRoot "junit-$Preset$suffix.xml"
            $ctestArguments=@('--test-dir',$BinaryDirectory,'-C','Release','--output-on-failure','--test-output-size-passed','0','--test-output-size-failed','0','--output-log',$log,'--output-junit',$junit,'--parallel',"$Jobs",'--no-tests=error')
            if($attempt -eq 1){$ctestArguments+=@('-R',$regex)}else{$ctestArguments+='--rerun-failed'}
            $script:ctestReports += [pscustomobject]@{preset=$Preset;attempt=$attempt;log=$log;junit=$junit;tests=$names}
            $env:SHANJIE_CTEST_ATTEMPT="$attempt"
            & ctest @ctestArguments
            $ctestExit=$LASTEXITCODE
            $lastTest=Join-Path $BinaryDirectory 'Testing/Temporary/LastTest.log'
            if(Test-Path -LiteralPath $lastTest){Copy-Item -LiteralPath $lastTest -Destination (Join-Path $reportRoot "LastTest-$Preset$suffix.log") -Force}
            if(!$ctestExit){break}
        }
        if($ctestExit){throw "CTest failed for $Preset (exit $ctestExit); see $log and $junit."}
    } catch { $status='failed';$errorMessage=$_.Exception.Message;throw }
    finally {
        $env:SHANJIE_CTEST_ATTEMPT=$previousAttempt
        $env:SHANJIE_TEST_PARENT_STAGE_ID=$previousParent
        Complete-TestStage -Stage $stage -Status $status -Details @{preset=$Preset;selected_tests=$names.Count;log=$log;junit=$junit} -ErrorMessage $errorMessage
    }
}

try {
    if($Level -eq 'RELEASE'){$sourceStart=Get-ReleaseIdentity $root}
    $changeSet=if($Level -eq 'CI'){[pscustomobject]@{Paths=@();Error=$null}}else{Get-ChangedPathList $BaseRef}
    $changedPaths=@($changeSet.Paths)
    if ($changeSet.Error) { $forceFull=$true;$notes.Add($changeSet.Error) }
    foreach($path in $changedPaths){Classify-ChangedPath $path}
    $codeChanges=@($changedPaths | Where-Object { !(Test-DocumentationOnly $_) })
    if ($Level -in @('FAST','AFFECTED') -and $changedPaths.Count -and !$codeChanges.Count) { $skipTests=$true }
    if (!$changedPaths.Count -and !$changeSet.Error -and $Level -eq 'AFFECTED') { $skipTests=$true;$notes.Add('No changed paths were found; selected no affected tests.') }
    if ($forceFull -and $Level -eq 'AFFECTED') { $notes.Add('Impact could not be bounded safely; expanded selection to FULL.') }
    if ($Level -eq 'FAST' -and !$skipTests) {
        $requestedLabels.Clear()
        [void]$requestedLabels.Add('fast')
        if(!$pathReasons.Contains('fast')){$pathReasons['fast']='FAST source coverage and Rust unit smoke checks.'}
        if(!$changedPaths.Count -and !$changeSet.Error){$notes.Add('No changed paths were found; running the FAST smoke set.')}
    }
    if ($Level -eq 'AFFECTED' -and !$skipTests) {
        if($forceFull){$requestedLabels.Clear();[void]$requestedLabels.Add('full')}
        else{[void]$requestedLabels.Add('fast')}
        if(!$pathReasons.Contains('fast')){$pathReasons['fast']='FAST source coverage and Rust unit smoke checks run before the mapped regressions.'}
    }
    if ($Level -eq 'FULL') { [void]$requestedLabels.Add('full') }
    if ($Level -eq 'CI') { [void]$requestedLabels.Add('ci');$notes.Add('Fixed CI baseline: ignores change impact and never expands to FULL or builds the product.') }
    if ($Level -eq 'RELEASE') { [void]$requestedLabels.Add('full');[void]$requestedLabels.Add('release') }
    $skipTests = $skipTests -and $Level -in @('FAST','AFFECTED') -and !$forceFull
    $presets=@(if(!$skipTests){'windows-x64'})
    if($Level -eq 'RELEASE'){$presets+=@('windows-x86')}
    foreach($preset in $presets) {
        Invoke-Timed -Name "CMake configure ($preset)" -Category 'build.cmake.configure' -Details @{preset=$preset} -Action {
            & cmake --preset $preset
            if($LASTEXITCODE){throw "CMake configure failed for $preset."}
        }
    }

    foreach($preset in $presets) {
        $binaryDirectory=Join-Path $root "build/cmake/$preset"
        $inventory=Get-CtestInventory $binaryDirectory
        $inventoryByPreset[$preset]=$inventory
        $labels=@($requestedLabels)
        if($Level -eq 'RELEASE' -and $preset -eq 'windows-x86'){$labels=@('release')}
        if($skipTests){$labels=@()}
        $selection=Get-SelectedTests $inventory $labels $preset
        if($Level -eq 'CI'){Assert-FixedCiSelection $selection.Selected}
        $selectedByPreset[$preset]=$selection.Selected
        $unselectedByPreset[$preset]=$selection.Omitted
        $needsProduct=$Level -ne 'CI' -and ($needsProduct -or [bool](@($selection.Selected | Where-Object { ($_.labels | Where-Object { $_ -in @('tsf','english','submission','word','private-desktop','smoke','models','ui','mode','package') }) }).Count))
    }

    if ($Level -eq 'AFFECTED' -and !$forceFull) {
        $needsUiOutput = [bool](@($changedPaths | Where-Object { $_ -match '^(ui/|scripts/verify-ui\.ps1$|tests/candidate_host_smoke\.cpp$)' }).Count) -or
            [bool](@($selectedByPreset.Values | ForEach-Object { $_ } | Where-Object { $_.name -match '^release\.ui\.(candidate\.|verify$)' }).Count)
        $skipSettingsBuild = !$needsUiOutput
        if ($skipSettingsBuild) { $notes.Add('AFFECTED product prerequisites omit WinUI publish because no changed path or selected interactive UI case needs it; FULL and RELEASE still include WinUI publish.') }
    }

    if($Level -eq 'FULL' -or $Level -eq 'RELEASE' -or ($Level -eq 'AFFECTED' -and $forceFull)) {
        $architecture=if($Level -eq 'RELEASE'){'all'}else{'x64'}
        Invoke-Timed -Name "Build $architecture release product" -Category 'build.product' -Details @{architecture=$architecture;incremental='Cargo, WinUI, model and data receipts'} -Action {
            & "$PSScriptRoot/build.ps1" -Architecture $architecture
        }
        $needsProduct=$true
    } elseif($needsProduct) {
        Invoke-Timed -Name 'Build x64 product prerequisites' -Category 'build.product' -Details @{architecture='x64';incremental='Cargo, WinUI, model and data receipts'} -Action {
            if ($skipSettingsBuild) { & "$PSScriptRoot/build.ps1" -Architecture x64 -SkipSettings }
            else { & "$PSScriptRoot/build.ps1" -Architecture x64 }
        }
    }

    if(!$skipTests) {
        foreach($preset in $presets) {
            $selected=@($selectedByPreset[$preset])
            if(!$selected.Count){continue}
            Invoke-CmakeBuild $preset $selected
            Invoke-CtestSelection $preset (Join-Path $root "build/cmake/$preset") $selected
        }
    }
} catch {
    $overallStatus='failed'
    $failureMessage=$_.Exception.Message
    Write-Host "FAILED: $failureMessage"
} finally {
    $whole.Stop()
    try { $testResults=@(Read-CtestResults $ctestReports) }
    catch { $overallStatus='failed';$failureMessage=$_.Exception.Message;$notes.Add("Could not read CTest JUnit: $failureMessage") }
    $observed=[Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach($result in $testResults){[void]$observed.Add("$($result.preset)::$($result.name)")}
    foreach($selected in @($selectedByPreset.Values | ForEach-Object { $_ })) {
        $key="$($selected.preset)::$($selected.name)"
        if(!$observed.Contains($key)){$testResults+=,[pscustomobject]@{name=$selected.name;class=$null;preset=$selected.preset;status='not-run';duration_seconds=$null;failure=$null;reason=if($overallStatus -eq 'failed'){'CTest did not return a JUnit result; see the stage failure and log.'}else{'Selected in the plan but not started.'};log=$null}}
    }
    if($overallStatus -eq 'passed' -and @($testResults | Where-Object status -NE 'passed').Count){
        $overallStatus='failed'
        $failureMessage='Some selected tests failed or did not run; see the per-test results and CTest logs.'
    }
    $timingEvents=@(Get-ChildItem -LiteralPath $eventsRoot -Filter '*.json' -File -ErrorAction SilentlyContinue | ForEach-Object { Get-Content -LiteralPath $_.FullName -Raw | ConvertFrom-Json })
    $timings=@(Get-ExclusiveTestTimings $timingEvents | Sort-Object started_utc)
    $caseRows=@(
        foreach($row in $selectedByPreset.Values | ForEach-Object { $_ }){[pscustomobject]@{name=$row.name;preset=$row.preset;reason=$row.reason;selected=$true}}
        foreach($row in $unselectedByPreset.Values | ForEach-Object { $_ }){[pscustomobject]@{name=$row.name;preset=$row.preset;reason=$row.reason;selected=$false}}
    )
    $subcases=@(Get-HarnessCasePlan $caseRows)
    foreach($subcase in $subcases | Where-Object selected) {
        $events=@($timings | Where-Object { $_.test_name -eq $subcase.parent -and $_.category -eq $subcase.category })
        foreach($key in $subcase.parameters.Keys){$events=@($events | Where-Object { $_.details.$key -ceq $subcase.parameters[$key] })}
        $subcase.attempts=@($events | Sort-Object attempt,started_utc | ForEach-Object { [pscustomobject]@{attempt=$_.attempt;status=$_.status;duration_ms=$_.duration_ms;error=$_.error} })
        if($subcase.attempts.Count){
            $subcase.status=$subcase.attempts[-1].status
            $subcase.flaky=$subcase.status -eq 'passed' -and [bool](@($subcase.attempts | Where-Object status -eq 'failed').Count)
        }else{$subcase.reason='Selected combination did not reach execution; inspect its parent CTest result and logs.'}
    }
    if($overallStatus -eq 'passed' -and @($subcases | Where-Object { $_.selected -and $_.status -ne 'passed' }).Count){
        $overallStatus='failed';$failureMessage='Some selected harness combinations did not pass or did not run; see subcases.'
    }
    $allSelected=@($selectedByPreset.Values | ForEach-Object { $_ })
    $allOmitted=@($unselectedByPreset.Values | ForEach-Object { $_ })
    if($skipTests){$allOmitted+=,[pscustomobject]@{name='CTest inventory';labels=@();reason='Inventory was not configured because this change needs no build or tests.';preset=$null}}
    if(!$skipTests){
        foreach($preset in $presets | Where-Object { $_ -notin $inventoryByPreset.Keys }){
            $allOmitted+=,[pscustomobject]@{name='CTest inventory';labels=@();reason='CMake configuration or CTest inventory failed before test rows could be listed.';preset=$preset}
        }
    }
    if($Level -eq 'RELEASE'){
        try {
            $sourceEnd=Get-ReleaseIdentity $root
            $releaseData=[ordered]@{source_start=$sourceStart;source_end=$sourceEnd;architectures=@('x64','x86');candidate_id=$runId;artifacts=$null}
            if($overallStatus -eq 'passed'){$releaseData.artifacts=Get-ReleaseArtifacts $root $runId $ctestReports}
        }catch{$overallStatus='failed';$failureMessage=$_.Exception.Message}
    }
    $report=[ordered]@{
        schema=3
        version=$version
        commit=if($sourceEnd){$sourceEnd.commit}else{(& git -C $root rev-parse HEAD)}
        worktree_dirty=if($sourceEnd){!$sourceEnd.clean}else{[bool](@(& git -C $root status --porcelain --untracked-files=normal).Count)}
        level=$Level
        status=$overallStatus
        wall_seconds=[Math]::Round($whole.Elapsed.TotalSeconds,3)
        base_ref=$BaseRef
        changed_paths=$changedPaths
        impact_reasons=$pathReasons
        selection=[ordered]@{selected=$allSelected;not_selected=$allOmitted;notes=@($notes);reason=if($Level -eq 'CI'){'Fixed CI baseline; no impact expansion.'}elseif($skipTests){'Documentation-only or no changes.'}elseif($forceFull -and $Level -eq 'AFFECTED'){'Conservative FULL expansion.'}else{"$Level change-impact selection."}}
        results=$testResults
        subcases=$subcases
        flaky_tests=@($testResults | Where-Object flaky | ForEach-Object name)
        release=$releaseData
        timings=$timings
        ctest=$ctestReports
        retry_failures=[bool]$RetryFailures
        ci_safe=[bool]$CiSafe
        cache=[ordered]@{cmake='incremental targets keyed by source and architecture';cargo='Cargo fingerprint cache';models='content hashes and pinned input receipts';ui='source/toolchain key plus output hashes';worktree_cache='reused when valid'}
        comparison=[ordered]@{legacy_seconds=$null;full_seconds=$null;saved_seconds=$null;speedup_percent=$null;status='not-measured';benchmark_command='pwsh -File scripts/benchmark-verification.ps1'}
        junit=$ctestReports
        console_log=$consoleLog
        failure=$failureMessage
        generated_utc=[DateTimeOffset]::UtcNow.ToString('O')
    }
    $json=Join-Path $reportRoot 'report.json'
    [IO.File]::WriteAllText($json,($report|ConvertTo-Json -Depth 18),[Text.UTF8Encoding]::new($false))
    Write-Host "Test report: $json"
    Write-Host "Selected CTest rows: $($allSelected.Count); not selected: $($allOmitted.Count); elapsed: $($whole.Elapsed.TotalSeconds.ToString('F3'))s"
    Stop-Transcript | Out-Null
    $env:SHANJIE_TEST_RUN_ID=$previousRunId
    $env:SHANJIE_TEST_TIMINGS_DIR=$previousTimingsDirectory
}
if($overallStatus -ne 'passed'){throw $failureMessage}
if($Level -eq 'RELEASE' -and $CiSafe -and $env:GITHUB_ACTIONS -eq 'true'){
    & "$PSScriptRoot/package.ps1" -ExportCiArtifact -ReportPath $json
}
