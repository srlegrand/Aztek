//! The rest of the reference's deformers: sculpt, jiggle, shrinkWrap, morph, wrinkle, proximityWrap, solidify, a point on curve
//! deformer, textureDeformer and curveWarp.
//!
//! Each works in world space through its geomMatrix and honours the weights and envelope like the others.

use std::any::Any;
use std::collections::BTreeMap;
use std::sync::Arc;

use aztek_geo::bvh::MeshBvh;
use aztek_geo::Mesh;
use aztek_ops::{fail, objects, ArgType, Args, CmdResult, Commands, FlagDef};
use glam::{DMat3, DMat4, DQuat, DVec3};
use rayon::prelude::*;
use serde_json::{json, Value};

use crate::attr_defs::{bool_attr, enum_attr, float_attr_short, mesh_attr};
use crate::components::{self, ComponentMode};
use crate::deform::{self, data_attr, f, flat, floats_of, input_mesh, insert_at, int_attr, matrix_in, plug, set_floats, unflat, weight_at, Cmd, Place, BASE_COUNT, ENVELOPE, GEOM, HANDLE, IN, OUT, WEIGHTS};
use crate::graph::{Graph, SelectMode};
use crate::mesh_nodes::MeshNode;
use crate::node::{types, vec3_attr, EvalContext, NodeBehavior, NodeData, NodeType, IN_STORE};
use crate::plug::{flags, AttrDef, NodeId, PlugRef, PlugValue};
use crate::session::Session;
use crate::skin::{asked, modes};
use crate::transform::Transform;

pub const TYPES: &[&str] = &["sculpt", "jiggle", "shrinkWrap", "morph", "wrinkle", "proximityWrap", "solidify", "pointOnCurve", "textureDeformer", "curveWarp"];

fn short(a: AttrDef, s: &str) -> AttrDef {
    AttrDef { short_name: s.into(), ..a }
}

fn fl(name: &str, s: &str, def: f64, doc: &str) -> AttrDef {
    float_attr_short(name, s, def, doc, None, None)
}

/// A vertex normal per point, in object space.
fn point_normals(m: &Mesh) -> Vec<DVec3> {
    let mut out = vec![DVec3::ZERO; m.point_capacity() as usize];
    for face in (0..m.face_capacity()).filter(|&x| m.face_valid(x)) {
        let n = m.face_area_vector(face);
        for p in m.face_points(face) {
            out[p as usize] += n;
        }
    }
    out.iter().map(|n| n.normalize_or_zero()).collect()
}

/// A mesh through a matrix.
fn in_world(m: &Mesh, w: &DMat4) -> Mesh {
    let mut out = m.clone();
    for p in 0..out.point_capacity() {
        if out.point_valid(p) {
            out.set_position(p, w.transform_point3(m.position(p)));
        }
    }
    out
}

fn pass(d: &mut NodeData) -> Option<Arc<Mesh>> {
    let input = d.mesh(IN).cloned();
    if input.is_none() {
        d.set_output(OUT, PlugValue::Mesh(None));
    }
    input
}

/// Moves each point to where f says in world space, by its weight.
fn deform_world(d: &mut NodeData, input: &Mesh, f: impl Fn(u32, DVec3) -> Option<DVec3>) {
    let geom = d.matrix(GEOM);
    let gi = geom.inverse();
    let env = d.float(ENVELOPE);
    let weights = floats_of(d, WEIGHTS);
    let mut m = input.clone();
    for p in 0..m.point_capacity() {
        if !m.point_valid(p) {
            continue;
        }
        let k = weight_at(weights, p, env);
        if k == 0.0 {
            continue;
        }
        let rest = m.position(p);
        if let Some(q) = f(p, geom.transform_point3(rest)) {
            m.set_position(p, rest + (gi.transform_point3(q) - rest) * k);
        }
    }
    d.set_output(OUT, PlugValue::Mesh(Some(Arc::new(m))));
}

// ---------------------------------------------------------------------------------------------------------------------
// sculpt

const SC_MODE: usize = BASE_COUNT;
const SC_INSIDE: usize = BASE_COUNT + 1;
const SC_MAX: usize = BASE_COUNT + 2;
const SC_DROP_TYPE: usize = BASE_COUNT + 3;
const SC_DROP: usize = BASE_COUNT + 4;
const SC_ORIGIN: usize = BASE_COUNT + 5;

struct Sculpt;

