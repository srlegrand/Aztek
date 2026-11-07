//! .ma: files as the reference writes them load to the right scene values; what Aztek writes reads back.

use aztek_geo::{names, Mesh};
use aztek_scene::ma_file::{self, read_text, write_text};
use aztek_scene::mesh_nodes::MeshNode;
use aztek_scene::{anim, components, skin, PlugRef, Session};
use serde_json::{json, Value};
use std::sync::Arc;

fn run(s: &mut Session, name: &str, call: Value) -> Value {
    s.run(name, call).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn get(s: &mut Session, plug: &str) -> Vec<f64> {
    let v = run(s, "getAttr", json!({"args": [plug]}));
    match v {
        Value::Array(a) => a.iter().map(|x| x.as_f64().unwrap()).collect(),
        Value::Bool(b) => vec![b as i32 as f64],
        v => vec![v.as_f64().unwrap_or_else(|| panic!("{plug}: {v}"))],
    }
}

fn mesh_of(s: &mut Session, name: &str) -> Arc<Mesh> {
    s.graph.evaluate();
    let id = s.graph.find(name).unwrap().unwrap_or_else(|| panic!("no {name}"));
    let shape = components::as_mesh(&s.graph, id).unwrap();
    s.graph.value(&PlugRef::new(shape, MeshNode::OUT_MESH as i32)).unwrap().as_mesh().unwrap().clone()
}

fn near(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-5)
}

const CUBE: &str = r#"//Maya ASCII 2024 scene
//Name: cube.ma
//Last modified: Fri, Nov 06, 2026 10:00:00 AM
//Codeset: UTF-8
requires maya "2024";
requires "stereoCamera" "10.0";
currentUnit -l centimeter -a degree -t film;
fileInfo "application" "maya";
fileInfo "product" "Maya 2024";
createNode transform -s -n "persp";
	rename -uid "1F3C0C80-4F4B-5F1E-4C3B-1E9D7B0A5B6C";
	setAttr ".v" no;
	setAttr ".t" -type "double3" 28 21 28 ;
createNode camera -s -n "perspShape" -p "persp";
	setAttr -k off ".v" no;
	setAttr ".fl" 34.999999999999993;
createNode transform -s -n "top";
createNode camera -s -n "topShape" -p "top";
createNode transform -s -n "front";
createNode camera -s -n "frontShape" -p "front";
createNode transform -s -n "side";
createNode camera -s -n "sideShape" -p "side";
createNode transform -n "pCube1";
	rename -uid "A0";
	setAttr ".t" -type "double3" 1 2 3 ;
	setAttr ".r" -type "double3" 0 45 0 ;
	setAttr ".s" -type "double3" 2 2 2 ;
	setAttr ".ro" 3;
	setAttr ".rp" -type "double3" 0.5 0 0 ;
	setAttr -l on ".sx";
createNode mesh -n "pCubeShape1" -p "pCube1";
	rename -uid "A1";
	setAttr -k off ".v";
	setAttr ".vir" yes;
	setAttr ".vif" yes;
	setAttr ".uvst[0].uvsn" -type "string" "map1";
	setAttr -s 14 ".uvst[0].uvsp[0:13]" -type "float2" 0.375 0 0.625 0 0.375
		 0.25 0.625 0.25 0.375 0.5 0.625 0.5 0.375 0.75 0.625 0.75 0.375 1 0.625 1 0.875
		 0 0.875 0.25 0.125 0 0.125 0.25;
	setAttr ".cuvs" -type "string" "map1";
	setAttr ".dcc" -type "string" "Ambient+Diffuse";
	setAttr ".covm[0]"  0 1 1;
	setAttr ".cdvm[0]"  0 1 1;
	setAttr -s 8 ".vt[0:7]"  -0.5 -0.5 0.5 0.5 -0.5 0.5 -0.5 0.5 0.5 0.5 0.5 0.5
		 -0.5 0.5 -0.5 0.5 0.5 -0.5 -0.5 -0.5 -0.5 0.5 -0.5 -0.5;
	setAttr -s 12 ".ed[0:11]"  0 1 0 2 3 0 4 5 0 6 7 0 0 2 0 1 3 0 2 4 0
		 3 5 0 4 6 0 5 7 0 6 0 0 7 1 0;
	setAttr -s 6 -ch 24 ".fc[0:5]" -type "polyFaces"
		f 4 0 5 -2 -5
		mu 0 4 0 1 3 2
		f 4 1 7 -3 -7
		mu 0 4 2 3 5 4
		f 4 2 9 -4 -9
		mu 0 4 4 5 7 6
		f 4 3 11 -1 -11
		mu 0 4 6 7 9 8
		f 4 -12 -10 -8 -6
		mu 0 4 1 10 11 3
		f 4 10 4 6 8
		mu 0 4 12 0 2 13;
	setAttr ".cd" -type "dataPolyComponent" Index_Data Edge 0 ;
	setAttr ".pt[1]" -type "float3" 0 0.25 0 ;
createNode lightLinker -s -n "lightLinker1";
	setAttr -s 2 ".lnk";
createNode displayLayerManager -n "layerManager";
createNode displayLayer -n "defaultLayer";
createNode renderLayerManager -n "renderLayerManager";
createNode renderLayer -n "defaultRenderLayer";
	setAttr ".g" yes;
createNode script -n "sceneConfigurationScriptNode";
	setAttr ".b" -type "string" "playbackOptions -min 1 -max 120 -ast 1 -aet 200 ";
	setAttr ".st" 6;
createNode polyBevel -n "ignoredNode1";
select -ne :time1;
	setAttr ".o" 7;
	setAttr ".unw" 7;
select -ne :initialShadingGroup;
	setAttr ".ro" yes;
connectAttr "layerManager.dli[0]" "defaultLayer.id";
connectAttr "pCubeShape1.iog" ":initialShadingGroup.dsm" -na;
// End of cube.ma
"#;

#[test]
fn a_cube_with_stored_geometry_loads() {
    let mut s = Session::new();
    let w = read_text(&mut s, CUBE, false);
    assert!(w.iter().any(|x| x.contains("polyBevel")), "{w:?}");
    assert!(!w.iter().any(|x| x.contains("camera") || x.contains("lightLinker") || x.contains("Layer")), "{w:?}");

    // Default cameras and their shapes are not scene objects.
    for name in ["persp", "perspShape", "top", "frontShape", "sideShape"] {
        assert_eq!(s.graph.find(name).unwrap(), None, "{name}");
    }
    assert!(near(&get(&mut s, "pCube1.t"), &[1.0, 2.0, 3.0]));
    assert!(near(&get(&mut s, "pCube1.r"), &[0.0, 45.0, 0.0]));
    assert!(near(&get(&mut s, "pCube1.s"), &[2.0, 2.0, 2.0]));
    assert!(near(&get(&mut s, "pCube1.rp"), &[0.5, 0.0, 0.0]));
    assert_eq!(get(&mut s, "pCube1.ro")[0], 3.0);
    let sx = s.graph.plug("pCube1.sx").unwrap();
    assert!(s.graph.is_locked(&sx));

    let m = mesh_of(&mut s, "pCube1");
    assert_eq!((m.num_points(), m.num_edges(), m.num_faces(), m.num_corners()), (8, 12, 6, 24));
    // .vt plus .pt.
    assert!(near(&m.position(1).to_array(), &[0.5, -0.25, 0.5]));
    assert!(near(&m.position(7).to_array(), &[0.5, -0.5, -0.5]));
    // Face 0: edge 0 forward (0-1), edge 5 forward (1-3), edge 1 reversed (3-2), edge 4 reversed (2-0).
    assert_eq!(m.face_points(0), vec![0, 1, 3, 2]);
    assert_eq!(m.face_points(4), vec![1, 7, 5, 3]);

    // UVs per corner from mu, hard edges from the edge flag.
    let uv = m.corner_attrs().get::<[f32; 2]>("uv0");
    let cs = m.face_corners(4);
    assert_eq!(uv[cs[1] as usize], [0.875, 0.0]);
    assert_eq!(uv[cs[2] as usize], [0.875, 0.25]);
    let hard = m.edge_attrs().get::<bool>(names::HARD);
    assert!(hard[m.find_edge(0, 1) as usize]);

    assert_eq!(anim::frame(&s.graph), 7.0);
}

#[test]
fn a_cube_from_history_loads_live() {
    let text = r#"//Maya ASCII 2024 scene
requires maya "2024";
currentUnit -l centimeter -a degree -t film;
createNode transform -n "pCube1";
createNode mesh -n "pCubeShape1" -p "pCube1";
	setAttr -k off ".v";
	setAttr ".vir" yes;
	setAttr ".uvst[0].uvsn" -type "string" "map1";
	setAttr ".cuvs" -type "string" "map1";
createNode polyCube -n "polyCube1";
	setAttr ".w" 4;
	setAttr ".h" 6;
	setAttr ".d" 8;
	setAttr ".sw" 2;
	setAttr ".cuv" 4;
connectAttr "polyCube1.out" "pCubeShape1.i";
"#;
    let mut s = Session::new();
    let w = read_text(&mut s, text, false);
    assert!(w.is_empty(), "{w:?}");
    let m = mesh_of(&mut s, "pCube1");
    let b = m.bounds();
    assert!(near(&b.size().to_array(), &[4.0, 6.0, 8.0]));
    assert_eq!(m.num_faces(), 10);
    // Still live.
    run(&mut s, "setAttr", json!({"args": ["polyCube1.width", 10.0]}));
    let m = mesh_of(&mut s, "pCube1");
    assert!((m.bounds().size().x - 10.0).abs() < 1e-9);
}

