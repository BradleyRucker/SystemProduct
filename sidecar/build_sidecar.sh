#!/usr/bin/env bash
set -euo pipefail

PYTHON="${PYTHON:-python3}"

echo "Using Python: $($PYTHON --version)"
echo

echo "Installing PyInstaller..."
"$PYTHON" -m pip install pyinstaller

echo
echo "Building req_parser executable..."
"$PYTHON" -m PyInstaller \
  --onefile \
  --name req_parser-x86_64-unknown-linux-gnu \
  --distpath dist \
  --workpath build_tmp \
  --specpath build_tmp \
  req_parser.py

echo
echo "Copying to src-tauri/binaries/..."
mkdir -p ../src-tauri/binaries
cp -f dist/req_parser-x86_64-unknown-linux-gnu ../src-tauri/binaries/req_parser-x86_64-unknown-linux-gnu

echo
echo "Done! Rebuild the Tauri app to pick up the sidecar."
