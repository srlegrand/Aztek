# Plan

The work queue after the parity review of 08/11/2026 (commit 04fb742). Work top to bottom. TICKETS.md still wins when
Simon puts something there; otherwise take the first open item here. PARITY.md stays the record of what is done and how
it differs; this file is what to do next.

## How to work an item

- One item, one branch, small commits, tests green, then merge. Mark the item `[x]` with the date and a one-line note
  of anything left, and update PARITY.md.
- Every item ends with a test that states the reference's result. Where the result comes from a capture (phase 0), the
  test reads the fixture. Where no capture exists yet, the test carries `// unverified: <what to capture>` and the item
  adds that request to `tools/reference_capture/WANTED.md`.
- A test that pins behaviour the reference doesn't have is a bug in the test. Change it to the reference's behaviour
  and fix whatever relied on it (see 1.10).
- Basics before breadth. No new command families, editors or menus until phases 0 to 3 are done, unless Simon asks.
- Check interactive items under Xvfb with xdotool, as in CLAUDE.md, and keep the screenshots out of the repo.

## Phase 0: ground truth

Almost nothing in PARITY.md has been compared with a running copy of the reference. That has to change first, or every
fix below is another guess.

- [x] 0.1 (09/11/2026: kit written and dry run in Aztek through `--batch`; never run in the reference, so the fixtures are still to come from Simon) Reference capture kit in `tools/reference_capture/`. Plain Python that runs inside the reference
  (`mayapy` or the Script Editor) and writes JSON fixtures to `tests/data/reference/<area>/`. Simon runs it, commits the
  output. It captures:
  - command return values and default names for a fixed list of calls (create, duplicate, group, rename with `#`,
    instance, parent, polyEvaluate with every flag, undoInfo, currentUnit round trips in m, mm, rad, pal);
  - for every node type Aztek registers: `attributeInfo -all`, short names, types, defaults, `listAttr -k`, `-cb`;
  - the full `runTimeCommand -q -uca`, `nameCommand` list and the default hotkey set (`hotkey -q` over every key and
    modifier);
  - `.ma` files of standard scenes saved by the reference (primitives with history, a keyed cube in each tangent type,
    a skinned arm, a constraint of each kind, a blendShape, a lattice, a display layer, a reference, an image plane);
  - posed results: world matrices and a sample of deformed points of those scenes at three frames.
  Keep the scripts free of anything copied from the reference's own scripts. `WANTED.md` lists what tests still need.
- [x] 0.2 (09/11/2026: in `crates/aztek_host/tests/reference_fixtures.rs`, as it needs MEL and Python; `tests/data/reference_open.txt` lists the open mismatches by item and can only shrink. Compares calls only; attributes, hotkeys and posed fixtures get comparisons when they land) `crates/aztek_scene/tests/reference_fixtures.rs`: one harness that loads a fixture, runs the same calls in
  Aztek and reports every mismatch, grouped by command. Until fixtures land, it runs on the hand-written expectations
  below and skips missing files with a message, not a pass.
- [x] 0.3 (08/11/2026: `crates/aztek_host/tests/basics.rs` and `basics_python.rs`; 15 pass, 18 phase 1 checks ignored and
  failing until their items land; found 1.14) Basics suite: `tests/basics.rs` turning the review's probe into tests. Transforms, pivots, freeze and
  reset, duplicate and instance, group and parent, names, undo, units, keys, files, selection, attributes. It
  should fail today on everything listed in phase 1.
- [x] 0.4 (09/11/2026: `aztek --batch`, `crates/aztek_host/src/batch.rs`; `tests/batch.rs` runs in process and, with `AZTEK_BIN`, through the real binary with no display, as CI does on Linux and macOS; Windows checks statuses through the binary. `tests/batch/basics.py` carries the basics end to end and grows with each item) A `--batch` mode (no window, no Vulkan) that runs `--script` and exits with the script's status, so the
  basics suite and CI can run Python and MEL end to end without Xvfb. Add it to `build.yml`.

## Phase 1: basics that are wrong today

Each was reproduced on 08/11/2026 through `maya.cmds` in the built app.

- [ ] 1.1 Units are labels only. `currentUnit -l m` then `getAttr pCube1.tx` on a 100 cm translate gives 100, not 1;
  `currentUnit -a rad` then `getAttr .ry` on 90 degrees gives 90, not 1.5708, and `setAttr .ry 1.0` in radians stores
  1 degree. Store centimetres and radians internally as the reference does, convert at every command, `.ma`, channel
  box, attribute editor, manipulator readout, numeric input and Python API boundary by the current unit, with
  `unitConversion` nodes on connections between differing types. Time units too (`pal`, `ntsc`, `film`, custom fps).
  Biggest item in the plan: audit every command that takes or returns a distance or angle; list them in PARITY.md.
