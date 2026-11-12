//! glTF 2.0 export: the file's structure, its units and axes, materials and UV sets.

use aztek_host::Core;
use serde_json::{json, Value};

fn glb(path: &std::path::Path) -> (Value, Vec<u8>) {
    let raw = std::fs::read(path).unwrap();
    let word = |at: usize| u32::from_le_bytes(raw[at..at + 4].try_into().unwrap()) as usize;
    assert_eq!(&raw[..4], b"glTF");
    assert_eq!(word(4), 2);
    assert_eq!(word(8), raw.len(), "the header's length is the file's");
    let n = word(12);
    assert_eq!(&raw[16..20], b"JSON");
    assert_eq!(n % 4, 0);
    let doc: Value = serde_json::from_slice(&raw[20..20 + n]).unwrap();
    let at = 20 + n;
    let m = word(at);
    assert_eq!(&raw[at + 4..at + 8], b"BIN\0");
    assert_eq!(at + 8 + m, raw.len());
    (doc, raw[at + 8..].to_vec())
}

fn floats(doc: &Value, bin: &[u8], accessor: usize) -> Vec<f32> {
    let a = &doc["accessors"][accessor];
    let v = &doc["bufferViews"][a["bufferView"].as_u64().unwrap() as usize];
    let width = match a["type"].as_str().unwrap() {
        "VEC2" => 2,
        "VEC3" => 3,
        "VEC4" => 4,
        _ => 1,
    };
    let at = v["byteOffset"].as_u64().unwrap() as usize;
    assert_eq!(at % 4, 0);
    (0..a["count"].as_u64().unwrap() as usize * width).map(|k| f32::from_le_bytes(bin[at + k * 4..at + k * 4 + 4].try_into().unwrap())).collect()
}

