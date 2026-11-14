// A command that reads the scene through the kit's iterators and function sets and gives back "name=value" strings, for
// the tests to check: azProbe mesh, azProbe dag, azProbe anim, azProbe skin, azProbe surface.
#include <maya/MAnimCurveChange.h>
#include <maya/MArgList.h>
#include <maya/MDagPath.h>
#include <maya/MDoubleArray.h>
#include <maya/MFnAnimCurve.h>
#include <maya/MFnDependencyNode.h>
#include <maya/MFnNurbsSurface.h>
#include <maya/MFnPlugin.h>
#include <maya/MFnSkinCluster.h>
#include <maya/MFnSingleIndexedComponent.h>
#include <maya/MItDag.h>
#include <maya/MItMeshEdge.h>
#include <maya/MItMeshPolygon.h>
#include <maya/MItMeshVertex.h>
#include <maya/MPxCommand.h>
#include <maya/MSelectionList.h>
#include <sstream>
#include <string>

namespace {
MDagPath pathOf(const char* name) {
    MSelectionList sel;
    sel.add(name);
    MDagPath p;
    sel.getDagPath(0, p);
    return p;
}
MObject nodeOf(const char* name) {
    MSelectionList sel;
    sel.add(name);
    MObject o;
    sel.getDependNode(0, o);
    return o;
}
void put(const std::string& key, double v) {
    std::ostringstream s;
    s.precision(12);
    s << key << "=" << v;
    MPxCommand::appendToResult(s.str().c_str());
}
void put(const std::string& key, const std::string& v) { MPxCommand::appendToResult((key + "=" + v).c_str()); }
}  // namespace

class Probe : public MPxCommand {
public:
    static void* creator() { return new Probe; }

    MStatus doIt(const MArgList& args) override {
        clearResult();
        MString what = args.asString(0);
        if (what == "mesh") return mesh();
        if (what == "dag") return dag();
        if (what == "anim") return anim();
        if (what == "skin") return skin();
        if (what == "surface") return surface();
        return MS::kInvalidParameter;
    }

    // A polyCube "box": faces, edges and points, and how they meet.
    MStatus mesh() {
        MDagPath box = pathOf("box");
        MItMeshPolygon faces(box);
        put("faces", faces.count());
        int quads = 0, fourNeighbours = 0;
        double area = 0;
        for (; !faces.isDone(); faces.next()) {
            if (faces.polygonVertexCount() == 4) ++quads;
            MIntArray nb;
            faces.getConnectedFaces(nb);
            if (nb.length() == 4) ++fourNeighbours;
            double a = 0;
            faces.getArea(a);
            area += a;
        }
        put("quads", quads);
        put("faceNeighbours4", fourNeighbours);
        put("area", area);
        faces.reset();
        MIntArray es;
        faces.getEdges(es);
        put("face0Edges", es.length());
        MVector n;
        faces.getNormal(n);
        put("face0NormalLength", n.length());
        MPoint c = faces.center();
        put("face0CenterFromOrigin", MVector(c.x, c.y, c.z).length());

        MItMeshEdge edges(box);
        put("edges", edges.count());
        int twoFaces = 0, fourEdges = 0, unit = 0;
        for (; !edges.isDone(); edges.next()) {
            if (edges.numConnectedFaces() == 2) ++twoFaces;
            if (edges.numConnectedEdges() == 4) ++fourEdges;
            double len = 0;
            edges.getLength(len);
            if (len > 0.999 && len < 1.001) ++unit;
        }
        put("edgesWithTwoFaces", twoFaces);
        put("edgesWithFourNeighbours", fourEdges);
        put("unitEdges", unit);

        // A vertex's edges are the mesh's own: each one has the vertex at an end.
        MItMeshVertex verts(box);
        MIntArray ve;
        verts.getConnectedEdges(ve);
        int ending = 0;
        MItMeshEdge check(box);
        for (unsigned int i = 0; i < ve.length(); ++i) {
            int prev;
            check.setIndex(ve[i], prev);
            if (check.index(0) == verts.index() || check.index(1) == verts.index()) ++ending;
        }
        put("vertex0Edges", ve.length());
        put("vertex0EdgesEndingThere", ending);

        // A component limits the iterator.
        MFnSingleIndexedComponent fn;
        MObject comp = fn.create(MFn::kMeshPolygonComponent);
        fn.addElement(2);
        fn.addElement(4);
        MItMeshPolygon some(box, comp);
        int n2 = 0;
        for (; !some.isDone(); some.next()) ++n2;
        put("pickedVisited", n2);
        return MS::kSuccess;
    }

