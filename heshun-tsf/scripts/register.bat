@echo off
setlocal EnableExtensions

rem TSF language profile registration writes protected CTF configuration.
rem Re-launch through UAC when this batch was started without elevation.
net session >nul 2>&1
if not "%errorlevel%"=="0" (
  echo Requesting Administrator permission for TSF registration...
  powershell -NoProfile -ExecutionPolicy Bypass -Command "Start-Process -FilePath '%ComSpec%' -Verb RunAs -WorkingDirectory '%CD%' -ArgumentList '/c','""%~f0"" %*'"
  exit /b %errorlevel%
)

set "DLL=%~f1"
if "%~1"=="" set "DLL=%~dp0..\..\build-tsf\bin\heshun_tsf.dll"
set "TOOL=%~dp0..\..\build-tsf\bin\heshun_tsf_profile.exe"

if not exist "%DLL%" (
  echo DLL not found: %DLL%
  exit /b 1
)
if not exist "%TOOL%" (
  echo Profile tool not found: %TOOL%
  exit /b 1
)

regsvr32 /s "%DLL%"
if errorlevel 1 (
  echo COM registration failed.
  exit /b 1
)
"%TOOL%" register "%DLL%"
if errorlevel 1 (
  echo TSF profile registration failed. COM registration remains installed.
  exit /b 1
)

echo heshun Zhengma TSF registered successfully.
