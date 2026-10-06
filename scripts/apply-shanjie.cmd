@echo off
setlocal
call "%~dp0bootstrap.cmd" apply "%~dp0profiles\shanjie.ini"
if errorlevel 1 echo Profile application failed.
pause
