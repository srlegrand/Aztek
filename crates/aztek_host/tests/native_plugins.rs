//! Compiled plug-ins: the kit's samples are built with CMake against devkit/ and loaded with loadPlugin. Plug-in node
//! types belong to the process, so one test walks through them in order. Without CMake and a C++ compiler it says so
//! and passes.

use std::path::{Path, PathBuf};
use std::process::Command;

use aztek_host::Core;
use serde_json::{json, Value};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Builds a CMake project of plug-ins into target/; None when there is no CMake or no compiler.
fn build(src: &Path, name: &str) -> Option<PathBuf> {
    let out = root().join("target").join(name);
    let configure = Command::new("cmake").arg("-S").arg(src).arg("-B").arg(&out).arg("-DCMAKE_BUILD_TYPE=Release").output().ok()?;
    if !configure.status.success() {
        eprintln!("cmake configure failed:\n{}", String::from_utf8_lossy(&configure.stderr));
        return None;
    }
    let built = Command::new("cmake").arg("--build").arg(&out).arg("--config").arg("Release").output().expect("cmake --build");
    assert!(built.status.success(), "the plug-ins don't build:\n{}{}", String::from_utf8_lossy(&built.stdout), String::from_utf8_lossy(&built.stderr));
    // Multi-config generators put them in Release/.
    Some(if out.join("Release").is_dir() { out.join("Release") } else { out })
}

fn mel(c: &mut Core, src: &str) -> Value {
    c.mel_checked(src).unwrap_or_else(|e| panic!("{src}: {e}"))
}

fn mel_err(c: &mut Core, src: &str) -> String {
    match c.mel_checked(src) {
        Ok(v) => panic!("{src} should fail, gave {v}"),
        Err(e) => e,
    }
}

fn num(v: &Value) -> f64 {
    match v {
        Value::Array(a) => num(&a[0]),
        v => v.as_f64().unwrap_or_else(|| panic!("not a number: {v}")),
    }
}

fn log_text(c: &mut Core) -> String {
    c.log.iter().map(|l| l.text.clone()).collect::<Vec<_>>().join("\n")
}

