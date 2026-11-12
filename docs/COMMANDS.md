# Command reference

Generated from the command registry by `cargo run -p aztek_host --example gen_commands`; do not edit by hand.
Commands take the reference's flags, long or short, in MEL and in Python (`aztek.cmds` or `maya.cmds`).

1233 commands.

## AL_usdmaya_LayerCreateLayer

Add a sublayer to the root layer: -open a file, or a new anonymous one.

| Flag | Short | Description |
| --- | --- | --- |
| `proxy` | `p` | The proxy shape. |
| `open` | `o` | The file. |

## AL_usdmaya_LayerCurrentEditTarget

The layer edits go to: -q reads, -layer sets.

| Flag | Short | Description |
| --- | --- | --- |
| `proxy` | `p` | The proxy shape. |
| `query` | `q` | Read. |
| `layer` | `l` | The layer's identifier. |

## AL_usdmaya_LayerGetLayers

The stage's layers; -rootLayer, -sessionLayer or -muted narrow it.

| Flag | Short | Description |
| --- | --- | --- |
| `proxy` | `p` | The proxy shape. |
| `rootLayer` | `rl` | Only the root. |
| `sessionLayer` | `sl` | Only the session. |
| `muted` | `m` | Only muted ones. |

## AL_usdmaya_LayerSave

Write a layer: to its file, to -filename, or as text with -string.

| Flag | Short | Description |
| --- | --- | --- |
| `proxy` | `p` | The proxy shape. |
| `layer` | `l` | The layer's identifier; the root by default. |
| `filename` | `f` | The file. |
| `string` | `s` | Return its text. |

## AL_usdmaya_ListEvents

The events USD work raises, for scriptJob.

## AL_usdmaya_ProxyShapeFindLoadable

Prims with payloads under -primPath of a proxy; -loaded or -unloaded narrow it.

Arguments: proxy

| Flag | Short | Description |
| --- | --- | --- |
| `primPath` | `pp` | Only under this prim. |
| `loaded` | `l` | Only loaded ones. |
| `unloaded` | `ul` | Only unloaded ones. |

## AL_usdmaya_ProxyShapeImport

A proxy shape on a USD file, as the Animal Logic USD plug-in makes one. Returns the shapes.

Arguments: transform to go under

| Flag | Short | Description |
| --- | --- | --- |
| `file` | `f` | The file. |
| `name` | `n` | The transform's name. |
| `primPath` | `pp` | The prim to show. |
| `excludePrimPath` | `epp` | Prims to leave out. |
| `unloaded` | `ul` | Leave payloads unloaded. |
| `stageId` | `sid` | An open stage's id. |

## AL_usdmaya_TranslatePrim

As the Animal Logic USD plug-in: -ip paths imports prims as nodes, -tp paths tears them down, in the proxy -p.

| Flag | Short | Description |
| --- | --- | --- |
| `proxy` | `p` | The proxy shape. |
| `importPaths` | `ip` | Prims to import, separated by commas. |
| `teardownPaths` | `tp` | Prims to tear down, separated by commas. |
| `forceImport` | `fi` | Accepted. |
| `forceDefault` | `fd` | Accepted. |

## AppendToPolygonTool

The Append to Polygon tool.

## ArtPaintAttrTool

The paint attribute tool, on deformers' weights.

## ArtPaintAttrToolOptions

The paint attribute tool and its settings.

## ArtPaintSkinWeightsTool

The paint skin weights tool.

## ArtPaintSkinWeightsToolOptions

The paint skin weights tool and its settings.

## AttributeEditor

Show the Attribute Editor.

## AztekFeedback

Report that something doesn't feel right.

## CVCurveTool

The CV curve tool: click to place CVs, Enter to finish.

## CreateAmbientLight

Make a light, selected: Create > Lights.

| Flag | Short | Description |
| --- | --- | --- |
| `rgb` | `rgb` | Colour. |
| `intensity` | `i` | intensity |
| `shadowSamples` | `sh` | shadowRays |
| `useRayTraceShadows` | `rs` | useRayTraceShadows |
| `ambientShade` | `ambientShade` | ambientShade |
| `discRadius` | `drs` | shadowRadius |

## CreateAreaLight

Make a light, selected: Create > Lights.

| Flag | Short | Description |
| --- | --- | --- |
| `rgb` | `rgb` | Colour. |
| `intensity` | `i` | intensity |
| `shadowSamples` | `sh` | shadowRays |
| `useRayTraceShadows` | `rs` | useRayTraceShadows |
| `decayRate` | `d` | decayRate |
| `softShadow` | `ss` | castSoftShadows |
| `discRadius` | `drs` | lightRadius |

## CreateDirectionalLight

Make a light, selected: Create > Lights.

| Flag | Short | Description |
| --- | --- | --- |
| `rgb` | `rgb` | Colour. |
| `intensity` | `i` | intensity |
| `shadowSamples` | `sh` | shadowRays |
| `useRayTraceShadows` | `rs` | useRayTraceShadows |
| `decayRate` | `d` | decayRate |
| `softShadow` | `ss` | castSoftShadows |
| `discRadius` | `drs` | lightRadius |

## CreatePointLight

Make a light, selected: Create > Lights.

| Flag | Short | Description |
| --- | --- | --- |
| `rgb` | `rgb` | Colour. |
| `intensity` | `i` | intensity |
| `shadowSamples` | `sh` | shadowRays |
| `useRayTraceShadows` | `rs` | useRayTraceShadows |
| `decayRate` | `d` | decayRate |
| `softShadow` | `ss` | castSoftShadows |
| `discRadius` | `drs` | lightRadius |

## CreatePolygonTool

The Create Polygon tool.

## CreateSpotLight

Make a light, selected: Create > Lights.

| Flag | Short | Description |
| --- | --- | --- |
| `rgb` | `rgb` | Colour. |
| `intensity` | `i` | intensity |
| `shadowSamples` | `sh` | shadowRays |
| `useRayTraceShadows` | `rs` | useRayTraceShadows |
| `decayRate` | `d` | decayRate |
| `softShadow` | `ss` | castSoftShadows |
| `discRadius` | `drs` | lightRadius |
| `coneAngle` | `ca` | coneAngle |
| `penumbra` | `p` | penumbraAngle |
| `dropOff` | `do` | dropoff |
| `barnDoors` | `bd` | barnDoors |
| `leftBarnDoorAngle` | `lbd` | leftBarnDoor |
| `rightBarnDoorAngle` | `rbd` | rightBarnDoor |
| `topBarnDoorAngle` | `tbd` | topBarnDoor |
| `bottomBarnDoorAngle` | `bbd` | bottomBarnDoor |

## CreateVolumeLight

Make a light, selected: Create > Lights.

| Flag | Short | Description |
| --- | --- | --- |
| `rgb` | `rgb` | Colour. |
| `intensity` | `i` | intensity |
| `shadowSamples` | `sh` | shadowRays |
| `useRayTraceShadows` | `rs` | useRayTraceShadows |
| `decayRate` | `d` | decayRate |
| `softShadow` | `ss` | castSoftShadows |
| `discRadius` | `drs` | lightRadius |

## DetachComponent

Detach the selected vertices or edges: polyDetachEdges on the selection.

## DopeSheetEditor

Open the Dope Sheet.

## EPCurveTool

The EP curve tool: click to place edit points, Enter to finish.

## EditMembershipTool

The edit membership tool: which points a deformer holds.

## FBXClose

Let go of the file FBXRead read.

## FBXExport

Export to FBX as scripts written for the reference ask: FBXExport -f "path" writes everything, with -s the selection.

| Flag | Short | Description |
| --- | --- | --- |
| `file` | `f` | The file to write. |
| `selected` | `s` | Only the selection. |

## FBXExportAnimation

Animation. -v value sets it, -q gives it. Starts as true.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXExportAnimationOnly

Animation only. -v value sets it, -q gives it. Starts as false.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXExportApplyConstantKeyReducer

Constant Key Reducer. -v value sets it, -q gives it. Starts as false.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXExportAudio

Audio. -v value sets it, -q gives it. Starts as true.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXExportAutoUnits

Automatic units. -v value sets it, -q gives it. Starts as true.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXExportAxisConversionMethod

How a change of up axis is made. -v value sets it, -q gives it. Starts as "convertAnimation".

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXExportBakeComplexAnimation

Bake Animation. -v value sets it, -q gives it. Starts as false.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXExportBakeComplexEnd

Bake end. -v value sets it, -q gives it. Starts as 120.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXExportBakeComplexStart

Bake start. -v value sets it, -q gives it. Starts as 1.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXExportBakeComplexStep

Bake step. -v value sets it, -q gives it. Starts as 1.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXExportBakeResampleAnimation

Resample All. -v value sets it, -q gives it. Starts as false.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXExportCacheFile

Geometry cache file(s). -v value sets it, -q gives it. Starts as false.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXExportCameras

Cameras. -v value sets it, -q gives it. Starts as true.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXExportColladaFrameRate

Collada: frame rate. -v value sets it, -q gives it. Starts as 24.0.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXExportColladaSingleMatrix

Collada: single matrix. -v value sets it, -q gives it. Starts as true.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXExportColladaTriangulate

Collada: triangulate. -v value sets it, -q gives it. Starts as true.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXExportConstraints

Constraints. -v value sets it, -q gives it. Starts as false.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXExportConvertNurbsSurfaceTo

Convert NURBS surface to. -v value sets it, -q gives it. Starts as "nurbs".

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXExportConvertUnitString

File units converted to; the scene's unit unless set. -v value sets it, -q gives it. Starts as "".

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXExportCurveFilters

Curve filters. -v value sets it, -q gives it. Starts as false.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXExportDxfDeformation

DXF: deformation. -v value sets it, -q gives it. Starts as true.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXExportDxfTriangulate

DXF: triangulate. -v value sets it, -q gives it. Starts as true.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXExportEmbeddedTextures

Embed Media: the texture files go inside the FBX file. -v value sets it, -q gives it. Starts as false.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXExportFileVersion

The FBX version written. -v value sets it, -q gives it. Starts as "FBX202000".

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXExportGenerateLog

Generate log data. -v value sets it, -q gives it. Starts as true.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXExportHardEdges

Split per-vertex Normals. -v value sets it, -q gives it. Starts as false.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXExportInAscii

ASCII rather than binary. -v value sets it, -q gives it. Starts as false.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXExportIncludeChildren

Include Children. -v value sets it, -q gives it. Starts as true.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXExportInputConnections

Input Connections. -v value sets it, -q gives it. Starts as true.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXExportInstances

Preserve Instances. -v value sets it, -q gives it. Starts as false.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXExportLights

Lights. -v value sets it, -q gives it. Starts as true.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXExportNullsAsLocators

Convert to Null Objects. -v value sets it, -q gives it. Starts as false.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXExportQuaternion

Quaternion interpolation mode. -v value sets it, -q gives it. Starts as "resample".

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXExportQuickSelectSetAsCache

The set exported as a geometry cache. -v value sets it, -q gives it. Starts as "".

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXExportReferencedAssetsContent

Referenced Asset Content. -v value sets it, -q gives it. Starts as true.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXExportRemoveSingleKey

Remove single key. -v value sets it, -q gives it. Starts as false.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXExportScaleFactor

The scale factor. -v value sets it, -q gives it. Starts as 1.0.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXExportSelectionSets

Selection Sets. -v value sets it, -q gives it. Starts as false.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXExportShapes

Blend Shapes. -v value sets it, -q gives it. Starts as true.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXExportShowUI

Show the warnings manager. -v value sets it, -q gives it. Starts as true.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXExportSkeletonDefinitions

Skeleton definitions. -v value sets it, -q gives it. Starts as false.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXExportSkins

Skins. -v value sets it, -q gives it. Starts as true.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXExportSmoothMesh

Smooth Mesh. -v value sets it, -q gives it. Starts as true.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXExportSmoothingGroups

Smoothing Groups. -v value sets it, -q gives it. Starts as false.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXExportSplitAnimationIntoTakes

Takes to write: -v name start end adds one, -c clears them, -q lists them.

Arguments: start end

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `clear` | `c` | Clear the takes. |
| `query` | `q` | Give the value. |

## FBXExportTangents

Tangents and Binormals. -v value sets it, -q gives it. Starts as false.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXExportTriangulate

Triangulate. -v value sets it, -q gives it. Starts as false.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXExportUpAxis

The file's up axis; the scene's unless set. -v value sets it, -q gives it. Starts as "".

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXExportUseSceneName

Use scene name for the take. -v value sets it, -q gives it. Starts as false.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXExtPlugin

FBX extension plug-ins: -l lists them. None are installed.

Arguments: plugin

| Flag | Short | Description |
| --- | --- | --- |
| `list` | `l` | List them. |
| `state` | `state` | on or off. |
| `opts` | `opts` | Options. |
| `query` | `q` | Give the value. |

## FBXGetTakeComment

A take's comment, by index from 1.

Arguments: index

## FBXGetTakeCount

How many takes the file FBXRead read has.

## FBXGetTakeIndex

The index, from 1, of the first take with this name.

Arguments: name

## FBXGetTakeLocalTimeSpan

A take's start and stop, in frames at the scene's rate, by index from 1.

Arguments: index

## FBXGetTakeName

The name of a take of the file FBXRead read, by index from 1.

Arguments: index

## FBXGetTakeReferenceTimeSpan

A take's start and stop, in frames at the scene's rate, by index from 1.

Arguments: index

## FBXImport

Import an FBX file by the FBXImport settings: FBXImport -f "path" [-t take].

| Flag | Short | Description |
| --- | --- | --- |
| `file` | `f` | The file to read. |
| `take` | `t` | The take: 0 none, -1 the last. |

## FBXImportAnimation

Animation. -v value sets it, -q gives it. Starts as true.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXImportAudio

Audio. -v value sets it, -q gives it. Starts as true.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXImportAutoUnits

Automatic units: the file's unit is converted to the scene's. -v value sets it, -q gives it. Starts as true.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXImportAxisConversionEnable

Axis conversion: a file with another up axis turns its top nodes. -v value sets it, -q gives it. Starts as true.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXImportCacheFile

Geometry Cache File(s). -v value sets it, -q gives it. Starts as true.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXImportCameras

Cameras. -v value sets it, -q gives it. Starts as true.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXImportConstraints

Constraints. -v value sets it, -q gives it. Starts as true.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXImportConvertDeformingNullsToJoint

Deforming Elements to Joints: a null a skin is bound to comes in as a joint. -v value sets it, -q gives it. Starts as true.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXImportConvertUnitString

File Units converted to; the scene's unit unless set. -v value sets it, -q gives it. Starts as "".

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXImportDxfObjectDerivation

DXF: object derivation. -v value sets it, -q gives it. Starts as "layer".

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXImportDxfReferenceNode

DXF: reference node. -v value sets it, -q gives it. Starts as false.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXImportDxfWeldVertice

DXF: weld vertices. -v value sets it, -q gives it. Starts as true.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXImportFillTimeline

Fill Timeline with the take's range. -v value sets it, -q gives it. Starts as false.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXImportForcedFileAxis

Read the file as if it said this up axis. -v value sets it, -q gives it. Starts as "disabled".

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXImportGenerateLog

Generate log data. -v value sets it, -q gives it. Starts as true.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXImportHardEdges

Combine per-vertex Normals. -v value sets it, -q gives it. Starts as false.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXImportLights

Lights. -v value sets it, -q gives it. Starts as true.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXImportMergeAnimationLayers

Bake Animation Layers. -v value sets it, -q gives it. Starts as true.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXImportMergeBackNullPivots

Update Pivots from Nulls: pre and post rotation go to joint orient and rotate axis on every node, not only on joints. -v value sets it, -q gives it. Starts as false.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXImportMode

File Content: add (Add), merge (Add and update animation), exmerge (Update animation). -v value sets it, -q gives it. Starts as "merge".

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXImportOpticalMarkers

Optical Markers. -v value sets it, -q gives it. Starts as false.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXImportPreNormalizeWeights

Pre-Normalize Weights: each point's weights add up to one. -v value sets it, -q gives it. Starts as true.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXImportProtectDrivenKeys

Protect Driven Keys. -v value sets it, -q gives it. Starts as false.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXImportQuaternion

Quaternion Interpolation Mode. -v value sets it, -q gives it. Starts as "resample".

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXImportRemoveBadPolysFromMesh

Test meshes and remove bad polygons. -v value sets it, -q gives it. Starts as true.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXImportResamplingRateSource

The key rate when resampling. -v value sets it, -q gives it. Starts as "Scene".

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXImportScaleFactor

The scale applied to the file's lengths when FBXImportScaleFactorEnable is on. -v value sets it, -q gives it. Starts as 1.0.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXImportScaleFactorEnable

Use FBXImportScaleFactor rather than the file's unit. -v value sets it, -q gives it. Starts as false.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXImportSetLockedAttribute

Channels locked in the file stay locked. -v value sets it, -q gives it. Starts as false.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXImportSetMayaFrameRate

Take the file's frame rate. -v value sets it, -q gives it. Starts as false.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXImportSetTake

The take FBXImport reads: -ti index, 1 the first, -1 the last, 0 none.

Arguments: index

| Flag | Short | Description |
| --- | --- | --- |
| `takeIndex` | `ti` | The take. |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXImportShapes

Blend Shapes. -v value sets it, -q gives it. Starts as true.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXImportShowUI

Show the warnings manager. -v value sets it, -q gives it. Starts as true.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXImportSkeletonType

Skeleton Definitions. -v value sets it, -q gives it. Starts as "none".

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXImportSkins

Skins: skin clusters with the file's weights and bind matrices. -v value sets it, -q gives it. Starts as true.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXImportSmoothingGroups

Smoothing Groups: edges come in hard or soft as the file's smoothing says. -v value sets it, -q gives it. Starts as true.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXImportUnlockNormals

Unlock Normals: the file's normals are not kept; the mesh works its own out. -v value sets it, -q gives it. Starts as false.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXImportUpAxis

The up axis the file is converted to; the scene's unless set. -v value sets it, -q gives it. Starts as "".

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXLoadExportPresetFile

Load an export preset file: -f path.

| Flag | Short | Description |
| --- | --- | --- |
| `file` | `f` | The preset file. |

## FBXLoadImportPresetFile

Load an import preset file: -f path.

| Flag | Short | Description |
| --- | --- | --- |
| `file` | `f` | The preset file. |

## FBXLoadMBExportPresetFile

The MotionBuilder export preset: constraints and skeleton definitions on.

## FBXLoadMBImportPresetFile

The MotionBuilder import preset: constraints and skeleton definitions on.

## FBXPopSettings

Bring back the settings FBXPushSettings kept.

## FBXPushSettings

Keep every import and export setting, for FBXPopSettings to bring back.

## FBXRead

Read a file's takes without importing it: -f path. The take commands then ask about it.

| Flag | Short | Description |
| --- | --- | --- |
| `file` | `f` | The file. |

## FBXResamplingRate

The rate keys are made at when animation is resampled. -v value sets it, -q gives it. Starts as 24.0.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | The value. |
| `query` | `q` | Give the value. |

## FBXResetExport

Every export setting back to the Autodesk Media & Entertainment preset.

## FBXResetImport

Every import setting back to the Autodesk Media & Entertainment preset.

## FourViewLayout

Four views: top, persp, front, side.

## FrameAll

Frame everything in the active view.

## FrameSelected

Frame the selection in the active view.

## GetHIKNodeName

The name of a slot by its id.

Arguments: id

## GhostSelected

Turn ghosting on for the selected objects.

Arguments: objects (the selection when none)

## GraphEditor

Open the Graph Editor.

## HIKCharacterControlsTool

The Human IK window.

## HumanIK

The Human IK window: characters, sources, definitions and controls.

## InsertJointTool

The insert joint tool: drag from a joint to add one on the bone.

## InvertSelection

The runtime command for invertSelection.

## JointTool

The joint tool: click to place joints.

## MoveTool

The move tool.

## MultiCutTool

The Multi-Cut tool.

## NodeEditorWindow

Open the Node Editor.

## OffsetEdgeLoopTool

The Offset Edge Loop tool.

## OutlinerWindow

Show the Outliner.

## PlaybackToggle

Start or stop playback.

## PolyCreaseTool

The Crease tool.

## PreferencesWindow

Open the Preferences.

## QuadDrawTool

The Quad Draw tool.

## RemoveConstraintTarget

Remove the selected targets from the constraints on the object selected last.

Arguments: targets, then the constrained object

## RotateTool

The rotate tool.

## ScaleTool

The scale tool.

## ScriptEditor

Open the Script Editor.

## SculptMeshActivateBrushSculpt

The sculpt brush.

## SelectTool

The select tool.

## SetDrivenKeyOptions

Open the Set Driven Key window.

## ShapeEditor

The Shape Editor: blend shapes and their targets.

## SingleViewLayout

One view: the perspective.

## SlideEdgeTool

The Slide Edge tool.

## SplitEdgeRingTool

The Insert Edge Loop tool.

## TargetWeldTool

The Target Weld tool.

## TextureViewWindow

Open the UV Editor.

## ToggleChannelsLayers

Show or hide the Channel Box / Layer Editor.

## ToolSettingsWindow

Show the Tool Settings.

## UVEditor

Open the UV Editor.

## UVSetEditor

Open the UV Set Editor.

## UnghostAll

Turn ghosting off for every object.

Arguments: objects (the selection when none)

## UnghostSelected

Turn ghosting off for the selected objects.

Arguments: objects (the selection when none)

## UsdLayerEditor

Open the USD Layer Editor.

## UsdStats

Open the USD Stats window.

## about

About the application and the system: -v, -os, -win, -linux, -mac, -batch, -api.

| Flag | Short | Description |
| --- | --- | --- |
| `version` | `v` | What to ask. |
| `apiVersion` | `api` | What to ask. |
| `operatingSystem` | `os` | What to ask. |
| `windows` | `win` | What to ask. |
| `nt` | `nt` | What to ask. |
| `linux` | `li` | What to ask. |
| `macOS` | `mac` | What to ask. |
| `batch` | `b` | What to ask. |
| `is64` | `x64` | What to ask. |
| `uiLanguage` | `uil` | What to ask. |
| `codeset` | `cs` | What to ask. |
| `product` | `p` | What to ask. |
| `application` | `a` | What to ask. |

## addAttr

Add an attribute to objects (the reference's addAttr): attributeType for a single value or, with multi, an array of them; dataType for strings and arrays (stringArray, doubleArray, Int32Array, vectorArray). attributeType='compound' groups the attributes then added with parent=; a multi compound gives each element every child, node.targets[2].weight. Compounds nest one level. Saved with the scene. Returns the new plugs.

Arguments: objects

| Flag | Short | Description |
| --- | --- | --- |
| `longName` | `ln` | The attribute's name. |
| `shortName` | `sn` | A short name. |
| `niceName` | `nn` | The label editors show. |
| `attributeType` | `at` | bool, long, float, double, double3, enum, matrix or compound (Aztek's int, vec3 too). |
| `parent` | `p` | The compound this attribute is a child of. |
| `numberOfChildren` | `nc` | Accepted for existing scripts: a compound takes any number of children. |
| `dataType` | `dt` | string, stringArray, doubleArray, floatArray, Int32Array, vectorArray or matrix. |
| `multi` | `m` | An array of attributeType: any number of elements, attr[0], attr[1] ... |
| `defaultValue` | `dv` | Default value (each element's, for an array). |
| `minValue` | `min` | Lowest value it takes: lower values are clamped. |
| `maxValue` | `max` | Highest value it takes: higher values are clamped. |
| `softMinValue` | `smn` | Slider minimum. |
| `softMaxValue` | `smx` | Slider maximum. |
| `enumName` | `en` | Enum labels, 'low:medium:high'. |
| `keyable` | `k` | Show it in the channel box, where it can be keyed. |
| `hidden` | `h` | Hide it from editors. |
| `usedAsColor` | `uac` | A double3 holding a colour: editors show a swatch. |
| `usedAsFilename` | `uaf` | A string holding a file path: editors offer a file browser. |
| `category` |  | A heading the Attribute Editor groups it under. |
| `doc` |  | What it is for: editors show it as a tooltip. |
| `query` | `q` | Query. |
| `edit` | `e` | Edit. |
| `exists` | `ex` | With -q: whether it exists. |
| `hasMinValue` | `hnv` | Whether it has a hard minimum. |
| `hasMaxValue` | `hxv` | Whether it has a hard maximum. |
| `hasSoftMinValue` | `hsn` | Accepted for existing scripts. |
| `hasSoftMaxValue` | `hsx` | Accepted for existing scripts. |
| `indexMatters` | `im` | Accepted: a multi's indices always matter here. |
| `readable` | `r` | Accepted for existing scripts. |
| `writable` | `w` | Accepted for existing scripts. |
| `storable` | `s` | Accepted for existing scripts. |
| `cachedInternally` | `ci` | Accepted for existing scripts. |
| `internalSet` | `is` | Accepted for existing scripts. |
| `disconnectBehaviour` | `dcb` | Accepted for existing scripts. |
| `fromPlugin` | `fp` | Accepted for existing scripts. |
| `binaryTag` | `bt` | Accepted for existing scripts. |
| `proxy` | `pxy` | Accepted for existing scripts. |
| `usedAsProxy` | `uap` | Accepted for existing scripts. |

## addMetadata

Attach a metadata stream of a structure to objects: -structure, -stn, -cn, -idt. Returns what it went on.

Arguments: objects or components (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `channelName` | `cn` | The channel: vertex, edge, face or your own. |
| `channelType` | `cht` | Obsolete: as -cn. |
| `endIndex` | `eix` | Last index of a range. |
| `index` | `idx` | An index. |
| `indexType` | `idt` | The index's type: numeric unless given. |
| `memberName` | `mn` | The structure's member. |
| `scene` | `scn` | On the scene rather than a node. |
| `startIndex` | `six` | First index of a range. |
| `streamName` | `stn` | The stream. |
| `structure` | `str` | The structure. |

## addModifier

Add a modifier to the top of a mesh's stack. params sets its parameters. Returns its name. modifierKinds(docs=True) also lists kinds extensions add later. Kinds: - array: Repeats the mesh in a row, around a circle or in a grid, each copy stepped from the last. Use it for railings, stairs, tiles, gear teeth and bolts round a flange. Radial with weld on turns one modelled wedge into an exact N-fold whole. - autoBevel: Bevels the fold edges it finds live on its input by angle, side and length rules, plus edges picked by hand. Use it for game-art bevels that keep following the mesh as you edit it. - bevel: Bevels every edge sharper than an angle so hard corners catch the light. A quick live bevel; Auto Bevel has finer edge rules and hand-picked edges. - boolean: A boolean kept live: cuts, joins or intersects this mesh with a cutter mesh, so moving or editing the cutter updates the result. Stack one per cutter. Apply bakes it. - decimate: Reduces the triangle count while keeping the shape, silhouette, UVs and normals. Use it for LODs or to lighten scans and sculpts. - extrude: Extrudes faces, or grows strips off edges, kept live: the faces or edges selected when it was added follow later edits under it, and its thickness and offset stay adjustable. Without a selection it extrudes every face. - mirror: Adds a mirrored copy across a plane through the object's origin and welds the seam. Model one half of a symmetrical object and let this build the other. - solidify: Gives a surface thickness: an inner shell offset along the normals, joined to the outer one at open borders. Use it to turn planes and open shells into solid panels. - splitRings: Subdivides one way, kept live: loops across the rings of the edges or faces selected when it was added, curved to the surface. Without a selection it splits the whole mesh. - subdivision: Smooths the mesh with Catmull-Clark subdivision. Use it to turn a low-poly cage into a smooth high-poly surface. - triangulate: Splits every face into triangles. Use it to see, or lock in, the triangulation an engine will get. - weightedNormals: Weights vertex normals by face area so big flat faces shade flat and the bevels take the curvature. Use it on bevelled hard-surface game meshes.

Arguments: mesh

| Flag | Short | Description |
| --- | --- | --- |
| `kind` |  | Modifier kind, such as bevel or mirror (see Kinds). |
| `params` |  | {param: value} to set on the new modifier. |

## addToSet

Add members (the selection by default) to a set. Returns the set's members.

Arguments: set, members

## aimConstraint

Constrain the object named (or selected) last to the targets before it; a second call adds targets. -mo keeps the current offset; -w sets the targets' weight; -e and -q work on the constraint or the object it drives. Returns [constraint].

Arguments: targets, then the object to drive or the constraint

| Flag | Short | Description |
| --- | --- | --- |
| `maintainOffset` | `mo` | Keep the object where it is now relative to the targets. |
| `weight` | `w` | Weight of the targets named (1). |
| `name` | `n` | Name of the constraint. |
| `offset` | `o` | Added to the result. |
| `remove` | `rm` | Remove the targets named. Removing the last deletes the constraint. |
| `targetList` | `tl` | Query: the targets. |
| `weightAliasList` | `wal` | Query: the targets' weight attributes. |
| `aimVector` | `aim` | The local axis that points at the target. |
| `upVector` | `u` | The local axis that points up. |
| `worldUpVector` | `wu` | The world direction up means. |
| `worldUpType` | `wut` | scene, object, objectrotation, vector or none. |
| `worldUpObject` | `wuo` | The object for worldUpType object and objectrotation. |
| `interpType` | `int` | How rotations blend: noflip, average, shortest, longest, cache, or 0 to 4. |
| `skipTranslate` | `st` | Translate axes to leave alone: x, y, z or none. May be used more than once. |
| `skipRotate` | `sr` | Rotate axes to leave alone: x, y, z or none. May be used more than once. |
| `skip` | `sk` | Axes of this constraint's channel to leave alone: x, y, z or none. May be used more than once. |
| `skipScale` |  | Scale axes to leave alone: x, y, z or none. |
| `edit` | `e` | Change the constraint. |
| `query` | `q` | Ask about the constraint. |

## air

Make a air field acting on the selected bodies, or -q/-e one. Returns [field].

Arguments: bodies to act on

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query a value. |
| `edit` | `e` | Edit an existing node. |
| `name` | `n` | Name of the new node. |
| `magnitude` | `m` | Strength. |
| `attenuation` | `att` | How fast it weakens with distance. |
| `maxDistance` | `mxd` | How far it reaches (turns the limit on). |
| `useMaxDistance` | `umd` | Limit its reach. |
| `directionX` | `dx` | Direction X. |
| `directionY` | `dy` | Direction Y. |
| `directionZ` | `dz` | Direction Z. |
| `speed` | `s` | Air: wind speed. |
| `inheritVelocity` | `iv` | Air: kept motion. |
| `frequency` | `f` | Turbulence: noise frequency. |
| `minDistance` | `mnd` | Newton: least distance. |
| `position` | `pos` | Where it is. |

## aliasAttr

aliasAttr('alias', 'node.attr') names an attribute another way. -q lists a node's aliases and their attributes; -rm removes.

Arguments: alias plug, or a node with -q

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | List the aliases. |
| `remove` | `rm` | Remove the alias. |

## align

Lines objects' bounding boxes up: align -x Mid a b c. Min, Mid or Max per axis; -atl to the last object.

Arguments: objects

| Flag | Short | Description |
| --- | --- | --- |
| `alignX` | `x` | Min, Mid or Max. |
| `alignY` | `y` | Min, Mid or Max. |
| `alignZ` | `z` | Min, Mid or Max. |
| `alignToLead` | `atl` | To the last object rather than the whole set. |

## allNodeTypes

Every node type there is, by name. -ia is taken: Aztek has no abstract types to add.

| Flag | Short | Description |
| --- | --- | --- |
| `includeAbstract` | `ia` | Taken; there are no abstract types. |

## ambientLight

Create an ambient light, or query and edit one. Returns its shape.

Arguments: the light, to edit or query

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `name` | `n` | Its name. |
| `rgb` | `rgb` | Colour. |
| `shadowColor` | `sc` | Shadow colour. |
| `position` | `pos` | Position. |
| `rotation` | `rot` | Rotation. |
| `exclusive` | `exc` | Out of defaultLightSet: lights only what it's linked to. |
| `shadowDither` | `sd` | Accepted for scripts. |
| `intensity` | `i` | intensity |
| `shadowSamples` | `sh` | shadowRays |
| `useRayTraceShadows` | `rs` | useRayTraceShadows |
| `ambientShade` | `ambientShade` | ambientShade |
| `discRadius` | `drs` | shadowRadius |

## angleBetween

The rotation from -v1 to -v2: axis and angle in degrees, or -er Euler angles.

| Flag | Short | Description |
| --- | --- | --- |
| `vector1` | `v1` | From. |
| `vector2` | `v2` | To. |
| `euler` | `er` | Euler angles instead. |
| `constructionHistory` | `ch` | Accepted for existing scripts. |

## animCurveEditor

Script UI: animCurveEditor, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `areCurvesSelected` | `acs` |  |
| `autoFit` | `af` |  |
| `autoFitTime` | `aft` |  |
| `classicMode` | `cm` |  |
| `clipTime` | `ct` |  |
| `constrainDrag` | `cd` |  |
| `curvesShown` | `cs` |  |
| `denormalizeCurvesCommand` | `dcc` |  |
| `displayActiveKeyTangents` | `dat` |  |
| `displayActiveKeys` | `dak` |  |
| `displayInfinities` | `di` |  |
| `displayKeys` | `dk` |  |
| `displayNormalized` | `dn` |  |
| `displayTangents` | `dtn` |  |
| `displayValues` | `dv` |  |
| `keyingTime` | `kt` |  |
| `lookAt` | `la` |  |
| `menu` | `m` |  |
| `normalizeCurvesCommand` | `ncc` |  |
| `outliner` | `o` |  |
| `preSelectionHighlight` | `psh` |  |
| `renormalizeCurves` | `rnc` |  |
| `resultSamples` | `rs` |  |
| `resultScreenSamples` | `rss` |  |
| `resultUpdate` | `ru` |  |
| `showActiveCurveNames` | `acn` |  |
| `showBufferCurves` | `sb` |  |
| `showCurveNames` | `scn` |  |
| `showResults` | `sr` |  |
| `showUpstreamCurves` | `suc` |  |
| `simpleKeyView` | `skv` |  |
| `smoothness` | `s` |  |
| `snapTime` | `st` |  |
| `snapValue` | `sv` |  |
| `stackedCurves` | `sc` |  |
| `stackedCurvesMax` | `scx` |  |
| `stackedCurvesMin` | `scm` |  |
| `stackedCurvesSpace` | `scs` |  |
| `valueLinesToggle` | `vlt` |  |
| `control` | `ctl` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `filter` | `f` |  |
| `forceMainConnection` | `fmc` |  |
| `highlightConnection` | `hlc` |  |
| `lockMainConnection` | `lck` |  |
| `mainListConnection` | `mlc` |  |
| `panel` | `pnl` |  |
| `selectionConnection` | `slc` |  |
| `stateString` | `sts` |  |
| `unParent` | `up` |  |
| `unlockMainConnection` | `ulk` |  |
| `updateMainConnection` | `upd` |  |
| `useTemplate` | `ut` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## animDisplay

How animation shows, kept for scripts: -upd, -tc, -tco, -rae; -q reads.

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `modelUpdate` | `upd` | A setting. |
| `refAnimCurvesEditable` | `rae` | A setting. |
| `timeCode` | `tc` | A setting. |
| `timeCodeOffset` | `tco` | A setting. |

## animLayer

Create (name optional), edit (-e layer) or query (-q layer) an animation layer. The first layer brings BaseAnimation with it. -aso or -at put channels in the layer; setKeyframe -al keys them there.

Arguments: the layer, or for -afl and -bl the objects or plugs

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Return the value of the flags named instead of setting them. |
| `edit` | `e` | Change the flags named. |
| `addRelatedKG` | `akg` | With -aso or -rso: the keying groups the objects are in too. |
| `addSelectedObjects` | `aso` | Add the selected objects' keyable channels. |
| `affectedLayers` | `afl` | Query: the layers the objects or plugs are in. |
| `animCurves` | `anc` | Query: the layer's curves. |
| `attribute` | `at` | Plug(s) to add; query: the layer's plugs. |
| `baseAnimCurves` | `bac` | Query: the base curves of the layer's channels. |
| `bestAnimLayer` | `blr` | Query: the best layers for keying the selected objects, or the -at plugs. |
| `bestLayer` | `bl` | Query: the layer that will be keyed for the attribute. |
| `blendNodes` | `bld` | Query: the layer's blend nodes. |
| `children` | `c` | Query: the child layers. |
| `collapse` | `col` | Collapsed in the layer editor. |
| `copy` | `cp` | Copy from this layer. |
| `copyAnimation` | `ca` | Copy animation from this layer, on channels both layers hold. |
| `copyNoAnimation` | `cna` | Copy from this layer without the curves. |
| `excludeBoolean` | `ebl` | With -aso or -rso: not boolean channels. |
| `excludeDynamic` | `edn` | With -aso or -rso: not dynamic attributes. |
| `excludeEnum` | `een` | With -aso or -rso: not enum channels. |
| `excludeProxy` | `epr` | With -aso or -rso: not proxy attributes (there are none here). |
| `excludeRotate` | `ert` | With -aso or -rso: not rotate. |
| `excludeScale` | `esc` | With -aso or -rso: not scale. |
| `excludeTranslate` | `etr` | With -aso or -rso: not translate. |
| `excludeVisibility` | `evs` | With -aso or -rso: not visibility. |
| `exists` | `ex` | Query: whether the layer exists. |
| `extractAnimation` | `ea` | Move animation from this layer, on channels both layers hold. |
| `findCurveForPlug` | `fcv` | Query: the curve holding a plug's keys on the layer. |
| `forceUIRebuild` | `fur` | Rebuild the layers interface. |
| `forceUIRefresh` | `uir` | Refresh the layers interface. |
| `ghost` | `gh` | The layer's ghost button. |
| `ghostColor` | `ghc` | The layer's ghost and key tick colour index. |
| `layeredHierarchy` | `lh` | Query: all layers, bottom to top. |
| `layeredPlug` | `lp` | Query: the plug on the blend node for this plug on the layer. |
| `lock` | `l` | Lock the layer against keying. |
| `maxLayers` | `ml` | Query: the most layers the product supports. |
| `moveLayerAfter` | `mva` | Move the layer after this one. |
| `moveLayerBefore` | `mvb` | Move the layer before this one. |
| `mute` | `m` | Mute the layer. |
| `override` | `o` | Override what is below instead of adding. |
| `parent` | `p` | The parent layer. |
| `passthrough` | `pth` | An override layer's weight lets the layers below show through. |
| `preferred` | `prf` | The best layer algorithm tries this layer first. |
| `removeAllAttributes` | `raa` | Remove every channel. |
| `removeAttribute` | `ra` | Plug(s) to remove. |
| `removeSelectedObjects` | `rso` | Remove the selected objects' channels. |
| `root` | `r` | Query: the base layer. |
| `rotationAccumulationMode` | `ram` | component (0) or layer (1). |
| `scaleAccumulationMode` | `sam` | additive (0) or multiply (1). |
| `selected` | `sel` | A selected layer shows in the Time Slider and Graph Editor and takes keys. |
| `solo` | `s` | Solo the layer among its siblings. |
| `weight` | `w` | The layer's weight, 0 to 1. |
| `writeBlendnodeDestinations` | `wbd` | Write the plugs the blends drive into the blends. |

## animLayerMerge

Merge layers into one, keeping the result: -mt top or bottom selected layer, -ic children too, -rlm automatic, additive or override, -db delete the merged layers, -sb sample by, -sm keys only where needed, -fd its tolerance. With none named, the selected layers. Returns the layer merged into.

Arguments: the layers

| Flag | Short | Description |
| --- | --- | --- |
| `mergeTo` | `mt` | top or bottom (the default). |
| `includeChildren` | `ic` | Merge the children of merged layers too (on by default). |
| `resultLayerMode` | `rlm` | automatic, additive or override. |
| `deleteBaked` | `db` | Delete the merged layers (on by default). |
| `sampleBy` | `sb` | Frames between samples (1). |
| `smart` | `sm` | Keep the sources' key times, adding keys only past the tolerance. |
| `fidelity` | `fd` | The smart bake's tolerance. |

## animLayerOptions

The Animation Layer Editor's Options menu: -km selected, active or hybrid; -lm Lock Muted Layer. With nothing, the editor's snapshot.

| Flag | Short | Description |
| --- | --- | --- |
| `keyMode` | `km` | selected, active or hybrid. |
| `showFilter` | `sf` | allAffecting, active, selected or activeAndSelected: the curves other tools show. |
| `lockMuted` | `lm` | Muted layers take no keys. |

## annotate

A note in the viewport: -tx text, -p where; the line points at the object named. Returns the shape.

Arguments: object

| Flag | Short | Description |
| --- | --- | --- |
| `text` | `tx` | The note. |
| `point` | `p` | Where it goes. |

## applyAllModifiers

Bake every modifier of a mesh into its base mesh, leaving an empty stack over an editable source.

Arguments: meshes

## applyModifier

Bake a modifier (and everything below it in the stack) into the base mesh.

Arguments: modifier

## arclen

The length of a curve along its arc, world space.

Arguments: a curve (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `constructionHistory` | `ch` | Accepted for existing scripts. |

## arnoldRender

hrender under the name other scripts use.

Arguments: camera

| Flag | Short | Description |
| --- | --- | --- |
| `camera` | `cam` | Camera to render through. |
| `width` | `w` | Image width. |
| `height` | `h` | Image height. |
| `region` | `reg` | x0 y0 x1 y1, pixels from the top left. |
| `seq` | `seq` | Frames: "1 10", "1 10 2" or one frame. |
| `batch` | `b` | Render the frames the render settings name and write them. |
| `ipr` | `ipr` | start, stop, refresh or pause interactive rendering. |
| `async` | `as` | Start the current frame and return at once; the render view shows it as it refines. |

## arrayCenter

Set what an array modifier's copies turn and scale about. Give an object to centre on its transform (its position and axes, kept live as it moves; run again after moving the arrayed mesh), pivot=True to copy the mesh's pivot into centerPoint, or neither to go back to the object's origin.

Arguments: modifier, [object]

| Flag | Short | Description |
| --- | --- | --- |
| `pivot` |  | Centre on the arrayed mesh's current pivot. |

## artAttrCtx

The paint attribute tool's settings. Edit and query: -val value, -op opacity, -r radius, -sao the operation (absolute, additive, scale, smooth), -stP the stamp (gaussian, poly, solid, square), -asl what it paints, as cluster.cluster1.weights. -clear floods. -q -pa lists what a selected mesh offers.

Arguments: the tool's name

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `val` | The value the brush paints, 0 to 1. |
| `opacity` | `op` | How much of the value each dab applies. |
| `radius` | `r` | Brush radius in world units. |
| `selectedattroper` | `sao` | absolute (replace), additive, scale or smooth. |
| `stampProfile` | `stP` | gaussian, poly, solid or square. |
| `attrSelected` | `asl` | What it paints: cluster.cluster1.weights. |
| `paintableAttrs` | `pa` | Query: what the selected meshes offer. |
| `colorfeedback` | `cl` | Show the values in colour. |
| `clear` | `clr` | Flood: the operation over the selection's points. |
| `edit` | `e` | Edit mode. |
| `query` | `q` | Query mode. |

## artAttrPaintVertexCtx

A artAttrPaintVertexCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `colorRGBValue` | `cl3` |  |
| `colorAlphaValue` | `cl4` |  |
| `paintRGB` | `pc3` |  |
| `paintAlpha` | `pa` |  |
| `paintoperationtype` | `pot` |  |
| `selectedattroper` | `sao` |  |
| `radius` | `r` |  |
| `lowerradius` | `lr` |  |
| `opacity` | `op` |  |
| `value` | `val` |  |
| `stampProfile` | `stP` |  |
| `accopacity` | `aco` |  |
| `minvalue` | `mnv` |  |
| `maxvalue` | `mxv` |  |
| `clamp` | `cl` |  |
| `clamplower` | `cll` |  |
| `clampupper` | `clu` |  |
| `colorfeedback` | `cf` |  |
| `mappressure` | `mp` |  |
| `reflection` | `rfl` |  |
| `reflectionaxis` | `ra` |  |
| `outline` | `o` |  |
| `tablet` | `tab` |  |
| `flood` | `fl` |  |
| `floodselect` | `fs` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## artAttrSkinPaintCtx

The paint skin weights tool's settings. Edit and query: -val value, -op opacity, -r radius, -sao the operation (absolute, additive, scale, smooth), -stP the stamp (gaussian, poly, solid, square), -inf the influence. -clear floods the mesh with the value.

Arguments: the tool's name

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `val` | The value the brush paints, 0 to 1. |
| `opacity` | `op` | How much of the value each dab applies. |
| `radius` | `r` | Brush radius in world units. |
| `selectedattroper` | `sao` | absolute (replace), additive, scale or smooth. |
| `stampProfile` | `stP` | gaussian, poly, solid or square. |
| `influence` | `inf` | The joint painted. |
| `colorfeedback` | `cl` | Show the weights in colour. |
| `clear` | `clr` | Flood: paint the value over the selection's points. |
| `edit` | `e` | Edit mode. |
| `query` | `q` | Query mode. |

## artAttrSkinWeightCopy

Copy Vertex Weights: holds the weights of the selected vertex, or their mean when several are selected.

## artAttrSkinWeightPaste

Paste Vertex Weights: gives the selected vertices the weights last copied.

## artPuttyCtx

A artPuttyCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `mouldtypehead` | `mth` |  |
| `mouldtypemouse` | `mtm` |  |
| `mouldtypesmooth` | `mts` |  |
| `dispdecr` | `dde` |  |
| `dispincr` | `din` |  |
| `invertrefvector` | `irv` |  |
| `maxdisp` | `md` |  |
| `refvector` | `rv` |  |
| `stampDepth` | `sd` |  |
| `radius` | `r` |  |
| `lowerradius` | `lr` |  |
| `opacity` | `op` |  |
| `value` | `val` |  |
| `stampProfile` | `stP` |  |
| `accopacity` | `aco` |  |
| `minvalue` | `mnv` |  |
| `maxvalue` | `mxv` |  |
| `clamp` | `cl` |  |
| `clamplower` | `cll` |  |
| `clampupper` | `clu` |  |
| `colorfeedback` | `cf` |  |
| `mappressure` | `mp` |  |
| `reflection` | `rfl` |  |
| `reflectionaxis` | `ra` |  |
| `outline` | `o` |  |
| `tablet` | `tab` |  |
| `flood` | `fl` |  |
| `floodselect` | `fs` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## artSelectCtx

A artSelectCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `selectop` | `sop` |  |
| `radius` | `r` |  |
| `lowerradius` | `lr` |  |
| `opacity` | `op` |  |
| `stampProfile` | `stP` |  |
| `addselection` | `ads` |  |
| `unselectall` | `ual` |  |
| `selectall` | `sal` |  |
| `outline` | `o` |  |
| `tablet` | `tab` |  |
| `reflection` | `rfl` |  |
| `reflectionaxis` | `ra` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## artSetPaintCtx

A artSetPaintCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `settomodify` | `stm` |  |
| `setcolorfeedback` | `scf` |  |
| `setdisplaycvs` | `dcv` |  |
| `setopertype` | `sot` |  |
| `radius` | `r` |  |
| `lowerradius` | `lr` |  |
| `opacity` | `op` |  |
| `value` | `val` |  |
| `stampProfile` | `stP` |  |
| `accopacity` | `aco` |  |
| `minvalue` | `mnv` |  |
| `maxvalue` | `mxv` |  |
| `clamp` | `cl` |  |
| `clamplower` | `cll` |  |
| `clampupper` | `clu` |  |
| `colorfeedback` | `cf` |  |
| `mappressure` | `mp` |  |
| `reflection` | `rfl` |  |
| `reflectionaxis` | `ra` |  |
| `outline` | `o` |  |
| `tablet` | `tab` |  |
| `flood` | `fl` |  |
| `floodselect` | `fs` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## artSetToolAndSelectAttr

Picks the paint attribute tool and what it paints: artSetToolAndSelectAttr artAttrCtx cluster.cluster1.weights.

Arguments: the tool, then nodeType.node.attribute

## artUserPaintCtx

A artUserPaintCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `toolSetupCmd` | `tsc` |  |
| `toolCleanupCmd` | `tcc` |  |
| `initializeCmd` | `ic` |  |
| `finalizeCmd` | `fc` |  |
| `setValueCommand` | `svc` |  |
| `getValueCommand` | `gvc` |  |
| `setArrayValueCommand` | `sac` |  |
| `getArrayAttrCommand` | `gac` |  |
| `getSurfaceCommand` | `gsc` |  |
| `radius` | `r` |  |
| `lowerradius` | `lr` |  |
| `opacity` | `op` |  |
| `value` | `val` |  |
| `stampProfile` | `stP` |  |
| `accopacity` | `aco` |  |
| `minvalue` | `mnv` |  |
| `maxvalue` | `mxv` |  |
| `clamp` | `cl` |  |
| `clamplower` | `cll` |  |
| `clampupper` | `clu` |  |
| `colorfeedback` | `cf` |  |
| `mappressure` | `mp` |  |
| `reflection` | `rfl` |  |
| `reflectionaxis` | `ra` |  |
| `outline` | `o` |  |
| `tablet` | `tab` |  |
| `flood` | `fl` |  |
| `floodselect` | `fs` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## attachCurve

Join curves in order. With -rpo the first takes the result. Returns the transform.

Arguments: curves (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `replaceOriginal` | `rpo` | The first curve takes the result. |
| `name` | `n` | Name of the new curve. |
| `method` | `m` | Accepted for existing scripts. |
| `blendBias` | `bb` | Accepted for existing scripts. |
| `constructionHistory` | `ch` | Accepted for existing scripts. |

## attachSurface

Join two surfaces along -d, 1 U or 0 V; -m 0 connects, 1 blends at -bb. The first takes the result with -rpo (on). Returns [surface, node].

Arguments: two surfaces (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `method` | `m` | 0 connect, 1 blend (0). |
| `blendBias` | `bb` | Where a blend meets (0.5). |
| `direction` | `d` | 1 U, 0 V (1). |
| `keepMultipleKnots` | `kmk` | Accepted for existing scripts. |
| `blendKnotInsertion` | `bki` | Accepted for existing scripts. |
| `parameter` | `p` | Accepted for existing scripts. |
| `constructionHistory` | `ch` | Keep a history node (on by default). |
| `replaceOriginal` | `rpo` | Change the surface itself (on by default). |
| `name` | `n` | Name of a new surface. |
| `object` | `o` | Accepted for existing scripts. |
| `polygon` | `po` | Accepted for existing scripts. |

## attrColorSliderGrp

attrColorSliderGrp: a control bound to the attribute given by -attribute; it shows the plug, sets it undoably, and follows it.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `rgbValue` | `rgb` |  |
| `hsvValue` | `hsv` |  |
| `showButton` | `sb` |  |
| `attrNavDecision` | `nd` |  |
| `hideButton` | `hb` |  |
| `attribute` | `at` |  |
| `label` | `l` |  |
| `changeCommand` | `cc` |  |
| `extraButton` | `eb` |  |
| `extraButtonCommand` | `ebc` |  |
| `extraButtonIcon` | `ebi` |  |
| `extraLabel` | `el` |  |
| `forceAddMapButton` | `fmb` |  |
| `hideMapButton` | `hmb` |  |
| `adjustableColumn` | `adj` |  |
| `columnWidth` | `cw` |  |
| `columnAlign` | `cal` |  |
| `columnAttach` | `cat` |  |
| `rowAttach` | `rat` |  |
| `vertical` | `vr` |  |
| `precision` | `pre` |  |
| `field` | `f` |  |
| `value` | `v` |  |
| `minValue` | `min` |  |
| `maxValue` | `max` |  |
| `fieldMinValue` | `fmn` |  |
| `fieldMaxValue` | `fmx` |  |
| `sliderStep` | `ss` |  |
| `step` | `s` |  |
| `fieldStep` | `fs` |  |
| `dragCommand` | `dc` |  |
| `numberOfFields` | `nf` |  |
| `value1` | `v1` |  |
| `value2` | `v2` |  |
| `value3` | `v3` |  |
| `value4` | `v4` |  |
| `changeCommand1` | `cc1` |  |
| `changeCommand2` | `cc2` |  |
| `changeCommand3` | `cc3` |  |
| `changeCommand4` | `cc4` |  |
| `dragCommand1` | `dc1` |  |
| `dragCommand2` | `dc2` |  |
| `dragCommand3` | `dc3` |  |
| `dragCommand4` | `dc4` |  |
| `enable1` | `en1` |  |
| `enable2` | `en2` |  |
| `enable3` | `en3` |  |
| `enable4` | `en4` |  |
| `columnWidth1` | `cw1` |  |
| `columnAlign1` | `cl1` |  |
| `columnAttach1` | `ct1` |  |
| `columnOffset1` | `co1` |  |
| `columnWidth2` | `cw2` |  |
| `columnAlign2` | `cl2` |  |
| `columnAttach2` | `ct2` |  |
| `columnOffset2` | `co2` |  |
| `adjustableColumn2` | `ad2` |  |
| `columnWidth3` | `cw3` |  |
| `columnAlign3` | `cl3` |  |
| `columnAttach3` | `ct3` |  |
| `columnOffset3` | `co3` |  |
| `adjustableColumn3` | `ad3` |  |
| `columnWidth4` | `cw4` |  |
| `columnAlign4` | `cl4` |  |
| `columnAttach4` | `ct4` |  |
| `columnOffset4` | `co4` |  |
| `adjustableColumn4` | `ad4` |  |
| `columnWidth5` | `cw5` |  |
| `columnAlign5` | `cl5` |  |
| `columnAttach5` | `ct5` |  |
| `columnOffset5` | `co5` |  |
| `adjustableColumn5` | `ad5` |  |
| `columnWidth6` | `cw6` |  |
| `columnAlign6` | `cl6` |  |
| `columnAttach6` | `ct6` |  |
| `columnOffset6` | `co6` |  |
| `adjustableColumn6` | `ad6` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## attrControlGrp

attrControlGrp: a control bound to the attribute given by -attribute; it shows the plug, sets it undoably, and follows it.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `attribute` | `a` |  |
| `enable` | `en` |  |
| `label` | `l` |  |
| `changeCommand` | `cc` |  |
| `extraButton` | `eb` |  |
| `extraButtonCommand` | `ebc` |  |
| `extraButtonIcon` | `ebi` |  |
| `extraLabel` | `el` |  |
| `forceAddMapButton` | `fmb` |  |
| `hideMapButton` | `hmb` |  |
| `adjustableColumn` | `adj` |  |
| `columnWidth` | `cw` |  |
| `columnAlign` | `cal` |  |
| `columnAttach` | `cat` |  |
| `rowAttach` | `rat` |  |
| `vertical` | `vr` |  |
| `precision` | `pre` |  |
| `field` | `f` |  |
| `value` | `v` |  |
| `minValue` | `min` |  |
| `maxValue` | `max` |  |
| `fieldMinValue` | `fmn` |  |
| `fieldMaxValue` | `fmx` |  |
| `sliderStep` | `ss` |  |
| `step` | `s` |  |
| `fieldStep` | `fs` |  |
| `dragCommand` | `dc` |  |
| `numberOfFields` | `nf` |  |
| `value1` | `v1` |  |
| `value2` | `v2` |  |
| `value3` | `v3` |  |
| `value4` | `v4` |  |
| `changeCommand1` | `cc1` |  |
| `changeCommand2` | `cc2` |  |
| `changeCommand3` | `cc3` |  |
| `changeCommand4` | `cc4` |  |
| `dragCommand1` | `dc1` |  |
| `dragCommand2` | `dc2` |  |
| `dragCommand3` | `dc3` |  |
| `dragCommand4` | `dc4` |  |
| `enable1` | `en1` |  |
| `enable2` | `en2` |  |
| `enable3` | `en3` |  |
| `enable4` | `en4` |  |
| `columnWidth1` | `cw1` |  |
| `columnAlign1` | `cl1` |  |
| `columnAttach1` | `ct1` |  |
| `columnOffset1` | `co1` |  |
| `columnWidth2` | `cw2` |  |
| `columnAlign2` | `cl2` |  |
| `columnAttach2` | `ct2` |  |
| `columnOffset2` | `co2` |  |
| `adjustableColumn2` | `ad2` |  |
| `columnWidth3` | `cw3` |  |
| `columnAlign3` | `cl3` |  |
| `columnAttach3` | `ct3` |  |
| `columnOffset3` | `co3` |  |
| `adjustableColumn3` | `ad3` |  |
| `columnWidth4` | `cw4` |  |
| `columnAlign4` | `cl4` |  |
| `columnAttach4` | `ct4` |  |
| `columnOffset4` | `co4` |  |
| `adjustableColumn4` | `ad4` |  |
| `columnWidth5` | `cw5` |  |
| `columnAlign5` | `cl5` |  |
| `columnAttach5` | `ct5` |  |
| `columnOffset5` | `co5` |  |
| `adjustableColumn5` | `ad5` |  |
| `columnWidth6` | `cw6` |  |
| `columnAlign6` | `cl6` |  |
| `columnAttach6` | `ct6` |  |
| `columnOffset6` | `co6` |  |
| `adjustableColumn6` | `ad6` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## attrEnumOptionMenu

attrEnumOptionMenu: a control bound to the attribute given by -attribute; it shows the plug, sets it undoably, and follows it.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `enumeratedItem` | `ei` |  |
| `value` | `v` |  |
| `select` | `sl` |  |
| `attribute` | `at` |  |
| `label` | `l` |  |
| `changeCommand` | `cc` |  |
| `extraButton` | `eb` |  |
| `extraButtonCommand` | `ebc` |  |
| `extraButtonIcon` | `ebi` |  |
| `extraLabel` | `el` |  |
| `forceAddMapButton` | `fmb` |  |
| `hideMapButton` | `hmb` |  |
| `adjustableColumn` | `adj` |  |
| `columnWidth` | `cw` |  |
| `columnAlign` | `cal` |  |
| `columnAttach` | `cat` |  |
| `rowAttach` | `rat` |  |
| `vertical` | `vr` |  |
| `precision` | `pre` |  |
| `field` | `f` |  |
| `minValue` | `min` |  |
| `maxValue` | `max` |  |
| `fieldMinValue` | `fmn` |  |
| `fieldMaxValue` | `fmx` |  |
| `sliderStep` | `ss` |  |
| `step` | `s` |  |
| `fieldStep` | `fs` |  |
| `dragCommand` | `dc` |  |
| `numberOfFields` | `nf` |  |
| `value1` | `v1` |  |
| `value2` | `v2` |  |
| `value3` | `v3` |  |
| `value4` | `v4` |  |
| `changeCommand1` | `cc1` |  |
| `changeCommand2` | `cc2` |  |
| `changeCommand3` | `cc3` |  |
| `changeCommand4` | `cc4` |  |
| `dragCommand1` | `dc1` |  |
| `dragCommand2` | `dc2` |  |
| `dragCommand3` | `dc3` |  |
| `dragCommand4` | `dc4` |  |
| `enable1` | `en1` |  |
| `enable2` | `en2` |  |
| `enable3` | `en3` |  |
| `enable4` | `en4` |  |
| `columnWidth1` | `cw1` |  |
| `columnAlign1` | `cl1` |  |
| `columnAttach1` | `ct1` |  |
| `columnOffset1` | `co1` |  |
| `columnWidth2` | `cw2` |  |
| `columnAlign2` | `cl2` |  |
| `columnAttach2` | `ct2` |  |
| `columnOffset2` | `co2` |  |
| `adjustableColumn2` | `ad2` |  |
| `columnWidth3` | `cw3` |  |
| `columnAlign3` | `cl3` |  |
| `columnAttach3` | `ct3` |  |
| `columnOffset3` | `co3` |  |
| `adjustableColumn3` | `ad3` |  |
| `columnWidth4` | `cw4` |  |
| `columnAlign4` | `cl4` |  |
| `columnAttach4` | `ct4` |  |
| `columnOffset4` | `co4` |  |
| `adjustableColumn4` | `ad4` |  |
| `columnWidth5` | `cw5` |  |
| `columnAlign5` | `cl5` |  |
| `columnAttach5` | `ct5` |  |
| `columnOffset5` | `co5` |  |
| `adjustableColumn5` | `ad5` |  |
| `columnWidth6` | `cw6` |  |
| `columnAlign6` | `cl6` |  |
| `columnAttach6` | `ct6` |  |
| `columnOffset6` | `co6` |  |
| `adjustableColumn6` | `ad6` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## attrEnumOptionMenuGrp

attrEnumOptionMenuGrp: a control bound to the attribute given by -attribute; it shows the plug, sets it undoably, and follows it.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `enumeratedItem` | `ei` |  |
| `value` | `v` |  |
| `select` | `sl` |  |
| `attribute` | `at` |  |
| `label` | `l` |  |
| `changeCommand` | `cc` |  |
| `extraButton` | `eb` |  |
| `extraButtonCommand` | `ebc` |  |
| `extraButtonIcon` | `ebi` |  |
| `extraLabel` | `el` |  |
| `forceAddMapButton` | `fmb` |  |
| `hideMapButton` | `hmb` |  |
| `adjustableColumn` | `adj` |  |
| `columnWidth` | `cw` |  |
| `columnAlign` | `cal` |  |
| `columnAttach` | `cat` |  |
| `rowAttach` | `rat` |  |
| `vertical` | `vr` |  |
| `precision` | `pre` |  |
| `field` | `f` |  |
| `minValue` | `min` |  |
| `maxValue` | `max` |  |
| `fieldMinValue` | `fmn` |  |
| `fieldMaxValue` | `fmx` |  |
| `sliderStep` | `ss` |  |
| `step` | `s` |  |
| `fieldStep` | `fs` |  |
| `dragCommand` | `dc` |  |
| `numberOfFields` | `nf` |  |
| `value1` | `v1` |  |
| `value2` | `v2` |  |
| `value3` | `v3` |  |
| `value4` | `v4` |  |
| `changeCommand1` | `cc1` |  |
| `changeCommand2` | `cc2` |  |
| `changeCommand3` | `cc3` |  |
| `changeCommand4` | `cc4` |  |
| `dragCommand1` | `dc1` |  |
| `dragCommand2` | `dc2` |  |
| `dragCommand3` | `dc3` |  |
| `dragCommand4` | `dc4` |  |
| `enable1` | `en1` |  |
| `enable2` | `en2` |  |
| `enable3` | `en3` |  |
| `enable4` | `en4` |  |
| `columnWidth1` | `cw1` |  |
| `columnAlign1` | `cl1` |  |
| `columnAttach1` | `ct1` |  |
| `columnOffset1` | `co1` |  |
| `columnWidth2` | `cw2` |  |
| `columnAlign2` | `cl2` |  |
| `columnAttach2` | `ct2` |  |
| `columnOffset2` | `co2` |  |
| `adjustableColumn2` | `ad2` |  |
| `columnWidth3` | `cw3` |  |
| `columnAlign3` | `cl3` |  |
| `columnAttach3` | `ct3` |  |
| `columnOffset3` | `co3` |  |
| `adjustableColumn3` | `ad3` |  |
| `columnWidth4` | `cw4` |  |
| `columnAlign4` | `cl4` |  |
| `columnAttach4` | `ct4` |  |
| `columnOffset4` | `co4` |  |
| `adjustableColumn4` | `ad4` |  |
| `columnWidth5` | `cw5` |  |
| `columnAlign5` | `cl5` |  |
| `columnAttach5` | `ct5` |  |
| `columnOffset5` | `co5` |  |
| `adjustableColumn5` | `ad5` |  |
| `columnWidth6` | `cw6` |  |
| `columnAlign6` | `cl6` |  |
| `columnAttach6` | `ct6` |  |
| `columnOffset6` | `co6` |  |
| `adjustableColumn6` | `ad6` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## attrFieldGrp

attrFieldGrp: a control bound to the attribute given by -attribute; it shows the plug, sets it undoably, and follows it.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `minValue` | `min` |  |
| `maxValue` | `max` |  |
| `step` | `s` |  |
| `numberOfFields` | `nf` |  |
| `attribute` | `at` |  |
| `label` | `l` |  |
| `changeCommand` | `cc` |  |
| `extraButton` | `eb` |  |
| `extraButtonCommand` | `ebc` |  |
| `extraButtonIcon` | `ebi` |  |
| `extraLabel` | `el` |  |
| `forceAddMapButton` | `fmb` |  |
| `hideMapButton` | `hmb` |  |
| `adjustableColumn` | `adj` |  |
| `columnWidth` | `cw` |  |
| `columnAlign` | `cal` |  |
| `columnAttach` | `cat` |  |
| `rowAttach` | `rat` |  |
| `vertical` | `vr` |  |
| `precision` | `pre` |  |
| `field` | `f` |  |
| `value` | `v` |  |
| `fieldMinValue` | `fmn` |  |
| `fieldMaxValue` | `fmx` |  |
| `sliderStep` | `ss` |  |
| `fieldStep` | `fs` |  |
| `dragCommand` | `dc` |  |
| `value1` | `v1` |  |
| `value2` | `v2` |  |
| `value3` | `v3` |  |
| `value4` | `v4` |  |
| `changeCommand1` | `cc1` |  |
| `changeCommand2` | `cc2` |  |
| `changeCommand3` | `cc3` |  |
| `changeCommand4` | `cc4` |  |
| `dragCommand1` | `dc1` |  |
| `dragCommand2` | `dc2` |  |
| `dragCommand3` | `dc3` |  |
| `dragCommand4` | `dc4` |  |
| `enable1` | `en1` |  |
| `enable2` | `en2` |  |
| `enable3` | `en3` |  |
| `enable4` | `en4` |  |
| `columnWidth1` | `cw1` |  |
| `columnAlign1` | `cl1` |  |
| `columnAttach1` | `ct1` |  |
| `columnOffset1` | `co1` |  |
| `columnWidth2` | `cw2` |  |
| `columnAlign2` | `cl2` |  |
| `columnAttach2` | `ct2` |  |
| `columnOffset2` | `co2` |  |
| `adjustableColumn2` | `ad2` |  |
| `columnWidth3` | `cw3` |  |
| `columnAlign3` | `cl3` |  |
| `columnAttach3` | `ct3` |  |
| `columnOffset3` | `co3` |  |
| `adjustableColumn3` | `ad3` |  |
| `columnWidth4` | `cw4` |  |
| `columnAlign4` | `cl4` |  |
| `columnAttach4` | `ct4` |  |
| `columnOffset4` | `co4` |  |
| `adjustableColumn4` | `ad4` |  |
| `columnWidth5` | `cw5` |  |
| `columnAlign5` | `cl5` |  |
| `columnAttach5` | `ct5` |  |
| `columnOffset5` | `co5` |  |
| `adjustableColumn5` | `ad5` |  |
| `columnWidth6` | `cw6` |  |
| `columnAlign6` | `cl6` |  |
| `columnAttach6` | `ct6` |  |
| `columnOffset6` | `co6` |  |
| `adjustableColumn6` | `ad6` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## attrFieldSliderGrp

attrFieldSliderGrp: a control bound to the attribute given by -attribute; it shows the plug, sets it undoably, and follows it.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `minValue` | `min` |  |
| `maxValue` | `max` |  |
| `fieldMinValue` | `fmn` |  |
| `fieldMaxValue` | `fmx` |  |
| `sliderMinValue` | `smn` |  |
| `sliderMaxValue` | `smx` |  |
| `step` | `s` |  |
| `fieldStep` | `fs` |  |
| `sliderStep` | `ss` |  |
| `value` | `v` |  |
| `attribute` | `at` |  |
| `label` | `l` |  |
| `changeCommand` | `cc` |  |
| `extraButton` | `eb` |  |
| `extraButtonCommand` | `ebc` |  |
| `extraButtonIcon` | `ebi` |  |
| `extraLabel` | `el` |  |
| `forceAddMapButton` | `fmb` |  |
| `hideMapButton` | `hmb` |  |
| `adjustableColumn` | `adj` |  |
| `columnWidth` | `cw` |  |
| `columnAlign` | `cal` |  |
| `columnAttach` | `cat` |  |
| `rowAttach` | `rat` |  |
| `vertical` | `vr` |  |
| `precision` | `pre` |  |
| `field` | `f` |  |
| `dragCommand` | `dc` |  |
| `numberOfFields` | `nf` |  |
| `value1` | `v1` |  |
| `value2` | `v2` |  |
| `value3` | `v3` |  |
| `value4` | `v4` |  |
| `changeCommand1` | `cc1` |  |
| `changeCommand2` | `cc2` |  |
| `changeCommand3` | `cc3` |  |
| `changeCommand4` | `cc4` |  |
| `dragCommand1` | `dc1` |  |
| `dragCommand2` | `dc2` |  |
| `dragCommand3` | `dc3` |  |
| `dragCommand4` | `dc4` |  |
| `enable1` | `en1` |  |
| `enable2` | `en2` |  |
| `enable3` | `en3` |  |
| `enable4` | `en4` |  |
| `columnWidth1` | `cw1` |  |
| `columnAlign1` | `cl1` |  |
| `columnAttach1` | `ct1` |  |
| `columnOffset1` | `co1` |  |
| `columnWidth2` | `cw2` |  |
| `columnAlign2` | `cl2` |  |
| `columnAttach2` | `ct2` |  |
| `columnOffset2` | `co2` |  |
| `adjustableColumn2` | `ad2` |  |
| `columnWidth3` | `cw3` |  |
| `columnAlign3` | `cl3` |  |
| `columnAttach3` | `ct3` |  |
| `columnOffset3` | `co3` |  |
| `adjustableColumn3` | `ad3` |  |
| `columnWidth4` | `cw4` |  |
| `columnAlign4` | `cl4` |  |
| `columnAttach4` | `ct4` |  |
| `columnOffset4` | `co4` |  |
| `adjustableColumn4` | `ad4` |  |
| `columnWidth5` | `cw5` |  |
| `columnAlign5` | `cl5` |  |
| `columnAttach5` | `ct5` |  |
| `columnOffset5` | `co5` |  |
| `adjustableColumn5` | `ad5` |  |
| `columnWidth6` | `cw6` |  |
| `columnAlign6` | `cl6` |  |
| `columnAttach6` | `ct6` |  |
| `columnOffset6` | `co6` |  |
| `adjustableColumn6` | `ad6` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## attrNavigationControlGrp

attrNavigationControlGrp: a control bound to the attribute given by -attribute; it shows the plug, sets it undoably, and follows it.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `attrNavDecision` | `nd` |  |
| `connectToExisting` | `ce` |  |
| `createNew` | `cn` |  |
| `defaultTraversal` | `dtv` |  |
| `delete` | `d` |  |
| `disconnect` | `dn` |  |
| `ignoreNotSupported` | `ins` |  |
| `noIgnorableMenu` | `nim` |  |
| `relatedNodes` | `ren` |  |
| `text` | `tx` |  |
| `unignore` | `u` |  |
| `attribute` | `at` |  |
| `label` | `l` |  |
| `changeCommand` | `cc` |  |
| `extraButton` | `eb` |  |
| `extraButtonCommand` | `ebc` |  |
| `extraButtonIcon` | `ebi` |  |
| `extraLabel` | `el` |  |
| `forceAddMapButton` | `fmb` |  |
| `hideMapButton` | `hmb` |  |
| `adjustableColumn` | `adj` |  |
| `columnWidth` | `cw` |  |
| `columnAlign` | `cal` |  |
| `columnAttach` | `cat` |  |
| `rowAttach` | `rat` |  |
| `vertical` | `vr` |  |
| `precision` | `pre` |  |
| `field` | `f` |  |
| `value` | `v` |  |
| `minValue` | `min` |  |
| `maxValue` | `max` |  |
| `fieldMinValue` | `fmn` |  |
| `fieldMaxValue` | `fmx` |  |
| `sliderStep` | `ss` |  |
| `step` | `s` |  |
| `fieldStep` | `fs` |  |
| `dragCommand` | `dc` |  |
| `numberOfFields` | `nf` |  |
| `value1` | `v1` |  |
| `value2` | `v2` |  |
| `value3` | `v3` |  |
| `value4` | `v4` |  |
| `changeCommand1` | `cc1` |  |
| `changeCommand2` | `cc2` |  |
| `changeCommand3` | `cc3` |  |
| `changeCommand4` | `cc4` |  |
| `dragCommand1` | `dc1` |  |
| `dragCommand2` | `dc2` |  |
| `dragCommand3` | `dc3` |  |
| `dragCommand4` | `dc4` |  |
| `enable1` | `en1` |  |
| `enable2` | `en2` |  |
| `enable3` | `en3` |  |
| `enable4` | `en4` |  |
| `columnWidth1` | `cw1` |  |
| `columnAlign1` | `cl1` |  |
| `columnAttach1` | `ct1` |  |
| `columnOffset1` | `co1` |  |
| `columnWidth2` | `cw2` |  |
| `columnAlign2` | `cl2` |  |
| `columnAttach2` | `ct2` |  |
| `columnOffset2` | `co2` |  |
| `adjustableColumn2` | `ad2` |  |
| `columnWidth3` | `cw3` |  |
| `columnAlign3` | `cl3` |  |
| `columnAttach3` | `ct3` |  |
| `columnOffset3` | `co3` |  |
| `adjustableColumn3` | `ad3` |  |
| `columnWidth4` | `cw4` |  |
| `columnAlign4` | `cl4` |  |
| `columnAttach4` | `ct4` |  |
| `columnOffset4` | `co4` |  |
| `adjustableColumn4` | `ad4` |  |
| `columnWidth5` | `cw5` |  |
| `columnAlign5` | `cl5` |  |
| `columnAttach5` | `ct5` |  |
| `columnOffset5` | `co5` |  |
| `adjustableColumn5` | `ad5` |  |
| `columnWidth6` | `cw6` |  |
| `columnAlign6` | `cl6` |  |
| `columnAttach6` | `ct6` |  |
| `columnOffset6` | `co6` |  |
| `adjustableColumn6` | `ad6` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## attrPaintStamp

One dab of the paint attribute brush at a world point, from the tool's settings: for scripts and tests. -inv paints the inverse, as Ctrl does in the tool; -sm smooths, as Shift does. Returns how many points it touched.

Arguments: a deformed mesh

| Flag | Short | Description |
| --- | --- | --- |
| `position` | `p` | World centre of the dab. |
| `invert` | `inv` | Paint the inverse. |
| `smooth` | `sm` | Smooth instead. |

## attributeInfo

A node's attributes filtered: -b boolean, -e enumerated, -h hidden, -w writable, -m multi, -lf leaf, -inh inherited; -t a type in place of a node; -s short names.

Arguments: object

| Flag | Short | Description |
| --- | --- | --- |
| `allAttributes` | `all` | All (what no filter gives). |
| `bool` | `b` | Booleans. |
| `enumerated` | `e` | Enums. |
| `hidden` | `h` | Hidden ones. |
| `inherited` | `inh` | Inherited from the base type. |
| `internal` | `i` | Internal ones: none here. |
| `leaf` | `lf` | Not compounds. |
| `logicalAnd` | `la` | All filters must hold, not any. |
| `multi` | `m` | Arrays. |
| `short` | `s` | Short names. |
| `type` | `t` | A type rather than a node. |
| `userInterface` | `ui` | Accepted for existing scripts. |
| `writable` | `w` | Writable ones. |

## attributeName

An attribute's name: -n nice (the default), -l long, -s short, -lf the leaf.

Arguments: plug

| Flag | Short | Description |
| --- | --- | --- |
| `nice` | `n` | The name editors show. |
| `long` | `l` | The long name. |
| `short` | `s` | The short name. |
| `leaf` | `lf` | Without its parent's name. |

## attributeQuery

Facts about an attribute: attributeQuery -n node -k attr, or -type nodeType.

Arguments: attribute

| Flag | Short | Description |
| --- | --- | --- |
| `exists` | `ex` | Whether it exists. |
| `keyable` | `k` | Keyable. |
| `channelBox` | `ch` | Shown in the channel box. |
| `hidden` | `h` | Hidden. |
| `writable` | `w` | Takes values. |
| `readable` | `r` | Can be read. |
| `connectable` | `c` | Can connect. |
| `storable` | `s` | Saved. |
| `multi` | `m` | A multi attribute. |
| `indexMatters` | `im` | A multi's indices matter. |
| `usedAsColor` | `uac` | A colour. |
| `usedAsFilename` | `uaf` | A file name. |
| `attributeType` | `at` | Its type. |
| `typeExact` | `tex` | Its exact type. |
| `longName` | `ln` | The long name. |
| `shortName` | `sn` | The short name. |
| `niceName` | `nn` | The nice name. |
| `listEnum` | `le` | An enum's labels. |
| `listParent` | `lp` | The compound it is in. |
| `listChildren` | `lc` | A compound's children. |
| `listSiblings` | `ls` | The other children. |
| `listDefault` | `ld` | The default. |
| `minExists` | `mne` | Has a hard minimum. |
| `maxExists` | `mxe` | Has a hard maximum. |
| `softMinExists` | `sme` | Has a soft minimum. |
| `softMaxExists` | `sxe` | Has a soft maximum. |
| `minimum` | `min` | The hard minimum. |
| `maximum` | `max` | The hard maximum. |
| `mn` |  | The hard minimum. |
| `mx` |  | The hard maximum. |
| `softMin` | `smn` | The soft minimum. |
| `softMax` | `smx` | The soft maximum. |
| `range` | `rg` | The hard range. |
| `softRange` | `sr` | The soft range. |
| `node` | `n` | The node. |
| `type` | `typ` | A node type. |

## autoBevel

Add an autoBevel modifier to the top of each mesh's stack: bevels fold edges found live on its input mesh. Returns the modifier names.

Arguments: meshes

| Flag | Short | Description |
| --- | --- | --- |
| `minAngle` |  | Smallest fold, degrees between the face normals. |
| `maxAngle` |  | Largest fold, degrees. |
| `side` |  | both, outside (convex) or inside (concave). |
| `thin` |  | Skip thickness edges narrower than this, cm. |
| `minLength` |  | Skip edges shorter than this, cm. |
| `completeCorners` |  | Add the quad loops joining folds that meet at an angle. |
| `mode` |  | auto, autoOverrides, manual, angle or weight. |
| `weightThreshold` |  | weight mode threshold. |
| `offset` |  | Width, cm. |
| `segments` |  | Edges across each bevel, 1 to 32. |
| `profile` |  | 0.5 round, 0.25 straight. |
| `clamp` |  | Clamp the width. |
| `smoothingAngle` |  | Hard-edge angle for the new edges. |

## autoKeyframe

Auto key: changing an animated channel keys it at the current frame. Returns whether it is on.

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Return the value of the flags named instead of setting them. |
| `edit` | `e` | Change the flags named. |
| `state` | `st` | On or off. |

## aztekChangeLightType

The Attribute Editor's Type menu: the light's shape becomes another type under the same transform, with the same name, keeping what the two types share.

Arguments: the light, then the type

## aztekEnvironment

The environment variables Aztek knows of: -q lists each with whether it is read and its value in force; -value name gives one, Aztek's own spelling first; -list name gives a list of folders; -icon name finds an icon on XBMLANGPATH; -shelfFiles gives the shelf files on MAYA_SHELF_PATH.

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Every variable: name, group, whether it is read, a note, and its value. |
| `value` | `v` | The value in force of this variable. |
| `list` | `l` | The folders of this variable, in order. |
| `icon` | `i` | The file of this icon, or an empty string. |
| `shelfFiles` | `sf` | The shelf_*.mel files on the shelf path. |

## aztekIlluminatesByDefault

The Attribute Editor's Illuminates by Default: whether a light is in defaultLightSet; -set puts it in or out.

Arguments: the light

| Flag | Short | Description |
| --- | --- | --- |
| `set` | `set` | In or out. |

## aztekPluginCallback

Runs a compiled plug-in's message callback by its id; scriptJobs the kit makes call it.

Arguments: callback id, then what it is told

## aztekPlugins

Every plug-in file in the folders of the plug-in path, for the Plug-in Manager: its name, file, folder, kind (aztek, python, or foreign for one compiled for another application), and whether it is loaded and loads at start.

## aztekPython

Run Python from inside a command, as schema translators do.

Arguments: code

## aztekUsdBeforeSave

What a scene save runs: proxies take in their session and unsaved layers. Returns the unsaved layers.

## aztekUsdFile

What file runs for a USD type or extension.

| Flag | Short | Description |
| --- | --- | --- |
| `file` |  | See file. |
| `options` |  | See file. |
| `import` |  | See file. |
| `open` |  | See file. |
| `exportAll` |  | See file. |
| `exportSelected` |  | See file. |

## aztekUserFolder

Where scripts, plug-ins, modules and preferences kept between versions are looked for: -mode aztek (Aztek's own folder), link (an existing folder of the reference's, used in place and never written to) or custom, with -folder and -version; -copyFrom folder copies one into Aztek's own. -q describes what is in force.

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Describe the choice, the folders on each path and the modules found. |
| `mode` | `m` | aztek, link or custom. |
| `folder` | `f` | The folder for link and custom. |
| `version` | `v` | The version's subfolder to use; empty takes the newest. |
| `userSetup` | `us` | Whether userSetup scripts run at start. |
| `copyFrom` | `cf` | Copy this folder into Aztek's own and use that. |
| `listVersions` | `lv` | The versions a folder holds, newest first. |

## bakeClip

Bake clips into one, frame by frame: -ci which (all by default), -bl a blend's two, -k keeps the originals disabled, -n its name. Returns the new clip.

Arguments: character

| Flag | Short | Description |
| --- | --- | --- |
| `clipIndex` | `ci` | A clip's place on the scheduler. |
| `blend` | `bl` | A blend's two clips. |
| `keepOriginals` | `k` | Keep the clips, disabled. |
| `name` | `n` | The new clip's name. |

## bakeModifierStack

Bake meshes to their current result, deleting the generator and modifiers feeding them. Curves likewise keep what their history makes and lose the history; other objects are skipped.

Arguments: objects

## bakePartialHistory

Delete by Type > Non-Deformer History: bakes what feeds the first deformer and keeps the deformers. A mesh with no deformers loses its history.

Arguments: objects

| Flag | Short | Description |
| --- | --- | --- |
| `prePostDeformers` | `ppt` | Before and after the deformers. |
| `preDeformers` | `pre` | Before the deformers. |
| `query` | `q` | Query. |

## bakePivot

Make the pivot the objects' own origin and axes, as Bake Pivot does: with -position the translate goes to the pivot and the pivot offsets go to zero; with -orientation the rotate takes up the pivot's orientation. The points are countered, so nothing moves in the world, and children stay where they are. Both are on unless one is given. Instances are made unique.

Arguments: objects

| Flag | Short | Description |
| --- | --- | --- |
| `position` | `pos` | Bake the pivot's position. |
| `orientation` | `ori` | Bake the pivot's orientation. |

## bakeResults

Key channels at every -sb frames over -t with the values they evaluate to, replacing what drove them. -hi below includes the hierarchy, -at the channels; -pok keeps keys outside the range. Returns how many channels.

Arguments: objects or plugs (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `attribute` | `at` | Channel name(s). |
| `time` | `t` | The range to bake: "1:100" (the playback range by default). |
| `sampleBy` | `sb` | Frames between keys (1). |
| `hierarchy` | `hi` | none or below. |
| `preserveOutsideKeys` | `pok` | Keep keys outside the baked range. |
| `simulation` | `sm` | Accepted for existing scripts (always evaluated in time order). |
| `disableImplicitControl` | `dic` | Accepted for existing scripts. |
| `sparseAnimCurveBake` | `sac` | Accepted for existing scripts. |
| `destinationLayer` | `dl` | The layer that takes the result. |
| `bakeOnOverrideLayer` | `bol` | The result goes on a new override layer on top. |
| `removeBakedAttributeFromLayer` | `ral` | Baked channels leave their layers, rather than those layers being muted. |
| `removeBakedAnimFromLayer` | `rba` | Baked animation leaves its layers, rather than those layers being muted. |
| `resolveWithoutLayer` | `rwl` | Layers merged in the bake: only these give way to the result. |
| `minimizeRotation` | `mr` | Accepted for existing scripts. |
| `controlPoints` | `cp` | Accepted for existing scripts. |
| `shape` | `s` | Accepted for existing scripts. |

## bakeSimulation

Key channels with what they evaluate to, stepping through time: bakeResults -simulation.

Arguments: objects or plugs (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `attribute` | `at` | Channel name(s). |
| `time` | `t` | The range to bake: "1:100" (the playback range by default). |
| `sampleBy` | `sb` | Frames between keys (1). |
| `hierarchy` | `hi` | none or below. |
| `preserveOutsideKeys` | `pok` | Keep keys outside the baked range. |
| `simulation` | `sm` | Accepted for existing scripts (always evaluated in time order). |
| `disableImplicitControl` | `dic` | Accepted for existing scripts. |
| `sparseAnimCurveBake` | `sac` | Accepted for existing scripts. |
| `destinationLayer` | `dl` | The layer that takes the result. |
| `bakeOnOverrideLayer` | `bol` | The result goes on a new override layer on top. |
| `removeBakedAttributeFromLayer` | `ral` | Baked channels leave their layers, rather than those layers being muted. |
| `removeBakedAnimFromLayer` | `rba` | Baked animation leaves its layers, rather than those layers being muted. |
| `resolveWithoutLayer` | `rwl` | Layers merged in the bake: only these give way to the result. |
| `minimizeRotation` | `mr` | Accepted for existing scripts. |
| `controlPoints` | `cp` | Accepted for existing scripts. |
| `shape` | `s` | Accepted for existing scripts. |

## baseTemplate

Base node templates: -load -fileName, -unload, -exists. Their contents are not read here: the name is kept.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `exists` | `ex` |  |
| `fileName` | `fn` |  |
| `force` | `f` |  |
| `load` | `l` |  |
| `matchFile` | `mf` |  |
| `silent` | `si` |  |
| `unload` | `u` |  |
| `viewList` | `vl` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## bevel

A bevelled wall round a curve: -w width, -d depth, -ed wall depth, -bst 1 straight or 2 arc. Returns [surface, node].

Arguments: a curve

| Flag | Short | Description |
| --- | --- | --- |
| `width` | `w` | How far out (0.1). |
| `depth` | `d` | How deep the bevel (0.1). |
| `extrudeDepth` | `ed` | How deep the wall (1). |
| `bevelShapeType` | `bst` | 1 straight, 2 arc. |
| `cornerType` | `ct` | Accepted for existing scripts. |
| `constructionHistory` | `ch` | Keep the node (on by default). |
| `name` | `n` | The new object's name. |
| `object` | `o` | Accepted for existing scripts. |
| `range` | `rn` | Accepted for existing scripts. |
| `caching` | `cch` | Accepted for existing scripts. |
| `nodeState` | `nds` | Accepted for existing scripts. |
| `tolerance` | `tol` | Accepted for existing scripts. |
| `polygon` | `po` | 0 NURBS, 1 polygons. |

## bevelApplyEdges

Store the selected edges as an autoBevel modifier's edge set (as overrides of its rule) and end Edit Edges. Returns {added, removed}.

Arguments: modifier or mesh

## bevelCancelEdges

End an autoBevel modifier's Edit Edges without changing its edge set.

Arguments: modifier or mesh

## bevelClearOverrides

Drop an autoBevel modifier's hand-picked edges, going back to its rule.

Arguments: modifier or mesh

## bevelEdgeInfo

The edges an autoBevel modifier picks on its input mesh: {edges, count, lost, editing}.

Arguments: modifier or mesh

## bevelEditEdges

Show an autoBevel modifier's input mesh (it and later modifiers pass through) with its edges selected, so they can be changed with the normal selection tools. Finish with bevelApplyEdges or bevelCancelEdges.

Arguments: modifier or mesh

## bevelMeasureThin

Set an autoBevel modifier's thin to 1.5 times the longest selected edge (pick a thickness edge). Returns it.

Arguments: modifier or mesh

## bevelPlus

A bevel with caps: -w, -d, -ed, -ns sides of the round, -cap 1 none, 2 front, 3 back, 4 both, -po 1 polygons (the default here). Returns [surface or group, node].

Arguments: a curve

| Flag | Short | Description |
| --- | --- | --- |
| `width` | `w` | How far out (0.1). |
| `depth` | `d` | How deep the bevel (0.1). |
| `extrudeDepth` | `ed` | How deep the wall (1). |
| `numberOfSides` | `ns` | Sides of the round. |
| `capSides` | `cap` | 1 none, 2 front, 3 back, 4 both. |
| `innerStyle` | `is` | Accepted for existing scripts. |
| `outerStyle` | `os` | Accepted for existing scripts. |
| `constructionHistory` | `ch` | Keep the node (on by default). |
| `name` | `n` | The new object's name. |
| `object` | `o` | Accepted for existing scripts. |
| `range` | `rn` | Accepted for existing scripts. |
| `caching` | `cch` | Accepted for existing scripts. |
| `nodeState` | `nds` | Accepted for existing scripts. |
| `tolerance` | `tol` | Accepted for existing scripts. |
| `polygon` | `po` | 0 NURBS, 1 polygons. |

## bezierAnchorPreset

Set selected anchors' tangents: -p 0 Bezier (smooth and even, along the line between the neighbours), 1 Bezier Corner (each a third of the way to its neighbour), 2 Corner (drawn in to the anchor). Returns how many anchors. The exact tangent lengths are this command's own: the reference does not document them.

| Flag | Short | Description |
| --- | --- | --- |
| `preset` | `p` | 0 Bezier, 1 Bezier Corner, 2 Corner. |

## bezierAnchorState

Set selected anchors or their tangents: -sm true lines the tangents up, -ev true makes them the same length. Returns how many anchors. Less than the reference: there is no lasting state, so false changes nothing and later drags do not keep the tangents smooth or even.

| Flag | Short | Description |
| --- | --- | --- |
| `even` | `ev` | Even weighting. |
| `smooth` | `sm` | Smooth rather than broken. |

## bezierCurveToNurbs

Make a Bezier curve a plain NURBS curve, keeping its CVs and knots. Returns [object, node].

Arguments: curves (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `constructionHistory` | `ch` | Keep a history node (on by default). |
| `replaceOriginal` | `rpo` | Change the curve itself (on by default). |
| `name` | `n` | Name of a new curve. |
| `object` | `o` | Make an object (accepted for existing scripts). |

## bezierInfo

Questions about Bezier CVs: -afc a CV's anchor, -cfa an anchor's CV; -ias, -its, -oas, -ots about what is selected, as 1 or 0.

| Flag | Short | Description |
| --- | --- | --- |
| `anchorFromCV` | `afc` | A CV index. |
| `cvFromAnchor` | `cfa` | An anchor index. |
| `isAnchorSelected` | `ias` | Is an anchor selected. |
| `isTangentSelected` | `its` | Is a tangent selected. |
| `onlyAnchorsSelected` | `oas` | Are only anchors selected. |
| `onlyTangentsSelected` | `ots` | Are only tangents selected. |

## bindSkin

Rigid bind: each point follows one joint, a skinCluster of 1.0 weights. -tsb only the bones named, -ub unbinds, -ubk keeps history. Returns 1.

Arguments: skeleton and geometry

| Flag | Short | Description |
| --- | --- | --- |
| `byClosestPoint` | `bcp` | Accepted: closest point is how it binds. |
| `byPartition` | `bp` | Accepted for existing scripts. |
| `colorJoints` | `cj` | Accepted for existing scripts. |
| `delete` | `del` | Remove the skin. |
| `doNotDescend` | `dnd` | Only the joints named. |
| `enable` | `en` | Accepted for existing scripts. |
| `name` | `n` | The skinCluster's name. |
| `partition` | `p` | Accepted for existing scripts. |
| `toAll` | `ta` | The whole skeleton (the default). |
| `toSelectedBones` | `tsb` | Only the joints named. |
| `unbind` | `ub` | Remove the skin. |
| `unbindKeepHistory` | `ubk` | Stop skinning, keep the node. |

## birail

A profile along two rails, its ends staying on them: profile, rail, rail. Returns [birailSurface1, singleProfileBirailSurface1].

Arguments: a profile and two rails (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `tangentContinuityProfile1` | `tp1` | Accepted for existing scripts. |
| `transformMode` | `tm` | Accepted for existing scripts. |
| `constructionHistory` | `ch` | Keep the node (on by default). |
| `polygon` | `po` | 0 NURBS, 1 polygons. |
| `range` | `rn` | Accepted for existing scripts. |
| `name` | `n` | Name of the new object. |
| `object` | `o` | Accepted for existing scripts. |

## blend2

A surface between two rails, curves or surface edges (surface.u[x]): -ls, -le, -rs, -re the ranges, -la, -ra on closed rails, -aa lines closed rails up, -fln, -frn flip the tangents off an edge, -rvl, -rvr with -an false. Returns [surface, node]. Less than the reference: one curve per rail, so -cfr must be 1; the result meets its rails exactly and -pt, -tt and -mk are only held.

Arguments: two rails (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `constructionHistory` | `ch` | Keep the node (on by default). |
| `name` | `n` | The new object's name. |
| `object` | `o` | Accepted for existing scripts. |
| `range` | `rn` | Accepted for existing scripts. |
| `caching` | `cch` | Accepted for existing scripts. |
| `nodeState` | `nds` | Accepted for existing scripts. |
| `tolerance` | `tol` | Accepted for existing scripts. |
| `polygon` | `po` | 0 NURBS, 1 polygons. |
| `autoAnchor` | `aa` | On closed rails, line them up. |
| `autoNormal` | `an` | Take the directions from the geometry. |
| `crvsInFirstRail` | `cfr` | Curves in the first rail: only 1 here. |
| `flipLeftNormal` | `fln` | Flip the tangent off the left. |
| `flipRightNormal` | `frn` | Flip the tangent off the right. |
| `leftAnchor` | `la` | Where a closed left rail starts. |
| `leftEnd` | `le` | Where it ends on the left (1). |
| `leftStart` | `ls` | Where it starts on the left. |
| `multipleKnots` | `mk` | Held. |
| `positionTolerance` | `pt` | Held. |
| `reverseLeft` | `rvl` | With -an false: reverse off the left. |
| `reverseRight` | `rvr` | With -an false: reverse off the right. |
| `rightAnchor` | `ra` | Where a closed right rail starts. |
| `rightEnd` | `re` | Where it ends on the right (1). |
| `rightStart` | `rs` | Where it starts on the right. |
| `tangentTolerance` | `tt` | Held. |

## blendShape

Blend shapes: blendShape target... base. Each target gets an element of .weight under an alias named after it: setAttr blendShape1.pSphere2 0.5. -t base index target weight adds targets, with -ib as in-betweens at that weight. -e -rm -t removes, -e -rtd index item clears deltas. Query -w, -wc, -t, -g, -en, -o, -tc.

Arguments: targets, then the base; the blendShape with -e and -q

| Flag | Short | Description |
| --- | --- | --- |
| `name` | `n` | Name of the deformer. |
| `before` | `bf` | Go just above the deformer nearest the mesh. |
| `after` | `af` | Go nearest the mesh, after the other deformers (the default). |
| `frontOfChain` | `foc` | Go above every other deformer. |
| `envelope` | `en` | How much of the deformation applies (1). |
| `edit` | `e` | Edit mode. |
| `query` | `q` | Query mode. |
| `weight` | `w` | index weight, repeatable. Query: all the weights. |
| `target` | `t` | base index target weight, repeatable. Query: the target shapes. |
| `inBetween` | `ib` | -t adds an in-between at its weight rather than replacing the target. |
| `inBetweenType` | `ibt` | absolute (the default), or relative to the full target. |
| `topologyCheck` | `tc` | Refuse targets with a different number of points (on). |
| `origin` | `o` | local (the default) or world. |
| `parallel` | `par` | Work alongside the deformers already there: measured from the original, added to their result. |
| `remove` | `rm` | With -e -t: remove the target. With -e -g: remove the geometry. |
| `removeTarget` | `rmt` | With -e: a target's name, to remove it. |
| `resetTargetDelta` | `rtd` | With -e: target index and item (0 for all), to clear their deltas; a name removes the target. |
| `geometry` | `g` | With -e: geometry to add or, with -rm, remove. Query: the deformed shapes. |
| `geometryIndices` | `gi` | Query: the indices of the deformed shapes. |
| `weightCount` | `wc` | Query: how many targets. |
| `foreignObject` | `fg` | Targets need not match the base: points are taken by number as far as they go. |
| `ignoreSelected` | `ie` | Use only the objects named, not the selection. |

## blendShapeEditor

Script UI: blendShapeEditor, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `targetControlList` | `tcl` |  |
| `targetList` | `tl` |  |
| `verticalSliders` | `vs` |  |
| `control` | `ctl` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `filter` | `f` |  |
| `forceMainConnection` | `fmc` |  |
| `highlightConnection` | `hlc` |  |
| `lockMainConnection` | `lck` |  |
| `mainListConnection` | `mlc` |  |
| `panel` | `pnl` |  |
| `selectionConnection` | `slc` |  |
| `stateString` | `sts` |  |
| `unParent` | `up` |  |
| `unlockMainConnection` | `ulk` |  |
| `updateMainConnection` | `upd` |  |
| `useTemplate` | `ut` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## blendShapePanel

A panel holding an editor of its kind: the editor has the panel's name.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `blendShapeEditor` | `be` |  |
| `label` | `l` |  |
| `parent` | `p` |  |
| `control` | `ctl` |  |
| `copy` | `cp` |  |
| `createString` | `cs` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `editString` | `es` |  |
| `exists` | `ex` |  |
| `init` | `in` |  |
| `isUnique` | `iu` |  |
| `menuBarVisible` | `mbv` |  |
| `needsInit` | `ni` |  |
| `popupMenuProcedure` | `pmp` |  |
| `replacePanel` | `rp` |  |
| `tearOff` | `to` |  |
| `tearOffCopy` | `toc` |  |
| `unParent` | `up` |  |
| `useTemplate` | `ut` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## blendTwoAttr

Blend attributes between two inputs with a blendTwoAttr node: -at on what's named or selected (every keyable attribute without it), -at0 and -at1 what feeds the inputs (what drove the attribute by default), -bl a blender attribute, -t the range to key the blend over, -d the driver index, -n a name. Returns the nodes. -s and -cp are accepted: shapes below transforms are not walked.

Arguments: objects or plugs (the selection when none); the node with -q and -e

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `attribute` | `at` | The attributes. |
| `attribute0` | `at0` | What feeds the first input. |
| `attribute1` | `at1` | What feeds the second input. |
| `blender` | `bl` | The attribute driving the blend. |
| `controlPoints` | `cp` | Accepted for existing scripts; has no effect. |
| `driver` | `d` | The driver index, 0 or 1. |
| `name` | `n` | The node's name. |
| `shape` | `s` | Accepted for existing scripts; has no effect. |
| `time` | `t` | The range to blend over; a single time is an abrupt change. |

## boneLattice

A flexor's node on a lattice along a bone: the joint's bend bulges the lattice's points. Returns the node. Less than the reference: it does not move the lattice's ends rigidly (flexor parents the lattice to the bone).

Arguments: the lattice (the selection when none); the node with -q and -e

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `name` | `n` | Name of the node. |
| `joint` | `j` | The joint whose bend drives the bulge. |
| `geometry` | `g` | With -q: what the lattice deforms. |
| `lengthIn` | `li` | Moves the points on the upper half along the bone. |
| `lengthOut` | `lo` | Moves the points on the lower half along the bone. |
| `widthLeft` | `wl` | Bulge on the left of the bend. |
| `widthRight` | `wr` | Bulge on the right of the bend. |
| `bicep` | `bi` | Bulge on the inside of the bend. |
| `tricep` | `tr` | Bulge on the outside of the bend. |
| `transform` | `t` | The transform the bend is measured against: the joint's parent by default. |

## boundary

A surface inside three or four curves, in order round the edge. Returns [boundarySurface1, boundary1].

Arguments: three or four curves (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `order` | `or` | Accepted for existing scripts. |
| `endPoint` | `ep` | Accepted for existing scripts. |
| `endPointTolerance` | `ept` | Accepted for existing scripts. |
| `constructionHistory` | `ch` | Keep the node (on by default). |
| `polygon` | `po` | 0 NURBS, 1 polygons. |
| `range` | `rn` | Accepted for existing scripts. |
| `name` | `n` | Name of the new object. |
| `object` | `o` | Accepted for existing scripts. |

## boxDollyCtx

A boxDollyCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `alternateContext` | `ac` |  |
| `toolName` | `tn` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## boxZoomCtx

A boxZoomCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `zoomScale` | `zs` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## bufferCurve

The Graph Editor's buffer: keeps a curve's keys (-ov, on by default) to swap back with -sw. -ex (or -q) says whether a curve has one.

Arguments: objects, plugs or curves (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `attribute` | `at` | Channel name(s). |
| `time` | `t` | A frame or range "10:20". |
| `float` | `f` | A range of driver values on driven key curves, as -t is for time. |
| `index` | `in` | Key index(es) on each curve, or a range "0:2". |
| `includeUpperBound` | `iub` | Whether a range's end counts (on by default). |
| `hierarchy` | `hi` | none, below, above or both. |
| `controlPoints` | `cp` | Accepted for existing scripts: control points have no channels of their own here. |
| `shape` | `s` | Include the channels of the objects' shapes. |
| `animation` | `an` | objects (the default), keys or keysOrObjects: keys means the selected keys. |
| `overwrite` | `ov` | Replace what the buffer holds (on by default). |
| `swap` | `sw` | Swap the curve and its buffer. |
| `exists` | `ex` | Whether a curve has a buffer. |
| `selected` | `sl` | Curves of the keys selectKey selected. |
| `query` | `q` | With -ex. |

## button

Script UI: button, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `label` | `l` |  |
| `align` | `al` |  |
| `command` | `c` |  |
| `recomputeSize` | `rs` |  |
| `actOnPress` | `aop` |  |
| `actionIsSubstitute` | `ais` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## camera

Create a camera, or query and edit one: -fl focal length, -hfa -vfa film, -ncp -fcp clips, -o orthographic, -ow width, -coi, -p position, -rot rotation, -wci world centre of interest. Returns [camera1, cameraShape1].

Arguments: a camera, with -q and -e

| Flag | Short | Description |
| --- | --- | --- |
| `focalLength` | `fl` | Focal length, mm. |
| `horizontalFilmAperture` | `hfa` | Film width, inches. |
| `verticalFilmAperture` | `vfa` | Film height, inches. |
| `nearClipPlane` | `ncp` | Near clip. |
| `farClipPlane` | `fcp` | Far clip. |
| `orthographic` | `o` | No perspective. |
| `startupCamera` | `sc` | Query or edit: a startup camera, which can't be deleted. |
| `orthographicWidth` | `ow` | View width when orthographic. |
| `centerOfInterest` | `coi` | Distance to what it tumbles about. |
| `lensSqueezeRatio` | `lsr` | Lens squeeze. |
| `position` | `p` | World position. |
| `rotation` | `rot` | Rotation, degrees. |
| `worldCenterOfInterest` | `wci` | The point it looks at. |
| `worldUp` | `wup` | Accepted for existing scripts. |
| `name` | `n` | Name. |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |

## canvas

Script UI: canvas, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `rgbValue` | `rgb` |  |
| `hsvValue` | `hsv` |  |
| `pressCommand` | `pc` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## centerPivot

Move pivots to the centre of each object's bounding box (mesh points, curve points and handles, and everything parented under it, so a group centres on its children) without moving the object.

Arguments: objects

## character

Create a character set from channels or objects (-n names it, -em for an empty one), add to one (-add), remove (-rm), clear (-cl). -q lists its channels, -q -no its nodes, -q -at its attribute names; -im asks whether channels are in it. Returns its name.

Arguments: channels or objects (the selection when none); the character when querying

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Return the value of the flags named instead of setting them. |
| `edit` | `e` | Change the flags named. |
| `name` | `n` | The new character's name. |
| `addElement` | `add` | The character to add to. |
| `forceElement` | `fe` | The character to move them to, from any other. |
| `remove` | `rm` | The character to remove from. |
| `clear` | `cl` | The character to empty. |
| `isMember` | `im` | The character to test membership of. |
| `empty` | `em` | Create it empty. |
| `nodesOnly` | `no` | Query: the nodes rather than the channels. |
| `attributes` | `at` | Query: the channels' attribute names. |
| `library` | `lib` | Query: its clip library. |
| `scheduler` | `sc` | Query: its clip scheduler. |
| `excludeTranslate` | `et` | Leave out translate channels. |
| `excludeRotate` | `er` | Leave out rotate channels. |
| `excludeScale` | `es` | Leave out scale channels. |
| `excludeVisibility` | `ev` | Leave out visibility. |
| `excludeDynamic` | `ed` | Accepted for existing scripts: objects give their keyable channels. |

## characterMap

Pair two characters' attributes: name them for a map matched by attribute name, -pm for the pairs alone, -ma and -ua with -e to change it. Returns the node; -q the pairs.

Arguments: two characters, or a map

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `mapping` | `m` | The node's name. |
| `proposedMapping` | `pm` | The pairs it would make. |
| `mapAttr` | `ma` | Two plugs to pair. |
| `unmapAttr` | `ua` | Two plugs to part. |
| `mapMethod` | `mm` | Accepted: by name is what it does. |
| `mapNode` | `mn` | Accepted for existing scripts. |
| `unmapNode` | `un` | Accepted for existing scripts. |

## characterize

Make a character from a skeleton: the joints named or selected go to slots by their names, -sk "joint slot, joint slot" sets others, -nm names it. Returns the character (a HIKCharacterNode). Less than the reference: -type label reads names too; the pivot, effector and stance flags fail, since the full body solver is not here; -pos and -aab are only held.

Arguments: the skeleton's root (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `name` | `nm` | The character's name. |
| `posture` | `pos` | biped or quadruped. |
| `sourceSkeleton` | `sk` | Joints and the slots they fill. |
| `type` | `typ` | label or name: names are read either way. |
| `autoActivateBodyPart` | `aab` | Held for scripts. |
| `pinHandFeet` | `phf` | Held for scripts. |
| `activatePivot` | `apv` | Not available: fails. |
| `addAuxEffector` | `aae` | Not available: fails. |
| `addFloorContactPlane` | `afp` | Not available: fails. |
| `addMissingEffectors` | `ame` | Not available: fails. |
| `changePivotPlacement` | `cpp` | Not available: fails. |
| `placeNewPivot` | `pnp` | Not available: fails. |
| `effectors` | `ef` | Not available: fails. |
| `fkSkeleton` | `fk` | Not available: fails. |
| `stancePose` | `sp` | Not available: fails. |
| `attributeFromHIKProperty` | `ahk` | Not available: fails. |
| `attributeFromHIKPropertyMode` | `mhk` | Not available: fails. |

## chdir

Change the current working directory.

Arguments: directory

## checkBox

Script UI: checkBox, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `label` | `l` |  |
| `value` | `v` |  |
| `changeCommand` | `cc` |  |
| `onCommand` | `onc` |  |
| `offCommand` | `ofc` |  |
| `align` | `al` |  |
| `editable` | `ed` |  |
| `recomputeSize` | `rs` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## checkBoxGrp

Script UI: checkBoxGrp, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `label` | `l` |  |
| `numberOfCheckBoxes` | `ncb` |  |
| `label1` | `l1` |  |
| `label2` | `l2` |  |
| `label3` | `l3` |  |
| `label4` | `l4` |  |
| `labelArray2` | `la2` |  |
| `labelArray3` | `la3` |  |
| `labelArray4` | `la4` |  |
| `value1` | `v1` |  |
| `value2` | `v2` |  |
| `value3` | `v3` |  |
| `value4` | `v4` |  |
| `valueArray2` | `va2` |  |
| `valueArray3` | `va3` |  |
| `valueArray4` | `va4` |  |
| `changeCommand` | `cc` |  |
| `changeCommand1` | `cc1` |  |
| `changeCommand2` | `cc2` |  |
| `changeCommand3` | `cc3` |  |
| `changeCommand4` | `cc4` |  |
| `onCommand` | `onc` |  |
| `onCommand1` | `on1` |  |
| `onCommand2` | `on2` |  |
| `onCommand3` | `on3` |  |
| `onCommand4` | `on4` |  |
| `offCommand` | `ofc` |  |
| `offCommand1` | `of1` |  |
| `offCommand2` | `of2` |  |
| `offCommand3` | `of3` |  |
| `offCommand4` | `of4` |  |
| `enable1` | `en1` |  |
| `enable2` | `en2` |  |
| `enable3` | `en3` |  |
| `enable4` | `en4` |  |
| `editable` | `ed` |  |
| `vertical` | `vr` |  |
| `columnWidth1` | `cw1` |  |
| `columnAlign1` | `cl1` |  |
| `columnAttach1` | `ct1` |  |
| `columnOffset1` | `co1` |  |
| `columnWidth2` | `cw2` |  |
| `columnAlign2` | `cl2` |  |
| `columnAttach2` | `ct2` |  |
| `columnOffset2` | `co2` |  |
| `adjustableColumn2` | `ad2` |  |
| `columnWidth3` | `cw3` |  |
| `columnAlign3` | `cl3` |  |
| `columnAttach3` | `ct3` |  |
| `columnOffset3` | `co3` |  |
| `adjustableColumn3` | `ad3` |  |
| `columnWidth4` | `cw4` |  |
| `columnAlign4` | `cl4` |  |
| `columnAttach4` | `ct4` |  |
| `columnOffset4` | `co4` |  |
| `adjustableColumn4` | `ad4` |  |
| `columnWidth5` | `cw5` |  |
| `columnAlign5` | `cl5` |  |
| `columnAttach5` | `ct5` |  |
| `columnOffset5` | `co5` |  |
| `adjustableColumn5` | `ad5` |  |
| `columnWidth6` | `cw6` |  |
| `columnAlign6` | `cl6` |  |
| `columnAttach6` | `ct6` |  |
| `columnOffset6` | `co6` |  |
| `adjustableColumn6` | `ad6` |  |
| `adjustableColumn` | `adj` |  |
| `columnWidth` | `cw` |  |
| `columnAlign` | `cal` |  |
| `columnAttach` | `cat` |  |
| `rowAttach` | `rat` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## circle

Create a NURBS circle, with a makeNurbCircle node (-nr normal, -c centre, -r radius, -d degree, -s sections, -sw sweep). Returns [nurbsCircle1, makeNurbCircle1].

| Flag | Short | Description |
| --- | --- | --- |
| `normal` | `nr` | The circle's normal (0 0 1). |
| `center` | `c` | The centre. |
| `firstPoint` | `fp` | Toward the first section's start. |
| `radius` | `r` | Radius (1). |
| `degree` | `d` | Degree (3). |
| `sections` | `s` | Sections (8). |
| `sweep` | `sw` | Degrees covered (360). |
| `useTolerance` | `ut` | Accepted for existing scripts. |
| `tolerance` | `tol` | Accepted for existing scripts. |
| `constructionHistory` | `ch` | Keep the node (on by default). |
| `object` | `o` | Accepted for existing scripts. |
| `name` | `n` | Name of the transform. |

## circularFillet

A round fillet between two surfaces: -pr, -sr offsets from each (signed, on their normals). Returns [surface, node].

Arguments: two surfaces

| Flag | Short | Description |
| --- | --- | --- |
| `radius` | `r` | Both offsets (1). |
| `primaryRadius` | `pr` | Offset from the first. |
| `secondaryRadius` | `sr` | Offset from the second. |
| `curveOnSurface` | `cos` | Accepted for existing scripts. |
| `constructionHistory` | `ch` | Keep the node (on by default). |
| `name` | `n` | The new object's name. |
| `object` | `o` | Accepted for existing scripts. |
| `range` | `rn` | Accepted for existing scripts. |
| `caching` | `cch` | Accepted for existing scripts. |
| `nodeState` | `nds` | Accepted for existing scripts. |
| `tolerance` | `tol` | Accepted for existing scripts. |
| `polygon` | `po` | 0 NURBS, 1 polygons. |

## cleanUpScene

Optimize Scene Size essentials: removes transforms with nothing under them, deepest first. Returns what went.

Arguments: [what]

| Flag | Short | Description |
| --- | --- | --- |
| `emptyTransforms` | `et` | Remove transforms with nothing under them. |

## clip

Make a clip of a character's keys between -s and -end (all of them when not given): the keys move to a source clip and, unless -sc is off, it is scheduled. Returns the source clip and the clip. -q -ac and -q -asc list clips, -rm -n removes one from the schedule.

Arguments: the character (the current one when none)

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Return the value of the flags named instead of setting them. |
| `edit` | `e` | Change the flags named. |
| `name` | `n` | The clip's name. |
| `character` | `ch` | The character. |
| `startTime` | `s` | Start of the keys taken. |
| `endTime` | `end` | End of the keys taken. |
| `st` | `st` | As -s. |
| `et` | `et` | As -end. |
| `scheduleClip` | `sc` | Schedule it as well as making the source (on by default). |
| `scheduleStart` | `ss` | Where to schedule it (where the keys were). |
| `sourceClipName` | `scn` | The source clip's name (the clip's plus Source). |
| `leaveKeys` | `lk` | Copy the keys rather than move them. |
| `allRelative` | `rel` | The clip adds to what is under it. |
| `allAbsolute` | `abs` | The clip replaces what is under it (the default). |
| `allClips` | `ac` | Query: the scheduled clips. |
| `allSourceClips` | `asc` | Query: the source clips. |
| `remove` | `rm` | Take the clip off the schedule. |

## clipEditor

Clips are not available here: warns, and opens the Dope Sheet.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `allTrackHeights` | `th` |  |
| `autoFit` | `af` |  |
| `autoFitTime` | `aft` |  |
| `clipDropCmd` | `cd` |  |
| `clipStyle` | `cs` |  |
| `deleteCmd` | `dlc` |  |
| `deselectAll` | `da` |  |
| `displayActiveKeyTangents` | `dat` |  |
| `displayActiveKeys` | `dak` |  |
| `displayInfinities` | `di` |  |
| `displayKeys` | `dk` |  |
| `displayTangents` | `dtn` |  |
| `displayValues` | `dv` |  |
| `frameAll` | `fa` |  |
| `frameRange` | `fr` |  |
| `highlightConnection` | `hlc` |  |
| `highlightedBlend` | `hb` |  |
| `highlightedClip` | `hc` |  |
| `initialized` | `in` |  |
| `listAllCharacters` | `lac` |  |
| `listCurrentCharacters` | `lc` |  |
| `lockMainConnection` | `lck` |  |
| `lookAt` | `la` |  |
| `mainListConnection` | `mlc` |  |
| `manageSequencer` | `ms` |  |
| `menuContext` | `mc` |  |
| `parent` | `p` |  |
| `selectBlend` | `sb` |  |
| `selectClip` | `sc` |  |
| `selectionConnection` | `slc` |  |
| `snapTime` | `st` |  |
| `snapValue` | `sv` |  |
| `stateString` | `sts` |  |
| `useTemplate` | `ut` |  |
| `defineTemplate` | `dt` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## clipMatching

Line a clip up with another at a frame each: -cs clip frame, -cd clip frame, -mt and -mr which channels (translate by default).

Arguments: character

| Flag | Short | Description |
| --- | --- | --- |
| `clipSrc` | `cs` | The clip to match and its frame. |
| `clipDst` | `cd` | The clip to shift and its frame. |
| `matchTranslation` | `mt` | Match translates (on by default). |
| `matchRotation` | `mr` | Match rotates. |

## clipSchedule

A character's schedule. -q gives one line per clip: name, index, start, source start, source end, scale, pre cycle, post cycle, hold. -e with -n or -ci changes a clip (-s, -sc, -ss, -se, -c, -w, -t, -en, -m, -so) or -rm removes it; -ins schedules a source clip again.

Arguments: the scheduler, or its character

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Return the value of the flags named instead of setting them. |
| `edit` | `e` | Change the flags named. |
| `name` | `n` | The clip. |
| `clipIndex` | `ci` | The clip, by index. |
| `start` | `s` | When it starts. |
| `sourceStart` | `ss` | The first of its source used. |
| `sourceEnd` | `se` | The last of its source used. |
| `scale` | `sc` | Its speed: 2 takes twice as long. |
| `cycle` | `c` | How many times it plays. |
| `preCycle` | `prc` | How many times it plays before its start. |
| `weight` | `w` | Its weight. |
| `track` | `t` | Its track. |
| `shift` | `sh` | Move it this many tracks. |
| `hold` | `h` | Frames held at the end. |
| `postHold` | `ph` | Frames held after. |
| `preHold` | `prh` | Frames held before. |
| `enable` | `en` | Plays or not. |
| `mute` | `m` | Muted or not. |
| `solo` | `so` | Soloed or not. |
| `lock` | `l` | Locked or not. |
| `remove` | `rm` | Take it off the schedule. |
| `instance` | `ins` | Schedule this source clip. |
| `insertTrack` | `it` | Open a track here. |
| `removeTrack` | `rt` | Remove this track and its clips. |
| `removeEmptyTracks` | `ret` | Close up empty tracks. |
| `allRelative` | `rel` | It adds to what is under it. |
| `allAbsolute` | `abs` | It replaces what is under it. |

## closeCurve

Close a curve. Returns [curve, node].

Arguments: curves (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `preserveShape` | `ps` | Accepted for existing scripts. |
| `blendBias` | `bb` | Accepted for existing scripts. |
| `constructionHistory` | `ch` | Keep a history node (on by default). |
| `replaceOriginal` | `rpo` | Change the curve itself (on by default). |
| `name` | `n` | Name of a new curve. |
| `object` | `o` | Make an object (accepted for existing scripts). |

## closeSurface

Close a surface: -d 0 U, 1 V, 2 both. Returns [surface, node].

Arguments: surfaces (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `direction` | `d` | 0 U, 1 V, 2 both (0). |
| `preserveShape` | `ps` | Accepted for existing scripts. |
| `blendBias` | `bb` | Accepted for existing scripts. |
| `blendKnotInsertion` | `bki` | Accepted for existing scripts. |
| `parameter` | `p` | Accepted for existing scripts. |
| `constructionHistory` | `ch` | Keep a history node (on by default). |
| `replaceOriginal` | `rpo` | Change the surface itself (on by default). |
| `name` | `n` | Name of a new surface. |
| `object` | `o` | Accepted for existing scripts. |
| `polygon` | `po` | Accepted for existing scripts. |

## closestPointOnMesh

Make a node on a mesh that answers the point on it closest to -ip. Returns the node.

Arguments: mesh

| Flag | Short | Description |
| --- | --- | --- |
| `inPosition` | `ip` | The position. |
| `name` | `n` | The node's name. |

## clothPreset

Set cloth to a material: -preset silk, cotton, leather, heavyDenim or rubber.

Arguments: cloth nodes or meshes

| Flag | Short | Description |
| --- | --- | --- |
| `preset` | `p` | The material. |

## cluster

Make a cluster of the selected vertices (or whole meshes): a handle they follow. Returns [cluster, handle].

Arguments: meshes or vertices (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `name` | `n` | Name of the cluster; its handle takes the name plus Handle. |

## cmdFileOutput

Writes the Script Editor's lines to a file as well: -open file gives a descriptor, -close descriptor and -closeAll stop, -status descriptor is 0 while the file is open.

| Flag | Short | Description |
| --- | --- | --- |
| `open` | `o` | Open this file and write every line to it from now on. The descriptor, or -1. |
| `close` | `c` | Close the file with this descriptor. |
| `closeAll` | `ca` | Close them all. |
| `status` | `s` | 0 while the file with this descriptor is open. |
| `query` | `q` | Query. |

## collider

Make the selected meshes colliders: cloth, particles and rigid bodies hit them. Returns the collider nodes.

Arguments: meshes

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query a value. |
| `edit` | `e` | Edit an existing node. |
| `name` | `n` | Name of the new node. |
| `solver` | `sv` | The solver to join. |
| `thickness` | `th` | Collision thickness. |

## color

Set the dormant wireframe colour of objects (the selection by default): -ud 1 to 8, -rgb r g b, or neither for the class colour.

Arguments: objects

| Flag | Short | Description |
| --- | --- | --- |
| `rgbColor` | `rgb` | An RGB colour. |
| `userDefined` | `ud` | One of the eight user colours, 1 to 8. |

## colorIndex

A palette entry's colour: index r g b, 1 to 31, -q asks.

Arguments: index [r g b]

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `hueSaturationValue` | `hsv` | Accepted for existing scripts. |
| `resetToFactory` | `rf` | Back to the factory colours. |
| `resetToSaved` | `rs` | Back to the saved colours. |

## colorIndexSliderGrp

Script UI: colorIndexSliderGrp, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `label` | `l` |  |
| `value` | `v` |  |
| `minValue` | `min` |  |
| `maxValue` | `max` |  |
| `invisible` | `inv` |  |
| `forceDragRefresh` | `fdr` |  |
| `extraLabel` | `el` |  |
| `changeCommand` | `cc` |  |
| `dragCommand` | `dc` |  |
| `columnWidth1` | `cw1` |  |
| `columnAlign1` | `cl1` |  |
| `columnAttach1` | `ct1` |  |
| `columnOffset1` | `co1` |  |
| `columnWidth2` | `cw2` |  |
| `columnAlign2` | `cl2` |  |
| `columnAttach2` | `ct2` |  |
| `columnOffset2` | `co2` |  |
| `adjustableColumn2` | `ad2` |  |
| `columnWidth3` | `cw3` |  |
| `columnAlign3` | `cl3` |  |
| `columnAttach3` | `ct3` |  |
| `columnOffset3` | `co3` |  |
| `adjustableColumn3` | `ad3` |  |
| `columnWidth4` | `cw4` |  |
| `columnAlign4` | `cl4` |  |
| `columnAttach4` | `ct4` |  |
| `columnOffset4` | `co4` |  |
| `adjustableColumn4` | `ad4` |  |
| `columnWidth5` | `cw5` |  |
| `columnAlign5` | `cl5` |  |
| `columnAttach5` | `ct5` |  |
| `columnOffset5` | `co5` |  |
| `adjustableColumn5` | `ad5` |  |
| `columnWidth6` | `cw6` |  |
| `columnAlign6` | `cl6` |  |
| `columnAttach6` | `ct6` |  |
| `columnOffset6` | `co6` |  |
| `adjustableColumn6` | `ad6` |  |
| `adjustableColumn` | `adj` |  |
| `columnWidth` | `cw` |  |
| `columnAlign` | `cal` |  |
| `columnAttach` | `cat` |  |
| `rowAttach` | `rat` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## colorSliderGrp

Script UI: colorSliderGrp, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `label` | `l` |  |
| `rgbValue` | `rgb` |  |
| `hsvValue` | `hsv` |  |
| `forceDragRefresh` | `fdr` |  |
| `changeCommand` | `cc` |  |
| `dragCommand` | `dc` |  |
| `columnWidth1` | `cw1` |  |
| `columnAlign1` | `cl1` |  |
| `columnAttach1` | `ct1` |  |
| `columnOffset1` | `co1` |  |
| `columnWidth2` | `cw2` |  |
| `columnAlign2` | `cl2` |  |
| `columnAttach2` | `ct2` |  |
| `columnOffset2` | `co2` |  |
| `adjustableColumn2` | `ad2` |  |
| `columnWidth3` | `cw3` |  |
| `columnAlign3` | `cl3` |  |
| `columnAttach3` | `ct3` |  |
| `columnOffset3` | `co3` |  |
| `adjustableColumn3` | `ad3` |  |
| `columnWidth4` | `cw4` |  |
| `columnAlign4` | `cl4` |  |
| `columnAttach4` | `ct4` |  |
| `columnOffset4` | `co4` |  |
| `adjustableColumn4` | `ad4` |  |
| `columnWidth5` | `cw5` |  |
| `columnAlign5` | `cl5` |  |
| `columnAttach5` | `ct5` |  |
| `columnOffset5` | `co5` |  |
| `adjustableColumn5` | `ad5` |  |
| `columnWidth6` | `cw6` |  |
| `columnAlign6` | `cl6` |  |
| `columnAttach6` | `ct6` |  |
| `columnOffset6` | `co6` |  |
| `adjustableColumn6` | `ad6` |  |
| `adjustableColumn` | `adj` |  |
| `columnWidth` | `cw` |  |
| `columnAlign` | `cal` |  |
| `columnAttach` | `cat` |  |
| `rowAttach` | `rat` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## columnLayout

Script UI: columnLayout, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `adjustableColumn` | `adj` |  |
| `columnAlign` | `cal` |  |
| `columnAttach` | `cat` |  |
| `columnOffset` | `co` |  |
| `columnWidth` | `cw` |  |
| `rowSpacing` | `rs` |  |
| `childArray` | `ca` |  |
| `numberOfChildren` | `nch` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## combinationShape

Drive a blend shape target from others through a combinationShape node: -bs, -cti or -ctn, -dti or -dtn, -cm 0 multiply, 1 lowest, 2 lowest smooth. With -e: -add, -rmd, -ald, -cm; removing the last driver deletes the node. -q: -ex, -ald, -cm. Returns the target's index, or with -e the node.

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `addDriver` | `add` | With -e: add the drivers. |
| `allDrivers` | `ald` | All the drivers. |
| `blendShape` | `bs` | The blend shape. |
| `combinationTargetIndex` | `cti` | The driven target. |
| `combinationTargetName` | `ctn` | The driven target. |
| `combineMethod` | `cm` | 0 multiplication, 1 lowest weighting, 2 lowest weighting (smooth). |
| `driverTargetIndex` | `dti` | A driver. |
| `driverTargetName` | `dtn` | A driver. |
| `exist` | `ex` | With -q: whether the target has one. |
| `removeDriver` | `rmd` | With -e: remove the drivers. |

## commandEcho

What is echoed: -st on echoes commands called from Python too, -f and -af filter them by how their names start. -ln is only held: messages have no line numbers to show or hide here. -q reads them.

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `addFilter` | `af` | Names to add to the filter. |
| `filter` | `f` | Names to filter out. |
| `lineNumbers` | `ln` | Line numbers in messages. |
| `state` | `st` | Echo everything. |

## componentEditor

Script UI: componentEditor, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `floatField` | `ff` |  |
| `floatSlider` | `fs` |  |
| `hidePathName` | `hpn` |  |
| `hideZeroColumns` | `hzc` |  |
| `lockInput` | `lck` |  |
| `newTab` | `nt` |  |
| `operationCount` | `oc` |  |
| `operationLabels` | `ol` |  |
| `operationType` | `ot` |  |
| `precision` | `pre` |  |
| `removeTab` | `rt` |  |
| `selected` | `sl` |  |
| `setOperationLabel` | `sol` |  |
| `showObjects` | `so` |  |
| `showSelected` | `ss` |  |
| `sortAlpha` | `sa` |  |
| `control` | `ctl` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `filter` | `f` |  |
| `forceMainConnection` | `fmc` |  |
| `highlightConnection` | `hlc` |  |
| `lockMainConnection` | `lck` |  |
| `mainListConnection` | `mlc` |  |
| `panel` | `pnl` |  |
| `selectionConnection` | `slc` |  |
| `stateString` | `sts` |  |
| `unParent` | `up` |  |
| `unlockMainConnection` | `ulk` |  |
| `updateMainConnection` | `upd` |  |
| `useTemplate` | `ut` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## componentInfo

Positions, normals and adjacency of a mesh's components in bulk. Faces: center, normal, area, points, edges, neighbors (faces across each edge). Edges: points, center, length, faces, border. Vertices: position, normal, edges, neighbors (points), faces. World space unless space='object'. ids= picks some (default: the selected ones of that kind, or all).

Arguments: [mesh]

| Flag | Short | Description |
| --- | --- | --- |
| `kind` | `k` | vertex, edge or face. |
| `ids` |  | Component ids; [first, last] pairs are ranges. |
| `space` |  | world or object. |
| `fields` |  | Keys to keep per component, e.g. ['id', 'center', 'normal'] (default all). |

## componentSelection

Selected components: {mesh: {"vertices": [...], "edges": [...], "faces": [...]}}. ordered=True gives a flat list [[mesh, kind, id], ...] in pick order instead (kind is vertex, edge or face).

| Flag | Short | Description |
| --- | --- | --- |
| `ordered` |  | True: a flat list [[mesh, kind, id], ...] in the order the components were picked. False: grouped per mesh and kind, ids sorted ascending, pick order lost. |

## condition

A named condition isTrue and scriptJob -ct can read: -s the MEL script that gives its state, -d conditions it follows (checked at idle), -i run the script now, -st set the state, -delete remove it. -q -st reads it. Scripts run just after the command, or at idle when a dependency changes. Not undoable, and not saved.

Arguments: its name

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `delete` | `delete` | Delete it. |
| `dependency` | `d` | A condition it follows. |
| `initialize` | `i` | Run its script now. |
| `script` | `s` | The MEL script giving its state. |
| `state` | `st` | Its state. |

## cone

Create a NURBS cone with a makeNurbCone node. Returns [nurbsCone1, makeNurbCone1].

| Flag | Short | Description |
| --- | --- | --- |
| `axis` | `ax` | The axis (1 0 0). |
| `pivot` | `p` | The centre. |
| `constructionHistory` | `ch` | Keep the make node (on by default). |
| `name` | `n` | Name of the transform. |
| `object` | `o` | Accepted for existing scripts. |
| `polygon` | `po` | 1 makes polygons. |
| `radius` | `r` | The radius (1). |
| `startSweep` | `ssw` | Start, degrees (0). |
| `endSweep` | `esw` | End, degrees (360). |
| `degree` | `d` | 1 linear, 3 cubic (3). |
| `sections` | `s` | Sections around (8). |
| `spans` | `nsp` | Spans along the axis. |
| `useTolerance` | `ut` | Accepted for existing scripts. |
| `tolerance` | `tol` | Accepted for existing scripts. |
| `heightRatio` | `hr` | Height over radius. |

## confirmDialog

A modal question with buttons. Returns the button pressed, or -dismissString when closed.

| Flag | Short | Description |
| --- | --- | --- |
| `title` | `t` |  |
| `message` | `m` |  |
| `messageAlign` | `ma` |  |
| `button` | `b` |  |
| `defaultButton` | `db` |  |
| `cancelButton` | `cb` |  |
| `dismissString` | `ds` |  |
| `icon` | `icn` |  |
| `backgroundColor` | `bgc` |  |
| `parent` | `p` |  |

## connectAttr

connectAttr('src.attr', 'dst.attr'). A whole array input ('loft1.sections') gets a new last element. Returns the destination plug.

Arguments: src dst

| Flag | Short | Description |
| --- | --- | --- |
| `force` | `f` | Replace an existing input connection. |
| `nextAvailable` | `na` | The next free element of a multi attribute. |
| `lock` | `l` | Lock the destination afterwards. |
| `referenceDest` | `rd` | Accepted for existing scripts. |

## connectControl

connectControl [-index n] control attribute...: binds a control to plugs. The control shows the plug, sets it undoably, and follows it. On a group, -index 1 is its label, 2 its first field.

Arguments: control attribute...

| Flag | Short | Description |
| --- | --- | --- |
| `index` | `in` | Which part of a group. |
| `fileName` | `fi` | A file name attribute. |
| `preventContextualMenu` | `pcm` | Accepted for existing scripts. |
| `preventOverride` | `po` | Accepted for existing scripts. |

## connectDynamic

Connect fields (-f), emitters (-em) and colliders (-c) to the bodies named; -d disconnects.

Arguments: bodies

| Flag | Short | Description |
| --- | --- | --- |
| `fields` | `f` | Fields. |
| `emitters` | `em` | Emitters. |
| `collisions` | `c` | Colliders. |
| `delete` | `d` | Disconnect instead. |
| `solver` | `sv` | The solver a body with none joins. |

## connectJoint

Join two skeletons: the second joint becomes the first one's parent. -pm keeps the first where it is, with a bone between them; -cm (the default) moves the first skeleton's root to the second joint.

Arguments: the joint to connect, then its new parent

| Flag | Short | Description |
| --- | --- | --- |
| `connectMode` | `cm` | Move the first joint to the second. |
| `parentMode` | `pm` | Keep the first joint where it is. |

## connectionInfo

Facts about a plug's connections.

Arguments: plug

| Flag | Short | Description |
| --- | --- | --- |
| `sourceFromDestination` | `sfd` | The plug driving this one, or "". |
| `destinationFromSource` | `dfs` | The plugs this one drives. |
| `getExactSource` | `ges` | The plug driving exactly this one. |
| `getExactDestination` | `ged` | The plugs driven by exactly this one. |
| `isDestination` | `id` | Driven. |
| `isExactDestination` | `ied` | Driven directly. |
| `isSource` | `is` | Drives something. |
| `isExactSource` | `ies` | Drives something directly. |
| `isLocked` | `il` | Locked. |

## constructionHistory

Turn construction history on or off for later poly edits, or query it. Returns whether it is on.

| Flag | Short | Description |
| --- | --- | --- |
| `toggle` | `tgl` | Turn it on if off, off if on. |
| `state` | `st` | Turn it on (True) or off (False). |

## container

Group nodes in a container node: -n, -an adds, -rn removes; with -e on an existing one. -q -nl lists, -q -fc finds the one holding a node.

Arguments: container

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `name` | `n` | The container's name. |
| `addNode` | `an` | Nodes to add. |
| `add` | `add` | Nodes to add. |
| `removeNode` | `rn` | Nodes to take out. |
| `rm` | `rm` | Nodes to take out. |
| `nodeList` | `nl` | With -q: its nodes. |
| `findContainer` | `fc` | With -q: the container a node is in. |
| `isContainer` | `isc` | With -q: whether it's a container. |
| `asset` | `a` | Marks it an asset. |
| `force` | `f` | Accepted for existing scripts. |
| `type` | `typ` | Accepted for existing scripts. |
| `includeHierarchyBelow` | `ihb` | Accepted for existing scripts. |

## contextInfo

About a tool context: -ex whether it exists, -c its class, -t its title.

Arguments: context

| Flag | Short | Description |
| --- | --- | --- |
| `exists` | `ex` |  |
| `class` | `c` |  |
| `title` | `t` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |

## control

Edit and query any control.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `childArray` | `ca` |  |
| `numberOfChildren` | `nch` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## controller

Tag objects as animation controls: controller obj; -p parent makes them its children; -q -p, -q -c read the tree; -ac all; -ic is it one; -up unparents.

Arguments: objects

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `parent` | `p` | The parent control, or with -q ask. |
| `children` | `c` | With -q: its children. |
| `allControllers` | `ac` | All the controls. |
| `isController` | `ic` | Whether the object is a control. |
| `unparent` | `unp` | Take from its parent. |
| `group` | `g` | Accepted for existing scripts. |
| `index` | `idx` | Accepted for existing scripts. |
| `pickWalkDown` | `pwd` | Accepted for existing scripts. |
| `pickWalkUp` | `pwu` | Accepted for existing scripts. |

## convertSelection

Convert the component selection to another kind and switch to that mode. In object mode the selected meshes convert as a whole: every component of that kind.

Arguments: to

## convertToInstance

Replace meshes identical to a source with instances of it, dropping their own copies of the data: cleans up imported kits. The source is the last mesh given (the lead); with only the source, every other mesh in the scene is checked. Returns the meshes converted.

Arguments: meshes... source

| Flag | Short | Description |
| --- | --- | --- |
| `tolerance` |  | Points may differ by this much, object space, cm. |
| `force` |  | Convert the given meshes even when their geometry differs from the source's. |

## convertUnit

Convert a value between units: convertUnit -f inch -t cm "1". Returns the value with its unit.

Arguments: value

| Flag | Short | Description |
| --- | --- | --- |
| `fromUnit` | `f` | The unit of the value (centimeters, or as the value spells it). |
| `toUnit` | `t` | The unit wanted. |

## copyAttr

Copy attributes between nodes: -v values, -ic and -oc move connections (-ksc leaves the source's), -at which attributes.

Arguments: source target

| Flag | Short | Description |
| --- | --- | --- |
| `attribute` | `at` | Attributes to copy (all that both nodes have). |
| `values` | `v` | Copy values. |
| `inConnections` | `ic` | Move incoming connections. |
| `outConnections` | `oc` | Move outgoing connections. |
| `keepSourceConnections` | `ksc` | Leave the source its connections. |
| `containerParentChild` | `cpc` | Accepted for existing scripts. |
| `renameTargetContainer` | `rtc` | Accepted for existing scripts. |

## copyDeformerWeights

Copy a deformer's weights to another by closest point in world space: -sd, -dd, -ss, -ds; -mm XY, XZ or YZ to mirror them.

| Flag | Short | Description |
| --- | --- | --- |
| `sourceDeformer` | `sd` | The deformer to copy from. |
| `destinationDeformer` | `dd` | The deformer to copy to. |
| `sourceShape` | `ss` | Its shape. |
| `destinationShape` | `ds` | Its shape. |
| `mirrorMode` | `mm` | XY, XZ or YZ. |
| `mirrorInverse` | `mi` | Accepted for existing scripts. |
| `noMirror` | `nm` | Don't mirror. |
| `smooth` | `sm` | Accepted for existing scripts. |
| `surfaceAssociation` | `sa` | Accepted: closest point is what's used. |
| `uvSpace` | `uv` | Accepted for existing scripts. |

## copyFlexor

Copy a flexor's settings and lattice tweaks to another joint: the flexor's lattice, then the joint. Returns the new node. Less than the reference: a mirrored bone does not reflect the lattice.

Arguments: the flexor's lattice and the joint

## copyKey

Copy keys of objects or channels to the key clipboard; -t limits them. Returns how many.

Arguments: objects, plugs or curves (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `attribute` | `at` | Channel name(s). |
| `time` | `t` | A frame or range "10:20". |
| `float` | `f` | A range of driver values on driven key curves, as -t is for time. |
| `index` | `in` | Key index(es) on each curve, or a range "0:2". |
| `includeUpperBound` | `iub` | Whether a range's end counts (on by default). |
| `hierarchy` | `hi` | none, below, above or both. |
| `controlPoints` | `cp` | Accepted for existing scripts: control points have no channels of their own here. |
| `shape` | `s` | Include the channels of the objects' shapes. |
| `animation` | `an` | objects (the default), keys or keysOrObjects: keys means the selected keys. |
| `selected` | `sl` | Copy the keys selectKey selected. |
| `option` | `o` | keys (the default) or curve. |
| `clipboard` | `cb` | Accepted for existing scripts. |

## copySkinWeights

Copy weights between skins, or mirror them on one skin: copySkinWeights -ss skinCluster1 -ds skinCluster2 -nm, or copySkinWeights -ss skinCluster1 -ds skinCluster1 -mm YZ -mi. Selected points limit what the destination takes.

Arguments: source then destination (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `sourceSkin` | `ss` | Source skinCluster. |
| `destinationSkin` | `ds` | Destination skinCluster. |
| `mirrorMode` | `mm` | Mirror plane: YZ (default), XZ or XY. |
| `mirrorInverse` | `mi` | Copy from the negative side to the positive. |
| `noMirror` | `nm` | A plain copy. |
| `surfaceAssociation` | `sa` | closestPoint (default), rayCast or closestComponent. |
| `influenceAssociation` | `ia` | closestJoint (default), closestBone, oneToOne, label or name, tried in turn. |
| `normalize` | `nr` | Normalize the copied weights (default on). |
| `uvSpace` | `uv` | Source and destination UV sets: points read the source at their UV position, between the corners of the face there. Sets beyond map1 aren't kept here. |
| `sampleSpace` | `spa` | Accepted for existing scripts. |

## createCloth

Make the selected (or named) meshes cloth in a solver: the mesh as it comes is the animation the cloth starts from and can be drawn to. Returns the cloth nodes.

Arguments: meshes

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query a value. |
| `edit` | `e` | Edit an existing node. |
| `name` | `n` | Name of the new node. |
| `solver` | `sv` | The solver to join (the newest, or a new one). |
| `preset` | `p` | The material it starts as: silk, cotton (the default), leather, heavyDenim or rubber. |

## createCurveWarp

Make a curveWarp on a mesh and a curve, as the Deform menu's Create Curve Warp does. The reference does not document this as a command; it runs curveWarp with what it is given.

Arguments: the mesh, then the curve (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `name` | `n` | Name of the deformer. |
| `before` | `bf` | Go just above the deformer nearest the mesh. |
| `after` | `af` | Go nearest the mesh, after the other deformers (the default). |
| `frontOfChain` | `foc` | Go above every other deformer. |
| `envelope` | `en` | How much of the deformation applies (1). |
| `edit` | `e` | Edit mode. |
| `query` | `q` | Query mode. |
| `curve` | `c` | The curve; or name it last. |
| `lengthScale` | `ls` | Stretches the mesh along the curve (1). |
| `offset` | `off` | Slides the mesh along the curve, as a fraction. |
| `twistRotation` | `tw` | Degrees of twist from end to end. |
| `rollRotation` | `rr` | Degrees the mesh rolls about the curve. |
| `alignmentMode` | `am` | 0 auto, 1 X, 2 Y, 3 Z. |
| `keepLength` | `kl` | Keep the mesh's length, not the curve's. |

## createDisplayLayer

Create a display layer holding the objects (the selection by default), taking them out of any layer they were in. Returns its name.

Arguments: objects

| Flag | Short | Description |
| --- | --- | --- |
| `name` | `n` | Name; layer1 when empty, made unique. |
| `empty` | `e` | Make it with no members, whatever is selected. |

## createMesh

Make a mesh from points and faces (lists of point indices, any size). Returns its name.

| Flag | Short | Description |
| --- | --- | --- |
| `points` |  | [[x, y, z], ...] in cm. |
| `faces` |  | [[i, j, k, ...], ...]. |
| `uvs` |  | [[u, v], ...]: one per face corner, face by face in faces' order. Fills uv0. |
| `name` | `n` | Name; made unique. |

## createNCloth

Make the selected (or named) meshes cloth in a solver: the mesh as it comes is the animation the cloth starts from and can be drawn to. Returns the cloth nodes.

Arguments: meshes

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query a value. |
| `edit` | `e` | Edit an existing node. |
| `name` | `n` | Name of the new node. |
| `solver` | `sv` | The solver to join (the newest, or a new one). |
| `preset` | `p` | The material it starts as: silk, cotton (the default), leather, heavyDenim or rubber. |

## createNConstraint

Hold points of bodies: -type weld, componentToComponent, pointToSurface, tear, pin, hinge, spring, nail or pointToPoint. The last item named is the target where the type takes one.

Arguments: components or objects

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query a value. |
| `edit` | `e` | Edit an existing node. |
| `name` | `n` | Name of the new node. |
| `type` | `t` | What it holds. |
| `strength` | `s` | How stiff. |
| `restLength` | `rl` | A fixed rest length. |
| `glueStrength` | `gs` | Tear: breaks past this stretch. |
| `solver` | `sv` | The solver to join. |

## createNode

Create a node of the given type. Returns its path (the name, unless another node shares it).

Arguments: type

| Flag | Short | Description |
| --- | --- | --- |
| `name` | `n` | Name; made unique among its siblings. |
| `parent` | `p` | Parent transform. |
| `skipSelect` | `ss` | Don't select the new node. |
| `shared` | `s` | Reuse a node of that name when there is one. |

## createPolyCubeCtx

A createPolyCubeCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `width` | `w` |  |
| `height` | `h` |  |
| `depth` | `d` |  |
| `subdivisionsX` | `sx` |  |
| `subdivisionsY` | `sy` |  |
| `subdivisionsZ` | `sz` |  |
| `axis` | `ax` |  |
| `createUVs` | `cuv` |  |
| `doSubdivisionsCapsUI` | `dsc` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## createPolyPlaneCtx

A createPolyPlaneCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `width` | `w` |  |
| `height` | `h` |  |
| `subdivisionsX` | `sx` |  |
| `subdivisionsY` | `sy` |  |
| `axis` | `ax` |  |
| `createUVs` | `cuv` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## createSet

Create an object set holding the members (the selected components in component mode, else the selected objects). Members are object names or components like pCube1.vtx[0:3], pCube1.e[5], pCube1.f[2]. Returns its name.

Arguments: members

| Flag | Short | Description |
| --- | --- | --- |
| `name` | `n` | Name; set1 when empty, made unique. |
| `quickSelect` | `qs` | Make it a quick select set (Edit > Quick Select Sets). |
| `empty` | `e` | Make it with no members, whatever is selected. |

## createSolver

Make a solver: solver1. Bodies made after it join it unless told another.

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query a value. |
| `edit` | `e` | Edit an existing node. |
| `name` | `n` | Name of the new node. |

## ctxAbort

Drops what the current tool has so far.

## ctxCompletion

Tells the current tool to finish, as Enter does.

## ctxEditMode

Toggle the current tool's edit mode: the pivot, for the move, rotate and scale tools. -q reads it.

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` |  |
| `buttonDown` | `bd` |  |
| `buttonUp` | `bu` |  |

## currentCharacters

The characters keys go to when nothing is selected. Names set them, -cl clears them, -q lists them.

Arguments: characters

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Return the value of the flags named instead of setting them. |
| `edit` | `e` | Change the flags named. |
| `clear` | `cl` | No current character. |

## currentCtx

The name of the current tool context.

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` |  |
| `edit` | `e` |  |

## currentTime

Set the current frame, or query it. Animated channels move to their values there. A value may carry a unit: 2sec, 12ntsc.

Arguments: frame

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Return the value of the flags named instead of setting them. |
| `edit` | `e` | Change the flags named. |
| `time` | `t` | The frame to go to. |
| `update` | `u` | Whether to evaluate now (on by default); either way the scene is up to date when asked. |

## currentTimeCtx

A currentTimeCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## currentUnit

The scene's units: -t time (film, pal, ntsc, game, show, palf, ntscf or Nfps), -l linear, -a angle. Changing the time unit keeps keys at the same moments unless -ua is off, which keeps their frame numbers.

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Return the value of the flags named instead of setting them. |
| `edit` | `e` | Change the flags named. |
| `time` | `t` | Time unit. |
| `linear` | `l` | Linear unit: mm, cm, m, km, in, ft, yd, mi. |
| `angle` | `a` | Angle unit: deg or rad. |
| `fullName` | `f` | Query: the unit's full name. |
| `updateAnimation` | `ua` | Keep keys at the same moments (on by default). |

## curve

Create a NURBS curve: curve -d 3 -p 0 0 0 -p 1 0 0 -p 2 1 0 -p 3 1 0. -k gives knots, -per a periodic curve, -ep edit points to pass through, -bezier takes Bezier points. Given a curve: -a appends the points, -r replaces it. Returns its transform.

Arguments: a curve, with -a or -r

| Flag | Short | Description |
| --- | --- | --- |
| `degree` | `d` | Degree (3). |
| `point` | `p` | A CV: x y z. Repeat it. |
| `editPoint` | `ep` | An edit point to pass through: x y z. Repeat it. |
| `knot` | `k` | A knot. Repeat it; CVs + degree - 1 of them. |
| `periodic` | `per` | A closed, periodic curve. |
| `bezier` | `bez` | The points are Bezier points. |
| `append` | `a` | Add the points to the curve named. |
| `replace` | `r` | Replace the curve named. |
| `worldSpace` | `ws` | Accepted for existing scripts. |
| `objectSpace` | `os` | Accepted for existing scripts. |
| `name` | `n` | Name of the transform. |

## curveAddPtCtx

A curveAddPtCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## curveBezierCtx

A curveBezierCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `degree` | `d` |  |
| `manipMode` | `mm` |  |
| `preserveShape` | `ps` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## curveCVCtx

A curveCVCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `degree` | `d` |  |
| `multEndKnots` | `me` |  |
| `preserveShape` | `ps` |  |
| `rational` | `r` |  |
| `refit` | `rf` |  |
| `uniform` | `un` |  |
| `bezier` | `bez` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## curveEPCtx

A curveEPCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `degree` | `d` |  |
| `multEndKnots` | `me` |  |
| `preserveShape` | `ps` |  |
| `rational` | `r` |  |
| `refit` | `rf` |  |
| `uniform` | `un` |  |
| `bezier` | `bez` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## curveEditorCtx

A curveEditorCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `direction` | `dir` |  |
| `relativeTangentSize` | `rts` |  |
| `tangentScale` | `ts` |  |
| `tangentTension` | `tt` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## curveIntersect

Where two curves cross, as text: "u1 u2 u1 u2 ..." on the first and second. -ud -d flattens along a direction first; -tol how close.

Arguments: two curves

| Flag | Short | Description |
| --- | --- | --- |
| `tolerance` | `tol` | How close counts (0.001). |
| `useDirection` | `ud` | Flatten along -d first. |
| `direction` | `d` | Which way. |
| `constructionHistory` | `ch` | Accepted for existing scripts. |

## curveMoveEPCtx

A curveMoveEPCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## curveOnSurface

A curve on a surface: -d degree, -uv u v for each point, -k knots, -per periodic. Returns surface->curve.

Arguments: a surface

| Flag | Short | Description |
| --- | --- | --- |
| `degree` | `d` | The degree (3). |
| `positionUV` | `uv` | A point. |
| `knot` | `k` | A knot. |
| `periodic` | `per` | Closed. |
| `name` | `n` | The curve's name. |
| `append` | `a` | Accepted for existing scripts. |
| `replace` | `r` | Accepted for existing scripts. |
| `positionTolerance` | `pt` | Accepted for existing scripts. |
| `tangentTolerance` | `tt` | Accepted for existing scripts. |

## curveSketchCtx

A curveSketchCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `degree` | `d` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## curveWarp

Curve Warp: lays a mesh along a curve, named by -c or last. Returns [curveWarp1].

Arguments: what to deform, and any target last (the selection when none); the deformer with -e and -q

| Flag | Short | Description |
| --- | --- | --- |
| `name` | `n` | Name of the deformer. |
| `before` | `bf` | Go just above the deformer nearest the mesh. |
| `after` | `af` | Go nearest the mesh, after the other deformers (the default). |
| `frontOfChain` | `foc` | Go above every other deformer. |
| `envelope` | `en` | How much of the deformation applies (1). |
| `edit` | `e` | Edit mode. |
| `query` | `q` | Query mode. |
| `curve` | `c` | The curve; or name it last. |
| `lengthScale` | `ls` | Stretches the mesh along the curve (1). |
| `offset` | `off` | Slides the mesh along the curve, as a fraction. |
| `twistRotation` | `tw` | Degrees of twist from end to end. |
| `rollRotation` | `rr` | Degrees the mesh rolls about the curve. |
| `alignmentMode` | `am` | 0 auto, 1 X, 2 Y, 3 Z. |
| `keepLength` | `kl` | Keep the mesh's length, not the curve's. |

## cutKey

Delete keys in -t (or all of them) from objects or channels. Curves left with no keys are deleted, leaving the channel at its value. -o curve keeps the rest of the curve's shape; curveCollapse, curveConnect and areaCollapse close the gap.

Arguments: objects, plugs or curves (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `attribute` | `at` | Channel name(s). |
| `time` | `t` | A frame or range "10:20". |
| `float` | `f` | A range of driver values on driven key curves, as -t is for time. |
| `index` | `in` | Key index(es) on each curve, or a range "0:2". |
| `includeUpperBound` | `iub` | Whether a range's end counts (on by default). |
| `hierarchy` | `hi` | none, below, above or both. |
| `controlPoints` | `cp` | Accepted for existing scripts: control points have no channels of their own here. |
| `shape` | `s` | Include the channels of the objects' shapes. |
| `animation` | `an` | objects (the default), keys or keysOrObjects: keys means the selected keys. |
| `clear` | `cl` | Delete without copying to the clipboard. |
| `selected` | `sl` | Delete the keys selectKey selected. |
| `option` | `o` | keys (the default), curve, curveCollapse, curveConnect or areaCollapse. |

## cycleCheck

Look for dependency cycles: -evaluation on|off the warning, -q asks; objects or -all: true when one is found, -l the plugs on it.

Arguments: objects

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `evaluation` | `ev` | Warn about cycles while evaluating. |
| `all` | `all` | Look through the whole scene. |
| `list` | `l` | List the plugs on a cycle. |
| `firstCycleOnly` | `fco` | Accepted: the first found is what's listed. |
| `children` | `c` | Accepted for existing scripts. |
| `parents` | `p` | Accepted for existing scripts. |
| `dag` | `dag` | Accepted for existing scripts. |

## cylinder

Create a NURBS cylinder with a makeNurbCylinder node. Returns [nurbsCylinder1, makeNurbCylinder1].

| Flag | Short | Description |
| --- | --- | --- |
| `axis` | `ax` | The axis (1 0 0). |
| `pivot` | `p` | The centre. |
| `constructionHistory` | `ch` | Keep the make node (on by default). |
| `name` | `n` | Name of the transform. |
| `object` | `o` | Accepted for existing scripts. |
| `polygon` | `po` | 1 makes polygons. |
| `radius` | `r` | The radius (1). |
| `startSweep` | `ssw` | Start, degrees (0). |
| `endSweep` | `esw` | End, degrees (360). |
| `degree` | `d` | 1 linear, 3 cubic (3). |
| `sections` | `s` | Sections around (8). |
| `spans` | `nsp` | Spans along the axis. |
| `useTolerance` | `ut` | Accepted for existing scripts. |
| `tolerance` | `tol` | Accepted for existing scripts. |
| `heightRatio` | `hr` | Height over radius. |

## dagPose

Saved poses of skeletons. -restore -bp returns the joints named to their bind pose; -save stores a pose; -q -bp lists the bind poses of the joints.

Arguments: joints or a dagPose (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `restore` | `r` | Return to the pose. |
| `save` | `s` | Save a new pose. |
| `bindPose` | `bp` | The bind pose. |
| `global` | `g` | The whole pose, not only what's below the joints named. |
| `name` | `n` | Name of a saved pose. |
| `edit` | `e` | Edit mode. |
| `query` | `q` | Query mode. |

## dataStructure

Define a metadata structure: -fmt raw -asString "name=N:int32=ID:string=label", or -af a file. -q lists them; -q -n N with -lmn, -dt or -asString reads one. -rm and -ral remove. Structures live in the session and are not saved.

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `asFile` | `af` | A file holding the description. |
| `asString` | `asString` | The description. |
| `dataType` | `dt` | With -lmn: each member's type too. |
| `format` | `fmt` | raw is the one there is. |
| `listMemberNames` | `lmn` | The member names. |
| `name` | `n` | The structure asked about. |
| `remove` | `rm` | Remove -n. |
| `removeAll` | `ral` | Remove all. |

## defaultAmbientLight

Make a light as the Create menu does: intensity, then the colour. Returns its transform.

Arguments: intensity, r, g, b and the rest

## defaultAreaLight

Make a light as the Create menu does: intensity, then the colour. Returns its transform.

Arguments: intensity, r, g, b and the rest

## defaultDirectionalLight

Make a light as the Create menu does: intensity, then the colour. Returns its transform.

Arguments: intensity, r, g, b and the rest

## defaultNavigation

Connects as dropping one node on another does: -connectToExisting -source file1 -destination lambert2.color. Plugs left out take the node's usual one.

| Flag | Short | Description |
| --- | --- | --- |
| `connectToExisting` | `ce` | Connect the source to the destination. |
| `source` | `s` | Node or plug to connect from. |
| `destination` | `d` | Node or plug to connect to. |
| `force` | `f` | Accepted: a connection is always replaced. |
| `quiet` | `qt` | Accepted for scripts. |
| `createNew` | `cn` | Accepted for scripts. |
| `defaultTraversal` | `dtv` | Accepted for scripts. |
| `defaultAttribute` | `da` | Accepted for scripts. |
| `navigatorDecisionString` | `nds` | Accepted for scripts. |
| `relatedNodes` | `ren` | Accepted for scripts. |
| `ignore` | `i` | Accepted for scripts. |
| `unignore` | `u` | Accepted for scripts. |
| `delete` | `del` | Accepted for scripts. |

## defaultPointLight

Make a light as the Create menu does: intensity, then the colour. Returns its transform.

Arguments: intensity, r, g, b and the rest

## defaultSpotLight

Make a light as the Create menu does: intensity, then the colour. Returns its transform.

Arguments: intensity, r, g, b and the rest

## defaultVolumeLight

Make a light as the Create menu does: intensity, then the colour. Returns its transform.

Arguments: intensity, r, g, b and the rest

## deferScript

evalDeferred for Python: the callable or source runs at the next idle. -lp last, -en next.

Arguments: script

| Flag | Short | Description |
| --- | --- | --- |
| `lowestPriority` | `low` |  |
| `lowPriority` | `lp` |  |
| `evaluateNext` | `en` |  |
| `list` | `ls` |  |

## deformableShape

The plugs of a deformable shape: -cog makes the intermediate original if there is none and returns its output, -og returns that output, -ogm the shape's own, -lsi and -lso the local in and out attributes, -wso the world out attribute.

Arguments: a mesh

| Flag | Short | Description |
| --- | --- | --- |
| `createOriginalGeometry` | `cog` | Make the original shape if there is none. |
| `originalGeometry` | `og` | The original shape's output plug. |
| `outputGeometry` | `ogm` | The shape's output plug. |
| `localShapeInAttr` | `lsi` | The local input attribute. |
| `localShapeOutAttr` | `lso` | The local output attribute. |
| `worldShapeOutAttr` | `wso` | The world output attribute. |

## deformer

Make a deformer: deformer -type ffd [-n name] objects, with ffd, cluster, softMod, deltaMush, tension, wrap, wire or a nonlinear type. Returns [node]. -q -g gives the deformed shapes, -q -gi their indices. -e -g obj adds geometry, -e -rm -g obj takes it off.

Arguments: objects to deform (the selection when none); the deformer with -e and -q

| Flag | Short | Description |
| --- | --- | --- |
| `edit` | `e` | Edit mode. |
| `query` | `q` | Query mode. |
| `type` | `typ` | The node type to make. |
| `name` | `n` | Name of the deformer. |
| `geometry` | `g` | Geometry to deform, add or remove. Query: the deformed shapes. |
| `geometryIndices` | `gi` | Query: the indices of the deformed shapes. |
| `remove` | `rm` | With -e -g: take the geometry off the deformer. |
| `before` | `bf` | Go just above the deformer nearest the mesh. |
| `after` | `af` | Go nearest the mesh (the default). |
| `frontOfChain` | `foc` | Go above every other deformer. |

## deformerWeights

Export or import a deformer's weights in the reference's layout: deformerWeights name.xml -ex -deformer skinCluster1 -path dir; -im with -method index, nearest, barycentric, bilinear or over. A .json file, or -fm JSON, goes as JSON. Returns the file's path.

Arguments: the file

| Flag | Short | Description |
| --- | --- | --- |
| `export` | `ex` | Write the file. |
| `import` | `im` | Read the file. |
| `deformer` | `df` | The deformers. -shape takes all those on a mesh. |
| `shape` | `sh` | The deformed shape. |
| `path` | `p` | The file's folder. |
| `method` | `m` | index (the default), nearest, barycentric, bilinear or over: over leaves points the file doesn't list. |
| `defaultValue` | `dv` | The weight of a point the file doesn't list. |
| `format` | `fm` | XML or JSON. |
| `weightPrecision` | `wp` | Decimal places in XML (3). |
| `weightTolerance` | `wt` | Leave out weights at or below this. |
| `ignoreName` | `ig` | Match deformers and sources by order, not name. |

## delete

Delete objects (and their children). Defaults to the selection.

Arguments: objects

| Flag | Short | Description |
| --- | --- | --- |
| `constructionHistory` | `ch` | Delete the objects' construction history instead: each mesh keeps what its history makes now. |
| `channels` | `c` | Animation curves and expressions on the channels. |
| `expressions` | `e` | Expressions on the channels. |
| `constraints` | `cn` | Constraints on the object. |
| `hierarchy` | `hi` | below or both: the objects under it too. |
| `shape` | `s` | Only the shapes. |
| `staticChannels` | `sc` | Curves whose keys all hold one value. |
| `timeAnimationCurves` | `tac` | Curves over time. |
| `unitlessAnimationCurves` | `uac` | Driven key curves. |
| `attribute` | `at` | Only on these attributes. |
| `inputConnectionsAndNodes` | `icn` | Everything feeding the channels. |
| `controlPoints` | `cp` | Accepted for existing scripts. |
| `motionPaths` | `mp` | Accepted for existing scripts. |

## deleteAttr

Delete attributes added with addAttr: deleteAttr('node.attr'), deleteAttr(objects, attribute=name), or a whole component with deleteAttr(objects, category=name), which also takes the name out of the object's usdApiSchemas.

Arguments: plugs or objects

| Flag | Short | Description |
| --- | --- | --- |
| `attribute` | `at` | The attribute to delete from each object. |
| `category` |  | Delete every added attribute under this Attribute Editor heading. |

## deleteComponents

Delete the selected vertices, edges or faces (with whatever depends on them).

## deleteDisplayLayer

Delete display layers; their objects stay, in no layer.

Arguments: layers

## deleteSet

Delete sets; their members stay.

Arguments: sets

## deleteUI

Delete windows, layouts, controls, menus: -window -layout -control -menu -menuItem pick the kind.

Arguments: names

| Flag | Short | Description |
| --- | --- | --- |
| `window` | `wnd` |  |
| `layout` | `lay` |  |
| `control` | `ctl` |  |
| `menu` | `m` |  |
| `menuItem` | `mi` |  |
| `panel` | `pnl` |  |
| `panelConfig` | `pc` |  |
| `editor` | `ed` |  |
| `collection` | `cl` |  |
| `radioMenuItemCollection` | `ric` |  |
| `uiTemplate` | `uit` |  |
| `exists` | `ex` |  |
| `toolContext` | `tc` |  |

## deltaMush

Add a delta mush after the mesh's other deformers: deltaMush -si 10 -ss 0.5. The mesh as it is now is the rest pose. Returns [deltaMush1].

Arguments: meshes or vertices (the selection when none); the deformer with -e and -q

| Flag | Short | Description |
| --- | --- | --- |
| `name` | `n` | Name of the deformer. |
| `before` | `bf` | Go just above the deformer nearest the mesh. |
| `after` | `af` | Go nearest the mesh, after the other deformers (the default). |
| `frontOfChain` | `foc` | Go above every other deformer. |
| `envelope` | `en` | How much of the deformation applies (1). |
| `edit` | `e` | Edit mode. |
| `query` | `q` | Query mode. |
| `smoothingIterations` | `si` | Smoothing passes (10). |
| `smoothingStep` | `ss` | How far each pass moves a point towards its neighbours (0.5). |
| `pinBorderVertices` | `pbv` | Border points stay out of the smoothing (on). |
| `pin` | `pin` | The same as -pinBorderVertices. |
| `inwardConstraint` | `ic` | Stops points moving in along the normal: 0 free, 1 not at all. |
| `outwardConstraint` | `oc` | Stops points moving out along the normal: 0 free, 1 not at all. |
| `displacement` | `dis` | How much of the rest pose's detail goes back on (1). |

## detachCurve

Cut a curve at -p (repeat it). With -rpo the first piece replaces it. Returns the pieces' transforms.

Arguments: a curve (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `parameter` | `p` | Where to cut. Repeat it. |
| `replaceOriginal` | `rpo` | The first piece replaces the curve. |
| `keep` | `k` | Accepted for existing scripts. |
| `constructionHistory` | `ch` | Accepted for existing scripts. |

## detachSurface

Cut a surface at -p (repeat it) along -d, 1 U or 0 V. Returns the pieces' transforms and the first node.

Arguments: a surface (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `parameter` | `p` | Where. Repeat it. |
| `direction` | `d` | 1 U, 0 V (1). |
| `keep` | `k` | Accepted for existing scripts. |
| `constructionHistory` | `ch` | Keep a history node (on by default). |
| `replaceOriginal` | `rpo` | Change the surface itself (on by default). |
| `name` | `n` | Name of a new surface. |
| `object` | `o` | Accepted for existing scripts. |
| `polygon` | `po` | Accepted for existing scripts. |

## dgInfo

The dependency graph as text: -nd nodes, -c connections, -nt a type, -d dirty ones, -of a file.

Arguments: objects

| Flag | Short | Description |
| --- | --- | --- |
| `allNodes` | `all` | Every node. |
| `nodes` | `nd` | List nodes (what it lists anyway). |
| `connections` | `c` | List connections. |
| `dirty` | `d` | Only nodes in this dirty state. |
| `nonDeletable` | `nod` | Only locked nodes. |
| `outputFile` | `of` | Write to this file. |
| `propagation` | `p` | Accepted for existing scripts. |
| `short` | `s` | Accepted for existing scripts. |
| `subgraph` | `sub` | Accepted for existing scripts. |
| `type` | `nt` | Only this type. |

## dgdirty

Marks nodes for recompute: named, or -a every one.

Arguments: objects or plugs

| Flag | Short | Description |
| --- | --- | --- |
| `allPlugs` | `a` | Every node. |
| `clean` | `c` | Accepted for existing scripts. |
| `verbose` | `v` | Accepted for existing scripts. |

## dgeval

Brings nodes or plugs up to date.

Arguments: objects or plugs

| Flag | Short | Description |
| --- | --- | --- |
| `verbose` | `v` | Accepted for existing scripts. |

## dgtimer

A stub: nothing is timed here. -on, -off, -reset are accepted, every result is 0.

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `timerOn` | `on` | Turn the timer on or off. |
| `timerOff` | `off` | Accepted. |
| `reset` | `r` | Accepted. |
| `outputFile` | `o` | Accepted. |
| `maxDisplay` | `m` | Accepted. |
| `sortMetric` | `sm` | Accepted. |
| `sortType` | `st` | Accepted. |
| `threshold` | `th` | Accepted. |
| `timerType` | `tt` | Accepted. |
| `uniqueName` | `un` | Accepted. |

## directKeyCtx

A directKeyCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `option` | `o` |  |
| `selectedOnly` | `slo` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## directionalLight

Create a directional light, or query and edit one. Returns its shape.

Arguments: the light, to edit or query

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `name` | `n` | Its name. |
| `rgb` | `rgb` | Colour. |
| `shadowColor` | `sc` | Shadow colour. |
| `position` | `pos` | Position. |
| `rotation` | `rot` | Rotation. |
| `exclusive` | `exc` | Out of defaultLightSet: lights only what it's linked to. |
| `shadowDither` | `sd` | Accepted for scripts. |
| `intensity` | `i` | intensity |
| `shadowSamples` | `sh` | shadowRays |
| `useRayTraceShadows` | `rs` | useRayTraceShadows |
| `decayRate` | `d` | decayRate |
| `softShadow` | `ss` | castSoftShadows |
| `discRadius` | `drs` | lightRadius |

## dirmap

Folders mapped to other folders, for scenes moved between machines: -m from to adds a mapping, -um from removes one, -cd path gives the path with its mapping applied (or as it is), -gmd from the folder one maps to, -gam all of them, -en turns mapping on or off.

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `convertDirectory` | `cd` | A path, with its mapping applied. |
| `enable` | `en` | Mapping on or off. |
| `getAllMappings` | `gam` | Every mapping: from, to, from, to. |
| `getMappedDirectory` | `gmd` | What a folder maps to. |
| `mapDirectory` | `m` | From and to. |
| `unmapDirectory` | `um` | Remove this folder's mapping. |

## disable

Enable or disable a control: -v true (the default) disables it, false enables it.

Arguments: the control

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | True disables, false enables. |

## disconnectAttr

disconnectAttr('dst.attr'), or (src, dst). A whole array input loses every element, or with src only the ones src drives.

Arguments: [src] dst

| Flag | Short | Description |
| --- | --- | --- |
| `nextAvailable` | `na` | Disconnect the next element src drives. |

## disconnectJoint

Break a skeleton at a joint, deleting handles across the break. Returns the new end joint left on the old parent, then the joint.

Arguments: joint

## displayAffected

Held for scripts: whether objects affected by the selection get their special colour. The views do not draw that colour. Takes on or off; -q reads it.

Arguments: on or off

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |

## displayColor

A display colour's index: name index, -a active or dormant, -q asks, -l lists, -c creates, -rf resets.

Arguments: name [index]

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `active` | `a` | The active colour. |
| `dormant` | `d` | The dormant colour (the default). |
| `create` | `c` | Create the name. |
| `list` | `l` | List the names. |
| `queryIndex` | `qi` | Accepted for existing scripts. |
| `resetToFactory` | `rf` | Back to the factory colours. |
| `resetToSaved` | `rs` | Back to the saved colours. |
| `userDefined` | `ud` | Accepted for existing scripts. |

## displayCull

Back face culling per object, kept for scripts that ask: -bfc on or off, -q.

Arguments: objects

| Flag | Short | Description |
| --- | --- | --- |
| `backFaceCulling` | `bfc` | On or off. |
| `query` | `q` | Query. |

## displayLayer

Edit a display layer's switches (V, P, display type, colour, name); with no flags, query it. Returns its info: name, visibility, hideOnPlayback, displayType, color (RGB, null for none), colorIndex, order and members.

Arguments: layer

| Flag | Short | Description |
| --- | --- | --- |
| `visibility` | `v` | Show or hide its objects. |
| `hideOnPlayback` | `hpb` | Hide its objects while the animation plays. |
| `displayType` | `dt` | normal, template or reference. |
| `color` | `c` | Palette index for its objects' wireframe, 0 to 31; 0 is none. Turns RGB off. |
| `rgbColor` | `rgb` | An RGB wireframe colour; turns RGB on. |
| `name` | `n` | Rename the layer; made unique. |

## displayLevelOfDetail

Level of detail display, as a setting scripts can read: -lod on|off.

Arguments: on or off

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `levelOfDetail` | `lod` | On or off. |

## displayPref

Viewport display preferences, kept for scripts: each flag sets, -q reads.

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `activeObjectPivots` | `aop` | A setting. |
| `defaultFontSize` | `dfs` | A setting. |
| `displayAffected` | `da` | A setting. |
| `displayGradient` | `dgr` | A setting. |
| `fontSettingMode` | `fm` | A setting. |
| `lineWidth` | `lw` | A setting. |
| `materialLoadingMode` | `mld` | A setting. |
| `maxHardwareTextureResolution` | `mhl` | A setting. |
| `maxTextureResolution` | `mtr` | A setting. |
| `purgeExistingTextures` | `pet` | A setting. |
| `regionOfEffect` | `roe` | A setting. |
| `shadeTemplates` | `st` | A setting. |
| `smallFontSize` | `sfs` | A setting. |
| `textureDrawPixel` | `tdp` | A setting. |
| `wireframeOnShadedActive` | `wsa` | A setting. |
| `wireframeOnBackgroundFaces` | `wbf` | A setting. |
| `workspaceMode` | `wsm` | A setting. |
| `xrayActiveComponents` | `xac` | A setting. |

## displayRGBColor

A display colour's RGB: name r g b [a], -q asks, -l lists, -c creates.

Arguments: name r g b [a]

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `alpha` | `a` | With alpha. |
| `create` | `c` | Create the name. |
| `hueSaturationValue` | `hsv` | Accepted for existing scripts. |
| `list` | `l` | List the names. |
| `resetToFactory` | `rf` | Back to the factory colours. |
| `resetToSaved` | `rs` | Back to the saved colours. |

## displaySmoothness

How finely surfaces draw: -du -dv isoparms within a span, -pw points to a drawn span, -ps shaded fineness. Acts on the objects named or selected, or every surface with -all.

Arguments: objects (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `divisionsU` | `du` | Isoparms within a span along U. |
| `divisionsV` | `dv` | Isoparms within a span along V. |
| `pointsWire` | `pw` | Points to a drawn span. |
| `pointsShaded` | `ps` | Shaded fineness. |
| `polygonObject` | `po` | Meshes: 1 the cage, 2 the cage and the smoothed mesh, 3 the smoothed mesh. |
| `query` | `q` | Ask: -po answers per object. |
| `all` | `all` | Every surface. |

## displayStats

Which statistics a viewport shows, kept for scripts: each flag sets, -q reads.

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `frameRate` | `fr` | A setting. |
| `headsUpDisplay` | `hud` | A setting. |
| `polyCount` | `pc` | A setting. |
| `cameraName` | `cn` | A setting. |
| `viewAxis` | `va` | A setting. |
| `selectionDetails` | `sd` | A setting. |

## displaySurface

How a surface draws, kept for scripts: -xr X-ray, -tsl two-sided lighting, -fn flip normals. On named or selected objects; -q reads the first.

Arguments: objects

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `xRay` | `xr` | Draw see-through. |
| `twoSidedLighting` | `tsl` | Light both sides (on by default). |
| `flipNormals` | `fn` | Flip the normals drawn. |

## distanceDimension

Measure between two points: -sp, -ep, with locators there (made when none is). Returns the shape, whose .distance reads the length.

| Flag | Short | Description |
| --- | --- | --- |
| `startPoint` | `sp` | One end. |
| `endPoint` | `ep` | The other end. |

## dockControl

Script UI: dockControl, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `area` | `a` |  |
| `content` | `con` |  |
| `allowedArea` | `aa` |  |
| `label` | `l` |  |
| `floating` | `fl` |  |
| `retain` | `r` |  |
| `moveable` | `mov` |  |
| `sizeable` | `s` |  |
| `closeCommand` | `cc` |  |
| `floatChangeCommand` | `fcc` |  |
| `childArray` | `ca` |  |
| `numberOfChildren` | `nch` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## dolly

Move a camera along its view: -d how far, positive in; -abs sets the distance to its centre; -os scales an orthographic width.

Arguments: [camera]

| Flag | Short | Description |
| --- | --- | --- |
| `distance` | `d` |  |
| `absolute` | `abs` |  |
| `relative` | `rel` |  |
| `dollyTowardsCenter` | `dtc` |  |
| `orthoScale` | `os` |  |

## dollyCtx

A dollyCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `scale` | `s` |  |
| `localDolly` | `ld` |  |
| `centerOfInterestDolly` | `cd` |  |
| `boxDollyType` | `bdt` |  |
| `dollyTowardsCenter` | `dtc` |  |
| `alternateContext` | `ac` |  |
| `toolName` | `tn` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## dopeSheetEditor

Script UI: dopeSheetEditor, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `autoFit` | `af` |  |
| `autoFitTime` | `aft` |  |
| `displayActiveKeyTangents` | `dat` |  |
| `displayActiveKeys` | `dak` |  |
| `displayInfinities` | `di` |  |
| `displayKeys` | `dk` |  |
| `displayTangents` | `dtn` |  |
| `displayValues` | `dv` |  |
| `dropCommand` | `dc` |  |
| `hierarchyBelow` | `hb` |  |
| `lookAt` | `la` |  |
| `outliner` | `o` |  |
| `selectionWindow` | `sel` |  |
| `showScene` | `sc` |  |
| `showSummary` | `ss` |  |
| `showTicks` | `stk` |  |
| `snapTime` | `st` |  |
| `snapValue` | `sv` |  |
| `control` | `ctl` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `filter` | `f` |  |
| `forceMainConnection` | `fmc` |  |
| `highlightConnection` | `hlc` |  |
| `lockMainConnection` | `lck` |  |
| `mainListConnection` | `mlc` |  |
| `panel` | `pnl` |  |
| `selectionConnection` | `slc` |  |
| `stateString` | `sts` |  |
| `unParent` | `up` |  |
| `unlockMainConnection` | `ulk` |  |
| `updateMainConnection` | `upd` |  |
| `useTemplate` | `ut` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## drag

Make a drag field acting on the selected bodies, or -q/-e one. Returns [field].

Arguments: bodies to act on

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query a value. |
| `edit` | `e` | Edit an existing node. |
| `name` | `n` | Name of the new node. |
| `magnitude` | `m` | Strength. |
| `attenuation` | `att` | How fast it weakens with distance. |
| `maxDistance` | `mxd` | How far it reaches (turns the limit on). |
| `useMaxDistance` | `umd` | Limit its reach. |
| `directionX` | `dx` | Direction X. |
| `directionY` | `dy` | Direction Y. |
| `directionZ` | `dz` | Direction Z. |
| `speed` | `s` | Air: wind speed. |
| `inheritVelocity` | `iv` | Air: kept motion. |
| `frequency` | `f` | Turbulence: noise frequency. |
| `minDistance` | `mnd` | Newton: least distance. |
| `position` | `pos` | Where it is. |

## dropoffLocator

Mark a wire's dropoff at a point: dropoff percent wire.u[...]. Returns the locator. The wire's falloff stays as it is.

Arguments: dropoff percent point

## duplicate

Duplicate objects with their children, keeping each mesh's generator and modifier stack live. Returns the new top-level names. count makes a run of copies, copy i placed by the translate/rotate/scale step applied i times about the pivot: a row, a ring or a spiral in one call. mode instance shares the mesh (edits to one change all); reference shares the base mesh but gives each copy its own modifier stack.

Arguments: objects

| Flag | Short | Description |
| --- | --- | --- |
| `name` | `n` | Name for a single duplicate. |
| `count` | `c` | Copies of each object, 1 to 10000. |
| `translate` | `t` | Move from one copy to the next, cm. |
| `rotate` | `r` | Rotation from one copy to the next about the pivot, degrees (XYZ). |
| `scale` | `s` | Scale from one copy to the next about the pivot. |
| `pivot` |  | What the step rotates and scales about. |
| `pivotPoint` |  | World point rotated and scaled about when pivot is point, cm. |
| `space` |  | Axes the step is measured along: the world's or each object's own. |
| `mode` | `m` | What a duplicated mesh shares with its source: copy shares nothing. instance shares the mesh and its whole modifier stack: every copy looks the same and exports as one mesh. reference shares only the base mesh and keeps a modifier stack of its own (same shape, a different bevel). Component edits on a shared mesh reach every copy. |
| `group` | `g` | Put the copies under a new group. |
| `returnRootsOnly` | `rr` | Only the top nodes in the result. |
| `upstreamNodes` | `un` | Copy the nodes feeding it too. |
| `parentOnly` | `po` | Only the node itself, no children. |
| `inputConnections` | `ic` | The copy takes the same inputs. |
| `renameChildren` | `rc` | Give the children new unique names. |
| `smartTransform` | `st` | Repeat the last duplicate's step. |
| `transformsOnly` | `to` | Leave out the shapes. |
| `instanceLeaf` | `ilf` | Accepted for existing scripts. |

## duplicateCurve

A curve from an isoparm: duplicateCurve surface1.u[0.5] or surface1.v[1], with a curveFromSurfaceIso node. A plain curve is copied. Returns [curve1, curveFromSurfaceIso1].

Arguments: an isoparm or a curve (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `constructionHistory` | `ch` | Keep the node (on by default). |
| `local` | `l` | Follow the surface in object space, not the world. |
| `range` | `rn` | Accepted for existing scripts. |
| `relative` | `r` | Accepted for existing scripts. |
| `name` | `n` | Name of the curve. |

## duplicateModifier

Copy a modifier with its settings and insert the copy right after it in the stack. Returns the copy's name.

Arguments: modifier

## duplicateSurface

A copy of a surface or of patches of it, surface.sf[0][1]: -l keeps following it. Returns [surface, node].

Arguments: a surface or patches

| Flag | Short | Description |
| --- | --- | --- |
| `local` | `l` | Keep following it. |
| `mergeItems` | `mi` | Accepted for existing scripts. |
| `constructionHistory` | `ch` | Keep the node (on by default). |
| `name` | `n` | The new object's name. |
| `object` | `o` | Accepted for existing scripts. |
| `range` | `rn` | Accepted for existing scripts. |
| `caching` | `cch` | Accepted for existing scripts. |
| `nodeState` | `nds` | Accepted for existing scripts. |
| `tolerance` | `tol` | Accepted for existing scripts. |
| `polygon` | `po` | 0 NURBS, 1 polygons. |

## duplicateWithTransform

Duplicate the last duplicated object again, placed by how its copy now sits relative to its source. Duplicate an object, move, rotate or scale the copy, then repeat this to continue the run. Returns the new names. The step is measured from the current world matrices of the last source and copy, so it follows any later edit to the copy and survives selection changes. It is forgotten when either object is deleted.

| Flag | Short | Description |
| --- | --- | --- |
| `count` | `c` | Copies to add, 1 to 10000. |

## dynCache

Disk caches for cloth: -create (cr) solves the range and writes it, -append (ap) adds frames to the current one, -replace (rp) writes over, -delete (d) stops using it.

Arguments: cloth

| Flag | Short | Description |
| --- | --- | --- |
| `create` | `cr` | Make a cache over the range. |
| `replace` | `rp` | Write a new cache over the old. |
| `append` | `ap` | Add frames from the cache's end to now. |
| `delete` | `d` | Stop using the cache. |
| `deleteFile` | `df` | With -d: remove the file too. |
| `file` | `f` | The file (in the temporary folder by default). |
| `startTime` | `st` | First frame (the playback start). |
| `endTime` | `et` | Last frame (the playback end). |

## dynExpression

Keep a creation (-c) or runtime (-rbd, -rad) expression on particles: -s the text.

Arguments: particles

| Flag | Short | Description |
| --- | --- | --- |
| `string` | `s` | The expression. |
| `creation` | `c` | Run for each new particle. |
| `runtimeBeforeDynamics` | `rbd` | Run every frame. |
| `runtimeAfterDynamics` | `rad` | Run every frame. |
| `query` | `q` | Read it back. |

## dynInfo

What the solvers are doing: vertices out of the simulation, contacts, timings. -oos returns just the count.

Arguments: solvers or bodies

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Accepted. |
| `outOfSimulation` | `oos` | Only how many vertices are out of the simulation. |

## dynamicConstraint

Hold points of bodies: -type weld, componentToComponent, pointToSurface, tear, pin, hinge, spring, nail or pointToPoint. The last item named is the target where the type takes one.

Arguments: components or objects

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query a value. |
| `edit` | `e` | Edit an existing node. |
| `name` | `n` | Name of the new node. |
| `type` | `t` | What it holds. |
| `strength` | `s` | How stiff. |
| `restLength` | `rl` | A fixed rest length. |
| `glueStrength` | `gs` | Tear: breaks past this stretch. |
| `solver` | `sv` | The solver to join. |

## editDisplayLayerMembers

Put objects (the selection by default) in a display layer, moving them out of their old one. Naming defaultLayer takes them out of any layer. Returns the layer's members.

Arguments: layer, objects

## editKeys

Edit keys picked by curve and index: -dt/-dv move them, -t/-v set them, -itt/-ott change their tangents, -delete removes them. Keys are [[curve, index], ...].

| Flag | Short | Description |
| --- | --- | --- |
| `keys` | `k` | The keys: [[curve, index], ...]. |
| `timeOffset` | `dt` | Frames to move them by. |
| `valueOffset` | `dv` | Value to move them by. |
| `time` | `t` | Frame to put them at. |
| `value` | `v` | Value to give them. |
| `inTangentType` | `itt` | Tangent into them. |
| `outTangentType` | `ott` | Tangent out of them. |
| `breakdown` | `bd` | Make them breakdowns, or normal keys. |
| `delete` | `del` | Remove them. |

## editMetadata

Set metadata: -stn, -mn, -v or -sv, on selected components or -idx, -six and -eix. -rm removes those indices. Returns true.

Arguments: objects or components (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `channelName` | `cn` | The channel: vertex, edge, face or your own. |
| `channelType` | `cht` | Obsolete: as -cn. |
| `endIndex` | `eix` | Last index of a range. |
| `index` | `idx` | An index. |
| `indexType` | `idt` | The index's type: numeric unless given. |
| `memberName` | `mn` | The structure's member. |
| `scene` | `scn` | On the scene rather than a node. |
| `startIndex` | `six` | First index of a range. |
| `streamName` | `stn` | The stream. |
| `value` | `v` | A number. |
| `stringValue` | `sv` | A string. |
| `remove` | `rm` | Remove the indices. |

## editorTemplate

Inside an AE<type>Template procedure: lays out the Attribute Editor with -beginLayout, -addControl, -suppress, -addSeparator, -addExtraControls, -callCustom.

Arguments: [attribute]

| Flag | Short | Description |
| --- | --- | --- |
| `addAdskAssetControl` | `aac` |  |
| `addComponents` | `acm` |  |
| `addControl` | `ac` |  |
| `addDynamicControl` | `adc` |  |
| `addExtraControls` | `aec` |  |
| `addSeparator` | `as` |  |
| `annotation` | `ann` |  |
| `beginLayout` | `bl` |  |
| `beginNoOptimize` | `bno` |  |
| `beginScrollLayout` | `bsl` |  |
| `callCustom` | `ccu` |  |
| `callback` | `cb` |  |
| `collapse` | `cl` |  |
| `debugMode` | `dbm` |  |
| `dimControl` | `dc` |  |
| `endLayout` | `el` |  |
| `endNoOptimize` | `eno` |  |
| `endScrollLayout` | `esl` |  |
| `extraControlsLabel` | `ecl` |  |
| `interruptOptimize` | `io` |  |
| `label` | `l` |  |
| `listExtraAttributes` | `lea` |  |
| `preventOverride` | `po` |  |
| `queryControl` | `qc` |  |
| `queryLabel` | `ql` |  |
| `queryName` | `qn` |  |
| `removeControl` | `rc` |  |
| `suppress` | `s` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## effector

An IK effector's name and hide flag: -n, -hi, in -e and -q. hideDisplay is only held: the views do not draw effectors. Returns the effector.

Arguments: the effector (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `hide` | `hi` | Hide it when attached to a handle. |
| `name` | `n` | Its name. |

## emitter

Make an emitter feeding particles: -type omni, dir, surf, curve or vol, -r rate, -spd speed. With a mesh selected it emits off its surface. Returns [emitter, particles].

Arguments: a mesh to emit from

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query a value. |
| `edit` | `e` | Edit an existing node. |
| `name` | `n` | Name of the new node. |
| `type` | `typ` | omni, dir, surf, curve or vol. |
| `rate` | `r` | Particles per second. |
| `speed` | `spd` | Speed. |
| `speedRandom` | `srn` | How much speeds vary. |
| `spread` | `sp` | Directional: 0 to 1. |
| `directionX` | `dx` | Direction X. |
| `directionY` | `dy` | Direction Y. |
| `directionZ` | `dz` | Direction Z. |
| `position` | `pos` | Where it is. |
| `volumeShape` | `vsh` | Volume: cube or sphere. |
| `particles` | `pt` | Particles to feed (new ones by default). |
| `solver` | `sv` | The solver new particles join. |

## encodeString

A string with quotes, backslashes and control characters escaped, as MEL would need them.

Arguments: string

## env

Every environment variable, as NAME=value lines.

## evaluationManager

The evaluation mode, as a setting scripts can read: -mode off|serial|parallel|emp. It doesn't change how the graph evaluates.

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `mode` | `m` | off, serial, parallel or emp. |
| `enabled` | `en` | Whether a mode other than off is set. |
| `invalidate` | `inv` | Accepted for existing scripts. |
| `manipulation` | `man` | Accepted for existing scripts. |
| `idleBuild` | `ib` | Accepted for existing scripts. |

## exactWorldBoundingBox

World bounds of objects with what is under them: xmin ymin zmin xmax ymax zmax.

Arguments: objects

| Flag | Short | Description |
| --- | --- | --- |
| `ignoreInvisible` | `ii` | Leave out hidden shapes. |
| `calculateExactly` | `ce` | Accepted: bounds are always exact here. |

## expandedSelection

The selection and what is connected to it: -d steps (1), -et DG, EG or SG. EG and SG follow the same connections as DG: there is no separate evaluation graph here, and clusters are not reported.

| Flag | Short | Description |
| --- | --- | --- |
| `depth` | `d` | Steps away from the selection. |
| `expansionType` | `et` | DG, EG or SG. |

## exportFbx

Export to FBX (binary, 7.4): the objects named, or the selection, with what is under them. Hierarchy, meshes as polygons with normals, UV sets and materials, colour textures by path, and skins (joints, clusters, bind pose), in centimetres with the scene's up axis. Animation, blend shapes, cameras and lights are not written. -all exports every top-level object.

Arguments: path objects...

| Flag | Short | Description |
| --- | --- | --- |
| `all` | `all` | Every top-level object, not the selection. |

## exportGltf

Export to glTF 2.0: a .glb file (binary, textures inside it) or a .gltf file (JSON, textures by path). The objects named, or the selection, with what is under them: hierarchy, meshes as they stand, normals, UV sets, vertex colours, materials, colour textures and skins (joints, weights, bind matrices), in metres. The file is Y up, as the format is: a Z-up scene is converted, with no node added to carry the turn. -upAxis z writes Z up instead. Animation, blend shapes, cameras and lights are not written. -all exports every top-level object.

Arguments: path objects...

| Flag | Short | Description |
| --- | --- | --- |
| `all` | `all` | Every top-level object, not the selection. |
| `upAxis` | `up` | The file's up axis: y (the format's, the default) or z. |

## exportObj

Export meshes to an OBJ file in world space. Defaults to the selection.

Arguments: path objects...

## exportPly

Export meshes to PLY in world space, merged into one mesh, defaulting to the selection and the meshes under it: polygons as they are, normals, uv0 and the current color set per vertex (points split where those differ). Loose points and edges are written too. Returns the path.

Arguments: path objects...

| Flag | Short | Description |
| --- | --- | --- |
| `unit` |  | The file's length unit. |
| `upAxis` |  | The file's up axis; converted to the scene's. |
| `binary` |  | Binary little endian; off writes ASCII. |
| `normals` |  | Write normals. |
| `uvs` |  | Write uv0 as s, t. |
| `colors` |  | Write the current color set as 8-bit RGBA. |

## exportStl

Export meshes to STL in world space, triangulated, defaulting to the selection and the meshes under it. Binary merges them into one solid; ASCII writes a solid per mesh. Returns the path.

Arguments: path objects...

| Flag | Short | Description |
| --- | --- | --- |
| `unit` |  | The file's length unit. 3D printing assumes mm. |
| `upAxis` |  | The file's up axis; converted to the scene's. |
| `binary` |  | Binary STL; off writes ASCII. |
| `colors` |  | Binary only: the current color set as per-triangle VisCAM colors. |

## expression

Create, edit or query an expression node: expression -s "pCube1.tx = sin(time);" -o pCube1 -ae 1 -uc all -n name. Errors in the source fail the command. Returns the node's name.

Arguments: [expression node, for -q and -e]

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query. |
| `edit` | `e` | Edit. |
| `string` | `s` | The source. |
| `object` | `o` | The object bare attribute names (tx, .tx) are on. |
| `alwaysEvaluate` | `ae` | Evaluate whenever asked (1, the default). |
| `unitConversion` | `uc` | all, none or angularOnly. With none, angles read and assign in radians. |
| `name` | `n` | The node's name. |
| `shortNames` | `sn` | Accepted for existing scripts. |
| `safe` | `sf` | Accepted for existing scripts. |
| `timeDependent` | `td` | With -q: whether it reads time or frame. |

## extendCurve

Lengthen a curve along its tangent: -d distance, -s 0 end, 1 start, 2 both. Returns [curve, node].

Arguments: curves (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `distance` | `d` | How far. |
| `start` | `s` | 0 the end, 1 the start, 2 both. |
| `extensionType` | `et` | Accepted for existing scripts. |
| `constructionHistory` | `ch` | Keep a history node (on by default). |
| `replaceOriginal` | `rpo` | Change the curve itself (on by default). |
| `name` | `n` | Name of a new curve. |
| `object` | `o` | Make an object (accepted for existing scripts). |

## extendSurface

Lengthen a surface: -d distance, -ed 0 U, 1 V, 2 both, -es 0 start, 1 end, 2 both, -et 0 tangent. Returns [surface, node].

Arguments: surfaces (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `distance` | `d` | How far (1). |
| `extendDirection` | `ed` | 0 U, 1 V, 2 both. |
| `extendSide` | `es` | 0 start, 1 end, 2 both. |
| `extensionType` | `et` | Accepted: tangent lines. |
| `extendMethod` | `em` | Accepted for existing scripts. |
| `join` | `jn` | Accepted for existing scripts. |
| `replaceOriginal` | `rpo` | Change the surface itself (on by default). |
| `constructionHistory` | `ch` | Keep the node (on by default). |
| `name` | `n` | The new object's name. |
| `object` | `o` | Accepted for existing scripts. |
| `range` | `rn` | Accepted for existing scripts. |
| `caching` | `cch` | Accepted for existing scripts. |
| `nodeState` | `nds` | Accepted for existing scripts. |
| `tolerance` | `tol` | Accepted for existing scripts. |
| `polygon` | `po` | 0 NURBS, 1 polygons. |

## extrude

Sweep a profile curve along a path curve (or -d -l a direction and length): extrude profile path -et 2 -sc 1 -ro 0. Returns [extrudedSurface1, extrude1].

Arguments: the profile, then the path (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `extrudeType` | `et` | 0 distance, 1 flat, 2 tube (2). |
| `scale` | `sc` | Scale at the path's end. |
| `rotation` | `ro` | Twist at the path's end, degrees. |
| `direction` | `d` | Direction, with no path. |
| `length` | `l` | Length, with no path. |
| `divisions` | `dv` | Mesh points per span of the profile. |
| `fixedPath` | `fpt` | The surface sits on the path, not where the profile is. |
| `useComponentPivot` | `ucp` | Accepted for existing scripts. |
| `useProfileNormal` | `upn` | Accepted for existing scripts. |
| `reverseSurfaceIfPathReversed` | `rsp` | Accepted for existing scripts. |
| `polygon` | `po` | 0 NURBS (the default), 1 polygons. |
| `constructionHistory` | `ch` | Keep the node (on by default). |
| `range` | `rn` | Accepted for existing scripts. |
| `name` | `n` | Name of the new object. |

## file

Scene files: -new, -open path, -save, -rename name, -import path, -exportAll path, -exportSelected path; -type mayaAscii. In query mode -sceneName (with -shortName for the file alone), -exists, -type, -modified.

Arguments: the file

| Flag | Short | Description |
| --- | --- | --- |
| `new` | `new` | A new, empty scene. |
| `open` | `o` | Replace the scene with the file. |
| `save` | `s` | Write the scene under its name. |
| `rename` | `rn` | Set the scene's name, for the next save. |
| `import` | `i` | Add the file's contents to the scene. |
| `exportAll` | `ea` | Write the whole scene to the file. |
| `exportSelected` | `es` | Write the selection, what lies below it, its parents and the skeletons of its skins. |
| `type` | `typ` | mayaAscii, or aztek for Aztek's own format. |
| `options` | `op` | File translator options. Accepted for existing scripts. |
| `force` | `f` | Don't ask about unsaved changes. Nothing asks here. |
| `query` | `q` | Query mode. |
| `sceneName` | `sn` | Query: the scene's path; empty when untitled. |
| `shortName` | `shn` | Query: with -sceneName, the file name alone. |
| `exists` | `ex` | Query: whether the file (or the scene's file) exists. |
| `modified` | `mf` | Query: whether the scene has edits. |
| `reference` | `r` | Bring the file in as a reference under -ns. Query: the referenced files. |
| `loadReference` | `lr` | Load a reference, by its reference node (or the path argument); loaded already, it reloads. |
| `unloadReference` | `ur` | Unload a reference, by its reference node (or the path argument). |
| `removeReference` | `rr` | Remove the reference named by the path argument. |
| `referenceNode` | `rfn` | The reference node's name, or with -q ask for it. |
| `deferReference` | `dr` | Add the reference unloaded. Query: whether it is. |
| `importReference` | `ir` | Make the reference's contents part of the scene. |
| `cleanReference` | `cr` | Remove the edits of an unloaded reference, or -editCommand's kind. |
| `editCommand` | `ec` | With -cleanReference, which edits. |
| `flushReference` | `fr` | Unload, keeping the reference nodes. |
| `loadReferencePreview` | `lrp` | Loads the reference; there's no partial load here. |
| `loadReferenceDepth` | `lrd` | Accepted for scripts: all, none, topOnly, asPrefs. |
| `groupReference` | `gr` | Group the reference's top nodes under one transform. |
| `groupName` | `gn` | The group's name. |
| `groupLocator` | `gl` | Make the group a locator. |
| `sharedNodes` | `shd` | Accepted for scripts: nothing's shared yet. |
| `sharedReferenceFile` | `srf` | Accepted for scripts: nothing's shared yet. |
| `mergeNamespacesOnClash` | `mnc` | Use the namespace even if it's there already. |
| `renamingPrefix` | `rpr` | A prefix instead of a namespace. Query: what it is. |
| `lockReference` | `lck` | Lock the reference's nodes and attributes. |
| `usingNamespaces` | `uns` | Query: whether it uses a namespace. |
| `unresolvedName` | `un` | Query: the path as written. |
| `withoutCopyNumber` | `wcn` | Query: the path without its {n}. |
| `expandName` | `exn` | Query: the file's path. |
| `swapNamespace` | `sns` | Accepted for scripts. |
| `edit` | `e` | Edit mode: -namespace renames a reference's. |
| `saveReferencesUnloaded` | `sru` | Accepted for scripts. |
| `exportUnloadedReferences` | `eur` | Accepted for scripts. |
| `referenceDepthInfo` | `rdi` | Accepted for existing scripts. |
| `prompt` | `pmt` | Accepted for existing scripts. |
| `ignoreVersion` | `iv` | Accepted for existing scripts. |
| `preserveReferences` | `pr` | Accepted for existing scripts. |
| `constructionHistory` | `ch` | Accepted for existing scripts; history is baked in .ma. |
| `channels` | `chn` | Accepted for existing scripts. |
| `constraints` | `con` | Accepted for existing scripts. |
| `expressions` | `exp` | Accepted for existing scripts. |
| `shader` | `sh` | Accepted for existing scripts. |
| `returnNewNodes` | `rnn` | With a reference, return the new nodes. |
| `namespace` | `ns` | The namespace a reference goes under. |

## fileDialog

The older file browser: -m 0 read, 1 write, -dm the mask, -t the title, -dfn the default name. Returns a path, or an empty string.

| Flag | Short | Description |
| --- | --- | --- |
| `title` | `t` |  |
| `mode` | `m` |  |
| `directoryMask` | `dm` |  |
| `defaultFileName` | `dfn` |  |
| `application` | `app` |  |

## fileDialog2

The file browser. -fm 0 any file, 1 an existing file, 2 a folder, 3 a folder only, 4 several files. Returns the paths, or nothing when cancelled.

| Flag | Short | Description |
| --- | --- | --- |
| `caption` | `cap` |  |
| `fileMode` | `fm` |  |
| `fileFilter` | `ff` |  |
| `selectFileFilter` | `sff` |  |
| `startingDirectory` | `dir` |  |
| `okCaption` | `okc` |  |
| `cancelCaption` | `cc` |  |
| `dialogStyle` | `ds` |  |
| `hideNameEdit` | `hne` |  |
| `returnFilter` | `rf` |  |
| `setProjectBtnEnabled` | `spe` |  |
| `useManifest` | `um` |  |
| `fileTypeChanged` | `ftc` |  |
| `optionsUICreate` | `ocr` |  |
| `optionsUIInit` | `oin` |  |
| `optionsUICommit` | `ocm` |  |
| `optionsUICommit2` | `oc2` |  |
| `optionsUICancel` | `ocl` |  |
| `buttonBoxOrientation` | `bbo` |  |
| `selectionChanged` | `sc` |  |

## filletCurve

A fillet between two curves: -cir circular with -r radius (the default), or -fb free-form with -d, -b; -cp1, -cp2 where. Returns [curve, node].

Arguments: two curves

| Flag | Short | Description |
| --- | --- | --- |
| `radius` | `r` | The radius (1). |
| `circular` | `cir` | A circular fillet (the default). |
| `freeformBlend` | `fb` | A free-form fillet. |
| `depth` | `d` | How far a free-form fillet reaches. |
| `bias` | `b` | Which way it leans. |
| `blendControl` | `bc` | Accepted for existing scripts. |
| `curveParameter1` | `cp1` | Where on the first curve. |
| `curveParameter2` | `cp2` | Where on the second curve. |
| `join` | `jn` | Trim the curves back too. |
| `trim` | `t` | Trim the curves back. |
| `constructionHistory` | `ch` | Keep the node (on by default). |
| `name` | `n` | The new object's name. |
| `object` | `o` | Accepted for existing scripts. |
| `range` | `rn` | Accepted for existing scripts. |
| `caching` | `cch` | Accepted for existing scripts. |
| `nodeState` | `nds` | Accepted for existing scripts. |
| `tolerance` | `tol` | Accepted for existing scripts. |
| `polygon` | `po` | 0 NURBS, 1 polygons. |

## filterCurve

Filter curves: euler (default: rotations without jumps), butterworth (-cof, -sr), gaussian (-wd), keyReducer (-pre percent), keySync (keys where any curve has one), peakRemover (-tol), resample (-per seconds), simplify (-tol, -tto). -stt/-edt limit it in time, -sk to the selected keys. Returns how many curves it changed.

Arguments: curves, objects or plugs (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `filter` | `f` | The filter (euler). |
| `attribute` | `at` | Channel name(s). |
| `cutoffFrequency` | `cof` | Butterworth: cutoff in Hz (7). |
| `samplingRate` | `sr` | Butterworth and gaussian: samples per second, at most one a frame. |
| `width` | `wd` | Gaussian: width in frames (1). |
| `period` | `per` | Resample: seconds between keys (1/30). |
| `keepKeysOnFrame` | `kof` | Resample: keep new keys on whole frames. |
| `precision` | `pre` | Key reducer: percent of the curve's range it may move (5). |
| `tolerance` | `tol` | Peak remover and simplify: value tolerance. |
| `timeTolerance` | `tto` | Simplify: time tolerance. |
| `startTime` | `stt` | Start of the part filtered. |
| `endTime` | `edt` | End of the part filtered. |
| `selectedKeys` | `sk` | Only between the selected keys. |
| `minTimeStep` | `mns` | Accepted for existing scripts. |
| `maxTimeStep` | `mxs` | Accepted for existing scripts. |
| `positionTolerance` | `pot` | Accepted for existing scripts. |
| `preserveKeyTangent` | `pkt` | Accepted for existing scripts. |

## filterExpand

The selection (or arguments) filtered by selection mask and flattened: -sm 31 vertices, 32 edges, 34 faces, 35 UVs, 70 vertex faces, 28 CVs, 12 meshes, 9 curves, 22 locators. -ex 0 keeps ranges. -fp full paths. None when nothing matches.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `expand` | `ex` | See the command reference. |
| `fullPath` | `fp` | See the command reference. |
| `selectionMask` | `sm` | See the command reference. |
| `symActive` | `sma` | See the command reference. |
| `symNegative` | `smn` | See the command reference. |
| `symPositive` | `smp` | See the command reference. |
| `symSeam` | `sms` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## findComponents

Ids of a mesh's vertices, edges or faces matching every predicate given (normal direction, side of the symmetry plane, box, distance, extreme along an axis, open border). Ids index the editable cage, like selectComponents. Changes nothing; see selectComponentsWhere to select them.

Arguments: [mesh]

| Flag | Short | Description |
| --- | --- | --- |
| `kind` | `k` | vertex, edge or face. |
| `normal` | `n` | Keep components whose normal is within angle of this direction (world space). |
| `angle` |  | Tolerance for normal=, degrees. |
| `side` | `s` | '+x', '-x', '+y', ... keeps components whose centre is on that side of the object-space origin plane; 'center' (or 'centerx', 'centery', 'centerz') keeps those on the plane. |
| `box` | `b` | [[minX, minY, minZ], [maxX, maxY, maxZ]], world space: keep components whose centre is inside. None leaves that side open, so [[None, 20, None], [None, None, None]] is everything above y=20. |
| `direction` | `dir` | Edges only: keep edges running along this world direction (either way) within angle; (0, 1, 0) finds vertical edges, whose ring polyInsertLoop cuts with a horizontal loop. |
| `near` |  | World point; with radius= keeps components within it, else keeps the count nearest. |
| `radius` | `r` | Distance for near=, cm. |
| `extreme` | `x` | '+y', '-z', ...: keep the count components furthest along that world direction (the top face is extreme='+y'). |
| `count` | `c` | How many to keep for extreme= or near= without radius. |
| `border` |  | True keeps only components on an open border, False only interior ones. |
| `shell` |  | Keep components of one connected shell: 0 is the largest, numbered as meshCheck lists them. |
| `fromSelection` | `fs` | Only consider the currently selected components of that kind. |
| `tolerance` | `t` | Distance counted as on a plane for side=, cm (default scales with the mesh). |

## findDeformers

The deformers on a mesh, nearest the shape first.

Arguments: objects

## findKeyframe

The frame of the next, previous, first or last key on the objects (the selection when none).

Arguments: objects

| Flag | Short | Description |
| --- | --- | --- |
| `which` | `w` | next, previous, first or last. |
| `time` | `t` | Search from this frame instead of the current one. |

## findRelatedSkinCluster

The skinCluster on a mesh or its transform, or an empty string.

Arguments: a mesh or its transform

## findType

Nodes of a type upstream of the objects (-fwd downstream), the nearest unless -d. -e exact type.

Arguments: objects

| Flag | Short | Description |
| --- | --- | --- |
| `type` | `t` | The type. |
| `deep` | `d` | Every one, not just the nearest. |
| `exact` | `e` | This type exactly. |
| `forward` | `fwd` | Look downstream. |

## fitBspline

A cubic through the curve's CVs. Returns [curve, node].

Arguments: curves (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `tolerance` | `tol` | Accepted for existing scripts. |
| `constructionHistory` | `ch` | Keep a history node (on by default). |
| `replaceOriginal` | `rpo` | Change the curve itself (on by default). |
| `name` | `n` | Name of a new curve. |
| `object` | `o` | Make an object (accepted for existing scripts). |

## flexor

A deformer at joints or bones: -typ jointLattice (the default), boneLattice or sculpt, -aj or -ab, -ts for the whole skeleton. Deforms what the joints are bound to, and any meshes named. -q -l lists the types. Returns the new nodes. Less than the reference: of -dc only the lattice's -dv and the sculpt's -mxd are read; a sculpt flexor follows its joint but no attribute drives it; the lattice sits after the skin.

Arguments: joints, and any extra meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `atBones` | `ab` | At bones. |
| `atJoints` | `aj` | At joints (the default). |
| `deformerCommand` | `dc` | The deformer's command text: -dv and -mxd are read from it. |
| `list` | `l` | With -q: the types. |
| `name` | `n` | Obsolete in the reference; has no effect. |
| `noScale` | `ns` | Do not size the flexor to the geometry nearby. |
| `toSkeleton` | `ts` | Every joint below those selected. |
| `type` | `typ` | Which flexor. |

## floatField

Script UI: floatField, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `precision` | `pre` |  |
| `showTrailingZeros` | `tze` |  |
| `value` | `v` |  |
| `minValue` | `min` |  |
| `maxValue` | `max` |  |
| `step` | `s` |  |
| `changeCommand` | `cc` |  |
| `dragCommand` | `dc` |  |
| `enterCommand` | `ec` |  |
| `receiveFocusCommand` | `rfc` |  |
| `editable` | `ed` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## floatFieldGrp

Script UI: floatFieldGrp, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `precision` | `pre` |  |
| `showTrailingZeros` | `tze` |  |
| `step` | `s` |  |
| `label` | `l` |  |
| `numberOfFields` | `nf` |  |
| `value` | `v` |  |
| `value1` | `v1` |  |
| `value2` | `v2` |  |
| `value3` | `v3` |  |
| `value4` | `v4` |  |
| `extraLabel` | `el` |  |
| `changeCommand` | `cc` |  |
| `changeCommand1` | `cc1` |  |
| `changeCommand2` | `cc2` |  |
| `changeCommand3` | `cc3` |  |
| `changeCommand4` | `cc4` |  |
| `dragCommand` | `dc` |  |
| `dragCommand1` | `dc1` |  |
| `dragCommand2` | `dc2` |  |
| `dragCommand3` | `dc3` |  |
| `dragCommand4` | `dc4` |  |
| `enable1` | `en1` |  |
| `enable2` | `en2` |  |
| `enable3` | `en3` |  |
| `enable4` | `en4` |  |
| `columnWidth1` | `cw1` |  |
| `columnAlign1` | `cl1` |  |
| `columnAttach1` | `ct1` |  |
| `columnOffset1` | `co1` |  |
| `columnWidth2` | `cw2` |  |
| `columnAlign2` | `cl2` |  |
| `columnAttach2` | `ct2` |  |
| `columnOffset2` | `co2` |  |
| `adjustableColumn2` | `ad2` |  |
| `columnWidth3` | `cw3` |  |
| `columnAlign3` | `cl3` |  |
| `columnAttach3` | `ct3` |  |
| `columnOffset3` | `co3` |  |
| `adjustableColumn3` | `ad3` |  |
| `columnWidth4` | `cw4` |  |
| `columnAlign4` | `cl4` |  |
| `columnAttach4` | `ct4` |  |
| `columnOffset4` | `co4` |  |
| `adjustableColumn4` | `ad4` |  |
| `columnWidth5` | `cw5` |  |
| `columnAlign5` | `cl5` |  |
| `columnAttach5` | `ct5` |  |
| `columnOffset5` | `co5` |  |
| `adjustableColumn5` | `ad5` |  |
| `columnWidth6` | `cw6` |  |
| `columnAlign6` | `cl6` |  |
| `columnAttach6` | `ct6` |  |
| `columnOffset6` | `co6` |  |
| `adjustableColumn6` | `ad6` |  |
| `adjustableColumn` | `adj` |  |
| `columnWidth` | `cw` |  |
| `columnAlign` | `cal` |  |
| `columnAttach` | `cat` |  |
| `rowAttach` | `rat` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## floatScrollBar

Script UI: floatScrollBar, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` |  |
| `minValue` | `min` |  |
| `maxValue` | `max` |  |
| `step` | `s` |  |
| `horizontal` | `hr` |  |
| `changeCommand` | `cc` |  |
| `dragCommand` | `dc` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## floatSlider

Script UI: floatSlider, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` |  |
| `minValue` | `min` |  |
| `maxValue` | `max` |  |
| `step` | `s` |  |
| `horizontal` | `hr` |  |
| `changeCommand` | `cc` |  |
| `dragCommand` | `dc` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## floatSliderButtonGrp

Script UI: floatSliderButtonGrp, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `precision` | `pre` |  |
| `showTrailingZeros` | `tze` |  |
| `buttonLabel` | `bl` |  |
| `buttonCommand` | `bc` |  |
| `symbolButtonCommand` | `sbc` |  |
| `symbolButtonDisplay` | `sbd` |  |
| `image` | `i` |  |
| `label` | `l` |  |
| `field` | `f` |  |
| `value` | `v` |  |
| `minValue` | `min` |  |
| `maxValue` | `max` |  |
| `fieldMinValue` | `fmn` |  |
| `fieldMaxValue` | `fmx` |  |
| `sliderStep` | `ss` |  |
| `step` | `s` |  |
| `fieldStep` | `fs` |  |
| `extraLabel` | `el` |  |
| `changeCommand` | `cc` |  |
| `dragCommand` | `dc` |  |
| `columnWidth1` | `cw1` |  |
| `columnAlign1` | `cl1` |  |
| `columnAttach1` | `ct1` |  |
| `columnOffset1` | `co1` |  |
| `columnWidth2` | `cw2` |  |
| `columnAlign2` | `cl2` |  |
| `columnAttach2` | `ct2` |  |
| `columnOffset2` | `co2` |  |
| `adjustableColumn2` | `ad2` |  |
| `columnWidth3` | `cw3` |  |
| `columnAlign3` | `cl3` |  |
| `columnAttach3` | `ct3` |  |
| `columnOffset3` | `co3` |  |
| `adjustableColumn3` | `ad3` |  |
| `columnWidth4` | `cw4` |  |
| `columnAlign4` | `cl4` |  |
| `columnAttach4` | `ct4` |  |
| `columnOffset4` | `co4` |  |
| `adjustableColumn4` | `ad4` |  |
| `columnWidth5` | `cw5` |  |
| `columnAlign5` | `cl5` |  |
| `columnAttach5` | `ct5` |  |
| `columnOffset5` | `co5` |  |
| `adjustableColumn5` | `ad5` |  |
| `columnWidth6` | `cw6` |  |
| `columnAlign6` | `cl6` |  |
| `columnAttach6` | `ct6` |  |
| `columnOffset6` | `co6` |  |
| `adjustableColumn6` | `ad6` |  |
| `adjustableColumn` | `adj` |  |
| `columnWidth` | `cw` |  |
| `columnAlign` | `cal` |  |
| `columnAttach` | `cat` |  |
| `rowAttach` | `rat` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## floatSliderGrp

Script UI: floatSliderGrp, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `precision` | `pre` |  |
| `showTrailingZeros` | `tze` |  |
| `label` | `l` |  |
| `field` | `f` |  |
| `value` | `v` |  |
| `minValue` | `min` |  |
| `maxValue` | `max` |  |
| `fieldMinValue` | `fmn` |  |
| `fieldMaxValue` | `fmx` |  |
| `sliderStep` | `ss` |  |
| `step` | `s` |  |
| `fieldStep` | `fs` |  |
| `extraLabel` | `el` |  |
| `changeCommand` | `cc` |  |
| `dragCommand` | `dc` |  |
| `columnWidth1` | `cw1` |  |
| `columnAlign1` | `cl1` |  |
| `columnAttach1` | `ct1` |  |
| `columnOffset1` | `co1` |  |
| `columnWidth2` | `cw2` |  |
| `columnAlign2` | `cl2` |  |
| `columnAttach2` | `ct2` |  |
| `columnOffset2` | `co2` |  |
| `adjustableColumn2` | `ad2` |  |
| `columnWidth3` | `cw3` |  |
| `columnAlign3` | `cl3` |  |
| `columnAttach3` | `ct3` |  |
| `columnOffset3` | `co3` |  |
| `adjustableColumn3` | `ad3` |  |
| `columnWidth4` | `cw4` |  |
| `columnAlign4` | `cl4` |  |
| `columnAttach4` | `ct4` |  |
| `columnOffset4` | `co4` |  |
| `adjustableColumn4` | `ad4` |  |
| `columnWidth5` | `cw5` |  |
| `columnAlign5` | `cl5` |  |
| `columnAttach5` | `ct5` |  |
| `columnOffset5` | `co5` |  |
| `adjustableColumn5` | `ad5` |  |
| `columnWidth6` | `cw6` |  |
| `columnAlign6` | `cl6` |  |
| `columnAttach6` | `ct6` |  |
| `columnOffset6` | `co6` |  |
| `adjustableColumn6` | `ad6` |  |
| `adjustableColumn` | `adj` |  |
| `columnWidth` | `cw` |  |
| `columnAlign` | `cal` |  |
| `columnAttach` | `cat` |  |
| `rowAttach` | `rat` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## flow

Deform an object on a motion path through a lattice that travels with it. Needs the lattice deformer.

Arguments: the object (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `divisions` | `dv` | Lattice divisions. |
| `localCompute` | `lc` | Accepted for existing scripts. |
| `localDivisions` | `ld` | Accepted for existing scripts. |
| `objectCentered` | `oc` | Accepted for existing scripts. |

## flowLayout

Script UI: flowLayout, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `vertical` | `v` |  |
| `wrap` | `wr` |  |
| `columnSpacing` | `cs` |  |
| `childArray` | `ca` |  |
| `numberOfChildren` | `nch` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## flushIdleQueue

Runs what waits for idle now: scriptJobs and deferred scripts.

## flushUndo

Clear undo history. which='past' drops the steps undo would revert, so undo cannot step back past this point; 'future' drops the redo steps; 'all' drops both.

| Flag | Short | Description |
| --- | --- | --- |
| `which` | `w` | past, future or all. |

## formLayout

Script UI: formLayout, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `numberOfDivisions` | `nd` |  |
| `attachForm` | `af` |  |
| `attachControl` | `ac` |  |
| `attachPosition` | `ap` |  |
| `attachNone` | `an` |  |
| `attachOppositeForm` | `aof` |  |
| `attachOppositeControl` | `aoc` |  |
| `childArray` | `ca` |  |
| `numberOfChildren` | `nch` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## frameLayout

Script UI: frameLayout, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `label` | `l` |  |
| `collapsable` | `cll` |  |
| `collapse` | `cl` |  |
| `marginHeight` | `mh` |  |
| `marginWidth` | `mw` |  |
| `borderVisible` | `bv` |  |
| `borderStyle` | `bs` |  |
| `labelVisible` | `lv` |  |
| `labelIndent` | `li` |  |
| `labelWidth` | `lw` |  |
| `labelAlign` | `la` |  |
| `font` | `fn` |  |
| `collapseCommand` | `cc` |  |
| `expandCommand` | `ec` |  |
| `preCollapseCommand` | `pcc` |  |
| `preExpandCommand` | `pec` |  |
| `childArray` | `ca` |  |
| `numberOfChildren` | `nch` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## freeFormFillet

A blend between two edges (surface.u[x] or surface.v[x]) leaving each along its surface: -d depth, -bb bias. Returns [surface, node].

Arguments: two edges

| Flag | Short | Description |
| --- | --- | --- |
| `depth` | `d` | How far the tangents reach (0.5). |
| `bias` | `bb` | Which way it leans. |
| `positionTolerance` | `pt` | Accepted for existing scripts. |
| `tangentTolerance` | `tt` | Accepted for existing scripts. |
| `constructionHistory` | `ch` | Keep the node (on by default). |
| `name` | `n` | The new object's name. |
| `object` | `o` | Accepted for existing scripts. |
| `range` | `rn` | Accepted for existing scripts. |
| `caching` | `cch` | Accepted for existing scripts. |
| `nodeState` | `nds` | Accepted for existing scripts. |
| `tolerance` | `tol` | Accepted for existing scripts. |
| `polygon` | `po` | 0 NURBS, 1 polygons. |

## freezeTransforms

Bake translate/rotate/scale into mesh or curve points, leaving identity values. World positions don't change. With hierarchy (the default, like the reference) everything parented under the objects is frozen too; without it the children's transforms absorb what their parent gives up. Joints keep their translate and take their rotation into jointOrient, or with jointOrient line up with the world.

Arguments: objects

| Flag | Short | Description |
| --- | --- | --- |
| `normal` | `n` | Accepted for existing scripts. |
| `preserveNormals` | `pn` | Accepted for existing scripts. |
| `hierarchy` |  | Freeze everything parented under the objects too. |
| `jointOrient` |  | Also line joints' axes up with the world, zeroing their orientation. |
| `apply` | `a` | On: bake into the geometry. Off, when given: back to identity, the object moving. |
| `translate` | `t` | Freeze translation. |
| `rotate` | `r` | Freeze rotation. |
| `scale` | `s` | Freeze scale. |

## geomBind

Recomputes a skinCluster's weights following the surface: -bm 3 (the geodesic voxel method, which runs the heat bind here), -mi, -fo.

Arguments: skinCluster or skinned mesh

| Flag | Short | Description |
| --- | --- | --- |
| `bindMethod` | `bm` | 3 geodesic voxel; 2 heat map; 0 and 1 closest distance and hierarchy. |
| `falloff` | `fo` | 0 to 1, how far influences reach. |
| `maxInfluences` | `mi` | Most influences on a point. |
| `geodesicVoxelParams` | `gvp` | Accepted for existing scripts. |
| `postVoxelSmooth` | `pvs` | Accepted for existing scripts. |

## geometryConstraint

Constrain the object named (or selected) last to the targets before it; a second call adds targets. -mo keeps the current offset; -w sets the targets' weight; -e and -q work on the constraint or the object it drives. Returns [constraint].

Arguments: targets, then the object to drive or the constraint

| Flag | Short | Description |
| --- | --- | --- |
| `maintainOffset` | `mo` | Keep the object where it is now relative to the targets. |
| `weight` | `w` | Weight of the targets named (1). |
| `name` | `n` | Name of the constraint. |
| `offset` | `o` | Added to the result. |
| `remove` | `rm` | Remove the targets named. Removing the last deletes the constraint. |
| `targetList` | `tl` | Query: the targets. |
| `weightAliasList` | `wal` | Query: the targets' weight attributes. |
| `aimVector` | `aim` | The local axis that points at the target. |
| `upVector` | `u` | The local axis that points up. |
| `worldUpVector` | `wu` | The world direction up means. |
| `worldUpType` | `wut` | scene, object, objectrotation, vector or none. |
| `worldUpObject` | `wuo` | The object for worldUpType object and objectrotation. |
| `interpType` | `int` | How rotations blend: noflip, average, shortest, longest, cache, or 0 to 4. |
| `skipTranslate` | `st` | Translate axes to leave alone: x, y, z or none. May be used more than once. |
| `skipRotate` | `sr` | Rotate axes to leave alone: x, y, z or none. May be used more than once. |
| `skip` | `sk` | Axes of this constraint's channel to leave alone: x, y, z or none. May be used more than once. |
| `skipScale` |  | Scale axes to leave alone: x, y, z or none. |
| `edit` | `e` | Change the constraint. |
| `query` | `q` | Ask about the constraint. |

## getAttr

Value of 'node.attr'; a whole array gives its elements' values in index order. lock=True returns whether it is locked instead; multiIndices=True an array's element indices.

Arguments: plug

| Flag | Short | Description |
| --- | --- | --- |
| `time` | `t` | The value at this frame. |
| `type` | `typ` | The attribute's type name. |
| `keyable` | `k` | Whether it is keyable. |
| `channelBox` | `cb` | Whether it shows in the channel box without being keyable. |
| `settable` | `se` | Whether setAttr would take: not locked, not driven, an input. |
| `size` | `s` | The element count of a multi attribute; 1 otherwise. |
| `silent` | `sl` | Accepted for existing scripts. |
| `asString` | `as` | An enum's label. |
| `expandEnvironmentVariables` | `x` | Accepted for existing scripts. |
| `lock` | `l` | Whether it is locked. |
| `multiIndices` | `mi` | The element indices of a multi attribute. |

## getClassification

A node type's classification strings: utility/general, and so on.

Arguments: type

| Flag | Short | Description |
| --- | --- | --- |
| `satisfies` | `sat` | Whether the type's classification satisfies this. |

## getFileList

File names in -fld matching -fs, sorted.

| Flag | Short | Description |
| --- | --- | --- |
| `filespec` | `fs` | A wildcard. |
| `folder` | `fld` | The directory. |

## getLastError

The text of the last error reported.

## getMetadata

Read metadata values, flattened in index order: -stn, -mn, -cn, indices as for editMetadata. -lsn, -lcn, -lmn list names, -dt a member's type.

Arguments: objects or components (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `channelName` | `cn` | The channel: vertex, edge, face or your own. |
| `channelType` | `cht` | Obsolete: as -cn. |
| `endIndex` | `eix` | Last index of a range. |
| `index` | `idx` | An index. |
| `indexType` | `idt` | The index's type: numeric unless given. |
| `memberName` | `mn` | The structure's member. |
| `scene` | `scn` | On the scene rather than a node. |
| `startIndex` | `six` | First index of a range. |
| `streamName` | `stn` | The stream. |
| `dataType` | `dt` | The member's type. |
| `listChannelNames` | `lcn` | The channels. |
| `listMemberNames` | `lmn` | The members. |
| `listStreamNames` | `lsn` | The streams. |

## getModifiers

Which modifier keys are down, as bits: none, when a script asks.

## getModulePath

Where a module lives: -mn name, along AZTEK_MODULE_PATH and MAYA_MODULE_PATH.

| Flag | Short | Description |
| --- | --- | --- |
| `moduleName` | `mn` | The module. |

## getPanel

The panels: -wf in focus, -up under the pointer, -wl by label, -typ of a type, -to the type of one, -vis visible, -all.

| Flag | Short | Description |
| --- | --- | --- |
| `withLabel` | `wl` |  |
| `type` | `typ` |  |
| `typeOf` | `to` |  |
| `scriptType` | `sty` |  |
| `containing` | `con` |  |
| `configWithLabel` | `cwl` |  |
| `withFocus` | `wf` |  |
| `underPointer` | `up` |  |
| `allPanels` | `all` |  |
| `visiblePanels` | `vis` |  |
| `invisiblePanels` | `inv` |  |
| `allTypes` | `at` |  |
| `allScriptedTypes` | `ast` |  |
| `allConfigs` | `ac` |  |
| `atPosition` | `ap` |  |

## getParticleAttr

Per particle values: -at position -a true gives every one; without -a, the mean then the spread.

Arguments: particles or particle1.pt[0:3]

| Flag | Short | Description |
| --- | --- | --- |
| `attribute` | `at` | position, velocity, rgbPP, opacityPP, lifespanPP, age, radiusPP, particleId. |
| `array` | `a` | Every value. |

## getenv

An environment variable's value, or "".

Arguments: name

## ghosting

Ghosting state: -act ghost, unghost or unghostAll on the selection, the options as defaults, -e to change objects (-ago all ghosted), -q an option, -q -go the ghosted objects, -q -f their ghost frames. Less than the reference: the views draw no ghosts, so -en, the colours, the opacities and -ud are only held.

Arguments: objects (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `action` | `act` | ghost, unghost or unghostAll. |
| `allGhostedObjects` | `ago` | With -e: every ghosted object. |
| `allInRange` | `a` | Keyframes mode uses the whole playback range. Held only. |
| `customFrames` | `cf` | The custom frames; -9999999 clears them. |
| `enable` | `en` | Ghosts on the whole scene. Held only. |
| `farOpacity` | `fo` | Opacity of the farthest ghost. Held only. |
| `frames` | `f` | With -q: the ghost frames. |
| `geometryFilter` | `gf` | Leave geometry out. |
| `ghostedObjects` | `go` | With -q: the ghosted objects. |
| `ghostsStep` | `gs` | Frames or keys between ghosts. |
| `hierarchy` | `h` | Walk down hierarchies. |
| `jointFilter` | `jf` | Leave joints out. |
| `locatorFilter` | `lf` | Leave locators out. |
| `mode` | `m` | preAndPost, pre, post, custom or keyframes. |
| `nearOpacity` | `no` | Opacity of the nearest ghost. Held only. |
| `postColor` | `poc` | Colour after the current time. Held only. |
| `postFrames` | `pof` | Ghosts after the current time. |
| `preColor` | `prc` | Colour before the current time. Held only. |
| `preFrames` | `prf` | Ghosts before the current time. |
| `preset` | `p` | 1s, 2s, 4s, 5s, 10s or Custom. |
| `resetAll` | `r` | Back to the defaults, everything unghosted. |
| `useDriver` | `ud` | Use the driver's frames. Held only. |

## globalStitch

Stitch a whole set of surfaces: every pair of edges within -ms of each other. Returns the surfaces and nodes.

Arguments: surfaces

| Flag | Short | Description |
| --- | --- | --- |
| `maxSeparation` | `ms` | How far apart edges may be (0.1). |
| `lockSurface` | `lk` | Accepted for existing scripts. |
| `modificationResistance` | `mr` | Accepted for existing scripts. |
| `sampling` | `sam` | Accepted for existing scripts. |
| `stitchCorners` | `sc` | Accepted for existing scripts. |
| `stitchEdges` | `se` | Accepted for existing scripts. |
| `stitchPartialEdges` | `spe` | Accepted for existing scripts. |
| `stitchSmoothness` | `ss` | Accepted for existing scripts. |
| `constructionHistory` | `ch` | Keep the node (on by default). |
| `name` | `n` | The new object's name. |
| `object` | `o` | Accepted for existing scripts. |
| `range` | `rn` | Accepted for existing scripts. |
| `caching` | `cch` | Accepted for existing scripts. |
| `nodeState` | `nds` | Accepted for existing scripts. |
| `tolerance` | `tol` | Accepted for existing scripts. |
| `polygon` | `po` | 0 NURBS, 1 polygons. |

## graphDollyCtx

A graphDollyCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## graphSelectContext

A graphSelectContext tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## graphTrackCtx

A graphTrackCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## gravity

Make a gravity field acting on the selected bodies, or -q/-e one. Returns [field].

Arguments: bodies to act on

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query a value. |
| `edit` | `e` | Edit an existing node. |
| `name` | `n` | Name of the new node. |
| `magnitude` | `m` | Strength. |
| `attenuation` | `att` | How fast it weakens with distance. |
| `maxDistance` | `mxd` | How far it reaches (turns the limit on). |
| `useMaxDistance` | `umd` | Limit its reach. |
| `directionX` | `dx` | Direction X. |
| `directionY` | `dy` | Direction Y. |
| `directionZ` | `dz` | Direction Z. |
| `speed` | `s` | Air: wind speed. |
| `inheritVelocity` | `iv` | Air: kept motion. |
| `frequency` | `f` | Turbulence: noise frequency. |
| `minDistance` | `mnd` | Newton: least distance. |
| `position` | `pos` | Where it is. |

## grid

The ground plane: -s size, -sp spacing between major lines, -d divisions between them, -tgl on or off, -r back to the defaults, -df the defaults themselves. These reach the four model panels: a line every spacing / divisions, reaching -s each way. The major lines stay darker every 5 lines whatever -d says, and the axes, division line and label flags are only held. -q reads a flag.

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `default` | `df` | Set or read the defaults. |
| `reset` | `r` | Back to the defaults. |
| `style` | `st` | Obsolete in the reference; has no effect. |
| `displayAxes` | `da` | A grid setting. |
| `displayAxesBold` | `dab` | A grid setting. |
| `displayDivisionLines` | `ddl` | A grid setting. |
| `displayGridLines` | `dgl` | A grid setting. |
| `displayOrthographicLabels` | `dol` | A grid setting. |
| `displayPerspectiveLabels` | `dpl` | A grid setting. |
| `divisions` | `d` | A grid setting. |
| `orthographicLabelPosition` | `olp` | A grid setting. |
| `perspectiveLabelPosition` | `plp` | A grid setting. |
| `size` | `s` | A grid setting. |
| `spacing` | `sp` | A grid setting. |
| `toggle` | `tgl` | A grid setting. |

## gridLayout

Script UI: gridLayout, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `numberOfColumns` | `nc` |  |
| `numberOfRows` | `nr` |  |
| `numberOfRowsColumns` | `nrc` |  |
| `cellWidth` | `cw` |  |
| `cellHeight` | `ch` |  |
| `cellWidthHeight` | `cwh` |  |
| `autoGrow` | `ag` |  |
| `allowEmptyCells` | `aec` |  |
| `columnsResizable` | `cr` |  |
| `position` | `pos` |  |
| `gridOrder` | `go` |  |
| `childArray` | `ca` |  |
| `numberOfChildren` | `nch` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## group

Group objects under a new transform, or create an empty group with empty=True.

Arguments: objects

| Flag | Short | Description |
| --- | --- | --- |
| `name` | `n` | Group name. |
| `empty` | `em` | Create an empty group. |
| `parent` | `p` | Parent for the new group. |
| `world` | `w` | Under the world, not the members' shared parent. |
| `absolute` | `a` | Keep world placement (the default). |
| `relative` | `r` | Keep local placement. |
| `useAsGroup` | `uag` | Accepted for existing scripts. |

## growSelection

Grow the component selection by one ring of neighbours: points to the points of their faces, edges and faces to those sharing a point with the selection.

| Flag | Short | Description |
| --- | --- | --- |
| `alongLoop` |  | Selected edges reach one further along their loops. |

## hardenPointCurve

Make a curve pass through CVs by raising the knot multiplicity: curve.cv[i], -m how many (-1 up to the degree). Returns [curve, node].

Arguments: CVs (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `multiplicity` | `m` | How many knots, -1 as many as the degree. |
| `constructionHistory` | `ch` | Keep a history node (on by default). |
| `replaceOriginal` | `rpo` | Change the curve itself (on by default). |
| `name` | `n` | Name of a new curve. |
| `object` | `o` | Make an object (accepted for existing scripts). |

## haroldSwatch

A shading node as a small picture rendered by Harold: a material on a ball, a texture or utility flat over the UV square. Writes a .ppm file and returns its path, size and the node's signature; -signature alone gives the signature, which changes when the node or anything connected into it does.

Arguments: node

| Flag | Short | Description |
| --- | --- | --- |
| `size` | `s` | Pixels a side; 128 without it. |
| `file` | `f` | Where to write the picture; a temporary file without it. |
| `signature` | `sig` | Only the signature. |

## headsUpDisplay

Text in the views: -s section -b block -l label -c command; -rem removes; -ex tests; -lh lists.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `section` | `s` | Screen section, 0 to 9. |
| `block` | `b` | Row in the section. |
| `label` | `l` | Label text. |
| `command` | `c` | MEL giving the value. |
| `visible` | `vis` | Show it. |
| `remove` | `rem` | Remove it. |
| `exists` | `ex` | Whether it exists. |
| `listHeadsUpDisplays` | `lh` | List them. |

## headsUpMessage

A short message over the view. -time how long it stays.

Arguments: message

| Flag | Short | Description |
| --- | --- | --- |
| `object` | `o` |  |
| `selection` | `s` |  |
| `time` | `t` |  |
| `uvTextureEditor` | `uve` |  |
| `verticalOffset` | `vo` |  |
| `horizontalOffset` | `hoo` |  |
| `viewport` | `vp` |  |

## help

A command's description and flags, from what it registered: help name; -so only the synopsis; -l a regular expression, listing the commands that match. -doc fails, since no documentation ships. The popup and rollover settings are only held.

Arguments: a command, or a pattern with -l

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `documentation` | `doc` | Not available: fails. |
| `language` | `lng` | mel or python, with -doc. |
| `list` | `l` | List the commands matching. |
| `popupDisplayTime` | `pdt` | Held only. |
| `popupMode` | `pm` | Held only. |
| `popupPauseTime` | `ppt` | Held only. |
| `popupSimpleMode` | `psm` | Held only. |
| `rolloverMode` | `rm` | Held only. |
| `syntaxOnly` | `so` | Only the synopsis. |

## hide

Hide objects (visibility off). Defaults to the selection.

Arguments: objects

| Flag | Short | Description |
| --- | --- | --- |
| `clearSelection` | `cs` | Accepted for existing scripts. |
| `returnHidden` | `rh` | Return what was hidden. |
| `invertComponents` | `ic` | Accepted for existing scripts. |
| `testVisibility` | `tv` | Whether they are hidden, changing nothing. |

## hikBake

As hikBakeToSkeleton.

Arguments: the character

| Flag | Short | Description |
| --- | --- | --- |
| `time` | `t` | The range: "1:100" (the playback range by default). |
| `sampleBy` | `sb` | Frames between keys (1). |
| `eulerFilter` | `ef` | Run the Euler filter on the rotations (on by default). |

## hikBakeToControlRig

Bake what drives a character onto a new control rig's keys, source None after.

Arguments: the character

| Flag | Short | Description |
| --- | --- | --- |
| `time` | `t` | The range: "1:100" (the playback range by default). |
| `sampleBy` | `sb` | Frames between keys (1). |
| `eulerFilter` | `ef` | Run the Euler filter on the rotations (on by default). |

## hikBakeToSkeleton

Bake what drives a character onto its joints, and leave them free of it.

Arguments: the character

| Flag | Short | Description |
| --- | --- | --- |
| `time` | `t` | The range: "1:100" (the playback range by default). |
| `sampleBy` | `sb` | Frames between keys (1). |
| `eulerFilter` | `ef` | Run the Euler filter on the rotations (on by default). |

## hikCharacterLock

Lock a character's definition, taking its T-pose from where the joints are: character, on, validate. A definition with a required slot empty won't lock when validating.

Arguments: character, on, validate

## hikControlRigSelectionChangedCallback

Refreshes the HumanIK window; here it follows the scene by itself.

Arguments: ignored

## hikCreateCharacter

Define a character. Joints named (or selected, with everything below them) go to slots by their names; setCharacterObject sets others. Returns the character.

Arguments: the new character's name, then joints

| Flag | Short | Description |
| --- | --- | --- |
| `name` | `n` | The character's name. |

## hikCreateControlRig

Build a control rig on a character (the current one when none is named): FK hips, spine, neck and head, IK hands and feet with pole vectors. Returns the controls.

Arguments: the character

## hikCreateDefinition

A new empty definition, Character1.

## hikCreateSkeleton

Make a standard humanoid skeleton in T-pose, joints named Character1_Hips and so on: -n the name, -h height, -sc spine joints, -fg fingers, -ch a character definition with it. Returns the joints.

| Flag | Short | Description |
| --- | --- | --- |
| `name` | `n` | Name, and the joints' prefix. |
| `height` | `h` | Height (170). |
| `spineCount` | `sc` | Spine joints (3). |
| `fingers` | `fg` | Fingers too. |
| `character` | `ch` | Make the character definition too. |

## hikDefinitionStatus

A character's definition checked: per slot green, yellow or red, the messages, and the summary.

Arguments: character

## hikDeleteControlRig

Remove a character's control rig; the joints stay where they are.

Arguments: the character

## hikEnableCharacter

Turn a character's solve on or off: character, on. Off lets the joints go and on puts back the last source given.

Arguments: character, on

## hikGetCurrentCharacter

The character the HumanIK window is on.

## hikGetCurrentSource

What drives the current character: None, Stance, Control Rig, or a character.

## hikGetDependNodes

The nodes a character depends on: its joints, then what is connected to it. The reference does not document this command, so what it returns is inferred from the name.

Arguments: a character

## hikGetNodeCount

How many slots a definition has.

## hikGetNodeIdFromName

The id of a slot by its name, -1 if there is none.

Arguments: name

## hikGetSkNode

The joint in a character's slot, by id or name; with only the character, every slot and joint in turn.

Arguments: character, slot

## hikGetSkeletonNodes

The joints of a character's definition.

Arguments: character

## hikGetSkeletonNodesList

The joints of a character's definition.

Arguments: character

## hikGlobals

Held for scripts: -rap, the release all pinning flag. It has no effect, since the full body solver is not here. Returns the state.

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `releaseAllPinning` | `rap` | Ignore pinning states. |

## hikInfo

What the HumanIK window draws, as JSON.

## hikIsCharacterizationLocked

Whether a character's definition is locked.

Arguments: character

## hikIsDefinitionLocked

Whether a character's definition is locked.

Arguments: character

## hikLoadTemplate

Map a character's slots from an .xml template of slot and joint names: file, character. -p a namespace or prefix joints carry. Returns how many.

Arguments: file, character

| Flag | Short | Description |
| --- | --- | --- |
| `prefix` | `p` | What joint names carry before the template's. |

## hikMirrorMatching

Fill empty slots from their opposites: by name with the side changed, else by mirrored place. Returns how many.

Arguments: character

## hikNameMatch

Map the joints under the named or selected roots to a character's slots by name: -c the character, -p a prefix to ignore such as mixamorig:. Returns how many.

Arguments: root joints

| Flag | Short | Description |
| --- | --- | --- |
| `character` | `c` | The character. |
| `prefix` | `p` | A prefix to ignore. |

## hikRemoveCharacterObject

Empty a character's slot: character, slot.

Arguments: character, slot

## hikSaveTemplate

Write a character's mapping to an .xml template: file, character.

Arguments: file, character

## hikSelectDefinitionTab

Refreshes the HumanIK window; here it follows the scene by itself.

Arguments: ignored

## hikSetCharacterObject

Put a joint in a character's slot, as setCharacterObject(joint, character, slot, 0); the slot by id or name, the last 1 to take it out.

Arguments: joint, character, slot, 0

## hikSetCurrentCharacter

Make a character the HumanIK window's. Returns it.

Arguments: character

## hikSetCurrentSource

Set what drives the current character: None, Stance, Control Rig, or a character.

Arguments: source

## hikSetCurrentSourceFromCharacter

hikSetCurrentSource by another name scripts use.

Arguments: source

## hikSetSource

Drive one character from another: hikSetSource target source; with no source, stops. Returns how many slots.

Arguments: target, source

## hikStraightenArms

For a rig bound with its arms down: writes a T-pose into the definition by turning the arms' stored stance out along X, the joints left where they are. Not a procedure of the reference's, where the rig is posed instead. Returns how many slots changed.

Arguments: the character

## hikToggleLockDefinition

Lock or unlock the current character's definition.

## hikUpdateCharacterList

Refreshes the HumanIK window; here it follows the scene by itself.

Arguments: ignored

## hikUpdateContextualUI

Refreshes the HumanIK window; here it follows the scene by itself.

Arguments: ignored

## hikUpdateCurrentCharacterFromUI

Refreshes the HumanIK window; here it follows the scene by itself.

Arguments: ignored

## hikUpdateCurrentSourceFromUI

Refreshes the HumanIK window; here it follows the scene by itself.

Arguments: ignored

## hikUpdateCustomRigUI

Refreshes the HumanIK window; here it follows the scene by itself.

Arguments: ignored

## hikUpdateDefinitionUI

Refreshes the HumanIK window; here it follows the scene by itself.

Arguments: ignored

## hikUpdateSkeletonUI

Refreshes the HumanIK window; here it follows the scene by itself.

Arguments: ignored

## hikUpdateSourceList

Refreshes the HumanIK window; here it follows the scene by itself.

Arguments: ignored

## hikUpdateTool

Refreshes the HumanIK window; here it follows the scene by itself.

Arguments: ignored

## hilite

Hilite objects for component work: named, -r to replace, -u to take away, -tgl.

Arguments: objects

| Flag | Short | Description |
| --- | --- | --- |
| `replace` | `r` | Replace what is hilited. |
| `toggle` | `tgl` | Toggle. |
| `unHilite` | `u` | Take away. |

## hitTest

A stub outside the UI: returns nothing, since panels and their cameras live there.

Arguments: control x y

## hotkey

Bind a key: -k the key, -alt -ctl -sht modifiers, -n the name command for press, -rn for release.

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Ask what the key is bound to: -n for press, -rn for release. |
| `keyShortcut` | `k` | The key. |
| `altModifier` | `alt` | With Alt. |
| `ctrlModifier` | `ctl` | With Ctrl. |
| `shiftModifier` | `sht` | With Shift. |
| `name` | `n` | The name command for press. |
| `releaseName` | `rn` | The name command for release. |

## hotkeyCheck

The annotation of the nameCommand a key is bound to, or an empty string: -k the key, -alt, -ctl, -cmd, -opt modifiers, -kup for release. Less than the reference: it sees keys bound with the hotkey command, not the built-in table the shell keeps.

| Flag | Short | Description |
| --- | --- | --- |
| `altModifier` | `alt` | With Alt. |
| `commandModifier` | `cmd` | With Command: as Ctrl here. |
| `ctrlModifier` | `ctl` | With Ctrl. |
| `isRepeatable` | `ir` | Accepted for existing scripts; has no effect. |
| `keyString` | `k` | The key. |
| `keyUp` | `kup` | On release. |
| `optionModifier` | `opt` | With Option: as Alt here. |

## hrender

Renders with Harold: the current frame, a sequence (-seq), every frame the settings name (-batch), or interactively (-ipr start).

Arguments: camera

| Flag | Short | Description |
| --- | --- | --- |
| `camera` | `cam` | Camera to render through. |
| `width` | `w` | Image width. |
| `height` | `h` | Image height. |
| `region` | `reg` | x0 y0 x1 y1, pixels from the top left. |
| `seq` | `seq` | Frames: "1 10", "1 10 2" or one frame. |
| `batch` | `b` | Render the frames the render settings name and write them. |
| `ipr` | `ipr` | start, stop, refresh or pause interactive rendering. |
| `async` | `as` | Start the current frame and return at once; the render view shows it as it refines. |

## hudButton

A control in the heads up display: -section, -block, its label, value and commands. headsUpDisplay -rem removes it.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `label` | `l` |  |
| `labelFontSize` | `lfs` |  |
| `buttonWidth` | `bw` |  |
| `buttonShape` | `bsh` |  |
| `pressCommand` | `pc` |  |
| `releaseCommand` | `rc` |  |
| `section` | `s` |  |
| `block` | `b` |  |
| `blockSize` | `bs` |  |
| `blockAlignment` | `ba` |  |
| `padding` | `p` |  |
| `visible` | `vis` |  |
| `allowOverlap` | `ao` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## hudSlider

A control in the heads up display: -section, -block, its label, value and commands. headsUpDisplay -rem removes it.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `label` | `l` |  |
| `labelFontSize` | `lfs` |  |
| `labelWidth` | `lw` |  |
| `type` | `t` |  |
| `value` | `v` |  |
| `minValue` | `min` |  |
| `maxValue` | `max` |  |
| `valueAlignment` | `va` |  |
| `valueFontSize` | `vfs` |  |
| `valueWidth` | `vw` |  |
| `sliderLength` | `sl` |  |
| `sliderIncrement` | `si` |  |
| `decimalPrecision` | `dp` |  |
| `internalPadding` | `ip` |  |
| `showFrame` | `sf` |  |
| `dragCommand` | `dc` |  |
| `pressCommand` | `pc` |  |
| `releaseCommand` | `rc` |  |
| `section` | `s` |  |
| `block` | `b` |  |
| `blockSize` | `bs` |  |
| `blockAlignment` | `ba` |  |
| `padding` | `p` |  |
| `visible` | `vis` |  |
| `allowOverlap` | `ao` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## hudSliderButton

A control in the heads up display: -section, -block, its label, value and commands. headsUpDisplay -rem removes it.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `sliderLabel` | `sl` |  |
| `sliderLabelFontSize` | `slf` |  |
| `sliderLabelWidth` | `slw` |  |
| `type` | `t` |  |
| `value` | `v` |  |
| `minValue` | `min` |  |
| `maxValue` | `max` |  |
| `valueAlignment` | `va` |  |
| `valueFontSize` | `vfs` |  |
| `valueWidth` | `vw` |  |
| `sliderLength` | `sln` |  |
| `sliderIncrement` | `si` |  |
| `decimalPrecision` | `dp` |  |
| `internalPadding` | `ip` |  |
| `sliderDragCommand` | `sdc` |  |
| `sliderPressCommand` | `spc` |  |
| `sliderReleaseCommand` | `src` |  |
| `buttonLabel` | `bl` |  |
| `buttonLabelFontSize` | `blf` |  |
| `buttonWidth` | `bw` |  |
| `buttonShape` | `bsh` |  |
| `buttonPressCommand` | `bpc` |  |
| `buttonReleaseCommand` | `brc` |  |
| `section` | `s` |  |
| `block` | `b` |  |
| `blockSize` | `bs` |  |
| `blockAlignment` | `ba` |  |
| `padding` | `p` |  |
| `visible` | `vis` |  |
| `allowOverlap` | `ao` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## hyperShade

Materials and what wears them: -assign gives the selection (objects or faces) a material, making its shading group if need be; -objects selects what wears one; -smn selects the materials of the selection; -lun and -ldn list a network's nodes.

Arguments: objects, for -assign

| Flag | Short | Description |
| --- | --- | --- |
| `assign` | `a` | The material to give the selection. |
| `objects` | `o` | The material whose objects to select; empty for the selected one. |
| `shaderNetworksSelectMaterialNodes` | `smn` | Select the materials of the selected objects. |
| `listUpstreamNodes` | `lun` | The nodes upstream of this one. |
| `listDownstreamNodes` | `ldn` | The nodes downstream of this one. |
| `listUpstreamShaderNodes` | `lus` | The shading nodes upstream of this one. |
| `listDownstreamShaderNodes` | `lds` | The shading nodes downstream of this one. |
| `shaderNetwork` | `snw` | Select the networks of the selected materials. |
| `duplicate` | `d` | Copy the selected materials with what feeds them. |
| `resetGraphPosition` | `rgp` | Accepted: the Node Editor lays out for itself. |
| `resetSwatch` | `rs` | Accepted: swatches redraw on change. |
| `incremental` | `inc` | Accepted for existing scripts. |
| `clearWorkArea` | `cwa` | Accepted: the Node Editor has its own work area. |
| `networks` | `n` | Accepted for existing scripts. |

## iconTextButton

Script UI: iconTextButton, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `style` | `st` |  |
| `image` | `i` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `label` | `l` |  |
| `command` | `c` |  |
| `doubleClickCommand` | `dcc` |  |
| `sourceType` | `stp` |  |
| `imageOverlayLabel` | `iol` |  |
| `flat` | `fla` |  |
| `disabledImage` | `di` |  |
| `align` | `al` |  |
| `marginWidth` | `mw` |  |
| `marginHeight` | `mh` |  |
| `font` | `fn` |  |
| `labelOffset` | `lo` |  |
| `rotation` | `rot` |  |
| `selectionImage` | `si` |  |
| `highlightImage` | `hi` |  |
| `overlayLabelColor` | `olc` |  |
| `overlayLabelBackColor` | `olb` |  |
| `scaleIcon` | `sic` |  |
| `commandRepeatable` | `rpt` |  |
| `ltVersion` | `lt` |  |
| `version` | `ver` |  |
| `useAlpha` | `ua` |  |
| `flexibleWidthType` | `fwt` |  |
| `flexibleWidthValue` | `fwv` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## iconTextCheckBox

Script UI: iconTextCheckBox, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `style` | `st` |  |
| `image` | `i` |  |
| `image1` | `i1` |  |
| `label` | `l` |  |
| `value` | `v` |  |
| `onCommand` | `onc` |  |
| `offCommand` | `ofc` |  |
| `changeCommand` | `cc` |  |
| `flat` | `fla` |  |
| `font` | `fn` |  |
| `align` | `al` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## iconTextRadioButton

Script UI: iconTextRadioButton, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `style` | `st` |  |
| `image` | `i` |  |
| `image1` | `i1` |  |
| `label` | `l` |  |
| `select` | `sl` |  |
| `collection` | `cl` |  |
| `onCommand` | `onc` |  |
| `offCommand` | `ofc` |  |
| `changeCommand` | `cc` |  |
| `flat` | `fla` |  |
| `font` | `fn` |  |
| `align` | `al` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## ikHandle

Create an IK handle from -sj to -ee (or two selected joints), or query or edit one. The RP solver bends the chain toward the pole vector; SC aims it. Returns [handle, effector].

Arguments: start and end joints (the selection when -sj/-ee are not given), or the handle to query or edit

| Flag | Short | Description |
| --- | --- | --- |
| `curve` | `c` | Spline IK: the curve to follow. Queried, the curve. |
| `createCurve` | `ccv` | Spline IK: make a curve through the joints (the default). |
| `parentCurve` | `pcv` | Accepted for existing scripts: the curve drives in world space. |
| `rootOnCurve` | `roc` | Spline IK: the start joint goes to the curve's start (on by default). |
| `simplifyCurve` | `scv` | Accepted for existing scripts. |
| `rootTwistMode` | `rtm` | Accepted for existing scripts. |
| `twistType` | `tws` | Accepted for existing scripts (linear). |
| `startJoint` | `sj` | First joint of the chain. |
| `endEffector` | `ee` | Last joint of the chain. Queried, the effector. |
| `solver` | `sol` | ikRPsolver (default), ikSCsolver or a solver node's name. |
| `name` | `n` | Name of the handle. |
| `priority` | `p` | Solving order among overlapping handles. |
| `weight` | `w` | The handle's weight. |
| `positionWeight` | `pw` | Position against orientation. |
| `twist` | `twi` | Turn about the handle line, degrees. |
| `roll` | `rl` | Turn of the chain's start, degrees. |
| `offset` | `ofs` | Slide along the curve (spline IK). |
| `autoPriority` | `ap` | Set the priority from the chain's depth in the hierarchy. |
| `createCurve` | `ccv` | Spline IK: create a curve. Accepted for the solver to come. |
| `parentCurve` | `pcv` | Spline IK: parent the curve. Accepted for the solver to come. |
| `snapCurve` | `snc` | Spline IK: snap the curve. Accepted for the solver to come. |
| `numSpans` | `ns` | Spline IK: spans of the curve. |
| `disableHandles` | `dh` | Set the handles to full FK: ikBlend 0. |
| `enableHandles` | `eh` | Set the handles to full IK: ikBlend 1. |
| `jointList` | `jl` | Query: the joints the handle drives. |
| `snapHandleFlagToggle` | `shf` | The handle follows the end joint while IK is off. |
| `snapHandleToEffector` | `see` | Put the handle at the end joint now. |
| `sticky` | `s` | off or sticky. |
| `forceSolver` | `fs` | Run the solver every time. |
| `exists` | `ex` | Whether a handle of this name exists. |
| `edit` | `e` | Change a handle. |
| `query` | `q` | Ask about a handle. |

## ikHandleCtx

A ikHandleCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `autoPriorityH` | `apH` |  |
| `createCurve` | `ccv` |  |
| `createRootAxis` | `cra` |  |
| `forceSolverH` | `fsH` |  |
| `ikFKSwitching` | `ikf` |  |
| `numSpans` | `ns` |  |
| `parentCurve` | `pcv` |  |
| `poWeightH` | `pwH` |  |
| `priorityH` | `pH` |  |
| `rootOnCurve` | `roc` |  |
| `rootTwistMode` | `rtm` |  |
| `simplifyCurve` | `scv` |  |
| `snapCurve` | `snc` |  |
| `snapHandleH` | `snH` |  |
| `solverTypeH` | `stH` |  |
| `stickyH` | `sH` |  |
| `twistType` | `tws` |  |
| `weightH` | `wH` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## ikHandleDisplayScale

How big IK handles draw in the views: the scale, -q reads it.

Arguments: the scale

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |

## ikSolver

Create a solver node handles can name with -sol. Returns its name.

Arguments: solver (edit and query)

| Flag | Short | Description |
| --- | --- | --- |
| `solverType` | `st` | ikRPsolver, ikSCsolver or ikSplineSolver. |
| `name` | `n` | Name of the solver. |
| `epsilon` | `ep` | How close counts as reached. |
| `maxIterations` | `mxi` | Most iterations a solve may take. |
| `edit` | `e` | Change a solver. |
| `query` | `q` | Ask about a solver. |

## ikSplineHandleCtx

A ikSplineHandleCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `autoPriorityH` | `apH` |  |
| `createCurve` | `ccv` |  |
| `createRootAxis` | `cra` |  |
| `forceSolverH` | `fsH` |  |
| `numSpans` | `ns` |  |
| `parentCurve` | `pcv` |  |
| `poWeightH` | `pwH` |  |
| `priorityH` | `pH` |  |
| `rootOnCurve` | `roc` |  |
| `rootTwistMode` | `rtm` |  |
| `simplifyCurve` | `scv` |  |
| `snapCurve` | `snc` |  |
| `snapHandleH` | `snH` |  |
| `solverTypeH` | `stH` |  |
| `stickyH` | `sH` |  |
| `twistType` | `tws` |  |
| `weightH` | `wH` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## ikSystem

Scene wide IK: -e -sol 0 stops every handle solving, -e -sn 0 stops handles snapping to their effectors.

| Flag | Short | Description |
| --- | --- | --- |
| `solve` | `sol` | Whether any IK solves. |
| `snap` | `sn` | Whether handles snap to their effectors. |
| `listSolvers` | `ls` | Query: the solver nodes. |
| `edit` | `e` | Change the settings. |
| `query` | `q` | Ask about the settings. |

## ikSystemInfo

The ikSystem's snap switch: -gsh off means no handle snaps to its effector while IK is off. -q reads it.

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `globalSnapHandle` | `gsh` | Whether handles snap. |

## ikfkDisplayMethod

Held for scripts: how IK/FK blending shows on a selected handle, -d none, ik, fk or ikfk. The views do not draw it. -q -d reads it.

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `display` | `d` | none, ik, fk or ikfk. |

## image

Script UI: image, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `image` | `i` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## importFbx

Import a binary FBX file as the reference's FBX plug-in does, by the FBXImport settings: the model hierarchy with each node's channels (pre rotation as a joint's orient), meshes as polygons with UV sets, colour sets, normals and hard edges, limb nodes as joints, lambert and phong materials with their file textures (place2dTexture each, normal maps through a bump2d), skins with the file's weights and bind matrices, and the bind pose. The file's axes and unit turn and scale its top nodes. Textures are looked for where the file says, beside it and in its .fbm folder; textures inside the file are written to the .fbm folder. Not read: ASCII FBX, animation, blend shapes, cameras and lights (their nodes come in as transforms). Returns the top nodes made.

Arguments: path

## importGltf

Import a glTF 2.0 file (.gltf or .glb): its node hierarchy, triangle meshes with their first UV set, and materials as standard surfaces. Lengths come in as centimetres and the file's Y up becomes the scene's up axis. Skin weights, animation, textures, cameras and lights are not read. Returns the top nodes made.

Arguments: path

## importObj

Import an OBJ file; one mesh per object. Returns the new mesh names.

Arguments: path

## importPly

Import a PLY file (ASCII or binary, either byte order) as one mesh: polygons with normals, UVs (per vertex or MeshLab's per-corner texcoord), vertex colors as colorSet and face colors as faceColor. Other numeric vertex and face properties (quality, intensity, confidence...) become float attributes of the same name. A file with only vertices comes in as a point cloud: a mesh of loose points. Returns the new mesh names, or with report=True a dict of nodes, pointCloud, textureFiles and skipped.

Arguments: path

| Flag | Short | Description |
| --- | --- | --- |
| `unit` |  | The file's length unit. Scanners and photogrammetry write metres. |
| `upAxis` |  | The file's up axis; converted to the scene's. |
| `normals` |  | Keep the file's normals. |
| `report` |  | Return the import report instead of just the names. |

## importStl

Import an STL file, binary or ASCII: one mesh per solid. Triangles sharing positions weld into a connected mesh, edges bent past hardAngle are hard, and binary per-triangle colors (VisCAM and Materialise) become a color set. Returns the new mesh names.

Arguments: path

| Flag | Short | Description |
| --- | --- | --- |
| `unit` |  | The file's length unit. STL has none; 3D printing assumes mm. |
| `upAxis` |  | The file's up axis; converted to the scene's. |
| `weld` |  | Merge identical positions into shared points. |
| `hardAngle` |  | Edges bent more than this many degrees are hard; -1 leaves all soft. |

## inViewMessage

A message that fades in the view: -amg assist message, -msg, -pos, -fade.

| Flag | Short | Description |
| --- | --- | --- |
| `message` | `msg` |  |
| `assistMessage` | `amg` |  |
| `statusMessage` | `smg` |  |
| `position` | `pos` |  |
| `fadeInTime` | `fit` |  |
| `fadeOutTime` | `fot` |  |
| `fadeStayTime` | `fst` |  |
| `alpha` | `a` |  |
| `backColor` | `bkc` |  |
| `textAlpha` | `ta` |  |
| `textOffset` | `to` |  |
| `frameOffset` | `fo` |  |
| `font` | `fn` |  |
| `clear` | `clr` |  |
| `fade` | `f` |  |
| `dragKill` | `dk` |  |
| `minimize` | `min` |  |
| `restore` | `rst` |  |
| `uvEditor` | `uv` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## inheritTransform

Whether objects follow their parent's transform: toggles, -on sets, -p keeps them where they are; -q reads each.

Arguments: objects

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `toggle` | `tgl` | Toggle (what it does anyway). |
| `on` | `on` | Set on or off. |
| `preserve` | `p` | Keep the world position. |

## insertJoint

Add a joint under the named or selected joint, at its place; its child joints move under the new one. Returns the new joint.

Arguments: joint

## insertJointCtx

A insertJointCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## insertKeyCtx

A insertKeyCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `breakdown` | `bd` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## insertKnotCurve

Add knots at -p: curve keeps its shape. Returns [curve, node].

Arguments: curves (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `parameter` | `p` | Where. |
| `numberOfKnots` | `nk` | How many. |
| `addKnots` | `add` | Accepted for existing scripts. |
| `insertBetween` | `ib` | Accepted for existing scripts. |
| `constructionHistory` | `ch` | Keep a history node (on by default). |
| `replaceOriginal` | `rpo` | Change the curve itself (on by default). |
| `name` | `n` | Name of a new curve. |
| `object` | `o` | Make an object (accepted for existing scripts). |

## insertKnotSurface

Add knots at -p (repeat it) along -d, 1 U or 0 V: the shape stays. -nk how many. Returns [surface, node].

Arguments: surfaces (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `parameter` | `p` | Where. Repeat it. |
| `direction` | `d` | 1 U, 0 V (1). |
| `numberOfKnots` | `nk` | How many (1). |
| `addKnots` | `add` | Add them (on), or bring the multiplicity up to the number. |
| `insertBetween` | `ib` | Accepted for existing scripts. |
| `constructionHistory` | `ch` | Keep a history node (on by default). |
| `replaceOriginal` | `rpo` | Change the surface itself (on by default). |
| `name` | `n` | Name of a new surface. |
| `object` | `o` | Accepted for existing scripts. |
| `polygon` | `po` | Accepted for existing scripts. |

## instance

Make instances of objects: meshes that share their source's mesh, so an edit to any of them changes all. Returns the new names. reference=True makes references instead, which share the base mesh under the source's modifiers but keep a modifier stack of their own (same base, a different bevel).

Arguments: objects

| Flag | Short | Description |
| --- | --- | --- |
| `name` | `n` | Name for a single instance. |
| `reference` |  | Share only the base mesh and keep a modifier stack of its own (same shape, a different bevel). Off, instances share the modifier stack too, so every copy looks the same and exports as one mesh. |
| `leaf` | `lf` | Accepted for existing scripts. |
| `smartTransform` | `st` | Accepted for existing scripts. |

## instanceInfo

How a mesh shares data: {kind: none | source | instance | reference, source, members}.

Arguments: mesh

## instanceable

Whether nodes may be instanced: -a false makes instance refuse them, -r their children too, -s their shapes too. Returns the states, and -q reads them. duplicate -ilf is not checked.

Arguments: nodes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `allow` | `a` | Whether they can be instanced. |
| `recursive` | `r` | And the non-shape children. |
| `shape` | `s` | And the shape children. |

## intField

Script UI: intField, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` |  |
| `minValue` | `min` |  |
| `maxValue` | `max` |  |
| `step` | `s` |  |
| `changeCommand` | `cc` |  |
| `dragCommand` | `dc` |  |
| `enterCommand` | `ec` |  |
| `receiveFocusCommand` | `rfc` |  |
| `editable` | `ed` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## intFieldGrp

Script UI: intFieldGrp, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `label` | `l` |  |
| `numberOfFields` | `nf` |  |
| `value` | `v` |  |
| `value1` | `v1` |  |
| `value2` | `v2` |  |
| `value3` | `v3` |  |
| `value4` | `v4` |  |
| `extraLabel` | `el` |  |
| `changeCommand` | `cc` |  |
| `changeCommand1` | `cc1` |  |
| `changeCommand2` | `cc2` |  |
| `changeCommand3` | `cc3` |  |
| `changeCommand4` | `cc4` |  |
| `dragCommand` | `dc` |  |
| `dragCommand1` | `dc1` |  |
| `dragCommand2` | `dc2` |  |
| `dragCommand3` | `dc3` |  |
| `dragCommand4` | `dc4` |  |
| `enable1` | `en1` |  |
| `enable2` | `en2` |  |
| `enable3` | `en3` |  |
| `enable4` | `en4` |  |
| `columnWidth1` | `cw1` |  |
| `columnAlign1` | `cl1` |  |
| `columnAttach1` | `ct1` |  |
| `columnOffset1` | `co1` |  |
| `columnWidth2` | `cw2` |  |
| `columnAlign2` | `cl2` |  |
| `columnAttach2` | `ct2` |  |
| `columnOffset2` | `co2` |  |
| `adjustableColumn2` | `ad2` |  |
| `columnWidth3` | `cw3` |  |
| `columnAlign3` | `cl3` |  |
| `columnAttach3` | `ct3` |  |
| `columnOffset3` | `co3` |  |
| `adjustableColumn3` | `ad3` |  |
| `columnWidth4` | `cw4` |  |
| `columnAlign4` | `cl4` |  |
| `columnAttach4` | `ct4` |  |
| `columnOffset4` | `co4` |  |
| `adjustableColumn4` | `ad4` |  |
| `columnWidth5` | `cw5` |  |
| `columnAlign5` | `cl5` |  |
| `columnAttach5` | `ct5` |  |
| `columnOffset5` | `co5` |  |
| `adjustableColumn5` | `ad5` |  |
| `columnWidth6` | `cw6` |  |
| `columnAlign6` | `cl6` |  |
| `columnAttach6` | `ct6` |  |
| `columnOffset6` | `co6` |  |
| `adjustableColumn6` | `ad6` |  |
| `adjustableColumn` | `adj` |  |
| `columnWidth` | `cw` |  |
| `columnAlign` | `cal` |  |
| `columnAttach` | `cat` |  |
| `rowAttach` | `rat` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## intScrollBar

Script UI: intScrollBar, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` |  |
| `minValue` | `min` |  |
| `maxValue` | `max` |  |
| `step` | `s` |  |
| `horizontal` | `hr` |  |
| `changeCommand` | `cc` |  |
| `dragCommand` | `dc` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## intSlider

Script UI: intSlider, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` |  |
| `minValue` | `min` |  |
| `maxValue` | `max` |  |
| `step` | `s` |  |
| `horizontal` | `hr` |  |
| `changeCommand` | `cc` |  |
| `dragCommand` | `dc` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## intSliderGrp

Script UI: intSliderGrp, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `label` | `l` |  |
| `field` | `f` |  |
| `value` | `v` |  |
| `minValue` | `min` |  |
| `maxValue` | `max` |  |
| `fieldMinValue` | `fmn` |  |
| `fieldMaxValue` | `fmx` |  |
| `sliderStep` | `ss` |  |
| `step` | `s` |  |
| `fieldStep` | `fs` |  |
| `extraLabel` | `el` |  |
| `changeCommand` | `cc` |  |
| `dragCommand` | `dc` |  |
| `columnWidth1` | `cw1` |  |
| `columnAlign1` | `cl1` |  |
| `columnAttach1` | `ct1` |  |
| `columnOffset1` | `co1` |  |
| `columnWidth2` | `cw2` |  |
| `columnAlign2` | `cl2` |  |
| `columnAttach2` | `ct2` |  |
| `columnOffset2` | `co2` |  |
| `adjustableColumn2` | `ad2` |  |
| `columnWidth3` | `cw3` |  |
| `columnAlign3` | `cl3` |  |
| `columnAttach3` | `ct3` |  |
| `columnOffset3` | `co3` |  |
| `adjustableColumn3` | `ad3` |  |
| `columnWidth4` | `cw4` |  |
| `columnAlign4` | `cl4` |  |
| `columnAttach4` | `ct4` |  |
| `columnOffset4` | `co4` |  |
| `adjustableColumn4` | `ad4` |  |
| `columnWidth5` | `cw5` |  |
| `columnAlign5` | `cl5` |  |
| `columnAttach5` | `ct5` |  |
| `columnOffset5` | `co5` |  |
| `adjustableColumn5` | `ad5` |  |
| `columnWidth6` | `cw6` |  |
| `columnAlign6` | `cl6` |  |
| `columnAttach6` | `ct6` |  |
| `columnOffset6` | `co6` |  |
| `adjustableColumn6` | `ad6` |  |
| `adjustableColumn` | `adj` |  |
| `columnWidth` | `cw` |  |
| `columnAlign` | `cal` |  |
| `columnAttach` | `cat` |  |
| `rowAttach` | `rat` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## internalVar

The user folder's places, each ending in a slash: -userAppDir, -userScriptDir, -userPrefDir, -userShelfDir and the rest.

| Flag | Short | Description |
| --- | --- | --- |
| `userAppDir` | `uad` | The user folder. |
| `userScriptDir` | `usd` | Its scripts. |
| `userPrefDir` | `upd` | Its preferences. |
| `userShelfDir` | `ush` | Its shelves. |
| `userMarkingMenuDir` | `umm` | Its marking menus. |
| `userBitmapsDir` | `ubd` | Its icons. |
| `userHotkeyDir` | `uhk` | Its hotkeys. |
| `userPresetsDir` | `ups` | Its presets. |
| `userWorkspaceDir` | `uwd` | Its projects. |
| `userTmpDir` | `utd` | The temporary folder. |
| `mayaInstallDir` | `mid` | Where the application is installed. |

## intersect

Curves where two surfaces cross, sampled from their tessellation: one object per run, -cos on the first surface. Returns [curves..., node].

Arguments: two surfaces

| Flag | Short | Description |
| --- | --- | --- |
| `curveOnSurface` | `cos` | As curves on the first surface. |
| `firstSurface` | `fs` | Accepted for existing scripts. |
| `constructionHistory` | `ch` | Keep the node (on by default). |
| `name` | `n` | The new object's name. |
| `object` | `o` | Accepted for existing scripts. |
| `range` | `rn` | Accepted for existing scripts. |
| `caching` | `cch` | Accepted for existing scripts. |
| `nodeState` | `nds` | Accepted for existing scripts. |
| `tolerance` | `tol` | Accepted for existing scripts. |
| `polygon` | `po` | 0 NURBS, 1 polygons. |

## invertSelection

Invert the selection: the unselected components of the current kind on the hilited meshes, or in object mode every unselected object. An isolated view inverts only what it shows.

## invertShape

invertShape deformed corrected: a new mesh, the shape which as a target in front of the deformers gives the corrected mesh at this pose. Returns its transform.

Arguments: the deformed mesh, then the corrected mesh

## isConnected

isConnected(src, dst): whether src drives dst.

Arguments: src dst

| Flag | Short | Description |
| --- | --- | --- |
| `ignoreUnitConversion` | `iuc` | Accepted: no conversion nodes are inserted here. |

## isDirty

Whether a plug's node needs evaluating: -c through connections, -d the datablock.

Arguments: plug

| Flag | Short | Description |
| --- | --- | --- |
| `connection` | `c` | Accepted: dirt is per node here. |
| `datablock` | `d` | Accepted: dirt is per node here. |

## isTrue

Whether a named condition holds, as scriptJob -conditionTrue reads them.

Arguments: condition

## isolateSelect

Isolate Select on a panel: -state on or off, -addSelected, -removeSelected, -update. -q -state reads it.

Arguments: panel

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` |  |
| `state` | `s` |  |
| `addSelected` | `as` |  |
| `removeSelected` | `rs` |  |
| `update` | `u` |  |
| `loadSelected` | `ls` |  |
| `addDagObject` | `ado` |  |
| `removeDagObject` | `rdo` |  |
| `viewObjects` | `vo` |  |

## itemFilter

A named test of nodes for editors and lsThroughFilter: -byName, -byType, -union, -intersect, -difference, -negate.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `byName` | `bn` |  |
| `byType` | `bt` |  |
| `byBin` | `bk` |  |
| `byScript` | `bs` |  |
| `secondScript` | `ss` |  |
| `classification` | `cls` |  |
| `clearByBin` | `cbk` |  |
| `clearByType` | `cbt` |  |
| `category` | `cat` |  |
| `customCategory` | `ctg` |  |
| `difference` | `di` |  |
| `intersect` | `in` |  |
| `union` | `un` |  |
| `negate` | `neg` |  |
| `listBuiltInFilters` | `lbf` |  |
| `listOtherFilters` | `lof` |  |
| `listUserFilters` | `luf` |  |
| `parent` | `p` |  |
| `text` | `t` |  |
| `uniqueNodeNames` | `unn` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## jiggle

Jiggle: points lag and overshoot, a frame at a time from the start frame. Returns [jiggle1].

Arguments: what to deform, and any target last (the selection when none); the deformer with -e and -q

| Flag | Short | Description |
| --- | --- | --- |
| `name` | `n` | Name of the deformer. |
| `before` | `bf` | Go just above the deformer nearest the mesh. |
| `after` | `af` | Go nearest the mesh, after the other deformers (the default). |
| `frontOfChain` | `foc` | Go above every other deformer. |
| `envelope` | `en` | How much of the deformation applies (1). |
| `edit` | `e` | Edit mode. |
| `query` | `q` | Query mode. |
| `enable` | `ena` | 0 enable (true), 1 disable (false), 2 only after the start frame. |
| `startFrame` | `sf` | Up to this frame nothing moves on its own (1). |
| `stiffness` | `stf` | How hard points are pulled back (0.5). |
| `damping` | `dmp` | How fast the motion dies (0.5). |
| `weight` | `wt` | Sets every point's weight. |
| `jiggleWeight` | `jw` | How much of the lag shows (1). |
| `motionMultiplier` | `mm` | How much of the motion counts (1). |
| `forceOnAxis` | `foa` | A steady push per frame, world space. |

## joint

Create a joint at -p (world), under the selected joint, and select it. With -e: -oj orients joints (xyz...), -sao the secondary axis (yup...), -ch their children too, -zso zeroes scale orient, -spa and -apa set and assume preferred angles. -q asks about one flag; alone it returns the position.

Arguments: joints (edit and query)

| Flag | Short | Description |
| --- | --- | --- |
| `position` | `p` | World position (or offset from the parent joint with -r). |
| `relative` | `r` | -p is relative to the parent joint. |
| `absolute` | `a` | -p is a world position (the default). |
| `component` | `co` | Edit: -p moves this joint alone; its children stay. |
| `name` | `n` | Name of the new joint. |
| `radius` | `rad` | Display size. |
| `orientation` | `o` | jointOrient, degrees. |
| `scaleOrientation` | `so` | rotateAxis, degrees. |
| `scale` | `s` | Scale. |
| `scaleCompensate` | `sc` | segmentScaleCompensate. |
| `segmentScaleCompensate` | `ssc` | The joint doesn't inherit its parent joint's scale. |
| `rotationOrder` | `roo` | xyz, yzx, zxy, xzy, yxz or zyx. |
| `degreeOfFreedom` | `dof` | The axes that may rotate: xyz, xz, y... |
| `stiffness` | `st` | Stiffness per axis. |
| `assumePreferredAngles` | `apa` | Set rotate to the preferred angle. |
| `setPreferredAngles` | `spa` | Set the preferred angle to rotate. |
| `orientJoint` | `oj` | Aim axis order, xyz, yzx... or none. |
| `secondaryAxisOrient` | `sao` | Where the second axis points, yup, zup, xdown... |
| `children` | `ch` | Edit: the joints below too. |
| `zeroScaleOrient` | `zso` | Set rotateAxis to zero; jointOrient takes the change. |
| `exists` | `ex` | Whether a joint of this name exists. |
| `edit` | `e` | Change existing joints. |
| `query` | `q` | Query a joint (its world position unless a flag is asked about). |
| `angleX` | `ax` | The rotate of this axis. |
| `stiffnessX` | `stx` | The stiffness of this axis. |
| `limitX` | `lx` | Lower and upper rotate limit of this axis, degrees. |
| `limitSwitchX` | `lsx` | Whether the lower and the upper limit of this axis apply. |
| `angleY` | `ay` | The rotate of this axis. |
| `stiffnessY` | `sty` | The stiffness of this axis. |
| `limitY` | `ly` | Lower and upper rotate limit of this axis, degrees. |
| `limitSwitchY` | `lsy` | Whether the lower and the upper limit of this axis apply. |
| `angleZ` | `az` | The rotate of this axis. |
| `stiffnessZ` | `stz` | The stiffness of this axis. |
| `limitZ` | `lz` | Lower and upper rotate limit of this axis, degrees. |
| `limitSwitchZ` | `lsz` | Whether the lower and the upper limit of this axis apply. |

## jointCluster

Record a joint's rigid bind settings on a jointCluster node: -j, -ab, -bb, -av, -bv. Returns the node. It softens nothing here.

Arguments: joint

| Flag | Short | Description |
| --- | --- | --- |
| `joint` | `j` | The joint. |
| `aboveBound` | `ab` | How far above, percent. |
| `belowBound` | `bb` | How far below, percent. |
| `aboveValue` | `av` | Weight above. |
| `belowValue` | `bv` | Weight below. |
| `aboveDropoffType` | `adt` | Accepted for existing scripts. |
| `belowDropoffType` | `bdt` | Accepted for existing scripts. |
| `aboveCluster` | `ac` | Accepted for existing scripts. |
| `belowCluster` | `bc` | Accepted for existing scripts. |
| `deformerTools` | `dt` | Accepted for existing scripts. |
| `name` | `n` | The node's name. |

## jointCtx

A jointCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `autoJointOrient` | `ajo` |  |
| `jointAutoLimits` | `jal` |  |
| `jointOrientationPlane` | `jop` |  |
| `secondaryAxisOrient` | `sao` |  |
| `orientJoint` | `oj` |  |
| `scaleJointOrient` | `sjo` |  |
| `scaleOrientation` | `so` |  |
| `scaleCompensate` | `sc` |  |
| `degreeOfFreedomJ` | `dfj` |  |
| `largeBoneRadius` | `lbr` |  |
| `largeJointRadius` | `ljr` |  |
| `smallBoneRadius` | `sbr` |  |
| `smallJointRadius` | `sjr` |  |
| `variableBoneSize` | `vbs` |  |
| `createIKHandle` | `ikh` |  |
| `ikSolverType` | `ikt` |  |
| `poleVectorType` | `pvt` |  |
| `snapHandleH` | `shh` |  |
| `symmetry` | `sym` |  |
| `symmetryAxis` | `sa` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## jointDisplayScale

How big joints draw in the views: the scale, -q reads it. -a takes it as the size, the same number here since the default size is 1. -ik is only held: joints of ik/fk skeletons draw at the common scale.

Arguments: the scale

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `absolute` | `a` | The number is the size. |
| `ikfk` | `ik` | The size of ik/fk skeleton joints. Held only. |

## jointLattice

A flexor's node on a lattice: the joint's bend bulges the lattice's points. Returns the node. Less than the reference: it does not move the lattice's ends rigidly (flexor keeps the lattice on the joints with constraints) and the bind skin flags do nothing.

Arguments: the lattice (the selection when none); the node with -q and -e

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `name` | `n` | Name of the node. |
| `joint` | `j` | The joint whose bend drives the bulge. |
| `geometry` | `g` | With -q: what the lattice deforms. |
| `lengthIn` | `li` | Moves the points on the upper half along the bone. |
| `lengthOut` | `lo` | Moves the points on the lower half along the bone. |
| `widthLeft` | `wl` | Bulge on the left of the bend. |
| `widthRight` | `wr` | Bulge on the right of the bend. |
| `creasing` | `cr` | Bulge on the inside of the bend. |
| `rounding` | `ro` | Bulge on the outside of the bend. |
| `upperTransform` | `ut` | The transform of the upper part: the joint's parent by default. |
| `lowerTransform` | `lt` | The transform of the lower part: the joint by default. |
| `upperBindSkin` | `ub` | Accepted for existing scripts; has no effect. |
| `lowerBindSkin` | `lb` | Accepted for existing scripts; has no effect. |

## keyTangent

Tangents of keys: types (-itt, -ott), angles in degrees (-ia, -oa), vectors (-ix, -iy, -ox, -oy), weights (-iw, -ow), locks (-l, -wl), weighted curves (-wt), or with -g the defaults for new keys. -r makes edits offsets. In query mode each key answers the flags asked.

Arguments: objects, plugs or curves (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Return the value of the flags named instead of setting them. |
| `edit` | `e` | Change the flags named. |
| `attribute` | `at` | Channel name(s). |
| `time` | `t` | A frame or range "10:20". |
| `float` | `f` | A range of driver values on driven key curves, as -t is for time. |
| `index` | `in` | Key index(es) on each curve, or a range "0:2". |
| `includeUpperBound` | `iub` | Whether a range's end counts (on by default). |
| `hierarchy` | `hi` | none, below, above or both. |
| `controlPoints` | `cp` | Accepted for existing scripts: control points have no channels of their own here. |
| `shape` | `s` | Include the channels of the objects' shapes. |
| `animation` | `an` | objects (the default), keys or keysOrObjects: keys means the selected keys. |
| `inTangentType` | `itt` | Tangent into the keys. |
| `outTangentType` | `ott` | Tangent out of the keys. |
| `inAngle` | `ia` | In tangent angle, degrees, in value per second (makes it fixed). |
| `outAngle` | `oa` | Out tangent angle, degrees, in value per second (makes it fixed). |
| `inWeight` | `iw` | Length of the in tangent, on weighted curves. |
| `outWeight` | `ow` | Length of the out tangent, on weighted curves. |
| `ix` | `ix` | In tangent x, seconds. |
| `iy` | `iy` | In tangent y, value. |
| `ox` | `ox` | Out tangent x, seconds. |
| `oy` | `oy` | Out tangent y, value. |
| `lock` | `l` | In and out tangents move together (on by default). |
| `weightLock` | `wl` | The weight stays when the handle is dragged (on by default). |
| `weightedTangents` | `wt` | Make curves weighted or not; with -g, whether new ones are. |
| `unify` | `uni` | Give a key's in and out tangents one angle, and lock them. |
| `relative` | `r` | Edits add to what is there. |
| `absolute` | `a` | Edits are new values (the default). |
| `selected` | `sl` | Work on the keys selectKey selected. |
| `stepAttributes` | `sa` | Accepted for existing scripts: step tangents apply to any channel. |
| `global` | `g` | The default tangents for new keys. |

## keyTimes

Frames that have keys on the objects (the selection when none), for the time slider.

Arguments: objects

## keyframe

Query or edit keys. -q -kc counts them, -q -tc lists their frames, -q -vc their values, -q -n the curves, -q -iv their indices, -q -ev evaluates the curves. -tc/-vc (with -r for relative) move the keys picked by -t, -f, -in or -sl.

Arguments: objects, plugs or curves (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Return the value of the flags named instead of setting them. |
| `edit` | `e` | Change the flags named. |
| `attribute` | `at` | Channel name(s). |
| `time` | `t` | A frame or range "10:20". |
| `float` | `f` | A range of driver values on driven key curves, as -t is for time. |
| `index` | `in` | Key index(es) on each curve, or a range "0:2". |
| `includeUpperBound` | `iub` | Whether a range's end counts (on by default). |
| `hierarchy` | `hi` | none, below, above or both. |
| `controlPoints` | `cp` | Accepted for existing scripts: control points have no channels of their own here. |
| `shape` | `s` | Include the channels of the objects' shapes. |
| `animation` | `an` | objects (the default), keys or keysOrObjects: keys means the selected keys. |
| `floatChange` | `fc` | Driven keys: the driver value, as -tc for time keys. |
| `breakdown` | `bd` | Query: the breakdown keys' frames. Edit: make keys breakdowns, or not. |
| `keyframeCount` | `kc` | Query: how many keys. |
| `timeChange` | `tc` | Query: the keys' frames. Edit: their new frame (or offset with -r). |
| `valueChange` | `vc` | Query: the keys' values. Edit: their new value (or offset with -r). |
| `indexValue` | `iv` | Query: the keys' indices. |
| `relative` | `r` | Edits are offsets. |
| `absolute` | `a` | Edits are new values (the default). |
| `option` | `o` | How keys move past others: over (the default), move, insert or segmentOver. |
| `name` | `n` | Query: the curves' names. |
| `eval` | `ev` | Query: the curves evaluated at -t (the current frame by default). |
| `selected` | `sl` | Work on the keys selectKey selected. |
| `lastSelected` | `lsl` | Work on the key selected last. |
| `adjustBreakdown` | `abd` | Breakdowns keep their place between keys that move (on by default). |
| `tickDrawSpecial` | `tds` | Query or set the special tick colour of keys. |

## keyframeOutliner

Script UI: keyframeOutliner, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `animCurve` | `an` |  |
| `display` | `dsp` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## keyframeRegionCurrentTimeCtx

A keyframeRegionCurrentTimeCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## keyframeRegionDirectKeyCtx

A keyframeRegionDirectKeyCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `option` | `o` |  |
| `selectedOnly` | `slo` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## keyframeRegionDollyCtx

A keyframeRegionDollyCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `scale` | `s` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## keyframeRegionInsertKeyCtx

A keyframeRegionInsertKeyCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `breakdown` | `bd` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## keyframeRegionMoveKeyCtx

A keyframeRegionMoveKeyCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `moveFunction` | `mf` |  |
| `option` | `o` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## keyframeRegionScaleKeyCtx

A keyframeRegionScaleKeyCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `scaleSpecifiedKeys` | `ssk` |  |
| `type` | `typ` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## keyframeRegionSelectKeyCtx

A keyframeRegionSelectKeyCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## keyframeRegionSetKeyCtx

A keyframeRegionSetKeyCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `breakdown` | `bd` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## keyframeRegionTrackCtx

A keyframeRegionTrackCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `scale` | `s` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## keyframeStats

The Graph Editor's stats fields: -q -ta the selected keys' frame, -q -va their value, blank when they differ.

Arguments: the control's name, accepted for existing scripts

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Return the value of the flags named instead of setting them. |
| `edit` | `e` | Change the flags named. |
| `timeAnnotation` | `ta` | Query: the time field. |
| `valueAnnotation` | `va` | Query: the value field. |
| `precision` | `pre` | Accepted for existing scripts. |
| `classicMode` | `cm` | Accepted for existing scripts. |
| `animEditor` | `ae` | Accepted for existing scripts. |

## keyingGroup

A set of objects keyed together: -n, -em empty; -e -add and -rm change it; -q lists it.

Arguments: objects

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `name` | `n` | The set's name. |
| `empty` | `em` | Make it empty. |
| `addElement` | `add` | With -e: what to add. |
| `include` | `in` | With -e: what to add. |
| `remove` | `rm` | With -e: what to remove. |
| `minimizeRotation` | `mr` | Accepted for existing scripts. |

## lassoContext

A lassoContext tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `drawClosed` | `dc` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## lattice

Add a lattice to the selected meshes or vertices: lattice -dv 2 5 2 -oc true. Returns [ffd1, ffd1Lattice, ffd1Base]. Move ffd1Lattice.pt[s][t][u] to deform. With -e on the ffd or lattice: -dv, -ldv, -ol, -ofd, -fm, -remap, -resetLattice. With -q: -dv, -ldv, -ol, -ofd, -fm, -g.

Arguments: meshes or vertices (the selection when none); the ffd or lattice with -e and -q

| Flag | Short | Description |
| --- | --- | --- |
| `name` | `n` | Name of the deformer. |
| `before` | `bf` | Go just above the deformer nearest the mesh. |
| `after` | `af` | Go nearest the mesh, after the other deformers (the default). |
| `frontOfChain` | `foc` | Go above every other deformer. |
| `envelope` | `en` | How much of the deformation applies (1). |
| `edit` | `e` | Edit mode. |
| `query` | `q` | Query mode. |
| `divisions` | `dv` | Control points in S, T and U (2 2 2). |
| `ldivisions` | `ldv` | Local influence in S, T and U: how many control points move a point (2 2 2). |
| `objectCentered` | `oc` | Fit the lattice round the geometry (on). Off, a unit lattice at the origin. |
| `outsideLattice` | `ol` | 0 only points inside deform, 1 all of them, 2 falling off with distance. |
| `outsideFalloffDist` | `ofd` | With -ol 2: how far the effect reaches, in lattice widths. |
| `freezeMapping` | `fm` | Hold each point's place in the lattice from now on, wherever it goes. |
| `position` | `pos` | Where to put the lattice. |
| `rotation` | `ro` | Rotation of the lattice, degrees. |
| `scale` | `s` | Size of the lattice. |
| `remap` | `rmp` | With -e: take the frozen mapping again from where the points are now. |
| `resetLattice` | `rl` | With -e: put the control points back on the regular grid. |
| `latticeReset` | `ltr` | With -e: the same as -resetLattice. |
| `removeTweaks` | `rt` | With -e: the same as -resetLattice. |
| `geometry` | `g` | Query: the deformed shapes. |

## layerButton

Script UI: layerButton, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `label` | `l` |  |
| `name` | `n` |  |
| `changedCommand` | `cc` |  |
| `color` | `cl` |  |
| `command` | `c` |  |
| `current` | `cr` |  |
| `doubleClickCommand` | `dcc` |  |
| `hideOnPlaybackCommand` | `hpc` |  |
| `identification` | `id` |  |
| `labelWidth` | `lw` |  |
| `layerHideOnPlayback` | `lhp` |  |
| `layerState` | `ls` |  |
| `layerVisible` | `lv` |  |
| `renameCommand` | `rc` |  |
| `select` | `s` |  |
| `transparent` | `t` |  |
| `typeCommand` | `tc` |  |
| `visibleCommand` | `vc` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## layerEditorCopyLayer

Duplicate the named (or selected) layers: with their animation, or with -cna without. Returns the copies.

Arguments: the layers

| Flag | Short | Description |
| --- | --- | --- |
| `noAnimation` | `cna` | Channels only. |

## layerEditorDeleteEmpty

Delete every layer with no channels. Returns how many.

## layerEditorEmpty

Remove every channel and curve from the named (or selected) layers, keeping the layers.

Arguments: the layers

## layerEditorExtractNonLayered

Move the non-layered animation of the selected objects, or -all objects, onto a new layer. Returns the layer.

| Flag | Short | Description |
| --- | --- | --- |
| `all` | `a` | Every animated object. |

## layerEditorExtractObjects

Move the selected objects off the named (or selected) layers onto new `<layer>_extract` layers. Returns the new layers.

Arguments: the layers

## layerEditorMoveToNewLayer

Move the selected objects from the layers they are in to one new layer, curves included. Returns it.

Arguments: the new layer's name

## layerEditorSelectObjects

Select every object with channels on the named (or selected) layers.

Arguments: the layers

## layout

Edit and query any layout.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `childArray` | `ca` |  |
| `numberOfChildren` | `nch` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## lightlink

Make, break and query which lights light which objects: -l lights -o objects, -m, -b, -q. With both and no mode it makes; with one it queries.

Arguments: lights and objects

| Flag | Short | Description |
| --- | --- | --- |
| `b` | `b` | Break the links. |
| `make` | `m` | Make the links. |
| `query` | `q` | Query. |
| `light` | `l` | A light or set of lights. |
| `object` | `o` | An object or set of objects. |
| `hierarchy` | `h` | Accepted: transforms above the first aren't listed. |
| `sets` | `set` | Query: include sets. |
| `shapes` | `shp` | Query: include shapes. |
| `transforms` | `t` | Query: include transforms. |
| `shadow` | `shd` | Link shadows. |
| `useActiveLights` | `ual` | Use the selected lights. |
| `useActiveObjects` | `uao` | Use the selected objects. |

## listAttr

Attribute names of a node.

Arguments: object

| Flag | Short | Description |
| --- | --- | --- |
| `keyable` | `k` | Only keyable. |
| `unlocked` | `u` | Only unlocked. |
| `connectable` | `c` | Only connectable. |
| `settable` | `s` | Only settable. |
| `multi` | `m` | Only multi attributes, with their elements. |
| `userDefined` | `ud` | Only added ones. |
| `locked` | `l` | Only locked. |
| `channelBox` | `cb` | Only those in the channel box without being keyable. |
| `read` | `r` | Only readable. |
| `write` | `w` | Only writable. |
| `visible` | `v` | Only visible. |
| `leaf` | `lf` | Only leaves of compounds. |
| `scalarAndArray` | `sa` | Only scalars and arrays of them. |
| `scalar` | `sc` | Only scalars. |
| `shortNames` | `sn` | Short names. |
| `array` | `a` | Only arrays. |
| `output` | `o` | Only outputs. |
| `hasData` | `hd` | Accepted for existing scripts. |
| `inUse` | `iu` | Accepted for existing scripts. |
| `changedSinceFileOpen` | `cfo` | Accepted for existing scripts. |
| `nodeName` | `nn` | Accepted for existing scripts. |
| `string` | `st` | Only names matching this pattern, or these. |
| `category` | `ct` | Accepted for existing scripts. |

## listCameras

The cameras: -p perspective only, -o orthographic only.

| Flag | Short | Description |
| --- | --- | --- |
| `perspective` | `p` |  |
| `orthographic` | `o` |  |

## listConnections

Nodes (or plugs with -p) connected to the objects or plugs: -s sources, -d destinations (both by default), -type filters by node type, -c pairs each with the plug on this side.

Arguments: objects or plugs

| Flag | Short | Description |
| --- | --- | --- |
| `source` | `s` | Include upstream connections (default on). |
| `destination` | `d` | Include downstream connections (default on). |
| `plugs` | `p` | Return plugs rather than nodes. |
| `connections` | `c` | Pair each with the plug on this side. |
| `exactType` | `et` | With -type: exactly this type, not derived ones. |
| `shapes` | `sh` | A connected shape is named, not its transform. |
| `skipConversionNodes` | `scn` | Report what is beyond unitConversion nodes. |
| `type` | `t` | Only nodes of this type. |

## listDisplayLayers

Every display layer in panel order, each as displayLayer returns it.

## listHistory

The nodes feeding an object's mesh, nearest first: history nodes, then the generator or intermediate shape at the bottom.

Arguments: objects

| Flag | Short | Description |
| --- | --- | --- |
| `type` | `typ` | Only nodes of this type or a type derived from it (geometryFilter for every deformer). |
| `pruneDagObjects` | `pdo` | Leave out shapes and transforms: only the history nodes. |

## listLights

The lights in the scene: their transforms, -s their shapes, -sl only the selected.

| Flag | Short | Description |
| --- | --- | --- |
| `shapes` | `s` | Their shapes. |
| `selection` | `sl` | Only the selected. |

## listNodeTypes

Node types of a classification: listNodeTypes "utility/general", -ex leaves one out.

Arguments: classification

| Flag | Short | Description |
| --- | --- | --- |
| `exclude` | `ex` | A classification to leave out. |

## listRelatives

Children (default), parent or all descendants.

Arguments: object

| Flag | Short | Description |
| --- | --- | --- |
| `parent` | `p` |  |
| `allDescendents` | `ad` |  |
| `children` | `c` | Children (the default). |
| `shapes` | `s` | Only shapes. |
| `allParents` | `ap` | Every parent. |
| `fullPath` | `f` | Full paths. |
| `path` | `pa` | Shortest unique paths. |
| `noIntermediate` | `ni` | Leave out intermediate objects. |
| `type` | `typ` | Only this type or these. |

## listSets

Every object set's name in creation order. quickSelect=True lists only quick select sets, False only plain ones; object= lists the sets that hold that object. info=True returns {name, quickSelect, members} each.

| Flag | Short | Description |
| --- | --- | --- |
| `quickSelect` | `qs` | Only quick select sets (True) or only plain sets (False). |
| `object` | `o` | Only sets holding this object, whole or by its components. |
| `info` | `i` | Return each set as a dict instead of its name. |
| `type` | `t` | 1 for rendering sets, 2 for deformer sets. |
| `allSets` | `as` | Every set, rendering sets too. |
| `extendToShape` | `ets` | With -object: a transform stands for its shapes. |

## loadPlugin

Load a plug-in: a compiled one (.so, .dll) built with Aztek's kit or a Python one (.py), by path or by name from the folders in AZTEK_PLUG_IN_PATH and MAYA_PLUG_IN_PATH. Returns its name in a list.

Arguments: plug-in file or name

| Flag | Short | Description |
| --- | --- | --- |
| `quiet` | `qt` | Don't complain when it can't be found. |

## lockNode

Lock nodes against deleting and renaming: -l on or off (on with no flag); -q -l reads each.

Arguments: objects

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `lock` | `l` | On or off. |
| `lockName` | `ln` | Accepted: a locked node keeps its name anyway. |
| `lockUnpublished` | `lu` | Accepted for existing scripts. |
| `ignoreComponents` | `ic` | Accepted for existing scripts. |

## loft

A surface through curves in order: loft c1 c2 c3 -c 0. Returns [loftedSurface1, loft1].

Arguments: curves in order (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `close` | `c` | Back to the first curve. |
| `divisions` | `dv` | Mesh points per span of the first curve. |
| `uniform` | `u` | Even parameters between the curves, rather than by their distance. |
| `autoReverse` | `ar` | Turn curves round to run alike (on). |
| `degree` | `d` | Degree across the curves, 1 to 3 (3). |
| `sectionSpans` | `ss` | Spans between each pair of curves (1). |
| `reverseSurfaceNormals` | `rsn` | Flip the normals. |
| `polygon` | `po` | 0 NURBS (the default), 1 polygons. |
| `constructionHistory` | `ch` | Keep the node (on by default). |
| `range` | `rn` | Accepted for existing scripts. |
| `name` | `n` | Name of the new object. |

## lookThru

Make a view look through a camera: lookThru camera [panel]. -q gives the camera.

Arguments: camera [panel]

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` |  |
| `objects` | `o` |  |
| `nearClip` | `nc` |  |
| `farClip` | `fc` |  |

## ls

List nodes and components: names, wildcards (pCube*), components (pCube1.vtx[*]), filtered by the flags.

Arguments: names or patterns

| Flag | Short | Description |
| --- | --- | --- |
| `selection` | `sl` | The selection. |
| `orderedSelection` | `os` | The selection, in order. |
| `flatten` | `fl` | Components one per item. |
| `long` | `l` | Full DAG paths. |
| `shortNames` | `sn` | Shortest unique names (the default). |
| `type` | `typ` | Only nodes of this type or derived; a list of types too. |
| `exactType` | `et` | Only nodes of exactly this type. |
| `dag` | `dag` | Only DAG nodes. |
| `referencedNodes` | `rn` | Only nodes from references. |
| `dagObjects` | `do` | DAG nodes and everything below them. |
| `shapes` | `s` | Only shapes. |
| `transforms` | `tr` | Only transforms. |
| `geometry` | `g` | Only geometry shapes. |
| `cameras` | `ca` | Only cameras. |
| `lights` | `lt` | Only lights. |
| `objectsOnly` | `o` | Objects, not their components. |
| `visible` | `v` | Only visible objects. |
| `invisible` | `iv` | Only hidden objects. |
| `assemblies` | `assemblies` | Only top level DAG nodes. |
| `leaf` | `lf` | Only nodes with no children. |
| `noIntermediate` | `ni` | No intermediate objects. |
| `intermediateObjects` | `io` | Only intermediate objects. |
| `hilite` | `hl` | The hilited objects. |
| `sets` | `set` | Only sets. |
| `materials` | `mat` | Only materials. |
| `textures` | `tex` | Only textures. |
| `showType` | `st` | Each name followed by its type. |
| `head` | `hd` | Only the first n. |
| `tail` | `tl` | Only the last n. |
| `uuid` | `uid` | UUIDs rather than names. |

## lsThroughFilter

The nodes a filter passes: -item limits it to those named, -selection to the selected, -sort, -reverse.

Arguments: filter

| Flag | Short | Description |
| --- | --- | --- |
| `item` | `it` |  |
| `nodeArray` | `na` |  |
| `reverse` | `rv` |  |
| `selection` | `sl` |  |
| `sort` | `so` |  |

## makeCollideNCloth

Make the selected meshes colliders: cloth, particles and rigid bodies hit them. Returns the collider nodes.

Arguments: meshes

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query a value. |
| `edit` | `e` | Edit an existing node. |
| `name` | `n` | Name of the new node. |
| `solver` | `sv` | The solver to join. |
| `thickness` | `th` | Collision thickness. |

## makeIdentity

makeIdentity -a 1 bakes translate, rotate and scale into the geometry; without -a they go back to identity and the object moves.

Arguments: objects

| Flag | Short | Description |
| --- | --- | --- |
| `normal` | `n` | Accepted for existing scripts. |
| `preserveNormals` | `pn` | Accepted for existing scripts. |
| `hierarchy` |  | Freeze everything parented under the objects too. |
| `jointOrient` |  | Also line joints' axes up with the world, zeroing their orientation. |
| `apply` | `a` | On: bake into the geometry. Off, when given: back to identity, the object moving. |
| `translate` | `t` | Freeze translation. |
| `rotate` | `r` | Freeze rotation. |
| `scale` | `s` | Freeze scale. |

## makeLive

Make a mesh the live surface: Quad Draw and the snap to live draw on it. -none clears it. -q asks.

Arguments: meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `none` | `n` | Clear the live surface. |
| `query` | `q` | Query mode. |

## makePaintable

Held for scripts: registers node type and attribute as paintable. -at, -ui, -ac, -aca, -rm, -ca; -q lists type, attribute, kind and name for each. The Paint Attributes tool does not read this list: it paints deformer weights only.

Arguments: node type and attribute

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `activate` | `ac` | Turn it on or off. |
| `activateAll` | `aca` | Turn all on or off. |
| `altAttribute` | `aa` | Accepted for existing scripts; has no effect. |
| `attrType` | `at` | intArray, doubleArray, vectorArray, multiInteger, multiFloat, multiDouble or multiVector. |
| `clearAll` | `ca` | Remove all. |
| `remove` | `rm` | Remove this one. |
| `shapeMode` | `sm` | Accepted for existing scripts; has no effect. |
| `uiName` | `ui` | The name shown. |

## makeSingleSurface

Join surfaces meeting edge to edge into one. Returns [surface].

Arguments: surfaces

| Flag | Short | Description |
| --- | --- | --- |
| `stitchTolerance` | `st` | Accepted for existing scripts. |
| `constructionHistory` | `ch` | Keep the node (on by default). |
| `name` | `n` | The new object's name. |
| `object` | `o` | Accepted for existing scripts. |
| `range` | `rn` | Accepted for existing scripts. |
| `caching` | `cch` | Accepted for existing scripts. |
| `nodeState` | `nds` | Accepted for existing scripts. |
| `tolerance` | `tol` | Accepted for existing scripts. |
| `polygon` | `po` | 0 NURBS, 1 polygons. |

## makeUnique

Give instances and references data of their own, so edits to them stop reaching the other members. An instance gets a live copy of its source's stack, a reference its own base mesh; a source hands its data to its first instance and keeps a copy. Returns the meshes changed.

Arguments: meshes

## manipMoveContext

A manipMoveContext tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `activeHandle` | `ah` |  |
| `alignAlong` | `aa` |  |
| `bakePivotOri` | `bpo` |  |
| `constrainAlongNormal` | `xn` |  |
| `currentActiveHandle` | `cah` |  |
| `editPivotMode` | `epm` |  |
| `editPivotPosition` | `epp` |  |
| `lastMode` | `lm` |  |
| `manipVisible` | `vis` |  |
| `mode` | `m` |  |
| `orientAxes` | `oa` |  |
| `orientObject` | `oo` |  |
| `orientTowards` | `ot` |  |
| `pinPivot` | `pin` |  |
| `pivotOriHandle` | `poh` |  |
| `position` | `p` |  |
| `postCommand` | `psc` |  |
| `postDragCommand` | `pod` |  |
| `preCommand` | `prc` |  |
| `preDragCommand` | `prd` |  |
| `preserveChildPosition` | `pcp` |  |
| `preserveUV` | `puv` |  |
| `reflection` | `rfl` |  |
| `reflectionAbout` | `rab` |  |
| `reflectionAxis` | `rfa` |  |
| `reflectionTolerance` | `rft` |  |
| `resetPivotMode` | `rpm` |  |
| `snap` | `s` |  |
| `snapPivotOri` | `spo` |  |
| `snapPivotPos` | `spp` |  |
| `snapRelative` | `sr` |  |
| `snapValue` | `sv` |  |
| `tweakMode` | `twk` |  |
| `xformConstraint` | `xc` |  |
| `activeHandleNormal` | `ahn` |  |
| `interactiveUpdate` | `iu` |  |
| `orientJoint` | `oj` |  |
| `orientJointEnabled` | `oje` |  |
| `secondaryAxisOrient` | `sao` |  |
| `snapComponentsRelative` | `scr` |  |
| `snapLiveFaceCenter` | `slf` |  |
| `snapLivePoint` | `slp` |  |
| `translate` | `tr` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## manipOptions

How manipulators look and work: -scale, -handleSize and so on.

| Flag | Short | Description |
| --- | --- | --- |
| `enableSmartDuplicate` | `esd` |  |
| `enableSmartExtrude` | `ese` |  |
| `forceRefresh` | `fr` |  |
| `handleSize` | `hs` |  |
| `hideManipOnCtrl` | `hmc` |  |
| `hideManipOnShift` | `hms` |  |
| `hideManipOnShiftCtrl` | `hsc` |  |
| `linePick` | `lp` |  |
| `lineSize` | `ls` |  |
| `pivotRotateHandleOffset` | `pro` |  |
| `planeHandleOffset` | `pho` |  |
| `pointSize` | `ps` |  |
| `preselectHighlight` | `psh` |  |
| `refreshMode` | `rm` |  |
| `relative` | `r` |  |
| `rememberActiveHandle` | `rah` |  |
| `rememberActiveHandleAfterToolSwitch` | `raa` |  |
| `scale` | `s` |  |
| `showExtrudeSliders` | `ses` |  |
| `showPivotRotateHandle` | `spr` |  |
| `showPlaneHandles` | `spl` |  |
| `smartDuplicateType` | `sdt` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## manipPivot

Puts the manipulator somewhere else until the selection changes: -p the place, -o the orientation, -reset, -q.

| Flag | Short | Description |
| --- | --- | --- |
| `position` | `p` |  |
| `orientation` | `o` |  |
| `pinPivot` | `pin` |  |
| `reset` | `r` |  |
| `resetOrientation` | `ro` |  |
| `resetPosition` | `rp` |  |
| `valid` | `v` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## manipRotateContext

A manipRotateContext tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `activeHandle` | `ah` |  |
| `alignAlong` | `aa` |  |
| `bakePivotOri` | `bpo` |  |
| `constrainAlongNormal` | `xn` |  |
| `currentActiveHandle` | `cah` |  |
| `editPivotMode` | `epm` |  |
| `editPivotPosition` | `epp` |  |
| `lastMode` | `lm` |  |
| `manipVisible` | `vis` |  |
| `mode` | `m` |  |
| `orientAxes` | `oa` |  |
| `orientObject` | `oo` |  |
| `orientTowards` | `ot` |  |
| `pinPivot` | `pin` |  |
| `pivotOriHandle` | `poh` |  |
| `position` | `p` |  |
| `postCommand` | `psc` |  |
| `postDragCommand` | `pod` |  |
| `preCommand` | `prc` |  |
| `preDragCommand` | `prd` |  |
| `preserveChildPosition` | `pcp` |  |
| `preserveUV` | `puv` |  |
| `reflection` | `rfl` |  |
| `reflectionAbout` | `rab` |  |
| `reflectionAxis` | `rfa` |  |
| `reflectionTolerance` | `rft` |  |
| `resetPivotMode` | `rpm` |  |
| `snap` | `s` |  |
| `snapPivotOri` | `spo` |  |
| `snapPivotPos` | `spp` |  |
| `snapRelative` | `sr` |  |
| `snapValue` | `sv` |  |
| `tweakMode` | `twk` |  |
| `xformConstraint` | `xc` |  |
| `centerTrackball` | `ctb` |  |
| `modifyTranslation` | `mt` |  |
| `rotate` | `ro` |  |
| `useCenterPivot` | `ucp` |  |
| `useManipPivot` | `ump` |  |
| `useObjectPivot` | `uop` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## manipScaleContext

A manipScaleContext tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `activeHandle` | `ah` |  |
| `alignAlong` | `aa` |  |
| `bakePivotOri` | `bpo` |  |
| `constrainAlongNormal` | `xn` |  |
| `currentActiveHandle` | `cah` |  |
| `editPivotMode` | `epm` |  |
| `editPivotPosition` | `epp` |  |
| `lastMode` | `lm` |  |
| `manipVisible` | `vis` |  |
| `mode` | `m` |  |
| `orientAxes` | `oa` |  |
| `orientObject` | `oo` |  |
| `orientTowards` | `ot` |  |
| `pinPivot` | `pin` |  |
| `pivotOriHandle` | `poh` |  |
| `position` | `p` |  |
| `postCommand` | `psc` |  |
| `postDragCommand` | `pod` |  |
| `preCommand` | `prc` |  |
| `preDragCommand` | `prd` |  |
| `preserveChildPosition` | `pcp` |  |
| `preserveUV` | `puv` |  |
| `reflection` | `rfl` |  |
| `reflectionAbout` | `rab` |  |
| `reflectionAxis` | `rfa` |  |
| `reflectionTolerance` | `rft` |  |
| `resetPivotMode` | `rpm` |  |
| `snap` | `s` |  |
| `snapPivotOri` | `spo` |  |
| `snapPivotPos` | `spp` |  |
| `snapRelative` | `sr` |  |
| `snapValue` | `sv` |  |
| `tweakMode` | `twk` |  |
| `xformConstraint` | `xc` |  |
| `preventNegativeScale` | `pns` |  |
| `scale` | `sc` |  |
| `useManipPivot` | `ump` |  |
| `useObjectPivot` | `uop` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## matchPivot

Copy the first object's pivot onto the others, keeping them where they are. position=False keeps their pivot positions and orient=False their pivot orientations.

Arguments: objects (the first is the source)

| Flag | Short | Description |
| --- | --- | --- |
| `position` | `p` | Copy the pivot's world position. |
| `orient` | `o` | Copy the pivot's world axes. |

## matchTransform

Match objects to the last one: -pos, -rot, -scl, -piv; all three transforms when none.

Arguments: objects, then the target

| Flag | Short | Description |
| --- | --- | --- |
| `position` | `pos` | Line the pivots up. |
| `rotation` | `rot` | Match rotation. |
| `scale` | `scl` | Match scale. |
| `pivots` | `piv` | Match the pivots. |
| `positionConstraint` | `pcp` | Accepted for existing scripts. |

## mayaHIKgetInputCharacter

The character driving a character, or nothing.

Arguments: character

## mayaUSDExport

Write the scene to a USD file: usda, usdc, usdz or usd by the file's extension. Flags as the reference's USD plug-in's. Returns the file.

| Flag | Short | Description |
| --- | --- | --- |
| `file` | `f` | The file to write. Its extension picks the format. |
| `defaultUSDFormat` | `duf` | usdc or usda, for a .usd file. |
| `selection` | `sl` | Only what is selected and below it. |
| `exportRoots` | `ert` | DAG subtrees to write, without their parents. |
| `exportUVs` | `uvs` | UV sets as st primvars (on). |
| `exportColorSets` | `cls` | Colour sets as primvars (on). |
| `exportDisplayColor` | `dsp` | The material's colour as primvars:displayColor (off). |
| `exportVisibility` | `vis` | visibility, animated or not (on). |
| `exportInstances` | `ein` | Instanced shapes as USD instances (on). |
| `exportBlendShapes` | `ebs` | Blend shapes as BlendShape prims (off). |
| `exportMaterials` | `mat` | Materials (on). |
| `exportAssignedMaterials` | `ama` | Only materials something uses (on). |
| `exportSkels` | `skl` | none, auto or explicit: skeletons as Skeleton prims. |
| `exportSkin` | `skn` | none, auto or explicit: skinClusters through UsdSkel. |
| `shadingMode` | `shd` | useRegistry or none. |
| `convertMaterialsTo` | `cmt` | UsdPreviewSurface. |
| `mergeTransformAndShape` | `mt` | A transform and its one shape become one prim (on). |
| `stripNamespaces` | `sn` | Drop namespaces from prim names (off). |
| `eulerFilter` | `ef` | Keep animated rotations from jumping by whole turns (off). |
| `worldspace` | `wsp` | Root prims take their world matrix (off). |
| `staticSingleSample` | `sss` | Accepted: values that do not change are always written static. |
| `defaultCameras` | `dc` | Include persp, top, front and side. |
| `includeEmptyTransforms` | `iet` | Keep transforms with nothing below them (on). |
| `preserveUVSetNames` | `puv` | Keep UV set names rather than st1, st2 (off). |
| `defaultMeshScheme` | `dms` | Subdivision scheme of meshes that say nothing: catmullClark, loop, bilinear or none. |
| `materialsScopeName` | `msn` | The scope materials go under (Looks). |
| `rootPrim` | `rpm` | A prim everything is written under. |
| `parentScope` | `psc` | Older name of rootPrim. |
| `rootPrimType` | `rpt` | Scope or Xform. |
| `defaultPrim` | `dp` | The layer's default prim; empty for none. |
| `kind` | `k` | The kind root prims take. |
| `frameRange` | `fr` | First and last frame of an animated export. |
| `frameStride` | `fst` | Frames between samples (1). |
| `frameSample` | `fs` | Offsets sampled at each stride. |
| `extraFrame` | `exf` | Further frames to sample. |
| `exportRelativeTextures` | `rtx` | automatic, relative or absolute. |
| `filterTypes` | `ft` | Node types to leave out. |
| `excludeExportTypes` | `eet` | Meshes or Cameras: families to leave out. |
| `remapUVSetsTo` | `ruv` | Pairs of a UV set and the primvar name it is written as. |
| `customLayerData` | `cld` | Triples of key, value and type for the layer's customLayerData. |
| `metersPerUnit` | `mpu` | metersPerUnit written; 0.01 by default. |
| `upAxis` | `upa` | mayaPrefs, y or z. |
| `unit` | `unt` | Accepted: mayaPrefs. |
| `exportStagesAsRefs` | `sar` | Proxy shapes as references to their files (on). |
| `melPerFrameCallback` | `mfc` | Accepted, not run. |
| `melPostCallback` | `mpc` | MEL run when the export is done. |
| `verbose` | `v` | Accepted. |

## mayaUSDImport

Read a USD file into the scene as nodes. Flags as the reference's USD plug-in's. Returns the new root transforms.

| Flag | Short | Description |
| --- | --- | --- |
| `file` | `f` | The USD file to read. |
| `primPath` | `pp` | The prim to read with what's below it; / for everything. Empty reads the default prim, or everything if there is none. |
| `readAnimData` | `ani` | Time samples become keys (off). |
| `frameRange` | `fr` | Only time samples in this range. |
| `parent` | `p` | A transform to read under. |
| `shadingMode` | `shd` | none, or useRegistry UsdPreviewSurface (the default). |
| `preferredMaterial` | `prm` | lambert, blinn, phong or standardSurface; empty chooses from the shader. |
| `variant` | `var` | A variant set and selection on the prim read. |
| `primVariant` | `pv` | path=set:selection, for any prim. |
| `importInstances` | `ii` | USD instances become shape instances (on). |
| `upAxis` | `upa` | Turn a file on another up axis to the scene's (on). |
| `unit` | `unt` | Scale a file in other units to centimetres (on). |
| `axisAndUnitMethod` | `aum` | rotateScale or overwritePrefs. |
| `metadata` | `md` | Metadata kept as attributes: hidden, instanceable, kind. |
| `excludePrimvar` | `epv` | Primvars not to read. |
| `remapUVSetsTo` | `ruv` | Pairs of a primvar and the UV set it becomes. |
| `apiSchema` | `api` | Accepted. |
| `jobContext` | `jc` | Accepted. |
| `chaser` | `chr` | Accepted. |
| `useAsAnimationCache` | `uac` | Accepted: animated points read their first sample. |
| `importUSDZTextures` | `itx` | Accepted. |
| `importRelativeTextures` | `rtx` | Accepted. |
| `verbose` | `v` | Accepted. |

## mayaUsdCacheMayaReference

Bake a loaded reference's group to a USD file and reference it from the prim.

Arguments: group

| Flag | Short | Description |
| --- | --- | --- |
| `file` | `f` | The cache file. |
| `frameRange` | `fr` | The frames to bake. |
| `exportSkels` | `skl` | As mayaUSDExport. |
| `exportSkin` | `skn` | As mayaUSDExport. |

## mayaUsdDiscardEdits

Throw pulled nodes away: the prim shows as it was. Returns the prim.

Arguments: node

## mayaUsdDuplicate

Copy a prim to nodes, or nodes to a prim: mayaUsdDuplicate source destination. Nothing stays linked. Returns the copy.

Arguments: source destination

## mayaUsdEditAsMaya

Edit As Scene Data: a prim becomes nodes under __mayaUsd__, its USD copy hidden. Returns the top node.

Arguments: proxy,/path

## mayaUsdEditTarget

The layer edits go to. -q -editTarget reads it; -e -editTarget layer sets it.

Arguments: proxy

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Read. |
| `edit` | `e` | Set. |
| `editTarget` | `et` | The layer's identifier. |

## mayaUsdLayerEditor

Edit a layer of a stage, named by identifier. Flags as the reference's USD plug-in's: sublayers, clearing, discarding edits, muting, locking.

Arguments: layer identifier

| Flag | Short | Description |
| --- | --- | --- |
| `insertSubPath` | `is` | A position and a file to add as a sublayer. |
| `removeSubPath` | `rs` | A position, and the proxy. |
| `addAnonymous` | `aa` | Add a new anonymous sublayer of this name. |
| `discardEdits` | `de` | Back to the file's contents. |
| `clear` | `cl` | Empty the layer. |
| `flatten` | `fl` | Compose everything into the root layer. Less than the reference's USD plug-in: references and payloads flatten too. |
| `muteLayer` | `mt` | On or off, and the proxy. |
| `lockLayer` | `lk` | 0 unlocked or 1 locked, and the proxy. |
| `skipSystemLocked` | `ssl` | Accepted. |

## mayaUsdLoadMayaReference

A MayaReference or ALMayaReference prim's file as a file reference under a group at the prim. Returns the group.

Arguments: proxy,/path

## mayaUsdMergeToUsd

Merge to USD: pulled nodes go back to the edit target and are removed. Returns the prim.

Arguments: node

| Flag | Short | Description |
| --- | --- | --- |
| `exportUVs` | `uvs` | As mayaUSDExport. |
| `exportColorSets` | `cls` | As mayaUSDExport. |
| `exportMaterials` | `mat` | As mayaUSDExport. |
| `exportSkels` | `skl` | As mayaUSDExport. |
| `exportSkin` | `skn` | As mayaUSDExport. |
| `exportBlendShapes` | `ebs` | As mayaUSDExport. |
| `frameRange` | `fr` | Bake samples over this range. |

## mayaUsd_createStageFromFile

A proxy shape on an existing USD file. Returns the shape.

Arguments: file

| Flag | Short | Description |
| --- | --- | --- |
| `file` | `f` | The file. |

## mayaUsd_createStageWithNewLayer

A proxy shape on a new anonymous layer. Returns the shape.

## menu

Script UI: menu, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `label` | `l` |  |
| `tearOff` | `to` |  |
| `allowOptionBoxes` | `aob` |  |
| `deleteAllItems` | `dai` |  |
| `itemArray` | `ia` |  |
| `numberOfItems` | `ni` |  |
| `postMenuCommand` | `pmc` |  |
| `postMenuCommandOnce` | `pmo` |  |
| `familyImage` | `fi` |  |
| `mnemonic` | `mn` |  |
| `ltVersion` | `lt` |  |
| `version` | `ver` |  |
| `scrollable` | `scr` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## menuBarLayout

Script UI: menuBarLayout, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `menuBarVisible` | `mbv` |  |
| `menuArray` | `ma` |  |
| `menuIndex` | `mi` |  |
| `numberOfMenus` | `nm` |  |
| `childArray` | `ca` |  |
| `numberOfChildren` | `nch` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## menuItem

Script UI: menuItem, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `label` | `l` |  |
| `command` | `c` |  |
| `subMenu` | `sm` |  |
| `tearOff` | `to` |  |
| `checkBox` | `cb` |  |
| `radioButton` | `rb` |  |
| `divider` | `d` |  |
| `dividerLabel` | `dl` |  |
| `optionBox` | `ob` |  |
| `optionBoxIcon` | `obi` |  |
| `boldFont` | `bld` |  |
| `italicized` | `itl` |  |
| `image` | `i` |  |
| `imageOverlayLabel` | `iol` |  |
| `insertAfter` | `ia` |  |
| `collection` | `cl` |  |
| `data` | `da` |  |
| `dragMenuCommand` | `dmc` |  |
| `dragDoubleClickCommand` | `ddc` |  |
| `echoCommand` | `ec` |  |
| `radialPosition` | `rp` |  |
| `mnemonic` | `mn` |  |
| `keyEquivalent` | `ke` |  |
| `altModifier` | `alt` |  |
| `ctrlModifier` | `ctl` |  |
| `shiftModifier` | `sht` |  |
| `commandModifier` | `cmd` |  |
| `sourceType` | `stp` |  |
| `isOptionBox` | `iob` |  |
| `isCheckBox` | `icb` |  |
| `isRadioButton` | `irb` |  |
| `familyImage` | `fi` |  |
| `version` | `ver` |  |
| `ltVersion` | `lt` |  |
| `allowOptionBoxes` | `aob` |  |
| `postMenuCommand` | `pmc` |  |
| `postMenuCommandOnce` | `pmo` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## mergeAnimLayers

Merge layers into one, keeping the result: -mt top or bottom selected layer, -ic children too, -rlm automatic, additive or override, -db delete the merged layers, -sb sample by, -sm keys only where needed, -fd its tolerance. With none named, the selected layers. Returns the layer merged into.

Arguments: the layers

| Flag | Short | Description |
| --- | --- | --- |
| `mergeTo` | `mt` | top or bottom (the default). |
| `includeChildren` | `ic` | Merge the children of merged layers too (on by default). |
| `resultLayerMode` | `rlm` | automatic, additive or override. |
| `deleteBaked` | `db` | Delete the merged layers (on by default). |
| `sampleBy` | `sb` | Frames between samples (1). |
| `smart` | `sm` | Keep the sources' key times, adding keys only past the tolerance. |
| `fidelity` | `fd` | The smart bake's tolerance. |

## meshCheck

Validate a model in one compact report: ok (no issues), shells with their world boxes (largest first), open border edges, non-manifold edges and vertices, zero-area faces, face/triangle counts, symmetry drift. issues says what is wrong and how to fix it. Changes nothing.

Arguments: [mesh]

| Flag | Short | Description |
| --- | --- | --- |
| `expectShells` |  | Connected parts the model should have (0 skips the check). |
| `maxTriangles` |  | Triangle budget; more is an issue. |

## meshData

A mesh's points and faces as plain lists: {points: [[x,y,z]...], faces: [[i,j,k,...]...]}. base=True reads the editable base mesh (under modifiers) instead of the final result.

Arguments: mesh

| Flag | Short | Description |
| --- | --- | --- |
| `base` |  | The base mesh, not the modified result. |
| `worldSpace` |  | Points in world space. |

## meshInfo

Counts and health of a mesh: points, edges, faces, triangles, tri/quad/ngon faces, open border edges and loops, non-manifold edges, shells, closed, object and world bounds, symmetry, smooth preview, modifiers, material slots (and the cage's counts under modifiers).

Arguments: object

## meshReorder

Reorder Vertices: renumbers a mesh's vertices outward from the given ones, so meshes of the same topology come to match when the same three vertices of a face are named on each.

Arguments: three vertices of a face

| Flag | Short | Description |
| --- | --- | --- |
| `intermediate` | `i` | Accepted. |

## mirrorJoint

Mirror a joint and the joints below it across a world plane: a new hierarchy beside the old one. -mb keeps mirrored behaviour (equal rotate values make mirrored motion); without it the new joints keep the old world axes. Returns the new joints.

Arguments: joints

| Flag | Short | Description |
| --- | --- | --- |
| `mirrorYZ` | `myz` | Across the YZ plane. |
| `mirrorXY` | `mxy` | Across the XY plane. |
| `mirrorXZ` | `mxz` | Across the XZ plane. |
| `mirrorBehavior` | `mb` | Mirror the behaviour, not the orientation. |
| `searchReplace` | `sr` | Two strings: replace the first with the second in the new names. |

## modelCurrentTimeCtx

A modelCurrentTimeCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `percent` | `per` |  |
| `snap` | `sn` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## modelEditor

What a model panel shows: -cam, -da, -wos, -grid, -joints, -nurbsCurves, -polymeshes, -locators, -deformers, -ikHandles, -cameras, -manipulators, -dtx and more; -q and -e.

Arguments: panel

| Flag | Short | Description |
| --- | --- | --- |
| `activeOnly` | `ao` |  |
| `activeView` | `av` |  |
| `allObjects` | `alo` |  |
| `backfaceCulling` | `bfc` |  |
| `camera` | `cam` |  |
| `cameras` | `ca` |  |
| `clipGhosts` | `cg` |  |
| `controlVertices` | `cv` |  |
| `controllers` | `ctrl` |  |
| `deformers` | `df` |  |
| `dimensions` | `dim` |  |
| `displayAppearance` | `da` |  |
| `displayLights` | `dl` |  |
| `displayTextures` | `dtx` |  |
| `dynamicConstraints` | `dc` |  |
| `dynamics` | `dy` |  |
| `fluids` | `fl` |  |
| `follicles` | `fo` |  |
| `fogging` | `fg` |  |
| `grid` | `gr` |  |
| `hairSystems` | `hs` |  |
| `handles` | `ha` |  |
| `headsUpDisplay` | `hud` |  |
| `hulls` | `hu` |  |
| `ikHandles` | `ikh` |  |
| `imagePlane` | `imp` |  |
| `joints` | `j` |  |
| `jointXray` | `jx` |  |
| `lights` | `lt` |  |
| `locators` | `lc` |  |
| `lockMainConnection` | `lck` |  |
| `lowQualityLighting` | `lql` |  |
| `manipulators` | `m` |  |
| `motionTrails` | `mt` |  |
| `nCloths` | `ncl` |  |
| `nParticles` | `npa` |  |
| `nRigids` | `nr` |  |
| `nurbsCurves` | `nc` |  |
| `nurbsSurfaces` | `ns` |  |
| `objectFilterShowInHUD` | `ofs` |  |
| `panel` | `pnl` |  |
| `parent` | `p` |  |
| `pivots` | `pv` |  |
| `planes` | `pl` |  |
| `pluginObjects` | `po` |  |
| `pluginShapes` | `ps` |  |
| `polymeshes` | `pm` |  |
| `rendererName` | `rnm` |  |
| `selectionHiliteDisplay` | `sel` |  |
| `shadows` | `sdw` |  |
| `smoothWireframe` | `swf` |  |
| `sortTransparent` | `st` |  |
| `strokes` | `str` |  |
| `subdivSurfaces` | `sds` |  |
| `textures` | `tx` |  |
| `twoSidedLighting` | `tsl` |  |
| `useDefaultMaterial` | `udm` |  |
| `viewObjects` | `vo` |  |
| `viewSelected` | `vs` |  |
| `wireframeBackingStore` | `wbs` |  |
| `wireframeOnShaded` | `wos` |  |
| `xray` | `xr` |  |
| `activeComponentsXray` | `acx` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## modelPanel

A panel holding a view: -q -cam, -e -cam, -me its editor.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `camera` | `cam` |  |
| `modelEditor` | `me` |  |
| `label` | `l` |  |
| `parent` | `p` |  |
| `control` | `ctl` |  |
| `copy` | `cp` |  |
| `createString` | `cs` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `editString` | `es` |  |
| `exists` | `ex` |  |
| `init` | `in` |  |
| `isUnique` | `iu` |  |
| `menuBarVisible` | `mbv` |  |
| `needsInit` | `ni` |  |
| `popupMenuProcedure` | `pmp` |  |
| `replacePanel` | `rp` |  |
| `tearOff` | `to` |  |
| `tearOffCopy` | `toc` |  |
| `unParent` | `up` |  |
| `useTemplate` | `ut` |  |
| `barLayout` | `bl` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## modifierKinds

Every modifier kind addModifier accepts, including ones extensions registered. docs=True returns {kind: description} instead.

| Flag | Short | Description |
| --- | --- | --- |
| `docs` |  | Return each kind's description too. |

## modifierStack

A mesh's stack as data: its source (generator, meshSource or null) and each modifier, applied first first, with name, type, enabled, exportEnabled and its last evaluation error.

Arguments: mesh

## modifiers

A mesh's modifier stack, bottom (applied first) to top.

Arguments: mesh

## moduleInfo

Ask about the modules found: -listModules, or -moduleName with -path, -version or -definition.

| Flag | Short | Description |
| --- | --- | --- |
| `listModules` | `lm` | Their names. |
| `moduleName` | `mn` | The module asked about. |
| `path` | `p` | Where it is. |
| `version` | `v` | Its version. |
| `definition` | `d` | Its .mod file. |

## morph

Morph: points move to the matching points of -morphTarget. Returns [morph1].

Arguments: what to deform, and any target last (the selection when none); the deformer with -e and -q

| Flag | Short | Description |
| --- | --- | --- |
| `name` | `n` | Name of the deformer. |
| `before` | `bf` | Go just above the deformer nearest the mesh. |
| `after` | `af` | Go nearest the mesh, after the other deformers (the default). |
| `frontOfChain` | `foc` | Go above every other deformer. |
| `envelope` | `en` | How much of the deformation applies (1). |
| `edit` | `e` | Edit mode. |
| `query` | `q` | Query mode. |
| `morphTarget` | `mt` | The mesh to morph to. |
| `morphSpace` | `ms` | 0 object, 1 world. |
| `normalScale` | `nsc` | How much of the morph along the normal applies (1). |
| `tangentScale` | `tsc` | How much of the morph across the surface applies (1). |

## motionTrail

An object's path through time, drawn in the viewport with a cross at each key: creates motionTrail1Handle and motionTrail1. -e -ev samples it again. -q -pts gives the points, -q -kt the key frames.

Arguments: the object (the selection when none), or the trail

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Return the value of the flags named instead of setting them. |
| `edit` | `e` | Change the flags named. |
| `startTime` | `st` | First frame (the playback start). |
| `endTime` | `et` | Last frame (the playback end). |
| `increment` | `inc` | Frames between points (1). |
| `name` | `n` | The trail's name. |
| `evaluate` | `ev` | With -e: sample again. |
| `points` | `pts` | Query: the points. |
| `keyframeTimes` | `kt` | Query: the frames with keys. |

## move

Move objects. move(by=(x, y, z)) moves relative in `space`; move(to=(x, y, z)) puts each pivot at a world position. A selected parent and child move once. move(x=0) sets one world axis and leaves the others: each selected object's pivot, or with components selected every selected point, so move(x=0) puts an edge loop on the centre line.

Arguments: objects

| Flag | Short | Description |
| --- | --- | --- |
| `by` |  | Relative offset, cm. |
| `to` |  | Absolute world position of the pivot, cm. |
| `space` |  | 'world', 'object' or 'parent' for `by`. |
| `relative` | `r` | The values add to what is there. |
| `absolute` | `a` | The values replace what is there (the default). |
| `worldSpace` | `ws` | World axes (the default). |
| `objectSpace` | `os` | The object's own axes. |
| `localSpace` | `ls` | The parent's axes. |
| `parent` | `pa` | The parent's axes. |
| `worldSpaceDistance` | `wd` | Accepted: distances are in world units already. |
| `preserveChildPosition` | `pcp` | Children stay where they are in the world. |
| `preserveGeometryPosition` | `pgp` | Accepted for existing scripts. |
| `rotatePivotRelative` | `rpr` | Accepted for existing scripts. |
| `scalePivotRelative` | `spr` | Accepted for existing scripts. |
| `deletePriorHistory` | `dph` | Accepted for existing scripts. |
| `xformConstraint` | `xc` | Accepted for existing scripts. |
| `constrainAlongNormal` | `xn` | Accepted for existing scripts. |
| `orientJoint` | `oj` | Accepted for existing scripts. |
| `secondaryAxisOrient` | `sao` | Accepted for existing scripts. |
| `componentSpace` | `cs` | Accepted for existing scripts. |
| `x` |  | This axis: a value, or on to read the positional value. |
| `y` |  | This axis. |
| `z` |  | This axis. |
| `moveX` |  | The axes the values are for. |
| `moveY` |  | The axes the values are for. |
| `moveZ` |  | The axes the values are for. |
| `moveXY` | `xy` | The axes the values are for. |
| `moveXZ` | `xz` | The axes the values are for. |
| `moveYZ` | `yz` | The axes the values are for. |
| `moveXYZ` | `xyz` | The axes the values are for. |

## moveComponents

Move the selected components by an offset in world space, or with normal= push each point along its own vertex normal (a face selection uses only the selected faces' normals, so the region inflates as a whole). uvn= also slides each point along its own surface tangents: U follows the uv0 u direction where the mesh has uvs (in edge mode, the selected edges), else the world axis least aligned with the selection's normal, and V completes the frame. Curve points use the curve's frame: U its tangent (a handle's own direction), N across it within its plane, V out of the plane.

| Flag | Short | Description |
| --- | --- | --- |
| `snapEach` |  | Each point lands on whole steps of this, by itself. |
| `by` |  | Offset, cm. |
| `normal` |  | Distance along the vertex normals, cm. |
| `uvn` |  | Distances along each point's own U tangent, V tangent and normal, cm. |
| `onto` |  | Meshes whose surface each moved point then lands on, at its closest point. |

## moveKeyCtx

A moveKeyCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `moveFunction` | `mf` |  |
| `option` | `o` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## moveModifier

Move a modifier up or down its stack by a number of steps (positive is up, applied later).

Arguments: modifier

| Flag | Short | Description |
| --- | --- | --- |
| `by` |  | Steps. |

## movePivot

Move objects' pivots without moving the objects: -by a world offset, or -to a world position. The reference spells it move -r x y z obj.scalePivot obj.rotatePivot.

Arguments: objects

| Flag | Short | Description |
| --- | --- | --- |
| `rotatePivot` | `rp` | Only the rotate pivot. |
| `scalePivot` | `sp` | Only the scale pivot. |
| `objectSpace` | `os` | The values are in the object's space, not the world's. |
| `by` | `r` | World offset. |
| `to` |  | World position. |

## moveVertexAlongDirection

Move vertices along their normal (-n), tangents (-u, -v), or -d with -m. One value per vertex, the last repeating.

Arguments: vertices (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `normalDirection` | `n` | Along the normal. |
| `uDirection` | `u` | Along the first tangent. |
| `vDirection` | `v` | Along the second tangent. |
| `direction` | `d` | A direction. |
| `magnitude` | `m` | How far along it. |
| `uvNormalDirection` | `uvn` | u, v and normal at once. |

## mute

Mute channels: they stay at their value while what drives them carries on underneath. -d turns the mute off and -d -f removes the mute node. -q says whether any is muted. Returns the mute nodes.

Arguments: plugs or objects (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Return the value of the flags named instead of setting them. |
| `edit` | `e` | Change the flags named. |
| `attribute` | `at` | Channel name(s). |
| `disable` | `d` | Turn the mute off. |
| `force` | `f` | With -d: delete the mute node. |
| `hierarchy` | `hi` | none, below, above or both. |

## muteChannel

Mute channels: they stay at their value while what drives them carries on underneath. -d turns the mute off and -d -f removes the mute node. -q says whether any is muted. Returns the mute nodes.

Arguments: plugs or objects (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Return the value of the flags named instead of setting them. |
| `edit` | `e` | Change the flags named. |
| `attribute` | `at` | Channel name(s). |
| `disable` | `d` | Turn the mute off. |
| `force` | `f` | With -d: delete the mute node. |
| `hierarchy` | `hi` | none, below, above or both. |

## nCloth

Make the selected (or named) meshes cloth in a solver: the mesh as it comes is the animation the cloth starts from and can be drawn to. Returns the cloth nodes.

Arguments: meshes

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query a value. |
| `edit` | `e` | Edit an existing node. |
| `name` | `n` | Name of the new node. |
| `solver` | `sv` | The solver to join (the newest, or a new one). |
| `preset` | `p` | The material it starts as: silk, cotton (the default), leather, heavyDenim or rubber. |

## nParticle

Make particles: -p x y z for each one, or a grid with -ll, -ur and -grs. -q -ct counts them; -q -at position -id 3 reads one. Returns [transform, shape].

Arguments: particles (query)

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query a value. |
| `edit` | `e` | Edit an existing node. |
| `name` | `n` | Name of the new node. |
| `position` | `p` | A particle there: x y z. Repeat it. |
| `lowerLeft` | `ll` | Grid: one corner. |
| `upperRight` | `ur` | Grid: the other corner. |
| `gridSpacing` | `grs` | Grid: spacing. |
| `conserve` | `c` | Accepted for old scripts. |
| `count` | `ct` | Query: how many. |
| `attribute` | `at` | Query: a per particle value. |
| `id` | `id` | Query: which particle. |
| `solver` | `sv` | The solver to join. |

## nameCommand

Create a name a hotkey can run: -c the command, -ann its annotation.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `command` | `c` | The command. |
| `annotation` | `ann` | What it does. |
| `sourceType` | `stp` | mel or python. |

## namespace

The reference's namespace command: -add, -set, -rm, -ex, -q, -mnr, -dnc, -ren.

Arguments: [namespace]

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query. |
| `addNamespace` | `add` | As in the reference. |
| `setNamespace` | `set` | As in the reference. |
| `removeNamespace` | `rm` | As in the reference. |
| `parent` | `p` | As in the reference. |
| `exists` | `ex` | As in the reference. |
| `relativeNames` | `rel` | As in the reference. |
| `isRootNamespace` | `ir` | As in the reference. |
| `validateName` | `vn` | As in the reference. |
| `mergeNamespaceWithRoot` | `mnr` | As in the reference. |
| `mergeNamespaceWithParent` | `mnp` | As in the reference. |
| `deleteNamespaceContent` | `dnc` | As in the reference. |
| `rename` | `ren` | As in the reference. |
| `moveNamespace` | `mv` | As in the reference. |
| `force` | `f` | As in the reference. |
| `recurse` | `r` | As in the reference. |
| `absoluteName` | `an` | As in the reference. |

## namespaceInfo

What a namespace holds: -cur, -lon, -lod, -ls, -r, -dp, -bn, -p.

Arguments: [namespace]

| Flag | Short | Description |
| --- | --- | --- |
| `currentNamespace` | `cur` | As in the reference. |
| `listOnlyNamespaces` | `lon` | As in the reference. |
| `listOnlyDependencyNodes` | `lod` | As in the reference. |
| `listNamespace` | `ls` | As in the reference. |
| `recurse` | `r` | As in the reference. |
| `dagPath` | `dp` | As in the reference. |
| `fullName` | `fn` | As in the reference. |
| `absoluteName` | `an` | As in the reference. |
| `baseName` | `bn` | As in the reference. |
| `parent` | `p` | As in the reference. |
| `isRootNamespace` | `ir` | As in the reference. |
| `shortName` | `sn` | As in the reference. |

## namingConvention

Naming rules for this workspace (.aztek/naming.json). Query with no flags; check lists violations; fix renames them, whether or not enforcement is on. check and fix cover the given objects and everything under them, or the whole scene; referenced nodes keep their names.

Arguments: [objects]

| Flag | Short | Description |
| --- | --- | --- |
| `enable` | `e` | Turn enforcement on or off for the workspace (user prefs if none open). |
| `mode` | `m` | fix \| warn \| block |
| `uniqueNames` | `un` | Make names unique across the whole scene, not just among siblings (workspace, else user prefs). |
| `check` | `c` | Return [[node, expectedName], ...] for names that break the rules. |
| `fix` | `f` | Rename every node that breaks the rules. |

## nearestPointOnMesh

Make a node on a mesh that answers the point on it closest to -ip. Returns the node.

Arguments: mesh

| Flag | Short | Description |
| --- | --- | --- |
| `inPosition` | `ip` | The position. |
| `name` | `n` | The node's name. |

## newCharacterWithName

Define a character. Joints named (or selected, with everything below them) go to slots by their names; setCharacterObject sets others. Returns the character.

Arguments: the new character's name, then joints

| Flag | Short | Description |
| --- | --- | --- |
| `name` | `n` | The character's name. |

## newScene

Clear the scene. Not undoable.

## newSceneUnits

The units a new scene starts in, as File > New Scene's options set them: -linear, -angle, -time. -q gives them.

| Flag | Short | Description |
| --- | --- | --- |
| `linear` | `l` | Linear unit. |
| `angle` | `a` | Angle unit. |
| `time` | `t` | Time unit. |
| `query` | `q` | Query mode. |

## newton

Make a newton field acting on the selected bodies, or -q/-e one. Returns [field].

Arguments: bodies to act on

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query a value. |
| `edit` | `e` | Edit an existing node. |
| `name` | `n` | Name of the new node. |
| `magnitude` | `m` | Strength. |
| `attenuation` | `att` | How fast it weakens with distance. |
| `maxDistance` | `mxd` | How far it reaches (turns the limit on). |
| `useMaxDistance` | `umd` | Limit its reach. |
| `directionX` | `dx` | Direction X. |
| `directionY` | `dy` | Direction Y. |
| `directionZ` | `dz` | Direction Z. |
| `speed` | `s` | Air: wind speed. |
| `inheritVelocity` | `iv` | Air: kept motion. |
| `frequency` | `f` | Turbulence: noise frequency. |
| `minDistance` | `mnd` | Newton: least distance. |
| `position` | `pos` | Where it is. |

## nodeCast

Replace the target node with the source: its connections, hierarchy and values move to the source, whose type must be the target's or a kind of it (-f skips that). -cda, -dua, -sn, -sv. Returns 0. -dsa and -dsj are accepted and change nothing. Referenced nodes are not refused.

Arguments: source, then target

| Flag | Short | Description |
| --- | --- | --- |
| `copyDynamicAttrs` | `cda` | Bring added attributes the source lacks. |
| `disableAPICallbacks` | `dsa` | Accepted for existing scripts; has no effect. |
| `disableScriptJobCallbacks` | `dsj` | Accepted for existing scripts; has no effect. |
| `disconnectUnmatchedAttrs` | `dua` | Drop connections the source has no attribute for. |
| `force` | `f` | Try even when the types differ. |
| `swapNames` | `sn` | Swap the names. |
| `swapValues` | `sv` | Exchange common values rather than copy them. |

## nodeEditor

Script UI: nodeEditor, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `addNewNodes` | `ann` |  |
| `addNode` | `an` |  |
| `additiveGraphingMode` | `agm` |  |
| `allAttributes` | `ala` |  |
| `allNodes` | `aln` |  |
| `autoSizeNodes` | `asn` |  |
| `beginCreateNode` | `bcn` |  |
| `consistentNameSize` | `cns` |  |
| `createNodeCommand` | `cnc` |  |
| `customAttributeListEdit` | `cal` |  |
| `cycleHUD` | `ch` |  |
| `defaultPinnedState` | `dps` |  |
| `deleteSelected` | `dsl` |  |
| `downstream` | `ds` |  |
| `dotFormat` | `dot` |  |
| `duplicateSelected` | `dup` |  |
| `errorCallback` | `ecb` |  |
| `extendToShapes` | `ets` |  |
| `feedbackConnection` | `fbc` |  |
| `feedbackNode` | `fbn` |  |
| `feedbackPlug` | `fbp` |  |
| `feedbackTabIndex` | `fbi` |  |
| `feedbackType` | `fbt` |  |
| `filterCreateNodeTypes` | `fcn` |  |
| `focusCommand` | `fc` |  |
| `frameAll` | `fa` |  |
| `frameModelSelection` | `fms` |  |
| `frameSelected` | `fs` |  |
| `getNodeList` | `gnl` |  |
| `graphSelectedConnections` | `gsc` |  |
| `graphSelection` | `gs` |  |
| `gridSnap` | `gsn` |  |
| `gridVisibility` | `gv` |  |
| `hasWatchpoint` | `hw` |  |
| `hudMessage` | `hm` |  |
| `ignoreAssets` | `ia` |  |
| `island` | `isl` |  |
| `keyPressCommand` | `kp` |  |
| `keyReleaseCommand` | `kr` |  |
| `layout` | `lay` |  |
| `layoutCommand` | `lc` |  |
| `nodeSwatchSize` | `nss` |  |
| `nodeTitleMode` | `ntm` |  |
| `nodeViewMode` | `nvm` |  |
| `panView` | `pv` |  |
| `pinSelectedNodes` | `psn` |  |
| `popupMenuScript` | `pms` |  |
| `redockSelectedNodes` | `rsn` |  |
| `removeDownstream` | `rd` |  |
| `removeNode` | `rem` |  |
| `removeUnselected` | `run` |  |
| `removeUpstream` | `ru` |  |
| `rootNode` | `rn` |  |
| `rootsFromSelection` | `rfs` |  |
| `scaleView` | `sv` |  |
| `selectAll` | `sa` |  |
| `selectConnectionNodes` | `scn` |  |
| `selectDownstream` | `sdn` |  |
| `selectFeedbackConnection` | `sfc` |  |
| `selectNode` | `sln` |  |
| `selectUpstream` | `su` |  |
| `setWatchpoint` | `sw` |  |
| `settingsChangedCallback` | `scc` |  |
| `shaderNetworks` | `sn` |  |
| `showAllNodeAttributes` | `saa` |  |
| `showNamespace` | `sns` |  |
| `showSGShapes` | `ssg` |  |
| `showShapes` | `ss` |  |
| `showTransforms` | `st` |  |
| `showUnitConversions` | `suc` |  |
| `syncedSelection` | `ssl` |  |
| `toggleAttrFilter` | `taf` |  |
| `toggleSelectedPins` | `tsp` |  |
| `toggleSwatchSize` | `tss` |  |
| `toolTipCommand` | `ttc` |  |
| `traversalDepthLimit` | `tdl` |  |
| `updateNodeAdded` | `una` |  |
| `upstream` | `ups` |  |
| `useAssets` | `ua` |  |
| `control` | `ctl` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `filter` | `f` |  |
| `forceMainConnection` | `fmc` |  |
| `highlightConnection` | `hlc` |  |
| `lockMainConnection` | `lck` |  |
| `mainListConnection` | `mlc` |  |
| `panel` | `pnl` |  |
| `selectionConnection` | `slc` |  |
| `stateString` | `sts` |  |
| `unParent` | `up` |  |
| `unlockMainConnection` | `ulk` |  |
| `updateMainConnection` | `upd` |  |
| `useTemplate` | `ut` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## nodePreset

Save and load a node's settings by name: -sv node name, -ld node name, -ls node, -del node name, -ex node name, -ivn name; -atr limits what is saved. Less than the reference: multis and hidden attributes are left out, files are JSON, -ctm does nothing.

| Flag | Short | Description |
| --- | --- | --- |
| `attributes` | `atr` | Attributes to save, separated by spaces. |
| `custom` | `ctm` | Accepted for existing scripts; has no effect. |
| `delete` | `delete` | Node and preset. |
| `exists` | `ex` | Node and preset. |
| `isValidName` | `ivn` | A name to check. |
| `list` | `ls` | The node. |
| `load` | `ld` | Node and preset. |
| `save` | `sv` | Node and preset. |

## nodeType

Type name of a node.

Arguments: object

| Flag | Short | Description |
| --- | --- | --- |
| `inherited` | `i` | Its types, base first. |
| `apiType` | `api` | The API type name. |
| `derived` | `d` | Types derived from it. |
| `isTypeName` | `itn` | The argument is a type, not a node. |

## nonLinear

Add a nonlinear deformer (-type bend, twist, squash, flare, sine or wave) to the selected meshes, with a handle fitted to them. Returns [deformer, handle].

Arguments: meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `name` | `n` | Name of the deformer; its handle takes the name plus Handle. |
| `type` | `typ` | bend, twist, squash, flare, sine or wave. |
| `curvature` | `cur` | bend: how far it bends. |
| `startAngle` | `sa` | twist: degrees at the low bound. |
| `endAngle` | `ea` | twist: degrees at the high bound. |
| `factor` | `f` | squash: stretch (positive) or squash (negative). |
| `amplitude` | `amp` | sine, wave: height of the waves. |
| `wavelength` | `wav` | sine, wave: length of the waves. |

## normalConstraint

Constrain the object named (or selected) last to the targets before it; a second call adds targets. -mo keeps the current offset; -w sets the targets' weight; -e and -q work on the constraint or the object it drives. Returns [constraint].

Arguments: targets, then the object to drive or the constraint

| Flag | Short | Description |
| --- | --- | --- |
| `maintainOffset` | `mo` | Keep the object where it is now relative to the targets. |
| `weight` | `w` | Weight of the targets named (1). |
| `name` | `n` | Name of the constraint. |
| `offset` | `o` | Added to the result. |
| `remove` | `rm` | Remove the targets named. Removing the last deletes the constraint. |
| `targetList` | `tl` | Query: the targets. |
| `weightAliasList` | `wal` | Query: the targets' weight attributes. |
| `aimVector` | `aim` | The local axis that points at the target. |
| `upVector` | `u` | The local axis that points up. |
| `worldUpVector` | `wu` | The world direction up means. |
| `worldUpType` | `wut` | scene, object, objectrotation, vector or none. |
| `worldUpObject` | `wuo` | The object for worldUpType object and objectrotation. |
| `interpType` | `int` | How rotations blend: noflip, average, shortest, longest, cache, or 0 to 4. |
| `skipTranslate` | `st` | Translate axes to leave alone: x, y, z or none. May be used more than once. |
| `skipRotate` | `sr` | Rotate axes to leave alone: x, y, z or none. May be used more than once. |
| `skip` | `sk` | Axes of this constraint's channel to leave alone: x, y, z or none. May be used more than once. |
| `skipScale` |  | Scale axes to leave alone: x, y, z or none. |
| `edit` | `e` | Change the constraint. |
| `query` | `q` | Ask about the constraint. |

## nurbsBoolean

A boolean of two surfaces, as a polygon mesh: there are no NURBS solids to trim back. -op 0 union, 1 difference, 2 intersection.

Arguments: two surfaces

| Flag | Short | Description |
| --- | --- | --- |
| `operation` | `op` | 0 union, 1 difference, 2 intersection. |
| `smartConnection` | `sc` | Accepted for existing scripts. |
| `constructionHistory` | `ch` | Accepted for existing scripts. |
| `nsrfsInFirstShell` | `nsf` | Accepted for existing scripts. |
| `name` | `n` | Accepted for existing scripts. |

## nurbsCopyUVSet

Copy the implicit parameterization to the surface's explicit set, as the reference's debugging command does. Returns true. The values are evenly spaced by CV.

Arguments: a surface (the selection when none)

## nurbsCube

Create a cube of six NURBS patches under a group, with a makeNurbCube node. Returns [nurbsCube1, makeNurbCube1].

| Flag | Short | Description |
| --- | --- | --- |
| `axis` | `ax` | The axis (1 0 0). |
| `pivot` | `p` | The centre. |
| `constructionHistory` | `ch` | Keep the make node (on by default). |
| `name` | `n` | Name of the transform. |
| `object` | `o` | Accepted for existing scripts. |
| `polygon` | `po` | 1 makes polygons. |
| `width` | `w` | The width (1). |
| `lengthRatio` | `lr` | Length over width (1). |
| `heightRatio` | `hr` | Height over width (1). |
| `degree` | `d` | The degree (3). |
| `patchesU` | `u` | Patches along U (1). |
| `patchesV` | `v` | Patches along V (1). |

## nurbsCurveToBezier

Make a curve Bezier: a cubic with every third CV an anchor, keeping its shape. Other degrees are refit and change a little. Returns [object, node]. Less than the reference: the shape stays a nurbsCurve with an added attribute marking it.

Arguments: curves (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `constructionHistory` | `ch` | Keep a history node (on by default). |
| `replaceOriginal` | `rpo` | Change the curve itself (on by default). |
| `name` | `n` | Name of a new curve. |
| `object` | `o` | Make an object (accepted for existing scripts). |

## nurbsEditUV

Edit the explicit UVs of surface CVs: -u, -v moves (absolute with -r false), -a degrees with -rr, -su, -sv about -pu, -pv (0 by default). -q gives u, v per CV; -q -u or -q -v one of them. Needs nurbsUVSet -c first.

Arguments: surface.cv[u][v] (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `angle` | `a` | Degrees to rotate. |
| `pivotU` | `pu` | The pivot in U. |
| `pivotV` | `pv` | The pivot in V. |
| `relative` | `r` | Add to the values (on by default). |
| `rotateRatio` | `rr` | Multiplies the angle (1). |
| `rotation` | `rot` | Editing rotation: -a says how much. |
| `scale` | `s` | Editing scale: -su and -sv say how much. |
| `scaleU` | `su` | Scale in U. |
| `scaleV` | `sv` | Scale in V. |
| `uValue` | `u` | U. |
| `vValue` | `v` | V. |

## nurbsPlane

Create a NURBS plane with a makeNurbPlane node. Returns [nurbsPlane1, makeNurbPlane1].

| Flag | Short | Description |
| --- | --- | --- |
| `axis` | `ax` | The axis (1 0 0). |
| `pivot` | `p` | The centre. |
| `constructionHistory` | `ch` | Keep the make node (on by default). |
| `name` | `n` | Name of the transform. |
| `object` | `o` | Accepted for existing scripts. |
| `polygon` | `po` | 1 makes polygons. |
| `width` | `w` | The width (1). |
| `lengthRatio` | `lr` | Length over width (1). |
| `degree` | `d` | The degree (3). |
| `patchesU` | `u` | Patches along U (1). |
| `patchesV` | `v` | Patches along V (1). |

## nurbsSelect

Pick a surface's CVs: -bs the border, -lb -rb -tb -bb one side, -gs -ss grow or shrink what's picked.

Arguments: surfaces (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `borderSelection` | `bs` | The whole border. |
| `leftBorder` | `lb` | The first row in U. |
| `rightBorder` | `rb` | The last row in U. |
| `bottomBorder` | `bb` | The first row in V. |
| `topBorder` | `tb` | The last row in V. |
| `growSelection` | `gs` | Rings to add. |
| `shrinkSelection` | `ss` | Rings to take off. |

## nurbsSquare

Create a square curve (-nr normal, -c centre, -sl1 side). Returns [nurbsSquare1, makeNurbsSquare1].

| Flag | Short | Description |
| --- | --- | --- |
| `normal` | `nr` | The normal. |
| `center` | `c` | The centre. |
| `sideLength1` | `sl1` | The side (1). |
| `name` | `n` | Name of the transform. |

## nurbsToPoly

Convert surfaces to polygons with a nurbsTessellate node. -f 0 count (-pc), 1 standard fit (-ft), 2 general (-ut -un -vt -vn), 3 CVs; -pt 0 triangles, 1 quads. Returns [nurbsToPoly1, nurbsTessellate1].

Arguments: surfaces (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `format` | `f` | 0 count, 1 standard fit, 2 general, 3 CVs (1). |
| `polygonType` | `pt` | 0 triangles, 1 quads (0). |
| `polygonCount` | `pc` | Polygons, for -f 0 (200). |
| `fractionalTolerance` | `ft` | Tolerance for -f 1 (0.01). |
| `uType` | `ut` | 1 per surface in 3D, 2 per surface, 3 per span (3). |
| `uNumber` | `un` | How many along U (3). |
| `vType` | `vt` | 1 per surface in 3D, 2 per surface, 3 per span (3). |
| `vNumber` | `vn` | How many along V (3). |
| `chordHeightRatio` | `chr` | Accepted for existing scripts. |
| `minEdgeLength` | `mel` | Accepted for existing scripts. |
| `delta` | `d` | Accepted for existing scripts. |
| `useChordHeight` | `uch` | Accepted for existing scripts. |
| `useChordHeightRatio` | `ucr` | Accepted for existing scripts. |
| `chordHeight` | `cht` | Accepted for existing scripts. |
| `edgeSwap` | `es` | Accepted for existing scripts. |
| `normalizeTrimmedUVRange` | `ntr` | Accepted for existing scripts. |
| `matchRenderTessellation` | `mrt` | Accepted for existing scripts. |
| `useSurfaceShader` | `uss` | Accepted for existing scripts. |
| `matchNormalDir` | `mnd` | Accepted for existing scripts. |
| `constructionHistory` | `ch` | Keep the node (on by default). |
| `name` | `n` | Name of the mesh. |

## nurbsUVSet

Explicit UVs on a surface: -c makes the set from the implicit ones, -ue switches to it. Returns true. Less than the reference: one set per surface, the implicit values are evenly spaced by CV rather than taken from the knots, and nothing draws from them yet.

Arguments: a surface (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `create` | `c` | Make the explicit set. |
| `useExplicit` | `ue` | Use the explicit set. |

## objExists

True if a node with this name or path exists.

Arguments: name

## objectCenter

An object's bounding box centre: world (-gl, the default) or -l its own space; -x -y -z one value.

Arguments: object

| Flag | Short | Description |
| --- | --- | --- |
| `gl` | `gl` | World space (the default). |
| `local` | `l` | The object's space. |
| `x` | `x` | X alone. |
| `y` | `y` | Y alone. |
| `z` | `z` | Z alone. |

## objectType

The node type of an object (objectType -isType checks one).

Arguments: object

| Flag | Short | Description |
| --- | --- | --- |
| `isAType` | `isa` | Whether it is this type or derived from it. |
| `isType` | `i` | Whether it is exactly this type. |

## objectTypeUI

The type of an interface element by name; -isType tests it, -listAll lists the types.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `isType` | `i` |  |
| `listAll` | `la` |  |
| `superClasses` | `sc` |  |

## offsetCurve

A copy offset in the curve's plane: -d distance, -nr normal. Returns [curve, node].

Arguments: curves (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `distance` | `d` | How far. |
| `normal` | `nr` | The plane's normal. |
| `useGivenNormal` | `ugn` | Accepted for existing scripts. |
| `tolerance` | `tol` | Accepted for existing scripts. |
| `constructionHistory` | `ch` | Keep a history node (on by default). |
| `replaceOriginal` | `rpo` | Change the curve itself (on by default). |
| `name` | `n` | Name of a new curve. |
| `object` | `o` | Make an object (accepted for existing scripts). |

## offsetCurveOnSurface

A curve on a surface offset sideways: -d distance in parameter space. Returns [surface->curve, node].

Arguments: a curve on a surface

| Flag | Short | Description |
| --- | --- | --- |
| `distance` | `d` | How far. |
| `constructionHistory` | `ch` | Keep a history node (on by default). |
| `replaceOriginal` | `rpo` | Change the curve itself (on by default). |
| `name` | `n` | Name of a new curve. |
| `object` | `o` | Make an object (accepted for existing scripts). |

## offsetSurface

Move a surface along its normals by -d. Returns [surface, node].

Arguments: surfaces (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `distance` | `d` | How far (1). |
| `method` | `m` | Accepted for existing scripts. |
| `constructionHistory` | `ch` | Keep a history node (on by default). |
| `replaceOriginal` | `rpo` | Change the surface itself (on by default). |
| `name` | `n` | Name of a new surface. |
| `object` | `o` | Accepted for existing scripts. |
| `polygon` | `po` | Accepted for existing scripts. |

## optionMenu

Script UI: optionMenu, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `label` | `l` |  |
| `value` | `v` |  |
| `select` | `sl` |  |
| `numberOfItems` | `ni` |  |
| `itemListLong` | `ill` |  |
| `itemListShort` | `ils` |  |
| `changeCommand` | `cc` |  |
| `alwaysCallChangeCommand` | `acc` |  |
| `deleteAllItems` | `dai` |  |
| `beforeShowPopup` | `bsp` |  |
| `postMenuCommand` | `pmc` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## optionMenuGrp

Script UI: optionMenuGrp, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `label` | `l` |  |
| `extraLabel` | `el` |  |
| `value` | `v` |  |
| `select` | `sl` |  |
| `numberOfItems` | `ni` |  |
| `itemListLong` | `ill` |  |
| `itemListShort` | `ils` |  |
| `changeCommand` | `cc` |  |
| `alwaysCallChangeCommand` | `acc` |  |
| `deleteAllItems` | `dai` |  |
| `columnWidth1` | `cw1` |  |
| `columnAlign1` | `cl1` |  |
| `columnAttach1` | `ct1` |  |
| `columnOffset1` | `co1` |  |
| `columnWidth2` | `cw2` |  |
| `columnAlign2` | `cl2` |  |
| `columnAttach2` | `ct2` |  |
| `columnOffset2` | `co2` |  |
| `adjustableColumn2` | `ad2` |  |
| `columnWidth3` | `cw3` |  |
| `columnAlign3` | `cl3` |  |
| `columnAttach3` | `ct3` |  |
| `columnOffset3` | `co3` |  |
| `adjustableColumn3` | `ad3` |  |
| `columnWidth4` | `cw4` |  |
| `columnAlign4` | `cl4` |  |
| `columnAttach4` | `ct4` |  |
| `columnOffset4` | `co4` |  |
| `adjustableColumn4` | `ad4` |  |
| `columnWidth5` | `cw5` |  |
| `columnAlign5` | `cl5` |  |
| `columnAttach5` | `ct5` |  |
| `columnOffset5` | `co5` |  |
| `adjustableColumn5` | `ad5` |  |
| `columnWidth6` | `cw6` |  |
| `columnAlign6` | `cl6` |  |
| `columnAttach6` | `ct6` |  |
| `columnOffset6` | `co6` |  |
| `adjustableColumn6` | `ad6` |  |
| `adjustableColumn` | `adj` |  |
| `columnWidth` | `cw` |  |
| `columnAlign` | `cal` |  |
| `columnAttach` | `cat` |  |
| `rowAttach` | `rat` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## optionVar

Named values that last the session: -iv, -fv, -sv, -q, -ex, -rm.

Arguments: a name with -q; a value in MEL

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | The value of the name given. |
| `exists` | `ex` | Whether this name is set. |
| `remove` | `rm` | Unset this name. |
| `clearArray` | `ca` | Empty this array. |
| `list` | `l` | Every name. |
| `intValue` | `iv` | Name and value. |
| `floatValue` | `fv` | Name and value. |
| `stringValue` | `sv` | Name and value. |
| `intValueAppend` | `iva` | Name and value. |
| `floatValueAppend` | `fva` | Name and value. |
| `stringValueAppend` | `sva` | Name and value. |

## orbit

Turn a camera about its centre: -rv right and up degrees, -ha -va, -pv the point to turn about.

Arguments: [camera]

| Flag | Short | Description |
| --- | --- | --- |
| `rotationAngles` | `ra` |  |
| `horizontalAngle` | `ha` |  |
| `verticalAngle` | `va` |  |
| `pivotPoint` | `pp` |  |

## orbitCtx

A orbitCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `alternateContext` | `ac` |  |
| `localOrbit` | `lo` |  |
| `orbitScale` | `os` |  |
| `toolName` | `tn` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## orientConstraint

Constrain the object named (or selected) last to the targets before it; a second call adds targets. -mo keeps the current offset; -w sets the targets' weight; -e and -q work on the constraint or the object it drives. Returns [constraint].

Arguments: targets, then the object to drive or the constraint

| Flag | Short | Description |
| --- | --- | --- |
| `maintainOffset` | `mo` | Keep the object where it is now relative to the targets. |
| `weight` | `w` | Weight of the targets named (1). |
| `name` | `n` | Name of the constraint. |
| `offset` | `o` | Added to the result. |
| `remove` | `rm` | Remove the targets named. Removing the last deletes the constraint. |
| `targetList` | `tl` | Query: the targets. |
| `weightAliasList` | `wal` | Query: the targets' weight attributes. |
| `aimVector` | `aim` | The local axis that points at the target. |
| `upVector` | `u` | The local axis that points up. |
| `worldUpVector` | `wu` | The world direction up means. |
| `worldUpType` | `wut` | scene, object, objectrotation, vector or none. |
| `worldUpObject` | `wuo` | The object for worldUpType object and objectrotation. |
| `interpType` | `int` | How rotations blend: noflip, average, shortest, longest, cache, or 0 to 4. |
| `skipTranslate` | `st` | Translate axes to leave alone: x, y, z or none. May be used more than once. |
| `skipRotate` | `sr` | Rotate axes to leave alone: x, y, z or none. May be used more than once. |
| `skip` | `sk` | Axes of this constraint's channel to leave alone: x, y, z or none. May be used more than once. |
| `skipScale` |  | Scale axes to leave alone: x, y, z or none. |
| `edit` | `e` | Change the constraint. |
| `query` | `q` | Ask about the constraint. |

## orientJoint

Orient joints: the first axis of the order aims at the child joint, the second toward -sao. Joints below follow unless -ch 0.

Arguments: order (xyz), then joints

| Flag | Short | Description |
| --- | --- | --- |
| `secondaryAxisOrient` | `sao` | yup, zup, xdown... |
| `zeroScaleOrient` | `zso` | Set rotateAxis to zero; jointOrient takes the change. |
| `children` | `ch` | The joints below too (default on). |

## outlinerEditor

Script UI: outlinerEditor, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `alwaysToggleSelect` | `ats` |  |
| `animLayerFilterOptions` | `alf` |  |
| `attrAlphaOrder` | `aao` |  |
| `attrFilter` | `af` |  |
| `autoExpand` | `xpd` |  |
| `autoExpandLayers` | `ael` |  |
| `autoSelectNewObjects` | `aso` |  |
| `containersIgnoreFilters` | `cif` |  |
| `directSelect` | `ds` |  |
| `displayMode` | `dm` |  |
| `doNotSelectNewObjects` | `dns` |  |
| `dropIsParent` | `dip` |  |
| `editAttrName` | `ean` |  |
| `expandAllItems` | `eai` |  |
| `expandAllSelectedItems` | `eas` |  |
| `expandAttribute` | `xta` |  |
| `expandConnections` | `xc` |  |
| `expandObjects` | `eo` |  |
| `feedbackItemName` | `fbn` |  |
| `feedbackRowNumber` | `fbr` |  |
| `getCurrentSetOfItem` | `gcs` |  |
| `highlightActive` | `ha` |  |
| `highlightSecondary` | `hs` |  |
| `ignoreDagHierarchy` | `hir` |  |
| `ignoreHiddenAttribute` | `iha` |  |
| `ignoreOutlinerColor` | `ioc` |  |
| `isChildSelected` | `ics` |  |
| `isSet` | `isSet` |  |
| `isSetMember` | `ism` |  |
| `lineHeight` | `lh` |  |
| `longNames` | `ln` |  |
| `niceNames` | `nn` |  |
| `object` | `obj` |  |
| `organizeByClip` | `obc` |  |
| `organizeByLayer` | `obl` |  |
| `parentObject` | `po` |  |
| `pinPlug` | `pin` |  |
| `refresh` | `rfs` |  |
| `removeFromCurrentSet` | `rcs` |  |
| `renameItem` | `rni` |  |
| `renameSelectedItem` | `rsi` |  |
| `renderFilterActive` | `rfa` |  |
| `renderFilterIndex` | `rfi` |  |
| `renderFilterVisible` | `rfv` |  |
| `selectCommand` | `sec` |  |
| `setDefaultFilter` | `sdf` |  |
| `setFilter` | `sf` |  |
| `setsIgnoreFilters` | `sif` |  |
| `showAnimCurvesOnly` | `aco` |  |
| `showAnimLayerWeight` | `salw` |  |
| `showAssets` | `a` |  |
| `showAssignedMaterials` | `am` |  |
| `showAttrValues` | `av` |  |
| `showAttributes` | `sat` |  |
| `showCompounds` | `cmp` |  |
| `showConnected` | `sc` |  |
| `showContainedOnly` | `sco` |  |
| `showContainerContents` | `scc` |  |
| `showDagOnly` | `dag` |  |
| `showLeafs` | `laf` |  |
| `showMuteInfo` | `smi` |  |
| `showNamespace` | `sn` |  |
| `showNumericAttrsOnly` | `num` |  |
| `showParentContainers` | `spc` |  |
| `showPinIcons` | `spi` |  |
| `showPublishedAsConnected` | `spu` |  |
| `showReferenceMembers` | `srm` |  |
| `showReferenceNodes` | `rn` |  |
| `showSelected` | `ssd` |  |
| `showSetMembers` | `ssm` |  |
| `showShapes` | `shp` |  |
| `showTextureNodesOnly` | `tno` |  |
| `showTimeEditor` | `ste` |  |
| `showUVAttrsOnly` | `uv` |  |
| `showUnitlessCurves` | `su` |  |
| `showUpstreamCurves` | `suc` |  |
| `sortOrder` | `so` |  |
| `transmitFilters` | `tf` |  |
| `control` | `ctl` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `filter` | `f` |  |
| `forceMainConnection` | `fmc` |  |
| `highlightConnection` | `hlc` |  |
| `lockMainConnection` | `lck` |  |
| `mainListConnection` | `mlc` |  |
| `panel` | `pnl` |  |
| `selectionConnection` | `slc` |  |
| `stateString` | `sts` |  |
| `unParent` | `up` |  |
| `unlockMainConnection` | `ulk` |  |
| `updateMainConnection` | `upd` |  |
| `useTemplate` | `ut` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## outlinerPanel

A panel holding an editor of its kind: the editor has the panel's name.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `outlinerEditor` | `oe` |  |
| `label` | `l` |  |
| `parent` | `p` |  |
| `control` | `ctl` |  |
| `copy` | `cp` |  |
| `createString` | `cs` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `editString` | `es` |  |
| `exists` | `ex` |  |
| `init` | `in` |  |
| `isUnique` | `iu` |  |
| `menuBarVisible` | `mbv` |  |
| `needsInit` | `ni` |  |
| `popupMenuProcedure` | `pmp` |  |
| `replacePanel` | `rp` |  |
| `tearOff` | `to` |  |
| `tearOffCopy` | `toc` |  |
| `unParent` | `up` |  |
| `useTemplate` | `ut` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## pairBlend

Put a pairBlend on translates or rotates: -nd node -at attr. What drove the attribute feeds input 1. Returns the node; -q -in1 -in2 -at read its connections.

Arguments: pairBlend, with -q

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `node` | `nd` | The node driven. |
| `attribute` | `at` | The attributes. |
| `input1` | `i1` | With -q: what feeds the first pair. |
| `input2` | `i2` | With -q: what feeds the second pair. |
| `name` | `n` | The node's name. |

## panZoom

panZoom [camera]: -left -right -up -down slide the camera, -zoomRatio scales what it sees. It moves the camera; there is no 2D film pan here.

Arguments: [camera]

| Flag | Short | Description |
| --- | --- | --- |
| `left` | `l` |  |
| `right` | `r` |  |
| `up` | `u` |  |
| `down` | `d` |  |
| `zoomRatio` | `zr` |  |
| `absolute` | `abs` |  |
| `relative` | `rel` |  |

## paneLayout

Script UI: paneLayout, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `configuration` | `cn` |  |
| `paneSize` | `ps` |  |
| `separatorThickness` | `st` |  |
| `separatorMovedCommand` | `smc` |  |
| `setPane` | `sp` |  |
| `activePane` | `ap` |  |
| `activePaneIndex` | `api` |  |
| `numberOfVisiblePanes` | `nvp` |  |
| `staticWidthPane` | `swp` |  |
| `staticHeightPane` | `shp` |  |
| `paneUnderPointer` | `pup` |  |
| `pane1` | `p1` |  |
| `pane2` | `p2` |  |
| `pane3` | `p3` |  |
| `pane4` | `p4` |  |
| `childArray` | `ca` |  |
| `numberOfChildren` | `nch` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## panel

Any panel: -ex, -l, -ctl, -p.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `label` | `l` |  |
| `parent` | `p` |  |
| `control` | `ctl` |  |
| `copy` | `cp` |  |
| `createString` | `cs` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `editString` | `es` |  |
| `exists` | `ex` |  |
| `init` | `in` |  |
| `isUnique` | `iu` |  |
| `menuBarVisible` | `mbv` |  |
| `needsInit` | `ni` |  |
| `popupMenuProcedure` | `pmp` |  |
| `replacePanel` | `rp` |  |
| `tearOff` | `to` |  |
| `tearOffCopy` | `toc` |  |
| `unParent` | `up` |  |
| `useTemplate` | `ut` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## panelConfiguration

A named arrangement of panels: -addPanel (fixed, type, label, create, edit), -configString, -label; getPanel -allConfigs lists them.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `addPanel` | `ap` |  |
| `configString` | `cfs` |  |
| `defaultImage` | `dimg` |  |
| `image` | `img` |  |
| `label` | `l` |  |
| `labelStrings` | `lbl` |  |
| `createStrings` | `ccs` |  |
| `editStrings` | `ecs` |  |
| `typeStrings` | `tys` |  |
| `isFixedState` | `ifs` |  |
| `numberOfPanels` | `np` |  |
| `removeAllPanels` | `rap` |  |
| `removeLastPanel` | `rlp` |  |
| `replaceCreateString` | `rcs` |  |
| `replaceEditString` | `res` |  |
| `replaceFixedState` | `rfs` |  |
| `replaceLabel` | `rl` |  |
| `replaceTypeString` | `rts` |  |
| `sceneConfig` | `sc` |  |
| `userCreated` | `uc` |  |
| `defineTemplate` | `dt` |  |
| `useTemplate` | `ut` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## paramDimension

Label a point with its parameter: curve1.u[0.5] or surface1.uv[0.5][0.5]. Returns the shape.

Arguments: point

## parent

parent(child..., parent), or parent(child..., world=True). Keeps world transforms unless relative=True. chain=True parents each object under the next one, in order: (a, b, c) puts a under b and b under c. index=N places the objects, in order, at position N among the parent's children (0 is first), which also reorders objects already under that parent. Leaves the moved objects selected.

Arguments: objects... parent

| Flag | Short | Description |
| --- | --- | --- |
| `world` | `w` | Unparent to the world. |
| `relative` | `r` | Keep local transforms instead of world. |
| `chain` | `ch` | Parent in selection order, each under the next. |
| `index` | `i` | Position among the parent's children, counted without the moved objects. |
| `absolute` | `a` | Keep world placement (the default). |
| `addObject` | `add` | Add as an instance under the parent. |
| `removeObject` | `rm` | Remove this instance. |
| `shape` | `s` | Move the shapes, not the transforms. |
| `noConnections` | `nc` | Accepted for existing scripts. |
| `noInvScale` | `nis` | Accepted for existing scripts. |

## parentConstraint

Constrain the object named (or selected) last to the targets before it; a second call adds targets. -mo keeps the current offset; -w sets the targets' weight; -e and -q work on the constraint or the object it drives. Returns [constraint].

Arguments: targets, then the object to drive or the constraint

| Flag | Short | Description |
| --- | --- | --- |
| `maintainOffset` | `mo` | Keep the object where it is now relative to the targets. |
| `weight` | `w` | Weight of the targets named (1). |
| `name` | `n` | Name of the constraint. |
| `offset` | `o` | Added to the result. |
| `remove` | `rm` | Remove the targets named. Removing the last deletes the constraint. |
| `targetList` | `tl` | Query: the targets. |
| `weightAliasList` | `wal` | Query: the targets' weight attributes. |
| `aimVector` | `aim` | The local axis that points at the target. |
| `upVector` | `u` | The local axis that points up. |
| `worldUpVector` | `wu` | The world direction up means. |
| `worldUpType` | `wut` | scene, object, objectrotation, vector or none. |
| `worldUpObject` | `wuo` | The object for worldUpType object and objectrotation. |
| `interpType` | `int` | How rotations blend: noflip, average, shortest, longest, cache, or 0 to 4. |
| `skipTranslate` | `st` | Translate axes to leave alone: x, y, z or none. May be used more than once. |
| `skipRotate` | `sr` | Rotate axes to leave alone: x, y, z or none. May be used more than once. |
| `skip` | `sk` | Axes of this constraint's channel to leave alone: x, y, z or none. May be used more than once. |
| `skipScale` |  | Scale axes to leave alone: x, y, z or none. |
| `edit` | `e` | Change the constraint. |
| `query` | `q` | Ask about the constraint. |

## particle

Make particles: -p x y z for each one, or a grid with -ll, -ur and -grs. -q -ct counts them; -q -at position -id 3 reads one. Returns [transform, shape].

Arguments: particles (query)

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query a value. |
| `edit` | `e` | Edit an existing node. |
| `name` | `n` | Name of the new node. |
| `position` | `p` | A particle there: x y z. Repeat it. |
| `lowerLeft` | `ll` | Grid: one corner. |
| `upperRight` | `ur` | Grid: the other corner. |
| `gridSpacing` | `grs` | Grid: spacing. |
| `conserve` | `c` | Accepted for old scripts. |
| `count` | `ct` | Query: how many. |
| `attribute` | `at` | Query: a per particle value. |
| `id` | `id` | Query: which particle. |
| `solver` | `sv` | The solver to join. |

## pasteKey

Paste the key clipboard onto channels at -t (the current time by default). -o insert (the default) moves later keys along, replace clears the pasted range, replaceCompletely clears the curve, merge overlays. -to and -vo offset, -cp repeats.

Arguments: objects or plugs (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `attribute` | `at` | Channel name(s). |
| `time` | `t` | Where to paste. |
| `option` | `o` | insert, replace, replaceCompletely or merge. |
| `timeOffset` | `to` | Added to the paste time. |
| `valueOffset` | `vo` | Added to the values. |
| `copies` | `cp` | How many times to paste. |
| `connect` | `c` | Accepted for existing scripts. |
| `clipboard` | `cb` | Accepted for existing scripts. |

## pathAnimation

Put objects on a curve: a motionPath node drives their translate, and with -f their rotate. -stu and -etu key uValue from -su to -eu (the curve's ends). Returns the motion path. -q and -e take the motion path or its object.

Arguments: objects (the selection when none); without -c the last is the curve

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Return the value of the flags named instead of setting them. |
| `edit` | `e` | Change the flags named. |
| `curve` | `c` | The path. |
| `name` | `n` | The motion path's name. |
| `follow` | `f` | Turn objects along the path. |
| `fractionMode` | `fm` | U is a fraction of the length. |
| `followAxis` | `fa` | The object's axis along the path: x, y or z (x). |
| `upAxis` | `ua` | The object's axis towards up: x, y or z (y). |
| `worldUpType` | `wut` | scene, object, objectrotation, vector or normal. |
| `worldUpVector` | `wu` | The up direction. |
| `worldUpObject` | `wuo` | The up object. |
| `useNormal` | `un` | Up is the curve's normal. |
| `inverseUp` | `iu` | The up axis points away from up. |
| `inverseFront` | `if` | The front axis points back along the path. |
| `bank` | `b` | Lean into turns. |
| `bankScale` | `bs` | How far it leans. |
| `bankThreshold` | `bt` | The most it leans, degrees. |
| `bankLimit` | `bl` | As -bt. |
| `startTimeU` | `stu` | The frame it starts. |
| `endTimeU` | `etu` | The frame it ends. |
| `startU` | `su` | The U it starts at (the curve's start). |
| `endU` | `eu` | The U it ends at (the curve's end). |

## percent

Set or query deformer weights of points: percent -v 0.5 cluster1 pCube1.vtx[0:3]. -m multiplies what is there. -q -v returns one weight per point. With no points named: the selected components, else all the deformer's points.

Arguments: the deformer, then points or meshes

| Flag | Short | Description |
| --- | --- | --- |
| `edit` | `e` | Edit mode. |
| `query` | `q` | Query mode. |
| `value` | `v` | The weight. |
| `multiply` | `m` | Multiply the weights by the value instead of setting them. |

## pickWalk

Walk the selection through the hierarchy like the arrow keys: up to the parent, down to the first child, left/right to the previous/next sibling (wrapping). Right after a delete, with nothing selected since, it walks from where the deleted objects were.

| Flag | Short | Description |
| --- | --- | --- |
| `direction` | `d` | up, down, left or right. |
| `type` | `typ` | Accepted for existing scripts. |

## picture

Script UI: picture, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `image` | `i` |  |
| `tile` | `tl` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## pivot

World position of an object's rotate pivot.

Arguments: object

## pixelMove

pixelMove x y: moves the selection this many pixels in the view's plane.

Arguments: x y

## planar

A surface filling a closed curve: a rectangle makes a plain patch, anything else a plane trimmed to it; -po 1 a polygon face. Returns [planarTrimmedSurface1, planarTrim1].

Arguments: a closed curve (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `degree` | `d` | Accepted for existing scripts. |
| `tolerance` | `tol` | Accepted for existing scripts. |
| `polygon` | `po` | 0 NURBS (the default), 1 polygons. |
| `constructionHistory` | `ch` | Keep the node (on by default). |
| `range` | `rn` | Accepted for existing scripts. |
| `name` | `n` | Name of the new object. |

## planarSrf

A trimmed plane in a closed curve: planar under its node's name.

Arguments: a closed curve (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `degree` | `d` | Accepted for existing scripts. |
| `tolerance` | `tol` | Accepted for existing scripts. |
| `polygon` | `po` | 0 NURBS (the default), 1 polygons. |
| `constructionHistory` | `ch` | Keep the node (on by default). |
| `range` | `rn` | Accepted for existing scripts. |
| `name` | `n` | Name of the new object. |

## plane

A construction plane: -s size, -w -lr each way, -p where, -r rotation. Returns its transform.

| Flag | Short | Description |
| --- | --- | --- |
| `size` | `s` | Its size (1). |
| `width` | `w` | Its width. |
| `length` | `l` | Its length. |
| `position` | `p` | Where. |
| `rotation` | `r` | How turned, degrees. |
| `name` | `n` | Its name. |

## play

Start or stop playback: -st on or off, -f forward or back. -w plays the range through once before returning. -q -st says whether it is playing.

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Return the value of the flags named instead of setting them. |
| `edit` | `e` | Change the flags named. |
| `forward` | `f` | Play forwards (on by default). |
| `state` | `st` | Playing or not. |
| `wait` | `w` | Play through before returning. |
| `record` | `rec` | Accepted for existing scripts: there are no dynamics to record. |
| `sound` | `s` | Accepted for existing scripts. |

## playbackOptions

The playback range (min/max), the whole animation range (ast/aet), looping and speed. In query mode returns the flag asked about.

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Return the value of the flags named instead of setting them. |
| `edit` | `e` | Change the flags named. |
| `minTime` | `min` | First frame of the playback range. |
| `maxTime` | `max` | Last frame of the playback range. |
| `animationStartTime` | `ast` | First frame of the whole animation. |
| `animationEndTime` | `aet` | Last frame of the whole animation. |
| `loop` | `l` | continuous, once or oscillate. |
| `playbackSpeed` | `ps` | 1 for real time, 0 for every frame. |
| `maxPlaybackSpeed` | `mps` | The most frames per second playback may reach, 0 for no limit. |
| `framesPerSecond` | `fps` | Frames per second (24 by default, film). |
| `by` | `by` | Frames between played frames. |
| `view` | `v` | Which views update during playback: all or active. |
| `preroll` | `pp` | Stored for scripts that set it. |

## playblast

Write the view's frames as an image sequence: -f name, -fmt image, -st -et or -fr, -wh, -p percent, -fp padding, -fo overwrite, -c png jpg bmp tif. Returns the file pattern.

| Flag | Short | Description |
| --- | --- | --- |
| `filename` | `f` |  |
| `format` | `fmt` |  |
| `startTime` | `st` |  |
| `endTime` | `et` |  |
| `widthHeight` | `wh` |  |
| `width` | `w` |  |
| `height` | `h` |  |
| `framePadding` | `fp` |  |
| `percent` | `p` |  |
| `compression` | `c` |  |
| `quality` | `qlt` |  |
| `frame` | `fr` |  |
| `editorPanelName` | `epn` |  |
| `sound` | `s` |  |
| `sequenceTime` | `sqt` |  |
| `completeFilename` | `cf` |  |
| `indexFromZero` | `ifz` |  |
| `viewer` | `v` |  |
| `forceOverwrite` | `fo` |  |
| `clearCache` | `cc` |  |
| `showOrnaments` | `orn` |  |
| `offScreen` | `os` |  |
| `offScreenViewportUpdate` | `ofv` |  |
| `activeEditor` | `ae` |  |
| `rawFrameNumbers` | `rfn` |  |
| `saveFilmGate` | `sfg` |  |
| `query` | `q` |  |

## pluginInfo

Ask about plug-ins: pluginInfo -q -loaded name, -listPlugins, -version, -vendor, -path, -command, -dependNode.

Arguments: plug-in name

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query. |
| `edit` | `e` | Accepted for scripts. |
| `loaded` | `l` | Whether it is loaded. |
| `registered` | `r` | Whether it is known. |
| `listPlugins` | `ls` | The loaded plug-ins. |
| `version` | `v` | Its version. |
| `vendor` | `vd` | Its vendor. |
| `path` | `p` | Its file. |
| `name` | `n` | Its name. |
| `command` | `c` | The commands it registered. |
| `dependNode` | `dn` | The node types it registered. |
| `apiVersion` | `av` | The API version it asked for. |
| `autoload` | `a` | Whether it loads at start: -e -autoload true name sets it, -q -autoload name asks. |
| `listPluginsPath` | `lsp` | The folders plug-ins are looked for in. |

## pointConstraint

Constrain the object named (or selected) last to the targets before it; a second call adds targets. -mo keeps the current offset; -w sets the targets' weight; -e and -q work on the constraint or the object it drives. Returns [constraint].

Arguments: targets, then the object to drive or the constraint

| Flag | Short | Description |
| --- | --- | --- |
| `maintainOffset` | `mo` | Keep the object where it is now relative to the targets. |
| `weight` | `w` | Weight of the targets named (1). |
| `name` | `n` | Name of the constraint. |
| `offset` | `o` | Added to the result. |
| `remove` | `rm` | Remove the targets named. Removing the last deletes the constraint. |
| `targetList` | `tl` | Query: the targets. |
| `weightAliasList` | `wal` | Query: the targets' weight attributes. |
| `aimVector` | `aim` | The local axis that points at the target. |
| `upVector` | `u` | The local axis that points up. |
| `worldUpVector` | `wu` | The world direction up means. |
| `worldUpType` | `wut` | scene, object, objectrotation, vector or none. |
| `worldUpObject` | `wuo` | The object for worldUpType object and objectrotation. |
| `interpType` | `int` | How rotations blend: noflip, average, shortest, longest, cache, or 0 to 4. |
| `skipTranslate` | `st` | Translate axes to leave alone: x, y, z or none. May be used more than once. |
| `skipRotate` | `sr` | Rotate axes to leave alone: x, y, z or none. May be used more than once. |
| `skip` | `sk` | Axes of this constraint's channel to leave alone: x, y, z or none. May be used more than once. |
| `skipScale` |  | Scale axes to leave alone: x, y, z or none. |
| `edit` | `e` | Change the constraint. |
| `query` | `q` | Ask about the constraint. |

## pointCurveConstraint

Make a curve pass through a point at curve.u[x] or curve.ep[i]: -p position (where it is now), -w weight. Returns [curve, node].

Arguments: a point on a curve

| Flag | Short | Description |
| --- | --- | --- |
| `position` | `p` | Where it goes. |
| `weight` | `w` | How much (1). |
| `pointWeight` | `pw` | Accepted for existing scripts. |
| `constructionHistory` | `ch` | Keep the node (on by default). |
| `replaceOriginal` | `rpo` | Accepted: the curve itself changes. |
| `name` | `n` | Accepted for existing scripts. |
| `object` | `o` | Accepted for existing scripts. |

## pointLight

Create a point light, or query and edit one. Returns its shape.

Arguments: the light, to edit or query

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `name` | `n` | Its name. |
| `rgb` | `rgb` | Colour. |
| `shadowColor` | `sc` | Shadow colour. |
| `position` | `pos` | Position. |
| `rotation` | `rot` | Rotation. |
| `exclusive` | `exc` | Out of defaultLightSet: lights only what it's linked to. |
| `shadowDither` | `sd` | Accepted for scripts. |
| `intensity` | `i` | intensity |
| `shadowSamples` | `sh` | shadowRays |
| `useRayTraceShadows` | `rs` | useRayTraceShadows |
| `decayRate` | `d` | decayRate |
| `softShadow` | `ss` | castSoftShadows |
| `discRadius` | `drs` | lightRadius |

## pointOnCurve

Facts at a parameter of a curve: -pr parameter (-top a fraction of its length), asking -p position, -t tangent, -nt unit tangent (position when none).

Arguments: a curve (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `parameter` | `pr` | The parameter. |
| `turnOnPercentage` | `top` | -pr is a fraction 0 to 1 of the length. |
| `position` | `p` | Ask for the point. |
| `tangent` | `t` | Ask for the tangent. |
| `normalizedTangent` | `nt` | Ask for the unit tangent. |
| `constructionHistory` | `ch` | Accepted for existing scripts. |

## pointOnCurveDeformer

Point on Curve: points go with one place on a curve, by how far it has moved. Returns [pointOnCurve1].

Arguments: what to deform, and any target last (the selection when none); the deformer with -e and -q

| Flag | Short | Description |
| --- | --- | --- |
| `name` | `n` | Name of the deformer. |
| `before` | `bf` | Go just above the deformer nearest the mesh. |
| `after` | `af` | Go nearest the mesh, after the other deformers (the default). |
| `frontOfChain` | `foc` | Go above every other deformer. |
| `envelope` | `en` | How much of the deformation applies (1). |
| `edit` | `e` | Edit mode. |
| `query` | `q` | Query mode. |
| `curve` | `c` | The curve; or name it last. |
| `parameter` | `pr` | The place on the curve. |
| `turnOnPercentage` | `top` | The parameter is a fraction of the length. |

## pointOnPolyConstraint

Constrain the object named (or selected) last to the targets before it; a second call adds targets. -mo keeps the current offset; -w sets the targets' weight; -e and -q work on the constraint or the object it drives. Returns [constraint].

Arguments: targets, then the object to drive or the constraint

| Flag | Short | Description |
| --- | --- | --- |
| `maintainOffset` | `mo` | Keep the object where it is now relative to the targets. |
| `weight` | `w` | Weight of the targets named (1). |
| `name` | `n` | Name of the constraint. |
| `offset` | `o` | Added to the result. |
| `remove` | `rm` | Remove the targets named. Removing the last deletes the constraint. |
| `targetList` | `tl` | Query: the targets. |
| `weightAliasList` | `wal` | Query: the targets' weight attributes. |
| `aimVector` | `aim` | The local axis that points at the target. |
| `upVector` | `u` | The local axis that points up. |
| `worldUpVector` | `wu` | The world direction up means. |
| `worldUpType` | `wut` | scene, object, objectrotation, vector or none. |
| `worldUpObject` | `wuo` | The object for worldUpType object and objectrotation. |
| `interpType` | `int` | How rotations blend: noflip, average, shortest, longest, cache, or 0 to 4. |
| `skipTranslate` | `st` | Translate axes to leave alone: x, y, z or none. May be used more than once. |
| `skipRotate` | `sr` | Rotate axes to leave alone: x, y, z or none. May be used more than once. |
| `skip` | `sk` | Axes of this constraint's channel to leave alone: x, y, z or none. May be used more than once. |
| `skipScale` |  | Scale axes to leave alone: x, y, z or none. |
| `edit` | `e` | Change the constraint. |
| `query` | `q` | Ask about the constraint. |

## pointOnSurface

Facts at parameters of a surface, world space: -u -v (-top fractions of the range), asking -p position, -no normal, -nn unit normal, -tu -tv tangents, -ntu -ntv unit tangents (position when none).

Arguments: a surface (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `parameterU` | `u` | The U parameter. |
| `parameterV` | `v` | The V parameter. |
| `turnOnPercentage` | `top` | The parameters are fractions 0 to 1 of the range. |
| `position` | `p` | Ask for the point. |
| `normal` | `no` | Ask for the normal. |
| `normalizedNormal` | `nn` | Ask for the unit normal. |
| `tangentU` | `tu` | Ask for the U derivative. |
| `tangentV` | `tv` | Ask for the V derivative. |
| `normalizedTangentU` | `ntu` | Ask for the unit U tangent. |
| `normalizedTangentV` | `ntv` | Ask for the unit V tangent. |
| `constructionHistory` | `ch` | Accepted for existing scripts. |

## pointPosition

The world position of a point: a vertex, a joint or transform (its pivot), or a locator. -l for local space.

Arguments: a point

| Flag | Short | Description |
| --- | --- | --- |
| `world` | `w` | World space (the default). |
| `local` | `l` | Object space. |

## poleVectorConstraint

Constrain the object named (or selected) last to the targets before it; a second call adds targets. -mo keeps the current offset; -w sets the targets' weight; -e and -q work on the constraint or the object it drives. Returns [constraint].

Arguments: targets, then the object to drive or the constraint

| Flag | Short | Description |
| --- | --- | --- |
| `maintainOffset` | `mo` | Keep the object where it is now relative to the targets. |
| `weight` | `w` | Weight of the targets named (1). |
| `name` | `n` | Name of the constraint. |
| `offset` | `o` | Added to the result. |
| `remove` | `rm` | Remove the targets named. Removing the last deletes the constraint. |
| `targetList` | `tl` | Query: the targets. |
| `weightAliasList` | `wal` | Query: the targets' weight attributes. |
| `aimVector` | `aim` | The local axis that points at the target. |
| `upVector` | `u` | The local axis that points up. |
| `worldUpVector` | `wu` | The world direction up means. |
| `worldUpType` | `wut` | scene, object, objectrotation, vector or none. |
| `worldUpObject` | `wuo` | The object for worldUpType object and objectrotation. |
| `interpType` | `int` | How rotations blend: noflip, average, shortest, longest, cache, or 0 to 4. |
| `skipTranslate` | `st` | Translate axes to leave alone: x, y, z or none. May be used more than once. |
| `skipRotate` | `sr` | Rotate axes to leave alone: x, y, z or none. May be used more than once. |
| `skip` | `sk` | Axes of this constraint's channel to leave alone: x, y, z or none. May be used more than once. |
| `skipScale` |  | Scale axes to leave alone: x, y, z or none. |
| `edit` | `e` | Change the constraint. |
| `query` | `q` | Ask about the constraint. |

## polyAppend

Add a face to a mesh along border edges and through new points: -ed edge (repeat; -a takes edges too), -p x y z (repeat). Returns the node.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `point` | `p` | A new point: x y z. Repeat it. |
| `edge` | `ed` | A border edge. Repeat it. |
| `append` | `a` | A border edge, or x y z. Repeat it. |
| `hole` | `hl` | See the command reference. |
| `subdivision` | `s` | See the command reference. |
| `texture` | `tx` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyAppendFacetCtx

A polyAppendFacetCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `append` | `ap` |  |
| `maximumNumberOfPoints` | `mnp` |  |
| `planarConstraint` | `pc` |  |
| `rotate` | `ro` |  |
| `subdivision` | `s` |  |
| `texture` | `tx` |  |
| `twist` | `tw` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## polyAppendVertex

Add a face to a mesh from existing vertices and new points: -a vertex (repeat), -p x y z (repeat), or -items [{vertex:}, {position:}] in order. With no mesh named or selected and only points, makes a new mesh. Returns the new face and the mesh.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `mesh` |  | The mesh; the selected one by default. |
| `append` | `a` | An existing vertex. Repeat it. |
| `point` | `p` | A new point: x y z. Repeat it. |
| `items` |  | In order: {vertex: id} or {position: [x, y, z]}. |
| `constructionHistory` | `ch` | Accepted for existing scripts. |
| `hole` | `h` | See the command reference. |
| `texture` | `tx` | See the command reference. |
| `vertex` | `v` | See the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyAutoProjection

Project UVs from the six box planes, each face from the one it faces most, then lay the shells out in 0 to 1.

Arguments: faces or meshes

| Flag | Short | Description |
| --- | --- | --- |
| `percentageSpace` | `ps` | Space between shells, percent of the square. |
| `layoutMethod` | `lm` | Accepted for existing scripts. |
| `layout` | `l` | Accepted for existing scripts. |
| `percentageSpaceLegacy` | `pb` | Accepted for existing scripts. |
| `insertBeforeDeformers` | `ibd` | Accepted for existing scripts. |
| `createNewMap` | `cm` | Accepted for existing scripts. |
| `scaleMode` | `sc` | Accepted for existing scripts. |
| `optimize` | `o` | Accepted for existing scripts. |
| `worldSpace` | `ws` | Accepted for existing scripts. |
| `uvSetName` | `uvs` | Accepted for existing scripts. |
| `constructionHistory` | `ch` | Accepted for existing scripts. |
| `name` | `n` | Accepted for existing scripts. |
| `planes` | `p` | Accepted for existing scripts (six planes). |

## polyAverageColor

Average vertices' colours with their neighbours': -i passes. Returns the node.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |
| `baseColorName` | `bcn` | The colour set (the current one). |
| `iterations` | `i` | How many passes. |

## polyAverageNormal

Average the normals at each vertex and among vertices within -d. Returns the node.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `allowZeroNormal` | `azn` | See the command reference. |
| `distance` | `d` | See the command reference. |
| `postnormalize` | `pon` | See the command reference. |
| `prenormalize` | `prn` | See the command reference. |
| `replaceNormalXYZ` | `xyz` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyAverageVertex

Average Vertices: each vertex moves toward the middle of its neighbours, -i times. Returns the node.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `iterations` | `i` | See the command reference. |
| `worldSpace` | `ws` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyBevel

Bevel the selected edges (vertex or face selections convert to their edges).

| Flag | Short | Description |
| --- | --- | --- |
| `offset` | `o` | Width, cm. |
| `fraction` | `f` | Width as a fraction, with offsetAsFraction. |
| `offsetAsFraction` | `oaf` | Width by fraction rather than distance. |
| `depth` | `dp` | How far the bevel rounds out; held for the node. |
| `chamfer` | `c` | Flat across rather than round. |
| `segments` | `sg` | Segments across the bevel. |
| `profile` |  | 0.5 round, 0.25 straight, toward 1 square. |
| `clamp` |  | Keep widths from overlapping. |
| `smoothingAngle` |  | New edges sharper than this are hard. |

## polyBevel3

Bevel edges: -o offset, -oaf as a fraction of the shortest edge, -sg segments, -c chamfer (flat) or round, -sa smoothing angle, -mv -mvt merge the result. Returns the node.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `profile` |  | Also accepted under this command's other name. |
| `clamp` |  | Also accepted under this command's other name. |
| `angleTolerance` | `at` | See the command reference. |
| `autoFit` | `af` | See the command reference. |
| `chamfer` | `c` | See the command reference. |
| `depth` | `d` | See the command reference. |
| `fillNgons` | `fn` | See the command reference. |
| `fraction` | `f` | See the command reference. |
| `mergeVertexTolerance` | `mvt` | See the command reference. |
| `mergeVertices` | `mv` | See the command reference. |
| `miterAlong` | `mia` | See the command reference. |
| `mitering` | `m` | See the command reference. |
| `miteringAngle` | `ma` | See the command reference. |
| `offset` | `o` | See the command reference. |
| `offsetAsFraction` | `oaf` | See the command reference. |
| `roundness` | `r` | See the command reference. |
| `segments` | `sg` | See the command reference. |
| `smoothingAngle` | `sa` | See the command reference. |
| `uvAssignment` | `ua` | See the command reference. |
| `worldSpace` | `ws` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyBlendColor

Blend two colour sets into -dst: -bcn, -src, -bfn (0 by weights -bwa -bwb, 1 add, 2 subtract, 3 multiply, 4 average, 5 replace). Returns the node.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |
| `baseColorName` | `bcn` | One set. |
| `srcColorName` | `src` | The other set. |
| `dstColorName` | `dst` | Where it goes. |
| `blendFunc` | `bfn` | How they combine. |
| `blendWeightA` | `bwa` | Weight of the first. |
| `blendWeightB` | `bwb` | Weight of the second. |
| `blendWeightC` | `bwc` | Alpha weight of the first. |
| `blendWeightD` | `bwd` | Alpha weight of the second. |

## polyBlindData

Store a number on components: -id, -at face|vertex|edge, -ldn name, -dbd, -ind or -bd; -rst removes it. Returns the node.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |
| `typeId` | `id` | The data's type id. |
| `associationType` | `at` | face, vertex or edge. |
| `longDataName` | `ldn` | The name. |
| `shortDataName` | `sdn` | The short name. |
| `doubleData` | `dbd` | A number. |
| `intData` | `ind` | A whole number. |
| `int64Data` | `lng` | A whole number. |
| `booleanData` | `bd` | On or off. |
| `stringData` | `sd` | Refused: numbers only. |
| `binaryData` | `bnd` | Refused: numbers only. |
| `reset` | `rst` | Remove the data. |
| `rescale` | `rs` | Accepted for existing scripts. |
| `shape` | `sh` | Accepted for existing scripts. |
| `blindIds` |  | The components, worked out from the arguments. |
| `blindValue` |  | The value, worked out from the arguments. |

## polyBoolOp

A boolean of two meshes: -op 1 union, 2 difference, 3 intersection. Returns [transform, node].

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `faceAreaThreshold` | `fat` | See the command reference. |
| `operation` | `op` | See the command reference. |
| `preserveColor` | `pcr` | See the command reference. |
| `useThresholds` | `uth` | See the command reference. |
| `vertexDistanceThreshold` | `vdt` | See the command reference. |
| `mergeUVSets` | `muv` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyBoolean

Union, difference or intersection of meshes (Manifold: robust, watertight output), with the seam cleaned up: slivers welded, the n-gons along the cut split into quads that carry on the edge loops, an optional rounded fillet, and weighted normals. The first object keeps the result; the others are deleted unless keepCutters=True.

Arguments: base cutters...

| Flag | Short | Description |
| --- | --- | --- |
| `operation` |  | union, difference or intersection. |
| `keepCutters` |  | Keep the cutter objects (hidden). |
| `live` |  | Keep it live instead of baking: a boolean modifier per cutter, so moving or editing a cutter updates the cut. The cutters go in a template display layer (BooleanCutters): grey wire you select from the outliner. |
| `cleanup` |  | Weld slivers and stray points along the seam. |
| `snapDistance` |  | Weld distance for cleanup, cm; negative is a tenth of the median seam edge. |
| `splitNgons` |  | Cut the n-gons along the seam into quads (and triangles) that carry on the edge loops. |
| `quadBand` |  | Remesh a band of faces around the seam into quads that follow it, welded back into the faces around it. Falls back to splitting n-gons where the band's border can't be kept. |
| `bandRings` |  | Rings of faces past the seam the quad band takes in. |
| `timeout` |  | Seconds the quad band remesh may take before the n-gon split runs instead; 0 waits forever. |
| `filletWidth` |  | Width of a rounded strip along the seam, cm; 0 keeps it sharp. |
| `filletSegments` |  | Rows of faces across the fillet. |
| `hardAngle` |  | New edges sharper than this many degrees are marked hard. |
| `weightedNormals` |  | Add a weighted normals modifier, if the mesh has none, so the n-gons shade flat. |

## polyBooleanCmd

A boolean of meshes: -op 1 union, 2 difference, 3 intersection. Returns [transform, node].

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `addMesh` | `am` | See the command reference. |
| `changeSmoothMesh` | `csm` | See the command reference. |
| `classification` | `cls` | See the command reference. |
| `interactiveUpdate` | `iu` | See the command reference. |
| `newInputDisplay` | `nid` | See the command reference. |
| `newInputOperation` | `nio` | See the command reference. |
| `operation` | `op` | See the command reference. |
| `removeMesh` | `rm` | See the command reference. |
| `reorderMesh` | `rom` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyBridge

Join selected border loops or chains with faces. Several pairs bridge at once; a single border round a slot or sliver bridges its two long sides; borders of different lengths pair by arc length. Selected faces are deleted and the holes bridged.

| Flag | Short | Description |
| --- | --- | --- |
| `divisions` |  | Rows between the borders; -1 picks them from the gap, the faces around it and the open ends. |
| `curveType` |  | How the rows run between the borders. |
| `smoothness` |  | 0 straightens the curve, 1 is the full curve. |
| `tensionSource` |  | Handle length leaving the source border (Blend and Auto). |
| `tensionTarget` |  | Handle length entering the target border (Blend and Auto). |
| `bridgeOffset` |  | Steps the pairing round closed loops. |
| `directionSource` |  | Read the source border the other way round. |
| `directionTarget` |  | Read the target border the other way round. |
| `twist` |  | Turns the middle rows about the bridge's axis, degrees at the target. |
| `taper` |  | Scale of the middle rows about their centre. |
| `bias` |  | Where the taper peaks, 0 at the source, 1 at the target. |
| `taperCurve` |  | Taper strength along the bridge, values evenly spaced from source to target. |
| `profileFactor` |  | Bulge (positive) or pinch (negative) of the middle rows, as a fraction of the gap. |
| `profileShape` |  | Falloff of the profile across the bridge. |
| `curve` |  | The curve Path follows. |
| `transition` |  | How borders of different lengths make up the difference. |
| `weldEnds` |  | Open bridges land their end rows on the border between the two sides' ends. |
| `weldTolerance` |  | How far a row may be from an end border point and still weld, as a fraction of a row. |
| `relax` |  | Smoothing passes over the new points. |
| `smoothingAngle` |  | New edges sharper than this are hard. |
| `merge` |  | Merge the borders into one instead of bridging. |
| `mergeFactor` |  | Where merged points go: 0 the source, 1 the target. |
| `connect` |  | Pair loops up, or run one bridge through them all in order. |
| `startSource` |  | Source point that pairs with startTarget. |
| `startTarget` |  | Target point that pairs with startSource. |
| `pairs` |  | More point pairs to hold: [[source, target], ...]. |

## polyBridgeEdge

Bridge border edges: -dv divisions, -ctp 0 linear, 1 blend, 2 along -inc, -twt twist, -tp taper, -bo offset, -sv1 -sv2 starting points. Returns the node.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `bridgeOffset` | `bo` | See the command reference. |
| `curveType` | `ctp` | See the command reference. |
| `divisions` | `dv` | See the command reference. |
| `inputCurve` | `inc` | See the command reference. |
| `smoothingAngle` | `sma` | See the command reference. |
| `startVert1` | `sv1` | See the command reference. |
| `startVert2` | `sv2` | See the command reference. |
| `taper` | `tp` | See the command reference. |
| `taperCurve_FloatValue` | `cfv` | See the command reference. |
| `taperCurve_Interp` | `ci` | See the command reference. |
| `taperCurve_Position` | `cp` | See the command reference. |
| `twist` | `twt` | See the command reference. |
| `worldSpace` | `ws` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyCBoolOp

A boolean of meshes: -op 1 union, 2 difference, 3 intersection. Returns [transform, node].

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `classification` | `cls` | See the command reference. |
| `faceAreaThreshold` | `fat` | See the command reference. |
| `operation` | `op` | See the command reference. |
| `preserveColor` | `pcr` | See the command reference. |
| `useCarveBooleans` | `ucb` | See the command reference. |
| `useThresholds` | `uth` | See the command reference. |
| `vertexDistanceThreshold` | `vdt` | See the command reference. |
| `mergeUVSets` | `muv` | See the command reference. |
| `tagIntersection` | `ti` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyCapsule

Create a Capsule mesh. Returns [mesh, generator] (generator is None with generator=False).

| Flag | Short | Description |
| --- | --- | --- |
| `name` | `n` | Name of the mesh; made unique. |
| `position` |  | World position. |
| `rotation` |  | Rotation in degrees. |
| `normal` |  | Turn the up axis to this direction, as for placing on a surface; a dome cut by fraction then sits on position by its open base. |
| `generator` |  | Keep the generator node so its parameters stay editable. |
| `object` | `o` | Accepted: the transform and shape are always made. |
| `createUVs` | `cuv` | 0 leaves the mesh without UVs. |
| `texture` | `tx` | How UVs are laid out; accepted. |
| `axis` | `ax` | The direction the primitive's up points, as a vector. |
| `caching` | `cch` | Accepted. |
| `nodeState` | `nds` | Accepted. |
| `constructionHistory` | `ch` | False: no generator node, as generator=False. |
| `radius` | `r` | cm. |
| `height` | `h` | End to end, caps included, cm. |
| `subdivisionsAxis` | `sa` | Segments around. |
| `subdivisionsHeight` | `sh` | Segments along the straight part. |
| `subdivisionsCaps` | `sc` | Rings on each rounded end. |
| `baseAtOrigin` |  | Sit on the ground. |
| `upAxis` |  | Direction the primitive's up points: x, y, z, -x, -y or -z (or a vector, as the reference's -ax 1 0 0); the scene up axis when left out. |

## polyChamferVtx

Chamfer Vertices: cut each vertex off -w along its edges, leaving one face. Also takes the positional form: history, width, delete face.

Arguments: history, width, delete face, then vertices

| Flag | Short | Description |
| --- | --- | --- |
| `width` | `w` | How far along each edge, cm. |
| `constructionHistory` | `ch` | Off: no history node. |
| `name` | `n` | Name of the node. |

## polyCheck

Check a mesh's insides: returns how many problems, each logged; -of writes them to a file.

Arguments: meshes

| Flag | Short | Description |
| --- | --- | --- |
| `openFile` | `of` | Write the problems here. |
| `edge` | `e` | Accepted: every check runs. |
| `face` | `f` | Accepted: every check runs. |
| `faceOffset` | `fo` | Accepted: every check runs. |

## polyChipOff

Chip faces off into their own shell of the mesh, moved by -ltz -t and the rest. -dup 0 removes them from where they were. Returns the node.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `worldInverse` |  | The object's inverse world matrix when the edit was made. |
| `attraction` | `att` | See the command reference. |
| `duplicate` | `dup` | See the command reference. |
| `gain` | `ga` | See the command reference. |
| `gravity` | `g` | See the command reference. |
| `gravityX` | `gx` | See the command reference. |
| `gravityY` | `gy` | See the command reference. |
| `gravityZ` | `gz` | See the command reference. |
| `keepFacesTogether` | `kft` | See the command reference. |
| `keepFacetTogether` | `xft` | See the command reference. |
| `localCenter` | `lc` | See the command reference. |
| `localDirection` | `ld` | See the command reference. |
| `localDirectionX` | `ldx` | See the command reference. |
| `localDirectionY` | `ldy` | See the command reference. |
| `localDirectionZ` | `ldz` | See the command reference. |
| `localRotate` | `lr` | See the command reference. |
| `localRotateX` | `lrx` | See the command reference. |
| `localRotateY` | `lry` | See the command reference. |
| `localRotateZ` | `lrz` | See the command reference. |
| `localScale` | `ls` | See the command reference. |
| `localScaleX` | `lsx` | See the command reference. |
| `localScaleY` | `lsy` | See the command reference. |
| `localScaleZ` | `lsz` | See the command reference. |
| `localTranslate` | `lt` | See the command reference. |
| `localTranslateX` | `ltx` | See the command reference. |
| `localTranslateY` | `lty` | See the command reference. |
| `localTranslateZ` | `ltz` | See the command reference. |
| `magnX` | `mx` | See the command reference. |
| `magnY` | `my` | See the command reference. |
| `magnZ` | `mz` | See the command reference. |
| `magnet` | `m` | See the command reference. |
| `offset` | `off` | See the command reference. |
| `pivot` | `pvt` | See the command reference. |
| `pivotX` | `pvx` | See the command reference. |
| `pivotY` | `pvy` | See the command reference. |
| `pivotZ` | `pvz` | See the command reference. |
| `random` | `ran` | See the command reference. |
| `scale` | `s` | See the command reference. |
| `scaleX` | `sx` | See the command reference. |
| `scaleY` | `sy` | See the command reference. |
| `scaleZ` | `sz` | See the command reference. |
| `translate` | `t` | See the command reference. |
| `translateX` | `tx` | See the command reference. |
| `translateY` | `ty` | See the command reference. |
| `translateZ` | `tz` | See the command reference. |
| `weight` | `w` | See the command reference. |
| `worldSpace` | `ws` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyCircularize

Fit the selected edge loops, the border of each selected face region, or the selected vertices to a circle in their best-fit plane, turned to move the points least. Points inside a face region follow the border smoothly, so a rectangular face comes out round, ready to extrude into a pipe. Open chains become an arc between their ends.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `spacing` |  | even (equal angles), keep (relative distances along the loop) or project (straight out to the circle). |
| `radius` |  | Radius, cm; 0 keeps the loop's average radius. |
| `rotation` |  | Extra turn about the circle's axis, degrees. |
| `sides` |  | 3 or more: conform to a regular polygon with this many sides, its corners on the circle. |
| `flatten` |  | Put the points in the circle's plane; off keeps each one's height off it. |
| `strength` |  | How far toward the shape, 0 to 1; soft selection fades it further. |
| `alignment` | `al` | See the command reference. |
| `createCurve` | `cc` | See the command reference. |
| `evenlyDistribute` | `ed` | See the command reference. |
| `inputCurve` | `inc` | See the command reference. |
| `normalOrientation` | `nor` | See the command reference. |
| `radialOffset` | `ro` | See the command reference. |
| `smoothingAngle` | `sa` | See the command reference. |
| `worldSpace` | `ws` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyClean

Remove edges and vertices no face uses: -ce, -cv, -cuv, -cpm (each on by default). Returns the node.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |
| `cleanEdges` | `ce` | Loose edges. |
| `cleanVertices` | `cv` | Loose vertices. |
| `cleanUVs` | `cuv` | Accepted: UVs belong to corners here, so none are loose. |
| `cleanPartialUVMapping` | `cpm` | UVs that aren't numbers go to 0. |

## polyCleanup

Find or fix problem geometry: -selectOnly selects it, otherwise it is fixed. Returns the components found.

Arguments: meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `allMeshes` | `am` | Every mesh in the scene. |
| `selectOnly` | `so` | Select what it finds rather than fixing it. |
| `keepHistory` | `kh` | Accepted. |
| `quads` | `q4` | Four-sided faces. |
| `nsided` | `ns` | Faces of more than four sides. |
| `concave` | `cv` | Concave faces. |
| `holed` | `ho` | Faces with holes. |
| `nonPlanar` | `np` | Non-planar faces. |
| `zeroGeometry` | `zg` | Faces of no area. |
| `zeroGeometryTolerance` | `zgt` | How small. |
| `zeroEdge` | `ze` | Edges of no length. |
| `zeroEdgeTolerance` | `zet` | How short. |
| `nonManifold` | `nm` | Non-manifold edges and vertices. |
| `lamina` | `lm` | Faces sharing all their edges. |

## polyCleanupArgList

polyCleanup's positional form: version, then the list: all meshes, select only (1) or fix (0), history, quads, n-sided, concave, holed, non-planar, zero geometry and its tolerance, zero edge and its tolerance, zero map and its tolerance, shared UVs, non-manifold, lamina, invalid components.

Arguments: version, then the argument list

## polyClipboard

Copy attributes of one face and paste them onto others: -cp or -ps, with -uv and -clr; -sh accepted. -cl empties it.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `clear` | `cl` | See the command reference. |
| `color` | `clr` | See the command reference. |
| `copy` | `cp` | See the command reference. |
| `paste` | `ps` | See the command reference. |
| `shader` | `sh` | See the command reference. |
| `uvCoordinates` | `uv` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyCloseBorder

Fill the holes whose borders the edges lie on; a mesh means all its holes. Returns the node.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyCollapse

Collapse each connected group of selected edges to its centre.

## polyCollapseEdge

Collapse each connected group of edges to its centre. Returns the node.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyCollapseFacet

Collapse faces to their centres; with -uat only those under -at in area. Returns the node.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `areaThreshold` | `at` | See the command reference. |
| `useAreaThreshold` | `uat` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyColorBlindData

Show blind data as colours in the set "blindData": -id, -at, -ldn, -m 0 binary or shaded between -mnv and -mxv; -cr -cg -cb and -ncr -ncg -ncb the colours. Returns the node.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |
| `typeId` | `id` | The data's type id. |
| `associationType` | `at` | face, vertex or edge. |
| `longDataName` | `ldn` | The name. |
| `shortDataName` | `sdn` | The short name. |
| `mode` | `m` | 0 binary, others shaded. |
| `colorRed` | `cr` | The colour. |
| `colorGreen` | `cg` | The colour. |
| `colorBlue` | `cb` | The colour. |
| `noColorRed` | `ncr` | Where there's no data. |
| `noColorGreen` | `ncg` | Where there's no data. |
| `noColorBlue` | `ncb` | Where there's no data. |
| `minValue` | `mnv` | Lowest of the shading. |
| `maxValue` | `mxv` | Highest of the shading. |

## polyColorDel

Remove a colour set: -cn (the current one). Returns the node.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |
| `colorSetName` | `cn` | The set. |

## polyColorMod

Change colours: -hv adds to hue, -sv and -v scale saturation and value. Returns the node.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |
| `baseColorName` | `bcn` | The colour set (the current one). |
| `huev` | `hv` | Degrees of hue. |
| `satv` | `sv` | Saturation scale. |
| `value` | `v` | Value scale. |

## polyColorPerVertex

Set vertex colours: -r -g -b -a or -rgb, -rel adds, -rem removes the set's colours. -cdo shows them. -q -r -g -b -a -rgb reads each in turn. Works on vtx and vtxFace. Returns the node.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `corners` |  | vtxFace pairs the edit works on, when it isn't whole vertices. |
| `alpha` | `a` | See the command reference. |
| `clamped` | `cla` | See the command reference. |
| `colorB` | `b` | See the command reference. |
| `colorDisplayOption` | `cdo` | See the command reference. |
| `colorG` | `g` | See the command reference. |
| `colorR` | `r` | See the command reference. |
| `colorRGB` | `rgb` | See the command reference. |
| `notUndoable` | `nun` | See the command reference. |
| `relative` | `rel` | See the command reference. |
| `remove` | `rem` | See the command reference. |
| `representation` | `rpt` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyColorSet

Colour sets: -cr -d -rn -cp with -cs and -nc, -ccs -cs makes one current. -q -acs lists them, -q -ccs names the current one.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `allColorSets` | `acs` | See the command reference. |
| `clamped` | `cla` | See the command reference. |
| `colorSet` | `cs` | See the command reference. |
| `copy` | `cp` | See the command reference. |
| `create` | `cr` | See the command reference. |
| `currentColorSet` | `ccs` | See the command reference. |
| `currentPerInstanceSet` | `cpi` | See the command reference. |
| `delete` | `d` | See the command reference. |
| `newColorSet` | `nc` | See the command reference. |
| `perInstance` | `pi` | See the command reference. |
| `rename` | `rn` | See the command reference. |
| `representation` | `rpt` | See the command reference. |
| `shareInstances` | `si` | See the command reference. |
| `unshared` | `us` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyCombine

Merge meshes into one object (world space kept). Returns its name.

Arguments: objects

| Flag | Short | Description |
| --- | --- | --- |
| `name` |  | Name of the combined mesh. |

## polyCompare

Compare two meshes: 0 when alike, else the bits of what differs: 1 vertices, 2 edges, 4 faces, 8 UV sets, 16 UV indices, 32 colour sets, 64 colour values.

Arguments: two meshes

| Flag | Short | Description |
| --- | --- | --- |
| `vertices` | `v` | Compare vertices. |
| `edges` | `e` | Compare edges. |
| `faceDesc` | `fd` | Compare faces. |
| `uvSets` | `uv` | Compare UVs. |
| `uvSetIndices` | `iuv` | Compare how UVs are shared. |
| `colorSets` | `c` | Compare colour sets. |
| `colorSetIndices` | `ic` | Compare colour values. |
| `userNormals` | `un` | Accepted for existing scripts. |

## polyCone

Create a Cone mesh. Returns [mesh, generator] (generator is None with generator=False).

| Flag | Short | Description |
| --- | --- | --- |
| `name` | `n` | Name of the mesh; made unique. |
| `position` |  | World position. |
| `rotation` |  | Rotation in degrees. |
| `normal` |  | Turn the up axis to this direction, as for placing on a surface; a dome cut by fraction then sits on position by its open base. |
| `generator` |  | Keep the generator node so its parameters stay editable. |
| `object` | `o` | Accepted: the transform and shape are always made. |
| `createUVs` | `cuv` | 0 leaves the mesh without UVs. |
| `texture` | `tx` | How UVs are laid out; accepted. |
| `axis` | `ax` | The direction the primitive's up points, as a vector. |
| `caching` | `cch` | Accepted. |
| `nodeState` | `nds` | Accepted. |
| `constructionHistory` | `ch` | False: no generator node, as generator=False. |
| `radius` | `r` | cm. |
| `height` | `h` | cm. |
| `subdivisionsAxis` | `sa` | Segments around. |
| `subdivisionsHeight` | `sh` | Segments up. |
| `subdivisionsCap` | `sc` | Rings of quads around the base's fill; 0 and 1 both leave just the fill. |
| `roundCap` | `rcp` | Accepted; the base stays flat. |
| `capType` |  | How each flat end is filled inside its rings: quads (a grid joined to the rim by a ring, one triangle when subdivisionsAxis is odd), fan (triangles to the centre), ngon (one polygon) or none (left open). |
| `baseAtOrigin` |  | Sit on the ground. |
| `fraction` |  | Portion of a full turn swept around the up axis: 1 is whole, 0.5 half. |
| `cutCaps` |  | Close the flat ends a partial sweep leaves. |
| `capTip` |  | Cut the point off at tipRadius and fill it as capType, so quads leave no triangles at the tip. |
| `tipRadius` |  | Radius of the capped tip, cm; small keeps it looking pointed. |
| `subdivisionsX` | `sx` | Another name for the attribute it sets. |
| `subdivisionsY` | `sy` | Another name for the attribute it sets. |
| `subdivisionsZ` | `sz` | Another name for the attribute it sets. |
| `upAxis` |  | Direction the primitive's up points: x, y, z, -x, -y or -z (or a vector, as the reference's -ax 1 0 0); the scene up axis when left out. |

## polyConform

Make face winding consistent; closed parts face outward.

## polyConformShape

Conform the selected components to a fitted shape: line, circle, square or rectangle (per chain or loop, corners on the points nearest the fitted rectangle's), sphere, cylinder (fitted axis) or plane (flatten). Face interiors follow their borders for the loop shapes.

| Flag | Short | Description |
| --- | --- | --- |
| `shape` |  | line, circle, square, rectangle, sphere, cylinder or plane. |
| `spacing` |  | Line, circle, square, rectangle: even, keep or project. |
| `radius` |  | Circle, sphere, cylinder: radius, cm; 0 fits it. |
| `rotation` |  | Circle, square, rectangle: extra turn in the plane, degrees. |
| `sides` |  | Circle: 3 or more makes it a regular polygon. |
| `axis` |  | Cylinder: auto (fitted), x, y or z (object axes). |
| `strength` |  | How far toward the shape, 0 to 1; soft selection fades it further. |

## polyConformToObject

Conform the selected components onto another mesh or a curve. loop: each selected edge loop (or face region border) lands one to one on the target's nearest closed loop (a cylinder's rim, a ring), at the turn that moves it least, or spread along it when the counts differ; face interiors follow. surface: every point to the nearest surface point. vertex: every point to the nearest target vertex. A curve target works through its line: loop spreads a loop along a closed curve, surface puts each point on the nearest point of the curve.

| Flag | Short | Description |
| --- | --- | --- |
| `target` |  | Mesh or curve to conform to; default: the selected mesh or curve that has no components selected. |
| `mode` |  | loop, surface or vertex. |
| `spacing` |  | Loop mode with unequal counts: even, keep or project. |
| `strength` |  | How far toward the shape, 0 to 1; soft selection fades it further. |

## polyConnect

Connect selected vertices that share a face with new edges, or the midpoints of selected edges across their faces.

## polyConnectComponents

Connect vertices, edges or faces with new edges through the faces between them: vertices directly, edges at their midpoints, faces along the edges they share. -ief -aef follow the surface. Returns the node.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `vertices` |  | Vertices the edit also connects. |
| `adjustEdgeFlow` | `aef` | See the command reference. |
| `insertWithEdgeFlow` | `ief` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyContourProjection

Contour stretch: fits UVs of a run of quads across 0 to 1 by their arc lengths along the rows, from the faces' own order. -ps accepted. Returns the node.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `createNewMap` | `cm` | See the command reference. |
| `flipRails` | `fr` | See the command reference. |
| `insertBeforeDeformers` | `ibd` | See the command reference. |
| `method` | `m` | See the command reference. |
| `offset0` | `o0` | See the command reference. |
| `offset1` | `o1` | See the command reference. |
| `offset2` | `o2` | See the command reference. |
| `offset3` | `o3` | See the command reference. |
| `reduceShear` | `rs` | See the command reference. |
| `smoothness0` | `s0` | See the command reference. |
| `smoothness1` | `s1` | See the command reference. |
| `smoothness2` | `s2` | See the command reference. |
| `smoothness3` | `s3` | See the command reference. |
| `userDefinedCorners` | `udc` | See the command reference. |
| `uvSetName` | `uvs` | See the command reference. |
| `worldSpace` | `ws` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyCopyUV

Copy one UV set onto the current UVs: -uvi the set to read. Returns the node.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `createNewMap` | `cm` | See the command reference. |
| `uvSetName` | `uvs` | See the command reference. |
| `uvSetNameInput` | `uvi` | See the command reference. |
| `worldSpace` | `ws` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyCrease

Set the crease of the selected edges (-v 0 to 10): it holds them sharper under smoothing. -q -v asks.

Arguments: edges (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | Crease value: 0 to 10. |
| `relativeValue` | `rv` | Accepted for existing scripts. |
| `query` | `q` | Query mode. |
| `operation` | `op` | See the command reference. |
| `vertexValue` | `vv` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyCreaseCtx

A polyCreaseCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `extendSelection` | `es` |  |
| `relative` | `r` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## polyCreateFacet

Create one face through -p points as a new mesh. Returns [transform, node].

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `point` | `p` | A point of the face: x y z. Repeat it. |
| `hole` | `hl` | See the command reference. |
| `subdivision` | `s` | See the command reference. |
| `texture` | `tx` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyCreateFacetCtx

A polyCreateFacetCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `append` | `ap` |  |
| `exists2` | `ex2` |  |
| `maximumNumberOfPoints` | `mnp` |  |
| `planarConstraint` | `pc` |  |
| `subdivision` | `s` |  |
| `texture` | `tx` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## polyCube

Create a Cube mesh. Returns [mesh, generator] (generator is None with generator=False).

| Flag | Short | Description |
| --- | --- | --- |
| `name` | `n` | Name of the mesh; made unique. |
| `position` |  | World position. |
| `rotation` |  | Rotation in degrees. |
| `normal` |  | Turn the up axis to this direction, as for placing on a surface; a dome cut by fraction then sits on position by its open base. |
| `generator` |  | Keep the generator node so its parameters stay editable. |
| `object` | `o` | Accepted: the transform and shape are always made. |
| `createUVs` | `cuv` | 0 leaves the mesh without UVs. |
| `texture` | `tx` | How UVs are laid out; accepted. |
| `axis` | `ax` | The direction the primitive's up points, as a vector. |
| `caching` | `cch` | Accepted. |
| `nodeState` | `nds` | Accepted. |
| `constructionHistory` | `ch` | False: no generator node, as generator=False. |
| `width` | `w` | Size along X, cm. |
| `height` | `h` | Size along the up axis, cm. |
| `depth` | `d` | Size along Z, cm. |
| `subdivisionsWidth` | `sw` | Segments across. |
| `subdivisionsHeight` | `sh` | Segments up. |
| `subdivisionsDepth` | `sd` | Segments deep. |
| `baseAtOrigin` |  | Sit on the ground instead of centring. |
| `subdivisionsX` | `sx` | Another name for the attribute it sets. |
| `subdivisionsY` | `sy` | Another name for the attribute it sets. |
| `subdivisionsZ` | `sz` | Another name for the attribute it sets. |
| `upAxis` |  | Direction the primitive's up points: x, y, z, -x, -y or -z (or a vector, as the reference's -ax 1 0 0); the scene up axis when left out. |

## polyCut

Cut Faces: -pc the plane's centre, -ro its rotation (world), -df deletes the side its normal points to, -ef pulls that side off by -eo. Returns the node.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |
| `cutPlaneCenter` | `pc` | The plane's centre. |
| `cutPlaneRotate` | `ro` | The plane's rotation. |
| `cutPlaneWidth` | `pw` | Accepted: the plane has no edge here. |
| `cutPlaneHeight` | `ph` | Accepted: the plane has no edge here. |
| `deleteFaces` | `df` | Delete the side the normal points to. |
| `extractFaces` | `ef` | Pull that side off. |
| `extractOffset` | `eo` | How far. |
| `onObject` | `oo` | Accepted for existing scripts. |
| `worldSpace` | `ws` | The plane is in world space (always). |
| `worldInverse` |  | The object's inverse world matrix when the edit was made. |

## polyCutCtx

A polyCutCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `deleteFaces` | `df` |  |
| `extractFaces` | `ef` |  |
| `extractOffset` | `eo` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## polyCylinder

Create a Cylinder mesh. Returns [mesh, generator] (generator is None with generator=False).

| Flag | Short | Description |
| --- | --- | --- |
| `name` | `n` | Name of the mesh; made unique. |
| `position` |  | World position. |
| `rotation` |  | Rotation in degrees. |
| `normal` |  | Turn the up axis to this direction, as for placing on a surface; a dome cut by fraction then sits on position by its open base. |
| `generator` |  | Keep the generator node so its parameters stay editable. |
| `object` | `o` | Accepted: the transform and shape are always made. |
| `createUVs` | `cuv` | 0 leaves the mesh without UVs. |
| `texture` | `tx` | How UVs are laid out; accepted. |
| `axis` | `ax` | The direction the primitive's up points, as a vector. |
| `caching` | `cch` | Accepted. |
| `nodeState` | `nds` | Accepted. |
| `constructionHistory` | `ch` | False: no generator node, as generator=False. |
| `radius` | `r` | cm. |
| `height` | `h` | cm. |
| `subdivisionsAxis` | `sa` | Segments around. |
| `subdivisionsHeight` | `sh` | Segments up. |
| `subdivisionsCaps` | `sc` | Rings of quads around each cap's fill; 0 and 1 both leave just the fill. |
| `roundCap` | `rcp` | Accepted; the caps stay flat. |
| `capType` |  | How each flat end is filled inside its rings: quads (a grid joined to the rim by a ring, one triangle when subdivisionsAxis is odd), fan (triangles to the centre), ngon (one polygon) or none (left open). |
| `caps` |  | Close the ends; off leaves them open whatever capType says. |
| `baseAtOrigin` |  | Sit on the ground. |
| `fraction` |  | Portion of a full turn swept around the up axis: 1 is whole, 0.5 half. |
| `cutCaps` |  | Close the flat ends a partial sweep leaves. |
| `subdivisionsX` | `sx` | Another name for the attribute it sets. |
| `subdivisionsY` | `sy` | Another name for the attribute it sets. |
| `subdivisionsZ` | `sz` | Another name for the attribute it sets. |
| `upAxis` |  | Direction the primitive's up points: x, y, z, -x, -y or -z (or a vector, as the reference's -ax 1 0 0); the scene up axis when left out. |

## polyDelEdge

Delete edges, merging the faces beside them. -cv also removes the vertices that leaves redundant. Returns the node.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `withFaces` |  | Delete the faces beside them instead. |
| `cleanVertices` | `cv` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyDelFacet

Delete faces. Returns the node.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyDelVertex

Delete vertices, merging the faces round them. Returns the node.

Arguments: vertices (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `withFaces` |  | Delete the faces round them instead. |
| `constructionHistory` | `ch` | Off: no history node. |
| `name` | `n` | Name of the node. |

## polyDetachEdges

Detach Components: vertices split at each face, edges part the faces beside them. Returns the node.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyDisc

Create a Disc mesh. Returns [mesh, generator] (generator is None with generator=False).

| Flag | Short | Description |
| --- | --- | --- |
| `name` | `n` | Name of the mesh; made unique. |
| `position` |  | World position. |
| `rotation` |  | Rotation in degrees. |
| `normal` |  | Turn the up axis to this direction, as for placing on a surface; a dome cut by fraction then sits on position by its open base. |
| `generator` |  | Keep the generator node so its parameters stay editable. |
| `object` | `o` | Accepted: the transform and shape are always made. |
| `createUVs` | `cuv` | 0 leaves the mesh without UVs. |
| `texture` | `tx` | How UVs are laid out; accepted. |
| `axis` | `ax` | The direction the primitive's up points, as a vector. |
| `caching` | `cch` | Accepted. |
| `nodeState` | `nds` | Accepted. |
| `constructionHistory` | `ch` | False: no generator node, as generator=False. |
| `radius` | `r` | cm. |
| `subdivisionsAxis` | `sa` | Segments around. |
| `subdivisionsRings` | `sr` | Rings; 0 is one n-gon. |
| `fraction` |  | Portion of a full turn swept around the up axis: 1 is whole, 0.5 half. |
| `capType` |  | How the innermost ring is filled: quads (a grid joined to it by a ring, one triangle when subdivisionsAxis is odd), fan (triangles to the centre) or ngon (one polygon). subdivisionsRings 0 is always one n-gon. |
| `sides` | `s` | Another name for the attribute it sets. |
| `subdivisions` | `sd` | Another name for the attribute it sets. |
| `subdivisionMode` | `sm` | Another name for the attribute it sets. |
| `upAxis` |  | Direction the primitive's up points: x, y, z, -x, -y or -z (or a vector, as the reference's -ax 1 0 0); the scene up axis when left out. |

## polyDissolve

Remove selected edges or vertices by merging the faces around them, cleanly.

## polyDuplicateAndConnect

Duplicate a mesh with its inMesh fed by the original's outMesh, so edits to the first show on the second. Returns the new objects.

Arguments: meshes

| Flag | Short | Description |
| --- | --- | --- |
| `removeOriginalFromShaders` | `ros` | Accepted for existing scripts. |
| `renameChildren` | `rc` | Accepted for existing scripts. |

## polyDuplicateEdge

Offset Edge Loop's node: insert a loop either side of edges, -of along each crossing edge. -de also removes the edges. Returns the node.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `adjustEdgeFlow` | `aef` | See the command reference. |
| `deleteEdge` | `de` | See the command reference. |
| `endVertexOffset` | `evo` | See the command reference. |
| `insertWithEdgeFlow` | `ief` | See the command reference. |
| `offset` | `of` | See the command reference. |
| `smoothingAngle` | `sma` | See the command reference. |
| `splitType` | `stp` | See the command reference. |
| `startVertexOffset` | `svo` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyDuplicateFaces

Copy the selected faces into a separate shell of the same mesh and select the copies.

## polyEditEdgeFlow

Move the selected edge loops onto the curvature of the surface around them (the reference's Edit Edge Flow). Each vertex lands on a curve through the nearest unselected vertices across the loop, shaped by the vertices beyond them, and neighbouring selected loops spread evenly between those, as subdividing would place them. Selected vertices off any chain, and face selections, fit both directions across each vertex.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `adjustEdgeFlow` | `aef` | 0 flat between the neighbours, 1 on the curve, above 1 exaggerated, negative inverted. |
| `edgeFlow` | `ef` | Shape the curve from the vertices beyond the neighbours; off, from the surface normals at the neighbours. |
| `tension` |  | Toward 1 flattens the curve, negative makes it fuller. |
| `loopSmooth` |  | Also smooth each loop along its length, 0 to 1, keeping its spacing. |
| `iterations` |  | Passes of Smooth Along Loop. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyEditUV

Move (-u -v), scale (-su -sv) or turn (-a degrees) UVs about the pivot (-pu -pv). Relative by default; -r 0 sets -u/-v. -q returns u, v of each UV.

Arguments: UVs (pCube1.map[0:3]), faces or meshes

| Flag | Short | Description |
| --- | --- | --- |
| `u` | `u` | U offset (or value with -r 0). |
| `v` | `v` | V offset (or value with -r 0). |
| `scaleU` | `su` | Scale in U about the pivot. |
| `scaleV` | `sv` | Scale in V about the pivot. |
| `angle` | `a` | Turn, degrees, about the pivot. |
| `pivotU` | `pu` | Pivot U. |
| `pivotV` | `pv` | Pivot V. |
| `relative` | `r` | Offsets (default) or values. |
| `query` | `q` | Return u, v of each UV. |

## polyEditUVShell

Move whole shells of the UVs named: -u -v, -su -sv, -a degrees, about -pu -pv or each shell's middle. -r 0 puts a shell's middle at -u -v. Returns the node.

Arguments: UVs, faces or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `constructionHistory` | `ch` | Accepted: a node is made while history is on. |
| `name` | `n` | Accepted for existing scripts. |
| `caching` | `cch` | Accepted for existing scripts. |
| `nodeState` | `nds` | Accepted for existing scripts. |
| `uValue` | `u` | U offset, or value with -r 0. |
| `vValue` | `v` | V offset, or value with -r 0. |
| `scaleU` | `su` | U scale. |
| `scaleV` | `sv` | V scale. |
| `rotateRatio` | `rr` | Accepted: the same as -a. |
| `angle` | `a` | Turn, degrees. |
| `pivotU` | `pu` | Pivot U. |
| `pivotV` | `pv` | Pivot V. |
| `relative` | `r` | Offsets (the default) or values. |
| `rotation` | `rot` | Accepted for existing scripts. |
| `query` | `q` | Return u, v of each UV. |

## polyEvaluate

Counts on meshes or components: -v vertices, -e edges, -f faces, -t triangles, -uv UVs, -s shells, -b bounding box. With no flag, a dictionary of the counts.

Arguments: meshes or components (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `vertex` | `v` | Vertex count. |
| `edge` | `e` | Edge count. |
| `face` | `f` | Face count. |
| `triangle` | `t` | Triangle count. |
| `uvcoord` | `uv` | UV count. |
| `shell` | `s` | Shell count (UV shells). |
| `boundingBox` | `b` | World bounding box ((xmin, xmax), (ymin, ymax), (zmin, zmax)). |
| `area` | `a` | Area, object space. |
| `worldArea` | `wa` | Area, world space. |
| `boundingBoxComponent` | `bc` | World bounds of the selected components. |
| `format` | `fmt` | The counts as text. |
| `accurateEvaluation` | `ae` | Accepted: counts are always exact here. |
| `ac` | `ac` | Accepted for existing scripts. |
| `triangleComponent` | `tc` | Triangle count of the selected components. |
| `usu` | `usu` | Accepted for existing scripts. |
| `vertexComponent` | `vc` | Selected vertex count. |
| `edgeComponent` | `ec` | Selected edge count. |
| `faceComponent` | `fc` | Selected face count. |
| `uvComponent` | `uvc` | Selected UV count. |
| `uvShell` | `us` | UV shell count. |
| `uvShellIds` | `uid` | The UV shell of each selected UV. |
| `uvsInShell` | `uis` | The UVs in this UV shell. |
| `activeUVShells` | `aus` | The UV shells the selection touches. |
| `activeShells` | `as` | The mesh shells the selection touches. |
| `uvSetName` | `uvs` | The UV set: the current one here. |
| `displayStats` | `ds` | Accepted for existing scripts. |
| `boundingBox2d` | `b2` | UV bounds: [[umin, umax], [vmin, vmax]]. |
| `boundingBoxComponent2d` | `bc2` | UV bounds of the components named. |

## polyExtract

Move the selected faces out into new mesh objects (one per source mesh). Returns their names.

## polyExtrude

Extrude the selected faces along their normals, or in edge mode grow a strip of faces off the selected edges. Connected faces extrude as regions unless individual=True.

| Flag | Short | Description |
| --- | --- | --- |
| `thickness` |  | Distance along the normals, cm. |
| `keepFacesTogether` | `kft` | Faces that touch extrude as one. |
| `divisions` | `d` | Segments along the extrusion. |
| `smoothingAngle` | `sma` | New edges sharper than this are hard. |
| `taper` | `tp` | Scale at the far end. |
| `twist` | `twt` | Turn at the far end, degrees. |
| `offset` |  | Faces: inward distance of the extruded border, cm; negative grows them. Edges: outward distance from their faces, cm. |
| `individual` |  | Each face separately. |
| `keepBack` |  | Faces: where the extruded faces only border open edges, as a whole open shell, keep a copy facing back at the base. |
| `duplicate` |  | Faces: leave the mesh as it is and put the extruded faces in a new mesh. |
| `keepSides` |  | With duplicate, the new mesh also keeps the side faces. |

## polyExtrudeEdge

Extrude edges into strips of faces: -ltz out from their faces, -off outward, -t -r -s about the pivot. Returns the node.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `worldInverse` |  | The object's inverse world matrix when the edit was made. |
| `createCurve` | `cc` | See the command reference. |
| `divisions` | `d` | See the command reference. |
| `gain` | `ga` | See the command reference. |
| `inputCurve` | `inc` | See the command reference. |
| `keepFacesTogether` | `kft` | See the command reference. |
| `localCenter` | `lc` | See the command reference. |
| `localDirection` | `ld` | See the command reference. |
| `localDirectionX` | `ldx` | See the command reference. |
| `localDirectionY` | `ldy` | See the command reference. |
| `localDirectionZ` | `ldz` | See the command reference. |
| `localRotate` | `lr` | See the command reference. |
| `localRotateX` | `lrx` | See the command reference. |
| `localRotateY` | `lry` | See the command reference. |
| `localRotateZ` | `lrz` | See the command reference. |
| `localScale` | `ls` | See the command reference. |
| `localScaleX` | `lsx` | See the command reference. |
| `localScaleY` | `lsy` | See the command reference. |
| `localScaleZ` | `lsz` | See the command reference. |
| `localTranslate` | `lt` | See the command reference. |
| `localTranslateX` | `ltx` | See the command reference. |
| `localTranslateY` | `lty` | See the command reference. |
| `localTranslateZ` | `ltz` | See the command reference. |
| `offset` | `off` | See the command reference. |
| `pivot` | `pvt` | See the command reference. |
| `pivotX` | `pvx` | See the command reference. |
| `pivotY` | `pvy` | See the command reference. |
| `pivotZ` | `pvz` | See the command reference. |
| `random` | `ran` | See the command reference. |
| `rotate` | `ro` | See the command reference. |
| `rotateX` | `rx` | See the command reference. |
| `rotateY` | `ry` | See the command reference. |
| `rotateZ` | `rz` | See the command reference. |
| `scale` | `s` | See the command reference. |
| `scaleX` | `sx` | See the command reference. |
| `scaleY` | `sy` | See the command reference. |
| `scaleZ` | `sz` | See the command reference. |
| `smoothingAngle` | `sma` | See the command reference. |
| `taper` | `tp` | See the command reference. |
| `taperCurve_FloatValue` | `cfv` | See the command reference. |
| `taperCurve_Interp` | `ci` | See the command reference. |
| `taperCurve_Position` | `cp` | See the command reference. |
| `thickness` | `tk` | See the command reference. |
| `translate` | `t` | See the command reference. |
| `translateX` | `tx` | See the command reference. |
| `translateY` | `ty` | See the command reference. |
| `translateZ` | `tz` | See the command reference. |
| `twist` | `twt` | See the command reference. |
| `worldSpace` | `ws` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyExtrudeFacet

Extrude faces: -ltz along their normals, -thickness too, -off inward, -ls -lr about each region, -t -r -s about the pivot, -d divisions, -inc along a curve with -tp taper and -twt twist. -kft 0 extrudes each face alone. Returns the node.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `individual` |  | Also accepted under this command's other name. |
| `keepBack` |  | Also accepted under this command's other name. |
| `worldInverse` |  | The object's inverse world matrix when the edit was made. |
| `curvePoints` |  | Samples of inputCurve when the edit was made, object space. |
| `attraction` | `att` | See the command reference. |
| `createCurve` | `cc` | See the command reference. |
| `divisions` | `d` | See the command reference. |
| `gain` | `ga` | See the command reference. |
| `gravity` | `g` | See the command reference. |
| `gravityX` | `gx` | See the command reference. |
| `gravityY` | `gy` | See the command reference. |
| `gravityZ` | `gz` | See the command reference. |
| `inputCurve` | `inc` | See the command reference. |
| `keepFacesTogether` | `kft` | See the command reference. |
| `keepFacetTogether` | `xft` | See the command reference. |
| `localCenter` | `lc` | See the command reference. |
| `localDirection` | `ld` | See the command reference. |
| `localDirectionX` | `ldx` | See the command reference. |
| `localDirectionY` | `ldy` | See the command reference. |
| `localDirectionZ` | `ldz` | See the command reference. |
| `localRotate` | `lr` | See the command reference. |
| `localRotateX` | `lrx` | See the command reference. |
| `localRotateY` | `lry` | See the command reference. |
| `localRotateZ` | `lrz` | See the command reference. |
| `localScale` | `ls` | See the command reference. |
| `localScaleX` | `lsx` | See the command reference. |
| `localScaleY` | `lsy` | See the command reference. |
| `localScaleZ` | `lsz` | See the command reference. |
| `localTranslate` | `lt` | See the command reference. |
| `localTranslateX` | `ltx` | See the command reference. |
| `localTranslateY` | `lty` | See the command reference. |
| `localTranslateZ` | `ltz` | See the command reference. |
| `magnX` | `mx` | See the command reference. |
| `magnY` | `my` | See the command reference. |
| `magnZ` | `mz` | See the command reference. |
| `magnet` | `m` | See the command reference. |
| `offset` | `off` | See the command reference. |
| `pivot` | `pvt` | See the command reference. |
| `pivotX` | `pvx` | See the command reference. |
| `pivotY` | `pvy` | See the command reference. |
| `pivotZ` | `pvz` | See the command reference. |
| `random` | `ran` | See the command reference. |
| `reverseAllFaces` | `raf` | See the command reference. |
| `rotate` | `ro` | See the command reference. |
| `rotateX` | `rx` | See the command reference. |
| `rotateY` | `ry` | See the command reference. |
| `rotateZ` | `rz` | See the command reference. |
| `scale` | `s` | See the command reference. |
| `scaleX` | `sx` | See the command reference. |
| `scaleY` | `sy` | See the command reference. |
| `scaleZ` | `sz` | See the command reference. |
| `smoothingAngle` | `sma` | See the command reference. |
| `taper` | `tp` | See the command reference. |
| `taperCurve_FloatValue` | `cfv` | See the command reference. |
| `taperCurve_Interp` | `ci` | See the command reference. |
| `taperCurve_Position` | `cp` | See the command reference. |
| `thickness` | `tk` | See the command reference. |
| `translate` | `t` | See the command reference. |
| `translateX` | `tx` | See the command reference. |
| `translateY` | `ty` | See the command reference. |
| `translateZ` | `tz` | See the command reference. |
| `twist` | `twt` | See the command reference. |
| `weight` | `w` | See the command reference. |
| `worldSpace` | `ws` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyExtrudeVertex

Extrude vertices: each becomes a pyramid's tip, -w wide at the base, -l long along its normal, -d rings. Returns the node.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `divisions` | `d` | See the command reference. |
| `length` | `l` | See the command reference. |
| `width` | `w` | See the command reference. |
| `worldSpace` | `ws` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyFillHole

Cap the holes bordered by the selected edges (every hole when nothing is selected).

## polyFlip

Reverse the winding of the selected faces (the whole mesh in object mode).

## polyFlipEdge

Flip edges between triangles to join their other corners. Returns the node.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyFlipUV

Mirror UVs across the middle of their bounds: -ft 0 horizontally (in U), 1 vertically.

Arguments: UVs, faces or meshes

| Flag | Short | Description |
| --- | --- | --- |
| `flipType` | `ft` | 0 horizontal, 1 vertical. |
| `local` | `l` | Each shell about its own middle. |
| `createNewMap` | `cm` | See the command reference. |
| `cutUV` | `cut` | See the command reference. |
| `insertBeforeDeformers` | `ibd` | See the command reference. |
| `pivotU` | `pu` | See the command reference. |
| `pivotV` | `pv` | See the command reference. |
| `usePivot` | `up` | See the command reference. |
| `uvSetName` | `uvs` | See the command reference. |
| `worldSpace` | `ws` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyForceUV

Change UVs in place: -fh -fv flip about their middle, -uni gives each face all of 0 to 1, -nor "transform" or "unitize" fits them. Returns the node.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `cameraProjection` | `cp` | See the command reference. |
| `createNewMap` | `cm` | See the command reference. |
| `flipHorizontal` | `fh` | See the command reference. |
| `flipVertical` | `fv` | See the command reference. |
| `g` | `g` | See the command reference. |
| `local` | `l` | See the command reference. |
| `normalize` | `nor` | See the command reference. |
| `numItems` | `ni` | See the command reference. |
| `preserveAspectRatio` | `par` | See the command reference. |
| `unitize` | `uni` | See the command reference. |
| `unshare` | `u` | See the command reference. |
| `uvSetName` | `uvs` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyGeoSampler

Fill a colour set from the mesh's own colours, scaled by -sf and clamped; there are no lights to sample here. Returns the node.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |
| `baseColorName` | `bcn` | The colour set (the current one). |
| `scaleFactor` | `sf` | Scale. |
| `clampMin` | `cmn` | Lowest value. |
| `clampMax` | `cmx` | Highest value. |
| `computeShadows` | `cs` | Accepted for existing scripts. |
| `sampleByFace` | `sbf` | Accepted for existing scripts. |

## polyGridFill

Rebuild triangle fans and other patches as a quad grid inside a ring of quads: select the fans' centre vertices or the faces. With edges selected, fills the holes they border the same way.

| Flag | Short | Description |
| --- | --- | --- |
| `gridSize` |  | The grid's size against the hole; 1 grids right to the border with no ring. |
| `rotate` |  | Steps the grid's corners round the border. |

## polyHelix

Create a Helix mesh. Returns [mesh, generator] (generator is None with generator=False).

| Flag | Short | Description |
| --- | --- | --- |
| `name` | `n` | Name of the mesh; made unique. |
| `position` |  | World position. |
| `rotation` |  | Rotation in degrees. |
| `normal` |  | Turn the up axis to this direction, as for placing on a surface; a dome cut by fraction then sits on position by its open base. |
| `generator` |  | Keep the generator node so its parameters stay editable. |
| `object` | `o` | Accepted: the transform and shape are always made. |
| `createUVs` | `cuv` | 0 leaves the mesh without UVs. |
| `texture` | `tx` | How UVs are laid out; accepted. |
| `axis` | `ax` | The direction the primitive's up points, as a vector. |
| `caching` | `cch` | Accepted. |
| `nodeState` | `nds` | Accepted. |
| `constructionHistory` | `ch` | False: no generator node, as generator=False. |
| `coils` | `c` | Turns. |
| `height` | `h` | cm. |
| `width` | `w` | Across the coil, cm. |
| `radius` | `r` | Tube radius, cm. |
| `direction` | `d` | 0 clockwise, 1 counterclockwise. |
| `subdivisionsAxis` | `sa` | Segments round the tube. |
| `subdivisionsCoil` | `sco` | Segments per coil. |
| `subdivisionsCaps` | `sc` | Rings on each end. |
| `roundCap` | `rcp` | Accepted; the ends stay flat. |
| `upAxis` |  | Direction the primitive's up points: x, y, z, -x, -y or -z (or a vector, as the reference's -ax 1 0 0); the scene up axis when left out. |

## polyHole

Assign Invisible Faces: -ah 1 marks faces invisible, 0 clears them. -q -ah tells whether all are. Returns the node.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `assignHole` | `ah` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyIcoSphere

Create a IcoSphere mesh. Returns [mesh, generator] (generator is None with generator=False).

| Flag | Short | Description |
| --- | --- | --- |
| `name` | `n` | Name of the mesh; made unique. |
| `position` |  | World position. |
| `rotation` |  | Rotation in degrees. |
| `normal` |  | Turn the up axis to this direction, as for placing on a surface; a dome cut by fraction then sits on position by its open base. |
| `generator` |  | Keep the generator node so its parameters stay editable. |
| `object` | `o` | Accepted: the transform and shape are always made. |
| `createUVs` | `cuv` | 0 leaves the mesh without UVs. |
| `texture` | `tx` | How UVs are laid out; accepted. |
| `axis` | `ax` | The direction the primitive's up points, as a vector. |
| `caching` | `cch` | Accepted. |
| `nodeState` | `nds` | Accepted. |
| `constructionHistory` | `ch` | False: no generator node, as generator=False. |
| `radius` | `r` | cm. |
| `subdivisions` |  | 1 is an icosahedron; each level splits every triangle in four. |
| `baseAtOrigin` |  | Sit on the ground. |
| `fraction` |  | Portion of the height kept, from the top: 1 is whole, 0.5 a dome open at the bottom. |
| `upAxis` |  | Direction the primitive's up points: x, y, z, -x, -y or -z (or a vector, as the reference's -ax 1 0 0); the scene up axis when left out. |

## polyInfo

Topology as text. Each of -fv -fe -ev -ef -ve -vf -fn gives one line per component: FACE, EDGE or VERTEX, its id, a colon, its vertices, edges or faces. -nme -nmv -lf -ie -iv give the components at fault.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `edgeToFace` | `ef` | See the command reference. |
| `edgeToVertex` | `ev` | See the command reference. |
| `faceNormals` | `fn` | See the command reference. |
| `faceToEdge` | `fe` | See the command reference. |
| `faceToVertex` | `fv` | See the command reference. |
| `invalidEdges` | `ie` | See the command reference. |
| `invalidVertices` | `iv` | See the command reference. |
| `laminaFaces` | `lf` | See the command reference. |
| `nonManifoldEdges` | `nme` | See the command reference. |
| `nonManifoldUVEdges` | `nue` | See the command reference. |
| `nonManifoldUVs` | `nuv` | See the command reference. |
| `nonManifoldVertices` | `nmv` | See the command reference. |
| `vertexToEdge` | `ve` | See the command reference. |
| `vertexToFace` | `vf` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyInsertLoop

Insert edge loops across the ring of the first selected edge. Several loops are centred on fraction, spacing apart.

| Flag | Short | Description |
| --- | --- | --- |
| `fraction` |  | Where along the ring, 0 to 1; with several loops, where their middle goes. |
| `count` |  | Number of loops. |
| `spacing` |  | Several loops: gap between neighbours, 1 even spacing across the edge, smaller packs them together. |

## polyInset

Inset the selected faces.

| Flag | Short | Description |
| --- | --- | --- |
| `thickness` |  | Inward distance, cm. |
| `depth` |  | Push along the normals afterwards, cm. |
| `individual` |  | Each face separately. |
| `clamp` |  | Shrink the thickness where the inset would cross itself, as on narrow faces. |

## polyInstallAction

Prepare for a poly action: -ic the component mode and for polyCloseBorder a border constraint, -id show borders, -cs convert the selection, -uc and -ud take them off. An action alone does -ic -id; no arguments does -uc -ud. Returns the selection on -ic. Less than the reference: only the actions it knows have a mode, and only polyCloseBorder a constraint and display.

Arguments: the action

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `commandName` | `cn` | The action installed. |
| `convertSelection` | `cs` | Convert selected objects to the action's components. |
| `installConstraint` | `ic` | Install the mode and constraint. |
| `installDisplay` | `id` | Install the display. |
| `keepInstances` | `ki` | Accepted for existing scripts; has no effect. |
| `uninstallConstraint` | `uc` | Take the constraint off. |
| `uninstallDisplay` | `ud` | Take the display off. |

## polyKnife

Knife / multi-cut: cut a mesh along a path of points, splitting the faces between them and the edges the path crosses. Between two points the cut follows the surface under the straight screen line joining them (seen from eye= or along direction=; without either, along the points' average normal). With loopEdge= it inserts an edge loop across that edge's quad ring instead, and with deleteEdge= it dissolves that edge. Follows the mesh's symmetry. A single point on an edge inserts a vertex there. Returns the new edges.

| Flag | Short | Description |
| --- | --- | --- |
| `mesh` |  | Mesh to cut; default: the selected or hilited mesh. |
| `points` |  | Path points: {"vertex": id}, {"edge": id, "t": 0..1 from its first point}, or {"face": id, "position": [x, y, z] world}. |
| `closed` |  | Also cut from the last point back to the first. |
| `through` |  | Cut every face under the line, hidden faces too. |
| `eye` |  | World eye of a perspective view. |
| `direction` |  | World view direction of an orthographic view. |
| `loopEdge` |  | Insert an edge loop across this edge's ring instead of cutting a path. |
| `fraction` |  | Where along loopEdge the loop crosses it, from its first point. |
| `deleteEdge` |  | Dissolve this edge, joining its two faces, instead of cutting. |
| `select` |  | Select the new edges. |

## polyLayoutUV

Pack UV shells into 0 to 1: -ps percent space between them, -rbf 0 keeps them unturned. Returns the node.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `flipReversed` | `fr` | See the command reference. |
| `layout` | `l` | See the command reference. |
| `layoutMethod` | `lm` | See the command reference. |
| `percentageSpace` | `ps` | See the command reference. |
| `rotateForBestFit` | `rbf` | See the command reference. |
| `scale` | `sc` | See the command reference. |
| `separate` | `se` | See the command reference. |
| `uvSetName` | `uvs` | See the command reference. |
| `worldSpace` | `ws` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyListComponentConversion

Convert components without selecting them: -fv -fe -ff -fuv -fvf from, -tv -te -tf -tuv -tvf to. By default whatever the components touch; -in only what they hold entirely, -bo only the border of the result. Returns component strings.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `internal` | `in` | Only what the components hold entirely. |
| `border` | `bo` | See the command reference. |
| `fromEdge` | `fe` | See the command reference. |
| `fromFace` | `ff` | See the command reference. |
| `fromUV` | `fuv` | See the command reference. |
| `fromVertex` | `fv` | See the command reference. |
| `fromVertexFace` | `fvf` | See the command reference. |
| `toEdge` | `te` | See the command reference. |
| `toFace` | `tf` | See the command reference. |
| `toUV` | `tuv` | See the command reference. |
| `toVertex` | `tv` | See the command reference. |
| `toVertexFace` | `tvf` | See the command reference. |
| `uvShell` | `uvs` | See the command reference. |
| `vertexFaceAllEdges` | `vfa` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyLockNormals

Store the current shading normals of the selected meshes as split normals, so later edits keep them.

## polyMapCut

Cut UVs along the selected edges: each side gets its own UVs, splitting shells.

Arguments: edges

## polyMapDel

Delete UVs of faces. Corners always hold a UV here: they go to 0, joined. Returns the node.

Arguments: UVs, faces or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `constructionHistory` | `ch` | Accepted: a node is made while history is on. |
| `name` | `n` | Accepted for existing scripts. |
| `caching` | `cch` | Accepted for existing scripts. |
| `nodeState` | `nds` | Accepted for existing scripts. |

## polyMapSew

Sew UVs along the selected edges: the two sides meet halfway.

Arguments: edges

## polyMapSewMove

Move and Sew: the smaller shell moves onto the other along the selected edges, then they're sewn.

Arguments: edges

| Flag | Short | Description |
| --- | --- | --- |
| `nf` | `nf` | Accepted for existing scripts. |
| `lps` | `lps` | Accepted for existing scripts. |

## polyMerge

Weld selected vertices closer than a distance (all of a mesh's points if it has no component selection). Exactly two selected vertices weld whatever the distance.

| Flag | Short | Description |
| --- | --- | --- |
| `distance` |  | Weld distance, cm. |
| `average` |  | Move merged points to their average. |

## polyMergeCenter

Weld the selected vertices into one at their centre.

## polyMergeEdge

Join two border edges: -fe -se name them, or give two. -mm 0 at the first, 1 halfway, 2 at the second. Returns the node.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `firstEdge` | `fe` | See the command reference. |
| `mergeMode` | `mm` | See the command reference. |
| `mergeTexture` | `mt` | See the command reference. |
| `secondEdge` | `se` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyMergeEdgeCtx

A polyMergeEdgeCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `activeNodes` | `anq` |  |
| `firstEdge` | `fe` |  |
| `secondEdge` | `se` |  |
| `immediate` | `im` |  |
| `mergeMode` | `mm` |  |
| `mergeTexture` | `mt` |  |
| `nodeType` | `nt` |  |
| `previous` | `pvs` |  |
| `reset` | `rs` |  |
| `toolNode` | `tnq` |  |
| `caching` | `cch` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## polyMergeFacet

Join two faces of different shells with the same number of corners: -ff -sf name them. -mm 0 moves the second to the first, 1 both to the average, 2 the first to the second, 6 nothing. Returns the node.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `firstFacet` | `ff` | See the command reference. |
| `mergeMode` | `mm` | See the command reference. |
| `secondFacet` | `sf` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyMergeFacetCtx

A polyMergeFacetCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `activeNodes` | `anq` |  |
| `firstFacet` | `ff` |  |
| `secondFacet` | `sf` |  |
| `immediate` | `im` |  |
| `mergeMode` | `mm` |  |
| `nodeType` | `nt` |  |
| `previous` | `pvs` |  |
| `reset` | `rs` |  |
| `toolNode` | `tnq` |  |
| `caching` | `cch` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## polyMergeUV

Merge UVs closer than -d into one, where they share a vertex. Returns the node.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `distance` | `d` | See the command reference. |
| `uvSetName` | `uvs` | See the command reference. |
| `worldSpace` | `ws` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyMergeVertex

Weld vertices closer than -d; -am welds exactly two whatever the distance. A mesh means all its vertices. Returns the node.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `average` |  | Also accepted under this command's other name. |
| `alwaysMergeTwoVertices` | `am` | See the command reference. |
| `distance` | `d` | See the command reference. |
| `mergeToComponents` | `mtc` | See the command reference. |
| `texture` | `tx` | See the command reference. |
| `worldSpace` | `ws` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyMirror

Mirror whole meshes across the plane through the object's or the world's origin facing an axis, welding along the seam. direction positive keeps the + side and rebuilds the - side from it, cutting faces that cross the plane; negative does the reverse; both adds a mirrored copy of the whole mesh. flip=True reflects the mesh across the plane instead, keeping nothing of the original.

Arguments: objects

| Flag | Short | Description |
| --- | --- | --- |
| `axis` | `a` | x, y or z; or 0, 1 or 2 with the reference's other flags. |
| `direction` | `d` | positive (+ to -), negative (- to +) or both. |
| `space` |  | object or world: whose origin and axes the plane uses. |
| `weld` |  | Weld points on the plane. |
| `tolerance` |  | Points this close to the plane are cut onto it and welded, cm. |
| `flip` |  | Reflect the mesh across the plane rather than adding a mirrored copy: the original is not kept. |
| `axisDirection` | `ad` | See the command reference. |
| `mergeMode` | `mm` | See the command reference. |
| `mergeThreshold` | `mt` | See the command reference. |
| `mergeThresholdType` | `mtt` | See the command reference. |
| `mirrorAxis` | `ma` | See the command reference. |
| `mirrorPosition` | `mps` | See the command reference. |
| `pivot` | `p` | See the command reference. |
| `pivotX` | `px` | See the command reference. |
| `pivotY` | `py` | See the command reference. |
| `pivotZ` | `pz` | See the command reference. |
| `worldSpace` | `ws` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |
| `cutMesh` | `cm` | Cut off what lies past the plane first. |
| `flipUVs` | `fuv` | Reflect the new faces' U. |

## polyMirrorFace

Mirror a mesh in place, welding along the plane: -a axis, -ad direction, -ma 0 bounding box, 1 object, 2 world, -mps plane position, -cutMesh removes what crosses it, -mm 0 leaves borders open, -mt how close welds. Returns the node.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `worldInverse` |  | The object's inverse world matrix when the edit was made. |
| `cutMesh` | `cm` | Cut off what lies past the plane first. |
| `flipUVs` | `fuv` | Reflect the new faces' U. |
| `smoothingAngle` | `sma` | Accepted. |
| `axis` | `a` | See the command reference. |
| `axisDirection` | `ad` | See the command reference. |
| `direction` | `d` | See the command reference. |
| `mergeMode` | `mm` | See the command reference. |
| `mergeThreshold` | `mt` | See the command reference. |
| `mergeThresholdType` | `mtt` | See the command reference. |
| `mirrorAxis` | `ma` | See the command reference. |
| `mirrorPosition` | `mps` | See the command reference. |
| `pivot` | `p` | See the command reference. |
| `pivotX` | `px` | See the command reference. |
| `pivotY` | `py` | See the command reference. |
| `pivotZ` | `pz` | See the command reference. |
| `worldSpace` | `ws` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyMoveEdge

Move edges: -t -r -s about the pivot, -lt -ls -lr each in its own frame. Returns the node.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `worldInverse` |  | The object's inverse world matrix when the edit was made. |
| `gain` | `ga` | See the command reference. |
| `localCenter` | `lc` | See the command reference. |
| `localDirection` | `ld` | See the command reference. |
| `localDirectionX` | `ldx` | See the command reference. |
| `localDirectionY` | `ldy` | See the command reference. |
| `localDirectionZ` | `ldz` | See the command reference. |
| `localRotate` | `lr` | See the command reference. |
| `localRotateX` | `lrx` | See the command reference. |
| `localRotateY` | `lry` | See the command reference. |
| `localRotateZ` | `lrz` | See the command reference. |
| `localScale` | `ls` | See the command reference. |
| `localScaleX` | `lsx` | See the command reference. |
| `localScaleY` | `lsy` | See the command reference. |
| `localScaleZ` | `lsz` | See the command reference. |
| `localTranslate` | `lt` | See the command reference. |
| `localTranslateX` | `ltx` | See the command reference. |
| `localTranslateY` | `lty` | See the command reference. |
| `localTranslateZ` | `ltz` | See the command reference. |
| `pivot` | `pvt` | See the command reference. |
| `pivotX` | `pvx` | See the command reference. |
| `pivotY` | `pvy` | See the command reference. |
| `pivotZ` | `pvz` | See the command reference. |
| `random` | `ran` | See the command reference. |
| `rotate` | `ro` | See the command reference. |
| `rotateX` | `rx` | See the command reference. |
| `rotateY` | `ry` | See the command reference. |
| `rotateZ` | `rz` | See the command reference. |
| `scale` | `s` | See the command reference. |
| `scaleX` | `sx` | See the command reference. |
| `scaleY` | `sy` | See the command reference. |
| `scaleZ` | `sz` | See the command reference. |
| `translate` | `t` | See the command reference. |
| `translateX` | `tx` | See the command reference. |
| `translateY` | `ty` | See the command reference. |
| `translateZ` | `tz` | See the command reference. |
| `worldSpace` | `ws` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyMoveFacet

Move faces: -t -r -s about the pivot, -ltz -ls -lr each in its own frame. Returns the node.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `worldInverse` |  | The object's inverse world matrix when the edit was made. |
| `attraction` | `att` | See the command reference. |
| `gain` | `ga` | See the command reference. |
| `gravity` | `g` | See the command reference. |
| `gravityX` | `gx` | See the command reference. |
| `gravityY` | `gy` | See the command reference. |
| `gravityZ` | `gz` | See the command reference. |
| `localCenter` | `lc` | See the command reference. |
| `localDirection` | `ld` | See the command reference. |
| `localDirectionX` | `ldx` | See the command reference. |
| `localDirectionY` | `ldy` | See the command reference. |
| `localDirectionZ` | `ldz` | See the command reference. |
| `localRotate` | `lr` | See the command reference. |
| `localRotateX` | `lrx` | See the command reference. |
| `localRotateY` | `lry` | See the command reference. |
| `localRotateZ` | `lrz` | See the command reference. |
| `localScale` | `ls` | See the command reference. |
| `localScaleX` | `lsx` | See the command reference. |
| `localScaleY` | `lsy` | See the command reference. |
| `localScaleZ` | `lsz` | See the command reference. |
| `localTranslate` | `lt` | See the command reference. |
| `localTranslateX` | `ltx` | See the command reference. |
| `localTranslateY` | `lty` | See the command reference. |
| `localTranslateZ` | `ltz` | See the command reference. |
| `magnX` | `mx` | See the command reference. |
| `magnY` | `my` | See the command reference. |
| `magnZ` | `mz` | See the command reference. |
| `magnet` | `m` | See the command reference. |
| `offset` | `off` | See the command reference. |
| `pivot` | `pvt` | See the command reference. |
| `pivotX` | `pvx` | See the command reference. |
| `pivotY` | `pvy` | See the command reference. |
| `pivotZ` | `pvz` | See the command reference. |
| `random` | `ran` | See the command reference. |
| `rotate` | `ro` | See the command reference. |
| `rotateX` | `rx` | See the command reference. |
| `rotateY` | `ry` | See the command reference. |
| `rotateZ` | `rz` | See the command reference. |
| `scale` | `s` | See the command reference. |
| `scaleX` | `sx` | See the command reference. |
| `scaleY` | `sy` | See the command reference. |
| `scaleZ` | `sz` | See the command reference. |
| `translate` | `t` | See the command reference. |
| `translateX` | `tx` | See the command reference. |
| `translateY` | `ty` | See the command reference. |
| `translateZ` | `tz` | See the command reference. |
| `weight` | `w` | See the command reference. |
| `worldSpace` | `ws` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyMoveFacetUV

Move UVs of faces: polyMoveUV under its other name.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `translate` | `t` | U and V offset. |
| `scale` | `s` | U and V scale. |
| `pivot` | `pvt` | U and V pivot. |
| `axisLen` | `l` | See the command reference. |
| `axisLenX` | `lx` | See the command reference. |
| `axisLenY` | `ly` | See the command reference. |
| `pivotU` | `pvu` | See the command reference. |
| `pivotV` | `pvv` | See the command reference. |
| `random` | `ran` | See the command reference. |
| `rotationAngle` | `ra` | See the command reference. |
| `scaleU` | `su` | See the command reference. |
| `scaleV` | `sv` | See the command reference. |
| `translateU` | `tu` | See the command reference. |
| `translateV` | `tv` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyMoveUV

Move UVs: -t -tu -tv, -ra degrees, -s -su -sv, about -pvt. Returns the node.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `translate` | `t` | U and V offset. |
| `scale` | `s` | U and V scale. |
| `pivot` | `pvt` | U and V pivot. |
| `axisLen` | `l` | See the command reference. |
| `axisLenX` | `lx` | See the command reference. |
| `axisLenY` | `ly` | See the command reference. |
| `pivotU` | `pvu` | See the command reference. |
| `pivotV` | `pvv` | See the command reference. |
| `random` | `ran` | See the command reference. |
| `rotationAngle` | `ra` | See the command reference. |
| `scaleU` | `su` | See the command reference. |
| `scaleV` | `sv` | See the command reference. |
| `translateU` | `tu` | See the command reference. |
| `translateV` | `tv` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyMoveVertex

Move vertices: -t -r -s about the pivot, -ltz along each normal. Returns the node.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `worldInverse` |  | The object's inverse world matrix when the edit was made. |
| `gain` | `ga` | See the command reference. |
| `localDirection` | `ld` | See the command reference. |
| `localDirectionX` | `ldx` | See the command reference. |
| `localDirectionY` | `ldy` | See the command reference. |
| `localDirectionZ` | `ldz` | See the command reference. |
| `localTranslate` | `lt` | See the command reference. |
| `localTranslateX` | `ltx` | See the command reference. |
| `localTranslateY` | `lty` | See the command reference. |
| `localTranslateZ` | `ltz` | See the command reference. |
| `pivot` | `pvt` | See the command reference. |
| `pivotX` | `pvx` | See the command reference. |
| `pivotY` | `pvy` | See the command reference. |
| `pivotZ` | `pvz` | See the command reference. |
| `random` | `ran` | See the command reference. |
| `rotate` | `ro` | See the command reference. |
| `rotateX` | `rx` | See the command reference. |
| `rotateY` | `ry` | See the command reference. |
| `rotateZ` | `rz` | See the command reference. |
| `scale` | `s` | See the command reference. |
| `scaleX` | `sx` | See the command reference. |
| `scaleY` | `sy` | See the command reference. |
| `scaleZ` | `sz` | See the command reference. |
| `translate` | `t` | See the command reference. |
| `translateX` | `tx` | See the command reference. |
| `translateY` | `ty` | See the command reference. |
| `translateZ` | `tz` | See the command reference. |
| `worldSpace` | `ws` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyNormal

Face winding: -nm 0 reverse, 1 propagate, 2 conform, 3 reverse and cut, 4 reverse and propagate. Returns the node.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `normalMode` | `nm` | See the command reference. |
| `userNormalMode` | `unm` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyNormalPerVertex

Set vertex normals: -xyz or -x -y -z, -rel adds. -fn freezes, -ufn lets them follow the surface again. -q -xyz gives each normal in turn; -q -fn whether they are frozen. Also takes vtxFace components when querying.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `allLocked` | `al` | See the command reference. |
| `deformable` | `deformable` | See the command reference. |
| `freezeNormal` | `fn` | See the command reference. |
| `normalX` | `x` | See the command reference. |
| `normalXYZ` | `xyz` | See the command reference. |
| `normalY` | `y` | See the command reference. |
| `normalZ` | `z` | See the command reference. |
| `relative` | `rel` | See the command reference. |
| `unFreezeNormal` | `ufn` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyNormalizeUV

Fit UVs into 0 to 1: all together (-nt 1) or each shell on its own (-nt 0), keeping proportions unless -pa 0.

Arguments: faces or meshes

| Flag | Short | Description |
| --- | --- | --- |
| `normalizeType` | `nt` | 1 all together, 0 each shell. |
| `preserveAspectRatio` | `pa` | Keep proportions (default on). |

## polyOffsetLoop

Insert an edge loop either side of the selected edges, parallel to them (the reference's Offset Edge Loop). Partial loops end on a border, or inside the mesh by meeting at the end vertex or wrapping round it. Returns the new edges.

| Flag | Short | Description |
| --- | --- | --- |
| `distance` |  | How far out: a fraction of each crossing edge's length, or cm with absolute. |
| `absolute` |  | distance is in cm, square to the selected edges. |
| `side` |  | both, left or right of the edges' direction. |
| `ends` |  | Open ends inside the mesh: converge (meet at the end vertex) or wrap (round it). |
| `deleteEdge` |  | Dissolve the selected edges afterward, leaving the two new loops. |

## polyOptUvs

Relax UVs: flattens each shell again, keeping its place and size. Returns the node.

Arguments: UVs, faces or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `constructionHistory` | `ch` | Accepted: a node is made while history is on. |
| `name` | `n` | Accepted for existing scripts. |
| `caching` | `cch` | Accepted for existing scripts. |
| `nodeState` | `nds` | Accepted for existing scripts. |
| `iterations` | `i` | Accepted: the solver decides. |
| `stoppingThreshold` | `ss` | Accepted: the solver decides. |
| `useScale` | `us` | Accepted for existing scripts. |
| `scale` | `s` | Accepted for existing scripts. |
| `areaWeight` | `aw` | Accepted for existing scripts. |
| `globalBlend` | `gb` | Accepted for existing scripts. |
| `globalMethodBlend` | `gmb` | Accepted for existing scripts. |
| `optimizeAxis` | `oa` | Accepted for existing scripts. |
| `pinSelected` | `ps` | Accepted for existing scripts. |
| `pinUvBorder` | `pub` | Accepted for existing scripts. |
| `applyToShell` | `as` | Accepted: shells are what it works on. |

## polyOptions

Display options for meshes: -bc -dn -sn -ds -dt -dv -dce and the rest are remembered and read back with -q. They set state for scripts; most don't change the viewport.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `activeObjects` | `ao` | See the command reference. |
| `allEdges` | `ae` | See the command reference. |
| `backCullVertex` | `bcv` | See the command reference. |
| `backCulling` | `bc` | See the command reference. |
| `colorMaterialChannel` | `cm` | See the command reference. |
| `colorShadedDisplay` | `cs` | See the command reference. |
| `displayAlphaAsGreyScale` | `dal` | See the command reference. |
| `displayBlueChannel` | `dbc` | See the command reference. |
| `displayBorder` | `db` | See the command reference. |
| `displayCenter` | `dc` | See the command reference. |
| `displayCreaseEdge` | `dce` | See the command reference. |
| `displayCreaseVertex` | `dcv` | See the command reference. |
| `displayGeometry` | `dg` | See the command reference. |
| `displayGreenChannel` | `dgc` | See the command reference. |
| `displayInvisibleFaces` | `dif` | See the command reference. |
| `displayItemNumbers` | `din` | See the command reference. |
| `displayMapBorder` | `dmb` | See the command reference. |
| `displayMetadata` | `dmt` | See the command reference. |
| `displayNormal` | `dn` | See the command reference. |
| `displayRedChannel` | `drc` | See the command reference. |
| `displaySubdComps` | `dsc` | See the command reference. |
| `displayTangent` | `dtn` | See the command reference. |
| `displayTriangle` | `dt` | See the command reference. |
| `displayUVTopology` | `uvt` | See the command reference. |
| `displayUVs` | `duv` | See the command reference. |
| `displayVertex` | `dv` | See the command reference. |
| `displayWarp` | `dw` | See the command reference. |
| `facet` | `f` | See the command reference. |
| `fullBack` | `fb` | See the command reference. |
| `gl` | `gl` | See the command reference. |
| `hardBack` | `hb` | See the command reference. |
| `hardEdge` | `he` | See the command reference. |
| `hardEdgeColor` | `hec` | See the command reference. |
| `materialBlend` | `mb` | See the command reference. |
| `newPolymesh` | `np` | See the command reference. |
| `point` | `pt` | See the command reference. |
| `pointFacet` | `pf` | See the command reference. |
| `relative` | `r` | See the command reference. |
| `reuseTriangles` | `rt` | See the command reference. |
| `sizeBorder` | `sb` | See the command reference. |
| `sizeNormal` | `sn` | See the command reference. |
| `sizeUV` | `suv` | See the command reference. |
| `sizeVertex` | `sv` | See the command reference. |
| `smoothDrawType` | `sdt` | See the command reference. |
| `softEdge` | `se` | See the command reference. |
| `vertexNormalMethod` | `vnm` | See the command reference. |
| `wireBackCulling` | `wbc` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyOutput

A mesh's description as text: -v, -ed, -f, -fn; -a all, the default; -of a file.

Arguments: meshes

| Flag | Short | Description |
| --- | --- | --- |
| `allValues` | `a` | Everything. |
| `vert` | `v` | Vertices. |
| `edge` | `ed` | Edges. |
| `face` | `f` | Faces. |
| `faceNorm` | `fn` | Face normals. |
| `vertNorm` | `vn` | Accepted for existing scripts. |
| `triangle` | `t` | Accepted for existing scripts. |
| `uvValue` | `uv` | Accepted for existing scripts. |
| `color` | `c` | Accepted for existing scripts. |
| `group` | `g` | Accepted for existing scripts. |
| `outputFile` | `of` | Write here. |

## polyPasteFace

Paste the clipboard's UVs and colours onto faces. Returns nothing.

Arguments: faces (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `constructionHistory` | `ch` | Accepted. |
| `name` | `n` | Accepted. |

## polyPinUV

Pin UVs so Unfold holds them: -v 1 pins, 0 unpins; -op 1 adds, 2 clears everything. -unp works on the rest. Changes the mesh directly.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `operation` | `op` | See the command reference. |
| `unpinned` | `unp` | See the command reference. |
| `uvSetName` | `uvs` | See the command reference. |
| `value` | `v` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyPipe

Create a Pipe mesh. Returns [mesh, generator] (generator is None with generator=False).

| Flag | Short | Description |
| --- | --- | --- |
| `name` | `n` | Name of the mesh; made unique. |
| `position` |  | World position. |
| `rotation` |  | Rotation in degrees. |
| `normal` |  | Turn the up axis to this direction, as for placing on a surface; a dome cut by fraction then sits on position by its open base. |
| `generator` |  | Keep the generator node so its parameters stay editable. |
| `object` | `o` | Accepted: the transform and shape are always made. |
| `createUVs` | `cuv` | 0 leaves the mesh without UVs. |
| `texture` | `tx` | How UVs are laid out; accepted. |
| `axis` | `ax` | The direction the primitive's up points, as a vector. |
| `caching` | `cch` | Accepted. |
| `nodeState` | `nds` | Accepted. |
| `constructionHistory` | `ch` | False: no generator node, as generator=False. |
| `radius` | `r` | Outer radius, cm. |
| `thickness` | `t` | Wall thickness, cm. |
| `height` | `h` | cm. |
| `subdivisionsAxis` | `sa` | Segments around. |
| `subdivisionsHeight` | `sh` | Segments up. |
| `subdivisionsCaps` | `sc` | Accepted; each end is one ring. |
| `roundCap` | `rcp` | Accepted; the ends stay flat. |
| `baseAtOrigin` |  | Sit on the ground. |
| `fraction` |  | Portion of a full turn swept around the up axis: 1 is whole, 0.5 half. |
| `cutCaps` |  | Close the flat ends a partial sweep leaves. |
| `subdivisionsX` | `sx` | Another name for the attribute it sets. |
| `subdivisionsY` | `sy` | Another name for the attribute it sets. |
| `subdivisionsZ` | `sz` | Another name for the attribute it sets. |
| `upAxis` |  | Direction the primitive's up points: x, y, z, -x, -y or -z (or a vector, as the reference's -ax 1 0 0); the scene up axis when left out. |

## polyPlane

Create a Plane mesh. Returns [mesh, generator] (generator is None with generator=False).

| Flag | Short | Description |
| --- | --- | --- |
| `name` | `n` | Name of the mesh; made unique. |
| `position` |  | World position. |
| `rotation` |  | Rotation in degrees. |
| `normal` |  | Turn the up axis to this direction, as for placing on a surface; a dome cut by fraction then sits on position by its open base. |
| `generator` |  | Keep the generator node so its parameters stay editable. |
| `object` | `o` | Accepted: the transform and shape are always made. |
| `createUVs` | `cuv` | 0 leaves the mesh without UVs. |
| `texture` | `tx` | How UVs are laid out; accepted. |
| `axis` | `ax` | The direction the primitive's up points, as a vector. |
| `caching` | `cch` | Accepted. |
| `nodeState` | `nds` | Accepted. |
| `constructionHistory` | `ch` | False: no generator node, as generator=False. |
| `width` | `w` | Size along X, cm. |
| `height` | `h` | Size along Z, cm. |
| `subdivisionsWidth` | `sw` | Segments across. |
| `subdivisionsHeight` | `sh` | Segments deep. |
| `subdivisionsX` | `sx` | Another name for the attribute it sets. |
| `subdivisionsY` | `sy` | Another name for the attribute it sets. |
| `subdivisionsZ` | `sz` | Another name for the attribute it sets. |
| `depth` |  | Another name for the attribute it sets. |
| `upAxis` |  | Direction the primitive's up points: x, y, z, -x, -y or -z (or a vector, as the reference's -ax 1 0 0); the scene up axis when left out. |

## polyPlatonicSolid

Create a Solid mesh. Returns [mesh, generator] (generator is None with generator=False).

| Flag | Short | Description |
| --- | --- | --- |
| `name` | `n` | Name of the mesh; made unique. |
| `position` |  | World position. |
| `rotation` |  | Rotation in degrees. |
| `normal` |  | Turn the up axis to this direction, as for placing on a surface; a dome cut by fraction then sits on position by its open base. |
| `generator` |  | Keep the generator node so its parameters stay editable. |
| `object` | `o` | Accepted: the transform and shape are always made. |
| `createUVs` | `cuv` | 0 leaves the mesh without UVs. |
| `texture` | `tx` | How UVs are laid out; accepted. |
| `axis` | `ax` | The direction the primitive's up points, as a vector. |
| `caching` | `cch` | Accepted. |
| `nodeState` | `nds` | Accepted. |
| `constructionHistory` | `ch` | False: no generator node, as generator=False. |
| `radius` | `r` | Centre to each point, cm. |
| `sideLength` | `w` | Accepted; the radius sets the size. |
| `solidType` | `st` | 0 dodecahedron, 1 icosahedron, 2 octahedron, 3 tetrahedron. |
| `upAxis` |  | Direction the primitive's up points: x, y, z, -x, -y or -z (or a vector, as the reference's -ax 1 0 0); the scene up axis when left out. |

## polyPoke

Fan faces into triangles round a new point, moved by -lt along each face's frame or -t. Returns the node.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `worldInverse` |  | The object's inverse world matrix when the edit was made. |
| `localTranslate` | `lt` | See the command reference. |
| `localTranslateX` | `ltx` | See the command reference. |
| `localTranslateY` | `lty` | See the command reference. |
| `localTranslateZ` | `ltz` | See the command reference. |
| `translate` | `t` | See the command reference. |
| `translateX` | `tx` | See the command reference. |
| `translateY` | `ty` | See the command reference. |
| `translateZ` | `tz` | See the command reference. |
| `worldSpace` | `ws` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyPrimitive

Create a Solid mesh. Returns [mesh, generator] (generator is None with generator=False).

| Flag | Short | Description |
| --- | --- | --- |
| `name` | `n` | Name of the mesh; made unique. |
| `position` |  | World position. |
| `rotation` |  | Rotation in degrees. |
| `normal` |  | Turn the up axis to this direction, as for placing on a surface; a dome cut by fraction then sits on position by its open base. |
| `generator` |  | Keep the generator node so its parameters stay editable. |
| `object` | `o` | Accepted: the transform and shape are always made. |
| `createUVs` | `cuv` | 0 leaves the mesh without UVs. |
| `texture` | `tx` | How UVs are laid out; accepted. |
| `axis` | `ax` | The direction the primitive's up points, as a vector. |
| `caching` | `cch` | Accepted. |
| `nodeState` | `nds` | Accepted. |
| `constructionHistory` | `ch` | False: no generator node, as generator=False. |
| `radius` | `r` | Centre to each point, cm. |
| `sideLength` | `w` | Accepted; the radius sets the size. |
| `polyType` | `pt` | 0 soccer ball. |
| `upAxis` |  | Direction the primitive's up points: x, y, z, -x, -y or -z (or a vector, as the reference's -ax 1 0 0); the scene up axis when left out. |

## polyPrism

Create a Prism mesh. Returns [mesh, generator] (generator is None with generator=False).

| Flag | Short | Description |
| --- | --- | --- |
| `name` | `n` | Name of the mesh; made unique. |
| `position` |  | World position. |
| `rotation` |  | Rotation in degrees. |
| `normal` |  | Turn the up axis to this direction, as for placing on a surface; a dome cut by fraction then sits on position by its open base. |
| `generator` |  | Keep the generator node so its parameters stay editable. |
| `object` | `o` | Accepted: the transform and shape are always made. |
| `createUVs` | `cuv` | 0 leaves the mesh without UVs. |
| `texture` | `tx` | How UVs are laid out; accepted. |
| `axis` | `ax` | The direction the primitive's up points, as a vector. |
| `caching` | `cch` | Accepted. |
| `nodeState` | `nds` | Accepted. |
| `constructionHistory` | `ch` | False: no generator node, as generator=False. |
| `length` | `l` | Height, cm. |
| `sideLength` | `w` | Length of each side of the ends, cm. |
| `numberOfSides` | `ns` | Sides. |
| `subdivisionsHeight` | `sh` | Segments up. |
| `subdivisionsCaps` | `sc` | Rings on each end. |
| `upAxis` |  | Direction the primitive's up points: x, y, z, -x, -y or -z (or a vector, as the reference's -ax 1 0 0); the scene up axis when left out. |

## polyProjectCurve

Project a curve onto a mesh along -d: makes a curve lying on the surface, for polySplit -pc. Takes the curve, then the mesh. Returns the new curve's transform.

Arguments: the curve, then the mesh

| Flag | Short | Description |
| --- | --- | --- |
| `addUnderTransform` | `aut` | See the command reference. |
| `direction` | `d` | See the command reference. |
| `directionX` | `dx` | See the command reference. |
| `directionY` | `dy` | See the command reference. |
| `directionZ` | `dz` | See the command reference. |
| `tolerance` | `tol` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyProjection

Project UVs of the selected faces (or meshes) through a plane, cylinder or sphere. -md picks the plane's axis (x, y, z) or b for best fit.

Arguments: faces or meshes

| Flag | Short | Description |
| --- | --- | --- |
| `type` | `t` | planar, cylindrical or spherical. |
| `mapDirection` | `md` | x, y, z, or b (best plane). |
| `constructionHistory` | `ch` | Accepted for existing scripts. |

## polyPyramid

Create a Pyramid mesh. Returns [mesh, generator] (generator is None with generator=False).

| Flag | Short | Description |
| --- | --- | --- |
| `name` | `n` | Name of the mesh; made unique. |
| `position` |  | World position. |
| `rotation` |  | Rotation in degrees. |
| `normal` |  | Turn the up axis to this direction, as for placing on a surface; a dome cut by fraction then sits on position by its open base. |
| `generator` |  | Keep the generator node so its parameters stay editable. |
| `object` | `o` | Accepted: the transform and shape are always made. |
| `createUVs` | `cuv` | 0 leaves the mesh without UVs. |
| `texture` | `tx` | How UVs are laid out; accepted. |
| `axis` | `ax` | The direction the primitive's up points, as a vector. |
| `caching` | `cch` | Accepted. |
| `nodeState` | `nds` | Accepted. |
| `constructionHistory` | `ch` | False: no generator node, as generator=False. |
| `sideLength` | `w` | Length of each side, cm. |
| `numberOfSides` | `ns` | Sides of the base. |
| `subdivisionsHeight` | `sh` | Segments up. |
| `subdivisionsCaps` | `sc` | Rings on the base. |
| `upAxis` |  | Direction the primitive's up points: x, y, z, -x, -y or -z (or a vector, as the reference's -ax 1 0 0); the scene up axis when left out. |

## polyQuad

Join triangles into quads where they meet at less than -a degrees. Returns the node.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `angle` | `a` | See the command reference. |
| `keepGroupBorder` | `kgb` | See the command reference. |
| `keepHardEdges` | `khe` | See the command reference. |
| `keepTextureBorders` | `ktb` | See the command reference. |
| `worldSpace` | `ws` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyQuadSphere

Create a QuadSphere mesh. Returns [mesh, generator] (generator is None with generator=False).

| Flag | Short | Description |
| --- | --- | --- |
| `name` | `n` | Name of the mesh; made unique. |
| `position` |  | World position. |
| `rotation` |  | Rotation in degrees. |
| `normal` |  | Turn the up axis to this direction, as for placing on a surface; a dome cut by fraction then sits on position by its open base. |
| `generator` |  | Keep the generator node so its parameters stay editable. |
| `object` | `o` | Accepted: the transform and shape are always made. |
| `createUVs` | `cuv` | 0 leaves the mesh without UVs. |
| `texture` | `tx` | How UVs are laid out; accepted. |
| `axis` | `ax` | The direction the primitive's up points, as a vector. |
| `caching` | `cch` | Accepted. |
| `nodeState` | `nds` | Accepted. |
| `constructionHistory` | `ch` | False: no generator node, as generator=False. |
| `radius` | `r` | cm. |
| `subdivisions` |  | Segments per cube face edge. |
| `fraction` |  | Portion of the height kept, from the top: 1 is whole, 0.5 a dome open at the bottom. |
| `upAxis` |  | Direction the primitive's up points: x, y, z, -x, -y or -z (or a vector, as the reference's -ax 1 0 0); the scene up axis when left out. |

## polyQuadrangulate

Join neighbouring triangles into quads.

| Flag | Short | Description |
| --- | --- | --- |
| `maxAngle` |  | Largest angle between the two triangles, degrees. |

## polyQueryBlindData

Read blind data back: -id, -at, -ldn or -ss a part of the name, -sc lists each component before its value.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `typeId` | `id` | The data's type id. |
| `associationType` | `at` | face, vertex or edge. |
| `longDataName` | `ldn` | The name. |
| `shortDataName` | `sdn` | The short name. |
| `subString` | `ss` | A part of the name. |
| `showComp` | `sc` | List the components too. |

## polyReduce

Reduce the polygon count of whole meshes while keeping their shape, silhouette, UVs and normals. The result is triangles (or quads with quads=True). A mesh with modifiers gets a decimate modifier on top instead. Honours the mesh's symmetry: one half (or one radial wedge) is reduced and mirrored (or repeated).

Arguments: objects

| Flag | Short | Description |
| --- | --- | --- |
| `ratio` |  | Fraction of triangles to keep, 0 to 1. |
| `targetTriangles` |  | Triangles to keep; above 0 it overrides ratio. |
| `keepBorders` |  | Open borders stay exactly where they are. |
| `keepHardEdges` |  | Hard edges stay intact, as UV seams and material borders always do; off lets them collapse for a lower count. |
| `symmetry` |  | auto (the mesh's symmetry setting), off, x, y or z. |
| `quads` |  | Join triangle pairs back into quads where they make good ones. |
| `cachingReduce` | `cr` | See the command reference. |
| `colorWeights` | `cwt` | See the command reference. |
| `compactness` | `com` | See the command reference. |
| `geomWeights` | `gwt` | See the command reference. |
| `invertVertexWeights` | `iwt` | See the command reference. |
| `keepBorder` | `kb` | See the command reference. |
| `keepBorderWeight` | `kbw` | See the command reference. |
| `keepColorBorder` | `kcb` | See the command reference. |
| `keepColorBorderWeight` | `kcw` | See the command reference. |
| `keepCreaseEdge` | `kce` | See the command reference. |
| `keepCreaseEdgeWeight` | `cew` | See the command reference. |
| `keepFaceGroupBorder` | `kfb` | See the command reference. |
| `keepFaceGroupBorderWeight` | `kfw` | See the command reference. |
| `keepHardEdge` | `khe` | See the command reference. |
| `keepHardEdgeWeight` | `khw` | See the command reference. |
| `keepMapBorder` | `kmb` | See the command reference. |
| `keepMapBorderWeight` | `kmw` | See the command reference. |
| `keepOriginalVertices` | `kev` | See the command reference. |
| `keepQuadsWeight` | `kqw` | See the command reference. |
| `percentage` | `p` | See the command reference. |
| `preserveLocation` | `pl` | See the command reference. |
| `preserveTopology` | `top` | See the command reference. |
| `replaceOriginal` | `rpo` | See the command reference. |
| `sharpness` | `shp` | See the command reference. |
| `symmetryPlaneW` | `sw` | See the command reference. |
| `symmetryPlaneX` | `sx` | See the command reference. |
| `symmetryPlaneY` | `sy` | See the command reference. |
| `symmetryPlaneZ` | `sz` | See the command reference. |
| `symmetryTolerance` | `stl` | See the command reference. |
| `termination` | `trm` | See the command reference. |
| `triangleCount` | `tct` | See the command reference. |
| `triangulate` | `t` | See the command reference. |
| `useVirtualSymmetry` | `uvs` | See the command reference. |
| `uvWeights` | `uwt` | See the command reference. |
| `version` | `ver` | See the command reference. |
| `vertexCount` | `vct` | See the command reference. |
| `vertexMapName` | `vmp` | See the command reference. |
| `vertexWeightCoefficient` | `vwc` | See the command reference. |
| `weightCoefficient` | `wc` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyRemesh

Isotropic remesh of faces or meshes: edges over -mel are split, under -cot percent of it collapsed, -smt relaxes. -urv reads -mel against the average edge length. Returns the node.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `collapseThreshold` | `cot` | See the command reference. |
| `interpolationType` | `ipt` | See the command reference. |
| `maxEdgeLength` | `mel` | See the command reference. |
| `smoothStrength` | `smt` | See the command reference. |
| `tessellateBorders` | `tsb` | See the command reference. |
| `useRelativeValues` | `urv` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyRetopo

Automatic retopology: rebuilds meshes as even quad-dominant faces, about -targetFaceCount of them. Returns the node.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `targetFaceCount` | `tfc` | About how many faces. |
| `targetFaceCountTolerance` | `tft` | Accepted. |
| `preserveHardEdges` | `phe` | Accepted. |
| `symmetry` | `sym` | Accepted. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyRotateUVs

Turn UVs about the middle of their bounds, degrees counterclockwise.

Arguments: angle, then UVs, faces or meshes

## polySelect

Select along the mesh: -el edge loop, -er ring, -eb border, -elb loop or border, -elp -erp -ebp a stretch between two edges, -lpt -rpt -bpt every nth, -sep shortest path, -sfp between faces, -ets a shell. -r -add -d -tgl say how; -ns only reports; -ass gives component strings, else ids.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `add` | `add` | See the command reference. |
| `addFirst` | `af` | See the command reference. |
| `asSelectString` | `ass` | See the command reference. |
| `deselect` | `d` | See the command reference. |
| `edgeBorder` | `eb` | See the command reference. |
| `edgeBorderPath` | `ebp` | See the command reference. |
| `edgeBorderPattern` | `bpt` | See the command reference. |
| `edgeLoop` | `el` | See the command reference. |
| `edgeLoopOrBorder` | `elb` | See the command reference. |
| `edgeLoopOrBorderPattern` | `lbp` | See the command reference. |
| `edgeLoopPath` | `elp` | See the command reference. |
| `edgeLoopPattern` | `lpt` | See the command reference. |
| `edgeRing` | `er` | See the command reference. |
| `edgeRingPath` | `erp` | See the command reference. |
| `edgeRingPattern` | `rpt` | See the command reference. |
| `edgeUVLoopOrBorder` | `euv` | See the command reference. |
| `everyN` | `en` | See the command reference. |
| `extendToShell` | `ets` | See the command reference. |
| `noSelection` | `ns` | See the command reference. |
| `replace` | `r` | See the command reference. |
| `shortestEdgePath` | `sep` | See the command reference. |
| `shortestEdgePathUV` | `spu` | See the command reference. |
| `shortestFacePath` | `sfp` | See the command reference. |
| `toggle` | `tgl` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polySelectConstraint

Constrain what selects: a state kept until changed. -m 0 off, 1 next, 2 current and next, 3 all and next; -t what kind; then -where, -sz, -bo, -sh, -cr, -l -lb, -a -ab, -ga -gab, -nm, -tp, -p, -order, -r -rr and more. -pp 1 grows, 2 shrinks, 3 borders. -rs reports instead of selecting. -dis clears.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `mode` | `m` | 0 off, 1 next, 2 current and next, 3 all and next. |
| `angle` | `a` | See the command reference. |
| `anglePropagation` | `ap` | See the command reference. |
| `angleTolerance` | `at` | See the command reference. |
| `anglebound` | `ab` | See the command reference. |
| `border` | `bo` | See the command reference. |
| `borderPropagation` | `bp` | See the command reference. |
| `convexity` | `c` | See the command reference. |
| `crease` | `cr` | See the command reference. |
| `disable` | `dis` | See the command reference. |
| `dist` | `d` | See the command reference. |
| `distaxis` | `da` | See the command reference. |
| `distbound` | `db` | See the command reference. |
| `distpoint` | `dp` | See the command reference. |
| `edgeDistance` | `ed` | See the command reference. |
| `geometricarea` | `ga` | See the command reference. |
| `geometricareabound` | `gab` | See the command reference. |
| `holes` | `h` | See the command reference. |
| `length` | `l` | See the command reference. |
| `lengthbound` | `lb` | See the command reference. |
| `loopPropagation` | `lp` | See the command reference. |
| `max2dAngle` | `m2a` | See the command reference. |
| `max3dAngle` | `m3a` | See the command reference. |
| `nonmanifold` | `nm` | See the command reference. |
| `oppositeEdges` | `oe` | See the command reference. |
| `order` | `order` | See the command reference. |
| `orderbound` | `orb` | See the command reference. |
| `orient` | `o` | See the command reference. |
| `orientaxis` | `oa` | See the command reference. |
| `orientbound` | `ob` | See the command reference. |
| `planarity` | `p` | See the command reference. |
| `propagate` | `pp` | See the command reference. |
| `random` | `r` | See the command reference. |
| `randomratio` | `rr` | See the command reference. |
| `returnSelection` | `rs` | See the command reference. |
| `ringPropagation` | `rp` | See the command reference. |
| `shell` | `sh` | See the command reference. |
| `size` | `sz` | See the command reference. |
| `smoothness` | `sm` | See the command reference. |
| `stateString` | `sts` | See the command reference. |
| `textured` | `tx` | See the command reference. |
| `texturedarea` | `ta` | See the command reference. |
| `texturedareabound` | `tab` | See the command reference. |
| `textureshared` | `ts` | See the command reference. |
| `topology` | `tp` | See the command reference. |
| `type` | `t` | See the command reference. |
| `uvBorderSelection` | `ubs` | See the command reference. |
| `uvConstraint` | `uvc` | See the command reference. |
| `uvEdgeLoopPropagation` | `ulp` | See the command reference. |
| `uvEdgeRingPropagation` | `urp` | See the command reference. |
| `uvFaceOrientation` | `ufo` | See the command reference. |
| `uvShell` | `uv` | See the command reference. |
| `visibility` | `v` | See the command reference. |
| `visibilityangle` | `va` | See the command reference. |
| `visibilitypoint` | `vp` | See the command reference. |
| `where` | `w` | See the command reference. |
| `wholeSensitive` | `ws` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polySelectCtx

A polySelectCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `mode` | `m` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## polySelectEditCtx

A polySelectEditCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `mode` | `m` |  |
| `insertWithEdgeFlow` | `ief` |  |
| `insertWithEdgeFlowEdge` | `ife` |  |
| `relativeInsert` | `r` |  |
| `splitType` | `stp` |  |
| `divisions` | `div` |  |
| `absoluteOffset` | `ao` |  |
| `autoComplete` | `ac` |  |
| `fixQuads` | `fq` |  |
| `smoothingAngle` | `sma` |  |
| `useEqualMultiplier` | `uem` |  |
| `deleteEdge` | `de` |  |
| `adjustEdgeFlow` | `aef` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## polySelectSp

Select from what is selected: -loop extends to edge loops, -ring to rings. Returns what was selected.

Arguments: edges

| Flag | Short | Description |
| --- | --- | --- |
| `loop` | `lp` | Edge loops. |
| `ring` | `rg` | Edge rings. |

## polySeparate

Separate a mesh into one mesh per shell, children of its transform, each fed by a polySeparate node. -ch 0 makes them independent objects and removes the original. Returns the new transforms, then the nodes.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `shell` |  | Which shell this node keeps. |
| `removeShells` | `rs` | See the command reference. |
| `separateSpecificShell` | `sss` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polySetToFaceNormal

Set each vertex normal of the faces to its face's normal, freezing it. Returns the node.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `setUserNormal` | `su` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polySewEdge

Sew border edges together where their ends are within -tx. Returns the node.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |
| `tolerance` | `tx` | How close the ends must be. |
| `texture` | `tx2` | Accepted for existing scripts. |
| `worldSpace` | `ws` | Accepted for existing scripts. |

## polyShortestPathCtx

A polyShortestPathCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `pathOnEdge` | `poe` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## polySlideEdge

Slide the selected edges within the faces beside them: -v -1 to 1, a fraction of the way along the neighbouring edges to one side or the other.

| Flag | Short | Description |
| --- | --- | --- |
| `value` | `v` | How far: -1 to 1. |
| `absolute` | `a` | Accepted for existing scripts. |
| `direction` | `d` | Accepted for existing scripts. |
| `edgeFlow` | `ef` | Accepted for existing scripts. |
| `symmetry` | `sym` | Accepted for existing scripts. |

## polySmartBevel

Bevel: polyBevel3 under its other name.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `profile` |  | Also accepted under this command's other name. |
| `clamp` |  | Also accepted under this command's other name. |
| `angleTolerance` | `at` | See the command reference. |
| `autoFit` | `af` | See the command reference. |
| `chamfer` | `c` | See the command reference. |
| `depth` | `d` | See the command reference. |
| `fillNgons` | `fn` | See the command reference. |
| `fraction` | `f` | See the command reference. |
| `mergeVertexTolerance` | `mvt` | See the command reference. |
| `mergeVertices` | `mv` | See the command reference. |
| `miterAlong` | `mia` | See the command reference. |
| `mitering` | `m` | See the command reference. |
| `miteringAngle` | `ma` | See the command reference. |
| `offset` | `o` | See the command reference. |
| `offsetAsFraction` | `oaf` | See the command reference. |
| `roundness` | `r` | See the command reference. |
| `segments` | `sg` | See the command reference. |
| `smoothingAngle` | `sa` | See the command reference. |
| `uvAssignment` | `ua` | See the command reference. |
| `worldSpace` | `ws` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polySmooth

Smooth meshes or faces: -mth 0 Catmull-Clark (whole meshes), 1 linear; -dv levels; -kb keep borders sharp, -khe hard edges. Returns the polySmoothFace node.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `continuity` | `c` | See the command reference. |
| `degree` | `deg` | See the command reference. |
| `divisions` | `dv` | See the command reference. |
| `divisionsPerEdge` | `dpe` | See the command reference. |
| `keepBorder` | `kb` | See the command reference. |
| `keepHardEdge` | `khe` | See the command reference. |
| `keepMapBorders` | `kmb` | See the command reference. |
| `keepSelectionBorder` | `ksb` | See the command reference. |
| `keepTesselation` | `xkt` | See the command reference. |
| `keepTessellation` | `kt` | See the command reference. |
| `method` | `mth` | See the command reference. |
| `osdCreaseMethod` | `ocr` | See the command reference. |
| `osdFvarBoundary` | `ofb` | See the command reference. |
| `osdFvarPropagateCorners` | `ofc` | See the command reference. |
| `osdSmoothTriangles` | `ost` | See the command reference. |
| `osdVertBoundary` | `ovb` | See the command reference. |
| `propagateEdgeHardness` | `peh` | See the command reference. |
| `pushStrength` | `ps` | See the command reference. |
| `roundness` | `ro` | See the command reference. |
| `smoothUVs` | `suv` | See the command reference. |
| `subdivisionLevels` | `sl` | See the command reference. |
| `subdivisionType` | `sdt` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polySoccerBall

Create a SoccerBall mesh. Returns [mesh, generator] (generator is None with generator=False).

| Flag | Short | Description |
| --- | --- | --- |
| `name` | `n` | Name of the mesh; made unique. |
| `position` |  | World position. |
| `rotation` |  | Rotation in degrees. |
| `normal` |  | Turn the up axis to this direction, as for placing on a surface; a dome cut by fraction then sits on position by its open base. |
| `generator` |  | Keep the generator node so its parameters stay editable. |
| `object` | `o` | Accepted: the transform and shape are always made. |
| `createUVs` | `cuv` | 0 leaves the mesh without UVs. |
| `texture` | `tx` | How UVs are laid out; accepted. |
| `axis` | `ax` | The direction the primitive's up points, as a vector. |
| `caching` | `cch` | Accepted. |
| `nodeState` | `nds` | Accepted. |
| `constructionHistory` | `ch` | False: no generator node, as generator=False. |
| `radius` | `r` | Centre to each point, cm. |
| `upAxis` |  | Direction the primitive's up points: x, y, z, -x, -y or -z (or a vector, as the reference's -ax 1 0 0); the scene up axis when left out. |

## polySoftEdge

Make the selected edges soft or hard by the angle their faces meet at: sharper than angle turns hard, the rest soft. 180 softens every edge, 0 hardens every edge. In object mode, whole selected meshes.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `angle` | `a` | Edges whose faces meet sharper than this turn hard, degrees. |
| `worldSpace` | `ws` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polySphere

Create a Sphere mesh. Returns [mesh, generator] (generator is None with generator=False).

| Flag | Short | Description |
| --- | --- | --- |
| `name` | `n` | Name of the mesh; made unique. |
| `position` |  | World position. |
| `rotation` |  | Rotation in degrees. |
| `normal` |  | Turn the up axis to this direction, as for placing on a surface; a dome cut by fraction then sits on position by its open base. |
| `generator` |  | Keep the generator node so its parameters stay editable. |
| `object` | `o` | Accepted: the transform and shape are always made. |
| `createUVs` | `cuv` | 0 leaves the mesh without UVs. |
| `texture` | `tx` | How UVs are laid out; accepted. |
| `axis` | `ax` | The direction the primitive's up points, as a vector. |
| `caching` | `cch` | Accepted. |
| `nodeState` | `nds` | Accepted. |
| `constructionHistory` | `ch` | False: no generator node, as generator=False. |
| `radius` | `r` | cm. |
| `subdivisionsAxis` | `sa` | Segments around. |
| `subdivisionsHeight` | `sh` | Segments pole to pole. |
| `baseAtOrigin` |  | Sit on the ground. |
| `fraction` |  | Portion of the height kept, from the top: 1 is whole, 0.5 a dome open at the bottom. |
| `subdivisionsX` | `sx` | Another name for the attribute it sets. |
| `subdivisionsY` | `sy` | Another name for the attribute it sets. |
| `upAxis` |  | Direction the primitive's up points: x, y, z, -x, -y or -z (or a vector, as the reference's -ax 1 0 0); the scene up axis when left out. |

## polySpherize

Push the selected components (whole meshes in object mode) onto a sphere fitted to them, or onto an ellipsoid with scale. center and radius override the fit (world space, cm); radiusMode picks the radius from the points otherwise. With alongNormals each point slides along its own normal to the sphere. Returns the sphere used: {center, radius, scale, rotation}, world space.

| Flag | Short | Description |
| --- | --- | --- |
| `amount` |  | How far toward the sphere, 0 to 1; soft selection fades it further. |
| `fit` |  | best (least-squares sphere) or center (around the points' centroid). |
| `radiusMode` |  | Radius when radius is 0: average distance, fit (the fitted radius), min, max or explicit. |
| `radius` |  | Radius, cm; above 0 it replaces radiusMode. |
| `center` |  | Sphere centre, world space cm; default: the fitted centre. |
| `scale` |  | Ellipsoid semi-axis factors along its X, Y and Z. |
| `rotation` |  | Turn of the ellipsoid's axes in world space, degrees (XYZ). |
| `alongNormals` |  | Move each point along its own normal onto the sphere. |

## polySpinEdge

Turn each selected edge one step within its two faces: -of 1 forward, -of -1 backward.

| Flag | Short | Description |
| --- | --- | --- |
| `offset` | `of` | 1 forward, -1 backward. |

## polySplit

Split faces along a path: -ep edge fraction, -fp face x y z, -ip the same in order. -pc splits along a curve lying on the mesh. Returns the node.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `points` |  | The cut's points: {edge, t}, {vertex} or {face, position}, object space. |
| `closed` |  | Also cut from the last point back to the first. |
| `through` |  | Cut every face under the line, hidden faces too. |
| `eye` |  | Object-space eye the cut was seen from. |
| `useEye` |  | eye is set. |
| `direction` |  | Object-space direction the cut was seen along. |
| `useDirection` |  | direction is set. |
| `loopEdge` |  | Cut across this edge's ring instead. |
| `fraction` |  | Where along loopEdge. |
| `adjustEdgeFlow` | `aef` | See the command reference. |
| `detachEdges` | `de` | See the command reference. |
| `edgepoint` | `ep` | See the command reference. |
| `facepoint` | `fp` | See the command reference. |
| `insertWithEdgeFlow` | `ief` | See the command reference. |
| `insertpoint` | `ip` | See the command reference. |
| `projectedCurve` | `pc` | See the command reference. |
| `projectedCurveTolerance` | `pct` | See the command reference. |
| `smoothingangle` | `sma` | See the command reference. |
| `subdivision` | `s` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polySplitCtx

A polySplitCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `precsnap` | `ps` |  |
| `enablesnap` | `es` |  |
| `magnetsnap` | `ms` |  |
| `snaptoedge` | `ste` |  |
| `smoothingangle` | `sma` |  |
| `subdivision` | `s` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## polySplitEdge

Part the faces along edges, wherever the edges divide them at a vertex. Returns the node.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `operation` | `op` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polySplitFacet

polySplit's older name.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `points` |  | The cut's points: {edge, t}, {vertex} or {face, position}, object space. |
| `closed` |  | Also cut from the last point back to the first. |
| `through` |  | Cut every face under the line, hidden faces too. |
| `eye` |  | Object-space eye the cut was seen from. |
| `useEye` |  | eye is set. |
| `direction` |  | Object-space direction the cut was seen along. |
| `useDirection` |  | direction is set. |
| `loopEdge` |  | Cut across this edge's ring instead. |
| `fraction` |  | Where along loopEdge. |
| `adjustEdgeFlow` | `aef` | See the command reference. |
| `detachEdges` | `de` | See the command reference. |
| `edgepoint` | `ep` | See the command reference. |
| `facepoint` | `fp` | See the command reference. |
| `insertWithEdgeFlow` | `ief` | See the command reference. |
| `insertpoint` | `ip` | See the command reference. |
| `projectedCurve` | `pc` | See the command reference. |
| `projectedCurveTolerance` | `pct` | See the command reference. |
| `smoothingangle` | `sma` | See the command reference. |
| `subdivision` | `s` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polySplitRing

Insert edge loops across the ring through the first edge (or -re). -wt where across, 0 to 1. -stp 2 with -div inserts that many evenly. -ief -aef follow the surface. Returns the node.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `fraction` |  | Also accepted under this command's other name. |
| `count` |  | Also accepted under this command's other name. |
| `spacing` |  | Also accepted under this command's other name. |
| `adjustEdgeFlow` | `aef` | See the command reference. |
| `direction` | `dr` | See the command reference. |
| `divisions` | `div` | See the command reference. |
| `enableProfileCurve` | `epc` | See the command reference. |
| `fixQuads` | `fq` | See the command reference. |
| `insertWithEdgeFlow` | `ief` | See the command reference. |
| `profileCurveInputOffset` | `pio` | See the command reference. |
| `profileCurveInputScale` | `pis` | See the command reference. |
| `profileCurve_FloatValue` | `pfv` | See the command reference. |
| `profileCurve_Interp` | `pi` | See the command reference. |
| `profileCurve_Position` | `pp` | See the command reference. |
| `rootEdge` | `re` | See the command reference. |
| `smoothingAngle` | `sma` | See the command reference. |
| `splitType` | `stp` | See the command reference. |
| `useEqualMultiplier` | `uem` | See the command reference. |
| `useFaceNormalsAtEnds` | `fne` | See the command reference. |
| `weight` | `wt` | See the command reference. |
| `worldSpace` | `ws` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polySplitRings

Subdivide one way: insert loops across every ring through the selected edges, or across the selected faces in one direction, without the perpendicular cuts a full subdivide adds. Returns the new edges.

| Flag | Short | Description |
| --- | --- | --- |
| `count` |  | Loops per ring, evenly spaced. |
| `direction` |  | Faces: across adds lines across the selection's short way, along the long way. |
| `edgeFlow` |  | Curve the new loops to follow the surrounding surface rather than lie flat. |

## polySplitVertex

Give each face at each vertex its own copy of it. Returns the node.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `worldSpace` | `ws` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyStraighten

Straighten the selected edge chains (or chains of selected vertices) into lines between their ends. Several separate chains straighten separately; closed loops are left alone. Vertices with no edges between them line up on their best-fit line.

| Flag | Short | Description |
| --- | --- | --- |
| `spacing` |  | keep (relative distances along the chain), even or project (straight across). |
| `fitLine` |  | Use the line through all of a chain's points instead of its ends. |
| `strength` |  | How far toward the shape, 0 to 1; soft selection fades it further. |

## polyStraightenUVBorder

Straighten a run of border UVs between its ends: -bo blends back to where they were, -cur bows the line, -pl keeps their spacing. Whole borders without maps picked. Returns the node.

Arguments: UVs, faces or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `constructionHistory` | `ch` | Accepted: a node is made while history is on. |
| `name` | `n` | Accepted for existing scripts. |
| `caching` | `cch` | Accepted for existing scripts. |
| `nodeState` | `nds` | Accepted for existing scripts. |
| `blendOriginal` | `bo` | 0 straight, 1 as it was. |
| `curvature` | `c` | How much to bow. |
| `preserveLength` | `pl` | 1 keeps spacing, 0 evens it. |
| `gapTolerance` | `gt` | Accepted for existing scripts. |

## polySubdivide

Catmull-Clark subdivide whole meshes (crease-aware).

Arguments: objects

| Flag | Short | Description |
| --- | --- | --- |
| `levels` |  | Subdivision levels. |
| `keepHardEdges` |  | Keep normals split along hard edges; off smooths the shading across them. |
| `continuity` | `c` | See the command reference. |
| `degree` | `deg` | See the command reference. |
| `divisions` | `dv` | See the command reference. |
| `divisionsPerEdge` | `dpe` | See the command reference. |
| `keepBorder` | `kb` | See the command reference. |
| `keepHardEdge` | `khe` | See the command reference. |
| `keepMapBorders` | `kmb` | See the command reference. |
| `keepSelectionBorder` | `ksb` | See the command reference. |
| `keepTesselation` | `xkt` | See the command reference. |
| `keepTessellation` | `kt` | See the command reference. |
| `method` | `mth` | See the command reference. |
| `osdCreaseMethod` | `ocr` | See the command reference. |
| `osdFvarBoundary` | `ofb` | See the command reference. |
| `osdFvarPropagateCorners` | `ofc` | See the command reference. |
| `osdSmoothTriangles` | `ost` | See the command reference. |
| `osdVertBoundary` | `ovb` | See the command reference. |
| `propagateEdgeHardness` | `peh` | See the command reference. |
| `pushStrength` | `ps` | See the command reference. |
| `roundness` | `ro` | See the command reference. |
| `smoothUVs` | `suv` | See the command reference. |
| `subdivisionLevels` | `sl` | See the command reference. |
| `subdivisionType` | `sdt` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polySubdivideEdge

Add Divisions to edges: -dv new points on each. Returns the node.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `divisions` | `dv` | See the command reference. |
| `size` | `s` | See the command reference. |
| `worldSpace` | `ws` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polySubdivideFacet

Add Divisions to faces: -dv levels, -m 0 quads, 1 triangles. Returns the node.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `divisions` | `dv` | See the command reference. |
| `divisionsU` | `duv` | See the command reference. |
| `divisionsV` | `dvv` | See the command reference. |
| `mode` | `m` | See the command reference. |
| `subdMethod` | `sbm` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polySuperCtx

A polySuperCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `attach` | `a` |  |
| `enable` | `en` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## polySuperShape

Create a SuperShape mesh. Returns [mesh, generator] (generator is None with generator=False).

| Flag | Short | Description |
| --- | --- | --- |
| `name` | `n` | Name of the mesh; made unique. |
| `position` |  | World position. |
| `rotation` |  | Rotation in degrees. |
| `normal` |  | Turn the up axis to this direction, as for placing on a surface; a dome cut by fraction then sits on position by its open base. |
| `generator` |  | Keep the generator node so its parameters stay editable. |
| `object` | `o` | Accepted: the transform and shape are always made. |
| `createUVs` | `cuv` | 0 leaves the mesh without UVs. |
| `texture` | `tx` | How UVs are laid out; accepted. |
| `axis` | `ax` | The direction the primitive's up points, as a vector. |
| `caching` | `cch` | Accepted. |
| `nodeState` | `nds` | Accepted. |
| `constructionHistory` | `ch` | False: no generator node, as generator=False. |
| `radius` | `r` | cm. |
| `horizontalDivisions` | `hd` | Segments around. |
| `verticalDivisions` | `vd` | Segments pole to pole. |
| `ultraM` | `um` | Lobes around. |
| `ultraN1` | `un1` | Shape around: first exponent. |
| `ultraN2` | `un2` | Shape around: second exponent. |
| `ultraN3` | `un3` | Shape around: third exponent. |
| `ultraVM` | `uvm` | Lobes pole to pole. |
| `ultraVN1` | `uvn1` | Shape pole to pole: first exponent. |
| `ultraVN2` | `uvn2` | Shape pole to pole: second exponent. |
| `ultraVN3` | `uvn3` | Shape pole to pole: third exponent. |
| `upAxis` |  | Direction the primitive's up points: x, y, z, -x, -y or -z (or a vector, as the reference's -ax 1 0 0); the scene up axis when left out. |

## polySymmetrize

Symmetrize meshes across an axis: -ma 0 x, 1 y, 2 z, the -d side rebuilt from the other. Runs symmetrize.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `axis` | `a` | 0 x, 1 y, 2 z. |
| `axisDirection` | `ad` | The side kept: 1 positive, 0 negative. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyTargetWeld

Target weld: merge one vertex into another, which keeps its place (or both meet halfway with center=), or an edge into another, end to nearest end. Follows the mesh's symmetry; a weld across the symmetry plane meets on it. Returns the welded vertices.

| Flag | Short | Description |
| --- | --- | --- |
| `mesh` |  | Mesh to weld; default: the selected or hilited mesh. |
| `source` |  | Vertex (or edge) that moves. |
| `target` |  | Vertex (or edge) it merges into. |
| `edge` |  | source and target are edges. |
| `center` |  | Meet halfway instead of at the target. |

## polyToCurve

A curve through a loop of selected edges (the reference's Mesh > Create Curve From Edges): -d the degree (3), -f 0 open or 2 best. Returns [curve].

| Flag | Short | Description |
| --- | --- | --- |
| `degree` | `d` | Degree. |
| `form` | `f` | Accepted for existing scripts. |
| `constructionHistory` | `ch` | Accepted for existing scripts. |
| `name` | `n` | Name of the curve. |

## polyTorus

Create a Torus mesh. Returns [mesh, generator] (generator is None with generator=False).

| Flag | Short | Description |
| --- | --- | --- |
| `name` | `n` | Name of the mesh; made unique. |
| `position` |  | World position. |
| `rotation` |  | Rotation in degrees. |
| `normal` |  | Turn the up axis to this direction, as for placing on a surface; a dome cut by fraction then sits on position by its open base. |
| `generator` |  | Keep the generator node so its parameters stay editable. |
| `object` | `o` | Accepted: the transform and shape are always made. |
| `createUVs` | `cuv` | 0 leaves the mesh without UVs. |
| `texture` | `tx` | How UVs are laid out; accepted. |
| `axis` | `ax` | The direction the primitive's up points, as a vector. |
| `caching` | `cch` | Accepted. |
| `nodeState` | `nds` | Accepted. |
| `constructionHistory` | `ch` | False: no generator node, as generator=False. |
| `radius` | `r` | Ring radius, cm. |
| `sectionRadius` | `sr` | Tube radius, cm. |
| `subdivisionsAxis` | `sa` | Segments around the ring. |
| `subdivisionsHeight` | `sh` | Segments around the tube. |
| `twist` | `tw` | Turns the tube's sections about the ring, degrees. |
| `fraction` |  | Portion of a full turn swept around the up axis: 1 is whole, 0.5 half. |
| `cutCaps` |  | Close the flat ends a partial sweep leaves. |
| `subdivisionsX` | `sx` | Another name for the attribute it sets. |
| `subdivisionsY` | `sy` | Another name for the attribute it sets. |
| `upAxis` |  | Direction the primitive's up points: x, y, z, -x, -y or -z (or a vector, as the reference's -ax 1 0 0); the scene up axis when left out. |

## polyTransfer

Copy from another mesh of the same topology, -ao names it: -uv UVs (on by default), -v vertices, -vc colours. UVs go through a polyTransfer node; vertices and colours change the mesh directly. Returns the node when there is one.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `alternateObject` | `ao` | See the command reference. |
| `uvSets` | `uv` | See the command reference. |
| `vertexColor` | `vc` | See the command reference. |
| `vertices` | `v` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyTriangulate

Triangulate the selected faces (the whole mesh in object mode), deterministically, as Unreal will. Concave and non-planar faces, collinear points and faces touching themselves split without overlaps or slivers, and no diagonal doubles an edge another face already has. With symmetry on, the mirrored faces split the mirrored way. A whole mesh with modifiers gets a triangulate modifier on top instead, so the result is triangulated rather than the cage.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `beauty` |  | Flip n-gon diagonals toward the largest smallest angle (what display and export use). |
| `quadSplit` |  | Quads split along the shorter diagonal (what display, export and MikkTSpace use) or always from the first corner. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyUVCoverage

How much of a UV range each mesh's faces cover, 0 to 1: -ur u0 v0 u1 v1 (0 0 1 1).

Arguments: faces or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `uvRange` | `ur` | The range. |

## polyUVOverlap

Faces whose UVs overlap another's, among those named: -oc; -noc those that don't.

Arguments: faces or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `overlappingComponents` | `oc` | The overlapping faces (the default). |
| `nonOverlappingComponents` | `noc` | The ones that don't overlap. |

## polyUVRectangle

Square a shell's UVs: its border onto its bounding box, the inside evened between. Returns the node.

Arguments: UVs, faces or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `constructionHistory` | `ch` | Accepted: a node is made while history is on. |
| `name` | `n` | Accepted for existing scripts. |
| `caching` | `cch` | Accepted for existing scripts. |
| `nodeState` | `nds` | Accepted for existing scripts. |

## polyUVSet

UV sets: -cr -d -rn -cp with -uvs and -nuv, -cuv -uvs makes one current. -q -auv lists them, -q -cuv names the current one.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `allUVSets` | `auv` | See the command reference. |
| `allUVSetsIndices` | `uvn` | See the command reference. |
| `allUVSetsWithCount` | `awc` | See the command reference. |
| `copy` | `cp` | See the command reference. |
| `create` | `cr` | See the command reference. |
| `currentLastUVSet` | `luv` | See the command reference. |
| `currentPerInstanceUVSet` | `cpi` | See the command reference. |
| `currentUVSet` | `cuv` | See the command reference. |
| `delete` | `d` | See the command reference. |
| `genNewUVSet` | `gen` | See the command reference. |
| `newUVSet` | `nuv` | See the command reference. |
| `perInstance` | `pi` | See the command reference. |
| `projections` | `pr` | See the command reference. |
| `rename` | `rn` | See the command reference. |
| `reorder` | `ro` | See the command reference. |
| `shareInstances` | `si` | See the command reference. |
| `unshared` | `us` | See the command reference. |
| `uvSet` | `uvs` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyUVStackSimilarShells

Stack shells that are alike (same counts, same 3D area within -tol) on the first of each kind, by their middles. Returns the node.

Arguments: UVs, faces or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `constructionHistory` | `ch` | Accepted: a node is made while history is on. |
| `name` | `n` | Accepted for existing scripts. |
| `caching` | `cch` | Accepted for existing scripts. |
| `nodeState` | `nds` | Accepted for existing scripts. |
| `tolerance` | `tol` | How close the areas must be, as a fraction. |
| `onlyMatch` | `om` | Accepted for existing scripts. |

## polyUnite

Combine meshes into one, world positions kept, faces in the order named. The originals stay as the hidden shapes feeding it, so moving them moves the result, unless -ch 0. Returns [transform, node].

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `mergeUVSets` | `muv` | See the command reference. |
| `centerPivot` | `cp` | See the command reference. |
| `objectPivot` | `op` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyUnlockNormals

Drop the stored split normals of the selected meshes, so shading comes from the hard and soft edges again.

## polyWedgeFace

Extrude faces in an arc: -wa degrees, -d divisions, round the edge -ed or the -ax axis through -cen. Returns the node.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `axis` | `ax` | See the command reference. |
| `center` | `cen` | See the command reference. |
| `divisions` | `d` | See the command reference. |
| `edge` | `ed` | See the command reference. |
| `wedgeAngle` | `wa` | See the command reference. |
| `worldSpace` | `ws` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## polyWeightedNormals

Face-weighted normals on the selected faces (whole meshes in object mode): each smooth fan takes the normal of its biggest faces, so chamfered or bevelled low poly shades its large faces flat and only the chamfers bend. Written as locked split normals, which export to FBX. polyUnlockNormals goes back to computed ones.

| Flag | Short | Description |
| --- | --- | --- |
| `mode` | `m` | What weighs a face: area, angle (its corner angle) or areaAngle. |
| `dominance` | `d` | Faces under this fraction of the heaviest face in a fan are left out; 0 keeps all. |
| `keepSharp` | `ks` | Keep hard edges split. |
| `faceStrength` | `fs` | Only the faces with the highest face int faceStrength count, when the mesh has it. |

## popupMenu

Script UI: popupMenu, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `button` | `b` |  |
| `ctrlModifier` | `ctl` |  |
| `altModifier` | `alt` |  |
| `shiftModifier` | `sht` |  |
| `markingMenu` | `mm` |  |
| `allowOptionBoxes` | `aob` |  |
| `deleteAllItems` | `dai` |  |
| `itemArray` | `ia` |  |
| `numberOfItems` | `ni` |  |
| `postMenuCommand` | `pmc` |  |
| `postMenuCommandOnce` | `pmo` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## pose

Keep a character's current values as a pose (-n names it), or apply one (-a -n). -q -ap lists them. Returns the pose's name.

Arguments: the character (the current one when none)

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Return the value of the flags named instead of setting them. |
| `edit` | `e` | Change the flags named. |
| `name` | `n` | The pose. |
| `apply` | `a` | Apply the pose. |
| `allPoses` | `ap` | Query: all poses. |

## poseEditor

Script UI: poseEditor, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `targetControlList` | `tcl` |  |
| `targetList` | `tl` |  |
| `verticalSliders` | `vs` |  |
| `control` | `ctl` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `filter` | `f` |  |
| `forceMainConnection` | `fmc` |  |
| `highlightConnection` | `hlc` |  |
| `lockMainConnection` | `lck` |  |
| `mainListConnection` | `mlc` |  |
| `panel` | `pnl` |  |
| `selectionConnection` | `slc` |  |
| `stateString` | `sts` |  |
| `unParent` | `up` |  |
| `unlockMainConnection` | `ulk` |  |
| `updateMainConnection` | `upd` |  |
| `useTemplate` | `ut` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## posePanel

A panel holding an editor of its kind: the editor has the panel's name.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `poseEditor` | `pe` |  |
| `label` | `l` |  |
| `parent` | `p` |  |
| `control` | `ctl` |  |
| `copy` | `cp` |  |
| `createString` | `cs` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `editString` | `es` |  |
| `exists` | `ex` |  |
| `init` | `in` |  |
| `isUnique` | `iu` |  |
| `menuBarVisible` | `mbv` |  |
| `needsInit` | `ni` |  |
| `popupMenuProcedure` | `pmp` |  |
| `replacePanel` | `rp` |  |
| `tearOff` | `to` |  |
| `tearOffCopy` | `toc` |  |
| `unParent` | `up` |  |
| `useTemplate` | `ut` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## prefixHierarchy

prefixHierarchy(root, prefix): puts prefix in front of the name of root and all under it.

Arguments: root prefix

## progressBar

Script UI: progressBar, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `progress` | `pr` |  |
| `minValue` | `min` |  |
| `maxValue` | `max` |  |
| `step` | `s` |  |
| `status` | `st` |  |
| `isInterruptable` | `ii` |  |
| `isCancelled` | `ic` |  |
| `isMainProgressBar` | `imp` |  |
| `beginProgress` | `bp` |  |
| `endProgress` | `ep` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## progressWindow

A progress window: -t -pr -st -min -max -ii, -e to update, -q -ic asks whether it was cancelled, -ep ends it.

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` |  |
| `edit` | `e` |  |
| `title` | `t` |  |
| `progress` | `pr` |  |
| `status` | `st` |  |
| `minValue` | `min` |  |
| `maxValue` | `max` |  |
| `step` | `s` |  |
| `isInterruptable` | `ii` |  |
| `isCancelled` | `ic` |  |
| `endProgress` | `ep` |  |

## projectCurve

A curve's shadow on a surface as a curve on it: -d the way (0 0 1), -un along the normal. Returns [surface->curve, node].

Arguments: a curve and a surface

| Flag | Short | Description |
| --- | --- | --- |
| `direction` | `d` | Which way. |
| `useNormal` | `un` | Along the surface's normal. |
| `curveSamples` | `cs` | How many samples (50). |
| `constructionHistory` | `ch` | Keep the node (on by default). |
| `name` | `n` | The new object's name. |
| `object` | `o` | Accepted for existing scripts. |
| `range` | `rn` | Accepted for existing scripts. |
| `caching` | `cch` | Accepted for existing scripts. |
| `nodeState` | `nds` | Accepted for existing scripts. |
| `tolerance` | `tol` | Accepted for existing scripts. |
| `polygon` | `po` | 0 NURBS, 1 polygons. |

## projectTangent

Turn a curve's end to follow what it meets: another curve or a surface. -ts scale, -ro rotate, -rt reverse, -td 1 U or 2 V. Returns [curve, node].

Arguments: a curve and what to follow

| Flag | Short | Description |
| --- | --- | --- |
| `tangentScale` | `tsc` | How far the next CV goes (1). |
| `rotate` | `ro` | Degrees. |
| `reverseTangent` | `rt` | The other way. |
| `tangentDirection` | `td` | 1 U or 2 V. |
| `curvature` | `cvt` | Accepted for existing scripts. |
| `ignoreEdges` | `ie` | Accepted for existing scripts. |
| `constructionHistory` | `ch` | Keep a history node (on by default). |
| `replaceOriginal` | `rpo` | Change the curve itself (on by default). |
| `name` | `n` | Name of a new curve. |
| `object` | `o` | Make an object (accepted for existing scripts). |

## promptDialog

A modal question with a text field. Returns the button pressed; -q -tx gives the text.

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` |  |
| `title` | `t` |  |
| `message` | `m` |  |
| `messageAlign` | `ma` |  |
| `text` | `tx` |  |
| `button` | `b` |  |
| `defaultButton` | `db` |  |
| `cancelButton` | `cb` |  |
| `dismissString` | `ds` |  |
| `style` | `st` |  |
| `scrollableField` | `sf` |  |
| `backgroundColor` | `bgc` |  |
| `parent` | `p` |  |

## proximityWrap

Proximity Wrap: points follow the nearest points of driver meshes. proximityWrap -drivers d obj; -e -addDrivers d; -q -drivers. Returns [proximityWrap1].

Arguments: what to deform, and any target last (the selection when none); the deformer with -e and -q

| Flag | Short | Description |
| --- | --- | --- |
| `name` | `n` | Name of the deformer. |
| `before` | `bf` | Go just above the deformer nearest the mesh. |
| `after` | `af` | Go nearest the mesh, after the other deformers (the default). |
| `frontOfChain` | `foc` | Go above every other deformer. |
| `envelope` | `en` | How much of the deformation applies (1). |
| `edit` | `e` | Edit mode. |
| `query` | `q` | Query mode. |
| `drivers` | `drv` | The driver meshes. Query: them. |
| `addDrivers` | `ad` | With -e: a driver to add. |
| `wrapMode` | `wm` | 0 offset, 1 surface, 2 snap. |
| `maxDrivers` | `md` | How many driver points each point follows (4). |
| `falloffScale` | `fsc` | How far each binding reaches (1). |
| `dropoffRateScale` | `drs` | How fast a binding's weight falls with distance (1). |

## putenv

Sets an environment variable for this process.

Arguments: name value

## pwd

The current working directory.

## radial

Make a radial field acting on the selected bodies, or -q/-e one. Returns [field].

Arguments: bodies to act on

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query a value. |
| `edit` | `e` | Edit an existing node. |
| `name` | `n` | Name of the new node. |
| `magnitude` | `m` | Strength. |
| `attenuation` | `att` | How fast it weakens with distance. |
| `maxDistance` | `mxd` | How far it reaches (turns the limit on). |
| `useMaxDistance` | `umd` | Limit its reach. |
| `directionX` | `dx` | Direction X. |
| `directionY` | `dy` | Direction Y. |
| `directionZ` | `dz` | Direction Z. |
| `speed` | `s` | Air: wind speed. |
| `inheritVelocity` | `iv` | Air: kept motion. |
| `frequency` | `f` | Turbulence: noise frequency. |
| `minDistance` | `mnd` | Newton: least distance. |
| `position` | `pos` | Where it is. |

## radioButton

Script UI: radioButton, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `label` | `l` |  |
| `select` | `sl` |  |
| `collection` | `cl` |  |
| `changeCommand` | `cc` |  |
| `onCommand` | `onc` |  |
| `offCommand` | `ofc` |  |
| `align` | `al` |  |
| `editable` | `ed` |  |
| `data` | `da` |  |
| `recomputeSize` | `rs` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## radioButtonGrp

Script UI: radioButtonGrp, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `label` | `l` |  |
| `numberOfRadioButtons` | `nrb` |  |
| `label1` | `l1` |  |
| `label2` | `l2` |  |
| `label3` | `l3` |  |
| `label4` | `l4` |  |
| `labelArray2` | `la2` |  |
| `labelArray3` | `la3` |  |
| `labelArray4` | `la4` |  |
| `select` | `sl` |  |
| `shareCollection` | `scl` |  |
| `changeCommand` | `cc` |  |
| `changeCommand1` | `cc1` |  |
| `changeCommand2` | `cc2` |  |
| `changeCommand3` | `cc3` |  |
| `changeCommand4` | `cc4` |  |
| `onCommand` | `onc` |  |
| `onCommand1` | `on1` |  |
| `onCommand2` | `on2` |  |
| `onCommand3` | `on3` |  |
| `onCommand4` | `on4` |  |
| `offCommand` | `ofc` |  |
| `offCommand1` | `of1` |  |
| `offCommand2` | `of2` |  |
| `offCommand3` | `of3` |  |
| `offCommand4` | `of4` |  |
| `enable1` | `en1` |  |
| `enable2` | `en2` |  |
| `enable3` | `en3` |  |
| `enable4` | `en4` |  |
| `data1` | `da1` |  |
| `data2` | `da2` |  |
| `data3` | `da3` |  |
| `data4` | `da4` |  |
| `editable` | `ed` |  |
| `vertical` | `vr` |  |
| `columnWidth1` | `cw1` |  |
| `columnAlign1` | `cl1` |  |
| `columnAttach1` | `ct1` |  |
| `columnOffset1` | `co1` |  |
| `columnWidth2` | `cw2` |  |
| `columnAlign2` | `cl2` |  |
| `columnAttach2` | `ct2` |  |
| `columnOffset2` | `co2` |  |
| `adjustableColumn2` | `ad2` |  |
| `columnWidth3` | `cw3` |  |
| `columnAlign3` | `cl3` |  |
| `columnAttach3` | `ct3` |  |
| `columnOffset3` | `co3` |  |
| `adjustableColumn3` | `ad3` |  |
| `columnWidth4` | `cw4` |  |
| `columnAlign4` | `cl4` |  |
| `columnAttach4` | `ct4` |  |
| `columnOffset4` | `co4` |  |
| `adjustableColumn4` | `ad4` |  |
| `columnWidth5` | `cw5` |  |
| `columnAlign5` | `cl5` |  |
| `columnAttach5` | `ct5` |  |
| `columnOffset5` | `co5` |  |
| `adjustableColumn5` | `ad5` |  |
| `columnWidth6` | `cw6` |  |
| `columnAlign6` | `cl6` |  |
| `columnAttach6` | `ct6` |  |
| `columnOffset6` | `co6` |  |
| `adjustableColumn6` | `ad6` |  |
| `adjustableColumn` | `adj` |  |
| `columnWidth` | `cw` |  |
| `columnAlign` | `cal` |  |
| `columnAttach` | `cat` |  |
| `rowAttach` | `rat` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## radioCollection

A set of radio items, one selected.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `select` | `sl` |  |
| `collectionItemArray` | `cia` |  |
| `numberOfCollectionItems` | `nci` |  |
| `global` | `gl` |  |
| `parent` | `p` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `defineTemplate` | `dt` |  |
| `useTemplate` | `ut` |  |

## radioMenuItemCollection

A set of radio items, one selected.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `select` | `sl` |  |
| `collectionItemArray` | `cia` |  |
| `numberOfCollectionItems` | `nci` |  |
| `global` | `gl` |  |
| `parent` | `p` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `defineTemplate` | `dt` |  |
| `useTemplate` | `ut` |  |

## rangeControl

Script UI: rangeControl, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `minRange` | `min` |  |
| `maxRange` | `max` |  |
| `changedCommand` | `cc` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## rebuildCurve

Rebuild with even spans: -s spans (0 keeps the count), -d degree, -kcp keeps the CVs. Returns [curve, node].

Arguments: curves (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `spans` | `s` | Spans. |
| `degree` | `d` | Degree. |
| `keepControlPoints` | `kcp` | Keep the CVs. |
| `rebuildType` | `rt` | Accepted for existing scripts (uniform). |
| `endKnots` | `end` | Accepted for existing scripts. |
| `keepRange` | `kr` | Accepted for existing scripts. |
| `keepEndPoints` | `kep` | Accepted for existing scripts. |
| `keepTangents` | `kt` | Accepted for existing scripts. |
| `tolerance` | `tol` | Accepted for existing scripts. |
| `constructionHistory` | `ch` | Keep a history node (on by default). |
| `replaceOriginal` | `rpo` | Change the curve itself (on by default). |
| `name` | `n` | Name of a new curve. |
| `object` | `o` | Make an object (accepted for existing scripts). |

## rebuildSurface

Rebuild with even spans: -su -sv spans (0 keeps the count), -du -dv degrees, -dir 0 U, 1 V, 2 both, -kr range. Returns [surface, node].

Arguments: surfaces (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `spansU` | `su` | Spans along U (4). |
| `spansV` | `sv` | Spans along V (4). |
| `degreeU` | `du` | Degree along U (3). |
| `degreeV` | `dv` | Degree along V (3). |
| `keepRange` | `kr` | 0 parameters 0 to 1, 1 as they were, 2 0 to the spans (1). |
| `direction` | `dir` | 0 U, 1 V, 2 both (2). |
| `keepControlPoints` | `kcp` | Leave the CVs as they are. |
| `rebuildType` | `rt` | Accepted for existing scripts (uniform). |
| `endKnots` | `end` | Accepted for existing scripts. |
| `keepCorners` | `kc` | Accepted for existing scripts. |
| `fitRebuild` | `fr` | Accepted for existing scripts. |
| `tolerance` | `tol` | Accepted for existing scripts. |
| `constructionHistory` | `ch` | Keep a history node (on by default). |
| `replaceOriginal` | `rpo` | Change the surface itself (on by default). |
| `name` | `n` | Name of a new surface. |
| `object` | `o` | Accepted for existing scripts. |
| `polygon` | `po` | Accepted for existing scripts. |

## recordAttr

Mark attributes for recording, kept for scripts: -at names them, all keyable ones when none; -d takes them off; -q lists them.

Arguments: objects or plugs

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `attribute` | `at` | The attributes. |
| `delete` | `d` | Take them off. |

## redo

Redo the last undone command.

## reference

File references: -f path -ns namespace brings one in; -ur, -lr, -rr unload, load and remove; -q -f lists the files.

Arguments: file

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `filename` | `f` | The file to reference. |
| `reference` | `r` | With -q: the referenced files. |
| `namespace` | `ns` | The namespace to put it under. |
| `referenceNode` | `rfn` | The reference node's name. |
| `deferReference` | `dr` | Add it unloaded. |
| `loadReference` | `lr` | Load this one. |
| `unloadReference` | `ur` | Unload this one. |
| `removeReference` | `rr` | Remove this one. |

## referenceEdit

Change a reference's kept edits: -r removes them (unloaded), -cet retargets them, -afe tries the failed ones again.

Arguments: reference node, node or plug

| Flag | Short | Description |
| --- | --- | --- |
| `removeEdits` | `r` | Remove the edits on this target, from an unloaded reference. |
| `changeEditTarget` | `cet` | Point edits on one target at another: old, new. |
| `applyFailedEdits` | `afe` | Try the edits that failed again. |
| `editCommand` | `ec` | Only edits of these commands. |
| `failedEdits` | `fld` | Act on failed edits; the default. |
| `successfulEdits` | `scs` | Act on successful edits. |
| `onReferenceNode` | `orn` | Only edits stored on this reference node. |

## referenceQuery

Ask about references, given a node, a reference node or a file: -inr isNodeReferenced, -f the file, -n nodes, -ns namespace, -rfn reference node, -il loaded, -es its edits.

Arguments: node, reference node or file

| Flag | Short | Description |
| --- | --- | --- |
| `isNodeReferenced` | `inr` | Whether the node comes from a reference. |
| `filename` | `f` | The file. |
| `nodes` | `n` | The nodes it brought in. |
| `namespace` | `ns` | Its namespace, from the root. |
| `parentNamespace` | `pns` | The namespace above its own. |
| `referenceNode` | `rfn` | Its reference node. |
| `isLoaded` | `il` | Whether it's loaded. |
| `isExportEdits` | `iee` | Whether it's an edits file. None are made here. |
| `isPreviewOnly` | `ipo` | Whether it's only a preview. None are here. |
| `shortName` | `shn` | The file's name without its folder, or the namespace's last part. |
| `withoutCopyNumber` | `wcn` | The file without its {n}. |
| `unresolvedName` | `un` | The file as written. |
| `dagPath` | `dp` | Full paths from -n. |
| `parent` | `p` | With -rfn or -f, the reference above it. |
| `child` | `ch` | With -rfn or -f, those directly below it. |
| `topReference` | `tr` | With -rfn or -f, the top one above it. |
| `editStrings` | `es` | Its edits, as commands. |
| `editNodes` | `en` | The nodes its edits touch. |
| `editAttrs` | `ea` | The attributes its edits touch. |
| `editCommand` | `ec` | Only edits of these commands. |
| `failedEdits` | `fld` | Include edits that failed on the last load. |
| `successfulEdits` | `scs` | Include edits that worked; the default. |
| `onReferenceNode` | `orn` | Only edits stored on this reference node. |
| `liveEdits` | `le` | Accepted for scripts. |
| `showDagPath` | `sdp` | Accepted for scripts; edits show full paths. |
| `showFullPath` | `sfp` | Accepted for scripts; edits show full paths. |
| `showNamespace` | `sns` | Accepted for scripts; edits show namespaces. |

## refresh

Brings the scene up to date; the viewports draw on their own.

| Flag | Short | Description |
| --- | --- | --- |
| `force` | `f` | Accepted for existing scripts. |
| `suspend` | `su` | Accepted for existing scripts. |
| `currentView` | `cv` | Accepted for existing scripts. |

## relationship

Held for scripts: relationship type owner members, -b breaks it, -rd its data; -q type owner lists the members. The reference makes the connections on reading a file; this only holds the list, and it is not saved.

Arguments: the type, the owner, then what is related

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `b` | `b` | Break it. |
| `relationshipData` | `rd` | Data for it. |

## removeFromDisplayLayer

Take objects (the selection by default) out of their display layers.

Arguments: objects

## removeFromSet

Take members (the selection by default) out of a set. An object takes its components with it. Returns the set's members.

Arguments: set, members

## removeJoint

Remove a joint from its skeleton. Its children stay where they are, under its parent.

Arguments: joint

## removeModifier

Remove modifiers, reconnecting the stack around them.

Arguments: modifiers

## removeMultiInstance

Remove array elements with their connections and values: 'node.attr[3]', or 'node.group[3]' for every child of a grouped array.

Arguments: plugs

## removeUnusedInfluences

Takes influences holding no weight off a skin. Returns those taken off.

Arguments: skinned meshes or skinClusters (the selection when none)

## rename

rename(old, new) or rename(new) for the single selected object. Returns the renamed node's path.

Arguments: [object] newName

| Flag | Short | Description |
| --- | --- | --- |
| `ignoreShape` | `is` | Leave the shape's name alone. |
| `uuid` | `uid` | The first argument is a uuid. |

## renameAttr

renameAttr('node.attr', 'newName'): renames an attribute added with addAttr.

Arguments: plug newName

## render

Renders the current frame and returns the image's path.

Arguments: camera

| Flag | Short | Description |
| --- | --- | --- |
| `camera` | `cam` | Camera to render through. |
| `width` | `w` | Image width. |
| `height` | `h` | Image height. |
| `region` | `reg` | x0 y0 x1 y1, pixels from the top left. |
| `batch` | `b` | Render the frames the render settings name. |

## renderSettings

Output file names the render settings give.

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `firstImageName` | `fin` | The first frame's file name. |
| `lastImageName` | `lin` | The last frame's file name. |
| `imageGenericName` | `ign` | The file name with # for the frame. |
| `fullPath` | `fp` | With the images folder. |
| `fullPathTemp` | `fpt` | The folder of images the render view keeps. |
| `camera` | `cam` | For this camera. |
| `layer` | `lyr` | For this render layer. |

## renderWindowEditor

The render view's images: write, keep, remove, exposure and gamma.

Arguments: editor

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `writeImage` | `wi` | Write the current image to a file. |
| `snapShot` | `snp` | Keep the current image. |
| `removeAllImages` | `ra` | Forget kept images. |
| `removeImage` | `ri` | Forget one kept image. |
| `nbImages` | `nim` | How many images are kept. |
| `exposure` | `exp` | Display exposure, stops. |
| `gamma` | `ga` | Display gamma. |
| `currentCamera` | `crc` | The camera rendered through. |

## reorder

Moves objects among their siblings: -f front, -b back, -r n places.

Arguments: objects

| Flag | Short | Description |
| --- | --- | --- |
| `front` | `f` | First among its siblings. |
| `back` | `b` | Last among its siblings. |
| `relative` | `r` | This many places along. |

## reorderDeformers

Swap two deformers' places in a mesh's history: reorderDeformers cluster1 ffd1 pCube1. Without geometry, on every mesh both are on.

Arguments: two deformers, then optionally the geometry

## reorderDisplayLayer

Move a display layer to a row of the Display Layers panel (0 is the top).

Arguments: layer

| Flag | Short | Description |
| --- | --- | --- |
| `index` | `i` | Row it moves to. |

## reroot

Make a joint the root of its skeleton. Every joint stays where it is.

Arguments: joint

## resetPivot

Put the pivots back at the object's origin, keeping the object where it is.

Arguments: objects

## resetTool

Put a tool context back to its default settings.

Arguments: [context]

## resetTransforms

Put translate, rotate and scale back to their defaults (the object moves).

Arguments: objects

## reverseCurve

Reverse the direction. Returns [curve, node].

Arguments: curves (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `constructionHistory` | `ch` | Keep a history node (on by default). |
| `replaceOriginal` | `rpo` | Change the curve itself (on by default). |
| `name` | `n` | Name of a new curve. |
| `object` | `o` | Make an object (accepted for existing scripts). |

## reverseSurface

Reverse the direction: -d 0 U, 1 V, 2 both, 3 swap U and V. Returns [surface, node].

Arguments: surfaces (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `direction` | `d` | 0 U, 1 V, 2 both, 3 swap (0). |
| `constructionHistory` | `ch` | Keep a history node (on by default). |
| `replaceOriginal` | `rpo` | Change the surface itself (on by default). |
| `name` | `n` | Name of a new surface. |
| `object` | `o` | Accepted for existing scripts. |
| `polygon` | `po` | Accepted for existing scripts. |

## revolve

Turn a curve about an axis: revolve curve1 -ax 0 1 0 -p 0 0 0 -s 8 -ssw 0 -esw 360. Returns [revolvedSurface1, revolve1].

Arguments: a curve (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `axis` | `ax` | The axis. |
| `pivot` | `p` | A point on the axis. |
| `sections` | `s` | Sections (8). |
| `divisions` | `dv` | Mesh points per span of the curve. |
| `startSweep` | `ssw` | Start, degrees. |
| `endSweep` | `esw` | End, degrees. |
| `degree` | `d` | Degree around (3). |
| `useTolerance` | `ut` | Accepted for existing scripts. |
| `tolerance` | `tol` | Accepted for existing scripts. |
| `polygon` | `po` | 0 NURBS (the default), 1 polygons. |
| `constructionHistory` | `ch` | Keep the node (on by default). |
| `range` | `rn` | Accepted for existing scripts. |
| `name` | `n` | Name of the new object. |

## rigidBind

Rigid bind: each point follows one joint, a skinCluster of 1.0 weights. -tsb only the bones named, -ub unbinds, -ubk keeps history. Returns 1.

Arguments: skeleton and geometry

| Flag | Short | Description |
| --- | --- | --- |
| `byClosestPoint` | `bcp` | Accepted: closest point is how it binds. |
| `byPartition` | `bp` | Accepted for existing scripts. |
| `colorJoints` | `cj` | Accepted for existing scripts. |
| `delete` | `del` | Remove the skin. |
| `doNotDescend` | `dnd` | Only the joints named. |
| `enable` | `en` | Accepted for existing scripts. |
| `name` | `n` | The skinCluster's name. |
| `partition` | `p` | Accepted for existing scripts. |
| `toAll` | `ta` | The whole skeleton (the default). |
| `toSelectedBones` | `tsb` | Only the joints named. |
| `unbind` | `ub` | Remove the skin. |
| `unbindKeepHistory` | `ubk` | Stop skinning, keep the node. |

## rigidBody

Make the selected objects rigid bodies: active ones are moved by the solver, passive ones only push. -q/-e read and change one.

Arguments: objects

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query a value. |
| `edit` | `e` | Edit an existing node. |
| `name` | `n` | Name of the new node. |
| `active` | `act` | Moved by the solver (the default). |
| `passive` | `pas` | Only pushes others. |
| `mass` | `m` | Mass. |
| `bounciness` | `b` | Kept normal speed in contacts. |
| `staticFriction` | `sf` | Friction at rest. |
| `dynamicFriction` | `df` | Friction sliding. |
| `standInObject` | `si` | 0 its mesh, 1 a box, 2 a sphere, 3 a capsule. |
| `particleResolution` | `prs` | Particles across the longest side. |
| `initialVelocity` | `iv` | Velocity at the start. |
| `initialSpin` | `is` | Spin at the start, degrees per second. |
| `solver` | `sv` | The solver to join. |

## roll

Roll a camera about its view: -d degrees; -abs sets the roll.

Arguments: [camera]

| Flag | Short | Description |
| --- | --- | --- |
| `degree` | `d` |  |
| `absolute` | `abs` |  |
| `relative` | `rel` |  |

## rollCtx

A rollCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `rollScale` | `rs` |  |
| `alternateContext` | `ac` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## rotate

Rotate objects about their pivots. rotate(by=(x, y, z)) adds a rotation (degrees, XYZ order) in `space`; rotate(to=(x, y, z)) sets the local Euler angles. rotate(x=90) sets one local angle and leaves the others. With components selected it turns them that far about the world axis through their centre instead.

Arguments: objects

| Flag | Short | Description |
| --- | --- | --- |
| `by` |  | Relative rotation, degrees. |
| `to` |  | Absolute local rotation, degrees. |
| `space` |  | 'world', 'object' or 'parent' for `by`. |
| `relative` | `r` | The values add to what is there. |
| `absolute` | `a` | The values replace what is there (the default). |
| `worldSpace` | `ws` | World axes (the default). |
| `objectSpace` | `os` | The object's own axes. |
| `localSpace` | `ls` | The parent's axes. |
| `parent` | `pa` | The parent's axes. |
| `worldSpaceDistance` | `wd` | Accepted: distances are in world units already. |
| `preserveChildPosition` | `pcp` | Children stay where they are in the world. |
| `preserveGeometryPosition` | `pgp` | Accepted for existing scripts. |
| `rotatePivotRelative` | `rpr` | Accepted for existing scripts. |
| `scalePivotRelative` | `spr` | Accepted for existing scripts. |
| `deletePriorHistory` | `dph` | Accepted for existing scripts. |
| `xformConstraint` | `xc` | Accepted for existing scripts. |
| `constrainAlongNormal` | `xn` | Accepted for existing scripts. |
| `orientJoint` | `oj` | Accepted for existing scripts. |
| `secondaryAxisOrient` | `sao` | Accepted for existing scripts. |
| `componentSpace` | `cs` | Accepted for existing scripts. |
| `x` |  | This axis: a value, or on to read the positional value. |
| `y` |  | This axis. |
| `z` |  | This axis. |
| `rotateX` |  | The axes the values are for. |
| `rotateY` |  | The axes the values are for. |
| `rotateZ` |  | The axes the values are for. |
| `rotateXY` | `xy` | The axes the values are for. |
| `rotateXZ` | `xz` | The axes the values are for. |
| `rotateYZ` | `yz` | The axes the values are for. |
| `rotateXYZ` | `xyz` | The axes the values are for. |
| `pivot` | `p` | The world point to turn about. |
| `euler` | `eu` | Accepted for existing scripts. |
| `forceOrderXYZ` | `fo` | Accepted for existing scripts. |
| `objectCenterPivot` | `ocp` | Accepted for existing scripts. |

## rotateComponents

Rotate the selected components about a world pivot (default: their centre; with symmetry, a pair clear of the mirror plane turns about each side's own centre). Degrees, XYZ order.

| Flag | Short | Description |
| --- | --- | --- |
| `by` |  | Rotation, degrees. |
| `pivot` |  | World pivot. |

## rotationInterpolation

How a rotate's three curves interpolate: -c none (independent), euler (keys kept together), quaternionSlerp (constant speed along the shortest arc), quaternionSquad or quaternion (cubic through the keys). -q returns each curve's mode.

Arguments: curves, objects or rotate plugs (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Return the value of the flags named instead of setting them. |
| `edit` | `e` | Change the flags named. |
| `attribute` | `at` | Channel name(s). |
| `time` | `t` | A frame or range "10:20". |
| `float` | `f` | A range of driver values on driven key curves, as -t is for time. |
| `index` | `in` | Key index(es) on each curve, or a range "0:2". |
| `includeUpperBound` | `iub` | Whether a range's end counts (on by default). |
| `hierarchy` | `hi` | none, below, above or both. |
| `controlPoints` | `cp` | Accepted for existing scripts: control points have no channels of their own here. |
| `shape` | `s` | Include the channels of the objects' shapes. |
| `animation` | `an` | objects (the default), keys or keysOrObjects: keys means the selected keys. |
| `convert` | `c` | The interpolation to convert to. |

## roundConstantRadius

Constant radius fillets where surface edges meet: edges in pairs, surface.u[x] surface.v[y], -rad the radius (one for all, or one per pair; 1 by default). Returns the fillets, then the nodes. Less than the reference: it makes the fillets but does not trim the surfaces or build corners, and -a, -sa, -sb and -s (compound edges) fail.

Arguments: edges in pairs

| Flag | Short | Description |
| --- | --- | --- |
| `constructionHistory` | `ch` | Keep the node (on by default). |
| `name` | `n` | The new object's name. |
| `object` | `o` | Accepted for existing scripts. |
| `range` | `rn` | Accepted for existing scripts. |
| `caching` | `cch` | Accepted for existing scripts. |
| `nodeState` | `nds` | Accepted for existing scripts. |
| `tolerance` | `tol` | Accepted for existing scripts. |
| `polygon` | `po` | 0 NURBS, 1 polygons. |
| `append` | `a` | Not supported: fails. |
| `radiuss` | `rad` | The radius, one or one per pair. |
| `side` | `s` | Not supported: fails. |
| `sidea` | `sa` | Not supported: fails. |
| `sideb` | `sb` | Not supported: fails. |

## rowColumnLayout

Script UI: rowColumnLayout, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `numberOfColumns` | `nc` |  |
| `numberOfRows` | `nr` |  |
| `numberOfRowsColumns` | `nrc` |  |
| `adjustableColumn` | `adj` |  |
| `adjustableRow` | `ar` |  |
| `columnAlign` | `cal` |  |
| `columnAttach` | `cat` |  |
| `columnOffset` | `co` |  |
| `columnSpacing` | `cs` |  |
| `columnWidth` | `cw` |  |
| `rowAlign` | `ral` |  |
| `rowAttach` | `rat` |  |
| `rowHeight` | `rh` |  |
| `rowOffset` | `ro` |  |
| `rowSpacing` | `rs` |  |
| `childArray` | `ca` |  |
| `numberOfChildren` | `nch` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## rowLayout

Script UI: rowLayout, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `numberOfColumns` | `nc` |  |
| `adjustableColumn` | `adj` |  |
| `rowAttach` | `rat` |  |
| `columnAlign` | `cal` |  |
| `columnAttach` | `cat` |  |
| `columnWidth` | `cw` |  |
| `columnOffset` | `co` |  |
| `columnWidth1` | `cw1` |  |
| `columnAlign1` | `cl1` |  |
| `columnAttach1` | `ct1` |  |
| `columnOffset1` | `co1` |  |
| `columnWidth2` | `cw2` |  |
| `columnAlign2` | `cl2` |  |
| `columnAttach2` | `ct2` |  |
| `columnOffset2` | `co2` |  |
| `adjustableColumn2` | `ad2` |  |
| `columnWidth3` | `cw3` |  |
| `columnAlign3` | `cl3` |  |
| `columnAttach3` | `ct3` |  |
| `columnOffset3` | `co3` |  |
| `adjustableColumn3` | `ad3` |  |
| `columnWidth4` | `cw4` |  |
| `columnAlign4` | `cl4` |  |
| `columnAttach4` | `ct4` |  |
| `columnOffset4` | `co4` |  |
| `adjustableColumn4` | `ad4` |  |
| `columnWidth5` | `cw5` |  |
| `columnAlign5` | `cl5` |  |
| `columnAttach5` | `ct5` |  |
| `columnOffset5` | `co5` |  |
| `adjustableColumn5` | `ad5` |  |
| `columnWidth6` | `cw6` |  |
| `columnAlign6` | `cl6` |  |
| `columnAttach6` | `ct6` |  |
| `columnOffset6` | `co6` |  |
| `adjustableColumn6` | `ad6` |  |
| `childArray` | `ca` |  |
| `numberOfChildren` | `nch` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## runTimeCommand

Create a named command that runs a script: -c the script, -ann its annotation, -cat its category, -cl mel or python.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query. |
| `edit` | `e` | Edit. |
| `exists` | `ex` | Whether the command exists. |
| `delete` | `del` | Delete it. |
| `command` | `c` | The script. |
| `annotation` | `ann` | What it does. |
| `category` | `cat` | Its category in the Hotkey Editor. |
| `commandLanguage` | `cl` | mel or python. |
| `default` | `d` | A factory command. |

## scale

Scale objects along their own axes. scale(by=(x, y, z)) multiplies; scale(to=(x, y, z)) sets. scale(x=2) sets one local scale and leaves the others. With components selected it scales them by that factor along the world axis about their centre instead, so scale(y=0) flattens them.

Arguments: objects

| Flag | Short | Description |
| --- | --- | --- |
| `by` |  | Factor per axis. |
| `to` |  | Absolute scale. |
| `relative` | `r` | The values add to what is there. |
| `absolute` | `a` | The values replace what is there (the default). |
| `worldSpace` | `ws` | World axes (the default). |
| `objectSpace` | `os` | The object's own axes. |
| `localSpace` | `ls` | The parent's axes. |
| `parent` | `pa` | The parent's axes. |
| `worldSpaceDistance` | `wd` | Accepted: distances are in world units already. |
| `preserveChildPosition` | `pcp` | Children stay where they are in the world. |
| `preserveGeometryPosition` | `pgp` | Accepted for existing scripts. |
| `rotatePivotRelative` | `rpr` | Accepted for existing scripts. |
| `scalePivotRelative` | `spr` | Accepted for existing scripts. |
| `deletePriorHistory` | `dph` | Accepted for existing scripts. |
| `xformConstraint` | `xc` | Accepted for existing scripts. |
| `constrainAlongNormal` | `xn` | Accepted for existing scripts. |
| `orientJoint` | `oj` | Accepted for existing scripts. |
| `secondaryAxisOrient` | `sao` | Accepted for existing scripts. |
| `componentSpace` | `cs` | Accepted for existing scripts. |
| `x` |  | This axis: a value, or on to read the positional value. |
| `y` |  | This axis. |
| `z` |  | This axis. |
| `scaleX` |  | The axes the values are for. |
| `scaleY` |  | The axes the values are for. |
| `scaleZ` |  | The axes the values are for. |
| `scaleXY` | `xy` | The axes the values are for. |
| `scaleXZ` | `xz` | The axes the values are for. |
| `scaleYZ` | `yz` | The axes the values are for. |
| `scaleXYZ` | `xyz` | The axes the values are for. |
| `pivot` | `p` | The world point to scale about. |
| `distanceOnly` | `dso` | Accepted for existing scripts. |
| `objectCenterPivot` | `ocp` | Accepted for existing scripts. |

## scaleComponents

Scale the selected components about a world pivot (default: their centre; with symmetry, a pair clear of the mirror plane scales about each side's own centre).

| Flag | Short | Description |
| --- | --- | --- |
| `by` |  | Factor per axis: the world axes, or the axes orient= turns them to. |
| `pivot` |  | World pivot. |
| `orient` |  | Rotation of the scale axes from the world axes, degrees, XYZ order. |

## scaleConstraint

Constrain the object named (or selected) last to the targets before it; a second call adds targets. -mo keeps the current offset; -w sets the targets' weight; -e and -q work on the constraint or the object it drives. Returns [constraint].

Arguments: targets, then the object to drive or the constraint

| Flag | Short | Description |
| --- | --- | --- |
| `maintainOffset` | `mo` | Keep the object where it is now relative to the targets. |
| `weight` | `w` | Weight of the targets named (1). |
| `name` | `n` | Name of the constraint. |
| `offset` | `o` | Added to the result. |
| `remove` | `rm` | Remove the targets named. Removing the last deletes the constraint. |
| `targetList` | `tl` | Query: the targets. |
| `weightAliasList` | `wal` | Query: the targets' weight attributes. |
| `aimVector` | `aim` | The local axis that points at the target. |
| `upVector` | `u` | The local axis that points up. |
| `worldUpVector` | `wu` | The world direction up means. |
| `worldUpType` | `wut` | scene, object, objectrotation, vector or none. |
| `worldUpObject` | `wuo` | The object for worldUpType object and objectrotation. |
| `interpType` | `int` | How rotations blend: noflip, average, shortest, longest, cache, or 0 to 4. |
| `skipTranslate` | `st` | Translate axes to leave alone: x, y, z or none. May be used more than once. |
| `skipRotate` | `sr` | Rotate axes to leave alone: x, y, z or none. May be used more than once. |
| `skip` | `sk` | Axes of this constraint's channel to leave alone: x, y, z or none. May be used more than once. |
| `skipScale` |  | Scale axes to leave alone: x, y, z or none. |
| `edit` | `e` | Change the constraint. |
| `query` | `q` | Ask about the constraint. |

## scaleKey

Scale keys in time about -tp (-ts) and in value about -vp (-vs). -t limits them. Returns how many.

Arguments: objects, plugs or curves (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `attribute` | `at` | Channel name(s). |
| `time` | `t` | A frame or range "10:20". |
| `float` | `f` | A range of driver values on driven key curves, as -t is for time. |
| `index` | `in` | Key index(es) on each curve, or a range "0:2". |
| `includeUpperBound` | `iub` | Whether a range's end counts (on by default). |
| `hierarchy` | `hi` | none, below, above or both. |
| `controlPoints` | `cp` | Accepted for existing scripts: control points have no channels of their own here. |
| `shape` | `s` | Include the channels of the objects' shapes. |
| `animation` | `an` | objects (the default), keys or keysOrObjects: keys means the selected keys. |
| `selected` | `sl` | Scale the keys selectKey selected. |
| `timeScale` | `ts` | Time scale. |
| `timePivot` | `tp` | Time pivot. |
| `valueScale` | `vs` | Value scale. |
| `valuePivot` | `vp` | Value pivot. |

## scaleKeyCtx

A scaleKeyCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `scaleSpecifiedKeys` | `ssk` |  |
| `type` | `typ` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## scriptCtx

A scriptCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `toolStart` | `ts` |  |
| `toolFinish` | `tf` |  |
| `finalCommandScript` | `fcs` |  |
| `setSelectionCount` | `ssc` |  |
| `setAllowExcessCount` | `sae` |  |
| `setAutoToggleSelection` | `sat` |  |
| `setAutoComplete` | `sac` |  |
| `setDoneSelectionPrompt` | `dsp` |  |
| `setNoSelectionPrompt` | `nsp` |  |
| `setNoSelectionHeadsUp` | `nsh` |  |
| `setSelectionPrompt` | `ssp` |  |
| `setSelectionHeadsUp` | `ssh` |  |
| `totalSelectionSets` | `tss` |  |
| `exitUponCompletion` | `euc` |  |
| `enableRootSelection` | `ers` |  |
| `ignoreInvalidItems` | `iii` |  |
| `cumulativeLists` | `cls` |  |
| `expandSelectionList` | `esl` |  |
| `showManipulators` | `sm` |  |
| `baseClassName` | `bcn` |  |
| `toolCursorType` | `tct` |  |
| `allComponents` | `alc` |  |
| `vertex` | `v` |  |
| `edge` | `eg` |  |
| `facet` | `fc` |  |
| `curve` | `c` |  |
| `polymesh` | `pl` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## scriptJob

Run a script when something happens: -e event script, -ct -cf -cc condition script, -ac attribute script, -nd node script, -uid control script, -tc script, -ie script. -ro once, -kws with the scene, -p with a control, -k -ka kill, -lj list. Returns the job number.

| Flag | Short | Description |
| --- | --- | --- |
| `event` | `e` |  |
| `conditionTrue` | `ct` |  |
| `conditionFalse` | `cf` |  |
| `conditionChange` | `cc` |  |
| `attributeChange` | `ac` |  |
| `attributeDeleted` | `ad` |  |
| `attributeAdded` | `aa` |  |
| `connectionChange` | `con` |  |
| `nodeDeleted` | `nd` |  |
| `nodeNameChanged` | `nnc` |  |
| `uiDeleted` | `uid` |  |
| `timeChange` | `tc` |  |
| `idleEvent` | `ie` |  |
| `kill` | `k` |  |
| `exists` | `ex` |  |
| `parent` | `p` |  |
| `killAll` | `ka` |  |
| `force` | `f` |  |
| `listJobs` | `lj` |  |
| `list` | `l` |  |
| `listEvents` | `le` |  |
| `listConditions` | `lc` |  |
| `runOnce` | `ro` |  |
| `killWithScene` | `kws` |  |
| `protected` | `pro` |  |
| `permanent` | `per` |  |
| `replacePrevious` | `rp` |  |
| `compressUndo` | `cu` |  |
| `disregardIndex` | `di` |  |
| `allChildren` | `alc` |  |

## scriptNode

A node holding scripts saved with the scene: -bs before, -as after, -st 0 demand, 1 open and close, 2 GUI open and close, 7 time changed, -stp mel or python, -eb -ea run them.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `beforeScript` | `bs` |  |
| `afterScript` | `as` |  |
| `scriptType` | `st` |  |
| `sourceType` | `stp` |  |
| `executeBefore` | `eb` |  |
| `executeAfter` | `ea` |  |
| `ignoreReferenceEdit` | `ire` |  |

## scriptTable

Script UI: scriptTable, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `rows` | `r` |  |
| `columns` | `c` |  |
| `label` | `l` |  |
| `columnWidth` | `cw` |  |
| `rowHeight` | `rh` |  |
| `getCellCmd` | `gcc` |  |
| `cellChangedCmd` | `ccc` |  |
| `afterCellChangedCmd` | `acc` |  |
| `selectionChangedCmd` | `scc` |  |
| `selectedRow` | `sr` |  |
| `selectedRows` | `srs` |  |
| `selectedColumns` | `scs` |  |
| `selectedCells` | `sc` |  |
| `clearTable` | `clr` |  |
| `clearRow` | `cr` |  |
| `deleteRow` | `dr` |  |
| `insertRow` | `ir` |  |
| `excludingHeaders` | `eh` |  |
| `multiEditEnabled` | `mee` |  |
| `underPointerRow` | `upr` |  |
| `underPointerColumn` | `upc` |  |
| `useDoubleClickEdit` | `dce` |  |
| `sortEnabled` | `se` |  |
| `cellIndex` | `ci` |  |
| `cellValue` | `cv` |  |
| `cellBackgroundColor` | `cbc` |  |
| `cellForegroundColor` | `cfc` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## scriptedPanel

A panel of a scripted type: -typ the type, -l, -p. The type's callbacks build it.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `type` | `typ` |  |
| `label` | `l` |  |
| `parent` | `p` |  |
| `control` | `ctl` |  |
| `copy` | `cp` |  |
| `createString` | `cs` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `editString` | `es` |  |
| `exists` | `ex` |  |
| `init` | `in` |  |
| `isUnique` | `iu` |  |
| `menuBarVisible` | `mbv` |  |
| `needsInit` | `ni` |  |
| `popupMenuProcedure` | `pmp` |  |
| `replacePanel` | `rp` |  |
| `tearOff` | `to` |  |
| `tearOffCopy` | `toc` |  |
| `unParent` | `up` |  |
| `useTemplate` | `ut` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## scriptedPanelType

Declare a scripted panel type: -ccb -icb -acb -rcb -dcb -scb the procedures that build and keep it.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `createCallback` | `ccb` |  |
| `initCallback` | `icb` |  |
| `addCallback` | `acb` |  |
| `removeCallback` | `rcb` |  |
| `deleteCallback` | `dcb` |  |
| `saveStateCallback` | `scb` |  |
| `hotkeyCtxCallback` | `hcb` |  |
| `customView` | `cv` |  |
| `defineTemplate` | `dt` |  |
| `exists` | `ex` |  |
| `label` | `l` |  |
| `obsolete` | `o` |  |
| `retainOnFileOpen` | `rfo` |  |
| `unique` | `u` |  |
| `useTemplate` | `ut` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## scrollField

Script UI: scrollField, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `text` | `tx` |  |
| `editable` | `ed` |  |
| `wordWrap` | `ww` |  |
| `changeCommand` | `cc` |  |
| `enterCommand` | `ec` |  |
| `keyPressCommand` | `kpc` |  |
| `font` | `fn` |  |
| `numberOfLines` | `nl` |  |
| `insertText` | `it` |  |
| `insertionPosition` | `ip` |  |
| `clear` | `cl` |  |
| `selection` | `sl` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## scrollLayout

Script UI: scrollLayout, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `horizontalScrollBarThickness` | `hst` |  |
| `verticalScrollBarThickness` | `vst` |  |
| `childResizable` | `cr` |  |
| `resizeCommand` | `rc` |  |
| `scrollAreaHeight` | `sah` |  |
| `scrollAreaWidth` | `saw` |  |
| `scrollAreaValue` | `sav` |  |
| `scrollByPixel` | `sbp` |  |
| `scrollPage` | `sp` |  |
| `minChildWidth` | `mcw` |  |
| `verticalScrollBarAlwaysVisible` | `vsb` |  |
| `panEnabled` | `pe` |  |
| `childArray` | `ca` |  |
| `numberOfChildren` | `nch` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## sculpt

Sculpt Deformer: a sphere that pushes points. Returns [sculpt1, sculptor1, sculpt1StretchOrigin]. -mode flip, project or stretch, -mxd the most a point moves, -dds and -dt how it fades past the surface, -st an object to use as the sphere.

Arguments: what to deform, and any target last (the selection when none); the deformer with -e and -q

| Flag | Short | Description |
| --- | --- | --- |
| `name` | `n` | Name of the deformer. |
| `before` | `bf` | Go just above the deformer nearest the mesh. |
| `after` | `af` | Go nearest the mesh, after the other deformers (the default). |
| `frontOfChain` | `foc` | Go above every other deformer. |
| `envelope` | `en` | How much of the deformation applies (1). |
| `edit` | `e` | Edit mode. |
| `query` | `q` | Query mode. |
| `mode` | `m` | flip, project or stretch (the default). |
| `insideMode` | `im` | ring or even (the default). |
| `maxDisplacement` | `mxd` | The most a point moves (0.1). |
| `dropoffType` | `dt` | none or linear (the default). |
| `dropoffDistance` | `dds` | How far past the surface it fades (1). |
| `sculptTool` | `st` | An object to use as the sphere. |

## sculptMeshCacheCtx

The sculpting tools' settings. -st the brush (sculpt, smooth, relax, grab, pinch, flatten, foamy, spray, repeat, imprint, wax, scrape, fill, knife, smear, bulge, amplify, freeze), -sz size, -str strength, -inv invert, -sym symmetry, -q to ask. -cf clears the freeze.

Arguments: the tool's name

| Flag | Short | Description |
| --- | --- | --- |
| `sculptTool` | `st` | The brush. |
| `size` | `sz` | Brush radius, world units. |
| `strength` | `str` | Strength, 0 to 1. |
| `invert` | `inv` | Push the other way. |
| `symmetry` | `sym` | Mirror across the object's YZ plane. |
| `clearFreeze` | `cf` | Unfreeze everything. |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |

## sculptStamp

One dab of the sculpting brush at a world point on a mesh: for scripts and tests. -p the point, -n the surface normal, -d the drag for Grab and Smear. Returns how many points it moved.

Arguments: a mesh

| Flag | Short | Description |
| --- | --- | --- |
| `position` | `p` | World centre. |
| `normal` | `n` | World normal. |
| `drag` | `d` | World drag since the last dab. |
| `invert` | `inv` | As Ctrl. |
| `smooth` | `sm` | As Shift. |

## sculptTarget

Edit a blend shape target through the base: sculptTarget -e -t 0 blendShape1 starts, and edits of the base go into target 0 when -t -1 ends it. -e -r -t 0 makes the target's shape again, live, and returns it. -q gives the target being edited.

Arguments: a blendShape

| Flag | Short | Description |
| --- | --- | --- |
| `edit` | `e` | Edit mode. |
| `query` | `q` | Query mode. |
| `target` | `t` | The target to edit, -1 for none. |
| `inbetweenWeight` | `ibw` | Which in-between of the target. |
| `regenerate` | `r` | Make the target's shape again. |

## searchReplaceNames

searchReplaceNames(search, replace, 'all'|'hierarchy'|'selected'): renames what holds search. Returns how many.

Arguments: search replace scope

## select

Select objects. Replaces the selection unless add/deselect/toggle.

Arguments: objects

| Flag | Short | Description |
| --- | --- | --- |
| `add` | `add` |  |
| `deselect` | `d` |  |
| `toggle` | `tgl` |  |
| `clear` | `cl` | Select nothing. |
| `replace` | `r` | Replace the selection (the default). |
| `noExpand` | `ne` | A set itself, not its members. |
| `hierarchy` | `hi` | Everything below too. |
| `addFirst` | `af` | Add at the front of the list. |
| `allDagObjects` | `ado` | Every top level object. |
| `allDependencyNodes` | `adn` | Every node. |
| `visible` | `vis` | Only visible objects. |
| `all` | `all` | Every object. |
| `symmetry` | `sym` | Accepted for existing scripts. |
| `symmetrySide` | `sys` | Accepted for existing scripts. |

## selectAllComponents

Select every component of the current kind on the hilited meshes.

## selectBorder

Select border edges: the perimeter of selected faces, or the whole open boundary through selected edges or vertices.

## selectByAngle

Grow the face selection across edges where neighbouring faces bend less than an angle.

| Flag | Short | Description |
| --- | --- | --- |
| `angle` |  | Largest bend between neighbouring faces, degrees. |

## selectComponents

Select vertices, edges or faces of a mesh by id. Ids may be [first, last] ranges. Without add/deselect/toggle it replaces the selection of that kind.

Arguments: mesh

| Flag | Short | Description |
| --- | --- | --- |
| `vertices` |  | Point ids. |
| `edges` |  | Edge ids. |
| `faces` |  | Face ids. |
| `add` |  | Add to the selection. |
| `deselect` |  | Remove from the selection. |
| `toggle` |  | Toggle. |
| `symmetric` |  | Also the mirror images, when the mesh's symmetry is on. |

## selectComponentsWhere

Select a mesh's components by predicates (the flags of findComponents) and switch to that component mode. With symmetry on, edits still mirror; select one side. Returns the selected ids.

Arguments: [mesh]

| Flag | Short | Description |
| --- | --- | --- |
| `kind` | `k` | vertex, edge or face. |
| `normal` | `n` | Keep components whose normal is within angle of this direction (world space). |
| `angle` |  | Tolerance for normal=, degrees. |
| `side` | `s` | '+x', '-x', '+y', ... keeps components whose centre is on that side of the object-space origin plane; 'center' (or 'centerx', 'centery', 'centerz') keeps those on the plane. |
| `box` | `b` | [[minX, minY, minZ], [maxX, maxY, maxZ]], world space: keep components whose centre is inside. None leaves that side open, so [[None, 20, None], [None, None, None]] is everything above y=20. |
| `direction` | `dir` | Edges only: keep edges running along this world direction (either way) within angle; (0, 1, 0) finds vertical edges, whose ring polyInsertLoop cuts with a horizontal loop. |
| `near` |  | World point; with radius= keeps components within it, else keeps the count nearest. |
| `radius` | `r` | Distance for near=, cm. |
| `extreme` | `x` | '+y', '-z', ...: keep the count components furthest along that world direction (the top face is extreme='+y'). |
| `count` | `c` | How many to keep for extreme= or near= without radius. |
| `border` |  | True keeps only components on an open border, False only interior ones. |
| `shell` |  | Keep components of one connected shell: 0 is the largest, numbered as meshCheck lists them. |
| `fromSelection` | `fs` | Only consider the currently selected components of that kind. |
| `tolerance` | `t` | Distance counted as on a plane for side=, cm (default scales with the mesh). |
| `add` |  | Add to the selection of that kind instead of replacing it. |
| `deselect` |  | Remove the matches from the selection. |

## selectConnected

Select every component of a kind on the connected piece (faces joined through shared edges) that one component belongs to. Without add/deselect/toggle it replaces the selection of that kind. Returns the ids.

Arguments: mesh

| Flag | Short | Description |
| --- | --- | --- |
| `kind` |  | vertex \| edge \| face |
| `id` |  | A component of the piece to select. |
| `add` |  | Add to the selection. |
| `deselect` |  | Remove from the selection. |
| `toggle` |  | Toggle. |
| `symmetric` |  | Also the mirror images, when the mesh's symmetry is on. |

## selectContainedFaces

Select the faces enclosed by the selected edges: a flood fill that stops at selected edges and mesh borders. On each piece of a mesh the smaller regions are kept (every one but the largest by area), or with larger=True only the largest. Switches to face mode.

| Flag | Short | Description |
| --- | --- | --- |
| `larger` |  | Keep the largest region instead of the smaller ones. |

## selectDisplayLayerMembers

Select the objects in display layers.

Arguments: layers

| Flag | Short | Description |
| --- | --- | --- |
| `add` |  | Add them to the selection instead of replacing it. |

## selectFoldEdges

Select fold edges of meshes by the autoBevel rule (on the editable cage). Returns the number selected.

Arguments: meshes

| Flag | Short | Description |
| --- | --- | --- |
| `add` |  | Add to the edge selection instead of replacing it. |
| `minAngle` |  | Smallest fold, degrees between the face normals. |
| `maxAngle` |  | Largest fold, degrees. |
| `side` |  | both, outside (convex) or inside (concave). |
| `thin` |  | Skip thickness edges narrower than this, cm. |
| `minLength` |  | Skip edges shorter than this, cm. |
| `completeCorners` |  | Add the quad loops joining folds that meet at an angle. |

## selectHierarchy

Add every transform below each selected object to the selection, parents before children.

## selectInstanceSource

Select the source of each given instance or reference (a source selects itself). Returns the selection.

Arguments: meshes

## selectInstances

Select every mesh sharing data with the given ones: their source, instances and references. Returns the selection.

Arguments: meshes

| Flag | Short | Description |
| --- | --- | --- |
| `add` |  | Add to the selection instead of replacing it. |

## selectKey

Select keys, for the commands that take -sl. -t limits them; -add, -rm and -tgl change the selection, -r (the default) replaces it; -cl clears it. -uk picks keys off multiples of a time. Returns how many keys it touched. -q -kc counts them.

Arguments: objects, plugs or curves (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `attribute` | `at` | Channel name(s). |
| `time` | `t` | A frame or range "10:20". |
| `float` | `f` | A range of driver values on driven key curves, as -t is for time. |
| `index` | `in` | Key index(es) on each curve, or a range "0:2". |
| `includeUpperBound` | `iub` | Whether a range's end counts (on by default). |
| `hierarchy` | `hi` | none, below, above or both. |
| `controlPoints` | `cp` | Accepted for existing scripts: control points have no channels of their own here. |
| `shape` | `s` | Include the channels of the objects' shapes. |
| `animation` | `an` | objects (the default), keys or keysOrObjects: keys means the selected keys. |
| `addTo` | `add` | Add to the selection. |
| `remove` | `rm` | Remove from the selection. |
| `toggle` | `tgl` | Toggle in the selection. |
| `replace` | `r` | Replace the selection (the default). |
| `clear` | `cl` | Clear the selection. |
| `keyframe` | `k` | Select the keys themselves (the default). |
| `inTangent` | `it` | Accepted for existing scripts: selects the keys. |
| `outTangent` | `ot` | Accepted for existing scripts: selects the keys. |
| `unsnappedKeys` | `uk` | Only keys not on a multiple of this time (1 when 0). |
| `selected` | `sl` | Work on the keys already selected. |
| `keyframeCount` | `kc` | Query: how many selected. |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |

## selectKeyCtx

A selectKeyCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## selectKeyframe

Select keys, as selectKey.

Arguments: objects, plugs or curves (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `attribute` | `at` | Channel name(s). |
| `time` | `t` | A frame or range "10:20". |
| `float` | `f` | A range of driver values on driven key curves, as -t is for time. |
| `index` | `in` | Key index(es) on each curve, or a range "0:2". |
| `includeUpperBound` | `iub` | Whether a range's end counts (on by default). |
| `hierarchy` | `hi` | none, below, above or both. |
| `controlPoints` | `cp` | Accepted for existing scripts: control points have no channels of their own here. |
| `shape` | `s` | Include the channels of the objects' shapes. |
| `animation` | `an` | objects (the default), keys or keysOrObjects: keys means the selected keys. |
| `addTo` | `add` | Add to the selection. |
| `remove` | `rm` | Remove from the selection. |
| `toggle` | `tgl` | Toggle in the selection. |
| `replace` | `r` | Replace the selection (the default). |
| `clear` | `cl` | Clear the selection. |
| `keyframe` | `k` | Select the keys themselves (the default). |
| `inTangent` | `it` | Accepted for existing scripts: selects the keys. |
| `outTangent` | `ot` | Accepted for existing scripts: selects the keys. |
| `unsnappedKeys` | `uk` | Only keys not on a multiple of this time (1 when 0). |
| `selected` | `sl` | Work on the keys already selected. |
| `keyframeCount` | `kc` | Query: how many selected. |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |

## selectLoop

Extend the selection to full loops (or rings with ring=True): edges along their own loops, vertices along the loops of the edges joining them, faces along the face loops crossing the edges they share.

| Flag | Short | Description |
| --- | --- | --- |
| `ring` |  | Select rings instead of loops. Faces have no ring and take their loops. |

## selectLoopThrough

Select the edge loop (or ring) through one edge of a mesh, or with faces=True the face loop crossing it. Without add/deselect/toggle it replaces the selection of that kind. Returns the ids.

Arguments: mesh

| Flag | Short | Description |
| --- | --- | --- |
| `edge` |  | The edge the loop runs through. |
| `ring` |  | The edge ring instead of the loop. |
| `faces` |  | The faces of the edge's ring: the face loop crossing the edge. |
| `add` |  | Add to the selection. |
| `deselect` |  | Remove from the selection. |
| `toggle` |  | Toggle. |
| `symmetric` |  | Also the mirror images, when the mesh's symmetry is on. |

## selectMode

Switch between object, vertex, edge and face selection. With no argument, returns the current mode.

Arguments: [mode]

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query. |
| `object` | `o` | Object mode. |
| `component` | `co` | Component mode. |
| `hierarchical` | `h` | Hierarchy mode: objects here. |
| `preset` | `p` | Accepted for existing scripts. |
| `root` | `r` | Accepted for existing scripts. |
| `leaf` | `l` | Accepted for existing scripts. |
| `template` | `t` | Accepted for existing scripts. |

## selectPathBetween

Select the components between two of a mesh's vertices, edges or faces: the shorter way along a loop or ring they share, else the shortest path across the surface. Neighbours (vertices on one edge, edges along a loop or across a quad, faces sharing an edge) select the whole loop or ring through both instead. Adds to the selection unless replace or deselect. Returns the ids in order.

Arguments: mesh

| Flag | Short | Description |
| --- | --- | --- |
| `kind` |  | vertex \| edge \| face |
| `from` |  | Id the path starts at. |
| `to` |  | Id the path ends at. |
| `replace` |  | Replace the selection of that kind. |
| `deselect` |  | Remove the path from the selection. |
| `symmetric` |  | Also the mirror images, when the mesh's symmetry is on. |

## selectPref

Selection preferences: -clickDrag, -trackSelectionOrder, -preSelectHilite, -xformNoSelect, -useDepth, -affectsActive, -lockSelection. -q reads.

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query. |
| `clickDrag` | `cld` | Click and drag to select or to move what's under the cursor. |
| `trackSelectionOrder` | `tso` | Keep the order things were picked in. |
| `preSelectHilite` | `psh` | Highlight what's under the cursor. |
| `xformNoSelect` | `xns` | A click on an object under the transform tools doesn't reselect. |
| `useDepth` | `ud` | Marquee only picks what's visible. |
| `affectsActive` | `aa` | Selection affects the active list. |
| `lockSelection` | `lck` | The selection doesn't change on a click. |

## selectPriority

Selection priorities by component and object type: -cv 5, -q -cv. Kept for scripts.

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `allComponents` | `alc` | The priority. |
| `allObjects` | `alo` | The priority. |
| `animBreakdown` | `abd` | The priority. |
| `animCurve` | `ac` | The priority. |
| `animInTangent` | `ait` | The priority. |
| `animKeyframe` | `ak` | The priority. |
| `animOutTangent` | `aot` | The priority. |
| `byName` | `bn` | The priority. |
| `camera` | `ca` | The priority. |
| `cluster` | `cl` | The priority. |
| `collisionModel` | `clm` | The priority. |
| `controlVertex` | `cv` | The priority. |
| `curve` | `c` | The priority. |
| `curveKnot` | `ck` | The priority. |
| `curveOnSurface` | `cos` | The priority. |
| `curveParameterPoint` | `cpp` | The priority. |
| `dimension` | `dim` | The priority. |
| `dynamicConstraint` | `dc` | The priority. |
| `edge` | `eg` | The priority. |
| `editPoint` | `ep` | The priority. |
| `emitter` | `em` | The priority. |
| `facet` | `fc` | The priority. |
| `field` | `fi` | The priority. |
| `flexor` | `fl` | The priority. |
| `hull` | `hl` | The priority. |
| `ikEndEffector` | `iee` | The priority. |
| `ikHandle` | `ikh` | The priority. |
| `imagePlane` | `ip` | The priority. |
| `implicitGeometry` | `ig` | The priority. |
| `isoparm` | `iso` | The priority. |
| `joint` | `j` | The priority. |
| `jointPivot` | `jp` | The priority. |
| `lattice` | `la` | The priority. |
| `latticePoint` | `lp` | The priority. |
| `light` | `lt` | The priority. |
| `localRotationAxis` | `ra` | The priority. |
| `locator` | `lc` | The priority. |
| `locatorUV` | `luv` | The priority. |
| `locatorXYZ` | `lxyz` | The priority. |
| `nCloth` | `ncl` | The priority. |
| `nParticle` | `npr` | The priority. |
| `nRigid` | `nr` | The priority. |
| `nurbsCurve` | `nc` | The priority. |
| `nurbsSurface` | `ns` | The priority. |
| `orientationLocator` | `ol` | The priority. |
| `particle` | `pr` | The priority. |
| `plane` | `pl` | The priority. |
| `polymesh` | `p` | The priority. |
| `polymeshEdge` | `pe` | The priority. |
| `polymeshFace` | `pf` | The priority. |
| `polymeshFreeEdge` | `pfe` | The priority. |
| `polymeshUV` | `puv` | The priority. |
| `polymeshVertex` | `pv` | The priority. |
| `polymeshVtxFace` | `pvf` | The priority. |
| `queueTangent` | `qt` | The priority. |
| `rigidBody` | `rb` | The priority. |
| `rigidConstraint` | `rc` | The priority. |
| `rotatePivot` | `rp` | The priority. |
| `scalePivot` | `sp` | The priority. |
| `sculpt` | `sc` | The priority. |
| `selectHandle` | `sh` | The priority. |
| `spring` | `spr` | The priority. |
| `springComponent` | `spc` | The priority. |
| `stroke` | `str` | The priority. |
| `subdiv` | `sd` | The priority. |
| `surfaceEdge` | `se` | The priority. |
| `surfaceFace` | `sf` | The priority. |
| `surfaceKnot` | `skn` | The priority. |
| `surfaceParameterPoint` | `spp` | The priority. |
| `surfaceRange` | `srn` | The priority. |
| `surfaceUV` | `suv` | The priority. |
| `texture` | `tx` | The priority. |
| `vertex` | `v` | The priority. |

## selectScope

Status line scope: hierarchy (roots), object or component. No argument returns it.

Arguments: [scope]

## selectSetMembers

Select what sets hold: their objects, and their components in component mode. Returns the members.

Arguments: sets

| Flag | Short | Description |
| --- | --- | --- |
| `add` |  | Add them to the selection instead of replacing it. |

## selectType

Selection masks: -polymesh, -joint, -nurbsCurve, -ikHandle, -locator, -cluster, -camera and so on, each on or off; -cv, -v, -pv points, -pe edges, -pf faces pick the component kind. -q reads one.

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query. |
| `allObjects` | `alc` | Everything on. |
| `polymesh` | `pm` | This kind can be selected. |
| `nurbsCurve` | `nc` | This kind can be selected. |
| `nurbsSurface` | `ns` | This kind can be selected. |
| `joint` | `j` | This kind can be selected. |
| `ikHandle` | `ikh` | This kind can be selected. |
| `handle` | `ha` | This kind can be selected. |
| `cluster` | `cl` | This kind can be selected. |
| `nonlinear` | `nl` | This kind can be selected. |
| `locator` | `lc` | This kind can be selected. |
| `camera` | `ca` | This kind can be selected. |
| `group` | `gr` | This kind can be selected. |
| `controlVertex` | `cv` | CVs. |
| `vertex` | `v` | Points. |
| `polymeshVertex` | `pv` | Mesh vertices. |
| `polymeshEdge` | `pe` | Mesh edges. |
| `polymeshFace` | `pf` | Mesh faces. |

## selected

The selection: objects, or components in a component mode.

## selectedNodes

The selection as full paths: -do leaves out what isn't in the DAG.

| Flag | Short | Description |
| --- | --- | --- |
| `dagObjects` | `do` | Only DAG objects. |

## selectionConnection

A named list of objects for editors to show and follow: -activeList is the scene's selection, -worldList the scene, -objectList its own.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `activeList` | `act` |  |
| `modelList` | `mdl` |  |
| `keyframeList` | `key` |  |
| `worldList` | `wl` |  |
| `objectList` | `ol` |  |
| `highlightList` | `hl` |  |
| `connectionList` | `crl` |  |
| `characterList` | `cl` |  |
| `setList` | `sl` |  |
| `addScript` | `as` |  |
| `removeScript` | `rs` |  |
| `addTo` | `add` |  |
| `clear` | `clr` |  |
| `deselect` | `d` |  |
| `select` | `s` |  |
| `remove` | `rm` |  |
| `object` | `obj` |  |
| `filter` | `f` |  |
| `findObject` | `fo` |  |
| `global` | `g` |  |
| `identify` | `id` |  |
| `lock` | `lck` |  |
| `parent` | `p` |  |
| `switch` | `sw` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## separator

Script UI: separator, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `style` | `st` |  |
| `horizontal` | `hr` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## setAttr

setAttr('node.attr', value). Vec3 takes a list or three numbers. lock=True/False locks or unlocks the plug, with or without a value: a locked plug can't be changed until it is unlocked.

Arguments: plug [value]

| Flag | Short | Description |
| --- | --- | --- |
| `keyable` | `k` | Make the plug keyable or not. |
| `channelBox` | `cb` | Show the plug in the channel box without keying. |
| `lock` | `l` | Lock or unlock the plug. |
| `type` | `typ` | double3, string, matrix, stringArray, doubleArray and so on. |
| `clamp` | `c` | Accepted: values are always held inside an attribute's limits. |
| `size` | `s` | Accepted for existing scripts. |
| `alteredValue` | `av` | Accepted for existing scripts. |
| `caching` | `ca` | Accepted for existing scripts. |

## setAttrMapping

Held only: an offset and scale for a device attachment. Input devices are not supported, so it has no effect. -q reads a flag back.

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `absolute` | `a` | Absolute mapping. |
| `attribute` | `at` | The attribute. |
| `axis` | `ax` | The device axis. |
| `clutch` | `c` | The clutch button. |
| `device` | `d` | The device. |
| `offset` | `o` | The offset. |
| `relative` | `r` | Relative mapping. |
| `scale` | `s` | The scale. |
| `selection` | `sl` | On the selected objects. |

## setCharacterObject

Put a joint in a character's slot, as setCharacterObject(joint, character, slot, 0); the slot by id or name, the last 1 to take it out.

Arguments: joint, character, slot, 0

## setCurvePoints

Move a curve's CVs at once, in object space: -p [[x, y, z], ...], from the first CV. A periodic curve's wrapped CVs follow their first lap.

Arguments: a curve (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `points` | `p` | [[x, y, z], ...]. |

## setDrivenKeyframe

Set a driven key: at the driver's value (-cd plug, -dv value, or its current one) the channel takes its value (-v, or its current one). -q -dr lists a channel's drivers, -q -dn a driver's driven channels.

Arguments: the driven object or channels (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `currentDriver` | `cd` | The driving attribute. |
| `driverValue` | `dv` | The driver's value at this key (its current one by default). |
| `value` | `v` | The channel's value at this key (its current one by default). |
| `attribute` | `at` | Channel name(s). |
| `inTangentType` | `itt` | Tangent into the key. |
| `outTangentType` | `ott` | Tangent out of the key. |
| `driver` | `dr` | Query: the channel's drivers. |
| `driven` | `dn` | Query: the driver's driven channels. |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |

## setEditCtx

A setEditCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `setName` | `sn` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## setFocus

Give a panel or control the focus.

Arguments: name

## setInfinity

What curves do before and after their keys: -pri and -poi constant, linear, cycle, cycleRelative or oscillate.

Arguments: objects, plugs or curves (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `attribute` | `at` | Channel name(s). |
| `preInfinite` | `pri` | Before the first key. |
| `postInfinite` | `poi` | After the last key. |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |

## setKeyCtx

A setKeyCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `breakdown` | `bd` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## setKeyPath

Key an object's position on a motion path at the current time: makes the path and curve the first time, adds a point after that. Returns [curve, motionPath].

Arguments: objects

## setKeyframe

Key channels at the current frame (or -t). Objects key all their keyable channels unless -at names some. -i inserts a key that keeps the curve's shape. -al names the animation layer. Returns the number of keys set.

Arguments: objects or object.attribute plugs (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `attribute` | `at` | Channel name(s) to key: tx, rotateY... |
| `time` | `t` | Frame(s) to key at. |
| `float` | `f` | Accepted for existing scripts: driven keys are set with setDrivenKeyframe. |
| `value` | `v` | Value to key instead of the current one. |
| `inTangentType` | `itt` | Tangent into the key. |
| `outTangentType` | `ott` | Tangent out of the key. |
| `breakdown` | `bd` | The key is a breakdown: it keeps its place between the keys around it. |
| `insert` | `i` | Insert a key on the curve, keeping its shape. The tangents around it become fixed. |
| `hierarchy` | `hi` | none, below, above or both. |
| `minimizeRotation` | `mr` | Key rotations as the turn nearest the previous key. |
| `noResolve` | `nr` | With -v: put the value straight on the layer's curve. |
| `respectKeyable` | `rk` | Skip channels that aren't keyable. |
| `controlPoints` | `cp` | Accepted for existing scripts: control points have no channels of their own here. |
| `shape` | `s` | Key the shapes' keyable channels too. |
| `animLayer` | `al` | The layer to key. Ignored for channels not in it, as in the reference. |
| `dirtyDG` | `dd` | Accepted for existing scripts: keying always dirties what it drives. |
| `identity` | `id` | An identity key: one that nullifies the layer here. Only on channels driven by layers. |

## setKeyframeBlendshapeTargetWts

Key per-point blend shape target weights at the current time: a selected base keys all its targets, a selected live target only itself. Returns how many keys. The keys go on the node's targetWeightKeys elements, which override the painted weight for that point: the reference has them under inputTarget, which are not plugs here.

Arguments: a base or a target (the selection when none)

## setMembers

A set's members as names and components (pCube1, pCube1.vtx[0:3]). nodes=True gives the distinct objects instead, component owners included.

Arguments: set

| Flag | Short | Description |
| --- | --- | --- |
| `nodes` |  | Return the objects the set touches instead of its members. |

## setMeshPoints

Move every point of a mesh's base (under its modifiers) at once, in meshData's point order, in object space. Topology stays. One undo step. Python's Node.set_points takes a numpy array, faster.

Arguments: mesh

| Flag | Short | Description |
| --- | --- | --- |
| `points` |  | [[x, y, z], ...], one per point, in cm. |
| `buffer` |  | Internal: points handed over from Python (Node.set_points). |

## setMeshUVs

Set the UV of every face corner of a mesh at once, face by face in order: -u and -v, one number per corner. One polyTweakUV history node.

Arguments: a mesh

| Flag | Short | Description |
| --- | --- | --- |
| `u` |  | U per face corner. |
| `v` |  | V per face corner. |

## setParent

The parent new controls go under: a name, .. for one level up, -top the window, -menu the menu parent instead. -q reads it.

Arguments: [name or ..]

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` |  |
| `edit` | `e` |  |
| `menu` | `m` |  |
| `topLevel` | `top` |  |
| `upLevel` | `u` |  |
| `defineTemplate` | `dt` |  |
| `useTemplate` | `ut` |  |

## setParticleAttr

Set per particle values now: -at velocity -vv 0 1 0 particle1.pt[3].

Arguments: particles or particle1.pt[0:3]

| Flag | Short | Description |
| --- | --- | --- |
| `attribute` | `at` | position, velocity, rgbPP, opacityPP, lifespanPP, radiusPP. |
| `vectorValue` | `vv` | A vector. |
| `floatValue` | `fv` | A number. |

## setPivot

Move the rotate and scale pivots to a world position, or orient the pivot, without moving the objects. A joint's pivot is the joint itself: position= moves the joint and orient= sets its jointOrient, leaving its children where they are.

Arguments: objects

| Flag | Short | Description |
| --- | --- | --- |
| `position` |  | World position, cm. |
| `orient` |  | Pivot orientation relative to the object's axes, Euler XYZ degrees (pivotOrient); on a joint, its jointOrient. |
| `turnAxis` |  | World axis to turn the pivot's axes about, by turnAngle. |
| `turnAngle` |  | Degrees to turn about turnAxis. |

## setSymmetry

Object-space symmetry for component edits on meshes: 'off', a mirror plane 'x', 'y' or 'z', or 'radial': count wedges repeated round radialAxis (with mirror=True also mirrored within each wedge), every edit applied to all of them.

Arguments: objects

| Flag | Short | Description |
| --- | --- | --- |
| `axis` |  | off, x, y, z or radial. |
| `count` |  | Radial: how many wedges repeat round the axis, 2 to 64; default: the mesh's own. |
| `radialAxis` |  | Radial: the axis, x, y, z or custom (direction=); default: the mesh's own. |
| `direction` |  | Radial: a custom axis direction, object space. |
| `mirror` |  | Radial: also mirror each wedge across a plane through the axis (dihedral symmetry). |
| `phase` |  | Radial: degrees the wedge boundaries and mirror planes turn about the axis. |

## setSymmetryPlane

Set where meshes' symmetry plane lies: through the selected edges or vertices (which must lie flat across an axis; that axis becomes the symmetry axis), or back through the pivot with pivot=True. With radial symmetry the radial axis passes through the selection's centre instead. Returns the plane per mesh as [axis, offset], or ['radial', centre].

Arguments: objects

| Flag | Short | Description |
| --- | --- | --- |
| `pivot` |  | Put the plane (or radial axis) back through the pivot. |

## setToolTo

Make a tool context current, by name.

Arguments: context

## setUITemplate

Make a template current: -pushTemplate, -popTemplate. Returns the current one.

Arguments: [template]

| Flag | Short | Description |
| --- | --- | --- |
| `pushTemplate` | `pst` |  |
| `popTemplate` | `ppt` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## setXformManip

The manipulator's settings: -showUnits, -suppress, -useRotatePivot, -useScalePivot, -worldSpace. Held; the views go by the tools' own.

| Flag | Short | Description |
| --- | --- | --- |
| `showUnits` | `su` |  |
| `suppress` | `s` |  |
| `useRotatePivot` | `urp` |  |
| `useScalePivot` | `usp` |  |
| `worldSpace` | `ws` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## sets

The reference's object set command: create, add to, take from, list and combine sets.

Arguments: items, or a set with -q

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | List a set's members. |
| `edit` | `e` | Edit. |
| `name` | `n` | The new set's name. |
| `empty` | `em` | Hold nothing. |
| `addElement` | `add` | Add the items to this set. |
| `forceElement` | `fe` | Add the items to this set. |
| `include` | `in` | Add the items to this set. |
| `remove` | `rm` | Take the items out of this set. |
| `clear` | `cl` | Empty this set. |
| `isMember` | `im` | Whether the items are in this set. |
| `isIntersecting` | `ii` | Whether this set and the others share members. |
| `union` | `un` | This set's members and the others'. |
| `intersection` | `int` | What this set shares with the others. |
| `subtract` | `sub` | This set less the others. |
| `size` | `s` | With -q: how many. |
| `flatten` | `fl` | Accepted for existing scripts. |
| `noWarnings` | `nw` | Accepted for existing scripts. |
| `vertices` | `v` | Accepted for existing scripts. |
| `edges` | `ed` | Accepted for existing scripts. |
| `facets` | `fc` | Accepted for existing scripts. |
| `text` | `t` | Accepted for existing scripts. |
| `noSurfaceShader` | `nss` | With -renderable: leave the new shading group with no material, rather than lambert1. |
| `renderable` | `r` | Make a shading group; with -q, whether the set is one. |

## shadingNode

Create a rendering or utility node: shadingNode -asUtility multiplyDivide -n name.

Arguments: type

| Flag | Short | Description |
| --- | --- | --- |
| `isColorManaged` | `icm` | The node's colours are colour managed. Accepted: textures read their colour space here. |
| `asRendering` | `ar` | A rendering node. |
| `asUtility` | `au` | A utility. |
| `asShader` | `as` | A shader. |
| `asTexture` | `at` | A texture. |
| `asLight` | `al` | A light. |
| `asPostProcess` | `app` | A post process. |
| `asRendering` | `ar` | A rendering node. |
| `name` | `n` | Its name. |
| `shared` | `s` | Reuse a node of that name. |
| `skipSelect` | `ss` | Accepted: shading nodes don't change the selection. |

## shelfButton

Script UI: shelfButton, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `style` | `st` |  |
| `image` | `i` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `label` | `l` |  |
| `command` | `c` |  |
| `doubleClickCommand` | `dcc` |  |
| `sourceType` | `stp` |  |
| `imageOverlayLabel` | `iol` |  |
| `flat` | `fla` |  |
| `disabledImage` | `di` |  |
| `align` | `al` |  |
| `marginWidth` | `mw` |  |
| `marginHeight` | `mh` |  |
| `font` | `fn` |  |
| `labelOffset` | `lo` |  |
| `rotation` | `rot` |  |
| `selectionImage` | `si` |  |
| `highlightImage` | `hi` |  |
| `overlayLabelColor` | `olc` |  |
| `overlayLabelBackColor` | `olb` |  |
| `scaleIcon` | `sic` |  |
| `commandRepeatable` | `rpt` |  |
| `ltVersion` | `lt` |  |
| `version` | `ver` |  |
| `useAlpha` | `ua` |  |
| `flexibleWidthType` | `fwt` |  |
| `flexibleWidthValue` | `fwv` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## shelfLayout

Script UI: shelfLayout, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `cellWidth` | `cwd` |  |
| `cellHeight` | `ch` |  |
| `cellWidthHeight` | `cwh` |  |
| `spacing` | `spa` |  |
| `alignment` | `aln` |  |
| `position` | `pos` |  |
| `style` | `st` |  |
| `version` | `ver` |  |
| `childArray` | `ca` |  |
| `numberOfChildren` | `nch` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## shelfTabLayout

Script UI: shelfTabLayout, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `tabLabel` | `tl` |  |
| `tabLabelIndex` | `tli` |  |
| `selectTabIndex` | `sti` |  |
| `selectTab` | `st` |  |
| `changeCommand` | `cc` |  |
| `selectCommand` | `sc` |  |
| `preSelectCommand` | `psc` |  |
| `doubleClickCommand` | `dcc` |  |
| `tabsVisible` | `tv` |  |
| `innerMarginWidth` | `imw` |  |
| `innerMarginHeight` | `imh` |  |
| `scrollable` | `scr` |  |
| `childResizable` | `cr` |  |
| `horizontalScrollBarThickness` | `hst` |  |
| `verticalScrollBarThickness` | `vst` |  |
| `moveTab` | `mt` |  |
| `newTabCommand` | `ntc` |  |
| `showNewTab` | `snt` |  |
| `tabPosition` | `tp` |  |
| `tabTooltip` | `tt` |  |
| `tabsClosable` | `tc` |  |
| `closeTabCommand` | `ctc` |  |
| `minChildWidth` | `mcw` |  |
| `image` | `i` |  |
| `imageVisible` | `iv` |  |
| `childArray` | `ca` |  |
| `numberOfChildren` | `nch` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## show

Show objects. all=True shows every hidden object.

Arguments: objects

| Flag | Short | Description |
| --- | --- | --- |
| `all` |  | Show everything. |

## showHidden

Show objects: named, -a every one, -b those below the named.

Arguments: objects

| Flag | Short | Description |
| --- | --- | --- |
| `all` | `a` | Every hidden object. |
| `below` | `b` | The objects under those named. |
| `above` | `ab` | The objects above those named. |

## showLastHidden

Show the objects the last hide (or hiding toggle) hid. Returns them.

## showManipCtx

A showManipCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `currentNodeName` | `cnn` |  |
| `incSnap` | `is` |  |
| `incSnapRelative` | `isr` |  |
| `incSnapUI` | `isu` |  |
| `incSnapValue` | `isv` |  |
| `lockSelection` | `ls` |  |
| `toggleIncSnap` | `tis` |  |
| `toolFinish` | `tf` |  |
| `toolStart` | `ts` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## showWindow

Show a window; with no name, the one being built.

Arguments: [window]

## shrinkSelection

Shrink the component selection by dropping what lies on its border.

| Flag | Short | Description |
| --- | --- | --- |
| `alongLoop` |  | Open ends draw in by one along their loops. |

## shrinkWrap

Shrink Wrap: moves points onto a target mesh, named by -t or last. Returns [shrinkWrap1].

Arguments: what to deform, and any target last (the selection when none); the deformer with -e and -q

| Flag | Short | Description |
| --- | --- | --- |
| `name` | `n` | Name of the deformer. |
| `before` | `bf` | Go just above the deformer nearest the mesh. |
| `after` | `af` | Go nearest the mesh, after the other deformers (the default). |
| `frontOfChain` | `foc` | Go above every other deformer. |
| `envelope` | `en` | How much of the deformation applies (1). |
| `edit` | `e` | Edit mode. |
| `query` | `q` | Query mode. |
| `target` | `t` | The mesh to shrink onto. |
| `projection` | `prj` | 0 off, 1 towards inner surface, 2 towards center, 3 parallel to axes, 4 vertex normals (the default), 5 closest point. |
| `closestIfNoIntersection` | `cin` | A point whose ray hits nothing goes to the closest point. |
| `bidirectional` | `bd` | Cast each way, taking the nearer hit. |
| `targetInflation` | `ti` | Push the target's surface out by this. |
| `offset` | `off` | Hold points this far off the surface. |
| `reverse` | `rev` | Cast the opposite way. |
| `alongX` | `ax` | Parallel to axes: along X (on). |
| `alongY` | `ay` | Parallel to axes: along Y (on). |
| `alongZ` | `az` | Parallel to axes: along Z (on). |

## simplify

Remove keys while the curve stays within -vtl of what it was. -t limits it to a range. Returns how many curves it went through.

Arguments: objects, plugs or curves (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `attribute` | `at` | Channel name(s). |
| `time` | `t` | A frame or range "10:20". |
| `float` | `f` | A range of driver values on driven key curves, as -t is for time. |
| `index` | `in` | Key index(es) on each curve, or a range "0:2". |
| `includeUpperBound` | `iub` | Whether a range's end counts (on by default). |
| `hierarchy` | `hi` | none, below, above or both. |
| `controlPoints` | `cp` | Accepted for existing scripts: control points have no channels of their own here. |
| `shape` | `s` | Include the channels of the objects' shapes. |
| `animation` | `an` | objects (the default), keys or keysOrObjects: keys means the selected keys. |
| `valueTolerance` | `vtl` | How far the curve may move (0.01). |
| `timeTolerance` | `tol` | Keys closer than this become one (0.05). |
| `floatTolerance` | `ftl` | As -tol, for driven curves. |

## singleProfileBirailSurface

A profile along two rails, its ends staying on them: profile, rail, rail. Returns [birailSurface1, singleProfileBirailSurface1].

Arguments: a profile and two rails (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `tangentContinuityProfile1` | `tp1` | Accepted for existing scripts. |
| `transformMode` | `tm` | Accepted for existing scripts. |
| `constructionHistory` | `ch` | Keep the node (on by default). |
| `polygon` | `po` | 0 NURBS, 1 polygons. |
| `range` | `rn` | Accepted for existing scripts. |
| `name` | `n` | Name of the new object. |
| `object` | `o` | Accepted for existing scripts. |

## skeletonEmbed

Fit a joint chain inside meshes: a plain heuristic, a straight run along the bounds' longest axis through the middle of each slice of points. The reference embeds a whole skeleton from a voxel segmentation; -sm and -sr are accepted and change nothing. -jc is this command's own: how many joints (5). Returns the joints; -q the positions; -q -mm the merged mesh as points and faces.

Arguments: meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `mergedMesh` | `mm` | With -q: the meshes merged. |
| `segmentationMethod` | `sm` | Accepted for existing scripts; has no effect. |
| `segmentationResolution` | `sr` | Accepted for existing scripts; has no effect. |
| `jointCount` | `jc` | How many joints. |

## skinBindCtx

A skinBindCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `about` | `a` |  |
| `axis` | `ax` |  |
| `colorRamp` | `cr` |  |
| `currentInfluence` | `ci` |  |
| `displayInactiveMode` | `dim` |  |
| `displayNormalized` | `dn` |  |
| `falloffCurve` | `fc` |  |
| `symmetry` | `sym` |  |
| `tolerance` | `tol` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## skinCluster

Bind skin: the mesh named last (or selected last) follows the joints, weighted by closest distance. -mi caps the influences per point, -dr sets the falloff, -bm the bind method, -sm the skinning method, -tsb binds to only the joints given. Edit: -ub unbinds, -ubk unbinds keeping history, -ai and -ri add and remove influences, -lw with -inf locks weights, -fnw normalizes, -sw smooths the selected points. Query: -inf, -wi, -g, -mi, -dr, -sm, -lw.

Arguments: joints, then the mesh (the selection when none); a skinCluster or skinned mesh with -e and -q

| Flag | Short | Description |
| --- | --- | --- |
| `toSelectedBones` | `tsb` | Only the joints given, not their whole skeleton. |
| `maximumInfluences` | `mi` | Most joints per point (5). |
| `obeyMaxInfluences` | `omi` | Hold points to maximumInfluences while weights are edited. |
| `dropoffRate` | `dr` | Falloff with distance (4). |
| `heatmapFalloff` | `hmf` | Heat map bind: 1 keeps the heat as solved, less tightens it (0.68). |
| `moveJointsMode` | `mjm` | With -e: on, joints move without moving the mesh; off, where they are becomes the bind pose. |
| `useGeometry` | `ug` | With -ai: the influence is a mesh, and points take weight by their distance from its surface. |
| `baseShape` | `bsh` | Accepted for existing scripts. |
| `polySmoothness` | `ps` | Accepted for existing scripts. |
| `bindMethod` | `bm` | 0 closest distance, 1 closest in hierarchy, 2 heat map, 3 geodesic voxel. |
| `skinMethod` | `sm` | 0 classic linear, 1 dual quaternion, 2 blended point by point by .blendWeights. |
| `normalizeWeights` | `nw` | 0 none, 1 interactive, 2 post. |
| `weightDistribution` | `wd` | Accepted for existing scripts. |
| `includeHiddenSelections` | `ihs` | Accepted for existing scripts. |
| `name` | `n` | Name of the skinCluster. |
| `unbind` | `ub` | With -e: remove the skin, back to the bind pose. |
| `unbindKeepHistory` | `ubk` | With -e: stop the skinning, keeping the node and its weights. |
| `addInfluence` | `ai` | With -e: joints to add. |
| `removeInfluence` | `ri` | With -e: joints to remove; their weight goes to the others. |
| `removeUnusedInfluence` | `rui` | With -e: remove influences that hold no weight. |
| `weight` | `wt` | With -ai: starting weight of the added influence. |
| `influence` | `inf` | With -e -lw: the joint. Query: the influences. |
| `weightedInfluence` | `wi` | Query: influences holding weight. |
| `geometry` | `g` | Query: the deformed shape. |
| `lockWeights` | `lw` | With -inf: lock or unlock an influence's weights; alone on -e, every influence. |
| `forceNormalizeWeights` | `fnw` | With -e: normalize the selected points now. |
| `smoothWeights` | `sw` | With -e: smooth the selected points' weights by this much (0 to 1). |
| `smoothWeightsMaxIterations` | `swi` | With -sw: passes (2). |
| `recacheBindMatrices` | `rbm` | With -e: the joints' current pose becomes the bind pose. |
| `edit` | `e` | Edit mode. |
| `query` | `q` | Query mode. |

## skinPaintStamp

One dab of the paint skin weights brush at a world point, from the tool's settings: for scripts and tests. -inv paints the inverse, as Ctrl does in the tool; -sm smooths, as Shift does.

Arguments: a skinned mesh

| Flag | Short | Description |
| --- | --- | --- |
| `position` | `p` | World centre of the dab. |
| `invert` | `inv` | Paint the inverse. |
| `smooth` | `sm` | Smooth instead. |

## skinPercent

Set or query skin weights of points: skinPercent -tv joint1 1 skinCluster1 pCube1.vtx[0]. -tv joint value (multi), -r relative, -zri zeroes the rest, -tmw from to moves weight, -prw prunes weights below a value, -nrm normalizes, -rtd resets to the default bind. Query: -v the weights, -t the influences or one influence's weight, -ib ignores small ones.

Arguments: skinCluster, then points or components (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `transformValue` | `tv` | joint weight pairs. |
| `dqBlendWeight` | `dqb` | The points' blend between linear and dual quaternion, for skinningMethod 2. |
| `transformMoveWeights` | `tmw` | from to pairs: moves the weight of one influence to another. |
| `relative` | `r` | -tv adds to the current weight. |
| `zeroRemainingInfluences` | `zri` | -tv zeroes the influences it doesn't name. |
| `pruneWeights` | `prw` | Removes weights below this value, normalizing what's left. |
| `normalize` | `nrm` | Normalizes the points. |
| `resetToDefault` | `rtd` | Back to the weights of a default bind. |
| `ignoreBelow` | `ib` | Query: skip influences below this value. |
| `transform` | `t` | Query: one influence's weight, or the list of influences. |
| `value` | `v` | Query: weights per influence. |
| `edit` | `e` | Edit mode. |
| `query` | `q` | Query mode. |

## smoothCurve

Smooth the CVs: -s how far, -i passes. Returns [curve, node].

Arguments: curves (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `smoothness` | `s` | How far each pass moves. |
| `iterations` | `i` | Passes. |
| `constructionHistory` | `ch` | Keep a history node (on by default). |
| `replaceOriginal` | `rpo` | Change the curve itself (on by default). |
| `name` | `n` | Name of a new curve. |
| `object` | `o` | Make an object (accepted for existing scripts). |

## smoothPreview

Display meshes subdivided without changing them (the 1/2/3 keys), including every mesh under a given group. level 0 is off.

Arguments: objects

| Flag | Short | Description |
| --- | --- | --- |
| `level` |  | Subdivision levels to display (0 to 3). |

## smoothTangentSurface

Line a surface's CVs up across the knot at -p so it's smooth there: -d 1 U, 0 V. Returns [surface, node].

Arguments: surfaces (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `parameter` | `p` | Which isoparm. |
| `direction` | `d` | 1 U, 0 V. |
| `smoothness` | `s` | Accepted for existing scripts. |
| `constructionHistory` | `ch` | Keep a history node (on by default). |
| `replaceOriginal` | `rpo` | Change the surface itself (on by default). |
| `name` | `n` | Name of a new surface. |
| `object` | `o` | Accepted for existing scripts. |
| `polygon` | `po` | Accepted for existing scripts. |

## snapKey

Move keys to whole multiples of -tm (1 by default), and to multiples of -vm if given.

Arguments: objects, plugs or curves (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `attribute` | `at` | Channel name(s). |
| `time` | `t` | A frame or range "10:20". |
| `float` | `f` | A range of driver values on driven key curves, as -t is for time. |
| `index` | `in` | Key index(es) on each curve, or a range "0:2". |
| `includeUpperBound` | `iub` | Whether a range's end counts (on by default). |
| `hierarchy` | `hi` | none, below, above or both. |
| `controlPoints` | `cp` | Accepted for existing scripts: control points have no channels of their own here. |
| `shape` | `s` | Include the channels of the objects' shapes. |
| `animation` | `an` | objects (the default), keys or keysOrObjects: keys means the selected keys. |
| `selected` | `sl` | Snap the keys selectKey selected. |
| `timeMultiple` | `tm` | Snap time to multiples of this. |
| `valueMultiple` | `vm` | Snap value to multiples of this. |

## snapMode

Move snapping switches: -grid, -point, -curve; -q to ask.

| Flag | Short | Description |
| --- | --- | --- |
| `grid` | `gr` | This snap. |
| `point` | `p` | This snap. |
| `curve` | `c` | This snap. |
| `liveFace` | `lf` | This snap. |
| `viewPlane` | `vp` | This snap. |
| `meshCenter` | `mc` | This snap. |
| `query` | `q` | Query. |

## snapTogetherCtx

A snapTogetherCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `clearSelection` | `cs` |  |
| `setOrientation` | `so` |  |
| `snapPolygonFace` | `spf` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## snapshot

Copies of objects at every -i frames from -st to -et, in a group. Returns the group and the snapshot node. With -mt, makes a motion trail instead.

Arguments: objects (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `startTime` | `st` | First frame (the playback start). |
| `endTime` | `et` | Last frame (the playback end). |
| `increment` | `i` | Frames between copies (1). |
| `constructionHistory` | `ch` | Accepted for existing scripts: copies are made once. |
| `c` | `c` | As -ch. |
| `name` | `n` | The snapshot's name. |
| `update` | `u` | always or animCurve: recorded on the node. |
| `motionTrail` | `mt` | Make a motion trail. |

## softMod

Add a soft modification to the selected vertices or meshes: softMod -fr 2 -fas true. Returns [softMod1, softMod1Handle]. Move the handle and points follow by their falloff. With vertices it falls off around them; with a mesh, around its centre or -fc. -e and -q on the softMod or handle: -fr, -fom, -fc, -fas, -rel, -en.

Arguments: vertices or meshes (the selection when none); the softMod with -e and -q

| Flag | Short | Description |
| --- | --- | --- |
| `name` | `n` | Name of the deformer. |
| `before` | `bf` | Go just above the deformer nearest the mesh. |
| `after` | `af` | Go nearest the mesh, after the other deformers (the default). |
| `frontOfChain` | `foc` | Go above every other deformer. |
| `envelope` | `en` | How much of the deformation applies (1). |
| `edit` | `e` | Edit mode. |
| `query` | `q` | Query mode. |
| `falloffRadius` | `fr` | How far the effect reaches, cm (5). |
| `falloffMode` | `fom` | 0 straight through space, 1 along the surface. |
| `falloffCenter` | `fc` | Centre of the falloff, world space. |
| `falloffAroundSelection` | `fas` | Fall off around the selected points rather than the centre (on). |
| `relative` | `rel` | Only the handle's own transform counts, not its parents'. |

## softModContext

A softModContext tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `falloffMode` | `fm` |  |
| `falloffRadius` | `fr` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## softSelect

Soft selection (B): -sse 1 turns it on, -ssd sets the falloff radius. -q -sse / -q -ssd query them.

| Flag | Short | Description |
| --- | --- | --- |
| `softSelectEnabled` | `sse` | 1 on, 0 off. |
| `softSelectDistance` | `ssd` | Falloff radius, world units. |
| `softSelectFalloff` | `ssf` | 0 volume, 1 surface, 2 global, 3 object. |
| `edit` | `e` | Edit mode. |
| `query` | `q` | Query mode. |

## softSelectCurve

Set or query the soft select falloff curve (weight against distance from the selection, 0 at the selection, 1 at the radius). With no flags it returns the current curve.

| Flag | Short | Description |
| --- | --- | --- |
| `keys` | `k` | Keys as [[pos, value, interp], ...]; interp is linear, smooth, spline or flat. |
| `preset` | `p` | Apply a built-in or saved preset by name. |
| `falloffMode` | `fm` | Distance measured through space or along the surface. |
| `save` |  | Save the current curve (after keys/preset) as a user preset with this name. |
| `remove` |  | Delete the named user preset. |
| `rename` |  | User preset to rename; the new name goes in to. |
| `to` |  | New name for rename. |

## softSelectOptionsCtx

A softSelectOptionsCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `buttonDown` | `btd` |  |
| `buttonUp` | `btu` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## solidify

Solidify: the mesh keeps its rest shape, fitted to what comes in. Returns [solidify1].

Arguments: what to deform, and any target last (the selection when none); the deformer with -e and -q

| Flag | Short | Description |
| --- | --- | --- |
| `name` | `n` | Name of the deformer. |
| `before` | `bf` | Go just above the deformer nearest the mesh. |
| `after` | `af` | Go nearest the mesh, after the other deformers (the default). |
| `frontOfChain` | `foc` | Go above every other deformer. |
| `envelope` | `en` | How much of the deformation applies (1). |
| `edit` | `e` | Edit mode. |
| `query` | `q` | Query mode. |
| `scaleEnvelope` | `se` | How much of the size change comes through (1). |

## spaceLocator

Create a locator: a transform with a locator shape under it. Returns [transform].

| Flag | Short | Description |
| --- | --- | --- |
| `name` | `n` | Name; made unique among its siblings. |
| `position` | `p` | Where the locator shape sits in its transform's space (localPosition). |
| `absolute` | `a` | The position is world, with the pivots at it. |
| `relative` | `r` | The position is local (the default). |

## sphere

Create a NURBS sphere with a makeNurbSphere node. Returns [nurbsSphere1, makeNurbSphere1].

| Flag | Short | Description |
| --- | --- | --- |
| `axis` | `ax` | The axis (1 0 0). |
| `pivot` | `p` | The centre. |
| `constructionHistory` | `ch` | Keep the make node (on by default). |
| `name` | `n` | Name of the transform. |
| `object` | `o` | Accepted for existing scripts. |
| `polygon` | `po` | 1 makes polygons. |
| `radius` | `r` | The radius (1). |
| `startSweep` | `ssw` | Start, degrees (0). |
| `endSweep` | `esw` | End, degrees (360). |
| `degree` | `d` | 1 linear, 3 cubic (3). |
| `sections` | `s` | Sections around (8). |
| `spans` | `nsp` | Spans along the axis. |
| `useTolerance` | `ut` | Accepted for existing scripts. |
| `tolerance` | `tol` | Accepted for existing scripts. |
| `heightRatio` | `hr` | Height over radius. |

## splitRingsApplySelection

Store the selected edges or faces (with their mirror images under symmetry) as what a splitRings modifier splits, and end Edit Selection. Returns how many it stored.

Arguments: modifier or mesh

## splitRingsCancelSelection

End a splitRings modifier's Edit Selection without changing what it splits.

Arguments: modifier or mesh

## splitRingsEditSelection

Show a splitRings modifier's input mesh (it and later modifiers pass through) with the edges or faces it splits selected, so they can be changed with the normal selection tools. Finish with splitRingsApplySelection or splitRingsCancelSelection.

Arguments: modifier or mesh

## spotLight

Create a spot light, or query and edit one. Returns its shape.

Arguments: the light, to edit or query

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `name` | `n` | Its name. |
| `rgb` | `rgb` | Colour. |
| `shadowColor` | `sc` | Shadow colour. |
| `position` | `pos` | Position. |
| `rotation` | `rot` | Rotation. |
| `exclusive` | `exc` | Out of defaultLightSet: lights only what it's linked to. |
| `shadowDither` | `sd` | Accepted for scripts. |
| `intensity` | `i` | intensity |
| `shadowSamples` | `sh` | shadowRays |
| `useRayTraceShadows` | `rs` | useRayTraceShadows |
| `decayRate` | `d` | decayRate |
| `softShadow` | `ss` | castSoftShadows |
| `discRadius` | `drs` | lightRadius |
| `coneAngle` | `ca` | coneAngle |
| `penumbra` | `p` | penumbraAngle |
| `dropOff` | `do` | dropoff |
| `barnDoors` | `bd` | barnDoors |
| `leftBarnDoorAngle` | `lbd` | leftBarnDoor |
| `rightBarnDoorAngle` | `rbd` | rightBarnDoor |
| `topBarnDoorAngle` | `tbd` | topBarnDoor |
| `bottomBarnDoorAngle` | `bbd` | bottomBarnDoor |

## spreadSheetEditor

Script UI: spreadSheetEditor, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `attrRegExp` | `are` |  |
| `execute` | `exe` |  |
| `fixedAttrList` | `fal` |  |
| `header` | `hdr` |  |
| `keyableOnly` | `ko` |  |
| `longNames` | `ln` |  |
| `niceNames` | `nn` |  |
| `precision` | `pre` |  |
| `selectedAttr` | `sla` |  |
| `showShapes` | `ssh` |  |
| `control` | `ctl` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `filter` | `f` |  |
| `forceMainConnection` | `fmc` |  |
| `highlightConnection` | `hlc` |  |
| `lockMainConnection` | `lck` |  |
| `mainListConnection` | `mlc` |  |
| `panel` | `pnl` |  |
| `selectionConnection` | `slc` |  |
| `stateString` | `sts` |  |
| `unParent` | `up` |  |
| `unlockMainConnection` | `ulk` |  |
| `updateMainConnection` | `upd` |  |
| `useTemplate` | `ut` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## square

A surface inside four curves, in order round the edge. Returns [squareSurface1, squareSrf1].

Arguments: four curves (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `degree` | `d` | Accepted for existing scripts. |
| `continuityType1` | `cp` | Accepted for existing scripts. |
| `endPointTolerance` | `ept` | Accepted for existing scripts. |
| `rebuildCurve1` | `rc1` | Accepted for existing scripts. |
| `constructionHistory` | `ch` | Keep the node (on by default). |
| `polygon` | `po` | 0 NURBS, 1 polygons. |
| `range` | `rn` | Accepted for existing scripts. |
| `name` | `n` | Name of the new object. |
| `object` | `o` | Accepted for existing scripts. |

## squareSurface

A surface from four curves in order round its edge: square under its other name.

Arguments: four curves (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `degree` | `d` | Accepted for existing scripts. |
| `continuityType1` | `cp` | Accepted for existing scripts. |
| `endPointTolerance` | `ept` | Accepted for existing scripts. |
| `rebuildCurve1` | `rc1` | Accepted for existing scripts. |
| `constructionHistory` | `ch` | Keep the node (on by default). |
| `polygon` | `po` | 0 NURBS, 1 polygons. |
| `range` | `rn` | Accepted for existing scripts. |
| `name` | `n` | Name of the new object. |
| `object` | `o` | Accepted for existing scripts. |

## stackTrace

Whether errors show the procedure stack: -state on or off, -q reads it. -dump writes the stack to the history.

| Flag | Short | Description |
| --- | --- | --- |
| `state` | `st` |  |
| `query` | `q` |  |
| `dump` | `d` |  |

## stitchSurface

Close the gap between two surfaces' edges (surface.u[x] or the nearest): -b 0.5 meets halfway, 0 moves only the second, 1 only the first. Returns the surfaces and nodes.

Arguments: two surfaces or edges

| Flag | Short | Description |
| --- | --- | --- |
| `bias` | `b` | Which moves (0.5). |
| `cascade` | `c` | Accepted for existing scripts. |
| `keepG0Continuity` | `kg0` | Accepted for existing scripts. |
| `keepG1Continuity` | `kg1` | Accepted for existing scripts. |
| `numberOfSamples` | `ns` | Accepted for existing scripts. |
| `stepCount` | `sc` | Accepted for existing scripts. |
| `fixBoundary` | `fb` | Accepted for existing scripts. |
| `replaceOriginal` | `rpo` | Accepted: the surfaces themselves change. |
| `constructionHistory` | `ch` | Keep the node (on by default). |
| `name` | `n` | The new object's name. |
| `object` | `o` | Accepted for existing scripts. |
| `range` | `rn` | Accepted for existing scripts. |
| `caching` | `cch` | Accepted for existing scripts. |
| `nodeState` | `nds` | Accepted for existing scripts. |
| `tolerance` | `tol` | Accepted for existing scripts. |
| `polygon` | `po` | 0 NURBS, 1 polygons. |

## stitchSurfacePoints

Move CVs to one place: their mean, or -ew 0 the first. Returns them.

Arguments: CVs

| Flag | Short | Description |
| --- | --- | --- |
| `equalWeight` | `ew` | Their mean (the default), or the first. |
| `bias` | `b` | Accepted for existing scripts. |
| `constructionHistory` | `ch` | Accepted for existing scripts. |
| `cascade` | `c` | Accepted for existing scripts. |
| `keepG0Continuity` | `kg0` | Accepted for existing scripts. |
| `keepG1Continuity` | `kg1` | Accepted for existing scripts. |
| `replaceOriginal` | `rpo` | Accepted: they change in place. |
| `stepCount` | `sc` | Accepted for existing scripts. |
| `tolerance` | `tol` | Accepted for existing scripts. |

## stringArrayIntersector

An object holding the intersection of string arrays so far: name it, -e -i array, -q reads it, -e -r starts again, -ex asks, -ad keeps duplicates. -dt and -ut are accepted and do nothing.

Arguments: its name

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `allowDuplicates` | `ad` | Keep duplicate entries. |
| `defineTemplate` | `dt` | Accepted for existing scripts; has no effect. |
| `exists` | `ex` | Whether it exists. |
| `intersect` | `i` | An array. |
| `reset` | `r` | Start again. |
| `useTemplate` | `ut` | Accepted for existing scripts; has no effect. |

## substituteGeometry

Substitute Geometry: the second mesh takes over from the skinned first, with its influences and weights. The old mesh goes unless -rog. Returns the new skinCluster.

Arguments: the skinned mesh, then its replacement (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `oldGeometry` | `og` | The skinned mesh. |
| `newGeometry` | `ng` | Its replacement. |
| `retainOldGeometry` | `rog` | Keep the old mesh. |

## surface

A surface from raw data: -du -dv degrees, -fu -fv open, closed or periodic, -ku -kv one knot each, -p x y z one point each (or -pw with a weight), V running fastest.

| Flag | Short | Description |
| --- | --- | --- |
| `degreeU` | `du` | Degree in U. |
| `degreeV` | `dv` | Degree in V. |
| `formU` | `fu` | open, closed or periodic. |
| `formV` | `fv` | open, closed or periodic. |
| `knotU` | `ku` | A knot in U. |
| `knotV` | `kv` | A knot in V. |
| `point` | `p` | A CV. |
| `pointWeight` | `pw` | A CV with a weight, dropped: surfaces aren't rational here. |
| `name` | `n` | The surface's name. |
| `objectSpace` | `ob` | Accepted for existing scripts. |
| `worldSpace` | `ws` | Accepted for existing scripts. |

## sweepMeshFromCurve

A tube along a curve with a sweepMeshCreator node: -scaleProfileX radius, -profilePolySides, -interpolationSteps, -taper, -twist. Returns [sweep1, sweepMeshCreator1].

Arguments: a curve (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `scaleProfileX` | `spx` | Radius of the profile. |
| `profilePolySides` | `pps` | Sides of the profile. |
| `interpolationSteps` | `is` | Rings per span. |
| `taper` | `tp` | Radius scale at the end. |
| `twist` | `tw` | Twist at the end, degrees. |
| `polygon` | `po` | 0 NURBS (the default), 1 polygons. |
| `constructionHistory` | `ch` | Keep the node (on by default). |
| `range` | `rn` | Accepted for existing scripts. |
| `name` | `n` | Name of the new object. |

## symbolButton

Script UI: symbolButton, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `image` | `i` |  |
| `command` | `c` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## symbolCheckBox

Script UI: symbolCheckBox, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `image` | `i` |  |
| `onImage` | `oni` |  |
| `offImage` | `ofi` |  |
| `value` | `v` |  |
| `onCommand` | `onc` |  |
| `offCommand` | `ofc` |  |
| `changeCommand` | `cc` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## symmetricModelling

Symmetry for component edits: -e -s on, -ax x|y|z, -ab world|object|topo, -ts on from the selected seam edge. On the selected meshes, or all. -q reads each.

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `symmetry` | `s` | On or off. |
| `axis` | `ax` | x, y or z. |
| `about` | `ab` | world, object or topo. All mirror in the object's axes here; topo pairs by topology. |
| `topoSymmetry` | `ts` | Pair by topology from the selected seam edge. |
| `worldSpace` | `ws` | About world. |
| `tolerance` | `tol` | How close a pair must be. |
| `reset` | `r` | Back to the defaults. |
| `allowPartial` | `ap` | Accepted for existing scripts. |
| `preserveSeam` | `ps` | 1 keeps points on the plane there; 0 lets them leave it. |
| `seamTolerance` | `st` | Accepted for existing scripts. |

## symmetrize

Make meshes exactly symmetric: every point on one side of the symmetry plane is copied, reflected, onto its partner, and on-plane points are put on the plane. With radial symmetry, one wedge's points are copied round the axis onto their orbits.

Arguments: objects

| Flag | Short | Description |
| --- | --- | --- |
| `side` |  | The side that is kept. |
| `wedge` |  | Radial: the wedge that is kept; -1: the one most of the selected components are in, else 0. |

## symmetrizeRadial

Rebuild meshes exactly radially symmetric: the kept wedge is repeated round the axis (and mirrored within each wedge with mirror=True), replacing the rest, and welded along the seams and the axis; then the mesh's symmetry is set to radial with these settings, so every edit after keeps it. Settings default to the mesh's own radial symmetry. Model one wedge and run this to get the whole. Returns the faces per mesh.

Arguments: objects

| Flag | Short | Description |
| --- | --- | --- |
| `count` |  | Radial: how many wedges repeat round the axis, 2 to 64; default: the mesh's own. |
| `radialAxis` |  | Radial: the axis, x, y, z or custom (direction=); default: the mesh's own. |
| `direction` |  | Radial: a custom axis direction, object space. |
| `mirror` |  | Radial: also mirror each wedge across a plane through the axis (dihedral symmetry). |
| `phase` |  | Radial: degrees the wedge boundaries and mirror planes turn about the axis. |
| `wedge` |  | The wedge that is kept; -1: the one most of the selected components are in, else the one holding most of the mesh. |
| `tolerance` |  | Weld distance along the seams and the axis, cm (0: scaled to the mesh). |

## symmetrizeSelection

Add the mirror images of the component selection, on each mesh whose symmetry is on.

## symmetryCheck

Report how symmetric meshes are across their symmetry plane (or round their radial axis): the largest distance between a point and its partner's reflection (or its orbit's image), and the points without a partner. For radial symmetry also the unmatched point ids and the count the mesh fits best. Changes nothing.

Arguments: objects

## symmetryRebuild

Rebuild meshes symmetric: the kept side is mirrored across the plane through the object's pivot, replacing the other side, welded on the plane, and the symmetry pairs are stored again. A mesh with radial symmetry is rebuilt from its kept wedge instead (see symmetrizeRadial). With pairsOnly, the geometry is left alone and only the stored pairs are matched again, by position or, for meshes that are no longer symmetric, by topology outward from the selected edge (which must lie on the plane; any edge for radial symmetry). Returns the points left unmatched per mesh.

Arguments: objects

| Flag | Short | Description |
| --- | --- | --- |
| `axis` |  | x, y or z; default: the mesh's symmetry axis, else x. |
| `side` |  | The side that is kept; auto: the side of the selected components, else the side holding nearly all of the mesh, else positive. |
| `wedge` |  | Radial: the wedge that is kept; -1: the one most of the selected components are in, else the one holding most of the mesh. |
| `pairsOnly` |  | Only match the stored pairs again, leaving the geometry as it is. |
| `topological` |  | With pairsOnly: match by topology from the selected edge. |
| `tolerance` |  | Weld or match distance, cm (0: scaled to the mesh). |

## sysFile

File operations on a path: -cp, -delete, -md, -mov, -red, -ren. Returns whether it worked. Not undoable, unlike what the reference's reference says.

Arguments: the path

| Flag | Short | Description |
| --- | --- | --- |
| `copy` | `cp` | Copy to this name. |
| `delete` | `delete` | Delete the file. |
| `makeDir` | `md` | Make the directory and those above it. |
| `move` | `mov` | As -rename. |
| `removeEmptyDir` | `red` | Remove the directory if empty. |
| `rename` | `ren` | Rename to this name. |

## tabLayout

Script UI: tabLayout, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `tabLabel` | `tl` |  |
| `tabLabelIndex` | `tli` |  |
| `selectTabIndex` | `sti` |  |
| `selectTab` | `st` |  |
| `changeCommand` | `cc` |  |
| `selectCommand` | `sc` |  |
| `preSelectCommand` | `psc` |  |
| `doubleClickCommand` | `dcc` |  |
| `tabsVisible` | `tv` |  |
| `innerMarginWidth` | `imw` |  |
| `innerMarginHeight` | `imh` |  |
| `scrollable` | `scr` |  |
| `childResizable` | `cr` |  |
| `horizontalScrollBarThickness` | `hst` |  |
| `verticalScrollBarThickness` | `vst` |  |
| `moveTab` | `mt` |  |
| `newTabCommand` | `ntc` |  |
| `showNewTab` | `snt` |  |
| `tabPosition` | `tp` |  |
| `tabTooltip` | `tt` |  |
| `tabsClosable` | `tc` |  |
| `closeTabCommand` | `ctc` |  |
| `minChildWidth` | `mcw` |  |
| `childArray` | `ca` |  |
| `numberOfChildren` | `nch` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## tangentConstraint

Constrain the object named (or selected) last to the targets before it; a second call adds targets. -mo keeps the current offset; -w sets the targets' weight; -e and -q work on the constraint or the object it drives. Returns [constraint].

Arguments: targets, then the object to drive or the constraint

| Flag | Short | Description |
| --- | --- | --- |
| `maintainOffset` | `mo` | Keep the object where it is now relative to the targets. |
| `weight` | `w` | Weight of the targets named (1). |
| `name` | `n` | Name of the constraint. |
| `offset` | `o` | Added to the result. |
| `remove` | `rm` | Remove the targets named. Removing the last deletes the constraint. |
| `targetList` | `tl` | Query: the targets. |
| `weightAliasList` | `wal` | Query: the targets' weight attributes. |
| `aimVector` | `aim` | The local axis that points at the target. |
| `upVector` | `u` | The local axis that points up. |
| `worldUpVector` | `wu` | The world direction up means. |
| `worldUpType` | `wut` | scene, object, objectrotation, vector or none. |
| `worldUpObject` | `wuo` | The object for worldUpType object and objectrotation. |
| `interpType` | `int` | How rotations blend: noflip, average, shortest, longest, cache, or 0 to 4. |
| `skipTranslate` | `st` | Translate axes to leave alone: x, y, z or none. May be used more than once. |
| `skipRotate` | `sr` | Rotate axes to leave alone: x, y, z or none. May be used more than once. |
| `skip` | `sk` | Axes of this constraint's channel to leave alone: x, y, z or none. May be used more than once. |
| `skipScale` |  | Scale axes to leave alone: x, y, z or none. |
| `edit` | `e` | Change the constraint. |
| `query` | `q` | Ask about the constraint. |

## targetWeldCtx

A targetWeldCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `mergeToCenter` | `mtc` |  |
| `preserveUV` | `puv` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## tension

Add a tension deformer: edges pull back towards their lengths in the mesh as it is now. Returns [tension1].

Arguments: meshes or vertices (the selection when none); the deformer with -e and -q

| Flag | Short | Description |
| --- | --- | --- |
| `name` | `n` | Name of the deformer. |
| `before` | `bf` | Go just above the deformer nearest the mesh. |
| `after` | `af` | Go nearest the mesh, after the other deformers (the default). |
| `frontOfChain` | `foc` | Go above every other deformer. |
| `envelope` | `en` | How much of the deformation applies (1). |
| `edit` | `e` | Edit mode. |
| `query` | `q` | Query mode. |
| `smoothingIterations` | `si` | Smoothing passes (10). |
| `smoothingStep` | `ss` | How far each pass moves a point towards its neighbours (0.5). |
| `pinBorderVertices` | `pbv` | Border points stay out of the smoothing (on). |
| `pin` | `pin` | The same as -pinBorderVertices. |
| `inwardConstraint` | `ic` | Stops points moving in along the normal: 0 free, 1 not at all. |
| `outwardConstraint` | `oc` | Stops points moving out along the normal: 0 free, 1 not at all. |
| `squashConstraint` | `sqc` | Stops squashed edges relaxing: 0 free, 1 not at all. |
| `stretchConstraint` | `stc` | Stops stretched edges relaxing: 0 free, 1 not at all. |

## texCutContext

A texCutContext tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `adjustSize` | `as` |  |
| `displayShellBorders` | `dsb` |  |
| `edgeSelectSensitive` | `ess` |  |
| `mode` | `m` |  |
| `moveRatio` | `mr` |  |
| `showTextureBorders` | `stb` |  |
| `size` | `sz` |  |
| `steadyStroke` | `ss` |  |
| `steadyStrokeDistance` | `ssd` |  |
| `touchToSew` | `tts` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## texLatticeDeformContext

A texLatticeDeformContext tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `envelope` | `ev` |  |
| `height` | `h` |  |
| `width` | `w` |  |
| `latticeColumns` | `lc` |  |
| `latticeRows` | `lr` |  |
| `snapPixelMode` | `spm` |  |
| `useBoundingRect` | `ubr` |  |
| `showMoveManipulator` | `smm` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## texMoveContext

A texMoveContext tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `snap` | `s` |  |
| `snapValue` | `sv` |  |
| `snapComponentsRelative` | `scr` |  |
| `position` | `p` |  |
| `editPivotMode` | `epm` |  |
| `editPivotPosition` | `epp` |  |
| `tweakMode` | `twk` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## texMoveUVShellContext

A texMoveUVShellContext tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `iterations` | `it` |  |
| `mask` | `m` |  |
| `position` | `p` |  |
| `shellBorder` | `sb` |  |
| `tolerance` | `to` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## texRotateContext

A texRotateContext tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `snap` | `s` |  |
| `snapValue` | `sv` |  |
| `snapComponentsRelative` | `scr` |  |
| `position` | `p` |  |
| `editPivotMode` | `epm` |  |
| `editPivotPosition` | `epp` |  |
| `tweakMode` | `twk` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## texScaleContext

A texScaleContext tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `snap` | `s` |  |
| `snapValue` | `sv` |  |
| `snapComponentsRelative` | `scr` |  |
| `position` | `p` |  |
| `editPivotMode` | `epm` |  |
| `editPivotPosition` | `epp` |  |
| `tweakMode` | `twk` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## texSelectContext

A texSelectContext tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## texSelectShortestPathCtx

A texSelectShortestPathCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## texSmoothContext

A texSmoothContext tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## texSmudgeUVContext

A texSmudgeUVContext tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `dragSlider` | `ds` |  |
| `effectType` | `et` |  |
| `functionType` | `ft` |  |
| `pressure` | `prs` |  |
| `radius` | `r` |  |
| `smudgeIsMiddle` | `sim` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |

## texTweakUVContext

A texTweakUVContext tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `position` | `p` |  |
| `tolerance` | `to` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## texWinToolCtx

A texWinToolCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `alternateContext` | `ac` |  |
| `boxzoom` | `bz` |  |
| `dolly` | `do` |  |
| `toolName` | `tn` |  |
| `track` | `tr` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## text

Script UI: text, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `label` | `l` |  |
| `align` | `al` |  |
| `font` | `fn` |  |
| `recomputeSize` | `rs` |  |
| `wordWrap` | `ww` |  |
| `hyperlink` | `hl` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## textCurves

Text as curves: -t words, -f font accepted (one stroke font). A group, a transform per letter, a curve per stroke. Returns [group, node].

| Flag | Short | Description |
| --- | --- | --- |
| `text` | `t` | The words. |
| `font` | `f` | Accepted: one stroke font here. |
| `name` | `n` | The group's name. |
| `object` | `o` | Accepted for existing scripts. |

## textField

Script UI: textField, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `text` | `tx` |  |
| `placeholderText` | `pht` |  |
| `editable` | `ed` |  |
| `changeCommand` | `cc` |  |
| `textChangedCommand` | `tcc` |  |
| `enterCommand` | `ec` |  |
| `alwaysInvokeEnterCommandOnReturn` | `aie` |  |
| `receiveFocusCommand` | `rfc` |  |
| `insertText` | `it` |  |
| `insertionPosition` | `ip` |  |
| `fileName` | `fi` |  |
| `font` | `fn` |  |
| `searchField` | `sf` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## textFieldButtonGrp

Script UI: textFieldButtonGrp, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `label` | `l` |  |
| `text` | `tx` |  |
| `placeholderText` | `pht` |  |
| `editable` | `ed` |  |
| `changeCommand` | `cc` |  |
| `textChangedCommand` | `tcc` |  |
| `forceChangeCommand` | `fcc` |  |
| `fileName` | `fi` |  |
| `insertText` | `it` |  |
| `insertionPosition` | `ip` |  |
| `buttonLabel` | `bl` |  |
| `buttonCommand` | `bc` |  |
| `enableButton` | `eb` |  |
| `columnWidth1` | `cw1` |  |
| `columnAlign1` | `cl1` |  |
| `columnAttach1` | `ct1` |  |
| `columnOffset1` | `co1` |  |
| `columnWidth2` | `cw2` |  |
| `columnAlign2` | `cl2` |  |
| `columnAttach2` | `ct2` |  |
| `columnOffset2` | `co2` |  |
| `adjustableColumn2` | `ad2` |  |
| `columnWidth3` | `cw3` |  |
| `columnAlign3` | `cl3` |  |
| `columnAttach3` | `ct3` |  |
| `columnOffset3` | `co3` |  |
| `adjustableColumn3` | `ad3` |  |
| `columnWidth4` | `cw4` |  |
| `columnAlign4` | `cl4` |  |
| `columnAttach4` | `ct4` |  |
| `columnOffset4` | `co4` |  |
| `adjustableColumn4` | `ad4` |  |
| `columnWidth5` | `cw5` |  |
| `columnAlign5` | `cl5` |  |
| `columnAttach5` | `ct5` |  |
| `columnOffset5` | `co5` |  |
| `adjustableColumn5` | `ad5` |  |
| `columnWidth6` | `cw6` |  |
| `columnAlign6` | `cl6` |  |
| `columnAttach6` | `ct6` |  |
| `columnOffset6` | `co6` |  |
| `adjustableColumn6` | `ad6` |  |
| `adjustableColumn` | `adj` |  |
| `columnWidth` | `cw` |  |
| `columnAlign` | `cal` |  |
| `columnAttach` | `cat` |  |
| `rowAttach` | `rat` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## textFieldGrp

Script UI: textFieldGrp, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `label` | `l` |  |
| `text` | `tx` |  |
| `placeholderText` | `pht` |  |
| `editable` | `ed` |  |
| `changeCommand` | `cc` |  |
| `textChangedCommand` | `tcc` |  |
| `forceChangeCommand` | `fcc` |  |
| `fileName` | `fi` |  |
| `insertText` | `it` |  |
| `insertionPosition` | `ip` |  |
| `columnWidth1` | `cw1` |  |
| `columnAlign1` | `cl1` |  |
| `columnAttach1` | `ct1` |  |
| `columnOffset1` | `co1` |  |
| `columnWidth2` | `cw2` |  |
| `columnAlign2` | `cl2` |  |
| `columnAttach2` | `ct2` |  |
| `columnOffset2` | `co2` |  |
| `adjustableColumn2` | `ad2` |  |
| `columnWidth3` | `cw3` |  |
| `columnAlign3` | `cl3` |  |
| `columnAttach3` | `ct3` |  |
| `columnOffset3` | `co3` |  |
| `adjustableColumn3` | `ad3` |  |
| `columnWidth4` | `cw4` |  |
| `columnAlign4` | `cl4` |  |
| `columnAttach4` | `ct4` |  |
| `columnOffset4` | `co4` |  |
| `adjustableColumn4` | `ad4` |  |
| `columnWidth5` | `cw5` |  |
| `columnAlign5` | `cl5` |  |
| `columnAttach5` | `ct5` |  |
| `columnOffset5` | `co5` |  |
| `adjustableColumn5` | `ad5` |  |
| `columnWidth6` | `cw6` |  |
| `columnAlign6` | `cl6` |  |
| `columnAttach6` | `ct6` |  |
| `columnOffset6` | `co6` |  |
| `adjustableColumn6` | `ad6` |  |
| `adjustableColumn` | `adj` |  |
| `columnWidth` | `cw` |  |
| `columnAlign` | `cal` |  |
| `columnAttach` | `cat` |  |
| `rowAttach` | `rat` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## textScrollList

Script UI: textScrollList, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `append` | `a` |  |
| `appendPosition` | `ap` |  |
| `removeItem` | `ri` |  |
| `removeIndexedItem` | `rii` |  |
| `removeAll` | `ra` |  |
| `selectItem` | `si` |  |
| `selectIndexedItem` | `sii` |  |
| `deselectAll` | `da` |  |
| `deselectItem` | `di` |  |
| `deselectIndexedItem` | `dii` |  |
| `allowMultiSelection` | `ams` |  |
| `numberOfRows` | `nr` |  |
| `allItems` | `ai` |  |
| `numberOfItems` | `ni` |  |
| `numberOfSelectedItems` | `nsi` |  |
| `showIndexedItem` | `shi` |  |
| `selectCommand` | `sc` |  |
| `doubleClickCommand` | `dcc` |  |
| `deleteKeyCommand` | `dkc` |  |
| `lineFont` | `lf` |  |
| `font` | `fn` |  |
| `allowAutomaticSelection` | `aas` |  |
| `uniqueTag` | `utg` |  |
| `selectUniqueTagItem` | `sut` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## textureDeformer

Texture Deformer: displaces points by a procedural value plus whatever is connected to .texture. Returns [textureDeformer1].

Arguments: what to deform, and any target last (the selection when none); the deformer with -e and -q

| Flag | Short | Description |
| --- | --- | --- |
| `name` | `n` | Name of the deformer. |
| `before` | `bf` | Go just above the deformer nearest the mesh. |
| `after` | `af` | Go nearest the mesh, after the other deformers (the default). |
| `frontOfChain` | `foc` | Go above every other deformer. |
| `envelope` | `en` | How much of the deformation applies (1). |
| `edit` | `e` | Edit mode. |
| `query` | `q` | Query mode. |
| `strength` | `str` | Multiplies the texture (1). |
| `offset` | `off` | Added to the displacement. |
| `direction` | `dir` | 0 normal, 1 vector. |
| `vectorStrength` | `vs` | With direction vector: on each axis. |
| `vectorOffset` | `vo` | With direction vector: added on each axis. |
| `noiseFrequency` | `nf` | How fine the procedural texture is (1). |
| `noiseAmplitude` | `na` | How strong the procedural texture is (1). |
| `pointSpace` | `ps` | 0 world, 1 local. |

## textureLassoContext

A textureLassoContext tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `drawClosed` | `dc` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## textureWindow

Script UI: textureWindow, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `capture` | `cpt` |  |
| `changeCommand` | `cc` |  |
| `checkerColor1` | `cc1` |  |
| `checkerColor2` | `cc2` |  |
| `checkerDensity` | `cd` |  |
| `clearImage` | `ci` |  |
| `cmEnabled` | `cme` |  |
| `displayAxes` | `dax` |  |
| `displayCheckered` | `dc` |  |
| `displayDistortion` | `dcd` |  |
| `displayDivisionLines` | `ddl` |  |
| `displayGridLines` | `dgl` |  |
| `displayImage` | `di` |  |
| `displayLabels` | `dl` |  |
| `displayPreselection` | `dpr` |  |
| `displaySolidMap` | `dsm` |  |
| `displayStyle` | `dst` |  |
| `displayUVShaded` | `dus` |  |
| `displayUVWireframe` | `duw` |  |
| `distortionAlpha` | `dal` |  |
| `distortionPerObject` | `dpo` |  |
| `divisions` | `d` |  |
| `doubleBuffer` | `dbf` |  |
| `drawAxis` | `da` |  |
| `drawSubregions` | `dsr` |  |
| `exposure` | `exp` |  |
| `frameAll` | `fa` |  |
| `frameSelected` | `fs` |  |
| `frontFacingColor` | `ffc` |  |
| `gamma` | `ga` |  |
| `gridLinesColor` | `glc` |  |
| `gridNumbersColor` | `gnc` |  |
| `imageBaseColor` | `ibc` |  |
| `imageDim` | `imd` |  |
| `imageDisplay` | `id` |  |
| `imageNames` | `imn` |  |
| `imageNumber` | `in` |  |
| `imagePixelSnap` | `ip` |  |
| `imageRatio` | `imr` |  |
| `imageRatioValue` | `irv` |  |
| `imageSize` | `is` |  |
| `imageTileRange` | `itr` |  |
| `imageUnfiltered` | `iuf` |  |
| `internalFaces` | `if` |  |
| `labelPosition` | `lp` |  |
| `loadImage` | `li` |  |
| `maxResolution` | `mrs` |  |
| `multiColorAlpha` | `mca` |  |
| `nbImages` | `nim` |  |
| `nextView` | `nv` |  |
| `numUvSets` | `nuv` |  |
| `numberOfImages` | `ni` |  |
| `numberOfTextures` | `nt` |  |
| `previousView` | `pv` |  |
| `realSize` | `rs` |  |
| `refresh` | `rf` |  |
| `relativeHandleSize` | `rhs` |  |
| `removeAllImages` | `rai` |  |
| `removeImage` | `ri` |  |
| `rendererString` | `rst` |  |
| `reset` | `r` |  |
| `saveImage` | `si` |  |
| `selectInternalFaces` | `sif` |  |
| `selectRelated` | `sr` |  |
| `setUvSet` | `suv` |  |
| `singleBuffer` | `sbf` |  |
| `size` | `s` |  |
| `solidMap3dView` | `sm3` |  |
| `solidMapColorSeed` | `smc` |  |
| `solidMapPerShell` | `smp` |  |
| `spacing` | `sp` |  |
| `style` | `st` |  |
| `subdivisionLinesColor` | `slc` |  |
| `textureBorder` | `tb` |  |
| `textureBorderColor` | `tbc` |  |
| `textureBorderWidth` | `tbw` |  |
| `textureNames` | `tn` |  |
| `textureNumber` | `tx` |  |
| `tileLabels` | `tlb` |  |
| `tileLinesColor` | `tlc` |  |
| `toggle` | `tgl` |  |
| `toggleExposure` | `txp` |  |
| `toggleGamma` | `tgm` |  |
| `useFaceGroup` | `ufg` |  |
| `usedPercentageHUD` | `upp` |  |
| `uvSets` | `uvs` |  |
| `viewPortImage` | `vpi` |  |
| `viewTransformName` | `vtn` |  |
| `wireframeComponentColor` | `wcc` |  |
| `wireframeObjectColor` | `woc` |  |
| `writeImage` | `wi` |  |
| `control` | `ctl` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `filter` | `f` |  |
| `forceMainConnection` | `fmc` |  |
| `highlightConnection` | `hlc` |  |
| `lockMainConnection` | `lck` |  |
| `mainListConnection` | `mlc` |  |
| `panel` | `pnl` |  |
| `selectionConnection` | `slc` |  |
| `stateString` | `sts` |  |
| `unParent` | `up` |  |
| `unlockMainConnection` | `ulk` |  |
| `updateMainConnection` | `upd` |  |
| `useTemplate` | `ut` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## threadCount

Threads to evaluate with, kept for scripts: -n count, 0 for the machine's; -q -n asks.

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `numberOfThreads` | `n` | How many. |

## threePointArc

A circular arc through -p1 -p2 -p3: -d degree, -s sections. Returns [curve, node]. -q -c, -q -r on the node read centre and radius.

Arguments: the node, with -q

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `point1` | `p1` | Start. |
| `point2` | `p2` | Middle. |
| `point3` | `p3` | End. |
| `degree` | `d` | The degree (3). |
| `sections` | `s` | The sections (8). |
| `center` | `c` | With -q: the centre. |
| `normal` | `nr` | With -q: the normal. |
| `constructionHistory` | `ch` | Keep the node (on by default). |
| `name` | `n` | The curve's name. |
| `object` | `o` | Accepted for existing scripts. |
| `spans` | `sp` | Accepted for existing scripts. |
| `radius` | `r` | With -q: the radius. |

## threePointArcCtx

A threePointArcCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `degree` | `d` |  |
| `spans` | `s` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## tickDrawSpecial

Draw keys' ticks in the special colour on the time slider: -sp on or off (on), -t which keys.

Arguments: objects or plugs (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `special` | `sp` | On or off. |
| `time` | `t` | Which keys. |

## timeCode

Held for scripts: the file's time code, -msf, -psf, -psh, -psm, -pss, read back with -q. Less than the reference: the numbers are in the session, not on a time1 node, so they are not saved, and nothing shows them.

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `mayaStartFrame` | `msf` | The start time, in frames. |
| `productionStartFrame` | `psf` | Production start: frames. |
| `productionStartHour` | `psh` | Production start: hours. |
| `productionStartMinute` | `psm` | Production start: minutes. |
| `productionStartSecond` | `pss` | Production start: seconds. |

## timeControl

The time slider's state: the range selected on it (-q -ra, -q -rng, -q -rv), its key display and snapping.

Arguments: the slider's name, accepted for existing scripts

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Return the value of the flags named instead of setting them. |
| `edit` | `e` | Change the flags named. |
| `rangeArray` | `ra` | The selected range as start and end; empty for none. |
| `range` | `rng` | Query: the selected range as a string. |
| `rangeVisible` | `rv` | Query: whether a range is selected. |
| `showKeys` | `sk` | Whose keys show: active, none or a selection connection. |
| `snap` | `snp` | Snap to whole frames. |
| `tickSize` | `ts` | How wide ticks are drawn. |
| `minTime` | `min` | Query: the playback start. |
| `maxTime` | `max` | Query: the playback end. |
| `beginScrub` | `bs` | Accepted for existing scripts. |
| `endScrub` | `es` | Accepted for existing scripts. |
| `forceRedraw` | `fr` | Accepted for existing scripts. |

## timePort

Script UI: timePort, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `globalTime` | `gt` |  |
| `snap` | `sn` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## timeRangeControl

Script UI: timeRangeControl, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `minRange` | `min` |  |
| `maxRange` | `max` |  |
| `changedCommand` | `cc` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## timeWarp

Warp time for animation curves: names objects or curves, -f the frames to warp at (the playback range by default), -g every curve. With -e: -mf index frame moves one, -df index removes one. -q -f reads them. Returns the node.

Arguments: objects or curves, or the node with -e

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `frame` | `f` | Frames to warp at, or with -q ask. |
| `moveFrame` | `mf` | A warp frame's index and where it goes. |
| `deleteFrame` | `df` | A warp frame's index to remove. |
| `global` | `g` | Warp every curve in the scene. |
| `interpType` | `it` | Accepted: warps interpolate linearly here. |
| `name` | `n` | The node's name. |

## timer

A stopwatch: -s starts, -lap gives seconds so far, -e ends and gives seconds. -n names one.

| Flag | Short | Description |
| --- | --- | --- |
| `startTimer` | `s` |  |
| `endTimer` | `e` |  |
| `lapTime` | `lap` |  |
| `name` | `n` |  |

## toggle

Turns display switches on the objects on and off: -la, -rp, -sp, -sh, -te; -st for a value, -q to ask.

Arguments: objects

| Flag | Short | Description |
| --- | --- | --- |
| `localAxis` | `la` | What to switch. |
| `rotatePivot` | `rp` | What to switch. |
| `scalePivot` | `sp` | What to switch. |
| `selectHandle` | `sh` | What to switch. |
| `template` | `te` | What to switch. |
| `state` | `st` | On or off, rather than the other way round. |
| `query` | `q` | Query. |

## toggleAxis

The display axes: -v the one at the bottom left of each view, which is the viewAxis heads up display; -o the one at the origin, which is only held. -q reads them.

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `origin` | `o` | The axis at the origin. |
| `view` | `v` | The axis at the bottom left. |

## toggleSymmetry

Symmetry off on the meshes if every one already has it on (on axis=, when given), else on for all of them. Non-meshes are skipped. Returns the axis set, or 'off'.

Arguments: objects

| Flag | Short | Description |
| --- | --- | --- |
| `axis` |  | x, y, z or radial to turn on with; default: the axis already in use, else the last one picked. |

## toggleVisibility

Toggle visibility (the H key). Defaults to the selection. Every object ends up the same: hidden if most were visible (a tie hides), shown otherwise. Returns the new visibility.

Arguments: objects

## toolButton

Script UI: toolButton, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `tool` | `t` |  |
| `allowMultipleTools` | `amt` |  |
| `collection` | `cl` |  |
| `select` | `sl` |  |
| `changeCommand` | `cc` |  |
| `onCommand` | `onc` |  |
| `offCommand` | `ofc` |  |
| `doubleClickCommand` | `dcc` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `imageOverlayLabel` | `iol` |  |
| `style` | `st` |  |
| `popupIndicatorVisible` | `pin` |  |
| `toolArray` | `ta` |  |
| `toolCount` | `tc` |  |
| `version` | `ver` |  |
| `ltVersion` | `lt` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## toolCollection

A set of radio items, one selected.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `select` | `sl` |  |
| `collectionItemArray` | `cia` |  |
| `numberOfCollectionItems` | `nci` |  |
| `global` | `gl` |  |
| `parent` | `p` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `defineTemplate` | `dt` |  |
| `useTemplate` | `ut` |  |

## toolHasOptions

Whether a tool context has settings to show.

Arguments: context

## torus

Create a NURBS torus with a makeNurbTorus node. Returns [nurbsTorus1, makeNurbTorus1].

| Flag | Short | Description |
| --- | --- | --- |
| `axis` | `ax` | The axis (1 0 0). |
| `pivot` | `p` | The centre. |
| `constructionHistory` | `ch` | Keep the make node (on by default). |
| `name` | `n` | Name of the transform. |
| `object` | `o` | Accepted for existing scripts. |
| `polygon` | `po` | 1 makes polygons. |
| `radius` | `r` | The radius (1). |
| `startSweep` | `ssw` | Start, degrees (0). |
| `endSweep` | `esw` | End, degrees (360). |
| `degree` | `d` | 1 linear, 3 cubic (3). |
| `sections` | `s` | Sections around (8). |
| `spans` | `nsp` | Spans along the axis. |
| `useTolerance` | `ut` | Accepted for existing scripts. |
| `tolerance` | `tol` | Accepted for existing scripts. |
| `heightRatio` | `hr` | Height over radius. |
| `minorSweep` | `msw` | Degrees of the tube's section (360). |

## track

Slide a camera sideways: -l left, -r right, -u up, -d down, in units.

Arguments: [camera]

| Flag | Short | Description |
| --- | --- | --- |
| `left` | `l` |  |
| `right` | `r` |  |
| `up` | `u` |  |
| `down` | `d` |  |

## trackCtx

A trackCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `trackScale` | `ts` |  |
| `trackGeometry` | `tg` |  |
| `alternateContext` | `ac` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## transferAttributes

Copy attributes from the first mesh to the others: -pos positions, -nml normals, -uvs UVs, -col colours, 1 or 2 on; -spa 0 world, 1 model, 3 UV, 4 component, 5 topology; -sm 0 along the normal, 3 nearest. Live: changing the source changes the target. Returns the nodes.

Arguments: components or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `after` | `af` | See the command reference. |
| `afterReference` | `ar` | See the command reference. |
| `before` | `bf` | See the command reference. |
| `colorBorders` | `clb` | See the command reference. |
| `components` | `cmp` | See the command reference. |
| `deformerTools` | `dt` | See the command reference. |
| `exclusive` | `ex` | See the command reference. |
| `flipUVs` | `fuv` | See the command reference. |
| `frontOfChain` | `foc` | See the command reference. |
| `geometry` | `g` | See the command reference. |
| `geometryIndices` | `gi` | See the command reference. |
| `ignoreSelected` | `ignoreSelected` | See the command reference. |
| `includeHiddenSelections` | `ihs` | See the command reference. |
| `matchChoice` | `mch` | See the command reference. |
| `parallel` | `par` | See the command reference. |
| `prune` | `pr` | See the command reference. |
| `remove` | `rm` | See the command reference. |
| `sampleSpace` | `spa` | See the command reference. |
| `searchMethod` | `sm` | See the command reference. |
| `searchScaleX` | `ssx` | See the command reference. |
| `searchScaleY` | `ssy` | See the command reference. |
| `searchScaleZ` | `ssz` | See the command reference. |
| `selectedComponents` | `cms` | See the command reference. |
| `sourceColorSet` | `scs` | See the command reference. |
| `sourceUvSet` | `suv` | See the command reference. |
| `sourceUvSpace` | `sus` | See the command reference. |
| `split` | `sp` | See the command reference. |
| `targetColorSet` | `tcs` | See the command reference. |
| `targetUvSet` | `tuv` | See the command reference. |
| `targetUvSpace` | `tus` | See the command reference. |
| `transferColors` | `col` | See the command reference. |
| `transferNormals` | `nml` | See the command reference. |
| `transferPositions` | `pos` | See the command reference. |
| `transferUVs` | `uvs` | See the command reference. |
| `useComponentTags` | `uct` | See the command reference. |
| `constructionHistory` | `ch` | Accepted; see the command reference. |
| `name` | `n` | Accepted; see the command reference. |
| `caching` | `cch` | Accepted; see the command reference. |
| `nodeState` | `nds` | Accepted; see the command reference. |
| `frozen` | `fzn` | Accepted; see the command reference. |

## transferBetweenInfluences

Moves weight from one influence to another on the points named or selected: transferBetweenInfluences skinCluster1 -src joint1 -dst joint2 -v 0.5.

Arguments: the skinCluster, then points

| Flag | Short | Description |
| --- | --- | --- |
| `source` | `src` | The influence giving weight. |
| `destination` | `dst` | The influence taking it. |
| `value` | `v` | How much of it moves (1). |

## transformCompare

0 when two transforms match, with what's under them, 1 otherwise. -r the root alone.

Arguments: two transforms

| Flag | Short | Description |
| --- | --- | --- |
| `root` | `r` | Only the root. |

## transformLimits

Set or query the limits of translate, rotate and scale: -tx lo hi, -etx onLo onHi.

Arguments: objects

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query. |
| `edit` | `e` | Edit. |
| `remove` | `rm` | Turn every limit off. |
| `translation` | `t` | Low and high values, or whether each is on. |
| `enableTranslation` | `et` | Low and high values, or whether each is on. |
| `translationX` | `tx` | Low and high values, or whether each is on. |
| `enableTranslationX` | `etx` | Low and high values, or whether each is on. |
| `translationY` | `ty` | Low and high values, or whether each is on. |
| `enableTranslationY` | `ety` | Low and high values, or whether each is on. |
| `translationZ` | `tz` | Low and high values, or whether each is on. |
| `enableTranslationZ` | `etz` | Low and high values, or whether each is on. |
| `rotation` | `r` | Low and high values, or whether each is on. |
| `enableRotation` | `er` | Low and high values, or whether each is on. |
| `rotationX` | `rx` | Low and high values, or whether each is on. |
| `enableRotationX` | `erx` | Low and high values, or whether each is on. |
| `rotationY` | `ry` | Low and high values, or whether each is on. |
| `enableRotationY` | `ery` | Low and high values, or whether each is on. |
| `rotationZ` | `rz` | Low and high values, or whether each is on. |
| `enableRotationZ` | `erz` | Low and high values, or whether each is on. |
| `scale` | `s` | Low and high values, or whether each is on. |
| `enableScale` | `es` | Low and high values, or whether each is on. |
| `scaleX` | `sx` | Low and high values, or whether each is on. |
| `enableScaleX` | `esx` | Low and high values, or whether each is on. |
| `scaleY` | `sy` | Low and high values, or whether each is on. |
| `enableScaleY` | `esy` | Low and high values, or whether each is on. |
| `scaleZ` | `sz` | Low and high values, or whether each is on. |
| `enableScaleZ` | `esz` | Low and high values, or whether each is on. |

## trim

Trim a surface along closed curves on it: the region holding -lu -lv (0 to 1, the middle by default) stays, or with -sl 1 goes. Returns [surface, node].

Arguments: a surface, then curves on it

| Flag | Short | Description |
| --- | --- | --- |
| `locatorU` | `lu` | Where, in U (0.5). |
| `locatorV` | `lv` | Where, in V (0.5). |
| `selected` | `sl` | 0 keeps that region, 1 throws it away. |
| `shrink` | `sh` | Accepted for existing scripts. |
| `constructionHistory` | `ch` | Keep the node (on by default). |
| `name` | `n` | The new object's name. |
| `object` | `o` | Accepted for existing scripts. |
| `range` | `rn` | Accepted for existing scripts. |
| `caching` | `cch` | Accepted for existing scripts. |
| `nodeState` | `nds` | Accepted for existing scripts. |
| `tolerance` | `tol` | Accepted for existing scripts. |
| `polygon` | `po` | 0 NURBS, 1 polygons. |

## tumble

Turn a camera about a point: -aa azimuth, -ea elevation, -ra both, -pp the point.

Arguments: [camera]

| Flag | Short | Description |
| --- | --- | --- |
| `azimuthAngle` | `aa` |  |
| `elevationAngle` | `ea` |  |
| `localTumble` | `lt` |  |
| `pivotPoint` | `pp` |  |
| `rotationAngles` | `ra` |  |

## tumbleCtx

A tumbleCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `autoOrthoConstrain` | `aoc` |  |
| `autoSetPivot` | `asp` |  |
| `localTumble` | `lt` |  |
| `objectTumble` | `ot` |  |
| `orthoLock` | `ol` |  |
| `orthoStep` | `os` |  |
| `tumbleScale` | `ts` |  |
| `alternateContext` | `ac` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## turbulence

Make a turbulence field acting on the selected bodies, or -q/-e one. Returns [field].

Arguments: bodies to act on

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query a value. |
| `edit` | `e` | Edit an existing node. |
| `name` | `n` | Name of the new node. |
| `magnitude` | `m` | Strength. |
| `attenuation` | `att` | How fast it weakens with distance. |
| `maxDistance` | `mxd` | How far it reaches (turns the limit on). |
| `useMaxDistance` | `umd` | Limit its reach. |
| `directionX` | `dx` | Direction X. |
| `directionY` | `dy` | Direction Y. |
| `directionZ` | `dz` | Direction Z. |
| `speed` | `s` | Air: wind speed. |
| `inheritVelocity` | `iv` | Air: kept motion. |
| `frequency` | `f` | Turbulence: noise frequency. |
| `minDistance` | `mnd` | Newton: least distance. |
| `position` | `pos` | Where it is. |

## twoPointArc

A circular arc between -p1 and -p2 with radius -r on the side -dv's normal gives: -tac the long way round, -d degree, -s sections. Returns [curve, node].

| Flag | Short | Description |
| --- | --- | --- |
| `point1` | `p1` | Start. |
| `point2` | `p2` | End. |
| `directionVector` | `dv` | The arc's normal. |
| `radius` | `r` | The radius (1). |
| `toggleArc` | `tac` | The long way round. |
| `degree` | `d` | The degree (3). |
| `sections` | `s` | The sections (8). |
| `constructionHistory` | `ch` | Keep the node (on by default). |
| `name` | `n` | The curve's name. |
| `object` | `o` | Accepted for existing scripts. |

## twoPointArcCtx

A twoPointArcCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `degree` | `d` |  |
| `spans` | `s` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## u3dAutoSeam

Cut seams where faces meet sharply: -s 0 to 1 how readily (90 to 30 degrees). Returns the node.

Arguments: UVs, faces or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `constructionHistory` | `ch` | Accepted: a node is made while history is on. |
| `name` | `n` | Accepted for existing scripts. |
| `caching` | `cch` | Accepted for existing scripts. |
| `nodeState` | `nds` | Accepted for existing scripts. |
| `splitShells` | `s` | How readily. |
| `cutPipes` | `p` | Accepted for existing scripts. |

## u3dLayout

Layout: pack the UV shells of the selection into 0 to 1, each scaled to its size in 3D and turned to fit.

Arguments: faces, UVs or meshes

| Flag | Short | Description |
| --- | --- | --- |
| `shellSpacing` | `spc` | Space between shells, in UV units. |
| `rotateStep` | `rst` | 0 keeps shells unturned. |
| `resolution` | `res` | Accepted for existing scripts. |

## u3dOptimize

Relax UVs, reducing distortion: each shell flattened again, keeping its place and size. Returns the node.

Arguments: UVs, faces or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `constructionHistory` | `ch` | Accepted: a node is made while history is on. |
| `name` | `n` | Accepted for existing scripts. |
| `caching` | `cch` | Accepted for existing scripts. |
| `nodeState` | `nds` | Accepted for existing scripts. |
| `iterations` | `ite` | Accepted: the solver decides. |
| `power` | `pow` | Accepted for existing scripts. |
| `surfangle` | `sa` | Accepted for existing scripts. |
| `borderintersection` | `bi` | Accepted for existing scripts. |
| `triangleflip` | `tf` | Accepted for existing scripts. |
| `mapsize` | `ms` | Accepted for existing scripts. |
| `roomspace` | `rs` | Accepted for existing scripts. |

## u3dUnfold

Unfold: flatten the UV shells of the selection with the least stretch, keeping each shell's place and size.

Arguments: faces, UVs or meshes

| Flag | Short | Description |
| --- | --- | --- |
| `iterations` | `ite` | Accepted for existing scripts. |
| `pack` | `p` | Accepted for existing scripts. |
| `borderintersection` | `bi` | Accepted for existing scripts. |
| `triangleflip` | `tf` | Accepted for existing scripts. |

## uiPref

An interface setting the views read: uiPref name value. No value returns it.

Arguments: name value

## uiTemplate

A named set of flag defaults, filled by the -defineTemplate flag of each control command.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `exists` | `ex` |  |
| `defineTemplate` | `dt` |  |
| `useTemplate` | `ut` |  |

## undo

Undo the last command.

## undoInfo

Query or control the undo queue. openChunk/closeChunk group everything in between into one undo step.

| Flag | Short | Description |
| --- | --- | --- |
| `openChunk` | `ock` | Start a chunk. |
| `closeChunk` | `cck` | End a chunk. |
| `chunkName` | `cn` | Label for openChunk. |
| `limit` | `l` | Max undo steps, 0 for infinite. |
| `memoryBudget` | `mb` | Drop the oldest steps past this many MB, 0 for unlimited. |
| `history` | `h` | Also return every step's label, oldest first, and the current position. |
| `query` | `q` | Query. |
| `state` | `st` | Turn undo on or off; off flushes the queue, as in the reference. |
| `stateWithoutFlush` | `swf` | Turn undo on or off, keeping the queue. |
| `undoName` | `un` | With -q: what undo would undo. |
| `redoName` | `rn` | With -q: what redo would redo. |
| `undoQueueEmpty` | `uqe` | With -q: whether there is nothing to undo. |
| `redoQueueEmpty` | `rqe` | With -q: whether there is nothing to redo. |
| `length` | `l` | How many steps to keep. |
| `infinity` | `infinity` | Keep every step. |
| `printQueue` | `pq` | Accepted for existing scripts. |

## undoJump

Undo or redo in one go until the given number of undo steps remain, as listed by undoInfo(history=True).

| Flag | Short | Description |
| --- | --- | --- |
| `position` | `p` | Undo steps left afterwards: 0 undoes everything. |

## unfrozenTransforms

List the objects whose translate, rotate or scale is not identity (0, 0, 0 and 1, 1, 1), with their values. Checks the objects given or selected, or with scene every mesh, curve and group (joints, cameras, lights and locators are placed by their transforms and are left out). With rotateStep, rotations that are whole multiples of the step count as frozen; invert lists only rotations on the step instead. Returns one {name, translate, rotate, scale, unfrozen} per object listed, unfrozen naming the channels that put it there.

Arguments: objects

| Flag | Short | Description |
| --- | --- | --- |
| `scene` |  | Check the whole scene instead of the objects. |
| `hierarchy` |  | Also check everything parented under the objects. |
| `translate` |  | List objects with translation. |
| `rotate` |  | List objects with rotation. |
| `scale` |  | List objects with scale. |
| `rotateStep` |  | Degrees: rotations on whole multiples of it count as frozen. 0 is off. |
| `invert` |  | With rotateStep, list rotations on the step rather than off it. |
| `tolerance` |  | How far from identity, or from the step, still counts as on it. |

## ungroup

Move each group's children up to the group's parent, keeping where they are, then delete the group. Defaults to the selection; returns the children.

Arguments: groups

| Flag | Short | Description |
| --- | --- | --- |
| `world` | `w` | Move the children to the world. |
| `parent` | `p` | Move the children here. |
| `absolute` | `a` | Keep world placement (the default). |
| `relative` | `r` | Keep local placement. |

## uniform

Make a uniform field acting on the selected bodies, or -q/-e one. Returns [field].

Arguments: bodies to act on

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query a value. |
| `edit` | `e` | Edit an existing node. |
| `name` | `n` | Name of the new node. |
| `magnitude` | `m` | Strength. |
| `attenuation` | `att` | How fast it weakens with distance. |
| `maxDistance` | `mxd` | How far it reaches (turns the limit on). |
| `useMaxDistance` | `umd` | Limit its reach. |
| `directionX` | `dx` | Direction X. |
| `directionY` | `dy` | Direction Y. |
| `directionZ` | `dz` | Direction Z. |
| `speed` | `s` | Air: wind speed. |
| `inheritVelocity` | `iv` | Air: kept motion. |
| `frequency` | `f` | Turbulence: noise frequency. |
| `minDistance` | `mnd` | Newton: least distance. |
| `position` | `pos` | Where it is. |

## unloadPlugin

Unload a plug-in: its uninitializePlugin runs and its commands and node types go. A compiled one in use by the scene needs -force.

Arguments: plug-in name

| Flag | Short | Description |
| --- | --- | --- |
| `force` | `f` | Unload even with its nodes in the scene. |

## untangleUV

Untangle a shell: -mb circular or square maps its border to that shape, the inside evened so nothing folds. Returns the node.

Arguments: UVs, faces or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `constructionHistory` | `ch` | Accepted: a node is made while history is on. |
| `name` | `n` | Accepted for existing scripts. |
| `caching` | `cch` | Accepted for existing scripts. |
| `nodeState` | `nds` | Accepted for existing scripts. |
| `mapBorder` | `mb` | circular, square or none. |
| `pinSelected` | `ps` | Hold the UVs named. |
| `pinBorder` | `pb` | Accepted: the border holds unless -mb moves it. |
| `pinUnselected` | `pu` | Accepted for existing scripts. |
| `relax` | `r` | Accepted: the inside is evened uniformly. |
| `maxRelaxIterations` | `mri` | Accepted for existing scripts. |
| `relaxTolerance` | `rt` | Accepted for existing scripts. |
| `shapeDetail` | `sd` | Accepted for existing scripts. |

## untrim

Take a surface's trims off. Returns [surface, node].

Arguments: surfaces (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `curveOnSurface` | `cos` | Accepted for existing scripts. |
| `noChanges` | `nc` | Accepted for existing scripts. |
| `constructionHistory` | `ch` | Keep a history node (on by default). |
| `replaceOriginal` | `rpo` | Change the surface itself (on by default). |
| `name` | `n` | Name of a new surface. |
| `object` | `o` | Accepted for existing scripts. |
| `polygon` | `po` | Accepted for existing scripts. |

## upAxis

The world's up axis, a preference kept between scenes: -ax y or z; -rv turns the cameras with it so the ground stays put on screen. -q -ax asks.

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `axis` | `ax` | y or z. |
| `rotateView` | `rv` | Turn the default cameras too. |

## usdAttr

Read or author an attribute named proxy,/path.attr. A sampled attribute takes a sample at the current time; -key makes one. -q reads at the current time, or -time; -timeSamples lists the times.

Arguments: proxy,/path.attr

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Read. |
| `value` | `v` | The value. |
| `type` | `typ` | The type, for a new attribute. |
| `key` | `k` | Author a sample at the current time. |
| `time` | `t` | With -q: the time to read at. |
| `timeSamples` | `ts` | With -q: the times it's sampled at. |

## usdCamera

A camera following a Camera prim: its transform and lens read from the stage at the current time. -lookThrough makes the view use it. Returns the transform.

Arguments: proxy,/path

| Flag | Short | Description |
| --- | --- | --- |
| `lookThrough` | `lt` | Look through it. |

## usdCollections

A prim's collections as JSON: name, rule, members.

Arguments: proxy,/path

## usdDriveFromNode

A transform drives a prim live, through the session layer: usdDriveFromNode proxy,/path -driver node. -remove stops it.

Arguments: proxy,/path

| Flag | Short | Description |
| --- | --- | --- |
| `driver` | `d` | The driving transform. |
| `remove` | `rm` | Stop driving. |

## usdLayer

Layer work the reference's USD plug-in does in its editor: -q gives the layer tree as JSON; -save, -saveAs, -reload, -flattenTo, -saveDirty.

Arguments: layer identifier

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | The layer tree. |
| `proxy` | `p` | The proxy shape. |
| `save` | `s` | Write it to its file. |
| `saveAs` | `sa` | Write it to this file. |
| `reload` | `rl` | Back to the file's contents. |
| `flattenTo` | `ft` | Write the composed stage to this file. |
| `saveDirty` | `sd` | Write every unsaved layer that has a file. |

## usdPrim

Author or read a prim named proxy,/path; with no name, the selected prim. Returns the prim's new name when it moved or was made. With -q, a JSON description; -children and -exists narrow it.

Arguments: proxy,/path

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Read, don't author. |
| `children` | `ch` | With -q: the children. |
| `exists` | `ex` | With -q: whether the prim is there. |
| `define` | `def` | Define the prim, of this type; empty for none. |
| `remove` | `rm` | Delete it, or deactivate what can't be deleted from the edit target. |
| `rename` | `rn` | A new name among its siblings. |
| `reparent` | `rp` | A new parent's path. |
| `active` | `act` | Activate or deactivate. |
| `instanceable` | `ins` | Whether it composes as an instance. |
| `kind` | `k` | The model kind. |
| `purpose` | `pur` | default, render, proxy or guide. |
| `load` | `l` | Load its payloads. |
| `unload` | `ul` | Unload its payloads. |
| `addReference` | `ar` | A file to reference. |
| `addPayload` | `ap` | A file to add as a payload. |
| `targetPrim` | `tp` | The prim in that file; empty for its default prim. |
| `setVariant` | `sv` | A variant set and its selection. |
| `assignMaterial` | `am` | The path of a Material prim to bind. |

## usdPy

What the pxr Python module calls: an operation and its values.

Arguments: operation values...

| Flag | Short | Description |
| --- | --- | --- |
| `quiet` | `q` | Accepted. |

## usdRegisterTranslator

A script reads prims of a type into nodes on import and pull. The command is Python with {file}, {prim} and {parent} replaced.

| Flag | Short | Description |
| --- | --- | --- |
| `type` | `t` | The prim type. |
| `command` | `c` | The Python to run. |
| `remove` | `rm` | Unregister it. |
| `query` | `q` | The types registered. |

## usdResolver

How assets resolve for stages opened from now on: -addSearchPath, -map logical real; -q reads them. A Python hook goes through pxr.Ar.SetResolveHook.

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Read. |
| `addSearchPath` | `asp` | A directory to search. |
| `clearSearchPaths` | `csp` | Forget them. |
| `map` | `m` | A logical asset path and the file it stands for. |

## usdStats

A stage's numbers as JSON: prims by type, instances, models, prototypes, layers and their sizes.

Arguments: proxy

## userCtx

A userCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `toolStart` | `ts` |  |
| `toolFinish` | `tf` |  |
| `finalCommandScript` | `fcs` |  |
| `setSelectionCount` | `ssc` |  |
| `setAllowExcessCount` | `sae` |  |
| `setAutoToggleSelection` | `sat` |  |
| `setAutoComplete` | `sac` |  |
| `setDoneSelectionPrompt` | `dsp` |  |
| `setNoSelectionPrompt` | `nsp` |  |
| `setNoSelectionHeadsUp` | `nsh` |  |
| `setSelectionPrompt` | `ssp` |  |
| `setSelectionHeadsUp` | `ssh` |  |
| `totalSelectionSets` | `tss` |  |
| `exitUponCompletion` | `euc` |  |
| `enableRootSelection` | `ers` |  |
| `ignoreInvalidItems` | `iii` |  |
| `cumulativeLists` | `cls` |  |
| `expandSelectionList` | `esl` |  |
| `showManipulators` | `sm` |  |
| `baseClassName` | `bcn` |  |
| `toolCursorType` | `tct` |  |
| `allComponents` | `alc` |  |
| `vertex` | `v` |  |
| `edge` | `eg` |  |
| `facet` | `fc` |  |
| `curve` | `c` |  |
| `polymesh` | `pl` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## uvLink

Held for scripts: links between UV sets and textures. -uvs and -t make one (or -m), -b breaks it; -q -uvs lists textures, -q -t lists UV sets, -qo limits to an object; -iv checks the objects exist. No uvChooser node is made and nothing draws from it. The list is not saved.

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `b` | `b` | Break the link. |
| `isValid` | `iv` | Whether the texture or UV set exists. |
| `make` | `m` | Make the link: what happens without -b. |
| `queryObject` | `qo` | With -q -t: only this object. |
| `texture` | `t` | The texture. |
| `uvSet` | `uvs` | The UV set plug. |

## uvSnapshot

Write the UV wireframe to an image: -n file, -xr -yr, -r -g -b the line colour, -um -uM -vm -vM the range, -o overwrites. PNG, or PPM with -ff ppm.

Arguments: faces or meshes (the selection when none)

| Flag | Short | Description |
| --- | --- | --- |
| `name` | `n` | The file. |
| `xResolution` | `xr` | Width (512). |
| `yResolution` | `yr` | Height (512). |
| `redColor` | `r` | 0 to 255. |
| `greenColor` | `g` | 0 to 255. |
| `blueColor` | `b` | 0 to 255. |
| `uMin` | `umn` | Range. |
| `uMax` | `umx` | Range. |
| `vMin` | `vmn` | Range. |
| `vMax` | `vmx` | Range. |
| `overwrite` | `o` | Replace the file. |
| `fileFormat` | `ff` | png, or ppm. |
| `antiAliased` | `aa` | Accepted for existing scripts. |
| `entireUVRange` | `euv` | Accepted for existing scripts. |

## viewCamera

Put a camera square on to another: -m the camera to move, -t on its top, -s on its side.

Arguments: [camera]

| Flag | Short | Description |
| --- | --- | --- |
| `move` | `m` |  |
| `topView` | `t` |  |
| `sideView` | `s` |  |

## viewClipPlane

A camera's clip planes: -ncp near, -fcp far, -acp set from the scene. -q reads.

Arguments: [camera]

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` |  |
| `edit` | `e` |  |
| `nearClipPlane` | `ncp` |  |
| `farClipPlane` | `fcp` |  |
| `autoClipPlane` | `acp` |  |
| `surfacesOnly` | `so` |  |

## viewFit

Frame the selection, or -all, in a camera: -f the fraction of the view to fill.

Arguments: [camera]

| Flag | Short | Description |
| --- | --- | --- |
| `allObjects` | `all` |  |
| `fitFactor` | `f` |  |
| `animate` | `an` |  |
| `center` | `c` |  |
| `namespace` | `ns` |  |
| `panel` | `p` |  |

## viewHeadOn

Turn a camera to face the selected faces along their normal, or the selection along the view.

Arguments: [camera]

## viewLookAt

Turn a camera to a point, or to the selection. The camera stays where it is.

Arguments: [camera]

| Flag | Short | Description |
| --- | --- | --- |
| `position` | `pos` |  |

## viewManip

The view's orientation aid: -visible shows or hides the axes in the corner, -goHome, -fitToView; the rest are held.

| Flag | Short | Description |
| --- | --- | --- |
| `bottomLeft` | `bl` |  |
| `bottomRight` | `br` |  |
| `compassAngle` | `ca` |  |
| `dragSnap` | `ds` |  |
| `drawCompass` | `dc` |  |
| `fitToView` | `ftv` |  |
| `frontParameter` | `fp` |  |
| `goDefault` | `gd` |  |
| `goHome` | `gh` |  |
| `homeParameter` | `hp` |  |
| `levelCamera` | `lc` |  |
| `minOpacity` | `mo` |  |
| `postCommand` | `pc` |  |
| `preCommand` | `prc` |  |
| `restoreCenter` | `rc` |  |
| `selectionLockParameter` | `slp` |  |
| `setHome` | `sh` |  |
| `size` | `s` |  |
| `topLeft` | `tl` |  |
| `topRight` | `tr` |  |
| `visible` | `v` |  |
| `zoomToFitScene` | `zf` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## viewPlace

Place a camera: -eye -la look at -up, -fov degrees, -o orthographic, -p perspective, -ow its width.

Arguments: [camera]

| Flag | Short | Description |
| --- | --- | --- |
| `eye` | `eye` |  |
| `lookAt` | `la` |  |
| `up` | `up` |  |
| `fieldOfView` | `fov` |  |
| `ortho` | `o` |  |
| `perspective` | `p` |  |
| `orthoWidth` | `ow` |  |

## viewSet

Put a camera on an axis, or back: -p -t -f -s -bo -bk -ls -rs, -home, -fit, -fitFactor.

Arguments: [camera]

| Flag | Short | Description |
| --- | --- | --- |
| `persp` | `p` |  |
| `top` | `t` |  |
| `front` | `f` |  |
| `side` | `s` |  |
| `bottom` | `bo` |  |
| `back` | `bk` |  |
| `leftSide` | `ls` |  |
| `rightSide` | `rs` |  |
| `home` | `h` |  |
| `fit` | `fit` |  |
| `previousView` | `pv` |  |
| `nextView` | `nv` |  |
| `viewNegativeX` | `vnx` |  |
| `viewNegativeY` | `vny` |  |
| `viewNegativeZ` | `vnz` |  |
| `viewX` | `vx` |  |
| `viewY` | `vy` |  |
| `viewZ` | `vz` |  |
| `keepRenderSettings` | `krs` |  |
| `animate` | `an` |  |
| `fitFactor` | `ff` |  |
| `namespace` | `ns` |  |

## volumeAxis

Make a volumeAxis field acting on the selected bodies, or -q/-e one. Returns [field].

Arguments: bodies to act on

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query a value. |
| `edit` | `e` | Edit an existing node. |
| `name` | `n` | Name of the new node. |
| `magnitude` | `m` | Strength. |
| `attenuation` | `att` | How fast it weakens with distance. |
| `maxDistance` | `mxd` | How far it reaches (turns the limit on). |
| `useMaxDistance` | `umd` | Limit its reach. |
| `directionX` | `dx` | Direction X. |
| `directionY` | `dy` | Direction Y. |
| `directionZ` | `dz` | Direction Z. |
| `speed` | `s` | Air: wind speed. |
| `inheritVelocity` | `iv` | Air: kept motion. |
| `frequency` | `f` | Turbulence: noise frequency. |
| `minDistance` | `mnd` | Newton: least distance. |
| `position` | `pos` | Where it is. |

## vortex

Make a vortex field acting on the selected bodies, or -q/-e one. Returns [field].

Arguments: bodies to act on

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query a value. |
| `edit` | `e` | Edit an existing node. |
| `name` | `n` | Name of the new node. |
| `magnitude` | `m` | Strength. |
| `attenuation` | `att` | How fast it weakens with distance. |
| `maxDistance` | `mxd` | How far it reaches (turns the limit on). |
| `useMaxDistance` | `umd` | Limit its reach. |
| `directionX` | `dx` | Direction X. |
| `directionY` | `dy` | Direction Y. |
| `directionZ` | `dz` | Direction Z. |
| `speed` | `s` | Air: wind speed. |
| `inheritVelocity` | `iv` | Air: kept motion. |
| `frequency` | `f` | Turbulence: noise frequency. |
| `minDistance` | `mnd` | Newton: least distance. |
| `position` | `pos` | Where it is. |

## waitCursor

The wait cursor: -state on or off.

| Flag | Short | Description |
| --- | --- | --- |
| `state` | `st` |  |
| `query` | `q` |  |

## walkCtx

A walkCtx tool context: create by name, -e and -q its settings. setToolTo makes it current.

Arguments: [name]

| Flag | Short | Description |
| --- | --- | --- |
| `alternateContext` | `ac` |  |
| `crouchCount` | `crc` |  |
| `toolName` | `tn` |  |
| `walkHeight` | `wh` |  |
| `walkSensitivity` | `wsn` |  |
| `walkSpeed` | `ws` |  |
| `walkToolHud` | `wth` |  |
| `image1` | `i1` |  |
| `image2` | `i2` |  |
| `image3` | `i3` |  |
| `title` | `t` |  |
| `exists` | `ex` |  |
| `query` | `q` |  |
| `edit` | `e` |  |
| `name` | `n` |  |
| `history` | `ch` |  |
| `dragSlider` | `ds` |  |

## weightHammerVerts

Hammer Skin Weights: the points named or selected take the average weights of the points around them.

Arguments: points (the selection when none)

## weightsColor

Held for scripts: the false colour settings for showing a deformer's weights. Nothing is drawn from them: the views have no weight display for this. -q reads them.

Arguments: objects

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `colorRamp` | `cr` | A ramp. |
| `deformer` | `dfm` | The deformer. |
| `falseColor` | `fc` | False colour on or off. |
| `outOfRangeColor` | `orc` | Colour outside the deformer's set. |
| `rampMaxColor` | `rxc` | Colour at or over the maximum. |
| `rampMinColor` | `rmc` | Colour at or under the minimum. |
| `useColorRamp` | `ucr` | Use the ramp. |
| `useMaxMinColor` | `umc` | Use the out of range colours. |

## window

Script UI: window, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `title` | `t` |  |
| `widthHeight` | `wh` |  |
| `sizeable` | `s` |  |
| `minimizeBox` | `mnb` |  |
| `maximizeBox` | `mxb` |  |
| `resizeToFitChildren` | `rtf` |  |
| `menuBar` | `mb` |  |
| `menuBarVisible` | `mbv` |  |
| `titleBar` | `tb` |  |
| `titleBarMenu` | `tbm` |  |
| `toolbox` | `tlb` |  |
| `topEdge` | `te` |  |
| `leftEdge` | `le` |  |
| `topLeftCorner` | `tlc` |  |
| `retain` | `ret` |  |
| `closeCommand` | `cc` |  |
| `minimizeCommand` | `mnc` |  |
| `restoreCommand` | `rc` |  |
| `iconName` | `in` |  |
| `iconify` | `i` |  |
| `interactivePlacement` | `ip` |  |
| `frontWindow` | `fw` |  |
| `numberOfWindows` | `nw` |  |
| `mainWindow` | `mw` |  |
| `nestedDockingEnabled` | `nde` |  |
| `dockCorner` | `dc` |  |
| `dockStation` | `dcs` |  |
| `dockingLayout` | `dl` |  |
| `state` | `st` |  |
| `menuArray` | `ma` |  |
| `childArray` | `ca` |  |
| `numberOfChildren` | `nch` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## windowPref

Saved window positions and sizes: -exists, -remove, -wh, -tlc. None are kept here between sessions.

Arguments: window

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` |  |
| `edit` | `e` |  |
| `exists` | `ex` |  |
| `remove` | `r` |  |
| `removeAll` | `ra` |  |
| `widthHeight` | `wh` |  |
| `width` | `w` |  |
| `height` | `h` |  |
| `topLeftCorner` | `tlc` |  |
| `topEdge` | `te` |  |
| `leftEdge` | `le` |  |
| `restoreMainWindowState` | `rms` |  |
| `saveMainWindowState` | `sms` |  |
| `enableAll` | `ea` |  |

## wire

Make points follow a curve: wire -w curve1 -dds 0 2.0 -en 1 -ce 0 -li 0 -n name mesh. Returns [wire1]. With -e on a wire: -w adds a wire, -dds and the settings change. With -q: -w and the settings.

Arguments: meshes to deform, then the wire unless -w names it (the selection when none); the wire with -e and -q

| Flag | Short | Description |
| --- | --- | --- |
| `name` | `n` | Name of the deformer. |
| `before` | `bf` | Go just above the deformer nearest the mesh. |
| `after` | `af` | Go nearest the mesh, after the other deformers (the default). |
| `frontOfChain` | `foc` | Go above every other deformer. |
| `envelope` | `en` | How much of the deformation applies (1). |
| `edit` | `e` | Edit mode. |
| `query` | `q` | Query mode. |
| `wire` | `w` | The curve to follow, or several. Any shape with points will do. |
| `dropoffDistance` | `dds` | Wire index, then how far it reaches, cm (1). |
| `groupWithBase` | `gw` | Accepted and ignored: the base wire is held on the node, so there is nothing to group. |
| `rotation` | `rt` | How much points turn with the wire: 0 not at all, 1 fully (1). |
| `crossingEffect` | `ce` | Where wires cross: 0 they blend, 1 they add up (0). |
| `tension` | `tn` | How tightly the effect gathers to the wire (1). |
| `localInfluence` | `li` | Where wires cross: how much a still wire holds its points, 0 to 1 (0). |

## workspace

The project: -q -rd or -fn gives its folder, -o and -dir change to one.

Arguments: [path]

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query. |
| `rootDirectory` | `rd` | The project folder, with a slash. |
| `fullName` | `fn` | The project folder. |
| `directory` | `dir` | Change to this folder. |
| `openWorkspace` | `o` | Open this project. |

## workspaceControl

Script UI: workspaceControl, with create, edit and query as in the reference's command.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `label` | `l` |  |
| `uiScript` | `ui` |  |
| `floating` | `fl` |  |
| `initialWidth` | `iw` |  |
| `initialHeight` | `ih` |  |
| `minimumWidth` | `mw` |  |
| `minimumHeight` | `mh` |  |
| `retain` | `r` |  |
| `widthProperty` | `wp` |  |
| `heightProperty` | `hp` |  |
| `dockToMainWindow` | `dtm` |  |
| `dockToControl` | `dtc` |  |
| `dockToPanel` | `dtp` |  |
| `tabToControl` | `ttc` |  |
| `collapse` | `clp` |  |
| `restore` | `rs` |  |
| `closeCommand` | `cc` |  |
| `requiredPlugin` | `rp` |  |
| `requiredControl` | `rc` |  |
| `loadImmediately` | `li` |  |
| `checksPlugins` | `cp` |  |
| `actLikeMayaUIElement` | `aui` |  |
| `duplicatable` | `dup` |  |
| `horizontal` | `hr` |  |
| `stateString` | `ss` |  |
| `uiScriptCommand` | `uic` |  |
| `childArray` | `ca` |  |
| `numberOfChildren` | `nch` |  |
| `annotation` | `ann` |  |
| `backgroundColor` | `bgc` |  |
| `defineTemplate` | `dt` |  |
| `docTag` | `dtg` |  |
| `dragCallback` | `dgc` |  |
| `dropCallback` | `dpc` |  |
| `enable` | `en` |  |
| `enableBackground` | `ebg` |  |
| `enableKeyboardFocus` | `ekf` |  |
| `exists` | `ex` |  |
| `fullPathName` | `fpn` |  |
| `height` | `h` |  |
| `highlightColor` | `hlc` |  |
| `isObscured` | `io` |  |
| `manage` | `m` |  |
| `noBackground` | `nbg` |  |
| `numberOfPopupMenus` | `npm` |  |
| `parent` | `p` |  |
| `popupMenuArray` | `pma` |  |
| `preventOverride` | `po` |  |
| `statusBarMessage` | `sbm` |  |
| `useTemplate` | `ut` |  |
| `visible` | `vis` |  |
| `visibleChangeCommand` | `vcc` |  |
| `width` | `w` |  |
| `query` | `q` |  |
| `edit` | `e` |  |

## workspaceControlState

Saved state of workspace controls: none are kept here, so -ex is false and -remove does nothing.

Arguments: name

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` |  |
| `edit` | `e` |  |
| `exists` | `ex` |  |
| `remove` | `r` |  |
| `defaultTopLeftCorner` | `dtc` |  |
| `defaultWidthHeight` | `dwh` |  |
| `heightProperty` | `hp` |  |
| `leftEdge` | `le` |  |
| `maximumWidthHeight` | `mwh` |  |
| `minimumWidthHeight` | `nwh` |  |
| `topEdge` | `te` |  |
| `topLeftCorner` | `tlc` |  |
| `widthHeight` | `wh` |  |
| `widthProperty` | `wp` |  |

## wrap

Make meshes follow a wrapper: wrap -inf wrapper mesh, or select the meshes and then the wrapper. Returns [wrap1]. With -e on a wrap (say one from deformer -type wrap): -inf adds an influence and binds again, -ri removes one, -rb binds again as things are now. With -q: -inf and the settings.

Arguments: meshes to deform, then the wrapper unless -inf names it (the selection when none); the wrap with -e and -q

| Flag | Short | Description |
| --- | --- | --- |
| `name` | `n` | Name of the deformer. |
| `before` | `bf` | Go just above the deformer nearest the mesh. |
| `after` | `af` | Go nearest the mesh, after the other deformers (the default). |
| `frontOfChain` | `foc` | Go above every other deformer. |
| `envelope` | `en` | How much of the deformation applies (1). |
| `edit` | `e` | Edit mode. |
| `query` | `q` | Query mode. |
| `influence` | `inf` | The wrapper mesh, or several. With -e: add. Query: the influences. |
| `removeInfluence` | `ri` | With -e: the influence to remove. |
| `rebind` | `rb` | With -e: bind again as things are now. |
| `maxDistance` | `md` | How far a point looks for triangles to bind to, cm (1); 0 has no limit. |
| `exclusiveBind` | `ebn` | Each point binds to its nearest triangle only. |
| `autoWeightThreshold` | `awt` | Every point binds to its nearest triangle, however far (on). |
| `weightThreshold` | `wth` | With -awt off: drop bindings of less weight than this. |
| `falloffMode` | `fom` | 0 any triangle in reach, 1 only ones touching the nearest. |

## wrinkle

Wrinkle: creases around a handle. Returns [wrinkle1, wrinkle1Handle].

Arguments: what to deform, and any target last (the selection when none); the deformer with -e and -q

| Flag | Short | Description |
| --- | --- | --- |
| `name` | `n` | Name of the deformer. |
| `before` | `bf` | Go just above the deformer nearest the mesh. |
| `after` | `af` | Go nearest the mesh, after the other deformers (the default). |
| `frontOfChain` | `foc` | Go above every other deformer. |
| `envelope` | `en` | How much of the deformation applies (1). |
| `edit` | `e` | Edit mode. |
| `query` | `q` | Query mode. |
| `wrinkleCount` | `wc` | How many creases (3). |
| `wrinkleIntensity` | `wi` | How deep they go (0.5). |
| `thickness` | `th` | How wide they are (1). |
| `randomness` | `rnd` | How irregular they are (0.2). |
| `style` | `st` | radial (the default), tangential or lines. |
| `dropoffDistance` | `dds` | How far they reach from the handle. |
| `scale` | `sc` | Multiplies the depth (1). |
| `x` | `x` | Lines run across X rather than Z. |

## xbmLangPathList

The folders icons are looked for in.

## xform

Query or set the transform of objects, in local or world space. Sets every flag given on each object, or with query=True returns one value of the first object: pass the flag to read as True. Values are local to the parent unless worldSpace is on.

Arguments: Transforms to edit or query; the selection when none are given.

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Return a value of the first object. |
| `worldSpace` | `ws` | World space. |
| `objectSpace` | `os` | Object space (the default). |
| `absolute` | `a` | Values replace what is there (the default). |
| `relative` | `r` | Values add to what is there. |
| `preserve` | `p` | With -roo: keep the orientation, changing the angles. |
| `translation` | `t` | Position. |
| `rotation` | `ro` | Rotation, degrees. |
| `scale` | `s` | Scale. |
| `shear` | `sh` | Shear: xy, xz, yz. |
| `matrix` | `m` | The matrix, 16 values. |
| `pivots` | `piv` | Both pivots: a point to set, six values queried. |
| `rotatePivot` | `rp` | The rotate pivot. |
| `scalePivot` | `sp` | The scale pivot. |
| `rotateAxis` | `ra` | The rotate axis. |
| `rotateOrder` | `roo` | The rotate order, by name. |
| `boundingBox` | `bb` | With -q: xmin ymin zmin xmax ymax zmax. |
| `boundingBoxInvisible` | `bbi` | With -q: bounding box, hidden shapes included. |
| `rotateTranslation` | `rt` | rotatePivotTranslate. |
| `scaleTranslation` | `st` | scalePivotTranslate. |
| `centerPivots` | `cp` | Centre the pivots on the bounding box. |
| `centerPivotsOnComponents` | `cpc` | Centre the pivots on the bounding box. |
| `zeroTransformPivots` | `ztp` | Put the pivots back at the origin of the object. |
| `worldSpaceDistance` | `wd` | Accepted: distances are in world units already. |
| `eulerFilter` | `eu` | Accepted for existing scripts. |
| `deletePriorHistory` | `dph` | Accepted for existing scripts. |
| `reflection` | `rfl` | Accepted for existing scripts. |
| `disableReflection` | `drp` | Accepted for existing scripts. |
| `requiresAll` | `rab` | Accepted for existing scripts. |

## xformConstraint

Held for scripts: the transform tools' constraint, -t none, surface, edge or live, -n along normals. The tools do not honour it yet. -q -t, -q -n and -q -l read it; -l is whether there is a live surface.

| Flag | Short | Description |
| --- | --- | --- |
| `query` | `q` | Query mode. |
| `edit` | `e` | Edit mode. |
| `type` | `t` | none, surface, edge or live. |
| `alongNormal` | `n` | Along the normals first. |
| `live` | `l` | With -q: whether a live surface is set. |

## zeroPivot

Move the pivots to the world origin, keeping the object where it is.

Arguments: objects

## zeroPivotRotation

Line the pivots' axes up with the world (or with the object's own axes), leaving their positions and the objects alone.

Arguments: objects

| Flag | Short | Description |
| --- | --- | --- |
| `space` |  | Axes to line up with. |

