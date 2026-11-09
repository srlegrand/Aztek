//! What works on whole curves: quaternion rotation interpolation (rotationInterpolation), filters (filterCurve,
//! simplify), the Graph Editor's buffer (bufferCurve) and stats (keyframeStats); and muting channels (mute).

use std::f64::consts::{PI, TAU};
use std::any::Any;
use std::sync::Arc;

use aztek_ops::{fail, ArgType, Args, CmdResult, Commands};
use glam::{DMat3, DQuat, DVec3};
use serde_json::{json, Value};

use crate::anim::{self, f, query_flags, queried, Cmd, Key, Keys, ROT_AXIS, ROT_INTERP, ROT_KEYS, ROT_ORDER};
use crate::anim_keys::{pick_flags, KeyPick};
use crate::graph::Graph;
use crate::node::{types, EvalContext, NodeBehavior, NodeData, NodeType};
use crate::plug::{flags, AttrDef, NodeId, PlugRef, PlugType, PlugValue, NO_NODE};
use crate::session::Session;
use crate::transform::{euler_near, euler_to_matrix, RotateOrder, Transform};

// ---------------------------------------------------------------------------------------------------------------------
// Quaternion interpolation

fn sibling<'a>(d: &'a NodeData, axis: usize, own: &'a Keys) -> Option<&'a Keys> {
    if axis as i64 == d.int(ROT_AXIS) {
        return Some(own);
    }
    match d.value(ROT_KEYS + axis) {
        PlugValue::Data(Some(k)) => k.as_any().downcast_ref::<Keys>(),
        _ => None,
    }
}

fn quat_log(q: DQuat) -> DVec3 {
    let v = DVec3::new(q.x, q.y, q.z);
    let l = v.length();
    if l < 1e-12 {
        DVec3::ZERO
    } else {
        v / l * l.atan2(q.w)
    }
}

fn quat_exp(v: DVec3) -> DQuat {
    let a = v.length();
    if a < 1e-12 {
        DQuat::IDENTITY
    } else {
        let s = a.sin() / a;
        DQuat::from_xyzw(v.x * s, v.y * s, v.z * s, a.cos())
    }
}

/// A rotation curve's value at frame t under quaternion interpolation, or None outside its keys (the infinities take
/// over) or while a sibling curve is missing.
pub(crate) fn eval_quaternion(own: &Keys, d: &NodeData, t: f64) -> Option<f64> {
    let axis = d.int(ROT_AXIS).clamp(0, 2) as usize;
    let curves = [sibling(d, 0, own)?, sibling(d, 1, own)?, sibling(d, 2, own)?];
    let mut times: Vec<f64> = curves.iter().flat_map(|k| k.keys.iter().map(|x| x.t)).collect();
    times.sort_by(f64::total_cmp);
    times.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
    if times.len() < 2 || t < times[0] || t > times[times.len() - 1] || curves.iter().any(|k| k.keys.is_empty()) {
        return None;
    }
    let order = RotateOrder::from_index(d.int(ROT_ORDER));
    let euler = |t: f64| DVec3::new(curves[0].eval_inside(t, false), curves[1].eval_inside(t, false), curves[2].eval_inside(t, false));
    let quat = |e: DVec3| DQuat::from_mat4(&euler_to_matrix(e, order));
    let i = times.partition_point(|x| *x <= t).clamp(1, times.len() - 1);
    let (t0, t1) = (times[i - 1], times[i]);
    let s = (t - t0) / (t1 - t0);
    let (e0, e1) = (euler(t0), euler(t1));
    let (q0, mut q1) = (quat(e0), quat(e1));
    if q0.dot(q1) < 0.0 {
        q1 = -q1;
    }
    let q = if d.int(ROT_INTERP) == 2 {
        q0.slerp(q1, s)
    } else {
        // Squad: a cubic through the keys, its inner quaternions taken from the keys around.
        let inner = |prev: DQuat, q: DQuat, next: DQuat| {
            let inv = q.inverse();
            let side = |o: DQuat| {
                let o = if q.dot(o) < 0.0 { -o } else { o };
                quat_log(inv * o)
            };
            q * quat_exp((side(prev) + side(next)) * -0.25)
        };
        let qm = if i >= 2 { quat(euler(times[i - 2])) } else { q0 };
        let qp = if i + 1 < times.len() { quat(euler(times[i + 1])) } else { q1 };
        let (a, b) = (inner(qm, q0, q1), inner(q0, q1, qp));
        q0.slerp(q1, s).slerp(a.slerp(b, s), 2.0 * s * (1.0 - s))
    };
    // Stays in the turn of the Euler values around it.
    let reference = e0 + (e1 - e0) * s;
    Some(euler_near(&DMat3::from_quat(q.normalize()), order, reference)[axis])
}

