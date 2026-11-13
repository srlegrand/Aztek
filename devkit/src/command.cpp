// Aztek plug-in kit: argument lists, syntax, parsers, MPxCommand, and commands as the host runs them.
#include "internal.h"

#include <cmath>
#include <cstdlib>
#include <cstring>

using namespace az;

const unsigned int MArgList::kInvalidArgIndex = 0xFFFFFFFFu;

static MArgList::Item itemOf(const std::string& text) {
    MArgList::Item it;
    it.text = text;
    it.number = false;
    it.value = 0;
    return it;
}

static MArgList::Item itemOf(double v) {
    MArgList::Item it;
    char buf[40];
    if (v == std::floor(v) && std::fabs(v) < 1e15) snprintf(buf, sizeof buf, "%lld", (long long)v);
    else snprintf(buf, sizeof buf, "%.17g", v);
    it.text = buf;
    it.number = true;
    it.value = v;
    return it;
}

static void addJson(MArgList& out, const Json& v) {
    if (v.isArray()) {
        for (size_t i = 0; i < v.size(); ++i) addJson(out, v[i]);
    } else if (v.kind() == Json::Bool) {
        out.azAdd(itemOf(v.truth() ? 1.0 : 0.0));
    } else if (v.isNumber()) {
        out.azAdd(itemOf(v.num()));
    } else if (!v.isNull()) {
        out.azAdd(itemOf(v.text()));
    }
}

static bool numberOf(const MArgList::Item& it, double& out) {
    if (it.number) {
        out = it.value;
        return true;
    }
    const std::string& t = it.text;
    if (t == "true" || t == "on" || t == "yes") { out = 1; return true; }
    if (t == "false" || t == "off" || t == "no") { out = 0; return true; }
    char* end = 0;
    out = strtod(t.c_str(), &end);
    return end && end != t.c_str() && *end == 0;
}

// ---------------------------------------------------------------------------------------------------------------------
// MArgList

MArgList::MArgList() {}
MArgList::MArgList(const MArgList& o) : items_(o.items_) {}
MArgList::~MArgList() {}
MArgList& MArgList::operator=(const MArgList& o) { items_ = o.items_; return *this; }
unsigned int MArgList::length(MStatus* st) const { if (st) *st = MS::kSuccess; return (unsigned int)items_.size(); }

static const MArgList::Item* at(const std::vector<MArgList::Item>& v, unsigned int i, MStatus* st) {
    if (i >= v.size()) {
        if (st) *st = MS::kInvalidParameter;
        return 0;
    }
    if (st) *st = MS::kSuccess;
    return &v[i];
}

static double num(const std::vector<MArgList::Item>& v, unsigned int i, MStatus* st) {
    const MArgList::Item* it = at(v, i, st);
    double d = 0;
    if (it && !numberOf(*it, d) && st) *st = MS::kInvalidParameter;
    return d;
}