- [ ] 1.2 `group` names: `group1`, `group2`, never `Group`. Also check `group -em`, `group -n`, MEL `group` and the
  Ctrl+G path.
- [ ] 1.3 `group` puts the new group under the members' shared parent and its pivot at the world origin, as the
  reference does (PARITY.md says it goes under the world because tests rely on it: fix the tests).
- [ ] 1.4 `rename` with `#` replaces the hashes with the next free number (`thing#` gives `thing1`), padded by the count
  of hashes (`thing###` gives `thing001`).
- [ ] 1.5 Duplicate naming: a trailing number increments and keeps its padding as the reference does (`arm_01` gave
  `arm_1`). `listRelatives` without `-path` or `-fullPath` gives short leaf names, where Aztek gives paths. Confirm the rule with a capture, including `_L_01` style names and names ending in a digit after a letter.
- [ ] 1.6 `move` and `xform` on `.rotatePivot` and `.scalePivot` take `-a`, `-r`, `-ws`, `-os`, `-rpr`, `-spr` like any
  move. `move -a 0 0 0 pCube1.scalePivot pCube1.rotatePivot` currently fails with "unknown flag 'absolute'".
- [ ] 1.7 `InvertSelection` inverts (objects in object mode, components in component mode, respecting the masks).
- [ ] 1.8 `hotkey -q` reads the key from `-k` and never from the selection.
- [ ] 1.9 Short flags that collide with the edit flag: `polyEvaluate -e` (edges) returns None. Audit every command for
  short flags `e`, `q`, `c` that mean something else in that command, and take the command's own meaning.
- [ ] 1.10 Tests that pin a divergence: `polyBevel31` (the reference makes `polyBevel1`), `group` under the world,
  `src.t -> dst.r` with `dst.r -> src.s` refused as a cycle. Go through PARITY.md for every "because a test relies on
  it" and fix both.
- [ ] 1.11 `polyEvaluate` with several flags returns a dict in Python and an array in MEL, as the reference does; add
  `-uvShell`, `-uvsInShell`, `-activeUVShells` and the rest of its flag table.
- [ ] 1.12 Smooth preview is `displaySmoothMesh` (0, 1, 2) on the shape with `smoothLevel`, `useSmoothPreviewForRender`
  and the rest of the reference's attribute names, so `.ma` files open smoothed and `displaySmoothness -q -po` answers.
  Keep `smoothPreview` as an alias for old Aztek files.
- [ ] 1.13 `shadingNode -isColorManaged`, `hyperShade -listUpstreamShaderNodes` and the other `hyperShade` flags.
- [ ] 1.14 Found by the basics suite (08/11/2026): an instance is its own shape node joined to the source by a
  connection, so `listRelatives -ap` on the shape lists one parent and attributes set on one instance's shape don't
  reach the other. The reference has one shape under several parents. Also `duplicate` keeps the source's shape name
  under the new transform (`pCube2|pCubeShape1`) where the reference names it `pCubeShape2`: in 1.5.

## Phase 2: missing basics

- [ ] 2.1 Runtime commands. Every name in the reference's default `runTimeCommand` list exists and does what its menu
  item does, so reference hotkey files, shelves and marking menus work. Today missing, among others:
  `GrowPolygonSelectionRegion`, `ShrinkPolygonSelectionRegion`, `FreezeTransformations`, `ResetTransformations`,
  `DuplicateSpecial`, `SelectHierarchy`, `SelectAllPolygonGeometry`, `HideSelectedObjects`, `ShowAll`,
  `ToggleIsolateSelect`, `SnapToGrid`, `BakeCustomPivot`, `MatchPivots`, `PolyExtrude`, `BevelPolygon`,
  `InsertEdgeLoopTool`, `MergeVertexTool`, `ReversePolygonNormals`, `TogglePolyDisplayHardEdges`, `ToggleBackfaceCulling`,
  `ToggleWireframeOnShaded`, `DisplayShadedAndTextured`, `ToggleAutoKey`, `Ungroup`, `ArchiveScene`, `OptimizeScene`,
  `Hypershade`, `PlayblastOptions`, `CreateImagePlane`, `CreateNURBSCircle`. A test walks the captured list and fails
  on any name `exists` doesn't know. `runTimeCommand -q -uca` and `-dca` answer.
- [ ] 2.2 Default hotkey set matches the captured one key for key, including Insert and D for pivot, X C V snapping,
  6 and 7, H, Ctrl+H, Ctrl+Shift+H, Alt+B, Ctrl+Shift+D, Shift+H, F1 help, Home, Page Up and Down. Hotkey sets
  import from the reference's `.mhk`.