/// The rotate channels among the curves: node, attr and the curves by axis.
fn rotate_groups(g: &Graph, curves: &[(PlugRef, NodeId)]) -> Vec<(NodeId, i32, [Option<NodeId>; 3])> {
    let mut out: Vec<(NodeId, i32, [Option<NodeId>; 3])> = Vec::new();
    for (p, c) in curves {
        if p.component < 0 || p.node == *c || g.attr(p).ty != PlugType::Vec3 || !g.attr(p).name.starts_with("rotate") {
            continue;
        }
        let i = match out.iter().position(|e| e.0 == p.node && e.1 == p.attr) {
            Some(i) => i,
            None => {
                out.push((p.node, p.attr, [None; 3]));
                out.len() - 1
            }
        };
        out[i].2[p.component as usize] = Some(*c);
    }
    out
}

/// The curves for a rotate given as names: curves, objects or plugs. Whole objects mean their rotate channels.
fn rotation_curves(s: &Session, a: &Args, cmd: &str) -> CmdResult<Vec<(PlugRef, NodeId)>> {
    let g = &s.graph;
    let mut out = Vec::new();
    for (p, c) in anim::target_curves(s, a, cmd)? {
        if g.attr(&p).name.starts_with("rotate") && !out.iter().any(|(_, o)| *o == c) {
            out.push((p, c));
        }
    }
    Ok(out)
}

fn rotation_interpolation(s: &mut Session, a: &Args) -> CmdResult {
    let curves = rotation_curves(s, a, "rotationInterpolation")?;
    if a.bool("query") {
        let g = &s.graph;
        return Ok(Value::Array(
            curves
                .iter()
                .map(|(_, c)| json!(anim::ROT_INTERP_NAMES.get(g.node(*c).map(|n| n.data().int(ROT_INTERP)).unwrap_or(0) as usize).copied().unwrap_or("none")))
                .collect(),
        ));
    }
    let Some(mode) = a.get("convert").and_then(Value::as_str) else { fail!("rotationInterpolation: -c names the interpolation") };
    let Some(index) = anim::ROT_INTERP_NAMES.iter().position(|n| *n == mode) else {
        fail!("rotationInterpolation: unknown interpolation {mode}: one of {}", anim::ROT_INTERP_NAMES.join(", "))
    };
    let groups = rotate_groups(&s.graph, &curves);
    let mut n = 0;
    for (node, attr, axes) in groups {
        let g = &mut s.graph;
        let order = g.node(node).map(|x| x.data().int(Transform::ROTATE_ORDER)).unwrap_or(0);
        // All but "none" keep keys together: every axis has a curve and a key where any of them has.
        if index >= 1 {
            let mut times: Vec<f64> = axes.iter().flatten().flat_map(|c| anim::keys_of(g, *c).keys.into_iter().map(|k| k.t)).collect();
            times.sort_by(f64::total_cmp);
            times.dedup_by(|x, y| (*x - *y).abs() < 1e-6);
            for ax in 0..3i32 {
                let p = PlugRef::component(node, attr, ax);
                if axes[ax as usize].is_none() {
                    let v = anim::plug_scalar(g, &p);
                    for &t in &times {
                        anim::key_plug(g, &p, t, Some(v), None, None)?;
                    }
                }
                let Some(c) = anim::curve_of(g, &p) else { continue };
                let mut k = anim::keys_of(g, c);
                let mut changed = false;
                for &t in &times {
                    changed |= k.insert(t);
                }
                if changed {
                    anim::set_keys(g, c, k)?;
                }
            }
        }
        let now: Vec<NodeId> = (0..3).filter_map(|ax| anim::curve_of(g, &PlugRef::component(node, attr, ax))).collect();
        for (ax, &c) in now.iter().enumerate() {
            g.set_value(PlugRef::new(c, ROT_INTERP as i32), PlugValue::Int(index as i64))?;
            g.set_value(PlugRef::new(c, ROT_AXIS as i32), PlugValue::Int(ax as i64))?;
            g.set_value(PlugRef::new(c, ROT_ORDER as i32), PlugValue::Int(order))?;
            for (other_ax, &o) in now.iter().enumerate() {
                let dst = PlugRef::new(c, (ROT_KEYS + other_ax) as i32);
                let copy = if other_ax != ax && index >= 2 && now.len() == 3 { g.node(o).map(|n| n.data().value(anim::KEYS).clone()) } else { None };
                g.set_value(dst, copy.unwrap_or(PlugValue::Data(None)))?;
            }
            n += 1;
        }
    }
    Ok(json!(n))
}

