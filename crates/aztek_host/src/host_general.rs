//! General commands that need the application rather than the scene: condition, disable, grid, toggleAxis, commandEcho, help,
//! jointDisplayScale and ikHandleDisplayScale. hotkeyCheck is with the hotkeys in `ui_cmds`.
//!
//! Differs from the reference. grid's size, spacing and toggle reach the views; its labels and bold axes are only held. commandEcho's
//! state echoes commands called from Python; MEL echoes its top-level statements regardless, and calls inside procedures
//! are not echoed. help -doc fails, since no documentation ships with the application.

use std::cell::RefCell;
use std::collections::BTreeMap;

use aztek_ops::{fail, ArgType, Args, CmdResult, CommandDef, FlagDef};
use aztek_scene::Session;
use serde_json::{json, Value};

use crate::core::push_ui_request;

type Cmd = CommandDef<Session>;
use ArgType::{Any as A, Bool as B, Float as Fl, Int as I, List as L, String as S};

fn f(name: &str, short: &str, ty: ArgType, doc: &str) -> FlagDef {
    FlagDef::new(name, short, ty, Value::Null, doc)
}

fn q_flags(c: Cmd) -> Cmd {
    c.flag(f("query", "q", B, "Query mode.")).flag(f("edit", "e", B, "Edit mode.")).flag(f("queried", "", L, "Flags asked about in query mode."))
}

fn asked(a: &Args, name: &str) -> bool {
    a.bool(name) || a.get("queried").and_then(Value::as_array).is_some_and(|q| q.iter().any(|x| x == name))
}

fn truth(v: &Value) -> bool {
    v.as_bool().unwrap_or(v.as_i64().map_or(!v.is_null(), |i| i != 0))
}

#[derive(Default)]
struct Condition {
    script: String,
    dependencies: Vec<String>,
    state: bool,
    /// What each dependency last was.
    seen: BTreeMap<String, bool>,
}

#[derive(Default)]
struct State {
    conditions: BTreeMap<String, Condition>,
    due: Vec<String>,
    echo_all: bool,
    echo_filter: Vec<String>,
    line_numbers: Option<bool>,
    grid: BTreeMap<String, Value>,
    grid_defaults: BTreeMap<String, Value>,
    axis_origin: Option<bool>,
    help: BTreeMap<String, Value>,
    joint_ikfk: Option<f64>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
    /// showHelp -helpTable.
    static HELP_TABLE: RefCell<String> = RefCell::new("helpTable".to_string());
}

fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with(|s| f(&mut s.borrow_mut()))
}

// ---------------------------------------------------------------------------------------------------------------------
// condition

/// A named condition's state, when there is one by that name.
pub fn condition_state(name: &str) -> Option<bool> {
    with(|s| s.conditions.get(name).map(|c| c.state))
}

/// Conditions due to run their scripts: (name, script).
pub(crate) fn take_due() -> Vec<(String, String)> {
    with(|s| {
        let names = std::mem::take(&mut s.due);
        names.into_iter().filter_map(|n| s.conditions.get(&n).map(|c| (n, c.script.clone()))).filter(|x| !x.1.is_empty()).collect()
    })
}

/// Marks conditions whose dependencies changed as due.
pub(crate) fn watch(session: &mut Session) {
    let wanted: Vec<(String, Vec<String>)> = with(|s| s.conditions.iter().filter(|(_, c)| !c.dependencies.is_empty()).map(|(n, c)| (n.clone(), c.dependencies.clone())).collect());
    for (name, deps) in wanted {
        for dep in deps {
            let now = crate::script_jobs::probe_condition(session, &dep);
            with(|s| {
                let Some(c) = s.conditions.get_mut(&name) else { return };
                let before = c.seen.insert(dep.clone(), now);
                if before.is_some_and(|b| b != now) && !s.due.contains(&name) {
                    s.due.push(name.clone());
                }
            });
        }
    }
}

pub(crate) fn set_condition(name: &str, state: bool) {
    with(|s| {
        if let Some(c) = s.conditions.get_mut(name) {
            c.state = state;
        }
    });
}

fn condition(session: &mut Session, a: &Args) -> CmdResult {
    let Some(name) = a.pos_str(0).map(str::to_string) else { fail!("condition: name it") };
    if a.bool("query") {
        let Some(state) = condition_state(&name) else { fail!("condition: no condition {name}") };
        if asked(a, "state") {
            return Ok(json!(state));
        }
        let script = with(|s| s.conditions.get(&name).map(|c| (c.script.clone(), c.dependencies.clone())));
        let (script, deps) = script.unwrap_or_default();
        if asked(a, "script") {
            return Ok(json!(script));
        }
        if asked(a, "dependency") {
            return Ok(json!(deps));
        }
        return Ok(json!(state));
    }
    if a.bool("delete") {
        let gone = with(|s| s.conditions.remove(&name).is_some());
        if !gone {
            fail!("condition: no condition {name}");
        }
        return Ok(Value::Null);
    }
    let edit = a.bool("edit");
    let exists = with(|s| s.conditions.contains_key(&name));
    if edit && !exists {
        fail!("condition: no condition {name}");
    }
    if !edit && exists {
        fail!("condition: {name} already exists");
    }
    let deps: Vec<String> = match a.get("dependency") {
        Some(Value::Array(l)) => l.iter().filter_map(Value::as_str).map(str::to_string).collect(),
        Some(Value::String(t)) => vec![t.clone()],
        _ => Vec::new(),
    };
    let seen: BTreeMap<String, bool> = deps.iter().map(|d| (d.clone(), crate::script_jobs::probe_condition(session, d))).collect();
    with(|s| {
        let c = s.conditions.entry(name.clone()).or_default();
        if let Some(t) = a.get("script").and_then(Value::as_str) {
            c.script = t.to_string();
        }
        if a.has("dependency") {
            c.dependencies = deps;
            c.seen = seen;
        }
        if let Some(v) = a.get("state") {
            c.state = truth(v);
        }
        if a.bool("initialize") {
            s.due.push(name.clone());
        }
    });
    Ok(json!(name))
}

// ---------------------------------------------------------------------------------------------------------------------
// commandEcho

/// Whether a command called from Python should be echoed.
pub(crate) fn echoes(name: &str) -> bool {
    with(|s| s.echo_all && !s.echo_filter.iter().any(|x| name.starts_with(x.as_str())))
}

fn strings(v: &Value) -> Vec<String> {
    match v {
        Value::Array(l) => l.iter().filter_map(Value::as_str).map(str::to_string).collect(),
        Value::String(t) => vec![t.clone()],
        _ => Vec::new(),
    }
}

fn command_echo(_s: &mut Session, a: &Args) -> CmdResult {
    with(|s| {
        if a.bool("query") {
            return Ok(if asked(a, "lineNumbers") {
                json!(s.line_numbers.unwrap_or(true))
            } else if asked(a, "filter") {
                json!(s.echo_filter)
            } else {
                json!(s.echo_all)
            });
        }
        if let Some(v) = a.get("state") {
            s.echo_all = truth(v);
        }
        if let Some(v) = a.get("lineNumbers") {
            s.line_numbers = Some(truth(v));
        }
        if let Some(v) = a.get("filter") {
            s.echo_filter = strings(v);
        }
        if let Some(v) = a.get("addFilter") {
            s.echo_filter.extend(strings(v));
        }
        Ok(Value::Null)
    })
}

// ---------------------------------------------------------------------------------------------------------------------
// grid

