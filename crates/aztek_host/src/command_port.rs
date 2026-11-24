//! The reference's commandPort: a socket that clients connect to and write commands into. Each port listens on its own
//! thread; what a client writes is queued and run on the core's idle step (the commands need the session), and the
//! answer goes back on the same connection.
//!
//! Names as the reference takes them: a plain name is a UNIX socket in /tmp, a name starting with "/" a socket at that
//! path, ":port" a local INET socket on that port, and "host:port" one on that host.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use aztek_ops::{fail, ArgType, Args, CmdResult, CommandDef, FlagDef};
use aztek_scene::Session;
use serde_json::{json, Value};

use crate::core::{Core, LineKind};

/// The name a port takes when none is given.
pub const DEFAULT_NAME: &str = "mayaCommand";
const DEFAULT_BUFFER: usize = 4096;

/// How a port treats what it is sent, fixed when it opens.
#[derive(Clone, Debug)]
struct Settings {
    buffer: usize,
    echo: bool,
    noreturn: bool,
    pickle: bool,
    prefix: Option<String>,
    count: bool,
    warn: bool,
    python: bool,
    /// The thread of the core that opened the port: its idle steps run what clients write.
    owner: std::thread::ThreadId,
}

struct Port {
    name: String,
    stop: Arc<AtomicBool>,
    /// The listening thread, waited for on close so the socket is gone when -close returns.
    thread: Option<std::thread::JoinHandle<()>>,
    socket_file: Option<std::path::PathBuf>,
}

/// One write from a client, waiting for the core.
struct Request {
    port: String,
    text: String,
    settings: Settings,
    /// The first write on its connection: -securityWarning speaks then.
    first: bool,
    reply: mpsc::Sender<Vec<u8>>,
}

static PORTS: Mutex<Vec<Port>> = Mutex::new(Vec::new());
static QUEUE: Mutex<Vec<Request>> = Mutex::new(Vec::new());

/// The names of the open ports, in the order they were opened.
pub fn open_ports() -> Vec<String> {
    PORTS.lock().map(|p| p.iter().map(|x| x.name.clone()).collect()).unwrap_or_default()
}

fn is_open(name: &str) -> bool {
    PORTS.lock().map(|p| p.iter().any(|x| x.name == name)).unwrap_or(false)
}

/// Where a name listens: an INET address, or a socket file.
enum Place {
    Inet(String),
    #[cfg_attr(not(unix), allow(dead_code))]
    File(std::path::PathBuf),
}

fn place_of(name: &str) -> Place {
    if let Some(port) = name.strip_prefix(':') {
        return Place::Inet(format!("127.0.0.1:{port}"));
    }
    if name.starts_with('/') {
        return Place::File(name.into());
    }
    // host:port, with a port that is a number.
    if let Some((host, port)) = name.rsplit_once(':') {
        if !host.is_empty() && !port.is_empty() && port.chars().all(|c| c.is_ascii_digit()) {
            return Place::Inet(format!("{host}:{port}"));
        }
    }
    Place::File(std::env::temp_dir().join(name))
}

