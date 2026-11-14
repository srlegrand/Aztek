// Aztek plug-in kit: vectors, points, matrices, rotations. Row vectors: p * M applies M to p, and A * B applies A first.
#include <aztek/kit/math.h>

#include <algorithm>
#include <cmath>
#include <cstring>
#include <ostream>

#ifndef M_PI
#define M_PI 3.14159265358979323846
#endif

static const double kEps = 1e-12;

const MVector MVector::zero(0, 0, 0);
const MVector MVector::one(1, 1, 1);
const MVector MVector::xAxis(1, 0, 0);
const MVector MVector::yAxis(0, 1, 0);
const MVector MVector::zAxis(0, 0, 1);
const MVector MVector::xNegAxis(-1, 0, 0);
const MVector MVector::yNegAxis(0, -1, 0);
const MVector MVector::zNegAxis(0, 0, -1);
const MPoint MPoint::origin(0, 0, 0);
const MMatrix MMatrix::identity;
const MEulerRotation MEulerRotation::identity;
const MQuaternion MQuaternion::identity;
const MTransformationMatrix MTransformationMatrix::identity;

// ---------------------------------------------------------------------------------------------------------------------
// Vectors and points

MVector::MVector(const MPoint& p) : x(p.x), y(p.y), z(p.z) {}
MVector::MVector(const MFloatVector& v) : x(v.x), y(v.y), z(v.z) {}
MVector::MVector(const MFloatPoint& p) : x(p.x), y(p.y), z(p.z) {}

MVector MVector::operator*(const MMatrix& m) const {
    return MVector(x * m[0][0] + y * m[1][0] + z * m[2][0], x * m[0][1] + y * m[1][1] + z * m[2][1], x * m[0][2] + y * m[1][2] + z * m[2][2]);
}
MVector& MVector::operator*=(const MMatrix& m) { return *this = *this * m; }
MVector operator*(int s, const MVector& v) { return v * (double)s; }
MVector operator*(short s, const MVector& v) { return v * (double)s; }
MVector operator*(unsigned int s, const MVector& v) { return v * (double)s; }
MVector operator*(unsigned short s, const MVector& v) { return v * (double)s; }
MVector operator*(float s, const MVector& v) { return v * (double)s; }
MVector operator*(double s, const MVector& v) { return v * s; }
MVector operator*(const MMatrix& m, const MVector& v) {
    return MVector(m[0][0] * v.x + m[0][1] * v.y + m[0][2] * v.z, m[1][0] * v.x + m[1][1] * v.y + m[1][2] * v.z, m[2][0] * v.x + m[2][1] * v.y + m[2][2] * v.z);
}
double MVector::length() const { return std::sqrt(x * x + y * y + z * z); }
MVector MVector::normal() const {
    double l = length();
    return l > kEps ? *this / l : *this;
}
MStatus MVector::normalize() {
    *this = normal();
    return MS::kSuccess;
}
double MVector::angle(const MVector& o) const {
    double l = length() * o.length();
    if (l < kEps) return 0.0;
    double c = (*this * o) / l;
    return std::acos(std::max(-1.0, std::min(1.0, c)));
}
bool MVector::isEquivalent(const MVector& o, double tol) const { return (*this - o).length() <= tol; }
bool MVector::isParallel(const MVector& o, double tol) const {
    MVector a = normal(), b = o.normal();
    return std::fabs(std::fabs(a * b) - 1.0) <= tol;
}
MVector MVector::transformAsNormal(const MMatrix& m) const { return (*this * m.inverse().transpose()).normal(); }
MVector MVector::rotateBy(const MQuaternion& q) const { return *this * q.asMatrix(); }
MVector MVector::rotateBy(const MEulerRotation& e) const { return *this * e.asMatrix(); }
MQuaternion MVector::rotateTo(const MVector& to) const { return MQuaternion(*this, to); }
std::ostream& operator<<(std::ostream& os, const MVector& v) { return os << "[" << v.x << ", " << v.y << ", " << v.z << "]"; }

MFloatVector::MFloatVector(const MFloatPoint& p) : x(p.x), y(p.y), z(p.z) {}
MFloatVector::MFloatVector(const MPoint& p) : x((float)p.x), y((float)p.y), z((float)p.z) {}
MFloatVector MFloatVector::operator*(const MFloatMatrix& m) const {
    return MFloatVector(x * m[0][0] + y * m[1][0] + z * m[2][0], x * m[0][1] + y * m[1][1] + z * m[2][1], x * m[0][2] + y * m[1][2] + z * m[2][2]);
}
float MFloatVector::length() const { return std::sqrt(x * x + y * y + z * z); }
MFloatVector MFloatVector::normal() const {
    float l = length();
    return l > 1e-12f ? *this / l : *this;
}
MStatus MFloatVector::normalize() {
    *this = normal();
    return MS::kSuccess;
}
bool MFloatVector::isEquivalent(const MFloatVector& o, float tol) const { return (*this - o).length() <= tol; }
MFloatVector operator*(int s, const MFloatVector& v) { return v * (float)s; }
MFloatVector operator*(short s, const MFloatVector& v) { return v * (float)s; }
MFloatVector operator*(unsigned int s, const MFloatVector& v) { return v * (float)s; }
MFloatVector operator*(unsigned short s, const MFloatVector& v) { return v * (float)s; }
MFloatVector operator*(float s, const MFloatVector& v) { return v * s; }
MFloatVector operator*(double s, const MFloatVector& v) { return v * (float)s; }
std::ostream& operator<<(std::ostream& os, const MFloatVector& v) { return os << "[" << v.x << ", " << v.y << ", " << v.z << "]"; }

