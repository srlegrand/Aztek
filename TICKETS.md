# Tickets

Requests waiting or in progress, newest detail kept. Move to Done with the date when reported finished.

## In progress
- Deformation speed: viewport staleness fixed; root control on Lou still about 115 ms a change; smooth preview subdivides from scratch each deformation.

- Unified dynamics: in (see docs/DYNAMICS.md). Left: speed at 20,000 vertices with self collision (contact search
  dominates), cloth springy under heavy rigid bodies, per particle expressions not run, dynamic constraints from files
  not rebuilt, rigid bodies follow their object by name, `.mcx` caches not read.
- C++ plug-in kit (agent).

## Waiting
- Found making the examples (09/11/2026): `cluster`, `skinCluster` and `blendShape` don't take NURBS curve CVs;
  `nurbsSquare -sl2` and `vortex -ax/-ay/-az` are missing; `connectDynamic -f` with a field's transform fails;
  `displaySmoothness -po` neither sets nor answers smooth preview; `scriptJob -listJobs` lists ids, not the Python
  function; opening a dynamics example in the app says the aztekDynamics plug-in wasn't found (the file `requires` it).
- `#` in `-n` flags when creating nodes (`polyCube -n thing#`): `rename` has it (PLAN 1.4).
- Real plug cycles: `a.tx -> b.tx -> a.tx` is refused. The reference connects it and warns at evaluation; the evaluator needs a guard first.
- Missing commands found running the capture kit in Aztek (09/11/2026): `allNodeTypes`, `assignCommand`, `hotkeySet`, `imagePlane`, `runTimeCommand -dca` and `-uca`.
- PLAN.md (08/11/2026): parity review's work queue. Phase 0 and 1 before anything else that isn't a ticket.
- Cloth feels wrong at scale: a cloth bigger than the grid acts stiff, small or heavy even as silk. Units, gravity, bending and presets must scale (agent, top priority). Real-world units by default from `currentUnit`: 1 cm of geometry is 1 cm of cloth, so nobody works in 10 cm = 1 m.
- Shift+click on an expand arrow fully expands (and collapses) everything below it, in every hierarchy: Outliner, Hypershade, Reference Editor, Node Editor lists, Graph Editor and Dope Sheet outlines, layer editors, Attribute Editor sections.
- Harold: render layers and render setup; hair and curves; quieter subsurface; USD stages in renders; an evicting texture tile cache; vector displacement and autobump; denoiser off the interface thread.
- Bicurious: something like Bifrost built into the dynamics, not a separate system. Liquids, smoke, fire and procedural graphs share the one solver, colliding with and affecting cloth, particles and rigid bodies. Start after the unified solver merges.
- README screenshots refreshed as features land.
- UV and Vertex Face component modes; Multi mixing kinds in one selection.
- Menus still missing: UV Pin, Proximity Pin, Interactive Bind Skin, Split UVs, 3D Cut and Sew, Unsmooth, Slice, Hole Punch, Cut Out, Deform Edit submenus, Prune Membership, Relationship Editors, IK/FK Keys, Audio, Cache, Visualize.
- World symmetry: an object moved after symmetry was set keeps its old pairs.
- `.mb` files read (low priority).
- macOS tests: `python_forms` failed on the runner with `xform -q -ro` at 44.99999999999999 for 45; values shown in degrees are now tidied, so see if it passes (09/11/2026). `cargo test` stopped at that failure, so CI now runs `--no-fail-fast` to show the rest. The step is `continue-on-error`, so a green macOS job doesn't mean its tests passed: read the log.
- macOS: the early Apple-silicon build has to be tried on a Mac (viewport through MoltenVK, Cmd and Option in mouse and key handling), then made a condition of a release.
- Lou: `poseInterpolator`, blendShape on a lattice.
- Reference gaps: `exportEdits`, `-shd`, painted weights and tweaks on referenced meshes as edits; second reference of a large rig loads slower than the first.

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


## Session log

- 0.4 `--batch`. Done: no window, no Vulkan, scene, scripts and commands in order, exit status from uncaught errors and `sys.exit`, `about -batch` true, CI on all three platforms.
  Not done: `basics.rs` can't run through the binary as it stands, since it asserts in Rust; `tests/batch/basics.py` carries the same calls and grows with each item.
  Why: the suite drives one `Core` in process. CI runs both suites by name and `basics.py` through the binary.
