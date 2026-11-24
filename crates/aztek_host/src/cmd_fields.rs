//! cmdScrollFieldExecuter, cmdScrollFieldReporter and cmdShell: the script editor's fields as controls scripts build;
//! and colorEditor, the modal colour picker.
//!
//! The fields are controls of the script UI model (scriptui), which stores their flags and has the shell draw them. This
//! module adds what they do: the text with its caret and selection, undo and redo, the clipboard, files and stored
//! contents, searching; the executer running its text; the reporter showing the history; the shell running the lines
//! typed into it. Positions are characters from 0; a selection runs from its start up to, not including, its end.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use aztek_ops::{fail, ArgType, Args, CmdResult, CommandDef, CommandError, FlagDef};
use aztek_scene::Session;
use serde_json::{json, Map, Value};

use crate::core::{Core, LineKind};
use crate::scriptui::{call_hook, callback_value, command_for, control_full_path, control_kind, control_props, model_edit, run_soon};

pub(crate) const EXECUTER_CMD: &str = "cmdScrollFieldExecuter";
pub(crate) const REPORTER_CMD: &str = "cmdScrollFieldReporter";
pub(crate) const SHELL_CMD: &str = "cmdShell";

/// The executer's own flags, past the ones every control has.
pub(crate) const EXECUTER: &str = "appendText:at:a autoCloseBraces:acb:b clear:clr:b commandCompletion:cco:b copySelection:cp:b currentLine:cl:a cutSelection:ct:b execute:exc:b executeAll:exa:b fileChangedCommand:fcc:a filename:fn:b filterKeyPress:fkp:a hasFocus:hf:b hasSelection:hsl:b insertText:it:a load:ld:b loadContents:ldc:a loadFile:ldf:a modificationChangedCommand:mcc:a modified:mod:b numberOfLines:nl:b objectPathCompletion:opc:b pasteSelection:pst:b redo:rd:b removeStoredContents:rsc:a replaceAll:rpa:a2 saveFile:svf:a saveSelection:sv:a saveSelectionToShelf:svs:b searchAndSelect:sas:b searchDown:sd:b searchMatchCase:smc:b searchString:ss:a searchWraps:sw:b select:sl:a2 selectAll:sla:b selectedText:slt:b showLineNumbers:sln:b showTabsAndSpaces:sts:b showTooltipHelp:sth:b source:src:b sourceType:st:a spacesPerTab:spt:a storeContents:stc:a tabsForIndent:tfi:b text:t:a textLength:tl:b undo:ud:b";

/// The reporter's own flags.
pub(crate) const REPORTER: &str = "clear:clr:b copySelection:cp:b cutSelection:ct:b echoAllCommands:eac:b filterSourceType:fst:a hasFocus:hf:b lineNumbers:ln:b pasteSelection:pst:b receiveFocusCommand:rfc:a saveSelection:sv:a saveSelectionToShelf:svs:b select:sl:a2 selectAll:sla:b stackTrace:st:b suppressErrors:se:b suppressInfo:si:b suppressResults:sr:b suppressStackTrace:sst:b suppressWarnings:sw:b text:t:a textLength:tl:b";

/// The shell's own flags.
pub(crate) const SHELL: &str = "clear:cl:b command:c:a numberOfHistoryLines:nhl:a numberOfSavedLines:nsl:a prompt:pr:a";

/// What the shell starts with and shows after each run, when -prompt gives nothing.
const DEFAULT_PROMPT: &str = "% ";

/// Flags that act rather than set: kept out of the stored flags and worked here.
const EXEC_ACTIONS: &[&str] = &["appendText", "clear", "copySelection", "currentLine", "cutSelection", "execute", "executeAll", "insertText", "load", "loadContents", "loadFile", "modified", "pasteSelection", "redo", "removeStoredContents", "replaceAll", "saveFile", "saveSelection", "saveSelectionToShelf", "select", "selectAll", "source", "storeContents", "text", "undo", "filterKeyPress", "filename", "hasFocus", "hasSelection", "numberOfLines", "searchAndSelect", "selectedText", "textLength"];
const REPORT_ACTIONS: &[&str] = &["clear", "copySelection", "cutSelection", "pasteSelection", "saveSelection", "saveSelectionToShelf", "select", "selectAll", "text", "echoAllCommands", "lineNumbers", "stackTrace", "hasFocus", "textLength"];
const SHELL_ACTIONS: &[&str] = &["clear", "prompt", "numberOfHistoryLines", "numberOfSavedLines"];

pub(crate) fn is_field(kind: &str) -> bool {
    matches!(kind, EXECUTER_CMD | REPORTER_CMD | SHELL_CMD)
}

#[derive(Default)]
struct Reporter {
    /// What -text set, or what was left after a cut or paste: shown before the history lines.
    base: String,
    /// The history line it shows from, counted from the first line of the session.
    from: usize,
}

#[derive(Default)]
struct History {
    /// (text, kind, from Python: Some(true), from MEL: Some(false), either: None)
    lines: Vec<(String, LineKind, Option<bool>)>,
    /// Lines dropped off the front to keep it bounded.
    dropped: usize,
}

thread_local! {
    static CLIP: RefCell<String> = const { RefCell::new(String::new()) };
    static UNDO: RefCell<HashMap<String, (Vec<String>, Vec<String>)>> = RefCell::new(HashMap::new());
    static HIST: RefCell<History> = RefCell::new(History::default());
    static REPORTERS: RefCell<HashMap<String, Reporter>> = RefCell::new(HashMap::new());
    static COLOR: RefCell<([f64; 3], f64, bool)> = const { RefCell::new(([0.0; 3], 1.0, false)) };
}

// ---------------------------------------------------------------------------------------------------------------------
// Small helpers

fn text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        Value::Bool(b) => (*b as i32).to_string(),
        o => o.to_string(),
    }
}

fn flat(v: &Value) -> Vec<Value> {
    match v {
        Value::Array(a) => a.iter().flat_map(flat).collect(),
        o => vec![o.clone()],
    }
}

fn number(v: &Value) -> f64 {
    v.as_f64().or_else(|| v.as_bool().map(|b| b as i32 as f64)).or_else(|| v.as_str().and_then(|s| s.trim().parse().ok())).unwrap_or(0.0)
}

fn truth(v: &Value) -> bool {
    v.as_bool().unwrap_or_else(|| number(v) != 0.0 || matches!(v.as_str(), Some("true" | "on" | "yes")))
}

fn err(m: impl Into<String>) -> CommandError {
    CommandError::new(m.into())
}

