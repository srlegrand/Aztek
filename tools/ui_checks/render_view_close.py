#!/usr/bin/env python3
"""Check Render View close and reopen under Xvfb. Usage: render_view_close.py build/Aztek/AppRun."""
import os
from pathlib import Path
import subprocess
import sys
import tempfile


app = Path(sys.argv[1]).resolve()
with tempfile.TemporaryDirectory(prefix="aztek_render_view_") as directory:
    work = Path(directory)
    script = work / "check.py"
    script.write_text("""
import os
from traceback import format_exc

from maya import mel
from PySide6.QtTest import QTest
from PySide6.QtWidgets import QApplication, QDockWidget, QMenu, QWidget

app = QApplication.instance()
main = next(w for w in app.topLevelWidgets() if w.objectName() == "MayaWindow")

def reopen():
    mel.eval("RenderViewWindow;")
    QTest.qWait(200)
    dock = main.findChild(QDockWidget, "workspace_renderView")
    view = dock.findChild(QWidget, "haroldRenderView")
    assert dock.isVisible() and view.isVisible(), "Render View did not reopen"
    return dock, view

try:
    dock, view = reopen()
    for floating in (True, False):
        dock.setFloating(floating)
        menus = view.findChildren(QMenu)
        actions = next(menu for menu in menus if menu.title() == "File").actions()
        next(action for action in actions if action.text() == "Close").trigger()
        assert not dock.isVisible(), "File > Close left the Render View dock visible"
        assert main.isVisible(), "File > Close closed the main window"
        dock, view = reopen()
        assert dock.isFloating() == floating, "Reopening changed the dock mode"
    dock.close()
    assert not dock.isVisible(), "Closing the outer dock left it visible"
    reopen()
except Exception:
    os.write(2, format_exc().encode())
    app.exit(1)
else:
    main.close()
""")
    env = dict(os.environ, XDG_CONFIG_HOME=str(work / "config"),
               AZTEK_PREFS=str(work / "prefs"), AZTEK_APP_DIR=str(work / "aztek"))
    subprocess.run([str(app), "--script", str(script)], env=env, check=True, timeout=20)
print("Floating and docked Render View close and reopen passed.")
