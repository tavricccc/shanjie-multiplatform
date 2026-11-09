#Requires -Version 7
# The built-in model bundle: upstream's whole released set, the main model mapped to .winmap.
param([string]$Output = 'dist/data')
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
Import-Module "$PSScriptRoot/test-support.psm1" -ErrorAction Stop
. "$PSScriptRoot/preserve-output.ps1"
$null = Initialize-TestTimings -ProjectRoot $root -Name 'prepare-models'
$manifest = [IO.File]::ReadAllText((Join-Path $root 'MODEL-SOURCES.json')) | ConvertFrom-Json
if ($manifest.format -ne 2) { throw 'MODEL-SOURCES.json must be format 2.' }
$cache = Join-Path $root 'build/model'
New-Item -ItemType Directory -Path $cache -Force | Out-Null
$bundle = $manifest.builtin
# A separately updated release records the checksum from the pinned core as pinned_sha256.
foreach ($asset in $bundle.assets) {
    $record = Join-Path $root "upstream/shanjie/data/$($asset.filename).sha256"
    if (!(Test-Path -LiteralPath $record)) { throw "Upstream does not ship $($asset.filename); review MODEL-SOURCES.json." }
    if (((Get-Content -LiteralPath $record -Raw) -split '\s+')[0] -ne ($asset.pinned_sha256 ?? $asset.sha256)) { throw "$($asset.filename) differs from the pinned upstream; update MODEL-SOURCES.json." }
}
function Get-VerifiedAsset($asset) {
    # Releases reuse file names (every main model is bigram.sjlm), so the cache key carries the release.
    $source = Join-Path $cache "$($asset.release)-$($asset.filename)"
    if (!(Test-Path -LiteralPath $source) -or (Get-FileHash -LiteralPath $source).Hash.ToLowerInvariant() -ne $asset.sha256) {
        if ($asset.source) {
            $local = Join-Path $root $asset.source
            if ((Get-Item -LiteralPath $local).Length -ne $asset.size -or (Get-FileHash -LiteralPath $local).Hash.ToLowerInvariant() -ne $asset.sha256) { throw "Bundled model source differs: $($asset.source)" }
            Copy-Item -LiteralPath $local -Destination $source -Force
        } else {
            & "$PSScriptRoot/download-asset.ps1" -Url "$($manifest.repository)/releases/download/$($asset.release)/$($asset.filename)" -Destination $source -Size $asset.size -Sha256 $asset.sha256
        }
    }
    return $source
}
$models = Join-Path $root "$Output/models"
$folder = Join-Path $root "$Output/$($bundle.folder)"
$sources = @{}
foreach ($asset in $bundle.assets) {
    $sources[$asset.role] = Invoke-Timed -Name "Verify model source $($asset.role)" -Category 'models.source' -Details @{model_id=$bundle.id;role=$asset.role;release=$asset.release} -Action { Get-VerifiedAsset $asset }
}
$main = @($bundle.assets | Where-Object role -eq 'bigram')
if ($main.Count -ne 1) { throw 'The built-in bundle needs exactly one main model.' }
$map = Join-Path $folder $main[0].mapped
$classes = '-'
$pin = (Get-Content -LiteralPath "$root/UPSTREAM.json" -Raw | ConvertFrom-Json).commit
$rustVersion = (& rustc --version --verbose 2>&1 | Out-String).Trim()
$cargoVersion = (& cargo --version 2>&1 | Out-String).Trim()
$sourceHashes = foreach ($asset in $bundle.assets) {
    # Get-VerifiedAsset has just checked this exact SHA-256; reuse it instead of hashing a large model twice.
    "$($asset.role)=$($asset.sha256.ToLowerInvariant())"
}
$cachePaths = @("$Output/$($bundle.folder)")
$cacheManifest = Join-Path $cache "$($bundle.id)-map-cache.json"
$cacheInputs = @('MODEL-SOURCES.json','UPSTREAM.json','scripts/prepare-models.ps1','scripts/test-support.psm1','build/core/src','build/core/Cargo.toml')
$cacheValues = @($pin,$bundle.id,$Output,'release','map-model --verify-equivalence; map-model --check',$rustVersion,$cargoVersion,$env:RUSTFLAGS,$env:CARGO_ENCODED_RUSTFLAGS)+@($sourceHashes)
$cacheKey = Get-ContentFingerprint -Root $root -Paths $cacheInputs -Values $cacheValues
if (Test-ContentCache -Root $root -ManifestPath $cacheManifest -Key $cacheKey -Paths $cachePaths) {
    Write-Host "Built-in model cache hit: $($bundle.id), inputs and output hashes verified."
    Invoke-Timed -Name 'Validate built-in model cache' -Category 'models.prepare' -Details @{ cache='hit'; model_id=$bundle.id; upstream_commit=$pin } -Action { $true | Out-Null }
} else {
    $modelStage = Start-TestStage -Name 'Prepare built-in model set' -Category 'models.prepare'
    try {
        Preserve-Output $folder
        New-Item -ItemType Directory -Path $folder -Force | Out-Null
        if ($sources.classes) {
            $classes = Join-Path $folder 'classes.sjc'
            Invoke-Timed -Name 'Copy built-in model class table' -Category 'models.copy' -Details @{model_id=$bundle.id;role='classes'} -Action { Copy-Item -LiteralPath $sources.classes -Destination $classes -Force }
        }
        # Map with only the class table beside it, then add the side files and validate the whole set once.
        Invoke-Timed -Name 'Convert built-in model' -Category 'models.convert' -Details @{ cache='miss'; model_id=$bundle.id; input_sha256=$sourceHashes[0] } -Action {
            & cargo run --release --quiet --manifest-path "$root/build/core/Cargo.toml" --bin map-model -- $sources.bigram $map $classes
            if ($LASTEXITCODE) { throw 'Mapped model equivalence verification failed.' }
        }
        Invoke-Timed -Name 'Copy built-in model side files' -Category 'models.copy' -Details @{model_id=$bundle.id;roles=@($bundle.assets | Where-Object { $_.role -notin @('bigram','classes') } | ForEach-Object role)} -Action {
            foreach ($asset in $bundle.assets | Where-Object { $_.role -notin @('bigram','classes') }) {
                Copy-Item -LiteralPath $sources[$asset.role] -Destination (Join-Path $folder $asset.filename) -Force
            }
        }
        Invoke-Timed -Name 'Check built-in model side files' -Category 'models.validate' -Details @{ cache='miss'; model_id=$bundle.id } -Action {
            & cargo run --release --quiet --manifest-path "$root/build/core/Cargo.toml" --bin map-model -- --check $map
            if ($LASTEXITCODE) { throw 'The built-in side files do not pair with the model.' }
        }
        Save-ContentCache -Root $root -ManifestPath $cacheManifest -Key $cacheKey -Paths $cachePaths
        Complete-TestStage -Stage $modelStage -Status 'passed' -Details @{ cache='miss'; model_id=$bundle.id; upstream_commit=$pin }
    } catch {
        Complete-TestStage -Stage $modelStage -Status 'failed' -Details @{ cache='miss'; model_id=$bundle.id; upstream_commit=$pin } -ErrorMessage $_.Exception.Message
        throw
    }
}
# Only the built-in folder belongs in the package; earlier layouts are removed from the output.
foreach ($retired in @(Get-ChildItem -LiteralPath $models -Directory | Where-Object { $_.FullName -ne (Get-Item $folder).FullName })) {
    if ($retired.Name -ne '.deleted_files') { Preserve-Output $retired.FullName }
}
Write-Output "prepared $($bundle.folder)"
