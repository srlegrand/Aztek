#!/usr/bin/env python3
"""Writes devkit/include/maya/<Class>.h, one per class, each including the kit header that defines it.

The classes are defined in a few headers under include/aztek/kit; plug-ins include them by the per-class names.
Run after adding a class to a kit header: python3 devkit/tools/make_headers.py
"""
import os
import re

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
KIT = os.path.join(ROOT, "include", "aztek", "kit")
OUT = os.path.join(ROOT, "include", "maya")

# Headers that are not one class: what they hold.
EXTRA = {
    "MTypes.h": "base.h",
    "MApiNamespace.h": "all.h",
    "MIOStream.h": None,
    "MIOStreamFwd.h": None,
    "MStreamUtils.h": "base.h",
    "MCallbackIdArray.h": "base.h",
    "MHWGeometryUtilities.h": "viewport.h",
    "MViewport2Renderer.h": "viewport.h",
    "MOpenCLAutoPtr.h": "gpu.h",
}

STREAM = """#include <iostream>
#include <fstream>
#include <iomanip>
using std::cerr;
using std::cin;
using std::cout;
using std::endl;
using std::flush;
using std::ios;
using std::istream;
using std::ofstream;
using std::ifstream;
using std::ostream;
"""


def classes(path):
    text = open(path).read()
    names = set(re.findall(r"^class (M\w+)\s*(?::|\{)", text, re.M))
    names |= set(re.findall(r"^AZ_ARRAY\((M\w+),", text, re.M))
    return names


def main():
    os.makedirs(OUT, exist_ok=True)
    written = set()
    for kit in sorted(os.listdir(KIT)):
        if not kit.endswith(".h") or kit == "all.h":
            continue
        for c in sorted(classes(os.path.join(KIT, kit))):
            name = c + ".h"
            with open(os.path.join(OUT, name), "w") as f:
                f.write("// Aztek plug-in kit. %s is defined in aztek/kit/%s.\n#pragma once\n#include <aztek/kit/%s>\n" % (c, kit, kit))
            written.add(name)
    with open(os.path.join(KIT, "all.h"), "w") as f:
        f.write("// Every class of the kit.\n#pragma once\n")
        for kit in sorted(os.listdir(KIT)):
            if kit.endswith(".h") and kit != "all.h":
                f.write("#include \"%s\"\n" % kit)
    for name, kit in EXTRA.items():
        with open(os.path.join(OUT, name), "w") as f:
            f.write("// Aztek plug-in kit.\n#pragma once\n")
            if kit is None:
                f.write(STREAM)
            else:
                f.write("#include <aztek/kit/%s>\n" % kit)
        written.add(name)
    for old in os.listdir(OUT):
        if old.endswith(".h") and old not in written:
            os.remove(os.path.join(OUT, old))
    print("%d headers in %s" % (len(written), OUT))


if __name__ == "__main__":
    main()
