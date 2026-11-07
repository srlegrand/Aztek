//! Embedded Python: aztek.cmds and the maya.cmds shim.
#![cfg(feature = "python")]

use std::ffi::{CStr, CString};

use aztek_host::*;
use serde_json::Value;

fn py(src: &str) {
    let c = CString::new(src).unwrap();
    aztek_python(c.as_ptr());
}

fn log() -> Vec<(i64, String)> {
    let p = aztek_take_log();
    let s = unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned();
    aztek_free(p);
    let v: Value = serde_json::from_str(&s).unwrap();
    v.as_array().unwrap().iter().map(|l| (l["kind"].as_i64().unwrap(), l["text"].as_str().unwrap().to_string())).collect()
}

#[test]
fn the_reference_scripts_run() {
    aztek_init();
    log();
    py("import maya.cmds as cmds\ncube = cmds.polyCube(w=2, sx=3)\nprint(cube)");
    let l = log();
    assert!(l.iter().any(|(k, t)| *k == 3 && t == "['pCube1', 'polyCube1']"), "{l:?}");
    python_keys();
    python_messages();
    py("cmds.polyCube('polyCube1', q=True, w=True)");
    let l = log();
    assert!(l.iter().any(|(_, t)| t == "# Result: 2.0 #"), "{l:?}");
    py("cmds.polyCube('polyCube1', e=True, h=3)\nprint(cmds.getAttr('polyCube1.height'))");
    let l = log();
    assert!(l.iter().any(|(_, t)| t == "3.0"), "{l:?}");
    py("cmds.select('pCube1.f[0:2]')\ncmds.polyExtrudeFacet(ltz=0.5)\nprint(len(cmds.ls(sl=True, fl=True)))");
    let l = log();
    assert!(l.iter().any(|(_, t)| t == "3"), "{l:?}");
    py("from aztek import cmds as az\naz.move(1, 2, 3, 'pCube1', r=True)\nprint(az.getAttr('pCube1.translate'))");
    let l = log();
    assert!(l.iter().any(|(_, t)| t == "[(1.0, 2.0, 3.0)]"), "{l:?}");
    py("cmds.nosuchcommand()");
    let l = log();
    assert!(l.iter().any(|(k, t)| *k == 2 && t.contains("AttributeError")), "{l:?}");
    py("import maya.mel as mel\nmel.eval('polySphere -r 2')");
    let l = log();
    assert!(l.iter().any(|(_, t)| t.contains("pSphere2")), "{l:?}");
}

/// Called from the_reference_scripts_run, as Python runs once per process.
fn python_messages() {
    log();
    py("import maya.cmds as cmds\ncmds.warning('careful')\ngetattr(cmds, 'print')('hello')");
    let l = log();
    assert!(l.iter().any(|(k, t)| *k == 4 && t == "# Warning: careful #"), "{l:?}");
    assert!(l.iter().any(|(_, t)| t == "hello"), "{l:?}");
    py("import maya.mel\nmaya.mel.eval('proc pyProc(int $i, string $s[]) {}')\nx = ['a', 'b']\nprint(cmds.clear(x), x)\nprint(cmds.getProcArguments('pyProc'))");
    let l = log();
    assert!(l.iter().any(|(_, t)| t == "2 []"), "{l:?}");
    assert!(l.iter().any(|(_, t)| t == "['int', 'string[]']"), "{l:?}");
    py("cmds.commandEcho(state=True)\ncmds.ls(sl=True)\ncmds.commandEcho(state=False)");
    let l = log();
    assert!(l.iter().any(|(_, t)| t.starts_with("ls ") && t.ends_with(';')), "commandEcho should echo: {l:?}");
    log();
    py("cmds.ls(sl=True)");
    assert!(!log().iter().any(|(_, t)| t.starts_with("ls ")));
    py("print(cmds.skeletonEmbed, cmds.jointDisplayScale, cmds.help, cmds.condition)");
    assert!(!log().iter().any(|(k, _)| *k == 2), "new commands should reach cmds");
}

/// Python runs once per process: called from the_reference_scripts_run.
fn python_keys() {
    log();
    py("import maya.cmds as cmds\nc = cmds.polySphere()[0]\ncmds.setKeyframe(c, at='ty', t=1, v=0)\ncmds.setKeyframe(c, at='ty', t=10, v=5)\nprint(cmds.keyframe(c, q=True, kc=True))\nprint(cmds.playbackOptions(q=True, max=True))\ncmds.currentTime(10)\nprint(cmds.getAttr(c + '.ty'))");
    let l = log();
    assert!(l.iter().any(|(_, t)| t == "2"), "{l:?}");
    assert!(l.iter().any(|(_, t)| t == "120"), "{l:?}");
    assert!(l.iter().any(|(_, t)| t == "5.0"), "{l:?}");
}
