"""Walks every mesh with the Python API: vertex and face counts, surface area and the bounding box."""
import maya.api.OpenMaya as om

it = om.MItDependencyNodes(om.MFn.kMesh)
total = 0
while not it.isDone():
    fn = om.MFnDagNode(it.thisNode())
    path = fn.getPath()
    mesh = om.MFnMesh(path)
    bb = fn.boundingBox
    area = mesh.area() if hasattr(mesh, "area") else 0.0
    print("%-30s %7d verts %7d faces  area %10.1f  size %.1f x %.1f x %.1f" % (
        path.partialPathName(), mesh.numVertices, mesh.numPolygons, area, bb.width, bb.height, bb.depth))
    total += mesh.numPolygons
    it.next()
print("%d faces in all." % total)
