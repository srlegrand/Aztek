//! Drawing overrides, templates, display layers and wireframe colours: read from .ma, written back, and how they decide
//! what an object draws like. Each hand-written scene stores its values the way the reference's files do.

use aztek_scene::display_layers as dl;
use aztek_scene::draw_style::{self as ds, DisplayKind, SelectState};
use aztek_scene::ma_file::{read_text, write_text};
use aztek_scene::plug::NodeId;
use aztek_scene::Session;
use serde_json::{json, Value};

const HEAD: &str = "//Maya ASCII 2020 scene\nrequires maya \"2020\";\ncurrentUnit -l centimeter -a degree -t film;\n";

const CURVE: &str = "\tsetAttr \".cc\" -type \"nurbsCurve\" \n\t\t1 1 0 no 3\n\t\t2 0 1\n\t\t2\n\t\t0 0 0\n\t\t1 0 0\n\t\t;\n";

fn load(body: &str) -> Session {
    let mut s = Session::new();
    let w = read_text(&mut s, &format!("{HEAD}{body}"), false);
    assert!(w.is_empty(), "{w:?}");
    s.graph.evaluate();
    s
}

fn curve(xf: &str, parent: Option<&str>, shape_attrs: &str) -> String {
    let p = parent.map_or(String::new(), |p| format!(" -p \"{p}\""));
    format!("createNode transform -n \"{xf}\"{p};\ncreateNode nurbsCurve -n \"{xf}Shape\" -p \"{xf}\";\n\tsetAttr -k off \".v\";\n{shape_attrs}{CURVE}")
}

fn id(s: &Session, n: &str) -> NodeId {
    s.graph.find(n).unwrap().unwrap_or_else(|| panic!("no {n}"))
}

fn get(s: &mut Session, plug: &str) -> Value {
    s.run("getAttr", json!({"args": [plug]})).unwrap_or_else(|e| panic!("{plug}: {e}"))
}

fn run(s: &mut Session, cmd: &str, args: Value) -> Value {
    s.run(cmd, args).unwrap_or_else(|e| panic!("{cmd}: {e}"))
}

fn rgb(s: &Session, n: &str) -> Option<[u8; 4]> {
    ds::style(&s.graph, id(s, n)).color
}

#[test]
fn a_curve_shape_keeps_its_override_from_the_file() {
    // Found on a production rig: the reader dropped these on shapes, so every control drew the default colour.
    let mut s = load(&curve("ctrl", None, "\tsetAttr \".ove\" yes;\n\tsetAttr \".ovc\" 13;\n"));
    assert_eq!(get(&mut s, "ctrlShape.ove"), json!(true));
    assert_eq!(get(&mut s, "ctrlShape.overrideEnabled"), json!(true));
    assert_eq!(get(&mut s, "ctrlShape.ovc"), json!(13));
    assert_eq!(rgb(&s, "ctrlShape"), Some([255, 0, 0, 255]));
}

#[test]
fn every_attribute_round_trips_on_transform_shape_joint_and_locator() {
    let attrs = "\tsetAttr \".ove\" yes;\n\tsetAttr \".ovdt\" 2;\n\tsetAttr \".ovlod\" 1;\n\tsetAttr \".ovs\" no;\n\tsetAttr \".ovt\" no;\n\tsetAttr \".ovp\" no;\n\tsetAttr \".ovv\" no;\n\tsetAttr \".hpb\" yes;\n\tsetAttr \".ovrgbf\" yes;\n\tsetAttr \".ovc\" 17;\n\tsetAttr \".ovrgb\" -type \"float3\" 0.25 0.5 0.75 ;\n\tsetAttr \".tmp\" yes;\n\tsetAttr \".uoc\" 2;\n\tsetAttr \".oc\" 3;\n\tsetAttr \".wfcc\" -type \"float3\" 0.25 0.5 1 ;\n\tsetAttr \".uocol\" yes;\n\tsetAttr \".oclr\" -type \"float3\" 1 0.5 0 ;\n";
    let body = format!(
        "createNode transform -n \"x\";\n{attrs}createNode nurbsCurve -n \"xShape\" -p \"x\";\n\tsetAttr -k off \".v\";\n{attrs}{CURVE}createNode joint -n \"j\";\n{attrs}createNode transform -n \"l\";\ncreateNode locator -n \"lShape\" -p \"l\";\n{attrs}"
    );
    let mut a = load(&body);
    let (text, w) = write_text(&mut a, "x.ma", None);
    assert!(w.is_empty(), "{w:?}");
    assert!(text.contains("setAttr \".ovrgb\" -type \"float3\" 0.25 0.5 0.75 ;"), "{text}");
    let mut b = Session::new();
    let w = read_text(&mut b, &text, false);
    assert!(w.is_empty(), "{w:?}");
    b.graph.evaluate();
    for mut s in [a, b] {
        for n in ["x", "xShape", "j", "lShape"] {
            for (attr, want) in [
                ("ove", json!(true)),
                ("overrideDisplayType", json!(2)),
                ("ovlod", json!(1)),
                ("ovs", json!(false)),
                ("ovt", json!(false)),
                ("ovp", json!(false)),
                ("ovv", json!(false)),
                ("hideOnPlayback", json!(true)),
                ("ovrgbf", json!(true)),
                ("ovc", json!(17)),
                ("ovcg", json!(0.5)),
                ("tmp", json!(true)),
                ("uoc", json!(2)),
                ("oc", json!(3)),
                ("wfcg", json!(0.5)),
                ("wireColorB", json!(1.0)),
                ("uocol", json!(true)),
                ("oclrg", json!(0.5)),
            ] {
                assert_eq!(get(&mut s, &format!("{n}.{attr}")), want, "{n}.{attr}");
            }
        }
    }
}

