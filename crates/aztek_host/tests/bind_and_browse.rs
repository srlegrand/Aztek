//! Issues 29 to 32: what a bind does to the bound object, what the Outliner and the Hypershade are told, which
//! attributes take a texture, and Harold's lights in the view.

use aztek_host::{query, Core};
use serde_json::{json, Value};

fn find<'a>(nodes: &'a Value, name: &str) -> Option<&'a Value> {
    for n in nodes.as_array()? {
        if n["name"] == name {
            return Some(n);
        }
        if let Some(hit) = find(&n["children"], name) {
            return Some(hit);
        }
    }
    None
}

// Issue 32: binding left the mesh free to move, a second transform on top of the joints'.
#[test]
fn a_bind_locks_the_bound_objects_transform_and_unbinding_frees_it() {
    let mut c = Core::new();
    c.mel("polySphere -n ball; select -cl; joint -n a -p 0 -1 0; joint -n b -p 0 1 0;");
    let locked = |c: &mut Core| -> Vec<bool> { "tx ty tz rx ry rz sx sy sz".split(' ').map(|a| c.mel(&format!("getAttr -lock ball.{a}")) == json!(true)).collect() };
    assert!(locked(&mut c).iter().all(|l| !l));
    c.mel("skinCluster a b ball;");
    assert!(locked(&mut c).iter().all(|l| *l), "translate, rotate and scale are locked");
    assert_eq!(c.mel("getAttr -lock ball.visibility"), json!(false));
    assert!(c.mel_checked("setAttr ball.tx 3").is_err(), "a locked channel refuses a value");
    // The joints still move it.
    c.mel("setAttr b.tx 2;");
    // The original shape the skin reads is there under the transform, by the reference's name.
    assert_eq!(c.mel("listRelatives -shapes ball"), json!(["ballShape", "ballShapeOrig"]));
    assert_eq!(c.mel("getAttr ballShapeOrig.intermediateObject"), json!(true));

    c.mel("undo; undo;");
    assert!(locked(&mut c).iter().all(|l| !l), "undoing the bind undoes the locks");
    c.mel("redo; redo;");
    assert!(locked(&mut c).iter().all(|l| *l));
    c.mel("skinCluster -e -unbind ball;");
    assert!(locked(&mut c).iter().all(|l| !l), "unbound, it moves again");
    c.mel("setAttr ball.tx 3;");
}

// Issue 32: the original shape could not be found in the Outliner with Shapes on.
#[test]
fn the_outliner_lists_the_original_shape_with_shapes_on() {
    let mut c = Core::new();
    c.mel("polySphere -n ball; select -cl; joint -n a; skinCluster a ball;");
    let plain = query::outliner(&c, "");
    assert!(find(&plain, "ball").is_some() && find(&plain, "ballShapeOrig").is_none());
    let shapes = query::outliner(&c, "shapes");
    let ball = find(&shapes, "ball").unwrap();
    let kids: Vec<&str> = ball["children"].as_array().unwrap().iter().map(|k| k["name"].as_str().unwrap()).collect();
    assert_eq!(kids, ["ballShape", "ballShapeOrig"]);
    assert_eq!(find(&shapes, "ballShapeOrig").unwrap()["intermediate"], json!(true));
}

fn attr<'a>(ae: &'a Value, tab: &str, name: &str) -> &'a Value {
    let t = ae["tabs"].as_array().unwrap().iter().find(|t| t["name"] == tab).unwrap_or_else(|| panic!("no tab {tab}: {ae}"));
    t["attrs"].as_array().unwrap().iter().find(|a| a["attr"] == name).unwrap_or_else(|| panic!("no {name} on {tab}"))
}

// Issue 29: only colours had a map button. Every number and colour a material, texture or light takes can be driven.
#[test]
fn numbers_and_colours_of_render_nodes_take_textures() {
    let mut c = Core::new();
    c.mel("shadingNode -asShader standardSurface -n mat; shadingNode -asTexture file -n tex; shadingNode -asLight pointLight -n lamp; polyCube -n box;");
    let ae = query::attribute_editor(&c, "mat");
    for a in ["base", "baseColor", "diffuseRoughness", "metalness", "specular", "specularRoughness", "specularIOR", "transmission", "coat", "emission", "opacity"] {
        assert_eq!(attr(&ae, "mat", a)["mappable"], json!(true), "{a}");
    }
    assert_eq!(attr(&ae, "mat", "thinWalled")["mappable"], json!(false), "a switch takes no texture");
    let lamp = query::attribute_editor(&c, "lampShape");
    assert_eq!(attr(&lamp, "lampShape", "intensity")["mappable"], json!(true));
    assert_eq!(attr(&lamp, "lampShape", "color")["mappable"], json!(true));
    let tex = query::attribute_editor(&c, "tex");
    assert_eq!(attr(&tex, "tex", "colorGain")["mappable"], json!(true));
    let xf = query::attribute_editor(&c, "box");
    assert_eq!(attr(&xf, "box", "translate")["mappable"], json!(false), "a transform is not a render node");

    // A number takes the texture's alpha, a colour its colour; the row then says what drives it.
    c.mel("defaultNavigation -connectToExisting -source tex -destination mat.metalness; defaultNavigation -connectToExisting -source tex -destination mat.baseColor;");
    assert_eq!(c.mel("listConnections -p 1 mat.metalness"), json!(["tex.outAlpha"]));
    assert_eq!(c.mel("listConnections -p 1 mat.baseColor"), json!(["tex.outColor"]));
    let ae = query::attribute_editor(&c, "mat");
    assert_eq!(attr(&ae, "mat", "metalness")["source"], json!("tex"));
    assert_eq!(attr(&ae, "mat", "metalness")["connected"], json!(true));
    // The slider's range is the soft one; the attribute's own limits come with it.
    assert_eq!(attr(&ae, "mat", "specularRoughness")["min"], json!(0.0));
    assert_eq!(attr(&ae, "mat", "specularRoughness")["max"], json!(1.0));
    assert!(attr(&ae, "mat", "specularRoughness").get("hardMax").is_some());
}

