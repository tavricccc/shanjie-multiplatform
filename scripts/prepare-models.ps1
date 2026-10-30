#Requires -Version 7
# The built-in model bundle: upstream's whole released set, the main model mapped to .winmap.
param([string]$Output = 'dist/data')
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$manifest = [IO.File]::ReadAllText((Join-Path $root 'MODEL-SOURCES.json')) | ConvertFrom-Json
if ($manifest.format -ne 2) { throw 'MODEL-SOURCES.json must be format 2.' }
$cache = Join-Path $root 'build/model'
New-Item -ItemType Directory -Path $cache -Force | Out-Null
$bundle = $manifest.builtin
# The built-in set must be exactly the one the pinned upstream ships (its data/<file>.sha256).
foreach ($asset in $bundle.assets) {
    $record = Join-Path $root "upstream/shanjie/data/$($asset.filename).sha256"
    if (!(Test-Path -LiteralPath $record)) { throw "Upstream does not ship $($asset.filename); review MODEL-SOURCES.json." }
    if (((Get-Content -LiteralPath $record -Raw) -split '\s+')[0] -ne $asset.sha256) { throw "$($asset.filename) differs from the pinned upstream; update MODEL-SOURCES.json." }
}
function Get-VerifiedAsset($asset) {
    # Releases reuse file names (every main model is bigram.sjlm), so the cache key carries the release.
    $source = Join-Path $cache "$($asset.release)-$($asset.filename)"
    if (!(Test-Path -LiteralPath $source) -or (Get-FileHash -LiteralPath $source).Hash.ToLowerInvariant() -ne $asset.sha256) {
        & "$PSScriptRoot/download-asset.ps1" -Url "$($manifest.repository)/releases/download/$($asset.release)/$($asset.filename)" -Destination $source -Size $asset.size -Sha256 $asset.sha256
    }
    return $source
}
$models = Join-Path $root "$Output/models"
$folder = Join-Path $root "$Output/$($bundle.folder)"
if (Test-Path -LiteralPath $folder) { Remove-Item -LiteralPath $folder -Recurse -Force }
New-Item -ItemType Directory -Path $folder -Force | Out-Null
$sources = @{}
foreach ($asset in $bundle.assets) { $sources[$asset.role] = Get-VerifiedAsset $asset }
$main = @($bundle.assets | Where-Object role -eq 'bigram')
if ($main.Count -ne 1) { throw 'The built-in bundle needs exactly one main model.' }
$map = Join-Path $folder $main[0].mapped
$classes = '-'
if ($sources.classes) { $classes = Join-Path $folder 'classes.sjc'; Copy-Item -LiteralPath $sources.classes -Destination $classes -Force }
# Map with only the class table beside it, then add the side files and load the whole set once.
cargo run --release --quiet --manifest-path "$root/build/core/Cargo.toml" --bin map-model -- $sources.bigram $map $classes
if ($LASTEXITCODE) { throw 'Mapped model equivalence verification failed.' }
foreach ($asset in $bundle.assets | Where-Object { $_.role -notin @('bigram','classes') }) {
    Copy-Item -LiteralPath $sources[$asset.role] -Destination (Join-Path $folder $asset.filename) -Force
}
cargo run --release --quiet --manifest-path "$root/build/core/Cargo.toml" --bin map-model -- --check $map
if ($LASTEXITCODE) { throw 'The built-in side files do not pair with the model.' }
# Only the built-in folder belongs in the package; earlier layouts are removed from the output.
foreach ($retired in @(Get-ChildItem -LiteralPath $models -Directory | Where-Object { $_.FullName -ne (Get-Item $folder).FullName })) {
    Remove-Item -LiteralPath $retired.FullName -Recurse -Force
}
Write-Output "prepared $($bundle.folder)"
