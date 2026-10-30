#Requires -Version 7
param([Parameter(Mandatory)][string]$TestsPath)
$ErrorActionPreference = 'Stop'
# oc_tiny maps each NaN model read-only; Windows refuses to delete a file while a mapping of it is alive, so the
# two models are dropped before the temporary directory is removed. The assertions are unchanged.
$path = Join-Path $TestsPath 'oc_tiny.rs'
$content = [IO.File]::ReadAllText($path).Replace("`r`n","`n")
$pairs = @(
    @('    let m = nan("tone5", tone5);', "    drop(m);`n    let m = nan(`"tone5`", tone5);"),
    @("vec![None; k.len()]);`n    }`n    std::fs::remove_dir_all(tmp).unwrap();", "vec![None; k.len()]);`n    }`n    drop(m);`n    std::fs::remove_dir_all(tmp).unwrap();")
)
foreach ($pair in $pairs) {
    if ([regex]::Matches($content,[regex]::Escape($pair[0])).Count -ne 1) { throw "Windows test contract changed: oc_tiny.rs $($pair[0])" }
    $content = $content.Replace($pair[0],$pair[1])
}
[IO.File]::WriteAllText($path,"// Windows mapping test adapter; see UPSTREAM-CHANGES.md.`n" + $content,[Text.UTF8Encoding]::new($false))
# tri_tiny maps each profile's table read-only; the models holding a table are dropped before the test deletes or
# rewrites that file. The assertions are unchanged.
$path = Join-Path $TestsPath 'tri_tiny.rs'
$content = [IO.File]::ReadAllText($path).Replace("`r`n","`n")
$pairs = @(
    @("    core::windows_test_remove(&formal).unwrap();`n    std::fs::write(&chat, chat_table_bytes()).unwrap();", "    drop(only_formal);`n    core::windows_test_remove(&formal).unwrap();`n    std::fs::write(&chat, chat_table_bytes()).unwrap();"),
    @("    for (bad, reads_bad, other) in [(&chat, Profile::Chat, Profile::Formal), (&formal, Profile::Formal, Profile::Chat)] {", "    drop((only_chat, on));`n    for (bad, reads_bad, other) in [(&chat, Profile::Chat, Profile::Formal), (&formal, Profile::Formal, Profile::Chat)] {")
)
foreach ($pair in $pairs) {
    if ([regex]::Matches($content,[regex]::Escape($pair[0])).Count -ne 1) { throw "Windows test contract changed: tri_tiny.rs $($pair[0])" }
    $content = $content.Replace($pair[0],$pair[1])
}
[IO.File]::WriteAllText($path,"// Windows mapping test adapter; see UPSTREAM-CHANGES.md.`n" + $content,[Text.UTF8Encoding]::new($false))
