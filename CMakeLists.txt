cmake_minimum_required(VERSION 3.21)
project(Aztek LANGUAGES CXX)

# C++20, the VFX Reference Platform CY2027 standard.
set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
# Standard C++ as MSVC compiles it, not GNU C++.
set(CMAKE_CXX_EXTENSIONS OFF)
set(CMAKE_AUTOMOC ON)
set(CMAKE_AUTORCC ON)
if(NOT CMAKE_BUILD_TYPE AND NOT CMAKE_CONFIGURATION_TYPES)
  set(CMAKE_BUILD_TYPE Release)
endif()

find_package(Qt6 6.2 REQUIRED COMPONENTS Widgets)
# Qt's Gui target tells whatever links it to link the system's OpenGL libraries as well (libOpenGL and libGLX, through
# WrapOpenGL). Aztek draws with Vulkan and calls nothing in them, but with Qt's own kit the linker still wrote both
# into the program, --as-needed or not: Qt's library uses OpenGL through libGL, so the linker counts libOpenGL as
# wanted. The program then would not start on a system without libOpenGL.so.0, which a desktop need not have. Taken
# off the link line here; Qt's libraries bring the OpenGL they need themselves.
if(UNIX AND NOT APPLE AND TARGET WrapOpenGL::WrapOpenGL)
  set_property(TARGET WrapOpenGL::WrapOpenGL PROPERTY INTERFACE_LINK_LIBRARIES "")
endif()

# Python for aztek.cmds and the maya.cmds shim: the interpreter cargo's pyo3 builds against, and its library. Python 3.13
# only, the VFX Reference Platform CY2027 version. AZTEK_PYTHON_BUNDLE names a Python 3.13 from tools/python/fetch-python.sh, for systems
# with no 3.13 of their own: the program links it and, on Linux, installs it beside itself in lib/aztek/python.
option(AZTEK_PYTHON "Embed Python" ON)
set(AZTEK_PYTHON_BUNDLE "" CACHE PATH "A Python 3.13 from tools/python/fetch-python.sh to link and ship")
set(AZTEK_CARGO_FEATURES "")
if(AZTEK_PYTHON)
  if(AZTEK_PYTHON_BUNDLE)
    set(Python3_ROOT_DIR ${AZTEK_PYTHON_BUNDLE})
    if(WIN32)
      set(Python3_EXECUTABLE ${AZTEK_PYTHON_BUNDLE}/python.exe)
    else()
      # Its sysconfig names the folder it was built in, not this one: the library and headers are given outright.
      set(Python3_EXECUTABLE ${AZTEK_PYTHON_BUNDLE}/bin/python3.13)
      set(Python3_INCLUDE_DIR ${AZTEK_PYTHON_BUNDLE}/include/python3.13)
      if(APPLE)
        set(Python3_LIBRARY ${AZTEK_PYTHON_BUNDLE}/lib/libpython3.13.dylib)
      else()
        set(Python3_LIBRARY ${AZTEK_PYTHON_BUNDLE}/lib/libpython3.13.so)
      endif()
    endif()
  endif()
  find_package(Python3 3.13 EXACT COMPONENTS Interpreter Development.Embed)
  if(NOT Python3_FOUND)
    message(FATAL_ERROR "Python 3.13 with its development files is required. ./build.sh --deps (or build.ps1) installs or "
                        "fetches it; tools/python/fetch-python.sh <folder> then -DAZTEK_PYTHON_BUNDLE=<folder> uses a "
                        "downloaded one; -DAZTEK_PYTHON=OFF builds without Python.")
  endif()
endif()
if(NOT AZTEK_PYTHON)
  set(AZTEK_CARGO_FEATURES --no-default-features)
endif()

# The Rust core, built by cargo as a static library. Release unless this is a Debug build.
find_program(CARGO cargo REQUIRED)
set(CARGO_TARGET_DIR ${CMAKE_BINARY_DIR}/cargo)
set(CARGO_PROFILE "$<IF:$<CONFIG:Debug>,debug,release>")
set(CARGO_FLAG "$<$<NOT:$<CONFIG:Debug>>:--release>")
if(WIN32)
  set(AZTEK_HOST_LIB ${CARGO_TARGET_DIR}/${CARGO_PROFILE}/aztek_host.lib)
else()
  set(AZTEK_HOST_LIB ${CARGO_TARGET_DIR}/${CARGO_PROFILE}/libaztek_host.a)
