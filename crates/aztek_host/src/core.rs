//! The application core the UI talks to: the session, the script history and a change counter the panels poll.

use std::sync::Mutex;

use aztek_ops::{execute, CallSource};
use aztek_scene::Session;
use serde_json::{json, Map, Value};

use crate::mel;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LineKind {
    Echo = 0,
    Result = 1,
    Error = 2,
    Info = 3,
    Warning = 4,
}

#[derive(Clone)]
pub struct LogLine {
    pub text: String,
    pub kind: LineKind,
}

/// Warnings the scene code logs, turned into script history lines.
static CAPTURED: Mutex<Vec<(log::Level, String)>> = Mutex::new(Vec::new());

struct Capture;

impl log::Log for Capture {
    fn enabled(&self, m: &log::Metadata) -> bool {
        m.level() <= log::Level::Warn
    }
    fn log(&self, r: &log::Record) {
        if self.enabled(r.metadata()) {
            if let Ok(mut c) = CAPTURED.lock() {
                c.push((r.level(), r.args().to_string()));
            }
            eprintln!("{}: {}", r.level(), r.args());
        }
    }
    fn flush(&self) {}
}

static LOGGER: Capture = Capture;

pub fn install_logger() {
    let _ = log::set_logger(&LOGGER);
    log::set_max_level(log::LevelFilter::Warn);
}

pub struct Core {
    pub s: Session,
    pub log: Vec<LogLine>,
    /// Lines the UI hasn't taken yet start here.
    pub log_read: usize,
    /// Bumped whenever anything may have changed; panels refresh when it moves.
    pub version: u64,
    /// The undo entry the last silent command made, which `retract_preview` may take back.
    preview: Vec<u64>,
    /// MEL's globals, procedures and top-level variables.
    pub mel_state: mel::MelState,
    /// An idle step is running: what it runs doesn't start another.
    in_idle: bool,
    /// The Show Manipulator tool's node and drag.
    pub node_manip: crate::node_manip::State,
    /// The up axis and units as last seen, to tell when a command changed them.
    settings: Settings,
    /// The optionVars as last saved.
    saved_vars: std::collections::BTreeMap<String, Value>,
}

/// The world settings commands change: the up axis and the current units.
#[derive(Clone, PartialEq, Default)]
struct Settings {
    up_z: bool,
    linear: &'static str,
    angle: &'static str,
    time: String,
}

impl Settings {
    fn now() -> Settings {
        Settings {
            up_z: aztek_scene::world::up_z(),
            linear: aztek_ops::units::linear_name(),
            angle: aztek_ops::units::angle_name(),
            time: aztek_scene::anim_time::time_unit_name(aztek_scene::anim::fps()),
        }
    }
}

impl Core {
    pub fn new() -> Core {
        let mut c = Core { s: Session::new(), log: Vec::new(), log_read: 0, version: 1, preview: Vec::new(), mel_state: Default::default(), in_idle: false, node_manip: Default::default(), settings: Settings::now(), saved_vars: Default::default() };
        register_ui_commands(&mut c.s);
        aztek_scene::scripted::set_post_create_hook(crate::native_plugin::run_post_create);
        crate::ui_cmds::register(&mut c.s);
        crate::scriptui::register(&mut c.s);
        crate::script_objects::register(&mut c.s);
        crate::script_jobs::register(&mut c.s);
        crate::ctx_cmds::register(&mut c.s);
        crate::panel_cmds::register(&mut c.s);
        crate::host_general::register(&mut c.s);
        aztek_usd::install(&mut c.s);
        c.mel_state.set_global_string("gMainWindow", crate::scriptui::MAIN_WINDOW);
        c.mel_state.set_global_string("gMainProgressBar", crate::scriptui::MAIN_PROGRESS);
        c.ensure_defaults();
        #[cfg(feature = "python")]
        crate::python::register_commands(&mut c.s);
        crate::native_plugin::register_commands(&mut c.s);
        crate::native_plugin::messages::install();
        crate::native_plugin::install_hooks();
        // The features kept as plug-ins: there from the start unless turned off.
        crate::builtin_plugins::load_defaults(&mut c.s);
        crate::user_folder::register_commands(&mut c.s);
        crate::environment::register_commands(&mut c.s);
        crate::environment::startup();
        aztek_scene::unit_tables::tag_commands(&mut c.s.commands);
        c
    }

    /// A core around a session moved here while Python is called back from inside a command: no UI commands of its own.
    pub(crate) fn nested(s: Session) -> Core {
        Core { s, log: Vec::new(), log_read: 0, version: 1, preview: Vec::new(), mel_state: Default::default(), in_idle: false, node_manip: Default::default(), settings: Settings::now(), saved_vars: Default::default() }
    }

    /// A new scene has the default cameras and sets, as the reference's does. Cheap when they're there.
    pub fn ensure_defaults(&mut self) {
        if !self.s.graph.undo_stack().can_undo() {
            aztek_scene::cameras::make_defaults(&mut self.s.graph);
        }
    }

