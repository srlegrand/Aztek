# The kit's samples: cmake -S devkit/samples -B build-samples && cmake --build build-samples
cmake_minimum_required(VERSION 3.13)
project(AztekDevkitSamples LANGUAGES CXX)
if(NOT CMAKE_BUILD_TYPE AND NOT CMAKE_CONFIGURATION_TYPES)
  set(CMAKE_BUILD_TYPE Release)
endif()

find_package(AztekDevkit REQUIRED CONFIG PATHS "${CMAKE_CURRENT_SOURCE_DIR}/../cmake" NO_DEFAULT_PATH)

aztek_add_plugin(helloCmd SOURCES helloCmd/helloCmd.cpp)
aztek_add_plugin(scaleNode SOURCES scaleNode/scaleNode.cpp)
aztek_add_plugin(rippleDeformer SOURCES rippleDeformer/rippleDeformer.cpp)