bool MArgList::asBool(unsigned int i, MStatus* st) const { return num(items_, i, st) != 0; }
int MArgList::asInt(unsigned int i, MStatus* st) const { return (int)num(items_, i, st); }
unsigned int MArgList::asUnsigned(unsigned int i, MStatus* st) const { return (unsigned int)num(items_, i, st); }
double MArgList::asDouble(unsigned int i, MStatus* st) const { return num(items_, i, st); }
MString MArgList::asString(unsigned int i, MStatus* st) const {
    const Item* it = at(items_, i, st);
    return it ? MString(it->text) : MString();
}
MAngle MArgList::asAngle(unsigned int i, MStatus* st) const { return MAngle(num(items_, i, st), MAngle::uiUnit()); }
MDistance MArgList::asDistance(unsigned int i, MStatus* st) const { return MDistance(num(items_, i, st), MDistance::uiUnit()); }
MTime MArgList::asTime(unsigned int i, MStatus* st) const { return MTime(num(items_, i, st), MTime::uiUnit()); }
MVector MArgList::asVector(unsigned int& i, unsigned int n, MStatus* st) const {
    MVector v;
    MStatus s;
    for (unsigned int k = 0; k < n && k < 3; ++k) {
        v[k] = num(items_, i + k, &s);
        if (!s) break;
    }
    if (st) *st = s;
    if (s) i += n;
    return v;
}
MPoint MArgList::asPoint(unsigned int& i, unsigned int n, MStatus* st) const {
    MPoint p;
    MStatus s;
    for (unsigned int k = 0; k < n && k < 4; ++k) {
        p[k] = num(items_, i + k, &s);
        if (!s) break;
    }
    if (st) *st = s;
    if (s) i += n;
    return p;
}
MMatrix MArgList::asMatrix(unsigned int& i, MStatus* st) const {
    MMatrix m;
    MStatus s;
    for (unsigned int k = 0; k < 16; ++k) {
        m.matrix[k / 4][k % 4] = num(items_, i + k, &s);
        if (!s) break;
    }
    if (st) *st = s;
    if (s) i += 16;
    return m;
}
MStringArray MArgList::asStringArray(unsigned int& i, MStatus* st) const {
    MStringArray out;
    const Item* it = at(items_, i, st);
    if (it) {
        out.append(MString(it->text));
        ++i;
    }
    return out;
}
MIntArray MArgList::asIntArray(unsigned int& i, MStatus* st) const {
    MIntArray out;
    MStatus s;
    double v = num(items_, i, &s);
    if (s) {
        out.append((int)v);
        ++i;
    }
    if (st) *st = s;
    return out;
}
MDoubleArray MArgList::asDoubleArray(unsigned int& i, MStatus* st) const {
    MDoubleArray out;
    MStatus s;
    double v = num(items_, i, &s);
    if (s) {
        out.append(v);
        ++i;
    }
    if (st) *st = s;
    return out;
}
unsigned int MArgList::flagIndex(const char* s, const char* l) const {
    std::string a = s ? s : "", b = l ? l : "";
    if (!a.empty() && a[0] != '-') a = "-" + a;
    if (!b.empty() && b[0] != '-') b = "-" + b;
    for (size_t i = 0; i < items_.size(); ++i)
        if (!items_[i].number && ((!a.empty() && items_[i].text == a) || (!b.empty() && items_[i].text == b))) return (unsigned int)i;
    return kInvalidArgIndex;
}
MArgList& MArgList::addArg(bool v) { items_.push_back(itemOf(v ? 1.0 : 0.0)); return *this; }
MArgList& MArgList::addArg(int v) { items_.push_back(itemOf((double)v)); return *this; }
MArgList& MArgList::addArg(double v) { items_.push_back(itemOf(v)); return *this; }
MArgList& MArgList::addArg(const MString& v) { items_.push_back(itemOf(std::string(v.asChar()))); return *this; }
MArgList& MArgList::addArg(const MVector& v) { for (int k = 0; k < 3; ++k) items_.push_back(itemOf(v[k])); return *this; }
MArgList& MArgList::addArg(const MPoint& v) { for (int k = 0; k < 4; ++k) items_.push_back(itemOf(v[k])); return *this; }
MArgList& MArgList::addArg(const MAngle& v) { items_.push_back(itemOf(v.asUnits(MAngle::uiUnit()))); return *this; }
MArgList& MArgList::addArg(const MDistance& v) { items_.push_back(itemOf(v.asUnits(MDistance::uiUnit()))); return *this; }
MArgList& MArgList::addArg(const MTime& v) { items_.push_back(itemOf(v.as(MTime::uiUnit()))); return *this; }

// ---------------------------------------------------------------------------------------------------------------------
// MSyntax

static std::string bare(const char* s) {
    std::string t = s ? s : "";
    while (!t.empty() && t[0] == '-') t.erase(0, 1);
    return t;
}

MSyntax::MSyntax() : format_(kNone), min_(0), max_(0xFFFFFFFFu), selectionDefault_(false), query_(false), edit_(false) {}
MSyntax::MSyntax(const MSyntax& o)
    : flags_(o.flags_), args_(o.args_), format_(o.format_), min_(o.min_), max_(o.max_), selectionDefault_(o.selectionDefault_), query_(o.query_), edit_(o.edit_) {}
