//! Smaller animation commands: timeWarp, setKeyPath, selectKeyframe, tickDrawSpecial, recordAttr, bakeSimulation,
//! animDisplay, keyingGroup, characterMap. bakeClip and clipMatching are in characters.rs, beside what they read.
//!
//! Differs from the reference: recordAttr marks attributes for scripts to read back; nothing records them, as there's no `record`.
//! A time warp's frames interpolate linearly, whatever -it asks for.

use std::any::Any;
use std::sync::Arc;

use aztek_geo::nurbs::NurbsCurve;
use aztek_ops::{fail, objects, ArgType, Args, CmdResult, CommandDef, Commands};
use glam::DVec3;
use serde_json::{json, Value};

use crate::anim;
use crate::ref_core::{asked, f};
use crate::node::{types, EvalContext, NodeBehavior, NodeData, NodeType};
use crate::plug::{flags, AttrDef, NodeId, PlugRef, PlugType, PlugValue, NO_NODE};
use crate::rig::{find, plug};
use crate::session::Session;
use crate::system_cmds::{q_flags, setting_store, Def};
use crate::transform::Transform;

type Cmd = CommandDef<Session>;
use ArgType::{Bool as B, Int as I, String as S};

// ---------------------------------------------------------------------------------------------------------------------
// timeWarp

const TW_APPLY: usize = 0;
const TW_ORIG: usize = 1;
const TW_WARPED: usize = 2;
const TW_OUT: usize = 4;

struct TimeWarpNode;

