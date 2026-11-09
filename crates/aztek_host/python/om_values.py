# aztek.api.OpenMaya, value classes: vectors, points, matrices, rotations, units, arrays. No scene access here.
# The reference's conventions: row vectors (p * m), matrices flat row by row with the translation in elements 12, 13, 14.
from json import loads as _json_loads
import math as _math
import struct as _struct


def _f32(x):
    return _struct.unpack("f", _struct.pack("f", x))[0]


def _fail(status="kInvalidParameter", message="Object is incompatible with this method"):
    raise RuntimeError("(%s): %s" % (status, message))


def _num(x):
    return "%.6g" % x


class MSpace:
    kInvalid = 0
    kTransform = 1
    kPreTransform = 2
    kPostTransform = 3
    kWorld = 4
    kObject = 2
    kLast = 5


class _Triple:
    """x, y, z as doubles or, in the Float classes, floats."""

    _conv = float
    __slots__ = ("x", "y", "z")

    def __init__(self, *args):
        c = self._conv
        if not args:
            self.x = self.y = self.z = c(0.0)
        elif len(args) == 1:
            s = list(args[0])
            if len(s) < 3:
                s = s + [0.0] * (3 - len(s))
            self.x, self.y, self.z = c(s[0]), c(s[1]), c(s[2])
        elif len(args) in (2, 3):
            self.x, self.y = c(args[0]), c(args[1])
            self.z = c(args[2]) if len(args) == 3 else c(0.0)
        else:
            _fail(message="Too many arguments")

    def __len__(self):
        return 3

    def __getitem__(self, i):
        return (self.x, self.y, self.z)[i]

    def __setitem__(self, i, v):
        setattr(self, "xyz"[range(3)[i]], self._conv(v))

    def __iter__(self):
        return iter((self.x, self.y, self.z))

    def __eq__(self, o):
        try:
            return self.x == o.x and self.y == o.y and self.z == o.z
        except AttributeError:
            return NotImplemented

    def __ne__(self, o):
        r = self.__eq__(o)
        return r if r is NotImplemented else not r

    __hash__ = None

    def __str__(self):
        return "(%s, %s, %s)" % (_num(self.x), _num(self.y), _num(self.z))

    def __repr__(self):
        return "aztek.api.OpenMaya.%s(%s, %s, %s)" % (type(self).__name__, _num(self.x), _num(self.y), _num(self.z))


class MVector(_Triple):
    __slots__ = ()

    def __add__(self, o):
        return type(self)(self.x + o[0], self.y + o[1], self.z + o[2])

    def __iadd__(self, o):
        c = self._conv
        self.x, self.y, self.z = c(self.x + o[0]), c(self.y + o[1]), c(self.z + o[2])
        return self

    def __sub__(self, o):
        return type(self)(self.x - o[0], self.y - o[1], self.z - o[2])

    def __isub__(self, o):
        c = self._conv
        self.x, self.y, self.z = c(self.x - o[0]), c(self.y - o[1]), c(self.z - o[2])
        return self

    def __neg__(self):
        return type(self)(-self.x, -self.y, -self.z)

    def __mul__(self, o):
        if isinstance(o, (int, float)):
            return type(self)(self.x * o, self.y * o, self.z * o)
        if isinstance(o, _MatrixBase):
            # A vector takes the matrix's rotation and scale, not its translation.
            m = o._m
            return type(self)(
                self.x * m[0] + self.y * m[4] + self.z * m[8],
                self.x * m[1] + self.y * m[5] + self.z * m[9],
                self.x * m[2] + self.y * m[6] + self.z * m[10],
            )
        if isinstance(o, MQuaternion):
            return self.rotateBy(o)
        if isinstance(o, _Triple):
            return self.x * o.x + self.y * o.y + self.z * o.z
        return NotImplemented

    def __rmul__(self, o):
        if isinstance(o, (int, float)):
            return type(self)(self.x * o, self.y * o, self.z * o)
        return NotImplemented

    def __imul__(self, o):
        r = self * o
        self.x, self.y, self.z = r.x, r.y, r.z
        return self

    def __truediv__(self, o):
        return type(self)(self.x / o, self.y / o, self.z / o)

    def __itruediv__(self, o):
        c = self._conv
        self.x, self.y, self.z = c(self.x / o), c(self.y / o), c(self.z / o)
        return self

    def __xor__(self, o):
        return type(self)(self.y * o[2] - self.z * o[1], self.z * o[0] - self.x * o[2], self.x * o[1] - self.y * o[0])

    def length(self):
        return _math.sqrt(self.x * self.x + self.y * self.y + self.z * self.z)

    def normal(self):
        n = self.length()
        return type(self)(self) if n == 0.0 else type(self)(self.x / n, self.y / n, self.z / n)

    def normalize(self):
        n = self.length()
        if n != 0.0:
            c = self._conv
            self.x, self.y, self.z = c(self.x / n), c(self.y / n), c(self.z / n)
        return self

    def angle(self, o):
        d = self.length() * _length(o)
        if d == 0.0:
            return 0.0
        return _math.acos(max(-1.0, min(1.0, (self.x * o[0] + self.y * o[1] + self.z * o[2]) / d)))

    def isEquivalent(self, o, tolerance=1e-10):
        return abs(self.x - o[0]) <= tolerance and abs(self.y - o[1]) <= tolerance and abs(self.z - o[2]) <= tolerance

    def isParallel(self, o, tolerance=1e-10):
        a, b = self.normal(), type(self)(o).normal()
        return abs(abs(a * b) - 1.0) <= tolerance

    def rotateBy(self, *args):
        """rotateBy(MQuaternion), rotateBy(MEulerRotation), or rotateBy(axis, angle)."""
        if len(args) == 2:
            axis, ang = args
            if isinstance(axis, int):
                axis = (MVector.kXaxisVector, MVector.kYaxisVector, MVector.kZaxisVector)[axis]
            q = MQuaternion(ang, axis)
        else:
            q = args[0]
            if isinstance(q, MEulerRotation):
                q = q.asQuaternion()
            elif not isinstance(q, MQuaternion):
                _fail()
        m = q.asMatrix()
        return self * m

    def rotateTo(self, target, factor=1.0):
        return MQuaternion(self, target, factor)

    def transformAsNormal(self, m):
        inv = m.inverse().transpose()
        return (MVector(self) * inv)


def _length(v):
    return _math.sqrt(v[0] * v[0] + v[1] * v[1] + v[2] * v[2])


MVector.kZeroVector = MVector(0, 0, 0)
MVector.kOneVector = MVector(1, 1, 1)
MVector.kXaxisVector = MVector(1, 0, 0)
MVector.kYaxisVector = MVector(0, 1, 0)
MVector.kZaxisVector = MVector(0, 0, 1)
MVector.kXnegAxisVector = MVector(-1, 0, 0)
MVector.kYnegAxisVector = MVector(0, -1, 0)
MVector.kZnegAxisVector = MVector(0, 0, -1)
MVector.kXaxis = 0
MVector.kYaxis = 1
MVector.kZaxis = 2
MVector.kWaxis = 3
MVector.kTolerance = 1e-10


class MFloatVector(MVector):
    _conv = staticmethod(_f32)
    __slots__ = ()


MFloatVector.kZeroVector = MFloatVector(0, 0, 0)
MFloatVector.kOneVector = MFloatVector(1, 1, 1)
MFloatVector.kXaxisVector = MFloatVector(1, 0, 0)
MFloatVector.kYaxisVector = MFloatVector(0, 1, 0)
MFloatVector.kZaxisVector = MFloatVector(0, 0, 1)
MFloatVector.kXnegAxisVector = MFloatVector(-1, 0, 0)
MFloatVector.kYnegAxisVector = MFloatVector(0, -1, 0)
MFloatVector.kZnegAxisVector = MFloatVector(0, 0, -1)
MFloatVector.kTolerance = 1e-5


