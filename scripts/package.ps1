#Requires -Version 7
# Normal packaging creates a candidate. Only guarded promotion completes a local release.
[CmdletBinding(DefaultParameterSetName='Candidate')]
param(
    [Parameter(ParameterSetName='Candidate')][switch]$UseExistingBuild,
    [Parameter(ParameterSetName='Candidate')][switch]$SkipVerify,
    [Parameter(ParameterSetName='Candidate')][ValidatePattern('^[0-9a-f]{32}$')][string]$CandidateId=[guid]::NewGuid().ToString('N'),
    [Parameter(Mandatory,ParameterSetName='Promote')][switch]$Promote,
    [Parameter(Mandatory,ParameterSetName='CiArtifact')][switch]$ExportCiArtifact,
    [Parameter(Mandatory,ParameterSetName='Promote')][Parameter(Mandatory,ParameterSetName='CiArtifact')][string]$ReportPath
)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
. "$PSScriptRoot/preserve-output.ps1"
Import-Module "$PSScriptRoot/release-support.psm1" -ErrorAction Stop
if($Promote -or $ExportCiArtifact){
    if($ExportCiArtifact -and $env:GITHUB_ACTIONS -ne 'true'){throw 'CI artifact export is available only inside GitHub Actions; it does not authorize a local formal release.'}
    $accepted=Assert-ReleaseAcceptance -ProjectRoot $root -ReportPath $ReportPath -CiArtifact:$ExportCiArtifact
    $setup=Join-Path $root 'dist/Shanjie-Windows-Setup.exe'
    Preserve-Output $setup
    Copy-Item -LiteralPath $accepted.installer -Destination $setup -Force
    if((Get-FileHash -LiteralPath $setup).Hash.ToLowerInvariant() -cne $accepted.sha256){throw 'Promoted installer does not match the verified candidate.'}
    Set-Content -LiteralPath "$setup.sha256" -Value "$($accepted.sha256)  Shanjie-Windows-Setup.exe" -Encoding utf8NoBOM
    $marker=[ordered]@{schema=1;status=if($accepted.local_release_verified){'verified-local-release'}else{'github-actions-artifact'};local_release_verified=$accepted.local_release_verified;commit=$accepted.identity.commit;version=$accepted.identity.version;architectures=@('x64','x86');installer_sha256=$accepted.sha256;candidate_id=$accepted.candidate_id;report=$accepted.report_path;report_sha256=$accepted.report_sha256;created_utc=[DateTimeOffset]::UtcNow.ToString('O')}
    $markerPath=Join-Path $root 'dist/release.json'
    [IO.File]::WriteAllText("$markerPath.tmp",($marker|ConvertTo-Json -Depth 6),[Text.UTF8Encoding]::new($false))
    Move-Item -LiteralPath "$markerPath.tmp" -Destination $markerPath -Force
    Write-Host "Verified artifact: $setup ($($marker.status))"
    return
}
$candidateRoot=Join-Path $root "build/release-candidates/$CandidateId"
# Recoverable build outputs are local maintenance data, never release content.
function Copy-PayloadDirectory([string]$Source, [string]$Destination) {
    foreach ($file in Get-ChildItem -LiteralPath $Source -File -Recurse) {
        $relative = [IO.Path]::GetRelativePath($Source,$file.FullName)
        if ($relative -match '(^|[\\/])\.deleted_files([\\/]|$)') { continue }
        $target = Join-Path $Destination $relative
        New-Item -ItemType Directory -Path (Split-Path $target -Parent) -Force | Out-Null
        Copy-Item -LiteralPath $file.FullName -Destination $target -Force
    }
}
Push-Location $root
try {
    if (!$UseExistingBuild) {
        & "$PSScriptRoot/build.ps1"
    }
    $output = Join-Path $candidateRoot 'package'
    if(!(Test-Path -LiteralPath "$root/dist/tools/shanjie-word-helper.exe")){throw 'Missing per-user add-word helper; refusing an incomplete package.'}
    Preserve-Output $output
    New-Item -ItemType Directory -Path $output -Force | Out-Null
    & "$PSScriptRoot/prepare-licenses.ps1"
    foreach ($arch in @('x64','x86')) {
        New-Item -ItemType Directory -Path "$output/$arch" -Force | Out-Null
        Copy-Item "$root/dist/$arch/Shanjie.dll" "$output/$arch" -Force
    }
    foreach ($name in 'data','LICENSES') { Copy-PayloadDirectory "$root/dist/$name" "$output/$name" }
    Copy-Item "$root/UPSTREAM.json","$root/README.md","$root/Changelog.md" $output -Force
    # Command-line alternatives to the setup window stay out of the package root.
    New-Item -ItemType Directory -Path "$output/scripts" -Force | Out-Null
    Copy-Item "$PSScriptRoot/install.ps1","$PSScriptRoot/uninstall.ps1","$PSScriptRoot/install.cmd","$PSScriptRoot/uninstall.cmd","$PSScriptRoot/profiles.ps1" "$output/scripts" -Force
    Copy-Item "$PSScriptRoot/apply-profile.ps1","$PSScriptRoot/apply-windows.cmd","$PSScriptRoot/apply-shanjie.cmd" "$output/scripts" -Force
    Copy-Item "$PSScriptRoot/settings.cmd","$PSScriptRoot/重新選擇鍵盤.cmd" "$output/scripts" -Force
    Copy-Item "$root/profiles" $output -Recurse -Force
    Copy-PayloadDirectory "$root/dist/ui" "$output/ui"
    New-Item -ItemType Directory -Path "$output/tools" -Force | Out-Null
    Get-ChildItem "$root/dist/tools" -File | Copy-Item -Destination "$output/tools" -Force
    Copy-Item "$root/dist/setup/安裝善解.exe" $output -Force
    Copy-Item "$root/UPSTREAM-CHANGES.md" $output -Force
    Copy-PayloadDirectory "$root/docs" "$output/docs"
    Copy-Item "$root/MODEL-SOURCES.json","$root/MODEL-CATALOG.json" $output -Force
    Copy-Item "$root/安裝說明.txt" $output -Force
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
    & "$root/dist/setup/shanjie-pack.exe" "$root/dist/setup/安裝善解.exe" $output $setup
    if ($LASTEXITCODE) { throw 'Single-file installer packing failed.' }
    $installerHash=(Get-FileHash -LiteralPath $setup).Hash.ToLowerInvariant()
    Set-Content "$setup.sha256" "$installerHash  Shanjie-Windows-Setup.exe" -Encoding utf8NoBOM
    $candidate=[ordered]@{schema=1;id=$CandidateId;source=(Get-ReleaseIdentity $root);integrity_verified=(!$SkipVerify);installer_sha256=$installerHash;created_utc=[DateTimeOffset]::UtcNow.ToString('O')}
    [IO.File]::WriteAllText((Join-Path $candidateRoot 'candidate.json'),($candidate|ConvertTo-Json -Depth 5),[Text.UTF8Encoding]::new($false))
    Write-Host "Candidate only: $setup. Complete local release with scripts/release.ps1."
} finally { Pop-Location }
