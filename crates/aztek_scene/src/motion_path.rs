//! Motion paths: the `motionPath` node and `pathAnimation`, which drive an object along a curve; `snapshot`, which leaves
//! copies along the animation; `motionTrail`, which keeps the positions for drawing; and `flow`.
//!
//! The reference feeds a motion path through addDoubleLinear nodes; here allCoordinates connects to translate directly.

use std::any::Any;
use std::sync::Arc;

use aztek_geo::nurbs::NurbsCurve;
use aztek_ops::{fail, objects, ArgType, Args, CmdResult, Commands};
use glam::{DMat3, DMat4, DVec3};
use serde_json::{json, Value};

use crate::anim::{self, f, query_flags, queried, Cmd};
use crate::curves::{self, CurveData, CurveShape};
use crate::graph::Graph;
use crate::node::{types, vec3_attr, EvalContext, NodeBehavior, NodeData, NodeType};
use crate::plug::{flags, AttrDef, NodeId, PlugRef, PlugType, PlugValue, NO_NODE};
use crate::rig::{data_attr, json_of, set_json};
use crate::session::Session;
use crate::transform::{matrix_to_euler, DagNode, RotateOrder, Shape, Transform};

const U_VALUE: usize = 0;
const FRONT_TWIST: usize = 1;
const UP_TWIST: usize = 2;
const SIDE_TWIST: usize = 3;
const FRACTION_MODE: usize = 4;
const FOLLOW: usize = 5;
const WORLD_UP_TYPE: usize = 6;
const WORLD_UP_VECTOR: usize = 7;
const WORLD_UP_MATRIX: usize = 8;
const FRONT_AXIS: usize = 9;
const UP_AXIS: usize = 10;
const INVERSE_FRONT: usize = 11;
const INVERSE_UP: usize = 12;
const BANK: usize = 13;
const BANK_SCALE: usize = 14;
const BANK_LIMIT: usize = 15;
const GEOMETRY_PATH: usize = 16;
const PARENT_SPACE: usize = 17;
const ROTATE_ORDER: usize = 18;
const POSITION_MARKER_TIME: usize = 19;
const ORIENTATION_MARKER_TIME: usize = 20;
const ALL_COORDINATES: usize = 21;
const ROTATE: usize = 22;
const ORIENT_MATRIX: usize = 23;

pub const WORLD_UP_TYPES: &[&str] = &["scene", "object", "objectrotation", "vector", "normal"];

fn axis_vec(i: usize) -> DVec3 {
    [DVec3::X, DVec3::Y, DVec3::Z][i.min(2)]
}

/// The frame on a curve at u, as motionPath computes it: columns are the object's axes in world space.
struct PathFrame {
    position: DVec3,
    rotation: DMat3,
}

/// The curve's unit tangent, and the rate it turns per unit of length: direction towards the centre of the turn, length
/// the curvature.
fn tangent_and_curvature(c: &NurbsCurve, u: f64) -> (DVec3, DVec3) {
    let (lo, hi) = c.domain();
    let t = c.tangent(u);
    let unit = t.normalize_or_zero();
    let e = (hi - lo).max(1e-6) * 1e-4;
    let (a, b) = ((u - e).max(lo), (u + e).min(hi));
    if b - a < 1e-12 || t.length() < 1e-12 {
        return (unit, DVec3::ZERO);
    }
    let dt = (c.tangent(b).normalize_or_zero() - c.tangent(a).normalize_or_zero()) / (b - a);
    (unit, (dt - unit * dt.dot(unit)) / t.length())
}

fn path_frame(c: &NurbsCurve, d: &NodeData) -> PathFrame {
    let (lo, hi) = c.domain();
    let raw = d.float(U_VALUE);
    let u = if d.bool(FRACTION_MODE) { c.param_at_fraction(raw.clamp(0.0, 1.0)) } else { raw.clamp(lo, hi) };
    let position = c.eval(u);
    let (front, turn) = tangent_and_curvature(c, u);
    let wum = d.matrix(WORLD_UP_MATRIX);
    let vector = d.vec3(WORLD_UP_VECTOR);
    let up_wanted = match d.int(WORLD_UP_TYPE) {
        0 => DVec3::Y,
        1 => wum.w_axis.truncate() - position,
        2 => wum.transform_vector3(vector),
        4 => turn.normalize_or_zero(),
        _ => vector,
    };
    // The up axis is as near the wanted direction as it can be and still square to the path.
    let mut up = up_wanted - front * up_wanted.dot(front);
    if up.length() < 1e-9 {
        let other = if front.x.abs() < 0.9 { DVec3::X } else { DVec3::Z };
        up = other - front * other.dot(front);
    }
    let mut up = up.normalize_or_zero();
    if d.bool(BANK) {
        // Leans into the turn in proportion to how tight it is.
        let limit = d.float(BANK_LIMIT).abs();
        let side = front.cross(up);
        let lean = turn.length() * d.float(BANK_SCALE);
        let angle = lean.clamp(-limit, limit) * if side.dot(turn) >= 0.0 { 1.0 } else { -1.0 };
        up = up * angle.cos() + side * angle.sin();
    }
    let (fa, ua) = (d.int(FRONT_AXIS).clamp(0, 2) as usize, d.int(UP_AXIS).clamp(0, 2) as usize);
    let f_col = if d.bool(INVERSE_FRONT) { -front } else { front };
    let u_col = if d.bool(INVERSE_UP) { -up } else { up };
    let mut cols = [DVec3::ZERO; 3];
    cols[fa] = f_col;
    cols[ua] = u_col;
    let mut third = 0;
    if fa != ua {
        third = 3 - fa - ua;
        // Right-handed whichever axes are named.
        let sign = if (ua + 3 - fa) % 3 == 1 { 1.0 } else { -1.0 };
        cols[third] = f_col.cross(u_col) * sign;
    } else {
        let (x, y) = f_col.any_orthonormal_pair();
        cols[(fa + 1) % 3] = x;
        cols[(fa + 2) % 3] = y;
    }
    let mut rotation = DMat3::from_cols(cols[0], cols[1], cols[2]);
    for (attr, axis) in [(FRONT_TWIST, fa), (UP_TWIST, ua), (SIDE_TWIST, third)] {
        let angle = d.float(attr);
        if angle != 0.0 {
            rotation = rotation * DMat3::from_axis_angle(axis_vec(axis), angle);
        }
    }
    PathFrame { position, rotation }
}

struct MotionPath;