endif()
add_custom_target(aztek_host_build
  COMMAND ${CMAKE_COMMAND} -E env CARGO_TARGET_DIR=${CARGO_TARGET_DIR} PYO3_PYTHON=${Python3_EXECUTABLE}
          ${CARGO} build -p aztek_host ${CARGO_FLAG} ${AZTEK_CARGO_FEATURES}
  WORKING_DIRECTORY ${CMAKE_SOURCE_DIR}
  BYPRODUCTS ${AZTEK_HOST_LIB}
  USES_TERMINAL
  COMMENT "cargo build -p aztek_host")

# harold, Harold's command-line renderer: renders .hss scene files written by haroldExportHss. Built beside aztek.
if(WIN32)
  set(HAROLD_BIN ${CARGO_TARGET_DIR}/${CARGO_PROFILE}/harold.exe)
  set(HAROLD_OUT ${CMAKE_BINARY_DIR}/harold.exe)
else()
  set(HAROLD_BIN ${CARGO_TARGET_DIR}/${CARGO_PROFILE}/harold)
  set(HAROLD_OUT ${CMAKE_BINARY_DIR}/harold)
endif()
add_custom_target(harold_program ALL
  COMMAND ${CMAKE_COMMAND} -E env CARGO_TARGET_DIR=${CARGO_TARGET_DIR}
          ${CARGO} build -p aztek_render --bin harold ${CARGO_FLAG}
  COMMAND ${CMAKE_COMMAND} -E copy_if_different ${HAROLD_BIN} ${HAROLD_OUT}
  WORKING_DIRECTORY ${CMAKE_SOURCE_DIR}
  BYPRODUCTS ${HAROLD_OUT}
  USES_TERMINAL
  COMMENT "cargo build -p aztek_render --bin harold")
add_dependencies(harold_program aztek_host_build)

# aztekpy, the reference's mayapy: a console program that runs the aztek beside it as `aztek --python`.
add_executable(aztekpy app/src/aztekpy.cpp)

add_executable(aztek WIN32
  app/src/main.cpp
  app/src/host.cpp
  app/src/icons.cpp
  app/src/sculpt_icons.cpp
  app/src/viewport.cpp
  app/src/panels.cpp
  app/src/animlayers.cpp
  app/src/hikwindow.cpp
  app/src/widgets.cpp
  app/src/toolsettings_xform.cpp
  app/src/markingmenu.cpp
  app/src/mainwindow.cpp
  app/src/grapheditor.cpp
  app/src/uveditor.cpp
  app/src/referenceeditor.cpp
  app/src/usdui.cpp
  app/src/fbxui.cpp
  app/src/fileoptions.cpp
  app/src/materials.cpp
  app/src/userfolder.cpp
  app/src/rendernodes.cpp
  app/src/hypershade.cpp
  app/src/toolsettings.cpp
  app/src/dopesheet.cpp
  app/src/nodeeditor.cpp
  app/src/shapeeditor.cpp
  app/src/relationship.cpp
  app/src/welcome.cpp
  app/src/modelingtoolkit.cpp
  app/src/uvtoolkit.cpp
  app/src/runtime.cpp
  app/src/statusline.cpp
  app/src/themes.cpp
  app/src/theme_editor.cpp
  app/src/tearoff.cpp
  app/src/hotbox.cpp
  app/src/shelves.cpp
  app/src/panelmenus.cpp
  app/src/appmenus.cpp
  app/src/dynmenus.cpp
  app/src/prefswindow.cpp
  app/src/pluginmanager.cpp
  app/src/feedback.cpp
  app/src/external.cpp
  app/src/mgearinstall.cpp
  app/src/editors.cpp
  app/src/attreditor.cpp
  app/src/colorchooser.cpp
  app/src/scripteditor.cpp
  app/src/completion.cpp
  app/src/updates.cpp
  app/src/outlinerpanel.cpp
  app/src/icons_more.cpp
  app/src/icons_render.cpp
  app/src/renderview.cpp
  app/src/rendersettings.cpp
  app/src/scriptui.cpp
  app/src/resources.qrc
  $<$<PLATFORM_ID:Windows>:app/icons/aztek.rc>)