MPoint::MPoint(const MFloatPoint& p) : x(p.x), y(p.y), z(p.z), w(p.w) {}
MPoint MPoint::operator*(const MMatrix& m) const {
    MPoint r;
    for (int c = 0; c < 4; ++c) r[c] = x * m[0][c] + y * m[1][c] + z * m[2][c] + w * m[3][c];
    return r;
}
MPoint& MPoint::operator*=(const MMatrix& m) { return *this = *this * m; }
MPoint operator*(const MMatrix& m, const MPoint& p) {
    MPoint r;
    for (int c = 0; c < 4; ++c) r[c] = m[c][0] * p.x + m[c][1] * p.y + m[c][2] * p.z + m[c][3] * p.w;
    return r;
}
MStatus MPoint::get(float out[4]) const {
    out[0] = (float)x; out[1] = (float)y; out[2] = (float)z; out[3] = (float)w;
    return MS::kSuccess;
}
MPoint& MPoint::cartesianize() {
    if (std::fabs(w) > kEps && w != 1.0) { x /= w; y /= w; z /= w; w = 1.0; }
    return *this;
}
MPoint& MPoint::rationalize() {
    if (std::fabs(w) > kEps) { x /= w; y /= w; z /= w; }
    return *this;
}
MPoint& MPoint::homogenize() {
    x *= w; y *= w; z *= w;
    return *this;
}
double MPoint::distanceTo(const MPoint& o) const { return (*this - o).length(); }
bool MPoint::isEquivalent(const MPoint& o, double tol) const { return distanceTo(o) <= tol && std::fabs(w - o.w) <= tol; }
std::ostream& operator<<(std::ostream& os, const MPoint& p) { return os << "[" << p.x << ", " << p.y << ", " << p.z << ", " << p.w << "]"; }

MFloatPoint MFloatPoint::operator*(const MFloatMatrix& m) const {
    MFloatPoint r;
    for (int c = 0; c < 4; ++c) r[c] = x * m[0][c] + y * m[1][c] + z * m[2][c] + w * m[3][c];
    return r;
}
float MFloatPoint::distanceTo(const MFloatPoint& o) const { return (*this - o).length(); }
std::ostream& operator<<(std::ostream& os, const MFloatPoint& p) { return os << "[" << p.x << ", " << p.y << ", " << p.z << ", " << p.w << "]"; }

// ---------------------------------------------------------------------------------------------------------------------
// Colours

MColor::MColor(MColorType t, float c1, float c2, float c3, float alpha) : r(c1), g(c2), b(c3), a(alpha) {
    if (t == kHSV) {
        float h = std::fmod(c1, 360.0f) / 60.0f, s = c2, v = c3;
        if (h < 0) h += 6.0f;
        int i = (int)h;
        float f = h - i, p = v * (1 - s), q = v * (1 - s * f), u = v * (1 - s * (1 - f));
        switch (i) {
        case 0: r = v; g = u; b = p; break;
        case 1: r = q; g = v; b = p; break;
        case 2: r = p; g = v; b = u; break;
        case 3: r = p; g = q; b = v; break;
        case 4: r = u; g = p; b = v; break;
        default: r = v; g = p; b = q; break;
        }
    } else if (t == kCMY || t == kCMYK) {
        r = 1 - c1; g = 1 - c2; b = 1 - c3;
    }
}

bool MColor::get(MColorType t, float& c1, float& c2, float& c3) const {
    if (t == kRGB) { c1 = r; c2 = g; c3 = b; return true; }
    if (t == kCMY || t == kCMYK) { c1 = 1 - r; c2 = 1 - g; c3 = 1 - b; return true; }
    float mx = std::max(r, std::max(g, b)), mn = std::min(r, std::min(g, b)), d = mx - mn;
    float h = 0;
    if (d > 1e-9f) {
        if (mx == r) h = 60.0f * std::fmod((g - b) / d, 6.0f);
        else if (mx == g) h = 60.0f * ((b - r) / d + 2.0f);
        else h = 60.0f * ((r - g) / d + 4.0f);
        if (h < 0) h += 360.0f;
    }
    c1 = h;
    c2 = mx > 0 ? d / mx : 0.0f;
    c3 = mx;
    return true;
}
std::ostream& operator<<(std::ostream& os, const MColor& c) { return os << "[" << c.r << ", " << c.g << ", " << c.b << ", " << c.a << "]"; }

// ---------------------------------------------------------------------------------------------------------------------
// Matrices

