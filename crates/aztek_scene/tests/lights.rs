//! Lights: node types, commands, the default set, links, and .ma as the reference lays it out.

use aztek_scene::ma_file::{read_text, write_text};
use aztek_scene::Session;
use serde_json::{json, Value};

fn run(s: &mut Session, name: &str, call: Value) -> Value {
    s.run(name, call).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn session() -> Session {
    let mut s = Session::new();
    aztek_scene::cameras::make_defaults(&mut s.graph);
    s
}

fn get(s: &mut Session, plug: &str) -> Value {
    run(s, "getAttr", json!({"args": [plug]}))
}

#[test]
fn every_light_type_has_its_transform_shape_and_defaults() {
    let mut s = session();
    for (ty, tr, sh) in [
        ("ambientLight", "ambientLight1", "ambientLightShape1"),
        ("directionalLight", "directionalLight1", "directionalLightShape1"),
        ("pointLight", "pointLight1", "pointLightShape1"),
        ("spotLight", "spotLight1", "spotLightShape1"),
        ("areaLight", "areaLight1", "areaLightShape1"),
        ("volumeLight", "volumeLight1", "volumeLightShape1"),
    ] {
        assert_eq!(run(&mut s, "shadingNode", json!({"args": [ty], "asLight": true})), tr);
        assert_eq!(run(&mut s, "listRelatives", json!({"args": [tr], "shapes": true})), json!([sh]));
        assert_eq!(get(&mut s, &format!("{sh}.intensity")), 1.0);
        assert_eq!(get(&mut s, &format!("{sh}.in")), 1.0);
        assert_eq!(get(&mut s, &format!("{sh}.colorR")), 1.0);
        assert_eq!(get(&mut s, &format!("{sh}.shadColorB")), 0.0);
        assert_eq!(run(&mut s, "getClassification", json!({"args": [ty], "satisfies": "light"})), true);
    }
    let shapes = run(&mut s, "ls", json!({"type": "light"}));
    assert_eq!(shapes.as_array().unwrap().len(), 6);
    assert_eq!(run(&mut s, "ls", json!({"lights": true})), shapes);
    assert_eq!(get(&mut s, "ambientLightShape1.as"), 0.45);
    assert_eq!(get(&mut s, "spotLightShape1.ca"), 40.0);
    assert_eq!(get(&mut s, "spotLightShape1.pa"), 0.0);
    assert_eq!(get(&mut s, "spotLightShape1.dro"), 0.0);
    assert_eq!(get(&mut s, "pointLightShape1.de"), 0);
    assert_eq!(get(&mut s, "pointLightShape1.edi"), true);
    assert_eq!(get(&mut s, "pointLightShape1.esp"), true);
    assert_eq!(get(&mut s, "pointLightShape1.dms"), false);
    assert_eq!(get(&mut s, "areaLightShape1.nrm"), true);
    assert_eq!(get(&mut s, "volumeLightShape1.lis"), 1);
    assert_eq!(get(&mut s, "volumeLightShape1.ac"), 360.0);
    // A volume light is built on a point light; a point light on the abstract ones.
    assert_eq!(run(&mut s, "ls", json!({"type": "pointLight"})).as_array().unwrap().len(), 2);
    assert_eq!(run(&mut s, "ls", json!({"type": "nonAmbientLightShapeNode"})).as_array().unwrap().len(), 5);
    assert_eq!(run(&mut s, "ls", json!({"type": "renderLight"})).as_array().unwrap().len(), 6);
}

#[test]
fn light_commands_create_edit_and_query() {
    let mut s = session();
    let sh = run(&mut s, "directionalLight", json!({"rotation": [45.0, 30.0, 15.0], "intensity": 0.8, "rgb": [1.0, 0.5, 0.25]}));
    assert_eq!(sh, "directionalLightShape1");
    assert_eq!(get(&mut s, "directionalLight1.rotateX"), 45.0);
    assert_eq!(get(&mut s, "directionalLightShape1.colorG"), 0.5);
    run(&mut s, "directionalLight", json!({"args": ["directionalLightShape1"], "edit": true, "intensity": 0.5}));
    assert_eq!(run(&mut s, "directionalLight", json!({"args": ["directionalLightShape1"], "query": true, "queried": ["intensity"]})), 0.5);
    assert_eq!(run(&mut s, "directionalLight", json!({"args": ["directionalLight1"], "query": true, "queried": ["rgb"]})), json!([1.0, 0.5, 0.25]));

    run(&mut s, "spotLight", json!({"coneAngle": 45.0, "penumbra": 5.0, "dropOff": 2.0}));
    assert_eq!(get(&mut s, "spotLightShape1.coneAngle"), 45.0);
    assert_eq!(get(&mut s, "spotLightShape1.penumbraAngle"), 5.0);
    assert_eq!(run(&mut s, "spotLight", json!({"args": ["spotLightShape1"], "query": true, "queried": ["dropOff"]})), 2.0);

    run(&mut s, "pointLight", json!({"decayRate": 2, "position": [1.0, 2.0, 3.0], "name": "key"}));
    assert_eq!(get(&mut s, "keyShape.decayRate"), 2);
    assert_eq!(get(&mut s, "key.translateY"), 2.0);

    run(&mut s, "ambientLight", json!({"ambientShade": 0.2}));
    assert_eq!(get(&mut s, "ambientLightShape1.ambientShade"), 0.2);
    assert!(s.run("spotLight", json!({"args": ["key"], "edit": true, "coneAngle": 3.0})).is_err());
}

#[test]
fn a_new_light_joins_the_default_set_and_listlights_finds_it() {
    let mut s = session();
    assert_eq!(run(&mut s, "listLights", json!({})), json!([]));
    run(&mut s, "pointLight", json!({}));
    assert_eq!(run(&mut s, "sets", json!({"args": ["defaultLightSet"], "query": true})), json!(["pointLight1"]));
    assert_eq!(run(&mut s, "listLights", json!({})), json!(["pointLight1"]));
    assert_eq!(run(&mut s, "listLights", json!({"shapes": true})), json!(["pointLightShape1"]));
    assert_eq!(run(&mut s, "ls", json!({"selection": true})), json!(["pointLight1"]));
    // Undo takes it all back, membership with the light.
    run(&mut s, "undo", json!({}));
    assert_eq!(run(&mut s, "listLights", json!({})), json!([]));
    assert_eq!(run(&mut s, "sets", json!({"args": ["defaultLightSet"], "query": true})), json!([]));
    // A light made with createNode isn't in the set: it lights nothing until linked.
    assert_eq!(run(&mut s, "createNode", json!({"args": ["spotLight"]})), "spotLightShape1");
    assert_eq!(run(&mut s, "sets", json!({"args": ["defaultLightSet"], "query": true})), json!([]));
    assert_eq!(run(&mut s, "spotLight", json!({"args": ["spotLight1"], "query": true, "queried": ["exclusive"]})), true);
    run(&mut s, "spotLight", json!({"args": ["spotLight1"], "edit": true, "exclusive": false}));
    assert_eq!(run(&mut s, "sets", json!({"args": ["defaultLightSet"], "query": true})), json!(["spotLight1"]));
}

#[test]
fn a_duplicate_light_stays_in_the_default_set() {
    let mut s = session();
    run(&mut s, "pointLight", json!({}));
    run(&mut s, "duplicate", json!({"args": ["pointLight1"]}));
    assert_eq!(run(&mut s, "sets", json!({"args": ["defaultLightSet"], "query": true})), json!(["pointLight1", "pointLight2"]));
}

#[test]
fn lightlink_breaks_makes_and_undoes() {
    let mut s = session();
    run(&mut s, "polyCube", json!({"name": "box"}));
    run(&mut s, "polySphere", json!({"name": "ball"}));
    run(&mut s, "pointLight", json!({}));
    let lit = |s: &mut Session| run(s, "lightlink", json!({"query": true, "light": "pointLight1", "shapes": false, "sets": false}));
    assert_eq!(lit(&mut s), json!(["box", "ball"]));
    run(&mut s, "lightlink", json!({"b": true, "light": "pointLight1", "object": "box"}));
    assert_eq!(lit(&mut s), json!(["ball"]));
    assert_eq!(run(&mut s, "lightlink", json!({"object": "box", "sets": false})), json!([]));
    assert_eq!(run(&mut s, "lightlink", json!({"object": "ball", "sets": false})), json!(["pointLight1"]));
    run(&mut s, "undo", json!({}));
    assert_eq!(lit(&mut s), json!(["box", "ball"]));
    // No mode and both ends makes a link: an exclusive light lights only what it's linked to.
    run(&mut s, "pointLight", json!({"args": [], "exclusive": true}));
    run(&mut s, "lightlink", json!({"light": ["pointLight2"], "object": ["box"]}));
    assert_eq!(run(&mut s, "lightlink", json!({"query": true, "light": "pointLight2", "shapes": false, "sets": false})), json!(["box"]));
}

const FILE: &str = r#"//Maya ASCII 2024 scene
requires maya "2024";
currentUnit -l centimeter -a degree -t film;
createNode transform -n "directionalLight1";
createNode directionalLight -n "directionalLightShape1" -p "directionalLight1";
	setAttr -k off ".v";
	setAttr ".in" 2;
	setAttr ".cl" -type "float3" 1 0.5 0.25 ;
createNode transform -n "spotLight1";
	setAttr ".t" -type "double3" 1 2 3 ;
createNode spotLight -n "spotLightShape1" -p "spotLight1";
	setAttr -k off ".v";
	setAttr ".de" 2;
	setAttr ".ca" 60;
	setAttr ".pa" -5;
	setAttr ".dro" 3;
createNode transform -n "ambientLight1";
createNode ambientLight -n "ambientLightShape1" -p "ambientLight1";
	setAttr -k off ".v";
	setAttr ".as" 0.2;
createNode transform -n "areaLight1";
createNode areaLight -n "areaLightShape1" -p "areaLight1";
	setAttr -k off ".v";
	setAttr ".esp" no;
createNode transform -n "pointLight1";
createNode pointLight -n "pointLightShape1" -p "pointLight1";
	setAttr -k off ".v";
createNode transform -n "volumeLight1";
createNode volumeLight -n "volumeLightShape1" -p "volumeLight1";
	setAttr -k off ".v";
	setAttr ".lis" 3;
	setAttr ".crg[0].crgp" 0;
	setAttr ".crg[0].crgc" -type "float3" 1 1 1 ;
createNode transform -n "box";
createNode mesh -n "boxShape" -p "box";
createNode polyCube -n "polyCube1";
createNode lightLinker -s -n "lightLinker1";
relationship "link" ":lightLinker1" ":initialShadingGroup.message" ":defaultLightSet.message";
relationship "ignore" ":lightLinker1" "box.message" "directionalLight1.message";
select -ne :lightList1;
	setAttr -s 6 ".l";
select -ne :defaultLightSet;
	setAttr -s 5 ".dsm";
connectAttr "polyCube1.out" "boxShape.i";
connectAttr "directionalLightShape1.ltd" ":lightList1.l" -na;
connectAttr "spotLightShape1.ltd" ":lightList1.l" -na;
connectAttr "ambientLightShape1.ltd" ":lightList1.l" -na;
connectAttr "areaLightShape1.ltd" ":lightList1.l" -na;
connectAttr "pointLightShape1.ltd" ":lightList1.l" -na;
connectAttr "volumeLightShape1.ltd" ":lightList1.l" -na;
connectAttr "directionalLight1.iog" ":defaultLightSet.dsm" -na;
connectAttr "spotLight1.iog" ":defaultLightSet.dsm" -na;
connectAttr "ambientLight1.iog" ":defaultLightSet.dsm" -na;
connectAttr "areaLight1.iog" ":defaultLightSet.dsm" -na;
connectAttr "volumeLight1.iog" ":defaultLightSet.dsm" -na;
"#;

fn check(s: &mut Session) {
    assert_eq!(get(s, "directionalLightShape1.intensity"), 2.0);
    assert_eq!(get(s, "directionalLightShape1.colorB"), 0.25);
    assert_eq!(get(s, "spotLight1.translateZ"), 3.0);
    assert_eq!(get(s, "spotLightShape1.decayRate"), 2);
    assert_eq!(get(s, "spotLightShape1.coneAngle"), 60.0);
    assert_eq!(get(s, "spotLightShape1.penumbraAngle"), -5.0);
    assert_eq!(get(s, "spotLightShape1.dropoff"), 3.0);
    assert_eq!(get(s, "ambientLightShape1.ambientShade"), 0.2);
    assert_eq!(get(s, "areaLightShape1.emitSpecular"), false);
    assert_eq!(get(s, "volumeLightShape1.lightShape"), 3);
    // pointLight1 was left out of the set.
    let set = run(s, "sets", json!({"args": ["defaultLightSet"], "query": true}));
    assert_eq!(set.as_array().unwrap().len(), 5, "{set}");
    assert!(!set.as_array().unwrap().iter().any(|x| x == "pointLight1"));
    assert_eq!(run(s, "lightlink", json!({"query": true, "light": "directionalLight1", "shapes": false, "sets": false})), json!([]));
    assert_eq!(run(s, "lightlink", json!({"query": true, "light": "spotLight1", "shapes": false, "sets": false})), json!(["box"]));
}

#[test]
fn a_file_with_every_light_reads_and_round_trips() {
    let mut s = session();
    let warnings = read_text(&mut s, FILE, false);
    assert!(warnings.iter().all(|w| !w.contains("ight")), "{warnings:?}");
    check(&mut s);
    let (text, warnings) = write_text(&mut s, "x.ma", None);
    assert!(warnings.iter().all(|w| !w.contains("ight")), "{warnings:?}");
    for line in [
        "createNode directionalLight -n \"directionalLightShape1\" -p \"directionalLight1\";",
        "setAttr \".in\" 2;",
        "setAttr \".cl\" -type \"float3\" 1 0.5 0.25 ;",
        "setAttr \".ca\" 60;",
        "connectAttr \"directionalLightShape1.ltd\" \":lightList1.l\" -na;",
        "connectAttr \"directionalLight1.iog\" \":defaultLightSet.dsm\" -na;",
        "relationship \"ignore\" \":lightLinker1\" \"box.message\" \"directionalLight1.message\";",
        "setAttr \".crg[0].crgp\" 0;",
    ] {
        assert!(text.contains(line), "{line}\n{text}");
    }
    assert!(!text.contains("pointLight1.iog"), "{text}");
    let mut again = session();
    let warnings = read_text(&mut again, &text, false);
    assert!(warnings.iter().all(|w| !w.contains("ight")), "{warnings:?}");
    check(&mut again);
    // The same text a second time: nothing was lost or doubled.
    let (second, _) = write_text(&mut again, "x.ma", None);
    assert_eq!(second.matches("lightList1.l").count(), text.matches("lightList1.l").count());
    assert_eq!(second.matches("setAttr \".crg[0].crgp\"").count(), 1);
}

#[test]
fn a_keyed_intensity_writes_with_its_connection() {
    let mut s = session();
    run(&mut s, "pointLight", json!({}));
    run(&mut s, "setKeyframe", json!({"args": ["pointLightShape1.intensity"], "time": 1.0, "value": 3.0}));
    assert_eq!(get(&mut s, "pointLightShape1.intensity"), 3.0);
    let (text, _) = write_text(&mut s, "x.ma", None);
    assert!(text.contains("pointLightShape1.in") || text.contains("pointLightShape1.intensity"), "{text}");
    let mut again = session();
    read_text(&mut again, &text, false);
    assert_eq!(get(&mut again, "pointLightShape1.intensity"), 3.0);
}