impl NodeBehavior for MotionPath {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        let curve = match d.value(GEOMETRY_PATH) {
            PlugValue::Data(Some(x)) => x.as_any().downcast_ref::<CurveData>().map(|c| c.0.clone()),
            _ => None,
        };
        let Some(curve) = curve else { return };
        if !curve.is_valid() {
            return;
        }
        let frame = path_frame(&curve, d);
        let pinv = d.matrix(PARENT_SPACE).inverse();
        let local = pinv.transform_point3(frame.position);
        let parent_rot = {
            let m = DMat3::from_mat4(pinv);
            DMat3::from_cols(m.x_axis.normalize_or_zero(), m.y_axis.normalize_or_zero(), m.z_axis.normalize_or_zero())
        };
        let r = parent_rot * frame.rotation;
        d.set_output(ALL_COORDINATES, PlugValue::Vec3(local));
        let euler = if d.bool(FOLLOW) { matrix_to_euler(&r, RotateOrder::from_index(d.int(ROTATE_ORDER))) } else { DVec3::ZERO };
        d.set_output(ROTATE, PlugValue::Vec3(euler));
        let mut m = DMat4::from_mat3(r);
        m.w_axis = local.extend(1.0);
        d.set_output(ORIENT_MATRIX, PlugValue::Matrix(m));
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub fn is_motion_path(g: &Graph, id: NodeId) -> bool {
    g.node(id).is_some_and(|n| n.ty().name == "motionPath")
}

// ---------------------------------------------------------------------------------------------------------------------
// Trails

const TRAIL_START: usize = 0;
const TRAIL_END: usize = 1;
const TRAIL_INCREMENT: usize = 2;
const TRAIL_TARGET: usize = 3;
const TRAIL_POINTS: usize = 4;
const TRAIL_KEYS: usize = 5;
/// The anchor transform's world matrix (snapshot -anchorTransform): the trail's points are in its space.
const TRAIL_ANCHOR: usize = 6;

struct PlainNode;

impl NodeBehavior for PlainNode {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

struct TrailShape;

impl TrailShape {
    pub const POINTS: usize = Shape::COUNT;
    pub const KEYS: usize = Shape::COUNT + 1;
}

impl NodeBehavior for TrailShape {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        Shape::compute_matrices(d)
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn points_of(v: &Value) -> Vec<DVec3> {
    v.as_array()
        .map(|a| a.iter().filter_map(|p| Some(DVec3::new(p.get(0)?.as_f64()?, p.get(1)?.as_f64()?, p.get(2)?.as_f64()?))).collect())
        .unwrap_or_default()
}

/// What the viewport draws for a motionTrailShape, world space: the path, and a cross at each key.
pub fn trail_lines(n: &crate::node::Node) -> Vec<(DVec3, DVec3)> {
    let d = n.data();
    // The points are in the handle's space: the world, or the anchor's under it.
    let m = d.matrix(DagNode::WORLD_MATRIX);
    let pts: Vec<DVec3> = points_of(&json_of(d, TrailShape::POINTS)).into_iter().map(|p| m.transform_point3(p)).collect();
    let mut out: Vec<(DVec3, DVec3)> = pts.windows(2).map(|w| (w[0], w[1])).collect();
    for k in points_of(&json_of(d, TrailShape::KEYS)).into_iter().map(|p| m.transform_point3(p)) {
        let r = 0.1;
        for ax in [DVec3::X, DVec3::Y, DVec3::Z] {
            out.push((k - ax * r, k + ax * r));
        }
    }
    out
}

fn range_of(a: &Args) -> (f64, f64, f64) {
    let r = anim::range();
    (a.get("startTime").and_then(Value::as_f64).unwrap_or(r.min), a.get("endTime").and_then(Value::as_f64).unwrap_or(r.max), a.get("increment").and_then(Value::as_f64).unwrap_or(1.0).abs().max(1e-3))
}

fn frames(start: f64, end: f64, by: f64) -> Vec<f64> {
    let n = ((end - start) / by).floor().max(0.0) as usize;
    (0..=n).map(|i| start + i as f64 * by).collect()
}

fn world_at(g: &mut Graph, id: NodeId, t: f64) -> DMat4 {
    anim::set_frame(g, t);
    g.evaluate();
    g.node(id).map(|n| n.data().matrix(DagNode::WORLD_MATRIX)).unwrap_or(DMat4::IDENTITY)
}

/// Samples a trail again: its object's position at each of its frames, and at each key. Moves the time and comes back,
/// so values held by the current frame go, as with any time change.
pub fn refresh_trail(g: &mut Graph, trail: NodeId) -> CmdResult<()> {
    let Some(n) = g.node(trail) else { return Ok(()) };
    let (start, end, by) = (n.data().float(TRAIL_START), n.data().float(TRAIL_END), n.data().float(TRAIL_INCREMENT).abs().max(1e-3));
    let Some(target) = g.source(&PlugRef::new(trail, TRAIL_TARGET as i32)).map(|s| s.node) else { return Ok(()) };
    let anchor = g.source(&PlugRef::new(trail, TRAIL_ANCHOR as i32)).map(|s| s.node);
    let now = anim::frame(g);
    let at = |g: &mut Graph, t: f64| {
        let p = world_at(g, target, t).w_axis.truncate();
        // Anchored: where the object is seen from the anchor at that time.
        let p = match anchor {
            Some(an) => g.node(an).map(|n| n.data().matrix(DagNode::WORLD_MATRIX)).unwrap_or(DMat4::IDENTITY).inverse().transform_point3(p),
            None => p,
        };
        json!([p.x, p.y, p.z])
    };
    let points: Vec<Value> = frames(start, end, by).into_iter().map(|t| at(g, t)).collect();
    let key_times: Vec<f64> = anim::key_times(g, &[target]).into_iter().filter(|t| *t >= start - 1e-6 && *t <= end + 1e-6).collect();
    let keys: Vec<Value> = key_times.iter().map(|&t| at(g, t)).collect();
    anim::set_frame(g, now);
    g.evaluate();
    set_json(g, trail, TRAIL_POINTS, json!(points))?;
    set_json(g, trail, TRAIL_KEYS, json!(key_times))?;
    // The shape holds the same points for the viewport.
    let shape = g.nodes().into_iter().find(|&s| g.node(s).is_some_and(|x| x.ty().name == "motionTrailShape") && g.source(&PlugRef::new(s, TrailShape::POINTS as i32)).is_some_and(|src| src.node == trail));
    if let Some(s) = shape {
        set_json(g, s, TrailShape::KEYS, json!(keys))?;
    }
    g.evaluate();
    Ok(())
}

/// Whether a command changes what trails show. Everything else leaves them alone, as sampling moves the time.
fn changes_animation(name: &str) -> bool {
    matches!(name, "setKeyframe" | "keyframe" | "cutKey" | "keyTangent" | "editKeys" | "pasteKey" | "scaleKey" | "snapKey" | "pathAnimation" | "filterCurve" | "simplify")
}

/// Keeps trails up to date after commands that change the animation.
pub fn after_command(s: &mut Session, name: &str, ok: bool) {
    if !ok || !changes_animation(name) {
        return;
    }
    let g = &mut s.graph;
    let trails: Vec<NodeId> = g.nodes().into_iter().filter(|&n| g.node(n).is_some_and(|x| x.ty().name == "motionTrail")).collect();
    for t in trails {
        let _ = refresh_trail(g, t);
    }
}

fn create_trail(g: &mut Graph, object: NodeId, name: Option<&str>, start: f64, end: f64, by: f64) -> CmdResult<Value> {
    create_trail_in(g, object, name, start, end, by, None, false)
}

/// The trail's handle, from its motionTrail node.
fn trail_handle(g: &Graph, trail: NodeId) -> Option<NodeId> {
    let shape = g.destinations(&PlugRef::new(trail, TRAIL_POINTS as i32)).into_iter().map(|d| d.node).find(|&s| g.node(s).is_some_and(|x| x.ty().name == "motionTrailShape"))?;
    Some(g.parent(shape)).filter(|&h| h != NO_NODE)
}

/// Puts a trail's handle in its anchor's space: under it, or with -opm through its offsetParentMatrix.
fn place_trail_handle(g: &mut Graph, trail: NodeId, opm: bool) -> CmdResult<()> {
    let (Some(handle), Some(anchor)) = (trail_handle(g, trail), g.source(&PlugRef::new(trail, TRAIL_ANCHOR as i32)).map(|s| s.node)) else { return Ok(()) };
    let opm_plug = PlugRef::new(handle, Transform::OFFSET_PARENT_MATRIX as i32);
    if opm {
        if g.parent(handle) != NO_NODE {
            g.set_parent(handle, NO_NODE, false, usize::MAX)?;
        }
        g.connect(&PlugRef::new(anchor, DagNode::WORLD_MATRIX as i32), &opm_plug, true)?;
    } else {
        if g.source(&opm_plug).is_some() {
            g.disconnect(&opm_plug);
            g.set_value(opm_plug, PlugValue::Matrix(DMat4::IDENTITY))?;
        }
        if g.parent(handle) != anchor {
            g.set_parent(handle, anchor, false, usize::MAX)?;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn create_trail_in(g: &mut Graph, object: NodeId, name: Option<&str>, start: f64, end: f64, by: f64, anchor: Option<NodeId>, opm: bool) -> CmdResult<Value> {
    let node_ty = types::find("motionTrail").unwrap();
    let node = g.create_node(&node_ty, name.unwrap_or("motionTrail1"), NO_NODE, true)?;
    let base = g.node(node).map(|n| n.name.clone()).unwrap_or_default();
    g.set_value(PlugRef::new(node, TRAIL_START as i32), PlugValue::Float(start))?;
    g.set_value(PlugRef::new(node, TRAIL_END as i32), PlugValue::Float(end))?;
    g.set_value(PlugRef::new(node, TRAIL_INCREMENT as i32), PlugValue::Float(by))?;
    g.connect(&PlugRef::new(object, DagNode::WORLD_MATRIX as i32), &PlugRef::new(node, TRAIL_TARGET as i32), false)?;
    let tt = types::find("transform").unwrap();
    let handle = g.create_node(&tt, &format!("{base}Handle"), NO_NODE, true)?;
    let hname = g.node(handle).map(|n| n.name.clone()).unwrap_or_default();
    let st = types::find("motionTrailShape").unwrap();
    let shape = g.create_node(&st, &format!("{hname}Shape"), handle, true)?;
    g.connect(&PlugRef::new(node, TRAIL_POINTS as i32), &PlugRef::new(shape, TrailShape::POINTS as i32), false)?;
    if let Some(an) = anchor {
        g.connect(&PlugRef::new(an, DagNode::WORLD_MATRIX as i32), &PlugRef::new(node, TRAIL_ANCHOR as i32), false)?;
        place_trail_handle(g, node, opm)?;
    }
    refresh_trail(g, node)?;
    Ok(json!([g.path(handle), g.path(node)]))
}

fn object_id(g: &Graph, name: &str, cmd: &str) -> CmdResult<NodeId> {
    match g.find(name.split('.').next().unwrap_or(name))? {
        Some(id) => Ok(crate::dag::transform_of(g, id)),
        None => fail!("{cmd}: no object named {name}"),
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// pathAnimation

fn axis_index(s: &str) -> Option<i64> {
    match s {
        "x" | "X" => Some(0),
        "y" | "Y" => Some(1),
        "z" | "Z" => Some(2),
        _ => None,
    }
}

pub(crate) fn u_value_plug(id: NodeId) -> PlugRef {
    u_plug(id)
}

fn u_plug(id: NodeId) -> PlugRef {
    PlugRef::new(id, U_VALUE as i32)
}

/// The path's curve transform, and the motion path driving an object.
fn path_of(g: &Graph, name: &str) -> CmdResult<NodeId> {
    let Some(id) = g.find(name)? else { fail!("pathAnimation: no object named {name}") };
    if is_motion_path(g, id) {
        return Ok(id);
    }
    let t = crate::dag::transform_of(g, id);
    for attr in [Transform::TRANSLATE, Transform::ROTATE] {
        if let Some(s) = g.source(&PlugRef::new(t, attr as i32)) {
            if is_motion_path(g, s.node) {
                return Ok(s.node);
            }
        }
    }
    fail!("pathAnimation: {name} is not on a motion path")
}

/// Sets the flags given on a motion path, in create or edit mode.
fn apply_path_flags(g: &mut Graph, mp: NodeId, a: &Args) -> CmdResult<()> {
    let set_b = |g: &mut Graph, flag: &str, at: usize| -> CmdResult<()> {
        if let Some(v) = a.get(flag).and_then(Value::as_bool) {
            g.set_value(PlugRef::new(mp, at as i32), PlugValue::Bool(v))?;
        }
        Ok(())
    };
    set_b(g, "fractionMode", FRACTION_MODE)?;
    set_b(g, "inverseFront", INVERSE_FRONT)?;
    set_b(g, "inverseUp", INVERSE_UP)?;
    set_b(g, "bank", BANK)?;
    for (flag, at) in [("bankScale", BANK_SCALE), ("bankThreshold", BANK_LIMIT), ("bankLimit", BANK_LIMIT)] {
        if let Some(v) = a.get(flag).and_then(Value::as_f64) {
            g.set_value(PlugRef::new(mp, at as i32), PlugValue::Float(v))?;
        }
    }
    for (flag, at) in [("followAxis", FRONT_AXIS), ("upAxis", UP_AXIS)] {
        if let Some(v) = a.get(flag).and_then(Value::as_str) {
            let Some(i) = axis_index(v) else { fail!("pathAnimation: axis must be x, y or z, not {v}") };
            g.set_value(PlugRef::new(mp, at as i32), PlugValue::Int(i))?;
        }
    }
    let (fa, ua) = {
        let n = g.node(mp).unwrap();
        (n.data().int(FRONT_AXIS), n.data().int(UP_AXIS))
    };
    if fa == ua {
        fail!("pathAnimation: the front and up axes must differ");
    }
    if let Some(v) = a.vec3("worldUpVector") {
        g.set_value(PlugRef::new(mp, WORLD_UP_VECTOR as i32), PlugValue::Vec3(DVec3::from_array(v)))?;
    }
    if let Some(n) = a.get("worldUpObject").and_then(Value::as_str) {
        let id = object_id(g, n, "pathAnimation")?;
        g.connect(&PlugRef::new(id, DagNode::WORLD_MATRIX as i32), &PlugRef::new(mp, WORLD_UP_MATRIX as i32), true)?;
        if !a.has("worldUpType") {
            g.set_value(PlugRef::new(mp, WORLD_UP_TYPE as i32), PlugValue::Int(1))?;
        }
    }
    if let Some(v) = a.get("worldUpType").and_then(Value::as_str) {
        let Some(i) = WORLD_UP_TYPES.iter().position(|x| *x == v) else { fail!("pathAnimation: unknown world up type {v}: one of {}", WORLD_UP_TYPES.join(", ")) };
        g.set_value(PlugRef::new(mp, WORLD_UP_TYPE as i32), PlugValue::Int(i as i64))?;
    }
    if a.bool("useNormal") {
        g.set_value(PlugRef::new(mp, WORLD_UP_TYPE as i32), PlugValue::Int(4))?;
    }
    if let Some(follow) = a.get("follow").and_then(Value::as_bool) {
        g.set_value(PlugRef::new(mp, FOLLOW as i32), PlugValue::Bool(follow))?;
        // Follow is what connects rotation: without it the object keeps turning freely.
        let rotate_out = PlugRef::new(mp, ROTATE as i32);
        let driven = g.destinations(&rotate_out);
        if follow && driven.is_empty() {
            let t = g.destinations(&PlugRef::new(mp, ALL_COORDINATES as i32)).first().map(|d| d.node);
            if let Some(t) = t {
                g.connect(&rotate_out, &PlugRef::new(t, Transform::ROTATE as i32), true)?;
            }
        } else if !follow {
            for d in driven {
                g.disconnect(&d);
            }
        }
    }
    let curve = g.source(&PlugRef::new(mp, GEOMETRY_PATH as i32)).map(|s| s.node);
    let domain = curve.and_then(|c| curves::world_curve(g, c)).map(|c| c.domain()).unwrap_or((0.0, 1.0));
    let fraction = g.node(mp).is_some_and(|n| n.data().bool(FRACTION_MODE));
    let (lo, hi) = if fraction { (0.0, 1.0) } else { domain };
    let (su, eu) = (a.get("startU").and_then(Value::as_f64).unwrap_or(lo), a.get("endU").and_then(Value::as_f64).unwrap_or(hi));
    let p = u_plug(mp);
    if let Some(t) = a.get("startTimeU").and_then(Value::as_f64) {
        anim::key_plug(g, &p, t, Some(su), None, None)?;
    }
    if let Some(t) = a.get("endTimeU").and_then(Value::as_f64) {
        anim::key_plug(g, &p, t, Some(eu), None, None)?;
    }
    Ok(())
}

fn path_query(g: &mut Graph, mp: NodeId, a: &Args) -> CmdResult {
    let n = g.node(mp).unwrap().data().clone();
    let keys = anim::curve_of(g, &u_plug(mp)).map(|c| anim::keys_of(g, c)).unwrap_or_default();
    let (first, last) = (keys.keys.first().copied(), keys.keys.last().copied());
    Ok(if queried(a, &["curve"]) {
        let c = g.source(&PlugRef::new(mp, GEOMETRY_PATH as i32)).map(|s| crate::dag::transform_of(g, s.node));
        c.map(|c| json!(g.path(c))).unwrap_or(Value::Null)
    } else if queried(a, &["startTimeU"]) {
        first.map(|k| anim::frame_json(k.t)).unwrap_or(Value::Null)
    } else if queried(a, &["endTimeU"]) {
        last.map(|k| anim::frame_json(k.t)).unwrap_or(Value::Null)
    } else if queried(a, &["startU"]) {
        first.map(|k| json!(k.v)).unwrap_or(Value::Null)
    } else if queried(a, &["endU"]) {
        last.map(|k| json!(k.v)).unwrap_or(Value::Null)
    } else if queried(a, &["follow"]) {
        json!(n.bool(FOLLOW))
    } else if queried(a, &["fractionMode"]) {
        json!(n.bool(FRACTION_MODE))
    } else if queried(a, &["followAxis"]) {
        json!(["x", "y", "z"][n.int(FRONT_AXIS).clamp(0, 2) as usize])
    } else if queried(a, &["upAxis"]) {
        json!(["x", "y", "z"][n.int(UP_AXIS).clamp(0, 2) as usize])
    } else if queried(a, &["worldUpType"]) {
        json!(WORLD_UP_TYPES[n.int(WORLD_UP_TYPE).clamp(0, 4) as usize])
    } else if queried(a, &["worldUpVector"]) {
        let v = n.vec3(WORLD_UP_VECTOR);
        json!([v.x, v.y, v.z])
    } else if queried(a, &["worldUpObject"]) {
        g.source(&PlugRef::new(mp, WORLD_UP_MATRIX as i32)).map(|s| json!(g.path(s.node))).unwrap_or(Value::Null)
    } else if queried(a, &["inverseFront"]) {
        json!(n.bool(INVERSE_FRONT))
    } else if queried(a, &["inverseUp"]) {
        json!(n.bool(INVERSE_UP))
    } else if queried(a, &["bank"]) {
        json!(n.bool(BANK))
    } else if queried(a, &["bankScale"]) {
        json!(n.float(BANK_SCALE))
    } else if queried(a, &["bankThreshold", "bankLimit"]) {
        json!(n.float(BANK_LIMIT))
    } else {
        json!(g.path(mp))
    })
}

fn path_animation(s: &mut Session, a: &Args) -> CmdResult {
    if a.bool("query") || a.bool("edit") {
        let Some(name) = a.named().first().cloned() else { fail!("pathAnimation: name the motion path or its object") };
        let mp = path_of(&s.graph, &name)?;
        if a.bool("query") {
            return path_query(&mut s.graph, mp, a);
        }
        apply_path_flags(&mut s.graph, mp, a)?;
        return Ok(json!(s.graph.path(mp)));
    }
    let mut names = objects(s, a);
    // Without -c the last thing named or selected is the curve.
    let curve_name = match a.get("curve").and_then(Value::as_str) {
        Some(c) => c.to_string(),
        None => match names.pop() {
            Some(c) => c,
            None => fail!("pathAnimation: name a curve and an object"),
        },
    };
    let g = &mut s.graph;
    let Some(cid) = g.find(&curve_name)? else { fail!("pathAnimation: no object named {curve_name}") };
    let Some(shape) = curves::as_curve(g, cid) else { fail!("pathAnimation: {curve_name} is not a curve") };
    names.retain(|n| g.find(n).ok().flatten() != Some(cid));
    if names.is_empty() {
        fail!("pathAnimation: name an object to put on the path");
    }
    let ty = types::find("motionPath").unwrap();
    let mut out = Vec::new();
    for n in names {
        let id = object_id(g, &n, "pathAnimation")?;
        let mp = g.create_node(&ty, a.get("name").and_then(Value::as_str).unwrap_or("motionPath1"), NO_NODE, true)?;
        g.connect(&PlugRef::new(shape, CurveShape::WORLD_SPACE as i32), &PlugRef::new(mp, GEOMETRY_PATH as i32), false)?;
        let parent = g.parent(id);
        if parent != NO_NODE {
            g.connect(&PlugRef::new(parent, DagNode::WORLD_MATRIX as i32), &PlugRef::new(mp, PARENT_SPACE as i32), false)?;
        }
        let order = g.node(id).map(|x| x.data().int(Transform::ROTATE_ORDER)).unwrap_or(0);
        g.set_value(PlugRef::new(mp, ROTATE_ORDER as i32), PlugValue::Int(order))?;
        g.connect(&PlugRef::new(mp, ALL_COORDINATES as i32), &PlugRef::new(id, Transform::TRANSLATE as i32), true)?;
        // The command's own defaults (front x, up y, scene up) differ from the node's (front y, up z, vector up).
        for (at, v) in [(FRONT_AXIS, 0), (UP_AXIS, 1), (WORLD_UP_TYPE, 0)] {
            g.set_value(PlugRef::new(mp, at as i32), PlugValue::Int(v))?;
        }
        apply_path_flags(g, mp, a)?;
        out.push(g.path(mp));
    }
    Ok(if out.len() == 1 { json!(out[0]) } else { json!(out) })
}

// ---------------------------------------------------------------------------------------------------------------------
// snapshot and flow

/// snapshot's own defaults: frames 1 to 10, every frame.
fn snapshot_range(a: &Args) -> (f64, f64, f64) {
    (a.get("startTime").and_then(Value::as_f64).unwrap_or(1.0), a.get("endTime").and_then(Value::as_f64).unwrap_or(10.0), a.get("increment").and_then(Value::as_f64).unwrap_or(1.0).abs().max(1e-3))
}

/// Makes a snapshot's copies again from what its node holds: the objects at each of its frames, in its group.
fn rebuild_snapshot(s: &mut Session, node: NodeId) -> CmdResult<()> {
    let (start, end, by) = {
        let n = s.graph.node(node).unwrap().data();
        (n.float(0), n.float(1), n.float(2).abs().max(1e-3))
    };
    let info = s.graph.node(node).map(|n| json_of(n.data(), SNAP_INPUTS)).unwrap_or(Value::Null);
    let names: Vec<String> = info.get("objects").and_then(Value::as_array).map(|v| v.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default();
    let Some(group_name) = info.get("group").and_then(Value::as_str) else { return Ok(()) };
    let Some(group) = s.graph.find(group_name)? else { return Ok(()) };
    for k in s.graph.children(group).to_vec() {
        s.graph.delete_node(k)?;
    }
    fill_snapshot(s, &names, group, start, end, by)
}

fn fill_snapshot(s: &mut Session, names: &[String], group: NodeId, start: f64, end: f64, by: f64) -> CmdResult<()> {
    let now = anim::frame(&s.graph);
    for name in names {
        let id = object_id(&s.graph, name, "snapshot")?;
        for t in frames(start, end, by) {
            let world = world_at(&mut s.graph, id, t);
            let copy = s.run("duplicate", json!({"args": [s.graph.path(id)]}))?;
            let Some(copy_name) = copy.as_array().and_then(|v| v.first()).and_then(Value::as_str).map(str::to_string).or(copy.as_str().map(str::to_string)) else {
                fail!("snapshot: duplicate gave nothing back")
            };
            let Some(c) = s.graph.find(&copy_name)? else { fail!("snapshot: lost the copy {copy_name}") };
            // Each copy stays where the object was at that frame, with nothing driving it.
            s.graph.set_parent(c, group, false, usize::MAX)?;
            Transform::set_local_matrix(&mut s.graph, c, &world)?;
        }
    }
    anim::set_frame(&mut s.graph, now);
    s.graph.evaluate();
    Ok(())
}

const SNAP_INPUTS: usize = 4;
const SNAP_HISTORY: usize = 5;
const UPDATE_NAMES: [&str; 3] = ["always", "animCurve", "demand"];

fn snapshot(s: &mut Session, a: &Args) -> CmdResult {
    if a.bool("query") || a.bool("edit") {
        return snapshot_edit(s, a);
    }
    let (start, end, by) = snapshot_range(a);
    let names = objects(s, a);
    if names.is_empty() {
        fail!("snapshot: name or select an object");
    }
    let wanted = a.get("name").and_then(Value::as_str).map(str::to_string);
    if a.bool("motionTrail") {
        let id = object_id(&s.graph, &names[0], "snapshot")?;
        let anchor = match a.get("anchorTransform").and_then(Value::as_str) {
            Some(n) => Some(object_id(&s.graph, n, "snapshot")?),
            None => None,
        };
        return create_trail_in(&mut s.graph, id, wanted.as_deref(), start, end, by, anchor, a.bool("offsetParentMatrix"));
    }
    let ty = types::find("snapshot").unwrap();
    let node = s.graph.create_node(&ty, wanted.as_deref().unwrap_or("snapshot1"), NO_NODE, true)?;
    let base = s.graph.node(node).map(|n| n.name.clone()).unwrap_or_default();
    s.graph.set_value(PlugRef::new(node, 0), PlugValue::Float(start))?;
    s.graph.set_value(PlugRef::new(node, 1), PlugValue::Float(end))?;
    s.graph.set_value(PlugRef::new(node, 2), PlugValue::Float(by))?;
    let update = a.string("update", "always");
    let Some(ui) = UPDATE_NAMES.iter().position(|x| *x == update) else { fail!("snapshot: -update is always, animCurve or demand") };
    s.graph.set_value(PlugRef::new(node, 3), PlugValue::Int(ui as i64))?;
    s.graph.set_value(PlugRef::new(node, SNAP_HISTORY as i32), PlugValue::Bool(a.bool("constructionHistory") || a.bool("c")))?;
    let tt = types::find("transform").unwrap();
    let group = s.graph.create_node(&tt, &format!("{base}Group"), NO_NODE, true)?;
    let full: Vec<String> = names.iter().map(|n| object_id(&s.graph, n, "snapshot").map(|id| s.graph.path(id))).collect::<CmdResult<_>>()?;
    let info = json!({"objects": full, "group": s.graph.path(group)});
    set_json(&mut s.graph, node, SNAP_INPUTS, info)?;
    fill_snapshot(s, &full, group, start, end, by)?;
    Ok(json!([s.graph.path(group), s.graph.path(node)]))
}

/// snapshot -q and -e, on a snapshot node, its group, or a motion trail made by snapshot -mt.
fn snapshot_edit(s: &mut Session, a: &Args) -> CmdResult {
    let Some(n) = a.named().first().cloned() else { fail!("snapshot: name the snapshot") };
    let g = &s.graph;
    let Some(id) = g.find(&n)? else { fail!("snapshot: no object named {n}") };
    // The node: a snapshot, a motion trail, or found from a group or trail handle.
    let node = if g.node(id).is_some_and(|x| matches!(x.ty().name.as_str(), "snapshot" | "motionTrail")) {
        id
    } else {
        let path = g.path(id);
        let found = g.nodes().into_iter().find(|&k| {
            g.node(k).is_some_and(|x| x.ty().name == "snapshot" && json_of(x.data(), SNAP_INPUTS).get("group").and_then(Value::as_str) == Some(path.as_str()))
                || g.node(k).is_some_and(|x| x.ty().name == "motionTrail") && trail_handle(g, k) == Some(id)
        });
        match found {
            Some(k) => k,
            None => fail!("snapshot: {n} is not a snapshot"),
        }
    };
    let trail = g.node(node).is_some_and(|x| x.ty().name == "motionTrail");
    let (st, et, inc) = if trail { (TRAIL_START, TRAIL_END, TRAIL_INCREMENT) } else { (0, 1, 2) };
    if a.bool("query") {
        let d = g.node(node).unwrap().data();
        return Ok(if queried(a, &["startTime"]) {
            json!(d.float(st))
        } else if queried(a, &["endTime"]) {
            json!(d.float(et))
        } else if queried(a, &["increment"]) {
            json!(d.float(inc))
        } else if queried(a, &["update"]) {
            json!(if trail { "always" } else { UPDATE_NAMES.get(d.int(3) as usize).copied().unwrap_or("always") })
        } else if queried(a, &["constructionHistory"]) {
            json!(!trail && d.bool(SNAP_HISTORY))
        } else if queried(a, &["anchorTransform"]) {
            json!(if trail { g.source(&PlugRef::new(node, TRAIL_ANCHOR as i32)).map(|x| g.path(x.node)).unwrap_or_default() } else { String::new() })
        } else {
            json!(g.path(node))
        });
    }
    // Edit.
    let mut changed = false;
    for (flag, at) in [("startTime", st), ("endTime", et), ("increment", inc)] {
        if let Some(v) = a.get(flag).and_then(Value::as_f64) {
            s.graph.set_value(PlugRef::new(node, at as i32), PlugValue::Float(if flag == "increment" { v.abs().max(1e-3) } else { v }))?;
            changed = true;
        }
    }
    if let Some(u) = a.get("update").and_then(Value::as_str) {
        let Some(ui) = UPDATE_NAMES.iter().position(|x| *x == u) else { fail!("snapshot: -update is always, animCurve or demand") };
        if !trail {
            s.graph.set_value(PlugRef::new(node, 3), PlugValue::Int(ui as i64))?;
        }
    }
    if trail {
        if let Some(opm) = a.get("offsetParentMatrix").and_then(Value::as_bool) {
            place_trail_handle(&mut s.graph, node, opm)?;
        }
        if changed {
            refresh_trail(&mut s.graph, node)?;
        }
    } else if changed {
        rebuild_snapshot(s, node)?;
    }
    if let Some(name) = a.get("name").and_then(Value::as_str) {
        s.graph.rename(node, name)?;
    }
    let g = &s.graph;
    Ok(if trail {
        json!([trail_handle(g, node).map(|h| g.path(h)).unwrap_or_default(), g.path(node)])
    } else {
        let grp = g.node(node).map(|x| json_of(x.data(), SNAP_INPUTS)).and_then(|v| v.get("group").and_then(Value::as_str).map(str::to_string)).unwrap_or_default();
        json!([grp, g.path(node)])
    })
}

fn three(v: Option<&Value>, def: [f64; 3]) -> [f64; 3] {
    let flat: Vec<f64> = match v {
        Some(Value::Array(x)) => x.iter().flat_map(|e| match e {
            Value::Array(i) => i.iter().filter_map(Value::as_f64).collect::<Vec<_>>(),
            o => o.as_f64().into_iter().collect(),
        }).collect(),
        _ => Vec::new(),
    };
    if flat.len() >= 3 { [flat[0], flat[1], flat[2]] } else { def }
}

/// The flow node a name stands for: itself, or the one an object flows through.
fn flow_node(g: &Graph, name: &str) -> Option<NodeId> {
    let id = g.find(name).ok()??;
    if g.node(id).is_some_and(|n| n.ty().name == "flow") {
        return Some(id);
    }
    let t = crate::dag::transform_of(g, id);
    let msg = g.node(t)?.ty().attr_index("message")? as i32;
    g.destinations(&PlugRef::new(t, msg)).into_iter().map(|p| p.node).find(|&n| g.node(n).is_some_and(|x| x.ty().name == "flow"))
}

fn flow(s: &mut Session, a: &Args) -> CmdResult {
    let names = objects(s, a);
    let Some(name) = names.first().cloned() else { fail!("flow: name or select an object on a motion path") };
    if a.bool("query") {
        let g = &s.graph;
        let Some(node) = flow_node(g, &name) else { fail!("flow: {name} has no flow") };
        let n = g.node(node).unwrap();
        let ints = |v: DVec3| json!([v.x.round() as i64, v.y.round() as i64, v.z.round() as i64]);
        return Ok(if queried(a, &["divisions"]) {
            ints(n.vec3(0))
        } else if queried(a, &["localDivisions"]) {
            ints(n.vec3(1))
        } else if queried(a, &["localCompute"]) {
            json!(n.bool(2))
        } else if queried(a, &["objectCentered"]) {
            json!(n.bool(3))
        } else {
            fail!("flow: name a flag to query: -dv, -ld, -lc or -oc")
        });
    }
    let mp = path_of(&s.graph, &name)?;
    if s.commands.find("lattice").is_none() {
        fail!("flow: needs the lattice deformer, which is not in this build");
    }
    let centred = !a.has("objectCentered") || a.bool("objectCentered");
    // Local control is on for a lattice round the path, off for one round the object, unless asked.
    let local = if a.has("localCompute") { a.bool("localCompute") } else { !centred };
    let dv = three(a.get("divisions"), [2.0, 5.0, 2.0]);
    let ld = three(a.get("localDivisions"), [2.0, 2.0, 2.0]);
    let mut call = serde_json::Map::new();
    call.insert("args".into(), json!([name]));
    call.insert("divisions".into(), json!(dv));
    call.insert("ldivisions".into(), json!(ld));
    if !centred {
        // Round the path: the lattice takes the curve's bounding box.
        let g = &mut s.graph;
        let Some(src) = g.source(&PlugRef::new(mp, GEOMETRY_PATH as i32)) else { fail!("flow: {name}'s motion path has no curve") };
        let c = curves::world_curve(g, src.node).unwrap_or_default();
        let pts = c.tessellate(16);
        let (lo, hi) = pts.iter().fold((DVec3::splat(f64::MAX), DVec3::splat(f64::MIN)), |(l, h), p| (l.min(*p), h.max(*p)));
        let size = (hi - lo).max(DVec3::splat(1e-3));
        call.insert("position".into(), json!(((lo + hi) * 0.5).to_array()));
        call.insert("scale".into(), json!(size.to_array()));
    }
    s.graph.open_chunk("Flow");
    let r = (|| -> CmdResult {
        let made = s.run("lattice", Value::Object(call))?;
        let list: Vec<String> = made.as_array().map(|v| v.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default();
        let g = &mut s.graph;
        let Some(ffd) = list.first().and_then(|n| g.find(n).ok().flatten()) else { fail!("flow: the lattice gave nothing back") };
        if let Some(i) = g.node(ffd).and_then(|n| n.ty().attr_index("local")) {
            g.set_value(PlugRef::new(ffd, i as i32), PlugValue::Bool(local))?;
        }
        let t = types::find("flow").unwrap();
        let node = g.create_node(&t, "flow1", NO_NODE, true)?;
        g.set_value(PlugRef::new(node, 0), PlugValue::Vec3(DVec3::from_array(dv)))?;
        g.set_value(PlugRef::new(node, 1), PlugValue::Vec3(DVec3::from_array(ld)))?;
        g.set_value(PlugRef::new(node, 2), PlugValue::Bool(local))?;
        g.set_value(PlugRef::new(node, 3), PlugValue::Bool(centred))?;
        let obj = crate::dag::transform_of(g, g.find(&name)?.unwrap());
        let msg = |g: &Graph, n: NodeId| g.node(n).and_then(|x| x.ty().attr_index("message")).unwrap() as i32;
        let (om, fm) = (msg(g, obj), msg(g, ffd));
        g.connect(&PlugRef::new(obj, om), &PlugRef::new(node, 4), false)?;
        g.connect(&PlugRef::new(ffd, fm), &PlugRef::new(node, 5), false)?;
        g.evaluate();
        let mut out = vec![json!(g.path(node))];
        out.extend(list.into_iter().map(Value::from));
        Ok(Value::Array(out))
    })();
    s.graph.close_chunk();
    r
}

// ---------------------------------------------------------------------------------------------------------------------

pub fn register_types() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let store = flags::INPUT | flags::STORABLE;
        let float = |name: &str, short: &str, def: f64, keyable: bool, doc: &str| AttrDef {
            name: name.into(),
            short_name: short.into(),
            ty: PlugType::Float,
            default_value: PlugValue::Float(def),
            flags: store | if keyable { flags::KEYABLE } else { 0 },
            doc: doc.into(),
            ..Default::default()
        };
        let boolean = |name: &str, short: &str, def: bool, doc: &str| AttrDef {
            name: name.into(),
            short_name: short.into(),
            ty: PlugType::Bool,
            default_value: PlugValue::Bool(def),
            flags: store,
            doc: doc.into(),
            ..Default::default()
        };
        let axis = |name: &str, short: &str, def: i64, doc: &str| AttrDef {
            name: name.into(),
            short_name: short.into(),
            ty: PlugType::Enum,
            default_value: PlugValue::Int(def),
            flags: store,
            enum_names: vec!["X".into(), "Y".into(), "Z".into()],
            doc: doc.into(),
            ..Default::default()
        };
        let times = |name: &str, short: &str, doc: &str| AttrDef {
            name: name.into(),
            short_name: short.into(),
            ty: PlugType::Float,
            default_value: PlugValue::Float(0.0),
            flags: store | flags::ARRAY,
            doc: doc.into(),
            ..Default::default()
        };
        let mut coords = vec3_attr("allCoordinates", "ac", DVec3::ZERO, flags::OUTPUT, "Where the object goes, in its parent's space.");
        coords.child_names = ["xCoordinate".into(), "yCoordinate".into(), "zCoordinate".into()];
        coords.child_short = ["xc".into(), "yc".into(), "zc".into()];
        let hidden_matrix = |name: &str, short: &str, doc: &str| AttrDef {
            name: name.into(),
            short_name: short.into(),
            ty: PlugType::Matrix,
            default_value: PlugValue::Matrix(DMat4::IDENTITY),
            flags: flags::INPUT,
            doc: doc.into(),
            ..Default::default()
        };
        types::register_type(NodeType {
            name: "motionPath".into(),
            icon: "chart-line".into(),
            doc: "Puts an object on a curve at uValue, and turns it along the curve if follow is on.".into(),
            own_attrs: vec![
                float("uValue", "u", 0.0, true, "Where on the curve: its parameter, or a fraction of its length with fractionMode."),
                float("frontTwist", "ft", 0.0, true, "Turn about the front axis, degrees."),
                float("upTwist", "ut", 0.0, true, "Turn about the up axis, degrees."),
                float("sideTwist", "st", 0.0, true, "Turn about the side axis, degrees."),
                boolean("fractionMode", "fm", false, "uValue is a fraction of the curve's length, so equal steps cover equal distance."),
                boolean("follow", "f", false, "Turn the object along the path."),
                AttrDef {
                    name: "worldUpType".into(),
                    short_name: "wut".into(),
                    ty: PlugType::Enum,
                    default_value: PlugValue::Int(3),
                    flags: store,
                    enum_names: WORLD_UP_TYPES.iter().map(|s| s.to_string()).collect(),
                    doc: "What up means: the scene's, towards an object, an object's rotation of worldUpVector, worldUpVector itself, or the curve's normal.".into(),
                    ..Default::default()
                },
                vec3_attr("worldUpVector", "wu", DVec3::Y, store, "The up direction for the vector and object rotation types."),
                hidden_matrix("worldUpMatrix", "wum", "The up object's world matrix."),
                axis("frontAxis", "fa", 1, "The object's axis along the path."),
                axis("upAxis", "ua", 2, "The object's axis towards up."),
                boolean("inverseFront", "if", false, "The front axis points back along the path."),
                boolean("inverseUp", "iu", false, "The up axis points away from up."),
                boolean("bank", "b", false, "Lean into turns."),
                float("bankScale", "bs", 1.0, false, "How far it leans for a given turn."),
                float("bankLimit", "bl", 90.0, false, "The most it leans, degrees."),
                AttrDef {
                    name: "geometryPath".into(),
                    short_name: "gp".into(),
                    ty: PlugType::Data,
                    default_value: PlugValue::Data(None),
                    flags: flags::INPUT | flags::HIDDEN,
                    doc: "The curve, from its worldSpace.".into(),
                    ..Default::default()
                },
                hidden_matrix("parentSpaceMatrix", "psm", "The driven object's parent's world matrix."),
                AttrDef { name: "rotateOrder".into(), short_name: "ro".into(), ty: PlugType::Int, default_value: PlugValue::Int(0), flags: store, doc: "The driven object's rotate order.".into(), ..Default::default() },
                times("positionMarkerTime", "pmt", "Frames of the position markers."),
                times("orientationMarkerTime", "omt", "Frames of the orientation markers."),
                coords,
                vec3_attr("rotate", "r", DVec3::ZERO, flags::OUTPUT, "How the object turns, degrees."),
                AttrDef { name: "orientMatrix".into(), short_name: "om".into(), ty: PlugType::Matrix, default_value: PlugValue::Matrix(DMat4::IDENTITY), flags: flags::OUTPUT, doc: "Position and rotation together.".into(), ..Default::default() },
            ],
            create: Some(Arc::new(|| Box::new(MotionPath))),
            ..Default::default()
        })
        .expect("motionPath");

        // Markers: a locator each, under the path curve, keeping the time of the key it stands for.
        let locator = types::find("locator").expect("locator");
        let mut time = AttrDef { name: "time".into(), short_name: "t".into(), ty: PlugType::Float, default_value: PlugValue::Float(0.0), flags: store | flags::OUTPUT, doc: "The frame of the key this marker stands for.".into(), ..Default::default() };
        time.unit = aztek_ops::units::Unit::None;
        types::register_type(NodeType {
            name: "positionMarker".into(),
            icon: "crosshair".into(),
            base: Some(locator.clone()),
            dag: true,
            doc: "A key of a motion path's uValue, shown where it puts the object on the curve.".into(),
            own_attrs: vec![time],
            create: locator.create.clone(),
            ..Default::default()
        })
        .expect("positionMarker");
        let pm = types::find("positionMarker").unwrap();
        let twist = |name: &str, short: &str, doc: &str| AttrDef { name: name.into(), short_name: short.into(), ty: PlugType::Float, default_value: PlugValue::Float(0.0), flags: store, unit: aztek_ops::units::Unit::Angle, doc: doc.into(), ..Default::default() };
        types::register_type(NodeType {
            name: "orientationMarker".into(),
            icon: "crosshair".into(),
            base: Some(pm),
            dag: true,
            doc: "Keys of a motion path's frontTwist, upTwist and sideTwist at one time.".into(),
            own_attrs: vec![twist("frontTwist", "ft", "Turn about the front direction."), twist("sideTwist", "st", "Turn about the side direction."), twist("upTwist", "ut", "Turn about the up direction.")],
            create: locator.create.clone(),
            ..Default::default()
        })
        .expect("orientationMarker");

        let msg_in = |name: &str, short: &str, doc: &str| AttrDef { name: name.into(), short_name: short.into(), ty: PlugType::Int, default_value: PlugValue::Int(0), flags: flags::INPUT | flags::MESSAGE | flags::HIDDEN, doc: doc.into(), ..Default::default() };
        types::register_type(NodeType {
            name: "flow".into(),
            icon: "route".into(),
            doc: "Bends an object on a motion path through a lattice: the settings flow made it with.".into(),
            own_attrs: vec![
                vec3_attr("divisions", "dv", DVec3::new(2.0, 5.0, 2.0), store, "Lattice slices in X, Y and Z."),
                vec3_attr("localDivisions", "ld", DVec3::splat(2.0), store, "The local region of effect in X, Y and Z."),
                boolean("localCompute", "lc", false, "The deformation has local control."),
                boolean("objectCentered", "oc", true, "The lattice is round the object; off, round the path."),
                msg_in("object", "obj", "The object flowing."),
                msg_in("deformer", "dfm", "The lattice deformer."),
            ],
            create: Some(Arc::new(|| Box::new(PlainNode))),
            ..Default::default()
        })
        .expect("flow");
        types::register_type(NodeType {
            name: "snapshot".into(),
            icon: "chart-line".into(),
            doc: "Records how a snapshot's copies were made.".into(),
            own_attrs: vec![
                float("startTime", "st", 1.0, false, "First frame."),
                float("endTime", "et", 10.0, false, "Last frame."),
                float("increment", "i", 1.0, false, "Frames between copies."),
                AttrDef { name: "update".into(), short_name: "up".into(), ty: PlugType::Enum, default_value: PlugValue::Int(0), flags: store, enum_names: vec!["always".into(), "animCurve".into(), "demand".into()], doc: "When copies update: always, animCurve or demand.".into(), ..Default::default() },
                data_attr("inputObjects", "iob", "The objects copied and the group holding the copies, by name."),
                boolean("constructionHistory", "ch", false, "Made with construction history."),
            ],
            create: Some(Arc::new(|| Box::new(PlainNode))),
            ..Default::default()
        })
        .expect("snapshot");

        types::register_type(NodeType {
            name: "motionTrail".into(),
            icon: "chart-line".into(),
            doc: "An object's path through time, sampled.".into(),
            own_attrs: vec![
                float("startTime", "st", 1.0, false, "First frame."),
                float("endTime", "et", 120.0, false, "Last frame."),
                float("increment", "inc", 1.0, false, "Frames between points."),
                hidden_matrix("inputMatrix", "im", "The object followed."),
                data_attr("points", "pts", "World positions, one per sample."),
                data_attr("keyframeTimes", "kt", "The frames with keys."),
                hidden_matrix("anchorMatrix", "anm", "The anchor transform's world matrix: the points are in its space."),
            ],
            create: Some(Arc::new(|| Box::new(PlainNode))),
            ..Default::default()
        })
        .expect("motionTrail");

        let shape = types::find("shape").expect("shape");
        types::register_type(NodeType {
            name: "motionTrailShape".into(),
            icon: "crosshair".into(),
            base: Some(shape),
            dag: true,
            doc: "Draws a motion trail.".into(),
            own_attrs: vec![data_attr("points", "pts", "World positions, one per sample."), data_attr("keyframes", "kf", "World positions at the keys.")],
            create: Some(Arc::new(|| Box::new(TrailShape))),
            ..Default::default()
        })
        .expect("motionTrailShape");
    });
}


// ---------------------------------------------------------------------------------------------------------------------
// marker

/// How far along a curve's length a parameter is, 0 to 1.
fn fraction_at_param(c: &NurbsCurve, u: f64) -> f64 {
    let (lo, hi) = c.domain();
    let n = (c.spans() * 32).max(32);
    let (mut total, mut upto) = (0.0, 0.0);
    let mut last = c.eval(lo);
    for i in 1..=n {
        let t = lo + (hi - lo) * i as f64 / n as f64;
        let p = c.eval(t);
        let d = (p - last).length();
        total += d;
        if t <= u {
            upto += d;
        } else if t - (hi - lo) / (n as f64) < u {
            upto += d * (u - (t - (hi - lo) / (n as f64))) / ((hi - lo) / (n as f64));
        }
        last = p;
    }
    if total > 1e-12 { upto / total } else { 0.0 }
}

/// The motion paths a curve drives.
fn paths_on_curve(g: &Graph, curve_shape: NodeId) -> Vec<NodeId> {
    g.destinations(&PlugRef::new(curve_shape, CurveShape::WORLD_SPACE as i32)).into_iter().filter(|d| is_motion_path(g, d.node) && d.attr == GEOMETRY_PATH as i32).map(|d| d.node).collect()
}

/// A marker's motion path and which of its marker time arrays it is in.
fn marker_path(g: &Graph, marker: NodeId) -> Option<(NodeId, usize)> {
    let at = g.node(marker)?.ty().attr_index("time")? as i32;
    g.destinations(&PlugRef::new(marker, at)).into_iter().find(|d| is_motion_path(g, d.node)).map(|d| (d.node, d.attr as usize))
}

fn is_marker(g: &Graph, id: NodeId) -> bool {
    g.node(id).is_some_and(|n| n.ty().is_a("positionMarker"))
}

/// A marker named, or the marker shape under a transform named.
fn marker_named(g: &Graph, name: &str) -> CmdResult<NodeId> {
    let Some(id) = g.find(name)? else { fail!("marker: no object named {name}") };
    if is_marker(g, id) {
        return Ok(id);
    }
    match g.children(id).iter().copied().find(|&c| is_marker(g, c)) {
        Some(m) => Ok(m),
        None => fail!("marker: {name} is not a marker"),
    }
}

/// A channel's value at frame t: its curve's there, or the value it holds.
fn value_at(g: &Graph, p: &PlugRef, t: f64) -> f64 {
    match anim::curve_of(g, p) {
        Some(c) => {
            let k = anim::keys_of(g, c);
            if k.keys.is_empty() {
                g.node(p.node).map(|n| n.data().float(p.attr as usize)).unwrap_or(0.0)
            } else {
                k.eval_inside(t.clamp(k.keys[0].t, k.keys[k.keys.len() - 1].t), k.weighted)
            }
        }
        None => g.node(p.node).map(|n| n.data().float(p.attr as usize)).unwrap_or(0.0),
    }
}

/// Where u puts a point on the path's curve, in the curve's object space.
fn point_on_path(g: &mut Graph, mp: NodeId, curve_shape: NodeId, u: f64) -> DVec3 {
    let fraction = g.node(mp).is_some_and(|n| n.data().bool(FRACTION_MODE));
    match curves::local_curve(g, curve_shape) {
        Some(c) => {
            let (lo, hi) = c.domain();
            c.eval(if fraction { c.param_at_fraction(u.clamp(0.0, 1.0)) } else { u.clamp(lo, hi) })
        }
        None => DVec3::ZERO,
    }
}

/// The group under the curve's transform that holds its markers, made the first time.
fn marker_group(g: &mut Graph, curve_shape: NodeId) -> CmdResult<NodeId> {
    let xf = crate::dag::transform_of(g, curve_shape);
    let base = g.node(xf).map(|n| n.name.rsplit(':').next().unwrap_or("").to_string()).unwrap_or_default();
    let wanted = format!("{base}Markers");
    if let Some(&grp) = g.children(xf).iter().find(|&&c| g.node(c).is_some_and(|n| n.name == wanted && n.ty().is_a("transform"))) {
        return Ok(grp);
    }
    let t = types::find("transform").unwrap();
    g.create_node(&t, &wanted, xf, true).map_err(Into::into)
}

fn marker(s: &mut Session, a: &Args) -> CmdResult {
    let names = a.named();
    let g = &mut s.graph;
    let to_user = |v: f64| aztek_ops::units::to_user(aztek_ops::units::Unit::Angle, v);
    let from_user = |v: f64| aztek_ops::units::to_stored(aztek_ops::units::Unit::Angle, v);
    if a.bool("query") {
        let Some(name) = names.first() else { fail!("marker: name a marker") };
        let m = marker_named(g, name)?;
        let n = g.node(m).unwrap();
        let ty = n.ty().clone();
        let t = n.data().float(ty.attr_index("time").unwrap());
        if queried(a, &["time"]) {
            return Ok(anim::frame_json(t));
        }
        if queried(a, &["positionMarker"]) {
            return Ok(json!(!ty.is_a("orientationMarker")));
        }
        if queried(a, &["orientationMarker"]) {
            return Ok(json!(ty.is_a("orientationMarker")));
        }
        let path = marker_path(g, m).map(|x| x.0);
        for (flag, attr, own) in [("frontTwist", FRONT_TWIST, "frontTwist"), ("upTwist", UP_TWIST, "upTwist"), ("sideTwist", SIDE_TWIST, "sideTwist")] {
            if queried(a, &[flag]) {
                let v = match path {
                    Some(mp) => value_at(g, &PlugRef::new(mp, attr as i32), t),
                    None => ty.attr_index(own).map(|i| n.data().float(i)).unwrap_or(0.0),
                };
                return Ok(json!(to_user(v)));
            }
        }
        if queried(a, &["valueU"]) {
            let Some(mp) = path else { fail!("marker: {name} is on no motion path") };
            return Ok(json!(value_at(g, &u_plug(mp), t)));
        }
        return Ok(anim::frame_json(t));
    }
    // -attach and -detach work on the position markers named or selected.
    if a.bool("attach") || a.bool("detach") {
        let picked: Vec<NodeId> = if names.is_empty() {
            g.selection().iter().filter_map(|&i| marker_named(g, &g.path(i)).ok()).collect()
        } else {
            names.iter().map(|n| marker_named(g, n)).collect::<CmdResult<Vec<_>>>()?
        };
        let mut out = Vec::new();
        for m in picked {
            let Some((mp, _)) = marker_path(g, m) else { continue };
            let Some(curve) = g.source(&PlugRef::new(mp, GEOMETRY_PATH as i32)).map(|p| p.node) else { continue };
            let xf = crate::dag::transform_of(g, m);
            if a.bool("detach") {
                // Into 3D space: the marker keeps where it is in the world, no longer under the curve.
                g.evaluate();
                let world = g.node(m).map(|n| n.data().matrix(DagNode::WORLD_MATRIX)).unwrap_or_default();
                let lp = g.node(m).map(|n| n.data().vec3(crate::transform::Locator::LOCAL_POSITION)).unwrap_or_default();
                let at = world.transform_point3(lp);
                let tt = types::find("transform").unwrap();
                let own = g.create_node(&tt, &format!("{}3D", g.node(m).unwrap().name), NO_NODE, true)?;
                g.set_parent(m, own, false, usize::MAX)?;
                g.set_value(PlugRef::new(m, crate::transform::Locator::LOCAL_POSITION as i32), PlugValue::Vec3(at))?;
                let _ = xf;
            } else {
                // Back on the curve: the nearest point, which becomes the marker's key on uValue.
                g.evaluate();
                let world = g.node(m).map(|n| n.data().matrix(DagNode::WORLD_MATRIX)).unwrap_or_default();
                let lp = g.node(m).map(|n| n.data().vec3(crate::transform::Locator::LOCAL_POSITION)).unwrap_or_default();
                let at = world.transform_point3(lp);
                let grp = marker_group(g, curve)?;
                let old_parent = crate::dag::transform_of(g, m);
                if old_parent != grp {
                    g.set_parent(m, grp, false, usize::MAX)?;
                    if g.children(old_parent).is_empty() {
                        g.delete_node(old_parent)?;
                    }
                }
                if let Some(c) = curves::world_curve(g, curve) {
                    let u = c.closest_param(at);
                    let fraction = g.node(mp).is_some_and(|n| n.data().bool(FRACTION_MODE));
                    let u = if fraction { fraction_at_param(&c, u) } else { u };
                    let t = g.node(m).map(|n| n.data().float(n.ty().attr_index("time").unwrap())).unwrap_or(0.0);
                    anim::key_plug(g, &u_plug(mp), t, Some(u), None, None)?;
                    let p = point_on_path(g, mp, curve, u);
                    g.set_value(PlugRef::new(m, crate::transform::Locator::LOCAL_POSITION as i32), PlugValue::Vec3(p))?;
                }
            }
            out.push(g.path(m));
        }
        return Ok(json!(out));
    }
    // Create: on the curve named (the selection when none), at -t (the current time).
    let curve_name = match names.first().cloned().or_else(|| g.selection().first().map(|&i| g.path(i))) {
        Some(n) => n,
        None => fail!("marker: name the path curve"),
    };
    let Some(cid) = g.find(&curve_name)? else { fail!("marker: no object named {curve_name}") };
    let Some(curve) = curves::as_curve(g, cid) else { fail!("marker: {curve_name} is not a curve") };
    let Some(&mp) = paths_on_curve(g, curve).first() else { fail!("marker: {curve_name} is not a motion path") };
    let t = a.get("time").and_then(Value::as_f64).unwrap_or_else(|| anim::frame(g));
    let orient = a.bool("orientationMarker");
    let position = a.bool("positionMarker") || !orient;
    let mut out = Vec::new();
    g.open_chunk("marker");
    let r = (|| -> CmdResult<()> {
        for (want, kind, times_attr) in [(position, "positionMarker", POSITION_MARKER_TIME), (orient, "orientationMarker", ORIENTATION_MARKER_TIME)] {
            if !want {
                continue;
            }
            // One marker of each kind a time: the one there already is updated.
            let existing = g.elements(&PlugRef::new(mp, times_attr as i32)).into_iter().filter_map(|e| g.source(&e)).map(|p| p.node).find(|&m| {
                g.node(m).is_some_and(|n| n.ty().name == kind && (n.data().float(n.ty().attr_index("time").unwrap()) - t).abs() < 1e-6)
            });
            let m = match existing {
                Some(m) => m,
                None => {
                    let grp = marker_group(g, curve)?;
                    let ty = types::find(kind).unwrap();
                    let m = g.create_node(&ty, &format!("{kind}1"), grp, true)?;
                    let at = ty.attr_index("time").unwrap() as i32;
                    g.set_value(PlugRef::new(m, at), PlugValue::Float(t))?;
                    g.connect(&PlugRef::new(m, at), &PlugRef::new(mp, times_attr as i32), false)?;
                    m
                }
            };
            let u = if kind == "positionMarker" {
                let u = a.get("valueU").and_then(Value::as_f64).unwrap_or_else(|| value_at(g, &u_plug(mp), t));
                anim::key_plug(g, &u_plug(mp), t, Some(u), None, None)?;
                u
            } else {
                for (flag, attr, own) in [("frontTwist", FRONT_TWIST, "frontTwist"), ("upTwist", UP_TWIST, "upTwist"), ("sideTwist", SIDE_TWIST, "sideTwist")] {
                    let v = a.get(flag).and_then(Value::as_f64).map(from_user).unwrap_or(0.0);
                    anim::key_plug(g, &PlugRef::new(mp, attr as i32), t, Some(v), None, None)?;
                    let i = g.node(m).unwrap().ty().attr_index(own).unwrap() as i32;
                    g.set_value(PlugRef::new(m, i), PlugValue::Float(v))?;
                }
                value_at(g, &u_plug(mp), t)
            };
            let p = point_on_path(g, mp, curve, u);
            g.set_value(PlugRef::new(m, crate::transform::Locator::LOCAL_POSITION as i32), PlugValue::Vec3(p))?;
            out.push(g.path(m));
        }
        Ok(())
    })();
    g.close_chunk();
    r?;
    Ok(json!(out))
}

pub fn register(c: &mut Commands<Session>) {
    register_types();

    c.register(
        query_flags(Cmd::new(
            "marker",
            "Markers on a motion path's curve at -t (the current time): a position marker (-pm, the default) keys uValue at -u, an \
             orientation marker (-om) keys the twists (-ft, -ut, -st). One of each kind a time. -a and -d attach and detach \
             position markers. Returns the markers; -q reads one.",
            marker,
        ))
        .positional("the path curve, or a marker with -q")
        .flag(f("attach", "a", ArgType::Bool, "Attach the selected position markers to their curve."))
        .flag(f("detach", "d", ArgType::Bool, "Detach the selected position markers from their curve into 3D space."))
        .flag(f("positionMarker", "pm", ArgType::Bool, "A position marker (the default)."))
        .flag(f("orientationMarker", "om", ArgType::Bool, "An orientation marker."))
        .flag(f("time", "t", ArgType::Float, "The marker's frame (the current one)."))
        .flag(f("valueU", "u", ArgType::Float, "Where the position marker is on the curve (where the path is at that time)."))
        .flag(f("frontTwist", "ft", ArgType::Float, "Turn about the front direction, degrees (0)."))
        .flag(f("upTwist", "ut", ArgType::Float, "Turn about the up direction, degrees (0)."))
        .flag(f("sideTwist", "st", ArgType::Float, "Turn about the side direction, degrees (0)."))
        .undoable(true),
    );

    c.register(
        query_flags(Cmd::new(
            "pathAnimation",
            "Put objects on a curve: a motionPath node drives their translate, and with -f their rotate. -stu and -etu key uValue from -su to -eu (the curve's ends). \
             Returns the motion path. -q and -e take the motion path or its object.",
            path_animation,
        ))
        .positional("objects (the selection when none); without -c the last is the curve")
        .flag(f("curve", "c", ArgType::String, "The path."))
        .flag(f("name", "n", ArgType::String, "The motion path's name."))
        .flag(f("follow", "f", ArgType::Bool, "Turn objects along the path."))
        .flag(f("fractionMode", "fm", ArgType::Bool, "U is a fraction of the length."))
        .flag(f("followAxis", "fa", ArgType::String, "The object's axis along the path: x, y or z (x)."))
        .flag(f("upAxis", "ua", ArgType::String, "The object's axis towards up: x, y or z (y)."))
        .flag(f("worldUpType", "wut", ArgType::String, "scene, object, objectrotation, vector or normal."))
        .flag(f("worldUpVector", "wu", ArgType::Vec3, "The up direction."))
        .flag(f("worldUpObject", "wuo", ArgType::String, "The up object."))
        .flag(f("useNormal", "un", ArgType::Bool, "Up is the curve's normal."))
        .flag(f("inverseUp", "iu", ArgType::Bool, "The up axis points away from up."))
        .flag(f("inverseFront", "if", ArgType::Bool, "The front axis points back along the path."))
        .flag(f("bank", "b", ArgType::Bool, "Lean into turns."))
        .flag(f("bankScale", "bs", ArgType::Float, "How far it leans."))
        .flag(f("bankThreshold", "bt", ArgType::Float, "The most it leans, degrees."))
        .flag(f("bankLimit", "bl", ArgType::Float, "As -bt."))
        .flag(f("startTimeU", "stu", ArgType::Float, "The frame it starts."))
        .flag(f("endTimeU", "etu", ArgType::Float, "The frame it ends."))
        .flag(f("startU", "su", ArgType::Float, "The U it starts at (the curve's start)."))
        .flag(f("endU", "eu", ArgType::Float, "The U it ends at (the curve's end).")),
    );

    c.register(
        Cmd::new(
            "flow",
            "Deform an object on a motion path through a lattice: round the object (-oc on) or round the path. Returns the flow node, \
             then the ffd, the lattice and its base. -q on the flow node or the object reads -dv, -ld, -lc and -oc.",
            flow,
        )
        .positional("the object (the selection when none); the flow node or the object with -q")
        .flag(f("divisions", "dv", ArgType::Any, "Lattice slices in X, Y and Z (2 5 2).").multi(3))
        .flag(f("localCompute", "lc", ArgType::Bool, "Local control of the deformation: on for a lattice round the path, off round the object."))
        .flag(f("localDivisions", "ld", ArgType::Any, "The local region of effect in X, Y and Z (2 2 2).").multi(3))
        .flag(f("objectCentered", "oc", ArgType::Bool, "Round the object (on); off, round the path."))
        .flag(f("query", "q", ArgType::Bool, "Query mode."))
        .flag(f("queried", "", ArgType::List, "Flags asked about in query mode.")),
    );

    c.register(
        Cmd::new(
            "snapshot",
            "Copies of objects at every -i frames from -st to -et, in a group. Returns the group and the snapshot node. With -mt, makes a motion trail instead.",
            snapshot,
        )
        .positional("objects (the selection when none); with -q or -e the snapshot node, its group, or a trail")
        .flag(f("startTime", "st", ArgType::Float, "First frame (1)."))
        .flag(f("endTime", "et", ArgType::Float, "Last frame (10)."))
        .flag(f("increment", "i", ArgType::Float, "Frames between copies (1)."))
        .flag(f("constructionHistory", "ch", ArgType::Bool, "Recorded on the node; -e with new times makes the copies again."))
        .flag(f("c", "c", ArgType::Bool, "As -ch."))
        .flag(f("name", "n", ArgType::String, "The snapshot's name."))
        .flag(f("update", "u", ArgType::String, "always (the default), animCurve or demand: recorded on the node.").choices(&["always", "animCurve", "demand"]))
        .flag(f("motionTrail", "mt", ArgType::Bool, "Make a motion trail."))
        .flag(f("anchorTransform", "at", ArgType::String, "With -mt: the trail is drawn in this transform's space, its handle under it."))
        .flag(f("offsetParentMatrix", "opm", ArgType::Bool, "With -at: the handle follows the anchor through its offsetParentMatrix instead of being parented under it."))
        .flag(f("query", "q", ArgType::Bool, "Query mode."))
        .flag(f("edit", "e", ArgType::Bool, "Edit mode: new times make the copies, or the trail, again."))
        .flag(f("queried", "", ArgType::List, "The flags asked about in query mode.")),
    );

    c.register(
        query_flags(Cmd::new(
            "motionTrail",
            "An object's path through time, drawn in the viewport with a cross at each key: creates motionTrail1Handle and motionTrail1. -e -ev samples it again. -q -pts gives the points, -q -kt the key frames.",
            |s, a| {
                if a.bool("query") || a.bool("edit") {
                    let Some(n) = a.named().first().cloned() else { fail!("motionTrail: name the trail") };
                    let g = &mut s.graph;
                    let Some(id) = g.find(&n)? else { fail!("motionTrail: no object named {n}") };
                    // The handle answers for its trail.
                    let shape = g.children(id).first().copied().unwrap_or(id);
                    let trail = if g.node(id).is_some_and(|x| x.ty().name == "motionTrail") {
                        id
                    } else {
                        match g.source(&PlugRef::new(shape, TrailShape::POINTS as i32)) {
                            Some(x) => x.node,
                            None => fail!("motionTrail: {n} is not a motion trail"),
                        }
                    };
                    if a.bool("edit") {
                        for (flag, at) in [("startTime", TRAIL_START), ("endTime", TRAIL_END), ("increment", TRAIL_INCREMENT)] {
                            if let Some(v) = a.get(flag).and_then(Value::as_f64) {
                                g.set_value(PlugRef::new(trail, at as i32), PlugValue::Float(v))?;
                            }
                        }
                        refresh_trail(g, trail)?;
                        return Ok(json!(g.path(trail)));
                    }
                    let d = g.node(trail).unwrap().data();
                    return Ok(if queried(a, &["keyframeTimes"]) {
                        json_of(d, TRAIL_KEYS)
                    } else if queried(a, &["startTime"]) {
                        json!(d.float(TRAIL_START))
                    } else if queried(a, &["endTime"]) {
                        json!(d.float(TRAIL_END))
                    } else if queried(a, &["increment"]) {
                        json!(d.float(TRAIL_INCREMENT))
                    } else {
                        json_of(d, TRAIL_POINTS)
                    });
                }
                let (start, end, by) = range_of(a);
                let names = objects(s, a);
                let Some(first) = names.first() else { fail!("motionTrail: name or select an object") };
                let id = object_id(&s.graph, first, "motionTrail")?;
                create_trail(&mut s.graph, id, a.get("name").and_then(Value::as_str), start, end, by)
            },
        ))
        .positional("the object (the selection when none), or the trail")
        .flag(f("startTime", "st", ArgType::Float, "First frame (the playback start)."))
        .flag(f("endTime", "et", ArgType::Float, "Last frame (the playback end)."))
        .flag(f("increment", "inc", ArgType::Float, "Frames between points (1)."))
        .flag(f("name", "n", ArgType::String, "The trail's name."))
        .flag(f("evaluate", "ev", ArgType::Bool, "With -e: sample again."))
        .flag(f("points", "pts", ArgType::Bool, "Query: the points."))
        .flag(f("keyframeTimes", "kt", ArgType::Bool, "Query: the frames with keys.")),
    );
}
