cmake_minimum_required(VERSION 3.21)
project(Aztek LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
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

# Python for aztek.cmds and the maya.cmds shim: the interpreter cargo's pyo3 builds against, and its library.
option(AZTEK_PYTHON "Embed Python" ON)
set(AZTEK_CARGO_FEATURES "")
if(AZTEK_PYTHON)
  find_package(Python3 COMPONENTS Interpreter Development.Embed)
  if(NOT Python3_FOUND)
    message(WARNING "Python 3 development files not found: building without Python")
    set(AZTEK_PYTHON OFF)
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
  app/src/runtime.cpp
  app/src/statusline.cpp
  app/src/hotbox.cpp
  app/src/shelves.cpp
  app/src/panelmenus.cpp
  app/src/appmenus.cpp
  app/src/dynmenus.cpp
  app/src/prefswindow.cpp
  app/src/pluginmanager.cpp
  app/src/feedback.cpp
  app/src/editors.cpp
  app/src/attreditor.cpp
  app/src/colorchooser.cpp
  app/src/scripteditor.cpp
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
endif()
if(MSVC)
  target_compile_options(aztek PRIVATE /utf-8 /permissive-)
  target_compile_definitions(aztek PRIVATE NOMINMAX WIN32_LEAN_AND_MEAN)
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
endif()
