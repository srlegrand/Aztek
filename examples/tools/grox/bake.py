"""Paints Grox's textures procedurally in 3D and bakes them into the UV layout: base colour, tangent-space normal,
ORM (occlusion, roughness, metalness) and a mask map (belly, markings, horn)."""
import json, sys, time
import numpy as np
from PIL import Image, ImageDraw
from scipy import ndimage
import design as D

RES = int(sys.argv[1]) if len(sys.argv) > 1 else 2048
d = json.load(open("uvdata.json"))
U = np.c_[d["u"], d["v"]]
uvi = np.array(d["uvids"]).reshape(-1, 3)
vi = np.array(d["vids"]).reshape(-1, 3)
P = np.array(d["p"])

# vertex normals
fn = np.cross(P[vi[:, 1]] - P[vi[:, 0]], P[vi[:, 2]] - P[vi[:, 0]])
VN = np.zeros_like(P)
for k in range(3):
    np.add.at(VN, vi[:, k], fn)
VN /= np.linalg.norm(VN, axis=1, keepdims=True) + 1e-12

# triangle id map
t0 = time.time()
im = Image.new("RGB", (RES, RES), (255, 255, 255)); dr = ImageDraw.Draw(im)
for t in range(len(uvi)):
    c = t + 1
    dr.polygon([(U[i, 0] * RES - 0.5, (1 - U[i, 1]) * RES - 0.5) for i in uvi[t]], fill=(c & 255, (c >> 8) & 255, (c >> 16) & 255), outline=(c & 255, (c >> 8) & 255, (c >> 16) & 255))
idm = np.asarray(im).astype(np.int64)
tid = idm[..., 0] + (idm[..., 1] << 8) + (idm[..., 2] << 16) - 1
tid[tid >= len(uvi)] = -1
print("raster", round(time.time() - t0, 1), flush=True)

ys, xs = np.nonzero(tid >= 0)
T = tid[ys, xs]
q = np.c_[(xs + 0.5) / RES, 1 - (ys + 0.5) / RES]
a, b, c = U[uvi[T, 0]], U[uvi[T, 1]], U[uvi[T, 2]]
v0, v1, v2 = b - a, c - a, q - a
den = v0[:, 0] * v1[:, 1] - v1[:, 0] * v0[:, 1]
den[np.abs(den) < 1e-14] = 1e-14
w1 = (v2[:, 0] * v1[:, 1] - v1[:, 0] * v2[:, 1]) / den
w2 = (v0[:, 0] * v2[:, 1] - v2[:, 0] * v0[:, 1]) / den
w0 = 1 - w1 - w2
W = np.clip(np.c_[w0, w1, w2], 0, 1); W /= W.sum(1, keepdims=True)
p = (P[vi[T]] * W[:, :, None]).sum(1)
n = (VN[vi[T]] * W[:, :, None]).sum(1); n /= np.linalg.norm(n, axis=1, keepdims=True) + 1e-12
# tangent frame per triangle
e1, e2 = P[vi[:, 1]] - P[vi[:, 0]], P[vi[:, 2]] - P[vi[:, 0]]
du1, du2 = U[uvi[:, 1]] - U[uvi[:, 0]], U[uvi[:, 2]] - U[uvi[:, 0]]
r = du1[:, 0] * du2[:, 1] - du2[:, 0] * du1[:, 1]; r[np.abs(r) < 1e-14] = 1e-14
TAN = (e1 * du2[:, 1:2] - e2 * du1[:, 1:2]) / r[:, None]
BIT = (e2 * du1[:, 0:1] - e1 * du2[:, 0:1]) / r[:, None]
print("texels", len(p), flush=True)

# ---------------------------------------------------------------------------------------------- noise
def hash3(i, j, k, seed):
    h = (i * 73856093) ^ (j * 19349663) ^ (k * 83492791) ^ (seed * 2654435761)
    h = (h ^ (h >> 13)) * 1274126177
    return ((h ^ (h >> 16)) & 0xFFFFFF) / float(0xFFFFFF)

def vnoise(x, seed=0):
    f = np.floor(x).astype(np.int64); t = x - f; t = t * t * (3 - 2 * t)
    out = 0
    for dx in (0, 1):
        for dy in (0, 1):
            for dz in (0, 1):
                w = (t[:, 0] if dx else 1 - t[:, 0]) * (t[:, 1] if dy else 1 - t[:, 1]) * (t[:, 2] if dz else 1 - t[:, 2])
                out = out + w * hash3(f[:, 0] + dx, f[:, 1] + dy, f[:, 2] + dz, seed)
    return out

def fbm(x, oct=4, seed=0):
    s, a, tot = 0, 1.0, 0
    for o in range(oct):
        s = s + a * vnoise(x * (2 ** o), seed + o); tot += a; a *= 0.5
    return s / tot

