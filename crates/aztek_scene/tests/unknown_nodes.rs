//! Nodes of a plug-in that isn't here, kept and written back as the reference keeps them; and the core types every
//! scene of the reference may hold: network, groupId, groupParts, tweak, the time curves and time conversions.

use aztek_scene::ma_file::{read_text, write_text};
use aztek_scene::Session;
use serde_json::{json, Value};

fn run(s: &mut Session, name: &str, call: Value) -> Value {
    s.run(name, call).unwrap_or_else(|e| panic!("{name}: {e}"))
}

const FILE: &str = r#"//Maya ASCII 2024 scene
requires maya "2024";
requires -nodeType "fooNode" -nodeType "fooShape" -nodeType "fooXform" "fooPlugin" "1.2";
currentUnit -l centimeter -a degree -t film;
createNode transform -n "drv";
	setAttr ".t" -type "double3" 4 0 0 ;
createNode fooXform -n "grp";
	setAttr ".t" -type "double3" 1 2 3 ;
	setAttr ".fooSetting" 7;
	setAttr -l on ".lockedThing";
	addAttr -ci true -sn "rigTag" -ln "rigTag" -dt "string";
	setAttr ".rigTag" -type "string" "spine";
createNode transform -n "child" -p "grp";
createNode locator -n "childShape" -p "child";
createNode transform -n "holder";
createNode fooShape -n "holderShape" -p "holder";
	setAttr -k off ".v";
	setAttr ".radius" 2.5;
	setAttr -s 2 ".weights[0:1]" 0.25 0.75;
createNode fooNode -n "solver1";
	setAttr ".mode" 2;
	addAttr -ci true -sn "gain" -ln "gain" -at "double";
	setAttr -k on ".gain" 0.5;
createNode transform -n "out";
connectAttr "drv.tx" "solver1.inValue";
connectAttr "solver1.outValue" "out.ry";
connectAttr "drv.wm" "holderShape.driverMatrix";
connectAttr "holderShape.result" "solver1.extra";
connectAttr "drv.tx" "grp.tx";
"#;

fn lines(text: &str) -> Vec<&str> {
    text.lines().filter(|l| !l.starts_with("//") && !l.starts_with("fileInfo")).collect()
}

#[test]
fn a_missing_plug_ins_nodes_are_kept_and_written_back() {
    let mut s = Session::new();
    let w = read_text(&mut s, FILE, false);
    assert!(w.iter().any(|x| x.starts_with("fooNode (plug-in fooPlugin): 1 node(s)")), "{w:?}");
    for (n, ty, real) in [("grp", "unknownTransform", "fooXform"), ("holderShape", "unknownDag", "fooShape"), ("solver1", "unknown", "fooNode")] {
        assert_eq!(run(&mut s, "nodeType", json!({"args": [n]})), json!(ty));
        assert_eq!(run(&mut s, "unknownNode", json!({"args": [n], "query": true, "realClassName": true})), json!(real));
        assert_eq!(run(&mut s, "unknownNode", json!({"args": [n], "query": true, "plugin": true})), json!("fooPlugin"));
    }
    // A transform kept as unknown moves its children and takes what drives it.
    assert_eq!(run(&mut s, "listRelatives", json!({"args": ["child"], "parent": true})), json!(["grp"]));
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["grp.tx"]})), json!(4.0));
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["grp.rigTag"]})), json!("spine"));
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["solver1.gain"]})), json!(0.5));

    // Renamed, edited: the kept connections follow the nodes, the added attribute keeps its new value.
    run(&mut s, "rename", json!({"args": ["drv", "driver"]}));
    run(&mut s, "rename", json!({"args": ["out", "outer"]}));
    run(&mut s, "setAttr", json!({"args": ["solver1.gain", 0.8]}));
    let (text, _) = write_text(&mut s, "u.ma", None);
    for want in [
        "requires -nodeType \"fooNode\" -nodeType \"fooShape\" -nodeType \"fooXform\" \"fooPlugin\" \"1.2\";",
        "createNode fooXform -n \"grp\";",
        "\tsetAttr \".fooSetting\" 7;",
        "\tsetAttr -l on \".lockedThing\";",
        "createNode fooShape -n \"holderShape\" -p \"holder\";",
        "\tsetAttr \".weights[0:1]\" 0.25 0.75;",
        "createNode fooNode -n \"solver1\";",
        "\tsetAttr \".gain\" 0.8;",
        "\tsetAttr \".mode\" 2;",
        "connectAttr \"driver.tx\" \"solver1.inValue\";",
        "connectAttr \"solver1.outValue\" \"outer.ry\";",
        "connectAttr \"driver.wm\" \"holderShape.driverMatrix\";",
        "connectAttr \"holderShape.result\" \"solver1.extra\";",
        "connectAttr \"driver.tx\" \"grp.tx\";",
    ] {
        assert!(text.lines().any(|l| l == want), "missing {want:?} in\n{text}");
    }
    assert!(!text.contains("createNode unknown"), "{text}");

    // Read back and written again: the same file.
    let mut b = Session::new();
    read_text(&mut b, &text, false);
    let (again, _) = write_text(&mut b, "u.ma", None);
    assert_eq!(lines(&text), lines(&again));
}