#[test]
fn the_other_creators_load() {
    let text = r#"
createNode transform -n "ball";
createNode mesh -n "ballShape" -p "ball";
createNode polySphere -n "polySphere1";
	setAttr ".r" 3;
	setAttr ".sa" 8;
	setAttr ".sh" 6;
	setAttr ".ax" -type "double3" 0 0 1 ;
createNode transform -n "can";
createNode mesh -n "canShape" -p "can";
createNode polyCylinder -n "polyCylinder1";
	setAttr ".r" 2;
	setAttr ".h" 5;
	setAttr ".sa" 10;
createNode transform -n "cone";
createNode mesh -n "coneShape" -p "cone";
createNode polyCone -n "polyCone1";
	setAttr ".r" 2;
	setAttr ".h" 7;
createNode transform -n "floor";
createNode mesh -n "floorShape" -p "floor";
createNode polyPlane -n "polyPlane1";
	setAttr ".w" 12;
	setAttr ".h" 14;
	setAttr ".sw" 2;
	setAttr ".sh" 3;
createNode transform -n "ring";
createNode mesh -n "ringShape" -p "ring";
createNode polyTorus -n "polyTorus1";
	setAttr ".r" 5;
	setAttr ".sr" 1;
connectAttr "polySphere1.out" "ballShape.i";
connectAttr "polyCylinder1.out" "canShape.i";
connectAttr "polyCone1.out" "coneShape.i";
connectAttr "polyPlane1.out" "floorShape.i";
connectAttr "polyTorus1.out" "ringShape.i";
"#;
    let mut s = Session::new();
    let w = read_text(&mut s, text, false);
    assert!(w.is_empty(), "{w:?}");
    // The sphere's axis is Z: its poles lie along Z.
    let b = mesh_of(&mut s, "ball").bounds();
    assert!((b.size().z - 6.0).abs() < 1e-9, "{:?}", b.size());
    assert!((mesh_of(&mut s, "can").bounds().size().y - 5.0).abs() < 1e-9);
    assert!((mesh_of(&mut s, "cone").bounds().size().y - 7.0).abs() < 1e-9);
    let p = mesh_of(&mut s, "floor");
    assert!(near(&p.bounds().size().to_array(), &[12.0, 0.0, 14.0]));
    assert_eq!(p.num_faces(), 6);
    assert!((mesh_of(&mut s, "ring").bounds().size().x - 12.0).abs() < 1e-9);
}

#[test]
fn stored_geometry_wins_over_history_it_cannot_replay() {
    // A shape holding its result under a node Aztek has no way to replay.
    let text = r#"
createNode transform -n "tri";
createNode mesh -n "triShape" -p "tri";
	setAttr -s 3 ".vt[0:2]"  0 0 0 1 0 0 0 1 0;
	setAttr -s 3 ".ed[0:2]"  0 1 1 1 2 1 2 0 1;
	setAttr -s 1 -ch 3 ".fc[0]" -type "polyFaces" f 3 0 1 2;
createNode polyCube -n "polyCube1";
createNode polyMagic -n "polyMagic1";
connectAttr "polyCube1.out" "polyMagic1.ip";
connectAttr "polyMagic1.out" "triShape.i";
"#;
    let mut s = Session::new();
    let w = read_text(&mut s, text, false);
    assert!(w.iter().any(|x| x.contains("polyMagic")), "{w:?}");
    assert_eq!(mesh_of(&mut s, "tri").num_points(), 3);

    // No stored geometry: the creator, and a warning naming what was lost.
    let text = text
        .replace("setAttr -s 3 \".vt[0:2]\"  0 0 0 1 0 0 0 1 0;", "")
        .replace("setAttr -s 3 \".ed[0:2]\"  0 1 1 1 2 1 2 0 1;", "")
        .replace("setAttr -s 1 -ch 3 \".fc[0]\" -type \"polyFaces\" f 3 0 1 2;", "");
    let mut s = Session::new();
    let w = read_text(&mut s, &text, false);
    assert!(w.iter().any(|x| x.contains("no stored geometry") && x.contains("polyMagic")), "{w:?}");
    assert_eq!(mesh_of(&mut s, "tri").num_points(), 8);
}

const CHAIN: &str = r#"
createNode joint -n "joint1";
	setAttr ".t" -type "double3" 0 1 0 ;
	setAttr ".jo" -type "double3" 0 0 90 ;
	setAttr ".radi" 0.5;
createNode joint -n "joint2" -p "joint1";
	setAttr ".t" -type "double3" 2 0 0 ;
createNode joint -n "joint3" -p "|joint1|joint2";
	setAttr ".t" -type "double3" 3 0 0 ;
	setAttr ".ssc" no;
connectAttr "joint1.s" "joint2.is";
connectAttr "joint2.s" "joint3.is";
"#;

#[test]
fn a_joint_chain_loads() {
    let mut s = Session::new();
    let w = read_text(&mut s, CHAIN, false);
    assert!(w.is_empty(), "{w:?}");
    assert!(near(&get(&mut s, "joint1.jo"), &[0.0, 0.0, 90.0]));
    assert_eq!(get(&mut s, "joint1.radi")[0], 0.5);
    let j = run(&mut s, "joint", json!({"args": ["joint3"], "query": true}));
    let p: Vec<f64> = j.as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect();
    // jointOrient turns the chain up Y.
    assert!(near(&p, &[0.0, 6.0, 0.0]), "{p:?}");
    let parent = s.graph.parent(s.graph.find("joint3").unwrap().unwrap());
    assert_eq!(s.graph.node(parent).unwrap().name(), "joint2");
}

#[test]
fn animation_loads_with_tangents_and_infinities() {
    let text = r#"
currentUnit -l centimeter -a degree -t film;
createNode transform -n "ball";
createNode animCurveTL -n "ball_translateX";
	setAttr ".tan" 18;
	setAttr ".wgt" no;
	setAttr -s 3 ".ktv[0:2]"  1 0 12 3 24 0;
	setAttr -s 3 ".kit[0:2]"  1 2 3;
	setAttr -s 3 ".kot[0:2]"  1 2 5;
	setAttr -s 3 ".kix[0:2]"  1 1 1;
	setAttr -s 3 ".kiy[0:2]"  48 0 0;
	setAttr -s 3 ".kox[0:2]"  1 1 1;
	setAttr -s 3 ".koy[0:2]"  24 0 0;
	setAttr ".pre" 3;
	setAttr ".pst" 1;
createNode animCurveTA -n "ball_rotateY";
	setAttr ".tan" 10;
	setAttr -s 2 ".ktv[0:1]"  1 0 24 90;
	setAttr ".kbd[1]" yes;
createNode animCurveTU -n "ball_visibility";
	setAttr -s 2 ".ktv[0:1]"  1 1 24 0;
	setAttr -s 2 ".kot[0:1]"  5 5;
createNode animCurveTL -n "orphan_translateX";
	setAttr -s 1 ".ktv[0]"  1 0;
connectAttr "ball_translateX.o" "ball.tx";
connectAttr "ball_rotateY.o" "ball.ry";
connectAttr "ball_visibility.o" "ball.v";
connectAttr "orphan_translateX.o" "ghost.tx";
"#;
    let mut s = Session::new();
    let w = read_text(&mut s, text, false);
    assert!(w.is_empty(), "{w:?}");
    let id = s.graph.find("ball_translateX").unwrap().unwrap();
    let k = anim::keys_of(&s.graph, id);
    assert_eq!(k.keys.len(), 3);
    assert_eq!((k.keys[1].t, k.keys[1].v), (12.0, 3.0));
    assert_eq!(k.keys[0].itt, anim::Tangent::Fixed);
    assert_eq!(k.keys[1].itt, anim::Tangent::Linear);
    assert_eq!(k.keys[2].ott, anim::Tangent::Step);
    // kiy / kix per second, as per frame at 24 fps.
    assert!((k.keys[0].in_slope - 2.0).abs() < 1e-9);
    assert!((k.keys[0].out_slope - 1.0).abs() < 1e-9);
    assert_eq!(get(&mut s, "ball_translateX.preInfinity")[0], 3.0);
    let r = anim::keys_of(&s.graph, s.graph.find("ball_rotateY").unwrap().unwrap());
    // .tan is the type of keys that don't name one.
    assert_eq!(r.keys[0].itt, anim::Tangent::Clamped);
    assert!(r.keys[1].breakdown);
    assert!(s.graph.find("orphan_translateX").unwrap().is_none());

    run(&mut s, "currentTime", json!({"args": [12]}));
    s.graph.evaluate();
    assert!((get(&mut s, "ball.tx")[0] - 3.0).abs() < 1e-6);
    run(&mut s, "currentTime", json!({"args": [24]}));
    s.graph.evaluate();
    assert!((get(&mut s, "ball.ry")[0] - 90.0).abs() < 1e-6);
}

