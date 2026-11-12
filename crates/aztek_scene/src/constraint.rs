//! Constraints as the reference has them: parent, point, orient, scale, aim, poleVector, geometry, normal and pointOnPoly.
//!
//! A constraint node blends its targets by weight and drives the object's channels from constraintTranslate,
//! constraintRotate and constraintScale. Each target has an element of `target` and a keyable weight attr
//! `<target>W<index>` added to the node; indices stay as they were when other targets are removed.
//!
//! The node has no place in the hierarchy under the object it drives, as the reference's has: the graph orders whole nodes, so
//! a child driving its parent would be a cycle. What it would read back from that object (rotate order, jointOrient,
//! translate) is copied to it by `sync`, and the constraint is found through its connections.
//!
//! tangentConstraint needs curves and is not here yet.

use std::any::Any;
use std::sync::Arc;

use aztek_geo::{bvh::MeshBvh, Mesh};
use aztek_ops::{fail, objects, ArgType, Args, CmdResult, Commands};
use glam::{DMat3, DMat4, DQuat, DVec2, DVec3, DVec4};
use serde_json::{json, Value};

use crate::attr_defs::{enum_attr, float_attr};
use crate::graph::{DrivenEdit, Graph};
use crate::node::{types, vec3_attr, EvalContext, NodeBehavior, NodeData, NodeType};
use crate::plug::{flags, AttrDef, NodeId, PlugRef, PlugType, PlugValue, NO_NODE};
use crate::rig::{asked, f, find, mat3, matrix_in, plug, set_vec3, vec3_of, world, Cmd, Joint};
use crate::session::Session;
use crate::transform::{matrix_to_euler, RotateOrder, Transform};

pub const CONSTRAINT_TYPES: &[&str] = &[
    "parentConstraint",
    "pointConstraint",
    "orientConstraint",
    "scaleConstraint",
    "aimConstraint",
    "poleVectorConstraint",
    "geometryConstraint",
    "normalConstraint",
    "pointOnPolyConstraint",
    "tangentConstraint",
];

const PARENT: usize = 0;
const POINT: usize = 1;
const ORIENT: usize = 2;
const SCALE: usize = 3;
const AIM: usize = 4;
const POLE: usize = 5;
const GEOMETRY: usize = 6;
const NORMAL: usize = 7;
const POINT_ON_POLY: usize = 8;
const TANGENT: usize = 9;

/// The rotation in a matrix. A mirrored one is a rotation times a scale of -1 on every axis, as the reference takes it apart, so a
/// constraint under a mirrored parent or on a mirrored target always works in true rotations.
fn rot3(m: &DMat4) -> DMat3 {
    let r = crate::rig::rot3(m);
    if r.determinant() < 0.0 {
        r * -1.0
    } else {
        r
    }
}

const T_MATRIX: usize = 0;
const T_OFFSET_T: usize = 1;
const T_OFFSET_R: usize = 2;
const T_GEOMETRY: usize = 3;
const PARENT_MATRIX: usize = 4;
const OUT_T: usize = 5;
const OUT_R: usize = 6;
const OUT_S: usize = 7;
const OFFSET: usize = 8;
const REST_T: usize = 9;
const REST_R: usize = 10;
const REST_S: usize = 11;
const ENABLE_REST: usize = 12;
const INTERP: usize = 13;
const NODE_STATE: usize = 14;
const ROTATE_ORDER: usize = 15;
const JOINT_ORIENT: usize = 16;
const ROTATE_AXIS: usize = 17;
const POSITION: usize = 18;
const PIVOT: usize = 19;
const AIM_VECTOR: usize = 20;
const UP_VECTOR: usize = 21;
const WORLD_UP_VECTOR: usize = 22;
const WORLD_UP_TYPE: usize = 23;
const WORLD_UP_MATRIX: usize = 24;
const T_CURVE: usize = 25;
const T_PIVOT: usize = 26;
// A target only its position counts for is read plug by plug, as the reference reads it: tpm * (tt + trp + trt). Its rotation may depend on what
// the constraint drives without that being a cycle.
const T_PARENT: usize = 27;
const T_TRANSLATE: usize = 28;
const T_ROTATE_TRANSLATE: usize = 29;
const T_OFFSET_PARENT: usize = 30;
const T_SCALE: usize = 31;
/// The driven object's offsetParentMatrix: what it is placed in is its parent's frame times this.
const OFFSET_PARENT: usize = 32;

/// A target's world matrix with its origin moved to its rotate pivot, which is where the reference constrains to.
fn at_pivot(m: DMat4, rp: DVec3) -> DMat4 {
    let mut out = m;
    out.w_axis = m.transform_point3(rp).extend(1.0);
    out
}

const WORLD_UP_NAMES: [&str; 5] = ["scene", "object", "objectrotation", "vector", "none"];

pub struct ConstraintNode {
    kind: usize,
    /// Computed at least once. A node blocked from the start holds its rest values.
    ran: bool,
}

/// A matrix's scale as a scale constraint reads it: a mirrored one has its Z negative, which offsets of (1, 1, -1) in rigs from the reference show.
fn scale_of(m: &DMat4) -> DVec3 {
    let m3 = DMat3::from_mat4(*m);
    let z = if m3.determinant() < 0.0 { -1.0 } else { 1.0 };
    DVec3::new(m3.x_axis.length(), m3.y_axis.length(), z * m3.z_axis.length())
}

/// Blends rotations by weight. interpType 2 (Shortest) and the default Average take each the short way round from the
/// first; 3 (Longest) takes the others the long way.
fn quat_avg(qs: &[(DQuat, f64)], interp: i64) -> DQuat {
    let mut acc = DVec4::ZERO;
    let first = qs.first().map(|q| q.0).unwrap_or(DQuat::IDENTITY);
    for (i, &(q, w)) in qs.iter().enumerate() {
        let short = if q.dot(first) < 0.0 { -q } else { q };
        let q = if interp == 3 && i > 0 { -short } else { short };
        acc += DVec4::new(q.x, q.y, q.z, q.w) * w;
    }
    if acc.length_squared() < 1e-18 {
        return first;
    }
    DQuat::from_xyzw(acc.x, acc.y, acc.z, acc.w).normalize()
}

struct Weighted {
    index: i32,
    matrix: DMat4,
    weight: f64,
}

/// The attr `<target><letter><index>` on the node, by what it ends with.
fn suffix_attr(ty: &NodeType, letter: char, index: i32) -> Option<usize> {
    let suffix = format!("{letter}{index}");
    ty.attrs.iter().position(|a| a.has(flags::USER) && a.name.ends_with(&suffix))
}

fn targets_in(d: &NodeData) -> Vec<Weighted> {
    let weight = |i: i32| suffix_attr(d.ty(), 'W', i).map(|a| d.float(a)).unwrap_or(1.0);
    let vec_at = |attr: usize, i: i32| d.element(attr, i).map_or(DVec3::ZERO, |p| p.as_vec3());
    let mut out: Vec<Weighted> = d
        .elements(T_MATRIX)
        .iter()
        .map(|(i, v)| Weighted { index: *i, matrix: at_pivot(v.as_matrix(), vec_at(T_PIVOT, *i)), weight: weight(*i) })
        .collect();
    for (i, pm) in d.elements(T_PARENT) {
        let opm = d.element(T_OFFSET_PARENT, *i).map_or(DMat4::IDENTITY, |m| m.as_matrix());
        let at = vec_at(T_TRANSLATE, *i) + vec_at(T_PIVOT, *i) + vec_at(T_ROTATE_TRANSLATE, *i);
        out.push(Weighted { index: *i, matrix: pm.as_matrix() * opm * DMat4::from_translation(at), weight: weight(*i) });
    }
    out.sort_by_key(|x| x.index);
    out
}

/// What each target contributes after normalizing, and what is left to the rest position. Weights that sum below 1
/// blend toward the rest position when it is enabled. None when nothing applies and the object should stay as it is.
fn shares(ts: &[Weighted], rest_on: bool) -> Option<(Vec<f64>, f64)> {
    let total: f64 = ts.iter().map(|t| t.weight.max(0.0)).sum();
    if rest_on {
        let norm = total.max(1.0);
        Some((ts.iter().map(|t| t.weight.max(0.0) / norm).collect(), (1.0 - total).max(0.0)))
    } else if total > 1e-12 {
        Some((ts.iter().map(|t| t.weight.max(0.0) / total).collect(), 0.0))
    } else {
        None
    }
}

