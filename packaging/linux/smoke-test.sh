#!/usr/bin/env bash
# Checks an installed package in a clean system: libraries resolve, the app starts, Python runs, a screenshot is saved.
# Whether the viewport drew is not checked: on Rocky 8 Mesa's software driver does not start, so without a GPU the
# viewport reports a Vulkan error and stays black. Look at window.png on a machine with a GPU.
# Needs xvfb-run. usage: smoke-test.sh [output folder]
# AZTEK_BIN: what to start instead of the installed aztek, such as an unpacked AppImage's AppRun.
set -euo pipefail

OUT=${1:-/tmp/aztek-smoke}
mkdir -p "$OUT"

if [ -n "${AZTEK_BIN:-}" ]; then
  BIN=$AZTEK_BIN
  # An AppImage brings its libraries: what its program and plugins still look for outside must be on this system.
  ROOT=$(dirname "$(readlink -f "$BIN")")
  for f in "$ROOT/usr/bin/aztek" "$ROOT"/usr/plugins/*/*.so; do
    [ -e "$f" ] || continue
    if LD_LIBRARY_PATH="$ROOT/usr/lib" ldd "$f" | grep "not found"; then
      echo "unresolved libraries in $f"
      exit 1
    fi
  done
else
  BIN=$(command -v aztek)
  echo "== ldd $BIN"
  ldd "$BIN" | tee "$OUT/ldd.txt"
  if grep -q "not found" "$OUT/ldd.txt"; then
    echo "unresolved libraries"
    exit 1
  fi
fi

# A bundled Qt's plugins are loaded later, by name, and would only fail at run time.
for lib in /usr/lib64/aztek/plugins/*/*.so; do
  [ -e "$lib" ] || continue
  if ldd "$lib" | grep "not found"; then
    echo "unresolved libraries in $lib"
    exit 1
  fi
done

echo "== aztek --help"
QT_QPA_PLATFORM=offscreen "$BIN" --help

ldconfig -p | grep -q 'libvulkan\.so\.1' || { echo "libvulkan.so.1 is missing"; exit 1; }

cat >"$OUT/check.py" <<PY
import sys, json, ctypes, ssl, sqlite3, zlib
import aztek.cmds as cmds
cube = cmds.polyCube()
with open("$OUT/python.txt", "w") as f:
    f.write(sys.version + "\n" + sys.prefix + "\n" + json.dumps(cube) + "\n")
PY

echo "== start under Xvfb"
rm -f "$OUT/python.txt" "$OUT/window.png"
timeout 180 xvfb-run -a -s "-screen 0 1280x800x24" "$BIN" --script "$OUT/check.py" --screenshot "$OUT/window.png"
cat "$OUT/python.txt"
test -s "$OUT/window.png"
echo "== passed"
