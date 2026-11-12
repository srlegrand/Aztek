//! Aztek's foundation: paths, the workspace, layered prefs and the main-thread queue.

pub mod env;
pub mod main_queue;
pub mod paths;
pub mod prefs;
pub mod workspace;

pub use prefs::{PrefScope, PrefStore};
pub use workspace::Workspace;

static BATCH: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Whether this process is `aztek --batch`: no window, as `about -batch` answers.
pub fn batch_mode() -> bool {
    BATCH.load(std::sync::atomic::Ordering::Relaxed)
}

pub fn set_batch_mode(on: bool) {
    BATCH.store(on, std::sync::atomic::Ordering::Relaxed);
}