#[test]
fn samples_load_and_work() {
    let Some(samples) = build(&root().join("devkit/samples"), "devkit-samples") else {
        eprintln!("no CMake or C++ compiler: compiled plug-ins not tested");
        return;
    };
    let tests = build(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/native"), "devkit-test-plugins").expect("test plug-ins");
    std::env::set_var("AZTEK_PLUG_IN_PATH", std::env::join_paths([&samples, &tests]).unwrap());
    let mut c = Core::new();

    // A command: flags, result, undo and redo.
    assert_eq!(mel(&mut c, "loadPlugin helloCmd;"), json!(["helloCmd"]));
    assert_eq!(mel(&mut c, "pluginInfo -q -loaded helloCmd;"), json!(true));
    assert_eq!(mel(&mut c, "pluginInfo -q -command helloCmd;"), json!(["azHello"]));
    assert_eq!(mel(&mut c, "pluginInfo -q -vendor helloCmd;"), json!("Aztek samples"));
    assert_eq!(mel(&mut c, "azHello -name \"mark\" -c 2;"), json!(["mark1", "mark2"]));
    assert_eq!(mel(&mut c, "objExists mark2;"), json!(true));
    // MDagModifier gives back the transform it makes over a shape: the rename lands there, and the shape is below.
    assert_eq!(mel(&mut c, "nodeType mark2;"), json!("transform"));
    assert_eq!(mel(&mut c, "listRelatives -s mark2;").as_array().map(|a| a.len()), Some(1));
    mel(&mut c, "undo;");
    assert_eq!(mel(&mut c, "objExists mark1;"), json!(false));
    mel(&mut c, "redo;");
    assert_eq!(mel(&mut c, "objExists mark1;"), json!(true));
    let e = mel_err(&mut c, "azHello -c 0;");
    assert!(e.contains("kInvalidParameter") && log_text(&mut c).contains("at least 1"), "{e}\n{}", log_text(&mut c));
    mel(&mut c, "azHello -n \"chatty\" -v;");
    assert!(log_text(&mut c).contains("azHello made chatty1"), "{}", log_text(&mut c));

    // A node: computes, saves with its plug-in required, and reads back.
    assert_eq!(mel(&mut c, "loadPlugin scaleNode;"), json!(["scaleNode"]));
    assert_eq!(mel(&mut c, "createNode azScale;"), json!("azScale1"));
    mel(&mut c, "setAttr azScale1.input 3;");
    assert_eq!(num(&mel(&mut c, "getAttr azScale1.output;")), 6.0);
    mel(&mut c, "setAttr azScale1.mode 1;");
    assert_eq!(num(&mel(&mut c, "getAttr azScale1.output;")), 5.0);
    mel(&mut c, "setAttr azScale1.vector 1 2 3;");
    let v = mel(&mut c, "getAttr azScale1.vectorOut;");
    assert_eq!(v.to_string().replace(".0", ""), "[3,4,5]");
    let dir = root().join("target/devkit-scenes");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("scale.ma").to_string_lossy().replace('\\', "/");
    mel(&mut c, &format!("file -rename \"{file}\"; file -save -type \"mayaAscii\";"));
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(text.contains("requires \"scaleNode\" \"1.0\";"), "{text}");
    assert!(text.contains("createNode azScale -n \"azScale1\";"), "{text}");
    mel(&mut c, "file -f -new;");
    assert_eq!(mel(&mut c, "objExists azScale1;"), json!(false));
    mel(&mut c, &format!("file -f -o \"{file}\";"));
    assert_eq!(num(&mel(&mut c, "getAttr azScale1.output;")), 5.0);

    // A deformer: moves points, takes the envelope, and is in the mesh's history.
    assert_eq!(mel(&mut c, "loadPlugin rippleDeformer;"), json!(["rippleDeformer"]));
    mel(&mut c, "polyPlane -sx 10 -sy 10 -w 10 -h 10;");
    let before = mel(&mut c, "pointPosition pPlane1.vtx[17];");
    assert_eq!(mel(&mut c, "deformer -type azRipple pPlane1;"), json!(["azRipple1"]));
    mel(&mut c, "setAttr azRipple1.amplitude 1;");
    let after = mel(&mut c, "pointPosition pPlane1.vtx[17];");
    assert_ne!(before, after, "the ripple moved nothing");
    let (x, z) = (num(&before[0]), num(&before[2]));
    assert!((num(&after[1]) - x.sin() * z.cos()).abs() < 1e-6, "{before} -> {after}");
    mel(&mut c, "setAttr azRipple1.envelope 0;");
    assert_eq!(mel(&mut c, "pointPosition pPlane1.vtx[17];"), before);
    let history = mel(&mut c, "listHistory pPlane1;");
    assert!(history.as_array().unwrap().contains(&json!("azRipple1")), "{history}");

    // Unloading: a command goes; a plug-in whose nodes are in the scene stays unless forced.
    mel(&mut c, "unloadPlugin helloCmd;");
    assert!(mel_err(&mut c, "azHello;").contains("azHello"));
    assert!(mel_err(&mut c, "unloadPlugin scaleNode;").contains("still in use"));
    mel(&mut c, "unloadPlugin -f scaleNode;");
    assert_eq!(mel(&mut c, "pluginInfo -q -loaded scaleNode;"), json!(false));

    // A plug-in that crashes in compute is reported, and its nodes stop computing; the process goes on.
    assert_eq!(mel(&mut c, "loadPlugin crashNode;"), json!(["crashNode"]));
    mel(&mut c, "createNode azCrash;");
    c.log.clear();
    mel(&mut c, "setAttr azCrash1.crash 1; getAttr azCrash1.output;");
    let text = log_text(&mut c);
    assert!(text.contains("crashed") && text.contains("SIGSEGV") || text.contains("access violation"), "{text}");
    assert_eq!(num(&mel(&mut c, "getAttr azCrash1.output;")), 0.0);
    // A C++ exception is a failed compute, not a crash.
    mel(&mut c, "createNode azThrow; setAttr azThrow1.input 1;");
    c.log.clear();
    mel(&mut c, "getAttr azThrow1.output;");
    assert!(log_text(&mut c).contains("compute failed"), "{}", log_text(&mut c));
}
