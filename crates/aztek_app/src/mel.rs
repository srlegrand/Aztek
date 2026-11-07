//! The command line's syntax: `polyCube -w 2 -sx 3; select -r pCube1.f[0:3];`, turned into command calls.
//! Flags are looked up by long or short name; each takes as many values as its type needs.

use aztek_ops::{ArgType, Commands};
use aztek_scene::Session;
use serde_json::{json, Map, Value};

fn tokens(line: &str) -> Vec<String> {
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
                // Quoted: marked so a quoted "-x" isn't read as a flag.
                out.push(format!("\u{1}{s}"));
            }
            c if c.is_whitespace() => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            _ => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
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

fn word(t: &str) -> (&str, bool) {
    match t.strip_prefix('\u{1}') {
        Some(s) => (s, true),
        None => (t, false),
    }
}

fn scalar(t: &str) -> Value {
    let (w, quoted) = word(t);
    if quoted {
        return json!(w);
    }
    match w {
        "true" | "on" | "yes" => return json!(true),
        "false" | "off" | "no" => return json!(false),
        _ => {}
    }
    if let Ok(i) = w.parse::<i64>() {
        return json!(i);
    }
    if let Ok(f) = w.parse::<f64>() {
        return json!(f);
    }
    json!(w)
}

fn is_flag(t: &str) -> bool {
    let (w, quoted) = word(t);
    !quoted && w.len() > 1 && w.starts_with('-') && w[1..].chars().next().is_some_and(|c| c.is_ascii_alphabetic())
}

fn is_bool_word(t: &str) -> bool {
    let (w, quoted) = word(t);
    !quoted && matches!(w, "0" | "1" | "true" | "false" | "on" | "off" | "yes" | "no")
}

/// One statement to (command, call).
pub fn parse(cmds: &Commands<Session>, stmt: &str) -> Result<(String, Value), String> {
    let toks = tokens(stmt);
    let Some(name) = toks.first() else { return Err("empty".into()) };
    let name = word(name).0.to_string();
    let def = cmds.find(&name).ok_or_else(|| format!("Cannot find procedure \"{name}\"."))?;
    let mut flags = Map::new();
    let mut args = Vec::new();
    let mut i = 1;
    while i < toks.len() {
        let t = &toks[i];
        if is_flag(t) {
            let f = &word(t).0[1..];
            let Some(fd) = def.flags.iter().find(|d| d.name == f || d.short_name == f) else {
                // The reference's selection-mode shorthands.
                if name == "select" && f == "r" {
                    i += 1;
                    continue;
                }
                return Err(format!("{name}: invalid flag -{f}"));
            };
            i += 1;
            let v = match fd.ty {
                ArgType::Bool => {
                    if i < toks.len() && is_bool_word(&toks[i]) {
                        i += 1;
                        match scalar(&toks[i - 1]) {
                            Value::Number(n) => json!(n.as_i64() != Some(0)),
                            v => v,
                        }
                    } else {
                        json!(true)
                    }
                }
                ArgType::Vec3 => {
                    let mut v = Vec::new();
                    for _ in 0..3 {
                        if i < toks.len() {
                            v.push(scalar(&toks[i]));
                            i += 1;
                        }
                    }
                    Value::Array(v)
                }
                ArgType::List => {
                    let mut v = Vec::new();
                    while i < toks.len() && !is_flag(&toks[i]) {
                        v.push(scalar(&toks[i]));
                        i += 1;
                    }
                    Value::Array(v)
                }
                _ => {
                    if i >= toks.len() {
                        return Err(format!("{name}: flag -{f} needs a value"));
                    }
                    i += 1;
                    scalar(&toks[i - 1])
                }
            };
            flags.insert(fd.name.clone(), v);
        } else {
            args.push(scalar(t));
            i += 1;
        }
    }
    if !args.is_empty() {
        flags.insert("args".into(), Value::Array(args));
    }
    Ok((name, Value::Object(flags)))
}

/// The MEL echo of a call, for the history pane.
pub fn echo(name: &str, call: &Value) -> String {
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
        v => v.to_string(),
    }
}

/// A result as the reference prints it.
pub fn result(v: &Value) -> Option<String> {
    match v {
        Value::Null => None,
        Value::Array(a) => Some(format!("// Result: {} //", a.iter().map(show).collect::<Vec<_>>().join(" "))),
        v => Some(format!("// Result: {} //", show(v))),
    }
}
