#Requires -Version 7
param([ValidateSet('x64','x86')][string]$Architecture = 'x64')
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
Push-Location $root
try {
    & "$PSScriptRoot/prepare-core.ps1"
    $target = if ($Architecture -eq 'x64') { 'x86_64-pc-windows-msvc' } else { 'i686-pc-windows-msvc' }
    rustup target add $target
    if ($LASTEXITCODE) { throw 'Rust target installation failed.' }
    cargo build --release --manifest-path build/core/Cargo.toml --target $target
    if ($LASTEXITCODE) { throw 'Core build failed.' }
    $vswhere = "${env:ProgramFiles(x86)}/Microsoft Visual Studio/Installer/vswhere.exe"
    $vs = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if (!$vs) { throw 'Install Visual Studio Build Tools with C++ desktop tools and Windows SDK.' }
    $vcvars = Join-Path $vs 'VC/Auxiliary/Build/vcvarsall.bat'
    $output = Join-Path $root "dist/$Architecture"
    $objects = Join-Path $root "build/native-$Architecture"
    New-Item -ItemType Directory -Path $output,$objects -Force | Out-Null
    $sources = @('engine','key_policy','preferences','profile_watcher','mode_button','mode_indicator','candidate_host','candidate_palette','candidates','special_input','associated_candidates','pinyin_input','service','dll') | ForEach-Object { '"' + (Join-Path $root "src/$_.cpp") + '"' }
    $library = Join-Path $root "build/core/target/$target/release/core.lib"
    $command = @"
@echo off
call "$vcvars" $Architecture >nul
if errorlevel 1 exit /b 1
cd /d "$objects"
rc /nologo /I"$root/assets/mode" /fo mode_icons.res "$root/src/mode_icons.rc"
if errorlevel 1 exit /b 1
cl /nologo /std:c++20 /EHsc /O2 /MT /W4 /utf-8 /DUNICODE /D_UNICODE /DNOMINMAX /D_WIN32_WINNT=0x0A00 /LD /I"$root/upstream/shanjie/core/include" $($sources -join ' ') "$library" mode_icons.res /link /OUT:"$output/Shanjie.dll" ole32.lib oleaut32.lib uuid.lib user32.lib gdi32.lib d2d1.lib dwrite.lib dwmapi.lib windowsapp.lib shell32.lib advapi32.lib bcrypt.lib ntdll.lib ws2_32.lib userenv.lib /DEF:"$root/src/exports.def"
"@
    $cmdFile = Join-Path $objects 'compile.cmd'
    Set-Content $cmdFile $command -Encoding utf8NoBOM
    & cmd.exe /d /c $cmdFile
    if ($LASTEXITCODE) { throw 'TSF DLL build failed.' }
    if ($Architecture -eq 'x64') {
        $uiOutput = [IO.Path]::GetFullPath((Join-Path $root 'dist/ui'))
        if ($uiOutput -ne [IO.Path]::Combine([IO.Path]::GetFullPath($root),'dist','ui')) { throw 'Unexpected UI output path.' }
        if (Test-Path -LiteralPath $uiOutput) { Remove-Item -LiteralPath $uiOutput -Recurse -Force }
        dotnet publish "$root/ui/Shanjie.UI.csproj" -c Release -p:Platform=x64 -o "$root/dist/ui" --nologo
        if ($LASTEXITCODE) { throw 'WinUI host build failed.' }
    }
    New-Item -ItemType Directory -Path "$root/dist/data","$root/dist/LICENSES" -Force | Out-Null
    Copy-Item "$root/upstream/shanjie/data/lexicon/*" "$root/dist/data" -Force
    Copy-Item "$root/data/pinyin-map.tsv" "$root/dist/data" -Force
    Copy-Item "$root/upstream/shanjie/LICENSE" "$root/dist/LICENSES" -Force
    Copy-Item "$root/upstream/shanjie/LICENSES/*" "$root/dist/LICENSES" -Force
    Copy-Item "$root/data/README.md" "$root/dist/LICENSES/pinyin-data.md" -Force
} finally { Pop-Location }
