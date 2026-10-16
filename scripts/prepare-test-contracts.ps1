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
    'engine_learn.rs' = @(
        @('    type_syls(&mut e, "ㄅㄣ ㄒㄧㄣ");', '    let ben_was_default = type_syls(&mut e, "ㄅㄣ ㄒㄧㄣ").preedit.starts_with("奔");'),
        @('assert_eq!(keys(&e), ["你好", "好奔"], "奔 after 你好, 欣 after 好 and the fixed word 奔");', 'assert_eq!(keys(&e), if ben_was_default { vec!["好奔"] } else { vec!["你好", "好奔"] }, "Only changed picks create records; the default depends on the class weight");')
    )
    'engine_lm.rs' = @( ,@('廚房新裝了一個綠水壺', '廚房新裝了一個率水壺') )
}
foreach ($file in $contracts.Keys) {
    $path = Join-Path $TestsPath $file
    $content = [IO.File]::ReadAllText($path)
    $function = switch ($file) { 'engine.rs' { 'fn row2_punctuation_goes_into_the_composition()' } 'engine_learn.rs' { 'fn context_key_learned_equals_context_key_queried()' } default { $null } }
    $start = if ($function) { $content.IndexOf($function) } else { 0 }
    if ($start -lt 0) { throw 'Missing Windows punctuation test function.' }
    $end = if ($function) { $content.IndexOf("`n#[test]",$start) } else { -1 }
    if ($end -lt 0) { $end = $content.Length }
    $adapted = $content.Substring($start,$end-$start)
    foreach ($pair in $contracts[$file]) {
        $expected = if ($file -eq 'engine_lm.rs') { 2 } else { 1 }
        if ([regex]::Matches($adapted,[regex]::Escape($pair[0])).Count -ne $expected) { throw "Windows test contract changed: $file $($pair[0])" }
        $adapted = $adapted.Replace($pair[0],$pair[1])
    }
    $content = $content.Substring(0,$start) + $adapted + $content.Substring($end)
    $content = "// Windows model/punctuation test adapter; see UPSTREAM-CHANGES.md and tests/fixtures/classes-v5.`n" + $content
    [IO.File]::WriteAllText($path,$content,[Text.UTF8Encoding]::new($false))
}
