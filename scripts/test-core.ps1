#Requires -Version 7
# Upstream's whole core test suite against the Windows-built core, with upstream's own model set
# (scripts/fetch-models.sh in upstream): every Windows overlay must keep upstream's assertions, apart
# from the documented Windows policies that prepare-test-contracts.ps1 adapts.
# -UseExistingCore tests build/core as it is (verify.ps1 has just built it).
# -MemoryLimit: no further test binary starts while physical memory use is above this percentage.
# A run whose test binaries, arguments, upstream data and models are byte-identical to an earlier
# passing run is skipped; -Rerun runs it anyway.
param([string[]]$Tests, [switch]$NoUnitTests, [switch]$UseExistingCore, [switch]$Rerun, [ValidateRange(10, 95)][int]$MemoryLimit = 80)
$ErrorActionPreference = 'Stop'
function Get-MemoryUse {
    $os = Get-CimInstance Win32_OperatingSystem
    100 * (1 - $os.FreePhysicalMemory / $os.TotalVisibleMemorySize)
}
# Runs up to one binary per core, starting the next only under the memory limit; prints a failed binary's output.
function Invoke-TestBinaries($binaries, [string]$directory, [string[]]$skipArguments, [int]$limit) {
    $logs = Join-Path $directory 'target/test-logs'
    New-Item -ItemType Directory $logs -Force | Out-Null
    $env:CARGO_MANIFEST_DIR = $directory  # what cargo test gives each binary
    $queue = [Collections.Generic.Queue[object]]::new([object[]]@($binaries))
    $running = @(); $failed = @()
    while ($queue.Count -or $running.Count) {
        foreach ($job in @($running | Where-Object { $_.Process.HasExited })) {
            $running = @($running | Where-Object { $_ -ne $job })
            $job.Process.WaitForExit()
            $seconds = [int]((Get-Date) - $job.Started).TotalSeconds
            if ($job.Process.ExitCode) { $failed += $job; Write-Host "FAIL $($job.Name) (${seconds}s)" } else { Write-Host "ok   $($job.Name) (${seconds}s)" }
        }
        # A binary just started has not loaded its models yet: wait a moment before judging the memory again.
        if ($queue.Count -and $running.Count -lt [Environment]::ProcessorCount -and ($running.Count -eq 0 -or (Get-MemoryUse) -lt $limit) -and
            !($running | Where-Object { ((Get-Date) - $_.Started).TotalSeconds -lt 3 })) {
            $next = $queue.Dequeue()
            $log = Join-Path $logs (($next.Name -replace '[^\w-]', '_') + '.txt')
            $process = Start-Process -FilePath $next.Path -ArgumentList $skipArguments -WorkingDirectory $directory -NoNewWindow -PassThru `
                -RedirectStandardOutput $log -RedirectStandardError "$log.err"
            $null = $process.Handle  # keeps the exit code readable after the process ends
            $running += [pscustomobject]@{ Name = $next.Name; Process = $process; Log = $log; Started = Get-Date }
            continue
        }
        Start-Sleep -Milliseconds 250
    }
    foreach ($job in $failed) { Write-Host "---- $($job.Name)"; Get-Content $job.Log, "$($job.Log).err" }
    if ($failed) { throw "Upstream tests failed: $(($failed | ForEach-Object Name) -join ', ')" }
}
# Mirrors $Source into $Destination, writing only files whose bytes differ and archiving files that
# are gone. target/ and archives in the destination are never touched.
function Sync-ChangedFiles([string]$Source, [string]$Destination) {
    New-Item -ItemType Directory -Path $Destination -Force | Out-Null
    $wanted = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    $written = 0
    foreach ($file in Get-ChildItem -LiteralPath $Source -Recurse -File | Where-Object { $_.FullName -notmatch '[\\/]\.deleted_files[\\/]' }) {
        $relative = [IO.Path]::GetRelativePath($Source, $file.FullName); [void]$wanted.Add($relative)
        $target = Join-Path $Destination $relative
        if ((Test-Path -LiteralPath $target) -and (Get-Item -LiteralPath $target).Length -eq $file.Length -and
            [Linq.Enumerable]::SequenceEqual([IO.File]::ReadAllBytes($target), [IO.File]::ReadAllBytes($file.FullName))) { continue }
        New-Item -ItemType Directory -Path (Split-Path $target -Parent) -Force | Out-Null
        Copy-Item -LiteralPath $file.FullName -Destination $target -Force; $written++
    }
    $existing = Get-ChildItem -LiteralPath $Destination -Force | Where-Object Name -notin 'target', '.deleted_files' |
        ForEach-Object { if ($_.PSIsContainer) { Get-ChildItem -LiteralPath $_.FullName -Recurse -File } else { $_ } }
    foreach ($stale in $existing | Where-Object {
            $relative = [IO.Path]::GetRelativePath($Destination, $_.FullName)
            $relative -notmatch '[\\/]\.deleted_files[\\/]' -and !$wanted.Contains($relative) }) {
        Preserve-Output $stale.FullName; $written++
    }
    $written
}
$root = Split-Path $PSScriptRoot -Parent
. "$PSScriptRoot/export-upstream.ps1"
. "$PSScriptRoot/preserve-output.ps1"
$upstream = Join-Path $root 'upstream/shanjie'
Push-Location $root
try {
    if (!$UseExistingCore) { & "$PSScriptRoot/prepare-core.ps1" }
    # Upstream's fixed candidate assertions use its pinned models; separately updated built-ins are checked by prepare-models.
    & "$PSScriptRoot/prepare-models.ps1" | Out-Null
    $manifest = [IO.File]::ReadAllText((Join-Path $root 'MODEL-SOURCES.json')) | ConvertFrom-Json
    New-Item -ItemType Directory -Path "$upstream/data/lm" -Force | Out-Null
    foreach ($asset in $manifest.builtin.assets) {
        $source = Join-Path $root "build/model/$($asset.release)-$($asset.filename)"
        if ($asset.pinned_sha256) {
            $row = Get-Content "$upstream/data/models.tsv" | Where-Object { ($_ -split "`t")[0] -eq $asset.filename }
            $release = ($row -split "`t")[3].ToLowerInvariant()
            $source = Join-Path $root "build/model/$release-$($asset.filename)"
            if (!(Test-Path -LiteralPath $source) -or (Get-FileHash -LiteralPath $source).Hash.ToLowerInvariant() -ne $asset.pinned_sha256) {
                & "$PSScriptRoot/download-asset.ps1" -Url "$($manifest.repository)/releases/download/$release/$($asset.filename)" -Destination $source -Size $asset.size -Sha256 $asset.pinned_sha256
            }
        }
        Copy-Item -LiteralPath $source -Destination "$upstream/data/lm/$($asset.filename)" -Force
    }
    # A separate checkout-shaped tree (test/core, test/data, test/eval), so the test patches below never reach
    # the core that is shipped and every path the tests take from CARGO_MANIFEST_DIR lands in upstream's files.
    $base = Join-Path $root 'build/test'
    # The tree is assembled in a fresh staging folder, then only files whose bytes changed are written
    # into build/test/core. Unchanged files keep their timestamps, so cargo does not rebuild the core
    # and every test binary on each run.
    $final = Join-Path $base 'core'
    $tree = Join-Path $base ('.stage-' + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $tree -Force | Out-Null
    Copy-Item "$root/build/core/src","$root/build/core/Cargo.toml" $tree -Recurse -Force
    Copy-Item "$upstream/core/tests" $tree -Recurse -Force
    $resources = Join-Path $root ('build/test-resources/'+[guid]::NewGuid().ToString('N'))
    Export-Upstream $resources @('data','eval')
    New-Item -ItemType Junction -Path "$resources/data/lm" -Target "$upstream/data/lm" | Out-Null
    foreach ($name in @('data','eval')) {
        $link = Join-Path $base $name
        if (Test-Path -LiteralPath $link) {
            $archive = Join-Path $base '.deleted_files'
            New-Item -ItemType Directory -Path $archive -Force | Out-Null
            # Move the link itself, never recursively delete or traverse its target.
            Move-Item -LiteralPath $link -Destination (Join-Path $archive ($name+'-'+[guid]::NewGuid().ToString('N')))
        }
        New-Item -ItemType Junction -Path $link -Target "$resources/$name" | Out-Null
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
    foreach ($name in $excluded) { Preserve-Output (Join-Path $tree "tests/$name.rs") }
    & "$PSScriptRoot/prepare-test-contracts.ps1" -TestsPath (Join-Path $tree 'tests')
    $written = Sync-ChangedFiles $tree $final
    Remove-Item -LiteralPath $tree -Recurse -Force  # generated in this run; build/test/core holds the result
    $tree = $final
    Write-Host "Upstream test tree: $written file(s) changed."
    $selected = if ($Tests) { $Tests } else { Get-ChildItem -LiteralPath "$tree/tests" -Filter *.rs | ForEach-Object BaseName }
    $arguments = @(foreach ($test in $selected) { '--test'; $test }) + @(if (!$NoUnitTests) { '--lib' })
    # Build every test binary once, then run them side by side: each binary has few tests and they mostly
    # decode on one thread, so cargo's one-binary-at-a-time run leaves most cores idle.
    $built = cargo test --release --no-run --message-format=json --manifest-path "$tree/Cargo.toml" @arguments
    if ($LASTEXITCODE) { throw 'Upstream tests did not build.' }
    $binaries = $built | ForEach-Object { $_ | ConvertFrom-Json } | Where-Object { $_.reason -eq 'compiler-artifact' -and $_.profile.test -and $_.executable } |
        ForEach-Object { [pscustomobject]@{ Name = $_.target.name + $(if ($_.target.kind -contains 'lib') { ' (lib)' } else { '' }); Path = $_.executable } }
    # The binaries hold all code under test; the rest of the result depends on the arguments and the
    # files the tests read (upstream data/eval at its commit, and the model files).
    $inputs = @(foreach ($binary in $binaries | Sort-Object Name) { "$($binary.Name)=$((Get-FileHash -LiteralPath $binary.Path).Hash)" }) +
        @("skip=$($skipArguments -join ' ')", "data=$(git -C $upstream rev-parse HEAD:data)", "eval=$(git -C $upstream rev-parse HEAD:eval)") +
        @(Get-ChildItem -LiteralPath "$upstream/data/lm" -File | Sort-Object Name | ForEach-Object { "lm/$($_.Name)=$((Get-FileHash -LiteralPath $_.FullName).Hash)" })
    $key = [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($inputs -join "`n"))).Substring(0, 32).ToLowerInvariant()
    $passed = Join-Path $base "passed/$key"
    if (!$Rerun -and (Test-Path -LiteralPath $passed)) {
        Write-Host "Upstream tests unchanged since a passing run ($key); skipped. Use -Rerun to run them."
        return
    }
    # These suites assert per-key latency. Isolate them from other binaries and
    # from each other's test cases; parallel decoding is not a latency baseline.
    $latency = @('engine_lm','engine_replay','engine_newword')
    Invoke-TestBinaries @($binaries | Where-Object Name -notin $latency) $tree $skipArguments $MemoryLimit
    foreach ($name in $latency) {
        Invoke-TestBinaries @($binaries | Where-Object Name -eq $name) $tree ($skipArguments + @('--test-threads=1')) $MemoryLimit
    }
    New-Item -ItemType Directory (Split-Path $passed -Parent) -Force | Out-Null
    Set-Content -LiteralPath $passed ($inputs -join "`n") -Encoding utf8NoBOM
} finally { Pop-Location }