/// The world rotation that puts local `aim` along `dir` and local `up` as near `world_up` as it can go.
fn aim_frame(dir: DVec3, world_up: DVec3, aim: DVec3, up: DVec3) -> Option<DMat3> {
    let a = dir.normalize_or_zero();
    let la = aim.normalize_or_zero();
    if a == DVec3::ZERO || la == DVec3::ZERO {
        return None;
    }
    let perpendicular = |v: DVec3, x: DVec3| -> DVec3 {
        let p = v - x * v.dot(x);
        if p.length() > 1e-9 {
            return p.normalize();
        }
        let other = if x.x.abs() < 0.9 { DVec3::X } else { DVec3::Y };
        (other - x * other.dot(x)).normalize()
    };
    let lu = perpendicular(up, la);
    let u = perpendicular(world_up, a);
    Some(DMat3::from_cols(a, u, a.cross(u)) * DMat3::from_cols(la, lu, la.cross(lu)).transpose())
}

/// The up direction for an aim or normal constraint at `eye`, in world space. None for worldUpType none.
fn world_up_at(d: &NodeData, eye: DVec3) -> Option<DVec3> {
    let m = d.matrix(WORLD_UP_MATRIX);
    match d.int(WORLD_UP_TYPE) {
        0 => Some(DVec3::Y),
        1 => Some(m.w_axis.truncate() - eye),
        2 => Some(crate::rig::rot3(&m) * d.vec3(WORLD_UP_VECTOR)),
        3 => Some(d.vec3(WORLD_UP_VECTOR)),
        _ => None,
    }
}

/// The driven object's rotation in its parent's frame, jointOrient and rotateAxis included, for rotate `e`.
fn local_frame(d: &NodeData, e: DVec3) -> DMat3 {
    let order = RotateOrder::from_index(d.int(ROTATE_ORDER));
    mat3(d.vec3(JOINT_ORIENT), RotateOrder::Xyz) * mat3(e, order) * mat3(d.vec3(ROTATE_AXIS), RotateOrder::Xyz)
}

/// The rotate that gives `frame` in the parent's frame.
fn rotate_for(d: &NodeData, frame: &DMat3) -> DVec3 {
    let order = RotateOrder::from_index(d.int(ROTATE_ORDER));
    let jo = mat3(d.vec3(JOINT_ORIENT), RotateOrder::Xyz);
    let ra = mat3(d.vec3(ROTATE_AXIS), RotateOrder::Xyz);
    matrix_to_euler(&(jo.transpose() * *frame * ra.transpose()), order)
}

/// Where a point on the surface lies, in the mesh's own space, with its smooth normal.
fn closest_on(m: &Mesh, p: DVec3) -> Option<(DVec3, DVec3)> {
    let bvh = MeshBvh::new(m);
    let c = bvh.closest_point(p)?;
    let pts = m.face_points(c.face);
    let mut n = DVec3::ZERO;
    for &q in &pts {
        // Vertex normals near the point, weighted toward the nearest corners.
        let w = 1.0 / ((m.position(q) - c.point).length_squared() + 1e-9);
        let mut vn = DVec3::ZERO;
        for f in m.point_faces(q) {
            vn += m.face_normal(f);
        }
        n += vn.normalize_or_zero() * w;
    }
    let n = if n.length() > 1e-12 { n.normalize() } else { m.face_normal(c.face) };
    Some((c.point, n))
}

/// The point at a UV in the mesh's own space, with its normal and the direction U grows in.
fn point_at_uv(m: &Mesh, uv: DVec2) -> Option<(DVec3, DVec3, DVec3)> {
    let layer = m.corner_attrs().get::<[f32; 2]>(crate::uv_ops::UV);
    let uv_at = |c: u32| layer.get(c as usize).map(|u| DVec2::new(u[0] as f64, u[1] as f64)).unwrap_or(DVec2::ZERO);
    let tris = aztek_geo::triangulate::triangulate(m, &Default::default());
    let mut best: Option<(f64, (DVec3, DVec3, DVec3))> = None;
    for (k, tri) in tris.corners.chunks_exact(3).enumerate() {
        let (a, b, c) = (uv_at(tri[0]), uv_at(tri[1]), uv_at(tri[2]));
        let (e1, e2, q) = (b - a, c - a, uv - a);
        let det = e1.x * e2.y - e1.y * e2.x;
        if det.abs() < 1e-14 {
            continue;
        }
        let v = (q.x * e2.y - q.y * e2.x) / det;
        let w = (e1.x * q.y - e1.y * q.x) / det;
        let u = 1.0 - v - w;
        // How far outside the triangle: 0 when inside.
        let miss = (-u).max(-v).max(-w).max(0.0);
        if best.as_ref().is_some_and(|(d, _)| *d <= miss) {
            continue;
        }
        let (pa, pb, pc) = (m.position(m.corner_point(tri[0])), m.position(m.corner_point(tri[1])), m.position(m.corner_point(tri[2])));
        let point = pa * u + pb * v + pc * w;
        let (dp1, dp2) = (pb - pa, pc - pa);
        let du = (dp1 * e2.y - dp2 * e1.y) / det;
        let normal = m.face_normal(tris.faces[k]);
        best = Some((miss, (point, normal, du)));
        if miss == 0.0 {
            break;
        }
    }
    best.map(|b| b.1)
}

