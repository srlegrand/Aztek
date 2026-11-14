// Aztek plug-in kit: MPlug, MDagPath, MSelectionList, MGlobal, over the host's commands and queries.
#include "internal.h"

#include <algorithm>
#include <set>
#include <cmath>
#include <cstdlib>

using namespace az;

static const double kDeg = 180.0 / 3.14159265358979323846;

namespace az {

// Every number in a value, flattened.
std::vector<double> numbers(const Json& v) {
    std::vector<double> out;
    if (v.isArray()) {
        for (size_t i = 0; i < v.size(); ++i) {
            std::vector<double> k = numbers(v[i]);
            out.insert(out.end(), k.begin(), k.end());
        }
    } else if (v.isNumber()) {
        out.push_back(v.num());
    }
    return out;
}

Json args1(const std::string& a) {
    Json j = Json::array();
    j.push(a);
    return j;
}

// The shared pointer behind a spec's parent: specs only hold a raw pointer up.
static std::shared_ptr<AttrSpec> findIn(const std::shared_ptr<AttrSpec>& at, const AttrSpec* s) {
    if (at.get() == s) return at;
    for (size_t j = 0; j < at->children.size(); ++j) {
        std::shared_ptr<AttrSpec> f = findIn(at->children[j], s);
        if (f) return f;
    }
    return std::shared_ptr<AttrSpec>();
}

std::shared_ptr<AttrSpec> sharedOf(const AttrSpec* s) {
    if (!s) return std::shared_ptr<AttrSpec>();
    for (size_t k = 0; k < nodeClasses().size(); ++k) {
        NodeClass* c = nodeClasses()[k];
        for (size_t i = 0; i < c->attrs.size(); ++i) {
            std::shared_ptr<AttrSpec> f = findIn(c->attrs[i], s);
            if (f) return f;
        }
    }
    const char* builtins[] = {"input", "weightList"};
    for (int i = 0; i < 2; ++i)
        if (builtinAttr(builtins[i]).get() == s) return builtinAttr(builtins[i]);
    return std::shared_ptr<AttrSpec>();
}

// A plug-in attribute of a node by its long or short name.
std::shared_ptr<AttrSpec> specNamed(int64_t node, const std::string& name) {
    NodeClass* c = classOfNode(node);
    if (!c) return std::shared_ptr<AttrSpec>();
    // Top-level attributes first, then children at any depth.
    for (size_t i = 0; i < c->attrs.size(); ++i) {
        const std::shared_ptr<AttrSpec>& s = c->attrs[i];
        if (s->name == name || s->shortName == name) return s;
    }
    struct Walk {
        static std::shared_ptr<AttrSpec> in(const std::shared_ptr<AttrSpec>& s, const std::string& name) {
            for (size_t j = 0; j < s->children.size(); ++j) {
                const std::shared_ptr<AttrSpec>& k = s->children[j];
                if (k->name == name || k->shortName == name) return k;
                std::shared_ptr<AttrSpec> f = in(k, name);
                if (f) return f;
            }
            return std::shared_ptr<AttrSpec>();
        }
    };
    for (size_t i = 0; i < c->attrs.size(); ++i) {
        std::shared_ptr<AttrSpec> f = Walk::in(c->attrs[i], name);
        if (f) return f;
    }
    return std::shared_ptr<AttrSpec>();
}

// node.attr, node.attr[3], node.parent[3].child.
MPlug plugNamed(const std::string& full) {
    size_t dot = full.find('.');
    if (dot == std::string::npos) return MPlug();
    int64_t node = nodeNamed(full.substr(0, dot));
    if (node < 0) return MPlug();
    std::string attr = full.substr(dot + 1);
    // a[1].b[2].c[3]: the indices of the parts before the last are the outer ones, the last part's is its own.
    std::vector<int> outer;
    int element = -1;
    size_t start = 0;
    for (;;) {
        size_t next = attr.find('.', start);
        std::string part = attr.substr(start, next == std::string::npos ? std::string::npos : next - start);
        size_t open = part.find('[');
        int index = open == std::string::npos ? -1 : atoi(part.c_str() + open + 1);
        if (next == std::string::npos) {
            element = index;
            attr = open == std::string::npos ? part : part.substr(0, open);
            break;
        }
        if (index >= 0) outer.push_back(index);
        start = next + 1;
    }
    std::shared_ptr<AttrSpec> spec = specNamed(node, attr);
    return MPlug::azMake(node, spec ? spec->name : attr, spec, element, outer);
}

bool isAngleName(const std::string& a) {
    static const char* names[] = {"rotate", "rotateX", "rotateY", "rotateZ", "r", "rx", "ry", "rz", "jointOrient", "jointOrientX", "jointOrientY",
                                  "jointOrientZ", "jo", "jox", "joy", "joz", "rotateAxis", "rotateAxisX", "rotateAxisY", "rotateAxisZ", "ra", "rax", "ray", "raz"};
    for (size_t i = 0; i < sizeof names / sizeof names[0]; ++i)
        if (a == names[i]) return true;
    return false;
}

}  // namespace az

// ---------------------------------------------------------------------------------------------------------------------
// MPlug

MPlug::MPlug() : node_(-1), element_(-1) {}
MPlug::MPlug(const MObject& node, const MObject& attr) : node_(nodeOf(node)), element_(-1) {
    const std::shared_ptr<ObjectData>& d = attr.azData();
    if (d && d->attr) {
        spec_ = d->attr;
        attr_ = spec_->name;
    } else if (d && !d->str.empty()) {
        attr_ = d->str;
    }
}
MPlug::MPlug(const MPlug& o) : node_(o.node_), attr_(o.attr_), spec_(o.spec_), element_(o.element_), outer_(o.outer_) {}
MPlug::~MPlug() {}
MPlug& MPlug::operator=(const MPlug& o) {
    node_ = o.node_;
    attr_ = o.attr_;
    spec_ = o.spec_;
    element_ = o.element_;
    outer_ = o.outer_;
    return *this;
}
MPlug MPlug::azMake(int64_t node, const std::string& attr, const std::shared_ptr<AttrSpec>& spec, int element, const std::vector<int>& outer) {
    MPlug p;
    p.node_ = node;
    p.attr_ = attr;
    p.spec_ = spec;
    p.element_ = element;
    p.outer_ = outer;
    return p;
}
bool MPlug::operator==(const MPlug& o) const {
    return node_ == o.node_ && attr_ == o.attr_ && element_ == o.element_ && outer_ == o.outer_;
}
bool MPlug::operator==(const MObject& attr) const {
    const std::shared_ptr<ObjectData>& d = attr.azData();
    if (!d || !d->attr) return false;
    if (spec_) return spec_ == d->attr;
    return attr_ == d->attr->name || attr_ == d->attr->shortName;
}
bool MPlug::operator!=(const MPlug& o) const { return !(*this == o); }
bool MPlug::operator!=(const MObject& a) const { return !(*this == a); }

// The array compounds above a plug with their elements, as names: "driverList[0].pose[2]." for long, or short names.
static std::string outerPath(const std::shared_ptr<AttrSpec>& spec, const std::vector<int>& outer, bool useLong) {
    std::string out;
    if (!spec || outer.empty()) return out;
    std::vector<const AttrSpec*> arrays = arrayAncestors(spec.get());
    for (size_t i = 0; i < arrays.size() && i < outer.size(); ++i)
        out += (useLong ? arrays[i]->name : arrays[i]->shortName) + "[" + std::to_string(outer[i]) + "].";
    return out;
}