fn grid_default(name: &str) -> Value {
    match name {
        // The views' grid: 12 units each way, a line per unit, as the Preferences have it. The reference's text says
        // spacing and divisions of 10.
        "size" => json!(12.0),
        "spacing" => json!(5.0),
        "divisions" => json!(5),
        "toggle" | "displayAxes" | "displayGridLines" | "displayDivisionLines" | "displayAxesBold" => json!(true),
        "displayOrthographicLabels" | "displayPerspectiveLabels" => json!(false),
        "orthographicLabelPosition" | "perspectiveLabelPosition" => json!("axis"),
        _ => Value::Null,
    }
}

const GRID_FLAGS: &[(&str, &str, ArgType)] = &[
    ("displayAxes", "da", B),
    ("displayAxesBold", "dab", B),
    ("displayDivisionLines", "ddl", B),
    ("displayGridLines", "dgl", B),
    ("displayOrthographicLabels", "dol", B),
    ("displayPerspectiveLabels", "dpl", B),
    ("divisions", "d", I),
    ("orthographicLabelPosition", "olp", S),
    ("perspectiveLabelPosition", "plp", S),
    ("size", "s", Fl),
    ("spacing", "sp", Fl),
    ("toggle", "tgl", B),
];

fn grid_value(s: &State, name: &str) -> Value {
    s.grid.get(name).or_else(|| s.grid_defaults.get(name)).cloned().unwrap_or_else(|| grid_default(name))
}

/// Tells each model panel's view what the grid should be. Its size and spacing are in the current linear unit, as the
/// reference's are: 12 units each way is 12 metres in a metre scene.
fn send_grid(s: &State) {
    let k = aztek_ops::units::cm_per_unit();
    let spacing = grid_value(s, "spacing").as_f64().unwrap_or(5.0) * k;
    let divisions = grid_value(s, "divisions").as_f64().unwrap_or(5.0).max(1.0);
    let size = grid_value(s, "size").as_f64().unwrap_or(12.0) * k;
    let on = grid_value(s, "toggle").as_bool().unwrap_or(true);
    let mut style = serde_json::Map::new();
    for (name, _, _) in GRID_FLAGS {
        style.insert(name.to_string(), grid_value(s, name));
    }
    for panel in ["modelPanel1", "modelPanel2", "modelPanel3", "modelPanel4"] {
        for (key, value) in [("grid", json!(on)), ("gridStyle", Value::Object(style.clone())), ("gridSpacing", json!(spacing / divisions)), ("gridSize", json!(size))] {
            push_ui_request(format!("panelSet|{}", json!({"panel": panel, "key": key, "value": value})));
        }
    }
}

/// The linear unit changed: the grid is sent again at its new size.
pub(crate) fn grid_units_changed() {
    with(|s| send_grid(s));
}

fn grid(_session: &mut Session, a: &Args) -> CmdResult {
    with(|s| {
        if a.bool("query") {
            let Some(item) = GRID_FLAGS.iter().find(|x| asked(a, x.0)) else { fail!("grid: name what to query") };
            return Ok(if asked(a, "default") {
                s.grid_defaults.get(item.0).cloned().unwrap_or_else(|| grid_default(item.0))
            } else {
                grid_value(s, item.0)
            });
        }
        if a.bool("reset") {
            s.grid.clear();
        }
        let to_defaults = a.bool("default");
        for (name, _, ty) in GRID_FLAGS {
            let Some(v) = a.get(name) else { continue };
            // A MEL or Python whole number is still a distance.
            let number = v.as_f64().filter(|_| *ty == Fl).map(|x| json!(x));
            let v = number.as_ref().unwrap_or(v);
            if *name == "divisions" && v.as_f64().is_some_and(|d| d < 1.0) {
                fail!("grid: -divisions must be at least 1");
            }
            if to_defaults {
                s.grid_defaults.insert((*name).to_string(), v.clone());
            } else {
                s.grid.insert((*name).to_string(), v.clone());
            }
        }
        send_grid(s);
        Ok(Value::Null)
    })
}

// ---------------------------------------------------------------------------------------------------------------------
// help

fn arg_name(t: ArgType) -> &'static str {
    match t {
        ArgType::Bool => "on|off",
        ArgType::Int => "Int",
        ArgType::Float => "Float",
        ArgType::String => "String",
        ArgType::Vec3 => "Float Float Float",
        ArgType::List | ArgType::Any => "Any",
    }
}

fn synopsis(c: &Cmd) -> String {
    let mut out = format!("Synopsis: {} [flags]", c.name);
    if !c.positional_doc.is_empty() {
        out.push_str(&format!(" [{}]", c.positional_doc));
    }
    out.push_str("\nFlags:\n");
    let mut flags: Vec<&FlagDef> = c.flags.iter().filter(|x| !x.short_name.is_empty()).collect();
    flags.sort_by(|x, y| x.short_name.cmp(&y.short_name));
    for fl in flags {
        out.push_str(&format!("{:>6} -{} ({})\n", format!("-{}", fl.short_name), fl.name, arg_name(fl.ty)));
    }
    out
}

fn help(s: &mut Session, a: &Args) -> CmdResult {
    for (long, def) in [("popupDisplayTime", json!(4)), ("popupMode", json!(true)), ("popupPauseTime", json!(800)), ("popupSimpleMode", json!(false)), ("rolloverMode", json!(false))] {
        if a.bool("query") && asked(a, long) {
            return Ok(with(|x| x.help.get(long).cloned()).unwrap_or(def));
        }
        if let Some(v) = a.get(long).filter(|_| !a.bool("query")) {
            with(|x| x.help.insert(long.to_string(), v.clone()));
            return Ok(Value::Null);
        }
    }
    if a.bool("documentation") {
        fail!("help: no documentation ships with this application; help without -doc gives a command's flags");
    }
    if a.has("language") && !matches!(a.string("language", "").as_str(), "mel" | "python") {
        fail!("help: the language must be mel or python");
    }
    let Some(name) = a.pos_str(0).map(str::to_string) else { fail!("help: name a command") };
    if a.bool("list") {
        let mut names = Vec::new();
        for n in s.commands.all().map(|d| d.name.clone()) {
            if crate::mel::pattern::regex_find(&name, &n).map_err(aztek_ops::CommandError::new)?.is_some() {
                names.push(n);
            }
        }
        names.sort();
        return Ok(json!(names));
    }
    let Some(c) = s.commands.find(&name) else { fail!("help: no command {name}") };
    if a.bool("syntaxOnly") {
        return Ok(json!(synopsis(c)));
    }
    Ok(json!(format!("{}\n\n{}", c.doc, synopsis(c))))
}

// ---------------------------------------------------------------------------------------------------------------------
// Display scales

fn scale_cmd(a: &Args, cmd: &str, pref: &str) -> CmdResult {
    if a.bool("query") {
        return Ok(json!(crate::ui_cmds::pref_f64(pref, 1.0)));
    }
    let Some(v) = a.positional.first().and_then(Value::as_f64) else { fail!("{cmd}: give the scale") };
    if v <= 0.0 {
        fail!("{cmd}: the scale must be above 0");
    }
    crate::ui_cmds::set_pref(pref, json!(v));
    Ok(Value::Null)
}

// ---------------------------------------------------------------------------------------------------------------------
// autoPlace

/// Rays the views last gave, by the camera they look through: the one through the view's centre, and the one through the
/// pointer.
#[derive(Clone, Copy)]
pub(crate) struct ViewRays {
    pub centre: (glam::DVec3, glam::DVec3),
    pub pointer: (glam::DVec3, glam::DVec3),
}

thread_local! {
    static VIEW_RAYS: RefCell<BTreeMap<String, ViewRays>> = RefCell::new(BTreeMap::new());
}

