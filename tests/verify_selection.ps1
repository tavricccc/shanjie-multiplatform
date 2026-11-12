#Requires -Version 7
# Preserve the upstream selection scenarios against the shared CTest impact policy.
$ErrorActionPreference='Stop'
. "$PSScriptRoot/../scripts/verify-selection.ps1"
$failures=0
$selectionCases=0
function Expect([string]$Name,[string[]]$Paths,[bool]$Full,[string[]]$Labels=@(),[bool]$Skip=$false){
    $script:selectionCases++
    $plan=Select-VerificationPlan $Paths
    $got="full=$($plan.Full) labels=$((@($plan.Labels)|Sort-Object)-join ',') skip=$($plan.Skip)"
    $want="full=$Full labels=$((@($Labels)|Sort-Object)-join ',') skip=$Skip"
    if($got -ne $want){Write-Host "FAIL $Name`n  want $want`n  got  $got";$script:failures++}else{Write-Host "ok   $Name"}
}
Expect 'nothing changed' @() $false @() $true
Expect 'docs only' @('docs/DESIGN.md','Changelog.md','README.md') $false @() $true
Expect 'windows-style separators' @('docs\ARCHITECTURE.md') $false @() $true
Expect 'release script' @('scripts/release.py') $false @('fast','test-framework')
Expect 'release workflow' @('.github/workflows/release.yml') $false @('fast','test-framework')
Expect 'ui' @('ui/Controls/Switch.cs') $false @('fast','settings-ui','candidate-host')
Expect 'add-word' @('windows/src/word_helper.rs') $false @('fast','windows','word','popup-lifecycle')
Expect 'candidate' @('windows/src/candidate.rs') $false @('fast','windows','candidate-host','tsf-compat')
Expect 'installer' @('windows/src/installer/progress.rs') $false @('fast','windows')
Expect 'platform' @('platform/smart_input_windows.rs') $false @('fast','rust','core','english','submission')
Expect 'suites merge without duplicates' @('windows/src/word_popup.rs','tests/popup_lifecycle.cpp','tests/english_keys.cpp') $false @('fast','windows','word','popup-lifecycle','rust','english')
Expect 'unlisted Windows service' @('windows/src/tsf/service.rs') $true @('full','windows','tsf','ui','mode')
Expect 'shared appearance' @('shared/src/appearance.rs') $true @('full','windows','tsf','ui','mode')
Expect 'profile templates require both interactions and UI' @('profiles/windows.ini','profiles/shanjie.ini') $true @('full','shared','windows','tsf','settings-ui','candidate-host')
Expect 'upstream pin' @('UPSTREAM.json') $true @('full')
Expect 'build script' @('scripts/build.ps1') $true @('full')
Expect 'verify script' @('scripts/verify.ps1') $true @('full')
Expect 'unknown new file' @('tools/something-new.txt') $true @('full')
Expect 'full expansion retains mapped UI and TSF' @('shared/src/lib.rs','ui/App.xaml.cs','platform/composition_windows.rs','windows/src/word_helper.rs') $true @('full','candidate-host','settings-ui','rust','core','english','submission','windows','word','popup-lifecycle')
Expect 'renamed file counts both names' @('docs/old.md','windows/src/tsf/service.rs') $true @('full','windows','tsf','ui','mode')
Expect 'CI workflow is a lightweight framework change' @('.github/workflows/ci.yml') $false @('fast','test-framework')
Expect 'English preserves mixed regressions' @('tests/english_keys.cpp') $false @('fast','rust','english')
Expect 'submission stays bounded' @('tests/submission.cpp') $false @('fast','submission')
Expect 'shared input retains upstream fallback' @('shared/src/input.rs') $true @('full','shared','windows','tsf','interaction')
Expect 'prepare-core preserves platform precision' @('scripts/prepare-core.ps1') $false @('fast','rust','core','english','submission')
if($failures){throw "$failures selection case(s) failed."}
Write-Host "PASS verification selection: $selectionCases cases, one CTest policy"

