@echo off
call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars32.bat"
if errorlevel 1 exit /b 1
cmake -S "D:\Proj\Heshun-ime\heshun-tsf" -B "D:\Proj\Heshun-ime\build-tsf-phase5-x86" -G Ninja -DCMAKE_BUILD_TYPE=Release
if errorlevel 1 exit /b 1
cmake --build "D:\Proj\Heshun-ime\build-tsf-phase5-x86"
if errorlevel 1 exit /b 1
ctest --test-dir "D:\Proj\Heshun-ime\build-tsf-phase5-x86" --output-on-failure
exit /b %errorlevel%
