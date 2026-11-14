// MFnAnimCurve and MFnSkinCluster: what the scene's commands and queries give, in the reference's terms.
#include "internal.h"
#include <aztek/kit/anim.h>
#include <cmath>
#include <map>

using namespace az;

namespace {

const char* tangentName(MFnAnimCurve::TangentType t) {
    switch (t) {
        case MFnAnimCurve::kTangentFixed: return "fixed";
        case MFnAnimCurve::kTangentLinear: return "linear";
        case MFnAnimCurve::kTangentFlat: return "flat";
        case MFnAnimCurve::kTangentSmooth: return "spline";
        case MFnAnimCurve::kTangentStep: return "step";
        case MFnAnimCurve::kTangentSlow: return "slow";
        case MFnAnimCurve::kTangentFast: return "fast";
        case MFnAnimCurve::kTangentClamped: return "clamped";
        case MFnAnimCurve::kTangentPlateau: return "plateau";
        case MFnAnimCurve::kTangentStepNext: return "stepnext";
        case MFnAnimCurve::kTangentAuto: return "auto";
        default: return 0;
    }
}

MFnAnimCurve::TangentType tangentOf(const std::string& s) {
    static const std::map<std::string, MFnAnimCurve::TangentType> m = {
        {"fixed", MFnAnimCurve::kTangentFixed}, {"linear", MFnAnimCurve::kTangentLinear}, {"flat", MFnAnimCurve::kTangentFlat},
        {"spline", MFnAnimCurve::kTangentSmooth}, {"step", MFnAnimCurve::kTangentStep}, {"slow", MFnAnimCurve::kTangentSlow},
        {"fast", MFnAnimCurve::kTangentFast}, {"clamped", MFnAnimCurve::kTangentClamped}, {"plateau", MFnAnimCurve::kTangentPlateau},
        {"stepnext", MFnAnimCurve::kTangentStepNext}, {"auto", MFnAnimCurve::kTangentAuto}};
    std::map<std::string, MFnAnimCurve::TangentType>::const_iterator it = m.find(s);
    return it == m.end() ? MFnAnimCurve::kTangentGlobal : it->second;
}

const char* infinityName(MFnAnimCurve::InfinityType t) {
    switch (t) {
        case MFnAnimCurve::kLinear: return "linear";
        case MFnAnimCurve::kCycle: return "cycle";
        case MFnAnimCurve::kCycleRelative: return "cycleRelative";
        case MFnAnimCurve::kOscillate: return "oscillate";
        default: return "constant";
    }
}

Json flags(const char* a, const Json& v) {
    Json f = Json::object();
    f.set(a, v);
    return f;
}

// time=(frame, frame) as the commands take it.
Json at(double frame) {
    Json one = Json::array();
    one.push(frame);
    one.push(frame);
    return one;
}
}  // namespace

// ---------------------------------------------------------------------------------------------------------------------
// MFnAnimCurve

MFnAnimCurve::MFnAnimCurve() {}
MFnAnimCurve::MFnAnimCurve(const MObject& o, MStatus* st) : MFnDependencyNode(o, st) {
    if (st && *st && !o.hasFn(MFn::kAnimCurve)) *st = MS::kInvalidParameter;
}
MFnAnimCurve::MFnAnimCurve(const MPlug& plug, MStatus* st) {
    MPlug src = plug.source();
    if (src.isNull() || !src.node().hasFn(MFn::kAnimCurve)) {
        if (st) *st = MS::kInvalidParameter;
        return;
    }
    obj_ = src.node();
    if (st) *st = MS::kSuccess;
}
MFn::Type MFnAnimCurve::type() const { return MFn::kAnimCurve; }

std::string MFnAnimCurve::azName() const { return nodePath(nodeOf(obj_)); }

