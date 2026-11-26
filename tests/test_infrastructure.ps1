#Requires -Version 7
param([string]$ResultRoot)
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
Import-Module "$root/scripts/test-support.psm1" -ErrorAction Stop
Import-Module "$root/scripts/test-plan.psm1" -ErrorAction Stop
Import-Module "$root/scripts/release-support.psm1" -ErrorAction Stop
if(!$ResultRoot){$ResultRoot=Join-Path $root ('build/test-infrastructure-'+[guid]::NewGuid().ToString('N'))}
$null=New-Item -ItemType Directory -Path $ResultRoot -Force
function Assert-Check([bool]$Condition,[string]$Name){if(!$Condition){throw "FAIL $Name"}}
& "$PSScriptRoot/verification_paths.ps1" -ResultRoot (Join-Path $ResultRoot 'verification-paths')
& "$PSScriptRoot/ctest_integration.ps1" -ResultRoot (Join-Path $ResultRoot 'ctest-integration')
& "$PSScriptRoot/verify_selection.ps1"
& "$PSScriptRoot/upstream_sources.ps1" -ResultRoot (Join-Path $ResultRoot 'upstream-sources')
# This existing Windows-only infrastructure test also exercises migration from
# the original CTest payload junction without traversing or modifying its target.
$toolSource=Join-Path $ResultRoot 'tools-source'
$toolCopy=Join-Path $ResultRoot 'tools-migration'
$null=New-Item -ItemType Directory -Path $toolSource -Force
[IO.File]::WriteAllText("$toolSource/core.exe",'private copy fixture')
$null=New-Item -ItemType Junction -Path $toolCopy -Target $toolSource
Copy-TestTools $toolSource $toolCopy
Assert-Check (!((Get-Item $toolCopy).Attributes -band [IO.FileAttributes]::ReparsePoint)) 'Former tools junction becomes a real private directory'
Assert-Check ((Get-Content "$toolSource/core.exe" -Raw) -eq 'private copy fixture') 'Tools migration leaves the junction target intact'
$unexpected=Join-Path $ResultRoot 'unexpected-tools-link'
$null=New-Item -ItemType Junction -Path $unexpected -Target $toolSource
$rejected=$false
try{Copy-TestTools $toolCopy $unexpected}catch{$rejected=$true}
Assert-Check $rejected 'An unexpected tools junction is refused'
Invoke-Timed -Name 'Release transaction self-tests (in-memory API)' -Category 'test.release-transactions' -Action {
    & python "$root/scripts/test_release.py"
    if($LASTEXITCODE){throw 'Release transaction self-tests failed.'}
}

foreach($path in 'CMakeLists.txt','scripts/test-support.psm1','shared/Cargo.toml','shared/src/protocol.rs','windows/src/broker.rs') {
    Assert-Check (Get-VerificationImpact $path).Full "$path must expand AFFECTED to FULL"
}
$impact=Get-VerificationImpact '.github/workflows/ci.yml'
Assert-Check (!$impact.Full -and $impact.Labels.Count -eq 1 -and $impact.Labels[0] -eq 'test-framework') 'CI workflow changes select only lightweight framework self-tests'
foreach($path in 'windows/src/profile.rs','shared/src/profile.rs') {
    $impact=Get-VerificationImpact $path
    foreach($label in 'windows','tsf','ui'){Assert-Check ($label -in $impact.Labels) "$path must cover $label"}
}
$impact=Get-VerificationImpact 'windows/src/bridge.rs'
Assert-Check (!$impact.Full -and @(Compare-Object @('windows','candidate-host','tsf-compat') $impact.Labels).Count -eq 0) 'Native candidate bridge preserves its bounded candidate-host and TSF compatibility coverage'
Assert-Check ('interaction' -in (Get-VerificationImpact 'shared/src/input.rs').Labels) 'Shared key policies need interaction coverage'
Assert-Check ('mode' -in (Get-VerificationImpact 'windows/src/native.rs').Labels) 'Native changes need mode coverage'
Assert-Check ('private-desktop' -in (Get-VerificationImpact 'tests/tsf_compat.cpp').Labels) 'Private-desktop harness mapping'
$ciRows=@('fast.infrastructure','fast.upstream-data','fast.prepare-core','fast.rust.shared','fast.rust.windows'|ForEach-Object{[pscustomobject]@{name=$_}})
Assert-FixedCiSelection $ciRows
foreach($badRows in @(@{rows=@($ciRows[0..3])},@{rows=@($ciRows)+@([pscustomobject]@{name='full.core-suite'})})){
    $rejected=$false
    try{Assert-FixedCiSelection $badRows.rows}catch{$rejected=$true}
    Assert-Check $rejected 'CI contract rejects missing baseline tests and expanded full suites'
}

