# aztek.api.OpenMayaAnim: skin weights, the current time and animation curves.
import sys as _sys

from aztek.api.OpenMaya import (
    MDagPath,
    MDagPathArray,
    MDoubleArray,
    MFn,
    MFnDependencyNode,
    MIntArray,
    MObject,
    MPlug,
    MTime,
    MTimeArray,
    _api,
    _check_node,
    _fail,
    _mesh_data,
    _node_id_named,
    _node_object,
)

# The API's commands: in stored units, as its values are.
from aztek.api.OpenMaya import _cmds


class MFnSkinCluster(MFnDependencyNode):
    def _check(self, obj):
        if not obj.hasFn(MFn.kSkinClusterFilter):
            _fail()

    def _data(self):
        return _api("skin.data", id=self._id())

    def influenceObjects(self):
        return MDagPathArray([MDagPath._of(j) for _, j in self._data()["influences"]])

    def indexForInfluenceObject(self, path):
        leaf = path._leaf()
        for i, j in self._data()["influences"]:
            if j == leaf:
                return i
        _fail(status="kFailure", message="Object is not an influence of this skin")

    def _positions(self):
        return [i for i, _ in self._data()["influences"]]

    def getWeights(self, path, components, influence=None):
        """getWeights(shape, comps) gives (weights, influences per point); with an influence, or a list of them, just those."""
        d = self._data()
        logical = [i for i, _ in d["influences"]]
        if influence is None:
            wanted = logical
        elif isinstance(influence, int):
            wanted = [influence]
        else:
            wanted = [int(i) for i in influence]
        rows = d["weights"]
        ids = self._points(path, components, len(rows))
        out = MDoubleArray()
        for p in ids:
            pairs = rows[p] if p < len(rows) else []
            have = {int(pairs[k]): pairs[k + 1] for k in range(0, len(pairs), 2)}
            for i in wanted:
                list.append(out, have.get(i, 0.0))
        if influence is None:
            return (out, len(wanted))
        return out

    def _points(self, path, components, count):
        if components is None or components.isNull():
            return list(range(count))
        return _mesh_data(path._leaf()).compact(MFn.kMeshVertComponent, components._comp)

    def setWeights(self, path, components, influences, weights, normalize=True, returnOldWeights=False):
        d = self._data()
        logical = {i: j for i, j in d["influences"]}
        infs = [influences] if isinstance(influences, int) else [int(i) for i in influences]
        names = {}
        for i in infs:
            if i not in logical:
                _fail(message="No influence at index %d" % (i,))
            names[i] = _api("node.info", id=logical[i])["path"]
        ids = self._points(path, components, len(d["weights"]))
        vals = list(weights)
        n = len(infs)
        if len(vals) == 1 and len(ids) > 1:
            vals = vals * (len(ids) * n)
        if len(vals) != len(ids) * n:
            _fail(message="Expected %d weights, got %d" % (len(ids) * n, len(vals)))
        old = MDoubleArray()
        if returnOldWeights:
            for p in ids:
                pairs = d["weights"][p] if p < len(d["weights"]) else []
                have = {int(pairs[k]): pairs[k + 1] for k in range(0, len(pairs), 2)}
                for i in infs:
                    list.append(old, have.get(i, 0.0))
        skin = self._nm()
        owner = _api("node.info", id=path._leaf())["path"]
        for k, p in enumerate(ids):
            tv = []
            for j, i in enumerate(infs):
                tv.extend([names[i], float(vals[k * n + j])])
            _cmds.skinPercent(skin, "%s.vtx[%d]" % (owner, p), transformValue=tv, normalize=bool(normalize))
        return old if returnOldWeights else None

    def getPointsAffectedByInfluence(self, path):
        from aztek.api.OpenMaya import MSelectionList, MFnSingleIndexedComponent

        d = self._data()
        logical = self.indexForInfluenceObject(path)
        geo = d["geometry"]
        ids = []
        wts = MDoubleArray()
        for p, pairs in enumerate(d["weights"]):
            have = {int(pairs[k]): pairs[k + 1] for k in range(0, len(pairs), 2)}
            w = have.get(logical, 0.0)
            if w > 0.0:
                ids.append(p)
                list.append(wts, w)
        fn = MFnSingleIndexedComponent()
        comp = fn.create(MFn.kMeshVertComponent)
        fn.addElements(ids)
        sel = MSelectionList()
        sel.add((MDagPath._of(geo), comp))
        return (sel, wts)

    def getOutputGeometry(self):
        from aztek.api.OpenMaya import MObjectArray

        geo = self._data()["geometry"]
        return MObjectArray([_node_object(geo)] if geo else [])

    def getInputGeometry(self):
        return self.getOutputGeometry()

    def deformerSet(self):
        return MObject()

    def getBlendWeights(self, path, components):
        n = len(self._points(path, components, len(self._data()["weights"])))
        return MDoubleArray([0.0] * n)


