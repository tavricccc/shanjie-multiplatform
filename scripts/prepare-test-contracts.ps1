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
    $content = "// Windows model/punctuation test adapter; see UPSTREAM-CHANGES.md and tests/fixtures/classes-v5.`n" + $content
    [IO.File]::WriteAllText($path,$content,[Text.UTF8Encoding]::new($false))
}
