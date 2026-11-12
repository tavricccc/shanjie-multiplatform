#Requires -Version 7
# The runtime data besides the models, the same files upstream's build-app.sh puts in the app.
param([string]$Output = 'dist/data', [string]$Licenses = "$Output/LICENSES")
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
Import-Module "$PSScriptRoot/test-support.psm1" -ErrorAction Stop
. "$PSScriptRoot/preserve-output.ps1"
. "$PSScriptRoot/export-upstream.ps1"
$null = Initialize-TestTimings -ProjectRoot $root -Name 'prepare-data'
$upstream = Join-Path $root 'upstream/shanjie'
$pin = (Get-Content -LiteralPath "$root/UPSTREAM.json" -Raw | ConvertFrom-Json).commit
$data = Join-Path $root $Output
$licenseDir = Join-Path $root $Licenses
$attribution = Join-Path $root "$Output/models/builtin/colloquial-attribution.tsv"
if (!(Test-Path -LiteralPath $attribution)) { throw 'Run prepare-models.ps1 before prepare-data.ps1.' }

$lexiconNames = @(Get-ChildItem -LiteralPath "$upstream/data/lexicon" -File | ForEach-Object Name)
$expectedRootNames=@($lexiconNames)+@('pinyin-map.tsv')
$existingRootNames=if(Test-Path -LiteralPath $data){@(Get-ChildItem -LiteralPath $data -File | ForEach-Object Name)}else{@()}
$outputPaths = @("$Output/packs",$Licenses) + @($expectedRootNames | ForEach-Object { "$Output/$_" }) + @($existingRootNames | ForEach-Object { "$Output/$_" })
$inputPaths = @(
    'UPSTREAM.json','MODEL-SOURCES.json','scripts/prepare-data.ps1','scripts/export-upstream.ps1',
    'data/pinyin-map.tsv','data/README.md','LICENSES','upstream/shanjie/scripts/build-app.sh','upstream/shanjie/tools/build_english_words.py'
)
$attributionHash = (Get-FileHash -LiteralPath $attribution -Algorithm SHA256).Hash.ToLowerInvariant()
$inputKey = Get-ContentFingerprint -Root $root -Paths $inputPaths -Values @($pin,$Output,$Licenses,$attributionHash)
$outputManifest = Join-Path $root 'build/data-output-cache.json'
$stage = Start-TestStage -Name 'Prepare runtime data and licenses' -Category 'data.prepare'
try {
    if (Test-ContentCache -Root $root -ManifestPath $outputManifest -Key $inputKey -Paths $outputPaths) {
        Complete-TestStage -Stage $stage -Status 'passed' -Details @{ cache='hit'; upstream_commit=$pin; files=$outputPaths.Count }
        Write-Host "Runtime-data cache hit: upstream $pin, output hashes verified."
        return
    }

    $exportRoot = Join-Path $root "build/upstream-data/$pin"
    $exportKey = Get-ContentFingerprint -Root $root -Paths @('UPSTREAM.json','scripts/export-upstream.ps1') -Values @($pin,'data/lexicon','data/packs')
    $exportManifest = Join-Path $exportRoot '.source-cache.json'
    $exportPaths = @('data/lexicon','data/packs')
    if (!(Test-ContentCache -Root $exportRoot -ManifestPath $exportManifest -Key $exportKey -Paths $exportPaths)) {
        if (Test-Path -LiteralPath $exportRoot) { Preserve-Output $exportRoot }
        New-Item -ItemType Directory -Path $exportRoot -Force | Out-Null
        Invoke-Timed -Name 'Export runtime data from pinned upstream' -Category 'data.export' -Details @{ cache='miss'; upstream_commit=$pin } -Action {
            Export-Upstream $exportRoot @('data/lexicon','data/packs')
        }
        Save-ContentCache -Root $exportRoot -ManifestPath $exportManifest -Key $exportKey -Paths $exportPaths
    } else { Write-Host "Upstream runtime-data export cache hit: $pin, contents verified." }

    Invoke-Timed -Name 'Copy runtime data and licenses' -Category 'data.copy' -Details @{upstream_commit=$pin;files=$outputPaths.Count} -Action {
        # Rebuilt when inputs change so notices for data no longer shipped never remain behind.
        Preserve-Output $licenseDir
        New-Item -ItemType Directory -Path "$data/packs",$licenseDir -Force | Out-Null
        foreach($stale in $existingRootNames | Where-Object { $_ -notin $expectedRootNames }) { Preserve-Output (Join-Path $data $stale) }
        Copy-Item "$exportRoot/data/lexicon/*" $data -Force
        Copy-Item "$root/data/pinyin-map.tsv" $data -Force
        foreach ($pack in @('acg','it','meme','beauty')) {
            Copy-Item "$exportRoot/data/packs/$pack-add.tsv","$exportRoot/data/packs/$pack-sources.tsv","$exportRoot/data/packs/$pack.json" "$data/packs" -Force
        }
        Copy-Item "$exportRoot/data/packs/quotes-add.tsv","$exportRoot/data/packs/quotes.json" "$data/packs" -Force
        # The English list of Windows mixed input is compiled into the core; its notices live in this repo since upstream 0.6.1 dropped the list.
        Copy-Item "$upstream/LICENSE","$upstream/LICENSES/*","$root/LICENSES/windows-models.md","$root/LICENSES/mode-icons.md","$root/LICENSES/octicons.txt","$root/LICENSES/english-words-attribution.txt","$root/LICENSES/SCOWL.txt","$root/LICENSES/SymSpell-MIT.txt" $licenseDir -Force
        Copy-Item "$root/data/README.md" (Join-Path $licenseDir 'pinyin-data.md') -Force
        # The two notices upstream's build-app.sh writes into the app, taken from that script so they follow it.
        $buildApp = [IO.File]::ReadAllText("$upstream/scripts/build-app.sh").Replace("`r`n","`n")
        $symspell = [regex]::Match([IO.File]::ReadAllText("$upstream/tools/build_english_words.py"), '(?m)^SYMSPELL_COMMIT = "([0-9a-f]{40})"').Groups[1].Value
        foreach ($notice in @('CC-BY-SA-4.0-attribution.txt')) {
            $match = [regex]::Match($buildApp, "(?ms)^cat > `"\`$RES/LICENSES/$([regex]::Escape($notice))`" <<'?EOF'?`n(.*?)`nEOF`n")
            $text = $match.Groups[1].Value.Replace('$EN_COMMIT', $symspell)
            if (!$match.Success -or !$symspell -or $text.Contains('$')) { throw "Upstream notice changed: $notice" }
            [IO.File]::WriteAllText((Join-Path $licenseDir $notice), "$text`n", [Text.UTF8Encoding]::new($false))
        }
        # The per-source attribution of 常談 ships beside the licenses, as upstream's app does.
        Copy-Item -LiteralPath $attribution -Destination $licenseDir -Force
    }
    Save-ContentCache -Root $root -ManifestPath $outputManifest -Key $inputKey -Paths $outputPaths
    Complete-TestStage -Stage $stage -Status 'passed' -Details @{ cache='miss'; upstream_commit=$pin; files=$outputPaths.Count }
} catch {
    Complete-TestStage -Stage $stage -Status 'failed' -Details @{ cache='miss'; upstream_commit=$pin } -ErrorMessage $_.Exception.Message
    throw
}