// ---------------------------------------------------------------------------------------------------------------------
// Euler filter

/// Turns a rotation sequence into one without jumps: each frame takes the equivalent Euler triple nearest the one
/// before, by whole turns on each axis, or the other solution for the same rotation.
pub fn euler_filter_values(values: &mut [DVec3], order: RotateOrder) {
    let axes = match order {
        RotateOrder::Xyz => [0, 1, 2],
        RotateOrder::Yzx => [1, 2, 0],
        RotateOrder::Zxy => [2, 0, 1],
        RotateOrder::Xzy => [0, 2, 1],
        RotateOrder::Yxz => [1, 0, 2],
        RotateOrder::Zyx => [2, 1, 0],
    };
    for i in 1..values.len() {
        let prev = values[i - 1];
        let unwrap = |mut e: DVec3| {
            for c in 0..3 {
                e[c] += TAU * ((prev[c] - e[c]) / TAU).round();
            }
            e
        };
        let a = unwrap(values[i]);
        // The same rotation reached the other way round the middle axis: first + 180, middle mirrored, last + 180.
        let mut other = values[i];
        other[axes[0]] += PI;
        other[axes[1]] = PI - other[axes[1]];
        other[axes[2]] += PI;
        let b = unwrap(other);
        let dist = |e: DVec3| (e - prev).length_squared();
        values[i] = if dist(b) + 1e-9 < dist(a) { b } else { a };
    }
}

/// The Euler filter on a rotate's curves. The times are all the axes' keys, so the axes stay consistent; each curve only
/// changes the keys it has.
fn euler_filter(g: &mut Graph, node: NodeId, axes: [Option<NodeId>; 3], window: Option<(f64, f64)>) -> CmdResult<usize> {
    let order = RotateOrder::from_index(g.node(node).map(|x| x.data().int(Transform::ROTATE_ORDER)).unwrap_or(0));
    let keys: Vec<Option<Keys>> = axes.iter().map(|c| c.map(|c| anim::keys_of(g, c))).collect();
    let present = axes.iter().flatten().count();
    let mut times: Vec<f64> = keys.iter().flatten().flat_map(|k| k.keys.iter().map(|x| x.t)).filter(|t| anim::in_range(window, *t)).collect();
    times.sort_by(f64::total_cmp);
    times.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
    if times.len() < 2 {
        return Ok(0);
    }
    let value = |ax: usize, t: f64| keys[ax].as_ref().map(|k| k.eval_inside(t, k.weighted)).unwrap_or(0.0);
    let mut values: Vec<DVec3> = times.iter().map(|&t| DVec3::new(value(0, t), value(1, t), value(2, t))).collect();
    if present == 3 {
        euler_filter_values(&mut values, order);
    } else {
        // Single curves can only lose whole turns.
        for i in 1..values.len() {
            let prev = values[i - 1];
            for c in 0..3 {
                values[i][c] += TAU * ((prev[c] - values[i][c]) / TAU).round();
            }
        }
    }
    for ax in 0..3 {
        let (Some(c), Some(mut k)) = (axes[ax], keys[ax].clone()) else { continue };
        for key in k.keys.iter_mut() {
            if let Some(i) = times.iter().position(|t| (t - key.t).abs() < 1e-6) {
                key.v = values[i][ax];
            }
        }
        anim::set_keys(g, c, k)?;
    }
    Ok(present)
}

// ---------------------------------------------------------------------------------------------------------------------
// Reducing keys

/// Removes keys while the curve stays within `tol` of `original` everywhere. `window` limits which keys may go.
pub(crate) fn reduce(original: &Keys, tol: f64, time_tol: f64, window: Option<(f64, f64)>) -> Keys {
    let mut k = original.clone();
    let weighted = original.weighted;
    let mergeable = |t: f64| anim::in_range(window, t);
    // Keys closer together than the time tolerance become one.
    let mut i = 1;
    while time_tol > 0.0 && i + 1 < k.keys.len() {
        if k.keys[i].t - k.keys[i - 1].t < time_tol && mergeable(k.keys[i].t) {
            k.keys.remove(i);
        } else {
            i += 1;
        }
    }
    loop {
        let mut removed = false;
        let mut i = 1;
        while i + 1 < k.keys.len() {
            if !mergeable(k.keys[i].t) {
                i += 1;
                continue;
            }
            let mut trial = k.clone();
            trial.keys.remove(i);
            // Removing a key changes the tangents of the keys next to it, and so the segments around them too.
            let lo = k.keys[i.saturating_sub(2)].t;
            let hi = k.keys[(i + 2).min(k.keys.len() - 1)].t;
            let steps = (((hi - lo) * 2.0).ceil() as usize).clamp(16, 400);
            let fits = (0..=steps).all(|s| {
                let t = lo + (hi - lo) * s as f64 / steps as f64;
                (trial.eval_inside(t, weighted) - original.eval_inside(t, weighted)).abs() <= tol
            });
            if fits {
                k = trial;
                removed = true;
            } else {
                i += 1;
            }
        }
        if !removed {
            return k;
        }
    }
}

