//! Polygon and UV commands: flags, modes and return values from the reference's documentation.

use aztek_scene::Session;
use serde_json::{json, Value};

fn run(s: &mut Session, name: &str, call: Value) -> Value {
    s.run(name, call).unwrap_or_else(|e| panic!("{name}: {e}"))
}
fn eval(s: &mut Session, obj: &str, flag: &str) -> i64 {
    let mut c = json!({"args": [obj]});
    c[flag] = json!(true);
    run(s, "polyEvaluate", c).as_i64().unwrap()
}
fn close(v: &Value, want: f64) {
    assert!((v.as_f64().unwrap_or_else(|| panic!("{v}")) - want).abs() < 1e-3, "got {v}, wanted {want}");
}
fn temp(name: &str) -> String {
    let d = std::env::temp_dir().join(format!("aztek_audit_poly_{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d.join(name).to_string_lossy().to_string()
}

#[test]
fn poly_cut_splits_a_cube() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"name": "c"}));
    let n = run(&mut s, "polyCut", json!({"args": ["c"], "pc": [0.0, 0.0, 0.0], "ro": [0.0, 0.0, 0.0]}));
    assert_eq!(run(&mut s, "nodeType", json!({"args": [n[0].clone()]})), "polyCut");
    // The plane z = 0 crosses the four side faces: 2 faces each, 4 new points.
    assert_eq!(eval(&mut s, "c", "f"), 10);
    assert_eq!(eval(&mut s, "c", "v"), 12);
    run(&mut s, "undo", json!({}));
    assert_eq!(eval(&mut s, "c", "f"), 6);
    // Moving the plane through history cuts it elsewhere.
    run(&mut s, "polyCut", json!({"args": ["c"], "pc": [0.0, 0.0, 0.0], "df": true}));
    assert_eq!(eval(&mut s, "c", "f"), 5);
    let bb = run(&mut s, "exactWorldBoundingBox", json!({"args": ["c"]}));
    close(&bb[5], 0.0);
    close(&bb[2], -0.5);
}

#[test]
fn poly_cut_extracts_a_side() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"name": "c"}));
    run(&mut s, "polyCut", json!({"args": ["c"], "pc": [0.0, 0.0, 0.0], "ro": [90.0, 0.0, 0.0], "ef": true, "eo": [0.0, 5.0, 0.0]}));
    assert_eq!(eval(&mut s, "c", "f"), 10);
    assert_eq!(eval(&mut s, "c", "v"), 16);
    // The side the plane's normal points to, -Y, went up by 5 past the rest.
    let bb = run(&mut s, "exactWorldBoundingBox", json!({"args": ["c"]}));
    close(&bb[1], 0.0);
    close(&bb[4], 5.0);
}

#[test]
fn sew_edge_joins_borders() {
    let mut s = Session::new();
    run(&mut s, "polyPlane", json!({"name": "a", "sx": 1, "sy": 1}));
    run(&mut s, "polyPlane", json!({"name": "b", "sx": 1, "sy": 1}));
    run(&mut s, "setAttr", json!({"args": ["b.tx", 1.0]}));
    let u = run(&mut s, "polyUnite", json!({"args": ["a", "b"], "ch": false, "name": "u"}));
    let u = u[0].as_str().unwrap().to_string();
    assert_eq!(eval(&mut s, &u, "v"), 8);
    run(&mut s, "polySewEdge", json!({"args": [u.clone()], "tx": 0.01}));
    assert_eq!(eval(&mut s, &u, "v"), 6);
    assert_eq!(eval(&mut s, &u, "f"), 2);
}