add_dependencies(aztek aztek_host_build)
if(APPLE)
  # Aztek.app: macdeployqt adds Qt, the release build adds MoltenVK and the icon.
  set_target_properties(aztek PROPERTIES MACOSX_BUNDLE TRUE
    MACOSX_BUNDLE_BUNDLE_NAME Aztek
    MACOSX_BUNDLE_GUI_IDENTIFIER io.github.srlegrand.aztek
    MACOSX_BUNDLE_ICON_FILE aztek.icns)
endif()
file(STRINGS ${CMAKE_SOURCE_DIR}/Cargo.toml AZTEK_VERSION_LINE REGEX "^version *= *\"")
string(REGEX REPLACE "^version *= *\"([^\"]*)\".*" "\\1" AZTEK_VERSION "${AZTEK_VERSION_LINE}")
# What a user reads: 2.N and the commit it was built from, so any build traces back to its change. CI writes 2.N into
# Cargo.toml before building; a local build from a git checkout works out the same number itself, so a build of a
# commit says the same as the release of that commit. Without git (a source tarball) it stays at Cargo.toml's version.
if(AZTEK_VERSION STREQUAL "1.0.0")
  execute_process(COMMAND git rev-list --count HEAD WORKING_DIRECTORY ${CMAKE_SOURCE_DIR}
                  OUTPUT_VARIABLE AZTEK_COUNT OUTPUT_STRIP_TRAILING_WHITESPACE ERROR_QUIET RESULT_VARIABLE AZTEK_COUNT_FAILED)
  if(NOT AZTEK_COUNT_FAILED AND AZTEK_COUNT MATCHES "^[0-9]+$")
    set(AZTEK_VERSION "2.${AZTEK_COUNT}.0")
  endif()
endif()
# A new commit or a checkout runs the configure step again, so the number and the commit stay current.
if(EXISTS "${CMAKE_SOURCE_DIR}/.git/HEAD")
  set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${CMAKE_SOURCE_DIR}/.git/HEAD")
  file(READ "${CMAKE_SOURCE_DIR}/.git/HEAD" AZTEK_GIT_HEAD)
  if(AZTEK_GIT_HEAD MATCHES "^ref: ([^\n]+)")
    string(STRIP "${CMAKE_MATCH_1}" AZTEK_GIT_REF)
    foreach(f "${AZTEK_GIT_REF}" "packed-refs")
      if(EXISTS "${CMAKE_SOURCE_DIR}/.git/${f}")
        set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${CMAKE_SOURCE_DIR}/.git/${f}")
      endif()
    endforeach()
  endif()
endif()
set(AZTEK_COMMIT "$ENV{GITHUB_SHA}")
if(NOT AZTEK_COMMIT)
  execute_process(COMMAND git rev-parse HEAD WORKING_DIRECTORY ${CMAKE_SOURCE_DIR}
                  OUTPUT_VARIABLE AZTEK_COMMIT OUTPUT_STRIP_TRAILING_WHITESPACE ERROR_QUIET)
endif()
string(REGEX REPLACE "\\.0$" "" AZTEK_SHOWN "${AZTEK_VERSION}")
if(AZTEK_COMMIT)
  string(SUBSTRING "${AZTEK_COMMIT}" 0 7 AZTEK_COMMIT)
  set(AZTEK_SHOWN "${AZTEK_SHOWN} (${AZTEK_COMMIT})")
endif()
target_compile_definitions(aztek PRIVATE AZTEK_VERSION="${AZTEK_SHOWN}")
# Builds made by CI are the released ones: they offer to install a newer release over themselves. Others only say so.
if(DEFINED ENV{GITHUB_ACTIONS})
  target_compile_definitions(aztek PRIVATE AZTEK_RELEASE_BUILD=1)
endif()
target_include_directories(aztek PRIVATE app/src)
target_link_libraries(aztek PRIVATE Qt6::Widgets ${AZTEK_HOST_LIB})
if(AZTEK_PYTHON)
  target_link_libraries(aztek PRIVATE Python3::Python)
  target_compile_definitions(aztek PRIVATE AZTEK_HAS_PYTHON=1)
  if(AZTEK_PYTHON_BUNDLE AND UNIX AND NOT APPLE)
    # The bundled library: from its own folder in the build (or from python/lib beside the program, as the Linux tar.gz
    # carries it), from lib/aztek/python/lib once installed.
    set_target_properties(aztek PROPERTIES BUILD_RPATH "${AZTEK_PYTHON_BUNDLE}/lib;$ORIGIN/python/lib"
                          INSTALL_RPATH "$ORIGIN/../lib/aztek/python/lib")
  endif()