/// A view saw the mouse: its rays, for autoPlace.
pub(crate) fn note_pointer(camera: &str, rays: ViewRays) {
    VIEW_RAYS.with(|r| {
        r.borrow_mut().insert(camera.to_string(), rays);
    });
}

fn view_rays(camera: &str, shape: &str) -> Option<ViewRays> {
    VIEW_RAYS.with(|r| {
        let r = r.borrow();
        r.get(camera).or_else(|| r.get(shape)).copied()
    })
}

/// Where a ray meets a triangle, as its distance along the ray.
fn ray_triangle(o: glam::DVec3, d: glam::DVec3, a: glam::DVec3, b: glam::DVec3, c: glam::DVec3) -> Option<f64> {
    let (e1, e2) = (b - a, c - a);
    let p = d.cross(e2);
    let det = e1.dot(p);
    if det.abs() < 1e-14 {
        return None;
    }
    let s = o - a;
    let u = s.dot(p) / det;
    if !(-1e-9..=1.0 + 1e-9).contains(&u) {
        return None;
    }
    let q = s.cross(e1);
    let v = d.dot(q) / det;
    if v < -1e-9 || u + v > 1.0 + 1e-9 {
        return None;
    }
    let t = e2.dot(q) / det;
    (t >= 0.0).then_some(t)
}

/// The nearest hit of a ray on a mesh's faces, world space.
fn ray_mesh(m: &aztek_geo::Mesh, world: &glam::DMat4, o: glam::DVec3, d: glam::DVec3) -> Option<f64> {
    let mut best: Option<f64> = None;
    for f in 0..m.face_capacity() {
        if !m.face_valid(f) {
            continue;
        }
        let pts: Vec<glam::DVec3> = m.face_points(f).iter().map(|&p| world.transform_point3(m.position(p))).collect();
        for k in 1..pts.len().saturating_sub(1) {
            if let Some(t) = ray_triangle(o, d, pts[0], pts[k], pts[k + 1]) {
                best = Some(best.map_or(t, |b: f64| b.min(t)));
            }
        }
    }
    best
}

/// autoPlace: the point at the centre of the view in focus (or under the pointer with -um) on the live surface, or on the
/// ground plane when nothing is live.
fn auto_place(s: &mut Session, a: &Args) -> CmdResult {
    let camera = crate::panel_cmds::active_camera();
    let g = &mut s.graph;
    g.evaluate();
    let Some(cam) = g.find(&camera)? else { fail!("autoPlace: no camera {camera}") };
    let tr = aztek_scene::dag::transform_of(g, cam);
    let shape = aztek_scene::dag::shapes(g, tr, false).first().map(|&x| g.path(x)).unwrap_or_default();
    let (o, d) = match view_rays(&g.path(tr), &shape).or_else(|| view_rays(&camera, &shape)) {
        Some(r) if a.bool("useMouse") => r.pointer,
        Some(r) => r.centre,
        None => {
            // No view has drawn this camera: its own axis.
            let w = aztek_scene::rig::world(g, tr);
            (w.w_axis.truncate(), -w.z_axis.truncate().normalize_or_zero())
        }
    };
    let mut best: Option<f64> = None;
    for &live in &s.live_surface.clone() {
        let g = &mut s.graph;
        let mesh = if let Some(m) = aztek_scene::components::as_mesh(g, live) {
            aztek_scene::modifiers::cage(g, m).map(|x| ((*x).clone(), aztek_scene::rig::world(g, aztek_scene::dag::transform_of(g, m))))
        } else if let Some(sf) = aztek_scene::surfaces::as_surface(g, live) {
            aztek_scene::surfaces::world_surface(g, sf).map(|x| (x.tessellate(8, 8), glam::DMat4::IDENTITY))
        } else {
            None
        };
        if let Some((m, w)) = mesh {
            if let Some(t) = ray_mesh(&m, &w, o, d) {
                best = Some(best.map_or(t, |b: f64| b.min(t)));
            }
        }
    }
    let at = match best {
        Some(t) => o + d * t,
        None if !s.live_surface.is_empty() => fail!("autoPlace: the view's centre misses the live surface"),
        None => {
            // The ground plane: Y up, or Z.
            let n = if aztek_scene::world::up_z() { glam::DVec3::Z } else { glam::DVec3::Y };
            let den = d.dot(n);
            if den.abs() < 1e-9 {
                fail!("autoPlace: the view looks along the ground plane");
            }
            let t = -o.dot(n) / den;
            o + d * t
        }
    };
    let k = if aztek_ops::units::user_call() { aztek_ops::units::to_user(aztek_ops::units::Unit::Linear, 1.0) } else { 1.0 };
    Ok(json!([at.x * k, at.y * k, at.z * k]))
}

/// overrideModifier: modifier keys forced down and forced up, as bits of the views' mouse modifiers (1 shift, 2 ctrl,
/// 4 alt).
static FORCED_DOWN: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);
static FORCED_UP: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);

/// The modifiers a view's mouse event goes on with, overrideModifier's forcing applied.
pub fn forced_modifiers(mods: i32) -> i32 {
    use std::sync::atomic::Ordering;
    (mods | FORCED_DOWN.load(Ordering::Relaxed)) & !FORCED_UP.load(Ordering::Relaxed)
}

fn modifier_bit(name: &str) -> CmdResult<i32> {
    Ok(match name.to_ascii_lowercase().as_str() {
        "shift" => 1,
        "ctrl" | "control" => 2,
        "alt" => 4,
        _ => fail!("overrideModifier: the modifiers are Alt, Ctrl and Shift, not {name}"),
    })
}

fn override_modifier(a: &Args) -> CmdResult {
    use std::sync::atomic::Ordering;
    let list = |k: &str| -> Vec<String> {
        match a.get(k) {
            Some(Value::Array(v)) => v.iter().filter_map(Value::as_str).map(str::to_string).collect(),
            Some(Value::String(x)) => vec![x.clone()],
            _ => Vec::new(),
        }
    };
    if a.bool("clear") {
        FORCED_DOWN.store(0, Ordering::Relaxed);
        FORCED_UP.store(0, Ordering::Relaxed);
    }
    for m in list("press") {
        let b = modifier_bit(&m)?;
        FORCED_DOWN.fetch_or(b, Ordering::Relaxed);
        FORCED_UP.fetch_and(!b, Ordering::Relaxed);
    }
    for m in list("release") {
        let b = modifier_bit(&m)?;
        FORCED_UP.fetch_or(b, Ordering::Relaxed);
        FORCED_DOWN.fetch_and(!b, Ordering::Relaxed);
    }
    push_ui_request(format!("overrideModifier|{}", json!({"down": FORCED_DOWN.load(Ordering::Relaxed), "up": FORCED_UP.load(Ordering::Relaxed)})));
    Ok(Value::Null)
}

/// A display filter a plug-in registered: its label, classification and how many times it was registered.
#[derive(Clone)]
struct DisplayFilter {
    label: String,
    classification: String,
    count: u32,
}

thread_local! {
    static DISPLAY_FILTERS: RefCell<Vec<(String, DisplayFilter)>> = const { RefCell::new(Vec::new()) };
}

