#Requires -Version 7
# Alternating old/new TinyLM comparison (old, new, old, new) modelled after upstream PR #93.
# Measures cold start, warm cache, per-key p50/p95, rerank p50/p95 and peak memory on short
# and 64-Han long-context sets without modifying dist/data or any real user profile.
param(
    [string]$OldTinylm,
    [string]$NewTinylm,
    [ValidateRange(1, 9)][int]$Rounds = 2,
    [ValidateRange(5, 302)][int]$ShortLimit = 60,
    [ValidateRange(5, 302)][int]$LongLimit = 60,
    [ValidateSet('chat', 'formal')][string]$Profile = 'chat',
    [string]$OutPath
)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
if (!$OldTinylm) { $OldTinylm = Join-Path $root 'build/model/sj-sr-8-15m-tinylm.sjtl' }
if (!$NewTinylm) { $NewTinylm = Join-Path $root 'build/model/sj-sr-10-31m-tinylm.sjtl' }
$OldTinylm = [IO.Path]::GetFullPath($OldTinylm)
$NewTinylm = [IO.Path]::GetFullPath($NewTinylm)
foreach ($pair in @(@('OldTinylm', $OldTinylm), @('NewTinylm', $NewTinylm))) {
    if (!(Test-Path -LiteralPath $pair[1])) { throw "$($pair[0]) not found: $($pair[1])" }
}
$oldSha = (Get-FileHash -LiteralPath $OldTinylm -Algorithm SHA256).Hash.ToLowerInvariant()
$newSha = (Get-FileHash -LiteralPath $NewTinylm -Algorithm SHA256).Hash.ToLowerInvariant()
if ($oldSha -eq $newSha) { throw 'OldTinylm and NewTinylm have identical SHA-256; refusing meaningless comparison.' }

$checker = Join-Path $root 'windows/target/x86_64-pc-windows-msvc/release/shanjie-model-check.exe'
if (!(Test-Path -LiteralPath $checker)) { throw "Missing release checker: $checker (build x64 first)." }
$distData = Join-Path $root 'dist/data'
if (!(Test-Path -LiteralPath (Join-Path $distData 'models/builtin/bigram.winmap'))) {
    throw "Missing prepared dist/data built-in model at $distData."
}