- [ ] 2.3 Image planes: `imagePlane` command and node (camera-attached and free), `CreateImagePlane`, View > Image
  Plane menu, file sequences with frame offset, depth, alpha gain, fit modes, display in all views or looking through,
  `.ma` read and write.
- [ ] 2.4 Autosave: `autoSave` command (enable, interval, limit, destination, prompt), the preference page, timed saves
  that don't block, recovery of the last autosave after a crash.
- [ ] 2.5 Sound: `sound` node and command, `timeControl -sound`, waveform in the time slider, scrubbing audio, sync to
  playback. WAV first.
- [ ] 2.6 `file -type "OBJexport"` and `file -i -type "OBJ"` with the reference's `OBJexport` options string; keep
  `exportObj` and `importObj` as Aztek's own. `translator -q -l` answers with the types Aztek has.
- [ ] 2.7 UV basics: `texGetTexelDensity`, `texSetTexelDensity`, the UV Toolkit's texel density row, UDIM-aware
  layout (tiles, padding by pixels), `polyEvaluate -uvShell`.
- [ ] 2.8 Rigging basics: `uvPin` and `proximityPin` nodes and menus (ribbons and pinned controls rely on them),
  `poseInterpolator` (already a ticket: Lou needs it).
- [ ] 2.9 `userSetup.py` and `userSetup.mel` run at start from the script paths; modules (`.mod` files on
  `MAYA_MODULE_PATH` and `AZTEK_MODULE_PATH`, `moduleInfo`) add script, plug-in, icon and Python paths.

## Phase 3: interaction basics

From PARITY.md's own lists; each one a user notices in the first hour.

- [ ] 3.1 UV and Vertex Face component modes in the viewport and marking menu, and Multi that mixes kinds in one
  selection. `ComponentMode` grows both; everything that matches on it gets a case.
- [ ] 3.2 Curve Edit Point and Curve Point modes; `curve1.ep[]` and `.u[]` picked in the view.
- [ ] 3.3 Graph Editor tangent handles drag (break and unify, weighted lengths, Shift to constrain), with one undo
  step per drag. Handles are drawn today and do nothing.
- [ ] 3.4 Ghosts drawn: `ghosting` settings, the Animation menu items, colours and opacities, steps before and after,
  keyframe ghosts.
- [ ] 3.5 Gimbal rotate draws the gimbal axes; rotate and scale step snap absolute as well as relative; Universal
  Manipulator.
- [ ] 3.6 Snap Pivot Orientation acts; D and Insert pivot mode with point, edge and face snapping of the pivot;
  Bake Pivot and Match Pivots.
- [ ] 3.7 Outliner: drag to reorder between siblings (and `reorder`), Middle-drag parenting, rename in place, Shift+click
  expand all (already a ticket), search filter.
- [ ] 3.8 Per-object display toggles (`displayHandle`, `displayLocalAxis`, `displayRotatePivot`, template, bounding box,
  drawing overrides) saved with the scene in both formats.
- [ ] 3.9 Z up: grid, view cube, default cameras, manipulators, new primitives' orientation and `.ma` header all follow
  `upAxis`. Camera roll lives on the camera node.
- [ ] 3.10 View tools: walk tool with WASD and speed keys, a view cube, 2D pan and zoom on the film back, tumble
  around the selection or pivot by preference.
- [ ] 3.11 Soft select Global and Object reach NURBS and show their colours; World symmetry follows a mesh moved after
  it was set.
- [ ] 3.12 The remaining menu items in PARITY.md's list: Split UVs, 3D Cut and Sew, Unsmooth, Slice, Hole Punch, Cut Out,
  Deform > Edit submenus, Prune Membership, Relationship Editors, IK/FK Keys, Audio, Cache, Visualize. Tools get option
  boxes.

## Phase 4: interchange

- [ ] 4.1 FBX import and export: meshes with normals, UVs, colours and materials, skeletons, skin weights,
  blendShapes, cameras, lights, animation, binary and ASCII, `FBXExport` and `FBXImport` with the common `FBX*`
  option commands. Pick a reader and writer whose licence fits MIT-0; write down the choice in PARITY.md.
- [ ] 4.2 Alembic: `AbcExport` and `AbcImport` with their `-j` job strings (frame range, step, roots, uv, normals,
  world space, attributes, euler filter), Ogawa archives, an `AlembicNode` that streams a cache into a shape.
- [ ] 4.3 `.mb` read (ticket, low priority): at least geometry, transforms, animation and skin.
- [ ] 4.4 `.ma` against real files: run every capture from 0.1 through `load_ma` and fix what differs; check the parts
  PARITY.md names first (`.kit`/`.kot`, `.kix`/`.kiy`, `.xm`, `conversionFactor`).

