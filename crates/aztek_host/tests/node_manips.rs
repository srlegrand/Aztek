//! The Show Manipulator tool: real presses, moves and releases through its mouse function, checking the attribute, the
//! mesh, the history line and the undo step.

use aztek_host::camera::CamKind;
use aztek_host::node_manip::{self, Form, Manip};
use aztek_host::viewport::Viewport;
use aztek_host::Core;
use glam::{DVec2, DVec3};
use serde_json::json;

fn view() -> Viewport {
    let mut vp = Viewport::new(CamKind::Persp);
    vp.rect = [0.0, 0.0, 1000.0, 800.0];
    vp
}

fn manip(c: &mut Core, vp: &Viewport) -> Manip {
    let id = node_manip::target(c).expect("a node");
    let world = c.node_manip.world;
    node_manip::build(&mut c.s, vp, id, world, 1.0).expect("handles")
}

fn grip(vp: &Viewport, m: &Manip, attr: &str) -> DVec2 {
    let h = m.handles.iter().find(|h| h.attr == attr).unwrap_or_else(|| panic!("no handle for {attr}: {:?}", m.handles.iter().map(|h| &h.attr).collect::<Vec<_>>()));
    let s = vp.screen();
    match &h.form {
        Form::Arrow { to, .. } | Form::Cube { to, .. } => s.project(*to).unwrap(),
        Form::Dot { at } => s.project(*at).unwrap(),
        Form::Ring { centre, axis, radius } => {
            let (u, v) = axis.any_orthonormal_pair();
            s.project(*centre + (u * 0.6 + v * 0.8) * *radius).unwrap()
        }
    }
}

/// Drags a handle by an offset in pixels, in three steps as a mouse does.
fn drag(c: &mut Core, vp: &Viewport, attr: &str, by: DVec2) {
    let m = manip(c, vp);
    let at = grip(vp, &m, attr);
    assert_eq!(node_manip::mouse(c, vp, 0, 1, at, 1, 1.0), Some(true), "press on {attr} at {at}");
    for k in 1..=3 {
        node_manip::mouse(c, vp, 1, 0, at + by * (k as f64 / 3.0), 1, 1.0);
    }
    node_manip::mouse(c, vp, 2, 1, at + by, 0, 1.0);
}

fn num(c: &mut Core, plug: &str) -> f64 {
    c.mel(&format!("getAttr {plug};")).as_f64().unwrap_or_else(|| panic!("{plug} has no number"))
}

/// The pixels that drag a handle's own axis outward.
fn outward(vp: &Viewport, m: &Manip, attr: &str, px: f64) -> DVec2 {
    let h = m.handles.iter().find(|h| h.attr == attr).unwrap();
    let s = vp.screen();
    let (a, b) = match &h.form {
        Form::Arrow { from, to } | Form::Cube { from, to } => (s.project(*from).unwrap(), s.project(*to).unwrap()),
        _ => panic!("{attr} has no axis"),
    };
    (b - a).normalize() * px
}

fn echoes(c: &mut Core) -> Vec<String> {
    c.take_log().as_array().unwrap().iter().filter(|l| l["kind"] == 0).map(|l| l["text"].as_str().unwrap().to_string()).collect()
}

fn extruded(c: &mut Core) {
    c.mel("polyCube;");
    let r = c.mel("select pCube1.f[1]; polyExtrudeFacet;");
    assert_eq!(r, json!(["polyExtrudeFace1"]));
    assert!(node_manip::after_action(c, &r));
}

#[test]
fn extrude_arrow_writes_local_translate_in_one_line_and_one_undo() {
    let mut c = Core::new();
    let vp = view();
    extruded(&mut c);
    let m = manip(&mut c, &vp);
    assert_eq!(m.node, "polyExtrudeFace1");
    let by = outward(&vp, &m, "localTranslateZ", 60.0);
    c.take_log();
    drag(&mut c, &vp, "localTranslateZ", by);
    let v = num(&mut c, "polyExtrudeFace1.localTranslateZ");
    assert!(v > 0.05, "moved outward: {v}");
    let lines = echoes(&mut c);
    assert_eq!(lines.iter().filter(|l| l.starts_with("setAttr")).count(), 1, "{lines:?}");
    assert!(lines[0].contains("polyExtrudeFace1.localTranslateZ"), "{lines:?}");
    c.mel("undo;");
    assert_eq!(num(&mut c, "polyExtrudeFace1.localTranslateZ"), 0.0);
}

