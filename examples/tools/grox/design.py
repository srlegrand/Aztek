"""Grox, a horned brute: the skeleton and the sculpt as signed distance primitives. Centimetres, Y up, facing +Z."""
import numpy as np

# ---------------------------------------------------------------------------------------------- skeleton
# name: (parent, position). Left side only for paired joints; the right side mirrors in X.
J = {}
def j(name, parent, p):
    J[name] = (parent, np.array(p, float))

j("C_root", None, (0, 106, -4))
j("C_spine01", "C_root", (0, 122, -5))
j("C_spine02", "C_spine01", (0, 140, -5))
j("C_spine03", "C_spine02", (0, 158, -3))
j("C_chest", "C_spine03", (0, 176, 0))
j("C_neck01", "C_chest", (0, 197, 6))
j("C_neck02", "C_neck01", (0, 208, 13))
j("C_head", "C_neck02", (0, 219, 20))
j("C_headEnd", "C_head", (0, 262, 26))
j("C_jaw", "C_head", (0, 213, 25))
j("C_jawEnd", "C_jaw", (0, 199, 54))
j("L_eye", "C_head", (9.5, 228.5, 41))
j("L_lidUpper", "C_head", (9.5, 228.5, 41))
j("L_lidLower", "C_head", (9.5, 228.5, 41))
# Ram horns: a curl out, up, back and down, five joints.
def horn_path(t, side=1):
    a = 0.05 * np.pi + t * 1.85 * np.pi
    R = 17 * (1 - 0.55 * t)
    c = np.array([side * (16 + 22 * t), 236, 6])
    return c + np.array([0, R * np.sin(a), R * np.cos(a)])
for i, t in enumerate(np.linspace(0, 1, 6)):
    j("L_horn%02d" % (i + 1) if i < 5 else "L_hornEnd", "C_head" if i == 0 else ("L_horn%02d" % i), horn_path(t))
j("L_brow", "C_head", (11, 235, 38))
j("L_clavicle", "C_chest", (7, 190, 8))
j("L_shoulder", "L_clavicle", (40, 192, 0))
j("L_elbow", "L_shoulder", (72, 150, -9))
j("L_wrist", "L_elbow", (98, 104, 4))
j("L_palm", "L_wrist", (104, 92, 9))
for k, (dx, dz) in enumerate([(0, 17), (6, 10), (10, 2)]):
    f = "L_claw%s" % "ABC"[k]
    base = np.array([104 + dx, 88, 9 + dz])
    d = np.array([0.18, -1, 0.35]); d /= np.linalg.norm(d)
    j(f + "01", "L_palm", base)
    j(f + "02", f + "01", base + d * 13)
    j(f + "03", f + "02", base + d * 23)
    j(f + "End", f + "03", base + d * 31)
j("L_thumb01", "L_palm", (99, 94, 17))
j("L_thumb02", "L_thumb01", (95, 84, 26))
j("L_thumbEnd", "L_thumb02", (92, 76, 33))
j("L_hip", "C_root", (19, 102, -2))
j("L_knee", "L_hip", (25, 60, 13))
j("L_ankle", "L_knee", (27, 18, -6))
j("L_ball", "L_ankle", (28, 5, 15))
j("L_toe", "L_ball", (28, 3, 31))
for k, dx in enumerate([-8, 0, 8]):
    t = "L_toe%s" % "ABC"[k]
    j(t + "01", "L_ball", (28 + dx, 5, 17))
    j(t + "End", t + "01", (28 + dx * 1.4, 2.5, 33))
# Tail
tail = [(0, 103, -24), (0, 99, -42), (0, 91, -60), (0, 80, -77), (0, 66, -93), (0, 52, -107), (0, 39, -120), (0, 28, -131), (0, 20, -141)]
for i, p in enumerate(tail):
    n = "C_tail%02d" % (i + 1) if i < len(tail) - 1 else "C_tailEnd"
    j(n, "C_root" if i == 0 else "C_tail%02d" % i, p)

def tail_curve(n):
    """Catmull-Rom through the tail joints."""
    c = [P("C_tail%02d" % i) for i in range(1, 9)] + [P("C_tailEnd")]
    c = [c[0] * 2 - c[1]] + c + [c[-1] * 2 - c[-2]]
    out = []
    for t in np.linspace(0, len(c) - 3, n):
        i = min(int(t), len(c) - 4); u = t - i
        p0, p1, p2, p3 = c[i:i + 4]
        out.append(0.5 * ((2 * p1) + (-p0 + p2) * u + (2 * p0 - 5 * p1 + 4 * p2 - p3) * u * u + (-p0 + 3 * p1 - 3 * p2 + p3) * u ** 3))
    return out

def mirror_name(n):
    return "R_" + n[2:] if n.startswith("L_") else n

def full_skeleton():
    out = {}
    for n, (par, p) in J.items():
        out[n] = (par, p)
        if n.startswith("L_"):
            q = p.copy(); q[0] = -q[0]
            out[mirror_name(n)] = (mirror_name(par) if par else None, q)
    return out

def P(n):
    if n.startswith("R_"):
        q = J["L_" + n[2:]][1].copy(); q[0] = -q[0]; return q
    return J[n][1]

