//! The polygon commands as scripts type them: flags, query and edit, component arguments.

use aztek_host::Core;
use serde_json::{json, Value};

fn errors(c: &mut Core) -> Vec<String> {
    c.take_log().as_array().unwrap().iter().filter(|l| l["kind"] == 2).map(|l| l["text"].as_str().unwrap().to_string()).collect()
}

fn faces(c: &mut Core, m: &str) -> i64 {
    c.s.run("meshInfo", json!({"args": [m]})).unwrap()["faces"].as_i64().unwrap()
}

#[test]
fn flags_query_and_edit() {
    let mut c = Core::new();
    assert_eq!(c.mel("polyCube -w 2 -h 3 -d 4 -sx 2 -cuv 3 -ch 1 -n box;"), json!(["box", "polyCube1"]));
    assert_eq!(c.mel("polyCube -q -w box;"), json!(2.0));
    assert_eq!(c.mel("polyCube -q -sx polyCube1;"), json!(2));
    c.mel("polyCube -e -w 3 polyCube1;");
    assert_eq!(c.mel("polyCube -q -w box;"), json!(3.0));
    c.mel("polyCube -e -sx 1 box;");
    assert_eq!(faces(&mut c, "box"), 6);

    assert_eq!(c.mel("polyPlane -w 2 -h 3 -sx 2 -sy 2 -ax 0 1 0;")[1], "polyPlane1");
    assert_eq!(c.mel("polyPlane -q -h pPlane1;"), json!(3.0));
    assert_eq!(c.mel("polyCylinder -r 2 -h 2 -sx 8 -sy 2 -sz 0 -rcp 0;")[0], "pCylinder1");
    assert_eq!(c.mel("polyCylinder -q -sx pCylinder1;"), json!(8));
    assert_eq!(c.mel("polyPlatonicSolid -st 2;")[0], "pSolid1");
    assert_eq!(c.mel("polyPlatonicSolid -q -st pSolid1;"), json!(2));
    assert_eq!(faces(&mut c, "pSolid1"), 8);
    assert!(errors(&mut c).is_empty());

    assert_eq!(c.mel("polyExtrudeFacet -ltz 0.5 -kft 1 -d 2 -ch 1 box.f[0];"), json!(["polyExtrudeFace1"]));
    assert_eq!(faces(&mut c, "box"), 6 + 8);
    assert_eq!(c.mel("polyExtrudeFacet -q -ltz polyExtrudeFace1;"), json!(0.5));
    assert_eq!(c.mel("polyExtrudeFacet -q -d box;"), json!(2));
    c.mel("polyExtrudeFacet -e -d 1 polyExtrudeFace1;");
    assert_eq!(faces(&mut c, "box"), 6 + 4);
    assert!(errors(&mut c).is_empty());
}

#[test]
fn arguments_stay_out_of_the_selection() {
    let mut c = Core::new();
    c.mel("polyCube;");
    c.mel("select -r pCube1.f[0];");
    let before = c.mel("ls -sl;");
    c.mel("polyBevel3 -o 0.1 -oaf 0 -sg 1 -sa 30 -ch 1 pCube1.e[6];");
    assert!(errors(&mut c).is_empty());
    assert_eq!(faces(&mut c, "pCube1"), 7);
    // The selected face keeps being selected, under whatever id it now has.
    assert_eq!(c.mel("ls -sl;").as_array().map(Vec::len), before.as_array().map(Vec::len));

    // Conversions read what they name without selecting it.
    c.mel("select -cl;");
    let r = c.mel("polyListComponentConversion -fv -tf pCube1.vtx[0];");
    assert!(!r.as_array().unwrap().is_empty());
    assert_eq!(c.mel("ls -sl;"), json!([]));
    assert!(errors(&mut c).is_empty());
}

#[test]
fn repeated_flags_gather() {
    let mut c = Core::new();
    let r = c.mel("polyCreateFacet -p 0 0 0 -p 1 0 0 -p 1 0 1 -p 0 0 1 -n tile;");
    assert_eq!(r[0], "tile");
    assert_eq!(faces(&mut c, "tile"), 1);
    let m = c.s.run("meshInfo", json!({"args": ["tile"]})).unwrap();
    assert_eq!(m["points"], json!(4));

    c.mel("polyPlane -sx 1 -sy 1;");
    let i = c.mel("polyInfo -fe pPlane1.f[0];");
    let nums: Vec<String> = i[0].as_str().unwrap().split_whitespace().skip(2).map(str::to_string).collect();
    let line = format!("polySplit -ep {} 0.5 -ep {} 0.5 pPlane1;", nums[0], nums[2]);
    assert_eq!(c.mel(&line), json!(["polySplit1"]));
    assert_eq!(faces(&mut c, "pPlane1"), 2);
    assert!(errors(&mut c).is_empty());
}

#[test]
fn select_constraint_as_typed() {
    let mut c = Core::new();
    c.mel("polyPlane -sx 3 -sy 3;");
    c.mel("polySelectConstraint -m 3 -t 0x8000 -w 1;");
    assert_eq!(c.mel("polySelectConstraint -q -m;"), json!(3));
    let picked = c.mel("filterExpand -sm 32;");
    assert_eq!(picked.as_array().unwrap().len(), 12);
    c.mel("polySelectConstraint -dis;");
    assert_eq!(c.mel("polySelectConstraint -q -m;"), json!(0));
    assert!(errors(&mut c).is_empty());
}

#[test]
fn python_keywords() {
    let mut c = Core::new();
    let r = c.py_call("polyCube", vec![], [("w".to_string(), json!(2.0)), ("sx".to_string(), json!(2))].into_iter().collect()).unwrap();
    assert_eq!(r, json!(["pCube1", "polyCube1"]));
    let q = c.py_call("polyCube", vec![json!("pCube1")], [("q".to_string(), json!(true)), ("w".to_string(), json!(true))].into_iter().collect()).unwrap();
    assert_eq!(q, json!(2.0));
    let v: Value = c.py_call("polyExtrudeFacet", vec![json!("pCube1.f[0]")], [("ltz".to_string(), json!(1.0)), ("kft".to_string(), json!(true))].into_iter().collect()).unwrap();
    assert_eq!(v, json!(["polyExtrudeFace1"]));
    let q = c.py_call("polyExtrudeFacet", vec![json!("polyExtrudeFace1")], [("q".to_string(), json!(true)), ("ltz".to_string(), json!(true))].into_iter().collect()).unwrap();
    assert_eq!(q, json!(1.0));
    let m = c.py_call("polyColorSet", vec![json!("pCube1")], [("q".to_string(), json!(true)), ("acs".to_string(), json!(true))].into_iter().collect()).unwrap();
    assert_eq!(m, Value::Null);
}
