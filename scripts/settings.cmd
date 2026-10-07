@echo off
if exist "%~dp0ui-path.txt" goto installed
start "" "%~dp0ui\Shanjie.UI.exe"
exit /b
:installed
set /p SHANJIE_UI=<"%~dp0ui-path.txt"
start "" "%~dp0%SHANJIE_UI%"
