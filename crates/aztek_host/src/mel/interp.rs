//! Runs parsed MEL against a Core. Everything that outlives a run (globals, procedures, top-level variables) is in
//! `MelState`, so a nested run (eval, Python's mel.eval) sees it.

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::Value as J;

use super::call::{self, flag_spelling, word_value, Tok};
use super::parser::{parse_program, BinOp, CmdArg, Decl, Expr, Proc, Stmt, Top};
use super::value::{Ty, Value};
use crate::core::{Core, LineKind};

/// A control transfer leaving the statement being run.
pub enum Flow {
    Error(String),
    Break,
    Continue,
    Return(Value),
}

impl From<String> for Flow {
    fn from(e: String) -> Flow {
        Flow::Error(e)
    }
}

pub type R<T> = Result<T, Flow>;

pub fn err<T>(m: impl Into<String>) -> R<T> {
    Err(Flow::Error(m.into()))
}

/// Procedure calls deeper than this are an error rather than an exhausted machine.
const MAX_CALL_DEPTH: usize = 4000;
/// Arrays grow on assignment; an index past this is a script's mistake, not a wish for gigabytes.
const MAX_ARRAY: usize = 1 << 26;
const RED_ZONE: usize = 256 * 1024;
const STACK_CHUNK: usize = 4 * 1024 * 1024;

enum Slot {
    Local(Value),
    /// A `global` declaration: the name means the global of that name here.
    Global,
}

struct Frame {
    scopes: Vec<HashMap<String, Slot>>,
    /// Whose local procedures are visible: 0 is what was typed or run directly, each sourced file has its own.
    unit: u32,
}

struct ProcEntry {
    proc: Arc<Proc>,
    unit: u32,
    file: Option<String>,
}

pub struct MelState {
    globals: HashMap<String, Value>,
    global_procs: HashMap<String, ProcEntry>,
    local_procs: HashMap<(u32, String), ProcEntry>,
    /// frames[0] is the top level, whose variables stay from one run to the next, as in the Script Editor.
    frames: Vec<Frame>,
    cur: usize,
    next_unit: u32,
    calls: usize,
    /// Runs under way: deferred scripts wait for the outermost to end.
    running: usize,
    deferred: Vec<String>,
    /// print's output not yet ended by a newline.
    print_buf: String,
    /// The JSON the last command returned, kept whole for the host's return value and result line.
    last_raw: Option<J>,
    pub(super) rng: u64,
}

impl MelState {
    /// A global variable the application provides, such as $gMainWindow.
    pub fn set_global_string(&mut self, name: &str, value: &str) {
        self.globals.insert(name.to_string(), Value::Str(value.to_string()));
    }
}

impl Default for MelState {
    fn default() -> Self {
        MelState {
            globals: HashMap::new(),
            global_procs: HashMap::new(),
            local_procs: HashMap::new(),
            frames: vec![Frame { scopes: vec![HashMap::new()], unit: 0 }],
            cur: 0,
            next_unit: 1,
            calls: 0,
            running: 0,
            deferred: Vec::new(),
            print_buf: String::new(),
            last_raw: None,
            rng: 0x2545_F491_4F6C_DD1D,
        }
    }
}

/// Runs a source at the top level: echoes each statement and prints its result. An error goes to the history and ends
/// the run; a syntax error means nothing of the source runs.
pub fn run(core: &mut Core, src: &str) -> Result<J, String> {
    let r = run_echoed(core, src);
    if core.mel_state.running == 0 {
        for d in std::mem::take(&mut core.mel_state.deferred) {
            let _ = run_echoed(core, &d);
        }
    }
    core.version += 1;
    r
}

/// A callback's run: no echo and no result lines, its print output kept. The error comes back to the caller.
/// Whether a global procedure of this name has been defined.
pub fn has_global_proc(core: &Core, name: &str) -> bool {
    core.mel_state.global_procs.contains_key(name)
}

/// As `run_callback`, with the script's value: nothing echoed, and what it prints is printed.
pub fn call_for_value(core: &mut Core, src: &str) -> Result<J, String> {
    core.mel_state.running += 1;
    let mut it = Interp { core };
    let r = it.run_quiet(src, None);
    it.flush_print();
    core.mel_state.running -= 1;
    match r {
        Ok(v) | Err(Flow::Return(v)) => Ok(v.to_json()),
        Err(Flow::Break | Flow::Continue) => Ok(J::Null),
        Err(Flow::Error(m)) => Err(m),
    }
}

pub fn run_callback(core: &mut Core, src: &str) -> Result<(), String> {
    core.mel_state.running += 1;
    let mut it = Interp { core };
    let r = it.run_quiet(src, None);
    it.flush_print();
    core.mel_state.running -= 1;
    if core.mel_state.running == 0 {
        for d in std::mem::take(&mut core.mel_state.deferred) {
            let _ = run_echoed(core, &d);
        }
    }
    match r {
        Ok(_) | Err(Flow::Return(_) | Flow::Break | Flow::Continue) => Ok(()),
        Err(Flow::Error(m)) => Err(m),
    }
}

