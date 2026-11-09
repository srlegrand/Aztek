//! MEL's own functions and the standard script procedures scripts rely on: strings, math, arrays, environment, eval.

use std::path::{Path, PathBuf};

use super::interp::{err, Flow, Interp, R};
use super::pattern::{glob, regex_find};
use super::value::{Ty, Value};
use crate::core::LineKind;

/// Every name `call` answers.
pub const NAMES: &[&str] = &[
    "getProcArguments", "print", "eval", "evalEcho", "evalDeferred", "source", "error", "warning", "python", "whatIs", "exists", "about", "env", "getenv", "putenv", "size", "sizeBytes",
    "substring", "tokenize", "tokenizeList", "match", "gmatch", "substitute", "substituteAllString", "strip", "toupper", "tolower", "startsWith", "endsWith", "strcmp", "stringArrayToString",
    "stringToStringArray", "stringArrayContains", "stringArrayRemove", "stringArrayRemoveExact", "stringArrayRemoveDuplicates", "stringArrayCount", "stringArrayFind", "stringArrayCatenate",
    "stringArrayInsertAtIndex", "stringArrayReverse", "startString", "endString", "capitalizeString", "shortNameOf", "rootOf", "longNameOf", "basename", "dirname", "fromNativePath",
    "toNativePath", "encodeString", "isValidString", "isValidObjectName", "formValidObjectName", "abs", "sin", "cos", "tan", "asin", "acos", "atan", "atan2", "sind", "cosd", "tand", "asind",
    "acosd", "atand", "atan2d", "sinh", "cosh", "tanh", "sqrt", "pow", "exp", "log", "log10", "floor", "ceil", "round", "trunc", "fmod", "min", "max", "clamp", "sign", "rand", "seed",
    "deg_to_rad", "rad_to_deg", "hypot", "mag", "cross", "dot", "unit", "angle", "linstep", "smoothstep", "sort", "clear", "appendStringArray", "intArrayFind", "floatArrayFind",
    "intArrayContains", "floatArrayContains", "intArrayCount", "floatArrayCount", "intArrayRemove", "floatArrayRemove", "intArrayRemoveDuplicates", "floatArrayRemoveDuplicates",
    "intArrayInsertAtIndex", "floatArrayInsertAtIndex", "intArrayToString", "floatArrayToString", "catch", "catchQuiet",
];

fn wrong(name: &str) -> Flow {
    Flow::Error(format!("Wrong number of arguments on call to {name}."))
}

fn s(v: &Value) -> String {
    v.to_text()
}

fn strs(v: &Value) -> Vec<String> {
    v.elements().iter().map(Value::to_text).collect()
}

fn vs(a: Vec<String>) -> Value {
    Value::StrArr(a)
}

fn chars_between(t: &str, a: usize, b: usize) -> String {
    t.chars().skip(a).take(b.saturating_sub(a)).collect()
}

fn tokenize(text: &str, seps: Option<&str>) -> Vec<String> {
    let is_sep = |c: char| match seps {
        Some(set) => set.contains(c),
        None => c.is_whitespace(),
    };
    text.split(is_sep).filter(|t| !t.is_empty()).map(str::to_string).collect()
}

/// Directories scripts are looked for in.
fn script_dirs() -> Vec<PathBuf> {
    let mut out = vec![PathBuf::from(".")];
    for var in ["AZTEK_SCRIPT_PATH", "MAYA_SCRIPT_PATH"] {
        if let Some(v) = std::env::var_os(var) {
            out.extend(std::env::split_paths(&v));
        }
    }
    out
}

/// `name.mel` on the script path. Only plain names: anything else isn't a procedure's file.
pub fn find_script(name: &str) -> Option<PathBuf> {
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }
    script_dirs().into_iter().map(|d| d.join(format!("{name}.mel"))).find(|p| p.is_file())
}