endif()
if(MSVC)
  target_compile_options(aztek PRIVATE /utf-8 /permissive-)
  target_compile_definitions(aztek PRIVATE NOMINMAX WIN32_LEAN_AND_MEAN)
elseif(CMAKE_CXX_COMPILER_ID MATCHES "GNU|Clang")
  # Plain C++20 and nothing more, as MSVC holds the shell to it: a later standard's syntax or a compiler extension fails
  # the Linux and macOS builds, and a build here, instead of only the Windows one.
  target_compile_options(aztek PRIVATE -pedantic-errors)
endif()
if(WIN32)
  # What the Rust standard library and its crates link on Windows (cargo rustc --print native-static-libs).
  target_link_libraries(aztek PRIVATE ws2_32 userenv bcrypt ntdll advapi32 user32 kernel32 ole32 oleaut32 shell32
                        synchronization dbghelp psapi pdh powrprof iphlpapi netapi32 secur32 crypt32 ncrypt
                        wbemuuid propsys uuid winmm gdi32 comdlg32 version runtimeobject)
elseif(APPLE)
  # What the Rust standard library, ash-window's Metal layer and the other crates link on macOS.
  target_link_libraries(aztek PRIVATE "-framework Foundation" "-framework CoreFoundation" "-framework AppKit"
                        "-framework QuartzCore" "-framework Metal" "-framework CoreGraphics" "-framework IOKit"
                        "-framework Security" "-framework SystemConfiguration" objc iconv dl pthread m)
else()
  target_link_libraries(aztek PRIVATE dl pthread m)
  # Only the libraries the program calls go into it.
  target_link_options(aztek PRIVATE "LINKER:--as-needed")
endif()

# Install: the binary, and on Linux its desktop entry and icon (cmake --install build --prefix ~/.local).
install(TARGETS aztek RUNTIME DESTINATION bin BUNDLE DESTINATION .)
if(NOT APPLE)
  install(TARGETS aztekpy RUNTIME DESTINATION bin)
endif()
if(APPLE)
  install(PROGRAMS ${HAROLD_OUT} DESTINATION aztek.app/Contents/MacOS)
else()
  install(PROGRAMS ${HAROLD_OUT} DESTINATION bin)
endif()
# The bundled Python: its library, standard library and headers (for pip's builds), without Tcl/Tk and IDLE.
if(AZTEK_PYTHON AND AZTEK_PYTHON_BUNDLE AND UNIX AND NOT APPLE)
  install(DIRECTORY ${AZTEK_PYTHON_BUNDLE}/lib ${AZTEK_PYTHON_BUNDLE}/include DESTINATION lib/aztek/python
          USE_SOURCE_PERMISSIONS
          PATTERN "__pycache__" EXCLUDE PATTERN "libtcl*" EXCLUDE PATTERN "libtk*" EXCLUDE PATTERN "tcl9*" EXCLUDE
          PATTERN "tk9*" EXCLUDE PATTERN "itcl*" EXCLUDE PATTERN "thread3*" EXCLUDE PATTERN "pkgconfig" EXCLUDE
          PATTERN "tkinter" EXCLUDE PATTERN "idlelib" EXCLUDE PATTERN "turtledemo" EXCLUDE
          PATTERN "_tkinter*" EXCLUDE PATTERN "config-3.13-*" EXCLUDE)
endif()
if(UNIX AND NOT APPLE)
  install(FILES app/aztek.desktop DESTINATION share/applications)
  install(FILES app/icons/aztek-icon.svg DESTINATION share/icons/hicolor/scalable/apps RENAME aztek.svg)
  install(FILES app/icons/aztek-icon.png DESTINATION share/icons/hicolor/512x512/apps RENAME aztek.png)
  foreach(size 16 24 32 48 64 128 256)
    install(FILES app/icons/hicolor/${size}.png DESTINATION share/icons/hicolor/${size}x${size}/apps RENAME aztek.png)
  endforeach()
  if(EXISTS ${CMAKE_SOURCE_DIR}/examples)
    install(DIRECTORY examples/ DESTINATION share/aztek/examples)
  endif()
  # The global MEL procedures that come with Aztek, to read: the program has them built in (scripts/README.md).
  install(DIRECTORY scripts/ DESTINATION share/aztek/scripts PATTERN "README.md" EXCLUDE)
endif()