MFnAnimCurve::AnimCurveType MFnAnimCurve::animCurveType(MStatus* st) const {
    if (st) *st = nodeOf(obj_) >= 0 ? MS::kSuccess : MS::kInvalidParameter;
    switch (fnOfNode(nodeOf(obj_))) {
        case MFn::kAnimCurveTimeToAngular: return kAnimCurveTA;
        case MFn::kAnimCurveTimeToDistance: return kAnimCurveTL;
        case MFn::kAnimCurveTimeToTime: return kAnimCurveTT;
        case MFn::kAnimCurveTimeToUnitless: return kAnimCurveTU;
        case MFn::kAnimCurveUnitlessToAngular: return kAnimCurveUA;
        case MFn::kAnimCurveUnitlessToDistance: return kAnimCurveUL;
        case MFn::kAnimCurveUnitlessToTime: return kAnimCurveUT;
        case MFn::kAnimCurveUnitlessToUnitless: return kAnimCurveUU;
        default: return kAnimCurveUnknown;
    }
}
bool MFnAnimCurve::isTimeInput(MStatus* st) const { return animCurveType(st) <= kAnimCurveTU; }
bool MFnAnimCurve::isUnitlessInput(MStatus* st) const {
    AnimCurveType t = animCurveType(st);
    return t >= kAnimCurveUA && t <= kAnimCurveUU;
}
bool MFnAnimCurve::isWeighted(MStatus* st) const {
    Json f = Json::object();
    f.set("query", true);
    f.set("weightedTangents", true);
    Json v = command("keyTangent", args1(azName()), f, st);
    return v.isArray() ? v.size() && v[(size_t)0].truth() : v.truth();
}
bool MFnAnimCurve::azAngular() const {
    AnimCurveType t = animCurveType();
    return t == kAnimCurveTA || t == kAnimCurveUA;
}

// One column of the keys, as `keyframe -q` gives it: timeChange (times or inputs), valueChange.
std::vector<double> MFnAnimCurve::azColumn(const char* which, MStatus* st) const {
    Json f = Json::object();
    f.set("query", true);
    f.set(which, true);
    return numbers(command("keyframe", args1(azName()), f, st));
}

unsigned int MFnAnimCurve::numKeys(MStatus* st) const { return (unsigned int)azColumn("timeChange", st).size(); }
unsigned int MFnAnimCurve::numKeyframes(MStatus* st) const { return numKeys(st); }

