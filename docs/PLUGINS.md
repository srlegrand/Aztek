# Plug-ins

Aztek loads two kinds of plug-in:

- Python plug-ins (`.py`), as they are.
- Compiled plug-ins (`.so`, `.dll`, `.bundle`) built with Aztek's plug-in kit.

A compiled plug-in built for the reference can't load: it links the reference's own libraries, which Aztek does not have. The Plug-in Manager lists such a file as "Compiled for Maya" and leaves it alone. With its source, it can be rebuilt for Aztek, usually with no change to the code.

## Plug-ins that come with Aztek

Some features are plug-ins that are part of the program: `fbx` (FBX import and export) and `gltf` (glTF import and
export). They load at start, are listed in the Plug-in Manager under "Aztek (built in)", and can be unloaded or kept
from loading there. `loadPlugin fbx`, `unloadPlugin fbx` and `pluginInfo -q -loaded fbx` work as for any other. File >
Import and File > Export list a format while its plug-in is loaded.

### Compiled plug-ins that ship with Aztek

`plugins/` holds the source of compiled plug-ins other projects publish under licences that let them ship, built with
the kit by the program's own build into `plug-ins/` beside the program (installed to `lib/aztek/plug-ins` on Linux,
`plug-ins\` beside `aztek.exe` on Windows, `Contents/PlugIns/aztek` in the Mac app). Aztek looks there after the
folders below, so a scene that requires one loads it with nothing else installed, and one of the same name on the path
is used first. A file of that name built for the reference, earlier on the path, is passed over for the one Aztek ships.

- `mgear_solvers`: mGear's solver nodes (`mgear_matrixConstraint`, `mgear_curveCns`, `mgear_rollSplineKine`,
  `mgear_ikfk2Bone` and the rest), from mGear's source, unchanged. A rig built with mGear opens and works without mGear
  installed.
- `mGearWeightDriver`: mGear's RBF and vector angle node, from its source, unchanged. Its locator shape computes in
  all three modes (vector angle, generic RBF, RBF on matrices); the cones and poses it draws in the reference are not
  drawn in Aztek's viewport yet.
- `cvwrap`: the cvWrap deformer and command, mGear's copy, unchanged. `cvWrap`, `cvWrap -rb` (rebind to some faces),
  `-ex`/`-im` (export and import a binding) and `-b` work. Its GPU override is registered and never used: Aztek
  deforms on the CPU. Loaded in the interface it asks Python for its menu module (`cvwrap.menu`), which is not shipped,
  as the reference does when that module is missing.

## The Plug-in Manager

Windows > Settings/Preferences > Plug-in Manager lists every plug-in found, grouped by folder.

- Loaded: tick to load, untick to unload. A plug-in whose nodes are in the scene asks before it is forced out.
- Auto load: load it each time Aztek starts.
- Browse: load a plug-in from anywhere.
- i: what the plug-in registered (commands, nodes), its vendor and version.

From script: `loadPlugin`, `unloadPlugin`, `pluginInfo` (`-q -listPlugins`, `-loaded`, `-autoload`, `-path`, `-command`, `-dependNode`).

## Where plug-ins are looked for

- The user folder's `plug-ins` (and the chosen version's `plug-ins`). See Preferences > User Folder.
- The `plug-ins` folder of each module found (`.mod` files in the user folder's `modules`, and in `AZTEK_MODULE_PATH` and `MAYA_MODULE_PATH`).
- The folders in `AZTEK_PLUG_IN_PATH` and `MAYA_PLUG_IN_PATH`.
- Last, the folder of the compiled plug-ins that ship with Aztek.

## The user folder

Preferences > User Folder chooses where scripts, plug-ins, modules and saved preferences are read from:

- Aztek's own folder (`~/aztek`, `Documents\aztek` on Windows). An existing folder of the reference's can be copied into it; nothing already there is replaced and the folder copied from is only read.
- A folder of the reference's, used where it is. Aztek reads it and never writes to it.
- A custom folder with the same layout.

At the first start with a folder of the reference's on the machine and no choice made yet, Aztek asks which of these
to do, before any of the folder's scripts run: copy it into Aztek's own folder, use it where it is, or take nothing
from it. With no folder of Aztek's yet the copy is the one offered first; with one there, leaving it as it is.
"Ask Me Next Time" saves nothing. The question is not asked when a script or a screenshot drives the start, in batch,
or when no such folder is found.

The layout is the reference's: `scripts`, `plug-ins`, `modules`, and a version's folder (`2025`) with the same again plus `prefs`. From the chosen folder Aztek takes:

- `scripts` on the MEL and Python paths, `userSetup.mel` and `userSetup.py` run at start (can be turned off).
- `plug-ins` and `modules` as above.
- `Aztek.env` or `Maya.env`: one file is read, the first found. Path variables are added to, other variables are set if
  not already set. The rules and every variable are in [ENVIRONMENT.md](ENVIRONMENT.md).
- `prefs/userPrefs.mel`: its optionVars, as defaults. A value saved in Aztek wins.
- `prefs/shelves`: imported on request (Import Its Shelves).

Hotkeys and marking menus saved by the reference are not read.

From script: `aztekUserFolder -q`, `aztekUserFolder -mode link -folder "~/maya" -version "2025"`, `aztekUserFolder -copyFrom "~/maya"`, `internalVar -userScriptDir`, `moduleInfo -lm`.

## Rebuilding a compiled plug-in for Aztek

The kit is the `devkit` folder of Aztek's source. It has the headers under the names plug-in code already includes (`maya/MPxCommand.h`, `maya/MFnMesh.h`, ...), and it is compiled into the plug-in itself, so nothing has to match the compiler Aztek was built with.

`CMakeLists.txt`:

```cmake
cmake_minimum_required(VERSION 3.13)
project(myPlugin LANGUAGES CXX)
find_package(AztekDevkit REQUIRED CONFIG PATHS "/path/to/Aztek/devkit/cmake" NO_DEFAULT_PATH)
aztek_add_plugin(myPlugin SOURCES myPlugin.cpp)
```

`aztek_add_plugin` also takes `INCLUDE_DIRS`, `LIBRARIES` and `DEFINITIONS`.

Build and load:

```sh
cmake -S . -B build -DCMAKE_BUILD_TYPE=Release
cmake --build build --config Release
```

```mel
loadPlugin "/path/to/build/myPlugin.so";
```

A class or method the kit does not have yet fails the build and names it. Report it at https://github.com/srlegrand/Aztek/issues with the name.

Samples: `devkit/samples` (`helloCmd`, `scaleNode`, `rippleDeformer`).

```sh
cmake -S devkit/samples -B build-samples
cmake --build build-samples
```