#[test]
fn driven_curves_follow_their_input() {
    let text = r#"
createNode transform -n "driver";
createNode transform -n "driven";
createNode animCurveUU -n "driven_sx";
	setAttr -s 2 ".ktv[0:1]"  0 1 10 3;
connectAttr "driver.tx" "driven_sx.i";
connectAttr "driven_sx.o" "driven.sx";
"#;
    let mut s = Session::new();
    let w = read_text(&mut s, text, false);
    assert!(w.is_empty(), "{w:?}");
    run(&mut s, "setAttr", json!({"args": ["driver.tx", 10]}));
    s.graph.evaluate();
    assert!((get(&mut s, "driven.sx")[0] - 3.0).abs() < 1e-6);
    let (text, _) = write_text(&mut s, "d.ma", None);
    assert!(text.contains(r#"connectAttr "driver.tx" "driven_sx.i";"#), "{text}");
    assert!(text.contains(r#"connectAttr "driven_sx.o" "driven.sx";"#), "{text}");
}

const SKINNED: &str = r#"//Maya ASCII 2024 scene
requires maya "2024";
currentUnit -l centimeter -a degree -t film;
createNode transform -n "bar";
createNode mesh -n "barShape" -p "bar";
	setAttr -k off ".v";
	setAttr ".vir" yes;
createNode mesh -n "barShapeOrig" -p "bar";
	setAttr -k off ".v";
	setAttr ".io" yes;
	setAttr ".uvst[0].uvsn" -type "string" "map1";
	setAttr -s 4 ".vt[0:3]"  0 0 0 2 0 0 2 1 0 0 1 0;
	setAttr -s 4 ".ed[0:3]"  0 1 0 1 2 0 2 3 0 3 0 0;
	setAttr -s 1 -ch 4 ".fc[0]" -type "polyFaces" f 4 0 1 2 3;
createNode joint -n "joint1";
createNode joint -n "joint2" -p "joint1";
	setAttr ".t" -type "double3" 2 0 0 ;
createNode skinCluster -n "skinCluster1";
	setAttr -s 4 ".wl";
	setAttr ".wl[0].w[0]"  1;
	setAttr ".wl[1].w[1]"  1;
	setAttr -s 2 ".wl[2].w[0:1]"  0.25 0.75;
	setAttr ".wl[3].w[0]"  1;
	setAttr -s 2 ".pm";
	setAttr ".pm[0]" -type "matrix" 1 0 0 0 0 1 0 0 0 0 1 0 0 0 0 1;
	setAttr ".pm[1]" -type "matrix" 1 0 0 0 0 1 0 0 0 0 1 0 -2 0 0 1;
	setAttr ".gm" -type "matrix" 1 0 0 0 0 1 0 0 0 0 1 0 0 0 0 1;
	setAttr -s 2 ".ma";
	setAttr -s 2 ".dpf[0:1]"  4 4;
	setAttr ".mmi" yes;
	setAttr ".mi" 3;
	setAttr ".dr" 2.5;
	setAttr ".ucm" yes;
createNode dagPose -n "bindPose1";
	setAttr -s 2 ".wm";
	setAttr -s 2 ".xm";
	setAttr ".xm[0]" -type "matrix" "xform" 1 1 1 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 1 0 0 0 1 1 1 1 yes;
	setAttr ".xm[1]" -type "matrix" "xform" 1 1 1 0 0 0 0 2 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 1 0 0 0 1 1 1 1 yes;
	setAttr -s 2 ".m";
	setAttr -s 2 ".p";
	setAttr ".bp" yes;
connectAttr "joint1.wm" "skinCluster1.ma[0]";
connectAttr "joint2.wm" "skinCluster1.ma[1]";
connectAttr "joint1.liw" "skinCluster1.lw[0]";
connectAttr "joint2.liw" "skinCluster1.lw[1]";
connectAttr "bindPose1.msg" "skinCluster1.bp";
connectAttr "barShapeOrig.w" "skinCluster1.ip[0].ig";
connectAttr "skinCluster1.og[0]" "barShape.i";
connectAttr "joint1.msg" "bindPose1.m[0]";
connectAttr "joint2.msg" "bindPose1.m[1]";
connectAttr "bindPose1.w" "bindPose1.p[0]";
connectAttr "bindPose1.m[0]" "bindPose1.p[1]";
connectAttr "joint1.bps" "bindPose1.wm[0]";
connectAttr "joint2.bps" "bindPose1.wm[1]";
"#;

fn weights(s: &Session, sk: &str) -> Vec<Vec<(u32, f64)>> {
    let id = s.graph.find(sk).unwrap().unwrap();
    skin::bind_of(&s.graph, id).unwrap().weights.clone()
}

#[test]
fn a_skin_cluster_loads_and_deforms() {
    let mut s = Session::new();
    let w = read_text(&mut s, SKINNED, false);
    assert!(w.is_empty(), "{w:?}");
    let sk = s.graph.find("skinCluster1").unwrap().unwrap();
    let bind = skin::bind_of(&s.graph, sk).unwrap();
    assert_eq!(bind.weights.len(), 4);
    assert_eq!(bind.weights[2], vec![(0, 0.25), (1, 0.75)]);
    assert!((bind.bind_pre[1].w_axis.x - -2.0).abs() < 1e-12);
    let names: Vec<String> = skin::influences(&s.graph, sk).iter().map(|&(_, j)| s.graph.path(j)).collect();
    assert_eq!(names, ["joint1", "joint2"]);
    assert_eq!(get(&mut s, "skinCluster1.maxInfluences")[0], 3.0);
    assert_eq!(get(&mut s, "skinCluster1.dropoffRate")[0], 2.5);

    // Orig stays an intermediate shape; the mesh is skinned.
    let shape = s.graph.find("barShape").unwrap().unwrap();
    assert_eq!(skin::skin_of(&s.graph, shape), Some(sk));
    let orig = s.graph.find("barShapeOrig").unwrap().unwrap();
    assert!(s.graph.node(orig).unwrap().bool(aztek_scene::transform::DagNode::INTERMEDIATE_OBJECT));

    // Moving joint2 moves point 1 fully and point 2 by its weight.
    run(&mut s, "setAttr", json!({"args": ["joint2.ty", 1.0]}));
    let m = mesh_of(&mut s, "bar");
    assert!(near(&m.position(1).to_array(), &[2.0, 1.0, 0.0]));
    assert!(near(&m.position(2).to_array(), &[2.0, 1.75, 0.0]));
    assert!(near(&m.position(0).to_array(), &[0.0, 0.0, 0.0]));

    // The bind pose came through as a node.
    assert!(s.graph.find("bindPose1").unwrap().is_some());
}

#[test]
fn compact_weight_lists_load() {
    let text = SKINNED
        .replace("\tsetAttr \".wl[0].w[0]\"  1;\n", "")
        .replace("\tsetAttr \".wl[1].w[1]\"  1;\n", "")
        .replace("\tsetAttr -s 2 \".wl[2].w[0:1]\"  0.25 0.75;\n", "")
        .replace("\tsetAttr \".wl[3].w[0]\"  1;\n", "\tsetAttr \".wl[0:3].w\"\n\t\t1 0 1\n\t\t1 1 1\n\t\t2 0 0.25 1 0.75\n\t\t1 0 1;\n");
    let mut s = Session::new();
    read_text(&mut s, &text, false);
    assert_eq!(weights(&s, "skinCluster1")[2], vec![(0, 0.25), (1, 0.75)]);
    assert_eq!(weights(&s, "skinCluster1")[1], vec![(1, 1.0)]);
}

#[test]
fn constraints_load_from_their_target_compounds() {
    let text = r#"
createNode transform -n "target1";
	setAttr ".t" -type "double3" 1 0 0 ;
createNode transform -n "target2";
	setAttr ".t" -type "double3" 3 0 0 ;
createNode transform -n "follower";
createNode pointConstraint -n "follower_pointConstraint1" -p "follower";
	addAttr -dcb 0 -ci true -k true -sn "w0" -ln "target1W0" -dv 1 -min 0 -at "double";
	addAttr -dcb 0 -ci true -k true -sn "w1" -ln "target2W1" -dv 1 -min 0 -at "double";
	setAttr -k on ".w1" 3;
	setAttr -s 2 ".tg";
connectAttr "target1.t" "follower_pointConstraint1.tg[0].tt";
connectAttr "target1.rp" "follower_pointConstraint1.tg[0].trp";
connectAttr "target1.pm" "follower_pointConstraint1.tg[0].tpm";
connectAttr "follower_pointConstraint1.w0" "follower_pointConstraint1.tg[0].tw";
connectAttr "target2.t" "follower_pointConstraint1.tg[1].tt";
connectAttr "target2.pm" "follower_pointConstraint1.tg[1].tpm";
connectAttr "follower_pointConstraint1.w1" "follower_pointConstraint1.tg[1].tw";
connectAttr "follower.pim" "follower_pointConstraint1.cpim";
connectAttr "follower_pointConstraint1.ctx" "follower.tx";
connectAttr "follower_pointConstraint1.cty" "follower.ty";
connectAttr "follower_pointConstraint1.ctz" "follower.tz";
"#;
    let mut s = Session::new();
    let w = read_text(&mut s, text, false);
    assert!(w.is_empty(), "{w:?}");
    s.graph.evaluate();
    // Weighted 1:3 between x=1 and x=3.
    assert!(near(&get(&mut s, "follower.t"), &[2.5, 0.0, 0.0]), "{:?}", get(&mut s, "follower.t"));

    let (out, _) = write_text(&mut s, "c.ma", None);
    assert!(out.contains(r#"createNode pointConstraint -n "follower_pointConstraint1" -p "follower";"#), "{out}");
    assert!(out.contains(r#"connectAttr "target2.pm" "follower|follower_pointConstraint1.tg[1].tpm";"#), "{out}");
    let mut b = Session::new();
    let w = read_text(&mut b, &out, false);
    assert!(w.is_empty(), "{w:?}");
    assert!(near(&get(&mut b, "follower.t"), &[2.5, 0.0, 0.0]));
}

#[test]
fn an_ik_handle_loads_and_solves() {
    let text = r#"
createNode joint -n "joint1";
createNode joint -n "joint2" -p "joint1";
	setAttr ".t" -type "double3" 2 0 0.25 ;
createNode joint -n "joint3" -p "joint2";
	setAttr ".t" -type "double3" 2 0 -0.25 ;
createNode ikEffector -n "effector1" -p "joint2";
	setAttr ".v" no;
	setAttr ".t" -type "double3" 2 0 -0.25 ;
	setAttr ".hd" yes;
createNode ikHandle -n "ikHandle1";
	setAttr ".t" -type "double3" 3 1 0 ;
	setAttr ".roc" yes;
createNode ikRPsolver -n "ikRPsolver";
connectAttr "joint1.msg" "ikHandle1.hsj";
connectAttr "effector1.hp" "ikHandle1.hee";
connectAttr "ikRPsolver.msg" "ikHandle1.hsv";
connectAttr "joint3.tx" "effector1.tx";
connectAttr "joint3.ty" "effector1.ty";
connectAttr "joint3.tz" "effector1.tz";
"#;
    let mut s = Session::new();
    let w = read_text(&mut s, text, false);
    assert!(w.is_empty(), "{w:?}");
    s.graph.evaluate();
    assert!(near(&get(&mut s, "ikHandle1.t"), &[3.0, 1.0, 0.0]));
    let p = run(&mut s, "joint", json!({"args": ["joint3"], "query": true}));
    let p: Vec<f64> = p.as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect();
    assert!(near(&p, &[3.0, 1.0, 0.0]), "{p:?}");

    // Written back as a handle, an effector and a solver; ikSolve doesn't leak.
    let (out, _) = write_text(&mut s, "ik.ma", None);
    assert!(out.contains(r#"createNode ikHandle -n "ikHandle1";"#), "{out}");
    assert!(out.contains(r#"createNode ikEffector -n "effector1" -p "joint2";"#), "{out}");
    assert!(out.contains(r#"createNode ikRPsolver -n "ikRPsolver";"#), "{out}");
    assert!(out.contains(r#"connectAttr "joint2|effector1.hp" "ikHandle1.hee";"#), "{out}");
    assert!(!out.contains("ikSolve"), "{out}");
    let mut b = Session::new();
    let w = read_text(&mut b, &out, false);
    assert!(w.is_empty(), "{w:?}");
    b.graph.evaluate();
    let p = run(&mut b, "joint", json!({"args": ["joint3"], "query": true}));
    let p: Vec<f64> = p.as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect();
    assert!(near(&p, &[3.0, 1.0, 0.0]), "{p:?}");
}

#[test]
fn unknown_things_warn_and_do_not_fail() {
    let text = r#"//Maya ASCII 2024 scene
file -rdi 1 -ns "rig" -rfn "rigRN" -op "v=0;" -typ "mayaAscii" "/x/rig.ma";
file -r -ns "rig" -dr 1 -rfn "rigRN" -op "v=0;" -typ "mayaAscii" "/x/rig.ma";
global proc foo() { print "hello"; }
someCommand -flag 1 "x";
createNode transform -n "group1";
createNode nurbsSurface -n "surfaceShape1" -p "group1";
createNode blendShape -n "blendShape1";
createNode blendShape -n "blendShape2";
createNode transform -n "kid" -p "group1";
createNode camera -n "cameraShape1" -p "kid";
createNode transform -n "keep" -p "group1";
setAttr ".doesNotExist" 1;
connectAttr "nobody.x" "keep.tx";
"#;
    let mut s = Session::new();
    let w = read_text(&mut s, text, false);
    assert!(w.iter().any(|x| x.contains("not loaded") && x.contains("rig.ma")), "{w:?}");
    assert!(w.iter().any(|x| x.contains("surfaceShape1: no stored surface")), "{w:?}");
    assert!(w.iter().any(|x| x.contains("blendShape not represented: blendShape1, blendShape2")), "{w:?}");
    assert!(s.graph.find("group1|keep").unwrap().is_some());
    // A camera of the scene's own comes through.
    assert_eq!(s.run("ls", json!({"type": "camera"})).unwrap(), json!(["cameraShape1"]));
}

#[test]
fn units_convert() {
    let text = r#"
currentUnit -l meter -a radian -t film;
createNode transform -n "a";
	setAttr ".t" -type "double3" 1 2 3 ;
	setAttr ".r" -type "double3" 3.141592653589793 0 0 ;
createNode animCurveTL -n "a_tz";
	setAttr -s 1 ".ktv[0]"  1 5;
connectAttr "a_tz.o" "a.tz";
"#;
    let mut s = Session::new();
    read_text(&mut s, text, false);
    s.graph.evaluate();
    assert!(near(&get(&mut s, "a.t"), &[100.0, 200.0, 500.0]));
    assert!(near(&get(&mut s, "a.r"), &[180.0, 0.0, 0.0]));
}

// ---------------------------------------------------------------------------------------------------------------------
// Writing

fn scene(s: &mut Session) {
    run(s, "polyCube", json!({"name": "box", "width": 2.0, "subdivisionsX": 2}));
    run(s, "setAttr", json!({"args": ["box.t", 1.0, 2.0, 3.0]}));
    run(s, "setAttr", json!({"args": ["box.r", 10.0, 0.0, 0.0]}));
    run(s, "polySphere", json!({"name": "ball", "subdivisionsAxis": 6, "subdivisionsHeight": 4}));
    run(s, "group", json!({"args": ["ball"], "name": "grp"}));
    run(s, "setAttr", json!({"args": ["grp.ty", 4.0]}));
    run(s, "setKeyframe", json!({"args": ["box.tx"], "time": 1, "value": 0.0}));
    run(s, "setKeyframe", json!({"args": ["box.tx"], "time": 10, "value": 5.0, "inTangentType": "linear", "outTangentType": "step"}));
    run(s, "setAttr", json!({"args": ["box_translateX.postInfinity", 3]}));
}

#[test]
fn the_writer_writes_what_the_reference_expects() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"name": "box"}));
    run(&mut s, "setAttr", json!({"args": ["box.t", 1.0, 2.0, 3.0]}));
    run(&mut s, "setKeyframe", json!({"args": ["box.tx"], "time": 1, "value": 0.0}));
    run(&mut s, "setKeyframe", json!({"args": ["box.tx"], "time": 12, "value": 3.0}));
    let (t, _) = write_text(&mut s, "/tmp/foo.ma", None);
    for want in [
        "//Maya ASCII 2024 scene\n",
        "//Name: foo.ma\n",
        "requires maya \"2024\";\n",
        "currentUnit -l centimeter -a degree -t film;\n",
        "createNode transform -n \"box\";\n",
        "setAttr \".t\" -type \"double3\" ",
        "createNode mesh -n \"boxShape\" -p \"box\";\n",
        "setAttr -k off \".v\";\n",
        "setAttr -s 8 \".vt\";\n",
        "setAttr -s 12 \".ed\";\n",
        "setAttr -s 6 -ch 24 \".fc[0:5]\" -type \"polyFaces\"",
        "mu 0 4 ",
        "setAttr \".uvst[0].uvsn\" -type \"string\" \"map1\";\n",
        "\".uvst[0].uvsp[0:",
        "setAttr \".cuvs\" -type \"string\" \"map1\";\n",
        "createNode animCurveTL -n \"box_translateX\";\n",
        "setAttr -s 2 \".ktv[0:1]\"  1 0 12 3;\n",
        "connectAttr \"box_translateX.o\" \"box.tx\";\n",
        "connectAttr \"boxShape.iog\" \":initialShadingGroup.dsm\" -na;\n",
        "select -ne :time1;\n",
        "playbackOptions -min ",
        "// End of foo.ma\n",
    ] {
        assert!(t.contains(want), "missing {want}\n{t}");
    }
    assert!(!t.contains("polyCube"), "history is baked:\n{t}");
    assert!(t.find("createNode transform -n \"box\"").unwrap() < t.find("createNode mesh").unwrap());
    // Same scene, same text.
    let (again, _) = write_text(&mut s, "/tmp/foo.ma", None);
    assert_eq!(t, again);
}

#[test]
fn a_scene_round_trips() {
    let mut a = Session::new();
    scene(&mut a);
    let (text, w) = write_text(&mut a, "x.ma", None);
    assert!(w.is_empty(), "{w:?}");
    let mut b = Session::new();
    let w = read_text(&mut b, &text, false);
    assert!(w.is_empty(), "{w:?}\n{text}");

    // Hierarchy and channels.
    assert!(b.graph.find("grp|ball").unwrap().is_some());
    assert!(near(&get(&mut b, "box.r"), &[10.0, 0.0, 0.0]));
    assert!((get(&mut b, "grp.ty")[0] - 4.0).abs() < 1e-12);

    // Geometry: points in order, faces in order, UVs by corner.
    for name in ["box", "ball"] {
        let (x, y) = (mesh_of(&mut a, name), mesh_of(&mut b, name));
        assert_eq!(x.num_points(), y.num_points(), "{name}");
        assert_eq!(x.num_faces(), y.num_faces(), "{name}");
        assert_eq!(x.num_edges(), y.num_edges(), "{name}");
        for p in 0..x.num_points() {
            assert!((x.position(p) - y.position(p)).length() < 1e-9, "{name} point {p}");
        }
        let (ua, ub) = (x.corner_attrs().get::<[f32; 2]>("uv0"), y.corner_attrs().get::<[f32; 2]>("uv0"));
        for f in 0..x.num_faces() {
            assert_eq!(x.face_points(f), y.face_points(f), "{name} face {f}");
            for (c, d) in x.face_corners(f).into_iter().zip(y.face_corners(f)) {
                assert_eq!(ua[c as usize], ub[d as usize], "{name} face {f}");
            }
        }
        // The same UV maps, hard edges and all.
        assert_eq!(aztek_scene::uv_ops::topo(&x).maps.len(), aztek_scene::uv_ops::topo(&y).maps.len(), "{name}");
        for e in 0..x.num_edges() {
            let eb = y.find_edge(x.edge_point(e, 0), x.edge_point(e, 1));
            let h = |m: &Mesh, e| m.edge_attrs().get::<bool>(names::HARD).get(e as usize).copied().unwrap_or(false);
            assert_eq!(h(&x, e), h(&y, eb), "{name} edge {e}");
        }
    }

    // Keys and tangents.
    let id = |s: &Session| s.graph.find("box_translateX").unwrap().unwrap();
    let (ka, kb) = (anim::keys_of(&a.graph, id(&a)), anim::keys_of(&b.graph, id(&b)));
    assert_eq!(ka.keys.len(), 2);
    for (x, y) in ka.keys.iter().zip(&kb.keys) {
        assert_eq!((x.t, x.v, x.itt, x.ott), (y.t, y.v, y.itt, y.ott));
    }
    assert_eq!(get(&mut b, "box_translateX.postInfinity")[0], 3.0);

    // Writing what was read gives the same text.
    let (again, _) = write_text(&mut b, "x.ma", None);
    for (x, y) in text.lines().zip(again.lines()) {
        assert_eq!(x, y);
    }
    assert_eq!(text.lines().count(), again.lines().count());
}

#[test]
fn fixed_tangents_round_trip() {
    let mut a = Session::new();
    run(&mut a, "createNode", json!({"args": ["transform"], "name": "t"}));
    run(&mut a, "setKeyframe", json!({"args": ["t.ry"], "time": 1, "value": 0.0}));
    run(&mut a, "setKeyframe", json!({"args": ["t.ry"], "time": 24, "value": 90.0}));
    let c = a.graph.find("t_rotateY").unwrap().unwrap();
    let mut k = anim::keys_of(&a.graph, c);
    k.keys[0].itt = anim::Tangent::Fixed;
    k.keys[0].ott = anim::Tangent::Fixed;
    k.keys[0].in_slope = 2.0;
    k.keys[0].out_slope = -3.0;
    anim::set_keys(&mut a.graph, c, k).unwrap();
    let (text, _) = write_text(&mut a, "x.ma", None);
    let mut b = Session::new();
    read_text(&mut b, &text, false);
    let k = anim::keys_of(&b.graph, b.graph.find("t_rotateY").unwrap().unwrap());
    assert!((k.keys[0].in_slope - 2.0).abs() < 1e-9, "{k:?}");
    assert!((k.keys[0].out_slope - -3.0).abs() < 1e-9);
}

#[test]
fn a_skin_round_trips_with_its_mesh_and_skeleton() {
    let mut a = Session::new();
    run(&mut a, "polyCube", json!({"name": "bar", "width": 4.0, "subdivisionsX": 4}));
    run(&mut a, "select", json!({"clear": true}));
    run(&mut a, "joint", json!({"position": [-2, 0, 0], "name": "root"}));
    run(&mut a, "joint", json!({"position": [0, 0, 0], "name": "mid"}));
    run(&mut a, "joint", json!({"position": [2, 0, 0], "name": "tip"}));
    run(&mut a, "skinCluster", json!({"args": ["root", "bar"], "name": "skin1"}));
    run(&mut a, "setAttr", json!({"args": ["mid.rz", 30.0]}));
    let (text, w) = write_text(&mut a, "skin.ma", None);
    assert!(w.is_empty(), "{w:?}");
    assert!(text.contains("createNode skinCluster -n \"skin1\";"), "{text}");
    assert!(text.contains("\".pm[1]\" -type \"matrix\""), "{text}");
    assert!(text.contains("connectAttr \"mid.wm\" \"skin1.ma[1]\";"), "{text}");
    assert!(text.contains("connectAttr \"barShapeOrig.w\" \"skin1.ip[0].ig\";"), "{text}");
    assert!(text.contains("connectAttr \"skin1.og[0]\" \"barShape.i\";"), "{text}");
    assert!(text.contains("\tsetAttr \".io\" yes;"), "{text}");
    assert!(text.contains("createNode dagPose -n \"bindPose1\";"), "{text}");
    assert!(text.contains("connectAttr \"bindPose1.msg\" \"skin1.bp\";"), "{text}");
    assert!(text.contains("connectAttr \"mid.s\" \"tip.is\";"), "{text}");
    // Only the deformed shape goes to the shading group.
    assert!(!text.contains("barShapeOrig.iog"), "{text}");

    let mut b = Session::new();
    let w = read_text(&mut b, &text, false);
    assert!(w.is_empty(), "{w:?}\n{text}");
    let (wa, wb) = (weights(&a, "skin1"), weights(&b, "skin1"));
    assert_eq!(wa.len(), wb.len());
    for (x, y) in wa.iter().zip(&wb) {
        assert_eq!(x.len(), y.len());
        for (p, q) in x.iter().zip(y) {
            assert_eq!(p.0, q.0);
            assert!((p.1 - q.1).abs() < 1e-12);
        }
    }
    // The posed result matches: the bind matrices, weights and joint rotation all came through.
    let (ma, mb) = (mesh_of(&mut a, "bar"), mesh_of(&mut b, "bar"));
    for p in 0..ma.num_points() {
        assert!((ma.position(p) - mb.position(p)).length() < 1e-9, "point {p}");
    }
    // The bind pose restores on the copy.
    run(&mut b, "dagPose", json!({"args": ["root"], "restore": true, "bindPose": true}));
    assert!(near(&get(&mut b, "mid.r"), &[0.0, 0.0, 0.0]), "{:?}", get(&mut b, "mid.r"));
    let (again, _) = write_text(&mut a, "skin.ma", None);
    assert_eq!(text, again);
}

#[test]
fn opening_a_rigged_file_works_through_the_command() {
    // Batched, with undo off: what `file -open` does, as against reading into a live scene.
    let mut a = Session::new();
    run(&mut a, "polyCube", json!({"name": "bar", "width": 4.0, "subdivisionsX": 4}));
    run(&mut a, "select", json!({"clear": true}));
    run(&mut a, "joint", json!({"position": [-2, 0, 0], "name": "root"}));
    run(&mut a, "joint", json!({"position": [0, 0, 0.5], "name": "mid"}));
    run(&mut a, "joint", json!({"position": [2, 0, 0], "name": "tip"}));
    run(&mut a, "skinCluster", json!({"args": ["root", "bar"], "name": "skin1"}));
    run(&mut a, "ikHandle", json!({"startJoint": "root", "endEffector": "tip", "name": "hand"}));
    run(&mut a, "setAttr", json!({"args": ["hand.t", 1.0, 1.0, 0.0]}));
    run(&mut a, "createNode", json!({"args": ["transform"], "name": "ctrl"}));
    run(&mut a, "setAttr", json!({"args": ["ctrl.ty", 2.0]}));
    run(&mut a, "createNode", json!({"args": ["transform"], "name": "follower"}));
    run(&mut a, "parentConstraint", json!({"args": ["ctrl", "follower"], "name": "pc", "maintainOffset": true}));
    let path = temp("rig.ma");
    run(&mut a, "file", json!({"args": [path], "exportAll": true, "type": "mayaAscii"}));
    let mut b = Session::new();
    run(&mut b, "file", json!({"args": [path], "open": true, "force": true}));
    assert!(!b.graph.undo_stack().can_undo());
    let (ma, mb) = (mesh_of(&mut a, "bar"), mesh_of(&mut b, "bar"));
    for p in 0..ma.num_points() {
        assert!((ma.position(p) - mb.position(p)).length() < 1e-6, "point {p}");
    }
    assert!(b.graph.find("hand").unwrap().is_some());
    // The constraint follows on the copy.
    run(&mut b, "setAttr", json!({"args": ["ctrl.ty", 5.0]}));
    b.graph.evaluate();
    assert!((get(&mut b, "follower.ty")[0] - 3.0).abs() < 1e-9, "{:?}", get(&mut b, "follower.ty"));
}

#[test]
fn weight_locks_round_trip() {
    let mut a = Session::new();
    let w = read_text(&mut a, SKINNED, false);
    assert!(w.is_empty(), "{w:?}");
    run(&mut a, "skinCluster", json!({"args": ["skinCluster1"], "edit": true, "influence": "joint2", "lockWeights": true}));
    let (t, _) = write_text(&mut a, "x.ma", None);
    assert!(t.contains("setAttr \"joint2.liw\" yes;"), "{t}");
    let mut b = Session::new();
    read_text(&mut b, &t, false);
    let id = b.graph.find("skinCluster1").unwrap().unwrap();
    assert_eq!(skin::bind_of(&b.graph, id).unwrap().locks, vec![false, true]);
}

#[test]
fn locks_user_attrs_and_layers_round_trip() {
    let mut a = Session::new();
    run(&mut a, "createNode", json!({"args": ["transform"], "name": "t"}));
    run(&mut a, "addAttr", json!({"args": ["t"], "longName": "wiggle", "attributeType": "double", "keyable": true, "defaultValue": 2.0}));
    run(&mut a, "setAttr", json!({"args": ["t.wiggle", 7.5]}));
    run(&mut a, "setAttr", json!({"args": ["t.tx"], "lock": true}));
    run(&mut a, "createDisplayLayer", json!({"name": "layerA", "empty": true}));
    run(&mut a, "editDisplayLayerMembers", json!({"args": ["layerA", "t"]}));
    let (text, w) = write_text(&mut a, "x.ma", None);
    assert!(w.is_empty(), "{w:?}");
    assert!(text.contains("addAttr -ci true -k true -sn \"wiggle\" -ln \"wiggle\" -dv 2 -at \"double\";"), "{text}");
    assert!(text.contains("setAttr -l on \".tx\";"), "{text}");
    assert!(text.contains("connectAttr \"layerA.di\" \"t.do\";"), "{text}");
    let mut b = Session::new();
    let w = read_text(&mut b, &text, false);
    assert!(w.is_empty(), "{w:?}");
    assert_eq!(get(&mut b, "t.wiggle")[0], 7.5);
    let tx = b.graph.plug("t.tx").unwrap();
    assert!(b.graph.is_locked(&tx));
    let id = b.graph.find("t").unwrap().unwrap();
    let layer = aztek_scene::display_layers::layer_of(&b.graph, id).unwrap();
    assert_eq!(b.graph.node(layer).unwrap().name(), "layerA");
}

// A scene's deformed points, by evaluating its mesh.
fn points(s: &mut Session, name: &str) -> Vec<glam::DVec3> {
    s.graph.evaluate();
    let id = s.graph.find(name).unwrap().unwrap();
    let shape = s.graph.children(id).iter().copied().find(|&c| s.graph.node(c).is_some_and(|n| n.ty().is_a("mesh") && !n.bool(aztek_scene::transform::DagNode::INTERMEDIATE_OBJECT))).unwrap();
    let out = PlugRef::new(shape, aztek_scene::mesh_nodes::MeshNode::OUT_MESH as i32);
    s.graph.value(&out).unwrap().as_mesh().unwrap().positions().to_vec()
}

fn assert_close(a: &[glam::DVec3], b: &[glam::DVec3], what: &str) {
    assert_eq!(a.len(), b.len(), "{what}");
    for (i, (x, y)) in a.iter().zip(b).enumerate() {
        assert!((*x - *y).length() < 1e-6, "{what}: point {i}: {x} and {y}");
    }
}

fn round_trip(s: &mut Session) -> (Session, String) {
    let (text, w) = write_text(s, "x.ma", None);
    assert!(!w.iter().any(|x| x.contains("not written")), "{w:?}");
    let mut b = Session::new();
    let w = read_text(&mut b, &text, false);
    assert!(!w.iter().any(|x| x.contains("not represented")), "{w:?}");
    (b, text)
}

#[test]
fn a_nonlinear_deformer_round_trips_as_the_reference_lays_it_out() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"name": "box", "subdivisionsY": 4}));
    run(&mut s, "nonLinear", json!({"args": ["box"], "type": "bend", "curvature": 1.2, "name": "bendy"}));
    let want = points(&mut s, "box");
    let (mut b, text) = round_trip(&mut s);
    assert!(text.contains("createNode nonLinear -n \"bendy\""), "{text}");
    assert!(text.contains("createNode deformBend -n \"bendyHandleShape\" -p \"bendyHandle\""), "{text}");
    assert!(text.contains("setAttr \".cur\" 1.2"), "{text}");
    assert!(text.contains("connectAttr \"bendyHandleShape.dd\" \"bendy.dd\""), "{text}");
    assert!(text.contains("connectAttr \"bendyHandle.worldMatrix[0]\" \"bendy.ma\""), "{text}");
    assert_close(&points(&mut b, "box"), &want, "bend");
    // The handle still drives it.
    run(&mut b, "setAttr", json!({"args": ["bendyHandle.rotateX", 40.0]}));
    run(&mut s, "setAttr", json!({"args": ["bendyHandle.rotateX", 40.0]}));
    assert_close(&points(&mut b, "box"), &points(&mut s, "box"), "bend, handle turned");
}

#[test]
fn a_cluster_keeps_its_points_weights_and_handle() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"name": "box", "subdivisionsY": 2}));
    run(&mut s, "cluster", json!({"args": ["box.vtx[0:3]"], "name": "cl"}));
    run(&mut s, "setAttr", json!({"args": ["clHandle.translateY", 2.0]}));
    run(&mut s, "percent", json!({"args": ["cl", "box.vtx[1]"], "value": 0.25}));
    let want = points(&mut s, "box");
    let (mut b, text) = round_trip(&mut s);
    assert!(text.contains("createNode clusterHandle"), "{text}");
    assert!(text.contains("createNode groupParts"), "{text}");
    assert!(text.contains("\"vtx[0:3]\""), "{text}");
    assert!(text.contains("createNode objectSet -n \"clSet\""), "{text}");
    assert_close(&points(&mut b, "box"), &want, "cluster");
    run(&mut b, "setAttr", json!({"args": ["clHandle.translateX", 1.0]}));
    run(&mut s, "setAttr", json!({"args": ["clHandle.translateX", 1.0]}));
    assert_close(&points(&mut b, "box"), &points(&mut s, "box"), "cluster, handle moved");
    // Its set came back with the same points.
    let set = run(&mut b, "sets", json!({"args": ["clSet"], "query": true}));
    assert_eq!(set, run(&mut s, "sets", json!({"args": ["clSet"], "query": true})));
}

#[test]
fn a_blend_shape_round_trips_with_its_aliases_and_weights() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"name": "box"}));
    run(&mut s, "polyCube", json!({"name": "smile"}));
    run(&mut s, "move", json!({"args": ["smile.vtx[2]", 0.0, 1.0, 0.0], "relative": true}));
    run(&mut s, "polyCube", json!({"name": "frown"}));
    run(&mut s, "move", json!({"args": ["frown.vtx[4]", 0.0, -1.0, 0.0], "relative": true}));
    run(&mut s, "blendShape", json!({"args": ["smile", "frown", "box"], "name": "faces"}));
    run(&mut s, "setAttr", json!({"args": ["faces.smile", 0.5]}));
    run(&mut s, "delete", json!({"args": ["smile", "frown"]}));
    let want = points(&mut s, "box");
    let (mut b, text) = round_trip(&mut s);
    assert!(text.contains("-type \"attributeAlias\" {\"smile\",\"weight[0]\",\"frown\",\"weight[1]\"}"), "{text}");
    assert!(text.contains(".it[0].itg[0].iti[6000].ipt\" -type \"pointArray\""), "{text}");
    assert_close(&points(&mut b, "box"), &want, "blend");
    run(&mut b, "setAttr", json!({"args": ["faces.frown", 1.0]}));
    run(&mut s, "setAttr", json!({"args": ["faces.frown", 1.0]}));
    assert_close(&points(&mut b, "box"), &points(&mut s, "box"), "blend, other target on");
    assert_eq!(run(&mut b, "blendShape", json!({"args": ["faces"], "query": true, "queried": ["target"]})), run(&mut s, "blendShape", json!({"args": ["faces"], "query": true, "queried": ["target"]})));
}

