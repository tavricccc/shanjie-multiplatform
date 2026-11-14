#Requires -Version 7
# Compatibility entry: use CTest's one planner/scheduler rather than duplicate script execution.
param([string]$Base='origin/main',[switch]$DryRun,[switch]$UseExistingBuild,
    [ValidateSet('Branch','Commit','Worktree')][string]$ChangeScope='Branch',
    [ValidateSet('x64','x86')][string]$Architecture='x64')
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
if($UseExistingBuild){Write-Verbose 'CTest validates and reuses incremental build caches; UseExistingBuild does not bypass their input checks.'}
Push-Location $root
try{& "$PSScriptRoot/test.ps1" -Level AFFECTED -Architecture $Architecture -BaseRef $Base -ChangeScope $ChangeScope -PlanOnly:$DryRun}
finally{Pop-Location}
