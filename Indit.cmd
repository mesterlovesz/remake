@echo off
rem A Mesterlovesz: Ujratoltve - ezt kell dupla kattintassal inditani.
rem Elso inditaskor mindent elokeszit (ellenorzes, adatok kinyerese az eredeti jatekbol, forditas), utana csak inditja a jatekot.
rem Reszletek: README.md es docs\SETUP-hu.md. A szoveges uzenetek magyarul jelennek meg (UTF-8).
setlocal
cd /d "%~dp0"
chcp 65001 >nul
set "PYTHONUTF8=1"
set "PYTHONIOENCODING=utf-8"
set "WGPU_BACKEND=vulkan"
title A Mesterlovesz: Ujratoltve

rem Python 3: a "py" indito vagy a "python" parancs. (A Microsoft Store-os ures "python" nem szamit.)
set "PY="
py -3 -c "import sys" >nul 2>&1 && set "PY=py -3"
if not defined PY (python -c "import sys" >nul 2>&1 && set "PY=python")
if not defined PY (
  echo.
  echo HIBA: A Python 3 nem talalhato.
  echo Telepitsd a https://www.python.org/downloads/ oldalrol, es a telepitoben pipald be az "Add python.exe to PATH" jelolonegyzetet.
  echo Utana inditsd ujra az Indit.cmd fajlt.
  echo.
  pause
  exit /b 1
)

%PY% -m tools.export_all --run %*
set "EXITCODE=%ERRORLEVEL%"
if not "%EXITCODE%"=="0" (
  echo.
  echo Az Indit.cmd hibakoddal allt le ^(%EXITCODE%^). A fenti uzenet mondja el, mit kell javitani; utana inditsd ujra: a kesz lepeseket kihagyja.
  echo Ha nem boldogulsz: README.md, "Hibaelharitas" fejezet.
  pause
)
exit /b %EXITCODE%