#[test]
fn check_compare_output() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"name": "a"}));
    run(&mut s, "polyCube", json!({"name": "b"}));
    assert_eq!(run(&mut s, "polyCheck", json!({"args": ["a"]})), 0);
    assert_eq!(run(&mut s, "polyCompare", json!({"args": ["a", "b"]})), 0);
    run(&mut s, "move", json!({"args": ["b.vtx[0]", 0.0, 1.0, 0.0], "r": true}));
    assert_eq!(run(&mut s, "polyCompare", json!({"args": ["a", "b"], "v": true})), 1);
    run(&mut s, "polySphere", json!({"name": "c", "sa": 4, "sh": 4}));
    let diff = run(&mut s, "polyCompare", json!({"args": ["a", "c"]})).as_i64().unwrap();
    assert_eq!(diff & 7, 7);
    let text = run(&mut s, "polyOutput", json!({"args": ["a"], "f": true}));
    assert!(text[0].as_str().unwrap().contains("8 vertices, 12 edges, 6 faces"));
    assert_eq!(text.as_array().unwrap().iter().filter(|x| x.as_str().unwrap().starts_with("face ")).count(), 6);
    let path = temp("out.txt");
    run(&mut s, "polyOutput", json!({"args": ["a"], "of": path.clone()}));
    assert!(std::fs::read_to_string(&path).unwrap().contains("vtx 0"));
}

#[test]
fn clean_removes_loose() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"name": "a"}));
    run(&mut s, "polyDelFacet", json!({"args": ["a.f[0]"]}));
    run(&mut s, "polyClean", json!({"args": ["a"]}));
    assert_eq!(eval(&mut s, "a", "f"), 5);
    assert_eq!(run(&mut s, "polyCheck", json!({"args": ["a"]})), 0);
}

#[test]
fn colours() {
    let mut s = Session::new();
    run(&mut s, "polyPlane", json!({"name": "p", "sx": 1, "sy": 1}));
    run(&mut s, "polyColorPerVertex", json!({"args": ["p.vtx[*]"], "rgb": [1.0, 0.0, 0.0]}));
    // Half the value darkens red.
    run(&mut s, "polyColorMod", json!({"args": ["p"], "v": 0.5}));
    close(&run(&mut s, "polyColorPerVertex", json!({"args": ["p.vtx[0]"], "q": true, "queried": ["colorR"]}))[0], 0.5);
    // A hue turn of 120 takes red to green.
    run(&mut s, "polyColorMod", json!({"args": ["p"], "hv": 120.0, "v": 2.0}));
    close(&run(&mut s, "polyColorPerVertex", json!({"args": ["p.vtx[0]"], "q": true, "queried": ["colorG"]}))[0], 1.0);
    run(&mut s, "polyColorPerVertex", json!({"args": ["p.vtx[0]"], "rgb": [0.0, 0.0, 0.0]}));
    run(&mut s, "polyAverageColor", json!({"args": ["p"]}));
    let v = run(&mut s, "polyColorPerVertex", json!({"args": ["p.vtx[1]"], "q": true, "queried": ["colorG"]}))[0].as_f64().unwrap();
    assert!(v > 0.0 && v < 1.0, "{v}");
    run(&mut s, "polyColorDel", json!({"args": ["p"]}));
    assert!(s.run("polyColorPerVertex", json!({"args": ["p.vtx[0]"], "q": true, "queried": ["colorG"]})).is_err());
}

#[test]
fn blend_colours() {
    let mut s = Session::new();
    run(&mut s, "polyPlane", json!({"name": "p", "sx": 1, "sy": 1}));
    run(&mut s, "polyColorSet", json!({"args": ["p"], "create": true, "colorSet": "a"}));
    run(&mut s, "polyColorSet", json!({"args": ["p"], "currentColorSet": true, "colorSet": "a"}));
    run(&mut s, "polyColorPerVertex", json!({"args": ["p.vtx[*]"], "rgb": [1.0, 0.0, 0.0]}));
    run(&mut s, "polyColorSet", json!({"args": ["p"], "create": true, "colorSet": "b"}));
    run(&mut s, "polyColorSet", json!({"args": ["p"], "currentColorSet": true, "colorSet": "b"}));
    run(&mut s, "polyColorPerVertex", json!({"args": ["p.vtx[*]"], "rgb": [0.0, 1.0, 0.0]}));
    run(&mut s, "polyBlendColor", json!({"args": ["p"], "bcn": "a", "src": "b", "dst": "b", "bfn": 4}));
    close(&run(&mut s, "polyColorPerVertex", json!({"args": ["p.vtx[0]"], "q": true, "queried": ["colorR"]}))[0], 0.5);
    close(&run(&mut s, "polyColorPerVertex", json!({"args": ["p.vtx[0]"], "q": true, "queried": ["colorG"]}))[0], 0.5);
}

