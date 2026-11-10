#Requires -Version 7
# Preserve the upstream selection scenarios against the shared CTest impact policy.
$ErrorActionPreference='Stop'
. "$PSScriptRoot/../scripts/verify-selection.ps1"
$failures=0
function Expect([string]$Name,[string[]]$Paths,[bool]$Full,[string[]]$Labels=@(),[bool]$Skip=$false){
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
Expect 'ui' @('ui/Controls/Switch.cs') $false @('fast','ui')
Expect 'add-word' @('windows/src/word_helper.rs') $false @('fast','windows','tsf','word','ui')
Expect 'candidate' @('windows/src/candidate.rs') $false @('fast','windows','tsf','ui','mode')
Expect 'installer' @('windows/src/installer/progress.rs') $false @('fast','windows')
Expect 'platform' @('platform/smart_input_windows.rs') $true @('full')
Expect 'suites merge without duplicates' @('windows/src/word_popup.rs','tests/popup_lifecycle.cpp','tests/english_keys.cpp') $false @('fast','windows','tsf','word','ui','private-desktop','english','models')
Expect 'unlisted Windows service' @('windows/src/tsf/service.rs') $true @('full','windows','tsf','ui','mode')
Expect 'shared appearance' @('shared/src/appearance.rs') $true @('full','windows','tsf','ui','mode')
Expect 'upstream pin' @('UPSTREAM.json') $true @('full')
Expect 'build script' @('scripts/build.ps1') $true @('full')
Expect 'verify script' @('scripts/verify.ps1') $true @('full')
Expect 'unknown new file' @('tools/something-new.txt') $true @('full')
Expect 'full expansion retains mapped UI and TSF' @('shared/src/lib.rs','ui/App.xaml.cs','platform/composition_windows.rs','windows/src/word_helper.rs') $true @('full','ui','windows','tsf','word')
Expect 'renamed file counts both names' @('docs/old.md','windows/src/tsf/service.rs') $true @('full','windows','tsf','ui','mode')
if($failures){throw "$failures selection case(s) failed."}
Write-Host 'PASS verification selection: 19 cases, one CTest policy'
