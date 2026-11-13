# Sets up the aztek package (aztek.cmds, aztek.mel), the compatibility modules and the script editor's output.
import json
import sys
import types

import _aztek


class _Out:
    def __init__(self, kind):
        self.kind = kind
        self.buf = ""

    def write(self, text):
        self.buf += text
        while "\n" in self.buf:
            line, self.buf = self.buf.split("\n", 1)
            _aztek.write(line, self.kind)
        return len(text)

    def flush(self):
        if self.buf:
            _aztek.write(self.buf, self.kind)
            self.buf = ""


sys.stdout = _Out(3)
sys.stderr = _Out(2)


# Run before each command: lets aztek.api write out point edits it holds back.
_before_call = []
_plain_arg_commands = set()


# Callables given to commands (button commands, scriptJobs) stay here; the core holds their numbers.
_callbacks = {}
_callback_ids = {}


def _keep_callable(fn):
    # The same function given twice is one entry; partials and lambdas each get their own.
    key = id(fn)
    cid = _callback_ids.get(key)
    if cid is None or _callbacks.get(cid) is not fn:
        cid = len(_callbacks) + 1
        _callbacks[cid] = fn
        _callback_ids[key] = cid
    return cid


def _plain(v):
    if isinstance(v, (list, tuple)):
        return [_plain(x) for x in v]
    if isinstance(v, dict):
        return {k: _plain(x) for k, x in v.items()}
    if callable(v):
        return {"__pycb__": _keep_callable(v)}
    if v is None or isinstance(v, (str, bytes, bool, int, float)):
        return v
    # A vector, a point, a matrix, an array: anything that can be gone through goes as a list of its numbers.
    try:
        return [_plain(x) for x in v]
    except TypeError:
        pass
    # Anything else goes as its text, as the reference takes a node object for its name.
    return str(v)


def _unplain(v):
    if isinstance(v, list):
        return [_unplain(x) for x in v]
    if isinstance(v, dict):
        if "__pycb__" in v:
            return _callbacks.get(v["__pycb__"])
        if "__pysrc__" in v:
            return v["__pysrc__"]
    return v


def _command(name):
    def run(*args, **kwargs):
        flat = []
        for a in args:
            # The reference takes lists of names wherever it takes names.
            if isinstance(a, (list, tuple)) and all(isinstance(x, str) for x in a):
                flat.extend(a)
            else:
                flat.append(a)
        if _before_call:
            _before_call[0]()
        if name in _plain_arg_commands:
            # A plug-in command with no syntax takes its flags as plain arguments.
            for k, v in kwargs.items():
                flat.append("-" + k)
                flat.extend(v if isinstance(v, (list, tuple)) else [v])
            kwargs = {}
        out = _aztek.call(name, json.dumps(_plain(flat)), json.dumps(_plain(kwargs)))
        result = json.loads(out)
        if isinstance(result, dict) and "__error__" in result:
            raise RuntimeError(result["__error__"])
        if '"__py' in out:
            result = _unplain(result)
        # The reference returns compound attributes as a list of one tuple; a multi attribute of as many elements stays a list.
        if name == "getAttr" and isinstance(result, list) and len(result) in (2, 3, 4) and all(isinstance(x, (int, float)) for x in result) and not (kwargs or {}).get("mi", (kwargs or {}).get("multiIndices")):
            size = json.loads(_aztek.call("getAttr", json.dumps(_plain(flat)), json.dumps({"size": True})))
            if size == 1:
                return [tuple(result)]
        # The reference's list queries give None rather than an empty list.
        if name in ("listRelatives", "listConnections", "listAttr") and result == []:
            return None
        return result

    run.__name__ = name
    run.__doc__ = _aztek.doc(name)
    return run


class _Cmds(types.ModuleType):
    """aztek.cmds: every Aztek command as a function. Flags are keyword arguments, long or short names."""

    def __getattr__(self, name):
        if name.startswith("__"):
            raise AttributeError(name)
        if not _aztek.exists(name):
            raise AttributeError("aztek.cmds has no command %r" % name)
        f = _command(name)
        setattr(self, name, f)
        return f

    def __dir__(self):
        return sorted(json.loads(_aztek.names()))


def _mel_eval(src):
    result = json.loads(_aztek.mel(src))
    if isinstance(result, dict) and "__error__" in result:
        raise RuntimeError(result["__error__"])
    return result


aztek = types.ModuleType("aztek")
aztek.__path__ = []
cmds = _Cmds("aztek.cmds")
mel = types.ModuleType("aztek.mel")
mel.eval = _mel_eval
aztek.cmds = cmds
aztek.mel = mel
sys.modules["aztek"] = aztek
sys.modules["aztek.cmds"] = cmds
sys.modules["aztek.mel"] = mel


def _message(args):
    return " ".join(str(a) for a in args)


def _warning(*args, **kwargs):
    _aztek.write("# Warning: %s #" % _message(args), 4)


def _error(*args, **kwargs):
    raise RuntimeError(_message(args))


def _print(*args, **kwargs):
    sys.stdout.write(_message(args))


