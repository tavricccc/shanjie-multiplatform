#Requires -Version 7
param([Parameter(Mandatory)][string]$TestsPath)
$ErrorActionPreference = 'Stop'
# Adapt only the Windows preservation policy. Keep the remaining upstream assertions.
$contracts = @{
    'engine.rs' = @(
        @('o.preedit == format!("你好{p}")', 'o.preedit == format!("你好ㄏ{p}")'),
        @('o.cursor_utf16 == 3, "{c}"', 'o.cursor_utf16 == 4, "{c}"'),
        @('o.commit == format!("你好{p}")', 'o.commit == format!("你好ㄏ{p}")'),
        @(')).preedit == "你好、"', ')).preedit == "你好ㄏ、"')
    )
    'engine_predict.rs' = @(
        @('("，", None));', '("ㄒ，", None));'),
        @('e.key(comma()).unwrap().preedit, "，");', 'e.key(comma()).unwrap().preedit, "ㄋㄔ，");')
    )
    'engine_english.rs' = @(
        @('run(&mut e, "r u");', 'let windows_pending = run(&mut e, "r u").preedit;'),
        @("let mut e = eng();`n    run(&mut e, `"r u s t`");", "let mut e = eng();`n    let windows_pending = run(&mut e, `"r u s t`").preedit;"),
        @('assert!(run(&mut e, "S-,").preedit == "，");', 'assert_eq!(run(&mut e, "S-,").preedit, format!("{windows_pending}，"), "Windows retains the displayed unfinished reading");')
    )
}
foreach ($file in $contracts.Keys) {
    $path = Join-Path $TestsPath $file
    $content = [IO.File]::ReadAllText($path).Replace("`r`n","`n")
    $function = switch ($file) { 'engine.rs' { 'fn row2_punctuation_goes_into_the_composition()' } 'engine_english.rs' { 'fn punctuation_keeps_the_letters_that_are_shown()' } default { $null } }
    $start = if ($function) { $content.IndexOf($function) } else { 0 }
    if ($start -lt 0) { throw 'Missing Windows punctuation test function.' }
    $end = if ($function) { $content.IndexOf("`n#[test]",$start) } else { -1 }
    if ($end -lt 0) { $end = $content.Length }
    $adapted = $content.Substring($start,$end-$start)
    foreach ($pair in $contracts[$file]) {
        $expected = if ($file -eq 'engine_english.rs' -and $pair[0].StartsWith('assert!')) { 2 } else { 1 }
        if ([regex]::Matches($adapted,[regex]::Escape($pair[0])).Count -ne $expected) { throw "Windows test contract changed: $file $($pair[0])" }
        $adapted = $adapted.Replace($pair[0],$pair[1])
    }
    $content = $content.Substring(0,$start) + $adapted + $content.Substring($end)
    $content = "// Windows punctuation test adapter; see UPSTREAM-CHANGES.md.`n" + $content
    [IO.File]::WriteAllText($path,$content,[Text.UTF8Encoding]::new($false))
}
# Digits off (upstream #40/#50) compares every row of #1-#39 with the base commit. Rows r01, r02 and r36 type a
# mark behind the unfinished ㄅ of a lone 1, which the Windows policy keeps; every other row is compared as is.
$path = Join-Path $TestsPath 'engine_digits_off.rs'
$content = [IO.File]::ReadAllText($path).Replace("`r`n","`n")
$old = 'let (g, w): (Vec<&str>, Vec<&str>) = (got.lines().collect(), want.lines().collect());'
$new = 'let windows = |l: &&str| !["r01\t", "r02\t", "r36\t"].iter().any(|r| l.starts_with(r)); let (g, w): (Vec<&str>, Vec<&str>) = (got.lines().filter(windows).collect(), want.lines().filter(windows).collect());'
if ([regex]::Matches($content,[regex]::Escape($old)).Count -ne 1) { throw 'Windows test contract changed: engine_digits_off.rs' }
$content = "// Windows punctuation test adapter; see UPSTREAM-CHANGES.md.`n" + $content.Replace($old,$new).Replace('std::os::unix::fs::symlink(', 'std::fs::copy(')
[IO.File]::WriteAllText($path,$content,[Text.UTF8Encoding]::new($false))
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