class MPoint:
    _conv = float
    _vector = MVector
    __slots__ = ("x", "y", "z", "w")

    def __init__(self, *args):
        c = self._conv
        if not args:
            self.x = self.y = self.z = c(0.0)
            self.w = c(1.0)
        elif len(args) == 1:
            a = args[0]
            if isinstance(a, MPoint):
                self.x, self.y, self.z, self.w = c(a.x), c(a.y), c(a.z), c(a.w)
            else:
                s = list(a)
                if len(s) < 2 or len(s) > 4:
                    _fail(message="Expected a sequence of 2 to 4 numbers")
                s = s + [0.0] * (3 - len(s)) if len(s) < 3 else s
                self.x, self.y, self.z = c(s[0]), c(s[1]), c(s[2])
                self.w = c(s[3]) if len(s) == 4 else c(1.0)
        elif 2 <= len(args) <= 4:
            self.x, self.y = c(args[0]), c(args[1])
            self.z = c(args[2]) if len(args) > 2 else c(0.0)
            self.w = c(args[3]) if len(args) > 3 else c(1.0)
        else:
            _fail(message="Too many arguments")

    def __len__(self):
        return 4

    def __getitem__(self, i):
        return (self.x, self.y, self.z, self.w)[i]

    def __setitem__(self, i, v):
        setattr(self, "xyzw"[range(4)[i]], self._conv(v))

    def __iter__(self):
        return iter((self.x, self.y, self.z, self.w))

    def __eq__(self, o):
        if not isinstance(o, MPoint):
            return NotImplemented
        return self.x * o.w == o.x * self.w and self.y * o.w == o.y * self.w and self.z * o.w == o.z * self.w

    def __ne__(self, o):
        r = self.__eq__(o)
        return r if r is NotImplemented else not r

    __hash__ = None

    def __add__(self, o):
        if isinstance(o, MPoint):
            return type(self)(self.x + o.x, self.y + o.y, self.z + o.z, self.w)
        return type(self)(self.x + o[0], self.y + o[1], self.z + o[2], self.w)

    def __iadd__(self, o):
        c = self._conv
        self.x, self.y, self.z = c(self.x + o[0]), c(self.y + o[1]), c(self.z + o[2])
        return self

    def __sub__(self, o):
        if isinstance(o, MPoint):
            return self._vector(self.x / self.w - o.x / o.w, self.y / self.w - o.y / o.w, self.z / self.w - o.z / o.w)
        return type(self)(self.x - o[0], self.y - o[1], self.z - o[2], self.w)

    def __isub__(self, o):
        c = self._conv
        self.x, self.y, self.z = c(self.x - o[0]), c(self.y - o[1]), c(self.z - o[2])
        return self

    def __neg__(self):
        return type(self)(-self.x, -self.y, -self.z, self.w)

    def __mul__(self, o):
        if isinstance(o, (int, float)):
            return type(self)(self.x * o, self.y * o, self.z * o, self.w)
        if isinstance(o, _MatrixBase):
            m = o._m
            x, y, z, w = self.x, self.y, self.z, self.w
            return type(self)(
                x * m[0] + y * m[4] + z * m[8] + w * m[12],
                x * m[1] + y * m[5] + z * m[9] + w * m[13],
                x * m[2] + y * m[6] + z * m[10] + w * m[14],
                x * m[3] + y * m[7] + z * m[11] + w * m[15],
            )
        return NotImplemented

    def __rmul__(self, o):
        if isinstance(o, (int, float)):
            return self * o
        return NotImplemented

    def __imul__(self, o):
        r = self * o
        self.x, self.y, self.z, self.w = r.x, r.y, r.z, r.w
        return self

    def __truediv__(self, o):
        return type(self)(self.x / o, self.y / o, self.z / o, self.w)

    def cartesianize(self):
        c = self._conv
        if self.w != 0.0:
            self.x, self.y, self.z = c(self.x / self.w), c(self.y / self.w), c(self.z / self.w)
            self.w = c(1.0)
        return self

    def rationalize(self):
        c = self._conv
        if self.w != 0.0:
            self.x, self.y, self.z = c(self.x / self.w), c(self.y / self.w), c(self.z / self.w)
        return self

    def homogenize(self):
        c = self._conv
        self.x, self.y, self.z = c(self.x * self.w), c(self.y * self.w), c(self.z * self.w)
        return self

    def distanceTo(self, o):
        return (self - o).length()

    def isEquivalent(self, o, tolerance=1e-10):
        return abs(self.x - o.x) <= tolerance and abs(self.y - o.y) <= tolerance and abs(self.z - o.z) <= tolerance and abs(self.w - o.w) <= tolerance

    def __str__(self):
        return "(%s, %s, %s, %s)" % (_num(self.x), _num(self.y), _num(self.z), _num(self.w))

    def __repr__(self):
        return "aztek.api.OpenMaya.%s(%s, %s, %s, %s)" % (type(self).__name__, _num(self.x), _num(self.y), _num(self.z), _num(self.w))


MPoint.kOrigin = MPoint(0, 0, 0)
MPoint.kTolerance = 1e-10


class MFloatPoint(MPoint):
    _conv = staticmethod(_f32)
    _vector = MFloatVector
    __slots__ = ()


MFloatPoint.kOrigin = MFloatPoint(0, 0, 0)
MFloatPoint.kTolerance = 1e-5


class MColor:
    kRGB = 1
    kHSV = 2
    kCMY = 3
    kCMYK = 4
    kFloat = 1
    kShort = 2
    kByte = 3
    kOpaqueBlack = None
    __slots__ = ("r", "g", "b", "a")

    def __init__(self, *args):
        if not args:
            self.r = self.g = self.b = 0.0
            self.a = 1.0
            return
        if len(args) >= 1 and not isinstance(args[0], (int, float)):
            s = list(args[0])
        else:
            s = list(args)
        s = s + [0.0] * (3 - len(s)) if len(s) < 3 else s
        self.r, self.g, self.b = _f32(s[0]), _f32(s[1]), _f32(s[2])
        self.a = _f32(s[3]) if len(s) > 3 else 1.0

    def __len__(self):
        return 4

    def __getitem__(self, i):
        return (self.r, self.g, self.b, self.a)[i]

    def __setitem__(self, i, v):
        setattr(self, "rgba"[range(4)[i]], _f32(v))

    def __iter__(self):
        return iter((self.r, self.g, self.b, self.a))

    def __eq__(self, o):
        return isinstance(o, MColor) and tuple(self) == tuple(o)

    def __ne__(self, o):
        return not self == o

    __hash__ = None

    def __add__(self, o):
        return MColor(self.r + o.r, self.g + o.g, self.b + o.b, self.a + o.a)

    def __sub__(self, o):
        return MColor(self.r - o.r, self.g - o.g, self.b - o.b, self.a - o.a)

    def __neg__(self):
        return MColor(-self.r, -self.g, -self.b, -self.a)

    def __mul__(self, o):
        if isinstance(o, MColor):
            return MColor(self.r * o.r, self.g * o.g, self.b * o.b, self.a * o.a)
        return MColor(self.r * o, self.g * o, self.b * o, self.a * o)

    __rmul__ = __mul__

    def __truediv__(self, o):
        return MColor(self.r / o, self.g / o, self.b / o, self.a / o)

    def getColor(self, model=1):
        return (self.r, self.g, self.b, self.a)

    def setColor(self, seq, model=1, dataType=1):
        s = list(seq)
        self.r, self.g, self.b = _f32(s[0]), _f32(s[1]), _f32(s[2])
        if len(s) > 3:
            self.a = _f32(s[3])
        return self

    def __str__(self):
        return "(%s, %s, %s, %s)" % (_num(self.r), _num(self.g), _num(self.b), _num(self.a))

    def __repr__(self):
        return "aztek.api.OpenMaya.MColor" + str(self)


