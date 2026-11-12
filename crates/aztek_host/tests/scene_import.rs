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
    assert_eq!(c.mel("nodeType red"), json!("blinn"));
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

    // Into a Z-up scene the Y-up file changes basis: what was 5 up Y is 5 up Z, and no node is added.
    c.mel("file -f -new; upAxis -ax z;");
    c.mel_checked(&format!("FBXImport -f \"{}\";", at(&dir, "s.fbx"))).unwrap();
    let z = nums(&c.mel("xform -q -ws -bb box"));
    let want = [bb[0], -bb[5], bb[1], bb[3], -bb[2], bb[4]];
    assert!(close(&z, &want, 1e-6), "{z:?} against {want:?}");
    assert_eq!(c.mel("listRelatives -p grp"), json!([]));
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
