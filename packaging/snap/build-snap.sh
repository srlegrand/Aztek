#!/usr/bin/env bash
# Builds the snap from an AppImage: packaging/snap/build-snap.sh aztek-<version>-x86_64.AppImage [output dir]
# Needs snapcraft (sudo snap install snapcraft --classic), which builds in LXD.
# Install the result with: sudo snap install --dangerous aztek_<version>_amd64.snap
set -euo pipefail

HERE=$(cd "$(dirname "$0")" && pwd)
IMAGE=$(readlink -f "$1")
OUT=$(mkdir -p "${2:-$PWD}" && cd "${2:-$PWD}" && pwd)
"$HERE/../flatpak/unpack-appimage.sh" "$IMAGE" "$HERE/AppDir"
basename "$IMAGE" | sed -n 's/^aztek-\(.*\)-x86_64\.AppImage$/\1/p' >"$HERE/version"
cd "$HERE"
snapcraft pack --output "$OUT/"
ls -l "$OUT"/aztek_*.snap
