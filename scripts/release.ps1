#Requires -Version 7
param([switch]$RetryFailures)
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
Import-Module "$PSScriptRoot/release-support.psm1" -ErrorAction Stop
$accepted=Complete-ReleaseArtifact -ProjectRoot $root
if(!$accepted){
    $identity=Get-ReleaseIdentity $root
    if(!$identity.clean){throw 'Commit the intended source first; a local formal release requires a clean worktree.'}
    & "$PSScriptRoot/test.ps1" -Level RELEASE -RetryFailures:$RetryFailures
    $latest=Get-ChildItem -LiteralPath (Join-Path $root 'build/reports') -Directory -Filter 'release-*' | Sort-Object LastWriteTimeUtc -Descending | Select-Object -First 1
    $null=Complete-ReleaseArtifact -ProjectRoot $root -ReportPath (Join-Path $latest.FullName 'report.json')
}else{Write-Host 'Reusing the complete RELEASE receipt: source, required tests, architectures and artifact hashes still match.'}
