// Aztek plug-in kit: vectors, points, matrices, rotations. Row vectors, as the API has them: p * M.
#ifndef AZTEK_KIT_MATH_H
#define AZTEK_KIT_MATH_H

#include "base.h"

class MPoint;
class MFloatPoint;
class MFloatVector;
class MMatrix;
class MFloatMatrix;
class MQuaternion;
class MEulerRotation;

class MVector {
public:
    enum Axis { kXaxis, kYaxis, kZaxis, kWaxis };
    MVector() : x(0), y(0), z(0) {}
    MVector(double xx, double yy, double zz = 0.0) : x(xx), y(yy), z(zz) {}
    MVector(const double d[3]) : x(d[0]), y(d[1]), z(d[2]) {}
    MVector(const float d[3]) : x(d[0]), y(d[1]), z(d[2]) {}
    MVector(const MPoint& p);
    MVector(const MFloatVector& v);
    MVector(const MFloatPoint& p);
    double operator[](unsigned int i) const { return i == 0 ? x : i == 1 ? y : z; }
    double& operator[](unsigned int i) { return i == 0 ? x : i == 1 ? y : z; }
    double operator()(unsigned int i) const { return (*this)[i]; }
    MVector operator+(const MVector& o) const { return MVector(x + o.x, y + o.y, z + o.z); }
    MVector operator-(const MVector& o) const { return MVector(x - o.x, y - o.y, z - o.z); }
    MVector operator-() const { return MVector(-x, -y, -z); }
    MVector operator*(double s) const { return MVector(x * s, y * s, z * s); }
    MVector operator/(double s) const { return MVector(x / s, y / s, z / s); }
    double operator*(const MVector& o) const { return x * o.x + y * o.y + z * o.z; }
    MVector operator^(const MVector& o) const { return MVector(y * o.z - z * o.y, z * o.x - x * o.z, x * o.y - y * o.x); }
    MVector operator*(const MMatrix& m) const;
    MVector& operator*=(const MMatrix& m);
    MVector& operator+=(const MVector& o) { x += o.x; y += o.y; z += o.z; return *this; }
    MVector& operator-=(const MVector& o) { x -= o.x; y -= o.y; z -= o.z; return *this; }
    MVector& operator*=(double s) { x *= s; y *= s; z *= s; return *this; }
    MVector& operator/=(double s) { x /= s; y /= s; z /= s; return *this; }
    bool operator==(const MVector& o) const { return x == o.x && y == o.y && z == o.z; }
    bool operator!=(const MVector& o) const { return !(*this == o); }
    MVector rotateBy(const MQuaternion& q) const;
    MVector rotateBy(const MEulerRotation& e) const;
    MQuaternion rotateTo(const MVector& to) const;
    MStatus get(double out[3]) const { out[0] = x; out[1] = y; out[2] = z; return MS::kSuccess; }
    double length() const;
    double lengthSquared() const { return x * x + y * y + z * z; }
    MVector normal() const;
    MStatus normalize();
    double angle(const MVector& o) const;
    bool isEquivalent(const MVector& o, double tol = 1e-10) const;
    bool isParallel(const MVector& o, double tol = 1e-10) const;
    MVector transformAsNormal(const MMatrix& m) const;
    double x, y, z;
    static const MVector zero, one, xAxis, yAxis, zAxis, xNegAxis, yNegAxis, zNegAxis;
};
MVector operator*(double s, const MVector& v);
MVector operator*(const MMatrix& m, const MVector& v);
std::ostream& operator<<(std::ostream& os, const MVector& v);