MSyntax::~MSyntax() {}
MSyntax& MSyntax::operator=(const MSyntax& o) {
    flags_ = o.flags_;
    args_ = o.args_;
    format_ = o.format_;
    min_ = o.min_;
    max_ = o.max_;
    selectionDefault_ = o.selectionDefault_;
    query_ = o.query_;
    edit_ = o.edit_;
    return *this;
}
MStatus MSyntax::addFlag(const char* s, const char* l, MArgType a1, MArgType a2, MArgType a3, MArgType a4, MArgType a5, MArgType a6) {
    Flag f;
    f.shortName = bare(s);
    f.longName = bare(l);
    f.multi = false;
    MArgType a[6] = {a1, a2, a3, a4, a5, a6};
    for (int i = 0; i < 6; ++i)
        if (a[i] != kNoArg && a[i] != kInvalidArgType) f.args.push_back(a[i]);
    if (azFlag(f.shortName) || azFlag(f.longName)) return MS::kInvalidParameter;
    flags_.push_back(f);
    return MS::kSuccess;
}
const MSyntax::Flag* MSyntax::azFlag(const std::string& name) const {
    std::string n = bare(name.c_str());
    for (size_t i = 0; i < flags_.size(); ++i)
        if (flags_[i].shortName == n || flags_[i].longName == n) return &flags_[i];
    return 0;
}
MStatus MSyntax::makeFlagMultiUse(const char* name) {
    const Flag* f = azFlag(name ? name : "");
    if (!f) return MS::kInvalidParameter;
    const_cast<Flag*>(f)->multi = true;
    return MS::kSuccess;
}
MStatus MSyntax::makeFlagQueryWithFullArgs(const char*, bool) { return MS::kSuccess; }
MStatus MSyntax::addArg(MArgType t) { args_.push_back(t); return MS::kSuccess; }
void MSyntax::setObjectType(MObjectFormat f, unsigned int mn) { format_ = f; min_ = mn; }
void MSyntax::setObjectType(MObjectFormat f, unsigned int mn, unsigned int mx) { format_ = f; min_ = mn; max_ = mx; }
void MSyntax::setMinObjects(unsigned int n) { min_ = n; }
void MSyntax::setMaxObjects(unsigned int n) { max_ = n; }
unsigned int MSyntax::minObjects() const { return min_; }
unsigned int MSyntax::maxObjects() const { return max_; }
void MSyntax::useSelectionAsDefault(bool on) { selectionDefault_ = on; }
void MSyntax::enableQuery(bool on) { query_ = on; }
void MSyntax::enableEdit(bool on) { edit_ = on; }
bool MSyntax::canQuery() const { return query_; }
bool MSyntax::canEdit() const { return edit_; }

static const char* hostKind(MSyntax::MArgType t) {
    switch (t) {
    case MSyntax::kBoolean: return "bool";
    case MSyntax::kLong:
    case MSyntax::kUnsigned: return "int";
    case MSyntax::kDouble:
    case MSyntax::kDistance:
    case MSyntax::kAngle:
    case MSyntax::kTime: return "float";
    case MSyntax::kString:
    case MSyntax::kSelectionItem: return "string";
    default: return "any";
    }
}

std::string MSyntax::azHostFlags() const {
    Json out = Json::array();
    for (size_t i = 0; i < flags_.size(); ++i) {
        const Flag& f = flags_[i];
        Json j = Json::object();
        j.set("name", f.longName);
        j.set("short", f.shortName);
        if (f.args.empty()) j.set("type", "bool");
        else if (f.args.size() == 1 && !f.multi) j.set("type", hostKind(f.args[0]));
        else j.set("type", "any");
        if (f.args.size() > 1) j.set("arity", (int)f.args.size());
        out.push(j);
    }
    if (query_) {
        Json q = Json::object();
        q.set("name", "query").set("short", "q").set("type", "bool");
        out.push(q);
        Json asked = Json::object();
        asked.set("name", "queried").set("short", "").set("type", "list");
        out.push(asked);
    }
    if (edit_) {
        Json e = Json::object();
        e.set("name", "edit").set("short", "e").set("type", "bool");
        out.push(e);
    }
    return out.dump();
}

// ---------------------------------------------------------------------------------------------------------------------
// MArgParser, MArgDatabase

MArgParser::MArgParser() : query_(false), edit_(false), selectionDefault_(false) {}
MArgParser::~MArgParser() {}

