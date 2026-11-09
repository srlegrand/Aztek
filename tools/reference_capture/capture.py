# Runs inside the reference and writes the fixtures Aztek's tests compare against. Plain Python, either way:
#
#   mayapy capture.py [output folder]
#   or, in the Script Editor:   exec(open("/path/to/tools/reference_capture/capture.py").read())
#
# Output defaults to tests/data/reference in this repository, one folder per area. Commit that folder. Set
# CAPTURE_ONLY=calls,attributes,commands,scenes to run a part again; a part that fails leaves the others alone and its
# error is in errors.json.
import json
import os
import platform
import sys
import traceback

try:
    HERE = os.path.dirname(os.path.abspath(__file__))
except NameError:
    HERE = os.environ.get("CAPTURE_DIR", os.getcwd())

_standalone = False
try:
    import maya.standalone

    maya.standalone.initialize(name="python")
    _standalone = True
except Exception:
    pass  # already inside a running session

import maya.cmds as cmds
import maya.mel as mel

OUT = None
ERRORS = {}


def plain(v):
    """What JSON can hold, with tuples as lists; anything else as its repr."""
    if v is None or isinstance(v, (bool, int, float, str)):
        return v
    if isinstance(v, (list, tuple)):
        return [plain(x) for x in v]
    if isinstance(v, dict):
        return {str(k): plain(x) for k, x in v.items()}
    return repr(v)


def write(rel, data):
    path = os.path.join(OUT, rel)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", newline="\n") as f:
        json.dump(data, f, indent=1, sort_keys=True)
        f.write("\n")


def fresh():
    cmds.file(new=True, force=True)
    cmds.currentUnit(linear="cm", angle="deg", time="film")


def attempt(fn, *args, **kwargs):
    """The answer, or {"error": text}: an error is a result worth having."""
    try:
        return plain(fn(*args, **kwargs))
    except Exception as e:
        return {"error": str(e).strip()}


def capture_calls():
    with open(os.path.join(HERE, "calls.json")) as f:
        calls = json.load(f)["calls"]
    areas = {}
    for c in calls:
        fresh()
        entry = {"call": c["call"], "setup": c["setup"]}
        if c["setup"]:
            setup = attempt(mel.eval, c["setup"])
            if isinstance(setup, dict) and "error" in setup:
                entry["setup_error"] = setup["error"]
        if c.get("lang") == "python":
            entry["result"] = attempt(eval, c["call"], {"cmds": cmds})
        else:
            entry["result"] = attempt(mel.eval, c["call"])
        areas.setdefault(c["area"], {})[c["id"]] = entry
    for area, entries in areas.items():
        write("calls/%s.json" % area, entries)


QUERIES = {
    "shortName": dict(shortName=True),
    "longName": dict(longName=True),
    "attributeType": dict(attributeType=True),
    "keyable": dict(keyable=True),
    "channelBox": dict(channelBox=True),
    "readable": dict(readable=True),
    "writable": dict(writable=True),
    "connectable": dict(connectable=True),
    "storable": dict(storable=True),
    "hidden": dict(hidden=True),
    "multi": dict(multi=True),
    "listParent": dict(listParent=True),
    "listChildren": dict(listChildren=True),
    "listDefault": dict(listDefault=True),
    "listEnum": dict(listEnum=True),
    "softRange": dict(softRange=True),
    "range": dict(range=True),
}


def capture_attributes():
    with open(os.path.join(HERE, "node_types.txt")) as f:
        types = [l.strip() for l in f if l.strip()]
    listed = attempt(lambda: cmds.allNodeTypes())
    known = set(listed) if isinstance(listed, list) else set(types)
    write("attributes/_missing_types.json", sorted(t for t in types if t not in known))
    for i, t in enumerate(types):
        if t not in known:
            continue
        if i % 20 == 0:
            fresh()
        info = {"all": attempt(cmds.attributeInfo, type=t, all=True), "inherited": attempt(cmds.nodeType, t, inherited=True, isTypeName=True)}
        node = attempt(cmds.createNode, t, skipSelect=True)
        if isinstance(node, str):
            info["default_name"] = node
            info["keyable"] = attempt(cmds.listAttr, node, keyable=True)
            info["channelBox"] = attempt(cmds.listAttr, node, channelBox=True)
            attrs = {}
            for a in cmds.listAttr(node) or []:
                if "." in a:
                    continue
                row = {k: attempt(cmds.attributeQuery, a, node=node, **q) for k, q in QUERIES.items()}
                row["type"] = attempt(cmds.getAttr, node + "." + a, type=True)
                row["value"] = attempt(cmds.getAttr, node + "." + a)
                attrs[a] = row
            info["attributes"] = attrs
            parents = cmds.listRelatives(node, parent=True) or []
            attempt(cmds.delete, parents or node)
        else:
            info["create_error"] = node
        write("attributes/%s.json" % t, info)


KEYS = (
    [chr(c) for c in range(ord("a"), ord("z") + 1)]
    + [chr(c) for c in range(ord("A"), ord("Z") + 1)]
    + [str(n) for n in range(10)]
    + ["F%d" % n for n in range(1, 13)]
    + ["Up", "Down", "Left", "Right", "Home", "End", "Page_Up", "Page_Down", "Insert", "Return", "Space", "Tab", "Esc", "Delete", "Backspace"]
    + list("`-=[]\\;',./~!@#$%^&*()_+{}|:\"<>?")
)