    pub fn push(&mut self, text: impl Into<String>, kind: LineKind) {
        let text = text.into();
        if kind == LineKind::Error {
            let bare = text.trim_start_matches("// Error: ").trim_start_matches("# Error: ").trim_end_matches(" //").trim_end_matches(" #");
            crate::script_jobs::set_last_error(bare);
        }
        crate::batch::echo(&text, kind);
        crate::environment::line(&text);
        self.log.push(LogLine { text, kind });
    }

    /// Before a command: a scene about to be replaced runs its closing scripts.
    fn before_command(&mut self, name: &str, call: &Value) {
        if crate::script_jobs::replaces_scene(name, call) {
            crate::script_jobs::note_event("PreFileNewOrOpened");
            let scripts = crate::script_jobs::scene_scripts(&mut self.s, &[1, 2], false);
            self.run_callbacks(scripts);
        }
    }

    /// After a command: what it did that jobs can't see in the scene, and the scripts it asked to run when free.
    /// Tells what changed the up axis or the units: the preference is kept (`upAxisDirection`), jobs hear of a unit
    /// change, and the grid, sized in the linear unit, is sent again.
    pub(crate) fn note_settings(&mut self) {
        let now = Settings::now();
        if now == self.settings {
            return;
        }
        let was = std::mem::replace(&mut self.settings, now.clone());
        if now.up_z != was.up_z {
            self.s.option_vars.insert("upAxisDirection".into(), json!(aztek_scene::world::up_name()));
        }
        if now.linear != was.linear {
            crate::script_jobs::note_event("linearUnitChanged");
            crate::host_general::grid_units_changed();
        }
        if now.angle != was.angle {
            crate::script_jobs::note_event("angularUnitChanged");
        }
        if now.time != was.time {
            crate::script_jobs::note_event("timeUnitChanged");
        }
        self.version += 1;
    }

    /// Reads the saved preferences and starts a scene under them: the application's start-up. From here on the
    /// optionVars are saved as they change.
    pub fn use_prefs(&mut self) {
        crate::ui_cmds::set_keeps_prefs(true);
        // The user folder first: its folders go on the script, plug-in and module paths, and the optionVars its
        // preferences hold stand in for the ones Aztek hasn't saved itself.
        let folder = crate::user_folder::apply(&crate::user_folder::load_choice());
        for w in &folder.warnings {
            self.push(format!("// Warning: {w} //"), LineKind::Warning);
        }
        let mut saved = crate::ui_cmds::saved_option_vars();
        if let Some(prefs) = &folder.prefs {
            for (k, v) in crate::user_folder::saved_option_vars(prefs) {
                saved.entry(k).or_insert(v);
            }
        }
        crate::ui_cmds::apply_startup_prefs(&saved);
        for (k, v) in saved {
            self.s.option_vars.entry(k).or_insert(v);
        }
        self.saved_vars = self.s.option_vars.clone();
        let _ = self.exec_quiet("newScene", json!({}), CallSource::Ui);
        self.s.scene_path = None;
        self.settings = Settings::now();
        crate::host_general::grid_units_changed();
    }

    /// What runs once the interface is up: the plug-ins marked to load at start, then the user folder's userSetup
    /// scripts. Returns the Python ones, for the caller to run outside the core.
    pub fn startup_scripts(&mut self) -> Vec<std::path::PathBuf> {
        for e in crate::native_plugin::load_autoload(&mut self.s) {
            self.push(format!("// Warning: {e} //"), LineKind::Warning);
        }
        if !crate::user_folder::load_choice().user_setup {
            return Vec::new();
        }
        let mut dirs: Vec<std::path::PathBuf> = Vec::new();
        for var in ["AZTEK_SCRIPT_PATH", "MAYA_SCRIPT_PATH", "PYTHONPATH"] {
            if let Some(v) = std::env::var_os(var) {
                for d in std::env::split_paths(&v) {
                    if !d.as_os_str().is_empty() && !dirs.contains(&d) {
                        dirs.push(d);
                    }
                }
            }
        }
        // One userSetup.mel, the first on the path; every userSetup.py, as the reference runs them.
        if let Some(mel) = dirs.iter().map(|d| d.join("userSetup.mel")).find(|p| p.is_file()) {
            let quoted = mel.to_string_lossy().replace('\\', "/").replace('"', "\\\"");
            self.mel(&format!("source \"{quoted}\";"));
        }
        if aztek_core::env::on("MAYA_SKIP_USERSETUP_PY") {
            return Vec::new();
        }
        dirs.iter().map(|d| d.join("userSetup.py")).filter(|p| p.is_file()).collect()
    }

    /// Saves the optionVars when a command changed them.
    fn save_option_vars(&mut self) {
        if crate::ui_cmds::keeps_prefs() && self.s.option_vars != self.saved_vars {
            crate::ui_cmds::save_option_var_map(&self.s.option_vars);
            self.saved_vars = self.s.option_vars.clone();
        }
    }