MArgParser::MArgParser(const MSyntax& syntax, const MArgList& args, MStatus* st) : query_(false), edit_(false), selectionDefault_(syntax.azSelectionDefault()) {
    const std::vector<MArgList::Item>& items = args.azItems();
    MStatus result;
    for (size_t i = 0; i < items.size(); ++i) {
        const MArgList::Item& it = items[i];
        if (!it.number && it.text.size() > 1 && it.text[0] == '-' && !isdigit((unsigned char)it.text[1]) && it.text[1] != '.') {
            std::string name = it.text.substr(1);
            if (name == "q" || name == "query") { query_ = true; continue; }
            if (name == "e" || name == "edit") { edit_ = true; continue; }
            const MSyntax::Flag* f = syntax.azFlag(name);
            if (!f) {
                result = fail(MStatus::kInvalidParameter, "Invalid flag: " + it.text);
                continue;
            }
            Use u;
            u.flag = *f;
            u.position = (unsigned int)i;
            size_t n = f->args.size();
            for (size_t k = 0; k < n; ++k) {
                if (i + 1 >= items.size()) {
                    // A query names the flag without its values.
                    if (!query_) result = fail(MStatus::kInvalidParameter, "Flag -" + f->longName + " needs a value");
                    break;
                }
                u.values.push_back(items[++i]);
            }
            uses_.push_back(u);
            continue;
        }
        if (args_.size() < syntax.azArgs().size()) args_.push_back(it);
        else objects_.push_back(it);
    }
    if (syntax.azObjectFormat() == MSyntax::kNone && !objects_.empty() && syntax.azArgs().empty()) {
        // Arguments the syntax doesn't name are its command arguments anyway.
        args_.insert(args_.end(), objects_.begin(), objects_.end());
    }
    if (st) *st = result;
}

const MArgParser::Use* MArgParser::azUse(const char* flag, unsigned int use) const {
    std::string n = bare(flag);
    unsigned int k = 0;
    for (size_t i = 0; i < uses_.size(); ++i) {
        if (uses_[i].flag.shortName == n || uses_[i].flag.longName == n) {
            if (k == use) return &uses_[i];
            ++k;
        }
    }
    return 0;
}

bool MArgParser::isFlagSet(const char* flag, MStatus* st) const {
    if (st) *st = MS::kSuccess;
    std::string n = bare(flag);
    if (n == "q" || n == "query") return query_;
    if (n == "e" || n == "edit") return edit_;
    return azUse(flag, 0) != 0;
}
bool MArgParser::isEdit(MStatus* st) const { if (st) *st = MS::kSuccess; return edit_; }
bool MArgParser::isQuery(MStatus* st) const { if (st) *st = MS::kSuccess; return query_; }
unsigned int MArgParser::numberOfFlagsUsed() const { return (unsigned int)uses_.size() + (query_ ? 1 : 0) + (edit_ ? 1 : 0); }
unsigned int MArgParser::numberOfFlagUses(const char* flag) const {
    unsigned int n = 0;
    while (azUse(flag, n)) ++n;
    return n;
}

const MArgList::Item* MArgParser::azFlagItem(const char* flag, unsigned int i, MStatus* st) const {
    const Use* u = azUse(flag, 0);
    if (!u) {
        if (st) *st = fail(MStatus::kInvalidParameter, std::string("Flag ") + (flag ? flag : "") + " is not set");
        return 0;
    }
    return at(u->values, i, st);
}

#define AZ_FLAG_NUM(T)                                                                                                 \
    MStatus MArgParser::getFlagArgument(const char* flag, unsigned int i, T& v) const {                               \
        MStatus st;                                                                                                    \
        const MArgList::Item* it = azFlagItem(flag, i, &st);                                                           \
        double d = 0;                                                                                                  \
        if (it && !numberOf(*it, d)) st = MS::kInvalidParameter;                                                       \
        if (st) v = (T)d;                                                                                              \
        return st;                                                                                                     \
    }
