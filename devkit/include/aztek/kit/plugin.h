// Aztek plug-in kit: attributes, data, commands and their arguments, data blocks, nodes, deformers, MFnPlugin.
#ifndef AZTEK_KIT_PLUGIN_H
#define AZTEK_KIT_PLUGIN_H

#include "scene.h"
#include <iosfwd>

namespace az {
struct BlockImpl;
struct NodeClass;
struct CommandClass;
}

class MDataBlock;
class MPxData;
class MItGeometry;

// ---------------------------------------------------------------------------------------------------------------------
// Data

class MFnData : public MFnBase {
public:
    enum Type {
        kInvalid,
        kNumeric,
        kPlugin,
        kPluginGeometry,
        kString,
        kMatrix,
        kStringArray,
        kDoubleArray,
        kFloatArray,
        kIntArray,
        kPointArray,
        kVectorArray,
        kMatrixArray,
        kComponentList,
        kMesh,
        kLattice,
        kNurbsCurve,
        kNurbsSurface,
        kSphere,
        kDynArrayAttrs,
        kDynSweptGeometry,
        kSubdSurface,
        kNObject,
        kNId,
        kAny,
        kLast
    };
    static const char* className() { return "MFnData"; }
};

class MFnNumericData : public MFnData {
public:
    enum Type {
        kInvalid,
        kBoolean,
        kByte,
        kChar,
        kShort,
        k2Short,
        k3Short,
        kLong,
        kInt = kLong,
        k2Long,
        k2Int = k2Long,
        k3Long,
        k3Int = k3Long,
        kInt64,
        kFloat,
        k2Float,
        k3Float,
        kDouble,
        k2Double,
        k3Double,
        k4Double,
        kAddr,
        kLast
    };
    MFnNumericData();
    MFnNumericData(const MObject& o, MStatus* status = 0);
    virtual MFn::Type type() const;
    MObject create(Type t, MStatus* status = 0);
    Type numericType(MStatus* status = 0);
    MStatus getData(int& a, int& b);
    MStatus getData(int& a, int& b, int& c);
    MStatus getData(float& a, float& b);
    MStatus getData(float& a, float& b, float& c);
    MStatus getData(double& a, double& b);
    MStatus getData(double& a, double& b, double& c);
    MStatus getData(double& a, double& b, double& c, double& d);
    MStatus setData(int a, int b);
    MStatus setData(int a, int b, int c);
    MStatus setData(float a, float b);
    MStatus setData(float a, float b, float c);
    MStatus setData(double a, double b);
    MStatus setData(double a, double b, double c);
    MStatus setData(double a, double b, double c, double d);
    // The named forms the reference's API also has.
    MStatus getData2Short(short& a, short& b);
    MStatus getData2Int(int& a, int& b);
    MStatus getData2Float(float& a, float& b);
    MStatus getData2Double(double& a, double& b);
    MStatus getData3Short(short& a, short& b, short& c);
    MStatus getData3Int(int& a, int& b, int& c);
    MStatus getData3Float(float& a, float& b, float& c);
    MStatus getData3Double(double& a, double& b, double& c);
    MStatus getData4Double(double& a, double& b, double& c, double& d);
    MStatus setData2Short(short a, short b);
    MStatus setData2Int(int a, int b);
    MStatus setData2Float(float a, float b);
    MStatus setData2Double(double a, double b);
    MStatus setData3Short(short a, short b, short c);
    MStatus setData3Int(int a, int b, int c);
    MStatus setData3Float(float a, float b, float c);
    MStatus setData3Double(double a, double b, double c);
    MStatus setData4Double(double a, double b, double c, double d);
};

class MFnMatrixData : public MFnData {
public:
    MFnMatrixData();
    MFnMatrixData(const MObject& o, MStatus* status = 0);
    virtual MFn::Type type() const;
    MObject create(const MMatrix& m, MStatus* status = 0);
    MObject create(MStatus* status = 0);
    const MMatrix& matrix(MStatus* status = 0) const;
    MStatus set(const MMatrix& m);
    bool isTransformation(MStatus* status = 0) const;
    MTransformationMatrix transformation(MStatus* status = 0) const;

private:
    mutable MMatrix cache_;
};

class MFnStringData : public MFnData {
public:
    MFnStringData();
    MFnStringData(const MObject& o, MStatus* status = 0);
    virtual MFn::Type type() const;
    MObject create(const MString& s, MStatus* status = 0);
    MObject create(MStatus* status = 0);
    MString string(MStatus* status = 0) const;
    MStatus set(const MString& s);
};

// Array data: what a typed attribute of kDoubleArray, kIntArray and the like holds.
class MFnDoubleArrayData : public MFnData {
public:
    MFnDoubleArrayData();
    MFnDoubleArrayData(const MObject& o, MStatus* status = 0);
    virtual MFn::Type type() const;
    MObject create(const MDoubleArray& values, MStatus* status = 0);
    MObject create(MStatus* status = 0);
    MDoubleArray array(MStatus* status = 0);
    unsigned int length(MStatus* status = 0) const;
    double operator[](unsigned int i) const;
    MStatus set(const MDoubleArray& values);
    MStatus copyTo(MDoubleArray& out) const;
};

class MFnFloatArrayData : public MFnData {
public:
    MFnFloatArrayData();
    MFnFloatArrayData(const MObject& o, MStatus* status = 0);
    virtual MFn::Type type() const;
    MObject create(const MFloatArray& values, MStatus* status = 0);
    MObject create(MStatus* status = 0);
    MFloatArray array(MStatus* status = 0);
    unsigned int length(MStatus* status = 0) const;
    MStatus set(const MFloatArray& values);
    MStatus copyTo(MFloatArray& out) const;
};

