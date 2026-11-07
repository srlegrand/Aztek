//! The audit's commands as MEL types them: flag values, query mode, component strings with a parameter in them.

use aztek_host::Core;
use serde_json::json;

fn errors(c: &mut Core) -> Vec<String> {
    c.take_log().as_array().unwrap().iter().filter(|l| l["kind"] == 2).map(|l| l["text"].as_str().unwrap().to_string()).collect()
}

#[test]
fn queries_and_settings() {
    let mut c = Core::new();
    assert_eq!(c.mel("upAxis -q -ax;"), json!("y"));
    c.mel("upAxis -ax \"z\";");
    assert_eq!(c.mel("upAxis -q -axis;"), json!("z"));
    c.mel("displayColor -active \"lead\" 4;");
    assert_eq!(c.mel("displayColor -q -active \"lead\";"), json!(4));
    assert_eq!(c.mel("convertUnit -fromUnit \"inch\" -toUnit \"cm\" \"1\";"), json!("2.54cm"));
    c.mel("cycleCheck -e off;");
    assert_eq!(c.mel("cycleCheck -q -e;"), json!(false));
    c.mel("cycleCheck -e on;");
    assert_eq!(c.mel("cycleCheck -q -e;"), json!(true));
    c.mel("polyCube -n \"a\";");
    c.mel("lockNode -l 1 a;");
    assert_eq!(c.mel("lockNode -q -l a;"), json!([true]));
    c.mel("lockNode -l 0 a;");
    c.mel("symmetricModelling -e -s 1 -ax \"x\" -ab \"world\";");
    assert_eq!(c.mel("symmetricModelling -q -s;"), json!(true));
    assert_eq!(c.mel("symmetricModelling -q -ab;"), json!("world"));
    assert_eq!(errors(&mut c), Vec::<String>::new());
}

#[test]
fn nurbs_from_mel() {
    let mut c = Core::new();
    c.mel("nurbsPlane -n \"p\" -ax 0 1 0 -w 10 -lr 1 -d 1;");
    let on = c.mel("curveOnSurface -d 1 -uv 0.25 0.25 -uv 0.75 0.25 -uv 0.75 0.75 -uv 0.25 0.75 -uv 0.25 0.25 p;");
    assert!(on.as_str().unwrap().contains("->"), "{on}");
    let name = on.as_str().unwrap().to_string();
    c.mel(&format!("trim -lu 0.5 -lv 0.5 p \"{name}\";"));
    c.mel("surface -du 1 -dv 1 -ku 0 -ku 1 -kv 0 -kv 1 -p 0 0 0 -p 0 0 1 -p 1 0 0 -p 1 0 1 -n \"raw\";");
    assert_eq!(c.mel("getAttr rawShape.degreeU;"), json!(1));
    c.mel("curve -d 1 -p 0 0 0 -p 4 0 0 -n \"l\";");
    c.mel("select -r l.ep[1];");
    assert_eq!(c.mel("ls -sl;"), json!(["l.ep[1]"]));
    c.mel("select -r l.u[0.5];");
    assert_eq!(c.mel("ls -sl;"), json!(["l.u[0.5]"]));
    assert_eq!(c.mel("distanceDimension -sp 0 0 0 -ep 3 4 0;"), json!("distanceDimensionShape1"));
    let d = c.mel("getAttr distanceDimensionShape1.distance;");
    assert!((d.as_f64().unwrap() - 5.0).abs() < 1e-9);
    assert_eq!(errors(&mut c), Vec::<String>::new());
}

#[test]
fn references_from_mel() {
    let dir = std::env::temp_dir().join(format!("aztek_audit_mel_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("source.ma").to_str().unwrap().to_string();
    let mut c = Core::new();
    c.mel("polyCube -n \"box\";");
    c.mel(&format!("file -f -save -type \"mayaAscii\" \"{path}\";"));
    let mut c = Core::new();
    c.mel(&format!("file -r -ns \"x\" \"{path}\";"));
    assert_eq!(c.mel("referenceQuery -isNodeReferenced x:box;"), json!(true));
    assert_eq!(c.mel("referenceQuery -filename x:box;"), json!(path));
    assert_eq!(c.mel("referenceQuery -rfn x:box;"), json!("xRN"));
    c.mel("file -unloadReference \"xRN\";");
    assert_eq!(c.mel("objExists x:box;"), json!(false));
    c.mel("file -loadReference \"xRN\";");
    assert_eq!(c.mel("objExists x:box;"), json!(true));
    assert_eq!(c.mel("file -q -r;"), json!([path]));
    assert_eq!(errors(&mut c), Vec::<String>::new());
}
