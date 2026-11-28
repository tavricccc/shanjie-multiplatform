#Requires -Version 7
# Portable regressions for build outputs and change selection; no Windows products are built.
param([string]$ResultRoot)
$ErrorActionPreference='Stop'
$project=Split-Path $PSScriptRoot -Parent
Import-Module "$project/scripts/test-plan.psm1" -ErrorAction Stop
Import-Module "$project/scripts/test-support.psm1" -ErrorAction Stop
if(!$ResultRoot){$ResultRoot=Join-Path $project ('build/verification-paths-'+[guid]::NewGuid().ToString('N'))}
if(Test-Path -LiteralPath $ResultRoot){throw 'Use a fresh verification-path fixture directory.'}
$null=New-Item -ItemType Directory -Path $ResultRoot -Force
$checks=0
function Assert-PathCheck([bool]$Condition,[string]$Name){$script:checks++;if(!$Condition){throw "FAIL $Name"}}
function Read-DriverAst([string]$Path){
    $tokens=$null;$parseErrors=$null
    $ast=[System.Management.Automation.Language.Parser]::ParseFile($Path,[ref]$tokens,[ref]$parseErrors)
    if($parseErrors){throw "Invalid driver syntax: $($parseErrors.Message -join '; ')"}
    return $ast
}
$buildAst=Read-DriverAst "$project/scripts/build.ps1"
$buildCommand=$buildAst.Find({param($node) $node -is [System.Management.Automation.Language.CommandAst] -and $node.GetCommandName() -eq 'Invoke-Timed' -and $node.Extent.Text.Contains("-Category 'build.rust.release'")},$true)
$buildAction=($buildCommand.CommandElements|Where-Object {$_ -is [System.Management.Automation.Language.ScriptBlockExpressionAst]}|Select-Object -First 1).ScriptBlock.GetScriptBlock()
$targetAssignment=$buildAst.Find({param($node) $node -is [System.Management.Automation.Language.AssignmentStatementAst] -and $node.Left.Extent.Text -eq '$target'},$true)
$dllCopy=$buildAst.Find({param($node) $node -is [System.Management.Automation.Language.CommandAst] -and $node.GetCommandName() -eq 'Copy-Item' -and $node.CommandElements[2].Extent.Text -ceq '"$output/Shanjie.dll"'},$true)
# The mock honors Cargo's output precedence. Without --target-dir it writes to
# CARGO_TARGET_DIR, leaving the old fixed-path products in place for the real copy command.
function cargo {
    $script:buildArguments=@($args)
    $directoryIndex=[Array]::IndexOf($args,'--target-dir')
    $directory=if($directoryIndex -ge 0){$args[$directoryIndex+1]}else{$env:CARGO_TARGET_DIR}
    $triple=$args[[Array]::IndexOf($args,'--target')+1]
    $release=Join-Path $directory "$triple/release"
    $null=New-Item -ItemType Directory -Path $release -Force
    foreach($name in $products){[IO.File]::WriteAllText((Join-Path $release $name),"current:$triple`:$name")}
    $global:LASTEXITCODE=0
}
$previousTargetDir=$env:CARGO_TARGET_DIR
try {
    $root=Join-Path $ResultRoot 'cargo project'
    $env:CARGO_TARGET_DIR=Join-Path $ResultRoot 'redirected-target'
    $products=@('shanjie_windows.dll','shanjie_windows.lib','shanjie-manager.exe','shanjie-core.exe','shanjie-word-helper.exe','shanjie-setup.exe','shanjie-pack.exe')
    $CargoVerbose=$false
    foreach($Architecture in 'x64','x86'){
        . ([scriptblock]::Create($targetAssignment.Extent.Text))
        $expected=if($Architecture -eq 'x64'){'x86_64-pc-windows-msvc'}else{'i686-pc-windows-msvc'}
        Assert-PathCheck ($target -ceq $expected) "Separate target triple for $Architecture"
        $fixed=Join-Path $root "windows/target/$expected/release"
        $output=Join-Path $root "dist/$Architecture"
        $null=New-Item -ItemType Directory -Path $fixed,$output -Force
        foreach($name in $products){[IO.File]::WriteAllText((Join-Path $fixed $name),'stale')}
        & $buildAction
        . ([scriptblock]::Create($dllCopy.Extent.Text))
        Assert-PathCheck ((Get-Content -LiteralPath "$output/Shanjie.dll" -Raw) -ceq "current:$expected`:shanjie_windows.dll") "Copy current $Architecture DLL with a redirected environment and stale products"
        foreach($name in $products){Assert-PathCheck ((Get-Content -LiteralPath (Join-Path $fixed $name) -Raw) -ceq "current:$expected`:$name") "Current $Architecture product at the fixed path: $name"}
        Assert-PathCheck ($buildArguments[[Array]::IndexOf($buildArguments,'--target-dir')+1] -ceq "$root/windows/target") 'Keep the existing incremental target root'
        Assert-PathCheck (('--config' -in $buildArguments) -eq ($Architecture -eq 'x64')) 'Use the optimized profile on x64 only'
        $PackageTargetsOnly=$true
        & $buildAction
        $bins=@(for($index=0;$index -lt $buildArguments.Count;$index++){
            if($buildArguments[$index] -eq '--bin'){$buildArguments[$index+1]}
        })
        $expectedBins=@('shanjie-reliability-check')+@(if($Architecture -eq 'x64'){
            $products | Where-Object { $_.EndsWith('.exe') } | ForEach-Object { [IO.Path]::GetFileNameWithoutExtension($_) }
        })
        Assert-PathCheck ('--lib' -in $buildArguments -and ($bins -join ';') -ceq ($expectedBins -join ';')) "RELEASE builds only packaged $Architecture targets and the reliability probe"
        $PackageTargetsOnly=$false
    }
    Assert-PathCheck (!(Test-Path -LiteralPath $env:CARGO_TARGET_DIR)) 'The ambient target directory was not used'
    Assert-PathCheck ((Get-Content -LiteralPath "$root/dist/x64/Shanjie.dll" -Raw) -ceq 'current:x86_64-pc-windows-msvc:shanjie_windows.dll') 'The x86 build did not overwrite the x64 DLL'
} finally {$env:CARGO_TARGET_DIR=$previousTargetDir;Remove-Item Function:cargo}

