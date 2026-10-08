@echo off
setlocal
(
    "%~dp0tools\shanjie-manager.exe" --elevate install
    if errorlevel 1 echo Installation did not complete. See the error above.
    pause
)
