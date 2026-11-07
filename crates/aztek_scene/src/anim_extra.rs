//! Animation commands: blendTwoAttr, ghosting and timeCode.
//!
//! Differs from the reference. ghosting sets the ghost attributes and answers about ghost frames, but the views draw no ghosts.
//! timeCode's numbers live in the session rather than on the time1 node, so they are not saved.

use aztek_ops::{fail, objects, ArgType, Args, CmdResult, CommandDef, Commands};
use serde_json::{json, Value};

use crate::graph::Graph;
use crate::ref_core::f;
use crate::plug::{NodeId, PlugValue};
use crate::rig::plug;
use crate::session::Session;
use crate::skin::asked;
use crate::system_cmds::q_flags;

type Cmd = CommandDef<Session>;
use ArgType::{Any as A, Bool as B, Float as Fl, Int as I, String as S};

fn text(v: &Value) -> String {
    v.as_str().map_or_else(|| v.to_string(), str::to_string)
}

fn driver_of(s: &mut Session, p: &str) -> CmdResult<String> {
    Ok(s.run("connectionInfo", json!({"args": [p], "sourceFromDestination": true}))?.as_str().unwrap_or("").to_string())
}

// ---------------------------------------------------------------------------------------------------------------------
// blendTwoAttr

/// The plugs a create call means: `-at` on each object, plugs as named, or an object's keyable attributes.
fn blend_plugs(s: &mut Session, a: &Args) -> CmdResult<Vec<String>> {
    let names = objects(s, a);
    let attrs: Vec<String> = match a.get("attribute") {
        Some(Value::Array(l)) => l.iter().map(text).collect(),
        Some(v) => vec![text(v)],
        None => Vec::new(),
    };
    let mut out = Vec::new();
    for n in names {
        if n.contains('.') {
            out.push(n);
        } else if !attrs.is_empty() {
            out.extend(attrs.iter().map(|at| format!("{n}.{at}")));
        } else {
            let keyable = s.run("listAttr", json!({"args": [n], "keyable": true}))?;
            out.extend(keyable.as_array().into_iter().flatten().filter_map(Value::as_str).map(|at| format!("{n}.{at}")));
        }
    }
    Ok(out)
}

fn set_blender(s: &mut Session, node: &str, a: &Args) -> CmdResult<()> {
    let ab = format!("{node}.attributesBlender");
    if let Some(b) = a.get("blender").and_then(Value::as_str) {
        s.run("connectAttr", json!({"args": [b, ab], "force": true}))?;
        return Ok(());
    }
    let Some((lo, hi)) = crate::anim::range_flag(a, "time")? else { return Ok(()) };
    if lo == hi {
        // A single time: an abrupt change there.
        s.run("setKeyframe", json!({"args": [ab], "time": lo - 1.0, "value": 0.0, "outTangentType": "step"}))?;
        s.run("setKeyframe", json!({"args": [ab], "time": lo, "value": 1.0}))?;
    } else {
        s.run("setKeyframe", json!({"args": [ab], "time": lo, "value": 0.0, "inTangentType": "linear", "outTangentType": "linear"}))?;
        s.run("setKeyframe", json!({"args": [ab], "time": hi, "value": 1.0, "inTangentType": "linear", "outTangentType": "linear"}))?;
    }
    Ok(())
}

