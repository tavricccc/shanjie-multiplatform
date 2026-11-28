#Requires -Version 7
param(
    [Parameter(Mandatory)][ValidateSet('Build','PrepareCore','Seal','Verify','Shard','Aggregate','Package')][string]$Phase,
    [ValidateSet('x64','x86')][string]$Architecture='x64',
    [ValidateSet('cap','mixed','share','latency')][string]$Shard,
    [string]$SnapshotId,
    [string]$SnapshotHash,
    [switch]$CargoVerbose
)
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
Import-Module "$PSScriptRoot/release-support.psm1" -ErrorAction Stop
Import-Module "$PSScriptRoot/core-shards.psm1" -ErrorAction Stop
Import-Module "$PSScriptRoot/test-support.psm1" -ErrorAction Stop
$parallel=Join-Path $root 'build/parallel'
$null=New-Item -ItemType Directory -Path $parallel -Force
$identity=Get-ReleaseIdentity $root
if(!$identity.clean){throw 'Parallel RELEASE requires a clean source checkout.'}

if($Phase -eq 'Build'){
    & "$PSScriptRoot/build.ps1" -Architecture $Architecture -PackageTargetsOnly -SkipSharedData:($Architecture -eq 'x86') -CargoVerbose:$CargoVerbose
    if($Architecture -eq 'x64'){& "$PSScriptRoot/prepare-licenses.ps1"}
    $triple=if($Architecture -eq 'x64'){'x86_64-pc-windows-msvc'}else{'i686-pc-windows-msvc'}
    $paths=@(if($Architecture -eq 'x64'){'dist'}else{'dist/x86'})+@("windows/target/$triple/release/shanjie_windows.lib","windows/target/$triple/release/shanjie-reliability-check.exe")
    @{schema=1;architecture=$Architecture;source=$identity;run_id=$env:GITHUB_RUN_ID;files=(Get-RelativeFileEntries -Root $root -Paths $paths -RequirePaths)} |
        ConvertTo-Json -Depth 10 | Set-Content "$parallel/build-$Architecture.json" -Encoding utf8NoBOM
    & python "$PSScriptRoot/release_artifacts.py" pack "$parallel/build-$Architecture.tar" --root $root --paths @paths "build/parallel/build-$Architecture.json"
    if($LASTEXITCODE){throw 'Cannot archive release build inputs.'}
    return
}
if($Phase -eq 'PrepareCore'){
    & "$PSScriptRoot/test-core.ps1" -PrepareOnly -BundleRoot "$parallel/core-bundle"
    & python "$PSScriptRoot/release_artifacts.py" pack "$parallel/core-bundle.tar" --root $parallel --paths core-bundle
    if($LASTEXITCODE){throw 'Cannot archive prepared core tests.'}
    return
}
if($Phase -eq 'Seal'){
    foreach($arch in 'x64','x86'){
        $receipt=Get-Content "$parallel/build-$arch.json" -Raw | ConvertFrom-Json -AsHashtable
        if($receipt.schema -ne 1 -or $receipt.architecture -cne $arch -or $receipt.source.commit -cne $identity.commit -or
            $receipt.source.version -cne $identity.version -or !$receipt.source.clean -or $receipt.run_id -cne $env:GITHUB_RUN_ID){throw 'Build artifacts belong to another commit/version/run.'}
        $files=Get-RelativeFileEntries -Root $root -Paths @($receipt.files.Keys) -RequirePaths
        Assert-ReleaseFileMap $receipt.files $files "Build $arch artifact"
    }
    $snapshot=New-ReleaseSnapshot $root ([guid]::NewGuid().ToString('N')) $identity "$parallel/core-bundle"
    "snapshot_id=$($snapshot.id)" >> $env:GITHUB_OUTPUT
    "snapshot_sha256=$($snapshot.sha256)" >> $env:GITHUB_OUTPUT
    & python "$PSScriptRoot/release_artifacts.py" pack "$parallel/snapshot.tar" --root "$root/build/release-snapshots" --paths $snapshot.id
    if($LASTEXITCODE){throw 'Cannot archive sealed snapshot.'}
    return
}
$snapshot=Get-ReleaseSnapshot $root $SnapshotId $SnapshotHash -VerifyFiles
if($snapshot.source.commit -cne $identity.commit -or $snapshot.source.version -cne $identity.version){throw 'Downloaded snapshot does not match the checked out source.'}
$evidence=Join-Path $root "build/reports/release-parallel-$SnapshotId"
if($Phase -eq 'Verify'){
    & "$PSScriptRoot/test.ps1" -Level RELEASE -BaseRef HEAD -CiSafe -ReleaseVerify -Architecture $Architecture -SnapshotId $SnapshotId -SnapshotHash $SnapshotHash -ReportDirectory "$evidence/verify-$Architecture"
    & python "$PSScriptRoot/release_artifacts.py" pack "$parallel/verify-$Architecture.tar" --root $root --paths "build/reports/release-parallel-$SnapshotId/verify-$Architecture"
    if($LASTEXITCODE){throw 'Cannot archive architecture verification evidence.'}
    return
}
if($Phase -eq 'Shard'){
    & "$PSScriptRoot/test-core.ps1" -Shard $Shard -SnapshotId $SnapshotId -SnapshotHash $SnapshotHash -ReceiptRoot "$evidence/core/$Shard"
    & python "$PSScriptRoot/release_artifacts.py" pack "$parallel/core-$Shard.tar" --root $root --paths "build/reports/release-parallel-$SnapshotId/core/$Shard"
    if($LASTEXITCODE){throw 'Cannot archive core shard evidence.'}
    return
}

