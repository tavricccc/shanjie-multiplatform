#Requires -Version 7
param(
    [Parameter(Mandatory)][ValidateSet('x64','x86')][string]$Architecture,
    [string]$Output='build/model-catalog',
    [string]$CacheContext=$env:SHANJIE_CARGO_CACHE_CONTEXT,
    [switch]$UseCurrentToolchain
)
$ErrorActionPreference='Stop'
$project=Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
Push-Location $project
try {
    if($CacheContext){
        try {
            & "$project/scripts/cargo-input-cache.ps1" -Operation Restore -Architecture $Architecture -Context $CacheContext
        } catch {
            # Rejected compiled outputs cannot be reused. Preserve them for
            # diagnosis and let the ordinary build recreate the files.
            Write-Warning "Cargo cache rejected; rebuilding: $($_.Exception.Message)"
            . "$project/scripts/preserve-output.ps1"
            foreach($path in 'windows/target','shared/target',"build/native-$Architecture","build/cargo-inputs/$Architecture.json"){
                Preserve-Output (Join-Path $project $path)
            }
        }
    }
    & "$project/scripts/build.ps1" -Architecture $Architecture -SkipSettings -SkipSharedData:($Architecture -eq 'x86') -CargoVerbose -UseCurrentToolchain:$UseCurrentToolchain
    if($LASTEXITCODE){throw 'Native checker build failed.'}
    if($Architecture -eq 'x86'){
        # On a separate runner the cached built-in model does not imply that
        # runtime data exists or is current. Prepare both exactly once here.
        & "$project/scripts/prepare-models.ps1"
        & "$project/scripts/prepare-data.ps1" -Licenses 'dist/LICENSES'
    }
    if($CacheContext){
        & "$project/scripts/cargo-input-cache.ps1" -Operation Save -Architecture $Architecture -Context $CacheContext
    }
    $target=if($Architecture -eq 'x64'){'x86_64-pc-windows-msvc'}else{'i686-pc-windows-msvc'}
    python scripts/model_catalog/catalog.py probe --out $Output --target "windows-$Architecture" --checker "windows/target/$target/release/shanjie-model-check.exe" --data dist/data
    if($LASTEXITCODE){throw 'Native probe infrastructure failed.'}
} finally{Pop-Location}
