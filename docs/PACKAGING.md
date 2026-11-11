# Packaging

Everything is built by `.github/workflows/build.yml` and attached to the `latest` pre-release (each push to main) and to
versioned releases (`v*` tags). The version comes from `Cargo.toml`, `[workspace.package]`. The scripts below also run
by hand.

| Package | Built on | Script | Qt | Python |
| --- | --- | --- | --- | --- |
| `aztek-<version>-1.el8.x86_64.rpm` | `rockylinux:8` container | `packaging/rpm/build-rpm.sh` | 6.8 bundled | 3.12 from AppStream |
| `aztek-<version>-1.el9.x86_64.rpm` | `rockylinux:9` container | `packaging/rpm/build-rpm.sh` | EPEL | 3.12 from AppStream |
| `aztek_<version>-1_ubuntu24.04_amd64.deb` | ubuntu-24.04 | `packaging/deb/build-deb.sh` | system | system 3.12 |
| `aztek_<version>-1_ubuntu22.04_amd64.deb` | `ubuntu:22.04` container | `packaging/deb/build-deb.sh` | system 6.2 | system 3.10 |
| `aztek-<version>-setup-x64.exe` | windows-2022 | `packaging/windows/build-installer.ps1` | 6.8 bundled | 3.12 bundled |
| `aztek-windows-x86_64.zip` | windows-2022 | `build.ps1` | 6.8 bundled | 3.12 bundled |
| `aztek-linux-x86_64.tar.gz` | ubuntu-24.04 | `build.sh` | system | system |
| `aztek-<version>-x86_64.AppImage` | `rockylinux:8` container | `packaging/appimage/build-appimage.sh` | 6.8 bundled | 3.12 bundled |

## Rocky Linux

Rocky 8 has no Qt 6 in AppStream, PowerTools or EPEL, so `packaging/linux/install-qt.sh` downloads Qt's own kit with
aqtinstall, and `packaging/linux/bundle-qt.sh` copies what the binary and its plugins need into `/usr/lib64/aztek`, with
`libxcb-cursor`, which Rocky 8 only has in EPEL. The package installs from the base repositories alone. The binary has an
RPATH to that folder rather than a RUNPATH, so a Qt on a studio's `LD_LIBRARY_PATH` does not replace it. It builds with
`gcc-toolset-13` because Qt's headers want more than gcc 8.

Rocky 9 needs EPEL enabled at install, for `qt6-qtbase-gui` and `libb2`. The package requires the Qt version it was built
against or newer (`libQt6Core.so.6(Qt_6.10)` at the moment), so a mirror frozen on an older EPEL will refuse it.

    dnf install epel-release            # Rocky 9 only
    dnf install ./aztek-*.rpm

Both use `python3.12` rather than the system Python (3.6 on Rocky 8, which pyo3 does not support, and 3.9 on Rocky 9).
It needs AppStream from 8.10 or 9.4 on.

To build by hand, as root in a container, with the checkout mounted:

    packaging/rpm/install-build-deps.sh
    curl https://sh.rustup.rs -sSf | sh -s -- -y --profile minimal && . ~/.cargo/env
    packaging/rpm/build-rpm.sh build dist

## Vulkan at run time

The viewport loads `libvulkan.so.1` with dlopen, so it is declared by hand: `vulkan-loader` and `libvulkan1`. It also
needs a driver:

- NVIDIA workstations: the proprietary driver has its own Vulkan driver (`nvidia_icd.json`). Nothing else is needed.
  Aztek picks a discrete GPU over Mesa's when both are there.
- `mesa-vulkan-drivers` is a weak dependency, for AMD, Intel and software rendering. It is harmless beside NVIDIA's.
- Rocky 8's Mesa software driver (lavapipe) fails to start, seen in `vulkaninfo` as well as Aztek. Without a GPU on Rocky
  8 the viewport is black with a Vulkan error in the status line; the rest of the application works.
- The NVIDIA driver crashes in `libnvidia-eglcore` when asked for a surface on an X server without GLX, which takes
  Aztek with it. Seen on Rocky 8's Xvfb, and a VNC server without GLX should behave the same. Either use a server with
  GLX, or select Mesa with `VK_DRIVER_FILES=/usr/share/vulkan/icd.d/lvp_icd.x86_64.json` where lavapipe works.
