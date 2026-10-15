@echo off
setlocal
(
    "%~dp0tools\shanjie-manager.exe" --elevate install
    if errorlevel 1 echo Installer exit was not confirmed. It may still be running; see the message above.
    pause
)
