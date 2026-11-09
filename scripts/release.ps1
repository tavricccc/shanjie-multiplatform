#Requires -Version 7
param([switch]$RetryFailures)
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
Import-Module "$PSScriptRoot/release-support.psm1" -ErrorAction Stop
$identity=Get-ReleaseIdentity $root
if(!$identity.clean){throw 'Commit the intended source first; a local formal release requires a clean worktree.'}
$accepted=$null
$receipts=@(Get-ChildItem -LiteralPath (Join-Path $root 'build/reports') -Directory -Filter 'release-*' -ErrorAction SilentlyContinue | Sort-Object LastWriteTimeUtc -Descending)
foreach($directory in $receipts){
    $path=Join-Path $directory.FullName 'report.json'
    if(!(Test-Path -LiteralPath $path)){continue}
    try{$accepted=Assert-ReleaseAcceptance -ProjectRoot $root -ReportPath $path;break}
    catch{Write-Verbose "Receipt cannot be reused: $($_.Exception.Message)"}
}
if(!$accepted){
    & "$PSScriptRoot/test.ps1" -Level RELEASE -RetryFailures:$RetryFailures
    $latest=Get-ChildItem -LiteralPath (Join-Path $root 'build/reports') -Directory -Filter 'release-*' | Sort-Object LastWriteTimeUtc -Descending | Select-Object -First 1
    $accepted=Assert-ReleaseAcceptance -ProjectRoot $root -ReportPath (Join-Path $latest.FullName 'report.json')
}else{Write-Host 'Reusing the complete RELEASE receipt: source, required tests, architectures and artifact hashes still match.'}
& "$PSScriptRoot/package.ps1" -Promote -ReportPath $accepted.report_path