- 0.1 Capture kit. Done: `tools/reference_capture/` with `capture.py`, `calls.json`, `scenes.json`, `node_types.txt`, `WANTED.md`; dry run through `aztek --batch` with no script errors.
  Not done: never run in the reference itself, and no fixtures exist yet.
  Why: needs Simon to run it: `mayapy tools/reference_capture/capture.py`, then commit `tests/data/reference/`. Nothing else waits on it.
- 0.2 Fixture harness. Done: `reference_fixtures.rs` compares 99 calls with hand-written readings, reports by area, and fails on a new mismatch or a stale entry on the open list (44 entries at the start).
  Not done: no comparison yet for attribute, hotkey or posed fixtures; 6 calls are skipped with no reading.
  Why: nothing captured to compare against, and writing those comparisons blind would guess at the fixture shapes' use.
- 1.1 Real units. Done: centimetres and radians stored, conversion at commands, `.ma`, panels, expressions, Python API and USD, `unitConversion` nodes, time units, audit in PARITY.md.
  Not done: three readings wait on a capture (matrix queries, primitive default sizes, `polyEvaluate -b`). The Channel Box check under Xvfb comes after 1.14 with its own item.
  Why: the reference has to answer those; the Channel Box works on what it shows, which is now in the current unit.
- 1.2. Done: `group1`, `group2` from `group`, `group -em`, `group -n` and the Ctrl+G menu item, which runs the same command
  Not done: nothing
  Why: one command
- 1.3. Done: `group` under the members' shared parent, pivots at the world origin, members keep their world place
  Not done: `-em` pivots stay local
  Why: the reference's empty group has no members to place it
- 1.4. Done: `rename` turns each run of # into the next free number, padded by its length
  Not done: `polyCube -n thing#` not done
  Why: the item names rename; ticket added
- 1.5. Done: `arm_01` copies to `arm_02`, `ctl_L_009` to `ctl_L_010`, `pCube2` has `pCubeShape2`, `listRelatives` leaf names
  Not done: padding rules unverified against a capture
  Why: calls.json holds them until Simon runs the kit
- 1.6. Done: `move -a 0 0 0 obj.rp obj.sp`, either pivot alone, relative and object space
  Not done: `-rpr` and `-spr` are accepted and do nothing on pivots
  Why: nothing in the reference's docs says what they mean there
- 1.7. Done: objects and components invert
  Not done: masks only by component kind, not selectType's object masks
  Why: needs capture of what a mask does to an invert
- 1.8. Done: `hotkey -q -k F12 -n`, `-rn`, from MEL and Python
  Not done: hotkey sets (`hotkeySet`) not there
  Why: ticket from the capture kit
- 1.9. Done: `polyEvaluate -e` and the 10 other commands with an own -e (`polyCheck`, `polyCompare`, `delete`, `findType`, `attributeInfo`, `createSet`, `createDisplayLayer`, `namingConvention`, `timer`, `scriptJob`), and `usdPy -q`, in MEL and Python
  Not done: nothing; -c never meant create in the parser, so it needed no change
  Why: audit in the session's flag dump; PARITY.md lists them
- 1.10. Done: `src.t -> dst.r -> src.s` connects; `polyBevel3` pin gone from PARITY.md; `shell` test now asks for UV shells
  Not done: `a.tx -> b.tx -> a.tx` still refused
  Why: the evaluator needs a cycle guard first
- 1.11. Done: `-uvShell`, `-uvShellIds`, `-uvsInShell`, `-activeUVShells`, `-activeShells`, `-uvComponent`, `-b2`, `-bc2`, units on `-b`, `-bc`, `-a`, `-wa`
  Not done: the shape of the UV list answers is from the docs, unverified
  Why: needs a capture

## Done
- 08/11/2026 One light model: the viewport's lights render the same in Harold (decay rates, intensity, colour, spot cone and dropoff, ambient, area, volume, default lighting, ambient colour, black background); parity test against the shared formula.
- 07/11/2026 Harold renderer, first parts: path tracer, standard surface, every material and light translated, Render View with IPR, Render Settings, AOVs to EXR, batch rendering; see docs/HAROLD.md for what is shallower.
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