$checks=0
function Check-Rule([bool]$Condition,[string]$Name){$script:checks++;if(!$Condition){throw "FAIL $Name"};Write-Host "ok   $Name"}
$latency=@(Get-AdditionalVerificationPlan -Paths @('platform/input_windows.rs','windows/src/tsf/keys.rs','windows/src/broker/client.rs') -Labels @('full'))
Check-Rule (@($latency|Where-Object id -eq 'engine-latency').Count -eq 1) 'Key/core changes require one independent latency measurement'
Check-Rule (($latency|Where-Object id -eq 'engine-latency').kind -eq 'required-extra') 'Functional FULL does not substitute for key latency'
$ui=@(Get-AdditionalVerificationPlan -Paths @('ui/View.xaml.cs') -Labels @('fast','ui'))
Check-Rule (($ui|Where-Object id -eq 'ui-acceptance').kind -eq 'ctest-planned' -and !($ui|Where-Object id -eq 'ui-acceptance').command) 'Selected UI rows do not request a duplicate legacy UI command'
Check-Rule (@($ui|Where-Object status -ne 'not-run').Count -eq 0) 'Planning selected coverage never claims a passing result'
$missing=@(Get-AdditionalVerificationPlan -Paths @('ui/View.xaml.cs') -Labels @('full'))
Check-Rule (($missing|Where-Object id -eq 'ui-acceptance').kind -eq 'ctest-needed' -and ($missing|Where-Object id -eq 'ui-acceptance').command -like 'ctest *') 'FULL without UI requests missing CTest rows, not the legacy whole suite'
$safe=@(Get-AdditionalVerificationPlan -Paths @('ui/View.xaml.cs') -Labels @('release') -CiSafe)
Check-Rule (($safe|Where-Object id -eq 'ui-acceptance').kind -eq 'ctest-needed') 'CI-safe cannot claim the skipped local UI environment'
$dictionary=@(Get-AdditionalVerificationPlan -Paths @('windows/src/dictionary_store.rs') -Labels @('full'))
Check-Rule (@($dictionary|Where-Object kind -eq 'review').Count -eq 1 -and ($dictionary|Where-Object kind -eq 'review').id -eq 'dictionary-store') 'Dictionary ABI changes do not request all dedicated tools'
$profile=@(Get-AdditionalVerificationPlan -Paths @('shared/src/profile.rs') -Labels @('fast','ui','tsf'))
Check-Rule (@($profile|Where-Object kind -eq 'review').Count -eq 2 -and @($profile|Where-Object id -eq 'profile-coexistence').Count -eq 1) 'Profile review preserves resident refresh and old-package prerequisites'
$input=@(Get-AdditionalVerificationPlan -Paths @('windows/src/input.rs') -Labels @('full'))
Check-Rule (($input|Where-Object id -eq 'input-features').kind -eq 'review' -and !($input|Where-Object id -eq 'input-features').command -and ($input|Where-Object id -eq 'input-features').reason -like '*-CaseIndices*') 'Partial input overlap calls for a configuration difference review, not a whole-script rerun'
$recovery=@(Get-AdditionalVerificationPlan -Paths @('windows/src/tsf/recovery.rs') -Labels @('full'))
Check-Rule (($recovery|Where-Object id -eq 'core-recovery').reason -like '*-Cases*') 'Recovery review narrows independent fault cases'
Check-Rule (@(Get-AdditionalVerificationPlan -Paths @('windows/src/runtime_log.rs') -Labels @('fast','windows')).Count -eq 0) 'Unrelated diagnostics do not trigger dedicated verification'
$docs=Select-VerificationPlan @('AGENTS.md','docs/TESTING.md')
Check-Rule ($docs.Skip -and !$docs.Labels.Count -and $docs.AdditionalVerification.Count -eq 1 -and $docs.AdditionalVerification[0].id -eq 'markdown-rules' -and !$docs.AdditionalVerification[0].command) 'Rule Markdown changes prompt review without adding tests or interpreting script names'
Check-Rule (@(Get-AdditionalVerificationPlan -Paths @('README.md') -Labels @()).Count -eq 0) 'Ordinary documentation has no supplemental test request'
Check-Rule ((Get-AdditionalVerificationPlan -Paths @('scripts/measure-engine.ps1') -Labels @('full')|Where-Object id -eq 'engine-measurement-tool').kind -eq 'optional') 'Changing the measurement tool alone is an optional sample collection'

