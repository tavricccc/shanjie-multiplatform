#Requires -Version 7
# Reduces what the resident core holds without changing any result. Upstream's code is otherwise unchanged; drop an
# overlay here when upstream takes the same change.
param([Parameter(Mandatory)][string]$CorePath)
$ErrorActionPreference = 'Stop'
function Set-Overlay([string]$File, [string]$Anchor, [string]$Replacement) {
    $path = Join-Path $CorePath "src/$File"
    $content = Get-Content $path -Raw
    if ([regex]::Matches($content,[regex]::Escape($Anchor)).Count -ne 1) { throw "Memory overlay contract changed: $File $Anchor" }
    Set-Content $path $content.Replace($Anchor,$Replacement) -Encoding utf8NoBOM -NoNewline
}
# Vectors filled by push keep up to twice their length for the life of the process.
Set-Overlay 'predict.rs' 'Index { ents, syls, names, pool, off, by_prefix }' "ents.shrink_to_fit();`n        by_prefix.values_mut().for_each(Vec::shrink_to_fit);`n        Index { ents, syls, names, pool, off, by_prefix }"
Set-Overlay 'lib.rs' 'lex.words.shrink_to_fit();' "lex.words.shrink_to_fit();`n        lex.key_pool.shrink_to_fit();`n        lex.readings.shrink_to_fit();"
# Upstream now deduplicates syllable expansion in Legal::new itself.
# The words only the overlay and packs have, kept for the colloquial term's k.
Set-Overlay 'colloquial.rs' 'overlay_only: HashSet<String>,' 'overlay_only: WordSet,'
Set-Overlay 'colloquial.rs' 'Ok(Colloquial { table, overlay_only, first_unigram: false, unseen_start_unigram: false })' 'Ok(Colloquial { table, overlay_only: WordSet::new(overlay_only), first_unigram: false, unseen_start_unigram: false })'
$root = Split-Path $PSScriptRoot -Parent
Add-Content (Join-Path $CorePath 'src/colloquial.rs') (Get-Content "$root/platform/word_set_windows.rs" -Raw)
