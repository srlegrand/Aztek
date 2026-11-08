#!/usr/bin/env bash
# Builds Aztek on Linux and macOS: ./build.sh [--deps] [--debug] [--run]
#   --deps   install the build dependencies first (apt, dnf, pacman, or Homebrew on macOS)
#   --debug  debug build in build-debug/
#   --run    start Aztek when the build is done
#   --install  install to ~/.local: the binary, a menu entry and the icon
set -euo pipefail
cd "$(dirname "$0")"

DEPS=0 RUN=0 INSTALL=0 TYPE=Release DIR=build
for a in "$@"; do
  case "$a" in
    --deps) DEPS=1 ;;
    --debug) TYPE=Debug DIR=build-debug ;;
    --run) RUN=1 ;;
    --install) INSTALL=1 ;;
    *) echo "unknown option $a"; exit 1 ;;
  esac
done

MAC=0
[ "$(uname -s)" = Darwin ] && MAC=1

if [ "$DEPS" = 1 ]; then
  # macOS first: its /usr/bin/apt is a stub for an old Java tool, not a package manager.
  if [ "$MAC" = 1 ]; then
    command -v brew >/dev/null || { echo "Install Homebrew first: https://brew.sh"; exit 1; }
    brew install cmake ninja qt python@3.12 molten-vk
  elif command -v apt >/dev/null; then
    sudo apt install -y build-essential cmake ninja-build qt6-base-dev libvulkan-dev mesa-vulkan-drivers python3-dev
  elif command -v dnf >/dev/null; then
    sudo dnf install -y gcc-c++ cmake ninja-build qt6-qtbase-devel vulkan-loader-devel mesa-vulkan-drivers python3-devel
  elif command -v pacman >/dev/null; then
    sudo pacman -S --needed base-devel cmake ninja qt6-base vulkan-icd-loader python
  else
    echo "Install by hand: a C++17 compiler, CMake 3.21+, Ninja, Qt 6.2+ (base, dev), Vulkan loader, Python 3 dev files."
  fi
fi

if ! command -v cargo >/dev/null; then
  echo "Rust is missing: curl https://sh.rustup.rs -sSf | sh"
  exit 1
fi
for t in cmake ninja; do
  command -v "$t" >/dev/null || { echo "$t is missing (./build.sh --deps installs it)"; exit 1; }
done

# Conda's Python and libraries shadow the system's: build against the system Python.
EXTRA=()
if [ "$MAC" = 1 ] && command -v brew >/dev/null; then
  # Homebrew's Python and Qt, not Xcode's Python or a conda one.
  PY="$(brew --prefix python@3.12 2>/dev/null)/bin/python3.12"
  [ -x "$PY" ] && EXTRA+=(-DPython3_EXECUTABLE="$PY")
  QTP="$(brew --prefix qt 2>/dev/null)"
  [ -d "$QTP" ] && EXTRA+=(-DCMAKE_PREFIX_PATH="$QTP")
  [ -n "${CONDA_PREFIX:-}" ] && EXTRA+=(-DCMAKE_IGNORE_PREFIX_PATH="$CONDA_PREFIX")
elif [ -n "${CONDA_PREFIX:-}" ] && [ -x /usr/bin/python3 ]; then
  EXTRA+=(-DPython3_EXECUTABLE=/usr/bin/python3 -DCMAKE_IGNORE_PREFIX_PATH="$CONDA_PREFIX")
fi

cmake -B "$DIR" -G Ninja -DCMAKE_BUILD_TYPE="$TYPE" "${EXTRA[@]}"
cmake --build "$DIR"
BIN="$DIR/aztek"
[ "$MAC" = 1 ] && BIN="$DIR/aztek.app/Contents/MacOS/aztek"
echo "Built $BIN"

if [ "$INSTALL" = 1 ]; then
  cmake --install "$DIR" --prefix "$HOME/.local"
  command -v update-desktop-database >/dev/null && update-desktop-database "$HOME/.local/share/applications" || true
  echo "Installed to $HOME/.local (bin/aztek, menu entry, icon)"
fi

if [ "$RUN" = 1 ]; then
  exec "$BIN"
fi
