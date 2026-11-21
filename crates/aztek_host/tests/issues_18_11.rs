//! Issues of 18/11/2026.

use aztek_host::Core;
use serde_json::{json, Value};

fn mel(c: &mut Core, src: &str) -> Value {
    c.mel_checked(src).unwrap_or_else(|e| panic!("{src}: {e}"))
}

/// #141: the tangent types the reference's API and keyTangent know, auto mix, ease and custom among them.
#[test]
fn tangent_types_of_the_reference() {
    let mut c = Core::new();
    mel(&mut c, "polyCube -n k; setKeyframe -t 1 -v 0 k.tx; setKeyframe -t 10 -v 5 k.tx; setKeyframe -t 20 -v 1 k.tx;");
    for t in ["autoease", "automix", "autocustom", "slow", "fast"] {
        mel(&mut c, &format!("keyTangent -e -t 10 -itt {t} -ott {t} k.tx;"));
        assert_eq!(mel(&mut c, "keyTangent -q -t 10 -ott k.tx;"), json!([t]));
    }
    for (name, v) in [("kTangentAuto", 11), ("kTangentAutoMix", 12), ("kTangentAutoEase", 13), ("kTangentAutoCustom", 14)] {
        mel(&mut c, "python(\"import maya.OpenMayaAnim as oma; import maya.api.OpenMayaAnim as oma2\");");
        assert_eq!(mel(&mut c, &format!("python(\"oma.MFnAnimCurve.{name}\");")), json!(v), "API 1.0 {name}");
        assert_eq!(mel(&mut c, &format!("python(\"oma2.MFnAnimCurve.{name}\");")), json!(v), "API 2.0 {name}");
    }
    // The API reads and sets them by name.
    mel(&mut c, "keyTangent -e -t 10 -ott autoease k.tx;");
    mel(&mut c, "python(\"import maya.api.OpenMaya as om2; sl = om2.MSelectionList(); sl.add('k_translateX'); crv = oma2.MFnAnimCurve(sl.getDependNode(0))\");");
    assert_eq!(mel(&mut c, "python(\"crv.outTangentType(1)\");"), json!(13));
}

fn points(c: &mut Core, shape: &str) -> Vec<[f64; 3]> {
    let v = mel(c, &format!("xform -q -ws -t \"{shape}.vtx[*]\";"));
    v.as_array().unwrap().chunks(3).map(|p| [p[0].as_f64().unwrap(), p[1].as_f64().unwrap(), p[2].as_f64().unwrap()]).collect()
}

/// #143: the reference's attributes, a texture connected to .texture moving each point by its own colour, the handle
/// giving the direction, and strength and offset as the reference applies them.
#[test]
fn texture_deformer_samples_its_texture() {
    let mut c = Core::new();
    mel(&mut c, "polyPlane -n p -w 4 -h 4 -sx 8 -sy 8;");
    let made = mel(&mut c, "textureDeformer p;");
    assert_eq!(made, json!(["textureDeformer1", "textureDeformerHandle1"]));
    assert_eq!(mel(&mut c, "getAttr textureDeformer1.pointSpace;"), json!(2));
    assert_eq!(mel(&mut c, "getAttr textureDeformer1.direction;"), json!(1));
    assert_eq!(mel(&mut c, "listConnections -s 1 -d 0 textureDeformer1.handleMatrix;"), json!(["textureDeformerHandle1"]));
    // Nothing connected and a black texture: no move.
    let flat = points(&mut c, "p");
    assert!(flat.iter().all(|q| q[1].abs() < 1e-9));
    // A checker: white squares go up by strength, black stay; offset -strength/2 centres them on the plane.
    mel(&mut c, "shadingNode -asTexture checker -n chk; connectAttr chk.outColor textureDeformer1.texture;");
    mel(&mut c, "setAttr textureDeformer1.strength 0.16;");
    let ys: Vec<f64> = points(&mut c, "p").iter().map(|q| q[1]).collect();
    assert!(ys.iter().any(|y| (y - 0.16).abs() < 1e-3), "{ys:?}");
    assert!(ys.iter().any(|y| y.abs() < 1e-3), "{ys:?}");
    assert!(ys.iter().all(|y| *y > -1e-6 && *y < 0.16 + 1e-6), "{ys:?}");
    mel(&mut c, "setAttr textureDeformer1.offset -0.08;");
    let ys: Vec<f64> = points(&mut c, "p").iter().map(|q| q[1]).collect();
    assert!(ys.iter().all(|y| (y.abs() - 0.08).abs() < 1e-3), "{ys:?}");
    // The texture's own settings reach the points.
    mel(&mut c, "setAttr chk.color1 -type double3 0 0 0;");
    let ys: Vec<f64> = points(&mut c, "p").iter().map(|q| q[1]).collect();
    assert!(ys.iter().all(|y| (y + 0.08).abs() < 1e-3), "{ys:?}");
    // Strength is uncapped, and the handle's up axis is the direction.
    mel(&mut c, "setAttr chk.color1 -type double3 1 1 1; setAttr textureDeformer1.strength 16; setAttr textureDeformer1.offset 0; rotate 0 0 90 textureDeformerHandle1;");
    let pts = points(&mut c, "p");
    assert!(pts.iter().any(|q| q[0] < -10.0), "{pts:?}");
    assert!(pts.iter().all(|q| q[1].abs() < 1e-6));
    // Normal direction on a plane is up again.
    mel(&mut c, "setAttr textureDeformer1.direction 0;");
    assert!(points(&mut c, "p").iter().any(|q| (q[1] - 16.0).abs() < 1e-3));
    // The texture row of the Attribute Editor takes a texture through its map button.
    let ae = aztek_host::query::attribute_editor(&c, "textureDeformer1")["tabs"][0].clone();
    let tex = ae["attrs"].as_array().unwrap().iter().find(|a| a["attr"] == "texture").cloned().unwrap_or_else(|| panic!("{ae}"));
    assert_eq!(tex["color"], json!(true));
    assert_eq!(tex["mappable"], json!(true));
    for a in ["strength", "offset"] {
        let row = ae["attrs"].as_array().unwrap().iter().find(|x| x["attr"] == a).unwrap().clone();
        assert_eq!(row["hardMin"], Value::Null, "{a}");
        assert_eq!(row["hardMax"], Value::Null, "{a}");
    }
}