fn plugin_display_filter(a: &Args) -> CmdResult {
    let name = a.pos_str(0).map(str::to_string);
    if asked(a, "listFilters") {
        return Ok(json!(DISPLAY_FILTERS.with(|f| f.borrow().iter().map(|(n, _)| n.clone()).collect::<Vec<_>>())));
    }
    let Some(name) = name else { fail!("pluginDisplayFilter: name a display filter") };
    let find = |n: &str| DISPLAY_FILTERS.with(|f| f.borrow().iter().find(|(k, _)| k == n).map(|(_, v)| v.clone()));
    if a.bool("exists") {
        return Ok(json!(find(&name).is_some()));
    }
    if a.bool("query") {
        let Some(d) = find(&name) else { fail!("pluginDisplayFilter: no display filter named {name}") };
        if asked(a, "classification") {
            return Ok(json!(d.classification));
        }
        return Ok(json!(d.label));
    }
    if a.bool("deregister") {
        let gone = DISPLAY_FILTERS.with(|f| {
            let mut f = f.borrow_mut();
            let Some(i) = f.iter().position(|(k, _)| *k == name) else { return Err(()) };
            f[i].1.count -= 1;
            if f[i].1.count == 0 {
                f.remove(i);
            }
            Ok(())
        });
        if gone.is_err() {
            fail!("pluginDisplayFilter: no display filter named {name}");
        }
    } else {
        // -register, which create mode means when neither is given. Registered twice, it takes two deregisters.
        let label = a.get("label").and_then(Value::as_str).map(str::to_string);
        let class = a.get("classification").and_then(Value::as_str).map(str::to_string);
        DISPLAY_FILTERS.with(|f| {
            let mut f = f.borrow_mut();
            match f.iter_mut().find(|(k, _)| *k == name) {
                Some((_, d)) => {
                    d.count += 1;
                    if let Some(l) = label {
                        d.label = l;
                    }
                    if let Some(c) = class {
                        d.classification = c;
                    }
                }
                None => f.push((name.clone(), DisplayFilter { label: label.unwrap_or_else(|| name.clone()), classification: class.unwrap_or_default(), count: 1 })),
            }
        });
    }
    let list: Vec<Value> = DISPLAY_FILTERS.with(|f| f.borrow().iter().map(|(n, d)| json!({"name": n, "label": d.label, "classification": d.classification})).collect());
    push_ui_request(format!("pluginDisplayFilters|{}", Value::Array(list)));
    Ok(json!(name))
}