MMatrix::MMatrix() { setToIdentity(); }
MMatrix::MMatrix(const double m[4][4]) { memcpy(matrix, m, sizeof matrix); }
MMatrix::MMatrix(const float m[4][4]) {
    for (int r = 0; r < 4; ++r) for (int c = 0; c < 4; ++c) matrix[r][c] = m[r][c];
}
MMatrix::MMatrix(const MFloatMatrix& m) {
    for (int r = 0; r < 4; ++r) for (int c = 0; c < 4; ++c) matrix[r][c] = m[r][c];
}
MStatus MMatrix::get(double out[4][4]) const { memcpy(out, matrix, sizeof matrix); return MS::kSuccess; }
MStatus MMatrix::get(float out[4][4]) const {
    for (int r = 0; r < 4; ++r) for (int c = 0; c < 4; ++c) out[r][c] = (float)matrix[r][c];
    return MS::kSuccess;
}
MMatrix MMatrix::transpose() const {
    MMatrix t;
    for (int r = 0; r < 4; ++r) for (int c = 0; c < 4; ++c) t.matrix[r][c] = matrix[c][r];
    return t;
}
MMatrix& MMatrix::setToIdentity() {
    for (int r = 0; r < 4; ++r) for (int c = 0; c < 4; ++c) matrix[r][c] = r == c ? 1.0 : 0.0;
    return *this;
}
MMatrix& MMatrix::setToProduct(const MMatrix& a, const MMatrix& b) { return *this = a * b; }

static double minor3(const double m[4][4], int r0, int r1, int r2, int c0, int c1, int c2) {
    return m[r0][c0] * (m[r1][c1] * m[r2][c2] - m[r1][c2] * m[r2][c1]) - m[r0][c1] * (m[r1][c0] * m[r2][c2] - m[r1][c2] * m[r2][c0]) +
           m[r0][c2] * (m[r1][c0] * m[r2][c1] - m[r1][c1] * m[r2][c0]);
}

MMatrix MMatrix::adjoint() const {
    MMatrix a;
    for (int r = 0; r < 4; ++r) {
        for (int c = 0; c < 4; ++c) {
            int rs[3], cs[3], k = 0, l = 0;
            for (int i = 0; i < 4; ++i) if (i != r) rs[k++] = i;
            for (int i = 0; i < 4; ++i) if (i != c) cs[l++] = i;
            double v = minor3(matrix, rs[0], rs[1], rs[2], cs[0], cs[1], cs[2]);
            a.matrix[c][r] = ((r + c) % 2 ? -v : v);
        }
    }
    return a;
}

double MMatrix::det4x4() const {
    double d = 0;
    for (int c = 0; c < 4; ++c) {
        int cs[3], l = 0;
        for (int i = 0; i < 4; ++i) if (i != c) cs[l++] = i;
        double v = matrix[0][c] * minor3(matrix, 1, 2, 3, cs[0], cs[1], cs[2]);
        d += c % 2 ? -v : v;
    }
    return d;
}
double MMatrix::det3x3() const { return minor3(matrix, 0, 1, 2, 0, 1, 2); }

MMatrix MMatrix::inverse() const {
    double d = det4x4();
    if (std::fabs(d) < 1e-300) return *this;
    MMatrix a = adjoint();
    for (int r = 0; r < 4; ++r) for (int c = 0; c < 4; ++c) a.matrix[r][c] /= d;
    return a;
}
MMatrix MMatrix::homogenize() const {
    MMatrix m(*this);
    for (int r = 0; r < 4; ++r) {
        double w = m.matrix[r][3];
        if (std::fabs(w) > kEps) for (int c = 0; c < 4; ++c) m.matrix[r][c] /= w;
    }
    return m;
}
bool MMatrix::isEquivalent(const MMatrix& o, double tol) const {
    for (int r = 0; r < 4; ++r) for (int c = 0; c < 4; ++c) if (std::fabs(matrix[r][c] - o.matrix[r][c]) > tol) return false;
    return true;
}
bool MMatrix::isSingular() const { return std::fabs(det4x4()) < 1e-12; }
MMatrix MMatrix::operator*(const MMatrix& o) const {
    MMatrix p;
    for (int r = 0; r < 4; ++r)
        for (int c = 0; c < 4; ++c) {
            double s = 0;
            for (int k = 0; k < 4; ++k) s += matrix[r][k] * o.matrix[k][c];
            p.matrix[r][c] = s;
        }
    return p;
}
MMatrix& MMatrix::operator*=(const MMatrix& o) { return *this = *this * o; }
MMatrix MMatrix::operator*(double s) const {
    MMatrix p(*this);
    for (int r = 0; r < 4; ++r) for (int c = 0; c < 4; ++c) p.matrix[r][c] *= s;
    return p;
}
MMatrix MMatrix::operator+(const MMatrix& o) const {
    MMatrix p(*this);
    for (int r = 0; r < 4; ++r) for (int c = 0; c < 4; ++c) p.matrix[r][c] += o.matrix[r][c];
    return p;
}
MMatrix MMatrix::operator-(const MMatrix& o) const {
    MMatrix p(*this);
    for (int r = 0; r < 4; ++r) for (int c = 0; c < 4; ++c) p.matrix[r][c] -= o.matrix[r][c];
    return p;
}
bool MMatrix::operator==(const MMatrix& o) const { return memcmp(matrix, o.matrix, sizeof matrix) == 0; }
bool MMatrix::operator!=(const MMatrix& o) const { return !(*this == o); }
std::ostream& operator<<(std::ostream& os, const MMatrix& m) {
    os << "[";
    for (int r = 0; r < 4; ++r) {
        os << (r ? ", [" : "[");
        for (int c = 0; c < 4; ++c) os << (c ? ", " : "") << m[r][c];
        os << "]";
    }
    return os << "]";
}

