@echo off
setlocal EnableExtensions EnableDelayedExpansion

set "LOG=%TEMP%\heshun-tsf-unregister.log"
set "HANDOFF=%TEMP%\heshun-tsf-dll-path.txt"
if /i "%~1"=="--elevated" goto elevated

set "DLL=%~f1"
if "%~1"=="" set "DLL=%~dp0heshun_tsf.dll"
if not exist "%DLL%" set "DLL=%~dp0..\..\build-tsf\bin\heshun_tsf.dll"
set "TOOL=%~dp0heshun_tsf_profile.exe"
if not exist "%TOOL%" set "TOOL=%~dp0..\..\build-tsf\bin\heshun_tsf_profile.exe"
>"%HANDOFF%" echo %DLL%

net session >nul 2>&1
if "!errorlevel!"=="0" goto elevated

echo Requesting Administrator permission for TSF removal...
echo The elevated removal log will be saved to: %LOG%
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0elevate-tsf.ps1" "%~f0" "%LOG%"
set "RC=!errorlevel!"
echo.
echo Elevated removal finished with exit code !RC!.
if exist "%LOG%" type "%LOG%"
del /q "%HANDOFF%" >nul 2>&1
exit /b !RC!

:elevated
set /p "DLL="<"%HANDOFF%"
if "%DLL%"=="" (
  echo Missing absolute DLL path handoff from elevation parent.
  exit /b 1
)
if not exist "%TOOL%" set "TOOL=%~dp0heshun_tsf_profile.exe"
if not exist "%TOOL%" set "TOOL=%~dp0..\..\build-tsf\bin\heshun_tsf_profile.exe"

echo [heshun-tsf] Running elevated removal.
echo [heshun-tsf] DLL: %DLL%
if exist "%TOOL%" "%TOOL%" unregister
if exist "%DLL%" regsvr32 /u /s "%DLL%"

echo heshun TSF input method and legacy profiles unregistered.