$english=Get-EnglishKeyCases
$officialWritingCases=@($english | Where-Object {$_.Count -gt 4 -and $_[4] -eq 1})
Assert-Check ($english.Count -eq 37 -and @($english | Where-Object {$_[1] -eq 'shift' -and $_.Count -eq 4}).Count -eq 2) 'Preserve the existing English combinations'
Assert-Check ($officialWritingCases.Count -eq 2 -and @($officialWritingCases | Where-Object {$_[0] -eq 1 -and $_[2] -eq 1}).Count -eq 2 -and @(Compare-Object @('shift','shift-caps-zh') @($officialWritingCases | ForEach-Object {$_[1]})).Count -eq 0) 'Exercise official writing in both raw-key Shift regressions'
foreach($keyboard in 1,2) {
    $submissionCases=Get-SubmissionCases $keyboard
    Assert-Check ($submissionCases.Count -eq $(if($keyboard -eq 1){66}else{30})) 'Preserve existing submission combinations and cover both interactions in the async lock regression'
    $asyncCases=@($submissionCases | Where-Object {$_[0] -eq 'async-keys'})
    Assert-Check ($asyncCases.Count -eq 2 -and @($asyncCases | Where-Object {$_[4] -eq 'shanjie'}).Count -eq 1) "Keep both async lock interaction profiles with keyboard $keyboard"
    foreach($mode in 'async-keys','rebuild-local','rebuild-pipe','segment') {
        Assert-Check ($mode -in ($submissionCases | ForEach-Object {$_[0]})) "Keep $mode covered with keyboard $keyboard"
    }
}
$rows=@(
    [pscustomobject]@{name='full.english.caps.x64';preset='windows-x64';selected=$true;reason='selected'},
    [pscustomobject]@{name='full.english.numpad.x64';preset='windows-x64';selected=$false;reason='unrelated'},
    [pscustomobject]@{name='full.submission.input-digits.x64';preset='windows-x64';selected=$true;reason='selected'}
)
$plan=@(Get-HarnessCasePlan $rows)
Assert-Check ($plan.Count -eq 42) 'Plan expands parent groups into every input combination'
Assert-Check (@($plan | Where-Object { !$_.selected -and $_.reason -eq 'unrelated' }).Count -eq 4) 'Unselected combinations retain their reason'
Assert-Check (@($plan | Where-Object status -ne 'not-run').Count -eq 0) 'Planning never marks combinations passed'