$manager=Select-VerificationPlan @('windows/src/bin/shanjie-manager.rs')
Check-Rule (@($manager.AdditionalVerification|Where-Object id -eq 'manager-binary-coverage').Count -eq 1) 'Manager command changes expose the --lib and after-update coverage boundary'
$completion=Get-VerificationCompletion AFFECTED passed $latency $false
Check-Rule ($completion.status -eq 'follow-up-required' -and $completion.follow_up_required -and @($completion.pending|Where-Object id -eq 'engine-latency').Count -eq 1) 'Passing selected tests cannot hide required latency follow-up'
$completion=Get-VerificationCompletion AFFECTED passed $manager.AdditionalVerification $false
Check-Rule ($completion.follow_up_required) 'Passing FULL does not clear the manager binary review'
Check-Rule ((Get-VerificationCompletion CI passed @() $false).status -eq 'ci-baseline-passed') 'CI success states only its fixed baseline scope'
Check-Rule ((Get-VerificationCompletion FAST passed @() $false).status -eq 'fast-baseline-passed') 'FAST success does not claim full acceptance'
Check-Rule ((Get-VerificationCompletion AFFECTED passed @() $true).status -eq 'not-required') 'Documentation remains free of build/test requirements'
Check-Rule ((Get-VerificationCompletion FULL failed @() $false).status -eq 'failed') 'Failed tests cannot become completed verification'

$project=Split-Path $PSScriptRoot -Parent
$tokens=$null;$errors=$null
$ast=[Management.Automation.Language.Parser]::ParseFile("$project/scripts/verify-ui.ps1",[ref]$tokens,[ref]$errors)
$uses=@($ast.FindAll({param($node) $node -is [Management.Automation.Language.VariableExpressionAst] -and $node.VariablePath.UserPath -eq 'Benchmark' -and $node.Parent -isnot [Management.Automation.Language.ParameterAst]},$true))
Check-Rule ($uses.Count -eq 0) 'The legacy UI Benchmark flag adds no independent measurement branch'

$featureAst=[Management.Automation.Language.Parser]::ParseFile("$project/scripts/verify-input-features.ps1",[ref]$tokens,[ref]$errors)
$filter=$featureAst.Find({param($node)$node -is [Management.Automation.Language.IfStatementAst] -and $node.Clauses[0].Item1.Extent.Text -like '*$index -notin $CaseIndices*'},$true)
$skipCase=[scriptblock]::Create($filter.Clauses[0].Item1.Extent.Text)
[int[]]$CaseIndices=@(0);$index=1
Check-Rule ([bool](& $skipCase)) 'Selecting only feature case zero skips other cases instead of rerunning the suite'
$index=0
Check-Rule (!(& $skipCase)) 'The selected zero-based feature case remains selected'
$CaseIndices=$null;$index=1
Check-Rule (!(& $skipCase)) 'Omitting the feature filter preserves the original full-suite behavior'

