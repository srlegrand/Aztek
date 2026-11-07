//! How dynamics draw in the view: particles as points, spheres or blobs; fields, emitters and solvers as glyphs; and
//! the cloth vertices out of the simulation, marked so it is plain when the impossible state handling stepped in.

use aztek_scene::dynamics as dy;
use aztek_scene::transform::DagNode;
use aztek_scene::{Graph, NodeId, NO_NODE};
use glam::{DMat4, DVec3};

use crate::viewport::RigItem;

fn circle(c: DVec3, u: DVec3, v: DVec3, r: f64, segs: usize, out: &mut Vec<(DVec3, DVec3)>) {
    let at = |k: usize| {
        let a = std::f64::consts::TAU * k as f64 / segs as f64;
        c + (u * a.cos() + v * a.sin()) * r
    };
    for k in 0..segs {
        out.push((at(k), at(k + 1)));
    }
}

fn cross(p: DVec3, k: f64, out: &mut Vec<(DVec3, DVec3)>) {
    for ax in [DVec3::X, DVec3::Y, DVec3::Z] {
        out.push((p - ax * k, p + ax * k));
    }
}

/// Lines for a dynamics shape, or None when the node is not one: (lines, factory colour, kind for display colours).
pub fn glyph(g: &Graph, id: NodeId) -> Option<Vec<RigItem>> {
    let n = g.node(id)?;
    let ty = n.ty().name.clone();
    let w: DMat4 = n.data().matrix(DagNode::WORLD_MATRIX);
    let c = w.w_axis.truncate();
    let xf = aztek_scene::dag::transform_of(g, id);
    let item = |color: [u8; 4], segs: Vec<(DVec3, DVec3)>| RigItem { node: xf, shape: id, color, segs };
    let num = |name: &str, d: f64| n.ty().attr_index(name).and_then(|a| n.value(a).scalar()).unwrap_or(d);
    match ty.as_str() {
        dy::PARTICLES => {
            let st = dy::particle_state(g, id)?;
            let kind = num("particleRenderType", 3.0) as i64;
            let mut segs = Vec::new();
            let mut out_segs = Vec::new();
            for (i, p) in st.position.iter().enumerate() {
                let r = st.radius.get(i).copied().unwrap_or(0.1);
                let dst = if st.weight.get(i).is_some_and(|&w| w < 1.0) { &mut out_segs } else { &mut segs };
                match kind {
                    // Spheres and blobs: three rings at the radius, wider for blobs.
                    4 | 7 => {
                        let rr = if kind == 7 { r * 1.6 } else { r };
                        circle(*p, DVec3::X, DVec3::Y, rr, 8, dst);
                        circle(*p, DVec3::Y, DVec3::Z, rr, 8, dst);
                        circle(*p, DVec3::Z, DVec3::X, rr, 8, dst);
                    }
                    _ => cross(*p, r.max(1e-3) * 0.35, dst),
                }
            }
            let mut items = vec![item([90, 170, 255, 255], segs)];
            if !out_segs.is_empty() {
                items.push(RigItem { node: NO_NODE, shape: NO_NODE, color: [255, 140, 40, 255], segs: out_segs });
            }
            Some(items)
        }
        dy::FIELD => {
            let kind = num("fieldType", 0.0) as usize;
            let mut segs = Vec::new();
            let dir = n.ty().attr_index("direction").map(|a| n.value(a).as_vec3()).unwrap_or(DVec3::NEG_Y);
            let d = w.transform_vector3(dir).normalize_or(DVec3::NEG_Y);
            let (u, v) = d.any_orthonormal_pair();
            match kind {
                // Radial, newton: rings; vortex and volume axis: a ring round the axis with the axis.
                3 | 4 => {
                    circle(c, u, v, 0.5, 16, &mut segs);
                    circle(c, d, u, 0.5, 16, &mut segs);
                    circle(c, d, v, 0.5, 16, &mut segs);
                }
                7 | 8 => {
                    circle(c, u, v, 0.6, 16, &mut segs);
                    segs.push((c - d * 0.8, c + d * 0.8));
                }
                5 => {
                    for k in 0..6 {
                        let a = k as f64;
                        segs.push((c + u * (a * 0.15 - 0.4), c + u * (a * 0.15 - 0.33) + v * if k % 2 == 0 { 0.15 } else { -0.15 }));
                    }
                }
                _ => {
                    // An arrow the way it pushes.
                    let tip = c + d;
                    segs.push((c, tip));
                    segs.push((tip, tip - d * 0.25 + u * 0.12));
                    segs.push((tip, tip - d * 0.25 - u * 0.12));
                    segs.push((tip, tip - d * 0.25 + v * 0.12));
                    segs.push((tip, tip - d * 0.25 - v * 0.12));
                }
            }
            Some(vec![item([120, 220, 120, 255], segs)])
        }
        dy::EMITTER => {
            let mut segs = Vec::new();
            for ax in [DVec3::X, DVec3::Y, DVec3::Z, DVec3::new(1.0, 1.0, 0.0).normalize(), DVec3::new(-1.0, 1.0, 0.0).normalize(), DVec3::new(0.0, 1.0, 1.0).normalize()] {
                let a = w.transform_vector3(ax).normalize_or(ax) * 0.3;
                segs.push((c - a, c + a));
            }
            Some(vec![item([255, 210, 90, 255], segs)])
        }
        dy::SOLVER => {
            let mut segs = Vec::new();
            circle(c, DVec3::X, DVec3::Y, 0.3, 16, &mut segs);
            circle(c, DVec3::Y, DVec3::Z, 0.3, 16, &mut segs);
            circle(c, DVec3::Z, DVec3::X, 0.3, 16, &mut segs);
            Some(vec![item([180, 120, 255, 255], segs)])
        }
        _ => None,
    }
}

/// Cloth vertices out of the simulation: orange while out, yellow while blending back. Not pickable.
pub fn cloth_marks(g: &Graph) -> Vec<RigItem> {
    let mut out = Vec::new();
    for c in dy::cloths(g) {
        let Some(weights) = dy::cloth_weights(g, c) else { continue };
        if weights.iter().all(|&w| w >= 1.0) {
            continue;
        }
        let Some(mesh) = dy::cloth_mesh(g, c) else { continue };
        let Some(m) = g.node(mesh) else { continue };
        let Some(a) = m.ty().attr_index("outMesh") else { continue };
        let Some(points) = m.value(a).as_mesh().map(|x| x.positions().to_vec()) else { continue };
        let w = m.data().matrix(DagNode::WORLD_MATRIX);
        let edge = points.windows(2).take(64).map(|p| (p[0] - p[1]).length()).filter(|l| *l > 0.0).fold(f64::INFINITY, f64::min);
        let k = if edge.is_finite() { edge * 0.3 } else { 0.02 };
        let (mut out_segs, mut back_segs) = (Vec::new(), Vec::new());
        for (i, wt) in weights.iter().enumerate() {
            if *wt >= 1.0 {
                continue;
            }
            let Some(p) = points.get(i) else { continue };
            let at = w.transform_point3(*p);
            cross(at, k, if *wt <= 0.0 { &mut out_segs } else { &mut back_segs });
        }
        out.push(RigItem { node: NO_NODE, shape: NO_NODE, color: [255, 120, 30, 255], segs: out_segs });
        out.push(RigItem { node: NO_NODE, shape: NO_NODE, color: [255, 230, 60, 255], segs: back_segs });
    }
    out
}