    fn after_command(&mut self, name: &str, call: &Value, ok: bool) {
        aztek_scene::scripted::run_post_create(&mut self.s);
        self.note_settings();
        self.save_option_vars();
        crate::usd_view::note_events();
        for w in crate::script_jobs::take_warnings() {
            self.push(format!("// Warning: {w} //"), LineKind::Warning);
        }
        if ok && crate::script_jobs::note_command(name, call) {
            self.run_open_scripts();
        }
        for (cb, args) in crate::scriptui::take_immediate() {
            self.run_callback(&cb, &args);
        }
        if crate::script_jobs::take_flush() {
            self.idle();
        }
        self.run_conditions();
    }

    fn run_open_scripts(&mut self) {
        let scripts = crate::script_jobs::scene_scripts(&mut self.s, &[1, 2], true);
        self.run_callbacks(scripts);
        crate::script_jobs::watch_time_scripts(&mut self.s);
    }

    fn run_callbacks(&mut self, scripts: Vec<Value>) {
        for s in scripts {
            self.run_callback(&s, &[]);
        }
    }

    /// Runs a callback as one undo step: a string is MEL, otherwise Python. Errors go to the history.
    pub fn run_callback(&mut self, cb: &Value, args: &[Value]) {
        use aztek_ops::CommandHost;
        let saved = crate::scriptui::set_from_python(false);
        self.s.open_undo_chunk("script");
        let substituted = |src: &str| {
            let mut out = src.to_string();
            for (i, a) in args.iter().enumerate() {
                let t = match a {
                    Value::String(s) => s.clone(),
                    Value::Bool(b) => (*b as i32).to_string(),
                    o => o.to_string(),
                };
                out = out.replace(&format!("#{}", i + 1), &t).replace(&format!("%{}", i + 1), &t);
            }
            out
        };
        let r = match cb {
            Value::String(src) => mel::run_callback(self, &substituted(src)),
            Value::Object(o) => {
                if let Some(id) = o.get("__pycb__").and_then(Value::as_i64) {
                    python_callback(self, id, args)
                } else if let Some(src) = o.get("__pysrc__").and_then(Value::as_str) {
                    python_source(self, &substituted(src))
                } else {
                    Ok(())
                }
            }
            _ => Ok(()),
        };
        self.s.close_undo_chunk();
        crate::scriptui::set_from_python(saved);
        self.drain_captured();
        self.version += 1;
        if let Err(m) = r {
            self.push(format!("// Error: {m} //"), LineKind::Error);
        }
    }

    /// What the user did in a script window; see `scriptui::shell_event`.
    pub fn ui_event(&mut self, ev: &Value) {
        let due = crate::scriptui::shell_event(ev);
        crate::attr_ui::shell_changed(self, ev);
        for (cb, args) in due {
            self.run_callback(&cb, &args);
        }
    }

    /// One idle step: script jobs whose watch fired, then deferred scripts. The shell calls this on its poll; a headless
    /// caller calls it when it wants them run.
    pub fn idle(&mut self) {
        if self.in_idle || crate::scriptui::in_hook() {
            return;
        }
        self.in_idle = true;
        self.note_settings();
        crate::attr_ui::idle(self);
        crate::host_general::watch(&mut self.s);
        aztek_scene::messages::flush(&mut self.s);
        self.run_conditions();
        let scripts = crate::script_jobs::scan(&mut self.s);
        let deferred = crate::script_jobs::take_deferred();
        self.drain_captured();
        self.run_callbacks(scripts);
        self.run_callbacks(deferred);
        self.in_idle = false;
    }

    /// Runs the scripts of named conditions that are due, and keeps their answers.
    fn run_conditions(&mut self) {
        for (name, script) in crate::host_general::take_due() {
            match self.mel_checked(&script) {
                Ok(v) => crate::host_general::set_condition(&name, v.as_bool().unwrap_or(v.as_i64().is_some_and(|i| i != 0))),
                Err(e) => self.push(format!("// Warning: condition {name}: {e} //"), LineKind::Warning),
            }
        }
    }

    fn drain_captured(&mut self) {
        #[cfg(feature = "python")]
        for (t, k) in crate::python::take_deferred() {
            self.push(t, k);
        }
        for (t, k) in crate::native_plugin::take_deferred() {
            self.push(t, k);
        }
        let lines: Vec<(log::Level, String)> = CAPTURED.lock().map(|mut c| std::mem::take(&mut *c)).unwrap_or_default();
        for (lvl, t) in lines {
            let kind = if lvl == log::Level::Error { LineKind::Error } else { LineKind::Warning };
            let prefix = if kind == LineKind::Error { "Error" } else { "Warning" };
            self.push(format!("// {prefix}: {t} //"), kind);
        }
    }

    /// Runs a command, echoing it as MEL; the result or error goes to the history.
    pub fn exec(&mut self, name: &str, call: Value) -> Result<Value, String> {
        // Echoed as the reference does: short flag names.
        let mut shown = call.clone();
        if aztek_ops::units::in_stored() {
            if let Some(def) = self.s.commands.find(name) {
                aztek_ops::in_user_units(def, &mut shown);
            }
        }
        if let (Some(def), Some(o)) = (self.s.commands.find(name), shown.as_object_mut()) {
            let keys: Vec<String> = o.keys().cloned().collect();
            for k in keys {
                if let Some(f) = def.flags.iter().find(|f| f.name == k && !f.short_name.is_empty()) {
                    let v = o.remove(&k).unwrap();
                    o.insert(f.short_name.clone(), v);
                }
            }
        }
        self.push(mel::echo(name, &shown), LineKind::Echo);
        self.exec_quiet(name, call, CallSource::Ui)
    }

