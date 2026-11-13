//! The nodes every scene has: made with the scene, kept from deletion and renaming, read from and written to files as
//! the reference names them (`select -ne :time1`), and time1 driving what is connected to it.

use aztek_scene::ma_file::{read_text, write_text};
use aztek_scene::Session;
use serde_json::{json, Value};

fn run(s: &mut Session, name: &str, call: Value) -> Value {
    s.run(name, call).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn num(s: &mut Session, plug: &str) -> f64 {
    let v = run(s, "getAttr", json!({"args": [plug]}));
    v.as_f64().or_else(|| v.as_bool().map(|b| b as i32 as f64)).unwrap_or_else(|| panic!("{plug}: {v}"))
}

fn names(v: &Value) -> Vec<String> {
    v.as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect()
}

/// A scene from the reference with nothing in it, as its 2026 release saves one, but for its values changed.
const EMPTY: &str = r#"//Maya ASCII 2026 scene
requires maya "2026";
currentUnit -l centimeter -a degree -t film;
createNode transform -s -n "persp";
createNode camera -s -n "perspShape" -p "persp";
createNode transform -s -n "top";
createNode camera -s -n "topShape" -p "top";
createNode transform -s -n "front";
createNode camera -s -n "frontShape" -p "front";
createNode transform -s -n "side";
createNode camera -s -n "sideShape" -p "side";
createNode multiplyDivide -n "md";
createNode lightLinker -s -n "lightLinker1";
	setAttr -s 2 ".lnk";
	setAttr -s 2 ".slnk";
createNode displayLayerManager -n "layerManager";
	setAttr ".cdl" 1;
	setAttr -s 2 ".dli[1]"  1;
	setAttr -s 2 ".dli";
createNode displayLayer -n "defaultLayer";
	setAttr ".ufem" -type "stringArray" 0  ;
createNode renderLayerManager -n "renderLayerManager";
createNode renderLayer -n "defaultRenderLayer";
	setAttr ".g" yes;
createNode displayLayer -n "layer1";
	setAttr ".ufem" -type "stringArray" 0  ;
	setAttr ".do" 1;
select -ne :time1;
	setAttr ".o" 1;
	setAttr ".unw" 1;
select -ne :hardwareRenderingGlobals;
	setAttr ".otfna" -type "stringArray" 2 "NURBS Curves" "NURBS Surfaces"  ;
	setAttr ".msaa" yes;
	setAttr ".fprt" yes;
select -ne :renderPartition;
	setAttr -s 2 ".st";
select -ne :defaultShaderList1;
	setAttr -s 6 ".s";
select -ne :standardSurface1;
	setAttr ".bc" -type "float3" 0.40000001 0.40000001 0.40000001 ;
	setAttr ".sr" 0.5;
select -ne :defaultRenderGlobals;
	addAttr -ci true -h true -sn "dss" -ln "defaultSurfaceShader" -dt "string";
	setAttr ".dss" -type "string" "openPBR_shader1";
select -ne :hardwareRenderGlobals;
	setAttr ".ctrs" 1024;
	setAttr ".btrs" 512;
connectAttr ":time1.o" "md.i1x";
connectAttr "layerManager.dli[0]" "defaultLayer.id";
connectAttr "renderLayerManager.rlmi[0]" "defaultRenderLayer.rlid";
connectAttr "layerManager.dli[1]" "layer1.id";
connectAttr "defaultRenderLayer.msg" ":defaultRenderingList1.r" -na;
// End of empty.ma
"#;

#[test]
fn a_new_scene_has_every_default_node() {
    let mut s = Session::new();
    aztek_scene::cameras::make_defaults(&mut s.graph);
    let dn = names(&run(&mut s, "ls", json!({"defaultNodes": true})));
    for (name, ty) in aztek_scene::defaults::DEFAULT_NODES {
        assert!(dn.contains(&name.to_string()), "{name} missing from {dn:?}");
        assert_eq!(run(&mut s, "nodeType", json!({"args": [name]})), json!(ty), "{name}");
    }
    assert_eq!(dn.len(), aztek_scene::defaults::DEFAULT_NODES.len(), "{dn:?}");
    for name in ["lightLinker1", "layerManager", "defaultLayer", "renderLayerManager", "defaultRenderLayer", "persp", "topShape"] {
        assert_eq!(run(&mut s, "objExists", json!({"args": [name]})), json!(true), "{name}");
        assert!(!dn.contains(&name.to_string()), "{name} is saved with a scene, so no default node");
    }
    // The values an empty scene of the reference's gives them.
    assert_eq!(num(&mut s, "standardSurface1.specularRoughness"), 0.5);
    assert_eq!(num(&mut s, "hardwareRenderingGlobals.floatingPointRTEnable"), 1.0);
    assert_eq!(num(&mut s, "defaultRenderLayer.global"), 1.0);
    // defaultLayer is no layer of the user's.
    assert!(aztek_scene::display_layers::all(&s.graph).is_empty());

    let ud = names(&run(&mut s, "ls", json!({"undeletable": true})));
    for name in ["time1", "lambert1", "defaultLayer", "lightLinker1", "persp", "perspShape"] {
        assert!(ud.contains(&name.to_string()), "{name} missing from {ud:?}");
    }
    for name in ["time1", "lambert1", "renderPartition", "defaultLayer"] {
        let e = s.run("delete", json!({"args": [name]})).unwrap_err().to_string();
        assert!(e.contains("can't be deleted"), "{e}");
        let e = s.run("rename", json!({"args": [name, "other"]})).unwrap_err().to_string();
        assert!(e.contains("can't be renamed"), "{e}");
    }
    run(&mut s, "select", json!({"args": ["time1"]}));
    assert!(s.run("delete", json!({})).is_err(), "selected, time1 stays too");

    // File > New makes them again.
    run(&mut s, "newScene", json!({}));
    aztek_scene::cameras::make_defaults(&mut s.graph);
    assert_eq!(run(&mut s, "objExists", json!({"args": ["dynController1"]})), json!(true));
}

#[test]
fn time1_follows_the_frame_and_drives_what_it_is_connected_to() {
    let mut s = Session::new();
    aztek_scene::cameras::make_defaults(&mut s.graph);
    run(&mut s, "createNode", json!({"args": ["multiplyDivide"], "name": "md"}));
    run(&mut s, "connectAttr", json!({"args": ["time1.outTime", "md.input1X"]}));
    run(&mut s, "setAttr", json!({"args": ["md.input2X", 2.0]}));
    for f in [1.0, 7.0, 24.5] {
        run(&mut s, "currentTime", json!({"args": [f]}));
        assert_eq!(num(&mut s, "time1.outTime"), f);
        assert_eq!(num(&mut s, "md.outputX"), 2.0 * f);
    }
}

#[test]
fn default_nodes_read_and_write_as_the_reference_names_them() {
    let mut s = Session::new();
    let w = read_text(&mut s, EMPTY, false);
    assert!(w.is_empty(), "{w:?}");
    // Values set on default nodes land on the scene's own; none was made again.
    assert_eq!(num(&mut s, "hardwareRenderingGlobals.multiSampleEnable"), 1.0);
    assert_eq!(names(&run(&mut s, "ls", json!({"args": ["time1*"]}))), ["time1"]);
    assert_eq!(names(&run(&mut s, "ls", json!({"args": ["defaultLayer*", "layerManager*"]}))), ["defaultLayer", "layerManager"]);
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["defaultRenderGlobals.defaultSurfaceShader"]})), json!("openPBR_shader1"));
    // time1 drives what the file connected it to.
    run(&mut s, "currentTime", json!({"args": [5.0]}));
    assert_eq!(num(&mut s, "md.input1X"), 5.0);

    let (text, w) = write_text(&mut s, "empty.ma", None);
    assert!(w.is_empty(), "{w:?}");
    for line in [
        "createNode lightLinker -s -n \"lightLinker1\";",
        "createNode displayLayerManager -n \"layerManager\";",
        "createNode displayLayer -n \"defaultLayer\";",
        "createNode renderLayer -n \"defaultRenderLayer\";\n\tsetAttr \".g\" yes;",
        "select -ne :time1;\n\tsetAttr \".o\" 5;",
        "select -ne :renderPartition;\n\tsetAttr -s 2 \".st\";",
        "select -ne :defaultShaderList1;\n\tsetAttr -s 6 \".s\";",
        "\tsetAttr \".msaa\" yes;",
        // What isn't modelled goes back as the file had it.
        "\tsetAttr \".otfna\" -type \"stringArray\" 2 \"NURBS Curves\" \"NURBS Surfaces\";",
        "\tsetAttr \".ctrs\" 1024;",
        "\taddAttr -ci true -sn \"dss\" -ln \"defaultSurfaceShader\" -dt \"string\";",
        "connectAttr \":time1.outTime\" \"md.input1X\";",
        "connectAttr \"layerManager.dli[1]\" \"layer1.id\";",
        "connectAttr \"defaultRenderLayer.msg\" \":defaultRenderingList1.r\" -na;",
    ] {
        assert!(text.contains(line), "{line}\n{text}");
    }
    assert!(!text.contains("createNode time"), "{text}");
    assert_eq!(text.matches("createNode displayLayer -n \"defaultLayer\"").count(), 1, "{text}");

    // And back again, the same.
    let mut b = Session::new();
    let w = read_text(&mut b, &text, false);
    assert!(w.is_empty(), "{w:?}");
    let (again, _) = write_text(&mut b, "empty.ma", None);
    let body = |t: &str| t.lines().filter(|l| !l.starts_with("//")).map(str::to_string).collect::<Vec<_>>();
    assert_eq!(body(&again), body(&text));
}