class MFnIntArrayData : public MFnData {
public:
    MFnIntArrayData();
    MFnIntArrayData(const MObject& o, MStatus* status = 0);
    virtual MFn::Type type() const;
    MObject create(const MIntArray& values, MStatus* status = 0);
    MObject create(MStatus* status = 0);
    MIntArray array(MStatus* status = 0);
    unsigned int length(MStatus* status = 0) const;
    int operator[](unsigned int i) const;
    MStatus set(const MIntArray& values);
    MStatus copyTo(MIntArray& out) const;
};

class MFnStringArrayData : public MFnData {
public:
    MFnStringArrayData();
    MFnStringArrayData(const MObject& o, MStatus* status = 0);
    virtual MFn::Type type() const;
    MObject create(const MStringArray& values, MStatus* status = 0);
    MObject create(MStatus* status = 0);
    MStringArray array(MStatus* status = 0);
    unsigned int length(MStatus* status = 0) const;
    MStatus set(const MStringArray& values);
    MStatus copyTo(MStringArray& out) const;
};

class MFnPointArrayData : public MFnData {
public:
    MFnPointArrayData();
    MFnPointArrayData(const MObject& o, MStatus* status = 0);
    virtual MFn::Type type() const;
    MObject create(const MPointArray& values, MStatus* status = 0);
    MObject create(MStatus* status = 0);
    MPointArray array(MStatus* status = 0);
    unsigned int length(MStatus* status = 0) const;
    MStatus set(const MPointArray& values);
    MStatus copyTo(MPointArray& out) const;
};

class MFnVectorArrayData : public MFnData {
public:
    MFnVectorArrayData();
    MFnVectorArrayData(const MObject& o, MStatus* status = 0);
    virtual MFn::Type type() const;
    MObject create(const MVectorArray& values, MStatus* status = 0);
    MObject create(MStatus* status = 0);
    MVectorArray array(MStatus* status = 0);
    unsigned int length(MStatus* status = 0) const;
    MStatus set(const MVectorArray& values);
    MStatus copyTo(MVectorArray& out) const;
};

class MFnGeometryData : public MFnData {
public:
    MFnGeometryData();
    MFnGeometryData(const MObject& o, MStatus* status = 0);
    MMatrix matrix(MStatus* status = 0) const;
    MStatus setMatrix(const MMatrix& m);
    bool isIdentity(MStatus* status = 0) const;
};

class MFnMeshData : public MFnGeometryData {
public:
    MFnMeshData();
    MFnMeshData(const MObject& o, MStatus* status = 0);
    virtual MFn::Type type() const;
    MObject create(MStatus* status = 0);
};

// ---------------------------------------------------------------------------------------------------------------------
// Attributes

class MFnAttribute : public MFnBase {
public:
    enum DisconnectBehavior { kDelete, kReset, kNothing };
    MFnAttribute();
    MFnAttribute(const MObject& o, MStatus* status = 0);
    virtual MFn::Type type() const;
    static const char* className() { return "MFnAttribute"; }
    MString name(MStatus* status = 0) const;
    MString shortName(MStatus* status = 0) const;
    MString niceNameOverride(MStatus* status = 0) const;
    MObject parent(MStatus* status = 0) const;
    bool isReadable(MStatus* status = 0) const;
    bool isWritable(MStatus* status = 0) const;
    bool isConnectable(MStatus* status = 0) const;
    bool isStorable(MStatus* status = 0) const;
    bool isCached(MStatus* status = 0) const;
    bool isArray(MStatus* status = 0) const;
    bool isKeyable(MStatus* status = 0) const;
    bool isHidden(MStatus* status = 0) const;
    bool isChannelBoxFlagSet(MStatus* status = 0) const;
    bool isUsedAsColor(MStatus* status = 0) const;
    bool isDynamic(MStatus* status = 0) const;
    bool usesArrayDataBuilder(MStatus* status = 0) const;
    MStatus setReadable(bool on);
    MStatus setWritable(bool on);
    MStatus setConnectable(bool on);
    MStatus setStorable(bool on);
    MStatus setCached(bool on);
    MStatus setArray(bool on);
    MStatus setKeyable(bool on);
    MStatus setHidden(bool on);
    MStatus setChannelBox(bool on);
    MStatus setUsedAsColor(bool on);
    MStatus setUsedAsFilename(bool on);
    MStatus setIndexMatters(bool on);
    MStatus setUsesArrayDataBuilder(bool on);
    MStatus setDisconnectBehavior(DisconnectBehavior b);
    MStatus setAffectsAppearance(bool on);
    MStatus setAffectsWorldSpace(bool on);
    MStatus setWorldSpace(bool on);
    MStatus setInternal(bool on);
    MStatus setRenderSource(bool on);
    MStatus setReadOnly(bool on);
    MStatus setNiceNameOverride(const MString& name);
    MStatus setMin(double v);
    MStatus setMax(double v);
    MStatus setSoftMin(double v);
    MStatus setSoftMax(double v);
    MStatus addToCategory(const MString& category);

protected:
    az::AttrSpec* azSpec(MStatus* status = 0) const;
    MObject azNew(MFn::Type fn, const MString& longName, const MString& shortName, const char* type, MStatus* status);
};