    pub fn exec_quiet(&mut self, name: &str, call: Value, src: CallSource) -> Result<Value, String> {
        let (name, call) = pivot_move(name, call);
        self.before_command(&name, &call);
        let r = execute(&mut self.s, &name, &call, src);
        self.ensure_defaults();
        self.version += 1;
        self.drain_captured();
        self.after_command(&name, &call, r.is_ok());
        match r {
            Ok(v) => {
                if let Some(t) = mel::result(&v) {
                    self.push(t, LineKind::Result);
                }
                Ok(v)
            }
            Err(e) => {
                let msg = e.to_string();
                self.push(format!("// Error: {msg} //"), LineKind::Error);
                Err(msg)
            }
        }
    }

    /// Undoes the last silent command, if it recorded anything and nothing came after it: a drag's preview step.
    pub fn retract_preview(&mut self) {
        if let Some(id) = self.preview.pop() {
            if self.s.graph.undo_stack().newest_id() == id && self.s.graph.undo_stack().can_undo() {
                self.s.undo();
                self.version += 1;
            } else {
                self.preview.clear();
            }
        }
    }

    /// Takes back the last command (a manipulator drag replacing its own preview), dropping its history lines.
    pub fn retract(&mut self, lines: usize) {
        self.s.undo();
        let keep = self.log.len().saturating_sub(lines).max(self.log_read.min(self.log.len()));
        self.log.truncate(keep);
        self.version += 1;
    }

    /// Runs MEL source: echoes each statement and prints its result. An error goes to the history, ends the run and
    /// gives null. Otherwise returns the value of the last statement.
    pub fn mel(&mut self, src: &str) -> Value {
        aztek_ops::units::as_script(|| mel::run(self, src)).unwrap_or(Value::Null)
    }

    /// As `mel`, with the error back to the caller as well.
    pub fn mel_checked(&mut self, src: &str) -> Result<Value, String> {
        aztek_ops::units::as_script(|| mel::run(self, src))
    }

    /// One command from MEL: no history lines; the interpreter reports the result or error as fits the statement.
    pub(crate) fn run_mel_command(&mut self, typed: &str, toks: &[mel::Tok]) -> Result<Value, String> {
        let (name, mut call) = mel::build(&self.s.commands, typed, toks)?;
        let qe = call.as_object_mut().and_then(|o| o.remove("_query").map(|a| (true, a)).or_else(|| o.remove("_edit").map(|a| (false, a))));
        if let Some((query, asked)) = qe {
            let o = call.as_object_mut().unwrap();
            let args: Vec<Value> = o.remove("args").and_then(|a| a.as_array().cloned()).unwrap_or_default();
            let mut flags = o.clone();
            for a in asked.as_array().into_iter().flatten() {
                if let Some(n) = a.as_str() {
                    flags.insert(n.to_string(), Value::Null);
                }
            }
            return self.query_edit(&name, args, flags, query);
        }
        let (name, call) = self.components_first(name, call);
        let asked = call.clone();
        let out = self.exec_plain(&name, call)?;
        // polyEvaluate with several flags: an array in MEL, in the order the flags were given.
        if let (true, Value::Object(o)) = (name == "polyEvaluate", &out) {
            if let Some(flags) = asked.as_object() {
                let several: Vec<Value> = flags.keys().filter_map(|k| o.get(k).cloned()).collect();
                if !several.is_empty() {
                    return Ok(Value::Array(several));
                }
            }
        }
        Ok(out)
    }

    fn exec_plain(&mut self, name: &str, call: Value) -> Result<Value, String> {
        let (name, call) = pivot_move(name, call);
        self.before_command(&name, &call);
        let r = execute(&mut self.s, &name, &call, CallSource::Script);
        self.version += 1;
        self.drain_captured();
        self.after_command(&name, &call, r.is_ok());
        r.map_err(|e| e.to_string())
    }

    /// The reference's component arguments (polyExtrudeFacet pCube1.f[2]): Aztek's component commands work on the selection,
    /// so select them first. move/rotate/scale/delete on components become their component commands.
    fn components_first(&mut self, mut name: String, mut call: Value) -> (String, Value) {
        let Some(args) = call.get("args").and_then(Value::as_array).cloned() else { return (name, call) };
        let (comps, rest): (Vec<Value>, Vec<Value>) = args.into_iter().partition(|a| a.as_str().is_some_and(mel::is_component));
        if comps.is_empty() {
            return (name, call);
        }
        let renamed = match name.as_str() {
            "move" => Some("moveComponents"),
            "rotate" => Some("rotateComponents"),
            "scale" => Some("scaleComponents"),
            "delete" => Some("deleteComponents"),
            _ => None,
        };
        if renamed.is_none() && !mel::takes_components(&name) {
            return (name, call);
        }
        if let Some(r) = renamed {
            name = r.to_string();
            if let Some(o) = call.as_object_mut() {
                // Components move relatively here.
                if let Some(to) = o.remove("to") {
                    o.insert("by".into(), to);
                }
                // The object form's switches mean nothing to the component commands.
                o.retain(|k, _| matches!(k.as_str(), "by" | "args" | "pivot"));
            }
        }
        let _ = self.exec_plain("select", json!({ "args": comps }));
        if let Some(o) = call.as_object_mut() {
            if rest.is_empty() {
                o.remove("args");
            } else {
                o.insert("args".into(), Value::Array(rest));
            }
        }
        (name, call)
    }

