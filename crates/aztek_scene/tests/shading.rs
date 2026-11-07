//! Materials, shading groups and assignment, as the reference's command reference describes them.

use aztek_scene::{shading, Session};
use serde_json::{json, Value};

fn run(s: &mut Session, name: &str, call: Value) -> Value {
    s.run(name, call).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn scene() -> Session {
    let mut s = Session::new();
    aztek_scene::cameras::make_defaults(&mut s.graph);
    s
}

fn cube(s: &mut Session) -> String {
    run(s, "polyCube", json!({}))[0].as_str().unwrap().to_string()
}

fn close(v: &Value, want: &[f64]) -> bool {
    let list = v.as_array().map(|a| a[0].as_array().cloned().unwrap_or_else(|| a.clone())).unwrap();
    list.len() == want.len() && list.iter().zip(want).all(|(a, b)| (a.as_f64().unwrap() - b).abs() < 1e-6)
}

#[test]
fn default_scene_has_lambert1_and_new_meshes_join_it() {
    let mut s = scene();
    assert_eq!(run(&mut s, "ls", json!({"type": "lambert"})), json!(["lambert1"]));
    assert_eq!(run(&mut s, "ls", json!({"materials": true})), json!(["lambert1"]));
    assert_eq!(run(&mut s, "ls", json!({"type": "shadingEngine"})), json!(["initialShadingGroup"]));
    assert_eq!(run(&mut s, "listConnections", json!({"args": ["initialShadingGroup.surfaceShader"]})), json!(["lambert1"]));
    cube(&mut s);
    assert_eq!(run(&mut s, "sets", json!({"args": ["initialShadingGroup"], "q": true})), json!(["pCubeShape1"]));
    assert_eq!(run(&mut s, "listConnections", json!({"args": ["pCubeShape1"], "type": "shadingEngine"})), json!(["initialShadingGroup"]));
    assert_eq!(run(&mut s, "listConnections", json!({"args": ["pCubeShape1.instObjGroups"], "plugs": true})), json!(["initialShadingGroup.dagSetMembers[0]"]));
    // One undo takes the mesh and its membership together.
    assert!(s.undo());
    assert_eq!(run(&mut s, "sets", json!({"args": ["initialShadingGroup"], "q": true})), json!([]));
    assert!(s.redo());
    assert_eq!(run(&mut s, "sets", json!({"args": ["initialShadingGroup"], "q": true})), json!(["pCubeShape1"]));
}

#[test]
fn defaults_of_materials_and_short_names() {
    let mut s = scene();
    assert_eq!(run(&mut s, "shadingNode", json!({"args": ["phong"], "asShader": true})), "phong1");
    assert!(close(&run(&mut s, "getAttr", json!({"args": ["phong1.color"]})), &[0.5, 0.5, 0.5]));
    assert!(close(&run(&mut s, "getAttr", json!({"args": ["phong1.c"]})), &[0.5, 0.5, 0.5]));
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["phong1.diffuse"]})), json!(0.8));
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["phong1.cp"]})), json!(20.0));
    assert!(close(&run(&mut s, "getAttr", json!({"args": ["phong1.specularColor"]})), &[0.5, 0.5, 0.5]));
    assert_eq!(run(&mut s, "ls", json!({"type": "lambert"})), json!(["lambert1", "phong1"]));
    assert_eq!(run(&mut s, "ls", json!({"exactType": "lambert"})), json!(["lambert1"]));
    run(&mut s, "setAttr", json!({"args": ["phong1.colorG", 0.25]}));
    assert!(close(&run(&mut s, "getAttr", json!({"args": ["phong1.outColor"]})), &[0.5, 0.25, 0.5]));
    assert_eq!(run(&mut s, "shadingNode", json!({"args": ["blinn"], "asShader": true})), "blinn1");
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["blinn1.eccentricity"]})), json!(0.3));
    assert_eq!(run(&mut s, "shadingNode", json!({"args": ["phongE"], "asShader": true})), "phongE1");
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["phongE1.roughness"]})), json!(0.5));
    assert_eq!(run(&mut s, "shadingNode", json!({"args": ["surfaceShader"], "asShader": true})), "surfaceShader1");
    assert_eq!(run(&mut s, "shadingNode", json!({"args": ["file"], "asTexture": true})), "file1");
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["file1.fileTextureName"]})), "");
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["file1.colorSpace"]})), "sRGB");
    assert_eq!(run(&mut s, "ls", json!({"textures": true})), json!(["file1"]));
    assert_eq!(run(&mut s, "shadingNode", json!({"args": ["place2dTexture"], "asUtility": true})), "place2dTexture1");
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["place2dTexture1.repeatU"]})), json!(1.0));
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["place2dTexture1.wrapU"]})), json!(true));
    assert_eq!(run(&mut s, "getClassification", json!({"args": ["lambert"]})), json!(["shader/surface"]));
}

