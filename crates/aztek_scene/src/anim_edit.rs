//! Key editing beyond the keys themselves: the key clipboard (copyKey, pasteKey), key selection (selectKey), scaling and
//! snapping keys, infinities, baking, and set driven keys (animCurveUL/UA/UU with their input, and blendWeighted to sum
//! several drivers).

use std::any::Any;
use std::sync::Arc;

use aztek_ops::{fail, objects, ArgType, Args, CmdResult, CommandDef, Commands, FlagDef};
use serde_json::{json, Value};

use crate::anim::{self, frame, key_plug, keys_of, set_keys, Key, Keys, Tangent, OUTPUT, POST, PRE};
use crate::graph::Graph;
use crate::node::{types, EvalContext, NodeBehavior, NodeData, NodeType};
use crate::plug::{flags, AttrDef, NodeId, PlugRef, PlugType, PlugValue, NO_NODE};
use crate::session::Session;

type Cmd = CommandDef<Session>;

fn f(name: &str, short: &str, ty: ArgType, doc: &str) -> FlagDef {
    FlagDef::new(name, short, ty, Value::Null, doc)
}

fn modes(c: Cmd) -> Cmd {
    c.flag(f("query", "q", ArgType::Bool, "Query mode."))
        .flag(f("edit", "e", ArgType::Bool, "Edit mode."))
        .flag(f("queried", "", ArgType::List, "The flags asked about in query mode."))
}

/// The key clipboard: one entry per channel copied.
#[derive(Clone, Debug, Default)]
pub struct KeyClipboard {
    pub entries: Vec<ClipEntry>,
    pub start: f64,
    pub end: f64,
}

#[derive(Clone, Debug)]
pub struct ClipEntry {
    pub attr: String,
    pub keys: Vec<Key>,
    pub pre: i64,
    pub post: i64,
}

// ---------------------------------------------------------------------------------------------------------------------
// Driven curves

/// The input of animCurveUL/UA/UU: what a driven key curve evaluates at.
pub const INPUT: usize = anim::BASE_ATTRS;

pub struct DrivenCurve;

impl NodeBehavior for DrivenCurve {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        let at = d.float(INPUT);
        let v = match d.value(anim::KEYS) {
            PlugValue::Data(Some(k)) => k.as_any().downcast_ref::<Keys>().map(|k| k.eval_mode(at, anim::infinity(d.int(PRE)), anim::infinity(d.int(POST)), d.bool(anim::WEIGHTED))),
            _ => None,
        };
        d.set_output(OUTPUT, PlugValue::Float(v.unwrap_or(0.0)));
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub struct BlendWeighted;

impl NodeBehavior for BlendWeighted {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        let sum: f64 = d.elements(0).iter().map(|(_, v)| v.scalar().unwrap_or(0.0)).sum();
        d.set_output(1, PlugValue::Float(sum));
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub fn register_types() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        anim::register_types();
        let base = types::find("animCurve").expect("animCurve");
        for (name, doc) in [
            ("animCurveUL", "Driven keys on a distance channel."),
            ("animCurveUA", "Driven keys on an angle channel."),
            ("animCurveUU", "Driven keys on a unitless channel."),
        ] {
            types::register_type(NodeType {
                name: name.into(),
                base: Some(base.clone()),
                time_dependent: false,
                doc: doc.into(),
                own_attrs: vec![AttrDef {
                    name: "input".into(),
                    short_name: "i".into(),
                    ty: PlugType::Float,
                    default_value: PlugValue::Float(0.0),
                    flags: flags::INPUT | flags::STORABLE,
                    doc: "The driver's value.".into(),
                    ..Default::default()
                }],
                create: Some(Arc::new(|| Box::new(DrivenCurve))),
                ..Default::default()
            })
            .expect(name);
        }
        types::register_type(NodeType {
            name: "blendWeighted".into(),
            icon: "chart-line".into(),
            doc: "Sums its inputs: several driven key curves on one channel.".into(),
            own_attrs: vec![
                AttrDef {
                    name: "input".into(),
                    short_name: "i".into(),
                    ty: PlugType::Float,
                    default_value: PlugValue::Float(0.0),
                    flags: flags::INPUT | flags::ARRAY,
                    doc: "The values summed.".into(),
                    ..Default::default()
                },
                AttrDef {
                    name: "output".into(),
                    short_name: "o".into(),
                    ty: PlugType::Float,
                    default_value: PlugValue::Float(0.0),
                    flags: flags::OUTPUT,
                    doc: "The sum.".into(),
                    ..Default::default()
                },
            ],
            create: Some(Arc::new(|| Box::new(BlendWeighted))),
            ..Default::default()
        })
        .expect("blendWeighted");
    });
}

