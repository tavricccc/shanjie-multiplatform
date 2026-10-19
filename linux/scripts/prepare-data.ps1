#Requires -Version 7
# The Linux runtime data: the same files as the Windows package's data folder (scripts/build.ps1 and
# scripts/prepare-models.ps1), so both platforms decode with the same lexicon, mapped model and word lists.
param([string]$Output = 'build/linux/data')
$ErrorActionPreference = 'Stop'
$root = Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
if ([IO.Path]::IsPathRooted($Output) -or $Output.Split('/') -contains '..') { throw 'Output must be a relative path inside the repository.' }
$target = [IO.Path]::GetFullPath((Join-Path $root $Output))
$stage = "$target.stage"
if (Test-Path -LiteralPath $stage) { Remove-Item -LiteralPath $stage -Recurse -Force }
New-Item -ItemType Directory -Path "$stage/LICENSES","$root/build/model" -Force | Out-Null
$manifest = [IO.File]::ReadAllText((Join-Path $root 'MODEL-SOURCES.json')) | ConvertFrom-Json
function Get-VerifiedAsset($asset,[string]$name) {
    $source = Join-Path $root "build/model/$($asset.source)"
    if (!(Test-Path -LiteralPath $source) -or (Get-FileHash -LiteralPath $source).Hash.ToLowerInvariant() -ne $asset.sha256) {
        & "$root/scripts/download-asset.ps1" -Url "$($manifest.repository)/releases/download/$($asset.release)/$name" -Destination $source -Size $asset.size -Sha256 $asset.sha256
    }
    return $source
}
foreach ($entry in $manifest.models) {
    $source = Get-VerifiedAsset $entry 'bigram.sjlm'
    $mapped = Join-Path $stage $entry.mapped
    New-Item -ItemType Directory -Path (Split-Path $mapped -Parent) -Force | Out-Null
    $classes = '-'
    if ($entry.classes) {
        $classes = Get-VerifiedAsset $entry.classes 'classes.sjc'
        Copy-Item -LiteralPath $classes -Destination (Join-Path (Split-Path $mapped -Parent) 'classes.sjc') -Force
    }
    cargo run --release --quiet --manifest-path "$root/build/core/Cargo.toml" --bin map-model -- $source $mapped $classes
    if ($LASTEXITCODE) { throw "Mapped model equivalence verification failed: $($entry.release)" }
}
Copy-Item "$root/upstream/shanjie/data/lexicon/*" $stage -Force
Copy-Item "$root/data/pinyin-map.tsv" $stage -Force
# Generated and hash-verified against upstream's recorded output (LF only, as its strict reader requires).
& "$root/scripts/prepare-english.ps1" | Out-Null
Copy-Item "$root/build/english-words.txt" "$stage/english-words.txt" -Force
Copy-Item "$root/upstream/shanjie/LICENSE","$root/upstream/shanjie/LICENSES/*","$root/LICENSES/windows-models.md" "$stage/LICENSES" -Force
Copy-Item "$root/data/README.md" "$stage/LICENSES/pinyin-data.md" -Force
if (Test-Path -LiteralPath $target) { Remove-Item -LiteralPath $target -Recurse -Force }
Move-Item -LiteralPath $stage -Destination $target
Write-Output "prepared $Output"
