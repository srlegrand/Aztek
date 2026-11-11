//! MEL's functions that reach outside the scene: files (`fopen` and its family, `popen`), `system`, `date`, `timerX`,
//! `filetest`, and the small string, path and math procedures scripts kept in a user's folder lean on.

use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use super::interp::{err, Flow, Interp, R};
use super::value::Value;

/// Every name `run` answers.
pub const NAMES: &[&str] = &[
    "fopen", "fclose", "fprint", "fwrite", "fread", "fgetline", "fgetword", "feof", "fflush", "frewind", "popen", "pclose", "system", "date", "timerX", "filetest", "fileExtension",
    "basenameEx", "trace", "format", "rehash", "gauss", "sphrand", "noise", "hermite", "rot", "stringArrayRemoveAtIndex", "stringAddPrefix", "stringRemovePrefix", "attributeExists",
];

/// An open file or pipe. Read handles hold the whole content and a position, which is all `fgetline`, `fgetword` and
/// `feof` need; a pipe opened for writing gathers what is printed to it and runs its command when closed.
enum Handle {
    Read { data: Vec<u8>, at: usize },
    Write(std::fs::File),
    PipeIn { command: String, data: Vec<u8> },
}

struct Files {
    open: HashMap<i32, Handle>,
    next: i32,
}

thread_local! {
    static FILES: RefCell<Files> = RefCell::new(Files { open: HashMap::new(), next: 1 });
    static STARTED: std::time::Instant = std::time::Instant::now();
}

fn add(h: Handle) -> i32 {
    FILES.with(|f| {
        let mut f = f.borrow_mut();
        let id = f.next;
        f.next += 1;
        f.open.insert(id, h);
        id
    })
}

fn wrong(name: &str) -> Flow {
    Flow::Error(format!("Wrong number of arguments on call to {name}."))
}

fn shell(command: &str) -> Command {
    if cfg!(windows) {
        let mut c = Command::new("cmd");
        c.args(["/C", command]);
        c
    } else {
        let mut c = Command::new("sh");
        c.args(["-c", command]);
        c
    }
}

/// What a command prints, as text.
fn run_shell(command: &str) -> String {
    match shell(command).stdin(Stdio::null()).stderr(Stdio::inherit()).output() {
        Ok(o) => String::from_utf8_lossy(&o.stdout).into_owned(),
        Err(_) => String::new(),
    }
}

/// The local date and time: year, month, day, hour, minute, second.
fn now() -> [i64; 6] {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    #[cfg(unix)]
    {
        let t = secs as libc::time_t;
        let mut tm: libc::tm = unsafe { std::mem::zeroed() };
        if !unsafe { libc::localtime_r(&t, &mut tm) }.is_null() {
            return [tm.tm_year as i64 + 1900, tm.tm_mon as i64 + 1, tm.tm_mday as i64, tm.tm_hour as i64, tm.tm_min as i64, tm.tm_sec as i64];
        }
    }
    // Without the system's time zone: UTC.
    let (days, rem) = (secs.div_euclid(86400), secs.rem_euclid(86400));
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let (d, m) = (doy - (153 * mp + 2) / 5 + 1, if mp < 10 { mp + 3 } else { mp - 9 });
    [yoe + era * 400 + (m <= 2) as i64, m, d, rem / 3600, rem % 3600 / 60, rem % 60]
}

/// `date -format`: YYYY, YY, MM, DD, hh, mm and ss are filled in, the rest is kept.
fn format_date(pattern: &str, t: [i64; 6]) -> String {
    let mut out = String::new();
    let mut rest = pattern;
    let fields: [(&str, String); 7] =
        [("YYYY", format!("{:04}", t[0])), ("YY", format!("{:02}", t[0] % 100)), ("MM", format!("{:02}", t[1])), ("DD", format!("{:02}", t[2])), ("hh", format!("{:02}", t[3])), ("mm", format!("{:02}", t[4])), ("ss", format!("{:02}", t[5]))];
    'next: while !rest.is_empty() {
        for (key, value) in &fields {
            if let Some(r) = rest.strip_prefix(key) {
                out.push_str(value);
                rest = r;
                continue 'next;
            }
        }
        let c = rest.chars().next().unwrap();
        out.push(c);
        rest = &rest[c.len_utf8()..];
    }
    out
}

