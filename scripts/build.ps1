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
    $sources = @('engine','key_policy','preferences','candidates','service','dll') | ForEach-Object { '"' + (Join-Path $root "src/$_.cpp") + '"' }
    $library = Join-Path $root "build/core/target/$target/release/core.lib"
    $command = @"
@echo off
call "$vcvars" $Architecture >nul
if errorlevel 1 exit /b 1
cd /d "$objects"
cl /nologo /std:c++20 /EHsc /O2 /MT /W4 /utf-8 /DUNICODE /D_UNICODE /DNOMINMAX /D_WIN32_WINNT=0x0A00 /LD /I"$root/upstream/shanjie/core/include" $($sources -join ' ') "$library" /link /OUT:"$output/Shanjie.dll" ole32.lib oleaut32.lib uuid.lib user32.lib gdi32.lib advapi32.lib bcrypt.lib ntdll.lib ws2_32.lib userenv.lib /DEF:"$root/src/exports.def"
"@
    $cmdFile = Join-Path $objects 'compile.cmd'
    Set-Content $cmdFile $command -Encoding utf8NoBOM
    & cmd.exe /d /c $cmdFile
    if ($LASTEXITCODE) { throw 'TSF DLL build failed.' }
    New-Item -ItemType Directory -Path "$root/dist/data","$root/dist/LICENSES" -Force | Out-Null
    Copy-Item "$root/upstream/shanjie/data/lexicon/*" "$root/dist/data" -Force
    Copy-Item "$root/upstream/shanjie/LICENSE" "$root/dist/LICENSES" -Force
    Copy-Item "$root/upstream/shanjie/LICENSES/*" "$root/dist/LICENSES" -Force
} finally { Pop-Location }