#[test]
fn surfaces_and_meshes_keep_overrides_too() {
    let mut s = Session::new();
    let cube = run(&mut s, "polyCube", json!({"n": "m"}));
    assert!(cube.is_array());
    run(&mut s, "setAttr", json!({"args": ["mShape.ove", 1]}));
    run(&mut s, "setAttr", json!({"args": ["mShape.ovc", 14]}));
    run(&mut s, "nurbsPlane", json!({"n": "p"}));
    run(&mut s, "setAttr", json!({"args": ["pShape.ove", 1]}));
    run(&mut s, "setAttr", json!({"args": ["pShape.ovc", 17]}));
    let (text, _) = write_text(&mut s, "x.ma", None);
    let mut b = Session::new();
    read_text(&mut b, &text, false);
    assert_eq!(get(&mut b, "mShape.ovc"), json!(14));
    assert_eq!(get(&mut b, "pShape.ovc"), json!(17));
}

#[test]
fn draw_override_reads_and_sets_as_a_compound() {
    // getAttr on .do failed with "has no attribute" though connections used the name.
    let mut s = load(&curve("c", None, ""));
    let v = get(&mut s, "c.do");
    assert_eq!(v.as_array().unwrap().len(), 12);
    assert_eq!(get(&mut s, "c.drawOverride"), v);
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["c.do"], "type": true})), json!("TdataCompound"));
    run(&mut s, "setAttr", json!({"args": ["c.ove", 1]}));
    assert_eq!(get(&mut s, "c.do").as_array().unwrap()[5], json!(true));
}

#[test]
fn nearer_overrides_win_and_a_group_colours_its_children() {
    let mut s = load(&format!(
        "createNode transform -n \"grp\";\n\tsetAttr \".ove\" yes;\n\tsetAttr \".ovc\" 17;\n{}{}{}",
        curve("a", Some("grp"), ""),
        curve("b", Some("grp"), "\tsetAttr \".ove\" yes;\n\tsetAttr \".ovc\" 13;\n"),
        curve("c", Some("grp"), "\tsetAttr \".ove\" yes;\n\tsetAttr \".ovc\" 0;\n"),
    ));
    assert_eq!(rgb(&s, "aShape"), Some([255, 255, 0, 255]), "inherited");
    assert_eq!(rgb(&s, "bShape"), Some([255, 0, 0, 255]), "own");
    assert_eq!(rgb(&s, "cShape"), Some([255, 255, 0, 255]), "index 0 is no colour");
    // Off, its values do nothing.
    run(&mut s, "setAttr", json!({"args": ["grp.ove", 0]}));
    assert_eq!(rgb(&s, "aShape"), None);
    // A shape beats its transform.
    run(&mut s, "setAttr", json!({"args": ["b.ove", 1]}));
    run(&mut s, "setAttr", json!({"args": ["b.ovc", 6]}));
    assert_eq!(rgb(&s, "bShape"), Some([255, 0, 0, 255]));
    run(&mut s, "setAttr", json!({"args": ["bShape.ove", 0]}));
    assert_eq!(rgb(&s, "bShape"), Some([0, 0, 255, 255]));
}