    /// A command run from Python: no history lines, errors back to the caller (who raises them).
    pub fn exec_silent(&mut self, name: &str, call: Value) -> Result<Value, String> {
        let before = self.s.graph.undo_stack().newest_id();
        let (name, call) = pivot_move(name, call);
        self.before_command(&name, &call);
        let r = execute(&mut self.s, &name, &call, CallSource::Script);
        self.ensure_defaults();
        self.after_command(&name, &call, r.is_ok());
        let after = self.s.graph.undo_stack().newest_id();
        if after != before {
            self.preview.push(after);
            if self.preview.len() > 64 {
                self.preview.remove(0);
            }
        }
        self.version += 1;
        self.drain_captured();
        r.map_err(|e| e.to_string())
    }

    /// aztek.cmds.<name>(*args, **kwargs): the reference's spellings, short flags, query and edit modes.
    pub fn py_call(&mut self, typed: &str, args: Vec<Value>, kwargs: Map<String, Value>) -> Result<Value, String> {
        // Callback strings given from Python are Python.
        let saved = crate::scriptui::set_from_python(true);
        let r = self.py_call_inner(typed, args, kwargs);
        crate::scriptui::set_from_python(saved);
        self.note_settings();
        r
    }

    fn py_call_inner(&mut self, typed: &str, args: Vec<Value>, mut kwargs: Map<String, Value>) -> Result<Value, String> {
        // optionVar(q='name'): the query flag carrying what is queried.
        let mut args = args;
        for k in ["q", "query"] {
            if let Some(Value::String(what)) = kwargs.get(k).cloned() {
                args.push(json!(what));
                kwargs.insert(k.to_string(), json!(true));
            }
        }
        if crate::host_general::echoes(typed) {
            let mut shown = kwargs.clone();
            shown.insert("args".to_string(), json!(args));
            self.push(mel::echo(typed, &Value::Object(shown)), LineKind::Echo);
        }
        let al = mel::alias(typed);
        let name = al.map(|a| a.target.to_string()).unwrap_or(typed.to_string());
        if self.s.commands.find(&name).is_some_and(|d| d.raw_args) {
            for (k, v) in kwargs {
                args.push(json!(format!("-{k}")));
                match v {
                    Value::Array(list) => args.extend(list),
                    v => args.push(v),
                }
            }
            return self.exec_silent(&name, json!({ "args": args }));
        }
        let def_flags: Vec<(String, String, aztek_ops::ArgType)> = match self.s.commands.find(&name) {
            Some(d) => d.flags.iter().map(|f| (f.name.clone(), f.short_name.clone(), f.ty)).collect(),
            None => return Err(format!("Cannot find procedure \"{typed}\".")),
        };
        let on = |v: &Value| v.as_bool().unwrap_or(v.as_i64().is_some_and(|i| i != 0));
        let own = |short: &str, long: &str| def_flags.iter().any(|(n, sh, _)| sh == short && n != long);
        let q_own = own("q", "query");
        let query = kwargs.get("query").is_some_and(on) || (!q_own && kwargs.get("q").is_some_and(on));
        let e_own = own("e", "edit");
        let edit = !e_own && kwargs.get("e").or(kwargs.get("edit")).is_some_and(on);
        // Commands with their own query mode take q=True and the flags asked about.
        let own_query = def_flags.iter().any(|(n, _, _)| n == "query");
        if own_query && (query || edit) {
            let mut flags = Map::new();
            let mut asked = Vec::new();
            flags.insert(if query { "query" } else { "edit" }.into(), json!(true));
            for (k, v) in &kwargs {
                if matches!(k.as_str(), "q" | "query" | "e" | "edit") {
                    continue;
                }
                let Some((long, _, ty)) = def_flags.iter().find(|(n, s, _)| n == k || (!s.is_empty() && s == k)) else {
                    return Err(format!("{typed}: invalid flag '{k}'"));
                };
                if query && v == &Value::Bool(true) && *ty != aztek_ops::ArgType::Bool {
                    asked.push(json!(long));
                } else if query && v == &Value::Bool(true) {
                    asked.push(json!(long));
                    flags.insert(long.clone(), json!(true));
                } else {
                    flags.insert(long.clone(), v.clone());
                }
            }
            if def_flags.iter().any(|(n, _, _)| n == "queried") {
                flags.insert("queried".into(), Value::Array(asked));
            } else {
                for a in asked {
                    if let Some(n) = a.as_str() {
                        flags.entry(n.to_string()).or_insert(json!(true));
                    }
                }
            }
            if !args.is_empty() {
                flags.insert("args".into(), Value::Array(args));
            }
            return self.exec_silent(&name, Value::Object(flags));
        }
        let mut flags = Map::new();
        let mut relative = false;
        for (k, v) in &kwargs {
            if matches!(k.as_str(), "q" | "query" | "e" | "edit") && !(e_own && k == "e") && !(q_own && k == "q") {
                continue;
            }
            let mut key = k.as_str();
            if let Some(a) = al {
                if let Some((_, ours)) = a.flags.iter().find(|(m, _)| *m == key) {
                    key = ours;
                }
            }
            match def_flags.iter().find(|(n, s, _)| n == key || (!s.is_empty() && s == key)) {
                Some((long, _, ty)) => {
                    let v = if *ty == aztek_ops::ArgType::Bool && !v.is_boolean() { json!(on(v)) } else { v.clone() };
                    flags.insert(long.clone(), v);
                }
                None => {
                    if matches!(key, "r" | "relative") && matches!(name.as_str(), "move" | "rotate" | "scale") {
                        relative = on(v);
                    } else if query || edit {
                        // Attribute flags for query/edit on a node.
                        flags.insert(key.to_string(), v.clone());
                    } else if !matches!(key, "r" | "replace" | "ws" | "worldSpace" | "os" | "objectSpace" | "a" | "absolute" | "ch" | "constructionHistory" | "kft" | "keepFacesTogether" | "divisions") {
                        return Err(format!("{typed}: invalid flag '{k}'"));
                    }
                }
            }
        }
        let mut args = args;
        if query || edit {
            return self.query_edit(&name, args, flags, query);
        }
        if matches!(name.as_str(), "move" | "rotate" | "scale") && args.len() >= 3 && args[..3].iter().all(|a| a.is_number()) {
            let v: Vec<Value> = args.drain(..3).collect();
            flags.insert(if relative { "by" } else { "to" }.into(), Value::Array(v));
        }
        if !args.is_empty() {
            flags.insert("args".into(), Value::Array(args));
        }
        let (name, call) = self.components_first(name, Value::Object(flags));
        self.exec_silent(&name, call)
    }

