@echo off
setlocal
chcp 65001 >nul
cd /d "%~dp0"
if not exist "%~dp0kilogram-peer.exe" (
  echo В этой папке нет kilogram-peer.exe. Распакуйте весь комплект Kilogram.
  pause
  exit /b 1
)
if "%~1"=="" (
  "%~dp0kilogram-peer.exe" --profile "%~dp0profile"
) else (
  "%~dp0kilogram-peer.exe" --profile "%~1"
)
set "peer_exit=%errorlevel%"
if not "%peer_exit%"=="0" (
  echo Kilogram завершился с кодом %peer_exit%. Профиль сохранён; не удаляйте папку profile.
  pause
)
exit /b %peer_exit%