std::string MPlug::azAttrPath() const {
    std::string out = outerPath(spec_, outer_, true) + attr_;
    if (element_ >= 0) out += "[" + std::to_string(element_) + "]";
    return out;
}
std::string MPlug::azPath() const { return nodePath(node_) + "." + azAttrPath(); }

static Json plugInfo(const MPlug& p, MStatus* st) {
    Json a = Json::object();
    a.set("name", p.azPath());
    return query("plug.info", a, st);
}

static void ok(MStatus* st) { if (st) *st = MS::kSuccess; }

bool MPlug::isNull(MStatus* st) const { ok(st); return node_ < 0 || attr_.empty(); }
MObject MPlug::attribute(MStatus* st) const {
    if (spec_) {
        ok(st);
        return attrObject(spec_);
    }
    MStatus s;
    Json info = plugInfo(*this, &s);
    if (st) *st = s;
    if (!s) return MObject();
    std::shared_ptr<AttrSpec> spec = std::make_shared<AttrSpec>();
    spec->name = info["component"].integer(-1) >= 0 && info["children"].size() ? attr_ : info["long"].str();
    spec->shortName = info["short"].str();
    spec->type = info["type"].str();
    spec->array = info["array"].truth();
    spec->keyable = info["keyable"].truth();
    spec->fn = spec->type == "vec3" ? MFn::kAttribute3Double : spec->type == "matrix" ? MFn::kMatrixAttribute : spec->type == "enum" ? MFn::kEnumAttribute
             : spec->type == "string" || spec->type == "mesh" ? MFn::kTypedAttribute : MFn::kNumericAttribute;
    if (isAngleName(spec->name)) spec->unit = "angle";
    return attrObject(spec);
}
MObject MPlug::node(MStatus* st) const { ok(st); return nodeObject(node_); }
MString MPlug::name(MStatus* st) const {
    ok(st);
    if (isNull()) return MString();
    return MString(nodePath(node_) + "." + azAttrPath());
}
MString MPlug::partialName(bool includeNode, bool, bool, bool, bool, bool useLong, MStatus* st) const {
    ok(st);
    std::string a = attr_;
    if (!useLong) {
        // An attribute added without a short name goes by its long one.
        if (spec_) {
            if (!spec_->shortName.empty()) a = spec_->shortName;
        } else {
            Json info = plugInfo(*this, 0);
            if (info["short"].isString() && !info["short"].str().empty() && info["component"].integer(-1) < 0) a = info["short"].str();
        }
    }
    std::string base = outerPath(spec_, outer_, useLong) + a;
    if (element_ >= 0) base += "[" + std::to_string(element_) + "]";
    if (includeNode) {
        Json args = Json::object();
        args.set("id", node_);
        base = query("node.info", args)["name"].str() + "." + base;
    }
    return MString(base);
}
MString MPlug::info(MStatus* st) const { return name(st); }
bool MPlug::isNetworked(MStatus* st) const { ok(st); return false; }
bool MPlug::isArray(MStatus* st) const {
    ok(st);
    if (element_ >= 0) return false;
    if (spec_) return spec_->array;
    return plugInfo(*this, 0)["array"].truth();
}
bool MPlug::isElement(MStatus* st) const { ok(st); return element_ >= 0; }
bool MPlug::isCompound(MStatus* st) const {
    ok(st);
    if (spec_) return spec_->type == "compound" || spec_->type == "vec3";
    Json info = plugInfo(*this, 0);
    return info["children"].size() > 0 && info["component"].integer(-1) < 0;
}
bool MPlug::isChild(MStatus* st) const {
    ok(st);
    if (spec_) return spec_->parent != 0;
    return plugInfo(*this, 0)["component"].integer(-1) >= 0;
}
bool MPlug::isDynamic(MStatus* st) const { ok(st); return !spec_ && plugInfo(*this, 0)["user"].truth(); }
bool MPlug::isKeyable(MStatus* st) const { return spec_ ? (ok(st), spec_->keyable) : plugInfo(*this, st)["keyable"].truth(); }
MStatus MPlug::setKeyable(bool on) {
    MStatus st;
    command("setAttr", args1(azPath()), Json::object().set("keyable", on), &st);
    return st;
}
bool MPlug::isChannelBox(MStatus* st) const { return plugInfo(*this, st)["channelBox"].truth(); }
MStatus MPlug::setChannelBox(bool on) {
    MStatus st;
    command("setAttr", args1(azPath()), Json::object().set("channelBox", on), &st);
    return st;
}
bool MPlug::isLocked(MStatus* st) const { return plugInfo(*this, st)["locked"].truth(); }
void MPlug::setLocked(bool on, MStatus* st) { command("setAttr", args1(azPath()), Json::object().set("lock", on), st); }

static Json connections(const MPlug& p) {
    Json a = Json::object();
    a.set("name", p.azPath());
    return query("plug.connections", a);
}
bool MPlug::isConnected(MStatus* st) const {
    ok(st);
    Json c = connections(*this);
    return !c["source"].isNull() || c["dests"].size() > 0;
}
bool MPlug::isSource(MStatus* st) const { ok(st); return connections(*this)["dests"].size() > 0; }
bool MPlug::isDestination(MStatus* st) const { ok(st); return !connections(*this)["source"].isNull(); }
bool MPlug::isFreeToChange(bool, bool, MStatus* st) const { return !isLocked(st) && !isDestination(); }
unsigned int MPlug::logicalIndex(MStatus* st) const {
    if (st) *st = element_ >= 0 ? MStatus(MS::kSuccess) : MStatus(MS::kFailure);
    return element_ >= 0 ? (unsigned int)element_ : 0;
}
MPlug MPlug::array(MStatus* st) const {
    if (element_ < 0) {
        if (st) *st = MS::kFailure;
        return MPlug();
    }
    ok(st);
    return azMake(node_, attr_, spec_, -1, outer_);
}
MPlug MPlug::parent(MStatus* st) const {
    if (spec_ && spec_->parent) {
        ok(st);
        std::shared_ptr<AttrSpec> p = sharedOf(spec_->parent);
        // An array compound's element: the last of the outer elements is its own.
        std::vector<int> outer = outer_;
        int element = -1;
        if (spec_->parent->array && !outer.empty()) {
            element = outer.back();
            outer.pop_back();
        }
        return azMake(node_, spec_->parent->name, p, element, outer);
    }
    if (!spec_) {
        Json info = plugInfo(*this, 0);
        if (info["component"].integer(-1) >= 0) {
            ok(st);
            return azMake(node_, info["long"].str(), std::shared_ptr<AttrSpec>(), -1);
        }
    }
    if (st) *st = MS::kFailure;
    return MPlug();
}
MPlug MPlug::child(unsigned int i, MStatus* st) const {
    if (spec_) {
        if (i >= spec_->children.size()) {
            if (st) *st = MS::kInvalidParameter;
            return MPlug();
        }
        ok(st);
        // A child of an array's element carries that element among its outer ones.
        std::vector<int> outer = outer_;
        if (spec_->array && element_ >= 0) outer.push_back(element_);
        return azMake(node_, spec_->children[i]->name, spec_->children[i], -1, outer);
    }
    Json info = plugInfo(*this, 0);
    const Json& kids = info["children"];
    if (i >= kids.size()) {
        if (st) *st = MS::kInvalidParameter;
        return MPlug();
    }
    ok(st);
    return azMake(node_, kids[i].str(), std::shared_ptr<AttrSpec>(), element_);
}
MPlug MPlug::child(const MObject& attr, MStatus* st) const {
    unsigned int n = numChildren();
    for (unsigned int i = 0; i < n; ++i) {
        MPlug c = child(i);
        if (c == attr) {
            ok(st);
            return c;
        }
    }
    if (st) *st = MS::kInvalidParameter;
    return MPlug();
}
unsigned int MPlug::numChildren(MStatus* st) const {
    ok(st);
    if (spec_) return (unsigned int)spec_->children.size();
    Json info = plugInfo(*this, 0);
    return info["component"].integer(-1) >= 0 ? 0 : (unsigned int)info["children"].size();
}