/// #143: a texture deformer, its handle and its texture come back from a saved file.
#[test]
fn texture_deformer_saves() {
    let mut c = Core::new();
    let d = std::env::temp_dir().join("aztek_issue143");
    let _ = std::fs::create_dir_all(&d);
    let f = d.join("td.ma");
    mel(&mut c, "polyPlane -n p -w 4 -h 4 -sx 8 -sy 8; textureDeformer p; shadingNode -asTexture checker -n chk; connectAttr chk.outColor textureDeformer1.texture; setAttr textureDeformer1.strength 0.5;");
    let before = points(&mut c, "p");
    mel(&mut c, &format!("file -rename \"{}\"; file -save -type \"mayaAscii\";", f.display().to_string().replace('\\', "/")));
    mel(&mut c, &format!("file -f -new; file -o \"{}\";", f.display().to_string().replace('\\', "/")));
    assert_eq!(mel(&mut c, "listConnections -s 1 -d 0 textureDeformer1.handleMatrix;"), json!(["textureDeformerHandle1"]));
    let after = points(&mut c, "p");
    for (a, b) in before.iter().zip(&after) {
        assert!((a[1] - b[1]).abs() < 1e-6, "{before:?} {after:?}");
    }
}

/// #144: Duplicate input graph copies everything upstream of the shape, Duplicate input connections shares it.
#[test]
fn duplicate_special_input_graph_and_connections() {
    let mut c = Core::new();
    mel(&mut c, "polyCube -n box; polyBevel box.e[0];");
    // Plain: no history on the copy.
    mel(&mut c, "duplicate box;");
    assert_eq!(mel(&mut c, "listHistory box1;"), json!(["boxShape1"]));
    // Input graph: its own bevel and cube, wired as the original's.
    mel(&mut c, "duplicate -un box;");
    let hist = mel(&mut c, "listHistory box2;");
    let names: Vec<&str> = hist.as_array().unwrap().iter().filter_map(Value::as_str).collect();
    assert_eq!(names.len(), 3, "{names:?}");
    assert!(names.iter().all(|n| !["polyBevel1", "polyCube1"].contains(n)), "{names:?}");
    let shape2 = mel(&mut c, "listRelatives -s box2;")[0].as_str().unwrap().to_string();
    let src = mel(&mut c, &format!("listConnections -s 1 -d 0 {shape2}.inMesh;"));
    let src = src[0].as_str().unwrap().to_string();
    assert!(src.starts_with("polyBevel") && src != "polyBevel1", "{src}");
    assert!(names.contains(&src.as_str()), "{names:?}");
    // Editing the copy's cube leaves the original alone.
    let before = points(&mut c, "box");
    let cube = names.iter().find(|n| n.starts_with("polyCube")).unwrap().to_string();
    mel(&mut c, &format!("setAttr {cube}.width 5;"));
    assert_eq!(points(&mut c, "box"), before);
    assert_ne!(points(&mut c, "box2"), before);
    // Input connections: the copy is fed by the original's bevel itself.
    mel(&mut c, "duplicate -ic box;");
    let shape3 = mel(&mut c, "listRelatives -s box3;")[0].as_str().unwrap().to_string();
    assert_eq!(mel(&mut c, &format!("listConnections -s 1 -d 0 {shape3}.inMesh;")), json!(["polyBevel1"]));
    mel(&mut c, "setAttr polyCube1.height 3;");
    assert_eq!(points(&mut c, "box3"), points(&mut c, "box"));
}