AZ_FLAG_NUM(int)
AZ_FLAG_NUM(unsigned int)
AZ_FLAG_NUM(double)
MStatus MArgParser::getFlagArgument(const char* flag, unsigned int i, bool& v) const {
    double d = 0;
    MStatus st = getFlagArgument(flag, i, d);
    if (st) v = d != 0;
    return st;
}
MStatus MArgParser::getFlagArgument(const char* flag, unsigned int i, MString& v) const {
    MStatus st;
    const MArgList::Item* it = azFlagItem(flag, i, &st);
    if (it) v = MString(it->text);
    return st;
}
MStatus MArgParser::getFlagArgument(const char* flag, unsigned int i, MDistance& v) const {
    double d = 0;
    MStatus st = getFlagArgument(flag, i, d);
    if (st) v = MDistance(d, MDistance::uiUnit());
    return st;
}
MStatus MArgParser::getFlagArgument(const char* flag, unsigned int i, MAngle& v) const {
    double d = 0;
    MStatus st = getFlagArgument(flag, i, d);
    if (st) v = MAngle(d, MAngle::uiUnit());
    return st;
}
MStatus MArgParser::getFlagArgument(const char* flag, unsigned int i, MTime& v) const {
    double d = 0;
    MStatus st = getFlagArgument(flag, i, d);
    if (st) v = MTime(d, MTime::uiUnit());
    return st;
}
bool MArgParser::flagArgumentBool(const char* f, unsigned int i, MStatus* st) const { bool v = false; MStatus s = getFlagArgument(f, i, v); if (st) *st = s; return v; }
int MArgParser::flagArgumentInt(const char* f, unsigned int i, MStatus* st) const { int v = 0; MStatus s = getFlagArgument(f, i, v); if (st) *st = s; return v; }
double MArgParser::flagArgumentDouble(const char* f, unsigned int i, MStatus* st) const { double v = 0; MStatus s = getFlagArgument(f, i, v); if (st) *st = s; return v; }
MString MArgParser::flagArgumentString(const char* f, unsigned int i, MStatus* st) const { MString v; MStatus s = getFlagArgument(f, i, v); if (st) *st = s; return v; }
MDistance MArgParser::flagArgumentMDistance(const char* f, unsigned int i, MStatus* st) const { MDistance v; MStatus s = getFlagArgument(f, i, v); if (st) *st = s; return v; }
MAngle MArgParser::flagArgumentMAngle(const char* f, unsigned int i, MStatus* st) const { MAngle v; MStatus s = getFlagArgument(f, i, v); if (st) *st = s; return v; }
MTime MArgParser::flagArgumentMTime(const char* f, unsigned int i, MStatus* st) const { MTime v; MStatus s = getFlagArgument(f, i, v); if (st) *st = s; return v; }
MStatus MArgParser::getFlagArgumentPosition(const char* flag, unsigned int use, unsigned int& pos) const {
    const Use* u = azUse(flag, use);
    if (!u) return MS::kInvalidParameter;
    pos = u->position;
    return MS::kSuccess;
}
MStatus MArgParser::getFlagArgumentList(const char* flag, unsigned int use, MArgList& out) const {
    const Use* u = azUse(flag, use);
    if (!u) return MS::kInvalidParameter;
    out = MArgList();
    for (size_t i = 0; i < u->values.size(); ++i) out.azAdd(u->values[i]);
    return MS::kSuccess;
}

#define AZ_CMD_NUM(T)                                                                                                  \
    MStatus MArgParser::getCommandArgument(unsigned int i, T& v) const {                                               \
        MStatus st;                                                                                                    \
        double d = num(args_, i, &st);                                                                                 \
        if (st) v = (T)d;                                                                                              \
        return st;                                                                                                     \
    }
AZ_CMD_NUM(int)
AZ_CMD_NUM(double)
MStatus MArgParser::getCommandArgument(unsigned int i, bool& v) const { double d = 0; MStatus st = getCommandArgument(i, d); if (st) v = d != 0; return st; }
MStatus MArgParser::getCommandArgument(unsigned int i, MString& v) const {
    MStatus st;
    const MArgList::Item* it = at(args_, i, &st);
    if (it) v = MString(it->text);
    return st;
}
MStatus MArgParser::getCommandArgument(unsigned int i, MDistance& v) const { double d = 0; MStatus st = getCommandArgument(i, d); if (st) v = MDistance(d, MDistance::uiUnit()); return st; }
MStatus MArgParser::getCommandArgument(unsigned int i, MAngle& v) const { double d = 0; MStatus st = getCommandArgument(i, d); if (st) v = MAngle(d, MAngle::uiUnit()); return st; }
MStatus MArgParser::getCommandArgument(unsigned int i, MTime& v) const { double d = 0; MStatus st = getCommandArgument(i, d); if (st) v = MTime(d, MTime::uiUnit()); return st; }
bool MArgParser::commandArgumentBool(unsigned int i, MStatus* st) const { bool v = false; MStatus s = getCommandArgument(i, v); if (st) *st = s; return v; }
int MArgParser::commandArgumentInt(unsigned int i, MStatus* st) const { int v = 0; MStatus s = getCommandArgument(i, v); if (st) *st = s; return v; }
double MArgParser::commandArgumentDouble(unsigned int i, MStatus* st) const { double v = 0; MStatus s = getCommandArgument(i, v); if (st) *st = s; return v; }
MString MArgParser::commandArgumentString(unsigned int i, MStatus* st) const { MString v; MStatus s = getCommandArgument(i, v); if (st) *st = s; return v; }