#[test]
fn rgb_override_and_the_palette_colorindex_edits() {
    let mut s = load(&curve("c", None, "\tsetAttr \".ove\" yes;\n\tsetAttr \".ovrgbf\" yes;\n\tsetAttr \".ovrgb\" -type \"float3\" 1 0.5 0 ;\n\tsetAttr \".ovc\" 13;\n"));
    assert_eq!(rgb(&s, "cShape"), Some([255, 128, 0, 255]));
    run(&mut s, "setAttr", json!({"args": ["cShape.ovrgbf", 0]}));
    assert_eq!(rgb(&s, "cShape"), Some([255, 0, 0, 255]));
    run(&mut s, "colorIndex", json!({"args": [13, 0.0, 1.0, 0.0]}));
    assert_eq!(rgb(&s, "cShape"), Some([0, 255, 0, 255]));
    run(&mut s, "colorIndex", json!({"resetToFactory": true}));
    assert_eq!(rgb(&s, "cShape"), Some([255, 0, 0, 255]));
}

#[test]
fn layers_store_an_index_and_drive_their_members() {
    // Layers in files hold a palette index in .c; the reader dropped it and the node held an RGB tint instead.
    let body = format!(
        "createNode transform -n \"g\";\n\tsetAttr \".ove\" yes;\n\tsetAttr \".ovc\" 6;\n{}{}createNode displayLayer -n \"L\";\n\tsetAttr \".c\" 13;\n\tsetAttr \".do\" 3;\nconnectAttr \"L.di\" \"g.do\";\n",
        curve("a", Some("g"), ""),
        curve("b", Some("g"), "\tsetAttr \".ove\" yes;\n\tsetAttr \".ovc\" 17;\n")
    );
    let mut s = load(&body);
    assert_eq!(get(&mut s, "L.c"), json!(13));
    assert_eq!(get(&mut s, "L.do"), json!(3));
    assert_eq!(dl::layer_of(&s.graph, id(&s, "g")), Some(id(&s, "L")));
    // The connection replaces the member's own override; a child's own still wins.
    assert_eq!(rgb(&s, "aShape"), Some([255, 0, 0, 255]));
    assert_eq!(rgb(&s, "bShape"), Some([255, 255, 0, 255]));
    assert_eq!(get(&mut s, "g.ovc"), json!(13));
    assert!(s.run("setAttr", json!({"args": ["g.ovc", 4]})).is_err());
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["g.ovc"], "settable": true})), json!(false));
    assert_eq!(get(&mut s, "g.do").as_array().unwrap().len(), 12);
    assert_eq!(get(&mut s, "L.di").as_array().unwrap().len(), 12);

    // RGB in place of the index, enabled off, and out of the layer.
    run(&mut s, "setAttr", json!({"args": ["L.ovrgbf", 1]}));
    run(&mut s, "setAttr", json!({"args": ["L.ovcg", 1.0]}));
    assert_eq!(rgb(&s, "aShape"), Some([0, 255, 0, 255]));
    run(&mut s, "setAttr", json!({"args": ["L.e", 0]}));
    assert_eq!(rgb(&s, "aShape"), None);
    run(&mut s, "removeFromDisplayLayer", json!({"args": ["g"]}));
    assert_eq!(rgb(&s, "aShape"), Some([0, 0, 255, 255]));
}

#[test]
fn layer_settings_round_trip_and_a_layer_on_a_shape_counts() {
    let body = format!(
        "{}createNode displayLayer -n \"L\";\n\tsetAttr \".v\" no;\n\tsetAttr \".dt\" 2;\n\tsetAttr \".lod\" 1;\n\tsetAttr \".s\" no;\n\tsetAttr \".t\" no;\n\tsetAttr \".p\" no;\n\tsetAttr \".hpb\" yes;\n\tsetAttr \".c\" 21;\n\tsetAttr \".ovrgbf\" yes;\n\tsetAttr \".ovrgb\" -type \"float3\" 0.5 0.25 1 ;\n\tsetAttr \".do\" 2;\nconnectAttr \"L.di\" \"cShape.do\";\n",
        curve("c", None, "")
    );
    let mut a = load(&body);
    let (text, w) = write_text(&mut a, "x.ma", None);
    assert!(w.is_empty(), "{w:?}");
    assert!(text.contains("connectAttr \"L.di\" \"") && text.contains("cShape.do\";"), "{text}");
    let mut b = Session::new();
    assert!(read_text(&mut b, &text, false).is_empty());
    for mut s in [a, b] {
        assert_eq!(dl::layer_of(&s.graph, id(&s, "cShape")), Some(id(&s, "L")));
        assert_eq!(get(&mut s, "L.dt"), json!(2));
        assert_eq!(get(&mut s, "L.lod"), json!(1));
        for off in ["L.v", "L.s", "L.t", "L.p"] {
            assert_eq!(get(&mut s, off), json!(false), "{off}");
        }
        assert_eq!(get(&mut s, "L.hpb"), json!(true));
        assert_eq!(get(&mut s, "L.c"), json!(21));
        assert_eq!(get(&mut s, "L.ovrgbf"), json!(true));
        assert_eq!(get(&mut s, "L.ovcb"), json!(1.0));
    }
}

