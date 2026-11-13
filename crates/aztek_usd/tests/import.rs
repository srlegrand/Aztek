//! mayaUSDImport: hand-written usda read into nodes, and scenes written and read back.

use aztek_scene::session::Session;
use glam::DVec3;
use serde_json::{json, Value};

fn session() -> Session {
    let mut s = Session::new();
    aztek_scene::cameras::make_defaults(&mut s.graph);
    aztek_usd::install(&mut s);
    s
}

fn temp(name: &str) -> String {
    let dir = std::env::temp_dir().join(format!("aztek_usd_import_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name).to_string_lossy().into_owned()
}

fn write(name: &str, text: &str) -> String {
    let f = temp(name);
    std::fs::write(&f, text).unwrap();
    f
}

fn get(s: &mut Session, plug: &str) -> Value {
    s.run("getAttr", json!({"args": [plug]})).unwrap()
}

fn num(v: &Value) -> f64 {
    match v {
        Value::Array(a) => num(&a[0]),
        other => other.as_f64().unwrap_or(f64::NAN),
    }
}

fn v3(v: &Value) -> DVec3 {
    let a = match v {
        Value::Array(a) if a.len() == 1 => a[0].as_array().unwrap().clone(),
        Value::Array(a) => a.clone(),
        _ => panic!("not a vector: {v}"),
    };
    DVec3::new(a[0].as_f64().unwrap(), a[1].as_f64().unwrap(), a[2].as_f64().unwrap())
}

const SIMPLE: &str = r#"#usda 1.0
(
    defaultPrim = "World"
    metersPerUnit = 0.01
    upAxis = "Y"
)
def Xform "World"
{
    def Mesh "tri" (
        prepend apiSchemas = ["MaterialBindingAPI"]
    )
    {
        double3 xformOp:translate = (1, 2, 3)
        uniform token[] xformOpOrder = ["xformOp:translate"]
        int[] faceVertexCounts = [3, 3]
        int[] faceVertexIndices = [0, 1, 2, 0, 2, 3]
        point3f[] points = [(0,0,0),(1,0,0),(1,1,0),(0,1,0)]
        texCoord2f[] primvars:st = [(0,0),(1,0),(1,1),(0,1)] (
            interpolation = "vertex"
        )
        token visibility = "invisible"
        uniform token subdivisionScheme = "none"
        int[] creaseIndices = [0, 1]
        int[] creaseLengths = [2]
        float[] creaseSharpnesses = [3]
    }
    def Xform "moved"
    {
        float xformOp:rotateY = 90
        uniform token[] xformOpOrder = ["xformOp:rotateY"]
    }
}
def Xform "other"
{
}
"#;

#[test]
fn default_prim_and_mesh_data() {
    let mut s = session();
    let f = write("simple.usda", SIMPLE);
    let r = s.run("mayaUSDImport", json!({"file": f})).unwrap();
    assert_eq!(r, json!(["World"]));
    assert!(s.graph.find("other").unwrap().is_none(), "only the default prim");
    assert_eq!(v3(&get(&mut s, "tri.translate")), DVec3::new(1.0, 2.0, 3.0));
    assert_eq!(s.run("polyEvaluate", json!({"args": ["tri"], "face": true})).unwrap(), json!(2));
    assert_eq!(s.run("polyEvaluate", json!({"args": ["tri"], "vertex": true})).unwrap(), json!(4));
    assert_eq!(get(&mut s, "tri.visibility"), json!(false));
    assert_eq!(get(&mut s, "triShape.USD_subdivisionScheme"), json!("none"));
    // An op the reference's channels can't hold keeps its place through the matrix.
    let ry = v3(&get(&mut s, "moved.rotate"));
    assert!((ry.y - 90.0).abs() < 1e-6, "{ry}");
}

#[test]
fn prim_path_and_everything() {
    let mut s = session();
    let f = write("simple2.usda", SIMPLE);
    let r = s.run("mayaUSDImport", json!({"file": f, "primPath": "/other"})).unwrap();
    assert_eq!(r, json!(["other"]));
    assert!(s.graph.find("World").unwrap().is_none());
    let mut s = session();
    let r = s.run("mayaUSDImport", json!({"file": f, "primPath": "/"})).unwrap();
    assert_eq!(r, json!(["World", "other"]));
    assert!(s.run("mayaUSDImport", json!({"file": f, "primPath": "/nope"})).is_err());
}

#[test]
fn undo_takes_the_whole_import() {
    let mut s = session();
    let f = write("simple3.usda", SIMPLE);
    s.run("mayaUSDImport", json!({"file": f})).unwrap();
    assert!(s.graph.find("World").unwrap().is_some());
    s.undo();
    assert!(s.graph.find("World").unwrap().is_none());
    s.redo();
    assert!(s.graph.find("World").unwrap().is_some());
}

#[test]
fn time_samples_become_keys_when_asked() {
    let text = r#"#usda 1.0
(
    startTimeCode = 1
    endTimeCode = 3
)
def Xform "a"
{
    double3 xformOp:translate.timeSamples = {
        1: (0, 0, 0),
        3: (2, 0, 0),
    }
    uniform token[] xformOpOrder = ["xformOp:translate"]
}
"#;
    let f = write("anim.usda", text);
    let mut s = session();
    s.run("mayaUSDImport", json!({"file": f, "readAnimData": true})).unwrap();
    assert_eq!(num(&s.run("keyframe", json!({"args": ["a.tx"], "query": true, "keyframeCount": true})).unwrap()), 2.0);
    assert_eq!(s.run("getAttr", json!({"args": ["a.tx"], "time": 3.0})).unwrap(), json!(2.0));
    assert!(aztek_scene::anim::range().max >= 3.0);
    // Without the flag, the first sample and no keys.
    let mut s = session();
    s.run("mayaUSDImport", json!({"file": f})).unwrap();
    assert_eq!(num(&s.run("keyframe", json!({"args": ["a.tx"], "query": true, "keyframeCount": true})).unwrap()), 0.0);
}

#[test]
fn units_and_axis_convert_through_a_group() {
    let text = r#"#usda 1.0
(
    metersPerUnit = 1
    upAxis = "Z"
)
def Xform "a"
{
    double3 xformOp:translate = (0, 0, 2)
    uniform token[] xformOpOrder = ["xformOp:translate"]
}
"#;
    let f = write("metres.usda", text);
    let mut s = session();
    s.run("mayaUSDImport", json!({"file": f})).unwrap();
    let t = s.run("xform", json!({"args": ["a"], "query": true, "worldSpace": true, "translation": true})).unwrap();
    let t = v3(&t);
    assert!((t - DVec3::new(0.0, 200.0, 0.0)).length() < 1e-6, "{t}");
    // Both off: as written.
    let mut s = session();
    s.run("mayaUSDImport", json!({"file": f, "upAxis": false, "unit": false})).unwrap();
    assert_eq!(v3(&get(&mut s, "a.translate")), DVec3::new(0.0, 0.0, 2.0));
}

#[test]
fn round_trip_mesh_uvs_colours_creases_and_keys() {
    let mut s = session();
    s.run("polyCube", json!({"name": "box"})).unwrap();
    s.run("polyColorSet", json!({"args": ["box"], "create": true, "colorSet": "tint"})).unwrap();
    s.run("polyCrease", json!({"args": ["box.e[0]"], "value": 2.0})).unwrap();
    s.run("setKeyframe", json!({"args": ["box.tx"], "time": 1.0, "value": 0.0})).unwrap();
    s.run("setKeyframe", json!({"args": ["box.tx"], "time": 3.0, "value": 6.0})).unwrap();
    s.run("xform", json!({"args": ["box"], "rotatePivot": [1.0, 0.0, 0.0]})).unwrap();
    let f = temp("rt.usdc");
    s.run("mayaUSDExport", json!({"file": f, "frameRange": [1.0, 3.0]})).unwrap();

    let mut b = session();
    b.run("mayaUSDImport", json!({"file": f, "readAnimData": true})).unwrap();
    assert_eq!(b.run("polyEvaluate", json!({"args": ["box"], "face": true})).unwrap(), json!(6));
    assert_eq!(b.run("polyEvaluate", json!({"args": ["box"], "uvcoord": true})).unwrap(), s.run("polyEvaluate", json!({"args": ["box"], "uvcoord": true})).unwrap());
    assert_eq!(b.run("getAttr", json!({"args": ["box.tx"], "time": 3.0})).unwrap(), json!(6.0));
    assert_eq!(v3(&get(&mut b, "box.rotatePivot")), DVec3::new(1.0, 0.0, 0.0));
    let sets = b.run("polyColorSet", json!({"args": ["box"], "query": true, "allColorSets": true})).unwrap();
    assert_eq!(sets, json!(["tint"]));
}

#[test]
fn round_trip_skin_poses_the_same() {
    let mut s = session();
    s.run("polyPlane", json!({"name": "skinned", "subdivisionsX": 1, "subdivisionsY": 4, "width": 1, "height": 4})).unwrap();
    s.run("xform", json!({"args": ["skinned"], "rotation": [90.0, 0.0, 0.0], "worldSpace": true})).unwrap();
    s.run("select", json!({"clear": true})).unwrap();
    for (n, y) in [("hip", -2.0), ("knee", 0.0), ("foot", 2.0)] {
        s.run("joint", json!({"name": n, "position": [0.0, y, 0.0]})).unwrap();
    }
    s.run("skinCluster", json!({"args": ["hip", "knee", "foot", "skinned"], "name": "sc1"})).unwrap();
    s.run("setKeyframe", json!({"args": ["knee.rz"], "time": 1.0, "value": 0.0})).unwrap();
    s.run("setKeyframe", json!({"args": ["knee.rz"], "time": 5.0, "value": 45.0})).unwrap();
    let f = temp("skin.usda");
    s.run("mayaUSDExport", json!({"file": f, "frameRange": [1.0, 5.0], "exportSkels": "auto", "exportSkin": "auto"})).unwrap();

    let mut b = session();
    b.run("mayaUSDImport", json!({"file": f, "readAnimData": true, "primPath": "/"})).unwrap();
    assert_eq!(b.run("nodeType", json!({"args": ["knee"]})).unwrap(), json!("joint"));
    for frame in [1.0, 3.0, 5.0] {
        s.run("currentTime", json!({"args": [frame]})).unwrap();
        b.run("currentTime", json!({"args": [frame]})).unwrap();
        for i in 0..10 {
            let name = format!("skinned.vtx[{i}]");
            let pa = v3(&s.run("xform", json!({"args": [name], "query": true, "worldSpace": true, "translation": true})).unwrap());
            let pb = v3(&b.run("xform", json!({"args": [name], "query": true, "worldSpace": true, "translation": true})).unwrap());
            assert!((pa - pb).length() < 1e-3, "frame {frame} vtx {i}: {pa} vs {pb}");
        }
    }
}

#[test]
fn round_trip_blend_shape_weight() {
    let mut s = session();
    s.run("polyCube", json!({"name": "base"})).unwrap();
    s.run("polyCube", json!({"name": "up"})).unwrap();
    s.run("move", json!({"args": [0.0, 1.0, 0.0, "up.vtx[0:3]"], "relative": true})).unwrap();
    s.run("blendShape", json!({"args": ["up", "base"], "name": "bs1"})).unwrap();
    s.run("setAttr", json!({"args": ["bs1.w[0]", 0.5]})).unwrap();
    s.run("delete", json!({"args": ["up"]})).unwrap();
    let f = temp("blend.usda");
    s.run("mayaUSDExport", json!({"file": f, "exportBlendShapes": true})).unwrap();

    let mut b = session();
    b.run("mayaUSDImport", json!({"file": f, "primPath": "/"})).unwrap();
    let want = v3(&s.run("xform", json!({"args": ["base.vtx[0]"], "query": true, "worldSpace": true, "translation": true})).unwrap());
    let got = v3(&b.run("xform", json!({"args": ["base.vtx[0]"], "query": true, "worldSpace": true, "translation": true})).unwrap());
    assert!((want - got).length() < 1e-5, "{want} vs {got}");
}

#[test]
fn round_trip_material_and_camera_and_curve() {
    let mut s = session();
    s.run("polySphere", json!({"name": "ball"})).unwrap();
    s.run("shadingNode", json!({"args": ["blinn"], "asShader": true, "name": "shiny"})).unwrap();
    s.run("sets", json!({"renderable": true, "noSurfaceShader": true, "empty": true, "name": "shinySG"})).unwrap();
    s.run("connectAttr", json!({"args": ["shiny.outColor", "shinySG.surfaceShader"]})).unwrap();
    s.run("setAttr", json!({"args": ["shiny.color", 0.8, 0.1, 0.2]})).unwrap();
    s.run("sets", json!({"args": ["ball"], "edit": true, "forceElement": "shinySG"})).unwrap();
    s.run("camera", json!({"name": "shot", "focalLength": 50.0})).unwrap();
    s.run("curve", json!({"name": "crv", "degree": 3, "point": [[0,0,0],[1,1,0],[2,0,0],[3,1,0]]})).unwrap();
    s.run("nurbsPlane", json!({"name": "patch", "patchesU": 2, "patchesV": 3})).unwrap();
    let f = temp("shade.usda");
    s.run("mayaUSDExport", json!({"file": f})).unwrap();

    let mut b = session();
    b.run("mayaUSDImport", json!({"file": f})).unwrap();
    assert_eq!(b.run("nodeType", json!({"args": ["shiny"]})).unwrap(), json!("blinn"));
    let c = v3(&get(&mut b, "shiny.color"));
    assert!((c - DVec3::new(0.8, 0.1, 0.2)).length() < 1e-5, "{c}");
    let groups = b.run("listSets", json!({"object": "ballShape", "type": 1})).unwrap();
    assert_eq!(groups, json!(["shinySG"]));
    let fl = num(&get(&mut b, "shotShape.focalLength"));
    assert!((fl - 50.0).abs() < 1e-4);
    assert_eq!(num(&get(&mut b, "crvShape.degree")), 3.0);
    assert_eq!(b.run("nodeType", json!({"args": ["patchShape"]})).unwrap(), json!("nurbsSurface"));
}

#[test]
fn file_command_uses_the_usd_translators() {
    let mut s = session();
    s.run("polyCube", json!({"name": "box"})).unwrap();
    let f = temp("byfile.usd");
    s.run("file", json!({"args": [f], "exportAll": true, "type": "USD Export"})).unwrap();
    assert!(std::fs::read(&f).unwrap().starts_with(b"PXR-USDC"));
    let mut b = session();
    b.run("file", json!({"args": [f], "import": true, "type": "USD Import"})).unwrap();
    assert!(b.graph.find("box").unwrap().is_some());
    // By extension alone, and export of the selection.
    let mut c = session();
    c.run("file", json!({"args": [f], "import": true})).unwrap();
    assert!(c.graph.find("box").unwrap().is_some());
    c.run("polySphere", json!({"name": "ball"})).unwrap();
    let g = temp("sel.usda");
    c.run("select", json!({"args": ["box"]})).unwrap();
    c.run("file", json!({"args": [g], "exportSelected": true, "type": "USD Export"})).unwrap();
    let text = std::fs::read_to_string(&g).unwrap();
    assert!(text.contains("\"box\"") && !text.contains("\"ball\""));
}

#[test]
fn round_trip_skin_under_a_moved_group() {
    let mut s = session();
    s.run("polyPlane", json!({"name": "skinned", "subdivisionsX": 1, "subdivisionsY": 2, "width": 1, "height": 2})).unwrap();
    s.run("select", json!({"clear": true})).unwrap();
    s.run("joint", json!({"name": "hip", "position": [0.0, 0.0, -1.0]})).unwrap();
    s.run("joint", json!({"name": "tip", "position": [0.0, 0.0, 1.0]})).unwrap();
    s.run("skinCluster", json!({"args": ["hip", "tip", "skinned"]})).unwrap();
    s.run("group", json!({"args": ["hip", "skinned"], "name": "rig"})).unwrap();
    s.run("move", json!({"args": [7.0, 0.0, 0.0, "rig"], "relative": true})).unwrap();
    s.run("rotate", json!({"args": [0.0, 30.0, 0.0, "rig"], "relative": true})).unwrap();
    s.run("setAttr", json!({"args": ["tip.rx", 40.0]})).unwrap();
    let f = temp("skin_group.usda");
    s.run("mayaUSDExport", json!({"file": f, "exportSkels": "auto", "exportSkin": "auto"})).unwrap();
    let mut b = session();
    b.run("mayaUSDImport", json!({"file": f, "primPath": "/"})).unwrap();
    for i in 0..6 {
        let name = format!("skinned.vtx[{i}]");
        let pa = v3(&s.run("xform", json!({"args": [name], "query": true, "worldSpace": true, "translation": true})).unwrap());
        let pb = v3(&b.run("xform", json!({"args": [name], "query": true, "worldSpace": true, "translation": true})).unwrap());
        assert!((pa - pb).length() < 1e-3, "vtx {i}: {pa} vs {pb}");
    }
}

#[test]
fn textures_and_placement_round_trip_and_usdz_textures_come_out() {
    let png = temp("t.png");
    // A two pixel image is enough: the path is what is read.
    std::fs::write(&png, [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]).unwrap();
    let mut s = session();
    s.run("polyPlane", json!({"name": "m"})).unwrap();
    s.run("shadingNode", json!({"args": ["lambert"], "asShader": true, "name": "mat"})).unwrap();
    s.run("sets", json!({"renderable": true, "noSurfaceShader": true, "empty": true, "name": "matSG"})).unwrap();
    s.run("connectAttr", json!({"args": ["mat.outColor", "matSG.surfaceShader"]})).unwrap();
    s.run("shadingNode", json!({"args": ["file"], "asTexture": true, "name": "pic"})).unwrap();
    s.run("shadingNode", json!({"args": ["place2dTexture"], "asUtility": true, "name": "pl"})).unwrap();
    let (pic, pl) = (s.graph.find("pic").unwrap().unwrap(), s.graph.find("pl").unwrap().unwrap());
    aztek_scene::shading::connect_placement(&mut s.graph, pl, pic).unwrap();
    s.run("setAttr", json!({"args": ["pl.repeatU", 3.0]})).unwrap();
    s.run("setAttr", json!({"args": ["pic.fileTextureName", png], "type": "string"})).unwrap();
    s.run("connectAttr", json!({"args": ["pic.outColor", "mat.color"]})).unwrap();
    s.run("sets", json!({"args": ["m"], "edit": true, "forceElement": "matSG"})).unwrap();
    let f = temp("tex.usda");
    s.run("mayaUSDExport", json!({"file": f})).unwrap();
    let text = std::fs::read_to_string(&f).unwrap();
    assert!(text.contains("UsdUVTexture") && text.contains("UsdTransform2d"), "{text}");
    assert!(text.contains("@./t.png@"), "relative beside the file: {text}");

    let mut b = session();
    b.run("mayaUSDImport", json!({"file": f})).unwrap();
    let got = get(&mut b, "pic.fileTextureName");
    assert!(got.as_str().unwrap().ends_with("t.png"), "{got}");
    let links = b.run("listConnections", json!({"args": ["pic.uvCoord"], "source": true})).unwrap();
    let place = links[0].as_str().unwrap().to_string();
    assert_eq!(num(&get(&mut b, &format!("{place}.repeatU"))), 3.0);

    // In a package, the image comes out to a file.
    let z = temp("tex.usdz");
    s.run("mayaUSDExport", json!({"file": z})).unwrap();
    let mut c = session();
    c.run("mayaUSDImport", json!({"file": z})).unwrap();
    let got = get(&mut c, "pic.fileTextureName");
    let got = got.as_str().unwrap();
    assert!(std::path::Path::new(got).exists(), "{got}");
    assert_eq!(std::fs::read(got).unwrap().len(), 8);
}

/// Arnold's USD: materials of Arnold shaders (outputs:arnold:surface), and the options' background as its writer lays it
/// out, a RenderSettings string naming an ArnoldNodeGraph that reads a flat shader.
#[test]
fn arnold_materials_and_the_background_come_in() {
    let text = r#"#usda 1.0
(
    defaultPrim = "World"
)
def Xform "World"
{
    def Mesh "card" (
        prepend apiSchemas = ["MaterialBindingAPI"]
    )
    {
        int[] faceVertexCounts = [3]
        int[] faceVertexIndices = [0, 1, 2]
        point3f[] points = [(0, 0, 0), (1, 0, 0), (0, 1, 0)]
        rel material:binding = </mtl/backdrop>
    }
}
def Scope "mtl"
{
    def Material "backdrop"
    {
        token outputs:arnold:surface.connect = </mtl/backdrop/backdropFlat.outputs:surface>
        def Shader "backdropFlat"
        {
            uniform token info:id = "arnold:flat"
            color3f inputs:color = (0.2, 0.4, 0.6)
            token outputs:surface
        }
    }
    def Scope "background"
    {
        def ArnoldNodeGraph "nodeGraph"
        {
            token outputs:background.connect = </mtl/background/nodeGraph/bgFlat.outputs:out>
            def Shader "bgFlat"
            {
                uniform token info:id = "arnold:flat"
                color3f inputs:color = (1, 0.5, 0.25)
                token outputs:out
            }
        }
    }
}
def Scope "Render"
{
    def RenderSettings "settings"
    {
        string arnold:global:background = "/mtl/background/nodeGraph"
    }
}
"#;
    let f = write("arnold.usda", text);
    let mut s = session();
    s.run("mayaUSDImport", json!({"file": f})).unwrap();
    assert!((v3(&get(&mut s, "backdropFlat.color")) - DVec3::new(0.2, 0.4, 0.6)).length() < 1e-6);
    let sg = s.run("listConnections", json!({"args": ["backdropFlat.outColor"], "destination": true})).unwrap();
    assert_eq!(sg[0].as_str(), Some("backdrop"), "{sg}");
    let members = s.run("sets", json!({"args": ["backdrop"], "query": true})).unwrap();
    assert!(members.to_string().contains("card"), "{members}");
    let bg = s.run("listConnections", json!({"args": ["defaultArnoldRenderOptions.background"], "source": true, "plugs": true})).unwrap();
    assert_eq!(bg[0].as_str(), Some("bgFlat.outColor"), "{bg}");
    assert!((v3(&get(&mut s, "bgFlat.color")) - DVec3::new(1.0, 0.5, 0.25)).length() < 1e-6);
    assert!(s.graph.find("Render").unwrap().is_none() && s.graph.find("nodeGraph").unwrap().is_none(), "settings prims don't come in as transforms");
}