MTime MFnAnimCurve::time(unsigned int i, MStatus* st) {
    std::vector<double> t = azColumn("timeChange", st);
    if (i >= t.size()) {
        if (st) *st = MS::kInvalidParameter;
        return MTime();
    }
    return MTime(t[i], MTime::uiUnit());
}
double MFnAnimCurve::unitlessInput(unsigned int i, MStatus* st) {
    std::vector<double> t = azColumn("floatChange", st);
    if (t.empty()) t = azColumn("timeChange", st);
    if (i >= t.size()) {
        if (st) *st = MS::kInvalidParameter;
        return 0.0;
    }
    return t[i];
}
double MFnAnimCurve::value(unsigned int i, MStatus* st) {
    std::vector<double> v = azColumn("valueChange", st);
    if (i >= v.size()) {
        if (st) *st = MS::kInvalidParameter;
        return 0.0;
    }
    return v[i];
}
double MFnAnimCurve::evaluate(const MTime& t, MStatus* st) {
    double v = 0.0;
    MStatus s = evaluate(t, v);
    if (st) *st = s;
    return v;
}
MStatus MFnAnimCurve::evaluate(const MTime& t, double& v) { return evaluate(t.as(MTime::uiUnit()), v); }
MStatus MFnAnimCurve::evaluate(double input, double& v) {
    Json a = Json::object();
    a.set("id", (int64_t)nodeOf(obj_));
    a.set("frame", input);
    MStatus st;
    Json r = query("anim.eval", a, &st);
    if (!st) return st;
    // The host keeps angles in radians: the reference's internal unit.
    v = r.num();
    return MS::kSuccess;
}
bool MFnAnimCurve::find(const MTime& t, unsigned int& index, MStatus* st) { return find(t.as(MTime::uiUnit()), index, st); }
bool MFnAnimCurve::find(double input, unsigned int& index, MStatus* st) {
    std::vector<double> t = azColumn("timeChange", st);
    for (size_t i = 0; i < t.size(); ++i)
        if (std::fabs(t[i] - input) < 1e-6) {
            index = (unsigned int)i;
            return true;
        }
    return false;
}
unsigned int MFnAnimCurve::findClosest(const MTime& at, MStatus* st) {
    std::vector<double> t = azColumn("timeChange", st);
    double want = at.as(MTime::uiUnit());
    unsigned int best = 0;
    for (size_t i = 1; i < t.size(); ++i)
        if (std::fabs(t[i] - want) < std::fabs(t[best] - want)) best = (unsigned int)i;
    if (t.empty() && st) *st = MS::kFailure;
    return best;
}
unsigned int MFnAnimCurve::addKey(const MTime& t, double value, TangentType in, TangentType out, MAnimCurveChange*, MStatus* st) {
    double frame = t.as(MTime::uiUnit());
    // Set through the plug the curve drives.
    MPlugArray dst;
    MFnDependencyNode(obj_).findPlug("output").connectedTo(dst, false, true);
    if (!dst.length()) {
        if (st) *st = fail(MStatus::kFailure, "(kFailure): the curve drives no plug to key");
        return 0;
    }
    Json f = Json::object();
    f.set("time", frame);
    f.set("value", value);
    if (tangentName(in)) f.set("inTangentType", tangentName(in));
    if (tangentName(out)) f.set("outTangentType", tangentName(out));
    MStatus s;
    command("setKeyframe", args1(dst[0].azPath()), f, &s);
    if (st) *st = s;
    unsigned int i = 0;
    find(frame, i);
    return i;
}
MStatus MFnAnimCurve::remove(unsigned int index, MAnimCurveChange*) {
    std::vector<double> t = azColumn("timeChange", 0);
    if (index >= t.size()) return MS::kInvalidParameter;
    Json f = flags("time", at(t[index]));
    f.set("clear", true);
    MStatus st;
    command("cutKey", args1(azName()), f, &st);
    return st;
}
MStatus MFnAnimCurve::setValue(unsigned int index, double value, MAnimCurveChange*) {
    std::vector<double> t = azColumn("timeChange", 0);
    if (index >= t.size()) return MS::kInvalidParameter;
    Json f = flags("time", at(t[index]));
    f.set("edit", true);
    f.set("absolute", true);
    f.set("valueChange", value);
    MStatus st;
    command("keyframe", args1(azName()), f, &st);
    return st;
}
MStatus MFnAnimCurve::setTime(unsigned int index, const MTime& when, MAnimCurveChange*) {
    std::vector<double> t = azColumn("timeChange", 0);
    if (index >= t.size()) return MS::kInvalidParameter;
    Json f = flags("time", at(t[index]));
    f.set("edit", true);
    f.set("absolute", true);
    f.set("timeChange", when.as(MTime::uiUnit()));
    MStatus st;
    command("keyframe", args1(azName()), f, &st);
    return st;
}
MFnAnimCurve::TangentType MFnAnimCurve::inTangentType(unsigned int index, MStatus* st) {
    std::vector<double> t = azColumn("timeChange", st);
    if (index >= t.size()) {
        if (st) *st = MS::kInvalidParameter;
        return kTangentGlobal;
    }
    Json f = flags("time", at(t[index]));
    f.set("query", true);
    f.set("inTangentType", true);
    Json v = command("keyTangent", args1(azName()), f, st);
    return tangentOf(v.isArray() && v.size() ? v[(size_t)0].str() : v.str());
}
MFnAnimCurve::TangentType MFnAnimCurve::outTangentType(unsigned int index, MStatus* st) {
    std::vector<double> t = azColumn("timeChange", st);
    if (index >= t.size()) {
        if (st) *st = MS::kInvalidParameter;
        return kTangentGlobal;
    }
    Json f = flags("time", at(t[index]));
    f.set("query", true);
    f.set("outTangentType", true);
    Json v = command("keyTangent", args1(azName()), f, st);
    return tangentOf(v.isArray() && v.size() ? v[(size_t)0].str() : v.str());
}
MStatus MFnAnimCurve::setInTangentType(unsigned int index, TangentType ty, MAnimCurveChange*) {
    std::vector<double> t = azColumn("timeChange", 0);
    if (index >= t.size() || !tangentName(ty)) return MS::kInvalidParameter;
    Json f = flags("time", at(t[index]));
    f.set("edit", true);
    f.set("inTangentType", tangentName(ty));
    MStatus st;
    command("keyTangent", args1(azName()), f, &st);
    return st;
}
MStatus MFnAnimCurve::setOutTangentType(unsigned int index, TangentType ty, MAnimCurveChange*) {
    std::vector<double> t = azColumn("timeChange", 0);
    if (index >= t.size() || !tangentName(ty)) return MS::kInvalidParameter;
    Json f = flags("time", at(t[index]));
    f.set("edit", true);
    f.set("outTangentType", tangentName(ty));
    MStatus st;
    command("keyTangent", args1(azName()), f, &st);
    return st;
}
MFnAnimCurve::InfinityType MFnAnimCurve::preInfinityType(MStatus* st) const {
    std::vector<double> v = numbers(command("getAttr", args1(azName() + ".preInfinity"), Json::object(), st));
    return v.empty() ? kConstant : (InfinityType)(int)v[0];
}
MFnAnimCurve::InfinityType MFnAnimCurve::postInfinityType(MStatus* st) const {
    std::vector<double> v = numbers(command("getAttr", args1(azName() + ".postInfinity"), Json::object(), st));
    return v.empty() ? kConstant : (InfinityType)(int)v[0];
}
MStatus MFnAnimCurve::setPreInfinityType(InfinityType t, MAnimCurveChange*) {
    MStatus st;
    command("setInfinity", args1(azName()), flags("preInfinite", infinityName(t)), &st);
    return st;
}
MStatus MFnAnimCurve::setPostInfinityType(InfinityType t, MAnimCurveChange*) {
    MStatus st;
    command("setInfinity", args1(azName()), flags("postInfinite", infinityName(t)), &st);
    return st;
}