fn simplify(s: &mut Session, a: &Args) -> CmdResult {
    let curves = anim::target_curves(s, a, "simplify")?;
    let pick = KeyPick::new(s, a)?;
    let tol = a.float("valueTolerance", 0.01);
    let time_tol = a.float("timeTolerance", 0.05).max(a.float("floatTolerance", 0.0));
    let mut n = 0;
    for (_, c) in curves {
        let k = anim::keys_of(&s.graph, c);
        let r = reduce(&k, tol, time_tol, pick.range());
        if r.keys.len() != k.keys.len() {
            anim::set_keys(&mut s.graph, c, r)?;
        }
        n += 1;
    }
    Ok(json!(n))
}

// ---------------------------------------------------------------------------------------------------------------------
// filterCurve

/// The curve sampled every `step` frames from lo to hi, hi included.
fn sample(k: &Keys, lo: f64, hi: f64, step: f64) -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    let n = ((hi - lo) / step).round().max(0.0) as usize;
    for i in 0..=n {
        let t = lo + i as f64 * step;
        out.push((t, k.eval_inside(t, k.weighted)));
    }
    if out.last().is_some_and(|x| (x.0 - hi).abs() > 1e-6) {
        out.push((hi, k.eval_inside(hi, k.weighted)));
    }
    out
}

/// Second-order Butterworth low pass run forwards and back, so nothing shifts in time.
pub(crate) fn butterworth(values: &[f64], cutoff: f64, rate: f64) -> Vec<f64> {
    if values.len() < 3 || cutoff <= 0.0 || cutoff >= rate * 0.5 {
        return values.to_vec();
    }
    let c = (std::f64::consts::PI * cutoff / rate).tan();
    let q = std::f64::consts::SQRT_2;
    let d = 1.0 + q * c + c * c;
    let (b0, b1, b2) = (c * c / d, 2.0 * c * c / d, c * c / d);
    let (a1, a2) = (2.0 * (c * c - 1.0) / d, (1.0 - q * c + c * c) / d);
    let pass = |x: &[f64]| -> Vec<f64> {
        // Starts settled on the first value, so the ends don't swing.
        let mut out = Vec::with_capacity(x.len());
        let (mut x1, mut x2, mut y1, mut y2) = (x[0], x[0], x[0], x[0]);
        for &v in x {
            let y = b0 * v + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2;
            x2 = x1;
            x1 = v;
            y2 = y1;
            y1 = y;
            out.push(y);
        }
        out
    };
    let mut r = pass(values);
    r.reverse();
    let mut r = pass(&r);
    r.reverse();
    r
}

pub(crate) fn gaussian(values: &[f64], width: f64) -> Vec<f64> {
    if width <= 0.0 {
        return values.to_vec();
    }
    let reach = (width * 3.0).ceil() as isize;
    let kernel: Vec<f64> = (-reach..=reach).map(|i| (-0.5 * (i as f64 / width).powi(2)).exp()).collect();
    (0..values.len() as isize)
        .map(|i| {
            let (mut sum, mut weight) = (0.0, 0.0);
            for (j, w) in kernel.iter().enumerate() {
                let at = i + j as isize - reach;
                if at >= 0 && (at as usize) < values.len() {
                    sum += values[at as usize] * w;
                    weight += w;
                }
            }
            sum / weight
        })
        .collect()
}

/// Replaces the keys in lo..hi with these samples.
fn replace_in_window(k: &Keys, lo: f64, hi: f64, samples: &[(f64, f64)]) -> Keys {
    let r = anim::range();
    let mut out = Keys { keys: k.keys.iter().filter(|x| x.t < lo - 1e-6 || x.t > hi + 1e-6).copied().collect(), weighted: k.weighted };
    for &(t, v) in samples {
        out.keys.push(Key::new(t, v, r.default_in, r.default_out));
    }
    out.keys.sort_by(|a, b| a.t.total_cmp(&b.t));
    out
}

