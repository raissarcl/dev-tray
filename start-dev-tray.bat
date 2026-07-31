@echo off
REM Starts Dev Tray detached (no console window). Safe to close this script immediately.
setlocal
set "EXE=%~dp0src-tauri\target\release\tray-for-projects.exe"
if not exist "%EXE%" (
  echo Building Dev Tray release binary first...
  pushd "%~dp0"
  call npm run build
  if errorlevel 1 exit /b 1
  pushd src-tauri
  call cargo build --release
  if errorlevel 1 exit /b 1
  popd
  popd
)
start "" "%EXE%"
echo Dev Tray started. Look for the teal/white icon in the system tray
echo (including the ^ overflow chevron if Windows hid it).
endlocal