impl NodeBehavior for ConstraintNode {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        // Anything but Normal: the outputs stay as they were.
        if d.int(NODE_STATE) != 0 {
            if !self.ran {
                self.ran = true;
                let (t, e, s) = (d.vec3(REST_T), d.vec3(REST_R), d.vec3(REST_S));
                d.set_output(OUT_T, PlugValue::Vec3(t));
                d.set_output(OUT_R, PlugValue::Vec3(e));
                d.set_output(OUT_S, PlugValue::Vec3(s));
            }
            return;
        }
        self.ran = true;
        let ts = targets_in(d);
        let rest_on = d.bool(ENABLE_REST);
        let Some((share, rest)) = shares(&ts, rest_on) else { return };
        // The driven object sits in its parent's frame moved by its offsetParentMatrix, which the reference counts in.
        let pm = d.matrix(PARENT_MATRIX) * d.matrix(OFFSET_PARENT);
        let pinv = pm.inverse();
        let pr = rot3(&pm);
        let offset = d.vec3(OFFSET);
        let pivot = d.vec3(PIVOT);
        let interp = d.int(INTERP);
        let (rest_t, rest_e, rest_s) = (d.vec3(REST_T), d.vec3(REST_R), d.vec3(REST_S));
        let mut t = rest_t;
        let mut e = rest_e;
        let mut s = rest_s;
        let rest_q = DQuat::from_mat3(&local_frame(d, rest_e));
        let with_rest = |mut qs: Vec<(DQuat, f64)>| {
            if rest > 0.0 || qs.is_empty() {
                qs.push((rest_q, rest.max(1e-9)));
            }
            DMat3::from_quat(quat_avg(&qs, interp))
        };
        match self.kind {
            PARENT => {
                let mut sum = rest_t * rest;
                let mut qs = Vec::new();
                for (x, &w) in ts.iter().zip(&share) {
                    let off_t = d.element(T_OFFSET_T, x.index).map(|v| v.as_vec3()).unwrap_or(DVec3::ZERO);
                    let off_r = d.element(T_OFFSET_R, x.index).map(|v| v.as_vec3()).unwrap_or(DVec3::ZERO);
                    sum += (pinv.transform_point3(x.matrix.transform_point3(off_t)) - pivot) * w;
                    qs.push((DQuat::from_mat3(&(pr.transpose() * rot3(&x.matrix) * mat3(off_r, RotateOrder::from_index(d.int(ROTATE_ORDER))))), w));
                }
                t = sum;
                e = rotate_for(d, &with_rest(qs));
            }
            POINT => {
                let mut sum = rest_t * rest;
                for (x, &w) in ts.iter().zip(&share) {
                    sum += (pinv.transform_point3(x.matrix.w_axis.truncate()) - pivot + offset) * w;
                }
                t = sum;
            }
            ORIENT => {
                let qs: Vec<(DQuat, f64)> = ts.iter().zip(&share).map(|(x, &w)| (DQuat::from_mat3(&(pr.transpose() * rot3(&x.matrix) * mat3(offset, RotateOrder::from_index(d.int(ROTATE_ORDER))))), w)).collect();
                e = rotate_for(d, &with_rest(qs));
            }
            SCALE => {
                let ps = scale_of(&pm);
                let ps = DVec3::new(if ps.x.abs() < 1e-12 { 1e-12 } else { ps.x }, if ps.y.abs() < 1e-12 { 1e-12 } else { ps.y }, if ps.z.abs() < 1e-12 { 1e-12 } else { ps.z });
                let mut sum = rest_s * rest;
                for (x, &w) in ts.iter().zip(&share) {
                    let o = if offset == DVec3::ZERO { DVec3::ONE } else { offset };
                    let own = d.element(T_SCALE, x.index).map_or(DVec3::ONE, |v| v.as_vec3());
                    sum += scale_of(&x.matrix) * own / ps * o * w;
                }
                s = sum;
            }
            AIM => {
                let eye = pm.transform_point3(d.vec3(POSITION) + pivot);
                let total: f64 = share.iter().sum();
                if total > 1e-12 {
                    let mut at = DVec3::ZERO;
                    for (x, &w) in ts.iter().zip(&share) {
                        at += x.matrix.w_axis.truncate() * w;
                    }
                    let dir = at / total - eye;
                    if let Some(frame) = aim_rotation(d, dir, eye, &pm, rest_e) {
                        let frame = frame * mat3(offset, RotateOrder::from_index(d.int(ROTATE_ORDER)));
                        e = rotate_for(d, &with_rest(vec![(DQuat::from_mat3(&frame), total)]));
                    }
                }
            }
            POLE => {
                // In the space of the start joint's parent, as the handle's poleVector is.
                let start = d.vec3(POSITION);
                let mut sum = rest_t * rest;
                for (x, &w) in ts.iter().zip(&share) {
                    sum += (pinv.transform_point3(x.matrix.w_axis.truncate()) - start + offset) * w;
                }
                t = sum;
            }
            GEOMETRY => {
                let wanted = pm.transform_point3(rest_t + pivot);
                let total: f64 = share.iter().sum();
                let mut sum = DVec3::ZERO;
                for (x, &w) in ts.iter().zip(&share) {
                    let on = d.element(T_GEOMETRY, x.index).and_then(|v| v.as_mesh()).and_then(|m| closest_on(m, x.matrix.inverse().transform_point3(wanted)));
                    let at = on.map(|(p, _)| x.matrix.transform_point3(p)).unwrap_or(wanted);
                    sum += at * w;
                }
                if total > 1e-12 {
                    t = pinv.transform_point3(sum / total) - pivot;
                }
            }
            NORMAL => {
                let eye = pm.transform_point3(d.vec3(POSITION) + pivot);
                let total: f64 = share.iter().sum();
                let mut dir = DVec3::ZERO;
                for (x, &w) in ts.iter().zip(&share) {
                    let on = d.element(T_GEOMETRY, x.index).and_then(|v| v.as_mesh()).and_then(|m| closest_on(m, x.matrix.inverse().transform_point3(eye)));
                    if let Some((_, n)) = on {
                        dir += (x.matrix.inverse().transpose().transform_vector3(n)).normalize_or_zero() * w;
                    }
                }
                if total > 1e-12 {
                    if let Some(frame) = aim_rotation(d, dir, eye, &pm, rest_e) {
                        let frame = frame * mat3(offset, RotateOrder::from_index(d.int(ROTATE_ORDER)));
                        e = rotate_for(d, &with_rest(vec![(DQuat::from_mat3(&frame), total)]));
                    }
                }
            }
            TANGENT => {
                // Aim along the target curves' tangents at the points nearest the object.
                let eye = pm.transform_point3(d.vec3(POSITION) + pivot);
                let total: f64 = share.iter().sum();
                let mut dir = DVec3::ZERO;
                for (x, &w) in ts.iter().zip(&share) {
                    let c = match d.element(T_CURVE, x.index) {
                        Some(PlugValue::Data(Some(k))) => k.as_any().downcast_ref::<crate::curves::CurveData>(),
                        _ => None,
                    };
                    if let Some(c) = c {
                        dir += c.0.tangent(c.0.closest_param(eye)).normalize_or_zero() * w;
                    }
                }
                if total > 1e-12 {
                    if let Some(frame) = aim_rotation(d, dir, eye, &pm, rest_e) {
                        let frame = frame * mat3(offset, RotateOrder::from_index(d.int(ROTATE_ORDER)));
                        e = rotate_for(d, &with_rest(vec![(DQuat::from_mat3(&frame), total)]));
                    }
                }
            }
            _ => {
                // Point on poly: the place at each target's UV; Y along the normal, X the way U grows.
                let mut sum = rest_t * rest;
                let mut qs = Vec::new();
                for (x, &w) in ts.iter().zip(&share) {
                    let (u, v) = (suffix_attr(d.ty(), 'U', x.index), suffix_attr(d.ty(), 'V', x.index));
                    let uv = DVec2::new(u.map(|a| d.float(a)).unwrap_or(0.0), v.map(|a| d.float(a)).unwrap_or(0.0));
                    let Some((p, n, du)) = d.element(T_GEOMETRY, x.index).and_then(|g| g.as_mesh()).and_then(|m| point_at_uv(m, uv)) else { continue };
                    sum += (pinv.transform_point3(x.matrix.transform_point3(p)) - pivot) * w;
                    let nw = x.matrix.inverse().transpose().transform_vector3(n);
                    let up = rot3(&x.matrix) * du;
                    if let Some(frame) = aim_frame(nw, up, DVec3::Y, DVec3::X) {
                        qs.push((DQuat::from_mat3(&(pr.transpose() * frame)), w));
                    }
                }
                t = sum;
                e = rotate_for(d, &with_rest(qs));
            }
        }
        d.set_output(OUT_T, PlugValue::Vec3(t));
        d.set_output(OUT_R, PlugValue::Vec3(e));
        d.set_output(OUT_S, PlugValue::Vec3(s));
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// The aim rotation in the parent's frame, where a mirrored parent cancels out as it does in the reference. With worldUpType none, the
/// shortest turn from the rest pose.
fn aim_rotation(d: &NodeData, dir: DVec3, eye: DVec3, pm: &DMat4, rest_e: DVec3) -> Option<DMat3> {
    let aim = d.vec3(AIM_VECTOR);
    let up = d.vec3(UP_VECTOR);
    let inv = pm.inverse();
    let dir = inv.transform_vector3(dir);
    // With no direction to go on, a zero world up vector among them, it is the shortest turn: the reference offsets stored in such rigs say so.
    match world_up_at(d, eye).filter(|u| u.length_squared() > 1e-18) {
        Some(world_up) => aim_frame(dir, inv.transform_vector3(world_up), aim, up),
        None => {
            let rest = local_frame(d, rest_e);
            let (from, to) = ((rest * aim).normalize_or_zero(), dir.normalize_or_zero());
            if from == DVec3::ZERO || to == DVec3::ZERO {
                return None;
            }
            Some(DMat3::from_quat(DQuat::from_rotation_arc(from, to)) * rest)
        }
    }
}

pub(crate) fn register_types() {
    let store = flags::INPUT | flags::STORABLE;
    for (k, name) in CONSTRAINT_TYPES.iter().enumerate() {
        let array_vec = |n: &str, short: &str, doc: &str| {
            let mut a = vec3_attr(n, short, DVec3::ZERO, store | flags::ARRAY | flags::HIDDEN, doc);
            a.child_names = Default::default();
            a.child_short = Default::default();
            a
        };
        let out = flags::OUTPUT | flags::HIDDEN;
        types::register_type(NodeType {
            name: name.to_string(),
            icon: "link".into(),
            doc: "Drives an object from its targets.".into(),
            own_attrs: vec![
                matrix_in("target.targetMatrix", "tgm", true, "Each target's world matrix."),
                array_vec("target.targetOffsetTranslate", "tot", "Parent constraint: the object's place in the target's frame."),
                array_vec("target.targetOffsetRotate", "tor", "Parent constraint: the object's rotation in the target's frame, Euler XYZ degrees."),
                AttrDef {
                    name: "target.targetGeometry".into(),
                    short_name: "tgg".into(),
                    ty: PlugType::Mesh,
                    default_value: PlugValue::Mesh(None),
                    flags: flags::INPUT | flags::HIDDEN | flags::ARRAY,
                    doc: "Each target's surface, in its own space.".into(),
                    ..Default::default()
                },
                matrix_in("constraintParentMatrix", "cpm", false, "World matrix of the driven object's parent."),
                vec3_attr("constraintTranslate", "ct", DVec3::ZERO, out, "Translate for the driven object."),
                vec3_attr("constraintRotate", "cr", DVec3::ZERO, out, "Rotate for the driven object."),
                vec3_attr("constraintScale", "cs", DVec3::ONE, out, "Scale for the driven object."),
                vec3_attr("offset", "o", if k == SCALE { DVec3::ONE } else { DVec3::ZERO }, store, "Added to the result: a move, a rotation in degrees, or for scale a factor."),
                vec3_attr("restTranslate", "rst", DVec3::ZERO, store, "Where the object goes when the weights leave room."),
                vec3_attr("restRotate", "rsrr", DVec3::ZERO, store, "How the object turns when the weights leave room."),
                vec3_attr("restScale", "rss", DVec3::ONE, store, "The object's scale when the weights leave room."),
                AttrDef { short_name: "erp".into(), ..crate::attr_defs::bool_attr("enableRestPosition", true, "Weights that sum below 1 blend toward the rest position.") },
                AttrDef { short_name: "int".into(), ..enum_attr("interpType", &["No Flip", "Average", "Shortest", "Longest", "Cache"], 1, "How rotations of several targets blend.") },
                AttrDef { short_name: "nds".into(), ..enum_attr("nodeState", &["Normal", "HasNoEffect", "Blocking", "Waiting-Normal", "Waiting-HasNoEffect", "Waiting-Blocking"], 0, "Anything but Normal leaves the outputs as they are.") },
                AttrDef { short_name: "cro".into(), ..enum_attr("constraintRotateOrder", &["xyz", "yzx", "zxy", "xzy", "yxz", "zyx"], 0, "The driven object's rotate order.") },
                vec3_attr("constraintJointOrient", "cjo", DVec3::ZERO, store | flags::HIDDEN, "The driven joint's jointOrient."),
                vec3_attr("constraintRotateAxis", "cra", DVec3::ZERO, store | flags::HIDDEN, "The driven object's rotateAxis."),
                vec3_attr("constraintPosition", "cps", DVec3::ZERO, store | flags::HIDDEN, "The driven object's translate (a pole vector constraint's: its start joint's)."),
                vec3_attr("constraintRotatePivot", "crp", DVec3::ZERO, store | flags::HIDDEN, "The driven object's rotatePivot plus rotatePivotTranslate."),
                vec3_attr("aimVector", "a", DVec3::X, store, "The local axis that points at the target."),
                vec3_attr("upVector", "u", DVec3::Y, store, "The local axis that points up."),
                vec3_attr("worldUpVector", "wu", DVec3::Y, store, "The world direction up means, for worldUpType vector and objectrotation."),
                AttrDef { short_name: "wut".into(), ..enum_attr("worldUpType", &["Scene Up", "Object Up", "Object Rotation Up", "Vector", "None"], 3, "What up means.") },
                matrix_in("worldUpMatrix", "wum", false, "The world up object's world matrix."),
                AttrDef {
                    name: "target.targetCurve".into(),
                    short_name: "tgc".into(),
                    ty: PlugType::Data,
                    default_value: PlugValue::Data(None),
                    flags: flags::INPUT | flags::HIDDEN | flags::ARRAY,
                    doc: "Tangent constraint: each target's curve, world space.".into(),
                    ..Default::default()
                },
                array_vec("target.targetRotatePivot", "trp", "Each target's rotate pivot: the place on it that constrains."),
                matrix_in("target.targetParentMatrix", "tpm", true, "The parent matrix of a target only its position counts for."),
                array_vec("target.targetTranslate", "tt", "That target's translate."),
                array_vec("target.targetRotateTranslate", "trt", "That target's rotatePivotTranslate."),
                matrix_in("target.targetOffsetParentMatrix", "topm", true, "That target's offsetParentMatrix."),
                array_vec("target.targetScale", "ts", "A scale constraint target's scale."),
                matrix_in("constraintOffsetParentMatrix", "copm", false, "The driven object's offsetParentMatrix."),
            ],
            create: Some(Arc::new(move || Box::new(ConstraintNode { kind: k, ran: false }))),
            ..Default::default()
        })
        .expect(name);
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Keeping in step

/// Where a constraint rests, from a file: what the command takes from the object as it is. Its driver, if any, gives way.
pub(crate) fn set_rest(g: &mut Graph, c: NodeId, t: Option<DVec3>, r: Option<DVec3>) -> CmdResult<()> {
    for (attr, v) in [(REST_T, t), (REST_R, r)] {
        if let Some(v) = v {
            let p = plug(c, attr);
            if g.source(&p).is_some() {
                g.disconnect(&p);
            }
            set_vec3(g, c, attr, v)?;
        }
    }
    Ok(())
}

/// The node a constraint drives. A constraint feeding a geometry constraint drives what that one drives.
pub(crate) fn driven_of(g: &Graph, c: NodeId) -> Option<NodeId> {
    for attr in [OUT_T, OUT_R, OUT_S] {
        let mut dsts = g.destinations(&plug(c, attr));
        for k in 0..3 {
            dsts.extend(g.destinations(&PlugRef::component(c, attr as i32, k)));
        }
        if let Some(d) = dsts.first() {
            if kind_of(g, d.node).is_some() {
                return driven_of(g, d.node);
            }
            // Blended with animation: what the pairBlend drives is the object.
            if g.node(d.node).is_some_and(|n| n.ty().name == "pairBlend") {
                let t = g.node(d.node).unwrap().type_arc().clone();
                let found = ["outTranslate", "outRotate"].iter().filter_map(|a| t.attr_index(a)).find_map(|a| {
                    let mut out = g.destinations(&plug(d.node, a));
                    for k in 0..3 {
                        out.extend(g.destinations(&PlugRef::component(d.node, a as i32, k)));
                    }
                    out.first().map(|x| x.node)
                });
                return found;
            }
            return Some(d.node);
        }
    }
    None
}

/// Whether a constraint drives any of the object's channels.
pub(crate) fn is_constrained(g: &Graph, id: NodeId) -> bool {
    g.connections().iter().any(|(d, s)| d.node == id && kind_of(g, s.node).is_some())
}

fn kind_of(g: &Graph, c: NodeId) -> Option<usize> {
    let name = g.node(c)?.ty().name.clone();
    CONSTRAINT_TYPES.iter().position(|t| *t == name)
}

fn set_if_changed(g: &mut Graph, c: NodeId, attr: usize, v: DVec3, changed: &mut bool) {
    if vec3_of(g, c, attr) != v {
        g.set_derived(&plug(c, attr), &PlugValue::Vec3(v));
        *changed = true;
    }
}

/// Copies what the constraint reads from the object it drives. True when anything changed.
pub(crate) fn sync(g: &mut Graph, c: NodeId) -> bool {
    let (Some(kind), Some(driven)) = (kind_of(g, c), driven_of(g, c)) else { return false };
    sync_from(g, c, kind, driven)
}

fn sync_from(g: &mut Graph, c: NodeId, kind: usize, driven: NodeId) -> bool {
    let mut changed = false;
    if kind == POLE {
        let Some(start) = crate::ik::start_of(g, driven) else { return false };
        let t = vec3_of(g, start, Transform::TRANSLATE);
        set_if_changed(g, c, POSITION, t, &mut changed);
        follow_parent(g, c, start, &mut changed);
        return changed;
    }
    let n = g.node(driven).unwrap().data();
    let order = n.int(Transform::ROTATE_ORDER);
    let t = n.vec3(Transform::TRANSLATE);
    let pivot = n.vec3(Transform::ROTATE_PIVOT) + n.vec3(Transform::ROTATE_PIVOT_TRANSLATE);
    let ra = n.vec3(Transform::ROTATE_AXIS);
    let jo = if n.ty().is_a("joint") { n.vec3(Joint::JOINT_ORIENT) } else { DVec3::ZERO };
    if g.node(c).unwrap().data().int(ROTATE_ORDER) != order {
        g.set_derived(&plug(c, ROTATE_ORDER), &PlugValue::Int(order));
        changed = true;
    }
    set_if_changed(g, c, JOINT_ORIENT, jo, &mut changed);
    set_if_changed(g, c, ROTATE_AXIS, ra, &mut changed);
    set_if_changed(g, c, PIVOT, pivot, &mut changed);
    if kind == AIM || kind == NORMAL || kind == TANGENT {
        // Where the object's translate comes from another node (a point constraint, say), the constraint reads it
        // from there in the same pass; copying it after each evaluation would cost another pass every frame.
        if !follow_translate_source(g, c, driven, &mut changed) {
            set_if_changed(g, c, POSITION, t, &mut changed);
        }
    }
    follow_parent(g, c, driven, &mut changed);
    changed
}

/// Connects each part of constraintPosition to what drives that part of `driven`'s translate. False, with any such
/// connections removed, when a part isn't driven or reading it would loop back.
fn follow_translate_source(g: &mut Graph, c: NodeId, driven: NodeId, changed: &mut bool) -> bool {
    let whole = g.source(&plug(driven, Transform::TRANSLATE));
    let mut sources = Vec::with_capacity(3);
    for k in 0..3 {
        let part = g.source(&PlugRef::component(driven, Transform::TRANSLATE as i32, k)).or_else(|| whole.map(|w| if w.component < 0 { PlugRef { component: k, ..w } } else { w }));
        match part {
            Some(src) if src.node != driven && src.node != c => sources.push(src),
            _ => break,
        }
    }
    let mut all = sources.len() == 3;
    // Already reading them: nothing to check again.
    if all && (0..3).all(|k| g.source(&PlugRef::component(c, POSITION as i32, k)) == Some(sources[k as usize])) {
        return true;
    }
    all = all && (0..3).all(|k| !g.reads_depend_on(&sources[k as usize], &PlugRef::component(c, POSITION as i32, k)));
    for k in 0..3 {
        let dst = PlugRef::component(c, POSITION as i32, k);
        let want = if all { Some(sources[k as usize]) } else { None };
        if g.source(&dst) != want {
            match want {
                Some(src) => g.connect_derived(&src, &dst),
                None => g.disconnect_derived(&dst),
            }
            *changed = true;
        }
    }
    all
}

/// The constraint's parent matrix comes from the parent of `node`, whichever that is now.
fn follow_parent(g: &mut Graph, c: NodeId, node: NodeId, changed: &mut bool) {
    let p = g.parent(node);
    let wanted = (p != NO_NODE).then(|| plug(p, Transform::WORLD_MATRIX));
    let input = plug(c, PARENT_MATRIX);
    if g.source(&input) != wanted {
        match wanted {
            Some(src) => g.connect_derived(&src, &input),
            None => {
                g.disconnect_derived(&input);
                g.set_derived(&input, &PlugValue::Matrix(DMat4::IDENTITY));
            }
        }
        *changed = true;
    }
    let opm = plug(node, Transform::OFFSET_PARENT_MATRIX);
    let input = plug(c, OFFSET_PARENT);
    if g.source(&input) != Some(opm) {
        g.connect_derived(&opm, &input);
        *changed = true;
    }
}

/// A move of an object on a geometry constraint: where it wants to be, which the constraint brings to the surface.
pub(crate) fn set_wanted_position(g: &mut Graph, c: NodeId, v: DVec3) -> DrivenEdit {
    match set_vec3(g, c, REST_T, v) {
        Ok(()) => DrivenEdit::Handled,
        Err(_) => DrivenEdit::Reject,
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Targets

/// A target name as written (pCube1.vtx[3] for point on poly): the node and the component part.
fn split_target<'a>(name: &'a str) -> (&'a str, Option<&'a str>) {
    match name.find('.') {
        Some(d) if name[d..].contains('[') => (&name[..d], Some(&name[d + 1..])),
        _ => (name, None),
    }
}

pub(crate) fn target_nodes(g: &Graph, c: NodeId) -> Vec<(i32, NodeId)> {
    let mut out: Vec<(i32, NodeId)> = [T_MATRIX, T_PARENT].iter().flat_map(|&a| g.elements(&plug(c, a))).filter_map(|e| g.source(&e).map(|s| (e.element, s.node))).collect();
    out.sort();
    out
}

fn alias_stem(g: &Graph, t: NodeId) -> String {
    g.node(t).map(|n| n.name().replace([':', '|'], "_")).unwrap_or_default()
}

fn index_in(component: &str) -> Option<(String, u32)> {
    let open = component.find('[')?;
    let close = component.find(']')?;
    let i = component[open + 1..close].parse().ok()?;
    Some((component[..open].to_string(), i))
}

/// The UV a point on poly target starts at: a vertex's first, the middle of an edge or face, or else the UV closest to
/// the driven object.
fn uv_of_component(m: &Mesh, component: Option<&str>, near: DVec3) -> DVec2 {
    let layer = m.corner_attrs().get::<[f32; 2]>(crate::uv_ops::UV);
    let uv_at = |c: u32| layer.get(c as usize).map(|u| DVec2::new(u[0] as f64, u[1] as f64)).unwrap_or(DVec2::ZERO);
    let mean = |corners: &[u32]| corners.iter().map(|&c| uv_at(c)).sum::<DVec2>() / corners.len().max(1) as f64;
    if let Some((kind, i)) = component.and_then(index_in) {
        match kind.as_str() {
            "vtx" => {
                let found = (0..m.corner_capacity()).find(|&c| m.corner_valid(c) && m.corner_point(c) == i);
                if let Some(c) = found {
                    return uv_at(c);
                }
            }
            "f" => return mean(&m.face_corners(i)),
            "e" => {
                let found = (0..m.corner_capacity()).find(|&c| m.corner_valid(c) && m.corner_edge(c) == i);
                if let Some(c) = found {
                    return (uv_at(c) + uv_at(m.corner_next(c))) * 0.5;
                }
            }
            _ => {}
        }
    }
    let Some((p, _)) = closest_on(m, near) else { return DVec2::ZERO };
    let mut best = (f64::MAX, DVec2::ZERO);
    for c in 0..m.corner_capacity() {
        if m.corner_valid(c) {
            let d = (m.position(m.corner_point(c)) - p).length_squared();
            if d < best.0 {
                best = (d, uv_at(c));
            }
        }
    }
    best.1
}

fn add_float(g: &mut Graph, c: NodeId, name: &str, value: f64, lo: Option<f64>, doc: &str) -> CmdResult<usize> {
    let attr = g.add_attr(c, float_attr(name, value, doc, lo, None))?;
    Ok(attr)
}

/// Adds a target: its matrix (and surface), its weight attr, and for point on poly its UV attrs.
fn add_target(g: &mut Graph, c: NodeId, kind: usize, target: NodeId, component: Option<&str>, weight: f64, driven: NodeId) -> CmdResult<i32> {
    let index = target_nodes(g, c).iter().map(|e| e.0).max().map(|m| m + 1).unwrap_or(0);
    let xf = crate::dag::transform_of(g, target);
    let inherits = g.node(xf).is_some_and(|n| !n.ty().is_a("transform") || n.data().bool(Transform::INHERITS_TRANSFORM));
    if matches!(kind, POINT | AIM | POLE) && inherits {
        g.connect(&plug(xf, Transform::PARENT_MATRIX), &PlugRef::element(c, T_PARENT as i32, index), false)?;
        g.connect(&plug(xf, Transform::TRANSLATE), &PlugRef::element(c, T_TRANSLATE as i32, index), false)?;
        g.connect(&plug(xf, Transform::ROTATE_PIVOT_TRANSLATE), &PlugRef::element(c, T_ROTATE_TRANSLATE as i32, index), false)?;
        g.connect(&plug(xf, Transform::OFFSET_PARENT_MATRIX), &PlugRef::element(c, T_OFFSET_PARENT as i32, index), false)?;
    } else if kind == SCALE && inherits {
        g.connect(&plug(xf, Transform::PARENT_MATRIX), &PlugRef::element(c, T_PARENT as i32, index), false)?;
        g.connect(&plug(xf, Transform::SCALE), &PlugRef::element(c, T_SCALE as i32, index), false)?;
    } else {
        g.connect(&plug(xf, Transform::WORLD_MATRIX), &PlugRef::element(c, T_MATRIX as i32, index), false)?;
    }
    // Geometry targets are read through their full matrix.
    if !matches!(kind, GEOMETRY | NORMAL | POINT_ON_POLY) {
        g.connect(&plug(xf, Transform::ROTATE_PIVOT), &PlugRef::element(c, T_PIVOT as i32, index), false)?;
    }
    if matches!(kind, GEOMETRY | NORMAL | POINT_ON_POLY) {
        let Some(shape) = crate::components::as_mesh(g, target) else { fail!("{}: {} has no geometry", CONSTRAINT_TYPES[kind], g.path(target)) };
        g.connect(&plug(shape, crate::mesh_nodes::MeshNode::OUT_MESH), &PlugRef::element(c, T_GEOMETRY as i32, index), false)?;
    }
    if kind == TANGENT {
        let Some(shape) = crate::curves::as_curve(g, target) else { fail!("tangentConstraint: {} is not a curve", g.path(target)) };
        g.connect(&plug(shape, crate::curves::CurveShape::WORLD_SPACE), &PlugRef::element(c, T_CURVE as i32, index), false)?;
    }
    let stem = alias_stem(g, xf);
    add_float(g, c, &format!("{stem}W{index}"), weight, Some(0.0), "This target's weight.")?;
    if kind == POINT_ON_POLY {
        let Some(shape) = crate::components::as_mesh(g, target) else { fail!("pointOnPolyConstraint: {} has no geometry", g.path(target)) };
        g.evaluate();
        let mesh = g.value(&plug(shape, crate::mesh_nodes::MeshNode::OUT_MESH)).ok().and_then(|v| v.as_mesh().cloned());
        let Some(mesh) = mesh else { fail!("pointOnPolyConstraint: {} has no geometry", g.path(target)) };
        let tw = world(g, xf);
        let dw = world(g, driven);
        let uv = uv_of_component(&mesh, component, tw.inverse().transform_point3(dw.w_axis.truncate()));
        add_float(g, c, &format!("{stem}U{index}"), uv.x, None, "This target's U.")?;
        add_float(g, c, &format!("{stem}V{index}"), uv.y, None, "This target's V.")?;
    }
    Ok(index)
}

/// Removes a target. A constraint left with none is deleted, the object holding its pose.
fn remove_target(g: &mut Graph, c: NodeId, index: i32) -> CmdResult<()> {
    if target_nodes(g, c).iter().all(|&(i, _)| i == index) {
        g.delete_node(c)?;
        return Ok(());
    }
    g.disconnect(&PlugRef::element(c, T_MATRIX as i32, index));
    for attr in [T_PARENT, T_TRANSLATE, T_ROTATE_TRANSLATE, T_OFFSET_PARENT, T_SCALE] {
        g.disconnect(&PlugRef::element(c, attr as i32, index));
    }
    g.disconnect(&PlugRef::element(c, T_GEOMETRY as i32, index));
    g.disconnect(&PlugRef::element(c, T_CURVE as i32, index));
    g.disconnect(&PlugRef::element(c, T_PIVOT as i32, index));
    for attr in [T_OFFSET_T, T_OFFSET_R] {
        let e = PlugRef::element(c, attr as i32, index);
        if g.node(c).unwrap().element(attr, index).is_some() {
            g.remove_element(&e)?;
        }
    }
    for letter in ['W', 'U', 'V'] {
        let ty = g.node(c).unwrap().type_arc().clone();
        if let Some(a) = suffix_attr(&ty, letter, index) {
            g.delete_attr(&plug(c, a))?;
        }
    }
    Ok(())
}

/// A constraint goes with the object it drives. A deleted target leaves the constraint.
pub(crate) fn before_delete(g: &mut Graph, doomed: &[NodeId]) {
    let cs: Vec<NodeId> = g.nodes().into_iter().filter(|&id| kind_of(g, id).is_some()).collect();
    for c in cs {
        if doomed.contains(&c) {
            continue;
        }
        if driven_of(g, c).is_some_and(|d| doomed.contains(&d)) {
            let _ = g.delete_node(c);
            continue;
        }
        for (index, t) in target_nodes(g, c) {
            if doomed.contains(&t) {
                if let Err(e) = remove_target(g, c, index) {
                    log::warn!("{e}");
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Offsets

fn world_rotation(g: &mut Graph, driven: NodeId) -> DMat3 {
    rot3(&world(g, driven))
}

/// -mo: the offset that keeps the object where it is. Parent constraints store it per target, the others on `offset`.
fn maintain_offset(g: &mut Graph, c: NodeId, kind: usize, driven: NodeId, only: Option<&[i32]>) -> CmdResult<()> {
    g.evaluate();
    let n = g.node(c).unwrap();
    // Offsets are in the driven object's rotate order, as the Eulers it is given.
    let order_now = RotateOrder::from_index(n.data().int(ROTATE_ORDER));
    let ty = n.type_arc().clone();
    let targets = target_nodes(g, c);
    let mut items = Vec::new();
    for &(i, t) in &targets {
        let xf = crate::dag::transform_of(g, t);
        let w = suffix_attr(&ty, 'W', i).map(|a| g.node(c).unwrap().data().float(a)).unwrap_or(1.0);
        let rp = g.node(xf).map_or(DVec3::ZERO, |n| n.data().vec3(Transform::ROTATE_PIVOT));
        items.push((i, at_pivot(world(g, xf), rp), w));
    }
    let total: f64 = items.iter().map(|x| x.2).sum::<f64>();
    let share = |w: f64| if total > 1e-12 { w / total } else { 1.0 / items.len().max(1) as f64 };
    let dp = g.parent(driven);
    let pm = if dp == NO_NODE { DMat4::IDENTITY } else { world(g, dp) };
    let pinv = pm.inverse();
    let pr = rot3(&pm);
    let (t_now, s_now) = (vec3_of(g, driven, Transform::TRANSLATE), vec3_of(g, driven, Transform::SCALE));
    let pivot = vec3_of(g, driven, Transform::ROTATE_PIVOT) + vec3_of(g, driven, Transform::ROTATE_PIVOT_TRANSLATE);
    let w_now = world_rotation(g, driven);
    let p_now = pm.transform_point3(t_now + pivot);
    match kind {
        PARENT => {
            for &(i, tm, _) in &items {
                if only.is_some_and(|o| !o.contains(&i)) {
                    continue;
                }
                let off_t = tm.inverse().transform_point3(p_now);
                let off_r = matrix_to_euler(&(rot3(&tm).transpose() * w_now), order_now);
                g.set_value(PlugRef::element(c, T_OFFSET_T as i32, i), PlugValue::Vec3(off_t))?;
                g.set_value(PlugRef::element(c, T_OFFSET_R as i32, i), PlugValue::Vec3(off_r))?;
            }
        }
        POINT => {
            let mut avg = DVec3::ZERO;
            for &(_, tm, w) in &items {
                avg += (pinv.transform_point3(tm.w_axis.truncate()) - pivot) * share(w);
            }
            set_vec3(g, c, OFFSET, t_now - avg)?;
        }
        ORIENT => {
            let qs: Vec<(DQuat, f64)> = items.iter().map(|&(_, tm, w)| (DQuat::from_mat3(&(pr.transpose() * rot3(&tm))), share(w))).collect();
            let avg = DMat3::from_quat(quat_avg(&qs, 1));
            set_vec3(g, c, OFFSET, matrix_to_euler(&(avg.transpose() * pr.transpose() * w_now), order_now))?;
        }
        SCALE => {
            let ps = scale_of(&pm).max(DVec3::splat(1e-12));
            let mut avg = DVec3::ZERO;
            for &(_, tm, w) in &items {
                avg += scale_of(&tm) / ps * share(w);
            }
            let inv = |a: f64, b: f64| if b.abs() > 1e-12 { a / b } else { 1.0 };
            set_vec3(g, c, OFFSET, DVec3::new(inv(s_now.x, avg.x), inv(s_now.y, avg.y), inv(s_now.z, avg.z)))?;
        }
        AIM => {
            let mut at = DVec3::ZERO;
            for &(_, tm, w) in &items {
                at += tm.w_axis.truncate() * share(w);
            }
            let d = g.node(c).unwrap().data();
            let frame = aim_rotation(d, at - p_now, p_now, &pm, d.vec3(REST_R));
            if let Some(frame) = frame {
                set_vec3(g, c, OFFSET, matrix_to_euler(&(frame.transpose() * pr.transpose() * w_now), order_now))?;
            }
        }
        POLE => {
            // In the space of the start joint's parent, as the constraint works it out.
            let joint = crate::ik::start_of(g, driven);
            let start = joint.map_or(DVec3::ZERO, |s| vec3_of(g, s, Transform::TRANSLATE));
            let up = joint.map_or(NO_NODE, |s| g.parent(s));
            let inv = if up == NO_NODE { DMat4::IDENTITY } else { world(g, up).inverse() };
            let mut avg = DVec3::ZERO;
            for &(_, tm, w) in &items {
                avg += (inv.transform_point3(tm.w_axis.truncate()) - start) * share(w);
            }
            let pv = vec3_of(g, driven, crate::ik::IkHandle::POLE_VECTOR);
            set_vec3(g, c, OFFSET, pv - avg)?;
        }
        _ => {}
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------------
// The commands

/// A -st/-sr/-sk value: axes to leave alone, from "x", "none", or a list of them.
fn axes_of(v: Option<&Value>) -> Vec<usize> {
    let mut out = Vec::new();
    let mut add = |s: &str| match s {
        "x" => out.push(0),
        "y" => out.push(1),
        "z" => out.push(2),
        _ => {}
    };
    match v {
        Some(Value::String(s)) => add(s),
        Some(Value::Array(a)) => a.iter().filter_map(Value::as_str).for_each(&mut add),
        _ => {}
    }
    out
}

/// Connects a constraint output, whole or the axes not skipped.
fn drive(g: &mut Graph, c: NodeId, out: usize, driven: NodeId, attr: usize, skip: &[usize]) -> CmdResult<()> {
    if skip.is_empty() {
        g.connect(&plug(c, out), &plug(driven, attr), true)?;
        return Ok(());
    }
    for k in 0..3 {
        if !skip.contains(&k) {
            g.connect(&PlugRef::component(c, out as i32, k as i32), &PlugRef::component(driven, attr as i32, k as i32), true)?;
        }
    }
    Ok(())
}

/// The settings -e and create share.
fn apply_settings(g: &mut Graph, c: NodeId, a: &Args) -> CmdResult<()> {
    for (flag, attr) in [("aimVector", AIM_VECTOR), ("upVector", UP_VECTOR), ("worldUpVector", WORLD_UP_VECTOR), ("offset", OFFSET)] {
        if let Some(v) = a.vec3(flag) {
            set_vec3(g, c, attr, DVec3::from_array(v))?;
        }
    }
    if a.has("worldUpType") {
        // By name, or by its number as the node's attribute counts them: scripts pass either.
        let given = a.get("worldUpType").cloned().unwrap_or(Value::Null);
        let name = given.as_str().map(str::to_string).unwrap_or_else(|| given.to_string());
        let by_number = given.as_i64().or_else(|| name.parse::<i64>().ok()).filter(|i| (0..WORLD_UP_NAMES.len() as i64).contains(i)).map(|i| i as usize);
        let Some(i) = by_number.or_else(|| WORLD_UP_NAMES.iter().position(|x| x.eq_ignore_ascii_case(&name))) else { fail!("worldUpType: {name} is not one of {}", WORLD_UP_NAMES.join(", ")) };
        g.set_value(plug(c, WORLD_UP_TYPE), PlugValue::Int(i as i64))?;
    }
    if a.has("worldUpObject") {
        let id = find(g, &a.string("worldUpObject", ""), "worldUpObject")?;
        g.connect(&plug(id, Transform::WORLD_MATRIX), &plug(c, WORLD_UP_MATRIX), true)?;
    }
    if a.has("interpType") {
        let v = a.get("interpType").unwrap();
        let names = ["noflip", "average", "shortest", "longest", "cache"];
        let i = match v.as_str() {
            Some(s) => names.iter().position(|n| n.eq_ignore_ascii_case(&s.replace(' ', ""))).map(|i| i as i64),
            None => v.as_i64(),
        };
        let Some(i) = i else { fail!("interpType: {v} is not an interpolation type") };
        g.set_value(plug(c, INTERP), PlugValue::Int(i))?;
    }
    Ok(())
}

/// A constraint of this kind that drives `node`.
fn constraint_on(g: &Graph, node: NodeId, kind: usize) -> Option<NodeId> {
    let connections = g.connections();
    let on_node: Vec<NodeId> = connections.iter().filter(|(d, s)| d.node == node && kind_of(g, s.node).is_some()).map(|(_, s)| s.node).collect();
    if let Some(&c) = on_node.iter().find(|&&c| kind_of(g, c) == Some(kind)) {
        return Some(c);
    }
    // One feeding the geometry constraint on the object.
    on_node.iter().filter_map(|&c| g.source(&plug(c, REST_T))).map(|s| s.node).find(|&c| kind_of(g, c) == Some(kind))
}

/// The constraint a query or edit means: named itself, or the kind on the object named.
fn resolve(g: &Graph, names: &[String], cmd: &str, kind: usize) -> CmdResult<(NodeId, Vec<NodeId>)> {
    let Some(last) = names.last() else { fail!("{cmd}: name a constraint or a constrained object") };
    let id = find(g, split_target(last).0, cmd)?;
    let c = if kind_of(g, id) == Some(kind) {
        id
    } else {
        match constraint_on(g, id, kind) {
            Some(c) => c,
            None => fail!("{cmd}: {last} has no {cmd}"),
        }
    };
    let mut targets = Vec::new();
    for n in &names[..names.len() - 1] {
        let t = find(g, split_target(n).0, cmd)?;
        targets.push(crate::dag::transform_of(g, t));
    }
    Ok((c, targets))
}

fn index_of_target(g: &Graph, c: NodeId, t: NodeId, cmd: &str) -> CmdResult<i32> {
    match target_nodes(g, c).into_iter().find(|&(_, x)| x == t) {
        Some((i, _)) => Ok(i),
        None => fail!("{cmd}: {} is not a target of {}", g.path(t), g.path(c)),
    }
}

fn query(g: &mut Graph, c: NodeId, targets: &[NodeId], a: &Args, cmd: &str) -> CmdResult {
    let flag = asked(a).into_iter().next().unwrap_or_else(|| "name".into());
    let ty = g.node(c).unwrap().type_arc().clone();
    let weight_of = |g: &Graph, i: i32| suffix_attr(&ty, 'W', i).map(|at| g.node(c).unwrap().data().float(at)).unwrap_or(0.0);
    let all = target_nodes(g, c);
    let d = g.node(c).unwrap().data();
    let vec = |v: DVec3| json!([v.x, v.y, v.z]);
    Ok(match flag.as_str() {
        "name" => json!(g.path(c)),
        "targetList" => json!(all.iter().map(|&(_, t)| g.path(t)).collect::<Vec<_>>()),
        "weightAliasList" => json!(all.iter().filter_map(|&(i, _)| suffix_attr(&ty, 'W', i).map(|at| ty.attrs[at].name.clone())).collect::<Vec<_>>()),
        "weight" => {
            if let [one] = targets {
                json!(weight_of(g, index_of_target(g, c, *one, cmd)?))
            } else {
                json!(all.iter().map(|&(i, _)| weight_of(g, i)).collect::<Vec<_>>())
            }
        }
        "offset" => vec(d.vec3(OFFSET)),
        "aimVector" => vec(d.vec3(AIM_VECTOR)),
        "upVector" => vec(d.vec3(UP_VECTOR)),
        "worldUpVector" => vec(d.vec3(WORLD_UP_VECTOR)),
        "worldUpType" => json!(WORLD_UP_NAMES[d.int(WORLD_UP_TYPE).clamp(0, 4) as usize]),
        "worldUpObject" => json!(g.source(&plug(c, WORLD_UP_MATRIX)).map(|s| g.path(s.node))),
        "interpType" => json!(d.int(INTERP)),
        other => fail!("{cmd}: can't query -{other}"),
    })
}

fn edit(g: &mut Graph, c: NodeId, targets: &[NodeId], a: &Args, cmd: &str, kind: usize, driven: NodeId) -> CmdResult {
    if a.bool("remove") {
        if targets.is_empty() {
            fail!("{cmd}: name the targets to remove");
        }
        let mut indices = Vec::new();
        for &t in targets {
            indices.push(index_of_target(g, c, t, cmd)?);
        }
        // All of them: the object holds the pose it has now, not the one the last target alone would give.
        if indices.len() == target_nodes(g, c).len() {
            g.delete_node(c)?;
            return Ok(Value::Null);
        }
        for i in indices {
            remove_target(g, c, i)?;
        }
        return Ok(Value::Null);
    }
    apply_settings(g, c, a)?;
    if let Some(w) = a.get("weight").and_then(Value::as_f64) {
        let chosen: Vec<i32> = if targets.is_empty() {
            target_nodes(g, c).iter().map(|x| x.0).collect()
        } else {
            let mut v = Vec::new();
            for &t in targets {
                v.push(index_of_target(g, c, t, cmd)?);
            }
            v
        };
        let ty = g.node(c).unwrap().type_arc().clone();
        for i in chosen {
            if let Some(attr) = suffix_attr(&ty, 'W', i) {
                g.set_value(plug(c, attr), PlugValue::Float(w))?;
            }
        }
    }
    if a.bool("maintainOffset") {
        maintain_offset(g, c, kind, driven, None)?;
    }
    Ok(Value::Null)
}

fn constrain(s: &mut Session, a: &Args, cmd: &str, kind: usize) -> CmdResult {
    let names = objects(s, a);
    let g = &mut s.graph;
    if a.bool("query") {
        let (c, targets) = resolve(g, &names, cmd, kind)?;
        return query(g, c, &targets, a, cmd);
    }
    if a.bool("edit") || a.bool("remove") {
        let (c, targets) = resolve(g, &names, cmd, kind)?;
        let driven = driven_of(g, c).unwrap_or(NO_NODE);
        return edit(g, c, &targets, a, cmd, kind, driven);
    }
    if names.len() < 2 {
        fail!("{cmd}: name or select the targets, then the object to drive");
    }
    let driven = find(g, names.last().unwrap(), cmd)?;
    if !g.node(driven).is_some_and(|n| n.ty().is_a("transform")) {
        fail!("{cmd}: {} is not a transform", names.last().unwrap());
    }
    if kind == POLE && !g.node(driven).is_some_and(|n| n.ty().name == "ikHandle") {
        fail!("{cmd}: {} is not an IK handle", names.last().unwrap());
    }
    let mut targets = Vec::new();
    for n in &names[..names.len() - 1] {
        let (node, component) = split_target(n);
        targets.push((find(g, node, cmd)?, component));
    }
    let skip_t = [axes_of(a.get("skipTranslate")), if kind == POINT || kind == GEOMETRY { axes_of(a.get("skip")) } else { Vec::new() }].concat();
    let skip_r = [axes_of(a.get("skipRotate")), if matches!(kind, ORIENT | AIM | NORMAL | TANGENT) { axes_of(a.get("skip")) } else { Vec::new() }].concat();
    let skip_s = [axes_of(a.get("skipScale")), if kind == SCALE { axes_of(a.get("skip")) } else { Vec::new() }].concat();
    let weight = a.float("weight", 1.0);
    g.evaluate();
    g.open_chunk(cmd);
    let r = (|| -> CmdResult<NodeId> {
        let existing = constraint_on(g, driven, kind);
        let is_new = existing.is_none();
        let c = match existing {
            Some(c) => c,
            None => {
                let t = types::find(cmd).unwrap();
                let driven_name = g.node(driven).map(|n| n.name().replace([':', '|'], "_")).unwrap_or_default();
                let c = g.create_node(&t, &a.string("name", &format!("{driven_name}_{cmd}1")), NO_NODE, true)?;
                // The rest position is where the object is now.
                let n = g.node(driven).unwrap().data();
                let (rt, rr, rs) = (n.vec3(Transform::TRANSLATE), n.vec3(Transform::ROTATE), n.vec3(Transform::SCALE));
                let (rt, rr, rs) = if kind == POLE { (vec3_of(g, driven, crate::ik::IkHandle::POLE_VECTOR), rr, rs) } else { (rt, rr, rs) };
                set_vec3(g, c, REST_T, rt)?;
                set_vec3(g, c, REST_R, rr)?;
                set_vec3(g, c, REST_S, rs)?;
                let parent_of = if kind == POLE { crate::ik::start_of(g, driven).unwrap_or(driven) } else { driven };
                let p = g.parent(parent_of);
                if p != NO_NODE {
                    g.connect(&plug(p, Transform::WORLD_MATRIX), &plug(c, PARENT_MATRIX), false)?;
                }
                c
            }
        };
        apply_settings(g, c, a)?;
        let mut added = Vec::new();
        for &(t, component) in &targets {
            let xf = crate::dag::transform_of(g, t);
            if target_nodes(g, c).iter().any(|&(_, x)| x == xf) {
                continue;
            }
            added.push(add_target(g, c, kind, t, component, weight, driven)?);
        }
        if is_new {
            sync_from(g, c, kind, driven);
        }
        // Worked out while the object is still where it was.
        if a.bool("maintainOffset") {
            maintain_offset(g, c, kind, driven, if is_new { None } else { Some(&added) })?;
        }
        if is_new {
            // A geometry constraint takes what drove the translate as where the object wants to be.
            if kind == GEOMETRY {
                if let Some(src) = g.source(&plug(driven, Transform::TRANSLATE)) {
                    g.connect(&src, &plug(c, REST_T), true)?;
                }
            }
            match kind {
                PARENT | POINT_ON_POLY => {
                    if skip_t.len() >= 3 && skip_r.len() >= 3 {
                        fail!("{cmd}: every channel is skipped");
                    }
                    if skip_t.len() < 3 {
                        drive(g, c, OUT_T, driven, Transform::TRANSLATE, &skip_t)?;
                    }
                    if skip_r.len() < 3 {
                        drive(g, c, OUT_R, driven, Transform::ROTATE, &skip_r)?;
                    }
                }
                POINT | GEOMETRY => {
                    if skip_t.len() >= 3 {
                        fail!("{cmd}: every channel is skipped");
                    }
                    drive(g, c, OUT_T, driven, Transform::TRANSLATE, &skip_t)?;
                }
                ORIENT | AIM | NORMAL | TANGENT => {
                    if skip_r.len() >= 3 {
                        fail!("{cmd}: every channel is skipped");
                    }
                    drive(g, c, OUT_R, driven, Transform::ROTATE, &skip_r)?;
                }
                SCALE => {
                    if skip_s.len() >= 3 {
                        fail!("{cmd}: every channel is skipped");
                    }
                    drive(g, c, OUT_S, driven, Transform::SCALE, &skip_s)?;
                }
                _ => drive(g, c, OUT_T, driven, crate::ik::IkHandle::POLE_VECTOR, &[])?,
            }
        }
        Ok(c)
    })();
    g.close_chunk();
    let c = r?;
    Ok(json!([g.path(c)]))
}

pub fn register(c: &mut Commands<Session>) {
    for (k, name) in CONSTRAINT_TYPES.iter().enumerate() {
        let name = name.to_string();
        let n2 = name.clone();
        c.register(
            Cmd::new(
                &name,
                "Constrain the object named (or selected) last to the targets before it; a second call adds targets. -mo keeps the \
                 current offset; -w sets the targets' weight; -e and -q work on the constraint or the object it drives. Returns \
                 [constraint].",
                move |s, a| constrain(s, a, &n2, k),
            )
            .positional("targets, then the object to drive or the constraint")
            .flag(f("maintainOffset", "mo", ArgType::Bool, "Keep the object where it is now relative to the targets."))
            .flag(f("weight", "w", ArgType::Float, "Weight of the targets named (1)."))
            .flag(f("name", "n", ArgType::String, "Name of the constraint."))
            .flag(f("offset", "o", ArgType::Vec3, "Added to the result."))
            .flag(f("remove", "rm", ArgType::Bool, "Remove the targets named. Removing the last deletes the constraint."))
            .flag(f("targetList", "tl", ArgType::Bool, "Query: the targets."))
            .flag(f("weightAliasList", "wal", ArgType::Bool, "Query: the targets' weight attributes."))
            .flag(f("aimVector", "aim", ArgType::Vec3, "The local axis that points at the target."))
            .flag(f("upVector", "u", ArgType::Vec3, "The local axis that points up."))
            .flag(f("worldUpVector", "wu", ArgType::Vec3, "The world direction up means."))
            .flag(f("worldUpType", "wut", ArgType::Any, "scene, object, objectrotation, vector or none."))
            .flag(f("worldUpObject", "wuo", ArgType::String, "The object for worldUpType object and objectrotation."))
            .flag(f("interpType", "int", ArgType::Any, "How rotations blend: noflip, average, shortest, longest, cache, or 0 to 4."))
            .flag(f("skipTranslate", "st", ArgType::Any, "Translate axes to leave alone: x, y, z or none. May be used more than once.").multi(1))
            .flag(f("skipRotate", "sr", ArgType::Any, "Rotate axes to leave alone: x, y, z or none. May be used more than once.").multi(1))
            .flag(f("skip", "sk", ArgType::Any, "Axes of this constraint's channel to leave alone: x, y, z or none. May be used more than once.").multi(1))
            .flag(f("skipScale", "", ArgType::Any, "Scale axes to leave alone: x, y, z or none.").multi(1))
            .flag(f("edit", "e", ArgType::Bool, "Change the constraint."))
            .flag(f("query", "q", ArgType::Bool, "Ask about the constraint."))
            .flag(f("queried", "", ArgType::List, "Flags asked about in query mode.")),
        );
    }

    c.register(
        Cmd::new(
            "RemoveConstraintTarget",
            "Remove the selected targets from the constraints on the object selected last.",
            |s, a| {
                let names = objects(s, a);
                let g = &mut s.graph;
                if names.len() < 2 {
                    fail!("RemoveConstraintTarget: select the targets, then the constrained object");
                }
                let driven = find(g, names.last().unwrap(), "RemoveConstraintTarget")?;
                let mut wanted = Vec::new();
                for n in &names[..names.len() - 1] {
                    wanted.push(find(g, n, "RemoveConstraintTarget")?);
                }
                let cs: Vec<NodeId> = g.nodes().into_iter().filter(|&c| kind_of(g, c).is_some() && driven_of(g, c) == Some(driven)).collect();
                for c in cs {
                    let targets = target_nodes(g, c);
                    if targets.iter().all(|(_, t)| wanted.contains(t)) {
                        g.delete_node(c)?;
                        continue;
                    }
                    for (i, t) in targets {
                        if wanted.contains(&t) {
                            remove_target(g, c, i)?;
                        }
                    }
                }
                Ok(Value::Null)
            },
        )
        .positional("targets, then the constrained object"),
    );
}
