# maya.OpenMayaUI: where interface tools get the main window and find script-built controls, so their Qt windows
# belong to the application. What is here is what tools ask for; the rest of the reference's module is not.
import _aztek


def _binding():
    """The Qt binding in this interpreter and its shiboken, or (None, None). It has to be built against the Qt Aztek
    runs on: two copies of Qt in one process do not work together."""
    for qt, shib in (("PySide6", "shiboken6"), ("PySide2", "shiboken2")):
        try:
            widgets = __import__(qt + ".QtWidgets", fromlist=["QtWidgets"])
            return widgets, __import__(shib)
        except Exception:
            continue
    return None, None


def _pointer(widget):
    widgets, shib = _binding()
    if widget is None or shib is None:
        return None
    return int(shib.getCppPointer(widget)[0])


def _find(name, kinds=None):
    """A widget by the name a command gave it: its full path or its last part."""
    widgets, shib = _binding()
    if widgets is None or not name:
        return None
    app = widgets.QApplication.instance()
    if app is None:
        return None
    short = str(name).split("|")[-1]
    for w in app.allWidgets():
        if w.objectName() in (name, short) and (kinds is None or isinstance(w, kinds)):
            return _pointer(w)
    return None


class MQtUtil(object):
    """Pointers to the application's Qt objects, for shiboken's wrapInstance."""

    @staticmethod
    def mainWindow():
        p = _aztek.main_window()
        return p if p else None

    @staticmethod
    def findControl(name, parent=None):
        return _find(name)

    @staticmethod
    def findLayout(name, parent=None):
        return _find(name)

    @staticmethod
    def findWindow(name):
        return _find(name)

    @staticmethod
    def findMenuItem(name):
        widgets, shib = _binding()
        if widgets is None or not name:
            return None
        app = widgets.QApplication.instance()
        short = str(name).split("|")[-1]
        for w in app.allWidgets() if app else []:
            for a in w.actions():
                if a.objectName() in (name, short):
                    return int(shib.getCppPointer(a)[0])
        return None

    @staticmethod
    def fullName(pointer):
        widgets, shib = _binding()
        if widgets is None or not pointer:
            return ""
        w = shib.wrapInstance(int(pointer), widgets.QWidget)
        names = []
        while w is not None:
            if w.objectName():
                names.insert(0, w.objectName())
            w = w.parentWidget()
        return "|".join(names)

    @staticmethod
    def getCurrentParent():
        import maya.cmds as cmds

        try:
            return _find(cmds.setParent(query=True))
        except Exception:
            return None

    @staticmethod
    def getParent(pointer):
        widgets, shib = _binding()
        if widgets is None or not pointer:
            return None
        return _pointer(shib.wrapInstance(int(pointer), widgets.QWidget).parentWidget())

    @staticmethod
    def dpiScale(size):
        widgets, shib = _binding()
        app = widgets.QApplication.instance() if widgets else None
        if app is None:
            return size
        return size * app.primaryScreen().logicalDotsPerInch() / 96.0


class M3dView(object):
    """A view. Only what can be answered from commands is here."""

    @staticmethod
    def active3dView():
        return M3dView()

    def _panel(self):
        import maya.cmds as cmds

        p = cmds.getPanel(withFocus=True)
        return p if p and cmds.getPanel(typeOf=p) == "modelPanel" else (cmds.getPanel(type="modelPanel") or [None])[0]

    def portWidth(self):
        import maya.cmds as cmds

        return int(cmds.control(self._panel(), query=True, width=True))

    def portHeight(self):
        import maya.cmds as cmds

        return int(cmds.control(self._panel(), query=True, height=True))

    def widget(self):
        return _find(self._panel())

    def refresh(self, *args, **kwargs):
        import maya.cmds as cmds

        cmds.refresh()

    def viewToWorld(self, *args):
        raise NotImplementedError("M3dView.viewToWorld is not in Aztek yet")

    def worldToView(self, *args):
        raise NotImplementedError("M3dView.worldToView is not in Aztek yet")


class MExternalDropCallback(object):
    """Files dropped on the application from outside. Callbacks are kept; Aztek does not call them yet."""

    kMayaDefault = 0
    kNoMayaDefaultAndAccept = 1
    kNoMayaDefaultAndNoAccept = 2
    _callbacks = []

    def __init__(self):
        pass

    @staticmethod
    def addCallback(callback):
        MExternalDropCallback._callbacks.append(callback)

    @staticmethod
    def removeCallback(callback):
        if callback in MExternalDropCallback._callbacks:
            MExternalDropCallback._callbacks.remove(callback)

    def externalDropCallback(self, doDrop, controlName, data):
        return MExternalDropCallback.kMayaDefault