#[test]
fn blind_data() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"name": "c"}));
    run(&mut s, "polyBlindData", json!({"args": ["c.f[1]"], "id": 9100, "at": "face", "ldn": "heat", "dbd": 0.25}));
    assert_eq!(run(&mut s, "polyQueryBlindData", json!({"args": ["c"], "id": 9100, "at": "face", "ldn": "heat", "sc": true})), json!(["c.f[1]", 0.25]));
    run(&mut s, "polyBlindData", json!({"args": ["c.vtx[2]"], "id": 9101, "at": "vertex", "ldn": "w", "ind": 3}));
    assert_eq!(run(&mut s, "polyQueryBlindData", json!({"args": ["c"], "id": 9101, "at": "vertex", "ss": "w"})), json!([3.0]));
    // Kept through a topology edit that keeps the face.
    run(&mut s, "polyExtrudeFacet", json!({"args": ["c.f[0]"], "ltz": 0.5}));
    assert_eq!(run(&mut s, "polyQueryBlindData", json!({"args": ["c.f[1]"], "id": 9100, "ldn": "heat"})), json!([0.25]));
    run(&mut s, "polyColorBlindData", json!({"args": ["c"], "id": 9100, "ldn": "heat", "cr": 1.0, "cg": 0.0, "cb": 0.0, "ncr": 0.0}));
    close(&run(&mut s, "polyColorPerVertex", json!({"args": ["c.vtx[0]"], "q": true, "queried": ["colorG"]}))[0], 0.2);
    assert!(s.run("polyBlindData", json!({"args": ["c.f[1]"], "id": 1, "ldn": "x", "sd": "text"})).is_err());
    run(&mut s, "polyBlindData", json!({"args": ["c.f[1]"], "id": 9100, "ldn": "heat", "rst": true}));
    assert_eq!(run(&mut s, "polyQueryBlindData", json!({"args": ["c.f[1]"], "id": 9100, "ldn": "heat"})), json!([]));
}

#[test]
fn move_along_normal() {
    let mut s = Session::new();
    run(&mut s, "polyPlane", json!({"name": "p", "sx": 1, "sy": 1}));
    let before = run(&mut s, "pointPosition", json!({"args": ["p.vtx[0]"], "l": true}));
    run(&mut s, "moveVertexAlongDirection", json!({"args": ["p.vtx[0]"], "n": 2.0}));
    let after = run(&mut s, "pointPosition", json!({"args": ["p.vtx[0]"], "l": true}));
    close(&after[1], before[1].as_f64().unwrap() + 2.0);
    close(&after[0], before[0].as_f64().unwrap());
    run(&mut s, "moveVertexAlongDirection", json!({"args": ["p.vtx[1]"], "d": [1.0, 0.0, 0.0], "m": 3.0}));
    close(&run(&mut s, "pointPosition", json!({"args": ["p.vtx[1]"], "l": true}))[0], 3.5);
}

