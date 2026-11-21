#Requires -Version 7
param(
    [Parameter(Mandatory)][ValidateSet('CI','FAST','AFFECTED','FULL','RELEASE')][string]$Level,
    [string]$BaseRef='origin/main',
    [ValidateSet('Branch','Commit','Worktree')][string]$ChangeScope='Branch',
    [ValidateRange(1,32)][int]$Jobs=4,
    [switch]$RetryFailures,
    [switch]$CiSafe,
    [switch]$PlanOnly,
    [ValidateSet('x64','x86')][string]$Architecture='x64',
    [ValidateRange(0,1024)][double]$TestLoad=0,
    [ValidateSet('fail-fast','continue')][string]$FailurePolicy='fail-fast',
    [string[]]$DiagnosticTests=@()
)
$ErrorActionPreference='Stop'
if($Architecture -eq 'x86' -and $Level -notin 'AFFECTED','RELEASE'){throw '-Architecture x86 is supported by AFFECTED/RELEASE; CI, FAST and FULL use the x64 baseline.'}
$root=Split-Path $PSScriptRoot -Parent
Import-Module "$PSScriptRoot/test-support.psm1" -ErrorAction Stop
Import-Module "$PSScriptRoot/test-plan.psm1" -ErrorAction Stop
Import-Module "$PSScriptRoot/release-support.psm1" -ErrorAction Stop
$channel=Get-Content -LiteralPath "$root/RELEASE-CHANNEL.json" -Raw|ConvertFrom-Json
$version=$channel.version
$sourceStart=$null;$sourceEnd=$null;$releaseData=$null;$snapshot=$null;$packagingPermit=$null
$requestedLevel=$Level
$changeSet=if($Level -eq 'CI'){[pscustomobject]@{Paths=@();Error=$null;Range=$null;EffectiveBase=$null}}else{Get-VerificationChanges $root $BaseRef $ChangeScope}
# Only genuinely unbounded x86 impact needs dual-architecture release acceptance.
# Determine the effective level before naming the report/receipt directory.
if($Level -eq 'AFFECTED' -and $Architecture -eq 'x86' -and ($changeSet.Error -or (Select-VerificationPlan $changeSet.Paths).Full)){$Level='RELEASE'}
$runId=[guid]::NewGuid().ToString('N')
$reportRoot=Join-Path $root "build/reports/$($Level.ToLowerInvariant())-$((Get-Date).ToString('yyyyMMdd-HHmmss'))-$($runId.Substring(0,8))"
$eventsRoot=Join-Path $reportRoot 'events'
$null=New-Item -ItemType Directory -Path $eventsRoot -Force
$consoleLog=Join-Path $reportRoot 'console.log'
Start-Transcript -Path $consoleLog -Force | Out-Null
$previousRunId=$env:SHANJIE_TEST_RUN_ID
$previousTimingsDirectory=$env:SHANJIE_TEST_TIMINGS_DIR
$previousLevel=$env:SHANJIE_TEST_LEVEL
$previousCoreReuse=$env:SHANJIE_REUSE_HEAVY_CORE_RESULTS
$previousSnapshotId=$env:SHANJIE_RELEASE_SNAPSHOT_ID
$previousSnapshotHash=$env:SHANJIE_RELEASE_SNAPSHOT_SHA256
$previousPermit=$env:SHANJIE_RELEASE_PACKAGING_PERMIT
$previousStopOnFailure=$env:SHANJIE_TEST_STOP_ON_FAILURE
# A nested test invocation must not inherit another run's sealed inputs/permit.
$env:SHANJIE_RELEASE_SNAPSHOT_ID=$null
$env:SHANJIE_RELEASE_SNAPSHOT_SHA256=$null
$env:SHANJIE_RELEASE_PACKAGING_PERMIT=$null
$env:SHANJIE_TEST_STOP_ON_FAILURE=if($FailurePolicy -eq 'fail-fast'){'1'}else{$null}
$env:SHANJIE_TEST_LEVEL=$Level
$env:SHANJIE_REUSE_HEAVY_CORE_RESULTS=if(Test-BetaCoreResultReuse $Level ([bool]$CiSafe) ($channel.prerelease -eq $true)){'1'}else{$null}
$env:SHANJIE_TEST_RUN_ID=$runId
$env:SHANJIE_TEST_TIMINGS_DIR=$eventsRoot
$whole=[Diagnostics.Stopwatch]::StartNew()
$resourceSampler=$null
function Start-ResourceSampler([int]$IntervalMs = 1000) {
    try {
        if (-not ('Shanjie.NativePerfSampler' -as [type])) {
            Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using System.Threading;
namespace Shanjie {
    public sealed class NativePerfSampler : IDisposable {
        [StructLayout(LayoutKind.Sequential)]
        public struct FILETIME { public uint Low; public uint High; }
        [StructLayout(LayoutKind.Sequential)]
        public struct MEMORYSTATUSEX {
            public uint dwLength;
            public uint dwMemoryLoad;
            public ulong ullTotalPhys;
            public ulong ullAvailPhys;
            public ulong ullTotalPageFile;
            public ulong ullAvailPageFile;
            public ulong ullTotalVirtual;
            public ulong ullAvailVirtual;
            public ulong ullAvailExtendedVirtual;
        }
        [DllImport("kernel32.dll", SetLastError = true)]
        public static extern bool GetSystemTimes(out FILETIME idle, out FILETIME kernel, out FILETIME user);
        [DllImport("kernel32.dll", SetLastError = true)]
        public static extern bool GlobalMemoryStatusEx(ref MEMORYSTATUSEX stat);
        public static ulong ToUInt64(FILETIME ft) { return ((ulong)ft.High << 32) | ft.Low; }
        public static bool Sample(out ulong idle, out ulong total, out double usedMb, out double usedPct) {
            idle = 0; total = 0; usedMb = 0; usedPct = 0;
            FILETIME i, k, u;
            if (!GetSystemTimes(out i, out k, out u)) return false;
            idle = ToUInt64(i);
            total = ToUInt64(k) + ToUInt64(u);
            var m = new MEMORYSTATUSEX();
            m.dwLength = (uint)Marshal.SizeOf(typeof(MEMORYSTATUSEX));
            if (!GlobalMemoryStatusEx(ref m) || m.ullTotalPhys == 0) return false;
            usedMb = (m.ullTotalPhys - m.ullAvailPhys) / (1024.0 * 1024.0);
            usedPct = (double)(m.ullTotalPhys - m.ullAvailPhys) * 100.0 / (double)m.ullTotalPhys;
            return true;
        }
        private readonly object _gate = new object();
        private Timer _timer;
        private ulong _prevIdle;
        private ulong _prevTotal;
        private double _cpuSum;
        private int _cpuCount;
        private double _cpuPeak;
        private double _ramPeakMb;
        private double _ramPeakPct;
        private bool _stopped;
        private NativePerfSampler(ulong i0, ulong t0, double m0, double p0, int intervalMs) {
            _prevIdle = i0;
            _prevTotal = t0;
            _ramPeakMb = m0;
            _ramPeakPct = p0;
            _timer = new Timer(_ => Record(), null, intervalMs, intervalMs);
        }
        public static NativePerfSampler Start(int intervalMs) {
            if (intervalMs <= 0) intervalMs = 1000;
            ulong i0, t0; double m0, p0;
            if (!Sample(out i0, out t0, out m0, out p0)) return null;
            return new NativePerfSampler(i0, t0, m0, p0, intervalMs);
        }
        private void Record() {
            lock (_gate) {
                if (_stopped) return;
                RecordLocked();
            }
        }
        private void RecordLocked() {
            ulong i1, t1; double m1, p1;
            if (!Sample(out i1, out t1, out m1, out p1)) return;
            if (t1 > _prevTotal) {
                ulong dTotal = t1 - _prevTotal;
                ulong dIdle = i1 >= _prevIdle ? (i1 - _prevIdle) : 0;
                double busy = Math.Clamp((1.0 - ((double)dIdle / (double)dTotal)) * 100.0, 0.0, 100.0);
                _cpuSum += busy;
                _cpuCount++;
                if (busy > _cpuPeak) _cpuPeak = busy;
            }
            _prevIdle = i1;
            _prevTotal = t1;
            if (m1 > _ramPeakMb) _ramPeakMb = m1;
            if (p1 > _ramPeakPct) _ramPeakPct = p1;
        }
        public void StopAndSnapshot(out int cpuCount, out double cpuAvg, out double cpuPeak, out double ramPeakMb, out double ramPeakPct) {
            lock (_gate) {
                if (!_stopped) {
                    _stopped = true;
                    RecordLocked();
                }
                cpuCount = _cpuCount;
                cpuAvg = _cpuCount > 0 ? _cpuSum / _cpuCount : 0.0;
                cpuPeak = _cpuPeak;
                ramPeakMb = _ramPeakMb;
                ramPeakPct = _ramPeakPct;
            }
            Dispose();
        }
        public void Dispose() {
            Timer t;
            lock (_gate) {
                _stopped = true;
                t = _timer;
                _timer = null;
            }
            if (t != null) t.Dispose();
        }
    }
}
'@ -ErrorAction Stop
        }
        return [Shanjie.NativePerfSampler]::Start($IntervalMs)
    } catch {
        return $null
    }
}
function Stop-ResourceSampler($Sampler) {
    if (!$Sampler) {
        return [ordered]@{ cpu_avg_percent = 'unavailable'; cpu_peak_percent = 'unavailable'; ram_peak_used_mb = 'unavailable'; ram_peak_percent = 'unavailable'; sample_count = 0 }
    }
    [int]$cpuCount = 0; [double]$cpuAvg = 0; [double]$cpuPeak = 0; [double]$ramPeakMb = 0; [double]$ramPeakPct = 0
    $Sampler.StopAndSnapshot([ref]$cpuCount, [ref]$cpuAvg, [ref]$cpuPeak, [ref]$ramPeakMb, [ref]$ramPeakPct)
    return [ordered]@{
        cpu_avg_percent = if ($cpuCount -gt 0) { [Math]::Round($cpuAvg, 1) } else { 'unavailable' }
        cpu_peak_percent = if ($cpuCount -gt 0) { [Math]::Round($cpuPeak, 1) } else { 'unavailable' }
        ram_peak_used_mb = [Math]::Round($ramPeakMb, 1)
        ram_peak_percent = [Math]::Round($ramPeakPct, 1)
        sample_count = $cpuCount
    }
}
$changedPaths=@()
$pathReasons=[ordered]@{}
$selectedByPreset=[ordered]@{}
$unselectedByPreset=[ordered]@{}
$inventoryByPreset=[ordered]@{}
$schedulingByPreset=[ordered]@{}
$presets=@()
$testResults=@()
$ctestReports=@()
$diagnosticReports=@()
$diagnosticResults=@()
$stoppedAfterWave=$null
$overallStatus='passed'
$failureMessage=$null
$requestedLabels=[Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
$forceFull=$false
$skipTests=$false
$needsProduct=$false
$skipSettingsBuild=$false
$notes=[Collections.Generic.List[string]]::new()
$additionalVerification=@()

function Add-Label([string]$Label,[string]$Reason) {
    [void]$requestedLabels.Add($Label)
    $pathReasons[$Label]=$Reason
}
function Get-ChangedPathList([string]$Ref,[string]$Scope='Branch') {
    Get-VerificationChanges $root $Ref $Scope
}
function Test-DocumentationOnly([string]$Path) {
    return (Test-VerificationDocumentation $Path)
}
function Classify-ChangedPath([string]$Path) {
    if (Test-DocumentationOnly $Path) { $pathReasons[$Path]='Documentation-only change; no build or tests.'; return }
    $impact=Get-VerificationImpact $Path
    $pathReasons[$Path]=$impact.Reason
    if($impact.Full){$script:forceFull=$true}
    foreach($label in $impact.Labels){Add-Label $label "$Path`: $($impact.Reason)"}
}
function Get-CtestInventory([string]$BinaryDirectory) {
    $raw=& ctest --test-dir $BinaryDirectory -C Release --show-only=json-v1 2>&1
    if ($LASTEXITCODE) { throw "CTest inventory failed for $BinaryDirectory`: $raw" }
    $text=($raw | Out-String).Trim()
    return (ConvertFrom-Json -InputObject $text)
}
function Get-TestLabels($Test) {
    $property=$Test.properties | Where-Object name -eq 'LABELS' | Select-Object -First 1
    if (!$property) { return @() }
    return @($property.value | ForEach-Object { ([string]$_ -split ';') })
}
function Get-TestFixtureProperty($Test,[string]$PropertyName) {
    $property=$Test.properties | Where-Object name -eq $PropertyName | Select-Object -First 1
    if (!$property) { return @() }
    return @($property.value | ForEach-Object { ([string]$_ -split ';') })
}
function Get-TestNumericProperty($Test,[string]$PropertyName,[double]$DefaultValue=0) {
    $property=$Test.properties | Where-Object name -eq $PropertyName | Select-Object -First 1
    if (!$property -or $null -eq $property.value) { return $DefaultValue }
    $parsed=0.0
    if ([double]::TryParse([string]$property.value,[Globalization.NumberStyles]::Float,[Globalization.CultureInfo]::InvariantCulture,[ref]$parsed)) { return $parsed }
    return $DefaultValue
}
function Get-CtestCostDataPath([string]$BinaryDirectory) {
    Join-Path $BinaryDirectory 'Testing/Temporary/CTestCostData.txt'
}
function Read-CtestPreviousFailures([string]$CostDataPath) {
    if (!$CostDataPath -or !(Test-Path -LiteralPath $CostDataPath)) { return @() }
    $lines = Get-Content -LiteralPath $CostDataPath
    $sepIndex = [Array]::IndexOf($lines, '---')
    if ($sepIndex -lt 0 -or $sepIndex -ge ($lines.Count - 1)) { return @() }
    return @($lines[($sepIndex + 1)..($lines.Count - 1)] | ForEach-Object { $_.Trim() } | Where-Object { $_ })
}
function Merge-CtestPreviousFailures([string]$CostDataPath, [string[]]$PreservedNames) {
    if (!$CostDataPath -or !$PreservedNames -or !$PreservedNames.Count -or !(Test-Path -LiteralPath $CostDataPath)) { return }
    $lines = @(Get-Content -LiteralPath $CostDataPath)
    $sepIndex = [Array]::IndexOf($lines, '---')
    $costRows = if ($sepIndex -ge 0) { if ($sepIndex -gt 0) { @($lines[0..($sepIndex - 1)]) } else { @() } } else { $lines }
    $existing = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    $failedRows = [Collections.Generic.List[string]]::new()
    if ($sepIndex -ge 0 -and $sepIndex -lt ($lines.Count - 1)) {
        foreach ($entry in $lines[($sepIndex + 1)..($lines.Count - 1)]) {
            $trimmed = $entry.Trim()
            if ($trimmed -and $existing.Add($trimmed)) { $failedRows.Add($trimmed) }
        }
    }
    foreach ($name in $PreservedNames) {
        if ($name -and $existing.Add($name)) { $failedRows.Add($name) }
    }
    $output = [Collections.Generic.List[string]]::new()
    foreach ($row in $costRows) { $output.Add($row) }
    $output.Add('---')
    foreach ($row in $failedRows) { $output.Add($row) }
    [IO.File]::WriteAllText($CostDataPath, (($output -join "`n") + "`n"), [Text.UTF8Encoding]::new($false))
}
# Separate multi-processor heavy suites (e.g. full.core-suite with PROCESSORS 4)
# into an upfront priority wave so CTest's single-slot backfill cannot starve
# them behind dozens of short 1-processor tests.
# When both FAST baseline checks and broader regressions are selected, gate
# subsequent waves behind the FAST baseline so baseline failures stop early.
function Get-CtestExecutionWaves($Inventory,[object[]]$Selected) {
    if (!$Selected -or !$Selected.Count) { return @() }
    $byName=@{}
    if ($Inventory -and $Inventory.tests) {
        foreach ($item in $Inventory.tests) { $byName[$item.name]=$item }
    }
    $waves=[Collections.Generic.List[object]]::new()
    $assigned=[Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    $completedFixtures=[Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)

    $fastRows=@(
        foreach ($row in $Selected) {
            $meta=$byName[$row.name]
            $labels=if($meta){@(Get-TestLabels $meta)}else{@()}
            if ('fast' -in $labels) { $row }
        }
    )
    if ($fastRows.Count -and $fastRows.Count -lt $Selected.Count) {
        $fastReq=[Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
        foreach ($row in $fastRows) {
            $meta=$byName[$row.name]
            if ($meta) {
                foreach ($fixture in (Get-TestFixtureProperty $meta 'FIXTURES_REQUIRED')) {
                    if ($fixture) { [void]$fastReq.Add($fixture) }
                }
            }
        }
        $fastWave=[Collections.Generic.List[object]]::new()
        foreach ($row in $Selected) {
            $meta=$byName[$row.name]
            if (!$meta) { continue }
            $setups=@(Get-TestFixtureProperty $meta 'FIXTURES_SETUP' | Where-Object { $_ -and $fastReq.Contains($_) })
            if ($setups.Count -and $assigned.Add($row.name)) {
                $fastWave.Add($row)
                foreach ($fixture in $setups) { [void]$completedFixtures.Add($fixture) }
            }
        }
        foreach ($row in $fastRows) {
            if ($assigned.Add($row.name)) {
                $fastWave.Add($row)
                $meta=$byName[$row.name]
                if ($meta) {
                    foreach ($fixture in (Get-TestFixtureProperty $meta 'FIXTURES_SETUP')) {
                        if ($fixture) { [void]$completedFixtures.Add($fixture) }
                    }
                }
            }
        }
        if ($fastWave.Count -lt $Selected.Count) {
            $waves.Add([pscustomobject]@{
                tag='fast'
                tests=@($fastWave)
                exclude_setup_fixtures=@()
                reason='FAST baseline gate runs first so baseline failures stop before heavy suites and broader regressions.'
            })
        } else {
            $assigned.Clear()
            $completedFixtures.Clear()
        }
    }

    $remainingPool=@($Selected | Where-Object { !$assigned.Contains($_.name) })
    $heavyMultiProc=@(
        foreach ($row in $remainingPool) {
            $meta=$byName[$row.name]
            if ($meta -and (Get-TestNumericProperty $meta 'PROCESSORS' 1) -ge 2) { $row }
        }
    )
    if ($heavyMultiProc.Count) {
        $excludeBeforeHeavy=@($completedFixtures | Sort-Object)
        $requiredFixtures=[Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
        foreach ($row in $heavyMultiProc) {
            $meta=$byName[$row.name]
            if ($meta) {
                foreach ($fixture in (Get-TestFixtureProperty $meta 'FIXTURES_REQUIRED')) {
                    if ($fixture -and !$completedFixtures.Contains($fixture)) { [void]$requiredFixtures.Add($fixture) }
                }
            }
        }
        $heavyWave=[Collections.Generic.List[object]]::new()
        foreach ($row in $remainingPool) {
            $meta=$byName[$row.name]
            if (!$meta) { continue }
            $setups=@(Get-TestFixtureProperty $meta 'FIXTURES_SETUP' | Where-Object { $_ -and $requiredFixtures.Contains($_) })
            if ($setups.Count -and $assigned.Add($row.name)) {
                $heavyWave.Add($row)
                foreach ($fixture in $setups) { [void]$completedFixtures.Add($fixture) }
            }
        }
        foreach ($row in $heavyMultiProc) {
            if ($assigned.Add($row.name)) {
                $heavyWave.Add($row)
                $meta=$byName[$row.name]
                if ($meta) {
                    foreach ($fixture in (Get-TestFixtureProperty $meta 'FIXTURES_SETUP')) {
                        if ($fixture) { [void]$completedFixtures.Add($fixture) }
                    }
                }
            }
        }
        $postHeavy=@($Selected | Where-Object { !$assigned.Contains($_.name) })
        if ($waves.Count -gt 0 -or $postHeavy.Count -gt 0) {
            $waves.Add([pscustomobject]@{
                tag='heavy'
                tests=@($heavyWave)
                exclude_setup_fixtures=$excludeBeforeHeavy
                reason='Priority wave for multi-processor heavy suites so 1-processor tests cannot starve them via backfill.'
            })
        } else {
            $assigned.Clear()
            $completedFixtures.Clear()
        }
    }

    $finalRemaining=@($Selected | Where-Object { !$assigned.Contains($_.name) })
    if ($waves.Count -eq 0) {
        return @(
            [pscustomobject]@{
                tag=''
                tests=@($Selected)
                exclude_setup_fixtures=@()
                reason='Single wave ordered by CTest per-architecture CTestCostData.txt and initial COST hints.'
            }
        )
    }
    if ($finalRemaining.Count) {
        $waves.Add([pscustomobject]@{
            tag='remaining'
            tests=@($finalRemaining)
            exclude_setup_fixtures=@($completedFixtures | Sort-Object)
            reason='Remaining 1-processor tests scheduled longest-first by per-architecture CTestCostData.txt and initial COST.'
        })
    }
    return @($waves)
}
function Get-SelectedTests($Inventory,[string[]]$Levels,[string]$Preset) {
    $selected=[Collections.Generic.List[object]]::new()
    $omitted=[Collections.Generic.List[object]]::new()
    foreach($test in $Inventory.tests) {
        $labels=Get-TestLabels $test
        $matches=@($labels | Where-Object { $_ -in $Levels })
        if($matches.Count -and $CiSafe -and 'ui' -in $labels -and 'ci-safe' -notin $labels) {
            $omitted.Add([pscustomobject]@{name=$test.name;labels=$labels;reason='Skipped in CI-safe mode: interactive WinUI/candidate-host checks require a local Windows verification session; CI still compiles the WinUI project.';preset=$Preset})
            continue
        }
        if ($matches.Count) {
            $reason=if($Level -in @('FAST','AFFECTED')) { ($matches | ForEach-Object { if($pathReasons.Contains($_)){$pathReasons[$_]}else{"Selected by $Level level."} }) -join ' ' } else { "$Level level selects the $($matches -join ', ') test group." }
            $selected.Add([pscustomobject]@{name=$test.name;labels=$labels;reason=$reason;preset=$Preset})
        } else {
            $reason=if($skipTests){'Documentation-only change; build and test not needed.'}elseif($Level -eq 'CI'){'Outside the fixed CI baseline; full product and desktop acceptance run locally or before release.'}elseif($Level -eq 'FAST'){'Outside the FAST smoke set.'}elseif($Level -eq 'AFFECTED'){'No selected label matches the changed modules.'}elseif($Level -eq 'FULL'){'Release-only check; outside FULL.'}elseif($Level -eq 'RELEASE' -and $Preset -eq 'windows-x86' -and $test.name -notlike 'release.*'){'x64 FULL suites are already run on the x64 preset; this x86 pass contains only x86 release checks.'}else{'Test is not selected for this architecture preset.'}
            $omitted.Add([pscustomobject]@{name=$test.name;labels=$labels;reason=$reason;preset=$Preset})
        }
    }
    $requiredFixtures=[Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach($test in $Inventory.tests | Where-Object name -In $selected.name) { foreach($fixture in (Get-TestFixtureProperty $test 'FIXTURES_REQUIRED')) { if($fixture){[void]$requiredFixtures.Add($fixture)} } }
    if($requiredFixtures.Count) {
        foreach($test in $Inventory.tests) {
            $setup=Get-TestFixtureProperty $test 'FIXTURES_SETUP'
            $missing=@($setup | Where-Object { $_ -in $requiredFixtures -and !($selected.name -contains $test.name) })
            if($missing.Count) {
                for($index=$omitted.Count-1;$index -ge 0;$index--) { if($omitted[$index].name -eq $test.name){$omitted.RemoveAt($index)} }
                $selected.Add([pscustomobject]@{name=$test.name;labels=(Get-TestLabels $test);reason="CTest fixture setup for $($missing -join ', ') is required by selected tests.";preset=$Preset})
            }
        }
    }
    return [pscustomobject]@{Selected=@($selected);Omitted=@($omitted)}
}
function Get-CmakeTargets([object[]]$Tests) {
    $targets=[Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach($test in $Tests) {
        $name=$test.name
        if($name -match '\.tsf-smoke\.') {[void]$targets.Add('shanjie_tsf_smoke')}
        if($name -match '\.word\.add\.') {[void]$targets.Add('shanjie_add_word_popup')}
        if($name -match '\.english\.') {[void]$targets.Add('shanjie_english_keys')}
        if($name -match '\.submission\.') {[void]$targets.Add('shanjie_submission')}
        if($name -match '\.private\.tsf-compat\.') {[void]$targets.Add('shanjie_tsf_compat')}
        if($name -match '\.private\.popup-lifecycle\.') {[void]$targets.Add('shanjie_popup_lifecycle')}
        if($name -match '\.ui\.candidate\.') {[void]$targets.Add('shanjie_candidate_host')}
        if($name -match '\.mode\.') {[void]$targets.Add('shanjie_mode_smoke')}
        if($name -match '\.interaction\.') {[void]$targets.Add('shanjie_windows_interaction')}
    }
    return @($targets)
}
function Invoke-CmakeBuild([string]$Preset,[object[]]$Tests) {
    $targets=@(Get-CmakeTargets $Tests)
    if (!$targets.Count) { return }
    $stage=Start-TestStage -Name "Incremental C++ harness build ($Preset)" -Category 'build.cpp.harness'
    $status='passed';$errorMessage=$null
    try {
        & cmake --build --preset $Preset --target @targets --config Release --parallel $Jobs
        if ($LASTEXITCODE) { throw "CMake harness build failed for $Preset." }
        Complete-TestStage -Stage $stage -Details @{preset=$Preset;targets=$targets;cache='CMake incremental build'}
    } catch { $status='failed';$errorMessage=$_.Exception.Message;Complete-TestStage -Stage $stage -Status $status -Details @{preset=$Preset;targets=$targets} -ErrorMessage $errorMessage;throw }
}
function Invoke-CtestSelection([string]$Preset,[string]$BinaryDirectory,[object[]]$Tests,[string]$ReportTag='',[string[]]$ExcludeFixtureSetup=@(),[switch]$DiagnosticOnly) {
    if (!$Tests.Count) { return }
    $names=@($Tests | ForEach-Object name)
    $regex='^(' + (($names | ForEach-Object { [regex]::Escape($_) }) -join '|') + ')$'
    $loadVar=Get-Variable -Name TestLoad -ErrorAction SilentlyContinue
    $loadLimit=if($loadVar -and $null -ne $loadVar.Value){[double]$loadVar.Value}else{0.0}
    $policyVar=Get-Variable -Name FailurePolicy -ErrorAction SilentlyContinue
    $policy=if($policyVar -and $policyVar.Value){[string]$policyVar.Value}else{'fail-fast'}
    $stopOnFailure=(!$DiagnosticOnly) -and ($policy -eq 'fail-fast')
    $stage=Start-TestStage -Name "CTest execution ($Preset)" -Category 'ctest.run'
    $status='passed';$errorMessage=$null
    $previousAttempt=$env:SHANJIE_CTEST_ATTEMPT
    $previousParent=$env:SHANJIE_TEST_PARENT_STAGE_ID
    $env:SHANJIE_TEST_PARENT_STAGE_ID=$stage.Id
    $waveReports=[Collections.Generic.List[object]]::new()
    try {
        $maxAttempts=if($RetryFailures -and !$DiagnosticOnly){2}else{1}
        $ctestExit=0
        for($attempt=1;$attempt -le $maxAttempts;$attempt++) {
            $suffix=$(if($ReportTag){"-$ReportTag"})+$(if($attempt -gt 1){'-retry'})
            $log=Join-Path $reportRoot "ctest-$Preset$suffix.log"
            $junit=Join-Path $reportRoot "junit-$Preset$suffix.xml"
            $ctestArguments=@('--test-dir',$BinaryDirectory,'-C','Release','--output-on-failure','--test-output-size-passed','0','--test-output-size-failed','0','--output-log',$log,'--output-junit',$junit,'--parallel',"$Jobs",'--no-tests=error')
            if($stopOnFailure){$ctestArguments+='--stop-on-failure'}
            if($loadLimit -gt 0){$ctestArguments+=@('--test-load',"$loadLimit")}
            if($ExcludeFixtureSetup -and $ExcludeFixtureSetup.Count){
                $fixtureRegex='^(' + (($ExcludeFixtureSetup | ForEach-Object { [regex]::Escape($_) }) -join '|') + ')$'
                $ctestArguments+=@('-FS',$fixtureRegex)
            }
            if($attempt -eq 1){$ctestArguments+=@('-R',$regex)}else{$ctestArguments+='--rerun-failed'}
            $reportEntry=[pscustomobject]@{preset=$Preset;wave=$ReportTag;attempt=$attempt;log=$log;junit=$junit;tests=$names;stop_on_failure=$stopOnFailure;diagnostic=[bool]$DiagnosticOnly;snapshot_id=$env:SHANJIE_RELEASE_SNAPSHOT_ID;snapshot_sha256=$env:SHANJIE_RELEASE_SNAPSHOT_SHA256}
            $waveReports.Add($reportEntry)
            if($DiagnosticOnly){$script:diagnosticReports += $reportEntry}else{$script:ctestReports += $reportEntry}
            $env:SHANJIE_CTEST_ATTEMPT="$attempt"
            & ctest @ctestArguments
            $ctestExit=$LASTEXITCODE
            $lastTest=Join-Path $BinaryDirectory 'Testing/Temporary/LastTest.log'
            if(Test-Path -LiteralPath $lastTest){Copy-Item -LiteralPath $lastTest -Destination (Join-Path $reportRoot "LastTest-$Preset$suffix.log") -Force}
            if(!$ctestExit){
                # If attempt 1 stopped early due to --stop-on-failure and retry
                # recovered the flaky test, execute only the unstarted rows once.
                if($attempt -gt 1 -and $stopOnFailure){
                    $executed=[Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
                    foreach($r in (Read-CtestResults @($waveReports))){[void]$executed.Add($r.name)}
                    $unstarted=@($names | Where-Object { !$executed.Contains($_) })
                    if($unstarted.Count){
                        $resumeSuffix=$(if($ReportTag){"-$ReportTag"})+'-resume'
                        $resumeLog=Join-Path $reportRoot "ctest-$Preset$resumeSuffix.log"
                        $resumeJunit=Join-Path $reportRoot "junit-$Preset$resumeSuffix.xml"
                        $resumeRegex='^(' + (($unstarted | ForEach-Object { [regex]::Escape($_) }) -join '|') + ')$'
                        $resumeArgs=@('--test-dir',$BinaryDirectory,'-C','Release','--output-on-failure','--test-output-size-passed','0','--test-output-size-failed','0','--output-log',$resumeLog,'--output-junit',$resumeJunit,'--parallel',"$Jobs",'--no-tests=error','--stop-on-failure')
                        if($loadLimit -gt 0){$resumeArgs+=@('--test-load',"$loadLimit")}
                        if($ExcludeFixtureSetup -and $ExcludeFixtureSetup.Count){
                            $fixtureRegex='^(' + (($ExcludeFixtureSetup | ForEach-Object { [regex]::Escape($_) }) -join '|') + ')$'
                            $resumeArgs+=@('-FS',$fixtureRegex)
                        }
                        $resumeArgs+=@('-R',$resumeRegex)
                        $resumeEntry=[pscustomobject]@{preset=$Preset;wave=$ReportTag;attempt=1;resumed=$true;log=$resumeLog;junit=$resumeJunit;tests=$unstarted;stop_on_failure=$true;diagnostic=$false;snapshot_id=$env:SHANJIE_RELEASE_SNAPSHOT_ID;snapshot_sha256=$env:SHANJIE_RELEASE_SNAPSHOT_SHA256}
                        $waveReports.Add($resumeEntry)
                        $script:ctestReports += $resumeEntry
                        $env:SHANJIE_CTEST_ATTEMPT='1'
                        & ctest @resumeArgs
                        $ctestExit=$LASTEXITCODE
                        if(Test-Path -LiteralPath $lastTest){Copy-Item -LiteralPath $lastTest -Destination (Join-Path $reportRoot "LastTest-$Preset$resumeSuffix.log") -Force}
                        $log=$resumeLog;$junit=$resumeJunit
                    }
                }
                break
            }
        }
        if($ctestExit){
            if(!$DiagnosticOnly){$script:stoppedAfterWave=if($ReportTag){$ReportTag}else{'single'}}
            throw "CTest failed for $Preset (exit $ctestExit); see $log and $junit."
        }
    } catch { $status='failed';$errorMessage=$_.Exception.Message;throw }
    finally {
        $env:SHANJIE_CTEST_ATTEMPT=$previousAttempt
        $env:SHANJIE_TEST_PARENT_STAGE_ID=$previousParent
        Complete-TestStage -Stage $stage -Status $status -Details @{preset=$Preset;wave=$ReportTag;selected_tests=$names.Count;stop_on_failure=$stopOnFailure;log=$log;junit=$junit} -ErrorMessage $errorMessage
    }
}

try {
    if($Level -eq 'RELEASE'){
        $sourceStart=Get-ReleaseIdentity $root
        if(!$PlanOnly -and !$sourceStart.clean){throw 'Commit the intended source first; RELEASE requires a clean commit before building its snapshot.'}
    }
    if($Level -ne $requestedLevel){$notes.Add('Unbounded x86 impact: expanded AFFECTED to dual-architecture RELEASE; packaging still requires both architectures to pass.')}
    $changedPaths=@($changeSet.Paths)
    if ($changeSet.Error) { $forceFull=$true;$notes.Add($changeSet.Error) }
    if($Level -ne 'CI'){$notes.Add("Change scope=$ChangeScope; committed range=$($changeSet.Range); staged, unstaged and untracked paths are included. Narrow scopes do not validate older branch commits.")}
    foreach($path in $changedPaths){Classify-ChangedPath $path}
    $codeChanges=@($changedPaths | Where-Object { !(Test-DocumentationOnly $_) })
    if ($Level -in @('FAST','AFFECTED') -and $changedPaths.Count -and !$codeChanges.Count) { $skipTests=$true }
    if (!$changedPaths.Count -and !$changeSet.Error -and $Level -eq 'AFFECTED') { $skipTests=$true;$notes.Add('No changed paths were found; selected no affected tests.') }
    if ($forceFull -and $Level -eq 'AFFECTED') { $notes.Add('Impact could not be bounded safely; expanded selection to FULL.') }
    if ($Level -eq 'FAST' -and !$skipTests) {
        $requestedLabels.Clear()
        [void]$requestedLabels.Add('fast')
        if(!$pathReasons.Contains('fast')){$pathReasons['fast']='FAST source coverage and Rust unit smoke checks.'}
        if(!$changedPaths.Count -and !$changeSet.Error){$notes.Add('No changed paths were found; running the FAST smoke set.')}
    }
    if ($Level -eq 'AFFECTED' -and !$skipTests) {
        if($forceFull){[void]$requestedLabels.Add('full')}
        else{[void]$requestedLabels.Add('fast')}
        if(!$pathReasons.Contains('fast')){$pathReasons['fast']='FAST source coverage and Rust unit smoke checks run before the mapped regressions.'}
    }
    if ($Level -eq 'FULL') { $requestedLabels.Clear();[void]$requestedLabels.Add('full') }
    if ($Level -eq 'CI') { $requestedLabels.Clear();[void]$requestedLabels.Add('ci');$notes.Add('Fixed CI baseline: ignores change impact and never expands to FULL or builds the product.') }
    if ($Level -eq 'RELEASE') { $requestedLabels.Clear();[void]$requestedLabels.Add('full');[void]$requestedLabels.Add('release') }
    $skipTests = $skipTests -and $Level -in @('FAST','AFFECTED') -and !$forceFull
    if($Level -ne 'CI'){
        $additionalVerification=@(Get-AdditionalVerificationPlan -Paths $changedPaths -Labels @($requestedLabels) -CiSafe:$CiSafe)
        foreach($item in $additionalVerification){
            Write-Host "[$($item.kind)] $($item.id): $($item.reason)"
            if($item.command){Write-Host "  $($item.command)"}
        }
        if($additionalVerification.Count){Write-Host 'Advisories only: planned CTest rows and extra checks are not PASS results. Credit actual matching JUnit/JSON evidence; do not run legacy wrappers again for those cases.'}
    }
    $presetLabels=[ordered]@{}
    foreach($preset in @('windows-x64')+@($(if($Level -eq 'RELEASE' -or ($Level -eq 'AFFECTED' -and $Architecture -eq 'x86')){'windows-x86'}))|Where-Object{$_}){
        $presetLabels[$preset]=@(if(!$skipTests){Get-VerificationPresetLabels $Level $Architecture $preset @($requestedLabels)})
    }
    if($PlanOnly){
        $overallStatus='planned'
        $notes.Add('Plan only: CMake inventory, builds and tests were not executed.')
        Write-Host "Requested CTest labels: $(@($requestedLabels) -join ', '); skip=$skipTests"
        Write-Host "Change scope: $ChangeScope; committed range: $($changeSet.Range); effective base: $($changeSet.EffectiveBase)"
        foreach($path in $changedPaths){Write-Host "$path : $($pathReasons[$path])"}
        return
    }
    $resourceSampler=Start-ResourceSampler
    $presets=@(if(!$skipTests){'windows-x64'})
    if($Level -eq 'RELEASE'){$presets+=@('windows-x86')}
    if(!$skipTests -and $Level -eq 'AFFECTED' -and $Architecture -eq 'x86'){$presets+=@('windows-x86')}
    if($Level -eq 'RELEASE'){
        # Build both architectures before freezing any input or linking harnesses.
        Invoke-Timed -Name 'Build both release architectures' -Category 'build.product' -Details @{architecture='all'} -Action {
            & "$PSScriptRoot/build.ps1" -Architecture all -PackageTargetsOnly
            & "$PSScriptRoot/prepare-licenses.ps1"
        }
        $snapshot=Invoke-Timed -Name 'Seal dual-architecture release snapshot' -Category 'release.snapshot' -Action {New-ReleaseSnapshot $root $runId $sourceStart}
        $env:SHANJIE_RELEASE_SNAPSHOT_ID=$snapshot.id
        $env:SHANJIE_RELEASE_SNAPSHOT_SHA256=$snapshot.sha256
        $needsProduct=$true
    }
    foreach($preset in $presets) {
        Invoke-Timed -Name "CMake configure ($preset)" -Category 'build.cmake.configure' -Details @{preset=$preset} -Action {
            $nativeRoot=if($snapshot){Join-Path $snapshot.root 'native'}else{''}
            & cmake --preset $preset "-DSHANJIE_RELEASE_NATIVE_ROOT=$nativeRoot"
            if($LASTEXITCODE){throw "CMake configure failed for $preset."}
        }
    }

    foreach($preset in $presets) {
        $binaryDirectory=Join-Path $root "build/cmake/$preset"
        $inventory=Get-CtestInventory $binaryDirectory
        $inventoryByPreset[$preset]=$inventory
        $labels=@(Get-VerificationPresetLabels $Level $Architecture $preset @($requestedLabels))
        if($skipTests){$labels=@()}
        $selection=Get-SelectedTests $inventory $labels $preset
        if($Level -eq 'CI'){Assert-FixedCiSelection $selection.Selected}
        $selectedByPreset[$preset]=$selection.Selected
        $unselectedByPreset[$preset]=$selection.Omitted
        $needsProduct=$Level -ne 'CI' -and ($needsProduct -or [bool](@($selection.Selected | Where-Object { ($_.labels | Where-Object { $_ -in @('tsf','english','submission','word','private-desktop','smoke','models','ui','mode','package') }) }).Count))
    }

    if ($Level -eq 'AFFECTED' -and !$forceFull) {
        $needsUiOutput = [bool](@($changedPaths | Where-Object { $_ -match '^(ui/|scripts/verify-ui\.ps1$|tests/candidate_host_smoke\.cpp$)' }).Count) -or
            [bool](@($selectedByPreset.Values | ForEach-Object { $_ } | Where-Object { $_.name -match '^release\.ui\.(candidate\.|verify$)' }).Count)
        $skipSettingsBuild = !$needsUiOutput
        if ($skipSettingsBuild) { $notes.Add('AFFECTED product prerequisites omit WinUI publish because no changed path or selected interactive UI case needs it; FULL and RELEASE still include WinUI publish.') }
    }

    if($Level -eq 'RELEASE'){
        $notes.Add('Both product architectures were built and sealed before CMake configuration. All release tests and packaging use that snapshot.')
    } elseif($Level -eq 'FULL' -or ($Level -eq 'AFFECTED' -and $forceFull)) {
        $buildArchitecture='x64'
        Invoke-Timed -Name "Build $buildArchitecture release product" -Category 'build.product' -Details @{architecture=$buildArchitecture;incremental='Cargo, WinUI, model and data receipts'} -Action {
            & "$PSScriptRoot/build.ps1" -Architecture $buildArchitecture
        }
        $needsProduct=$true
    } elseif($needsProduct) {
        $buildArchitecture=if($Architecture -eq 'x86' -and $Level -eq 'AFFECTED'){'all'}else{'x64'}
        Invoke-Timed -Name "Build $buildArchitecture product prerequisites" -Category 'build.product' -Details @{architecture=$buildArchitecture;incremental='Cargo, WinUI, model and data receipts'} -Action {
            if ($skipSettingsBuild) { & "$PSScriptRoot/build.ps1" -Architecture $buildArchitecture -SkipSettings }
            else { & "$PSScriptRoot/build.ps1" -Architecture $buildArchitecture }
        }
    }

    if(!$skipTests) {
        foreach($preset in $presets) {
            $selected=@($selectedByPreset[$preset]|Where-Object name -NE 'release.package')
            if(!$selected.Count){continue}
            if($snapshot){$null=Get-ReleaseSnapshot $root $snapshot.id $snapshot.sha256 -VerifyFiles}
            Invoke-CmakeBuild $preset $selected
            $binaryDirectory=Join-Path $root "build/cmake/$preset"
            $costDataPath=Get-CtestCostDataPath $binaryDirectory
            $waves=@(Get-CtestExecutionWaves $inventoryByPreset[$preset] $selected)
            $schedulingByPreset[$preset]=[ordered]@{
                cost_data_path=$costDataPath
                cost_data_existed_before=(Test-Path -LiteralPath $costDataPath)
                waves=@($waves | ForEach-Object { [ordered]@{ tag=$_.tag; count=$_.tests.Count; tests=@($_.tests | ForEach-Object name); exclude_setup_fixtures=$_.exclude_setup_fixtures; reason=$_.reason } })
            }
            $priorFailed=@(if($waves.Count -gt 1){Read-CtestPreviousFailures $costDataPath})
            for($waveIndex=0;$waveIndex -lt $waves.Count;$waveIndex++) {
                $wave=$waves[$waveIndex]
                Invoke-CtestSelection $preset $binaryDirectory $wave.tests -ReportTag $wave.tag -ExcludeFixtureSetup $wave.exclude_setup_fixtures
                if($priorFailed.Count -and $waveIndex -lt ($waves.Count - 1)){
                    $upcoming=[Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
                    for($nextIndex=$waveIndex+1;$nextIndex -lt $waves.Count;$nextIndex++){
                        foreach($upcomingRow in $waves[$nextIndex].tests){[void]$upcoming.Add($upcomingRow.name)}
                    }
                    $carryOver=@($priorFailed | Where-Object { $upcoming.Contains($_) })
                    if($carryOver.Count){Merge-CtestPreviousFailures $costDataPath $carryOver}
                }
            }
            $schedulingByPreset[$preset]['cost_data_exists_after']=(Test-Path -LiteralPath $costDataPath)
            if($snapshot){
                $null=Get-ReleaseSnapshot $root $snapshot.id $snapshot.sha256 -VerifyFiles
                Assert-ReleaseRuntimeFixture $snapshot (Join-Path $root "build/cmake/$preset/test-payload-$($snapshot.id)") $preset.Substring(8)
            }
        }
    }
    if($snapshot){
        $prepackRows=@($selectedByPreset.Values|ForEach-Object{$_}|Where-Object name -NE 'release.package')
        $prepackCases=@($prepackRows|ForEach-Object{[pscustomobject]@{name=$_.name;preset=$_.preset;selected=$true;reason=$_.reason}})
        $prepackTimings=@(Get-ChildItem -LiteralPath $eventsRoot -Filter '*.json'|ForEach-Object{Get-Content -LiteralPath $_.FullName -Raw|ConvertFrom-Json})
        $permitRecord=[ordered]@{schema=4;level='RELEASE';status='tests-passed';ci_safe=[bool]$CiSafe;ctest=$ctestReports;results=@(Read-CtestResults $ctestReports);selection=@{selected=$prepackRows};subcases=@(Read-HarnessCaseResults $prepackCases $prepackTimings);release=@{architectures=@('x64','x86');candidate_id=$runId;snapshot_sha256=$snapshot.sha256}}
        $packagingPermit=New-ReleasePackagingPermit $root (Join-Path $reportRoot 'packaging-permit.json') $permitRecord
        $env:SHANJIE_RELEASE_PACKAGING_PERMIT=$packagingPermit.path
        $packageRows=@($selectedByPreset['windows-x64']|Where-Object name -EQ 'release.package')
        if($packageRows.Count -ne 1){throw 'RELEASE inventory must contain exactly one deferred package test.'}
        Invoke-CtestSelection 'windows-x64' (Join-Path $root 'build/cmake/windows-x64') $packageRows -ReportTag 'package'
        $null=Get-ReleaseSnapshot $root $snapshot.id $snapshot.sha256 -VerifyFiles
    }
} catch {
    $overallStatus='failed'
    $failureMessage=$_.Exception.Message
    Write-Host "FAILED: $failureMessage"
    if($DiagnosticTests -and $DiagnosticTests.Count -and !$PlanOnly -and $inventoryByPreset.Count){
        Write-Host "Running explicitly requested post-failure diagnostic tests: $($DiagnosticTests -join ', ')"
        foreach($diagPreset in $inventoryByPreset.Keys){
            $inv=$inventoryByPreset[$diagPreset]
            if(!$inv -or !$inv.tests){continue}
            $matched=@($inv.tests | Where-Object { $_.name -in $DiagnosticTests } | ForEach-Object { [pscustomobject]@{name=$_.name;preset=$diagPreset} })
            if(!$matched.Count){continue}
            try {
                Invoke-CmakeBuild $diagPreset $matched
                Invoke-CtestSelection $diagPreset (Join-Path $root "build/cmake/$diagPreset") $matched -ReportTag 'diag' -DiagnosticOnly
            } catch {
                $notes.Add("Post-failure diagnostic execution on $diagPreset reported failure: $($_.Exception.Message)")
            }
        }
    }
} finally {
    $whole.Stop()
    $resourceUsage=Stop-ResourceSampler $resourceSampler
    try { $testResults=@(Read-CtestResults $ctestReports) }
    catch { $overallStatus='failed';$failureMessage=$_.Exception.Message;$notes.Add("Could not read CTest JUnit: $failureMessage") }
    if($diagnosticReports.Count){
        try { $diagnosticResults=@(Read-CtestResults $diagnosticReports) }
        catch { $notes.Add("Could not read diagnostic CTest JUnit: $($_.Exception.Message)") }
    }
    $firstFailure=$testResults | Where-Object { $_.status -ne 'passed' } | Select-Object -First 1
    $observed=[Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach($result in $testResults){[void]$observed.Add("$($result.preset)::$($result.name)")}
    $executedCount=$testResults.Count
    foreach($selected in @($selectedByPreset.Values | ForEach-Object { $_ })) {
        $key="$($selected.preset)::$($selected.name)"
        if(!$observed.Contains($key)){
            $notRunReason=if($overallStatus -eq 'failed' -and $FailurePolicy -eq 'fail-fast' -and $firstFailure){"Stopped early after failure in $($firstFailure.preset)::$($firstFailure.name)."}elseif($overallStatus -eq 'failed'){'CTest did not return a JUnit result; see the stage failure and log.'}else{'Selected in the plan but not started.'}
            $testResults+=,[pscustomobject]@{name=$selected.name;class=$null;preset=$selected.preset;status='not-run';duration_seconds=$null;failure=$null;reason=$notRunReason;log=$null}
        }
    }
    $notRunRows=@($testResults | Where-Object status -eq 'not-run')
    if($overallStatus -eq 'passed' -and @($testResults | Where-Object status -NE 'passed').Count){
        $overallStatus='failed'
        $failureMessage='Some selected tests failed or did not run; see the per-test results and CTest logs.'
    }
    $timingEvents=@(Get-ChildItem -LiteralPath $eventsRoot -Filter '*.json' -File -ErrorAction SilentlyContinue | ForEach-Object { Get-Content -LiteralPath $_.FullName -Raw | ConvertFrom-Json })
    $timings=@(Get-ExclusiveTestTimings $timingEvents | Sort-Object started_utc)
    foreach($hit in $timings|Where-Object{ $_.category -eq 'core.result-cache' -and $_.details.cache -eq 'hit' }){
        $notes.Add("Core assertions reused a verified passing result instead of executing again: $($hit.details.receipt)")
        foreach($result in $testResults|Where-Object name -eq $hit.test_name){$result|Add-Member -NotePropertyName core_result_reused -NotePropertyValue $true}
    }
    foreach($hit in $timings|Where-Object{ $_.category -eq 'core.heavy-result-cache' -and $_.details.cache -eq 'hit' }){
        $reused=@($hit.details.binaries)
        $notes.Add("Beta release reused prior passing results for $($reused -join ', '): $($hit.details.receipt). These binaries did not run again.")
        foreach($result in $testResults|Where-Object name -eq $hit.test_name){$result|Add-Member -NotePropertyName core_reused_binaries -NotePropertyValue $reused}
    }
    $caseRows=@(
        foreach($row in $selectedByPreset.Values | ForEach-Object { $_ }){[pscustomobject]@{name=$row.name;preset=$row.preset;reason=$row.reason;selected=$true}}
        foreach($row in $unselectedByPreset.Values | ForEach-Object { $_ }){[pscustomobject]@{name=$row.name;preset=$row.preset;reason=$row.reason;selected=$false}}
    )
    $subcases=@(Read-HarnessCaseResults $caseRows $timings)
    if($overallStatus -eq 'passed' -and @($subcases | Where-Object { $_.selected -and $_.status -ne 'passed' }).Count){
        $overallStatus='failed';$failureMessage='Some selected harness combinations did not pass or did not run; see subcases.'
    }
    $allSelected=@($selectedByPreset.Values | ForEach-Object { $_ })
    $allOmitted=@($unselectedByPreset.Values | ForEach-Object { $_ })
    if($skipTests){$allOmitted+=,[pscustomobject]@{name='CTest inventory';labels=@();reason='Inventory was not configured because this change needs no build or tests.';preset=$null}}
    if(!$skipTests){
        foreach($preset in $presets | Where-Object { $_ -notin $inventoryByPreset.Keys }){
            $allOmitted+=,[pscustomobject]@{name='CTest inventory';labels=@();reason='CMake configuration or CTest inventory failed before test rows could be listed.';preset=$preset}
        }
    }
    if($Level -eq 'RELEASE'){
        try {
            $sourceEnd=Get-ReleaseIdentity $root
            if($overallStatus -eq 'passed' -and (!$sourceStart.clean -or !$sourceEnd.clean -or $sourceStart.commit -cne $sourceEnd.commit -or $sourceStart.version -cne $sourceEnd.version)){throw 'Release source changed during acceptance.'}
            $releaseData=[ordered]@{source_start=$sourceStart;source_end=$sourceEnd;architectures=@('x64','x86');candidate_id=$runId;snapshot_sha256=$snapshot.sha256;packaging_permit=$packagingPermit.path;artifacts=$null}
            if($overallStatus -eq 'passed'){
                $null=Get-ReleaseSnapshot $root $snapshot.id $snapshot.sha256 -VerifyFiles
                Assert-ReleasePackageCopy $snapshot (Join-Path $root "build/release-candidates/$runId/package")
                $releaseData.artifacts=Get-ReleaseArtifacts $root $runId $ctestReports $packagingPermit.path
            }
        }catch{$overallStatus='failed';$failureMessage=$_.Exception.Message}
    }
    $report=[ordered]@{
        schema=if($Level -eq 'RELEASE'){4}else{3}
        version=$version
        commit=if($sourceEnd){$sourceEnd.commit}else{(& git -C $root rev-parse HEAD)}
        worktree_dirty=if($sourceEnd){!$sourceEnd.clean}else{[bool](@(& git -C $root status --porcelain --untracked-files=normal).Count)}
        level=$Level
        requested_level=$requestedLevel
        architecture=$Architecture
        plan_only=[bool]$PlanOnly
        status=$overallStatus
        wall_seconds=[Math]::Round($whole.Elapsed.TotalSeconds,3)
        ctest_parallel=$Jobs
        resource_usage=$resourceUsage
        base_ref=$BaseRef
        change_scope=$ChangeScope
        change_range=$changeSet.Range
        changed_paths=$changedPaths
        impact_reasons=$pathReasons
        additional_verification=$additionalVerification
        verification_summary=(Get-VerificationCompletion $Level $overallStatus $additionalVerification $skipTests)
        selection=[ordered]@{selected=$allSelected;not_selected=$allOmitted;requested_labels=@($requestedLabels);preset_labels=$presetLabels;notes=@($notes);reason=if($Level -eq 'CI'){'Fixed CI baseline; no impact expansion.'}elseif($skipTests){'Documentation-only or no changes.'}elseif($forceFull -and $Level -eq 'AFFECTED'){'Conservative FULL expansion with additional mapped regressions retained.'}else{"$Level change-impact selection."}}
        results=$testResults
        subcases=$subcases
        flaky_tests=@($testResults | Where-Object flaky | ForEach-Object name)
        release=$releaseData
        timings=$timings
        ctest=$ctestReports
        retry_failures=[bool]$RetryFailures
        ci_safe=[bool]$CiSafe
        failure_handling=[ordered]@{
            policy=$FailurePolicy
            stop_on_failure=($FailurePolicy -eq 'fail-fast')
            stopped_early=($overallStatus -eq 'failed' -and $FailurePolicy -eq 'fail-fast' -and $notRunRows.Count -gt 0)
            stopped_after_wave=$stoppedAfterWave
            executed_tests=$executedCount
            not_run_tests=$notRunRows.Count
            not_run_names=@($notRunRows | ForEach-Object name)
            first_failure=if($firstFailure){[ordered]@{name=$firstFailure.name;preset=$firstFailure.preset;status=$firstFailure.status;duration_seconds=$firstFailure.duration_seconds;failure=$firstFailure.failure;log=$firstFailure.log;junit=$firstFailure.junit}}else{$null}
            diagnostics=[ordered]@{requested=@($DiagnosticTests);reports=$diagnosticReports;results=$diagnosticResults}
        }
        scheduling=[ordered]@{
            schedule_random=$false
            test_load=$TestLoad
            core_cost_history=(Join-Path $root 'build/test/core-cost-data.json')
            presets=$schedulingByPreset
        }
        cache=[ordered]@{cmake='incremental targets keyed by source and architecture';cargo='Cargo fingerprint cache';models='content hashes and pinned input receipts';ui='source/toolchain key plus output hashes';worktree_cache='reused when valid'}
        comparison=[ordered]@{legacy_seconds=$null;full_seconds=$null;saved_seconds=$null;speedup_percent=$null;status='not-measured';benchmark_command='pwsh -File scripts/benchmark-verification.ps1'}
        junit=$ctestReports
        console_log=$consoleLog
        failure=$failureMessage
        generated_utc=[DateTimeOffset]::UtcNow.ToString('O')
    }
    $json=Join-Path $reportRoot 'report.json'
    [IO.File]::WriteAllText($json,($report|ConvertTo-Json -Depth 18),[Text.UTF8Encoding]::new($false))
    Write-Host "Test report: $json"
    Write-Host "Verification scope: $($report.verification_summary.status); follow-up required=$($report.verification_summary.follow_up_required)"
    Write-Host "Selected CTest rows: $($allSelected.Count); not selected: $($allOmitted.Count); elapsed: $($whole.Elapsed.TotalSeconds.ToString('F3'))s"
    Stop-Transcript | Out-Null
    $env:SHANJIE_TEST_RUN_ID=$previousRunId
    $env:SHANJIE_TEST_TIMINGS_DIR=$previousTimingsDirectory
    $env:SHANJIE_TEST_LEVEL=$previousLevel
    $env:SHANJIE_REUSE_HEAVY_CORE_RESULTS=$previousCoreReuse
    $env:SHANJIE_RELEASE_SNAPSHOT_ID=$previousSnapshotId
    $env:SHANJIE_RELEASE_SNAPSHOT_SHA256=$previousSnapshotHash
    $env:SHANJIE_RELEASE_PACKAGING_PERMIT=$previousPermit
    $env:SHANJIE_TEST_STOP_ON_FAILURE=$previousStopOnFailure
}
if($overallStatus -ne 'passed'){throw $failureMessage}
if($Level -eq 'RELEASE' -and $CiSafe -and $env:GITHUB_ACTIONS -eq 'true'){
    & "$PSScriptRoot/package.ps1" -ExportCiArtifact -ReportPath $json
}
