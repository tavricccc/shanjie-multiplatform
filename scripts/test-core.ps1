#Requires -Version 7
# Upstream's whole core test suite against the Windows-built core, with upstream's own model set
# (scripts/fetch-models.sh in upstream): every Windows overlay must keep upstream's assertions, apart
# from the documented Windows policies that prepare-test-contracts.ps1 adapts.
param([string[]]$Tests, [switch]$NoUnitTests)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$upstream = Join-Path $root 'upstream/shanjie'
Push-Location $root
try {
    # The models upstream's tests read from data/lm (ignored by its git): the built-in set, checked against data/*.sha256.
    & "$PSScriptRoot/prepare-models.ps1" | Out-Null
    $manifest = [IO.File]::ReadAllText((Join-Path $root 'MODEL-SOURCES.json')) | ConvertFrom-Json
    New-Item -ItemType Directory -Path "$upstream/data/lm" -Force | Out-Null
    foreach ($asset in $manifest.builtin.assets) {
        Copy-Item (Join-Path $root "build/model/$($asset.release)-$($asset.filename)") "$upstream/data/lm/$($asset.filename)" -Force
    }
    # A separate checkout-shaped tree (test/core, test/data, test/eval), so the test patches below never reach
    # the core that is shipped and every path the tests take from CARGO_MANIFEST_DIR lands in upstream's files.
    $base = Join-Path $root 'build/test'
    $tree = Join-Path $base 'core'
    if (Test-Path -LiteralPath $tree) { Get-ChildItem -LiteralPath $tree -Exclude target | Remove-Item -Recurse -Force }
    New-Item -ItemType Directory -Path $tree -Force | Out-Null
    Copy-Item "$root/build/core/src","$root/build/core/Cargo.toml" $tree -Recurse -Force
    Copy-Item "$upstream/core/tests" $tree -Recurse -Force
    foreach ($name in @('data','eval')) {
        $link = Join-Path $base $name
        $target = Join-Path $upstream $name
        if (Test-Path -LiteralPath $link) {
            $item = Get-Item -LiteralPath $link
            if ($item.LinkType -eq 'Junction' -and [IO.Path]::GetFullPath($item.Target) -eq [IO.Path]::GetFullPath($target)) { continue }
            if (!$item.LinkType) { throw "build/test/$name is not a junction; remove it first." }
            Remove-Item -LiteralPath $link -Force
        }
        New-Item -ItemType Junction -Path $link -Target $target | Out-Null
    }
    # Unprivileged Windows cannot make symlinks. An existing source is copied; a missing one (upstream's
    # "dangling symlink" cases) becomes a directory, which is the same "exists but cannot be read" case,
    # so removing the link in those files also removes such a directory.
    Add-Content "$tree/src/lib.rs" @'

#[doc(hidden)]
pub fn windows_test_link(source: impl AsRef<std::path::Path>, link: impl AsRef<std::path::Path>) -> std::io::Result<()> {
    let (source, link) = (source.as_ref(), link.as_ref());
    if source.exists() { std::fs::copy(source, link).map(|_| ()) } else { std::fs::create_dir(link) }
}
#[doc(hidden)]
pub fn windows_test_remove(path: impl AsRef<std::path::Path>) -> std::io::Result<()> {
    let path = path.as_ref();
    if path.is_dir() { std::fs::remove_dir(path) } else { std::fs::remove_file(path) }
}
'@
    foreach ($file in Get-ChildItem -LiteralPath "$tree/src","$tree/tests" -Recurse -Filter *.rs) {
        $content = [IO.File]::ReadAllText($file.FullName)
        if (!$content.Contains('std::os::unix::fs::symlink(')) { continue }
        $crate = if ($file.FullName.StartsWith((Join-Path $tree 'src'))) { 'crate' } else { 'core' }
        $content = $content.Replace('std::os::unix::fs::symlink(', "${crate}::windows_test_link(").Replace('std::fs::remove_file(', "${crate}::windows_test_remove(")
        [IO.File]::WriteAllText($file.FullName, $content, [Text.UTF8Encoding]::new($false))
    }
    # Unix file modes only prepare fixtures for upstream's learning store; Windows has its own store (platform/learning_windows.rs).
    $fuzzy = Join-Path $tree 'tests/engine_fuzzy_rerank.rs'
    $content = [IO.File]::ReadAllText($fuzzy)
    $setup = [regex]'(?m)^\s*(use std::os::unix::fs::PermissionsExt;|std::fs::set_permissions\([^\n]*from_mode\(0o[0-7]+\)\)\.unwrap\(\);)\r?\n'
    if ($setup.Matches($content).Count -ne 3) { throw 'Upstream fuzzy learning fixture changed.' }
    [IO.File]::WriteAllText($fuzzy, $setup.Replace($content, ''), [Text.UTF8Encoding]::new($false))
    # Tests of behavior Windows does not have, each named with the reason:
    $excluded = @(
        'learn_store'  # upstream's unix learning file (modes, FIFOs, hard links); Windows replaces the store
    )
    $skips = @(
        'english_codes_through_the_c_abi'      # makes a FIFO with /usr/bin/mkfifo
        'load_lm_refuses_a_bad_trigram_table'  # rewrites a table a live engine still maps; Windows refuses writes to a mapped file
    )
    $skipArguments = foreach ($skip in $skips) { '--skip'; $skip }
    # Unix modes, FIFOs and hard links of upstream's store; peak memory through unix getrusage:
    $unixOnly = [regex]'(?m)^(#\[test\]\r?\n)?fn (store_append_refusals_fall_back_to_a_full_rewrite|max_rss|perf_full_store_per_key_and_enter_with_write)\('
    $learn = Join-Path $tree 'tests/engine_learn.rs'
    $content = [IO.File]::ReadAllText($learn)
    if ($unixOnly.Matches($content).Count -ne 3) { throw 'Upstream learning store test changed.' }
    [IO.File]::WriteAllText($learn, $unixOnly.Replace($content, "#[cfg(unix)]`n`$1fn `$2("), [Text.UTF8Encoding]::new($false))
    foreach ($name in $excluded) { Remove-Item -LiteralPath (Join-Path $tree "tests/$name.rs") }
    & "$PSScriptRoot/prepare-test-contracts.ps1" -TestsPath (Join-Path $tree 'tests')
    if (!$NoUnitTests) {
        cargo test --release --manifest-path "$tree/Cargo.toml" --lib -- @skipArguments
        if ($LASTEXITCODE) { throw 'Upstream unit tests failed.' }
    }
    $selected = if ($Tests) { $Tests } else { Get-ChildItem -LiteralPath "$tree/tests" -Filter *.rs | ForEach-Object BaseName }
    $arguments = foreach ($test in $selected) { '--test'; $test }
    cargo test --release --manifest-path "$tree/Cargo.toml" @arguments -- @skipArguments
    if ($LASTEXITCODE) { throw 'Upstream engine tests failed.' }
} finally { Pop-Location }