#[test]
fn template_and_reference_reach_children_and_refuse_picks() {
    let mut s = load(&format!(
        "createNode transform -n \"g\";\n\tsetAttr \".tmp\" yes;\n{}{}{}",
        curve("a", Some("g"), ""),
        curve("r", None, "\tsetAttr \".ove\" yes;\n\tsetAttr \".ovdt\" 2;\n\tsetAttr \".ovc\" 13;\n"),
        curve("n", None, ""),
    ));
    assert_eq!(ds::display_kind(&s.graph, id(&s, "aShape")), DisplayKind::Template);
    assert!(!ds::pickable(&s.graph, id(&s, "aShape")));
    assert_eq!(ds::display_kind(&s.graph, id(&s, "rShape")), DisplayKind::Reference);
    assert!(!ds::pickable(&s.graph, id(&s, "rShape")));
    assert!(ds::pickable(&s.graph, id(&s, "nShape")));
    assert_eq!(rgb(&s, "aShape"), Some([153, 153, 153, 255]));
    assert_eq!(rgb(&s, "rShape"), Some([0, 0, 0, 255]), "reference is black, over its colour");
    assert!(!ds::style(&s.graph, id(&s, "aShape")).shaded);
    assert!(ds::style(&s.graph, id(&s, "rShape")).shaded);
    // Lists and scripts still select them.
    run(&mut s, "select", json!({"args": ["a"]}));
    assert_eq!(s.graph.selection(), &[id(&s, "a")]);
    let sel = s.graph.selection().to_vec();
    let wire = ds::wire_color(&s.graph, id(&s, "aShape"), &sel, "curve", [1, 2, 3, 255]);
    assert_ne!(wire, [153, 153, 153, 255], "a selected template shows differently");
    assert_ne!(wire, ds::LEAD);
}

#[test]
fn level_of_detail_shading_texturing_visibility_and_playback() {
    let mut s = load(&format!(
        "createNode transform -n \"g\";\n\tsetAttr \".ove\" yes;\n\tsetAttr \".ovlod\" 1;\n\tsetAttr \".ovs\" no;\n\tsetAttr \".ovt\" no;\n{}{}",
        curve("a", Some("g"), ""),
        curve("b", None, "\tsetAttr \".ove\" yes;\n\tsetAttr \".hpb\" yes;\n"),
    ));
    let st = ds::style(&s.graph, id(&s, "aShape"));
    assert!(st.bounding_box && !st.shaded && !st.textured);
    let vis = |s: &Session, n: &str| aztek_scene::transform::effectively_visible(&s.graph, id(s, n));
    assert!(vis(&s, "bShape"));
    run(&mut s, "setAttr", json!({"args": ["g.ovv", 0]}));
    assert!(!vis(&s, "aShape"), "hidden by the group's override");
    // Not enabled, nothing happens.
    run(&mut s, "setAttr", json!({"args": ["g.ove", 0]}));
    assert!(vis(&s, "aShape"));
    ds::set_play_direction(1);
    let during = vis(&s, "bShape");
    ds::set_play_direction(0);
    assert!(!during);
    assert!(vis(&s, "bShape"));
}

