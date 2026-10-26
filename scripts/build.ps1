#Requires -Version 7
param([ValidateSet('all','x64','x86')][string]$Architecture = 'all', [switch]$UseExistingCore, [switch]$SkipSettings, [switch]$UseCurrentToolchain)
$ErrorActionPreference = 'Stop'
# Default builds both: x86 is the DLL that 32-bit apps on 64-bit Windows load.
if ($Architecture -eq 'all') {
    & $PSCommandPath -Architecture x64 -UseExistingCore:$UseExistingCore -SkipSettings:$SkipSettings -UseCurrentToolchain:$UseCurrentToolchain
    & $PSCommandPath -Architecture x86 -UseExistingCore -UseCurrentToolchain:$UseCurrentToolchain
    return
}
$root = Split-Path $PSScriptRoot -Parent
. "$PSScriptRoot/preserve-output.ps1"
Push-Location $root
try {
    $target = if ($Architecture -eq 'x64') { 'x86_64-pc-windows-msvc' } else { 'i686-pc-windows-msvc' }
    # The data copied below comes straight from the submodule working tree, so it must be the pinned commit
    # even when the core is reused (a moved submodule would otherwise ship data the pinned core does not read).
    $pin = (Get-Content "$root/UPSTREAM.json" -Raw | ConvertFrom-Json).commit
    if ((git -C "$root/upstream/shanjie" rev-parse HEAD) -ne $pin) { throw 'upstream/shanjie is not at the commit in UPSTREAM.json.' }
    if (!$UseExistingCore) { & "$PSScriptRoot/prepare-core.ps1" }
    rustup target add $target
    if ($LASTEXITCODE) { throw 'Rust target installation failed.' }
    $vswhere = "${env:ProgramFiles(x86)}/Microsoft Visual Studio/Installer/vswhere.exe"
    if(!$UseCurrentToolchain){
        $vs = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
        if (!$vs) { throw 'Install Visual Studio Build Tools with the Windows SDK resource compiler.' }
    }elseif(!(Get-Command cl.exe,rc.exe -ErrorAction SilentlyContinue | Measure-Object).Count -eq 2){
        throw 'UseCurrentToolchain requires MSVC cl.exe and Windows SDK rc.exe on PATH.'
    }
    $objects = Join-Path $root "build/native-$Architecture"
    $output = Join-Path $root "dist/$Architecture"
    New-Item -ItemType Directory -Path $objects,$output -Force | Out-Null
    $toolchainSetup = if($UseCurrentToolchain){''}else{'call "'+(Join-Path $vs 'VC/Auxiliary/Build/vcvarsall.bat')+'" '+$Architecture+' >nul'}
    $command = @"
@echo off
$toolchainSetup
if errorlevel 1 exit /b 1
rc /nologo /I"$root/assets/mode" /fo "$objects/mode_icons.res" "$root/windows/resources.rc"
if errorlevel 1 exit /b 1
rc /nologo /I"$root/ui/Assets" /fo "$objects/setup.res" "$root/windows/setup.rc"
"@
    Set-Content "$objects/resources.cmd" $command -Encoding utf8NoBOM
    & cmd.exe /d /c "$objects/resources.cmd"
    if ($LASTEXITCODE) { throw 'Mode resource build failed.' }
    $env:SHANJIE_ICON_RESOURCE = Join-Path $objects 'mode_icons.res'
    $env:SHANJIE_SETUP_RESOURCE = Join-Path $objects 'setup.res'
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
        Copy-Item "$root/windows/target/$target/release/shanjie-word-helper.exe" "$root/dist/tools" -Force
        # Package-root launcher for the WinUI setup window.
        New-Item -ItemType Directory "$root/dist/setup" -Force | Out-Null
        Copy-Item "$root/windows/target/$target/release/shanjie-setup.exe" "$root/dist/setup/安裝善解.exe" -Force
        # Build-time only: appends the package to the setup stub for the single-file installer.
        Copy-Item "$root/windows/target/$target/release/shanjie-pack.exe" "$root/dist/setup" -Force
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
} finally { Remove-Item Env:SHANJIE_ICON_RESOURCE,Env:SHANJIE_SETUP_RESOURCE -ErrorAction SilentlyContinue; Pop-Location }
