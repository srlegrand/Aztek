// Aztek plug-in kit: basic types. Written for Aztek from the public API reference; see devkit/README.md.
#ifndef AZTEK_KIT_BASE_H
#define AZTEK_KIT_BASE_H

#include <cstddef>
#include <cstdint>
#include <iosfwd>
#include <memory>
#include <string>
#include <vector>
// As the reference's headers do on Linux, where its builds define REQUIRE_IOSTREAM: cout, endl and ios at file scope.
#ifdef REQUIRE_IOSTREAM
#include <maya/MIOStream.h>
#endif

#if defined(_WIN32)
#define PLUGIN_EXPORT __declspec(dllexport)
#else
#define PLUGIN_EXPORT __attribute__((visibility("default")))
#endif
#ifndef MLLEXPORT
#define MLLEXPORT PLUGIN_EXPORT
#endif
#define AZTEK_KIT 1
// The API version the kit answers to: what MGlobal::apiVersion() returns.
#define MAYA_API_VERSION 20240000

typedef unsigned int uint;
typedef float float2[2];
typedef float float3[3];
typedef double double2[2];
typedef double double3[3];
typedef int int2[2];
typedef int int3[3];
typedef short short2[2];
typedef short short3[3];
typedef std::uintptr_t MUintPtrSz;
typedef MUintPtrSz MCallbackId;

namespace az {
struct ObjectData;
struct AttrSpec;
struct MeshData;
}

// ---------------------------------------------------------------------------------------------------------------------

namespace MS {
enum MStatusCode {
    kSuccess = 0,
    kFailure,
    kInsufficientMemory,
    kInvalidParameter,
    kLicenseFailure,
    kUnknownParameter,
    kNotImplemented,
    kNotFound,
    kEndOfFile
};
}

class MString;

class MStatus {
public:
    enum MStatusCode {
        kSuccess = MS::kSuccess,
        kFailure = MS::kFailure,
        kInsufficientMemory = MS::kInsufficientMemory,
        kInvalidParameter = MS::kInvalidParameter,
        kLicenseFailure = MS::kLicenseFailure,
        kUnknownParameter = MS::kUnknownParameter,
        kNotImplemented = MS::kNotImplemented,
        kNotFound = MS::kNotFound,
        kEndOfFile = MS::kEndOfFile
    };
    MStatus();
    MStatus(MS::MStatusCode code);
    MStatus(MStatusCode code);
    MStatus(const MStatus& other);
    ~MStatus();
    MStatus& operator=(const MStatus& other);
    bool operator==(const MStatus& other) const;
    bool operator==(MS::MStatusCode code) const;
    bool operator!=(const MStatus& other) const;
    bool operator!=(MS::MStatusCode code) const;
    bool operator==(MStatusCode code) const { return *this == MS::MStatusCode(code); }
    bool operator!=(MStatusCode code) const { return *this != MS::MStatusCode(code); }
    operator bool() const;
    bool error() const;
    void clear();
    MS::MStatusCode statusCode() const;
    MString errorString() const;
    void perror(const char* text = 0) const;
    void perror(const MString& text) const;
    void setErrorString(const MString& text);

private:
    MS::MStatusCode code_;
    std::shared_ptr<std::string> text_;
};

// A code compared with a status, the code first: without these the status became a bool, and kSuccess (0) never
// equalled a successful status (true).
inline bool operator==(MS::MStatusCode code, const MStatus& s) { return s == code; }
inline bool operator!=(MS::MStatusCode code, const MStatus& s) { return s != code; }
// The same with the code spelled MStatus::kSuccess, as the reference's own check macros spell it.
inline bool operator==(MStatus::MStatusCode code, const MStatus& s) { return s == code; }
inline bool operator!=(MStatus::MStatusCode code, const MStatus& s) { return s != code; }

std::ostream& operator<<(std::ostream& os, const MStatus& s);

// Reports a failed status with where it happened.
void azReportStatus(const MStatus& s, const char* expr, const char* file, int line);

