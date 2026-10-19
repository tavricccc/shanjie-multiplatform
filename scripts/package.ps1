#Requires -Version 7
param([switch]$UseExistingBuild)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
. "$PSScriptRoot/preserve-output.ps1"
Push-Location $root
try {
    if (!$UseExistingBuild) {
        & "$PSScriptRoot/build.ps1"
    }
    $output = Join-Path $root 'dist/package'
    if ([IO.Path]::GetFullPath($output) -ne [IO.Path]::Combine([IO.Path]::GetFullPath($root),'dist','package')) { throw 'Unexpected package output path.' }
    Preserve-Output $output
    New-Item -ItemType Directory -Path $output -Force | Out-Null
    if (!$UseExistingBuild) { & "$PSScriptRoot/prepare-models.ps1" }
    & "$PSScriptRoot/prepare-licenses.ps1"
    foreach ($arch in @('x64','x86')) {
        New-Item -ItemType Directory -Path "$output/$arch" -Force | Out-Null
        Copy-Item "$root/dist/$arch/Shanjie.dll" "$output/$arch" -Force
    }
    Copy-Item "$root/dist/data","$root/dist/LICENSES" $output -Recurse -Force
    Copy-Item "$root/LICENSES/windows-models.md" "$output/LICENSES" -Force
    Copy-Item "$PSScriptRoot/install.ps1","$PSScriptRoot/uninstall.ps1","$PSScriptRoot/install.cmd","$PSScriptRoot/uninstall.cmd","$PSScriptRoot/profiles.ps1","$root/UPSTREAM.json","$root/README.md","$root/Changelog.md" $output -Force
    Copy-Item "$PSScriptRoot/apply-profile.ps1","$PSScriptRoot/apply-windows.cmd","$PSScriptRoot/apply-shanjie.cmd" $output -Force
    Copy-Item "$root/profiles" $output -Recurse -Force
    Copy-Item "$root/dist/ui" $output -Recurse -Force
    New-Item -ItemType Directory -Path "$output/tools" -Force | Out-Null
    Get-ChildItem "$root/dist/tools" -File | Copy-Item -Destination "$output/tools" -Force
    Copy-Item "$PSScriptRoot/settings.cmd" $output -Force
    Copy-Item "$PSScriptRoot/重新選擇鍵盤.cmd" $output -Force
    Copy-Item "$root/UPSTREAM-CHANGES.md" $output -Force
    Copy-Item "$root/docs" $output -Recurse -Force
    Copy-Item "$root/MODEL-SOURCES.json","$root/MODEL-CATALOG.json" $output -Force
    Copy-Item "$root/安裝說明.txt" $output -Force
    $hashes = Get-ChildItem $output -File -Recurse | Where-Object Name -ne 'SHA256SUMS.txt' | ForEach-Object {
        $relative = [System.IO.Path]::GetRelativePath($output,$_.FullName)
        "$( (Get-FileHash $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant() )  $relative"
    }
    Set-Content "$output/SHA256SUMS.txt" $hashes -Encoding utf8NoBOM
    & "$output/tools/shanjie-manager.exe" verify-package $output
    if ($LASTEXITCODE) { throw 'Native package verification failed.' }
    Preserve-Output "$root/dist/Shanjie-Windows.zip"
    Compress-Archive -Path "$output/*" -DestinationPath "$root/dist/Shanjie-Windows.zip"
} finally { Pop-Location }
