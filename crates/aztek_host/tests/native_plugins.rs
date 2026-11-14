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

    // The kit's iterators and function sets, reading the scene: azProbe gives back name=value strings.
    assert_eq!(mel(&mut c, "loadPlugin apiProbe;"), json!(["apiProbe"]));
    let probe = |c: &mut Core, what: &str| -> std::collections::HashMap<String, String> {
        let v = mel(c, &format!("azProbe {what};"));
        v.as_array().unwrap_or_else(|| panic!("azProbe {what}: {v}")).iter().filter_map(|x| x.as_str()?.split_once('=').map(|(a, b)| (a.to_string(), b.to_string()))).collect()
    };
    let f = |m: &std::collections::HashMap<String, String>, k: &str| -> f64 { m.get(k).unwrap_or_else(|| panic!("no {k} in {m:?}")).parse().unwrap() };
    mel(&mut c, "file -f -new; polyCube -n box;");
    let m = probe(&mut c, "mesh");
    for (k, want) in [("faces", 6.0), ("quads", 6.0), ("faceNeighbours4", 6.0), ("area", 6.0), ("face0Edges", 4.0), ("face0NormalLength", 1.0), ("face0CenterFromOrigin", 0.5), ("edges", 12.0), ("edgesWithTwoFaces", 12.0), ("edgesWithFourNeighbours", 12.0), ("unitEdges", 12.0), ("vertex0Edges", 3.0), ("vertex0EdgesEndingThere", 3.0), ("pickedVisited", 2.0)] {
        assert!((f(&m, k) - want).abs() < 1e-6, "{k}: {m:?}");
    }
    mel(&mut c, "group -n grp box; spaceLocator -n loc; parent loc grp;");
    let m = probe(&mut c, "dag");
    assert_eq!(f(&m, "meshes"), 1.0, "{m:?}");
    assert_eq!(f(&m, "boxShapeDepth"), 3.0, "{m:?}");
    assert_eq!(m["underGrp"], "grp,box,loc,boxShape,locShape,", "{m:?}");
    assert_eq!(f(&m, "pruned"), 1.0, "{m:?}");
    mel(&mut c, "setKeyframe -t 1 -v 0 box.tx; setKeyframe -t 10 -v 5 box.tx; setKeyframe -t 1 -v 0 box.ry; setKeyframe -t 10 -v 90 box.ry;");
    let m = probe(&mut c, "anim");
    for (k, want) in [("txType", 1.0), ("txKeys", 2.0), ("txTime1", 10.0), ("txValue1", 5.0), ("txAt10", 5.0), ("ryValue1", std::f64::consts::FRAC_PI_2), ("ryAt10", std::f64::consts::FRAC_PI_2), ("ryValue0", 0.5), ("ryValue2", 1.0), ("txKeysAfterAdd", 3.0), ("found20", 2.0), ("outTangent20", 2.0), ("txValue0", 2.0), ("txKeysAfterRemove", 2.0)] {
        assert!((f(&m, k) - want).abs() < 1e-6, "{k}: {m:?}");
    }
    mel(&mut c, "file -f -new; polyCube -n box -h 2 -sy 2; select -cl; joint -n j1 -p 0 -1 0; joint -n j2 -p 0 1 0; skinCluster -n skin1 j1 box;");
    let m = probe(&mut c, "skin");
    for (k, want) in [("influences", 2.0), ("weightCount", 24.0), ("perPoint", 2.0), ("point0Sum", 1.0), ("v0Influence1", 1.0), ("logicalOfJ2", 1.0)] {
        assert!((f(&m, k) - want).abs() < 1e-6, "{k}: {m:?}");
    }
    assert_eq!(m["influence1"], "j2", "{m:?}");
    mel(&mut c, "file -f -new; nurbsPlane -n surf -ax 0 1 0 -w 4 -u 2 -v 3 -d 3; move 0 2 0 surf;");
    let m = probe(&mut c, "surface");
    for (k, want) in [("cvsU", 5.0), ("cvsV", 6.0), ("degreeU", 3.0), ("midY", 2.0), ("normalLength", 1.0), ("closestY", 2.0), ("cvCount", 30.0)] {
        assert!((f(&m, k) - want).abs() < 1e-6, "{k}: {m:?}");
    }
    assert!((f(&m, "area") - 16.0).abs() < 0.05, "{m:?}");

    // Messages: what a plug-in's callbacks hear. azMsg log gives back one line per call, and forgets them.
    assert_eq!(mel(&mut c, "loadPlugin msgProbe;"), json!(["msgProbe"]));
    let heard = |c: &mut Core, src: &str| -> Vec<String> {
        if !src.is_empty() {
            mel(c, src);
        }
        let v = mel(c, "azMsg log;");
        v.as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default()
    };
    mel(&mut c, "file -f -new; createNode transform -n a; azMsg start; azMsg watch a;");
    for (src, want) in [
        ("createNode transform -n b -p a;", vec!["added b transform", "dag 0 b a", "dag 2 b a"]),
        // setAttr: kAttributeSet with kIncomingDirection, the node dirty, its matrix's translateX.
        ("setAttr a.tx 2;", vec!["attr 2056 a.translateX", "dirty a", "matrix a 512"]),
        ("rename a aa;", vec!["name aa was a"]),
        ("connectAttr aa.tx b.ty;", vec!["conn aa.translateX b.translateY made", "attr 1 aa.translateX b.translateY"]),
        ("disconnectAttr aa.tx b.ty;", vec!["conn aa.translateX b.translateY broken", "attr 2 aa.translateX b.translateY"]),
        ("addAttr -ln extra -at double aa;", vec!["attr 64 aa.extra", "attrAddRemove 64 extra"]),
        ("setAttr -l 1 aa.tz;", vec!["attr 16 aa.translateZ"]),
        ("parent -w b;", vec!["dag 1 b aa", "dag 3 b aa", "dag 0 b world", "dag 2 b world"]),
        ("currentTime 12;", vec!["time 12"]),
        // Told before it goes, its name and type still there; then removed.
        ("delete aa;", vec!["aboutToDelete aa transform", "preRemoval aa transform", "dag 1 aa world", "dag 3 aa world", "removed aa transform"]),
        ("undo;", vec!["added aa transform", "dag 0 aa world", "dag 2 aa world"]),
    ] {
        assert_eq!(heard(&mut c, src), want, "{src}");
    }
    assert_eq!(mel(&mut c, "azMsg nodeCallbacks aa;"), json!(7));
    // Event messages come with the idle loop, as script jobs do.
    mel(&mut c, "select -r b;");
    c.idle();
    assert_eq!(heard(&mut c, ""), vec!["event"]);
    // A file open: the check may pick another file; the old scene's nodes go, the file's arrive, then the after messages.
    let dir = std::env::temp_dir();
    let (fa, fb) = (dir.join("azmsg_a.ma").to_string_lossy().replace('\\', "/"), dir.join("azmsg_b.ma").to_string_lossy().replace('\\', "/"));
    let h = heard(&mut c, &format!("file -rename \"{fa}\"; file -f -save;"));
    assert_eq!(h.iter().filter(|l| l.starts_with("scene")).collect::<Vec<_>>(), ["scene 12", "scene 13"], "{h:?}");
    heard(&mut c, &format!("createNode transform -n onlyInB; file -rename \"{fb}\"; file -f -save; azMsg redirect \"{fa}\";"));
    let h = heard(&mut c, &format!("file -f -o \"{fb}\";"));
    assert_eq!(h[..3], ["check open azmsg_b.ma", "scene 5", "scene 7"], "{h:?}");
    assert!(h.contains(&"removed onlyInB transform".to_string()) && h.contains(&"added aa transform".to_string()), "{h:?}");
    assert_eq!(h[h.len() - 4..], ["scene 8", "scene 6", "scene 9", "scene 0"], "{h:?}");
    assert_eq!(mel(&mut c, "objExists onlyInB;"), json!(false));
    mel(&mut c, "azMsg redirect;");
    // An import, and a reference made, unloaded and removed: each with the reference's messages around it.
    let h = heard(&mut c, &format!("file -i -ns imp \"{fb}\";"));
    assert_eq!(h[..2], ["scene 3", "scene 7"], "{h:?}");
    assert_eq!(h[h.len() - 3..], ["scene 8", "scene 4", "scene 9"], "{h:?}");
    let h = heard(&mut c, &format!("file -r -ns ref \"{fb}\";"));
    assert_eq!(h[..6], ["scene 14", "scene 44", "scene 49", "scene 36", "scene 47", "scene 7"], "{h:?}");
    assert_eq!(h[h.len() - 8..], ["scene 8", "scene 15", "scene 45", "scene 50", "scene 37", "scene 48", "scene 9", "scene 0"], "{h:?}");
    let h = heard(&mut c, &format!("file -ur \"{fb}\";"));
    assert_eq!(h[..1], ["scene 22"], "{h:?}");
    assert_eq!(h[h.len() - 2..], ["scene 23", "scene 0"], "{h:?}");
    let h = heard(&mut c, &format!("file -rr \"{fb}\";"));
    assert_eq!(h[..1], ["scene 16"], "{h:?}");
    assert_eq!(h[h.len() - 2..], ["scene 17", "scene 0"], "{h:?}");
    // A new scene: once, with its check, every node removed, then after new and scene update.
    let h = heard(&mut c, "file -f -new;");
    assert_eq!(h[..2], ["check new".to_string(), "scene 1".to_string()], "{h:?}");
    assert_eq!(h.iter().filter(|l| *l == "scene 1").count(), 1, "{h:?}");
    assert!(h.contains(&"removed b transform".to_string()), "{h:?}");
    assert_eq!(h[h.len() - 2..], ["scene 2".to_string(), "scene 0".to_string()], "{h:?}");
    // A check callback that says no stops the new scene.
    mel(&mut c, "createNode transform -n keep; azMsg refuse 1;");
    assert!(mel_err(&mut c, "file -f -new;").contains("File operation cancelled by user supplied callback."));
    assert_eq!(mel(&mut c, "objExists keep;"), json!(true));
    mel(&mut c, "azMsg refuse 0; azMsg log;");
    // A callback that removes itself hears once.
    let h = heard(&mut c, "azMsg once; createNode transform -n x1; createNode transform -n x2;");
    assert!(h.contains(&"once x1".to_string()) && !h.contains(&"once x2".to_string()), "{h:?}");
    // Plug-ins loaded: the file, then the name.
    let h = heard(&mut c, "unloadPlugin apiProbe; loadPlugin apiProbe;");
    assert!(h.iter().any(|l| l.starts_with("plugin ") && l.contains("apiProbe") && l.ends_with("|apiProbe")), "{h:?}");
    // Removed, nothing is heard.
    mel(&mut c, "azMsg stop;");
    assert!(heard(&mut c, "createNode transform -n quiet; currentTime 3; file -f -new;").is_empty());

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