# Exercise the real selection and retry functions without invoking compilers or CTest.
$tokens=$null;$errors=$null
$ast=[System.Management.Automation.Language.Parser]::ParseFile("$root/scripts/test.ps1",[ref]$tokens,[ref]$errors)
$Level='RELEASE';$skipTests=$false
# Evaluate the driver's real preset assignments on the RELEASE path. A scalar
# assignment followed by += silently concatenated the two architecture names.
$Architecture='x64'
$assignment=$ast.Find({param($node)$node -is [Management.Automation.Language.AssignmentStatementAst] -and $node.Left.Extent.Text -eq '$presets' -and $node.Right.Extent.Text -like '*windows-x64*'},$false)
. ([scriptblock]::Create($assignment.Extent.Text))
foreach($branch in $ast.FindAll({param($node)$node -is [Management.Automation.Language.IfStatementAst] -and $node.Extent.Text -match '^if\([^\n]+\)\{\$presets\+='},$false)){
    . ([scriptblock]::Create($branch.Extent.Text))
}
Assert-Check ($presets.Count -eq 2 -and $presets[0] -ceq 'windows-x64' -and $presets[1] -ceq 'windows-x86') 'RELEASE retains two separate architecture presets'
foreach($name in 'Get-TestLabels','Get-TestFixtureProperty','Get-TestNumericProperty','Read-CtestPreviousFailures','Merge-CtestPreviousFailures','Get-CtestExecutionWaves','Get-SelectedTests','Invoke-CtestSelection') {
    $definition=$ast.Find({param($node) $node -is [System.Management.Automation.Language.FunctionDefinitionAst] -and $node.Name -eq $name},$false)
    . ([scriptblock]::Create($definition.Extent.Text))
}
$docFunction=$ast.Find({param($node) $node -is [System.Management.Automation.Language.FunctionDefinitionAst] -and $node.Name -eq 'Test-DocumentationOnly'},$false)
. ([scriptblock]::Create($docFunction.Extent.Text))
foreach($path in 'windows/src/profile.rs','tests/submission.cpp','scripts/build.ps1'){
    Assert-Check (!(Test-DocumentationOnly $path)) "$path cannot skip tests based on comment-like code"
}
Assert-Check ($null -eq $ast.Find({param($node) $node -is [System.Management.Automation.Language.FunctionDefinitionAst] -and $node.Name -eq 'Test-CommentOnlyChange'},$false)) 'No regex shortcut can misclassify dereferences or inline block comments'
$Level='AFFECTED';$CiSafe=$false;$skipTests=$false;$pathReasons=[ordered]@{}
$inventory=[pscustomobject]@{tests=@(
    [pscustomobject]@{name='full.payload';properties=@([pscustomobject]@{name='LABELS';value=@('fixture')},[pscustomobject]@{name='FIXTURES_SETUP';value=@('RuntimePayload')})},
    [pscustomobject]@{name='full.english.caps.x64';properties=@([pscustomobject]@{name='LABELS';value=@('english')},[pscustomobject]@{name='FIXTURES_REQUIRED';value=@('RuntimePayload')})}
)}
$selection=Get-SelectedTests $inventory @('english') 'windows-x64'
Assert-Check ($selection.Selected.Count -eq 2) 'Report explicitly includes required fixture setup'
Assert-Check ($selection.Omitted.Count -eq 0) 'A required fixture is not simultaneously reported omitted'
$waveInventory=[pscustomobject]@{tests=@(
    [pscustomobject]@{name='fast.prepare-core';properties=@([pscustomobject]@{name='FIXTURES_SETUP';value=@('CoreOverlay')},[pscustomobject]@{name='PROCESSORS';value=1})},
    [pscustomobject]@{name='full.core-suite';properties=@([pscustomobject]@{name='FIXTURES_REQUIRED';value=@('CoreOverlay')},[pscustomobject]@{name='PROCESSORS';value=4})},
    [pscustomobject]@{name='fast.rust.windows';properties=@([pscustomobject]@{name='FIXTURES_REQUIRED';value=@('CoreOverlay')},[pscustomobject]@{name='PROCESSORS';value=1})},
    [pscustomobject]@{name='full.english.caps.x64';properties=@([pscustomobject]@{name='PROCESSORS';value=1})}
)}
$waves=@(Get-CtestExecutionWaves $waveInventory @($waveInventory.tests | ForEach-Object {[pscustomobject]@{name=$_.name}}))
Assert-Check ($waves.Count -eq 2 -and $waves[0].tag -eq 'heavy' -and ($waves[0].tests.name -join ',') -eq 'fast.prepare-core,full.core-suite') 'Multi-processor heavy suite forms an upfront priority wave with its required fixture setup'
Assert-Check ($waves[1].tag -eq 'remaining' -and ($waves[1].tests.name -join ',') -eq 'fast.rust.windows,full.english.caps.x64' -and ($waves[1].exclude_setup_fixtures -join ',') -eq 'CoreOverlay') 'Remaining wave excludes already-completed setup fixtures instead of rerunning them'
$singleWave=@(Get-CtestExecutionWaves $waveInventory @([pscustomobject]@{name='fast.prepare-core'},[pscustomobject]@{name='fast.rust.windows'}))
Assert-Check ($singleWave.Count -eq 1 -and $singleWave[0].tag -eq '') 'Single-processor selections stay in one CTest wave'
$gatedInventory=[pscustomobject]@{tests=@(
    [pscustomobject]@{name='fast.upstream-data';properties=@([pscustomobject]@{name='LABELS';value=@('ci;fast;full')},[pscustomobject]@{name='PROCESSORS';value=1})},
    [pscustomobject]@{name='fast.prepare-core';properties=@([pscustomobject]@{name='LABELS';value=@('ci;fast;full')},[pscustomobject]@{name='FIXTURES_SETUP';value=@('CoreOverlay')},[pscustomobject]@{name='PROCESSORS';value=1})},
    [pscustomobject]@{name='fast.rust.windows';properties=@([pscustomobject]@{name='LABELS';value=@('ci;fast;full')},[pscustomobject]@{name='FIXTURES_REQUIRED';value=@('CoreOverlay')},[pscustomobject]@{name='PROCESSORS';value=1})},
    [pscustomobject]@{name='full.core-suite';properties=@([pscustomobject]@{name='LABELS';value=@('full;core')},[pscustomobject]@{name='FIXTURES_REQUIRED';value=@('CoreOverlay')},[pscustomobject]@{name='PROCESSORS';value=4})},
    [pscustomobject]@{name='full.english.caps.x64';properties=@([pscustomobject]@{name='LABELS';value=@('full;english')},[pscustomobject]@{name='PROCESSORS';value=1})}
)}
$gatedWaves=@(Get-CtestExecutionWaves $gatedInventory @($gatedInventory.tests | ForEach-Object {[pscustomobject]@{name=$_.name}}))
Assert-Check ($gatedWaves.Count -eq 3 -and $gatedWaves[0].tag -eq 'fast' -and ($gatedWaves[0].tests.name -join ',') -eq 'fast.prepare-core,fast.upstream-data,fast.rust.windows') 'FAST baseline forms an upfront gate before heavy and remaining waves'
Assert-Check ($gatedWaves[1].tag -eq 'heavy' -and ($gatedWaves[1].tests.name -join ',') -eq 'full.core-suite' -and ($gatedWaves[1].exclude_setup_fixtures -join ',') -eq 'CoreOverlay') 'Heavy wave reuses completed FAST setup fixtures via -FS'
Assert-Check ($gatedWaves[2].tag -eq 'remaining' -and ($gatedWaves[2].tests.name -join ',') -eq 'full.english.caps.x64' -and ($gatedWaves[2].exclude_setup_fixtures -join ',') -eq 'CoreOverlay') 'Remaining wave also excludes fixtures completed in earlier waves'
$mockCostFile=Join-Path $ResultRoot 'CTestCostData.txt'
[IO.File]::WriteAllText($mockCostFile,"heavy 1 10`nshort_12 1 1`n---`nshort_12`n",[Text.UTF8Encoding]::new($false))
Assert-Check (((Read-CtestPreviousFailures $mockCostFile) -join ',') -eq 'short_12') 'CTestCostData previous-failure section is parsed before multi-wave execution'
[IO.File]::WriteAllText($mockCostFile,"heavy 2 10`nshort_12 1 1`n---`n",[Text.UTF8Encoding]::new($false))
Merge-CtestPreviousFailures $mockCostFile @('short_12')
Assert-Check (((Read-CtestPreviousFailures $mockCostFile) -join ',') -eq 'short_12') 'Passing wave 1 preserves previous-failure priority for upcoming waves'