class MFnNumericAttribute : public MFnAttribute {
public:
    MFnNumericAttribute();
    MFnNumericAttribute(const MObject& o, MStatus* status = 0);
    virtual MFn::Type type() const;
    MObject create(const MString& longName, const MString& shortName, MFnNumericData::Type t, double defaultValue = 0.0, MStatus* status = 0);
    MObject create(const MString& longName, const MString& shortName, const MObject& child1, const MObject& child2,
                   const MObject& child3 = MObject::kNullObj, MStatus* status = 0);
    MObject createPoint(const MString& longName, const MString& shortName, MStatus* status = 0);
    MObject createColor(const MString& longName, const MString& shortName, MStatus* status = 0);
    MObject createAddr(const MString& longName, const MString& shortName, void* defaultValue = 0, MStatus* status = 0);
    // One of the two or three numbers of an attribute made as a pair, a triple, a point or a colour.
    MObject child(unsigned int index, MStatus* status = 0) const;
    MFnNumericData::Type unitType(MStatus* status = 0) const;
    MFnNumericData::Type numericType(MStatus* status = 0) const;
    MStatus setDefault(bool v);
    MStatus setDefault(int v);
    MStatus setDefault(float v);
    MStatus setDefault(double v);
    MStatus setDefault(int a, int b);
    MStatus setDefault(int a, int b, int c);
    MStatus setDefault(float a, float b);
    MStatus setDefault(float a, float b, float c);
    MStatus setDefault(double a, double b);
    MStatus setDefault(double a, double b, double c);
    MStatus getDefault(bool& v);
    MStatus getDefault(int& v);
    MStatus getDefault(float& v);
    MStatus getDefault(double& v);
    MStatus getDefault(double& a, double& b, double& c);
    MStatus setMin(int v);
    MStatus setMin(float v);
    MStatus setMin(double v);
    MStatus setMin(double a, double b, double c);
    MStatus setMax(int v);
    MStatus setMax(float v);
    MStatus setMax(double v);
    MStatus setMax(double a, double b, double c);
    MStatus setSoftMin(double v);
    MStatus setSoftMax(double v);
    bool hasMin(MStatus* status = 0) const;
    bool hasMax(MStatus* status = 0) const;
    bool hasSoftMin(MStatus* status = 0) const;
    bool hasSoftMax(MStatus* status = 0) const;
    MStatus getMin(double& v) const;
    MStatus getMax(double& v) const;
    MStatus getSoftMin(double& v) const;
    MStatus getSoftMax(double& v) const;
};

class MFnUnitAttribute : public MFnAttribute {
public:
    enum Type { kInvalid, kAngle, kDistance, kTime, kLast };
    MFnUnitAttribute();
    MFnUnitAttribute(const MObject& o, MStatus* status = 0);
    virtual MFn::Type type() const;
    MObject create(const MString& longName, const MString& shortName, Type t, double defaultValue = 0.0, MStatus* status = 0);
    MObject create(const MString& longName, const MString& shortName, const MAngle& defaultValue, MStatus* status = 0);
    MObject create(const MString& longName, const MString& shortName, const MDistance& defaultValue, MStatus* status = 0);
    MObject create(const MString& longName, const MString& shortName, const MTime& defaultValue, MStatus* status = 0);
    Type unitType(MStatus* status = 0) const;
    MStatus setDefault(double v);
    MStatus setDefault(const MAngle& v);
    MStatus setDefault(const MDistance& v);
    MStatus setDefault(const MTime& v);
    MStatus setMin(double v);
    MStatus setMax(double v);
    MStatus setSoftMin(double v);
    MStatus setSoftMax(double v);
    bool hasMin(MStatus* status = 0) const;
    bool hasMax(MStatus* status = 0) const;
};

class MFnTypedAttribute : public MFnAttribute {
public:
    MFnTypedAttribute();
    MFnTypedAttribute(const MObject& o, MStatus* status = 0);
    virtual MFn::Type type() const;
    MObject create(const MString& longName, const MString& shortName, MFnData::Type t, const MObject& defaultData = MObject::kNullObj, MStatus* status = 0);
    MObject create(const MString& longName, const MString& shortName, MFnData::Type t, MStatus* status);
    MObject create(const MString& longName, const MString& shortName, const MTypeId& id, const MObject& defaultData = MObject::kNullObj, MStatus* status = 0);
    MFnData::Type attrType(MStatus* status = 0) const;
    MStatus setDefault(const MObject& data);
    MStatus getDefault(MObject& data);
};

class MFnEnumAttribute : public MFnAttribute {
public:
    MFnEnumAttribute();
    MFnEnumAttribute(const MObject& o, MStatus* status = 0);
    virtual MFn::Type type() const;
    MObject create(const MString& longName, const MString& shortName, short defaultValue = 0, MStatus* status = 0);
    MStatus addField(const MString& name, short value);
    MString fieldName(short value, MStatus* status = 0) const;
    short fieldIndex(const MString& name, MStatus* status = 0) const;
    MStatus getMin(short& v) const;
    MStatus getMax(short& v) const;
    MStatus setDefault(short v);
    MStatus setDefault(const MString& name);
    MStatus getDefault(short& v) const;
    MString defaultValue(MStatus* status = 0) const;
};

class MFnCompoundAttribute : public MFnAttribute {
public:
    MFnCompoundAttribute();
    MFnCompoundAttribute(const MObject& o, MStatus* status = 0);
    virtual MFn::Type type() const;
    MObject create(const MString& longName, const MString& shortName, MStatus* status = 0);
    MStatus addChild(const MObject& child);
    MStatus removeChild(const MObject& child);
    unsigned int numChildren(MStatus* status = 0) const;
    MObject child(unsigned int i, MStatus* status = 0) const;
};

class MFnMatrixAttribute : public MFnAttribute {
public:
    enum Type { kFloat, kDouble };
    MFnMatrixAttribute();
    MFnMatrixAttribute(const MObject& o, MStatus* status = 0);
    virtual MFn::Type type() const;
    MObject create(const MString& longName, const MString& shortName, Type t = kDouble, MStatus* status = 0);
    MStatus setDefault(const MMatrix& m);
    MStatus setDefault(const MFloatMatrix& m);
};

class MFnMessageAttribute : public MFnAttribute {
public:
    MFnMessageAttribute();
    MFnMessageAttribute(const MObject& o, MStatus* status = 0);
    virtual MFn::Type type() const;
    MObject create(const MString& longName, const MString& shortName, MStatus* status = 0);
};

