@echo off
setlocal
set "SHANJIE_MANAGER=tools\shanjie-manager.exe"
if exist "%~dp0manager-path.txt" set /p SHANJIE_MANAGER=<"%~dp0manager-path.txt"
rem Parse the whole block before removal deletes this launcher.
(
    "%~dp0%SHANJIE_MANAGER%" --elevate uninstall %*
    if errorlevel 1 echo Uninstallation did not complete. See the error above.
    pause
)
