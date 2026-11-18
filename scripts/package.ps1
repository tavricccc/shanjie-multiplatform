#Requires -Version 7
# Normal packaging creates a candidate. Only guarded promotion completes a local release.
[CmdletBinding(DefaultParameterSetName='Candidate')]
param(
    [Parameter(ParameterSetName='Candidate')][switch]$UseExistingBuild,
    [Parameter(ParameterSetName='Candidate')][switch]$SkipVerify,
    [Parameter(ParameterSetName='Candidate')][ValidatePattern('^[0-9a-f]{32}$')][string]$CandidateId=[guid]::NewGuid().ToString('N'),
    [Parameter(ParameterSetName='Candidate')][string]$SnapshotHash,
    [Parameter(ParameterSetName='Candidate')][string]$PackagingPermitPath,
    [Parameter(Mandatory,ParameterSetName='Promote')][switch]$Promote,
    [Parameter(Mandatory,ParameterSetName='CiArtifact')][switch]$ExportCiArtifact,
    [Parameter(Mandatory,ParameterSetName='Promote')][Parameter(Mandatory,ParameterSetName='CiArtifact')][string]$ReportPath
)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
. "$PSScriptRoot/preserve-output.ps1"
Import-Module "$PSScriptRoot/release-support.psm1" -ErrorAction Stop
if($Promote -or $ExportCiArtifact){
    $null=Complete-ReleaseArtifact -ProjectRoot $root -ReportPath $ReportPath -CiArtifact:$ExportCiArtifact
    return
}
$candidateRoot=Join-Path $root "build/release-candidates/$CandidateId"
$snapshot=$null;$permit=$null
if($SnapshotHash -or $PackagingPermitPath){
    if(!$UseExistingBuild -or $SkipVerify){throw 'A RELEASE snapshot requires existing builds and full package integrity verification.'}
    $permit=Assert-ReleasePackagingPermit $root $PackagingPermitPath $CandidateId $SnapshotHash
    $snapshot=$permit.snapshot
}
Push-Location $root
try {
    if (!$UseExistingBuild) {
        & "$PSScriptRoot/build.ps1"
    }
    $output = Join-Path $candidateRoot 'package'
    $runtimeRoot=if($snapshot){$snapshot.runtime_root}else{Join-Path $root 'dist'}
    if(!(Test-Path -LiteralPath "$runtimeRoot/tools/shanjie-word-helper.exe")){throw 'Missing per-user add-word helper; refusing an incomplete package.'}
    Preserve-Output $output
    if($snapshot){
        Copy-ReleaseFiles $snapshot.runtime_root $output
        Assert-ReleasePackageCopy $snapshot $output
    }else{
        & "$PSScriptRoot/prepare-licenses.ps1"
        Copy-ReleasePackageInputs $root $runtimeRoot $output
    }
    $hashes = Get-ChildItem $output -File -Recurse | Where-Object Name -ne 'SHA256SUMS.txt' | ForEach-Object {
        $relative = [System.IO.Path]::GetRelativePath($output,$_.FullName)
        "$( (Get-FileHash $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant() )  $relative"
    }
    Set-Content "$output/SHA256SUMS.txt" $hashes -Encoding utf8NoBOM
    if (!$SkipVerify) {
        & "$output/tools/shanjie-manager.exe" verify-package $output
        if ($LASTEXITCODE) { throw 'Native package verification failed.' }
    }
    # The only distributed form is the single-file installer: the setup stub carrying this same verified package.
    $setup = Join-Path $candidateRoot 'Shanjie-Windows-Setup.exe'
    Preserve-Output $setup
    $packagingRoot=if($snapshot){Join-Path $snapshot.root 'packaging'}else{Join-Path $root 'dist/setup'}
    if($snapshot){Assert-ReleasePackageCopy $snapshot $output}
    & "$packagingRoot/shanjie-pack.exe" "$packagingRoot/安裝善解.exe" $output $setup
    if ($LASTEXITCODE) { throw 'Single-file installer packing failed.' }
    if($snapshot){
        $null=Get-ReleaseSnapshot $root $snapshot.id $snapshot.sha256 -VerifyFiles
        Assert-ReleasePackageCopy $snapshot $output
        $null=Assert-ReleasePackagingPermit $root $PackagingPermitPath $CandidateId $SnapshotHash
    }
    $installerHash=(Get-FileHash -LiteralPath $setup).Hash.ToLowerInvariant()
    Set-Content "$setup.sha256" "$installerHash  Shanjie-Windows-Setup.exe" -Encoding utf8NoBOM
    $candidate=[ordered]@{schema=2;id=$CandidateId;snapshot_id=$snapshot.id;snapshot_sha256=$snapshot.sha256;packaging_permit_sha256=$permit.sha256;source=(Get-ReleaseIdentity $root);integrity_verified=(!$SkipVerify);installer_sha256=$installerHash;created_utc=[DateTimeOffset]::UtcNow.ToString('O')}
    [IO.File]::WriteAllText((Join-Path $candidateRoot 'candidate.json'),($candidate|ConvertTo-Json -Depth 5),[Text.UTF8Encoding]::new($false))
    Write-Host "Candidate only: $setup. Complete local release with scripts/release.ps1."
} finally { Pop-Location }