fn run_echoed(core: &mut Core, src: &str) -> Result<J, String> {
    let prog = match parse_program(src) {
        Ok(p) => p,
        Err((line, e)) => {
            core.push(src.trim(), LineKind::Echo);
            let m = format!("line {line}: {e}");
            core.push(format!("// Error: {m} //"), LineKind::Error);
            return Err(m);
        }
    };
    let saved_cur = core.mel_state.cur;
    core.mel_state.cur = 0;
    core.mel_state.running += 1;
    let mut it = Interp { core };
    it.hoist(&prog, None);
    let mut last = J::Null;
    let mut failure = None;
    for top in &prog {
        it.core.push(top.text.clone(), LineKind::Echo);
        if matches!(top.stmt, Stmt::Proc(_)) {
            continue;
        }
        it.core.mel_state.last_raw = None;
        let r = it.exec(&top.stmt);
        it.flush_print();
        match r {
            Ok(v) => {
                let direct = matches!(top.stmt, Stmt::Expr(Expr::Command(..) | Expr::Call(..)));
                let raw = it.core.mel_state.last_raw.take().filter(|_| direct);
                let j = raw.unwrap_or_else(|| v.to_json());
                if let Some(t) = call::result(&j) {
                    it.core.push(t, LineKind::Result);
                }
                last = j;
            }
            Err(Flow::Error(m)) => {
                it.core.push(format!("// Error: {m} //"), LineKind::Error);
                failure = Some(m);
                break;
            }
            // return at the top level ends the run; a stray break or continue ends it too.
            Err(Flow::Return(v)) => {
                last = v.to_json();
                break;
            }
            Err(Flow::Break | Flow::Continue) => break,
        }
    }
    let st = &mut core.mel_state;
    st.cur = saved_cur;
    st.running -= 1;
    match failure {
        Some(m) => Err(m),
        None => Ok(last),
    }
}

/// Which operand of a builtin a call writes back to the caller's variable. `tokenize`'s array is its last.
fn writes_back(name: &str, count: usize) -> Option<usize> {
    match name {
        "clear" | "appendStringArray" => Some(0),
        "tokenize" if count >= 2 => Some(count - 1),
        "stringArrayInsertAtIndex" | "intArrayInsertAtIndex" | "floatArrayInsertAtIndex" => Some(1),
        _ => None,
    }
}

pub struct Interp<'a> {
    pub core: &'a mut Core,
}