#define CHECK_MSTATUS(_status)                                                                                         \
    do {                                                                                                               \
        MStatus _azs = (_status);                                                                                      \
        if (_azs != MS::kSuccess) azReportStatus(_azs, #_status, __FILE__, __LINE__);                                  \
    } while (0)
#define CHECK_MSTATUS_AND_RETURN(_status, _retVal)                                                                     \
    do {                                                                                                               \
        MStatus _azs = (_status);                                                                                      \
        if (_azs != MS::kSuccess) {                                                                                    \
            azReportStatus(_azs, #_status, __FILE__, __LINE__);                                                        \
            return (_retVal);                                                                                          \
        }                                                                                                              \
    } while (0)
#define CHECK_MSTATUS_AND_RETURN_IT(_status) CHECK_MSTATUS_AND_RETURN((_status), (_status))
#define MFAIL(_status) (MS::kSuccess != (_status))
#define MSUCCESS(_status) (MS::kSuccess == (_status))

// ---------------------------------------------------------------------------------------------------------------------

class MStringArray;

class MString {
public:
    MString();
    MString(const char* text);
    MString(const char* text, int length);
    MString(const wchar_t* text);
    MString(const MString& other);
    MString(const std::string& text);
    ~MString();
    MString& operator=(const MString& other);
    MString& operator=(const char* text);
    MString& operator=(double v);
    MString& operator+=(const MString& other);
    MString& operator+=(const char* text);
    MString& operator+=(double v);
    MString& operator+=(int v);
    MString& operator+=(unsigned int v);
    MString& operator+=(float v);
    MString operator+(const MString& other) const;
    MString operator+(const char* other) const;
    MString operator+(double v) const;
    MString operator+(int v) const;
    MString operator+(unsigned int v) const;
    bool operator==(const MString& other) const;
    bool operator==(const char* other) const;
    bool operator!=(const MString& other) const;
    bool operator!=(const char* other) const;
    bool operator<(const MString& other) const;
    bool operator>(const MString& other) const;
    MString& set(const char* text);
    MString& set(const char* text, int length);
    MString& set(double v);
    MString& set(double v, int precision);
    const char* asChar() const;
    const char* asChar(int& length) const;
    const char* asUTF8() const;
    const char* asUTF8(int& length) const;
    const wchar_t* asWChar() const;
    unsigned int length() const;
    unsigned int numChars() const;
    void clear();
    int index(char c) const;
    int rindex(char c) const;
    int indexW(const MString& s) const;
    int indexW(char c) const;
    int rindexW(const MString& s) const;
    int rindexW(char c) const;
    MStatus split(char c, MStringArray& out) const;
    MString substring(int start, int end) const;
    MString substringW(int start, int end) const;
    MString& toUpperCase();
    MString& toLowerCase();
    MString toUpperCase() const;
    MString toLowerCase() const;
    bool isShort() const;
    short asShort() const;
    bool isInt() const;
    int asInt() const;
    bool isUnsigned() const;
    unsigned int asUnsigned() const;
    bool isFloat() const;
    float asFloat() const;
    bool isDouble() const;
    double asDouble() const;
    MString expandEnvironmentVariablesAndTilde(MStatus* status = 0) const;
    MString expandFilePath(MStatus* status = 0) const;
    MString& substitute(const MString& find, const MString& with);
    MString& substituteFirst(const MString& find, const MString& with);
    MString& substituteW(const MString& find, const MString& with);
    MString& substituteFirstW(const MString& find, const MString& with);
    bool isEmpty() const;
    MStatus format(const MString& fmt, const MString& a1 = MString(), const MString& a2 = MString(),
                   const MString& a3 = MString(), const MString& a4 = MString(), const MString& a5 = MString(),
                   const MString& a6 = MString(), const MString& a7 = MString(), const MString& a8 = MString(),
                   const MString& a9 = MString(), const MString& a10 = MString());
    // The kit's own: the text as a std::string.
    const std::string& str() const { return s_; }

private:
    std::string s_;
    mutable std::wstring w_;
};

MString operator+(const char* a, const MString& b);
std::ostream& operator<<(std::ostream& os, const MString& s);

// ---------------------------------------------------------------------------------------------------------------------
// Arrays: each array class of the reference is a class of its own over one template, so plug-ins may forward-declare them.

namespace az {
template <typename T>
class ArrayOf {
public:
    ArrayOf() {}
    ArrayOf(unsigned int n, const T& init) : v_(n, init) {}
    ArrayOf(const T* src, unsigned int n) : v_(src, src + n) {}
    unsigned int length() const { return (unsigned int)v_.size(); }
    MStatus setLength(unsigned int n) { v_.resize(n); return MS::kSuccess; }
    MStatus set(const T& x, unsigned int i) {
        if (i >= v_.size()) return MS::kFailure;
        v_[i] = x;
        return MS::kSuccess;
    }
    MStatus append(const T& x) { v_.push_back(x); return MS::kSuccess; }
    MStatus insert(const T& x, unsigned int i) {
        if (i > v_.size()) return MS::kFailure;
        v_.insert(v_.begin() + i, x);
        return MS::kSuccess;
    }
    MStatus remove(unsigned int i) {
        if (i >= v_.size()) return MS::kFailure;
        v_.erase(v_.begin() + i);
        return MS::kSuccess;
    }
    MStatus clear() { v_.clear(); return MS::kSuccess; }
    MStatus get(T* out) const {
        for (size_t i = 0; i < v_.size(); ++i) out[i] = v_[i];
        return MS::kSuccess;
    }
    MStatus copy(const ArrayOf& other) { v_ = other.v_; return MS::kSuccess; }
    void setSizeIncrement(unsigned int) {}
    unsigned int sizeIncrement() const { return 1; }
    T& operator[](unsigned int i) { return v_[i]; }
    const T& operator[](unsigned int i) const { return v_[i]; }
    // STL-style access, as newer API arrays have.
    typedef typename std::vector<T>::iterator iterator;
    typedef typename std::vector<T>::const_iterator const_iterator;
    iterator begin() { return v_.begin(); }
    iterator end() { return v_.end(); }
    const_iterator begin() const { return v_.begin(); }
    const_iterator end() const { return v_.end(); }
    std::vector<T>& vec() { return v_; }
    const std::vector<T>& vec() const { return v_; }

protected:
    std::vector<T> v_;
};
}

#define AZ_ARRAY(Name, T)                                                                                              \
    class Name : public az::ArrayOf<T> {                                                                               \
    public:                                                                                                            \
        typedef T Elem;                                                                                                \
        Name() {}                                                                                                      \
        Name(unsigned int n, const T& init = Elem()) : az::ArrayOf<T>(n, init) {}                                      \
        Name(const T* src, unsigned int n) : az::ArrayOf<T>(src, n) {}                                                 \
        Name(const Name& o) : az::ArrayOf<T>(o) {}                                                                     \
        Name& operator=(const Name& o) { v_ = o.v_; return *this; }                                                    \
    };

AZ_ARRAY(MIntArray, int)
AZ_ARRAY(MUintArray, unsigned int)
AZ_ARRAY(MInt64Array, int64_t)
AZ_ARRAY(MDoubleArray, double)
AZ_ARRAY(MFloatArray, float)
AZ_ARRAY(MStringArray, MString)
AZ_ARRAY(MCallbackIdArray, MCallbackId)

std::ostream& operator<<(std::ostream& os, const MIntArray& a);
std::ostream& operator<<(std::ostream& os, const MDoubleArray& a);

// ---------------------------------------------------------------------------------------------------------------------

class MTypeId {
public:
    MTypeId();
    MTypeId(unsigned int id);
    MTypeId(unsigned int prefix, unsigned int id);
    MTypeId(const MTypeId& other);
    MTypeId& operator=(const MTypeId& other);
    bool operator==(const MTypeId& other) const;
    bool operator!=(const MTypeId& other) const;
    unsigned int id(MStatus* status = 0) const;

private:
    unsigned int id_;
};

// Function set and object kinds. A subset, with the reference's numbers (2027).
class MFn {
public:
    enum Type {
        kInvalid = 0,
        kBase = 1,
        kNamedObject = 2,
        kDependencyNode = 4,
        kDagNode = 107,
        kTransform = 110,
        kJoint = 121,
        kIkHandle = 120,
        kIkEffector = 119,
        kShape = 248,
        kGeometric = 265,
        kSurface = 293,
        kMesh = 296,
        kCurve = 266,
        kNurbsCurve = 267,
        kNurbsSurface = 294,
        kLocator = 281,
        kCamera = 250,
        kLight = 302,
        kLattice = 279,
        kGeometryFilt = 334,
        kWeightGeometryFilt = 346,
        kSkinClusterFilter = 687,
        kBlendShape = 336,
        kCluster = 251,
        kFFD = 339,
        kNonLinear = 624,
        kWire = 355,
        kTweak = 345,
        kPolyTweak = 402,
        kAnimCurve = 7,
        kAnimCurveTimeToAngular = 8,
        kAnimCurveTimeToDistance = 9,
        kAnimCurveTimeToTime = 10,
        kAnimCurveTimeToUnitless = 11,
        kAnimCurveUnitlessToAngular = 12,
        kAnimCurveUnitlessToDistance = 13,
        kAnimCurveUnitlessToTime = 14,
        kAnimCurveUnitlessToUnitless = 15,
        kConstraint = 933,
        kAimConstraint = 111,
        kOrientConstraint = 239,
        kParentConstraint = 242,
        kPointConstraint = 240,
        kScaleConstraint = 244,
        kSet = 471,
        kShadingEngine = 320,
        kLambert = 371,
        kMultiplyDivide = 448,
        kPolyCreator = 435,
        kPolyPrimitive = 436,
        kPolyCube = 438,
        kPolySphere = 441,
        kPolyCylinder = 439,
        kPolyCone = 437,
        kPolyMesh = 440,
        kPolyTorus = 442,
        kPluginDependNode = 459,
        kPluginDeformerNode = 616,
        kPluginLocatorNode = 460,
        kPluginTransformNode = 914,
        kPluginConstraintNode = 1018,
        kPluginShape = 712,
        kPluginIkSolver = 762,
        kPluginData = 602,
        kDisplayLayer = 734,
        kTime = 520,
        kUnknown = 532,
        kAttribute = 565,
        kNumericAttribute = 566,
        kUnitAttribute = 573,
        kTypedAttribute = 574,
        kEnumAttribute = 572,
        kCompoundAttribute = 575,
        kMatrixAttribute = 578,
        kMessageAttribute = 580,
        kGenericAttribute = 576,
        kAttribute3Double = 752,
        kAttribute3Float = 753,
        kDoubleLinearAttribute = 569,
        kDoubleAngleAttribute = 567,
        kTimeAttribute = 571,
        kData = 583,
        kNumericData = 593,
        kMeshData = 590,
        kMatrixData = 589,
        kStringData = 606,
        kGeometryData = 713,
        kNurbsCurveData = 592,
        kDoubleArrayData = 585,
        kFloatArrayData = 1038,
        kIntArrayData = 586,
        kStringArrayData = 607,
        kPointArrayData = 603,
        kVectorArrayData = 608,
        kRamp = 504,
        kComponent = 535,
        kSingleIndexedComponent = 714,
        kDoubleIndexedComponent = 715,
        kMeshComponent = 550,
        kMeshVertComponent = 554,
        kMeshEdgeComponent = 551,
        kMeshPolygonComponent = 552,
        kMeshMapComponent = 818,
        kMeshVtxFaceComponent = 746,
        kCurveCVComponent = 536,
        kCurveEPComponent = 537,
        kPlugin = 582,
        kWorld = 247,
    };
};

class MObject {
public:
    MObject();
    MObject(const MObject& other);
    ~MObject();
    MObject& operator=(const MObject& other);
    bool isNull() const;
    MFn::Type apiType() const;
    bool hasFn(MFn::Type type) const;
    const char* apiTypeStr() const;
    bool operator==(const MObject& other) const;
    bool operator!=(const MObject& other) const;
    static const MObject kNullObj;

    // The kit's own.
    explicit MObject(const std::shared_ptr<az::ObjectData>& d);
    const std::shared_ptr<az::ObjectData>& azData() const { return d_; }

private:
    std::shared_ptr<az::ObjectData> d_;
};

AZ_ARRAY(MObjectArray, MObject)

class MObjectHandle {
public:
    MObjectHandle();
    MObjectHandle(const MObject& o);
    MObject object() const;
    const MObject& objectRef() const;
    bool isValid() const;
    bool isAlive() const;
    unsigned int hashCode() const;
    bool operator==(const MObjectHandle& o) const;
    bool operator==(const MObject& o) const;
    bool operator!=(const MObjectHandle& o) const;

private:
    MObject o_;
};

// ---------------------------------------------------------------------------------------------------------------------
// Units

class MTime {
public:
    enum Unit {
        kInvalid = 0,
        kHours,
        kMinutes,
        kSeconds,
        kMilliseconds,
        k15FPS,
        k24FPS,
        k25FPS,
        k30FPS,
        k48FPS,
        k50FPS,
        k60FPS,
        k120FPS,
        kGames = k15FPS,
        kFilm = k24FPS,
        kPALFrame = k25FPS,
        kNTSCFrame = k30FPS,
        kShowScan = k48FPS,
        kPALField = k50FPS,
        kNTSCField = k60FPS,
        kUserDef,
        kLast
    };
    MTime();
    MTime(double value, Unit unit = kInvalid);
    MTime(const MTime& other);
    MTime& operator=(const MTime& other);
    Unit unit() const;
    double value() const;
    MStatus setUnit(Unit u);
    MStatus setValue(double v);
    double as(Unit u) const;
    double asUnits(Unit u) const;
    static Unit uiUnit();
    static MStatus setUIUnit(Unit u);
    static double framesPerSecond(Unit u);
    MTime operator+(const MTime& o) const;
    MTime operator-(const MTime& o) const;
    MTime operator*(double d) const;
    MTime operator/(double d) const;
    MTime& operator+=(const MTime& o);
    MTime& operator-=(const MTime& o);
    bool operator==(const MTime& o) const;
    bool operator!=(const MTime& o) const;
    bool operator<(const MTime& o) const;
    bool operator>(const MTime& o) const;
    bool operator<=(const MTime& o) const;
    bool operator>=(const MTime& o) const;

private:
    double seconds_;
    Unit unit_;
};

AZ_ARRAY(MTimeArray, MTime)

class MAngle {
public:
    enum Unit { kInvalid = 0, kRadians, kDegrees, kAngMinutes, kAngSeconds, kLast };
    MAngle();
    MAngle(double value, Unit unit = kRadians);
    MAngle(const MAngle& other);
    MAngle& operator=(const MAngle& other);
    Unit unit() const;
    double value() const;
    MStatus setUnit(Unit u);
    MStatus setValue(double v);
    double as(Unit u) const;
    double asUnits(Unit u) const;
    double asRadians() const;
    double asDegrees() const;
    double asAngMinutes() const;
    double asAngSeconds() const;
    static Unit uiUnit();
    static MStatus setUIUnit(Unit u);
    static Unit internalUnit();

private:
    double radians_;
    Unit unit_;
};

class MDistance {
public:
    enum Unit { kInvalid = 0, kInches, kFeet, kYards, kMiles, kMillimeters, kCentimeters, kKilometers, kMeters, kLast };
    MDistance();
    MDistance(double value, Unit unit = kCentimeters);
    MDistance(const MDistance& other);
    MDistance& operator=(const MDistance& other);
    Unit unit() const;
    double value() const;
    MStatus setUnit(Unit u);
    MStatus setValue(double v);
    double as(Unit u) const;
    double asUnits(Unit u) const;
    double asCentimeters() const;
    double asMeters() const;
    double asMillimeters() const;
    double asInches() const;
    double asFeet() const;
    static Unit uiUnit();
    static MStatus setUIUnit(Unit u);
    static Unit internalUnit();

private:
    double cm_;
    Unit unit_;
};

class MSpace {
public:
    enum Space { kInvalid = 0, kTransform, kPreTransform, kPostTransform, kWorld, kObject = kPreTransform, kLast };
};

#endif
