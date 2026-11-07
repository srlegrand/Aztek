//! Tests: isolation.

use aztek_geo::ops::subdivide::{subdivide, SubdivideOptions};
use aztek_geo::primitives::{make_plane, PlaneOptions};
use aztek_geo::{Index, Mesh, INVALID};
use aztek_scene::components::{self, ComponentMode};
use aztek_scene::isolation as iso;
use aztek_scene::plug::MeshPtr;
use aztek_scene::{modifiers, NodeId, SelectMode, Session};
use serde_json::{json, Value};

fn run(s: &mut Session, name: &str, call: Value) -> Value {
    s.run(name, call).unwrap_or_else(|e| panic!("{name}: {e}"))
}

struct Plane {
    name: String,
    /// The mesh shape.
    id: NodeId,
    /// Its transform.
    xf: NodeId,
    mesh: MeshPtr,
}

/// A 4 x 4 quad plane on the integer grid from -2 to 2, face centres on the half units.
fn plane(s: &mut Session) -> Plane {
    let name = run(s, "polyPlane", json!({"width": 4, "depth": 4, "subdivisionsX": 4, "subdivisionsZ": 4}))[0]
        .as_str()
        .unwrap()
        .to_string();
    s.graph.evaluate();
    // The mesh shape: component selections and partial isolation key on it.
    let xf = s.graph.find(&name).unwrap().unwrap();
    let id = components::as_mesh(&s.graph, xf).unwrap();
    let mesh = modifiers::cage(&mut s.graph, id).unwrap();
    Plane { name, id, xf, mesh }
}

fn point_at(m: &Mesh, x: f64, z: f64) -> Index {
    (0..m.point_capacity())
        .find(|&p| m.point_valid(p) && (m.position(p).x - x).abs() < 1e-6 && (m.position(p).z - z).abs() < 1e-6)
        .unwrap_or(INVALID)
}

fn face_at(m: &Mesh, x: f64, z: f64) -> Index {
    (0..m.face_capacity())
        .find(|&f| m.face_valid(f) && (m.face_centroid(f).x - x).abs() < 1e-6 && (m.face_centroid(f).z - z).abs() < 1e-6)
        .unwrap_or(INVALID)
}

fn shown_count(mask: &[u8]) -> usize {
    mask.iter().filter(|&&v| v == 1).count()
}

#[test]
fn isolating_objects_shows_them_and_their_children_nothing_else() {
    let mut s = Session::new();
    run(&mut s, "selectMode", json!({"args": ["object"]}));
    let (a, b) = (plane(&mut s), plane(&mut s));
    run(&mut s, "select", json!({"args": [a.name]}));
    let mut set = iso::from_selection(&mut s.graph);
    assert!(set.active());
    assert!(set.shows(a.id));
    assert!(!set.shows(b.id));
    assert!(!set.is_partial(a.id));
    assert!(set.shown_faces(a.id, &a.mesh).is_empty());

    // again: no change
    iso::add_selection(&mut set, &mut s.graph);
    run(&mut s, "select", json!({"args": [b.name]}));
    iso::add_selection(&mut set, &mut s.graph);
    assert!(set.shows(b.id));
    run(&mut s, "select", json!({"args": [a.name]}));
    iso::remove_selection(&mut set, &mut s.graph);
    assert!(!set.shows(a.id));
    assert!(set.shows(b.id));
    run(&mut s, "select", json!({"args": [b.name]}));
    iso::remove_selection(&mut set, &mut s.graph);
    // Removing the last one leaves an isolation showing nothing rather than everything.
    assert!(set.active());
    assert!(!set.shows(b.id));
}

#[test]
fn isolating_a_child_leaves_its_unselected_parent_hidden() {
    let mut s = Session::new();
    run(&mut s, "selectMode", json!({"args": ["object"]}));
    let (parent, child) = (plane(&mut s), plane(&mut s));
    run(&mut s, "parent", json!({"args": [child.name, parent.name]}));
    s.graph.select(&[child.xf], SelectMode::Replace);
    let set = iso::from_selection(&mut s.graph);
    assert!(set.shows(child.xf));
    assert!(!set.shows(parent.xf));

    s.graph.select(&[parent.xf], SelectMode::Replace);
    let set = iso::from_selection(&mut s.graph);
    assert!(set.shows(parent.xf));
    assert!(set.shows(child.xf));
}