def _frame(v):
    """Playback times travel in seconds here: this gives back the frame that was set."""
    return MTime(round(float(v), 6))


class MAnimControl:
    kPlayOnce = 0
    kPlayLoop = 1
    kPlayOscillate = 2
    _playing = False

    @staticmethod
    def currentTime():
        return _frame(_cmds.currentTime(q=True))

    @staticmethod
    def setCurrentTime(t):
        _cmds.currentTime(t.asUnits(MTime.uiUnit()))

    @staticmethod
    def minTime():
        return _frame(_cmds.playbackOptions(q=True, minTime=True))

    @staticmethod
    def maxTime():
        return _frame(_cmds.playbackOptions(q=True, maxTime=True))

    @staticmethod
    def setMinTime(t):
        _cmds.playbackOptions(minTime=t.asUnits(MTime.uiUnit()))

    @staticmethod
    def setMaxTime(t):
        _cmds.playbackOptions(maxTime=t.asUnits(MTime.uiUnit()))

    @staticmethod
    def setMinMaxTime(a, b):
        MAnimControl.setMinTime(a)
        MAnimControl.setMaxTime(b)

    @staticmethod
    def animationStartTime():
        return _frame(_cmds.playbackOptions(q=True, animationStartTime=True))

    @staticmethod
    def animationEndTime():
        return _frame(_cmds.playbackOptions(q=True, animationEndTime=True))

    @staticmethod
    def setAnimationStartTime(t):
        _cmds.playbackOptions(animationStartTime=t.asUnits(MTime.uiUnit()))

    @staticmethod
    def setAnimationEndTime(t):
        _cmds.playbackOptions(animationEndTime=t.asUnits(MTime.uiUnit()))

    @staticmethod
    def setAnimationStartEndTime(a, b):
        MAnimControl.setAnimationStartTime(a)
        MAnimControl.setAnimationEndTime(b)

    @staticmethod
    def playbackSpeed():
        return float(_cmds.playbackOptions(q=True, playbackSpeed=True))

    @staticmethod
    def setPlaybackSpeed(v):
        _cmds.playbackOptions(playbackSpeed=float(v))

    @staticmethod
    def isPlaying():
        return False

    @staticmethod
    def isScrubbing():
        return False


