#Requires -Version 7
param(
    [Parameter(Mandatory)][ValidateSet('CI','FAST','AFFECTED','FULL','RELEASE')][string]$Level,
    [string]$BaseRef='origin/main',
    [ValidateSet('Branch','Commit','Worktree')][string]$ChangeScope='Branch',
    [ValidateRange(1,32)][int]$Jobs=4,
    [switch]$RetryFailures,
    [switch]$CiSafe,
    [switch]$PlanOnly,
    [ValidateSet('x64','x86')][string]$Architecture='x64'
)
$ErrorActionPreference='Stop'
if($Architecture -eq 'x86' -and $Level -notin 'AFFECTED','RELEASE'){throw '-Architecture x86 is supported by AFFECTED/RELEASE; CI, FAST and FULL use the x64 baseline.'}
$root=Split-Path $PSScriptRoot -Parent
Import-Module "$PSScriptRoot/test-support.psm1" -ErrorAction Stop
Import-Module "$PSScriptRoot/test-plan.psm1" -ErrorAction Stop
Import-Module "$PSScriptRoot/release-support.psm1" -ErrorAction Stop
$version=(Get-Content -LiteralPath "$root/RELEASE-CHANNEL.json" -Raw|ConvertFrom-Json).version
$sourceStart=$null;$sourceEnd=$null;$releaseData=$null;$snapshot=$null;$packagingPermit=$null
$requestedLevel=$Level
$changeSet=if($Level -eq 'CI'){[pscustomobject]@{Paths=@();Error=$null;Range=$null;EffectiveBase=$null}}else{Get-VerificationChanges $root $BaseRef $ChangeScope}
# Only genuinely unbounded x86 impact needs dual-architecture release acceptance.
# Determine the effective level before naming the report/receipt directory.
if($Level -eq 'AFFECTED' -and $Architecture -eq 'x86' -and ($changeSet.Error -or (Select-VerificationPlan $changeSet.Paths).Full)){$Level='RELEASE'}
$runId=[guid]::NewGuid().ToString('N')
$reportRoot=Join-Path $root "build/reports/$($Level.ToLowerInvariant())-$((Get-Date).ToString('yyyyMMdd-HHmmss'))-$($runId.Substring(0,8))"
$eventsRoot=Join-Path $reportRoot 'events'
$null=New-Item -ItemType Directory -Path $eventsRoot -Force
$consoleLog=Join-Path $reportRoot 'console.log'
Start-Transcript -Path $consoleLog -Force | Out-Null
$previousRunId=$env:SHANJIE_TEST_RUN_ID
$previousTimingsDirectory=$env:SHANJIE_TEST_TIMINGS_DIR
$previousLevel=$env:SHANJIE_TEST_LEVEL
$previousSnapshotId=$env:SHANJIE_RELEASE_SNAPSHOT_ID
$previousSnapshotHash=$env:SHANJIE_RELEASE_SNAPSHOT_SHA256
$previousPermit=$env:SHANJIE_RELEASE_PACKAGING_PERMIT
# A nested test invocation must not inherit another run's sealed inputs/permit.
$env:SHANJIE_RELEASE_SNAPSHOT_ID=$null
$env:SHANJIE_RELEASE_SNAPSHOT_SHA256=$null
$env:SHANJIE_RELEASE_PACKAGING_PERMIT=$null
$env:SHANJIE_TEST_LEVEL=$Level
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
$additionalVerification=@()