$reportRoot=$ResultRoot;$Jobs=1;$RetryFailures=$true;$FailurePolicy='fail-fast';$script:ctestReports=@();$script:diagnosticReports=@();$script:mockCalls=@();$script:stoppedAfterWave=$null
function ctest {
    $script:mockCalls+=,@($args)
    $attempt=$script:mockCalls.Count
    $junit=$args[[Array]::IndexOf($args,'--output-junit')+1]
    $log=$args[[Array]::IndexOf($args,'--output-log')+1]
    if($attempt -eq 1){
        $xml='<testsuite><testcase name="stable" classname="stable" time="0.1"/><testcase name="retry" classname="retry" time="0.2"><failure message="Failed"/><system-out>first failure</system-out></testcase></testsuite>'
        $global:LASTEXITCODE=8
    }else{
        $xml='<testsuite><testcase name="retry" classname="retry" time="0.3"/></testsuite>'
        $global:LASTEXITCODE=0
    }
    [IO.File]::WriteAllText($junit,$xml)
    [IO.File]::WriteAllText($log,"mock attempt $attempt")
}
Invoke-CtestSelection 'windows-x64' $ResultRoot @([pscustomobject]@{name='stable'},[pscustomobject]@{name='retry'})
Assert-Check ($script:mockCalls.Count -eq 2) 'Retry at most once'
Assert-Check ('--stop-on-failure' -in $script:mockCalls[0]) 'Fail-fast policy passes --stop-on-failure to CTest by default'
Assert-Check ('--rerun-failed' -in $script:mockCalls[1]) 'Retry asks CTest for failed rows only'
foreach($option in '--test-output-size-passed','--test-output-size-failed'){
    Assert-Check ($script:mockCalls[0][[Array]::IndexOf($script:mockCalls[0],$option)+1] -eq '0') 'JUnit keeps complete output for coverage comparison'
}
$results=@(Read-CtestResults $script:ctestReports)
$retried=$results | Where-Object name -eq 'retry'
$stable=$results | Where-Object name -eq 'stable'
Assert-Check ($stable.attempts.Count -eq 1) 'Passed rows are not retried'
Assert-Check ($retried.status -eq 'passed' -and $retried.flaky -and $retried.attempts.Count -eq 2) 'A retry pass retains the first failure'
Assert-Check ($retried.attempts[0].failure -eq 'first failure') 'First failure text remains available'
$firstJUnit=$script:ctestReports[0].junit
$firstBytes=[IO.File]::ReadAllText($firstJUnit)
$RetryFailures=$false
$TestLoad=6
Invoke-CtestSelection 'windows-x64' $ResultRoot @([pscustomobject]@{name='release.package'}) -ReportTag 'package' -ExcludeFixtureSetup @('CoreOverlay')
$TestLoad=0
Assert-Check ($script:ctestReports[-1].junit -ne $firstJUnit -and [IO.File]::ReadAllText($firstJUnit) -ceq $firstBytes) 'Deferred package has its own JUnit and retains architecture evidence'
Assert-Check ('--test-load' -in $script:mockCalls[-1] -and $script:mockCalls[-1][[Array]::IndexOf($script:mockCalls[-1],'--test-load')+1] -eq '6') 'Optional TestLoad forwards --test-load to CTest'
Assert-Check ('-FS' -in $script:mockCalls[-1] -and $script:mockCalls[-1][[Array]::IndexOf($script:mockCalls[-1],'-FS')+1] -eq '^(CoreOverlay)$') 'Subsequent waves pass -FS to avoid rerunning completed fixture setup'
Assert-Check ('--schedule-random' -notin $script:mockCalls[0] -and '--schedule-random' -notin $script:mockCalls[-1]) 'Scheduling never enables --schedule-random'
$FailurePolicy='continue'
Invoke-CtestSelection 'windows-x64' $ResultRoot @([pscustomobject]@{name='retry'}) -ReportTag 'continue'
Assert-Check ('--stop-on-failure' -notin $script:mockCalls[-1]) 'Continue policy omits --stop-on-failure'
$FailurePolicy='fail-fast'
Invoke-CtestSelection 'windows-x64' $ResultRoot @([pscustomobject]@{name='retry'}) -ReportTag 'diag' -DiagnosticOnly
Assert-Check ($script:diagnosticReports.Count -eq 1 -and '--stop-on-failure' -notin $script:mockCalls[-1]) 'Explicit diagnostic runs are tracked separately without mutating main CTest reports'
$script:ctestReports=@();$script:mockCalls=@();$RetryFailures=$true
function ctest {
    $script:mockCalls+=,@($args)
    $call=$script:mockCalls.Count
    $junit=$args[[Array]::IndexOf($args,'--output-junit')+1]
    $log=$args[[Array]::IndexOf($args,'--output-log')+1]
    if($call -eq 1){
        $xml='<testsuite><testcase name="passed_first" classname="passed_first" time="0.1"/><testcase name="flaky_mid" classname="flaky_mid" time="0.2"><failure message="Failed"/><system-out>flaky initial failure</system-out></testcase></testsuite>'
        $global:LASTEXITCODE=8
    }elseif($call -eq 2){
        $xml='<testsuite><testcase name="flaky_mid" classname="flaky_mid" time="0.2"/></testsuite>'
        $global:LASTEXITCODE=0
    }else{
        $xml='<testsuite><testcase name="unstarted_tail" classname="unstarted_tail" time="0.1"/></testsuite>'
        $global:LASTEXITCODE=0
    }
    [IO.File]::WriteAllText($junit,$xml)
    [IO.File]::WriteAllText($log,"mock resume call $call")
}
Invoke-CtestSelection 'windows-x64' $ResultRoot @([pscustomobject]@{name='passed_first'},[pscustomobject]@{name='flaky_mid'},[pscustomobject]@{name='unstarted_tail'}) -ReportTag 'wave'
$resumeResults=@(Read-CtestResults $script:ctestReports)
Assert-Check ($script:mockCalls.Count -eq 3 -and ('--rerun-failed' -in $script:mockCalls[1]) -and ('^(unstarted_tail)$' -in $script:mockCalls[2])) 'Recovering a flaky failure resumes only unstarted rows without rerunning passed tests'
Assert-Check (($resumeResults | Where-Object name -eq 'flaky_mid').flaky -and ($resumeResults | Where-Object name -eq 'flaky_mid').attempts[0].failure -eq 'flaky initial failure' -and ($resumeResults | Where-Object name -eq 'unstarted_tail').status -eq 'passed') 'Flaky retry plus tail resume preserves first failure evidence and completes the wave'
Remove-Item Function:ctest
& "$root/tests/release_guard.ps1" -ResultRoot (Join-Path $ResultRoot 'release-guard')
# Windows fixture plumbing: the real TSF payload must use sealed bytes and data,
# while its private executable path continues to isolate the core IPC identity.
$guardSource=Join-Path $ResultRoot 'release-guard/source'
$sealed=Get-ChildItem "$guardSource/build/release-snapshots" -Directory|Select-Object -First 1
$snapshot=Get-ReleaseSnapshot $guardSource $sealed.Name (Get-FileHash "$($sealed.FullName)/snapshot.json").Hash.ToLowerInvariant()
$privatePayload=Join-Path $root ('build/cmake/release-guard-payload-'+[guid]::NewGuid().ToString('N'))
& "$root/scripts/prepare-test-payload.ps1" -Architecture x86 -RuntimeRoot $snapshot.runtime_root -PayloadRoot $privatePayload
Assert-ReleaseRuntimeFixture $snapshot $privatePayload x86
$privateDll=Join-Path $privatePayload 'x86/Shanjie.dll'
(Get-Item -LiteralPath $privateDll).IsReadOnly=$false
[IO.File]::AppendAllText($privateDll,'different tested DLL')
$rejected=$false
try{Assert-ReleaseRuntimeFixture $snapshot $privatePayload x86}catch{$rejected=$true}
Assert-Check $rejected 'A private runtime copy differing from the sealed package DLL is rejected'
Copy-ReleaseFiles "$($snapshot.runtime_root)/x86/Shanjie.dll" $privateDll
Move-Item -LiteralPath "$privatePayload/data" -Destination "$privatePayload/sealed-data-link"
$null=New-Item -ItemType Junction -Path "$privatePayload/data" -Target "$guardSource/dist/data"
$rejected=$false
try{Assert-ReleaseRuntimeFixture $snapshot $privatePayload x86}catch{$rejected=$true}
Assert-Check $rejected 'Live dist data cannot replace the frozen runtime data junction'
$cache=Join-Path $ResultRoot 'empty-cache.json'
[IO.File]::WriteAllText($cache,'{"key":"test","files":{}}')
Assert-Check (!(Test-ContentCache -Root $ResultRoot -ManifestPath $cache -Key 'test' -Paths @())) 'Empty UI output is a cache miss even with a receipt'
# Evaluate the build driver's actual assignment: an empty array returned through
# an if pipeline becomes null unless the assignment itself collects the output.
$buildAst=[System.Management.Automation.Language.Parser]::ParseFile("$root/scripts/build.ps1",[ref]$tokens,[ref]$errors)
$uiAssignment=$buildAst.Find({param($node) $node -is [System.Management.Automation.Language.AssignmentStatementAst] -and $node.Left.Extent.Text -eq '$existingUiPaths'},$true)
$uiOutput=Join-Path $ResultRoot 'missing-ui-output'
. ([scriptblock]::Create($uiAssignment.Extent.Text))
Assert-Check ($null -ne $existingUiPaths -and $existingUiPaths.Count -eq 0) 'A clean checkout produces an empty UI file array, not null'
Assert-Check (!(Test-ContentCache -Root $ResultRoot -ManifestPath $cache -Key 'test' -Paths $existingUiPaths)) 'A clean checkout reaches the UI publish cache miss'
$null=New-Item -ItemType Directory -Path $uiOutput -Force
. ([scriptblock]::Create($uiAssignment.Extent.Text))
Assert-Check ($null -ne $existingUiPaths -and $existingUiPaths.Count -eq 0) 'An empty UI output directory also preserves the array'
$missingInputFailed=$false
try{$null=Get-ContentFingerprint -Root $ResultRoot -Paths @('missing-input')}catch{$missingInputFailed=$true}
Assert-Check $missingInputFailed 'Missing required fingerprint input is an error'
$probeOutput=Join-Path $ResultRoot 'probe'
$null=New-Item -ItemType Directory -Path $probeOutput -Force
$previousTiming=$env:SHANJIE_TEST_TIMINGS_DIR
$env:SHANJIE_TEST_TIMINGS_DIR=Join-Path $probeOutput 'events'
$deadline=[Diagnostics.Stopwatch]::StartNew();$timedOut=$false
try {
    try { Wait-TestCoreReady -CorePath (Get-Command pwsh).Source -OutputDirectory $probeOutput -TimeoutMilliseconds 500 -ProbeArguments @('-NoLogo','-NoProfile','-Command','"Start-Sleep -Seconds 30"') | Out-Null }
    catch { $timedOut=$_.Exception.Message -like '*exceeded*' }
} finally { $env:SHANJIE_TEST_TIMINGS_DIR=$previousTiming }
Assert-Check ($timedOut -and $deadline.ElapsedMilliseconds -lt 2500) 'A hung status probe respects its deadline and is retired'
$sample=@(
    [pscustomobject]@{id='parent';parent_id=$null;started_utc='2026-11-09T00:00:00Z';duration_ms=100},
    [pscustomobject]@{id='a';parent_id='parent';started_utc='2026-11-09T00:00:00.010Z';duration_ms=60},
    [pscustomobject]@{id='b';parent_id='parent';started_utc='2026-11-09T00:00:00.040Z';duration_ms=50}
)
$exclusive=@(Get-ExclusiveTestTimings $sample)
Assert-Check (($exclusive | Where-Object id -eq 'parent').exclusive_duration_ms -eq 20) 'Exclusive timing subtracts the union of overlapping child stages'