MFloatMatrix::MFloatMatrix() { setToIdentity(); }
MFloatMatrix::MFloatMatrix(const float m[4][4]) { memcpy(matrix, m, sizeof matrix); }
MFloatMatrix::MFloatMatrix(const double m[4][4]) {
    for (int r = 0; r < 4; ++r) for (int c = 0; c < 4; ++c) matrix[r][c] = (float)m[r][c];
}
MFloatMatrix::MFloatMatrix(const MMatrix& m) {
    for (int r = 0; r < 4; ++r) for (int c = 0; c < 4; ++c) matrix[r][c] = (float)m[r][c];
}
MStatus MFloatMatrix::get(float out[4][4]) const { memcpy(out, matrix, sizeof matrix); return MS::kSuccess; }
MFloatMatrix MFloatMatrix::transpose() const { return MFloatMatrix(MMatrix(*this).transpose()); }
MFloatMatrix& MFloatMatrix::setToIdentity() {
    for (int r = 0; r < 4; ++r) for (int c = 0; c < 4; ++c) matrix[r][c] = r == c ? 1.0f : 0.0f;
    return *this;
}
MFloatMatrix MFloatMatrix::inverse() const { return MFloatMatrix(MMatrix(*this).inverse()); }
MFloatMatrix MFloatMatrix::operator*(const MFloatMatrix& o) const { return MFloatMatrix(MMatrix(*this) * MMatrix(o)); }
bool MFloatMatrix::operator==(const MFloatMatrix& o) const { return memcmp(matrix, o.matrix, sizeof matrix) == 0; }
bool MFloatMatrix::operator!=(const MFloatMatrix& o) const { return !(*this == o); }

// ---------------------------------------------------------------------------------------------------------------------
// Rotations

// The axes each order applies, first to last.
static const int kOrderAxes[6][3] = {{0, 1, 2}, {1, 2, 0}, {2, 0, 1}, {0, 2, 1}, {1, 0, 2}, {2, 1, 0}};

static MMatrix axisMatrix(int axis, double a) {
    MMatrix m;
    double c = std::cos(a), s = std::sin(a);
    int i = (axis + 1) % 3, j = (axis + 2) % 3;
    m.matrix[i][i] = c;
    m.matrix[i][j] = s;
    m.matrix[j][i] = -s;
    m.matrix[j][j] = c;
    return m;
}

MMatrix MEulerRotation::asMatrix() const {
    const int* ax = kOrderAxes[order];
    double v[3] = {x, y, z};
    return axisMatrix(ax[0], v[ax[0]]) * axisMatrix(ax[1], v[ax[1]]) * axisMatrix(ax[2], v[ax[2]]);
}

MQuaternion MEulerRotation::asQuaternion() const {
    MQuaternion q;
    q = asMatrix();
    return q;
}

MEulerRotation MEulerRotation::decompose(const MMatrix& m, RotationOrder o) {
    const int* ax = kOrderAxes[o];
    // In column form C = M^T the first axis applied is the rightmost factor. Permuting the axes to x, y, z reduces every
    // order to one; an odd permutation turns each angle round.
    double c[3][3];
    for (int r = 0; r < 3; ++r) for (int k = 0; k < 3; ++k) c[r][k] = m[ax[k]][ax[r]];
    bool odd = !((ax[1] == (ax[0] + 1) % 3) && (ax[2] == (ax[1] + 1) % 3));
    double a0, a1, a2;
    double s = -c[2][0];
    if (s >= 1.0 - 1e-12) {
        a1 = M_PI / 2;
        a0 = std::atan2(c[0][1], c[1][1]);
        a2 = 0;
    } else if (s <= -1.0 + 1e-12) {
        a1 = -M_PI / 2;
        a0 = std::atan2(-c[0][1], c[1][1]);
        a2 = 0;
    } else {
        a1 = std::asin(s);
        a0 = std::atan2(c[2][1], c[2][2]);
        a2 = std::atan2(c[1][0], c[0][0]);
    }
    if (odd) { a0 = -a0; a1 = -a1; a2 = -a2; }
    double v[3];
    v[ax[0]] = a0;
    v[ax[1]] = a1;
    v[ax[2]] = a2;
    return MEulerRotation(v[0], v[1], v[2], o);
}

MEulerRotation MEulerRotation::reorder(RotationOrder o) const { return decompose(asMatrix(), o); }
MEulerRotation& MEulerRotation::reorderIt(RotationOrder o) { return *this = reorder(o); }

static double wrapPi(double a) {
    a = std::fmod(a + M_PI, 2 * M_PI);
    if (a < 0) a += 2 * M_PI;
    return a - M_PI;
}