- On Ubuntu 22.04, Mesa's software driver under Xvfb draws the viewport very coarsely. The cause is not known. The same
  package with the NVIDIA driver on a real X server draws properly.

Aztek runs through X11 only (`QT_QPA_PLATFORM=xcb`, XWayland on a Wayland session).

## AppImage

    chmod +x aztek-*-x86_64.AppImage
    ./aztek-*-x86_64.AppImage

For any other x86_64 Linux, and Ubuntu releases the `.deb` files aren't built for. One file with Qt, Python and its
standard library, and the libraries a distribution may not have, inside; built on Rocky 8 so that its C library
(glibc 2.28) is the newest it needs. What it takes from the system is what has to match the machine or every desktop
has: the C library, GL and Vulkan, X11, fontconfig and freetype, glib and D-Bus (the list is in the script, after the
AppImage project's excludelist). It needs FUSE to mount itself; without it, `--appimage-extract-and-run` unpacks and
runs. Help > Check for Updates replaces the file with the new one.

CI starts it unpacked in clean `ubuntu:24.04` and `ubuntu:rolling` containers (`verify-appimage`). A build or check
that fails leaves that release without an AppImage rather than holding the release back.

The program itself must not need an OpenGL library. Qt's `Gui` target puts `libOpenGL` and `libGLX` on the link line
of whatever links it, and with Qt's own kit the linker kept both in the program even with `--as-needed`, so it would
not start in the clean containers, which have no `libOpenGL.so.0`. `CMakeLists.txt` takes them off the link line, and
`build-appimage.sh` stops if the program needs one again. Its log lists what the AppImage takes from the system.

## Ubuntu

    sudo apt install ./aztek_*.deb

Depends come from `dpkg-shlibdeps`, plus what it cannot see from `packaging/deb/control.in`: `libvulkan1`,
`qt6-qpa-plugins` and `hicolor-icon-theme`.

## Windows

`build.ps1` makes `build\dist`, which runs on a machine with nothing else installed: Qt from `windeployqt`, the VC runtime
copied beside the exe, Python's DLL, its standard library in `Lib` and its extension modules in `DLLs`. CMake is told to
use the Python on PATH, so the exe links the one that is copied.

`packaging/windows/aztek.iss` is the Inno Setup script: Start Menu entry, uninstaller, and two optional tasks, off by
default, for a desktop shortcut and for opening `.ma` files with Aztek. It installs for all users or the current one.

The installer is not signed, so SmartScreen warns about it until someone signs it with a code signing certificate.

Inno Setup's compiler prints "Non-commercial use only". Commercial use of Aztek's installer build needs a licence from
its author, or a move to NSIS.

## Checks

`packaging/linux/smoke-test.sh` and `packaging/windows/smoke-test.ps1` run in the workflow on the installed package, in a
clean container on Linux: libraries resolve, the application starts, a Python script runs through `aztek.cmds`, and a
screenshot is saved. A failure stops the release. They do not check that the viewport drew, since the runners have no
GPU.

## Examples

`examples/` in the repository, if it exists, is installed to `/usr/share/aztek/examples` on Linux and `examples` beside
the exe on Windows.

## macOS

Early. The `macos` job builds `aztek-macos-arm64.zip` on a `macos-15` runner (Apple silicon), for macOS 14 and later. It isn't a condition of a
release yet: when it fails, the other packages still go out.

- `Aztek.app` carries Qt (`macdeployqt`) and `libMoltenVK.dylib` in `Contents/Frameworks`, with an ad-hoc signature.
  It isn't notarized: the first start is a right-click and Open.
- Python is Homebrew's `python@3.12`, linked from `/opt/homebrew`: `brew install python@3.12` before starting it.
- `gpu.rs` loads MoltenVK from the bundle first, then a Vulkan loader on the library path, then Homebrew's folders.
  The viewport draws through `AZTEK_APPKIT`: the widget's `NSView`, with a Metal layer from `ash_window`.
- `./build-macos.sh` builds it (`--deps` installs the dependencies with Homebrew); `--package` makes the same `Aztek.app` and zip as the release build, which runs that script.
- Not checked yet: the viewport on a real Mac, and whether Ctrl and Alt in mouse and key handling should be Cmd and Option.