class MFloatVector {
public:
    MFloatVector() : x(0), y(0), z(0) {}
    MFloatVector(float xx, float yy, float zz = 0.0f) : x(xx), y(yy), z(zz) {}
    MFloatVector(const float d[3]) : x(d[0]), y(d[1]), z(d[2]) {}
    MFloatVector(const MVector& v) : x((float)v.x), y((float)v.y), z((float)v.z) {}
    MFloatVector(const MFloatPoint& p);
    float operator[](unsigned int i) const { return i == 0 ? x : i == 1 ? y : z; }
    float& operator[](unsigned int i) { return i == 0 ? x : i == 1 ? y : z; }
    MFloatVector operator+(const MFloatVector& o) const { return MFloatVector(x + o.x, y + o.y, z + o.z); }
    MFloatVector operator-(const MFloatVector& o) const { return MFloatVector(x - o.x, y - o.y, z - o.z); }
    MFloatVector operator-() const { return MFloatVector(-x, -y, -z); }
    MFloatVector operator*(float s) const { return MFloatVector(x * s, y * s, z * s); }
    MFloatVector operator/(float s) const { return MFloatVector(x / s, y / s, z / s); }
    float operator*(const MFloatVector& o) const { return x * o.x + y * o.y + z * o.z; }
    MFloatVector operator^(const MFloatVector& o) const { return MFloatVector(y * o.z - z * o.y, z * o.x - x * o.z, x * o.y - y * o.x); }
    MFloatVector operator*(const MFloatMatrix& m) const;
    MFloatVector& operator+=(const MFloatVector& o) { x += o.x; y += o.y; z += o.z; return *this; }
    MFloatVector& operator-=(const MFloatVector& o) { x -= o.x; y -= o.y; z -= o.z; return *this; }
    MFloatVector& operator*=(float s) { x *= s; y *= s; z *= s; return *this; }
    MFloatVector& operator/=(float s) { x /= s; y /= s; z /= s; return *this; }
    bool operator==(const MFloatVector& o) const { return x == o.x && y == o.y && z == o.z; }
    bool operator!=(const MFloatVector& o) const { return !(*this == o); }
    MStatus get(float out[3]) const { out[0] = x; out[1] = y; out[2] = z; return MS::kSuccess; }
    float length() const;
    MFloatVector normal() const;
    MStatus normalize();
    bool isEquivalent(const MFloatVector& o, float tol = 1e-5f) const;
    float x, y, z;
};
MFloatVector operator*(float s, const MFloatVector& v);
std::ostream& operator<<(std::ostream& os, const MFloatVector& v);

class MPoint {
public:
    MPoint() : x(0), y(0), z(0), w(1) {}
    MPoint(double xx, double yy, double zz = 0.0, double ww = 1.0) : x(xx), y(yy), z(zz), w(ww) {}
    MPoint(const double d[4]) : x(d[0]), y(d[1]), z(d[2]), w(d[3]) {}
    MPoint(const float d[4]) : x(d[0]), y(d[1]), z(d[2]), w(d[3]) {}
    MPoint(const MVector& v) : x(v.x), y(v.y), z(v.z), w(1) {}
    MPoint(const MFloatPoint& p);
    MPoint(const MFloatVector& v) : x(v.x), y(v.y), z(v.z), w(1) {}
    double operator[](unsigned int i) const { return i == 0 ? x : i == 1 ? y : i == 2 ? z : w; }
    double& operator[](unsigned int i) { return i == 0 ? x : i == 1 ? y : i == 2 ? z : w; }
    double operator()(unsigned int i) const { return (*this)[i]; }
    MVector operator-(const MPoint& o) const { return MVector(x - o.x, y - o.y, z - o.z); }
    MPoint operator+(const MVector& v) const { return MPoint(x + v.x, y + v.y, z + v.z, w); }
    MPoint operator-(const MVector& v) const { return MPoint(x - v.x, y - v.y, z - v.z, w); }
    MPoint operator+(const MPoint& o) const { return MPoint(x + o.x, y + o.y, z + o.z, w); }
    MPoint operator*(double s) const { return MPoint(x * s, y * s, z * s, w); }
    MPoint operator/(double s) const { return MPoint(x / s, y / s, z / s, w); }
    MPoint operator*(const MMatrix& m) const;
    MPoint& operator*=(const MMatrix& m);
    MPoint& operator+=(const MVector& v) { x += v.x; y += v.y; z += v.z; return *this; }
    MPoint& operator-=(const MVector& v) { x -= v.x; y -= v.y; z -= v.z; return *this; }
    MPoint& operator*=(double s) { x *= s; y *= s; z *= s; return *this; }
    bool operator==(const MPoint& o) const { return x == o.x && y == o.y && z == o.z && w == o.w; }
    bool operator!=(const MPoint& o) const { return !(*this == o); }
    MStatus get(double out[4]) const { out[0] = x; out[1] = y; out[2] = z; out[3] = w; return MS::kSuccess; }
    MStatus get(float out[4]) const;
    MPoint& cartesianize();
    MPoint& rationalize();
    MPoint& homogenize();
    double distanceTo(const MPoint& o) const;
    bool isEquivalent(const MPoint& o, double tol = 1e-10) const;
    double x, y, z, w;
    static const MPoint origin;
};
MPoint operator*(const MMatrix& m, const MPoint& p);
std::ostream& operator<<(std::ostream& os, const MPoint& p);