    /// The reference's -q and -e on a node's settings: each flag is an attribute of the node named first (or selected), or of
    /// its shape's history (polyCube1 for pCube1).
    fn query_edit(&mut self, name: &str, args: Vec<Value>, flags: Map<String, Value>, query: bool) -> Result<Value, String> {
        // hotkey asks about a key, not an object.
        if name == "hotkey" && query {
            let mut call = flags;
            let asked: Vec<Value> = call.iter().filter(|(_, v)| v.is_null()).map(|(k, _)| json!(k)).collect();
            call.retain(|_, v| !v.is_null());
            call.insert("query".into(), json!(true));
            call.insert("queried".into(), Value::Array(asked));
            if let Some(key) = args.first() {
                call.entry("keyShortcut").or_insert(key.clone());
            }
            return self.exec_silent(name, Value::Object(call));
        }
        let g = &self.s.graph;
        let target = match args.first().and_then(Value::as_str) {
            Some(t) => t.to_string(),
            None => match g.selection().last() {
                Some(&id) => g.path(id),
                None => return Err(format!("{name}: no object given or selected")),
            },
        };
        let mut nodes = vec![target.clone()];
        if let Ok(Some(id)) = g.find(&target) {
            if let Some(shape) = aztek_scene::components::as_mesh(g, id) {
                for h in aztek_scene::history::history_of(g, shape) {
                    nodes.push(g.path(h));
                }
            }
        }
        let mut out = Value::Null;
        for (attr, v) in flags {
            let plug = nodes.iter().map(|n| format!("{n}.{attr}")).find(|p| execute(&mut self.s, "getAttr", &json!({ "args": [p] }), CallSource::Script).is_ok());
            let Some(plug) = plug else { return Err(format!("{name}: {target} has no {attr}")) };
            if query {
                out = self.exec_silent("getAttr", json!({ "args": [plug] }))?;
            } else {
                self.exec_silent("setAttr", json!({ "args": [plug, v] }))?;
            }
        }
        Ok(out)
    }

    pub fn take_log(&mut self) -> Value {
        self.drain_captured();
        let start = self.log_read.min(self.log.len());
        let lines: Vec<Value> = self.log[start..].iter().map(|l| json!({"text": l.text, "kind": l.kind as i32})).collect();
        self.log_read = self.log.len();
        if self.log.len() > 20000 {
            self.log.drain(..10000);
            self.log_read = self.log.len();
        }
        Value::Array(lines)
    }

    /// Saves in the format the path's extension names: `.ma` or Aztek's own.
    pub fn save(&mut self, path: &str) -> Result<(), String> {
        crate::native_plugin::messages::before_ui(&mut self.s, "save", path)?;
        aztek_scene::ma_file::save_scene(&mut self.s, path, aztek_scene::ma_file::Format::of_path(path))?;
        crate::native_plugin::messages::after_ui(&mut self.s, "save", path);
        self.drain_captured();
        self.version += 1;
        crate::script_jobs::note_event("SceneSaved");
        self.push(format!("// Saved {path} //"), LineKind::Info);
        Ok(())
    }

