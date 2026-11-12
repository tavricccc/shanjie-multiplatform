#Requires -Version 7
# Builds the English word list exactly as upstream does: the pinned SymSpell input (hash-checked),
# upstream's own generator, and the output hash upstream recorded in v0.6.0 (0.6.1 dropped the list; Windows mixed input keeps it). Nothing here is committed; build/ is generated.
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
Import-Module "$PSScriptRoot/test-support.psm1" -ErrorAction Stop
$upstream = Join-Path $root 'upstream/shanjie'
$tool = Join-Path $upstream 'tools/build_english_words.py'
$python = if (Get-Command python -ErrorAction SilentlyContinue) { 'python' } else { 'python3' }
$url = (& $python $tool --print-url).Trim()
$inputSha = (& $python $tool --print-input-sha256).Trim()
$source = Join-Path $root 'build/english/frequency_dictionary_en_82_765.txt'
$output = Join-Path $root 'build/english-words.txt'
$recorded = '3352d0505002181cd88ba2e547ee1a86a132f915ff030a4f8e53105e10874c01'
$pin = (Get-Content (Join-Path $root 'UPSTREAM.json') -Raw | ConvertFrom-Json).commit
$pythonVersion = (& $python --version 2>&1 | Select-Object -First 1).ToString().Trim()
$sourceKey = Get-ContentFingerprint -Root $root -Paths @('UPSTREAM.json') -Values @($pin,$inputSha,$recorded,$pythonVersion,(Get-FileHash -LiteralPath $tool -Algorithm SHA256).Hash.ToLowerInvariant())
$cacheFile = Join-Path $root 'build/english-words.cache.json'
$cacheStage=Start-TestStage -Name 'Validate generated English word-list cache' -Category 'prepare.cache'
$cacheHit=$false
try {
if ((Test-Path -LiteralPath $output) -and (Get-FileHash -LiteralPath $output -Algorithm SHA256).Hash.ToLowerInvariant() -eq $recorded) {
    try { $cached = Get-Content -LiteralPath $cacheFile -Raw | ConvertFrom-Json }
    catch { $cached = $null }
    $cacheHit = $cached.key -eq $sourceKey
}
Complete-TestStage -Stage $cacheStage -Details @{cache=if($cacheHit){'hit'}else{'miss'};upstream_commit=$pin}
} catch {
    Complete-TestStage -Stage $cacheStage -Status 'failed' -Details @{cache='miss';upstream_commit=$pin} -ErrorMessage $_.Exception.Message
    throw
}
if($cacheHit){Write-Host 'English word-list cache hit (pinned output hash verified).';Write-Output $output;return}
New-Item -ItemType Directory -Path (Split-Path $source) -Force | Out-Null
if (!(Test-Path $source) -or (Get-FileHash $source -Algorithm SHA256).Hash.ToLowerInvariant() -ne $inputSha) {
    Invoke-Timed -Name 'Download and validate English frequency source' -Category 'prepare.download' -Details @{upstream_commit=$pin;expected_sha256=$inputSha} -Action {
        Invoke-WebRequest $url -OutFile "$source.part"
        Move-Item "$source.part" $source -Force
        if ((Get-FileHash $source -Algorithm SHA256).Hash.ToLowerInvariant() -ne $inputSha) { Remove-Item $source; throw "English word list input does not match SHA-256 $inputSha." }
    }
}
Invoke-Timed -Name 'Generate pinned English words' -Category 'prepare.data' -Details @{ cache='miss'; upstream_commit=$pin } -Action {
    & $python $tool --input $source --out $output | Out-Null
    if ($LASTEXITCODE) { throw 'tools/build_english_words.py failed.' }
}
$actual = (Get-FileHash $output -Algorithm SHA256).Hash.ToLowerInvariant()
if ($actual -ne $recorded) { throw "Generated english-words.txt ($actual) does not match the pinned hash ($recorded)." }
$manifest = [ordered]@{ schema=1; key=$sourceKey; sha256=$actual; upstream_commit=$pin; tool_sha256=(Get-FileHash -LiteralPath $tool -Algorithm SHA256).Hash.ToLowerInvariant() }
[IO.File]::WriteAllText($cacheFile,($manifest|ConvertTo-Json -Depth 4),[Text.UTF8Encoding]::new($false))
Write-Output $output
