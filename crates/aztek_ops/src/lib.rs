//! The command registry and `execute`.
//!
//! The registry lives in the
//! context commands run against (`C`, the session), which hands it out through [`CommandHost`]; `execute` takes that
//! context, so commands can run other commands.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::{json, Map, Value};

/// A command failing with a message for the user.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CommandError {
    #[error("{0}")]
    Failed(String),
    /// A command the user aborted from the progress card ("importFbx: cancelled").
    #[error("{0}")]
    Cancelled(String),
}

impl CommandError {
    pub fn new(msg: impl Into<String>) -> Self {
        CommandError::Failed(msg.into())
    }
}

pub type CmdResult<T = Value> = Result<T, CommandError>;

/// Shorthand for `Err(CommandError::new(format!(..)))`.
#[macro_export]
macro_rules! fail {
    ($($t:tt)*) => { return Err($crate::CommandError::new(format!($($t)*))) };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgType {
    Bool,
    Int,
    Float,
    String,
    Vec3,
    List,
    Any,
}

/// One flag. `doc` is read by the API reference, the Python stubs and runtime help(), so write it for someone typing
/// the call.
#[derive(Debug, Clone)]
pub struct FlagDef {
    pub name: String,
    pub short_name: String,
    pub ty: ArgType,
    /// Null = not passed unless given.
    pub default_value: Value,
    pub doc: String,
    /// Allowed values of a String flag; empty means free text.
    pub choices: Vec<String>,
    /// A short Python call showing the flag in use.
    pub example: String,
    /// Bounds of an Int or Float flag: calls are clamped to them.
    pub min: Option<f64>,
    pub max: Option<f64>,
    /// MEL: tokens each use of the flag takes, for multi-use flags (`-tv joint1 0.5 -tv joint2 0.5`); values gather
    /// into one flat list. 0 reads the flag by its type.
    pub arity: u8,
}

impl FlagDef {
    pub fn new(name: &str, short: &str, ty: ArgType, default_value: Value, doc: &str) -> Self {
        Self {
            name: name.into(),
            short_name: short.into(),
            ty,
            default_value,
            doc: doc.into(),
            choices: Vec::new(),
            example: String::new(),
            min: None,
            max: None,
            arity: 0,
        }
    }
    /// A multi-use flag taking n values per use.
    pub fn multi(mut self, n: u8) -> Self {
        self.arity = n;
        self
    }
    pub fn choices(mut self, c: &[&str]) -> Self {
        self.choices = c.iter().map(|s| s.to_string()).collect();
        self
    }
    pub fn example(mut self, e: &str) -> Self {
        self.example = e.into();
        self
    }
    pub fn min(mut self, v: f64) -> Self {
        self.min = Some(v);
        self
    }
    pub fn max(mut self, v: f64) -> Self {
        self.max = Some(v);
        self
    }
}

/// Normalized call: positional args plus long-name flags, defaults filled in.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Args {
    pub positional: Vec<Value>,
    pub flags: Map<String, Value>,
}

