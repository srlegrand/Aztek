#!/usr/bin/env bash
# Copies the Qt libraries and plugins a binary needs from an aqtinstall kit: bundle-qt.sh <kit> <binary> <dest>.
# The kit is relocatable: libQt6Core finds the plugins at ../plugins, so no qt.conf or wrapper script is needed.
set -euo pipefail

KIT=$1 BIN=$2 DEST=$3

mkdir -p "$DEST/lib"
for p in platforms/libqxcb.so platforms/libqoffscreen.so \
         platforminputcontexts/libcomposeplatforminputcontextplugin.so \
         platforminputcontexts/libibusplatforminputcontextplugin.so \
         xcbglintegrations/libqxcb-glx-integration.so xcbglintegrations/libqxcb-egl-integration.so \
         imageformats/libqjpeg.so imageformats/libqgif.so imageformats/libqico.so; do
  mkdir -p "$DEST/plugins/$(dirname "$p")"
  cp -p "$KIT/plugins/$p" "$DEST/plugins/$p"
done

# Every library of the kit, or that only EPEL has, needed by the binary or the plugins, and by those libraries.
copy_needed() {
  local lib name
  for lib in $(LD_LIBRARY_PATH="$KIT/lib" ldd "$1" | awk '/=>/ {print $3}'); do
    case "$lib" in
      "$KIT"/lib/*|*/libxcb-cursor.so*)
        name=$(basename "$lib")
        if [ ! -e "$DEST/lib/$name" ]; then
          cp -pL "$lib" "$DEST/lib/$name"
          chmod 755 "$DEST/lib/$name"
          copy_needed "$lib"
        fi ;;
    esac
  done
}
copy_needed "$BIN"
for p in $(find "$DEST/plugins" -name '*.so'); do copy_needed "$p"; done
