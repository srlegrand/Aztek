# Tickets

Requests waiting or in progress, newest detail kept. Move to Done with the date when reported finished.

## In progress
- Deformation speed: viewport staleness fixed; root control on Lou still about 115 ms a change; smooth preview subdivides from scratch each deformation.

- Unified dynamics: cloth, particles, rigid bodies in one solver (agent).
- Harold renderer (agent).
- C++ plug-in kit (agent).

## Waiting
- Shift+click on an expand arrow fully expands (and collapses) everything below it, in every hierarchy: Outliner, Hypershade, Reference Editor, Node Editor lists, Graph Editor and Dope Sheet outlines, layer editors, Attribute Editor sections.
- Bicurious: something like Bifrost built into the dynamics, not a separate system. Liquids, smoke, fire and procedural graphs share the one solver, colliding with and affecting cloth, particles and rigid bodies. Start after the unified solver merges.
- README screenshots refreshed as features land.
- UV and Vertex Face component modes; Multi mixing kinds in one selection.
- Menus still missing: UV Pin, Proximity Pin, Interactive Bind Skin, Split UVs, 3D Cut and Sew, Unsmooth, Slice, Hole Punch, Cut Out, Deform Edit submenus, Prune Membership, Relationship Editors, IK/FK Keys, Audio, Cache, Visualize.
- World symmetry: an object moved after symmetry was set keeps its old pairs.
- `.mb` files read (low priority).
- macOS build, if it's little trouble.
- Lou: `poseInterpolator`, blendShape on a lattice.
- Reference gaps: `exportEdits`, `-shd`, painted weights and tweaks on referenced meshes as edits; second reference of a large rig loads slower than the first.
- `conversionFactor` written normalised (0.0436 saves as 2.5), which the reference would read differently.

- USD gaps: MaterialX draws grey, subdivision meshes draw their hull, no component selection in a stage, time samples not on the time slider or in the Graph Editor, Outliner collections.
- polyMergeUV, polyForceUV with no UV selection do nothing where the reference may still add a node: check.


- More examples once they merge: anim layers, HumanIK, USD.

- Shadows, spot cone manipulators, Light Linking editor; check whether keys 5 and 6 go back to default lighting.
- Waitress rig about 14 fps measured during playback in the viewport: see what else takes the time now the skin is faster.

- C++ plug-in SDK, so developers with a plug-in's source can rebuild it; precompiled `.so` and `.dll` can't load.
  - Headers written clean under the same paths and names (`maya/MFnMesh.h`, `MObject`, `MStatus`, `MPxCommand`), with no text copied from the reference's devkit, whose licence forbids it.
  - `aztek_api` library over the core, mirroring the Python API 2.0 subset. Same entry points (`initializePlugin`, `uninitializePlugin`, `MFnPlugin`), and `loadPlugin` accepts what's built against the kit.
  - CMake package: `find_package(AztekDevkit)`, `aztek_add_plugin(myPlugin SOURCES ...)`. Same MSVC as Aztek on Windows.
  - A missing class or method is a compile error naming it; a small tool turns those into a report for the backlog.
  - First: `MPxCommand`, `MArgDatabase`, `MSyntax`, `MPxNode`, `MPxDeformerNode`, mesh, transform, skin and animation function sets, iterators, math classes. Later: locators and custom drawing, `MPxContext`, `MQtUtil`. Then the dynamics and rendering classes, once the unified solver and Harold have landed.


## Done
- 07/11/2026 History from every route: Smooth, Mirror, Reduce, Extract no longer wipe it; `G` after each; Hypergraph shows the whole chain.
- 07/11/2026 Smooth Preview wire as asked.
- 07/11/2026 First class USD, priorities 1 to 8 with the gaps above.
- 07/11/2026 Skin core: parallel, flat table, shared topology; a deformed mesh can't go stale in the viewport.
- 07/11/2026 Anim layers and HumanIK retargeting; gaps in PARITY.md (Custom Rig, full-body solver, ghosts).
- 07/11/2026 File > Examples with modelling history, materials, a coloured keyed arm rig and nested references.
- 07/11/2026 All six default lights in the viewport, commands and `.ma`; no shadows yet.
- 07/11/2026 Sculpting icons drawn to show what each tool does.
- 07/11/2026 Symmetry on polygon actions, B fixed, hold B falloff circle, Global, Object and Surface falloff, many menu items and option boxes.
- 07/11/2026 Every build of main is a numbered release, 1.N with N the commits so far, titled with the commit; the app shows the same.