#[test]
fn a_live_blend_target_stays_connected() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"name": "box"}));
    run(&mut s, "polyCube", json!({"name": "smile"}));
    run(&mut s, "blendShape", json!({"args": ["smile", "box"], "name": "faces"}));
    run(&mut s, "setAttr", json!({"args": ["faces.smile", 1.0]}));
    let (mut b, text) = round_trip(&mut s);
    assert!(text.contains("smileShape.outMesh\" \"faces.it[0].itg[0].iti[6000].igt\""), "{text}");
    run(&mut b, "move", json!({"args": ["smile.vtx[2]", 0.0, 1.0, 0.0], "relative": true}));
    run(&mut s, "move", json!({"args": ["smile.vtx[2]", 0.0, 1.0, 0.0], "relative": true}));
    assert_close(&points(&mut b, "box"), &points(&mut s, "box"), "live target moved");
}

#[test]
fn keys_on_a_blend_weight_and_a_bend_survive() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"name": "box", "subdivisionsY": 4}));
    run(&mut s, "polyCube", json!({"name": "smile", "subdivisionsY": 4}));
    run(&mut s, "move", json!({"args": ["smile.vtx[2]", 0.0, 1.0, 0.0], "relative": true}));
    run(&mut s, "blendShape", json!({"args": ["smile", "box"], "name": "faces"}));
    run(&mut s, "delete", json!({"args": ["smile"]}));
    run(&mut s, "nonLinear", json!({"args": ["box"], "type": "bend", "name": "bendy"}));
    run(&mut s, "setKeyframe", json!({"args": ["faces.smile"], "time": 1.0, "value": 0.0}));
    run(&mut s, "setKeyframe", json!({"args": ["faces.smile"], "time": 10.0, "value": 1.0}));
    run(&mut s, "setKeyframe", json!({"args": ["bendy.curvature"], "time": 1.0, "value": 0.0}));
    run(&mut s, "setKeyframe", json!({"args": ["bendy.curvature"], "time": 10.0, "value": 1.5}));
    run(&mut s, "currentTime", json!({"args": [10.0]}));
    let want = points(&mut s, "box");
    let (mut b, _) = round_trip(&mut s);
    run(&mut b, "currentTime", json!({"args": [10.0]}));
    assert_close(&points(&mut b, "box"), &want, "keyed");
    run(&mut b, "currentTime", json!({"args": [1.0]}));
    run(&mut s, "currentTime", json!({"args": [1.0]}));
    assert_close(&points(&mut b, "box"), &points(&mut s, "box"), "keyed, frame 1");
}