class MFnGenericAttribute : public MFnAttribute {
public:
    MFnGenericAttribute();
    MFnGenericAttribute(const MObject& o, MStatus* status = 0);
    virtual MFn::Type type() const;
    MObject create(const MString& longName, const MString& shortName, MStatus* status = 0);
    MStatus addNumericDataAccept(MFnNumericData::Type t);
    MStatus addDataAccept(MFnData::Type t);
    MStatus addAccept(MFnNumericData::Type t);
    MStatus addAccept(MFnData::Type t);
};

// ---------------------------------------------------------------------------------------------------------------------
// Commands

class MArgList {
public:
    MArgList();
    MArgList(const MArgList& other);
    ~MArgList();
    MArgList& operator=(const MArgList& other);
    unsigned int length(MStatus* status = 0) const;
    bool asBool(unsigned int i, MStatus* status = 0) const;
    int asInt(unsigned int i, MStatus* status = 0) const;
    unsigned int asUnsigned(unsigned int i, MStatus* status = 0) const;
    double asDouble(unsigned int i, MStatus* status = 0) const;
    MString asString(unsigned int i, MStatus* status = 0) const;
    MAngle asAngle(unsigned int i, MStatus* status = 0) const;
    MDistance asDistance(unsigned int i, MStatus* status = 0) const;
    MTime asTime(unsigned int i, MStatus* status = 0) const;
    MVector asVector(unsigned int& i, unsigned int numElements = 3, MStatus* status = 0) const;
    MPoint asPoint(unsigned int& i, unsigned int numElements = 3, MStatus* status = 0) const;
    MMatrix asMatrix(unsigned int& i, MStatus* status = 0) const;
    MStringArray asStringArray(unsigned int& i, MStatus* status = 0) const;
    MIntArray asIntArray(unsigned int& i, MStatus* status = 0) const;
    MDoubleArray asDoubleArray(unsigned int& i, MStatus* status = 0) const;
    unsigned int flagIndex(const char* shortFlag, const char* longFlag = 0) const;
    MArgList& addArg(bool v);
    MArgList& addArg(int v);
    MArgList& addArg(double v);
    MArgList& addArg(const MString& v);
    MArgList& addArg(const MVector& v);
    MArgList& addArg(const MPoint& v);
    MArgList& addArg(const MAngle& v);
    MArgList& addArg(const MDistance& v);
    MArgList& addArg(const MTime& v);
    static const unsigned int kInvalidArgIndex;

    // The kit's own: each item as text, and whether it was a number.
    struct Item {
        std::string text;
        bool number;
        double value;
    };
    const std::vector<Item>& azItems() const { return items_; }
    void azAdd(const Item& it) { items_.push_back(it); }

private:
    std::vector<Item> items_;
};

class MSyntax {
public:
    enum MArgType { kInvalidArgType, kNoArg, kBoolean, kLong, kDouble, kString, kUnsigned, kDistance, kAngle, kTime, kSelectionItem, kLastArgType };
    enum MObjectFormat { kInvalidObjectFormat, kNone, kStringObjects, kSelectionList, kLastObjectFormat };
    MSyntax();
    MSyntax(const MSyntax& other);
    ~MSyntax();
    MSyntax& operator=(const MSyntax& other);
    MStatus addFlag(const char* shortName, const char* longName, MArgType a1 = kNoArg, MArgType a2 = kNoArg, MArgType a3 = kNoArg,
                    MArgType a4 = kNoArg, MArgType a5 = kNoArg, MArgType a6 = kNoArg);
    MStatus makeFlagMultiUse(const char* name);
    MStatus makeFlagQueryWithFullArgs(const char* name, bool argsOptional);
    MStatus addArg(MArgType t);
    void setObjectType(MObjectFormat format, unsigned int minimum = 0);
    void setObjectType(MObjectFormat format, unsigned int minimum, unsigned int maximum);
    void setMinObjects(unsigned int n);
    void setMaxObjects(unsigned int n);
    unsigned int minObjects() const;
    unsigned int maxObjects() const;
    void useSelectionAsDefault(bool on);
    void enableQuery(bool on = true);
    void enableEdit(bool on = true);
    bool canQuery() const;
    bool canEdit() const;

    // The kit's own.
    struct Flag {
        std::string shortName, longName;
        std::vector<MArgType> args;
        bool multi;
    };
    const Flag* azFlag(const std::string& name) const;
    const std::vector<Flag>& azFlags() const { return flags_; }
    const std::vector<MArgType>& azArgs() const { return args_; }
    MObjectFormat azObjectFormat() const { return format_; }
    bool azSelectionDefault() const { return selectionDefault_; }
    std::string azHostFlags() const;

private:
    std::vector<Flag> flags_;
    std::vector<MArgType> args_;
    MObjectFormat format_;
    unsigned int min_, max_;
    bool selectionDefault_, query_, edit_;
};