impl Interp<'_> {
    fn st(&mut self) -> &mut MelState {
        &mut self.core.mel_state
    }

    fn frame(&mut self) -> &mut Frame {
        let st = &mut self.core.mel_state;
        &mut st.frames[st.cur]
    }

    /// Whole lines go to the history; the rest waits for its newline or the statement's end.
    pub fn write_print(&mut self, text: &str) {
        self.st().print_buf.push_str(text);
        while let Some(nl) = self.st().print_buf.find('\n') {
            let line: String = self.st().print_buf.drain(..=nl).collect();
            self.core.push(line.trim_end_matches('\n'), LineKind::Info);
        }
    }

    pub fn flush_print(&mut self) {
        if !self.st().print_buf.is_empty() {
            let t = std::mem::take(&mut self.st().print_buf);
            self.core.push(t, LineKind::Info);
        }
    }

    fn hoist(&mut self, prog: &[Top], file: Option<&str>) {
        let unit = self.frame().unit;
        for t in prog {
            if let Stmt::Proc(p) = &t.stmt {
                self.define(p, unit, file);
            }
        }
    }

    fn define(&mut self, p: &Arc<Proc>, unit: u32, file: Option<&str>) {
        let e = ProcEntry { proc: p.clone(), unit, file: file.map(str::to_string) };
        let st = self.st();
        if p.global {
            st.global_procs.insert(p.name.clone(), e);
        } else {
            st.local_procs.insert((unit, p.name.clone()), e);
        }
    }

    fn find_proc(&mut self, name: &str) -> Option<(Arc<Proc>, u32)> {
        let unit = self.frame().unit;
        let st = self.st();
        st.local_procs.get(&(unit, name.to_string())).or_else(|| st.global_procs.get(name)).map(|e| (e.proc.clone(), e.unit))
    }

    /// A procedure's argument types, as getProcArguments gives them.
    pub fn proc_params(&mut self, name: &str) -> Option<Vec<String>> {
        let (p, _) = self.find_proc(name)?;
        Some(p.params.iter().map(|(ty, _)| ty.name().to_string()).collect())
    }

    pub fn is_proc(&mut self, name: &str) -> bool {
        self.find_proc(name).is_some()
    }

    pub fn proc_file(&mut self, name: &str) -> Option<Option<String>> {
        let unit = self.frame().unit;
        let st = self.st();
        st.local_procs.get(&(unit, name.to_string())).or_else(|| st.global_procs.get(name)).map(|e| e.file.clone())
    }

    pub fn is_command(&self, name: &str) -> bool {
        // These answer as MEL expects (about -linux is 1, env lists globals); the registered ones are for Python.
        if matches!(name, "about" | "env" | "getenv" | "putenv") {
            return false;
        }
        call::alias(name).is_some() || self.core.s.commands.find(name).is_some()
    }

    pub fn global_names(&mut self) -> Vec<String> {
        let mut v: Vec<String> = self.st().globals.keys().map(|k| format!("${k}")).collect();
        v.sort();
        v
    }

    // Variables.

    fn lookup(&mut self, name: &str) -> Option<&Value> {
        let st = &self.core.mel_state;
        for s in st.frames[st.cur].scopes.iter().rev() {
            match s.get(name) {
                Some(Slot::Local(v)) => return Some(v),
                Some(Slot::Global) => return st.globals.get(name),
                None => {}
            }
        }
        None
    }

    fn lookup_mut(&mut self, name: &str) -> Option<&mut Value> {
        let st = &mut self.core.mel_state;
        let MelState { frames, cur, globals, .. } = st;
        for s in frames[*cur].scopes.iter_mut().rev() {
            match s.get_mut(name) {
                Some(Slot::Local(v)) => return Some(v),
                Some(Slot::Global) => return globals.get_mut(name),
                None => {}
            }
        }
        None
    }

    pub fn var_type(&mut self, name: &str) -> Option<Ty> {
        self.lookup(name).and_then(Value::ty)
    }

    fn declare_local(&mut self, name: &str, v: Value) {
        if let Some(s) = self.frame().scopes.last_mut() {
            s.insert(name.to_string(), Slot::Local(v));
        }
    }

    /// Assigning keeps the variable's type; an undeclared one takes the value's.
    fn set_var(&mut self, name: &str, v: Value) -> R<Value> {
        if let Some(slot) = self.lookup_mut(name) {
            let converted = match (&*slot, &v) {
                (Value::Matrix(r, c, _), Value::Matrix(r2, c2, _)) if (r, c) != (r2, c2) => {
                    return err(format!("Cannot assign a {r2}x{c2} matrix to a {r}x{c} matrix."));
                }
                (Value::Matrix(..), Value::Matrix(..)) => v,
                _ => v.coerce(slot.ty().unwrap_or(Ty::Str))?,
            };
            *slot = converted.clone();
            return Ok(converted);
        }
        let v = if matches!(v, Value::Void) { Value::Str(String::new()) } else { v };
        self.declare_local(name, v.clone());
        Ok(v)
    }

    fn declare(&mut self, global: bool, d: &Decl) -> R<Value> {
        let ty = if d.array && d.base != Ty::Matrix { d.base.array_of() } else { d.base };
        let mut dims = Vec::new();
        for e in &d.dims {
            let n = self.eval(e)?.to_int();
            if n < 0 || n as usize > MAX_ARRAY {
                return err("Array size out of range.");
            }
            dims.push(n as usize);
        }
        let initial = match ty {
            Ty::Matrix => {
                let (r, c) = (dims.first().copied().unwrap_or(0), dims.get(1).copied().unwrap_or(0));
                if r.saturating_mul(c) > MAX_ARRAY {
                    return err("Array size out of range.");
                }
                Value::Matrix(r, c, vec![0.0; r * c])
            }
            Ty::IntArr => Value::IntArr(vec![0; dims.first().copied().unwrap_or(0)]),
            Ty::FloatArr => Value::FloatArr(vec![0.0; dims.first().copied().unwrap_or(0)]),
            Ty::StrArr => Value::StrArr(vec![String::new(); dims.first().copied().unwrap_or(0)]),
            Ty::VecArr => Value::VecArr(vec![[0.0; 3]; dims.first().copied().unwrap_or(0)]),
            t => t.default_value(),
        };
        let redeclared = |have: &Value| -> R<()> {
            if have.ty() != Some(ty) {
                return err(format!("Invalid redeclaration of variable \"${}\" as a different type.", d.name));
            }
            Ok(())
        };
        if global {
            // A global is initialised once; declaring it again only makes the name visible here.
            let exists = match self.st().globals.get(&d.name) {
                Some(have) => {
                    redeclared(have)?;
                    true
                }
                None => false,
            };
            if !exists {
                let v = match &d.init {
                    Some(e) => self.init_value(e, ty, initial)?,
                    None => initial,
                };
                self.st().globals.insert(d.name.clone(), v);
            }
            if let Some(s) = self.frame().scopes.last_mut() {
                s.insert(d.name.clone(), Slot::Global);
            }
            return Ok(match &d.init {
                Some(_) => self.st().globals.get(&d.name).cloned().unwrap_or(Value::Void),
                None => Value::Void,
            });
        }
        // In the same scope a redeclaration keeps the variable; without an initialiser its value stays too.
        let same_scope = match self.frame().scopes.last() {
            Some(s) => match s.get(&d.name) {
                Some(Slot::Local(v)) => Some(v.clone()),
                Some(Slot::Global) => self.st().globals.get(&d.name).cloned(),
                None => None,
            },
            None => None,
        };
        if let Some(have) = same_scope {
            redeclared(&have)?;
            return match &d.init {
                Some(e) => {
                    let v = self.init_value(e, ty, initial)?;
                    self.set_var(&d.name, v)
                }
                None => Ok(Value::Void),
            };
        }
        match &d.init {
            Some(e) => {
                let v = self.init_value(e, ty, initial)?;
                self.declare_local(&d.name, v.clone());
                Ok(v)
            }
            None => {
                self.declare_local(&d.name, initial);
                Ok(Value::Void)
            }
        }
    }

    fn init_value(&mut self, e: &Expr, ty: Ty, initial: Value) -> R<Value> {
        let v = self.eval(e)?;
        if let (Value::Matrix(r, c, _), Value::Matrix(r2, c2, _)) = (&initial, &v) {
            if (*r, *c) != (0, 0) && (r, c) != (r2, c2) {
                return err(format!("Cannot assign a {r2}x{c2} matrix to a {r}x{c} matrix."));
            }
            return Ok(v);
        }
        Ok(v.coerce(ty)?)
    }

    /// Assigns to a variable, array element or matrix cell; returns what was stored.
    fn assign(&mut self, target: &Expr, v: Value) -> R<Value> {
        match target {
            Expr::Var(n) => self.set_var(n, v),
            Expr::Index(base, idx) => {
                // $m[r][c]
                if let Expr::Index(mbase, ridx) = &**base {
                    let Expr::Var(n) = &**mbase else { return err("Cannot assign to this expression.") };
                    let r = self.eval(ridx)?.to_int();
                    let c = self.eval(idx)?.to_int();
                    let f = v.to_float();
                    let Some(Value::Matrix(rows, cols, d)) = self.lookup_mut(n) else {
                        return err(format!("\"${n}\" is not a matrix."));
                    };
                    if r < 0 || c < 0 || r as usize >= *rows || c as usize >= *cols {
                        return err("Array index out of bounds.");
                    }
                    let at = r as usize * *cols + c as usize;
                    d[at] = f;
                    return Ok(Value::Float(f));
                }
                let Expr::Var(n) = &**base else { return err("Cannot assign to this expression.") };
                let i = self.eval(idx)?.to_int();
                if i < 0 {
                    return err("Array index out of bounds.");
                }
                let i = i as usize;
                if i >= MAX_ARRAY {
                    return err("Array index out of bounds.");
                }
                // $a[3] = "x" on an undeclared variable declares a string array.
                if self.lookup(n).is_none() {
                    let ty = v.ty().map(Ty::array_of).unwrap_or(Ty::StrArr);
                    self.declare_local(n, ty.default_value());
                }
                let Some(slot) = self.lookup_mut(n) else { return err(format!("\"${n}\" is an undeclared variable.")) };
                fn put<T: Default + Clone>(a: &mut Vec<T>, i: usize, x: T) {
                    if a.len() <= i {
                        a.resize(i + 1, T::default());
                    }
                    a[i] = x;
                }
                match slot {
                    Value::IntArr(a) => {
                        let x = v.coerce(Ty::Int)?.to_int();
                        put(a, i, x);
                        Ok(Value::Int(x))
                    }
                    Value::FloatArr(a) => {
                        let x = v.coerce(Ty::Float)?.to_float();
                        put(a, i, x);
                        Ok(Value::Float(x))
                    }
                    Value::StrArr(a) => {
                        let x = v.coerce(Ty::Str)?.to_text();
                        put(a, i, x.clone());
                        Ok(Value::Str(x))
                    }
                    Value::VecArr(a) => {
                        let x = v.coerce(Ty::Vector)?.to_vector();
                        put(a, i, x);
                        Ok(Value::Vector(x))
                    }
                    _ => err(format!("\"${n}\" is not an array.")),
                }
            }
            _ => err("Cannot assign to this expression."),
        }
    }

    // Statements.

    pub fn exec(&mut self, s: &Stmt) -> R<Value> {
        stacker::maybe_grow(RED_ZONE, STACK_CHUNK, || self.exec_inner(s))
    }

    fn exec_block(&mut self, body: &[Stmt]) -> R<Value> {
        self.frame().scopes.push(HashMap::new());
        let mut r = Ok(Value::Void);
        for s in body {
            r = self.exec(s);
            if r.is_err() {
                break;
            }
        }
        self.frame().scopes.pop();
        r
    }

    /// The body of a loop: Ok(false) when it says break.
    fn loop_body(&mut self, body: &Stmt) -> R<bool> {
        match self.exec(body) {
            Ok(_) | Err(Flow::Continue) => Ok(true),
            Err(Flow::Break) => Ok(false),
            Err(f) => Err(f),
        }
    }

    fn exec_inner(&mut self, s: &Stmt) -> R<Value> {
        match s {
            Stmt::Empty | Stmt::Proc(_) => Ok(Value::Void),
            Stmt::Expr(e) => self.eval(e),
            Stmt::Decl(global, ds) => {
                let mut last = Value::Void;
                for d in ds {
                    last = self.declare(*global, d)?;
                }
                Ok(last)
            }
            Stmt::Block(b) => {
                self.exec_block(b)?;
                Ok(Value::Void)
            }
            Stmt::If(c, a, b) => {
                if self.eval(c)?.truth() {
                    self.exec(a)?;
                } else if let Some(b) = b {
                    self.exec(b)?;
                }
                Ok(Value::Void)
            }
            Stmt::While(c, body) => {
                while self.eval(c)?.truth() {
                    if !self.loop_body(body)? {
                        break;
                    }
                }
                Ok(Value::Void)
            }
            Stmt::DoWhile(body, c) => {
                loop {
                    if !self.loop_body(body)? || !self.eval(c)?.truth() {
                        break;
                    }
                }
                Ok(Value::Void)
            }
            Stmt::For(init, c, step, body) => {
                for i in init {
                    self.exec(i)?;
                }
                loop {
                    if let Some(c) = c {
                        if !self.eval(c)?.truth() {
                            break;
                        }
                    }
                    if !self.loop_body(body)? {
                        break;
                    }
                    for e in step {
                        self.eval(e)?;
                    }
                }
                Ok(Value::Void)
            }
            Stmt::ForIn(ty, var, it, body) => {
                let items = self.eval(it)?;
                if !items.is_array() {
                    return err(format!("Cannot iterate over data of type {}.", items.type_name()));
                }
                if let Some(t) = ty {
                    let have = self.frame().scopes.last().is_some_and(|s| s.contains_key(var));
                    if !have {
                        self.declare_local(var, t.default_value());
                    }
                }
                for x in items.elements() {
                    self.set_var(var, x)?;
                    if !self.loop_body(body)? {
                        break;
                    }
                }
                Ok(Value::Void)
            }
            Stmt::Switch(subject, cases) => {
                let v = self.eval(subject)?;
                let mut start = None;
                for (i, (label, _)) in cases.iter().enumerate() {
                    if let Some(l) = label {
                        let lv = self.eval(l)?;
                        if self.binary(BinOp::Eq, &v, &lv)?.truth() {
                            start = Some(i);
                            break;
                        }
                    }
                }
                let start = start.or_else(|| cases.iter().position(|(l, _)| l.is_none()));
                if let Some(start) = start {
                    self.frame().scopes.push(HashMap::new());
                    let mut r = Ok(Value::Void);
                    'cases: for (_, body) in &cases[start..] {
                        for s in body {
                            if let Err(f) = self.exec(s) {
                                r = match f {
                                    Flow::Break => Ok(Value::Void),
                                    f => Err(f),
                                };
                                break 'cases;
                            }
                        }
                    }
                    self.frame().scopes.pop();
                    r?;
                }
                Ok(Value::Void)
            }
            Stmt::Break => Err(Flow::Break),
            Stmt::Continue => Err(Flow::Continue),
            Stmt::Return(e) => {
                let v = match e {
                    Some(e) => self.eval(e)?,
                    None => Value::Void,
                };
                Err(Flow::Return(v))
            }
        }
    }

    /// eval's and source's run: no echo; the value of the last statement.
    pub fn run_quiet(&mut self, src: &str, file: Option<&str>) -> R<Value> {
        let prog = match parse_program(src) {
            Ok(p) => p,
            Err((line, e)) => return err(format!("line {line}: {e}")),
        };
        // Top-level variables are what eval sees, wherever it is called from.
        let saved = self.st().cur;
        let r = if file.is_some() {
            let unit = self.st().next_unit;
            self.st().next_unit += 1;
            self.st().frames.push(Frame { scopes: vec![HashMap::new()], unit });
            let cur = self.st().frames.len() - 1;
            self.st().cur = cur;
            let r = self.run_program(&prog, file);
            self.st().frames.truncate(cur);
            r
        } else {
            self.st().cur = 0;
            self.run_program(&prog, None)
        };
        self.st().cur = saved;
        r
    }

    fn run_program(&mut self, prog: &[Top], file: Option<&str>) -> R<Value> {
        self.hoist(prog, file);
        let mut last = Value::Void;
        for t in prog {
            if !matches!(t.stmt, Stmt::Proc(_)) {
                last = self.exec(&t.stmt)?;
            }
        }
        Ok(last)
    }

    // Expressions.

    pub fn eval(&mut self, e: &Expr) -> R<Value> {
        stacker::maybe_grow(RED_ZONE, STACK_CHUNK, || self.eval_inner(e))
    }

    fn eval_inner(&mut self, e: &Expr) -> R<Value> {
        match e {
            Expr::Int(i) => Ok(Value::Int(*i)),
            Expr::Float(f) => Ok(Value::Float(*f)),
            Expr::Str(s) => Ok(Value::Str(s.clone())),
            Expr::Var(n) => match self.lookup(n) {
                Some(v) => Ok(v.clone()),
                None => err(format!("\"${n}\" is an undeclared variable.")),
            },
            Expr::Vector(items) => {
                let mut o = [0.0; 3];
                for (i, x) in items.iter().enumerate().take(3) {
                    o[i] = self.eval(x)?.to_float();
                }
                Ok(Value::Vector(o))
            }
            Expr::Matrix(rows) => {
                let (r, c) = (rows.len(), rows.first().map(Vec::len).unwrap_or(0));
                let mut d = Vec::with_capacity(r * c);
                for row in rows {
                    for x in row {
                        d.push(self.eval(x)?.to_float());
                    }
                }
                Ok(Value::Matrix(r, c, d))
            }
            Expr::Array(items) => {
                let mut vals = Vec::with_capacity(items.len());
                for x in items {
                    vals.push(self.eval(x)?);
                }
                array_of(vals)
            }
            Expr::Index(base, idx) => self.index(base, idx),
            Expr::Member(base, axis) => match self.eval(base)? {
                Value::Vector(v) => Ok(Value::Float(v[*axis])),
                v => err(format!("Cannot use .x, .y or .z on data of type {}.", v.type_name())),
            },
            Expr::Neg(x) => match self.eval(x)? {
                Value::Int(i) => Ok(Value::Int(i.wrapping_neg())),
                Value::Float(f) => Ok(Value::Float(-f)),
                Value::Vector(v) => Ok(Value::Vector([-v[0], -v[1], -v[2]])),
                Value::Matrix(r, c, d) => Ok(Value::Matrix(r, c, d.iter().map(|x| -x).collect())),
                v => err(format!("Cannot negate data of type {}.", v.type_name())),
            },
            Expr::Not(x) => Ok(Value::Int(!self.eval(x)?.truth() as i32)),
            Expr::Binary(op, a, b) => {
                let x = self.eval(a)?;
                let y = self.eval(b)?;
                self.binary(*op, &x, &y)
            }
            Expr::And(a, b) => Ok(Value::Int((self.eval(a)?.truth() && self.eval(b)?.truth()) as i32)),
            Expr::Or(a, b) => Ok(Value::Int((self.eval(a)?.truth() || self.eval(b)?.truth()) as i32)),
            Expr::Ternary(c, a, b) => {
                if self.eval(c)?.truth() {
                    self.eval(a)
                } else {
                    self.eval(b)
                }
            }
            Expr::Assign(target, op, val) => {
                let v = match op {
                    None => self.eval(val)?,
                    Some(op) => {
                        let cur = self.eval(target)?;
                        let r = self.eval(val)?;
                        self.binary(*op, &cur, &r)?
                    }
                };
                self.assign(target, v)
            }
            Expr::Step(target, d, prefix) => {
                let cur = self.eval(target)?;
                let next = match &cur {
                    Value::Int(i) => Value::Int(i.wrapping_add(*d)),
                    Value::Float(f) => Value::Float(f + *d as f64),
                    v => return err(format!("Cannot increment data of type {}.", v.type_name())),
                };
                let stored = self.assign(target, next)?;
                Ok(if *prefix { stored } else { cur })
            }
            Expr::Cast(t, x) => Ok(self.eval(x)?.coerce(*t)?),
            Expr::Call(name, args) => self.call_exprs(name, args),
            Expr::Command(name, args) => self.call_command(name, args),
        }
    }

    fn index(&mut self, base: &Expr, idx: &Expr) -> R<Value> {
        let i = self.eval(idx)?.to_int();
        // A variable's array is read in place: a loop over it shouldn't copy it each time.
        if let Expr::Var(n) = base {
            let Some(v) = self.lookup(n) else { return err(format!("\"${n}\" is an undeclared variable.")) };
            return element(v, i);
        }
        let v = self.eval(base)?;
        element(&v, i)
    }

    pub fn binary(&mut self, op: BinOp, a: &Value, b: &Value) -> R<Value> {
        use BinOp::*;
        if a.is_array() || b.is_array() {
            let t = if a.is_array() { a } else { b };
            return err(format!("Cannot use data of type {} in a scalar operation.", t.type_name()));
        }
        if matches!(a, Value::Void) || matches!(b, Value::Void) {
            return err("Cannot use a value of type void in an expression.");
        }
        let cmp = matches!(op, Eq | Ne | Lt | Le | Gt | Ge);
        let compare = |o: std::cmp::Ordering| -> Value {
            use std::cmp::Ordering::*;
            let t = match op {
                Eq => o == Equal,
                Ne => o != Equal,
                Lt => o == Less,
                Le => o != Greater,
                Gt => o == Greater,
                Ge => o != Less,
                _ => false,
            };
            Value::Int(t as i32)
        };
        // Strings: + joins, comparisons compare text.
        if matches!(a, Value::Str(_)) || matches!(b, Value::Str(_)) {
            if op == Add {
                let mut s = a.to_text();
                s.push_str(&b.to_text());
                return Ok(Value::Str(s));
            }
            if cmp {
                return Ok(compare(a.to_text().cmp(&b.to_text())));
            }
            return err("Illegal operation on data of type string.");
        }
        if let (Value::Matrix(..), _) | (_, Value::Matrix(..)) = (a, b) {
            return matrix_op(op, a, b);
        }
        if matches!(a, Value::Vector(_)) || matches!(b, Value::Vector(_)) || op == Cross {
            let (x, y) = (a.to_vector(), b.to_vector());
            let both = matches!(a, Value::Vector(_)) && matches!(b, Value::Vector(_));
            let each = |f: &dyn Fn(f64, f64) -> f64| Value::Vector([f(x[0], y[0]), f(x[1], y[1]), f(x[2], y[2])]);
            return Ok(match op {
                Add => each(&|p, q| p + q),
                Sub => each(&|p, q| p - q),
                // Two vectors: the dot product.
                Mul if both => Value::Float(x[0] * y[0] + x[1] * y[1] + x[2] * y[2]),
                Mul => each(&|p, q| p * q),
                Div => {
                    if y.contains(&0.0) {
                        return err("Attempt to divide by zero.");
                    }
                    each(&|p, q| p / q)
                }
                Mod => return err("Illegal operation on data of type vector."),
                Cross => Value::Vector([x[1] * y[2] - x[2] * y[1], x[2] * y[0] - x[0] * y[2], x[0] * y[1] - x[1] * y[0]]),
                Eq | Ne => {
                    let same = x == y;
                    Value::Int((same == (op == Eq)) as i32)
                }
                _ => {
                    let m = |v: [f64; 3]| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
                    compare(m(x).partial_cmp(&m(y)).unwrap_or(std::cmp::Ordering::Equal))
                }
            });
        }
        if let (Value::Int(x), Value::Int(y)) = (a, b) {
            let (x, y) = (*x, *y);
            return Ok(match op {
                Add => Value::Int(x.wrapping_add(y)),
                Sub => Value::Int(x.wrapping_sub(y)),
                Mul => Value::Int(x.wrapping_mul(y)),
                Div | Mod if y == 0 => return err("Attempt to divide by zero."),
                Div => Value::Int(x.wrapping_div(y)),
                Mod => Value::Int(x.wrapping_rem(y)),
                _ => compare(x.cmp(&y)),
            });
        }
        let (x, y) = (a.to_float(), b.to_float());
        Ok(match op {
            Add => Value::Float(x + y),
            Sub => Value::Float(x - y),
            Mul => Value::Float(x * y),
            Div | Mod if y == 0.0 => return err("Attempt to divide by zero."),
            Div => Value::Float(x / y),
            Mod => Value::Float(x % y),
            _ => match x.partial_cmp(&y) {
                Some(o) => compare(o),
                None => Value::Int((op == Ne) as i32),
            },
        })
    }

    // Calls.

    fn call_exprs(&mut self, name: &str, args: &[Expr]) -> R<Value> {
        let is_proc = self.is_proc(name);
        if !is_proc {
            if matches!(name, "catch" | "catchQuiet") {
                return self.catch(name == "catchQuiet", args.first());
            }
            // size($a) on a variable doesn't copy it.
            if name == "size" && !self.is_command(name) {
                if let [Expr::Var(n)] = args {
                    if let Some(v) = self.lookup(n) {
                        if matches!(v, Value::Vector(_) | Value::Int(_) | Value::Float(_) | Value::Matrix(..)) {
                            return err("Wrong type of argument on call to size.");
                        }
                        return Ok(Value::Int(v.len() as i32));
                    }
                }
            }
        }
        let mut vals = Vec::with_capacity(args.len());
        for a in args {
            vals.push(self.eval(a)?);
        }
        let targets: Vec<Option<&Expr>> = args.iter().map(Some).collect();
        if is_proc {
            return self.call_proc(name, vals, &targets);
        }
        if self.is_command(name) {
            let mut toks = Vec::new();
            for v in &vals {
                // In function syntax a string spelled as a flag is one: ls("-sl").
                match v {
                    Value::Str(s) if flag_spelling(s) => toks.push(Tok::Word(s.clone())),
                    v => push_tokens(v, &mut toks),
                }
            }
            return self.run_command(name, &toks);
        }
        self.call_builtin(name, vals, &targets)
    }

    fn call_command(&mut self, name: &str, args: &[CmdArg]) -> R<Value> {
        let is_proc = self.is_proc(name);
        if !is_proc && matches!(name, "catch" | "catchQuiet") {
            let e = args.iter().find_map(|a| if let CmdArg::Expr(e) = a { Some(e) } else { None });
            return self.catch(name == "catchQuiet", e);
        }
        if !is_proc && self.is_command(name) {
            let mut toks = Vec::new();
            for a in args {
                match a {
                    CmdArg::Word(w) => toks.push(Tok::Word(w.clone())),
                    CmdArg::Expr(e) => {
                        let v = self.eval(e)?;
                        push_tokens(&v, &mut toks);
                    }
                }
            }
            return self.run_command(name, &toks);
        }
        // Procedures and built-ins take values: a bare word is a number by its look, or a string.
        let mut vals = Vec::with_capacity(args.len());
        let mut targets = Vec::with_capacity(args.len());
        for a in args {
            match a {
                CmdArg::Word(w) => {
                    vals.push(match word_value(w) {
                        J::Number(n) => Value::from_json(&J::Number(n)),
                        _ => Value::Str(w.clone()),
                    });
                    targets.push(None);
                }
                CmdArg::Expr(e) => {
                    vals.push(self.eval(e)?);
                    targets.push(Some(e));
                }
            }
        }
        if is_proc {
            return self.call_proc(name, vals, &targets);
        }
        self.call_builtin(name, vals, &targets)
    }

    fn run_command(&mut self, name: &str, toks: &[Tok]) -> R<Value> {
        let j = self.core.run_mel_command(name, toks)?;
        let v = Value::from_json(&j);
        self.st().last_raw = Some(j);
        Ok(v)
    }

    fn call_builtin(&mut self, name: &str, mut vals: Vec<Value>, targets: &[Option<&Expr>]) -> R<Value> {
        let Some(r) = super::builtins::call(self, name, &mut vals) else {
            if self.auto_source(name)? {
                return self.call_proc(name, vals, targets);
            }
            return err(format!("Cannot find procedure \"{name}\"."));
        };
        let out = r?;
        if let Some(i) = writes_back(name, vals.len()) {
            if let (Some(Some(t)), Some(v)) = (targets.get(i), vals.get(i)) {
                if is_lvalue(t) {
                    self.assign(t, v.clone())?;
                }
            }
        }
        self.st().last_raw = None;
        Ok(out)
    }

    /// An unknown name found as name.mel on the script path is sourced, as the reference does.
    fn auto_source(&mut self, name: &str) -> R<bool> {
        let Some(path) = super::builtins::find_script(name) else { return Ok(false) };
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(_) => return Ok(false),
        };
        self.run_quiet(&text, Some(&path.to_string_lossy()))?;
        Ok(self.is_proc(name))
    }

    fn call_proc(&mut self, name: &str, vals: Vec<Value>, targets: &[Option<&Expr>]) -> R<Value> {
        let Some((p, unit)) = self.find_proc(name) else { return err(format!("Cannot find procedure \"{name}\".")) };
        if vals.len() != p.params.len() {
            return err(format!("Wrong number of arguments on call to {name}."));
        }
        if self.st().calls >= MAX_CALL_DEPTH {
            return err(format!("Procedure calls nested too deeply in {name}."));
        }
        let mut scope = HashMap::with_capacity(p.params.len());
        for ((ty, pname), v) in p.params.iter().zip(vals) {
            let converted = match (&v, ty) {
                (Value::Matrix(..), Ty::Matrix) => v,
                _ => v.coerce(*ty)?,
            };
            scope.insert(pname.clone(), Slot::Local(converted));
        }
        let saved = self.st().cur;
        self.st().frames.push(Frame { scopes: vec![scope], unit });
        let here = self.st().frames.len() - 1;
        self.st().cur = here;
        self.st().calls += 1;
        let r = self.exec_block(&p.body);
        self.st().calls -= 1;
        self.st().cur = saved;
        // Arrays are passed by reference: what the procedure did to them shows in the caller's variable.
        let mut frame = self.st().frames.drain(here..).next();
        let mut written = Vec::new();
        if let Some(f) = frame.as_mut() {
            for (i, (ty, pname)) in p.params.iter().enumerate() {
                if ty.is_array() {
                    if let (Some(Some(t)), Some(Slot::Local(v))) = (targets.get(i), f.scopes.first_mut().and_then(|s| s.remove(pname))) {
                        if is_lvalue(t) {
                            written.push((*t, v));
                        }
                    }
                }
            }
        }
        let out = match r {
            Ok(_) => Value::Void,
            Err(Flow::Return(v)) => v,
            Err(Flow::Break | Flow::Continue) => Value::Void,
            Err(e) => return Err(e),
        };
        for (t, v) in written {
            self.assign(t, v)?;
        }
        self.st().last_raw = None;
        match p.ret {
            Some(t) => match (&out, t) {
                (Value::Matrix(..), Ty::Matrix) => Ok(out),
                _ => Ok(out.coerce(t)?),
            },
            None => Ok(out),
        }
    }

    fn catch(&mut self, quiet: bool, e: Option<&Expr>) -> R<Value> {
        let Some(e) = e else { return Ok(Value::Int(0)) };
        match self.eval(e) {
            Ok(_) => Ok(Value::Int(0)),
            Err(Flow::Error(m)) => {
                if !quiet {
                    self.flush_print();
                    self.core.push(format!("// Error: {m} //"), LineKind::Error);
                }
                Ok(Value::Int(1))
            }
            Err(f) => Err(f),
        }
    }

    pub fn defer(&mut self, src: String) {
        self.st().deferred.push(src);
    }
}

