# USD in Aztek

USD is read and written by the native Rust crate `openusd`, with no C++ library to install. Commands take the names and
flags of the reference's USD plug-in and of the Animal Logic USD plug-in, so scripts written for those run where the commands exist. `docs/COMMANDS.md`
lists every command with its flags.

## Files as nodes

File > Import and File > Export All or Export Selection take `.usd`, `.usda`, `.usdc` and `.usdz`. The option box next to each
sets what they do. The same from a script:

    mayaUSDImport -file "shot.usd" -readAnimData 1 -primPath "/World";
    mayaUSDExport -file "out.usdc" -frameRange 1 120 -exportSkels auto -exportSkin auto -exportBlendShapes 1;
    file -i -type "USD Import" -options "readAnimData=1" "shot.usd";
    file -es -type "USD Export" -options "animation=1;startTime=1;endTime=120" "out.usda";

With no `-primPath`, import reads the default prim, or everything if the file has none. Export writes a default prim when the
export has one root, because of that rule: several roots with a default prim would read back as one.

What crosses:

| In the scene | In USD |
| --- | --- |
| Transforms and their pivots | xformOps in the reference's stack; any other stack reads through its matrix |
| Meshes | points, faces, invisible faces, creases, normals, UV sets as `st`, `st1`, colour sets, subdivision scheme |
| NURBS curves and surfaces | `NurbsCurves`, `NurbsPatch`; `BasisCurves` read as curves |
| `Cube`, `Sphere`, `Cylinder`, `Cone`, `Capsule`, `Plane` | read as polygon meshes |
| Cameras | `Camera`, lens in millimetres |
| Joints and skinClusters | `Skeleton`, `SkelAnimation`, skel bindings under a `SkelRoot` |
| blendShape deformers | `BlendShape` prims with inbetweens |
| Keys | time samples, a sample per frame; reading makes linear keys |
| lambert, blinn, phong, standardSurface, file, place2dTexture | `UsdPreviewSurface`, `UsdUVTexture`, `UsdPrimvarReader_float2`, `UsdTransform2d` |
| Arnold shaders (`aiFlat`, `aiStandardSurface` ...), read only | `arnold:` shaders on `outputs:arnold:surface`; the RenderSettings' `arnold:global:background` to `defaultArnoldRenderOptions.background` |
| Face assignments | `GeomSubset`s in the `materialBind` family |
| Instanced shapes | instanceable prims on a hidden prototype; `PointInstancer` reads as instances |
| Attributes tagged `USD_ATTR_x`, or listed in `USD_UserExportedAttributesJson` | custom attributes, `userProperties:` |

Files in another up axis or unit come in under a group that turns and scales them. `-upAxis 0 -unit 0` turns that off.

A USD proxy shape exports as a reference to its stage, as the reference's `exportStagesAsRefs`. Two flags are Aztek's own:
`-exportStagesAsPayloads` writes payloads instead, and `-flatten` writes one layer with the composition baked in.

Textures in a `.usdz` are copied out to a temporary directory to be read.

## A stage in the scene

Create > USD > Stage from File makes a proxy shape: a transform over a `mayaUsdProxyShape`, drawn in the viewport with no node per
prim. Dropping a USD file on the window does the same.

    mayaUsd_createStageFromFile "shot.usd";
    mayaUsd_createStageWithNewLayer;
    AL_usdmaya_ProxyShapeImport -file "shot.usd" -name "shot";

The shape's attributes: `filePath`, `primPath`, `excludePrimPaths`, `time`, `loadPayloads`, `shareStage`, `stageCacheId`,
`drawRenderPurpose`, `drawProxyPurpose`, `drawGuidePurpose`. `AL_usdmaya_ProxyShape` is the same thing under AL's name.

Drawing shares geometry between prims that hold the same data and instances it on the GPU. It reads the stage in slices, so a big
stage fills in rather than stalling, and after an edit only reads the prims the edit touched. Meshes pose by their skeleton and
blend shapes. `UsdPreviewSurface` colours and diffuse textures draw; a MaterialX-only look draws grey.

## Prims

A prim is named `|stage1|stageShape1,/World/thing`, in strings. `select`, `ls`, `move`, `rotate`, `scale`, `delete`, `rename`,
`parent`, `getAttr`, `setAttr` and `setKeyframe` take such names. Clicking selects a prim, the Outliner lists them under the stage,
and the Channel Box and Attribute Editor show the selected prim. The manipulators author xform ops on the edit target. An attribute
that already has samples takes a sample at the current time. All of this is on the undo stack.

    select -r "|stage1|stageShape1,/World/thing";
    move -r 1 0 0;
    setAttr "|stage1|stageShape1,/World/thing.radius" 2;
    getAttr "|stage1|stageShape1,/World/thing.translateX";
    usdPrim -define "Cube" "|stage1|stageShape1,/World/new";
    usdPrim -setVariant "shape" "two" "|stage1|stageShape1,/World/thing";
    usdPrim -unload "|stage1|stageShape1,/World/heavy";
    usdPrim -addReference "./asset.usd" -targetPrim "/Asset" "|stage1|stageShape1,/World/thing";
    usdPrim -assignMaterial "/Looks/red" "|stage1|stageShape1,/World/thing";
    usdPrim -q "|stage1|stageShape1,/World/thing";

