#Requires -Version 7
# Self-check Windows verification plumbing with file fixtures; no product processes or SDK tools.
param([string]$ResultRoot)
$ErrorActionPreference='Stop'
$project=Split-Path $PSScriptRoot -Parent
Import-Module "$project/scripts/test-plan.psm1" -ErrorAction Stop
Import-Module "$project/scripts/test-support.psm1" -ErrorAction Stop
. "$project/scripts/preserve-output.ps1"
if(!$ResultRoot){$ResultRoot=Join-Path $project ('build/ctest-integration-check-'+[guid]::NewGuid().ToString('N'))}
if(Test-Path -LiteralPath $ResultRoot){throw 'Use a fresh CTest integration fixture directory.'}
$null=New-Item -ItemType Directory -Path $ResultRoot -Force
$checks=0
function Check([bool]$Condition,[string]$Name){$script:checks++;if(!$Condition){throw "FAIL $Name"}}

$cmake=Get-Content "$project/CMakeLists.txt" -Raw
$english=Get-EnglishKeyCases
$scenarios=[regex]::Match($cmake,'(?m)^set\(english_scenarios ([^)]+)\)').Groups[1].Value -split '\s+'
$caseScenarios=@($english|ForEach-Object{$_[1]}|Sort-Object -Unique)
Check (@(Compare-Object $scenarios $caseScenarios).Count -eq 0) 'Every English scenario has a CTest row'
Check (@($english|Where-Object{$_[1] -eq 'digit-navigation'}).Count -eq 2) 'Both mixing modes retain digit-navigation coverage'
$rows=@(
    foreach($phase in 'clean','reopen','different-directory'){[pscustomobject]@{name="release.ui.startup.$phase";preset='windows-x64';selected=$true;reason='fixture'}}
    foreach($keyboard in 1,3,2){[pscustomobject]@{name="release.ui.disabled.keyboard$keyboard";preset='windows-x64';selected=$true;reason='fixture'}}
)
$plan=@(Get-HarnessCasePlan $rows)
Check ($plan.Count -eq 9) 'Startup and initial/reopen UI checks produce nine subcases'
Check (@($plan|Where-Object status -ne 'not-run').Count -eq 0) 'Planning UI coverage never claims execution'
Check (@($plan.name|Sort-Object -Unique).Count -eq 9) 'UI subcase identities are unique'
foreach($case in $plan){Check ($case.parameters.architecture -eq 'x64') 'WinUI subcases keep their x64 host identity'}