/// #144: Group under World and New group, a run of copies, and Instance leaf nodes.
#[test]
fn duplicate_special_group_under_and_instance_leaf() {
    let mut c = Core::new();
    mel(&mut c, "polyCube -n leaf; group -n inner leaf; group -n outer inner;");
    let made = mel(&mut c, "duplicate -groupUnder world -rr inner;");
    assert_eq!(made, json!(["inner1"]));
    assert!(mel(&mut c, "listRelatives -p inner1;").as_array().is_none_or(|a| a.is_empty()));
    let made = mel(&mut c, "duplicate -groupUnder group -rr -count 3 -t 2 0 0 inner;");
    assert_eq!(made.as_array().unwrap().len(), 3);
    let parent = mel(&mut c, "listRelatives -p inner2;");
    assert_ne!(parent, json!(["outer"]));
    assert_eq!(mel(&mut c, &format!("listRelatives -p {};", parent[0].as_str().unwrap())), json!(["outer"]));
    // Instance leaf nodes: the transforms are new, the shape is an instance of the original's mesh.
    let made = mel(&mut c, "duplicate -ilf -rr inner;");
    let top = made[0].as_str().unwrap().to_string();
    let shape = mel(&mut c, &format!("listRelatives -s -f {top}|leaf;"));
    let s = shape[0].as_str().unwrap().to_string();
    mel(&mut c, "move -r 0 1 0 outer|inner|leaf.vtx[0];");
    let a = points(&mut c, "outer|inner|leaf");
    let b = points(&mut c, &s);
    let local = |p: &[[f64; 3]]| p.iter().map(|q| [q[0] - p[0][0], q[1] - p[0][1], q[2] - p[0][2]]).collect::<Vec<_>>();
    assert_eq!(local(&a), local(&b));
}

/// #144: the option box has the reference's rows, greys the input switches under Instance and makes the two input
/// switches exclude each other.
#[test]
fn duplicate_special_option_box_layout() {
    let rows = aztek_host::option_boxes::rows("duplicate");
    let flags: Vec<&str> = rows.iter().map(|r| r["flag"].as_str().unwrap()).collect();
    assert_eq!(flags, ["mode", "groupUnder", "smartTransform", "translate", "rotate", "scale", "count", "upstreamNodes", "inputConnections", "instanceLeaf", "renameChildren"]);
    for f in ["upstreamNodes", "inputConnections", "instanceLeaf"] {
        let r = rows.iter().find(|r| r["flag"] == f).unwrap();
        assert_eq!(r["disabledWhen"], json!({"flag": "mode", "value": "instance"}));
    }
    assert_eq!(rows[7]["excludes"], json!("inputConnections"));
    assert_eq!(rows[8]["excludes"], json!("upstreamNodes"));
}

fn faces(m: &aztek_geo::Mesh) -> usize {
    (0..m.face_capacity()).filter(|&f| m.face_valid(f)).count()
}

fn verts(m: &aztek_geo::Mesh) -> usize {
    (0..m.point_capacity()).filter(|&p| m.point_valid(p)).count()
}

fn mesh_of(c: &mut Core, shape: &str) -> std::sync::Arc<aztek_geo::Mesh> {
    let g = &mut c.s.graph;
    g.evaluate();
    let id = g.find(shape).unwrap().unwrap();
    let id = aztek_scene::components::as_mesh(g, id).unwrap();
    g.node(id).unwrap().data().mesh(aztek_scene::mesh_nodes::MeshNode::OUT_MESH).cloned().unwrap()
}