A prim's state reads as attributes that aren't USD properties: `active`, `loaded`, `instanceable`, `kind`, `variant:<set>`, and the
channels `translateX` to `scaleZ` whatever ops hold them. Right click a prim in the Outliner for its menu.

## Layers

Windows > USD Layer Editor shows the session layer and the root with their sublayers. Double click sends edits to a layer. Mute and
lock are checkboxes; the context menu saves, saves as, reloads, clears, adds and removes sublayers, and flattens.

    mayaUsdEditTarget -e -editTarget "/show/shot_layer.usda" "stageShape1";
    mayaUsdLayerEditor -insertSubPath 0 "./over.usda" "/show/shot.usda";
    mayaUsdLayerEditor -muteLayer 1 "stageShape1" "/show/over.usda";
    usdLayer -q -proxy "stageShape1";
    usdLayer -save "/show/shot.usda";
    usdLayer -saveAs "/show/copy.usda" -proxy "stageShape1" "/show/shot.usda";
    AL_usdmaya_LayerCurrentEditTarget -q -proxy "stageShape1";
    AL_usdmaya_LayerGetLayers -proxy "stageShape1" -rootLayer;
    AL_usdmaya_LayerSave -proxy "stageShape1" -string;

Saving a scene keeps layers with no edits as paths. A layer with unsaved edits and the session layer go into the proxy's attributes
in the scene file, and come back when it opens. The File menu's save asks first: save them to their files, or keep them in the scene.

## Between prims and nodes

    mayaUsdEditAsMaya "|stage1|stageShape1,/World/thing";     // nodes under __mayaUsd__, the prim hidden
    mayaUsdMergeToUsd "thing";                                 // back to the edit target; the nodes go
    mayaUsdDiscardEdits "thing";                               // the nodes go; the prim shows as it was
    mayaUsdDuplicate "|stage1|stageShape1,/World/thing" "";    // a copy as nodes, nothing linked
    mayaUsdDuplicate "pCube1" "|stage1|stageShape1,/World";    // a copy as a prim
    AL_usdmaya_TranslatePrim -proxy "stageShape1" -importPaths "/World/thing";

A `MayaReference` or `ALMayaReference` prim's file loads as a file reference, under a group where the prim stands. A cache bakes it
to a USD file and references that from the prim.

    mayaUsdLoadMayaReference "|stage1|stageShape1,/World/rig";
    mayaUsdCacheMayaReference -file "rig_cache.usdc" -frameRange 1 120 "rig_group";

`usdDriveFromNode "|stage1|stageShape1,/World/thing" -driver "ctrl"` makes a transform drive a prim live, through the session layer.

## Cameras

`usdCamera -lookThrough "|stage1|stageShape1,/World/shot"` makes a `ProxyUsdGeomCamera` following a `Camera` prim and looks through
it. Lights and cameras draw as small diamonds.

## Python

    from pxr import Usd, UsdGeom, Sdf, Gf, Vt, Tf
    stage = Usd.Stage.CreateNew("hello.usda")
    mesh = UsdGeom.Mesh.Define(stage, "/hello")
    mesh.CreatePointsAttr([(0, 0, 0), (1, 0, 0), (0, 1, 0)])
    UsdGeom.Xformable(mesh).AddTranslateOp().Set(Gf.Vec3d(1, 2, 3), 1)
    stage.GetRootLayer().Save()

    import mayaUsd.ufe, mayaUsd.lib
    stage = mayaUsd.ufe.getStage("|stage1|stageShape1")
    prim = mayaUsd.lib.GetPrim("|stage1|stageShape1,/World/thing")

This is a subset, written in Python over the command `usdPy`. `Usd`: `Stage`, `Prim`, `Attribute`, `Relationship`, `TimeCode`,
`VariantSets`, `EditTarget`. `Sdf`: `Path`, `Layer`, `ValueTypeNames`. `UsdGeom`: `Xform`, `Scope`, `Mesh`, the implicit shapes,
`Camera`, `Xformable` and `XformOp`, `PrimvarsAPI`, `XformCache`, the stage's up axis and units. `Gf`: vectors, `Quatf`, `Matrix4d`.
`Vt`: arrays as lists. `UsdShade`: defining a `Material` and `Shader`, `MaterialBindingAPI.Bind`. `Ar.SetResolveHook`. Anything else
raises `AttributeError`. Values cross as JSON, so whole arrays of a big mesh are slow.

## Further

- `usdStats "stageShape1"` and Windows > USD Stats: prims by type, instances, layers and their sizes.
- `usdCollections "|stage1|stageShape1,/World/set"`: a prim's collections and what's in them.
- `usdResolver -addSearchPath "/show/assets"`, `usdResolver -map "logical://thing.usda" "/show/thing.usda"`, and from Python
  `pxr.Ar.SetResolveHook(fn)`, for stages opened afterwards. The logical name needs a USD extension. The hook must not touch stages.
- `scriptJob -event "usdStageChanged" "proc"`, also `usdLayerChanged` and `usdEditTargetChanged`.
- Translators from Python: subclass `AL.usdmaya.TranslatorBase`, or `aztek.usd.TranslatorBase`, set `primType`, write
  `importObject(self, prim, parent)`, and call `register()`. Import and pull call it for prims of that type.
- Value clips play as the crate composes them: a clip's samples lose to a `default` in the same layer, as in Pixar's library.