    pub fn open(&mut self, path: &str) -> Result<(), String> {
        crate::native_plugin::messages::before_ui(&mut self.s, "open", path)?;
        self.before_command("newScene", &json!({}));
        let warnings = aztek_scene::ma_file::open_scene(&mut self.s, path)?;
        crate::native_plugin::messages::after_ui(&mut self.s, "open", path);
        // Files older than the default cameras, and .ma files, which leave them out, get them now.
        aztek_scene::cameras::make_defaults(&mut self.s.graph);
        // Warnings from a .ma file arrive through the log.
        self.drain_captured();
        if aztek_scene::ma_file::Format::of_path(path) != aztek_scene::ma_file::Format::Ma {
            for w in warnings {
                self.push(format!("// Warning: {w} //"), LineKind::Warning);
            }
        }
        self.version += 1;
        self.push(format!("// Opened {path} //"), LineKind::Info);
        crate::script_jobs::note_command("file", &json!({"open": true}));
        self.run_open_scripts();
        Ok(())
    }

    pub fn new_scene(&mut self) {
        let _ = self.exec("newScene", json!({}));
        self.s.scene_path = None;
    }
}

/// MEL's python("..."): runs Python and gives back the value of a lone expression.
#[cfg(feature = "python")]
pub(crate) fn python_from_mel(core: &mut Core, src: &str) -> Result<Value, String> {
    crate::python::eval_from_mel(core, src)
}

#[cfg(not(feature = "python"))]
pub(crate) fn python_from_mel(_core: &mut Core, _src: &str) -> Result<Value, String> {
    Err("python: this build has no Python.".into())
}

#[cfg(feature = "python")]
fn python_callback(core: &mut Core, id: i64, args: &[Value]) -> Result<(), String> {
    crate::python::fire_callback(core, id, args)
}

#[cfg(not(feature = "python"))]
fn python_callback(_core: &mut Core, _id: i64, _args: &[Value]) -> Result<(), String> {
    Err("python: this build has no Python.".into())
}

#[cfg(feature = "python")]
pub(crate) fn python_value(core: &mut Core, id: i64, args: &[Value]) -> Result<Value, String> {
    crate::python::fire_value(core, id, args)
}

#[cfg(not(feature = "python"))]
pub(crate) fn python_value(_core: &mut Core, _id: i64, _args: &[Value]) -> Result<Value, String> {
    Err("python: this build has no Python.".into())
}

#[cfg(feature = "python")]
fn python_source(core: &mut Core, src: &str) -> Result<(), String> {
    crate::python::fire_source(core, src)
}

#[cfg(not(feature = "python"))]
fn python_source(_core: &mut Core, _src: &str) -> Result<(), String> {
    Err("python: this build has no Python.".into())
}

