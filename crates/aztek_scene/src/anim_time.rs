//! The clock: currentTime, playbackOptions, play, timeControl and currentUnit; and ghosting.

use aztek_ops::units;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use aztek_ops::{fail, objects, ArgType, Args, CmdResult, Commands};
use serde_json::{json, Value};

use crate::anim::{self, f, query_flags, queried, Cmd};
use crate::plug::{NodeId, PlugRef, PlugValue};
use crate::session::Session;

/// The reference's time unit names and their frames per second.
pub const TIME_UNITS: &[(&str, f64)] = &[
    ("game", 15.0),
    ("film", 24.0),
    ("pal", 25.0),
    ("ntsc", 30.0),
    ("show", 48.0),
    ("palf", 50.0),
    ("ntscf", 60.0),
    ("2fps", 2.0),
    ("3fps", 3.0),
    ("4fps", 4.0),
    ("5fps", 5.0),
    ("6fps", 6.0),
    ("8fps", 8.0),
    ("10fps", 10.0),
    ("12fps", 12.0),
    ("16fps", 16.0),
    ("20fps", 20.0),
    ("23.976fps", 23.976),
    ("29.97fps", 29.97),
    ("29.97df", 29.97),
    ("40fps", 40.0),
    ("47.952fps", 47.952),
    ("59.94fps", 59.94),
    ("75fps", 75.0),
    ("80fps", 80.0),
    ("100fps", 100.0),
    ("120fps", 120.0),
    ("125fps", 125.0),
    ("150fps", 150.0),
    ("200fps", 200.0),
    ("240fps", 240.0),
    ("250fps", 250.0),
    ("300fps", 300.0),
    ("375fps", 375.0),
    ("400fps", 400.0),
    ("500fps", 500.0),
    ("600fps", 600.0),
    ("750fps", 750.0),
    ("1200fps", 1200.0),
    ("1500fps", 1500.0),
    ("2000fps", 2000.0),
    ("3000fps", 3000.0),
    ("6000fps", 6000.0),
    ("44100fps", 44100.0),
    ("48000fps", 48000.0),
    ("sec", 1.0),
    ("min", 1.0 / 60.0),
    ("hour", 1.0 / 3600.0),
    ("millisec", 1000.0),
];

pub fn time_unit_fps(name: &str) -> Option<f64> {
    if let Some((_, v)) = TIME_UNITS.iter().find(|(n, _)| *n == name) {
        return Some(*v);
    }
    name.strip_suffix("fps").and_then(|n| n.parse::<f64>().ok()).filter(|v| *v > 0.0)
}

/// The name of a frame rate: film for 24, or "90fps" for one the reference has no name for.
pub fn time_unit_name(fps: f64) -> String {
    match TIME_UNITS.iter().find(|(_, v)| (v - fps).abs() < 1e-9) {
        Some((n, _)) => n.to_string(),
        None => format!("{fps}fps"),
    }
}

/// A frame given with a unit, as currentTime takes it: "10", "2sec", "12ntsc", "1.5min".
fn parse_time(v: &Value) -> Option<f64> {
    if let Some(x) = v.as_f64() {
        return Some(x);
    }
    let s = v.as_str()?.trim();
    if let Ok(x) = s.parse::<f64>() {
        return Some(x);
    }
    let split = s.find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-')).unwrap_or(s.len());
    let (n, unit) = s.split_at(split);
    Some(n.parse::<f64>().ok()? / time_unit_fps(unit)? * anim::fps())
}

// ---------------------------------------------------------------------------------------------------------------------

static PLAYING: AtomicBool = AtomicBool::new(false);
static PLAY_FORWARD: AtomicBool = AtomicBool::new(true);

/// Whether a script asked for playback, which the application's clock follows.
pub fn playing() -> Option<i32> {
    PLAYING.load(Ordering::Relaxed).then(|| if PLAY_FORWARD.load(Ordering::Relaxed) { 1 } else { -1 })
}

