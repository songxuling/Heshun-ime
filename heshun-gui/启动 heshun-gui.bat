@echo off
setlocal

rem Double-click to start heshun-gui. Closing the GUI window exits it.
set "ROOT=%~dp0"
set "GUI_EXE=%ROOT%heshun-gui.exe"

rem Development-tree fallback: workspace target artifact.
if not exist "%GUI_EXE%" set "GUI_EXE=%ROOT%..\target\x86_64-pc-windows-msvc\debug\heshun-gui.exe"

if not exist "%GUI_EXE%" (
  echo GUI executable was not found:
  echo %GUI_EXE%
  echo Build the workspace first, then run this launcher again.
  pause
  exit /b 1
)

start "heshun-gui" /D "%ROOT%" "%GUI_EXE%"
exit /b 0
