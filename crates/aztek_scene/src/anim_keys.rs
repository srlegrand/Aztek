//! The key commands proper: setKeyframe, keyframe, cutKey and keyTangent, and what picks the keys they work on.
//!
//! Tangents follow the reference: a tangent is a vector (x in seconds, y in value). An unweighted curve uses only its direction
//! and evaluates as a Hermite cubic; a weighted curve is a cubic Bezier in time and value whose handles are a third of
//! the tangent. Angles are measured in that space, so `-ia 45` is one unit of value per second.

use aztek_ops::{fail, ArgType, Args, CmdResult, Commands};
use serde_json::{json, Value};

use crate::anim::{self, f, query_flags, queried, Cmd, Key, Keys, Tangent, POST, PRE, WEIGHTED};
use crate::plug::{NodeId, PlugRef, PlugValue};
use crate::session::Session;

/// The flags every key command shares for saying which keys.
pub(crate) fn pick_flags(c: Cmd) -> Cmd {
    c.flag(f("attribute", "at", ArgType::Any, "Channel name(s).").multi(1))
        .flag(f("time", "t", ArgType::Any, "A frame or range \"10:20\"."))
        .flag(f("float", "f", ArgType::Any, "A range of driver values on driven key curves, as -t is for time."))
        .flag(f("index", "in", ArgType::Any, "Key index(es) on each curve, or a range \"0:2\"."))
        .flag(f("includeUpperBound", "iub", ArgType::Bool, "Whether a range's end counts (on by default)."))
        .flag(f("hierarchy", "hi", ArgType::String, "none, below, above or both.").choices(&["none", "below", "above", "both"]))
        .flag(f("controlPoints", "cp", ArgType::Bool, "Accepted for existing scripts: control points have no channels of their own here."))
        .flag(f("shape", "s", ArgType::Bool, "Include the channels of the objects' shapes."))
        .flag(f("animation", "an", ArgType::String, "objects (the default), keys or keysOrObjects: keys means the selected keys.").choices(&["objects", "keys", "keysOrObjects"]))
}

/// Which keys of a curve a command works on.
pub(crate) struct KeyPick {
    time: Option<(f64, f64)>,
    float: Option<(f64, f64)>,
    upper: bool,
    index: Option<Vec<usize>>,
    selected: Option<Vec<(NodeId, f64)>>,
}

impl KeyPick {
    pub(crate) fn new(s: &Session, a: &Args) -> CmdResult<KeyPick> {
        let index = match a.get("index") {
            Some(Value::Array(v)) => Some(v.iter().filter_map(Value::as_u64).map(|i| i as usize).collect()),
            Some(Value::Number(n)) => Some(vec![n.as_u64().unwrap_or(0) as usize]),
            Some(Value::String(x)) => match x.split_once(':') {
                Some((lo, hi)) => {
                    let (lo, hi): (usize, usize) = (lo.trim().parse().unwrap_or(0), hi.trim().parse().unwrap_or(usize::MAX - 1));
                    Some((lo..=hi.min(lo + 100_000)).collect())
                }
                None => Some(vec![x.trim().parse().unwrap_or(0)]),
            },
            _ => None,
        };
        let selected = if a.bool("selected") || a.string("animation", "") == "keys" {
            Some(s.key_selection.clone())
        } else if a.bool("lastSelected") {
            Some(s.key_selection.last().copied().into_iter().collect())
        } else {
            None
        };
        Ok(KeyPick {
            time: anim::range_flag(a, "time")?,
            float: anim::range_flag(a, "float")?,
            upper: a.get("includeUpperBound").and_then(Value::as_bool).unwrap_or(true),
            index,
            selected,
        })
    }

    fn in_span(&self, r: Option<(f64, f64)>, t: f64) -> bool {
        let Some((lo, hi)) = r else { return true };
        // A single frame always counts, whatever -iub says.
        let hi_ok = if self.upper || hi - lo < 1e-9 { t <= hi + 1e-6 } else { t < hi - 1e-6 };
        t >= lo - 1e-6 && hi_ok
    }

    pub(crate) fn hits(&self, curve: NodeId, i: usize, k: &Key) -> bool {
        self.in_span(self.time, k.t)
            && self.in_span(self.float, k.t)
            && self.index.as_ref().is_none_or(|ix| ix.contains(&i))
            && self.selected.as_ref().is_none_or(|sel| sel.iter().any(|(c, t)| *c == curve && (t - k.t).abs() < 1e-6))
    }

    pub(crate) fn range(&self) -> Option<(f64, f64)> {
        self.time.or(self.float)
    }
}