#[test]
fn a_blend_shape_under_a_skin_keeps_the_order() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"name": "box", "subdivisionsY": 4}));
    run(&mut s, "polyCube", json!({"name": "smile", "subdivisionsY": 4}));
    run(&mut s, "move", json!({"args": ["smile.vtx[2]", 0.0, 1.0, 0.0], "relative": true}));
    run(&mut s, "blendShape", json!({"args": ["smile", "box"], "name": "faces"}));
    run(&mut s, "delete", json!({"args": ["smile"]}));
    run(&mut s, "setAttr", json!({"args": ["faces.smile", 1.0]}));
    run(&mut s, "select", json!({"args": [], "clear": true}));
    run(&mut s, "joint", json!({"name": "a", "position": [0.0, -1.0, 0.0]}));
    run(&mut s, "joint", json!({"name": "c", "position": [0.0, 1.0, 0.0]}));
    run(&mut s, "skinCluster", json!({"args": ["a", "c", "box"], "name": "skin"}));
    run(&mut s, "setAttr", json!({"args": ["c.rotateX", 30.0]}));
    let want = points(&mut s, "box");
    let (mut b, _) = round_trip(&mut s);
    assert_close(&points(&mut b, "box"), &want, "blend then skin");
    let hist = run(&mut b, "listHistory", json!({"args": ["box"]}));
    let names: Vec<String> = hist.as_array().unwrap().iter().filter_map(|v| v.as_str().map(str::to_string)).collect();
    let (x, y) = (names.iter().position(|n| n == "skin").unwrap(), names.iter().position(|n| n == "faces").unwrap());
    assert!(x < y, "{names:?}");
}

