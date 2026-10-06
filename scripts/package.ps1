#Requires -Version 7
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
Push-Location $root
try {
    $model = Join-Path $root 'build/model/bigram.sjlm'
    $expected = (Get-Content "$root/upstream/shanjie/data/bigram.sjlm.sha256" -Raw).Split(' ')[0]
    if (!(Test-Path $model) -or (Get-FileHash $model -Algorithm SHA256).Hash.ToLowerInvariant() -ne $expected) {
        New-Item -ItemType Directory -Path (Split-Path $model) -Force | Out-Null
        curl.exe --fail --location --silent --show-error --output "$model.download" 'https://github.com/Nanako0129/shanjie/releases/download/model-v1/bigram.sjlm'
        if ($LASTEXITCODE) { throw 'Model download failed.' }
        if ((Get-FileHash "$model.download" -Algorithm SHA256).Hash.ToLowerInvariant() -ne $expected) { throw 'Model SHA-256 mismatch.' }
        Move-Item -LiteralPath "$model.download" -Destination $model -Force
    }
    & "$PSScriptRoot/build.ps1" -Architecture x64
    & "$PSScriptRoot/build.ps1" -Architecture x86
    $output = Join-Path $root 'dist/package'
    New-Item -ItemType Directory -Path $output -Force | Out-Null
    # Packaging conversion always runs in a native x64 process, including x86 packages.
    cargo run --release --manifest-path build/core/Cargo.toml --bin map-model -- $model "$root/dist/data/bigram.winmap"
    if ($LASTEXITCODE) { throw 'Mapped model conversion or equivalence verification failed.' }
    foreach ($arch in @('x64','x86')) {
        New-Item -ItemType Directory -Path "$output/$arch" -Force | Out-Null
        Copy-Item "$root/dist/$arch/Shanjie.dll" "$output/$arch" -Force
    }
    Copy-Item "$root/dist/data","$root/dist/LICENSES" $output -Recurse -Force
    Copy-Item "$PSScriptRoot/install.ps1","$PSScriptRoot/uninstall.ps1","$PSScriptRoot/install.cmd","$PSScriptRoot/uninstall.cmd","$PSScriptRoot/bootstrap.cmd","$PSScriptRoot/elevate.ps1","$PSScriptRoot/profiles.ps1","$root/UPSTREAM.json","$root/README.md","$root/VERIFICATION.md" $output -Force
    Copy-Item "$PSScriptRoot/apply-profile.ps1","$PSScriptRoot/apply-windows.cmd","$PSScriptRoot/apply-shanjie.cmd" $output -Force
    Copy-Item "$root/profiles" $output -Recurse -Force
    Copy-Item "$root/dist/settings.exe","$PSScriptRoot/settings.cmd" $output -Force
    Copy-Item "$root/UPSTREAM-CHANGES.md" $output -Force
    Copy-Item "$root/安裝說明.txt" $output -Force
    $hashes = Get-ChildItem $output -File -Recurse | Where-Object Name -ne 'SHA256SUMS.txt' | ForEach-Object {
        $relative = [System.IO.Path]::GetRelativePath($output,$_.FullName)
        "$( (Get-FileHash $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant() )  $relative"
    }
    Set-Content "$output/SHA256SUMS.txt" $hashes -Encoding utf8NoBOM
    Compress-Archive -Path "$output/*" -DestinationPath "$root/dist/Shanjie-Windows.zip" -Force
} finally { Pop-Location }