class MFloatPoint {
public:
    MFloatPoint() : x(0), y(0), z(0), w(1) {}
    MFloatPoint(float xx, float yy, float zz = 0.0f, float ww = 1.0f) : x(xx), y(yy), z(zz), w(ww) {}
    MFloatPoint(const MPoint& p) : x((float)p.x), y((float)p.y), z((float)p.z), w((float)p.w) {}
    MFloatPoint(const MFloatVector& v) : x(v.x), y(v.y), z(v.z), w(1) {}
    float operator[](unsigned int i) const { return i == 0 ? x : i == 1 ? y : i == 2 ? z : w; }
    float& operator[](unsigned int i) { return i == 0 ? x : i == 1 ? y : i == 2 ? z : w; }
    MFloatVector operator-(const MFloatPoint& o) const { return MFloatVector(x - o.x, y - o.y, z - o.z); }
    MFloatPoint operator+(const MFloatVector& v) const { return MFloatPoint(x + v.x, y + v.y, z + v.z, w); }
    MFloatPoint operator-(const MFloatVector& v) const { return MFloatPoint(x - v.x, y - v.y, z - v.z, w); }
    MFloatPoint operator*(const MFloatMatrix& m) const;
    bool operator==(const MFloatPoint& o) const { return x == o.x && y == o.y && z == o.z && w == o.w; }
    bool operator!=(const MFloatPoint& o) const { return !(*this == o); }
    MStatus setCast(const MPoint& p) { *this = MFloatPoint(p); return MS::kSuccess; }
    float distanceTo(const MFloatPoint& o) const;
    float x, y, z, w;
};
std::ostream& operator<<(std::ostream& os, const MFloatPoint& p);

class MColor {
public:
    enum MColorType { kRGB, kHSV, kCMY, kCMYK };
    MColor() : r(0), g(0), b(0), a(1) {}
    MColor(float rr, float gg, float bb = 0.0f, float aa = 1.0f) : r(rr), g(gg), b(bb), a(aa) {}
    MColor(MColorType t, float c1, float c2, float c3, float alpha = 1.0f);
    MColor(const float d[3]) : r(d[0]), g(d[1]), b(d[2]), a(1) {}
    float operator[](unsigned int i) const { return i == 0 ? r : i == 1 ? g : i == 2 ? b : a; }
    float& operator[](unsigned int i) { return i == 0 ? r : i == 1 ? g : i == 2 ? b : a; }
    MColor operator+(const MColor& o) const { return MColor(r + o.r, g + o.g, b + o.b, a + o.a); }
    MColor operator-(const MColor& o) const { return MColor(r - o.r, g - o.g, b - o.b, a - o.a); }
    MColor operator*(const MColor& o) const { return MColor(r * o.r, g * o.g, b * o.b, a * o.a); }
    MColor operator*(float s) const { return MColor(r * s, g * s, b * s, a * s); }
    MColor operator/(float s) const { return MColor(r / s, g / s, b / s, a / s); }
    bool operator==(const MColor& o) const { return r == o.r && g == o.g && b == o.b && a == o.a; }
    bool operator!=(const MColor& o) const { return !(*this == o); }
    bool get(MColorType t, float& c1, float& c2, float& c3) const;
    float r, g, b, a;
};
std::ostream& operator<<(std::ostream& os, const MColor& c);

class MMatrix {
public:
    MMatrix();
    MMatrix(const double m[4][4]);
    MMatrix(const float m[4][4]);
    MMatrix(const MFloatMatrix& m);
    double operator()(unsigned int r, unsigned int c) const { return matrix[r][c]; }
    double& operator()(unsigned int r, unsigned int c) { return matrix[r][c]; }
    double* operator[](unsigned int r) { return matrix[r]; }
    const double* operator[](unsigned int r) const { return matrix[r]; }
    MStatus get(double out[4][4]) const;
    MStatus get(float out[4][4]) const;
    MMatrix transpose() const;
    MMatrix& setToIdentity();
    MMatrix& setToProduct(const MMatrix& a, const MMatrix& b);
    MMatrix inverse() const;
    MMatrix adjoint() const;
    MMatrix homogenize() const;
    double det4x4() const;
    double det3x3() const;
    bool isEquivalent(const MMatrix& o, double tol = 1e-10) const;
    bool isSingular() const;
    MMatrix operator*(const MMatrix& o) const;
    MMatrix& operator*=(const MMatrix& o);
    MMatrix operator*(double s) const;
    MMatrix operator+(const MMatrix& o) const;
    MMatrix operator-(const MMatrix& o) const;
    bool operator==(const MMatrix& o) const;
    bool operator!=(const MMatrix& o) const;
    double matrix[4][4];
    static const MMatrix identity;
};
std::ostream& operator<<(std::ostream& os, const MMatrix& m);

