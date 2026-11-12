//! FBX and glTF import: a scene written out and read back is the scene it was.

use aztek_host::Core;
use serde_json::{json, Value};

fn at(dir: &std::path::Path, n: &str) -> String {
    dir.join(n).display().to_string().replace('\\', "/")
}

fn nums(v: &Value) -> Vec<f64> {
    fn walk(v: &Value, out: &mut Vec<f64>) {
        match v {
            Value::Array(a) => a.iter().for_each(|x| walk(x, out)),
            other => out.extend(other.as_f64()),
        }
    }
    let mut out = Vec::new();
    walk(v, &mut out);
    out
}

fn close(a: &[f64], b: &[f64], eps: f64) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < eps)
}

/// Each corner's point in the world with its UV, sorted: what a mesh looks like whatever order its parts are in.
fn corners(c: &mut Core, shape: &str) -> Vec<[i64; 5]> {
    let id = c.s.graph.find(shape).unwrap().unwrap_or_else(|| panic!("no {shape}"));
    let m = aztek_scene::modifiers::export_mesh(&mut c.s.graph, id).unwrap();
    let world = aztek_scene::rig::world(&mut c.s.graph, id);
    let uv = m.corner_attrs().get::<[f32; 2]>("uv0").to_vec();
    let mut out = Vec::new();
    for f in 0..m.num_faces() {
        for corner in m.face_corners(f) {
            let p = world.transform_point3(m.positions()[m.corner_point(corner) as usize]);
            let st = uv.get(corner as usize).copied().unwrap_or([0.0, 0.0]);
            let r = |x: f64| (x * 1000.0).round() as i64;
            out.push([r(p.x), r(p.y), r(p.z), r(st[0] as f64), r(st[1] as f64)]);
        }
    }
    out.sort_unstable();
    out
}

fn scene(c: &mut Core) {
    c.mel("polyCube -n box -w 2 -h 4 -d 6; group -n grp box; move 3 0 0 grp; move 0 5 0 box; rotate 0 90 0 box; scale 1 2 1 box;");
    c.mel("shadingNode -asShader blinn -n red; setAttr red.color 0.8 0.1 0.1; select -r box.f[0]; hyperShade -assign red;");
    c.mel("shadingNode -asShader lambert -n blue; setAttr blue.color 0.1 0.1 0.9; select -r box.f[1:5]; hyperShade -assign blue;");
    c.mel("select -cl; joint -n shoulder -p 1 2 3; joint -n elbow -p 1 6 3; setAttr elbow.rz 40; select -cl;");
}

