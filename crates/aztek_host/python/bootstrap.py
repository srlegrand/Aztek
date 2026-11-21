# Sets up the aztek package (aztek.cmds, aztek.mel), the compatibility modules and the script editor's output.
import json
import sys
import types

import _aztek

# This module's namespace under a name of its own: a script run by aztekpy is __main__ while it runs, as in the
# reference's mayapy, so the modules that reach back here can't go through __main__.
sys.modules["_aztek_main"] = sys.modules["__main__"]


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
    # A number first: MPlug's states are numbers that can also be called.
    if isinstance(v, int) and not isinstance(v, bool) and type(v).__name__ == "_Flag":
        return bool(v)
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
        if name in ("listRelatives", "listConnections", "listAttr", "filterExpand") and result == []:
            return None
        return result

    run.__name__ = name
    run.__doc__ = _aztek.doc(name)
    return run


# Procedures the reference has only in MEL: its Python commands module has none of them.
_MEL_ONLY = {"isParentOf", "lsType", "listTransforms", "findRelatedDeformer", "getChain", "getAllChains", "gotoBindPose", "setConstraintRestPosition", "moveIKtoFK"}


class _Cmds(types.ModuleType):
    """aztek.cmds: every Aztek command as a function. Flags are keyword arguments, long or short names."""

    def __getattr__(self, name):
        if name.startswith("__"):
            raise AttributeError(name)
        if name in _MEL_ONLY or not _aztek.exists(name):
            raise AttributeError("aztek.cmds has no command %r" % name)
        f = _command(name)
        setattr(self, name, f)
        return f

    def __dir__(self):
        return sorted(n for n in json.loads(_aztek.names()) if n not in _MEL_ONLY)


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


def _exists(name, *args, **kwargs):
    # MEL's exists: a command, a procedure, a built-in or a script on the script path.
    if not isinstance(name, str):
        raise TypeError("exists: give a name.")
    return bool(_mel_eval("exists " + json.dumps(name)))