    MStatus dag() {
        MItDag it(MItDag::kDepthFirst, MFn::kMesh);
        int meshes = 0;
        unsigned int depth = 0;
        for (; !it.isDone(); it.next()) {
            ++meshes;
            if (it.partialPathName() == "boxShape") depth = it.depth();
        }
        put("meshes", meshes);
        put("boxShapeDepth", depth);
        MItDag under;
        under.reset(nodeOf("grp"), MItDag::kBreadthFirst);
        std::string order;
        for (; !under.isDone(); under.next()) order += std::string(under.partialPathName().asChar()) + ",";
        put("underGrp", order);
        MItDag pruned;
        pruned.reset(nodeOf("grp"));
        int seen = 0;
        for (; !pruned.isDone(); pruned.next()) {
            ++seen;
            pruned.prune();
        }
        put("pruned", seen);
        return MS::kSuccess;
    }

    MStatus anim() {
        MStatus st;
        MFnAnimCurve tx(MFnDependencyNode(nodeOf("box")).findPlug("translateX"), &st);
        if (!st) return st;
        put("txType", tx.animCurveType());
        put("txKeys", tx.numKeys());
        put("txTime1", tx.time(1).as(MTime::kFilm));
        put("txValue1", tx.value(1));
        put("txAt10", tx.evaluate(MTime(10.0, MTime::kFilm)));
        MFnAnimCurve ry(MFnDependencyNode(nodeOf("box")).findPlug("rotateY"), &st);
        put("ryValue1", ry.value(1));
        put("ryAt10", ry.evaluate(MTime(10.0, MTime::kFilm)));
        ry.setValue(0, 0.5);
        put("ryValue0", ry.value(0));
        ry.addKey(MTime(20.0, MTime::kFilm), 1.0);
        put("ryValue2", ry.value(2));
        tx.addKey(MTime(20.0, MTime::kFilm), 7.0, MFnAnimCurve::kTangentLinear, MFnAnimCurve::kTangentLinear);
        put("txKeysAfterAdd", tx.numKeys());
        unsigned int at = 99;
        put("found20", tx.find(MTime(20.0, MTime::kFilm), at) ? at : -1);
        put("outTangent20", tx.outTangentType(at));
        tx.setValue(0, 2.0);
        put("txValue0", tx.value(0));
        tx.remove(2);
        put("txKeysAfterRemove", tx.numKeys());
        return MS::kSuccess;
    }

    MStatus skin() {
        MStatus st;
        MFnSkinCluster fn(nodeOf("skin1"), &st);
        if (!st) return st;
        MDagPathArray infs;
        put("influences", fn.influenceObjects(infs));
        put("influence1", infs.length() > 1 ? std::string(infs[1].partialPathName().asChar()) : std::string());
        MDagPath box = pathOf("boxShape");
        MDoubleArray w;
        unsigned int n = 0;
        fn.getWeights(box, MObject::kNullObj, w, n);
        put("weightCount", w.length());
        put("perPoint", n);
        double sum = 0;
        for (unsigned int i = 0; i < n; ++i) sum += w[i];
        put("point0Sum", sum);
        MFnSingleIndexedComponent cf;
        MObject v0 = cf.create(MFn::kMeshVertComponent);
        cf.addElement(0);
        fn.setWeights(box, v0, 1u, 1.0);
        MDoubleArray one;
        fn.getWeights(box, v0, 1u, one);
        put("v0Influence1", one.length() ? one[0] : -1);
        put("logicalOfJ2", fn.indexForInfluenceObject(pathOf("j2")));
        return MS::kSuccess;
    }

    MStatus surface() {
        MStatus st;
        MFnNurbsSurface fn(pathOf("surf"), &st);
        if (!st) return st;
        put("cvsU", fn.numCVsInU());
        put("cvsV", fn.numCVsInV());
        put("degreeU", fn.degreeU());
        double u0, u1, v0, v1;
        fn.getKnotDomain(u0, u1, v0, v1);
        MPoint mid;
        fn.getPointAtParam((u0 + u1) * 0.5, (v0 + v1) * 0.5, mid, MSpace::kWorld);
        put("midY", mid.y);
        MVector n = fn.normal((u0 + u1) * 0.5, (v0 + v1) * 0.5);
        put("normalLength", n.length());
        double u, v;
        MPoint near = fn.closestPoint(MPoint(0, 5, 0), &u, &v, false, 1e-3, MSpace::kWorld);
        put("closestY", near.y);
        MPointArray cvs;
        fn.getCVs(cvs);
        put("cvCount", cvs.length());
        put("area", fn.area());
        return MS::kSuccess;
    }
};

MStatus initializePlugin(MObject obj) {
    MFnPlugin plugin(obj, "Aztek tests", "1.0", "Any");
    return plugin.registerCommand("azProbe", Probe::creator);
}

MStatus uninitializePlugin(MObject obj) {
    MFnPlugin plugin(obj);
    return plugin.deregisterCommand("azProbe");
}