// Windows and tools scripts open by the reference's runtime command names; the UI takes them from `take_ui_requests`.
thread_local! {
    // Per thread, as the host is: one UI thread in the app, and tests in parallel don't take each other's.
    static UI_REQUESTS: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

pub const UI_COMMANDS: &[(&str, &str)] = &[
    ("GraphEditor", "Open the Graph Editor."),
    ("UsdLayerEditor", "Open the USD Layer Editor."),
    ("UsdStats", "Open the USD Stats window."),
    ("AztekFeedback", "Report that something doesn't feel right."),
    ("NodeEditorWindow", "Open the Node Editor."),
    ("DopeSheetEditor", "Open the Dope Sheet."),
    ("SetDrivenKeyOptions", "Open the Set Driven Key window."),
    ("ScriptEditor", "Open the Script Editor."),
    ("OutlinerWindow", "Show the Outliner."),
    ("AttributeEditor", "Show the Attribute Editor."),
    ("TextureViewWindow", "Open the UV Editor."),
    ("UVSetEditor", "Open the UV Set Editor."),
    ("UVEditor", "Open the UV Editor."),
    ("PreferencesWindow", "Open the Preferences."),
    ("ToggleChannelsLayers", "Show or hide the Channel Box / Layer Editor."),
    ("FrameSelected", "Frame the selection in the active view."),
    ("FrameAll", "Frame everything in the active view."),
    ("PlaybackToggle", "Start or stop playback."),
    ("JointTool", "The joint tool: click to place joints."),
    ("InsertJointTool", "The insert joint tool: drag from a joint to add one on the bone."),
    ("SculptMeshActivateBrushSculpt", "The sculpt brush."),
    ("MultiCutTool", "The Multi-Cut tool."),
    ("SplitEdgeRingTool", "The Insert Edge Loop tool."),
    ("OffsetEdgeLoopTool", "The Offset Edge Loop tool."),
    ("TargetWeldTool", "The Target Weld tool."),
    ("SlideEdgeTool", "The Slide Edge tool."),
    ("PolyCreaseTool", "The Crease tool."),
    ("CreatePolygonTool", "The Create Polygon tool."),
    ("AppendToPolygonTool", "The Append to Polygon tool."),
    ("QuadDrawTool", "The Quad Draw tool."),
    ("CVCurveTool", "The CV curve tool: click to place CVs, Enter to finish."),
    ("EPCurveTool", "The EP curve tool: click to place edit points, Enter to finish."),
    ("ArtPaintSkinWeightsTool", "The paint skin weights tool."),
    ("ArtPaintSkinWeightsToolOptions", "The paint skin weights tool and its settings."),
    ("ArtPaintAttrTool", "The paint attribute tool, on deformers' weights."),
    ("ArtPaintAttrToolOptions", "The paint attribute tool and its settings."),
    ("EditMembershipTool", "The edit membership tool: which points a deformer holds."),
    ("ShapeEditor", "The Shape Editor: blend shapes and their targets."),
    ("HumanIK", "The Human IK window: characters, sources, definitions and controls."),
    ("HIKCharacterControlsTool", "The Human IK window."),
    ("ToolSettingsWindow", "Show the Tool Settings."),
    ("SelectTool", "The select tool."),
    ("MoveTool", "The move tool."),
    ("RotateTool", "The rotate tool."),
    ("ScaleTool", "The scale tool."),
    ("FourViewLayout", "Four views: top, persp, front, side."),
    ("SingleViewLayout", "One view: the perspective."),
];

fn register_ui_commands(s: &mut Session) {
    s.register(
        aztek_ops::CommandDef::<Session>::new(
            "artSetToolAndSelectAttr",
            "Picks the paint attribute tool and what it paints: artSetToolAndSelectAttr artAttrCtx cluster.cluster1.weights.",
            |s, a| {
                let names = a.named();
                let Some(attr) = names.last().filter(|_| names.len() >= 2) else { aztek_ops::fail!("artSetToolAndSelectAttr: give the tool and nodeType.node.attribute") };
                aztek_scene::deform_paint::select_attr(s, attr)?;
                UI_REQUESTS.with(|q| q.borrow_mut().push("ArtPaintAttrTool".to_string()));
                Ok(Value::Null)
            },
        )
        .positional("the tool, then nodeType.node.attribute")
        .undoable(false),
    );
    for (name, doc) in UI_COMMANDS {
        let n = name.to_string();
        s.register(
            aztek_ops::CommandDef::<Session>::new(name, doc, move |_s, _a| {
                push_ui_request(n.clone());
                Ok(Value::Null)
            })
            .undoable(false),
        );
    }
}

/// Asks the shell to do something, by runtime command name or "name|payload".
pub fn push_ui_request(r: String) {
    UI_REQUESTS.with(|q| q.borrow_mut().push(r));
}

pub fn take_ui_requests() -> Value {
    json!(UI_REQUESTS.with(|q| std::mem::take(&mut *q.borrow_mut())))
}

/// The reference moves pivots with move on obj.scalePivot / obj.rotatePivot: that is movePivot here.
fn pivot_move(name: &str, call: Value) -> (String, Value) {
    if name != "move" {
        return (name.to_string(), call);
    }
    let Some(args) = call.get("args").and_then(Value::as_array) else { return (name.to_string(), call) };
    if !args.iter().any(|a| a.as_str().is_some_and(|s| s.ends_with(".rotatePivot") || s.ends_with(".scalePivot") || s.ends_with(".rp") || s.ends_with(".sp"))) {
        return (name.to_string(), call);
    }
    let mut objs: Vec<Value> = Vec::new();
    let (mut rotate, mut scale) = (false, false);
    for a in args.iter().filter_map(Value::as_str) {
        let (o, plug) = a.split_once('.').unwrap_or((a, ""));
        rotate |= matches!(plug, "rotatePivot" | "rp");
        scale |= matches!(plug, "scalePivot" | "sp");
        let o = json!(o);
        if !objs.contains(&o) {
            objs.push(o);
        }
    }
    let values: Vec<f64> = args.iter().filter_map(Value::as_f64).collect();
    let mut c = call.as_object().cloned().unwrap_or_default();
    let flag = |c: &Map<String, Value>, names: &[&str]| names.iter().any(|n| c.get(*n).and_then(Value::as_bool) == Some(true));
    let relative = flag(&c, &["relative", "r"]);
    let object_space = flag(&c, &["objectSpace", "os"]);
    for f in ["absolute", "a", "relative", "r", "worldSpace", "ws", "worldSpaceDistance", "wd", "objectSpace", "os", "rotatePivotRelative", "rpr", "scalePivotRelative", "spr", "xyz"] {
        c.remove(f);
    }
    if values.len() >= 3 {
        c.insert(if relative { "by" } else { "to" }.into(), json!(values[..3]));
    }
    c.insert("args".into(), Value::Array(objs));
    if rotate {
        c.insert("rotatePivot".into(), json!(true));
    }
    if scale {
        c.insert("scalePivot".into(), json!(true));
    }
    if object_space {
        c.insert("objectSpace".into(), json!(true));
    }
    ("movePivot".into(), Value::Object(c))
}