pub fn register(s: &mut Session) {
    s.register(
        Cmd::new(
            "overrideModifier",
            "Make another input act as a modifier key: -p Alt, Ctrl or Shift forces it down, -r forces it up (each may be used more than once), \
             -cl forces none. The keys themselves go on working; the views take the forced ones with their mouse events.",
            |_s, a| override_modifier(a),
        )
        .flag(f("clear", "cl", B, "Force no modifier keys."))
        .flag(f("press", "p", L, "Force this modifier down: Alt, Ctrl or Shift.").multi(1))
        .flag(f("release", "r", L, "Force this modifier up: Alt, Ctrl or Shift.").multi(1))
        .undoable(false),
    );
    s.register(
        q_flags(Cmd::new(
            "pluginDisplayFilter",
            "A plug-in's display filter for the views' Show menus: -r registers one (the default) with -l a label and -cls the classification of \
             the nodes it filters; -dr deregisters it (as many times as it was registered); -ex whether one exists; -q -l, -q -cls, -q -lf every filter.",
            |_s, a| plugin_display_filter(a),
        ))
        .positional("the filter's name")
        .flag(f("register", "r", B, "Register the filter (what create mode does without -dr)."))
        .flag(f("deregister", "dr", B, "Deregister the filter."))
        .flag(f("exists", "ex", B, "Whether the filter exists."))
        .flag(f("label", "l", S, "The label the Show menus give it; its name by default."))
        .flag(f("classification", "cls", S, "The classification of the nodes it filters, as the plug-in registered them; empty filters nothing."))
        .flag(f("listFilters", "lf", B, "With -q: every display filter."))
        .undoable(false),
    );
    s.register(
        Cmd::new("autoPlace", "The point at the centre of the view in focus on the live surface, or on the ground plane when nothing is live; -um the point under the mouse. Returns x y z.", auto_place)
            .flag(f("useMouse", "um", B, "The mouse's place in the view rather than its centre.")),
    );
    s.register(
        Cmd::new(
            "quit",
            "Exit the application. -f quits without asking to save; -a also leaves the preferences unsaved; -ec is the exit code (0).",
            |s, a| {
                let code = a.int("exitCode", 0) as i32;
                let force = a.bool("force") || a.bool("abort");
                if aztek_core::batch_mode() {
                    crate::native_plugin::messages::scene(s, crate::native_plugin::messages::MAYA_EXITING, json!({}));
                    std::process::exit(code);
                }
                push_ui_request(format!("quit|{}", json!({"force": force, "abort": a.bool("abort"), "exitCode": code})));
                Ok(Value::Null)
            },
        )
        .flag(f("force", "f", B, "Quit without asking to save."))
        .flag(f("abort", "a", B, "Quit without saving anything, preferences included."))
        .flag(f("exitCode", "ec", I, "The exit code.")),
    );
    s.register(
        q_flags(Cmd::new(
            "condition",
            "A named condition isTrue and scriptJob -ct can read: -s the MEL script that gives its state, -d conditions it follows (checked at idle), -i run the script now, -st set the state, -delete remove it. -q -st reads it. Scripts run just after the command, or at idle when a dependency changes. Not undoable, and not saved.",
            condition,
        ))
        .positional("its name")
        .flag(f("delete", "delete", B, "Delete it."))
        .flag(f("dependency", "d", A, "A condition it follows.").multi(1))
        .flag(f("initialize", "i", B, "Run its script now."))
        .flag(f("script", "s", A, "The MEL script giving its state."))
        .flag(f("state", "st", A, "Its state."))
        .undoable(false),
    );
    s.register(
        Cmd::new("disable", "Enable or disable a control: -v true (the default) disables it, false enables it.", |s, a| {
            let Some(name) = a.pos_str(0).map(str::to_string) else { fail!("disable: name a control") };
            let off = a.get("value").map_or(true, truth);
            s.run("control", json!({"args": [name], "edit": true, "enable": !off}))?;
            Ok(Value::Null)
        })
        .positional("the control")
        .flag(f("value", "v", B, "True disables, false enables."))
        .undoable(false),
    );
    let mut g = q_flags(Cmd::new(
        "grid",
        "The ground plane: -s size, -sp spacing between major lines, -d divisions between them, -tgl on or off, -r back to the defaults, -df the defaults themselves. These reach the four model panels: a line every spacing / divisions, reaching -s each way. -da, -dab, -dgl and -ddl show the axes, draw them thicker, and show the grid lines and the subdivision lines; -dpl and -dol number the grid lines in the perspective and orthographic views, along the axes or the edge (-plp, -olp). The colours are displayRGBColor's gridAxis, gridHighlight and gridColor. -q reads a flag.",
        grid,
    ))
    .flag(f("default", "df", B, "Set or read the defaults."))
    .flag(f("reset", "r", B, "Back to the defaults."))
    .flag(f("style", "st", I, "Obsolete in the reference; has no effect."))
    .undoable(false);
    for (long, short, ty) in GRID_FLAGS {
        g = g.flag(f(long, short, *ty, "A grid setting."));
    }
    s.register(g);
    s.register(
        q_flags(Cmd::new("toggleAxis", "The display axes: -v the one at the bottom left of each view, which is the viewAxis heads up display; -o the one at the origin, which is only held. -q reads them.", |_s, a| {
            if a.bool("query") {
                return Ok(if asked(a, "origin") { json!(with(|x| x.axis_origin.unwrap_or(true))) } else { json!(crate::ui_cmds::hud_on("viewAxis")) });
            }
            if let Some(v) = a.get("origin") {
                with(|x| x.axis_origin = Some(truth(v)));
            }
            if let Some(v) = a.get("view") {
                crate::ui_cmds::set_hud("viewAxis", truth(v));
            }
            Ok(Value::Null)
        }))
        .flag(f("origin", "o", B, "The axis at the origin."))
        .flag(f("view", "v", B, "The axis at the bottom left."))
        .undoable(false),
    );
    s.register(
        q_flags(Cmd::new("commandEcho", "What is echoed: -st on echoes commands called from Python too, -f and -af filter them by how their names start. -ln is only held: messages have no line numbers to show or hide here. -q reads them.", command_echo))
            .flag(f("addFilter", "af", A, "Names to add to the filter."))
            .flag(f("filter", "f", A, "Names to filter out."))
            .flag(f("lineNumbers", "ln", B, "Line numbers in messages."))
            .flag(f("state", "st", B, "Echo everything."))
            .undoable(false),
    );
    s.register(
        q_flags(Cmd::new("help", "A command's description and flags, from what it registered: help name; -so only the synopsis; -l a regular expression, listing the commands that match. -doc fails, since no documentation ships. The popup and rollover settings are only held.", help))
            .positional("a command, or a pattern with -l")
            .flag(f("documentation", "doc", B, "Not available: fails."))
            .flag(f("language", "lng", S, "mel or python, with -doc."))
            .flag(f("list", "l", B, "List the commands matching."))
            .flag(f("popupDisplayTime", "pdt", I, "Held only."))
            .flag(f("popupMode", "pm", A, "Held only."))
            .flag(f("popupPauseTime", "ppt", I, "Held only."))
            .flag(f("popupSimpleMode", "psm", A, "Held only."))
            .flag(f("rolloverMode", "rm", A, "Held only."))
            .flag(f("syntaxOnly", "so", B, "Only the synopsis."))
            .undoable(false),
    );
    s.register(
        q_flags(Cmd::new("repeatLast", "The recent commands Edit > Repeat runs: -ac a script to add as the latest, -acl its label; on its own repeats the latest, -i the one that many back; -q -cl and -cnl the scripts and labels (up to the number given), -q -nhi how many; -hl how many are kept.", |_s, a| {
            if a.bool("query") {
                let q = a.get("queried").map(|v| v.to_string()).unwrap_or_default();
                return Ok(REPEAT.with(|r| {
                    let r = r.borrow();
                    if q.contains("numberOfHistoryItems") {
                        json!(r.items.len())
                    } else if q.contains("historyLimit") {
                        json!(r.limit)
                    } else {
                        let n = a.get("commandNameList").or_else(|| a.get("commandList")).and_then(Value::as_i64).map(|n| n.max(0) as usize).unwrap_or(usize::MAX);
                        let names = q.contains("commandNameList") || a.has("commandNameList");
                        json!(r.items.iter().rev().take(n).map(|x| if names { x.1.clone() } else { x.0.clone() }).collect::<Vec<_>>())
                    }
                }));
            }
            if let Some(n) = a.get("historyLimit").and_then(Value::as_i64) {
                REPEAT.with(|r| {
                    let mut r = r.borrow_mut();
                    r.limit = n.max(1) as usize;
                    let lim = r.limit;
                    let len = r.items.len();
                    if len > lim {
                        r.items.drain(..len - lim);
                    }
                });
            }
            if let Some(cmd) = a.get("addCommand").and_then(Value::as_str) {
                let label = a.get("addCommandLabel").and_then(Value::as_str).unwrap_or(cmd).to_string();
                REPEAT.with(|r| {
                    let mut r = r.borrow_mut();
                    r.items.retain(|x| x.0 != cmd);
                    r.items.push((cmd.to_string(), label.clone()));
                    let lim = r.limit;
                    let len = r.items.len();
                    if len > lim {
                        r.items.drain(..len - lim);
                    }
                });
                push_ui_request(format!("repeatAdd|{}", json!({"command": cmd, "label": label})));
                return Ok(Value::Null);
            }
            if a.has("historyLimit") {
                return Ok(Value::Null);
            }
            // Repeat: the item that many back, or the latest.
            let back = a.get("item").and_then(Value::as_i64).unwrap_or(0).max(0) as usize;
            let cmd = REPEAT.with(|r| r.borrow().items.iter().rev().nth(back).map(|x| x.0.clone()));
            match cmd {
                Some(c) => push_ui_request(format!("mel|{c}")),
                None => push_ui_request("repeatLast".into()),
            }
            Ok(Value::Null)
        }))
        .undoable(false)
        .flag(f("addCommand", "ac", A, "A script to add as the latest command."))
        .flag(f("addCommandLabel", "acl", A, "Its label in Recent Commands."))
        .flag(f("commandList", "cl", I, "With -q: the scripts, latest first."))
        .flag(f("commandNameList", "cnl", I, "With -q: the labels, latest first."))
        .flag(f("historyLimit", "hl", I, "How many are kept."))
        .flag(f("item", "i", I, "Repeat the one this many back."))
        .flag(f("numberOfHistoryItems", "nhi", B, "With -q: how many there are.")),
    );
    s.register(renderer_cmd());
    s.register(
        q_flags(Cmd::new("preferredRenderer", "The renderer new scenes start with: preferredRenderer name sets it, -q reads it, -mc takes the current one, -f the fallback when the preferred isn't there.", |s, a| {
            let get = |s: &Session, k: &str, d: &str| s.option_vars.get(k).and_then(Value::as_str).unwrap_or(d).to_string();
            if a.bool("query") {
                let q = a.get("queried").map(|v| v.to_string()).unwrap_or_default();
                return Ok(json!(if q.contains("fallback") { get(s, "preferredRendererFallback", "arnold") } else { get(s, "preferredRenderer", "arnold") }));
            }
            if let Some(f) = a.get("fallback").and_then(Value::as_str) {
                s.option_vars.insert("preferredRendererFallback".into(), json!(f));
            }
            if a.bool("makeCurrent") {
                let cur = s.run("getAttr", json!({"args": ["defaultRenderGlobals.currentRenderer"]})).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_else(|| "arnold".into());
                s.option_vars.insert("preferredRenderer".into(), json!(cur));
            } else if let Some(n) = a.pos_str(0) {
                if !RENDERERS.with(|r| r.borrow().contains_key(n)) {
                    fail!("preferredRenderer: {n} is not a registered renderer.");
                }
                s.option_vars.insert("preferredRenderer".into(), json!(n));
            }
            Ok(Value::Null)
        }))
        .positional("[renderer]")
        .undoable(false)
        .flag(f("fallback", "f", A, "The renderer used when the preferred one isn't there."))
        .flag(f("makeCurrent", "mc", B, "The current renderer becomes the preferred one.")),
    );
    s.register(
        Cmd::new("listAnimatable", "The animatable (keyable) attributes of the objects named or selected and the shapes under them, as full plug paths; -typ their node types instead; -m and -mh only those the current manipulator moves.", |s, a| {
            let mut names = a.named();
            if names.is_empty() {
                names = s.run("ls", json!({"selection": true, "long": true}))?.as_array().cloned().unwrap_or_default().iter().filter_map(|v| v.as_str().map(str::to_string)).collect();
            }
            let mut nodes: Vec<String> = Vec::new();
            for n in names {
                let obj = n.split('.').next().unwrap_or(&n).to_string();
                let long = s.run("ls", json!({"args": [obj], "long": true}))?.as_array().and_then(|v| v.first().cloned()).and_then(|v| v.as_str().map(str::to_string)).unwrap_or(obj);
                if !nodes.contains(&long) {
                    nodes.push(long.clone());
                }
                for sh in s.run("listRelatives", json!({"args": [long], "shapes": true, "fullPath": true})).ok().and_then(|v| v.as_array().cloned()).unwrap_or_default() {
                    if let Some(sh) = sh.as_str() {
                        if !nodes.iter().any(|x| x == sh) {
                            nodes.push(sh.to_string());
                        }
                    }
                }
            }
            if a.bool("type") {
                let mut types: Vec<String> = Vec::new();
                for n in &nodes {
                    if let Some(t) = s.run("nodeType", json!({"args": [n]})).ok().and_then(|v| v.as_str().map(str::to_string)) {
                        if !types.contains(&t) {
                            types.push(t);
                        }
                    }
                }
                return Ok(json!(types));
            }
            // With the move, rotate or scale tool current, -m and -mh keep only what it moves.
            let manip = (a.bool("manip") || a.bool("manipHandle")).then(|| match crate::ctx_cmds::current().as_str() {
                "moveSuperContext" | "Move" => Some("translate"),
                "RotateSuperContext" | "Rotate" => Some("rotate"),
                "scaleSuperContext" | "Scale" => Some("scale"),
                _ => None,
            }).flatten();
            let mut out = Vec::new();
            for n in &nodes {
                for attr in s.run("listAttr", json!({"args": [n], "keyable": true})).ok().and_then(|v| v.as_array().cloned()).unwrap_or_default() {
                    let Some(attr) = attr.as_str() else { continue };
                    if manip.is_some_and(|m| !attr.starts_with(m)) {
                        continue;
                    }
                    out.push(format!("{n}.{attr}"));
                }
            }
            Ok(json!(out))
        })
        .positional("[objects]")
        .undoable(false)
        .flag(f("active", "act", B, "Obsolete."))
        .flag(f("manip", "m", B, "Only what the current manipulator moves."))
        .flag(f("manipHandle", "mh", B, "Only what the current manipulator handle moves."))
        .flag(f("shape", "s", B, "Obsolete."))
        .flag(f("type", "typ", B, "Node types instead of attributes.")),
    );
    s.register(
        q_flags(Cmd::new("showHelp", "Opens help in the web browser: -a a full address as given; otherwise Aztek's documentation, -d a page under it, or a topic. -q gives the address it would open; -v the reference version it follows. -ht is held.", |_s, a| {
            const DOCS: &str = "https://github.com/srlegrand/Aztek";
            if a.bool("query") {
                if a.bool("version") || a.get("queried").is_some_and(|q| q.to_string().contains("version")) {
                    return Ok(json!(aztek_core::REFERENCE_VERSION));
                }
                if a.get("queried").is_some_and(|q| q.to_string().contains("helpTable")) {
                    return Ok(json!(HELP_TABLE.with(|h| h.borrow().clone())));
                }
            }
            if let Some(t) = a.get("helpTable").and_then(Value::as_str) {
                HELP_TABLE.with(|h| *h.borrow_mut() = t.to_string());
                return Ok(Value::Null);
            }
            let what = a.get("docs").and_then(Value::as_str).or_else(|| a.get("absolute").and_then(Value::as_str)).or_else(|| a.pos_str(0)).unwrap_or_default().to_string();
            // Aztek's documentation is its README and docs folder: a page or topic opens the README, searched for it.
            let url = if a.has("absolute") || what.contains("://") {
                what
            } else if what.is_empty() {
                format!("{DOCS}#readme")
            } else {
                format!("{DOCS}/search?q={}&type=code", what.replace(' ', "+"))
            };
            if a.bool("query") {
                return Ok(json!(url));
            }
            crate::core::push_ui_request(format!("openUrl|{url}"));
            Ok(Value::Null)
        }))
        .positional("[topic]")
        .flag(f("absolute", "a", A, "A full address, opened as given."))
        .flag(f("docs", "d", A, "A page under the documentation."))
        .flag(f("helpTable", "ht", A, "The table topics are looked up in (held)."))
        .flag(f("version", "v", B, "The reference version followed."))
        .undoable(false),
    );
    s.register(
        q_flags(Cmd::new("mayaDpiSetting", "Interface scaling, from the next start: -m 0 by the system's DPI, 1 by -sv (1, 1.25, 1.5 or 2), 2 none. -q -m, -sv, -rsv (the scale in effect) and -sd (the system DPI).", |s, a| {
            // Kept as optionVars, which the application reads before it opens its windows.
            let mode = s.option_vars.get("aztekDpiMode").and_then(|v| v.as_i64()).unwrap_or(0);
            let scale = s.option_vars.get("aztekDpiScale").and_then(|v| v.as_f64()).unwrap_or(1.0);
            let sys = std::env::var("AZTEK_SYSTEM_DPI").ok().and_then(|v| v.parse::<f64>().ok()).unwrap_or(96.0);
            if a.bool("query") {
                let q = a.get("queried").map(|v| v.to_string()).unwrap_or_default();
                return Ok(if q.contains("realScaleValue") {
                    json!(match mode {
                        0 => sys / 96.0,
                        1 => scale * sys / 96.0,
                        _ => 1.0,
                    })
                } else if q.contains("scaleValue") {
                    json!(if mode == 1 { scale } else { 1.0 })
                } else if q.contains("systemDpi") {
                    json!(sys.round() as i64)
                } else {
                    json!(mode)
                });
            }
            if let Some(m) = a.get("mode").and_then(Value::as_i64) {
                if !(0..=2).contains(&m) {
                    fail!("mayaDpiSetting: -mode takes 0, 1 or 2.");
                }
                s.option_vars.insert("aztekDpiMode".into(), json!(m));
            }
            if let Some(v) = a.get("scaleValue").and_then(Value::as_f64) {
                if ![1.0, 1.25, 1.5, 2.0].contains(&v) {
                    fail!("mayaDpiSetting: -scaleValue takes 1.0, 1.25, 1.5 or 2.0.");
                }
                s.option_vars.insert("aztekDpiScale".into(), json!(v));
            }
            Ok(Value::Null)
        }))
        .flag(f("mode", "m", I, "0 system DPI, 1 custom, 2 none."))
        .flag(f("scaleValue", "sv", Fl, "The custom scale."))
        .flag(f("realScaleValue", "rsv", B, "The scale in effect."))
        .flag(f("systemDpi", "sd", B, "The system's DPI."))
        .undoable(false),
    );
    s.register(
        q_flags(Cmd::new("jointDisplayScale", "How big joints draw in the views: the scale, -q reads it. -a takes it as the size, the same number here since the default size is 1. -ik is only held: joints of ik/fk skeletons draw at the common scale.", |_s, a| {
            if let Some(v) = a.get("ikfk").filter(|_| !a.bool("query")) {
                with(|x| x.joint_ikfk = v.as_f64());
                return Ok(Value::Null);
            }
            if a.bool("query") && asked(a, "ikfk") {
                return Ok(json!(with(|x| x.joint_ikfk.unwrap_or(1.0))));
            }
            scale_cmd(a, "jointDisplayScale", "jointScale")
        }))
        .positional("the scale")
        .flag(f("absolute", "a", B, "The number is the size."))
        .flag(f("ikfk", "ik", A, "The size of ik/fk skeleton joints. Held only."))
        .undoable(false),
    );
    undo_info_print(s);
    s.register(
        q_flags(Cmd::new("ikHandleDisplayScale", "How big IK handles draw in the views: the scale, -q reads it.", |_s, a| scale_cmd(a, "ikHandleDisplayScale", "ikHandleScale")))
            .positional("the scale")
            .undoable(false),
    );
}


