//! Tests: core.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use aztek_core::{main_queue, paths, PrefScope, PrefStore, Workspace};
use serde_json::{json, Value};

/// The store and the workspace are process-wide: one test at a time.
static SERIAL: Mutex<()> = Mutex::new(());

fn store() -> &'static PrefStore {
    // Keep test writes out of the real config folder.
    let dir = std::env::temp_dir().join("aztek_test_config");
    std::env::set_var("AZTEK_CONFIG_DIR", &dir);
    PrefStore::get()
}

#[test]
fn prefs_write_through_and_pick_up_external_edits() {
    let _l = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    main_queue::init();
    let p = store();
    p.start();
    let file = p.user_dir().join("test_prefs.json");
    let _ = std::fs::remove_file(&file);

    p.set("test_prefs", "/a/b", json!(42), PrefScope::User);
    p.flush();
    let j: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    assert_eq!(j["a"]["b"], 42);

    let notified = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let n2 = notified.clone();
    let id = p.on_changed(std::sync::Arc::new(move |f: &str| {
        if f == "test_prefs" {
            n2.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }));
    // A different mtime than our own write.
    std::thread::sleep(Duration::from_millis(1100));
    std::fs::write(&file, r#"{"a": {"b": 7}}"#).unwrap();

    for _ in 0..40 {
        if p.i64("test_prefs", "/a/b", 0) == 7 {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(p.i64("test_prefs", "/a/b", 0), 7);
    main_queue::drain();
    assert!(notified.load(std::sync::atomic::Ordering::SeqCst));

    p.remove_on_changed(id);
    let _ = std::fs::remove_file(&file);
}

#[test]
fn studio_prefs_are_defaults_locked_keys_win_and_an_offline_share_uses_the_local_copy() {
    let _l = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let p = store();
    let studio = std::env::temp_dir().join("aztek_test_studio_prefs");
    let moved = std::env::temp_dir().join("aztek_test_studio_prefs_offline");
    let _ = std::fs::remove_dir_all(&studio);
    let _ = std::fs::remove_dir_all(&moved);
    std::fs::create_dir_all(studio.join("shelves")).unwrap();
    std::fs::write(studio.join("test_studio.json"), r#"{"a": 1, "b": {"c": 2}, "$locked": ["/b"]}"#).unwrap();
    std::fs::write(studio.join("shelves").join("Studio Tools.json"), r#"{"items": [], "$locked": ["/"]}"#).unwrap();
    p.remove("test_studio", "/a", PrefScope::User);
    p.set_studio_dirs(vec![studio.clone()]);

    assert_eq!(p.i64("test_studio", "/a", 0), 1);
    p.set("test_studio", "/a", json!(5), PrefScope::User);
    assert_eq!(p.i64("test_studio", "/a", 0), 5);

    assert!(p.is_locked("test_studio", "/b/c"));
    assert!(!p.is_locked("test_studio", "/a"));
    p.set("test_studio", "/b/c", json!(9), PrefScope::User);
    assert_eq!(p.i64("test_studio", "/b/c", 0), 2);
    let merged = p.merged("test_studio");
    assert_eq!(merged["a"], 5);
    assert_eq!(merged["b"]["c"], 2);
    assert!(merged.get("$locked").is_none());

    assert!(p.is_locked("shelves/Studio Tools", "/items"));
    assert_eq!(p.studio_files("shelves/"), vec!["shelves/Studio Tools".to_string()]);

    // The share goes offline: the cached copy keeps the values and the lock.
    std::fs::rename(&studio, &moved).unwrap();
    p.set_studio_dirs(vec![studio.clone()]);
    assert!(p.is_locked("test_studio", "/b/c"));
    assert_eq!(p.i64("test_studio", "/b/c", 0), 2);
    assert_eq!(p.studio_files("shelves/"), vec!["shelves/Studio Tools".to_string()]);

    p.set_studio_dirs(Vec::new());
    assert!(!p.is_locked("test_studio", "/b/c"));
    p.remove("test_studio", "/a", PrefScope::User);
    let _ = std::fs::remove_dir_all(&moved);
}

#[test]
fn paths_relative_to() {
    if cfg!(windows) {
        assert_eq!(paths::relative_to(Path::new("X:/Voidtide/Aztek/tex/a.png"), Path::new("X:/Voidtide/Aztek")), Some(PathBuf::from("tex/a.png")));
        assert!(paths::relative_to(Path::new("C:/Ref/a.png"), Path::new("X:/Voidtide/Aztek")).is_none());
    } else {
        assert_eq!(paths::to_utf8(&paths::relative_to(Path::new("/v/Ref/a.png"), Path::new("/v/Aztek")).unwrap()), "../Ref/a.png");
        assert!(paths::relative_to(Path::new("tex/a.png"), Path::new("/v/Aztek")).is_none());
        assert_eq!(paths::relative_to(Path::new("/v/Aztek"), Path::new("/v/Aztek")), Some(PathBuf::from(".")));
    }
}

/// <temp>/aztek_paths/{Aztek (workspace), Ref, scenes}, with Ref/fighter.png on disk.
struct Tree {
    top: PathBuf,
    ws: PathBuf,
    scenes: PathBuf,
    image: PathBuf,
}

impl Tree {
    fn new() -> Self {
        let raw = std::env::temp_dir().join("aztek_paths");
        let _ = std::fs::remove_dir_all(&raw);
        std::fs::create_dir_all(&raw).unwrap();
        let top = raw.canonicalize().unwrap();
        let ws = top.join("Aztek");
        let r = top.join("Ref");
        let scenes = ws.join("scenes");
        let image = r.join("fighter.png");
        std::fs::create_dir_all(ws.join(".aztek")).unwrap();
        std::fs::create_dir_all(&r).unwrap();
        std::fs::create_dir_all(&scenes).unwrap();
        std::fs::write(&image, "png").unwrap();
        Tree { top, ws, scenes, image }
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        Workspace::close();
        let _ = std::fs::remove_dir_all(&self.top);
    }
}

fn same(a: &Path, b: &Path) -> bool {
    a.canonicalize().ok().is_some_and(|x| b.canonicalize().ok().is_some_and(|y| x == y))
}

#[test]
fn paths_stored_and_resolved() {
    let _l = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    store();

    // empty and URLs pass through
    {
        let t = Tree::new();
        assert_eq!(paths::stored_path(Path::new(""), &t.scenes), "");
        assert_eq!(paths::stored_path(Path::new("https://example.com/a.png"), &t.scenes), "https://example.com/a.png");
        assert_eq!(paths::resolve_path("https://example.com/a.png", &t.scenes), PathBuf::from("https://example.com/a.png"));
    }
    // workspace: inside the root
    {
        let t = Tree::new();
        Workspace::open(&t.ws);
        let tex = Workspace::root().unwrap().join("tex").join("a.png");
        assert_eq!(paths::stored_path(&tex, &t.scenes), "tex/a.png");
        assert_eq!(paths::resolve_path("tex/a.png", &t.scenes), paths::normalize(&tex));
    }
    // workspace: a sibling of the root via ..
    {
        let t = Tree::new();
        Workspace::open(&t.ws);
        let stored = paths::stored_path(&t.image, &t.scenes);
        assert_eq!(stored, "../Ref/fighter.png");
        assert!(same(&paths::resolve_path(&stored, &t.scenes), &t.image));
    }
    // no workspace: relative to the scene folder
    {
        let t = Tree::new();
        let stored = paths::stored_path(&t.image, &t.scenes);
        assert_eq!(stored, "../../Ref/fighter.png");
        assert!(same(&paths::resolve_path(&stored, &t.scenes), &t.image));
    }
    // no base: absolute
    {
        let t = Tree::new();
        assert_eq!(paths::stored_path(&t.image, Path::new("")), paths::to_utf8(&t.image));
    }
    // relative paths: workspace root first, then the scene folder
    {
        let t = Tree::new();
        Workspace::open(&t.ws);
        std::fs::write(t.scenes.join("local.png"), "png").unwrap();
        assert!(same(&paths::resolve_path("local.png", &t.scenes), &t.scenes.join("local.png")));
        std::fs::write(t.ws.join("local.png"), "png").unwrap();
        assert!(same(&paths::resolve_path("local.png", &t.scenes), &t.ws.join("local.png")));
    }
    // round trip, stored again for another scene folder
    {
        let t = Tree::new();
        let old = paths::current_scene_dir();
        paths::set_current_scene_dir(&t.scenes);
        let stored = paths::stored_path(&t.image, &paths::current_scene_dir());
        assert_eq!(stored, "../../Ref/fighter.png");
        assert_eq!(paths::stored_path(Path::new(&stored), &paths::current_scene_dir()), stored);
        assert_eq!(paths::stored_path(Path::new(&stored), &t.ws), "../Ref/fighter.png");
        paths::set_current_scene_dir(&old);
    }
}
