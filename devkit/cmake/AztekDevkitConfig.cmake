# Aztek plug-in kit for CMake.
#
#   find_package(AztekDevkit REQUIRED)
#   aztek_add_plugin(myPlugin SOURCES a.cpp b.cpp [INCLUDE_DIRS dir...] [LIBRARIES lib...] [DEFINITIONS d...])
#
# The kit's classes are built from source into a static library with the plug-in's own compiler and flags, so nothing
# of the kit's needs to match a compiler or runtime Aztek was built with. A plug-in links nothing of Aztek's: it reaches
# the host through a table of C functions it is given when loaded.

if(TARGET AztekDevkit::api)
  return()
endif()

get_filename_component(AZTEK_DEVKIT_ROOT "${CMAKE_CURRENT_LIST_DIR}/.." ABSOLUTE)
set(AZTEK_DEVKIT_INCLUDE_DIR "${AZTEK_DEVKIT_ROOT}/include")
set(AZTEK_DEVKIT_ENTRY "${AZTEK_DEVKIT_ROOT}/src/entry.cpp")
file(GLOB AZTEK_DEVKIT_SOURCES "${AZTEK_DEVKIT_ROOT}/src/*.cpp")
list(REMOVE_ITEM AZTEK_DEVKIT_SOURCES "${AZTEK_DEVKIT_ENTRY}")

add_library(aztek_api STATIC ${AZTEK_DEVKIT_SOURCES})
add_library(AztekDevkit::api ALIAS aztek_api)
target_include_directories(aztek_api PUBLIC "${AZTEK_DEVKIT_INCLUDE_DIR}")
target_compile_features(aztek_api PUBLIC cxx_std_11)
set_target_properties(aztek_api PROPERTIES POSITION_INDEPENDENT_CODE ON CXX_VISIBILITY_PRESET hidden VISIBILITY_INLINES_HIDDEN ON)
if(MSVC)
  target_compile_options(aztek_api PRIVATE /EHsc /utf-8)
  target_compile_definitions(aztek_api PUBLIC NT_PLUGIN NOMINMAX _CRT_SECURE_NO_WARNINGS)
else()
  target_compile_definitions(aztek_api PUBLIC LINUX)
  target_link_libraries(aztek_api PUBLIC ${CMAKE_DL_LIBS})
endif()
# What plug-in builds for the other application define, for code that tests them.
target_compile_definitions(aztek_api PUBLIC _BOOL REQUIRE_IOSTREAM)

function(aztek_add_plugin name)
  cmake_parse_arguments(AZ "" "" "SOURCES;INCLUDE_DIRS;LIBRARIES;DEFINITIONS" ${ARGN})
  if(NOT AZ_SOURCES)
    message(FATAL_ERROR "aztek_add_plugin(${name}): give its SOURCES")
  endif()
  add_library(${name} MODULE ${AZ_SOURCES} "${AZTEK_DEVKIT_ENTRY}")
  target_link_libraries(${name} PRIVATE AztekDevkit::api ${AZ_LIBRARIES})
  if(AZ_INCLUDE_DIRS)
    target_include_directories(${name} PRIVATE ${AZ_INCLUDE_DIRS})
  endif()
  if(AZ_DEFINITIONS)
    target_compile_definitions(${name} PRIVATE ${AZ_DEFINITIONS})
  endif()
  set_target_properties(${name} PROPERTIES PREFIX "" CXX_VISIBILITY_PRESET hidden VISIBILITY_INLINES_HIDDEN ON)
  if(WIN32)
    set_target_properties(${name} PROPERTIES SUFFIX ".dll")
  else()
    set_target_properties(${name} PROPERTIES SUFFIX ".so")
    # A method the kit declares but doesn't have fails the link here, naming it, rather than at load.
    if(NOT APPLE)
      target_link_options(${name} PRIVATE "-Wl,--no-undefined")
    endif()
  endif()
endfunction()

set(AztekDevkit_FOUND TRUE)