#[test]
fn shading_group_by_sets_and_whole_object_assignment() {
    let mut s = scene();
    let c = cube(&mut s);
    run(&mut s, "shadingNode", json!({"args": ["lambert"], "asShader": true}));
    let sg = run(&mut s, "sets", json!({"renderable": true, "noSurfaceShader": true, "empty": true, "name": "lambert2SG"}));
    assert_eq!(sg, "lambert2SG");
    assert_eq!(run(&mut s, "listConnections", json!({"args": ["lambert2SG.surfaceShader"]})), json!([]));
    run(&mut s, "connectAttr", json!({"args": ["lambert2.outColor", "lambert2SG.surfaceShader"], "force": true}));
    assert_eq!(run(&mut s, "sets", json!({"args": ["lambert2SG"], "q": true, "renderable": true})), json!(true));
    run(&mut s, "sets", json!({"args": [c.clone()], "edit": true, "forceElement": "lambert2SG"}));
    assert_eq!(run(&mut s, "sets", json!({"args": ["lambert2SG"], "q": true})), json!(["pCubeShape1"]));
    assert_eq!(run(&mut s, "sets", json!({"args": ["initialShadingGroup"], "q": true})), json!([]));
    assert_eq!(run(&mut s, "listConnections", json!({"args": ["pCubeShape1"], "type": "shadingEngine"})), json!(["lambert2SG"]));
    assert_eq!(run(&mut s, "listSets", json!({"type": 1, "object": "pCubeShape1"})), json!(["lambert2SG"]));
    assert_eq!(run(&mut s, "listSets", json!({"type": 1, "object": c.clone(), "extendToShape": true})), json!(["lambert2SG"]));
    assert_eq!(run(&mut s, "sets", json!({"args": [c.clone()], "isMember": "lambert2SG"})), json!(true));
    // A plain listSets stays as it was: sets the user made.
    assert_eq!(run(&mut s, "listSets", json!({})), json!(["defaultLightSet", "defaultObjectSet"]));
    assert!(s.undo());
    assert_eq!(run(&mut s, "sets", json!({"args": ["initialShadingGroup"], "q": true})), json!(["pCubeShape1"]));
    assert_eq!(run(&mut s, "listConnections", json!({"args": ["pCubeShape1"], "type": "shadingEngine"})), json!(["initialShadingGroup"]));
}

#[test]
fn faces_take_a_material_and_the_rest_keep_theirs() {
    let mut s = scene();
    let c = cube(&mut s);
    let mat = run(&mut s, "shadingNode", json!({"args": ["lambert"], "asShader": true})).as_str().unwrap().to_string();
    run(&mut s, "sets", json!({"renderable": true, "empty": true, "noSurfaceShader": true, "name": "redSG"}));
    run(&mut s, "connectAttr", json!({"args": [format!("{mat}.outColor"), "redSG.surfaceShader"]}));
    run(&mut s, "sets", json!({"args": [format!("{c}.f[0:3]")], "edit": true, "forceElement": "redSG"}));
    assert_eq!(run(&mut s, "sets", json!({"args": ["redSG"], "q": true})), json!(["pCube1.f[0:3]"]));
    assert_eq!(run(&mut s, "sets", json!({"args": ["initialShadingGroup"], "q": true})), json!(["pCube1.f[4:5]"]));
    let both = run(&mut s, "listConnections", json!({"args": ["pCubeShape1"], "type": "shadingEngine"}));
    assert_eq!(both, json!(["initialShadingGroup", "redSG"]));
    assert_eq!(run(&mut s, "listConnections", json!({"args": ["redSG.dagSetMembers"], "plugs": true})), json!(["pCubeShape1.instObjGroups[0].objectGroups[1]"]));
    assert_eq!(run(&mut s, "sets", json!({"args": [format!("{c}.f[1]")], "isMember": "redSG"})), json!(true));
    assert_eq!(run(&mut s, "sets", json!({"args": [format!("{c}.f[4]")], "isMember": "redSG"})), json!(false));
    // A face again from the first group.
    run(&mut s, "sets", json!({"args": [format!("{c}.f[0]")], "edit": true, "forceElement": "initialShadingGroup"}));
    assert_eq!(run(&mut s, "sets", json!({"args": ["redSG"], "q": true})), json!(["pCube1.f[1:3]"]));
    assert_eq!(run(&mut s, "sets", json!({"args": ["initialShadingGroup"], "q": true})), json!(["pCube1.f[0]", "pCube1.f[4:5]"]));
    // The whole object takes over.
    run(&mut s, "sets", json!({"args": [c.clone()], "edit": true, "forceElement": "redSG"}));
    assert_eq!(run(&mut s, "sets", json!({"args": ["redSG"], "q": true})), json!(["pCubeShape1"]));
    assert_eq!(run(&mut s, "sets", json!({"args": ["initialShadingGroup"], "q": true})), json!([]));
    assert_eq!(run(&mut s, "listConnections", json!({"args": ["redSG.dagSetMembers"], "plugs": true})), json!(["pCubeShape1.instObjGroups[0]"]));
    // Vertices don't take materials.
    assert!(s.run("sets", json!({"args": [format!("{c}.vtx[0]")], "edit": true, "forceElement": "redSG"})).is_err());
}