fn is_lvalue(e: &Expr) -> bool {
    match e {
        Expr::Var(_) => true,
        Expr::Index(b, _) => is_lvalue(b),
        _ => false,
    }
}

/// An array literal's type is the widest of its elements'.
fn array_of(vals: Vec<Value>) -> R<Value> {
    let rank = |v: &Value| match v {
        Value::Int(_) | Value::IntArr(_) => 0,
        Value::Float(_) | Value::FloatArr(_) => 1,
        Value::Vector(_) | Value::VecArr(_) => 2,
        _ => 3,
    };
    if vals.iter().any(|v| matches!(v, Value::Matrix(..) | Value::Void)) {
        return err("An array can't hold this type of data.");
    }
    if vals.is_empty() {
        return Ok(Value::StrArr(Vec::new()));
    }
    let t = match vals.iter().map(rank).max().unwrap_or(3) {
        0 => Ty::IntArr,
        1 => Ty::FloatArr,
        2 => Ty::VecArr,
        _ => Ty::StrArr,
    };
    let flat: Vec<Value> = vals.iter().flat_map(Value::elements).collect();
    let mut out = t.default_value();
    match &mut out {
        Value::IntArr(a) => a.extend(flat.iter().map(Value::to_int)),
        Value::FloatArr(a) => a.extend(flat.iter().map(Value::to_float)),
        Value::VecArr(a) => a.extend(flat.iter().map(Value::to_vector)),
        Value::StrArr(a) => a.extend(flat.iter().map(Value::to_text)),
        _ => {}
    }
    Ok(out)
}

