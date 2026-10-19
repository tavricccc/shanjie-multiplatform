#Requires -Version 7
param([ValidateSet('all','x64','x86')][string]$Architecture = 'all', [switch]$UseExistingCore, [switch]$SkipSettings)
$ErrorActionPreference = 'Stop'
# Default builds both: x86 is the DLL that 32-bit apps on 64-bit Windows load.
if ($Architecture -eq 'all') {
    & $PSCommandPath -Architecture x64 -UseExistingCore:$UseExistingCore -SkipSettings:$SkipSettings
    & $PSCommandPath -Architecture x86 -UseExistingCore
    return
}
$root = Split-Path $PSScriptRoot -Parent
. "$PSScriptRoot/preserve-output.ps1"
Push-Location $root
try {
    $target = if ($Architecture -eq 'x64') { 'x86_64-pc-windows-msvc' } else { 'i686-pc-windows-msvc' }
    if (!$UseExistingCore) { & "$PSScriptRoot/prepare-core.ps1" }
    rustup target add $target
    if ($LASTEXITCODE) { throw 'Rust target installation failed.' }
    $vswhere = "${env:ProgramFiles(x86)}/Microsoft Visual Studio/Installer/vswhere.exe"
    $vs = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if (!$vs) { throw 'Install Visual Studio Build Tools with the Windows SDK resource compiler.' }
    $objects = Join-Path $root "build/native-$Architecture"
    $output = Join-Path $root "dist/$Architecture"
    New-Item -ItemType Directory -Path $objects,$output -Force | Out-Null
    $vcvars = Join-Path $vs 'VC/Auxiliary/Build/vcvarsall.bat'
    $command = @"
@echo off
call "$vcvars" $Architecture >nul
if errorlevel 1 exit /b 1
rc /nologo /I"$root/assets/mode" /fo "$objects/mode_icons.res" "$root/windows/resources.rc"
"@
    Set-Content "$objects/resources.cmd" $command -Encoding utf8NoBOM
    & cmd.exe /d /c "$objects/resources.cmd"
    if ($LASTEXITCODE) { throw 'Mode resource build failed.' }
    $env:SHANJIE_ICON_RESOURCE = Join-Path $objects 'mode_icons.res'
    cargo build --release --manifest-path windows/Cargo.toml --target $target
    if ($LASTEXITCODE) { throw 'Rust Windows build failed.' }
    Copy-Item "$root/windows/target/$target/release/shanjie_windows.dll" "$output/Shanjie.dll" -Force
    if ($Architecture -eq 'x64' -and !$SkipSettings) {
        $uiOutput = [IO.Path]::GetFullPath((Join-Path $root 'dist/ui'))
        if ($uiOutput -ne [IO.Path]::Combine([IO.Path]::GetFullPath($root),'dist','ui')) { throw 'Unexpected UI output path.' }
        Preserve-Output $uiOutput
        dotnet publish "$root/ui/Shanjie.UI.csproj" -c Release -p:Platform=x64 -o $uiOutput --nologo
        if ($LASTEXITCODE) { throw 'WinUI settings build failed.' }
    }
    if ($Architecture -eq 'x64') {
        if (Test-Path "$root/dist/ui") { Copy-Item "$root/windows/target/$target/release/shanjie_windows.dll" "$root/dist/ui" -Force }
        New-Item -ItemType Directory "$root/dist/tools" -Force | Out-Null
        Copy-Item "$root/windows/target/$target/release/shanjie-manager.exe" "$root/dist/tools" -Force
        Preserve-Output "$root/dist/tools/shanjie-core.exe"
        Copy-Item "$root/windows/target/$target/release/shanjie-core.exe" "$root/dist/tools" -Force
    }
    New-Item -ItemType Directory -Path "$root/dist/data","$root/dist/LICENSES" -Force | Out-Null
    Copy-Item "$root/upstream/shanjie/data/lexicon/*" "$root/dist/data" -Force
    Copy-Item "$root/data/pinyin-map.tsv" "$root/dist/data" -Force
    # The ACG word pack and its provenance files, the same three upstream ships.
    New-Item -ItemType Directory -Path "$root/dist/data/packs" -Force | Out-Null
    Copy-Item "$root/upstream/shanjie/data/packs/acg-add.tsv","$root/upstream/shanjie/data/packs/acg-sources.tsv","$root/upstream/shanjie/data/packs/acg.json" "$root/dist/data/packs" -Force
    # Generated and hash-verified against upstream's recorded output (LF only, as its strict reader requires).
    & "$PSScriptRoot/prepare-english.ps1" | Out-Null
    Copy-Item "$root/build/english-words.txt" "$root/dist/data" -Force
    Copy-Item "$root/upstream/shanjie/LICENSE","$root/upstream/shanjie/LICENSES/*","$root/LICENSES/windows-models.md" "$root/dist/LICENSES" -Force
    Copy-Item "$root/data/README.md" "$root/dist/LICENSES/pinyin-data.md" -Force
    Copy-Item "$root/LICENSES/mode-icons.md" "$root/dist/LICENSES" -Force
} finally { Remove-Item Env:SHANJIE_ICON_RESOURCE -ErrorAction SilentlyContinue; Pop-Location }
