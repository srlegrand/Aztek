//! Command syntax: `polyCube -w 2 -sx 3`, turned into a command call. The interpreter evaluates the arguments to tokens;
//! flags are looked up by long or short name and each takes as many values as its type or arity needs.

use aztek_ops::{ArgType, Commands, FlagDef};
use aztek_scene::Session;
use serde_json::{json, Map, Value};

/// One evaluated argument of a command: a bare word as typed, or a value (quoted strings, variables, expressions).
/// Only a word can be a flag; function syntax turns "-sl" into a word first.
#[derive(Clone, Debug, PartialEq)]
pub enum Tok {
    Word(String),
    Val(Value),
}

fn tokens(line: &str) -> Vec<Tok> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                let mut s = String::new();
                while let Some(c) = chars.next() {
                    if c == '\\' {
                        if let Some(n) = chars.next() {
                            s.push(match n {
                                'n' => '\n',
                                't' => '\t',
                                o => o,
                            });
                        }
                    } else if c == '"' {
                        break;
                    } else {
                        s.push(c);
                    }
                }
                out.push(Tok::Val(json!(s)));
            }
            c if c.is_whitespace() => {
                if !cur.is_empty() {
                    out.push(Tok::Word(std::mem::take(&mut cur)));
                }
            }
            _ => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(Tok::Word(cur));
    }
    out
}

/// Splits on `;` outside quotes.
pub fn statements(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    for c in src.chars() {
        if c == '"' {
            quoted = !quoted;
        }
        if c == ';' && !quoted {
            out.push(std::mem::take(&mut cur));
        } else {
            cur.push(c);
        }
    }
    out.push(cur);
    out.into_iter().map(|s| s.trim().to_string()).filter(|s| !s.is_empty() && !s.starts_with("//")).collect()
}

/// A bare word's value: on/off words and numbers read as such.
pub fn word_value(w: &str) -> Value {
    match w {
        "true" | "on" | "yes" => return json!(true),
        "false" | "off" | "no" => return json!(false),
        _ => {}
    }
    if let Ok(i) = w.parse::<i64>() {
        return json!(i);
    }
    if let Some(i) = w.strip_prefix("0x").and_then(|h| i64::from_str_radix(h, 16).ok()) {
        return json!(i);
    }
    if let Ok(f) = w.parse::<f64>() {
        if f.is_finite() {
            return json!(f);
        }
    }
    json!(w)
}

fn scalar(t: &Tok) -> Value {
    match t {
        Tok::Word(w) => word_value(w),
        Tok::Val(v) => v.clone(),
    }
}

/// True for text spelled as a flag: a dash then a letter.
pub fn flag_spelling(w: &str) -> bool {
    w.len() > 1 && w.starts_with('-') && w[1..].chars().next().is_some_and(|c| c.is_ascii_alphabetic())
}

fn flag_of(t: &Tok) -> Option<&str> {
    match t {
        Tok::Word(w) if flag_spelling(w) => Some(&w[1..]),
        _ => None,
    }
}

fn is_flag(t: &Tok) -> bool {
    flag_of(t).is_some()
}

fn is_bool_word(t: &Tok) -> bool {
    match t {
        Tok::Word(w) => matches!(w.as_str(), "0" | "1" | "true" | "false" | "on" | "off" | "yes" | "no"),
        Tok::Val(v) => v.as_i64().is_some_and(|i| i == 0 || i == 1),
    }
}

fn bool_of(t: &Tok) -> Value {
    match scalar(t) {
        Value::Number(n) => json!(n.as_i64() != Some(0)),
        v => v,
    }
}

/// Flags that take a value in query mode when they come after -q: the value picks what is asked about, as in
/// `skinPercent -q -t joint1 sk v` or `keyframe -q -at tx -vc pCube1`. All others name what is asked, so a word after
/// them is the object (`xform -q -ws -t joint1`). A non-boolean flag before -q always takes its value.
fn query_takes_value(cmd: &str, flag: &str) -> bool {
    matches!(
        (cmd, flag),
        ("skinPercent", "transform" | "ignoreBelow")
            | ("keyframe", "attribute" | "time" | "index")
            | ("keyTangent", "attribute" | "time")
            | ("hotkey", "keyShortcut")
            | ("cmdFileOutput", "status")
    )
}

/// Whether a command's -e is its own flag and not edit: polyEvaluate's edges, scriptJob's event, timer's end. The
/// command's flag table says, so a new command can't collide.
pub fn short_is_own<C>(def: &aztek_ops::CommandDef<C>, short: &str, generic: &str) -> bool {
    def.flags.iter().any(|f| f.short_name == short && f.name != generic)
}

/// A reference command spelled as one of ours: the command it runs, and its flags (long or short) as ours.
pub struct Alias {
    pub the_reference: &'static str,
    pub target: &'static str,
    pub flags: &'static [(&'static str, &'static str)],
}