#[test]
fn hyper_shade_assigns_and_selects() {
    let mut s = scene();
    let c = cube(&mut s);
    let d = cube(&mut s);
    run(&mut s, "shadingNode", json!({"args": ["blinn"], "asShader": true, "name": "shiny"}));
    run(&mut s, "select", json!({"args": [c.clone()]}));
    run(&mut s, "hyperShade", json!({"assign": "shiny"}));
    assert_eq!(run(&mut s, "sets", json!({"args": ["shinySG"], "q": true})), json!(["pCubeShape1"]));
    assert_eq!(run(&mut s, "ls", json!({"type": "materialInfo"})).as_array().unwrap().len(), 2);
    // The shading group is made once.
    run(&mut s, "select", json!({"args": [format!("{d}.f[2]")]}));
    run(&mut s, "hyperShade", json!({"assign": "shiny"}));
    assert_eq!(run(&mut s, "ls", json!({"type": "shadingEngine"})), json!(["initialShadingGroup", "shinySG"]));
    assert_eq!(run(&mut s, "sets", json!({"args": ["shinySG"], "q": true})), json!(["pCubeShape1", "pCube2.f[2]"]));
    run(&mut s, "select", json!({"clear": true}));
    run(&mut s, "hyperShade", json!({"objects": "shiny"}));
    let picked = run(&mut s, "ls", json!({"selection": true}));
    assert!(picked.as_array().unwrap().iter().any(|x| x == "pCube2.f[2]"), "{picked}");
    run(&mut s, "select", json!({"args": [c.clone()]}));
    run(&mut s, "hyperShade", json!({"shaderNetworksSelectMaterialNodes": true}));
    assert_eq!(run(&mut s, "ls", json!({"selection": true})), json!(["shiny"]));
    // lambert1 can be assigned like any other, and neither default goes away.
    run(&mut s, "select", json!({"args": [c.clone()]}));
    run(&mut s, "hyperShade", json!({"assign": "lambert1"}));
    assert!(run(&mut s, "sets", json!({"args": ["initialShadingGroup"], "q": true})).as_array().unwrap().iter().any(|x| x == "pCubeShape1"));
    assert!(s.run("delete", json!({"args": ["lambert1"]})).is_err());
    assert!(s.run("delete", json!({"args": ["initialShadingGroup"]})).is_err());
    assert!(s.run("rename", json!({"args": ["lambert1", "x"]})).is_err());
}

#[test]
fn default_navigation_and_connections_through_a_network() {
    let mut s = scene();
    run(&mut s, "shadingNode", json!({"args": ["lambert"], "asShader": true}));
    run(&mut s, "shadingNode", json!({"args": ["file"], "asTexture": true}));
    run(&mut s, "shadingNode", json!({"args": ["place2dTexture"], "asUtility": true}));
    run(&mut s, "defaultNavigation", json!({"connectToExisting": true, "source": "file1", "destination": "lambert2.color"}));
    assert_eq!(run(&mut s, "listConnections", json!({"args": ["lambert2.color"], "plugs": true})), json!(["file1.outColor"]));
    run(&mut s, "defaultNavigation", json!({"connectToExisting": true, "source": "file1", "destination": "lambert2.transparency"}));
    assert_eq!(run(&mut s, "listConnections", json!({"args": ["lambert2.transparency"], "plugs": true})), json!(["file1.outTransparency"]));
    run(&mut s, "defaultNavigation", json!({"connectToExisting": true, "source": "place2dTexture1", "destination": "file1"}));
    assert_eq!(run(&mut s, "listConnections", json!({"args": ["file1.uvCoord"], "plugs": true})), json!(["place2dTexture1.outUV"]));
    run(&mut s, "setAttr", json!({"args": ["place2dTexture1.repeatU", 4.0]}));
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["file1.repeatU"]})), json!(4.0));
    assert!(s.undo());
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["file1.repeatU"]})), json!(1.0));
}

