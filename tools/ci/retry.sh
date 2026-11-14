#!/usr/bin/env bash
# Runs a command until it succeeds: tools/ci/retry.sh <attempts> <command...>.
# CI uses it so a dropped download or a build killed for memory doesn't fail the run. After a failure the next attempt
# waits longer and builds with one job at a time, which needs the least memory.
set -uo pipefail
n=$1
shift
for i in $(seq 1 "$n"); do
  "$@"
  rc=$?
  [ "$rc" = 0 ] && exit 0
  if [ "$i" = "$n" ]; then
    echo "::error::failed $n times ($rc): $*"
    exit "$rc"
  fi
  echo "::warning::attempt $i of $n failed ($rc): $*. Trying again with one job at a time."
  export CARGO_BUILD_JOBS=1 CMAKE_BUILD_PARALLEL_LEVEL=1 RUST_TEST_THREADS=1
  sleep $((i * 60))
done