pub fn set_playing(on: bool, forward: bool) {
    PLAYING.store(on, Ordering::Relaxed);
    PLAY_FORWARD.store(forward, Ordering::Relaxed);
    crate::draw_style::set_play_direction(if !on { 0 } else if forward { 1 } else { -1 });
}

#[derive(Clone, Debug)]
struct SliderState {
    range: Option<(f64, f64)>,
    show_keys: String,
    snap: bool,
    tick_size: f64,
}

static SLIDER: Mutex<Option<SliderState>> = Mutex::new(None);

fn slider() -> SliderState {
    SLIDER.lock().unwrap().clone().unwrap_or(SliderState { range: None, show_keys: "active".into(), snap: true, tick_size: 1.0 })
}

/// The range selected on the time slider, which the UI reports through timeControl -e -ra.
pub fn slider_range() -> Option<(f64, f64)> {
    slider().range
}

fn play(s: &mut Session, a: &Args) -> CmdResult {
    if a.bool("query") {
        if queried(a, &["forward"]) {
            return Ok(json!(PLAY_FORWARD.load(Ordering::Relaxed)));
        }
        return Ok(json!(PLAYING.load(Ordering::Relaxed)));
    }
    let forward = a.get("forward").and_then(Value::as_bool).unwrap_or(PLAY_FORWARD.load(Ordering::Relaxed));
    let state = a.get("state").and_then(Value::as_bool).unwrap_or(true);
    if a.bool("wait") && state {
        // Runs through the range once, then stops, as -w blocks in the reference.
        let r = anim::range();
        let by = r.by.abs().max(1e-3);
        let (mut t, end, step) = if forward { (anim::frame(&s.graph), r.max, by) } else { (anim::frame(&s.graph), r.min, -by) };
        while (step > 0.0 && t < end) || (step < 0.0 && t > end) {
            t = if step > 0.0 { (t + step).min(end) } else { (t + step).max(end) };
            anim::set_frame(&mut s.graph, t);
            s.graph.evaluate();
        }
        set_playing(false, forward);
        return Ok(Value::Null);
    }
    set_playing(state, forward);
    Ok(Value::Null)
}

fn playback_options(a: &Args) -> CmdResult {
    if a.bool("query") {
        let r = anim::range();
        return Ok(if queried(a, &["maxTime"]) {
            anim::frame_json(r.max)
        } else if queried(a, &["animationStartTime"]) {
            anim::frame_json(r.anim_start)
        } else if queried(a, &["animationEndTime"]) {
            anim::frame_json(r.anim_end)
        } else if queried(a, &["loop"]) {
            json!(r.loop_mode)
        } else if queried(a, &["playbackSpeed"]) {
            json!(r.speed)
        } else if queried(a, &["maxPlaybackSpeed"]) {
            json!(r.max_speed)
        } else if queried(a, &["framesPerSecond"]) {
            json!(anim::fps())
        } else if queried(a, &["by"]) {
            json!(r.by)
        } else if queried(a, &["view"]) {
            json!(r.view)
        } else if queried(a, &["preroll"]) {
            json!(r.preroll)
        } else {
            anim::frame_json(r.min)
        });
    }
    let num = |n: &str| a.get(n).and_then(Value::as_f64);
    anim::set_range(|r| {
        if let Some(v) = num("minTime") {
            r.min = v;
        }
        if let Some(v) = num("maxTime") {
            r.max = v;
        }
        if let Some(v) = num("animationStartTime") {
            r.anim_start = v;
        }
        if let Some(v) = num("animationEndTime") {
            r.anim_end = v;
        }
        if let Some(v) = a.get("loop").and_then(Value::as_str) {
            r.loop_mode = v.to_string();
        }
        if let Some(v) = num("playbackSpeed") {
            r.speed = v;
        }
        if let Some(v) = num("maxPlaybackSpeed") {
            r.max_speed = v;
        }
        if let Some(v) = num("by") {
            r.by = v;
        }
        if let Some(v) = num("preroll") {
            r.preroll = v;
        }
        if let Some(v) = a.get("view").and_then(Value::as_str) {
            r.view = v.to_string();
        }
        r.anim_start = r.anim_start.min(r.min);
        r.anim_end = r.anim_end.max(r.max);
    });
    if let Some(v) = num("framesPerSecond") {
        anim::set_fps(v);
    }
    Ok(Value::Null)
}

