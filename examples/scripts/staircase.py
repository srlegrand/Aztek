"""A spiral staircase: steps duplicated round a post, grouped, with a handrail curve through their ends."""
from aztek import cmds

STEPS, RISE, TURN, RADIUS = 24, 18.0, 15.0, 120.0

post = cmds.polyCylinder(n="post", r=12, h=STEPS * RISE + 100, sx=16)[0]
cmds.move(0, (STEPS * RISE + 100) / 2, 0, post)
steps, rail = [], []
for i in range(STEPS):
    s = cmds.polyCube(n="step%02d" % i, w=RADIUS, h=6, d=40)[0]
    cmds.xform(s, piv=(-RADIUS / 2, 0, 0), os=True)
    cmds.move(RADIUS / 2, i * RISE + 3, 0, s)
    cmds.rotate(0, i * TURN, 0, s, ws=True)
    steps.append(s)
    end = cmds.xform(s, q=True, ws=True, t=True)
    a = (i * TURN) * 3.14159265 / 180
    rail.append((RADIUS * 0.95 * __import__("math").cos(a), i * RISE + 90, -RADIUS * 0.95 * __import__("math").sin(a)))
cmds.group(steps, post, n="staircase")
cmds.curve(n="handrail", d=3, p=rail)
print("Built %d steps." % STEPS)
