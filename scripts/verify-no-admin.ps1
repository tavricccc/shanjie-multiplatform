#Requires -Version 7
# Runs every check that needs no elevation: build, core and upstream regression,
# background TSF harness for both profiles and architectures, UI render and packaging.
# Installing into the system (UAC) is not covered.
param([switch]$UseExistingBuild)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$log = Join-Path $root "build/verify-no-admin-$(Get-Date -Format yyyyMMdd-HHmmss)"
New-Item -ItemType Directory -Path $log -Force | Out-Null

$steps = @(
    if (!$UseExistingBuild) { @{ Name = 'build'; Args = @('-File', "$PSScriptRoot/build.ps1") } }
    @{ Name = 'core-and-tsf-x64'; Args = @('-File', "$PSScriptRoot/verify.ps1", '-UseExistingBuild') }
    @{ Name = 'tsf-x86';          Args = @('-File', "$PSScriptRoot/verify.ps1", '-NativeOnly', '-Architecture', 'x86') }
    @{ Name = 'tsf-shanjie';      Args = @('-File', "$PSScriptRoot/verify.ps1", '-NativeOnly', '-Interaction', 'shanjie') }
    @{ Name = 'interaction';      Args = @('-File', "$PSScriptRoot/verify-interaction.ps1") }
    @{ Name = 'mode';             Args = @('-File', "$PSScriptRoot/verify-mode.ps1") }
    @{ Name = 'ui';               Args = @('-File', "$PSScriptRoot/verify-ui.ps1") }
    @{ Name = 'windows-crate';    Exe = 'cargo'; Args = @('test', '--manifest-path', "$root/windows/Cargo.toml", '--lib') }
    @{ Name = 'package';          Args = @('-File', "$PSScriptRoot/package.ps1", '-UseExistingBuild') }
)

# A test core left running from dist/ locks shanjie-core.exe and breaks the build.
Get-Process shanjie-core -ErrorAction SilentlyContinue | Where-Object Path -like "$root\dist\*" | Stop-Process -Force

$results = foreach ($step in $steps) {
    $exe = $step.Exe ?? 'pwsh'
    $out = Join-Path $log "$($step.Name).log"
    $watch = [Diagnostics.Stopwatch]::StartNew()
    Write-Host "== $($step.Name)"
    & $exe @($step.Args) *> $out
    $code = $LASTEXITCODE
    [pscustomobject]@{ Step = $step.Name; Result = $(if ($code -eq 0) { 'PASS' } else { "FAIL ($code)" }); Seconds = [int]$watch.Elapsed.TotalSeconds; Log = $out }
    # Later steps would test a stale dist/ if the build did not finish.
    if ($code -and $step.Name -eq 'build') { break }
}

$results | Format-Table -AutoSize | Out-String | Tee-Object (Join-Path $log 'summary.txt')
if ($results.Result -ne 'PASS') { exit 1 }
