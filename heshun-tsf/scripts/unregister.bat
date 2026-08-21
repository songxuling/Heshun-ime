@echo off
setlocal EnableExtensions

net session >nul 2>&1
if not "%errorlevel%"=="0" (
  echo Requesting Administrator permission for TSF removal...
  powershell -NoProfile -ExecutionPolicy Bypass -Command "Start-Process -FilePath '%ComSpec%' -Verb RunAs -WorkingDirectory '%CD%' -ArgumentList '/c','""%~f0"" %*'"
  exit /b %errorlevel%
)

set "DLL=%~f1"
if "%~1"=="" set "DLL=%~dp0..\..\build-tsf\bin\heshun_tsf.dll"
set "TOOL=%~dp0..\..\build-tsf\bin\heshun_tsf_profile.exe"

if exist "%TOOL%" "%TOOL%" unregister
if exist "%DLL%" regsvr32 /u /s "%DLL%"

echo heshun Zhengma TSF unregistered.