// The elements of the host attribute behind a spec, each as its indices at every array level.
static std::vector<std::vector<int> > hostElements(int64_t node, const AttrSpec* leaf) {
    std::vector<std::vector<int> > out;
    std::vector<const AttrSpec*> arrays = arrayAncestors(leaf);
    std::string name;
    for (size_t i = 0; i < arrays.size(); ++i) name += arrays[i]->name + ".";
    Json a = Json::object();
    a.set("name", nodePath(node) + "." + name + leaf->name);
    std::vector<double> e = numbers(query("plug.elements", a));
    int levels = arrayDepth(leaf);
    for (size_t k = 0; k < e.size(); ++k) out.push_back(unpackPath((int)e[k], levels));
    return out;
}

// The indices at the level below `outer` that any leaf under a spec has.
static void elementsUnder(int64_t node, const AttrSpec* s, const std::vector<int>& outer, std::set<int>& out) {
    if (s->type == "compound") {
        for (size_t i = 0; i < s->children.size(); ++i) elementsUnder(node, s->children[i].get(), outer, out);
        return;
    }
    std::vector<std::vector<int> > all = hostElements(node, s);
    for (size_t k = 0; k < all.size(); ++k)
        if (all[k].size() > outer.size() && std::equal(outer.begin(), outer.end(), all[k].begin())) out.insert(all[k][outer.size()]);
}

unsigned int MPlug::numConnectedChildren(MStatus* st) const {
    ok(st);
    unsigned int n = 0, kids = numChildren();
    for (unsigned int i = 0; i < kids; ++i)
        if (child(i).isConnected()) ++n;
    return n;
}
void MPlug::setAttribute(MObject& attr, MStatus* st) {
    const std::shared_ptr<ObjectData>& d = attr.azData();
    if (!d || !d->attr) {
        if (st) *st = MS::kInvalidParameter;
        return;
    }
    spec_ = d->attr;
    attr_ = spec_->name;
    element_ = -1;
    outer_.clear();
    ok(st);
}

static std::vector<int> elementsOf(const MPlug& p) {
    std::vector<int> out;
    if (p.azSpec() && (p.azSpec()->type == "compound" || arrayDepth(p.azSpec().get()) >= 2)) {
        // An array compound's elements: those of its children; inside arrays, under the plug's outer elements.
        std::set<int> all;
        elementsUnder(p.azNode(), p.azSpec().get(), p.azOuter(), all);
        out.assign(all.begin(), all.end());
        return out;
    }
    Json a = Json::object();
    a.set("name", p.azPath());
    std::vector<double> e = numbers(query("plug.elements", a));
    for (size_t k = 0; k < e.size(); ++k) out.push_back((int)e[k]);
    return out;
}
unsigned int MPlug::numElements(MStatus* st) const { ok(st); return (unsigned int)elementsOf(*this).size(); }
unsigned int MPlug::evaluateNumElements(MStatus* st) const { return numElements(st); }
unsigned int MPlug::numConnectedElements(MStatus* st) const {
    ok(st);
    unsigned int n = 0;
    std::vector<int> ids = elementsOf(*this);
    for (size_t i = 0; i < ids.size(); ++i)
        if (elementByLogicalIndex(ids[i]).isConnected()) ++n;
    return n;
}
unsigned int MPlug::getExistingArrayAttributeIndices(MIntArray& out, MStatus* st) const {
    ok(st);
    out.clear();
    std::vector<int> ids = elementsOf(*this);
    for (size_t i = 0; i < ids.size(); ++i) out.append(ids[i]);
    return out.length();
}
MPlug MPlug::elementByLogicalIndex(unsigned int i, MStatus* st) const {
    ok(st);
    return azMake(node_, attr_, spec_, (int)i, outer_);
}
MPlug MPlug::elementByPhysicalIndex(unsigned int i, MStatus* st) const {
    std::vector<int> ids = elementsOf(*this);
    if (i >= ids.size()) {
        if (st) *st = MS::kInvalidParameter;
        return MPlug();
    }
    return elementByLogicalIndex(ids[i], st);
}
MPlug MPlug::operator[](unsigned int i) const { return elementByPhysicalIndex(i); }
bool MPlug::connectedTo(MPlugArray& out, bool asDst, bool asSrc, MStatus* st) const {
    ok(st);
    out.clear();
    Json c = connections(*this);
    if (asDst && c["source"].isString()) out.append(plugNamed(c["source"].str()));
    if (asSrc)
        for (size_t i = 0; i < c["dests"].size(); ++i) out.append(plugNamed(c["dests"][i].str()));
    return out.length() > 0;
}
MPlug MPlug::source(MStatus* st) const {
    ok(st);
    Json c = connections(*this);
    return c["source"].isString() ? plugNamed(c["source"].str()) : MPlug();
}
MPlug MPlug::sourceWithConversion(MStatus* st) const { return source(st); }
bool MPlug::destinations(MPlugArray& out, MStatus* st) const { return connectedTo(out, false, true, st); }
MStatus MPlug::selectAncestorLogicalIndex(unsigned int i, const MObject& attr) {
    // The level the attribute named is at among the arrays above; the plug's own when it names none of them.
    const std::shared_ptr<ObjectData>& d = attr.azData();
    if (spec_ && d && d->attr) {
        std::vector<const AttrSpec*> arrays = arrayAncestors(spec_.get());
        for (size_t k = 0; k < arrays.size(); ++k)
            if (arrays[k] == d->attr.get()) {
                if (outer_.size() <= k) outer_.resize(k + 1, 0);
                outer_[k] = (int)i;
                return MS::kSuccess;
            }
    }
    if (spec_ && spec_->parent && !spec_->array) {
        if (outer_.empty()) outer_.push_back(0);
        outer_.back() = (int)i;
    } else {
        element_ = (int)i;
    }
    return MS::kSuccess;
}

// Values. Inside its own node's compute a plug reads the data block; elsewhere getAttr.

static bool fromBlock(const MPlug& p, MDataHandle& h) {
    BlockImpl* b = currentBlock();
    if (!b || b->node != p.azNode() || !p.azSpec()) return false;
    h.azBlock = b;
    h.azSpec = p.azSpec();
    h.azElement = p.azOwnElement();
    h.azOuter = p.azOuter();
    return true;
}