def _clear(array):
    # MEL's clear on a Python list: empties it in place and gives how many it held.
    held = len(array)
    del array[:]
    return held


def _get_proc_arguments(name):
    return _mel_eval('getProcArguments "%s"' % name)


def _what_is(name):
    return _mel_eval('whatIs "%s"' % name)


def _eval_deferred(*args, **kwargs):
    # MEL's evalDeferred is the interpreter's; from Python it waits for the next idle step.
    if not args and not kwargs.get("ls", kwargs.get("list")):
        raise RuntimeError("evalDeferred: give a script.")
    src = args[0] if len(args) == 1 else " ".join(str(a) for a in args)
    return _command("deferScript")(*([src] if args else []), **kwargs)


cmds.warning = _warning
cmds.error = _error
setattr(cmds, "print", _print)
cmds.whatIs = _what_is
cmds.clear = _clear
cmds.getProcArguments = _get_proc_arguments
cmds.evalDeferred = _eval_deferred


def _execute_deferred(fn, *args, **kwargs):
    import functools

    if args or kwargs:
        fn = functools.partial(fn, *args, **kwargs)
    _eval_deferred(fn)


def _in_main_thread(fn, *args, **kwargs):
    return fn(*args, **kwargs)


def _process_idle():
    _aztek.call("flushIdleQueue", "[]", "{}")


utils = types.ModuleType("aztek.utils")
utils.executeDeferred = _execute_deferred
utils.executeInMainThreadWithResult = _in_main_thread
utils.processIdleEvents = _process_idle
aztek.utils = utils
sys.modules["aztek.utils"] = utils

# Scripts written for the reference import maya.cmds and maya.mel; both are Aztek's.
maya = types.ModuleType("maya")
maya.__path__ = []
maya.cmds = cmds
maya.mel = mel
sys.modules["maya"] = maya
sys.modules["maya.cmds"] = cmds
sys.modules["maya.mel"] = mel
maya.utils = utils
sys.modules["maya.utils"] = utils

import __main__

__main__.cmds = cmds


class _ApiImporter:
    """OpenAztek, Aztek's Python API, embedded in the host and loaded on first import.

    aztek.api.OpenAztek and aztek.api.OpenAztekAnim are the 2.0 API; aztek.OpenAztek, aztek.OpenAztekAnim,
    aztek.OpenAztekMPx and aztek.OpenAztekUI are the 1.0 API built on them. Scripts written for the reference import
    them under the reference's names (maya.api.OpenMaya, maya.OpenMaya, ...) and get the very same modules: one module
    object, whichever name is imported first. aztek.api.OpenMaya and aztek.api.OpenMayaAnim, Aztek's earlier names,
    are kept the same way."""

    # Canonical name: the embedded sources it is made of.
    SOURCES = {
        "aztek.api.OpenAztek": ["om_values", "om_core", "om_fn", "om_iter", "om_plugin"],
        "aztek.api.OpenAztekAnim": ["om_anim"],
        "aztek.OpenAztek": ["om1"],
        "aztek.OpenAztekAnim": ["om1_anim"],
        "aztek.OpenAztekMPx": ["om1_mpx"],
        "aztek.OpenAztekUI": ["omui"],
    }

    # Other names for the same modules: the reference's, and Aztek's earlier ones.
    ALIASES = {
        "maya.api.OpenMaya": "aztek.api.OpenAztek",
        "maya.api.OpenMayaAnim": "aztek.api.OpenAztekAnim",
        "aztek.api.OpenMaya": "aztek.api.OpenAztek",
        "aztek.api.OpenMayaAnim": "aztek.api.OpenAztekAnim",
        "maya.OpenMaya": "aztek.OpenAztek",
        "maya.OpenMayaAnim": "aztek.OpenAztekAnim",
        "maya.OpenMayaMPx": "aztek.OpenAztekMPx",
        "maya.OpenMayaUI": "aztek.OpenAztekUI",
    }

    PACKAGES = ("aztek.api", "maya.api")

    # Other modules of the reference's that tools import: name to embedded source, or None for a plain package.
    EXTRA = {
        "maya.standalone": "maya_misc",
        "maya.app": None,
        "maya.app.general": None,
        "maya.app.general.mayaMixin": "maya_mixin",
        "maya.app.flux": None,
        "maya.app.flux.core": "maya_misc",
        "maya.app.renderSetup": None,
        "maya.app.renderSetup.model": None,
        "maya.app.renderSetup.model.renderSetup": "maya_misc",
        "maya.app.renderSetup.model.renderLayer": "maya_misc",
        "maya.app.renderSetup.model.typeIDs": "maya_misc",
        "maya.internal": None,
        "maya.internal.nodes": None,
        "maya.internal.nodes.proximitywrap": None,
        "maya.internal.nodes.proximitywrap.node_interface": "maya_misc",
    }

    @classmethod
    def canonical(cls, name):
        return cls.ALIASES.get(name, name)

    def find_spec(self, name, path=None, target=None):
        # Only names of ours: importing machinery for any other name would come back here.
        if name in self.EXTRA:
            package = self.EXTRA[name] is None
        elif name in self.PACKAGES:
            package = True
        elif self.canonical(name) in self.SOURCES:
            package = False
        else:
            return None
        import importlib.machinery as m

        return m.ModuleSpec(name, self, is_package=package)

    def create_module(self, spec):
        canon = self.canonical(spec.name)
        if canon in self.SOURCES:
            # One module under every name: whichever was imported first is the one.
            for n in [canon] + [k for k, v in self.ALIASES.items() if v == canon]:
                if n != spec.name and n in sys.modules:
                    return sys.modules[n]
        return None

    def exec_module(self, module):
        name = module.__spec__.name
        if name in self.EXTRA:
            part = self.EXTRA[name]
            if part is not None:
                exec(compile(_aztek.source(part), "<%s>" % part, "exec"), module.__dict__)
            return
        canon = self.canonical(name)
        if canon not in self.SOURCES:
            return
        import importlib

        module.__name__ = canon
        for n in [canon] + [k for k, v in self.ALIASES.items() if v == canon]:
            sys.modules[n] = module
            # The parent package too, so `import aztek.api.OpenMaya as x` finds aztek.api on the way.
            parent, _, leaf = n.rpartition(".")
            setattr(importlib.import_module(parent), leaf, module)
        if "_loaded" in module.__dict__:
            return
        module.__dict__["_loaded"] = True
        for part in self.SOURCES[canon]:
            exec(compile(_aztek.source(part), "<%s>" % part, "exec"), module.__dict__)


