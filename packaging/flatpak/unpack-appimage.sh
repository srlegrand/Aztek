#!/usr/bin/env bash
# Unpacks an AppImage into a folder, without mounting it: unpack-appimage.sh file.AppImage folder
set -euo pipefail
IMAGE=$(readlink -f "$1")
mkdir -p "$(dirname "$2")"
DEST=$(cd "$(dirname "$2")" && pwd)/$(basename "$2")
WORK=$(mktemp -d)
chmod +x "$IMAGE"
(cd "$WORK" && "$IMAGE" --appimage-extract >/dev/null)
rm -rf "$DEST"
mv "$WORK/squashfs-root" "$DEST"
rmdir "$WORK"
test -x "$DEST/usr/bin/aztek"
