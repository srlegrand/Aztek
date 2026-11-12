//! FBX export: the file read back record by record, and the plug-in that brings the commands.

use aztek_host::Core;
use serde_json::json;

#[derive(Debug, Clone, PartialEq)]
enum P {
    I(i64),
    D(f64),
    S(String),
    Raw(Vec<u8>),
    Ints(Vec<i32>),
    Doubles(Vec<f64>),
}

#[derive(Debug, Clone)]
struct Rec {
    name: String,
    props: Vec<P>,
    kids: Vec<Rec>,
}

impl Rec {
    fn kid(&self, name: &str) -> &Rec {
        self.kids.iter().find(|k| k.name == name).unwrap_or_else(|| panic!("no {name} in {}", self.name))
    }
    fn all(&self, name: &str) -> Vec<&Rec> {
        self.kids.iter().filter(|k| k.name == name).collect()
    }
    fn s(&self, k: usize) -> &str {
        match &self.props[k] {
            P::S(s) => s,
            other => panic!("{}: {other:?} is not text", self.name),
        }
    }
    fn i(&self, k: usize) -> i64 {
        match &self.props[k] {
            P::I(v) => *v,
            other => panic!("{}: {other:?} is not a whole number", self.name),
        }
    }
    fn ints(&self) -> &[i32] {
        match &self.props[0] {
            P::Ints(v) => v,
            other => panic!("{}: {other:?}", self.name),
        }
    }
    fn doubles(&self) -> &[f64] {
        match &self.props[0] {
            P::Doubles(v) => v,
            other => panic!("{}: {other:?}", self.name),
        }
    }
}

fn u32_at(b: &[u8], at: usize) -> usize {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap()) as usize
}

/// One record at `at`, or None at the empty record that closes a list. Returns where the next begins.
fn read_rec(b: &[u8], at: usize) -> (Option<Rec>, usize) {
    let (end, count, props_len, name_len) = (u32_at(b, at), u32_at(b, at + 4), u32_at(b, at + 8), b[at + 12] as usize);
    if end == 0 {
        assert!(b[at..at + 13].iter().all(|x| *x == 0), "the closing record is thirteen zeros");
        return (None, at + 13);
    }
    let name = String::from_utf8(b[at + 13..at + 13 + name_len].to_vec()).unwrap();
    let mut p = at + 13 + name_len;
    let props_end = p + props_len;
    let mut props = Vec::new();
    for _ in 0..count {
        let code = b[p];
        p += 1;
        match code {
            b'I' => {
                props.push(P::I(i32::from_le_bytes(b[p..p + 4].try_into().unwrap()) as i64));
                p += 4;
            }
            b'L' => {
                props.push(P::I(i64::from_le_bytes(b[p..p + 8].try_into().unwrap())));
                p += 8;
            }
            b'D' => {
                props.push(P::D(f64::from_le_bytes(b[p..p + 8].try_into().unwrap())));
                p += 8;
            }
            b'S' | b'R' => {
                let n = u32_at(b, p);
                let bytes = b[p + 4..p + 4 + n].to_vec();
                props.push(if code == b'S' { P::S(String::from_utf8(bytes).unwrap()) } else { P::Raw(bytes) });
                p += 4 + n;
            }
            b'i' | b'd' => {
                let (n, packed, len) = (u32_at(b, p), u32_at(b, p + 4), u32_at(b, p + 8));
                assert_eq!(packed, 0, "arrays are written plain");
                let width = if code == b'i' { 4 } else { 8 };
                assert_eq!(len, n * width);
                let data = &b[p + 12..p + 12 + len];
                props.push(if code == b'i' {
                    P::Ints(data.chunks(4).map(|c| i32::from_le_bytes(c.try_into().unwrap())).collect())
                } else {
                    P::Doubles(data.chunks(8).map(|c| f64::from_le_bytes(c.try_into().unwrap())).collect())
                });
                p += 12 + len;
            }
            other => panic!("{name}: unknown value type {}", other as char),
        }
    }
    assert_eq!(p, props_end, "{name}: the values are as long as the record says");
    let mut kids = Vec::new();
    while p < end {
        let (k, next) = read_rec(b, p);
        p = next;
        match k {
            Some(k) => kids.push(k),
            None => break,
        }
    }
    assert_eq!(p, end, "{name}: the record ends where it says");
    (Some(Rec { name, props, kids }), end)
}

