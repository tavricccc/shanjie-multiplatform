#Requires -Version 7
# Compatibility entry: use CTest's one planner/scheduler rather than duplicate script execution.
param([string]$Base='origin/main',[switch]$DryRun,[switch]$UseExistingBuild,
    [ValidateSet('Branch','Commit','Worktree')][string]$ChangeScope='Branch',
    [ValidateSet('x64','x86')][string]$Architecture='x64')
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
$level=if($Architecture -eq 'x86'){'RELEASE'}else{'AFFECTED'}
if($Architecture -eq 'x86'){Write-Host 'The CTest x86 compatibility path uses RELEASE (x64 and x86); use CTest presets for an individual x86 case.'}
if($UseExistingBuild){Write-Verbose 'CTest validates and reuses incremental build caches; UseExistingBuild does not bypass their input checks.'}
Push-Location $root
try{& "$PSScriptRoot/test.ps1" -Level $level -BaseRef $Base -ChangeScope $ChangeScope -PlanOnly:$DryRun}
finally{Pop-Location}