MDataHandle MPlug::asMDataHandle(MStatus* st) const {
    MDataHandle h;
    ok(st);
    if (fromBlock(*this, h)) return h;
    // Outside a compute: the plug's numbers now, as getAttr gives them.
    h.azSpec = spec_;
    h.azElement = element_;
    h.azOuter = outer_;
    h.azLoose = numbers(command("getAttr", args1(azPath()), Json::object(), st));
    return h;
}

static bool angled(const MPlug& p) {
    if (p.azSpec()) return p.azSpec()->unit == "angle";
    std::string a = p.azAttrPath();
    size_t k = a.find('[');
    return isAngleName(k == std::string::npos ? a : a.substr(0, k));
}

static double plugNumber(const MPlug& p, MStatus* st) {
    MDataHandle h;
    if (fromBlock(p, h)) {
        ok(st);
        return h.asDouble();
    }
    MStatus s;
    Json v = command("getAttr", args1(p.azPath()), Json::object(), &s);
    if (st) *st = s;
    std::vector<double> n = numbers(v);
    double x = n.empty() ? (v.isString() ? v.num() : 0.0) : n[0];
    return angled(p) ? x / kDeg : x;
}

bool MPlug::asBool(MStatus* st) const { return plugNumber(*this, st) != 0; }
char MPlug::asChar(MStatus* st) const { return (char)plugNumber(*this, st); }
short MPlug::asShort(MStatus* st) const { return (short)plugNumber(*this, st); }
int MPlug::asInt(MStatus* st) const { return (int)std::floor(plugNumber(*this, st) + 0.5); }
int64_t MPlug::asInt64(MStatus* st) const { return (int64_t)std::floor(plugNumber(*this, st) + 0.5); }
float MPlug::asFloat(MStatus* st) const { return (float)plugNumber(*this, st); }
double MPlug::asDouble(MStatus* st) const { return plugNumber(*this, st); }
MString MPlug::asString(MStatus* st) const {
    MDataHandle h;
    if (fromBlock(*this, h)) {
        ok(st);
        return h.asString();
    }
    Json v = command("getAttr", args1(azPath()), Json::object(), st);
    return MString(v.isString() ? v.str() : v.isNull() ? std::string() : v.dump());
}
MAngle MPlug::asMAngle(MStatus* st) const { return MAngle(plugNumber(*this, st), MAngle::kRadians); }
MDistance MPlug::asMDistance(MStatus* st) const { return MDistance(plugNumber(*this, st), MDistance::kCentimeters); }
MTime MPlug::asMTime(MStatus* st) const { return MTime(plugNumber(*this, st), MTime::uiUnit()); }
MObject MPlug::asMObject(MStatus* st) const {
    MDataHandle h;
    if (fromBlock(*this, h)) {
        ok(st);
        return h.data();
    }
    MStatus s;
    Json info = plugInfo(*this, &s);
    if (!s) {
        if (st) *st = s;
        return MObject();
    }
    std::string type = info["type"].str();
    if (type == "mesh") {
        char* err = 0;
        AzMesh* m = host->mesh_of_node(node_, &err);
        take(err);
        if (st) *st = m ? MStatus(MS::kSuccess) : MStatus(MS::kFailure);
        if (!m) return MObject();
        std::shared_ptr<ObjectData> d = std::make_shared<ObjectData>();
        d->type = MFn::kMeshData;
        d->mesh = std::make_shared<MeshData>();
        d->mesh->setHandle(m);
        return MObject(d);
    }
    Json v = command("getAttr", args1(azPath()), Json::object(), st);
    const std::string kind = info["arrayData"].str();
    if (!kind.empty()) {
        MFn::Type t = kind == "doubleArray" ? MFn::kDoubleArrayData : kind == "floatArray" ? MFn::kFloatArrayData : kind == "Int32Array" ? MFn::kIntArrayData
                    : kind == "stringArray" ? MFn::kStringArrayData : kind == "pointArray" ? MFn::kPointArrayData : MFn::kVectorArrayData;
        std::shared_ptr<ObjectData> d = std::make_shared<ObjectData>();
        d->type = t;
        if (t == MFn::kStringArrayData) {
            for (size_t i = 0; i < v.size(); ++i) d->strs.push_back(v[i].text());
        } else {
            d->nums = numbers(v);
        }
        return MObject(d);
    }
    if (type == "matrix") {
        std::vector<double> n = numbers(v);
        MMatrix m;
        if (n.size() == 16)
            for (int r = 0; r < 4; ++r) for (int c = 0; c < 4; ++c) m.matrix[r][c] = n[r * 4 + c];
        MFnMatrixData f;
        return f.create(m);
    }
    if (type == "string") {
        MFnStringData f;
        return f.create(MString(v.text()));
    }
    std::vector<double> n = numbers(v);
    MFnNumericData f;
    MObject o = f.create(n.size() >= 3 ? MFnNumericData::k3Double : MFnNumericData::kDouble);
    o.azData()->nums = n;
    return o;
}
MStatus MPlug::getValue(bool& v) const { MStatus st; v = asBool(&st); return st; }
MStatus MPlug::getValue(int& v) const { MStatus st; v = asInt(&st); return st; }
MStatus MPlug::getValue(short& v) const { MStatus st; v = asShort(&st); return st; }
MStatus MPlug::getValue(float& v) const { MStatus st; v = asFloat(&st); return st; }
MStatus MPlug::getValue(double& v) const { MStatus st; v = asDouble(&st); return st; }
MStatus MPlug::getValue(MString& v) const { MStatus st; v = asString(&st); return st; }
MStatus MPlug::getValue(MObject& v) const { MStatus st; v = asMObject(&st); return st; }
MStatus MPlug::getValue(MAngle& v) const { MStatus st; v = asMAngle(&st); return st; }
MStatus MPlug::getValue(MDistance& v) const { MStatus st; v = asMDistance(&st); return st; }
MStatus MPlug::getValue(MTime& v) const { MStatus st; v = asMTime(&st); return st; }