/// #140: Quad Draw's Extend picks the edge, its run along the border, or the whole border; Relax's Auto-Lock keeps the
/// border while there are inner points.
#[test]
fn quad_draw_extend_and_relax_picks() {
    use aztek_host::tools_quad::{extend_edges, relax_points};
    let mut c = Core::new();
    mel(&mut c, "polyPlane -n grid -w 4 -h 4 -sx 4 -sy 4;");
    let m = mesh_of(&mut c, "grid");
    let border: Vec<u32> = (0..m.edge_capacity()).filter(|&e| m.edge_valid(e) && m.is_boundary_edge(e)).collect();
    assert_eq!(border.len(), 16);
    // An edge in the middle of one side: alone, the side's four, all sixteen.
    let side = border.iter().copied().find(|&e| {
        let (a, b) = (m.position(m.edge_point(e, 0)), m.position(m.edge_point(e, 1)));
        (a.z - 2.0).abs() < 1e-9 && (b.z - 2.0).abs() < 1e-9 && a.x.min(b.x) > -1.5 && a.x.max(b.x) < 1.5
    }).unwrap();
    assert_eq!(extend_edges(&m, side, 0), vec![side]);
    let mut run = extend_edges(&m, side, 1);
    run.sort();
    assert_eq!(run.len(), 4, "{run:?}");
    assert!(run.iter().all(|&e| (m.position(m.edge_point(e, 0)).z - 2.0).abs() < 1e-9));
    assert_eq!(extend_edges(&m, side, 2).len(), 16);
    // Relax: auto-lock moves the inner points only; border only; all.
    let all: Vec<u32> = (0..m.point_capacity()).filter(|&p| m.point_valid(p)).collect();
    assert_eq!(relax_points(&m, &all, 0).len(), 9);
    assert_eq!(relax_points(&m, &all, 1).len(), 9);
    assert_eq!(relax_points(&m, &all, 2).len(), 16);
    assert_eq!(relax_points(&m, &all, 3).len(), 25);
}

/// #140: what the tools run: border edges extruded together and moved in world space, loops with edge flow and more than
/// one division, a loop deleted, and a slice across a plane, deleting or pulling off a side.
#[test]
fn quad_draw_and_multi_cut_commands() {
    use aztek_host::tools_quad::{cut_rotation, snap45, CutOpts};
    let mut c = Core::new();
    mel(&mut c, "polyPlane -n grid -w 4 -h 4 -sx 4 -sy 4;");
    let m = mesh_of(&mut c, "grid");
    let run = aztek_host::tools_quad::extend_edges(&m, (0..m.edge_capacity()).find(|&e| m.edge_valid(e) && m.is_boundary_edge(e) && (m.position(m.edge_point(e, 0)).z - 2.0).abs() < 1e-9 && (m.position(m.edge_point(e, 1)).z - 2.0).abs() < 1e-9).unwrap(), 1);
    let list: Vec<String> = run.iter().map(|e| format!("grid.e[{e}]")).collect();
    c.exec("polyExtrudeEdge", json!({"args": list, "worldSpace": true, "translate": [0.0, 0.0, 1.0], "keepFacesTogether": true})).unwrap();
    let m2 = mesh_of(&mut c, "grid");
    assert_eq!(faces(&m2), 16 + 4);
    assert_eq!(verts(&m2), 25 + 5, "the new faces share their new points");
    assert!((0..m2.point_capacity()).any(|p| m2.point_valid(p) && (m2.position(p).z - 3.0).abs() < 1e-6));
    // A centred loop with two divisions and edge flow.
    mel(&mut c, "polyCube -n cube -sx 1 -sy 1 -sz 1;");
    c.exec("polySplitRing", json!({"args": ["cube.e[0]"], "weight": 0.5, "splitType": 2, "divisions": 2, "insertWithEdgeFlow": true, "adjustEdgeFlow": 1.0})).unwrap();
    assert_eq!(faces(&mesh_of(&mut c, "cube")), 6 + 4 * 2);
    // Deleting a loop takes it out again.
    let cm = mesh_of(&mut c, "cube");
    let lp = aztek_geo::topology::edge_loop(&cm, (0..cm.edge_capacity()).find(|&e| cm.edge_valid(e) && aztek_geo::topology::edge_loop(&cm, e).len() == 4 && {
        let (a, b) = (cm.position(cm.edge_point(e, 0)), cm.position(cm.edge_point(e, 1)));
        (a.x - b.x).abs() < 1e-9 && a.x.abs() < 0.45 && a.x.abs() > 1e-6
    }).unwrap());
    let args: Vec<String> = lp.iter().map(|e| format!("cube.e[{e}]")).collect();
    c.exec("polyDelEdge", json!({"args": args, "cleanVertices": true})).unwrap();
    assert_eq!(faces(&mesh_of(&mut c, "cube")), 6 + 4);
    // Slice along the YZ plane through the middle: the plane's rotation in degrees.
    mel(&mut c, "polyCube -n s -w 2 -h 2 -d 2;");
    // As the tools run it: in stored units, the rotation in radians.
    aztek_ops::units::stored(|| c.exec("polyCut", json!({"args": ["s.f[*]"], "cutPlaneCenter": [0.0, 0.0, 0.0], "cutPlaneRotate": cut_rotation(glam::DVec3::X), "worldSpace": true, "deleteFaces": true}))).unwrap();
    let sm = mesh_of(&mut c, "s");
    let xs: Vec<f64> = (0..sm.point_capacity()).filter(|&p| sm.point_valid(p)).map(|p| sm.position(p).x).collect();
    assert!(xs.iter().all(|x| *x < 1e-6) || xs.iter().all(|x| *x > -1e-6), "{xs:?}");
    // Shift snaps a slice to 45 degrees and a point along an edge to the snap step.
    let to = snap45(glam::DVec2::ZERO, glam::DVec2::new(10.0, 1.0));
    assert!((to.y).abs() < 1e-9 && (to.x - 101f64.sqrt()).abs() < 1e-9);
    let o = CutOpts { snap_step: 25.0, ..Default::default() };
    assert_eq!(o.snap(0.3), 0.25);
    assert_eq!(o.snap(0.4), 0.5);
}