class MArgParser {
public:
    MArgParser();
    MArgParser(const MSyntax& syntax, const MArgList& args, MStatus* status = 0);
    virtual ~MArgParser();
    bool isFlagSet(const char* flag, MStatus* status = 0) const;
    bool isEdit(MStatus* status = 0) const;
    bool isQuery(MStatus* status = 0) const;
    unsigned int numberOfFlagsUsed() const;
    unsigned int numberOfFlagUses(const char* flag) const;
    MStatus getFlagArgument(const char* flag, unsigned int i, bool& v) const;
    MStatus getFlagArgument(const char* flag, unsigned int i, int& v) const;
    MStatus getFlagArgument(const char* flag, unsigned int i, unsigned int& v) const;
    MStatus getFlagArgument(const char* flag, unsigned int i, double& v) const;
    MStatus getFlagArgument(const char* flag, unsigned int i, MString& v) const;
    MStatus getFlagArgument(const char* flag, unsigned int i, MDistance& v) const;
    MStatus getFlagArgument(const char* flag, unsigned int i, MAngle& v) const;
    MStatus getFlagArgument(const char* flag, unsigned int i, MTime& v) const;
    bool flagArgumentBool(const char* flag, unsigned int i, MStatus* status = 0) const;
    int flagArgumentInt(const char* flag, unsigned int i, MStatus* status = 0) const;
    double flagArgumentDouble(const char* flag, unsigned int i, MStatus* status = 0) const;
    MString flagArgumentString(const char* flag, unsigned int i, MStatus* status = 0) const;
    MDistance flagArgumentMDistance(const char* flag, unsigned int i, MStatus* status = 0) const;
    MAngle flagArgumentMAngle(const char* flag, unsigned int i, MStatus* status = 0) const;
    MTime flagArgumentMTime(const char* flag, unsigned int i, MStatus* status = 0) const;
    MStatus getFlagArgumentPosition(const char* flag, unsigned int use, unsigned int& position) const;
    MStatus getFlagArgumentList(const char* flag, unsigned int use, MArgList& out) const;
    MStatus getCommandArgument(unsigned int i, bool& v) const;
    MStatus getCommandArgument(unsigned int i, int& v) const;
    MStatus getCommandArgument(unsigned int i, double& v) const;
    MStatus getCommandArgument(unsigned int i, MString& v) const;
    MStatus getCommandArgument(unsigned int i, MDistance& v) const;
    MStatus getCommandArgument(unsigned int i, MAngle& v) const;
    MStatus getCommandArgument(unsigned int i, MTime& v) const;
    bool commandArgumentBool(unsigned int i, MStatus* status = 0) const;
    int commandArgumentInt(unsigned int i, MStatus* status = 0) const;
    double commandArgumentDouble(unsigned int i, MStatus* status = 0) const;
    MString commandArgumentString(unsigned int i, MStatus* status = 0) const;
    MStatus getObjects(MStringArray& out) const;

protected:
    struct Use {
        // A copy: the syntax a parser is made from is often a temporary (MPxCommand::syntax() returns one).
        MSyntax::Flag flag;
        std::vector<MArgList::Item> values;
        unsigned int position;
    };
    const Use* azUse(const char* flag, unsigned int use) const;
    const MArgList::Item* azFlagItem(const char* flag, unsigned int i, MStatus* status) const;
    std::vector<Use> uses_;
    std::vector<MArgList::Item> args_;
    std::vector<MArgList::Item> objects_;
    bool query_, edit_;
    bool selectionDefault_;
};

class MArgDatabase : public MArgParser {
public:
    MArgDatabase();
    MArgDatabase(const MSyntax& syntax, const MArgList& args, MStatus* status = 0);
    virtual ~MArgDatabase();
    using MArgParser::getFlagArgument;
    using MArgParser::getCommandArgument;
    using MArgParser::getObjects;
    MStatus getFlagArgument(const char* flag, unsigned int i, MSelectionList& v) const;
    MStatus getCommandArgument(unsigned int i, MSelectionList& v) const;
    MStatus getObjects(MSelectionList& out) const;
};

class MPxCommand {
public:
    enum MResultType { kLong, kDouble, kString, kNoArg };
    MPxCommand();
    virtual ~MPxCommand();
    virtual MStatus doIt(const MArgList& args);
    virtual MStatus undoIt();
    virtual MStatus redoIt();
    virtual bool isUndoable() const;
    virtual bool hasSyntax() const;
    MSyntax syntax() const;
    bool isHistoryOn() const;
    MString commandString() const;
    MStatus setHistoryOn(bool on);
    MStatus setCommandString(const MString& s);
    static void displayInfo(const MString& text);
    static void displayWarning(const MString& text, bool showLineNumber = false);
    static void displayError(const MString& text, bool showLineNumber = false);
    static void clearResult();
    static void setResult(unsigned int v);
    static void setResult(int v);
    static void setResult(float v);
    static void setResult(double v);
    static void setResult(bool v);
    static void setResult(const char* v);
    static void setResult(const MString& v);
    static void setResult(const MDoubleArray& v);
    static void setResult(const MIntArray& v);
    static void setResult(const MStringArray& v);
    static void appendToResult(int v);
    static void appendToResult(double v);
    static void appendToResult(bool v);
    static void appendToResult(const char* v);
    static void appendToResult(const MString& v);
    static void appendToResult(const MStringArray& v);
    static bool isCurrentResultArray();
    static MResultType currentResultType();
    static int currentIntResult(MStatus* status = 0);
    static double currentDoubleResult(MStatus* status = 0);
    static MString currentStringResult(MStatus* status = 0);

    // The kit's own.
    MSyntax azSyntax_;
    MString azCommandString_;
};

// ---------------------------------------------------------------------------------------------------------------------
// Data blocks

class MDGContext {
public:
    MDGContext();
    MDGContext(const MTime& t);
    bool isNormal(MStatus* status = 0) const;
    bool isCurrent(MStatus* status = 0) const;
    MStatus getTime(MTime& t) const;
    MTime getTime(MStatus* status = 0) const;
    static MDGContext fsNormal;

private:
    bool normal_;
    MTime time_;
};