static MStatus setNumber(MPlug& p, double v) {
    MDataHandle h;
    if (fromBlock(p, h)) {
        h.setDouble(v);
        return MS::kSuccess;
    }
    Json a = args1(p.azPath());
    a.push(angled(p) ? v * kDeg : v);
    MStatus st;
    command("setAttr", a, Json::object(), &st);
    return st;
}
MStatus MPlug::setValue(bool v) { return setNumber(*this, v ? 1 : 0); }
MStatus MPlug::setValue(int v) { return setNumber(*this, v); }
MStatus MPlug::setValue(short v) { return setNumber(*this, v); }
MStatus MPlug::setValue(float v) { return setNumber(*this, v); }
MStatus MPlug::setValue(double v) { return setNumber(*this, v); }
MStatus MPlug::setValue(const MString& v) {
    MDataHandle h;
    if (fromBlock(*this, h)) {
        h.setString(v);
        return MS::kSuccess;
    }
    Json a = args1(azPath());
    a.push(v.asChar());
    MStatus st;
    command("setAttr", a, Json::object().set("type", "string"), &st);
    return st;
}
MStatus MPlug::setValue(const char* v) { return setValue(MString(v)); }
MStatus MPlug::setValue(const MObject& data) {
    MDataHandle h;
    if (fromBlock(*this, h)) return h.set(data);
    const std::shared_ptr<ObjectData>& d = data.azData();
    if (!d) return MS::kInvalidParameter;
    Json a = args1(azPath());
    MStatus st;
    switch (d->type) {
    case MFn::kMatrixData:
        for (size_t i = 0; i < d->nums.size(); ++i) a.push(d->nums[i]);
        command("setAttr", a, Json::object().set("type", "matrix"), &st);
        return st;
    case MFn::kStringData: return setValue(MString(d->str));
    case MFn::kNumericData:
        for (size_t i = 0; i < d->nums.size(); ++i) a.push(angled(*this) ? d->nums[i] * kDeg : d->nums[i]);
        command("setAttr", a, Json::object(), &st);
        return st;
    case MFn::kDoubleArrayData:
    case MFn::kFloatArrayData:
    case MFn::kIntArrayData:
    case MFn::kVectorArrayData:
    case MFn::kPointArrayData: {
        // The count first, as a file has it, then the values: x y z for points and vectors.
        const char* kind = d->type == MFn::kDoubleArrayData ? "doubleArray" : d->type == MFn::kFloatArrayData ? "floatArray"
                         : d->type == MFn::kIntArrayData ? "Int32Array" : d->type == MFn::kVectorArrayData ? "vectorArray" : "pointArray";
        size_t per = d->type == MFn::kPointArrayData ? 4 : d->type == MFn::kVectorArrayData ? 3 : 1;
        size_t n = d->nums.size() / per;
        a.push((int64_t)n);
        for (size_t i = 0; i < n; ++i)
            for (size_t k = 0; k < (per == 4 ? 3 : per); ++k) a.push(d->nums[i * per + k]);
        command("setAttr", a, Json::object().set("type", kind), &st);
        return st;
    }
    case MFn::kStringArrayData:
        a.push((int64_t)d->strs.size());
        for (size_t i = 0; i < d->strs.size(); ++i) a.push(d->strs[i]);
        command("setAttr", a, Json::object().set("type", "stringArray"), &st);
        return st;
    default: return fail(MStatus::kNotImplemented, "setValue: data of that kind can't be set on a scene plug yet");
    }
}
MStatus MPlug::setValue(const MAngle& v) { return setNumber(*this, angled(*this) ? v.asRadians() : v.asDegrees()); }
MStatus MPlug::setValue(const MDistance& v) { return setNumber(*this, v.asCentimeters()); }
MStatus MPlug::setValue(const MTime& v) { return setNumber(*this, v.as(MTime::uiUnit())); }
MStatus MPlug::setBool(bool v) { return setValue(v); }
MStatus MPlug::setChar(char v) { return setNumber(*this, v); }
MStatus MPlug::setShort(short v) { return setValue(v); }
MStatus MPlug::setInt(int v) { return setValue(v); }
MStatus MPlug::setInt64(int64_t v) { return setNumber(*this, (double)v); }
MStatus MPlug::setFloat(float v) { return setValue(v); }
MStatus MPlug::setDouble(double v) { return setValue(v); }
MStatus MPlug::setString(const MString& v) { return setValue(v); }
MStatus MPlug::setMObject(const MObject& v) { return setValue(v); }
MStatus MPlug::setMAngle(const MAngle& v) { return setValue(v); }
MStatus MPlug::setMDistance(const MDistance& v) { return setValue(v); }
MStatus MPlug::setMTime(const MTime& v) { return setValue(v); }

// ---------------------------------------------------------------------------------------------------------------------
// MDagPath

static Json nodeInfo(int64_t id, MStatus* st = 0) {
    Json a = Json::object();
    a.set("id", id);
    Json info = query("node.info", a, st);
    if (st && *st && !info.isObject()) *st = fail(MStatus::kInvalidParameter, "(kInvalidParameter): Object does not exist");
    return info;
}

static bool isShapeNode(int64_t id) {
    Json info = nodeInfo(id);
    const Json& chain = info["chain"];
    for (size_t i = 0; i < chain.size(); ++i)
        if (chain[i].str() == "shape") return true;
    return false;
}

static MMatrix matrixFrom(const Json& v) {
    MMatrix m;
    std::vector<double> n = numbers(v);
    if (n.size() == 16)
        for (int r = 0; r < 4; ++r) for (int c = 0; c < 4; ++c) m.matrix[r][c] = n[r * 4 + c];
    return m;
}

static Json dagMatrices(int64_t id, MStatus* st) {
    Json a = Json::object();
    a.set("id", id);
    return query("dag.matrices", a, st);
}