/// The reference commands and flags Aztek runs under its own names.
pub const ALIASES: &[Alias] = &[
    Alias { the_reference: "makeIdentity", target: "freezeTransforms", flags: &[("a", "apply")] },
    Alias { the_reference: "CenterPivot", target: "centerPivot", flags: &[] },
    Alias { the_reference: "unfold", target: "u3dUnfold", flags: &[] },
    Alias { the_reference: "polyMultiLayoutUV", target: "u3dLayout", flags: &[("ps", "shellSpacing")] },
    Alias { the_reference: "DeleteHistory", target: "delete", flags: &[] },
];

pub fn alias(name: &str) -> Option<&'static Alias> {
    ALIASES.iter().find(|a| a.the_reference == name)
}

/// Commands whose positional component strings (pCube1.f[0:3]) are what they work on: they're selected first.
pub fn takes_components(name: &str) -> bool {
    // These read their own component arguments, as the reference's do, and leave the selection alone.
    if aztek_scene::mesh_edit::ref_poly::reads_components(name) {
        return false;
    }
    (name.starts_with("poly")
        && !matches!(
            name,
            "polyCube" | "polySphere" | "polyCylinder" | "polyCone" | "polyTorus" | "polyPlane" | "polyDisc" | "polyPipe" | "polyCapsule"
                | "polyIcoSphere" | "polyQuadSphere" | "polyCombine" | "polySeparate" | "polyBoolean"
                // UV commands read their own arguments, map[] included.
                | "polyProjection" | "polyPlanarProjection" | "polyCylindricalProjection" | "polySphericalProjection" | "polyAutoProjection" | "polyEditUV" | "polyMapCut" | "polyMapSew" | "polyMapSewMove"
                | "polyNormalizeUV" | "polyFlipUV" | "polyRotateUVs"
        ))
        || matches!(name, "deleteComponents" | "moveComponents" | "rotateComponents" | "scaleComponents" | "convertSelection" | "growSelection" | "shrinkSelection" | "selectLoop" | "selectBorder")
}

pub fn is_component(s: &str) -> bool {
    s.ends_with(']') && s.find('[').is_some_and(|o| s[..o].contains('.'))
}

/// One statement of plain command syntax to (command, call).
pub fn parse(cmds: &Commands<Session>, stmt: &str) -> Result<(String, Value), String> {
    let toks = tokens(stmt);
    let Some(first) = toks.first() else { return Err("empty".into()) };
    let typed = match first {
        Tok::Word(w) => w.clone(),
        Tok::Val(v) => v.as_str().unwrap_or_default().to_string(),
    };
    build(cmds, &typed, &toks[1..])
}

/// Reads the value of a flag at `toks[*i]` by its arity or type.
fn flag_value(fd: &FlagDef, name: &str, toks: &[Tok], i: &mut usize) -> Result<Value, String> {
    let f = &fd.name;
    if fd.arity > 0 {
        let mut v = Vec::new();
        for _ in 0..fd.arity {
            match toks.get(*i) {
                Some(t) if !is_flag(t) => v.push(scalar(t)),
                _ => return Err(format!("{name}: flag -{f} needs {} value(s)", fd.arity)),
            }
            *i += 1;
        }
        return Ok(Value::Array(v));
    }
    Ok(match fd.ty {
        ArgType::Bool => {
            // move -r 1 0 0: move, rotate and scale take their values positionally, so a 1 after a switch is a value.
            if !matches!(name, "move" | "rotate" | "scale") && toks.get(*i).is_some_and(is_bool_word) {
                *i += 1;
                bool_of(&toks[*i - 1])
            } else {
                json!(true)
            }
        }
        ArgType::Vec3 => {
            let mut v = Vec::new();
            for _ in 0..3 {
                if *i < toks.len() {
                    v.push(scalar(&toks[*i]));
                    *i += 1;
                }
            }
            Value::Array(v)
        }
        ArgType::List => {
            // Not multi-use: a run of numbers, the values of an array variable, or one word. What follows is
            // positional.
            let mut v = Vec::new();
            let numbers = toks.get(*i).is_some_and(|t| scalar(t).is_number());
            while let Some(t) = toks.get(*i) {
                if is_flag(t) || (numbers && !scalar(t).is_number()) || (!numbers && !v.is_empty() && matches!(t, Tok::Word(_))) {
                    break;
                }
                v.push(scalar(t));
                *i += 1;
                if !numbers && matches!(t, Tok::Word(_)) {
                    break;
                }
            }
            if v.is_empty() {
                return Err(format!("{name}: flag -{f} needs a value"));
            }
            Value::Array(v)
        }
        ArgType::Any if *i + 1 < toks.len() && scalar(&toks[*i]).is_number() && scalar(&toks[*i + 1]).is_number() && !is_flag(&toks[*i + 1]) => {
            // Several numbers: a vector (-ax 1 0 0).
            let mut v = Vec::new();
            while *i < toks.len() && !is_flag(&toks[*i]) && scalar(&toks[*i]).is_number() {
                v.push(scalar(&toks[*i]));
                *i += 1;
            }
            Value::Array(v)
        }
        _ => {
            if *i >= toks.len() {
                return Err(format!("{name}: flag -{f} needs a value"));
            }
            *i += 1;
            scalar(&toks[*i - 1])
        }
    })
}

