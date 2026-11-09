Set-StrictMode -Version Latest

function Get-RelativeFileEntries {
    param(
        [Parameter(Mandatory)][string]$Root,
        [Parameter(Mandatory)][string[]]$Paths
    )

    $rootPath = [IO.Path]::GetFullPath($Root)
    $files = [Collections.Generic.SortedDictionary[string,string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach ($path in $Paths) {
        $full = [IO.Path]::GetFullPath((Join-Path $rootPath $path))
        if (!(Test-Path -LiteralPath $full)) { continue }
        $item = Get-Item -LiteralPath $full -Force
        if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) { continue }
        if (!$item.PSIsContainer) {
            $relative = [IO.Path]::GetRelativePath($rootPath, $full).Replace('\','/')
            $files[$relative] = (Get-FileHash -LiteralPath $full -Algorithm SHA256).Hash.ToLowerInvariant()
            continue
        }

        $pending = [Collections.Generic.Stack[string]]::new()
        $pending.Push($full)
        while ($pending.Count) {
            foreach ($child in Get-ChildItem -LiteralPath $pending.Pop() -Force) {
                if ($child.Attributes -band [IO.FileAttributes]::ReparsePoint) { continue }
                if ($child.PSIsContainer) { $pending.Push($child.FullName); continue }
                $relative = [IO.Path]::GetRelativePath($rootPath, $child.FullName).Replace('\','/')
                $files[$relative] = (Get-FileHash -LiteralPath $child.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
            }
        }
    }

    return ,$files
}

function Get-ContentFingerprint {
    param(
        [Parameter(Mandatory)][string]$Root,
        [Parameter(Mandatory)][string[]]$Paths,
        [string[]]$Values = @()
    )

    $entries = Get-RelativeFileEntries -Root $Root -Paths $Paths
    $builder = [Text.StringBuilder]::new()
    foreach ($entry in $entries.GetEnumerator()) {
        [void]$builder.Append($entry.Key).Append("`0").Append($entry.Value).Append("`n")
    }
    foreach ($value in $Values) { [void]$builder.Append("value").Append("`0").Append($value).Append("`n") }
    $bytes = [Text.Encoding]::UTF8.GetBytes($builder.ToString())
    return [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($bytes)).ToLowerInvariant()
}

function Test-ContentCache {
    param(
        [Parameter(Mandatory)][string]$Root,
        [Parameter(Mandatory)][string]$ManifestPath,
        [Parameter(Mandatory)][string]$Key,
        [Parameter(Mandatory)][string[]]$Paths
    )

    if (!(Test-Path -LiteralPath $ManifestPath)) { return $false }
    try { $saved = Get-Content -LiteralPath $ManifestPath -Raw | ConvertFrom-Json -AsHashtable }
    catch { return $false }
    if ($saved.key -ne $Key -or !$saved.files) { return $false }
    $current = Get-RelativeFileEntries -Root $Root -Paths $Paths
    if ($current.Count -ne $saved.files.Count) { return $false }
    foreach ($entry in $current.GetEnumerator()) {
        if ($entry.Key -notin $saved.files.Keys -or $saved.files[$entry.Key] -ne $entry.Value) { return $false }
    }
    return $true
}

function Save-ContentCache {
    param(
        [Parameter(Mandatory)][string]$Root,
        [Parameter(Mandatory)][string]$ManifestPath,
        [Parameter(Mandatory)][string]$Key,
        [Parameter(Mandatory)][string[]]$Paths
    )

    $manifest = [ordered]@{
        schema = 1
        key = $Key
        files = Get-RelativeFileEntries -Root $Root -Paths $Paths
    }
    $parent = Split-Path -Parent $ManifestPath
    New-Item -ItemType Directory -Path $parent -Force | Out-Null
    $temporary = "$ManifestPath.$([guid]::NewGuid().ToString('N')).tmp"
    [IO.File]::WriteAllText($temporary, ($manifest | ConvertTo-Json -Depth 8), [Text.UTF8Encoding]::new($false))
    Move-Item -LiteralPath $temporary -Destination $ManifestPath -Force
}

function Initialize-TestTimings {
    param([Parameter(Mandatory)][string]$ProjectRoot, [string]$Name = 'powershell')
    if ($env:SHANJIE_TEST_TIMINGS_DIR) { return $env:SHANJIE_TEST_TIMINGS_DIR }
    $run = '{0}-{1}-{2}' -f $Name,(Get-Date -Format 'yyyyMMdd-HHmmss'),[guid]::NewGuid().ToString('N').Substring(0,8)
    $directory = Join-Path $ProjectRoot "build/reports/$run/events"
    New-Item -ItemType Directory -Path $directory -Force | Out-Null
    $env:SHANJIE_TEST_TIMINGS_DIR = $directory
    return $directory
}

function Start-TestStage {
    param([Parameter(Mandatory)][string]$Name,[Parameter(Mandatory)][string]$Category)
    [pscustomobject]@{
        Name = $Name
        Category = $Category
        Started = [DateTimeOffset]::UtcNow
        Watch = [Diagnostics.Stopwatch]::StartNew()
    }
}

function Complete-TestStage {
    param(
        [Parameter(Mandatory)]$Stage,
        [ValidateSet('passed','failed','skipped')][string]$Status = 'passed',
        [hashtable]$Details = @{},
        [string]$ErrorMessage
    )

    $Stage.Watch.Stop()
    $event = [ordered]@{
        schema = 1
        name = $Stage.Name
        category = $Stage.Category
        status = $Status
        started_utc = $Stage.Started.ToString('O')
        duration_ms = [Math]::Round($Stage.Watch.Elapsed.TotalMilliseconds, 3)
        details = $Details
        error = $ErrorMessage
    }
    $directory = $env:SHANJIE_TEST_TIMINGS_DIR
    if ($directory) {
        New-Item -ItemType Directory -Path $directory -Force | Out-Null
        $path = Join-Path $directory ("{0}-{1}-{2}.json" -f (Get-Date -Format 'yyyyMMddTHHmmssfffZ'),$PID,[guid]::NewGuid().ToString('N'))
        [IO.File]::WriteAllText($path, ($event | ConvertTo-Json -Depth 8), [Text.UTF8Encoding]::new($false))
    }
    Write-Host ('[TIME] {0}/{1} {2:F3}s ({3})' -f $Stage.Category,$Stage.Name,$Stage.Watch.Elapsed.TotalSeconds,$Status)
}

function Wait-TestCoreReady {
    param(
        [Parameter(Mandatory)][string]$CorePath,
        [Parameter(Mandatory)][string]$OutputDirectory,
        [ValidateRange(100,10000)][int]$TimeoutMilliseconds = 1000
    )

    $stage = Start-TestStage -Name 'Wait for isolated core readiness' -Category 'core.lifecycle'
    $watch = [Diagnostics.Stopwatch]::StartNew()
    $attempt = 0
    try {
        while ($watch.ElapsedMilliseconds -lt $TimeoutMilliseconds) {
            $attempt++
            $stdout = Join-Path $OutputDirectory "core-status-$attempt.json"
            $stderr = "$stdout.err"
            $probe = Start-Process -FilePath $CorePath -ArgumentList '--status' -WindowStyle Hidden -PassThru -Wait `
                -RedirectStandardOutput $stdout -RedirectStandardError $stderr
            if ($probe.ExitCode -eq 0 -and (Test-Path -LiteralPath $stdout)) {
                try { $status = Get-Content -LiteralPath $stdout -Raw | ConvertFrom-Json }
                catch { $status = $null }
                if ($status -and $status.ok) {
                    Complete-TestStage -Stage $stage -Status 'passed' -Details @{ attempts=$attempt; probe_exit_code=$probe.ExitCode }
                    return $true
                }
            }
            Start-Sleep -Milliseconds 100
        }
        throw "Isolated core did not become ready within ${TimeoutMilliseconds}ms."
    } catch {
        Complete-TestStage -Stage $stage -Status 'failed' -Details @{ attempts=$attempt; timeout_ms=$TimeoutMilliseconds } -ErrorMessage $_.Exception.Message
        throw
    }
}

function Invoke-Timed {
    param(
        [Parameter(Mandatory)][string]$Name,
        [Parameter(Mandatory)][string]$Category,
        [Parameter(Mandatory)][scriptblock]$Action,
        [hashtable]$Details = @{}
    )

    $stage = Start-TestStage -Name $Name -Category $Category
    $status = 'passed'
    $failure = $null
    try { & $Action }
    catch {
        $status = 'failed'
        $failure = $_.Exception.Message
        throw
    }
    finally {
        Complete-TestStage -Stage $stage -Status $status -Details $Details -ErrorMessage $failure
    }
}

function Export-TestTimingSummary {
    param(
        [Parameter(Mandatory)][string]$Directory,
        [Parameter(Mandatory)][string]$Level,
        [Parameter(Mandatory)][double]$WallSeconds,
        [string]$JUnitPath
    )

    $events = @(Get-ChildItem -LiteralPath $Directory -Filter '*.json' -File -ErrorAction SilentlyContinue |
        ForEach-Object { Get-Content -LiteralPath $_.FullName -Raw | ConvertFrom-Json })
    $summary = [ordered]@{
        schema = 1
        level = $Level
        wall_seconds = [Math]::Round($WallSeconds, 3)
        comparisons = [ordered]@{ legacy_seconds = $null; full_seconds = $null; saved_seconds = $null; speedup_percent = $null; status = 'not-measured' }
        timings = @($events | Sort-Object started_utc)
        junit = if ($JUnitPath) { [IO.Path]::GetFullPath($JUnitPath) } else { $null }
        generated_utc = [DateTimeOffset]::UtcNow.ToString('O')
    }
    $path = Join-Path (Split-Path -Parent $Directory) 'report.json'
    [IO.File]::WriteAllText($path, ($summary | ConvertTo-Json -Depth 12), [Text.UTF8Encoding]::new($false))
    Write-Host "Report: $path"
}

Export-ModuleMember -Function Get-ContentFingerprint,Test-ContentCache,Save-ContentCache,Initialize-TestTimings,Start-TestStage,Complete-TestStage,Wait-TestCoreReady,Invoke-Timed,Export-TestTimingSummary