class MFloatMatrix {
public:
    MFloatMatrix();
    MFloatMatrix(const float m[4][4]);
    MFloatMatrix(const double m[4][4]);
    MFloatMatrix(const MMatrix& m);
    float operator()(unsigned int r, unsigned int c) const { return matrix[r][c]; }
    float& operator()(unsigned int r, unsigned int c) { return matrix[r][c]; }
    float* operator[](unsigned int r) { return matrix[r]; }
    const float* operator[](unsigned int r) const { return matrix[r]; }
    MStatus get(float out[4][4]) const;
    MFloatMatrix transpose() const;
    MFloatMatrix& setToIdentity();
    MFloatMatrix inverse() const;
    MFloatMatrix operator*(const MFloatMatrix& o) const;
    bool operator==(const MFloatMatrix& o) const;
    bool operator!=(const MFloatMatrix& o) const;
    float matrix[4][4];
};

class MEulerRotation {
public:
    enum RotationOrder { kXYZ, kYZX, kZXY, kXZY, kYXZ, kZYX };
    MEulerRotation() : x(0), y(0), z(0), order(kXYZ) {}
    MEulerRotation(double xx, double yy, double zz, RotationOrder o = kXYZ) : x(xx), y(yy), z(zz), order(o) {}
    MEulerRotation(const MVector& v, RotationOrder o = kXYZ) : x(v.x), y(v.y), z(v.z), order(o) {}
    double operator[](unsigned int i) const { return i == 0 ? x : i == 1 ? y : z; }
    double& operator[](unsigned int i) { return i == 0 ? x : i == 1 ? y : z; }
    MEulerRotation operator+(const MEulerRotation& o) const { return MEulerRotation(x + o.x, y + o.y, z + o.z, order); }
    MEulerRotation operator-(const MEulerRotation& o) const { return MEulerRotation(x - o.x, y - o.y, z - o.z, order); }
    MEulerRotation operator-() const { return MEulerRotation(-x, -y, -z, order); }
    MEulerRotation operator*(double s) const { return MEulerRotation(x * s, y * s, z * s, order); }
    bool operator==(const MEulerRotation& o) const { return x == o.x && y == o.y && z == o.z && order == o.order; }
    bool operator!=(const MEulerRotation& o) const { return !(*this == o); }
    MEulerRotation& setValue(double xx, double yy, double zz, RotationOrder o = kXYZ) { x = xx; y = yy; z = zz; order = o; return *this; }
    MMatrix asMatrix() const;
    MQuaternion asQuaternion() const;
    MVector asVector() const { return MVector(x, y, z); }
    MEulerRotation reorder(RotationOrder o) const;
    MEulerRotation& reorderIt(RotationOrder o);
    MEulerRotation bound() const;
    MEulerRotation closestSolution(const MEulerRotation& target) const;
    MEulerRotation& incrementalRotateBy(const MVector& axis, double angle);
    bool isEquivalent(const MEulerRotation& o, double tol = 1e-10) const;
    static MEulerRotation decompose(const MMatrix& m, RotationOrder o);
    double x, y, z;
    RotationOrder order;
    static const MEulerRotation identity;
};
std::ostream& operator<<(std::ostream& os, const MEulerRotation& e);

class MQuaternion {
public:
    MQuaternion() : x(0), y(0), z(0), w(1) {}
    MQuaternion(double xx, double yy, double zz, double ww) : x(xx), y(yy), z(zz), w(ww) {}
    MQuaternion(const MVector& a, const MVector& b);
    MQuaternion(const MVector& a, const MVector& b, double factor);
    MQuaternion(double angle, const MVector& axis);
    double operator[](unsigned int i) const { return i == 0 ? x : i == 1 ? y : i == 2 ? z : w; }
    double& operator[](unsigned int i) { return i == 0 ? x : i == 1 ? y : i == 2 ? z : w; }
    MQuaternion operator*(const MQuaternion& o) const;
    MQuaternion& operator*=(const MQuaternion& o);
    MQuaternion operator+(const MQuaternion& o) const { return MQuaternion(x + o.x, y + o.y, z + o.z, w + o.w); }
    MQuaternion operator-(const MQuaternion& o) const { return MQuaternion(x - o.x, y - o.y, z - o.z, w - o.w); }
    MQuaternion operator-() const { return MQuaternion(-x, -y, -z, -w); }
    bool operator==(const MQuaternion& o) const { return x == o.x && y == o.y && z == o.z && w == o.w; }
    bool operator!=(const MQuaternion& o) const { return !(*this == o); }
    MMatrix asMatrix() const;
    MEulerRotation asEulerRotation() const;
    MQuaternion& setAxisAngle(const MVector& axis, double angle);
    bool getAxisAngle(MVector& axis, double& angle) const;
    MQuaternion normal() const;
    MQuaternion& normalizeIt();
    MQuaternion conjugate() const;
    MQuaternion& conjugateIt();
    MQuaternion inverse() const;
    MQuaternion& invertIt();
    MQuaternion log() const;
    MQuaternion exp() const;
    MQuaternion& setToXAxis(double a);
    MQuaternion& setToYAxis(double a);
    MQuaternion& setToZAxis(double a);
    bool isEquivalent(const MQuaternion& o, double tol = 1e-10) const;
    MQuaternion& operator=(const MMatrix& m);
    MQuaternion& operator=(const MEulerRotation& e);
    double x, y, z, w;
    static const MQuaternion identity;
};
MQuaternion slerp(const MQuaternion& a, const MQuaternion& b, double t, short spin = 0);
std::ostream& operator<<(std::ostream& os, const MQuaternion& q);