MEulerRotation MEulerRotation::bound() const { return MEulerRotation(wrapPi(x), wrapPi(y), wrapPi(z), order); }

MEulerRotation MEulerRotation::closestSolution(const MEulerRotation& t) const {
    MEulerRotation a = *this;
    double v[3] = {a.x, a.y, a.z}, tv[3] = {t.x, t.y, t.z};
    for (int i = 0; i < 3; ++i) v[i] = tv[i] + wrapPi(v[i] - tv[i]);
    return MEulerRotation(v[0], v[1], v[2], order);
}

MEulerRotation& MEulerRotation::incrementalRotateBy(const MVector& axis, double angle) {
    MEulerRotation r = decompose(asMatrix() * MQuaternion(angle, axis).asMatrix(), order);
    return *this = r.closestSolution(*this);
}

bool MEulerRotation::isEquivalent(const MEulerRotation& o, double tol) const {
    return asMatrix().isEquivalent(o.asMatrix(), tol);
}
std::ostream& operator<<(std::ostream& os, const MEulerRotation& e) { return os << "[" << e.x << ", " << e.y << ", " << e.z << "]"; }

MQuaternion::MQuaternion(double angle, const MVector& axis) { setAxisAngle(axis, angle); }
MQuaternion::MQuaternion(const MVector& a, const MVector& b) {
    MVector u = a.normal(), v = b.normal();
    double d = u * v;
    if (d < -1.0 + 1e-12) {
        MVector p = u ^ MVector::xAxis;
        if (p.length() < 1e-6) p = u ^ MVector::yAxis;
        setAxisAngle(p, M_PI);
        return;
    }
    MVector c = u ^ v;
    x = c.x; y = c.y; z = c.z; w = 1.0 + d;
    normalizeIt();
}
MQuaternion::MQuaternion(const MVector& a, const MVector& b, double factor) {
    MQuaternion full(a, b);
    MVector axis;
    double ang;
    full.getAxisAngle(axis, ang);
    setAxisAngle(axis, ang * factor);
}

// q1 * q2 applies q1 first, as matrices do.
MQuaternion MQuaternion::operator*(const MQuaternion& o) const {
    const MQuaternion& a = o;
    const MQuaternion& b = *this;
    return MQuaternion(a.w * b.x + a.x * b.w + a.y * b.z - a.z * b.y, a.w * b.y - a.x * b.z + a.y * b.w + a.z * b.x,
                       a.w * b.z + a.x * b.y - a.y * b.x + a.z * b.w, a.w * b.w - a.x * b.x - a.y * b.y - a.z * b.z);
}
MQuaternion& MQuaternion::operator*=(const MQuaternion& o) { return *this = *this * o; }

MMatrix MQuaternion::asMatrix() const {
    MQuaternion q = normal();
    double xx = q.x * q.x, yy = q.y * q.y, zz = q.z * q.z, xy = q.x * q.y, xz = q.x * q.z, yz = q.y * q.z, wx = q.w * q.x, wy = q.w * q.y, wz = q.w * q.z;
    MMatrix m;
    m.matrix[0][0] = 1 - 2 * (yy + zz);
    m.matrix[0][1] = 2 * (xy + wz);
    m.matrix[0][2] = 2 * (xz - wy);
    m.matrix[1][0] = 2 * (xy - wz);
    m.matrix[1][1] = 1 - 2 * (xx + zz);
    m.matrix[1][2] = 2 * (yz + wx);
    m.matrix[2][0] = 2 * (xz + wy);
    m.matrix[2][1] = 2 * (yz - wx);
    m.matrix[2][2] = 1 - 2 * (xx + yy);
    return m;
}

MQuaternion& MQuaternion::operator=(const MMatrix& m) {
    // Column form c = M^T; the rotation part only, rows normalised so scale doesn't count.
    double r[3][3];
    for (int i = 0; i < 3; ++i) {
        double l = std::sqrt(m[i][0] * m[i][0] + m[i][1] * m[i][1] + m[i][2] * m[i][2]);
        if (l < kEps) l = 1;
        for (int k = 0; k < 3; ++k) r[i][k] = m[i][k] / l;
    }
    double c00 = r[0][0], c11 = r[1][1], c22 = r[2][2];
    double c21 = r[1][2], c12 = r[2][1], c02 = r[2][0], c20 = r[0][2], c10 = r[0][1], c01 = r[1][0];
    double t = c00 + c11 + c22;
    if (t > 0) {
        double s = std::sqrt(t + 1.0) * 2;
        w = 0.25 * s; x = (c21 - c12) / s; y = (c02 - c20) / s; z = (c10 - c01) / s;
    } else if (c00 > c11 && c00 > c22) {
        double s = std::sqrt(1.0 + c00 - c11 - c22) * 2;
        w = (c21 - c12) / s; x = 0.25 * s; y = (c01 + c10) / s; z = (c02 + c20) / s;
    } else if (c11 > c22) {
        double s = std::sqrt(1.0 + c11 - c00 - c22) * 2;
        w = (c02 - c20) / s; x = (c01 + c10) / s; y = 0.25 * s; z = (c12 + c21) / s;
    } else {
        double s = std::sqrt(1.0 + c22 - c00 - c11) * 2;
        w = (c10 - c01) / s; x = (c02 + c20) / s; y = (c12 + c21) / s; z = 0.25 * s;
    }
    return *this;
}