pub fn is_driven_curve(g: &Graph, id: NodeId) -> bool {
    g.node(id).is_some_and(|n| matches!(n.ty().name.as_str(), "animCurveUL" | "animCurveUA" | "animCurveUU"))
}

/// A driven curve on a plug coming straight or through a blendWeighted, from this driver.
fn driven_curve_for(g: &Graph, driven: &PlugRef, driver: &PlugRef) -> Option<NodeId> {
    let matches = |g: &Graph, c: NodeId| is_driven_curve(g, c) && g.source(&PlugRef::new(c, INPUT as i32)).is_some_and(|s| s == *driver);
    let s = g.source(driven)?;
    if matches(g, s.node) {
        return Some(s.node);
    }
    if g.node(s.node).is_some_and(|n| n.ty().name == "blendWeighted") {
        for e in g.elements(&PlugRef::new(s.node, 0)) {
            if let Some(src) = g.source(&e) {
                if matches(g, src.node) {
                    return Some(src.node);
                }
            }
        }
    }
    None
}

fn scalar(g: &mut Graph, p: &PlugRef) -> f64 {
    g.value(p).ok().and_then(|v| v.scalar()).unwrap_or(0.0)
}

fn split_vec(g: &Graph, p: PlugRef) -> Vec<PlugRef> {
    if g.attr(&p).ty == PlugType::Vec3 && p.component < 0 {
        (0..3).map(|c| PlugRef::component(p.node, p.attr, c)).collect()
    } else {
        vec![p]
    }
}

fn set_driven_key(g: &mut Graph, driven: PlugRef, driver: PlugRef, at: f64, value: f64, itt: Tangent, ott: Tangent) -> CmdResult<NodeId> {
    let curve = match driven_curve_for(g, &driven, &driver) {
        Some(c) => c,
        None => {
            let a = g.attr(&driven);
            let ty = if a.name.starts_with("rotate") { "animCurveUA" } else if a.name.starts_with("translate") { "animCurveUL" } else { "animCurveUU" };
            let t = types::find(ty).unwrap();
            let name = format!("{}_{}", g.node(driven.node).map(|n| n.name.clone()).unwrap_or_default(), anim::channel_name(g, &driven));
            let c = g.create_node(&t, &name, NO_NODE, true)?;
            g.connect(&driver, &PlugRef::new(c, INPUT as i32), false)?;
            let out = PlugRef::new(c, OUTPUT as i32);
            match g.source(&driven) {
                None => {
                    g.connect(&out, &driven, false)?;
                }
                Some(s) if g.node(s.node).is_some_and(|n| n.ty().name == "blendWeighted") => {
                    let i = g.elements(&PlugRef::new(s.node, 0)).iter().map(|e| e.element + 1).max().unwrap_or(0);
                    g.connect(&out, &PlugRef::element(s.node, 0, i), false)?;
                }
                Some(s) if is_driven_curve(g, s.node) => {
                    // A second driver: blendWeighted sums the curves, as the reference makes it.
                    let bt = types::find("blendWeighted").unwrap();
                    let b = g.create_node(&bt, "blendWeighted1", NO_NODE, true)?;
                    g.disconnect(&driven);
                    g.connect(&s, &PlugRef::element(b, 0, 0), false)?;
                    g.connect(&out, &PlugRef::element(b, 0, 1), false)?;
                    g.connect(&PlugRef::new(b, 1), &driven, false)?;
                }
                Some(_) => fail!("setDrivenKeyframe: {} is already driven", g.plug_name(&driven)),
            }
            c
        }
    };
    let mut k = keys_of(g, curve);
    match k.keys.iter_mut().find(|e| (e.t - at).abs() < 1e-6) {
        Some(e) => e.v = value,
        None => k.set(Key::new(at, value, itt, ott)),
    }
    set_keys(g, curve, k)?;
    Ok(curve)
}