#[test]
fn extrude_arrow_moves_the_mesh_along_its_drawn_axis() {
    for attr in ["localTranslateX", "localTranslateY", "localTranslateZ"] {
        let mut c = Core::new();
        let vp = view();
        extruded(&mut c);
        let m = manip(&mut c, &vp);
        let h = m.handles.iter().find(|h| h.attr == attr).unwrap();
        let Form::Arrow { from, to } = h.form else { panic!() };
        let axis = (to - from).normalize();
        let before = c.mel("xform -q -ws -t pCube1.vtx[8];");
        let by = outward(&vp, &m, attr, 50.0);
        drag(&mut c, &vp, attr, by);
        let after = c.mel("xform -q -ws -t pCube1.vtx[8];");
        let p = |v: &serde_json::Value| DVec3::new(v[0].as_f64().unwrap(), v[1].as_f64().unwrap(), v[2].as_f64().unwrap());
        let moved = p(&after) - p(&before);
        assert!(moved.length() > 0.02, "{attr} moved nothing");
        assert!(moved.normalize().dot(axis) > 0.99, "{attr}: moved {moved} against axis {axis}");
    }
}

#[test]
fn extrude_scale_and_rotate_handles_write_their_channels() {
    let mut c = Core::new();
    let vp = view();
    extruded(&mut c);
    let m = manip(&mut c, &vp);
    let by = outward(&vp, &m, "localScaleX", 40.0);
    drag(&mut c, &vp, "localScaleX", by);
    assert!(num(&mut c, "polyExtrudeFace1.localScaleX") > 1.01);
    drag(&mut c, &vp, "localRotateZ", DVec2::new(30.0, 30.0));
    assert!(num(&mut c, "polyExtrudeFace1.localRotateZ").abs() > 0.1);
}

#[test]
fn extrude_toggle_swaps_local_and_world_handles() {
    let mut c = Core::new();
    let vp = view();
    extruded(&mut c);
    let m = manip(&mut c, &vp);
    let at = grip(&vp, &m, "world");
    node_manip::mouse(&mut c, &vp, 0, 1, at, 1, 1.0);
    node_manip::mouse(&mut c, &vp, 2, 1, at, 0, 1.0);
    let m = manip(&mut c, &vp);
    assert!(m.handles.iter().any(|h| h.attr == "translateX"));
    let by = outward(&vp, &m, "translateX", 50.0);
    drag(&mut c, &vp, "translateX", by);
    assert!(num(&mut c, "polyExtrudeFace1.translateX") > 0.02);
}

#[test]
fn a_press_off_the_handles_leaves_the_view_to_select() {
    let mut c = Core::new();
    let vp = view();
    extruded(&mut c);
    assert_eq!(node_manip::mouse(&mut c, &vp, 0, 1, DVec2::new(2.0, 2.0), 1, 1.0), None);
}

#[test]
fn a_middle_drag_anywhere_takes_the_highlighted_channel() {
    let mut c = Core::new();
    let vp = view();
    extruded(&mut c);
    c.node_manip.active_attr = Some("localTranslateZ".into());
    let at = DVec2::new(3.0, 3.0);
    assert_eq!(node_manip::mouse(&mut c, &vp, 0, 2, at, 2, 1.0), Some(true));
    node_manip::mouse(&mut c, &vp, 1, 0, at + DVec2::new(40.0, -40.0), 2, 1.0);
    node_manip::mouse(&mut c, &vp, 2, 2, at + DVec2::new(40.0, -40.0), 0, 1.0);
    assert!(num(&mut c, "polyExtrudeFace1.localTranslateZ").abs() > 1e-3);
}

#[test]
fn t_finds_the_newest_node_with_handles_and_a_new_selection_forgets_the_asked_one() {
    let mut c = Core::new();
    let vp = view();
    c.mel("polyCube; select pCube1.f[1]; polyExtrudeFacet; select pCube1.f[2]; polyPoke;");
    c.mel("select pCube1;");
    assert_eq!(manip(&mut c, &vp).node, "polyPoke1");
    node_manip::show_node(&mut c, "polyExtrudeFace1");
    assert_eq!(manip(&mut c, &vp).node, "polyExtrudeFace1");
    c.mel("select -cl; select pCube1.f[0];");
    assert_eq!(manip(&mut c, &vp).node, "polyPoke1");
}

#[test]
fn primitives_have_size_handles() {
    let mut c = Core::new();
    let vp = view();
    for (make, node, attrs) in [
        ("polyCube;", "polyCube1", vec!["width", "height", "depth"]),
        ("polySphere;", "polySphere1", vec!["radius"]),
        ("polyCylinder;", "polyCylinder1", vec!["radius", "height"]),
        ("polyCone;", "polyCone1", vec!["radius", "height"]),
        ("polyTorus;", "polyTorus1", vec!["radius", "sectionRadius"]),
        ("polyPlane;", "polyPlane1", vec!["width", "height"]),
    ] {
        c.mel("file -f -new;");
        c.mel(make);
        for attr in attrs {
            let before = num(&mut c, &format!("{node}.{attr}"));
            let m = manip(&mut c, &vp);
            let h = m.handles.iter().find(|h| h.attr == attr).expect(attr);
            let Form::Dot { at } = h.form else { panic!() };
            let aztek_host::node_manip::Drive::Along { axis, .. } = h.drive else { panic!() };
            let s = vp.screen();
            let by = (s.project(at + axis).unwrap() - s.project(at).unwrap()).normalize() * 40.0;
            drag(&mut c, &vp, attr, by);
            let after = num(&mut c, &format!("{node}.{attr}"));
            assert!(after > before, "{node}.{attr}: {before} to {after}");
        }
    }
}