MQuaternion& MQuaternion::operator=(const MEulerRotation& e) { return *this = e.asMatrix(); }
MEulerRotation MQuaternion::asEulerRotation() const { return MEulerRotation::decompose(asMatrix(), MEulerRotation::kXYZ); }

MQuaternion& MQuaternion::setAxisAngle(const MVector& axis, double angle) {
    MVector a = axis.normal();
    double s = std::sin(angle / 2);
    x = a.x * s; y = a.y * s; z = a.z * s; w = std::cos(angle / 2);
    return *this;
}
bool MQuaternion::getAxisAngle(MVector& axis, double& angle) const {
    MQuaternion q = normal();
    double s = std::sqrt(q.x * q.x + q.y * q.y + q.z * q.z);
    angle = 2 * std::atan2(s, q.w);
    axis = s > kEps ? MVector(q.x / s, q.y / s, q.z / s) : MVector(0, 0, 1);
    return s > kEps;
}
MQuaternion MQuaternion::normal() const {
    double l = std::sqrt(x * x + y * y + z * z + w * w);
    return l > kEps ? MQuaternion(x / l, y / l, z / l, w / l) : *this;
}
MQuaternion& MQuaternion::normalizeIt() { return *this = normal(); }
MQuaternion MQuaternion::conjugate() const { return MQuaternion(-x, -y, -z, w); }
MQuaternion& MQuaternion::conjugateIt() { return *this = conjugate(); }
MQuaternion MQuaternion::inverse() const {
    double n = x * x + y * y + z * z + w * w;
    if (n < kEps) return *this;
    return MQuaternion(-x / n, -y / n, -z / n, w / n);
}
MQuaternion& MQuaternion::invertIt() { return *this = inverse(); }
MQuaternion MQuaternion::log() const {
    double s = std::sqrt(x * x + y * y + z * z);
    double a = std::atan2(s, w);
    double k = s > kEps ? a / s : 1.0;
    return MQuaternion(x * k, y * k, z * k, 0);
}
MQuaternion MQuaternion::exp() const {
    double a = std::sqrt(x * x + y * y + z * z);
    double k = a > kEps ? std::sin(a) / a : 1.0;
    return MQuaternion(x * k, y * k, z * k, std::cos(a));
}
MQuaternion& MQuaternion::setToXAxis(double a) { return setAxisAngle(MVector::xAxis, a); }
MQuaternion& MQuaternion::setToYAxis(double a) { return setAxisAngle(MVector::yAxis, a); }
MQuaternion& MQuaternion::setToZAxis(double a) { return setAxisAngle(MVector::zAxis, a); }
bool MQuaternion::isEquivalent(const MQuaternion& o, double tol) const {
    double d = std::fabs(x * o.x + y * o.y + z * o.z + w * o.w);
    return std::fabs(d - 1.0) <= tol;
}

MQuaternion slerp(const MQuaternion& a, const MQuaternion& b0, double t, short spin) {
    MQuaternion b = b0;
    double d = a.x * b.x + a.y * b.y + a.z * b.z + a.w * b.w;
    if (d < 0 && spin == 0) { b = -b; d = -d; }
    if (d > 1 - 1e-9) {
        MQuaternion r(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t, a.z + (b.z - a.z) * t, a.w + (b.w - a.w) * t);
        return r.normal();
    }
    double th = std::acos(std::max(-1.0, std::min(1.0, d))) + spin * M_PI;
    double s = std::sin(th);
    double ka = std::sin((1 - t) * th) / s, kb = std::sin(t * th) / s;
    return MQuaternion(a.x * ka + b.x * kb, a.y * ka + b.y * kb, a.z * ka + b.z * kb, a.w * ka + b.w * kb);
}
std::ostream& operator<<(std::ostream& os, const MQuaternion& q) { return os << "[" << q.x << ", " << q.y << ", " << q.z << ", " << q.w << "]"; }

// ---------------------------------------------------------------------------------------------------------------------
// Transformation matrices: scale, shear, rotate, translate, with pivots that don't move anything here.

MTransformationMatrix::MTransformationMatrix() {
    for (int i = 0; i < 3; ++i) { s_[i] = 1; sh_[i] = 0; }
}

