@echo off
setlocal
set "PWSH=%ProgramW6432%\PowerShell\7\pwsh.exe"
if not exist "%PWSH%" set "PWSH=%LOCALAPPDATA%\Microsoft\WindowsApps\pwsh.exe"
if not exist "%PWSH%" (
    echo PowerShell 7 x64 is required. Install it from https://aka.ms/powershell-release
    pause
    exit /b 1
)
(
    "%PWSH%" -NoProfile -ExecutionPolicy Bypass -File "%~dp0elevate.ps1" -Action uninstall
    if errorlevel 1 echo Uninstallation failed. See the error above.
    pause
)