#[test]
fn scripts_connect_dag_set_members_by_hand() {
    let mut s = scene();
    cube(&mut s);
    run(&mut s, "sets", json!({"renderable": true, "empty": true, "name": "byHand"}));
    run(&mut s, "connectAttr", json!({"args": ["pCubeShape1.instObjGroups[0]", "byHand.dagSetMembers"], "nextAvailable": true, "force": true}));
    assert_eq!(run(&mut s, "sets", json!({"args": ["byHand"], "q": true})), json!(["pCubeShape1"]));
    assert_eq!(run(&mut s, "sets", json!({"args": ["initialShadingGroup"], "q": true})), json!([]));
    run(&mut s, "disconnectAttr", json!({"args": ["pCubeShape1.instObjGroups[0]", "byHand.dagSetMembers[0]"]}));
    assert_eq!(run(&mut s, "sets", json!({"args": ["byHand"], "q": true})), json!([]));
}

#[test]
fn duplicates_keep_their_material_and_deleted_meshes_leave() {
    let mut s = scene();
    let c = cube(&mut s);
    run(&mut s, "shadingNode", json!({"args": ["lambert"], "asShader": true}));
    run(&mut s, "select", json!({"args": [format!("{c}.f[0]")]}));
    run(&mut s, "hyperShade", json!({"assign": "lambert2"}));
    run(&mut s, "duplicate", json!({"args": [c.clone()]}));
    assert_eq!(run(&mut s, "sets", json!({"args": ["lambert2SG"], "q": true})), json!(["pCube1.f[0]", "pCube2.f[0]"]));
    run(&mut s, "delete", json!({"args": [c.clone()]}));
    assert_eq!(run(&mut s, "sets", json!({"args": ["lambert2SG"], "q": true})), json!(["pCube2.f[0]"]));
    assert!(s.undo());
    assert_eq!(run(&mut s, "sets", json!({"args": ["lambert2SG"], "q": true})), json!(["pCube1.f[0]", "pCube2.f[0]"]));
}

#[test]
fn every_face_finds_a_group() {
    let mut s = scene();
    cube(&mut s);
    let id = s.graph.find("pCubeShape1").unwrap().unwrap();
    let mesh = aztek_scene::components::mesh_of(&mut s.graph, id).unwrap();
    // Group 0 holds face 0, group 1 face 5; the rest flood from their neighbours.
    let groups = vec![(1, Some(vec![0])), (2, Some(vec![5]))];
    let out = shading::face_groups(&mesh, &groups);
    assert_eq!(out.len(), 6);
    assert_eq!(out[0], 0);
    assert_eq!(out[5], 1);
    assert!(out.iter().all(|&k| k < 2));
    assert!(shading::face_groups(&mesh, &[(1, None)]).iter().all(|&k| k == 0));
}

#[test]
fn texture_paths_are_found_as_they_are_written_then_by_name() {
    let dir = std::env::temp_dir().join(format!("aztek_tex_{}", std::process::id()));
    let (scenes, images) = (dir.join("scenes"), dir.join("sourceimages"));
    std::fs::create_dir_all(&scenes).unwrap();
    std::fs::create_dir_all(images.join("sub")).unwrap();
    let scene = scenes.join("a.ma");
    std::fs::write(scenes.join("Beside.png"), b"x").unwrap();
    std::fs::write(images.join("Project.png"), b"x").unwrap();
    std::fs::write(images.join("sub").join("Deep.png"), b"x").unwrap();
    let (sc, pr) = (scene.to_str().unwrap().to_string(), dir.to_str().unwrap().to_string());
    let find = |w: &str| shading::resolve_texture_uncached(w, Some(&sc), Some(&pr));
    // As written.
    let abs = images.join("Project.png");
    assert_eq!(find(abs.to_str().unwrap()), Some(abs.clone()));
    // From another machine: by name, next to the scene first.
    assert_eq!(find("D:/somewhere/Beside.png"), Some(scenes.join("Beside.png")));
    assert_eq!(find("D:\\somewhere\\Beside.png"), Some(scenes.join("Beside.png")));
    assert_eq!(find("D:/somewhere/beside.PNG"), Some(scenes.join("Beside.png")));
    // In the project's sourceimages.
    assert_eq!(find("D:/other/Project.png"), Some(images.join("Project.png")));
    assert_eq!(find("sourceimages/Project.png"), Some(images.join("Project.png")));
    assert_eq!(find("D:/proj/sourceimages/sub/Deep.png"), Some(images.join("sub").join("Deep.png")));
    assert_eq!(find("D:/nowhere/Missing.png"), None);
    assert_eq!(find(""), None);
    std::fs::remove_dir_all(&dir).unwrap();
}

