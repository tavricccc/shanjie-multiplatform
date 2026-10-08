@echo off
setlocal
"%~dp0tools\shanjie-manager.exe" apply-profile "%~dp0profiles\windows.ini"
if errorlevel 1 echo Profile application failed.
pause
