//! Issues of 17/11/2026.

use aztek_host::camera::CamKind;
use aztek_host::viewport::{gather, Viewport};
use aztek_host::Core;
use glam::{DVec2, DVec3};
use serde_json::{json, Value};

fn mel(c: &mut Core, src: &str) -> Value {
    c.mel_checked(src).unwrap_or_else(|e| panic!("{src}: {e}"))
}

fn top_view() -> Viewport {
    let mut vp = Viewport::new(CamKind::Persp);
    vp.rect = [0.0, 0.0, 800.0, 800.0];
    vp.cam.coi = DVec3::ZERO;
    vp.cam.yaw = 0.0;
    vp.cam.pitch = 89.9;
    vp.cam.distance = 30.0;
    vp
}

/// #116: the live surface leaves the selection and can't be picked; what is under it can.
#[test]
fn a_live_surface_is_not_picked() {
    let mut c = Core::new();
    mel(&mut c, "polyPlane -n ground -w 20 -h 20; polyCube -n box; move 0 -5 0 box; select ground;");
    mel(&mut c, "makeLive;");
    assert_eq!(mel(&mut c, "makeLive -q;"), json!(["groundShape"]));
    assert_eq!(mel(&mut c, "ls -sl;"), json!([]));
    let mut vp = top_view();
    let items = gather(&mut c.s.graph);
    let at = vp.screen().project(DVec3::new(0.0, 0.0, 0.0)).unwrap();
    assert_eq!(vp.pick(&c.s, &items, at), vec!["box".to_string()]);
    let off: DVec2 = vp.screen().project(DVec3::new(8.0, 0.0, 8.0)).unwrap();
    assert!(vp.pick(&c.s, &items, off).is_empty());
    mel(&mut c, "makeLive -none;");
    assert_eq!(vp.pick(&c.s, &items, off), vec!["ground".to_string()]);
}

/// #114: Shift toggles, Ctrl removes, Ctrl+Shift adds, in objects and components.
#[test]
fn click_modifiers_are_the_references() {
    use aztek_host::view::{select_picked, CTRL, SHIFT};
    let mut c = Core::new();
    mel(&mut c, "polyCube -n a; polyCube -n b; select -cl;");
    let p = |s: &str| vec![s.to_string()];
    select_picked(&mut c, p("a"), 0);
    select_picked(&mut c, p("b"), SHIFT);
    assert_eq!(mel(&mut c, "ls -sl;"), json!(["a", "b"]));
    select_picked(&mut c, p("a"), SHIFT);
    assert_eq!(mel(&mut c, "ls -sl;"), json!(["b"]));
    select_picked(&mut c, p("b"), CTRL);
    assert_eq!(mel(&mut c, "ls -sl;"), json!([]));
    select_picked(&mut c, p("b"), CTRL);
    assert_eq!(mel(&mut c, "ls -sl;"), json!([]));
    select_picked(&mut c, p("a"), CTRL | SHIFT);
    select_picked(&mut c, p("a"), CTRL | SHIFT);
    assert_eq!(mel(&mut c, "ls -sl;"), json!(["a"]));
    mel(&mut c, "select a; selectMode -component; selectType -v true; select a.vtx[0];");
    select_picked(&mut c, p("a.vtx[1]"), SHIFT);
    select_picked(&mut c, p("a.vtx[0]"), SHIFT);
    assert_eq!(mel(&mut c, "ls -sl;"), json!(["a.vtx[1]"]));
    select_picked(&mut c, p("a.vtx[2]"), CTRL | SHIFT);
    select_picked(&mut c, p("a.vtx[1]"), CTRL);
    assert_eq!(mel(&mut c, "ls -sl;"), json!(["a.vtx[2]"]));
}

/// #123: setting the ViewCube's preferences, as Alt+B's preference pass did, leaves the camera where it is.
#[test]
fn view_cube_settings_leave_the_camera() {
    let mut c = Core::new();
    mel(&mut c, "polyCube -w 10 -h 10 -d 10; select -cl; setAttr persp.t 3 4 5; setAttr persp.r -20 30 0;");
    let before = mel(&mut c, "getAttr persp.t;");
    mel(&mut c, "viewManip -visible true -topRight true -zoomToFitScene true -preserveSceneUp true;");
    assert_eq!(mel(&mut c, "getAttr persp.t;"), before);
    assert_eq!(mel(&mut c, "viewManip -q -zoomToFitScene;"), json!(true));
}