/// Reading past the end gives the type's zero, as in the reference.
fn element(v: &Value, i: i32) -> R<Value> {
    if i < 0 {
        return err("Array index out of bounds.");
    }
    let i = i as usize;
    Ok(match v {
        Value::IntArr(a) => Value::Int(a.get(i).copied().unwrap_or(0)),
        Value::FloatArr(a) => Value::Float(a.get(i).copied().unwrap_or(0.0)),
        Value::StrArr(a) => Value::Str(a.get(i).cloned().unwrap_or_default()),
        Value::VecArr(a) => Value::Vector(a.get(i).copied().unwrap_or([0.0; 3])),
        Value::Matrix(r, c, d) => {
            if i >= *r {
                return err("Array index out of bounds.");
            }
            Value::FloatArr(d[i * c..(i + 1) * c].to_vec())
        }
        v => return err(format!("Cannot index data of type {}.", v.type_name())),
    })
}

fn matrix_op(op: BinOp, a: &Value, b: &Value) -> R<Value> {
    let scalar = |f: f64, m: &Value, g: &dyn Fn(f64, f64) -> f64| -> Value {
        match m {
            Value::Matrix(r, c, d) => Value::Matrix(*r, *c, d.iter().map(|x| g(*x, f)).collect()),
            v => v.clone(),
        }
    };
    match (a, b, op) {
        (Value::Matrix(r, c, x), Value::Matrix(r2, c2, y), BinOp::Add | BinOp::Sub) => {
            if (r, c) != (r2, c2) {
                return err("Matrices of different sizes.");
            }
            let s = if op == BinOp::Add { 1.0 } else { -1.0 };
            Ok(Value::Matrix(*r, *c, x.iter().zip(y).map(|(p, q)| p + s * q).collect()))
        }
        (Value::Matrix(r, c, x), Value::Matrix(r2, c2, y), BinOp::Mul) => {
            if c != r2 {
                return err("Matrices of different sizes.");
            }
            let mut d = vec![0.0; r * c2];
            for i in 0..*r {
                for j in 0..*c2 {
                    for k in 0..*c {
                        d[i * c2 + j] += x[i * c + k] * y[k * c2 + j];
                    }
                }
            }
            Ok(Value::Matrix(*r, *c2, d))
        }
        (Value::Matrix(_, _, x), Value::Matrix(_, _, y), BinOp::Eq | BinOp::Ne) => Ok(Value::Int(((x == y) == (op == BinOp::Eq)) as i32)),
        (m @ Value::Matrix(..), Value::Int(_) | Value::Float(_), BinOp::Mul) => Ok(scalar(b.to_float(), m, &|x, f| x * f)),
        (Value::Int(_) | Value::Float(_), m @ Value::Matrix(..), BinOp::Mul) => Ok(scalar(a.to_float(), m, &|x, f| x * f)),
        (m @ Value::Matrix(..), Value::Int(_) | Value::Float(_), BinOp::Div) => {
            if b.to_float() == 0.0 {
                return err("Attempt to divide by zero.");
            }
            Ok(scalar(b.to_float(), m, &|x, f| x / f))
        }
        _ => err("Illegal operation on data of type matrix."),
    }
}

/// A value as command arguments: arrays and vectors spread out.
fn push_tokens(v: &Value, toks: &mut Vec<Tok>) {
    match v {
        Value::Void => {}
        Value::Vector(x) => toks.extend(x.iter().map(|f| Tok::Val(Value::Float(*f).to_json()))),
        Value::VecArr(a) => {
            for x in a {
                toks.extend(x.iter().map(|f| Tok::Val(Value::Float(*f).to_json())));
            }
        }
        Value::Matrix(_, _, d) => toks.extend(d.iter().map(|f| Tok::Val(Value::Float(*f).to_json()))),
        a if a.is_array() => toks.extend(a.elements().iter().map(|e| Tok::Val(e.to_json()))),
        s => toks.push(Tok::Val(s.to_json())),
    }
}
