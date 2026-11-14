#!/usr/bin/env bash
# Compiles the plug-in kit and every shipped plug-in with clang and GCC, to catch in a minute what would fail the
# macOS (clang) and Linux builds: tools/check_kit.sh. Only errors fail it; the plug-ins' own warnings are theirs.
set -uo pipefail
cd "$(dirname "$0")/.."
fail=0
for cxx in clang++ g++; do
  command -v "$cxx" >/dev/null || { echo "$cxx: not installed, skipped"; continue; }
  for f in devkit/src/*.cpp plugins/*/*.cpp devkit/samples/*/*.cpp; do
    [ -f "$f" ] || continue
    if ! out=$("$cxx" -std=c++14 -DREQUIRE_IOSTREAM -D_BOOL -DLINUX -fsyntax-only -Idevkit/include -Idevkit/src -I"$(dirname "$f")" "$f" 2>&1); then
      echo "$cxx $f:"; echo "$out" | grep -E "error" | head -5; fail=1
    fi
  done
done
[ $fail = 0 ] && echo "kit and plug-ins: no errors"
exit $fail
