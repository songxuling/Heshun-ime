@echo off
setlocal EnableExtensions EnableDelayedExpansion

set "LOG=%TEMP%\heshun-tsf-unregister.log"
if /i "%~1"=="--elevated" goto elevated

set "DLL=%~f1"
if "%~1"=="" set "DLL=%~dp0..\..\build-tsf\bin\heshun_tsf.dll"
for %%I in ("%DLL%") do set "DLL=%%~fI"

net session >nul 2>&1
if "!errorlevel!"=="0" goto elevated

echo Requesting Administrator permission for TSF removal...
echo The elevated removal log will be saved to: %LOG%
set "HESHUN_TSF_DLL=%DLL%"
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0elevate-tsf.ps1" "%~f0" "%LOG%"
set "RC=!errorlevel!"
echo.
echo Elevated removal finished with exit code !RC!.
if exist "%LOG%" type "%LOG%"
exit /b !RC!

:elevated
set "DLL=%HESHUN_TSF_DLL%"
if "%DLL%"=="" (
  echo Missing absolute DLL path from elevation parent.
  exit /b 1
)
set "TOOL=%~dp0..\..\build-tsf\bin\heshun_tsf_profile.exe"

echo [heshun-tsf] Running elevated removal.
echo [heshun-tsf] DLL: %DLL%
if exist "%TOOL%" "%TOOL%" unregister
if exist "%DLL%" regsvr32 /u /s "%DLL%"

echo heshun Zhengma TSF unregistered.