impl NodeBehavior for TimeWarpNode {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        let list: Vec<Value> = if d.bool(TW_APPLY) {
            let warped = d.elements(TW_WARPED).to_vec();
            d.elements(TW_ORIG)
                .iter()
                .filter_map(|(i, o)| {
                    let w = warped.iter().find(|(j, _)| j == i).map(|(_, x)| x)?;
                    match (o, w) {
                        (PlugValue::Float(a), PlugValue::Float(b)) => Some(json!([a, b])),
                        _ => None,
                    }
                })
                .collect()
        } else {
            Vec::new()
        };
        d.set_output(TW_OUT, PlugValue::Data(Some(Arc::new(crate::rig::JsonData(Value::Array(list))))));
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// The frame a curve reads at scene frame t: what the warp shows at t. Frames it marks move from orig to warped, so the
/// key at orig plays at warped. Past either end the offset of that end carries on.
pub fn warped_frame(warp: &Value, t: f64) -> f64 {
    let Some(list) = warp.as_array().filter(|l| l.len() >= 2) else { return t };
    let mut pts: Vec<(f64, f64)> = list.iter().filter_map(|p| Some((p.get(0)?.as_f64()?, p.get(1)?.as_f64()?))).collect();
    if pts.len() < 2 {
        return t;
    }
    pts.sort_by(|a, b| a.1.total_cmp(&b.1));
    let (first, last) = (pts[0], pts[pts.len() - 1]);
    if t <= first.1 {
        return t - first.1 + first.0;
    }
    if t >= last.1 {
        return t - last.1 + last.0;
    }
    for w in pts.windows(2) {
        let ((o0, w0), (o1, w1)) = (w[0], w[1]);
        if t <= w1 && w1 > w0 {
            return o0 + (o1 - o0) * (t - w0) / (w1 - w0);
        }
    }
    t
}

fn is_warp(g: &crate::graph::Graph, id: NodeId) -> bool {
    g.node(id).is_some_and(|n| n.ty().name == "timeWarp")
}

fn frames_of(g: &crate::graph::Graph, tw: NodeId, attr: usize) -> Vec<f64> {
    g.node(tw).unwrap().elements(attr).iter().filter_map(|(_, v)| if let PlugValue::Float(x) = v { Some(*x) } else { None }).collect()
}

fn set_frames(g: &mut crate::graph::Graph, tw: NodeId, attr: usize, list: &[f64]) -> CmdResult<()> {
    g.set_elements(&PlugRef::new(tw, attr as i32), &list.iter().map(|&x| PlugValue::Float(x)).collect::<Vec<_>>())?;
    Ok(())
}

fn time_warp(s: &mut Session, a: &Args) -> CmdResult {
    let names = a.named();
    let g = &mut s.graph;
    let warp_named = |g: &crate::graph::Graph, list: &[String]| -> CmdResult<NodeId> {
        for n in list {
            let id = find(g, n, "timeWarp")?;
            if is_warp(g, id) {
                return Ok(id);
            }
        }
        fail!("timeWarp: name a timeWarp node")
    };
    if a.bool("query") {
        let tw = warp_named(g, &names)?;
        if asked(a, "frame") {
            return Ok(json!(frames_of(g, tw, TW_ORIG)));
        }
        fail!("timeWarp: query what?");
    }
    if a.bool("edit") {
        let tw = warp_named(g, &names)?;
        let (mut orig, mut warped) = (frames_of(g, tw, TW_ORIG), frames_of(g, tw, TW_WARPED));
        if let Some(fr) = a.get("frame") {
            for x in nums(fr) {
                orig.push(x);
                warped.push(x);
            }
            let mut pairs: Vec<(f64, f64)> = orig.iter().copied().zip(warped.iter().copied()).collect();
            pairs.sort_by(|x, y| x.0.total_cmp(&y.0));
            orig = pairs.iter().map(|p| p.0).collect();
            warped = pairs.iter().map(|p| p.1).collect();
        }
        if let Some(l) = a.get("moveFrame") {
            let v = nums(l);
            let [i, to] = v.as_slice() else { fail!("timeWarp: -mf takes an index and a frame") };
            let Some(w) = warped.get_mut(*i as usize) else { fail!("timeWarp: no warp frame {i}") };
            *w = *to;
        }
        if a.has("deleteFrame") {
            let i = a.int("deleteFrame", -1) as usize;
            if i >= orig.len() {
                fail!("timeWarp: no warp frame {i}");
            }
            orig.remove(i);
            warped.remove(i);
        }
        g.open_chunk("timeWarp");
        let r = set_frames(g, tw, TW_ORIG, &orig).and_then(|_| set_frames(g, tw, TW_WARPED, &warped));
        g.close_chunk();
        r?;
        return Ok(json!(g.path(tw)));
    }
    // The curves: named, keyed on what's named, or with -g every one in the scene.
    let mut curves: Vec<NodeId> = Vec::new();
    if a.bool("global") {
        curves.extend(g.nodes().into_iter().filter(|&n| anim::is_curve(g, n)));
    }
    for n in &names {
        let id = find(g, n, "timeWarp")?;
        if anim::is_curve(g, id) {
            curves.push(id);
        } else {
            curves.extend(anim::curves_on(g, id).into_iter().map(|(_, c)| c));
        }
    }
    if curves.is_empty() {
        fail!("timeWarp: name animated objects or curves");
    }
    let frames: Vec<f64> = a.get("frame").map(nums).unwrap_or_default();
    let frames = if frames.is_empty() {
        let r = anim::range();
        vec![r.min, r.max]
    } else {
        frames
    };
    let t = types::find("timeWarp").unwrap();
    g.open_chunk("timeWarp");
    let r = (|| -> CmdResult<NodeId> {
        let tw = g.create_node(&t, &a.string("name", "timeWarp1"), NO_NODE, true)?;
        set_frames(g, tw, TW_ORIG, &frames)?;
        set_frames(g, tw, TW_WARPED, &frames)?;
        for c in curves {
            g.connect(&plug(tw, TW_OUT), &plug(c, anim::TIME_WARP), true)?;
        }
        Ok(tw)
    })();
    g.close_chunk();
    let tw = r?;
    g.evaluate();
    Ok(json!(g.path(tw)))
}

fn nums(v: &Value) -> Vec<f64> {
    match v {
        Value::Array(l) => l.iter().filter_map(Value::as_f64).collect(),
        x => x.as_f64().into_iter().collect(),
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// setKeyPath

/// The curve a motion path follows.
fn path_curve(g: &crate::graph::Graph, mp: NodeId) -> Option<NodeId> {
    g.connections().into_iter().find(|(d, s)| d.node == mp && g.node(s.node).is_some_and(|n| n.ty().is_a("nurbsCurve"))).map(|(_, s)| s.node)
}

fn set_key_path(s: &mut Session, a: &Args) -> CmdResult {
    let names = objects(s, a);
    if names.is_empty() {
        fail!("setKeyPath: name or select an object");
    }
    let frame = anim::frame(&s.graph);
    let mut out = Vec::new();
    s.graph.open_chunk("Set Key Path");
    let r = (|| -> CmdResult<()> {
        for n in &names {
            let g = &mut s.graph;
            let id = find(g, n, "setKeyPath")?;
            g.evaluate();
            let at = g.node(id).map_or(DVec3::ZERO, |x| x.matrix(Transform::WORLD_MATRIX).w_axis.truncate());
            let mp = g.source(&plug(id, Transform::TRANSLATE)).filter(|p| g.node(p.node).is_some_and(|x| x.ty().name == "motionPath")).map(|p| p.node);
            let (curve_shape, mp) = match mp {
                Some(mp) => (path_curve(g, mp), mp),
                None => {
                    let line = NurbsCurve::open(1, vec![at, at + DVec3::X * 1e-3]);
                    let (tr, sh) = crate::curves::create_curve(g, "curve1", line, false)?;
                    let path = g.path(tr);
                    let name = g.path(id);
                    let made = s.run("pathAnimation", json!({"args": [name], "curve": path, "fractionMode": false, "follow": false}))?;
                    let g = &mut s.graph;
                    let Some(mp) = made.as_str().and_then(|m| g.find(m).ok().flatten()) else { fail!("setKeyPath: no motion path") };
                    (Some(sh), mp)
                }
            };
            let g = &mut s.graph;
            let Some(sh) = curve_shape else { fail!("setKeyPath: the motion path on {n} has no curve") };
            let u_plug = crate::motion_path::u_value_plug(mp);
            // The points so far: the path at each key, with this one in time order.
            let old = crate::curves::world_curve(g, sh).unwrap_or_default();
            let mut pts: Vec<(f64, DVec3)> = Vec::new();
            if let Some(c) = anim::curve_of(g, &u_plug) {
                for k in anim::keys_of(g, c).keys {
                    pts.push((k.t, old.eval(k.v)));
                }
            }
            pts.retain(|p| (p.0 - frame).abs() > 1e-6);
            pts.push((frame, at));
            pts.sort_by(|x, y| x.0.total_cmp(&y.0));
            let positions: Vec<DVec3> = pts.iter().map(|p| p.1).collect();
            let curve = match positions.len() {
                1 => NurbsCurve::open(1, vec![at, at + DVec3::X * 1e-3]),
                n => NurbsCurve::through(if n >= 4 { 3 } else { 1 }, &positions),
            };
            let params: Vec<f64> = positions.iter().map(|p| curve.closest_param(*p)).collect();
            crate::curves::set_curve_data(g, sh, curve)?;
            if let Some(c) = anim::curve_of(g, &u_plug) {
                g.delete_node(c)?;
            }
            for ((t, _), u) in pts.iter().zip(&params) {
                anim::key_plug(g, &u_plug, *t, Some(*u), Some(anim::Tangent::Linear), Some(anim::Tangent::Linear))?;
            }
            out.push(json!(g.path(crate::dag::transform_of(g, sh))));
            out.push(json!(g.path(mp)));
        }
        Ok(())
    })();
    s.graph.close_chunk();
    r?;
    s.graph.evaluate();
    Ok(Value::Array(out))
}

// ---------------------------------------------------------------------------------------------------------------------
// recordAttr, tickDrawSpecial

fn record_attr(s: &mut Session, a: &Args) -> CmdResult {
    let names = objects(s, a);
    let attrs: Vec<String> = match a.get("attribute") {
        Some(Value::Array(v)) => v.iter().filter_map(|x| x.as_str().map(str::to_string)).collect(),
        Some(Value::String(t)) => vec![t.clone()],
        _ => Vec::new(),
    };
    let mut plugs = Vec::new();
    for n in &names {
        if let Some((obj, attr)) = n.split_once('.') {
            plugs.push(format!("{obj}.{attr}"));
            continue;
        }
        if attrs.is_empty() {
            let g = &s.graph;
            let id = find(g, n, "recordAttr")?;
            plugs.extend(anim::keyable_plugs(g, id).iter().map(|p| g.plug_name(p)));
        }
        plugs.extend(attrs.iter().map(|x| format!("{n}.{x}")));
    }
    let store = &mut s.extras.recording;
    if a.bool("query") {
        return Ok(json!(store));
    }
    if a.bool("delete") {
        store.retain(|p| !plugs.contains(p));
    } else {
        for p in plugs {
            if !store.contains(&p) {
                store.push(p);
            }
        }
    }
    Ok(Value::Null)
}

fn tick_draw_special(s: &mut Session, a: &Args) -> CmdResult {
    let mut call = json!({"args": a.named()});
    let on = a.get("special").is_none_or(|v| v.as_bool().unwrap_or(v.as_i64().unwrap_or(0) != 0));
    call["tickDrawSpecial"] = json!(on);
    if let Some(t) = a.get("time") {
        call["time"] = t.clone();
    }
    s.run("keyframe", call)?;
    Ok(Value::Null)
}

// ---------------------------------------------------------------------------------------------------------------------
// keyingGroup

fn keying_group(s: &mut Session, a: &Args) -> CmdResult {
    let names = a.named();
    if a.bool("query") {
        return s.run("sets", json!({"args": names, "query": true}));
    }
    if a.bool("edit") {
        let Some(set) = names.first() else { fail!("keyingGroup: name a keying group") };
        let mut call = json!({"args": [set]});
        if let Some(v) = a.get("addElement").or(a.get("include")) {
            call = json!({"args": [v], "addElement": set});
        }
        if let Some(v) = a.get("remove") {
            call = json!({"args": [v], "remove": set});
        }
        return s.run("sets", call);
    }
    let mut members = objects(s, a);
    if a.bool("empty") {
        members.clear();
    }
    let g = &mut s.graph;
    let list = crate::object_sets::parse(g, &members).map_err(aztek_ops::CommandError::new)?;
    let t = types::find("keyingGroup").unwrap();
    g.open_chunk("keyingGroup");
    let r = (|| -> CmdResult<NodeId> {
        let n = g.create_node(&t, &a.string("name", "keyingGroup1"), NO_NODE, true)?;
        crate::object_sets::add(g, n, &list)?;
        Ok(n)
    })();
    g.close_chunk();
    let n = r?;
    Ok(json!(g.path(n)))
}

// ---------------------------------------------------------------------------------------------------------------------
// characterMap

const MAP_LIST: usize = 0;

fn character_map(s: &mut Session, a: &Args) -> CmdResult {
    let names = a.named();
    let map_node = names.iter().filter_map(|n| s.graph.find(n).ok().flatten()).find(|&i| s.graph.node(i).is_some_and(|n| n.ty().name == "characterMap"));
    let member_attrs = |s: &Session, id: NodeId| -> Vec<String> {
        let g = &s.graph;
        crate::characters::member_plugs(g, id).iter().map(|p| g.plug_name(p)).collect()
    };
    // Names matched attribute to attribute, for -proposedMapping and a new map.
    let proposed = |s: &Session, one: NodeId, two: NodeId| -> Vec<String> {
        let (x, y) = (member_attrs(s, one), member_attrs(s, two));
        let attr = |p: &str| p.rsplit('.').next().unwrap_or(p).to_string();
        let mut out = Vec::new();
        for p in &x {
            if let Some(q) = y.iter().find(|q| attr(q) == attr(p)) {
                out.push(format!("{p} {q}"));
            }
        }
        out
    };
    let chars: Vec<NodeId> = names.iter().filter_map(|n| s.graph.find(n).ok().flatten()).filter(|&i| crate::characters::is_character(&s.graph, i)).collect();
    if a.bool("proposedMapping") {
        let [one, two] = chars.as_slice() else { fail!("characterMap: name two characters") };
        return Ok(json!(proposed(s, *one, *two)));
    }
    let list = |s: &Session, m: NodeId| -> Vec<String> {
        s.graph.node(m).unwrap().elements(MAP_LIST).iter().filter_map(|(_, v)| if let PlugValue::String(t) = v { Some(t.clone()) } else { None }).collect()
    };
    if a.bool("query") {
        let Some(m) = map_node else { fail!("characterMap: name a characterMap") };
        return Ok(json!(list(s, m)));
    }
    let edits = a.get("mapAttr").is_some() || a.get("unmapAttr").is_some();
    let m = match map_node {
        Some(m) => m,
        None => {
            let [one, two] = chars.as_slice() else { fail!("characterMap: name two characters") };
            let first = proposed(s, *one, *two);
            let g = &mut s.graph;
            let t = types::find("characterMap").unwrap();
            g.open_chunk("characterMap");
            let n = g.create_node(&t, &a.string("mapping", "characterMap1"), NO_NODE, true);
            let n = n.map_err(aztek_ops::CommandError::from).and_then(|n| {
                g.set_elements(&PlugRef::new(n, MAP_LIST as i32), &first.into_iter().map(PlugValue::String).collect::<Vec<_>>())?;
                Ok(n)
            });
            g.close_chunk();
            n?
        }
    };
    if edits {
        let mut now = list(s, m);
        let pair = |v: Option<&Value>| -> Option<String> {
            let l = v?.as_array()?;
            Some(format!("{} {}", l.first()?.as_str()?, l.get(1)?.as_str()?))
        };
        if let Some(p) = pair(a.get("mapAttr")) {
            if !now.contains(&p) {
                now.push(p);
            }
        }
        if let Some(p) = pair(a.get("unmapAttr")) {
            now.retain(|x| *x != p);
        }
        s.graph.set_elements(&PlugRef::new(m, MAP_LIST as i32), &now.into_iter().map(PlugValue::String).collect::<Vec<_>>())?;
    }
    Ok(json!([s.graph.path(m)]))
}

// ---------------------------------------------------------------------------------------------------------------------

const ANIM_DISPLAY: &[(&str, &str, ArgType, Def)] = &[
    ("modelUpdate", "upd", S, Def::S("interactive")),
    ("refAnimCurvesEditable", "rae", B, Def::B(false)),
    ("timeCode", "tc", B, Def::B(false)),
    ("timeCodeOffset", "tco", S, Def::S("00:00:00:00")),
];

fn real_attr(name: &str, short: &str, ty: PlugType, default: PlugValue, fl: u32) -> AttrDef {
    AttrDef { name: name.into(), short_name: short.into(), ty, default_value: default, flags: fl, doc: "A setting.".into(), ..Default::default() }
}

pub fn register_types() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let store = flags::INPUT | flags::STORABLE;
        types::register_type(NodeType {
            name: "timeWarp".into(),
            icon: "chart-line".into(),
            doc: "Moves time for the animation curves it drives: the key at each original frame plays at its warped frame.".into(),
            own_attrs: vec![
                real_attr("apply", "ap", PlugType::Bool, PlugValue::Bool(true), store | flags::KEYABLE),
                real_attr("origFrames", "of", PlugType::Float, PlugValue::Float(0.0), store | flags::ARRAY),
                real_attr("warpedFrames", "wf", PlugType::Float, PlugValue::Float(0.0), store | flags::ARRAY),
                real_attr("interpType", "it", PlugType::Int, PlugValue::Int(1), store | flags::ARRAY),
                crate::curves::curve_attr("warpOutput", "wo", flags::OUTPUT | flags::HIDDEN, "The frames, for the curves."),
            ],
            create: Some(Arc::new(|| Box::new(TimeWarpNode))),
            ..Default::default()
        })
        .expect("timeWarp");
        types::register_type(NodeType {
            name: "characterMap".into(),
            doc: "Pairs the attributes of two characters.".into(),
            own_attrs: vec![real_attr("mapping", "mp", PlugType::String, PlugValue::String(String::new()), store | flags::ARRAY)],
            create: Some(Arc::new(|| Box::new(crate::node::Plain))),
            ..Default::default()
        })
        .expect("characterMap");
        let set = types::find(crate::object_sets::TYPE_NAME).expect("objectSet");
        types::register_type(NodeType {
            name: "keyingGroup".into(),
            base: Some(set.clone()),
            doc: "A set of objects keyed together.".into(),
            own_attrs: vec![real_attr("minimizeRotation", "mr", PlugType::Bool, PlugValue::Bool(false), store), real_attr("category", "cat", PlugType::String, PlugValue::String(String::new()), store)],
            create: set.create.clone(),
            ..Default::default()
        })
        .expect("keyingGroup");
    });
}