#[test]
fn every_other_deformer_round_trips() {
    let setup = |s: &mut Session| {
        run(s, "polyCube", json!({"name": "box", "subdivisionsX": 3, "subdivisionsY": 3, "subdivisionsZ": 3}));
    };
    let cases: Vec<(&str, Box<dyn Fn(&mut Session)>)> = vec![
        ("lattice", Box::new(|s| {
            let r = run(s, "lattice", json!({"args": ["box"], "divisions": [2, 3, 2]}));
            let shape = r.as_array().unwrap()[1].as_str().unwrap().to_string();
            run(s, "move", json!({"args": [format!("{shape}.pt[0][0][0]"), 0.3, 0.0, 0.0], "relative": true}));
        })),
        ("softMod", Box::new(|s| {
            let r = run(s, "softMod", json!({"args": ["box.vtx[0]"], "falloffRadius": 1.5}));
            let h = r.as_array().unwrap()[1].as_str().unwrap().to_string();
            run(s, "setAttr", json!({"args": [format!("{h}.translateY"), 1.0]}));
        })),
        ("deltaMush", Box::new(|s| {
            run(s, "deltaMush", json!({"args": ["box"], "smoothingIterations": 4}));
            run(s, "move", json!({"args": ["box.vtx[0]", 0.2, 0.2, 0.0], "relative": true}));
        })),
        ("wire", Box::new(|s| {
            run(s, "curve", json!({"name": "crv", "point": [[-1.0, 0.5, 0.0], [1.0, 0.5, 0.0]], "degree": 1}));
            run(s, "wire", json!({"args": ["box"], "wire": ["crv"], "name": "wr"}));
            run(s, "setAttr", json!({"args": ["crv.translateY", 0.5]}));
        })),
        ("wrap", Box::new(|s| {
            run(s, "polyCube", json!({"name": "cage", "width": 2.0, "height": 2.0, "depth": 2.0}));
            run(s, "deformer", json!({"args": ["box"], "type": "wrap", "name": "wr"}));
        })),
    ];
    for (what, make) in cases {
        let mut s = Session::new();
        setup(&mut s);
        make(&mut s);
        let want = points(&mut s, "box");
        let (mut b, text) = round_trip(&mut s);
        assert!(text.contains(&format!("createNode {}", if what == "lattice" { "ffd" } else { what })), "{what}: {text}");
        assert_close(&points(&mut b, "box"), &want, what);
    }
}

