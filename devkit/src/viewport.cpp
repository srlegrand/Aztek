// Aztek plug-in kit: locator nodes, Viewport 2.0 draw overrides, ramps and scene messages.
#include "internal.h"

#include <algorithm>
#include <atomic>
#include <cmath>

using namespace az;

// ---------------------------------------------------------------------------------------------------------------------
// M3dView: the legacy view a locator's draw got. Nothing calls it here.

M3dView::M3dView() {}
M3dView M3dView::active3dView(MStatus* st) {
    if (st) *st = MS::kSuccess;
    return M3dView();
}
MStatus M3dView::beginGL() { return MS::kSuccess; }
MStatus M3dView::endGL() { return MS::kSuccess; }
void M3dView::setDrawColor(const MColor&) {}
void M3dView::setDrawColor(unsigned int, ColorTable) {}
MStatus M3dView::drawText(const MString&, const MPoint&, TextPosition) { return MS::kSuccess; }
MStatus M3dView::modelViewMatrix(MMatrix& out) const {
    out = MMatrix();
    return MS::kSuccess;
}
MStatus M3dView::projectionMatrix(MMatrix& out) const {
    out = MMatrix();
    return MS::kSuccess;
}
MStatus M3dView::refresh(bool, bool) { return MS::kSuccess; }
int M3dView::portWidth(MStatus* st) {
    if (st) *st = MS::kSuccess;
    return 640;
}
int M3dView::portHeight(MStatus* st) {
    if (st) *st = MS::kSuccess;
    return 480;
}

// ---------------------------------------------------------------------------------------------------------------------
// MPxLocatorNode

MPxLocatorNode::MPxLocatorNode() {}
MPxLocatorNode::~MPxLocatorNode() {}
MPxNode::Type MPxLocatorNode::type() const { return kLocatorNode; }
void MPxLocatorNode::draw(M3dView&, const MDagPath&, M3dView::DisplayStyle, M3dView::DisplayStatus) {}
bool MPxLocatorNode::isBounded() const { return false; }
MBoundingBox MPxLocatorNode::boundingBox() const { return MBoundingBox(); }
bool MPxLocatorNode::isTransparent() const { return false; }
bool MPxLocatorNode::drawLast() const { return false; }
bool MPxLocatorNode::excludeAsLocator() const { return false; }
MColor MPxLocatorNode::color(M3dView::DisplayStatus status) {
    // The reference's dormant and lead locator colours are of its colour preferences; these stand in.
    return status == M3dView::kLead ? MColor(0.26f, 1.0f, 0.64f) : status == M3dView::kActive ? MColor(1.0f, 1.0f, 1.0f) : MColor(0.0f, 0.016f, 0.376f);
}

MObject MPxLocatorNode::underWorldObject = attrObject(builtinAttr("underWorldObject"));
MObject MPxLocatorNode::localPosition = attrObject(builtinAttr("localPosition"));
MObject MPxLocatorNode::localPositionX = attrObject(builtinAttr("localPositionX"));
MObject MPxLocatorNode::localPositionY = attrObject(builtinAttr("localPositionY"));
MObject MPxLocatorNode::localPositionZ = attrObject(builtinAttr("localPositionZ"));
MObject MPxLocatorNode::worldPosition = attrObject(builtinAttr("worldPosition"));
MObject MPxLocatorNode::worldPositionX = attrObject(builtinAttr("worldPositionX"));
MObject MPxLocatorNode::worldPositionY = attrObject(builtinAttr("worldPositionY"));
MObject MPxLocatorNode::worldPositionZ = attrObject(builtinAttr("worldPositionZ"));
MObject MPxLocatorNode::localScale = attrObject(builtinAttr("localScale"));
MObject MPxLocatorNode::localScaleX = attrObject(builtinAttr("localScaleX"));
MObject MPxLocatorNode::localScaleY = attrObject(builtinAttr("localScaleY"));
MObject MPxLocatorNode::localScaleZ = attrObject(builtinAttr("localScaleZ"));
MObject MPxLocatorNode::nodeBoundingBox = attrObject(builtinAttr("boundingBox"));

