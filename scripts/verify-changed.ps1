#Requires -Version 7
# Runs only the verification the changed paths need (AGENTS.md「驗證：做多少」).
# Daily use only: before a release or merging into main, run the full verify.ps1.
param([string]$Base = 'origin/main', [switch]$DryRun, [switch]$UseExistingBuild,
    [ValidateSet('x64','x86')][string]$Architecture = 'x64')
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
. "$PSScriptRoot/verify-selection.ps1"
Push-Location $root
try {
    function Invoke-Git([string[]]$Arguments) { $out = & git.exe @Arguments; if ($LASTEXITCODE) { throw "git $($Arguments -join ' ') failed." }; $out }
    $mergeBase = Invoke-Git @('merge-base', $Base, 'HEAD')
    # --no-renames lists a move as a deletion plus an addition, so both paths are considered.
    $paths = @(Invoke-Git @('diff', '--name-only', '--no-renames', "$mergeBase", 'HEAD')) +
        @(Invoke-Git @('diff', '--name-only', '--no-renames', 'HEAD')) +
        @(Invoke-Git @('ls-files', '--others', '--exclude-standard'))
    $plan = Select-VerificationPlan $paths
    Write-Host "Changes since $Base ($($mergeBase.Substring(0, 8))):"
    foreach ($reason in $plan.Reasons) { Write-Host ("  {0}  [{1}] {2}" -f $reason.Path, $reason.Rule, ($(if ($reason.Steps) { $reason.Steps -join ', ' } else { 'skip' }))) }
    $summary = @($plan.Steps) + @($plan.Suites | ForEach-Object { "regressions:$_" })
    Write-Host "Plan: $(if ($summary) { $summary -join ', ' } else { 'nothing to verify' })$(if ($plan.Build -and !$UseExistingBuild) { ' (after build.ps1)' })"
    Write-Host 'Before a release or merging into main, run the full scripts/verify.ps1.'
    if ($DryRun -or !$summary) { return }

    function Run([string]$Label, [scriptblock]$Command) {
        Write-Host "== $Label"; $started = [Diagnostics.Stopwatch]::StartNew()
        $global:LASTEXITCODE = 0; & $Command
        if ($LASTEXITCODE) { throw "$Label failed (exit $LASTEXITCODE)." }
        Write-Host ("== {0} passed in {1:n1} s" -f $Label, $started.Elapsed.TotalSeconds)
    }
    $steps = @($plan.Steps)
    if ('selection' -in $steps) { Run 'selection rules' { pwsh -NoProfile -File tests/verify_selection.ps1 } }
    if ('release' -in $steps) { Run 'release transactions' { python scripts/test_release.py } }
    if ('rust' -in $steps) {
        # build/core is generated from platform/; a stale copy silently tests the old overlay.
        Run 'prepare core' { & "$PSScriptRoot/prepare-core.ps1" }
        Run 'shared unit tests' { cargo test --manifest-path shared/Cargo.toml --lib --target x86_64-pc-windows-msvc }
        Run 'windows unit tests' { cargo test --manifest-path windows/Cargo.toml --lib --target x86_64-pc-windows-msvc }
    }
    if ('full' -in $steps) {
        Run "verify.ps1 ($Architecture)" { & "$PSScriptRoot/verify.ps1" -Architecture $Architecture -UseExistingBuild:$UseExistingBuild }
    } else {
        if ($plan.Build -and !$UseExistingBuild) { Run 'build' { & "$PSScriptRoot/build.ps1" -Architecture $Architecture } }
        if ('core' -in $steps) { Run 'upstream core suite' { & "$PSScriptRoot/test-core.ps1" -UseExistingCore } }
        if ('mixed' -in $steps) { Run 'mixed regression (built data)' { cargo test --manifest-path windows/Cargo.toml --lib --target x86_64-pc-windows-msvc -- --ignored mixed_regression --test-threads=1 } }
        if ($plan.Suites) { Run "regressions: $($plan.Suites -join ', ')" { & "$PSScriptRoot/verify-regressions.ps1" -Architecture $Architecture -Suites $plan.Suites } }
        if ('ui-native' -in $steps -and 'ui' -notin $steps) { Run 'candidate host' { & "$PSScriptRoot/verify-ui.ps1" -NativeOnly -Architectures $Architecture } }
    }
    if ('ui' -in $steps) { Run 'settings UI' { & "$PSScriptRoot/verify-ui.ps1" -Architectures $Architecture -SkipNative:('ui-native' -notin $steps -and 'full' -notin $steps) } }
    if ('measure' -in $steps) { Run 'engine latency' { & "$PSScriptRoot/measure-engine.ps1" } }
    Write-Host 'PASS changed-path verification'
} finally { Pop-Location }