#[test]
fn isolating_components_shows_only_the_faces_they_touch() {
    let mut s = Session::new();
    let p = plane(&mut s);
    let m = &*p.mesh;
    let (f0, f1) = (face_at(m, -1.5, -1.5), face_at(m, -0.5, -1.5));
    run(&mut s, "selectComponents", json!({"args": [p.name], "faces": [f0, f1]}));
    assert_eq!(components::mode(&s.graph), ComponentMode::Face);
    let mut set = iso::from_selection(&mut s.graph);
    assert!(set.is_partial(p.id));
    let mask = set.shown_faces(p.id, m);
    assert_eq!(shown_count(&mask), 2);
    assert_eq!(mask[f0 as usize], 1);
    assert_eq!(mask[f1 as usize], 1);

    let vis = iso::visibility(m, mask);
    assert!(vis.face(f0));
    assert!(!vis.face(face_at(m, 1.5, 1.5)));
    assert!(vis.point(point_at(m, -2.0, -2.0)));
    assert!(vis.point(point_at(m, 0.0, -1.0)));
    assert!(!vis.point(point_at(m, 2.0, 2.0)));
    // the border of the region shows
    assert!(vis.edge(m.find_edge(point_at(m, 0.0, -2.0), point_at(m, 0.0, -1.0))));
    assert!(!vis.edge(m.find_edge(point_at(m, 1.0, -2.0), point_at(m, 1.0, -1.0))));

    // A vertex isolates the faces around it.
    run(&mut s, "selectMode", json!({"args": ["vertex"]}));
    run(&mut s, "selectComponents", json!({"args": [p.name], "vertices": [point_at(m, 0.0, 0.0)]}));
    let around = iso::from_selection(&mut s.graph);
    assert_eq!(shown_count(&around.shown_faces(p.id, m)), 4);

    // Adding more faces grows it; removing takes faces away.
    run(&mut s, "selectMode", json!({"args": ["face"]}));
    run(&mut s, "selectComponents", json!({"args": [p.name], "faces": [face_at(m, 1.5, 1.5)]}));
    iso::add_selection(&mut set, &mut s.graph);
    assert_eq!(shown_count(&set.shown_faces(p.id, m)), 3);
    run(&mut s, "selectComponents", json!({"args": [p.name], "faces": [f0]}));
    iso::remove_selection(&mut set, &mut s.graph);
    let mask = set.shown_faces(p.id, m);
    assert_eq!(shown_count(&mask), 2);
    assert_eq!(mask[f0 as usize], 0);

    // A whole object loses just the removed faces.
    run(&mut s, "selectMode", json!({"args": ["object"]}));
    run(&mut s, "select", json!({"args": [p.name]}));
    let mut whole = iso::from_selection(&mut s.graph);
    assert!(!whole.is_partial(p.id));
    run(&mut s, "selectMode", json!({"args": ["face"]}));
    run(&mut s, "selectComponents", json!({"args": [p.name], "faces": [f0]}));
    iso::remove_selection(&mut whole, &mut s.graph);
    assert_eq!(shown_count(&whole.shown_faces(p.id, m)), 15);

    // Saved with the scene by name.
    let back = iso::from_json(&iso::to_json(&set, &s.graph), &s.graph);
    assert_eq!(back.shown_faces(p.id, m), set.shown_faces(p.id, m));
    assert_ne!(back.version, set.version);
    assert!(!iso::from_json(&Value::Null, &s.graph).active());
}