MColor.kOpaqueBlack = MColor(0, 0, 0, 1)


class _MatrixBase:
    __slots__ = ("_m",)
    _conv = float

    def __init__(self, src=None):
        c = self._conv
        if src is None:
            self._m = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0]
        elif isinstance(src, _MatrixBase):
            self._m = [c(x) for x in src._m]
        else:
            s = list(src)
            if len(s) == 4 and not isinstance(s[0], (int, float)):
                s = [x for row in s for x in row]
            if len(s) != 16:
                _fail(message="Matrix needs 16 values")
            self._m = [c(x) for x in s]

    def __len__(self):
        return 16

    def __getitem__(self, i):
        return self._m[i]

    def __setitem__(self, i, v):
        self._m[i] = self._conv(v)

    def __iter__(self):
        return iter(self._m)

    def getElement(self, row, col):
        return self._m[row * 4 + col]

    def setElement(self, row, col, value):
        self._m[row * 4 + col] = self._conv(value)
        return self

    def setToIdentity(self):
        self._m = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0]
        return self

    def setToProduct(self, a, b):
        self._m = (a * b)._m
        return self

    def __eq__(self, o):
        if not isinstance(o, _MatrixBase):
            return NotImplemented
        return self._m == o._m

    def __ne__(self, o):
        r = self.__eq__(o)
        return r if r is NotImplemented else not r

    __hash__ = None

    def __add__(self, o):
        return type(self)([a + b for a, b in zip(self._m, o._m)])

    def __iadd__(self, o):
        self._m = [self._conv(a + b) for a, b in zip(self._m, o._m)]
        return self

    def __sub__(self, o):
        return type(self)([a - b for a, b in zip(self._m, o._m)])

    def __isub__(self, o):
        self._m = [self._conv(a - b) for a, b in zip(self._m, o._m)]
        return self

    def __mul__(self, o):
        if isinstance(o, (int, float)):
            return type(self)([a * o for a in self._m])
        if isinstance(o, _MatrixBase):
            a, b = self._m, o._m
            out = [0.0] * 16
            for r in range(4):
                i = r * 4
                a0, a1, a2, a3 = a[i], a[i + 1], a[i + 2], a[i + 3]
                out[i] = a0 * b[0] + a1 * b[4] + a2 * b[8] + a3 * b[12]
                out[i + 1] = a0 * b[1] + a1 * b[5] + a2 * b[9] + a3 * b[13]
                out[i + 2] = a0 * b[2] + a1 * b[6] + a2 * b[10] + a3 * b[14]
                out[i + 3] = a0 * b[3] + a1 * b[7] + a2 * b[11] + a3 * b[15]
            return type(self)(out)
        return NotImplemented

    def __rmul__(self, o):
        if isinstance(o, (int, float)):
            return type(self)([a * o for a in self._m])
        return NotImplemented

    def __imul__(self, o):
        self._m = (self * o)._m
        return self

    def transpose(self):
        m = self._m
        return type(self)([m[0], m[4], m[8], m[12], m[1], m[5], m[9], m[13], m[2], m[6], m[10], m[14], m[3], m[7], m[11], m[15]])

    def det3x3(self):
        m = self._m
        return m[0] * (m[5] * m[10] - m[6] * m[9]) - m[1] * (m[4] * m[10] - m[6] * m[8]) + m[2] * (m[4] * m[9] - m[5] * m[8])

    def _cofactors(self):
        a, b, c, d, e, f, g, h, i, j, k, l, m, n, o, p = self._m
        s0, s1, s2, s3, s4, s5 = a * f - e * b, a * g - e * c, a * h - e * d, b * g - f * c, b * h - f * d, c * h - g * d
        c5, c4, c3, c2, c1, c0 = k * p - o * l, j * p - n * l, j * o - n * k, i * p - m * l, i * o - m * k, i * n - m * j
        adj = [
            f * c5 - g * c4 + h * c3, -b * c5 + c * c4 - d * c3, n * s5 - o * s4 + p * s3, -j * s5 + k * s4 - l * s3,
            -e * c5 + g * c2 - h * c1, a * c5 - c * c2 + d * c1, -m * s5 + o * s2 - p * s1, i * s5 - k * s2 + l * s1,
            e * c4 - f * c2 + h * c0, -a * c4 + b * c2 - d * c0, m * s4 - n * s2 + p * s0, -i * s4 + j * s2 - l * s0,
            -e * c3 + f * c1 - g * c0, a * c3 - b * c1 + c * c0, -m * s3 + n * s1 - o * s0, i * s3 - j * s1 + k * s0,
        ]
        det = s0 * c5 - s1 * c4 + s2 * c3 + s3 * c2 - s4 * c1 + s5 * c0
        return adj, det

    def det4x4(self):
        return self._cofactors()[1]

    def adjoint(self):
        return type(self)(self._cofactors()[0])

    def inverse(self):
        adj, det = self._cofactors()
        if det == 0.0:
            return type(self)()
        return type(self)([x / det for x in adj])

    def homogenize(self):
        m = self._m
        w = m[15]
        return type(self)(m) if w == 0.0 else type(self)([x / w for x in m])

    def isEquivalent(self, o, tolerance=1e-10):
        return all(abs(a - b) <= tolerance for a, b in zip(self._m, o._m))

    def isSingular(self):
        return abs(self.det4x4()) < 1e-10

    def __str__(self):
        return "(%s)" % ", ".join("(%s)" % ", ".join(_num(x) for x in self._m[r * 4 : r * 4 + 4]) for r in range(4))

    def __repr__(self):
        return "aztek.api.OpenMaya.%s(%s)" % (type(self).__name__, str(self))


class MMatrix(_MatrixBase):
    __slots__ = ()
    kTolerance = 1e-10


MMatrix.kIdentity = MMatrix()


class MFloatMatrix(_MatrixBase):
    __slots__ = ()
    _conv = staticmethod(_f32)
    kTolerance = 1e-5


MFloatMatrix.kIdentity = MFloatMatrix()


# Rotation orders: index into these with MEulerRotation.kXYZ...; the axis applied first comes first.
_ORDERS = ("xyz", "yzx", "zxy", "xzy", "yxz", "zyx")


def _axis_matrix(axis, a):
    c, s = _math.cos(a), _math.sin(a)
    if axis == "x":
        return MMatrix([1, 0, 0, 0, 0, c, s, 0, 0, -s, c, 0, 0, 0, 0, 1])
    if axis == "y":
        return MMatrix([c, 0, -s, 0, 0, 1, 0, 0, s, 0, c, 0, 0, 0, 0, 1])
    return MMatrix([c, s, 0, 0, -s, c, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1])