#[test]
fn a_deformer_on_two_meshes_as_the_reference_files_it() {
    let text = r#"//Maya ASCII 2024 scene
requires maya "2024";
createNode transform -n "a";
createNode mesh -n "aShape" -p "a";
createNode mesh -n "aShapeOrig" -p "a";
	setAttr ".io" yes;
	setAttr -s 3 ".vt";
	setAttr ".vt[0:2]" 0 0 0 1 0 0 0 1 0;
	setAttr -s 3 ".ed";
	setAttr ".ed[0:2]" 0 1 0 1 2 0 2 0 0;
	setAttr ".fc[0]" -type "polyFaces" f 3 0 1 2;
createNode transform -n "b";
createNode mesh -n "bShape" -p "b";
createNode mesh -n "bShapeOrig" -p "b";
	setAttr ".io" yes;
	setAttr -s 3 ".vt";
	setAttr ".vt[0:2]" 0 0 5 1 0 5 0 1 5;
	setAttr -s 3 ".ed";
	setAttr ".ed[0:2]" 0 1 0 1 2 0 2 0 0;
	setAttr ".fc[0]" -type "polyFaces" f 3 0 1 2;
createNode transform -n "clHandle";
	setAttr ".t" -type "double3" 0 2 0 ;
createNode clusterHandle -n "clHandleShape" -p "clHandle";
createNode cluster -n "cl";
	setAttr -s 2 ".wl";
	setAttr ".wl[0].w[0:2]" 1 0.5 0;
	setAttr ".wl[1].w[1]" 0.25;
createNode tweak -n "tw";
	setAttr ".vl[0].vt[2]" -type "float3" 0 0 1;
createNode groupParts -n "gp";
	setAttr ".ic" -type "componentList" 1 "vtx[*]";
connectAttr "clHandle.wm" "cl.ma";
connectAttr "aShapeOrig.w" "tw.ip[0].ig";
connectAttr "tw.og[0]" "gp.ig";
connectAttr "gp.og" "cl.ip[0].ig";
connectAttr "bShapeOrig.w" "cl.ip[1].ig";
connectAttr "cl.og[0]" "aShape.i";
connectAttr "cl.og[1]" "bShape.i";
"#;
    let mut s = Session::new();
    let w = read_text(&mut s, text, false);
    assert!(!w.iter().any(|x| x.contains("cluster") || x.contains("no geometry")), "{w:?}");
    let a = points(&mut s, "a");
    assert_close(&a, &[glam::dvec3(0.0, 2.0, 0.0), glam::dvec3(1.0, 1.0, 0.0), glam::dvec3(0.0, 1.0, 1.0)], "first geometry, tweak under it");
    let b = points(&mut s, "b");
    assert_close(&b, &[glam::dvec3(0.0, 2.0, 5.0), glam::dvec3(1.0, 0.5, 5.0), glam::dvec3(0.0, 3.0, 5.0)], "second geometry");
}

fn temp(name: &str) -> String {
    let dir = std::env::temp_dir().join(format!("aztek_ma_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name).to_str().unwrap().to_string()
}

#[test]
fn the_file_command() {
    let mut s = Session::new();
    assert_eq!(run(&mut s, "file", json!({"query": true, "sceneName": true})), json!(""));
    run(&mut s, "polyCube", json!({"name": "box"}));
    run(&mut s, "polyCube", json!({"name": "other"}));

    let path = temp("scene.ma");
    assert_eq!(run(&mut s, "file", json!({"rename": path})), json!(path));
    assert_eq!(run(&mut s, "file", json!({"query": true, "sceneName": true})), json!(path));
    assert_eq!(run(&mut s, "file", json!({"query": true, "sceneName": true, "shortName": true})), json!("scene.ma"));
    assert_eq!(run(&mut s, "file", json!({"query": true, "exists": true})), json!(false));
    run(&mut s, "file", json!({"save": true, "type": "mayaAscii"}));
    assert_eq!(run(&mut s, "file", json!({"query": true, "exists": true})), json!(true));
    assert_eq!(run(&mut s, "file", json!({"query": true, "type": true})), json!(["mayaAscii"]));
    assert!(std::fs::read_to_string(&path).unwrap().starts_with("//Maya ASCII"));

    // A bare name takes the type's extension.
    let bare = temp("bare");
    run(&mut s, "file", json!({"rename": bare}));
    assert_eq!(run(&mut s, "file", json!({"save": true, "type": "mayaAscii"})), json!(format!("{bare}.ma")));

    assert_eq!(run(&mut s, "file", json!({"new": true, "force": true})), json!("untitled"));
    assert_eq!(s.graph.find("box").unwrap(), None);
    assert_eq!(run(&mut s, "file", json!({"query": true, "sceneName": true})), json!(""));

    run(&mut s, "file", json!({"args": [path], "open": true, "force": true, "options": "v=0;"}));
    assert!(s.graph.find("box").unwrap().is_some());
    assert_eq!(run(&mut s, "file", json!({"query": true, "sceneName": true})), json!(path));

    // Export selected takes the selection, not the rest.
    let sel = temp("sel.ma");
    run(&mut s, "select", json!({"args": ["box"]}));
    run(&mut s, "file", json!({"args": [sel], "exportSelected": true, "type": "mayaAscii"}));
    let t = std::fs::read_to_string(&sel).unwrap();
    assert!(t.contains("\"box\"") && !t.contains("\"other\""), "{t}");

    // Import adds and is one undo step; clashing names are made unique.
    let before = s.graph.nodes().len();
    run(&mut s, "file", json!({"args": [sel], "import": true}));
    assert!(s.graph.nodes().len() > before);
    s.undo();
    assert_eq!(s.graph.nodes().len(), before);

    assert!(s.run("file", json!({"args": ["/nowhere/none.ma"], "open": true, "force": true})).is_err());
    let mb = temp("x.mb");
    std::fs::write(&mb, b"FOR8 data").unwrap();
    let e = s.run("file", json!({"args": [mb], "open": true, "force": true})).unwrap_err().to_string();
    assert!(e.contains("binary"), "{e}");
    assert!(s.run("file", json!({"args": [temp("y.mb")], "save": true, "type": "mayaBinary"})).is_err());
    // A failed open leaves the scene alone.
    assert!(s.graph.find("box").unwrap().is_some());
}

#[test]
fn aztek_files_still_work_through_the_same_routes() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"name": "box"}));
    let path = temp("own.aztek");
    ma_file::save_scene(&mut s, &path, ma_file::Format::of_path(&path)).unwrap();
    assert!(std::fs::read_to_string(&path).unwrap().contains("\"aztek\""));
    ma_file::open_scene(&mut s, &path).unwrap();
    assert!(s.graph.find("box").unwrap().is_some());
    assert_eq!(s.scene_path.as_deref(), Some(path.as_str()));
}