fn read(path: &std::path::Path) -> Rec {
    let b = std::fs::read(path).unwrap();
    assert_eq!(&b[..23], b"Kaydara FBX Binary  \x00\x1a\x00");
    assert_eq!(u32_at(&b, 23), 7400);
    let mut top = Vec::new();
    let mut at = 27;
    loop {
        let (r, next) = read_rec(&b, at);
        at = next;
        match r {
            Some(r) => top.push(r),
            None => break,
        }
    }
    // The footer: its mark, the version again, and its closing mark at the file's end.
    assert_eq!(&b[at..at + 4], &[0xfa, 0xbc, 0xab, 0x09]);
    assert_eq!(&b[b.len() - 16..b.len() - 12], &[0xf8, 0x5a, 0x8c, 0x6a]);
    assert_eq!(u32_at(&b, b.len() - 16 - 120 - 4), 7400);
    assert_eq!((b.len() - 16 - 120 - 4) % 16, 0, "the version in the footer sits on a 16-byte line");
    Rec { name: String::new(), props: Vec::new(), kids: top }
}

fn object<'a>(doc: &'a Rec, kind: &str, name: &str) -> &'a Rec {
    doc.kid("Objects").all(kind).into_iter().find(|r| r.s(1).split('\u{0}').next() == Some(name)).unwrap_or_else(|| panic!("no {kind} {name}"))
}

fn property<'a>(r: &'a Rec, name: &str) -> &'a Rec {
    r.kid("Properties70").kids.iter().find(|p| p.s(0) == name).unwrap_or_else(|| panic!("no property {name}"))
}

fn linked(doc: &Rec, child: i64, parent: i64) -> bool {
    doc.kid("Connections").kids.iter().any(|c| c.i(1) == child && c.i(2) == parent)
}

