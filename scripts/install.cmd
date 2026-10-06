@echo off
setlocal
"%ProgramFiles%\PowerShell\7\pwsh.exe" -NoProfile -ExecutionPolicy Bypass -File "%~dp0elevate.ps1" -Action install
if errorlevel 1 echo Installation failed. See the error above.
pause
