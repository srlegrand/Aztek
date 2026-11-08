//! Dynamics in .ma files: Aztek's own nodes saved and read back the same, and files in the layouts of the old
//! systems (nucleus with nCloth and nRigid, classic particles with fields and an emitter, rigid bodies, hair) read into
//! the one solver. The old layouts are written by hand from the reference's node documentation; no real scene files are
//! used.

use aztek_scene::ma_file::{read_text, write_text};
use aztek_scene::{PlugRef, Session};
use glam::DVec3;
use serde_json::{json, Value};

fn run(s: &mut Session, name: &str, call: Value) -> Value {
    s.run(name, call).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn frame(s: &mut Session, f: f64) {
    run(s, "currentTime", json!({"args": [f]}));
    s.graph.evaluate();
}

fn points(s: &mut Session, name: &str) -> Vec<DVec3> {
    let id = s.graph.find(name).unwrap().unwrap_or_else(|| panic!("no {name}"));
    let shape = aztek_scene::components::as_mesh(&s.graph, id).unwrap();
    let out = s.graph.node(shape).unwrap().ty().attr_index("outMesh").unwrap();
    let m = s.graph.value(&PlugRef::new(shape, out as i32)).unwrap();
    let mesh = m.as_mesh().unwrap().clone();
    let xf = aztek_scene::dag::transform_of(&s.graph, shape);
    let w = aztek_scene::rig::world(&mut s.graph, xf);
    mesh.positions().iter().map(|p| w.transform_point3(*p)).collect()
}

fn type_of(s: &Session, name: &str) -> String {
    let id = s.graph.find(name).unwrap().unwrap_or_else(|| panic!("no {name}"));
    s.graph.node(id).unwrap().ty().name.clone()
}

fn get(s: &mut Session, plug: &str) -> Value {
    run(s, "getAttr", json!({"args": [plug]}))
}

#[test]
fn own_nodes_save_and_read_back_the_same() {
    let mut s = Session::new();
    run(&mut s, "playbackOptions", json!({"minTime": 1, "maxTime": 40}));
    run(&mut s, "polySphere", json!({"name": "ball", "radius": 0.5}));
    run(&mut s, "polyPlane", json!({"name": "sheet", "width": 2, "height": 2, "sx": 10, "sy": 10}));
    run(&mut s, "move", json!({"args": [0, 1, 0, "sheet"]}));
    run(&mut s, "createCloth", json!({"args": ["sheet"]}));
    run(&mut s, "collider", json!({"args": ["ball"]}));
    run(&mut s, "setAttr", json!({"args": ["cloth1.stretchResistance", 55]}));
    let em = run(&mut s, "emitter", json!({"type": "omni", "rate": 24}));
    run(&mut s, "gravity", json!({"args": [em[1].clone()]}));
    let (text, warnings) = write_text(&mut s, "drape.ma", None);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert!(text.contains("requires \"aztekDynamics\""), "a file with dynamics says so");
    let mut back = Session::new();
    let w = read_text(&mut back, &text, false);
    assert!(w.is_empty(), "{w:?}");
    assert_eq!(type_of(&back, "cloth1"), "cloth");
    assert_eq!(type_of(&back, "collider1"), "collider");
    assert_eq!(type_of(&back, "solverShape1"), "solver");
    assert_eq!(get(&mut back, "cloth1.stretchResistance"), json!(55.0));
    // Written again, the same file.
    let (again, _) = write_text(&mut back, "drape.ma", None);
    assert_eq!(text, again);
    // And it simulates the same.
    for f in 1..=12 {
        frame(&mut s, f as f64);
        frame(&mut back, f as f64);
    }
    let (a, b) = (points(&mut s, "sheet"), points(&mut back, "sheet"));
    let worst = a.iter().zip(&b).map(|(x, y)| (*x - *y).length()).fold(0.0, f64::max);
    if worst >= 1e-9 {
        for at in ["solverShape1", "cloth1", "collider1"] {
            let la = run(&mut s, "listAttr", json!({"args": [at]}));
            for name in la.as_array().unwrap() {
                let p = format!("{at}.{}", name.as_str().unwrap());
                if let (Ok(x), Ok(y)) = (s.run("getAttr", json!({"args": [p.clone()]})), back.run("getAttr", json!({"args": [p.clone()]}))) {
                    if x != y {
                        eprintln!("DIFF {p}: {x} vs {y}");
                    }
                }
            }
        }
    }
    assert!(worst < 1e-9, "the file solves differently by {worst}");
    assert!(a.iter().map(|p| p.y).fold(f64::INFINITY, f64::min) < 0.9, "it moved");
}

const NUCLEUS_FILE: &str = r#"//Maya ASCII 2024 scene
requires maya "2024";
currentUnit -l centimeter -a degree -t film;
createNode transform -n "pPlane1";
	setAttr ".t" -type "double3" 0 1 0 ;
createNode mesh -n "pPlaneShape1" -p "pPlane1";
	setAttr -k off ".v";
	setAttr ".io" yes;
createNode mesh -n "outputCloth1" -p "pPlane1";
	setAttr -k off ".v";
createNode transform -n "pSphere1";
createNode mesh -n "pSphereShape1" -p "pSphere1";
	setAttr -k off ".v";
createNode nucleus -n "nucleus1";
	setAttr ".sstp" 4;
	setAttr ".wnoi" 0.25;
	setAttr ".nupl" yes;
	setAttr ".npor" -type "float3" 0 -2 0 ;
createNode transform -n "nCloth1";
createNode nCloth -n "nClothShape1" -p "nCloth1";
	setAttr -k off ".v";
	setAttr ".thss" 0.04;
	setAttr ".stch" 35;
	setAttr ".lft" 0.2;
	setAttr ".stck" 0.5;
createNode transform -n "nRigid1";
createNode nRigid -n "nRigidShape1" -p "nRigid1";
	setAttr -k off ".v";
	setAttr ".thss" 0.02;
createNode polyPlane -n "polyPlane1";
	setAttr ".w" 2;
	setAttr ".h" 2;
	setAttr ".sw" 8;
	setAttr ".sh" 8;
createNode polySphere -n "polySphere1";
	setAttr ".r" 0.5;
connectAttr "polyPlane1.out" "pPlaneShape1.i";
connectAttr "polySphere1.out" "pSphereShape1.i";
connectAttr "pPlaneShape1.w" "nClothShape1.imsh";
connectAttr "nClothShape1.omsh" "outputCloth1.i";
connectAttr ":time1.o" "nClothShape1.cti";
connectAttr ":time1.o" "nucleus1.cti";
connectAttr "nClothShape1.cust" "nucleus1.niao[0]";
connectAttr "nClothShape1.stst" "nucleus1.nias[0]";
connectAttr "nucleus1.noao[0]" "nClothShape1.nxst";
connectAttr "nucleus1.stf" "nClothShape1.stf";
connectAttr "pSphereShape1.w" "nRigidShape1.imsh";
connectAttr "nRigidShape1.cust" "nucleus1.nipo[0]";
connectAttr "nRigidShape1.stst" "nucleus1.nips[0]";
connectAttr "nucleus1.stf" "nRigidShape1.stf";
"#;

#[test]
fn nucleus_file_reads_as_cloth_and_a_collider() {
    let mut s = Session::new();
    run(&mut s, "playbackOptions", json!({"minTime": 1, "maxTime": 40}));
    let w = read_text(&mut s, NUCLEUS_FILE, false);
    assert_eq!(type_of(&s, "nClothShape1"), "cloth");
    assert_eq!(type_of(&s, "nRigidShape1"), "collider");
    assert_eq!(type_of(&s, "nucleusShape1"), "solver");
    // Settings mapped by name; substeps counted as they mean here.
    assert_eq!(get(&mut s, "nClothShape1.thickness"), json!(0.04));
    assert_eq!(get(&mut s, "nClothShape1.stretchResistance"), json!(35.0));
    assert_eq!(get(&mut s, "nucleusShape1.subSteps"), json!(16));
    assert_eq!(get(&mut s, "nucleusShape1.usePlane"), json!(true));
    // What is not used is kept and named.
    let kept = get(&mut s, "nClothShape1.fileAttributes");
    assert!(kept.as_str().unwrap().contains(".lft") && kept.as_str().unwrap().contains(".stck"), "{kept}");
    assert!(w.iter().any(|x| x.contains("nClothShape1") && x.contains("lft")), "{w:?}");
    assert_eq!(get(&mut s, "nClothShape1.fileType"), json!("nCloth"));
    // The cloth belongs to the solver, the collider too, and the plane falls onto the sphere.
    let solved = run(&mut s, "dynInfo", json!({}));
    assert!(solved.to_string().contains("nucleusShape1"), "{solved}");
    for f in 1..=30 {
        frame(&mut s, f as f64);
    }
    let p = points(&mut s, "pPlane1");
    let top = p.iter().map(|q| q.y).fold(f64::NEG_INFINITY, f64::max);
    let low = p.iter().map(|q| q.y).fold(f64::INFINITY, f64::min);
    assert!(top < 0.65 && top > 0.45, "rests on the sphere: {top}");
    assert!(low < 0.3, "the edges hang: {low}");
    // Written back as the one system's nodes, the kept attributes with them.
    let (text, _) = write_text(&mut s, "n.ma", None);
    assert!(text.contains("createNode cloth -n \"nClothShape1\""), "{text}");
    assert!(text.contains(".lft"));
}

const CLASSIC_FILE: &str = r#"//Maya ASCII 2024 scene
requires maya "2024";
currentUnit -l centimeter -a degree -t film;
createNode transform -n "particle1";
createNode particle -n "particleShape1" -p "particle1";
	setAttr ".pos0" -type "vectorArray" 3 0 2 0 1 2 0 2 2 0 ;
	setAttr ".lfm" 1;
	setAttr ".lfsp" 5;
	setAttr ".prt" 4;
createNode gravityField -n "gravityField1";
	setAttr ".mag" 9.8;
	setAttr ".dy" -1;
createNode airField -n "airField1";
	setAttr ".t" -type "double3" 1 0 0 ;
	setAttr ".mag" 2;
	setAttr ".spd" 3;
createNode pointEmitter -n "emitter1";
	setAttr ".emt" 1;
	setAttr ".rat" 50;
	setAttr ".spd" 2;
createNode transform -n "pCube1";
	setAttr ".t" -type "double3" 0 4 0 ;
createNode mesh -n "pCubeShape1" -p "pCube1";
createNode rigidBody -n "rigidBody1" -p "pCube1";
	setAttr ".act" yes;
	setAttr ".mas" 2;
	setAttr ".b" 0.3;
createNode rigidSolver -n "rigidSolver";
	setAttr ".stp" 0.03;
createNode polyCube -n "polyCube1";
createNode transform -n "hairSystem1";
createNode hairSystem -n "hairSystemShape1" -p "hairSystem1";
	setAttr ".clc" 3;
	setAttr ".stfs" 0.4;
connectAttr "polyCube1.out" "pCubeShape1.i";
connectAttr "gravityField1.of[0]" "particleShape1.ifc[0]";
connectAttr "airField1.of[0]" "particleShape1.ifc[1]";
connectAttr "particleShape1.fd" "gravityField1.ind[0]";
connectAttr "emitter1.ot[0]" "particleShape1.npt[0]";
connectAttr "rigidSolver.rsid" "rigidBody1.slv";
select -ne :time1;
connectAttr ":time1.o" "hairSystemShape1.cti";
"#;

#[test]
fn classic_particles_fields_emitter_rigid_body_and_held_hair() {
    let mut s = Session::new();
    run(&mut s, "playbackOptions", json!({"minTime": 1, "maxTime": 40}));
    let w = read_text(&mut s, CLASSIC_FILE, false);
    assert_eq!(type_of(&s, "particleShape1"), "particles");
    assert_eq!(type_of(&s, "gravityFieldShape1"), "field");
    assert_eq!(get(&mut s, "airFieldShape1.fieldType"), json!(1));
    assert_eq!(get(&mut s, "airFieldShape1.speed"), json!(3.0));
    assert_eq!(get(&mut s, "airField1.translateX"), json!(1.0));
    assert_eq!(type_of(&s, "emitterShape1"), "emitter");
    assert_eq!(get(&mut s, "emitterShape1.rate"), json!(50.0));
    assert_eq!(type_of(&s, "rigidBody1"), "rigidBody");
    assert_eq!(get(&mut s, "rigidBody1.mass"), json!(2.0));
    assert_eq!(type_of(&s, "hairSystemShape1"), "heldNode");
    assert!(w.iter().any(|x| x.contains("hairSystemShape1")), "{w:?}");
    // Classic dynamics had no solver node for particles: a body with no solver joins one made for it.
    run(&mut s, "connectDynamic", json!({"args": ["particleShape1"]}));
    for f in 1..=12 {
        frame(&mut s, f as f64);
    }
    let count = run(&mut s, "particle", json!({"args": ["particleShape1"], "query": true, "count": true}));
    assert!(count.as_i64().unwrap() > 3, "the three stored particles and new ones from the emitter: {count}");
    let mean = run(&mut s, "getParticleAttr", json!({"args": ["particleShape1.pt[0:2]"], "attribute": "position"}));
    assert!(mean[1].as_f64().unwrap() < 2.0, "the stored ones fell: {mean}");
    let ty = get(&mut s, "pCube1.translateY").as_f64().unwrap();
    assert!(ty < 4.0, "the rigid body fell: {ty}");
    // The hair is written back as it came.
    let (text, _) = write_text(&mut s, "c.ma", None);
    assert!(text.contains("createNode hairSystem -n \"hairSystemShape1\" -p \"hairSystem1\""), "{text}");
    assert!(text.contains("setAttr \".stfs\" 0.4;"));
    assert!(text.contains("connectAttr \":time1.o\" \"hairSystemShape1.cti\""), "{text}");
}