fn mel_quote(t: &str) -> String {
    format!("\"{}\"", t.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n").replace('\r', "\\r").replace('\t', "\\t"))
}

/// The flag asked about in a query: the first of -queried, else the first flag given.
fn asked(a: &Args) -> Option<String> {
    let mut list: Vec<String> = a.get("queried").and_then(Value::as_array).map(|q| q.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default();
    for k in a.flags.keys() {
        if !matches!(k.as_str(), "query" | "queried" | "edit") && !list.contains(k) {
            list.push(k.clone());
        }
    }
    list.into_iter().next()
}

fn prop(path: &str, key: &str) -> Option<Value> {
    control_props(path).and_then(|p| p.get(key).cloned()).filter(|v| !v.is_null())
}

fn edit(path: &str, props: Map<String, Value>) -> Result<(), CommandError> {
    if props.is_empty() {
        return Ok(());
    }
    model_edit(path, &props).map_err(err)
}

fn edit1(path: &str, key: &str, v: Value) -> Result<(), CommandError> {
    let mut m = Map::new();
    m.insert(key.into(), v);
    edit(path, m)
}

// ---------------------------------------------------------------------------------------------------------------------
// Text, caret and selection

struct Field {
    text: Vec<char>,
    caret: usize,
    sel: (usize, usize),
}

impl Field {
    fn read(path: &str) -> Field {
        let props = control_props(path).unwrap_or_default();
        let text: Vec<char> = if control_kind(path).as_deref() == Some(REPORTER_CMD) { reporter_text(path, &props).chars().collect() } else { props.get("text").map(text).unwrap_or_default().chars().collect() };
        let n = text.len();
        let caret = props.get("__caret").map(|v| number(v) as usize).unwrap_or(n).min(n);
        let s = props.get("__sel").map(flat).unwrap_or_default();
        let sel = match s.as_slice() {
            [a, b, ..] => {
                let (a, b) = ((number(a) as usize).min(n), (number(b) as usize).min(n));
                (a.min(b), a.max(b))
            }
            _ => (caret, caret),
        };
        Field { text, caret, sel }
    }

    fn string(&self) -> String {
        self.text.iter().collect()
    }

    fn selected(&self) -> String {
        self.text[self.sel.0..self.sel.1].iter().collect()
    }

    fn has_selection(&self) -> bool {
        self.sel.1 > self.sel.0
    }

    /// Puts text in place of the selection (or at the caret), the caret after it.
    fn replace_selection(&mut self, with: &str) {
        let (a, b) = if self.has_selection() { self.sel } else { (self.caret, self.caret) };
        let ins: Vec<char> = with.chars().collect();
        let n = ins.len();
        self.text.splice(a..b, ins);
        self.caret = a + n;
        self.sel = (self.caret, self.caret);
    }

    fn line_of(&self, pos: usize) -> usize {
        self.text[..pos.min(self.text.len())].iter().filter(|c| **c == '\n').count() + 1
    }

    fn line_start(&self, line: usize) -> usize {
        if line <= 1 {
            return 0;
        }
        let mut seen = 1;
        for (i, c) in self.text.iter().enumerate() {
            if *c == '\n' {
                seen += 1;
                if seen == line {
                    return i + 1;
                }
            }
        }
        self.text.len()
    }

    fn lines(&self) -> usize {
        self.text.iter().filter(|c| **c == '\n').count() + 1
    }

    /// The caret and selection as props for the model and the shell.
    fn place_props(&self) -> Map<String, Value> {
        let mut m = Map::new();
        m.insert("__caret".into(), json!(self.caret));
        m.insert("__sel".into(), json!([self.sel.0, self.sel.1]));
        m
    }
}

fn push_undo(path: &str, old: String) {
    UNDO.with(|u| {
        let mut u = u.borrow_mut();
        let e = u.entry(path.to_string()).or_default();
        if e.0.last() != Some(&old) {
            e.0.push(old);
            if e.0.len() > 500 {
                e.0.remove(0);
            }
        }
        e.1.clear();
    });
}

/// Sets the modified state; a change runs -modificationChangedCommand.
fn set_modified(path: &str, on: bool) -> Result<(), CommandError> {
    let was = prop(path, "modified").is_some_and(|v| truth(&v));
    if was == on {
        return Ok(());
    }
    edit1(path, "modified", json!(on))?;
    if let Some(cb) = prop(path, "modificationChangedCommand").filter(|v| v.as_str() != Some("")) {
        run_soon(cb, Vec::new());
    }
    Ok(())
}

/// Writes a field's new text and place, keeping the old text for -undo. A changed text is a modification.
fn write_field(path: &str, f: &Field, old: &str, modifies: bool) -> Result<(), CommandError> {
    let new = f.string();
    let mut props = f.place_props();
    if new != old {
        push_undo(path, old.to_string());
        props.insert("text".into(), json!(new));
    }
    edit(path, props)?;
    if new != old && modifies {
        set_modified(path, true)?;
    }
    Ok(())
}

fn set_clipboard(path: &str, t: String) -> Result<(), CommandError> {
    CLIP.with(|c| *c.borrow_mut() = t.clone());
    // The shell puts it on the system clipboard too.
    edit1(path, "__clipboard", json!(t))
}

// ---------------------------------------------------------------------------------------------------------------------
// Files

/// Where -storeContents keeps contents given by a name rather than a full path.
fn stored_dir() -> PathBuf {
    crate::user_folder::user_pref_dir().join("scriptEditorTemp")
}

fn stored_path(name: &str) -> PathBuf {
    let p = Path::new(name);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        stored_dir().join(name)
    }
}

fn mtime(p: &str) -> Value {
    match std::fs::metadata(p).and_then(|m| m.modified()) {
        Ok(t) => json!(t.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as f64).unwrap_or(0.0)),
        Err(_) => Value::Null,
    }
}

fn file_filter(path: &str) -> &'static str {
    if prop(path, "sourceType").is_some_and(|v| text(&v) == "python") {
        "Python (*.py);;All Files (*.*)"
    } else {
        "MEL (*.mel);;All Files (*.*)"
    }
}

/// Asks the shell for a file; None with no shell or when cancelled.
fn ask_file(caption: &str, mode: i32, filter: &str, start: Option<String>) -> Option<String> {
    let mut p = json!({"caption": caption, "fileMode": mode, "fileFilter": filter});
    if let Some(s) = start {
        p["startingDirectory"] = json!(s);
    }
    call_hook("fileDialog2", &p).and_then(|v| flat(&v).first().map(text)).filter(|s| !s.is_empty())
}

fn load_into(path: &str, file: &str) -> Result<(), CommandError> {
    let body = std::fs::read_to_string(file).map_err(|e| err(format!("{EXECUTER_CMD}: can't read {file}: {e}")))?;
    let old = Field::read(path).string();
    if old != body {
        push_undo(path, old);
    }
    let n = body.chars().count();
    let mut m = Map::new();
    m.insert("text".into(), json!(body));
    m.insert("__caret".into(), json!(n));
    m.insert("__sel".into(), json!([n, n]));
    m.insert("filename".into(), json!(file.replace('\\', "/")));
    m.insert("__mtime".into(), mtime(file));
    edit(path, m)?;
    set_modified(path, false)
}

