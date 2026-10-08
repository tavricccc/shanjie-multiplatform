@echo off
setlocal
"%~dp0tools\shanjie-manager.exe" apply-profile "%~dp0profiles\shanjie.ini"
if errorlevel 1 echo Profile application failed.
pause
