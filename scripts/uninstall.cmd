@echo off
setlocal
set "SHANJIE_MANAGER=tools\shanjie-manager.exe"
if exist "%~dp0manager-path.txt" set /p SHANJIE_MANAGER=<"%~dp0manager-path.txt"
rem Parse the whole block before removal deletes this launcher.
(
    "%~dp0%SHANJIE_MANAGER%" --elevate uninstall %*
    if errorlevel 1 echo Uninstaller exit was not confirmed. It may still be running; see the message above.
    pause
)