#[test]
fn symmetric_modelling_toggles_and_topology() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"name": "c"}));
    assert_eq!(run(&mut s, "symmetricModelling", json!({"q": true})), false);
    run(&mut s, "symmetricModelling", json!({"e": true, "symmetry": 1, "axis": "x", "about": "world"}));
    assert_eq!(run(&mut s, "symmetricModelling", json!({"q": true})), true);
    assert_eq!(run(&mut s, "symmetricModelling", json!({"q": true, "queried": ["axis"]})), "x");
    assert_eq!(run(&mut s, "symmetricModelling", json!({"q": true, "queried": ["about"]})), "world");
    assert!(run(&mut s, "getAttr", json!({"args": ["cShape.symmetry"]})).as_i64().unwrap() > 0 || run(&mut s, "getAttr", json!({"args": ["cShape.symmetry"]})).as_bool().unwrap_or(false));
    // Topology needs the seam edge.
    assert!(s.run("symmetricModelling", json!({"topoSymmetry": true})).is_err());
    // A plane split down the middle, one side pushed off its mirror: the topology still pairs them.
    let mut s = Session::new();
    run(&mut s, "polyPlane", json!({"name": "p", "sx": 2, "sy": 2}));
    // Vertices run row by row: 0 1 2 / 3 4 5 / 6 7 8. Column x = 0 is the seam.
    run(&mut s, "move", json!({"args": ["p.vtx[2]", 0.3, 0.2, 0.0], "r": true}));
    let seam = (0..12).map(|i| format!("p.e[{i}]")).find(|e| {
        let v = run(&mut s, "polyListComponentConversion", json!({"args": [e], "fromEdge": true, "toVertex": true}));
        let l = run(&mut s, "ls", json!({"args": v, "flatten": true}));
        l == json!(["p.vtx[1]", "p.vtx[4]"])
    });
    run(&mut s, "select", json!({"args": [seam.unwrap()]}));
    run(&mut s, "symmetricModelling", json!({"topoSymmetry": true, "axis": "x"}));
    assert_eq!(run(&mut s, "symmetricModelling", json!({"q": true, "queried": ["about"]})), "topo");
    assert_eq!(run(&mut s, "symmetricModelling", json!({"q": true, "queried": ["topoSymmetry"]})), true);
    // Moving a point on one side puts its partner at the mirror image, though they didn't mirror in space.
    run(&mut s, "select", json!({"args": ["p.vtx[0]"]}));
    run(&mut s, "moveComponents", json!({"by": [0.0, 1.0, 0.0]}));
    let b1 = run(&mut s, "pointPosition", json!({"args": ["p.vtx[2]"], "l": true}));
    close(&b1[1], 1.0);
}

#[test]
fn duplicate_and_connect() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"name": "a"}));
    let made = run(&mut s, "polyDuplicateAndConnect", json!({"args": ["a"]}));
    let b = made[0].as_str().unwrap().to_string();
    run(&mut s, "polyExtrudeFacet", json!({"args": ["a.f[0]"], "ltz": 1.0}));
    assert_eq!(eval(&mut s, &b, "f"), 10);
    run(&mut s, "polySmartBevel", json!({"args": ["a.e[0]"]}));
}

fn plane_uvs(s: &mut Session) {
    run(s, "polyPlane", json!({"name": "p", "sx": 2, "sy": 2}));
}

fn uv(s: &mut Session, comp: &str) -> (f64, f64) {
    let v = run(s, "polyEditUV", json!({"args": [comp], "q": true}));
    (v[0].as_f64().unwrap(), v[1].as_f64().unwrap())
}

/// The map at a UV.
fn map_at(s: &mut Session, obj: &str, u: f64, v: f64) -> String {
    for i in 0..64 {
        let name = format!("{obj}.map[{i}]");
        let p = uv(s, &name);
        if (p.0 - u).abs() < 1e-4 && (p.1 - v).abs() < 1e-4 {
            return name;
        }
    }
    panic!("no map at {u} {v}");
}