// ---------------------------------------------------------------------------------------------------------------------
// Files

const REF_FILE: &str = r#"//Maya ASCII 2024 scene
requires maya "2024";
currentUnit -l centimeter -a degree -t film;
createNode transform -n "box";
createNode mesh -n "boxShape" -p "box";
	setAttr -s 2 ".iog[0].og";
	setAttr ".iog[0].og[0].gcl" -type "componentList" 1 "f[0]";
	setAttr ".iog[0].og[1].gcl" -type "componentList" 1 "f[1]";
	setAttr -s 4 ".vt" 0 0 0 1 0 0 1 0 1 0 0 1;
	setAttr -s 6 ".vt[4:5]" 2 0 0 2 0 1;
	setAttr -s 7 ".ed" 0 1 0 1 2 0 2 3 0 3 0 0 1 4 0 4 5 0 5 2 0;
	setAttr -s 2 ".fc[0:1]" -type "polyFaces" f 4 0 1 2 3 f 4 4 5 6 -2;
createNode transform -n "whole";
createNode mesh -n "wholeShape" -p "whole";
	setAttr -s 4 ".vt" 0 0 0 1 0 0 1 1 0 0 1 0;
	setAttr -s 4 ".ed" 0 1 0 1 2 0 2 3 0 3 0 0;
	setAttr -s 1 ".fc[0]" -type "polyFaces" f 4 0 1 2 3;
createNode phong -n "skin";
	setAttr ".c" -type "float3" 0.25 0.5 0.75 ;
	setAttr ".cp" 8;
createNode lambert -n "cloth";
createNode shadingEngine -n "skinSG";
	setAttr ".ihi" 0;
	setAttr -s 2 ".dsm";
	setAttr ".ro" yes;
	setAttr -s 2 ".gn";
createNode shadingEngine -n "clothSG";
	setAttr ".ro" yes;
createNode materialInfo -n "materialInfo1";
createNode file -n "skinFile";
	setAttr ".ftn" -type "string" "D:/elsewhere/Skin.png";
	setAttr ".cs" -type "string" "sRGB";
createNode place2dTexture -n "skinPlace";
	setAttr ".re" -type "float2" 2 3 ;
	setAttr ".of" -type "float2" 0.25 0.5 ;
createNode groupId -n "groupId1";
createNode groupId -n "groupId2";
select -ne :lambert1;
	setAttr ".c" -type "float3" 0.1 0.2 0.3 ;
select -ne :initialShadingGroup;
	setAttr -s 2 ".dsm";
	setAttr ".ro" yes;