#[test]
fn an_fbx_file_reads_back_as_the_scene_it_was_written_from() {
    let dir = std::env::temp_dir().join(format!("aztek_fbx_in_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut c = Core::new();
    scene(&mut c);
    let before = corners(&mut c, "boxShape");
    let bb = nums(&c.mel("xform -q -ws -bb box"));
    c.mel_checked(&format!("exportFbx \"{}\" grp shoulder;", at(&dir, "s.fbx"))).unwrap();

    c.mel("file -f -new;");
    let made = c.mel_checked(&format!("importFbx \"{}\";", at(&dir, "s.fbx"))).unwrap();
    assert_eq!(made, json!(["grp", "shoulder"]), "the top nodes, which are left selected");
    assert_eq!(c.mel("ls -sl"), json!(["grp", "shoulder"]));
    // The hierarchy and each node's place.
    assert_eq!(c.mel("listRelatives -p box"), json!(["grp"]));
    assert!(close(&nums(&c.mel("getAttr grp.translate")), &[3.0, 0.0, 0.0], 1e-9));
    assert!(close(&nums(&c.mel("getAttr box.rotate")), &[0.0, 90.0, 0.0], 1e-6), "{}", c.mel("getAttr box.rotate"));
    assert!(close(&nums(&c.mel("getAttr box.scale")), &[1.0, 2.0, 1.0], 1e-9));
    assert!(close(&nums(&c.mel("xform -q -ws -bb box")), &bb, 1e-6));
    // The mesh: six quads, every corner where it was with the UV it had.
    assert_eq!(c.mel("polyEvaluate -f box"), json!(6));
    assert_eq!(c.mel("polyEvaluate -v box"), json!(8));
    assert_eq!(corners(&mut c, "boxShape"), before);
    // The materials, with their colours, on the faces that wore them.
    assert!(close(&nums(&c.mel("getAttr red.color")), &[0.8, 0.1, 0.1], 1e-6));
    // As the reference: a material with a highlight goes out as phong and comes back a phong.
    assert_eq!(c.mel("nodeType red"), json!("phong"));
    assert_eq!(c.mel("nodeType blue"), json!("lambert"));
    assert_eq!(c.mel("sets -q redSG"), json!(["box.f[0]"]));
    assert_eq!(c.mel("sets -q blueSG"), json!(["box.f[1:5]"]));
    // Joints are joints, posed as they were.
    assert_eq!(c.mel("nodeType elbow"), json!("joint"));
    assert_eq!(c.mel("listRelatives -p elbow"), json!(["shoulder"]));
    assert!(close(&nums(&c.mel("getAttr shoulder.translate")), &[1.0, 2.0, 3.0], 1e-9));
    assert!(close(&nums(&c.mel("getAttr elbow.translate")), &[0.0, 4.0, 0.0], 1e-9));
    assert!((nums(&c.mel("getAttr elbow.rz"))[0] - 40.0).abs() < 1e-6);
    // One undo takes the whole import back.
    c.mel("undo;");
    assert_eq!(c.mel("ls box grp shoulder red"), json!([]));
    c.mel("redo;");
    assert_eq!(c.mel("polyEvaluate -f box"), json!(6));

    // Into a Z-up scene the Y-up file's top nodes are turned, as the reference converts axes: what was 5 up Y is 5 up
    // Z, the group is turned a quarter about X, and no node is added.
    c.mel("file -f -new; upAxis -ax z;");
    c.mel_checked(&format!("FBXImport -f \"{}\";", at(&dir, "s.fbx"))).unwrap();
    let z = nums(&c.mel("xform -q -ws -bb box"));
    let want = [bb[0], -bb[5], bb[1], bb[3], -bb[2], bb[4]];
    assert!(close(&z, &want, 1e-6), "{z:?} against {want:?}");
    assert_eq!(c.mel("listRelatives -p grp"), json!([]));
    assert!(close(&nums(&c.mel("getAttr grp.rotate")), &[90.0, 0.0, 0.0], 1e-6), "{}", c.mel("getAttr grp.rotate"));
    assert!(close(&nums(&c.mel("getAttr box.rotate")), &[0.0, 90.0, 0.0], 1e-6), "only the top is turned");
    c.mel("upAxis -ax y;");

    assert!(c.mel_checked(&format!("importFbx \"{}\";", at(&dir, "none.fbx"))).is_err());
    std::fs::write(dir.join("text.fbx"), b"; FBX 7.4.0 project file\n").unwrap();
    assert!(c.mel_checked(&format!("importFbx \"{}\";", at(&dir, "text.fbx"))).unwrap_err().contains("ASCII"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_gltf_file_reads_back_as_the_scene_it_was_written_from() {
    let dir = std::env::temp_dir().join(format!("aztek_gltf_in_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut c = Core::new();
    scene(&mut c);
    // Bound, so the joints go out with the mesh.
    c.mel("polyCylinder -n arm -r 1 -h 4 -sy 4; move 1 4 3 arm; skinCluster shoulder elbow arm; select -cl;");
    let bb = nums(&c.mel("xform -q -ws -bb box"));
    let before = corners(&mut c, "boxShape");
    for name in ["s.glb", "s.gltf"] {
        c.mel_checked(&format!("exportGltf \"{}\" grp arm;", at(&dir, name))).unwrap();
    }
    for name in ["s.glb", "s.gltf"] {
        c.mel("file -f -new;");
        let made = c.mel_checked(&format!("importGltf \"{}\";", at(&dir, name))).unwrap();
        let tops: Vec<&str> = made.as_array().unwrap().iter().filter_map(Value::as_str).collect();
        assert!(tops.contains(&"grp") && tops.contains(&"arm") && tops.contains(&"shoulder"), "{name}: {made}");
        // Metres in the file, centimetres here.
        assert_eq!(c.mel("listRelatives -p box"), json!(["grp"]));
        assert!(close(&nums(&c.mel("getAttr grp.translate")), &[3.0, 0.0, 0.0], 1e-6), "{name}");
        assert!(close(&nums(&c.mel("xform -q -ws -bb box")), &bb, 1e-4), "{name}: {}", c.mel("xform -q -ws -bb box"));
        // Triangles, since that is what the format holds: twelve of them, the corners and UVs of the quads' own.
        assert_eq!(c.mel("polyEvaluate -f box"), json!(12));
        let after = corners(&mut c, "boxShape");
        let mut seen = after.clone();
        seen.dedup();
        let mut had = before.clone();
        had.dedup();
        assert_eq!(seen, had, "{name}: every corner of the quads is a corner of the triangles, with its UV");
        // Materials come in as standard surfaces with the colour and roughness written.
        assert_eq!(c.mel("nodeType red"), json!("standardSurface"));
        assert!(close(&nums(&c.mel("getAttr red.baseColor")), &[0.8, 0.1, 0.1], 1e-5), "{name}");
        assert_eq!(nums(&c.mel("polyEvaluate -f box"))[0] as usize, 12);
        let red: Vec<String> = c.mel("sets -q redSG").as_array().unwrap().iter().filter_map(|v| v.as_str().map(str::to_string)).collect();
        c.mel(&format!("select -r {};", red.join(" ")));
        assert_eq!(c.mel("polyEvaluate -fc"), json!(2), "{name}: the one red quad is two red triangles");
        // The skin's joints are joints, in their hierarchy.
        assert_eq!(c.mel("nodeType elbow"), json!("joint"));
        assert_eq!(c.mel("listRelatives -p elbow"), json!(["shoulder"]));
        assert!(close(&nums(&c.mel("getAttr elbow.translate")), &[0.0, 4.0, 0.0], 1e-6), "{name}");
    }
    // Into a Z-up scene: 5 up Y in the file is 5 up Z here.
    c.mel("file -f -new; upAxis -ax z;");
    c.mel_checked(&format!("importGltf \"{}\";", at(&dir, "s.glb"))).unwrap();
    let z = nums(&c.mel("xform -q -ws -bb box"));
    let want = [bb[0], -bb[5], bb[1], bb[3], -bb[2], bb[4]];
    assert!(close(&z, &want, 1e-4), "{z:?} against {want:?}");
    c.mel("upAxis -ax y;");
    assert!(c.mel_checked(&format!("importGltf \"{}\";", at(&dir, "none.glb"))).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

/// A skinned, textured character as the reference's FBX plug-in brings one in: joints with their orients, a
/// skinCluster with the file's weights and bind pose, lambert and phong materials with their file nodes, a normal map
/// through a bump2d, textures found in the .fbm folder, and the FBX settings commands deciding what comes in.
#[test]
fn a_skinned_textured_fbx_comes_in_as_the_reference_brings_it() {
    let dir = std::env::temp_dir().join(format!("aztek_fbx_char_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("skin_d.png"), b"not really a picture").unwrap();
    std::fs::write(dir.join("skin_n.png"), b"not really a picture").unwrap();
    let mut c = Core::new();
    c.mel("FBXResetImport; FBXResetExport;");
    c.mel("polyCylinder -n arm -r 1 -h 8 -sy 8; move 0 4 0 arm; select -cl; joint -n shoulder -p 0 0 0; joint -n elbow -p 0 4 0; joint -n wrist -p 0 8 0;");
    c.mel("setAttr shoulder.jointOrient 0 0 30; setAttr shoulder.rotateOrder 3; setAttr elbow.rotateAxis 10 0 0;");
    c.mel("skinCluster -n armSkin shoulder elbow wrist arm;");
    c.mel(&format!("shadingNode -asShader phong -n shiny; shadingNode -asTexture file -n skinColor; setAttr -type \"string\" skinColor.fileTextureName \"{}\";", at(&dir, "skin_d.png")));
    c.mel("connectAttr skinColor.outColor shiny.color;");
    c.mel(&format!("shadingNode -asTexture file -n skinNormal; setAttr -type \"string\" skinNormal.fileTextureName \"{}\";", at(&dir, "skin_n.png")));
    c.mel("shadingNode -asUtility bump2d -n nm; setAttr nm.bumpInterp 1; connectAttr skinNormal.outAlpha nm.bumpValue; connectAttr nm.outNormal shiny.normalCamera;");
    c.mel("select -r arm; hyperShade -assign shiny; setAttr elbow.rz 40;");
    let weights = nums(&c.mel("skinPercent -q -v armSkin arm.vtx[20]"));
    let bb = nums(&c.mel("xform -q -ws -bb arm"));
    let jo = nums(&c.mel("getAttr shoulder.jointOrient"));
    c.mel_checked(&format!("file -force -options \"v=0\" -type \"FBX export\" -exportAll \"{}\";", at(&dir, "char.fbx"))).unwrap();

    // The textures move into the .fbm folder beside the file, and the file's own paths no longer lead anywhere.
    let fbm = dir.join("char.fbm");
    std::fs::create_dir_all(&fbm).unwrap();
    std::fs::rename(dir.join("skin_d.png"), fbm.join("skin_d.png")).unwrap();
    std::fs::rename(dir.join("skin_n.png"), fbm.join("skin_n.png")).unwrap();

    c.mel("file -f -new;");
    c.mel_checked(&format!("file -import -type \"FBX\" \"{}\";", at(&dir, "char.fbx"))).unwrap();
    // Joints keep their orient, rotate order and rotate axis, and stand as posed.
    assert_eq!(c.mel("nodeType elbow"), json!("joint"));
    assert!(close(&nums(&c.mel("getAttr shoulder.jointOrient")), &jo, 1e-6), "{}", c.mel("getAttr shoulder.jointOrient"));
    assert_eq!(c.mel("getAttr shoulder.rotateOrder"), json!(3));
    assert!(close(&nums(&c.mel("getAttr elbow.rotateAxis")), &[10.0, 0.0, 0.0], 1e-6), "{}", c.mel("getAttr elbow.rotateAxis"));
    assert!((nums(&c.mel("getAttr elbow.rz"))[0] - 40.0).abs() < 1e-6);
    // The skin: the file's influences and weights, and the mesh bent as it was.
    let sk = c.mel("ls -type skinCluster");
    assert_eq!(sk, json!(["skinCluster1"]));
    assert_eq!(c.mel("skinCluster -q -inf skinCluster1"), json!(["shoulder", "elbow", "wrist"]));
    assert!(close(&nums(&c.mel("skinPercent -q -v skinCluster1 arm.vtx[20]")), &weights, 1e-6));
    assert!(close(&nums(&c.mel("xform -q -ws -bb arm")), &bb, 1e-4), "{} against {bb:?}", c.mel("xform -q -ws -bb arm"));
    assert_eq!(c.mel("ls -type dagPose"), json!(["bindPose1"]));
    // The material and what feeds it.
    assert_eq!(c.mel("nodeType shiny"), json!("phong"));
    assert_eq!(c.mel("listConnections -s 1 -d 0 shiny.color"), json!(["skinColor"]));
    assert_eq!(c.mel("getAttr skinColor.fileTextureName"), json!(at(&fbm, "skin_d.png")));
    assert_eq!(c.mel("listConnections -s 1 -d 0 skinColor.uvCoord"), json!(["place2dTexture1"]));
    let bump = c.mel("listConnections -s 1 -d 0 shiny.normalCamera");
    assert_eq!(c.mel(&format!("getAttr {}.bumpInterp", bump[0].as_str().unwrap())), json!(1), "tangent space normals");
    assert_eq!(c.mel(&format!("listConnections -s 1 -d 0 {}.bumpValue", bump[0].as_str().unwrap())), json!(["skinNormal"]));
    assert_eq!(c.mel("sets -q shinySG"), json!(["armShape"]));

    // Add and update: the same file again makes nothing new.
    c.mel_checked(&format!("FBXImport -f \"{}\";", at(&dir, "char.fbx"))).unwrap();
    assert_eq!(c.mel("ls -type joint").as_array().unwrap().len(), 3);
    // Add makes it all again.
    c.mel("FBXImportMode -v add;");
    c.mel_checked(&format!("FBXImport -f \"{}\";", at(&dir, "char.fbx"))).unwrap();
    assert_eq!(c.mel("ls -type joint").as_array().unwrap().len(), 6);
    assert_eq!(c.mel("FBXImportMode -q"), json!("add"));

    // Without skins the mesh comes in unbound; the setting answers a query.
    c.mel("file -f -new; FBXResetImport; FBXImportSkins -v false;");
    assert_eq!(c.mel("FBXImportSkins -q"), json!(false));
    c.mel_checked(&format!("FBXImport -f \"{}\";", at(&dir, "char.fbx"))).unwrap();
    assert_eq!(c.mel("ls -type skinCluster"), json!([]));
    c.mel("FBXResetImport;");
    assert_eq!(c.mel("FBXImportSkins -q"), json!(true));

    // Written Z up, the top node is turned a quarter about X in the file; read back into the Y-up scene it turns back.
    c.mel("file -f -new; FBXExportUpAxis z; polyCube -n box; group -n grp box; move 0 5 0 box;");
    let bb = nums(&c.mel("xform -q -ws -bb box"));
    c.mel_checked(&format!("FBXExport -f \"{}\";", at(&dir, "z.fbx"))).unwrap();
    c.mel("FBXResetExport; file -f -new;");
    c.mel_checked(&format!("FBXImport -f \"{}\";", at(&dir, "z.fbx"))).unwrap();
    assert!(close(&nums(&c.mel("getAttr grp.rotate")), &[0.0, 0.0, 0.0], 1e-6), "{}", c.mel("getAttr grp.rotate"));
    assert!(close(&nums(&c.mel("xform -q -ws -bb box")), &bb, 1e-6));
    // Without axis conversion it stays as the file has it.
    c.mel("file -f -new; FBXImportAxisConversionEnable -v false;");
    c.mel_checked(&format!("FBXImport -f \"{}\";", at(&dir, "z.fbx"))).unwrap();
    assert!(close(&nums(&c.mel("getAttr grp.rotate")), &[90.0, 0.0, 0.0], 1e-6), "{}", c.mel("getAttr grp.rotate"));
    c.mel("FBXResetImport;");
    let _ = std::fs::remove_dir_all(&dir);
}