# ---------------------------------------------------------------------------------------------- sculpt
def round_cone(p, a, b, r1, r2):
    """Distance to a capsule from a (radius r1) to b (radius r2)."""
    ba = b - a; l2 = ba @ ba; rr = r1 - r2; a2 = l2 - rr * rr; il2 = 1.0 / l2
    pa = p - a; y = pa @ ba; z = y - l2
    xv = pa * l2 - np.outer(y, ba); x2 = np.einsum("ij,ij->i", xv, xv)
    y2 = y * y * l2; z2 = z * z * l2; k = np.sign(rr) * rr * rr * x2
    out = (np.sqrt(x2 + z2) * il2 - r2).copy()
    m1 = np.sign(z) * a2 * z2 > k
    m2 = np.sign(y) * a2 * y2 < k
    out[m2] = (np.sqrt(x2[m2] + y2[m2]) * il2 - r1)
    rest = ~m1 & ~m2
    out[rest] = ((np.sqrt(x2[rest] * a2 * il2) + y[rest] * rr) * il2 - r1)
    out[m1] = (np.sqrt(x2[m1] + z2[m1]) * il2 - r2)
    return out

def ellipsoid(p, c, r):
    q = (p - c) / r
    k0 = np.linalg.norm(q, axis=1); k1 = np.linalg.norm(q / r, axis=1)
    return k0 * (k0 - 1.0) / np.maximum(k1, 1e-9)

def smin(a, b, k):
    h = np.clip(0.5 + 0.5 * (b - a) / k, 0, 1)
    return b * (1 - h) + a * h - k * h * (1 - h)

def smax(a, b, k):
    return -smin(-a, -b, k)

def mirrored(fn):
    def f(p):
        q = p.copy(); q[:, 0] = np.abs(q[:, 0]); return fn(q)
    return f

BIG = 1e9

def body_left(p):
    """Everything that is symmetric, evaluated with |x|."""
    d = np.full(len(p), BIG)
    U = lambda dd, k: smin(d, dd, k)
    # shoulder masses and arms
    d = U(ellipsoid(p, np.array([40, 186, -3]), np.array([24, 21, 22])), 10)
    d = U(round_cone(p, P("L_shoulder"), P("L_elbow"), 16, 12), 9)
    d = U(ellipsoid(p, (P("L_shoulder") * 0.55 + P("L_elbow") * 0.45) + np.array([0, 0, 4]), np.array([13, 20, 14])), 8)  # biceps
    d = U(round_cone(p, P("L_elbow"), P("L_wrist"), 13, 9), 6)
    d = U(ellipsoid(p, (P("L_elbow") * 0.65 + P("L_wrist") * 0.35) + np.array([2, 0, -2]), np.array([13, 17, 12])), 7)  # forearm bulk
    d = U(ellipsoid(p, P("L_palm") + np.array([2, -3, 3]), np.array([10, 12, 10])), 6)
    for k in "ABC":
        a, b, c, e = (P("L_claw%s%s" % (k, s)) for s in ("01", "02", "03", "End"))
        d = U(round_cone(p, a, b, 5.2, 4.2), 3)
        d = U(round_cone(p, b, c, 4.2, 3.4), 2)
    d = U(round_cone(p, P("L_thumb01"), P("L_thumb02"), 5.5, 4.3), 3)
    # elbow spur
    d = U(round_cone(p, P("L_elbow") + np.array([0, 2, -6]), P("L_elbow") + np.array([6, 6, -22]), 6, 0.8), 3)
    # legs
    d = U(round_cone(p, P("L_hip"), P("L_knee"), 19, 13), 10)
    d = U(ellipsoid(p, (P("L_hip") * 0.55 + P("L_knee") * 0.45) + np.array([3, 0, 2]), np.array([16, 22, 17])), 8)
    d = U(round_cone(p, P("L_knee"), P("L_ankle"), 13, 8), 6)
    d = U(ellipsoid(p, (P("L_knee") * 0.6 + P("L_ankle") * 0.4) + np.array([0, 0, -6]), np.array([11, 15, 11])), 6)  # calf
    d = U(round_cone(p, P("L_ankle"), P("L_ball"), 9, 7), 5)
    d = U(ellipsoid(p, P("L_ball") + np.array([0, 0, -4]), np.array([13, 6, 13])), 5)
    for k in "ABC":
        d = U(round_cone(p, P("L_toe%s01" % k), P("L_toe%sEnd" % k) - np.array([0, 0, 6]), 4.5, 3.5), 3)
    # head: brow, cheeks, ear knobs
    d = U(round_cone(p, np.array([3, 234, 41]), np.array([19, 233, 33]), 6.5, 5), 4)
    d = U(ellipsoid(p, np.array([15, 216, 32]), np.array([10, 9, 12])), 6)  # cheek
    return d