// Issue 28: the Browser's tabs.
#[test]
fn the_hypershade_is_told_of_materials_lights_cameras_and_shading_groups() {
    let mut c = Core::new();
    c.mel("shadingNode -asShader blinn -n shiny; sets -renderable true -noSurfaceShader true -empty -name shinySG; connectAttr shiny.outColor shinySG.surfaceShader;");
    c.mel("shadingNode -asLight spotLight -n key; shadingNode -asTexture file -n tex; shadingNode -asUtility place2dTexture -n place;");
    let all = query::shading(&mut c);
    let names = |key: &str| -> Vec<String> { all[key].as_array().unwrap().iter().map(|n| n["name"].as_str().unwrap().to_string()).collect() };
    assert!(names("materials").contains(&"shiny".to_string()) && names("materials").contains(&"lambert1".to_string()));
    assert!(names("lights").contains(&"keyShape".to_string()), "{:?}", names("lights"));
    assert!(names("cameras").contains(&"perspShape".to_string()));
    assert!(names("groups").contains(&"shinySG".to_string()) && names("groups").contains(&"initialShadingGroup".to_string()));
    assert_eq!(names("textures"), ["tex"]);
    assert!(names("utilities").contains(&"place".to_string()));
    // Every kind the Create panel offers can be asked for by its classification.
    for (class, has) in [("shader/surface", "standardSurface"), ("texture/2d", "file"), ("light", "aiSkyDomeLight"), ("utility", "place2dTexture")] {
        let types = c.mel(&format!("listNodeTypes \"{class}\""));
        assert!(types.as_array().unwrap().contains(&json!(has)), "{class}: {types}");
    }
}

// Issues 30 and 31: Harold's lights had the other renderer's name and nothing to see or pick in the view.
#[test]
fn harolds_lights_are_named_as_harolds_and_have_a_shape_in_the_view() {
    let mut c = Core::new();
    for (ty, name) in [("aiSkyDomeLight", "haroldSkyDomeLight1"), ("aiAreaLight", "haroldAreaLight1"), ("aiPhotometricLight", "haroldPhotometricLight1"), ("aiMeshLight", "haroldMeshLight1")] {
        assert_eq!(c.mel(&format!("shadingNode -asLight {ty}")), json!(name));
        // The node type keeps the name scene files use.
        let shape = format!("{}Shape1", name.trim_end_matches('1'));
        assert_eq!(c.mel(&format!("nodeType {shape}")), json!(ty), "the shape is {shape}");
        let id = c.s.graph.find(&shape).unwrap().unwrap();
        c.s.graph.evaluate();
        let w = aztek_scene::rig::world(&mut c.s.graph, id);
        let lines = aztek_scene::lights::icon_lines(c.s.graph.node(id).unwrap(), &w);
        assert!(lines.len() >= 9, "{ty} draws {} lines", lines.len());
        assert!(lines.iter().all(|(a, b)| a.is_finite() && b.is_finite() && a != b));
    }
    // The area light is its real size: a quad two units across, a disc of radius one, a cylinder two tall.
    let reach = |c: &mut Core| -> [f64; 3] {
        let id = c.s.graph.find("haroldAreaLightShape1").unwrap().unwrap();
        c.s.graph.evaluate();
        let w = aztek_scene::rig::world(&mut c.s.graph, id);
        let mut hi = [0.0f64; 3];
        for (a, b) in aztek_scene::lights::icon_lines(c.s.graph.node(id).unwrap(), &w) {
            for p in [a, b] {
                for k in 0..3 {
                    hi[k] = hi[k].max(p[k].abs());
                }
            }
        }
        hi
    };
    c.mel("setAttr haroldAreaLight1.scale 3 2 1;");
    let quad = reach(&mut c);
    assert!((quad[0] - 3.0).abs() < 1e-9 && (quad[1] - 2.0).abs() < 1e-9, "{quad:?}");
    c.mel("setAttr haroldAreaLightShape1.aiTranslator 2; setAttr haroldAreaLight1.scale 1 5 1;");
    let cylinder = reach(&mut c);
    assert!((cylinder[1] - 5.0).abs() < 1e-9 && (cylinder[0] - 1.0).abs() < 1e-9 && (cylinder[2] - 1.0).abs() < 1e-9, "{cylinder:?}");
    // The dome is a sphere of its Sky Radius.
    let id = c.s.graph.find("haroldSkyDomeLightShape1").unwrap().unwrap();
    c.mel("setAttr haroldSkyDomeLightShape1.skyRadius 50;");
    c.s.graph.evaluate();
    let w = aztek_scene::rig::world(&mut c.s.graph, id);
    let far = aztek_scene::lights::icon_lines(c.s.graph.node(id).unwrap(), &w).iter().map(|(a, _)| a.length()).fold(0.0, f64::max);
    assert!((far - 50.0).abs() < 1e-6, "{far}");
}