fn blend_two_attr(s: &mut Session, a: &Args) -> CmdResult {
    if a.bool("query") || a.bool("edit") {
        let Some(node) = a.named().into_iter().next() else { fail!("blendTwoAttr: name the node") };
        if a.bool("query") {
            for (flag, plug_name) in [("attribute0", "input[0]"), ("attribute1", "input[1]"), ("blender", "attributesBlender")] {
                if asked(a, flag) {
                    return Ok(json!(driver_of(s, &format!("{node}.{plug_name}"))?));
                }
            }
            if asked(a, "driver") {
                return s.run("getAttr", json!({"args": [format!("{node}.current")]}));
            }
            fail!("blendTwoAttr: ask for -at0, -at1, -bl or -d");
        }
        if let Some(x) = a.get("attribute0").and_then(Value::as_str) {
            s.run("connectAttr", json!({"args": [x, format!("{node}.input[0]")], "force": true}))?;
        }
        if let Some(x) = a.get("attribute1").and_then(Value::as_str) {
            s.run("connectAttr", json!({"args": [x, format!("{node}.input[1]")], "force": true}))?;
        }
        if a.has("driver") {
            s.run("setAttr", json!({"args": [format!("{node}.current"), a.int("driver", 0)]}))?;
        }
        set_blender(s, &node, a)?;
        return Ok(json!(node));
    }
    let plugs = blend_plugs(s, a)?;
    if plugs.is_empty() {
        fail!("blendTwoAttr: name or select attributes");
    }
    let mut out = Vec::new();
    for p in plugs {
        let driver = driver_of(s, &p)?;
        let held = s.run("getAttr", json!({"args": [p]}))?;
        let mut call = json!({"args": ["blendTwoAttr"]});
        if let Some(n) = a.get("name").and_then(Value::as_str) {
            call["name"] = json!(n);
        }
        let node = s.run("createNode", call)?.as_str().unwrap_or("").to_string();
        let (i0, i1) = (format!("{node}.input[0]"), format!("{node}.input[1]"));
        // The first input keeps what the attribute had, unless told otherwise; so does the second until something feeds it.
        match a.get("attribute0").and_then(Value::as_str) {
            Some(x) => {
                s.run("connectAttr", json!({"args": [x, i0]}))?;
            }
            None if !driver.is_empty() => {
                s.run("connectAttr", json!({"args": [driver, i0]}))?;
            }
            None => {
                s.run("setAttr", json!({"args": [i0, held]}))?;
            }
        }
        match a.get("attribute1").and_then(Value::as_str) {
            Some(x) => {
                s.run("connectAttr", json!({"args": [x, i1]}))?;
            }
            None => {
                s.run("setAttr", json!({"args": [i1, held]}))?;
            }
        }
        s.run("connectAttr", json!({"args": [format!("{node}.output"), p], "force": true}))?;
        if a.has("driver") {
            s.run("setAttr", json!({"args": [format!("{node}.current"), a.int("driver", 0)]}))?;
        }
        set_blender(s, &node, a)?;
        out.push(json!(node));
    }
    Ok(Value::Array(out))
}

// ---------------------------------------------------------------------------------------------------------------------
// ghosting

fn ghost_default(name: &str) -> Value {
    match name {
        "enable" | "hierarchy" | "allInRange" => json!(true),
        "geometryFilter" | "jointFilter" | "locatorFilter" | "useDriver" => json!(false),
        "preFrames" | "postFrames" => json!(3),
        "ghostsStep" => json!(1),
        "mode" => json!("preAndPost"),
        "preset" => json!("Custom"),
        "nearOpacity" => json!(0.5),
        "farOpacity" => json!(0.1),
        "preColor" => json!([0.5, 0.5, 1.0]),
        "postColor" => json!([1.0, 0.5, 0.5]),
        "customFrames" => json!([]),
        _ => Value::Null,
    }
}

const GHOST_OPTIONS: &[&str] = &["allInRange", "customFrames", "enable", "farOpacity", "geometryFilter", "ghostsStep", "hierarchy", "jointFilter", "locatorFilter", "mode", "nearOpacity", "postColor", "postFrames", "preColor", "preFrames", "preset", "useDriver"];

fn ghost_option(s: &Session, name: &str) -> Value {
    s.extras.more.ghosting.get(name).cloned().unwrap_or_else(|| ghost_default(name))
}

fn ghost_attr(g: &Graph, n: NodeId, name: &str) -> Option<usize> {
    g.node(n)?.ty().attr_index(name)
}

fn is_ghosted(g: &Graph, n: NodeId) -> bool {
    let Some(i) = ghost_attr(g, n, "ghosting") else { return false };
    g.node(n).is_some_and(|x| matches!(x.data().value(i), PlugValue::Bool(true)))
}

