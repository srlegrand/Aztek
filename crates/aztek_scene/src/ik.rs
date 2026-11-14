//! IK as the reference has it: ikHandle with an ikEffector under the end joint's parent, the RP and SC solvers, the global
//! ikSystem switch. A handle's chain is every joint from the start joint down to, but not including, the end joint.
//!
//! Each handle has an `ikSolve` node that holds a copy of its joints' settings (see `sync_handle`), solves, and drives
//! their rotate through one `outRotate<n>` per joint. The rest pose is the preferred angles. Solving always starts from
//! it, so a pose depends only on the handle's state, never on the pose before.
//!
//! Spline IK lays the chain along a curve, its bones keeping their lengths: `ikHandle -sol ikSplineSolver -c curve1`, or
//! with -ccv a curve made through the joints. The handle's twist runs up the chain and roll turns it all.

use std::any::Any;
use std::sync::Arc;

use aztek_ops::{fail, objects, ArgType, Args, CmdResult, Commands};
use glam::{DMat3, DMat4, DQuat, DVec3};
use serde_json::{json, Value};

use crate::attr_defs::{bool_attr, enum_attr, float_attr_short};
use crate::graph::{DrivenEdit, Graph, SelectMode};
use crate::node::{types, vec3_attr, EvalContext, NodeBehavior, NodeData, NodeType};
use crate::plug::{flags, AttrDef, NodeId, PlugRef, PlugType, PlugValue, NO_NODE};
use crate::rig::{asked, data_attr, f, find, is_joint, json_of, mat3, matrix_in, plug, rot3, rotate_now, set_json, set_vec3, vec3_json, vec3_of, world, Cmd, Joint, JsonData};
use crate::session::Session;
use crate::transform::{euler_near, RotateOrder, Transform, TransformBehavior};