#[test]
fn edit_shell_moves_it_all() {
    let mut s = Session::new();
    plane_uvs(&mut s);
    let (u0, v0) = uv(&mut s, "p.map[0]");
    let (u8, _) = uv(&mut s, "p.map[8]");
    run(&mut s, "polyEditUVShell", json!({"args": ["p.map[0]"], "u": 1.0, "v": 2.0}));
    let (u1, v1) = uv(&mut s, "p.map[0]");
    assert!((u1 - u0 - 1.0).abs() < 1e-5 && (v1 - v0 - 2.0).abs() < 1e-5);
    assert!((uv(&mut s, "p.map[8]").0 - u8 - 1.0).abs() < 1e-5);
    run(&mut s, "undo", json!({}));
    assert!((uv(&mut s, "p.map[0]").0 - u0).abs() < 1e-5);
}

#[test]
fn overlap_and_coverage() {
    let mut s = Session::new();
    plane_uvs(&mut s);
    assert_eq!(run(&mut s, "polyUVOverlap", json!({"args": ["p"]})), json!([]));
    close(&run(&mut s, "polyUVCoverage", json!({"args": ["p"]}))[0], 1.0);
    close(&run(&mut s, "polyUVCoverage", json!({"args": ["p.f[0]"]}))[0], 0.25);
    close(&run(&mut s, "polyUVCoverage", json!({"args": ["p"], "ur": [0.0, 0.0, 2.0, 2.0]}))[0], 0.25);
    // Stack the same plane's two copies in UV space.
    run(&mut s, "polyPlane", json!({"name": "a", "sx": 1, "sy": 1}));
    run(&mut s, "polyPlane", json!({"name": "b", "sx": 1, "sy": 1}));
    run(&mut s, "setAttr", json!({"args": ["b.tx", 2.0]}));
    let u = run(&mut s, "polyUnite", json!({"args": ["a", "b"], "ch": false, "name": "u"}));
    let u = u[0].as_str().unwrap().to_string();
    let over = run(&mut s, "polyUVOverlap", json!({"args": [u.clone()]}));
    assert_eq!(over.as_array().unwrap().len(), 2, "{over}");
    assert_eq!(run(&mut s, "polyUVOverlap", json!({"args": [u], "noc": true})), json!([]));
}

#[test]
fn stack_similar_shells() {
    let mut s = Session::new();
    run(&mut s, "polyPlane", json!({"name": "a", "sx": 1, "sy": 1}));
    run(&mut s, "polyPlane", json!({"name": "b", "sx": 1, "sy": 1}));
    run(&mut s, "setAttr", json!({"args": ["b.tx", 2.0]}));
    let u = run(&mut s, "polyUnite", json!({"args": ["a", "b"], "ch": false, "name": "u"}));
    let u = u[0].as_str().unwrap().to_string();
    run(&mut s, "polyEditUVShell", json!({"args": [format!("{u}.f[1]")], "u": 3.0}));
    assert_eq!(run(&mut s, "polyUVOverlap", json!({"args": [u.clone()]})), json!([]));
    run(&mut s, "polyUVStackSimilarShells", json!({"args": [u.clone()]}));
    assert_eq!(run(&mut s, "polyUVOverlap", json!({"args": [u]})).as_array().unwrap().len(), 2);
}