// ---------------------------------------------------------------------------------------------------------------------
// MFnSkinCluster

MFnSkinCluster::MFnSkinCluster() {}
MFnSkinCluster::MFnSkinCluster(const MObject& o, MStatus* st) : MFnGeometryFilter(o, st) {
    if (st && *st && !o.hasFn(MFn::kSkinClusterFilter)) *st = MS::kInvalidParameter;
}
MFn::Type MFnSkinCluster::type() const { return MFn::kSkinClusterFilter; }

// The host's skin data: influences as (logical index, node), and each point's (logical index, weight) pairs.
bool MFnSkinCluster::azData(std::vector<std::pair<int, int64_t> >& infs, std::vector<std::vector<double> >& weights, int64_t& geometry) const {
    Json a = Json::object();
    a.set("id", (int64_t)nodeOf(obj_));
    MStatus st;
    Json d = query("skin.data", a, &st);
    if (!st) return false;
    for (size_t i = 0; i < d["influences"].size(); ++i) infs.push_back(std::make_pair((int)d["influences"][i][(size_t)0].integer(), d["influences"][i][(size_t)1].integer()));
    for (size_t p = 0; p < d["weights"].size(); ++p) weights.push_back(numbers(d["weights"][p]));
    geometry = d["geometry"].integer(-1);
    return true;
}