impl NodeBehavior for Sculpt {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        let Some(input) = pass(d) else { return };
        let h = d.matrix(HANDLE);
        let hi = h.inverse();
        let radius = h.x_axis.truncate().length().max(1e-9);
        let (mode, inside, max, linear, drop) = (d.int(SC_MODE), d.int(SC_INSIDE), d.float(SC_MAX), d.int(SC_DROP_TYPE) == 1, d.float(SC_DROP));
        let origin = d.matrix(SC_ORIGIN).w_axis.truncate();
        deform_world(d, &input, |_, p| {
            let q = hi.transform_point3(p);
            let r = q.length();
            // How much applies: all inside, falling off past the surface; a ring inside leaves the middle alone.
            let strength = if r <= 1.0 {
                if inside == 0 {
                    r
                } else {
                    1.0
                }
            } else if linear && drop > 0.0 {
                (1.0 - (r - 1.0) * radius / drop).max(0.0)
            } else {
                0.0
            };
            if strength <= 0.0 || r < 1e-9 {
                return None;
            }
            let along = q / r;
            let target = match mode {
                // Flip: from depth inside to as far outside.
                0 if r <= 1.0 => along * (2.0 - r),
                0 => return None,
                // Project: onto the surface.
                1 => along,
                // Stretch: away from the origin, by the most it may move.
                _ => return Some(p + (p - origin).normalize_or_zero() * max * strength),
            };
            let change = h.transform_point3(target) - p;
            Some(p + change.clamp_length_max(max) * strength)
        });
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn sculpt_create(s: &mut Session, a: &Args) -> CmdResult {
    let meshes = deform::targets(s, a)?;
    if meshes.is_empty() {
        fail!("sculpt: select meshes or vertices");
    }
    let g = &mut s.graph;
    g.evaluate();
    let (mut lo, mut hi) = (DVec3::splat(f64::MAX), DVec3::splat(f64::MIN));
    for (m, _) in &meshes {
        let (l, h) = deform::world_bounds(g, *m);
        lo = lo.min(l);
        hi = hi.max(h);
    }
    let centre = (lo + hi) * 0.5;
    let radius = ((hi - lo) * 0.5).max_element().max(1e-3);
    let name = a.string("name", "sculpt1");
    g.open_chunk("Sculpt");
    let r = (|| -> CmdResult<Vec<NodeId>> {
        let n = insert_at(g, meshes[0].0, "sculpt", &name, Place::from_args(a))?;
        let stem = g.node(n).unwrap().name().to_string();
        sculpt_flags(g, n, a)?;
        let tool = match a.get("sculptTool").and_then(Value::as_str) {
            Some(t) => crate::rig::find(g, t, "sculpt")?,
            None => deform::make_handle(g, "sculptor1", "sculpt", centre, DVec3::splat(radius))?,
        };
        g.connect(&plug(tool, Transform::WORLD_MATRIX), &plug(n, HANDLE), false)?;
        let (loc, _) = crate::dag::create_with_shape(g, "locator", &format!("{stem}StretchOrigin"), crate::plug::NO_NODE)?;
        g.set_value(plug(loc, Transform::TRANSLATE), PlugValue::Vec3(centre))?;
        g.connect(&plug(loc, Transform::WORLD_MATRIX), &plug(n, SC_ORIGIN), false)?;
        if let Some(input) = input_mesh(g, n) {
            deform::limit_to(g, n, input.point_capacity(), &meshes[0].1)?;
        }
        for (m, pts) in &meshes[1..] {
            let x = deform::add_geometry(g, n, *m, pts)?;
            g.connect(&plug(tool, Transform::WORLD_MATRIX), &plug(x, HANDLE), true)?;
            g.connect(&plug(loc, Transform::WORLD_MATRIX), &plug(x, SC_ORIGIN), true)?;
        }
        components::set_mode_undoable(g, ComponentMode::Object);
        g.select(&[tool], SelectMode::Replace);
        Ok(vec![n, tool, loc])
    })();
    g.close_chunk();
    let list = r?;
    Ok(json!(list.iter().map(|&x| g.path(x)).collect::<Vec<_>>()))
}

fn sculpt_flags(g: &mut Graph, n: NodeId, a: &Args) -> CmdResult<()> {
    let mode = |v: &str, names: &[&str]| names.iter().position(|x| *x == v).map(|i| i as i64);
    if let Some(m) = a.get("mode").and_then(Value::as_str) {
        let Some(i) = mode(m, &["flip", "project", "stretch"]) else { fail!("sculpt: mode {m} is not flip, project or stretch") };
        g.set_value(plug(n, SC_MODE), PlugValue::Int(i))?;
    }
    if let Some(m) = a.get("insideMode").and_then(Value::as_str) {
        let Some(i) = mode(m, &["ring", "even"]) else { fail!("sculpt: inside mode {m} is not ring or even") };
        g.set_value(plug(n, SC_INSIDE), PlugValue::Int(i))?;
    }
    if let Some(m) = a.get("dropoffType").and_then(Value::as_str) {
        let Some(i) = mode(m, &["none", "linear"]) else { fail!("sculpt: dropoff type {m} is not none or linear") };
        g.set_value(plug(n, SC_DROP_TYPE), PlugValue::Int(i))?;
    }
    for (flag, at) in [("maxDisplacement", SC_MAX), ("dropoffDistance", SC_DROP), ("envelope", ENVELOPE)] {
        if let Some(v) = a.get(flag).and_then(Value::as_f64) {
            g.set_value(plug(n, at), PlugValue::Float(v))?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------------
// jiggle

const JG_ENABLE: usize = BASE_COUNT;
const JG_START: usize = BASE_COUNT + 1;
const JG_STIFFNESS: usize = BASE_COUNT + 2;
const JG_DAMPING: usize = BASE_COUNT + 3;
const JG_WEIGHT: usize = BASE_COUNT + 4;
const JG_MOTION: usize = BASE_COUNT + 5;
const JG_FORCE: usize = BASE_COUNT + 6;

/// What a frame ended with.
#[derive(Clone)]
struct JiggleFrame {
    target: Vec<DVec3>,
    pos: Vec<DVec3>,
    vel: Vec<DVec3>,
}

/// The simulation is stepped a frame at a time, so a run of frames gives the same answer every time. Frames seen are kept:
/// going back shows them again, and moving on steps from the nearest one before.
#[derive(Default)]
struct Jiggle {
    frames: BTreeMap<i64, JiggleFrame>,
    key: [u64; 6],
}

const JIGGLE_KEEP: usize = 2048;

fn jiggle_step(state: &mut JiggleFrame, target: &[DVec3], stiffness: f64, damping: f64, motion: f64, force: DVec3) {
    let moved: Vec<DVec3> = target.iter().zip(&state.target).map(|(t, o)| *o + (*t - *o) * motion).collect();
    for i in 0..target.len().min(state.pos.len()) {
        let v = state.vel[i] * (1.0 - damping) + (moved[i] - state.pos[i]) * stiffness + force;
        state.vel[i] = v;
        state.pos[i] += v;
    }
    state.target = target.to_vec();
}

impl NodeBehavior for Jiggle {
    fn compute(&mut self, ctx: &EvalContext, d: &mut NodeData) {
        let Some(input) = pass(d) else { return };
        let enable = d.int(JG_ENABLE);
        let start = d.float(JG_START).round() as i64;
        let frame = (ctx.time * crate::anim::fps()).round() as i64;
        if enable == 1 || (enable == 2 && frame <= start) {
            self.frames.clear();
            d.set_output(OUT, PlugValue::Mesh(Some(input)));
            return;
        }
        let geom = d.matrix(GEOM);
        let target: Vec<DVec3> = input.positions().iter().map(|p| geom.transform_point3(*p)).collect();
        let (stiffness, damping, motion, force) = (d.float(JG_STIFFNESS), d.float(JG_DAMPING), d.float(JG_MOTION), d.vec3(JG_FORCE));
        let key = [stiffness.to_bits(), damping.to_bits(), motion.to_bits(), force.x.to_bits(), force.y.to_bits(), force.z.to_bits()];
        if key != self.key || self.frames.values().next().is_some_and(|f| f.pos.len() != target.len()) {
            self.frames.clear();
            self.key = key;
        }
        let state = if frame <= start {
            self.frames.clear();
            JiggleFrame { target: target.clone(), pos: target.clone(), vel: vec![DVec3::ZERO; target.len()] }
        } else if let Some(f) = self.frames.get(&frame).filter(|f| f.target == target) {
            f.clone()
        } else {
            // A changed input invalidates this frame and what follows it.
            let _ = self.frames.split_off(&frame);
            let (from, mut s) = match self.frames.iter().next_back() {
                Some((k, f)) => (*k, f.clone()),
                None => (start.min(frame - 1), JiggleFrame { target: target.clone(), pos: target.clone(), vel: vec![DVec3::ZERO; target.len()] }),
            };
            for _ in 0..(frame - from).clamp(0, JIGGLE_KEEP as i64) {
                jiggle_step(&mut s, &target, stiffness, damping, motion, force);
            }
            s
        };
        if self.frames.len() >= JIGGLE_KEEP {
            let first = *self.frames.keys().next().unwrap();
            self.frames.remove(&first);
        }
        self.frames.insert(frame, state.clone());
        let amount = d.float(JG_WEIGHT);
        deform_world(d, &input, |p, at| state.pos.get(p as usize).map(|x| at + (*x - at) * amount));
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// shrinkWrap

const SW_PROJECTION: usize = BASE_COUNT;
const SW_CLOSEST: usize = BASE_COUNT + 1;
const SW_BI: usize = BASE_COUNT + 2;
const SW_INFLATION: usize = BASE_COUNT + 3;
const SW_OFFSET: usize = BASE_COUNT + 4;
const SW_REVERSE: usize = BASE_COUNT + 5;
const SW_AXES: usize = BASE_COUNT + 6;
const SW_TARGET: usize = BASE_COUNT + 9;
const SW_TARGET_MATRIX: usize = BASE_COUNT + 10;

pub const SW_PROJECTIONS: &[&str] = &["Off", "Towards Inner Surface", "Towards Center", "Parallel To Axes", "Vertex Normals", "Closest Point"];

#[derive(Default)]
struct ShrinkWrap {
    /// The target as last built, and the world matrix it was built under.
    built: Option<(usize, DMat4, MeshBvh, DVec3)>,
}

impl NodeBehavior for ShrinkWrap {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        let Some(input) = pass(d) else { return };
        let projection = d.int(SW_PROJECTION);
        let Some(tm) = d.mesh(SW_TARGET).cloned().filter(|_| projection != 0) else {
            d.set_output(OUT, PlugValue::Mesh(Some(input)));
            return;
        };
        let tw = d.matrix(SW_TARGET_MATRIX);
        let ptr = Arc::as_ptr(&tm) as usize;
        if !self.built.as_ref().is_some_and(|b| b.0 == ptr && b.1 == tw) {
            let world_mesh = in_world(&tm, &tw);
            let pts: Vec<DVec3> = (0..world_mesh.point_capacity()).filter(|&p| world_mesh.point_valid(p)).map(|p| world_mesh.position(p)).collect();
            let centre = pts.iter().fold(DVec3::ZERO, |s, p| s + *p) / pts.len().max(1) as f64;
            self.built = Some((ptr, tw, MeshBvh::new(&world_mesh), centre));
        }
        let (bvh, target_centre) = {
            let b = self.built.as_ref().unwrap();
            (&b.2, b.3)
        };
        let geom = d.matrix(GEOM);
        let normals = point_normals(&input);
        let own: Vec<DVec3> = (0..input.point_capacity()).filter(|&p| input.point_valid(p)).map(|p| geom.transform_point3(input.position(p))).collect();
        let centre = own.iter().fold(DVec3::ZERO, |s, p| s + *p) / own.len().max(1) as f64;
        let (closest_fallback, both, inflation, offset, reverse) = (d.bool(SW_CLOSEST), d.bool(SW_BI), d.float(SW_INFLATION), d.float(SW_OFFSET), d.bool(SW_REVERSE));
        let axes = [d.bool(SW_AXES), d.bool(SW_AXES + 1), d.bool(SW_AXES + 2)];
        let sign = if reverse { -1.0 } else { 1.0 };
        let bvh_ref = bvh;
        let hit_of = move |p: u32, at: DVec3| -> Option<(DVec3, DVec3)> {
            let dirs: Vec<DVec3> = match projection {
                1 => vec![(target_centre - at).normalize_or_zero() * sign],
                2 => vec![(centre - at).normalize_or_zero() * sign],
                3 => axes.iter().enumerate().filter(|x| *x.1).flat_map(|(i, _)| [DVec3::AXES[i] * sign, DVec3::AXES[i] * -sign]).collect(),
                4 => vec![geom.transform_vector3(normals[p as usize]).normalize_or_zero() * sign],
                _ => Vec::new(),
            };
            let mut best: Option<(f64, DVec3, DVec3)> = None;
            for dir in dirs {
                let mut consider = |dir: DVec3| {
                    if let Some(h) = bvh_ref.raycast_with(at, dir, f64::MAX, true) {
                        if best.as_ref().is_none_or(|b| h.t < b.0) {
                            best = Some((h.t, h.point, -dir));
                        }
                    }
                };
                consider(dir);
                if both {
                    consider(-dir);
                }
            }
            if let Some((_, q, back)) = best {
                return Some((q, back));
            }
            if projection == 5 || closest_fallback {
                return bvh_ref.closest_point(at).map(|c| (c.point, (at - c.point).normalize_or_zero()));
            }
            None
        };
        deform_world(d, &input, |p, at| {
            let (q, out) = hit_of(p, at)?;
            // The target's surface pushed out by its inflation, and the point held off that by the offset.
            Some(q + out * (inflation + offset))
        });
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// morph

const MO_TARGET: usize = BASE_COUNT;
const MO_MATRIX: usize = BASE_COUNT + 1;
const MO_REST: usize = BASE_COUNT + 2;
const MO_SPACE: usize = BASE_COUNT + 3;
const MO_NORMAL: usize = BASE_COUNT + 4;
const MO_TANGENT: usize = BASE_COUNT + 5;

struct Morph;

impl NodeBehavior for Morph {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        let Some(input) = pass(d) else { return };
        let Some(target) = d.mesh(MO_TARGET).cloned() else {
            d.set_output(OUT, PlugValue::Mesh(Some(input)));
            return;
        };
        let rest = floats_of(d, MO_REST).map(|r| unflat(&r.0)).unwrap_or_default();
        let (tw, geom) = (d.matrix(MO_MATRIX), d.matrix(GEOM));
        let is_world = d.int(MO_SPACE) == 1;
        let (ns, ts) = (d.float(MO_NORMAL), d.float(MO_TANGENT));
        let normals = point_normals(&input);
        deform_world(d, &input, |p, at| {
            if p >= target.point_capacity() || !target.point_valid(p) {
                return None;
            }
            let r = *rest.get(p as usize)?;
            let delta = if is_world { tw.transform_point3(target.position(p)) - geom.transform_point3(r) } else { geom.transform_vector3(target.position(p) - r) };
            let n = geom.transform_vector3(normals[p as usize]).normalize_or_zero();
            let along = n * delta.dot(n);
            Some(at + along * ns + (delta - along) * ts)
        });
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// wrinkle

const WR_COUNT: usize = BASE_COUNT;
const WR_INTENSITY: usize = BASE_COUNT + 1;
const WR_THICKNESS: usize = BASE_COUNT + 2;
const WR_RANDOM: usize = BASE_COUNT + 3;
const WR_STYLE: usize = BASE_COUNT + 4;
const WR_DROPOFF: usize = BASE_COUNT + 5;
const WR_SCALE: usize = BASE_COUNT + 6;
const WR_ALONG_X: usize = BASE_COUNT + 7;

struct Wrinkle;

impl NodeBehavior for Wrinkle {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        let Some(input) = pass(d) else { return };
        let h = d.matrix(HANDLE);
        let (hi, geom) = (h.inverse(), d.matrix(GEOM));
        let (count, intensity, thick, rnd, style, drop, scale, along_x) = (d.int(WR_COUNT).max(1) as f64, d.float(WR_INTENSITY), d.float(WR_THICKNESS).max(1e-3), d.float(WR_RANDOM), d.int(WR_STYLE), d.float(WR_DROPOFF).max(1e-6), d.float(WR_SCALE), d.bool(WR_ALONG_X));
        let normals = point_normals(&input);
        deform_world(d, &input, |p, at| {
            let q = hi.transform_point3(at);
            let r = (q.x * q.x + q.z * q.z).sqrt();
            let phase = match style {
                0 => q.z.atan2(q.x) * count,
                1 => r / drop * count * std::f64::consts::TAU,
                _ => (if along_x { q.x } else { q.z }) / drop * count * std::f64::consts::TAU,
            };
            // The same point always gets the same irregularity.
            let wobble = rnd * ((p as f64 * 12.9898).sin() * 43758.5453).fract();
            let wave = (phase + wobble).cos().abs().powf(1.0 / thick);
            let reach = (1.0 - r / drop).clamp(0.0, 1.0);
            let n = geom.transform_vector3(normals[p as usize]).normalize_or_zero();
            Some(at - n * (1.0 - wave) * intensity * scale * reach)
        });
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn wrinkle_create(s: &mut Session, a: &Args) -> CmdResult {
    let meshes = deform::targets(s, a)?;
    if meshes.is_empty() {
        fail!("wrinkle: select meshes or vertices");
    }
    let g = &mut s.graph;
    g.evaluate();
    let (lo, hi) = deform::world_bounds(g, meshes[0].0);
    let name = a.string("name", "wrinkle1");
    let style = match a.string("style", "radial").as_str() {
        "radial" => 0,
        "tangential" => 1,
        "lines" => 2,
        o => fail!("wrinkle: style {o} is not radial, tangential or lines"),
    };
    g.open_chunk("Wrinkle");
    let r = (|| -> CmdResult<(NodeId, NodeId)> {
        let n = insert_at(g, meshes[0].0, "wrinkle", &name, Place::from_args(a))?;
        let stem = g.node(n).unwrap().name().to_string();
        g.set_value(plug(n, WR_STYLE), PlugValue::Int(style))?;
        g.set_value(plug(n, WR_DROPOFF), PlugValue::Float(a.float("dropoffDistance", ((hi - lo) * 0.5).max_element().max(1e-3))))?;
        apply_settings(g, n, a, WRINKLE)?;
        if a.has("x") {
            g.set_value(plug(n, WR_ALONG_X), PlugValue::Bool(a.bool("x")))?;
        }
        let h = deform::make_handle(g, &format!("{stem}Handle"), "wrinkle", (lo + hi) * 0.5, DVec3::ONE)?;
        g.connect(&plug(h, Transform::WORLD_MATRIX), &plug(n, HANDLE), false)?;
        if let Some(input) = input_mesh(g, n) {
            deform::limit_to(g, n, input.point_capacity(), &meshes[0].1)?;
        }
        components::set_mode_undoable(g, ComponentMode::Object);
        g.select(&[h], SelectMode::Replace);
        Ok((n, h))
    })();
    g.close_chunk();
    let (n, h) = r?;
    Ok(json!([g.path(n), g.path(h)]))
}

// ---------------------------------------------------------------------------------------------------------------------
// proximityWrap

const PW_MODE: usize = BASE_COUNT;
const PW_MAX: usize = BASE_COUNT + 1;
const PW_FALLOFF: usize = BASE_COUNT + 2;
const PW_DROPOFF: usize = BASE_COUNT + 3;
const PW_DRIVER: usize = BASE_COUNT + 4;
const PW_MATRIX: usize = BASE_COUNT + 5;
const PW_BIND: usize = BASE_COUNT + 6;

/// The stored binding, flat: how many drivers; each one's element, point count and bind points in world space; then per point
/// the number of bindings and each as driver, point, weight.
struct PwBind {
    drivers: Vec<(i32, Vec<DVec3>)>,
    points: Vec<Vec<(usize, u32, f64)>>,
}

fn pw_unpack(v: &[f64]) -> PwBind {
    let mut at = 0;
    let mut take = || {
        let x = v.get(at).copied().unwrap_or(0.0);
        at += 1;
        x
    };
    let n = take() as usize;
    let mut drivers = Vec::with_capacity(n);
    for _ in 0..n {
        let (e, c) = (take() as i32, take() as usize);
        let mut pts = Vec::with_capacity(c);
        for _ in 0..c {
            let (x, y, z) = (take(), take(), take());
            pts.push(DVec3::new(x, y, z));
        }
        drivers.push((e, pts));
    }
    let mut points = Vec::new();
    while at < v.len() {
        let k = {
            let x = v[at];
            at += 1;
            x as usize
        };
        let mut one = Vec::with_capacity(k);
        for _ in 0..k {
            one.push((v[at] as usize, v[at + 1] as u32, v[at + 2]));
            at += 3;
        }
        points.push(one);
    }
    PwBind { drivers, points }
}

struct ProximityWrap;

impl NodeBehavior for ProximityWrap {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        let Some(input) = pass(d) else { return };
        let Some(bind) = floats_of(d, PW_BIND).map(|b| pw_unpack(&b.0)) else {
            d.set_output(OUT, PlugValue::Mesh(Some(input)));
            return;
        };
        let mode = d.int(PW_MODE);
        let live: Vec<Option<(Arc<Mesh>, DMat4)>> = bind.drivers.iter().map(|(e, _)| d.element(PW_DRIVER, *e).and_then(|v| v.as_mesh().cloned()).map(|m| (m, d.element(PW_MATRIX, *e).map_or(DMat4::IDENTITY, |x| x.as_matrix())))).collect();
        deform_world(d, &input, |p, at| {
            let b = bind.points.get(p as usize)?;
            if b.is_empty() {
                return None;
            }
            let mut change = DVec3::ZERO;
            let mut snapped = DVec3::ZERO;
            for &(di, id, w) in b {
                let Some(Some((m, wm))) = live.get(di) else { continue };
                if !m.point_valid(id) {
                    continue;
                }
                let now = wm.transform_point3(m.position(id));
                change += (now - bind.drivers[di].1[id as usize]) * w;
                snapped += now * w;
            }
            // Snap goes onto the drivers; the others carry their offset from them along.
            Some(if mode == 2 { snapped } else { at + change })
        });
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn pw_bind(g: &mut Graph, n: NodeId) -> CmdResult<()> {
    let Some(input) = input_mesh(g, n) else { return Ok(()) };
    let node = g.node(n).unwrap();
    let geom = node.data().matrix(GEOM);
    let (max, falloff, rate) = (node.data().int(PW_MAX).max(1) as usize, node.float(PW_FALLOFF).max(1e-6), node.float(PW_DROPOFF).max(1e-6));
    let mut drivers: Vec<(i32, Vec<DVec3>)> = Vec::new();
    for (e, v) in node.elements(PW_DRIVER) {
        let Some(m) = v.as_mesh() else { continue };
        let w = node.element(PW_MATRIX, *e).map_or(DMat4::IDENTITY, |x| x.as_matrix());
        drivers.push((*e, m.positions().iter().map(|p| w.transform_point3(*p)).collect()));
    }
    let flat_candidates: Vec<(usize, u32, DVec3)> = drivers.iter().enumerate().flat_map(|(i, (_, pts))| pts.iter().enumerate().map(move |(k, p)| (i, k as u32, *p))).collect();
    let pts: Vec<DVec3> = input.positions().iter().map(|p| geom.transform_point3(*p)).collect();
    let bound: Vec<Vec<(usize, u32, f64)>> = pts
        .par_iter()
        .map(|at| {
            let mut near: Vec<(f64, usize, u32)> = flat_candidates.iter().map(|(i, k, q)| ((*q - *at).length(), *i, *k)).collect();
            near.sort_by(|a, b| a.0.total_cmp(&b.0));
            near.truncate(max);
            let reach = near.last().map_or(1.0, |x| x.0).max(1e-9) * falloff;
            let mut w: Vec<(usize, u32, f64)> = near.iter().map(|&(x, i, k)| (i, k, (1.0 - (x / (reach * 1.0001)).min(1.0)).powf(rate) + 1e-9)).collect();
            let total: f64 = w.iter().map(|x| x.2).sum();
            for x in w.iter_mut() {
                x.2 /= total.max(1e-12);
            }
            w
        })
        .collect();
    let mut out = vec![drivers.len() as f64];
    for (e, p) in &drivers {
        out.push(*e as f64);
        out.push(p.len() as f64);
        out.extend(flat(p));
    }
    for b in &bound {
        out.push(b.len() as f64);
        for (i, k, w) in b {
            out.extend([*i as f64, *k as f64, *w]);
        }
    }
    set_floats(g, n, PW_BIND, out)
}

fn pw_add_driver(g: &mut Graph, n: NodeId, name: &str) -> CmdResult<()> {
    let id = crate::rig::find(g, name, "proximityWrap")?;
    let Some(m) = components::as_mesh(g, id) else { fail!("proximityWrap: {name} is not a mesh") };
    let e = g.elements(&plug(n, PW_DRIVER)).iter().map(|x| x.element + 1).max().unwrap_or(0);
    g.connect(&plug(m, MeshNode::OUT_MESH), &PlugRef::element(n, PW_DRIVER as i32, e), false)?;
    let xf = crate::dag::transform_of(g, m);
    g.connect(&plug(xf, Transform::WORLD_MATRIX), &PlugRef::element(n, PW_MATRIX as i32, e), false)?;
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------------
// solidify

const SO_SCALE: usize = BASE_COUNT;
const SO_REST: usize = BASE_COUNT + 1;

/// The rotation nearest to a matrix, by repeated averaging with its inverse transpose.
fn nearest_rotation(a: DMat3) -> DMat3 {
    if a.determinant().abs() < 1e-12 {
        return DMat3::IDENTITY;
    }
    let mut r = a;
    for _ in 0..24 {
        let it = r.inverse().transpose();
        let next = DMat3::from_cols((r.x_axis + it.x_axis) * 0.5, (r.y_axis + it.y_axis) * 0.5, (r.z_axis + it.z_axis) * 0.5);
        let done = (next.x_axis - r.x_axis).length() + (next.y_axis - r.y_axis).length() + (next.z_axis - r.z_axis).length() < 1e-10;
        r = next;
        if done {
            break;
        }
    }
    r
}

struct Solidify;

impl NodeBehavior for Solidify {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        let Some(input) = pass(d) else { return };
        let Some(rest) = floats_of(d, SO_REST).map(|r| unflat(&r.0)) else {
            d.set_output(OUT, PlugValue::Mesh(Some(input)));
            return;
        };
        let geom = d.matrix(GEOM);
        let now: Vec<DVec3> = input.positions().iter().map(|p| geom.transform_point3(*p)).collect();
        let valid: Vec<usize> = (0..now.len().min(rest.len())).filter(|&p| input.point_valid(p as u32)).collect();
        if valid.is_empty() {
            d.set_output(OUT, PlugValue::Mesh(Some(input)));
            return;
        }
        let k = valid.len() as f64;
        let cn = valid.iter().fold(DVec3::ZERO, |s, &p| s + now[p]) / k;
        let cr = valid.iter().fold(DVec3::ZERO, |s, &p| s + rest[p]) / k;
        // The best rigid fit of the rest shape to what comes in, with the scale change kept as far as scaleEnvelope says.
        let mut acc = [DVec3::ZERO; 3];
        let (mut sn, mut sr) = (0.0, 0.0);
        for &p in &valid {
            let (q, o) = (now[p] - cn, rest[p] - cr);
            acc[0] += q * o.x;
            acc[1] += q * o.y;
            acc[2] += q * o.z;
            sn += q.length_squared();
            sr += o.length_squared();
        }
        let r = nearest_rotation(DMat3::from_cols(acc[0], acc[1], acc[2]));
        let scale = if sr > 1e-12 { (sn / sr).sqrt() } else { 1.0 };
        let sc = 1.0 + (scale - 1.0) * d.float(SO_SCALE);
        deform_world(d, &input, |p, _| rest.get(p as usize).map(|o| cn + r * (*o - cr) * sc));
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// pointOnCurve

const PC_CURVE: usize = BASE_COUNT;
const PC_REST: usize = BASE_COUNT + 1;
const PC_PARAM: usize = BASE_COUNT + 2;
const PC_PERCENT: usize = BASE_COUNT + 3;

struct PointOnCurve;

impl NodeBehavior for PointOnCurve {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        let Some(input) = pass(d) else { return };
        let (Some(c), Some(r)) = (crate::curves::curve_value(d.value(PC_CURVE)).cloned(), crate::curves::curve_value(d.value(PC_REST)).cloned()) else {
            d.set_output(OUT, PlugValue::Mesh(Some(input)));
            return;
        };
        let at = |k: &aztek_geo::nurbs::NurbsCurve, u: f64, pct: bool| {
            if pct {
                k.eval(k.param_at_fraction(u.clamp(0.0, 1.0)))
            } else {
                k.eval(u)
            }
        };
        let (u, pct) = (d.float(PC_PARAM), d.bool(PC_PERCENT));
        let change = at(&c, u, pct) - at(&r, u, pct);
        deform_world(d, &input, |_, p| Some(p + change));
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// textureDeformer

const TD_STRENGTH: usize = BASE_COUNT;
const TD_OFFSET: usize = BASE_COUNT + 1;
const TD_DIRECTION: usize = BASE_COUNT + 2;
const TD_VECTOR_STRENGTH: usize = BASE_COUNT + 3;
const TD_VECTOR_OFFSET: usize = BASE_COUNT + 4;
const TD_TEXTURE: usize = BASE_COUNT + 5;
const TD_FREQUENCY: usize = BASE_COUNT + 6;
const TD_AMPLITUDE: usize = BASE_COUNT + 7;
const TD_SPACE: usize = BASE_COUNT + 8;

/// A smooth value from -1 to 1 through space: the node's own texture, since nothing is shaded here.
fn procedural(p: DVec3) -> f64 {
    ((p.x * 1.7 + 0.3).sin() * (p.y * 2.3 + 1.1).sin() * (p.z * 1.9 + 2.1).sin() + 0.5 * (p.x * 3.1 + p.y * 4.3 + p.z * 2.7).sin()) / 1.5
}

struct TextureDeformer;

impl NodeBehavior for TextureDeformer {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        let Some(input) = pass(d) else { return };
        let (strength, offset, dir, vs, vo, tex, freq, amp, local) = (d.float(TD_STRENGTH), d.float(TD_OFFSET), d.int(TD_DIRECTION), d.vec3(TD_VECTOR_STRENGTH), d.vec3(TD_VECTOR_OFFSET), d.float(TD_TEXTURE), d.float(TD_FREQUENCY), d.float(TD_AMPLITUDE), d.int(TD_SPACE) == 1);
        let geom = d.matrix(GEOM);
        let normals = point_normals(&input);
        let positions = input.positions().to_vec();
        deform_world(d, &input, |p, at| {
            let sample = if local { positions[p as usize] } else { at };
            let value = tex + procedural(sample * freq) * amp;
            Some(if dir == 1 {
                at + (vs * value + vo)
            } else {
                at + geom.transform_vector3(normals[p as usize]).normalize_or_zero() * (strength * value + offset)
            })
        });
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// curveWarp

const CW_CURVE: usize = BASE_COUNT;
const CW_LENGTH: usize = BASE_COUNT + 1;
const CW_OFFSET: usize = BASE_COUNT + 2;
const CW_TWIST: usize = BASE_COUNT + 3;
const CW_ROLL: usize = BASE_COUNT + 4;
const CW_ALIGN: usize = BASE_COUNT + 5;
const CW_KEEP: usize = BASE_COUNT + 6;

/// A polyline with the running length at each point and the frame each segment carries, turned from the one before.
struct Warp {
    points: Vec<DVec3>,
    at: Vec<f64>,
    frames: Vec<DQuat>,
}

impl Warp {
    fn new(points: Vec<DVec3>, along: DVec3) -> Warp {
        let mut at = vec![0.0];
        for w in points.windows(2) {
            at.push(at.last().unwrap() + (w[1] - w[0]).length());
        }
        let mut frames = Vec::new();
        let mut last = along;
        let mut q = DQuat::IDENTITY;
        for w in points.windows(2) {
            let t = (w[1] - w[0]).normalize_or_zero();
            if t != DVec3::ZERO {
                q = (DQuat::from_rotation_arc(last, t) * q).normalize();
                last = t;
            }
            frames.push(q);
        }
        Warp { points, at, frames }
    }

    fn length(&self) -> f64 {
        self.at.last().copied().unwrap_or(0.0)
    }

    /// The place at a length along, carrying on straight past the ends, and its frame.
    fn sample(&self, s: f64) -> (DVec3, DQuat) {
        let last = self.frames.len().saturating_sub(1);
        let seg = self.at.partition_point(|&x| x <= s).saturating_sub(1).min(last);
        let (a, b) = (self.points[seg], self.points[seg + 1]);
        let span = (self.at[seg + 1] - self.at[seg]).max(1e-12);
        (a + (b - a) * ((s - self.at[seg]) / span), self.frames[seg])
    }
}

struct CurveWarp;

impl NodeBehavior for CurveWarp {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        let Some(input) = pass(d) else { return };
        let Some(curve) = crate::curves::curve_value(d.value(CW_CURVE)).cloned() else {
            d.set_output(OUT, PlugValue::Mesh(Some(input)));
            return;
        };
        let geom = d.matrix(GEOM);
        let pts: Vec<DVec3> = (0..input.point_capacity()).filter(|&p| input.point_valid(p)).map(|p| geom.transform_point3(input.position(p))).collect();
        if pts.is_empty() {
            d.set_output(OUT, PlugValue::Mesh(Some(input)));
            return;
        }
        let (lo, hi) = pts.iter().fold((DVec3::splat(f64::MAX), DVec3::splat(f64::MIN)), |(l, h), p| (l.min(*p), h.max(*p)));
        let size = hi - lo;
        let axis = match d.int(CW_ALIGN) {
            1 => 0,
            2 => 1,
            3 => 2,
            _ => (0..3).max_by(|&a, &b| size[a].total_cmp(&size[b])).unwrap_or(1),
        };
        let line = curve.tessellate(16);
        if line.len() < 2 {
            d.set_output(OUT, PlugValue::Mesh(Some(input)));
            return;
        }
        let warp = Warp::new(line, DVec3::AXES[axis]);
        let extent = size[axis].max(1e-9);
        let run = if d.bool(CW_KEEP) { extent } else { warp.length() };
        let (ls, off, twist, roll) = (d.float(CW_LENGTH), d.float(CW_OFFSET), d.float(CW_TWIST).to_radians(), d.float(CW_ROLL).to_radians());
        let mid = (lo + hi) * 0.5;
        let along = DVec3::AXES[axis];
        deform_world(d, &input, |_, at| {
            let t = (at[axis] - lo[axis]) / extent;
            let (c, q) = warp.sample((t * ls + off) * run);
            let mut side = at - mid;
            side[axis] = 0.0;
            let spin = DQuat::from_axis_angle(along, roll + twist * t);
            Some(c + q * (spin * side))
        });
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Types

pub(crate) fn register_types(gf: Arc<NodeType>) {
    let array = |a: AttrDef| AttrDef { flags: a.flags | flags::ARRAY, ..a };
    let reg = |name: &str, icon: &str, doc: &str, time: bool, own: Vec<AttrDef>, make: crate::node::CreateFn| {
        types::register_type(NodeType { name: name.into(), base: Some(gf.clone()), icon: icon.into(), doc: doc.into(), time_dependent: time, own_attrs: own, create: Some(make), ..Default::default() }).expect(name);
    };
    reg(
        "sculpt",
        "sphere",
        "Pushes points out of a sphere, or onto it.",
        false,
        vec![
            short(enum_attr("mode", &["Flip", "Project", "Stretch"], 2, "Flip through the middle, project onto the sphere, or stretch away from the origin."), "mod"),
            short(enum_attr("insideMode", &["Ring", "Even"], 1, "All the sphere's inside acts evenly, or increasingly toward its surface."), "im"),
            fl("maximumDisplacement", "mxd", 0.1, "The most a point moves."),
            short(enum_attr("dropoffType", &["None", "Linear"], 1, "Whether the effect carries on past the surface."), "dt"),
            fl("dropoffDistance", "dds", 1.0, "How far past the surface it fades out."),
            matrix_in("startPosMatrix", "spm", "The stretch origin's world matrix."),
        ],
        Arc::new(|| Box::new(Sculpt)),
    );
    let mut force = vec3_attr("forceOnAxis", "foa", DVec3::ZERO, IN_STORE, "A steady push on each point per frame, world space.");
    force.flags |= flags::KEYABLE;
    reg(
        "jiggle",
        "wave-sine",
        "Points lag behind and overshoot what comes in, stepped a frame at a time.",
        true,
        vec![
            short(enum_attr("enable", &["Enable", "Disable", "Enable Only After Start Frame"], 0, "Whether it runs."), "ena"),
            fl("startFrame", "sf", 1.0, "Up to this frame nothing moves on its own."),
            float_attr_short("stiffness", "stf", 0.5, "How hard points are pulled back.", Some(0.0), Some(1.0)),
            float_attr_short("damping", "dmp", 0.5, "How fast the motion dies.", Some(0.0), Some(1.0)),
            float_attr_short("jiggleWeight", "jw", 1.0, "How much of the lag shows.", Some(0.0), None),
            fl("motionMultiplier", "mm", 1.0, "How much of what comes in's motion counts."),
            force,
        ],
        Arc::new(|| Box::<Jiggle>::default()),
    );
    let axis = |name: &str, s: &str| short(bool_attr(name, true, "Cast along this axis, with Parallel To Axes."), s);
    reg(
        "shrinkWrap",
        "box-model",
        "Moves points onto another mesh's surface.",
        false,
        vec![
            short(enum_attr("projection", SW_PROJECTIONS, 4, "How a point finds the surface."), "prj"),
            short(bool_attr("closestIfNoIntersection", false, "A point whose ray hits nothing goes to the closest point."), "cin"),
            short(bool_attr("bidirectional", false, "Cast each way, and take the nearer hit."), "bd"),
            fl("targetInflation", "ti", 0.0, "Push the target's surface out by this."),
            fl("offset", "off", 0.0, "Hold points this far off the surface."),
            short(bool_attr("reverse", false, "Cast the opposite way."), "rev"),
            axis("alongX", "ax"),
            axis("alongY", "ay"),
            axis("alongZ", "az"),
            mesh_attr("targetGeom", "tg", flags::INPUT | flags::HIDDEN, "The mesh to shrink onto."),
            matrix_in("targetMatrix", "tgm", "Its world matrix."),
        ],
        Arc::new(|| Box::<ShrinkWrap>::default()),
    );
    reg(
        "morph",
        "shape",
        "Moves points to another mesh's by matching point numbers.",
        false,
        vec![
            mesh_attr("morphTarget", "mt", flags::INPUT | flags::HIDDEN, "The mesh to morph to."),
            matrix_in("morphTargetMatrix", "mtm", "Its world matrix."),
            data_attr("restPoints", "rp", "The points when it was made."),
            short(enum_attr("morphSpace", &["Object", "World"], 0, "Measure the morph in object space or world space."), "ms"),
            fl("normalScale", "nsc", 1.0, "How much of the morph along the normal applies."),
            fl("tangentScale", "tsc", 1.0, "How much of the morph across the surface applies."),
        ],
        Arc::new(|| Box::new(Morph)),
    );
    reg(
        "wrinkle",
        "wave-sine",
        "Creases the surface around its handle.",
        false,
        vec![
            int_attr("wrinkleCount", "wc", 3, "How many creases.", Some(1.0)),
            fl("wrinkleIntensity", "wi", 0.5, "How deep they go."),
            fl("thickness", "th", 1.0, "How wide they are."),
            float_attr_short("randomness", "rnd", 0.2, "How irregular they are.", Some(0.0), Some(1.0)),
            short(enum_attr("style", &["Radial", "Tangential", "Lines"], 0, "Creases out from the handle, around it or in lines."), "st"),
            fl("dropoffDistance", "dds", 1.0, "How far they reach from the handle."),
            fl("scale", "sc", 1.0, "Multiplies the depth."),
            short(bool_attr("alongX", false, "Lines run across X rather than Z."), "x"),
        ],
        Arc::new(|| Box::new(Wrinkle)),
    );
    reg(
        "proximityWrap",
        "box-model",
        "Points follow nearby points of driver meshes, weighted by how close they were at binding.",
        false,
        vec![
            short(enum_attr("wrapMode", &["Offset", "Surface", "Snap"], 0, "Keep the distance from the drivers, or snap onto them."), "wm"),
            int_attr("maxDrivers", "md", 4, "How many driver points each point follows.", Some(1.0)),
            float_attr_short("falloffScale", "fsc", 1.0, "How far each binding reaches.", Some(0.0), None),
            float_attr_short("dropoffRateScale", "drs", 1.0, "How fast a binding's weight falls with distance.", Some(0.0), None),
            array(mesh_attr("driverGeometry", "dg", flags::INPUT | flags::HIDDEN, "The drivers.")),
            AttrDef { flags: flags::INPUT | flags::HIDDEN | flags::ARRAY, ..matrix_in("driverMatrix", "dm", "Their world matrices.") },
            data_attr("proximityBind", "pb", "Each point's bindings and the drivers as they were."),
        ],
        Arc::new(|| Box::new(ProximityWrap)),
    );
    reg(
        "solidify",
        "cube",
        "Keeps a mesh rigid while what comes in moves it: the rest shape fitted to the deformed one.",
        false,
        vec![fl("scaleEnvelope", "se", 1.0, "How much of the size change comes through."), data_attr("restPoints", "rp", "The world points when it was made.")],
        Arc::new(|| Box::new(Solidify)),
    );
    reg(
        "pointOnCurve",
        "chart-line",
        "Points go with one place on a curve, by how far it has moved.",
        false,
        vec![
            data_attr("inputCurve", "ic", "The curve, in world space."),
            data_attr("restCurve", "rc", "The curve when it was made."),
            fl("parameter", "pr", 0.0, "The place on the curve."),
            short(bool_attr("turnOnPercentage", false, "The parameter is a fraction of the length."), "top"),
        ],
        Arc::new(|| Box::new(PointOnCurve)),
    );
    let vs = vec3_attr("vectorStrength", "vs", DVec3::ONE, IN_STORE, "With direction Vector: multiplies the texture on each axis.");
    let vo = vec3_attr("vectorOffset", "vo", DVec3::ZERO, IN_STORE, "With direction Vector: added on each axis.");
    reg(
        "textureDeformer",
        "wave-sine",
        "Displaces points by a texture value: its own procedural one, plus what is connected to .texture.",
        false,
        vec![
            fl("strength", "str", 1.0, "Multiplies the texture."),
            fl("offset", "off", 0.0, "Added to the displacement."),
            short(enum_attr("direction", &["Normal", "Vector"], 0, "Along each point's normal, or by vector."), "dir"),
            vs,
            vo,
            fl("texture", "tx", 0.0, "A value to add to the procedural texture; connect anything to it."),
            fl("noiseFrequency", "nf", 1.0, "How fine the procedural texture is."),
            fl("noiseAmplitude", "na", 1.0, "How strong the procedural texture is."),
            short(enum_attr("pointSpace", &["World", "Local"], 0, "Where the texture is sampled."), "ps"),
        ],
        Arc::new(|| Box::new(TextureDeformer)),
    );
    reg(
        "curveWarp",
        "chart-line",
        "Lays a mesh along a curve.",
        false,
        vec![
            data_attr("inputCurve", "ic", "The curve, in world space."),
            fl("lengthScale", "ls", 1.0, "Stretches the mesh along the curve."),
            fl("offset", "off", 0.0, "Slides the mesh along the curve, as a fraction."),
            fl("twistRotation", "tw", 0.0, "Degrees of twist from one end of the mesh to the other."),
            fl("rollRotation", "rr", 0.0, "Degrees the whole mesh rolls about the curve."),
            short(enum_attr("alignmentMode", &["Auto", "X", "Y", "Z"], 0, "The mesh's axis laid along the curve."), "am"),
            short(bool_attr("keepLength", false, "Keep the mesh's length, not the curve's."), "kl"),
        ],
        Arc::new(|| Box::new(CurveWarp)),
    );
}

// ---------------------------------------------------------------------------------------------------------------------
// Commands

const JIGGLE: &[&str] = &["startFrame", "stiffness", "damping", "jiggleWeight", "motionMultiplier", "forceOnAxis", "envelope"];
const SHRINK: &[&str] = &["closestIfNoIntersection", "bidirectional", "targetInflation", "offset", "reverse", "alongX", "alongY", "alongZ", "envelope"];
const MORPH: &[&str] = &["morphSpace", "normalScale", "tangentScale", "envelope"];
const WRINKLE: &[&str] = &["wrinkleCount", "wrinkleIntensity", "thickness", "randomness", "scale", "envelope"];
const PROXIMITY: &[&str] = &["wrapMode", "maxDrivers", "falloffScale", "dropoffRateScale", "envelope"];
const SOLIDIFY: &[&str] = &["scaleEnvelope", "envelope"];
const POINT_CURVE: &[&str] = &["parameter", "turnOnPercentage", "envelope"];
const TEXTURE: &[&str] = &["strength", "offset", "direction", "vectorStrength", "vectorOffset", "noiseFrequency", "noiseAmplitude", "pointSpace", "envelope"];
const WARP: &[&str] = &["lengthScale", "offset", "twistRotation", "rollRotation", "alignmentMode", "keepLength", "envelope"];

fn apply_settings(g: &mut Graph, n: NodeId, a: &Args, names: &[&str]) -> CmdResult<()> {
    deform::apply_flags(g, n, a, names)
}

/// A deformer on what's named or selected, set up by `then`. Names keep their order; `split` takes those not to deform off the
/// end and hands them on.
fn make_on(s: &mut Session, a: &Args, ty: &str, settings: &[&str], split: impl Fn(&Graph, &[String]) -> usize, then: impl FnOnce(&mut Graph, NodeId, &[String], &[(NodeId, Option<Vec<u32>>)]) -> CmdResult<()>) -> CmdResult {
    let names = objects(s, a);
    let cut = split(&s.graph, &names).min(names.len());
    let (deformed, others) = names.split_at(names.len() - cut);
    let g = &mut s.graph;
    let meshes = if a.named().is_empty() && cut == 0 { Vec::new() } else { deform::named_targets(g, deformed)? };
    let meshes = if meshes.is_empty() && a.named().is_empty() && cut == 0 { deform::targets(s, a)? } else { meshes };
    let g = &mut s.graph;
    if meshes.is_empty() {
        fail!("{ty}: select the meshes to deform");
    }
    let name = a.string("name", &format!("{ty}1"));
    g.evaluate();
    g.open_chunk(ty);
    let r = (|| -> CmdResult<NodeId> {
        let n = insert_at(g, meshes[0].0, ty, &name, Place::from_args(a))?;
        apply_settings(g, n, a, settings)?;
        then(g, n, others, &meshes)?;
        if let Some(input) = input_mesh(g, n) {
            deform::limit_to(g, n, input.point_capacity(), &meshes[0].1)?;
        }
        Ok(n)
    })();
    g.close_chunk();
    let n = r?;
    Ok(json!([g.path(n)]))
}

fn named(g: &Graph, name: &str, ty: &str) -> CmdResult<NodeId> {
    let n = crate::deform_cmds::deformer_named(g, name, ty)?;
    if !g.node(n).is_some_and(|x| x.ty().is_a(ty)) {
        fail!("{ty}: {name} is not a {ty}");
    }
    Ok(deform::leader_of(g, n))
}

/// Edit and query of a deformer named first, by its settings.
fn settings_modes(s: &mut Session, a: &Args, ty: &str, settings: &[&str]) -> Option<CmdResult> {
    if !a.bool("query") && !a.bool("edit") {
        return None;
    }
    Some((|| {
        let Some(name) = a.named().into_iter().next() else { fail!("{ty}: name the deformer") };
        let n = named(&s.graph, &name, ty)?;
        if a.bool("query") {
            return deform::query_flags(&mut s.graph, n, a, settings);
        }
        apply_settings(&mut s.graph, n, a, settings)?;
        Ok(json!([s.graph.path(n)]))
    })())
}

fn setting_flags(c: Cmd, flags: Vec<FlagDef>) -> Cmd {
    flags.into_iter().fold(c.positional("what to deform, and any target last (the selection when none); the deformer with -e and -q"), |c, x| c.flag(x))
}

fn fx(name: &str, short: &str, doc: &str) -> FlagDef {
    f(name, short, ArgType::Float, doc)
}

fn bx(name: &str, short: &str, doc: &str) -> FlagDef {
    f(name, short, ArgType::Bool, doc)
}

fn is_curve_name(g: &Graph, n: &str) -> bool {
    g.find(n).ok().flatten().is_some_and(|id| crate::curves::as_curve(g, id).is_some())
}

fn connect_curve(g: &mut Graph, n: NodeId, at: usize, name: &str) -> CmdResult<()> {
    let id = crate::rig::find(g, name, "deformer")?;
    let Some(c) = crate::curves::as_curve(g, id) else { fail!("deformer: {name} is not a curve") };
    g.connect(&plug(c, crate::curves::CurveShape::WORLD_SPACE), &plug(n, at), true)?;
    Ok(())
}

pub(crate) fn register(c: &mut Commands<Session>) {
    c.register(setting_flags(
        modes(deform::common_flags(Cmd::new(
            "sculpt",
            "Sculpt Deformer: a sphere that pushes points. Returns [sculpt1, sculptor1, sculpt1StretchOrigin]. -mode flip, project or stretch, \
             -mxd the most a point moves, -dds and -dt how it fades past the surface, -st an object to use as the sphere.",
            |s, a| {
                if a.bool("query") {
                    let Some(name) = a.named().into_iter().next() else { fail!("sculpt: name the deformer") };
                    let n = named(&s.graph, &name, "sculpt")?;
                    let g = &mut s.graph;
                    let data = g.node(n).unwrap().data();
                    if asked(a, "mode") {
                        return Ok(json!(["flip", "project", "stretch"][data.int(SC_MODE).clamp(0, 2) as usize]));
                    }
                    if asked(a, "maxDisplacement") {
                        return Ok(json!(data.float(SC_MAX)));
                    }
                    if asked(a, "dropoffDistance") {
                        return Ok(json!(data.float(SC_DROP)));
                    }
                    fail!("sculpt: query -mode, -mxd or -dds");
                }
                if a.bool("edit") {
                    let Some(name) = a.named().into_iter().next() else { fail!("sculpt: name the deformer") };
                    let n = named(&s.graph, &name, "sculpt")?;
                    sculpt_flags(&mut s.graph, n, a)?;
                    return Ok(json!([s.graph.path(n)]));
                }
                sculpt_create(s, a)
            },
        ))),
        vec![
            f("mode", "m", ArgType::String, "flip, project or stretch (the default)."),
            f("insideMode", "im", ArgType::String, "ring or even (the default)."),
            fx("maxDisplacement", "mxd", "The most a point moves (0.1)."),
            f("dropoffType", "dt", ArgType::String, "none or linear (the default)."),
            fx("dropoffDistance", "dds", "How far past the surface it fades (1)."),
            f("sculptTool", "st", ArgType::String, "An object to use as the sphere."),
        ],
    ));

    c.register(setting_flags(
        modes(deform::common_flags(Cmd::new(
            "jiggle",
            "Jiggle: points lag and overshoot, a frame at a time from the start frame. Returns [jiggle1].",
            |s, a| {
                let set_extras = |g: &mut Graph, n: NodeId| -> CmdResult<()> {
                    if let Some(v) = a.get("enable") {
                        let i = v.as_bool().map_or_else(|| v.as_i64().unwrap_or(0), |on| if on { 0 } else { 1 });
                        g.set_value(plug(n, JG_ENABLE), PlugValue::Int(i))?;
                    }
                    if let Some(w) = a.get("weight").and_then(Value::as_f64) {
                        if let Some(input) = input_mesh(g, n) {
                            set_floats(g, n, WEIGHTS, vec![w; input.point_capacity() as usize])?;
                        }
                    }
                    Ok(())
                };
                if a.bool("query") && asked(a, "enable") {
                    let Some(name) = a.named().into_iter().next() else { fail!("jiggle: name the deformer") };
                    let n = named(&s.graph, &name, "jiggle")?;
                    return Ok(json!(s.graph.node(n).unwrap().data().int(JG_ENABLE)));
                }
                if a.bool("edit") {
                    let Some(name) = a.named().into_iter().next() else { fail!("jiggle: name the deformer") };
                    let n = named(&s.graph, &name, "jiggle")?;
                    set_extras(&mut s.graph, n)?;
                }
                if let Some(r) = settings_modes(s, a, "jiggle", JIGGLE) {
                    return r;
                }
                make_on(s, a, "jiggle", JIGGLE, |_, _| 0, |g, n, _, _| set_extras(g, n))
            },
        ))),
        vec![
            f("enable", "ena", ArgType::Any, "0 enable (true), 1 disable (false), 2 only after the start frame."),
            fx("startFrame", "sf", "Up to this frame nothing moves on its own (1)."),
            fx("stiffness", "stf", "How hard points are pulled back (0.5)."),
            fx("damping", "dmp", "How fast the motion dies (0.5)."),
            fx("weight", "wt", "Sets every point's weight."),
            fx("jiggleWeight", "jw", "How much of the lag shows (1)."),
            fx("motionMultiplier", "mm", "How much of the motion counts (1)."),
            f("forceOnAxis", "foa", ArgType::Vec3, "A steady push per frame, world space."),
        ],
    ));

    c.register(setting_flags(
        modes(deform::common_flags(Cmd::new(
            "shrinkWrap",
            "Shrink Wrap: moves points onto a target mesh, named by -t or last. Returns [shrinkWrap1].",
            |s, a| {
                let projection = |g: &mut Graph, n: NodeId| -> CmdResult<()> {
                    if let Some(v) = a.get("projection") {
                        let i = v.as_i64().or_else(|| v.as_str().and_then(|t| SW_PROJECTIONS.iter().position(|x| x.eq_ignore_ascii_case(t)).map(|i| i as i64)));
                        let Some(i) = i else { fail!("shrinkWrap: unknown projection {v}") };
                        g.set_value(plug(n, SW_PROJECTION), PlugValue::Int(i))?;
                    }
                    Ok(())
                };
                if a.bool("query") && asked(a, "projection") {
                    let Some(name) = a.named().into_iter().next() else { fail!("shrinkWrap: name the deformer") };
                    let n = named(&s.graph, &name, "shrinkWrap")?;
                    return Ok(json!(s.graph.node(n).unwrap().data().int(SW_PROJECTION)));
                }
                if a.bool("edit") {
                    let Some(name) = a.named().into_iter().next() else { fail!("shrinkWrap: name the deformer") };
                    let n = named(&s.graph, &name, "shrinkWrap")?;
                    projection(&mut s.graph, n)?;
                }
                if let Some(r) = settings_modes(s, a, "shrinkWrap", SHRINK) {
                    return r;
                }
                let flag = a.get("target").and_then(Value::as_str).map(str::to_string);
                let cut = if flag.is_some() { 0 } else { 1 };
                make_on(s, a, "shrinkWrap", SHRINK, move |_, _| cut, |g, n, rest, _| {
                    projection(g, n)?;
                    let Some(t) = flag.as_deref().or(rest.first().map(String::as_str)) else { fail!("shrinkWrap: name the target") };
                    let id = crate::rig::find(g, t, "shrinkWrap")?;
                    let Some(m) = components::as_mesh(g, id) else { fail!("shrinkWrap: {t} is not a mesh") };
                    g.connect(&plug(m, MeshNode::OUT_MESH), &plug(n, SW_TARGET), false)?;
                    let xf = crate::dag::transform_of(g, m);
                    g.connect(&plug(xf, Transform::WORLD_MATRIX), &plug(n, SW_TARGET_MATRIX), false)?;
                    Ok(())
                })
            },
        ))),
        vec![
            f("target", "t", ArgType::String, "The mesh to shrink onto."),
            f("projection", "prj", ArgType::Any, "0 off, 1 towards inner surface, 2 towards center, 3 parallel to axes, 4 vertex normals (the default), 5 closest point."),
            bx("closestIfNoIntersection", "cin", "A point whose ray hits nothing goes to the closest point."),
            bx("bidirectional", "bd", "Cast each way, taking the nearer hit."),
            fx("targetInflation", "ti", "Push the target's surface out by this."),
            fx("offset", "off", "Hold points this far off the surface."),
            bx("reverse", "rev", "Cast the opposite way."),
            bx("alongX", "ax", "Parallel to axes: along X (on)."),
            bx("alongY", "ay", "Parallel to axes: along Y (on)."),
            bx("alongZ", "az", "Parallel to axes: along Z (on)."),
        ],
    ));

    c.register(setting_flags(
        modes(deform::common_flags(Cmd::new(
            "morph",
            "Morph: points move to the matching points of -morphTarget. Returns [morph1].",
            |s, a| {
                if let Some(r) = settings_modes(s, a, "morph", MORPH) {
                    return r;
                }
                let flag = a.get("morphTarget").and_then(Value::as_str).map(str::to_string);
                let Some(flag) = flag else { fail!("morph: give -morphTarget") };
                make_on(s, a, "morph", MORPH, |_, _| 0, |g, n, _, _| {
                    let id = crate::rig::find(g, &flag, "morph")?;
                    let Some(m) = components::as_mesh(g, id) else { fail!("morph: {flag} is not a mesh") };
                    if let Some(input) = input_mesh(g, n) {
                        set_floats(g, n, MO_REST, flat(input.positions()))?;
                    }
                    g.connect(&plug(m, MeshNode::OUT_MESH), &plug(n, MO_TARGET), false)?;
                    let xf = crate::dag::transform_of(g, m);
                    g.connect(&plug(xf, Transform::WORLD_MATRIX), &plug(n, MO_MATRIX), false)?;
                    Ok(())
                })
            },
        ))),
        vec![
            f("morphTarget", "mt", ArgType::String, "The mesh to morph to."),
            f("morphSpace", "ms", ArgType::Int, "0 object, 1 world."),
            fx("normalScale", "nsc", "How much of the morph along the normal applies (1)."),
            fx("tangentScale", "tsc", "How much of the morph across the surface applies (1)."),
        ],
    ));

    c.register(setting_flags(
        modes(deform::common_flags(Cmd::new(
            "wrinkle",
            "Wrinkle: creases around a handle. Returns [wrinkle1, wrinkle1Handle].",
            |s, a| {
                if let Some(r) = settings_modes(s, a, "wrinkle", WRINKLE) {
                    return r;
                }
                wrinkle_create(s, a)
            },
        ))),
        vec![
            f("wrinkleCount", "wc", ArgType::Int, "How many creases (3)."),
            fx("wrinkleIntensity", "wi", "How deep they go (0.5)."),
            fx("thickness", "th", "How wide they are (1)."),
            fx("randomness", "rnd", "How irregular they are (0.2)."),
            f("style", "st", ArgType::String, "radial (the default), tangential or lines."),
            fx("dropoffDistance", "dds", "How far they reach from the handle."),
            fx("scale", "sc", "Multiplies the depth (1)."),
            bx("x", "x", "Lines run across X rather than Z."),
        ],
    ));

    c.register(setting_flags(
        modes(deform::common_flags(Cmd::new(
            "proximityWrap",
            "Proximity Wrap: points follow the nearest points of driver meshes. proximityWrap -drivers d obj; -e -addDrivers d; -q -drivers. Returns \
             [proximityWrap1].",
            |s, a| {
                if a.bool("query") && asked(a, "drivers") {
                    let Some(name) = a.named().into_iter().next() else { fail!("proximityWrap: name the deformer") };
                    let n = named(&s.graph, &name, "proximityWrap")?;
                    let g = &s.graph;
                    let list: Vec<String> = g.elements(&plug(n, PW_DRIVER)).iter().filter_map(|e| g.source(e)).map(|p| g.path(p.node)).collect();
                    return Ok(json!(list));
                }
                if a.bool("edit") {
                    let Some(name) = a.named().into_iter().next() else { fail!("proximityWrap: name the deformer") };
                    let n = named(&s.graph, &name, "proximityWrap")?;
                    if let Some(d) = a.get("addDrivers").and_then(Value::as_str) {
                        pw_add_driver(&mut s.graph, n, d)?;
                        pw_bind(&mut s.graph, n)?;
                    }
                }
                if let Some(r) = settings_modes(s, a, "proximityWrap", PROXIMITY) {
                    if r.is_ok() && a.bool("edit") {
                        let name = a.named().into_iter().next().unwrap_or_default();
                        let n = named(&s.graph, &name, "proximityWrap")?;
                        pw_bind(&mut s.graph, n)?;
                    }
                    return r;
                }
                let drivers: Vec<String> = match a.get("drivers") {
                    Some(Value::String(d)) => vec![d.clone()],
                    Some(Value::Array(v)) => v.iter().filter_map(|x| x.as_str().map(str::to_string)).collect(),
                    _ => Vec::new(),
                };
                make_on(s, a, "proximityWrap", PROXIMITY, |_, _| 0, |g, n, _, _| {
                    for d in &drivers {
                        pw_add_driver(g, n, d)?;
                    }
                    if !drivers.is_empty() {
                        pw_bind(g, n)?;
                    }
                    Ok(())
                })
            },
        ))),
        vec![
            f("drivers", "drv", ArgType::Any, "The driver meshes. Query: them."),
            f("addDrivers", "ad", ArgType::String, "With -e: a driver to add."),
            f("wrapMode", "wm", ArgType::Int, "0 offset, 1 surface, 2 snap."),
            f("maxDrivers", "md", ArgType::Int, "How many driver points each point follows (4)."),
            fx("falloffScale", "fsc", "How far each binding reaches (1)."),
            fx("dropoffRateScale", "drs", "How fast a binding's weight falls with distance (1)."),
        ],
    ));

    c.register(setting_flags(
        modes(deform::common_flags(Cmd::new(
            "solidify",
            "Solidify: the mesh keeps its rest shape, fitted to what comes in. Returns [solidify1].",
            |s, a| {
                if let Some(r) = settings_modes(s, a, "solidify", SOLIDIFY) {
                    return r;
                }
                make_on(s, a, "solidify", SOLIDIFY, |_, _| 0, |g, n, _, _| {
                    let Some(input) = input_mesh(g, n) else { return Ok(()) };
                    let geom = g.node(n).unwrap().data().matrix(GEOM);
                    let pts: Vec<DVec3> = input.positions().iter().map(|p| geom.transform_point3(*p)).collect();
                    set_floats(g, n, SO_REST, flat(&pts))
                })
            },
        ))),
        vec![fx("scaleEnvelope", "se", "How much of the size change comes through (1).")],
    ));

    let on_curve = |ty: &'static str, at: usize, settings: &'static [&'static str], rest_at: Option<usize>| {
        move |s: &mut Session, a: &Args| -> CmdResult {
            if let Some(r) = settings_modes(s, a, ty, settings) {
                return r;
            }
            let flag = a.get("curve").and_then(Value::as_str).map(str::to_string);
            make_on(s, a, ty, settings, |g, names| names.iter().rev().take_while(|n| is_curve_name(g, n)).count(), |g, n, rest, _| {
                let Some(c) = flag.as_deref().or(rest.first().map(String::as_str)) else { fail!("{ty}: name a curve") };
                connect_curve(g, n, at, c)?;
                if let Some(r) = rest_at {
                    g.evaluate();
                    let v = g.value(&plug(n, at))?;
                    g.set_value(plug(n, r), v)?;
                }
                Ok(())
            })
        }
    };
    c.register(setting_flags(
        modes(deform::common_flags(Cmd::new("pointOnCurveDeformer", "Point on Curve: points go with one place on a curve, by how far it has moved. Returns [pointOnCurve1].", on_curve("pointOnCurve", PC_CURVE, POINT_CURVE, Some(PC_REST))))),
        vec![f("curve", "c", ArgType::String, "The curve; or name it last."), fx("parameter", "pr", "The place on the curve."), bx("turnOnPercentage", "top", "The parameter is a fraction of the length.")],
    ));
    c.register(setting_flags(
        modes(deform::common_flags(Cmd::new("curveWarp", "Curve Warp: lays a mesh along a curve, named by -c or last. Returns [curveWarp1].", on_curve("curveWarp", CW_CURVE, WARP, None)))),
        vec![
            f("curve", "c", ArgType::String, "The curve; or name it last."),
            fx("lengthScale", "ls", "Stretches the mesh along the curve (1)."),
            fx("offset", "off", "Slides the mesh along the curve, as a fraction."),
            fx("twistRotation", "tw", "Degrees of twist from end to end."),
            fx("rollRotation", "rr", "Degrees the mesh rolls about the curve."),
            f("alignmentMode", "am", ArgType::Int, "0 auto, 1 X, 2 Y, 3 Z."),
            bx("keepLength", "kl", "Keep the mesh's length, not the curve's."),
        ],
    ));

    c.register(setting_flags(
        modes(deform::common_flags(Cmd::new(
            "textureDeformer",
            "Texture Deformer: displaces points by a procedural value plus whatever is connected to .texture. Returns [textureDeformer1].",
            |s, a| {
                if let Some(r) = settings_modes(s, a, "textureDeformer", TEXTURE) {
                    return r;
                }
                make_on(s, a, "textureDeformer", TEXTURE, |_, _| 0, |_, _, _, _| Ok(()))
            },
        ))),
        vec![
            fx("strength", "str", "Multiplies the texture (1)."),
            fx("offset", "off", "Added to the displacement."),
            f("direction", "dir", ArgType::Int, "0 normal, 1 vector."),
            f("vectorStrength", "vs", ArgType::Vec3, "With direction vector: on each axis."),
            f("vectorOffset", "vo", ArgType::Vec3, "With direction vector: added on each axis."),
            fx("noiseFrequency", "nf", "How fine the procedural texture is (1)."),
            fx("noiseAmplitude", "na", "How strong the procedural texture is (1)."),
            f("pointSpace", "ps", ArgType::Int, "0 world, 1 local."),
        ],
    ));
}
