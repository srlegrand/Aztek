//! Symmetry on every polygon action: one side is picked, both sides change, and the mesh comes out mirrored.

use aztek_host::Core;
use glam::DVec3;
use serde_json::{json, Value};

/// Soft selection's settings belong to the process, so these take turns.
fn serial() -> std::sync::MutexGuard<'static, ()> {
    static TURN: std::sync::Mutex<()> = std::sync::Mutex::new(());
    TURN.lock().unwrap_or_else(|e| e.into_inner())
}

/// Every point, in world space.
fn points(c: &mut Core, mesh: &str) -> Vec<DVec3> {
    let n = c.s.run("meshInfo", json!({"args": [mesh]})).unwrap()["points"].as_u64().unwrap();
    (0..n)
        .map(|i| {
            let v = c.mel(&format!("xform -q -ws -t {mesh}.vtx[{i}];"));
            DVec3::new(v[0].as_f64().unwrap(), v[1].as_f64().unwrap(), v[2].as_f64().unwrap())
        })
        .collect()
}

/// Points with no partner across the plane x = at.
fn unpaired(pts: &[DVec3], at: f64) -> usize {
    pts.iter().filter(|p| !pts.iter().any(|q| (*q - DVec3::new(2.0 * at - p.x, p.y, p.z)).length() < 1e-3)).count()
}

fn counts(c: &mut Core, mesh: &str) -> (u64, u64) {
    let i = c.s.run("meshInfo", json!({"args": [mesh]})).unwrap();
    (i["points"].as_u64().unwrap(), i["faces"].as_u64().unwrap())
}

/// A symmetric box in object space, grid 4 x 2 x 2, symmetry on, and its counts.
fn setup(c: &mut Core) -> (u64, u64) {
    c.mel("polyCube -n m -w 4 -h 2 -d 2 -sx 4 -sy 2 -sz 2;");
    c.mel("symmetricModelling -e -s 1 -ax x -ab object;");
    counts(c, "m")
}

fn comp_on_plus_x(c: &mut Core, kind: &str) -> String {
    // The first component whose every point lies right of x = 0.5.
    let n = match kind {
        "f" => c.s.run("meshInfo", json!({"args": ["m"]})).unwrap()["faces"].as_u64().unwrap(),
        "e" => c.s.run("meshInfo", json!({"args": ["m"]})).unwrap()["edges"].as_u64().unwrap(),
        _ => c.s.run("meshInfo", json!({"args": ["m"]})).unwrap()["points"].as_u64().unwrap(),
    };
    for i in 0..n {
        let name = format!("m.{kind}[{i}]");
        if kind == "vtx" {
            let v = c.mel(&format!("xform -q -ws -t {name};"));
            if v[0].as_f64().is_some_and(|x| x > 0.5 && x < 1.9) {
                return name;
            }
            continue;
        }
        let v = c.mel(&format!("xform -q -ws -t {name};"));
        let xs: Vec<f64> = v.as_array().unwrap().iter().step_by(3).filter_map(Value::as_f64).collect();
        if !xs.is_empty() && xs.iter().all(|x| *x > 0.5) {
            return name;
        }
    }
    panic!("no {kind} on +x");
}

/// Faces whose centre has no partner across x = 0.
fn face_centres(c: &mut Core, mesh: &str) -> usize {
    let n = c.s.run("meshInfo", json!({"args": [mesh]})).unwrap()["faces"].as_u64().unwrap();
    let centres: Vec<DVec3> = (0..n)
        .map(|i| {
            let v = c.mel(&format!("xform -q -ws -t {mesh}.f[{i}];"));
            let l: Vec<f64> = v.as_array().unwrap().iter().filter_map(Value::as_f64).collect();
            let k = (l.len() / 3).max(1) as f64;
            let mut s = DVec3::ZERO;
            for p in l.chunks(3) {
                s += DVec3::new(p[0], p[1], p[2]);
            }
            s / k
        })
        .collect();
    unpaired(&centres, 0.0)
}

