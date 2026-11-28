Import-Module "$PSScriptRoot/test-support.psm1" -ErrorAction Stop
Import-Module "$PSScriptRoot/test-plan.psm1" -ErrorAction Stop

function Get-CiEnvironment([string]$Path) {
    if(!(Test-Path -LiteralPath $Path)){
        & python "$PSScriptRoot/release_evidence.py" environment --output $Path
        if($LASTEXITCODE){throw 'Cannot identify the CI execution environment.'}
    }
    Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json -AsHashtable
}

function New-CiPlan([string]$ProjectRoot,$Identity,[string]$SnapshotId,[string]$BundleRoot,[string]$ReuseRoot,[string]$Directory) {
    $inventories=@{}
    foreach($arch in 'x64','x86'){
        $preset="windows-$arch"
        & cmake --preset $preset | Out-Host
        if($LASTEXITCODE){throw "Cannot configure the $preset CI catalog."}
        $raw=& ctest --test-dir (Join-Path $ProjectRoot "build/cmake/$preset") -C Release --show-only=json-v1
        if($LASTEXITCODE){throw "Cannot read the $preset CI catalog."}
        $inventory=($raw|Out-String)|ConvertFrom-Json
        foreach($test in $inventory.tests){
            $subcases=@(Get-HarnessCasePlan @([pscustomobject]@{name=$test.name;preset=$preset;selected=$true;reason='Sealed CI catalog'}))
            $test | Add-Member -NotePropertyName subcases -NotePropertyValue $subcases
        }
        $inventories[$preset]=$inventory
    }
    $environment=Get-CiEnvironment "$Directory/validation-environment.json"
    $reused=@{}
    if($ReuseRoot){
        & python "$PSScriptRoot/release_evidence.py" candidates --root $ReuseRoot --bundle $BundleRoot --environment-file "$Directory/validation-environment.json" --output "$Directory/reused-core.json" | Out-Host
        if($LASTEXITCODE){throw 'Cannot validate Core reuse candidates.'}
        $reused=Get-Content "$Directory/reused-core.json" -Raw | ConvertFrom-Json -AsHashtable
    }
    $inputPath=Join-Path $Directory 'plan-input.json'
    $source=@{commit=$Identity.commit;version=$Identity.version;snapshot_id=$SnapshotId;run_id=$env:GITHUB_RUN_ID;run_attempt=$env:GITHUB_RUN_ATTEMPT}
    & python -c 'import sys;sys.path.insert(0,sys.argv[1]);import release_evidence;print(release_evidence.local_fingerprint(sys.argv[2]))' $PSScriptRoot $ProjectRoot | Set-Content "$Directory/input-fingerprint.txt" -Encoding utf8NoBOM
    if($LASTEXITCODE){throw 'Cannot fingerprint product and test inputs.'}
    $source.test_input_fingerprint=(Get-Content "$Directory/input-fingerprint.txt" -Raw).Trim()
    @{inventories=$inventories;source=$source;bundle=(Get-Content "$BundleRoot/bundle.json" -Raw|ConvertFrom-Json);reused_core=$reused;validation_environment=$environment} |
        ConvertTo-Json -Depth 30 | Set-Content $inputPath -Encoding utf8NoBOM
    & python "$PSScriptRoot/release_ci.py" plan --input $inputPath --output "$Directory/ci-plan.json" | Out-Host
    if($LASTEXITCODE){throw 'Cannot create a complete CI partition plan.'}
    Get-Content "$Directory/ci-plan.json" -Raw | ConvertFrom-Json -AsHashtable
}

function Get-CiSelection($Snapshot,[string]$Partition,[string]$Directory,$Inventory,[string]$Preset) {
    $inputPath=Join-Path $Directory 'ci-inventory.json'
    @{preset=$Preset;inventory=$Inventory}|ConvertTo-Json -Depth 25|Set-Content $inputPath -Encoding utf8NoBOM
    $null=Get-CiEnvironment "$Directory/ci-environment.json"
    & python "$PSScriptRoot/release_ci.py" selection --snapshot-root $Snapshot.root --partition $Partition --env-file "$Directory/ci-environment.json" --input $inputPath --output "$Directory/ci-selection.json" | Out-Host
    if($LASTEXITCODE){throw 'CTest inventory or partition assignment changed.'}
    Get-Content "$Directory/ci-selection.json" -Raw|ConvertFrom-Json -AsHashtable
}

function Complete-CiTestReport($Snapshot,[string]$Directory,[string]$Partition) {
    & python "$PSScriptRoot/release_ci.py" collect --snapshot-root $Snapshot.root --snapshot-hash $Snapshot.sha256 --partition $Partition --evidence-root $Directory
    if($LASTEXITCODE){throw 'Could not seal per-test CI evidence.'}
}

Export-ModuleMember -Function New-CiPlan,Get-CiSelection,Complete-CiTestReport
