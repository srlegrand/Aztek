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
    let pts = points_of(&json_of(d, TrailShape::POINTS));
    let mut out: Vec<(DVec3, DVec3)> = pts.windows(2).map(|w| (w[0], w[1])).collect();
    for k in points_of(&json_of(d, TrailShape::KEYS)) {
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
    let now = anim::frame(g);
    let at = |g: &mut Graph, t: f64| {
        let p = world_at(g, target, t).w_axis;
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
        apply_path_flags(g, mp, a)?;
        out.push(g.path(mp));
    }
    Ok(if out.len() == 1 { json!(out[0]) } else { json!(out) })
}

// ---------------------------------------------------------------------------------------------------------------------
// snapshot and flow

fn snapshot(s: &mut Session, a: &Args) -> CmdResult {
    let (start, end, by) = range_of(a);
    let names = objects(s, a);
    if names.is_empty() {
        fail!("snapshot: name or select an object");
    }
    let wanted = a.get("name").and_then(Value::as_str).map(str::to_string);
    if a.bool("motionTrail") {
        let id = object_id(&s.graph, &names[0], "snapshot")?;
        return create_trail(&mut s.graph, id, wanted.as_deref(), start, end, by);
    }
    let ty = types::find("snapshot").unwrap();
    let node = s.graph.create_node(&ty, wanted.as_deref().unwrap_or("snapshot1"), NO_NODE, true)?;
    let base = s.graph.node(node).map(|n| n.name.clone()).unwrap_or_default();
    s.graph.set_value(PlugRef::new(node, 0), PlugValue::Float(start))?;
    s.graph.set_value(PlugRef::new(node, 1), PlugValue::Float(end))?;
    s.graph.set_value(PlugRef::new(node, 2), PlugValue::Float(by))?;
    let update = a.string("update", "always");
    s.graph.set_value(PlugRef::new(node, 3), PlugValue::Int(i64::from(update == "animCurve")))?;
    let tt = types::find("transform").unwrap();
    let group = s.graph.create_node(&tt, &format!("{base}Group"), NO_NODE, true)?;
    let now = anim::frame(&s.graph);
    for name in &names {
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
    Ok(json!([s.graph.path(group), s.graph.path(node)]))
}

fn flow(s: &mut Session, a: &Args) -> CmdResult {
    let names = objects(s, a);
    let Some(name) = names.first().cloned() else { fail!("flow: name or select an object on a motion path") };
    path_of(&s.graph, &name)?;
    if s.commands.find("lattice").is_none() {
        fail!("flow: needs the lattice deformer, which is not in this build");
    }
    let dv = a.get("divisions").cloned().unwrap_or(json!([2, 5, 2]));
    let mut call = serde_json::Map::new();
    call.insert("args".into(), json!([name]));
    call.insert("divisions".into(), dv);
    s.run("lattice", Value::Object(call))
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
                    default_value: PlugValue::Int(0),
                    flags: store,
                    enum_names: WORLD_UP_TYPES.iter().map(|s| s.to_string()).collect(),
                    doc: "What up means: the scene's, towards an object, an object's rotation of worldUpVector, worldUpVector itself, or the curve's normal.".into(),
                    ..Default::default()
                },
                vec3_attr("worldUpVector", "wu", DVec3::Y, store, "The up direction for the vector and object rotation types."),
                hidden_matrix("worldUpMatrix", "wum", "The up object's world matrix."),
                axis("frontAxis", "fa", 0, "The object's axis along the path."),
                axis("upAxis", "ua", 1, "The object's axis towards up."),
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

        types::register_type(NodeType {
            name: "snapshot".into(),
            icon: "chart-line".into(),
            doc: "Records how a snapshot's copies were made.".into(),
            own_attrs: vec![
                float("startTime", "st", 1.0, false, "First frame."),
                float("endTime", "et", 10.0, false, "Last frame."),
                float("increment", "i", 1.0, false, "Frames between copies."),
                AttrDef { name: "update".into(), short_name: "up".into(), ty: PlugType::Enum, default_value: PlugValue::Int(0), flags: store, enum_names: vec!["always".into(), "animCurve".into()], doc: "When copies would update.".into(), ..Default::default() },
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

pub fn register(c: &mut Commands<Session>) {
    register_types();

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
        Cmd::new("flow", "Deform an object on a motion path through a lattice that travels with it. Needs the lattice deformer.", flow)
            .positional("the object (the selection when none)")
            .flag(f("divisions", "dv", ArgType::Any, "Lattice divisions."))
            .flag(f("localCompute", "lc", ArgType::Bool, "Accepted for existing scripts."))
            .flag(f("localDivisions", "ld", ArgType::Any, "Accepted for existing scripts."))
            .flag(f("objectCentered", "oc", ArgType::Bool, "Accepted for existing scripts.")),
    );

    c.register(
        Cmd::new(
            "snapshot",
            "Copies of objects at every -i frames from -st to -et, in a group. Returns the group and the snapshot node. With -mt, makes a motion trail instead.",
            snapshot,
        )
        .positional("objects (the selection when none)")
        .flag(f("startTime", "st", ArgType::Float, "First frame (the playback start)."))
        .flag(f("endTime", "et", ArgType::Float, "Last frame (the playback end)."))
        .flag(f("increment", "i", ArgType::Float, "Frames between copies (1)."))
        .flag(f("constructionHistory", "ch", ArgType::Bool, "Accepted for existing scripts: copies are made once."))
        .flag(f("c", "c", ArgType::Bool, "As -ch."))
        .flag(f("name", "n", ArgType::String, "The snapshot's name."))
        .flag(f("update", "u", ArgType::String, "always or animCurve: recorded on the node."))
        .flag(f("motionTrail", "mt", ArgType::Bool, "Make a motion trail.")),
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
