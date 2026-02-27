@echo off
set PYTHON=C:\Users\aliso\miniconda3\python.exe

echo Using Python: %PYTHON%
echo.

echo Installing PyInstaller...
%PYTHON% -m pip install pyinstaller

echo.
echo Building req_parser executable...
%PYTHON% -m PyInstaller ^
  --onefile ^
  --name req_parser-x86_64-pc-windows-msvc ^
  --distpath dist ^
  --workpath build_tmp ^
  --specpath build_tmp ^
  req_parser.py

echo.
echo Copying to src-tauri/binaries/...
if not exist "..\src-tauri\binaries" mkdir "..\src-tauri\binaries"
copy /Y "dist\req_parser-x86_64-pc-windows-msvc.exe" "..\src-tauri\binaries\req_parser-x86_64-pc-windows-msvc.exe"

echo.
echo Done! Rebuild the Tauri app to pick up the sidecar.
pause
