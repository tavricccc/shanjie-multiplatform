#Requires -Version 7
# Builds the English word list exactly as upstream does: the pinned SymSpell input (hash-checked),
# upstream's own generator, and the output hash upstream recorded in v0.6.0 (0.6.1 dropped the list; Windows mixed input keeps it). Nothing here is committed; build/ is generated.
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$upstream = Join-Path $root 'upstream/shanjie'
$tool = Join-Path $upstream 'tools/build_english_words.py'
$python = if (Get-Command python -ErrorAction SilentlyContinue) { 'python' } else { 'python3' }
$url = (& $python $tool --print-url).Trim()
$inputSha = (& $python $tool --print-input-sha256).Trim()
$source = Join-Path $root 'build/english/frequency_dictionary_en_82_765.txt'
$output = Join-Path $root 'build/english-words.txt'
New-Item -ItemType Directory -Path (Split-Path $source) -Force | Out-Null
if (!(Test-Path $source) -or (Get-FileHash $source -Algorithm SHA256).Hash.ToLowerInvariant() -ne $inputSha) {
    Invoke-WebRequest $url -OutFile "$source.part"
    Move-Item "$source.part" $source -Force
    if ((Get-FileHash $source -Algorithm SHA256).Hash.ToLowerInvariant() -ne $inputSha) { Remove-Item $source; throw "English word list input does not match SHA-256 $inputSha." }
}
& $python $tool --input $source --out $output | Out-Null
if ($LASTEXITCODE) { throw 'tools/build_english_words.py failed.' }
$recorded = '3352d0505002181cd88ba2e547ee1a86a132f915ff030a4f8e53105e10874c01'
$actual = (Get-FileHash $output -Algorithm SHA256).Hash.ToLowerInvariant()
if ($actual -ne $recorded) { throw "Generated english-words.txt ($actual) does not match the pinned hash ($recorded)." }
Write-Output $output