fn open(name: &str, settings: Settings) -> Result<(), String> {
    if is_open(name) {
        return Err(format!("commandPort: Could not open command port {name}: it is already open."));
    }
    let stop = Arc::new(AtomicBool::new(false));
    let mut socket_file = None;
    let thread;
    match place_of(name) {
        Place::Inet(addr) => {
            let l = TcpListener::bind(&addr).map_err(|e| format!("commandPort: Could not open command port {name}: {e}"))?;
            l.set_nonblocking(true).map_err(|e| e.to_string())?;
            let (stop2, port) = (stop.clone(), name.to_string());
            thread = std::thread::spawn(move || {
                while !stop2.load(Ordering::Relaxed) {
                    match l.accept() {
                        Ok((stream, _)) => {
                            let _ = stream.set_nonblocking(false);
                            let (port, settings) = (port.clone(), settings.clone());
                            std::thread::spawn(move || serve_tcp(stream, port, settings));
                        }
                        Err(_) => std::thread::sleep(Duration::from_millis(15)),
                    }
                }
            });
        }
        Place::File(path) => {
            #[cfg(unix)]
            {
                use std::os::unix::net::UnixListener;
                // A socket file left behind by an earlier run would stop the bind.
                if path.exists() && std::os::unix::net::UnixStream::connect(&path).is_err() {
                    let _ = std::fs::remove_file(&path);
                }
                let l = UnixListener::bind(&path).map_err(|e| format!("commandPort: Could not open command port {name}: {e}"))?;
                l.set_nonblocking(true).map_err(|e| e.to_string())?;
                let (stop2, port) = (stop.clone(), name.to_string());
                thread = std::thread::spawn(move || {
                    while !stop2.load(Ordering::Relaxed) {
                        match l.accept() {
                            Ok((stream, _)) => {
                                let _ = stream.set_nonblocking(false);
                                let (port, settings) = (port.clone(), settings.clone());
                                std::thread::spawn(move || serve_stream(stream, port, settings));
                            }
                            Err(_) => std::thread::sleep(Duration::from_millis(15)),
                        }
                    }
                });
                socket_file = Some(path);
            }
            #[cfg(not(unix))]
            {
                let _ = path;
                return Err(format!("commandPort: Could not open command port {name}: named sockets need a UNIX system here; use \":port\"."));
            }
        }
    }
    PORTS.lock().map_err(|e| e.to_string())?.push(Port { name: name.to_string(), stop, thread: Some(thread), socket_file });
    Ok(())
}

/// Closes the port and deletes its socket file. Connections already made stay open.
fn close(name: &str) -> Result<(), String> {
    let mut ports = PORTS.lock().map_err(|e| e.to_string())?;
    let Some(i) = ports.iter().position(|p| p.name == name) else { return Err(format!("commandPort: No command port named {name} is open.")) };
    let mut p = ports.remove(i);
    drop(ports);
    p.stop.store(true, Ordering::Relaxed);
    if let Some(t) = p.thread.take() {
        let _ = t.join();
    }
    if let Some(f) = p.socket_file {
        let _ = std::fs::remove_file(f);
    }
    Ok(())
}

fn serve_tcp(stream: TcpStream, port: String, settings: Settings) {
    serve_stream(stream, port, settings)
}

/// Reads writes off one connection until it closes. A write longer than the buffer closes the connection.
fn serve_stream<S: Read + Write>(mut stream: S, port: String, settings: Settings) {
    let mut buf = vec![0u8; settings.buffer + 1];
    let mut first = true;
    loop {
        let n = match stream.read(&mut buf) {
            Ok(0) | Err(_) => return,
            Ok(n) => n,
        };
        if n > settings.buffer {
            return;
        }
        let text = String::from_utf8_lossy(&buf[..n]).trim_end_matches('\0').to_string();
        let (tx, rx) = mpsc::channel();
        if let Ok(mut q) = QUEUE.lock() {
            q.push(Request { port: port.clone(), text, settings: settings.clone(), first, reply: tx });
        }
        first = false;
        let Ok(answer) = rx.recv() else { return };
        if settings.noreturn {
            continue;
        }
        if stream.write_all(&answer).is_err() {
            return;
        }
        let _ = stream.flush();
    }
}

/// A value as text for the client: arrays tab-separated, true and false as 1 and 0.
fn plain(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        Value::Bool(b) => (*b as i32).to_string(),
        Value::Number(n) => match n.as_i64() {
            Some(i) => i.to_string(),
            None => crate::mel::fmt_float(n.as_f64().unwrap_or(0.0)),
        },
        Value::Array(a) => a.iter().map(plain).collect::<Vec<_>>().join("\t"),
        o => o.to_string(),
    }
}

#[cfg(feature = "python")]
fn run_python(core: &mut Core, src: &str) -> Result<Value, String> {
    crate::python::eval_from_mel(core, src)
}