#[test]
fn the_core_types_are_made_and_work() {
    let mut s = Session::new();
    for ty in ["unknown", "unknownDag", "unknownTransform", "network", "groupId", "groupParts", "tweak", "animCurveTT", "animCurveUT", "unitToTimeConversion", "timeToUnitConversion"] {
        let made = run(&mut s, "createNode", json!({"args": [ty]}));
        assert_eq!(run(&mut s, "nodeType", json!({"args": [made]})), json!(ty));
    }
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["groupParts1.groupId"]})), json!(-1));
    // 250 of the reference's time units a frame at 24 frames a second.
    run(&mut s, "setAttr", json!({"args": ["unitToTimeConversion1.input", 2.0]}));
    run(&mut s, "setAttr", json!({"args": ["unitToTimeConversion1.conversionFactor", 250.0]}));
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["unitToTimeConversion1.output"]})), json!(2.0));
    run(&mut s, "setAttr", json!({"args": ["timeToUnitConversion1.input", 3.0]}));
    run(&mut s, "setAttr", json!({"args": ["timeToUnitConversion1.conversionFactor", 0.004]}));
    let v = run(&mut s, "getAttr", json!({"args": ["timeToUnitConversion1.output"]})).as_f64().unwrap();
    assert!((v - 3.0).abs() < 1e-9, "{v}");
    // tweak as a deformer.
    let cube = run(&mut s, "polyCube", json!({}))[0].as_str().unwrap().to_string();
    let made = run(&mut s, "deformer", json!({"args": [cube], "type": "tweak"}));
    assert_eq!(run(&mut s, "nodeType", json!({"args": [made[0]]})), json!("tweak"));
}

#[test]
fn time_curves_read_and_write() {
    let file = r#"//Maya ASCII 2024 scene
requires maya "2024";
currentUnit -l centimeter -a degree -t film;
createNode animCurveTT -n "warp";
	setAttr ".tan" 18;
	setAttr -s 2 ".ktv[0:1]"  1 1 11 21;
createNode transform -n "t";
	addAttr -ci true -sn "tm" -ln "tm" -at "double";
connectAttr "warp.o" "t.tm";
"#;
    let mut s = Session::new();
    read_text(&mut s, file, false);
    assert_eq!(run(&mut s, "nodeType", json!({"args": ["warp"]})), json!("animCurveTT"));
    let (text, _) = write_text(&mut s, "w.ma", None);
    assert!(text.contains("createNode animCurveTT -n \"warp\";"), "{text}");
    assert!(text.contains("\".ktv[0:1]\"  1 1 11 21;"), "{text}");
}
