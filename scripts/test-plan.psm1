# Resolve changes once for every architecture; --no-renames keeps both sides of
# moves, including source files moved under docs/.
function Get-VerificationChanges([string]$ProjectRoot,[string]$Ref,[string]$Scope='Branch') {
    $base=if($Scope -eq 'Commit'){'HEAD^'}elseif($Scope -eq 'Worktree'){'HEAD'}else{$Ref}
    $range=if($Scope -eq 'Worktree'){$null}elseif($Scope -eq 'Commit'){"$base..HEAD"}else{"$base...HEAD"}
    if (!(git -C $ProjectRoot rev-parse --verify --quiet $base 2>$null)) { return [pscustomobject]@{ Paths=@(); Error="Cannot resolve base ref '$base'."; Range=$range; EffectiveBase=$base } }
    $items=[Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    # Treat moves as delete/add so both paths participate in impact and documentation checks.
    $diff=if($Scope -eq 'Worktree'){@()}else{@(git -C $ProjectRoot diff --no-renames --name-only $range 2>$null)}
    if ($LASTEXITCODE) { return [pscustomobject]@{ Paths=@(); Error="Cannot compare HEAD with '$base'."; Range=$range; EffectiveBase=$base } }
    foreach($path in $diff){if($path){[void]$items.Add($path.Replace('\','/'))}}
    foreach($path in @(git -C $ProjectRoot diff --no-renames --name-only)){if($path){[void]$items.Add($path.Replace('\','/'))}}
    foreach($path in @(git -C $ProjectRoot diff --cached --no-renames --name-only)){if($path){[void]$items.Add($path.Replace('\','/'))}}
    foreach($path in @(git -C $ProjectRoot ls-files --others --exclude-standard)){if($path){[void]$items.Add($path.Replace('\','/'))}}
    return [pscustomobject]@{ Paths=@($items | Sort-Object); Error=$null; Range=$range; EffectiveBase=$base }
}

# x64 hosts the common suites/tools; an x86 affected run adds only the mapped
# x86 regressions. Unknown impact is expanded to RELEASE by the caller.
function Get-VerificationPresetLabels([string]$Level,[string]$Architecture,[string]$Preset,[string[]]$Labels) {
    if($Level -eq 'RELEASE' -and $Preset -eq 'windows-x86'){return @('release')}
    if($Level -ne 'AFFECTED' -or $Architecture -ne 'x86'){return @($Labels)}
    if($Preset -eq 'windows-x64'){
        return @('fast')+@($Labels|Where-Object{$_ -in 'core','shared','data','interaction','settings-ui'})
    }
    return @($Labels|Where-Object{$_ -notin 'fast','full','release','core','shared','data','interaction','settings-ui'})
}

function Test-VerificationDocumentation([string]$Path) {
    return ($Path -match '^(docs/|LICENSES/|\.github/(ISSUE_TEMPLATE/|pull_request_template\.md$)|README(?:\..*)?$|AGENTS\.md$|Changelog\.md$|UPSTREAM-CHANGES\.md$)' -or $Path -match '\.(md|rst|adoc)$')
}

function Get-VerificationImpact([string]$Path) {
    if(Test-VerificationDocumentation $Path){return [pscustomobject]@{Full=$false;Labels=@();Reason='Documentation-only change; no build or tests.'}}
    $full = $false
    $labels = @()
    switch -Regex ($Path) {
        '^(scripts/(release|test_release)\.py|\.github/workflows/(release|ci)\.yml|scripts/verify-(changed|selection)\.ps1|tests/verify_selection\.ps1)$' {
            $labels=@('test-framework');$reason='Selection and release transaction self-tests; no product build.';break
        }
        '^(platform/|scripts/prepare-core\.ps1$)' {
            $labels=@('rust','core','english','submission');$reason='Preserve upstream platform selection: Rust, core, mixed input, English/submission; latency is additional.';break
        }
        '^(windows/src/(word_helper|word_popup|selection_text)\.rs|windows/src/tsf/add_word\.rs|tests/(add_word_popup|popup_lifecycle)\.cpp|scripts/verify-add-word\.ps1)$' {
            $labels=@('windows','word','popup-lifecycle');$reason='Preserve upstream add-word and popup lifecycle selection without unrelated TSF/UI suites.';break
        }
        '^(windows/src/(candidate|glass|appearance|font|bridge)\.rs|windows/src/tsf/candidates\.rs|tests/candidate_host_smoke\.cpp)$' {
            $labels=@('windows','candidate-host','tsf-compat');$reason='Preserve upstream native candidate-host and TSF compatibility selection.';break
        }
        '^(windows/src/mixed_regression\.rs|tests/english_keys\.cpp|scripts/verify-english-keys\.ps1)$' {
            $labels=@('rust','english');$reason='Preserve upstream Rust/mixed-input and English-key regression selection.';break
        }
        '^ui/|^scripts/verify-ui\.ps1$' {
            $labels=@('settings-ui','candidate-host');$reason='Settings UI and native candidate host, without unrelated mode/TSF suites.';break
        }
        '^profiles/.*\.ini$' {
            $full=$true;$labels=@('shared','windows','tsf','settings-ui','candidate-host');$reason='Profile defaults affect both interactions, migrations and embedded WinUI presets; FULL alone omits the required Shanjie/UI cases.';break
        }
        '^(windows/Cargo\.(toml|lock)|shared/Cargo\.(toml|lock)|upstream/|data/|profiles/|MODEL-|UPSTREAM\.json)' {
            $full = $true; $reason = 'Core, model, profile, dependency, or upstream change.'; break
        }
        '^shared/src/profile\.rs$' {
            $full=$true;$labels=@('shared','windows','tsf','ui');$reason='Shared profile logic was unbounded upstream; preserve FULL and mapped settings persistence checks.';break
        }
        '^shared/src/(input|spacing)\.rs$' {
            $full=$true;$labels=@('shared','windows','tsf','interaction');$reason='Shared input logic has no bounded upstream rule; preserve FULL plus mapped TSF and engine interaction.';break
        }
        '^shared/src/appearance\.rs$' {
            $full=$true;$labels=@('windows','tsf','ui','mode');$reason='Shared appearance policy affects native and WinUI rendering.';break
        }
        '^shared/' {
            $full = $true; $reason = 'Shared protocol, dictionary, model, or module changes affect downstream input behavior.'; break
        }
        '^windows/src/profile\.rs$' {
            $full=$true;$labels=@('windows','tsf','ui');$reason='Unbounded Windows profile logic preserves FULL plus mapped UI persistence.';break
        }
        '^windows/src/native\.rs$' {
            $full=$true;$labels=@('windows','tsf','ui','mode');$reason='Unbounded native UI changes preserve FULL and native/WinUI checks.';break
        }
        '^windows/src/(word_|dictionary_store|selection_)' {
            $full=$true;$labels=@('windows','tsf','word','ui');$reason='Unbounded dictionary/selection changes preserve FULL plus mapped word/settings checks.';break
        }
        '^windows/src/installer(\.rs|/)|^windows/src/runtime_log\.rs$' {
            $labels=@('windows');$reason='Installer and diagnostic logic use the Windows unit suite.';break
        }
        '^windows/src/tsf/service\.rs$' {
            $full=$true;$labels=@('windows','tsf','ui','mode');$reason='TSF service lifecycle and appearance cross native and WinUI boundaries.';break
        }
        '^windows/src/tsf/' {
            $full=$true;$labels=@('windows','tsf');$reason='Unmapped TSF lifecycle/key logic preserves upstream conservative FULL coverage and mapped TSF cases.';break
        }
        '^windows/src/' {
            $full = $true; $reason = 'Windows engine, broker, lifecycle, installation, or unmapped module change.'; break
        }
        '^tests/submission\.cpp$|^scripts/verify-submission\.ps1$' { $labels = @('submission'); $reason = 'Submission regression harness.'; break }
        '^tests/tsf_compat\.cpp$|^scripts/verify-private-desktop\.ps1$' { $labels = @('private-desktop'); $reason = 'Private-desktop regression harness.'; break }
        '^tests/tsf_smoke\.cpp$' { $labels = @('smoke'); $reason = 'TSF smoke harness.'; break }
        '^tests/mode_button_smoke\.cpp$|^tests/support/mode_indicator\.(cpp|h)$|^scripts/verify-mode\.ps1$' { $labels = @('mode'); $reason = 'Offscreen mode-indicator harness.'; break }
        '^tests/windows_interaction\.cpp$|^scripts/verify-interaction\.ps1$' { $labels = @('interaction'); $reason = 'Isolated Windows interaction harness.'; break }
        default { $full = $true; $reason = 'Build, test framework, CI, or unmapped change; use conservative FULL coverage.' }
    }
    [pscustomobject]@{ Full = $full; Labels = $labels; Reason = $reason }
}

# Only entries without a direct registered CTest delegate belong here. This is
# the existing supplemental policy, shared with the lightweight inventory check.
function Get-VerificationExceptions {
    @(
        @{id='input-features';script='verify-input-features.ps1';category='conditional';pattern='^(windows/src/(input|mixed_regression)\.rs|windows/src/tsf/keys\.rs|shared/src/input\.rs|tests/submission\.cpp|scripts/verify-input-features\.ps1)$';scope='verify-input-features.ps1: choose -Architectures and zero-based -CaseIndices only for required, unverified configurations. submission.cpp shares input assertions, but input-space-commit and profile values differ from CTest input-digits; matching key text alone cannot authorize reuse.'}
        @{id='profile-refresh';script='verify-profile-refresh.ps1';category='conditional';pattern='^(profiles/|shared/src/profile\.rs$|windows/src/(profile|bridge)\.rs$|windows/src/tsf/profile_refresh\.rs$|tests/profile_refresh\.cpp$|scripts/verify-profile-refresh\.ps1$)';scope='verify-profile-refresh.ps1: review only changed resident-core notification/fault scenarios; select -Architectures/-Scenarios/-Faults. CTest submission refresh uses an in-process core, so its PASS does not establish resident prediction fault coverage.'}
        @{id='core-recovery';script='verify-core-recovery.ps1';category='conditional';pattern='^windows/src/(broker(\.rs$|/)|engine\.rs$|tsf/(resident|recovery|rerank)\.rs$|bin/shanjie-reliability-check\.rs$)|^scripts/verify-core-recovery\.ps1$';scope='verify-core-recovery.ps1: select only relevant -Architectures/-Cases for resident pipe delivery, restart, rerank faults or capacity. Upstream core-suite and submission busy tests are different fixtures, not proof of these reliability-check cases.'}
        @{id='dictionary-store';script='verify-dictionary-store.ps1';category='conditional';pattern='^(windows/src/dictionary_store\.rs|tests/dictionary_store\.cpp|scripts/verify-dictionary-store\.ps1)$';scope='verify-dictionary-store.ps1: review the sj_dictionary_save ABI, truncated-file repair/hash and simultaneous x64/x86 publication. Ordinary word-add/unit tests do not establish this cross-process fixture. No case filter exists; report overlap/remaining need before running the whole tool.'}
        @{id='profile-coexistence';script='verify-profile-coexistence.ps1';category='conditional';pattern='^(profiles/|shared/src/profile\.rs$|windows/src/(profile|bridge)\.rs$|tests/profile_coexistence\.cpp$|scripts/verify-profile-coexistence\.ps1$)';scope='verify-profile-coexistence.ps1: only when format/migration compatibility needs checking; requires -OldPackage and matching -OldSchema, and supports -Architecture. Current-version UI persistence is not old/new DLL coexistence. Missing old package means not run, never PASS.'}
        @{id='update-probes';script='verify-update.ps1';category='conditional';pattern='^windows/src/(tsf/(keys|focus_handoff|service)\.rs$|broker(\.rs$|/)|bin/shanjie-core-check\.rs$)|^tests/caps_lock\.cpp$|^scripts/verify-update\.ps1$';scope='verify-update.ps1 tests Caps Lock/field/immediate-input and resident-core probes, not an installer update. Compare its configurations with actual CTest evidence first. It has no scenario/architecture filter: identify the remaining need and duplication explicitly; do not automatically rerun it.'}
        @{id='no-admin-wrapper';script='verify-no-admin.ps1';category='wrapper';pattern='^scripts/verify-no-admin\.ps1$';scope='verify-no-admin.ps1 is a composite wrapper, not an extra assertion suite. Matching successful RELEASE cases in the same non-elevated environment need no wrapper rerun; use CTest for any missing required cases. A different privilege/environment claim still needs separate evidence.'}
        @{id='engine-measurement';script='measure-engine.ps1';category='required-extra';pattern=$null;scope='IPC/key latency is required for key/core-request changes; core-suite and TSF correctness are not equivalent.'}
        @{id='test-entry';script='test.ps1';category='dispatcher';pattern=$null;scope='Canonical CMake/CTest planner and report entry, not another assertion suite.'}
        @{id='changed-entry';script='verify-changed.ps1';category='dispatcher';pattern=$null;scope='Compatibility entry delegates to test.ps1 and the shared impact policy.'}
        @{id='selection-api';script='verify-selection.ps1';category='dispatcher';pattern=$null;scope='Pure compatibility API imports test-plan.psm1; exercised by fast.infrastructure.'}
        @{id='regression-wrapper';script='verify-regressions.ps1';category='wrapper';pattern=$null;scope='Legacy suite wrapper; individual TSF/English/submission/private-desktop CTest rows retain its assertions.'}
        @{id='model-fixture-helper';script='test-models.ps1';category='helper';pattern=$null;scope='Public model fixture helper used by verify-update.ps1, not an independent test entry.'}
    )
}

# Compatibility for upstream's pure planner. Both entry points use this module's
# one impact policy; the old script/Steps scheduler is replaced by CTest levels.
function Get-AdditionalVerificationPlan([string[]]$Paths,[string[]]$Labels,[switch]$CiSafe) {
    $normalized=@($Paths|Where-Object{$_}|ForEach-Object{$_.Replace('\','/')}|Sort-Object -Unique)
    $ruleDocuments=@($normalized|Where-Object{$_ -match '^(AGENTS\.md|docs/(TESTING|ARCHITECTURE|PERFORMANCE)\.md)$'})
    if($ruleDocuments.Count){
        [pscustomobject]@{id='markdown-rules';kind='review';status='not-run';command=$null;paths=$ruleDocuments;reason='Verification-rule Markdown changed: review added/changed requirements and synchronize test-plan.psm1 plus selection tests. This is a path-change notice, not natural-language interpretation. Documentation alone still causes no build/test; mentioning a script does not make it mandatory.'}
    }
    $paths=@($normalized|Where-Object{!(Test-VerificationDocumentation $_)})
    if(!$paths.Count){return}
    # These are advisories, never execution results or another CTest inventory.
    # Only the upstream platform rule and known key/request modules require latency.
    $keyPaths=@($paths|Where-Object{$_ -match '^(platform/|scripts/prepare-core\.ps1$|windows/src/(engine|input|broker)(\.rs$|/)|windows/src/tsf/(keys|resident|recovery|rerank)\.rs$|shared/src/(input|spacing|protocol)\.rs$)'})
    if($keyPaths.Count){
        [pscustomobject]@{id='engine-latency';kind='required-extra';status='not-run';command='pwsh -File scripts/measure-engine.ps1';paths=$keyPaths;reason='AGENTS.md requires latency measurement for key paths/core requests; functional CTest cases do not measure equivalent IPC/key latency. Requires matching x64 release library, dist data/core and MSVC; report the actual measurements.'}
    }
    $uiPaths=@($paths|Where-Object{$_ -match '^ui/|^scripts/verify-ui\.ps1$'})
    if($uiPaths.Count){
        $uiSelected=(!$CiSafe -and (@($Labels|Where-Object{$_ -in 'ui','release'}).Count -gt 0 -or ('settings-ui' -in $Labels -and 'candidate-host' -in $Labels)))
        [pscustomobject]@{id='ui-acceptance';kind=if($uiSelected){'ctest-planned'}else{'ctest-needed'};status='not-run';command=if($uiSelected){$null}else{'ctest --preset windows-x64 -L "settings-ui|candidate-host" --output-on-failure'};paths=$uiPaths;reason=if($uiSelected){'UI checks are requested through CTest; credit only the actually selected/passing rows and subcases. Do not also run verify-ui.ps1 or its inactive -Benchmark switch.'}else{'The upstream UI requirement is not covered by these labels or CI-safe execution. Prepare the matching product/CMake harnesses; use -R for any remaining subset if some UI cases already passed. FULL alone does not include release.ui.*. Do not rerun the whole legacy UI script.'}}
    }
    $candidatePaths=@($paths|Where-Object{$_ -match '^windows/src/(candidate|glass|appearance|font|bridge)\.rs$|^windows/src/tsf/candidates\.rs$|^tests/candidate_host_smoke\.cpp$'})
    if($candidatePaths.Count){
        $candidateSelected=(!$CiSafe -and @($Labels|Where-Object{$_ -in 'candidate-host','ui','release'}).Count -gt 0)
        [pscustomobject]@{id='candidate-acceptance';kind=if($candidateSelected){'ctest-planned'}else{'ctest-needed'};status='not-run';command=if($candidateSelected){$null}else{'ctest --preset windows-x64 -L candidate-host --output-on-failure'};paths=$candidatePaths;reason='Upstream candidate changes need the native candidate-host probe, not a duplicate full WinUI script. Credit actual matching results; if missing, prepare the corresponding harness/product and run only the missing CTest candidate-host rows.'}
    }
    if(@($Labels|Where-Object{$_ -in 'full','release','tsf','tsf-compat','english','submission','word','private-desktop'}).Count){
        [pscustomobject]@{id='legacy-regressions';kind='ctest-planned';status='not-run';command=$null;paths=@();reason='Use the selected CTest core/TSF/English/submission/private-desktop rows and parameter subcases instead of also running verify.ps1 or verify-regressions.ps1. A requested label is not proof of execution; unselected, skipped or failed required cases still need the missing CTest rows.'}
    }
    # Upstream only says "related verify-*.ps1" for these modules; it gives no
    # mandatory per-file matrix. Dependencies below identify review candidates,
    # not a claim that all their cases are required or absent from CTest.
    $rules=@(Get-VerificationExceptions|Where-Object pattern)
    foreach($rule in $rules){
        $affected=@($paths|Where-Object{$_ -match $rule.pattern})
        if($affected.Count){[pscustomobject]@{id=$rule.id;kind='review';status='not-run';command=$null;paths=$affected;reason=$rule.scope}}
    }
    $managerPaths=@($paths|Where-Object{$_ -eq 'windows/src/bin/shanjie-manager.rs'})
    if($managerPaths.Count){
        [pscustomobject]@{id='manager-binary-coverage';kind='coverage-review';status='not-run';command=$null;paths=$managerPaths;reason='Cargo --lib does not execute the manager binary. CTest package verification covers verify-package, not after-update notification failure/logging or real installation. Review the changed command and add a targeted regression for new behavior; do not claim FULL proves those branches.'}
    }
    if(!$keyPaths.Count -and @($paths|Where-Object{$_ -match '^scripts/measure-engine\.ps1$|^tests/engine_perf\.cpp$'}).Count){
        [pscustomobject]@{id='engine-measurement-tool';kind='optional';status='not-run';command='pwsh -File scripts/measure-engine.ps1';paths=$paths;reason='Only the measurement tool changed; collect new latency samples if needed. This is not a functional CTest result; -Learning is an optional 8000-row learning fixture.'}
    }
}

function Select-VerificationPlan([string[]]$Paths) {
    $labels=[Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    $reasons=@();$full=$false;$code=$false
    foreach($path in @($Paths|Where-Object{$_}|ForEach-Object{$_.Replace('\','/')}|Sort-Object -Unique)){
        $impact=Get-VerificationImpact $path
        if(!(Test-VerificationDocumentation $path)){$code=$true}
        $full=$full -or $impact.Full
        foreach($label in $impact.Labels){[void]$labels.Add($label)}
        $reasons+=,[pscustomobject]@{Path=$path;Reason=$impact.Reason}
    }
    if($code){[void]$labels.Add($(if($full){'full'}else{'fast'}))}
    [pscustomobject]@{Full=$full;Skip=(!$code);Labels=@($labels|Sort-Object);Reasons=$reasons;AdditionalVerification=@(Get-AdditionalVerificationPlan -Paths $Paths -Labels @($labels))}
}

function Get-EnglishKeyCases {
    # Mode bits: 1 raw-key mixing, 2 upstream English; these are mutually exclusive choices.
    $cases=@();foreach($mode in 1,2){foreach($scenario in 'caps','caps-zh','english-symbol-order','digit-navigation'){$cases+=,@($mode,$scenario,1,1)}}
    foreach($mode in 1,2){$cases+=,@($mode,'shift-caps-zh',1,1)}
    foreach($scenario in 'shift','shift-caps'){$cases+=,@(2,$scenario,1,1)}
    $cases+=,@(1,'eten',2,1);$cases+=,@(0,'partial',1,1);$cases+=,@(0,'whole',1,0);$cases+=,@(0,'candidate',1,1)
    foreach($keyboard in 1,3,4){$cases+=,@(0,'backquote',$keyboard,1)}
    $cases+=,@(0,'backquote-disabled',1,1)
    foreach($mode in 0,1,2){$cases+=,@($mode,'numpad',1,1)}
    $cases+=,@(0,'numpad',2,1)
    foreach($mode in 0,1,2){$cases+=,@($mode,'standalone-tones',1,1)}
    $cases+=,@(2,'standalone-tones-off',1,1);$cases+=,@(2,'standalone-tones-shanjie',1,1)
    $cases+=,@(0,'fullwidth',1,1);$cases+=,@(0,'fullwidth-english',1,1);$cases+=,@(0,'fullwidth-disabled',1,1)
    return ,$cases
}

function Get-SubmissionCases([int]$Keyboard) {
    $prefix=if($Keyboard -eq 1){'SU3'}else{'NE3'}
    $cases=@(@('commit','ADB','adb'),@('commit','XYZ','xyz'),@('commit',$prefix,'你'),@('commit',($prefix+'ADB'),'你adb'),@('commit',('A'*96),('a'*96)),@('cancel','ADB',''),@('focus','ADB','adb'),@('toggle','ADB','adb'),@('refresh','ADB','adb'),@('return','ADB','adb'),@('cancel-generation','ADB',''),@('deactivate','ADB','adb'),@('refresh-twice','ADB','adb'),@('busy','A','adb'),@('busy-epoch','A','adb'),@('busy-cancel','A',''),@('busy-focus','A','a'),@('busy-expire','A','adb'),@('busy-pick',$prefix,'妳'),@('busy-mouse',$prefix,'妳'),@('retire-write-fail','ADB','adb'),@('retire-request-fail','ADB','adb'),@('retire-escape','ADB','adb'),@('retire-deactivate','ADB','adb'),@('retire-stale','ADB','adb'))
    if($Keyboard -eq 1) {
        foreach($mix in 0,1,2) {
            $cases+=,@('input-digits','9 U. ','哎呦',$mix)
            $cases+=,@('input-digits','GK42U/4','設定',$mix)
            $cases+=,@('input-digits','9 ⎋GK42U/4','設定',$mix)
            if(!($mix -band 2)) { $cases+=,@('input-digits','0D4','看',$mix) }
            if($mix) { $cases+=,@('input-digits','13 ','13 ',$mix) }
            if($mix -band 1) {
                foreach($digits in '13','14','16','17','13467','R3','R4','R6','R7','R3467') { $cases+=,@('input-digits',$digits,$digits.ToLowerInvariant(),$mix) }
                $cases+=,@('input-digits','GK42U⌫U/4','設定',$mix)
            }
            if($mix -band 2) { foreach($digits in '3.14','0.5','2026/10/15') { $cases+=,@('input-digits',$digits,$digits,$mix) } }
        }
    }
    return ,$cases
}

function Get-HarnessCasePlan([object[]]$Rows) {
    foreach($row in $Rows) {
        if($row.name -match '\.english\.(.+)\.(x64|x86)$') {
            $scenario=$Matches[1];$architecture=$Matches[2]
            foreach($case in (Get-EnglishKeyCases)) {
                if($case[1] -ne $scenario){continue}
                [pscustomobject]@{
                    name="$($row.name)/mode$($case[0])-keyboard$($case[2])-enter$($case[3])"
                    parent=$row.name; preset=$row.preset; category='english.keys'
                    parameters=@{architecture=$architecture;scenario=$scenario;mode=$case[0];keyboard=$case[2];enter_to_cursor=$case[3]}
                    selected=$row.selected; reason=$row.reason; status='not-run'; attempts=@(); flaky=$false
                }
            }
        } elseif($row.name -match '^release\.ui\.startup\.(clean|reopen|different-directory)$') {
            $phase=$Matches[1]
            [pscustomobject]@{
                name="$($row.name)/$phase";parent=$row.name;preset=$row.preset;category='ui.startup'
                parameters=@{architecture='x64';phase=$phase}
                selected=$row.selected;reason=$row.reason;status='not-run';attempts=@();flaky=$false
            }
        } elseif($row.name -match '^release\.ui\.disabled\.keyboard([123])$') {
            $keyboard=[int]$Matches[1]
            foreach($phase in 'initial','reopen'){
                [pscustomobject]@{
                    name="$($row.name)/$phase";parent=$row.name;preset=$row.preset;category='ui.disabled'
                    parameters=@{architecture='x64';keyboard=$keyboard;phase=$phase}
                    selected=$row.selected;reason=$row.reason;status='not-run';attempts=@();flaky=$false
                }
            }
        } elseif($row.name -match '\.submission\.(.+)\.(x64|x86)$') {
            $mode=$Matches[1];$architecture=$Matches[2]
            foreach($keyboard in 1,2) {
                $index=0
                foreach($case in (Get-SubmissionCases $keyboard)) {
                    $index++
                    if($case[0] -ne $mode){continue}
                    [pscustomobject]@{
                        name="$($row.name)/keyboard$keyboard-case$index"
                        parent=$row.name; preset=$row.preset; category='submission.case'
                        parameters=@{architecture=$architecture;keyboard=$keyboard;mode=$mode;mix=if($case.Count -gt 3){$case[3]}else{1};keys=$case[1];expected=$case[2]}
                        selected=$row.selected; reason=$row.reason; status='not-run'; attempts=@(); flaky=$false
                    }
                }
            }
        }
    }
}

function Read-HarnessCaseResults([object[]]$CaseRows,[object[]]$Timings) {
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
    return $subcases
}

function Get-VerificationCompletion([string]$Level,[string]$Status,[object[]]$AdditionalVerification,[bool]$NoTestsRequired) {
    $pending=@($AdditionalVerification|Where-Object{$_.kind -in 'required-extra','ctest-needed','coverage-review'})
    $summary=if($Status -ne 'passed'){$Status}elseif($NoTestsRequired){'not-required'}elseif($Level -eq 'CI'){'ci-baseline-passed'}elseif($pending.Count){'follow-up-required'}elseif($Level -eq 'FAST'){'fast-baseline-passed'}else{'selected-tests-passed'}
    [pscustomobject]@{status=$summary;executed_scope=$Level;selected_tests_status=$Status;follow_up_required=(!$NoTestsRequired -and $pending.Count -gt 0);pending=@($pending|Select-Object id,kind,status,paths,reason);conditional_reviews=@($AdditionalVerification|Where-Object kind -eq 'review');meaning='Selected test results do not prove coverage of arbitrary code branches. Resolve required extras and relevant reviews separately; never infer whole-change acceptance from CI/CTest PASS alone.'}
}

function Assert-FixedCiSelection([object[]]$Rows) {
    $expected=@('fast.infrastructure','fast.upstream-data','fast.prepare-core','fast.rust.shared','fast.rust.windows')
    $actual=@($Rows|ForEach-Object name)
    if($actual.Count -ne $expected.Count -or @(Compare-Object $expected $actual).Count){throw 'CI must contain exactly the fixed lightweight baseline; no missing tests or FULL expansion is allowed.'}
}

Export-ModuleMember -Function Get-VerificationExceptions,Get-VerificationCompletion,Get-VerificationChanges,Get-VerificationPresetLabels,Read-HarnessCaseResults,Test-VerificationDocumentation,Select-VerificationPlan,Get-VerificationImpact,Get-AdditionalVerificationPlan,Get-EnglishKeyCases,Get-SubmissionCases,Get-HarnessCasePlan,Assert-FixedCiSelection
