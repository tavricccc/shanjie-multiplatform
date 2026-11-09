#Requires -Version 7
param(
    [Parameter(Mandatory)][ValidateSet('FAST','AFFECTED','FULL','RELEASE')][string]$Level,
    [string]$BaseRef='origin/main',
    [ValidateRange(1,32)][int]$Jobs=4,
    [switch]$RetryFailures,
    [switch]$CiSafe
)
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
Import-Module "$PSScriptRoot/test-support.psm1" -ErrorAction Stop
$runId=[guid]::NewGuid().ToString('N')
$reportRoot=Join-Path $root "build/reports/$($Level.ToLowerInvariant())-$((Get-Date).ToString('yyyyMMdd-HHmmss'))-$($runId.Substring(0,8))"
$eventsRoot=Join-Path $reportRoot 'events'
$null=New-Item -ItemType Directory -Path $eventsRoot -Force
$consoleLog=Join-Path $reportRoot 'console.log'
Start-Transcript -Path $consoleLog -Force | Out-Null
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
function Test-CommentOnlyChange([string]$Path) {
    $extension=[IO.Path]::GetExtension($Path).ToLowerInvariant()
    $hashComments=@('.ps1','.psm1','.cmake','.yml','.yaml','.toml','.lock','.ini')
    $slashComments=@('.rs','.cpp','.cc','.c','.h','.hpp','.cs')
    if($extension -notin ($hashComments+$slashComments) -and $Path -ne 'CMakeLists.txt'){return $false}
    $diffLines=@()
    $diffLines+=@(git -C $root diff --unified=0 "$BaseRef...HEAD" -- "$Path" 2>$null)
    $diffLines+=@(git -C $root diff --unified=0 -- "$Path" 2>$null)
    $diffLines+=@(git -C $root diff --cached --unified=0 -- "$Path" 2>$null)
    $changed=@($diffLines | Where-Object { $_ -match '^[+-]' -and $_ -notmatch '^(\+\+\+|---)' })
    if(!$changed.Count){return $false}
    foreach($line in $changed){
        $content=$line.Substring(1).Trim()
        if(!$content){return $false}
        $comment=if($extension -in $slashComments){$content -match '^(//|/\*|\*|\*/)' -or $content -match '^<!--'}else{$content -match '^#' -or $content -match '^;'}
        if(!$comment){return $false}
        if($extension -in @('.ps1','.psm1') -and $content -match '^#requires\b'){return $false}
    }
    return $true
}
function Classify-ChangedPath([string]$Path) {
    if (Test-DocumentationOnly $Path) { $pathReasons[$Path]='Documentation-only change; no build or tests.'; return }
    if ($Level -in @('FAST','AFFECTED') -and (Test-CommentOnlyChange $Path)) { $pathReasons[$Path]='Only full-line comments changed; no build or tests.'; return }
    if ($Path -match '^(shared/|shared\\)') { Add-Label 'shared' "$Path changes shared Rust logic; test shared and downstream Windows crates."; Add-Label 'windows' "$Path is consumed by the Windows crate."; return }
    if ($Path -match '^(windows/src/engine\.rs|windows/src/input\.rs|windows/src/mixed_regression\.rs|windows/src/model_store\.rs|windows/Cargo\.(toml|lock)|shared/Cargo\.(toml|lock)|platform/|upstream/|data/|profiles/|MODEL-|UPSTREAM\.json)') { $script:forceFull=$true; $pathReasons[$Path]='Core, model, profile, dependency, or upstream change has broad regression impact.'; return }
    if ($Path -match '^windows/src/tsf/') { Add-Label 'windows' "$Path changes the Windows TSF crate."; Add-Label 'tsf' "$Path changes TSF behavior; run the isolated TSF regression matrix."; return }
    if ($Path -match '^windows/src/') { Add-Label 'windows' "$Path changes the Windows Rust crate."; return }
    if ($Path -match '^tests/(english_keys\.cpp)$') { Add-Label 'english' "$Path is the English-key harness."; return }
    if ($Path -match '^tests/submission\.cpp$') { Add-Label 'submission' "$Path is the input-submission harness."; return }
    if ($Path -match '^tests/add_word_popup\.cpp$') { Add-Label 'word' "$Path is the add-word popup harness."; return }
    if ($Path -match '^tests/candidate_host_smoke\.cpp$') { Add-Label 'ui' "$Path is the candidate-host UI harness."; return }
    if ($Path -match '^tests/mode_button_smoke\.cpp$|^tests/support/mode_indicator\.(cpp|h)$') { Add-Label 'mode' "$Path is the off-screen mode-indicator harness."; return }
    if ($Path -match '^tests/windows_interaction\.cpp$') { Add-Label 'interaction' "$Path is the isolated Windows interaction harness."; return }
    if ($Path -match '^tests/tsf_smoke\.cpp$') { Add-Label 'smoke' "$Path is the TSF smoke harness."; return }
    if ($Path -match '^tests/tsf_compat\.cpp$') { Add-Label 'private-desktop' "$Path is the private-desktop TSF compatibility harness."; return }
    if ($Path -match '^tests/popup_lifecycle\.cpp$') { Add-Label 'private-desktop' "$Path is the private-desktop popup lifecycle harness."; return }
    if ($Path -match '^tests/') { $script:forceFull=$true; $pathReasons[$Path]='Shared C++ support or test inventory changed; run the full regression matrix.'; return }
    if ($Path -match '^(ui/|scripts/verify-ui\.ps1$)') { Add-Label 'ui' "$Path affects WinUI or its host harness."; return }
    if ($Path -match '^scripts/verify-mode\.ps1$') { Add-Label 'mode' "$Path runs the off-screen mode-indicator smoke test."; return }
    if ($Path -match '^scripts/verify-interaction\.ps1$') { Add-Label 'interaction' "$Path runs the isolated Windows interaction harness."; return }
    if ($Path -match '^scripts/verify-(english-keys|submission|add-word|private-desktop)\.ps1$') {
        if ($Path -match 'english') { Add-Label 'english' "$Path runs English-key regressions." }
        elseif ($Path -match 'submission') { Add-Label 'submission' "$Path runs submission regressions." }
        elseif ($Path -match 'add-word') { Add-Label 'word' "$Path runs add-word regressions." }
        else { Add-Label 'private-desktop' "$Path runs isolated private-desktop regressions." }
        return
    }
    if ($Path -match '^(scripts/(build|prepare|test-core|verify|verify-regressions|verify-upstream-data|ctest-case|test\.ps1|prepare-test-payload)\.ps1|CMakeLists\.txt|CMakePresets\.json|\.github/|Cargo\.toml|Cargo\.lock)') { $script:forceFull=$true; $pathReasons[$Path]='Build, test framework, CI, or dependency logic changed; run FULL.'; return }
    $script:forceFull=$true
    $pathReasons[$Path]='Change impact is not mapped; use conservative FULL coverage.'
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
            $reason=if($skipTests){'Documentation-only change; build and test not needed.'}elseif($Level -eq 'FAST'){'Outside the FAST smoke set.'}elseif($Level -eq 'AFFECTED'){'No selected label matches the changed modules.'}elseif($Level -eq 'FULL'){'Release-only check; outside FULL.'}elseif($Level -eq 'RELEASE' -and $Preset -eq 'windows-x86' -and $test.name -notlike 'release.*'){'x64 FULL suites are already run on the x64 preset; this x86 pass contains only x86 release checks.'}else{'Test is not selected for this architecture preset.'}
            $omitted.Add([pscustomobject]@{name=$test.name;labels=$labels;reason=$reason;preset=$Preset})
        }
    }
    $requiredFixtures=[Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach($test in $selected) { foreach($fixture in (Get-TestFixtureProperty $test 'FIXTURES_REQUIRED')) { if($fixture){[void]$requiredFixtures.Add($fixture)} } }
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
        & cmake --build --preset $Preset --target @targets --config Release
        if ($LASTEXITCODE) { throw "CMake harness build failed for $Preset." }
        Complete-TestStage -Stage $stage -Details @{preset=$Preset;targets=$targets;cache='CMake incremental build'}
    } catch { $status='failed';$errorMessage=$_.Exception.Message;Complete-TestStage -Stage $stage -Status $status -Details @{preset=$Preset;targets=$targets} -ErrorMessage $errorMessage;throw }
}
function Invoke-CtestSelection([string]$Preset,[string]$BinaryDirectory,[object[]]$Tests) {
    if (!$Tests.Count) { return }
    $names=@($Tests | ForEach-Object name)
    $regex='^(' + (($names | ForEach-Object { [regex]::Escape($_) }) -join '|') + ')$'
    $log=Join-Path $reportRoot "ctest-$Preset.log"
    $junit=Join-Path $reportRoot "junit-$Preset.xml"
    $ctestArguments=@('--test-dir',$BinaryDirectory,'-C','Release','--output-on-failure','--output-log',$log,'--output-junit',$junit,'--parallel',"$Jobs",'--no-tests=error','-R',$regex)
    if($RetryFailures){$ctestArguments+=@('--repeat','until-pass:2')}
    $stage=Start-TestStage -Name "CTest execution ($Preset)" -Category 'ctest.run'
    $script:ctestReports += [pscustomobject]@{preset=$Preset;log=$log;junit=$junit;tests=$names}
    $status='passed';$errorMessage=$null
    try { & ctest @ctestArguments; if($LASTEXITCODE){throw "CTest failed for $Preset (exit $LASTEXITCODE); see $log and $junit."} }
    catch { $status='failed';$errorMessage=$_.Exception.Message;throw }
    finally { Complete-TestStage -Stage $stage -Status $status -Details @{preset=$Preset;selected_tests=$names.Count;log=$log;junit=$junit} -ErrorMessage $errorMessage }
}