// ---------------------------------------------------------------------------------------------------------------------
// MUserData

MUserData::MUserData() : delete_(true) {}
MUserData::MUserData(bool deleteAfterUse) : delete_(deleteAfterUse) {}
MUserData::~MUserData() {}
bool MUserData::deleteAfterUse() const { return delete_; }
void MUserData::setDeleteAfterUse(bool on) { delete_ = on; }

// ---------------------------------------------------------------------------------------------------------------------
// Viewport 2.0

namespace MHWRender {

MFrameContext::MFrameContext() {}
MFrameContext::~MFrameContext() {}
MDagPath MFrameContext::getCurrentCameraPath(MStatus* st) const {
    MDagPath p;
    int64_t cam = nodeNamed("perspShape");
    MStatus s = MDagPath::getAPathTo(nodeObject(cam), p);
    if (st) *st = s;
    return p;
}
void MFrameContext::getViewportDimensions(int& x, int& y, int& w, int& h) const {
    x = 0;
    y = 0;
    w = 640;
    h = 480;
}

MDrawContext::MDrawContext() {}

MUIDrawManager::MUIDrawManager() : color_(1.0f, 1.0f, 1.0f) {}
void MUIDrawManager::beginDrawable() {}
void MUIDrawManager::endDrawable() {}
void MUIDrawManager::setColor(const MColor& c) { color_ = c; }
void MUIDrawManager::setLineWidth(float) {}
void MUIDrawManager::setLineStyle(LineStyle) {}
void MUIDrawManager::setPointSize(float) {}
void MUIDrawManager::setFontSize(unsigned int) {}
static MUIDrawManager::Item item(int kind, const MColor& c, const MPoint& a, const MPoint& b, double r = 0, double h = 0, bool filled = false) {
    MUIDrawManager::Item i;
    i.kind = kind;
    i.color = c;
    i.a = a;
    i.b = b;
    i.r = r;
    i.h = h;
    i.filled = filled;
    return i;
}
void MUIDrawManager::line(const MPoint& a, const MPoint& b) { azItems.push_back(item(0, color_, a, b)); }
void MUIDrawManager::point(const MPoint& p) { azItems.push_back(item(4, color_, p, p)); }
void MUIDrawManager::circle(const MPoint& c, const MVector& n, double r, bool filled) { azItems.push_back(item(1, color_, c, MPoint(n.x, n.y, n.z), r, 0, filled)); }
void MUIDrawManager::rect(const MPoint& c, const MVector& up, const MVector& normal, double sx, double sy, bool) {
    MVector right = (up ^ normal).normal();
    MVector u = up.normal();
    MPoint p[4] = {c + right * sx + u * sy, c - right * sx + u * sy, c - right * sx - u * sy, c + right * sx - u * sy};
    for (int i = 0; i < 4; ++i) line(p[i], p[(i + 1) % 4]);
}
void MUIDrawManager::sphere(const MPoint& c, double r, bool filled) { azItems.push_back(item(5, color_, c, c, r, 0, filled)); }
void MUIDrawManager::cone(const MPoint& base, const MVector& dir, double r, double h, bool filled) {
    azItems.push_back(item(2, color_, base, MPoint(dir.x, dir.y, dir.z), r, h, filled));
}
void MUIDrawManager::box(const MPoint& c, const MVector& up, const MVector& right, double sx, double sy, double sz, bool) {
    MVector u = up.normal() * sy, r = right.normal() * sx, f = (right ^ up).normal() * sz;
    MPoint p[8];
    for (int i = 0; i < 8; ++i) p[i] = c + r * ((i & 1) ? 1 : -1) + u * ((i & 2) ? 1 : -1) + f * ((i & 4) ? 1 : -1);
    static const int e[12][2] = {{0, 1}, {2, 3}, {4, 5}, {6, 7}, {0, 2}, {1, 3}, {4, 6}, {5, 7}, {0, 4}, {1, 5}, {2, 6}, {3, 7}};
    for (int i = 0; i < 12; ++i) line(p[e[i][0]], p[e[i][1]]);
}
void MUIDrawManager::text(const MPoint& p, const MString& t, TextAlignment, const int*, const MColor*, bool) {
    Item i = item(3, color_, p, p);
    i.text = t.asChar();
    azItems.push_back(i);
}
void MUIDrawManager::mesh(Primitive mode, const MPointArray& pts) {
    if (mode == kLines) {
        for (unsigned int i = 0; i + 1 < pts.length(); i += 2) line(pts[i], pts[i + 1]);
    } else if (mode == kLineStrip || mode == kClosedLine) {
        for (unsigned int i = 0; i + 1 < pts.length(); ++i) line(pts[i], pts[i + 1]);
        if (mode == kClosedLine && pts.length() > 2) line(pts[pts.length() - 1], pts[0]);
    } else if (mode == kPoints) {
        for (unsigned int i = 0; i < pts.length(); ++i) point(pts[i]);
    }
}

MPxDrawOverride::MPxDrawOverride(const MObject&, GeometryDrawOverrideCb, bool) {}
MPxDrawOverride::~MPxDrawOverride() {}
DrawAPI MPxDrawOverride::supportedDrawAPIs() const { return kOpenGL; }
bool MPxDrawOverride::isBounded(const MDagPath&, const MDagPath&) const { return false; }
MBoundingBox MPxDrawOverride::boundingBox(const MDagPath&, const MDagPath&) const { return MBoundingBox(); }
bool MPxDrawOverride::disableInternalBoundingBoxDraw() const { return false; }
bool MPxDrawOverride::hasUIDrawables() const { return false; }
void MPxDrawOverride::addUIDrawables(const MDagPath&, MUIDrawManager&, const MFrameContext&, const MUserData*) {}
bool MPxDrawOverride::excludedFromPostEffects() const { return true; }
bool MPxDrawOverride::isTransparent() const { return false; }

MStatus MDrawRegistry::registerDrawOverrideCreator(const MString& classification, const MString& id, DrawOverrideCreator creator) {
    drawOverrides()[classification.asChar()] = std::make_pair(std::string(id.asChar()), creator);
    return MS::kSuccess;
}
MStatus MDrawRegistry::deregisterDrawOverrideCreator(const MString& classification, const MString&) {
    drawOverrides().erase(classification.asChar());
    return MS::kSuccess;
}

DisplayStatus MGeometryUtilities::displayStatus(const MDagPath& path, MStatus* st) {
    if (st) *st = MS::kSuccess;
    // Selected last: lead; selected: active; otherwise dormant.
    Json sel = command("ls", Json::array(), Json::object().set("selection", true).set("long", true), 0);
    std::string name = path.fullPathName().asChar();
    std::string transform = name.substr(0, name.rfind('|'));
    for (size_t i = 0; i < sel.size(); ++i) {
        std::string s = sel[i].str();
        if (s == name || s == transform || ("|" + s) == name || ("|" + s) == transform) return i + 1 == sel.size() ? kLead : kActive;
    }
    return kDormant;
}
MColor MGeometryUtilities::wireframeColor(const MDagPath& path) {
    DisplayStatus s = displayStatus(path);
    return s == kLead ? MColor(0.26f, 1.0f, 0.64f) : s == kActive ? MColor(1.0f, 1.0f, 1.0f) : MColor(0.0f, 0.016f, 0.376f);
}

void MRenderer::setGeometryDrawDirty(const MObject&, bool) {}

}  // namespace MHWRender