#[cfg(not(feature = "python"))]
fn run_python(_core: &mut Core, _src: &str) -> Result<Value, String> {
    Err("Python is not available in this build.".into())
}

/// A Python value pickled (protocol 0, which is text).
fn pickled(core: &mut Core, v: &Value) -> Result<String, String> {
    let lit = serde_json::to_string(&v.to_string()).map_err(|e| e.to_string())?;
    let r = run_python(core, &format!("__import__('pickle').dumps(__import__('json').loads({lit}), 0).decode('ascii')"))?;
    Ok(plain(&r))
}

fn mel_quote(t: &str) -> String {
    format!("\"{}\"", t.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n"))
}

/// How many commands a write holds: MEL statements, or Python's top-level lines.
fn command_count(text: &str, python: bool) -> usize {
    if python {
        text.lines().filter(|l| !l.trim().is_empty() && !l.starts_with([' ', '\t']) && !l.trim_start().starts_with('#')).count()
    } else {
        crate::mel::statements(text).len()
    }
}

/// Runs one write as the port's settings say and gives what goes back to the client.
fn run_request(core: &mut Core, r: &Request) -> Vec<u8> {
    let st = &r.settings;
    if st.warn && r.first {
        core.push(format!("// Warning: commandPort: a client of port {} is running commands with this session's permissions. //", r.port), LineKind::Warning);
    }
    let before = core.log.len();
    let mut ran = 0usize;
    let result: Result<Value, String> = if let Some(pre) = st.prefix.as_deref().filter(|p| !p.is_empty()) {
        // Each line is a write of its own, given to the prefix command as its one string; the last answer counts.
        let mut last = Ok(Value::Null);
        for line in r.text.split('\n').map(|l| l.trim_end_matches('\r')).filter(|l| !l.is_empty()) {
            last = core.mel_checked(&format!("{pre} {};", mel_quote(line)));
            ran += 1;
            if last.is_err() {
                break;
            }
        }
        last
    } else if st.python {
        ran = command_count(&r.text, true);
        run_python(core, &r.text)
    } else {
        ran = command_count(&r.text, false);
        core.mel_checked(&r.text)
    };
    let printed: Vec<String> = core.log.get(before..).unwrap_or(&[]).iter().filter(|l| !matches!(l.kind, LineKind::Echo | LineKind::Result)).map(|l| l.text.clone()).collect();
    core.version += 1;
    if st.noreturn {
        // The results go to the Script Editor instead.
        match &result {
            Ok(v) if !v.is_null() => core.push(if st.python { format!("# Result: {} #", plain(v)) } else { format!("// Result: {} //", plain(v)) }, LineKind::Result),
            Err(e) => core.push(if st.python { format!("# Error: {e} #") } else { format!("// Error: {e} //") }, LineKind::Error),
            _ => {}
        }
        return Vec::new();
    }
    let mut body = if st.count {
        ran.to_string()
    } else {
        match &result {
            Ok(v) if st.pickle && st.python => pickled(core, v).unwrap_or_else(|e| format!("# Error: {e} #")),
            Ok(v) => plain(v),
            Err(e) => {
                if st.python {
                    format!("# Error: {e} #")
                } else {
                    format!("// Error: {e} //")
                }
            }
        }
    };
    if st.echo && !st.count {
        let mut all = printed.join("\n");
        if !all.is_empty() && !body.is_empty() {
            all.push('\n');
        }
        all.push_str(&body);
        body = all;
    }
    if body.len() > st.buffer {
        body = format!("// Error: commandPort: the result is longer than the port's buffer of {} characters. //", st.buffer);
    }
    let mut out = body.into_bytes();
    out.extend_from_slice(b"\n\0");
    out
}

/// The core's idle step: runs what clients have written since the last one.
pub(crate) fn serve(core: &mut Core) {
    let me = std::thread::current().id();
    let due: Vec<Request> = QUEUE
        .lock()
        .map(|mut q| {
            let (mine, rest): (Vec<Request>, Vec<Request>) = std::mem::take(&mut *q).into_iter().partition(|r| r.settings.owner == me);
            *q = rest;
            mine
        })
        .unwrap_or_default();
    for r in due {
        let answer = run_request(core, &r);
        let _ = r.reply.send(answer);
    }
}

fn command(_s: &mut Session, a: &Args) -> CmdResult {
    let named = a.get("name").map(|v| match v {
        Value::String(s) => s.clone(),
        o => plain(o),
    });
    let name = named.or_else(|| a.pos_str(0).map(str::to_string)).unwrap_or_else(|| DEFAULT_NAME.to_string());
    if a.bool("listPorts") {
        return Ok(json!(open_ports()));
    }
    if a.bool("query") {
        return Ok(json!(is_open(&name)));
    }
    if a.bool("close") {
        close(&name).map_err(aztek_ops::CommandError::new)?;
        return Ok(Value::Null);
    }
    let buffer = a.int("bufferSize", DEFAULT_BUFFER as i64);
    if buffer < 1 {
        fail!("commandPort: -bufferSize must be at least 1.");
    }
    let source = a.string("sourceType", "mel");
    if source != "mel" && source != "python" {
        fail!("commandPort: -sourceType is \"mel\" or \"python\", not \"{source}\".");
    }
    let settings = Settings {
        buffer: buffer as usize,
        echo: a.bool("echoOutput"),
        noreturn: a.bool("noreturn"),
        pickle: a.bool("pickleOutput"),
        prefix: a.get("prefix").and_then(Value::as_str).map(str::to_string),
        count: a.bool("returnNumCommands") && !a.bool("noreturn"),
        warn: a.bool("securityWarning"),
        python: source == "python",
        owner: std::thread::current().id(),
    };
    open(&name, settings).map_err(aztek_ops::CommandError::new)?;
    Ok(Value::Null)
}

pub fn register(s: &mut Session) {
    let f = |n: &str, sh: &str, ty: ArgType, doc: &str| FlagDef::new(n, sh, ty, Value::Null, doc);
    s.register(
        CommandDef::<Session>::new(
            "commandPort",
            "Opens or closes a command port, a socket clients connect to and write commands into; each write runs and its result \
             goes back. A plain name is a UNIX socket in /tmp, \"/path\" a socket at that path, \":port\" a local INET socket. \
             Query: whether the named port is open. There is no authentication: any client runs commands with this session's \
             permissions.",
            command,
        )
        .positional("the port's name (mayaCommand when none is given)")
        .undoable(false)
        .flag(f("bufferSize", "bs", ArgType::Int, "The buffer's size in characters, 4096 by default. A longer command closes the connection; a longer result is replaced by an error."))
        .flag(f("close", "cl", ArgType::Bool, "Close the port and delete its socket file. Connections already made stay open."))
        .flag(f("echoOutput", "eo", ArgType::Bool, "Send everything the commands print to the client, not only the results."))
        .flag(f("listPorts", "lp", ArgType::Bool, "The open ports' names."))
        .flag(f("name", "n", ArgType::String, "The port's name: a UNIX socket in /tmp, a full socket path starting with \"/\", or \":port\" for a local INET socket."))
        .flag(f("noreturn", "nr", ArgType::Bool, "Results go to the Script Editor rather than back to the client."))
        .flag(f("pickleOutput", "po", ArgType::Bool, "Python results go back pickled."))
        .flag(f("prefix", "pre", ArgType::String, "A command taking one string, run with each write (each line a write of its own); only the last answer goes back."))
        .flag(f("returnNumCommands", "rnc", ArgType::Bool, "Send back how many commands were read and run, not their results. Ignored with -noreturn."))
        .flag(f("securityWarning", "sw", ArgType::Bool, "Warn when a client sends commands."))
        .flag(f("sourceType", "stp", ArgType::String, "The language of what is written: \"mel\" (the default) or \"python\"."))
        .flag(f("query", "q", ArgType::Bool, "Query mode."))
        .flag(f("queried", "", ArgType::List, "Flags asked about in query mode.")),
    );
}
