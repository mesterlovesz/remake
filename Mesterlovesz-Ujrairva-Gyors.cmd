@echo off
rem Mesterlovesz Ujrairva, optimised (release) build: the fastest way to play. The first run compiles it (long, once); later runs start at once.
rem The normal launchers use the debug build (Forditas.cmd), which is optimised only for the dependencies and is slower.
setlocal
set "WGPU_BACKEND=vulkan"
set "ROOT=%~dp0"
set "TOOLCHAIN=%ROOT%..\..\_toolchain"
if exist "%TOOLCHAIN%\cargo\bin\cargo.exe" (
  set "CARGO_HOME=%TOOLCHAIN%\cargo"
  set "RUSTUP_HOME=%TOOLCHAIN%\rustup"
  set "PATH=%TOOLCHAIN%\cargo\bin;%PATH%"
)
set "VIEWER=%ROOT%crates\level-viewer\target\release\level-viewer.exe"
if not exist "%VIEWER%" (
  echo Elso inditas: a gyors valtozat fordul, ez sokaig tart...
  pushd "%ROOT%crates\level-viewer"
  cargo build --release || (popd & echo A forditas nem sikerult. & pause & exit /b 1)
  popd
)
pushd "%ROOT%crates\level-viewer"
"%VIEWER%" menu "..\..\output"
set "EXITCODE=%ERRORLEVEL%"
popd
exit /b %EXITCODE%