# Real Git + PlanOnly fixture: no SDK, product build, model, core or UI process.
$fixture=Join-Path $project ('build/conditional-plan-'+[guid]::NewGuid().ToString('N'))
$null=New-Item -ItemType Directory -Path "$fixture/scripts","$fixture/windows/src","$fixture/build" -Force
foreach($name in 'test.ps1','test-plan.psm1','test-support.psm1','release-support.psm1','preserve-output.ps1','verify-changed.ps1'){Copy-Item "$project/scripts/$name" "$fixture/scripts/$name"}
[IO.File]::WriteAllText("$fixture/RELEASE-CHANNEL.json",'{"version":"fixture"}')
[IO.File]::WriteAllText("$fixture/.gitignore",'build/')
& git -C $fixture init -q -b fixture
& git -C $fixture -c user.name=Fixture -c user.email=fixture@example.invalid -c commit.gpgsign=false add .
& git -C $fixture -c user.name=Fixture -c user.email=fixture@example.invalid -c commit.gpgsign=false commit -qm fixture
if($LASTEXITCODE){throw 'Cannot initialize the conditional-plan fixture'}
function Read-Planned([string]$Level,[string]$Scope='Branch',[string]$Ref='HEAD',[string]$Architecture='x64'){
    $log=Join-Path $fixture ("build/$Level-$Scope-"+[guid]::NewGuid().ToString('N')+'.log')
    & (Get-Command pwsh -CommandType Application|Select-Object -First 1).Source -NoProfile -File "$fixture/scripts/test.ps1" -Level $Level -BaseRef $Ref -ChangeScope $Scope -Architecture $Architecture -PlanOnly *> $log
    if($LASTEXITCODE){throw "PlanOnly $Level failed; see $log"}
    $directory=Get-ChildItem "$fixture/build/reports" -Directory -Filter "*-*"|Sort-Object LastWriteTime -Descending|Select-Object -First 1
    Get-Content (Join-Path $directory.FullName 'report.json') -Raw|ConvertFrom-Json
}
[IO.File]::WriteAllText("$fixture/windows/src/input.rs",'// changed input fixture')
$planned=Read-Planned AFFECTED
Check-Rule ($planned.status -eq 'planned' -and !$planned.results.Count -and !$planned.subcases.Count -and !$planned.ctest.Count) 'Real PlanOnly never counts requested cases as executed'
Check-Rule (@($planned.additional_verification|Where-Object kind -eq 'required-extra').Count -eq 1) 'Real PlanOnly prints and persists the required latency reminder'
Check-Rule ($planned.verification_summary.status -eq 'planned' -and $planned.verification_summary.follow_up_required) 'PlanOnly persists pending work without claiming completion'
& git -C $fixture add windows/src/input.rs
& git -C $fixture -c user.name=Fixture -c user.email=fixture@example.invalid -c commit.gpgsign=false commit -qm 'keep input unchanged for documentation scenario'
if($LASTEXITCODE){throw 'Cannot prepare the documentation-only fixture'}
[IO.File]::WriteAllText("$fixture/AGENTS.md",'# New requirement: verify the fictional new-module condition. This prose must not execute a tool.')
$planned=Read-Planned FAST
Check-Rule ($planned.selection.reason -eq 'Documentation-only or no changes.' -and !$planned.selection.requested_labels.Count -and !$planned.results.Count) 'New Markdown prose leaves documentation-only FAST unbuilt and untested'
Check-Rule ($planned.additional_verification.Count -eq 1 -and $planned.additional_verification[0].id -eq 'markdown-rules' -and $planned.additional_verification[0].status -eq 'not-run') 'New Markdown requirements are flagged for review, not claimed as semantically recognized or passed'
$planned=Read-Planned CI
Check-Rule ($planned.changed_paths.Count -eq 0 -and $planned.additional_verification.Count -eq 0 -and $planned.selection.requested_labels.Count -eq 1 -and $planned.selection.requested_labels[0] -eq 'ci') 'CI ignores rule changes and retains only its fixed lightweight label'
& git -C $fixture add AGENTS.md
& git -C $fixture -c user.name=Fixture -c user.email=fixture@example.invalid -c commit.gpgsign=false commit -qm 'rule prose checkpoint'
$branchBase=(& git -C $fixture rev-parse HEAD).Trim()
[IO.File]::WriteAllText("$fixture/CMakeLists.txt",'# accumulated framework change')
& git -C $fixture add CMakeLists.txt
& git -C $fixture -c user.name=Fixture -c user.email=fixture@example.invalid -c commit.gpgsign=false commit -qm 'older framework change'
$null=New-Item -ItemType Directory -Path "$fixture/tests","$fixture/ui","$fixture/docs" -Force
[IO.File]::WriteAllText("$fixture/tests/english_keys.cpp",'// latest bounded English change')
& git -C $fixture add tests/english_keys.cpp
& git -C $fixture -c user.name=Fixture -c user.email=fixture@example.invalid -c commit.gpgsign=false commit -qm 'latest English change'
if($LASTEXITCODE){throw 'Cannot prepare the branch/commit fixture'}
$branchPlan=Read-Planned AFFECTED Branch $branchBase
Check-Rule ('CMakeLists.txt' -in $branchPlan.changed_paths -and 'full' -in $branchPlan.selection.requested_labels) 'Branch scope retains the accumulated framework change and justified FULL fallback'
$commitPlan=Read-Planned AFFECTED Commit
Check-Rule ($commitPlan.changed_paths.Count -eq 1 -and $commitPlan.changed_paths[0] -eq 'tests/english_keys.cpp' -and 'full' -notin $commitPlan.selection.requested_labels -and 'english' -in $commitPlan.selection.requested_labels -and 'rust' -in $commitPlan.selection.requested_labels) 'Commit scope tests the latest English change without dragging in the older CMake commit'
[IO.File]::AppendAllText("$fixture/tests/english_keys.cpp",'// unstaged change')
[IO.File]::WriteAllText("$fixture/ui/View.xaml",'staged UI fixture')
& git -C $fixture add ui/View.xaml
[IO.File]::WriteAllText("$fixture/docs/notes.md",'untracked documentation fixture')
$workPlan=Read-Planned AFFECTED Worktree
Check-Rule ($workPlan.changed_paths.Count -eq 3 -and 'tests/english_keys.cpp' -in $workPlan.changed_paths -and 'ui/View.xaml' -in $workPlan.changed_paths -and 'docs/notes.md' -in $workPlan.changed_paths -and 'full' -notin $workPlan.selection.requested_labels) 'Worktree scope includes staged, unstaged and untracked changes, without older branch commits'
Check-Rule ($workPlan.change_scope -eq 'Worktree' -and !$workPlan.change_range -and $workPlan.status -eq 'planned' -and !$workPlan.results.Count) 'A narrow scope is recorded explicitly and never claims branch acceptance or PASS'
# Use the compatibility entry itself for x86 documentation: no PlanOnly flag,
# no compilers/build script in the fixture, and no CTest invocation is possible.
& git -C $fixture add .
& git -C $fixture -c user.name=Fixture -c user.email=fixture@example.invalid -c commit.gpgsign=false commit -qm 'checkpoint bounded paths'
[IO.File]::WriteAllText("$fixture/README.md",'documentation only')
$docsLog=Join-Path $fixture 'build/x86-docs.log'
& (Get-Command pwsh -CommandType Application|Select-Object -First 1).Source -NoProfile -File "$fixture/scripts/verify-changed.ps1" -Architecture x86 -Base HEAD -ChangeScope Worktree *> $docsLog
Check-Rule ($LASTEXITCODE -eq 0) 'x86 verify-changed documentation really skips without build tools'
$docsReport=Get-ChildItem "$fixture/build/reports" -Directory|Sort-Object LastWriteTime -Descending|Select-Object -First 1|ForEach-Object{Get-Content (Join-Path $_.FullName 'report.json') -Raw|ConvertFrom-Json}
Check-Rule ($docsReport.level -eq 'AFFECTED' -and $docsReport.architecture -eq 'x86' -and !$docsReport.selection.selected.Count -and !$docsReport.results.Count -and !$docsReport.ctest.Count -and !$docsReport.release) 'x86 documentation does not build, test, package, or create a release receipt'
[IO.File]::AppendAllText("$fixture/tests/english_keys.cpp",'// x86 regression')
$x86Plan=Read-Planned AFFECTED Worktree HEAD x86
Check-Rule ($x86Plan.level -eq 'AFFECTED' -and 'english' -in $x86Plan.selection.preset_labels.'windows-x86' -and 'rust' -in $x86Plan.selection.preset_labels.'windows-x86' -and 'release' -notin $x86Plan.selection.requested_labels) 'x86 English changes select x86 English and mixed regressions, without RELEASE/package'
Check-Rule (@($x86Plan.selection.preset_labels.'windows-x64').Count -eq 1 -and $x86Plan.selection.preset_labels.'windows-x64'[0] -eq 'fast') 'Bounded x86 changes retain only the common x64 FAST baseline'
$submission=Select-VerificationPlan @('tests/submission.cpp')
Check-Rule ((@(Get-VerificationPresetLabels AFFECTED x86 windows-x86 $submission.Labels) -join ',') -eq 'submission') 'x86 submission changes select the actual x86 submission group'
$uiPlan=Select-VerificationPlan @('ui/View.xaml')
Check-Rule ('settings-ui' -in @(Get-VerificationPresetLabels AFFECTED x86 windows-x64 $uiPlan.Labels) -and 'candidate-host' -in @(Get-VerificationPresetLabels AFFECTED x86 windows-x86 $uiPlan.Labels)) 'x86 UI changes preserve x64 settings acceptance and x86 native candidate host'
# Unknown x86 changes retain the strongest fallback. Supply the ordinary version
# identity so PlanOnly can report RELEASE without running a product build.
$null=New-Item -ItemType Directory -Path "$fixture/ui","$fixture/windows" -Force
[IO.File]::WriteAllText("$fixture/windows/Cargo.toml","[package]`nversion = `"fixture`"`n")
[IO.File]::WriteAllText("$fixture/windows/Cargo.lock","[[package]]`nname = `"shanjie-windows`"`nversion = `"fixture`"`n")
[IO.File]::WriteAllText("$fixture/ui/Shanjie.UI.csproj",'<Project><PropertyGroup><Version>fixture</Version></PropertyGroup></Project>')
[IO.File]::WriteAllText("$fixture/unknown.bin",'unknown impact')
$fallback=Read-Planned AFFECTED Worktree HEAD x86
Check-Rule ($fallback.level -eq 'RELEASE' -and $fallback.requested_level -eq 'AFFECTED' -and 'full' -in $fallback.selection.preset_labels.'windows-x64' -and 'release' -in $fallback.selection.preset_labels.'windows-x86' -and !$fallback.results.Count -and $fallback.status -eq 'planned') 'Unknown x86 impact expands to dual-architecture RELEASE without claiming acceptance'
Write-Host "PASS supplemental verification: $checks checks; no native verification executed"