def capture_commands():
    names = attempt(cmds.runTimeCommand, query=True, defaultCommandArray=True)
    if not isinstance(names, list):
        write("commands/runtime_query_error.json", names)
    user = attempt(cmds.runTimeCommand, query=True, userCommandArray=True)
    rows = {}
    for n in names if isinstance(names, list) else []:
        rows[n] = {
            "annotation": attempt(cmds.runTimeCommand, n, query=True, annotation=True),
            "category": attempt(cmds.runTimeCommand, n, query=True, category=True),
            "language": attempt(cmds.runTimeCommand, n, query=True, commandLanguage=True),
            "command": attempt(cmds.runTimeCommand, n, query=True, command=True),
        }
    write("commands/runtime.json", {"default": rows, "user": user})

    count = attempt(lambda: cmds.assignCommand(query=True, numElements=True))
    named = []
    for i in range(1, count + 1) if isinstance(count, int) else []:
        named.append(
            {
                "name": attempt(cmds.assignCommand, i, query=True, name=True),
                "annotation": attempt(cmds.assignCommand, i, query=True, annotation=True),
                "command": attempt(cmds.assignCommand, i, query=True, command=True),
                "key": attempt(cmds.assignCommand, i, query=True, keyString=True),
                "alt": attempt(cmds.assignCommand, i, query=True, altModifier=True),
                "ctrl": attempt(cmds.assignCommand, i, query=True, ctrlModifier=True),
            }
        )
    write("commands/name_commands.json", {"count": count, "commands": named})

    hot = {}
    for k in KEYS:
        for alt in (False, True):
            for ctl in (False, True):
                for cmd_ in (False, True):
                    mods = dict(altModifier=alt, ctrlModifier=ctl, commandModifier=cmd_)
                    press = attempt(cmds.hotkey, keyShortcut=k, query=True, name=True, **mods)
                    release = attempt(cmds.hotkey, keyShortcut=k, query=True, releaseName=True, **mods)
                    if (press and not isinstance(press, dict)) or (release and not isinstance(release, dict)):
                        tag = "".join(n for n, on in (("alt+", alt), ("ctl+", ctl), ("cmd+", cmd_)) if on) + k
                        hot[tag] = {"press": press, "release": release}
    write("commands/hotkeys.json", {"set": attempt(lambda: cmds.hotkeySet(query=True, current=True)), "keys": hot})


def sample_points(mesh):
    count = cmds.polyEvaluate(mesh, vertex=True)
    step = max(1, count // 8)
    return {str(i): cmds.pointPosition("%s.vtx[%d]" % (mesh, i), world=True) for i in range(0, count, step)}


def capture_scenes():
    with open(os.path.join(HERE, "scenes.json")) as f:
        scenes = json.load(f)["scenes"]
    folder = os.path.join(OUT, "scenes")
    os.makedirs(folder, exist_ok=True)

    fresh()
    cmds.polyCube(name="referencedCube")
    referenced = os.path.join(folder, "referenced.ma").replace("\\", "/")
    cmds.file(rename=referenced)
    cmds.file(save=True, type="mayaAscii")

    for s in scenes:
        path = os.path.join(folder, s["name"] + ".ma").replace("\\", "/")
        fresh()
        built = attempt(mel.eval, s["mel"].replace("@REFERENCED@", referenced))
        report = {"build": built, "units": {}, "frames": {}}
        cmds.file(rename=path)
        cmds.file(save=True, type="mayaAscii", force=True)
        cmds.file(path, open=True, force=True)
        for u, kw in (("linear", dict(linear=True)), ("angle", dict(angle=True)), ("time", dict(time=True))):
            report["units"][u] = cmds.currentUnit(query=True, **kw)
        report["node_types"] = sorted(set(cmds.nodeType(n) for n in cmds.ls(long=True)))
        for frame in s["frames"]:
            cmds.currentTime(frame)
            at = {"matrix": {}, "points": {}}
            for t in cmds.ls(type="transform", long=True) or []:
                at["matrix"][t] = cmds.xform(t, query=True, worldSpace=True, matrix=True)
            for m in cmds.ls(type="mesh", long=True, noIntermediate=True) or []:
                at["points"][m] = sample_points(m)
            report["frames"][str(frame)] = plain(at)
        write("posed/%s.json" % s["name"], report)


PARTS = [("calls", capture_calls), ("attributes", capture_attributes), ("commands", capture_commands), ("scenes", capture_scenes)]


def main(out=None):
    global OUT
    OUT = out or os.path.normpath(os.path.join(HERE, "..", "..", "tests", "data", "reference"))
    only = [p for p in os.environ.get("CAPTURE_ONLY", "").split(",") if p]
    os.makedirs(OUT, exist_ok=True)
    write(
        "meta.json",
        {
            "version": attempt(cmds.about, version=True),
            "api": attempt(cmds.about, apiVersion=True),
            "os": platform.platform(),
            "python": sys.version.split()[0],
        },
    )
    for name, fn in PARTS:
        if only and name not in only:
            continue
        print("capture: " + name)
        try:
            fn()
        except Exception:
            ERRORS[name] = traceback.format_exc()
            print(ERRORS[name])
    write("errors.json", ERRORS)
    print("capture: wrote %s, %d part(s) failed" % (OUT, len(ERRORS)))


if __name__ == "__main__" or "CAPTURE_DIR" in os.environ or "__file__" not in globals():
    main(sys.argv[1] if len(sys.argv) > 1 and __name__ == "__main__" else None)
    if _standalone:
        maya.standalone.uninitialize()