fn save_to(file: &str, body: &str) -> Result<(), CommandError> {
    let p = Path::new(file);
    if let Some(dir) = p.parent().filter(|d| !d.as_os_str().is_empty()) {
        let _ = std::fs::create_dir_all(dir);
    }
    std::fs::write(p, body).map_err(|e| err(format!("{EXECUTER_CMD}: can't write {file}: {e}")))
}

// ---------------------------------------------------------------------------------------------------------------------
// The command

/// The three field commands: create, edit and query, the stored flags through scriptui and the rest here.
pub(crate) fn command(s: &mut Session, cmd: &str, a: &Args) -> CmdResult {
    if a.bool("query") {
        return query(s, cmd, a);
    }
    if a.has("defineTemplate") || (a.bool("exists") && !a.bool("edit")) {
        return command_for(cmd, a);
    }
    let actions = match cmd {
        EXECUTER_CMD => EXEC_ACTIONS,
        REPORTER_CMD => REPORT_ACTIONS,
        _ => SHELL_ACTIONS,
    };
    let mut plain = a.clone();
    for k in actions {
        plain.flags.remove(*k);
    }
    let made = !a.bool("edit");
    let r = command_for(cmd, &plain)?;
    let name = if made { r.as_str().map(str::to_string) } else { a.pos_str(0).map(str::to_string) };
    let Some(path) = name.as_deref().and_then(control_full_path) else { return Ok(r) };
    if made {
        create_state(s, cmd, &path, a)?;
    }
    let out = match cmd {
        EXECUTER_CMD => executer_actions(&path, a)?,
        REPORTER_CMD => reporter_actions(s, &path, a)?,
        _ => shell_actions(&path, a)?,
    };
    Ok(out.unwrap_or(r))
}

fn create_state(_s: &mut Session, cmd: &str, path: &str, a: &Args) -> Result<(), CommandError> {
    UNDO.with(|u| u.borrow_mut().remove(path));
    match cmd {
        REPORTER_CMD => {
            REPORTERS.with(|r| r.borrow_mut().insert(path.to_string(), Reporter { base: String::new(), from: 0 }));
            if !a.has("text") {
                edit1(path, "text", json!(reporter_text(path, &control_props(path).unwrap_or_default())))?;
            }
        }
        SHELL_CMD => {
            let p = a.get("prompt").map(text).unwrap_or_else(|| DEFAULT_PROMPT.to_string());
            let mut m = Map::new();
            m.insert("prompt".into(), json!(p));
            m.insert("text".into(), json!(p));
            m.insert("__history".into(), json!([]));
            edit(path, m)?;
        }
        _ => {
            edit1(path, "modified", json!(false))?;
        }
    }
    Ok(())
}

fn executer_actions(path: &str, a: &Args) -> Result<Option<Value>, CommandError> {
    let mut out = None;
    if let Some(v) = a.get("filterKeyPress") {
        edit1(path, "filterKeyPress", callback_value(v))?;
    }
    if let Some(v) = a.get("text") {
        let old = Field::read(path).string();
        let t = text(v);
        let n = t.chars().count();
        let f = Field { text: t.chars().collect(), caret: n, sel: (n, n) };
        write_field(path, &f, &old, true)?;
    }
    if a.bool("clear") {
        let old = Field::read(path).string();
        write_field(path, &Field { text: Vec::new(), caret: 0, sel: (0, 0) }, &old, true)?;
    }
    if let Some(v) = a.get("loadFile") {
        let file = text(v);
        let file = if Path::new(&file).is_file() { Some(file) } else { ask_file("Load Script", 1, file_filter(path), None) };
        if let Some(f) = file {
            load_into(path, &f)?;
        }
    }
    if a.bool("load") {
        if let Some(f) = ask_file("Load Script", 1, file_filter(path), None) {
            load_into(path, &f)?;
        }
    }
    if let Some(v) = a.get("loadContents") {
        let p = stored_path(&text(v));
        let body = std::fs::read_to_string(&p).map_err(|e| err(format!("{EXECUTER_CMD}: no stored contents {}: {e}", p.display())))?;
        let old = Field::read(path).string();
        let n = body.chars().count();
        write_field(path, &Field { text: body.chars().collect(), caret: n, sel: (n, n) }, &old, false)?;
    }
    if let Some(v) = a.get("appendText") {
        let mut f = Field::read(path);
        let old = f.string();
        f.text.extend(text(v).chars());
        write_field(path, &f, &old, true)?;
    }
    if let Some(v) = a.get("select") {
        let vals = flat(v);
        let [s, e, ..] = vals.as_slice() else { return Err(err(format!("{EXECUTER_CMD}: -select takes a start and an end."))) };
        let mut f = Field::read(path);
        let n = f.text.len();
        let (s, e) = ((number(s).max(0.0) as usize).min(n), (number(e).max(0.0) as usize).min(n));
        f.sel = (s.min(e), s.max(e));
        f.caret = e;
        edit(path, f.place_props())?;
    }
    if a.bool("selectAll") {
        let mut f = Field::read(path);
        f.sel = (0, f.text.len());
        f.caret = f.text.len();
        edit(path, f.place_props())?;
    }
    if let Some(v) = a.get("currentLine") {
        let mut f = Field::read(path);
        let line = (number(v).max(1.0) as usize).min(f.lines());
        f.caret = f.line_start(line);
        f.sel = (f.caret, f.caret);
        edit(path, f.place_props())?;
    }
    if a.bool("copySelection") || a.bool("cutSelection") {
        let mut f = Field::read(path);
        if f.has_selection() {
            set_clipboard(path, f.selected())?;
            if a.bool("cutSelection") {
                let old = f.string();
                f.replace_selection("");
                write_field(path, &f, &old, true)?;
            }
        }
    }
    if let Some(v) = a.get("insertText") {
        let mut f = Field::read(path);
        let old = f.string();
        f.replace_selection(&text(v));
        write_field(path, &f, &old, true)?;
    }
    if a.bool("pasteSelection") {
        let clip = CLIP.with(|c| c.borrow().clone());
        let mut f = Field::read(path);
        let old = f.string();
        f.replace_selection(&clip);
        write_field(path, &f, &old, true)?;
    }
    if let Some(v) = a.get("replaceAll") {
        let vals = flat(v);
        let [from, to, ..] = vals.as_slice() else { return Err(err(format!("{EXECUTER_CMD}: -replaceAll takes the text to find and its replacement."))) };
        let (from, to) = (text(from), text(to));
        if !from.is_empty() {
            let case = prop(path, "searchMatchCase").is_some_and(|v| truth(&v));
            let mut f = Field::read(path);
            let old = f.string();
            let new = replace_all(&old, &from, &to, case);
            let n = new.chars().count();
            f.text = new.chars().collect();
            f.caret = f.caret.min(n);
            f.sel = (f.caret, f.caret);
            write_field(path, &f, &old, true)?;
        }
    }
    if a.bool("undo") || a.bool("redo") {
        let undo = a.bool("undo");
        let cur = Field::read(path).string();
        let next = UNDO.with(|u| {
            let mut u = u.borrow_mut();
            let e = u.entry(path.to_string()).or_default();
            let (from, to) = if undo { (&mut e.0, &mut e.1) } else { (&mut e.1, &mut e.0) };
            let t = from.pop()?;
            to.push(cur.clone());
            Some(t)
        });
        if let Some(t) = next {
            let n = t.chars().count();
            let f = Field { text: t.chars().collect(), caret: n, sel: (n, n) };
            let mut props = f.place_props();
            props.insert("text".into(), json!(t));
            edit(path, props)?;
            set_modified(path, true)?;
        }
    }
    if let Some(v) = a.get("modified") {
        set_modified(path, truth(v))?;
    }
    if let Some(v) = a.get("saveFile") {
        let file = text(v);
        if file.is_empty() {
            return Err(err(format!("{EXECUTER_CMD}: -saveFile needs a file.")));
        }
        save_to(&file, &Field::read(path).string())?;
        let mut m = Map::new();
        m.insert("filename".into(), json!(file.replace('\\', "/")));
        m.insert("__mtime".into(), mtime(&file));
        edit(path, m)?;
        set_modified(path, false)?;
    }
    if let Some(v) = a.get("storeContents") {
        let p = stored_path(&text(v));
        save_to(&p.to_string_lossy(), &Field::read(path).string())?;
        out = Some(json!(p.to_string_lossy().replace('\\', "/")));
    }
    if let Some(v) = a.get("removeStoredContents") {
        let p = stored_path(&text(v));
        if p.is_file() {
            std::fs::remove_file(&p).map_err(|e| err(format!("{EXECUTER_CMD}: can't remove {}: {e}", p.display())))?;
        }
    }
    if let Some(v) = a.get("saveSelection") {
        save_selection(path, &text(v))?;
    }
    if a.bool("saveSelectionToShelf") {
        save_to_shelf(path)?;
    }
    if a.bool("source") {
        if let Some(f) = ask_file("Source Script", 1, file_filter(path), None) {
            run_soon(source_script(&f), Vec::new());
        }
    }
    Ok(out)
}

