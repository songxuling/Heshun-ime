@echo off
setlocal EnableExtensions EnableDelayedExpansion

set "LOG=%TEMP%\heshun-tsf-register.log"
if /i "%~1"=="--elevated" goto elevated

rem Resolve the DLL before UAC. The child consumes HESHUN_TSF_DLL rather than
rem parsing a relative command line from C:\Windows\System32.
set "DLL=%~f1"
if "%~1"=="" set "DLL=%~dp0..\..\build-tsf\bin\heshun_tsf.dll"
for %%I in ("%DLL%") do set "DLL=%%~fI"
if not exist "%DLL%" (
  echo DLL not found: %DLL%
  exit /b 1
)

net session >nul 2>&1
if "!errorlevel!"=="0" goto elevated

echo Requesting Administrator permission for TSF registration...
echo The elevated installer log will be saved to: %LOG%
set "HESHUN_TSF_DLL=%DLL%"
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0elevate-tsf.ps1" "%~f0" "%LOG%"
set "RC=!errorlevel!"
echo.
echo Elevated installer finished with exit code !RC!.
if exist "%LOG%" type "%LOG%"
exit /b !RC!

:elevated
set "DLL=%HESHUN_TSF_DLL%"
if "%DLL%"=="" (
  echo Missing absolute DLL path from elevation parent.
  exit /b 1
)
set "TOOL=%~dp0..\..\build-tsf\bin\heshun_tsf_profile.exe"

echo [heshun-tsf] Running elevated registration.
echo [heshun-tsf] DLL: %DLL%
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
