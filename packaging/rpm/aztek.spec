# Built by packaging/rpm/build-rpm.sh, which passes aztek_src (the checkout), aztek_build (the CMake build folder),
# aztek_version and dist. This builds from the checkout rather than a source tarball, so a build already done in that
# folder is reused.
#
# --with bundled_qt: Qt from aztek_qt (a kit from aqtinstall) goes into the private folder _libdir/aztek. Rocky 8 needs
# this: none of its repositories has Qt 6. Rocky 9 uses EPEL's Qt.

%bcond_with bundled_qt

%global debug_package %{nil}
# No /usr/lib/.build-id links: a bundled Qt library would clash with the same library in any other package.
%global _build_id_links none
%global python_bin /usr/bin/python3.12

%if %{with bundled_qt}
# The bundled libraries are private: nothing else may resolve against them, and rpm must not look for them elsewhere.
%global __provides_exclude_from ^%{_libdir}/aztek/.*$
%global __requires_exclude ^(libQt6|libicu(data|i18n|uc)\\.so\\.[0-9]+\\(|libxcb-cursor\\.so)
%endif

Name:           aztek
Version:        %{aztek_version}
Release:        1%{?dist}
Summary:        Modelling, UV, rigging and animation application
License:        MIT-0
URL:            https://github.com/srlegrand/AKA-Crest-Aztek
ExclusiveArch:  x86_64

BuildRequires:  cmake >= 3.21
BuildRequires:  ninja-build
BuildRequires:  gcc-c++
BuildRequires:  python3.12-devel
BuildRequires:  vulkan-loader-devel
BuildRequires:  desktop-file-utils
%if %{without bundled_qt}
BuildRequires:  qt6-qtbase-devel
Requires:       qt6-qtbase-gui
%endif

# The viewport loads libvulkan.so.1 with dlopen, so rpm's scan of the binary cannot see it.
Requires:       vulkan-loader
Requires:       hicolor-icon-theme
# A Vulkan driver is what draws. NVIDIA's own driver brings one, and is preferred over Mesa's when both are there.
Recommends:     mesa-vulkan-drivers

%description
Aztek is a modelling, UV, rigging and animation application, with MEL and Python scripting and .ma files.
The viewport needs a Vulkan driver: the NVIDIA driver's own, or Mesa's.

%build
cd %{aztek_src}
%if %{with bundled_qt}
QT_ARGS='-DCMAKE_PREFIX_PATH=%{aztek_qt} -DCMAKE_INSTALL_RPATH=$ORIGIN/../%{_lib}/aztek/lib -DCMAKE_EXE_LINKER_FLAGS=-Wl,--disable-new-dtags'
%else
QT_ARGS=
%endif
cmake -S . -B %{aztek_build} -G Ninja -DCMAKE_BUILD_TYPE=Release -DCMAKE_INSTALL_PREFIX=%{_prefix} \
  -DPython3_EXECUTABLE=%{python_bin} $QT_ARGS
cmake --build %{aztek_build}

%install
DESTDIR=%{buildroot} cmake --install %{aztek_build}
install -d %{buildroot}%{_datadir}/aztek/examples
%if %{with bundled_qt}
%{aztek_src}/packaging/linux/bundle-qt.sh %{aztek_qt} %{buildroot}%{_bindir}/aztek %{buildroot}%{_libdir}/aztek
%endif
desktop-file-validate %{buildroot}%{_datadir}/applications/aztek.desktop
install -Dm644 %{aztek_src}/LICENSE %{buildroot}%{_datadir}/licenses/aztek/LICENSE

%files
%{_datadir}/licenses/aztek/LICENSE
%{_bindir}/aztek
%{_datadir}/applications/aztek.desktop
%{_datadir}/icons/hicolor/*/apps/aztek.*
%{_datadir}/aztek
# The plug-ins that ship with Aztek (plugins/CMakeLists.txt installs them to lib/aztek/plug-ins).
%{_prefix}/lib/aztek
%if %{with bundled_qt}
%{_libdir}/aztek
%endif