class MTransformationMatrix {
public:
    enum RotationOrder { kInvalid, kXYZ, kYZX, kZXY, kXZY, kYXZ, kZYX, kLast };
    MTransformationMatrix();
    MTransformationMatrix(const MMatrix& m);
    MMatrix asMatrix() const;
    MMatrix asMatrix(double percent) const;
    MMatrix asMatrixInverse() const;
    MMatrix asScaleMatrix() const;
    MMatrix asRotateMatrix() const;
    MVector getTranslation(MSpace::Space space, MStatus* status = 0) const;
    MVector translation(MSpace::Space space, MStatus* status = 0) const;
    MStatus setTranslation(const MVector& v, MSpace::Space space);
    MStatus addTranslation(const MVector& v, MSpace::Space space);
    MStatus getScale(double out[3], MSpace::Space space) const;
    MStatus setScale(const double s[3], MSpace::Space space);
    MStatus addScale(const double s[3], MSpace::Space space);
    MStatus getShear(double out[3], MSpace::Space space) const;
    MStatus setShear(const double s[3], MSpace::Space space);
    MQuaternion rotation() const;
    MStatus getRotation(double out[3], RotationOrder& order) const;
    MStatus setRotation(const double r[3], RotationOrder order);
    MTransformationMatrix& rotateTo(const MQuaternion& q);
    MTransformationMatrix& rotateTo(const MEulerRotation& e);
    MTransformationMatrix& rotateBy(const MQuaternion& q, MSpace::Space space, MStatus* status = 0);
    MTransformationMatrix& rotateBy(const MEulerRotation& e, MSpace::Space space, MStatus* status = 0);
    MEulerRotation eulerRotation() const;
    MQuaternion rotationOrientation() const;
    MStatus setRotationOrientation(const MQuaternion& q);
    RotationOrder rotationOrder(MStatus* status = 0) const;
    MStatus reorderRotation(RotationOrder order);
    MPoint scalePivot(MSpace::Space space, MStatus* status = 0) const;
    MStatus setScalePivot(const MPoint& p, MSpace::Space space, bool balance);
    MPoint rotatePivot(MSpace::Space space, MStatus* status = 0) const;
    MStatus setRotatePivot(const MPoint& p, MSpace::Space space, bool balance);
    bool isEquivalent(const MTransformationMatrix& o, double tol = 1e-10) const;
    bool operator==(const MTransformationMatrix& o) const;
    bool operator!=(const MTransformationMatrix& o) const;
    static const MTransformationMatrix identity;

private:
    MVector t_;
    double s_[3];
    double sh_[3];
    MEulerRotation r_;
    MQuaternion ro_;
    MPoint rp_, sp_;
};

class MBoundingBox {
public:
    MBoundingBox();
    MBoundingBox(const MPoint& a, const MPoint& b);
    void clear();
    void transformUsing(const MMatrix& m);
    void expand(const MPoint& p);
    void expand(const MBoundingBox& b);
    bool contains(const MPoint& p) const;
    bool intersects(const MBoundingBox& b, double tol = 0.0) const;
    double width() const;
    double height() const;
    double depth() const;
    MPoint center() const;
    MPoint min() const;
    MPoint max() const;

private:
    MPoint lo_, hi_;
    bool empty_;
};

AZ_ARRAY(MPointArray, MPoint)
AZ_ARRAY(MFloatPointArray, MFloatPoint)
AZ_ARRAY(MVectorArray, MVector)
AZ_ARRAY(MFloatVectorArray, MFloatVector)
AZ_ARRAY(MColorArray, MColor)
AZ_ARRAY(MMatrixArray, MMatrix)

#endif
