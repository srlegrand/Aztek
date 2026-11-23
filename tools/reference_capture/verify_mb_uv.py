"""Open an Aztek-written Maya binary scene and print its mesh UV data as JSON."""
import json
import sys

import maya.standalone

maya.standalone.initialize(name="python")
import maya.api.OpenMaya as om
import maya.cmds as cmds

try:
    scenes = []
    for path in sys.argv[1:]:
        cmds.file(path, open=True, force=True, executeScriptNodes=False)
        meshes = {}
        for shape in ("uvProbeShape", "cubeProbeShape"):
            selection = om.MSelectionList()
            selection.add(shape)
            mesh = om.MFnMesh(selection.getDagPath(0))
            sets = {}
            for uv_set in mesh.getUVSetNames():
                u, v = mesh.getUVs(uv_set)
                counts, ids = mesh.getAssignedUVs(uv_set)
                sets[uv_set] = {
                    "uvCount": len(u),
                    "uvs": [component for pair in zip(u, v) for component in pair],
                    "faceCounts": list(counts),
                    "faceIds": list(ids),
                }
            meshes[shape] = {"uvSets": mesh.getUVSetNames(), "sets": sets}
        scenes.append({"path": path, "meshes": meshes})
    print(json.dumps({
        "version": cmds.about(version=True),
        "apiVersion": cmds.about(apiVersion=True),
        "cutIdentifier": cmds.about(cutIdentifier=True),
        "scenes": scenes,
    }))
finally:
    maya.standalone.uninitialize()
