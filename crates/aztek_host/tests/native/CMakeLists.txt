# Plug-ins the tests build: ones that misbehave on purpose.
cmake_minimum_required(VERSION 3.13)
project(AztekTestPlugins LANGUAGES CXX)
if(NOT CMAKE_BUILD_TYPE AND NOT CMAKE_CONFIGURATION_TYPES)
  set(CMAKE_BUILD_TYPE Release)
endif()
find_package(AztekDevkit REQUIRED CONFIG PATHS "${CMAKE_CURRENT_SOURCE_DIR}/../../../../devkit/cmake" NO_DEFAULT_PATH)
aztek_add_plugin(crashNode SOURCES crashNode.cpp)