def cells(x, seed=0):
    """Distance to the nearest of jittered points: scales."""
    f = np.floor(x).astype(np.int64); best = np.full(len(x), 9.0); second = np.full(len(x), 9.0)
    for dx in (-1, 0, 1):
        for dy in (-1, 0, 1):
            for dz in (-1, 0, 1):
                g = f + np.array([dx, dy, dz])
                j = np.c_[hash3(g[:, 0], g[:, 1], g[:, 2], seed), hash3(g[:, 0], g[:, 1], g[:, 2], seed + 7), hash3(g[:, 0], g[:, 1], g[:, 2], seed + 13)]
                dd = np.linalg.norm(g + j - x, axis=1)
                second = np.where(dd < best, best, np.minimum(second, dd)); best = np.minimum(best, dd)
    return best, second

# ---------------------------------------------------------------------------------------------- regions
mid, db, dh, dc = D.material_id(p)
horn = (mid == 1).astype(float); claw = (mid == 2).astype(float); skin = (mid == 0).astype(float)
# soften the borders between materials
blend_h = np.clip(0.5 + (db - dh) / 2.0, 0, 1)
blend_c = np.clip(0.5 + (db - dc) / 2.0, 0, 1)

# occlusion from the distance field
t0 = time.time()
ao = np.ones(len(p))
for dist, wgt in ((1.5, 0.5), (4.0, 0.35), (9.0, 0.25), (18.0, 0.15)):
    s = D.sdf(p + n * dist)
    ao -= wgt * np.clip((dist - s) / dist, 0, 1)
ao = np.clip(ao, 0.15, 1)
# convexity for painted edge light
cv = np.clip((D.sdf(p + n * 2.5) - 2.5) * -0.6, -1, 1)  # >0 in creases
print("ao", round(time.time() - t0, 1), flush=True)

x, y, z = p[:, 0], p[:, 1], p[:, 2]
ax = np.abs(x)
low = fbm(p * 0.035, 3, 1)
mid_n = fbm(p * 0.12, 3, 5)
# hand-painted strokes: noise stretched along the body's long axis
stroke = fbm(p * np.array([0.25, 0.04, 0.25]), 3, 9)

# belly and inner limbs: lighter, warmer
belly = np.clip((n[:, 2] - 0.15) * 2.5, 0, 1) * np.clip(1 - (ax - 24) / 14, 0, 1) * np.clip((y - 95) / 15, 0, 1) * np.clip((205 - y) / 15, 0, 1)
throat = np.clip((n[:, 2] * 0.5 - n[:, 1]) * 2, 0, 1) * np.clip((y - 192) / 6, 0, 1) * np.clip((214 - y) / 6, 0, 1) * np.clip(1 - ax / 16, 0, 1)
belly = np.clip(belly + throat, 0, 1) * (0.75 + 0.25 * mid_n)
# back markings: dark bands across the back and the tail, broken by noise
back = np.clip(-n[:, 2] * 1.8 + 0.3, 0, 1)
bands = np.clip((np.sin(y * 0.16 + 6 * low + ax * 0.05) - 0.15) * 3, 0, 1)
tailband = np.clip((np.sin(-z * 0.11 + 4 * low) - 0.1) * 3, 0, 1) * (z < -25)
marks = np.clip(np.maximum(bands * back, tailband * back) * (mid_n > 0.38), 0, 1)
# extremities darker: hands, feet, tail tip
extrem = np.clip(np.maximum.reduce([(ax - 90) / 12, (18 - y) / 14, (-z - 110) / 25]), 0, 1)

skin_base = np.array([0.30, 0.075, 0.035])
skin_dark = np.array([0.09, 0.022, 0.03])
belly_col = np.array([0.62, 0.34, 0.14])
mark_col = np.array([0.045, 0.012, 0.025])
col = skin_base * (0.82 + 0.36 * low[:, None]) + (np.array([0.10, 0.06, -0.02]) * (stroke[:, None] - 0.5))
col = col * (1 - extrem[:, None]) + skin_dark * extrem[:, None]
col = col * (1 - belly[:, None]) + belly_col * (0.9 + 0.2 * stroke[:, None]) * belly[:, None]
col = col * (1 - marks[:, None] * 0.85) + mark_col * marks[:, None] * 0.85
# scales: darker outlines on the back and limbs
sc1, sc2 = cells(p * 0.45, 3)
edge = np.clip(1 - (sc2 - sc1) * 6, 0, 1) * (1 - belly) * 0.35
col *= (1 - edge[:, None] * 0.5)
# painted light: warm top, cool bottom; creases dark and red; edges light
col *= (0.82 + 0.28 * np.clip(n[:, 1], -1, 1)[:, None])
col += np.array([-0.03, -0.01, 0.04]) * np.clip(-n[:, 1], 0, 1)[:, None]
col = col * (0.55 + 0.45 * ao[:, None]) + np.array([0.12, 0.0, 0.0]) * (1 - ao[:, None]) * 0.5
col += np.array([0.05, 0.035, 0.02]) * np.clip(-cv, 0, 1)[:, None]