fn delete_curve(s: &mut Session, curve: NodeId, plug: PlugRef) -> CmdResult<()> {
    let v = anim::plug_scalar(&mut s.graph, &plug);
    s.graph.delete_node(curve)?;
    // Left with nothing driving it, the channel keeps the value it had; in a layer the blend node carries on.
    if plug.node != curve && s.graph.source(&plug).is_none() && s.graph.source(&plug.whole()).is_none() {
        let _ = s.graph.set_value(plug, PlugValue::Float(v));
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------------
// setKeyframe

fn set_keyframe(s: &mut Session, a: &Args) -> CmdResult {
    let plugs = anim::targets(s, a, "setKeyframe")?;
    let times: Vec<f64> = match a.get("time") {
        Some(Value::Array(v)) => v.iter().filter_map(Value::as_f64).collect(),
        Some(v) => v.as_f64().into_iter().collect(),
        None => vec![anim::frame(&s.graph)],
    };
    let value = a.get("value").and_then(Value::as_f64);
    let (itt, ott) = (anim::tangent_flag(a, "inTangentType")?, anim::tangent_flag(a, "outTangentType")?);
    let respect = a.bool("respectKeyable");
    let insert = a.bool("insert");
    let minimize = a.bool("minimizeRotation");
    let no_resolve = a.bool("noResolve");
    let layer = match a.get("animLayer").and_then(Value::as_str) {
        Some(n) => match s.graph.find(n)? {
            Some(id) if crate::anim_layers::is_layer(&s.graph, id) => Some(id),
            _ => fail!("setKeyframe: no animation layer named {n}"),
        },
        None => None,
    };
    let breakdown = a.get("breakdown").and_then(Value::as_bool);
    let mut n = 0;
    let mut solved: std::collections::HashMap<(NodeId, i32, u64), glam::DVec3> = Default::default();
    for p in plugs {
        if respect && !anim::is_keyable(&s.graph, &p) {
            continue;
        }
        for &t in &times {
            if insert {
                // Keeps the curve as it is; a channel with no curve has nothing to insert into.
                if let Some(c) = anim::curve_of(&s.graph, &p) {
                    let mut k = anim::keys_of(&s.graph, c);
                    if k.insert(t) {
                        if let (Some(b), Some(e)) = (breakdown, k.keys.iter_mut().find(|e| (e.t - t).abs() < 1e-6)) {
                            e.breakdown = b;
                        }
                        anim::set_keys(&mut s.graph, c, k)?;
                        n += 1;
                    }
                }
                continue;
            }
            let mut v = value;
            if minimize && anim::curve_type_for(&s.graph, &p) == "animCurveTA" {
                // The turn nearest the previous key's, so the curve doesn't spin round the long way.
                let now = match v {
                    Some(x) => x,
                    None => anim::plug_scalar(&mut s.graph, &p),
                };
                if let Some(c) = anim::curve_of(&s.graph, &p) {
                    let k = anim::keys_of(&s.graph, c);
                    let before = k.keys.iter().rev().find(|e| e.t < t - 1e-6).or(k.keys.first());
                    if let Some(prev) = before {
                        v = Some(now + 360.0 * ((prev.v - now) / 360.0).round());
                    }
                }
            }
            // An identity key nullifies the layer, and only means anything on a layered channel.
            let identity = if a.bool("identity") { crate::anim_layers::identity_value(&s.graph, &p, layer, t) } else { None };
            let (v, no_resolve) = match identity {
                Some(x) => (Some(x), true),
                None => (v, no_resolve),
            };
            // A layered rotate keyed as it stands is solved once for all its axes.
            let (v, no_resolve) = if v.is_none() && !no_resolve && p.component >= 0 && g_is_rotate(&s.graph, &p) {
                let key = (p.node, p.attr, t.to_bits());
                if !solved.contains_key(&key) {
                    if let Some(x) = crate::anim_layers::solve_rotate_key(&mut s.graph, &p, t, layer, None) {
                        solved.insert(key, x);
                    }
                }
                match solved.get(&key) {
                    Some(x) => (Some(x[p.component as usize]), true),
                    None => (v, no_resolve),
                }
            } else {
                (v, no_resolve)
            };
            // A held (changed but unkeyed) value is what gets keyed at the current frame.
            if anim::key_plug_in(&mut s.graph, &p, t, v, itt, ott, layer, no_resolve)? {
                if let Some(bd) = breakdown {
                    if let Some(c) = anim::curve_of(&s.graph, &p) {
                        let mut k = anim::keys_of(&s.graph, c);
                        if let Some(e) = k.keys.iter_mut().find(|e| (e.t - t).abs() < 1e-6) {
                            e.breakdown = bd;
                        }
                        anim::set_keys(&mut s.graph, c, k)?;
                    }
                }
                n += 1;
            }
        }
    }
    Ok(json!(n))
}

// ---------------------------------------------------------------------------------------------------------------------
// keyframe

fn bool_value(v: &Value) -> Option<bool> {
    v.as_bool().or(v.as_i64().map(|i| i != 0))
}

/// Moves picked keys in time as -option says: over (the default) may pass other keys and replaces any it lands on,
/// move stops short of them, insert moves the keys beyond along too, segmentOver takes the picked keys as one block and
/// moves what it passes to the other side.
fn retime(keys: &mut Vec<Key>, picked: &[usize], new_t: impl Fn(f64) -> f64, option: &str) {
    if picked.is_empty() {
        return;
    }
    let (first, last) = (picked[0], picked[picked.len() - 1]);
    let is_picked = |i: usize| picked.contains(&i);
    match option {
        "move" => {
            let before = (0..first).rev().find(|&i| !is_picked(i)).map(|i| keys[i].t);
            let after = (last + 1..keys.len()).find(|&i| !is_picked(i)).map(|i| keys[i].t);
            for &i in picked {
                let t = new_t(keys[i].t);
                let lo = before.map(|b| b + 1.0).unwrap_or(f64::NEG_INFINITY);
                let hi = after.map(|b| b - 1.0).unwrap_or(f64::INFINITY);
                keys[i].t = if lo <= hi { t.clamp(lo, hi) } else { keys[i].t };
            }
        }
        "insert" => {
            let d = new_t(keys[first].t) - keys[first].t;
            for i in 0..keys.len() {
                if (d >= 0.0 && i >= first) || (d < 0.0 && i <= last) {
                    keys[i].t += d;
                }
            }
        }
        "segmentOver" => {
            let (lo0, hi0) = (keys[first].t, keys[last].t);
            let d = new_t(lo0) - lo0;
            let (lo, hi) = (lo0 + d, hi0 + d);
            let span = hi0 - lo0 + 1.0;
            for i in 0..keys.len() {
                if is_picked(i) {
                    keys[i].t += d;
                } else if keys[i].t >= lo - 1e-6 && keys[i].t <= hi + 1e-6 {
                    keys[i].t += if d >= 0.0 { -span } else { span };
                }
            }
        }
        _ => {
            for &i in picked {
                keys[i].t = new_t(keys[i].t);
            }
        }
    }
    // Moved keys replace what they landed on.
    let moved: Vec<f64> = picked.iter().map(|&i| keys[i].t).collect();
    let mut i = 0;
    keys.retain(|k| {
        let keep = is_picked(i) || !moved.iter().any(|t| (t - k.t).abs() < 1e-6);
        i += 1;
        keep
    });
    keys.sort_by(|a, b| a.t.total_cmp(&b.t));
}

fn keyframe(s: &mut Session, a: &Args) -> CmdResult {
    let curves = anim::target_curves(s, a, "keyframe")?;
    let pick = KeyPick::new(s, a)?;
    if a.bool("query") {
        return keyframe_query(s, a, &curves, &pick);
    }
    let rel = a.bool("relative");
    let tc = a.get("timeChange").and_then(Value::as_f64).or(a.get("floatChange").and_then(Value::as_f64));
    let vc = a.get("valueChange").and_then(Value::as_f64);
    let bd = a.get("breakdown").and_then(bool_value);
    let tick = a.get("tickDrawSpecial").and_then(bool_value);
    let index_to = a.get("indexValue").and_then(Value::as_u64).map(|i| i as usize);
    let option = a.string("option", "over");
    let adjust = a.get("adjustBreakdown").and_then(bool_value).unwrap_or(true);
    let mut n = 0;
    for (_, c) in &curves {
        let mut k = anim::keys_of(&s.graph, *c);
        let before = k.keys.clone();
        let picked: Vec<usize> = k.keys.iter().enumerate().filter(|(i, x)| pick.hits(*c, *i, x)).map(|(i, _)| i).collect();
        for &i in &picked {
            let key = &mut k.keys[i];
            if let Some(b) = bd {
                key.breakdown = b;
            }
            if let Some(b) = tick {
                key.tick = b;
            }
            if let Some(v) = vc {
                key.v = if rel { key.v + v } else { v };
            }
        }
        n += picked.len();
        let target = index_to.and_then(|i| before.get(i).map(|x| x.t));
        if tc.is_some() || target.is_some() {
            let change = tc.or(target).unwrap();
            if adjust && picked.len() < k.keys.len() {
                // Breakdowns between moved keys stretch with them.
                let mut moved = k.keys.clone();
                for &i in &picked {
                    moved[i].t = if rel { moved[i].t + change } else { change };
                }
                crate::anim_edit::retime_breakdowns(&before, &mut moved);
                let carried: Vec<usize> = (0..moved.len()).filter(|i| !picked.contains(i) && (moved[*i].t - k.keys[*i].t).abs() > 1e-9).collect();
                for i in carried {
                    k.keys[i].t = moved[i].t;
                }
            }
            retime(&mut k.keys, &picked, |t| if rel { t + change } else { change }, &option);
            let moved_from: Vec<f64> = picked.iter().map(|&i| before[i].t).collect();
            for sel in s.key_selection.iter_mut().filter(|x| x.0 == *c) {
                if moved_from.iter().any(|t| (t - sel.1).abs() < 1e-6) {
                    sel.1 = if rel { sel.1 + change } else { change };
                }
            }
        }
        anim::set_keys(&mut s.graph, *c, k)?;
    }
    Ok(json!(n))
}

fn keyframe_query(s: &mut Session, a: &Args, curves: &[(PlugRef, NodeId)], pick: &KeyPick) -> CmdResult {
    let g = &s.graph;
    if queried(a, &["name"]) {
        // -sl -n: the curves with selected keys; otherwise every curve asked about.
        let names: Vec<String> = curves
            .iter()
            .filter(|(_, c)| pick.selected.is_none() || anim::keys_of(g, *c).keys.iter().enumerate().any(|(i, k)| pick.hits(*c, i, k)))
            .map(|(_, c)| g.path(*c))
            .collect();
        return Ok(json!(names));
    }
    if queried(a, &["eval"]) {
        let at = pick.range().map(|r| r.0).filter(|t| t.is_finite()).unwrap_or_else(|| anim::frame(g));
        let v: Vec<Value> = curves
            .iter()
            .map(|(_, c)| {
                let d = g.node(*c).map(|n| (n.data().int(PRE), n.data().int(POST))).unwrap_or((0, 0));
                json!(anim::keys_of(g, *c).eval(at, anim::infinity(d.0), anim::infinity(d.1)))
            })
            .collect();
        return Ok(Value::Array(v));
    }
    let mut count = 0;
    let (mut times, mut values, mut both, mut indices, mut breakdowns, mut ticks) = (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for (_, c) in curves {
        for (i, k) in anim::keys_of(g, *c).keys.iter().enumerate() {
            if !pick.hits(*c, i, k) {
                continue;
            }
            count += 1;
            times.push(json!(k.t));
            values.push(json!(k.v));
            both.push(json!(k.t));
            both.push(json!(k.v));
            indices.push(json!(i));
            ticks.push(json!(k.tick));
            if k.breakdown {
                breakdowns.push(json!(k.t));
            }
        }
    }
    Ok(if queried(a, &["keyframeCount"]) {
        json!(count)
    } else if queried(a, &["breakdown"]) {
        Value::Array(breakdowns)
    } else if queried(a, &["tickDrawSpecial"]) {
        Value::Array(ticks)
    } else if queried(a, &["indexValue"]) {
        Value::Array(indices)
    } else if queried(a, &["valueChange"]) && queried(a, &["timeChange"]) {
        Value::Array(both)
    } else if queried(a, &["valueChange"]) {
        Value::Array(values)
    } else {
        Value::Array(times)
    })
}

// ---------------------------------------------------------------------------------------------------------------------
// cutKey

fn cut_key(s: &mut Session, a: &Args) -> CmdResult {
    let curves = anim::target_curves(s, a, "cutKey")?;
    let pick = KeyPick::new(s, a)?;
    let option = a.string("option", "keys");
    if !a.bool("clear") {
        // The reference's cut leaves what it took on the clipboard.
        let mut clip = crate::anim_edit::KeyClipboard::default();
        let (mut lo, mut hi) = (f64::MAX, f64::MIN);
        for (p, c) in &curves {
            let keys: Vec<Key> = anim::keys_of(&s.graph, *c).keys.iter().enumerate().filter(|(i, k)| pick.hits(*c, *i, k)).map(|(_, k)| *k).collect();
            if keys.is_empty() {
                continue;
            }
            for k in &keys {
                lo = lo.min(k.t);
                hi = hi.max(k.t);
            }
            let g = &s.graph;
            let (pre, post) = g.node(*c).map(|n| (n.data().int(PRE), n.data().int(POST))).unwrap_or((0, 0));
            clip.entries.push(crate::anim_edit::ClipEntry { attr: anim::channel_name(g, p), keys, pre, post });
        }
        if !clip.entries.is_empty() {
            clip.start = lo;
            clip.end = hi;
            s.key_clipboard = clip;
        }
    }
    let mut n = 0;
    for (p, c) in curves {
        let mut k = anim::keys_of(&s.graph, c);
        // Removing keys of a curve whose range is given: the range's ends, whether or not there are keys on them.
        let (lo, hi) = pick.range().filter(|r| r.0.is_finite() && r.1.is_finite()).unwrap_or_else(|| {
            let t: Vec<f64> = k.keys.iter().enumerate().filter(|(i, x)| pick.hits(c, *i, x)).map(|(_, x)| x.t).collect();
            (t.first().copied().unwrap_or(0.0), t.last().copied().unwrap_or(0.0))
        });
        if option == "curve" {
            // The rest of the curve keeps its shape: keys hold it at the ends of the range.
            k.insert(lo);
            k.insert(hi);
        }
        let before = k.keys.len();
        let keep_ends = option == "curve";
        let mut i = 0;
        k.keys.retain(|x| {
            let hit = pick.hits(c, i, x) || (pick.range().is_some() && keep_ends && x.t > lo && x.t < hi);
            let end = keep_ends && ((x.t - lo).abs() < 1e-6 || (x.t - hi).abs() < 1e-6);
            i += 1;
            !hit || end
        });
        n += before.saturating_sub(k.keys.len());
        // Collapsing closes the gap the range leaves.
        let span = hi - lo;
        match option.as_str() {
            "curveCollapse" | "areaCollapse" => k.keys.iter_mut().filter(|x| x.t > hi).for_each(|x| x.t -= span),
            "curveConnect" => {
                let v_lo = anim::keys_of(&s.graph, c).eval_inside(lo, k.weighted);
                let v_hi = anim::keys_of(&s.graph, c).eval_inside(hi, k.weighted);
                k.keys.iter_mut().filter(|x| x.t > hi).for_each(|x| {
                    x.t -= span;
                    x.v += v_lo - v_hi;
                });
            }
            _ => {}
        }
        if k.keys.is_empty() {
            delete_curve(s, c, p)?;
        } else {
            anim::set_keys(&mut s.graph, c, k)?;
        }
    }
    s.key_selection.retain(|(c, _)| s.graph.node(*c).is_some());
    Ok(json!(n))
}

// ---------------------------------------------------------------------------------------------------------------------
// keyTangent

/// A key's tangent while being edited: direction as a slope per frame, and length.
#[derive(Clone, Copy)]
struct Tan {
    slope: f64,
    weight: f64,
}

fn tan_of(k: &Keys, i: usize, out: bool) -> Tan {
    let (si, so) = k.slopes(i);
    let (x, y) = k.tangent_xy(i, out);
    Tan { slope: if out { so } else { si }, weight: if k.weighted { x.hypot(y) } else { 1.0 } }
}

/// The angle of a slope per frame: the reference measures in value per second.
fn angle_of(slope: f64) -> f64 {
    (slope * anim::fps()).atan().to_degrees()
}

fn slope_of_angle(deg: f64) -> f64 {
    deg.to_radians().tan() / anim::fps()
}

fn slope_of_xy(x: f64, y: f64) -> f64 {
    y / (x * anim::fps()).max(1e-12)
}

/// Writes one side's edits (angle, then x and y, then weight), absolute or relative.
fn edited(t: Tan, k: &Keys, i: usize, out: bool, flags: [Option<f64>; 4], rel: bool) -> Tan {
    let [angle, x, y, weight] = flags;
    let mut t = t;
    if let Some(d) = angle {
        t.slope = slope_of_angle(if rel { angle_of(t.slope) + d } else { d });
    }
    if x.is_some() || y.is_some() {
        let (cx, cy) = k.tangent_vector(i, out);
        let nx = x.map(|v| if rel { cx + v } else { v }).unwrap_or(cx);
        let ny = y.map(|v| if rel { cy + v } else { v }).unwrap_or(cy);
        t.slope = slope_of_xy(nx, ny);
        if k.weighted {
            t.weight = nx.hypot(ny);
        }
    }
    if let Some(w) = weight {
        t.weight = if rel { t.weight + w } else { w }.max(0.0);
    }
    t
}

fn key_tangent(s: &mut Session, a: &Args) -> CmdResult {
    let (itt, ott) = (anim::tangent_flag(a, "inTangentType")?, anim::tangent_flag(a, "outTangentType")?);
    if a.bool("global") {
        return key_tangent_global(a, itt, ott);
    }
    let curves = anim::target_curves(s, a, "keyTangent")?;
    let pick = KeyPick::new(s, a)?;
    if a.bool("query") {
        return key_tangent_query(s, a, &curves, &pick);
    }
    let rel = a.bool("relative");
    let fl = |n: &str| a.get(n).and_then(Value::as_f64);
    let in_flags = [fl("inAngle"), fl("ix"), fl("iy"), fl("inWeight")];
    let out_flags = [fl("outAngle"), fl("ox"), fl("oy"), fl("outWeight")];
    let weight_lock = a.get("weightLock").and_then(bool_value);
    let lock = a.get("lock").and_then(bool_value);
    let unify = a.bool("unify");
    let weighted = a.get("weightedTangents").and_then(bool_value);
    let mut n = 0;
    for (_, c) in &curves {
        let mut k = anim::keys_of(&s.graph, *c);
        if let Some(w) = weighted {
            if w != k.weighted {
                set_curve_weighted(&mut k, w);
                s.graph.set_value(PlugRef::new(*c, WEIGHTED as i32), PlugValue::Bool(w))?;
            }
        }
        for i in 0..k.keys.len() {
            if !pick.hits(*c, i, &k.keys[i]) {
                continue;
            }
            n += 1;
            if let Some(t) = itt {
                if t == Tangent::Fixed {
                    let tan = tan_of(&k, i, false);
                    k.keys[i].in_slope = tan.slope;
                    k.keys[i].in_weight = tan.weight;
                }
                k.keys[i].itt = t;
            }
            if let Some(t) = ott {
                if t == Tangent::Fixed {
                    let tan = tan_of(&k, i, true);
                    k.keys[i].out_slope = tan.slope;
                    k.keys[i].out_weight = tan.weight;
                }
                k.keys[i].ott = t;
            }
            if let Some(b) = weight_lock {
                k.keys[i].weight_lock = b;
            }
            if let Some(b) = lock {
                k.keys[i].tan_lock = b;
            }
            let edit_in = in_flags.iter().any(Option::is_some);
            let edit_out = out_flags.iter().any(Option::is_some);
            if edit_in || edit_out {
                let (ti, to) = (tan_of(&k, i, false), tan_of(&k, i, true));
                let new_in = edited(ti, &k, i, false, in_flags, rel);
                let new_out = edited(to, &k, i, true, out_flags, rel);
                let locked = k.keys[i].tan_lock;
                // Locked tangents turn together; only the side named changes length.
                let (slope_in, slope_out) = match (edit_in, edit_out, locked) {
                    (true, false, true) => (new_in.slope, new_in.slope),
                    (false, true, true) => (new_out.slope, new_out.slope),
                    _ => (new_in.slope, new_out.slope),
                };
                let key = &mut k.keys[i];
                if edit_in || locked {
                    key.itt = Tangent::Fixed;
                    key.in_slope = slope_in;
                    key.in_weight = new_in.weight;
                }
                if edit_out || locked {
                    key.ott = Tangent::Fixed;
                    key.out_slope = slope_out;
                    key.out_weight = new_out.weight;
                }
            }
            if unify {
                let (ti, to) = (tan_of(&k, i, false), tan_of(&k, i, true));
                let mid = slope_of_angle(0.5 * (angle_of(ti.slope) + angle_of(to.slope)));
                let key = &mut k.keys[i];
                key.itt = Tangent::Fixed;
                key.ott = Tangent::Fixed;
                key.in_slope = mid;
                key.out_slope = mid;
                key.in_weight = ti.weight;
                key.out_weight = to.weight;
                key.tan_lock = true;
            }
        }
        anim::set_keys(&mut s.graph, *c, k)?;
    }
    Ok(json!(n))
}

/// Turns a curve weighted or back. Fixed tangents take a length reaching the next key, so the shape stays.
fn set_curve_weighted(k: &mut Keys, on: bool) {
    if on {
        let f = anim::fps();
        for i in 0..k.keys.len() {
            let (si, so) = k.slopes(i);
            let reach = |o: Option<&Key>, t: f64| o.map(|o| (o.t - t).abs()).unwrap_or(f);
            let t = k.keys[i].t;
            let prev = i.checked_sub(1).map(|j| k.keys[j]);
            let next = k.keys.get(i + 1).copied();
            let (di, do_) = (reach(prev.as_ref().or(next.as_ref()), t), reach(next.as_ref().or(prev.as_ref()), t));
            let key = &mut k.keys[i];
            if key.itt == Tangent::Fixed {
                key.in_weight = (di / f).hypot(si * di);
            }
            if key.ott == Tangent::Fixed {
                key.out_weight = (do_ / f).hypot(so * do_);
            }
        }
    }
    k.weighted = on;
}

fn key_tangent_global(a: &Args, itt: Option<Tangent>, ott: Option<Tangent>) -> CmdResult {
    if a.bool("query") {
        let r = anim::range();
        if queried(a, &["weightedTangents"]) {
            return Ok(json!(r.default_weighted));
        }
        return Ok(json!(if queried(a, &["outTangentType"]) { r.default_out.name() } else { r.default_in.name() }));
    }
    let weighted = a.get("weightedTangents").and_then(bool_value);
    anim::set_range(|r| {
        if let Some(t) = itt {
            r.default_in = t;
        }
        if let Some(t) = ott {
            r.default_out = t;
        }
        if let Some(w) = weighted {
            r.default_weighted = w;
        }
    });
    Ok(Value::Null)
}

const TANGENT_QUERIES: &[&str] = &["inAngle", "outAngle", "inWeight", "outWeight", "ix", "iy", "ox", "oy", "inTangentType", "outTangentType", "lock", "weightLock"];

fn key_tangent_query(s: &Session, a: &Args, curves: &[(PlugRef, NodeId)], pick: &KeyPick) -> CmdResult {
    let g = &s.graph;
    if queried(a, &["weightedTangents"]) {
        return Ok(Value::Array(curves.iter().map(|(_, c)| json!(anim::keys_of(g, *c).weighted)).collect()));
    }
    // Asked flags in the order asked; each key answers all of them in turn.
    let mut asked: Vec<String> = a.get("queried").and_then(Value::as_array).map(|v| v.iter().filter_map(|x| x.as_str().map(str::to_string)).filter(|x| TANGENT_QUERIES.contains(&x.as_str())).collect()).unwrap_or_default();
    for q in TANGENT_QUERIES {
        if !asked.iter().any(|x| x == q) && a.get(q).is_some_and(|v| v == &Value::Bool(true)) {
            asked.push(q.to_string());
        }
    }
    if asked.is_empty() {
        asked.push("inTangentType".into());
    }
    let mut out = Vec::new();
    for (_, c) in curves {
        let k = anim::keys_of(g, *c);
        for (i, key) in k.keys.iter().enumerate() {
            if !pick.hits(*c, i, key) {
                continue;
            }
            let (si, so) = k.slopes(i);
            let (ix, iy) = k.tangent_vector(i, false);
            let (ox, oy) = k.tangent_vector(i, true);
            for q in &asked {
                out.push(match q.as_str() {
                    "inAngle" => json!(angle_of(si)),
                    "outAngle" => json!(angle_of(so)),
                    "inWeight" => json!(ix.hypot(iy)),
                    "outWeight" => json!(ox.hypot(oy)),
                    "ix" => json!(ix),
                    "iy" => json!(iy),
                    "ox" => json!(ox),
                    "oy" => json!(oy),
                    "outTangentType" => json!(key.ott.name()),
                    "lock" => json!(key.tan_lock),
                    "weightLock" => json!(key.weight_lock),
                    _ => json!(key.itt.name()),
                });
            }
        }
    }
    Ok(Value::Array(out))
}

// ---------------------------------------------------------------------------------------------------------------------

pub fn register(c: &mut Commands<Session>) {
    c.register(
        Cmd::new(
            "setKeyframe",
            "Key channels at the current frame (or -t). Objects key all their keyable channels unless -at names some. -i inserts a \
             key that keeps the curve's shape. -al names the animation layer. Returns the number of keys set.",
            set_keyframe,
        )
        .positional("objects or object.attribute plugs (the selection when none)")
        .flag(f("attribute", "at", ArgType::Any, "Channel name(s) to key: tx, rotateY...").multi(1))
        .flag(f("time", "t", ArgType::Any, "Frame(s) to key at.").multi(1))
        .flag(f("float", "f", ArgType::Any, "Accepted for existing scripts: driven keys are set with setDrivenKeyframe."))
        .flag(f("value", "v", ArgType::Float, "Value to key instead of the current one."))
        .flag(f("inTangentType", "itt", ArgType::String, "Tangent into the key.").choices(Tangent::NAMES))
        .flag(f("outTangentType", "ott", ArgType::String, "Tangent out of the key.").choices(Tangent::NAMES))
        .flag(f("breakdown", "bd", ArgType::Bool, "The key is a breakdown: it keeps its place between the keys around it."))
        .flag(f("insert", "i", ArgType::Bool, "Insert a key on the curve, keeping its shape. The tangents around it become fixed."))
        .flag(f("hierarchy", "hi", ArgType::String, "none, below, above or both.").choices(&["none", "below", "above", "both"]))
        .flag(f("minimizeRotation", "mr", ArgType::Bool, "Key rotations as the turn nearest the previous key."))
        .flag(f("noResolve", "nr", ArgType::Bool, "With -v: put the value straight on the layer's curve."))
        .flag(f("respectKeyable", "rk", ArgType::Bool, "Skip channels that aren't keyable."))
        .flag(f("controlPoints", "cp", ArgType::Bool, "Accepted for existing scripts: control points have no channels of their own here."))
        .flag(f("shape", "s", ArgType::Bool, "Key the shapes' keyable channels too."))
        .flag(f("animLayer", "al", ArgType::String, "The layer to key. Ignored for channels not in it, as in the reference."))
        .flag(f("dirtyDG", "dd", ArgType::Bool, "Accepted for existing scripts: keying always dirties what it drives."))
        .flag(f("identity", "id", ArgType::Bool, "An identity key: one that nullifies the layer here. Only on channels driven by layers.")),
    );

    c.register(pick_flags(query_flags(Cmd::new(
        "keyframe",
        "Query or edit keys. -q -kc counts them, -q -tc lists their frames, -q -vc their values, -q -n the curves, -q -iv their \
         indices, -q -ev evaluates the curves. -tc/-vc (with -r for relative) move the keys picked by -t, -f, -in or -sl.",
        keyframe,
    )))
    .positional("objects, plugs or curves (the selection when none)")
    .flag(f("floatChange", "fc", ArgType::Float, "Driven keys: the driver value, as -tc for time keys."))
    .flag(f("breakdown", "bd", ArgType::Bool, "Query: the breakdown keys' frames. Edit: make keys breakdowns, or not."))
    .flag(f("keyframeCount", "kc", ArgType::Bool, "Query: how many keys."))
    .flag(f("timeChange", "tc", ArgType::Float, "Query: the keys' frames. Edit: their new frame (or offset with -r)."))
    .flag(f("valueChange", "vc", ArgType::Float, "Query: the keys' values. Edit: their new value (or offset with -r)."))
    .flag(f("indexValue", "iv", ArgType::Int, "Query: the keys' indices."))
    .flag(f("relative", "r", ArgType::Bool, "Edits are offsets."))
    .flag(f("absolute", "a", ArgType::Bool, "Edits are new values (the default)."))
    .flag(f("option", "o", ArgType::String, "How keys move past others: over (the default), move, insert or segmentOver.").choices(&["over", "move", "insert", "segmentOver"]))
    .flag(f("name", "n", ArgType::Bool, "Query: the curves' names."))
    .flag(f("eval", "ev", ArgType::Bool, "Query: the curves evaluated at -t (the current frame by default)."))
    .flag(f("selected", "sl", ArgType::Bool, "Work on the keys selectKey selected."))
    .flag(f("lastSelected", "lsl", ArgType::Bool, "Work on the key selected last."))
    .flag(f("adjustBreakdown", "abd", ArgType::Bool, "Breakdowns keep their place between keys that move (on by default)."))
    .flag(f("tickDrawSpecial", "tds", ArgType::Bool, "Query or set the special tick colour of keys.")));

    c.register(
        pick_flags(Cmd::new(
            "cutKey",
            "Delete keys in -t (or all of them) from objects or channels. Curves left with no keys are deleted, leaving the channel \
             at its value. -o curve keeps the rest of the curve's shape; curveCollapse, curveConnect and areaCollapse close the gap.",
            cut_key,
        ))
        .positional("objects, plugs or curves (the selection when none)")
        .flag(f("clear", "cl", ArgType::Bool, "Delete without copying to the clipboard."))
        .flag(f("selected", "sl", ArgType::Bool, "Delete the keys selectKey selected."))
        .flag(f("option", "o", ArgType::String, "keys (the default), curve, curveCollapse, curveConnect or areaCollapse.").choices(&["keys", "curve", "curveCollapse", "curveConnect", "areaCollapse"])),
    );

    c.register(pick_flags(query_flags(Cmd::new(
        "keyTangent",
        "Tangents of keys: types (-itt, -ott), angles in degrees (-ia, -oa), vectors (-ix, -iy, -ox, -oy), weights (-iw, -ow), locks (-l, -wl), \
         weighted curves (-wt), or with -g the defaults for new keys. -r makes edits offsets. In query mode each key answers the flags asked.",
        key_tangent,
    )))
    .positional("objects, plugs or curves (the selection when none)")
    .flag(f("inTangentType", "itt", ArgType::String, "Tangent into the keys.").choices(Tangent::NAMES))
    .flag(f("outTangentType", "ott", ArgType::String, "Tangent out of the keys.").choices(Tangent::NAMES))
    .flag(f("inAngle", "ia", ArgType::Float, "In tangent angle, degrees, in value per second (makes it fixed)."))
    .flag(f("outAngle", "oa", ArgType::Float, "Out tangent angle, degrees, in value per second (makes it fixed)."))
    .flag(f("inWeight", "iw", ArgType::Float, "Length of the in tangent, on weighted curves."))
    .flag(f("outWeight", "ow", ArgType::Float, "Length of the out tangent, on weighted curves."))
    .flag(f("ix", "ix", ArgType::Float, "In tangent x, seconds."))
    .flag(f("iy", "iy", ArgType::Float, "In tangent y, value."))
    .flag(f("ox", "ox", ArgType::Float, "Out tangent x, seconds."))
    .flag(f("oy", "oy", ArgType::Float, "Out tangent y, value."))
    .flag(f("lock", "l", ArgType::Bool, "In and out tangents move together (on by default)."))
    .flag(f("weightLock", "wl", ArgType::Bool, "The weight stays when the handle is dragged (on by default)."))
    .flag(f("weightedTangents", "wt", ArgType::Bool, "Make curves weighted or not; with -g, whether new ones are."))
    .flag(f("unify", "uni", ArgType::Bool, "Give a key's in and out tangents one angle, and lock them."))
    .flag(f("relative", "r", ArgType::Bool, "Edits add to what is there."))
    .flag(f("absolute", "a", ArgType::Bool, "Edits are new values (the default)."))
    .flag(f("selected", "sl", ArgType::Bool, "Work on the keys selectKey selected."))
    .flag(f("stepAttributes", "sa", ArgType::Bool, "Accepted for existing scripts: step tangents apply to any channel."))
    .flag(f("global", "g", ArgType::Bool, "The default tangents for new keys.")));
}

fn g_is_rotate(g: &crate::graph::Graph, p: &PlugRef) -> bool {
    g.attr(p).name == "rotate"
}