connectAttr "groupId1.id" "boxShape.iog.og[0].gid";
connectAttr "skinSG.mwc" "boxShape.iog.og[0].gco";
connectAttr "groupId2.id" "boxShape.iog.og[1].gid";
connectAttr ":initialShadingGroup.mwc" "boxShape.iog.og[1].gco";
connectAttr "skin.oc" "skinSG.ss";
connectAttr "boxShape.iog.og[0]" "skinSG.dsm" -na;
connectAttr "wholeShape.iog" "skinSG.dsm" -na;
connectAttr "groupId1.msg" "skinSG.gn" -na;
connectAttr "boxShape.iog.og[1]" ":initialShadingGroup.dsm" -na;
connectAttr "groupId2.msg" ":initialShadingGroup.gn" -na;
connectAttr "cloth.oc" "clothSG.ss";
connectAttr "skinSG.msg" "materialInfo1.sg";
connectAttr "skin.msg" "materialInfo1.m";
connectAttr "skinFile.msg" "materialInfo1.t" -na;
connectAttr "skinFile.oc" "skin.c";
connectAttr "skinPlace.c" "skinFile.c";
connectAttr "skinPlace.tf" "skinFile.tf";
connectAttr "skinPlace.rf" "skinFile.rf";
connectAttr "skinPlace.mu" "skinFile.mu";
connectAttr "skinPlace.mv" "skinFile.mv";
connectAttr "skinPlace.s" "skinFile.s";
connectAttr "skinPlace.wu" "skinFile.wu";
connectAttr "skinPlace.wv" "skinFile.wv";
connectAttr "skinPlace.re" "skinFile.re";
connectAttr "skinPlace.of" "skinFile.of";
connectAttr "skinPlace.r" "skinFile.ro";
connectAttr "skinPlace.n" "skinFile.n";
connectAttr "skinPlace.vt1" "skinFile.vt1";
connectAttr "skinPlace.vt2" "skinFile.vt2";
connectAttr "skinPlace.vt3" "skinFile.vt3";
connectAttr "skinPlace.vc1" "skinFile.vc1";
connectAttr "skinPlace.o" "skinFile.uv";
connectAttr "skinPlace.ofs" "skinFile.fs";
connectAttr "skinSG.pa" ":renderPartition.st" -na;
connectAttr "skin.msg" ":defaultShaderList1.s" -na;
connectAttr "skinFile.msg" ":defaultTextureList1.tx" -na;
// End of test.ma
"#;

fn check_read(s: &mut Session, w: &[String]) {
    assert!(!w.iter().any(|x| x.contains("not represented") && (x.contains("phong") || x.contains("file") || x.contains("place2d"))), "{w:?}");
    assert!(!w.iter().any(|x| x.contains("skin") || x.contains("place") || x.contains("SG")), "{w:?}");
    assert_eq!(run(s, "ls", json!({"type": "lambert"})), json!(["lambert1", "skin", "cloth"]));
    assert_eq!(run(s, "ls", json!({"type": "phong"})), json!(["skin"]));
    assert_eq!(run(s, "ls", json!({"type": "file"})), json!(["skinFile"]));
    assert_eq!(run(s, "ls", json!({"type": "place2dTexture"})), json!(["skinPlace"]));
    assert!(close(&run(s, "getAttr", json!({"args": ["lambert1.color"]})), &[0.1, 0.2, 0.3]));
    assert_eq!(run(s, "getAttr", json!({"args": ["skin.cosinePower"]})), json!(8.0));
    assert_eq!(run(s, "getAttr", json!({"args": ["skinFile.fileTextureName"]})), "D:/elsewhere/Skin.png");
    assert_eq!(run(s, "getAttr", json!({"args": ["skinFile.repeatV"]})), json!(3.0));
    assert_eq!(run(s, "getAttr", json!({"args": ["skinFile.offsetU"]})), json!(0.25));
    assert_eq!(run(s, "listConnections", json!({"args": ["skin.color"]})), json!(["skinFile"]));
    assert_eq!(run(s, "listConnections", json!({"args": ["skinSG.surfaceShader"]})), json!(["skin"]));
    assert_eq!(run(s, "listConnections", json!({"args": ["skinFile.uvCoord"]})), json!(["skinPlace"]));
    let mut held: Vec<String> = serde_json::from_value(run(s, "sets", json!({"args": ["skinSG"], "q": true}))).unwrap();
    held.sort();
    assert_eq!(held, vec!["box.f[0]", "wholeShape"], "skinSG");
    assert_eq!(run(s, "sets", json!({"args": ["initialShadingGroup"], "q": true})), json!(["box.f[1]"]));
    assert_eq!(run(s, "sets", json!({"args": ["clothSG"], "q": true})), json!([]));
    let both = run(s, "listConnections", json!({"args": ["boxShape"], "type": "shadingEngine"}));
    assert_eq!(both.as_array().unwrap().len(), 2, "{both}");
}