/// What sources a script file: MEL's source, or the file run as Python, by its extension.
fn source_script(file: &str) -> Value {
    let f = file.replace('\\', "/");
    if f.ends_with(".py") {
        json!({ "__pysrc__": format!("exec(compile(open({0:?}).read(), {0:?}, 'exec'), {{'__name__': '__main__'}})", f) })
    } else {
        json!(format!("source {};", mel_quote(&f)))
    }
}

fn replace_all(hay: &str, from: &str, to: &str, case: bool) -> String {
    if case {
        return hay.replace(from, to);
    }
    let lower_hay: Vec<char> = hay.chars().flat_map(char::to_lowercase).collect();
    // Lowercasing can change lengths; fall back to an exact match when it does.
    if lower_hay.len() != hay.chars().count() {
        return hay.replace(from, to);
    }
    let pat: Vec<char> = from.chars().flat_map(char::to_lowercase).collect();
    let src: Vec<char> = hay.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < src.len() {
        if i + pat.len() <= src.len() && lower_hay[i..i + pat.len()] == pat[..] {
            out.push_str(to);
            i += pat.len();
        } else {
            out.push(src[i]);
            i += 1;
        }
    }
    out
}

/// -searchAndSelect: from the caret, down or up, wrapping and matching case as set. Selects what it finds.
fn search_and_select(path: &str) -> Result<bool, CommandError> {
    let want = prop(path, "searchString").map(|v| text(&v)).unwrap_or_default();
    if want.is_empty() {
        return Ok(false);
    }
    let down = prop(path, "searchDown").is_none_or(|v| truth(&v));
    let wraps = prop(path, "searchWraps").is_none_or(|v| truth(&v));
    let case = prop(path, "searchMatchCase").is_some_and(|v| truth(&v));
    let mut f = Field::read(path);
    let fold = |c: &char| if case { *c } else { c.to_lowercase().next().unwrap_or(*c) };
    let hay: Vec<char> = f.text.iter().map(fold).collect();
    let pat: Vec<char> = want.chars().map(|c| fold(&c)).collect();
    let n = hay.len();
    if pat.len() > n {
        return Ok(false);
    }
    let at = |i: usize| hay[i..i + pat.len()] == pat[..];
    let last = n - pat.len();
    let found = if down {
        let start = f.sel.1.max(f.caret).min(n);
        (start..=last).find(|&i| at(i)).or_else(|| if wraps { (0..start.min(last + 1)).find(|&i| at(i)) } else { None })
    } else {
        let start = f.sel.0.min(f.caret);
        (0..start.min(last + 1)).rev().find(|&i| at(i)).or_else(|| if wraps { (start.min(last + 1)..=last).rev().find(|&i| at(i)) } else { None })
    };
    let Some(i) = found else { return Ok(false) };
    f.sel = (i, i + pat.len());
    f.caret = if down { i + pat.len() } else { i };
    edit(path, f.place_props())?;
    Ok(true)
}

/// -saveSelection: asks where, the default name starting with the prefix given, and writes the selection there.
fn save_selection(path: &str, prefix: &str) -> Result<(), CommandError> {
    let f = Field::read(path);
    let body = if f.has_selection() { f.selected() } else { f.string() };
    let py = prop(path, "sourceType").is_some_and(|v| text(&v) == "python");
    let ext = if py { "py" } else { "mel" };
    let start = crate::user_folder::user_pref_dir().parent().map(|p| p.join("scripts").join(format!("{prefix}.{ext}")).to_string_lossy().replace('\\', "/"));
    if let Some(file) = ask_file("Save Selection", 0, file_filter(path), start) {
        save_to(&file, &body)?;
    }
    Ok(())
}