pub const SOLVER_TYPES: [&str; 3] = ["ikRPsolver", "ikSCsolver", "ikSplineSolver"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SolverKind {
    RotationPlane,
    SingleChain,
    Spline,
    /// A compiled plug-in's solver (MPxIkSolverNode): the chain goes to it, it gives back each joint's rotate.
    Plugin,
}

impl SolverKind {
    fn from_index(i: i64) -> Self {
        match i {
            1 => Self::SingleChain,
            2 => Self::Spline,
            PLUGIN_SOLVER => Self::Plugin,
            _ => Self::RotationPlane,
        }
    }
}

/// The handle's solverType for a plug-in's solver, past the reference's three.
const PLUGIN_SOLVER: i64 = 3;

/// Solves for a plug-in's solver type: the solver's type name and the chain as `plugin_input` writes it; each joint's
/// rotate, or None when the solver gave nothing.
pub type PluginSolve = Arc<dyn Fn(&str, &Value) -> Option<Vec<DVec3>> + Send + Sync>;

static PLUGIN_SOLVE: std::sync::RwLock<Option<PluginSolve>> = std::sync::RwLock::new(None);

pub fn set_plugin_solve(f: PluginSolve) {
    *PLUGIN_SOLVE.write().unwrap() = Some(f);
}

// ---------------------------------------------------------------------------------------------------------------------
// Nodes

pub struct IkHandle;

impl IkHandle {
    pub const POLE_VECTOR: usize = Transform::COUNT;
    pub const TWIST: usize = Transform::COUNT + 1;
    pub const ROLL: usize = Transform::COUNT + 2;
    pub const IK_BLEND: usize = Transform::COUNT + 3;
    pub const OFFSET: usize = Transform::COUNT + 4;
    pub const WEIGHT: usize = Transform::COUNT + 5;
    pub const PO_WEIGHT: usize = Transform::COUNT + 6;
    pub const PRIORITY: usize = Transform::COUNT + 7;
    pub const STICKINESS: usize = Transform::COUNT + 8;
    pub const SNAP_ENABLE: usize = Transform::COUNT + 9;
    pub const SOLVER_ENABLE: usize = Transform::COUNT + 10;
    pub const SOLVER_TYPE: usize = Transform::COUNT + 11;
    pub const IK_SOLVER: usize = Transform::COUNT + 12;
    pub const FORCE_SOLVER: usize = Transform::COUNT + 13;
    pub const NUM_SPANS: usize = Transform::COUNT + 14;
    pub const MESSAGE: usize = Transform::COUNT + 15;
}

impl TransformBehavior for IkHandle {}

impl NodeBehavior for IkHandle {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        let local = Transform::local_matrix_of(self, d);
        let parent = d.matrix(Transform::PARENT_MATRIX);
        d.set_output(Transform::MATRIX, local);
        d.set_output(Transform::WORLD_MATRIX, parent * local);
    }
    fn as_transform(&self) -> Option<&dyn TransformBehavior> {
        Some(self)
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

const EFFECTOR_HANDLE: usize = Transform::COUNT;

const IK_HANDLE: usize = 0;
const IK_PARENT: usize = 1;
const IK_CHAIN: usize = 2;
const IK_FK: usize = 3;
const IK_POLE: usize = 4;
const IK_TWIST: usize = 5;
const IK_ROLL: usize = 6;
const IK_BLEND: usize = 7;
const IK_ENABLE: usize = 8;
const IK_GLOBAL: usize = 9;
const IK_TYPE: usize = 10;
const IK_REST_POLE: usize = 11;
const IK_CURVE: usize = 12;
const IK_ROOT: usize = 13;
const IK_REST_TURN: usize = 14;
/// The handle's advanced twist settings, in the order of ADVANCED_TWIST.
const IK_ADVANCED: usize = 15;
const ADVANCED_TWIST: [&str; 12] = ["dTwistControlEnable", "dWorldUpType", "dForwardAxis", "dWorldUpAxis", "dWorldUpVector", "dWorldUpVectorEnd", "dWorldUpMatrix", "dWorldUpMatrixEnd", "dTwistValueType", "dTwistStart", "dTwistEnd", "dTwistRampMult"];
const IK_RAMP: usize = IK_ADVANCED + 12;

const SYSTEM_SOLVE: usize = 0;
const SYSTEM_SNAP: usize = 1;

fn out_name(i: usize) -> String {
    format!("outRotate{i}")
}

/// One chain joint, as its solver sees it.
#[derive(Clone)]
struct ChainJoint {
    t: DVec3,
    jo: DMat3,
    ra: DMat3,
    s: DVec3,
    /// The parent scale this joint undoes (one when segment scale compensate is off).
    is: DVec3,
    pa: DVec3,
    order: RotateOrder,
    min: DVec3,
    max: DVec3,
    min_on: [bool; 3],
    max_on: [bool; 3],
    dof: [bool; 3],
}

impl ChainJoint {
    fn limited(&self) -> bool {
        (0..3).any(|k| self.min_on[k] || self.max_on[k] || !self.dof[k])
    }
    /// Euler angles held inside the joint's limits; an axis it may not rotate about stays at its preferred angle.
    fn clamp(&self, e: DVec3) -> DVec3 {
        let mut out = e;
        for k in 0..3 {
            if !self.dof[k] {
                out[k] = self.pa[k];
                continue;
            }
            if self.min_on[k] {
                out[k] = out[k].max(self.min[k]);
            }
            if self.max_on[k] {
                out[k] = out[k].min(self.max[k]);
            }
        }
        out
    }
    /// The local matrix for rotate `e`.
    fn local(&self, e: DVec3) -> DMat4 {
        self.before_rotate() * DMat4::from_mat3(mat3(e, self.order) * self.ra) * DMat4::from_scale(self.s)
    }
    fn before_rotate(&self) -> DMat4 {
        DMat4::from_translation(self.t) * DMat4::from_scale(self.is) * DMat4::from_mat3(self.jo)
    }
    /// The rotate that gives world rotation `w`, under `prev`, closest to `near`.
    fn rotate_for(&self, prev: &DMat4, w: &DMat3, near: DVec3) -> DVec3 {
        let a = rot3(&(*prev * self.before_rotate()));
        euler_near(&(a.transpose() * *w * self.ra.transpose()), self.order, near)
    }
}

fn v3(j: &Value, k: &str) -> DVec3 {
    let x = j.get(k).and_then(Value::as_array);
    let f = |i: usize| x.and_then(|x| x.get(i)).and_then(Value::as_f64).unwrap_or(0.0);
    DVec3::new(f(0), f(1), f(2))
}

fn b3(j: &Value, k: &str, def: bool) -> [bool; 3] {
    let x = j.get(k).and_then(Value::as_array);
    let f = |i: usize| x.and_then(|x| x.get(i)).and_then(Value::as_bool).unwrap_or(def);
    [f(0), f(1), f(2)]
}

fn chain_joint(j: &Value) -> ChainJoint {
    ChainJoint {
        t: v3(j, "t"),
        jo: mat3(v3(j, "jo"), RotateOrder::Xyz),
        ra: mat3(v3(j, "ra"), RotateOrder::Xyz),
        s: v3(j, "s"),
        is: v3(j, "is"),
        pa: v3(j, "pa"),
        order: RotateOrder::from_index(j.get("ro").and_then(Value::as_i64).unwrap_or(0)),
        min: v3(j, "min"),
        max: v3(j, "max"),
        min_on: b3(j, "minOn", false),
        max_on: b3(j, "maxOn", false),
        dof: b3(j, "dof", true),
    }
}

/// World positions (one per joint, then the end joint) and matrices for rotates `e`.
fn pose(parent: &DMat4, js: &[ChainJoint], end: DVec3, e: &[DVec3]) -> (Vec<DVec3>, Vec<DMat4>) {
    let mut w = *parent;
    let mut points = Vec::with_capacity(js.len() + 1);
    let mut worlds = Vec::with_capacity(js.len());
    for (j, &r) in js.iter().zip(e) {
        w *= j.local(r);
        points.push(w.w_axis.truncate());
        worlds.push(w);
    }
    points.push(w.transform_point3(end));
    (points, worlds)
}

fn any_perpendicular(x: DVec3) -> DVec3 {
    let other = if x.x.abs() < 0.9 { DVec3::X } else { DVec3::Y };
    (other - x * other.dot(x)).normalize_or_zero()
}

fn rotation_about(axis: DVec3, radians: f64) -> DMat3 {
    DMat3::from_quat(DQuat::from_axis_angle(axis, radians))
}

fn arc(a: DVec3, b: DVec3) -> DMat3 {
    let (a, b) = (a.normalize_or_zero(), b.normalize_or_zero());
    if a == DVec3::ZERO || b == DVec3::ZERO {
        return DMat3::IDENTITY;
    }
    DMat3::from_quat(DQuat::from_rotation_arc(a, b))
}

/// The joint the bend shows at: the one furthest from the start to end line, and its offset from that line.
fn bend_offset(points: &[DVec3]) -> DVec3 {
    let (s, e) = (points[0], points[points.len() - 1]);
    let x = (e - s).normalize_or_zero();
    let mut best = DVec3::ZERO;
    for &p in &points[1..points.len() - 1] {
        let v = p - s;
        let perp = v - x * v.dot(x);
        if perp.length() > best.length() {
            best = perp;
        }
    }
    best
}

/// The plane a rest chain bends in, and each inner joint's share of the bend.
struct Bend {
    /// Normal of the plane. Rotating about it by a positive angle turns the start to end line toward the bend side.
    axis: DVec3,
    /// How far the rest pose is bent: 0 is straight.
    rest: f64,
    /// Per inner joint, its share of the bend. Largest is 1 in size.
    shares: Vec<f64>,
}

impl Bend {
    fn of(points: &[DVec3], rest_pole: DVec3) -> Bend {
        let n = points.len() - 1;
        let (s, e) = (points[0], points[n]);
        let length: f64 = points.windows(2).map(|w| (w[1] - w[0]).length()).sum();
        let mut x = (e - s).normalize_or_zero();
        if x == DVec3::ZERO {
            x = (points[1] - s).normalize_or_zero();
        }
        let offset = bend_offset(points);
        let side = if offset.length() > 1e-6 * length.max(1e-12) {
            offset.normalize()
        } else {
            let y = rest_pole - x * rest_pole.dot(x);
            if y.length() > 1e-9 {
                y.normalize()
            } else {
                any_perpendicular(x)
            }
        };
        let axis = x.cross(side).normalize_or_zero();
        let angles: Vec<f64> = (1..n)
            .map(|i| {
                let (a, b) = (points[i] - points[i - 1], points[i + 1] - points[i]);
                axis.dot(a.cross(b)).atan2(a.dot(b))
            })
            .collect();
        let rest = angles.iter().fold(0.0_f64, |m, a| m.max(a.abs()));
        let shares = if rest > 1e-6 { angles.iter().map(|a| a / rest).collect() } else { vec![-1.0; angles.len()] };
        Bend { axis, rest: if rest > 1e-6 { rest } else { 0.0 }, shares }
    }

    /// The turn of each bone (0 is the first) at bend amount `k`, relative to the rest pose.
    fn turns(&self, k: f64, bones: usize) -> Vec<f64> {
        let mut out = vec![0.0; bones];
        let mut sum = 0.0;
        for i in 1..bones {
            sum += (k - self.rest) * self.shares[i - 1];
            out[i] = sum;
        }
        out
    }

    /// Where the chain ends, relative to its start, at bend amount `k`.
    fn end_at(&self, k: f64, points: &[DVec3]) -> DVec3 {
        let turns = self.turns(k, points.len() - 1);
        let mut at = DVec3::ZERO;
        for (i, w) in points.windows(2).enumerate() {
            at += rotation_about(self.axis, turns[i]) * (w[1] - w[0]);
        }
        at
    }

    /// The bend amount that puts the chain's end `distance` from its start, or the nearest it can come. Straight (0) is
    /// the furthest; the amount grows until the first folded pose, so the answer moves smoothly with the distance.
    fn amount_for(&self, distance: f64, points: &[DVec3]) -> f64 {
        const STEPS: usize = 128;
        let reach = |k: f64| self.end_at(k, points).length();
        if reach(0.0) <= distance {
            return 0.0;
        }
        let top = std::f64::consts::PI;
        let (mut prev, mut nearest, mut nearest_at) = (0.0, f64::MAX, 0.0);
        for i in 1..=STEPS {
            let k = top * i as f64 / STEPS as f64;
            let r = reach(k);
            if r < nearest {
                (nearest, nearest_at) = (r, k);
            }
            if r <= distance {
                let (mut lo, mut hi) = (prev, k);
                for _ in 0..60 {
                    let mid = 0.5 * (lo + hi);
                    if reach(mid) > distance {
                        lo = mid;
                    } else {
                        hi = mid;
                    }
                }
                return 0.5 * (lo + hi);
            }
            prev = k;
        }
        nearest_at
    }
}

/// Spline IK's advanced twist: each joint's frame comes from the curve and an up direction.
struct AdvancedTwist {
    up_type: i64,
    up_axis: i64,
    up: DVec3,
    up_end: DVec3,
    matrix: DMat4,
    matrix_end: DMat4,
    value_type: i64,
    twist_start: f64,
    twist_end: f64,
    ramp_mult: f64,
    ramp: Vec<(f64, f64)>,
}

impl AdvancedTwist {
    fn of(d: &NodeData) -> Option<Self> {
        if !d.bool(IK_ADVANCED) {
            return None;
        }
        let keys = json_of(d, IK_RAMP);
        let ramp = keys.as_array().map(|a| a.iter().filter_map(|k| Some((k.get(0)?.as_f64()?, k.get(1)?.as_f64()?))).collect()).unwrap_or_default();
        Some(AdvancedTwist {
            up_type: d.int(IK_ADVANCED + 1),
            up_axis: d.int(IK_ADVANCED + 3),
            up: d.vec3(IK_ADVANCED + 4),
            up_end: d.vec3(IK_ADVANCED + 5),
            matrix: d.matrix(IK_ADVANCED + 6),
            matrix_end: d.matrix(IK_ADVANCED + 7),
            value_type: d.int(IK_ADVANCED + 8),
            twist_start: d.float(IK_ADVANCED + 9),
            twist_end: d.float(IK_ADVANCED + 10),
            ramp_mult: d.float(IK_ADVANCED + 11),
            ramp,
        })
    }

    /// The up direction wanted at this fraction of the chain, at this place.
    fn up_at(&self, s: f64, at: DVec3) -> DVec3 {
        let (a, b) = (self.matrix, self.matrix_end);
        match self.up_type {
            1 => a.w_axis.truncate() - at,
            2 => a.w_axis.truncate().lerp(b.w_axis.truncate(), s) - at,
            3 => a.transform_vector3(self.up),
            4 => a.transform_vector3(self.up).lerp(b.transform_vector3(self.up_end), s),
            5 => self.up,
            6 => self.up.lerp(self.up_end, s),
            _ => DVec3::Y,
        }
    }

    /// Degrees about the chain at this fraction of it.
    fn twist_at(&self, s: f64, total: f64, roll: f64) -> f64 {
        match self.value_type {
            1 => self.twist_start + (self.twist_end - self.twist_start) * s,
            2 => {
                let k = &self.ramp;
                let v = match (k.first(), k.last()) {
                    (Some(first), _) if s <= first.0 => first.1,
                    (_, Some(last)) if s >= last.0 => last.1,
                    _ => k.windows(2).find(|w| s <= w[1].0).map_or(0.0, |w| w[0].1 + (w[1].1 - w[0].1) * (s - w[0].0) / (w[1].0 - w[0].0).max(1e-12)),
                };
                v * self.ramp_mult
            }
            _ => roll + total * s,
        }
    }

    /// The world rotation that lays the joint along `dir` with its up axis toward `up`. `f` is the way the bone leaves the joint in the
    /// joint's own space: a mirrored chain runs down negative X whatever dForwardAxis says, and the reference keeps it so.
    fn frame(&self, f: DVec3, dir: DVec3, up: DVec3) -> DMat3 {
        // Positive Y, negative Y, closest Y, then the same for Z and X.
        let u = [DVec3::Y, -DVec3::Y, DVec3::Y, DVec3::Z, -DVec3::Z, DVec3::Z, DVec3::X, -DVec3::X, DVec3::X][self.up_axis.clamp(0, 8) as usize];
        let f = if f == DVec3::ZERO { DVec3::X } else { f.normalize() };
        let u = if f.dot(u).abs() > 0.9 { f.any_orthonormal_vector() } else { u };
        let s = f.cross(u).normalize();
        let u = s.cross(f);
        let d = dir.normalize_or_zero();
        let mut wu = up - d * up.dot(d);
        if wu.length() < 1e-9 {
            wu = d.any_orthonormal_vector();
        }
        let wu = wu.normalize();
        let ws = d.cross(wu);
        DMat3::from_cols(d, wu, ws) * DMat3::from_cols(f, u, s).transpose()
    }
}

struct SolveInput<'a> {
    parent: DMat4,
    handle: DMat4,
    joints: &'a [ChainJoint],
    end: DVec3,
    pole: DVec3,
    rest_pole: DVec3,
    rest_turn: DMat3,
    advanced: Option<AdvancedTwist>,
    twist: f64,
    roll: f64,
    kind: SolverKind,
    curve: Option<&'a aztek_geo::nurbs::NurbsCurve>,
    root_on_curve: bool,
}

/// The rotation plane and single chain solvers. The chain bends in its rest plane until its end is the right distance
/// from its start, the same share of the bend at each inner joint as at rest, then turns as one about the start:
///
/// - RP: onto the start to handle line, with the bend side toward the pole vector and `twist` degrees about the line.
/// - SC: by the handle's rotation, then the shortest way onto the line. The pole vector and twist have no part in it.
fn solve_plane(input: &SolveInput) -> Vec<DVec3> {
    let js = input.joints;
    let n = js.len();
    let rest_e: Vec<DVec3> = js.iter().map(|j| j.pa).collect();
    if n == 0 {
        return rest_e;
    }
    let (points, worlds) = pose(&input.parent, js, input.end, &rest_e);
    let start = points[0];
    let target = input.handle.w_axis.truncate();
    let to = target - start;
    let bend = Bend::of(&points, input.rest_pole);
    let k = if n > 1 { bend.amount_for(to.length(), &points) } else { bend.rest };
    let reached = bend.end_at(k, &points);
    let x0 = reached.normalize_or_zero();
    let y0 = bend.axis.cross(x0).normalize_or_zero();
    let f0 = DMat3::from_cols(x0, y0, x0.cross(y0));
    let x1 = if to.length() > 1e-9 { to.normalize() } else { x0 };
    let q = match input.kind {
        SolverKind::SingleChain => {
            let h = rot3(&input.handle) * input.rest_turn;
            arc(h * x0, x1) * h
        }
        _ => {
            let mut y = input.pole - x1 * input.pole.dot(x1);
            if y.length() < 1e-9 {
                y = y0 - x1 * y0.dot(x1);
            }
            if y.length() < 1e-9 {
                y = any_perpendicular(x1);
            }
            let y = (rotation_about(x1, input.twist) * y).normalize();
            DMat3::from_cols(x1, y, x1.cross(y)) * f0.transpose()
        }
    };
    let turns = bend.turns(k, n);
    let mut prev = input.parent;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let w = q * rotation_about(bend.axis, turns[i]) * rot3(&worlds[i]);
        let e = js[i].rotate_for(&prev, &w, js[i].pa);
        prev *= js[i].local(e);
        out.push(e);
    }
    out
}

