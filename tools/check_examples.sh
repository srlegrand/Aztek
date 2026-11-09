#!/usr/bin/env bash
# Every example under --batch: scenes open with no warnings, scripts exit 0. Usage: tools/check_examples.sh build/aztek
set -u
cd "$(dirname "$0")/.."
bin=$1
case "$bin" in /*) ;; *) bin="$PWD/$bin" ;; esac
bad=0
while IFS= read -r f; do
  out=$("$bin" --batch "$f" 2>&1)
  rc=$?
  if [ "$rc" -ne 0 ] || printf '%s' "$out" | grep -qiE 'warning|error'; then
    echo "FAILED $f (exit $rc)"
    printf '%s\n' "$out" | grep -iE 'warning|error' | head -5
    bad=$((bad + 1))
  fi
done < <(find examples -name '*.ma' | sort)
echo "examples: $bad failed"
exit "$((bad > 0))"