/// -saveSelectionToShelf: asks for a label and puts the selection (or all the text) on the current shelf as a button.
fn save_to_shelf(path: &str) -> Result<(), CommandError> {
    let f = Field::read(path);
    let body = if f.has_selection() { f.selected() } else { f.string() };
    if body.trim().is_empty() {
        return Ok(());
    }
    let p = json!({"title": "Save Script to Shelf", "message": "Enter Name:", "button": ["OK", "Cancel"], "defaultButton": "OK", "cancelButton": "Cancel", "dismissString": "Cancel"});
    let Some(ans) = call_hook("promptDialog", &p) else { return Ok(()) };
    if text(&ans["button"]) != "OK" {
        return Ok(());
    }
    let label = text(&ans["text"]);
    let lang = if prop(path, "sourceType").is_some_and(|v| text(&v) == "python") { "python" } else { "mel" };
    crate::core::push_ui_request(format!("shelfAdd|{}", json!({"label": label, "command": body, "sourceType": lang})));
    Ok(())
}

fn reporter_actions(s: &mut Session, path: &str, a: &Args) -> Result<Option<Value>, CommandError> {
    if let Some(v) = a.get("echoAllCommands") {
        s.run("commandEcho", json!({"state": truth(v)}))?;
    }
    if let Some(v) = a.get("lineNumbers") {
        s.run("commandEcho", json!({"lineNumbers": truth(v)}))?;
    }
    if let Some(v) = a.get("stackTrace") {
        s.run("stackTrace", json!({"state": truth(v)}))?;
    }
    let total = HIST.with(|h| {
        let h = h.borrow();
        h.dropped + h.lines.len()
    });
    if let Some(v) = a.get("text") {
        REPORTERS.with(|r| r.borrow_mut().insert(path.to_string(), Reporter { base: text(v), from: total }));
        refresh_reporter(path)?;
    }
    if a.bool("clear") {
        REPORTERS.with(|r| r.borrow_mut().insert(path.to_string(), Reporter { base: String::new(), from: total }));
        refresh_reporter(path)?;
    }
    // The filters change what it shows.
    if ["suppressErrors", "suppressInfo", "suppressResults", "suppressStackTrace", "suppressWarnings", "filterSourceType"].iter().any(|k| a.has(k)) && a.bool("edit") {
        refresh_reporter(path)?;
    }
    if let Some(v) = a.get("select") {
        let vals = flat(v);
        let [st, e, ..] = vals.as_slice() else { return Err(err(format!("{REPORTER_CMD}: -select takes a start and an end."))) };
        let mut f = Field::read(path);
        let n = f.text.len();
        let (st, e) = ((number(st).max(0.0) as usize).min(n), (number(e).max(0.0) as usize).min(n));
        f.sel = (st.min(e), st.max(e));
        f.caret = e;
        edit(path, f.place_props())?;
    }
    if a.bool("selectAll") {
        let mut f = Field::read(path);
        f.sel = (0, f.text.len());
        f.caret = f.text.len();
        edit(path, f.place_props())?;
    }
    if a.bool("copySelection") || a.bool("cutSelection") {
        let mut f = Field::read(path);
        if f.has_selection() {
            set_clipboard(path, f.selected())?;
            if a.bool("cutSelection") {
                f.replace_selection("");
                settle_reporter(path, &f, total)?;
            }
        }
    }
    if a.bool("pasteSelection") {
        let clip = CLIP.with(|c| c.borrow().clone());
        let mut f = Field::read(path);
        f.replace_selection(&clip);
        settle_reporter(path, &f, total)?;
    }
    if let Some(v) = a.get("saveSelection") {
        save_selection(path, &text(v))?;
    }
    if a.bool("saveSelectionToShelf") {
        save_to_shelf(path)?;
    }
    Ok(None)
}

/// A reporter's text after a cut or paste: what it shows now becomes its base, history lines after it still added.
fn settle_reporter(path: &str, f: &Field, total: usize) -> Result<(), CommandError> {
    REPORTERS.with(|r| r.borrow_mut().insert(path.to_string(), Reporter { base: f.string(), from: total }));
    let mut props = f.place_props();
    props.insert("text".into(), json!(f.string()));
    edit(path, props)
}

fn refresh_reporter(path: &str) -> Result<(), CommandError> {
    let t = reporter_text(path, &control_props(path).unwrap_or_default());
    let n = t.chars().count();
    let mut m = Map::new();
    m.insert("text".into(), json!(t));
    m.insert("__caret".into(), json!(n));
    m.insert("__sel".into(), json!([n, n]));
    edit(path, m)
}

fn shell_actions(path: &str, a: &Args) -> Result<Option<Value>, CommandError> {
    let mut m = Map::new();
    if let Some(v) = a.get("numberOfHistoryLines") {
        m.insert("numberOfHistoryLines".into(), json!((number(v) as i64).clamp(0, 32767)));
    }
    if let Some(v) = a.get("numberOfSavedLines") {
        m.insert("numberOfSavedLines".into(), json!((number(v) as i64).clamp(1, 32767)));
    }
    edit(path, m)?;
    if let Some(v) = a.get("prompt").filter(|_| a.bool("edit")) {
        let new = text(v);
        let old = prop(path, "prompt").map(|v| text(&v)).unwrap_or_else(|| DEFAULT_PROMPT.to_string());
        let mut t = prop(path, "text").map(|v| text(&v)).unwrap_or_default();
        if t.ends_with(&old) {
            t.truncate(t.len() - old.len());
            t.push_str(&new);
        }
        let mut m = Map::new();
        m.insert("prompt".into(), json!(new));
        m.insert("text".into(), json!(t));
        edit(path, m)?;
    }
    let lines = shell_saved_lines(path);
    if let Some(t) = prop(path, "text").map(|v| text(&v)) {
        let trimmed = keep_last_lines(&t, lines);
        if trimmed != t {
            edit1(path, "text", json!(trimmed))?;
        }
    }
    if a.bool("clear") {
        let p = prop(path, "prompt").map(|v| text(&v)).unwrap_or_else(|| DEFAULT_PROMPT.to_string());
        edit1(path, "text", json!(p))?;
        if let Some(cb) = prop(path, "command").filter(|v| v.as_str() != Some("")) {
            run_soon(cb, Vec::new());
        }
    }
    Ok(None)
}

fn shell_saved_lines(path: &str) -> usize {
    prop(path, "numberOfSavedLines").map(|v| number(&v) as usize).unwrap_or(100).max(1)
}

fn keep_last_lines(t: &str, n: usize) -> String {
    let lines: Vec<&str> = t.split('\n').collect();
    if lines.len() <= n {
        return t.to_string();
    }
    lines[lines.len() - n..].join("\n")
}

// ---------------------------------------------------------------------------------------------------------------------
// Query

