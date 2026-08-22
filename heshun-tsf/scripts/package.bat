@echo off
setlocal EnableExtensions

set "BIN=%~f1"
if "%~1"=="" set "BIN=%~dp0..\..\build-tsf\bin"
set "DIST=%~dp0..\dist"

if not exist "%BIN%\heshun_tsf.dll" (
  echo TSF DLL not found: %BIN%\heshun_tsf.dll
  exit /b 1
)
if not exist "%BIN%\heshun.dll" (
  echo Engine DLL not found: %BIN%\heshun.dll
  exit /b 1
)
if not exist "%BIN%\heshun_tsf_profile.exe" (
  echo Profile tool not found: %BIN%\heshun_tsf_profile.exe
  exit /b 1
)
if not exist "%BIN%\schemas\zhengma66.schema.yaml" (
  echo Schema directory is incomplete: %BIN%\schemas
  exit /b 1
)

rmdir /s /q "%DIST%" 2>nul
mkdir "%DIST%\schemas" || exit /b 1
copy /y "%BIN%\heshun_tsf.dll" "%DIST%\" >nul || exit /b 1
copy /y "%BIN%\heshun.dll" "%DIST%\" >nul || exit /b 1
copy /y "%BIN%\heshun_tsf_profile.exe" "%DIST%\" >nul || exit /b 1
xcopy /e /i /y /q "%BIN%\schemas" "%DIST%\schemas" >nul || exit /b 1
copy /y "%~dp0register.bat" "%DIST%\" >nul || exit /b 1
copy /y "%~dp0unregister.bat" "%DIST%\" >nul || exit /b 1
copy /y "%~dp0elevate-tsf.ps1" "%DIST%\" >nul || exit /b 1
copy /y "%~dp0..\README.md" "%DIST%\README.md" >nul || exit /b 1

echo Package created: %DIST%
echo Install with: %DIST%\register.bat
