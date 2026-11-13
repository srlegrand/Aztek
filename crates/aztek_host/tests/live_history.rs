//! Modelling and curve edits make live history nodes, as the reference's do, whether run from a script or from the
//! menus' commands: changing an input or a node's attribute changes the result.

use aztek_host::Core;
use serde_json::{json, Value};

fn nums(v: &Value) -> Vec<f64> {
    v.as_array().map(|a| a.iter().filter_map(Value::as_f64).collect()).unwrap_or_default()
}

fn types_in(c: &mut Core, obj: &str) -> Vec<String> {
    c.mel(&format!("listHistory {obj}")).as_array().unwrap().iter().map(|n| c.mel(&format!("nodeType {}", n.as_str().unwrap())).as_str().unwrap().to_string()).collect()
}

#[test]
fn booleans_and_combine_are_live_nodes() {
    let mut c = Core::new();
    c.mel("polyCube -n a; polyCube -n b; move 0.5 0.5 0.5 b;");
    let made = c.mel("polyBoolean -operation \"union\" a b");
    let made = made.as_array().unwrap();
    assert_eq!(c.mel(&format!("nodeType {}", made[1].as_str().unwrap())), json!("polyCBoolOp"));
    assert_eq!(c.mel(&format!("getAttr {}.operation", made[1].as_str().unwrap())), json!(1));
    let out = made[0].as_str().unwrap().to_string();
    assert!(types_in(&mut c, &out).contains(&"polyCBoolOp".to_string()));
    // Moving an input moves the result.
    let before = nums(&c.mel(&format!("exactWorldBoundingBox {out}")));
    c.mel("move -r 0 2 0 b;");
    let after = nums(&c.mel(&format!("exactWorldBoundingBox {out}")));
    assert!((after[4] - before[4] - 2.0).abs() < 1e-6, "{before:?} {after:?}");
    // The node's own settings are the reference's, and Aztek's seam clean-up after them.
    for attr in ["classification", "useThresholds", "vertexDistanceThreshold", "faceAreaThreshold", "preserveColor", "cleanup", "filletWidth"] {
        c.mel_checked(&format!("getAttr {}.{attr}", made[1].as_str().unwrap())).unwrap();
    }

    c.mel("file -f -new; polyCube -n a; polyCube -n b; move 3 0 0 b;");
    // Mesh > Combine: a polyUnite node.
    let made = c.mel("polyCombine a b");
    let made = made.as_str().unwrap().to_string();
    assert!(types_in(&mut c, &made).contains(&"polyUnite".to_string()), "{:?}", types_in(&mut c, &made));
    c.mel("move -r 0 5 0 b;");
    let bb = nums(&c.mel(&format!("exactWorldBoundingBox {made}")));
    assert!((bb[4] - 5.5).abs() < 1e-6, "{bb:?}");
}

#[test]
fn face_and_edge_edits_are_nodes() {
    let mut c = Core::new();
    c.mel("polyCube -n a -sx 2;");
    let made = c.mel("polyExtract a.f[1]");
    assert_eq!(made.as_array().unwrap().len(), 2, "{made}");
    assert!(!c.mel("ls -type polyChipOff").as_array().unwrap().is_empty());
    assert!(!c.mel("ls -type polySeparate").as_array().unwrap().is_empty());

    c.mel("file -f -new; polyCube -n a;");
    assert_eq!(c.mel("polySphericalProjection a.f[*]"), json!(["polySphProj1"]));
    assert_eq!(c.mel("polyCylindricalProjection a.f[*]"), json!(["polyCylProj1"]));
    assert_eq!(c.mel("polyPlanarProjection a.f[*]"), json!(["polyPlanarProj1"]));

    c.mel("file -f -new; polyCylinder -n a -sh 4; select -r a.e[60:79];");
    c.mel("polyEditEdgeFlow -adjustEdgeFlow 1;");
    assert!(types_in(&mut c, "a").contains(&"polyEditEdgeFlow".to_string()), "{:?}", types_in(&mut c, "a"));

    c.mel("file -f -new; polyCube -n a -sx 3 -sy 3 -sz 3; select -r a.f[4];");
    c.mel("polyCircularize;");
    assert!(types_in(&mut c, "a").contains(&"polyCircularize".to_string()), "{:?}", types_in(&mut c, "a"));
}

#[test]
fn curve_edits_are_nodes() {
    let mut c = Core::new();
    c.mel("curve -d 1 -p 0 0 0 -p 1 0 0 -n p; curve -d 1 -p 1 0 0 -p 2 0 0 -n q;");
    let made = c.mel("attachCurve p q");
    assert_eq!(made, json!(["p", "attachCurve1"]));
    c.mel("move -r 0 3 0 q;");
    let bb = nums(&c.mel("exactWorldBoundingBox p"));
    assert!((bb[4] - 3.0).abs() < 1e-6, "the joined curve follows the second: {bb:?}");
    assert!(types_in(&mut c, "p").contains(&"attachCurve".to_string()), "{:?}", types_in(&mut c, "p"));

    c.mel("file -f -new; curve -d 3 -p 0 0 0 -p 1 1 0 -p 2 0 0 -p 3 1 0 -p 4 0 0 -p 5 1 0 -n p;");
    let made = c.mel("detachCurve -p 0.8 -p 2 p");
    assert_eq!(made.as_array().unwrap().len(), 4, "{made}");
    assert_eq!(c.mel("nodeType detachCurve1"), json!("detachCurve"));
    let pieces = made.as_array().unwrap()[1].as_str().unwrap().to_string();
    let before = nums(&c.mel(&format!("exactWorldBoundingBox {pieces}")));
    c.mel("setAttr detachCurve1.parameter[1] 2.5;");
    let after = nums(&c.mel(&format!("exactWorldBoundingBox {pieces}")));
    assert_ne!(before, after, "the pieces follow the node");

    c.mel("file -f -new; polyCube -n a; select -r a.e[0];");
    let made = c.mel("polyToCurve -dg 1");
    assert_eq!(c.mel(&format!("nodeType {}", made[1].as_str().unwrap())), json!("polyEdgeToCurve"));
    let crv = made[0].as_str().unwrap().to_string();
    let before = nums(&c.mel(&format!("exactWorldBoundingBox {crv}")));
    c.mel("move -r 0 0 2 a;");
    let after = nums(&c.mel(&format!("exactWorldBoundingBox {crv}")));
    assert!((after[2] - before[2] - 2.0).abs() < 1e-6, "the curve follows the mesh: {before:?} {after:?}");
}
