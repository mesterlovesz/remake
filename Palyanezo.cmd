@echo off
rem Walks one level directly, without the menu: Palyanezo.cmd rh1-wiezienie3
setlocal
set "WGPU_BACKEND=vulkan"
set "LEVEL=%~1"
if "%LEVEL%"=="" set "LEVEL=rh1-wiezienie1"
set "ROOT=%~dp0"
set "VIEWER=%ROOT%crates\level-viewer\target\debug\level-viewer.exe"
if not exist "%VIEWER%" (
  echo A jatek meg nincs leforditva. Futtasd a Forditas.cmd fajlt.
  pause
  exit /b 1
)
pushd "%ROOT%crates\level-viewer"
"%VIEWER%" "%LEVEL%" "..\..\output"
set "EXITCODE=%ERRORLEVEL%"
popd
exit /b %EXITCODE%
