//! The pxr module in Aztek's Python.
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

fn expect(lines: &[(i64, String)], want: &str) {
    assert!(lines.iter().any(|(_, t)| t == want), "no line {want:?} in {lines:?}");
}

fn no_errors(lines: &[(i64, String)]) {
    assert!(!lines.iter().any(|(k, _)| *k == 2), "{lines:?}");
}

// One test: the interpreter and host are one per process.
#[test]
fn pxr_scripts_run() {
    aztek_init();
    log();
    let dir = std::env::temp_dir().join(format!("aztek_usd_py_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("made.usda");
    let path = path.to_string_lossy().replace('\\', "/");

    // A stage made and saved, as the tutorials do it.
    py(&format!(
        r#"
from pxr import Usd, UsdGeom, Sdf, Gf, Vt, Tf
stage = Usd.Stage.CreateNew("{path}")
world = UsdGeom.Xform.Define(stage, "/World")
UsdGeom.SetStageUpAxis(stage, UsdGeom.Tokens.y)
stage.SetDefaultPrim(world.GetPrim())
mesh = UsdGeom.Mesh.Define(stage, "/World/tri")
mesh.CreatePointsAttr([(0, 0, 0), (1, 0, 0), (0, 1, 0)])
mesh.CreateFaceVertexCountsAttr([3])
mesh.CreateFaceVertexIndicesAttr([0, 1, 2])
mesh.CreateSubdivisionSchemeAttr(UsdGeom.Tokens.none)
xf = UsdGeom.Xformable(mesh.GetPrim())
t = xf.AddTranslateOp()
t.Set(Gf.Vec3d(1, 2, 3), 1)
t.Set(Gf.Vec3d(5, 2, 3), 5)
xf.AddRotateYOp().Set(90.0)
st = UsdGeom.PrimvarsAPI(mesh).CreatePrimvar("st", Sdf.ValueTypeNames.TexCoord2fArray, UsdGeom.Tokens.faceVarying)
st.Set([(0, 0), (1, 0), (0, 1)])
print(stage.GetPrimAtPath("/World/tri").GetTypeName())
print(sorted(p.GetPath().pathString for p in stage.Traverse()))
print(mesh.GetPointsAttr().Get()[1])
print(t.GetAttr().GetTimeSamples())
print(t.Get(3))
print(st.GetInterpolation())
print(stage.GetPrimAtPath("/nope").IsValid())
print(Sdf.Path("/a/b.c").GetPrimPath(), Sdf.Path("/a/b").GetParentPath(), Sdf.Path("/a").AppendChild("x").name)
stage.GetRootLayer().Save()
"#
    ));
    let l = log();
    no_errors(&l);
    expect(&l, "Mesh");
    expect(&l, "['/World', '/World/tri']");
    expect(&l, "Gf.Vec3f(1.0, 0.0, 0.0)");
    expect(&l, "[1.0, 5.0]");
    expect(&l, "Gf.Vec3d(3.0, 2.0, 3.0)");
    expect(&l, "faceVarying");
    expect(&l, "False");
    expect(&l, "/a/b /a x");
    let saved = std::fs::read_to_string(&path).unwrap();
    assert!(saved.contains("def Mesh \"tri\""), "{saved}");
    assert!(saved.contains("xformOp:rotateY"));

    // A proxy's stage through mayaUsd.ufe; a script's edit undoes with the scene's undo.
    py(&format!(
        r#"
import maya.cmds as cmds
import mayaUsd.ufe, mayaUsd.lib
shape = cmds.mayaUsd_createStageFromFile("{path}")
stage2 = mayaUsd.ufe.getStage(shape)
print(stage2.GetPrimAtPath("/World/tri").GetName())
p = mayaUsd.lib.GetPrim(shape + ",/World/tri")
print(p.GetTypeName())
p.CreateAttribute("note", Sdf.ValueTypeNames.String).Set("hello")
print(p.GetAttribute("note").Get())
print(cmds.getAttr(shape + ",/World/tri.note"))
cmds.undo()
print(p.GetAttribute("note").Get())
vs = p.GetVariantSets()
print(vs.GetNames())
layer = stage2.GetRootLayer()
print(layer.subLayerPaths)
print(UsdGeom.GetStageUpAxis(stage2))
"#
    ));
    let l = log();
    no_errors(&l);
    expect(&l, "tri");
    expect(&l, "Mesh");
    expect(&l, "hello");
    expect(&l, "None");
    expect(&l, "[]");
    expect(&l, "Y");

    // A translator from Python, as the Animal Logic USD plug-in has them; import calls it for its prim type.
    let tool = dir.join("custom.usda");
    let tool = tool.to_string_lossy().replace('\\', "/");
    std::fs::write(&tool, "#usda 1.0\ndef Gadget \"thing\"\n{\n    double size = 4\n}\n").unwrap();
    py(&format!(
        r#"
from AL.usdmaya import TranslatorBase
class Gadget(TranslatorBase):
    primType = "Gadget"
    def importObject(self, prim, parent):
        size = prim.GetAttribute("size").Get()
        made = cmds.polyCube(w=size, name=prim.GetName() + "Geo")[0]
        cmds.parent(made, parent)
Gadget.register()
print(cmds.mayaUSDImport(file="{tool}"))
print(cmds.objExists("thingGeo"), cmds.polyCube("polyCube1", q=True, w=True))
print(cmds.listRelatives("thingGeo", parent=True))
Gadget.unregister()
"#
    ));
    let l = log();
    no_errors(&l);
    expect(&l, "['thing']");
    expect(&l, "['thing']");

    // A hook for asset paths, from Python.
    let target = dir.join("target.usda");
    let target = target.to_string_lossy().replace('\\', "/");
    std::fs::write(&target, "#usda 1.0\ndef Cube \"C\"\n{\n}\n").unwrap();
    let uses = dir.join("uses.usda");
    let uses = uses.to_string_lossy().replace('\\', "/");
    std::fs::write(&uses, "#usda 1.0\ndef \"a\" (\n    references = @studio://cube.usda@</C>\n)\n{\n}\n").unwrap();
    py(&format!(
        r#"
from pxr import Ar
Ar.SetResolveHook(lambda asset: "{target}" if asset == "studio://cube.usda" else None)
hooked = Usd.Stage.Open("{uses}")
print(hooked.GetPrimAtPath("/a").GetTypeName())
Ar.SetResolveHook(None)
"#
    ));
    let l = log();
    no_errors(&l);
    expect(&l, "Cube");
}
