#!/usr/bin/env bash
# Frees disk on a GitHub-hosted Ubuntu runner by removing the SDKs Aztek's build never uses (about 25 GB).
# tools/ci/free-disk.sh [root]: root is where the runner's / is seen, for a container job that mounts it.
# Never fails the job: what it can't remove is left.
R=${1:-}
SUDO=
[ -z "$R" ] && [ "$(id -u)" != 0 ] && SUDO=sudo
for d in usr/share/dotnet usr/local/lib/android opt/ghc usr/local/.ghcup opt/hostedtoolcache/CodeQL usr/share/swift \
         usr/local/share/powershell usr/local/share/chromium usr/local/lib/node_modules; do
  [ -d "$R/$d" ] && $SUDO rm -rf "${R:?}/$d"/* 2>/dev/null || true
done
df -h "${R:-/}" | tail -1
