@echo off
setlocal

set "DLL=%~1"
if "%DLL%"=="" set "DLL=%~dp0..\..\build-tsf\bin\heshun_tsf.dll"
set "TOOL=%~dp0..\..\build-tsf\bin\heshun_tsf_profile.exe"

if exist "%TOOL%" "%TOOL%" unregister
if exist "%DLL%" regsvr32 /u /s "%DLL%"

echo heshun Zhengma TSF unregistered.