# Exercise the actual level branches, so broad changes keep mapped UI checks in
# AFFECTED while the FULL/RELEASE inventories remain fixed across branch diffs.
$tokens=$null;$errors=$null
$driverAst=[System.Management.Automation.Language.Parser]::ParseFile("$project/scripts/test.ps1",[ref]$tokens,[ref]$errors)
foreach($Level in 'AFFECTED','FULL','RELEASE'){
    $requestedLabels=[Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    [void]$requestedLabels.Add('ui')
    $forceFull=$true;$skipTests=$false;$pathReasons=[ordered]@{};$notes=[Collections.Generic.List[string]]::new()
    $condition="`$Level -eq '$Level'";if($Level -eq 'AFFECTED'){$condition+=' -and !$skipTests'}
    $branch=$driverAst.Find({param($node) $node -is [System.Management.Automation.Language.IfStatementAst] -and $node.Clauses[0].Item1.Extent.Text -eq $condition -and $node.Extent.Text.Contains('$requestedLabels')},$false)
    . ([scriptblock]::Create($branch.Extent.Text))
    if($Level -eq 'AFFECTED'){Check ($requestedLabels.Contains('full') -and $requestedLabels.Contains('ui')) 'FULL expansion retains mapped UI cases'}
    elseif($Level -eq 'FULL'){Check ($requestedLabels.Count -eq 1 -and $requestedLabels.Contains('full')) 'FULL has a fixed inventory'}
    else{Check ($requestedLabels.Count -eq 2 -and $requestedLabels.Contains('full') -and $requestedLabels.Contains('release')) 'RELEASE has a fixed inventory'}
}

# Tool bytes are copied to a private executable path, and identical outputs keep their timestamps.
$source=Join-Path $ResultRoot 'tools-source'
$destination=Join-Path $ResultRoot 'tools-copy'
$null=New-Item -ItemType Directory -Path $source -Force
[IO.File]::WriteAllText("$source/shanjie-core.exe",'core fixture')
[IO.File]::WriteAllText("$source/old-tool.exe",'retired fixture')
Copy-TestTools $source $destination
Check (!((Get-Item $destination).Attributes -band [IO.FileAttributes]::ReparsePoint)) 'Fixture tools are real copies'
Check ((Get-FileHash "$source/shanjie-core.exe").Hash -eq (Get-FileHash "$destination/shanjie-core.exe").Hash) 'Private tool copy preserves bytes'
$stamp=[datetime]::SpecifyKind([datetime]'2001-01-01',[DateTimeKind]::Utc)
[IO.File]::SetLastWriteTimeUtc("$destination/shanjie-core.exe",$stamp)
Copy-TestTools $source $destination
Check ((Get-Item "$destination/shanjie-core.exe").LastWriteTimeUtc -eq $stamp) 'Identical private tools are not recopied'
[IO.File]::WriteAllText("$source/shanjie-core.exe",'changed core fixture')
Move-Item "$source/old-tool.exe" "$ResultRoot/retired-tool.exe"
Copy-TestTools $source $destination
Check ((Get-Content "$destination/shanjie-core.exe" -Raw) -eq 'changed core fixture') 'Changed tools invalidate the copy'
Check (!(Test-Path "$destination/old-tool.exe")) 'Retired tools leave the active fixture directory'

# A successful core result must invalidate on every input category, including the retained logs.
$resources=Join-Path $ResultRoot 'resources';$models=Join-Path $ResultRoot 'models'
$null=New-Item -ItemType Directory -Path "$resources/data","$resources/eval",$models -Force
[IO.File]::WriteAllText("$resources/data/input.txt",'data')
[IO.File]::WriteAllText("$resources/eval/input.txt",'eval')
[IO.File]::WriteAllText("$models/model.bin",'model')
$binary=Join-Path $ResultRoot 'test.exe';[IO.File]::WriteAllText($binary,'test executable')
$binaries=@([pscustomobject]@{Name='fixture';Path=$binary})
$arguments=@('--skip','unix-fixture')
$first=Get-CoreTestRunKey $binaries $resources $models $arguments 80
Check ($first -ceq (Get-CoreTestRunKey $binaries $resources $models $arguments 80)) 'Identical core inputs reuse one key'
foreach($file in @($binary,"$resources/data/input.txt","$resources/eval/input.txt","$models/model.bin")){
    $text=[IO.File]::ReadAllText($file)
    [IO.File]::AppendAllText($file,'changed')
    Check ($first -cne (Get-CoreTestRunKey $binaries $resources $models $arguments 80)) "Changed core input invalidates the key: $file"
    [IO.File]::WriteAllText($file,$text)
}
Check ($first -cne (Get-CoreTestRunKey $binaries $resources $models @('--skip','another-fixture') 80)) 'Changed arguments invalidate the core key'
$logs=Join-Path $ResultRoot 'target/test-logs';$null=New-Item -ItemType Directory -Path $logs -Force
[IO.File]::WriteAllText("$logs/fixture.txt",'test fixture ... ok')
$receipt=Join-Path $ResultRoot 'passed.json'
Save-ContentCache -Root $ResultRoot -ManifestPath $receipt -Key $first -Paths @('target/test-logs')
Check (Test-ContentCache -Root $ResultRoot -ManifestPath $receipt -Key $first -Paths @('target/test-logs')) 'Core result reuse retains verified output evidence'
[IO.File]::WriteAllText("$logs/fixture.txt",'changed evidence')
Check (!(Test-ContentCache -Root $ResultRoot -ManifestPath $receipt -Key $first -Paths @('target/test-logs'))) 'Changed output evidence rejects core reuse'

$tokens=$null;$errors=$null
$ast=[System.Management.Automation.Language.Parser]::ParseFile("$project/scripts/test-core.ps1",[ref]$tokens,[ref]$errors)
$sync=$ast.Find({param($node) $node -is [System.Management.Automation.Language.FunctionDefinitionAst] -and $node.Name -eq 'Sync-ChangedFiles'},$false)
. ([scriptblock]::Create($sync.Extent.Text))
$sourceTree=Join-Path $ResultRoot 'source';$output=Join-Path $ResultRoot 'tree'
$null=New-Item -ItemType Directory -Path $sourceTree,"$output/target" -Force
[IO.File]::WriteAllText("$sourceTree/core.rs",'source fixture')
[IO.File]::WriteAllText("$output/target/keep",'incremental output')
Check ((Sync-ChangedFiles $sourceTree $output) -eq 1) 'Changed-file sync writes the first source'
[IO.File]::SetLastWriteTimeUtc("$output/core.rs",$stamp)
Check ((Sync-ChangedFiles $sourceTree $output) -eq 0) 'Changed-file sync does not rewrite identical source'
Check ((Get-Item "$output/core.rs").LastWriteTimeUtc -eq $stamp) 'Unchanged source timestamps are preserved'
Check ((Get-Content "$output/target/keep" -Raw) -eq 'incremental output') 'Cargo target outputs survive source sync'
"PASS CTest integration: $checks checks; no Windows acceptance executed"