MDagPath::MDagPath() : node_(-1) {}
MDagPath::MDagPath(const MDagPath& o) : node_(o.node_) {}
MDagPath::~MDagPath() {}
MDagPath& MDagPath::operator=(const MDagPath& o) { node_ = o.node_; return *this; }
bool MDagPath::operator==(const MDagPath& o) const { return node_ == o.node_; }
bool MDagPath::operator!=(const MDagPath& o) const { return node_ != o.node_; }
MDagPath MDagPath::azOf(int64_t node) {
    MDagPath p;
    p.node_ = node;
    return p;
}
MStatus MDagPath::getAPathTo(const MObject& node, MDagPath& path) {
    int64_t id = nodeOf(node);
    if (id < 0 || !nodeInfo(id)["dag"].truth()) return fail(MStatus::kInvalidParameter, "(kInvalidParameter): Object is not a DAG node");
    path.node_ = id;
    return MS::kSuccess;
}
MStatus MDagPath::getAllPathsTo(const MObject& node, MDagPathArray& paths) {
    MDagPath p;
    MStatus st = getAPathTo(node, p);
    paths.clear();
    if (st) paths.append(p);
    return st;
}
bool MDagPath::isValid(MStatus* st) const {
    ok(st);
    if (node_ < 0) return false;
    Json a = Json::object();
    a.set("id", node_);
    return query("node.exists", a).truth();
}
bool MDagPath::hasFn(MFn::Type t, MStatus* st) const { ok(st); return node_ >= 0 && nodeHasFn(node_, t); }
MFn::Type MDagPath::apiType(MStatus* st) const { ok(st); return node_ >= 0 ? fnOfNode(node_) : MFn::kInvalid; }
MObject MDagPath::node(MStatus* st) const {
    if (st) *st = node_ >= 0 ? MStatus(MS::kSuccess) : MStatus(MS::kFailure);
    return nodeObject(node_);
}
MObject MDagPath::transform(MStatus* st) const {
    if (node_ < 0) {
        if (st) *st = MS::kFailure;
        return MObject();
    }
    ok(st);
    if (isShapeNode(node_)) return nodeObject(nodeInfo(node_)["parent"].integer(-1));
    return nodeObject(node_);
}
unsigned int MDagPath::length(MStatus* st) const {
    ok(st);
    std::string full = fullPathName().asChar();
    unsigned int n = 0;
    for (size_t i = 0; i < full.size(); ++i) if (full[i] == '|') ++n;
    return n;
}
MStatus MDagPath::push(const MObject& child) {
    int64_t c = nodeOf(child);
    if (c < 0) return MS::kInvalidParameter;
    node_ = c;
    return MS::kSuccess;
}
MStatus MDagPath::pop(unsigned int levels) {
    for (unsigned int i = 0; i < levels; ++i) {
        int64_t p = nodeInfo(node_)["parent"].integer(-1);
        if (p <= 0 && p != 0) return MS::kFailure;
        node_ = p;
    }
    return MS::kSuccess;
}
unsigned int MDagPath::childCount(MStatus* st) const { ok(st); return (unsigned int)nodeInfo(node_)["children"].size(); }
MObject MDagPath::child(unsigned int i, MStatus* st) const {
    Json kids = nodeInfo(node_)["children"];
    if (i >= kids.size()) {
        if (st) *st = MS::kInvalidParameter;
        return MObject();
    }
    ok(st);
    return nodeObject(kids[i].integer());
}
static std::vector<int64_t> shapesBelow(int64_t id) {
    std::vector<int64_t> out;
    Json kids = nodeInfo(id)["children"];
    for (size_t i = 0; i < kids.size(); ++i) {
        int64_t k = kids[i].integer();
        if (isShapeNode(k) && !nodeInfo(k)["intermediate"].truth()) out.push_back(k);
    }
    return out;
}
MStatus MDagPath::extendToShape() {
    if (isShapeNode(node_)) return MS::kSuccess;
    std::vector<int64_t> s = shapesBelow(node_);
    if (s.size() != 1) return fail(MStatus::kFailure, "(kFailure): no single shape below " + nodePath(node_));
    node_ = s[0];
    return MS::kSuccess;
}
MStatus MDagPath::extendToShapeDirectlyBelow(unsigned int i) {
    std::vector<int64_t> s = shapesBelow(node_);
    if (i >= s.size()) return MS::kInvalidParameter;
    node_ = s[i];
    return MS::kSuccess;
}
MStatus MDagPath::numberOfShapesDirectlyBelow(unsigned int& n) const {
    n = (unsigned int)shapesBelow(node_).size();
    return MS::kSuccess;
}
unsigned int MDagPath::instanceNumber(MStatus* st) const { ok(st); return 0; }
bool MDagPath::isInstanced(MStatus* st) const { ok(st); return false; }
MMatrix MDagPath::inclusiveMatrix(MStatus* st) const { return matrixFrom(dagMatrices(node_, st)["world"]); }
MMatrix MDagPath::exclusiveMatrix(MStatus* st) const { return matrixFrom(dagMatrices(node_, st)["parent"]); }
MMatrix MDagPath::inclusiveMatrixInverse(MStatus* st) const { return inclusiveMatrix(st).inverse(); }
MMatrix MDagPath::exclusiveMatrixInverse(MStatus* st) const { return exclusiveMatrix(st).inverse(); }
MString MDagPath::fullPathName(MStatus* st) const { return MString(nodeInfo(node_, st)["full"].str()); }
MString MDagPath::partialPathName(MStatus* st) const { return MString(nodeInfo(node_, st)["path"].str()); }
bool MDagPath::isVisible(MStatus* st) const {
    MStatus s;
    Json v = command("getAttr", args1(nodePath(node_) + ".visibility"), Json::object(), &s);
    if (st) *st = s;
    return v.truth();
}
MStatus MDagPath::set(const MDagPath& src) { node_ = src.node_; return MS::kSuccess; }

// ---------------------------------------------------------------------------------------------------------------------
// Components from their strings

namespace az {

// pCube1.vtx[0:3] -> the node part, its kind and its ids.
bool parseComponent(const std::string& s, std::string& node, MObject& comp) {
    size_t dot = s.rfind('.');
    if (dot == std::string::npos) return false;
    std::string rest = s.substr(dot + 1);
    size_t open = rest.find('[');
    if (open == std::string::npos) return false;
    std::string kind = rest.substr(0, open);
    MFn::Type t = kind == "vtx" ? MFn::kMeshVertComponent : kind == "e" ? MFn::kMeshEdgeComponent : kind == "f" ? MFn::kMeshPolygonComponent
                : kind == "map" ? MFn::kMeshMapComponent : kind == "cv" ? MFn::kCurveCVComponent : kind == "ep" ? MFn::kCurveEPComponent : MFn::kInvalid;
    if (t == MFn::kInvalid) return false;
    std::shared_ptr<ObjectData> d = std::make_shared<ObjectData>();
    d->type = t;
    node = s.substr(0, dot);
    std::string inner = rest.substr(open + 1, rest.find(']') - open - 1);
    if (inner == "*") {
        d->complete = true;
    } else {
        size_t colon = inner.find(':');
        int a = atoi(inner.c_str());
        int b = colon == std::string::npos ? a : atoi(inner.c_str() + colon + 1);
        for (int i = a; i <= b; ++i) d->ids.push_back(i);
    }
    d->str = node;
    comp = MObject(d);
    return true;
}

std::string componentStrings(const std::string& node, const MObject& comp, std::vector<std::string>& out) {
    const std::shared_ptr<ObjectData>& d = comp.azData();
    if (!d) return std::string();
    const char* kind = d->type == MFn::kMeshVertComponent ? "vtx" : d->type == MFn::kMeshEdgeComponent ? "e" : d->type == MFn::kMeshPolygonComponent ? "f"
                     : d->type == MFn::kMeshMapComponent ? "map" : d->type == MFn::kCurveCVComponent ? "cv" : d->type == MFn::kCurveEPComponent ? "ep" : "vtx";
    if (d->complete) {
        out.push_back(node + "." + kind + "[*]");
        return kind;
    }
    for (size_t i = 0; i < d->ids.size();) {
        size_t j = i;
        while (j + 1 < d->ids.size() && d->ids[j + 1] == d->ids[j] + 1) ++j;
        std::string r = std::to_string(d->ids[i]);
        if (j > i) r += ":" + std::to_string(d->ids[j]);
        out.push_back(node + "." + kind + "[" + r + "]");
        i = j + 1;
    }
    return kind;
}

}  // namespace az

// ---------------------------------------------------------------------------------------------------------------------
// MSelectionList

MSelectionList::MSelectionList() {}
MSelectionList::MSelectionList(const MSelectionList& o) : items_(o.items_), comps_(o.comps_) {}
MSelectionList::~MSelectionList() {}
MSelectionList& MSelectionList::operator=(const MSelectionList& o) {
    items_ = o.items_;
    comps_ = o.comps_;
    return *this;
}
void MSelectionList::azAdd(const std::string& name, const MObject& comp) {
    for (size_t i = 0; i < items_.size(); ++i)
        if (items_[i] == name && comps_[i].isNull() && comp.isNull()) return;
    items_.push_back(name);
    comps_.push_back(comp);
}
MStatus MSelectionList::clear() {
    items_.clear();
    comps_.clear();
    return MS::kSuccess;
}
bool MSelectionList::isEmpty(MStatus* st) const { ok(st); return items_.empty(); }
unsigned int MSelectionList::length(MStatus* st) const { ok(st); return (unsigned int)items_.size(); }

static void addName(MSelectionList& l, const std::string& name) {
    std::string node;
    MObject comp;
    if (parseComponent(name, node, comp)) l.azAdd(node, comp);
    else l.azAdd(name, MObject());
}