/// What an action goes over: the objects, down their hierarchies when asked, minus what the filters leave out.
fn ghost_objects(s: &Session, a: &Args) -> CmdResult<Vec<NodeId>> {
    let g = &s.graph;
    let hierarchy = ghost_option(s, "hierarchy").as_bool().unwrap_or(true);
    let mut out: Vec<NodeId> = Vec::new();
    for name in objects(s, a) {
        let Some(id) = g.find(&name)? else { fail!("ghosting: no object {name}") };
        let mut list = vec![id];
        if hierarchy {
            list.extend(g.descendants(id));
        }
        for n in list {
            let Some(node) = g.node(n) else { continue };
            if !node.ty().is_a("transform") || out.contains(&n) {
                continue;
            }
            if node.ty().attr_index("visibility").is_some_and(|i| matches!(node.data().value(i), PlugValue::Bool(false))) {
                continue;
            }
            let is_joint = node.ty().is_a("joint");
            let shapes = crate::dag::shapes(g, n, false);
            let has = |ty: &str| shapes.iter().any(|&k| g.node(k).is_some_and(|x| x.ty().is_a(ty)));
            let on = |flag: &str| ghost_option(s, flag).as_bool().unwrap_or(false);
            if (is_joint && on("jointFilter")) || (has("locator") && on("locatorFilter")) || ((has("mesh") || has("nurbsSurface") || has("nurbsCurve")) && on("geometryFilter")) {
                continue;
            }
            out.push(n);
        }
    }
    Ok(out)
}

fn apply_ghost_options(s: &mut Session, n: NodeId) -> CmdResult<()> {
    let settings = [("preFrames", "ghostPreSteps"), ("postFrames", "ghostPostSteps"), ("ghostsStep", "ghostStepSize")];
    for (option, attr) in settings {
        let v = ghost_option(s, option).as_i64().unwrap_or(1);
        if let Some(i) = ghost_attr(&s.graph, n, attr) {
            s.graph.set_value(plug(n, i), PlugValue::Int(v))?;
        }
    }
    Ok(())
}

fn ghost_frames(s: &mut Session, n: NodeId) -> Vec<f64> {
    let now = crate::anim::frame(&s.graph);
    let read = |s: &mut Session, name: &str, d: i64| -> i64 {
        match ghost_attr(&s.graph, n, name).map(|i| s.graph.value(&plug(n, i))) {
            Some(Ok(PlugValue::Int(v))) => v,
            _ => d,
        }
    };
    let (pre, post, step) = (read(s, "ghostPreSteps", 3), read(s, "ghostPostSteps", 3), read(s, "ghostStepSize", 1).max(1));
    let mode = ghost_option(s, "mode");
    let mode = mode.as_str().unwrap_or("preAndPost");
    if mode == "custom" {
        return ghost_option(s, "customFrames").as_array().map(|l| l.iter().filter_map(Value::as_f64).collect()).unwrap_or_default();
    }
    if mode == "keyframes" {
        let mut keys = crate::anim::key_times(&s.graph, &[n]);
        keys.sort_by(f64::total_cmp);
        keys.dedup();
        return keys.into_iter().step_by(step as usize).filter(|&t| t != now).collect();
    }
    let mut out = Vec::new();
    if mode != "post" {
        out.extend((1..=pre).rev().map(|k| now - (k * step) as f64));
    }
    if mode != "pre" {
        out.extend((1..=post).map(|k| now + (k * step) as f64));
    }
    out
}

