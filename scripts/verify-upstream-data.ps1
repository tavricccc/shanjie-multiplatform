#Requires -Version 7
# Everything the pinned upstream ships (lexicon files, word packs, models) must ship here and be on by upstream's default.
# A new or changed upstream item fails this check until it is wired in; nothing is silently left out or switched off.
# Static checks always run; -Dist also compares the built dist/data and dist model folder with upstream (run after a build).
param([switch]$Dist)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$upstream = Join-Path $root 'upstream/shanjie/data'
$failures = [Collections.Generic.List[string]]::new()
function Fail([string]$message) { $failures.Add($message) }

# --- Models: keep every pinned model; separately updated releases record their pinned checksum. ---
$sources = Get-Content (Join-Path $root 'MODEL-SOURCES.json') -Raw | ConvertFrom-Json
$assets = @{}
foreach ($asset in $sources.builtin.assets) { $assets[$asset.filename] = ($asset.pinned_sha256 ?? $asset.sha256).ToLowerInvariant() }
$expected = @{}
foreach ($row in Get-Content (Join-Path $upstream 'models.tsv') | Where-Object { $_.Trim() }) {
    $columns = $row -split "`t"
    $expected[$columns[0]] = $columns[1].ToLowerInvariant()
}
foreach ($file in Get-ChildItem $upstream -Filter '*.sha256' -File) {
    $name = $file.Name -replace '\.sha256$', ''
    $expected[$name] = ((Get-Content $file.FullName -Raw).Trim() -split '\s+')[0].ToLowerInvariant()
}
foreach ($name in $expected.Keys | Sort-Object) {
    if (!$assets.ContainsKey($name)) { Fail "Upstream model $name is missing from MODEL-SOURCES.json (builtin)."; continue }
    if ($assets[$name] -ne $expected[$name]) { Fail "Model $name differs from upstream: MODEL-SOURCES.json $($assets[$name]), upstream $($expected[$name])." }
}

# --- Word packs: every upstream pack is shipped by prepare-data.ps1, has a profile switch, and keeps upstream's default. ---
# Upstream defaults: ACG, IT and beauty are on; quotes is off (ShanjieKit/Shell.swift).
$packs = @{ acg = @('acg_pack', '1'); it = @('it_pack', '1'); quotes = @('quote_pack', '0'); beauty = @('beauty_pack', '1') }
$upstreamPacks = Get-ChildItem (Join-Path $upstream 'packs') -Filter '*.json' -File | ForEach-Object { $_.BaseName }
foreach ($id in $upstreamPacks) { if (!$packs.ContainsKey($id)) { Fail "Upstream pack '$id' is new: ship it in prepare-data.ps1, add its switch to the profiles and shared/src/profile.rs, and list it here with upstream's default." } }
foreach ($id in $packs.Keys) { if ($id -notin $upstreamPacks) { Fail "Pack '$id' is no longer in upstream; remove it here, in the profiles and in prepare-data.ps1." } }
$prepareData = Get-Content (Join-Path $root 'scripts/prepare-data.ps1') -Raw
$profileRust = Get-Content (Join-Path $root 'shared/src/profile.rs') -Raw
foreach ($id in $upstreamPacks | Where-Object { $packs.ContainsKey($_) }) {
    $key, $default = $packs[$id]
    # The id as a quoted list item or a pack file name, so the same word in a comment does not count.
    if ($prepareData -notmatch "'$id'|/$id-add\.tsv") { Fail "prepare-data.ps1 does not ship the '$id' pack." }
    if ($profileRust -notmatch "`"$key`"") { Fail "shared/src/profile.rs has no '$key'." }
    foreach ($profile in 'windows', 'shanjie') {
        $line = Get-Content (Join-Path $root "profiles/$profile.ini") | Where-Object { $_ -match "^$key=" }
        if (!$line) { Fail "profiles/$profile.ini has no $key." }
        elseif ($line -ne "$key=$default") { Fail "profiles/$profile.ini sets $line; upstream's default is $key=$default." }
    }
}

# --- Lexicon: prepare-data.ps1 copies the whole upstream lexicon folder, not a picked list. ---
if ($prepareData -notmatch 'data/lexicon/\*') { Fail 'prepare-data.ps1 must copy every file of upstream data/lexicon (the folder, not a list).' }

# --- Built output (after a build): every upstream file is there and identical. ---
if ($Dist) {
    # Compare with the pinned Git blobs: the shipped data comes from them, and the checkout may have other line endings.
    function Same-AsUpstream([string]$built, [string]$path) { (git -C (Join-Path $root 'upstream/shanjie') rev-parse "HEAD:data/$path") -eq (git hash-object --no-filters $built) }
    $data = Join-Path $root 'dist/data'
    foreach ($file in Get-ChildItem (Join-Path $upstream 'lexicon') -File) {
        $built = Join-Path $data $file.Name
        if (!(Test-Path $built)) { Fail "dist/data is missing lexicon file $($file.Name)."; continue }
        if (!(Same-AsUpstream $built "lexicon/$($file.Name)")) { Fail "dist/data/$($file.Name) differs from upstream." }
    }
    foreach ($id in $upstreamPacks) {
        foreach ($name in "$id-add.tsv", "$id.json") {
            $built = Join-Path $data "packs/$name"
            if (!(Test-Path $built)) { Fail "dist/data/packs is missing $name."; continue }
            if (!(Same-AsUpstream $built "packs/$name")) { Fail "dist/data/packs/$name differs from upstream." }
        }
    }
    foreach ($name in $assets.Keys) {
        $built = Join-Path $data "models/builtin/$($sources.builtin.assets | Where-Object filename -eq $name | ForEach-Object { if ($_.mapped) { $_.mapped } else { $_.filename } })"
        if (!(Test-Path $built)) { Fail "dist/data/models/builtin is missing $name." }
    }
}

if ($failures.Count) { $failures | ForEach-Object { Write-Host "FAIL $_" }; throw "Upstream data coverage: $($failures.Count) problem(s)." }
Write-Host "Upstream data coverage ok: $($expected.Count) models, $($upstreamPacks.Count) packs$(if ($Dist) { ', built output compared' })."