class MEulerRotation:
    kXYZ = 0
    kYZX = 1
    kZXY = 2
    kXZY = 3
    kYXZ = 4
    kZYX = 5
    kTolerance = 1e-10
    __slots__ = ("x", "y", "z", "order")

    def __init__(self, *args, **kwargs):
        order = kwargs.get("order", 0)
        if not args:
            self.x = self.y = self.z = 0.0
        elif len(args) == 1 and isinstance(args[0], MEulerRotation):
            e = args[0]
            self.x, self.y, self.z = e.x, e.y, e.z
            order = e.order
        elif not isinstance(args[0], (int, float)):
            s = list(args[0])
            self.x, self.y, self.z = float(s[0]), float(s[1]), float(s[2])
            if len(args) > 1:
                order = args[1]
        else:
            self.x, self.y, self.z = float(args[0]), float(args[1]), float(args[2])
            if len(args) > 3:
                order = args[3]
        self.order = order

    def __len__(self):
        return 3

    def __getitem__(self, i):
        return (self.x, self.y, self.z)[i]

    def __setitem__(self, i, v):
        setattr(self, "xyz"[range(3)[i]], float(v))

    def __iter__(self):
        return iter((self.x, self.y, self.z))

    def __eq__(self, o):
        return isinstance(o, MEulerRotation) and (self.x, self.y, self.z, self.order) == (o.x, o.y, o.z, o.order)

    def __ne__(self, o):
        return not self == o

    __hash__ = None

    def __neg__(self):
        return MEulerRotation(-self.x, -self.y, -self.z, self.order)

    def __add__(self, o):
        return MEulerRotation(self.x + o.x, self.y + o.y, self.z + o.z, self.order)

    def __sub__(self, o):
        return MEulerRotation(self.x - o.x, self.y - o.y, self.z - o.z, self.order)

    def __mul__(self, o):
        if isinstance(o, (int, float)):
            return MEulerRotation(self.x * o, self.y * o, self.z * o, self.order)
        if isinstance(o, MEulerRotation):
            o = o.asQuaternion()
        if isinstance(o, MQuaternion):
            return (self.asQuaternion() * o).asEulerRotation().reorderIt(self.order)
        return NotImplemented

    def asMatrix(self):
        vals = {"x": self.x, "y": self.y, "z": self.z}
        a, b, c = _ORDERS[self.order]
        return _axis_matrix(a, vals[a]) * _axis_matrix(b, vals[b]) * _axis_matrix(c, vals[c])

    def asQuaternion(self):
        q = MQuaternion()
        vals = {"x": self.x, "y": self.y, "z": self.z}
        axes = {"x": MVector.kXaxisVector, "y": MVector.kYaxisVector, "z": MVector.kZaxisVector}
        for a in _ORDERS[self.order]:
            q = q * MQuaternion(vals[a], axes[a])
        return q

    def asVector(self):
        return MVector(self.x, self.y, self.z)

    def setValue(self, *args):
        e = MEulerRotation(*args)
        self.x, self.y, self.z, self.order = e.x, e.y, e.z, e.order
        return self

    def setToIdentity(self):
        self.x = self.y = self.z = 0.0
        return self

    def isEquivalent(self, o, tolerance=1e-10):
        return self.order == o.order and abs(self.x - o.x) <= tolerance and abs(self.y - o.y) <= tolerance and abs(self.z - o.z) <= tolerance

    def isZero(self, tolerance=1e-10):
        return abs(self.x) <= tolerance and abs(self.y) <= tolerance and abs(self.z) <= tolerance

    def inverse(self):
        rev = {0: 5, 1: 3, 2: 4, 3: 1, 4: 2, 5: 0}
        return MEulerRotation(-self.x, -self.y, -self.z, rev[self.order])

    def invertIt(self):
        e = self.inverse()
        self.x, self.y, self.z, self.order = e.x, e.y, e.z, e.order
        return self

    def reorder(self, order):
        return MEulerRotation.decompose(self.asMatrix(), order)

    def reorderIt(self, order):
        e = self.reorder(order)
        self.x, self.y, self.z, self.order = e.x, e.y, e.z, e.order
        return self

    def bound(self):
        def wrap(a):
            return (a + _math.pi) % (2 * _math.pi) - _math.pi

        return MEulerRotation(wrap(self.x), wrap(self.y), wrap(self.z), self.order)

    def boundIt(self):
        e = self.bound()
        self.x, self.y, self.z = e.x, e.y, e.z
        return self

    def alternateSolution(self):
        vals = {"x": self.x, "y": self.y, "z": self.z}
        a, b, c = _ORDERS[self.order]
        pi = _math.pi
        out = {a: vals[a] + pi, b: pi - vals[b], c: vals[c] + pi}
        return MEulerRotation(out["x"], out["y"], out["z"], self.order).bound()

    def setToAlternateSolution(self):
        e = self.alternateSolution()
        self.x, self.y, self.z = e.x, e.y, e.z
        return self

    def closestCut(self, dst):
        def near(a, b):
            return a + round((b - a) / (2 * _math.pi)) * 2 * _math.pi

        return MEulerRotation(near(self.x, dst.x), near(self.y, dst.y), near(self.z, dst.z), self.order)

    def setToClosestCut(self, dst):
        e = self.closestCut(dst)
        self.x, self.y, self.z = e.x, e.y, e.z
        return self

    def closestSolution(self, dst):
        a = self.closestCut(dst)
        b = self.alternateSolution().closestCut(dst)
        da = abs(a.x - dst.x) + abs(a.y - dst.y) + abs(a.z - dst.z)
        db = abs(b.x - dst.x) + abs(b.y - dst.y) + abs(b.z - dst.z)
        return a if da <= db else b

    def setToClosestSolution(self, dst):
        e = self.closestSolution(dst)
        self.x, self.y, self.z = e.x, e.y, e.z
        return self

    def incrementalRotateBy(self, axis, angle):
        q = self.asQuaternion() * MQuaternion(angle, axis)
        e = q.asEulerRotation().reorderIt(self.order).closestSolution(self)
        self.x, self.y, self.z = e.x, e.y, e.z
        return self

    @staticmethod
    def computeAlternateSolution(e):
        return e.alternateSolution()

    @staticmethod
    def computeBound(e):
        return e.bound()

    @staticmethod
    def computeClosestCut(a, b):
        return a.closestCut(b)

    @staticmethod
    def computeClosestSolution(a, b):
        return a.closestSolution(b)

    @staticmethod
    def decompose(matrix, order=0):
        """The rotation of a pure rotation matrix, in the order."""
        m = matrix
        names = _ORDERS[order]
        idx = {"x": 0, "y": 1, "z": 2}
        i, j, k = idx[names[0]], idx[names[1]], idx[names[2]]
        # Row-vector matrix: R = Ri * Rj * Rk with the axes applied in turn.
        even = (j - i) % 3 == 1
        sign = 1.0 if even else -1.0
        e = lambda r, c: m[r * 4 + c]
        cy = _math.sqrt(e(i, i) * e(i, i) + e(i, j) * e(i, j))
        if cy > 1e-9:
            ai = _math.atan2(sign * e(j, k), e(k, k))
            aj = _math.atan2(-sign * e(i, k), cy)
            ak = _math.atan2(sign * e(i, j), e(i, i))
        else:
            ai = _math.atan2(-sign * e(k, j), e(j, j))
            aj = _math.atan2(-sign * e(i, k), cy)
            ak = 0.0
        out = [0.0, 0.0, 0.0]
        out[i], out[j], out[k] = ai, aj, ak
        return MEulerRotation(out[0], out[1], out[2], order)

    def __str__(self):
        return "(%s, %s, %s, %s)" % (_num(self.x), _num(self.y), _num(self.z), ("kXYZ", "kYZX", "kZXY", "kXZY", "kYXZ", "kZYX")[self.order])

    def __repr__(self):
        return "aztek.api.OpenMaya.MEulerRotation" + str(self)


MEulerRotation.kIdentity = MEulerRotation()