#[derive(Debug)]
struct Outcome {
    action: String,
    error: Option<String>,
    changed: bool,
    unpaired: usize,
    odd: bool,
    /// Faces with no partner across the plane.
    lopsided: usize,
    moved: bool,
}

fn run(action: &str, kind: &str, mel_cmd: &str) -> Outcome {
    let mut c = Core::new();
    let before = setup(&mut c);
    let start = points(&mut c, "m");
    let comp = comp_on_plus_x(&mut c, kind);
    c.mel_checked(&format!("select {comp};")).unwrap();
    let cmd = mel_cmd.replace("%", &comp);
    let r = c.mel_checked(&cmd);
    let pts = points(&mut c, "m");
    let after = counts(&mut c, "m");
    let lopsided = face_centres(&mut c, "m");
    let moved = pts != start;
    Outcome {
        action: action.to_string(),
        error: r.err(),
        changed: after != before,
        unpaired: unpaired(&pts, 0.0),
        // A one-sided topology change adds an odd count on a mesh mirrored about no point on the plane, or leaves points unpaired.
        odd: after.1 % 2 != before.1 % 2,
        lopsided,
        moved,
    }
}

/// The table is every component action in the Edit Mesh and Mesh menus with a selection, as scripts run them.
fn actions() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        ("extrude face", "f", "polyExtrude -thickness 0.3;"),
        ("polyExtrudeFacet", "f", "polyExtrudeFacet -ltz 0.3;"),
        ("polyExtrudeEdge", "e", "polyExtrudeEdge -ltz 0.3;"),
        ("polyExtrudeVertex", "vtx", "polyExtrudeVertex -l 0.2 -w 0.1;"),
        ("bevel", "e", "polyBevel -offset 0.1;"),
        ("polyBevel3", "e", "polyBevel3 -o 0.1;"),
        ("delete face", "f", "deleteComponents;"),
        ("delete vertex", "vtx", "polyDelVertex;"),
        ("delete edge", "e", "polyDelEdge;"),
        ("dissolve", "e", "polyDissolve;"),
        ("poke", "f", "polyPoke;"),
        ("chamfer", "vtx", "polyChamferVtx;"),
        ("inset", "f", "polyInset -thickness 0.1;"),
        ("add divisions", "f", "polySubdivideFacet;"),
        ("collapse", "e", "polyCollapse;"),
        ("merge center", "e", "polyMergeCenter;"),
        ("detach", "e", "polyDetachEdges;"),
        ("spin", "e", "polySpinEdge;"),
        ("chip off", "f", "polyChipOff -dup 1 -ltz 0.2;"),
        ("extract chip", "f", "polyChipOff -dup 0 -ltz 0.2;"),
        ("wedge", "f", "polyWedgeFace -wa 45 -d 2 -ed 0;"),
        ("split ring", "e", "polySplitRing -wt 0.3;"),
        ("split rings", "e", "polySplitRings;"),
        ("connect", "e", "polyConnectComponents;"),
        ("triangulate", "f", "polyTriangulate;"),
        ("flip triangle", "e", "polyFlipEdge;"),
        ("average", "vtx", "polyAverageVertex;"),
        ("move face normal", "f", "polyMoveFacet -ltz 0.3;"),
        ("crease", "e", "polyCrease -v 2;"),
        ("spherize", "f", "polySpherize;"),
        ("circularize", "f", "polyCircularize;"),
        ("offset loop", "e", "polyDuplicateEdge -of 0.3;"),
        ("edge flow", "e", "polyEditEdgeFlow;"),
        ("assign hole", "f", "polyHole;"),
    ]
}

#[test]
#[ignore = "lists what each action does; the assertions below are what must hold"]
fn report() {
    let _one_at_a_time = serial();
    for (a, k, m) in actions() {
        println!("{:?}", run(a, k, m));
    }
}