impl Args {
    pub fn has(&self, flag: &str) -> bool {
        self.flags.get(flag).is_some_and(|v| !v.is_null())
    }
    fn raw(&self, flag: &str) -> Option<&Value> {
        self.flags.get(flag).filter(|v| !v.is_null())
    }
    pub fn get(&self, flag: &str) -> Option<&Value> {
        self.raw(flag)
    }
    pub fn bool(&self, flag: &str) -> bool {
        self.raw(flag)
            .and_then(|v| v.as_bool().or_else(|| v.as_i64().map(|i| i != 0)))
            .unwrap_or(false)
    }
    pub fn int(&self, flag: &str, fallback: i64) -> i64 {
        self.raw(flag)
            .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)))
            .unwrap_or(fallback)
    }
    pub fn float(&self, flag: &str, fallback: f64) -> f64 {
        self.raw(flag).and_then(Value::as_f64).unwrap_or(fallback)
    }
    pub fn string(&self, flag: &str, fallback: &str) -> String {
        self.raw(flag)
            .and_then(Value::as_str)
            .unwrap_or(fallback)
            .to_string()
    }
    /// A three-number flag.
    pub fn vec3(&self, flag: &str) -> Option<[f64; 3]> {
        let a = self.raw(flag)?.as_array()?;
        if a.len() != 3 {
            return None;
        }
        Some([a[0].as_f64()?, a[1].as_f64()?, a[2].as_f64()?])
    }
    /// A positional string.
    pub fn pos_str(&self, i: usize) -> Option<&str> {
        self.positional.get(i).and_then(Value::as_str)
    }

    /// Positional args flattened to strings (lists included). The selection is the host's to supply when empty.
    pub fn named(&self) -> Vec<String> {
        let mut out = Vec::new();
        for v in &self.positional {
            if let Some(s) = v.as_str() {
                out.push(s.to_string());
            } else if let Some(a) = v.as_array() {
                out.extend(a.iter().filter_map(|x| x.as_str().map(str::to_string)));
            }
        }
        out
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallSource {
    /// Python, MCP.
    Script,
    /// Menus, hotkeys, tools: echoed to the script editor.
    Ui,
}

pub type RunFn<C> = Arc<dyn Fn(&mut C, &Args) -> CmdResult + Send + Sync>;

/// A registered command. `doc` is the summary line, then optionally a blank line and a longer description.
pub struct CommandDef<C> {
    pub name: String,
    pub doc: String,
    /// Empty = takes none.
    pub positional_doc: String,
    pub flags: Vec<FlagDef>,
    pub undoable: bool,
    pub run: RunFn<C>,
    /// Extension that registered it; empty = built in.
    pub owner: String,
    /// Re-running it with different flags after undoing it is safe, so the UI may offer to adjust the last call live.
    pub adjustable: bool,
    /// Python snippets, each one or more lines of runnable source.
    pub examples: Vec<String>,
    /// Registered from Python at runtime, whatever its owner.
    pub scripted: bool,
}

impl<C> Clone for CommandDef<C> {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            doc: self.doc.clone(),
            positional_doc: self.positional_doc.clone(),
            flags: self.flags.clone(),
            undoable: self.undoable,
            run: self.run.clone(),
            owner: self.owner.clone(),
            adjustable: self.adjustable,
            examples: self.examples.clone(),
            scripted: self.scripted,
        }
    }
}

impl<C> CommandDef<C> {
    pub fn new(
        name: &str,
        doc: &str,
        run: impl Fn(&mut C, &Args) -> CmdResult + Send + Sync + 'static,
    ) -> Self {
        Self {
            name: name.into(),
            doc: doc.into(),
            positional_doc: String::new(),
            flags: Vec::new(),
            undoable: true,
            run: Arc::new(run),
            owner: String::new(),
            adjustable: false,
            examples: Vec::new(),
            scripted: false,
        }
    }
    pub fn positional(mut self, doc: &str) -> Self {
        self.positional_doc = doc.into();
        self
    }
    pub fn flag(mut self, f: FlagDef) -> Self {
        self.flags.push(f);
        self
    }
    pub fn undoable(mut self, on: bool) -> Self {
        self.undoable = on;
        self
    }
    pub fn adjustable(mut self, on: bool) -> Self {
        self.adjustable = on;
        self
    }
    pub fn examples(mut self, e: &[&str]) -> Self {
        self.examples = e.iter().map(|s| s.to_string()).collect();
        self
    }
}

/// One top-level execute, reported before the command runs and again after it finished or failed.
#[derive(Debug, Clone)]
pub struct ExecuteEvent<'a> {
    pub name: &'a str,
    pub call: &'a Value,
    pub source: CallSource,
    /// False: about to run.
    pub done: bool,
    /// With done: it returned without failing.
    pub ok: bool,
}

pub type ExecuteListener = Arc<dyn Fn(&ExecuteEvent) + Send + Sync>;
pub type ExecuteGate = Arc<dyn Fn(&str) -> CmdResult<()> + Send + Sync>;

/// The registry and execution state.
pub struct Commands<C> {
    registry: BTreeMap<String, CommandDef<C>>,
    listeners: BTreeMap<i32, ExecuteListener>,
    next_listener: i32,
    gate: Option<ExecuteGate>,
    depth: i32,
    running: Vec<Args>,
    previewing: bool,
    agent_calling: bool,
}

impl<C> Default for Commands<C> {
    fn default() -> Self {
        Self {
            registry: BTreeMap::new(),
            listeners: BTreeMap::new(),
            next_listener: 1,
            gate: None,
            depth: 0,
            running: Vec::new(),
            previewing: false,
            agent_calling: false,
        }
    }
}