class MDataHandle {
public:
    MDataHandle();
    MDataHandle(const MDataHandle& other);
    MDataHandle& operator=(const MDataHandle& other);
    bool isNumeric() const;
    bool isGeneric(bool& isNumeric, bool& isNull) const;
    MFnNumericData::Type numericType() const;
    MFnData::Type type() const;
    MStatus acceptedTypeIds(MUintArray& out) const;
    bool& asBool() const;
    char& asChar() const;
    unsigned char& asUChar() const;
    short& asShort() const;
    int& asInt() const;
    int& asLong() const;
    int64_t& asInt64() const;
    float& asFloat() const;
    double& asDouble() const;
    MDistance asDistance() const;
    MAngle asAngle() const;
    MTime asTime() const;
    short2& asShort2() const;
    int2& asInt2() const;
    int2& asLong2() const;
    float2& asFloat2() const;
    double2& asDouble2() const;
    short3& asShort3() const;
    int3& asInt3() const;
    int3& asLong3() const;
    float3& asFloat3() const;
    double3& asDouble3() const;
    MVector& asVector() const;
    MFloatVector& asFloatVector() const;
    MMatrix& asMatrix() const;
    MFloatMatrix& asFloatMatrix() const;
    MString& asString() const;
    MObject asMesh() const;
    MObject asMeshTransformed() const;
    MObject asNurbsCurve() const;
    MObject asNurbsCurveTransformed() const;
    MObject data() const;
    MStatus setData(const MObject& data);
    double asGenericDouble() const;
    MMatrix geometryTransformMatrix() const;
    MDataHandle child(const MPlug& plug);
    MDataHandle child(const MObject& attr);
    void set(bool v);
    void set(char v);
    void set(short v);
    void set(int v);
    void set(int64_t v);
    void set(float v);
    void set(double v);
    void set(const MMatrix& v);
    void set(const MFloatMatrix& v);
    void set(const MVector& v);
    void set(const MFloatVector& v);
    void set(const MDistance& v);
    void set(const MAngle& v);
    void set(const MTime& v);
    void set(short a, short b);
    void set(int a, int b);
    void set(float a, float b);
    void set(double a, double b);
    void set(short a, short b, short c);
    void set(int a, int b, int c);
    void set(float a, float b, float c);
    void set(double a, double b, double c);
    void set(const MString& v);
    MStatus set(const MObject& data);
    // A plug-in data value: the one held (never change it), or one given to the host, which frees it.
    MPxData* asPluginData() const;
    MStatus set(MPxData* data);
    void setBool(bool v);
    void setChar(char v);
    void setShort(short v);
    void setInt(int v);
    void setInt64(int64_t v);
    void setFloat(float v);
    void setDouble(double v);
    void setMMatrix(const MMatrix& v);
    void setMFloatMatrix(const MFloatMatrix& v);
    void setMVector(const MVector& v);
    void setMFloatVector(const MFloatVector& v);
    void setMDistance(const MDistance& v);
    void setMAngle(const MAngle& v);
    void setMTime(const MTime& v);
    void set2Short(short a, short b);
    void set2Int(int a, int b);
    void set2Float(float a, float b);
    void set2Double(double a, double b);
    void set3Short(short a, short b, short c);
    void set3Int(int a, int b, int c);
    void set3Float(float a, float b, float c);
    void set3Double(double a, double b, double c);
    void setString(const MString& v);
    MStatus copy(const MDataHandle& src);
    MStatus copyWritable(const MDataHandle& src);
    void setClean();

    // The kit's own: where the handle reads and writes.
    az::BlockImpl* azBlock;
    std::shared_ptr<az::AttrSpec> azSpec;
    int azElement;               // the attribute's own element when it is an array, else -1
    std::vector<int> azOuter;    // the elements of the array compounds it is inside, outermost first
    std::vector<double> azLoose; // numbers read outside a compute (MPlug::asMDataHandle), when there is no block

private:
    int index(int* component) const;
    double number(int component = -1) const;
    void setNumber(double v, int component = -1);
    void setVec(double a, double b, double c);
};

class MArrayDataBuilder {
public:
    MArrayDataBuilder();
    MArrayDataBuilder(const MObject& attr, unsigned int growSize, MStatus* status = 0);
    MArrayDataBuilder(MDataBlock* block, const MObject& attr, unsigned int growSize, MStatus* status = 0);
    MArrayDataBuilder(const MArrayDataBuilder& other);
    MArrayDataBuilder& operator=(const MArrayDataBuilder& other);
    MDataHandle addElement(unsigned int index, MStatus* status = 0);
    MDataHandle addLast(MStatus* status = 0);
    MStatus removeElement(unsigned int index);
    unsigned int elementCount(MStatus* status = 0) const;
    MStatus growArray(unsigned int amount);
    MStatus setGrowSize(unsigned int size);

    az::BlockImpl* azBlock;
    std::shared_ptr<az::AttrSpec> azSpec;
    std::vector<int> azAdded;
    std::vector<int> azOuter;
};

class MArrayDataHandle {
public:
    MArrayDataHandle(const MDataHandle& in, MStatus* status = 0);
    MArrayDataHandle(const MArrayDataHandle& other);
    MArrayDataHandle& operator=(const MArrayDataHandle& other);
    MDataHandle inputValue(MStatus* status = 0);
    MDataHandle outputValue(MStatus* status = 0);
    MArrayDataHandle inputArrayValue(MStatus* status = 0);
    MArrayDataHandle outputArrayValue(MStatus* status = 0);
    MStatus next();
    unsigned int elementCount(MStatus* status = 0);
    unsigned int elementIndex(MStatus* status = 0);
    MStatus jumpToElement(unsigned int index);
    MStatus jumpToArrayElement(unsigned int position);
    MStatus jumpToLogicalElement(unsigned int index);
    MStatus jumpToPhysicalElement(unsigned int position);
    MStatus setClean();
    MStatus setAllClean();
    MArrayDataBuilder builder(MStatus* status = 0);
    MStatus set(const MArrayDataBuilder& builder);

    // The kit's own.
    MArrayDataHandle();
    az::BlockImpl* azBlock;
    std::shared_ptr<az::AttrSpec> azSpec;
    std::vector<int> azIds;
    size_t azAt;
    std::vector<int> azOuter;
};

