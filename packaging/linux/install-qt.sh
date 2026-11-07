#!/usr/bin/env bash
# Downloads Qt's own Linux kit with aqtinstall for systems without Qt 6 packages (Rocky 8).
# usage: install-qt.sh <python> <version> <dir>; the kit lands in <dir>/<version>/gcc_64.
set -euo pipefail

PY=$1 VER=$2 DIR=$3
if [ -d "$DIR/$VER/gcc_64/lib/cmake/Qt6" ]; then
  echo "Qt $VER already in $DIR"
  exit 0
fi
mkdir -p "$DIR"
"$PY" -m venv "$DIR/.aqt"
"$DIR/.aqt/bin/python" -m pip install -q aqtinstall
"$DIR/.aqt/bin/python" -m aqt install-qt linux desktop "$VER" linux_gcc_64 --outputdir "$DIR"
rm -rf "$DIR/.aqt"