/// Keeps a solved pose inside the joints' limits: the pose is clamped, then cyclic coordinate descent, clamping as it
/// goes, brings the end back toward the handle as far as the limits let it.
fn apply_limits(input: &SolveInput, solved: Vec<DVec3>) -> Vec<DVec3> {
    let js = input.joints;
    if !js.iter().any(ChainJoint::limited) {
        return solved;
    }
    let mut e: Vec<DVec3> = js.iter().zip(&solved).map(|(j, &x)| j.clamp(x)).collect();
    if e.iter().zip(&solved).all(|(a, b)| (*a - *b).length() < 1e-9) {
        return e;
    }
    let target = input.handle.w_axis.truncate();
    let distance = |e: &[DVec3]| (pose(&input.parent, js, input.end, e).0[js.len()] - target).length();
    let mut best = distance(&e);
    for _ in 0..64 {
        for i in (0..js.len()).rev() {
            let (points, worlds) = pose(&input.parent, js, input.end, &e);
            let turn = arc(points[js.len()] - points[i], target - points[i]);
            let prev = if i == 0 { input.parent } else { worlds[i - 1] };
            let w = turn * rot3(&worlds[i]);
            e[i] = js[i].clamp(js[i].rotate_for(&prev, &w, e[i]));
        }
        let d = distance(&e);
        if d < 1e-6 || best - d < 1e-9 {
            break;
        }
        best = d;
    }
    e
}

/// The rotates for a chain: where each solver plugs in.
fn solve(input: &SolveInput) -> Vec<DVec3> {
    match input.kind {
        SolverKind::RotationPlane | SolverKind::SingleChain | SolverKind::Plugin => apply_limits(input, solve_plane(input)),
        SolverKind::Spline => apply_limits(input, solve_spline(input).0),
    }
}

/// Where the next joint goes: the first point along the polyline, from `at` on, a bone's length from `from`. Past the
/// curve's end the chain carries straight on along it.
fn walk_along(line: &[DVec3], at: &mut usize, from: DVec3, len: f64) -> DVec3 {
    let mut i = *at;
    while i + 1 < line.len() {
        let (a, b) = (line[i], line[i + 1]);
        if (b - from).length() >= len {
            // Solve |a + t(b - a) - from| = len.
            let d = b - a;
            let m = a - from;
            let (qa, qb, qc) = (d.dot(d), 2.0 * d.dot(m), m.dot(m) - len * len);
            let root = (qb * qb - 4.0 * qa * qc).max(0.0).sqrt();
            let t = if qa > 1e-18 { ((-qb + root) / (2.0 * qa)).clamp(0.0, 1.0) } else { 0.0 };
            *at = i;
            return a + d * t;
        }
        i += 1;
    }
    *at = line.len().saturating_sub(1);
    let n = line.len();
    let dir = if n >= 2 { (line[n - 1] - line[n - 2]).normalize_or_zero() } else { DVec3::X };
    from + dir * len
}

/// The chain along the curve: rotates per joint, and where the start joint's translate goes with root on curve.
fn solve_spline(input: &SolveInput) -> (Vec<DVec3>, Option<DVec3>) {
    let rest_e: Vec<DVec3> = input.joints.iter().map(|j| j.pa).collect();
    let n = input.joints.len();
    let Some(curve) = input.curve.filter(|c| c.is_valid() && n > 0) else { return (rest_e, None) };
    let (points, worlds) = pose(&input.parent, input.joints, input.end, &rest_e);
    let line = curve.tessellate(32);
    // Root on curve starts at the curve's start; otherwise the chain starts where the root is, at the nearest point.
    let (start, mut at) = if input.root_on_curve {
        (line[0], 0)
    } else {
        let u = curve.closest_param(points[0]);
        let near = curve.eval(u);
        let i = line.iter().enumerate().min_by(|a, b| (*a.1 - near).length_squared().total_cmp(&(*b.1 - near).length_squared())).map_or(0, |x| x.0);
        (points[0], i)
    };
    let mut targets = vec![start];
    for i in 0..n {
        let len = (points[i + 1] - points[i]).length();
        let from = targets[i];
        targets.push(walk_along(&line, &mut at, from, len));
    }
    let mut js: Vec<ChainJoint> = input.joints.to_vec();
    let root = input.root_on_curve.then(|| input.parent.inverse().transform_point3(start));
    if let Some(t) = root {
        js[0].t = t;
    }
    let mut prev = input.parent;
    let mut turn = DQuat::IDENTITY;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let rest_dir = (points[i + 1] - points[i]).normalize_or_zero();
        let want = (targets[i + 1] - targets[i]).normalize_or_zero();
        let have = (turn * rest_dir).normalize_or_zero();
        if have != DVec3::ZERO && want != DVec3::ZERO {
            turn = DQuat::from_rotation_arc(have, want) * turn;
        }
        // Twist grows up the chain; roll turns all of it.
        let t = if n > 1 { i as f64 / (n - 1) as f64 } else { 1.0 };
        let w = match &input.advanced {
            Some(a) => {
                let spin = DQuat::from_axis_angle(want, a.twist_at(t, input.twist, input.roll));
                let f = rot3(&worlds[i]).transpose() * rest_dir;
                DMat3::from_quat(spin) * a.frame(f, want, a.up_at(t, targets[i]))
            }
            None => {
                let spin = DQuat::from_axis_angle(want, input.roll + input.twist * t);
                DMat3::from_quat(spin * turn) * rot3(&worlds[i])
            }
        };
        let e = js[i].rotate_for(&prev, &w, js[i].pa);
        prev *= js[i].local(e);
        out.push(e);
    }
    (out, root)
}

fn blend_rotate(j: &ChainJoint, fk: DVec3, ik: DVec3, t: f64) -> DVec3 {
    let (a, b) = (DQuat::from_mat3(&mat3(fk, j.order)), DQuat::from_mat3(&mat3(ik, j.order)));
    euler_near(&DMat3::from_quat(a.slerp(b, t)), j.order, fk)
}

pub struct IkSolve;

