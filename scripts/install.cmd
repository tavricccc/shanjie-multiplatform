@echo off
setlocal
(
    call "%~dp0bootstrap.cmd" install
    if errorlevel 1 echo Installation failed. See the error above.
    pause
)