// ---------------------------------------------------------------------------------------------------------------------
// MFnCamera: what a draw override reads of the camera, from its world matrix and attributes.

MFnCamera::MFnCamera() {}
MFnCamera::MFnCamera(const MObject& o, MStatus* st) : MFnDagNode(o, st) {}
MFnCamera::MFnCamera(const MDagPath& p, MStatus* st) : MFnDagNode(p, st) {}
MFn::Type MFnCamera::type() const { return MFn::kCamera; }
static MMatrix cameraMatrix(const MDagPath& p, MSpace::Space space) { return space == MSpace::kWorld ? p.inclusiveMatrix() : MMatrix(); }
MPoint MFnCamera::eyePoint(MSpace::Space space, MStatus* st) const {
    if (st) *st = MS::kSuccess;
    MMatrix m = cameraMatrix(path_, space);
    return MPoint(m[3][0], m[3][1], m[3][2]);
}
MVector MFnCamera::viewDirection(MSpace::Space space, MStatus* st) const {
    if (st) *st = MS::kSuccess;
    return (MVector(0, 0, -1) * cameraMatrix(path_, space)).normal();
}
MVector MFnCamera::upDirection(MSpace::Space space, MStatus* st) const {
    if (st) *st = MS::kSuccess;
    return (MVector(0, 1, 0) * cameraMatrix(path_, space)).normal();
}
MVector MFnCamera::rightDirection(MSpace::Space space, MStatus* st) const {
    if (st) *st = MS::kSuccess;
    return (MVector(1, 0, 0) * cameraMatrix(path_, space)).normal();
}
bool MFnCamera::isOrtho(MStatus* st) const {
    std::vector<double> v = numbers(command("getAttr", args1(std::string(path_.fullPathName().asChar()) + ".orthographic"), Json::object(), st));
    return !v.empty() && v[0] != 0.0;
}
double MFnCamera::focalLength(MStatus* st) const {
    std::vector<double> v = numbers(command("getAttr", args1(std::string(path_.fullPathName().asChar()) + ".focalLength"), Json::object(), st));
    return v.empty() ? 35.0 : v[0];
}