class MFnAnimCurve(MFnDependencyNode):
    kAnimCurveTA = 0
    kAnimCurveTL = 1
    kAnimCurveTT = 2
    kAnimCurveTU = 3
    kAnimCurveUA = 4
    kAnimCurveUL = 5
    kAnimCurveUT = 6
    kAnimCurveUU = 7
    kAnimCurveUnknown = 8
    kTangentGlobal = 0
    kTangentFixed = 1
    kTangentLinear = 2
    kTangentFlat = 3
    kTangentSmooth = 4
    kTangentStep = 5
    kTangentSlow = 6
    kTangentFast = 7
    kTangentClamped = 8
    kTangentPlateau = 9
    kTangentStepNext = 10
    kTangentAuto = 11
    kConstant = 0
    kLinear = 1
    kCycle = 3
    kCycleRelative = 4
    kOscillate = 5

    _TANGENTS = {"fixed": 1, "linear": 2, "flat": 3, "spline": 4, "smooth": 4, "step": 5, "slow": 6, "fast": 7, "clamped": 8, "plateau": 9, "stepnext": 10, "auto": 11}
    _TANGENT_NAMES = {1: "fixed", 2: "linear", 3: "flat", 4: "spline", 5: "step", 6: "slow", 7: "fast", 8: "clamped", 9: "plateau", 10: "stepnext", 11: "auto"}

    def __init__(self, obj=None):
        self._target = None
        MFnDependencyNode.__init__(self)
        if isinstance(obj, MPlug):
            self._target = MPlug(obj)
            src = obj.source()
            if src.isNull() or not src.node().hasFn(MFn.kAnimCurve):
                _fail()
            self._obj = src.node()
        elif obj is not None:
            self.setObject(obj)

    def _check(self, obj):
        if not obj.hasFn(MFn.kAnimCurve):
            _fail()

    def create(self, plug, animCurveType=None, modifier=None):
        """A curve driving the plug. It appears with its first key, as curves do here."""
        self._target = MPlug(plug)
        self._obj = MObject()
        src = plug.source()
        if not src.isNull() and src.node().hasFn(MFn.kAnimCurve):
            self._obj = src.node()
        return self.object()

    def _curve(self):
        return self._nm()

    def _driven(self):
        """The plug this curve drives: keys are set through it."""
        if self._target is not None:
            return self._target
        for src, dst in _api("node.connections", id=self._id()):
            if _node_id_named(src.split(".", 1)[0]) == self._id():
                return MPlug._named(dst)
        return None

    def _times(self):
        return _cmds.keyframe(self._curve(), q=True, timeChange=True) or []

    def _values(self):
        return _cmds.keyframe(self._curve(), q=True, valueChange=True) or []

    def _is_angle(self):
        return self._obj.hasFn(MFn.kAnimCurveTimeToAngular) or self._obj.hasFn(MFn.kAnimCurveUnitlessToAngular)

    def _in(self, v):
        import math

        return float(v)

    def _out(self, v):
        import math

        return float(v)

    @property
    def animCurveType(self):
        t = self.object().apiType()
        return {
            MFn.kAnimCurveTimeToAngular: 0,
            MFn.kAnimCurveTimeToDistance: 1,
            MFn.kAnimCurveTimeToTime: 2,
            MFn.kAnimCurveTimeToUnitless: 3,
            MFn.kAnimCurveUnitlessToAngular: 4,
            MFn.kAnimCurveUnitlessToDistance: 5,
            MFn.kAnimCurveUnitlessToTime: 6,
            MFn.kAnimCurveUnitlessToUnitless: 7,
        }.get(t, 8)

    @property
    def numKeys(self):
        from aztek.api.OpenMaya import _CInt

        return _CInt(len(self._times()))

    def numKeyframes(self):
        return len(self._times())

    def input(self, index):
        ts = self._times()
        if not 0 <= index < len(ts):
            raise IndexError("MFnAnimCurve key index out of range")
        return MTime(float(ts[index]))

    time = input

    def value(self, index):
        vs = self._values()
        if not 0 <= index < len(vs):
            raise IndexError("MFnAnimCurve key index out of range")
        return self._out(vs[index])

    def evaluate(self, t):
        """The curve's value at a time."""
        frame = t.asUnits(MTime.uiUnit()) if isinstance(t, MTime) else float(t)
        return self._out(_api("anim.eval", id=self._id(), frame=frame))

    def find(self, t):
        ts = self._times()
        frame = t.asUnits(MTime.uiUnit())
        for i, x in enumerate(ts):
            if abs(x - frame) < 1e-6:
                return i
        return None

    def findClosest(self, t):
        ts = self._times()
        if not ts:
            _fail(status="kFailure", message="Curve has no keys")
        frame = t.asUnits(MTime.uiUnit())
        return min(range(len(ts)), key=lambda i: abs(ts[i] - frame))

    def addKey(self, t, value, tangentInType=kTangentGlobal, tangentOutType=kTangentGlobal, change=None):
        frame = t.asUnits(MTime.uiUnit())
        kw = {"time": frame}
        if tangentInType in self._TANGENT_NAMES:
            kw["inTangentType"] = self._TANGENT_NAMES[tangentInType]
        if tangentOutType in self._TANGENT_NAMES:
            kw["outTangentType"] = self._TANGENT_NAMES[tangentOutType]
        p = self._driven()
        if p is None:
            _fail(message="Curve drives no plug: call create first")
        kw["value"] = _math_plug_value(p, value)
        _cmds.setKeyframe(p._nodename(), attribute=p._attr, **kw)
        if self._obj.isNull():
            self._obj = p.source().node()
        return self.find(t)

    def addKeys(self, times, values, tangentInType=kTangentGlobal, tangentOutType=kTangentGlobal, keepExistingKeys=False, change=None):
        for t, v in zip(times, values):
            self.addKey(t, v, tangentInType, tangentOutType)
        return self

    def remove(self, index, change=None):
        frame = self._times()[index]
        _cmds.cutKey(self._curve(), time=(frame, frame), clear=True)
        return self

    def setValue(self, index, value, change=None):
        frame = self._times()[index]
        _cmds.keyframe(self._curve(), edit=True, time=(frame, frame), valueChange=self._in(value), absolute=True)
        return self

    def setInput(self, index, t, change=None):
        frame = self._times()[index]
        _cmds.keyframe(self._curve(), edit=True, time=(frame, frame), timeChange=t.asUnits(MTime.uiUnit()), absolute=True)
        return self

    setTime = setInput

    def inTangentType(self, index):
        frame = self._times()[index]
        return self._TANGENTS.get(_cmds.keyTangent(self._curve(), q=True, time=(frame, frame), inTangentType=True)[0], self.kTangentGlobal)

    def outTangentType(self, index):
        frame = self._times()[index]
        return self._TANGENTS.get(_cmds.keyTangent(self._curve(), q=True, time=(frame, frame), outTangentType=True)[0], self.kTangentGlobal)

    @property
    def isWeighted(self):
        return False

    @property
    def isTimeInput(self):
        return self.animCurveType <= 3

    @property
    def isUnitlessInput(self):
        return self.animCurveType > 3


def _math_plug_value(plug, value):
    import math

    return float(value)
