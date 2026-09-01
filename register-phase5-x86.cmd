@echo off
cd /d D:\Proj\Heshun-ime
call heshun-tsf\scripts\register.bat build-tsf-phase5-x86\bin\heshun_tsf.dll
exit /b %errorlevel%
