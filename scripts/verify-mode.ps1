#Requires -Version 7
param(
    [ValidateSet('x64','x86')][string[]]$Architectures=@('x64','x86'),
    [string]$HarnessPath,
    [string]$CaptureRoot
)
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
Import-Module "$PSScriptRoot/test-support.psm1" -ErrorAction Stop
. "$PSScriptRoot/msvc-command.ps1"
$null=Initialize-TestTimings -ProjectRoot $root -Name 'verify-mode'
if($HarnessPath -and $Architectures.Count -ne 1){throw 'A CMake-built mode harness can only be used for one architecture per invocation.'}
$vs=$null;$vcvars=$null
if(!$HarnessPath){
    $vs=& "${env:ProgramFiles(x86)}/Microsoft Visual Studio/Installer/vswhere.exe" -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    $vcvars=Join-Path $vs 'VC/Auxiliary/Build/vcvarsall.bat'
}
$captureBase=if($CaptureRoot){[IO.Path]::GetFullPath($CaptureRoot)}else{Join-Path $root 'build/mode-review'}
foreach($architecture in $Architectures){
    $capture=if($Architectures.Count -gt 1){Join-Path $captureBase $architecture}else{$captureBase}
    New-Item -ItemType Directory -Path $capture -Force|Out-Null
    if($HarnessPath){
        $exe=[IO.Path]::GetFullPath($HarnessPath)
        if(!(Test-Path -LiteralPath $exe)){throw "Mode harness is missing: $exe"}
    }else{
        $exe="$root/build/mode-$architecture.exe"
        $target=if($architecture -eq 'x64'){'x86_64'}else{'i686'}
        $command=@"
@echo off
call "$vcvars" $architecture >nul
if errorlevel 1 exit /b 1
cd /d "$root/build/native-$architecture"
cl /nologo /std:c++20 /EHsc /O2 /MT /W4 /utf-8 /DUNICODE /D_UNICODE /DNOMINMAX /I"$root/tests/support" "$root/tests/mode_button_smoke.cpp" "$root/tests/support/mode_indicator.cpp" "$root/windows/target/$target-pc-windows-msvc/release/shanjie_windows.lib" /link /OUT:"$exe" ole32.lib oleaut32.lib uuid.lib user32.lib gdi32.lib d2d1.lib dwrite.lib dwmapi.lib windowsapp.lib advapi32.lib bcrypt.lib ntdll.lib ws2_32.lib userenv.lib
"@
        $script="$root/build/native-$architecture/mode-smoke.cmd"
        Set-Content -LiteralPath $script -Value $command -Encoding utf8NoBOM
        Invoke-Timed -Name "Compile mode harness ($architecture)" -Category 'build.cpp.harness' -Details @{architecture=$architecture;source='tests/mode_button_smoke.cpp'} -Action {
            & cmd.exe /d /c $script
            if($LASTEXITCODE){throw 'Mode harness build failed.'}
        }
    }
    Invoke-Timed -Name "Mode surface smoke ($architecture)" -Category 'ui.mode-smoke' -Details @{architecture=$architecture;capture_root=$capture} -Action {
        & $exe "$root/dist/$architecture/Shanjie.dll" $capture
        if($LASTEXITCODE){throw "$architecture mode surface failed."}
    }
}