# horns: dark at the root, ivory at the tip, grooves along their length
hp = np.array([D.horn_path(t) for t in np.linspace(0, 1, 46)])
hq = p.copy(); hq[:, 0] = np.abs(hq[:, 0])
hd = np.linalg.norm(hq[:, None, :] - hp[None, :, :], axis=2)
ht = np.argmin(hd, 1) / 45.0
brow = (np.linalg.norm(hq - np.array([12, 255, 50]), axis=1) < 22) | (y < 230)  # brow horns, tusks and spikes
ht = np.where(brow, np.clip(1 - dh * 0, 0, 1) * 0.6, ht)
groove = 0.5 + 0.5 * np.cos(ht * 2 * np.pi * 9)
horn_col = np.array([0.07, 0.045, 0.03]) * (1 - ht[:, None]) + np.array([0.74, 0.64, 0.46]) * ht[:, None]
horn_col = horn_col * (0.78 + 0.22 * groove[:, None]) * (0.85 + 0.3 * stroke[:, None])
horn_col = horn_col * (0.6 + 0.4 * ao[:, None])
claw_col = np.array([0.10, 0.07, 0.07]) * (1 - mid_n[:, None]) + np.array([0.75, 0.70, 0.60]) * mid_n[:, None] * 0.5
claw_col = claw_col * (0.7 + 0.3 * ao[:, None])
col = col * (1 - blend_h[:, None]) + horn_col * blend_h[:, None]
col = col * (1 - blend_c[:, None]) + claw_col * blend_c[:, None]
# mouth inside: dark wet red
mouth = np.clip(1 - D.ellipsoid(p, np.array([0, 210.5, 44]), np.array([17, 4.5, 18])) / 2, 0, 1)
col = col * (1 - mouth[:, None]) + np.array([0.25, 0.04, 0.05]) * mouth[:, None]
col = np.clip(col, 0, 1)

# ---------------------------------------------------------------------------------------------- normal
def height(q):
    s1, s2 = cells(q * 0.45, 3)
    sc = np.clip((s2 - s1) * 4, 0, 1)
    wr = fbm(q * np.array([0.08, 0.5, 0.08]), 2, 21)  # wrinkles across limbs
    pores = vnoise(q * 1.6, 31)
    return 0.9 * sc + 0.5 * wr + 0.25 * pores

t0 = time.time()
eps = 0.25
g = np.c_[[(height(p + e) - height(p - e)) / (2 * eps) for e in np.eye(3) * eps]].T
gt = g - (g * n).sum(1, keepdims=True) * n
k = (0.35 * skin + 0.15 * horn + 0.05 * claw) * (1 - mouth)
nn = n - k[:, None] * gt
# horn ribs
hg = np.sin(ht * 2 * np.pi * 9)
nn += (horn * 0.25 * hg)[:, None] * np.cross(n, np.cross(n, np.gradient(ht)[:, None] * 0 + np.array([0, 1, 0]))) * 0
nn /= np.linalg.norm(nn, axis=1, keepdims=True)
Tt, Bt = TAN[T], BIT[T]
Tt = Tt - (Tt * n).sum(1, keepdims=True) * n; Tt /= np.linalg.norm(Tt, axis=1, keepdims=True) + 1e-12
hand = np.sign((np.cross(n, Tt) * Bt).sum(1))
Bt = np.cross(n, Tt) * hand[:, None]
ts = np.c_[(nn * Tt).sum(1), (nn * Bt).sum(1), (nn * n).sum(1)]
print("normal", round(time.time() - t0, 1), flush=True)

rough = np.clip(0.62 * skin + 0.42 * horn + 0.30 * claw + 0.1 * (stroke - 0.5) - 0.35 * mouth, 0.15, 1)

def to_img(vals, fill):
    img = np.zeros((RES, RES, vals.shape[1]), np.float32); m = np.zeros((RES, RES), bool)
    img[ys, xs] = vals; m[ys, xs] = True
    # pad the islands outward so filtering doesn't bleed the background in
    idx = ndimage.distance_transform_edt(~m, return_distances=False, return_indices=True)
    img = img[idx[0], idx[1]]
    return img

def save(name, vals, srgb=True):
    img = to_img(vals, 0)
    if srgb:
        img = np.where(img <= 0.0031308, img * 12.92, 1.055 * np.power(np.clip(img, 0, 1), 1 / 2.4) - 0.055)
    Image.fromarray((np.clip(img, 0, 1) * 255 + 0.5).astype(np.uint8)).save(name, optimize=True)
    print("saved", name, flush=True)

save("grox_body_baseColor.png", col)
save("grox_body_normal.png", ts * 0.5 + 0.5, srgb=False)
save("grox_body_ORM.png", np.c_[ao, rough, np.zeros(len(ao))], srgb=False)
save("grox_body_mask.png", np.c_[belly, marks, np.clip(blend_h + blend_c, 0, 1)], srgb=False)