#[test]
fn nonlinear_handles_write_their_own_attributes() {
    for (kind, attrs) in [
        ("bend", vec!["curvature", "highBound"]),
        ("twist", vec!["highBound"]),
        ("squash", vec!["factor", "expand", "lowBound"]),
        ("flare", vec!["startFlareX", "endFlareZ"]),
        ("sine", vec!["amplitude", "wavelength"]),
        ("wave", vec!["amplitude", "maxRadius"]),
    ] {
        let mut c = Core::new();
        let vp = view();
        c.mel("polyCube -sy 4; ");
        c.mel(&format!("nonLinear -type {kind};"));
        let node = format!("{kind}1");
        for attr in attrs {
            let plug = format!("{node}.{attr}");
            let before = num(&mut c, &plug);
            let m = manip(&mut c, &vp);
            assert_eq!(m.node, node);
            let by = outward(&vp, &m, attr, 40.0);
            drag(&mut c, &vp, attr, by);
            let after = num(&mut c, &plug);
            assert!((after - before).abs() > 1e-3, "{plug}: {before} to {after}");
        }
    }
}

#[test]
fn twist_rings_write_their_angles() {
    let mut c = Core::new();
    let vp = view();
    c.mel("polyCube; nonLinear -type twist;");
    drag(&mut c, &vp, "endAngle", DVec2::new(20.0, 30.0));
    assert!(num(&mut c, "twist1.endAngle").abs() > 0.1);
}

#[test]
fn soft_mod_handles_write_radius_and_centre() {
    let mut c = Core::new();
    let vp = view();
    c.mel("polyCube; select pCube1.vtx[0:3]; softMod -fr 2;");
    let m = manip(&mut c, &vp);
    assert_eq!(m.node, "softMod1");
    assert!(!m.lines.is_empty(), "the falloff is drawn");
    let by = outward(&vp, &m, "falloffCenterX", 40.0);
    drag(&mut c, &vp, "falloffCenterX", by);
    assert!(num(&mut c, "softMod1.falloffCenterX").abs() > 1e-3);
}

#[test]
fn mirror_plane_moves_with_its_arrow() {
    let mut c = Core::new();
    let vp = view();
    c.mel("polyCube; polyMirrorFace pCube1;");
    let m = manip(&mut c, &vp);
    assert_eq!(m.node, "polyMirror1");
    let by = outward(&vp, &m, "mirrorPosition", 40.0);
    drag(&mut c, &vp, "mirrorPosition", by);
    assert!(num(&mut c, "polyMirror1.mirrorPosition").abs() > 1e-3);
}

#[test]
fn a_plain_object_gets_translate_arrows() {
    let mut c = Core::new();
    let vp = view();
    c.mel("spaceLocator;");
    let m = manip(&mut c, &vp);
    let by = outward(&vp, &m, "translateY", 40.0);
    drag(&mut c, &vp, "translateY", by);
    assert!(num(&mut c, "locator1.translateY") > 1e-3);
}

#[test]
fn the_in_view_rows_are_the_nodes_own() {
    let mut c = Core::new();
    extruded(&mut c);
    let f = node_manip::fields_json(&mut c);
    assert_eq!(f["node"], "polyExtrudeFace1");
    let labels: Vec<&str> = f["fields"].as_array().unwrap().iter().map(|r| r["label"].as_str().unwrap()).collect();
    assert_eq!(labels, ["Thickness", "Offset", "Divisions", "Keep Faces Together"]);
}

fn uv_of(c: &mut Core, comp: &str) -> Vec<f64> {
    c.mel(&format!("polyEditUV -q {comp};")).as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect()
}

