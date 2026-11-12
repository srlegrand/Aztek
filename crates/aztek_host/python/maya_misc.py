# Small modules of the reference's that tools import at the top of their files. One source, run under each name.
_name = __name__

if _name == "maya.standalone":
    # Aztek's batch mode is ready before a script runs: there is nothing to start or stop.
    def initialize(name="python"):
        pass

    def uninitialize():
        pass

elif _name == "maya.app.flux.core":
    def dpi(value):
        import maya.OpenMayaUI as omui

        return omui.MQtUtil.dpiScale(value)

    def pix(value):
        return int(dpi(value))

elif _name == "maya.internal.nodes.proximitywrap.node_interface":
    class NodeInterface(object):
        def __init__(self, *args, **kwargs):
            raise RuntimeError("The proximity wrap deformer is not in Aztek")

elif _name.startswith("maya.app.renderSetup.model"):
    def __getattr__(attr):
        if attr.startswith("__"):
            raise AttributeError(attr)
        raise RuntimeError("Render Setup's Python model is not in Aztek (%s.%s)" % (_name, attr))