$cacheDriver="$project/scripts/cargo-input-cache.ps1"
$resourceLoop=$buildAst.Find({param($node) $node -is [Management.Automation.Language.ForEachStatementAst] -and $node.Extent.Text.Contains("foreach("+'$name'+" in 'mode_icons','setup')")},$true)
foreach($cacheArch in 'x64','x86'){
    $cacheRoot=Join-Path $ResultRoot "cargo-input-cache-$cacheArch"
    $cacheSources=@('windows/src/lib.rs','windows/build.rs','shared/src/lib.rs','platform/half_punctuation.rs','profiles/windows.ini')
    $cacheObjects=@('windows/target/deps/library.rlib',"build/native-$cacheArch/mode_icons.res","build/native-$cacheArch/setup.res")
    $originalTime=[DateTime]::Parse('2026-01-01T00:00:00Z').ToUniversalTime().AddTicks(1234567)
    $newTime=$originalTime.AddDays(2)
    foreach($path in $cacheSources+$cacheObjects){
        $file=Join-Path $cacheRoot $path
        $null=New-Item -ItemType Directory -Path (Split-Path $file -Parent) -Force
        [IO.File]::WriteAllText($file,"fixture:$path")
        [IO.File]::SetLastWriteTimeUtc($file,$originalTime)
    }
    & $cacheDriver -Operation Restore -Architecture $cacheArch -Context 'compiler-profile-v1' -Root $cacheRoot
    Assert-PathCheck ((Get-Item "$cacheRoot/windows/src/lib.rs").LastWriteTimeUtc -eq $originalTime) "$cacheArch cold cache leaves source metadata intact"
    & $cacheDriver -Operation Save -Architecture $cacheArch -Context 'compiler-profile-v1' -Root $cacheRoot
    foreach($path in $cacheSources){[IO.File]::SetLastWriteTimeUtc((Join-Path $cacheRoot $path),$newTime)}
    & $cacheDriver -Operation Restore -Architecture $cacheArch -Context 'compiler-profile-v1' -Root $cacheRoot
    foreach($path in $cacheSources){Assert-PathCheck ((Get-Item (Join-Path $cacheRoot $path)).LastWriteTimeUtc -eq $originalTime) "$cacheArch identical input restores its recorded timestamp: $path"}
    Assert-PathCheck ((Get-Item "$cacheRoot/windows/target/deps/library.rlib").LastWriteTimeUtc -eq $originalTime) "$cacheArch restore never rewrites compiled artifact timestamps"
    foreach($case in 'content','added','missing','context'){
        foreach($path in $cacheSources){
            [IO.File]::WriteAllText((Join-Path $cacheRoot $path),"fixture:$path")
            [IO.File]::SetLastWriteTimeUtc((Join-Path $cacheRoot $path),$newTime)
        }
        if($case -eq 'content'){[IO.File]::WriteAllText("$cacheRoot/shared/src/lib.rs",'gixture:shared/src/lib.rs')}
        if($case -eq 'added'){[IO.File]::WriteAllText("$cacheRoot/windows/src/new.rs",'new input')}
        if($case -eq 'missing'){Remove-Item -LiteralPath "$cacheRoot/shared/src/lib.rs"}
        $context=if($case -eq 'context'){'different-compiler-or-flags'}else{'compiler-profile-v1'}
        & $cacheDriver -Operation Restore -Architecture $cacheArch -Context $context -Root $cacheRoot
        Assert-PathCheck ((Get-Item "$cacheRoot/windows/src/lib.rs").LastWriteTimeUtc -eq $newTime) "$cacheArch $case mismatch cannot authorize any timestamp restoration"
        if($case -eq 'content'){Assert-PathCheck ((Get-Content "$cacheRoot/shared/src/lib.rs" -Raw) -ceq 'gixture:shared/src/lib.rs') "$cacheArch cache never overwrites modified source content"}
        if($case -eq 'added'){Remove-Item -LiteralPath "$cacheRoot/windows/src/new.rs"}
        if($case -eq 'missing'){Assert-PathCheck (!(Test-Path "$cacheRoot/shared/src/lib.rs")) "$cacheArch cache never recreates missing source"}
    }
    foreach($path in $cacheSources){
        [IO.File]::WriteAllText((Join-Path $cacheRoot $path),"fixture:$path")
        [IO.File]::SetLastWriteTimeUtc((Join-Path $cacheRoot $path),$newTime)
    }
    [IO.File]::WriteAllText("$cacheRoot/windows/target/deps/library.rlib",'corrupted cached output')
    $rejected=$false
    try{& $cacheDriver -Operation Restore -Architecture $cacheArch -Context 'compiler-profile-v1' -Root $cacheRoot}catch{$rejected=$true}
    Assert-PathCheck ($rejected -and (Get-Item "$cacheRoot/windows/src/lib.rs").LastWriteTimeUtc -eq $newTime) "$cacheArch changed artifacts fail before any source timestamp is restored"

    # Execute the real catalog driver against an invalid cache. Only the product
    # build and native checker are mocked; restoration and quarantine are real.
    $driverRoot=Join-Path $ResultRoot "catalog-driver-$cacheArch"
    $null=New-Item -ItemType Directory -Path "$driverRoot/scripts/model_catalog" -Force
    foreach($folder in 'windows','shared','platform','profiles','build'){
        Copy-Item -LiteralPath (Join-Path $cacheRoot $folder) -Destination $driverRoot -Recurse
    }
    Copy-Item -LiteralPath "$project/scripts/model_catalog/probe.ps1" -Destination "$driverRoot/scripts/model_catalog/probe.ps1"
    foreach($script in 'cargo-input-cache.ps1','test-support.psm1','preserve-output.ps1'){
        Copy-Item -LiteralPath "$project/scripts/$script" -Destination "$driverRoot/scripts/$script"
    }
    [IO.File]::WriteAllText("$driverRoot/scripts/build.ps1",@'
param([string]$Architecture,[switch]$SkipSettings,[switch]$SkipSharedData,[switch]$CargoVerbose,[switch]$UseCurrentToolchain)
$root=Split-Path $PSScriptRoot -Parent
if(Test-Path "$root/windows/target"){throw 'Rejected target directory reached the build.'}
$null=New-Item -ItemType Directory -Path "$root/windows/target/deps" -Force
[IO.File]::WriteAllText("$root/windows/target/deps/library.rlib",'rebuilt output')
@{architecture=$Architecture;skip_settings=[bool]$SkipSettings;skip_shared_data=[bool]$SkipSharedData;verbose=[bool]$CargoVerbose}|ConvertTo-Json|Set-Content "$root/build-call.json"
$global:LASTEXITCODE=0
'@)
    if($cacheArch -eq 'x86'){
        $null=New-Item -ItemType Directory -Path "$driverRoot/dist/data/models/builtin" -Force
        [IO.File]::WriteAllText("$driverRoot/dist/data/models/builtin/bigram.winmap",'restored model cache without runtime data')
        [IO.File]::WriteAllText("$driverRoot/scripts/prepare-models.ps1",@'
$root=Split-Path $PSScriptRoot -Parent
if(!(Test-Path "$root/dist/data/models/builtin/bigram.winmap")){throw 'Expected cached built-in model fixture.'}
[IO.File]::AppendAllText("$root/data-call.txt","models`n")
'@)
        [IO.File]::WriteAllText("$driverRoot/scripts/prepare-data.ps1",@'
param([string]$Licenses)
$root=Split-Path $PSScriptRoot -Parent
if($Licenses -ne 'dist/LICENSES'){throw 'Licenses were not prepared.'}
[IO.File]::WriteAllText("$root/dist/data/dictionary.tsv",'runtime data from the pinned source')
[IO.File]::AppendAllText("$root/data-call.txt","data`n")
'@)
    }
    function python {ConvertTo-Json -InputObject @($args)|Set-Content "$driverRoot/probe-call.json";$global:LASTEXITCODE=0}
    try { & "$driverRoot/scripts/model_catalog/probe.ps1" -Architecture $cacheArch -CacheContext 'compiler-profile-v1' }
    finally {Remove-Item Function:python}
    $built=Get-Content "$driverRoot/build-call.json" -Raw|ConvertFrom-Json
    Assert-PathCheck ($built.architecture -eq $cacheArch -and $built.skip_settings -and $built.verbose) "$cacheArch rejected cache still builds the same verbose product targets"
    if($cacheArch -eq 'x86'){
        Assert-PathCheck ($built.skip_shared_data -and (Get-Content "$driverRoot/data-call.txt" -Raw) -ceq "models`ndata`n" -and (Test-Path "$driverRoot/dist/data/dictionary.tsv")) 'Independent x86 runner validates cached models and prepares missing runtime data exactly once'
    }else{Assert-PathCheck (!$built.skip_shared_data) 'x64 keeps its normal model and runtime data preparation'}
    Assert-PathCheck ((Get-Content "$driverRoot/windows/target/deps/library.rlib" -Raw) -eq 'rebuilt output') "$cacheArch corrupted compilation output is replaced by a normal build"
    Assert-PathCheck ((@(Get-ChildItem "$driverRoot/windows/.deleted_files" -Directory)).Count -eq 1) "$cacheArch invalid cache is retained for diagnosis"
    $saved=Get-Content "$driverRoot/build/cargo-inputs/$cacheArch.json" -Raw|ConvertFrom-Json -AsHashtable
    Assert-PathCheck ($saved.artifacts['windows/target/deps/library.rlib'] -eq (Get-FileHash "$driverRoot/windows/target/deps/library.rlib").Hash.ToLowerInvariant()) "$cacheArch successful rebuild saves a fresh integrity receipt"
    $catalogArguments=Get-Content "$driverRoot/probe-call.json" -Raw|ConvertFrom-Json
    Assert-PathCheck ('probe' -in $catalogArguments -and "windows-$cacheArch" -in $catalogArguments -and 'discover' -notin $catalogArguments -and 'assemble' -notin $catalogArguments) "$cacheArch driver probes only its architecture without discovering or assembling on a worker"

    $objects=Join-Path $cacheRoot "build/native-$cacheArch"
    foreach($name in 'mode_icons','setup'){
        [IO.File]::WriteAllText("$objects/$name.res.tmp",(Get-Content "$objects/$name.res" -Raw))
        [IO.File]::SetLastWriteTimeUtc("$objects/$name.res.tmp",$newTime)
    }
    . ([scriptblock]::Create($resourceLoop.Extent.Text))
    Assert-PathCheck ((Get-Item "$objects/mode_icons.res").LastWriteTimeUtc -eq $originalTime) "$cacheArch byte-identical resources retain the cached timestamp"
    $resourceBytes=Get-Content "$objects/mode_icons.res" -Raw
    Remove-Item -LiteralPath "$objects/mode_icons.res"
    foreach($name in 'mode_icons','setup'){
        $bytes=if($name -eq 'mode_icons'){$resourceBytes}else{Get-Content "$objects/setup.res" -Raw}
        [IO.File]::WriteAllText("$objects/$name.res.tmp",$bytes)
        [IO.File]::SetLastWriteTimeUtc("$objects/$name.res.tmp",$newTime)
    }
    . ([scriptblock]::Create($resourceLoop.Extent.Text))
    Assert-PathCheck ((Get-Item "$objects/mode_icons.res").LastWriteTimeUtc -eq $newTime -and (Get-Content "$objects/mode_icons.res" -Raw) -ceq $resourceBytes) "$cacheArch missing native cache regenerates identical bytes with a new timestamp"
    [IO.File]::WriteAllText("$objects/mode_icons.res.tmp",'changed resource bytes')
    [IO.File]::WriteAllText("$objects/setup.res.tmp",(Get-Content "$objects/setup.res" -Raw))
    . ([scriptblock]::Create($resourceLoop.Extent.Text))
    Assert-PathCheck ((Get-Content "$objects/mode_icons.res" -Raw) -ceq 'changed resource bytes') "$cacheArch changed resource output replaces the cached resource"
}

$verifyAst=Read-DriverAst "$project/scripts/verify.ps1"
$runtimeAssignment=$verifyAst.Find({param($node) $node -is [Management.Automation.Language.AssignmentStatementAst] -and $node.Left.Extent.Text -eq '$runtimeRoot'},$true)
$modelGuard=$verifyAst.Find({param($node) $node -is [Management.Automation.Language.IfStatementAst] -and $node.Extent.Text.Contains("throw 'Run prepare-models.ps1 first to prepare the built-in model set.'")},$true)
foreach($Architecture in 'x64','x86'){
    $root=[IO.Path]::GetFullPath((Join-Path $ResultRoot "model-root-$Architecture"))
    $PayloadRoot=Join-Path $root "build/test-payload-$Architecture"
    $model=Join-Path $PayloadRoot 'data/models/builtin/bigram.winmap'
    $null=New-Item -ItemType Directory -Path (Split-Path $model -Parent) -Force
    [IO.File]::WriteAllText($model,'synthetic sealed model')
    Push-Location $root
    try {
        . ([scriptblock]::Create($runtimeAssignment.Extent.Text))
        . ([scriptblock]::Create($modelGuard.Extent.Text))
        Assert-PathCheck (!(Test-Path "$root/dist")) "$Architecture smoke uses its payload model without a live dist tree"
        $legacyModel=Join-Path $root 'dist/data/models/builtin/bigram.winmap'
        $null=New-Item -ItemType Directory -Path (Split-Path $legacyModel -Parent) -Force
        [IO.File]::WriteAllText($legacyModel,'unrelated live dist model')
        Move-Item -LiteralPath $model -Destination "$model.missing"
        $rejected=$false
        try{. ([scriptblock]::Create($modelGuard.Extent.Text))}catch{$rejected=$true}
        finally{Move-Item -LiteralPath "$model.missing" -Destination $model}
        Assert-PathCheck $rejected "$Architecture smoke rejects a missing payload model even when live dist has one"
        $PayloadRoot=$null
        . ([scriptblock]::Create($runtimeAssignment.Extent.Text))
        . ([scriptblock]::Create($modelGuard.Extent.Text))
        Assert-PathCheck ($runtimeRoot -ceq [IO.Path]::GetFullPath("$root/dist")) "$Architecture legacy smoke retains the dist fallback"
    } finally {Pop-Location}
}

$testAst=Read-DriverAst "$project/scripts/test.ps1"
foreach($name in 'Get-ChangedPathList','Test-DocumentationOnly'){
    $definition=$testAst.Find({param($node) $node -is [System.Management.Automation.Language.FunctionDefinitionAst] -and $node.Name -eq $name},$false)
    . ([scriptblock]::Create($definition.Extent.Text))
}
function Invoke-FixtureGit {
    & git -C $root @args
    if($LASTEXITCODE){throw "Fixture git failed: $args"}
}
foreach($state in 'staged','committed','unstaged','docs-only'){
    $root=Join-Path $ResultRoot "git-$state"
    $null=New-Item -ItemType Directory -Path "$root/windows/src","$root/docs" -Force
    [IO.File]::WriteAllText("$root/windows/src/keys.rs",'fn fixture() {}')
    [IO.File]::WriteAllText("$root/docs/old.md",'Documentation fixture')
    Invoke-FixtureGit init -b test/paths *> $null
    Invoke-FixtureGit config diff.renames true
    Invoke-FixtureGit config core.autocrlf false
    Invoke-FixtureGit add -- .
    Invoke-FixtureGit -c user.name=VerificationPaths -c user.email=paths@example.invalid -c commit.gpgsign=false -c core.hooksPath=NUL commit -m 'test: seed path fixture' *> $null
    $base=Invoke-FixtureGit rev-parse HEAD
    $old=if($state -eq 'docs-only'){'docs/old.md'}else{'windows/src/keys.rs'}
    $new=if($state -eq 'docs-only'){'docs/new.md'}else{'docs/keys.rs'}
    Move-Item -LiteralPath (Join-Path $root $old) -Destination (Join-Path $root $new)
    if($state -ne 'unstaged'){Invoke-FixtureGit add --all}
    if($state -eq 'committed'){Invoke-FixtureGit -c user.name=VerificationPaths -c user.email=paths@example.invalid -c commit.gpgsign=false -c core.hooksPath=NUL commit -m 'test: move source into docs' *> $null}
    $changed=Get-ChangedPathList $base
    Assert-PathCheck (!$changed.Error) "Read $state changes"
    Assert-PathCheck ($old -cin $changed.Paths -and $new -cin $changed.Paths) "Retain original and new $state paths"
    $code=@($changed.Paths|Where-Object {!(Test-DocumentationOnly $_)})
    if($state -eq 'docs-only'){Assert-PathCheck (!$code.Count) 'Pure documentation renames can still skip builds and tests'}
    else{Assert-PathCheck ($code.Count -gt 0) "A $state source move into docs cannot skip FAST/AFFECTED"}
}
foreach($catalogPath in '.github/workflows/model-catalog-maintenance.yml','scripts/model_catalog/catalog.py','scripts/model_catalog/probe.ps1','scripts/model_catalog/test_catalog.py'){
    $impact=Get-VerificationImpact $catalogPath
    Assert-PathCheck (!$impact.Full -and 'test-framework' -in $impact.Labels) "$catalogPath selects the catalog and infrastructure regressions"
}
"PASS verification paths: $checks checks (mock Cargo outputs and real Git rename/cache fixtures; no Windows acceptance)"
