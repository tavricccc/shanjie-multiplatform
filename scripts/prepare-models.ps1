#Requires -Version 7
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$manifest = [IO.File]::ReadAllText((Join-Path $root 'MODEL-SOURCES.json')) | ConvertFrom-Json
New-Item -ItemType Directory -Path "$root/build/model","$root/dist/data" -Force | Out-Null
function Get-VerifiedAsset($asset,[string]$name) {
    $source = Join-Path $root "build/model/$($asset.source)"
    if (!(Test-Path -LiteralPath $source) -or (Get-FileHash -LiteralPath $source).Hash.ToLowerInvariant() -ne $asset.sha256) {
        curl.exe --fail --location --silent --show-error --continue-at - --connect-timeout 20 --speed-time 30 --speed-limit 1024 --max-time 240 --retry 3 --retry-delay 2 --retry-all-errors --retry-max-time 300 --output "$source.download" "$($manifest.repository)/releases/download/$($asset.release)/$name"
        if ($LASTEXITCODE) { throw "Asset download failed: $($asset.release)" }
        if ((Get-FileHash -LiteralPath "$source.download").Hash.ToLowerInvariant() -ne $asset.sha256) { throw "Asset SHA-256 mismatch: $($asset.release)" }
        Move-Item -LiteralPath "$source.download" -Destination $source -Force
    }
    return $source
}
foreach ($model in $manifest.models) {
    $source = Get-VerifiedAsset $model 'bigram.sjlm'
    $mapped = Join-Path $root "dist/data/$($model.mapped)"
    New-Item -ItemType Directory -Path (Split-Path $mapped -Parent) -Force | Out-Null
    $classes = '-'
    if ($model.classes) {
        $classes = Get-VerifiedAsset $model.classes 'classes.sjc'
        Copy-Item -LiteralPath $classes -Destination (Join-Path (Split-Path $mapped -Parent) 'classes.sjc') -Force
    }
    cargo run --release --manifest-path "$root/build/core/Cargo.toml" --bin map-model -- $source $mapped $classes
    if ($LASTEXITCODE) { throw "Mapped model equivalence verification failed: $($model.release)" }
}
# Schema 2 kept models in the data root. Packages now have one folder per pairing.
foreach ($legacy in @('bigram.winmap','bigram-v2.winmap')) {
    $path = Join-Path $root "dist/data/$legacy"
    if (Test-Path -LiteralPath $path) { Remove-Item -LiteralPath $path -Force }
}