#[test]
fn every_action_comes_out_mirrored() {
    let _one_at_a_time = serial();
    let mut bad = Vec::new();
    for (a, k, m) in actions() {
        let o = run(a, k, m);
        if o.error.is_some() || o.unpaired != 0 || o.lopsided != 0 {
            bad.push(format!("{a}: {o:?}"));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

#[test]
fn the_tools_mirror_too() {
    let _one_at_a_time = serial();
    // Insert Edge Loop.
    let mut c = Core::new();
    setup(&mut c);
    let e = comp_on_plus_x(&mut c, "e");
    c.mel(&format!("select {e};"));
    c.exec("polyInsertLoop", json!({"fraction": 0.3})).unwrap();
    let pts = points(&mut c, "m");
    assert_eq!(unpaired(&pts, 0.0), 0, "insert loop");
    assert_eq!(face_centres(&mut c, "m"), 0, "insert loop");

    // Merge within a distance.
    let mut c = Core::new();
    setup(&mut c);
    let v = comp_on_plus_x(&mut c, "f");
    c.mel(&format!("select {v};"));
    c.exec("polyMerge", json!({"distance": 1.1})).unwrap();
    let pts = points(&mut c, "m");
    assert_eq!(unpaired(&pts, 0.0), 0, "merge");

    // Bridge two border edges, after deleting faces on one side.
    let mut c = Core::new();
    c.mel("polyPlane -n m -w 6 -h 2 -sx 6 -sy 1;");
    c.mel("symmetricModelling -e -s 1 -ax x -ab object;");
    let before = counts(&mut c, "m");
    c.mel("select m.f[4]; deleteComponents;");
    let (edges_a, edges_b) = shared_edges_of_hole(&mut c);
    c.mel(&format!("select m.e[{edges_a}] m.e[{edges_b}];"));
    c.exec("polyBridge", json!({})).unwrap();
    assert_eq!(counts(&mut c, "m").1, before.1, "both holes bridged");
    assert_eq!(face_centres(&mut c, "m"), 0, "bridge");

    // Quad Draw's face.
    let mut c = Core::new();
    c.mel("polyPlane -n m -w 4 -h 2 -sx 4 -sy 1;");
    c.mel("symmetricModelling -e -s 1 -ax x -ab object;");
    let before = counts(&mut c, "m").1;
    c.exec("polyAppendVertex", json!({"mesh": "m", "items": [{"position": [1.0, 0.0, 2.0]}, {"position": [2.0, 0.0, 2.0]}, {"position": [2.0, 1.0, 2.0]}]})).unwrap();
    assert_eq!(counts(&mut c, "m").1, before + 2, "the face and its image");
    assert_eq!(unpaired(&points(&mut c, "m"), 0.0), 0);

    // Multi-Cut's loop.
    let mut c = Core::new();
    setup(&mut c);
    let e = comp_on_plus_x(&mut c, "e");
    let id: i64 = e.trim_start_matches("m.e[").trim_end_matches(']').parse().unwrap();
    c.exec("polyKnife", json!({"mesh": "m", "loopEdge": id, "fraction": 0.3})).unwrap();
    assert_eq!(unpaired(&points(&mut c, "m"), 0.0), 0, "knife loop");
    assert_eq!(face_centres(&mut c, "m"), 0, "knife loop");
}

/// The two border edges of the hole at face 4 of the strip, facing each other.
fn shared_edges_of_hole(c: &mut Core) -> (i64, i64) {
    let n = c.s.run("meshInfo", json!({"args": ["m"]})).unwrap()["edges"].as_i64().unwrap();
    let (mut a, mut b) = (-1, -1);
    for i in 0..n {
        let v = c.mel(&format!("xform -q -ws -t m.e[{i}];"));
        let l: Vec<f64> = v.as_array().unwrap().iter().filter_map(Value::as_f64).collect();
        if l.len() == 6 && (l[0] - l[3]).abs() < 1e-6 {
            if (l[0] - 1.0).abs() < 1e-6 {
                a = i;
            }
            if (l[0] - 2.0).abs() < 1e-6 {
                b = i;
            }
        }
    }
    (a, b)
}

#[test]
fn soft_select_moves_both_sides_alike() {
    let _one_at_a_time = serial();
    let mut c = Core::new();
    c.mel("polyCube -n m -w 4 -h 2 -d 2 -sx 8 -sy 4 -sz 4;");
    c.mel("symmetricModelling -e -s 1 -ax x -ab object;");
    c.mel("softSelect -e -sse 1 -ssd 1.5;");
    let v = comp_on_plus_x(&mut c, "vtx");
    c.mel(&format!("select {v};"));
    c.exec("moveComponents", json!({"by": [0.0, 0.4, 0.0]})).unwrap();
    assert_eq!(unpaired(&points(&mut c, "m"), 0.0), 0);
    c.mel("softSelect -e -sse 0;");
}

#[test]
fn world_symmetry_mirrors_in_world_space() {
    let _one_at_a_time = serial();
    // Moved off the origin: the plane stays at the world's x = 0, not the object's.
    let mut c = Core::new();
    c.mel("polyCube -n m -w 6 -h 2 -d 2 -sx 6 -sy 2 -sz 2; move 1 0 0 m;");
    c.mel("symmetricModelling -e -s 1 -ax x -ab world;");
    assert_eq!(c.mel("symmetricModelling -q -ab;"), json!("world"));
    // The vertices at world x = 2 and -2 are images of each other.
    let n = points(&mut c, "m").len();
    let mut at_two = None;
    for i in 0..n {
        let v = c.mel(&format!("xform -q -ws -t m.vtx[{i}];"));
        if (v[0].as_f64().unwrap() - 2.0).abs() < 1e-6 && (v[1].as_f64().unwrap() - 1.0).abs() < 1e-6 {
            at_two = Some(i);
            break;
        }
    }
    let i = at_two.expect("a vertex at x = 2");
    c.mel(&format!("select m.vtx[{i}];"));
    c.exec("moveComponents", json!({"by": [0.0, 0.5, 0.0]})).unwrap();
    let raised: Vec<DVec3> = points(&mut c, "m").into_iter().filter(|p| p.y > 1.2).collect();
    assert_eq!(raised.len(), 2, "{raised:?}");
    assert!((raised[0].x + raised[1].x).abs() < 1e-6, "mirrored about world x = 0: {raised:?}");

    // The same cube in object space mirrors about its pivot's own plane instead.
    let mut c = Core::new();
    c.mel("polyCube -n m -w 6 -h 2 -d 2 -sx 6 -sy 2 -sz 2; move 1 0 0 m;");
    c.mel("symmetricModelling -e -s 1 -ax x -ab object;");
    let n = points(&mut c, "m").len();
    for i in 0..n {
        let v = c.mel(&format!("xform -q -ws -t m.vtx[{i}];"));
        if (v[0].as_f64().unwrap() - 2.0).abs() < 1e-6 && (v[1].as_f64().unwrap() - 1.0).abs() < 1e-6 {
            c.mel(&format!("select m.vtx[{i}];"));
            break;
        }
    }
    c.exec("moveComponents", json!({"by": [0.0, 0.5, 0.0]})).unwrap();
    let raised: Vec<DVec3> = points(&mut c, "m").into_iter().filter(|p| p.y > 1.2).collect();
    assert_eq!(raised.len(), 2);
    assert!((raised[0].x + raised[1].x - 2.0).abs() < 1e-6, "mirrored about the object's x: {raised:?}");
}

#[test]
fn world_symmetry_on_a_turned_object() {
    let _one_at_a_time = serial();
    // A 12-sided cylinder turned 30 degrees still has a mirror at world x = 0.
    let mut c = Core::new();
    c.mel("polyCylinder -n m -sx 12 -sy 1 -r 2 -h 2; rotate -a 0 30 0 m;");
    c.mel("symmetricModelling -e -s 1 -ax x -ab world;");
    let pts = points(&mut c, "m");
    let i = pts.iter().position(|p| p.x > 1.0 && p.y > 0.5).unwrap();
    c.mel(&format!("select m.vtx[{i}];"));
    c.exec("moveComponents", json!({"by": [0.0, 0.5, 0.0]})).unwrap();
    let raised: Vec<DVec3> = points(&mut c, "m").into_iter().filter(|p| p.y > 1.2).collect();
    assert_eq!(raised.len(), 2, "{raised:?}");
    assert!((raised[0].x + raised[1].x).abs() < 1e-6, "{raised:?}");
    assert!((raised[0].z - raised[1].z).abs() < 1e-6, "{raised:?}");
}

#[test]
fn global_falloff_reaches_other_meshes_and_volume_does_not() {
    let _one_at_a_time = serial();
    let scene = |c: &mut Core, mode: i32| {
        c.mel("polyPlane -n a -w 1 -h 1 -sx 2 -sy 2; polyPlane -n b -w 1 -h 1 -sx 2 -sy 2; move 1.2 0 0 b;");
        c.mel(&format!("softSelect -e -sse 1 -ssd 2 -ssf {mode};"));
        c.mel("select a.vtx[4];");
        c.exec("moveComponents", json!({"by": [0.0, 1.0, 0.0]})).unwrap();
        let moved = points(c, "b").iter().filter(|p| p.y > 1e-3).count();
        c.mel("softSelect -e -sse 0 -ssf 0;");
        moved
    };
    assert_eq!(scene(&mut Core::new(), 0), 0, "volume stays on its mesh");
    assert!(scene(&mut Core::new(), 2) > 0, "global reaches the other mesh");
}

#[test]
fn surface_falloff_goes_by_the_surface() {
    let _one_at_a_time = serial();
    // Two lips: a strip folded back so its ends lie close in space but far along it.
    let measure = |mode: i32| {
        let mut c = Core::new();
        c.mel("polyPlane -n m -w 8 -h 1 -sx 8 -sy 1;");
        let pts = points(&mut c, "m");
        for (i, p) in pts.iter().enumerate() {
            if p.x > 0.0 {
                c.mel(&format!("move -r {} 0.2 0 m.vtx[{i}];", -2.0 * p.x));
            }
        }
        let start = points(&mut c, "m");
        let seed = start.iter().position(|p| p.y == 0.0 && p.x < -3.5).unwrap();
        c.mel(&format!("softSelect -e -sse 1 -ssd 1 -ssf {mode};"));
        c.mel(&format!("select m.vtx[{seed}];"));
        c.exec("moveComponents", json!({"by": [0.0, -0.5, 0.0]})).unwrap();
        let end = points(&mut c, "m");
        let upper_moved = start.iter().zip(&end).filter(|(a, b)| a.y > 0.1 && (**a - **b).length() > 1e-4).count();
        c.mel("softSelect -e -sse 0 -ssf 0;");
        upper_moved
    };
    assert!(measure(0) > 0, "volume reaches across the gap");
    assert_eq!(measure(1), 0, "surface does not");
}

#[test]
fn object_falloff_takes_other_objects_along() {
    let _one_at_a_time = serial();
    let mut c = Core::new();
    c.mel("polyCube -n a; polyCube -n b; move 1 0 0 b; polyCube -n far; move 50 0 0 far;");
    c.mel("softSelect -e -sse 1 -ssd 4 -ssf 3;");
    c.mel("select a;");
    let shape_before = points(&mut c, "b");
    c.exec("move", json!({"args": ["a"], "by": [0.0, 2.0, 0.0]})).unwrap();
    let y = c.mel("getAttr b.translateY;").as_f64().unwrap();
    assert!(y > 0.1 && y < 2.0, "b takes a share: {y}");
    assert_eq!(c.mel("getAttr far.translateY;").as_f64().unwrap(), 0.0);
    let shape_after = points(&mut c, "b");
    for (p, q) in shape_before.iter().zip(&shape_after) {
        assert!(((*q - *p) - DVec3::new(0.0, y, 0.0)).length() < 1e-6, "b keeps its shape");
    }
    c.mel("undo;");
    assert_eq!(c.mel("getAttr b.translateY;").as_f64().unwrap(), 0.0, "one undo");
    c.mel("softSelect -e -sse 0 -ssf 0;");
}