namespace {
// The points a component names, or all of them.
std::vector<int> pointsOf(const MObject& comp, size_t count) {
    std::vector<int> out;
    const std::shared_ptr<ObjectData>& d = comp.azData();
    if (!d || d->complete || d->ids.empty()) {
        for (size_t i = 0; i < count; ++i) out.push_back((int)i);
        return out;
    }
    for (size_t i = 0; i < d->ids.size(); ++i) out.push_back(d->ids[i]);
    return out;
}

double weightOf(const std::vector<double>& pairs, int logical) {
    for (size_t k = 0; k + 1 < pairs.size(); k += 2)
        if ((int)pairs[k] == logical) return pairs[k + 1];
    return 0.0;
}
}  // namespace

unsigned int MFnSkinCluster::influenceObjects(MDagPathArray& out, MStatus* st) const {
    out.clear();
    std::vector<std::pair<int, int64_t> > infs;
    std::vector<std::vector<double> > w;
    int64_t geo;
    if (!azData(infs, w, geo)) {
        if (st) *st = MS::kFailure;
        return 0;
    }
    for (size_t i = 0; i < infs.size(); ++i) out.append(MDagPath::azOf(infs[i].second));
    if (st) *st = MS::kSuccess;
    return out.length();
}
unsigned int MFnSkinCluster::indexForInfluenceObject(const MDagPath& influence, MStatus* st) const {
    std::vector<std::pair<int, int64_t> > infs;
    std::vector<std::vector<double> > w;
    int64_t geo;
    azData(infs, w, geo);
    for (size_t i = 0; i < infs.size(); ++i)
        if (infs[i].second == influence.azNode()) {
            if (st) *st = MS::kSuccess;
            return (unsigned int)infs[i].first;
        }
    if (st) *st = fail(MStatus::kFailure, "(kFailure): Object is not an influence of this skin");
    return 0;
}
MStatus MFnSkinCluster::getWeights(const MDagPath&, const MObject& comps, MDoubleArray& out, unsigned int& count) const {
    out.clear();
    std::vector<std::pair<int, int64_t> > infs;
    std::vector<std::vector<double> > w;
    int64_t geo;
    if (!azData(infs, w, geo)) return MS::kFailure;
    std::vector<int> pts = pointsOf(comps, w.size());
    for (size_t k = 0; k < pts.size(); ++k)
        for (size_t i = 0; i < infs.size(); ++i) out.append(pts[k] >= 0 && (size_t)pts[k] < w.size() ? weightOf(w[pts[k]], infs[i].first) : 0.0);
    count = (unsigned int)infs.size();
    return MS::kSuccess;
}
MStatus MFnSkinCluster::getWeights(const MDagPath& shape, const MObject& comps, unsigned int influence, MDoubleArray& out) const {
    MIntArray one;
    one.append((int)influence);
    return getWeights(shape, comps, one, out);
}
MStatus MFnSkinCluster::getWeights(const MDagPath&, const MObject& comps, MIntArray& which, MDoubleArray& out) const {
    out.clear();
    std::vector<std::pair<int, int64_t> > infs;
    std::vector<std::vector<double> > w;
    int64_t geo;
    if (!azData(infs, w, geo)) return MS::kFailure;
    for (unsigned int j = 0; j < which.length(); ++j)
        if (which[j] < 0 || (size_t)which[j] >= infs.size()) return fail(MStatus::kInvalidParameter, "(kInvalidParameter): influence index out of range");
    std::vector<int> pts = pointsOf(comps, w.size());
    for (size_t k = 0; k < pts.size(); ++k)
        for (unsigned int j = 0; j < which.length(); ++j) out.append(pts[k] >= 0 && (size_t)pts[k] < w.size() ? weightOf(w[pts[k]], infs[which[j]].first) : 0.0);
    return MS::kSuccess;
}
MStatus MFnSkinCluster::setWeights(const MDagPath& shape, const MObject& comps, unsigned int influence, double value, bool normalize, MDoubleArray* old) {
    MIntArray one;
    one.append((int)influence);
    std::vector<std::pair<int, int64_t> > infs;
    std::vector<std::vector<double> > w;
    int64_t geo;
    if (!azData(infs, w, geo)) return MS::kFailure;
    MDoubleArray values;
    std::vector<int> pts = pointsOf(comps, w.size());
    for (size_t k = 0; k < pts.size(); ++k) values.append(value);
    return setWeights(shape, comps, one, values, normalize, old);
}
MStatus MFnSkinCluster::setWeights(const MDagPath& shape, const MObject& comps, MIntArray& which, MDoubleArray& values, bool normalize, MDoubleArray* old) {
    std::vector<std::pair<int, int64_t> > infs;
    std::vector<std::vector<double> > w;
    int64_t geo;
    if (!azData(infs, w, geo)) return MS::kFailure;
    std::vector<int> pts = pointsOf(comps, w.size());
    size_t n = which.length();
    for (size_t j = 0; j < n; ++j)
        if (which[(unsigned int)j] < 0 || (size_t)which[(unsigned int)j] >= infs.size()) return fail(MStatus::kInvalidParameter, "(kInvalidParameter): influence index out of range");
    if (values.length() != pts.size() * n) return fail(MStatus::kInvalidParameter, "(kInvalidParameter): one value for each point and influence");
    if (old) {
        old->clear();
        for (size_t k = 0; k < pts.size(); ++k)
            for (size_t j = 0; j < n; ++j) old->append((size_t)pts[k] < w.size() ? weightOf(w[pts[k]], infs[which[(unsigned int)j]].first) : 0.0);
    }
    std::string skin = nodePath(nodeOf(obj_));
    int64_t owner = shape.azNode() >= 0 ? shape.azNode() : geo;
    std::string ownerPath = nodePath(owner);
    for (size_t k = 0; k < pts.size(); ++k) {
        Json tv = Json::array();
        for (size_t j = 0; j < n; ++j) {
            Json pair = Json::array();
            pair.push(nodePath(infs[which[(unsigned int)j]].second));
            pair.push(values[(unsigned int)(k * n + j)]);
            tv.push(pair);
        }
        Json f = Json::object();
        f.set("transformValue", tv);
        f.set("normalize", normalize);
        Json a = Json::array();
        a.push(skin);
        a.push(ownerPath + ".vtx[" + std::to_string(pts[k]) + "]");
        MStatus st;
        command("skinPercent", a, f, &st);
        if (!st) return st;
    }
    return MS::kSuccess;
}
MStatus MFnSkinCluster::getPointsAffectedByInfluence(const MDagPath& influence, MSelectionList& result, MDoubleArray& weights) {
    result.clear();
    weights.clear();
    std::vector<std::pair<int, int64_t> > infs;
    std::vector<std::vector<double> > w;
    int64_t geo;
    if (!azData(infs, w, geo)) return MS::kFailure;
    MStatus st;
    unsigned int logical = indexForInfluenceObject(influence, &st);
    if (!st) return st;
    MFnSingleIndexedComponent fn;
    MObject comp = fn.create(MFn::kMeshVertComponent);
    for (size_t p = 0; p < w.size(); ++p) {
        double x = weightOf(w[p], (int)logical);
        if (x > 0.0) {
            fn.addElement((int)p);
            weights.append(x);
        }
    }
    result.add(MDagPath::azOf(geo), comp);
    return MS::kSuccess;
}
MStatus MFnSkinCluster::getBlendWeights(const MDagPath&, const MObject& comps, MDoubleArray& out) {
    out.clear();
    std::vector<std::pair<int, int64_t> > infs;
    std::vector<std::vector<double> > w;
    int64_t geo;
    if (!azData(infs, w, geo)) return MS::kFailure;
    std::vector<int> pts = pointsOf(comps, w.size());
    for (size_t k = 0; k < pts.size(); ++k) out.append(0.0);
    return MS::kSuccess;
}