fn default_of(kind: &str, key: &str) -> Option<Value> {
    Some(match (kind, key) {
        (EXECUTER_CMD, "autoCloseBraces" | "commandCompletion" | "objectPathCompletion" | "showLineNumbers" | "showTooltipHelp" | "tabsForIndent" | "searchDown" | "searchWraps") => json!(true),
        (EXECUTER_CMD, "showTabsAndSpaces" | "searchMatchCase" | "modified") => json!(false),
        (EXECUTER_CMD, "spacesPerTab") => json!(4),
        (EXECUTER_CMD, "sourceType") => json!("mel"),
        (EXECUTER_CMD, "searchString" | "filename") => json!(""),
        (EXECUTER_CMD, "filterKeyPress" | "fileChangedCommand" | "modificationChangedCommand") => json!(""),
        (REPORTER_CMD, "suppressErrors" | "suppressInfo" | "suppressResults" | "suppressStackTrace" | "suppressWarnings") => json!(false),
        (REPORTER_CMD, "filterSourceType") => json!(""),
        (SHELL_CMD, "numberOfHistoryLines") => json!(10),
        (SHELL_CMD, "numberOfSavedLines") => json!(100),
        (SHELL_CMD, "prompt") => json!(DEFAULT_PROMPT),
        (SHELL_CMD, "command") => json!(""),
        (_, "hasFocus") => json!(false),
        _ => return None,
    })
}

fn query(s: &mut Session, cmd: &str, a: &Args) -> CmdResult {
    let Some(key) = asked(a) else { return command_for(cmd, a) };
    if key == "exists" {
        return command_for(cmd, a);
    }
    let Some(name) = a.pos_str(0) else { return command_for(cmd, a) };
    let Some(path) = control_full_path(name).filter(|p| control_kind(p).as_deref() == Some(cmd)) else { return command_for(cmd, a) };
    let f = || Field::read(&path);
    let v = match (cmd, key.as_str()) {
        (EXECUTER_CMD | REPORTER_CMD, "text") => json!(f().string()),
        (EXECUTER_CMD | REPORTER_CMD, "textLength") => json!(f().text.len()),
        (EXECUTER_CMD, "numberOfLines") => json!(f().lines()),
        (EXECUTER_CMD, "currentLine") => {
            let f = f();
            json!(f.line_of(f.caret))
        }
        (EXECUTER_CMD, "selectedText") => json!(f().selected()),
        (EXECUTER_CMD, "hasSelection") => json!(f().has_selection()),
        (EXECUTER_CMD, "searchAndSelect") => json!(search_and_select(&path)?),
        (REPORTER_CMD, "echoAllCommands") => s.run("commandEcho", json!({"query": true, "queried": ["state"]}))?,
        (REPORTER_CMD, "lineNumbers") => s.run("commandEcho", json!({"query": true, "queried": ["lineNumbers"]}))?,
        (REPORTER_CMD, "stackTrace") => s.run("stackTrace", json!({"query": true}))?,
        _ => match prop(&path, &key) {
            Some(v) => v,
            None => match default_of(cmd, &key) {
                Some(v) => v,
                None => return command_for(cmd, a),
            },
        },
    };
    Ok(v)
}

// ---------------------------------------------------------------------------------------------------------------------
// The reporter and the history

/// Whether a history line came from Python, from MEL, or can't be told.
fn source_of(t: &str, kind: LineKind) -> Option<bool> {
    let s = t.trim_start();
    if s.starts_with('#') {
        return Some(true);
    }
    if s.starts_with("//") {
        return Some(false);
    }
    if kind == LineKind::Echo {
        let e = s.trim_end();
        return Some(!(e.ends_with(';') || e.ends_with('}') || e.is_empty()));
    }
    None
}

fn is_stack_line(t: &str) -> bool {
    let s = t.trim_start_matches("// ").trim_start_matches("# ");
    s.starts_with("Stack trace") || s.starts_with("Traceback") || t.starts_with("  File ") || t.starts_with("#   File ")
}

fn shows(props: &Map<String, Value>, t: &str, kind: LineKind, py: Option<bool>) -> bool {
    let on = |k: &str| props.get(k).is_some_and(truth);
    let hidden = match kind {
        LineKind::Error => on("suppressErrors"),
        LineKind::Warning => on("suppressWarnings"),
        LineKind::Info => on("suppressInfo"),
        LineKind::Result => on("suppressResults"),
        LineKind::Echo => false,
    };
    if hidden || (on("suppressStackTrace") && is_stack_line(t)) {
        return false;
    }
    match props.get("filterSourceType").map(text).as_deref() {
        Some("mel") => py != Some(false),
        Some("python") => py != Some(true),
        _ => true,
    }
}

fn reporter_text(path: &str, props: &Map<String, Value>) -> String {
    let (base, from) = REPORTERS.with(|r| r.borrow().get(path).map(|x| (x.base.clone(), x.from)).unwrap_or_default());
    let mut out = base;
    HIST.with(|h| {
        let h = h.borrow();
        let start = from.saturating_sub(h.dropped).min(h.lines.len());
        for (t, k, py) in &h.lines[start..] {
            if shows(props, t, *k, *py) {
                out.push_str(t);
                out.push('\n');
            }
        }
    });
    out
}

/// A line gone into the history: kept for the reporters, and sent on to each one that shows it.
pub(crate) fn history_line(t: &str, kind: LineKind) {
    let py = source_of(t, kind);
    HIST.with(|h| {
        let mut h = h.borrow_mut();
        h.lines.push((t.to_string(), kind, py));
        if h.lines.len() > 20000 {
            h.lines.drain(..10000);
            h.dropped += 10000;
        }
    });
    let live = REPORTERS.with(|r| !r.borrow().is_empty());
    if !live {
        return;
    }
    let Some(reps) = crate::scriptui::try_controls_of(REPORTER_CMD) else { return };
    for (path, props) in reps {
        if shows(&props, t, kind, py) {
            let mut m = Map::new();
            m.insert("__append".into(), json!(format!("{t}\n")));
            crate::scriptui::try_model_edit(&path, &m);
        }
    }
}

/// A field gone with its control.
pub(crate) fn forget(path: &str) {
    REPORTERS.with(|r| r.borrow_mut().remove(path));
    UNDO.with(|u| u.borrow_mut().remove(path));
}

// ---------------------------------------------------------------------------------------------------------------------
// The shell's events and the host's side

/// What a field held before the shell's event, for `after_shell_event`.
pub(crate) struct Before {
    path: String,
    kind: String,
    text: String,
}

pub(crate) fn before_shell_event(ev: &Value) -> Option<Before> {
    let path = control_full_path(ev["path"].as_str()?)?;
    let kind = control_kind(&path)?;
    if !is_field(&kind) {
        return None;
    }
    let text = prop(&path, "text").map(|v| text(&v)).unwrap_or_default();
    Some(Before { path, kind, text })
}