/// One registered renderer: its procedures and settings by flag, its globals nodes and its Render Settings tabs.
#[derive(Default, Clone)]
struct RendererEntry {
    props: serde_json::Map<String, Value>,
    globals: Vec<String>,
    /// (label, create proc, update proc).
    tabs: Vec<(String, String, String)>,
    capabilities: serde_json::Map<String, Value>,
}

/// repeatLast's recent commands, oldest first, and how many are kept.
struct RepeatQueue {
    items: Vec<(String, String)>,
    limit: usize,
}

thread_local! {
    static REPEAT: RefCell<RepeatQueue> = const { RefCell::new(RepeatQueue { items: Vec::new(), limit: 30 }) };
    static RENDERERS: RefCell<std::collections::BTreeMap<String, RendererEntry>> = RefCell::new(builtin_renderers());
    static RENDERER_ORDER: RefCell<Vec<String>> = RefCell::new(vec!["arnold".to_string()]);
}

/// Harold, under the name the reference's Arnold has, so scenes and scripts that name it find it.
fn builtin_renderers() -> std::collections::BTreeMap<String, RendererEntry> {
    let mut m = std::collections::BTreeMap::new();
    let mut e = RendererEntry::default();
    e.props.insert("rendererUIName".into(), json!("Harold"));
    e.props.insert("supportColorManagement".into(), json!(true));
    e.globals = vec!["defaultRenderGlobals".into(), "defaultResolution".into(), "defaultArnoldRenderOptions".into()];
    m.insert("arnold".to_string(), e);
    m
}

