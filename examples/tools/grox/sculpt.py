"""Step 1: samples the distance field in design.py, meshes it and reduces it to grox_body.obj: python3 sculpt.py 1.25 60000"""
import numpy as np, time, sys
from skimage import measure
import pyfqmr
from design import sdf
h = float(sys.argv[1]) if len(sys.argv) > 1 else 2.0
lo = np.array([-165, -4, -175.]); hi = np.array([165, 285, 85.])
xs = [np.arange(lo[i], hi[i] + h, h) for i in range(3)]
shape = tuple(len(x) for x in xs)
X, Y, Z = np.meshgrid(*xs, indexing="ij")
pts = np.stack([X.ravel(), Y.ravel(), Z.ravel()], 1)
t = time.time(); vol = np.empty(len(pts))
for i in range(0, len(pts), 400000):
    vol[i:i + 400000] = sdf(pts[i:i + 400000])
print("sdf", shape, round(time.time() - t, 1), "s", flush=True)
vol = vol.reshape(shape)
v, f, n, _ = measure.marching_cubes(vol, 0.0, spacing=(h, h, h))
v += lo
print("mc", len(v), len(f))
# Taubin smoothing to take out the grid steps
import scipy.sparse as sp
def taubin(v, f, it=6, lam=0.5, mu=-0.53):
    nv = len(v); e = np.vstack([f[:, [0, 1]], f[:, [1, 2]], f[:, [2, 0]]]); e = np.vstack([e, e[:, ::-1]])
    A = sp.coo_matrix((np.ones(len(e)), (e[:, 0], e[:, 1])), shape=(nv, nv)).tocsr(); A.data[:] = 1
    deg = np.asarray(A.sum(1)).ravel(); deg[deg == 0] = 1
    for _ in range(it):
        for k in (lam, mu):
            v = v + k * (A @ v / deg[:, None] - v)
    return v
v = taubin(v, f)
s = pyfqmr.Simplify(); s.setMesh(v, f)
target = int(sys.argv[2]) if len(sys.argv) > 2 else 36000
s.simplify_mesh(target_count=target, aggressiveness=5, preserve_border=True, verbose=0)
v, f, _ = s.getMesh()
print("reduced", len(v), len(f))
np.savez("grox_mesh.npz", v=v, f=f)
with open("grox_body.obj", "w") as o:
    o.write("".join("v %.4f %.4f %.4f\n" % tuple(x) for x in v))
    o.write("".join("f %d %d %d\n" % tuple(x + 1) for x in f))