MStatus MArgParser::getObjects(MStringArray& out) const {
    out.clear();
    for (size_t i = 0; i < objects_.size(); ++i) out.append(MString(objects_[i].text));
    if (out.length() == 0 && selectionDefault_) {
        MSelectionList sel;
        MGlobal::getActiveSelectionList(sel);
        sel.getSelectionStrings(out);
    }
    return MS::kSuccess;
}

MArgDatabase::MArgDatabase() {}
MArgDatabase::MArgDatabase(const MSyntax& s, const MArgList& a, MStatus* st) : MArgParser(s, a, st) {}
MArgDatabase::~MArgDatabase() {}
MStatus MArgDatabase::getFlagArgument(const char* flag, unsigned int i, MSelectionList& v) const {
    MString name;
    MStatus st = MArgParser::getFlagArgument(flag, i, name);
    if (st) st = v.add(name);
    return st;
}
MStatus MArgDatabase::getCommandArgument(unsigned int i, MSelectionList& v) const {
    MString name;
    MStatus st = MArgParser::getCommandArgument(i, name);
    if (st) st = v.add(name);
    return st;
}
MStatus MArgDatabase::getObjects(MSelectionList& out) const {
    out.clear();
    MStringArray names;
    MArgParser::getObjects(names);
    MStatus result;
    for (unsigned int i = 0; i < names.length(); ++i) {
        MStatus st = out.add(names[i]);
        if (!st) result = st;
    }
    return result;
}

// ---------------------------------------------------------------------------------------------------------------------
// MPxCommand

Json& az::commandResult() {
    static Json r;
    return r;
}

MPxCommand::MPxCommand() {}
MPxCommand::~MPxCommand() {}
MStatus MPxCommand::doIt(const MArgList&) { return MS::kFailure; }
MStatus MPxCommand::undoIt() { return MS::kSuccess; }
MStatus MPxCommand::redoIt() { return MS::kSuccess; }
bool MPxCommand::isUndoable() const { return false; }
bool MPxCommand::hasSyntax() const { return true; }
MSyntax MPxCommand::syntax() const { return azSyntax_; }
bool MPxCommand::isHistoryOn() const { return true; }
MString MPxCommand::commandString() const { return azCommandString_; }
MStatus MPxCommand::setHistoryOn(bool) { return MS::kSuccess; }
MStatus MPxCommand::setCommandString(const MString& s) { azCommandString_ = s; return MS::kSuccess; }
void MPxCommand::displayInfo(const MString& t) { MGlobal::displayInfo(t); }
void MPxCommand::displayWarning(const MString& t, bool) { MGlobal::displayWarning(t); }
void MPxCommand::displayError(const MString& t, bool) { MGlobal::displayError(t); }
void MPxCommand::clearResult() { commandResult() = Json(); }
void MPxCommand::setResult(unsigned int v) { commandResult() = Json(v); }
void MPxCommand::setResult(int v) { commandResult() = Json(v); }
void MPxCommand::setResult(float v) { commandResult() = Json((double)v); }
void MPxCommand::setResult(double v) { commandResult() = Json(v); }
void MPxCommand::setResult(bool v) { commandResult() = Json(v); }
void MPxCommand::setResult(const char* v) { commandResult() = Json(v); }
void MPxCommand::setResult(const MString& v) { commandResult() = Json(v); }
void MPxCommand::setResult(const MDoubleArray& v) {
    Json a = Json::array();
    for (unsigned int i = 0; i < v.length(); ++i) a.push(v[i]);
    commandResult() = a;
}
void MPxCommand::setResult(const MIntArray& v) {
    Json a = Json::array();
    for (unsigned int i = 0; i < v.length(); ++i) a.push(v[i]);
    commandResult() = a;
}
void MPxCommand::setResult(const MStringArray& v) {
    Json a = Json::array();
    for (unsigned int i = 0; i < v.length(); ++i) a.push(v[i]);
    commandResult() = a;
}
static void append(const Json& v) {
    Json& r = commandResult();
    if (!r.isArray()) {
        Json a = Json::array();
        if (!r.isNull()) a.push(r);
        r = a;
    }
    r.push(v);
}
void MPxCommand::appendToResult(int v) { append(Json(v)); }
void MPxCommand::appendToResult(double v) { append(Json(v)); }
void MPxCommand::appendToResult(bool v) { append(Json(v)); }
void MPxCommand::appendToResult(const char* v) { append(Json(v)); }
void MPxCommand::appendToResult(const MString& v) { append(Json(v)); }
void MPxCommand::appendToResult(const MStringArray& v) { for (unsigned int i = 0; i < v.length(); ++i) append(Json(v[i])); }
bool MPxCommand::isCurrentResultArray() { return commandResult().isArray(); }
MPxCommand::MResultType MPxCommand::currentResultType() {
    const Json& r = commandResult().isArray() && commandResult().size() ? commandResult()[(size_t)0] : commandResult();
    if (r.isString()) return kString;
    if (r.isNumber()) return r.num() == std::floor(r.num()) ? kLong : kDouble;
    return kNoArg;
}
int MPxCommand::currentIntResult(MStatus* st) { if (st) *st = MS::kSuccess; return (int)commandResult().num(); }
double MPxCommand::currentDoubleResult(MStatus* st) { if (st) *st = MS::kSuccess; return commandResult().num(); }
MString MPxCommand::currentStringResult(MStatus* st) { if (st) *st = MS::kSuccess; return MString(commandResult().text()); }

