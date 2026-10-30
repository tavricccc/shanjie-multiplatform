#Requires -Version 7
# The runtime data besides the models, the same files upstream's build-app.sh puts in the app.
param([string]$Output = 'dist/data', [string]$Licenses = "$Output/LICENSES")
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$upstream = Join-Path $root 'upstream/shanjie'
$data = Join-Path $root $Output
$licenseDir = Join-Path $root $Licenses
# Rebuilt each time, so a notice for data that is no longer shipped never stays behind.
if (Test-Path -LiteralPath $licenseDir) { Remove-Item -LiteralPath $licenseDir -Recurse -Force }
New-Item -ItemType Directory -Path "$data/packs",$licenseDir -Force | Out-Null
Copy-Item "$upstream/data/lexicon/*" $data -Force
Copy-Item "$root/data/pinyin-map.tsv" $data -Force
foreach ($pack in @('acg','it')) {
    Copy-Item "$upstream/data/packs/$pack-add.tsv","$upstream/data/packs/$pack-sources.tsv","$upstream/data/packs/$pack.json" "$data/packs" -Force
}
# Generated and hash-verified against upstream's recorded output (LF only, as its strict reader requires).
& "$PSScriptRoot/prepare-english.ps1" | Out-Null
Copy-Item "$root/build/english-words.txt" $data -Force
Copy-Item "$upstream/LICENSE","$upstream/LICENSES/*","$root/LICENSES/windows-models.md","$root/LICENSES/mode-icons.md" $licenseDir -Force
Copy-Item "$root/data/README.md" (Join-Path $licenseDir 'pinyin-data.md') -Force
# The two notices upstream's build-app.sh writes into the app, taken from that script so they follow it.
$buildApp = [IO.File]::ReadAllText("$upstream/scripts/build-app.sh").Replace("`r`n","`n")
$symspell = [regex]::Match([IO.File]::ReadAllText("$upstream/tools/build_english_words.py"), '(?m)^SYMSPELL_COMMIT = "([0-9a-f]{40})"').Groups[1].Value
foreach ($notice in @('CC-BY-SA-4.0-attribution.txt','english-words-attribution.txt')) {
    $match = [regex]::Match($buildApp, "(?ms)^cat > `"\`$RES/LICENSES/$([regex]::Escape($notice))`" <<'?EOF'?`n(.*?)`nEOF`n")
    $text = $match.Groups[1].Value.Replace('$EN_COMMIT', $symspell)
    if (!$match.Success -or !$symspell -or $text.Contains('$')) { throw "Upstream notice changed: $notice" }
    [IO.File]::WriteAllText((Join-Path $licenseDir $notice), "$text`n", [Text.UTF8Encoding]::new($false))
}
# The per-source attribution of 常談 ships beside the licenses, as upstream's app does (prepare-models.ps1 runs first).
$attribution = Join-Path $data 'models/builtin/colloquial-attribution.tsv'
if (!(Test-Path -LiteralPath $attribution)) { throw 'Run prepare-models.ps1 before prepare-data.ps1.' }
Copy-Item -LiteralPath $attribution $licenseDir -Force