const RENDERER_PROCS: &[(&str, &str)] = &[
    ("batchRenderOptionsProcedure", "bro"), ("batchRenderOptionsStringProcedure", "bso"), ("batchRenderProcedure", "br"), ("cancelBatchRenderProcedure", "cbr"),
    ("changeIprRegionProcedure", "cir"), ("commandRenderProcedure", "cr"), ("iprOptionsMenuLabel", "ipm"), ("iprOptionsProcedure", "io"),
    ("iprOptionsSubMenuProcedure", "ips"), ("iprRenderProcedure", "ipr"), ("iprRenderSubMenuProcedure", "irs"), ("isRunningIprProcedure", "isr"),
    ("logoCallbackProcedure", "lgc"), ("logoImageName", "log"), ("pauseIprRenderProcedure", "psi"), ("polyPrelightProcedure", "pp"),
    ("refreshIprRenderProcedure", "rfi"), ("renderDiagnosticsProcedure", "rd"), ("renderGlobalsProcedure", "rg"), ("renderMenuProcedure", "rm"),
    ("renderOptionsProcedure", "ro"), ("renderProcedure", "r"), ("renderRegionProcedure", "rr"), ("renderSequenceProcedure", "rs"),
    ("rendererUIName", "ui"), ("renderingEditorsSubMenuProcedure", "res"), ("showBatchRenderLogProcedure", "brl"), ("showBatchRenderProcedure", "sbr"),
    ("showRenderLogProcedure", "srl"), ("startIprRenderProcedure", "sti"), ("stopIprRenderProcedure", "spi"), ("textureBakingProcedure", "tb"),
];

fn renderer_cmd() -> Cmd {
    let mut c = Cmd::new(
        "renderer",
        "The registry of renderers: renderer name registers one (or edits it) with its procedures and settings, -q reads them; -ex whether it is registered, -ava the names of all, -gn its globals nodes, -gtl -gtc -gtu its Render Settings tabs, -unr removes it.",
        |_s, a| {
            let name = a.pos_str(0).map(str::to_string);
            let q = a.bool("query");
            let asked_flag = a.get("queried").and_then(Value::as_array).and_then(|v| v.first()).and_then(Value::as_str).map(str::to_string);
            let wants = |k: &str| a.bool(k) || asked_flag.as_deref() == Some(k);
            if wants("namesOfAvailableRenderers") {
                return Ok(json!(RENDERER_ORDER.with(|o| o.borrow().clone())));
            }
            if wants("materialViewRendererList") {
                return Ok(json!([]));
            }
            let Some(name) = name else { fail!("renderer: name a renderer.") };
            if wants("exists") {
                return Ok(json!(RENDERERS.with(|r| r.borrow().contains_key(&name))));
            }
            if wants("unregisterRenderer") {
                RENDERERS.with(|r| r.borrow_mut().remove(&name));
                RENDERER_ORDER.with(|o| o.borrow_mut().retain(|n| *n != name));
                return Ok(Value::Null);
            }
            if q {
                let Some(e) = RENDERERS.with(|r| r.borrow().get(&name).cloned()) else { fail!("renderer: {name} is not a registered renderer.") };
                let k = asked_flag.unwrap_or_default();
                return Ok(match k.as_str() {
                    "globalsNodes" | "addGlobalsNode" => json!(e.globals),
                    "globalsTabLabels" => json!(e.tabs.iter().map(|t| t.0.clone()).collect::<Vec<_>>()),
                    "globalsTabCreateProcNames" => json!(e.tabs.iter().map(|t| t.1.clone()).collect::<Vec<_>>()),
                    "globalsTabUpdateProcNames" => json!(e.tabs.iter().map(|t| t.2.clone()).collect::<Vec<_>>()),
                    "capability" => {
                        let cap = a.get("capability").and_then(Value::as_str).unwrap_or("").to_lowercase();
                        e.capabilities.get(&cap).cloned().unwrap_or(json!(""))
                    }
                    "supportColorManagement" | "supportUFECameras" | "materialViewRendererPause" | "materialViewRendererSuspend" => e.props.get(&k).cloned().unwrap_or(json!(false)),
                    _ => e.props.get(&k).cloned().unwrap_or(json!("")),
                });
            }
            // Create or edit: registered on first use, then each flag given is kept.
            if wants("globalsNodes") || wants("globalsTabLabels") || wants("globalsTabCreateProcNames") || wants("globalsTabUpdateProcNames") {
                let e = RENDERERS.with(|r| r.borrow().get(&name).cloned()).unwrap_or_default();
                return Ok(if wants("globalsNodes") { json!(e.globals) } else if wants("globalsTabLabels") { json!(e.tabs.iter().map(|t| t.0.clone()).collect::<Vec<_>>()) } else if wants("globalsTabCreateProcNames") { json!(e.tabs.iter().map(|t| t.1.clone()).collect::<Vec<_>>()) } else { json!(e.tabs.iter().map(|t| t.2.clone()).collect::<Vec<_>>()) });
            }
            RENDERERS.with(|r| {
                let mut r = r.borrow_mut();
                let e = r.entry(name.clone()).or_default();
                for (k, _) in RENDERER_PROCS {
                    if let Some(v) = a.get(k) {
                        e.props.insert(k.to_string(), v.clone());
                    }
                }
                for k in ["supportColorManagement", "supportUFECameras", "materialViewRendererPause", "materialViewRendererSuspend"] {
                    if a.has(k) {
                        e.props.insert(k.to_string(), json!(a.bool(k)));
                    }
                }
                if let Some(n) = a.get("addGlobalsNode").and_then(Value::as_str) {
                    if !e.globals.iter().any(|g| g == n) {
                        e.globals.push(n.to_string());
                    }
                }
                if let Some(Value::Array(v)) = a.get("addGlobalsTab") {
                    let s: Vec<String> = v.iter().map(|x| x.as_str().unwrap_or("").to_string()).collect();
                    if s.len() == 3 {
                        e.tabs.push((s[0].clone(), s[1].clone(), s[2].clone()));
                    }
                }
                if let (Some(cap), Some(val)) = (a.get("capability").and_then(Value::as_str), a.get("capabilityValue")) {
                    e.capabilities.insert(cap.to_lowercase(), val.clone());
                }
            });
            RENDERER_ORDER.with(|o| {
                let mut o = o.borrow_mut();
                if !o.contains(&name) {
                    o.push(name.clone());
                }
            });
            Ok(Value::Null)
        },
    )
    .positional("renderer")
    .undoable(false)
    .flag(f("query", "q", B, "Query."))
    .flag(f("edit", "e", B, "Edit."))
    .flag(f("queried", "", L, "What -q asks for."));
    for (k, short) in RENDERER_PROCS {
        c = c.flag(f(k, short, A, "A procedure or setting of the renderer."));
    }
    for (k, short) in [("exists", "ex"), ("globalsNodes", "gn"), ("globalsTabCreateProcNames", "gtc"), ("globalsTabLabels", "gtl"), ("globalsTabUpdateProcNames", "gtu"), ("materialViewRendererList", "mvl"), ("namesOfAvailableRenderers", "ava"), ("unregisterRenderer", "unr")] {
        c = c.flag(f(k, short, B, "See the description."));
    }
    for (k, short) in [("supportColorManagement", "scm"), ("supportUFECameras", "suc"), ("materialViewRendererPause", "mvp"), ("materialViewRendererSuspend", "mvs")] {
        c = c.flag(f(k, short, A, "A switch of the renderer."));
    }
    c.flag(f("addGlobalsNode", "agn", A, "A render settings node the renderer uses."))
        .flag(f("addGlobalsTab", "agt", A, "A Render Settings tab: label, create procedure, update procedure.").multi(3))
        .flag(f("capability", "cap", A, "A named capability."))
        .flag(f("capabilityValue", "cpv", A, "The capability's value."))
}