fn uniform(it: &mut Interp) -> f64 {
    let st = &mut it.core.mel_state;
    let mut x = st.rng;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    st.rng = x;
    (x >> 11) as f64 / (1u64 << 53) as f64
}

/// A normally distributed number, mean 0 and deviation 1.
fn normal(it: &mut Interp) -> f64 {
    let (u, v) = (uniform(it).max(1e-300), uniform(it));
    (-2.0 * u.ln()).sqrt() * (std::f64::consts::TAU * v).cos()
}

fn lattice(x: i64, y: i64, z: i64) -> [f64; 3] {
    let mut h = (x as u64).wrapping_mul(0x9E3779B97F4A7C15) ^ (y as u64).wrapping_mul(0xC2B2AE3D27D4EB4F) ^ (z as u64).wrapping_mul(0x165667B19E3779F9);
    h ^= h >> 29;
    h = h.wrapping_mul(0xBF58476D1CE4E5B9);
    h ^= h >> 32;
    const G: [[f64; 3]; 12] =
        [[1.0, 1.0, 0.0], [-1.0, 1.0, 0.0], [1.0, -1.0, 0.0], [-1.0, -1.0, 0.0], [1.0, 0.0, 1.0], [-1.0, 0.0, 1.0], [1.0, 0.0, -1.0], [-1.0, 0.0, -1.0], [0.0, 1.0, 1.0], [0.0, -1.0, 1.0], [0.0, 1.0, -1.0], [0.0, -1.0, -1.0]];
    G[(h % 12) as usize]
}

/// Gradient noise between -1 and 1, zero at whole numbers.
fn noise3(p: [f64; 3]) -> f64 {
    let f = [p[0].floor(), p[1].floor(), p[2].floor()];
    let (i, d) = ([f[0] as i64, f[1] as i64, f[2] as i64], [p[0] - f[0], p[1] - f[1], p[2] - f[2]]);
    let fade = |t: f64| t * t * t * (t * (t * 6.0 - 15.0) + 10.0);
    let w = [fade(d[0]), fade(d[1]), fade(d[2])];
    let corner = |a: i64, b: i64, c: i64| {
        let g = lattice(i[0] + a, i[1] + b, i[2] + c);
        g[0] * (d[0] - a as f64) + g[1] * (d[1] - b as f64) + g[2] * (d[2] - c as f64)
    };
    let mix = |a: f64, b: f64, t: f64| a + (b - a) * t;
    let x = |b: i64, c: i64| mix(corner(0, b, c), corner(1, b, c), w[0]);
    mix(mix(x(0, 0), x(1, 0), w[1]), mix(x(0, 1), x(1, 1), w[1]), w[2])
}

fn vec3(v: &Value) -> Option<[f64; 3]> {
    match v {
        Value::Vector(a) => Some(*a),
        _ => None,
    }
}

/// A flag as a command-form call passes it: a word starting with a dash.
fn flag(v: &Value) -> Option<&str> {
    match v {
        Value::Str(t) if t.starts_with('-') && t.len() > 1 => Some(t.as_str()),
        _ => None,
    }
}

fn with_handle<T>(id: i32, f: impl FnOnce(&mut Handle) -> T) -> Option<T> {
    FILES.with(|files| files.borrow_mut().open.get_mut(&id).map(f))
}

fn write_to(id: i32, bytes: &[u8]) {
    with_handle(id, |h| match h {
        Handle::Write(f) => {
            let _ = f.write_all(bytes);
        }
        Handle::PipeIn { data, .. } => data.extend_from_slice(bytes),
        Handle::Read { .. } => {}
    });
}