#[test]
fn planar_projection_handles_move_the_uvs() {
    let mut c = Core::new();
    let vp = view();
    c.mel("polyCube; select pCube1.f[*];");
    let r = c.exec("polyProjection", json!({"type": "planar", "mapDirection": "y"})).unwrap();
    assert_eq!(r, json!(["polyPlanarProj1"]));
    assert!(node_manip::after_action(&mut c, &r));
    let m = manip(&mut c, &vp);
    assert_eq!(m.node, "polyPlanarProj1");
    assert_eq!(num(&mut c, "polyPlanarProj1.projectionWidth"), 1.0);
    let before = uv_of(&mut c, "pCube1.map[2]");
    let by = outward(&vp, &m, "projectionCenterX", 60.0);
    c.take_log();
    drag(&mut c, &vp, "projectionCenterX", by);
    assert!(num(&mut c, "polyPlanarProj1.projectionCenterX") > 0.01);
    let lines = echoes(&mut c);
    assert_eq!(lines.iter().filter(|l| l.starts_with("setAttr")).count(), 1, "{lines:?}");
    let after = uv_of(&mut c, "pCube1.map[2]");
    assert_ne!(before, after, "the UVs followed");
    c.mel("undo;");
    assert_eq!(uv_of(&mut c, "pCube1.map[2]"), before, "undo restores the projection's own UVs");
}

#[test]
fn projection_width_and_rings_write_attributes() {
    let mut c = Core::new();
    let vp = view();
    c.mel("polyCube; select pCube1.f[1];");
    c.exec("polyProjection", json!({"type": "planar", "mapDirection": "y"})).unwrap();
    let m = manip(&mut c, &vp);
    let h = m.handles.iter().find(|h| h.attr == "projectionWidth").unwrap();
    let node_manip::Drive::Along { axis, .. } = h.drive else { panic!() };
    let Form::Dot { at } = h.form else { panic!() };
    let s = vp.screen();
    let by = (s.project(at + axis).unwrap() - s.project(at).unwrap()).normalize() * 40.0;
    drag(&mut c, &vp, "projectionWidth", by);
    assert!(num(&mut c, "polyPlanarProj1.projectionWidth") > 1.0);
    drag(&mut c, &vp, "rotateY", DVec2::new(30.0, 25.0));
    assert!(num(&mut c, "polyPlanarProj1.rotateY").abs() > 0.1);
}

#[test]
fn cylindrical_and_spherical_have_handles() {
    for (ty, node) in [("cylindrical", "polyCylProj1"), ("spherical", "polySphProj1")] {
        let mut c = Core::new();
        let vp = view();
        c.mel("polyCube;");
        c.exec("polyProjection", json!({"args": ["pCube1"], "type": ty})).unwrap();
        c.mel(&format!("select pCube1;"));
        let m = manip(&mut c, &vp);
        assert_eq!(m.node, node);
        let by = outward(&vp, &m, "projectionCenterY", 40.0);
        drag(&mut c, &vp, "projectionCenterY", by);
        assert!(num(&mut c, &format!("{node}.projectionCenterY")).abs() > 1e-3);
    }
}

#[test]
fn sculpt_handles() {
    let mut c = Core::new();
    let vp = view();
    c.mel("polyCube; sculpt;");
    c.mel("select pCube1;");
    let m = manip(&mut c, &vp);
    assert_eq!(m.node, "sculpt1");
    let by = outward(&vp, &m, "dropoffDistance", 40.0);
    drag(&mut c, &vp, "dropoffDistance", by);
    assert!(num(&mut c, "sculpt1.dropoffDistance") > 1.0);
}

#[test]
fn ik_handle_pole_vector_and_twist() {
    let mut c = Core::new();
    let vp = view();
    c.mel("select -cl; joint -p 0 0 0 -n a; joint -p 0 1 0.2 -n b; joint -p 0 2 0 -n d; ikHandle -sj a -ee d -sol ikRPsolver -n hd;");
    c.mel("select hd;");
    let m = manip(&mut c, &vp);
    let before = num(&mut c, "hd.poleVectorX");
    let by = outward(&vp, &m, "poleVectorX", 40.0);
    drag(&mut c, &vp, "poleVectorX", by);
    assert!((num(&mut c, "hd.poleVectorX") - before).abs() > 1e-3);
}

#[test]
fn camera_centre_of_interest() {
    let mut c = Core::new();
    let vp = view();
    c.mel("camera -n shot; move 0 0 8 shot1;");
    c.mel("select shot1;");
    let m = manip(&mut c, &vp);
    let h = m.handles.iter().find(|h| h.attr == "centerOfInterest").expect("the mark");
    let node_manip::Drive::Along { start, .. } = h.drive else { panic!() };
    let at = grip(&vp, &m, "centerOfInterest");
    node_manip::mouse(&mut c, &vp, 0, 1, at, 1, 1.0);
    node_manip::mouse(&mut c, &vp, 1, 0, at + DVec2::new(0.0, 40.0), 1, 1.0);
    node_manip::mouse(&mut c, &vp, 2, 1, at + DVec2::new(0.0, 40.0), 0, 1.0);
    let v = num(&mut c, &format!("{}.centerOfInterest", m.node));
    assert!((v - start).abs() > 1e-3, "{start} to {v}");
}
