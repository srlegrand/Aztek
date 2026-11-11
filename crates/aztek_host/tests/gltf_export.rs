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

    // Z up: a node over everything turns it Y up.
    c.mel("upAxis -ax z;");
    c.mel(&format!("exportGltf -all \"{}\";", at("zup.glb")));
    let (z, _) = glb(&dir.join("zup.glb"));
    let top = &z["nodes"][z["scenes"][0]["nodes"][0].as_u64().unwrap() as usize];
    assert_eq!(top["name"], json!("zUp"));
    let q: Vec<f64> = top["rotation"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
    assert!((q[0] + std::f64::consts::FRAC_1_SQRT_2).abs() < 1e-9 && (q[3] - std::f64::consts::FRAC_1_SQRT_2).abs() < 1e-9, "{q:?}");
    c.mel("upAxis -ax y;");

    assert!(c.mel_checked(&format!("select -cl; exportGltf \"{}\";", at("none.glb"))).is_err());
    assert!(c.mel_checked(&format!("exportGltf \"{}\" shotCam;", at("none.glb"))).is_err(), "a camera alone is nothing to write");
    let _ = std::fs::remove_dir_all(&dir);
}