/// #140: a relax step keeps a border straight and its corners still, and evens out the points along it.
#[test]
fn quad_draw_relax_keeps_the_border() {
    use aztek_host::tools_quad::{relax_points, relaxed};
    let mut c = Core::new();
    mel(&mut c, "polyPlane -n strip -w 6 -h 1 -sx 6 -sy 1; move -r 0.3 0 0 strip.vtx[2] strip.vtx[9];");
    let m = mesh_of(&mut c, "strip");
    let all: Vec<u32> = (0..m.point_capacity()).filter(|&p| m.point_valid(p)).collect();
    let moved = relaxed(&m, &relax_points(&m, &all, 0));
    for (p, q) in &moved {
        let was = m.position(*p);
        assert!((q.z - was.z).abs() < 1e-9 && (q.y - was.y).abs() < 1e-9, "a border point stays on its side");
    }
    assert!(moved.iter().all(|(p, _)| m.position(*p).x.abs() < 2.99), "corners stay");
    let dragged = moved.iter().find(|(p, _)| (m.position(*p).x - (-1.0 + 0.3)).abs() < 1e-6).unwrap();
    assert!(dragged.1.x < -0.7 + 1e-9, "the moved point goes back toward its neighbours");
}

/// #140: what auto-weld runs when a dragged vertex is let go near another: onto it, then welded.
#[test]
fn quad_draw_auto_weld_commands() {
    let mut c = Core::new();
    c.exec("polyAppendVertex", json!({"items": [{"position": [0.0, 0.0, 0.0]}, {"position": [1.0, 0.0, 0.0]}, {"position": [1.0, 0.0, 1.0]}, {"position": [0.0, 0.0, 1.0]}]})).unwrap();
    c.exec("polyAppendVertex", json!({"items": [{"position": [1.05, 0.0, 0.0]}, {"position": [2.0, 0.0, 0.0]}, {"position": [2.0, 0.0, 1.0]}, {"position": [1.05, 0.0, 1.0]}]})).unwrap();
    let shape = mel(&mut c, "ls -type mesh;")[0].as_str().unwrap().to_string();
    assert_eq!(verts(&mesh_of(&mut c, &shape)), 8);
    aztek_ops::units::stored(|| {
        c.exec("xform", json!({"args": [format!("{shape}.vtx[4]")], "worldSpace": true, "translation": [1.0, 0.0, 0.0]})).unwrap();
        c.exec("polyMergeVertex", json!({"args": [format!("{shape}.vtx[4]"), format!("{shape}.vtx[1]")], "distance": 1e-4})).unwrap();
    });
    assert_eq!(verts(&mesh_of(&mut c, &shape)), 7);
}