/// A command (the reference's spelling or ours) and its evaluated arguments to (command, call).
pub fn build(cmds: &Commands<Session>, typed: &str, toks: &[Tok]) -> Result<(String, Value), String> {
    let al = alias(typed);
    let name = al.map(|a| a.target.to_string()).unwrap_or(typed.to_string());
    let def = cmds.find(&name).ok_or_else(|| format!("Cannot find procedure \"{typed}\"."))?;
    if def.raw_args {
        let args: Vec<Value> = toks.iter().map(scalar).collect();
        return Ok((name, json!({ "args": args })));
    }
    let mut flags = Map::new();
    let mut args = Vec::new();
    let own_query = def.flags.iter().any(|d| d.name == "query");
    let q_own = short_is_own(def, "q", "query");
    let q_pos = toks.iter().position(|t| matches!(flag_of(t), Some("query")) || (!q_own && matches!(flag_of(t), Some("q"))));
    let query = q_pos.is_some();
    let e_own = short_is_own(def, "e", "edit");
    let edit = !e_own && toks.iter().any(|t| matches!(flag_of(t), Some("e" | "edit")));
    let mut asked = Vec::new();
    let mut i = 0;
    while i < toks.len() {
        let t = &toks[i];
        let Some(typed_flag) = flag_of(t) else {
            args.push(scalar(t));
            i += 1;
            continue;
        };
        if matches!(typed_flag, "q" | "query" | "e" | "edit") && !(e_own && typed_flag == "e") {
            i += 1;
            continue;
        }
        let mut f = typed_flag;
        if let Some(a) = al {
            if let Some((_, ours)) = a.flags.iter().find(|(m, _)| *m == f) {
                f = ours;
            }
        }
        let fd = def.flags.iter().find(|d| d.name == f || d.short_name == f);
        if query {
            let long = fd.map(|d| d.name.clone()).unwrap_or(f.to_string());
            let before_q = q_pos.is_some_and(|q| i < q);
            i += 1;
            // Switches (xform -q -ws -t): on unless given a value that says off.
            if fd.is_some_and(|d| d.ty == ArgType::Bool) {
                let on = if toks.get(i).is_some_and(is_bool_word) {
                    i += 1;
                    bool_of(&toks[i - 1]) == json!(true)
                } else {
                    true
                };
                if on {
                    asked.push(json!(long.clone()));
                }
                flags.insert(long, json!(on));
                continue;
            }
            // scriptTable's cell: a row and a column, whichever side of -q it is.
            if long == "cellIndex" {
                let mut at = Vec::new();
                while at.len() < 2 && toks.get(i).is_some_and(|n| !is_flag(n)) {
                    at.push(scalar(&toks[i]));
                    i += 1;
                }
                flags.insert(long, Value::Array(at));
                continue;
            }
            let next_is_value = toks.get(i).is_some_and(|n| !is_flag(n));
            if next_is_value && fd.is_some() && (before_q || query_takes_value(&name, &long)) {
                flags.insert(long, scalar(&toks[i]));
                i += 1;
            } else {
                asked.push(json!(long));
            }
            continue;
        }
        let Some(fd) = fd else {
            // The reference flags with no counterpart that change nothing here: replace-selection, construction history
            // on, relative moves (Aztek's "by"), world space.
            if matches!((name.as_str(), f), ("select", "r") | ("select", "replace") | (_, "ws") | (_, "worldSpace") | (_, "os") | (_, "objectSpace")) {
                i += 1;
                continue;
            }
            if matches!(f, "r" | "relative") && matches!(name.as_str(), "move" | "rotate" | "scale") {
                flags.insert("_relative".into(), json!(true));
                i += 1;
                continue;
            }
            if matches!(f, "a" | "absolute") && matches!(name.as_str(), "move" | "rotate" | "scale") {
                i += 1;
                continue;
            }
            // Flags Aztek doesn't have yet: skip them and their value rather than refuse the whole line.
            let skips = f == "ch" || f == "constructionHistory" || f == "kft" || f == "keepFacesTogether" || f == "d" || f == "divisions" || (name == "setAttr" && matches!(f, "type" | "typ"));
            if skips {
                i += 1;
                let comp = match toks.get(i) {
                    Some(Tok::Word(w)) => is_component(w),
                    Some(Tok::Val(Value::String(w))) => is_component(w),
                    _ => false,
                };
                if i < toks.len() && !is_flag(&toks[i]) && !comp {
                    i += 1;
                }
                continue;
            }
            return Err(format!("{typed}: invalid flag -{f}"));
        };
        i += 1;
        let v = flag_value(fd, &name, toks, &mut i)?;
        if fd.arity > 0 {
            // A multi-use flag gathers its uses into one flat list.
            if let (Some(Value::Array(have)), Value::Array(more)) = (flags.get_mut(&fd.name), &v) {
                have.extend(more.iter().cloned());
                continue;
            }
        }
        flags.insert(fd.name.clone(), v);
    }
    // move/rotate/scale take the values first: "move -r 1 0 0 pCube1".
    if matches!(name.as_str(), "move" | "rotate" | "scale") && args.len() >= 3 && args[..3].iter().all(|a| a.is_number()) {
        let v: Vec<Value> = args.drain(..3).collect();
        let rel = flags.remove("_relative").is_some();
        flags.insert(if rel { "by" } else { "to" }.into(), Value::Array(v));
    }
    flags.remove("_relative");
    if !args.is_empty() {
        flags.insert("args".into(), Value::Array(args));
    }
    if own_query && (query || edit) {
        flags.insert(if query { "query" } else { "edit" }.into(), json!(true));
        if query {
            if def.flags.iter().any(|d| d.name == "queried") {
                flags.insert("queried".into(), Value::Array(asked));
            } else {
                // Commands without a queried list read query flags as switches.
                for a in asked {
                    if let Some(n) = a.as_str() {
                        flags.entry(n.to_string()).or_insert(json!(true));
                    }
                }
            }
        }
    } else if query || edit {
        // The reference's -q/-e on a node's settings (polyCube -q -w pCube1): Core turns these into getAttr/setAttr.
        flags.insert(if query { "_query" } else { "_edit" }.into(), Value::Array(asked));
    }
    Ok((name, Value::Object(flags)))
}

