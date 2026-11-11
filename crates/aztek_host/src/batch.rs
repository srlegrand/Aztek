//! `aztek --batch`: no window and no Vulkan. Opens a scene, runs scripts and commands, and exits with their status, as
//! the reference's `-batch` does. Handled before Qt starts, so it needs no display.
//!
//! Script output goes out as it happens: printed lines to stdout, warnings and errors to stderr.

use std::ffi::c_char;
use std::io::Write;

use std::sync::atomic::{AtomicUsize, Ordering};

use crate::core::LineKind;

static ERRORS: AtomicUsize = AtomicUsize::new(0);

const USAGE: &str = "usage: aztek --batch [--file scene] [--script file.py|file.mel] [--command mel] [scene]\n\
Runs in order: the scene, then each script and command as given. Exits 0, 1 on an uncaught error, or sys.exit's value.\n";

/// A history line, when in batch mode.
pub(crate) fn echo(text: &str, kind: LineKind) {
    if !aztek_core::batch_mode() {
        return;
    }
    match kind {
        LineKind::Info => {
            let mut o = std::io::stdout().lock();
            let _ = write!(o, "{text}{}", if text.ends_with('\n') { "" } else { "\n" });
            let _ = o.flush();
        }
        LineKind::Error => {
            ERRORS.fetch_add(1, Ordering::Relaxed);
            eprintln!("{}", text.trim_end());
        }
        LineKind::Warning => eprintln!("{}", text.trim_end()),
        LineKind::Echo | LineKind::Result => {}
    }
}

enum Step {
    Scene(String),
    Script(String),
    Command(String),
}

fn parse(args: &[String]) -> Result<Vec<Step>, String> {
    let mut steps = Vec::new();
    let mut i = 1;
    while i < args.len() {
        let a = args[i].as_str();
        let mut value = || {
            i += 1;
            args.get(i).cloned().ok_or_else(|| format!("{a} needs a value"))
        };
        match a {
            "--batch" | "-batch" => {}
            "--file" | "-file" => steps.push(Step::Scene(value()?)),
            "--script" | "-script" | "-s" => steps.push(Step::Script(value()?)),
            "--command" | "-command" | "-c" => steps.push(Step::Command(value()?)),
            other if other.starts_with('-') => return Err(format!("unknown option {other}")),
            other => steps.push(Step::Scene(other.to_string())),
        }
        i += 1;
    }
    Ok(steps)
}

fn run_mel(src: &str) -> i32 {
    let before = ERRORS.load(Ordering::Relaxed);
    match crate::with(|h| h.core.mel_checked(src)) {
        Ok(_) => 0,
        Err(e) => {
            // The interpreter reports most errors itself; this is for one that reached here unsaid.
            if ERRORS.load(Ordering::Relaxed) == before {
                crate::with(|h| h.core.push(format!("// Error: {e} //"), LineKind::Error));
            }
            1
        }
    }
}

fn run_script(path: &str) -> i32 {
    let src = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("aztek --batch: can't read {path}: {e}");
            return 1;
        }
    };
    if path.ends_with(".py") {
        #[cfg(feature = "python")]
        {
            crate::python::run_status(&src)
        }
        #[cfg(not(feature = "python"))]
        {
            eprintln!("aztek --batch: this build has no Python");
            1
        }
    } else {
        run_mel(&src)
    }
}

/// Runs the batch; the status is the first failing step's, and steps after it don't run.
pub fn run(args: &[String]) -> i32 {
    run_with(args, false)
}

/// As `run`; with `user_folder`, the user folder's paths, modules, auto loaded plug-ins and userSetup scripts are
/// there first, as the application has them. The saved optionVars are still not read.
fn run_with(args: &[String], user_folder: bool) -> i32 {
    let steps = match parse(args) {
        Ok(s) => s,
        Err(e) => {
            eprint!("aztek --batch: {e}\n{USAGE}");
            return 2;
        }
    };
    aztek_core::set_batch_mode(true);
    crate::aztek_init();
    if user_folder {
        crate::user_folder::apply(&crate::user_folder::load_choice());
        crate::aztek_startup_scripts();
    }
    for step in &steps {
        let status = match step {
            Step::Scene(path) => match crate::with(|h| h.core.open(path)) {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("aztek --batch: {path}: {e}");
                    1
                }
            },
            Step::Script(path) => run_script(path),
            Step::Command(src) => run_mel(src),
        };
        if status != 0 {
            return status;
        }
        // What the script deferred runs now, while its status can still count.
        let before = ERRORS.load(Ordering::Relaxed);
        for _ in 0..1000 {
            crate::with(|h| h.core.idle());
            if !crate::script_jobs::deferred_pending() {
                break;
            }
        }
        if ERRORS.load(Ordering::Relaxed) != before {
            return 1;
        }
    }
    0
}

/// The C entry point: `main` calls this before Qt starts.
#[no_mangle]
pub extern "C" fn aztek_batch(argc: i32, argv: *const *const c_char) -> i32 {
    let args: Vec<String> = (0..argc.max(0) as usize).map(|i| crate::cstr(unsafe { *argv.add(i) })).collect();
    run_with(&args, true)
}
