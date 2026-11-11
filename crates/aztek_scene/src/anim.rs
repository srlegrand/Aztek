//! Keyframe animation as the reference does it: animCurve nodes (TL, TA, TU) driving channels, keyed with setKeyframe and
//! edited with keyframe, cutKey and keyTangent; the time range in playbackOptions; times in frames.

use std::any::Any;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use aztek_ops::{fail, objects, ArgType, CmdResult, CommandDef, Commands, FlagDef};
use serde_json::{json, Value};

use crate::graph::{DrivenEdit, Graph};
use crate::node::{types, EvalContext, NodeBehavior, NodeData, NodeType};
use crate::plug::{flags, AttrDef, NodeId, PlugRef, PlugType, PlugValue, NO_NODE};
use crate::session::Session;

// ---------------------------------------------------------------------------------------------------------------------
// Settings

static FPS_BITS: AtomicU64 = AtomicU64::new(0x4038_0000_0000_0000); // 24.0
/// 29.97 frames a second counted as drop frame timecode (`29.97df`) rather than plain `29.97fps`.
static DROP_FRAME: AtomicBool = AtomicBool::new(false);
static AUTO_KEY: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Debug)]
pub struct Range {
    pub min: f64,
    pub max: f64,
    pub anim_start: f64,
    pub anim_end: f64,
    pub loop_mode: String,
    pub speed: f64,
    pub default_in: Tangent,
    pub default_out: Tangent,
    /// New curves are weighted (keyTangent -g -wt).
    pub default_weighted: bool,
    /// Frames between played frames (playbackOptions -by).
    pub by: f64,
    /// all or active: which views update during playback.
    pub view: String,
    /// The most frames per second playback may reach, 0 for no limit.
    pub max_speed: f64,
    /// playbackOptions -pp: stored for scripts that set it.
    pub preroll: f64,
}

static RANGE: Mutex<Option<Range>> = Mutex::new(None);

fn default_range() -> Range {
    Range {
        min: 1.0,
        max: 120.0,
        anim_start: 1.0,
        anim_end: 200.0,
        loop_mode: "continuous".into(),
        speed: 1.0,
        default_in: Tangent::Auto,
        default_out: Tangent::Auto,
        default_weighted: false,
        by: 1.0,
        view: "all".into(),
        max_speed: 0.0,
        preroll: 0.0,
    }
}

pub fn range() -> Range {
    RANGE.lock().unwrap().clone().unwrap_or_else(default_range)
}

pub(crate) fn set_range(f: impl FnOnce(&mut Range)) {
    let mut r = RANGE.lock().unwrap();
    let mut v = r.clone().unwrap_or_else(default_range);
    f(&mut v);
    *r = Some(v);
}

pub fn fps() -> f64 {
    f64::from_bits(FPS_BITS.load(Ordering::Relaxed))
}

pub(crate) fn set_fps(v: f64) {
    if v > 0.0 {
        FPS_BITS.store(v.to_bits(), Ordering::Relaxed);
        DROP_FRAME.store(false, Ordering::Relaxed);
    }
}

pub fn drop_frame() -> bool {
    DROP_FRAME.load(Ordering::Relaxed)
}

pub(crate) fn set_drop_frame(on: bool) {
    DROP_FRAME.store(on, Ordering::Relaxed);
}

pub fn auto_key() -> bool {
    AUTO_KEY.load(Ordering::Relaxed)
}

/// The graph's time (seconds) as a frame.
pub fn frame(g: &Graph) -> f64 {
    let f = g.time() * fps();
    let r = f.round();
    if (f - r).abs() < 1e-6 {
        r
    } else {
        f
    }
}

/// A frame as JSON: whole frames print as integers, as the reference's results do.
pub fn frame_json(f: f64) -> Value {
    if f.fract() == 0.0 && f.abs() < 1e15 {
        json!(f as i64)
    } else {
        json!(f)
    }
}

pub fn set_frame(g: &mut Graph, f: f64) {
    g.set_time(f / fps());
}

// ---------------------------------------------------------------------------------------------------------------------
// Keys

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tangent {
    Auto,
    Spline,
    Linear,
    Flat,
    Step,
    StepNext,
    Clamped,
    Plateau,
    Fixed,
}

