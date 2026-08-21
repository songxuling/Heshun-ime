@echo off
setlocal

set "DLL=%~1"
if "%DLL%"=="" set "DLL=%~dp0..\..\build-tsf\bin\heshun_tsf.dll"
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