class MQuaternion:
    kTolerance = 1e-10
    __slots__ = ("x", "y", "z", "w")

    def __init__(self, *args):
        if not args:
            self.x = self.y = self.z = 0.0
            self.w = 1.0
        elif len(args) == 1:
            a = args[0]
            s = list(a)
            if len(s) != 4:
                _fail(message="Expected 4 numbers")
            self.x, self.y, self.z, self.w = float(s[0]), float(s[1]), float(s[2]), float(s[3])
        elif len(args) == 2 and isinstance(args[0], (int, float)):
            self._set_axis_angle(args[1], args[0])
        elif len(args) in (2, 3) and not isinstance(args[0], (int, float)):
            self._set_between(args[0], args[1], args[2] if len(args) == 3 else 1.0)
        elif len(args) == 4:
            self.x, self.y, self.z, self.w = float(args[0]), float(args[1]), float(args[2]), float(args[3])
        else:
            _fail(message="Bad arguments")

    def _set_axis_angle(self, axis, angle):
        n = _length(axis)
        s = _math.sin(angle * 0.5) / n if n != 0.0 else 0.0
        self.x, self.y, self.z, self.w = axis[0] * s, axis[1] * s, axis[2] * s, _math.cos(angle * 0.5)

    def _set_between(self, a, b, factor):
        a, b = MVector(a).normal(), MVector(b).normal()
        d = max(-1.0, min(1.0, a * b))
        if d > 1.0 - 1e-12:
            self.x = self.y = self.z = 0.0
            self.w = 1.0
            return
        if d < -1.0 + 1e-12:
            axis = MVector(1, 0, 0) ^ a
            if axis.length() < 1e-6:
                axis = MVector(0, 1, 0) ^ a
            self._set_axis_angle(axis, _math.pi * factor)
            return
        self._set_axis_angle(a ^ b, _math.acos(d) * factor)

    def __len__(self):
        return 4

    def __getitem__(self, i):
        return (self.x, self.y, self.z, self.w)[i]

    def __setitem__(self, i, v):
        setattr(self, "xyzw"[range(4)[i]], float(v))

    def __iter__(self):
        return iter((self.x, self.y, self.z, self.w))

    def __eq__(self, o):
        return isinstance(o, MQuaternion) and tuple(self) == tuple(o)

    def __ne__(self, o):
        return not self == o

    __hash__ = None

    def __add__(self, o):
        return MQuaternion(self.x + o.x, self.y + o.y, self.z + o.z, self.w + o.w)

    def __sub__(self, o):
        return MQuaternion(self.x - o.x, self.y - o.y, self.z - o.z, self.w - o.w)

    def __neg__(self):
        return MQuaternion(-self.x, -self.y, -self.z, -self.w)

    def __mul__(self, o):
        if isinstance(o, (int, float)):
            return MQuaternion(self.x * o, self.y * o, self.z * o, self.w * o)
        if not isinstance(o, MQuaternion):
            return NotImplemented
        # a * b applies a first, as the matrix product does.
        ax, ay, az, aw = o.x, o.y, o.z, o.w
        bx, by, bz, bw = self.x, self.y, self.z, self.w
        return MQuaternion(
            aw * bx + ax * bw + ay * bz - az * by,
            aw * by - ax * bz + ay * bw + az * bx,
            aw * bz + ax * by - ay * bx + az * bw,
            aw * bw - ax * bx - ay * by - az * bz,
        )

    def __rmul__(self, o):
        return self * o

    def __imul__(self, o):
        r = self * o
        self.x, self.y, self.z, self.w = r.x, r.y, r.z, r.w
        return self

    def asMatrix(self):
        n = self.x * self.x + self.y * self.y + self.z * self.z + self.w * self.w
        s = 2.0 / n if n else 0.0
        x, y, z, w = self.x, self.y, self.z, self.w
        xx, yy, zz = x * x * s, y * y * s, z * z * s
        xy, xz, yz = x * y * s, x * z * s, y * z * s
        wx, wy, wz = w * x * s, w * y * s, w * z * s
        return MMatrix([1 - yy - zz, xy + wz, xz - wy, 0, xy - wz, 1 - xx - zz, yz + wx, 0, xz + wy, yz - wx, 1 - xx - yy, 0, 0, 0, 0, 1])

    def asEulerRotation(self):
        return MEulerRotation.decompose(self.asMatrix(), MEulerRotation.kXYZ)

    def asAxisAngle(self):
        w = max(-1.0, min(1.0, self.w))
        angle = 2.0 * _math.acos(w)
        s = _math.sqrt(max(0.0, 1.0 - w * w))
        if s < 1e-12:
            return (MVector(0, 0, 1), 0.0) if angle < 1e-9 else (MVector(1, 0, 0), angle)
        return (MVector(self.x / s, self.y / s, self.z / s), angle)

    def setValue(self, *args):
        q = MQuaternion(*args)
        self.x, self.y, self.z, self.w = q.x, q.y, q.z, q.w
        return self

    def setToXAxis(self, angle):
        return self.setValue(angle, MVector.kXaxisVector)

    def setToYAxis(self, angle):
        return self.setValue(angle, MVector.kYaxisVector)

    def setToZAxis(self, angle):
        return self.setValue(angle, MVector.kZaxisVector)

    def conjugate(self):
        return MQuaternion(-self.x, -self.y, -self.z, self.w)

    def conjugateIt(self):
        self.x, self.y, self.z = -self.x, -self.y, -self.z
        return self

    def inverse(self):
        n = self.x * self.x + self.y * self.y + self.z * self.z + self.w * self.w
        if n == 0.0:
            _fail(message="Quaternion has no inverse")
        return MQuaternion(-self.x / n, -self.y / n, -self.z / n, self.w / n)

    def invertIt(self):
        q = self.inverse()
        self.x, self.y, self.z, self.w = q.x, q.y, q.z, q.w
        return self

    def negateIt(self):
        self.x, self.y, self.z, self.w = -self.x, -self.y, -self.z, -self.w
        return self

    def normal(self):
        n = _math.sqrt(self.x * self.x + self.y * self.y + self.z * self.z + self.w * self.w)
        return MQuaternion(self) if n == 0.0 else MQuaternion(self.x / n, self.y / n, self.z / n, self.w / n)

    def normalizeIt(self):
        q = self.normal()
        self.x, self.y, self.z, self.w = q.x, q.y, q.z, q.w
        return self

    def log(self):
        v = _math.sqrt(self.x * self.x + self.y * self.y + self.z * self.z)
        if v < 1e-12:
            return MQuaternion(0, 0, 0, 0)
        a = _math.atan2(v, self.w)
        return MQuaternion(self.x / v * a, self.y / v * a, self.z / v * a, 0.0)

    def exp(self):
        a = _math.sqrt(self.x * self.x + self.y * self.y + self.z * self.z)
        if a < 1e-12:
            return MQuaternion()
        s = _math.sin(a) / a
        return MQuaternion(self.x * s, self.y * s, self.z * s, _math.cos(a))

    def isEquivalent(self, o, tolerance=1e-10):
        return all(abs(a - b) <= tolerance for a, b in zip(self, o))

    @staticmethod
    def slerp(p, q, t, spin=0):
        d = p.x * q.x + p.y * q.y + p.z * q.z + p.w * q.w
        flip = d < 0.0
        if flip and spin == 0:
            d = -d
        elif flip:
            flip = False
        if d > 1.0 - 1e-10:
            a, b = 1.0 - t, t
        else:
            om = _math.acos(max(-1.0, min(1.0, d)))
            phase = om + spin * _math.pi
            a, b = _math.sin(om - t * phase) / _math.sin(om), _math.sin(t * phase) / _math.sin(om)
        if flip:
            b = -b
        return MQuaternion(a * p.x + b * q.x, a * p.y + b * q.y, a * p.z + b * q.z, a * p.w + b * q.w)

    @staticmethod
    def squad(p, a, b, q, t, spin=0):
        return MQuaternion.slerp(MQuaternion.slerp(p, q, t, spin), MQuaternion.slerp(a, b, t), 2.0 * t * (1.0 - t))

    def __str__(self):
        return "(%s, %s, %s, %s)" % (_num(self.x), _num(self.y), _num(self.z), _num(self.w))

    def __repr__(self):
        return "aztek.api.OpenMaya.MQuaternion" + str(self)


MQuaternion.kIdentity = MQuaternion()


