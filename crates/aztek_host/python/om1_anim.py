# aztek.OpenAztekAnim (maya.OpenMayaAnim): API 1.0 over aztek.api.OpenAztekAnim.

import aztek.api.OpenAztekAnim as _oma2
import aztek.OpenAztek as _om1

_v1, _to1, _put, _Ptr, _out, _arr_fill, _copy = _om1._v1, _om1._to1, _om1._put, _om1._Ptr, _om1._out, _om1._arr_fill, _om1._copy
_MDoubleArray = _om1.MDoubleArray


def _sk_influences(self, arr=None):
    got = _oma2.MFnSkinCluster.influenceObjects(self)
    if arr is None:
        return _to1(got)
    _arr_fill(arr, got)
    return len(got)


def _sk_get_weights(self, path, comp, *args):
    # 1.0: (path, comp, weights, uint* influenceCount), (path, comp, influenceIndex, weights) or
    # (path, comp, MIntArray influenceIndices, weights).
    if not args:
        return _to1(_oma2.MFnSkinCluster.getWeights(self, path, comp))
    if isinstance(args[0], list) and len(args) >= 2 and isinstance(args[1], _Ptr):
        w, n = _oma2.MFnSkinCluster.getWeights(self, path, comp)
        _arr_fill(args[0], w)
        _put(args[1], n)
        return None
    if len(args) >= 2 and isinstance(args[1], list):
        which = int(args[0]) if isinstance(args[0], int) else [int(i) for i in args[0]]
        _arr_fill(args[1], _oma2.MFnSkinCluster.getWeights(self, path, comp, which))
        return None
    return _to1(_oma2.MFnSkinCluster.getWeights(self, path, comp, *args))


def _sk_set_weights(self, path, comp, influences, values, normalize=True, old=None):
    if isinstance(values, (int, float)):
        values = [float(values)]
    got = _oma2.MFnSkinCluster.setWeights(self, path, comp, influences, _om1.MDoubleArray(values), normalize, old is not None)
    if old is not None and got is not None:
        _arr_fill(old, got)


def _sk_affected(self, path, sel=None, weights=None):
    s, w = _oma2.MFnSkinCluster.getPointsAffectedByInfluence(self, path)
    if sel is None:
        return _to1((s, w))
    _om1.MSelectionList.clear(sel)
    _om1._om2.MSelectionList.merge(sel, s)
    _arr_fill(weights, w)


def _sk_geometry(name):
    f = getattr(_oma2.MFnSkinCluster, name)

    def call(self, arr=None):
        return _out(arr, f(self))

    return call


def _sk_blend(self, path, comp, arr=None):
    return _out(arr, _oma2.MFnSkinCluster.getBlendWeights(self, path, comp))


MFnSkinCluster = _v1(
    _oma2.MFnSkinCluster,
    {
        "influenceObjects": _sk_influences,
        "getWeights": _sk_get_weights,
        "setWeights": _sk_set_weights,
        "getPointsAffectedByInfluence": _sk_affected,
        "getOutputGeometry": _sk_geometry("getOutputGeometry"),
        "getInputGeometry": _sk_geometry("getInputGeometry"),
        "getBlendWeights": _sk_blend,
    },
)
MFnSkinCluster.__module__ = "aztek.OpenAztekAnim"
MAnimControl = _v1(_oma2.MAnimControl)
MAnimControl.__module__ = "aztek.OpenAztekAnim"


def _ac_find(self, t, ptr=None):
    i = _oma2.MFnAnimCurve.find(self, t)
    if ptr is None:
        return i
    if i is None:
        return False
    _put(ptr, i)
    return True


_curve_type = _oma2.MFnAnimCurve.animCurveType.fget
MFnAnimCurve = _v1(
    _oma2.MFnAnimCurve,
    {
        "find": _ac_find,
        "isTimeInput": lambda self: _curve_type(self) <= 3,
        "isUnitlessInput": lambda self: _curve_type(self) > 3,
    },
)
MFnAnimCurve.__module__ = "aztek.OpenAztekAnim"

__all__ = ["MFnSkinCluster", "MAnimControl", "MFnAnimCurve"]
