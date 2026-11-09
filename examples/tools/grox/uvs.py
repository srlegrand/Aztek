"""Step 2: imports the sculpt, gives it UVs (automatic projection, unfold, layout) and saves grox_geo.ma and uvdata.json.
Run inside Aztek from the work folder: GROX_WORK=$PWD aztek --script uvs.py"""
import sys, os, json, traceback, time
import maya.cmds as mc
M = os.environ.get("GROX_WORK", os.getcwd()) + "/"
log = []
try:
    mc.file(new=True, f=True)
    n = mc.importObj(M + "grox_body.obj"); log.append(("import", n))
    g = mc.rename(mc.listRelatives(mc.ls(type="mesh")[0], p=True)[0], "grox_body_GEO")
    mc.polySoftEdge(g, a=180)
    t = time.time(); mc.polyAutoProjection(g, ps=0.2, pb=False); log.append(("auto", time.time() - t, mc.polyEvaluate(g, uvShell=True) if False else ""))
    t = time.time(); mc.u3dUnfold(g); log.append(("unfold", time.time() - t))
    t = time.time(); mc.u3dLayout(g, res=2048); log.append(("layout", time.time() - t))
    mc.delete(g, ch=True)
    import maya.api.OpenMaya as om
    sl = om.MSelectionList(); sl.add(g); dp = sl.getDagPath(0); fn = om.MFnMesh(dp)
    us, vs = fn.getUVs()
    counts, uvids = fn.getAssignedUVs()
    vc, vids = fn.getVertices()
    pts = fn.getPoints()
    json.dump({"u": list(us), "v": list(vs), "counts": list(counts), "uvids": list(uvids), "vids": list(vids),
               "p": [[p.x, p.y, p.z] for p in pts]}, open(M + "uvdata.json", "w"))
    mc.file(rename=M + "grox_geo.ma"); mc.file(save=True, type="mayaAscii")
    log.append(("ok", len(us), len(pts)))
except Exception:
    log.append(traceback.format_exc())
open(M + "stepA.log", "w").write("\n".join(map(str, log)))
os._exit(0)