// ---------------------------------------------------------------------------------------------------------------------
// Commands as the host runs them

namespace az {
struct CommandClass {
    std::string name;
    MCreatorFunction creator;
    MSyntax syntax;
    bool hasSyntax;
    AzCommandClass cls;
};
}

static std::vector<CommandClass*>& commandClasses() {
    static std::vector<CommandClass*> v;
    return v;
}

static char* hostString(const std::string& s) { return host ? host->string_new(s.c_str()) : 0; }

struct CmdCall {
    CommandClass* cls;
    MPxCommand* cmd;
    MArgList args;
    int which;  // 0 create, 1 doIt, 2 undoIt, 3 redoIt, 4 destroy
    MStatus status;
    bool undoable;
};

static int runCall(void* p) {
    CmdCall* c = (CmdCall*)p;
    switch (c->which) {
    case 0:
        c->cmd = (MPxCommand*)c->cls->creator();
        if (!c->cmd) return AZ_FAILURE;
        c->cmd->azSyntax_ = c->cls->syntax;
        c->undoable = c->cmd->isUndoable();
        return AZ_OK;
    case 1:
        c->status = c->cmd->doIt(c->args);
        c->undoable = c->cmd->isUndoable();
        break;
    case 2: c->status = c->cmd->undoIt(); break;
    case 3: c->status = c->cmd->redoIt(); break;
    default: delete c->cmd; return AZ_OK;
    }
    return c->status ? AZ_OK : AZ_FAILURE;
}

static int finish(CmdCall& c, int rc, const std::string& guardError, char** error) {
    if (rc == AZ_OK) return AZ_OK;
    std::string e = guardError;
    if (e.empty() && c.status.error()) e = c.status.errorString().asChar();
    if (e.empty()) e = lastError();
    if (error) *error = hostString(c.cls->name + ": " + (e.empty() ? std::string("failed") : e));
    return rc;
}

static void* cmdCreate(void* ctx, int* undoable, char** error) {
    CmdCall c = {(CommandClass*)ctx, 0, MArgList(), 0, MStatus(), false};
    std::string e;
    int rc = guarded(runCall, &c, e);
    if (rc != AZ_OK || !c.cmd) {
        finish(c, rc == AZ_OK ? AZ_FAILURE : rc, e.empty() ? std::string("the creator made nothing") : e, error);
        return 0;
    }
    if (undoable) *undoable = c.undoable ? 1 : 0;
    return c.cmd;
}

static void cmdDestroy(void* ctx, void* inst) {
    CmdCall c = {(CommandClass*)ctx, (MPxCommand*)inst, MArgList(), 4, MStatus(), false};
    std::string e;
    guarded(runCall, &c, e);
}