class MTransformationMatrix:
    kInvalid = 0
    kXYZ = 1
    kYZX = 2
    kZXY = 3
    kXZY = 4
    kYXZ = 5
    kZYX = 6
    kLast = 7
    kTolerance = 1e-10

    def __init__(self, src=None):
        self._t = MVector()
        self._q = MQuaternion()
        self._order = 0
        self._euler = None
        self._s = [1.0, 1.0, 1.0]
        self._sh = [0.0, 0.0, 0.0]
        self._rp = MVector()
        self._rpt = MVector()
        self._sp = MVector()
        self._spt = MVector()
        self._ro = MQuaternion()
        if isinstance(src, MTransformationMatrix):
            self.__dict__.update({k: (type(v)(v) if v is not None and k[0] == "_" and not isinstance(v, int) else v) for k, v in src.__dict__.items()})
        elif src is not None:
            self._from_matrix(MMatrix(src))

    def _from_matrix(self, m):
        self._t = MVector(m[12], m[13], m[14])
        r0, r1, r2 = MVector(m[0], m[1], m[2]), MVector(m[4], m[5], m[6]), MVector(m[8], m[9], m[10])
        sx = r0.length()
        r0n = r0 / sx if sx else r0
        xy = r0n * r1
        r1o = r1 - r0n * xy
        sy = r1o.length()
        r1n = r1o / sy if sy else r1o
        xz = r0n * r2
        yz = r1n * r2
        r2o = r2 - r0n * xz - r1n * yz
        sz = r2o.length()
        r2n = r2o / sz if sz else r2o
        if (r0n ^ r1n) * r2n < 0.0:
            sx = -sx
            xy, xz = -xy, -xz
            r0n = -r0n
        self._s = [sx, sy, sz]
        self._sh = [xy / sy if sy else 0.0, xz / sz if sz else 0.0, yz / sz if sz else 0.0]
        rot = MMatrix([r0n.x, r0n.y, r0n.z, 0, r1n.x, r1n.y, r1n.z, 0, r2n.x, r2n.y, r2n.z, 0, 0, 0, 0, 1])
        self._euler = MEulerRotation.decompose(rot, self._order)
        self._q = self._euler.asQuaternion()

    def _scale_matrix(self):
        sx, sy, sz = self._s
        return MMatrix([sx, 0, 0, 0, 0, sy, 0, 0, 0, 0, sz, 0, 0, 0, 0, 1])

    def _shear_matrix(self):
        xy, xz, yz = self._sh
        return MMatrix([1, 0, 0, 0, xy, 1, 0, 0, xz, yz, 1, 0, 0, 0, 0, 1])

    def _rot_matrix(self):
        return self._q.asMatrix()

    def asMatrix(self, percent=1.0):
        def tr(v, sign=1.0):
            return MMatrix([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, v[0] * sign, v[1] * sign, v[2] * sign, 1])

        if percent != 1.0:
            o = MTransformationMatrix(self)
            o._t = self._t * percent
            o._q = MQuaternion.slerp(MQuaternion(), self._q, percent)
            o._euler = None
            o._s = [1.0 + (s - 1.0) * percent for s in self._s]
            o._sh = [s * percent for s in self._sh]
            return o.asMatrix()
        m = tr(self._sp, -1.0) * self._scale_matrix() * self._shear_matrix() * tr(self._sp) * tr(self._spt)
        m = m * tr(self._rp, -1.0) * self._ro.asMatrix() * self._rot_matrix() * tr(self._rp) * tr(self._rpt) * tr(self._t)
        return m

    def asMatrixInverse(self):
        return self.asMatrix().inverse()

    def asScaleMatrix(self):
        return self._scale_matrix() * self._shear_matrix()

    def asRotateMatrix(self):
        return self._ro.asMatrix() * self._rot_matrix()

    def translation(self, space=MSpace.kTransform):
        return MVector(self._t)

    def setTranslation(self, vector, space=MSpace.kTransform):
        self._t = MVector(vector)
        return self

    def translateBy(self, vector, space=MSpace.kTransform):
        v = MVector(vector)
        if space == MSpace.kPreTransform:
            v = v * self._rot_matrix()
        self._t = self._t + v
        return self

    def rotation(self, asQuaternion=False):
        if asQuaternion:
            return MQuaternion(self._q)
        if self._euler is not None:
            return MEulerRotation(self._euler)
        return MEulerRotation.decompose(self._q.asMatrix(), self._order)

    def setRotation(self, rot, order=None):
        if isinstance(rot, MEulerRotation):
            self._euler = MEulerRotation(rot)
            self._order = rot.order
            self._q = rot.asQuaternion()
        elif isinstance(rot, MQuaternion):
            self._q = MQuaternion(rot)
            self._euler = None
        else:
            _fail()
        return self

    def rotateBy(self, rot, space=MSpace.kTransform):
        q = rot.asQuaternion() if isinstance(rot, MEulerRotation) else rot
        self._q = q * self._q if space in (MSpace.kObject, MSpace.kPreTransform) else self._q * q
        self._euler = None
        return self

    def rotationOrder(self):
        return self._order + 1

    def reorderRotation(self, order):
        self._order = order - 1
        self._euler = None
        return self

    def rotationComponents(self, asQuaternion=False):
        if asQuaternion:
            return list(self._q)
        return list(self.rotation())

    def setRotationComponents(self, seq, asQuaternion=False):
        if asQuaternion:
            return self.setRotation(MQuaternion(seq))
        return self.setRotation(MEulerRotation(seq[0], seq[1], seq[2], self._order))

    def rotationOrientation(self):
        return MQuaternion(self._ro)

    def setRotationOrientation(self, q):
        self._ro = MQuaternion(q)
        return self

    def scale(self, space=MSpace.kTransform):
        return list(self._s)

    def setScale(self, seq, space=MSpace.kTransform):
        self._s = [float(seq[0]), float(seq[1]), float(seq[2])]
        return self

    def scaleBy(self, seq, space=MSpace.kTransform):
        self._s = [self._s[i] * seq[i] for i in range(3)]
        return self

    def shear(self, space=MSpace.kTransform):
        return list(self._sh)

    def setShear(self, seq, space=MSpace.kTransform):
        self._sh = [float(seq[0]), float(seq[1]), float(seq[2])]
        return self

    def shearBy(self, seq, space=MSpace.kTransform):
        self._sh = [self._sh[i] * seq[i] for i in range(3)]
        return self

    def rotatePivot(self, space=MSpace.kTransform):
        return MPoint(self._rp)

    def setRotatePivot(self, point, space=MSpace.kTransform, balance=True):
        p = MVector(point[0], point[1], point[2])
        if balance:
            # The rotated offset is taken up so the matrix doesn't change.
            before = self.asMatrix()
            self._rp = p
            after = self.asMatrix()
            self._rpt = self._rpt + MVector(before[12] - after[12], before[13] - after[13], before[14] - after[14])
        else:
            self._rp = p
        return self

    def rotatePivotTranslation(self, space=MSpace.kTransform):
        return MVector(self._rpt)

    def setRotatePivotTranslation(self, v, space=MSpace.kTransform):
        self._rpt = MVector(v)
        return self

    def scalePivot(self, space=MSpace.kTransform):
        return MPoint(self._sp)

    def setScalePivot(self, point, space=MSpace.kTransform, balance=True):
        p = MVector(point[0], point[1], point[2])
        if balance:
            before = self.asMatrix()
            self._sp = p
            after = self.asMatrix()
            self._spt = self._spt + MVector(before[12] - after[12], before[13] - after[13], before[14] - after[14])
        else:
            self._sp = p
        return self

    def scalePivotTranslation(self, space=MSpace.kTransform):
        return MVector(self._spt)

    def setScalePivotTranslation(self, v, space=MSpace.kTransform):
        self._spt = MVector(v)
        return self

    def isEquivalent(self, o, tolerance=1e-10):
        return self.asMatrix().isEquivalent(o.asMatrix(), tolerance)

    def __eq__(self, o):
        return isinstance(o, MTransformationMatrix) and self.asMatrix() == o.asMatrix()

    def __ne__(self, o):
        return not self == o

    __hash__ = None


