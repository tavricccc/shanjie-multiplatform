#Requires -Version 7
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$manifest = [IO.File]::ReadAllText((Join-Path $root 'MODEL-SOURCES.json')) | ConvertFrom-Json
New-Item -ItemType Directory -Path "$root/build/model","$root/dist/data" -Force | Out-Null
function Get-VerifiedAsset($asset,[string]$name) {
    $source = Join-Path $root "build/model/$($asset.source)"
    if (!(Test-Path -LiteralPath $source) -or (Get-FileHash -LiteralPath $source).Hash.ToLowerInvariant() -ne $asset.sha256) {
        & "$PSScriptRoot/download-asset.ps1" -Url "$($manifest.repository)/releases/download/$($asset.release)/$name" -Destination $source -Size $asset.size -Sha256 $asset.sha256
    }
    return $source
}
foreach ($entry in $manifest.models) {
    $source = Get-VerifiedAsset $entry 'bigram.sjlm'
    $mapped = Join-Path $root "dist/data/$($entry.mapped)"
    New-Item -ItemType Directory -Path (Split-Path $mapped -Parent) -Force | Out-Null
    $classes = '-'
    if ($entry.classes) {
        $classes = Get-VerifiedAsset $entry.classes 'classes.sjc'
        Copy-Item -LiteralPath $classes -Destination (Join-Path (Split-Path $mapped -Parent) 'classes.sjc') -Force
    }
    cargo run --release --manifest-path "$root/build/core/Cargo.toml" --bin map-model -- $source $mapped $classes
    if ($LASTEXITCODE) { throw "Mapped model equivalence verification failed: $($entry.release)" }
}
# Remove retired model directories from build output as well as legacy root files.
foreach ($retired in @(Get-ChildItem -LiteralPath "$root/dist/data/models" -Directory | Where-Object Name -NotIn @($manifest.models.release))) {
    $resolved = [IO.Path]::GetFullPath($retired.FullName)
    if (!$resolved.StartsWith([IO.Path]::GetFullPath("$root/dist/data/models") + [IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) { throw 'Unexpected model path.' }
    Remove-Item -LiteralPath $resolved -Recurse -Force
}
foreach ($legacy in @('bigram.winmap','bigram-v2.winmap')) {
    $path = Join-Path $root "dist/data/$legacy"
    if (Test-Path -LiteralPath $path) { Remove-Item -LiteralPath $path -Force }
}
foreach ($retired in @('bigram.sjlm','bigram-v2.sjlm','bigram-v3.sjlm','bigram-v4.sjlm','classes-v1.sjc','classes-v2.sjc')) {
    $path = [IO.Path]::GetFullPath((Join-Path $root "build/model/$retired"))
    if (!$path.StartsWith([IO.Path]::GetFullPath("$root/build/model") + [IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) { throw 'Unexpected model cache path.' }
    if (Test-Path -LiteralPath $path) { Remove-Item -LiteralPath $path -Force }
}