/// Every time-based curve in the scene. Driven curves have no time in them.
fn time_curves(s: &Session) -> Vec<NodeId> {
    let g = &s.graph;
    g.nodes().into_iter().filter(|&n| anim::is_curve(g, n) && !crate::anim_edit::is_driven_curve(g, n)).collect()
}

/// The linear and angular units are saved scene settings; nothing else in the scene reads them yet.

fn current_unit(s: &mut Session, a: &Args) -> CmdResult {
    if a.bool("query") {
        if queried(a, &["time"]) {
            return Ok(json!(time_unit_name(anim::fps())));
        }
        if queried(a, &["angle"]) {
            let u = units::angle_name();
            return Ok(json!(if a.bool("fullName") { full_angle(u) } else { u }));
        }
        let u = units::linear_name();
        return Ok(json!(if a.bool("fullName") { full_linear(u) } else { u }));
    }
    if let Some(v) = a.get("linear").and_then(Value::as_str) {
        let Some(f) = units::linear_factor(v) else { fail!("currentUnit: unknown linear unit '{v}'") };
        units::set_linear(f);
    }
    if let Some(v) = a.get("angle").and_then(Value::as_str) {
        let Some(f) = units::angle_factor(v) else { fail!("currentUnit: unknown angle unit '{v}'") };
        units::set_angle(f);
    }
    if let Some(v) = a.get("time").and_then(Value::as_str) {
        let Some(new) = time_unit_fps(v) else { fail!("currentUnit: unknown time unit '{v}'") };
        let old = anim::fps();
        let update = a.get("updateAnimation").and_then(Value::as_bool).unwrap_or(true);
        if (new - old).abs() > 1e-12 {
            let frame = anim::frame(&s.graph);
            if update {
                // The keys stay at the same moments, so their frame numbers scale. Seconds don't change.
                let ratio = new / old;
                for c in time_curves(s) {
                    let mut k = anim::keys_of(&s.graph, c);
                    for key in k.keys.iter_mut() {
                        key.t *= ratio;
                        key.in_slope /= ratio;
                        key.out_slope /= ratio;
                    }
                    anim::set_keys(&mut s.graph, c, k)?;
                }
                let round = |x: f64| if (x - x.round()).abs() < 1e-6 { x.round() } else { x };
                anim::set_range(|r| {
                    r.min = round(r.min * ratio);
                    r.max = round(r.max * ratio);
                    r.anim_start = round(r.anim_start * ratio);
                    r.anim_end = round(r.anim_end * ratio);
                });
                anim::set_fps(new);
            } else {
                // The frame numbers stay; the animation runs at the new rate.
                anim::set_fps(new);
                anim::set_frame(&mut s.graph, frame);
            }
        }
    }
    Ok(Value::Null)
}

fn full_angle(v: &str) -> &'static str {
    match v {
        "rad" => "radian",
        "min" => "minute",
        "sec" => "second",
        _ => "degree",
    }
}

fn full_linear(v: &str) -> &'static str {
    match v {
        "mm" => "millimeter",
        "m" => "meter",
        "km" => "kilometer",
        "in" => "inch",
        "ft" => "foot",
        "yd" => "yard",
        "mi" => "mile",
        _ => "centimeter",
    }
}

