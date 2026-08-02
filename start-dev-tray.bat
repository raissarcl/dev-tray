@echo off
REM Starts Dev Tray detached (no console window). Safe to close this script immediately.
setlocal
cd /d "%~dp0"
set "EXE=%~dp0src-tauri\target\release\tray-for-projects.exe"
if not exist "%EXE%" (
  echo Building Dev Tray release binary first...
  call npm run build
  if errorlevel 1 exit /b 1
  pushd src-tauri
  call cargo build --release
  if errorlevel 1 exit /b 1
  popd
)
REM Working directory = repo root so projects.json next to this .bat is found.
start "" /D "%~dp0" "%EXE%"
echo Dev Tray started. Look for the teal/white icon in the system tray
echo (including the ^ overflow chevron if Windows hid it).
echo Config: %~dp0projects.json
endlocal
