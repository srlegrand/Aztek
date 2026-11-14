# Thin classes: the reference's names, so scripts that import them load and plug-ins that derive from them register.
# What a thin class does not do raises NotImplementedError when it is called, not when it is named. Registration
# calls (register..., deregister...) are kept and do nothing: a plug-in's initializePlugin goes through. Constants of
# a thin class (k...) that are not written out are numbers of their own, not the reference's values.
import itertools as _thin_itertools

_thin_numbers = _thin_itertools.count(1)
_thin_constants = {}


def _not_here(owner, name):
    def call(*args, **kwargs):
        raise NotImplementedError("%s.%s is not in Aztek yet" % (owner, name))

    call.__name__ = name
    return call


def _kept(*args, **kwargs):
    return None


def _quiet(name):
    return name.startswith("register") or name.startswith("deregister")


class _ThinType(type):
    """Class-level: unknown constants are placeholders, unknown calls raise when called."""

    def __getattr__(cls, name):
        if name.startswith("__"):
            raise AttributeError(name)
        if name.startswith("k") and name[1:2].isupper():
            key = (cls.__name__, name)
            if key not in _thin_constants:
                _thin_constants[key] = next(_thin_numbers)
            return _thin_constants[key]
        if _quiet(name):
            return _kept
        return _not_here(cls.__name__, name)


class _Thin(metaclass=_ThinType):
    """A thin class: made with any arguments; its calls that are not written out raise when called."""

    def __init__(self, *args, **kwargs):
        pass

    def __getattr__(self, name):
        if name.startswith("__"):
            raise AttributeError(name)
        if name.startswith("k") and name[1:2].isupper():
            return getattr(type(self), name)
        if _quiet(name):
            return _kept
        return _not_here(type(self).__name__, name)


def _thin_classes(names, ns, base=_Thin):
    for n in names:
        if n not in ns:
            ns[n] = _ThinType(n, (base,), {"__doc__": "The reference's %s. Thin: what it does is not in Aztek yet." % (n,)})
