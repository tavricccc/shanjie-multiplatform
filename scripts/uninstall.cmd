@echo off
setlocal
(
    call "%~dp0bootstrap.cmd" uninstall
    if errorlevel 1 echo Uninstallation failed. See the error above.
    pause
)