# Aggregate and package use fresh CTest inventories from the same source. The
# original CTest rows, fixtures and parameterized assertions remain authoritative.
foreach($preset in 'windows-x64','windows-x86'){
    & cmake --preset $preset "-DSHANJIE_RELEASE_NATIVE_ROOT=$($snapshot.root)/native"
    if($LASTEXITCODE){throw "Cannot configure the $preset release inventory."}
}
$env:SHANJIE_RELEASE_SNAPSHOT_ID=$snapshot.id
$env:SHANJIE_RELEASE_SNAPSHOT_SHA256=$snapshot.sha256
$env:SHANJIE_TEST_RUN_ID=$snapshot.id
$env:SHANJIE_TEST_LEVEL='RELEASE'
$env:SHANJIE_TEST_STOP_ON_FAILURE='1'
$env:SHANJIE_TEST_TIMINGS_DIR="$evidence/events-$Phase"
$null=New-Item -ItemType Directory -Path $evidence,$env:SHANJIE_TEST_TIMINGS_DIR -Force
$permitPath=Join-Path $evidence 'packaging-permit.json'
if($Phase -eq 'Aggregate'){
    $reports=@();$selected=@();$subcases=@();$verification=@();$omitted=@()
    foreach($arch in 'x64','x86'){
        $receipt=Get-Content "$evidence/verify-$arch/evidence.json" -Raw | ConvertFrom-Json -AsHashtable
        if($receipt.root -cne [IO.Path]::GetFullPath("$evidence/verify-$arch")){throw 'Architecture evidence was relocated outside its receipt directory.'}
        $record=Get-Content "$evidence/verify-$arch/report.json" -Raw | ConvertFrom-Json -AsHashtable
        if(!$record.release_verify -or $record.level -ne 'RELEASE' -or $record.status -ne 'passed' -or !$record.ci_safe -or
            $record.commit -cne $identity.commit -or $record.version -cne $identity.version -or $record.worktree_dirty){throw 'Architecture verification is incomplete or source identity differs.'}
        $reports+=@($record.ctest);$selected+=@($record.selection.selected);$omitted+=@($record.selection.not_selected);$subcases+=@($record.subcases | Where-Object selected)
        $verification+=@($receipt)
    }
    $env:SHANJIE_CORE_SHARD_EVIDENCE="$evidence/core"
    # This CTest command actually runs the core-suite delegate: it verifies every
    # shard receipt, log and assertion count. CTest itself writes the logical JUnit.
    $junit=Join-Path $evidence 'junit-core-shards.xml'
    $log=Join-Path $evidence 'ctest-core-shards.log'
    & ctest --test-dir "$root/build/cmake/windows-x64" -C Release -R '^full[.]core-suite$' -FS '^CoreOverlay$' --output-junit $junit --output-log $log --output-on-failure --stop-on-failure --no-tests=error
    if($LASTEXITCODE){throw 'Core shard evidence aggregation failed.'}
    $reports+=@(@{preset='windows-x64';attempt=1;junit=$junit;log=$log;snapshot_id=$snapshot.id;snapshot_sha256=$snapshot.sha256})
    $selected+=@(@{preset='windows-x64';name='full.core-suite'})
    $permitRecord=@{schema=4;level='RELEASE';status='tests-passed';ci_safe=$true;commit=$identity.commit;version=$identity.version;worktree_dirty=$false;
        ctest=$reports;results=@(Read-CtestResults $reports);selection=@{selected=$selected;not_selected=$omitted};subcases=$subcases;
        parallel=@{run_id=$env:GITHUB_RUN_ID;core_evidence="$evidence/core";verification=$verification};
        release=@{source_start=$identity;source_end=$identity;architectures=@('x64','x86');candidate_id=$snapshot.id;snapshot_sha256=$snapshot.sha256}}
    $null=New-ReleasePackagingPermit $root $permitPath $permitRecord
    & python "$PSScriptRoot/release_artifacts.py" pack "$parallel/evidence.tar" --root $root --paths "build/reports/release-parallel-$SnapshotId"
    if($LASTEXITCODE){throw 'Cannot archive accepted release evidence.'}
    return
}

$permit=Assert-ReleasePackagingPermit $root $permitPath $snapshot.id $snapshot.sha256
$env:SHANJIE_RELEASE_PACKAGING_PERMIT=$permit.path
$junit=Join-Path $evidence 'junit-package.xml'
$log=Join-Path $evidence 'ctest-package.log'
& ctest --test-dir "$root/build/cmake/windows-x64" -C Release -R '^release[.]package$' --output-junit $junit --output-log $log --output-on-failure --stop-on-failure --no-tests=error
if($LASTEXITCODE){throw 'Release package integrity verification failed.'}
$record=Get-Content $permitPath -Raw | ConvertFrom-Json -AsHashtable
$record.status='passed'
$record.ctest+=@(@{preset='windows-x64';attempt=1;junit=$junit;log=$log;snapshot_id=$snapshot.id;snapshot_sha256=$snapshot.sha256})
$record.results=@(Read-CtestResults $record.ctest)
$record.selection.selected+=@(@{preset='windows-x64';name='release.package'})
$record.release.packaging_permit=$permit.path
$record.release.artifacts=Get-ReleaseArtifacts $root $snapshot.id $record.ctest $permit.path
$report=Join-Path $evidence 'report.json'
$record | ConvertTo-Json -Depth 20 | Set-Content $report -Encoding utf8NoBOM
& "$PSScriptRoot/package.ps1" -ExportCiArtifact -ReportPath $report
python "$PSScriptRoot/release.py" manifest --platform windows --folder dist
if($LASTEXITCODE){throw 'Release artifact manifest failed.'}
