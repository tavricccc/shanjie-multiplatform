#Requires -Version 7
# Upstream's whole core test suite against the Windows-built core, with upstream's own model set
# (scripts/fetch-models.sh in upstream): every Windows overlay must keep upstream's assertions, apart
# from the documented Windows policies that prepare-test-contracts.ps1 adapts.
# -UseExistingCore tests build/core as it is (verify.ps1 has just built it).
# -UseExistingModels reuses the validated build output prepared by the parent verification run.
# -MemoryLimit: no further test binary starts while physical memory use is above this percentage.
param([string[]]$Tests, [switch]$NoUnitTests, [switch]$UseExistingCore, [switch]$UseExistingModels, [ValidateRange(10, 95)][int]$MemoryLimit = 80)
$ErrorActionPreference = 'Stop'
Import-Module "$PSScriptRoot/test-support.psm1" -ErrorAction Stop
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
            $seconds = $job.Started.Elapsed.TotalSeconds
            $status = if ($job.Process.ExitCode) { 'failed' } else { 'passed' }
            Complete-TestStage -Stage $job.Stage -Status $status -Details @{ executable=$job.Path; exit_code=$job.Process.ExitCode }
            if ($job.Process.ExitCode) { $failed += $job; Write-Host ("FAIL {0} ({1:F3}s)" -f $job.Name,$seconds) } else { Write-Host ("ok   {0} ({1:F3}s)" -f $job.Name,$seconds) }
        }
        # A binary just started has not loaded its models yet: wait a moment before judging the memory again.
        if ($queue.Count -and $running.Count -lt [Environment]::ProcessorCount -and ($running.Count -eq 0 -or (Get-MemoryUse) -lt $limit) -and
            !($running | Where-Object { $_.Started.Elapsed.TotalSeconds -lt 3 })) {
            $next = $queue.Dequeue()
            $log = Join-Path $logs (($next.Name -replace '[^\w-]', '_') + '.txt')
            $stage = Start-TestStage -Name $next.Name -Category 'core.test'
            $process = Start-Process -FilePath $next.Path -ArgumentList $skipArguments -WorkingDirectory $directory -NoNewWindow -PassThru `
                -RedirectStandardOutput $log -RedirectStandardError "$log.err"
            $null = $process.Handle  # keeps the exit code readable after the process ends
            $running += [pscustomobject]@{ Name = $next.Name; Path=$next.Path; Process = $process; Log = $log; Started = [Diagnostics.Stopwatch]::StartNew(); Stage=$stage }
            continue
        }
        Start-Sleep -Milliseconds 250
    }
    foreach ($job in $failed) { Write-Host "---- $($job.Name)"; Get-Content $job.Log, "$($job.Log).err" }
    if ($failed) { throw "Upstream tests failed: $(($failed | ForEach-Object Name) -join ', ')" }
}
$root = Split-Path $PSScriptRoot -Parent
. "$PSScriptRoot/export-upstream.ps1"
. "$PSScriptRoot/preserve-output.ps1"
$upstream = Join-Path $root 'upstream/shanjie'
Push-Location $root
try {
    $null = Initialize-TestTimings -ProjectRoot $root -Name 'test-core'
    if (!$UseExistingCore) { Invoke-Timed -Name 'Prepare Windows core' -Category 'core.prepare' -Action { & "$PSScriptRoot/prepare-core.ps1" } }
    # Upstream's fixed candidate assertions use its pinned models; separately updated built-ins are checked by prepare-models.
    if (!$UseExistingModels) { Invoke-Timed -Name 'Prepare built-in models' -Category 'models.prepare' -Action { & "$PSScriptRoot/prepare-models.ps1" | Out-Null } }
    $manifest = [IO.File]::ReadAllText((Join-Path $root 'MODEL-SOURCES.json')) | ConvertFrom-Json
    $upstreamPin = (Get-Content -LiteralPath "$root/UPSTREAM.json" -Raw | ConvertFrom-Json).commit
    $modelFixture = Join-Path $root "build/test-models/$upstreamPin/lm"
    $modelSources = @{}
    $modelCheck = Start-TestStage -Name 'Validate upstream core model inputs' -Category 'models.validate'
    try {
        if (!(Test-Path -LiteralPath "$root/dist/data/models/builtin/bigram.winmap")) { throw 'The prepared built-in model output is missing.' }
        $upstreamModelRows = Get-Content -LiteralPath "$upstream/data/models.tsv"
        foreach ($asset in $manifest.builtin.assets) {
            $source = Join-Path $root "build/model/$($asset.release)-$($asset.filename)"
            $expected = $asset.sha256.ToLowerInvariant()
            if ($asset.pinned_sha256) {
                $rows = @($upstreamModelRows | Where-Object { ($_ -split "`t")[0] -eq $asset.filename })
                if ($rows.Count -ne 1) { throw "Pinned upstream model must have exactly one models.tsv row: $($asset.filename)" }
                $release = ($rows[0] -split "`t")[3].ToLowerInvariant()
                $source = Join-Path $root "build/model/$release-$($asset.filename)"
                $expected = $asset.pinned_sha256.ToLowerInvariant()
                $downloaded = $false
                if (Test-Path -LiteralPath $source) {
                    $item = Get-Item -LiteralPath $source
                    $actual = if ($item.Length -eq $asset.size) { (Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash.ToLowerInvariant() } else { $null }
                    if ($actual -ne $expected) {
                        & "$PSScriptRoot/download-asset.ps1" -Url "$($manifest.repository)/releases/download/$release/$($asset.filename)" -Destination $source -Size $asset.size -Sha256 $expected
                        $downloaded = $true
                    }
                } else {
                    & "$PSScriptRoot/download-asset.ps1" -Url "$($manifest.repository)/releases/download/$release/$($asset.filename)" -Destination $source -Size $asset.size -Sha256 $expected
                    $downloaded = $true
                }
                # download-asset.ps1 verifies both the complete size and pinned SHA-256 before publishing the file.
                $sourceHash = if ($downloaded) { $expected } else { $actual }
            } else {
                if (!(Test-Path -LiteralPath $source)) { throw "Validated model cache is missing: $($asset.filename)" }
                $item = Get-Item -LiteralPath $source
                $sourceHash = if ($item.Length -eq $asset.size) { (Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash.ToLowerInvariant() } else { $null }
            }
            if ((Get-Item -LiteralPath $source).Length -ne $asset.size -or $sourceHash -ne $expected) {
                throw "Model input failed size or SHA-256 validation: $($asset.filename)"
            }
            $modelSources[$asset.filename] = [pscustomobject]@{ Path = $source; Hash = $sourceHash; Size = $asset.size }
        }
        Complete-TestStage -Stage $modelCheck -Details @{cache=if($UseExistingModels){'validated'}else{'prepared-and-validated'};model_id=$manifest.builtin.id;assets=$modelSources.Count}
    } catch {
        Complete-TestStage -Stage $modelCheck -Status 'failed' -Details @{cache=if($UseExistingModels){'invalid'}else{'prepared'};model_id=$manifest.builtin.id} -ErrorMessage $_.Exception.Message
        throw
    }

    $modelCopy = Start-TestStage -Name 'Copy validated models to isolated core fixtures' -Category 'models.copy'
    try {
        New-Item -ItemType Directory -Path $modelFixture -Force | Out-Null
        foreach ($asset in $manifest.builtin.assets) {
            $sourceInfo = $modelSources[$asset.filename]
            $destination = Join-Path $modelFixture $asset.filename
            $destinationMatches = $false
            if (Test-Path -LiteralPath $destination) {
                $destinationItem = Get-Item -LiteralPath $destination
                if ($destinationItem.Length -eq $sourceInfo.Size) {
                    $destinationMatches = (Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash.ToLowerInvariant() -eq $sourceInfo.Hash
                }
            }
            if (!$destinationMatches) { Copy-Item -LiteralPath $sourceInfo.Path -Destination $destination -Force }
        }
        Complete-TestStage -Stage $modelCopy -Details @{model_id=$manifest.builtin.id;assets=$modelSources.Count;source_hashes_reused=$true}
    } catch {
        Complete-TestStage -Stage $modelCopy -Status 'failed' -Details @{model_id=$manifest.builtin.id;assets=$modelSources.Count} -ErrorMessage $_.Exception.Message
        throw
    }
    # A separate checkout-shaped tree (test/core, test/data, test/eval), so neither test patches nor model
    # fixtures touch the pinned submodule or shipped core; test-relative paths resolve inside build/.
    $base = Join-Path $root 'build/test'
    $tree = Join-Path $base 'core'
    $resources = Join-Path $root "build/test-resources/$upstreamPin"
    $resourceKey = Get-ContentFingerprint -Root $root -Paths @('UPSTREAM.json','scripts/export-upstream.ps1') -Values @($upstreamPin,'git archive data eval')
    $resourceManifest = Join-Path $resources '.source-cache.json'
    $resourcePaths = @('data','eval')
    $resourceCheck = Start-TestStage -Name 'Validate upstream core fixture cache' -Category 'core.cache'
    try {
        $resourceCacheHit = Test-ContentCache -Root $resources -ManifestPath $resourceManifest -Key $resourceKey -Paths $resourcePaths
        Complete-TestStage -Stage $resourceCheck -Details @{cache=if($resourceCacheHit){'hit'}else{'miss'};upstream_commit=$upstreamPin}
    } catch {
        Complete-TestStage -Stage $resourceCheck -Status 'failed' -Details @{cache='miss';upstream_commit=$upstreamPin} -ErrorMessage $_.Exception.Message
        throw
    }
    if (!$resourceCacheHit) {
        if (Test-Path -LiteralPath $resources) { Preserve-Output $resources }
        New-Item -ItemType Directory -Path $resources -Force | Out-Null
        Invoke-Timed -Name 'Export upstream core fixtures' -Category 'core.prepare' -Details @{ cache='miss'; upstream_commit=$upstreamPin } -Action {
            Export-Upstream $resources @('data','eval')
        }
        Save-ContentCache -Root $resources -ManifestPath $resourceManifest -Key $resourceKey -Paths $resourcePaths
    } else { Write-Host "Core fixture cache hit: upstream $upstreamPin, contents verified." }

    $lmLink = Join-Path $resources 'data/lm'
    $lmItem = Get-Item -LiteralPath $lmLink -Force -ErrorAction SilentlyContinue
    if (!$lmItem) { New-Item -ItemType Junction -Path $lmLink -Target $modelFixture | Out-Null }
    elseif (!($lmItem.Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw 'Refusing to replace a non-link upstream LM fixture directory.' }
    elseif ([IO.Path]::GetFullPath([string]$lmItem.Target) -ne [IO.Path]::GetFullPath($modelFixture)) {
        $legacyModelTarget = [IO.Path]::GetFullPath("$upstream/data/lm")
        if ([IO.Path]::GetFullPath([string]$lmItem.Target) -ne $legacyModelTarget) { throw 'Refusing to reuse an upstream LM fixture link with an unexpected target.' }
        # Migrate only the known link from the former fixture layout. Move the junction itself; never traverse or delete its target.
        $archive = Join-Path $resources '.deleted_files'
        New-Item -ItemType Directory -Path $archive -Force | Out-Null
        Move-Item -LiteralPath $lmLink -Destination (Join-Path $archive ('legacy-lm-link-' + [guid]::NewGuid().ToString('N')))
        New-Item -ItemType Junction -Path $lmLink -Target $modelFixture | Out-Null
    }
    foreach ($name in $resourcePaths) {
        $link = Join-Path $base $name
        $target = Join-Path $resources $name
        $linkItem = Get-Item -LiteralPath $link -Force -ErrorAction SilentlyContinue
        $linkedTarget = if ($linkItem -and ($linkItem.Attributes -band [IO.FileAttributes]::ReparsePoint)) { [string]$linkItem.Target } else { $null }
        if ($linkedTarget) {
            if ([IO.Path]::GetFullPath($linkedTarget) -eq [IO.Path]::GetFullPath($target)) { continue }
            throw "Refusing to replace a fixture link with an unexpected target: $link"
        }
        if ($linkItem) {
            $archive = Join-Path $base '.deleted_files'
            New-Item -ItemType Directory -Path $archive -Force | Out-Null
            # Move the link itself, never recursively delete or traverse its target.
            Move-Item -LiteralPath $link -Destination (Join-Path $archive ($name+'-'+[guid]::NewGuid().ToString('N')))
        }
        New-Item -ItemType Junction -Path $link -Target $target | Out-Null
    }

    $rustVersion = (& rustc --version --verbose 2>&1 | Out-String).Trim()
    $cargoVersion = (& cargo --version 2>&1 | Out-String).Trim()
    $testKey = Get-ContentFingerprint -Root $root -Paths @(
        'build/core/src','build/core/Cargo.toml','upstream/shanjie/core/tests','upstream/shanjie/core/Cargo.toml',
        'scripts/test-core.ps1','scripts/prepare-test-contracts.ps1','scripts/test-support.psm1'
    ) -Values @($upstreamPin,$rustVersion,$cargoVersion,$env:RUSTFLAGS,$env:CARGO_ENCODED_RUSTFLAGS)
    $testManifest = Join-Path $tree '.source-cache.json'
    $testPaths = @('src','tests','Cargo.toml')
    $testTreeCheck = Start-TestStage -Name 'Validate isolated core test-source cache' -Category 'core.cache'
    try {
        $testTreeCacheHit = Test-ContentCache -Root $tree -ManifestPath $testManifest -Key $testKey -Paths $testPaths
        Complete-TestStage -Stage $testTreeCheck -Details @{cache=if($testTreeCacheHit){'hit'}else{'miss'};upstream_commit=$upstreamPin}
    } catch {
        Complete-TestStage -Stage $testTreeCheck -Status 'failed' -Details @{cache='miss';upstream_commit=$upstreamPin} -ErrorMessage $_.Exception.Message
        throw
    }
    if (!$testTreeCacheHit) {
        $stage = Start-TestStage -Name 'Prepare isolated core test tree' -Category 'core.prepare'
        try {
            foreach ($relative in $testPaths) {
                $path = Join-Path $tree $relative
                if (Test-Path -LiteralPath $path) { Preserve-Output $path }
            }
            New-Item -ItemType Directory -Path $tree -Force | Out-Null
            Copy-Item "$root/build/core/src","$root/build/core/Cargo.toml" $tree -Recurse -Force
            Copy-Item "$upstream/core/tests" $tree -Recurse -Force
            # Unprivileged Windows cannot make symlinks. An existing source is copied; a missing one (upstream's
            # "dangling symlink" cases) becomes a directory, which is the same "exists but cannot be read" case.
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
            # Unix file modes only prepare fixtures for upstream's learning store; Windows has its own store.
            $fuzzy = Join-Path $tree 'tests/engine_fuzzy_rerank.rs'
            $content = [IO.File]::ReadAllText($fuzzy)
            $setup = [regex]'(?m)^\s*(use std::os::unix::fs\PermissionsExt;|std::fs::set_permissions\([^\n]*from_mode\(0o[0-7]+\)\)\.unwrap\(\);)\r?\n'
            if ($setup.Matches($content).Count -ne 3) { throw 'Upstream fuzzy learning fixture changed.' }
            [IO.File]::WriteAllText($fuzzy, $setup.Replace($content, ''), [Text.UTF8Encoding]::new($false))
            # Tests of behavior Windows does not have, each named with the reason.
            $unixOnly = [regex]'(?m)^(#\[test\]\r?\n)?fn (store_append_refusals_fall_back_to_a_full_rewrite|max_rss|perf_full_store_per_key_and_enter_with_write)\('
            $learn = Join-Path $tree 'tests/engine_learn.rs'
            $content = [IO.File]::ReadAllText($learn)
            if ($unixOnly.Matches($content).Count -ne 3) { throw 'Upstream learning store test changed.' }
            [IO.File]::WriteAllText($learn, $unixOnly.Replace($content, "#[cfg(unix)]`n`$1fn `$2("), [Text.UTF8Encoding]::new($false))
            Preserve-Output (Join-Path $tree 'tests/learn_store.rs')
            & "$PSScriptRoot/prepare-test-contracts.ps1" -TestsPath (Join-Path $tree 'tests')
            Save-ContentCache -Root $tree -ManifestPath $testManifest -Key $testKey -Paths $testPaths
            Complete-TestStage -Stage $stage -Status 'passed' -Details @{ cache='miss'; upstream_commit=$upstreamPin }
        } catch {
            Complete-TestStage -Stage $stage -Status 'failed' -Details @{ cache='miss'; upstream_commit=$upstreamPin } -ErrorMessage $_.Exception.Message
            throw
        }
    } else { Write-Host "Core test-source cache hit: upstream $upstreamPin, source tree verified." }
    $skips = @(
        'english_codes_through_the_c_abi'      # makes a FIFO with /usr/bin/mkfifo
        'load_lm_refuses_a_bad_trigram_table'  # rewrites a table a live engine still maps; Windows refuses writes to a mapped file
    )
    $skipArguments = foreach ($skip in $skips) { '--skip'; $skip }
    $selected = if ($Tests) { $Tests } else { Get-ChildItem -LiteralPath "$tree/tests" -Filter *.rs | ForEach-Object BaseName }
    $arguments = @(foreach ($test in $selected) { '--test'; $test }) + @(if (!$NoUnitTests) { '--lib' })
    # Build every test binary once, then run them side by side: each binary has few tests and they mostly
    # decode on one thread, so cargo's one-binary-at-a-time run leaves most cores idle.
    $built = Invoke-Timed -Name 'Compile upstream core test binaries' -Category 'core.compile' -Details @{ test_binaries=$selected.Count; unit_tests=(!$NoUnitTests); cargo_profile='release';cache='Cargo target fingerprints' } -Action {
        $output = cargo test --release --no-run --message-format=json --manifest-path "$tree/Cargo.toml" @arguments
        if ($LASTEXITCODE) { throw 'Upstream tests did not build.' }
        $output
    }
    $binaries = $built | ForEach-Object { $_ | ConvertFrom-Json } | Where-Object { $_.reason -eq 'compiler-artifact' -and $_.profile.test -and $_.executable } |
        ForEach-Object { [pscustomobject]@{ Name = $_.target.name + $(if ($_.target.kind -contains 'lib') { ' (lib)' } else { '' }); Path = $_.executable } }
    # Both suites assert per-key latency. Isolate them from other binaries and
    # from each other's test cases; parallel decoding is not a latency baseline.
    $latency = @('engine_lm','engine_replay')
    Invoke-TestBinaries @($binaries | Where-Object Name -notin $latency) $tree $skipArguments $MemoryLimit
    foreach ($name in $latency) {
        Invoke-TestBinaries @($binaries | Where-Object Name -eq $name) $tree ($skipArguments + @('--test-threads=1')) $MemoryLimit
    }
} finally { Pop-Location }
