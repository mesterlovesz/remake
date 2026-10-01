@echo off
rem Mesterlovesz Ujrairva: the unmodded 1:1 remake.
setlocal
set "WGPU_BACKEND=vulkan"
set "ROOT=%~dp0"
set "VIEWER=%ROOT%crates\level-viewer\target\debug\level-viewer.exe"
if not exist "%VIEWER%" (
  echo A Mesterlovesz Ujrairva meg nincs leforditva. Futtasd a Forditas.cmd fajlt.
  pause
  exit /b 1
)
pushd "%ROOT%crates\level-viewer"
"%VIEWER%" menu "..\..\output"
set "EXITCODE=%ERRORLEVEL%"
popd
exit /b %EXITCODE%