impl<C> Commands<C> {
    /// How many commands are running, one inside another.
    pub fn depth(&self) -> i32 {
        self.depth
    }
    /// Registering a name again replaces it.
    pub fn register(&mut self, def: CommandDef<C>) {
        self.registry.insert(def.name.clone(), def);
    }
    pub fn unregister(&mut self, name: &str) {
        self.registry.remove(name);
    }
    /// Removes every command `owner` registered. Returns how many.
    pub fn remove_owned(&mut self, owner: &str) -> usize {
        let before = self.registry.len();
        self.registry.retain(|_, d| d.owner != owner);
        before - self.registry.len()
    }
    pub fn find(&self, name: &str) -> Option<&CommandDef<C>> {
        self.registry.get(name)
    }
    pub fn all(&self) -> impl Iterator<Item = &CommandDef<C>> {
        self.registry.values()
    }
    /// Marks an already registered command adjustable. False if it isn't registered.
    pub fn set_adjustable(&mut self, name: &str, on: bool) -> bool {
        match self.registry.get_mut(name) {
            Some(d) => {
                d.adjustable = on;
                true
            }
            None => false,
        }
    }
    /// The arguments of the innermost command running now, or None outside any.
    pub fn current_args(&self) -> Option<&Args> {
        self.running.last()
    }
    /// Called around top-level executes only; commands run by other commands aren't reported.
    pub fn add_execute_listener(&mut self, f: ExecuteListener) -> i32 {
        let id = self.next_listener;
        self.next_listener += 1;
        self.listeners.insert(id, f);
        id
    }
    pub fn remove_execute_listener(&mut self, id: i32) {
        self.listeners.remove(&id);
    }
    /// Asked before every command runs, nested ones too but not previews; an error refuses it. None removes it.
    pub fn set_execute_gate(&mut self, gate: Option<ExecuteGate>) {
        self.gate = gate;
    }
    /// Whether an agent's MCP request is being handled; commands run meanwhile are the agent's.
    pub fn agent_calling(&self) -> bool {
        self.agent_calling
    }
    pub fn set_agent_calling(&mut self, on: bool) {
        self.agent_calling = on;
    }
    /// True while commands run only to show what they would do and are reverted straight after.
    pub fn previewing(&self) -> bool {
        self.previewing
    }
    pub fn set_previewing(&mut self, on: bool) {
        self.previewing = on;
    }
}

/// What `execute` needs from the context commands run against.
pub trait CommandHost: Sized + 'static {
    fn commands(&self) -> &Commands<Self>;
    fn commands_mut(&mut self) -> &mut Commands<Self>;
    /// The selected objects' paths, for `objects()` with no positional args.
    fn selection_paths(&self) -> Vec<String>;
    fn open_undo_chunk(&mut self, label: &str);
    fn close_undo_chunk(&mut self);
    /// Told the innermost running command's args whenever they change (None outside any command).
    fn running_args_changed(&mut self, _args: Option<&Args>) {}
    /// A top-level command (not a preview) is about to run.
    fn top_level_started(&mut self, _name: &str, _call: &Value, _source: CallSource) {}
    /// A top-level command finished; `ok` is false when it failed.
    fn top_level_finished(&mut self, _name: &str, _call: &Value, _source: CallSource, _ok: bool) {}
}

/// Positional args, or the selection when none were given.
pub fn objects<C: CommandHost>(ctx: &C, a: &Args) -> Vec<String> {
    let out = a.named();
    if out.is_empty() {
        ctx.selection_paths()
    } else {
        out
    }
}

fn matches(ty: ArgType, v: &Value) -> bool {
    match ty {
        ArgType::Bool => v.is_boolean() || v.is_i64() || v.is_u64(),
        ArgType::Int => v.is_i64() || v.is_u64() || v.is_boolean(),
        ArgType::Float => v.is_number(),
        ArgType::String => v.is_string(),
        ArgType::Vec3 => v
            .as_array()
            .is_some_and(|a| a.len() == 3 && a.iter().all(Value::is_number)),
        ArgType::List => v.is_array(),
        ArgType::Any => true,
    }
}

fn coerce(flag: &FlagDef, v: &Value) -> Value {
    if flag.ty == ArgType::Bool {
        if let Some(i) = v.as_i64() {
            return Value::Bool(i != 0);
        }
    }
    if flag.ty == ArgType::Int {
        if let Some(b) = v.as_bool() {
            return json!(if b { 1 } else { 0 });
        }
        if let Some(i) = v.as_i64() {
            if flag.min.is_some() || flag.max.is_some() {
                let lo = flag.min.map(|m| m.ceil() as i64).unwrap_or(i64::MIN);
                let hi = flag.max.map(|m| m.floor() as i64).unwrap_or(i64::MAX);
                return json!(i.clamp(lo, hi));
            }
        }
    }
    if flag.ty == ArgType::Float && (flag.min.is_some() || flag.max.is_some()) {
        if let Some(d) = v.as_f64() {
            return json!(d.clamp(
                flag.min.unwrap_or(f64::NEG_INFINITY),
                flag.max.unwrap_or(f64::INFINITY)
            ));
        }
    }
    v.clone()
}

