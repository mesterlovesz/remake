@echo off
setlocal
cd /d "%~dp0"
chcp 65001 >nul
title A Mesterlovesz: Ujratoltve
if not exist ".player\python\python.exe" (
  echo A teljes jatekoscsomagot csomagold ki, ne csak ezt az inditot.
  pause
  exit /b 1
)
".player\python\python.exe" -X utf8 -m tools.player_launcher %*
set "RESULT=%ERRORLEVEL%"
if not "%RESULT%"=="0" pause
exit /b %RESULT%