/// The scripts a field's event asks for: the executer's text typed (a modification), its Ctrl+Enter (-execute), and a
/// line entered in a shell.
pub(crate) fn after_shell_event(b: Before, ev: &Value) -> Vec<(Value, Vec<Value>)> {
    let mut out = Vec::new();
    if b.kind == EXECUTER_CMD {
        if let Some(t) = ev["set"]["text"].as_str() {
            if t != b.text {
                push_undo(&b.path, b.text.clone());
                let _ = set_modified(&b.path, true);
            }
        }
    }
    for flag in ev["fire"].as_array().into_iter().flatten().filter_map(Value::as_str) {
        match flag {
            "__exec" if b.kind == EXECUTER_CMD => out.push((json!(format!("{EXECUTER_CMD} -e -execute {};", mel_quote(&b.path))), Vec::new())),
            "__execAll" if b.kind == EXECUTER_CMD => out.push((json!(format!("{EXECUTER_CMD} -e -executeAll {};", mel_quote(&b.path))), Vec::new())),
            "__shellRun" if b.kind == SHELL_CMD => {
                let line = ev["args"].as_array().and_then(|a| a.first()).map(text).unwrap_or_default();
                out.push((json!(format!("aztekCmdShellRun {} {};", mel_quote(&b.path), mel_quote(&line))), Vec::new()));
            }
            _ => {}
        }
    }
    out.extend(take_soon());
    out
}

fn take_soon() -> Vec<(Value, Vec<Value>)> {
    crate::scriptui::take_immediate()
}

fn long_keys(s: &Session, name: &str, call: &Value) -> Map<String, Value> {
    let mut out = Map::new();
    let def = s.commands.find(name);
    for (k, v) in call.as_object().into_iter().flatten() {
        let long = def.and_then(|d| d.flags.iter().find(|f| f.name == *k || f.short_name == *k)).map(|f| f.name.clone()).unwrap_or_else(|| k.clone());
        out.insert(long, v.clone());
    }
    out
}

/// What needs the host: the executer running its text, a line run in a shell, a key given to -filterKeyPress.
pub(crate) fn intercept(core: &mut Core, name: &str, call: &Value) -> Option<Result<Value, String>> {
    match name {
        EXECUTER_CMD => {
            let o = long_keys(&core.s, name, call);
            if o.get("query").is_some_and(truth) {
                return None;
            }
            let exec = o.get("execute").is_some_and(truth);
            let all = o.get("executeAll").is_some_and(truth);
            if !exec && !all {
                return None;
            }
            let mut rest = o.clone();
            rest.remove("execute");
            rest.remove("executeAll");
            let r = match core.s.run(name, Value::Object(rest)) {
                Ok(v) => v,
                Err(e) => return Some(Err(e.to_string())),
            };
            let named = o.get("args").and_then(|a| flat(a).first().map(text)).or_else(|| r.as_str().map(str::to_string)).unwrap_or_default();
            let Some(path) = control_full_path(&named) else { return Some(Err(format!("{EXECUTER_CMD}: Object '{named}' not found."))) };
            let f = Field::read(&path);
            let src = if exec && !all && f.has_selection() { f.selected() } else { f.string() };
            run_source(core, &path, &src);
            Some(Ok(r))
        }
        "aztekCmdShellRun" => {
            let args = call.get("args").map(flat).unwrap_or_default();
            let (Some(p), line) = (args.first().map(text), args.get(1).map(text).unwrap_or_default()) else { return Some(Err("aztekCmdShellRun: give the shell and the line.".into())) };
            Some(shell_run(core, &p, &line))
        }
        "aztekCmdFieldKey" => {
            let args = call.get("args").map(flat).unwrap_or_default();
            let p = args.first().map(text).unwrap_or_default();
            let mods = args.get(1).map(|v| number(v) as i64).unwrap_or(0);
            let key = args.get(2).map(text).unwrap_or_default();
            Some(Ok(json!(filter_key(core, &p, mods, &key))))
        }
        _ => None,
    }
}

/// Runs an executer's text in its language, echoed to the history as the Script Editor does; errors go there too.
fn run_source(core: &mut Core, path: &str, src: &str) {
    if src.trim().is_empty() {
        return;
    }
    if prop(path, "sourceType").is_some_and(|v| text(&v) == "python") {
        core.push(src.to_string(), LineKind::Echo);
        core.run_callback(&json!({ "__pysrc__": src }), &[]);
    } else {
        core.mel(src);
    }
}

/// A line entered in a shell: run as MEL, what it printed and gave added to the field after the line, then the prompt.
fn shell_run(core: &mut Core, name: &str, line: &str) -> Result<Value, String> {
    let Some(path) = control_full_path(name).filter(|p| control_kind(p).as_deref() == Some(SHELL_CMD)) else { return Err(format!("{SHELL_CMD}: Object '{name}' not found.")) };
    let prompt = prop(&path, "prompt").map(|v| text(&v)).unwrap_or_else(|| DEFAULT_PROMPT.to_string());
    let before = core.log.len();
    let _ = core.mel_checked(line);
    let said: Vec<String> = core.log.get(before..).unwrap_or(&[]).iter().filter(|l| l.kind != LineKind::Echo).map(|l| l.text.clone()).collect();
    let mut t = prop(&path, "text").map(|v| text(&v)).unwrap_or_default();
    if !t.ends_with(&prompt) {
        if !t.is_empty() && !t.ends_with('\n') {
            t.push('\n');
        }
        t.push_str(&prompt);
    }
    t.push_str(line);
    t.push('\n');
    for s in &said {
        t.push_str(s);
        t.push('\n');
    }
    t.push_str(&prompt);
    let t = keep_last_lines(&t, shell_saved_lines(&path));
    let keep = prop(&path, "numberOfHistoryLines").map(|v| number(&v) as usize).unwrap_or(10);
    let mut hist: Vec<String> = prop(&path, "__history").map(|v| flat(&v).iter().map(text).collect()).unwrap_or_default();
    if !line.trim().is_empty() && keep > 0 {
        hist.retain(|h| h != line);
        hist.push(line.to_string());
        let n = hist.len();
        if n > keep {
            hist.drain(..n - keep);
        }
    }
    let n = t.chars().count();
    let mut m = Map::new();
    m.insert("text".into(), json!(t));
    m.insert("__history".into(), json!(hist));
    m.insert("__caret".into(), json!(n));
    m.insert("__sel".into(), json!([n, n]));
    model_edit(&path, &m)?;
    if let Some(cb) = prop(&path, "command").filter(|v| v.as_str() != Some("")) {
        core.run_callback(&cb, &[]);
    }
    Ok(json!(path))
}