#[test]
fn reads_the_reference_layout_and_keeps_it_through_a_write() {
    let mut s = Session::new();
    let w = aztek_scene::ma_file::read_text(&mut s, REF_FILE, false);
    check_read(&mut s, &w);

    let (text, w) = aztek_scene::ma_file::write_text(&mut s, "out.ma", None);
    assert!(w.is_empty(), "{w:?}");
    // The layout, line by line.
    assert!(text.contains("createNode phong -n \"skin\";"), "{text}");
    assert!(!text.contains("createNode lambert -n \"lambert1\""), "{text}");
    assert!(text.contains("select -ne :lambert1;"), "{text}");
    assert!(text.contains("createNode shadingEngine -n \"skinSG\";"), "{text}");
    assert!(text.contains("setAttr \".c\" -type \"float3\" 0.1 0.2 0.3"), "{text}");
    assert!(text.contains("-type \"float2\" 2 3"), "{text}");
    assert!(text.contains("setAttr \".ftn\" -type \"string\" \"D:/elsewhere/Skin.png\""), "{text}");
    assert!(text.contains("connectAttr \"skin.oc\" \"skinSG.ss\";"), "{text}");
    assert!(text.contains("connectAttr \"skinFile.oc\" \"skin.c\";"), "{text}");
    assert!(text.contains("connectAttr \"skinPlace.o\" \"skinFile.uv\";"), "{text}");
    assert!(text.contains("connectAttr \"wholeShape.iog\" \"skinSG.dsm\" -na;"), "{text}");
    assert!(text.contains("connectAttr \"boxShape.iog.og[1]\" \"skinSG.dsm\" -na;"), "{text}");
    assert!(text.contains("connectAttr \"boxShape.iog.og[0]\" \":initialShadingGroup.dsm\" -na;"), "{text}");
    assert!(text.contains("setAttr \".iog[0].og[0].gcl\" -type \"componentList\""), "{text}");
    assert!(text.contains(".iog.og[0].gid\";"), "{text}");
    assert!(text.contains("\"skinSG.mwc\" \"boxShape.iog.og[1].gco\""), "{text}");
    assert!(text.contains("\"skinSG.pa\" \":renderPartition.st\" -na;"), "{text}");
    assert!(text.contains("\"skin.msg\" \":defaultShaderList1.s\" -na;"), "{text}");
    assert!(text.contains("\"skinFile.msg\" \":defaultTextureList1.tx\" -na;"), "{text}");
    assert!(text.contains("\"skinPlace.msg\" \":defaultRenderUtilityList1.u\" -na;"), "{text}");
    assert!(text.contains("relationship \"link\" \":lightLinker1\" \"skinSG.message\" \":defaultLightSet.message\";"), "{text}");

    let mut b = Session::new();
    let w = aztek_scene::ma_file::read_text(&mut b, &text, false);
    check_read(&mut b, &w);
    // And written again, the same text.
    let (again, _) = aztek_scene::ma_file::write_text(&mut b, "out.ma", None);
    assert_eq!(text, again);
}

#[test]
fn group_parts_hold_the_faces_when_the_shape_does_not() {
    let file = r#"createNode transform -n "m";
createNode mesh -n "mShape" -p "m";
	setAttr -s 4 ".vt" 0 0 0 1 0 0 1 1 0 0 1 0;
	setAttr -s 4 ".vt[4:5]" 2 0 0 2 1 0;
	setAttr -s 7 ".ed" 0 1 0 1 2 0 2 3 0 3 0 0 1 4 0 4 5 0 5 2 0;
	setAttr -s 2 ".fc[0:1]" -type "polyFaces" f 4 0 1 2 3 f 4 4 5 6 -2;
createNode lambert -n "a";
createNode shadingEngine -n "aSG";
createNode groupId -n "gid";
createNode groupParts -n "parts";
	setAttr ".ic" -type "componentList" 1 "f[1]";
connectAttr "a.oc" "aSG.ss";
connectAttr "gid.id" "mShape.iog.og[0].gid";
connectAttr "gid.id" "parts.gi";
connectAttr "mShape.iog.og[0]" "aSG.dsm" -na;
"#;
    let mut s = Session::new();
    aztek_scene::ma_file::read_text(&mut s, file, false);
    assert_eq!(run(&mut s, "sets", json!({"args": ["aSG"], "q": true})), json!(["m.f[1]"]));
}

#[test]
fn native_scene_files_keep_shading() {
    let mut s = scene();
    let c = cube(&mut s);
    run(&mut s, "shadingNode", json!({"args": ["lambert"], "asShader": true}));
    run(&mut s, "select", json!({"args": [format!("{c}.f[0:1]")]}));
    run(&mut s, "hyperShade", json!({"assign": "lambert2"}));
    let doc = aztek_scene::scene_file::serialize(&s.graph, false);
    let mut b = Session::new();
    let w = aztek_scene::scene_file::deserialize(&mut b.graph, &doc).unwrap();
    assert!(w.is_empty(), "{w:?}");
    assert_eq!(run(&mut b, "sets", json!({"args": ["lambert2SG"], "q": true})), json!(["pCube1.f[0:1]"]));
    assert_eq!(run(&mut b, "sets", json!({"args": ["initialShadingGroup"], "q": true})), json!(["pCube1.f[2:5]"]));
    assert_eq!(run(&mut b, "listConnections", json!({"args": ["lambert2SG.surfaceShader"]})), json!(["lambert2"]));
    assert_eq!(run(&mut b, "listConnections", json!({"args": ["lambert2SG.dagSetMembers"]})), json!(["pCube1"]));
}