def claws_left(p):
    d = np.full(len(p), BIG)
    for k in "ABC":
        c, e = P("L_claw%s03" % k), P("L_claw%sEnd" % k)
        d = np.minimum(d, round_cone(p, c, e + (e - c) * 0.6, 3.4, 0.4))
        t = P("L_toe%sEnd" % k)
        d = np.minimum(d, round_cone(p, t - np.array([0, 0, 6]), t + np.array([0, -2, 6]), 3.4, 0.4))
    d = np.minimum(d, round_cone(p, P("L_thumb02"), P("L_thumbEnd") + np.array([-2, -4, 5]), 4.2, 0.5))
    return d

def horns_left(p):
    d = np.full(len(p), BIG)
    ts = np.linspace(0, 1, 46)
    pts = [horn_path(t) for t in ts]
    rad = [(8.8 * (1 - t) ** 0.75 + 0.8) * (1 + 0.06 * np.cos(t * 2 * np.pi * 9)) for t in ts]
    for i in range(len(ts) - 1):
        d = np.minimum(d, round_cone(p, pts[i], pts[i + 1], rad[i], rad[i + 1]))
    # brow horns, forward and up
    d = np.minimum(d, round_cone(p, np.array([9, 239, 38]), np.array([12, 255, 50]), 4.2, 0.6))
    # tusks from the jaw
    d = np.minimum(d, round_cone(p, np.array([12, 207, 46]), np.array([16, 223, 52]), 3.4, 0.5))
    return d

def spikes(p):
    d = np.full(len(p), BIG)
    for y, z, L in [(196, -20, 12), (182, -32, 15), (166, -36, 16), (148, -36, 14), (130, -32, 11), (114, -30, 8)]:
        a = np.array([0, y, z]); d = np.minimum(d, round_cone(p, a, a + np.array([0, 0.45 * L, -L]), 4.5, 0.5))
    for i in range(1, 7):
        a = P("C_tail%02d" % i); b = P("C_tail%02d" % (i + 1)) if i < 8 else P("C_tailEnd")
        m = (a + b) / 2; n = np.cross(b - a, [1, 0, 0]); n /= np.linalg.norm(n)
        L = 9 - i
        d = np.minimum(d, round_cone(p, m - n * 6 * (1.2 - i / 9), m - n * (6 * (1.2 - i / 9) + L) + (b - a) * 0.25, 3.0, 0.4))
    return d

def body(p):
    d = np.full(len(p), BIG)
    U = lambda dd, k: smin(d, dd, k)
    d = U(ellipsoid(p, np.array([0, 108, -6]), np.array([36, 27, 29])), 1)
    d = U(ellipsoid(p, np.array([0, 136, 5]), np.array([38, 32, 31])), 14)
    d = U(ellipsoid(p, np.array([0, 170, -2]), np.array([50, 33, 35])), 14)
    d = U(ellipsoid(p, np.array([0, 188, -15]), np.array([38, 19, 25])), 12)
    d = U(round_cone(p, np.array([0, 194, 2]), np.array([0, 213, 17]), 18, 16), 10)
    d = U(ellipsoid(p, np.array([0, 222, 20]), np.array([23, 17, 24])), 8)   # skull, low and flat
    d = U(ellipsoid(p, np.array([0, 232, 34]), np.array([17, 6, 10])), 4)    # forehead plate
    d = U(ellipsoid(p, np.array([0, 217, 43]), np.array([16, 11, 15])), 6)   # snout
    d = U(round_cone(p, np.array([0, 229, 44]), np.array([0, 219, 57]), 5, 4.5), 3)  # nose ridge
    d = U(ellipsoid(p, np.array([0, 203, 38]), np.array([22, 10, 22])), 7)   # heavy jaw, underbite
    d = U(ellipsoid(p, np.array([0, 198, 33]), np.array([15, 8, 15])), 6)    # chin
    # tail
    ts = tail_curve(40)
    rs = 15 * (1 - np.linspace(0, 1, len(ts))) ** 0.9 + 2.0
    for i in range(len(ts) - 1):
        d = np.minimum(d, round_cone(p, ts[i], ts[i + 1], rs[i], rs[i + 1])) if i else U(round_cone(p, ts[i], ts[i + 1], rs[i], rs[i + 1]), 9)
    d = smin(d, mirrored(body_left)(p), 9)
    # mouth: a slot between the lips, and eye sockets
    slot = ellipsoid(p, np.array([0, 210.5, 44]), np.array([15, 1.8, 16]))
    d = smax(d, -slot, 1.5)
    eyes = mirrored(lambda q: ellipsoid(q, np.array([9.5, 228.5, 41]), np.array([5.6, 5.6, 5.6])))(p)
    d = smax(d, -eyes, 1.0)
    return d

def sdf(p):
    d = body(p)
    d = smin(d, spikes(p), 2.5)
    d = smin(d, mirrored(horns_left)(p), 2.0)
    d = smin(d, mirrored(claws_left)(p), 1.5)
    return d

def material_id(p):
    """0 skin, 1 horn and spikes, 2 claws and tusks."""
    db = body(p)
    dh = np.minimum(mirrored(horns_left)(p), spikes(p))
    dc = mirrored(claws_left)(p)
    m = np.zeros(len(p), int)
    m[(dh < db) & (dh <= dc)] = 1
    m[(dc < db) & (dc < dh)] = 2
    return m, db, dh, dc
