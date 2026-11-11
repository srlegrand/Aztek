#!/usr/bin/env bash
# Builds the Flatpak from an AppImage: packaging/flatpak/build-flatpak.sh aztek-<version>-x86_64.AppImage [output dir]
# Needs flatpak and flatpak-builder, and the freedesktop runtime and SDK the manifest names:
#   flatpak install --user flathub org.freedesktop.Platform//25.08 org.freedesktop.Sdk//25.08
# Install the result with: flatpak install --user aztek-<version>-x86_64.flatpak
set -euo pipefail

HERE=$(cd "$(dirname "$0")" && pwd)
IMAGE=$(readlink -f "$1")
OUT=$(mkdir -p "${2:-$PWD}" && cd "${2:-$PWD}" && pwd)
"$HERE/unpack-appimage.sh" "$IMAGE" "$HERE/AppDir"
VERSION=$(basename "$IMAGE" | sed -n 's/^aztek-\(.*\)-x86_64\.AppImage$/\1/p')
cd "$HERE"
flatpak-builder --user --force-clean --repo=repo build io.github.srlegrand.Aztek.yml
flatpak build-bundle repo "$OUT/aztek-${VERSION:-dev}-x86_64.flatpak" io.github.srlegrand.Aztek
ls -l "$OUT"/aztek-*.flatpak