#[test]
fn object_colour_and_the_color_command() {
    let mut s = load(&format!("{}createNode joint -n \"j\";\n\tsetAttr \".uoc\" 1;\n\tsetAttr \".oc\" 2;\n", curve("c", None, "")));
    // A joint's stored object colour: user colour 3.
    assert_eq!(rgb(&s, "j"), Some(ds::palette_bytes(ds::USER_COLOR_BASE + 2)));
    run(&mut s, "color", json!({"args": ["c"], "userDefined": 2}));
    assert_eq!(get(&mut s, "cShape.uoc"), json!(1));
    assert_eq!(get(&mut s, "cShape.oc"), json!(1));
    assert_eq!(rgb(&s, "cShape"), Some(ds::palette_bytes(ds::USER_COLOR_BASE + 1)));
    run(&mut s, "color", json!({"args": ["c"], "rgbColor": [1.0, 0.0, 0.0]}));
    assert_eq!(get(&mut s, "cShape.uoc"), json!(2));
    assert_eq!(rgb(&s, "cShape"), Some([255, 0, 0, 255]));
    // An override beats it.
    run(&mut s, "setAttr", json!({"args": ["cShape.ove", 1]}));
    run(&mut s, "setAttr", json!({"args": ["cShape.ovc", 6]}));
    assert_eq!(rgb(&s, "cShape"), Some([0, 0, 255, 255]));
    run(&mut s, "setAttr", json!({"args": ["cShape.ove", 0]}));
    run(&mut s, "color", json!({"args": ["c"]}));
    assert_eq!(rgb(&s, "cShape"), None);
    run(&mut s, "undo", json!({}));
    assert_eq!(rgb(&s, "cShape"), Some([255, 0, 0, 255]));
    assert!(s.run("color", json!({"args": ["c"], "userDefined": 9})).is_err());
}

#[test]
fn display_colours_are_the_defaults_when_set() {
    let mut s = load(&curve("c", None, ""));
    let factory = [10, 10, 110, 255];
    let c = id(&s, "cShape");
    assert_eq!(ds::wire_color(&s.graph, c, &[], "curve", factory), factory);
    run(&mut s, "displayColor", json!({"args": ["curve", 13]}));
    assert_eq!(ds::wire_color(&s.graph, c, &[], "curve", factory), [255, 0, 0, 255]);
    run(&mut s, "displayRGBColor", json!({"args": ["lead", 0.0, 0.0, 1.0]}));
    assert_eq!(ds::wire_color(&s.graph, c, &[id(&s, "c")], "curve", factory), [0, 0, 255, 255]);
}

#[test]
fn selected_colours_win_and_children_highlight() {
    let s = load(&format!(
        "createNode transform -n \"g\";\n{}{}",
        curve("a", Some("g"), "\tsetAttr \".ove\" yes;\n\tsetAttr \".ovc\" 13;\n"),
        curve("b", Some("g"), "\tsetAttr \".sech\" no;\n"),
    ));
    let (g, a, xa, b) = (id(&s, "g"), id(&s, "aShape"), id(&s, "a"), id(&s, "bShape"));
    let f = [1, 2, 3, 255];
    assert_eq!(ds::wire_color(&s.graph, a, &[], "curve", f), [255, 0, 0, 255]);
    assert_eq!(ds::wire_color(&s.graph, a, &[xa], "curve", f), ds::LEAD);
    assert_eq!(ds::wire_color(&s.graph, a, &[xa, g], "curve", f), ds::SELECTED);
    assert_eq!(ds::select_state(&s.graph, a, &[g]), SelectState::Child);
    assert_eq!(ds::wire_color(&s.graph, a, &[g], "curve", f), ds::CHILD_HIGHLIGHT);
    assert_eq!(ds::select_state(&s.graph, b, &[g]), SelectState::None, "selectionChildHighlighting off");
}

#[test]
fn a_connection_can_drive_the_display_type() {
    // A rig's enum on a control commonly drives its geometry's display type.
    let mut s = load(&format!(
        "createNode transform -n \"ctl\";\n\taddAttr -ci true -k true -sn \"geo\" -ln \"geo\" -min 0 -max 2 -en \"Normal:Template:Reference\" -at \"enum\";\n\tsetAttr -k on \".geo\" 2;\n{}connectAttr \"ctl.geo\" \"gShape.ovdt\";\n",
        curve("g", None, "\tsetAttr \".ove\" yes;\n")
    ));
    assert_eq!(get(&mut s, "gShape.ovdt"), json!(2));
    assert_eq!(ds::display_kind(&s.graph, id(&s, "gShape")), DisplayKind::Reference);
    let (text, _) = write_text(&mut s, "x.ma", None);
    assert!(text.contains("connectAttr \"ctl.geo\" \"gShape.ovdt\";"), "{text}");
}
