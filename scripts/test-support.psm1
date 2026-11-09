Set-StrictMode -Version Latest

function Get-RelativeFileEntries {
    param(
        [Parameter(Mandatory)][string]$Root,
        [Parameter(Mandatory)][string[]]$Paths,
        [switch]$RequirePaths
    )

    $rootPath = [IO.Path]::GetFullPath($Root)
    $files = [Collections.Generic.SortedDictionary[string,string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach ($path in $Paths) {
        $full = [IO.Path]::GetFullPath((Join-Path $rootPath $path))
        if (!(Test-Path -LiteralPath $full)) {
            if($RequirePaths){throw "Fingerprint input is missing: $full"}
            continue
        }
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
                if ($child.PSIsContainer -and $child.Name -eq '.deleted_files') { continue }
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

    $entries = Get-RelativeFileEntries -Root $Root -Paths $Paths -RequirePaths
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
        [Parameter(Mandatory)][AllowEmptyCollection()][string[]]$Paths
    )

    if (!$Paths.Count -or !(Test-Path -LiteralPath $ManifestPath)) { return $false }
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
    param([Parameter(Mandatory)][string]$Name,[Parameter(Mandatory)][string]$Category,[hashtable]$Details=@{})
    [pscustomobject]@{
        Id = [guid]::NewGuid().ToString('N')
        ParentId = $env:SHANJIE_TEST_PARENT_STAGE_ID
        Name = $Name
        Category = $Category
        Started = [DateTimeOffset]::UtcNow
        Watch = [Diagnostics.Stopwatch]::StartNew()
        Details = $Details
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
    $stageDetails = $Stage.Details.Clone()
    foreach($key in $Details.Keys){$stageDetails[$key]=$Details[$key]}
    $event = [ordered]@{
        schema = 1
        id = $Stage.Id
        parent_id = $Stage.ParentId
        process_id = $PID
        name = $Stage.Name
        category = $Stage.Category
        status = $Status
        started_utc = $Stage.Started.ToString('O')
        duration_ms = [Math]::Round($Stage.Watch.Elapsed.TotalMilliseconds, 3)
        details = $stageDetails
        test_name = $env:SHANJIE_CTEST_TEST_NAME
        attempt = if($env:SHANJIE_CTEST_ATTEMPT){[int]$env:SHANJIE_CTEST_ATTEMPT}else{1}
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
        [ValidateRange(100,10000)][int]$TimeoutMilliseconds = 10000,
        [string[]]$ProbeArguments = @('--status')
    )

    $stage = Start-TestStage -Name 'Wait for isolated core readiness' -Category 'core.lifecycle'
    $watch = [Diagnostics.Stopwatch]::StartNew()
    $attempt = 0
    try {
        while ($watch.ElapsedMilliseconds -lt $TimeoutMilliseconds) {
            $attempt++
            $stdout = Join-Path $OutputDirectory "core-status-$attempt.json"
            $stderr = "$stdout.err"
            $probe = Start-Process -FilePath $CorePath -ArgumentList $ProbeArguments -WindowStyle Hidden -PassThru `
                -RedirectStandardOutput $stdout -RedirectStandardError $stderr
            try {
                $remaining=[Math]::Max(1,$TimeoutMilliseconds-[int]$watch.ElapsedMilliseconds)
                if(!$probe.WaitForExit($remaining)) {
                    $probe.Kill()
                    $null=$probe.WaitForExit(1000)
                    throw "Isolated core status probe exceeded ${TimeoutMilliseconds}ms."
                }
                $probeExit=$probe.ExitCode
            } finally { $probe.Dispose() }
            if ($probeExit -eq 0 -and (Test-Path -LiteralPath $stdout)) {
                try { $status = Get-Content -LiteralPath $stdout -Raw | ConvertFrom-Json }
                catch { $status = $null }
                if ($status -and $status.ok) {
                    Complete-TestStage -Stage $stage -Status 'passed' -Details @{ attempts=$attempt; probe_exit_code=$probeExit }
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
    $previousParent=$env:SHANJIE_TEST_PARENT_STAGE_ID
    $env:SHANJIE_TEST_PARENT_STAGE_ID=$stage.Id
    try { & $Action }
    catch {
        $status = 'failed'
        $failure = $_.Exception.Message
        throw
    }
    finally {
        $env:SHANJIE_TEST_PARENT_STAGE_ID=$previousParent
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
        timings = @(Get-ExclusiveTestTimings $events | Sort-Object started_utc)
        junit = if ($JUnitPath) { [IO.Path]::GetFullPath($JUnitPath) } else { $null }
        generated_utc = [DateTimeOffset]::UtcNow.ToString('O')
    }
    $path = Join-Path (Split-Path -Parent $Directory) 'report.json'
    [IO.File]::WriteAllText($path, ($summary | ConvertTo-Json -Depth 12), [Text.UTF8Encoding]::new($false))
    Write-Host "Report: $path"
}

function Read-CtestResults([object[]]$Reports) {
    $attempts=foreach($report in $Reports) {
        if(!(Test-Path -LiteralPath $report.junit)){continue}
        [xml]$junit=Get-Content -LiteralPath $report.junit -Raw
        foreach($case in $junit.SelectNodes('//testcase')) {
            $failure=$case.SelectSingleNode('failure')
            $skipped=$case.SelectSingleNode('skipped')
            [pscustomobject]@{
                name=$case.name; class=$case.classname; preset=$report.preset; attempt=$report.attempt
                status=if($failure){'failed'}elseif($skipped){'not-run'}else{'passed'}
                duration_seconds=[double]$case.time
                failure=if($failure){$case.SelectSingleNode('system-out').InnerText}else{$null}
                reason=if($skipped){$skipped.GetAttribute('message')}else{$null}
                log=$report.log; junit=$report.junit
            }
        }
    }
    foreach($group in $attempts | Group-Object { "$($_.preset)::$($_.name)" }) {
        $history=@($group.Group | Sort-Object attempt)
        $last=$history[-1]
        [pscustomobject]@{
            name=$last.name; class=$last.class; preset=$last.preset; status=$last.status
            duration_seconds=($history | Measure-Object duration_seconds -Sum).Sum
            failure=$last.failure; reason=$last.reason; log=$last.log
            attempts=$history
            flaky=($last.status -eq 'passed' -and [bool](@($history | Where-Object status -eq 'failed').Count))
        }
    }
}

function Get-ExclusiveTestTimings([object[]]$Events) {
    foreach($entry in $Events) {
        $start=[DateTimeOffset]::Parse($entry.started_utc)
        $end=$start.AddMilliseconds($entry.duration_ms)
        $covered=0.0;$lastEnd=$start
        foreach($child in $Events | Where-Object parent_id -eq $entry.id | Sort-Object started_utc) {
            $childStart=[DateTimeOffset]::Parse($child.started_utc)
            $left=if($childStart -gt $lastEnd){$childStart}else{$lastEnd}
            $childEnd=$childStart.AddMilliseconds($child.duration_ms)
            $right=if($childEnd -lt $end){$childEnd}else{$end}
            if($right -gt $left){$covered+=($right-$left).TotalMilliseconds;$lastEnd=$right}
        }
        $entry | Add-Member -NotePropertyName exclusive_duration_ms -NotePropertyValue ([Math]::Round([Math]::Max(0,$entry.duration_ms-$covered),3)) -Force
        $entry
    }
}

Export-ModuleMember -Function Get-RelativeFileEntries,Get-ContentFingerprint,Test-ContentCache,Save-ContentCache,Initialize-TestTimings,Start-TestStage,Complete-TestStage,Wait-TestCoreReady,Invoke-Timed,Export-TestTimingSummary,Read-CtestResults,Get-ExclusiveTestTimings
