@echo off
setlocal
"%~dp0tools\shanjie-manager.exe" --elevate install --choose-keyboard
if errorlevel 1 echo Keyboard selection did not complete. See the error above.
pause
