function Test-VerificationDocumentation([string]$Path) {
    return ($Path -match '^(docs/|LICENSES/|\.github/(ISSUE_TEMPLATE/|pull_request_template\.md$)|README(?:\..*)?$|AGENTS\.md$|Changelog\.md$|UPSTREAM-CHANGES\.md$)' -or $Path -match '\.(md|rst|adoc)$')
}

function Get-VerificationImpact([string]$Path) {
    if(Test-VerificationDocumentation $Path){return [pscustomobject]@{Full=$false;Labels=@();Reason='Documentation-only change; no build or tests.'}}
    $full = $false
    $labels = @()
    switch -Regex ($Path) {
        '^(scripts/(release|test_release)\.py|\.github/workflows/release\.yml|scripts/verify-(changed|selection)\.ps1|tests/verify_selection\.ps1)$' {
            $labels=@('test-framework');$reason='Selection and release transaction self-tests; no product build.';break
        }
        '^(windows/Cargo\.(toml|lock)|shared/Cargo\.(toml|lock)|platform/|upstream/|data/|profiles/|MODEL-|UPSTREAM\.json)' {
            $full = $true; $reason = 'Core, model, profile, dependency, or upstream change.'; break
        }
        '^shared/src/profile\.rs$' {
            $labels = @('shared','windows','tsf','ui'); $reason = 'Shared profile changes affect input behavior and settings persistence.'; break
        }
        '^shared/src/(input|spacing)\.rs$' {
            $labels = @('shared','windows','tsf','interaction'); $reason = 'Shared key decisions affect TSF and engine interaction.'; break
        }
        '^shared/src/appearance\.rs$' {
            $full=$true;$labels=@('windows','tsf','ui','mode');$reason='Shared appearance policy affects native and WinUI rendering.';break
        }
        '^shared/' {
            $full = $true; $reason = 'Shared protocol, dictionary, model, or module changes affect downstream input behavior.'; break
        }
        '^windows/src/(profile|bridge)\.rs$' {
            $labels = @('windows','tsf','ui'); $reason = 'Profile and bridge changes affect refresh, TSF, and WinUI persistence.'; break
        }
        '^windows/src/(candidate|native|font|glass|appearance)\.rs$' {
            $labels = @('windows','tsf','ui','mode'); $reason = 'Native UI changes affect candidate, mode, and TSF behavior.'; break
        }
        '^windows/src/(word_|dictionary_store|selection_)' {
            $labels = @('windows','tsf','word','ui'); $reason = 'Dictionary and selection changes affect word entry and popup behavior.'; break
        }
        '^windows/src/installer(\.rs|/)|^windows/src/runtime_log\.rs$' {
            $labels=@('windows');$reason='Installer and diagnostic logic use the Windows unit suite.';break
        }
        '^windows/src/mixed_regression\.rs$' {
            $labels=@('windows','english','models');$reason='Mixed-input regressions and native English/digit cases.';break
        }
        '^windows/src/tsf/service\.rs$' {
            $full=$true;$labels=@('windows','tsf','ui','mode');$reason='TSF service lifecycle and appearance cross native and WinUI boundaries.';break
        }
        '^windows/src/tsf/' {
            $labels = @('windows','tsf'); $reason = 'TSF changes require the isolated input regression matrix.'; break
        }
        '^windows/src/' {
            $full = $true; $reason = 'Windows engine, broker, lifecycle, installation, or unmapped module change.'; break
        }
        '^tests/english_keys\.cpp$|^scripts/verify-english-keys\.ps1$' { $labels = @('english','models'); $reason = 'English-key and mixed-input regression harnesses.'; break }
        '^tests/submission\.cpp$|^scripts/verify-submission\.ps1$' { $labels = @('submission'); $reason = 'Submission regression harness.'; break }
        '^tests/add_word_popup\.cpp$|^scripts/verify-add-word\.ps1$' { $labels = @('word','private-desktop'); $reason = 'Add-word and popup lifecycle regression harnesses.'; break }
        '^tests/popup_lifecycle\.cpp$' { $labels=@('word','private-desktop');$reason='Popup lifecycle and add-word regressions.';break }
        '^tests/(tsf_compat|popup_lifecycle)\.cpp$|^scripts/verify-private-desktop\.ps1$' { $labels = @('private-desktop'); $reason = 'Private-desktop regression harness.'; break }
        '^tests/tsf_smoke\.cpp$' { $labels = @('smoke'); $reason = 'TSF smoke harness.'; break }
        '^tests/candidate_host_smoke\.cpp$|^ui/|^scripts/verify-ui\.ps1$' { $labels = @('ui'); $reason = 'WinUI and candidate-host behavior.'; break }
        '^tests/mode_button_smoke\.cpp$|^tests/support/mode_indicator\.(cpp|h)$|^scripts/verify-mode\.ps1$' { $labels = @('mode'); $reason = 'Offscreen mode-indicator harness.'; break }
        '^tests/windows_interaction\.cpp$|^scripts/verify-interaction\.ps1$' { $labels = @('interaction'); $reason = 'Isolated Windows interaction harness.'; break }
        default { $full = $true; $reason = 'Build, test framework, CI, or unmapped change; use conservative FULL coverage.' }
    }
    [pscustomobject]@{ Full = $full; Labels = $labels; Reason = $reason }
}

# Compatibility for upstream's pure planner. Both entry points use this module's
# one impact policy; the old script/Steps scheduler is replaced by CTest levels.
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
    [pscustomobject]@{Full=$full;Skip=(!$code);Labels=@($labels|Sort-Object);Reasons=$reasons}
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

function Assert-FixedCiSelection([object[]]$Rows) {
    $expected=@('fast.infrastructure','fast.upstream-data','fast.prepare-core','fast.rust.shared','fast.rust.windows')
    $actual=@($Rows|ForEach-Object name)
    if($actual.Count -ne $expected.Count -or @(Compare-Object $expected $actual).Count){throw 'CI must contain exactly the fixed lightweight baseline; no missing tests or FULL expansion is allowed.'}
}

Export-ModuleMember -Function Test-VerificationDocumentation,Select-VerificationPlan,Get-VerificationImpact,Get-EnglishKeyCases,Get-SubmissionCases,Get-HarnessCasePlan,Assert-FixedCiSelection
