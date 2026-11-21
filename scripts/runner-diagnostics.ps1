#Requires -Version 7
param(
    [ValidateSet('pre','post')][string]$Phase = 'pre',
    [string]$OutputPath,
    [string]$RunnerLabel,
    [string]$TestLevel = 'unavailable',
    [int]$Jobs = 0,
    [string]$TestReportPath,
    [string]$CacheSpecJson,
    [switch]$Quiet
)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
if (!$OutputPath) { $OutputPath = Join-Path $root 'build/reports/runner-diagnostics.json' }

function Protect-DiagnosticText([AllowNull()][string]$Text) {
    if ([string]::IsNullOrEmpty($Text)) { return $Text }
    $sanitized = $Text
    # Strip credentials embedded in URLs (e.g., https://user:token@host/path).
    $sanitized = [regex]::Replace($sanitized, '(?i)(https?://)([^/\s@]+)@', '$1[REDACTED]@')
    # Mask GitHub tokens, Bearer headers, and common secret key-value patterns.
    $sanitized = [regex]::Replace($sanitized, '\b(ghp|gho|ghu|ghs|ghr)_[A-Za-z0-9_]{10,}\b', '[REDACTED]')
    $sanitized = [regex]::Replace($sanitized, '\bgithub_pat_[A-Za-z0-9_]{10,}\b', '[REDACTED]')
    $sanitized = [regex]::Replace($sanitized, '(?i)\b(Bearer|Basic)\s+[A-Za-z0-9._~+/=-]{8,}', '$1 [REDACTED]')
    $sanitized = [regex]::Replace($sanitized, '(?i)\b(token|secret|password|apikey|api_key|access_key)\s*[:=]\s*[^\s,;''"]+', '$1=[REDACTED]')
    return $sanitized
}

function Get-ValueOrUnavailable($Value) {
    if ($null -eq $Value) { return 'unavailable' }
    if ($Value -is [string] -and [string]::IsNullOrWhiteSpace($Value)) { return 'unavailable' }
    return $Value
}

function Get-CommandFirstLine([string]$Executable, [string[]]$Arguments) {
    if (!$Executable) { return 'unavailable' }
    try {
        $output = & $Executable @Arguments 2>&1
        if ($LASTEXITCODE -ne 0) { return 'unavailable' }
        $line = @($output | ForEach-Object { ([string]$_).Trim() } | Where-Object { $_ }) | Select-Object -First 1
        if (!$line) { return 'unavailable' }
        return (Protect-DiagnosticText $line)
    } catch {
        return 'unavailable'
    }
}

function Get-SafeRepositoryName([string]$ProjectRoot) {
    if ($env:GITHUB_REPOSITORY) {
        return (Protect-DiagnosticText $env:GITHUB_REPOSITORY.Trim())
    }
    try {
        $remote = (& git -C $ProjectRoot remote get-url origin 2>$null | Out-String).Trim()
        if ($LASTEXITCODE -eq 0 -and $remote) {
            $clean = Protect-DiagnosticText $remote
            if ($clean -match '(?:github\.com[:/]|/)([^/\s:]+/[^/\s]+?)(?:\.git)?$') {
                return $Matches[1]
            }
            return $clean
        }
    } catch {}
    return 'unavailable'
}

