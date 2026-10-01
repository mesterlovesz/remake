@echo off
rem Starts the knajpa (Bullseye bar) level right next to Stella, who sits at the bar (native position 680,-252,-744).
rem The view faces her (yaw -pi/2 = towards +X). MESTER_SPAWN is a debug aid: x,y,z in native units, yaw in radians.
setlocal
set "WGPU_BACKEND=vulkan"
set "MESTER_SPAWN=560,-230,-744,-1.5708"
set "ROOT=%~dp0"
set "VIEWER=%ROOT%crates\level-viewer\target\debug\level-viewer.exe"
if not exist "%VIEWER%" (
  echo A jatek meg nincs leforditva. Futtasd a Forditas.cmd fajlt.
  pause
  exit /b 1
)
pushd "%ROOT%crates\level-viewer"
"%VIEWER%" knajpa "..\..\output"
set "EXITCODE=%ERRORLEVEL%"
popd
exit /b %EXITCODE%