/// Keys that stick out: the median of the keys around is further from them than `tol`. Their removal closes the spike.
fn remove_peaks(k: &Keys, tol: f64, lo: f64, hi: f64) -> Keys {
    let vals: Vec<f64> = k.keys.iter().map(|x| x.v).collect();
    let mut out = k.clone();
    out.keys.clear();
    for (i, key) in k.keys.iter().enumerate() {
        let keep_end = i == 0 || i + 1 == vals.len() || key.t < lo - 1e-6 || key.t > hi + 1e-6;
        if !keep_end {
            let from = i.saturating_sub(2);
            let to = (i + 3).min(vals.len());
            let mut w: Vec<f64> = (from..to).filter(|j| *j != i).map(|j| vals[j]).collect();
            w.sort_by(f64::total_cmp);
            let median = 0.5 * (w[w.len() / 2] + w[(w.len() - 1) / 2]);
            if (key.v - median).abs() > tol {
                continue;
            }
        }
        out.keys.push(*key);
    }
    out
}

fn filter_curve(s: &mut Session, a: &Args) -> CmdResult {
    let filter = a.string("filter", "euler");
    let curves = anim::target_curves(s, a, "filterCurve")?;
    // The reference's names, and the spellings scripts use for them.
    let kind = match filter.as_str() {
        "peak_remove" | "peakRemove" | "peakRemover" => "peakRemover",
        "key_sync" | "keySync" => "keySync",
        o => o,
    }
    .to_string();
    let selected = a.bool("selectedKeys");
    let window = match (a.get("startTime").and_then(Value::as_f64), a.get("endTime").and_then(Value::as_f64)) {
        (None, None) => None,
        (x, y) => Some((x.unwrap_or(f64::NEG_INFINITY), y.unwrap_or(f64::INFINITY))),
    };
    let fps = anim::fps();
    match kind.as_str() {
        "euler" => {
            let groups = rotate_groups(&s.graph, &curves);
            let mut n = 0;
            for (node, _, axes) in groups {
                n += euler_filter(&mut s.graph, node, axes, window)?;
            }
            return Ok(json!(n));
        }
        "keySync" => {
            let mut times: Vec<f64> = curves.iter().flat_map(|(_, c)| anim::keys_of(&s.graph, *c).keys.into_iter().map(|k| k.t)).filter(|t| anim::in_range(window, *t)).collect();
            times.sort_by(f64::total_cmp);
            times.dedup_by(|x, y| (*x - *y).abs() < 1e-6);
            for (_, c) in &curves {
                let mut k = anim::keys_of(&s.graph, *c);
                let mut changed = false;
                for &t in &times {
                    changed |= k.insert(t);
                }
                if changed {
                    anim::set_keys(&mut s.graph, *c, k)?;
                }
            }
            return Ok(json!(curves.len()));
        }
        "butterworth" | "gaussian" | "keyReducer" | "peakRemover" | "resample" | "simplify" => {}
        o => fail!("filterCurve: unknown filter {o}"),
    }
    let mut n = 0;
    for (_, c) in &curves {
        let k = anim::keys_of(&s.graph, *c);
        if k.keys.is_empty() {
            continue;
        }
        let sel: Vec<f64> = s.key_selection.iter().filter(|x| x.0 == *c).map(|x| x.1).collect();
        let (kl, kh) = (k.keys[0].t, k.keys[k.keys.len() - 1].t);
        let (mut lo, mut hi) = window.unwrap_or((kl, kh));
        if selected && !sel.is_empty() {
            lo = sel.iter().copied().fold(f64::INFINITY, f64::min);
            hi = sel.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        }
        let (lo, hi) = (lo.max(kl), hi.min(kh));
        if hi <= lo {
            continue;
        }
        let new = match kind.as_str() {
            "butterworth" | "gaussian" => {
                // At least a frame apart: the filter never asks for finer keys than the curve can play.
                let step = (fps / a.float("samplingRate", fps).max(1e-6)).max(1.0);
                let samples = sample(&k, lo, hi, step);
                let values: Vec<f64> = samples.iter().map(|x| x.1).collect();
                let filtered = if kind == "butterworth" { butterworth(&values, a.float("cutoffFrequency", 7.0), fps / step) } else { gaussian(&values, a.float("width", 1.0)) };
                let out: Vec<(f64, f64)> = samples.iter().zip(filtered).map(|(x, y)| (x.0, y)).collect();
                replace_in_window(&k, lo, hi, &out)
            }
            "resample" => {
                let mut step = a.float("period", 1.0 / 30.0).max(1e-6) * fps;
                if a.get("keepKeysOnFrame").and_then(Value::as_bool).unwrap_or(false) {
                    step = step.round().max(1.0);
                }
                replace_in_window(&k, lo, hi, &sample(&k, lo, hi, step))
            }
            "peakRemover" => {
                let span = k.keys.iter().map(|x| x.v).fold(f64::NEG_INFINITY, f64::max) - k.keys.iter().map(|x| x.v).fold(f64::INFINITY, f64::min);
                remove_peaks(&k, a.get("tolerance").and_then(Value::as_f64).unwrap_or(0.1 * span), lo, hi)
            }
            "keyReducer" => {
                let span = k.keys.iter().map(|x| x.v).fold(f64::NEG_INFINITY, f64::max) - k.keys.iter().map(|x| x.v).fold(f64::INFINITY, f64::min);
                // -pre is a percentage of the curve's range.
                reduce(&k, a.float("precision", 5.0) * 0.01 * span, 0.0, Some((lo, hi)))
            }
            _ => reduce(&k, a.float("tolerance", 0.05), a.float("timeTolerance", 0.05), Some((lo, hi))),
        };
        anim::set_keys(&mut s.graph, *c, new)?;
        n += 1;
    }
    Ok(json!(n))
}

