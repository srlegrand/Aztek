"""A scriptJob that prints the selection whenever it changes. Run it again to stop it."""
from aztek import cmds

job = cmds.optionVar(q="exSelectionWatchJob") if cmds.optionVar(exists="exSelectionWatchJob") else 0
if job and cmds.scriptJob(exists=job):
    cmds.scriptJob(kill=job, force=True)
    cmds.optionVar(remove="exSelectionWatchJob")
    print("Selection watcher stopped.")
else:
    def watch():
        sel = cmds.ls(sl=True)
        print("Selected: %s" % (", ".join(sel) if sel else "nothing"))
    cmds.optionVar(iv=("exSelectionWatchJob", cmds.scriptJob(event=["SelectionChanged", watch])))
    print("Selection watcher started. Run this script again to stop it.")