// ---------------------------------------------------------------------------------------------------------------------
// MRampAttribute: entries are elements of the array compound, children position, value (or colour) and interpolation.

MRampAttribute::MRampAttribute() {}
MRampAttribute::MRampAttribute(const MPlug& p, MStatus* st) : plug_(p) {
    if (st) *st = p.isNull() ? MStatus(MS::kInvalidParameter) : MStatus(MS::kSuccess);
}
MRampAttribute::MRampAttribute(const MObject& node, const MObject& attr, MStatus* st) : plug_(node, attr) {
    if (st) *st = plug_.isNull() ? MStatus(MS::kInvalidParameter) : MStatus(MS::kSuccess);
}
MRampAttribute::MRampAttribute(const MRampAttribute& o) : plug_(o.plug_) {}
MRampAttribute& MRampAttribute::operator=(const MRampAttribute& o) {
    plug_ = o.plug_;
    return *this;
}
MRampAttribute::~MRampAttribute() {}

namespace {
struct Entry {
    int index;
    double position;
    double value[3];
    int interp;
};
std::vector<Entry> entriesOf(const MPlug& ramp, bool colour) {
    std::vector<Entry> out;
    MIntArray ids;
    ramp.getExistingArrayAttributeIndices(ids);
    for (unsigned int i = 0; i < ids.length(); ++i) {
        MPlug e = ramp.elementByLogicalIndex(ids[i]);
        Entry x;
        x.index = ids[i];
        x.position = e.child(0).asDouble();
        if (colour) {
            MPlug c = e.child(1);
            for (int k = 0; k < 3; ++k) x.value[k] = c.child(k).asDouble();
        } else {
            x.value[0] = e.child(1).asDouble();
            x.value[1] = x.value[2] = 0.0;
        }
        x.interp = e.child(2).asInt();
        out.push_back(x);
    }
    std::sort(out.begin(), out.end(), [](const Entry& a, const Entry& b) { return a.position < b.position; });
    return out;
}
// A ramp at t: None holds, Linear, Smooth and Spline (Catmull-Rom over the neighbouring spans) as remapValue's ramp
// is evaluated; each span takes its interpolation from its left point.
double rampAt(const std::vector<Entry>& pts, double t, int k3) {
    if (pts.empty()) return 0.0;
    if (t <= pts.front().position) return pts.front().value[k3];
    if (t >= pts.back().position) return pts.back().value[k3];
    size_t k = 0;
    for (size_t i = 0; i < pts.size(); ++i)
        if (pts[i].position <= t) k = i;
    const Entry& a = pts[k];
    const Entry& c = pts[k + 1];
    double span = c.position - a.position;
    double u = span > 0 ? (t - a.position) / span : 0.0;
    double av = a.value[k3], cv = c.value[k3];
    switch (a.interp) {
    case 0: return av;
    case 2: {
        double s = u * u * (3.0 - 2.0 * u);
        return av + (cv - av) * s;
    }
    case 3: {
        double before = k > 0 ? pts[k - 1].value[k3] : av;
        double after = k + 2 < pts.size() ? pts[k + 2].value[k3] : cv;
        double t0 = (cv - before) * 0.5, t1 = (after - av) * 0.5;
        double u2 = u * u, u3 = u2 * u;
        return av * (2 * u3 - 3 * u2 + 1) + t0 * (u3 - 2 * u2 + u) + cv * (-2 * u3 + 3 * u2) + t1 * (u3 - u2);
    }
    default: return av + (cv - av) * u;
    }
}
}  // namespace

