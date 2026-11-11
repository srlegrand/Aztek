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
    for panel in ["modelPanel1", "modelPanel2", "modelPanel3", "modelPanel4"] {
        for (key, value) in [("grid", json!(on)), ("gridSpacing", json!(spacing / divisions)), ("gridSize", json!(size))] {
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

pub fn register(s: &mut Session) {
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
        "The ground plane: -s size, -sp spacing between major lines, -d divisions between them, -tgl on or off, -r back to the defaults, -df the defaults themselves. These reach the four model panels: a line every spacing / divisions, reaching -s each way. The major lines stay darker every 5 lines whatever -d says, and the axes, division line and label flags are only held. -q reads a flag.",
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
    s.register(
        q_flags(Cmd::new("ikHandleDisplayScale", "How big IK handles draw in the views: the scale, -q reads it.", |_s, a| scale_cmd(a, "ikHandleDisplayScale", "ikHandleScale")))
            .positional("the scale")
            .undoable(false),
    );
}