class MDataBlock {
public:
    MDataHandle inputValue(const MPlug& plug, MStatus* status = 0);
    MDataHandle inputValue(const MObject& attr, MStatus* status = 0);
    MDataHandle outputValue(const MPlug& plug, MStatus* status = 0);
    MDataHandle outputValue(const MObject& attr, MStatus* status = 0);
    MArrayDataHandle inputArrayValue(const MPlug& plug, MStatus* status = 0);
    MArrayDataHandle inputArrayValue(const MObject& attr, MStatus* status = 0);
    MArrayDataHandle outputArrayValue(const MPlug& plug, MStatus* status = 0);
    MArrayDataHandle outputArrayValue(const MObject& attr, MStatus* status = 0);
    MStatus setClean(const MPlug& plug);
    MStatus setClean(const MObject& attr);
    bool isClean(const MPlug& plug, MStatus* status = 0);
    bool isClean(const MObject& attr, MStatus* status = 0);
    MDGContext context(MStatus* status = 0);

    // The kit's own: made only by the kit for a compute.
    explicit MDataBlock(az::BlockImpl* b) : b_(b) {}
    az::BlockImpl* azBlock() const { return b_; }

private:
    az::BlockImpl* b_;
};

// ---------------------------------------------------------------------------------------------------------------------
// Nodes

class MPxNode {
public:
    enum Type {
        kDependNode,
        kLocatorNode,
        kDeformerNode,
        kManipContainer,
        kSurfaceShape,
        kFieldNode,
        kEmitterNode,
        kSpringNode,
        kIkSolverNode,
        kHardwareShader,
        kHwShaderNode,
        kTransformNode,
        kObjectSet,
        kFluidEmitterNode,
        kImagePlaneNode,
        kParticleAttributeMapperNode,
        kCameraSetNode,
        kConstraintNode,
        kManipulatorNode,
        kMotionPathNode,
        kClientDeviceNode,
        kThreadedDeviceNode,
        kAssembly,
        kSkinCluster,
        kGeometryFilter,
        kBlendShape,
        kLast
    };
    enum SchedulingType { kParallel, kSerial, kGloballySerial, kUntrusted, kDefaultScheduling = kUntrusted };
    MPxNode();
    virtual ~MPxNode();
    virtual void postConstructor();
    virtual MStatus compute(const MPlug& plug, MDataBlock& data);
    virtual SchedulingType schedulingType() const;
    virtual bool isAbstractClass() const;
    virtual bool isPassiveOutput(const MPlug& plug) const;
    virtual MStatus setDependentsDirty(const MPlug& plug, MPlugArray& affected);
    virtual MStatus connectionMade(const MPlug& plug, const MPlug& other, bool asSrc);
    virtual MStatus connectionBroken(const MPlug& plug, const MPlug& other, bool asSrc);
    virtual Type type() const;
    MObject thisMObject() const;
    MString name(MStatus* status = 0) const;
    MTypeId typeId() const;
    MString typeName() const;
    MStatus setExistWithoutInConnections(bool on);
    bool existWithoutInConnections(MStatus* status = 0) const;
    MStatus setExistWithoutOutConnections(bool on);
    bool existWithoutOutConnections(MStatus* status = 0) const;
    static MStatus addAttribute(const MObject& attr);
    static MStatus attributeAffects(const MObject& whenChanges, const MObject& isAffected);
    static MStatus inheritAttributesFrom(const MString& parentClassName);

    static MObject message;
    static MObject isHistoricallyInteresting;
    static MObject caching;
    static MObject state;
    static MObject frozen;

    // The kit's own.
    int64_t azNodeId_;
    az::NodeClass* azClass_;
};

class MPxGeometryFilter : public MPxNode {
public:
    MPxGeometryFilter();
    virtual ~MPxGeometryFilter();
    virtual Type type() const;
    virtual MStatus compute(const MPlug& plug, MDataBlock& data);
    virtual MStatus deform(MDataBlock& block, MItGeometry& iter, const MMatrix& localToWorld, unsigned int multiIndex);
    virtual MObject& accessoryAttribute() const;
    virtual MStatus accessoryNodeSetup(MDagModifier& modifier);
    MObject getInputGeom(unsigned int index, MStatus* status = 0);

    static MObject input;
    static MObject inputGeom;
    static MObject groupId;
    static MObject outputGeom;
    static MObject originalGeometry;
    static MObject envelope;
};

// A ramp attribute: an array compound of positions, values and interpolations (blendCurve[0].blendCurve_Position).
class MRampAttribute {
public:
    enum MInterpolation { kNone = 0, kLinear = 1, kSmooth = 2, kSpline = 3 };
    MRampAttribute();
    MRampAttribute(const MPlug& plug, MStatus* status = 0);
    MRampAttribute(const MObject& node, const MObject& attr, MStatus* status = 0);
    MRampAttribute(const MRampAttribute& other);
    MRampAttribute& operator=(const MRampAttribute& other);
    ~MRampAttribute();
    unsigned int getNumEntries(MStatus* status = 0);
    void getEntries(MIntArray& indexes, MFloatArray& positions, MFloatArray& values, MIntArray& interps, MStatus* status = 0);
    void getEntries(MIntArray& indexes, MFloatArray& positions, MColorArray& colors, MIntArray& interps, MStatus* status = 0);
    void setRamp(const MFloatArray& values, const MFloatArray& positions, const MIntArray& interps, MStatus* status = 0);
    void setRamp(float value, float position = 1.0f, unsigned int index = 0, MStatus* status = 0);
    void addEntries(MFloatArray& positions, MFloatArray& values, MIntArray& interps, MStatus* status = 0);
    void deleteEntries(MIntArray& indexes, MStatus* status = 0);
    void getValueAtPosition(float position, float& value, MStatus* status = 0);
    void getColorAtPosition(float position, MColor& color, MStatus* status = 0);
    bool isCurveRamp(MStatus* status = 0) const;
    bool isColorRamp(MStatus* status = 0) const;
    static MObject createCurveRamp(const MString& longName, const MString& shortName, MStatus* status = 0);
    static MObject createColorRamp(const MString& longName, const MString& shortName, MStatus* status = 0);
    static MObject createRamp(const MString& longName, const MString& shortName, const MObject& parent, MStatus* status = 0);

private:
    MPlug plug_;
};