$work = Join-Path $root ('build/perf-compare-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $work -Force | Out-Null
$shortFile = Join-Path $work 'short-rows.txt'
$longFile = Join-Path $work 'long-rows.txt'

# Build deterministic short-context and 64-Han long-context row sets from pinned upstream eval sources.
$pyScript = @'
import os, re, sys
root, short_out, long_out, short_n, long_n = sys.argv[1], sys.argv[2], sys.argv[3], int(sys.argv[4]), int(sys.argv[5])
dev302 = os.path.join(root, "upstream", "shanjie", "eval", "bench", "suite-v1", "dev302.txt")
short_lines = [l for l in open(dev302, encoding="utf-8").read().splitlines() if l and not l.startswith("#")]
with open(short_out, "w", encoding="utf-8", newline="\n") as f:
    for l in short_lines[:short_n]:
        f.write(l + "\n")
sys.path.insert(0, os.path.join(root, "upstream", "shanjie", "scripts", "ci"))
import longctx_rows
with open(long_out, "w", encoding="utf-8", newline="\n") as f:
    f.write(longctx_rows.build(os.path.join(root, "upstream", "shanjie"), long_n, 64))
'@
$pyFile = Join-Path $work 'make_rows.py'
[IO.File]::WriteAllText($pyFile, $pyScript, [Text.UTF8Encoding]::new($false))
& python $pyFile $root $shortFile $longFile $ShortLimit $LongLimit
if ($LASTEXITCODE) { throw 'Failed to generate benchmark row sets.' }

function New-ModelDataTree([string]$Name, [string]$TinylmSource) {
    $dataDir = Join-Path $work "data-$Name"
    $builtinDst = Join-Path $dataDir 'models/builtin'
    New-Item -ItemType Directory -Path $builtinDst -Force | Out-Null
    foreach ($item in Get-ChildItem -LiteralPath $distData) {
        if ($item.Name -eq 'models') { continue }
        if ($item.PSIsContainer) {
            New-Item -ItemType Junction -Path (Join-Path $dataDir $item.Name) -Target $item.FullName | Out-Null
        } else {
            Copy-Item -LiteralPath $item.FullName -Destination (Join-Path $dataDir $item.Name) -Force
        }
    }
    $builtinSrc = Join-Path $distData 'models/builtin'
    foreach ($file in Get-ChildItem -LiteralPath $builtinSrc -File) {
        if ($file.Name -eq 'tinylm.sjtl') { continue }
        Copy-Item -LiteralPath $file.FullName -Destination (Join-Path $builtinDst $file.Name) -Force
    }
    Copy-Item -LiteralPath $TinylmSource -Destination (Join-Path $builtinDst 'tinylm.sjtl') -Force
    return $dataDir
}

$oldData = New-ModelDataTree 'old' $OldTinylm
$newData = New-ModelDataTree 'new' $NewTinylm
$ranking = if ($Profile -eq 'formal') { '1' } else { '0' }

function Invoke-Arm([string]$Arm, [int]$Round, [string]$DataDir, [string]$ExpectedSha) {
    $profileDir = Join-Path $work "profile-$Arm-r$Round"
    $modelsRoot = Join-Path $profileDir 'Shanjie/Models'
    New-Item -ItemType Directory -Path $modelsRoot -Force | Out-Null
    $start = [Diagnostics.ProcessStartInfo]::new($checker)
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    $start.StandardOutputEncoding = [Text.UTF8Encoding]::new($false)
    $start.Environment['LOCALAPPDATA'] = $profileDir
    $start.Environment['SHANJIE_TEST_PROFILE'] = '1'
    $start.Environment['SHANJIE_CORE_LOCAL'] = '1'
    foreach ($arg in @('bench', $modelsRoot, 'builtin', $DataDir, $shortFile, $longFile, $ranking)) {
        $start.ArgumentList.Add($arg)
    }
    $p = [Diagnostics.Process]::Start($start)
    $outTask = $p.StandardOutput.ReadToEndAsync()
    $errTask = $p.StandardError.ReadToEndAsync()
    if (!$p.WaitForExit(300000)) { $p.Kill(); throw "Benchmark timed out ($Arm round $Round)" }
    $stdout = $outTask.GetAwaiter().GetResult().Trim()
    $stderr = $errTask.GetAwaiter().GetResult().Trim()
    $code = $p.ExitCode
    $p.Dispose()
    if ($code -ne 0) { throw "Benchmark failed ($Arm round $Round, exit $code): $stderr" }
    $parsed = $stdout | ConvertFrom-Json
    if ($parsed.tinylm_sha256 -ne $ExpectedSha) {
        throw "Model hash mismatch for $Arm round ${Round}: expected $ExpectedSha, got $($parsed.tinylm_sha256)"
    }
    [pscustomobject]@{ arm = $Arm; round = $Round; metrics = $parsed }
}

$runs = [Collections.Generic.List[object]]::new()
for ($r = 1; $r -le $Rounds; $r++) {
    Write-Host "Running round $r/${Rounds}: old -> new ..."
    $runs.Add((Invoke-Arm 'old' $r $oldData $oldSha))
    $runs.Add((Invoke-Arm 'new' $r $newData $newSha))
}

function Get-Median([double[]]$Values) {
    $sorted = @($Values | Sort-Object)
    $n = $sorted.Count
    if ($n -eq 0) { return 0.0 }
    if ($n % 2 -eq 1) { return [double]$sorted[($n - 1) / 2] }
    return ([double]$sorted[$n / 2 - 1] + [double]$sorted[$n / 2]) / 2.0
}
function Summarize-Arm([string]$Arm) {
    $items = @($runs | Where-Object arm -eq $Arm | ForEach-Object metrics)
    $first = $items[0]
    [ordered]@{
        tinylm_sha256 = $first.tinylm_sha256
        tinylm_size = $first.tinylm_size
        magic = $first.magic
        boundary_k = $first.boundary_k
        rounds = $items.Count
        cold_open_ms = [Math]::Round((Get-Median @($items | ForEach-Object cold_open_ms)), 3)
        warm_open_ms = [Math]::Round((Get-Median @($items | ForEach-Object warm_open_ms)), 3)
        peak_working_set_mib = [Math]::Round((Get-Median @($items | ForEach-Object { $_.memory_mib.peak_working_set })), 2)
        peak_pagefile_mib = [Math]::Round((Get-Median @($items | ForEach-Object { $_.memory_mib.peak_pagefile })), 2)
        final_private_mib = [Math]::Round((Get-Median @($items | ForEach-Object { $_.memory_mib.final_private })), 2)
        short_top1 = $first.short.top1
        short_rows = $first.short.rows
        short_key_p50_ms = [Math]::Round((Get-Median @($items | ForEach-Object { $_.short.key_p50_ms })), 3)
        short_key_p95_ms = [Math]::Round((Get-Median @($items | ForEach-Object { $_.short.key_p95_ms })), 3)
        short_rerank_jobs = $first.short.rerank_jobs
        short_rerank_p50_ms = [Math]::Round((Get-Median @($items | ForEach-Object { $_.short.rerank_p50_ms })), 3)
        short_rerank_p95_ms = [Math]::Round((Get-Median @($items | ForEach-Object { $_.short.rerank_p95_ms })), 3)
        long_top1 = $first.long.top1
        long_rows = $first.long.rows
        long_key_p50_ms = [Math]::Round((Get-Median @($items | ForEach-Object { $_.long.key_p50_ms })), 3)
        long_key_p95_ms = [Math]::Round((Get-Median @($items | ForEach-Object { $_.long.key_p95_ms })), 3)
        long_rerank_jobs = $first.long.rerank_jobs
        long_rerank_p50_ms = [Math]::Round((Get-Median @($items | ForEach-Object { $_.long.rerank_p50_ms })), 3)
        long_rerank_p95_ms = [Math]::Round((Get-Median @($items | ForEach-Object { $_.long.rerank_p95_ms })), 3)
    }
}

$cpu = (Get-CimInstance Win32_Processor | Select-Object -First 1).Name.Trim()
$os = Get-CimInstance Win32_OperatingSystem
$report = [ordered]@{
    schema = 1
    generated_utc = [DateTimeOffset]::UtcNow.ToString('O')
    commit = (git -C $root rev-parse HEAD).Trim()
    architecture = 'x64'
    profile = $Profile
    rounds = $Rounds
    schedule = ((1..$Rounds | ForEach-Object { "old(r$_),new(r$_)" }) -join ',')
    machine = [ordered]@{
        cpu = $cpu
        logical_processors = [Environment]::ProcessorCount
        total_memory_gib = [Math]::Round($os.TotalVisibleMemorySize / 1048576.0, 2)
        os = "$($os.Caption.Trim()) ($($os.Version))"
    }
    summary = [ordered]@{
        old = Summarize-Arm 'old'
        new = Summarize-Arm 'new'
    }
    runs = @($runs)
}
if (!$OutPath) {
    $reportsDir = Join-Path $root 'build/reports'
    New-Item -ItemType Directory -Path $reportsDir -Force | Out-Null
    $OutPath = Join-Path $reportsDir ("tinylm-compare-" + [DateTimeOffset]::UtcNow.ToString('yyyyMMdd-HHmmss') + '.json')
}
[IO.File]::WriteAllText($OutPath, ($report | ConvertTo-Json -Depth 8), [Text.UTF8Encoding]::new($false))

$o = $report.summary.old
$n = $report.summary.new
Write-Host "Machine: $($report.machine.cpu) ($($report.machine.logical_processors) logical cores, $($report.machine.total_memory_gib) GiB RAM), $($report.machine.os)"
Write-Host "Schedule: $($report.schedule), profile=$Profile, short=$($o.short_rows) rows, long(64-Han)=$($o.long_rows) rows"
Write-Host ("OLD ({0}, sha {1}): cold={2}ms warm={3}ms peak_ws={4}MiB peak_pf={5}MiB | short key p50/p95={6}/{7}ms rerank p50/p95={8}/{9}ms top1={10}/{11} | long key p50/p95={12}/{13}ms rerank p50/p95={14}/{15}ms top1={16}/{17}" -f $o.magic, $o.tinylm_sha256.Substring(0,8), $o.cold_open_ms, $o.warm_open_ms, $o.peak_working_set_mib, $o.peak_pagefile_mib, $o.short_key_p50_ms, $o.short_key_p95_ms, $o.short_rerank_p50_ms, $o.short_rerank_p95_ms, $o.short_top1, $o.short_rows, $o.long_key_p50_ms, $o.long_key_p95_ms, $o.long_rerank_p50_ms, $o.long_rerank_p95_ms, $o.long_top1, $o.long_rows)
Write-Host ("NEW ({0}, sha {1}): cold={2}ms warm={3}ms peak_ws={4}MiB peak_pf={5}MiB | short key p50/p95={6}/{7}ms rerank p50/p95={8}/{9}ms top1={10}/{11} | long key p50/p95={12}/{13}ms rerank p50/p95={14}/{15}ms top1={16}/{17}" -f $n.magic, $n.tinylm_sha256.Substring(0,8), $n.cold_open_ms, $n.warm_open_ms, $n.peak_working_set_mib, $n.peak_pagefile_mib, $n.short_key_p50_ms, $n.short_key_p95_ms, $n.short_rerank_p50_ms, $n.short_rerank_p95_ms, $n.short_top1, $n.short_rows, $n.long_key_p50_ms, $n.long_key_p95_ms, $n.long_rerank_p50_ms, $n.long_rerank_p95_ms, $n.long_top1, $n.long_rows)
Write-Host "Report: $OutPath"
