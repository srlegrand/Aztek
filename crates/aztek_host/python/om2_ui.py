# aztek.api.OpenAztekUI (maya.api.OpenMayaUI): the reference's 2.0 interface module. MPxLocatorNode and M3dView work;
# MPxSurfaceShapeUI and the rest are thin (om_thin): their names are here, what they do mostly is not.
import aztek.api.OpenAztek as _om
import aztek.OpenAztekUI as _ui1

MPxNode = _om.MPxNode


class MPxLocatorNode(_om.MPxNode):
    """A locator of a plug-in's own: a DAG shape with a position and a scale. Aztek draws it as a locator; draw() is
    not called. Its bounds come from boundingBox() when isBounded() says so."""

    def isBounded(self):
        return False

    def boundingBox(self):
        return _om.MBoundingBox()

    def draw(self, view, path, style, status):
        pass

    def color(self, status):
        return _om.MColor()

    def colorRGB(self, status):
        return _om.MColor()

    def drawLast(self):
        return False

    def isTransparent(self):
        return False

    def excludeAsLocator(self):
        return True

    def useClosestPointForSelection(self):
        return False

    def closestPoint(self, cursorRay, rayDirection):
        return _om.MPoint()


for _n in (
    "underWorldObject", "localPosition", "localPositionX", "localPositionY", "localPositionZ", "worldPosition",
    "worldPositionX", "worldPositionY", "worldPositionZ", "localScale", "localScaleX", "localScaleY", "localScaleZ",
    "instObjGroups", "intermediateObject", "inverseMatrix", "isTemplated", "matrix", "objectColor", "parentInverseMatrix",
    "parentMatrix", "visibility", "worldInverseMatrix", "worldMatrix", "center", "boundingBoxCenterX",
    "boundingBoxCenterY", "boundingBoxCenterZ",
):
    setattr(MPxLocatorNode, _n, _om._builtin_attr(_n))
MPxLocatorNode.nodeBoundingBox = _om._builtin_attr("boundingBox")
for _a in ("Min", "Max", "Size"):
    setattr(MPxLocatorNode, "nodeBoundingBox" + _a, _om._builtin_attr("boundingBox" + _a))
    for _x in "XYZ":
        setattr(MPxLocatorNode, "nodeBoundingBox" + _a + _x, _om._builtin_attr("boundingBox" + _a + _x))
del _n, _a, _x


class M3dView(_ui1.M3dView, _Thin):
    """A view: what can be answered from commands is here; the rest raises when called."""

    @staticmethod
    def active3dView():
        return M3dView()

    @staticmethod
    def numberOf3dViews():
        import maya.cmds as cmds

        return len(cmds.getPanel(type="modelPanel") or [])

    @staticmethod
    def get3dView(index):
        import maya.cmds as cmds

        panels = cmds.getPanel(type="modelPanel") or []
        if not 0 <= index < len(panels):
            raise RuntimeError("(kInvalidParameter): No view at that index")
        v = M3dView()
        v._fixed = panels[index]
        return v

    @staticmethod
    def getM3dViewFromModelPanel(panel):
        v = M3dView()
        v._fixed = panel
        return v

    getM3dViewFromModelEditor = getM3dViewFromModelPanel

    def _panel(self):
        fixed = self.__dict__.get("_fixed")
        return fixed if fixed else _ui1.M3dView._panel(self)

    def getCamera(self):
        import maya.cmds as cmds

        cam = cmds.modelPanel(self._panel(), query=True, camera=True)
        sl = _om.MSelectionList()
        sl.add(cam)
        path = sl.getDagPath(0)
        if path.hasFn(_om.MFn.kTransform):
            try:
                path.extendToShape()
            except RuntimeError:
                pass
        return path


class MPxSurfaceShapeUI(_Thin):
    """A plug-in shape's drawing and selection in the viewport. Aztek doesn't call it; registerShape keeps the creator."""

    def __init__(self, *args, **kwargs):
        pass

    def surfaceShape(self):
        raise NotImplementedError("MPxSurfaceShapeUI.surfaceShape is not in Aztek yet")


_thin_classes(
    [
        "MCursor", "MDrawData", "MDrawInfo", "MDrawProperties", "MDrawRequest", "MEvent", "MFnCircleSweepManip",
        "MFnCurveSegmentManip", "MFnDirectionManip", "MFnDiscManip", "MFnDistanceManip", "MFnFreePointTriadManip",
        "MFnManip3D", "MFnPointOnCurveManip", "MFnPointOnSurfaceManip", "MFnRotateManip", "MFnScaleManip",
        "MFnStateManip", "MFnToggleManip", "MHWShaderSwatchGenerator", "MManipData", "MMaterial", "MMaterialArray",
        "MPaintMessage", "MPanelCanvas", "MPanelCanvasInfo", "MPxContext", "MPxContextCommand",
        "MPxDragAndDropBehavior", "MPxHardwareShader", "MPxHwShaderNode", "MPxManipContainer", "MPxManipulatorNode",
        "MPxSelectionContext", "MPxToolCommand", "MSelectInfo", "MSequencerCustomDrawManager",
        "MSequencerDrawPrimitive", "MTextureEditorDrawInfo", "MTimeSliderCustomDrawManager", "MTimeSliderDrawPrimitive",
        "MUiMessage", "RenderParameters", "ShaderContext",
    ],
    globals(),
)