// The host's call as an MArgList: flags by their long names, with values, then the arguments; asked-about flags of a
// query last, so nothing after them reads as their values.
static MArgList argsOf(CommandClass* cls, const char* callJson) {
    Json call = Json::parse(callJson ? callJson : "{}");
    MArgList out;
    const Json& flags = call["flags"];
    for (std::map<std::string, Json>::const_iterator it = flags.fields().begin(); it != flags.fields().end(); ++it) {
        const std::string& k = it->first;
        if (k == "queried") continue;
        if (k == "query") { if (it->second.truth()) out.azAdd(itemOf(std::string("-q"))); continue; }
        if (k == "edit") { if (it->second.truth()) out.azAdd(itemOf(std::string("-e"))); continue; }
        const MSyntax::Flag* f = cls->syntax.azFlag(k);
        if (!f) continue;
        if (f->args.empty()) {
            if (it->second.truth()) out.azAdd(itemOf("-" + f->longName));
            continue;
        }
        const Json& v = it->second;
        if (f->multi && v.isArray() && f->args.size() >= 1 && v.size() > f->args.size() && v.size() % f->args.size() == 0) {
            // A multi-use flag: its uses' values in one list.
            for (size_t u = 0; u < v.size(); u += f->args.size()) {
                out.azAdd(itemOf("-" + f->longName));
                for (size_t k2 = 0; k2 < f->args.size(); ++k2) addJson(out, v[u + k2]);
            }
            continue;
        }
        out.azAdd(itemOf("-" + f->longName));
        addJson(out, v);
    }
    addJson(out, call["args"]);
    const Json& asked = flags["queried"];
    for (size_t i = 0; i < asked.size(); ++i) {
        const MSyntax::Flag* f = cls->syntax.azFlag(asked[i].text());
        out.azAdd(itemOf("-" + (f ? f->longName : asked[i].text())));
    }
    return out;
}

static int cmdDoIt(void* ctx, void* inst, const char* callJson, int* undoable, char** result, char** error) {
    CommandClass* cls = (CommandClass*)ctx;
    CmdCall c = {cls, (MPxCommand*)inst, argsOf(cls, callJson), 1, MStatus(), false};
    commandResult() = Json();
    std::string e;
    int rc = guarded(runCall, &c, e);
    if (undoable) *undoable = c.undoable ? 1 : 0;
    if (rc == AZ_OK && result && !commandResult().isNull()) *result = hostString(commandResult().dump());
    commandResult() = Json();
    return finish(c, rc, e, error);
}

static int cmdUndo(void* ctx, void* inst, char** error) {
    CmdCall c = {(CommandClass*)ctx, (MPxCommand*)inst, MArgList(), 2, MStatus(), false};
    std::string e;
    return finish(c, guarded(runCall, &c, e), e, error);
}

static int cmdRedo(void* ctx, void* inst, char** error) {
    CmdCall c = {(CommandClass*)ctx, (MPxCommand*)inst, MArgList(), 3, MStatus(), false};
    std::string e;
    return finish(c, guarded(runCall, &c, e), e, error);
}

MStatus MFnPlugin::registerCommand(const MString& name, MCreatorFunction creator, MCreateSyntaxFunction syntax) {
    if (!host || !creator) return fail(MStatus::kInvalidParameter, "registerCommand: no creator");
    CommandClass* c = new CommandClass();
    c->name = name.asChar();
    c->creator = creator;
    c->hasSyntax = syntax != 0;
    if (syntax) c->syntax = syntax();
    c->cls.ctx = c;
    c->cls.create = cmdCreate;
    c->cls.destroy = cmdDestroy;
    c->cls.do_it = cmdDoIt;
    c->cls.undo_it = cmdUndo;
    c->cls.redo_it = cmdRedo;
    std::string err = take(host->register_command(pluginToken, c->name.c_str(), c->syntax.azHostFlags().c_str(), syntax ? 0 : 1, &c->cls));
    if (!err.empty()) {
        delete c;
        return fail(MStatus::kFailure, err);
    }
    commandClasses().push_back(c);
    return MS::kSuccess;
}

MStatus MFnPlugin::deregisterCommand(const MString& name) {
    if (!host) return MS::kFailure;
    std::string err = take(host->deregister_command(pluginToken, name.asChar()));
    if (!err.empty()) return fail(MStatus::kFailure, err);
    // The host may still run undo steps of the command: its class stays until the plug-in goes.
    return MS::kSuccess;
}
