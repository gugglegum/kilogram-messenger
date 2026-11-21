@echo off
setlocal
chcp 65001 >nul
cd /d "%~dp0"
echo Это ограниченный сетевой тест на отдельном аккаунте, не обычная переписка.
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0test-usable-peer-receiver.ps1"
set "probe_exit=%errorlevel%"
pause
exit /b %probe_exit%