// ---------------------------------------------------------------------------------------------------------------------
// connectDynamic's script handlers

/// Python functions given to connectDynamic -addScriptHandler, by the number it returned.
static DYNAMIC_HANDLERS: std::sync::Mutex<Vec<(i64, Value)>> = std::sync::Mutex::new(Vec::new());
static NEXT_HANDLER: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(1);

/// Commands the host answers before the scene does: connectDynamic's handlers, which run Python.
pub(crate) fn intercept(core: &mut crate::core::Core, name: &str, call: &Value) -> Option<Result<Value, String>> {
    if let Some(r) = crate::sys_more::intercept(core, name, call) {
        return Some(r);
    }
    if let Some(r) = crate::sys_info::intercept(core, name, call) {
        return Some(r);
    }
    if let Some(r) = crate::channel_box::intercept(core, name, call) {
        return Some(r);
    }
    if let Some(r) = crate::cmd_fields::intercept(core, name, call) {
        return Some(r);
    }
    if let Some(r) = crate::command_line::intercept(core, name, call) {
        return Some(r);
    }
    if let Some(r) = crate::attr_menus::intercept(core, name, call) {
        return Some(r);
    }
    if name == "connectDynamic" {
        return connect_dynamic(core, call);
    }
    if name == "callbacks" {
        return crate::sys_host::execute_callbacks(core, call);
    }
    None
}

fn connect_dynamic(core: &mut crate::core::Core, call: &Value) -> Option<Result<Value, String>> {
    let o = call.as_object()?;
    let others = o.keys().any(|k| !matches!(k.as_str(), "addScriptHandler" | "removeScriptHandler" | "args") );
    if let Some(cb) = o.get("addScriptHandler").filter(|v| !v.is_null()) {
        if others || o.contains_key("removeScriptHandler") {
            return Some(Err("connectDynamic: -addScriptHandler can't be used with other flags".into()));
        }
        let id = NEXT_HANDLER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        DYNAMIC_HANDLERS.lock().unwrap_or_else(|e| e.into_inner()).push((id, cb.clone()));
        return Some(Ok(json!(id)));
    }
    if let Some(id) = o.get("removeScriptHandler").filter(|v| !v.is_null()) {
        if others {
            return Some(Err("connectDynamic: -removeScriptHandler can't be used with other flags".into()));
        }
        // An unknown number does nothing.
        let id = id.as_i64().unwrap_or(0);
        DYNAMIC_HANDLERS.lock().unwrap_or_else(|e| e.into_inner()).retain(|h| h.0 != id);
        return Some(Ok(Value::Null));
    }
    let handlers: Vec<(i64, Value)> = DYNAMIC_HANDLERS.lock().unwrap_or_else(|e| e.into_inner()).clone();
    if handlers.is_empty() {
        return None;
    }
    let list = |k: &str| -> Value {
        match o.get(k) {
            Some(Value::Array(v)) => Value::Array(v.clone()),
            Some(Value::String(x)) => json!([x]),
            _ => json!([]),
        }
    };
    let mut objects: Vec<Value> = o.get("args").and_then(Value::as_array).cloned().unwrap_or_default();
    if objects.is_empty() {
        objects = core.s.graph.selection().iter().map(|&id| json!(core.s.graph.path(id))).collect();
    }
    let kwargs = json!({"fields": list("fields"), "emitters": list("emitters"), "collisionObjects": list("collisions"), "objects": objects});
    // The first handler to say it dealt with the call ends it; with none, the command runs as usual.
    for (_, cb) in handlers {
        match handler_value(core, &cb, &kwargs) {
            Ok(v) if v.as_bool() == Some(true) || v.as_i64().is_some_and(|x| x != 0) => return Some(Ok(json!(""))),
            Ok(_) => {}
            Err(e) => return Some(Err(e)),
        }
    }
    None
}

#[cfg(feature = "python")]
fn handler_value(core: &mut crate::core::Core, cb: &Value, kwargs: &Value) -> Result<Value, String> {
    if let Some(id) = cb.get("__pycb__").and_then(Value::as_i64) {
        return crate::python::fire_value_kw(core, id, kwargs);
    }
    // A function named by a string, or Python source for one.
    let name = cb.as_str().or_else(|| cb.get("__pysrc__").and_then(Value::as_str)).unwrap_or_default();
    if name.is_empty() {
        return Ok(Value::Null);
    }
    let text = serde_json::to_string(&kwargs.to_string()).unwrap_or_default();
    crate::core::python_from_mel(core, &format!("{name}(**__import__('json').loads({text}))"))
}

#[cfg(not(feature = "python"))]
fn handler_value(_core: &mut crate::core::Core, _cb: &Value, _kwargs: &Value) -> Result<Value, String> {
    Err("connectDynamic: this build has no Python for script handlers".into())
}

/// undoInfo -q -printQueue and -printRedoQueue: the undo queue, or what redo would replay in order, printed to the
/// Script Editor a line a step.
fn undo_info_print(s: &mut Session) {
    let Some(mut cmd) = s.commands.find("undoInfo").cloned() else { return };
    let run = cmd.run.clone();
    for (name, short, doc) in [("printQueue", "pq", "With -q: print the undo queue to the Script Editor."), ("printRedoQueue", "prq", "With -q: print the redo queue to the Script Editor.")] {
        cmd.flags.retain(|x| x.name != name);
        cmd = cmd.flag(f(name, short, B, doc));
    }
    cmd.run = std::sync::Arc::new(move |s: &mut Session, a: &Args| {
        let asked = |k: &str| a.bool(k) || a.get("queried").and_then(Value::as_array).is_some_and(|q| q.iter().any(|x| x == k));
        if a.bool("query") && (asked("printQueue") || asked("printRedoQueue")) {
            let u = s.graph.undo_stack();
            let steps = u.steps();
            let done = u.undo_count();
            let list: Vec<String> = if asked("printQueue") { steps[..done].iter().map(|x| x.label.clone()).collect() } else { steps[done..].iter().map(|x| x.label.clone()).collect() };
            for (i, l) in list.iter().enumerate() {
                crate::native_plugin::defer(format!("# {i}: {l}"), crate::core::LineKind::Info);
            }
            return Ok(Value::Null);
        }
        run(s, a)
    });
    s.register(cmd);
}
