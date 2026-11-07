//! Custom axis orientation: the manipulator's axes from angles, an object, a direction, or the selected point, edge or
//! face (Tool Settings' Set to Point, Set to Edge and Set to Face).

use aztek_scene::components::{self, ComponentMode};
use aztek_scene::session::Session;
use glam::{DMat3, DQuat, DVec3};
use serde_json::{Map, Value};

/// Axes from XYZ Euler angles in radians, as -orientAxes holds them.
pub fn frame_from_radians(e: [f64; 3]) -> DMat3 {
    let q = DQuat::from_euler(glam::EulerRot::ZYX, e[2], e[1], e[0]);
    DMat3::from_quat(q)
}

fn radians(f: &DMat3) -> [f64; 3] {
    let (z, y, x) = DQuat::from_mat3(f).to_euler(glam::EulerRot::ZYX);
    [x, y, z]
}

/// A frame with X along a direction.
fn along(x: DVec3) -> Option<DMat3> {
    let x = x.normalize_or_zero();
    if x == DVec3::ZERO {
        return None;
    }
    let up = if x.y.abs() < 0.9 { DVec3::Y } else { DVec3::Z };
    let z = x.cross(up).normalize();
    Some(DMat3::from_cols(x, z.cross(x), z))
}

/// A frame with Z along a normal.
fn facing(n: DVec3) -> Option<DMat3> {
    let n = n.normalize_or_zero();
    if n == DVec3::ZERO {
        return None;
    }
    let up = if n.y.abs() < 0.9 { DVec3::Y } else { DVec3::X };
    let x = up.cross(n).normalize();
    Some(DMat3::from_cols(x, n.cross(x), n))
}

/// The custom axes a context's flags ask for, as radians.
pub fn from_flags(s: &mut Session, flags: &Map<String, Value>) -> Option<[f64; 3]> {
    let triple = |key: &str| -> Option<DVec3> {
        let v = flags.get(key)?.as_array()?;
        Some(DVec3::new(v.first()?.as_f64()?, v.get(1)?.as_f64()?, v.get(2)?.as_f64()?))
    };
    if let Some(e) = triple("orientAxes") {
        return Some(e.to_array());
    }
    if let Some(name) = flags.get("orientObject").and_then(Value::as_str) {
        let id = s.graph.find(name).ok()??;
        let w = aztek_scene::rig::world(&mut s.graph, id);
        let m = DMat3::from_cols(w.x_axis.truncate().normalize_or_zero(), w.y_axis.truncate().normalize_or_zero(), w.z_axis.truncate().normalize_or_zero());
        return Some(radians(&m));
    }
    if let Some(d) = triple("alignAlong") {
        return along(d).map(|f| radians(&f));
    }
    None
}

/// The axes for Set to Point (aim from the pivot of the other points to the last one), Set to Edge (X along it) or Set to
/// Face (Z along its normal), from what is selected. Radians, or None when nothing of that kind is.
pub fn from_selection(s: &mut Session, kind: &str) -> Option<[f64; 3]> {
    let items = crate::viewport::gather(&mut s.graph);
    let g = &s.graph;
    let mode = components::mode(g);
    let it = items.iter().find(|it| g.selection().contains(&it.xf) && !components::selected(g, it.shape, mode).is_empty())?;
    let ids = components::selected(g, it.shape, mode).to_vec();
    let m = &it.mesh;
    let world = |p: u32| it.world.transform_point3(m.position(p));
    let frame = match kind {
        "face" => {
            let faces = components::convert(m, mode, &ids, ComponentMode::Face, false);
            let n = faces.iter().map(|&f| m.face_area_vector(f)).sum::<DVec3>();
            facing(glam::DMat3::from_mat4(it.world).inverse().transpose() * n)
        }
        "edge" => {
            let edges = components::convert(m, mode, &ids, ComponentMode::Edge, false);
            let e = *edges.first()?;
            let pts = components::convert(m, ComponentMode::Edge, &[e], ComponentMode::Vertex, false);
            along(world(*pts.get(1)?) - world(*pts.first()?))
        }
        _ => {
            let pts = components::convert(m, mode, &ids, ComponentMode::Vertex, false);
            let last = world(*pts.last()?);
            let c = pts.iter().map(|&p| world(p)).sum::<DVec3>() / pts.len() as f64;
            // One point alone: from the object's centre to it.
            let from = if pts.len() > 1 { c } else { it.world.w_axis.truncate() };
            along(last - from)
        }
    }?;
    Some(radians(&frame))
}
