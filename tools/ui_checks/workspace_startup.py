#!/usr/bin/env python3
"""Check workspace layouts under Xvfb. Usage: workspace_startup.py build/Aztek/AppRun."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile


app = Path(sys.argv[1]).resolve()
with tempfile.TemporaryDirectory(prefix="aztek_workspace_") as directory:
    work = Path(directory)
    settings = work / "config/aztek/Aztek.conf"
    settings.parent.mkdir(parents=True)
    settings.write_text("[General]\nworkspaces_current=General\n")
    probe = work / "probe.py"
    probe.write_text("""
import json
import os
from pathlib import Path
from PySide6.QtWidgets import QApplication, QComboBox, QDockWidget

window = next(w for w in QApplication.topLevelWidgets() if w.objectName() == "MayaWindow")
uv = window.findChild(QDockWidget, "workspace_uvEditor")
Path(os.environ["AZTEK_WORKSPACE_PROBE"]).write_text(json.dumps([
    window.findChild(QComboBox, "workspaceBox").currentText(),
    window.findChild(QDockWidget, "TimeSlider").isVisible(),
    uv is not None and uv.isVisible(),
]))
window.close()
""")
    result = work / "result.json"
    env = dict(os.environ, XDG_CONFIG_HOME=str(work / "config"), AZTEK_PREFS=str(work / "prefs"),
               AZTEK_APP_DIR=str(work / "aztek"), AZTEK_WORKSPACE_PROBE=str(result))
    cases = [("General", True, False), ("Modeling - Expert", False, False),
             ("UV Editing", False, True), ("UV Editing", False, True), ("General", True, False)]
    for name, timeline, uv in cases:
        # Change the chosen workspace while retaining the previous session's dock state.
        lines = settings.read_text().splitlines()
        settings.write_text("\n".join(f"workspaces_current={name}" if line.startswith("workspaces_current=")
                                      else line for line in lines) + "\n")
        result.unlink(missing_ok=True)
        subprocess.run([str(app), "--script", str(probe)], env=env, check=True, timeout=20)
        observed = json.loads(result.read_text())
        assert observed == [name, timeline, uv], observed
print("Startup workspace and dock restore passed.")