# Runner environment diagnostics: schema, unavailable fallback, secret redaction and post-run aggregation.
$diagDir=Join-Path $ResultRoot 'runner-diagnostics'
$diagCacheFixture=Join-Path $diagDir 'cache-fixture'
$null=New-Item -ItemType Directory -Path $diagCacheFixture -Force
[IO.File]::WriteAllBytes((Join-Path $diagCacheFixture 'sample.bin'), [byte[]]::new(262144))
$diagJsonPath=Join-Path $diagDir 'runner-diagnostics.json'
$diagReportPath=Join-Path $diagDir 'mock-report.json'
[IO.File]::WriteAllText($diagReportPath, (@{
    level='CI'; status='passed'; wall_seconds=12.345; ctest_parallel=4
    selection=@{ selected=@(@{name='fast.upstream-data'}, @{name='fast.rust.windows'}) }
    resource_usage=@{ cpu_avg_percent=32.5; cpu_peak_percent=78.4; ram_peak_used_mb=8192.5; ram_peak_percent=50.0; sample_count=12 }
    timings=@(
        @{ category='build.cmake.configure'; name='CMake configure (windows-x64)'; status='passed'; duration_ms=190; exclusive_duration_ms=190 },
        @{ category='ctest.run'; name='CTest execution (windows-x64)'; status='passed'; duration_ms=12100; exclusive_duration_ms=12100 }
    )
    results=@(
        @{ name='fast.upstream-data'; preset='windows-x64'; status='passed'; duration_seconds=0.6 },
        @{ name='fast.rust.windows'; preset='windows-x64'; status='passed'; duration_seconds=9.5 }
    )
} | ConvertTo-Json -Depth 8), [Text.UTF8Encoding]::new($false))
$diagCacheSpec=(@{
    cargo_fixture=@{ hit='true'; paths=@($diagCacheFixture) }
    miss_cache=@{ hit='false'; paths=@((Join-Path $diagDir 'does-not-exist')) }
    missing_cache=@{ hit=''; paths=@((Join-Path $diagDir 'does-not-exist')) }
} | ConvertTo-Json -Depth 4 -Compress)
$prevGhToken=$env:GITHUB_TOKEN; $prevSecret=$env:SHANJIE_TEST_FAKE_SECRET
$env:GITHUB_TOKEN='ghp_FakeSecretToken12345678901234567890'
$env:SHANJIE_TEST_FAKE_SECRET='github_pat_DoNotLeak9876543210'
try {
    & "$root/scripts/runner-diagnostics.ps1" -Phase pre -OutputPath $diagJsonPath -RunnerLabel 'windows-2025-vs2026' -TestLevel CI -Jobs 4 -CacheSpecJson $diagCacheSpec -Quiet
    $preRaw=[IO.File]::ReadAllText($diagJsonPath)
    Assert-Check ($preRaw -notlike '*FakeSecretToken*' -and $preRaw -notlike '*DoNotLeak*') 'Runner diagnostics never emit environment secret values'
    $preObj=ConvertFrom-Json -InputObject $preRaw
    Assert-Check ($preObj.schema -eq 1 -and $preObj.phase -eq 'pre' -and $preObj.environment.runner.image_label -eq 'windows-2025-vs2026' -and $preObj.environment.cpu.logical_processors -gt 0 -and $preObj.environment.memory.total_physical_mb -gt 0 -and $preObj.environment.toolchain.powershell -and $preObj.cache.cargo_fixture.hit -eq $true -and $preObj.cache.cargo_fixture.restored_size_mb -gt 0 -and $preObj.cache.miss_cache.hit -eq $false -and $preObj.cache.missing_cache.hit -eq 'unavailable') 'Pre-phase runner diagnostics capture hardware, toolchain, cache and unavailable markers'
    & "$root/scripts/runner-diagnostics.ps1" -Phase post -OutputPath $diagJsonPath -RunnerLabel 'windows-2025-vs2026' -TestLevel CI -Jobs 4 -TestReportPath $diagReportPath -Quiet
    $postObj=ConvertFrom-Json -InputObject ([IO.File]::ReadAllText($diagJsonPath))
    Assert-Check ($postObj.phase -eq 'post' -and $postObj.environment.git_and_test.selected_test_count -eq 2 -and $postObj.runtime_performance.cpu_peak_percent -eq 78.4 -and $postObj.runtime_performance.ram_peak_used_mb -eq 8192.5 -and $postObj.runtime_performance.stage_timings.Count -eq 2 -and $postObj.runtime_performance.test_durations[0].name -eq 'fast.rust.windows' -and $postObj.attribution_notes.Count -ge 2) 'Post-phase runner diagnostics merge runtime metrics, stage timings, test durations and attribution notes'
    & "$root/scripts/runner-diagnostics.ps1" -Phase post -OutputPath $diagJsonPath -RunnerLabel 'windows-2025-vs2026' -TestLevel CI -Jobs 4 -Quiet
} finally {
    $env:GITHUB_TOKEN=$prevGhToken; $env:SHANJIE_TEST_FAKE_SECRET=$prevSecret
}
$diagAst=[System.Management.Automation.Language.Parser]::ParseFile("$root/scripts/runner-diagnostics.ps1",[ref]$tokens,[ref]$errors)
$protectFn=$diagAst.Find({param($n) $n -is [Management.Automation.Language.FunctionDefinitionAst] -and $n.Name -eq 'Protect-DiagnosticText'},$false)
$diskFn=$diagAst.Find({param($n) $n -is [Management.Automation.Language.FunctionDefinitionAst] -and $n.Name -eq 'Get-DiskInfo'},$false)
. ([scriptblock]::Create($protectFn.Extent.Text))
. ([scriptblock]::Create($diskFn.Extent.Text))
$redacted=Protect-DiagnosticText 'https://alice:topsecret@github.com/org/repo.git ghp_1234567890abcdefghij Bearer SecretToken12345 api_key=mysecret99'
Assert-Check ($redacted -notlike '*topsecret*' -and $redacted -notlike '*1234567890abcdefghij*' -and $redacted -notlike '*SecretToken12345*' -and $redacted -notlike '*mysecret99*') 'Diagnostic sanitizer redacts credentials, tokens and bearer headers'
$missingDisk=Get-DiskInfo 'Z:'
Assert-Check ($missingDisk.total_gb -eq 'unavailable' -and $missingDisk.media_type -eq 'unavailable') 'Unavailable disk queries report explicit unavailable values without guessing'
$ciYaml=[IO.File]::ReadAllText("$root/.github/workflows/ci.yml")
Assert-Check ($ciYaml -match 'runner-diagnostics\.ps1 -Phase pre' -and $ciYaml -match 'runner-diagnostics\.ps1 -Phase post' -and $ciYaml -match 'scripts/test\.ps1 -Level CI -CiSafe') 'CI workflow records runner diagnostics while keeping the fixed lightweight CI command'
foreach($fnName in 'Start-ResourceSampler','Stop-ResourceSampler') {
    $fnDef=$ast.Find({param($n) $n -is [Management.Automation.Language.FunctionDefinitionAst] -and $n.Name -eq $fnName},$false)
    . ([scriptblock]::Create($fnDef.Extent.Text))
}
$sampler=Start-ResourceSampler -IntervalMs 100
try {
    & pwsh -NoProfile -Command 'Start-Sleep -Milliseconds 450'
} finally {
    $sampled=Stop-ResourceSampler $sampler
}
Assert-Check ($sampled.sample_count -ge 2 -and $sampled.ram_peak_used_mb -gt 0) "Background resource sampler records samples during blocking child processes (got $($sampled.sample_count))"
'PASS impact, harness plans, fixtures, retry history, empty caches, required inputs, and probe deadline'
