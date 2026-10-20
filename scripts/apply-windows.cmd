@echo off
setlocal
"%~dp0..\tools\shanjie-manager.exe" apply-profile "%~dp0..\profiles\windows.ini"
if errorlevel 1 echo Profile application failed.
pause