#[test]
fn component_tools_leave_hidden_faces_alone() {
    let mut s = Session::new();
    let p = plane(&mut s);
    let m = &*p.mesh;
    // The left two columns show.
    let mut left = Vec::new();
    for x in [-1.5, -0.5] {
        for z in [-1.5, -0.5, 0.5, 1.5] {
            left.push(face_at(m, x, z));
        }
    }
    run(&mut s, "selectComponents", json!({"args": [p.name], "faces": left}));
    let set = iso::from_selection(&mut s.graph);
    s.graph.active_isolation = Some(set);
    let faces_sel = |s: &Session, k| components::selected(&s.graph, p.id, k).to_vec();

    // Grow from a face on the region's edge stays inside it.
    run(&mut s, "selectComponents", json!({"args": [p.name], "faces": [face_at(m, -0.5, 0.5)]}));
    run(&mut s, "growSelection", json!({}));
    for f in faces_sel(&s, ComponentMode::Face) {
        assert!(m.face_centroid(f).x < 0.0);
    }
    assert_eq!(faces_sel(&s, ComponentMode::Face).len(), 6);

    // Converting from the object takes only what shows.
    run(&mut s, "selectMode", json!({"args": ["object"]}));
    run(&mut s, "select", json!({"args": [p.name]}));
    run(&mut s, "convertSelection", json!({"args": ["face"]}));
    assert_eq!(faces_sel(&s, ComponentMode::Face).len(), 8);
    run(&mut s, "convertSelection", json!({"args": ["vertex"]}));
    assert_eq!(faces_sel(&s, ComponentMode::Vertex).len(), 15);

    // Select All Components and grow on vertices.
    run(&mut s, "selectAllComponents", json!({}));
    assert_eq!(faces_sel(&s, ComponentMode::Vertex).len(), 15);

    // An edge loop across the plane stops where the faces end.
    run(&mut s, "selectMode", json!({"args": ["edge"]}));
    let e = m.find_edge(point_at(m, -2.0, 0.0), point_at(m, -1.0, 0.0));
    let lp = run(&mut s, "selectLoopThrough", json!({"args": [p.name], "edge": e}));
    assert_eq!(lp.as_array().unwrap().len(), 2);
    run(&mut s, "selectComponents", json!({"args": [p.name], "edges": [e]}));
    run(&mut s, "selectLoop", json!({}));
    assert_eq!(faces_sel(&s, ComponentMode::Edge).len(), 2);

    // Face loop through a hidden-region crossing edge keeps the shown run.
    let crossing = m.find_edge(point_at(m, -1.0, 0.0), point_at(m, -1.0, 1.0));
    let fs = run(&mut s, "selectLoopThrough", json!({"args": [p.name], "edge": crossing, "faces": true}));
    assert_eq!(fs.as_array().unwrap().len(), 2);
    s.graph.active_isolation = None;
}

#[test]
fn chains_clip_at_hidden_elements_wrapping_when_closed() {
    let chain = [0u32, 1, 2, 3, 4, 5];
    let hide3 = |x: u32| x != 3;
    assert_eq!(iso::clip_chain(&chain, 1, false, hide3), vec![0, 1, 2]);
    assert_eq!(iso::clip_chain(&chain, 1, true, hide3), vec![4, 5, 0, 1, 2]);
    assert!(iso::clip_chain(&chain, 3, true, hide3).is_empty());
    assert_eq!(iso::clip_chain(&chain, 2, true, |_| true), chain.to_vec());
}

#[test]
fn smooth_preview_faces_map_back_to_the_isolated_ones() {
    let m = make_plane(&PlaneOptions { width: 2.0, depth: 2.0, subdiv_x: 2, subdiv_z: 1 });
    let mut shown = vec![0u8; m.face_capacity() as usize];
    shown[0] = 1;
    for levels in [1, 2] {
        let o = SubdivideOptions { levels, ..Default::default() };
        let smooth = subdivide(&m, &o, None);
        let mask = iso::subdivided_faces(&m, levels, &shown, smooth.face_capacity());
        assert_eq!(mask.len(), smooth.face_capacity() as usize);
        assert_eq!(shown_count(&mask) as u32, smooth.face_capacity() / 2);
        // The shown half is the side face 0 lies on.
        let side = m.face_centroid(0).x;
        for f in 0..smooth.face_capacity() {
            assert_eq!(smooth.face_centroid(f).x * side > 0.0, mask[f as usize] != 0);
        }
    }
}