/// -filterKeyPress: the script given the modifiers (Shift 1, Ctrl 4, Alt 8, the Windows or Command key 16) and the key;
/// 1 when it took the key.
fn filter_key(core: &mut Core, name: &str, mods: i64, key: &str) -> i64 {
    let Some(path) = control_full_path(name) else { return 0 };
    let Some(cb) = prop(&path, "filterKeyPress") else { return 0 };
    let v = match &cb {
        Value::String(p) if !p.trim().is_empty() => crate::mel::call_for_value(core, &format!("{}({mods}, {})", p.trim(), mel_quote(key))).unwrap_or(Value::Null),
        Value::Object(o) => match o.get("__pycb__").and_then(Value::as_i64) {
            Some(id) => crate::core::python_value(core, id, &[json!(mods), json!(key)]).unwrap_or(Value::Null),
            None => Value::Null,
        },
        _ => Value::Null,
    };
    (number(&v) != 0.0) as i64
}

/// -fileChangedCommand: each executer whose file changed on disk, was renamed or went since it was loaded or saved.
pub(crate) fn file_changes() -> Vec<(Value, Vec<Value>)> {
    let mut out = Vec::new();
    for (path, props) in crate::scriptui::try_controls_of(EXECUTER_CMD).unwrap_or_default() {
        let file = props.get("filename").map(text).unwrap_or_default();
        if file.is_empty() {
            continue;
        }
        let Some(cb) = props.get("fileChangedCommand").filter(|v| !v.is_null() && v.as_str() != Some("")).cloned() else { continue };
        let now = mtime(&file);
        let was = props.get("__mtime").cloned().unwrap_or(Value::Null);
        if now == was {
            continue;
        }
        let mut m = Map::new();
        m.insert("__mtime".into(), now);
        crate::scriptui::try_model_edit(&path, &m);
        match &cb {
            Value::String(src) => out.push((json!(format!("{src} {}", mel_quote(&file))), Vec::new())),
            o => out.push((o.clone(), vec![json!(file)])),
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------------------------------
// colorEditor

fn rgb_to_hsv([r, g, b]: [f64; 3]) -> [f64; 3] {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let h = if d <= 0.0 {
        0.0
    } else if max == r {
        60.0 * (((g - b) / d).rem_euclid(6.0))
    } else if max == g {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    let s = if max <= 0.0 { 0.0 } else { d / max };
    [h, s, max]
}

fn hsv_to_rgb([h, s, v]: [f64; 3]) -> [f64; 3] {
    let h = h.rem_euclid(360.0) / 60.0;
    let c = v * s;
    let x = c * (1.0 - (h.rem_euclid(2.0) - 1.0).abs());
    let (r, g, b) = match h as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    [r + m, g + m, b + m]
}

fn three(v: &Value) -> Option<[f64; 3]> {
    let f = flat(v);
    (f.len() >= 3).then(|| [number(&f[0]), number(&f[1]), number(&f[2])])
}

fn color_editor(_s: &mut Session, a: &Args) -> CmdResult {
    if a.bool("query") {
        let (rgb, alpha, ok) = COLOR.with(|c| *c.borrow());
        let key = asked(a).unwrap_or_default();
        return Ok(match key.as_str() {
            "rgbValue" => json!(rgb),
            "hsvValue" => json!(rgb_to_hsv(rgb)),
            "alpha" => json!(alpha),
            "result" => json!(ok),
            other => fail!("colorEditor: -{other} can't be queried."),
        });
    }
    let mut rgb = a.get("rgbValue").and_then(three).or_else(|| a.get("hsvValue").and_then(three).map(hsv_to_rgb)).unwrap_or([0.0; 3]);
    for c in &mut rgb {
        *c = c.clamp(0.0, 1.0);
    }
    let mut alpha = a.get("alpha").map(number).unwrap_or(1.0).clamp(0.0, 1.0);
    let mut p = json!({"rgbValue": rgb, "alpha": alpha, "mini": a.bool("mini"), "useDisplaySpace": a.bool("useDisplaySpace"), "useVpColorPicker": a.bool("useVpColorPicker")});
    if let Some(v) = a.get("parent") {
        p["parent"] = json!(text(v));
    }
    if let Some(v) = a.get("position") {
        p["position"] = Value::Array(flat(v).iter().map(|x| json!(number(x) as i64)).collect());
    }
    // With no shell to show it, the dialog gives back what it was opened with, as pressing OK would.
    let mut ok = true;
    if let Some(ans) = call_hook("colorEditor", &p) {
        ok = ans["result"].as_bool().unwrap_or(false);
        if let Some(c) = ans.get("rgbValue").and_then(three) {
            rgb = c;
        }
        if let Some(x) = ans["alpha"].as_f64() {
            alpha = x;
        }
    }
    COLOR.with(|c| *c.borrow_mut() = (rgb, alpha, ok));
    let f = crate::mel::fmt_float;
    Ok(json!(format!("{} {} {} {}", f(rgb[0]), f(rgb[1]), f(rgb[2]), ok as i32)))
}

// ---------------------------------------------------------------------------------------------------------------------

fn flag(name: &str, short: &str, ty: ArgType) -> FlagDef {
    FlagDef::new(name, short, ty, Value::Null, "")
}

pub(crate) fn register(s: &mut Session) {
    s.register(
        CommandDef::new(
            "colorEditor",
            "The modal colour picker: opens on -rgbValue or -hsvValue (black when neither), -alpha 1 unless given. Returns \"r g b 1\" when OK was pressed, \"r g b 0\" when cancelled; -q -rgbValue, -hsvValue, -alpha and -result read what it gave last.",
            color_editor,
        )
        .undoable(false)
        .flag(flag("query", "q", ArgType::Bool))
        .flag(flag("queried", "", ArgType::Any))
        .flag(flag("alpha", "a", ArgType::Any))
        .flag(flag("hsvValue", "hsv", ArgType::Any).multi(3))
        .flag(flag("rgbValue", "rgb", ArgType::Any).multi(3))
        .flag(flag("mini", "m", ArgType::Bool))
        .flag(flag("parent", "p", ArgType::Any))
        .flag(flag("position", "pos", ArgType::Any).multi(2))
        .flag(flag("result", "r", ArgType::Bool))
        .flag(flag("useDisplaySpace", "uds", ArgType::Bool))
        .flag(flag("useVpColorPicker", "vcp", ArgType::Bool)),
    );
    s.register(
        CommandDef::new("aztekCmdShellRun", "For a cmdShell: runs a line typed into it, as MEL, and adds the line and what it printed and gave to the field.", |_s, _a| {
            Err(CommandError::new("aztekCmdShellRun: needs the application's host."))
        })
        .positional("shell line")
        .undoable(false),
    );
    s.register(
        CommandDef::new("aztekCmdFieldKey", "For a cmdScrollFieldExecuter: gives a key press (modifiers, key) to its -filterKeyPress script. Returns 1 when the script took it.", |_s, _a| Ok(json!(0)))
            .positional("field modifiers key")
            .undoable(false),
    );
}