MStatus MSelectionList::add(const MString& pattern, bool) {
    std::string p = pattern.asChar();
    MStatus st;
    Json names = command("ls", args1(p), Json::object(), &st);
    if (st && names.size()) {
        for (size_t i = 0; i < names.size(); ++i) addName(*this, names[i].str());
        return MS::kSuccess;
    }
    // A plug: node.attr.
    if (p.find('.') != std::string::npos && !plugNamed(p).isNull()) {
        MStatus s;
        Json a = Json::object();
        a.set("name", p);
        query("plug.info", a, &s);
        if (s) {
            azAdd(p, MObject());
            return MS::kSuccess;
        }
    }
    return fail(MStatus::kInvalidParameter, "(kInvalidParameter): Object " + p + " is invalid");
}
MStatus MSelectionList::add(const MObject& o, bool) {
    int64_t id = nodeOf(o);
    if (id < 0) return MS::kInvalidParameter;
    Json info = nodeInfo(id);
    azAdd(info["dag"].truth() ? info["full"].str() : info["name"].str(), MObject());
    return MS::kSuccess;
}
MStatus MSelectionList::add(const MDagPath& path, const MObject& comp, bool) {
    if (path.azNode() < 0) return MS::kInvalidParameter;
    azAdd(path.fullPathName().asChar(), comp);
    return MS::kSuccess;
}
MStatus MSelectionList::add(const MPlug& plug, bool) {
    if (plug.isNull()) return MS::kInvalidParameter;
    azAdd(plug.azPath(), MObject());
    return MS::kSuccess;
}
MStatus MSelectionList::remove(unsigned int i) {
    if (i >= items_.size()) return MS::kInvalidParameter;
    items_.erase(items_.begin() + i);
    comps_.erase(comps_.begin() + i);
    return MS::kSuccess;
}
MStatus MSelectionList::merge(const MSelectionList& o, MergeStrategy how) {
    for (size_t i = 0; i < o.items_.size(); ++i) {
        size_t k = 0;
        while (k < items_.size() && !(items_[k] == o.items_[i] && comps_[k] == o.comps_[i])) ++k;
        bool have = k < items_.size();
        if (how == kRemoveFromList || (how == kXORWithList && have)) {
            if (have) remove((unsigned int)k);
        } else if (!have) {
            azAdd(o.items_[i], o.comps_[i]);
        }
    }
    return MS::kSuccess;
}
MStatus MSelectionList::toggle(const MDagPath& path, const MObject& comp) {
    MSelectionList one;
    one.add(path, comp);
    return merge(one, kXORWithList);
}
bool MSelectionList::hasItem(const MDagPath& path, const MObject&, MStatus* st) const {
    ok(st);
    std::string full = path.fullPathName().asChar();
    for (size_t i = 0; i < items_.size(); ++i)
        if (nodeNamed(items_[i]) == path.azNode() || items_[i] == full) return true;
    return false;
}
bool MSelectionList::hasItem(const MObject& o, MStatus* st) const {
    ok(st);
    int64_t id = nodeOf(o);
    for (size_t i = 0; i < items_.size(); ++i)
        if (nodeNamed(items_[i]) == id) return true;
    return false;
}

static std::string nodePart(const std::string& item) {
    size_t dot = item.find('.');
    return dot == std::string::npos ? item : item.substr(0, dot);
}

MStatus MSelectionList::getDependNode(unsigned int i, MObject& out) const {
    if (i >= items_.size()) return MS::kInvalidParameter;
    int64_t id = nodeNamed(nodePart(items_[i]));
    if (id < 0) return fail(MStatus::kInvalidParameter, "(kInvalidParameter): " + items_[i] + " no longer exists");
    out = nodeObject(id);
    return MS::kSuccess;
}
MStatus MSelectionList::getDagPath(unsigned int i, MDagPath& out, MObject& comp) const {
    if (i >= items_.size()) return MS::kInvalidParameter;
    MObject node;
    MStatus st = getDependNode(i, node);
    if (!st) return st;
    st = MDagPath::getAPathTo(node, out);
    comp = comps_[i];
    return st;
}
MStatus MSelectionList::getDagPath(unsigned int i, MDagPath& out) const {
    MObject comp;
    return getDagPath(i, out, comp);
}
MStatus MSelectionList::getPlug(unsigned int i, MPlug& out) const {
    if (i >= items_.size()) return MS::kInvalidParameter;
    out = plugNamed(items_[i]);
    return out.isNull() ? MStatus(MS::kInvalidParameter) : MStatus(MS::kSuccess);
}
MStatus MSelectionList::getSelectionStrings(MStringArray& out) const {
    out.clear();
    for (unsigned int i = 0; i < items_.size(); ++i) {
        MStringArray one;
        getSelectionStrings(i, one);
        for (unsigned int k = 0; k < one.length(); ++k) out.append(one[k]);
    }
    return MS::kSuccess;
}
MStatus MSelectionList::getSelectionStrings(unsigned int i, MStringArray& out) const {
    out.clear();
    if (i >= items_.size()) return MS::kInvalidParameter;
    if (comps_[i].isNull()) {
        out.append(MString(items_[i]));
        return MS::kSuccess;
    }
    std::vector<std::string> s;
    componentStrings(items_[i], comps_[i], s);
    for (size_t k = 0; k < s.size(); ++k) out.append(MString(s[k]));
    return MS::kSuccess;
}

// ---------------------------------------------------------------------------------------------------------------------
// MGlobal

