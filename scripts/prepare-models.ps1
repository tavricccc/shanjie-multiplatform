#Requires -Version 7
# The built-in model bundle: upstream's whole released set, the main model mapped to .winmap.
param([string]$Output = 'dist/data')
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
. "$PSScriptRoot/preserve-output.ps1"
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
foreach ($asset in $bundle.assets) { $sources[$asset.role] = Get-VerifiedAsset $asset }
$main = @($bundle.assets | Where-Object role -eq 'bigram')
if ($main.Count -ne 1) { throw 'The built-in bundle needs exactly one main model.' }
# Mapping and checking 88 MB takes most of this script. Skip it when the bundle definition, the
# mapper's source (build/core) and every output file are exactly what the last successful run left.
# The record lives in build/, never in the packaged folder.
function Get-PreparedInputs {
    $core = Get-ChildItem -LiteralPath "$root/build/core/src" -Recurse -File | Sort-Object FullName |
        ForEach-Object { "$([IO.Path]::GetRelativePath($root, $_.FullName))=$((Get-FileHash -LiteralPath $_.FullName).Hash)" }
    $text = (@(($bundle | ConvertTo-Json -Depth 8 -Compress), (Get-FileHash -LiteralPath "$root/build/core/Cargo.toml").Hash) + $core) -join "`n"
    [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($text)))
}
function Get-PreparedOutputs { if (Test-Path -LiteralPath $folder) { Get-ChildItem -LiteralPath $folder -File | Sort-Object Name | ForEach-Object { "$($_.Name)=$((Get-FileHash -LiteralPath $_.FullName).Hash)" } } }
$record = Join-Path $cache ('prepared-' + [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes([IO.Path]::GetFullPath($folder).ToLowerInvariant()))).Substring(0, 16) + '.json')
$inputs = Get-PreparedInputs
$previous = if (Test-Path -LiteralPath $record) { Get-Content -LiteralPath $record -Raw | ConvertFrom-Json } else { $null }
if ($previous -and $previous.inputs -eq $inputs -and ($previous.outputs -join "`n") -eq (@(Get-PreparedOutputs) -join "`n")) {
    Write-Host "Built-in models unchanged: $($bundle.folder)"
} else {
    Preserve-Output $folder
    New-Item -ItemType Directory -Path $folder -Force | Out-Null
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
    [IO.File]::WriteAllText($record, (@{ inputs = $inputs; outputs = @(Get-PreparedOutputs) } | ConvertTo-Json), [Text.UTF8Encoding]::new($false))
}
# Only the built-in folder belongs in the package; earlier layouts are removed from the output.
foreach ($retired in @(Get-ChildItem -LiteralPath $models -Directory | Where-Object { $_.FullName -ne (Get-Item $folder).FullName })) {
    if ($retired.Name -ne '.deleted_files') { Preserve-Output $retired.FullName }
}
Write-Output "prepared $($bundle.folder)"