fn resolve_source(file: &str) -> Option<PathBuf> {
    let direct = Path::new(file);
    let named = if direct.extension().is_some() { file.to_string() } else { format!("{file}.mel") };
    if direct.is_absolute() {
        return [direct.to_path_buf(), PathBuf::from(&named)].into_iter().find(|p| p.is_file());
    }
    for d in script_dirs() {
        for candidate in [d.join(file), d.join(&named)] {
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn step(a: f64, b: f64, x: f64) -> f64 {
    if a == b {
        return if x < a { 0.0 } else { 1.0 };
    }
    ((x - a) / (b - a)).clamp(0.0, 1.0)
}

fn int_like(a: &Value, b: &Value) -> bool {
    matches!((a, b), (Value::Int(_), Value::Int(_)))
}

fn v_mag(v: [f64; 3]) -> f64 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

fn v_dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Next value in 0..1 from the seeded generator.
fn next_rand(it: &mut Interp) -> f64 {
    let st = &mut it.core.mel_state;
    let mut x = st.rng;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    st.rng = x;
    (x >> 11) as f64 / (1u64 << 53) as f64
}

fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => x == y,
        _ => a.to_float() == b.to_float(),
    }
}

/// A builtin by name; None when there is none of that name.
pub fn call(it: &mut Interp, name: &str, a: &mut Vec<Value>) -> Option<R<Value>> {
    if !NAMES.contains(&name) {
        return None;
    }
    Some(run(it, name, a))
}

fn run(it: &mut Interp, name: &str, a: &mut Vec<Value>) -> R<Value> {
    let n = a.len();
    let need = |want: usize| -> R<()> {
        if n == want {
            Ok(())
        } else {
            Err(wrong(name))
        }
    };
    // One float in, one float out.
    let unary: Option<fn(f64) -> f64> = match name {
        "sin" => Some(f64::sin),
        "cos" => Some(f64::cos),
        "tan" => Some(f64::tan),
        "asin" => Some(f64::asin),
        "acos" => Some(f64::acos),
        "atan" => Some(f64::atan),
        "sind" => Some(|x| x.to_radians().sin()),
        "cosd" => Some(|x| x.to_radians().cos()),
        "tand" => Some(|x| x.to_radians().tan()),
        "asind" => Some(|x| x.asin().to_degrees()),
        "acosd" => Some(|x| x.acos().to_degrees()),
        "atand" => Some(|x| x.atan().to_degrees()),
        "sinh" => Some(f64::sinh),
        "cosh" => Some(f64::cosh),
        "tanh" => Some(f64::tanh),
        "sqrt" => Some(f64::sqrt),
        "exp" => Some(f64::exp),
        "log" => Some(f64::ln),
        "log10" => Some(f64::log10),
        "floor" => Some(f64::floor),
        "ceil" => Some(f64::ceil),
        "round" => Some(f64::round),
        "trunc" => Some(f64::trunc),
        "deg_to_rad" => Some(f64::to_radians),
        "rad_to_deg" => Some(f64::to_degrees),
        _ => None,
    };
    if let Some(f) = unary {
        need(1)?;
        if a[0].is_array() || matches!(a[0], Value::Void) {
            return err(format!("Wrong type of argument on call to {name}."));
        }
        return Ok(Value::Float(f(a[0].to_float())));
    }
    match name {
        "getProcArguments" => {
            need(1)?;
            let w = s(&a[0]);
            match it.proc_params(&w) {
                Some(list) => Ok(Value::StrArr(list)),
                None => err(format!("Cannot find procedure \"{w}\".")),
            }
        }
        "print" => {
            for v in a.iter() {
                match v {
                    Value::Str(t) => it.write_print(t),
                    Value::Void => {}
                    Value::Int(_) | Value::Float(_) => it.write_print(&v.to_text()),
                    // Vectors, matrices and each array element get a line.
                    Value::Vector(_) | Value::Matrix(..) => it.write_print(&format!("{}\n", v.to_text())),
                    arr => {
                        for e in arr.elements() {
                            it.write_print(&format!("{}\n", e.to_text()));
                        }
                    }
                }
            }
            Ok(Value::Void)
        }
        "eval" | "evalEcho" => {
            let src = a.iter().map(s).collect::<Vec<_>>().join(" ");
            if name == "evalEcho" {
                it.flush_print();
                it.core.push(src.clone(), LineKind::Echo);
            }
            it.run_quiet(&src, None)
        }
        "evalDeferred" => {
            let src = a.iter().filter(|v| !matches!(v, Value::Str(t) if t.starts_with('-') && t.len() > 1 && !t.contains(' '))).map(s).collect::<Vec<_>>().join(" ");
            it.defer(src);
            Ok(Value::Void)
        }
        "source" => {
            need(1)?;
            let file = s(&a[0]);
            let Some(path) = resolve_source(&file) else { return err(format!("Cannot find file \"{file}\" for source statement.")) };
            let text = std::fs::read_to_string(&path).map_err(|e| Flow::Error(format!("{file}: {e}")))?;
            it.run_quiet(&text, Some(&path.to_string_lossy()))?;
            Ok(Value::Void)
        }
        "error" | "warning" => {
            // -noContext, -showLineNumber and -showLineNumber's value say how to word the message, which is plain here.
            let msg = a.iter().rev().find(|v| !matches!(v, Value::Str(t) if t.starts_with('-'))).or(a.last()).map(s).unwrap_or_default();
            if name == "error" {
                return err(msg);
            }
            it.flush_print();
            it.core.push(format!("// Warning: {msg} //"), LineKind::Warning);
            Ok(Value::Void)
        }
        "python" => {
            need(1)?;
            it.flush_print();
            let j = crate::core::python_from_mel(it.core, &s(&a[0]))?;
            Ok(Value::from_json(&j))
        }
        "whatIs" => {
            need(1)?;
            let w = s(&a[0]);
            if let Some(v) = w.strip_prefix('$') {
                return Ok(Value::Str(match it.var_type(v) {
                    Some(t) => format!("{} variable", t.name()),
                    None => "Unknown".into(),
                }));
            }
            if let Some(file) = it.proc_file(&w) {
                return Ok(Value::Str(match file {
                    Some(f) => format!("Mel procedure found in: {f}"),
                    None => "Mel procedure entered interactively.".into(),
                }));
            }
            if crate::core::UI_COMMANDS.iter().any(|(n, _)| *n == w) {
                return Ok(Value::Str("Run Time Command".into()));
            }
            if it.is_command(&w) || NAMES.contains(&w.as_str()) {
                return Ok(Value::Str("Command".into()));
            }
            Ok(Value::Str(match find_script(&w) {
                Some(p) => format!("Script found in: {}", p.display()),
                None => "Unknown".into(),
            }))
        }
        "exists" => {
            need(1)?;
            let w = s(&a[0]);
            Ok(Value::Int((it.is_proc(&w) || it.is_command(&w) || NAMES.contains(&w.as_str()) || find_script(&w).is_some()) as i32))
        }
        "about" => {
            let flag = a.iter().map(s).find(|f| f.starts_with('-')).unwrap_or_default();
            let on = |b: bool| Value::Int(b as i32);
            Ok(match flag.trim_start_matches('-') {
                "v" | "version" | "iv" | "installedVersion" | "f" | "file" => Value::Str("2025".into()),
                "api" | "apiVersion" => Value::Int(20250000),
                "os" | "operatingSystem" => Value::Str(if cfg!(windows) { "win64" } else if cfg!(target_os = "macos") { "mac" } else { "linux64" }.into()),
                "ov" | "operatingSystemVersion" => Value::Str(std::env::consts::OS.into()),
                "a" | "application" => Value::Str("aztek".into()),
                "p" | "product" => Value::Str("Aztek".into()),
                "b" | "batch" => on(aztek_core::batch_mode()),
                "li" | "linux" => on(cfg!(target_os = "linux")),
                "nt" | "windows" | "win" => on(cfg!(windows)),
                "mac" | "macOS" => on(cfg!(target_os = "macos")),
                "ui" | "uiLanguage" => Value::Str("en_US".into()),
                "" => return err("about: name what to ask about."),
                f => return err(format!("about: invalid flag -{f}")),
            })
        }
        "env" => {
            need(0)?;
            Ok(vs(it.global_names()))
        }
        "getenv" => {
            need(1)?;
            Ok(Value::Str(std::env::var(s(&a[0])).unwrap_or_default()))
        }
        "putenv" => {
            need(2)?;
            let k = s(&a[0]);
            if k.is_empty() || k.contains('=') || k.contains('\0') || s(&a[1]).contains('\0') {
                return err("putenv: invalid variable name or value.");
            }
            std::env::set_var(k, s(&a[1]));
            Ok(Value::Void)
        }
        "catch" | "catchQuiet" => Ok(Value::Int(0)),

        // Strings.
        "size" => {
            need(1)?;
            match &a[0] {
                Value::Int(_) | Value::Float(_) | Value::Vector(_) | Value::Matrix(..) | Value::Void => err("Wrong type of argument on call to size."),
                v => Ok(Value::Int(v.len() as i32)),
            }
        }
        "sizeBytes" => {
            need(1)?;
            Ok(Value::Int(s(&a[0]).len() as i32))
        }
        "substring" => {
            need(3)?;
            let t = s(&a[0]);
            let start = a[1].to_int().max(1) as usize;
            let end = a[2].to_int().max(0) as usize;
            Ok(Value::Str(chars_between(&t, start - 1, end)))
        }
        "tokenize" | "tokenizeList" => {
            if !(1..=3).contains(&n) {
                return Err(wrong(name));
            }
            // tokenize(text), tokenize(text, buffer), tokenize(text, separators, buffer)
            let t = s(&a[0]);
            let seps = if n == 3 { Some(s(&a[1])) } else if n == 2 && !a[1].is_array() { Some(s(&a[1])) } else { None };
            let parts = tokenize(&t, seps.as_deref());
            let count = parts.len() as i32;
            if name == "tokenizeList" {
                return Ok(vs(parts));
            }
            if n >= 2 {
                a[n - 1] = vs(parts);
            }
            Ok(Value::Int(count))
        }
        "match" => {
            need(2)?;
            let t = s(&a[1]);
            Ok(Value::Str(match regex_find(&s(&a[0]), &t).map_err(Flow::Error)? {
                Some((x, y)) => chars_between(&t, x, y),
                None => String::new(),
            }))
        }
        "gmatch" => {
            need(2)?;
            Ok(Value::Int(glob(&s(&a[1]), &s(&a[0])) as i32))
        }
        "substitute" => {
            need(3)?;
            let t = s(&a[1]);
            Ok(Value::Str(match regex_find(&s(&a[0]), &t).map_err(Flow::Error)? {
                Some((x, y)) => {
                    let mut o = chars_between(&t, 0, x);
                    o.push_str(&s(&a[2]));
                    o.extend(t.chars().skip(y));
                    o
                }
                None => t,
            }))
        }
        "substituteAllString" => {
            need(3)?;
            let (t, from, to) = (s(&a[0]), s(&a[1]), s(&a[2]));
            Ok(Value::Str(if from.is_empty() { t } else { t.replace(&from, &to) }))
        }
        "strip" => {
            need(1)?;
            Ok(Value::Str(s(&a[0]).trim().to_string()))
        }
        "toupper" => {
            need(1)?;
            Ok(Value::Str(s(&a[0]).to_uppercase()))
        }
        "tolower" => {
            need(1)?;
            Ok(Value::Str(s(&a[0]).to_lowercase()))
        }
        "startsWith" => {
            need(2)?;
            Ok(Value::Int(s(&a[0]).starts_with(&s(&a[1])) as i32))
        }
        "endsWith" => {
            need(2)?;
            Ok(Value::Int(s(&a[0]).ends_with(&s(&a[1])) as i32))
        }
        "strcmp" => {
            need(2)?;
            Ok(Value::Int(s(&a[0]).cmp(&s(&a[1])) as i32))
        }
        "stringArrayToString" | "intArrayToString" | "floatArrayToString" => {
            need(2)?;
            Ok(Value::Str(strs(&a[0]).join(&s(&a[1]))))
        }
        "stringToStringArray" => {
            need(2)?;
            Ok(vs(tokenize(&s(&a[0]), Some(&s(&a[1])))))
        }
        "stringArrayContains" => {
            need(2)?;
            let w = s(&a[0]);
            Ok(Value::Int(strs(&a[1]).contains(&w) as i32))
        }
        "stringArrayCount" => {
            need(2)?;
            let w = s(&a[0]);
            Ok(Value::Int(strs(&a[1]).iter().filter(|x| **x == w).count() as i32))
        }
        "stringArrayFind" => {
            need(3)?;
            let w = s(&a[0]);
            let start = a[1].to_int().max(0) as usize;
            Ok(Value::Int(strs(&a[2]).iter().skip(start).position(|x| *x == w).map(|p| (p + start) as i32).unwrap_or(-1)))
        }
        "stringArrayRemove" => {
            need(2)?;
            let gone = strs(&a[0]);
            Ok(vs(strs(&a[1]).into_iter().filter(|x| !gone.contains(x)).collect()))
        }
        "stringArrayRemoveExact" => {
            need(2)?;
            let mut list = strs(&a[1]);
            for g in strs(&a[0]) {
                if let Some(p) = list.iter().position(|x| *x == g) {
                    list.remove(p);
                }
            }
            Ok(vs(list))
        }
        "stringArrayRemoveDuplicates" => {
            need(1)?;
            let mut seen = std::collections::HashSet::new();
            Ok(vs(strs(&a[0]).into_iter().filter(|x| seen.insert(x.clone())).collect()))
        }
        "stringArrayCatenate" => {
            need(2)?;
            let mut o = strs(&a[0]);
            o.extend(strs(&a[1]));
            Ok(vs(o))
        }
        "stringArrayReverse" => {
            need(1)?;
            let mut o = strs(&a[0]);
            o.reverse();
            Ok(vs(o))
        }
        "stringArrayInsertAtIndex" => {
            need(3)?;
            let at = a[0].to_int();
            let mut list = strs(&a[1]);
            if at < 0 {
                return Ok(Value::Int(0));
            }
            list.insert((at as usize).min(list.len()), s(&a[2]));
            a[1] = vs(list);
            Ok(Value::Int(1))
        }
        "intArrayInsertAtIndex" => {
            need(3)?;
            let at = a[0].to_int();
            let Value::IntArr(mut list) = a[1].coerce(Ty::IntArr)? else { return Ok(Value::Int(0)) };
            if at < 0 {
                return Ok(Value::Int(0));
            }
            list.insert((at as usize).min(list.len()), a[2].to_int());
            a[1] = Value::IntArr(list);
            Ok(Value::Int(1))
        }
        "floatArrayInsertAtIndex" => {
            need(3)?;
            let at = a[0].to_int();
            let Value::FloatArr(mut list) = a[1].coerce(Ty::FloatArr)? else { return Ok(Value::Int(0)) };
            if at < 0 {
                return Ok(Value::Int(0));
            }
            list.insert((at as usize).min(list.len()), a[2].to_float());
            a[1] = Value::FloatArr(list);
            Ok(Value::Int(1))
        }
        "startString" => {
            need(2)?;
            let t = s(&a[0]);
            let k = a[1].to_int().max(0) as usize;
            Ok(Value::Str(t.chars().take(k).collect()))
        }
        "endString" => {
            need(2)?;
            let t = s(&a[0]);
            let k = a[1].to_int().max(0) as usize;
            let len = t.chars().count();
            Ok(Value::Str(t.chars().skip(len.saturating_sub(k)).collect()))
        }
        "capitalizeString" => {
            need(1)?;
            let t = s(&a[0]);
            let mut c = t.chars();
            Ok(Value::Str(match c.next() {
                Some(f) => f.to_uppercase().chain(c).collect(),
                None => String::new(),
            }))
        }
        "shortNameOf" => {
            need(1)?;
            let t = s(&a[0]);
            Ok(Value::Str(t.rsplit('|').next().unwrap_or(&t).to_string()))
        }
        "rootOf" => {
            need(1)?;
            let t = s(&a[0]);
            let t = t.split('.').next().unwrap_or("");
            Ok(Value::Str(t.split('|').find(|p| !p.is_empty()).unwrap_or("").to_string()))
        }
        "longNameOf" => {
            need(1)?;
            let t = s(&a[0]);
            let g = &it.core.s.graph;
            Ok(Value::Str(match g.find(&t) {
                Ok(Some(id)) => g.full_path(id),
                _ => String::new(),
            }))
        }
        "basename" => {
            need(2)?;
            let t = s(&a[0]);
            let b = t.rsplit(['/', '\\']).next().unwrap_or(&t).to_string();
            let suffix = s(&a[1]);
            Ok(Value::Str(if !suffix.is_empty() && b.ends_with(&suffix) { b[..b.len() - suffix.len()].to_string() } else { b }))
        }
        "dirname" => {
            need(1)?;
            let t = s(&a[0]);
            Ok(Value::Str(match t.rfind(['/', '\\']) {
                Some(p) => t[..p].to_string(),
                None => String::new(),
            }))
        }
        "fromNativePath" => {
            need(1)?;
            Ok(Value::Str(s(&a[0]).replace('\\', "/")))
        }
        "toNativePath" => {
            need(1)?;
            let t = s(&a[0]);
            Ok(Value::Str(if cfg!(windows) { t.replace('/', "\\") } else { t }))
        }
        "encodeString" => {
            need(1)?;
            Ok(Value::Str(s(&a[0]).replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n").replace('\t', "\\t")))
        }
        "isValidString" => {
            need(2)?;
            let t = s(&a[0]);
            let whole = match regex_find(&s(&a[1]), &t).map_err(Flow::Error)? {
                Some((0, e)) => e == t.chars().count(),
                _ => false,
            };
            Ok(Value::Int(whole as i32))
        }
        "isValidObjectName" => {
            need(1)?;
            let t = s(&a[0]);
            let mut c = t.chars();
            let ok = c.next().is_some_and(|f| f.is_ascii_alphabetic() || f == '_') && c.all(|x| x.is_ascii_alphanumeric() || x == '_');
            Ok(Value::Int(ok as i32))
        }
        "formValidObjectName" => {
            need(1)?;
            let t: String = s(&a[0]).chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' }).collect();
            Ok(Value::Str(if t.starts_with(|c: char| c.is_ascii_digit()) { format!("_{t}") } else { t }))
        }

        // Math with more than one float.
        "abs" => {
            need(1)?;
            Ok(match &a[0] {
                Value::Int(i) => Value::Int(i.wrapping_abs()),
                Value::Vector(v) => Value::Vector([v[0].abs(), v[1].abs(), v[2].abs()]),
                v => Value::Float(v.to_float().abs()),
            })
        }
        "atan2" | "atan2d" | "pow" | "fmod" | "hypot" => {
            need(2)?;
            let (x, y) = (a[0].to_float(), a[1].to_float());
            Ok(Value::Float(match name {
                "atan2" => x.atan2(y),
                "atan2d" => x.atan2(y).to_degrees(),
                "pow" => x.powf(y),
                "fmod" => {
                    if y == 0.0 {
                        return err("Attempt to divide by zero.");
                    }
                    x % y
                }
                _ => x.hypot(y),
            }))
        }
        "min" | "max" => {
            need(2)?;
            let pick_first = if name == "min" { a[0].to_float() <= a[1].to_float() } else { a[0].to_float() >= a[1].to_float() };
            if int_like(&a[0], &a[1]) {
                return Ok(if pick_first { a[0].clone() } else { a[1].clone() });
            }
            if let (Value::Vector(x), Value::Vector(y)) = (&a[0], &a[1]) {
                let f: fn(f64, f64) -> f64 = if name == "min" { f64::min } else { f64::max };
                return Ok(Value::Vector([f(x[0], y[0]), f(x[1], y[1]), f(x[2], y[2])]));
            }
            Ok(Value::Float(if pick_first { a[0].to_float() } else { a[1].to_float() }))
        }
        // clamp(min, max, value)
        "clamp" => {
            need(3)?;
            let (lo, hi, x) = (a[0].to_float(), a[1].to_float(), a[2].to_float());
            Ok(Value::Float(if x < lo {
                lo
            } else if x > hi {
                hi
            } else {
                x
            }))
        }
        "sign" => {
            need(1)?;
            Ok(match &a[0] {
                Value::Int(i) => Value::Int(i.signum()),
                v => {
                    let f = v.to_float();
                    Value::Float(if f > 0.0 {
                        1.0
                    } else if f < 0.0 {
                        -1.0
                    } else {
                        0.0
                    })
                }
            })
        }
        // rand(max), rand(min, max), rand(<<vector>>)
        "rand" => match n {
            1 => {
                if let Value::Vector(v) = &a[0] {
                    let v = *v;
                    return Ok(Value::Vector([next_rand(it) * v[0], next_rand(it) * v[1], next_rand(it) * v[2]]));
                }
                Ok(Value::Float(next_rand(it) * a[0].to_float()))
            }
            2 => {
                if let (Value::Vector(lo), Value::Vector(hi)) = (&a[0], &a[1]) {
                    let (lo, hi) = (*lo, *hi);
                    let mut o = [0.0; 3];
                    for i in 0..3 {
                        o[i] = lo[i] + next_rand(it) * (hi[i] - lo[i]);
                    }
                    return Ok(Value::Vector(o));
                }
                let (lo, hi) = (a[0].to_float(), a[1].to_float());
                Ok(Value::Float(lo + next_rand(it) * (hi - lo)))
            }
            _ => Err(wrong(name)),
        },
        "seed" => {
            need(1)?;
            // Spread the seed over the bits; xorshift can't start from zero.
            let k = (a[0].to_int() as u32 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
            it.core.mel_state.rng = k;
            next_rand(it);
            Ok(Value::Void)
        }
        "mag" => {
            need(1)?;
            Ok(Value::Float(v_mag(a[0].to_vector())))
        }
        "dot" => {
            need(2)?;
            Ok(Value::Float(v_dot(a[0].to_vector(), a[1].to_vector())))
        }
        "cross" => {
            need(2)?;
            let (x, y) = (a[0].to_vector(), a[1].to_vector());
            Ok(Value::Vector([x[1] * y[2] - x[2] * y[1], x[2] * y[0] - x[0] * y[2], x[0] * y[1] - x[1] * y[0]]))
        }
        "unit" => {
            need(1)?;
            let v = a[0].to_vector();
            let m = v_mag(v);
            Ok(Value::Vector(if m == 0.0 { [0.0; 3] } else { [v[0] / m, v[1] / m, v[2] / m] }))
        }
        "angle" => {
            need(2)?;
            let (x, y) = (a[0].to_vector(), a[1].to_vector());
            let m = v_mag(x) * v_mag(y);
            Ok(Value::Float(if m == 0.0 { 0.0 } else { (v_dot(x, y) / m).clamp(-1.0, 1.0).acos() }))
        }
        "linstep" => {
            need(3)?;
            Ok(Value::Float(step(a[0].to_float(), a[1].to_float(), a[2].to_float())))
        }
        "smoothstep" => {
            need(3)?;
            let t = step(a[0].to_float(), a[1].to_float(), a[2].to_float());
            Ok(Value::Float(t * t * (3.0 - 2.0 * t)))
        }

        // Arrays.
        "sort" => {
            need(1)?;
            Ok(match &a[0] {
                Value::IntArr(x) => {
                    let mut o = x.clone();
                    o.sort();
                    Value::IntArr(o)
                }
                Value::FloatArr(x) => {
                    let mut o = x.clone();
                    o.sort_by(f64::total_cmp);
                    Value::FloatArr(o)
                }
                Value::StrArr(x) => {
                    let mut o = x.clone();
                    o.sort();
                    Value::StrArr(o)
                }
                v => return err(format!("Cannot sort data of type {}.", v.type_name())),
            })
        }
        "clear" => {
            need(1)?;
            let cleared = a[0].ty().filter(|t| t.is_array()).map(Ty::default_value);
            let Some(c) = cleared else { return err("clear: give an array.") };
            let was = a[0].len();
            a[0] = c;
            Ok(Value::Int(was as i32))
        }
        "appendStringArray" => {
            need(3)?;
            let mut to = strs(&a[0]);
            let count = a[2].to_int().max(0) as usize;
            to.extend(strs(&a[1]).into_iter().take(count));
            a[0] = vs(to);
            Ok(Value::Void)
        }
        "intArrayFind" | "floatArrayFind" => {
            need(3)?;
            let start = a[1].to_int().max(0) as usize;
            let item = a[0].clone();
            Ok(Value::Int(a[2].elements().iter().skip(start).position(|x| same(x, &item)).map(|p| (p + start) as i32).unwrap_or(-1)))
        }
        "intArrayContains" | "floatArrayContains" => {
            need(2)?;
            let item = a[0].clone();
            Ok(Value::Int(a[1].elements().iter().any(|x| same(x, &item)) as i32))
        }
        "intArrayCount" | "floatArrayCount" => {
            need(2)?;
            let item = a[0].clone();
            Ok(Value::Int(a[1].elements().iter().filter(|x| same(x, &item)).count() as i32))
        }
        "intArrayRemove" | "floatArrayRemove" => {
            need(2)?;
            let gone = a[0].elements();
            let kept: Vec<Value> = a[1].elements().into_iter().filter(|x| !gone.iter().any(|g| same(x, g))).collect();
            Ok(if name == "intArrayRemove" { Value::IntArr(kept.iter().map(Value::to_int).collect()) } else { Value::FloatArr(kept.iter().map(Value::to_float).collect()) })
        }
        "intArrayRemoveDuplicates" | "floatArrayRemoveDuplicates" => {
            need(1)?;
            let mut kept: Vec<Value> = Vec::new();
            for x in a[0].elements() {
                if !kept.iter().any(|k| same(k, &x)) {
                    kept.push(x);
                }
            }
            Ok(if name == "intArrayRemoveDuplicates" { Value::IntArr(kept.iter().map(Value::to_int).collect()) } else { Value::FloatArr(kept.iter().map(Value::to_float).collect()) })
        }
        _ => err(format!("Cannot find procedure \"{name}\".")),
    }
}