fn drivers_of(g: &Graph, driven: &PlugRef) -> Vec<PlugRef> {
    let Some(s) = g.source(driven) else { return Vec::new() };
    let mut curves = Vec::new();
    if is_driven_curve(g, s.node) {
        curves.push(s.node);
    } else if g.node(s.node).is_some_and(|n| n.ty().name == "blendWeighted") {
        for e in g.elements(&PlugRef::new(s.node, 0)) {
            if let Some(x) = g.source(&e) {
                if is_driven_curve(g, x.node) {
                    curves.push(x.node);
                }
            }
        }
    }
    curves.into_iter().filter_map(|c| g.source(&PlugRef::new(c, INPUT as i32))).collect()
}

/// Plugs driven by keys from a driver plug.
fn driven_by_plug(g: &Graph, driver: &PlugRef) -> Vec<PlugRef> {
    let mut out = Vec::new();
    for d in g.destinations(driver) {
        if !is_driven_curve(g, d.node) {
            continue;
        }
        for x in g.destinations(&PlugRef::new(d.node, OUTPUT as i32)) {
            if g.node(x.node).is_some_and(|n| n.ty().name == "blendWeighted") {
                out.extend(g.destinations(&PlugRef::new(x.node, 1)));
            } else {
                out.push(x);
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------------------------------
// Commands

fn asked(a: &Args, name: &str) -> bool {
    a.bool(name) || a.get("queried").and_then(Value::as_array).is_some_and(|q| q.iter().any(|x| x == name))
}

/// Curves named by the usual arguments, or the selected keys' curves with -sl.
fn curves_for(s: &Session, a: &Args, cmd: &str) -> CmdResult<Vec<(PlugRef, NodeId)>> {
    anim::target_curves(s, a, cmd)
}

fn copy_keys(s: &mut Session, a: &Args) -> CmdResult {
    let curves = curves_for(s, a, "copyKey")?;
    let range = anim::time_range(a)?;
    let pick = crate::anim_keys::KeyPick::new(s, a)?;
    let mut clip = KeyClipboard::default();
    let (mut lo, mut hi) = (f64::MAX, f64::MIN);
    let mut n = 0;
    for (p, c) in &curves {
        let k = keys_of(&s.graph, *c);
        let picked: Vec<Key> = k.keys.iter().enumerate().filter(|(i, x)| pick.hits(*c, *i, x)).map(|(_, x)| *x).collect();
        if picked.is_empty() {
            continue;
        }
        n += picked.len();
        for x in &picked {
            lo = lo.min(x.t);
            hi = hi.max(x.t);
        }
        let g = &s.graph;
        let (pre, post) = (g.node(*c).map(|x| x.data().int(PRE)).unwrap_or(0), g.node(*c).map(|x| x.data().int(POST)).unwrap_or(0));
        clip.entries.push(ClipEntry { attr: anim::channel_name(g, p), keys: picked, pre, post });
    }
    if clip.entries.is_empty() {
        fail!("copyKey: no keys to copy");
    }
    clip.start = match range {
        Some((a, _)) if a.is_finite() => a,
        _ => lo,
    };
    clip.end = match range {
        Some((_, b)) if b.is_finite() => b,
        _ => hi,
    };
    s.key_clipboard = clip;
    Ok(json!(n))
}

fn paste_keys(s: &mut Session, a: &Args) -> CmdResult {
    let clip = s.key_clipboard.clone();
    if clip.entries.is_empty() {
        fail!("pasteKey: the clipboard is empty");
    }
    let plugs = anim::targets(s, a, "pasteKey")?;
    if plugs.is_empty() {
        fail!("pasteKey: name or select where to paste");
    }
    let option = a.string("option", "insert");
    let range = anim::time_range(a)?;
    let start = range.map(|r| r.0).filter(|x| x.is_finite()).unwrap_or_else(|| frame(&s.graph));
    let to = a.float("timeOffset", 0.0);
    let vo = a.float("valueOffset", 0.0);
    let copies = a.int("copies", 1).max(1);
    let span = clip.end - clip.start;
    let offset0 = start - clip.start + to;
    let g = &mut s.graph;
    let mut pasted = 0;
    // Matched by channel name when the clip has several, in order otherwise.
    for (i, p) in plugs.iter().enumerate() {
        let name = anim::channel_name(g, p);
        let entry = clip.entries.iter().find(|e| e.attr == name).or_else(|| if clip.entries.len() == 1 { clip.entries.first() } else { None }).or(clip.entries.get(i));
        let Some(entry) = entry else { continue };
        let fresh = anim::curve_of(g, p).is_none();
        if fresh {
            let (t0, v0) = (entry.keys[0].t + offset0, entry.keys[0].v + vo);
            if !key_plug(g, p, t0, Some(v0), Some(entry.keys[0].itt), Some(entry.keys[0].ott))? {
                continue;
            }
        }
        let Some(curve) = anim::curve_of(g, p) else { continue };
        let mut k = keys_of(g, curve);
        if fresh {
            k.keys.clear();
        }
        let total = span.max(0.0);
        let block = total + 1.0;
        match option.as_str() {
            "replaceCompletely" => k.keys.clear(),
            "replace" => {
                let end = offset0 + clip.start + block * copies as f64 - 1.0;
                k.keys.retain(|x| x.t < offset0 + clip.start - 1e-6 || x.t > end + 1e-6);
            }
            "insert" => {
                // Keys at and after the paste time make room for the clip.
                let shift = block * copies as f64;
                for x in k.keys.iter_mut() {
                    if x.t >= start - 1e-6 {
                        x.t += shift;
                    }
                }
            }
            _ => {}
        }
        for c in 0..copies {
            let off = offset0 + c as f64 * block;
            for key in &entry.keys {
                let mut nk = *key;
                nk.t += off;
                nk.v += vo;
                k.keys.retain(|x| (x.t - nk.t).abs() > 1e-6);
                k.keys.push(nk);
                pasted += 1;
            }
        }
        k.keys.sort_by(|x, y| x.t.total_cmp(&y.t));
        set_keys(g, curve, k)?;
    }
    Ok(json!(pasted))
}

pub fn register(c: &mut Commands<Session>) {
    register_types();

    c.register(
        crate::anim_keys::pick_flags(Cmd::new("copyKey", "Copy keys of objects or channels to the key clipboard; -t limits them. Returns how many.", copy_keys))
            .undoable(false)
            .positional("objects, plugs or curves (the selection when none)")
            .flag(f("selected", "sl", ArgType::Bool, "Copy the keys selectKey selected."))
            .flag(f("option", "o", ArgType::String, "keys (the default) or curve."))
            .flag(f("clipboard", "cb", ArgType::String, "Accepted for existing scripts.")),
    );

    c.register(
        Cmd::new(
            "pasteKey",
            "Paste the key clipboard onto channels at -t (the current time by default). -o insert (the default) moves later keys \
             along, replace clears the pasted range, replaceCompletely clears the curve, merge overlays. -to and -vo offset, -cp \
             repeats.",
            paste_keys,
        )
        .positional("objects or plugs (the selection when none)")
        .flag(f("attribute", "at", ArgType::Any, "Channel name(s)."))
        .flag(f("time", "t", ArgType::Any, "Where to paste."))
        .flag(f("option", "o", ArgType::String, "insert, replace, replaceCompletely or merge.").choices(&["insert", "replace", "replaceCompletely", "merge"]))
        .flag(f("timeOffset", "to", ArgType::Float, "Added to the paste time."))
        .flag(f("valueOffset", "vo", ArgType::Float, "Added to the values."))
        .flag(f("copies", "cp", ArgType::Int, "How many times to paste."))
        .flag(f("connect", "c", ArgType::Bool, "Accepted for existing scripts."))
        .flag(f("clipboard", "cb", ArgType::String, "Accepted for existing scripts.")),
    );

    c.register(modes(crate::anim_keys::pick_flags(
        Cmd::new(
            "selectKey",
            "Select keys, for the commands that take -sl. -t limits them; -add, -rm and -tgl change the selection, -r (the default) \
             replaces it; -cl clears it. -uk picks keys off multiples of a time. Returns how many keys it touched. -q -kc counts them.",
            |s, a| {
                if a.bool("clear") {
                    s.key_selection.clear();
                    return Ok(json!(0));
                }
                if a.bool("query") {
                    return Ok(json!(s.key_selection.len()));
                }
                let curves = curves_for(s, a, "selectKey")?;
                let pick = crate::anim_keys::KeyPick::new(s, a)?;
                let unsnapped = a.get("unsnappedKeys").and_then(Value::as_f64);
                let (add, rm, tgl) = (a.bool("addTo"), a.bool("remove"), a.bool("toggle"));
                if !(add || rm || tgl) {
                    s.key_selection.clear();
                }
                let mut n = 0;
                for (_, c) in curves {
                    for (i, k) in keys_of(&s.graph, c).keys.iter().enumerate() {
                        if !pick.hits(c, i, k) {
                            continue;
                        }
                        if let Some(m) = unsnapped {
                            let m = if m > 0.0 { m } else { 1.0 };
                            if ((k.t / m) - (k.t / m).round()).abs() < 1e-6 {
                                continue;
                            }
                        }
                        let has = s.key_selection.iter().position(|x| x.0 == c && (x.1 - k.t).abs() < 1e-6);
                        match (has, rm, tgl) {
                            (Some(i), true, _) | (Some(i), _, true) => {
                                s.key_selection.remove(i);
                            }
                            (None, false, _) => s.key_selection.push((c, k.t)),
                            _ => {}
                        }
                        n += 1;
                    }
                }
                Ok(json!(n))
            },
        ))
        .undoable(false)
        .positional("objects, plugs or curves (the selection when none)")
        .flag(f("addTo", "add", ArgType::Bool, "Add to the selection."))
        .flag(f("remove", "rm", ArgType::Bool, "Remove from the selection."))
        .flag(f("toggle", "tgl", ArgType::Bool, "Toggle in the selection."))
        .flag(f("replace", "r", ArgType::Bool, "Replace the selection (the default)."))
        .flag(f("clear", "cl", ArgType::Bool, "Clear the selection."))
        .flag(f("keyframe", "k", ArgType::Bool, "Select the keys themselves (the default)."))
        .flag(f("inTangent", "it", ArgType::Bool, "Accepted for existing scripts: selects the keys."))
        .flag(f("outTangent", "ot", ArgType::Bool, "Accepted for existing scripts: selects the keys."))
        .flag(f("unsnappedKeys", "uk", ArgType::Float, "Only keys not on a multiple of this time (1 when 0)."))
        .flag(f("selected", "sl", ArgType::Bool, "Work on the keys already selected."))
        .flag(f("keyframeCount", "kc", ArgType::Bool, "Query: how many selected.")),
    ));

    c.register(
        crate::anim_keys::pick_flags(Cmd::new(
            "scaleKey",
            "Scale keys in time about -tp (-ts) and in value about -vp (-vs). -t limits them. Returns how many.",
            |s, a| {
                let curves = curves_for(s, a, "scaleKey")?;
                let pick = crate::anim_keys::KeyPick::new(s, a)?;
                let (ts, tp, vs, vp) = (a.float("timeScale", 1.0), a.float("timePivot", 0.0), a.float("valueScale", 1.0), a.float("valuePivot", 0.0));
                let mut n = 0;
                for (_, c) in curves {
                    let mut k = keys_of(&s.graph, c);
                    let mut i = 0;
                    for key in k.keys.iter_mut() {
                        i += 1;
                        if !pick.hits(c, i - 1, key) {
                            continue;
                        }
                        key.t = tp + (key.t - tp) * ts;
                        key.v = vp + (key.v - vp) * vs;
                        key.in_slope *= vs / ts.abs().max(1e-9);
                        key.out_slope *= vs / ts.abs().max(1e-9);
                        n += 1;
                    }
                    k.keys.sort_by(|x, y| x.t.total_cmp(&y.t));
                    set_keys(&mut s.graph, c, k)?;
                }
                Ok(json!(n))
            },
        ))
        .positional("objects, plugs or curves (the selection when none)")
        .flag(f("selected", "sl", ArgType::Bool, "Scale the keys selectKey selected."))
        .flag(f("timeScale", "ts", ArgType::Float, "Time scale."))
        .flag(f("timePivot", "tp", ArgType::Float, "Time pivot."))
        .flag(f("valueScale", "vs", ArgType::Float, "Value scale."))
        .flag(f("valuePivot", "vp", ArgType::Float, "Value pivot.")),
    );

    c.register(
        crate::anim_keys::pick_flags(Cmd::new(
            "snapKey",
            "Move keys to whole multiples of -tm (1 by default), and to multiples of -vm if given.",
            |s, a| {
                let curves = curves_for(s, a, "snapKey")?;
                let pick = crate::anim_keys::KeyPick::new(s, a)?;
                let tm = a.float("timeMultiple", 1.0);
                let vm = a.float("valueMultiple", 0.0);
                let mut n = 0;
                for (_, c) in curves {
                    let mut k = keys_of(&s.graph, c);
                    let mut i = 0;
                    for key in k.keys.iter_mut() {
                        i += 1;
                        if !pick.hits(c, i - 1, key) {
                            continue;
                        }
                        if tm > 0.0 {
                            key.t = (key.t / tm).round() * tm;
                        }
                        if vm > 0.0 {
                            key.v = (key.v / vm).round() * vm;
                        }
                        n += 1;
                    }
                    k.sort();
                    set_keys(&mut s.graph, c, k)?;
                }
                Ok(json!(n))
            },
        ))
        .positional("objects, plugs or curves (the selection when none)")
        .flag(f("selected", "sl", ArgType::Bool, "Snap the keys selectKey selected."))
        .flag(f("timeMultiple", "tm", ArgType::Float, "Snap time to multiples of this."))
        .flag(f("valueMultiple", "vm", ArgType::Float, "Snap value to multiples of this.")),
    );

    c.register(modes(
        Cmd::new(
            "setInfinity",
            "What curves do before and after their keys: -pri and -poi constant, linear, cycle, cycleRelative or oscillate.",
            |s, a| {
                let curves = curves_for(s, a, "setInfinity")?;
                let index = |n: &str| anim::INFINITY_NAMES.iter().enumerate().find(|(i, x)| **x == n && *i != 2).map(|(i, _)| i as i64);
                if a.bool("query") {
                    let at = if asked(a, "postInfinite") { POST } else { PRE };
                    let v: Vec<Value> = curves
                        .iter()
                        .map(|(_, c)| {
                            let i = s.graph.node(*c).map(|n| n.data().int(at)).unwrap_or(0) as usize;
                            json!(anim::INFINITY_NAMES.get(i).copied().unwrap_or("constant"))
                        })
                        .collect();
                    return Ok(Value::Array(v));
                }
                for (_, c) in curves {
                    for (flag, at) in [("preInfinite", PRE), ("postInfinite", POST)] {
                        if let Some(n) = a.get(flag).and_then(Value::as_str) {
                            let Some(i) = index(n) else { fail!("setInfinity: unknown infinity {n}") };
                            s.graph.set_value(PlugRef::new(c, at as i32), PlugValue::Int(i))?;
                        }
                    }
                }
                Ok(Value::Null)
            },
        )
        .positional("objects, plugs or curves (the selection when none)")
        .flag(f("attribute", "at", ArgType::Any, "Channel name(s)."))
        .flag(f("preInfinite", "pri", ArgType::String, "Before the first key."))
        .flag(f("postInfinite", "poi", ArgType::String, "After the last key.")),
    ));

    c.register(
        Cmd::new(
            "bakeResults",
            "Key channels at every -sb frames over -t with the values they evaluate to, replacing what drove them. -hi below includes \
             the hierarchy, -at the channels; -pok keeps keys outside the range. Returns how many channels.",
            |s, a| {
                let names = objects(s, a);
                let mut plugs = anim::targets(s, a, "bakeResults")?;
                if a.string("hierarchy", "none") == "below" {
                    let g = &s.graph;
                    for n in &names {
                        if let Ok(Some(id)) = g.find(n.split('.').next().unwrap_or(n)) {
                            for d in g.descendants(id) {
                                if g.node(d).is_some_and(|x| x.ty().is_a("transform")) {
                                    plugs.extend(anim::keyable_plugs(g, d));
                                }
                            }
                        }
                    }
                }
                // Only channels something drives, unless named outright.
                let g = &mut s.graph;
                let r = anim::range();
                let (t0, t1) = anim::time_range(a)?.filter(|x| x.0.is_finite() && x.1.is_finite()).unwrap_or((r.min, r.max));
                let by = a.float("sampleBy", 1.0).max(1e-3);
                let now = frame(g);
                let mut samples: Vec<Vec<(f64, f64)>> = vec![Vec::new(); plugs.len()];
                let mut t = t0;
                while t <= t1 + 1e-9 {
                    anim::set_frame(g, t);
                    g.evaluate();
                    for (i, p) in plugs.iter().enumerate() {
                        samples[i].push((t, scalar(g, p)));
                    }
                    t += by;
                }
                anim::set_frame(g, now);
                let keep = a.bool("preserveOutsideKeys");
                let mut done = 0;
                g.open_chunk("Bake");
                let result = (|| -> CmdResult<()> {
                    let layer = crate::anim_layers::prepare_bake(g, &plugs, a)?;
                    for (p, sampled) in plugs.iter().zip(samples) {
                        // A channel in layers: the result goes on the destination, solved under what stays above it.
                        if !crate::anim_layers::chain(g, p).is_empty() || layer.is_some() {
                            let target = layer.or_else(|| crate::anim_layers::base_layer(g));
                            if let Some(t) = target {
                                crate::anim_layers::add_plug(g, t, p)?;
                            }
                            if crate::anim_layers::chain(g, p).is_empty() {
                                continue;
                            }
                            crate::anim_layers::bake_onto(g, p, target, &sampled, keep, (t0, t1))?;
                            done += 1;
                            continue;
                        }
                        // Whatever drove it now gives way to keys.
                        let old = anim::curve_of(g, p);
                        if old.is_none() && g.source(p).is_some() {
                            g.disconnect(p);
                        }
                        if old.is_none() && p.component >= 0 && g.source(&p.whole()).is_some() {
                            // A vector driven whole: split off to per-axis curves.
                            g.disconnect(&p.whole());
                        }
                        if let Some(&(t, v)) = sampled.first() {
                            if !key_plug(g, p, t, Some(v), None, None)? {
                                continue;
                            }
                        }
                        let Some(c) = anim::curve_of(g, p) else { continue };
                        let mut k = keys_of(g, c);
                        if !keep {
                            k.keys.clear();
                        } else {
                            k.keys.retain(|x| x.t < t0 - 1e-6 || x.t > t1 + 1e-6);
                        }
                        let r = anim::range();
                        for (t, v) in sampled {
                            k.keys.push(Key::new(t, v, r.default_in, r.default_out));
                        }
                        k.sort();
                        set_keys(g, c, k)?;
                        done += 1;
                    }
                    Ok(())
                })();
                g.close_chunk();
                result?;
                Ok(json!(done))
            },
        )
        .positional("objects or plugs (the selection when none)")
        .flag(f("attribute", "at", ArgType::Any, "Channel name(s)."))
        .flag(f("time", "t", ArgType::Any, "The range to bake: \"1:100\" (the playback range by default)."))
        .flag(f("sampleBy", "sb", ArgType::Float, "Frames between keys (1)."))
        .flag(f("hierarchy", "hi", ArgType::String, "none or below."))
        .flag(f("preserveOutsideKeys", "pok", ArgType::Bool, "Keep keys outside the baked range."))
        .flag(f("simulation", "sm", ArgType::Bool, "Accepted for existing scripts (always evaluated in time order)."))
        .flag(f("disableImplicitControl", "dic", ArgType::Bool, "Accepted for existing scripts."))
        .flag(f("sparseAnimCurveBake", "sac", ArgType::Bool, "Accepted for existing scripts."))
        .flag(f("destinationLayer", "dl", ArgType::String, "The layer that takes the result."))
        .flag(f("bakeOnOverrideLayer", "bol", ArgType::Bool, "The result goes on a new override layer on top."))
        .flag(f("removeBakedAttributeFromLayer", "ral", ArgType::Bool, "Baked channels leave their layers, rather than those layers being muted."))
        .flag(f("removeBakedAnimFromLayer", "rba", ArgType::Bool, "Baked animation leaves its layers, rather than those layers being muted."))
        .flag(f("resolveWithoutLayer", "rwl", ArgType::Any, "Layers merged in the bake: only these give way to the result.").multi(1))
        .flag(f("minimizeRotation", "mr", ArgType::Bool, "Accepted for existing scripts."))
        .flag(f("controlPoints", "cp", ArgType::Bool, "Accepted for existing scripts."))
        .flag(f("shape", "s", ArgType::Bool, "Accepted for existing scripts.")),
    );

    c.register(modes(
        Cmd::new(
            "setDrivenKeyframe",
            "Set a driven key: at the driver's value (-cd plug, -dv value, or its current one) the channel takes its value (-v, or \
             its current one). -q -dr lists a channel's drivers, -q -dn a driver's driven channels.",
            |s, a| {
                let g = &mut s.graph;
                let names = a.named();
                if a.bool("query") {
                    let n = names.first().cloned().unwrap_or_default();
                    let p = g.plug(&n)?;
                    if asked(a, "driven") {
                        let v: Vec<String> = driven_by_plug(g, &p).iter().map(|x| g.plug_name(x)).collect();
                        return Ok(json!(v));
                    }
                    let v: Vec<String> = drivers_of(g, &p).iter().map(|x| g.plug_name(x)).collect();
                    return Ok(json!(v));
                }
                let Some(driver) = a.get("currentDriver").and_then(Value::as_str) else { fail!("setDrivenKeyframe: -cd driver.attr") };
                let driver = g.plug(driver)?;
                g.evaluate();
                let at = match a.get("driverValue").and_then(Value::as_f64) {
                    Some(v) => v,
                    None => scalar(g, &driver),
                };
                let mut plugs = Vec::new();
                let attrs: Vec<String> = match a.get("attribute") {
                    Some(Value::String(x)) => vec![x.clone()],
                    Some(Value::Array(v)) => v.iter().filter_map(|x| x.as_str().map(str::to_string)).collect(),
                    _ => Vec::new(),
                };
                let names = if names.is_empty() { g.selection().iter().map(|&i| g.path(i)).collect() } else { names };
                for n in &names {
                    if n.contains('.') {
                        plugs.extend(split_vec(g, g.plug(n)?));
                    } else {
                        let Some(id) = g.find(n)? else { fail!("setDrivenKeyframe: no object named {n}") };
                        if attrs.is_empty() {
                            plugs.extend(anim::keyable_plugs(g, id));
                        } else {
                            for at in &attrs {
                                let p = g.plug(&format!("{}.{at}", g.path(id)))?;
                                plugs.extend(split_vec(g, p));
                            }
                        }
                    }
                }
                if plugs.is_empty() {
                    fail!("setDrivenKeyframe: name the driven channels");
                }
                let itt = anim::tangent_flag(a, "inTangentType")?.unwrap_or(Tangent::Clamped);
                let ott = anim::tangent_flag(a, "outTangentType")?.unwrap_or(Tangent::Clamped);
                let mut out = 0;
                g.open_chunk("Set Driven Key");
                let r = (|| -> CmdResult<()> {
                    for p in plugs {
                        let v = match a.get("value").and_then(Value::as_f64) {
                            Some(v) if aztek_ops::units::user_call() => aztek_ops::units::to_stored(g.attr(&p).unit, v),
                            Some(v) => v,
                            None => scalar(g, &p),
                        };
                        set_driven_key(g, p, driver, at, v, itt, ott)?;
                        out += 1;
                    }
                    Ok(())
                })();
                g.close_chunk();
                r?;
                Ok(json!(out))
            },
        )
        .positional("the driven object or channels (the selection when none)")
        .flag(f("currentDriver", "cd", ArgType::String, "The driving attribute."))
        .flag(f("driverValue", "dv", ArgType::Float, "The driver's value at this key (its current one by default)."))
        .flag(f("value", "v", ArgType::Float, "The channel's value at this key (its current one by default)."))
        .flag(f("attribute", "at", ArgType::Any, "Channel name(s)."))
        .flag(f("inTangentType", "itt", ArgType::String, "Tangent into the key.").choices(Tangent::NAMES))
        .flag(f("outTangentType", "ott", ArgType::String, "Tangent out of the key.").choices(Tangent::NAMES))
        .flag(f("driver", "dr", ArgType::Bool, "Query: the channel's drivers."))
        .flag(f("driven", "dn", ArgType::Bool, "Query: the driver's driven channels.")),
    ));
}

/// Moving keys carries the breakdowns between them along in proportion, as the reference does: breakdowns between `moved` keys
/// retime to keep their place. `before` is the curve before the move, `after` the moved one, matched by index.
pub fn retime_breakdowns(before: &[Key], after: &mut [Key]) {
    if before.len() != after.len() {
        return;
    }
    let plain: Vec<usize> = (0..before.len()).filter(|&i| !before[i].breakdown).collect();
    for w in plain.windows(2) {
        let (a, b) = (w[0], w[1]);
        let (t0, t1) = (before[a].t, before[b].t);
        let (n0, n1) = (after[a].t, after[b].t);
        if (t1 - t0).abs() < 1e-9 || ((t0 - n0).abs() < 1e-9 && (t1 - n1).abs() < 1e-9) {
            continue;
        }
        for i in a + 1..b {
            // Only breakdowns the user did not move themselves.
            if (after[i].t - before[i].t).abs() < 1e-9 {
                let s = (before[i].t - t0) / (t1 - t0);
                after[i].t = n0 + (n1 - n0) * s;
            }
        }
    }
}
