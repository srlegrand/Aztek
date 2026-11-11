#!/usr/bin/env bash
# Builds an AppImage: one file that runs on any x86_64 Linux desktop with a Vulkan driver, with Qt, Python and the
# libraries a distribution may not have inside it.
#   packaging/appimage/build-appimage.sh [build dir] [output dir]
# Built on the oldest system it should run on: its C library is the newest the result needs (CI uses Rocky 8).
# AZTEK_QT: a Qt kit to bundle (as packaging/linux/install-qt.sh makes); without it the system's Qt 6 is bundled.
# AZTEK_PYTHON: the interpreter to embed, python3 by default. APPIMAGETOOL: an appimagetool to use, else downloaded.
set -euo pipefail

SRC=$(cd "$(dirname "$0")/../.." && pwd)
mkdir -p "${1:-$SRC/build}" "${2:-$SRC/dist}"
BUILD=$(cd "${1:-$SRC/build}" && pwd)
OUT=$(cd "${2:-$SRC/dist}" && pwd)
PY=${AZTEK_PYTHON:-$(command -v python3)}
QT=${AZTEK_QT:-}
VERSION=$(sed -n 's/^version *= *"\(.*\)"/\1/p' "$SRC/Cargo.toml" | head -n1)
FILE="aztek-${VERSION}-x86_64.AppImage"

ARGS=(-DPython3_EXECUTABLE="$PY")
[ -n "$QT" ] && ARGS+=(-DCMAKE_PREFIX_PATH="$QT")
PYO3_PYTHON="$PY" cmake -S "$SRC" -B "$BUILD" -G Ninja -DCMAKE_BUILD_TYPE=Release "${ARGS[@]}"
PYO3_PYTHON="$PY" cmake --build "$BUILD"

APPDIR=$BUILD/AppDir
rm -rf "$APPDIR"
DESTDIR=$APPDIR cmake --install "$BUILD" --prefix /usr --strip
LIB=$APPDIR/usr/lib
mkdir -p "$LIB" "$APPDIR/usr/share/aztek/examples"

# Qt's plugins: the kit's, or the system's.
if [ -n "$QT" ]; then
  PLUGINS=$QT/plugins
  export LD_LIBRARY_PATH="$QT/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
else
  PLUGINS=$(qmake6 -query QT_INSTALL_PLUGINS 2>/dev/null || qtpaths6 --plugin-dir)
fi
for p in platforms/libqxcb.so platforms/libqoffscreen.so \
         platforminputcontexts/libcomposeplatforminputcontextplugin.so platforminputcontexts/libibusplatforminputcontextplugin.so \
         xcbglintegrations/libqxcb-glx-integration.so xcbglintegrations/libqxcb-egl-integration.so \
         imageformats/libqjpeg.so imageformats/libqgif.so imageformats/libqico.so imageformats/libqsvg.so iconengines/libqsvgicon.so; do
  # The SVG plugins are there when the Qt that is bundled has them.
  [ -e "$PLUGINS/$p" ] || continue
  mkdir -p "$APPDIR/usr/plugins/$(dirname "$p")"
  cp -pL "$PLUGINS/$p" "$APPDIR/usr/plugins/$p"
done
# The system's Qt looks for its plugins where it was installed unless told otherwise.
printf '[Paths]\nPrefix = ..\nPlugins = plugins\n' >"$APPDIR/usr/bin/qt.conf"