#[test]
fn rectangle_and_untangle() {
    let mut s = Session::new();
    run(&mut s, "polyPlane", json!({"name": "p", "sx": 1, "sy": 1}));
    run(&mut s, "polyEditUV", json!({"args": ["p.map[0]"], "u": -0.2, "v": 0.1}));
    run(&mut s, "polyUVRectangle", json!({"args": ["p"]}));
    let b = run(&mut s, "polyEvaluate", json!({"args": ["p"], "b2": true}));
    close(&b[0][0], -0.2);
    // Rectangle: every UV on the box, so 4 corners cover its whole area.
    let area = (b[0][1].as_f64().unwrap() - b[0][0].as_f64().unwrap()) * (b[1][1].as_f64().unwrap() - b[1][0].as_f64().unwrap());
    close(&run(&mut s, "polyUVCoverage", json!({"args": ["p"], "ur": [b[0][0], b[1][0], b[0][1], b[1][1]]}))[0], 1.0);
    assert!(area > 1.0);
    let mut s = Session::new();
    plane_uvs(&mut s);
    // Fold the middle over; untangling evens it out.
    let mid = map_at(&mut s, "p", 0.5, 0.5);
    run(&mut s, "polyEditUV", json!({"args": [mid], "u": 2.0, "v": 2.0}));
    assert!(!run(&mut s, "polyUVOverlap", json!({"args": ["p"]})).as_array().unwrap().is_empty());
    run(&mut s, "untangleUV", json!({"args": ["p"], "mb": "square"}));
    assert_eq!(run(&mut s, "polyUVOverlap", json!({"args": ["p"]})), json!([]));
}

#[test]
fn straighten_border() {
    let mut s = Session::new();
    plane_uvs(&mut s);
    // Bottom row of a 2 by 2 plane: maps 0, 1, 2. Pull the middle off the line.
    let (a, m, b) = (map_at(&mut s, "p", 0.0, 0.0), map_at(&mut s, "p", 0.5, 0.0), map_at(&mut s, "p", 1.0, 0.0));
    let before = uv(&mut s, &m);
    run(&mut s, "polyEditUV", json!({"args": [m.clone()], "v": 0.2}));
    run(&mut s, "polyStraightenUVBorder", json!({"args": [a, m.clone(), b]}));
    let after = uv(&mut s, &m);
    assert!((after.1 - before.1).abs() < 1e-3, "{after:?} {before:?}");
}

#[test]
fn auto_seam_and_optimize() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"name": "c"}));
    run(&mut s, "polyAutoProjection", json!({"args": ["c"]}));
    let n = run(&mut s, "u3dOptimize", json!({"args": ["c"]}));
    assert!(n.is_array());
    run(&mut s, "polyOptUvs", json!({"args": ["c"]}));
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"name": "c"}));
    run(&mut s, "u3dAutoSeam", json!({"args": ["c"], "s": 1.0}));
    // Every cube edge is 90 degrees: all cut, six shells.
    assert_eq!(run(&mut s, "polyEvaluate", json!({"args": ["c"], "shell": true})), 6);
}

#[test]
fn map_del_joins_them() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"name": "c"}));
    assert_eq!(eval(&mut s, "c", "uv"), 14);
    run(&mut s, "polyMapDel", json!({"args": ["c"]}));
    assert_eq!(uv(&mut s, "c.map[0]"), (0.0, 0.0));
    let b = run(&mut s, "polyEvaluate", json!({"args": ["c"], "b2": true}));
    assert_eq!(b, json!([[0.0, 0.0], [0.0, 0.0]]));
}

#[test]
fn uv_snapshot_writes_a_png() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"name": "c"}));
    let path = temp("uv.png");
    let _ = std::fs::remove_file(&path);
    run(&mut s, "uvSnapshot", json!({"args": ["c"], "n": path.clone(), "xr": 32, "yr": 16}));
    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(&bytes[..4], b"\x89PNG");
    assert_eq!(u32::from_be_bytes(bytes[16..20].try_into().unwrap()), 32);
    assert_eq!(u32::from_be_bytes(bytes[20..24].try_into().unwrap()), 16);
    assert!(s.run("uvSnapshot", json!({"args": ["c"], "n": path.clone()})).is_err());
    run(&mut s, "uvSnapshot", json!({"args": ["c"], "n": path, "o": true}));
    let ppm = temp("uv.ppm");
    run(&mut s, "uvSnapshot", json!({"args": ["c"], "n": ppm.clone(), "xr": 4, "yr": 4}));
    assert!(std::fs::read(&ppm).unwrap().starts_with(b"P6\n4 4\n255\n"));
}
