"""Grox's skin weights: distance to each bone over its thickness, limited by region, smoothed over the surface,
four influences, written into the skinCluster in the saved .ma."""
import re, sys, json
import numpy as np
import scipy.sparse as sp
import design as D

ma = sys.argv[1]
SK = D.full_skeleton()
d = json.load(open("uvdata.json"))
P = np.array(d["p"]); F = np.array(d["vids"]).reshape(-1, 3)
text = open(ma).read()
idx = {m.group(1): int(m.group(2)) for m in re.finditer(r'connectAttr "(\w+)_JNT\.wm" "grox_body_SKN\.ma\[(\d+)\]";', text)}

def child(n):
    kids = [k for k, (p, _) in SK.items() if p == n]
    pref = [k for k in kids if not any(t in k for t in ("lid", "eye", "brow", "jaw", "clavicle", "thumb", "hip", "tail", "horn", "toeA", "toeB", "toeC", "clawA", "clawB", "clawC"))]
    return (pref or kids or [None])[0]

def radius(n):
    b = n[2:]
    table = [("root", 34), ("spine01", 34), ("spine02", 36), ("spine03", 40), ("chest", 44), ("neck", 18), ("head", 24), ("jaw", 18),
             ("clavicle", 16), ("shoulder", 16), ("elbow", 12), ("wrist", 9), ("palm", 9), ("claw", 5), ("thumb", 5),
             ("hip", 18), ("knee", 12), ("ankle", 9), ("ball", 9), ("toeA", 5), ("toeB", 5), ("toeC", 5), ("horn", 7), ("brow", 5),
             ("eye", 5), ("lid", 4), ("tail", 12)]
    for k, r in table:
        if b.startswith(k):
            if k == "tail":
                return max(3.0, 15 - int(b[4:6]) * 1.6)
            if k == "horn":
                return max(2.0, 8.5 - int(b[4:6]) * 1.4)
            return r
    return 10

def seg_dist(p, a, b):
    if b is None:
        return np.linalg.norm(p - a, axis=1)
    ab = b - a; t = np.clip(((p - a) @ ab) / (ab @ ab), 0, 1)
    return np.linalg.norm(p - (a + t[:, None] * ab), axis=1)

names = [n for n in idx]
nv = len(P)
score = np.zeros((nv, len(names)))
mid, db, dh, dc = D.material_id(P)
x, y, z = P[:, 0], P[:, 1], P[:, 2]; ax = np.abs(x)
headzone = y > 196
mouthline = 210.5 + 0 * z
for k, n in enumerate(names):
    a = SK[n][1]; c = child(n); b = SK[c][1] if c else None
    dist = seg_dist(P, a, b) / radius(n)
    w = 1.0 / (dist ** 4 + 1e-3)
    allow = np.ones(nv, bool)
    side = n[0]
    if side in "LR":
        allow &= (x > -2) if side == "L" else (x < 2)
    b_ = n[2:]
    if b_.startswith("horn"):
        allow &= (mid == 1) & (y > 228)
    elif mid is not None and not b_.startswith(("head", "brow", "claw", "thumb", "toe", "tail", "spine", "chest", "root")):
        pass
    if b_.startswith(("lid", "eye")):
        allow &= np.linalg.norm(P - SK[n][1], axis=1) < 9
        w *= 0.6
    if b_.startswith("brow"):
        allow &= np.linalg.norm(P - SK[n][1], axis=1) < 11
    if b_.startswith("jaw"):
        allow &= headzone & (y < mouthline + 1) & (z > 14)
    if b_.startswith("head"):
        allow &= headzone & ~((y < mouthline - 1) & (z > 22) & (ax < 24))
    if b_.startswith(("claw", "thumb", "palm")):
        allow &= ax > 88
    if b_.startswith("toe"):
        allow &= y < 14
    if b_.startswith("tail"):
        allow &= z < -18
    if b_.startswith(("root", "spine", "chest", "hip")):
        allow &= ~((z < -40) & (y < 100))
    # horn, claw and tusk vertices only follow their own bones (plus head and hands)
    if not b_.startswith(("horn", "head", "brow")):
        allow &= ~((mid == 1) & (y > 228))
    score[:, k] = np.where(allow, w, 0)

# vertices left with nothing (rare): nearest bone without limits
empty = score.sum(1) == 0
if empty.any():
    for k, n in enumerate(names):
        a = SK[n][1]; c = child(n); b = SK[c][1] if c else None
        score[empty, k] = 1 / (seg_dist(P[empty], a, b) ** 4 + 1e-3)
W = score / score.sum(1, keepdims=True)

# smooth over the surface
e = np.vstack([F[:, [0, 1]], F[:, [1, 2]], F[:, [2, 0]]]); e = np.vstack([e, e[:, ::-1]])
A = sp.coo_matrix((np.ones(len(e)), (e[:, 0], e[:, 1])), shape=(nv, nv)).tocsr(); A.data[:] = 1
deg = np.asarray(A.sum(1)).ravel()
rigid = (mid != 0)  # horns and claws stay sharp
for it in range(10):
    Ws = A @ W / deg[:, None]
    W = np.where(rigid[:, None], W, 0.5 * W + 0.5 * Ws)
# four largest, normalised
top = np.argsort(-W, 1)[:, :4]
vals = np.take_along_axis(W, top, 1)
vals[vals < 0.01] = 0
vals /= vals.sum(1, keepdims=True)

lines = ['\tsetAttr -s %d ".wl";' % nv]
for i in range(nv):
    pairs = [(idx[names[top[i, j]]], vals[i, j]) for j in range(4) if vals[i, j] > 0]
    pairs.sort()
    lines.append('\tsetAttr -s %d ".wl[%d].w";' % (len(pairs), i))
    for m, v in pairs:
        lines.append('\tsetAttr ".wl[%d].w[%d]" %.6g;' % (i, m, v))
start = text.index('createNode skinCluster -n "grox_body_SKN";')
s0 = text.index("\n", start) + 1
# the weight lines run until the first line that isn't a weightList setAttr
pos = s0
while True:
    nl = text.index("\n", pos)
    line = text[pos:nl]
    if not (line.startswith('\tsetAttr -s') and '.wl' in line or line.startswith('\tsetAttr ".wl[')):
        break
    pos = nl + 1
text = text[:s0] + "\n".join(lines) + "\n" + text[pos:]
open(ma, "w").write(text)
print("weights written", nv, "influences", len(names))
