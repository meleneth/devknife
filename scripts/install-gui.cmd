@echo off
setlocal

powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0install-gui.ps1" %*
exit /b %ERRORLEVEL%
