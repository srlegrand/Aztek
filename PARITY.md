# Aztek: behaviour target

Aztek follows the reference application's behaviour for modelling, UVs, rigging and animation, plus viewport navigation,
manipulators, the script editor and preferences, and renders with Harold, its own path tracer, in place of every renderer
the reference carries. Dynamics depart on purpose: one solver stands for the reference's several dynamics systems, and
their files load into it (see the dated section and docs/DYNAMICS.md). The product carries none of the reference's names, icons or branding.

## Names and pivots

Done 09/11/2026 (PLAN 1.2 to 1.8). `group` is `group1`, `group2`, under its members' shared parent with pivots at the world's origin. `rename` takes `#`. A trailing number keeps its padding in every generated name. A copied shape takes the next free shape name. `listRelatives` gives leaf names unless `-path` or `-fullPath`, so a script handing a result to another command wants one of those, as in the reference. `move` on `obj.rotatePivot` and `obj.scalePivot` takes `-a`, `-r`, `-ws`, `-os`, one pivot at a time. `InvertSelection` and `hotkey -q` work. Unverified against a capture: all the naming rules (`tools/reference_capture/WANTED.md`).

## Short flags and polyEvaluate

Done 09/11/2026 (PLAN 1.9 to 1.11). Whether `-e` or `-q` means edit and query now comes from each command's flag table. Commands where it doesn't: `-e` on `polyEvaluate` and `polyCheck` (edge), `polyCompare` (edges), `delete` (expressions), `findType` (exact), `attributeInfo` (enumerated), `createSet` and `createDisplayLayer` (empty), `namingConvention` (enable), `timer` (endTimer), `scriptJob` (event); `-q` on `usdPy` (quiet). `-c` was never read as anything generic.

`polyEvaluate`: `-v -e -f -t -uv -s -b -b2 -a -wa -vc -ec -fc -uvc -tc -bc -bc2 -us -uid -uis -aus -as`, with `-uvs` accepted for the current set and `-ds` accepted. Several flags give a dictionary by long name in Python and an array in MEL in the order given. `-s` counts mesh shells, where it counted UV shells; `-us` counts those. Lengths and areas are in the current unit: unverified (`units.m.polyEvaluate.b`).

## Smooth preview and shading flags

Done 09/11/2026 (PLAN 1.12, 1.13). Smooth preview is `displaySmoothMesh` (0, 1, 2) and `smoothLevel` on the shape, with `useSmoothPreviewForRender` and `renderSmoothLevel` for Harold. `displaySmoothness -po` maps 1, 2, 3 onto it and `-q -po` answers per object. `smoothPreview` stays as a command and as an alias for old files and scripts. `shadingNode` takes `-isColorManaged` and `-asRendering`; `hyperShade` has `-listUpstreamShaderNodes`, `-listDownstreamShaderNodes`, `-shaderNetwork` and `-duplicate`. Its window flags are accepted and do nothing.

## Instances

PLAN 1.14, partly (09/11/2026). An instanced shape behaves as one shape to a script: `listRelatives -ap` gives every parent, `setAttr` on one reaches the others, a rename follows, geometry is shared and `.ma` holds the reference's `parent -s -add` form. Inside, each parent still holds a node of its own, so a direct graph write doesn't carry across, `MFnDagNode.instanceCount` doesn't see it, and only meshes instance. TICKETS.md has the rest.

## Checks

`tools/check_examples.sh` opens every example under `--batch` and fails on any warning. `tools/ui_checks/channel_box.py` drives the Channel Box's multi-edit by mouse and keyboard on Xvfb with a window manager; without one, Qt never takes the window as active and keys go to the hotkeys. Both run in CI on Linux.

## Units

Done 09/11/2026 (PLAN 1.1). What's stored is centimetres and radians. A command called from a script or the interface is in the current units, and `execute` converts its flags, arguments and results by tags in `crates/aztek_scene/src/unit_tables.rs`; commands called by other commands, viewport tools and the Python API are in stored units. `currentUnit` lives in `aztek_ops::units`, per scene: a new scene starts in File > New Scene's default working units (cm, degrees and film unless changed, kept as the `workingUnitLinearDefault`, `workingUnitAngularDefault` and `workingUnitTimeDefault` optionVars), opening a file takes on the file's.

- Scene files: written in the scene's current units, which the header names; read by the units the file names. `.ma` angle curves hold stored radians once read. Tangents in files stay as the file has them (value per second).
- `connectAttr` between plugs that measure different things makes a `unitConversion` node with the reference's factor (translate to rotate is 0.0174533), one per axis for a whole vector. `disconnectAttr` removes it, `isConnected` sees through it, `listConnections` shows it unless `-scn`. A file's own connection is taken as written.
- Python API (`maya.api.OpenMaya`, plug-ins): centimetres and radians whatever the scene, as the reference's are. `MAngle.uiUnit` and `MDistance.uiUnit` follow `currentUnit`.
- Panels: Channel Box and its multi-edit (`+=`, `-=`, `*=`, `/=` work on what's shown), Attribute Editor, Graph Editor and Dope Sheet key values read in the current units through `query.rs`. Manipulator drags work in stored units and echo in the current ones.
- Expressions: angular plugs read in the current unit unless `-uc none`, distances under `-uc all`.
- Time: film, pal, ntsc, show and the rest of the reference's names, `29.97df` as its own unit beside `29.97fps`, and custom rates such as `37fps`. Keys keep their seconds across a change unless `-ua 0` (Keep keys at current frame). Frames are what's stored, rescaled on a change, where the reference stores ticks: same results.
- Preferences > Settings, as of 10/11/2026: Working Units (linear, angular, time, Keep keys at current frame) set the current scene's units; New Scene Options (File > New Scene's option box) set the defaults. A change tells `scriptJob -e linearUnitChanged`, `angularUnitChanged` and `timeUnitChanged`.
- Grid: `grid` sizes and spacings are in the current linear unit, as the reference's are, so 12 units each way is 12 metres in a metre scene. The move tool's step and a held J step in the linear unit; the drag's readout in the view shows the current units.
- USD stages hold degrees: converted where they meet the scene.

## Up axis

As of 10/11/2026. Y or Z up, as Preferences > Settings > World Coordinate System sets it: a preference kept between
sessions (`optionVar upAxisDirection`), not part of a scene, so a new scene keeps it. `upAxis -ax z` changes it;
`-rv` turns the default cameras with the world so the ground stays where it was on screen, as the Preferences do on
Save. With Z up:

- The default cameras of a new scene, `viewSet -home`, `-top` (down Z, Y up the screen), `-front` (from -Y),
  `-side`, `-persp`, `orbit`, `tumble` and `viewPlace` work about Z; the views tumble about Z and their pitch is
  above the XY ground.
- The grid lies on XY in the perspective view. Clicks that place on the ground (joints, curves, interactive
  primitives, `X` grid snapping) use XY.
- Primitives are built standing up Z (their `upAxis` attribute says z).
- Saving writes the reference's `upAxisScriptNode` (`upAxis -ax "z"; viewSet -home persp;`). Opening such a file in a Y
  up session switches to Z up and homes the perspective camera, as running that script does; the node isn't kept and
  is written again on save. A Y up file opened in a Z up session leaves the session Z up, as the reference does. The
  file's default cameras start from the defaults of the axis it was saved in.
- `MGlobal.isYAxisUp`, `isZAxisUp`, `upAxis`, `setYAxisUp` and `setZAxisUp` follow it. OBJ, PLY and STL `-upAxis`
  and USD import convert to it; USD import with `-axisAndUnitMethod overwritePrefs` takes the stage's axis.
- Not changed by the axis: attribute defaults (aim vectors, gravity and field directions). Unchecked against the
  reference: whether its gravity and nucleus defaults follow Z up.
- Batch mode (`aztek --batch`) starts Y up with the default units: it doesn't read the saved preferences. It does use the user folder: its paths and modules, the plug-ins marked Auto load and `userSetup`.
- Left as they are: HumanIK's per-joint state attributes keep what their solver works in. Attributes Aztek adds itself with no counterpart (`hardAngle`, `minAngle`) and UV angles the reference documents in degrees (`polyEditUV -angle`, `polyMoveUV -ra`) stay in degrees.
- Unverified, all on the capture list: whether `xform -q -m` answers in the current unit (here centimetres), whether a primitive's default size follows the unit (here it doesn't: `polyCube` with no `-w` in a metre scene is 1 cm wide), and whether `polyEvaluate -b` scales (1.11).

Audited, commands whose flags carry a unit:

- `ambientLight`: -position (distance), -rotation (angle)
- `directionalLight`: -position (distance), -rotation (angle)
- `pointLight`: -position (distance), -rotation (angle)
- `unfrozenTransforms`: -rotateStep (angle)
- `xform`: -translation (distance), -rotatePivot (distance), -scalePivot (distance), -pivots (distance), -rotateTranslation (distance), -scaleTranslation (distance), -rotation (angle), -rotateAxis (angle), -boundingBox (distance)
- `rotate`: -pivot (distance), -by (angle), -to (angle), -x (angle), -y (angle), -z (angle)
- `move`: -x (distance), -y (distance), -z (distance), -by (distance), -to (distance)
- `scale`: -pivot (distance)
- `duplicate`: -translate (distance), -rotate (angle), -pivotPoint (distance)
- `joint`: -position (distance), -orientation (angle), -scaleOrientation (angle), -angleX (angle), -angleY (angle), -angleZ (angle), -limitX (angle), -limitY (angle), -limitZ (angle)
- `ikHandle`: -twist (angle), -roll (angle), -offset (distance)
- `spaceLocator`: -position (distance)
- `camera`: -position (distance), -rotation (angle), -nearClipPlane (distance), -farClipPlane (distance), -orthographicWidth (distance), -centerOfInterest (distance), -worldCenterOfInterest (distance)
- `viewPlace`: -orthoWidth (distance)
- `viewClipPlane`: -nearClipPlane (distance), -farClipPlane (distance)
- `lookThru`: -nearClip (distance), -farClip (distance)
- `dolly`: -distance (distance)
- `tumble`: -azimuthAngle (angle), -elevationAngle (angle), -rotationAngles (angle)
- `orbit`: -horizontalAngle (angle), -verticalAngle (angle), -rotationAngles (angle)
- `roll`: -degree (angle)
- `spotLight`: -position (distance), -rotation (angle), -coneAngle (angle), -penumbra (angle), -leftBarnDoorAngle (angle), -rightBarnDoorAngle (angle), -topBarnDoorAngle (angle), -bottomBarnDoorAngle (angle)
- `curve`: -point (distance), -editPoint (distance)
- `circle`: -center (distance), -radius (distance), -sweep (angle), -firstPoint (distance)
- `nurbsSquare`: -center (distance), -sideLength1 (distance)
- `sphere`: -pivot (distance), -radius (distance), -startSweep (angle), -endSweep (angle)
- `cone`: -pivot (distance), -radius (distance), -startSweep (angle), -endSweep (angle)
- `cylinder`: -pivot (distance), -radius (distance), -startSweep (angle), -endSweep (angle)
- `torus`: -pivot (distance), -radius (distance), -startSweep (angle), -endSweep (angle), -minorSweep (angle)
- `nurbsPlane`: -pivot (distance), -width (distance)
- `nurbsCube`: -pivot (distance), -width (distance)
- `revolve`: -pivot (distance), -startSweep (angle), -endSweep (angle)
- `extrude`: -length (distance), -rotation (angle)
- `offsetCurve`: -distance (distance)
- `offsetSurface`: -distance (distance)
- `extendCurve`: -distance (distance)
- `extendSurface`: -distance (distance)
- `threePointArc`: -point1 (distance), -point2 (distance), -point3 (distance), -radius (distance)
- `twoPointArc`: -point1 (distance), -point2 (distance), -radius (distance)
- `nonLinear`: -startAngle (angle), -endAngle (angle), -wavelength (distance)
- `softMod`: -falloffRadius (distance), -falloffCenter (distance)
- `softSelect`: -softSelectDistance (distance)
- `wire`: -dropoffDistance (distance), -rotation (angle)
- `wrap`: -maxDistance (distance)
- `lattice`: -position (distance), -rotation (angle)
- `pathAnimation`: -bankLimit (angle), -bankThreshold (angle)
- `polyMergeVertex`: -distance (distance)
- `polyMerge`: -distance (distance)
- `polyBevel`: -offset (distance), -smoothingAngle (angle)
- `polyBevel3`: -offset (distance), -angleTolerance (angle), -miteringAngle (angle), -smoothingAngle (angle)
- `polySoftEdge`: -angle (angle)
- `polyCut`: -cutPlaneCenter (distance), -cutPlaneRotate (angle), -cutPlaneWidth (distance), -cutPlaneHeight (distance), -extractOffset (distance)
- `polyMirrorFace`: -mirrorPosition (distance), -pivot (distance), -pivotX (distance), -pivotY (distance), -pivotZ (distance), -smoothingAngle (angle)
- `polyMirror`: -mirrorPosition (distance), -pivot (distance), -pivotX (distance), -pivotY (distance), -pivotZ (distance), -tolerance (distance)
- `polyPoke`: -translate (distance), -translateX (distance), -translateY (distance), -translateZ (distance), -localTranslate (distance), -localTranslateX (distance), -localTranslateY (distance), -localTranslateZ (distance)
- `polyInset`: -thickness (distance), -depth (distance)
- `polyCreateFacet`: -point (distance)
- `polyAppend`: -point (distance)
- `polyChamferVtx`: -width (distance)
- `polyWedgeFace`: -wedgeAngle (angle), -center (distance)
- `polyQuad`: -angle (angle)
- `polyQuadrangulate`: -maxAngle (angle)
- `polyBridgeEdge`: -twist (angle), -smoothingAngle (angle)
- `polyBridge`: -twist (angle), -smoothingAngle (angle)
- `polySpherize`: -radius (distance), -center (distance)
- `rotateComponents`: -by (angle), -pivot (distance)
- `scaleComponents`: -pivot (distance)
- `moveComponents`: -by (distance), -to (distance)
- `distanceDimension`: -startPoint (distance), -endPoint (distance)
- `closestPointOnMesh`: -inPosition (distance)
- `nearestPointOnMesh`: -inPosition (distance)
- `setPivot`: -position (distance)
- `transformLimits`: -translation (distance), -translationX (distance), -translationY (distance), -translationZ (distance), -rotation (angle), -rotationX (angle), -rotationY (angle), -rotationZ (angle)
- Primitives (`polyCube`, `polySphere`, `polyCylinder`, `polyCone`, `polyPlane`, `polyTorus`, `polyPrism`, `polyPyramid`, `polyPipe`, `polyHelix`, `polyDisc`, `polyCapsule`, `polyPlatonicSolid`, `polyPrimitive`, `polySoccerBall`, `polyIcoSphere`, `polyQuadSphere`, `polySuperShape`): -position, -width, -height, -depth, -radius, -sideLength, -length, -tipRadius, -sectionRadius, -thickness (distance); -rotation, -twist (angle)
- Component edits (`polyExtrudeFacet`, `polyExtrudeEdge`, `polyExtrude`, `polyMoveVertex`, `polyMoveEdge`, `polyMoveFacet`, `polyChipOff`, `polyDuplicateEdge`): -translate, -localTranslate, -pivot and their X Y Z, -offset, -thickness (distance); -rotate, -localRotate and their X Y Z, -smoothingAngle, -twist (angle)
- Constraints (`pointConstraint`, `orientConstraint`, `aimConstraint`, `geometryConstraint`, `normalConstraint`, `tangentConstraint`, `poleVectorConstraint`, `pointOnPolyConstraint`): -offset (angle on orient and aim, distance on the rest)
- Any command's -smoothingAngle, -miteringAngle, -angleTolerance (angle)
- Arguments: `move` (distance), `rotate` (angle)
- Results: `pointPosition`, `objectCenter`, `exactWorldBoundingBox`, `arclen`, `pointOnCurve`, `pointOnSurface` (distance); `angleBetween` (angle)
- In their bodies: `getAttr`, `setAttr`, `addAttr`, `attributeQuery`, `setKeyframe -v`, `keyframe -vc`, `keyTangent` angles, `setDrivenKeyframe -v`, `unfrozenTransforms`, `getParticleAttr`

Attributes tagged as angles: `rotate`, `rotateAxis`, `jointOrient`, `preferredAngle`, `minRotLimit`, `maxRotLimit`, `restRotate`, `constraintRotate`, `constraintJointOrient`, `constraintRotateAxis`, `targetOffsetRotate`, `targetRotate`, `targetJointOrient`, `localRotate`, `twist`, `roll`, `startAngle`, `endAngle`, `coneAngle`, `penumbraAngle`, `shutterAngle`, `startSweep`, `endSweep`, `sweep`, `minorSweep`, `smoothingAngle`, `smoothingangle`, `miteringAngle`, `angleTolerance`, `outRotate`, `outputRotate`, `inputRotate`, `inRotate1`, `inRotate2`, `rotateFrame`, `rotateUV`, `pivotOrient`, `cutPlaneRotate`, `bankLimit`, `wedgeAngle`, `frontTwist`, `upTwist`, `sideTwist`, `twistRotation`, `rollRotation`, `projectionHorizontalSweep`, `rotation`, `dTwistStart`, `dTwistEnd`, `dTwistRampMult`, `euler`. As distances: `translate`, `rotatePivot`, `scalePivot`, `rotatePivotTranslate`, `scalePivotTranslate`, `transMinusRotatePivot`, `center`, `boundingBoxMin`, `boundingBoxMax`, `boundingBoxSize`, `constraintTranslate`, `constraintRotatePivot`, `constraintRotateTranslate`, `targetTranslate`, `targetRotatePivot`, `targetRotateTranslate`, `targetScalePivot`, `targetOffsetTranslate`, `restTranslate`, `localTranslate`, `localPosition`, `cutPlaneCenter`, `centerOfIllumination`, `centerOfInterest`, `focusDistance`, `nearClipPlane`, `farClipPlane`, `orthographicWidth`, `distance`, `point1`, `point2`, `falloffRadius`, `falloffCenter`, `maxDistance`, `minDistance`, and sizes on primitives. The attribute fixtures check these (`doubleAngle`, `doubleLinear`) once captured.

## Reference and checks

- The reference's published command reference (flags, short names, create/query/edit modes, return values) and node reference
  (node types, attribute long and short names, defaults) are the spec.
- Every command and node gets tests written from that spec. Behaviour the reference does not pin down is checked
  against the reference by Simon and recorded in the test.

## Scene model

- DAG with transform and shape nodes: `pCube1` (transform) over `pCubeShape1` (mesh). Shapes are DAG leaves.
  `listRelatives -s`, instancing (one shape under several transforms) and DAG paths (`|grp|pCube1|pCubeShape1`) behave
  as the reference's.
- Dependency graph: plugs, connections, dirty propagation and lazy evaluation (the existing graph core).
- Construction history: creation nodes (`polyCube1.output -> pCubeShape1.inMesh`) and one poly node per edit
  (`polyExtrudeFace1`, `polySplitRing1`, `polyTweak1`, ...) inserted upstream of the shape, each with
  `inputPolymesh`, `output` and `inputComponents`. The `constructionHistory` toggle and `-ch` flags decide whether an
  edit adds a node or writes the shape's own mesh. `delete -ch`, `bakePartialHistory`, `polyEvaluate`.