// ---------------------------------------------------------------------------------------------------------------------
// Mute

pub const MUTE_INPUT: usize = 0;
pub const MUTE_MUTE: usize = 1;
pub const MUTE_HOLD: usize = 2;
pub const MUTE_OUTPUT: usize = 3;

struct MuteNode;

impl NodeBehavior for MuteNode {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        let v = if d.bool(MUTE_MUTE) { d.float(MUTE_HOLD) } else { d.float(MUTE_INPUT) };
        d.set_output(MUTE_OUTPUT, PlugValue::Float(v));
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub fn is_mute(g: &Graph, id: NodeId) -> bool {
    g.node(id).is_some_and(|n| n.ty().name == "mute")
}

pub fn register_types() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let float = |name: &str, short: &str, fl: u32, doc: &str| AttrDef {
            name: name.into(),
            short_name: short.into(),
            ty: PlugType::Float,
            default_value: PlugValue::Float(0.0),
            flags: fl,
            doc: doc.into(),
            ..Default::default()
        };
        types::register_type(NodeType {
            name: "mute".into(),
            icon: "chart-line".into(),
            doc: "Sits between a channel and what drives it: while muted the channel stays at the value it held.".into(),
            own_attrs: vec![
                float("input", "i", flags::INPUT | flags::STORABLE, "What drives the channel."),
                AttrDef {
                    name: "mute".into(),
                    short_name: "m".into(),
                    ty: PlugType::Bool,
                    default_value: PlugValue::Bool(true),
                    flags: flags::INPUT | flags::STORABLE | flags::KEYABLE,
                    doc: "Muted or not.".into(),
                    ..Default::default()
                },
                float("hold", "h", flags::INPUT | flags::STORABLE, "The value kept while muted."),
                float("output", "o", flags::OUTPUT, "What the channel takes."),
            ],
            create: Some(Arc::new(|| Box::new(MuteNode))),
            ..Default::default()
        })
        .expect("mute");
    });
}

fn mute_targets(s: &Session, a: &Args) -> CmdResult<Vec<PlugRef>> {
    anim::targets(s, a, "mute")
}

/// The mute node right before a plug, if any.
fn mute_of(g: &Graph, p: &PlugRef) -> Option<NodeId> {
    g.source(p).filter(|s| is_mute(g, s.node)).map(|s| s.node)
}

fn mute_cmd(s: &mut Session, a: &Args) -> CmdResult {
    let plugs = mute_targets(s, a)?;
    if a.bool("query") {
        let g = &s.graph;
        let any = plugs.iter().any(|p| mute_of(g, p).is_some_and(|m| g.node(m).is_some_and(|n| n.data().bool(MUTE_MUTE))));
        return Ok(json!(any));
    }
    let disable = a.bool("disable");
    let force = a.bool("force");
    let mut out = Vec::new();
    for p in plugs {
        let existing = mute_of(&s.graph, &p);
        if disable {
            let Some(m) = existing else { continue };
            if force {
                // The node goes; what it was holding back drives the channel again.
                let g = &mut s.graph;
                let src = g.source(&PlugRef::new(m, MUTE_INPUT as i32));
                let value = g.value(&PlugRef::new(m, MUTE_INPUT as i32))?.scalar().unwrap_or(0.0);
                g.delete_node(m)?;
                match src {
                    Some(x) => {
                        g.connect(&x, &p, true)?;
                    }
                    None => g.set_value(p, PlugValue::Float(value))?,
                }
            } else {
                s.graph.set_value(PlugRef::new(m, MUTE_MUTE as i32), PlugValue::Bool(false))?;
            }
            continue;
        }
        let g = &mut s.graph;
        let held = anim::plug_scalar(g, &p);
        let m = match existing {
            Some(m) => m,
            None => {
                if p.component >= 0 && g.source(&p.whole()).is_some() {
                    fail!("mute: {} is driven with its whole vector", g.plug_name(&p));
                }
                let t = types::find("mute").unwrap();
                let name = format!("mute_{}_{}", g.node(p.node).map(|n| n.name.clone()).unwrap_or_default(), anim::channel_name(g, &p));
                let m = g.create_node(&t, &name, NO_NODE, true)?;
                match g.source(&p) {
                    Some(src) => {
                        g.disconnect(&p);
                        g.connect(&src, &PlugRef::new(m, MUTE_INPUT as i32), false)?;
                    }
                    None => g.set_value(PlugRef::new(m, MUTE_INPUT as i32), PlugValue::Float(held))?,
                }
                g.connect(&PlugRef::new(m, MUTE_OUTPUT as i32), &p, false)?;
                m
            }
        };
        g.set_value(PlugRef::new(m, MUTE_HOLD as i32), PlugValue::Float(held))?;
        g.set_value(PlugRef::new(m, MUTE_MUTE as i32), PlugValue::Bool(true))?;
        out.push(g.path(m));
    }
    Ok(json!(out))
}