sys.meta_path.insert(0, _ApiImporter())


def _run(src):
    """Runs script editor source in __main__; a lone expression's value is shown as the reference shows it."""
    import traceback

    g = __main__.__dict__
    g["_aztek_run_failed"] = False
    try:
        try:
            code = compile(src, "<script>", "eval")
        except SyntaxError:
            code = None
        if code is not None:
            value = eval(code, g)
            if value is not None:
                _aztek.write("# Result: %r #" % (value,), 1)
        else:
            exec(compile(src, "<script>", "exec"), g)
    except Exception as e:
        lines = traceback.format_exception(type(e), e, e.__traceback__)
        for l in "".join(lines[1:-1]).rstrip().splitlines():
            _aztek.write("# " + l, 2)
        _aztek.write("# Error: %s: %s #" % (type(e).__name__, e), 2)
        g["_aztek_run_failed"] = True
    finally:
        if _before_call:
            try:
                _before_call[0]()
            except Exception as e:
                _aztek.write("# Error: %s #" % (e,), 2)
        sys.stdout.flush()
        sys.stderr.flush()


def _eval_from_mel(src):
    """MEL's python(): a lone expression's value as JSON, statements give null. Exceptions go back to MEL."""
    g = __main__.__dict__
    try:
        try:
            code = compile(src, "<python>", "eval")
        except SyntaxError:
            exec(compile(src, "<python>", "exec"), g)
            return "null"
        return json.dumps(_plain(eval(code, g)), default=str)
    finally:
        sys.stdout.flush()
        sys.stderr.flush()


def _report_exception(e):
    import traceback

    lines = traceback.format_exception(type(e), e, e.__traceback__)
    for l in "".join(lines[1:-1]).rstrip().splitlines():
        _aztek.write("# " + l, 2)
    _aztek.write("# Error: %s: %s #" % (type(e).__name__, e), 2)


def _finish_callback():
    if _before_call:
        try:
            _before_call[0]()
        except Exception as e:
            _aztek.write("# Error: %s #" % (e,), 2)
    sys.stdout.flush()
    sys.stderr.flush()


def _fire(cid, args):
    """A callable kept for a control or job, with what the reference passes it."""
    fn = _callbacks.get(cid)
    if fn is None:
        return
    try:
        fn(*json.loads(args))
    except Exception as e:
        _report_exception(e)
    finally:
        _finish_callback()


def _fire_value(cid, args):
    """As _fire, with what the callable returns, as JSON."""
    fn = _callbacks.get(cid)
    if fn is None:
        return "null"
    try:
        return json.dumps(_plain(fn(*json.loads(args))))
    except Exception as e:
        _report_exception(e)
        return "null"
    finally:
        _finish_callback()


def _fire_source(src):
    """Python source given as a callback, run in __main__."""
    try:
        exec(compile(src, "<callback>", "exec"), __main__.__dict__)
    except Exception as e:
        _report_exception(e)
    finally:
        _finish_callback()


__main__.__dict__["_aztek_fire"] = _fire
__main__.__dict__["_aztek_fire_value"] = _fire_value
__main__.__dict__["_aztek_fire_source"] = _fire_source
__main__.__dict__["_aztek_run"] = _run
__main__.__dict__["_aztek_eval"] = _eval_from_mel


# USD: pxr, mayaUsd.ufe and mayaUsd.lib.
_pxr_shim = types.ModuleType("_aztek_pxr_shim")
sys.modules["_aztek_pxr_shim"] = _pxr_shim
exec(compile(_aztek.source("pxr_shim"), "<pxr_shim>", "exec"), _pxr_shim.__dict__)