fn time_control(s: &Session, a: &Args) -> CmdResult {
    if a.bool("query") {
        let st = slider();
        let r = anim::range();
        let (lo, hi) = st.range.unwrap_or((anim::frame(&s.graph), anim::frame(&s.graph) + 1.0));
        return Ok(if queried(a, &["rangeArray"]) {
            json!([lo, hi])
        } else if queried(a, &["range"]) {
            json!(format!("\"{lo}:{hi}\""))
        } else if queried(a, &["rangeVisible"]) {
            json!(st.range.is_some())
        } else if queried(a, &["showKeys"]) {
            json!(st.show_keys)
        } else if queried(a, &["snap"]) {
            json!(st.snap)
        } else if queried(a, &["tickSize"]) {
            json!(st.tick_size)
        } else if queried(a, &["minTime"]) {
            anim::frame_json(r.min)
        } else if queried(a, &["maxTime"]) {
            anim::frame_json(r.max)
        } else {
            Value::Null
        });
    }
    let mut st = slider();
    match a.get("rangeArray") {
        Some(Value::Array(v)) if v.len() == 2 => st.range = Some((v[0].as_f64().unwrap_or(0.0), v[1].as_f64().unwrap_or(0.0))),
        Some(Value::Array(v)) if v.is_empty() => st.range = None,
        Some(Value::Null) | None => {}
        Some(o) => fail!("timeControl: -rangeArray takes a start and an end, not {o}"),
    }
    if let Some(v) = a.get("showKeys").and_then(Value::as_str) {
        st.show_keys = v.to_string();
    }
    if let Some(v) = a.get("snap").and_then(Value::as_bool) {
        st.snap = v;
    }
    if let Some(v) = a.get("tickSize").and_then(Value::as_f64) {
        st.tick_size = v;
    }
    *SLIDER.lock().unwrap() = Some(st);
    Ok(Value::Null)
}

fn set_ghosting(s: &mut Session, a: &Args, on: Option<bool>, all: bool) -> CmdResult {
    let g = &s.graph;
    let mut ids = Vec::new();
    if all {
        ids = g.nodes().into_iter().filter(|&n| g.node(n).is_some_and(|x| x.ty().is_a("transform"))).collect();
    } else {
        for n in objects(s, a) {
            if let Some(id) = g.find(n.split('.').next().unwrap_or(&n))? {
                ids.push(id);
            }
        }
    }
    let mut out = Vec::new();
    for id in ids {
        let at = crate::transform::DagNode::GHOSTING as i32;
        s.graph.set_value(PlugRef::new(id, at), PlugValue::Bool(on.unwrap_or(false)))?;
        out.push(s.graph.path(id));
    }
    Ok(json!(out))
}

// ---------------------------------------------------------------------------------------------------------------------