MStatus MGlobal::executeCommand(const MString& c, bool, bool) {
    MStatus st;
    mel(c.asChar(), &st);
    if (!st) displayError(st.errorString());
    return st;
}
MStatus MGlobal::executeCommand(const MString& c, int& r, bool, bool) {
    MStatus st;
    Json v = mel(c.asChar(), &st);
    std::vector<double> n = numbers(v);
    r = n.empty() ? (int)v.num() : (int)n[0];
    return st;
}
MStatus MGlobal::executeCommand(const MString& c, double& r, bool, bool) {
    MStatus st;
    Json v = mel(c.asChar(), &st);
    std::vector<double> n = numbers(v);
    r = n.empty() ? v.num() : n[0];
    return st;
}
MStatus MGlobal::executeCommand(const MString& c, MString& r, bool, bool) {
    MStatus st;
    Json v = mel(c.asChar(), &st);
    r = MString(v.isArray() && v.size() ? v[(size_t)0].text() : v.isNull() ? std::string() : v.text());
    return st;
}
MStatus MGlobal::executeCommand(const MString& c, MIntArray& r, bool, bool) {
    MStatus st;
    std::vector<double> n = numbers(mel(c.asChar(), &st));
    r.clear();
    for (size_t i = 0; i < n.size(); ++i) r.append((int)n[i]);
    return st;
}
MStatus MGlobal::executeCommand(const MString& c, MDoubleArray& r, bool, bool) {
    MStatus st;
    std::vector<double> n = numbers(mel(c.asChar(), &st));
    r.clear();
    for (size_t i = 0; i < n.size(); ++i) r.append(n[i]);
    return st;
}
MStatus MGlobal::executeCommand(const MString& c, MStringArray& r, bool, bool) {
    MStatus st;
    Json v = mel(c.asChar(), &st);
    r.clear();
    if (v.isArray())
        for (size_t i = 0; i < v.size(); ++i) r.append(MString(v[i].text()));
    else if (!v.isNull())
        r.append(MString(v.text()));
    return st;
}
MString MGlobal::executeCommandStringResult(const MString& c, bool d, bool u, MStatus* st) {
    MString r;
    MStatus s = executeCommand(c, r, d, u);
    if (st) *st = s;
    return r;
}
MStatus MGlobal::executeCommandOnIdle(const MString& c, bool) {
    MStatus st;
    command("evalDeferred", args1(c.asChar()), Json::object(), &st);
    return st;
}
MStatus MGlobal::executePythonCommand(const MString& c, bool, bool) {
    MStatus st;
    python(c.asChar(), &st);
    return st;
}
MStatus MGlobal::executePythonCommand(const MString& c, int& r, bool, bool) {
    MStatus st;
    r = (int)python(c.asChar(), &st).num();
    return st;
}
MStatus MGlobal::executePythonCommand(const MString& c, double& r, bool, bool) {
    MStatus st;
    r = python(c.asChar(), &st).num();
    return st;
}
MStatus MGlobal::executePythonCommand(const MString& c, MString& r, bool, bool) {
    MStatus st;
    Json v = python(c.asChar(), &st);
    r = MString(v.isNull() ? std::string() : v.text());
    return st;
}
MString MGlobal::executePythonCommandStringResult(const MString& c, bool d, bool u, MStatus* st) {
    MString r;
    MStatus s = executePythonCommand(c, r, d, u);
    if (st) *st = s;
    return r;
}
MStatus MGlobal::executePythonCommandOnIdle(const MString& c, bool) {
    MStatus st;
    command("evalDeferred", args1("python(" + quoted(c.asChar()) + ")"), Json::object(), &st);
    return st;
}
MStatus MGlobal::sourceFile(const MString& f) {
    MStatus st;
    mel("source " + quoted(f.asChar()), &st);
    return st;
}
void MGlobal::displayInfo(const MString& t) { az::log(t.asChar(), 0); }
void MGlobal::displayWarning(const MString& t) { az::log(std::string("// Warning: ") + t.asChar() + " //", 4); }
void MGlobal::displayError(const MString& t) { az::log(std::string("// Error: ") + t.asChar() + " //", 2); }
MStatus MGlobal::getActiveSelectionList(MSelectionList& out, bool) {
    out.clear();
    MStatus st;
    Json names = command("ls", Json::array(), Json::object().set("selection", true), &st);
    for (size_t i = 0; i < names.size(); ++i) addName(out, names[i].str());
    return st;
}
MStatus MGlobal::setActiveSelectionList(const MSelectionList& l, ListAdjustment how) {
    MStringArray names;
    l.getSelectionStrings(names);
    Json a = Json::array();
    for (unsigned int i = 0; i < names.length(); ++i) a.push(names[i]);
    Json f = Json::object();
    switch (how) {
    case kReplaceList:
        if (a.size() == 0) f.set("clear", true);
        else f.set("replace", true);
        break;
    case kXORWithList: f.set("toggle", true); break;
    case kRemoveFromList: f.set("deselect", true); break;
    default: f.set("add", true); break;
    }
    MStatus st;
    command("select", a, f, &st);
    return st;
}
MStatus MGlobal::getSelectionListByName(const MString& name, MSelectionList& out) {
    out.clear();
    return out.add(name);
}
MStatus MGlobal::selectByName(const MString& name, ListAdjustment how) {
    MSelectionList l;
    MStatus st = l.add(name);
    return st ? setActiveSelectionList(l, how) : st;
}
MStatus MGlobal::select(const MObject& o, ListAdjustment how) {
    MSelectionList l;
    MStatus st = l.add(o);
    return st ? setActiveSelectionList(l, how) : st;
}
MStatus MGlobal::select(const MDagPath& p, const MObject& c, ListAdjustment how) {
    MSelectionList l;
    MStatus st = l.add(p, c);
    return st ? setActiveSelectionList(l, how) : st;
}
MStatus MGlobal::clearSelectionList() {
    MStatus st;
    command("select", Json::array(), Json::object().set("clear", true), &st);
    return st;
}
MStatus MGlobal::getLiveList(MSelectionList& out) { out.clear(); return MS::kSuccess; }
MGlobal::MMayaState MGlobal::mayaState(MStatus* st) {
    ok(st);
    // Batch when Aztek runs without its window (--batch), as the reference's mayabatch does.
    MStatus s;
    Json b = command("about", Json::array(), Json::object().set("batch", true), &s);
    return s && b.truth() ? kBatch : kInteractive;
}
int MGlobal::apiVersion() { return MAYA_API_VERSION; }
MString MGlobal::mayaVersion() { return MString("2024"); }
MString MGlobal::mayaName() { return MString("Aztek"); }
MString MGlobal::defaultErrorLogPathName(MStatus* st) { ok(st); return MString(); }
static Json optionVar(const MString& name, bool* exists) {
    MStatus st;
    Json e = command("optionVar", Json::array(), Json::object().set("exists", name.asChar()), &st);
    if (exists) *exists = e.truth();
    if (!e.truth()) return Json();
    return command("optionVar", Json::array(), Json::object().set("query", name.asChar()), &st);
}
bool MGlobal::optionVarIntValue(const MString& n, bool* e) { return optionVar(n, e).num() != 0; }
double MGlobal::optionVarDoubleValue(const MString& n, bool* e) { return optionVar(n, e).num(); }
MString MGlobal::optionVarStringValue(const MString& n, bool* e) { Json v = optionVar(n, e); return MString(v.isNull() ? std::string() : v.text()); }
bool MGlobal::setOptionVarValue(const MString& n, int v) {
    MStatus st;
    Json p = Json::array();
    p.push(n).push(v);
    command("optionVar", Json::array(), Json::object().set("intValue", p), &st);
    return st;
}
bool MGlobal::setOptionVarValue(const MString& n, double v) {
    MStatus st;
    Json p = Json::array();
    p.push(n).push(v);
    command("optionVar", Json::array(), Json::object().set("floatValue", p), &st);
    return st;
}
bool MGlobal::setOptionVarValue(const MString& n, const MString& v) {
    MStatus st;
    Json p = Json::array();
    p.push(n).push(v);
    command("optionVar", Json::array(), Json::object().set("stringValue", p), &st);
    return st;
}
bool MGlobal::removeOptionVar(const MString& n) {
    MStatus st;
    command("optionVar", Json::array(), Json::object().set("remove", n.asChar()), &st);
    return st;
}
MStatus MGlobal::viewFrame(const MTime& t) { return viewFrame(t.as(MTime::uiUnit())); }
MStatus MGlobal::viewFrame(double f) {
    MStatus st;
    Json a = Json::array();
    a.push(f);
    command("currentTime", a, Json::object(), &st);
    return st;
}
bool MGlobal::isUndoing() { return false; }
bool MGlobal::isRedoing() { return false; }
MGlobal::MSelectionMode MGlobal::selectionMode(MStatus* st) { ok(st); return kSelectObjectMode; }
MStatus MGlobal::setSelectionMode(MSelectionMode m) {
    MStatus st;
    command("selectMode", Json::array(), Json::object().set(m == kSelectComponentMode ? "component" : "object", true), &st);
    return st;
}