/// The MEL echo of a call, for the history pane.
pub fn echo(name: &str, call: &Value) -> String {
    // move/rotate/scale print as the reference's: values first, -r for relative.
    if matches!(name, "move" | "rotate" | "scale") {
        if let Some(m) = call.as_object() {
            for (k, rel) in [("by", true), ("to", false)] {
                if let Some(Value::Array(v)) = m.get(k) {
                    let mut out = name.to_string();
                    if rel {
                        out.push_str(" -r");
                    }
                    for (key, val) in m {
                        if key != "args" && key != k {
                            match val {
                                Value::Bool(true) => out.push_str(&format!(" -{key}")),
                                Value::Bool(false) => out.push_str(&format!(" -{key} 0")),
                                v => out.push_str(&format!(" -{key} {}", show(v))),
                            }
                        }
                    }
                    for x in v {
                        out.push(' ');
                        out.push_str(&show(x));
                    }
                    if let Some(Value::Array(a)) = m.get("args") {
                        for x in a {
                            out.push(' ');
                            out.push_str(&show(x));
                        }
                    }
                    out.push(';');
                    return out;
                }
            }
        }
    }
    let mut out = name.to_string();
    if let Some(m) = call.as_object() {
        for (k, v) in m {
            if k == "args" {
                continue;
            }
            match v {
                Value::Bool(true) => out.push_str(&format!(" -{k}")),
                Value::Bool(false) => out.push_str(&format!(" -{k} 0")),
                Value::Array(a) => {
                    out.push_str(&format!(" -{k}"));
                    for x in a {
                        out.push(' ');
                        out.push_str(&show(x));
                    }
                }
                v => out.push_str(&format!(" -{k} {}", show(v))),
            }
        }
        if let Some(Value::Array(a)) = m.get("args") {
            for x in a {
                out.push(' ');
                out.push_str(&show(x));
            }
        }
    }
    out.push(';');
    out
}

fn show(v: &Value) -> String {
    match v {
        Value::String(s) if s.contains(' ') || s.is_empty() => format!("\"{s}\""),
        Value::String(s) => s.clone(),
        Value::Number(n) if n.is_f64() => super::value::fmt_float(n.as_f64().unwrap()),
        v => v.to_string(),
    }
}

fn plain(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Bool(b) => (*b as i32).to_string(),
        v => show(v),
    }
}

/// A result as the reference prints it.
pub fn result(v: &Value) -> Option<String> {
    match v {
        Value::Null => None,
        Value::Array(a) => Some(format!("// Result: {} //", a.iter().map(plain).collect::<Vec<_>>().join(" "))),
        v => Some(format!("// Result: {} //", plain(v))),
    }
}