cmds.exists = _exists
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

    aztek.api.OpenAztek, aztek.api.OpenAztekAnim, aztek.api.OpenAztekUI and aztek.api.OpenAztekRender are the 2.0 API; aztek.OpenAztek, aztek.OpenAztekAnim,
    aztek.OpenAztekMPx and aztek.OpenAztekUI are the 1.0 API built on them. Scripts written for the reference import
    them under the reference's names (maya.api.OpenMaya, maya.OpenMaya, ...) and get the very same modules: one module
    object, whichever name is imported first. aztek.api.OpenMaya and aztek.api.OpenMayaAnim, Aztek's earlier names,
    are kept the same way."""

    # Canonical name: the embedded sources it is made of.
    SOURCES = {
        "aztek.api.OpenAztek": ["om_values", "om_core", "om_fn", "om_iter", "om_plugin", "om_msg", "om_surface"],
        "aztek.api.OpenAztekAnim": ["om_anim"],
        "aztek.api.OpenAztekUI": ["om_thin", "om2_ui"],
        "aztek.api.OpenAztekRender": ["om_thin", "om2_render"],
        "aztek.OpenAztek": ["om1"],
        "aztek.OpenAztekAnim": ["om1_anim"],
        "aztek.OpenAztekMPx": ["om1_mpx"],
        "aztek.OpenAztekUI": ["omui"],
    }

    # Other names for the same modules: the reference's, and Aztek's earlier ones.
    ALIASES = {
        "maya.api.OpenMaya": "aztek.api.OpenAztek",
        "maya.api.OpenMayaAnim": "aztek.api.OpenAztekAnim",
        "maya.api.OpenMayaUI": "aztek.api.OpenAztekUI",
        "maya.api.OpenMayaRender": "aztek.api.OpenAztekRender",
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


class _QtBindingCheck:
    """PySide uses Qt's private parts, so a PySide only works with the very Qt it was built for: another one aborts
    the whole application on import. Checked here first, so a mismatch is an ImportError that says what to install.
    AZTEK_SKIP_QT_CHECK=1 turns it off."""

    NAMES = ("PySide6", "shiboken6", "PySide2", "shiboken2")

    def __init__(self):
        self.checked = set()

    def find_spec(self, name, path=None, target=None):
        if name not in self.NAMES or name in self.checked or path is not None:
            return None
        self.checked.add(name)
        import os

        if os.environ.get("AZTEK_SKIP_QT_CHECK"):
            return None
        import importlib.machinery as m

        spec = m.PathFinder.find_spec(name)
        if spec is None or not spec.submodule_search_locations:
            return None
        here = list(spec.submodule_search_locations)[0]
        built = self.built_for(here)
        # Asked of the application: Python may have started before Qt set it in the environment.
        try:
            qt = json.loads(_aztek.call("about", "[]", json.dumps({"qtVersion": True})))
        except Exception:
            qt = ""
        if not isinstance(qt, str):
            qt = ""
        # Two other ways shiboken aborts the whole application as PySide loads, "init_enum: libshiboken: could not
        # init enum" (#169): a shiboken6 of another version than the PySide6, and an `enum` module that isn't Python's
        # own (a backport, or a tool's enum.py, earlier on the path). Each an ImportError that says which.
        if name in ("PySide6", "shiboken6"):
            other = m.PathFinder.find_spec("shiboken6" if name == "PySide6" else "PySide6")
            if other is not None and other.submodule_search_locations:
                there = list(other.submodule_search_locations)[0]
                theirs = self.built_for(there, "__version__") or self.built_for(there)
                mine = self.built_for(here, "__version__") or built
                if theirs and mine and theirs[:2] != mine[:2]:
                    raise ImportError(
                        "%s is version %s but %s is %s, at %s; shiboken6 and PySide6 must be the same version or "
                        "PySide stops Aztek as it loads. Install the two together (%s)."
                        % (name, ".".join(map(str, mine)), "shiboken6" if name == "PySide6" else "PySide6", ".".join(map(str, theirs)), there, self.how(qt or ".".join(map(str, mine))))
                    )
            stray = self.stray_enum()
            if stray:
                raise ImportError(
                    "The enum module Python loaded is %s, not Python's own, and PySide stops Aztek when it isn't. Take that "
                    "folder off the script path or PYTHONPATH (a Python 2 backport of enum, enum34, is the usual one), "
                    "or rename the file." % stray
                )
        if not qt:
            return None
        qt_mm = tuple(int(x) for x in qt.split(".")[:2])
        if name.endswith("2"):
            raise ImportError(
                "%s is for Qt 5, and Aztek runs on Qt %s. Use PySide6 %s (%s)." % (name, qt, qt, self.how(qt))
            )
        if built is None or built[:2] == qt_mm:
            return None
        raise ImportError(
            "%s %s is built for Qt %d.%d, but Aztek runs on Qt %s. PySide only works with the Qt it was built for, so "
            "this one would stop Aztek. Use PySide6 %s (%s). Found at %s."
            % (name, ".".join(map(str, built)), built[0], built[1], qt, qt, self.how(qt), here)
        )

    @staticmethod
    def built_for(folder, key="version"):
        """From the package's _config.py: version is the Qt it was built for, __version__ its own."""
        import os
        import re

        try:
            with open(os.path.join(folder, "_config.py"), encoding="utf-8") as f:
                text = f.read()
        except OSError:
            return None
        found = re.search(r'^%s\s*=\s*"(\d+)\.(\d+)(?:\.(\d+))?' % re.escape(key), text, re.M)
        return tuple(int(x) for x in found.groups() if x is not None) if found else None

    @staticmethod
    def stray_enum():
        """Where the enum module came from when it isn't Python's standard library, else None."""
        import os
        import sysconfig

        e = sys.modules.get("enum")
        f = getattr(e, "__file__", None)
        if not f:
            return None
        std = os.path.normcase(os.path.realpath(sysconfig.get_paths()["stdlib"]))
        here = os.path.normcase(os.path.realpath(os.path.dirname(f)))
        return None if here == std else f

    @staticmethod
    def how(qt):
        v = "%d.%d" % sys.version_info[:2]
        return 'pip: python%s -m pip install --user "PySide6-Essentials==%s", or your distribution\'s PySide6 when Aztek uses its Qt' % (v, qt)


sys.meta_path.insert(0, _QtBindingCheck())


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


def _fire_value_kw(cid, kwargs):
    """A kept callable given keyword arguments, with what it returns, as JSON."""
    fn = _callbacks.get(cid)
    if fn is None:
        return "null"
    try:
        return json.dumps(_plain(fn(**json.loads(kwargs))))
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
__main__.__dict__["_aztek_fire_value_kw"] = _fire_value_kw
__main__.__dict__["_aztek_fire_source"] = _fire_source
__main__.__dict__["_aztek_run"] = _run
__main__.__dict__["_aztek_eval"] = _eval_from_mel


# USD: pxr, mayaUsd.ufe and mayaUsd.lib.
_pxr_shim = types.ModuleType("_aztek_pxr_shim")
sys.modules["_aztek_pxr_shim"] = _pxr_shim
exec(compile(_aztek.source("pxr_shim"), "<pxr_shim>", "exec"), _pxr_shim.__dict__)


def _aztek_add_packages(folder):
    """Aztek's own packages folder on the path, with its .pth files, ahead of the system's site-packages."""
    import site

    before = list(sys.path)
    site.addsitedir(folder)
    added = [p for p in sys.path if p not in before]
    for p in added:
        sys.path.remove(p)
    at = next((i for i, p in enumerate(sys.path) if p.rstrip("/\\").endswith(("site-packages", "dist-packages"))), len(sys.path))
    sys.path[at:at] = added


