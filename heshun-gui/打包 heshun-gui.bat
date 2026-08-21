@echo off
setlocal

rem Build a movable release distribution folder beside this script.
set "ROOT=%~dp0"
set "EXE=%ROOT%..\target\x86_64-pc-windows-msvc\release\heshun-gui.exe"
set "DIST=%ROOT%dist"
set "SCHEMAS=%ROOT%..\heshun\schemas"

if not exist "%EXE%" (
  echo Release executable was not found:
  echo %EXE%
  echo Build the workspace with the MSVC Rust toolchain first.
  pause
  exit /b 1
)

if not exist "%SCHEMAS%\zhengma.bin" (
  echo Runtime dictionaries were not found:
  echo %SCHEMAS%
  pause
  exit /b 1
)

if exist "%DIST%" rmdir /s /q "%DIST%"
mkdir "%DIST%\schemas" "%DIST%\data" || exit /b 1
copy /y "%EXE%" "%DIST%\heshun-gui.exe" >nul || exit /b 1
copy /y "%SCHEMAS%\*.schema.yaml" "%DIST%\schemas\" >nul || exit /b 1
copy /y "%SCHEMAS%\*.bin" "%DIST%\schemas\" >nul || exit /b 1
copy /y "%ROOT%启动 heshun-gui.bat" "%DIST%\启动 heshun-gui.bat" >nul || exit /b 1

echo Distribution created:
echo %DIST%
pause