fn d(p: &P) -> f64 {
    match p {
        P::D(v) => *v,
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_hierarchy_goes_out_as_polygons_with_uvs_materials_and_its_place() {
    let dir = std::env::temp_dir().join(format!("aztek_fbx_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let at = |n: &str| dir.join(n).display().to_string().replace('\\', "/");
    let mut c = Core::new();
    c.mel("polyCube -n box -w 2 -h 2 -d 2; group -n grp box; move 3 0 0 grp; move 0 5 0 box; rotate 0 90 0 box;");
    c.mel("shadingNode -asShader blinn -n red; setAttr red.color 0.8 0.1 0.1; select -r box.f[0]; hyperShade -assign red; select -cl;");
    let r = c.mel_checked(&format!("exportFbx \"{}\" grp;", at("box.fbx"))).unwrap();
    assert_eq!((r["models"].clone(), r["meshes"].clone()), (json!(2), json!(1)), "{r}");
    let doc = read(&dir.join("box.fbx"));

    // The file's own word on its axes and units: Y up, centimetres.
    let settings = doc.kid("GlobalSettings");
    assert_eq!(property(settings, "UpAxis").i(4), 1);
    assert_eq!(d(&property(settings, "UnitScaleFactor").props[4]), 1.0);

    // The group sits where it is in the world, the box under it where it is in the group.
    let (top, the_box) = (object(&doc, "Model", "grp"), object(&doc, "Model", "box"));
    assert_eq!(top.s(2), "Null");
    assert_eq!(the_box.s(2), "Mesh");
    let t = property(top, "Lcl Translation");
    assert_eq!((d(&t.props[4]), d(&t.props[5]), d(&t.props[6])), (3.0, 0.0, 0.0));
    // The box was moved to (0, 5, 0) in the world: under a group at 3 that is -3 across.
    let t = property(the_box, "Lcl Translation");
    assert_eq!((d(&t.props[4]), d(&t.props[5]), d(&t.props[6])), (-3.0, 5.0, 0.0));
    let turn = property(the_box, "Lcl Rotation");
    assert!((d(&turn.props[5]) - 90.0).abs() < 1e-6, "degrees: {turn:?}");
    assert!(linked(&doc, top.i(0), 0), "the top is under the scene's root");
    assert!(linked(&doc, the_box.i(0), top.i(0)));

    // The mesh: eight points, six quads (each closed by a complemented index), a normal and a UV a corner.
    let geo = object(&doc, "Geometry", "boxShape");
    assert!(linked(&doc, geo.i(0), the_box.i(0)));
    assert_eq!(geo.kid("Vertices").doubles().len(), 24);
    let index = geo.kid("PolygonVertexIndex").ints();
    assert_eq!(index.len(), 24);
    assert_eq!(index.iter().filter(|i| **i < 0).count(), 6, "six polygons");
    assert!(index.chunks(4).all(|q| q[..3].iter().all(|i| *i >= 0) && q[3] < 0), "quads stay quads");
    let mut seen: Vec<i32> = index.iter().map(|i| if *i < 0 { -i - 1 } else { *i }).collect();
    seen.sort_unstable();
    seen.dedup();
    assert_eq!(seen, (0..8).collect::<Vec<i32>>());
    let normals = geo.kid("LayerElementNormal");
    assert_eq!(normals.kid("MappingInformationType").s(0), "ByPolygonVertex");
    assert_eq!(normals.kid("Normals").doubles().len(), 72);
    let uv = geo.kid("LayerElementUV");
    assert_eq!(uv.kid("Name").s(0), "map1");
    assert_eq!(uv.kid("UVIndex").ints().len(), 24);
    assert_eq!(uv.kid("UV").doubles().len(), 48);

    // Two materials, one a face: the faces say which, in the order the materials are linked to the model.
    let by_face = geo.kid("LayerElementMaterial");
    assert_eq!(by_face.kid("MappingInformationType").s(0), "ByPolygon");
    let slots = by_face.kid("Materials").ints();
    assert_eq!(slots.len(), 6);
    assert_eq!(slots.iter().filter(|s| **s == slots[0]).count(), 1, "one face wears red");
    let red = object(&doc, "Material", "red");
    assert_eq!(red.kid("ShadingModel").s(0), "phong");
    let colour = property(red, "DiffuseColor");
    assert!((d(&colour.props[4]) - 0.8).abs() < 1e-6);
    let order: Vec<i64> = doc.kid("Connections").kids.iter().filter(|c| c.i(2) == the_box.i(0) && doc.kid("Objects").all("Material").iter().any(|m| m.i(0) == c.i(1))).map(|c| c.i(1)).collect();
    assert_eq!(order.len(), 2);
    assert_eq!(order[slots[0] as usize], red.i(0));

    // What the file says it holds is what it holds.
    let defs = doc.kid("Definitions");
    for kind in ["Model", "Geometry", "Material"] {
        let said = defs.all("ObjectType").into_iter().find(|t| t.s(0) == kind).unwrap().kid("Count").i(0);
        assert_eq!(said as usize, doc.kid("Objects").all(kind).len(), "{kind}");
    }

    // The reference's own command name, for scripts written for it.
    c.mel(&format!("select -r box; FBXExport -f \"{}\" -s;", at("sel.fbx")));
    let sel = read(&dir.join("sel.fbx"));
    assert_eq!(sel.kid("Objects").all("Model").len(), 1);
    assert!(c.mel_checked(&format!("select -cl; exportFbx \"{}\";", at("none.fbx"))).is_err());

    // A Z-up scene says so in the file, and its data is as it is.
    c.mel("upAxis -ax z;");
    c.mel(&format!("exportFbx \"{}\" grp;", at("z.fbx")));
    let z = read(&dir.join("z.fbx"));
    assert_eq!(property(z.kid("GlobalSettings"), "UpAxis").i(4), 2);
    assert_eq!(property(z.kid("GlobalSettings"), "FrontAxisSign").i(4), -1);
    c.mel("upAxis -ax y;");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_bound_mesh_goes_out_with_its_joints_clusters_and_bind_pose() {
    let dir = std::env::temp_dir().join(format!("aztek_fbx_skin_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let at = |n: &str| dir.join(n).display().to_string().replace('\\', "/");
    let mut c = Core::new();
    c.mel("polyCylinder -n arm -r 1 -h 8 -sy 8; move 0 4 0 arm; select -cl; joint -n shoulder -p 0 0 0; joint -n elbow -p 0 4 0; joint -n wrist -p 0 8 0;");
    c.mel("skinCluster shoulder elbow wrist arm; setAttr elbow.rz 40;");
    let r = c.mel(&format!("exportFbx \"{}\" arm;", at("arm.fbx")));
    assert_eq!(r["skins"], json!(1), "{r}");
    let doc = read(&dir.join("arm.fbx"));

    // The joints are limb nodes in their hierarchy, each with its skeleton attribute, though only the mesh was asked for.
    let (shoulder, elbow, wrist) = (object(&doc, "Model", "shoulder"), object(&doc, "Model", "elbow"), object(&doc, "Model", "wrist"));
    assert_eq!(elbow.s(2), "LimbNode");
    assert!(linked(&doc, elbow.i(0), shoulder.i(0)) && linked(&doc, wrist.i(0), elbow.i(0)));
    assert!((d(&property(elbow, "Lcl Rotation").props[6]) - 40.0).abs() < 1e-6, "the pose is on the joint");
    assert_eq!(doc.kid("Objects").all("NodeAttribute").len(), 3);

    // The mesh is in its bind pose: a straight cylinder.
    let geo = object(&doc, "Geometry", "armShape");
    let v = geo.kid("Vertices").doubles();
    assert!(v.chunks(3).all(|p| (p[0] * p[0] + p[2] * p[2]).sqrt() < 1.0 + 1e-6), "no point has been swung out by the elbow");

    // One skin on the geometry, a cluster a joint under it, the joint linked to its cluster.
    let deformers = doc.kid("Objects").all("Deformer");
    let skin = deformers.iter().find(|x| x.s(2) == "Skin").unwrap();
    assert!(linked(&doc, skin.i(0), geo.i(0)));
    let clusters: Vec<&&Rec> = deformers.iter().filter(|x| x.s(2) == "Cluster").collect();
    assert_eq!(clusters.len(), 3);
    let points = v.len() / 3;
    let mut total = vec![0.0f64; points];
    for cl in &clusters {
        assert!(linked(&doc, cl.i(0), skin.i(0)));
        let (idx, w) = (cl.kid("Indexes").ints(), cl.kid("Weights").doubles());
        assert_eq!(idx.len(), w.len());
        for (i, x) in idx.iter().zip(w) {
            total[*i as usize] += x;
        }
        // Where the joint and the mesh stood at the bind, in the world, as the format's own library writes them.
        let (transform, link) = (cl.kid("Transform").doubles(), cl.kid("TransformLink").doubles());
        let joint = doc.kid("Connections").kids.iter().find(|k| k.i(2) == cl.i(0)).map(|k| k.i(1)).unwrap();
        let bind_y = if joint == shoulder.i(0) { 0.0 } else if joint == elbow.i(0) { 4.0 } else { 8.0 };
        assert!((link[13] - bind_y).abs() < 1e-9, "the joint's bind height: {link:?}");
        assert!((transform[13] - 4.0).abs() < 1e-9, "the mesh sat 4 up: {transform:?}");
    }
    assert!(total.iter().all(|t| (t - 1.0).abs() < 1e-9), "every point's weights sum to one");

    // The bind pose names the mesh and the three joints.
    let pose = doc.kid("Objects").kid("Pose");
    assert_eq!(pose.kid("NbPoseNodes").i(0), 4);
    assert_eq!(pose.all("PoseNode").len(), 4);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_formats_are_plug_ins_that_load_at_start_and_unload() {
    let mut c = Core::new();
    assert_eq!(c.mel("pluginInfo -q -loaded fbx"), json!(true));
    assert_eq!(c.mel("pluginInfo -q -loaded gltf"), json!(true));
    let listed = c.mel("pluginInfo -q -listPlugins");
    assert!(listed.as_array().unwrap().contains(&json!("fbx")) && listed.as_array().unwrap().contains(&json!("gltf")), "{listed}");
    // The commands, then the settings commands the reference's plug-in has (FBXImportSkins, FBXExportUpAxis, ...).
    let cmds = c.mel("pluginInfo -q -command fbx");
    let cmds = cmds.as_array().unwrap();
    assert_eq!(cmds[..4], [json!("exportFbx"), json!("FBXExport"), json!("importFbx"), json!("FBXImport")]);
    for name in ["FBXImportSkins", "FBXImportMode", "FBXExportUpAxis", "FBXResetImport", "FBXPushSettings"] {
        assert!(cmds.contains(&json!(name)), "{name}");
    }
    assert_eq!(c.mel("pluginInfo -q -vendor fbx"), json!("Aztek"));
    // The names they had when they only exported still find them.
    assert_eq!(c.mel("pluginInfo -q -loaded fbxExport"), json!(true));

    // Unloaded, its commands are gone, reading and writing; loaded again, they are back.
    c.mel("polyCube -n box;");
    c.mel("unloadPlugin fbx;");
    assert_eq!(c.mel("pluginInfo -q -loaded fbx"), json!(false));
    assert!(c.mel_checked("exportFbx \"/tmp/never.fbx\" box;").is_err());
    assert!(c.mel_checked("importFbx \"/tmp/never.fbx\";").is_err());
    assert!(c.mel_checked("FBXImportSkins -q;").is_err(), "the settings go with the plug-in");
    assert!(c.mel_checked("file -import -type \"FBX\" \"/tmp/never.fbx\";").unwrap_err().contains("unknown file type"));
    assert!(c.mel_checked("exportGltf \"/nonexistent-folder/never.glb\" box;").unwrap_err().contains("can't write"), "the other plug-in is untouched");
    assert_eq!(c.mel("loadPlugin fbx"), json!(["fbx"]));
    assert_eq!(c.mel("pluginInfo -q -loaded fbx"), json!(true));
    assert!(c.mel_checked("select -cl; exportFbx \"/tmp/never.fbx\";").unwrap_err().contains("nothing selected"));
    assert!(c.mel_checked("importFbx \"/nonexistent-folder/never.fbx\";").unwrap_err().contains("can't read"));
}