unsigned int MRampAttribute::getNumEntries(MStatus* st) {
    if (st) *st = MS::kSuccess;
    return (unsigned int)entriesOf(plug_, false).size();
}
void MRampAttribute::getEntries(MIntArray& indexes, MFloatArray& positions, MFloatArray& values, MIntArray& interps, MStatus* st) {
    indexes.clear();
    positions.clear();
    values.clear();
    interps.clear();
    std::vector<Entry> all = entriesOf(plug_, false);
    for (size_t i = 0; i < all.size(); ++i) {
        indexes.append(all[i].index);
        positions.append((float)all[i].position);
        values.append((float)all[i].value[0]);
        interps.append(all[i].interp);
    }
    if (st) *st = MS::kSuccess;
}
void MRampAttribute::getEntries(MIntArray& indexes, MFloatArray& positions, MColorArray& colors, MIntArray& interps, MStatus* st) {
    indexes.clear();
    positions.clear();
    colors.clear();
    interps.clear();
    std::vector<Entry> all = entriesOf(plug_, true);
    for (size_t i = 0; i < all.size(); ++i) {
        indexes.append(all[i].index);
        positions.append((float)all[i].position);
        colors.append(MColor((float)all[i].value[0], (float)all[i].value[1], (float)all[i].value[2]));
        interps.append(all[i].interp);
    }
    if (st) *st = MS::kSuccess;
}
void MRampAttribute::setRamp(const MFloatArray& values, const MFloatArray& positions, const MIntArray& interps, MStatus* st) {
    for (unsigned int i = 0; i < values.length() && i < positions.length(); ++i) {
        MPlug e = plug_.elementByLogicalIndex(i);
        e.child(0).setFloat(positions[i]);
        e.child(1).setFloat(values[i]);
        e.child(2).setInt(i < interps.length() ? interps[i] : 1);
    }
    if (st) *st = MS::kSuccess;
}
void MRampAttribute::setRamp(float value, float position, unsigned int index, MStatus* st) {
    MPlug e = plug_.elementByLogicalIndex(index);
    e.child(0).setFloat(position);
    e.child(1).setFloat(value);
    if (st) *st = MS::kSuccess;
}
void MRampAttribute::addEntries(MFloatArray& positions, MFloatArray& values, MIntArray& interps, MStatus* st) {
    MIntArray ids;
    plug_.getExistingArrayAttributeIndices(ids);
    int next = 0;
    for (unsigned int i = 0; i < ids.length(); ++i) next = std::max(next, ids[i] + 1);
    for (unsigned int i = 0; i < positions.length() && i < values.length(); ++i) {
        MPlug e = plug_.elementByLogicalIndex(next + i);
        e.child(0).setFloat(positions[i]);
        e.child(1).setFloat(values[i]);
        e.child(2).setInt(i < interps.length() ? interps[i] : 1);
    }
    if (st) *st = MS::kSuccess;
}
void MRampAttribute::deleteEntries(MIntArray& indexes, MStatus* st) {
    for (unsigned int i = 0; i < indexes.length(); ++i) {
        Json a = Json::object();
        a.set("b", true);
        command("removeMultiInstance", args1(plug_.elementByLogicalIndex(indexes[i]).azPath()), a, 0);
    }
    if (st) *st = MS::kSuccess;
}
void MRampAttribute::getValueAtPosition(float position, float& value, MStatus* st) {
    value = (float)rampAt(entriesOf(plug_, false), position, 0);
    if (st) *st = MS::kSuccess;
}
void MRampAttribute::getColorAtPosition(float position, MColor& color, MStatus* st) {
    std::vector<Entry> all = entriesOf(plug_, true);
    color = MColor((float)rampAt(all, position, 0), (float)rampAt(all, position, 1), (float)rampAt(all, position, 2));
    if (st) *st = MS::kSuccess;
}
bool MRampAttribute::isCurveRamp(MStatus* st) const {
    if (st) *st = MS::kSuccess;
    return plug_.azSpec() && plug_.azSpec()->children.size() >= 2 && plug_.azSpec()->children[1]->type != "vec3";
}
bool MRampAttribute::isColorRamp(MStatus* st) const {
    if (st) *st = MS::kSuccess;
    return plug_.azSpec() && plug_.azSpec()->children.size() >= 2 && plug_.azSpec()->children[1]->type == "vec3";
}