/// #140: the Live Constraint Options and Multi-Cut's colours and Force Cage + Smooth, as the toolkit sends them.
#[test]
fn live_constraint_and_cut_colour_settings() {
    use aztek_host::viewport::LiveOpts;
    let mut o = LiveOpts::default();
    assert_eq!(o.wire, Some([0.0; 3]));
    assert!(!o.backfaces && o.offset.is_none());
    o.set(&json!({"wireOverlay": false, "meshColor": "#ff0000", "meshAlphaOn": true, "meshAlpha": 0.4, "surfaceOffsetOn": true, "surfaceOffset": 0.2, "snapToBackfaces": true}));
    assert_eq!(o.wire, None);
    assert_eq!(o.color, Some([1.0, 0.0, 0.0]));
    assert_eq!(o.alpha, Some(0.4));
    assert_eq!(o.offset, Some(0.2));
    assert!(o.backfaces);
    o.set(&json!({"meshColorOn": false, "meshAlphaOn": false}));
    assert_eq!((o.color, o.alpha), (None, None));
    let mut c = aztek_host::tools_quad::CutOpts::default();
    c.set(&json!({"forceCage": true, "lineColor": "#00ff00", "faceColor": "#102030"}));
    assert!(c.force_cage);
    assert_eq!(c.line_color, [0, 255, 0, 255]);
    assert_eq!(c.face_color, [16, 32, 48, 255]);
}

/// #140: Slice Along Plane leaves the cut's plane in the Show Manipulator tool: arrows for its centre, rings to turn it,
/// and moving its centre moves the cut.
#[test]
fn slice_plane_handles() {
    let mut c = Core::new();
    mel(&mut c, "polyCube -n box -w 2 -h 2 -d 2 -sx 1;");
    aztek_ops::units::stored(|| c.exec("polyCut", json!({"args": ["box.f[*]"], "cutPlaneCenter": [0.0, 0.0, 0.0], "cutPlaneRotate": aztek_host::tools_quad::cut_rotation(glam::DVec3::X), "worldSpace": true, "deleteFaces": true}))).unwrap();
    let xs = |c: &mut Core| points(c, "box").iter().map(|p| p[0]).fold(f64::MIN, f64::max);
    assert!(xs(&mut c).abs() < 1e-6 || xs(&mut c) > 0.99);
    let before = xs(&mut c);
    assert!((mel(&mut c, "getAttr polyCut1.cutPlaneRotateY;").as_f64().unwrap() - 90.0).abs() < 1e-6);
    mel(&mut c, "setAttr polyCut1.cutPlaneCenterX 0.5;");
    assert!((xs(&mut c) - before).abs() > 0.4, "{before} {}", xs(&mut c));
    let mut vp = aztek_host::viewport::Viewport::new(aztek_host::camera::CamKind::Persp);
    vp.rect = [0.0, 0.0, 800.0, 600.0];
    let id = c.s.graph.find("polyCut1").unwrap().unwrap();
    let m = aztek_host::node_manip::build(&mut c.s, &vp, id, true, 1.0).unwrap();
    let attrs: Vec<&str> = m.handles.iter().map(|h| h.attr.as_str()).collect();
    assert_eq!(attrs, ["cutPlaneCenterX", "cutPlaneCenterY", "cutPlaneCenterZ", "cutPlaneRotateX", "cutPlaneRotateY", "cutPlaneRotateZ"]);
    assert_eq!(m.lines.len(), 5);
}

/// #149: a held X snaps to the grid lines the view shows, past the grid's edge at the same spacing, and the move
/// tool's centre handle takes the grid point under the cursor on the grid's plane.
#[test]
fn grid_snap_follows_the_visible_grid() {
    use glam::DVec3;
    let mut vp = aztek_host::viewport::Viewport::new(aztek_host::camera::CamKind::Persp);
    vp.rect = [0.0, 0.0, 800.0, 600.0];
    vp.grid_spacing = 0.5;
    assert_eq!(vp.snap_step(), 0.5);
    vp.grid_style.division_lines = false;
    assert_eq!(vp.snap_step(), 0.5 * vp.grid_style.divisions as f64);
    // The cursor over a point on the ground finds that point.
    let at = DVec3::new(3.2, 0.0, -1.7);
    let px = vp.screen().project(at).unwrap();
    let hit = vp.grid_point(px).unwrap();
    assert!((hit - at).length() < 1e-6, "{hit}");
}

/// #150: joints draw in the darker blue.
#[test]
fn joints_draw_in_their_blue() {
    let mut c = Core::new();
    mel(&mut c, "select -cl; joint -p 0 0 0;");
    let items = aztek_host::viewport::rig_items(&c.s.graph);
    assert_eq!(items[0].color, aztek_host::viewport::JOINT_COLOR);
}