MTransformationMatrix.kIdentity = MTransformationMatrix()


class MAngle:
    kInvalid = 0
    kRadians = 1
    kDegrees = 2
    kAngMinutes = 3
    kAngSeconds = 4
    kLast = 5
    _ui = 2
    _to_rad = {1: 1.0, 2: _math.pi / 180.0, 3: _math.pi / 10800.0, 4: _math.pi / 648000.0}

    def __init__(self, value=0.0, unit=1):
        if isinstance(value, MAngle):
            self.value, self.unit = value.value, value.unit
        else:
            self.value, self.unit = float(value), unit

    def asUnits(self, unit):
        return self.value * self._to_rad[self.unit] / self._to_rad[unit]

    def asRadians(self):
        return self.asUnits(1)

    def asDegrees(self):
        return self.asUnits(2)

    def asAngMinutes(self):
        return self.asUnits(3)

    def asAngSeconds(self):
        return self.asUnits(4)

    @staticmethod
    def uiUnit():
        import _aztek

        name = _json_loads(_aztek.call("currentUnit", "[]", '{"query": true, "queried": ["angle"]}'))
        return {"deg": MAngle.kDegrees, "rad": MAngle.kRadians, "min": MAngle.kAngMinutes, "sec": MAngle.kAngSeconds}.get(name, MAngle.kDegrees)

    @staticmethod
    def setUIUnit(unit):
        import _aztek

        name = {MAngle.kDegrees: "deg", MAngle.kRadians: "rad", MAngle.kAngMinutes: "min", MAngle.kAngSeconds: "sec"}[unit]
        _aztek.call("currentUnit", "[]", '{"angle": "%s"}' % name)

    @staticmethod
    def internalUnit():
        return MAngle.kRadians

    @staticmethod
    def internalToUI(v):
        return v / MAngle._to_rad[MAngle.uiUnit()]

    @staticmethod
    def uiToInternal(v):
        return v * MAngle._to_rad[MAngle.uiUnit()]

    def __repr__(self):
        return "aztek.api.OpenMaya.MAngle(%s, %d)" % (_num(self.value), self.unit)


class MDistance:
    kInvalid = 0
    kInches = 1
    kFeet = 2
    kYards = 3
    kMiles = 4
    kMillimeters = 5
    kCentimeters = 6
    _UNIT_NAMES = {"in": 1, "ft": 2, "yd": 3, "mi": 4, "mm": 5, "cm": 6, "km": 7, "m": 8}
    kKilometers = 7
    kMeters = 8
    kLast = 9
    _ui = 6
    _to_cm = {1: 2.54, 2: 30.48, 3: 91.44, 4: 160934.4, 5: 0.1, 6: 1.0, 7: 100000.0, 8: 100.0}

    def __init__(self, value=0.0, unit=6):
        if isinstance(value, MDistance):
            self.value, self.unit = value.value, value.unit
        else:
            self.value, self.unit = float(value), unit

    def asUnits(self, unit):
        return self.value * self._to_cm[self.unit] / self._to_cm[unit]

    def asInches(self):
        return self.asUnits(1)

    def asFeet(self):
        return self.asUnits(2)

    def asYards(self):
        return self.asUnits(3)

    def asMiles(self):
        return self.asUnits(4)

    def asMillimeters(self):
        return self.asUnits(5)

    def asCentimeters(self):
        return self.asUnits(6)

    def asKilometers(self):
        return self.asUnits(7)

    def asMeters(self):
        return self.asUnits(8)

    @staticmethod
    def uiUnit():
        import _aztek

        name = _json_loads(_aztek.call("currentUnit", "[]", '{"query": true, "queried": ["linear"]}'))
        return MDistance._UNIT_NAMES.get(name, MDistance.kCentimeters)

    @staticmethod
    def setUIUnit(unit):
        import _aztek

        name = next(n for n, u in MDistance._UNIT_NAMES.items() if u == unit)
        _aztek.call("currentUnit", "[]", '{"linear": "%s"}' % name)

    @staticmethod
    def internalUnit():
        return MDistance.kCentimeters

    @staticmethod
    def internalToUI(v):
        return v / MDistance._to_cm[MDistance.uiUnit()]

    @staticmethod
    def uiToInternal(v):
        return v * MDistance._to_cm[MDistance.uiUnit()]

    def __repr__(self):
        return "aztek.api.OpenMaya.MDistance(%s, %d)" % (_num(self.value), self.unit)


class MTime:
    kInvalid = 0
    kHours = 1
    kMinutes = 2
    kSeconds = 3
    kMilliseconds = 4
    k15FPS = 5
    kGames = 5
    k24FPS = 6
    kFilm = 6
    k25FPS = 7
    kPALFrame = 7
    k30FPS = 8
    kNTSCFrame = 8
    k48FPS = 9
    kShowScan = 9
    k50FPS = 10
    kPALField = 10
    k60FPS = 11
    kNTSCField = 11
    k2FPS = 12
    k3FPS = 13
    k4FPS = 14
    k5FPS = 15
    k6FPS = 16
    k8FPS = 17
    k10FPS = 18
    k12FPS = 19
    k16FPS = 20
    k20FPS = 21
    k40FPS = 22
    k75FPS = 23
    k80FPS = 24
    k100FPS = 25
    k120FPS = 26
    k125FPS = 27
    k150FPS = 28
    k200FPS = 29
    k240FPS = 30
    k250FPS = 31
    k300FPS = 32
    k375FPS = 33
    k400FPS = 34
    k500FPS = 35
    k600FPS = 36
    k750FPS = 37
    k1200FPS = 38
    k1500FPS = 39
    k2000FPS = 40
    k3000FPS = 41
    k6000FPS = 42
    kLast = 43
    _ui = None
    _seconds = {1: 3600.0, 2: 60.0, 3: 1.0, 4: 0.001}
    _fps = {
        5: 15, 6: 24, 7: 25, 8: 30, 9: 48, 10: 50, 11: 60, 12: 2, 13: 3, 14: 4, 15: 5, 16: 6, 17: 8, 18: 10, 19: 12, 20: 16,
        21: 20, 22: 40, 23: 75, 24: 80, 25: 100, 26: 120, 27: 125, 28: 150, 29: 200, 30: 240, 31: 250, 32: 300, 33: 375,
        34: 400, 35: 500, 36: 600, 37: 750, 38: 1200, 39: 1500, 40: 2000, 41: 3000, 42: 6000,
    }

    def __init__(self, value=0.0, unit=None):
        if isinstance(value, MTime):
            self.value, self.unit = value.value, value.unit
        else:
            self.value = float(value)
            self.unit = MTime.uiUnit() if unit is None or unit == 0 else unit

    @classmethod
    def _unit_seconds(cls, unit):
        if unit in cls._seconds:
            return cls._seconds[unit]
        return 1.0 / cls._fps[unit]

    def asUnits(self, unit):
        return self.value * self._unit_seconds(self.unit) / self._unit_seconds(unit)

    def _in(self, o):
        return o.asUnits(self.unit) if isinstance(o, MTime) else float(o)

    def __add__(self, o):
        return MTime(self.value + self._in(o), self.unit)

    __radd__ = __add__

    def __sub__(self, o):
        return MTime(self.value - self._in(o), self.unit)

    def __rsub__(self, o):
        return MTime(self._in(o) - self.value, self.unit)

    def __mul__(self, o):
        return MTime(self.value * o, self.unit)

    __rmul__ = __mul__

    def __truediv__(self, o):
        if isinstance(o, MTime):
            return self.value / o.asUnits(self.unit)
        return MTime(self.value / o, self.unit)

    def __neg__(self):
        return MTime(-self.value, self.unit)

    def __eq__(self, o):
        return isinstance(o, MTime) and abs(self.value - o.asUnits(self.unit)) < 1e-9

    def __ne__(self, o):
        return not self == o

    def __lt__(self, o):
        return self.value < self._in(o) - 1e-9

    def __le__(self, o):
        return self.value <= self._in(o) + 1e-9

    def __gt__(self, o):
        return self.value > self._in(o) + 1e-9

    def __ge__(self, o):
        return self.value >= self._in(o) - 1e-9

    __hash__ = None

    @staticmethod
    def uiUnit():
        if MTime._ui is None:
            MTime._ui = MTime.kFilm
        return MTime._ui

    @staticmethod
    def setUIUnit(unit):
        MTime._ui = unit

    @staticmethod
    def ticksPerSecond():
        return 141120000

    def __repr__(self):
        return "aztek.api.OpenMaya.MTime(%s, %d)" % (_num(self.value), self.unit)


