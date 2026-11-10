#Requires -Version 7
param([string]$ResultRoot)
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
Import-Module "$root/scripts/test-support.psm1" -ErrorAction Stop
Import-Module "$root/scripts/test-plan.psm1" -ErrorAction Stop
if(!$ResultRoot){$ResultRoot=Join-Path $root ('build/test-infrastructure-'+[guid]::NewGuid().ToString('N'))}
$null=New-Item -ItemType Directory -Path $ResultRoot -Force
function Assert-Check([bool]$Condition,[string]$Name){if(!$Condition){throw "FAIL $Name"}}
& "$PSScriptRoot/verification_paths.ps1" -ResultRoot (Join-Path $ResultRoot 'verification-paths')
& "$PSScriptRoot/ctest_integration.ps1" -ResultRoot (Join-Path $ResultRoot 'ctest-integration')
& "$PSScriptRoot/verify_selection.ps1"
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

foreach($path in 'CMakeLists.txt','.github/workflows/ci.yml','scripts/test-support.psm1','shared/Cargo.toml','shared/src/protocol.rs','windows/src/broker.rs') {
    Assert-Check (Get-VerificationImpact $path).Full "$path must expand AFFECTED to FULL"
}
foreach($path in 'windows/src/profile.rs','shared/src/profile.rs','windows/src/bridge.rs') {
    $impact=Get-VerificationImpact $path
    foreach($label in 'windows','tsf','ui'){Assert-Check ($label -in $impact.Labels) "$path must cover $label"}
}
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
Assert-Check ($english.Count -eq 32) 'Preserve all 32 English mode/layout combinations, including digit navigation'
Assert-Check ((Get-SubmissionCases 1).Count -eq 52) 'Preserve all 52 Standard submission combinations'
Assert-Check ((Get-SubmissionCases 2).Count -eq 25) 'Preserve all 25 Eten submission combinations'
$rows=@(
    [pscustomobject]@{name='full.english.caps.x64';preset='windows-x64';selected=$true;reason='selected'},
    [pscustomobject]@{name='full.english.numpad.x64';preset='windows-x64';selected=$false;reason='unrelated'},
    [pscustomobject]@{name='full.submission.input-digits.x64';preset='windows-x64';selected=$true;reason='selected'}
)
$plan=@(Get-HarnessCasePlan $rows)
Assert-Check ($plan.Count -eq 33) 'Plan expands parent groups into every input combination'
Assert-Check (@($plan | Where-Object { !$_.selected -and $_.reason -eq 'unrelated' }).Count -eq 4) 'Unselected combinations retain their reason'
Assert-Check (@($plan | Where-Object status -ne 'not-run').Count -eq 0) 'Planning never marks combinations passed'

# Exercise the real selection and retry functions without invoking compilers or CTest.
$tokens=$null;$errors=$null
$ast=[System.Management.Automation.Language.Parser]::ParseFile("$root/scripts/test.ps1",[ref]$tokens,[ref]$errors)
$Level='RELEASE';$skipTests=$false
# Evaluate the driver's real preset assignments on the RELEASE path. A scalar
# assignment followed by += silently concatenated the two architecture names.
foreach($assignment in $ast.FindAll({param($node) $node -is [System.Management.Automation.Language.AssignmentStatementAst] -and $node.Left.Extent.Text -eq '$presets'},$false)){
    . ([scriptblock]::Create($assignment.Extent.Text))
}
Assert-Check ($presets.Count -eq 2 -and $presets[0] -ceq 'windows-x64' -and $presets[1] -ceq 'windows-x86') 'RELEASE retains two separate architecture presets'
foreach($name in 'Get-TestLabels','Get-TestFixtureProperty','Get-SelectedTests','Invoke-CtestSelection') {
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

$reportRoot=$ResultRoot;$Jobs=1;$RetryFailures=$true;$script:ctestReports=@();$script:mockCalls=@()
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
Remove-Item Function:ctest
& "$root/tests/release_guard.ps1" -ResultRoot (Join-Path $ResultRoot 'release-guard')
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
'PASS impact, harness plans, fixtures, retry history, empty caches, required inputs, and probe deadline'