def _aztek_pymain(argv, exe, packages):
    """aztekpy: Python's own command line, run by Aztek's Python. The exit status comes back."""
    import code
    import os
    import runpy

    # The terminal's own streams, so prompts and output read as Python's do.
    sys.stdout, sys.stderr = sys.__stdout__, sys.__stderr__
    if exe:
        sys.executable = exe
    args, flags = _aztek_py_options(list(argv))
    if args is None:
        return 2
    _aztek_py_isolate(flags)
    # -I and -P: neither the current folder nor the script's goes on the path.
    safe_path = "I" in flags or "P" in flags
    # pip installs into Aztek's own packages unless told where: the reference's mayapy keeps its packages to itself.
    if len(args) >= 3 and args[0] == "-m" and args[1] == "pip" and args[2] == "install":
        given = {"-t", "--target", "--prefix", "--root", "--user", "-e", "--editable"}
        if not any(a.split("=", 1)[0] in given for a in args[3:]):
            os.makedirs(packages, exist_ok=True)
            args[3:3] = ["--target", packages]
    main = sys.modules["__main__"].__dict__
    try:
        if args and args[0] in ("-V", "--version"):
            print("Python " + sys.version.split()[0])
            return 0
        if not args:
            sys.argv = [""]
            code.interact(banner="Aztek Python " + sys.version.split()[0] + "\nType help() for help.", local=main, exitmsg="")
            return 0
        if args[0] == "-c":
            if len(args) < 2:
                print("Argument expected for the -c option", file=sys.stderr)
                return 2
            sys.argv = ["-c"] + args[2:]
            exec(compile(args[1], "<string>", "exec"), main)
        elif args[0] == "-m":
            if len(args) < 2:
                print("Argument expected for the -m option", file=sys.stderr)
                return 2
            sys.argv = [args[1]] + args[2:]
            if not safe_path:
                sys.path.insert(0, os.getcwd())
            runpy.run_module(args[1], run_name="__main__", alter_sys=True)
        elif args[0] == "-":
            sys.argv = ["-"] + args[1:]
            exec(compile(sys.stdin.read(), "<stdin>", "exec"), main)
        else:
            sys.argv = list(args)
            if not safe_path:
                sys.path.insert(0, os.path.dirname(os.path.abspath(args[0])))
            runpy.run_path(args[0], run_name="__main__")
    except SystemExit as e:
        c = e.code
        if c is None:
            return 0
        if isinstance(c, int):
            return c
        print(c, file=sys.stderr)
        return 1
    except BaseException:
        import traceback

        traceback.print_exc()
        return 1
    finally:
        sys.stdout.flush()
        sys.stderr.flush()
    return 0


# Python's interpreter options that take a value, and those that take none. The interpreter is already running, so -W,
# and the path flags -I, -P, -E and -s, are what take effect; the rest are accepted so that command lines written for
# python.exe (ensurepip, pip, venv) run.
_AZTEK_PY_VALUED = "WX"
_AZTEK_PY_FLAGS = "bBdEiIOPqsSuvx"


def _aztek_py_options(args):
    """Takes Python's interpreter options off the front of aztekpy's arguments: (the rest, the flag letters given), or
    (None, "") on a bad option."""
    import warnings

    flags = ""
    # -c, -m, -V and --long options are _aztek_pymain's to read.
    while args and args[0].startswith("-") and len(args[0]) > 1 and args[0][1] not in "cmV-":
        opt = args.pop(0)
        letters = opt[1:]
        while letters:
            letter, letters = letters[0], letters[1:]
            if letter in _AZTEK_PY_VALUED:
                value = letters or (args.pop(0) if args else None)
                if value is None:
                    print("Argument expected for the -" + letter + " option", file=sys.stderr)
                    return None, ""
                if letter == "W":
                    warnings._setoption(value)
                letters = ""
            elif letter in _AZTEK_PY_FLAGS:
                flags += letter
            elif letter in "cm":
                # -Ec, -Im pip: the rest of the token is -c's code or -m's module, as Python reads it.
                args[0:0] = ["-" + letter] + ([letters] if letters else [])
                letters = ""
            else:
                print("Unknown option: -" + letter, file=sys.stderr)
                return None, ""
    return args, flags


def _aztek_py_isolate(flags):
    """-E and -I take PYTHONPATH's folders off the path, -s and -I the user's site-packages, -I and -P the current
    folder's "" entry, as Python would have."""
    import os
    import site

    drop = set()
    keep_empty = not ("I" in flags or "P" in flags)
    env = os.environ.get("PYTHONPATH", "")
    if env and ("E" in flags or "I" in flags):
        entries = env.split(os.pathsep)
        drop.update(p for p in entries if p)
        # An empty entry is the current folder, on the path as "" or as the folder itself.
        if "" in entries:
            drop.add(os.getcwd())
            keep_empty = False
    if "s" in flags or "I" in flags:
        drop.add(site.getusersitepackages())
        site.ENABLE_USER_SITE = False
    norm = {os.path.normcase(os.path.abspath(p)) for p in drop}
    sys.path[:] = [p for p in sys.path if (keep_empty if not p else os.path.normcase(os.path.abspath(p)) not in norm)]
