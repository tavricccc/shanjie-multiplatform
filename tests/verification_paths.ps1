#Requires -Version 7
# Portable regressions for build outputs and change selection; no Windows products are built.
param([string]$ResultRoot)
$ErrorActionPreference='Stop'
$project=Split-Path $PSScriptRoot -Parent
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
    }
    Assert-PathCheck (!(Test-Path -LiteralPath $env:CARGO_TARGET_DIR)) 'The ambient target directory was not used'
    Assert-PathCheck ((Get-Content -LiteralPath "$root/dist/x64/Shanjie.dll" -Raw) -ceq 'current:x86_64-pc-windows-msvc:shanjie_windows.dll') 'The x86 build did not overwrite the x64 DLL'
} finally {$env:CARGO_TARGET_DIR=$previousTargetDir;Remove-Item Function:cargo}

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
"PASS verification paths: $checks checks (mock Cargo outputs and real Git rename fixtures; no Windows acceptance)"
