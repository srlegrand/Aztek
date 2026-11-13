# Plug-ins that ship with Aztek, built from their own source with Aztek's plug-in kit. Each folder holds the source as
# its project publishes it, with its licence; THIRD_PARTY.md says where each came from.
#
#   mgear_solvers     mGear's solver nodes (mgear_curveCns, mgear_ikfk2Bone, mgear_matrixConstraint ...), from
#                     mGear 5's src/ (github.com/mgear-dev/mgear), unchanged
#   mGearWeightDriver mGear's RBF and vector angle node (github.com/mgear-dev/mgear), unchanged
#   cvwrap            the cvWrap deformer and command, mGear's copy (github.com/mgear-dev/cvwrap), unchanged
#
# They are written to plug-ins/ beside the program and installed to lib/aztek/plug-ins (Linux); Windows builds copy
# plug-ins/ beside aztek.exe and the Mac app keeps them in Contents/PlugIns/aztek. Aztek looks there after the folders
# of the plug-in path, so a scene that requires one of them loads it with nothing else installed.

find_package(AztekDevkit REQUIRED CONFIG PATHS "${CMAKE_CURRENT_SOURCE_DIR}/../devkit/cmake" NO_DEFAULT_PATH)

set(AZTEK_SHIPPED_PLUGIN_DIR "${CMAKE_BINARY_DIR}/plug-ins")

function(aztek_shipped_plugin name)
  aztek_add_plugin(${name} ${ARGN})
  if(UNIX AND NOT APPLE)
    install(TARGETS ${name} LIBRARY DESTINATION lib/aztek/plug-ins)
  endif()
  set_target_properties(${name} PROPERTIES
    LIBRARY_OUTPUT_DIRECTORY "${AZTEK_SHIPPED_PLUGIN_DIR}"
    LIBRARY_OUTPUT_DIRECTORY_DEBUG "${AZTEK_SHIPPED_PLUGIN_DIR}"
    LIBRARY_OUTPUT_DIRECTORY_RELEASE "${AZTEK_SHIPPED_PLUGIN_DIR}"
    LIBRARY_OUTPUT_DIRECTORY_RELWITHDEBINFO "${AZTEK_SHIPPED_PLUGIN_DIR}")
endfunction()

file(GLOB MGEAR_SOLVERS_SOURCES "${CMAKE_CURRENT_SOURCE_DIR}/mgear_solvers/*.cpp")
aztek_shipped_plugin(mgear_solvers SOURCES ${MGEAR_SOLVERS_SOURCES})


file(GLOB MGEAR_WEIGHT_DRIVER_SOURCES "${CMAKE_CURRENT_SOURCE_DIR}/mGearWeightDriver/*.cpp")
aztek_shipped_plugin(mGearWeightDriver SOURCES ${MGEAR_WEIGHT_DRIVER_SOURCES})

file(GLOB CVWRAP_SOURCES "${CMAKE_CURRENT_SOURCE_DIR}/cvwrap/*.cpp")
aztek_shipped_plugin(cvwrap SOURCES ${CVWRAP_SOURCES})
