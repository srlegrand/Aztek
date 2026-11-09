#!/usr/bin/env bash
# Builds Aztek on macOS: ./build-macos.sh [--deps] [--debug] [--run] [--package] [--install]
#   --deps     install the build dependencies with Homebrew first
#   --debug    debug build in build-debug/
#   --run      start Aztek when the build is done
#   --package  make a self-contained Aztek.app (Qt and MoltenVK inside, signed for this Mac) and
#              dist/aztek-macos-<arch>.zip with the examples, as the release build does
#   --install  copy Aztek.app to ~/Applications (implies --package)
# Qt comes from Homebrew, or from QT_ROOT_DIR when that's set (the release build's Qt).
# Python is Homebrew's python@3.12: the app links it from /opt/homebrew (or /usr/local on Intel Macs).
set -euo pipefail
cd "$(dirname "$0")"

[ "$(uname -s)" = Darwin ] || { echo "This is the macOS build; on Linux use ./build.sh"; exit 1; }

DEPS=0 RUN=0 PACKAGE=0 INSTALL=0 TYPE=Release DIR=build
for a in "$@"; do
  case "$a" in
    --deps) DEPS=1 ;;
    --debug) TYPE=Debug DIR=build-debug ;;
    --run) RUN=1 ;;
    --package) PACKAGE=1 ;;
    --install) INSTALL=1 PACKAGE=1 ;;
    *) echo "unknown option $a"; exit 1 ;;
  esac
done

command -v brew >/dev/null || { echo "Install Homebrew first: https://brew.sh"; exit 1; }
if [ "$DEPS" = 1 ]; then
  xcode-select -p >/dev/null 2>&1 || { echo "Installing Xcode's command line tools; run this again when they're in."; xcode-select --install; exit 1; }
  brew install cmake ninja python@3.12 molten-vk
  [ -n "${QT_ROOT_DIR:-}" ] || brew install qt
  [ -f "$HOME/.cargo/env" ] || command -v cargo >/dev/null || curl https://sh.rustup.rs -sSf | sh -s -- -y --no-modify-path
fi

# Cargo from rustup's default place, for shells that haven't read ~/.cargo/env yet.
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
command -v cargo >/dev/null || { echo "Rust is missing: ./build-macos.sh --deps, or curl https://sh.rustup.rs -sSf | sh"; exit 1; }
for t in cmake ninja; do
  command -v "$t" >/dev/null || { echo "$t is missing (./build-macos.sh --deps installs it)"; exit 1; }
done

PY="$(brew --prefix python@3.12 2>/dev/null || true)/bin/python3.12"
[ -x "$PY" ] || { echo "Homebrew's python@3.12 is missing (./build-macos.sh --deps installs it)"; exit 1; }
QT="${QT_ROOT_DIR:-$(brew --prefix qt 2>/dev/null || true)}"
[ -d "$QT/lib/cmake/Qt6" ] || { echo "Qt 6 is missing (./build-macos.sh --deps installs it, or set QT_ROOT_DIR)"; exit 1; }
MVK="$(brew --prefix molten-vk 2>/dev/null || true)/lib/libMoltenVK.dylib"
[ -f "$MVK" ] || { echo "MoltenVK is missing (./build-macos.sh --deps installs it)"; exit 1; }

EXTRA=(-DPython3_EXECUTABLE="$PY" -DCMAKE_PREFIX_PATH="$QT")
# Conda's libraries would shadow Homebrew's.
[ -n "${CONDA_PREFIX:-}" ] && EXTRA+=(-DCMAKE_IGNORE_PREFIX_PATH="$CONDA_PREFIX")
export PYO3_PYTHON="$PY"

cmake -B "$DIR" -G Ninja -DCMAKE_BUILD_TYPE="$TYPE" "${EXTRA[@]}"
cmake --build "$DIR"
APP="$DIR/aztek.app"
echo "Built $APP"

if [ "$PACKAGE" = 1 ]; then
  # MoltenVK inside, where the app looks first; then the icon; then Qt; then an ad-hoc signature, without which
  # Apple silicon won't start it. The signature has to come last: everything before it changes the bundle.
  mkdir -p "$APP/Contents/Frameworks" "$APP/Contents/Resources"
  cp "$MVK" "$APP/Contents/Frameworks/"
  ICONS="$DIR/aztek.iconset"
  rm -rf "$ICONS" && mkdir -p "$ICONS"
  for s in 16 32 128 256 512; do
    sips -z $s $s app/icons/aztek-icon.png --out "$ICONS/icon_${s}x${s}.png" >/dev/null
    sips -z $((s * 2)) $((s * 2)) app/icons/aztek-icon.png --out "$ICONS/icon_${s}x${s}@2x.png" >/dev/null
  done
  iconutil -c icns "$ICONS" -o "$APP/Contents/Resources/aztek.icns"
  "$QT/bin/macdeployqt" "$APP"
  codesign --force --deep --sign - "$APP"
  codesign --verify --deep --strict "$APP"

  ARCH="$(uname -m)"
  rm -rf dist/Aztek && mkdir -p dist/Aztek
  cp -R "$APP" dist/Aztek/Aztek.app
  cp -R examples dist/Aztek/examples
  rm -f "dist/aztek-macos-$ARCH.zip"
  ditto -c -k --keepParent dist/Aztek "dist/aztek-macos-$ARCH.zip"
  echo "Packaged dist/Aztek/Aztek.app and dist/aztek-macos-$ARCH.zip"
fi

if [ "$INSTALL" = 1 ]; then
  mkdir -p "$HOME/Applications"
  rm -rf "$HOME/Applications/Aztek.app"
  cp -R dist/Aztek/Aztek.app "$HOME/Applications/Aztek.app"
  echo "Installed to $HOME/Applications/Aztek.app"
fi

if [ "$RUN" = 1 ]; then
  if [ "$INSTALL" = 1 ]; then
    open "$HOME/Applications/Aztek.app"
  else
    exec "$APP/Contents/MacOS/aztek"
  fi
fi
