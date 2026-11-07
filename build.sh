#!/usr/bin/env bash
# Builds Aztek on Linux: ./build.sh [--deps] [--debug] [--run]
#   --deps   install the build dependencies first (apt, dnf or pacman)
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

if [ "$DEPS" = 1 ]; then
  if command -v apt >/dev/null; then
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
if [ -n "${CONDA_PREFIX:-}" ] && [ -x /usr/bin/python3 ]; then
  EXTRA+=(-DPython3_EXECUTABLE=/usr/bin/python3 -DCMAKE_IGNORE_PREFIX_PATH="$CONDA_PREFIX")
fi

cmake -B "$DIR" -G Ninja -DCMAKE_BUILD_TYPE="$TYPE" "${EXTRA[@]}"
cmake --build "$DIR"
echo "Built $DIR/aztek"

if [ "$INSTALL" = 1 ]; then
  cmake --install "$DIR" --prefix "$HOME/.local"
  command -v update-desktop-database >/dev/null && update-desktop-database "$HOME/.local/share/applications" || true
  echo "Installed to $HOME/.local (bin/aztek, menu entry, icon)"
fi

if [ "$RUN" = 1 ]; then
  exec "$DIR/aztek"
fi