impl Tangent {
    pub const NAMES: &'static [&'static str] = &["auto", "spline", "linear", "flat", "step", "stepnext", "clamped", "plateau", "fixed"];

    pub fn name(self) -> &'static str {
        Self::NAMES[self as usize]
    }

    pub fn parse(s: &str) -> Option<Tangent> {
        use Tangent::*;
        Some(match s {
            "auto" => Auto,
            "spline" | "smooth" => Spline,
            "linear" => Linear,
            "flat" => Flat,
            "step" => Step,
            "stepnext" => StepNext,
            "clamped" => Clamped,
            "plateau" => Plateau,
            "fixed" => Fixed,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Key {
    /// Frame.
    pub t: f64,
    pub v: f64,
    pub itt: Tangent,
    pub ott: Tangent,
    /// Slopes (value per frame) of fixed tangents.
    pub in_slope: f64,
    pub out_slope: f64,
    pub breakdown: bool,
    /// Length of fixed tangents on a weighted curve: the tangent (x seconds, y value) has this length.
    pub in_weight: f64,
    pub out_weight: f64,
    /// In and out tangents move together (keyTangent -l).
    pub tan_lock: bool,
    /// The graph editor keeps the handle's length when dragging it (keyTangent -wl).
    pub weight_lock: bool,
    /// Drawn in the special tick colour on the time slider (keyframe -tds).
    pub tick: bool,
}

impl Key {
    pub fn new(t: f64, v: f64, itt: Tangent, ott: Tangent) -> Key {
        Key { t, v, itt, ott, in_slope: 0.0, out_slope: 0.0, breakdown: false, in_weight: 1.0, out_weight: 1.0, tan_lock: true, weight_lock: true, tick: false }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Keys {
    pub keys: Vec<Key>,
    /// The curve's weightedTangents attr, filled in by `keys_of`; not stored with the keys.
    pub weighted: bool,
}

impl crate::plug::Data for Keys {
    fn kind(&self) -> &str {
        "animKeys"
    }
    fn to_json(&self) -> Value {
        let k: Vec<Value> = self
            .keys
            .iter()
            .map(|k| json!([k.t, k.v, k.itt.name(), k.ott.name(), k.in_slope, k.out_slope, k.breakdown, k.in_weight, k.out_weight, k.tan_lock, k.weight_lock, k.tick]))
            .collect();
        json!({"kind": "animKeys", "keys": k})
    }
    fn bytes(&self) -> usize {
        self.keys.len() * std::mem::size_of::<Key>()
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Keys {
    pub fn from_json(j: &Value) -> Keys {
        let mut keys = Vec::new();
        for e in j.get("keys").and_then(Value::as_array).into_iter().flatten() {
            let f = |i: usize| e.get(i).and_then(Value::as_f64).unwrap_or(0.0);
            let t = |i: usize| e.get(i).and_then(Value::as_str).and_then(Tangent::parse).unwrap_or(Tangent::Auto);
            let b = |i: usize, d: bool| e.get(i).and_then(Value::as_bool).unwrap_or(d);
            keys.push(Key {
                t: f(0),
                v: f(1),
                itt: t(2),
                ott: t(3),
                in_slope: f(4),
                out_slope: f(5),
                breakdown: b(6, false),
                in_weight: e.get(7).and_then(Value::as_f64).unwrap_or(1.0),
                out_weight: e.get(8).and_then(Value::as_f64).unwrap_or(1.0),
                tan_lock: b(9, true),
                weight_lock: b(10, true),
                tick: b(11, false),
            });
        }
        Keys { keys, weighted: false }
    }

    pub fn sort(&mut self) {
        self.keys.sort_by(|a, b| a.t.total_cmp(&b.t));
        self.keys.dedup_by(|b, a| (a.t - b.t).abs() < 1e-9);
    }

    /// Sets or replaces the key at t.
    pub fn set(&mut self, k: Key) {
        match self.keys.iter_mut().find(|e| (e.t - k.t).abs() < 1e-6) {
            Some(e) => {
                e.v = k.v;
            }
            None => {
                self.keys.push(k);
                self.sort();
            }
        }
    }

    /// (in, out) slopes of key i, value per frame.
    pub fn slopes(&self, i: usize) -> (f64, f64) {
        let k = &self.keys[i];
        let prev = i.checked_sub(1).map(|j| &self.keys[j]);
        let next = self.keys.get(i + 1);
        let seg = |a: &Key, b: &Key| if (b.t - a.t).abs() < 1e-12 { 0.0 } else { (b.v - a.v) / (b.t - a.t) };
        let left = prev.map(|p| seg(p, k));
        let right = next.map(|n| seg(k, n));
        let smooth = match (prev, next) {
            (Some(p), Some(n)) => seg(p, n),
            _ => left.or(right).unwrap_or(0.0),
        };
        let auto = || -> f64 {
            let (Some(p), Some(n)) = (prev, next) else { return 0.0 };
            // Flat at peaks and valleys; never overshooting the neighbours otherwise.
            if (k.v - p.v) * (n.v - k.v) <= 0.0 {
                return 0.0;
            }
            let m = smooth;
            let lim = 3.0 * left.unwrap().abs().min(right.unwrap().abs());
            m.clamp(-lim, lim)
        };
        let clamped = || -> f64 {
            // The reference flattens a clamped key when a neighbour is within 0.05 of it.
            let near = |o: Option<&Key>| o.is_some_and(|o| (o.v - k.v).abs() <= 0.05);
            if near(prev) || near(next) {
                0.0
            } else {
                smooth
            }
        };
        let one = |t: Tangent, fixed: f64, side: Option<f64>| -> f64 {
            match t {
                Tangent::Auto | Tangent::Plateau => auto(),
                Tangent::Spline => smooth,
                Tangent::Clamped => clamped(),
                Tangent::Linear => side.unwrap_or(0.0),
                Tangent::Flat | Tangent::Step | Tangent::StepNext => 0.0,
                Tangent::Fixed => fixed,
            }
        };
        (one(k.itt, k.in_slope, left.or(right)), one(k.ott, k.out_slope, right.or(left)))
    }

    /// The tangent of key i as the reference holds it: x in seconds, y in value. Fixed tangents have their stored length;
    /// the others reach as far in time as the neighbouring key, which makes a weighted curve match the Hermite one.
    pub fn tangent_xy(&self, i: usize, out: bool) -> (f64, f64) {
        let (si, so) = self.slopes(i);
        let k = &self.keys[i];
        let (ty, slope, w) = if out { (k.ott, so, k.out_weight) } else { (k.itt, si, k.in_weight) };
        if ty == Tangent::Fixed {
            let a = (slope * fps()).atan();
            return (w * a.cos(), w * a.sin());
        }
        let side = if out { self.keys.get(i + 1) } else { i.checked_sub(1).map(|j| &self.keys[j]) };
        let other = if out { i.checked_sub(1).map(|j| &self.keys[j]) } else { self.keys.get(i + 1) };
        let dt = side.or(other).map(|o| (o.t - k.t).abs()).unwrap_or(fps());
        (dt / fps(), slope * dt)
    }

    /// What keyTangent reports: the tangent itself on a weighted curve, a unit vector on an unweighted one.
    pub fn tangent_vector(&self, i: usize, out: bool) -> (f64, f64) {
        if self.weighted {
            return self.tangent_xy(i, out);
        }
        let (si, so) = self.slopes(i);
        let a = (if out { so } else { si } * fps()).atan();
        (a.cos(), a.sin())
    }

    /// Holds a key's current tangents as fixed ones, keeping the curve's shape. Step tangents stay.
    pub fn freeze(&mut self, i: usize) {
        let (si, so) = self.slopes(i);
        let (ix, iy) = self.tangent_xy(i, false);
        let (ox, oy) = self.tangent_xy(i, true);
        let k = &mut self.keys[i];
        if !matches!(k.itt, Tangent::Step | Tangent::StepNext) {
            k.itt = Tangent::Fixed;
            k.in_slope = si;
            k.in_weight = ix.hypot(iy);
        }
        if !matches!(k.ott, Tangent::Step | Tangent::StepNext) {
            k.ott = Tangent::Fixed;
            k.out_slope = so;
            k.out_weight = ox.hypot(oy);
        }
    }

    /// The control points between keys a and a+1 of a weighted curve, in frames and value, kept inside the span in
    /// time so the curve stays a function of time.
    fn bezier_handles(&self, a: usize) -> [(f64, f64); 2] {
        let (k0, k1) = (&self.keys[a], &self.keys[a + 1]);
        let h = k1.t - k0.t;
        let (ox, oy) = self.tangent_xy(a, true);
        let (ix, iy) = self.tangent_xy(a + 1, false);
        let f = fps();
        // A handle is a third of the tangent.
        let (mut dx1, mut dy1) = (ox * f / 3.0, oy / 3.0);
        let (mut dx2, mut dy2) = (ix * f / 3.0, iy / 3.0);
        if dx1 > h {
            dy1 *= h / dx1;
            dx1 = h;
        }
        if dx2 > h {
            dy2 *= h / dx2;
            dx2 = h;
        }
        [(k0.t + dx1, k0.v + dy1), (k1.t - dx2, k1.v - dy2)]
    }

    fn eval_bezier(&self, a: usize, t: f64) -> f64 {
        let (k0, k1) = (&self.keys[a], &self.keys[a + 1]);
        let [(x1, y1), (x2, y2)] = self.bezier_handles(a);
        let (x0, x3) = (k0.t, k1.t);
        let s = bezier_param(x0, x1, x2, x3, t);
        let u = 1.0 - s;
        u * u * u * k0.v + 3.0 * u * u * s * y1 + 3.0 * u * s * s * y2 + s * s * s * k1.v
    }

    /// The curve between its first and last key.
    pub(crate) fn eval_inside(&self, t: f64, weighted: bool) -> f64 {
        let ks = &self.keys;
        let i = ks.partition_point(|k| k.t <= t);
        if i == 0 {
            return ks[0].v;
        }
        if i >= ks.len() {
            return ks[ks.len() - 1].v;
        }
        let (a, b) = (&ks[i - 1], &ks[i]);
        if a.ott == Tangent::Step {
            return a.v;
        }
        if a.ott == Tangent::StepNext {
            return b.v;
        }
        let h = b.t - a.t;
        if h <= 1e-12 {
            return b.v;
        }
        if weighted {
            return self.eval_bezier(i - 1, t);
        }
        let m0 = self.slopes(i - 1).1;
        let m1 = self.slopes(i).0;
        let s = (t - a.t) / h;
        let (s2, s3) = (s * s, s * s * s);
        (2.0 * s3 - 3.0 * s2 + 1.0) * a.v + (s3 - 2.0 * s2 + s) * h * m0 + (-2.0 * s3 + 3.0 * s2) * b.v + (s3 - s2) * h * m1
    }

    /// Splits the curve at frame t with a key that leaves its shape as it was. The tangents around it become fixed.
    /// False when the curve is empty or already has a key there.
    pub fn insert(&mut self, t: f64) -> bool {
        let n = self.keys.len();
        if n == 0 || self.keys.iter().any(|k| (k.t - t).abs() < 1e-6) {
            return false;
        }
        let i = self.keys.partition_point(|k| k.t <= t);
        let r = range();
        if i == 0 || i >= n {
            // Outside the keys: a key at the value the curve holds there.
            let v = if i == 0 { self.keys[0].v } else { self.keys[n - 1].v };
            self.keys.insert(i, Key::new(t, v, r.default_in, r.default_out));
            return true;
        }
        let v = self.eval_inside(t, self.weighted);
        let stepped = matches!(self.keys[i - 1].ott, Tangent::Step | Tangent::StepNext);
        let weighted = self.weighted;
        let (mut s_in, mut s_out, mut w_in, mut w_out) = (0.0, 0.0, 1.0, 1.0);
        let mut left = None;
        let mut right = None;
        if !stepped {
            let (a, b) = (self.keys[i - 1], self.keys[i]);
            let h = b.t - a.t;
            if weighted {
                // De Casteljau: the two halves' control points.
                let [(x1, y1), (x2, y2)] = self.bezier_handles(i - 1);
                let s = bezier_param(a.t, x1, x2, b.t, t);
                let l = |p: (f64, f64), q: (f64, f64)| (p.0 + (q.0 - p.0) * s, p.1 + (q.1 - p.1) * s);
                let (p01, p12, p23) = (l((a.t, a.v), (x1, y1)), l((x1, y1), (x2, y2)), l((x2, y2), (b.t, b.v)));
                let (p012, p123) = (l(p01, p12), l(p12, p23));
                let f = fps();
                let measure = |d: (f64, f64)| (d.1 / d.0.max(1e-9), (3.0 * d.0 / f).hypot(3.0 * d.1));
                left = Some(measure((p01.0 - a.t, p01.1 - a.v)));
                right = Some(measure((b.t - p23.0, b.v - p23.1)));
                (s_in, w_in) = measure((t - p012.0, v - p012.1));
                (s_out, w_out) = measure((p123.0 - t, p123.1 - v));
            } else {
                let (m0, m1) = (self.slopes(i - 1).1, self.slopes(i).0);
                let s = (t - a.t) / h;
                // The derivative of the Hermite segment.
                let d = ((6.0 * s * s - 6.0 * s) * a.v + (3.0 * s * s - 4.0 * s + 1.0) * h * m0 + (-6.0 * s * s + 6.0 * s) * b.v + (3.0 * s * s - 2.0 * s) * h * m1) / h;
                s_in = d;
                s_out = d;
            }
        }
        self.freeze(i - 1);
        self.freeze(i);
        if let Some((sl, wl)) = left {
            self.keys[i - 1].out_slope = sl;
            self.keys[i - 1].out_weight = wl;
        }
        if let Some((sr, wr)) = right {
            self.keys[i].in_slope = sr;
            self.keys[i].in_weight = wr;
        }
        let mut k = Key::new(t, v, Tangent::Fixed, Tangent::Fixed);
        k.in_slope = s_in;
        k.out_slope = s_out;
        k.in_weight = w_in;
        k.out_weight = w_out;
        k.tan_lock = (s_in - s_out).abs() < 1e-9;
        if stepped {
            k.ott = self.keys[i - 1].ott;
        }
        self.keys.insert(i, k);
        true
    }

    /// The value at frame t, with the curve's infinities before and after its keys.
    pub fn eval(&self, t: f64, pre: Infinity, post: Infinity) -> f64 {
        self.eval_mode(t, pre, post, self.weighted)
    }

    pub(crate) fn eval_mode(&self, t: f64, pre: Infinity, post: Infinity, weighted: bool) -> f64 {
        let ks = &self.keys;
        if ks.is_empty() {
            return 0.0;
        }
        let (t0, t1) = (ks[0].t, ks[ks.len() - 1].t);
        let span = t1 - t0;
        if t >= t0 && t <= t1 || span <= 1e-9 {
            return self.eval_inside(t.clamp(t0, t1), weighted);
        }
        let (inf, before) = if t < t0 { (pre, true) } else { (post, false) };
        let (v0, v1) = (ks[0].v, ks[ks.len() - 1].v);
        match inf {
            Infinity::Constant => {
                if before {
                    v0
                } else {
                    v1
                }
            }
            Infinity::Linear => {
                if before {
                    v0 + self.slopes(0).0 * (t - t0)
                } else {
                    v1 + self.slopes(ks.len() - 1).1 * (t - t1)
                }
            }
            Infinity::Cycle | Infinity::CycleRelative | Infinity::Oscillate => {
                let n = ((t - t0) / span).floor();
                let mut local = t - n * span;
                if inf == Infinity::Oscillate && (n as i64).rem_euclid(2) == 1 {
                    local = t1 - (local - t0);
                }
                let v = self.eval_inside(local, weighted);
                if inf == Infinity::CycleRelative {
                    v + n * (v1 - v0)
                } else {
                    v
                }
            }
        }
    }
}

/// The s where a cubic Bezier from x0 to x3 with these inner control points reaches x. The x's rise, so Newton steps
/// kept inside a shrinking bracket always land.
fn bezier_param(x0: f64, x1: f64, x2: f64, x3: f64, x: f64) -> f64 {
    let (mut lo, mut hi) = (0.0_f64, 1.0_f64);
    let mut s = ((x - x0) / (x3 - x0)).clamp(0.0, 1.0);
    for _ in 0..60 {
        let u = 1.0 - s;
        let e = u * u * u * x0 + 3.0 * u * u * s * x1 + 3.0 * u * s * s * x2 + s * s * s * x3 - x;
        if e.abs() < 1e-10 {
            break;
        }
        if e > 0.0 {
            hi = s;
        } else {
            lo = s;
        }
        let d = 3.0 * (u * u * (x1 - x0) + 2.0 * u * s * (x2 - x1) + s * s * (x3 - x2));
        let n = if d.abs() > 1e-12 { s - e / d } else { -1.0 };
        s = if n > lo && n < hi { n } else { 0.5 * (lo + hi) };
    }
    s
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Infinity {
    Constant,
    Linear,
    Cycle,
    CycleRelative,
    Oscillate,
}

pub const INFINITY_NAMES: &[&str] = &["constant", "linear", "constant", "cycle", "cycleRelative", "oscillate"];

pub(crate) fn infinity(i: i64) -> Infinity {
    match i {
        1 => Infinity::Linear,
        3 => Infinity::Cycle,
        4 => Infinity::CycleRelative,
        5 => Infinity::Oscillate,
        _ => Infinity::Constant,
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Curve nodes

pub const CURVE_TYPES: &[&str] = &["animCurveTL", "animCurveTA", "animCurveTU"];
pub const KEYS: usize = 0;
pub const PRE: usize = 1;
pub const POST: usize = 2;
pub const OUTPUT: usize = 3;
pub const WEIGHTED: usize = 4;
pub const ROT_INTERP: usize = 5;
/// Rotation curves converted to quaternion interpolation read the keys of all three axes through these.
pub const ROT_KEYS: usize = 6;
pub const ROT_AXIS: usize = 9;
pub const ROT_ORDER: usize = 10;
/// The attrs every animCurve has; driven curves add theirs after these.
/// From a timeWarp: the frames it moves.
pub const TIME_WARP: usize = 11;
pub const BASE_ATTRS: usize = 12;

pub const ROT_INTERP_NAMES: &[&str] = &["none", "euler", "quaternionSlerp", "quaternionSquad", "quaternion"];

pub struct CurveNode;

impl NodeBehavior for CurveNode {
    fn compute(&mut self, ctx: &EvalContext, d: &mut NodeData) {
        let v = match d.value(KEYS) {
            PlugValue::Data(Some(k)) => k.as_any().downcast_ref::<Keys>().map(|k| {
                let f = ctx.time * fps();
                let r = f.round();
                let f = if (f - r).abs() < 1e-6 { r } else { f };
                let f = match d.value(TIME_WARP) {
                    PlugValue::Data(Some(w)) => w.as_any().downcast_ref::<crate::rig::JsonData>().map_or(f, |w| crate::anim_misc::warped_frame(&w.0, f)),
                    _ => f,
                };
                if let Some(v) = (d.int(ROT_INTERP) >= 2).then(|| crate::anim_filter::eval_quaternion(k, d, f)).flatten() {
                    return v;
                }
                k.eval_mode(f, infinity(d.int(PRE)), infinity(d.int(POST)), d.bool(WEIGHTED))
            }),
            _ => None,
        };
        d.set_output(OUTPUT, PlugValue::Float(v.unwrap_or(0.0)));
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn hidden_keys(name: &str, short: &str) -> AttrDef {
    AttrDef {
        name: name.into(),
        short_name: short.into(),
        ty: PlugType::Data,
        default_value: PlugValue::Data(None),
        flags: flags::INPUT | flags::STORABLE | flags::HIDDEN,
        doc: "A copy of the keys of one rotation axis.".into(),
        ..Default::default()
    }
}

fn hidden_int(name: &str, short: &str) -> AttrDef {
    AttrDef {
        name: name.into(),
        short_name: short.into(),
        ty: PlugType::Int,
        default_value: PlugValue::Int(0),
        flags: flags::INPUT | flags::STORABLE | flags::HIDDEN,
        doc: "Used by quaternion interpolation.".into(),
        ..Default::default()
    }
}

pub fn register_types() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(register_types_now);
}

fn register_types_now() {
    crate::plug::data::register_kind("animKeys", Arc::new(|j| Ok(Arc::new(Keys::from_json(j)) as Arc<dyn crate::plug::Data>)));
    let inf = |name: &str, short: &str, doc: &str| AttrDef {
        name: name.into(),
        short_name: short.into(),
        ty: PlugType::Enum,
        default_value: PlugValue::Int(0),
        flags: flags::INPUT | flags::STORABLE,
        enum_names: INFINITY_NAMES.iter().map(|s| s.to_string()).collect(),
        doc: doc.into(),
        ..Default::default()
    };
    let base = NodeType {
        name: "animCurve".into(),
        icon: "chart-line".into(),
        doc: "Keys over time driving one channel.".into(),
        time_dependent: true,
        own_attrs: vec![
            AttrDef {
                name: "keys".into(),
                short_name: "ktv".into(),
                ty: PlugType::Data,
                default_value: PlugValue::Data(None),
                flags: flags::INPUT | flags::STORABLE | flags::HIDDEN,
                doc: "The keys: time, value and tangents.".into(),
                ..Default::default()
            },
            inf("preInfinity", "pre", "What the curve does before its first key."),
            inf("postInfinity", "pst", "What the curve does after its last key."),
            AttrDef {
                name: "output".into(),
                short_name: "o".into(),
                ty: PlugType::Float,
                default_value: PlugValue::Float(0.0),
                flags: flags::OUTPUT,
                doc: "The value at the current time.".into(),
                ..Default::default()
            },
            AttrDef {
                name: "weightedTangents".into(),
                short_name: "wgt".into(),
                ty: PlugType::Bool,
                default_value: PlugValue::Bool(false),
                flags: flags::INPUT | flags::STORABLE,
                doc: "Tangents have length: the curve is a Bezier in time and value.".into(),
                ..Default::default()
            },
            AttrDef {
                name: "rotationInterpolation".into(),
                short_name: "roti".into(),
                ty: PlugType::Enum,
                default_value: PlugValue::Int(0),
                flags: flags::INPUT | flags::STORABLE,
                enum_names: ROT_INTERP_NAMES.iter().map(|s| s.to_string()).collect(),
                doc: "Rotation curves: how the three axes interpolate together. See rotationInterpolation.".into(),
                ..Default::default()
            },
            hidden_keys("rotationKeysX", "rkx"),
            hidden_keys("rotationKeysY", "rky"),
            hidden_keys("rotationKeysZ", "rkz"),
            hidden_int("rotationAxis", "rax"),
            hidden_int("rotationOrder", "rord"),
            AttrDef { name: "timeWarp".into(), short_name: "twp".into(), ty: PlugType::Data, default_value: PlugValue::Data(None), flags: flags::INPUT | flags::HIDDEN, doc: "From a timeWarp node.".into(), ..Default::default() },
        ],
        ..Default::default()
    };
    let base = types::register_type(base).expect("animCurve");
    for (name, doc) in [
        ("animCurveTL", "Keys on a distance channel (translate)."),
        ("animCurveTA", "Keys on an angle channel (rotate), in degrees."),
        ("animCurveTU", "Keys on a unitless channel (scale, visibility, custom)."),
    ] {
        let t = NodeType {
            name: name.into(),
            base: Some(base.clone()),
            time_dependent: true,
            doc: doc.into(),
            create: Some(Arc::new(|| Box::new(CurveNode))),
            ..Default::default()
        };
        types::register_type(t).expect(name);
    }
}

pub fn is_curve(g: &Graph, id: NodeId) -> bool {
    g.node(id).is_some_and(|n| n.ty().is_a("animCurve"))
}

pub fn keys_of(g: &Graph, curve: NodeId) -> Keys {
    match g.node(curve).map(|n| n.data().value(KEYS)) {
        Some(PlugValue::Data(Some(k))) => {
            let mut k = k.as_any().downcast_ref::<Keys>().cloned().unwrap_or_default();
            k.weighted = g.node(curve).is_some_and(|n| n.data().bool(WEIGHTED));
            k
        }
        _ => Keys { keys: Vec::new(), weighted: g.node(curve).is_some_and(|n| n.data().bool(WEIGHTED)) },
    }
}

pub fn set_keys(g: &mut Graph, curve: NodeId, k: Keys) -> CmdResult<()> {
    let data: Arc<dyn crate::plug::Data> = Arc::new(k);
    g.set_value(PlugRef::new(curve, KEYS as i32), PlugValue::Data(Some(data.clone())))?;
    // Curves under quaternion interpolation each keep a copy of the others' keys: connecting them would be a cycle.
    let d = g.node(curve).map(|n| (n.data().int(ROT_INTERP), n.data().int(ROT_AXIS)));
    if let Some((mode, axis)) = d {
        if mode >= 2 {
            for other in rotation_siblings(g, curve) {
                if other != curve {
                    g.set_value(PlugRef::new(other, (ROT_KEYS as i64 + axis) as i32), PlugValue::Data(Some(data.clone())))?;
                }
            }
        }
    }
    Ok(())
}

/// The curves of a rotate's three axes, given one of them.
pub(crate) fn rotation_siblings(g: &Graph, curve: NodeId) -> Vec<NodeId> {
    let Some(p) = driven_by(g, curve).into_iter().find(|p| p.component >= 0) else { return Vec::new() };
    (0..3).filter_map(|c| curve_of(g, &PlugRef::component(p.node, p.attr, c))).collect()
}

/// The curve driving a plug, if any: through a mute node, and for a channel in animation layers, the curve of the layer
/// keys go to.
pub fn curve_of(g: &Graph, p: &PlugRef) -> Option<NodeId> {
    let s = crate::anim_layers::driver(g, p)?;
    if is_curve(g, s.node) && s.attr == OUTPUT as i32 {
        return Some(s.node);
    }
    if crate::anim_layers::is_blend(g, s.node) {
        return crate::anim_layers::best_curve(g, p);
    }
    None
}

/// The channels a curve drives, through mute and blend nodes.
pub fn driven_by(g: &Graph, curve: NodeId) -> Vec<PlugRef> {
    let mut out = Vec::new();
    for d in g.destinations(&PlugRef::new(curve, OUTPUT as i32)) {
        out.extend(crate::anim_layers::final_plugs(g, d));
    }
    out
}

/// Every (plug, curve) pair animating a node.
pub fn curves_on(g: &Graph, node: NodeId) -> Vec<(PlugRef, NodeId)> {
    let mut out: Vec<(PlugRef, NodeId)> = Vec::new();
    for (d, s) in g.connections() {
        if d.node != node {
            continue;
        }
        if is_curve(g, s.node) && s.attr == OUTPUT as i32 {
            out.push((d, s.node));
        } else if crate::anim_layers::is_routing(g, s.node) {
            // A rotation blend drives the whole vector; each axis has its own curves.
            let plugs = if d.component < 0 && g.attr(&d).ty == PlugType::Vec3 { (0..3).map(|c| PlugRef::component(d.node, d.attr, c)).collect() } else { vec![d] };
            for p in plugs {
                if let Some(c) = curve_of(g, &p) {
                    out.push((p, c));
                }
            }
        }
    }
    out.sort_by_key(|(d, _)| (d.attr, d.component));
    out
}

/// A transform's keyable channels, as the reference keys them with S.
pub fn keyable_plugs(g: &Graph, node: NodeId) -> Vec<PlugRef> {
    let Some(n) = g.node(node) else { return Vec::new() };
    let mut out = Vec::new();
    for (i, a) in n.ty().attrs.iter().enumerate() {
        if !a.has(flags::INPUT) || a.has(flags::HIDDEN) || a.is_array() {
            continue;
        }
        match a.ty {
            PlugType::Vec3 => (0..3).for_each(|c| {
                let p = PlugRef::component(node, i as i32, c);
                if is_keyable(g, &p) {
                    out.push(p);
                }
            }),
            PlugType::Float | PlugType::Int | PlugType::Bool | PlugType::Enum => {
                let p = PlugRef::new(node, i as i32);
                if is_keyable(g, &p) {
                    out.push(p);
                }
            }
            _ => {}
        }
    }
    out
}

/// Node by node, setAttr -k changes which channels are keyable. The changes live in an attr of their own, added the
/// first time, so they save and undo with the scene.
const KEYABLE_OVERRIDES: &str = "channelKeyable";

fn keyable_overrides(g: &Graph, node: NodeId) -> Option<Value> {
    let n = g.node(node)?;
    let at = n.ty().attr_index(KEYABLE_OVERRIDES)?;
    Some(crate::rig::json_of(n.data(), at))
}

/// Whether a channel is keyable: as setAttr -k left it, else as its type says.
pub fn is_keyable(g: &Graph, p: &PlugRef) -> bool {
    if let Some(v) = keyable_overrides(g, p.node).and_then(|o| o.get(channel_name(g, p)).and_then(Value::as_bool)) {
        return v;
    }
    g.attr(p).has(flags::KEYABLE)
}

/// setAttr -k. A whole vector means its three channels.
pub fn set_keyable(g: &mut Graph, p: &PlugRef, on: bool) -> CmdResult<()> {
    let at = match g.node(p.node).and_then(|n| n.ty().attr_index(KEYABLE_OVERRIDES)) {
        Some(at) => at,
        None => g.add_attr(
            p.node,
            AttrDef {
                name: KEYABLE_OVERRIDES.into(),
                short_name: "chky".into(),
                ty: PlugType::Data,
                default_value: PlugValue::Data(None),
                flags: flags::INPUT | flags::STORABLE | flags::HIDDEN,
                doc: "Channels made keyable, or not, on this node.".into(),
                ..Default::default()
            },
        )?,
    };
    let mut o = keyable_overrides(g, p.node).filter(Value::is_object).unwrap_or_else(|| json!({}));
    if p.component < 0 && g.attr(p).ty == PlugType::Vec3 {
        for c in 0..3 {
            o[channel_name(g, &PlugRef::component(p.node, p.attr, c))] = json!(on);
        }
    } else {
        o[channel_name(g, p)] = json!(on);
    }
    crate::rig::set_json(g, p.node, at, o)
}

pub(crate) fn curve_type_for(g: &Graph, p: &PlugRef) -> &'static str {
    let a = g.attr(p);
    if a.name == "rotate" || a.name.starts_with("rotate") && a.ty == PlugType::Float {
        "animCurveTA"
    } else if a.name == "translate" || a.name == "uValue" {
        "animCurveTL"
    } else {
        "animCurveTU"
    }
}

pub(crate) fn channel_name(g: &Graph, p: &PlugRef) -> String {
    if let Some(alias) = g.alias_of(p) {
        return alias;
    }
    let a = g.attr(p);
    if p.component >= 0 && a.ty == PlugType::Vec3 {
        a.child_names[p.component as usize].clone()
    } else {
        a.name.clone()
    }
}

pub(crate) fn plug_scalar(g: &mut Graph, p: &PlugRef) -> f64 {
    match g.value(p) {
        Ok(v) => v.scalar().unwrap_or(0.0),
        Err(_) => 0.0,
    }
}

fn steps_only(g: &Graph, p: &PlugRef) -> bool {
    matches!(g.attr(p).ty, PlugType::Bool | PlugType::Enum) && p.component < 0
}

/// Sets a key on a plug at frame t (value: the plug's current one when None). Makes and connects the curve if
/// needed. Returns false when the plug can't be keyed (locked, or driven by something else).
pub fn key_plug(g: &mut Graph, p: &PlugRef, t: f64, value: Option<f64>, itt: Option<Tangent>, ott: Option<Tangent>) -> CmdResult<bool> {
    key_plug_in(g, p, t, value, itt, ott, None, false)
}

/// key_plug, into a given animation layer if the plug is in it. no_resolve puts the value straight on the layer's curve
/// instead of the value that makes the channel come out at it.
#[allow(clippy::too_many_arguments)]
pub fn key_plug_in(g: &mut Graph, p: &PlugRef, t: f64, value: Option<f64>, itt: Option<Tangent>, ott: Option<Tangent>, layer: Option<NodeId>, no_resolve: bool) -> CmdResult<bool> {
    if g.is_locked(p) {
        return Ok(false);
    }
    let v = match value {
        Some(v) => v,
        None => plug_scalar(g, p),
    };
    let r = range();
    let stepped = steps_only(g, p);
    let new_itt = itt.unwrap_or(if stepped { Tangent::Step } else { r.default_in });
    let new_ott = ott.unwrap_or(if stepped { Tangent::Step } else { r.default_out });
    let driver = crate::anim_layers::driver(g, p);
    let (curve, v) = if driver.is_some_and(|s| crate::anim_layers::is_blend(g, s.node)) {
        let Some((c, v)) = crate::anim_layers::layer_curve_for_key(g, p, t, layer, v, value.is_some() && no_resolve)? else { return Ok(false) };
        (c, v)
    } else {
        match driver {
            Some(s) if is_curve(g, s.node) && s.attr == OUTPUT as i32 => (s.node, v),
            Some(_) => return Ok(false),
            None => {
                let name = format!("{}_{}", g.node(p.node).map(|n| n.name.rsplit(':').next().unwrap_or("").to_string()).unwrap_or_default(), channel_name(g, p));
                let slot = crate::anim_layers::curve_slot(g, p);
                (new_curve(g, curve_type_for(g, p), &name, &slot)?, v)
            }
        }
    };
    let mut k = keys_of(g, curve);
    match k.keys.iter_mut().find(|e| (e.t - t).abs() < 1e-6) {
        Some(e) => {
            e.v = v;
            if let Some(x) = itt {
                e.itt = x;
            }
            if let Some(x) = ott {
                e.ott = x;
            }
        }
        None => k.set(Key::new(t, v, new_itt, new_ott)),
    }
    set_keys(g, curve, k)?;
    Ok(true)
}

/// A new curve connected to a plug, weighted if that is the default.
pub(crate) fn new_curve(g: &mut Graph, ty: &str, name: &str, to: &PlugRef) -> CmdResult<NodeId> {
    let t = types::find(ty).unwrap();
    let c = g.create_node(&t, name, NO_NODE, true)?;
    if range().default_weighted {
        g.set_value(PlugRef::new(c, WEIGHTED as i32), PlugValue::Bool(true))?;
    }
    g.connect(&PlugRef::new(c, OUTPUT as i32), to, false)?;
    Ok(c)
}

/// Turns the plug arguments of a key command into plugs: "node.attr" as named, a node as its keyable channels (or the
/// -attribute ones), a character as its members. -hi and -s widen a node to its hierarchy and shapes.
pub(crate) fn targets(s: &Session, a: &aztek_ops::Args, cmd: &str) -> CmdResult<Vec<PlugRef>> {
    let g = &s.graph;
    let attrs: Vec<String> = match a.get("attribute") {
        Some(Value::String(x)) => vec![x.clone()],
        Some(Value::Array(v)) => v.iter().filter_map(|x| x.as_str().map(str::to_string)).collect(),
        _ => Vec::new(),
    };
    let split = |p: PlugRef| -> Vec<PlugRef> {
        if g.attr(&p).ty == PlugType::Vec3 && p.component < 0 {
            (0..3).map(|c| PlugRef::component(p.node, p.attr, c)).collect()
        } else {
            vec![p]
        }
    };
    let hi = a.string("hierarchy", "none");
    let want_shape = a.get("shape").and_then(Value::as_bool) == Some(true);
    let mut out = Vec::new();
    let mut names = objects(s, a);
    if a.named().is_empty() && names.is_empty() {
        names = s.current_characters.iter().map(|&c| g.path(c)).collect();
    }
    for name in names {
        if name.contains('.') && !name.ends_with(']') {
            out.extend(split(g.plug(&name)?));
            continue;
        }
        // An array element is a plug (blendShape1.w[0]); anything else ending in ] is a component.
        if name.contains('.') {
            if let Ok(p) = g.plug(&name) {
                if p.element >= 0 && g.attr(&p).is_array() {
                    out.push(p);
                    continue;
                }
            }
        }
        let base = name.split('.').next().unwrap_or(&name);
        let Some(id) = g.find(base)? else { fail!("{cmd}: no object named {name}") };
        if crate::characters::is_character(g, id) {
            let members = crate::characters::member_plugs(g, id);
            out.extend(members.into_iter().filter(|p| attrs.is_empty() || attrs.iter().any(|x| crate::characters::plug_matches(g, p, x))));
            continue;
        }
        // Components and shapes key their transform.
        let id = if g.node(id).is_some_and(|n| n.ty().dag && !n.ty().is_a("transform")) { crate::dag::transform_of(g, id) } else { id };
        let mut nodes = vec![id];
        if matches!(hi.as_str(), "below" | "both") {
            nodes.extend(g.descendants(id).into_iter().filter(|&d| g.node(d).is_some_and(|x| x.ty().is_a("transform"))));
        }
        if matches!(hi.as_str(), "above" | "both") {
            let mut n = g.parent(id);
            while n != NO_NODE {
                nodes.push(n);
                n = g.parent(n);
            }
        }
        for node in nodes {
            if attrs.is_empty() {
                out.extend(keyable_plugs(g, node));
                if want_shape {
                    for &k in g.children(node) {
                        if g.node(k).is_some_and(|x| x.ty().dag && !x.ty().is_a("transform")) {
                            // A shape's visibility isn't one of the channels S keys.
                            out.extend(keyable_plugs(g, k).into_iter().filter(|p| p.attr != crate::transform::DagNode::VISIBILITY as i32));
                        }
                    }
                }
            } else {
                for at in &attrs {
                    let path = format!("{}.{}", g.path(node), at);
                    match g.plug(&path) {
                        Ok(p) => out.extend(split(p)),
                        // Below or above the named object, a channel it doesn't have is passed over.
                        Err(_) if node != id => {}
                        Err(e) => fail!("{cmd}: {e}"),
                    }
                }
            }
        }
    }
    let mut seen = std::collections::HashSet::new();
    out.retain(|p| seen.insert((p.node, p.attr, p.component, p.element)));
    Ok(out)
}

/// A -time flag: a frame, "10:20", "10:" or ":20", or [start, end]. None means all time.
pub(crate) fn time_range(a: &aztek_ops::Args) -> CmdResult<Option<(f64, f64)>> {
    range_flag(a, "time")
}

/// A flag holding a range of times or driver values, as -time and -float do.
pub(crate) fn range_flag(a: &aztek_ops::Args, flag: &str) -> CmdResult<Option<(f64, f64)>> {
    let Some(v) = a.get(flag) else { return Ok(None) };
    let parse = |s: &str| -> CmdResult<(f64, f64)> {
        let s = s.trim().trim_matches(|c| c == '"' || c == '(' || c == ')');
        if let Some((x, y)) = s.split_once(':') {
            let x = if x.trim().is_empty() { f64::NEG_INFINITY } else { x.trim().parse::<f64>().map_err(|_| aztek_ops::CommandError::new(format!("bad time {s}")))? };
            let y = if y.trim().is_empty() { f64::INFINITY } else { y.trim().parse::<f64>().map_err(|_| aztek_ops::CommandError::new(format!("bad time {s}")))? };
            Ok((x, y))
        } else {
            let t = s.parse::<f64>().map_err(|_| aztek_ops::CommandError::new(format!("bad time {s}")))?;
            Ok((t, t))
        }
    };
    Ok(Some(match v {
        Value::Number(n) => {
            let t = n.as_f64().unwrap_or(0.0);
            (t, t)
        }
        Value::String(s) => parse(s)?,
        Value::Array(x) if x.len() == 2 && x.iter().all(Value::is_number) => (x[0].as_f64().unwrap(), x[1].as_f64().unwrap()),
        Value::Array(x) if x.len() == 1 => match &x[0] {
            Value::String(s) => parse(s)?,
            Value::Array(y) if y.len() == 2 => (y[0].as_f64().unwrap_or(0.0), y[1].as_f64().unwrap_or(0.0)),
            Value::Number(n) => (n.as_f64().unwrap_or(0.0), n.as_f64().unwrap_or(0.0)),
            _ => fail!("bad time {v}"),
        },
        _ => fail!("bad time {v}"),
    }))
}

pub(crate) fn in_range(r: Option<(f64, f64)>, t: f64) -> bool {
    r.is_none_or(|(a, b)| t >= a - 1e-6 && t <= b + 1e-6)
}

pub(crate) fn queried(a: &aztek_ops::Args, names: &[&str]) -> bool {
    let q: Vec<&str> = a.get("queried").and_then(Value::as_array).map(|v| v.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
    names.iter().any(|n| q.contains(n) || a.get(n).is_some_and(|v| v == &Value::Bool(true)))
}

pub(crate) fn tangent_flag(a: &aztek_ops::Args, name: &str) -> CmdResult<Option<Tangent>> {
    match a.get(name).and_then(Value::as_str) {
        None => Ok(None),
        Some(s) => match Tangent::parse(s) {
            Some(t) => Ok(Some(t)),
            None => fail!("unknown tangent type '{s}'"),
        },
    }
}

/// The curves of the targets, each with the channel it drives.
pub(crate) fn target_curves(s: &Session, a: &aztek_ops::Args, cmd: &str) -> CmdResult<Vec<(PlugRef, NodeId)>> {
    let g = &s.graph;
    let mut out = Vec::new();
    // The selected keys' curves, with -sl or nothing named.
    if a.named().is_empty() && (a.bool("selected") || a.bool("lastSelected") || a.string("animation", "") == "keys") {
        for (c, _) in &s.key_selection {
            if is_curve(g, *c) && !out.iter().any(|(_, o)| o == c) {
                let p = driven_by(g, *c).first().copied().unwrap_or(PlugRef::new(*c, OUTPUT as i32));
                out.push((p, *c));
            }
        }
        return Ok(out);
    }
    // Curves named directly.
    let names = objects(s, a);
    if names.iter().all(|n| g.find(n).ok().flatten().is_some_and(|id| is_curve(g, id))) && !names.is_empty() {
        for n in names {
            let id = g.find(&n)?.unwrap();
            let p = driven_by(g, id).first().copied().unwrap_or(PlugRef::new(id, OUTPUT as i32));
            out.push((p, id));
        }
        return Ok(out);
    }
    for p in targets(s, a, cmd)? {
        if let Some(c) = curve_of(g, &p) {
            out.push((p, c));
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------------------------------------------------
// Hooks

/// A set on an animated channel: autokey keys it; otherwise the value holds until the time changes, as in the reference.
pub fn install(g: &mut Graph) {
    set_frame(g, range().min);
    g.set_driven_set_hook(Arc::new(|g: &mut Graph, dst: &PlugRef, src: &PlugRef, v: f64| {
        if let Some(edit) = crate::rig::driven_scalar_set(g, dst, src, v) {
            return edit;
        }
        if !holds(g, src) {
            return DrivenEdit::Reject;
        }
        if auto_key() {
            let t = frame(g);
            if key_plug_in(g, dst, t, Some(v), None, None, None, false).unwrap_or(false) {
                return DrivenEdit::Handled;
            }
        }
        DrivenEdit::Hold
    }));
    g.set_driven_vec_set_hook(Arc::new(|g: &mut Graph, dst: &PlugRef, src: &PlugRef, v: glam::DVec3| {
        if let Some(edit) = crate::rig::driven_vec_set(g, dst, src, v) {
            return edit;
        }
        if !crate::anim_layers::is_routing(g, src.node) {
            return DrivenEdit::Reject;
        }
        if auto_key() {
            let t = frame(g);
            let mut done = true;
            let solved = crate::anim_layers::solve_rotate_key(g, &PlugRef::component(dst.node, dst.attr, 0), t, None, Some(v));
            for c in 0..3 {
                let p = PlugRef::component(dst.node, dst.attr, c);
                let (x, raw) = match solved {
                    Some(s) => (s[c as usize], true),
                    None => (v[c as usize], false),
                };
                done &= key_plug_in(g, &p, t, Some(x), None, None, None, raw).unwrap_or(false);
            }
            if done {
                return DrivenEdit::Handled;
            }
        }
        DrivenEdit::Hold
    }));
}

/// What drives a plug but lets a set on it hold until the time changes: curves, and the mute, blend and clip nodes
/// they feed through.
fn holds(g: &Graph, src: &PlugRef) -> bool {
    is_curve(g, src.node) || crate::anim_layers::is_routing(g, src.node) || crate::characters::is_clip_channel(g, src.node)
}

/// Curves left driving nothing after their channels' nodes were deleted, as the reference removes them, with the mute, blend
/// and motion path nodes between.
pub fn orphans(g: &Graph, doomed: &[NodeId]) -> Vec<NodeId> {
    let mut out: Vec<NodeId> = Vec::new();
    let mut gone: Vec<NodeId> = doomed.to_vec();
    let conns = g.connections();
    loop {
        let mut grew = false;
        for (d, s) in &conns {
            if !gone.contains(&d.node) || gone.contains(&s.node) {
                continue;
            }
            let goes = is_curve(g, s.node) || crate::anim_layers::is_routing(g, s.node) || crate::motion_path::is_motion_path(g, s.node) || crate::characters::is_clip_channel(g, s.node);
            // A blend's message to its layer is no reason to stay.
            if goes && conns.iter().filter(|(x, y)| y.node == s.node && !crate::anim_layers::is_layer(g, x.node)).all(|(x, _)| gone.contains(&x.node)) {
                gone.push(s.node);
                out.push(s.node);
                grew = true;
            }
        }
        if !grew {
            return out;
        }
    }
}

/// Key times on these nodes' curves, sorted and unique.
pub fn key_times(g: &Graph, nodes: &[NodeId]) -> Vec<f64> {
    let mut t: Vec<f64> = Vec::new();
    for &n in nodes {
        for (p, c) in curves_on(g, n) {
            if crate::anim_edit::is_driven_curve(g, c) {
                continue;
            }
            t.extend(keys_of(g, c).keys.iter().map(|k| k.t));
            // The active layers' keys show, every selected layer's among them.
            for x in crate::anim_layers::active_curves(g, &p) {
                if x != c {
                    t.extend(keys_of(g, x).keys.iter().map(|k| k.t));
                }
            }
        }
    }
    t.sort_by(f64::total_cmp);
    t.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
    t
}

// ---------------------------------------------------------------------------------------------------------------------
// Commands

pub(crate) type Cmd = CommandDef<Session>;

pub(crate) fn f(name: &str, short: &str, ty: ArgType, doc: &str) -> FlagDef {
    FlagDef::new(name, short, ty, Value::Null, doc)
}

pub(crate) fn query_flags(c: Cmd) -> Cmd {
    c.flag(f("query", "q", ArgType::Bool, "Return the value of the flags named instead of setting them."))
        .flag(f("edit", "e", ArgType::Bool, "Change the flags named."))
        .flag(f("queried", "", ArgType::List, "The flags asked about in query mode (filled in from q=True calls)."))
}

pub fn register(c: &mut Commands<Session>) {
    register_types();
    register_edit_keys(c);
    crate::anim_edit::register(c);
    crate::anim_keys::register(c);
    crate::anim_time::register(c);
    crate::anim_filter::register(c);
    crate::anim_layers::register(c);
    crate::motion_path::register(c);
    crate::characters::register(c);

    c.register(
        query_flags(Cmd::new(
            "autoKeyframe",
            "Auto key: changing an animated channel keys it at the current frame. Returns whether it is on.",
            |_s, a| {
                if !a.bool("query") {
                    if let Some(v) = a.get("state").and_then(Value::as_bool) {
                        AUTO_KEY.store(v, Ordering::Relaxed);
                    }
                }
                Ok(json!(auto_key()))
            },
        ))
        .flag(f("state", "st", ArgType::Bool, "On or off."))
        .undoable(false),
    );

    c.register(
        Cmd::new(
            "findKeyframe",
            "The frame of the next, previous, first or last key on the objects (the selection when none).",
            |s, a| {
                let g = &s.graph;
                let mut nodes = Vec::new();
                for n in objects(s, a) {
                    if let Some(id) = g.find(n.split('.').next().unwrap_or(&n))? {
                        nodes.push(id);
                    }
                }
                let times = key_times(g, &nodes);
                let now = a.get("time").and_then(Value::as_f64).unwrap_or_else(|| frame(g));
                let which = a.string("which", "next");
                let r = match which.as_str() {
                    "first" => times.first().copied(),
                    "last" => times.last().copied(),
                    "previous" => times.iter().rev().find(|&&t| t < now - 1e-6).copied().or(times.last().copied()),
                    _ => times.iter().find(|&&t| t > now + 1e-6).copied().or(times.first().copied()),
                };
                Ok(frame_json(r.unwrap_or(now)))
            },
        )
        .positional("objects")
        .flag(f("which", "w", ArgType::String, "next, previous, first or last.").choices(&["next", "previous", "first", "last"]))
        .flag(f("time", "t", ArgType::Float, "Search from this frame instead of the current one."))
        .undoable(false),
    );

    c.register(
        Cmd::new(
            "keyTimes",
            "Frames that have keys on the objects (the selection when none), for the time slider.",
            |s, a| {
                let g = &s.graph;
                let mut nodes = Vec::new();
                for n in objects(s, a) {
                    if let Some(id) = g.find(n.split('.').next().unwrap_or(&n))? {
                        nodes.push(id);
                    }
                }
                Ok(json!(key_times(g, &nodes)))
            },
        )
        .positional("objects")
        .undoable(false),
    );
}

/// The graph editor's edits on picked keys: [[curve, index], ...] moved, set, retangented or deleted in one undo step.
fn register_edit_keys(c: &mut Commands<Session>) {
    c.register(
        Cmd::new(
            "editKeys",
            "Edit keys picked by curve and index: -dt/-dv move them, -t/-v set them, -itt/-ott change their tangents, -delete \
             removes them. Keys are [[curve, index], ...].",
            |s, a| {
                let Some(list) = a.get("keys").and_then(Value::as_array) else { fail!("editKeys: -keys [[curve, index], ...]") };
                let mut by_curve: Vec<(NodeId, Vec<usize>)> = Vec::new();
                for e in list {
                    let (Some(name), Some(i)) = (e.get(0).and_then(Value::as_str), e.get(1).and_then(Value::as_u64)) else {
                        fail!("editKeys: bad key {e}")
                    };
                    let Some(id) = s.graph.find(name)? else { fail!("editKeys: no curve named {name}") };
                    if !is_curve(&s.graph, id) {
                        fail!("editKeys: {name} is not an animation curve");
                    }
                    match by_curve.iter_mut().find(|(c, _)| *c == id) {
                        Some((_, v)) => v.push(i as usize),
                        None => by_curve.push((id, vec![i as usize])),
                    }
                }
                let dt = a.float("timeOffset", 0.0);
                let dv = a.float("valueOffset", 0.0);
                let (itt, ott) = (tangent_flag(a, "inTangentType")?, tangent_flag(a, "outTangentType")?);
                let mut n = 0;
                for (curve, idx) in by_curve {
                    let mut k = keys_of(&s.graph, curve);
                    if a.bool("delete") {
                        let mut i = 0;
                        k.keys.retain(|_| {
                            let keep = !idx.contains(&i);
                            i += 1;
                            keep
                        });
                        n += idx.len();
                        if k.keys.is_empty() {
                            let drives = driven_by(&s.graph, curve);
                            let vals: Vec<(PlugRef, f64)> = drives.iter().map(|p| (*p, plug_scalar(&mut s.graph, p))).collect();
                            s.graph.delete_node(curve)?;
                            for (p, v) in vals {
                                let _ = s.graph.set_value(p, PlugValue::Float(v));
                            }
                            continue;
                        }
                        set_keys(&mut s.graph, curve, k)?;
                        continue;
                    }
                    let before = k.keys.clone();
                    for &i in &idx {
                        let Some(key) = k.keys.get_mut(i) else { continue };
                        if let Some(b) = a.get("breakdown").and_then(Value::as_bool) {
                            key.breakdown = b;
                        }
                        key.t = a.get("time").and_then(Value::as_f64).unwrap_or(key.t + dt);
                        key.v = a.get("value").and_then(Value::as_f64).unwrap_or(key.v + dv);
                        if let Some(t) = itt {
                            key.itt = t;
                        }
                        if let Some(t) = ott {
                            key.ott = t;
                        }
                        n += 1;
                    }
                    crate::anim_edit::retime_breakdowns(&before, &mut k.keys);
                    // Keys passing others replace them, as in the reference.
                    let moved: Vec<Key> = idx.iter().filter_map(|&i| k.keys.get(i).copied()).collect();
                    let mut rest: Vec<Key> = k.keys.iter().enumerate().filter(|(i, _)| !idx.contains(i)).map(|(_, k)| *k).collect();
                    rest.retain(|r| !moved.iter().any(|m| (m.t - r.t).abs() < 1e-6));
                    rest.extend(moved);
                    k.keys = rest;
                    k.keys.sort_by(|a, b| a.t.total_cmp(&b.t));
                    set_keys(&mut s.graph, curve, k)?;
                }
                Ok(json!(n))
            },
        )
        .flag(f("keys", "k", ArgType::List, "The keys: [[curve, index], ...]."))
        .flag(f("timeOffset", "dt", ArgType::Float, "Frames to move them by."))
        .flag(f("valueOffset", "dv", ArgType::Float, "Value to move them by."))
        .flag(f("time", "t", ArgType::Float, "Frame to put them at."))
        .flag(f("value", "v", ArgType::Float, "Value to give them."))
        .flag(f("inTangentType", "itt", ArgType::String, "Tangent into them.").choices(Tangent::NAMES))
        .flag(f("outTangentType", "ott", ArgType::String, "Tangent out of them.").choices(Tangent::NAMES))
        .flag(f("breakdown", "bd", ArgType::Bool, "Make them breakdowns, or normal keys."))
        .flag(f("delete", "del", ArgType::Bool, "Remove them.")),
    );
}

/// What a curve's values measure, by its type: animCurveTA and animCurveUA hold radians, TL and UL centimetres.
pub fn curve_unit(g: &Graph, c: NodeId) -> aztek_ops::units::Unit {
    match g.node(c).map(|n| n.ty().name.chars().last()) {
        Some(Some('A')) => aztek_ops::units::Unit::Angle,
        Some(Some('L')) => aztek_ops::units::Unit::Linear,
        _ => aztek_ops::units::Unit::None,
    }
}
