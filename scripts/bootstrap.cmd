@echo off
setlocal
if not defined ProgramW6432 (
    echo Shanjie requires x64 Windows.
    exit /b 1
)
call :findRuntime
if defined PWSH goto run
where /q winget.exe
if errorlevel 1 (
    echo Windows App Installer is required to install PowerShell 7 automatically.
    echo Install App Installer from Microsoft Store and run this script again.
    exit /b 1
)
echo Installing Microsoft PowerShell 7 x64...
winget.exe install --id Microsoft.PowerShell --exact --source winget --architecture x64 --silent --accept-package-agreements --accept-source-agreements --disable-interactivity
if errorlevel 1 exit /b 1
call :findRuntime
if not defined PWSH (
    echo PowerShell installation completed but its executable was not found.
    exit /b 1
)
:run
rem Parse this block before uninstall deletes its own launcher files.
(
    start "" /b /wait "%PWSH%" -NoProfile -ExecutionPolicy Bypass -File "%~dp0elevate.ps1" -Action "%~1"
    if errorlevel 1 (exit /b 1) else (exit /b 0)
)
:findRuntime
set "PWSH="
if exist "%ProgramW6432%\PowerShell\7\pwsh.exe" set "PWSH=%ProgramW6432%\PowerShell\7\pwsh.exe"
if defined PWSH exit /b 0
rem Windows PowerShell only locates the Store package; all installation work
rem runs in PowerShell 7. App execution aliases do not relay exit codes in CMD.
for /f "usebackq delims=" %%P in (`powershell.exe -NoProfile -Command "(Get-AppxPackage Microsoft.PowerShell).InstallLocation"`) do if exist "%%P\pwsh.exe" set "PWSH=%%P\pwsh.exe"
exit /b 0
