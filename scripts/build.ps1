#Requires -Version 7
param([ValidateSet('all','x64','x86')][string]$Architecture = 'all', [switch]$UseExistingCore, [switch]$SkipSettings, [switch]$UseCurrentToolchain, [switch]$CargoVerbose)
$ErrorActionPreference = 'Stop'
# Default builds both: x86 is the DLL that 32-bit apps on 64-bit Windows load.
if ($Architecture -eq 'all') {
    & $PSCommandPath -Architecture x64 -UseExistingCore:$UseExistingCore -SkipSettings:$SkipSettings -UseCurrentToolchain:$UseCurrentToolchain -CargoVerbose:$CargoVerbose
    & $PSCommandPath -Architecture x86 -UseExistingCore -UseCurrentToolchain:$UseCurrentToolchain -CargoVerbose:$CargoVerbose
    return
}
$root = Split-Path $PSScriptRoot -Parent
Import-Module "$PSScriptRoot/test-support.psm1" -ErrorAction Stop
. "$PSScriptRoot/preserve-output.ps1"
$null = Initialize-TestTimings -ProjectRoot $root -Name 'build'
Push-Location $root
try {
    $target = if ($Architecture -eq 'x64') { 'x86_64-pc-windows-msvc' } else { 'i686-pc-windows-msvc' }
    # The data copied below comes straight from the submodule working tree, so it must be the pinned commit
    # even when the core is reused (a moved submodule would otherwise ship data the pinned core does not read).
    $pin = (Get-Content "$root/UPSTREAM.json" -Raw | ConvertFrom-Json).commit
    if ((git -C "$root/upstream/shanjie" rev-parse HEAD) -ne $pin) { throw 'upstream/shanjie is not at the commit in UPSTREAM.json.' }
    if (!$UseExistingCore) { Invoke-Timed -Name 'Prepare Windows core' -Category 'core.prepare' -Details @{ architecture=$Architecture } -Action { & "$PSScriptRoot/prepare-core.ps1" } }
    $installedTargets = @(rustup target list --installed)
    if ($LASTEXITCODE) { throw 'Unable to read installed Rust targets.' }
    if ($target -notin $installedTargets) {
        Invoke-Timed -Name 'Install Rust target' -Category 'toolchain.prepare' -Details @{ architecture=$Architecture; target=$target } -Action {
            rustup target add $target
            if ($LASTEXITCODE) { throw 'Rust target installation failed.' }
        }
    }
    $vswhere = "${env:ProgramFiles(x86)}/Microsoft Visual Studio/Installer/vswhere.exe"
    if(!$UseCurrentToolchain){
        $vs = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
        if (!$vs) { throw 'Install Visual Studio Build Tools with the Windows SDK resource compiler.' }
    }elseif((@(Get-Command cl.exe,rc.exe -ErrorAction SilentlyContinue)).Count -ne 2){
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
rc /nologo /I"$root/assets/mode" /fo "$objects/mode_icons.res.tmp" "$root/windows/resources.rc"
if errorlevel 1 exit /b 1
rc /nologo /I"$root/ui/Assets" /fo "$objects/setup.res.tmp" "$root/windows/setup.rc"
"@
    Set-Content "$objects/resources.cmd" $command -Encoding utf8NoBOM
    Invoke-Timed -Name 'Compile Windows resources' -Category 'build.native' -Details @{ architecture=$Architecture; resource_count=2 } -Action {
        & cmd.exe /d /c "$objects/resources.cmd"
        if ($LASTEXITCODE) { throw 'Mode resource build failed.' }
        foreach($name in 'mode_icons','setup') {
            $resource=Join-Path $objects "$name.res"
            $temporary="$resource.tmp"
            if((Test-Path -LiteralPath $resource) -and (Get-FileHash -LiteralPath $resource).Hash -eq (Get-FileHash -LiteralPath $temporary).Hash){
                Remove-Item -LiteralPath $temporary -Force
            }else{Move-Item -LiteralPath $temporary -Destination $resource -Force}
        }
    }
    $env:SHANJIE_ICON_RESOURCE = Join-Path $objects 'mode_icons.res'
    $env:SHANJIE_SETUP_RESOURCE = Join-Path $objects 'setup.res'
    Invoke-Timed -Name 'Rust Windows release build' -Category 'build.rust.release' -Details @{ architecture=$Architecture; target=$target; incremental='Cargo fingerprints' } -Action {
        $cargoArguments=@('build','--release','--manifest-path','windows/Cargo.toml','--target',$target)
        if($CargoVerbose){$cargoArguments+='-vv'}
        & cargo @cargoArguments
        if ($LASTEXITCODE) { throw 'Rust Windows build failed.' }
    }
    Copy-Item "$root/windows/target/$target/release/shanjie_windows.dll" "$output/Shanjie.dll" -Force
    if ($Architecture -eq 'x64' -and !$SkipSettings) {
        $uiOutput = [IO.Path]::GetFullPath((Join-Path $root 'dist/ui'))
        if ($uiOutput -ne [IO.Path]::Combine([IO.Path]::GetFullPath($root),'dist','ui')) { throw 'Unexpected UI output path.' }
        $uiStage=Start-TestStage -Name 'Validate or publish WinUI output' -Category 'build.winui.publish'
        $uiStageStatus='passed';$uiFailure=$null
        try {
            $dotnetVersion = (& dotnet --version 2>&1 | Select-Object -First 1).ToString().Trim()
            $uiSourcePaths = @(Get-ChildItem -LiteralPath "$root/ui" -File -Recurse | Where-Object {
                $_.FullName -notmatch '[\\/](bin|obj)[\\/]'
            } | ForEach-Object { [IO.Path]::GetRelativePath($root,$_.FullName) }) + @('profiles/windows.ini','profiles/shanjie.ini','MODEL-CATALOG.json','MODEL-SOURCES.json','NuGet.Config','scripts/build.ps1')
            $uiKey = Get-ContentFingerprint -Root $root -Paths $uiSourcePaths -Values @($dotnetVersion,'Release','x64','dotnet publish')
            $uiCacheManifest = Join-Path $root 'build/ui-publish-cache.json'
            $existingUiPaths = if (Test-Path -LiteralPath $uiOutput) {
                @(Get-ChildItem -LiteralPath $uiOutput -File -Recurse | Where-Object Name -ne 'shanjie_windows.dll' | ForEach-Object { [IO.Path]::GetRelativePath($root,$_.FullName) })
            } else { @() }
            $uiPublishMiss = !(Test-ContentCache -Root $root -ManifestPath $uiCacheManifest -Key $uiKey -Paths $existingUiPaths)
            if ($uiPublishMiss) {
                Preserve-Output $uiOutput
                dotnet publish "$root/ui/Shanjie.UI.csproj" -c Release -p:Platform=x64 -o $uiOutput --nologo
                if ($LASTEXITCODE) { throw 'WinUI settings build failed.' }
            } else { Write-Host '.NET publish cache hit: source, SDK, and published file hashes verified.' }
            Complete-TestStage -Stage $uiStage -Details @{ architecture='x64'; configuration='Release'; cache=if($uiPublishMiss){'miss'}else{'hit'};output_files=$existingUiPaths.Count }
        } catch {
            $uiStageStatus='failed';$uiFailure=$_.Exception.Message
            Complete-TestStage -Stage $uiStage -Status 'failed' -Details @{ architecture='x64'; configuration='Release'; cache='miss' } -ErrorMessage $uiFailure
            throw
        }
    }
    if ($Architecture -eq 'x64') {
        if (Test-Path "$root/dist/ui") { Copy-Item "$root/windows/target/$target/release/shanjie_windows.dll" "$root/dist/ui" -Force }
        if (!$SkipSettings -and $uiPublishMiss) {
            $publishedPaths = @(Get-ChildItem -LiteralPath "$root/dist/ui" -File -Recurse | Where-Object Name -ne 'shanjie_windows.dll' | ForEach-Object { [IO.Path]::GetRelativePath($root,$_.FullName) })
            Save-ContentCache -Root $root -ManifestPath $uiCacheManifest -Key $uiKey -Paths $publishedPaths
        }
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
    # The data is the same for both architectures; the x64 pass (first in an 'all' build) prepares it.
    if ($Architecture -eq 'x64' -or !(Test-Path "$root/dist/data/models/builtin/bigram.winmap")) {
        Invoke-Timed -Name 'Prepare built-in models' -Category 'models.prepare' -Details @{ architecture=$Architecture } -Action { & "$PSScriptRoot/prepare-models.ps1" }
        Invoke-Timed -Name 'Prepare runtime data and licenses' -Category 'data.prepare' -Details @{ architecture=$Architecture } -Action { & "$PSScriptRoot/prepare-data.ps1" -Licenses 'dist/LICENSES' }
    }
} finally { Remove-Item Env:SHANJIE_ICON_RESOURCE,Env:SHANJIE_SETUP_RESOURCE -ErrorAction SilentlyContinue; Pop-Location }