#[test]
fn a_hierarchy_goes_out_in_metres_with_its_materials_and_uv_sets() {
    let dir = std::env::temp_dir().join(format!("aztek_gltf_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let at = |name: &str| dir.join(name).to_string_lossy().replace('\\', "/");
    let mut c = Core::new();
    c.mel("group -em -n rig; polyCube -n box -w 200 -h 100 -d 50; parent box rig; setAttr box.tx 300; polySphere -n ball; parent ball box; setAttr ball.ty 400;");
    c.mel("shadingNode -asShader lambert -n red; setAttr red.color -type double3 0.8 0.1 0.1; setAttr red.transparency -type double3 0.25 0.25 0.25; select box.f[0:1]; hyperShade -assign red;");
    c.mel("polyUVSet -cp -uvs \"map1\" -nuv \"lightmap\" box; polyUVSet -cuv -uvs \"lightmap\" box;");
    c.mel("camera -n shotCam; spaceLocator -n marker; parent marker rig;");
    let r = c.mel(&format!("select rig; exportGltf \"{}\";", at("out.glb")));
    assert_eq!(r["meshes"], json!(2), "{r}");
    assert_eq!(r["nodes"], json!(3), "the locator and the camera have nothing to write: {r}");

    let (doc, bin) = glb(&dir.join("out.glb"));
    assert_eq!(doc["asset"]["version"], json!("2.0"));
    assert_eq!(doc["buffers"][0]["byteLength"], json!(bin.len()));
    let nodes = doc["nodes"].as_array().unwrap();
    let node = |name: &str| nodes.iter().find(|n| n["name"] == name).unwrap_or_else(|| panic!("no node {name}"));
    // One root in the scene, the box under it, the ball under the box; centimetres became metres.
    let roots = doc["scenes"][0]["nodes"].as_array().unwrap();
    assert_eq!(roots.len(), 1);
    assert_eq!(nodes[roots[0].as_u64().unwrap() as usize]["name"], json!("rig"));
    assert_eq!(node("box")["translation"], json!([3.0, 0.0, 0.0]));
    // Parented where it stood, at the world origin: three metres back from the box, then four up.
    assert_eq!(node("ball")["translation"], json!([-3.0, 4.0, 0.0]));
    assert_eq!(node("box")["children"].as_array().unwrap().len(), 1);

    let mesh = &doc["meshes"][node("box")["mesh"].as_u64().unwrap() as usize];
    let prims = mesh["primitives"].as_array().unwrap();
    assert_eq!(prims.len(), 2, "a primitive for each material on the box");
    let attrs = &prims[0]["attributes"];
    let pos = &doc["accessors"][attrs["POSITION"].as_u64().unwrap() as usize];
    assert_eq!(pos["min"], json!([-1.0, -0.5, -0.25]));
    assert_eq!(pos["max"], json!([1.0, 0.5, 0.25]));
    assert!(attrs.get("NORMAL").is_some() && attrs.get("TEXCOORD_0").is_some() && attrs.get("TEXCOORD_1").is_some(), "{attrs}");
    // Two sets with the same UVs: the first is map1 whichever is current, and V is turned over.
    let (uv0, uv1) = (floats(&doc, &bin, attrs["TEXCOORD_0"].as_u64().unwrap() as usize), floats(&doc, &bin, attrs["TEXCOORD_1"].as_u64().unwrap() as usize));
    assert_eq!(uv0, uv1);
    assert!(uv0.iter().all(|v| (-1e-6..=1.0 + 1e-6).contains(v)));
    let mut triangles = 0;
    for p in prims {
        let i = &doc["accessors"][p["indices"].as_u64().unwrap() as usize];
        assert_eq!(i["componentType"], json!(5125));
        triangles += i["count"].as_u64().unwrap() / 3;
    }
    assert_eq!(triangles, 12);

    let materials = doc["materials"].as_array().unwrap();
    let red = materials.iter().find(|m| m["name"] == "red").expect("the red material");
    let base: Vec<f64> = red["pbrMetallicRoughness"]["baseColorFactor"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
    assert!((base[0] - 0.8).abs() < 1e-6 && (base[3] - 0.75).abs() < 1e-6, "{base:?}");
    assert_eq!(red["alphaMode"], json!("BLEND"));
    assert_eq!(red["pbrMetallicRoughness"]["roughnessFactor"], json!(1.0));

    // JSON: the same, with its buffer inside it.
    c.mel(&format!("exportGltf \"{}\" box;", at("box.gltf")));
    let text: Value = serde_json::from_slice(&std::fs::read(dir.join("box.gltf")).unwrap()).unwrap();
    assert!(text["buffers"][0]["uri"].as_str().unwrap().starts_with("data:application/octet-stream;base64,"));
    // The box alone sits where it is in the world.
    assert_eq!(text["nodes"].as_array().unwrap().iter().find(|n| n["name"] == "box").unwrap()["translation"], json!([3.0, 0.0, 0.0]));

    assert!(c.mel_checked(&format!("select -cl; exportGltf \"{}\";", at("none.glb"))).is_err());
    assert!(c.mel_checked(&format!("exportGltf \"{}\" shotCam;", at("none.glb"))).is_err(), "a camera alone is nothing to write");
    let _ = std::fs::remove_dir_all(&dir);
}

fn node_named<'a>(doc: &'a Value, name: &str) -> &'a Value {
    doc["nodes"].as_array().unwrap().iter().find(|n| n["name"] == name).unwrap_or_else(|| panic!("no node {name}"))
}

fn triple(v: &Value) -> [f64; 3] {
    let a = v.as_array().expect("three numbers");
    [a[0].as_f64().unwrap(), a[1].as_f64().unwrap(), a[2].as_f64().unwrap()]
}

fn close(a: [f64; 3], b: [f64; 3]) -> bool {
    (0..3).all(|k| (a[k] - b[k]).abs() < 1e-5)
}

// Issue 26: a Z-up scene went out with a node over everything carrying a quarter turn. The data is converted instead:
// what was up Z is up Y, on the node itself and in its points, and nothing is added.
#[test]
fn a_z_up_scene_is_converted_with_no_node_added_and_can_be_kept_z_up() {
    let dir = std::env::temp_dir().join(format!("aztek_gltf_up_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let at = |n: &str| dir.join(n).display().to_string().replace('\\', "/");
    let mut c = Core::new();
    c.mel("upAxis -ax z;");
    c.mel("polyCube -n box -w 2 -h 4 -d 6; move 10 20 30 box; scale 2 2 2 box;");

    c.mel(&format!("exportGltf \"{}\" box;", at("y.glb")));
    let (y, bin) = glb(&dir.join("y.glb"));
    assert_eq!(y["nodes"].as_array().unwrap().len(), 1, "the box and nothing over it");
    let n = node_named(&y, "box");
    // Scene (x, y, z) is the file's (x, z, -y), in metres.
    assert!(close(triple(&n["translation"]), [0.1, 0.3, -0.2]), "{}", n["translation"]);
    assert!(n.get("rotation").is_none(), "no turn the scene did not have: {n}");
    assert!(close(triple(&n["scale"]), [2.0, 2.0, 2.0]));
    let p = &y["meshes"][0]["primitives"][0];
    let a = &y["accessors"][p["attributes"]["POSITION"].as_u64().unwrap() as usize];
    let pos = floats(&y, &bin, p["attributes"]["POSITION"].as_u64().unwrap() as usize);
    assert_eq!(pos.len() % 3, 0);
    let extent = |k: usize| a["max"][k].as_f64().unwrap() - a["min"][k].as_f64().unwrap();
    // In a Z-up scene a cube's height runs up Z and its depth along Y: 2 across, 4 up, 6 deep in the file.
    assert!((extent(0) - 0.02).abs() < 1e-6 && (extent(1) - 0.04).abs() < 1e-6 && (extent(2) - 0.06).abs() < 1e-6, "{a}");
    // Normals turn with the points: the top face's point up Y.
    let normals = floats(&y, &bin, p["attributes"]["NORMAL"].as_u64().unwrap() as usize);
    assert!(pos.chunks(3).zip(normals.chunks(3)).any(|(q, n)| q[1] > 0.019 && n[1] > 0.99));

    // -upAxis z: the scene as it is.
    c.mel(&format!("exportGltf -upAxis z \"{}\" box;", at("z.glb")));
    let (z, _) = glb(&dir.join("z.glb"));
    assert_eq!(z["nodes"].as_array().unwrap().len(), 1);
    assert!(close(triple(&node_named(&z, "box")["translation"]), [0.1, 0.2, 0.3]));
    let a = &z["accessors"][z["meshes"][0]["primitives"][0]["attributes"]["POSITION"].as_u64().unwrap() as usize];
    assert!((a["max"][2].as_f64().unwrap() - 0.02).abs() < 1e-6, "{a}");

    // And the other way: a Y-up scene written Z up.
    c.mel("upAxis -ax y;");
    c.mel(&format!("exportGltf -upAxis z \"{}\" box;", at("yz.glb")));
    let (yz, _) = glb(&dir.join("yz.glb"));
    assert!(close(triple(&node_named(&yz, "box")["translation"]), [0.1, -0.3, 0.2]), "{}", node_named(&yz, "box")["translation"]);
    assert!(c.mel_checked(&format!("exportGltf -upAxis x \"{}\" box;", at("x.glb"))).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

// Issue 26: joints and skinning did not go out at all.
#[test]
fn a_bound_mesh_goes_out_with_its_joints_weights_and_bind_matrices() {
    let dir = std::env::temp_dir().join(format!("aztek_gltf_skin_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let at = |n: &str| dir.join(n).display().to_string().replace('\\', "/");
    let mut c = Core::new();
    c.mel("polyCylinder -n arm -r 1 -h 8 -sy 8; move 0 4 0 arm; select -cl; joint -n shoulder -p 0 0 0; joint -n elbow -p 0 4 0; joint -n wrist -p 0 8 0;");
    c.mel("skinCluster shoulder elbow wrist arm;");
    // Posed: the file holds the bind pose in the mesh and the pose on the joints.
    c.mel("setAttr elbow.rz 40;");
    let r = c.mel(&format!("exportGltf \"{}\" arm;", at("arm.glb")));
    assert_eq!(r["skins"], json!(1), "{r}");
    let (doc, bin) = glb(&dir.join("arm.glb"));

    // The joints are nodes, in their hierarchy, though only the mesh was asked for.
    let shoulder = node_named(&doc, "shoulder");
    let elbow = node_named(&doc, "elbow");
    let index_of = |name: &str| doc["nodes"].as_array().unwrap().iter().position(|n| n["name"] == name).unwrap();
    assert_eq!(shoulder["children"], json!([index_of("elbow")]));
    assert_eq!(elbow["children"], json!([index_of("wrist")]));
    assert!(close(triple(&elbow["translation"]), [0.0, 0.04, 0.0]));
    assert!(elbow.get("rotation").is_some(), "the pose is on the joint");
    let roots: Vec<u64> = doc["scenes"][0]["nodes"].as_array().unwrap().iter().map(|v| v.as_u64().unwrap()).collect();
    assert!(roots.contains(&(index_of("shoulder") as u64)) && roots.contains(&(index_of("arm") as u64)), "{roots:?}");

    let skin = &doc["skins"][0];
    assert_eq!(skin["joints"], json!([index_of("shoulder"), index_of("elbow"), index_of("wrist")]));
    assert_eq!(node_named(&doc, "arm")["skin"], json!(0));
    let ibm = &doc["accessors"][skin["inverseBindMatrices"].as_u64().unwrap() as usize];
    assert_eq!((ibm["type"].as_str().unwrap(), ibm["count"].as_u64().unwrap()), ("MAT4", 3));
    let view = &doc["bufferViews"][ibm["bufferView"].as_u64().unwrap() as usize];
    assert!(view.get("target").is_none(), "bind matrices are not vertex data");
    let m_at = view["byteOffset"].as_u64().unwrap() as usize;
    let mats: Vec<f32> = (0..48).map(|k| f32::from_le_bytes(bin[m_at + k * 4..m_at + k * 4 + 4].try_into().unwrap())).collect();

    let p = &doc["meshes"][0]["primitives"][0]["attributes"];
    let pos = floats(&doc, &bin, p["POSITION"].as_u64().unwrap() as usize);
    let weights = floats(&doc, &bin, p["WEIGHTS_0"].as_u64().unwrap() as usize);
    let ja = &doc["accessors"][p["JOINTS_0"].as_u64().unwrap() as usize];
    assert_eq!((ja["componentType"].as_u64().unwrap(), ja["type"].as_str().unwrap()), (5123, "VEC4"));
    let jv = &doc["bufferViews"][ja["bufferView"].as_u64().unwrap() as usize];
    let j_at = jv["byteOffset"].as_u64().unwrap() as usize;
    let count = ja["count"].as_u64().unwrap() as usize;
    assert_eq!(count * 3, pos.len());
    assert_eq!(count * 4, weights.len());
    // The mesh is in its bind pose: a straight cylinder, 8 cm tall, whatever the elbow does.
    let a = &doc["accessors"][p["POSITION"].as_u64().unwrap() as usize];
    assert!((a["max"][0].as_f64().unwrap() - 0.01).abs() < 1e-5 && (a["min"][0].as_f64().unwrap() + 0.01).abs() < 1e-5, "{a}");
    for v in 0..count {
        let w = &weights[v * 4..v * 4 + 4];
        assert!((w.iter().sum::<f32>() - 1.0).abs() < 1e-5, "weights sum to one: {w:?}");
        // With the joints where they were bound, a point skins back to where it is: the sum of weight times the
        // joint's bind world times its inverse bind, on the point, is the point in the world. The joints were bound
        // at (0, 0, 0), (0, 4, 0) and (0, 8, 0) cm with no turn, and the mesh sat 4 cm up.
        let q = [pos[v * 3] as f64, pos[v * 3 + 1] as f64, pos[v * 3 + 2] as f64];
        let mut out = [0.0f64; 3];
        for k in 0..4 {
            let joint = u16::from_le_bytes(bin[j_at + (v * 4 + k) * 2..j_at + (v * 4 + k) * 2 + 2].try_into().unwrap()) as usize;
            assert!(joint < 3);
            let m = &mats[joint * 16..joint * 16 + 16];
            let bind_y = joint as f64 * 0.04;
            for r in 0..3 {
                let local = m[r] as f64 * q[0] + m[4 + r] as f64 * q[1] + m[8 + r] as f64 * q[2] + m[12 + r] as f64;
                out[r] += w[k] as f64 * (local + if r == 1 { bind_y } else { 0.0 });
            }
        }
        assert!(close(out, [q[0], q[1] + 0.04, q[2]]), "point {v}: {out:?} from {q:?}");
    }
    // Both halves of the arm hold weight: the top follows the elbow or wrist, the bottom the shoulder.
    let top_joint = (0..count).filter(|&v| pos[v * 3 + 1] > 0.035).map(|v| u16::from_le_bytes(bin[j_at + v * 8..j_at + v * 8 + 2].try_into().unwrap())).max().unwrap();
    assert!(top_joint >= 1);
    let _ = std::fs::remove_dir_all(&dir);
}