## Phase 5: scripting ecosystem

What studio tools need before they run at all.

- [ ] 5.1 PySide6 and shiboken6 bundled to match Aztek's Qt, `maya.OpenMayaUI.MQtUtil` (`mainWindow`, `findControl`,
  `findLayout`, `fullName`, `getCurrentParent`), so tools that build Qt windows parent them to the main window and find
  script-built controls.
- [ ] 5.2 `maya.OpenMaya` (API 1) subset over the same core as API 2: `MGlobal`, `MSelectionList`, `MDagPath`,
  `MObject`, `MFnMesh`, `MFnTransform`, `MFnDependencyNode`, the iterators, `MScriptUtil`, `MPxCommand`, `MPxNode`.
- [ ] 5.3 API 2 gaps from PARITY.md: `MPxDeformerNode`, `MPxLocatorNode`, mesh attributes into `compute`, compound
  arrays, several UV sets, `MRampAttribute`, shear on `MFnTransform`.
- [ ] 5.4 Check that `pip install pymel` imports and runs a common set of calls against the shims; fix the shim where
  pymel's import-time introspection fails.
- [~] 5.5 (08/11/2026: in are the ABI, loading, commands, nodes, deformers, data blocks, plugs, DAG and transform
  function sets, modifiers, CMake package and three samples; MFnMesh and iterators were under way, paused) C++ plug-in kit (in progress): keep going, but after 5.1 to 5.3, since most studio tools are Python.

## Phase 6: speed

- [ ] 6.1 Numbers first: a timing example per rig on the machine (Lou root control, Waitress playback, a 1M face
  smooth preview, a 100k vertex skin), recorded in PARITY.md each time a change lands.
- [ ] 6.2 Parallel evaluation for real: `evaluationManager` modes act, nodes evaluate across cores by the graph, and
  `threadCount` is read.
- [ ] 6.3 Cached playback: frames cached in the background after a change, the cache line on the time slider,
  invalidated by the graph.
- [ ] 6.4 Smooth preview refines incrementally (topology once, positions per deformation) instead of from scratch.
- [ ] 6.5 Targets: Waitress at 24 fps or better in playback, a root control drag under 16 ms on Lou.

## Phase 7: rigging depth

- [ ] 7.1 Layered skin weights with masks and mirroring, as the reference's 2027 release ships them, saved on the
  skinCluster and in `.ma`.
- [ ] 7.2 One deformer node across several meshes, as the reference (today one node per mesh); skinCluster with its set
  so membership limits a skin.
- [ ] 7.3 IK handle `weight`, `poWeight`, `priority`, `stickiness` and joint `stiffness` change the solve.
- [ ] 7.4 HumanIK Custom Rig and the full body solve with Pull, Stiffness, Reach and pinning.
- [ ] 7.5 The remaining items: `combinationShape` formula against a capture, `skeletonEmbed`, `flexor`, the deformer
  differences PARITY.md lists.

## Phase 8: animation depth

- [ ] 8.1 Camera Sequencer: `shot` and `sequencer` nodes, the Sequencer editor, uber camera, playblast per shot, trim
  and retime by dragging, OTIO import and export as the reference's 2027.1 adds.
- [ ] 8.2 Time Editor with clips, composition and layers; then Trax.
- [ ] 8.3 Blue Pencil drawing on views by frame.
- [ ] 8.4 Animation bookmarks on the time slider.

## Phase 9: look development in the viewport

- [ ] 9.1 Colour management: OCIO config, `colorManagementPrefs`, view and display transforms in the viewport, Render
  View and playblast, input colour spaces per file node.
- [ ] 9.2 Viewport textures: ramp, checker, noise and the other procedurals baked for display, layered textures, UV
  sets other than the first, UDIM and frame sequences.
- [ ] 9.3 Viewport shadows, then reflections and bump; spot cone manipulators; Light Linking editor (already tickets).
- [ ] 9.4 MaterialX draws, in the viewport and Harold.
- [ ] 9.5 Hypershade work area and the Create Render Node window.

## Phase 10: rendering, dynamics, platform

Keep working the existing tickets for Harold (render layers and render setup first), the unified dynamics and
Bicurious in their own order; this plan doesn't reorder them. macOS: CI builds an early Apple-silicon package that no release waits on; checking it on a Mac (viewport, Cmd and Option) comes after the phases above.

## Not planned

Out of scope unless Simon says otherwise: XGen and hair grooming, MASH, Paint Effects, the reference's fluids (Bicurious
covers liquids and smoke), MotionMaker, the reference's help chatbot and its cloud service button.
