@echo off
setlocal
(
    "%ProgramFiles%\PowerShell\7\pwsh.exe" -NoProfile -ExecutionPolicy Bypass -File "%~dp0elevate.ps1" -Action uninstall
    if errorlevel 1 echo Uninstallation failed. See the error above.
    pause
)