class MPxDeformerNode : public MPxGeometryFilter {
public:
    MPxDeformerNode();
    virtual ~MPxDeformerNode();
    virtual Type type() const;
    float weightValue(MDataBlock& block, unsigned int multiIndex, unsigned int pointIndex);
    static MObject weightList;
    static MObject weights;
};

// The points a deformer moves: those of its mesh, with their weights.
class MItGeometry {
public:
    MItGeometry(const MDagPath& path, MStatus* status = 0);
    MItGeometry(const MDagPath& path, MObject& component, MStatus* status = 0);
    MItGeometry(const MObject& dagObject, MStatus* status = 0);
    MItGeometry(MDataHandle& handle, unsigned int groupId, bool readOnly = true, MStatus* status = 0);
    MItGeometry(MDataHandle& handle, bool readOnly = true, MStatus* status = 0);
    ~MItGeometry();
    bool isDone(MStatus* status = 0);
    MStatus next();
    MStatus reset();
    MPoint position(MSpace::Space space = MSpace::kObject, MStatus* status = 0) const;
    MStatus setPosition(const MPoint& p, MSpace::Space space = MSpace::kObject);
    MVector normal(MSpace::Space space = MSpace::kObject, MStatus* status = 0) const;
    MObject currentItem(MStatus* status = 0);
    MStatus component(MObject& out);
    int index(MStatus* status = 0) const;
    int count(MStatus* status = 0) const;
    int exactCount(MStatus* status = 0) const;
    float weight(MStatus* status = 0) const;
    MStatus allPositions(MPointArray& out, MSpace::Space space = MSpace::kObject) const;
    MStatus setAllPositions(const MPointArray& pts, MSpace::Space space = MSpace::kObject);

    // The kit's own.
    MItGeometry(const std::shared_ptr<az::MeshData>& mesh, const MMatrix& world);
    std::shared_ptr<az::MeshData> azMesh() const { return mesh_; }

private:
    void azSkip();
    std::shared_ptr<az::MeshData> mesh_;
    int64_t node_;
    MMatrix world_;
    int at_;
    std::vector<int> order_;
};

// ---------------------------------------------------------------------------------------------------------------------

typedef void* (*MCreatorFunction)();
typedef MSyntax (*MCreateSyntaxFunction)();
typedef MStatus (*MInitializeFunction)();

// A plug-in's own data type: its values flow through typed attributes made with its id. The host keeps each value it is
// given and never changes it; writeASCII and readASCII carry it in a scene file.
class MPxData {
public:
    enum Type { kData, kGeometryData, kLast };
    MPxData();
    virtual ~MPxData();
    virtual MStatus readASCII(const MArgList& argList, unsigned int& endOfTheLastParsedElement);
    virtual MStatus readBinary(std::istream& in, unsigned int length);
    virtual MStatus writeASCII(std::ostream& out);
    virtual MStatus writeBinary(std::ostream& out);
    virtual void copy(const MPxData& src) = 0;
    virtual MTypeId typeId() const = 0;
    virtual MString name() const = 0;
    static const char* className() { return "MPxData"; }
};

class MFnPlugin : public MFnBase {
public:
    MFnPlugin();
    MFnPlugin(MObject& obj, const char* vendor = "Unknown", const char* version = "Unknown", const char* requiredApiVersion = "Any",
              MStatus* status = 0);
    virtual ~MFnPlugin();
    virtual MFn::Type type() const;
    MString vendor(MStatus* status = 0) const;
    MString version(MStatus* status = 0) const;
    MString apiVersion(MStatus* status = 0) const;
    MString name(MStatus* status = 0) const;
    MString loadPath(MStatus* status = 0) const;
    MStatus setName(const MString& name, bool allowRename = true);
    MStatus setVersion(const MString& version);
    MStatus registerCommand(const MString& name, MCreatorFunction creator, MCreateSyntaxFunction syntax = 0);
    MStatus deregisterCommand(const MString& name);
    MStatus registerNode(const MString& name, const MTypeId& id, MCreatorFunction creator, MInitializeFunction init,
                         MPxNode::Type type = MPxNode::kDependNode, const MString* classification = 0);
    MStatus deregisterNode(const MTypeId& id);
    // A shape and the object that would draw it in the reference's legacy viewport, which is kept and never called.
    MStatus registerShape(const MString& name, const MTypeId& id, MCreatorFunction creator, MInitializeFunction init, MCreatorFunction uiCreator,
                          const MString* classification = 0);
    MStatus registerData(const MString& name, const MTypeId& id, MCreatorFunction creator, MPxData::Type type = MPxData::kData);
    MStatus deregisterData(const MTypeId& id);
    MStatus registerConstraintCommand(const MString& name, MCreatorFunction creator);
    MStatus deregisterConstraintCommand(const MString& name);
    MStatus registerUI(const MString& createProc, const MString& deleteProc, const MString& createBatchProc = "",
                       const MString& deleteBatchProc = "");
    MStatus registerUIStrings(void (*proc)(), const MString& stringResourceFile);
    static MObject findPlugin(const MString& name);
    static bool isNodeRegistered(const MString& type);
};

#endif