- Component addressing: `pCube1.vtx[0:7]`, `.e[]`, `.f[]`, `.map[]`, `.vtxFace[][]`, `*`, `[1:3]`; `ls -fl`,
  `filterExpand`, selection of components through the transform or the shape.
- Default naming: `pCube1`, `pCubeShape1`, `polyCube1`, `joint1`, `ikHandle1`, `skinCluster1`, ... with the reference's
  numbering rules.
- Attribute names, short names and defaults as the reference's (`translateX`/`tx`, `rotateOrder`, `jointOrient`,
  `segmentScaleCompensate`, `inheritsTransform`, `displayHandle`, ...).

## Commands and scripting

- Command layer with the reference's create, query (`-q`) and edit (`-e`) modes, flag names and short names, multi-use flags,
  and the reference's return values.
- Embedded CPython. The official module is `aztek.cmds`; a `the_reference` package shim maps `maya.cmds`, `maya.mel` and
  `maya.api.OpenMaya` onto Aztek's, so simple scripts run unchanged.
- Ground truth: `tools/reference_capture/` writes fixtures from a running copy of the reference (calls, attributes per node type, runtime commands, hotkeys, `.ma` scenes, posed results). None has been captured yet, so expectations remain hand-written readings. `reference_fixtures.rs` compares against a capture where there is one, and `tests/data/reference_open.txt` lists what still differs.
- Batch: `aztek --batch` (also `-batch`, `-file`, `-script`, `-command`) opens a scene and runs scripts with no window and no Vulkan, and `about -batch` answers true. Exit 0, 1 on an uncaught error or a failed deferred call, or `sys.exit`'s value. A failed step stops the steps after it. Output comes out as it happens. Unverified: whether the reference's exit status counts a deferred error the same way.
- MEL: an interpreter for the subset scripts use (procedures, variables, arrays, control flow, command syntax,
  backticks, `eval`).
- OpenMaya API 2.0 subset: `MSelectionList`, `MDagPath`, `MObject`, `MFnMesh`, `MFnTransform`, `MFnDependencyNode`,
  `MPoint`/`MVector`/`MMatrix`/`MTransformationMatrix`, `MItMeshVertex`/`Edge`/`Polygon`, `MGlobal`, and Python
  plugins through `MFnPlugin` with `MPxCommand` and simple `MPxNode`s. Compiled C++ plugins cannot load.
- Script editor with Python and MEL tabs, history, echo all commands.

## Order of work

1. Scene model: transform and shape split, construction history nodes, naming, component addressing.
2. Command modes (create, query, edit) in the command layer; the reference's signatures on the modelling and UV commands.
3. Modelling and UV commands to complete the reference's Mesh, Edit Mesh, Mesh Tools and UV menus.
4. Rigging: joints (orient, mirror, insert, segment scale compensate), IK (`ikSCsolver`, `ikRPsolver`, spline IK, pole
   vectors), constraints (point, orient, parent, scale, aim, pole vector), `skinCluster` with weight painting and
   mirroring, `blendShape`, set driven keys, `nurbsCurve` controls.
5. Animation: `animCurve` nodes, keyframes and tangents, time slider, graph editor, playback.
6. Python with `aztek.cmds` and the `the_reference` shim, then MEL, then the OpenMaya subset.
7. Application: ash viewport, egui shell; Alt-drag navigation, QWERTY tools and manipulators, marking menus, hotbox,
   channel box, attribute editor, outliner, shelf, preferences.
8. Files: Aztek's own scene format; `.ma` read and write for exchange.

## Status

Step 1 (scene model), as of 05/11/2026:

- Transform and shape split: every mesh is a `transform` over a `mesh` shape (`pCube1` / `pCubeShape1`); `locator`
  is a shape under its transform. Names and selections take either; edits resolve the transform to its shape, and
  results and component strings name the transform, as the reference does.
- Construction history: component edits insert a node per edit after the generator (`polyCube1 -> polyExtrudeFace1
  -> pCubeShape1`), its settings the command's flags and its components in `inputComponents` (`f[0:3]`). Changing a
  setting, or the generator's, re-runs the chain. A mesh with no history gets an intermediate `polySurfaceShape1` to
  feed the first node. Vertex moves on a mesh with history go into a `polyTweak`. `delete -ch` bakes and removes the
  history; deleting an object takes its history; `constructionHistory -toggle` turns recording off; `listHistory`
  lists the chain; primitives take `constructionHistory=False`. History saves and loads.
- Edits with history nodes: extrude (faces, edges), inset, bevel, bridge (not along a path curve), delete components,
  insert edge loop (`polySplitRing`), split rings, offset loop, merge, merge to centre, collapse, dissolve, poke, spin
  edge, fill hole (`polyCloseBorder`), grid fill, flip (`polyNormal`), conform, triangulate, quadrangulate
  (`polyQuad`), soft/hard edge, duplicate faces, connect, weighted normals, lock and unlock normals.
- Component strings: `select("pCube1.f[0:2]")` with add, deselect and toggle; `ls(sl=True)` returns the current
  mode's components as ranges, `flatten=True` one per item.

Polygon command set, as of 06/11/2026 (`crates/aztek_scene/src/mesh_edit/ref_poly*`, `tests/ref_poly.rs`): the reference's
command names run natively with all their flags, taken from the reference's tables. Component arguments are read by
the command and leave the selection alone; commands return their history node, or [transform, node]. Query and edit
reach each node's attributes by the reference's names, short names included (`polyExtrudeFace1.ltz`). Primitives:
`polyCube`, `polyPlane`, `polySphere`, `polyCylinder`, `polyCone`, `polyTorus`, `polyDisc`, `polyPipe` answer to the
reference's attribute names, old names as aliases; `polyPrism`, `polyPyramid`, `polyHelix`, `polyPlatonicSolid`,
`polyPrimitive`, `polySoccerBall`, `polySuperShape` are new. New or now native with a node: extrude facet, edge and
vertex (with divisions, local and world moves, along a curve), `polyMoveVertex/Edge/Facet`, `polyChipOff`, `polyBevel3`,
`polySplitRing`, `polyMergeVertex/Edge/Facet`, `polyDelVertex/Edge/Facet`, `polyCollapseEdge/Facet`, `polyChamferVtx`,
`polyCircularize`, `polyConnectComponents`, `polySplitVertex`, `polySplitEdge`, `polyDetachEdges`, `polySubdivideFacet/Edge`,
`polyPoke`, `polyWedgeFace`, `polyDuplicateEdge`, `polyFlipEdge`, `polyHole`, `polyAverageVertex`, `polySplit` (`-ep -fp
-ip -pc`), `polyProjectCurve`, `polyAppend`, `polyCreateFacet`, `polySmooth`, `polyReduce` (`-p -trm -vct -tct`),
`polyRemesh`, `polyRetopo`, `polyMirrorFace`, `polyUnite`, `polySeparate`, `polyCBoolOp`, `polyBoolOp`, `polyBooleanCmd`,
`transferAttributes` (live), `polyNormal`, `polyNormalPerVertex`, `polySetToFaceNormal`, `polyAverageNormal`,
`polyColorPerVertex`, `polyColorSet`, `polyUVSet`, `polyMoveUV`, `polyMergeUV`, `polyLayoutUV`, `polyCopyUV`, `polyForceUV`,
`polyContourProjection`. Without nodes: `polyInfo`, `polySelect`, `polySelectConstraint` (state, `-q`, `-rs`, `-pp`),
`polySelectSp`, `polyListComponentConversion`, `filterExpand`, `polyCleanup`, `polyCleanupArgList`, `polyClipboard`,
`polyPasteFace`, `polyOptions` (a table for scripts), `meshReorder`, `polyPinUV`.

Falls short of the reference: `polySeparate` makes one node per shell rather than one with several outputs;
`polyExtrudeFacet -inc` keeps the curve's samples, so a later change to the curve doesn't follow; `polyWedgeFace` and
`polyMirrorFace` have no smoothing; `polySmooth` on part of a mesh relaxes the new points rather than running Catmull-Clark,
`-mth 1` ignores `-ps -ro`, and the `osd` flags are accepted only; `polyHole` marks faces but they still draw;
`polyContourProjection` is a planar projection unfolded; `polyCreateFacet` and `polyAppend` take no holes; `-roundCap` is
accepted and the ends stay flat; `polySubdivideFacet -m 1` is one fan per level, from memory, not checked against the
original. Mixed `-ep` and `-fp` run edges first, since flags don't keep their order across kinds.

Still destructive (they bake the history before them): target weld, symmetrize, bridge along a path, `meshReorder`,
`polyPinUV`, `polyClipboard -ps`, `polyCleanup` when it fixes, `polyTransfer` of vertices and colours, edits under radial
symmetry, vertex moves with symmetry on (the symmetry pairs live on the stored mesh). Node names that differ from the reference's where Aztek's edit has no twin in the reference:
`polySplitRingsFace/Edge`, `polyOffsetLoop`, `polyMergeCenter`, `polyDissolveVertex/Edge`, `polyBevelFace`,
`polyBridgeFace/Edge`, `polyGridFillVertex/Edge/Face`, `polyConnectVertex/Edge`, `polyDuplicateFaces`,
`polyWeightedNormals`, `polyUnlockNormals`, `polySpinEdge`.

Next: the remaining destructive edits.

Application, as of 06/11/2026: Qt Widgets shell over the Rust core (`cmake --build build`, see README.md). Menus by
menu set (Modeling, Rigging, Animation), status line, shelves (Poly Modeling, General, Rigging, Animation, UV Editing,
Custom), toolbox and layouts, four views with model panel menus, outliner, channel box with the reference's channel colours,
picked channels and the middle-drag virtual slider, layer editor, attribute editor, script editor, command line, help
line, time and range sliders with key ticks, playback, marking menus, hotbox, option boxes, preferences, the reference's
hotkeys. Viewport: Alt navigation, picking with pre-selection highlight, marquee, edge loops on double-click, move,
rotate and scale manipulators, smooth preview, soft selection falloff colours, joints, IK handles, locators and
deformer handles.

Animation, as of 06/11/2026: `animCurveTL/TA/TU` nodes with auto, spline, clamped, linear, flat, step, plateau and
fixed tangents and infinity modes; `setKeyframe`, `keyframe` (query and edit), `cutKey`, `keyTangent`,
`findKeyframe`, `currentTime` (frames), `playbackOptions`, `autoKeyframe`; changed channels hold until the time moves,
as in the reference. Graph Editor with key picking, dragging, tangent buttons and stats.