try {
    $changeSet=Get-ChangedPathList $BaseRef
    $changedPaths=@($changeSet.Paths)
    if ($changeSet.Error) { $forceFull=$true;$notes.Add($changeSet.Error+' Conservative full test selection is active.') }
    foreach($path in $changedPaths){Classify-ChangedPath $path}
    $codeChanges=@($changedPaths | Where-Object { !(Test-DocumentationOnly $_) -and !(Test-CommentOnlyChange $_) })
    if ($changedPaths.Count -and !$codeChanges.Count) { $skipTests=$true }
    if (!$changedPaths.Count -and !$changeSet.Error -and $Level -eq 'AFFECTED') { $skipTests=$true;$notes.Add('No changed paths were found; selected no affected tests.') }
    if ($forceFull -and $Level -in @('FAST','AFFECTED')) { $notes.Add('Impact could not be bounded safely; expanded selection to FULL.') }
    if ($Level -eq 'FAST' -and !$skipTests) {
        $requestedLabels.Clear()
        [void]$requestedLabels.Add($(if($forceFull){'full'}else{'fast'}))
        if(!$pathReasons.Contains('fast')){$pathReasons['fast']='FAST source coverage and Rust unit smoke checks.'}
        if(!$changedPaths.Count -and !$changeSet.Error){$notes.Add('No changed paths were found; running the FAST smoke set.')}
    }
    if ($Level -eq 'AFFECTED' -and !$skipTests) {
        if($forceFull){$requestedLabels.Clear();[void]$requestedLabels.Add('full')}
        else{[void]$requestedLabels.Add('fast')}
        if(!$pathReasons.Contains('fast')){$pathReasons['fast']='FAST source coverage and Rust unit smoke checks run before the mapped regressions.'}
    }
    if ($Level -eq 'FULL') { [void]$requestedLabels.Add('full') }
    if ($Level -eq 'RELEASE') { [void]$requestedLabels.Add('full');[void]$requestedLabels.Add('release') }
    $skipTests = $skipTests -and $Level -in @('FAST','AFFECTED') -and !$forceFull
    $presets=if($skipTests){@()}else{@('windows-x64')}
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
        $selectedByPreset[$preset]=$selection.Selected
        $unselectedByPreset[$preset]=$selection.Omitted
        $needsProduct=$needsProduct -or [bool](@($selection.Selected | Where-Object { ($_.labels | Where-Object { $_ -in @('tsf','english','submission','word','private-desktop','smoke','models','ui','mode','package') }) }).Count)
    }

    if ($Level -eq 'AFFECTED' -and !$forceFull) {
        $needsUiOutput = [bool](@($changedPaths | Where-Object { $_ -match '^(ui/|scripts/verify-ui\.ps1$|tests/candidate_host_smoke\.cpp$)' }).Count) -or
            [bool](@($selectedByPreset.Values | ForEach-Object { $_ } | Where-Object { $_.name -match '^release\.ui\.(candidate|verify)$' }).Count)
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
    Write-Error $failureMessage
} finally {
    $whole.Stop()
    foreach($report in $ctestReports) {
        if(Test-Path -LiteralPath $report.junit) {
            try {
                [xml]$junit=Get-Content -LiteralPath $report.junit -Raw
                foreach($case in $junit.SelectNodes('//testcase')) {
                    $failure=$case.SelectSingleNode('failure')
                    $skipped=$case.SelectSingleNode('skipped')
                    $status=if($failure){'failed'}elseif($skipped){'not-run'}else{'passed'}
                    $testResults+=,[pscustomobject]@{name=$case.name;class=$case.classname;preset=$report.preset;status=$status;duration_seconds=[double]$case.time;failure=if($failure){$failure.InnerText}else{$null};log=$report.log}
                }
            } catch { $notes.Add("Could not parse JUnit report $($report.junit): $($_.Exception.Message)") }
        }
    }
    $observed=[Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach($result in $testResults){[void]$observed.Add("$($result.preset)::$($result.name)")}
    foreach($selected in @($selectedByPreset.Values | ForEach-Object { $_ })) {
        $key="$($selected.preset)::$($selected.name)"
        if(!$observed.Contains($key)){$testResults+=,[pscustomobject]@{name=$selected.name;class=$null;preset=$selected.preset;status='not-run';duration_seconds=$null;failure=$null;reason=if($overallStatus -eq 'failed'){'CTest did not return a JUnit result; see the stage failure and log.'}else{'Selected in the plan but not started.'};log=$null}}
    }
    $timings=@(Get-ChildItem -LiteralPath $eventsRoot -Filter '*.json' -File -ErrorAction SilentlyContinue | ForEach-Object { Get-Content -LiteralPath $_.FullName -Raw | ConvertFrom-Json } | Sort-Object started_utc)
    $allSelected=@($selectedByPreset.Values | ForEach-Object { $_ })
    $allOmitted=@($unselectedByPreset.Values | ForEach-Object { $_ })
    if($skipTests){$allOmitted+=,[pscustomobject]@{name='CTest inventory';labels=@();reason='Inventory was not configured because this change needs no build or tests.';preset=$null}}
    if(!$skipTests){
        foreach($preset in $presets | Where-Object { $_ -notin $inventoryByPreset.Keys }){
            $allOmitted+=,[pscustomobject]@{name='CTest inventory';labels=@();reason='CMake configuration or CTest inventory failed before test rows could be listed.';preset=$preset}
        }
    }
    $report=[ordered]@{
        schema=2
        level=$Level
        status=$overallStatus
        wall_seconds=[Math]::Round($whole.Elapsed.TotalSeconds,3)
        base_ref=$BaseRef
        changed_paths=$changedPaths
        impact_reasons=$pathReasons
        selection=[ordered]@{selected=$allSelected;not_selected=$allOmitted;notes=@($notes);reason=if($skipTests){'Documentation-only or no changes.'}elseif($forceFull){'Conservative FULL expansion.'}else{"$Level change-impact selection."}}
        results=$testResults
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
}
if($overallStatus -ne 'passed'){throw $failureMessage}