pub fn register(c: &mut Commands<Session>) {
    c.register(
        query_flags(Cmd::new(
            "currentTime",
            "Set the current frame, or query it. Animated channels move to their values there. A value may carry a unit: 2sec, 12ntsc.",
            |s, a| {
                if !a.bool("query") {
                    let t = match a.positional.first().and_then(parse_time).or_else(|| a.get("time").and_then(parse_time)) {
                        Some(t) => t,
                        None => fail!("currentTime: expected a frame"),
                    };
                    anim::set_frame(&mut s.graph, t);
                    if a.get("update").and_then(Value::as_bool) != Some(false) {
                        s.graph.evaluate();
                    }
                }
                Ok(anim::frame_json(anim::frame(&s.graph)))
            },
        ))
        .positional("frame")
        .flag(f("time", "t", ArgType::Any, "The frame to go to."))
        .flag(f("update", "u", ArgType::Bool, "Whether to evaluate now (on by default); either way the scene is up to date when asked."))
        .undoable(false),
    );

    c.register(
        query_flags(Cmd::new(
            "playbackOptions",
            "The playback range (min/max), the whole animation range (ast/aet), looping and speed. In query mode returns the flag asked about.",
            |_s, a| playback_options(a),
        ))
        .flag(f("minTime", "min", ArgType::Float, "First frame of the playback range."))
        .flag(f("maxTime", "max", ArgType::Float, "Last frame of the playback range."))
        .flag(f("animationStartTime", "ast", ArgType::Float, "First frame of the whole animation."))
        .flag(f("animationEndTime", "aet", ArgType::Float, "Last frame of the whole animation."))
        .flag(f("loop", "l", ArgType::String, "continuous, once or oscillate.").choices(&["continuous", "once", "oscillate"]))
        .flag(f("playbackSpeed", "ps", ArgType::Float, "1 for real time, 0 for every frame."))
        .flag(f("maxPlaybackSpeed", "mps", ArgType::Float, "The most frames per second playback may reach, 0 for no limit."))
        .flag(f("framesPerSecond", "fps", ArgType::Float, "Frames per second (24 by default, film)."))
        .flag(f("by", "by", ArgType::Float, "Frames between played frames."))
        .flag(f("view", "v", ArgType::String, "Which views update during playback: all or active.").choices(&["all", "active"]))
        .flag(f("preroll", "pp", ArgType::Float, "Stored for scripts that set it."))
        .undoable(false),
    );

    c.register(
        query_flags(Cmd::new(
            "play",
            "Start or stop playback: -st on or off, -f forward or back. -w plays the range through once before returning. -q -st says whether it is playing.",
            play,
        ))
        .flag(f("forward", "f", ArgType::Bool, "Play forwards (on by default)."))
        .flag(f("state", "st", ArgType::Bool, "Playing or not."))
        .flag(f("wait", "w", ArgType::Bool, "Play through before returning."))
        .flag(f("record", "rec", ArgType::Bool, "Accepted for existing scripts: there are no dynamics to record."))
        .flag(f("sound", "s", ArgType::String, "Accepted for existing scripts."))
        .undoable(false),
    );

    c.register(
        query_flags(Cmd::new(
            "timeControl",
            "The time slider's state: the range selected on it (-q -ra, -q -rng, -q -rv), its key display and snapping.",
            |s, a| time_control(s, a),
        ))
        .positional("the slider's name, accepted for existing scripts")
        .flag(f("rangeArray", "ra", ArgType::Any, "The selected range as start and end; empty for none."))
        .flag(f("range", "rng", ArgType::Bool, "Query: the selected range as a string."))
        .flag(f("rangeVisible", "rv", ArgType::Bool, "Query: whether a range is selected."))
        .flag(f("showKeys", "sk", ArgType::String, "Whose keys show: active, none or a selection connection."))
        .flag(f("snap", "snp", ArgType::Bool, "Snap to whole frames."))
        .flag(f("tickSize", "ts", ArgType::Float, "How wide ticks are drawn."))
        .flag(f("minTime", "min", ArgType::Bool, "Query: the playback start."))
        .flag(f("maxTime", "max", ArgType::Bool, "Query: the playback end."))
        .flag(f("beginScrub", "bs", ArgType::Bool, "Accepted for existing scripts."))
        .flag(f("endScrub", "es", ArgType::Bool, "Accepted for existing scripts."))
        .flag(f("forceRedraw", "fr", ArgType::Bool, "Accepted for existing scripts."))
        .undoable(false),
    );

    c.register(
        query_flags(Cmd::new(
            "currentUnit",
            "The scene's units: -t time (film, pal, ntsc, game, show, palf, ntscf or Nfps), -l linear, -a angle. Changing the time unit \
             keeps keys at the same moments unless -ua is off, which keeps their frame numbers.",
            current_unit,
        ))
        .flag(f("time", "t", ArgType::String, "Time unit."))
        .flag(f("linear", "l", ArgType::String, "Linear unit: mm, cm, m, km, in, ft, yd, mi."))
        .flag(f("angle", "a", ArgType::String, "Angle unit: deg or rad."))
        .flag(f("fullName", "f", ArgType::Bool, "Query: the unit's full name."))
        .flag(f("updateAnimation", "ua", ArgType::Bool, "Keep keys at the same moments (on by default)."))
        .undoable(false),
    );

    for (name, on, all, doc) in [
        ("GhostSelected", Some(true), false, "Turn ghosting on for the selected objects."),
        ("UnghostSelected", Some(false), false, "Turn ghosting off for the selected objects."),
        ("UnghostAll", Some(false), true, "Turn ghosting off for every object."),
    ] {
        c.register(Cmd::new(name, doc, move |s, a| set_ghosting(s, a, on, all)).positional("objects (the selection when none)"));
    }
}

/// Metres in one stored unit: what's stored is centimetres, whatever the scene shows.
pub fn metres_per_unit() -> f64 {
    0.01
}