Animation in full, as of 06/11/2026: tangents as the reference holds them, a vector of seconds and value, with angles in value per
second; weighted curves are cubic Beziers in time and value (`keyTangent -wt -iw -ow -ix -iy -ox -oy -l -wl -unify -r`),
unweighted ones Hermite. `keyframe`, `setKeyframe` (`-i` keeps the curve's shape), `cutKey -o`, `selectKey` with their
remaining flags. `rotationInterpolation`, `filterCurve` (euler, butterworth, gaussian, keyReducer, keySync, peakRemover,
resample, simplify), `simplify`, `bufferCurve`, `keyframeStats`, `mute`, `setAttr -k`. `motionPath`, `pathAnimation`,
`snapshot`, `motionTrail` (drawn in the viewport). `animLayer` with blend node chains, keys resolved through the stack,
`mergeAnimLayers`. `character`, `currentCharacters`, `clip`, `clipSchedule`, `pose`. `playbackOptions`, `play`,
`timeControl`, `currentUnit`, ghosting as scene state. Graph Editor menus: infinities, tangents, Euler filter, simplify,
resample, buffer curves, stacked and normalized views, isolate, copy, paste, snap, retime hotkeys.

Not yet: tangent handles dragged in the Graph Editor; ghosts drawn; `-cp` keys no control points, which have no
channels of their own; the Trax Editor. Bank amount and the unweighted-to-weighted
conversion of fixed tangents are our reading, not checked against the reference, as are the tangent formulas beyond what its
documentation states. Clips play through a `clipChannel` node per channel, which the reference does not have.

UVs, as of 06/11/2026: maps (`pCube1.map[3]`) and shells derived from the per-corner UVs and seams; `polyProjection`
(planar, cylindrical, spherical), `polyAutoProjection`, `u3dUnfold`, `u3dLayout`, `polyMapCut`, `polyMapSew`,
`polyMapSewMove`, `polyEditUV` (query too), `polyNormalizeUV`, `polyFlipUV`, `polyRotateUVs`, each a history node
(`polyPlanarProj1`, `polyTweakUV1`, ...). UV Editor.

Skeletons, IK and constraints, as of 06/11/2026: `joint` with its query and edit flags (limits, degrees of freedom,
preferred angles, zero scale orient), joint labels, segment scale compensate through `inverseScale`, `mirrorJoint`,
`insertJoint`, `removeJoint`, `connectJoint`, `disconnectJoint`, `reroot`. `ikHandle` query and edit, a real `ikEffector`,
`ikSolver`, `ikSystem`, `.ikBlend`, `.twist`, `.roll`, chains of any length, solving from the preferred angles within
joint limits. Handle `weight`, `poWeight`, `priority`, `stickiness` and joint `stiffness` are stored but don't change the
solve. Constraints: parent, point, orient, scale, aim, `poleVectorConstraint`, `geometryConstraint`, `normalConstraint`,
`pointOnPolyConstraint` with `-e`, `-q`, `-rm`, skip flags, offsets, rest position, `interpType`, `nodeState` and weight
attrs `<target>W0`. Constraint nodes
are not DAG children of what they drive, as they are in the reference: the graph orders whole nodes, so that would be a cycle.

Rigging and deformers, as of 06/11/2026: `joint` (create, edit with orient, query), `orientJoint`, the joint tool,
`skinCluster` (closest distance weights, unbind, influence query), `skinPercent`, `parentConstraint`,
`pointConstraint`, `orientConstraint`, `scaleConstraint`, `aimConstraint` (with maintain offset), `ikHandle` (RP and SC
solvers, pole vector, twist), `nonLinear` (bend, twist, squash, flare, sine, wave) with handles, `cluster`,
`blendShape` with a weight per target. Everything saves and loads.

MEL, as of 06/11/2026: a real interpreter (`crates/aztek_host/src/mel/`). Comments, all five types with arrays and
coercions, globals, top-level variables that stay between runs, operators, control flow, procedures (array arguments by
reference, local to their file), command and function syntax, backticks, `eval`, `source` and auto-sourcing from
`AZTEK_SCRIPT_PATH` or `MAYA_SCRIPT_PATH`, `catch`, `python`, `whatIs`, `exists`, `about`, and the standard string, math
and array functions. Multi-use flags read their arity; query mode gives a flag a value only by a per-command table.
Differs from the reference: unknown procedures and undeclared variables fail when reached, not before the script starts; errors
carry a line number only for syntax errors; an array of one element converts to a scalar on assignment.

Deform menu, as of 06/11/2026: `lattice` (`ffd`, `lattice`, `baseLattice`; `ffd1Lattice.pt[s][t][u]` select, move and list with
`ls -fl`; local influence, outside modes, freeze mapping, `-e -remap`, `-e -resetLattice`), `softMod` (selection or centre,
volume or surface, `falloffCurve` ramp), `deltaMush`, `tension`, `wrap` (with `deformer -type wrap` and `wrap -e -inf`), `wire`,
`deformer` (`-type`, `-q -g`, `-q -gi`, `-e -g`, `-e -rm -g`), `percent`, `reorderDeformers`, `listHistory -type`. Deleting a deformer
closes the chain and takes its handle or lattice. Differences: a deformer on several meshes is one chain node per mesh with the
first node's settings connected in, so `ls` shows ffd1 and ffd2 where the reference has one node. `wire` follows the driver's points as a
polyline, so a NURBS curve follows its control points until a sampler is added; the base wire is data on the node, with no
`curve1BaseWire` shape.

Deform and skin weights, as of 06/11/2026. `blendShape`: `.weight[i]` with target aliases (`aliasAttr`, `listAttr -m`), `-t`, `-w`,
`-ib`, `-ibt`, `-tc`, `-o`, `-par`, `-foc`, `-e -rm -t`, `-rtd`, `-q -w -wc -t -g -en -o -tc`, live targets baked on delete, per-point
base and target weights (`.inputTarget[0].baseWeights[v]`, `.inputTargetGroup[t].targetWeights[v]`), `sculptTarget`, `invertShape`,
Shape Editor. `sculpt`, `jiggle` (stepped a frame at a time, seen frames kept), `shrinkWrap`, `morph`, `wrinkle`, `proximityWrap`,
`solidify`, `pointOnCurveDeformer` (`deformer -type pointOnCurve`), `textureDeformer` (procedural value plus `.texture`), `curveWarp`.
`artAttrCtx`, `artSetToolAndSelectAttr`, the Paint Attribute tool with its Tool Settings page and Deform > Paint Weights. Skin:
`-bm 2` real heat diffusion with `-hmf`, `-sm 2` with `.blendWeights` and `skinPercent -dqb`, `deformerWeights` (XML, JSON; index,
nearest, barycentric, bilinear, over), `weightHammerVerts`, `copySkinWeights -uv`, `transferBetweenInfluences`, `-ai -ug`,
`substituteGeometry`, `-mjm`, `removeUnusedInfluences`, `-nw 2`, Hold shown in the paint tool. `pCubeShape1Orig` as a real
intermediate shape (`listHistory -pdo`, `deformableShape`, Outliner Display > Intermediate Objects), `cluster1Set` as real membership
(`sets`, `deformer -e -g`, Edit Membership Tool).

Differences and gaps: `blendShape` origin defaults to local, where the documentation says world and the option box sends local.
`-par` makes no parallelBlender node: the blendShape sits last, measured from the original, which adds the same way. wrinkle is one
node with a handle, not a cluster over wires, and creases by normal. solidify fits the whole mesh, so several shells count as one.
shrinkWrap's `.projection` enum order is from memory and needs checking against the reference. `copySkinWeights -uv` reads the one UV set.
`transferBetweenInfluences`, `attrPaintStamp` and `pointOnCurveDeformer` are Aztek's own names. A skinCluster has no set, so
membership doesn't limit a skin. `.ma` doesn't carry the new deformers. Not done: `poseInterpolator`.

Files, as of 06/11/2026: `.ma` read and write (`ma_file.rs`) and the `file` command (`-new`, `-open`, `-save`, `-rename`,
`-import`, `-exportAll`, `-exportSelected`, `-type`, `-q -sn -ex -type`). Reads transforms, joints, locators, meshes
(`.vt .ed .fc`, UV sets, `.pt`, hard edges), poly creators, `polyTweak` over stored geometry, animCurves (T and U kinds,
tangents, infinities), skinCluster, dagPose, constraints, RP and SC ikHandle, display layers and user attributes. Other
history is taken from stored geometry; what cannot be represented is a warning naming its node type. Writes the same,
meshes baked. Written from knowledge of the format with no copy of the reference to check against; `.kit`/`.kot` codes, `.kix`/`.kiy`
units and `.xm` xform order are the parts to check first. Not handled: references, instances, NURBS, blendShape and
cluster (baked with a warning), multiple shapes sharing a skinCluster.

Python API, as of 06/11/2026: `aztek.api.OpenMaya` and `aztek.api.OpenMayaAnim`, also as `maya.api.*`: value classes
and arrays, `MGlobal`, `MSelectionList`, `MObject`, `MDagPath`, `MPlug`, modifiers, function sets for nodes, transforms,
meshes (rays and closest points included), curves, sets, attributes, data and components, the selection, DAG, graph and
mesh iterators, `MFnSkinCluster`, `MAnimControl`, `MFnAnimCurve`. Python plug-ins with `loadPlugin`, `unloadPlugin` and
`pluginInfo`: `MPxCommand` (runs from `cmds` and MEL, `undoIt` and `redoIt` on the undo stack) and `MPxNode` for plain
dependency nodes. Differences from the reference: only dependency nodes can come from plug-ins (no `MPxDeformerNode` or
`MPxLocatorNode`), mesh attributes don't reach `compute`, compound attributes can't be arrays, one UV set (`map1`), one
path per DAG node, `setKeyable` only on added attributes, `MFnTransform` has no shear, and cycles are refused node by
node, not plug by plug. MEL can't pass flags to a plug-in command with no syntax. `MRampAttribute` isn't there.

Python API 1.0, as of 10/11/2026: `maya.OpenMaya`, `maya.OpenMayaAnim` and `maya.OpenMayaMPx`, built on the 2.0 classes
above, so it covers the same classes. Each 1.0 class is its 2.0 class with the 1.0 calls on top: results handed back
through the arguments (`sel.getDagPath(0, path)`, `mesh.getPoints(points, space)`, `it.getConnectedVertices(ids)`),
`MScriptUtil` pointers for numbers (`asIntPtr`, `asDoublePtr`, `asDouble3Ptr`, the `get`/`set` and `ArrayItem` calls,
`createMatrixFromList`), the long `closestIntersection`/`allIntersections` argument lists, the `getX()` spellings
(`getTranslation`, `getRotatePivot`), calls where 2.0 has properties (`numVertices()`, `fnAttr.name()`,
`setKeyable(True)`, `isKeyable()`), `length()` and `set(value, index)` on arrays, `m(row, col)` and `m[row][col]` on
matrices, `MStatus` and `MCommandResult`. A plug-in without `maya_useNewAPI()` loads as a 1.0 plug-in, registered
through `OpenMayaMPx.MFnPlugin` with `asMPxPtr` creators. `MDagPath`s compare by the path they lead to and follow their
node through renames and reparenting. Not there in either API: `MMessage` and its callback classes, `MFnNurbsSurface`,
`MFnCamera`, `MFnLight`, `MFnIkJoint`, `MFnLattice`, the geometry data sets (`MFnMeshData` and the array data sets),
`MItCurveCV`, `MItSurfaceCV`, `MAnimUtil`, `MFileIO`, and the `MPx` classes beyond commands and plain nodes.

NURBS surfaces, as of 06/11/2026: `nurbsSurface` shape, CVs as `surface1.cv[u][v]` (select, move, undo, tweaks on top of
history), drawn shaded with isoparms and a CV hull in vertex mode, `selectType -cv`, `displaySmoothness` and the 1, 2, 3
keys. `sphere`, `nurbsCube`, `cylinder`, `cone`, `torus`, `nurbsPlane` with `make*` nodes. `loft`, `revolve`, `extrude`
and `planar` make NURBS, polygons with `-po 1`; `boundary`, `square`, `singleProfileBirailSurface`, `rebuildSurface`,
`reverseSurface`, `insertKnotSurface`, `detachSurface`, `attachSurface`, `offsetSurface`, `closeSurface`, `nurbsToPoly`
(`nurbsTessellate`), `duplicateCurve` for isoparms, `pointOnSurface`, and the `surfaceInfo`, `closestPointOnSurface` and
`pointOnSurfaceInfo` nodes. Differs from the reference: non-rational, so a sphere is the reference's cubic approximation only; `-ut` and `-tol` on primitives
are accepted and ignored; `loft` takes degrees 1 to 3; `attachSurface` copies its second surface rather than following
it. None of this has been compared with a running copy of the reference: the sphere's CV values, which way U runs on `loft` and `extrude`,
the direction flags' numbering and the bare commands' axis of 1 0 0 are from memory of the reference's files and docs.

Audit of the command list, as of 07/11/2026. NURBS (`nurbs_ops.rs`, `nurbs_components.rs`): `filletCurve`, `circularFillet`,
`freeFormFillet`, `curveIntersect`, `intersect` (sampled from the tessellation), `curveOnSurface` (surface->curve names),
`projectCurve`, `projectTangent`, `offsetCurveOnSurface`, `trim` and `untrim` with real trims (kept on the surface as (u, v)
loops, drawn and tessellated, saved in both formats; `.ma` as an `aztekTrim` string, so the reference keeps but doesn't draw them),
`planar` and `planarSrf` trim non-rectangles, `stitchSurface` (boundaries only, copies its partner when run),
`stitchSurfacePoints`, `globalStitch`, `extendSurface`, `duplicateSurface`, `surface`, `plane` (a square curve),
`squareSurface`, `smoothTangentSurface`, `hardenPointCurve`, `makeSingleSurface`, `nurbsSelect`, `bevel` and `bevelPlus` (one
surface per wall; `bevelPlus` caps are in the polygon output, or trimmed surfaces under a group), `textCurves` (one stroke
font, Create > Text), `twoPointArc`, `threePointArc`, `pointCurveConstraint`, components `curve1.ep[0]`, `curve1.u[0.5]`,
`surface1.ep[0][1]`, `.u[]`, `.v[]`, `.sf[][]` (edit points move; the rest select and answer). `nurbsBoolean` returns a polygon
mesh. Not here: `roundConstantRadius` (needs NURBS solids to trim back).
Polygons (`ref_poly/more_cmds.rs`): `polyCut`, `polySewEdge`, `polyDuplicateAndConnect`, `polyMoveFacetUV`, `polyOutput`,
`polyCheck`, `polyCompare`, `polyClean`, `polySmartBevel`, blind data (numbers only), `polyColorMod`, `polyColorDel`,
`polyAverageColor`, `polyBlendColor`, `polyGeoSampler` (the mesh's own colours, no lights), `moveVertexAlongDirection`,
`symmetricModelling` with `-topoSymmetry` from a selected seam edge, on the Symmetry dropdown. A topological partner goes to
the mirror image rather than moving by the mirrored delta. Which side `polyCut -df` deletes (the normal's) is from memory.
UVs (`uv_misc.rs`): `polyMapDel` (corners always hold a UV: they go to 0), `polyEditUVShell`, `polyOptUvs`, `polyUVRectangle`
(squares the border on its bounding box, not from two edges), `polyUVOverlap`, `polyUVCoverage` (512 a side),
`polyUVStackSimilarShells`, `polyStraightenUVBorder`, `u3dAutoSeam`, `u3dOptimize`, `untangleUV`, `uvSnapshot` (PNG with its
own encoder, or PPM), `polyEvaluate -b2 -bc2`.
Rigging (`rig_misc.rs`): `bindSkin` and `rigidBind` (a skinCluster with one influence a point), `geomBind` (`-bm 3` runs the heat
bind), `pairBlend`, `matchTransform`, `inheritTransform`, `container`, `controller`, `annotate`, `distanceDimension`,
`paramDimension` (drawn with labels), `findDeformers`, `copyDeformerWeights` (closest point). `jointCluster` holds settings and
softens nothing; `dropoffLocator` marks the place and doesn't change the wire. Not here: `flexor`, `combinationShape`,
`skeletonEmbed`, `weightsColor`.
Animation (`anim_misc.rs`, `characters.rs`): `timeWarp` (linear between its frames), `setKeyPath`, `selectKeyframe`,
`tickDrawSpecial`, `recordAttr` (kept for scripts; no `record`), `bakeClip`, `clipMatching`, `bakeSimulation`, `animDisplay`,
`keyingGroup`, `characterMap`. Not here: `characterize`.
System (`system_cmds.rs`): `upAxis` (see Up axis), `cycleCheck`, `convertUnit`,
`copyAttr`, `evaluationManager` and `threadCount` (settings only), `dgInfo`, `dgtimer` (zeros), `displayColor`, `displayRGBColor`,
`colorIndex`, `displayLevelOfDetail`, `displayPref`, `displayStats`, `displaySurface` (all kept for scripts, not yet drawn by),
`listNodeTypes`, `listLights` (empty), `selectPriority`, `selectedNodes`, `findType`, `objectCenter`, `nearestPointOnMesh`,
`closestPointOnMesh` with U and V from the mesh's UVs, `hitTest` (empty outside the UI), `isDirty`, `isTrue`, `getModulePath`,
`encodeString`, `attributeName`, `attributeInfo`, `transformCompare`, `lockNode` (a locked node can't be deleted or renamed).
References (`references.rs`): `file -r -ns`, `reference`, `referenceQuery`, unload, reload and remove. Edits are attribute values
and connections with the scene, kept as plain setAttr and connectAttr lines in `.ma` rather than as reference-node edits, so
The reference reads them but not as edits. References are flat: one inside a referenced file becomes its own. Instances, `container`,
`distanceDimShape` and locks go through `.ma`; an instance reads back as a mesh of its own fed by the first.

Utility nodes, expressions and core commands, as of 06/11/2026: `multiplyDivide`, `plusMinusAverage`, `condition`,
`clamp`, `setRange`, `reverse`, `blendColors`, `blendTwoAttr`, `remapValue`, `distanceBetween`, `angleBetween`,
`vectorProduct`, `pairBlend`, `decomposeMatrix`, `composeMatrix`, `multMatrix` and the other matrix nodes, the quaternion
nodes, `blendMatrix`, `aimMatrix`, `pickMatrix`, the float, colour and modern math nodes, `closestPointOnMesh`
(`utility_nodes.rs`). Connections depend on plugs rather than nodes, so `pCube1.tx -> node -> pCube1.ty` runs; array
elements have components (`input3D[0].input3Dx`) and can drive. `expression` node and command (`expression.rs`) over real
connections. Transforms: `offsetParentMatrix`, `shear`, limits and `transformLimits`, `inheritsTransform`, inverse
matrices, `rotateQuaternion`, bounding box attributes, `hiddenInOutliner`. UUIDs and namespaces. Full flag sets for the
core commands (`ref_attrs.rs`, `ref_xforms.rs`, `ref_scene.rs`).

Differences from the reference in these: a connection that truly feeds a plug back into itself (`a.tx -> b.tx -> a.tx`) is refused, where the reference connects it and warns when it evaluates (ticket). `parentMatrix` read through a connection includes `offsetParentMatrix` (getAttr gives the reference's value). Expression
numbers are all doubles, so `1/2` is 0.5. `parent -add` makes an instance through `instance`. `closestPointOnMesh` leaves
`parameterU` and `parameterV` at 0. Limits apply when the node computes, so a plug reading a limited channel directly can
see the value before the limit. The defaults of `multiplyDivide.input1`, `condition.colorIfTrue` and `floatMath.floatB`
were written from memory with no copy of the reference to check against. In MEL a 0 or 1 after a
switch of move, rotate or scale is a value, not the switch's own, so `move -r 1 0 0` reads as it should.

`.ma` files keep utility nodes and expressions, and read the reference's slot spelling of an expression (`.O[0] = .I[0] * 2;`). On
reading, a `unitConversion` is seen through, its factor dropped, since Aztek's angles are degrees throughout.

Not yet in these: `displayCull`, `snapMode` and the display attributes (`displayHandle`, `displayLocalAxis` and so on) hold their values
but the viewport does not draw from them.

(As of that date. All of these have since been done; see below.)

Interface, as of 06/11/2026: named actions callable from hotkeys, MEL and Python (`runTimeCommand`, `nameCommand`,
`hotkey`), an editable hotkey table in the Hotkey Editor, saved in settings. Status line: selection scope, masks
(`selectType`), snap toggles beside the X, C and V hold keys (grid, curve, point, view plane, live), lock and highlight
selection, symmetry dropdown, soft select, numeric input and select by name. Selection: lasso, paint selection, Tab-drag,
Shift+double-click loops and rings, Shortest Edge Path Tool, All by Type, Similar, Shell, Shell Border, Quick Select Sets.
Manipulators: plane handles, Ctrl-click constraints, view ring, trackball, numeric readout, J discrete rotate, step snap,
component scale along the manipulator's axes, B-drag sizing the falloff; W, E, R, Q + left press menus; right-click menus
by object type with history inputs and Paint Weights on a joint. Hotbox with zones. Shelves saved between sessions, Shelf
Editor, middle-drag from the Script Editor, Ctrl+Shift+click on a menu item, `shelf_*.mel` import; Curves / Surfaces and
Sculpting shelves. Windows: Attribute Spread Sheet, Component Editor with skin weights, Connection Editor, Hypergraph,
Namespace Editor (listing only), Channel Control, Modeling Toolkit, Shape Editor,
workspaces, main pane only, panel layouts, saved layouts, a panel holding an outliner or editor. Panel menus: cameras,
`lookThru`, bookmarks, camera tools, gates, safe action; flat, bounding box, X-Ray and its joint and component kinds,
backface culling; Show filters, `isolateSelect`. Heads up display and `headsUpDisplay`; `polyOptions`. In-View Editor and
the channel box's virtual slider on INPUTS. Attribute Editor sections by node type, sliders, Notes, Copy Tab. Outliner
filters and icons. Layer editor V, P, T, R, colour, reorder. Script Editor tabs, highlighting, line numbers, save and
source. File, Edit, Modify and Display menus; Preferences saved and `optionVar`; time units by name; playback speed and
looping in real time.

As of 11/11/2026:

- Shift-drag on a move, rotate or scale handle makes something new and drags that: selected faces or edges are
  extruded (`polyExtrude`), objects duplicated. The extrude or duplicate and the drag are two undo steps.
- Shift + right-click with nothing selected: the polygon primitives of the Create menu and the Create Polygon Tool,
  made as the menu's items make them. The items' compass positions are unchecked against the reference.
- Ctrl+Shift + right-click, laid out from screenshots of the reference's Move tool and Select tool menus. With
  something selected and Move, Rotate or Scale active: Symmetry, Object, Component, World, Snap, Axis, Keep Spacing
  (Move) and Select round the compass; Selection Constraints, Transform Constraints, Shift Extrude, Shift Duplicate,
  Preserve UVs, Preserve Children, Tweak Mode, Update Triad (Move) and the tool's Options below. Otherwise: Symmetry,
  Paint Select, Lasso, Select, and the two constraint menus. The items set the same context flags as Tool Settings,
  which shows them (Shift Extrude, Shift Duplicate and Update Triad are new there). It replaces a primitives menu
  that was on these keys. Not checked against the reference: what the submenus hold (Symmetry, Select, Snap, Axis and
  both constraint menus are filled from Aztek's own settings), and the Rotate and Scale menus, built on the Move
  tool's layout. Selection Constraints holds Off and the Use Constraints window, not the reference's list. Update
  Triad and the transform constraint are stored but the tools do not act on them yet.
- The UV Editor's Checker also shows on the selected meshes in the views (shaded, not wireframe), as well as behind
  the UVs. One checker, eight squares to a UV tile; `checkerColor1`, `checkerColor2` and `checkerDensity` are accepted
  and not applied.
- Outliner Display > DAG Objects Only starts on. Off, every other node is listed after the DAG: a skin's bind pose,
  skin clusters, materials, history nodes. Sets show either way.
- `uiPref name value` from MEL and Python stores the value (it stored nothing before, so the settings the Preferences
  pass through it, the drag readout among them, now take effect).

Not yet in the interface: Topology symmetry (World X, Y, Z act in object space: `setSymmetry` has no world mode), Cut,
curve Edit Point and Curve Point modes, UV and Vertex Face component modes in the marking menu,
per-object display toggles are not saved with the scene,
Outliner reorder between siblings. Roll stays in the view, not the
camera node.

Scripts that build their own interface, as of 07/11/2026. The ELF commands (`window`, the layouts, the controls and
their groups, menus, `textScrollList`, dialogs, `progressWindow`, `workspaceControl`, `uiTemplate`) create, edit and
query against a model in the core (`aztek_host/src/scriptui.rs`), so they work without an interface; `app/src/scriptui.cpp`
mirrors it in Qt. Names and `|` paths as the reference returns them, `setParent` with `..`, `$gMainWindow` and `$gMainProgressBar`.
Callbacks in MEL (`#1` and `%1` take the value) or Python: callables get what the reference passes, strings from Python are
Python, one undo step each. `scriptJob` (events, conditions, attributes, nodes, controls; fired at idle by comparing
state), `scriptNode` (saved in both file formats, run on open and close), `evalDeferred`, `executeDeferred`, `timer`,
`stackTrace`, `getLastError`, and `warning`, `error`, `print`, `whatIs` in `cmds`. `getPanel`, `modelPanel` (also inside a
script's layout), `modelEditor`, `panel`, `scriptedPanel`, `lookThru`, `viewFit`, `viewSet`, `viewLookAt`, `viewCamera`,
`viewPlace`, `viewHeadOn`, `viewClipPlane`, `dolly`, `orbit`, `roll`, `track`, `listCameras`, `playblast` (image sequences
through the view). Tool contexts: `setToolTo`, `currentCtx`, `resetTool`, `ctxEditMode`, `contextInfo`, the manip contexts
and about thirty Ctx commands.

Events with no source in the core (`linearUnitChanged` and the like) are accepted and never fire. Contexts with no tool
yet warn once and stay on Select: IK handle, spline IK, curve sketch, poly cut, select edit, skin bind, soft mod, and
`texSmudgeUVContext` and `texLatticeDeformContext` in the UV Editor.

### More script-built interface (07/11/2026)

Attribute-bound controls (`attr_ui.rs`). `connectControl` (with `-index`, 1 being a group's label) and `attrFieldSliderGrp`,
`attrFieldGrp`, `attrControlGrp`, `attrColorSliderGrp`, `attrEnumOptionMenuGrp`, `attrEnumOptionMenu` pick the plain control
the attribute calls for and bind it. The control shows the plug and follows it at idle, drags preview silently, and letting
go leaves one echoed, undoable `setAttr`. A script's `-e -v` on a bound control sets the plug. Fields take the channel box's
colours for keyed, on a key, connected, changed, muted and locked.

Controls: `floatSliderButtonGrp`, `symbolButton`, `shelfTabLayout`, `scriptTable` (cells from `-getCellCmd`, `-cellChangedCmd`
may refuse an edit), `layerButton`, `toolButton` and `toolCollection` (they set the tool and follow it), `toolHasOptions`,
`hudButton`, `hudSlider`, `hudSliderButton` (drawn and working in the views, removed by `headsUpDisplay -rem`), `timePort`,
`rangeControl`, `timeRangeControl`. Objects: `itemFilter` with `lsThroughFilter`, `selectionConnection`, `panelConfiguration`
(`getPanel -allConfigs`, `-configWithLabel`), `editorTemplate` (an `AE<type>Template` procedure lays out the Attribute Editor's
tab for that type), `baseTemplate`.

Editors a script makes are the real Qt editors in its layout: `outlinerEditor`, `outlinerPanel`, `nodeEditor`,
`dopeSheetEditor`, `animCurveEditor`, `blendShapeEditor`, `blendShapePanel`, `componentEditor`, `spreadSheetEditor`,
`textureWindow`, `keyframeOutliner`. Made with no parent, `-e -p` puts them in a layout and `-up` takes them out.
`outlinerPanel1` and the like answer under the reference's names; of those only the main Outliner hears edits so far.

Tools. Views: `orbitCtx`, `boxDollyCtx`, `walkCtx`, `modelCurrentTimeCtx`, `curveAddPtCtx`, `curveMoveEPCtx`, `curveBezierCtx` (a
Bezier curve tool), `twoPointArcCtx`, `threePointArcCtx`, `polySelectCtx`, `polySuperCtx`, `polyCreaseCtx`, `polyMergeEdgeCtx`,
`polyMergeFacetCtx`, `createPolyCubeCtx`, `createPolyPlaneCtx`, `snapTogetherCtx`, `showManipCtx`, `setEditCtx`,
`softSelectOptionsCtx`, `scriptCtx` (`$Selection1`, `ctxCompletion`), `userCtx`, `artPuttyCtx`, `artSetPaintCtx`,
`artUserPaintCtx`, `artAttrPaintVertexCtx`. Graph Editor and Dope Sheet: `graphDollyCtx`, `graphTrackCtx`,
`graphSelectContext`, `directKeyCtx`, `currentTimeCtx`, `curveEditorCtx`, the key tools and the `keyframeRegion*Ctx` family. UV
Editor, by `setToolTo`: `texMoveContext`, `texRotateContext`, `texScaleContext`, `texSelectContext`, `texMoveUVShellContext`,
`texCutContext`, `texSmoothContext`, `texTweakUVContext`, `texSelectShortestPathCtx`, `textureLassoContext`, `texWinToolCtx`.
Commands: `tumble`, `manipPivot`, `manipOptions`, `setXformManip`, `pixelMove`, `viewManip`, `panZoom`, `ctxAbort`.
`manipMoveContext -ah` and its like reach the manipulator: a middle drag off the handles takes that handle.

Menus and layouts: `-optionBox` puts the box on the row of the item before it. `workspaceControl` and `dockControl` dock in
the main window (`-dtm`, `-dtc`, `-dtp`, `-ttc`, `-a`, `-fl`). `paneLayout -ps`. `-dgc` and `-dpc` work with the middle button,
as in the reference. `playblast -fmt qt`, `avi` or `movie` writes a frame sequence and runs ffmpeg, with a plain error if none is on
the PATH.

Less than the reference does, in these:

- `attrControlGrp -e -a` onto another type of attribute makes the control again at the end of its layout. Bound controls have
  no right-click key menu and no map button; `-extraButton` is held and not shown.
- `editorTemplate`: `-callCustom` is recorded and not run, and `-addControl` names only what the editor lists, so `translate`
  and not `translateX`. `-dimControl` is recorded and does nothing. `baseTemplate` keeps names and reads no files.
- `layerButton` ignores `-color`. `timePort` always shows the global time. `scriptTable` asks for every cell (to 2000 rows, 200
  columns) rather than those drawn; `-sortEnabled` and the cell colours are held only.
- `hudSlider` has no `-sliderIncrement` stepping or frame. `selectionConnection -switch`, `-addScript` and `-removeScript` are
  held; the Outliner is the only editor that reads a connection or filter. `-filter` on the other editors is held only.
- `poseEditor` and `posePanel` are a list of pose interpolator nodes, as there is no pose system. `clipEditor` warns and opens
  the Dope Sheet. `keyframeOutliner` lists times and values. `nodeEditor` acts on `-rfs`, `-addNode` and `-rootNode`; the rest are
  held. `componentEditor` and `spreadSheetEditor` flags are held, and the widgets go by the selection.
- `walkCtx`: dragging only, no WASD. `snapTogetherCtx` moves the object and does not turn it to match the normals.
  `createPolyCubeCtx` and `createPolyPlaneCtx` take their size from the mouse and ignore their other flags. `curveMoveEPCtx` is
  the move tool and does not switch component mode itself.
- `scriptCtx` has one selection set: `-totalSelectionSets` and the masks are held. `panZoom` moves the camera, as there is no
  2D film pan, and `-absolute` is held. `viewManip` shows and hides the corner axes; there is no cube. `manipPivot -o` is held,
  and `setXformManip` is held only.
- The paint contexts `artSetPaintCtx`, `artUserPaintCtx` and `artAttrPaintVertexCtx` paint by selecting: each point the brush
  takes gets the action once, at full value, and stays selected. No brush falloff or opacity reaches the action.
- `texSmoothContext` is one unfold on a press, not a drag. Tool buttons exist for four of the UV tools; the others have none lit.
- The key tools are one setting shared by the Graph Editor and the Dope Sheet, and the UV tool is shared by every UV Editor.
  The Dope Sheet's set key tool inserts at the frame rather than taking a value.
- `-dtc` beside a dock in a tab group joins the group, which is Qt's rule. That is the Outliner, tabbed with Tool Settings.
  `-dtp` docks to the side of the main window. There are no draggable separators in `paneLayout`.

### .ma deformers (07/11/2026)

Deformers save and load in `.ma` as the reference lays them out: Orig shape, groupParts and groupId, the deformer, its objectSet, `.wl` weights, `.gm`. blendShape uses `.it[g].itg[i].iti[k].ipt/.ict`, `.tw`, `.bw` and `attributeAlias`; live targets connect at `.igt`. cluster and nonLinear handles are a transform with the reference's shape (clusterHandle, deformBend and so on), with nonlinear settings on the shape. A deformer on several meshes reads from `.ip[n]`/`.og[n]` into one node per mesh, with the followers linked to the leader. Point moves in a `tweak` node go into the Orig geometry; a polyTweak below a deformer stays in the chain. Data that the reference's attributes can't hold goes in an added string attribute, aztekState. Other history below a deformer still bakes, with a warning.

To check against the reference: the `.cc` lattice layout, whether a file opens without a set for every deformer, the skinCluster's own groupParts, and scripted reading of a real rig.

## A real rig (07/11/2026)

Tested against a production rig made in the reference (2020): a character of 1,588 transforms, 414 joints, 37 skinClusters, 17 blendShapes, 102
follicles, 82 IK handles, ribbons, wires and pose readers. `crates/aztek_scene/examples/load_ma.rs` loads any `.ma` with no UI and
reports warnings and how far the result is from what the file says: influences against their bind matrices, channels against the values
The reference stored, meshes against their originals. `rig_diff.rs` compares two files under a pose. `tests/rig_files.rs` holds each fault found,
from small `.ma` files written by hand.

Works on that rig: opens in about 3 seconds with nothing dropped but render settings and polyBridgeEdge history (its meshes hold their
geometry); every stored translate comes out as saved, and every stored rotate except on 63 eyelid joints, which are up to 0.51 degrees off (cause not found); body, arm IK and FK, legs, spine, head, jaw, lips,
cheeks, brows and blink all reach the final meshes; 13 to 44 ms per change; saving and reopening gives the same result under every pose
tried, to the last digit.

Added or fixed for it:

- A node connected to itself no longer hangs the graph.
- `follicle`, on surfaces and meshes: X along U, Y along V, Z the normal. It may drive its own transform (`free_of_parent` on its type).
- `transformGeometry`, makeNurbCircle and makeNurbPlane in files; curve `.cp[k]` drivable; `blendWeighted` and `pointOnCurveInfo` read.
- Deformers on curves and surfaces: `nurbsToPoints` and `pointsToNurbs` carry the CVs through skinCluster, blendShape, wire and tweak.
  They are extra nodes in `listHistory`, which the reference does not have. They are not saved: files hold the reference's own connections.
- Constraints go to the target's rotate pivot; aim, normal and tangent work in the parent's frame; a mirrored matrix is a rotation with
  every axis scaled by -1. Weights can be driven (space switches), world up objects connect, rest values read and write, pairBlend between
  constraint and object.
- Pole vectors are in the space of the start joint's parent. A file's stored pole is the chain's rest reference.
- Spline IK from files, and it keeps rootOnCurve.
- Live blendShape targets from meshes (`.w`). Importing no longer pins a driven plug at the value in the file.
- The default cameras exist as soon as a file opens, take the file's values, and are written with `-s`.

Shallower than the reference:

- Nothing here has been compared with the reference running. The checks are against values the reference saved at rest; posed results are only checked to be
  plausible and the same after saving. Whether the reference opens what Aztek writes is unchecked.
- Follicle hair, simulation and most of its attributes are ignored: it is a point and frame. `pu` and `pv` are clamped to 0 to 1.
- Spline IK stretch comes only from what the rig wires up. Advanced twist: see the next section.
- blendWeighted ignores `.w`. A periodic curve's skin weights cover its first lap only.
- A mesh named the same as its transform needs a path: `hair|hair`, as in the reference.
- A wire reads driver curves by sampling; `.sc` (scale) on a wire is not used.
- A rest error of 5.3 on one mesh of that rig is unexplained. It is probably the blend targets the file has on at weight 1, but this has not been shown.
### Three more production rigs (07/11/2026)

A second production rig (the reference (2023), 11,936 nodes), a third (the reference (2020), 4,516 nodes, 161 MB, its spine on a ramp and matrix nodes) and a fourth (the reference (2019), AdvancedSkeleton style, 8,146 nodes, 68 expressions). None of their data is in the repo; each fault below has a small hand-written file in `tests/rig_files.rs`. `load_ma` now also compares constraints with the rotation the reference last stored (`.lr`), which are posed results the reference computed. `rig_pose` tries controls and says what moved and how long it took.

| | second | third | fourth |
|---|---|---|---|
| open, before | 47.4 s, 4050 warnings | 11.6 s, 88 | 7.7 s, 200 |
| open, now | 5.3 s, 0 | 3.7 s, 0 | 4.6 s, 2 |
| influences off their bind, before | 126 of 244 | 202 of 975 | 67 of 411 |
| now | 0 | 10, explained below | 0 |
| rest error on final meshes, now | 0.000 | 0.000 but the tongue | 0.000 |
| stored channels that differ, now | 15, none on a skin | 0 | 0 |

Faults found, all general:

- Opening ran an evaluation for every constraint; 3,500 passes took 42 of the 47 s. Evaluation is held while constraints are made.
- Dependency is closer to per plug. `parentMatrix`, `parentInverseMatrix` and `transMinusRotatePivot` depend on the parent or the pivots alone. An attribute added to a DAG node or a utility node, and a DAG node's visibility and display settings, order nothing and are filled in last. A position's constraint reads a target's `tt`, `trp`, `trt` and `tpm`, a scale constraint its `ts` and `tpm`. That removed every refusal of the form "would create a cycle" on these rigs.
- `message` exists on every node, shows in `listAttr`, and neither orders nor dirties anything. Added message attributes, multis among them, are read and saved. `controller` holds real connections, as the reference wires them.
- Long strings are written `("a" + "b")`, and the brackets were taken for the value, so most long expressions ran as nothing. `||` lexed as a DAG path; an expression can hold `setAttr x.y (expr);`, whose value is worked out and dropped.
- Target weights driven by a utility node were dropped without a word, as were `ikBlend` and most handle channels. `AZTEK_MA_DROPS=1` now lists what a file held that was not read.
- `unitConversion` was always skipped. A factor beyond radians and degrees is now kept: 0.1, -1, 2.5 degrees per unit. The node is written with the factor as Aztek reads it, so 0.0436 comes back as 2.5.
- Offsets (`.o`, `.tg[k].tor`) are in the driven object's rotate order. A scale constraint on a mirrored target has Z negative. An aim with a zero `wu` turns the shortest way. A constraint drives only the channels the file wires. `nodeState` and offset can be driven, and a node blocked from the start holds its rest values. `interpType` is read and saved.
- `.cp[k]` on a curve with history is a move on top of it. A curve fed straight by another curve keeps the connection. Deformer membership on curves (`cv[...]`) reaches the weights. `pointOnCurveInfo.top` is a share of the parameter range. Edit points exist (`eps[k].yve`). A wrap can be driven by a curve, each point keeping its offset from the nearest CV.
- `multiplyDivide.input1` starts at 0. `pickMatrix`, `aimMatrix`, `blendMatrix` and `setRange` have the reference's short names; a blendMatrix target has `useMatrix`. `pairBlend` has its modes. `ramp`, `motionPath`, `makeNurbsSquare`, `extrude` and `reverseSurface` come from files. Transform limits read and can be driven (`mntl`, `mxtl`, `xtxe`). `.pa` is read and saved, where saving used to lose it. A single chain handle's rotation means its difference from the file's pose.
- Spline IK advanced twist: up types, world up axis, vectors and matrices, Total, Start/End and Ramp values. The bone's way out of the joint counts as forward, whatever `dForwardAxis` says, as a mirrored chain at rest shows.
- `createColorSet` passes through. `deleteComponent` and `polyDelEdge` apply to the stored mesh. Renderer nodes (RenderMan, Arnold) are quiet and left out on saving. Images are also looked for in `Textures` beside the scene.

Looked at: all four rigs posed through arm IK and FK, legs, spine, head, jaw and fingers, each reaching the final meshes, and back at rest to 0.0000. Screenshots at rest and posed, textured, under Xvfb. Saved and reopened: 0 warnings on the second and third, and on the fourth 0 where it had 2, as what it can't represent is dropped by the save.

Shallower than the reference, most noticeable first:

- The fourth rig's `poseInterpolator` nodes are not read, so the corrective shapes on elbows and wrists don't fire. Nothing stands in for them and they are lost on saving.
- Its `SquintRightToLeftBS`, a blendShape on a lattice, is not read: the left squint lattice doesn't follow the right. A lattice has no deformers on it here.
- Per change, measured over 30 changes: 13 to 127 ms on the second and third rigs and 29 to 206 ms on the fourth, the top of each range being the root control. A change to an attribute that computes nothing, such as a visibility switch on the main control, still recomputes everything below that node: 65 to 90 ms there.
- Saved and reopened, the fourth rig under an arm IK pose differs by 8 on one proxy mesh's transform and 0.1 on twist joints, the third by 0.09 on a shoulder under a spine pose, the second by 0.05 on a finger joint under an FK arm pose. Cause not found. Other poses tried agree to 0.01.
- The second rig has 15 stored rotations that differ, on spline IK joints not on a skin: up to 11 degrees on one thumb guide, 95 on one wrist joint. The wrist's up locators reach a rotation the reference's `.lr` disagrees with by 120 degrees, unexplained: the constraint's parent would have to be ignored for the reference's number to come out.
- `.lr` is sometimes old: a constraint at weight 0 keeps the last value. Those differences are noise, and the check only points at places to look.
- The third rig's tongue is 204 units off its bind at rest. Its constraint's `.lr` holds the same 25 degrees, so the reference had it there too.
- `dForwardAxis` and the Closest up axes are not used as such; closest is taken as positive. `setAttr` in an expression has no effect. Faces and edges deleted by history are matched by number against the stored mesh, which has only been seen on proxy meshes.
- `distanceDimShape` labels draw even when the shape's parents are hidden.

### Remaining rigging, animation, NURBS and general commands (07/11/2026)

Flags and short names come from the reference's 2024 command documentation; no copy of the reference was run. In `rig_extra.rs`, `anim_extra.rs`, `model_extra.rs`, `general_cmds.rs`, `nurbs_ops.rs` and `aztek_host/src/host_general.rs`.

Working as the reference describes them: `combinationShape` with its node (the smooth method's formula is our own, the reference doesn't document it), `blendTwoAttr`, `instanceable` (`instance` refuses, not `duplicate -ilf`), `effector`, `ikSystemInfo`, `jointDisplayScale` and `ikHandleDisplayScale` (the views read them), `expandedSelection`, `dataStructure`, `addMetadata`, `editMetadata`, `getMetadata`, `nodeCast`, `sysFile`, `getFileList`, `stringArrayIntersector`, `condition` (also through `isTrue` and `scriptJob -ct`), `disable`, `getProcArguments`, `clear` and `print` and `warning` from `cmds`, `hotkeyCheck` (keys bound with `hotkey`, not the shell's built-in table), `help` (from each command's registered text).

Shallower than the reference:

- `flexor`, `copyFlexor`, `jointLattice`, `boneLattice`: the node bulges the lattice's points by the bend, and `.ma` keeps them. The bulge's shape is our own. The lattice follows the joints through constraints and its ffd runs after the skin, where the reference's node moves the lattice's ends rigidly. Of `-dc` only `-dv` and `-mxd` are read. A sculpt flexor follows its joint and no attribute drives it. `copyFlexor` doesn't reflect a mirrored bone.
- `skeletonEmbed` is a shallow heuristic: a straight chain along the bounds' longest axis through the middle of each slice of points. It embeds no skeleton, and `-sm` and `-sr` change nothing. `-jc` is ours.
- `characterize` makes a character from names and `-sk`. `-type label` reads names too, and the pivot, effector and stance flags fail, as there is no full body solver. `hikGlobals` is only held. `hikGetDependNodes` has no the reference documentation: what it returns (joints, then connected nodes) is inferred from its name.
- `setKeyframeBlendshapeTargetWts` keys `targetWeightKeys` elements on the blendShape, which override the painted weight: the reference's per-point weights under `inputTarget` aren't plugs here. `.ma` keeps them under that name, which the reference won't read as weights.
- `ghosting` sets the ghost attributes and answers about frames, but the views draw no ghosts, so `-en`, the colours and the opacities have no effect.
- `blend2` takes one curve or edge per rail. `-pt`, `-tt` and `-mk` are only held. `roundConstantRadius` makes the fillets but doesn't trim the surfaces or build corners; compound edges and `-a` fail; the sections are cubics, up to about 0.2% of the radius off a true arc, and assume the surfaces are flat at each section.
- `nurbsCurveToBezier` keeps a `nurbsCurve` shape with tripled knots and a marker attribute, not the reference's `bezierCurve` type, as `.ma` writes curves by that name. `bezierAnchorState` and `bezierAnchorPreset` move the tangents once: no state is kept, and later drags don't keep them smooth or even. Preset tangent lengths are our own.
- `nurbsUVSet`, `nurbsCopyUVSet`, `nurbsEditUV`: one explicit set per surface, implicit values evenly spaced by CV rather than from the knots, and nothing draws from them.
- `polyInstallAction` knows a table of actions; only `polyCloseBorder` has a constraint and a display.
- `grid`: size, spacing, divisions and toggle reach the four model panels, as a line every spacing over divisions. Darker lines stay every 5, and new panels don't get the settings. Defaults are 12, 5, 5 as the views and Preferences have them; the reference's text says spacing and divisions of 10.
- `commandEcho -st` echoes commands from Python only. `nodePreset` writes JSON and leaves out multis and hidden attributes; `-ctm` does nothing. `nodeCast` doesn't refuse referenced nodes.

Stored with no effect, as their command text says: `weightsColor`, `ikfkDisplayMethod`, `xformConstraint`, `relationship`, `displayAffected`, `timeCode`, `makePaintable`, `uvLink`, `setAttrMapping`, `toggleAxis -o`, `grid`'s label, axis and division line flags, `help`'s popup settings, `hideDisplay` on effectors. Only `instanceable`, metadata on nodes, explicit UVs and the Bezier marker are saved in files; structures, scene metadata and the rest last for the session.

Not done, and why:

- No documentation from the reference in the 2016 to 2025 references, and nothing found on what they do, so there is no behaviour to copy: `proxyWire`, `setDrawOverride`, `meshIntersectTarget`, `mouldMesh` (real, listed by PyMEL in 2011 with no arguments). `createCurveWarp` has no page either; it is the Deform menu's item and runs `curveWarp`.
- Not commands of the reference, as far as its documentation shows: `birailOp`, `reparent`.
- `blendCtx`: no page, and Ctx commands are separate work. `getDefaultBrush`: Paint Effects, out of scope. `surfaceSampler`: bakes texture maps, render only.
- `selectKeyframeRegionCtx`-free helpers: the keyframe region commands are all contexts; no non-UI ones were identified.

Fixed along the way: `setKeyframe` and `keyframe` on an array element such as `blendShape1.w[0]` took it for a component and keyed the node's own channels (the envelope); `freeFormFillet` and `projectTangent` took a surface's point for its U derivative.

Found and left: after `select shape`, `ls -sl` still lists the earlier components, and `select -cl` leaves the component lists in place. The reference clears them.

### Lights in the viewport (07/11/2026)

Nodes (`aztek_scene/src/lights.rs`): `light`, `renderLight`, `nonAmbientLightShapeNode` and `nonExtendedLightShapeNode` (abstract), and on them
`ambientLight`, `directionalLight`, `pointLight`, `spotLight`, `areaLight` and `volumeLight` (built on `pointLight`), a transform over a shape, with the
names, short names and defaults of the 2024 node reference, so `ls -lights` and `ls -type light` find them all. Attributes that only a renderer samples
are there for scripts, hidden. `listNodeTypes` and `getClassification` answer `light`.

Commands (`light_cmds.rs`): `directionalLight`, `pointLight`, `spotLight`, `ambientLight` with create, query and edit and the flags of the command
reference, returning the shape; `shadingNode -asLight`, returning the transform; `listLights`; `lightlink` with `-light -object -make -b -query -sets -shapes
-transforms -shadow -useActiveLights -useActiveObjects`; `CreateAmbientLight` to `CreateVolumeLight`; `defaultAmbientLight` to `defaultVolumeLight`. A light
made by any of them joins `defaultLightSet`; one from `createNode` doesn't, and lights nothing until linked. A duplicate keeps its place in the set. A light
lights an object if a link says so and no break says otherwise, or else if it is in the set. Links are on a `lightLinker1` made when first needed, and undo.

`.ma` (`ma_file/lights.rs`): attributes with short names, `.ltd` to `:lightList1.l`, `.iog` to `:defaultLightSet.dsm`, `relationship "link"` and `"ignore"`
lines with their shadow forms. A volume light's `colorRange` and `penumbra` ramps are kept as read and written back.

Viewport (`aztek_host/src/lighting.rs`, the material shader in `render.rs`): Use Default Lighting, Use All Lights (7), Use Selected Lights, Use Flat Lighting,
Use No Lights, Two Sided Lighting, and `modelEditor -displayLights` with `default`, `all`, `selected`, `active`, `flat`, `none`, `-twoSidedLighting`,
`-lights`, `-shadows`. 5 and 6 go back to default lighting; 7 keeps the textures. Up to 16 lights in one forward loop, the first in the scene past that. Lambert
diffuse from each, multiplying the texture as before; decay 0 to 3; a spot's cone, penumbra either way and dropoff; ambientShade; `emitDiffuse`, `emitSpecular`;
links per shape; hidden lights don't light. blinn and phong highlights from `specularColor`, `eccentricity`, `specularRollOff`, `cosinePower`. Default lighting
takes the shader's old path: the same picture to the pixel as before lights were added, with materials and transparency. It has no highlights, as before.

Shaders stay GLSL compiled by naga at start-up, so no shader tool is needed on either platform and CI is unchanged. Vulkan's 256 bytes of push constants were
full, so each view has a uniform buffer of lights and one of highlights, as descriptor set 2 of the material pipelines. A test compiles every shader without a device.

Interface: Create > Lights with option boxes; icons drawn in the view in the display colour, with selection colours, picked by clicking, on the rendering
selection mask; Show > Lights; outliner icons; Look Through Selected, a spot through its cone; a panel button for all lights; Attribute Editor sections with
Type, which swaps the shape and keeps what the two types share, and Illuminates by Default; colour channels in the Channel Box read R, G, B.

Checked: `aztek_scene/tests/lights.rs`, `aztek_host/tests/lights_mel.rs`, and pictures of hand-made scenes: each light type, decay rates, a spot's edge, highlights,
emit flags, each mode, two sided both ways, a textured sphere under a coloured light, linking, hotkeys, menus, a click on an icon. Nothing has been compared
to a running copy of the reference, and no file from the reference with lights has been read: the test files are written by hand from the documented layout.

Not in the reference, so from memory and unconfirmed: the arguments of the `default*Light` procedures (intensity and colour are used, and for spot and ambient
lights their own values) and what they return; `listLights`; "flat" as a `-displayLights` value, which the reference doesn't list; `selected` meaning the same as
`active`, where the reference may hold on to the lights selected earlier; 5 and 6 returning to default lighting; the file's `shadowLink` and `shadowIgnore` words; the
direction of a cone volume light; the enum names of `decayRate`, `lightShape`, `volumeLightDir` and `fogType`, which the reference doesn't give. It gives
`useRayTraceShadows` as on by default, followed here, though the interface of older versions showed it off.

Shallower than the reference:

- No shadows: `-shadows`, depth map and ray trace settings are stored and saved, not drawn.
- An area light is the rectangle's nearest point to the surface, lit from one side and fading with the angle; no real area integral, and its highlight is a point's.
- A volume light fades in a straight line from its middle to its boundary. Its ramps are kept and not read; `arc` and `coneEndRadius` for a cylinder are not drawn.
- Highlights: the blinn formula is the textbook one and not checked against the reference's. phongE, anisotropic and standardSurface draw a blinn of about the same width.
- No image based light, reflections, bump, fog, barn doors, decay regions, or light colour from a texture.
- A link is per shape: one made to a shading group holds for shapes the group holds whole or in part, not per face.
- X-Ray draws with default lighting. A device with under 256 bytes of push constants draws flat colours, as it did.
- No spot light cone or penumbra manipulators, no Show Manipulator tool for lights, no Light Linking editor. Use Default Material under lights has no highlights.
- `lightlink -hierarchy` is accepted and has no effect. Past 16 lights nothing says some were left out.

### Materials, texturing and viewport textures (07/11/2026)

On the real rig above: `Body.png` and `Eyes.png` draw on the character from beside the scene despite their `D:/` paths, the missing `waitress.bmp` gives one warning, the textures stay on the skin when arm IK, arm FK and fingers are posed, and saving and reopening keeps the members of `initialShadingGroup`, NURBS ribbons included. Opening a file keeps the camera it saved rather than framing everything.

Nodes (`aztek_scene/src/shading.rs`): `lambert`, `reflect` (abstract), `phong`, `blinn`, `phongE`, `anisotropic`,
`surfaceShader`, `standardSurface`, `shadingEngine` (an `objectSet`), `materialInfo`, `file`, `place2dTexture`, `bump2d`,
with the reference's long and short names and defaults; `ls -type lambert` finds the ones built on it. Every scene has `lambert1`,
`initialShadingGroup` and `initialMaterialInfo`, which can't be deleted or renamed. A new mesh joins
`initialShadingGroup` in the undo step that made it; a duplicate keeps its material.

Membership is a shading group's member list (shapes, or a mesh's faces, one group to a face), mirrored in real
connections: `shape.instObjGroups[0]` or `shape.instObjGroups[0].objectGroups[k]` to `SG.dagSetMembers`. Scripts that
connect or disconnect those by hand move the member list with them. Members follow history and direct edits. A face no
group names, as an extrusion's sides, takes the group of its neighbour, the group with fewest faces first.

Commands: `shadingNode`, `sets -renderable -noSurfaceShader -empty -name`, `sets -e -forceElement` on objects and faces,
`sets -q -renderable`, `hyperShade -assign -objects -shaderNetworksSelectMaterialNodes -listUpstreamNodes
-listDownstreamNodes`, `defaultNavigation -connectToExisting`, `ls -mat -tex`, `listSets -type -allSets -object
-extendToShape`, `listConnections`, `getAttr`, `setAttr`, undo. `select` of objects now replaces picked components.

`.ma` (`ma_file/shading.rs`): `.iog` and `.iog.og[k]` to `.dsm`, faces from `.iog[0].og[k].gcl` or the groupParts behind
its groupId, `.gid`, `.gco`, `.gn`, `.ss`, materials and textures attribute by attribute with short names, `select -ne
:lambert1`, and on writing what the reference's file also has: `.pa` to `:renderPartition.st`, `:defaultShaderList1.s`,
`:defaultTextureList1.tx`, `:defaultRenderUtilityList1.u`, `lightLinker1` and its `relationship` lines. Exporting a
selection takes its materials along.

Texture paths: as written; relative to the project and the scene; what follows `sourceimages/` in the path; then by file
name, any case, beside the scene and in the sourceimages folders. A missing or broken image is one warning in the script
history, and the material draws the file node's default colour.

Viewport: `textures.rs` decodes png, jpeg, tga, bmp and tiff in Rust, uploads with a mip chain, and keeps one cache per
device keyed by path and colour space, reloading when the file's time changes. Triangles are batched per shading group, so
per-face assignments draw; smooth preview keeps them. Shaded (5) draws each material's colour, diffuse, ambient and
incandescence; Textured (6, Shading > Hardware Texturing, the panel button, `modelEditor -displayTextures`) samples the
colour's file through coverage, translateFrame, rotateFrame, repeat, offset, rotateUV, wrap and mirror, with colorGain and
colorOffset. Transparency, flat or from a file's alpha (luminance when the image has none), blends after everything solid,
far meshes first. `modelEditor -useDefaultMaterial` draws grey. A shape no group holds draws green.

Interface: Lighting/Shading menu; Assign New Material, Assign Existing Material and Material Attributes on the object
marking menu, applying to picked faces in face mode; Attribute Editor tabs for the shading groups and materials of a mesh,
colour swatches, a texture slot that browses for an image and builds file and place2dTexture, goes to what is connected,
and breaks the connection on right click; the file node's path with browse and where it was found. Windows > Rendering
Editors > Hypershade: materials, textures and utilities with swatches; create, assign to selection, select objects,
attributes, graph, rename, delete. The UV Editor draws the image behind the UVs, with an Image button to hide it.

Checked: the tests in `aztek_scene/tests/shading.rs` and `aztek_host/tests/shading_mel.rs`; screenshots of hand-made
scenes and of one real rig. None of it has been read back by a real copy of the reference.

Shallower than the reference:

- Under default lighting every material is lit by the same two fixed lights, with no highlights. Blinn and phong highlights and real lights: see Lights in the
  viewport. No reflections or bump.
- Only `uv0` is sampled; `uvChooser` and other UV sets aren't. No `layeredTexture`, ramp or procedural textures: a node
  Aztek doesn't know is reported "not represented", and a shader it doesn't know draws as `lambert1`.
- A file reached through utility nodes draws the file alone, ignoring what they do. `stagger`, `noiseUV`, `filterType`,
  `exposure` and `invert` are stored and not drawn. A material's alpha map uses its colour map's placement when it has both.
- `colorSpace` is sRGB or not; no OCIO, no view transform. Frame sequences and UDIM tiles aren't read.
- `file.outColor` as a plug is the default colour: the node never samples its image outside the viewport.
- Transparent meshes sort as wholes, not by triangle, and don't sort within a mesh.
- The shape's instance number is not kept: membership is per shape, and an instance draws its source's material.
- `polyUnite` and `polySeparate` don't carry per-face assignments: the result goes in `initialShadingGroup`. `createNode
  mesh` also joins it, where the reference leaves the mesh unassigned.
- A written deformed mesh with per-face materials has `.gcl` but no groupParts in its chain: unchecked in the reference.
- No `Create Render Node` window or material favourites. Hypershade has no work area;
  Graph opens the Node Editor. Assign Existing is a plain popup, not a submenu of the marking menu.

## Drawing overrides and wireframe colours (07/11/2026)

The colour a control has in its file now carries through. Found on a production rig: `.ove` and `.ovc` on curve shapes were dropped on
reading, so every control drew the default blue. Names, short names and defaults are from the reference's 2024 node reference for dagNode and
displayLayer. `crates/aztek_scene/src/draw_style.rs` makes every decision, so `tests/draw_style.rs` tests them with no GPU; the viewport
only asks it.

Done:

- `drawOverride` (`do`) and its children on every DAG node, read, written, by long and short name: `ovdt`, `ovlod`, `ovs`, `ovt`, `ovp`,
  `ove`, `ovv`, `hpb`, `ovrgbf`, `ovc`, `ovrgb` with `ovcr` `ovcg` `ovcb`, `ovca`. Also `tmp`, `uoc`, `oc`, `obcc`, `wfcc`, `uocol`, `oclr`,
  `hio`, `sech`. `getAttr` and `setAttr` take `.do` as a compound.
- `displayLayer` as the reference has it: `color` is a palette index and 0 is none, with `overrideRGBColors`, `overrideColorRGB`,
  `levelOfDetail`, `shading`, `texturing`, `playback`, `enabled`, `identification`. The old `useColor` and RGB `color` are gone, and the
  `displayLayer` command takes `-c` index or `-rgb` r g b. Layers read and write all of it, and a shape can be a member.
- A layer works by its connection: while `layer.di` drives `node.do`, the node's override children read the layer's values and `setAttr` on
  them refuses, as on any connected plug.
- Drawn: index and RGB overrides, inherited from parents with the nearest winning; template and reference; shading, texturing, visibility,
  level of detail, hide on playback; object colours; selected, lead and child highlight colours. For curves, surfaces, meshes, joints,
  locators, IK handles, lattices, deformer handles. Template and reference refuse viewport picks, on the shape as well as the transform.
- One palette: `colorIndex` edits what is drawn, and `displayColor` and `displayRGBColor` change the type defaults, the lead and the template
  colour. They were stored and not read. The `color` command is new. `play` now tells hide on playback, which nothing called before.
- Attribute Editor: Drawing Overrides laid out as a section, greyed until enabled, index slider with swatch or RGB swatch. Display >
  Wireframe Color. Layer Editor swatch opens the 32 entries and RGB. Outliner: `outlinerColor`, templates dim. `toggle -template` from Display
  > Object Display now sets `.template`, not an override. `createDisplayLayer -e` is empty rather than edit mode.

Not confirmed against the reference, which we have no copy of. These are best readings, and the order is written at the top of `draw_style.rs`:

- Precedence: selection, then display type, then the nearest override colour, then object colour, then the type default. The reference
  gives the attributes but not this order.
- A child with its own colour keeps it under a coloured layer. A child in a normal layer of its own is normal under a template layer; a
  child with its own normal override is not (it stays template or reference, on the API's description of a templated path).
- Reference draws black whatever its override colour, and template draws in the template colour.
- Shading, texturing and level of detail count as off if off anywhere on the path.
- `overridePlayback` off hides during playback. The reference only says it overrides the ability to animate.
- `objectColor` 0 to 7 is palette entries 24 to 31. `color` writes on the node named and on a transform's shapes. Object colour looks at the
  shape and its transform, no further up.
- The child highlight colour and the selected template colour (index 20) are chosen by eye. So is the outliner's dimming of templates.
- RGB overrides draw as stored, with no colour management.
- `useOutlinerColor`, `outlinerColor` and `hiddenInOutliner` are not on the reference's dagNode or transform pages. Their names and short
  names are from memory, and the test rig stores none of them.

Shallower than the reference:

- Level of detail is a world aligned box for line items and an object one for meshes. Overrides don't reach component display or CV colours.
- A see-through material keeps its textures with texturing off. `overrideColorA` is stored and not drawn. `objectColorRGB` is stored and not drawn.
- `colorIndex` and `displayColor` aren't undoable, aren't saved as preferences, and `-rs` resets to factory. `displayColor -qi` and the 2024
  `colorIndex -atv -dor -uc` flags are not done.
- The Wireframe Color window's Index row sets drawing overrides on what is selected: unverified whether that is what the reference's does.
- A wheel over a combo box in the Attribute Editor changes it, which is how Qt behaves and predates this.

### Default cameras (07/11/2026)

`persp`, `top`, `front` and `side` with `perspShape` and the rest, in every new scene from start-up, with the positions, centres of interest, `.rnd`, image names and home commands a new scene saved by the reference holds (checked against the reference-written files in `tests/data/ref_files`). The camera shape has the node reference's stored attributes by long and short name, read and written in `.ma`; those past the view's own are kept and don't yet change a view. They can't be deleted until `camera -e -startupCamera false`.

## The feedback button (07/11/2026)

A project feature, not a behaviour of the reference application. "This doesn't feel right!" sits in the shelf tab row's corner, so it stays when tabs scroll, at any window width and with any shelf or menu set. Help has an entry, and `AztekFeedback` is a runtime command. `app/src/feedback.cpp` is the window, `crates/aztek_host/src/feedback.rs` the link and the zip.

GitHub has no API for attachments and a write token can't go in a public binary, so the report goes through the browser: the text in a `issues/new` link of at most 7,000 characters, the picture on the clipboard and as a PNG, the scene as `aztek-scene.zip`. A long description is cut in the link with a note, keeping the environment section, and Copy Full Text gets the whole of it. `AZTEK_FEEDBACK_DRYRUN=<file>` writes the link to the file and opens no browser or folder.

Shallower than it could be:

- Checked under Xvfb in dry-run mode only. The real browser, pasting into GitHub, a real upload, opening the folder with the file selected, and Windows have not been exercised.
- The screenshot covers the app's own windows, placed as they are on the screen and overlapping in no particular order. Under Wayland they go one under another (not exercised).
- Folders under `feedback/` in the data location are kept, and nothing cleans them up.

### References (07/11/2026)

`references.rs` rewritten against scenes written by the reference (`tests/data/ref_files`), whose world matrices match the reference's recorded values. Load state from `-rdi`, nesting, the reference's `.ed` edits and placeholders, failed edits kept, `referenceEdit`, `file -ir -cr -gr -lck -rpr -mnc`, `ls -rn`, Reference Editor, File > Create Reference. Three production rigs referenced twice save and reopen with no joint difference (`examples/ref_rig.rs`). Shallower: `.mb` files are read but not written; `-shd` and `exportEdits` not done; painted skin weights and mesh tweaks on a referenced mesh aren't kept as edits; a few IK solver values show as edits; a second reference of a large rig loads slower than the first.

## USD (07/11/2026)

Built on the native Rust crate `openusd` 0.8 and `openusd-schemas`, with no C++ library. `crates/aztek_usd`; `docs/USD.md` says how to
use it. The target is the reference's USD plug-in's commands and the Animal Logic USD plug-in's, neither checked against the real plug-ins: flags and names
come from their published docs. Compositional results were checked against Pixar's library, `usd-core` 26.11, run by hand and not in CI:

- Files Pixar's library wrote (usda and usdc: references, payloads, variants, instances, indexed UVs, subsets, skinning, time samples)
  and real assets from `usd-wg/assets` (Teapot, OpenChessSet, StandardShaderBall, ElephantWithMonochord, TextureCoordinateTest,
  AnimatedCube, RiggedSimple, CesiumMan, BrainStem) import with every mesh's world points equal to Pixar's reading within 0.001, skinning
  included. Two differ a little: BrainStem by up to 0.004 at a frame between samples, ElephantWithMonochord by up to 0.025 at its
  last frame. Joints key as Euler channels with linear tangents, and non-uniform scale on a joint loses shear.
- Files Aztek wrote (usda, usdc, usdz) validate clean under `UsdValidation`'s full set of validators, except a stage with several roots,
  which it flags for having no default prim. Pixar's skinning of them gives Aztek's own deformed points.

In: import and export, the proxy shape drawn in the viewport, picking, the Outliner, Channel Box and Attribute Editor on prims, manipulators
authoring xform ops, variants, payloads, activation, namespace edits, the Layer Editor, the commands of both plug-ins for layers, pull and
push, MayaReference prims, cameras following prims, `pxr` in Python, translators, events, a resolver, stats, collections, drop.

Shallower than the plug-ins, what a user would notice first:

- MaterialX and other non-preview looks draw grey in the viewport and read as plain materials. Only `UsdPreviewSurface` carries colour,
  opacity and a diffuse texture; emissive, normal maps and the rest aren't drawn.
- Subdivision meshes draw their hull, with no refinement. Polygons draw as fans, so concave faces can look wrong. Flat normals aren't
  worked out for `subdivisionScheme = none`.
- A selected prim shows a box, not its wireframe. A marquee takes prims by their centre. There's no point, edge or face selection in a
  stage, no pick mode for models or groups, and no prim hover highlight.
- Time slider, Graph Editor and Dope Sheet don't see time samples. `setKeyframe` authors samples, a channel takes one when it has any,
  and the Channel Box colours it; there's no curve to edit.
- Time-sampled points read their first sample on import: no point cache node. Animated points on a proxy do draw. On a proxy, a
  time-varying stage reads all its prims again per frame, which is slow for a big animated stage.
- Merge writes specs in place of what the edit target holds at the prim; children an arc brings in stay. Transform, geometry and bindings
  merge; a pulled material's edits don't.
- Editing inside a variant, a reference or an instance's prototype isn't offered: edit targets are layers only.
- `flatten` also flattens references and payloads, where the reference's USD plug-in's flattens the layer stack.
- `time` has no `time1` to connect to by default: with nothing connected, a proxy follows the frame. No `outStageData` consumers.
- `shareStage` shares by file; turning it off gives a separate stage rather than an unshared root under a session sublayer.
- `AL_usdmaya_ProxyShapeImportAllTransforms`, `chaser`, `jobContext`, `apiSchema`, `useAsAnimationCache`, `melPerFrameCallback` and
  `referenceObjectMode` are accepted or missing. Export of collections and render settings isn't there; schema translators import only.
- Opening a stage with many instances stalls for a moment once: 64,201 prims took 1.8 s in one step, of 4.8 s reading in all, as
  composition finds the prototypes. Not divisible from here.
- The `pxr` module is a subset in Python over JSON; whole arrays of a large mesh are slow, and there's no `UsdSkel`, `UsdLux` or
  `UsdShade.ConnectableAPI`.

## Show Manipulator, tool options and menus (07/11/2026)

None of this has been compared with a running copy of the reference. Handle layouts, in-view rows, option box labels and defaults are from memory unless said otherwise.

Show Manipulator (T, `showManipCtx`, `crates/aztek_host/src/node_manip.rs`). Handles write the node's own attributes. A drag previews silently and runs one `setAttr` on release, so the history shows one line and undo takes one step. Worked out with no GPU, so `tests/node_manips.rs` drives real presses, moves and releases. A poly edit run from a menu or hotkey brings its handles up straight away; the same command from a script does not.

- Extrude, Chip Off, Duplicate, polyMoveFace, polyMoveEdge, polyMoveVertex: `localTranslate`, `localRotate`, `localScale` arrows, rings and boxes in the node's own axes, with a box that swaps to world `translate`, `rotate`, `scale`.
- polyExtrudeVertex `length`, polyPoke `localTranslate`, polyMirror `mirrorPosition` with its plane, polyWedgeFace `wedgeAngle`.
- polyCube, polySphere, polyCylinder, polyCone, polyTorus, polyPlane: size handles.
- polyPlanarProj, polyCylProj, polySphProj: now hold `projectionCenter`, `rotate`, `projectionWidth`, `projectionHeight`, `imageCenter`, `imageScale`, `projectionHorizontalSweep`. The stored UVs stand until an attribute changes, then the node reprojects from them.
- bend, flare (with its missing `endFlareZ`), sine, squash, twist, wave on their own attributes; softMod `falloffCenter` and `falloffRadius` with its circle; sculpt; ikHandle `poleVector` and `twist`; camera `centerOfInterest`. Any other object gets translate arrows, which covers cluster handles and lattices.
- Picking a node or channel under INPUTS shows that node's handles; a highlighted channel lights its handle and a middle drag anywhere takes it.

In-view editor: rows named per node type (Thickness, Offset, Divisions, Keep Faces Together and so on), the labels drag as virtual sliders, the fields take typing, and they follow the handles. It no longer opens for every `poly` line a script prints. `polyBevel3 -fraction` is new, and a bare Bevel now goes by fraction 0.5.

Tool options. `manipMoveContext`, `manipRotateContext` and `manipScaleContext` take the flags and short names of the 2024 command reference. The modes are the reference's: move and scale 0 object, 1 parent, 2 world, 3 normal, 4 rotation axis, 5 live object, 6 custom, 10 component; rotate 0 object, 1 world, 2 gimbal, 3 custom, 10 component. Each tool keeps its own. Done: Move Normal (each point on its own U, V, N, labelled in the view; Modify > Transformation Tools > Move Normal Tool), custom axes (Set to Point, Edge, Face, Reset, `-oa`, `-oo`, `-aa`), relative and absolute step snap, J stepping with no step set, Shift+middle drag taking the axis it runs along, Tweak Mode, Preserve Children, Prevent Negative Scale, soft select's four falloff modes kept with `softSelect -e -ssf`. Tool Settings has foldable sections with a falloff curve editor and presets, saved between sessions. The W, E and R marking menus set the contexts, so the page and scripts agree.

Menus. Option boxes for constraints, Bind Skin, IK, the deformers, Extrude, Bevel, curves, surfaces and keys are laid out by command in `option_boxes.rs`, and a test checks every row is a flag of its command. An option box now keeps what its item passes, such as a projection's type. Added: Edit Mesh and UV sections with titles, Transform, Spin Edge Forward and Backward, Reorder Vertices, Project Curve on Mesh, Difference (B - A), Contour Stretch, Auto Seams, Merge UVs, Delete UVs, Optimize, Straighten, Mirror Deformer Weights, Export and Import Weights, Match Transformations, Hide Unselected Objects, Show All, Scale and Snap Keys, Bezier and arc tools, more primitives, Interactive Creation (dragging cube, plane, sphere, cylinder, cone, torus). Repaired: Cut, Non-Deformer History (`bakePartialHistory`), Grow and Shrink Along Loop, Bounding Box display.

Held only, read by nothing yet: Preserve UVs, Keep Spacing, Pin Component Pivot, Snap Pivot Position and Orientation, Use Manipulator Pivot, the Constraint row, Preserve Seam.

Not done, the ones a user would notice first:

- UV, Vertex Face, Multi component modes in the view and marking menu: disabled entries, as `ComponentMode` has no such modes.
- Soft select Global and Object are kept but act as Volume. World symmetry still mirrors in object space.
- Gimbal rotate draws world axes, and rotate and scale step snap are always relative. Universal Manipulator is missing.
- Extract makes a new object with no history node, so it has no handles. Bevel and Bridge have numbers but no handles in the view, from memory as the reference does. Wire dropoff locators, jiggle, wrap, blendShape and delta mush have Envelope only. Lattice has translate arrows on the whole lattice. motionPath markers, pointConstraint and the light cones have nothing.
- The extrude handles do not follow `offset`, `localRotate` or several separate regions: one set sits at the middle of them.
- Option boxes with no layout of their own still list flags, and some are empty: NURBS primitives, Curves such as Detach, several Surfaces, Unfold and Layout. Tools have no option boxes. Boxes for Mesh Reduce and Combine are raw flags.
- Missing menu items, by use: Joint Labelling, Rivet, UV Pin, Proximity Pin, Copy and Paste Vertex Weights, Mirror Skin Weights as an option box, Interactive Bind Skin, UV Set Editor, Split UVs, 3D Cut and Sew UV Tool, Smooth Proxy, Unsmooth, Slice, Hole Punch, Cut Out, the Deform > Edit submenus other than Lattice, Prune Membership, Display Intermediate Objects, Relationship Editors, Hold Current Keys, IK/FK Keys, the Audio and Cache menus, a top level Visualize.
- `polyBevel3`'s node is `polyBevel1`, with no test pinning anything else. From memory, not confirmed: `bevel.name` in the capture kit.

## Animation layers and HumanIK, 07/11/2026

Nothing here has been compared with a running copy of the reference. The layout comes from the node and command pages and from scenes saved by the reference on this machine: one layered scene, one with three characters. Those files stay out of the repo.

### Animation layers

`anim_layers.rs` (chains, values at any frame, solving, editing), `anim_layer_nodes.rs`, `anim_layer_cmds.rs`, `anim_layer_menu.rs`, `app/src/animlayers.cpp`, `tests/anim_layers.rs`.

- Nodes with the attribute names the reference's files use: `animLayer` (`mt lo sl ovrd wgth fgwt bgwt pwth ram sam oram slct pref pthg omte pmte sslo csol chsl cdly play bnds gh ghc coll`), `animBlendNodeAdditive*`, `animBlendNodeAdditiveRotation` by axis (`iax ibx ox`), `...Scale`, `...Boolean`, `...Enum`, `...Time`. Blends by the pages' formulas: `a*wa + b*wb`, `power(a,wa)*power(b,wb)` when multiplying, b unless the weight is 0 for boolean and enum. A key on additive scale is therefore an amount added, 0 for nothing, where this used to take 1 for nothing.
- Modes: additive, override, passthrough (off: opaque). Rotation by component, or by layer as concatenated quaternions, a slerp when overriding. Mute goes down to children. Solo is per level as the guide says. Weight can be keyed and everything solved at a frame reads that frame's weight.
- Every flag on the `animLayer` page, query and edit. `-copy`, `-copyAnimation`, `-copyNoAnimation`, `-extractAnimation` take a layer name as the page says, `-layeredPlug`, `-bestAnimLayer`, `-removeSelectedObjects`, `-addRelatedKG`, `-writeBlendnodeDestinations`.
- Keys: Selected, Last Active and Hybrid (default) modes (`animLayerOptions -km`), preferred first. With no layer selected a key now goes to the last active layer holding the channel, not the base, as the guide says of Hybrid. `setKeyframe -identity` is the Zero Key. A layered rotate keyed as it stands is solved once for its three axes.
- `animLayerMerge` (also `mergeAnimLayers`): `-mergeTo`, `-includeChildren`, `-resultLayerMode`, `-deleteBaked`, `-sampleBy`, `-smart`, `-fidelity`. `bakeResults -dl -bol -ral -rba -rwl`; by default baked layers are muted, as its page says.
- `layerEditorExtractObjects`, `layerEditorExtractNonLayered`, `layerEditorSelectObjects`, `layerEditorCopyLayer`, `layerEditorEmpty`, `layerEditorDeleteEmpty`, `layerEditorMoveToNewLayer`: what the menus do by the guide. These names are ours.
- Anim tab: the stack with active and affected dots, M S L G, weight slider with its key state, K, Zero Key, Weight Key, Layers, Options and Show menus, the same menu on right click, drag between rows to reorder and onto a row to parent. Channels in a layer with nothing to key on the active layer are tinted. Graph Editor and Dope Sheet list a curve per layer by the Show filter; the Time Slider shows the active and selected layers' keys.
- `.ma`: a layered scene saved by the reference read and written again keeps all 5061 layer connections, 616 blends and 1540 layer curves, indices of `.bnds` aside.

Shallower, most noticed first:

- Ghosts aren't drawn, as for ghosting in general: the button and colour are held. Key ticks don't take the layer's colour.
- Multiple selected layers: a key goes to the topmost, where the guide's plural may mean all of them. Not confirmed.
- By-layer concatenation order (`a * b`, the layer applied first in the joint's frame) is our reading; the page's formula doesn't settle it.
- An override layer with no keys holds what was under it when added, rather than following it.
- Extract Non-Layered makes an override layer so the result stays; the guide doesn't say what the base keeps. `-maxLayers` answers the largest integer: the page gives no number. `-excludeProxy` has nothing to exclude. Bake's `-smart` isn't in; merge's is. Channel box tint colour is ours. Export Layer, Package Into Assets, Exclusive Solo, Template Curves of Locked Layers, the layer drop-down by the Range Slider and Ghost options aren't in.
- `.dsm` isn't a connection here, as it would be a cycle: read past and written from the blends. `.bllm`, `.ri`, `.im` aren't kept.

### HumanIK

`hik.rs`, `hik_nodes.rs`, `hik_solve.rs`, `hik_slots.rs`, `hik_rig.rs`, `hik_commands.rs`, `app/src/hikwindow.cpp`, `tests/hik_retarget.rs`. The reference documents none of this beyond `characterize` and `hikGlobals`; the node pages are absent from its reference. Procedure names are as community scripts quote them, argument lists partly inferred.

- Nodes as a saved file shows them: `HIKCharacterNode` (joint `.ch` to `.Hips`, world T-pose in `.HipsTx`, `.HipsRx`, `.HipsSx`, `.HipsJointOrientx`, limits, `.InputCharacterizationLock`, `.propertyState`), `HIKProperty2State`, `HIKSolverNode`, `HIKState2SK`, `HIKSK2State`, `HIKFK2State`, `HIKRetargeterNode`, in the same connections. `HIKEffector2State`, `HIKPinning2State`, `HIKState2FK`, `HIKState2GlobalSK`, `HIKState2Effector`, `HIKControlSetNode`, `CustomRigRetargeterNode` are held only. Slots 0 to 171 in a saved file's order, 172 to 211 from a published table's end.
- Commands: `hikCreateCharacter`, `newCharacterWithName`, `hikCreateDefinition`, `setCharacterObject`, `hikSetCharacterObject`, `GetHIKNodeName`, `hikGetNodeCount`, `hikGetSkNode`, `hikGetSkeletonNodes`, `hikGetCurrentCharacter`, `hikSetCurrentCharacter`, `hikGetCurrentSource`, `hikSetCurrentSource`, `hikSetCurrentSourceFromCharacter`, `mayaHIKgetInputCharacter`, `hikCharacterLock`, `hikToggleLockDefinition`, `hikIsDefinitionLocked`, `hikEnableCharacter`, `hikCreateControlRig`, `hikDeleteControlRig`, `hikBakeToSkeleton`, `hikBakeToControlRig`, the `hikUpdate*` refreshes as no-ops. Ours: `hikDefinitionStatus`, `hikNameMatch`, `hikMirrorMatching`, `hikSaveTemplate`, `hikLoadTemplate`, `hikStraightenArms`, `hikInfo`.
- The retarget is ours, as HumanIK's solver is closed, to what the Retarget Specific attributes describe. World turns from the T-poses, spine and neck by position along the chain so counts may differ, joints with no source follow their parent, hips and ankles moved by the action space scale (Auto: hip heights), a two bone solve at the ankle keeping the knee's side. Match Source, Mirror, Action Space, Hips Level, Feet Spacing and Ankle Height modes, hand reach.
- Human IK window (`HumanIK`, Skeleton > HumanIK): Character, Source, Lock, Create Control Rig, Stance, a drawn figure with slots red, yellow, green and the messages, Assign Selected Bone, Mirror Matching, Name Match, Controls figure selecting the control rig, Properties, bake options, templates.
- Tried on a mocap skeleton from a scene on this machine onto a rig whose limbs hang under groups: joints follow in proportion and the feet land. Its torso mesh stretches when the source lies down, as that rig skins to spine joints not in the definition.

Shallower, most noticed first:

- Custom Rig is not here: the retarget drives joints. A rig's controls don't follow; its constraints on mapped joints are set aside while driven, and rig-driven groups above them held still. So a rig with a spline spine doesn't deform as its own spine would.
- A retarget's stance is where the joints are at lock. A rig saved in a pose can't be stood up here but by posing it: the checks say so. `hikStraightenArms` covers arms-down binds and isn't the reference's.
- Pull, Stiffness, Mass Center and most Reach effectors aren't solved, nor pinning. No full body solve: Control Rig source is FK and IK with constraints, not `HIKFK2State` through the solver, so a real file's control rig reads as held nodes only.
- Finger and roll joints without source follow their parents; Roll attributes (`RollEx`, `ParamLeaf*`) are held, no roll extraction.
- `HIKState2SK`'s parent matrix inputs are written for the reference but not read: unmapped transforms above a joint must hold still, and connections set aside aren't remembered through a file.
- Property attribute names for Retarget Specific (`MatchSource`, `ScaleCompensation` and the like) couldn't be checked: no file on this machine has them changed. Template `.xml` layout is ours, reading any element with key and value. Name Matching knows common words, no the reference template set. No FBX import, so `TOPH_RIG_simple.fbx` wasn't tried; the frame range field stops at 100000.


### Symmetry on every action, soft select, more options (07/11/2026)

Not compared with a running copy of the reference. The reference's pages confirm the four falloff modes and that Multi-Cut and Insert Edge Loop take object, world and topological symmetry. I found no page listing which actions the reference does not mirror, so none are excluded here on that account.

Symmetry. Every history edit now takes the picked components and their mirror images, in `history::apply_edit`, so an action can't forget. `tests/symmetry_actions.rs` runs 34 actions by their commands from one side and checks both points and faces come out paired, and Insert Edge Loop, Multi-Cut's loop, Merge, Bridge, Quad Draw's face and soft select on top. Spin and Flip Triangle turn the mirror side the other way. Merge goes to the centre under symmetry, so the two sides meet alike. A pick in the view takes its mirror too, the mirror side drawn blue (the colour is chosen by eye); a `select` from a script does not. Handles sit on the picked side. Sculpt mirrors about the mesh's own plane. World symmetry goes by the world's axis wherever the object is: a turned object at an angle to its axes gets a plane at that angle, whose edits bake rather than record history, as radial's do. The status line lists Object, World and Topology and shows what is in force.

Soft select. B's tap stopped toggling after any B-drag, because the drag's mark was never cleared: reproduced with real keys and fixed. It now toggles in every tool, with focus in a number field too, and a held key lets go if the window loses the keyboard. Held, B draws the falloff circle on the surface under the mouse, and a drag sizes it with the colours following. Global reaches other meshes and colours them. Object takes other top-level objects along by a share of the move, rotate or scale, undoing as one step, for `move`, `rotate` and `scale` run from scripts as well. Surface goes by distance along the mesh.

Options that now act: Preserve UVs (a UV edit after the move, first order at each corner), Keep Spacing off with a grid snap (`moveComponents -snapEach`, not a reference flag), Pin Component Pivot, Snap Pivot Position, Use Manipulator Pivot, the edge and surface constraints (`xformConstraint -type`), Preserve Seam (`symmetricModelling -ps`). Shelf buttons and the Modeling Toolkit bring up handles for MEL edits; Python shelf buttons still don't.

Menus added: Joint Labelling, Copy and Paste Vertex Weights (`artAttrSkinWeightCopy`, `artAttrSkinWeightPaste`), Rivet (a locator with `pointOnPolyConstraint`; from memory the reference's uses a uvPin), Subdiv Proxy (a connected smoothed copy, with no proxy node or display toggles), Hold Current Keys, Display and Hide Intermediate Objects, UV Set Editor (New, Copy, Set Current, Delete), Select > Multi. Option boxes added for the NURBS primitives, Detach, Insert Knot, Attach Surface, Unfold, Layout, Delta Mush, Tension, Jiggle, Combine, Freeze Transformations, `copySkinWeights`.

Still not done:

- UV and Vertex Face component modes in the view: `ComponentMode` has neither and they touch everything that uses it. Multi switches to the kind under the mouse, so a selection doesn't mix kinds.
- Snap Pivot Orientation is held only. Gimbal rotate draws world axes.
- Global reaches meshes only, not NURBS. Object mode has no colours in the view.
- A World symmetry mesh moved after symmetry was set keeps its pairs from before. Set symmetry again after moving it.
- UV Pin, Proximity Pin, Interactive Bind Skin, Split UVs, 3D Cut and Sew, Unsmooth, Slice, Hole Punch, Cut Out, Deform > Edit other than Lattice, Prune Membership, Relationship Editors, IK/FK Keys, Audio, Cache, a top level Visualize.

## Harold renderer (07/11/2026)

One renderer, named Harold everywhere in the interface: a CPU path tracer in Rust (`crates/aztek_render`), translated
from the scene by `aztek_scene::harold`, with its commands, Render View, Render Settings, Attribute Editor frames,
Create > Harold, a Rendering shelf and batch rendering with no display. [docs/HAROLD.md](docs/HAROLD.md) has what it does,
the checks with their numbers, and timings. Nothing was compared with a running Maya or any other renderer: the furnace,
point and quad light, clamp and Cornell box checks compare against closed forms and against Harold's own high sample
renders.

Settings use the node and attribute names files hold (`defaultRenderGlobals`, `defaultResolution`,
`defaultArnoldRenderOptions` as `aiOptions`, `aiAOVFilter`, `aiAOVDriver`, `aiAOV`) and save with the scene; the shared
nodes are written with `createNode -s`. The shape attributes (`aiSubdivType`, `primaryVisibility`, ...) save on meshes
and NURBS surfaces. `render`, `hrender` with `arnoldRender` as a second name, `renderSettings`, `renderWindowEditor`.

From older renderers, not honoured (each named in a warning when a scene uses it):

- lambert family: translucence, glow, reflected colour (ambient colour is added as the viewport adds it); Maya's highlight shapes are approximated by a
  roughness (blinn's eccentricity, phong's cosine power, phongE's roughness, anisotropic's spreads).
- rampShader: its colour, specular and transparency ramps.
- layeredShader: textures as layers, glow.
- mia_material and its variants: the custom reflection curve (brdf_*), translucency.
- Lights: depth map shadows and their settings, `useRayTraceShadows` (shadows are always traced, `aiCastShadows` turns
  them off), light glow and optical effects, a volume light's colour and penumbra ramps.
- Settings of other renderers in `defaultRenderGlobals.currentRenderer`: read and ignored, with a warning.
- Render layers and render setup: not read; the master layer renders.

Shallower than the brief, ranked by what a user would notice: render layers; hair and curves not rendered;
subsurface noisier than it should be at default samples; USD stages not rendered; whole-image texture loading with a
memory limit rather than an evicting tile cache; no vector displacement or autobump; a simple non-local means
denoiser run on the interface thread; light group AOVs by the light that started the light, mesh lights' direct light
seen in reflections counted as indirect; rough metal at roughness 1 keeps 90% of the light (99.9% at 0.6).
## Unified dynamics (07/11/2026)

One solver (`aztek_dyn`, XPBD on particles in substeps) for cloth, particles and rigid bodies, which collide with and
push each other. Dynamics menu set (F5) with Cloth, Particles, Rigid Bodies, Fields, Constraints, Cache and Solver
menus; Dynamics shelf; cloth presets in the Attribute Editor; Interactive Playback. Details and numbers in
docs/DYNAMICS.md.

Done:
- Cloth as a deformer on any incoming mesh (skinned, animated), stretch, compression, dihedral bending, area, input
  attract with a per vertex map, pins, tearing, stretch limits and long range attachments; colliders with side
  tracking so fast ones do not tunnel; self collision; impossible states handled per vertex with a blend to the input or
  the neighbours and back, marked in the view and counted (`outOfSimulation`, `dynInfo`).
- Particles from omni, directional, surface, volume, curve and point emitters with lifespans; drawn as points, spheres
  or blobs; `particle`, `nParticle`, `getParticleAttr`, `setParticleAttr`, `connectDynamic`.
- Rigid bodies, active and passive, from their mesh, a box, sphere or capsule; pin, hinge, spring and nail.
- Fields: air, drag, gravity, newton, radial, turbulence, uniform, vortex, volume axis, with attenuation and maximum
  distance.
- Time as the reference: no solving backwards, the start frame starts again, a jump forward solves one frame, solved
  frames kept in memory; a disk cache in Aztek's own format with create, append, replace and delete.
- Files: nucleus, nCloth, nParticle, particle, nRigid, rigidBody, rigidSolver, the Bullet solver and rigid body,
  pointEmitter and the nine fields read into the one system with settings mapped by name and unused ones kept; hair,
  fluids, nComponent, caches and the like held and written back unchanged. Own nodes round trip exactly.
- Tests: analytic free fall and pendulum, drape, rigid stack, determinism across thread counts, squeeze, pushed
  through, 24 random fuzz cases with extreme inputs, scene and file tests.

Not honoured from the older systems (kept in the file, not used): nCloth lift, stickiness, pressure, rigidity, wind
shadow, self collision flags, per vertex maps but input attract and mass; nucleus wind noise, plane bounce, plane
normal, collision layers; particle ramps; per particle creation and runtime expressions (kept as text, not run);
field volume shapes and falloff curves; rigid solver settings; Bullet constraints and soft bodies; the components of
dynamic constraints from files; hair, fluid and molecular matter simulation.

Shallower than the reference, most important first:
1. Speed: a 3,000 vertex shirt on a 15,000 vertex skinned body solves in 130 to 190 ms a frame, 12,000 vertices in 240
   to 320 ms; a free 20,000 vertex drape landing on a sphere with self collision takes 140 to 270 ms and more as it
   folds, as contact search grows with relative motion.
2. Fine cloth stretches under its own weight where nothing pins it: at 20,000 vertices the mean edge ends about 1.1
   times rest and single edges near the contact reach several times, where the impossible state handling then takes
   them out. The stretch limits measure across a curve rather than along it.
3. Rigid bodies on cloth are springy: boxes dropped on a hammock rebound and slide apart.
4. Self collision is vertex against triangle only, no edge against edge.
5. Torn edges stop pulling but the mesh is not split.
6. Rigid bodies find their object by its path, so renaming it breaks the link.
7. Per particle expressions, ramps and the instancer are not there; particles keep colour and opacity per particle but
   nothing sets them but `setParticleAttr`.
8. The disk cache is Aztek's own format; `.mcx` caches are not read.
9. Nothing has been compared against a running copy of the reference.


### One light model (08/11/2026)

The reference's six lights now light a render as they light the viewport: decay rates, colour times intensity, spot
edge and dropoff, ambient shade, area and volume lights, emit flags and light linking, from one formula
(`aztek_scene::lights::viewport_diffuse`; the viewport's GLSL is its copy, pinned by a host test). `tests/light_parity.rs`
compares the renderer's direct light on a white matte surface with it at 25 points and normals for every type, all four
decay rates, both penumbra signs, normalised and not, and every volume shape and direction: exact for point, spot,
directional and volume lights, within 0.004% for the sampled ambient parts, within 0.8% for area lights 200 units off
(the viewport lights from the rectangle's nearest point, the renderer from all of it; closer, that is the soft light
only a renderer has). A scene with no lights renders with the viewport's default lighting. `physicalLights` on the
options node gives physical units instead. Specular highlights stay the standard surface's, not blinn's and phong's
shapes, and the legacy materials' specular layer takes a few percent from the diffuse beneath it where the viewport adds
the two.

Measured beside the viewport (Use All Lights) from the same session at 955 x 720, render over viewport in linear light
on lit pixels: `examples/lights.ma` median 1.02 (quartiles 1.01 and 1.03); a scene with no lights under default
lighting 1.03; the Toph rig with a key, a rim and a fill added 0.98, the shadowed quarter lower. Found on the way: the
spot light was missing from renders before this (no decay in the file, intensity 2, but rendered with quadratic falloff
about 13 units from the floor: a hundredth of the light), ambient
colour wasn't honoured, and an ambient light's all-round half was lost to a multiple importance weight no ray could
balance; each has a test.

### Binary scene files (09/11/2026)

`.mb` files open, import and load as references (File > Open, Import, Create Reference, `file -o/-i/-r`, the command
line). `mb_file.rs` reads the IFF tree and writes it out as `.ma` text for the `.ma` reader, so both formats go through
the same code: nodes with their ids, values of every kind seen so far (numbers, compounds, strings and string arrays,
Int32/double/point arrays, matrices and transformation matrices, component lists, attribute aliases, key times),
meshes (points, edges, normals, faces with holes, uv and colour sets), added attributes, `select -ne` values on default
nodes, connections, relationships, plug-in requires, references with their load state and reference edits. Values are
kept in internal units with the file's own units given to the scene afterwards. Checked by opening twelve scenes the
reference saved both ways and writing out what Aztek holds: eleven match line for line, and the metre scene differs only
by the camera distances below. Node types come from a table of
four-character ids plus the ids a file's plug-ins declare; a node of a type not in it is skipped with a warning (a
transform stands in for an unknown transform kind). Not yet: writing `.mb`; added attributes beyond bool, double and the
data types; NURBS geometry data. Found on the way and fixed for `.ma`: mesh points (`.vt`, `.pt`) were read and written
without the file's linear unit, so a mesh in a metre scene came out a hundredth of its size; a display layer's colour
set a channel at a time wasn't read. Still to do in `.ma`: camera distances (`.coi`, `.ow`, clip planes) don't take
the file's linear unit.

### Rigs from later releases, outliner keys, viewport (09/11/2026)

Three production rigs saved by a 2027 release open at rest and pose. What they needed:
- unitConversion carries a rotation's three angles as well as one value (its input and output take either), as the
  reference's does; 146 connections per rig went through one.
- addDL, multDL, distanceBetweenDL and angleBetweenDL, the later releases' twins of the double-linear nodes.
- A nurbsCurve's control points by their parts, `.cp[3].xv`, which eyelid and lip rigs drive.
- Constraints count the driven object's offsetParentMatrix, as the reference's do: a buffer group placed by its
  offsetParentMatrix and point-constrained sat that far off twice.
- The transform's own space-switch attribute was called `space`, the name rigs give their own enum: it is now
  `aztekSpace`.
- A plug-in named in `requires` with its node types warns only when one of those types is used and unknown.
The worst body went from 721 units off its rest shape to under 2. Not yet: colorCorrect (shading only), ngSkinTools'
layer data (the skinCluster's own weights are used).

Outliner: F frames its selection (opens what is above and scrolls to it), the arrow keys walk the list, Shift-click on
an arrow opens or closes the whole hierarchy under it, as do Shift+Right and Shift+Left. A change of selection or a
value no longer rebuilds the tree, and nothing is rebuilt while a button is held over a view.

Viewport: 4x multisampling (AZTEK_MSAA=1, 2, 4 or 8 for another count; fewer where the device allows fewer).
Evaluation keeps the orders of the last 32 passes instead of one, so a drag's repeated passes (a wire sampling its
drivers before the scene evaluates) stop working the order out again; an aim constraint reads its object's translate
from whatever drives it in the same pass instead of a second evaluation after each frame. Dragging a rig's body control
went from 114 to 75 ms a step on two cores, an arm from 34 to 17.

The Script Editor's history is kept from start-up: opening it shows everything since, as the reference's does.

### Object and component mode after modelling (10/11/2026)

Checked against the reference's behaviour: Bevel given a whole object bevels every edge and leaves the object selected
in object mode (it used to switch to edge mode with every edge picked, so the next click or Ctrl+D worked on edges).
Duplicate copies a mesh with construction history as it looks now, with no history, as the reference's Duplicate does
without -upstreamNodes; the copy used to take a broken chain of the history nodes and came out empty. A stack of
Aztek's own modifiers is still copied live. `ls -hl` is empty in object mode. `polyBevel -sg` sets the segments.

### Plug-in Manager and the user folder (11/11/2026)

Windows > Settings/Preferences > Plug-in Manager: every plug-in file on the plug-in path grouped by folder, Loaded and Auto load
ticks, Browse, Refresh, a filter and an information box, as the reference's. Auto load is kept by file in `pluginPrefs.json`.
`pluginInfo -autoload` and `-listPluginsPath`, `internalVar` for the user folder's places, `moduleInfo`.

The user folder (Preferences > User Folder, `aztekUserFolder`) is Aztek's own (`~/aztek`), an existing folder of the reference's
used in place, or a custom one; an existing folder can be copied into Aztek's own. From it: `scripts` (MEL and Python paths),
`plug-ins`, `modules` with `.mod` files (`MAYAVERSION`, `PLATFORM` and `LOCALE` conditions, `=`, `:=`, `+=`, `+:=`, `scripts:` and
`plug-ins:` lines), `Maya.env` or `Aztek.env`, the optionVars in `prefs/userPrefs.mel` as defaults, `userSetup.mel` (the first on
the path) and every `userSetup.py`, and its shelves on request.

Differences from the reference:

- A compiled plug-in built for the reference is listed as "Compiled for Maya" and never loaded; it is told by reading the file for
  what it exports, not by loading it.
- The reference writes its preferences into its user folder. Aztek keeps its own in `~/.aztek` and never writes to a linked folder.
- Of the saved preferences only optionVars are read. Hotkeys, marking menus, colours, window layouts and `pluginPrefs.mel` are not.
- `.mod` conditions other than the three above stop the line being read as conditions; `[r]` recursion on `scripts:` is taken as
  the folder alone. Module blocks for several versions: the block for the version in use, else the first.
- Plug-ins are listed one folder deep, as the reference lists them.
- `internalVar -mayaInstallDir` is Aztek's folder.
- Not checked against the reference: the order of `userSetup` against auto loaded plug-ins (plug-ins first here).

### Script functions for files, the shell and dates (11/11/2026)

`fopen`, `fclose`, `fprint`, `fwrite`, `fread`, `fgetline`, `fgetword`, `feof`, `fflush`, `frewind`, `popen`, `pclose`, `system`,
`date` (`-format`, `-date`, `-time`, `-shortDate`, `-shortTime`), `timerX`, `filetest`, `fileExtension`, `basenameEx`, `trace`,
`format -stringArg`, `rehash`, `gauss`, `sphrand`, `noise`, `hermite`, `rot`, `stringArrayRemoveAtIndex`, `stringAddPrefix`,
`stringRemovePrefix`, `attributeExists`. Found missing by running a list of the reference's common commands against the build.

Differences from the reference, none checked against it:

- `noise` is gradient noise in the same range, zero at whole numbers; its values are not the reference's. `gauss` and `sphrand`
  draw from Aztek's own generator, so a seeded script gives other numbers.
- `fwrite` and `fread` take strings (ended by a zero byte), ints and floats in four bytes each; the reference's byte layout for
  numbers was not checked.
- `filetest` has `-e -f -d -r -w -x -s -z -l`; the others are an error.
- `date` is local time on Linux and macOS, UTC on Windows.
- `trace` writes to the terminal. `rehash` does nothing: scripts are looked for on the path each time.
- A file opened for reading is read whole at `fopen`.

Still missing from that list, as commands: `commandPort`, `fileInfo`, `lsUI`, `listAnimatable`, `scriptEditorInfo`, `hotkeySet`,
`dimWhen`, `memory`, `exec`, `treeView`, `iconTextScrollList`, `channelBox`, `cmdScrollFieldExecuter`, `cmdScrollFieldReporter`,
`toolBar`, `draggerContext`, `repeatLast`, `quit`, `createRenderLayer` and the render layer commands, `cacheFile`, `geometryCache`,
`timeEditor`, `AbcExport`, `AbcImport`, `FBXExport`, `FBXImport`, `gpuCache`, the fluid commands, `polyToSubdiv`, `subdiv`.

### Audit and timing (11/11/2026)

- Compiler warnings: none. Clippy stopped at its first errors in `aztek_dyn`, so the crates after it were never linted; those
  errors are fixed (ordering types whose `PartialOrd` was derived beside a hand-written `Ord`, two literal values of pi, an empty
  range on purpose, the C interface's pointer arguments) and it now runs through the workspace with style warnings only.
- `-staticSingleSample` on USD export changes nothing: samples all alike are written as a static value either way.
- Creating meshes slowed with every mesh in the scene: each new one re-read and re-wired the whole default shading group, and the
  search for a free default name counted from 1. 4000 `polyCube` calls took 40 s and take 1.9 s; 4000 `spaceLocator` 0.84 s to 0.08 s
  (release build, batch).
- Not fixed: with something dirty in the scene, each `getAttr` walks every node (the dirty scan in `Graph::evaluate_dirty` and the
  rig hook `rig::sync`), about 1 ms a call at 4000 cubes. Deleting 4000 cubes takes 4.4 s. Opening a 73 MB `.ma` of one dense
  mesh takes 1.0 s and saving it 0.8 s.

### Edit Pivot, Preserve UVs, UV sets, glTF export (11/11/2026)

Edit Pivot (issue 23):

- A drag in Edit Pivot mode did nothing: the command it sends, `movePivot` with both pivots named, asked `xform` a
  question in a form `xform` refuses. It moves the pivot now.
- D and Insert: a tap turns Edit Pivot on or off, as the reference's do; D held through a drag edits the pivot only
  while held. A tap is under 300 ms. The Tool Settings button shows the mode and sets the same one. A ring round
  the manipulator's centre shows the mode is on; the reference draws its own pivot manipulator.
- Bake Pivot baked only a pivot orientation, so with the pivot only moved it did nothing. It bakes the position too:
  the translate goes to the pivot, the four pivot offsets go to zero, the points are countered and children stay
  where they are. `bakePivot -position` and `-orientation` choose; both are on by default.
- Choosing a tool (W, E, R, Q, a tool button) leaves Edit Pivot, as the reference does.
- The pivot turns in the view: in Edit Pivot on an object the manipulator has three rings outside its arrows, and a
  drag on one turns the pivot's axes about that axis. The object stays. Step Snap and J apply. The tools' Axis
  Orientation then goes to Object, which shows the pivot's axes (`pivotOrient`); the reference goes to its Custom
  orientation. Edit Pivot shows the pivot's axes whatever the orientation chosen. `setPivot -turnAxis -turnAngle`
  is the command; Bake Pivot turns the object onto the axes. Not on joints.
- Not done: in component mode Edit Pivot still moves the components rather than the manipulator.

Preserve UVs (issue 24): each face answered for its own corner of a moved point, and where the faces disagreed (a
move out of their plane, or faces mapped at different scales) the UV point came apart. Corners that shared a UV take
the mean of their faces' answers and stay one UV point.

UV sets (issue 25):

- A mesh's sets keep their names and order (`uv_sets.rs`). The current one is what the UV Editor, the UV tools and
  the views use. Each set has its own seams: before, making a set current kept the other set's seams, so its shells
  were cut wrongly. A new set is empty; `polyUVSet -create` and `-copy` return the name, made unique.
- `.ma` files keep every set, its name, the order and the current one. Before, only numbered layers were written, as
  map1, map2, and sets made with `polyUVSet` were lost on save.
- The UV Set Editor (Windows > Modeling Editors, `UVSetEditor`) lists the selected mesh's sets and follows the
  selection: picking one makes it current, New, Copy, Rename and Delete act on the one picked. The UV Editor has a
  UV Set list.
- Differences from the reference: textures in the views and renders go by the current set, where the reference uses
  the first set unless `uvLink` says otherwise (`uvLink` is still stored only). No per-instance sets, no Propagate, no
  UV Linking editors. `mesh.uvSet[i].uvSetName` and `mesh.currentUVSet` are not attributes: ask `polyUVSet -q`.
  `polyEvaluate -uvs` counts the current set whichever is named. The editor's buttons are from memory of the
  reference's, unchecked.

Undo after reading a file: a mesh operation, then anything that evaluated the mesh, then undo left the edited mesh
on the shape, because undoing a connection did not put back a mesh plug's own value. It does now.

glTF 2.0 (issue 26): `exportGltf path [objects]`, `-all`, and File > Export All and Export Selection with a `.glb` or
`.gltf` name.

- Written: the transform hierarchy leading to meshes; meshes as they stand (deformed, triangulated, split along hard
  edges and seams); normals; every UV set in set order (`TEXCOORD_0` is the first); vertex colours; one primitive per
  shading group; materials as metallic-roughness with base colour, opacity, emission, metalness and roughness from
  standardSurface (a lambert is fully rough, the other older types half), `KHR_materials_unlit` for a surfaceShader;
  the colour texture, inside a `.glb` and by relative path from a `.gltf`, with `KHR_texture_transform` for repeat and
  offset. Centimetres become metres. A Z-up scene gets a node over it that turns it Y up.
- Not written: skins, animation, blend shapes, cameras, lights, textures on anything but colour, and images that are
  not PNG or JPEG (the export says which).
- Checked against the format's rules by a script here and against Aztek's own world bounds. Not opened in another
  application or run through the Khronos validator: neither was available.
- FBX is not written. The format can be written without its owner's SDK, but nothing here can read one back to check it.

### Issues 26 to 33 (12/11/2026)

What the reference does was taken from its documentation where a page says it, and from the screenshots in the issues.
Where neither says, the line says so.

View switching (issue 33): a Space tap swaps one view for four and back. After the first tap the keyboard went to the
menu set box, which Space then opened. The view that had the keyboard was hidden while the panes were laid out again,
and the window gave the keyboard to the next widget along. It stays with the views. Choosing a menu set hands it
back as well. The view under the pointer is the one that fills the window, as in the reference.

Attribute Editor sliders (issue 27): the editor rebuilt itself at each change of the value it was showing, which took
the slider from under the mouse a step or two into a drag. It waits for the release.

- A press on a slider's track puts the handle there and the drag goes on, as the reference's sliders do.
- A slider runs over the attribute's soft range. A value typed past it, inside the attribute's limits, widens the
  slider so the value sits half way (0 to 1, type 5: 0 to 10). Unverified against the reference: the factor of two.
- Rows fit the panel's width (the sliders give), names sit in a column of one width down the tab, and a name too long
  for it loses its start.

Map buttons (issue 29): the reference puts one on every attribute a texture can drive. Only colours had one.

- Every input colour and number of a material, texture, light or shading utility has the checker button at the end
  of its row. It opens Create Render Node; the node chosen is made (a file texture with its placement) and wired in:
  its colour into a colour, its alpha into a number. The new node is then selected, so the editor shows it.
- Connected, the button is an arrow into a box and goes to the input; the field is yellow and read only. Right click on
  the button or the name: Break Connection.
- A row's name has the reference's menu for what the attribute can take: the input node, Set Key, Break Connection,
  Create New Texture, Color Chooser, Lock or Unlock Attribute. Not there: Create New Expression, Set Driven Key,
  Ignore when Rendering.
- A colour row has a slider for its value, as the reference's has.
- The standard surface's rows are named as the reference's: Weight, Color, Roughness, IOR inside each frame. Base and
  Specular are from the issue's screenshot; the other frames follow the same rule and are unverified, as is Dispersion
  Abbe. IOR has a slider to 3 (the screenshot's 1.5 sits half way).
- The map button no longer opens a file dialog: the reference makes the file node and shows it, and so does this.

Hypershade (issue 28): it was three lists. It is laid out as the reference's.

- Browser: Materials, Textures, Utilities, Rendering, Lights, Cameras, Shading Groups, Bake Sets, Projects, Asset
  Nodes, with a search field. Materials are drawn on a lit ball from their own values. A swatch picked is the node
  selected. Right click: Graph Network, Assign Material To Selection, Select Objects With Material, Rename, Delete.
  Projects shows the project's folders. Bake Sets and Asset Nodes are empty: Aztek has neither kind of node.
- Create: the reference's groups (Surface, Volumetric, Displacement, 2D, 3D, Env and Other Textures, Lights,
  Utilities) and Harold's (Texture, Light, Shader, Utility), filled from the node types by their classification,
  with a search field. A surface material is made with its shading group.
- Work area: a node graph a tab (Tab creates a node), with Input Connections, Input and Output Connections, Add
  Selected, Clear Graph and Frame All.
- Material Viewer: the selected material on a ball. It is drawn by the interface from the material's main values
  (colour, specular, roughness, metalness, emission), not by Harold, and has no choice of renderer, shape or light.
- Property Editor: the Attribute Editor on the selection.
- Menus: Edit, View, Create, Tabs, Graph, Window. Not there: File, Options, Help, bins, the toolbar's icons, drag and
  drop from the Browser to the graph, and Delete Unused Nodes.

Outliner (issues 30 and 32):

- An icon is drawn the same on a selected row, with a dark edge round it: the style's tint washed blue icons into the
  blue of the selection.
- Pictures for what had letters in a box: curves, surfaces, annotations and measures. Joints are a filled bone, an
  empty transform three heavy axes.
- Shapes on lists the original shapes deformers read (`pSphereShape1Orig`) under the transform, as the reference's
  Outliner does. It took the Intermediate Objects switch before.

Harold's lights (issues 30 and 31):

- New ones are named `haroldSkyDomeLight1`, `haroldAreaLight1`, `haroldMeshLight1`, `haroldPhotometricLight1`. The
  names are Aztek's own choice. The node types (`aiSkyDomeLight` and the rest) and attribute names stay as scene
  files write them; the Attribute Editor's heading says `haroldSkyDomeLight`.
- They draw in the view and can be picked there: the area light's quad, disc or cylinder at its real size with the
  way it faces; the dome as a sphere of its Sky Radius (a new attribute, 1000 by default, drawing only); the
  photometric light as a point and its aim; a marker for a mesh light, whose surface is the mesh.

Bind Skin (issue 32): the bound object's translate, rotate and scale are locked, as the reference locks them, and
freed by Unbind Skin. Undo takes the locks off with the bind. Not done: the reference's `tweak` node, and a warning
when the bound object is under a moving parent.

glTF (issue 26):

- A Z-up scene went out with a node over everything carrying a quarter turn. Everything in it changes basis instead
  (each node's matrix, points, normals, bind matrices), so a point at Z 2 is at Y 2 and no node is added.
- `exportGltf -upAxis z` writes Z up: for a Z-up scene, the data as it is. File > Export asks which when the scene
  is Z up. A Y-up scene can be written Z up the same way. The format is Y up, and the result says so when Z is asked for.
- Skins: the joints go out as nodes in their hierarchy (also when only the mesh was asked for), a bound mesh in its
  bind pose with `JOINTS_0` and `WEIGHTS_0` (the four heaviest a point, summing to one), and a skin with the joints
  and their inverse bind matrices. A mesh whose points change after its skin (a smooth on top) goes out as it
  stands, with a warning.
- Not written: animation, blend shapes, cameras, lights. Not checked: the files have not been opened in another
  application from here.

### Viewport: anti-aliasing shown, ambient occlusion (12/11/2026)

- Anti-aliasing was on (4 samples a pixel, since 09/11/2026) with nothing in the interface saying so. A view's
  Lighting > Anti-aliasing lists Off, 2, 4 and 8 samples, ticks the one in use, and kept the choice for the next
  start only (changed 12/11/2026, see "Anti-aliasing changes at once" below). `AZTEK_MSAA` overrides the saved choice
  at start.
- Ambient occlusion: Lighting > Ambient Occlusion, with Radius (scene units) and Intensity under Ambient Occlusion
  Settings. Surfaces darken in creases and where they meet. Off by default; the choice is kept for new views. X-Ray
  turns it off.
- How it is made: the scene is drawn into an image of its own, with its depth kept. A second pass reads the depth and
  works out, for each pixel, how much of what is within the radius stands in front of the surface (16 samples on a
  spiral, the surface's normal from the depth). The last pass smooths that over the pixels of the same surface and
  multiplies the picture by it, onto the window. Something farther in front than the radius (a manipulator, another
  object) casts nothing.
- Limits: it is worked out from one depth sample a pixel, so its edges are not anti-aliased; it darkens everything a
  pixel shows, lights' direct light included, where a renderer would darken only the light from around; lines and
  the grid sit in the same picture and are darkened with it; on screen the radius is held to 64 pixels, so close up
  it reaches less far than asked.
- Not done, of what a real-time view of a physically based scene wants: image-based light from the sky dome,
  specular reflections of it, soft shadows from area lights, the standard surface's full model (the view draws its
  base colour and one highlight), tone mapping, depth of field, motion blur, fog, sorted transparency.

### FBX export, and features as plug-ins (12/11/2026)

FBX export: File > Export All or Export Selection with a .fbx name, `exportFbx path [objects] [-all]`, and `FBXExport
-f path [-s]` for scripts written for the reference.

- The file is binary FBX 7.4, written from the format's public descriptions. No library of its vendor's is used.
- Written: the hierarchy; meshes as polygons (quads stay quads) with a normal a corner, every UV set and a material
  a face; materials with colour, ambient, glow and transparency, and the colour texture by path; skins as limb nodes,
  a cluster a joint with its points and weights, and the bind pose, the mesh in its bind pose.
- Centimetres, and the scene's up axis in the file's settings: nothing is turned or scaled.
- A node's pivots and a joint's orientation are folded into its translation, rotation and scale. Shear is dropped,
  with a warning.
- Not written: animation, blend shapes, cameras, lights, vertex colours, smoothing groups, FBX import.
- Not checked: the files have been read back by a reader written for the test, which shows they are well formed and
  hold what they should. They have not been opened in another application. The footer and file id are the fixed
  bytes other writers of the format use, taken from memory of them.

Plug-ins that come with Aztek: `fbxExport` and `gltfExport` are the first two. Each registers its commands when it
loads and takes them away when it unloads; they load at start unless unticked in the Plug-in Manager, where they are
listed under "Aztek (built in)", and answer to `loadPlugin`, `unloadPlugin` and `pluginInfo` by name.

- They are compiled into the program, not files on the plug-in path, and are not written against the plug-in kit:
  the kit has commands, nodes and deformers (MPxCommand, MPxNode, MPxDeformerNode, MPxGeometryFilter) and no file
  translator class yet.
- Harold is not one yet: its node types, commands, render settings and Render View are still registered by the core.

### The user folder is asked about at the first start (12/11/2026)

With the reference's user folder found and no choice saved, the first start asks how to use it, before the plug-ins
marked to load and the userSetup scripts run: copy it into Aztek's own folder, use it where it is (read only), or
take nothing. The version and whether its userSetup scripts run are chosen there too. The copy is offered first
when Aztek's folder does not exist, leaving Aztek's folder as it is when it does. Putting it off saves nothing, so
it is asked again. Not asked with `--script`, `--screenshot` or `--batch`, or when no such folder is there.
The optionVars in the folder's `userPrefs.mel` are read before the question, so on that first start they come in
at the next one.

### FBX and glTF import; the two plug-ins read and write (12/11/2026)

File > Import lists FBX and glTF, and File > Export lists them, while their plug-ins are loaded, as the reference
lists a translator's formats. The plug-ins are now `fbx` and `gltf` (they were `fbxExport` and `gltfExport`, names
that still find them): each brings its import and its export commands.

FBX import (`importFbx path`, `FBXImport -f path`):

- Binary FBX, versions with 32-bit and 64-bit offsets, arrays plain or deflated.
- Read: the model hierarchy; each node's place from its translation, rotation in its order, scale, pre and post
  rotation, pivots and offsets, folded into one translation, rotation and scale; meshes as polygons with their UV
  sets (by corner or by point, direct or indexed) and geometric offset; limb nodes as joints; materials' colour and
  opacity (phong as a blinn, others as a lambert) on the faces that wear them.
- The file's unit comes in as centimetres. A file whose up axis is not the scene's changes basis: no node is added.
- Not read: ASCII FBX (it says so), skin weights, animation, blend shapes, textures, normals (the mesh's own are
  used), cameras and lights (their nodes come in as transforms).

glTF import (`importGltf path`): `.gltf` with its buffers inside or beside it, and `.glb`. The node hierarchy,
triangle meshes with their first UV set, materials as standard surfaces (base colour, metalness, roughness), nodes a
skin names as joints. Metres come in as centimetres and Y up becomes the scene's up. Not read: skin weights,
animation, textures, cameras, lights, morph targets, sparse accessors, lines and points.

One undo takes a whole import back. What was made at the top is left selected.

Checked by writing a scene out and reading it back in each format: hierarchy, places, every corner's position and
UV, materials by face, joints and their pose, and the change of basis into a Z-up scene. FBX files from other
writers have not been read here: the reader has only seen files this exporter wrote, and packed arrays and 64-bit
offsets built by a test.

FBX export: a joint chain asked for goes out whole (its child joints were left out unless a skin used them).


### Anti-aliasing changes at once (12/11/2026)

- Lighting > Anti-aliasing changes the samples a pixel in every view of the layout when chosen, and keeps the choice
  for the next start. Before, the choice was only saved: picking Off did nothing until a restart, then left
  anti-aliasing off.
- A device that cannot give the count asked for gives the nearest lower one it has; the tick shows the count in use
  and the Script Editor says so.
- Checked on a software Vulkan device that has 1 and 4 samples only: Off and 4 Samples compared pixel for pixel
  against a start with `AZTEK_MSAA=1` and `AZTEK_MSAA=4`, in one view and four, with ambient occlusion on, and after a
  window resize. 2 and 8 samples have not been drawn here.
- Tests: the unit-setting tests share one frame rate for the whole process and ran side by side; one could read the
  rate another had just set (seen once in 60 runs). They now run one at a time.

### Environment variables (12/11/2026)

- The reference's environment pages for 2026 and its start-up give 122 variables. Each is in `docs/ENVIRONMENT.md`
  with whether Aztek reads it; the file is written from `crates/aztek_core/src/env.rs` and a test fails when the two differ.
- Names: a variable spelled `MAYA_X` is read as `AZTEK_X` first, then `MAYA_X`; lists of folders use both.
- Newly read: `MAYA_ENV_DIR`, `MAYA_CMD_FILE_OUTPUT` (with the `cmdFileOutput` command), `MAYA_PROJECT`,
  `MAYA_PROJECTS_DIR`, `MAYA_SHELF_PATH`, `MAYA_PRESET_PATH`, `XBMLANGPATH` (shelf buttons and the image flags of
  interface commands, `xbmLangPathList`), `MAYA_SCRIPT_EDITOR_STDOUT`, `MAYA_DISABLE_BACKSPACE_DELETE`,
  `MAYA_SKIP_USERSETUP_PY`, `AW_JPEG_Q_FACTOR`. `MAYA_LOCATION` and `MAYA_APP_DIR` are set inside a session.
- The environment file: one file is read where two to four were; the user folder, the home folder and the
  application's place cannot be set from it.
- Differences: the file's folders for the path lists are added after the system's, where the reference's page says
  the file replaces some of them (not checked against the application); `MAYA_PRESET_PATH` finds Aztek's `.json`
  presets only; shelves from `MAYA_SHELF_PATH` are not written back; without `MAYA_PROJECT` the project is still the
  folder Aztek was started in, not `projects/default` in the user folder.
- `nodePreset -save`, `-load`, `-exists` and `-delete` take their node and preset in MEL; they took one word.
- Saved Move, Rotate and Scale options with a switch or a number were refused at the next start on Linux, where the
  settings file keeps them as text.


### Hypershade: ports, placement, rendered swatches; anti-aliased lines (12/11/2026)

- A node made in the Hypershade shows its attributes, outputs first, so it can be wired at once. Every node has a
  port each side of its title standing for all its inputs and all its outputs: a wire dragged from one asks which
  attribute, and dropped on a node asks which of that node's. Before, a node with no connections had no ports.
- Making a 2D texture makes its `place2dTexture` and connects `outUV` to `uvCoord` (and the filter size where the
  texture has one); a `file` keeps its full set of connections. Making a 3D texture makes a `place3dTexture` and
  connects `worldInverseMatrix[0]` to `placementMatrix`. The same from the Attribute Editor's map button.
- New node types, with the attribute names, short names and defaults of the reference's node pages (2025, read
  12/11/2026): `bulge`, `fractal`, `grid`, `mountain`, `water`; `brownian`, `cloud`, `crater`, `granite`, `leather`,
  `marble`, `rock`, `snow`, `solidFractal`, `stucco`, `volumeNoise`, `wood`; `place3dTexture`, `samplerInfo`,
  `gammaCorrect`, `contrast`, `bump3d`. `ramp` is classed as a 2D texture, as the reference classes it. Harold
  shades all of them.
- Differences: the reference's noise is not published, so each pattern has the same character and answers the same
  attributes without matching point for point. Their colour attributes are read as plain values, not as textures.
  `crater` and `stucco` do not give their `outNormal`. `water`'s reflection box and `volumeNoise`'s implode and
  falloff are not done. `samplerInfo` gives its facing ratio whichever output is read. `place3dTexture` is not drawn
  in the views.
- Not registered: the reference's `cloth` texture, because Aztek's cloth simulation node already has that name. Not
  made yet: see the node list in PLAN.md.
- The Material Viewer and the Browser's swatches are rendered by Harold (`haroldSwatch`): a material on a ball,
  a texture flat. They were drawn from the attribute's single value, so a mapped colour showed black.
- The views draw a material whose colour comes from a network that is not one image file with that network worked
  out into a 256 pixel picture over the UV square, and with the picture's mean colour where textures are off. Both
  were black. A 3D texture's picture is a slice through it, not what a render shows on the object.
- Anti-aliasing covers lines: with it on, wireframes, curves, the grid and manipulators are drawn with smoothed
  lines where the graphics device has them (rectangular lines otherwise). A device was free to step its lines under
  multisampling. The view's toolbar has a button for anti-aliasing beside the one for shadows.
- Checked on a software Vulkan device only.
