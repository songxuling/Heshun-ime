@echo off
setlocal EnableExtensions EnableDelayedExpansion

rem Resolve the DLL while still in the caller's working directory. The UAC child
rem starts from System32 on some systems, so it must receive an absolute path.
set "DLL=%~f1"
if "%~1"=="" set "DLL=%~dp0..\..\build-tsf\bin\heshun_tsf.dll"
for %%I in ("%DLL%") do set "DLL=%%~fI"
set "LOG=%TEMP%\heshun-tsf-register.log"

rem TSF profile registration writes protected CTF configuration.
net session >nul 2>&1
if not "!errorlevel!"=="0" (
  echo Requesting Administrator permission for TSF registration...
  echo The elevated installer log will be saved to: %LOG%
  powershell -NoProfile -ExecutionPolicy Bypass -Command "$p = Start-Process -FilePath $env:ComSpec -Verb RunAs -Wait -PassThru -ArgumentList '/d /c """"%~f0"" --elevated ""%DLL%"" ^> ""%LOG%"" 2^>^&1""'; exit $p.ExitCode"
  set "RC=!errorlevel!"
  echo.
  echo Elevated installer finished with exit code !RC!.
  if exist "%LOG%" type "%LOG%"
  exit /b !RC!
)

if /i "%~1"=="--elevated" set "DLL=%~f2"
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