MTransformationMatrix::MTransformationMatrix(const MMatrix& m) {
    MVector r0(m[0][0], m[0][1], m[0][2]), r1(m[1][0], m[1][1], m[1][2]), r2(m[2][0], m[2][1], m[2][2]);
    double sx = r0.length();
    MVector n0 = sx > kEps ? r0 / sx : MVector::xAxis;
    double p01 = r1 * n0;
    MVector q1 = r1 - n0 * p01;
    double sy = q1.length();
    MVector n1 = sy > kEps ? q1 / sy : MVector::yAxis;
    double p02 = r2 * n0, p12 = r2 * n1;
    MVector q2 = r2 - n0 * p02 - n1 * p12;
    double sz = q2.length();
    MVector n2 = sz > kEps ? q2 / sz : (n0 ^ n1);
    if (((n0 ^ n1) * n2) < 0) { sz = -sz; n2 = -n2; }
    s_[0] = sx; s_[1] = sy; s_[2] = sz;
    sh_[0] = sy > kEps ? p01 / sy : 0;
    sh_[1] = sz != 0 ? p02 / sz : 0;
    sh_[2] = sz != 0 ? p12 / sz : 0;
    MMatrix rot;
    for (int k = 0; k < 3; ++k) { rot.matrix[0][k] = n0[k]; rot.matrix[1][k] = n1[k]; rot.matrix[2][k] = n2[k]; }
    r_ = MEulerRotation::decompose(rot, MEulerRotation::kXYZ);
    t_ = MVector(m[3][0], m[3][1], m[3][2]);
}

MMatrix MTransformationMatrix::asScaleMatrix() const {
    MMatrix s, sh;
    for (int i = 0; i < 3; ++i) s.matrix[i][i] = s_[i];
    sh.matrix[1][0] = sh_[0];
    sh.matrix[2][0] = sh_[1];
    sh.matrix[2][1] = sh_[2];
    return s * sh;
}
MMatrix MTransformationMatrix::asRotateMatrix() const { return ro_.asMatrix() * r_.asMatrix(); }
MMatrix MTransformationMatrix::asMatrix() const {
    MMatrix m = asScaleMatrix() * asRotateMatrix();
    m.matrix[3][0] = t_.x; m.matrix[3][1] = t_.y; m.matrix[3][2] = t_.z;
    return m;
}
MMatrix MTransformationMatrix::asMatrix(double percent) const {
    MTransformationMatrix t;
    double s[3];
    for (int i = 0; i < 3; ++i) s[i] = 1 + (s_[i] - 1) * percent;
    t.setScale(s, MSpace::kTransform);
    t.r_ = MEulerRotation::decompose(slerp(MQuaternion(), r_.asQuaternion(), percent).asMatrix(), r_.order);
    t.t_ = t_ * percent;
    return t.asMatrix();
}
MMatrix MTransformationMatrix::asMatrixInverse() const { return asMatrix().inverse(); }
MVector MTransformationMatrix::getTranslation(MSpace::Space, MStatus* st) const { if (st) *st = MS::kSuccess; return t_; }
MVector MTransformationMatrix::translation(MSpace::Space sp, MStatus* st) const { return getTranslation(sp, st); }
MStatus MTransformationMatrix::setTranslation(const MVector& v, MSpace::Space) { t_ = v; return MS::kSuccess; }
MStatus MTransformationMatrix::addTranslation(const MVector& v, MSpace::Space) { t_ += v; return MS::kSuccess; }
MStatus MTransformationMatrix::getScale(double out[3], MSpace::Space) const { for (int i = 0; i < 3; ++i) out[i] = s_[i]; return MS::kSuccess; }
MStatus MTransformationMatrix::setScale(const double s[3], MSpace::Space) { for (int i = 0; i < 3; ++i) s_[i] = s[i]; return MS::kSuccess; }
MStatus MTransformationMatrix::addScale(const double s[3], MSpace::Space) { for (int i = 0; i < 3; ++i) s_[i] += s[i]; return MS::kSuccess; }
MStatus MTransformationMatrix::getShear(double out[3], MSpace::Space) const { for (int i = 0; i < 3; ++i) out[i] = sh_[i]; return MS::kSuccess; }
MStatus MTransformationMatrix::setShear(const double s[3], MSpace::Space) { for (int i = 0; i < 3; ++i) sh_[i] = s[i]; return MS::kSuccess; }
MQuaternion MTransformationMatrix::rotation() const { return r_.asQuaternion(); }
MStatus MTransformationMatrix::getRotation(double out[3], RotationOrder& order) const {
    out[0] = r_.x; out[1] = r_.y; out[2] = r_.z;
    order = (RotationOrder)(r_.order + 1);
    return MS::kSuccess;
}
MStatus MTransformationMatrix::setRotation(const double r[3], RotationOrder order) {
    if (order <= kInvalid || order >= kLast) return MS::kInvalidParameter;
    r_ = MEulerRotation(r[0], r[1], r[2], (MEulerRotation::RotationOrder)(order - 1));
    return MS::kSuccess;
}
MStatus MTransformationMatrix::setRotationQuaternion(double x, double y, double z, double w, MSpace::Space) {
    rotateTo(MQuaternion(x, y, z, w));
    return MS::kSuccess;
}
MStatus MTransformationMatrix::getRotationQuaternion(double& x, double& y, double& z, double& w, MSpace::Space) const {
    MQuaternion q = rotation();
    x = q.x; y = q.y; z = q.z; w = q.w;
    return MS::kSuccess;
}
MTransformationMatrix& MTransformationMatrix::rotateTo(const MQuaternion& q) {
    r_ = MEulerRotation::decompose(q.asMatrix(), r_.order);
    return *this;
}
MTransformationMatrix& MTransformationMatrix::rotateTo(const MEulerRotation& e) { r_ = e; return *this; }
MTransformationMatrix& MTransformationMatrix::rotateBy(const MQuaternion& q, MSpace::Space space, MStatus* st) {
    MMatrix m = space == MSpace::kWorld ? r_.asMatrix() * q.asMatrix() : q.asMatrix() * r_.asMatrix();
    r_ = MEulerRotation::decompose(m, r_.order);
    if (st) *st = MS::kSuccess;
    return *this;
}
MTransformationMatrix& MTransformationMatrix::rotateBy(const MEulerRotation& e, MSpace::Space space, MStatus* st) {
    return rotateBy(e.asQuaternion(), space, st);
}
MEulerRotation MTransformationMatrix::eulerRotation() const { return r_; }
MQuaternion MTransformationMatrix::rotationOrientation() const { return ro_; }
MStatus MTransformationMatrix::setRotationOrientation(const MQuaternion& q) { ro_ = q; return MS::kSuccess; }
MTransformationMatrix::RotationOrder MTransformationMatrix::rotationOrder(MStatus* st) const {
    if (st) *st = MS::kSuccess;
    return (RotationOrder)(r_.order + 1);
}
MStatus MTransformationMatrix::reorderRotation(RotationOrder order) {
    if (order <= kInvalid || order >= kLast) return MS::kInvalidParameter;
    r_.reorderIt((MEulerRotation::RotationOrder)(order - 1));
    return MS::kSuccess;
}
MPoint MTransformationMatrix::scalePivot(MSpace::Space, MStatus* st) const { if (st) *st = MS::kSuccess; return sp_; }
MStatus MTransformationMatrix::setScalePivot(const MPoint& p, MSpace::Space, bool) { sp_ = p; return MS::kSuccess; }
MPoint MTransformationMatrix::rotatePivot(MSpace::Space, MStatus* st) const { if (st) *st = MS::kSuccess; return rp_; }
MStatus MTransformationMatrix::setRotatePivot(const MPoint& p, MSpace::Space, bool) { rp_ = p; return MS::kSuccess; }
bool MTransformationMatrix::isEquivalent(const MTransformationMatrix& o, double tol) const { return asMatrix().isEquivalent(o.asMatrix(), tol); }
bool MTransformationMatrix::operator==(const MTransformationMatrix& o) const { return asMatrix() == o.asMatrix(); }
bool MTransformationMatrix::operator!=(const MTransformationMatrix& o) const { return !(*this == o); }

