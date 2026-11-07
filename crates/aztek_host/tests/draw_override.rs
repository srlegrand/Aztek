//! Colours and display types as scripts and the panels meet them: MEL spellings, the panels' JSON, Template in the menu.

use aztek_host::query;
use aztek_host::Core;
use serde_json::{json, Value};

fn errors(c: &mut Core) -> Vec<String> {
    c.take_log().as_array().unwrap().iter().filter(|l| l["kind"] == 2).map(|l| l["text"].as_str().unwrap().to_string()).collect()
}

fn find<'a>(nodes: &'a Value, name: &str) -> Option<&'a Value> {
    for n in nodes.as_array()? {
        if n["name"] == name {
            return Some(n);
        }
        if let Some(k) = find(&n["children"], name) {
            return Some(k);
        }
    }
    None
}

#[test]
fn the_color_command_from_mel() {
    let mut c = Core::new();
    c.mel("circle -n \"a\";");
    c.mel("color -ud 3 a;");
    assert_eq!(c.mel("getAttr aShape.uoc;"), json!(1));
    assert_eq!(c.mel("getAttr aShape.oc;"), json!(2));
    c.mel("color -rgb 1 0.5 0 a;");
    assert_eq!(c.mel("getAttr aShape.uoc;"), json!(2));
    assert_eq!(c.mel("getAttr aShape.wireColorG;"), json!(0.5));
    c.mel("select a; color;");
    assert_eq!(c.mel("getAttr aShape.uoc;"), json!(0));
    assert_eq!(errors(&mut c), Vec::<String>::new());
}

#[test]
fn draw_override_by_name_and_a_layer_through_connectattr() {
    let mut c = Core::new();
    c.mel("circle -n \"a\";");
    // -e is createDisplayLayer's empty, not edit mode.
    c.mel("select a; createDisplayLayer -e -n \"L\";");
    assert_eq!(c.mel("editDisplayLayerMembers L a;"), json!(["a"]));
    c.mel("setAttr L.color 13;");
    assert_eq!(c.mel("getAttr a.overrideEnabled;"), json!(true));
    assert_eq!(c.mel("getAttr a.overrideColor;"), json!(13));
    assert_eq!(c.mel("listConnections -s 1 -d 0 a.drawOverride;"), json!(["L"]));
    c.mel("disconnectAttr L.drawInfo a.drawOverride;");
    assert_eq!(c.mel("getAttr a.overrideEnabled;"), json!(false));
    c.mel("connectAttr L.drawInfo a.drawOverride;");
    assert_eq!(c.mel("getAttr a.overrideColor;"), json!(13));
    assert_eq!(errors(&mut c), Vec::<String>::new());
}

#[test]
fn the_panels_see_layer_colours_and_outliner_colours() {
    let mut c = Core::new();
    c.mel("circle -n \"a\"; circle -n \"b\"; circle -n \"t\";");
    c.mel("setAttr a.useOutlinerColor 1; setAttr a.outlinerColor -type \"float3\" 1 0 0;");
    c.mel("setAttr tShape.ove 1; setAttr tShape.ovdt 1;");
    c.mel("createDisplayLayer -e -n \"L\"; setAttr L.color 13;");
    let layers = query::layers(&c);
    assert_eq!(layers[0]["colorIndex"], json!(13));
    assert_eq!(layers[0]["color"], json!([1.0, 0.0, 0.0]));
    assert_eq!(layers[0]["useRgb"], json!(false));
    let out = query::outliner(&c, "");
    let list = if out.is_array() { out.clone() } else { out["nodes"].clone() };
    assert_eq!(find(&list, "a").unwrap()["outlinerColor"], json!([1.0, 0.0, 0.0]));
    assert!(find(&list, "b").unwrap()["outlinerColor"].is_null());
    assert_eq!(find(&list, "b").unwrap()["display"], json!(0));
    assert_eq!(find(&list, "t").unwrap()["display"], json!(1), "a templated shape shows on its transform");
}

#[test]
fn template_from_the_menu_sets_the_attribute() {
    // It set overrideDisplayType, leaving .template off and a saved file saying something else.
    let mut c = Core::new();
    c.mel("circle -n \"a\";");
    c.mel("select a; toggle -template -state on;");
    assert_eq!(c.mel("getAttr a.template;"), json!(true));
    assert_eq!(c.mel("getAttr a.overrideEnabled;"), json!(false));
    c.mel("toggle -template -state off a;");
    assert_eq!(c.mel("getAttr a.template;"), json!(false));
    assert_eq!(errors(&mut c), Vec::<String>::new());
}

#[test]
fn the_viewport_draws_and_refuses_as_the_decision_says() {
    use aztek_host::viewport::rig_items;
    let mut c = Core::new();
    c.mel("circle -n \"plain\"; circle -n \"tmp\"; circle -n \"ref\"; circle -n \"red\";");
    c.mel("setAttr tmp.tmp 1; setAttr refShape.ove 1; setAttr refShape.ovdt 2; setAttr redShape.ove 1; setAttr redShape.ovc 13;");
    c.mel("select -cl;");
    let g = &mut c.s.graph;
    g.evaluate();
    let items = rig_items(g);
    let item = |n: &str| {
        let id = g.find(n).unwrap().unwrap();
        items.iter().find(|i| i.node == id).unwrap()
    };
    assert_eq!(item("plain").color, [10, 10, 110, 255]);
    assert_eq!(item("red").color, [255, 0, 0, 255]);
    assert_eq!(item("ref").color, [0, 0, 0, 255]);
    assert_eq!(item("tmp").color, [153, 153, 153, 255]);
    assert!(aztek_scene::draw_style::pickable(g, item("plain").shape));
    assert!(!aztek_scene::draw_style::pickable(g, item("tmp").shape));
    assert!(!aztek_scene::draw_style::pickable(g, item("ref").shape));
    // The Outliner's way in still selects them.
    c.mel("select tmp ref;");
    assert_eq!(c.mel("ls -sl;"), json!(["tmp", "ref"]));
}

#[test]
fn bounding_box_detail_replaces_a_curve_with_a_box() {
    use aztek_host::viewport::rig_items;
    let mut c = Core::new();
    c.mel("circle -n \"a\"; setAttr aShape.ove 1; setAttr aShape.ovlod 1; select -cl;");
    let g = &mut c.s.graph;
    g.evaluate();
    let items = rig_items(g);
    let id = g.find("a").unwrap().unwrap();
    assert_eq!(items.iter().find(|i| i.node == id).unwrap().segs.len(), 12);
}

#[test]
fn playing_hides_what_hides_on_playback() {
    let mut c = Core::new();
    c.mel("circle -n \"a\"; setAttr aShape.ove 1; setAttr aShape.hideOnPlayback 1;");
    let shape = c.s.graph.find("aShape").unwrap().unwrap();
    assert!(!aztek_scene::draw_style::hidden_by_override(&c.s.graph, shape));
    c.mel("play -st 1;");
    let hidden = aztek_scene::draw_style::hidden_by_override(&c.s.graph, shape);
    c.mel("play -st 0;");
    assert!(hidden);
    assert!(!aztek_scene::draw_style::hidden_by_override(&c.s.graph, shape));
}
