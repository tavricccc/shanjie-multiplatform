#Requires -Version 7
# Table-driven checks for scripts/verify-selection.ps1. No git, builds or processes.
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot/../scripts/verify-selection.ps1"
$failures = 0
function Expect([string]$Name, [string[]]$Paths, [string[]]$Steps, [string[]]$Suites = @(), [bool]$Build = $false) {
    $plan = Select-VerificationPlan $Paths
    $got = "steps=$((@($plan.Steps) | Sort-Object) -join ',') suites=$((@($plan.Suites) | Sort-Object) -join ',') build=$($plan.Build)"
    $want = "steps=$((@($Steps) | Sort-Object) -join ',') suites=$((@($Suites) | Sort-Object) -join ',') build=$Build"
    if ($got -ne $want) { Write-Host "FAIL $Name`n  want $want`n  got  $got"; $script:failures++ } else { Write-Host "ok   $Name" }
}
Expect 'nothing changed' @() @()
Expect 'docs only' @('docs/DESIGN.md', 'Changelog.md', 'README.md') @()
Expect 'windows-style separators' @('docs\ARCHITECTURE.md') @()
Expect 'release script' @('scripts/release.py') @('release')
Expect 'release workflow' @('.github/workflows/release.yml') @('release')
Expect 'ui' @('ui/Controls/Switch.cs') @('rust', 'ui') -Build $true
Expect 'add-word' @('windows/src/word_helper.rs') @('rust') @('add-word', 'popup_lifecycle') -Build $true
Expect 'candidate' @('windows/src/candidate.rs') @('rust', 'ui-native') @('tsf_compat') -Build $true
Expect 'installer' @('windows/src/installer/progress.rs') @('rust')
Expect 'platform' @('platform/smart_input_windows.rs') @('rust', 'core', 'mixed', 'measure') @('english-keys', 'submission') -Build $true
Expect 'suites merge without duplicates' @('windows/src/word_popup.rs', 'tests/popup_lifecycle.cpp', 'tests/english_keys.cpp') @('rust', 'mixed') @('add-word', 'popup_lifecycle', 'english-keys') -Build $true
# Anything unrecognised, and shared or core-wide inputs, must fall back to the full verification.
Expect 'unlisted windows source' @('windows/src/tsf/service.rs') @('rust', 'full')
Expect 'shared crate' @('shared/src/appearance.rs') @('rust', 'full')
Expect 'upstream pin' @('UPSTREAM.json') @('rust', 'full')
Expect 'build script' @('scripts/build.ps1') @('rust', 'full')
Expect 'verify script' @('scripts/verify.ps1') @('rust', 'full')
Expect 'unknown new file' @('tools/something-new.txt') @('rust', 'full')
Expect 'full absorbs harness suites but keeps ui and latency' @('shared/src/lib.rs', 'ui/App.xaml.cs', 'platform/composition_windows.rs', 'windows/src/word_helper.rs') @('rust', 'full', 'ui', 'measure')
Expect 'renamed file counts both names' @('docs/old.md', 'windows/src/tsf/service.rs') @('rust', 'full')
if ($failures) { throw "$failures selection case(s) failed." }
Write-Host 'PASS verification selection'