fn ghosting(s: &mut Session, a: &Args) -> CmdResult {
    if a.bool("resetAll") {
        s.extras.more.ghosting.clear();
        let ghosted: Vec<NodeId> = s.graph.nodes().into_iter().filter(|&n| is_ghosted(&s.graph, n)).collect();
        for n in ghosted {
            if let Some(i) = ghost_attr(&s.graph, n, "ghosting") {
                s.graph.set_value(plug(n, i), PlugValue::Bool(false))?;
            }
        }
        return Ok(Value::Null);
    }
    if a.bool("query") {
        if asked(a, "ghostedObjects") {
            let nodes = s.graph.nodes();
            let found: Vec<String> = nodes.into_iter().filter(|&n| is_ghosted(&s.graph, n)).map(|n| s.graph.path(n)).collect();
            return Ok(json!(found));
        }
        if asked(a, "frames") {
            let mut out: Vec<f64> = Vec::new();
            for n in ghost_objects(s, a)? {
                for t in ghost_frames(s, n) {
                    if !out.contains(&t) {
                        out.push(t);
                    }
                }
            }
            out.sort_by(f64::total_cmp);
            return Ok(json!(out));
        }
        if let Some(name) = GHOST_OPTIONS.iter().find(|n| asked(a, n)) {
            return Ok(ghost_option(s, name));
        }
        fail!("ghosting: nothing to query");
    }
    for name in GHOST_OPTIONS {
        if let Some(v) = a.get(name) {
            // -9999999 is how the reference clears the custom frames.
            let v = if *name == "customFrames" && v.as_i64() == Some(-9999999) { json!([]) } else { v.clone() };
            s.extras.more.ghosting.insert((*name).to_string(), v);
        }
    }
    if let Some(p) = a.get("preset").and_then(Value::as_str) {
        let step = match p {
            "1s" => Some(1),
            "2s" => Some(2),
            "4s" => Some(4),
            "5s" => Some(5),
            "10s" => Some(10),
            "Custom" => None,
            _ => fail!("ghosting: preset {p} is not 1s, 2s, 4s, 5s, 10s or Custom"),
        };
        if let Some(step) = step {
            s.extras.more.ghosting.insert("ghostsStep".into(), json!(step));
        }
    }
    if a.bool("edit") {
        let nodes: Vec<NodeId> = if a.bool("allGhostedObjects") {
            let all = s.graph.nodes();
            all.into_iter().filter(|&n| is_ghosted(&s.graph, n)).collect()
        } else {
            ghost_objects(s, a)?
        };
        for n in nodes {
            apply_ghost_options(s, n)?;
        }
        return Ok(Value::Null);
    }
    let Some(action) = a.get("action").and_then(Value::as_str) else { return Ok(Value::Null) };
    let nodes = match action {
        "ghost" | "unghost" => ghost_objects(s, a)?,
        "unghostAll" => {
            let all = s.graph.nodes();
            all.into_iter().filter(|&n| is_ghosted(&s.graph, n)).collect()
        }
        _ => fail!("ghosting: action {action} is not ghost, unghost or unghostAll"),
    };
    let mut names = Vec::new();
    for n in nodes {
        let Some(i) = ghost_attr(&s.graph, n, "ghosting") else { continue };
        s.graph.set_value(plug(n, i), PlugValue::Bool(action == "ghost"))?;
        if action == "ghost" {
            apply_ghost_options(s, n)?;
        }
        names.push(s.graph.path(n));
    }
    Ok(json!(names.join(" ")))
}

// ---------------------------------------------------------------------------------------------------------------------
// timeCode

const TIME_CODE: &[&str] = &["mayaStartFrame", "productionStartFrame", "productionStartHour", "productionStartMinute", "productionStartSecond"];

fn time_code(s: &mut Session, a: &Args) -> CmdResult {
    let map = &mut s.extras.more.time_code;
    if a.bool("query") {
        let Some(name) = TIME_CODE.iter().find(|n| asked(a, n)) else { fail!("timeCode: name what to query") };
        return Ok(json!(map.get(*name).copied().unwrap_or(0.0)));
    }
    for name in TIME_CODE {
        if let Some(v) = a.get(name).and_then(Value::as_f64) {
            map.insert((*name).to_string(), v);
        }
    }
    Ok(Value::Null)
}