pub fn run(it: &mut Interp, name: &str, a: &mut Vec<Value>) -> R<Value> {
    let n = a.len();
    let text = |i: usize| a.get(i).map(Value::to_text).unwrap_or_default();
    let id = |i: usize| a.get(i).map(Value::to_int).unwrap_or(0);
    match name {
        "fopen" => {
            if n == 0 || n > 2 {
                return Err(wrong(name));
            }
            let (path, mode) = (text(0), if n == 2 { text(1) } else { "w".into() });
            let opened = if mode.starts_with('r') && !mode.contains('+') {
                std::fs::read(&path).ok().map(|data| Handle::Read { data, at: 0 })
            } else {
                let mut o = std::fs::OpenOptions::new();
                if mode.starts_with('a') {
                    o.append(true).create(true);
                } else if mode.starts_with('r') {
                    o.write(true);
                } else {
                    o.write(true).create(true).truncate(true);
                }
                o.open(&path).ok().map(Handle::Write)
            };
            // 0 says the file could not be opened, which scripts test for.
            Ok(Value::Int(opened.map(add).unwrap_or(0)))
        }
        "popen" => {
            if n != 2 {
                return Err(wrong(name));
            }
            let (command, mode) = (text(0), text(1));
            Ok(Value::Int(add(if mode.starts_with('w') { Handle::PipeIn { command, data: Vec::new() } } else { Handle::Read { data: run_shell(&command).into_bytes(), at: 0 } })))
        }
        "fclose" | "pclose" => {
            if n != 1 {
                return Err(wrong(name));
            }
            if let Some(Handle::PipeIn { command, data }) = FILES.with(|f| f.borrow_mut().open.remove(&id(0))) {
                if let Ok(mut child) = shell(&command).stdin(Stdio::piped()).spawn() {
                    if let Some(mut stdin) = child.stdin.take() {
                        let _ = stdin.write_all(&data);
                    }
                    let _ = child.wait();
                }
            }
            Ok(Value::Void)
        }
        "fflush" => {
            with_handle(id(0), |h| {
                if let Handle::Write(f) = h {
                    let _ = f.flush();
                }
            });
            Ok(Value::Void)
        }
        "frewind" => {
            with_handle(id(0), |h| match h {
                Handle::Read { at, .. } => *at = 0,
                Handle::Write(f) => {
                    use std::io::Seek;
                    let _ = f.rewind();
                }
                Handle::PipeIn { .. } => {}
            });
            Ok(Value::Void)
        }
        "fprint" => {
            if n != 2 {
                return Err(wrong(name));
            }
            // Arrays go out an element after another with a space between, as print's line each would not fit a file.
            let out = if a[1].is_array() { a[1].elements().iter().map(Value::to_text).collect::<Vec<_>>().join(" ") } else { a[1].to_text() };
            write_to(id(0), out.as_bytes());
            Ok(Value::Void)
        }
        "fwrite" => {
            if n != 2 {
                return Err(wrong(name));
            }
            // Binary: a string with the zero that ends it, an int in four bytes, a float in four.
            let mut bytes = Vec::new();
            for v in if a[1].is_array() { a[1].elements() } else { vec![a[1].clone()] } {
                match v {
                    Value::Int(i) => bytes.extend_from_slice(&i.to_ne_bytes()),
                    Value::Float(f) => bytes.extend_from_slice(&(f as f32).to_ne_bytes()),
                    Value::Vector(x) => x.iter().for_each(|f| bytes.extend_from_slice(&(*f as f32).to_ne_bytes())),
                    other => {
                        bytes.extend_from_slice(other.to_text().as_bytes());
                        bytes.push(0);
                    }
                }
            }
            write_to(id(0), &bytes);
            Ok(Value::Void)
        }
        "fread" => {
            if n != 2 {
                return Err(wrong(name));
            }
            let like = a[1].clone();
            let got = with_handle(id(0), |h| {
                let Handle::Read { data, at } = h else { return None };
                let rest = &data[(*at).min(data.len())..];
                Some(match like {
                    Value::Int(_) => {
                        let mut b = [0u8; 4];
                        let k = rest.len().min(4);
                        b[..k].copy_from_slice(&rest[..k]);
                        *at += k;
                        Value::Int(i32::from_ne_bytes(b))
                    }
                    Value::Float(_) => {
                        let mut b = [0u8; 4];
                        let k = rest.len().min(4);
                        b[..k].copy_from_slice(&rest[..k]);
                        *at += k;
                        Value::Float(f32::from_ne_bytes(b) as f64)
                    }
                    _ => {
                        // A string runs to the zero that ends it, or to the end of the file.
                        let end = rest.iter().position(|&c| c == 0).unwrap_or(rest.len());
                        let s = String::from_utf8_lossy(&rest[..end]).into_owned();
                        *at += (end + 1).min(rest.len());
                        Value::Str(s)
                    }
                })
            });
            Ok(got.flatten().unwrap_or(Value::Str(String::new())))
        }
        "fgetline" => {
            if n != 1 {
                return Err(wrong(name));
            }
            let line = with_handle(id(0), |h| {
                let Handle::Read { data, at } = h else { return String::new() };
                let rest = &data[(*at).min(data.len())..];
                let end = rest.iter().position(|&c| c == b'\n').map(|p| p + 1).unwrap_or(rest.len());
                *at += end;
                String::from_utf8_lossy(&rest[..end]).into_owned()
            });
            Ok(Value::Str(line.unwrap_or_default()))
        }
        "fgetword" => {
            if n == 0 || n > 2 {
                return Err(wrong(name));
            }
            let seps: Vec<u8> = if n == 2 { text(1).into_bytes() } else { b" \t\r\n".to_vec() };
            let word = with_handle(id(0), |h| {
                let Handle::Read { data, at } = h else { return String::new() };
                while *at < data.len() && seps.contains(&data[*at]) {
                    *at += 1;
                }
                let start = *at;
                while *at < data.len() && !seps.contains(&data[*at]) {
                    *at += 1;
                }
                String::from_utf8_lossy(&data[start..*at]).into_owned()
            });
            Ok(Value::Str(word.unwrap_or_default()))
        }
        "feof" => {
            if n != 1 {
                return Err(wrong(name));
            }
            let end = with_handle(id(0), |h| match h {
                Handle::Read { data, at } => *at >= data.len(),
                _ => false,
            });
            Ok(Value::Int(end.unwrap_or(true) as i32))
        }
        "system" => {
            if n != 1 {
                return Err(wrong(name));
            }
            let command = text(0);
            // A command sent to the background is started and left: nothing of its output is waited for.
            let trimmed = command.trim_end();
            if !cfg!(windows) && trimmed.ends_with('&') && !trimmed.ends_with("&&") {
                let _ = shell(trimmed).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn();
                return Ok(Value::Str(String::new()));
            }
            if cfg!(windows) && (trimmed.starts_with("start ") || trimmed.starts_with("shell ")) {
                let _ = shell(trimmed.strip_prefix("shell ").unwrap_or(trimmed)).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn();
                return Ok(Value::Str(String::new()));
            }
            Ok(Value::Str(run_shell(&command)))
        }
        "date" => {
            let t = now();
            let mut i = 0;
            let mut out = format!("{:04}/{:02}/{:02} {:02}:{:02}:{:02}", t[0], t[1], t[2], t[3], t[4], t[5]);
            while i < n {
                match flag(&a[i]) {
                    Some("-format" | "-f") if i + 1 < n => {
                        out = format_date(&text(i + 1), t);
                        i += 1;
                    }
                    Some("-date" | "-d") => out = format!("{:04}/{:02}/{:02}", t[0], t[1], t[2]),
                    Some("-time" | "-t") => out = format!("{:02}:{:02}:{:02}", t[3], t[4], t[5]),
                    Some("-shortDate" | "-sd") => out = format!("{:02}/{:02}", t[1], t[2]),
                    Some("-shortTime" | "-st") => out = format!("{:02}:{:02}", t[3], t[4]),
                    _ => {}
                }
                i += 1;
            }
            Ok(Value::Str(out))
        }
        "timerX" => {
            let elapsed = STARTED.with(|s| s.elapsed().as_secs_f64());
            match (a.first().and_then(flag), n) {
                (Some("-startTime" | "-st"), 2) => Ok(Value::Float(elapsed - a[1].to_float())),
                (None, 0) => Ok(Value::Float(elapsed)),
                _ => Err(wrong(name)),
            }
        }
        "filetest" => {
            if n != 2 {
                return Err(wrong(name));
            }
            let (test, path) = (text(0), text(1));
            let p = Path::new(&path);
            let meta = std::fs::metadata(p).ok();
            #[cfg(unix)]
            let access = |mode: libc::c_int| std::ffi::CString::new(path.as_bytes()).map(|c| unsafe { libc::access(c.as_ptr(), mode) } == 0).unwrap_or(false);
            #[cfg(unix)]
            let (readable, writable, runnable) = (access(libc::R_OK), access(libc::W_OK), access(libc::X_OK));
            #[cfg(not(unix))]
            let (readable, writable, runnable) = (meta.is_some(), meta.as_ref().is_some_and(|m| !m.permissions().readonly()), meta.is_some());
            let yes = match test.as_str() {
                "-e" => meta.is_some(),
                "-f" => meta.as_ref().is_some_and(|m| m.is_file()),
                "-d" => meta.as_ref().is_some_and(|m| m.is_dir()),
                "-r" | "-R" => readable,
                "-w" | "-W" => writable,
                "-x" | "-X" => runnable,
                "-s" => meta.as_ref().is_some_and(|m| m.len() > 0),
                "-z" => meta.as_ref().is_some_and(|m| m.len() == 0),
                "-l" | "-h" | "-L" => std::fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_symlink()),
                other => return err(format!("filetest: {other} is not a test this build has (-e -f -d -r -w -x -s -z -l).")),
            };
            Ok(Value::Int(yes as i32))
        }
        "fileExtension" => {
            if n != 1 {
                return Err(wrong(name));
            }
            let t = text(0);
            let base = t.rsplit(['/', '\\']).next().unwrap_or(&t);
            Ok(Value::Str(base.rfind('.').map(|p| base[p + 1..].to_string()).unwrap_or_default()))
        }
        "basenameEx" => {
            if n != 1 {
                return Err(wrong(name));
            }
            let t = text(0);
            let base = t.rsplit(['/', '\\']).next().unwrap_or(&t);
            Ok(Value::Str(base.rfind('.').map(|p| base[..p].to_string()).unwrap_or(base.to_string())))
        }
        "trace" => {
            // To the terminal the application was started from, where the reference's Output Window is.
            let msg = a.iter().filter(|v| flag(v) != Some("-where") && flag(v) != Some("-w")).map(Value::to_text).collect::<Vec<_>>().join(" ");
            eprintln!("{msg}");
            Ok(Value::Void)
        }
        "format" => {
            let mut args: Vec<String> = Vec::new();
            let mut pattern = String::new();
            let mut i = 0;
            while i < n {
                match flag(&a[i]) {
                    Some("-stringArg" | "-s") if i + 1 < n => {
                        args.push(text(i + 1));
                        i += 1;
                    }
                    _ => pattern = text(i),
                }
                i += 1;
            }
            // ^1s is the first argument, ^2s the second.
            let chars: Vec<char> = pattern.chars().collect();
            let mut out = String::new();
            let mut k = 0;
            while k < chars.len() {
                if chars[k] == '^' {
                    let mut j = k + 1;
                    while j < chars.len() && chars[j].is_ascii_digit() {
                        j += 1;
                    }
                    if j > k + 1 && chars.get(j) == Some(&'s') {
                        let index: usize = chars[k + 1..j].iter().collect::<String>().parse().unwrap_or(0);
                        if let Some(v) = index.checked_sub(1).and_then(|x| args.get(x)) {
                            out.push_str(v);
                        }
                        k = j + 1;
                        continue;
                    }
                }
                out.push(chars[k]);
                k += 1;
            }
            Ok(Value::Str(out))
        }
        // Scripts are looked for on the path each time one is wanted, so there is no table to rebuild.
        "rehash" => Ok(Value::Void),
        "gauss" => match n {
            1 => match vec3(&a[0]) {
                Some(v) => Ok(Value::Vector([normal(it) * v[0], normal(it) * v[1], normal(it) * v[2]])),
                None => Ok(Value::Float(normal(it) * a[0].to_float())),
            },
            2 => Ok(Value::Vector([normal(it) * a[0].to_float(), normal(it) * a[1].to_float(), 0.0])),
            3 => Ok(Value::Vector([normal(it) * a[0].to_float(), normal(it) * a[1].to_float(), normal(it) * a[2].to_float()])),
            _ => Err(wrong(name)),
        },
        "sphrand" => {
            if n != 1 {
                return Err(wrong(name));
            }
            let r = vec3(&a[0]).unwrap_or([a[0].to_float(); 3]);
            // A point anywhere in the unit ball, then stretched.
            loop {
                let p = [uniform(it) * 2.0 - 1.0, uniform(it) * 2.0 - 1.0, uniform(it) * 2.0 - 1.0];
                if p[0] * p[0] + p[1] * p[1] + p[2] * p[2] <= 1.0 {
                    return Ok(Value::Vector([p[0] * r[0], p[1] * r[1], p[2] * r[2]]));
                }
            }
        }
        "noise" => match n {
            1 => Ok(Value::Float(match vec3(&a[0]) {
                Some(v) => noise3(v),
                None => noise3([a[0].to_float(), 0.0, 0.0]),
            })),
            2 => Ok(Value::Float(noise3([a[0].to_float(), a[1].to_float(), 0.0]))),
            _ => Err(wrong(name)),
        },
        "hermite" => {
            if n != 5 {
                return Err(wrong(name));
            }
            let t = a[4].to_float();
            let (t2, t3) = (t * t, t * t * t);
            let h = [2.0 * t3 - 3.0 * t2 + 1.0, -2.0 * t3 + 3.0 * t2, t3 - 2.0 * t2 + t, t3 - t2];
            match (vec3(&a[0]), vec3(&a[1]), vec3(&a[2]), vec3(&a[3])) {
                (Some(p0), Some(p1), Some(m0), Some(m1)) => {
                    let mut o = [0.0; 3];
                    for (i, o) in o.iter_mut().enumerate() {
                        *o = h[0] * p0[i] + h[1] * p1[i] + h[2] * m0[i] + h[3] * m1[i];
                    }
                    Ok(Value::Vector(o))
                }
                _ => Ok(Value::Float(h[0] * a[0].to_float() + h[1] * a[1].to_float() + h[2] * a[2].to_float() + h[3] * a[3].to_float())),
            }
        }
        "rot" => {
            if n != 3 {
                return Err(wrong(name));
            }
            let (Some(p), Some(axis)) = (vec3(&a[0]), vec3(&a[1])) else { return err("Wrong type of argument on call to rot.") };
            let (p, k, angle) = (glam::DVec3::from_array(p), glam::DVec3::from_array(axis).normalize_or_zero(), a[2].to_float());
            let r = p * angle.cos() + k.cross(p) * angle.sin() + k * k.dot(p) * (1.0 - angle.cos());
            Ok(Value::Vector(r.to_array()))
        }
        "stringArrayRemoveAtIndex" => {
            if n != 2 {
                return Err(wrong(name));
            }
            let at = a[0].to_int();
            let mut list: Vec<String> = a[1].elements().iter().map(Value::to_text).collect();
            if at < 0 || at as usize >= list.len() {
                return Ok(Value::Int(0));
            }
            list.remove(at as usize);
            a[1] = Value::StrArr(list);
            Ok(Value::Int(1))
        }
        "stringAddPrefix" => {
            if n != 2 {
                return Err(wrong(name));
            }
            let prefix = text(1);
            Ok(Value::StrArr(a[0].elements().iter().map(|v| format!("{prefix}{}", v.to_text())).collect()))
        }
        "stringRemovePrefix" => {
            if n != 2 {
                return Err(wrong(name));
            }
            let (t, prefix) = (text(0), text(1));
            Ok(Value::Str(t.strip_prefix(prefix.as_str()).unwrap_or(&t).to_string()))
        }
        "attributeExists" => {
            if n != 2 {
                return Err(wrong(name));
            }
            let quote = |t: String| t.replace('\\', "\\\\").replace('"', "\\\"");
            let (attr, node) = (quote(text(0)), quote(text(1)));
            if attr.is_empty() || node.is_empty() {
                return Ok(Value::Int(0));
            }
            // No node of that name is an answer, not an error.
            let there = it.run_quiet(&format!("objExists \"{node}\""), None).is_ok_and(|v| v.to_int() != 0);
            let has = there && it.run_quiet(&format!("attributeQuery -node \"{node}\" -exists \"{attr}\""), None).is_ok_and(|v| v.to_int() != 0);
            Ok(Value::Int(has as i32))
        }
        _ => err(format!("Cannot find procedure \"{name}\".")),
    }
}