#[test]
fn statements_span_lines_and_strings_join() {
    let text = "createNode\n transform\n -n\n \"a\"\n;\nsetAttr \".t\"\n -type \"double3\"\n 1\n 2\n 3 ;\n/* a block\ncomment */ createNode transform -n \"b\"; // trailing\nrename \"b\" \"c\";";
    let mut s = Session::new();
    read_text(&mut s, text, false);
    assert!(near(&get(&mut s, "a.t"), &[1.0, 2.0, 3.0]));
    assert!(s.graph.find("c").unwrap().is_some());
}

#[test]
fn big_meshes_chunk_like_the_reference_and_read_back() {
    let mut a = Session::new();
    run(&mut a, "polyPlane", json!({"name": "grid", "subdivisionsX": 40, "subdivisionsZ": 40}));
    let (text, _) = write_text(&mut a, "x.ma", None);
    assert!(text.contains("\".vt[0:165]\""), "chunks");
    assert!(text.contains("\".vt[166:331]\""), "chunks");
    assert!(text.contains("\".fc[250:499]\" -type \"polyFaces\""), "chunks");
    let mut b = Session::new();
    read_text(&mut b, &text, false);
    let (x, y) = (mesh_of(&mut a, "grid"), mesh_of(&mut b, "grid"));
    assert_eq!(x.num_faces(), y.num_faces());
    for f in 0..x.num_faces() {
        assert_eq!(x.face_points(f), y.face_points(f));
    }
}

#[test]
fn curves_and_cameras_round_trip() {
    let mut s = Session::new();
    s.run("circle", json!({"normal": [0, 1, 0], "radius": 2.0, "name": "ctrl"})).unwrap();
    s.run("curve", json!({"degree": 3, "point": [[0, 0, 0], [1, 1, 0], [2, 0, 0], [3, 1, 0], [4, 0, 0]], "name": "spline"})).unwrap();
    s.run("camera", json!({"focalLength": 50.0, "name": "shot"})).unwrap();
    aztek_scene::cameras::make_defaults(&mut s.graph);
    let (text, _) = aztek_scene::ma_file::write_text(&mut s, "t.ma", None);
    assert!(text.contains("createNode nurbsCurve -n \"splineShape\" -p \"spline\";"), "{text}");
    assert!(text.contains("-type \"nurbsCurve\""));
    assert!(text.contains("createNode camera -n \"shotShape\" -p \"shot\";"), "{text}");
    assert!(text.contains("createNode transform -s -n \"persp\";"), "{text}");
    let dir = std::env::temp_dir().join(format!("aztek_curve_{}.ma", std::process::id()));
    std::fs::write(&dir, &text).unwrap();
    let mut t = Session::new();
    aztek_scene::ma_file::open_scene(&mut t, dir.to_str().unwrap()).unwrap();
    std::fs::remove_file(&dir).ok();
    let len = |s: &mut Session, n: &str| s.run("arclen", json!({"args": [n]})).unwrap().as_f64().unwrap();
    assert!((len(&mut t, "ctrl") - std::f64::consts::TAU * 2.0).abs() < 0.2);
    assert!((len(&mut t, "spline") - len(&mut s, "spline")).abs() < 1e-6);
    assert_eq!(t.run("getAttr", json!({"args": ["ctrl.form"]})).unwrap().as_f64().unwrap(), 2.0);
    assert_eq!(t.run("getAttr", json!({"args": ["shot.focalLength"]})).unwrap().as_f64().unwrap(), 50.0);
}

#[test]
fn a_circle_from_the_reference_loads_with_its_cv_text() {
    let ma = r#"//Maya ASCII 2024 scene
requires maya "2024";
currentUnit -l centimeter -a degree -t film;
createNode transform -n "nurbsCircle1";
createNode nurbsCurve -n "nurbsCircleShape1" -p "nurbsCircle1";
	setAttr -k off ".v";
	setAttr ".cc" -type "nurbsCurve" 
		3 8 2 no 3
		13 -2 -1 0 1 2 3 4 5 6 7 8 9 10
		11
		0.78361162489122504 4.7982373409884682e-17 -0.78361162489122382
		-1.2643170607829326e-16 6.7857323231109134e-17 -1.1081941875543879
		-0.78361162489122427 4.7982373409884713e-17 -0.78361162489122427
		-1.1081941875543879 3.2112695072372299e-33 -5.2643170607829326e-16
		-0.78361162489122449 -4.7982373409884694e-17 0.78361162489122405
		-3.3392053635905195e-16 -6.7857323231109146e-17 1.1081941875543881
		0.78361162489122382 -4.7982373409884719e-17 0.78361162489122438
		1.1081941875543879 -8.7290431644343137e-33 1.4300133022715681e-15
		0.78361162489122504 4.7982373409884682e-17 -0.78361162489122382
		-1.2643170607829326e-16 6.7857323231109134e-17 -1.1081941875543879
		-0.78361162489122427 4.7982373409884713e-17 -0.78361162489122427
		;
// End of test.ma
"#;
    let dir = std::env::temp_dir().join(format!("aztek_circle_{}.ma", std::process::id()));
    std::fs::write(&dir, ma).unwrap();
    let mut s = Session::new();
    aztek_scene::ma_file::open_scene(&mut s, dir.to_str().unwrap()).unwrap();
    std::fs::remove_file(&dir).ok();
    assert_eq!(s.run("getAttr", json!({"args": ["nurbsCircle1.spans"]})).unwrap().as_f64().unwrap(), 8.0);
    let len = s.run("arclen", json!({"args": ["nurbsCircle1"]})).unwrap().as_f64().unwrap();
    assert!((len - std::f64::consts::TAU).abs() < 0.1, "{len}");
}

#[test]
fn surfaces_round_trip() {
    let mut s = Session::new();
    s.run("sphere", json!({"radius": 2.0, "name": "ball"})).unwrap();
    s.run("nurbsPlane", json!({"width": 4.0, "name": "sheet"})).unwrap();
    let (text, _) = write_text(&mut s, "t.ma", None);
    assert!(text.contains("createNode nurbsSurface -n \"ballShape\" -p \"ball\";"), "{text}");
    assert!(text.contains("-type \"nurbsSurface\""));
    let mut t = Session::new();
    read_text(&mut t, &text, false);
    for attr in ["ball.spansU", "ball.spansV", "ball.degreeU", "ball.formV", "sheet.spansU"] {
        assert_eq!(t.run("getAttr", json!({"args": [attr]})).unwrap(), s.run("getAttr", json!({"args": [attr]})).unwrap(), "{attr}");
    }
    let a = s.run("xform", json!({"args": ["ball.cv[2][3]"], "query": true, "worldSpace": true, "translation": true})).unwrap();
    let b = t.run("xform", json!({"args": ["ball.cv[2][3]"], "query": true, "worldSpace": true, "translation": true})).unwrap();
    for k in 0..3 {
        assert!((a[k].as_f64().unwrap() - b[k].as_f64().unwrap()).abs() < 1e-9, "{a} {b}");
    }
}

/// One attribute of a node driving another on the same node, which rigs do: the walk that marks nodes dirty has to stop
/// when it comes back to where it started.
#[test]
fn a_node_connected_to_itself_does_not_hang() {
    let mut s = Session::new();
    run(&mut s, "createNode", json!({"args": ["transform"], "n": "a"}));
    run(&mut s, "addAttr", json!({"args": ["a"], "ln": "foo", "at": "double"}));
    s.graph.evaluate();
    run(&mut s, "connectAttr", json!({"args": ["a.translateX", "a.foo"]}));
    run(&mut s, "setAttr", json!({"args": ["a.translateX", 3.0]}));
    assert_eq!(get(&mut s, "a.foo"), vec![3.0]);
}