pub fn register(c: &mut Commands<Session>) {
    c.register(
        q_flags(Cmd::new(
            "blendTwoAttr",
            "Blend attributes between two inputs with a blendTwoAttr node: -at on what's named or selected (every keyable attribute without it), -at0 and -at1 what feeds the inputs (what drove the attribute by default), -bl a blender attribute, -t the range to key the blend over, -d the driver index, -n a name. Returns the nodes. -s and -cp are accepted: shapes below transforms are not walked.",
            blend_two_attr,
        ))
        .positional("objects or plugs (the selection when none); the node with -q and -e")
        .flag(f("attribute", "at", A, "The attributes.").multi(1))
        .flag(f("attribute0", "at0", A, "What feeds the first input."))
        .flag(f("attribute1", "at1", A, "What feeds the second input."))
        .flag(f("blender", "bl", A, "The attribute driving the blend."))
        .flag(f("controlPoints", "cp", B, "Accepted for existing scripts; has no effect."))
        .flag(f("driver", "d", A, "The driver index, 0 or 1."))
        .flag(f("name", "n", S, "The node's name."))
        .flag(f("shape", "s", B, "Accepted for existing scripts; has no effect."))
        .flag(f("time", "t", A, "The range to blend over; a single time is an abrupt change.")),
    );
    c.register(
        q_flags(Cmd::new(
            "ghosting",
            "Ghosting state: -act ghost, unghost or unghostAll on the selection, the options as defaults, -e to change objects (-ago all ghosted), -q an option, -q -go the ghosted objects, -q -f their ghost frames. Less than the reference: the views draw no ghosts, so -en, the colours, the opacities and -ud are only held.",
            ghosting,
        ))
        .positional("objects (the selection when none)")
        .flag(f("action", "act", S, "ghost, unghost or unghostAll."))
        .flag(f("allGhostedObjects", "ago", B, "With -e: every ghosted object."))
        .flag(f("allInRange", "a", B, "Keyframes mode uses the whole playback range. Held only."))
        .flag(f("customFrames", "cf", A, "The custom frames; -9999999 clears them.").multi(1))
        .flag(f("enable", "en", B, "Ghosts on the whole scene. Held only."))
        .flag(f("farOpacity", "fo", Fl, "Opacity of the farthest ghost. Held only."))
        .flag(f("frames", "f", B, "With -q: the ghost frames."))
        .flag(f("geometryFilter", "gf", B, "Leave geometry out."))
        .flag(f("ghostedObjects", "go", B, "With -q: the ghosted objects."))
        .flag(f("ghostsStep", "gs", I, "Frames or keys between ghosts."))
        .flag(f("hierarchy", "h", B, "Walk down hierarchies."))
        .flag(f("jointFilter", "jf", B, "Leave joints out."))
        .flag(f("locatorFilter", "lf", B, "Leave locators out."))
        .flag(f("mode", "m", S, "preAndPost, pre, post, custom or keyframes."))
        .flag(f("nearOpacity", "no", Fl, "Opacity of the nearest ghost. Held only."))
        .flag(f("postColor", "poc", ArgType::Vec3, "Colour after the current time. Held only."))
        .flag(f("postFrames", "pof", I, "Ghosts after the current time."))
        .flag(f("preColor", "prc", ArgType::Vec3, "Colour before the current time. Held only."))
        .flag(f("preFrames", "prf", I, "Ghosts before the current time."))
        .flag(f("preset", "p", S, "1s, 2s, 4s, 5s, 10s or Custom."))
        .flag(f("resetAll", "r", B, "Back to the defaults, everything unghosted."))
        .flag(f("useDriver", "ud", B, "Use the driver's frames. Held only.")),
    );
    c.register(
        q_flags(Cmd::new(
            "timeCode",
            "Held for scripts: the file's time code, -msf, -psf, -psh, -psm, -pss, read back with -q. Less than the reference: the numbers are in the session, not on a time1 node, so they are not saved, and nothing shows them.",
            time_code,
        ))
        .flag(f("mayaStartFrame", "msf", Fl, "The start time, in frames."))
        .flag(f("productionStartFrame", "psf", Fl, "Production start: frames."))
        .flag(f("productionStartHour", "psh", Fl, "Production start: hours."))
        .flag(f("productionStartMinute", "psm", Fl, "Production start: minutes."))
        .flag(f("productionStartSecond", "pss", Fl, "Production start: seconds."))
        .undoable(false),
    );
}