impl NodeBehavior for IkSolve {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        let chain = json_of(d, IK_CHAIN);
        let joints: Vec<ChainJoint> = chain.get("joints").and_then(Value::as_array).map(|a| a.iter().map(chain_joint).collect()).unwrap_or_default();
        let fk = json_of(d, IK_FK);
        let fk_of = |i: usize, j: &ChainJoint| -> DVec3 {
            fk.get(i).and_then(Value::as_array).map(|a| DVec3::new(a[0].as_f64().unwrap_or(0.0), a[1].as_f64().unwrap_or(0.0), a[2].as_f64().unwrap_or(0.0))).unwrap_or(j.pa)
        };
        let weight = if chain.get("valid").and_then(Value::as_bool) == Some(true) && d.bool(IK_ENABLE) && d.bool(IK_GLOBAL) { d.float(IK_BLEND).clamp(0.0, 1.0) } else { 0.0 };
        let curve = match d.value(IK_CURVE) {
            PlugValue::Data(Some(x)) => x.as_any().downcast_ref::<crate::curves::CurveData>().map(|c| c.0.clone()),
            _ => None,
        };
        let root_on_curve = chain.get("rootOnCurve").and_then(Value::as_bool).unwrap_or(false);
        let mut root_at = None;
        let plugin = SolverKind::from_index(d.int(IK_TYPE)) == SolverKind::Plugin;
        let solved = if weight > 0.0 && plugin {
            let hook = PLUGIN_SOLVE.read().unwrap().clone();
            let solver = chain.get("solver").and_then(Value::as_str).unwrap_or("").to_string();
            let mut input = chain.clone();
            let m = |a: usize| json!(d.matrix(a).to_cols_array().to_vec());
            input["handleMatrix"] = m(IK_HANDLE);
            input["parentMatrix"] = m(IK_PARENT);
            input["poleVector"] = vec3_json(d.vec3(IK_POLE));
            input["twist"] = json!(d.float(IK_TWIST));
            input["roll"] = json!(d.float(IK_ROLL));
            hook.and_then(|h| h(&solver, &input)).unwrap_or_default()
        } else if weight > 0.0 {
            let input = SolveInput {
                parent: d.matrix(IK_PARENT),
                handle: d.matrix(IK_HANDLE),
                joints: &joints,
                end: v3(&chain, "end"),
                // Both are in the space of the start joint's parent.
                pole: d.matrix(IK_PARENT).transform_vector3(d.vec3(IK_POLE)),
                rest_pole: d.matrix(IK_PARENT).transform_vector3(d.vec3(IK_REST_POLE)),
                rest_turn: DMat3::from_mat4(d.matrix(IK_REST_TURN)),
                advanced: AdvancedTwist::of(d),
                // Outside spline IK, twist and roll both turn the plane about the handle line.
                twist: if SolverKind::from_index(d.int(IK_TYPE)) == SolverKind::Spline { d.float(IK_TWIST) } else { d.float(IK_TWIST) + d.float(IK_ROLL) },
                roll: d.float(IK_ROLL),
                kind: SolverKind::from_index(d.int(IK_TYPE)),
                curve: curve.as_ref(),
                root_on_curve,
            };
            if input.kind == SolverKind::Spline {
                root_at = solve_spline(&input).1;
            }
            solve(&input)
        } else {
            Vec::new()
        };
        if let Some(t) = root_at {
            d.set_output(IK_ROOT, PlugValue::Vec3(t));
        }
        for (i, j) in joints.iter().enumerate() {
            let Some(attr) = d.ty().attr_index(&out_name(i)) else { continue };
            let e = fk_of(i, j);
            let out = match solved.get(i) {
                Some(&ik) if weight >= 1.0 => ik,
                Some(&ik) => blend_rotate(j, e, ik, weight),
                None => e,
            };
            d.set_output(attr, PlugValue::Vec3(out));
        }
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub(crate) fn register_types() {
    let xf = types::find("transform").expect("transform");
    let store = flags::INPUT | flags::STORABLE;
    let keyable = store | flags::KEYABLE;
    let handle = NodeType {
        name: "ikHandle".into(),
        base: Some(xf.clone()),
        dag: true,
        icon: "target".into(),
        doc: "Where an IK chain's end reaches for. Move it; the joints follow.".into(),
        own_attrs: vec![
            vec3_attr("poleVector", "pv", DVec3::new(0.0, 0.0, -1.0), keyable, "Which way the chain's middle bends (RP solver)."),
            float_attr_short("twist", "twi", 0.0, "Turn of the chain about the handle line, degrees (RP solver).", None, None),
            float_attr_short("roll", "rol", 0.0, "Turn of the chain at its start, about the handle line, degrees (RP solver).", None, None),
            float_attr_short("ikBlend", "ikb", 1.0, "Blend between the joints' own rotates (0) and the solved pose (1).", Some(0.0), Some(1.0)),
            float_attr_short("offset", "ofs", 0.0, "Slide along the curve (spline IK).", None, None),
            float_attr_short("weight", "w", 1.0, "How much this handle counts against others on the same joints.", Some(0.0), None),
            float_attr_short("poWeight", "pow", 1.0, "Position against orientation, 1 is position only.", Some(0.0), Some(1.0)),
            AttrDef { name: "priority".into(), short_name: "pri".into(), ty: PlugType::Int, default_value: PlugValue::Int(1), flags: store, doc: "Solving order among overlapping handles.".into(), ..Default::default() },
            AttrDef { short_name: "stk".into(), ..enum_attr("stickiness", &["off", "sticky"], 0, "Whether the handle holds its place when its parents move.") },
            AttrDef { short_name: "snp".into(), ..bool_attr("snapEnable", true, "The handle follows the end joint while IK is off.") },
            AttrDef { short_name: "hse".into(), ..bool_attr("solverEnable", true, "Whether the solver drives the chain.") },
            AttrDef { short_name: "hst".into(), flags: store | flags::HIDDEN, ..enum_attr("solverType", &SOLVER_TYPES, 0, "The kind of solver.") },
            AttrDef { name: "ikSolver".into(), short_name: "hsv".into(), ty: PlugType::String, default_value: PlugValue::String(SOLVER_TYPES[0].into()), flags: store | flags::HIDDEN, doc: "The solver node's name.".into(), ..Default::default() },
            AttrDef { short_name: "fs".into(), flags: store | flags::HIDDEN, ..bool_attr("forceSolver", false, "Run the solver every time.") },
            AttrDef { name: "numSpans".into(), short_name: "ns".into(), ty: PlugType::Int, default_value: PlugValue::Int(1), flags: store | flags::HIDDEN, doc: "Spans of the created curve (spline IK).".into(), ..Default::default() },
            AttrDef { name: "message".into(), short_name: "msg".into(), ty: PlugType::Int, default_value: PlugValue::Int(0), flags: flags::OUTPUT | flags::HIDDEN | flags::MESSAGE, doc: "Connects the handle to its effector.".into(), ..Default::default() },
            AttrDef { short_name: "dtce".into(), flags: store | flags::HIDDEN, ..bool_attr("dTwistControlEnable", false, "Spline IK: advanced twist on.") },
            AttrDef { short_name: "dwut".into(), flags: store | flags::HIDDEN, ..enum_attr("dWorldUpType", &["Scene Up", "Object Up", "Object Up (Start/End)", "Object Rotation Up", "Object Rotation Up (Start/End)", "Vector", "Vector (Start/End)", "Relative"], 0, "Spline IK: what up means along the chain.") },
            AttrDef { short_name: "dpa".into(), aliases: vec!["dfa".into()], flags: store | flags::HIDDEN, ..enum_attr("dForwardAxis", &["Positive X", "Negative X", "Positive Y", "Negative Y", "Positive Z", "Negative Z"], 0, "Spline IK: the joints' axis along the curve.") },
            AttrDef { short_name: "dwua".into(), flags: store | flags::HIDDEN, ..enum_attr("dWorldUpAxis", &["Positive Y", "Negative Y", "Closest Y", "Positive Z", "Negative Z", "Closest Z", "Positive X", "Negative X", "Closest X"], 0, "Spline IK: the joints' axis towards up.") },
            AttrDef { child_short: ["dwux".into(), "dwuy".into(), "dwuz".into()], ..vec3_attr("dWorldUpVector", "dwuv", DVec3::Y, store | flags::HIDDEN, "Spline IK: up at the start, in the up object's space.") },
            AttrDef { child_short: ["dwvx".into(), "dwvy".into(), "dwvz".into()], ..vec3_attr("dWorldUpVectorEnd", "dwve", DVec3::Y, store | flags::HIDDEN, "Spline IK: up at the end, in the up object's space.") },
            matrix_in("dWorldUpMatrix", "dwum", false, "Spline IK: the up object at the start."),
            matrix_in("dWorldUpMatrixEnd", "dwue", false, "Spline IK: the up object at the end."),
            AttrDef { short_name: "dtvt".into(), flags: store | flags::HIDDEN, ..enum_attr("dTwistValueType", &["Total", "Start/End", "Ramp"], 0, "Spline IK: where the twist comes from.") },
            float_attr_short("dTwistStart", "dtst", 0.0, "Spline IK: twist at the start, degrees.", None, None),
            float_attr_short("dTwistEnd", "dten", 0.0, "Spline IK: twist at the end, degrees.", None, None),
            AttrDef { child_short: ["dtrr".into(), "dtrg".into(), "dtrb".into()], ..vec3_attr("dTwistRamp", "dtra", DVec3::ZERO, store | flags::HIDDEN, "Spline IK: a texture's colour, its red a twist along the chain.") },
            float_attr_short("dTwistRampMult", "dtrm", 90.0, "Spline IK: what the ramp's value is multiplied by.", None, None),
        ],
        create: Some(Arc::new(|| Box::new(IkHandle))),
        ..Default::default()
    };
    types::register_type(handle).expect("ikHandle");
    types::register_type(NodeType {
        name: "ikEffector".into(),
        base: Some(xf),
        dag: true,
        icon: "target".into(),
        doc: "Marks the end of an IK chain, under the end joint's parent at the end joint's place.".into(),
        own_attrs: vec![AttrDef {
            name: "handlePath".into(),
            short_name: "hp".into(),
            ty: PlugType::Int,
            default_value: PlugValue::Int(0),
            flags: flags::INPUT | flags::HIDDEN,
            doc: "The handle this effector ends.".into(),
            ..Default::default()
        }, AttrDef { short_name: "hd".into(), ..bool_attr("hideDisplay", false, "Hide the effector when it is attached to a handle. Held for scripts: the views do not draw effectors.") }],
        create: Some(Arc::new(|| Box::new(Transform))),
        ..Default::default()
    })
    .expect("ikEffector");
    types::register_type(NodeType {
        name: "ikSolve".into(),
        icon: "target".into(),
        doc: "Solves an IK handle's chain.".into(),
        own_attrs: vec![
            matrix_in("handleMatrix", "hm", false, "The handle's world matrix."),
            matrix_in("startParentMatrix", "spm", false, "World matrix of the start joint's parent."),
            data_attr("chain", "ch", "The joints' settings, copied from them as they change."),
            data_attr("fkRotate", "fk", "Each joint's own rotate, used where IK is off."),
            vec3_attr("poleVector", "pv", DVec3::ZERO, flags::INPUT | flags::HIDDEN, "From the handle."),
            AttrDef { flags: flags::INPUT | flags::HIDDEN, ..float_attr_short("twist", "twi", 0.0, "From the handle.", None, None) },
            AttrDef { flags: flags::INPUT | flags::HIDDEN, ..float_attr_short("roll", "rol", 0.0, "From the handle.", None, None) },
            AttrDef { flags: flags::INPUT | flags::HIDDEN, ..float_attr_short("ikBlend", "ikb", 1.0, "From the handle.", None, None) },
            AttrDef { short_name: "hse".into(), flags: flags::INPUT | flags::HIDDEN, ..bool_attr("solverEnable", true, "From the handle.") },
            AttrDef { short_name: "gsl".into(), flags: flags::INPUT | flags::HIDDEN, ..bool_attr("globalSolve", true, "From the ikSystem node.") },
            AttrDef { short_name: "hst".into(), flags: flags::INPUT | flags::HIDDEN, ..enum_attr("solverType", &SOLVER_TYPES, 0, "From the handle.") },
            vec3_attr("restPoleVector", "rpv", DVec3::new(0.0, 0.0, -1.0), flags::INPUT | flags::STORABLE | flags::HIDDEN, "The bend side of a chain straight at rest."),
            AttrDef { name: "inCurve".into(), short_name: "ic".into(), ty: PlugType::Data, default_value: PlugValue::Data(None), flags: flags::INPUT | flags::HIDDEN, doc: "Spline IK: the curve, world space.".into(), ..Default::default() },
            vec3_attr("outRootTranslate", "ort", DVec3::ZERO, flags::OUTPUT | flags::HIDDEN, "Spline IK with root on curve: the start joint's translate."),
            AttrDef { name: "restTurn".into(), short_name: "rtn".into(), ty: PlugType::Matrix, default_value: PlugValue::Matrix(DMat4::IDENTITY), flags: flags::INPUT | flags::STORABLE | flags::HIDDEN, doc: "Single chain: from the handle's rotation to the chain's. A new handle's is the identity.".into(), ..Default::default() },
            AttrDef { short_name: "dtce".into(), flags: flags::INPUT | flags::HIDDEN, ..bool_attr("dTwistControlEnable", false, "From the handle.") },
            AttrDef { name: "dWorldUpType".into(), short_name: "dwut".into(), ty: PlugType::Int, default_value: PlugValue::Int(0), flags: flags::INPUT | flags::HIDDEN, ..Default::default() },
            AttrDef { name: "dForwardAxis".into(), short_name: "dpa".into(), ty: PlugType::Int, default_value: PlugValue::Int(0), flags: flags::INPUT | flags::HIDDEN, ..Default::default() },
            AttrDef { name: "dWorldUpAxis".into(), short_name: "dwua".into(), ty: PlugType::Int, default_value: PlugValue::Int(0), flags: flags::INPUT | flags::HIDDEN, ..Default::default() },
            vec3_attr("dWorldUpVector", "dwuv", DVec3::Y, flags::INPUT | flags::HIDDEN, "From the handle."),
            vec3_attr("dWorldUpVectorEnd", "dwve", DVec3::Y, flags::INPUT | flags::HIDDEN, "From the handle."),
            matrix_in("dWorldUpMatrix", "dwum", false, "From the handle."),
            matrix_in("dWorldUpMatrixEnd", "dwue", false, "From the handle."),
            AttrDef { name: "dTwistValueType".into(), short_name: "dtvt".into(), ty: PlugType::Int, default_value: PlugValue::Int(0), flags: flags::INPUT | flags::HIDDEN, ..Default::default() },
            AttrDef { flags: flags::INPUT | flags::HIDDEN, ..float_attr_short("dTwistStart", "dtst", 0.0, "From the handle.", None, None) },
            AttrDef { flags: flags::INPUT | flags::HIDDEN, ..float_attr_short("dTwistEnd", "dten", 0.0, "From the handle.", None, None) },
            AttrDef { flags: flags::INPUT | flags::HIDDEN, ..float_attr_short("dTwistRampMult", "dtrm", 90.0, "From the handle.", None, None) },
            data_attr("dTwistRampKeys", "dtrk", "The ramp's entries, from the texture on the handle's dTwistRamp."),
        ],
        create: Some(Arc::new(|| Box::new(IkSolve))),
        ..Default::default()
    })
    .expect("ikSolve");
    for name in SOLVER_TYPES {
        types::register_type(NodeType {
            name: name.into(),
            icon: "target".into(),
            doc: "An IK solver handles refer to.".into(),
            own_attrs: vec![
                float_attr_short("tolerance", "tol", 1e-5, "How close counts as reached.", Some(0.0), None),
                AttrDef { name: "maxIterations".into(), short_name: "mxi".into(), ty: PlugType::Int, default_value: PlugValue::Int(2147483647), flags: flags::INPUT | flags::STORABLE, doc: "Most iterations a solve may take.".into(), ..Default::default() },
            ],
            create: Some(Arc::new(|| Box::new(PlainNode))),
            ..Default::default()
        })
        .expect(name);
    }
    types::register_type(NodeType {
        name: "ikSystem".into(),
        icon: "target".into(),
        doc: "Scene wide IK switches.".into(),
        own_attrs: vec![
            AttrDef { name: "globalSolve".into(), short_name: "gsl".into(), ty: PlugType::Bool, default_value: PlugValue::Bool(true), flags: flags::INPUT | flags::STORABLE | flags::OUTPUT, doc: "Whether any IK solves.".into(), ..Default::default() },
            AttrDef { short_name: "gsn".into(), ..bool_attr("globalSnap", true, "Whether handles snap to their effectors while IK is off.") },
        ],
        create: Some(Arc::new(|| Box::new(PlainNode))),
        ..Default::default()
    })
    .expect("ikSystem");
}

struct PlainNode;

impl NodeBehavior for PlainNode {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// The wiring

/// The pole vector a chain's rest pose goes with: where a file says it was, in place of what a new handle works out.
pub(crate) fn set_rest_pole(g: &mut Graph, handle: NodeId, pole: DVec3) {
    if let Some(sv) = solver_of(g, handle) {
        let _ = set_vec3(g, sv, IK_REST_POLE, pole);
    }
}

/// A file holds a single chain handle wherever it was and the start joint's rotate that goes with it: their difference is what the
/// handle's rotation means from then on. Call with everything evaluated.
pub(crate) fn set_rest_turn(g: &mut Graph, handle: NodeId, start_rotate: DVec3) {
    let Some(sv) = solver_of(g, handle) else { return };
    let n = g.node(sv).unwrap().data();
    if SolverKind::from_index(n.int(IK_TYPE)) != SolverKind::SingleChain {
        return;
    }
    let chain = json_of(n, IK_CHAIN);
    let joints: Vec<ChainJoint> = chain.get("joints").and_then(Value::as_array).map(|a| a.iter().map(chain_joint).collect()).unwrap_or_default();
    let Some(j) = joints.first() else { return };
    let parent = n.matrix(IK_PARENT);
    let h = rot3(&n.matrix(IK_HANDLE));
    // The chain's turn from its preferred pose, which is what solve_plane turns.
    let want = rot3(&(parent * j.local(start_rotate))) * rot3(&(parent * j.local(j.pa))).transpose();
    let turn = h.transpose() * want;
    let _ = g.set_value(plug(sv, IK_REST_TURN), PlugValue::Matrix(DMat4::from_mat3(turn)));
}

pub(crate) fn solver_of(g: &Graph, handle: NodeId) -> Option<NodeId> {
    g.destinations(&plug(handle, Transform::WORLD_MATRIX)).iter().map(|d| d.node).find(|&n| g.node(n).is_some_and(|x| x.ty().name == "ikSolve"))
}

pub(crate) fn effector_of(g: &Graph, handle: NodeId) -> Option<NodeId> {
    g.destinations(&plug(handle, IkHandle::MESSAGE)).first().map(|d| d.node)
}

pub(crate) fn end_of(g: &Graph, handle: NodeId) -> Option<NodeId> {
    let eff = effector_of(g, handle)?;
    g.source(&plug(eff, Transform::TRANSLATE)).map(|s| s.node)
}

/// The joints a solver drives, first to last.
pub(crate) fn joints_of(g: &Graph, solver: NodeId) -> Vec<NodeId> {
    let mut out = Vec::new();
    let Some(n) = g.node(solver) else { return out };
    let ty = n.type_arc().clone();
    for i in 0.. {
        let Some(attr) = ty.attr_index(&out_name(i)) else { break };
        match g.destinations(&plug(solver, attr)).first() {
            Some(d) => out.push(d.node),
            None => break,
        }
    }
    out
}

fn all_handles(g: &Graph) -> Vec<NodeId> {
    g.nodes().into_iter().filter(|&id| g.node(id).is_some_and(|n| n.ty().name == "ikHandle")).collect()
}

/// The start joint of the handle, for a pole vector constraint.
pub(crate) fn start_of(g: &Graph, handle: NodeId) -> Option<NodeId> {
    joints_of(g, solver_of(g, handle)?).first().copied()
}

fn joint_json(g: &Graph, j: NodeId) -> Value {
    let n = g.node(j).unwrap().data();
    let three = |base: usize| json!([n.bool(base), n.bool(base + 1), n.bool(base + 2)]);
    let is = if n.bool(Joint::SEGMENT_SCALE_COMPENSATE) {
        let s = n.vec3(Joint::INVERSE_SCALE);
        let inv = |x: f64| if x.abs() > 1e-12 { 1.0 / x } else { 1.0 };
        DVec3::new(inv(s.x), inv(s.y), inv(s.z))
    } else {
        DVec3::ONE
    };
    json!({
        "t": vec3_json(n.vec3(Transform::TRANSLATE)),
        "jo": vec3_json(n.vec3(Joint::JOINT_ORIENT)),
        "ra": vec3_json(n.vec3(Transform::ROTATE_AXIS)),
        "s": vec3_json(n.vec3(Transform::SCALE)),
        "is": vec3_json(is),
        "pa": vec3_json(n.vec3(Joint::PREFERRED_ANGLE)),
        "ro": n.int(Transform::ROTATE_ORDER),
        "min": vec3_json(n.vec3(Joint::MIN_ROT_LIMIT)),
        "max": vec3_json(n.vec3(Joint::MAX_ROT_LIMIT)),
        "minOn": three(Joint::MIN_LIMIT_ENABLE),
        "maxOn": three(Joint::MAX_LIMIT_ENABLE),
        "dof": three(Joint::JOINT_TYPE),
    })
}

/// Copies the chain's settings to the solver when they changed, follows a start joint moved to a new parent, and snaps
/// the handle to the end joint while IK is off. True when anything changed.
pub(crate) fn sync_handle(g: &mut Graph, handle: NodeId, after: bool) -> bool {
    let (Some(solver), Some(end)) = (solver_of(g, handle), end_of(g, handle)) else { return false };
    let joints = joints_of(g, solver);
    let Some(&start) = joints.first() else { return false };
    let mut changed = false;
    let mut valid = g.parent(end) == *joints.last().unwrap();
    for w in joints.windows(2) {
        valid &= g.parent(w[1]) == w[0];
    }
    let end_t = g.node(end).unwrap().data().vec3(Transform::TRANSLATE);
    let mut chain = json!({"valid": valid, "joints": joints.iter().map(|&j| joint_json(g, j)).collect::<Vec<_>>(), "end": vec3_json(end_t)});
    // A plug-in's solver is told which nodes these are, to answer for them while it solves.
    if g.node(handle).is_some_and(|n| n.int(IkHandle::SOLVER_TYPE) == PLUGIN_SOLVER) {
        let named = g.node(handle).map(|n| n.value(IkHandle::IK_SOLVER).as_str().to_string()).unwrap_or_default();
        let ty = g.find(&named).ok().flatten().and_then(|s| g.node(s)).map(|n| n.ty().name.clone()).unwrap_or_default();
        let node = |id: NodeId| json!({"id": id, "path": g.path(id), "full": g.full_path(id), "name": g.node(id).map(|n| n.name.clone()).unwrap_or_default(), "parent": g.parent(id)});
        chain["solver"] = json!(ty);
        chain["nodes"] = json!({
            "joints": joints.iter().map(|&j| node(j)).collect::<Vec<_>>(),
            "end": node(end),
            "handle": node(handle),
            "effector": effector_of(g, handle).map(node).unwrap_or(Value::Null),
        });
    }
    // Set once by the command; everything else here follows the joints.
    if let Some(r) = json_of(g.node(solver).unwrap().data(), IK_CHAIN).get("rootOnCurve").cloned() {
        chain["rootOnCurve"] = r;
    }
    if json_of(g.node(solver).unwrap().data(), IK_CHAIN) != chain {
        g.set_derived(&plug(solver, IK_CHAIN), &PlugValue::Data(Some(Arc::new(JsonData(chain)))));
        changed = true;
    }
    let parent = g.parent(start);
    let input = plug(solver, IK_PARENT);
    let wanted = (parent != NO_NODE).then(|| plug(parent, Transform::WORLD_MATRIX));
    if g.source(&input) != wanted {
        match wanted {
            Some(src) => g.connect_derived(&src, &input),
            None => {
                g.disconnect_derived(&input);
                g.set_derived(&input, &PlugValue::Matrix(DMat4::IDENTITY));
            }
        }
        changed = true;
    }
    // A ramp on the handle: the solver reads its entries, sampling them along the chain.
    if let Some(ramp) = g.node(handle).and_then(|n| n.ty().attr_index("dTwistRamp")).and_then(|a| {
        (0..3).map(|c| PlugRef::component(handle, a as i32, c)).chain([plug(handle, a)]).find_map(|p| g.source(&p))
    }) {
        if let Some(keys) = g.node(ramp.node).and_then(|n| n.ty().attr_index("rampKeys")) {
            let wanted = plug(ramp.node, keys);
            if g.source(&plug(solver, IK_RAMP)) != Some(wanted) {
                g.connect_derived(&wanted, &plug(solver, IK_RAMP));
                changed = true;
            }
        }
    }
    if after {
        changed |= snap_handle(g, handle, end, solver);
    }
    changed
}

/// While IK is off the handle waits at the end joint, so turning IK back on starts from the pose the joints are in.
fn snap_handle(g: &mut Graph, handle: NodeId, end: NodeId, solver: NodeId) -> bool {
    let d = g.node(solver).unwrap().data();
    let off = d.float(IK_BLEND) <= 0.0 || !d.bool(IK_ENABLE);
    if !off || !g.node(handle).unwrap().data().bool(IkHandle::SNAP_ENABLE) {
        return false;
    }
    let translate = plug(handle, Transform::TRANSLATE);
    if g.source(&translate).is_some() || (0..3).any(|c| g.source(&PlugRef::component(handle, Transform::TRANSLATE as i32, c)).is_some()) {
        return false;
    }
    if let Some(sys) = find_system(g) {
        if !g.node(sys).unwrap().data().bool(SYSTEM_SNAP) {
            return false;
        }
    }
    let at = world(g, end).w_axis.truncate();
    let p = g.parent(handle);
    let pw = if p == NO_NODE { DMat4::IDENTITY } else { world(g, p) };
    let local = pw.inverse().transform_point3(at);
    if (local - vec3_of(g, handle, Transform::TRANSLATE)).length() < 1e-9 {
        return false;
    }
    g.set_derived(&translate, &PlugValue::Vec3(local));
    true
}

/// A set on a joint's rotate while a solver drives it: the joint's own rotate, which applies wherever IK is off.
pub(crate) fn set_fk(g: &mut Graph, src: &PlugRef, v: DVec3) -> DrivenEdit {
    let solver = src.node;
    let name = g.attr(&src.whole()).name.clone();
    let Some(i) = name.strip_prefix("outRotate").and_then(|x| x.parse::<usize>().ok()) else { return DrivenEdit::Reject };
    let mut fk = json_of(g.node(solver).unwrap().data(), IK_FK);
    let Some(list) = fk.as_array_mut() else { return DrivenEdit::Reject };
    if i >= list.len() {
        return DrivenEdit::Reject;
    }
    list[i] = vec3_json(v);
    match set_json(g, solver, IK_FK, fk) {
        Ok(()) => DrivenEdit::Handled,
        Err(_) => DrivenEdit::Reject,
    }
}

fn find_system(g: &Graph) -> Option<NodeId> {
    g.nodes().into_iter().find(|&id| g.node(id).is_some_and(|n| n.ty().name == "ikSystem"))
}

pub(crate) fn ensure_system(g: &mut Graph) -> CmdResult<NodeId> {
    if let Some(id) = find_system(g) {
        return Ok(id);
    }
    let t = types::find("ikSystem").unwrap();
    Ok(g.create_node(&t, "ikSystem", NO_NODE, false)?)
}

/// The solver's node of a type, made the first time one is asked for, as the reference makes them.
fn ensure_solver_node(g: &mut Graph, type_name: &str) -> CmdResult<NodeId> {
    if let Some(id) = g.nodes().into_iter().find(|&id| g.node(id).is_some_and(|n| n.ty().name == type_name)) {
        return Ok(id);
    }
    let t = types::find(type_name).unwrap();
    Ok(g.create_node(&t, type_name, NO_NODE, false)?)
}

/// The kind and node for -solver, which takes a solver type or a solver node's name.
fn resolve_solver(g: &mut Graph, name: &str) -> CmdResult<(usize, NodeId)> {
    let by_node = g.find(name).ok().flatten().and_then(|id| {
        let ty = g.node(id)?.ty().name.clone();
        SOLVER_TYPES.iter().position(|t| *t == ty).map(|k| (k, id))
    });
    if let Some(found) = by_node {
        return Ok(found);
    }
    // A plug-in's solver, by its type or its node.
    let plugin_node = g.find(name).ok().flatten().filter(|&id| g.node(id).is_some_and(|n| crate::scripted::is_plugin_ik_solver(&n.ty().name)));
    if let Some(id) = plugin_node {
        return Ok((PLUGIN_SOLVER as usize, id));
    }
    if crate::scripted::is_plugin_ik_solver(name) {
        return Ok((PLUGIN_SOLVER as usize, ensure_solver_node(g, name)?));
    }
    match SOLVER_TYPES.iter().position(|t| *t == name) {
        Some(k) => Ok((k, ensure_solver_node(g, name)?)),
        None => fail!("ikHandle: no solver named {name}"),
    }
}

fn set_solver(g: &mut Graph, handle: NodeId, kind: usize, node: NodeId) -> CmdResult<()> {
    g.set_value(plug(handle, IkHandle::SOLVER_TYPE), PlugValue::Int(kind as i64))?;
    let name = g.path(node);
    g.set_value(plug(handle, IkHandle::IK_SOLVER), PlugValue::String(name))?;
    Ok(())
}

fn chain_between(g: &Graph, start: NodeId, end: NodeId) -> CmdResult<Vec<NodeId>> {
    if start == end {
        fail!("ikHandle: the start and end joints are the same");
    }
    let mut chain = Vec::new();
    let mut j = g.parent(end);
    loop {
        if j == NO_NODE || !is_joint(g, j) {
            fail!("ikHandle: {} is not below {}", g.path(end), g.path(start));
        }
        chain.push(j);
        if j == start {
            break;
        }
        j = g.parent(j);
    }
    chain.reverse();
    Ok(chain)
}

/// Points the solver at a chain: its outputs, each joint's own rotate, and the effector under the end joint's parent.
fn wire_chain(g: &mut Graph, handle: NodeId, solver: NodeId, effector: NodeId, chain: &[NodeId], end: NodeId) -> CmdResult<()> {
    for old in 0.. {
        let ty = g.node(solver).unwrap().type_arc().clone();
        let Some(attr) = ty.attr_index(&out_name(old)) else { break };
        for d in g.destinations(&plug(solver, attr)) {
            g.disconnect(&d);
        }
    }
    let rotates: Vec<DVec3> = chain.iter().map(|&j| rotate_now(g, j)).collect();
    // A chain with no preferred angles keeps the pose it is in as its rest pose, so making a handle doesn't move it.
    if chain.iter().all(|&j| vec3_of(g, j, Joint::PREFERRED_ANGLE) == DVec3::ZERO) {
        for (&j, &r) in chain.iter().zip(&rotates) {
            set_vec3(g, j, Joint::PREFERRED_ANGLE, r)?;
        }
    }
    set_json(g, solver, IK_FK, Value::Array(rotates.iter().map(|&r| vec3_json(r)).collect()))?;
    for (i, &j) in chain.iter().enumerate() {
        let name = out_name(i);
        let ty = g.node(solver).unwrap().type_arc().clone();
        let attr = match ty.attr_index(&name) {
            Some(a) => a,
            None => g.add_attr(solver, vec3_attr(&name, &format!("or{i}"), DVec3::ZERO, flags::HIDDEN | flags::OUTPUT, "A chain joint's rotate."))?,
        };
        g.connect(&plug(solver, attr), &plug(j, Transform::ROTATE), true)?;
    }
    let end_parent = g.parent(end);
    if g.parent(effector) != end_parent {
        g.set_parent(effector, end_parent, false, usize::MAX)?;
    }
    g.connect(&plug(end, Transform::TRANSLATE), &plug(effector, Transform::TRANSLATE), true)?;
    sync_handle(g, handle, false);
    Ok(())
}

fn create_handle(g: &mut Graph, name: &str, start: NodeId, end: NodeId, solver_name: &str) -> CmdResult<(NodeId, NodeId)> {
    let chain = chain_between(g, start, end)?;
    let (kind, solver_node) = resolve_solver(g, solver_name)?;
    g.evaluate();
    let end_pos = world(g, end).w_axis.truncate();
    let ht = types::find("ikHandle").unwrap();
    let h = g.create_node(&ht, name, NO_NODE, true)?;
    set_vec3(g, h, Transform::TRANSLATE, end_pos)?;
    set_solver(g, h, kind, solver_node)?;
    let st = types::find("ikSolve").unwrap();
    let sv = g.create_node(&st, "ikSolve1", NO_NODE, true)?;
    let et = types::find("ikEffector").unwrap();
    let eff = g.create_node(&et, "effector1", g.parent(end), true)?;
    g.connect(&plug(h, IkHandle::MESSAGE), &plug(eff, EFFECTOR_HANDLE), false)?;
    g.connect(&plug(h, Transform::WORLD_MATRIX), &plug(sv, IK_HANDLE), false)?;
    g.connect(&plug(h, IkHandle::POLE_VECTOR), &plug(sv, IK_POLE), false)?;
    g.connect(&plug(h, IkHandle::TWIST), &plug(sv, IK_TWIST), false)?;
    g.connect(&plug(h, IkHandle::ROLL), &plug(sv, IK_ROLL), false)?;
    g.connect(&plug(h, IkHandle::IK_BLEND), &plug(sv, IK_BLEND), false)?;
    g.connect(&plug(h, IkHandle::SOLVER_ENABLE), &plug(sv, IK_ENABLE), false)?;
    g.connect(&plug(h, IkHandle::SOLVER_TYPE), &plug(sv, IK_TYPE), false)?;
    for (k, name) in ADVANCED_TWIST.iter().enumerate() {
        if let Some(a) = g.node(h).and_then(|n| n.ty().attr_index(name)) {
            g.connect(&plug(h, a), &plug(sv, IK_ADVANCED + k), false)?;
        }
    }
    let system = ensure_system(g)?;
    g.connect(&plug(system, SYSTEM_SOLVE), &plug(sv, IK_GLOBAL), false)?;
    wire_chain(g, h, sv, eff, &chain, end)?;
    set_default_pole(g, h, sv)?;
    Ok((h, eff))
}

/// The pole vector a new handle starts with: from the start to end line toward the bend, so twist 0 keeps the pose.
fn set_default_pole(g: &mut Graph, handle: NodeId, solver: NodeId) -> CmdResult<()> {
    let chain = json_of(g.node(solver).unwrap().data(), IK_CHAIN);
    let js: Vec<ChainJoint> = chain.get("joints").and_then(Value::as_array).map(|a| a.iter().map(chain_joint).collect()).unwrap_or_default();
    let start = joints_of(g, solver)[0];
    let p = g.parent(start);
    let parent = if p == NO_NODE { DMat4::IDENTITY } else { world(g, p) };
    let rest: Vec<DVec3> = js.iter().map(|j| j.pa).collect();
    let (points, _) = pose(&parent, &js, v3(&chain, "end"), &rest);
    let mut pole = bend_offset(&points);
    let x = (points[points.len() - 1] - points[0]).normalize_or_zero();
    if pole.length() < 1e-6 {
        pole = DVec3::new(0.0, 0.0, -1.0);
        pole -= x * pole.dot(x);
        if pole.length() < 1e-6 {
            pole = any_perpendicular(x);
        }
    }
    // Held in the space of the start joint's parent.
    let pole = parent.inverse().transform_vector3(pole).normalize();
    set_vec3(g, handle, IkHandle::POLE_VECTOR, pole)?;
    set_vec3(g, solver, IK_REST_POLE, pole)
}

/// After a skeleton edit: chains follow their start and end joints, and a handle whose joints no longer connect goes.
pub(crate) fn refresh_handles(g: &mut Graph) -> CmdResult<()> {
    for h in all_handles(g) {
        let (Some(sv), Some(eff), Some(end)) = (solver_of(g, h), effector_of(g, h), end_of(g, h)) else { continue };
        let now = joints_of(g, sv);
        let Some(&start) = now.first() else { continue };
        match chain_between(g, start, end) {
            Ok(chain) => {
                if chain != now || g.parent(eff) != g.parent(end) {
                    wire_chain(g, h, sv, eff, &chain, end)?;
                }
            }
            Err(_) => delete_handle(g, h, &[])?,
        }
    }
    Ok(())
}

/// A joint about to be removed leaves any chain it is the middle of, and the effector comes off it; `refresh_handles`
/// closes the gap. A start or end joint stays: the handle goes with it.
pub(crate) fn leave_chain(g: &mut Graph, j: NodeId) -> CmdResult<()> {
    for h in all_handles(g) {
        let Some(sv) = solver_of(g, h) else { continue };
        let joints = joints_of(g, sv);
        if joints.first() == Some(&j) || end_of(g, h) == Some(j) || !joints.contains(&j) {
            continue;
        }
        g.disconnect(&plug(j, Transform::ROTATE));
        if let Some(eff) = effector_of(g, h).filter(|&e| g.parent(e) == j) {
            let up = g.parent(j);
            g.set_parent(eff, up, false, usize::MAX)?;
        }
    }
    Ok(())
}

fn delete_handle(g: &mut Graph, h: NodeId, spare: &[NodeId]) -> CmdResult<()> {
    let (sv, eff) = (solver_of(g, h), effector_of(g, h));
    for id in [sv, eff, Some(h)].into_iter().flatten() {
        if !spare.contains(&id) {
            g.delete_node(id)?;
        }
    }
    Ok(())
}

/// A handle, its effector and its solver go together; so does a handle whose start or end joint goes, or a joint in
/// its chain. The joints keep the pose they are in.
pub(crate) fn before_delete(g: &mut Graph, doomed: &[NodeId]) {
    for h in all_handles(g) {
        let (sv, eff, end) = (solver_of(g, h), effector_of(g, h), end_of(g, h));
        let joints = sv.map(|s| joints_of(g, s)).unwrap_or_default();
        let hit = doomed.contains(&h)
            || sv.is_some_and(|x| doomed.contains(&x))
            || eff.is_some_and(|x| doomed.contains(&x))
            || end.is_some_and(|x| doomed.contains(&x))
            || joints.iter().any(|x| doomed.contains(x));
        if hit {
            if let Err(e) = delete_handle(g, h, doomed) {
                log::warn!("{e}");
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Commands

fn named_handle(s: &Session, a: &Args) -> CmdResult<NodeId> {
    let names = objects(s, a);
    let Some(n) = names.last() else { fail!("ikHandle: name or select a handle") };
    let id = find(&s.graph, n, "ikHandle")?;
    if !s.graph.node(id).is_some_and(|x| x.ty().name == "ikHandle") {
        fail!("ikHandle: {n} is not an IK handle");
    }
    Ok(id)
}

fn apply_handle_flags(g: &mut Graph, h: NodeId, a: &Args) -> CmdResult<()> {
    let floats = [("priority", IkHandle::PRIORITY), ("weight", IkHandle::WEIGHT), ("positionWeight", IkHandle::PO_WEIGHT), ("twist", IkHandle::TWIST), ("roll", IkHandle::ROLL), ("offset", IkHandle::OFFSET)];
    for (flag, attr) in floats {
        if let Some(v) = a.get(flag).and_then(Value::as_f64) {
            let value = if attr == IkHandle::PRIORITY { PlugValue::Int(v as i64) } else { PlugValue::Float(v) };
            g.set_value(plug(h, attr), value)?;
        }
    }
    if a.has("snapHandleFlagToggle") {
        g.set_value(plug(h, IkHandle::SNAP_ENABLE), PlugValue::Bool(a.bool("snapHandleFlagToggle")))?;
    }
    if a.has("sticky") {
        let v = a.string("sticky", "off");
        if v != "off" && v != "sticky" {
            fail!("ikHandle: -sticky takes off or sticky");
        }
        g.set_value(plug(h, IkHandle::STICKINESS), PlugValue::Int((v == "sticky") as i64))?;
    }
    if a.has("forceSolver") {
        g.set_value(plug(h, IkHandle::FORCE_SOLVER), PlugValue::Bool(a.bool("forceSolver")))?;
    }
    if a.has("numSpans") {
        g.set_value(plug(h, IkHandle::NUM_SPANS), PlugValue::Int(a.int("numSpans", 1)))?;
    }
    if a.bool("snapHandleToEffector") {
        if let Some(end) = end_of(g, h) {
            let at = world(g, end).w_axis.truncate();
            let p = g.parent(h);
            let pw = if p == NO_NODE { DMat4::IDENTITY } else { world(g, p) };
            set_vec3(g, h, Transform::TRANSLATE, pw.inverse().transform_point3(at))?;
        }
    }
    Ok(())
}

fn handle_query(g: &mut Graph, h: NodeId, a: &Args) -> CmdResult {
    let flag = asked(a).into_iter().next().unwrap_or_else(|| "name".into());
    let n = g.node(h).unwrap().data();
    let solver = solver_of(g, h);
    Ok(match flag.as_str() {
        "name" => json!(g.node(h).unwrap().name()),
        "startJoint" => json!(solver.and_then(|s| joints_of(g, s).first().copied()).map(|j| g.path(j))),
        "endEffector" => json!(effector_of(g, h).map(|e| g.path(e))),
        "curve" => json!(solver_of(g, h).and_then(|sv| g.source(&plug(sv, IK_CURVE))).map(|c| g.path(crate::dag::transform_of(g, c.node)))),
        "jointList" => json!(solver.map(|s| joints_of(g, s)).unwrap_or_default().into_iter().map(|j| g.path(j)).collect::<Vec<_>>()),
        "solver" => json!(n.string(IkHandle::IK_SOLVER)),
        "priority" => json!(n.int(IkHandle::PRIORITY)),
        "weight" => json!(n.float(IkHandle::WEIGHT)),
        "positionWeight" => json!(n.float(IkHandle::PO_WEIGHT)),
        "twist" => json!(n.float(IkHandle::TWIST)),
        "roll" => json!(n.float(IkHandle::ROLL)),
        "offset" => json!(n.float(IkHandle::OFFSET)),
        "snapHandleFlagToggle" => json!(n.bool(IkHandle::SNAP_ENABLE)),
        "sticky" => json!(if n.int(IkHandle::STICKINESS) == 1 { "sticky" } else { "off" }),
        "forceSolver" => json!(n.bool(IkHandle::FORCE_SOLVER)),
        "numSpans" => json!(n.int(IkHandle::NUM_SPANS)),
        "disableHandles" => json!(n.float(IkHandle::IK_BLEND) <= 0.0),
        "enableHandles" => json!(n.float(IkHandle::IK_BLEND) >= 1.0),
        "createCurve" | "parentCurve" | "snapCurve" | "autoPriority" => json!(false),
        other => fail!("ikHandle: can't query -{other}"),
    })
}

fn handle_command(s: &mut Session, a: &Args) -> CmdResult {
    if a.bool("query") {
        if asked(a).iter().any(|x| x == "exists") {
            let names = objects(s, a);
            let g = &s.graph;
            return Ok(json!(names.last().and_then(|n| g.find(n).ok().flatten()).is_some_and(|id| g.node(id).is_some_and(|x| x.ty().name == "ikHandle"))));
        }
        let h = named_handle(s, a)?;
        return handle_query(&mut s.graph, h, a);
    }
    if a.has("exists") {
        let n = a.string("exists", "");
        let g = &s.graph;
        return Ok(json!(g.find(&n)?.is_some_and(|id| g.node(id).is_some_and(|x| x.ty().name == "ikHandle"))));
    }
    if a.bool("edit") || a.bool("disableHandles") || a.bool("enableHandles") {
        if a.bool("disableHandles") || a.bool("enableHandles") {
            let names = objects(s, a);
            let g = &mut s.graph;
            let handles = if names.is_empty() { all_handles(g) } else { names.iter().map(|n| find(g, n, "ikHandle")).collect::<CmdResult<Vec<_>>>()? };
            let v = if a.bool("enableHandles") { 1.0 } else { 0.0 };
            for h in handles {
                g.set_value(plug(h, IkHandle::IK_BLEND), PlugValue::Float(v))?;
            }
            return Ok(Value::Null);
        }
        let h = named_handle(s, a)?;
        let g = &mut s.graph;
        if a.has("solver") {
            let (kind, node) = resolve_solver(g, &a.string("solver", ""))?;
            let to_spline = SOLVER_TYPES[kind] == "ikSplineSolver";
            if to_spline && !a.has("curve") {
                fail!("ikHandle: ikSplineSolver needs a curve: give -c");
            }
            set_solver(g, h, kind, node)?;
            if to_spline {
                setup_spline(g, h, a)?;
            }
        }
        if a.has("startJoint") || a.has("endEffector") {
            let (Some(sv), Some(eff), Some(old_end)) = (solver_of(g, h), effector_of(g, h), end_of(g, h)) else { fail!("ikHandle: the handle has no chain") };
            let old_start = joints_of(g, sv)[0];
            let start = if a.has("startJoint") { find(g, &a.string("startJoint", ""), "ikHandle")? } else { old_start };
            let end = if a.has("endEffector") { find(g, &a.string("endEffector", ""), "ikHandle")? } else { old_end };
            let chain = chain_between(g, start, end)?;
            wire_chain(g, h, sv, eff, &chain, end)?;
        }
        if a.bool("autoPriority") {
            let mut depth = 0;
            if let Some(start) = start_of(g, h) {
                let mut p = g.parent(start);
                while p != NO_NODE {
                    depth += 1;
                    p = g.parent(p);
                }
            }
            g.set_value(plug(h, IkHandle::PRIORITY), PlugValue::Int(depth + 1))?;
        }
        apply_handle_flags(g, h, a)?;
        return Ok(Value::Null);
    }
    let names = objects(s, a);
    let g = &mut s.graph;
    let start = match a.get("startJoint").and_then(Value::as_str) {
        Some(n) => find(g, n, "ikHandle")?,
        None => match names.first() {
            Some(n) => find(g, n, "ikHandle")?,
            None => fail!("ikHandle: -sj and -ee, or select two joints"),
        },
    };
    let end = match a.get("endEffector").and_then(Value::as_str) {
        Some(n) => find(g, n, "ikHandle")?,
        None => match names.get(1) {
            Some(n) => find(g, n, "ikHandle")?,
            None => fail!("ikHandle: -sj and -ee, or select two joints"),
        },
    };
    g.open_chunk("IK Handle");
    let r = (|| -> CmdResult<(NodeId, NodeId, Option<NodeId>)> {
        let solver = a.string("solver", "ikRPsolver");
        let (h, eff) = create_handle(g, &a.string("name", "ikHandle1"), start, end, &solver)?;
        apply_handle_flags(g, h, a)?;
        let spline = solver == "ikSplineSolver" || solver.contains("Spline");
        let curve = if spline { Some(setup_spline(g, h, a)?) } else { None };
        g.select(&[h], SelectMode::Replace);
        Ok((h, eff, curve))
    })();
    g.close_chunk();
    let (h, eff, curve) = r?;
    let mut out = vec![json!(g.path(h)), json!(g.path(eff))];
    if let Some(c) = curve {
        out.push(json!(g.path(c)));
    }
    Ok(Value::Array(out))
}

/// Spline IK's curve: the one given with -c, or (-ccv, the default) a new one through the joints with -ns spans. Returns its
/// transform.
fn setup_spline(g: &mut Graph, h: NodeId, a: &Args) -> CmdResult<NodeId> {
    let Some(sv) = solver_of(g, h) else { fail!("ikHandle: the handle has no chain") };
    let shape = match a.get("curve").and_then(Value::as_str) {
        Some(n) => {
            let id = find(g, n, "ikHandle")?;
            match crate::curves::as_curve(g, id) {
                Some(s) => s,
                None => fail!("ikHandle: {n} is not a curve"),
            }
        }
        None => {
            if a.get("createCurve").is_some_and(|v| v == &json!(false)) {
                fail!("ikHandle: ikSplineSolver needs -c, or -ccv on");
            }
            g.evaluate();
            let mut pts: Vec<DVec3> = joints_of(g, sv).into_iter().map(|j| world(g, j).w_axis.truncate()).collect();
            if let Some(e) = end_of(g, h) {
                pts.push(world(g, e).w_axis.truncate());
            }
            let spans = a.int("numSpans", 4).max(1) as usize;
            let c = aztek_geo::nurbs::NurbsCurve::through(3, &pts).rebuilt(spans, 3);
            let (_, sh) = crate::curves::create_curve_node(g, "curve1", c)?;
            sh
        }
    };
    g.connect(&plug(shape, crate::curves::CurveShape::WORLD_SPACE), &plug(sv, IK_CURVE), false)?;
    let root = a.get("rootOnCurve").and_then(Value::as_bool).unwrap_or(true);
    let mut chain = json_of(g.node(sv).unwrap().data(), IK_CHAIN);
    chain["rootOnCurve"] = json!(root);
    set_json(g, sv, IK_CHAIN, chain)?;
    if root {
        let start = joints_of(g, sv)[0];
        g.connect(&plug(sv, IK_ROOT), &plug(start, Transform::TRANSLATE), true)?;
    }
    Ok(crate::dag::transform_of(g, shape))
}

pub fn register(c: &mut Commands<Session>) {
    c.register(
        Cmd::new(
            "ikHandle",
            "Create an IK handle from -sj to -ee (or two selected joints), or query or edit one. The RP solver bends the chain \
             toward the pole vector; SC aims it. Returns [handle, effector].",
            handle_command,
        )
        .positional("start and end joints (the selection when -sj/-ee are not given), or the handle to query or edit")
        .flag(f("curve", "c", ArgType::String, "Spline IK: the curve to follow. Queried, the curve."))
        .flag(f("createCurve", "ccv", ArgType::Bool, "Spline IK: make a curve through the joints (the default)."))
        .flag(f("parentCurve", "pcv", ArgType::Bool, "Accepted for existing scripts: the curve drives in world space."))
        .flag(f("rootOnCurve", "roc", ArgType::Bool, "Spline IK: the start joint goes to the curve's start (on by default)."))
        .flag(f("simplifyCurve", "scv", ArgType::Bool, "Accepted for existing scripts."))
        .flag(f("rootTwistMode", "rtm", ArgType::Bool, "Accepted for existing scripts."))
        .flag(f("twistType", "tws", ArgType::String, "Accepted for existing scripts (linear)."))
        .flag(f("startJoint", "sj", ArgType::String, "First joint of the chain."))
        .flag(f("endEffector", "ee", ArgType::String, "Last joint of the chain. Queried, the effector."))
        .flag(f("solver", "sol", ArgType::String, "ikRPsolver (default), ikSCsolver or a solver node's name."))
        .flag(f("name", "n", ArgType::String, "Name of the handle."))
        .flag(f("priority", "p", ArgType::Int, "Solving order among overlapping handles."))
        .flag(f("weight", "w", ArgType::Float, "The handle's weight."))
        .flag(f("positionWeight", "pw", ArgType::Float, "Position against orientation."))
        .flag(f("twist", "twi", ArgType::Float, "Turn about the handle line, degrees."))
        .flag(f("roll", "rl", ArgType::Float, "Turn of the chain's start, degrees."))
        .flag(f("offset", "ofs", ArgType::Float, "Slide along the curve (spline IK)."))
        .flag(f("autoPriority", "ap", ArgType::Bool, "Set the priority from the chain's depth in the hierarchy."))
        .flag(f("createCurve", "ccv", ArgType::Bool, "Spline IK: create a curve. Accepted for the solver to come."))
        .flag(f("parentCurve", "pcv", ArgType::Bool, "Spline IK: parent the curve. Accepted for the solver to come."))
        .flag(f("snapCurve", "snc", ArgType::Bool, "Spline IK: snap the curve. Accepted for the solver to come."))
        .flag(f("numSpans", "ns", ArgType::Int, "Spline IK: spans of the curve."))
        .flag(f("disableHandles", "dh", ArgType::Bool, "Set the handles to full FK: ikBlend 0."))
        .flag(f("enableHandles", "eh", ArgType::Bool, "Set the handles to full IK: ikBlend 1."))
        .flag(f("jointList", "jl", ArgType::Bool, "Query: the joints the handle drives."))
        .flag(f("snapHandleFlagToggle", "shf", ArgType::Bool, "The handle follows the end joint while IK is off."))
        .flag(f("snapHandleToEffector", "see", ArgType::Bool, "Put the handle at the end joint now."))
        .flag(f("sticky", "s", ArgType::String, "off or sticky."))
        .flag(f("forceSolver", "fs", ArgType::Bool, "Run the solver every time."))
        .flag(f("exists", "ex", ArgType::String, "Whether a handle of this name exists."))
        .flag(f("edit", "e", ArgType::Bool, "Change a handle."))
        .flag(f("query", "q", ArgType::Bool, "Ask about a handle."))
        .flag(f("queried", "", ArgType::List, "Flags asked about in query mode.")),
    );

    c.register(
        Cmd::new(
            "ikSolver",
            "Create a solver node handles can name with -sol. Returns its name.",
            |s, a| {
                let g = &mut s.graph;
                if a.bool("query") {
                    let Some(n) = a.named().into_iter().next() else { fail!("ikSolver: name a solver") };
                    let id = find(g, &n, "ikSolver")?;
                    let flag = asked(a).into_iter().next().unwrap_or_default();
                    let d = g.node(id).unwrap().data();
                    return Ok(match flag.as_str() {
                        "solverType" => json!(d.ty().name),
                        "epsilon" => json!(d.float(0)),
                        "maxIterations" => json!(d.int(1)),
                        _ => json!(g.node(id).unwrap().name()),
                    });
                }
                if a.bool("edit") {
                    let Some(n) = a.named().into_iter().next() else { fail!("ikSolver: name a solver") };
                    let id = find(g, &n, "ikSolver")?;
                    if let Some(v) = a.get("epsilon").and_then(Value::as_f64) {
                        g.set_value(plug(id, 0), PlugValue::Float(v))?;
                    }
                    if a.has("maxIterations") {
                        g.set_value(plug(id, 1), PlugValue::Int(a.int("maxIterations", 0)))?;
                    }
                    return Ok(Value::Null);
                }
                let ty = a.string("solverType", "ikRPsolver");
                if !SOLVER_TYPES.contains(&ty.as_str()) {
                    fail!("ikSolver: {ty} is not a solver type");
                }
                let t = types::find(&ty).unwrap();
                let name = a.string("name", &ty);
                let id = g.create_node(&t, &name, NO_NODE, true)?;
                if let Some(v) = a.get("epsilon").and_then(Value::as_f64) {
                    g.set_value(plug(id, 0), PlugValue::Float(v))?;
                }
                if a.has("maxIterations") {
                    g.set_value(plug(id, 1), PlugValue::Int(a.int("maxIterations", 0)))?;
                }
                Ok(json!(g.path(id)))
            },
        )
        .positional("solver (edit and query)")
        .flag(f("solverType", "st", ArgType::String, "ikRPsolver, ikSCsolver or ikSplineSolver."))
        .flag(f("name", "n", ArgType::String, "Name of the solver."))
        .flag(f("epsilon", "ep", ArgType::Float, "How close counts as reached."))
        .flag(f("maxIterations", "mxi", ArgType::Int, "Most iterations a solve may take."))
        .flag(f("edit", "e", ArgType::Bool, "Change a solver."))
        .flag(f("query", "q", ArgType::Bool, "Ask about a solver."))
        .flag(f("queried", "", ArgType::List, "Flags asked about in query mode.")),
    );

    c.register(
        Cmd::new(
            "ikSystem",
            "Scene wide IK: -e -sol 0 stops every handle solving, -e -sn 0 stops handles snapping to their effectors.",
            |s, a| {
                let g = &mut s.graph;
                if a.bool("query") {
                    let flag = asked(a).into_iter().next().unwrap_or_else(|| "solve".into());
                    let system = find_system(g);
                    let read = |g: &Graph, attr: usize| system.map(|x| g.node(x).unwrap().data().bool(attr)).unwrap_or(true);
                    return Ok(match flag.as_str() {
                        "solve" => json!(read(g, SYSTEM_SOLVE)),
                        "snap" => json!(read(g, SYSTEM_SNAP)),
                        "list" | "listSolvers" => {
                            let nodes: Vec<String> = g.nodes().into_iter().filter(|&id| g.node(id).is_some_and(|n| SOLVER_TYPES.contains(&n.ty().name.as_str()))).map(|id| g.path(id)).collect();
                            json!(nodes)
                        }
                        other => fail!("ikSystem: can't query -{other}"),
                    });
                }
                let system = ensure_system(g)?;
                if a.has("solve") {
                    g.set_value(plug(system, SYSTEM_SOLVE), PlugValue::Bool(a.bool("solve")))?;
                }
                if a.has("snap") {
                    g.set_value(plug(system, SYSTEM_SNAP), PlugValue::Bool(a.bool("snap")))?;
                }
                Ok(Value::Null)
            },
        )
        .flag(f("solve", "sol", ArgType::Bool, "Whether any IK solves."))
        .flag(f("snap", "sn", ArgType::Bool, "Whether handles snap to their effectors."))
        .flag(f("listSolvers", "ls", ArgType::Bool, "Query: the solver nodes."))
        .flag(f("edit", "e", ArgType::Bool, "Change the settings."))
        .flag(f("query", "q", ArgType::Bool, "Ask about the settings."))
        .flag(f("queried", "", ArgType::List, "Flags asked about in query mode.")),
    );
}
