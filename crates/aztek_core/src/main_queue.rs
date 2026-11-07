//! Work posted from any thread to run on the main thread.

use std::sync::{Mutex, OnceLock};
use std::thread::ThreadId;

type Work = Box<dyn FnOnce() + Send>;
type Wake = std::sync::Arc<dyn Fn() + Send + Sync>;

static MAIN: OnceLock<ThreadId> = OnceLock::new();
static QUEUE: Mutex<Vec<Work>> = Mutex::new(Vec::new());
static WAKE: Mutex<Option<Wake>> = Mutex::new(None);

thread_local! {
    static HOLDS: std::cell::Cell<i32> = const { std::cell::Cell::new(0) };
}

/// Call once from the main thread at startup.
pub fn init() {
    let _ = MAIN.set(std::thread::current().id());
}

pub fn is_main_thread() -> bool {
    MAIN.get().is_some_and(|&id| id == std::thread::current().id())
}

/// Queue work for the main thread. Safe from any thread.
pub fn post(f: impl FnOnce() + Send + 'static) {
    QUEUE.lock().unwrap().push(Box::new(f));
    let wake = WAKE.lock().unwrap().clone();
    if let Some(w) = wake {
        w();
    }
}

/// Run everything queued so far. Main thread only. Does nothing while held.
pub fn drain() {
    if HOLDS.with(|h| h.get()) > 0 {
        return;
    }
    let work = std::mem::take(&mut *QUEUE.lock().unwrap());
    for f in work {
        f();
    }
}

/// Holds queued work back (drain does nothing) while long work pumps the event loop; nests.
pub struct Hold(());

impl Hold {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        HOLDS.with(|h| h.set(h.get() + 1));
        Hold(())
    }
}

impl Drop for Hold {
    fn drop(&mut self) {
        let left = HOLDS.with(|h| {
            h.set(h.get() - 1);
            h.get()
        });
        if left > 0 {
            return;
        }
        let pending = !QUEUE.lock().unwrap().is_empty();
        let wake = if pending { WAKE.lock().unwrap().clone() } else { None };
        if let Some(w) = wake {
            w();
        }
    }
}

/// Called (on the posting thread) whenever post() queues work, so the host event loop can schedule a drain.
pub fn set_wakeup(wake: Option<Wake>) {
    *WAKE.lock().unwrap() = wake;
}
