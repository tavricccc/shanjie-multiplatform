#Requires -Version 7
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
Push-Location $root
try {
    & "$PSScriptRoot/build.ps1" -Architecture x64
    & "$PSScriptRoot/build.ps1" -Architecture x86
    $output = Join-Path $root 'dist/package'
    if ([IO.Path]::GetFullPath($output) -ne [IO.Path]::Combine([IO.Path]::GetFullPath($root),'dist','package')) { throw 'Unexpected package output path.' }
    if (Test-Path -LiteralPath $output) { Remove-Item -LiteralPath $output -Recurse -Force }
    New-Item -ItemType Directory -Path $output -Force | Out-Null
    & "$PSScriptRoot/prepare-models.ps1"
    foreach ($arch in @('x64','x86')) {
        New-Item -ItemType Directory -Path "$output/$arch" -Force | Out-Null
        Copy-Item "$root/dist/$arch/Shanjie.dll" "$output/$arch" -Force
    }
    Copy-Item "$root/dist/data","$root/dist/LICENSES" $output -Recurse -Force
    Copy-Item "$PSScriptRoot/install.ps1","$PSScriptRoot/uninstall.ps1","$PSScriptRoot/install.cmd","$PSScriptRoot/uninstall.cmd","$PSScriptRoot/bootstrap.cmd","$PSScriptRoot/elevate.ps1","$PSScriptRoot/profiles.ps1","$root/UPSTREAM.json","$root/README.md","$root/VERIFICATION.md" $output -Force
    Copy-Item "$PSScriptRoot/apply-profile.ps1","$PSScriptRoot/profile-ini.ps1","$PSScriptRoot/apply-windows.cmd","$PSScriptRoot/apply-shanjie.cmd" $output -Force
    Copy-Item "$root/profiles" $output -Recurse -Force
    Copy-Item "$root/dist/ui" $output -Recurse -Force
    Copy-Item "$PSScriptRoot/settings.cmd" $output -Force
    Copy-Item "$root/UPSTREAM-CHANGES.md" $output -Force
    Copy-Item "$root/MODEL-SOURCES.json" $output -Force
    Copy-Item "$root/WINUI-MIGRATION.md","$root/MICROSOFT-IME-DIFFS.md","$root/DESIGN.md","$root/PERFORMANCE.md" $output -Force
    Copy-Item "$root/安裝說明.txt" $output -Force
    $hashes = Get-ChildItem $output -File -Recurse | Where-Object Name -ne 'SHA256SUMS.txt' | ForEach-Object {
        $relative = [System.IO.Path]::GetRelativePath($output,$_.FullName)
        "$( (Get-FileHash $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant() )  $relative"
    }
    Set-Content "$output/SHA256SUMS.txt" $hashes -Encoding utf8NoBOM
    Compress-Archive -Path "$output/*" -DestinationPath "$root/dist/Shanjie-Windows.zip" -Force
} finally { Pop-Location }