/// #124: in a component mode, a click on another object takes it, in object mode; a click on nothing keeps the mode.
#[test]
fn a_click_on_another_object_leaves_component_mode() {
    use aztek_host::view::select_picked;
    let mut c = Core::new();
    mel(&mut c, "polyCube -n a -w 2 -h 2 -d 2; move -4 0 0; polyCube -n b -w 2 -h 2 -d 2; move 4 0 0; select a; selectMode -component; selectType -v true; select a.vtx[0];");
    let mut vp = top_view();
    let items = gather(&mut c.s.graph);
    let on_b = vp.screen().project(DVec3::new(4.0, 1.0, 0.0)).unwrap();
    let nothing = vp.screen().project(DVec3::new(0.0, 0.0, 9.0)).unwrap();
    let picked = vp.pick(&c.s, &items, nothing);
    assert!(picked.is_empty());
    let picked = vp.pick(&c.s, &items, on_b);
    assert_eq!(picked, vec!["b".to_string()]);
    select_picked(&mut c, picked, 0);
    assert_eq!(mel(&mut c, "selectMode -q -object;"), json!(true));
    assert_eq!(mel(&mut c, "ls -sl;"), json!(["b"]));
}

/// #89: referenced nodes and startup cameras among those deleted are passed over too; the rest go.
#[test]
fn delete_passes_over_referenced_nodes_and_startup_cameras() {
    let dir = std::env::temp_dir().join(format!("az_del_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let prop = dir.join("prop.ma").to_string_lossy().replace('\\', "/");
    let mut c = Core::new();
    mel(&mut c, &format!("polyCube -n crate; file -rename \"{prop}\"; file -save -type \"mayaAscii\"; file -f -new;"));
    mel(&mut c, &format!("file -r -ns \"rd\" \"{prop}\"; polySphere -n ball; shadingNode -asShader lambert -n mat;"));
    mel(&mut c, "select -r rd:crate ball persp mat defaultRenderGlobals; delete;");
    assert_eq!(mel(&mut c, "objExists ball;"), json!(false));
    assert_eq!(mel(&mut c, "objExists mat;"), json!(false));
    assert_eq!(mel(&mut c, "objExists rd:crate;"), json!(true));
    assert_eq!(mel(&mut c, "objExists persp;"), json!(true));
    assert!(c.mel_checked("delete rd:crate;").is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

/// #91: an OBJ's exporter "default" group doesn't name the mesh; `file -i -rnn` returns the new nodes.
#[test]
fn obj_import_names_and_returns_new_nodes() {
    let mut c = Core::new();
    let dir = std::env::temp_dir().join("aztek_issue_91");
    std::fs::create_dir_all(&dir).unwrap();
    let a = dir.join("a.obj");
    std::fs::write(&a, "g default\nv 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n").unwrap();
    let b = dir.join("b.obj");
    std::fs::write(&b, "o Thing\nv 0 0 0\nv 1 0 0\nv 0 1 0\nusemtl default\nf 1 2 3\n").unwrap();
    mel(&mut c, "loadPlugin objExport;");
    let n = mel(&mut c, &format!("file -i -rnn \"{}\";", a.display().to_string().replace('\\', "/")));
    assert!(n.as_array().unwrap().iter().any(|x| x.as_str().unwrap().ends_with("polySurface1")), "{n}");
    let n = mel(&mut c, &format!("file -i -rnn \"{}\";", b.display().to_string().replace('\\', "/")));
    assert!(n.as_array().unwrap().iter().any(|x| x == "Thing"), "{n}");
    assert_eq!(mel(&mut c, "ls -type transform \"default*\";"), json!([]));
}

/// #92: every type an importer reads can be referenced, its nodes named as the file has them inside the namespace.
#[test]
fn every_imported_type_can_be_referenced() {
    let mut c = Core::new();
    let dir = std::env::temp_dir().join("aztek_issue_92");
    std::fs::create_dir_all(&dir).unwrap();
    let d = dir.display().to_string().replace('\\', "/");
    mel(&mut c, "loadPlugin objExport; loadPlugin fbx; loadPlugin gltf; loadPlugin AbcImport; loadPlugin AbcExport;");
    mel(&mut c, "polyCube -n c;");
    mel(&mut c, &format!("file -f -type \"FBX export\" -exportAll \"{d}/c.fbx\";"));
    mel(&mut c, &format!("exportGltf \"{d}/c.glb\";"));
    mel(&mut c, &format!("AbcExport -j \"-root |c -file {d}/c.abc\";"));
    mel(&mut c, &format!("file -f -exportAll -type OBJexport \"{d}/c.obj\";"));
    mel(&mut c, &format!("file -f -exportAll -type STL \"{d}/c.stl\";"));
    for ext in ["fbx", "glb", "abc", "obj", "stl"] {
        mel(&mut c, &format!("file -r -ns r{ext} \"{d}/c.{ext}\";"));
        let rn = mel(&mut c, &format!("referenceQuery -rfn \"{d}/c.{ext}\";"));
        assert_eq!(rn, json!(format!("r{ext}RN")), "{ext}");
        let nodes = mel(&mut c, &format!("ls -type transform \"r{ext}:*\";"));
        assert!(!nodes.as_array().unwrap().is_empty(), "{ext}: {nodes}");
        if matches!(ext, "fbx" | "glb" | "abc") {
            assert!(nodes.as_array().unwrap().contains(&json!(format!("r{ext}:c"))), "{ext}: {nodes}");
        }
    }
    // Nothing merged into the scene's own cube.
    assert_eq!(mel(&mut c, "ls -type transform \"c*\";"), json!(["c"]));
}

/// #94: Combine keeps each mesh's materials face by face, and Separate gives them to the pieces.
#[test]
fn combine_and_separate_keep_materials() {
    let mut c = Core::new();
    mel(&mut c, "polyCube -n a; polyCube -n b; move 3 0 0 b;");
    mel(&mut c, "shadingNode -asShader lambert -n red; sets -renderable true -noSurfaceShader true -empty -n redSG; connectAttr red.outColor redSG.surfaceShader;");
    mel(&mut c, "shadingNode -asShader lambert -n blue; sets -renderable true -noSurfaceShader true -empty -n blueSG; connectAttr blue.outColor blueSG.surfaceShader;");
    mel(&mut c, "sets -e -forceElement redSG a; sets -e -forceElement blueSG b.f[0:2]; sets -e -forceElement redSG b.f[3:5];");
    mel(&mut c, "polyUnite -n u a b;");
    let red = mel(&mut c, "sets -q redSG;");
    assert!(red.as_array().unwrap().contains(&json!("u.f[0:5]")) && red.as_array().unwrap().contains(&json!("u.f[9:11]")), "{red}");
    assert!(mel(&mut c, "sets -q blueSG;").as_array().unwrap().contains(&json!("u.f[6:8]")));
    let pieces = mel(&mut c, "polySeparate u;");
    let first = pieces[0].as_str().unwrap().to_string();
    let second = pieces[1].as_str().unwrap().to_string();
    let shading = |c: &mut Core, x: &str| mel(c, &format!("listConnections -type shadingEngine `listRelatives -s {x}`;"));
    assert_eq!(shading(&mut c, &first), json!(["redSG"]));
    let both = shading(&mut c, &second);
    assert!(both.as_array().unwrap().contains(&json!("redSG")) && both.as_array().unwrap().contains(&json!("blueSG")), "{both}");
}

/// #96: emitter makes only the emitter; the reference's type names find these nodes; connectDynamic says what it got.
#[test]
fn emitter_alone_reference_type_names_and_clear_errors() {
    let mut c = Core::new();
    assert_eq!(mel(&mut c, "emitter -type volume -n em1;"), json!(["em1"]));
    assert_eq!(mel(&mut c, "ls -type particles;"), json!([]));
    assert_eq!(mel(&mut c, "ls -type nucleus;"), json!([]));
    assert!(!mel(&mut c, "ls -type pointEmitter;").as_array().unwrap().is_empty());
    let p = mel(&mut c, "nParticle -n np;");
    assert_eq!(p, json!(["np", "npShape"]));
    assert_eq!(mel(&mut c, "nParticle;"), json!(["nParticle1", "nParticleShape1"]));
    assert_eq!(mel(&mut c, "particle;")[0], json!("particle1"));
    assert_eq!(mel(&mut c, "ls -type nParticle;")[0], json!("npShape"));
    assert_eq!(mel(&mut c, "ls -type particle;")[0], json!("npShape"));
    assert_eq!(mel(&mut c, "size(`ls -type nucleus`);"), json!(1));
    mel(&mut c, "connectDynamic -em em1 np;");
    // Again: nothing doubled.
    mel(&mut c, "connectDynamic -em em1 np;");
    assert_eq!(mel(&mut c, "size(`listConnections -s 1 -d 0 npShape.newParticles`);"), json!(1));
    let e = c.mel_checked("connectDynamic -em np np;").unwrap_err().to_string();
    assert!(e.contains("takes an emitter") && e.contains("np is a transform of a particles") && e.contains("em1"), "{e}");
    mel(&mut c, "polySphere -n ball;");
    assert_eq!(mel(&mut c, "emitter -type surface ball;"), json!(["ball", "emitter1"]));
}

/// #100: a stage's references and payloads to Alembic (as usdAbc), OBJ, FBX and glTF files read as USD.
#[test]
fn a_stage_reads_alembic_obj_fbx_and_gltf_references() {
    let mut c = Core::new();
    let dir = std::env::temp_dir().join("aztek_issue_100");
    std::fs::create_dir_all(&dir).unwrap();
    let d = dir.display().to_string().replace('\\', "/");
    mel(&mut c, "loadPlugin objExport; loadPlugin fbx; loadPlugin gltf; loadPlugin AbcExport; loadPlugin mayaUsdPlugin;");
    mel(&mut c, "polyCube -n c;");
    mel(&mut c, &format!("AbcExport -j \"-root |c -file {d}/c.abc\";"));
    mel(&mut c, &format!("file -f -exportAll -type OBJexport \"{d}/c.obj\";"));
    mel(&mut c, &format!("file -f -type \"FBX export\" -exportAll \"{d}/c.fbx\";"));
    mel(&mut c, &format!("exportGltf \"{d}/c.glb\";"));
    std::fs::write(
        dir.join("stage.usda"),
        "#usda 1.0\ndef Xform \"World\"\n{\n    def \"A\" ( references = @./c.abc@ ) {}\n    def \"P\" ( payload = @./c.abc@ ) {}\n    def \"O\" ( references = @./c.obj@ ) {}\n    def \"F\" ( references = @./c.fbx@ ) {}\n    def \"G\" ( references = @./c.glb@ ) {}\n}\n",
    )
    .unwrap();
    mel(&mut c, "file -f -new;");
    mel(&mut c, &format!("file -i \"{d}/stage.usda\";"));
    let meshes = mel(&mut c, "ls -type mesh;");
    for m in ["AShape", "PShape", "OShape", "FShape", "GShape"] {
        assert!(meshes.as_array().unwrap().contains(&json!(m)), "{m}: {meshes}");
    }
}

/// #108: convertSolidTx bakes a network over the UVs; Create PSD Network makes a PSD with a layer set for each
/// attribute and psdFileTex nodes in their place; Update PSD Networks redraws the snapshot; UV Editor Baking bakes the
/// material behind the UVs.
#[test]
fn bakes_and_psd_networks() {
    let mut c = Core::new();
    let dir = std::env::temp_dir().join(format!("aztek_issue_108_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let d = dir.display().to_string().replace('\\', "/");
    mel(&mut c, "polyPlane -n p -sx 1 -sy 1; shadingNode -asShader lambert -n m; sets -renderable true -noSurfaceShader true -empty -n mSG; connectAttr m.outColor mSG.surfaceShader; sets -e -forceElement mSG p;");
    mel(&mut c, "shadingNode -asTexture checker -n ch; connectAttr ch.outColor m.color; setAttr ch.color1 -type double3 1 0 0; setAttr ch.color2 -type double3 0 0 1;");
    let made = mel(&mut c, &format!("convertSolidTx -rx 64 -ry 64 -fin \"{d}/bake.png\" -force m.color p;"));
    assert_eq!(made.as_array().unwrap().len(), 1, "{made}");
    let img = image::open(dir.join("bake.png")).unwrap().to_rgba8();
    assert_eq!(img.dimensions(), (64, 64));
    // The checker's two colours, red at one corner and blue beside it.
    let (a, b) = (img.get_pixel(4, 4).0, img.get_pixel(4 + 16, 4).0);
    let red = |p: [u8; 4]| p[0] > 200 && p[2] < 50;
    let blue = |p: [u8; 4]| p[2] > 200 && p[0] < 50;
    assert!((red(a) && blue(b)) || (blue(a) && red(b)), "{a:?} {b:?}");
    let file = mel(&mut c, &format!("getAttr {}.fileTextureName;", made[0].as_str().unwrap()));
    assert!(file.as_str().unwrap().ends_with("bake.png"));

    let r = mel(&mut c, &format!("createPsdNetwork m -psf \"{d}/m.psd\" -xr 64 -yr 64 -attributes color -attributes transparency;"));
    assert_eq!(r["nodes"].as_array().unwrap().len(), 2, "{r}");
    let sets = mel(&mut c, &format!("psdExport -psf \"{d}/m.psd\";"));
    assert_eq!(sets, json!(["m.transparency", "m.color"]));
    let tex = r["nodes"][0].as_str().unwrap().to_string();
    assert_eq!(mel(&mut c, &format!("nodeType {tex};")), json!("psdFileTex"));
    assert_eq!(mel(&mut c, "listConnections -s 1 -d 0 m.color;"), json!([tex.clone()]));
    assert_eq!(mel(&mut c, &format!("getAttr {tex}.layerSetName;")), json!("m.color"));
    // The layer set as the texture shows it: the checker, now painted in the PSD.
    let out = dir.join("set.png").display().to_string().replace('\\', "/");
    mel(&mut c, &format!("psdExport -psf \"{d}/m.psd\" -lsn m.color -ofn \"{out}\";"));
    let set = image::open(&out).unwrap().to_rgba8();
    assert_eq!(set.get_pixel(4, 4).0, img.get_pixel(4, 4).0);
    let id = c.s.graph.find(&tex).unwrap().unwrap();
    let png = aztek_scene::shading::on_disk(&mut c.s.graph, id, dir.join("m.psd")).unwrap();
    assert!(png.extension().unwrap() == "png" && png.is_file());

    // The snapshot follows the UVs.
    let before = aztek_scene::psd::read_file(&dir.join("m.psd")).unwrap();
    let snap = |p: &aztek_scene::psd::Psd| p.layers.iter().find(|l| l.name == "UVSnapShot").unwrap().rgba.clone();
    mel(&mut c, "polyEditUV -u 0.3 -v 0.2 p.map[*];");
    assert_eq!(mel(&mut c, "updatePsdNetworks;").as_array().unwrap().len(), 1);
    let after = aztek_scene::psd::read_file(&dir.join("m.psd")).unwrap();
    assert_ne!(snap(&before), snap(&after));
    // The painted sets are kept.
    assert_eq!(after.composite(Some("m.color")), before.composite(Some("m.color")));

    // UV Editor Baking.
    mel(&mut c, "select -r p; uiPref uvEditorBaking 1;");
    let t = aztek_host::query::uv_texture(&mut c);
    assert_eq!(t["baked"], json!(true), "{t}");
    assert!(std::path::Path::new(t["path"].as_str().unwrap()).is_file());
    mel(&mut c, "uiPref uvEditorBaking 0;");
}

/// #125: a live surface picked in the Outliner doesn't go into component mode.
#[test]
fn a_live_surface_has_no_component_mode() {
    let mut c = Core::new();
    mel(&mut c, "polySphere -n ball; makeLive ball; select -r ball; selectMode -component; selectType -vertex 1;");
    assert_eq!(mel(&mut c, "ls -hilite;"), json!([]));
    mel(&mut c, "makeLive -none; select -r ball; selectMode -component;");
    assert_eq!(mel(&mut c, "ls -hilite;"), json!(["ballShape"]));
}

/// #130: DeleteHistory, as the General shelf runs it, takes the history off and leaves the object.
#[test]
fn delete_history_keeps_the_object() {
    let mut c = Core::new();
    c.mel("polyCube; polySmooth pCube1; select pCube1;");
    assert!(c.mel("listHistory pCube1;").as_array().unwrap().iter().any(|v| v.as_str().is_some_and(|n| n.starts_with("polySmooth"))));
    c.mel("DeleteHistory;");
    assert_eq!(c.mel("objExists pCube1;"), json!(true));
    assert_eq!(c.mel("objExists pCubeShape1;"), json!(true));
    assert!(!c.mel("listHistory pCube1;").as_array().unwrap().iter().any(|v| v.as_str().is_some_and(|n| n.starts_with("poly"))));
    // Named objects too.
    c.mel("polySphere; polySmooth pSphere1; select -cl;");
    c.mel("DeleteHistory pSphere1;");
    assert_eq!(c.mel("objExists pSphere1;"), json!(true));
    assert!(!c.mel("listHistory pSphere1;").as_array().unwrap().iter().any(|v| v.as_str().is_some_and(|n| n.starts_with("poly"))));
}

/// #130's guard: every shelf button's command runs, and none but those that make something new out of the selection
/// takes the selected object away.
#[test]
fn every_shelf_button_runs_and_keeps_the_selection() {
    let src = include_str!("../../../app/src/shelves.cpp");
    // The shell's own runtime commands, which it hands the host at start (ui_cmds::register_ui_names).
    let runtime = include_str!("../../../app/src/runtime.cpp");
    let shell: std::collections::HashSet<&str> = ["    r(\"", "    m(\"", "    tool(\""]
        .iter()
        .flat_map(|lead| runtime.split(lead).skip(1).filter_map(|p| p.split('"').next()))
        .collect();
    let mut c = Core::new();
    let mut seen = 0;
    let mut wrong = Vec::new();
    for part in src.split("item(\"").skip(1) {
        let fields: Vec<&str> = part.splitn(4, "\", \"").collect();
        if fields.len() < 3 {
            continue;
        }
        let Some(end) = fields[2].find("\")") else { continue };
        let cmd = &fields[2][..end];
        seen += 1;
        c.mel("file -f -new; polyCube; select -r pCube1;");
        let r = c.mel_checked(cmd);
        if let Err(e) = &r {
            let first = cmd.split([' ', ';']).next().unwrap_or("");
            if e.contains("Cannot find procedure") && !shell.contains(first) {
                wrong.push(format!("{cmd}: {e}"));
                continue;
            }
        }
        // Combine and Separate replace the object with what they make, as in the reference.
        let replaces = cmd.starts_with("polyCombine") || cmd.starts_with("polySeparate");
        if !replaces && c.mel("objExists pCube1;") != json!(true) {
            wrong.push(format!("{cmd}: removed pCube1"));
        }
    }
    assert!(seen > 80, "{seen} buttons read");
    assert!(wrong.is_empty(), "{wrong:#?}");
}

/// #132: outside a procedure, code is in the global scope, so a global reads without being declared there, as in the
/// reference; inside a procedure it still has to be declared.
#[test]
fn globals_read_at_the_top_level_without_a_declaration() {
    let mut c = Core::new();
    assert_eq!(c.mel_checked("$w = $gMainWindow;").unwrap(), json!("MayaWindow"));
    assert_eq!(c.mel_checked("eval(\"$w2 = $gMainWindow\");").unwrap(), json!("MayaWindow"));
    assert_eq!(c.mel_checked("$p = $gMainProgressBar;").unwrap(), json!("MainProgressBar"));
    // A script's own global, declared once, then read and set at the top level of later runs.
    c.mel_checked("global int $issue132 = 4;").unwrap();
    assert_eq!(c.mel_checked("$issue132 + 1;").unwrap(), json!(5));
    c.mel_checked("$issue132 = 9;").unwrap();
    assert_eq!(c.mel_checked("global int $issue132; $issue132;").unwrap(), json!(9));
    // In a procedure, undeclared is still an error, and declared reads it.
    assert!(c.mel_checked("proc string issue132a() { return $gMainWindow; } issue132a();").is_err());
    assert_eq!(c.mel_checked("proc string issue132b() { global string $gMainWindow; return $gMainWindow; } issue132b();").unwrap(), json!("MayaWindow"));
    // From Python, as the reporter's start-up package does.
    assert_eq!(c.mel_checked(r#"python("import maya.mel as mel"); python("mel.eval('$mainWindow = $gMainWindow')");"#).unwrap(), json!("MayaWindow"));
}

/// The welcome screen's parity bar: from the parity ledger's summary (parity/summary.tsv), the commands and flags among
/// its areas.
#[test]
fn parity_counts_the_reference_commands_and_flags() {
    let mut c = Core::new();
    let p = c.exec("aztekParity", json!({"query": true})).unwrap();
    assert_eq!(p["commands"], json!(1198));
    let have = p["commandsHave"].as_u64().unwrap();
    assert!(have > 500 && have < 1198, "{have}");
    let percent = p["percent"].as_f64().unwrap();
    assert!(percent > 0.0 && percent < 100.0);
    let areas = p["areas"].as_array().unwrap();
    assert!(areas.iter().any(|a| a["area"] == json!("command_flags") && a["rows"] == p["flags"]));
}

/// #138: a selected joint draws its axes, from its centre to its ball; not when it isn't selected.
#[test]
fn a_selected_joint_shows_its_transform_hint() {
    let mut c = Core::new();
    c.mel("select -cl; joint -p 0 0 0 -rad 2; joint -p 4 0 0; select -cl;");
    assert!(aztek_host::viewport::joint_hints(&c.s.graph).is_empty());
    c.mel("select -r joint1;");
    let lines = aztek_host::viewport::joint_hints(&c.s.graph);
    assert_eq!(lines.len(), 6);
    // Out to the ball: half the radius, as the joint is drawn.
    let len = |i: usize| {
        let (a, b) = (lines[2 * i].pos, lines[2 * i + 1].pos);
        ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
    };
    for i in 0..3 {
        assert!((len(i) - 1.0).abs() < 1e-4, "{}", len(i));
    }
    c.mel("selectMode -component;");
    assert!(aztek_host::viewport::joint_hints(&c.s.graph).is_empty());
}

/// #136: picked translate channels put the Move manipulator in the parent's axes; letting them go puts it back.
#[test]
fn picked_translate_channels_turn_the_move_manipulator_to_the_parent() {
    let mut c = Core::new();
    c.mel("group -em -n p; rotate 0 0 90 p; polyCube -n box; parent box p; select -r box; setToolTo moveSuperContext;");
    let mut vp = Viewport::new(CamKind::Persp);
    vp.tool = aztek_host::viewport::Tool::Move;
    vp.axis_mode = "world".into();
    vp.update_frame(&mut c.s);
    assert!((vp.frame.x_axis - DVec3::X).length() < 1e-9);
    aztek_host::viewport::set_channel_space(true);
    vp.update_frame(&mut c.s);
    // The parent is turned 90 degrees about Z: its X is the world's Y.
    assert!((vp.frame.x_axis - DVec3::Y).length() < 1e-6, "{:?}", vp.frame);
    aztek_host::viewport::set_channel_space(false);
    vp.update_frame(&mut c.s);
    assert!((vp.frame.x_axis - DVec3::X).length() < 1e-9);
}

/// #134: the reference's workspaces, by name, and the one shown.
#[test]
fn workspaces_list_and_switch() {
    let mut c = Core::new();
    let names = mel(&mut c, "workspaceLayoutManager -listLayouts;");
    for n in ["General", "Modeling - Standard", "Modeling - Expert", "Sculpting", "Pose Sculpting", "UV Editing", "XJEN", "Rigging", "Animation", "Rendering - Standard", "Rendering - Expert"] {
        assert!(names.as_array().unwrap().contains(&json!(n)), "{n}");
    }
    mel(&mut c, "workspaceLayoutManager -setCurrent \"Rigging\";");
    assert_eq!(mel(&mut c, "workspaceLayoutManager -q -current;"), json!("Rigging"));
}