// ---------------------------------------------------------------------------------------------------------------------

fn buffer_curve(s: &mut Session, a: &Args) -> CmdResult {
    let curves = anim::target_curves(s, a, "bufferCurve")?;
    if a.bool("query") || queried(a, &["exists"]) {
        return Ok(json!(curves.iter().any(|(_, c)| s.buffer_curves.contains_key(c))));
    }
    let overwrite = a.get("overwrite").and_then(Value::as_bool).unwrap_or(true);
    let mut n = 0;
    for (_, c) in curves {
        let now = anim::keys_of(&s.graph, c);
        if a.bool("swap") {
            if let Some(old) = s.buffer_curves.insert(c, now) {
                anim::set_keys(&mut s.graph, c, old)?;
            }
            n += 1;
        } else if overwrite || !s.buffer_curves.contains_key(&c) {
            s.buffer_curves.insert(c, now);
            n += 1;
        }
    }
    Ok(json!(n))
}

fn keyframe_stats(s: &Session, a: &Args) -> CmdResult {
    let g = &s.graph;
    let mut keys: Vec<(f64, f64)> = Vec::new();
    for (c, t) in &s.key_selection {
        if anim::is_curve(g, *c) {
            if let Some(k) = anim::keys_of(g, *c).keys.iter().find(|k| (k.t - t).abs() < 1e-6) {
                keys.push((k.t, k.v));
            }
        }
    }
    let same = |f: &dyn Fn(&(f64, f64)) -> f64| keys.first().filter(|x| keys.iter().all(|y| (f(y) - f(x)).abs() < 1e-9)).map(f);
    // The field is blank when the selected keys disagree, as the stats boxes show them.
    let shown = |v: Option<f64>| v.map(|x| x.to_string()).unwrap_or_default();
    Ok(if queried(a, &["valueAnnotation"]) {
        json!(shown(same(&|k| k.1)))
    } else {
        json!(shown(same(&|k| k.0)))
    })
}

