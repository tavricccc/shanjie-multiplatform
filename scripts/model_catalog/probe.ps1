#Requires -Version 7
param([string]$Output='build/model-catalog')
$ErrorActionPreference='Stop'
$project=Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
Push-Location $project
try {
    python scripts/model_catalog/catalog.py discover --out $Output
    if($LASTEXITCODE){throw 'Discovery failed; no publication.'}
    foreach($arch in 'x64','x86'){
        & ./scripts/build.ps1 -Architecture $arch -SkipSettings
        if($LASTEXITCODE){throw 'Native checker build failed.'}
        $target=if($arch -eq 'x64'){'x86_64-pc-windows-msvc'}else{'i686-pc-windows-msvc'}
        python scripts/model_catalog/catalog.py probe --out $Output --target "windows-$arch" --checker "windows/target/$target/release/shanjie-model-check.exe" --data dist/data
        if($LASTEXITCODE){throw 'Native probe infrastructure failed.'}
    }
    python scripts/model_catalog/catalog.py assemble --out $Output
    if($LASTEXITCODE){throw 'Catalog assembly failed.'}
    python scripts/model_catalog/catalog.py validate --out $Output
    if($LASTEXITCODE){throw 'Catalog validation failed.'}
} finally{Pop-Location}