// The ramp's attributes, named as the reference names them: blendCurve[0].blendCurve_Position (bcp),
// blendCurve_FloatValue (bcfv) or blendCurve_Color (bcc), and blendCurve_Interp (bci).
static MObject makeRamp(const MString& ln, const MString& sn, bool colour, MStatus* st) {
    std::string l = ln.asChar(), s = sn.asChar();
    MFnNumericAttribute n;
    MObject pos = n.create(MString((l + "_Position").c_str()), MString((s + "p").c_str()), MFnNumericData::kFloat, 0.0);
    MObject val;
    if (colour) {
        val = n.createColor(MString((l + "_Color").c_str()), MString((s + "c").c_str()));
    } else {
        val = n.create(MString((l + "_FloatValue").c_str()), MString((s + "fv").c_str()), MFnNumericData::kFloat, 0.0);
    }
    MFnEnumAttribute e;
    MObject interp = e.create(MString((l + "_Interp").c_str()), MString((s + "i").c_str()), 1);
    e.addField("None", 0);
    e.addField("Linear", 1);
    e.addField("Smooth", 2);
    e.addField("Spline", 3);
    MFnCompoundAttribute c;
    MObject ramp = c.create(ln, sn);
    c.addChild(pos);
    c.addChild(val);
    c.addChild(interp);
    c.setArray(true);
    if (st) *st = MS::kSuccess;
    return ramp;
}
MObject MRampAttribute::createCurveRamp(const MString& ln, const MString& sn, MStatus* st) { return makeRamp(ln, sn, false, st); }
MObject MRampAttribute::createColorRamp(const MString& ln, const MString& sn, MStatus* st) { return makeRamp(ln, sn, true, st); }
MObject MRampAttribute::createRamp(const MString& ln, const MString& sn, const MObject&, MStatus* st) { return makeRamp(ln, sn, false, st); }