pub fn register(c: &mut Commands<Session>) {
    register_types();

    c.register(pick_flags(query_flags(Cmd::new(
        "rotationInterpolation",
        "How a rotate's three curves interpolate: -c none (independent), euler (keys kept together), quaternionSlerp (constant speed along the \
         shortest arc), quaternionSquad or quaternion (cubic through the keys). -q returns each curve's mode.",
        rotation_interpolation,
    )))
    .positional("curves, objects or rotate plugs (the selection when none)")
    .flag(f("convert", "c", ArgType::String, "The interpolation to convert to.").choices(anim::ROT_INTERP_NAMES)));

    c.register(
        pick_flags(Cmd::new(
            "simplify",
            "Remove keys while the curve stays within -vtl of what it was. -t limits it to a range. Returns how many curves it went through.",
            simplify,
        ))
        .positional("objects, plugs or curves (the selection when none)")
        .flag(f("valueTolerance", "vtl", ArgType::Float, "How far the curve may move (0.01)."))
        .flag(f("timeTolerance", "tol", ArgType::Float, "Keys closer than this become one (0.05)."))
        .flag(f("floatTolerance", "ftl", ArgType::Float, "As -tol, for driven curves.")),
    );

    c.register(
        Cmd::new(
            "filterCurve",
            "Filter curves: euler (default: rotations without jumps), butterworth (-cof, -sr), gaussian (-wd), keyReducer (-pre percent), keySync (keys \
             where any curve has one), peakRemover (-tol), resample (-per seconds), simplify (-tol, -tto). -stt/-edt limit it in time, -sk to the selected keys. \
             Returns how many curves it changed.",
            filter_curve,
        )
        .positional("curves, objects or plugs (the selection when none)")
        .flag(f("filter", "f", ArgType::String, "The filter (euler).").choices(&["euler", "butterworth", "gaussian", "keyReducer", "keySync", "key_sync", "peakRemover", "peak_remove", "peakRemove", "resample", "simplify"]))
        .flag(f("attribute", "at", ArgType::Any, "Channel name(s).").multi(1))
        .flag(f("cutoffFrequency", "cof", ArgType::Float, "Butterworth: cutoff in Hz (7)."))
        .flag(f("samplingRate", "sr", ArgType::Float, "Butterworth and gaussian: samples per second, at most one a frame."))
        .flag(f("width", "wd", ArgType::Float, "Gaussian: width in frames (1)."))
        .flag(f("period", "per", ArgType::Float, "Resample: seconds between keys (1/30)."))
        .flag(f("keepKeysOnFrame", "kof", ArgType::Bool, "Resample: keep new keys on whole frames."))
        .flag(f("precision", "pre", ArgType::Float, "Key reducer: percent of the curve's range it may move (5)."))
        .flag(f("tolerance", "tol", ArgType::Float, "Peak remover and simplify: value tolerance."))
        .flag(f("timeTolerance", "tto", ArgType::Float, "Simplify: time tolerance."))
        .flag(f("startTime", "stt", ArgType::Float, "Start of the part filtered."))
        .flag(f("endTime", "edt", ArgType::Float, "End of the part filtered."))
        .flag(f("selectedKeys", "sk", ArgType::Bool, "Only between the selected keys."))
        .flag(f("minTimeStep", "mns", ArgType::Float, "Accepted for existing scripts."))
        .flag(f("maxTimeStep", "mxs", ArgType::Float, "Accepted for existing scripts."))
        .flag(f("positionTolerance", "pot", ArgType::Float, "Accepted for existing scripts."))
        .flag(f("preserveKeyTangent", "pkt", ArgType::String, "Accepted for existing scripts.")),
    );

    c.register(pick_flags(Cmd::new(
        "bufferCurve",
        "The Graph Editor's buffer: keeps a curve's keys (-ov, on by default) to swap back with -sw. -ex (or -q) says whether a curve has one.",
        buffer_curve,
    ))
    .positional("objects, plugs or curves (the selection when none)")
    .flag(f("overwrite", "ov", ArgType::Bool, "Replace what the buffer holds (on by default)."))
    .flag(f("swap", "sw", ArgType::Bool, "Swap the curve and its buffer."))
    .flag(f("exists", "ex", ArgType::Bool, "Whether a curve has a buffer."))
    .flag(f("selected", "sl", ArgType::Bool, "Curves of the keys selectKey selected."))
    .flag(f("query", "q", ArgType::Bool, "With -ex.")));

    c.register(
        query_flags(Cmd::new(
            "keyframeStats",
            "The Graph Editor's stats fields: -q -ta the selected keys' frame, -q -va their value, blank when they differ.",
            |s, a| keyframe_stats(s, a),
        ))
        .positional("the control's name, accepted for existing scripts")
        .flag(f("timeAnnotation", "ta", ArgType::Bool, "Query: the time field."))
        .flag(f("valueAnnotation", "va", ArgType::Bool, "Query: the value field."))
        .flag(f("precision", "pre", ArgType::Int, "Accepted for existing scripts."))
        .flag(f("classicMode", "cm", ArgType::Bool, "Accepted for existing scripts."))
        .flag(f("animEditor", "ae", ArgType::String, "Accepted for existing scripts."))
        .undoable(false),
    );

    for name in ["mute", "muteChannel"] {
        c.register(
            query_flags(Cmd::new(
                name,
                "Mute channels: they stay at their value while what drives them carries on underneath. -d turns the mute off and -d -f removes the mute \
                 node. -q says whether any is muted. Returns the mute nodes.",
                mute_cmd,
            ))
            .positional("plugs or objects (the selection when none)")
            .flag(f("attribute", "at", ArgType::Any, "Channel name(s).").multi(1))
            .flag(f("disable", "d", ArgType::Bool, "Turn the mute off."))
            .flag(f("force", "f", ArgType::Bool, "With -d: delete the mute node."))
            .flag(f("hierarchy", "hi", ArgType::String, "none, below, above or both.").choices(&["none", "below", "above", "both"])),
        );
    }
}