// ---------------------------------------------------------------------------------------------------------------------

MBoundingBox::MBoundingBox() : empty_(true) {}
MBoundingBox::MBoundingBox(const MPoint& a, const MPoint& b) : empty_(true) { expand(a); expand(b); }
void MBoundingBox::clear() { empty_ = true; lo_ = hi_ = MPoint(); }
void MBoundingBox::expand(const MPoint& p0) {
    MPoint p = p0;
    p.cartesianize();
    if (empty_) { lo_ = hi_ = p; empty_ = false; return; }
    lo_.x = std::min(lo_.x, p.x); lo_.y = std::min(lo_.y, p.y); lo_.z = std::min(lo_.z, p.z);
    hi_.x = std::max(hi_.x, p.x); hi_.y = std::max(hi_.y, p.y); hi_.z = std::max(hi_.z, p.z);
}
void MBoundingBox::expand(const MBoundingBox& b) {
    if (b.empty_) return;
    expand(b.lo_);
    expand(b.hi_);
}
void MBoundingBox::transformUsing(const MMatrix& m) {
    if (empty_) return;
    MPoint lo = lo_, hi = hi_;
    clear();
    for (int i = 0; i < 8; ++i) expand(MPoint(i & 1 ? hi.x : lo.x, i & 2 ? hi.y : lo.y, i & 4 ? hi.z : lo.z) * m);
}
bool MBoundingBox::contains(const MPoint& p) const {
    return !empty_ && p.x >= lo_.x && p.x <= hi_.x && p.y >= lo_.y && p.y <= hi_.y && p.z >= lo_.z && p.z <= hi_.z;
}
bool MBoundingBox::intersects(const MBoundingBox& b, double tol) const {
    if (empty_ || b.empty_) return false;
    return lo_.x <= b.hi_.x + tol && hi_.x >= b.lo_.x - tol && lo_.y <= b.hi_.y + tol && hi_.y >= b.lo_.y - tol && lo_.z <= b.hi_.z + tol && hi_.z >= b.lo_.z - tol;
}
double MBoundingBox::width() const { return empty_ ? 0 : hi_.x - lo_.x; }
double MBoundingBox::height() const { return empty_ ? 0 : hi_.y - lo_.y; }
double MBoundingBox::depth() const { return empty_ ? 0 : hi_.z - lo_.z; }
MPoint MBoundingBox::center() const { return MPoint((lo_.x + hi_.x) / 2, (lo_.y + hi_.y) / 2, (lo_.z + hi_.z) / 2); }
MPoint MBoundingBox::min() const { return lo_; }
MPoint MBoundingBox::max() const { return hi_; }
