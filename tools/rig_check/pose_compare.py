"""Compares two pose dumps from pose_dump.py: the reference's and Aztek's. For each pose, the joint furthest off (its
axes' angle and its position) and the skinned mesh furthest off; poses where everything matches are summed up.

    python3 tools/rig_check/pose_compare.py reference.json aztek.json
"""

import json
import math
import sys


def axes_angle(a, b):
    worst = 0.0
    for r in range(3):
        u, v = a[r * 4:r * 4 + 3], b[r * 4:r * 4 + 3]
        lu, lv = math.sqrt(sum(x * x for x in u)), math.sqrt(sum(x * x for x in v))
        if lu < 1e-9 or lv < 1e-9:
            continue
        d = max(-1.0, min(1.0, sum(x * y for x, y in zip(u, v)) / (lu * lv)))
        worst = max(worst, math.degrees(math.acos(d)))
    return worst


def main():
    ref, got = (json.load(open(p)) for p in sys.argv[1:3])
    tol_deg, tol_pos = 0.5, 0.01
    same = 0
    for pr, pg in zip(ref["poses"], got["poses"]):
        if pr["name"] != pg["name"]:
            print("pose lists differ at", pr["name"], pg["name"])
            return 1
        jw = (0.0, 0.0, None)
        for j, m in pr["joints"].items():
            n = pg["joints"].get(j)
            if n is None:
                continue
            ang = axes_angle(m, n)
            pos = math.dist(m[12:15], n[12:15])
            if ang + pos > jw[0] + jw[1]:
                jw = (ang, pos, j)
        mw = (0.0, None)
        for mesh, pts in pr["points"].items():
            other = pg["points"].get(mesh)
            if not other:
                continue
            d = max((math.dist(a, b) for a, b in zip(pts, other)), default=0.0)
            if d > mw[0]:
                mw = (d, mesh)
        if jw[0] <= tol_deg and jw[1] <= tol_pos and mw[0] <= tol_pos:
            same += 1
            continue
        print("%-34s joint %-40s %7.2f deg %8.3f   mesh %-28s %8.3f" % (pr["name"], jw[2], jw[0], jw[1], mw[1], mw[0]))
    print("%d of %d poses match" % (same, len(ref["poses"])))
    return 0


if __name__ == "__main__":
    sys.exit(main())
