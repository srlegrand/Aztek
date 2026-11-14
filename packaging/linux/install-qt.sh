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
"$DIR/.aqt/bin/python" -m pip install -q --retries 10 --timeout 120 aqtinstall
# Qt's download servers drop connections now and then: a few tries, each starting clean.
for i in 1 2 3 4 5; do
  if "$DIR/.aqt/bin/python" -m aqt install-qt linux desktop "$VER" linux_gcc_64 --outputdir "$DIR"; then
    break
  fi
  [ "$i" = 5 ] && { echo "Qt $VER: download failed 5 times"; exit 1; }
  rm -rf "${DIR:?}/$VER"
  sleep $((i * 60))
done
rm -rf "$DIR/.aqt"