function Add-Label([string]$Label,[string]$Reason) {
    [void]$requestedLabels.Add($Label)
    $pathReasons[$Label]=$Reason
}
function Get-ChangedPathList([string]$Ref,[string]$Scope='Branch') {
    Get-VerificationChanges $root $Ref $Scope
}
function Test-DocumentationOnly([string]$Path) {
    return (Test-VerificationDocumentation $Path)
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
function Invoke-CtestSelection([string]$Preset,[string]$BinaryDirectory,[object[]]$Tests,[string]$ReportTag='') {
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
            $suffix=$(if($ReportTag){"-$ReportTag"})+$(if($attempt -gt 1){'-retry'})
            $log=Join-Path $reportRoot "ctest-$Preset$suffix.log"
            $junit=Join-Path $reportRoot "junit-$Preset$suffix.xml"
            $ctestArguments=@('--test-dir',$BinaryDirectory,'-C','Release','--output-on-failure','--test-output-size-passed','0','--test-output-size-failed','0','--output-log',$log,'--output-junit',$junit,'--parallel',"$Jobs",'--no-tests=error')
            if($attempt -eq 1){$ctestArguments+=@('-R',$regex)}else{$ctestArguments+='--rerun-failed'}
            $script:ctestReports += [pscustomobject]@{preset=$Preset;attempt=$attempt;log=$log;junit=$junit;tests=$names;snapshot_id=$env:SHANJIE_RELEASE_SNAPSHOT_ID;snapshot_sha256=$env:SHANJIE_RELEASE_SNAPSHOT_SHA256}
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
    if($Level -eq 'RELEASE'){
        $sourceStart=Get-ReleaseIdentity $root
        if(!$PlanOnly -and !$sourceStart.clean){throw 'Commit the intended source first; RELEASE requires a clean commit before building its snapshot.'}
    }
    if($Level -ne $requestedLevel){$notes.Add('Unbounded x86 impact: expanded AFFECTED to dual-architecture RELEASE; packaging still requires both architectures to pass.')}
    $changedPaths=@($changeSet.Paths)
    if ($changeSet.Error) { $forceFull=$true;$notes.Add($changeSet.Error) }
    if($Level -ne 'CI'){$notes.Add("Change scope=$ChangeScope; committed range=$($changeSet.Range); staged, unstaged and untracked paths are included. Narrow scopes do not validate older branch commits.")}
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
        if($forceFull){[void]$requestedLabels.Add('full')}
        else{[void]$requestedLabels.Add('fast')}
        if(!$pathReasons.Contains('fast')){$pathReasons['fast']='FAST source coverage and Rust unit smoke checks run before the mapped regressions.'}
    }
    if ($Level -eq 'FULL') { $requestedLabels.Clear();[void]$requestedLabels.Add('full') }
    if ($Level -eq 'CI') { $requestedLabels.Clear();[void]$requestedLabels.Add('ci');$notes.Add('Fixed CI baseline: ignores change impact and never expands to FULL or builds the product.') }
    if ($Level -eq 'RELEASE') { $requestedLabels.Clear();[void]$requestedLabels.Add('full');[void]$requestedLabels.Add('release') }
    $skipTests = $skipTests -and $Level -in @('FAST','AFFECTED') -and !$forceFull
    if($Level -ne 'CI'){
        $additionalVerification=@(Get-AdditionalVerificationPlan -Paths $changedPaths -Labels @($requestedLabels) -CiSafe:$CiSafe)
        foreach($item in $additionalVerification){
            Write-Host "[$($item.kind)] $($item.id): $($item.reason)"
            if($item.command){Write-Host "  $($item.command)"}
        }
        if($additionalVerification.Count){Write-Host 'Advisories only: planned CTest rows and extra checks are not PASS results. Credit actual matching JUnit/JSON evidence; do not run legacy wrappers again for those cases.'}
    }
    $presetLabels=[ordered]@{}
    foreach($preset in @('windows-x64')+@($(if($Level -eq 'RELEASE' -or ($Level -eq 'AFFECTED' -and $Architecture -eq 'x86')){'windows-x86'}))|Where-Object{$_}){
        $presetLabels[$preset]=@(if(!$skipTests){Get-VerificationPresetLabels $Level $Architecture $preset @($requestedLabels)})
    }
    if($PlanOnly){
        $overallStatus='planned'
        $notes.Add('Plan only: CMake inventory, builds and tests were not executed.')
        Write-Host "Requested CTest labels: $(@($requestedLabels) -join ', '); skip=$skipTests"
        Write-Host "Change scope: $ChangeScope; committed range: $($changeSet.Range); effective base: $($changeSet.EffectiveBase)"
        foreach($path in $changedPaths){Write-Host "$path : $($pathReasons[$path])"}
        return
    }
    $presets=@(if(!$skipTests){'windows-x64'})
    if($Level -eq 'RELEASE'){$presets+=@('windows-x86')}
    if(!$skipTests -and $Level -eq 'AFFECTED' -and $Architecture -eq 'x86'){$presets+=@('windows-x86')}
    if($Level -eq 'RELEASE'){
        # Build both architectures before freezing any input or linking harnesses.
        Invoke-Timed -Name 'Build both release architectures' -Category 'build.product' -Details @{architecture='all'} -Action {
            & "$PSScriptRoot/build.ps1" -Architecture all
            & "$PSScriptRoot/prepare-licenses.ps1"
        }
        $snapshot=Invoke-Timed -Name 'Seal dual-architecture release snapshot' -Category 'release.snapshot' -Action {New-ReleaseSnapshot $root $runId $sourceStart}
        $env:SHANJIE_RELEASE_SNAPSHOT_ID=$snapshot.id
        $env:SHANJIE_RELEASE_SNAPSHOT_SHA256=$snapshot.sha256
        $needsProduct=$true
    }
    foreach($preset in $presets) {
        Invoke-Timed -Name "CMake configure ($preset)" -Category 'build.cmake.configure' -Details @{preset=$preset} -Action {
            $nativeRoot=if($snapshot){Join-Path $snapshot.root 'native'}else{''}
            & cmake --preset $preset "-DSHANJIE_RELEASE_NATIVE_ROOT=$nativeRoot"
            if($LASTEXITCODE){throw "CMake configure failed for $preset."}
        }
    }

    foreach($preset in $presets) {
        $binaryDirectory=Join-Path $root "build/cmake/$preset"
        $inventory=Get-CtestInventory $binaryDirectory
        $inventoryByPreset[$preset]=$inventory
        $labels=@(Get-VerificationPresetLabels $Level $Architecture $preset @($requestedLabels))
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

    if($Level -eq 'RELEASE'){
        $notes.Add('Both product architectures were built and sealed before CMake configuration. All release tests and packaging use that snapshot.')
    } elseif($Level -eq 'FULL' -or ($Level -eq 'AFFECTED' -and $forceFull)) {
        $buildArchitecture='x64'
        Invoke-Timed -Name "Build $buildArchitecture release product" -Category 'build.product' -Details @{architecture=$buildArchitecture;incremental='Cargo, WinUI, model and data receipts'} -Action {
            & "$PSScriptRoot/build.ps1" -Architecture $buildArchitecture
        }
        $needsProduct=$true
    } elseif($needsProduct) {
        $buildArchitecture=if($Architecture -eq 'x86' -and $Level -eq 'AFFECTED'){'all'}else{'x64'}
        Invoke-Timed -Name "Build $buildArchitecture product prerequisites" -Category 'build.product' -Details @{architecture=$buildArchitecture;incremental='Cargo, WinUI, model and data receipts'} -Action {
            if ($skipSettingsBuild) { & "$PSScriptRoot/build.ps1" -Architecture $buildArchitecture -SkipSettings }
            else { & "$PSScriptRoot/build.ps1" -Architecture $buildArchitecture }
        }
    }

    if(!$skipTests) {
        foreach($preset in $presets) {
            $selected=@($selectedByPreset[$preset]|Where-Object name -NE 'release.package')
            if(!$selected.Count){continue}
            if($snapshot){$null=Get-ReleaseSnapshot $root $snapshot.id $snapshot.sha256 -VerifyFiles}
            Invoke-CmakeBuild $preset $selected
            Invoke-CtestSelection $preset (Join-Path $root "build/cmake/$preset") $selected
            if($snapshot){
                $null=Get-ReleaseSnapshot $root $snapshot.id $snapshot.sha256 -VerifyFiles
                Assert-ReleaseRuntimeFixture $snapshot (Join-Path $root "build/cmake/$preset/test-payload-$($snapshot.id)") $preset.Substring(8)
            }
        }
    }
    if($snapshot){
        $prepackRows=@($selectedByPreset.Values|ForEach-Object{$_}|Where-Object name -NE 'release.package')
        $prepackCases=@($prepackRows|ForEach-Object{[pscustomobject]@{name=$_.name;preset=$_.preset;selected=$true;reason=$_.reason}})
        $prepackTimings=@(Get-ChildItem -LiteralPath $eventsRoot -Filter '*.json'|ForEach-Object{Get-Content -LiteralPath $_.FullName -Raw|ConvertFrom-Json})
        $permitRecord=[ordered]@{schema=4;level='RELEASE';status='tests-passed';ci_safe=[bool]$CiSafe;ctest=$ctestReports;results=@(Read-CtestResults $ctestReports);selection=@{selected=$prepackRows};subcases=@(Read-HarnessCaseResults $prepackCases $prepackTimings);release=@{architectures=@('x64','x86');candidate_id=$runId;snapshot_sha256=$snapshot.sha256}}
        $packagingPermit=New-ReleasePackagingPermit $root (Join-Path $reportRoot 'packaging-permit.json') $permitRecord
        $env:SHANJIE_RELEASE_PACKAGING_PERMIT=$packagingPermit.path
        $packageRows=@($selectedByPreset['windows-x64']|Where-Object name -EQ 'release.package')
        if($packageRows.Count -ne 1){throw 'RELEASE inventory must contain exactly one deferred package test.'}
        Invoke-CtestSelection 'windows-x64' (Join-Path $root 'build/cmake/windows-x64') $packageRows -ReportTag 'package'
        $null=Get-ReleaseSnapshot $root $snapshot.id $snapshot.sha256 -VerifyFiles
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
    foreach($hit in $timings|Where-Object{ $_.category -eq 'core.result-cache' -and $_.details.cache -eq 'hit' }){
        $notes.Add("Core assertions reused a verified passing result instead of executing again: $($hit.details.receipt)")
        foreach($result in $testResults|Where-Object name -eq $hit.test_name){$result|Add-Member -NotePropertyName core_result_reused -NotePropertyValue $true}
    }
    $caseRows=@(
        foreach($row in $selectedByPreset.Values | ForEach-Object { $_ }){[pscustomobject]@{name=$row.name;preset=$row.preset;reason=$row.reason;selected=$true}}
        foreach($row in $unselectedByPreset.Values | ForEach-Object { $_ }){[pscustomobject]@{name=$row.name;preset=$row.preset;reason=$row.reason;selected=$false}}
    )
    $subcases=@(Read-HarnessCaseResults $caseRows $timings)
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
            if($overallStatus -eq 'passed' -and (!$sourceStart.clean -or !$sourceEnd.clean -or $sourceStart.commit -cne $sourceEnd.commit -or $sourceStart.version -cne $sourceEnd.version)){throw 'Release source changed during acceptance.'}
            $releaseData=[ordered]@{source_start=$sourceStart;source_end=$sourceEnd;architectures=@('x64','x86');candidate_id=$runId;snapshot_sha256=$snapshot.sha256;packaging_permit=$packagingPermit.path;artifacts=$null}
            if($overallStatus -eq 'passed'){
                $null=Get-ReleaseSnapshot $root $snapshot.id $snapshot.sha256 -VerifyFiles
                Assert-ReleasePackageCopy $snapshot (Join-Path $root "build/release-candidates/$runId/package")
                $releaseData.artifacts=Get-ReleaseArtifacts $root $runId $ctestReports $packagingPermit.path
            }
        }catch{$overallStatus='failed';$failureMessage=$_.Exception.Message}
    }
    $report=[ordered]@{
        schema=if($Level -eq 'RELEASE'){4}else{3}
        version=$version
        commit=if($sourceEnd){$sourceEnd.commit}else{(& git -C $root rev-parse HEAD)}
        worktree_dirty=if($sourceEnd){!$sourceEnd.clean}else{[bool](@(& git -C $root status --porcelain --untracked-files=normal).Count)}
        level=$Level
        requested_level=$requestedLevel
        architecture=$Architecture
        plan_only=[bool]$PlanOnly
        status=$overallStatus
        wall_seconds=[Math]::Round($whole.Elapsed.TotalSeconds,3)
        base_ref=$BaseRef
        change_scope=$ChangeScope
        change_range=$changeSet.Range
        changed_paths=$changedPaths
        impact_reasons=$pathReasons
        additional_verification=$additionalVerification
        verification_summary=(Get-VerificationCompletion $Level $overallStatus $additionalVerification $skipTests)
        selection=[ordered]@{selected=$allSelected;not_selected=$allOmitted;requested_labels=@($requestedLabels);preset_labels=$presetLabels;notes=@($notes);reason=if($Level -eq 'CI'){'Fixed CI baseline; no impact expansion.'}elseif($skipTests){'Documentation-only or no changes.'}elseif($forceFull -and $Level -eq 'AFFECTED'){'Conservative FULL expansion with additional mapped regressions retained.'}else{"$Level change-impact selection."}}
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
    Write-Host "Verification scope: $($report.verification_summary.status); follow-up required=$($report.verification_summary.follow_up_required)"
    Write-Host "Selected CTest rows: $($allSelected.Count); not selected: $($allOmitted.Count); elapsed: $($whole.Elapsed.TotalSeconds.ToString('F3'))s"
    Stop-Transcript | Out-Null
    $env:SHANJIE_TEST_RUN_ID=$previousRunId
    $env:SHANJIE_TEST_TIMINGS_DIR=$previousTimingsDirectory
    $env:SHANJIE_TEST_LEVEL=$previousLevel
    $env:SHANJIE_RELEASE_SNAPSHOT_ID=$previousSnapshotId
    $env:SHANJIE_RELEASE_SNAPSHOT_SHA256=$previousSnapshotHash
    $env:SHANJIE_RELEASE_PACKAGING_PERMIT=$previousPermit
}
if($overallStatus -ne 'passed'){throw $failureMessage}
if($Level -eq 'RELEASE' -and $CiSafe -and $env:GITHUB_ACTIONS -eq 'true'){
    & "$PSScriptRoot/package.ps1" -ExportCiArtifact -ReportPath $json
}