function Get-DiskInfo([string]$DriveLetter) {
    if (!$DriveLetter) {
        return [ordered]@{ drive = 'unavailable'; total_gb = 'unavailable'; free_gb = 'unavailable'; media_type = 'unavailable' }
    }
    $letter = ($DriveLetter.TrimEnd(':', '\', '/')).ToUpperInvariant()
    $totalGb = 'unavailable'
    $freeGb = 'unavailable'
    $mediaType = 'unavailable'
    try {
        $driveInfo = [IO.DriveInfo]::new($letter)
        if ($driveInfo.IsReady) {
            $totalGb = [Math]::Round($driveInfo.TotalSize / 1GB, 2)
            $freeGb = [Math]::Round($driveInfo.AvailableFreeSpace / 1GB, 2)
        }
    } catch {}
    try {
        $partition = Get-Partition -DriveLetter $letter -ErrorAction Stop | Select-Object -First 1
        if ($partition) {
            $disk = Get-PhysicalDisk -ErrorAction Stop | Where-Object { $_.DeviceId -eq [string]$partition.DiskNumber } | Select-Object -First 1
            if ($disk) {
                $rawType = [string]$disk.MediaType
                $busType = [string]$disk.BusType
                if ($busType -eq 'NVMe') { $mediaType = 'NVMe SSD' }
                elseif ($rawType -and $rawType -ne 'Unspecified') { $mediaType = $rawType }
                elseif ($busType -and $busType -ne 'Unknown') { $mediaType = $busType }
            }
        }
    } catch {}
    return [ordered]@{
        drive = "${letter}:"
        total_gb = $totalGb
        free_gb = $freeGb
        media_type = $mediaType
    }
}

function Get-DirectorySizeMb([string[]]$Paths, [string]$ProjectRoot) {
    $totalBytes = [int64]0
    $foundAny = $false
    foreach ($rawPath in $Paths) {
        if (!$rawPath) { continue }
        $expanded = $rawPath
        if ($expanded.StartsWith('~/') -or $expanded.StartsWith('~\')) {
            $expanded = Join-Path $HOME $expanded.Substring(2)
        } elseif (![IO.Path]::IsPathRooted($expanded)) {
            $expanded = Join-Path $ProjectRoot $expanded
        }
        if (!(Test-Path -LiteralPath $expanded)) { continue }
        $foundAny = $true
        try {
            $item = Get-Item -LiteralPath $expanded -ErrorAction Stop
            if ($item.PSIsContainer) {
                $dirInfo = [IO.DirectoryInfo]::new($item.FullName)
                foreach ($file in $dirInfo.EnumerateFiles('*', [IO.SearchOption]::AllDirectories)) {
                    $totalBytes += $file.Length
                }
            } else {
                $totalBytes += $item.Length
            }
        } catch {}
    }
    if (!$foundAny) { return 0.0 }
    return [Math]::Round($totalBytes / 1MB, 2)
}

function Resolve-CacheDiagnostics([string]$SpecJson, [string]$ProjectRoot) {
    $items = [ordered]@{}
    if (!$SpecJson) { return $items }
    try {
        $parsed = ConvertFrom-Json -InputObject $SpecJson -AsHashtable
        if ($parsed -is [System.Collections.IDictionary]) {
            foreach ($name in ($parsed.Keys | Sort-Object)) {
                $entry = $parsed[$name]
                $rawHit = if ($entry -is [System.Collections.IDictionary] -and $entry.ContainsKey('hit')) { [string]$entry['hit'] } else { '' }
                $hitValue = switch ($rawHit.ToLowerInvariant()) {
                    'true' { $true }
                    'false' { $false }
                    default { 'unavailable' }
                }
                $paths = if ($entry -is [System.Collections.IDictionary] -and $entry.ContainsKey('paths')) { @($entry['paths']) } else { @() }
                $sizeMb = if ($paths.Count) { Get-DirectorySizeMb $paths $ProjectRoot } else { 'unavailable' }
                $items[$name] = [ordered]@{
                    hit = $hitValue
                    restored_size_mb = $sizeMb
                }
            }
        }
    } catch {}
    return $items
}

function Get-EnvironmentSnapshot([string]$ProjectRoot, [string]$ImageLabel, [string]$LevelName, [int]$ParallelJobs) {
    # Runner & OS (explicit allowlist only; never inspect arbitrary environment variables).
    $osCim = $null
    $csCim = $null
    $cpus = @()
    try { $osCim = Get-CimInstance -ClassName Win32_OperatingSystem -ErrorAction Stop } catch {}
    try { $csCim = Get-CimInstance -ClassName Win32_ComputerSystem -ErrorAction Stop } catch {}
    try { $cpus = @(Get-CimInstance -ClassName Win32_Processor -ErrorAction Stop) } catch {}

    $winProduct = if ($osCim -and $osCim.Caption) { $osCim.Caption.Trim() } else { 'unavailable' }
    $winVersion = if ($osCim -and $osCim.Version) { $osCim.Version.Trim() } else { [Environment]::OSVersion.Version.ToString() }
    $winBuild = if ($osCim -and $osCim.BuildNumber) { $osCim.BuildNumber.Trim() } else { 'unavailable' }
    try {
        $cv = Get-ItemProperty -LiteralPath 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion' -ErrorAction Stop
        if ($cv.CurrentBuildNumber) {
            $ubr = if ($null -ne $cv.UBR) { ".$($cv.UBR)" } else { '' }
            $winBuild = "$($cv.CurrentBuildNumber)$ubr"
        }
        if ($cv.DisplayVersion -and $winVersion -ne 'unavailable') {
            $winVersion = "$winVersion ($($cv.DisplayVersion))"
        }
    } catch {}

    $runnerSection = [ordered]@{
        os = Get-ValueOrUnavailable $(if ($env:RUNNER_OS) { $env:RUNNER_OS } else { 'Windows' })
        image_label = Get-ValueOrUnavailable $ImageLabel
        image_os = Get-ValueOrUnavailable $env:ImageOS
        image_version = Get-ValueOrUnavailable $env:ImageVersion
        runner_environment = Get-ValueOrUnavailable $(if ($env:RUNNER_ENVIRONMENT) { $env:RUNNER_ENVIRONMENT } elseif ($env:GITHUB_ACTIONS -eq 'true') { 'github-actions' } else { 'local' })
        architecture = Get-ValueOrUnavailable $(if ($env:RUNNER_ARCH) { $env:RUNNER_ARCH } else { [Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString() })
        windows_product = Protect-DiagnosticText $winProduct
        windows_version = Protect-DiagnosticText $winVersion
        windows_build = Protect-DiagnosticText $winBuild
    }

    # CPU & virtualization
    $firstCpu = $cpus | Select-Object -First 1
    $physicalCores = 'unavailable'
    if ($cpus.Count) {
        $coreSum = ($cpus | Measure-Object -Property NumberOfCores -Sum).Sum
        if ($coreSum -gt 0) { $physicalCores = [int]$coreSum }
    }
    $logicalProcessors = [Environment]::ProcessorCount
    if ($logicalProcessors -le 0) { $logicalProcessors = 'unavailable' }
    $baseClock = if ($firstCpu -and $firstCpu.MaxClockSpeed) { [int]$firstCpu.MaxClockSpeed } else { 'unavailable' }
    $currClock = if ($firstCpu -and $firstCpu.CurrentClockSpeed) { [int]$firstCpu.CurrentClockSpeed } else { 'unavailable' }
    $hypervisor = if ($csCim -and $null -ne $csCim.HypervisorPresent) { [bool]$csCim.HypervisorPresent } else { 'unavailable' }
    $virtFirmware = if ($firstCpu -and $null -ne $firstCpu.VirtualizationFirmwareEnabled) { [bool]$firstCpu.VirtualizationFirmwareEnabled } else { 'unavailable' }

    $cpuSection = [ordered]@{
        manufacturer = Get-ValueOrUnavailable $(if ($firstCpu -and $firstCpu.Manufacturer) { $firstCpu.Manufacturer.Trim() } else { $null })
        model = Get-ValueOrUnavailable $(if ($firstCpu -and $firstCpu.Name) { ($firstCpu.Name -replace '\s+', ' ').Trim() } else { $null })
        logical_processors = $logicalProcessors
        physical_cores = $physicalCores
        base_clock_mhz = $baseClock
        current_clock_mhz = $currClock
        virtualization = [ordered]@{
            hypervisor_present = $hypervisor
            firmware_enabled = $virtFirmware
            system_manufacturer = Get-ValueOrUnavailable $(if ($csCim -and $csCim.Manufacturer) { $csCim.Manufacturer.Trim() } else { $null })
            system_model = Get-ValueOrUnavailable $(if ($csCim -and $csCim.Model) { $csCim.Model.Trim() } else { $null })
        }
    }

    # Memory
    $memorySection = [ordered]@{
        total_physical_mb = if ($osCim -and $osCim.TotalVisibleMemorySize) { [Math]::Round($osCim.TotalVisibleMemorySize / 1024.0, 1) } else { 'unavailable' }
        available_physical_mb = if ($osCim -and $osCim.FreePhysicalMemory) { [Math]::Round($osCim.FreePhysicalMemory / 1024.0, 1) } else { 'unavailable' }
        commit_limit_mb = if ($osCim -and $osCim.TotalVirtualMemorySize) { [Math]::Round($osCim.TotalVirtualMemorySize / 1024.0, 1) } else { 'unavailable' }
        commit_available_mb = if ($osCim -and $osCim.FreeVirtualMemory) { [Math]::Round($osCim.FreeVirtualMemory / 1024.0, 1) } else { 'unavailable' }
    }

    # Disks
    $sysDrive = if ($env:SystemDrive) { $env:SystemDrive } else { 'C:' }
    $wsDrive = [IO.Path]::GetPathRoot([IO.Path]::GetFullPath($ProjectRoot))
    $disksSection = [ordered]@{
        system = Get-DiskInfo $sysDrive
        workspace = Get-DiskInfo $wsDrive
    }

    # Toolchain (Visual Studio, MSVC, CMake, Ninja, Rustc, Cargo, PowerShell)
    $vsVersion = 'unavailable'
    $vsInstallPath = $null
    $vswhere = if (${env:ProgramFiles(x86)}) { Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe' } else { $null }
    if ($vswhere -and (Test-Path -LiteralPath $vswhere)) {
        try {
            $vsJson = & $vswhere -version '[18.0,19.0)' -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -format json 2>$null | Out-String
            $vsItems = if ($vsJson.Trim()) { @(ConvertFrom-Json -InputObject $vsJson) } else { @() }
            if (!$vsItems.Count) {
                $vsJson = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -format json 2>$null | Out-String
                $vsItems = if ($vsJson.Trim()) { @(ConvertFrom-Json -InputObject $vsJson) } else { @() }
            }
            if ($vsItems.Count) {
                $vs = $vsItems[0]
                $vsInstallPath = $vs.installationPath
                $vsVersion = Protect-DiagnosticText ("$($vs.displayName) ($($vs.installationVersion))".Trim())
            }
        } catch {}
    }
    $msvcVersion = 'unavailable'
    if ($env:VCToolsVersion) {
        $msvcVersion = Protect-DiagnosticText $env:VCToolsVersion.Trim()
    } elseif ($vsInstallPath) {
        $vcDefault = Join-Path $vsInstallPath 'VC/Auxiliary/Build/Microsoft.VCToolsVersion.default.txt'
        if (Test-Path -LiteralPath $vcDefault) {
            $msvcVersion = Protect-DiagnosticText ((Get-Content -LiteralPath $vcDefault -Raw).Trim())
        }
    }
    $cmakeExe = (Get-Command cmake -ErrorAction SilentlyContinue | Select-Object -First 1).Source
    if (!$cmakeExe -and $vsInstallPath) {
        $cand = Join-Path $vsInstallPath 'Common7/IDE/CommonExtensions/Microsoft/CMake/CMake/bin/cmake.exe'
        if (Test-Path -LiteralPath $cand) { $cmakeExe = $cand }
    }
    $ctestExe = (Get-Command ctest -ErrorAction SilentlyContinue | Select-Object -First 1).Source
    if (!$ctestExe -and $vsInstallPath) {
        $cand = Join-Path $vsInstallPath 'Common7/IDE/CommonExtensions/Microsoft/CMake/CMake/bin/ctest.exe'
        if (Test-Path -LiteralPath $cand) { $ctestExe = $cand }
    }
    $ninjaExe = (Get-Command ninja -ErrorAction SilentlyContinue | Select-Object -First 1).Source
    if (!$ninjaExe -and $vsInstallPath) {
        $cand = Join-Path $vsInstallPath 'Common7/IDE/CommonExtensions/Microsoft/CMake/Ninja/ninja.exe'
        if (Test-Path -LiteralPath $cand) { $ninjaExe = $cand }
    }
    $rustcExe = (Get-Command rustc -ErrorAction SilentlyContinue | Select-Object -First 1).Source
    $cargoExe = (Get-Command cargo -ErrorAction SilentlyContinue | Select-Object -First 1).Source

    $toolchainSection = [ordered]@{
        visual_studio = $vsVersion
        msvc = $msvcVersion
        cmake = Get-CommandFirstLine $cmakeExe @('--version')
        ninja = Get-CommandFirstLine $ninjaExe @('--version')
        rustc = Get-CommandFirstLine $rustcExe @('--version')
        cargo = Get-CommandFirstLine $cargoExe @('--version')
        powershell = $PSVersionTable.PSVersion.ToString()
    }

    # Git & Test context
    $branch = 'unavailable'
    if ($env:GITHUB_HEAD_REF) { $branch = $env:GITHUB_HEAD_REF.Trim() }
    elseif ($env:GITHUB_REF_NAME) { $branch = $env:GITHUB_REF_NAME.Trim() }
    else {
        try {
            $b = (& git -C $ProjectRoot rev-parse --abbrev-ref HEAD 2>$null | Out-String).Trim()
            if ($LASTEXITCODE -eq 0 -and $b) { $branch = $b }
        } catch {}
    }
    $commitSha = 'unavailable'
    if ($env:GITHUB_SHA) { $commitSha = $env:GITHUB_SHA.Trim() }
    else {
        try {
            $s = (& git -C $ProjectRoot rev-parse HEAD 2>$null | Out-String).Trim()
            if ($LASTEXITCODE -eq 0 -and $s) { $commitSha = $s }
        } catch {}
    }
    $gitAndTestSection = [ordered]@{
        repository = Get-SafeRepositoryName $ProjectRoot
        branch = Protect-DiagnosticText $branch
        commit_sha = Protect-DiagnosticText $commitSha
        ctest_version = Get-CommandFirstLine $ctestExe @('--version')
        ctest_parallel = if ($ParallelJobs -gt 0) { $ParallelJobs } else { 'unavailable' }
        test_level = Get-ValueOrUnavailable $LevelName
        selected_test_count = 'unavailable'
        github_run_id = Get-ValueOrUnavailable $env:GITHUB_RUN_ID
        github_run_attempt = Get-ValueOrUnavailable $env:GITHUB_RUN_ATTEMPT
        github_workflow = Get-ValueOrUnavailable $env:GITHUB_WORKFLOW
        github_job = Get-ValueOrUnavailable $env:GITHUB_JOB
    }

    return [ordered]@{
        runner = $runnerSection
        cpu = $cpuSection
        memory = $memorySection
        disks = $disksSection
        toolchain = $toolchainSection
        git_and_test = $gitAndTestSection
    }
}

function Find-LatestTestReport([string]$ProjectRoot, [Nullable[DateTimeOffset]]$NotBeforeUtc = $null) {
    $reportsDir = Join-Path $ProjectRoot 'build/reports'
    if (!(Test-Path -LiteralPath $reportsDir)) { return $null }
    $cutoff = if ($null -ne $NotBeforeUtc) { ([DateTimeOffset]$NotBeforeUtc).UtcDateTime.AddSeconds(-2) } else { $null }
    $candidate = Get-ChildItem -LiteralPath $reportsDir -Directory -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTimeUtc -Descending |
        ForEach-Object { Join-Path $_.FullName 'report.json' } |
        Where-Object {
            if (!(Test-Path -LiteralPath $_)) { return $false }
            if ($null -ne $cutoff) {
                return ((Get-Item -LiteralPath $_).LastWriteTimeUtc -ge $cutoff)
            }
            return $true
        } |
        Select-Object -First 1
    return $candidate
}

function Format-EnvironmentSummary($Payload) {
    $r = $Payload.environment.runner
    $c = $Payload.environment.cpu
    $m = $Payload.environment.memory
    $d = $Payload.environment.disks
    $t = $Payload.environment.toolchain
    $g = $Payload.environment.git_and_test
    $lines = @(
        '=== Runner Environment Diagnostics ==='
        "Runner      : OS=$($r.os) | Label=$($r.image_label) | Image=$($r.image_version) | Arch=$($r.architecture) | Env=$($r.runner_environment)"
        "Windows     : $($r.windows_product) | Version=$($r.windows_version) | Build=$($r.windows_build)"
        "CPU         : $($c.manufacturer) | $($c.model) | Logical=$($c.logical_processors) | Physical=$($c.physical_cores) | Clock(MHz) base=$($c.base_clock_mhz) curr=$($c.current_clock_mhz)"
        "Hypervisor  : Present=$($c.virtualization.hypervisor_present) | FirmwareVirt=$($c.virtualization.firmware_enabled) | System=$($c.virtualization.system_manufacturer) / $($c.virtualization.system_model)"
        "Memory (MB) : Total=$($m.total_physical_mb) | Available=$($m.available_physical_mb) | CommitLimit=$($m.commit_limit_mb) | CommitAvail=$($m.commit_available_mb)"
        "Disks (GB)  : System($($d.system.drive))=$($d.system.free_gb)/$($d.system.total_gb) ($($d.system.media_type)) | Workspace($($d.workspace.drive))=$($d.workspace.free_gb)/$($d.workspace.total_gb) ($($d.workspace.media_type))"
        "Toolchain   : VS=$($t.visual_studio) | MSVC=$($t.msvc) | CMake=$($t.cmake) | Ninja=$($t.ninja)"
        "Rust / Pwsh : Rustc=$($t.rustc) | Cargo=$($t.cargo) | PowerShell=$($t.powershell)"
        "Git / CTest : Repo=$($g.repository) | Branch=$($g.branch) | Commit=$($g.commit_sha) | CTest=$($g.ctest_version) | Level=$($g.test_level) | Parallel=$($g.ctest_parallel) | Selected=$($g.selected_test_count)"
    )
    if ($Payload.cache -and $Payload.cache.Count) {
        $cacheParts = @(foreach ($k in $Payload.cache.Keys) {
            $entry = $Payload.cache[$k]
            "${k}:hit=$($entry.hit)/size=$($entry.restored_size_mb)MB"
        })
        $lines += "Cache       : $($cacheParts -join ' | ')"
    }
    return $lines
}

$existing = $null
if ($Phase -eq 'post' -and (Test-Path -LiteralPath $OutputPath)) {
    try { $existing = Get-Content -LiteralPath $OutputPath -Raw | ConvertFrom-Json -AsHashtable } catch {}
}

$environment = if ($existing -and $existing.ContainsKey('environment')) {
    $existing['environment']
} else {
    Get-EnvironmentSnapshot -ProjectRoot $root -ImageLabel $RunnerLabel -LevelName $TestLevel -ParallelJobs $Jobs
}
$cacheDiag = Resolve-CacheDiagnostics -SpecJson $CacheSpecJson -ProjectRoot $root
if (!$cacheDiag.Count -and $existing -and $existing.ContainsKey('cache')) {
    $cacheDiag = $existing['cache']
}

$runtimePerf = [ordered]@{
    status = if ($Phase -eq 'pre') { 'pending' } else { 'unavailable' }
    wall_seconds = 'unavailable'
    ctest_parallel = if ($Jobs -gt 0) { $Jobs } else { 'unavailable' }
    cpu_avg_percent = 'unavailable'
    cpu_peak_percent = 'unavailable'
    ram_peak_used_mb = 'unavailable'
    ram_peak_percent = 'unavailable'
    sample_count = 0
    stage_timings = @()
    test_durations = @()
}

if ($Phase -eq 'post') {
    $notBefore = $null
    if ($existing -and $existing.ContainsKey('collected_utc') -and $existing['collected_utc']) {
        $parsedUtc = [DateTimeOffset]::MinValue
        if ([DateTimeOffset]::TryParse([string]$existing['collected_utc'], [ref]$parsedUtc)) {
            $notBefore = [Nullable[DateTimeOffset]]$parsedUtc
        }
    }
    $reportFile = if ($TestReportPath -and (Test-Path -LiteralPath $TestReportPath)) { $TestReportPath } else { Find-LatestTestReport $root $notBefore }
    if ($reportFile -and (Test-Path -LiteralPath $reportFile)) {
        try {
            $testReport = Get-Content -LiteralPath $reportFile -Raw | ConvertFrom-Json
            $selectedCount = if ($testReport.selection -and $null -ne $testReport.selection.selected) { @($testReport.selection.selected).Count } else { 'unavailable' }
            $environment['git_and_test']['selected_test_count'] = $selectedCount
            if ($testReport.level) { $environment['git_and_test']['test_level'] = [string]$testReport.level }
            $parallelFromReport = if ($testReport.PSObject.Properties['ctest_parallel'] -and $testReport.ctest_parallel) { [int]$testReport.ctest_parallel } elseif ($Jobs -gt 0) { $Jobs } else { 'unavailable' }
            $environment['git_and_test']['ctest_parallel'] = $parallelFromReport

            $resUsage = if ($testReport.PSObject.Properties['resource_usage']) { $testReport.resource_usage } else { $null }
            $stageRows = @()
            if ($testReport.timings) {
                $stageRows = @(
                    $testReport.timings | Where-Object { $_.category -like 'build.*' -or $_.category -like 'ctest.*' -or $_.category -like 'rust.*' -or $_.category -like 'core.*' } |
                    ForEach-Object {
                        [ordered]@{
                            category = [string]$_.category
                            name = Protect-DiagnosticText ([string]$_.name)
                            status = [string]$_.status
                            duration_seconds = [Math]::Round([double]$_.duration_ms / 1000.0, 3)
                            exclusive_seconds = if ($null -ne $_.exclusive_duration_ms) { [Math]::Round([double]$_.exclusive_duration_ms / 1000.0, 3) } else { 'unavailable' }
                        }
                    }
                )
            }
            $testRows = @()
            if ($testReport.results) {
                $testRows = @(
                    $testReport.results |
                    Sort-Object @{ Expression = { if ($null -ne $_.duration_seconds) { [double]$_.duration_seconds } else { -1.0 } }; Descending = $true }, name |
                    ForEach-Object {
                        [ordered]@{
                            name = [string]$_.name
                            preset = [string]$_.preset
                            status = [string]$_.status
                            duration_seconds = if ($null -ne $_.duration_seconds) { [Math]::Round([double]$_.duration_seconds, 3) } else { 'unavailable' }
                        }
                    }
                )
            }
            $runtimePerf = [ordered]@{
                status = Get-ValueOrUnavailable $testReport.status
                wall_seconds = if ($null -ne $testReport.wall_seconds) { [Math]::Round([double]$testReport.wall_seconds, 3) } else { 'unavailable' }
                ctest_parallel = $parallelFromReport
                cpu_avg_percent = if ($resUsage -and $null -ne $resUsage.cpu_avg_percent) { $resUsage.cpu_avg_percent } else { 'unavailable' }
                cpu_peak_percent = if ($resUsage -and $null -ne $resUsage.cpu_peak_percent) { $resUsage.cpu_peak_percent } else { 'unavailable' }
                ram_peak_used_mb = if ($resUsage -and $null -ne $resUsage.ram_peak_used_mb) { $resUsage.ram_peak_used_mb } else { 'unavailable' }
                ram_peak_percent = if ($resUsage -and $null -ne $resUsage.ram_peak_percent) { $resUsage.ram_peak_percent } else { 'unavailable' }
                sample_count = if ($resUsage -and $null -ne $resUsage.sample_count) { [int]$resUsage.sample_count } else { 0 }
                stage_timings = $stageRows
                test_durations = $testRows
            }
        } catch {}
    }
}

$payload = [ordered]@{
    schema = 1
    phase = $Phase
    collected_utc = [DateTimeOffset]::UtcNow.ToString('O')
    environment = $environment
    cache = $cacheDiag
    runtime_performance = $runtimePerf
    comparison_dimensions = [ordered]@{
        hardware = [ordered]@{
            cpu_model = $environment['cpu']['model']
            logical_processors = $environment['cpu']['logical_processors']
            physical_cores = $environment['cpu']['physical_cores']
            total_physical_mb = $environment['memory']['total_physical_mb']
            workspace_media_type = $environment['disks']['workspace']['media_type']
        }
        runner_image = [ordered]@{
            image_label = $environment['runner']['image_label']
            image_version = $environment['runner']['image_version']
            windows_build = $environment['runner']['windows_build']
        }
        toolchain = $environment['toolchain']
        cache = $cacheDiag
        parallelism_and_scope = [ordered]@{
            test_level = $environment['git_and_test']['test_level']
            ctest_parallel = $environment['git_and_test']['ctest_parallel']
            selected_test_count = $environment['git_and_test']['selected_test_count']
        }
        stage_durations_seconds = [ordered]@{
            wall_seconds = $runtimePerf['wall_seconds']
            stages = $runtimePerf['stage_timings']
        }
    }
    attribution_notes = @(
        'Cross-run duration differences must distinguish hardware differences (CPU/RAM/disk), cache hit and restored size differences, CTest parallelism or selected test scope differences, and unmeasured shared-host/virtualization variance.'
        'A different CPU model alone does not establish that CPU hardware caused the observed runtime difference.'
        'Fields marked unavailable could not be reliably queried in this environment and are never estimated.'
    )
}

$parentDir = Split-Path -Parent $OutputPath
if ($parentDir) { New-Item -ItemType Directory -Path $parentDir -Force | Out-Null }
$jsonText = $payload | ConvertTo-Json -Depth 12
[IO.File]::WriteAllText($OutputPath, $jsonText, [Text.UTF8Encoding]::new($false))

if (!$Quiet) {
    $inActions = $env:GITHUB_ACTIONS -eq 'true'
    if ($Phase -eq 'pre') {
        if ($inActions) { Write-Host '::group::Runner Environment Diagnostics' }
        foreach ($line in (Format-EnvironmentSummary $payload)) { Write-Host $line }
        Write-Host "Diagnostics JSON: $OutputPath"
        if ($inActions) { Write-Host '::endgroup::' }
    } else {
        if ($inActions) { Write-Host '::group::Runner Performance Summary' }
        Write-Host "=== Runner Performance Summary ($($environment['git_and_test']['test_level'])) ==="
        Write-Host ("Status=$($runtimePerf['status']) | Wall=$($runtimePerf['wall_seconds'])s | Parallel=$($runtimePerf['ctest_parallel']) | Selected=$($environment['git_and_test']['selected_test_count'])")
        Write-Host ("CPU Avg/Peak=$($runtimePerf['cpu_avg_percent'])%/$($runtimePerf['cpu_peak_percent'])% | RAM Peak=$($runtimePerf['ram_peak_used_mb'])MB ($($runtimePerf['ram_peak_percent'])%) | Samples=$($runtimePerf['sample_count'])")
        if ($runtimePerf['test_durations'].Count) {
            $topSlow = @($runtimePerf['test_durations'] | Select-Object -First 5 | ForEach-Object { "$($_.name)=$($_.duration_seconds)s" })
            Write-Host ("Slowest tests : $($topSlow -join ', ')")
        }
        Write-Host "Diagnostics JSON: $OutputPath"
        if ($inActions) { Write-Host '::endgroup::' }
    }
}