# Python: its library comes with the other libraries below; the standard library goes where PYTHONHOME=usr finds it.
STDLIB=$("$PY" -c 'import sysconfig; print(sysconfig.get_path("stdlib"))')
PYPREFIX=$("$PY" -c 'import sys; print(sys.base_prefix)')
PYDEST=$APPDIR/usr/${STDLIB#"$PYPREFIX"/}
mkdir -p "$PYDEST"
(cd "$STDLIB" && tar -cf - --exclude=__pycache__ --exclude=test --exclude=tests --exclude=idlelib --exclude=tkinter \
  --exclude=turtledemo --exclude=ensurepip --exclude=site-packages --exclude=dist-packages --exclude='config-*' .) | tar -xf - -C "$PYDEST"
mkdir -p "$PYDEST/site-packages"
# Debian keeps the compiled modules of the standard library beside it under another name.
DYNLOAD=$("$PY" -c 'import sysconfig; print(sysconfig.get_config_var("DESTSHARED"))')
if [ ! -d "$PYDEST/lib-dynload" ] && [ -d "$DYNLOAD" ]; then cp -a "$DYNLOAD" "$PYDEST/lib-dynload"; fi

# Every library the program, the plugins and Python's modules load, and what those load, except what every desktop
# has and what has to match the machine: the C library, the graphics drivers' side (GL, Vulkan, DRM), X11 and
# Wayland's own, fontconfig and freetype, and the like. The list follows the AppImage project's excludelist.
SKIP='^(ld-linux.*|libc|libm|libdl|libpthread|librt|libutil|libresolv|libnsl|libanl|libBrokenLocale|libnss_.*|libcrypt|libgcc_s|libstdc\+\+|libGL|libGLX|libGLdispatch|libEGL|libOpenGL|libGLESv2|libvulkan|libdrm|libgbm|libglapi|libX11|libX11-xcb|libxcb|libxcb-dri2|libxcb-dri3|libxcb-glx|libxcb-present|libxshmfence|libXau|libXdmcp|libXext|libXrender|libXfixes|libXi|libXcursor|libXrandr|libXdamage|libXxf86vm|libICE|libSM|libwayland-client|libwayland-cursor|libwayland-egl|libwayland-server|libfontconfig|libfreetype|libharfbuzz|libfribidi|libexpat|libz|libuuid|libcom_err|libgpg-error|libglib-2\.0|libgobject-2\.0|libgio-2\.0|libgmodule-2\.0|libgthread-2\.0|libdbus-1|libasound|libjack|libpipewire.*|libp11-kit|libusb-1\.0|libthai|libgmp|libselinux|libsystemd|libudev|libcap|libmount|libblkid)\.so'
copy_needed() {
  local lib name
  for lib in $(ldd "$1" 2>/dev/null | awk '/=> \// {print $3}'); do
    name=$(basename "$lib")
    if echo "$name" | grep -Eq "$SKIP" || [ -e "$LIB/$name" ]; then continue; fi
    cp -pL "$lib" "$LIB/$name"
    chmod 755 "$LIB/$name"
    copy_needed "$lib"
  done
}
copy_needed "$APPDIR/usr/bin/aztek"
for f in $(find "$APPDIR/usr/plugins" "$PYDEST/lib-dynload" -name '*.so'); do copy_needed "$f"; done
find "$LIB" -maxdepth 1 -name '*.so*' -exec strip --strip-unneeded {} + 2>/dev/null || true

cat >"$APPDIR/AppRun" <<'RUN'
#!/bin/sh
# Starts Aztek from wherever the AppImage is mounted or unpacked, with the libraries and Python inside it.
HERE=$(dirname "$(readlink -f "$0")")
export PYTHONHOME="$HERE/usr"
export PYTHONNOUSERSITE=1
export LD_LIBRARY_PATH="$HERE/usr/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
exec "$HERE/usr/bin/aztek" "$@"
RUN
chmod 755 "$APPDIR/AppRun"
cp "$APPDIR/usr/share/applications/aztek.desktop" "$APPDIR/aztek.desktop"
cp "$APPDIR/usr/share/icons/hicolor/256x256/apps/aztek.png" "$APPDIR/aztek.png"
ln -sf aztek.png "$APPDIR/.DirIcon"
install -Dm644 "$SRC/LICENSE" "$APPDIR/usr/share/doc/aztek/LICENSE"

# The program draws with Vulkan. An OpenGL library among what it needs itself means its link line has grown one, and it
# would not start where that library is missing (libOpenGL.so.0 is not on every desktop).
if readelf -d "$APPDIR/usr/bin/aztek" | grep -E 'NEEDED.*lib(OpenGL|GLX|GL|EGL|GLESv2)\.so'; then
  echo "the program itself links an OpenGL library: see WrapOpenGL in CMakeLists.txt" >&2
  exit 1
fi
# For the log: what the AppImage expects the system to have.
echo "== taken from the system, not bundled:"
for f in "$APPDIR/usr/bin/aztek" $(find "$APPDIR/usr/plugins" "$LIB" -name '*.so*' -type f); do
  LD_LIBRARY_PATH="$LIB" ldd "$f" 2>/dev/null | awk -v own="$APPDIR" '/=> \// && index($3, own) != 1 {print $1}' || true
done | sort -u | tr '\n' ' '
echo

# Nothing inside may be left looking outside for a library that was meant to come along.
if LD_LIBRARY_PATH="$LIB" ldd "$APPDIR/usr/bin/aztek" | grep "not found"; then
  echo "unresolved libraries" >&2
  exit 1
fi

TOOL=${APPIMAGETOOL:-}
if [ -z "$TOOL" ]; then
  TOOL=$BUILD/appimagetool-x86_64.AppImage
  [ -x "$TOOL" ] || { curl -fsSL -o "$TOOL" https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage && chmod 755 "$TOOL"; }
fi
# Run unpacked: a container has no FUSE to mount the tool with.
ARCH=x86_64 "$TOOL" --appimage-extract-and-run --no-appstream "$APPDIR" "$OUT/$FILE"
chmod 755 "$OUT/$FILE"
ls -l "$OUT/$FILE"