fn copy_of(c: &mut Commands<Session>, from: &str, name: &str, doc: &str, also: &[(&str, Value)]) {
    let mut def = c.find(from).unwrap_or_else(|| panic!("no command {from}")).clone();
    def.name = name.to_string();
    def.doc = doc.to_string();
    let run = def.run.clone();
    let also: Vec<(String, Value)> = also.iter().map(|(k, v)| (k.to_string(), v.clone())).collect();
    def.run = Arc::new(move |s: &mut Session, a: &Args| {
        let mut call = a.clone();
        for (k, v) in &also {
            if !call.has(k) {
                call.flags.insert(k.clone(), v.clone());
            }
        }
        run(s, &call)
    });
    c.register(def);
}

pub fn register(c: &mut Commands<Session>) {
    register_types();
    c.register(
        q_flags(Cmd::new(
            "timeWarp",
            "Warp time for animation curves: names objects or curves, -f the frames to warp at (the playback range by default), -g every curve. With -e: -mf index frame moves one, -df index removes one. -q -f reads them. Returns the node.",
            time_warp,
        ))
        .positional("objects or curves, or the node with -e")
        .flag(f("frame", "f", ArgType::Any, "Frames to warp at, or with -q ask."))
        .flag(f("moveFrame", "mf", ArgType::List, "A warp frame's index and where it goes.").multi(2))
        .flag(f("deleteFrame", "df", I, "A warp frame's index to remove."))
        .flag(f("global", "g", B, "Warp every curve in the scene."))
        .flag(f("interpType", "it", ArgType::List, "Accepted: warps interpolate linearly here.").multi(2))
        .flag(f("name", "n", S, "The node's name.")),
    );
    c.register(Cmd::new("setKeyPath", "Key an object's position on a motion path at the current time: makes the path and curve the first time, adds a point after that. Returns [curve, motionPath].", set_key_path).positional("objects"));
    copy_of(c, "selectKey", "selectKeyframe", "Select keys, as selectKey.", &[]);
    copy_of(c, "bakeResults", "bakeSimulation", "Key channels with what they evaluate to, stepping through time: bakeResults -simulation.", &[("simulation", json!(true))]);
    c.register(
        Cmd::new("tickDrawSpecial", "Draw keys' ticks in the special colour on the time slider: -sp on or off (on), -t which keys.", tick_draw_special)
            .positional("objects or plugs (the selection when none)")
            .flag(f("special", "sp", ArgType::Any, "On or off."))
            .flag(f("time", "t", ArgType::Any, "Which keys.")),
    );
    c.register(
        q_flags(Cmd::new("recordAttr", "Mark attributes for recording, kept for scripts: -at names them, all keyable ones when none; -d takes them off; -q lists them.", record_attr))
            .positional("objects or plugs")
            .flag(f("attribute", "at", ArgType::Any, "The attributes.").multi(1))
            .flag(f("delete", "d", B, "Take them off."))
            .undoable(false),
    );
    setting_store(c, "animDisplay", "How animation shows, kept for scripts: -upd, -tc, -tco, -rae; -q reads.", ANIM_DISPLAY, |s| &mut s.extras.anim_display);
    c.register(
        q_flags(Cmd::new("keyingGroup", "A set of objects keyed together: -n, -em empty; -e -add and -rm change it; -q lists it.", keying_group))
            .positional("objects")
            .flag(f("name", "n", S, "The set's name."))
            .flag(f("empty", "em", B, "Make it empty."))
            .flag(f("addElement", "add", ArgType::Any, "With -e: what to add."))
            .flag(f("include", "in", ArgType::Any, "With -e: what to add."))
            .flag(f("remove", "rm", ArgType::Any, "With -e: what to remove."))
            .flag(f("minimizeRotation", "mr", B, "Accepted for existing scripts.")),
    );
    c.register(
        q_flags(Cmd::new("characterMap", "Pair two characters' attributes: name them for a map matched by attribute name, -pm for the pairs alone, -ma and -ua with -e to change it. Returns the node; -q the pairs.", character_map))
            .positional("two characters, or a map")
            .flag(f("mapping", "m", S, "The node's name."))
            .flag(f("proposedMapping", "pm", B, "The pairs it would make."))
            .flag(f("mapAttr", "ma", ArgType::List, "Two plugs to pair.").multi(2))
            .flag(f("unmapAttr", "ua", ArgType::List, "Two plugs to part.").multi(2))
            .flag(f("mapMethod", "mm", S, "Accepted: by name is what it does."))
            .flag(f("mapNode", "mn", ArgType::List, "Accepted for existing scripts.").multi(2))
            .flag(f("unmapNode", "un", ArgType::List, "Accepted for existing scripts.").multi(2)),
    );
}
