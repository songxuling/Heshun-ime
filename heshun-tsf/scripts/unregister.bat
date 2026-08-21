@echo off
setlocal EnableExtensions

set "LOG=%TEMP%\heshun-tsf-unregister.log"
net session >nul 2>&1
if not "%errorlevel%"=="0" (
  echo Requesting Administrator permission for TSF removal...
  echo The elevated removal log will be saved to: %LOG%
  powershell -NoProfile -ExecutionPolicy Bypass -Command "$p = Start-Process -FilePath $env:ComSpec -Verb RunAs -Wait -PassThru -WorkingDirectory $pwd -ArgumentList '/c','""%~f0"" --elevated %* > ""%LOG%"" 2^>^&1'; exit $p.ExitCode"
  set "RC=%errorlevel%"
  echo.
  echo Elevated removal finished with exit code %RC%.
  if exist "%LOG%" type "%LOG%"
  exit /b %RC%
)

set "DLL=%~f1"
if /i "%~1"=="--elevated" set "DLL=%~f2"
if "%DLL%"=="" set "DLL=%~dp0..\..\build-tsf\bin\heshun_tsf.dll"
set "TOOL=%~dp0..\..\build-tsf\bin\heshun_tsf_profile.exe"

echo [heshun-tsf] Running elevated removal.
if exist "%TOOL%" "%TOOL%" unregister
if exist "%DLL%" regsvr32 /u /s "%DLL%"

echo heshun Zhengma TSF unregistered.