#[test]
fn an_extrusions_new_faces_go_with_the_face_they_grew_from() {
    let mut s = scene();
    let c = cube(&mut s);
    run(&mut s, "shadingNode", json!({"args": ["lambert"], "asShader": true}));
    run(&mut s, "select", json!({"args": [format!("{c}.f[1]")]}));
    run(&mut s, "hyperShade", json!({"assign": "lambert2"}));
    run(&mut s, "polyExtrudeFacet", json!({"localTranslateZ": 0.5}));
    let id = s.graph.find("pCubeShape1").unwrap().unwrap();
    let mesh = aztek_scene::components::mesh_of(&mut s.graph, id).unwrap();
    let groups = shading::membership(&s.graph).remove(&id).unwrap();
    let green = groups.iter().position(|g| g.1.as_ref().is_some_and(|f| f == &vec![1])).unwrap();
    let out = shading::face_groups(&mesh, &groups);
    assert_eq!(out.iter().filter(|&&k| k == green).count(), 5, "{out:?}");
}

#[test]
fn faces_follow_a_deletion_with_history() {
    let mut s = scene();
    let c = cube(&mut s);
    run(&mut s, "shadingNode", json!({"args": ["lambert"], "asShader": true}));
    run(&mut s, "select", json!({"args": [format!("{c}.f[5]")]}));
    run(&mut s, "hyperShade", json!({"assign": "lambert2"}));
    run(&mut s, "polyDelFacet", json!({"args": [format!("{c}.f[0]")]}));
    let held = run(&mut s, "sets", json!({"args": ["lambert2SG"], "q": true}));
    assert_eq!(held, json!(["pCube1.f[4]"]));
}

#[test]
fn exporting_an_object_takes_its_material_and_leaves_others() {
    let mut s = scene();
    let c = cube(&mut s);
    cube(&mut s);
    run(&mut s, "shadingNode", json!({"args": ["phong"], "asShader": true, "name": "kept"}));
    run(&mut s, "shadingNode", json!({"args": ["phong"], "asShader": true, "name": "left"}));
    run(&mut s, "shadingNode", json!({"args": ["file"], "asTexture": true, "name": "keptFile"}));
    run(&mut s, "connectAttr", json!({"args": ["keptFile.outColor", "kept.color"]}));
    run(&mut s, "select", json!({"args": [c.clone()]}));
    run(&mut s, "hyperShade", json!({"assign": "kept"}));
    run(&mut s, "select", json!({"args": ["pCube2"]}));
    run(&mut s, "hyperShade", json!({"assign": "left"}));
    let id = s.graph.find(&c).unwrap().unwrap();
    let (text, _) = aztek_scene::ma_file::write_text(&mut s, "x.ma", Some(&[id]));
    assert!(text.contains("createNode phong -n \"kept\";"), "{text}");
    assert!(text.contains("createNode file -n \"keptFile\";"), "{text}");
    assert!(text.contains("createNode shadingEngine -n \"keptSG\";"), "{text}");
    assert!(!text.contains("\"left\""), "{text}");
}

/// A surface keeps its shading group through a save, and saving a scene warns about nothing it writes.
#[test]
fn a_surface_keeps_its_group_through_a_save_and_default_cameras_raise_no_warning() {
    let mut s = scene();
    run(&mut s, "nurbsPlane", json!({"name": "ribbon"}));
    assert!(run(&mut s, "sets", json!({"query": true, "args": ["initialShadingGroup"]})).to_string().contains("ribbonShape"));
    let (text, warnings) = aztek_scene::ma_file::write_text(&mut s, "a.ma", None);
    assert!(warnings.is_empty(), "{warnings:?}");
    let mut back = Session::new();
    let w = aztek_scene::ma_file::read_text(&mut back, &text, false);
    assert!(w.is_empty(), "{w:?}");
    assert!(run(&mut back, "sets", json!({"query": true, "args": ["initialShadingGroup"]})).to_string().contains("ribbonShape"));
}
