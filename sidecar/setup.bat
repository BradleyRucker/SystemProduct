@echo off
set PYTHON=C:\Users\aliso\miniconda3\python.exe

echo Using Python: %PYTHON%
echo.

echo Installing spaCy...
%PYTHON% -m pip install spacy

echo.
echo Downloading en_core_web_sm model...
%PYTHON% -m spacy download en_core_web_sm

echo.
echo Testing req_parser...
echo {"sentences":["The system shall transmit data at 100 Mbps.", "It must be noted that this is important.", "The sensor shall measure temperature within plus or minus 0.5 degrees C."]} | %PYTHON% req_parser.py

echo.
echo Done! If you see JSON output above, the sidecar is working.
pause
