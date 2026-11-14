# Plug-ins the tests build: ones that misbehave on purpose, one that reads the scene through the kit, and one that
# listens to its messages.
cmake_minimum_required(VERSION 3.13)
project(AztekTestPlugins LANGUAGES CXX)
if(NOT CMAKE_BUILD_TYPE AND NOT CMAKE_CONFIGURATION_TYPES)
  set(CMAKE_BUILD_TYPE Release)
endif()
find_package(AztekDevkit REQUIRED CONFIG PATHS "${CMAKE_CURRENT_SOURCE_DIR}/../../../../devkit/cmake" NO_DEFAULT_PATH)
aztek_add_plugin(crashNode SOURCES crashNode.cpp)
aztek_add_plugin(apiProbe SOURCES apiProbe.cpp)
aztek_add_plugin(msgProbe SOURCES msgProbe.cpp)
aztek_add_plugin(pxProbe SOURCES pxProbe.cpp)
