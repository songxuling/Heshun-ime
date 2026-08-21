@echo off
setlocal EnableExtensions EnableDelayedExpansion

set "DLL=%~f1"
if "%~1"=="" set "DLL=%~dp0..\..\build-tsf\bin\heshun_tsf.dll"
for %%I in ("%DLL%") do set "DLL=%%~fI"
set "LOG=%TEMP%\heshun-tsf-unregister.log"

net session >nul 2>&1
if not "!errorlevel!"=="0" (
  echo Requesting Administrator permission for TSF removal...
  echo The elevated removal log will be saved to: %LOG%
  powershell -NoProfile -ExecutionPolicy Bypass -Command "$p = Start-Process -FilePath $env:ComSpec -Verb RunAs -Wait -PassThru -ArgumentList '/d /c """"%~f0"" --elevated ""%DLL%"" ^> ""%LOG%"" 2^>^&1""'; exit $p.ExitCode"
  set "RC=!errorlevel!"
  echo.
  echo Elevated removal finished with exit code !RC!.
  if exist "%LOG%" type "%LOG%"
  exit /b !RC!
)

if /i "%~1"=="--elevated" set "DLL=%~f2"
set "TOOL=%~dp0..\..\build-tsf\bin\heshun_tsf_profile.exe"

echo [heshun-tsf] Running elevated removal.
echo [heshun-tsf] DLL: %DLL%
if exist "%TOOL%" "%TOOL%" unregister
if exist "%DLL%" regsvr32 /u /s "%DLL%"

echo heshun Zhengma TSF unregistered.
