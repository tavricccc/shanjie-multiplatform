#Requires -Version 7
param([ValidateSet('x64','x86')][string[]]$Architectures=@('x64','x86'),[ValidateSet(0,1,2)][int[]]$Themes=@(0),[string]$ResultRoot,[string]$ModelId='builtin',[string]$ModelFixture,[string]$HarnessPath,[string]$PayloadRoot)
$ErrorActionPreference='Stop'
Import-Module "$PSScriptRoot/test-support.psm1" -ErrorAction Stop
. "$PSScriptRoot/msvc-command.ps1"
$root=Split-Path $PSScriptRoot -Parent
$null=Initialize-TestTimings -ProjectRoot $root -Name 'add-word'
$result=if($ResultRoot){[IO.Path]::GetFullPath($ResultRoot)}else{Join-Path $root ('build/word-popup-'+[guid]::NewGuid().ToString('N'))}
$runtimeRoot=if($PayloadRoot){[IO.Path]::GetFullPath($PayloadRoot)}else{[IO.Path]::GetFullPath((Join-Path $root 'dist'))}
foreach($arch in $Architectures){if(!(Test-Path -LiteralPath "$runtimeRoot/$arch/Shanjie.dll")){throw "Runtime payload is missing $arch/Shanjie.dll."}}
if(!(Test-Path -LiteralPath "$runtimeRoot/tools/shanjie-core.exe")){throw 'Runtime payload is missing tools/shanjie-core.exe.'}
foreach($arch in $Architectures) {foreach($theme in $Themes){
    $out=Join-Path $result "$arch-theme$theme"
    $worker=$null
    if(Test-Path -LiteralPath $out){throw 'Use a fresh result directory to isolate popup dictionaries.'}
    New-Item -ItemType Directory "$out/profile/Shanjie" -Force | Out-Null
    $text=(Get-Content "$root/profiles/windows.ini" -Raw) -replace '(?m)^learning=.*','learning=0' -replace '(?m)^theme=.*',("theme="+$theme)
    if($ModelId -ne 'builtin'){
        $receipt=Get-Content -LiteralPath "$ModelFixture/model.json" -Raw|ConvertFrom-Json
        if($receipt.id -ne $ModelId -or $ModelId -notmatch '^[a-z0-9-]{1,80}$'){throw 'A matching validated public model fixture is required.'}
        New-Item -ItemType Directory "$out/profile/Shanjie/Models" -Force|Out-Null
        Copy-Item -LiteralPath $ModelFixture -Destination "$out/profile/Shanjie/Models/$ModelId" -Recurse
        $text=$text -replace '(?m)^model_id=.*',("model_id="+$ModelId)
    }
    Set-Content "$out/profile/Shanjie/profile.ini" $text -Encoding utf8NoBOM
    $exe=if($HarnessPath){[IO.Path]::GetFullPath($HarnessPath)}else{Join-Path $out 'features.exe'}
    if($HarnessPath -and $Architectures.Count -ne 1){throw 'A prebuilt add-word harness can only be used for one architecture per invocation.'}
    if($HarnessPath -and !(Test-Path -LiteralPath $exe)){throw "Add-word harness is missing: $exe"}
    if(!$HarnessPath){
    $command=@"
@echo off
$(Get-MsvcSetup $arch)
if errorlevel 1 exit /b 1
cl /nologo /std:c++20 /EHsc /O2 /MT /W4 /utf-8 /DUNICODE /D_UNICODE /DNOMINMAX /I"$root/tests/support" "$root/tests/add_word_popup.cpp" /Fo"$out/features.obj" /Fe"$exe" /link ole32.lib oleaut32.lib uuid.lib user32.lib gdi32.lib advapi32.lib
"@
    Set-Content "$out/compile.cmd" $command -Encoding utf8NoBOM
    Invoke-Timed -Name "Compile add-word harness ($arch)" -Category 'build.cpp.harness' -Details @{architecture=$arch;source='tests/add_word_popup.cpp'} -Action { & cmd.exe /d /c "$out/compile.cmd" *> "$out/compile.log";if($LASTEXITCODE){throw "Add-word TSF harness compile failed: $arch"} }
    }
    $previous=$env:LOCALAPPDATA; $previousTest=$env:SHANJIE_TEST_PROFILE; $previousHeadless=$env:SHANJIE_HEADLESS;$previousTheme=$env:SHANJIE_TEST_POPUP_THEME;$previousModel=$env:SHANJIE_TEST_POPUP_MODEL
    try {
        $env:SHANJIE_TEST_POPUP_THEME="$theme"
        $env:SHANJIE_TEST_POPUP_MODEL=$ModelId
        $env:LOCALAPPDATA=[IO.Path]::Combine($out,'profile'); $env:SHANJIE_TEST_PROFILE='1'; $env:SHANJIE_HEADLESS='1'
        # Elevated test hosts deliberately cannot auto-start a user core. Start
        # the isolated fixture explicitly; its endpoint includes this profile.
        $startStage=Start-TestStage -Name "Start add-word core ($arch)" -Category 'core.lifecycle'
        try{$worker=Start-Process -FilePath "$runtimeRoot/tools/shanjie-core.exe" -ArgumentList '--serve' -WindowStyle Hidden -RedirectStandardError "$out/core-error.log" -RedirectStandardOutput "$out/core-output.log" -PassThru;Complete-TestStage -Stage $startStage -Details @{architecture=$arch;profile="$out/profile";pid=$worker.Id}}
        catch{Complete-TestStage -Stage $startStage -Status 'failed' -Details @{architecture=$arch;profile="$out/profile"} -ErrorMessage $_.Exception.Message;throw}
        # A pipe exists before resource preload finishes. Wait for a real reply;
        # a fixed sleep races the TSF preload connection on slower cold starts.
        Wait-TestCoreReady -CorePath "$runtimeRoot/tools/shanjie-core.exe" -OutputDirectory $out | Out-Null
        Invoke-Timed -Name "Add-word TSF regression ($arch)" -Category 'word.add' -Details @{architecture=$arch;theme=$theme;model=$ModelId} -Action { & $exe "$runtimeRoot/$arch/Shanjie.dll" $out *> "$out/result.log";if($LASTEXITCODE){throw "Add-word TSF regression failed: $arch, exit $LASTEXITCODE"} }
    } finally {
        $stopStage=Start-TestStage -Name "Stop add-word core ($arch)" -Category 'core.lifecycle'
        $stopFailure=$null
        $stopCompleted=$false
        try {
            $stop=Start-Process -FilePath "$runtimeRoot/tools/shanjie-core.exe" -ArgumentList '--stop' -WindowStyle Hidden -PassThru
            $stopExited=$stop.WaitForExit(5000)
            $stopCode=if($stopExited){$stop.ExitCode}else{$null}
            if(!$stopExited){try{$stop.Kill();$stop.WaitForExit(5000)|Out-Null}catch{}}
            $workerExited=if($worker){$worker.WaitForExit(5000)}else{$true}
            if(!$workerExited){try{$worker.Kill();$worker.WaitForExit(5000)|Out-Null}catch{}}
            if(!$stopExited -or $stopCode -ne 0 -or !$workerExited){$stopFailure='An isolated add-word core did not stop cleanly within five seconds.';Complete-TestStage -Stage $stopStage -Status 'failed' -Details @{architecture=$arch;pid=$worker.Id;stop_exit_code=$stopCode;worker_force_stopped=(!$workerExited)} -ErrorMessage $stopFailure;$stopCompleted=$true}
            else { Complete-TestStage -Stage $stopStage -Details @{architecture=$arch;pid=$worker.Id;stop_exit_code=$stopCode};$stopCompleted=$true }
        } catch {
            $stopFailure=$_.Exception.Message
            $workerForceStopped=$false
            if($worker -and !$worker.HasExited){try{$worker.Kill();$worker.WaitForExit(5000)|Out-Null;$workerForceStopped=$true}catch{}}
            if(!$stopCompleted){Complete-TestStage -Stage $stopStage -Status 'failed' -Details @{architecture=$arch;pid=if($worker){$worker.Id}else{$null};worker_force_stopped=$workerForceStopped} -ErrorMessage $stopFailure;$stopCompleted=$true}
        } finally {
            $env:LOCALAPPDATA=$previous; $env:SHANJIE_TEST_PROFILE=$previousTest; $env:SHANJIE_HEADLESS=$previousHeadless;$env:SHANJIE_TEST_POPUP_THEME=$previousTheme;$env:SHANJIE_TEST_POPUP_MODEL=$previousModel
        }
        if($stopFailure){throw $stopFailure}
    }
}}
Write-Host 'PASS offscreen TSF Shift/add-word, theme, duplicate inspection and explicit override regression'