fn normalize<C>(def: &CommandDef<C>, call: &Value) -> CmdResult<Args> {
    let mut a = Args::default();
    let Some(obj) = call.as_object() else {
        fail!("{}: call must be an object", def.name)
    };
    for (key, v) in obj {
        if key == "args" {
            a.positional = match v.as_array() {
                Some(arr) => arr.clone(),
                None => vec![v.clone()],
            };
            continue;
        }
        let Some(flag) = def
            .flags
            .iter()
            .rev()
            .find(|f| f.name == *key || (!f.short_name.is_empty() && f.short_name == *key))
        else {
            fail!("{}: unknown flag '{key}'", def.name)
        };
        if !v.is_null() && !matches(flag.ty, v) {
            fail!("{}: bad value for '{}': {v}", def.name, flag.name);
        }
        if let Some(s) = v.as_str() {
            if !flag.choices.is_empty() && !flag.choices.iter().any(|c| c == s) {
                fail!(
                    "{}: bad value for '{}': {v} (one of {})",
                    def.name,
                    flag.name,
                    flag.choices.join(", ")
                );
            }
        }
        a.flags.insert(flag.name.clone(), coerce(flag, v));
    }
    if def.positional_doc.is_empty() && !a.positional.is_empty() {
        fail!("{} takes no positional arguments", def.name);
    }
    for f in &def.flags {
        if !a.flags.contains_key(&f.name) && !f.default_value.is_null() {
            a.flags.insert(f.name.clone(), f.default_value.clone());
        }
    }
    Ok(a)
}

/// `call` is {"args": [...], flag: value, ...}, flags by long or short name. Main thread only.
pub fn execute<C: CommandHost>(
    ctx: &mut C,
    name: &str,
    call: &Value,
    source: CallSource,
) -> CmdResult {
    let Some(def) = ctx.commands().find(name).cloned() else {
        fail!("unknown command '{name}'")
    };
    let empty = json!({});
    let args = normalize(&def, if call.is_null() { &empty } else { call })?;

    let c = ctx.commands();
    if !c.previewing {
        if let Some(gate) = c.gate.clone() {
            gate(&def.name)?;
        }
    }
    // A preview run is reverted at once, so nothing watching executes should see it.
    let top = c.depth == 0 && !c.previewing;
    let listeners: Vec<ExecuteListener> = if top {
        c.listeners.values().cloned().collect()
    } else {
        Vec::new()
    };
    let event = |done, ok| ExecuteEvent {
        name,
        call,
        source,
        done,
        ok,
    };
    for l in &listeners {
        l(&event(false, false));
    }
    if top {
        ctx.top_level_started(name, call, source);
    }

    let c = ctx.commands_mut();
    c.depth += 1;
    c.running.push(args.clone());
    ctx.running_args_changed(Some(&args));
    let result = if def.undoable {
        ctx.open_undo_chunk(&def.name);
        let r = (def.run)(ctx, &args);
        ctx.close_undo_chunk();
        r
    } else {
        (def.run)(ctx, &args)
    };
    let c = ctx.commands_mut();
    c.running.pop();
    c.depth -= 1;
    let current = ctx.commands().running.last().cloned();
    ctx.running_args_changed(current.as_ref());

    if top {
        ctx.top_level_finished(name, call, source, result.is_ok());
    }
    for l in &listeners {
        l(&event(true, result.is_ok()));
    }
    if result.is_ok() && source == CallSource::Ui {
        log::info!(target: "echo", "{}", to_python(name, call));
    }
    result
}

/// `execute` from a script.
pub fn run<C: CommandHost>(ctx: &mut C, name: &str, call: Value) -> CmdResult {
    execute(ctx, name, &call, CallSource::Script)
}

