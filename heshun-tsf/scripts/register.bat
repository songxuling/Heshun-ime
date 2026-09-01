@echo off
setlocal EnableExtensions EnableDelayedExpansion

set "LOG=%TEMP%\heshun-tsf-register.log"
set "HANDOFF=%TEMP%\heshun-tsf-dll-path.txt"
if /i "%~1"=="--elevated" goto elevated

rem Resolve the DLL before UAC. The elevated child reads this handoff file,
rem because UAC does not preserve newly-set process environment variables.
set "DLL=%~f1"
if "%~1"=="" set "DLL=%~dp0heshun_tsf.dll"
if not exist "%DLL%" set "DLL=%~dp0..\..\build-tsf\bin\heshun_tsf.dll"
set "TOOL=%~dp0heshun_tsf_profile.exe"
if not "%~1"=="" if exist "%~dp1heshun_tsf_profile.exe" set "TOOL=%~dp1heshun_tsf_profile.exe"
if not exist "%TOOL%" set "TOOL=%~dp0..\..\build-tsf\bin\heshun_tsf_profile.exe"
if not exist "%DLL%" (
  echo DLL not found: %DLL%
  exit /b 1
)
>"%HANDOFF%" echo %DLL%

net session >nul 2>&1
if "!errorlevel!"=="0" goto elevated

echo Requesting Administrator permission for TSF registration...
echo The elevated installer log will be saved to: %LOG%
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0elevate-tsf.ps1" "%~f0" "%LOG%"
set "RC=!errorlevel!"
echo.
echo Elevated installer finished with exit code !RC!.
if exist "%LOG%" type "%LOG%"
del /q "%HANDOFF%" >nul 2>&1
exit /b !RC!

:elevated
set /p "DLL="<"%HANDOFF%"
if "%DLL%"=="" (
  echo Missing absolute DLL path handoff from elevation parent.
  exit /b 1
)
for %%I in ("%DLL%") do if exist "%%~dpIheshun_tsf_profile.exe" set "TOOL=%%~dpIheshun_tsf_profile.exe"
if not exist "%TOOL%" set "TOOL=%~dp0heshun_tsf_profile.exe"
if not exist "%TOOL%" set "TOOL=%~dp0..\..\build-tsf\bin\heshun_tsf_profile.exe"

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

echo heshun TSF input method registered successfully. Use Ctrl+` to switch Zhengma/Pinyin.
