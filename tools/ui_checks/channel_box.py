#!/usr/bin/env python3
"""The Channel Box's multi-edit, by mouse and keyboard, on a real X server. Usage: channel_box.py build/aztek [shots dir]

Needs Xvfb, xdotool, ImageMagick's import and a window manager (openbox, or kwin_x11), without which Qt never takes the
window as active and keys go to the hotkeys. Exits 0 when every step gives what it should.
"""
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time

BIN = os.path.abspath(sys.argv[1])
SHOTS = sys.argv[2] if len(sys.argv) > 2 else None
HERE = os.path.dirname(os.path.abspath(__file__))
DISPLAY = ":%d" % (90 + os.getpid() % 9)
work = tempfile.mkdtemp(prefix="aztek_cb_")
state_file = os.path.join(work, "state.json")
env = dict(os.environ, DISPLAY=DISPLAY, CHECK_STATE=state_file, XDG_CONFIG_HOME=work, HOME=work, QT_QPA_PLATFORM="xcb", AZTEK_ECHO="1")

# Where the rows are on a fresh layout at this screen size: the labels' and the fields' x, each row's y.
LABEL_X, FIELD_X = 1348, 1470
ROW = {"tx": 207, "ty": 226, "tz": 245, "rx": 264, "ry": 283, "rz": 302, "sx": 321, "sy": 340, "sz": 359}


def xdo(*args):
    subprocess.run(["xdotool", *map(str, args)], env=env, check=True)


def move(x, y):
    xdo("mousemove", "--window", WINDOW, x, y)


def click(x, y, mods=()):
    move(x, y)
    time.sleep(0.15)
    for m in mods:
        xdo("keydown", m)
    xdo("click", 1)
    for m in mods:
        xdo("keyup", m)
    time.sleep(0.35)


def state():
    time.sleep(0.8)
    with open(state_file) as f:
        return json.load(f)


def shot(name):
    if SHOTS:
        os.makedirs(SHOTS, exist_ok=True)
        subprocess.run(["import", "-window", "root", os.path.join(SHOTS, name + ".png")], env=env)


def type_in(row, text):
    """A click gives the field the keyboard; what's typed replaces its text, and Return commits."""
    move(FIELD_X, ROW[row])
    time.sleep(0.15)
    xdo("click", 1)
    time.sleep(0.3)
    xdo("key", "ctrl+a")
    xdo("type", "--delay", 40, text)
    xdo("key", "Return")
    time.sleep(0.5)


failures = []
WINDOW = None


def check(what, got, want):
    ok = all(abs(a - b) < 1e-3 for a, b in zip(got, want)) and len(got) == len(want)
    print(("ok   " if ok else "FAIL ") + what + ("" if ok else ": got %s, want %s" % (got, want)))
    if not ok:
        failures.append(what)


def scales(s):
    return s["pCube1"][:3] + s["pCube2"][:3]


def translates(s):
    return s["pCube1"][3:] + s["pCube2"][3:]


xvfb = subprocess.Popen(["Xvfb", DISPLAY, "-screen", "0", "1700x1050x24"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
time.sleep(1)
wm_name = shutil.which("openbox") or shutil.which("kwin_x11")
if not wm_name:
    raise SystemExit("no window manager: install openbox")
wm = subprocess.Popen([wm_name], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
time.sleep(2)
log = open(os.path.join(work, "app.log"), "w")
app = subprocess.Popen([BIN, "--script", os.path.join(HERE, "channel_box_setup.py")], env=env, stdout=log, stderr=log)
try:
    for _ in range(120):
        if os.path.exists(state_file):
            break
        time.sleep(0.5)
    else:
        raise SystemExit("the app never wrote its state: see " + log.name)
    time.sleep(3)
    # The main window, active.
    wid = subprocess.run(["xdotool", "search", "--onlyvisible", "--name", "Aztek"], env=env, capture_output=True, text=True).stdout.split()
    WINDOW = wid[0]
    xdo("windowactivate", "--sync", WINDOW)
    time.sleep(0.5)
    shot("0_start")

    # Scale X picked, Shift+click on Scale Z picks the run.
    click(LABEL_X, ROW["sx"])
    click(LABEL_X, ROW["sz"], ["shift"])
    shot("1_picked")
    type_in("sy", "2.5")
    check("2.5 typed in Scale Y reaches all three on both cubes", scales(state()), [2.5] * 6)
    shot("2_typed")

    type_in("sy", "+=1")
    check("+=1 adds to all three on both cubes", scales(state()), [3.5] * 6)

    # One undo takes the whole edit back.
    move(600, 500)
    time.sleep(0.2)
    xdo("key", "z")
    check("one undo takes both cubes back to 2.5", scales(state()), [2.5] * 6)
    shot("3_undone")

    # A drag down the names picks a run.
    move(LABEL_X, ROW["tx"])
    time.sleep(0.2)
    xdo("mousedown", 1)
    for y in range(ROW["tx"], ROW["tz"] + 1, 4):
        move(LABEL_X, y)
        time.sleep(0.03)
    xdo("mouseup", 1)
    time.sleep(0.4)
    shot("4_dragged")
    type_in("ty", "1.5")
    check("a drag down the names picks a run", translates(state()), [1.5] * 6)

    # Ctrl+click takes one out.
    click(LABEL_X, ROW["ty"], ["ctrl"])
    shot("5_toggled")
    type_in("tx", "4")
    s = state()
    check("Ctrl+click leaves Translate Y out", translates(s), [4, 1.5, 4, 4, 1.5, 4])
finally:
    app.terminate()
    try:
        app.wait(5)
    except subprocess.TimeoutExpired:
        app.kill()
    wm.terminate()
    xvfb.terminate()
    log.close()
    if failures:
        print(open(log.name).read()[-1500:])
    shutil.rmtree(work, ignore_errors=True)
sys.exit(1 if failures else 0)
