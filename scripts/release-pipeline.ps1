#Requires -Version 7
param(
    [Parameter(Mandatory)][ValidateSet('Build','PrepareCore','Seal','Verify','Shard','Aggregate','Package')][string]$Phase,
    [ValidateSet('x64','x86')][string]$Architecture='x64',
    [ValidatePattern('^(bootstrap|x(64|86)-[1-3]|core-[1-6])$')][string]$Shard,
    [string]$SnapshotId,
    [string]$SnapshotHash,
    [string]$ReuseSourceCommit,
    [switch]$CargoVerbose
)
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
Import-Module "$PSScriptRoot/release-support.psm1" -ErrorAction Stop
Import-Module "$PSScriptRoot/core-shards.psm1" -ErrorAction Stop
Import-Module "$PSScriptRoot/test-support.psm1" -ErrorAction Stop
Import-Module "$PSScriptRoot/release-ci.psm1" -ErrorAction Stop
$parallel=Join-Path $root 'build/parallel'
$null=New-Item -ItemType Directory -Path $parallel -Force
$identity=Get-ReleaseIdentity $root
if(!$identity.clean){throw 'Parallel RELEASE requires a clean source checkout.'}

if($Phase -eq 'Build'){
    & "$PSScriptRoot/build.ps1" -Architecture $Architecture -PackageTargetsOnly -SkipSharedData:($Architecture -eq 'x86') -CargoVerbose:$CargoVerbose
    if($Architecture -eq 'x64'){& "$PSScriptRoot/prepare-licenses.ps1"}
    $triple=if($Architecture -eq 'x64'){'x86_64-pc-windows-msvc'}else{'i686-pc-windows-msvc'}
    $paths=@(if($Architecture -eq 'x64'){'dist'}else{'dist/x86'})+@("windows/target/$triple/release/shanjie_windows.lib","windows/target/$triple/release/shanjie-reliability-check.exe")
    @{schema=1;architecture=$Architecture;source=$identity;run_id=$env:GITHUB_RUN_ID;run_attempt=$env:GITHUB_RUN_ATTEMPT;files=(Get-RelativeFileEntries -Root $root -Paths $paths -RequirePaths)} |
        ConvertTo-Json -Depth 10 | Set-Content "$parallel/build-$Architecture.json" -Encoding utf8NoBOM
    & python "$PSScriptRoot/release_artifacts.py" pack "$parallel/build-$Architecture.tar" --root $root --paths @paths "build/parallel/build-$Architecture.json"
    if($LASTEXITCODE){throw 'Cannot archive release build inputs.'}
    if($env:SHANJIE_CARGO_CACHE_CONTEXT){
        & "$PSScriptRoot/cargo-input-cache.ps1" -Operation Save -Architecture $Architecture -Context $env:SHANJIE_CARGO_CACHE_CONTEXT
    }
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
            $receipt.source.version -cne $identity.version -or !$receipt.source.clean -or $receipt.run_id -cne $env:GITHUB_RUN_ID -or $receipt.run_attempt -cne $env:GITHUB_RUN_ATTEMPT){throw 'Build artifacts belong to another commit/version/run.'}
        $files=Get-RelativeFileEntries -Root $root -Paths @($receipt.files.Keys) -RequirePaths
        Assert-ReleaseFileMap $receipt.files $files "Build $arch artifact"
    }
    $reuseRoot=$null
    if($ReuseSourceCommit -and $env:GITHUB_EVENT_NAME -eq 'push' -and $env:GITHUB_REF -eq 'refs/heads/main'){
        & python "$PSScriptRoot/release_evidence.py" prepare --root $root --before $ReuseSourceCommit --bundle "$parallel/core-bundle" --output "$parallel/ci-reuse"
        if($LASTEXITCODE){throw 'Cannot evaluate the optional Release reuse source.'}
        if(Test-Path "$parallel/ci-reuse/source.json"){$reuseRoot="$parallel/ci-reuse"}
    }
    $snapshotId=[guid]::NewGuid().ToString('N')
    $plan=New-CiPlan $root $identity $snapshotId "$parallel/core-bundle" $reuseRoot $parallel
    $snapshot=New-ReleaseSnapshot $root $snapshotId $identity "$parallel/core-bundle" $reuseRoot "$parallel/ci-plan.json"
    # FAST must succeed before the matrix expands. Keeping the x64 mixed tests
    # here reuses the exact Cargo test compilation without transferring binaries.
    $bootstrap="build/reports/release-parallel-$snapshotId/verify-bootstrap"
    & "$PSScriptRoot/test.ps1" -Level RELEASE -BaseRef HEAD -CiSafe -ReleaseVerify -Architecture x64 -SnapshotId $snapshot.id -SnapshotHash $snapshot.sha256 -ReportDirectory "$root/$bootstrap" -CiPartition bootstrap
    & python "$PSScriptRoot/release_artifacts.py" pack "$parallel/verify-bootstrap.tar" --root $root --paths $bootstrap
    if($LASTEXITCODE){throw 'Cannot archive the FAST prerequisite evidence.'}
    & python "$PSScriptRoot/release_artifacts.py" pack "$parallel/snapshot.tar" --root "$root/build/release-snapshots" --paths $snapshot.id
    if($LASTEXITCODE){throw 'Cannot archive sealed snapshot.'}
    "snapshot_id=$($snapshot.id)" >> $env:GITHUB_OUTPUT
    "snapshot_sha256=$($snapshot.sha256)" >> $env:GITHUB_OUTPUT
    "matrix=$(@{include=@($plan.matrix)}|ConvertTo-Json -Depth 10 -Compress)" >> $env:GITHUB_OUTPUT
    return
}
$snapshot=Get-ReleaseSnapshot $root $SnapshotId $SnapshotHash -VerifyFiles
if($snapshot.source.commit -cne $identity.commit -or $snapshot.source.version -cne $identity.version){throw 'Downloaded snapshot does not match the checked out source.'}
$plan=Get-Content "$($snapshot.root)/ci-plan.json" -Raw|ConvertFrom-Json -AsHashtable
if($plan.source.run_id -cne $env:GITHUB_RUN_ID -or $plan.source.run_attempt -cne $env:GITHUB_RUN_ATTEMPT){throw 'The sealed plan belongs to a different run or attempt.'}
$evidence=Join-Path $root "build/reports/release-parallel-$SnapshotId"
if($Phase -eq 'Verify'){
    & "$PSScriptRoot/test.ps1" -Level RELEASE -BaseRef HEAD -CiSafe -ReleaseVerify -Architecture $Architecture -SnapshotId $SnapshotId -SnapshotHash $SnapshotHash -ReportDirectory "$evidence/verify-$Shard" -CiPartition $Shard
    & python "$PSScriptRoot/release_artifacts.py" pack "$parallel/verify-$Shard.tar" --root $root --paths "build/reports/release-parallel-$SnapshotId/verify-$Shard"
    if($LASTEXITCODE){throw 'Cannot archive architecture partition evidence.'}
    return
}
if($Phase -eq 'Shard'){
    $assignment=@($plan.core|Where-Object id -CEQ $Shard)
    if($assignment.Count -ne 1 -or !$assignment[0].tests.Count){throw 'Core partition has no sealed test assignment.'}
    Invoke-CoreTestShard $root $snapshot $Shard "$evidence/core/$Shard" $assignment[0].tests
    & python "$PSScriptRoot/release_artifacts.py" pack "$parallel/$Shard.tar" --root $root --paths "build/reports/release-parallel-$SnapshotId/core/$Shard"
    if($LASTEXITCODE){throw 'Cannot archive Core partition evidence.'}
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
    $env:SHANJIE_CORE_SHARD_EVIDENCE="$evidence/core"
    # JUnit here records the live Core coverage check. Per-binary reuse stays
    # separately identified in the schema 6 ledger and original source ZIP.
    $junit=Join-Path $evidence 'junit-core-shards.xml'
    $log=Join-Path $evidence 'ctest-core-shards.log'
    & ctest --test-dir "$root/build/cmake/windows-x64" -C Release -R '^full[.]core-suite$' -FS '^CoreOverlay$' --output-junit $junit --output-log $log --output-on-failure --stop-on-failure --no-tests=error
    if($LASTEXITCODE){throw 'Core shard evidence aggregation failed.'}
    & python "$PSScriptRoot/release_ci.py" aggregate --root $root --snapshot-root $snapshot.root --snapshot-hash $snapshot.sha256 --evidence-root $evidence --output "$parallel/ci-aggregate.json"
    if($LASTEXITCODE){throw 'Cannot assemble the CI partition ledger.'}
    $record=Get-Content "$parallel/ci-aggregate.json" -Raw|ConvertFrom-Json -AsHashtable
    $null=New-ReleasePackagingPermit $root $permitPath $record
    "run_full_tests=$(($record.evidence_mode -ceq 'executed').ToString().ToLowerInvariant())" >> $env:GITHUB_OUTPUT
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
$record.results+=@(@{preset='windows-x64';name='release.package';status='passed';evidence_mode='executed'})
$record.selection.selected+=@(@{preset='windows-x64';name='release.package'})
$record.release.packaging_permit=$permit.path
$record.release.artifacts=Get-ReleaseArtifacts $root $snapshot.id $record.ctest $permit.path
$report=Join-Path $evidence 'report.json'
$record | ConvertTo-Json -Depth 35 | Set-Content $report -Encoding utf8NoBOM
& "$PSScriptRoot/package.ps1" -ExportCiArtifact -ReportPath $report
python "$PSScriptRoot/release.py" manifest --platform windows --folder dist
if($LASTEXITCODE){throw 'Release artifact manifest failed.'}
if($env:GITHUB_EVENT_NAME -eq 'push' -and $env:GITHUB_REF -eq 'refs/heads/main' -and $record.evidence_mode -ceq 'executed'){
    & python "$PSScriptRoot/release_evidence.py" create --root $root --snapshot-id $SnapshotId --output "$parallel/full-attestation"
    if($LASTEXITCODE){throw 'Full Release attestation creation failed.'}
}
