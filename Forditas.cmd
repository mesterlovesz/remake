@echo off
rem Builds Mesterlovesz Ujrairva with the project-local Rust toolchain.
setlocal
set "TOOLCHAIN=%~dp0..\..\_toolchain"
if exist "%TOOLCHAIN%\cargo\bin\cargo.exe" (
  set "CARGO_HOME=%TOOLCHAIN%\cargo"
  set "RUSTUP_HOME=%TOOLCHAIN%\rustup"
  set "PATH=%TOOLCHAIN%\cargo\bin;%PATH%"
)
pushd "%~dp0crates\level-viewer"
cargo build || goto :fail
popd
echo Kesz.
exit /b 0
:fail
popd
echo A forditas nem sikerult.
pause
exit /b 1