fn py_literal(v: &Value) -> String {
    match v {
        Value::Bool(b) => if *b { "True" } else { "False" }.into(),
        Value::Null => "None".into(),
        Value::Array(a) => format!(
            "[{}]",
            a.iter().map(py_literal).collect::<Vec<_>>().join(", ")
        ),
        Value::Object(o) => {
            format!(
                "{{{}}}",
                o.iter()
                    .map(|(k, x)| format!("{}: {}", json!(k), py_literal(x)))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
        other => other.to_string(),
    }
}

/// Python source equivalent of a call, for echo.
pub fn to_python(name: &str, call: &Value) -> String {
    let mut parts = Vec::new();
    if let Some(o) = call.as_object() {
        if let Some(pos) = o.get("args") {
            match pos.as_array() {
                Some(a) => parts.extend(a.iter().map(py_literal)),
                None => parts.push(py_literal(pos)),
            }
        }
        for (k, v) in o {
            if k != "args" {
                parts.push(format!("{k}={}", py_literal(v)));
            }
        }
    }
    format!("{name}({})", parts.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Ctx {
        commands: Commands<Ctx>,
        chunks: Vec<String>,
        log: Vec<String>,
    }

    impl CommandHost for Ctx {
        fn commands(&self) -> &Commands<Self> {
            &self.commands
        }
        fn commands_mut(&mut self) -> &mut Commands<Self> {
            &mut self.commands
        }
        fn selection_paths(&self) -> Vec<String> {
            vec!["sel".into()]
        }
        fn open_undo_chunk(&mut self, label: &str) {
            self.chunks.push(format!("open {label}"));
        }
        fn close_undo_chunk(&mut self) {
            self.chunks.push("close".into());
        }
    }

    fn ctx() -> Ctx {
        let mut c = Ctx::default();
        c.commands.register(
            CommandDef::new("echo", "Returns its flags.", |_, a| {
                Ok(Value::Object(a.flags.clone()))
            })
            .flag(
                FlagDef::new("count", "c", ArgType::Int, json!(1), "Loops.")
                    .min(1.0)
                    .max(32.0),
            )
            .flag(
                FlagDef::new("amount", "a", ArgType::Float, json!(0.5), "Fraction.")
                    .min(0.0)
                    .max(1.0),
            )
            .flag(FlagDef::new("mode", "m", ArgType::String, Value::Null, "").choices(&["a", "b"]))
            .undoable(false),
        );
        c.commands.register(
            CommandDef::new("outer", "Runs echo.", |c: &mut Ctx, a| {
                c.log.push(format!("objects {:?}", objects(c, a)));
                run(c, "echo", json!({"c": 5}))
            })
            .positional("objects"),
        );
        c
    }

    #[test]
    fn flags_are_clamped_to_their_bounds() {
        let mut c = ctx();
        let r = run(&mut c, "echo", json!({"count": -3, "amount": 4.0})).unwrap();
        assert_eq!(r["count"], 1);
        assert_eq!(r["amount"], 1.0);
        let r = run(&mut c, "echo", json!({"count": 40, "amount": 0.25})).unwrap();
        assert_eq!(r["count"], 32);
        assert_eq!(r["amount"], 0.25);
    }

    #[test]
    fn bad_calls_fail() {
        let mut c = ctx();
        assert!(run(&mut c, "nope", json!({})).is_err());
        assert!(run(&mut c, "echo", json!({"bogus": 1})).is_err());
        assert!(run(&mut c, "echo", json!({"count": "x"})).is_err());
        assert!(run(&mut c, "echo", json!({"mode": "z"})).is_err());
        assert!(run(&mut c, "echo", json!({"args": ["x"]})).is_err());
    }

    #[test]
    fn nested_commands_and_undo_chunks() {
        let mut c = ctx();
        let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let s = seen.clone();
        c.commands.add_execute_listener(Arc::new(move |e| {
            s.lock().unwrap().push((e.name.to_string(), e.done))
        }));
        let r = run(&mut c, "outer", json!({})).unwrap();
        assert_eq!(r["count"], 5);
        assert_eq!(c.log, ["objects [\"sel\"]"]);
        assert_eq!(c.chunks, ["open outer", "close"]);
        // Only the top-level call is reported.
        assert_eq!(
            *seen.lock().unwrap(),
            [("outer".to_string(), false), ("outer".to_string(), true)]
        );
    }

    #[test]
    fn python_echo() {
        assert_eq!(
            to_python(
                "xform",
                &json!({"args": ["cube1"], "q": true, "t": [1, 2, 3]})
            ),
            "xform(\"cube1\", q=True, t=[1, 2, 3])"
        );
    }
}
