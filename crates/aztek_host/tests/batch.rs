//! `aztek --batch`: exit status from the script, output as it happens, `about -batch` true. In-process here; with
//! AZTEK_BIN set (CI, after the build) the same cases also run through the real binary with no display.
#![cfg(feature = "python")]

use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;

static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

fn dir() -> PathBuf {
    let d = std::env::temp_dir().join(format!("aztek_batch_test_{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn script(name: &str, body: &str) -> String {
    let p = dir().join(name);
    std::fs::write(&p, body).unwrap();
    p.to_string_lossy().into_owned()
}

/// The status, in this process and, when there's a binary to try, through it with no display.
fn status(args: &[&str]) -> i32 {
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let mut full = vec!["aztek".to_string(), "--batch".to_string()];
    full.extend(args.iter().map(|s| s.to_string()));
    let here = aztek_host::batch_run(&full);
    if let Some(bin) = std::env::var_os("AZTEK_BIN") {
        let out = Command::new(bin).args(&full[1..]).env_remove("DISPLAY").env_remove("WAYLAND_DISPLAY").output().unwrap();
        assert_eq!(out.status.code(), Some(here), "binary and library differ on {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    }
    here
}

#[test]
fn a_clean_script_is_zero() {
    assert_eq!(status(&["--script", &script("ok.py", "import maya.cmds as cmds\ncmds.polyCube()\nassert cmds.objExists('pCube1')\n")]), 0);
    assert_eq!(status(&["--script", &script("ok.mel", "polyCube -n batchCube;\n")]), 0);
}

#[test]
fn an_uncaught_error_is_one() {
    assert_eq!(status(&["--script", &script("bad.py", "raise ValueError('no')\n")]), 1);
    assert_eq!(status(&["--script", &script("bad.mel", "error \"no\";\n")]), 1);
    assert_eq!(status(&["--script", &script("nocmd.mel", "thereIsNoSuchCommand;\n")]), 1);
    assert_eq!(status(&["--script", "/nonexistent/x.py"]), 1);
}

#[test]
fn a_caught_error_is_zero() {
    let body = "import maya.cmds as cmds\ntry:\n    cmds.getAttr('nothing.tx')\nexcept Exception:\n    pass\n";
    assert_eq!(status(&["--script", &script("caught.py", body)]), 0);
}

#[test]
fn sys_exit_sets_the_status() {
    assert_eq!(status(&["--script", &script("exit3.py", "import sys\nsys.exit(3)\n")]), 3);
    assert_eq!(status(&["--script", &script("exit0.py", "import sys\nsys.exit()\n")]), 0);
}

#[test]
fn about_says_batch() {
    let body = "import maya.cmds as cmds\nimport maya.mel as mel\nassert cmds.about(batch=True) is True\nassert mel.eval('about -batch') == 1\n";
    assert_eq!(status(&["--script", &script("about.py", body)]), 0);
}

#[test]
fn deferred_work_runs_and_counts() {
    let body = "import maya.utils\ndef boom():\n    raise ValueError('later')\nmaya.utils.executeDeferred(boom)\n";
    assert_eq!(status(&["--script", &script("later.py", body)]), 1);
}

#[test]
fn steps_run_in_order_and_a_failure_stops_them() {
    let ma = dir().join("batch_scene.ma").to_string_lossy().replace('\\', "/");
    let save = format!("import maya.cmds as cmds\ncmds.file(new=True, force=True)\ncmds.polyCube(n='kept')\ncmds.file(rename='{ma}')\ncmds.file(save=True, type='mayaAscii')\n");
    assert_eq!(status(&["--script", &script("save.py", &save)]), 0);
    let check = script("check.py", "import maya.cmds as cmds\nimport sys\nsys.exit(0 if cmds.objExists('kept') else 5)\n");
    assert_eq!(status(&[&ma, "--script", &check]), 0);
    assert_eq!(status(&["--command", "error \"stop\";", "--script", &script("after.py", "import sys\nsys.exit(9)\n")]), 1);
    assert_eq!(status(&["--file", "/nonexistent/scene.ma"]), 1);
}

#[test]
fn a_bad_option_is_two() {
    assert_eq!(status(&["--nonsense"]), 2);
}

#[test]
fn the_basics_run_end_to_end() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/batch/basics.py");
    assert_eq!(status(&["--script", path]), 0);
}