class MPlane:
    def __init__(self, src=None):
        self._n = MVector(0, 0, 1)
        self._d = 0.0
        if src is not None:
            self._n, self._d = MVector(src._n), src._d

    def setPlane(self, *args):
        if len(args) == 2:
            self._n, self._d = MVector(args[0]), float(args[1])
        elif len(args) == 4:
            self._n, self._d = MVector(args[0], args[1], args[2]), float(args[3])
        else:
            _fail()
        return self

    def normal(self):
        return MVector(self._n)

    def distance(self):
        return self._d

    def relativeDistance(self, p):
        return self._n.x * p[0] + self._n.y * p[1] + self._n.z * p[2] + self._d

    def directedDistance(self, p):
        return self.relativeDistance(p) / self._n.length()


class MBoundingBox:
    def __init__(self, a=None, b=None):
        self._min = None
        self._max = None
        if isinstance(a, MBoundingBox):
            self._min, self._max = (a._min and MPoint(a._min)), (a._max and MPoint(a._max))
        elif a is not None:
            self.expand(MPoint(a))
            if b is not None:
                self.expand(MPoint(b))

    @property
    def min(self):
        return MPoint(self._min) if self._min else MPoint()

    @property
    def max(self):
        return MPoint(self._max) if self._max else MPoint()

    @property
    def center(self):
        a, b = self.min, self.max
        return MPoint((a.x + b.x) / 2, (a.y + b.y) / 2, (a.z + b.z) / 2)

    @property
    def width(self):
        return self.max.x - self.min.x

    @property
    def height(self):
        return self.max.y - self.min.y

    @property
    def depth(self):
        return self.max.z - self.min.z

    def clear(self):
        self._min = self._max = None
        return self

    def expand(self, o):
        if isinstance(o, MBoundingBox):
            if o._min is not None:
                self.expand(o._min)
                self.expand(o._max)
            return self
        p = MPoint(o)
        p.cartesianize()
        if self._min is None:
            self._min, self._max = MPoint(p), MPoint(p)
        else:
            a, b = self._min, self._max
            a.x, a.y, a.z = min(a.x, p.x), min(a.y, p.y), min(a.z, p.z)
            b.x, b.y, b.z = max(b.x, p.x), max(b.y, p.y), max(b.z, p.z)
        return self

    def contains(self, p):
        if self._min is None:
            return False
        a, b = self._min, self._max
        return a.x <= p[0] <= b.x and a.y <= p[1] <= b.y and a.z <= p[2] <= b.z

    def intersects(self, o, tolerance=0.0):
        if self._min is None or o._min is None:
            return False
        a, b, c, d = self._min, self._max, o._min, o._max
        return (
            a.x <= d.x + tolerance and c.x <= b.x + tolerance and a.y <= d.y + tolerance
            and c.y <= b.y + tolerance and a.z <= d.z + tolerance and c.z <= b.z + tolerance
        )

    def transformUsing(self, m):
        if self._min is None:
            return self
        a, b = self._min, self._max
        corners = [MPoint(x, y, z) * m for x in (a.x, b.x) for y in (a.y, b.y) for z in (a.z, b.z)]
        self.clear()
        for c in corners:
            self.expand(c)
        return self

    def __repr__(self):
        return "aztek.api.OpenMaya.MBoundingBox(%s, %s)" % (self.min, self.max)


class _Array(list):
    """A Python list that keeps its items in one type. remove takes an index, as the reference's arrays do."""

    _item = staticmethod(lambda x: x)
    _blank = staticmethod(lambda: 0)

    def __init__(self, *args):
        list.__init__(self)
        if not args:
            return
        if len(args) == 2:
            list.extend(self, [self._item(args[1]) for _ in range(args[0])])
        elif isinstance(args[0], int):
            list.extend(self, [self._blank() for _ in range(args[0])])
        else:
            list.extend(self, [self._item(x) for x in args[0]])

    def __setitem__(self, i, v):
        if isinstance(i, slice):
            list.__setitem__(self, i, [self._item(x) for x in v])
        else:
            list.__setitem__(self, i, self._item(v))

    def __getitem__(self, i):
        r = list.__getitem__(self, i)
        return type(self)(r) if isinstance(i, slice) else r

    def append(self, v):
        list.append(self, self._item(v))

    def insert(self, *args):
        # The reference's order is (value, index); Python's is (index, value).
        if len(args) == 2 and isinstance(args[1], int) and not isinstance(args[0], int):
            v, i = args
        elif len(args) == 2 and isinstance(args[0], int) and not isinstance(args[1], int):
            i, v = args
        else:
            v, i = args
        list.insert(self, i, self._item(v))

    def extend(self, seq):
        list.extend(self, [self._item(x) for x in seq])

    def remove(self, i):
        del self[i]

    def copy(self, src=None):
        if src is None:
            return type(self)(self)
        self[:] = list(src)
        return self

    def setLength(self, n):
        have = len(self)
        if n < have:
            del self[n:]
        else:
            list.extend(self, [self._blank() for _ in range(n - have)])
        return self

    def __str__(self):
        return "[%s]" % ", ".join(str(x) for x in self)

    def __repr__(self):
        return "aztek.api.OpenMaya.%s([%s])" % (type(self).__name__, ", ".join(repr(x) for x in self))

    @property
    def sizeIncrement(self):
        return 0

    @sizeIncrement.setter
    def sizeIncrement(self, v):
        pass


class MIntArray(_Array):
    _item = staticmethod(int)
    _blank = staticmethod(lambda: 0)


class MFloatArray(_Array):
    _item = staticmethod(_f32)
    _blank = staticmethod(lambda: 0.0)


class MDoubleArray(_Array):
    _item = staticmethod(float)
    _blank = staticmethod(lambda: 0.0)


class MStringArray(_Array):
    _item = staticmethod(str)
    _blank = staticmethod(str)


class MPointArray(_Array):
    _item = staticmethod(lambda x: MPoint(x))
    _blank = staticmethod(MPoint)


class MFloatPointArray(_Array):
    _item = staticmethod(lambda x: MFloatPoint(x))
    _blank = staticmethod(MFloatPoint)


class MVectorArray(_Array):
    _item = staticmethod(lambda x: MVector(x))
    _blank = staticmethod(MVector)


class MFloatVectorArray(_Array):
    _item = staticmethod(lambda x: MFloatVector(x))
    _blank = staticmethod(MFloatVector)


class MMatrixArray(_Array):
    _item = staticmethod(lambda x: MMatrix(x))
    _blank = staticmethod(MMatrix)


class MColorArray(_Array):
    _item = staticmethod(lambda x: MColor(x))
    _blank = staticmethod(MColor)


class MTimeArray(_Array):
    _item = staticmethod(lambda x: MTime(x))
    _blank = staticmethod(MTime)
