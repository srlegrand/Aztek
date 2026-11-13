//! The plug-ins that ship with Aztek (plugins/), built with CMake against devkit/ as the program's build builds them,
//! and loaded by name as a scene that requires them loads them. Without CMake and a C++ compiler it says so and passes.

use std::path::{Path, PathBuf};
use std::process::Command;

use aztek_host::Core;
use serde_json::{json, Value};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Built once for all the tests here, which run side by side.
fn build() -> Option<PathBuf> {
    static BUILT: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();
    BUILT.get_or_init(build_now).clone()
}

fn build_now() -> Option<PathBuf> {
    let out = root().join("target").join("shipped-plugins");
    let configure = Command::new("cmake").arg("-S").arg(root().join("plugins")).arg("-B").arg(&out).arg("-DCMAKE_BUILD_TYPE=Release").output().ok()?;
    if !configure.status.success() {
        eprintln!("cmake configure failed:\n{}", String::from_utf8_lossy(&configure.stderr));
        return None;
    }
    let built = Command::new("cmake").arg("--build").arg(&out).arg("--config").arg("Release").output().expect("cmake --build");
    assert!(built.status.success(), "the shipped plug-ins don't build:\n{}{}", String::from_utf8_lossy(&built.stdout), String::from_utf8_lossy(&built.stderr));
    Some(out.join("plug-ins"))
}

fn mel(c: &mut Core, src: &str) -> Value {
    c.mel_checked(src).unwrap_or_else(|e| panic!("{src}: {e}"))
}

fn nums(v: &Value) -> Vec<f64> {
    match v {
        Value::Array(a) => a.iter().flat_map(nums).collect(),
        v => vec![v.as_f64().unwrap_or_else(|| panic!("not a number: {v}"))],
    }
}

fn near(a: &[f64], b: &[f64], tol: f64) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < tol)
}

#[test]
fn mgear_solvers_ship_and_work() {
    let Some(dir) = build() else {
        eprintln!("no CMake or C++ compiler: the shipped plug-ins not tested");
        return;
    };
    std::env::set_var("AZTEK_PLUG_IN_PATH", &dir);
    let mut c = Core::new();
    assert_eq!(mel(&mut c, "loadPlugin mgear_solvers;"), json!(["mgear_solvers"]));

    // Angles cross the kit as the host holds them, in radians: a matrix constraint puts a child where its driver is,
    // under a parent turned and scaled.
    mel(&mut c, "createNode transform -n par; setAttr par.r 30 40 50; createNode joint -n obj -p par;");
    mel(&mut c, "createNode transform -n drv; setAttr drv.t 5 1 0; setAttr drv.r 10 20 30;");
    mel(&mut c, "createNode mgear_matrixConstraint -n mc;");
    mel(&mut c, "connectAttr drv.worldMatrix[0] mc.driverMatrix; connectAttr obj.parentInverseMatrix[0] mc.drivenParentInverseMatrix;");
    for a in ["translate", "rotate", "scale"] {
        mel(&mut c, &format!("connectAttr mc.{a} obj.{a};"));
    }
    let (want, got) = (nums(&mel(&mut c, "xform -q -ws -m drv;")), nums(&mel(&mut c, "xform -q -ws -m obj;")));
    assert!(near(&want, &got, 1e-6), "{want:?}\n{got:?}");

    // A deformer on a curve as the reference writes one reads in, deforms, and saves back in the same form.
    let file = root().join("crates/aztek_host/tests/data/mgear_curve_cns.ma");
    mel(&mut c, &format!("file -f -o \"{}\";", file.display().to_string().replace('\\', "/")));
    assert_eq!(nums(&mel(&mut c, "pointPosition crvShape.cv[1];")), vec![5.0, 0.0, 0.0]);
    mel(&mut c, "setAttr b.ty 4;");
    assert_eq!(nums(&mel(&mut c, "pointPosition crvShape.cv[1];")), vec![5.0, 4.0, 0.0]);
    assert_eq!(mel(&mut c, "objExists crvShapeOrig;"), json!(false), "the deformer keeps the original curve");
    let out = std::env::temp_dir().join(format!("aztek-curve-cns-{}.ma", std::process::id()));
    let out_s = out.display().to_string().replace('\\', "/");
    mel(&mut c, &format!("file -rename \"{out_s}\"; file -save -type \"mayaAscii\";"));
    let text = std::fs::read_to_string(&out).unwrap();
    assert!(text.contains("connectAttr \"mgear_curveCns1.og[0]\" \"crvShape.cr\";") && text.contains("crvShapeOrig.ws\" \"mgear_curveCns1.ip[0].ig\";"), "{text}");
    mel(&mut c, &format!("file -f -o \"{out_s}\";"));
    mel(&mut c, "setAttr b.ty 7;");
    assert_eq!(nums(&mel(&mut c, "pointPosition crvShape.cv[1];")), vec![5.0, 7.0, 0.0]);
    let _ = std::fs::remove_file(&out);

    // The source's status checks spell the code MStatus::kSuccess: a spring's goal reads, and the spring follows it.
    mel(&mut c, "file -f -new; createNode transform -n goalT; createNode mgear_springNode -n spr; connectAttr goalT.t spr.goal; setAttr goalT.t 1 2 3;");
    assert_eq!(nums(&mel(&mut c, "getAttr spr.output;")), vec![1.0, 2.0, 3.0]);
}

fn save_and_open(c: &mut Core, name: &str) {
    let out = std::env::temp_dir().join(format!("aztek-{name}-{}.ma", std::process::id()));
    let out_s = out.display().to_string().replace('\\', "/");
    mel(c, &format!("file -rename \"{out_s}\"; file -save -type \"mayaAscii\";"));
    mel(c, "file -f -new;");
    mel(c, &format!("file -f -o \"{out_s}\";"));
    let _ = std::fs::remove_file(&out);
}

#[test]
fn mgear_weight_driver_ships_and_works() {
    let Some(dir) = build() else {
        eprintln!("no CMake or C++ compiler: the shipped plug-ins not tested");
        return;
    };
    std::env::set_var("AZTEK_PLUG_IN_PATH", &dir);
    let mut c = Core::new();
    assert_eq!(mel(&mut c, "loadPlugin mGearWeightDriver;"), json!(["mGearWeightDriver"]));

    // A locator shape under its own transform, as the reference makes one.
    mel(&mut c, "createNode mGearWeightDriver;");
    assert_eq!(mel(&mut c, "listRelatives -p mGearWeightDriverShape1;"), json!(["mGearWeightDriver1"]));

    // Vector angle: the weight falls off with the driver's angle from the reader's axis, by the ramp the node made.
    mel(&mut c, "createNode transform -n piv; createNode transform -n drv -p piv; setAttr drv.tx 1;");
    mel(&mut c, "connectAttr drv.worldMatrix[0] mGearWeightDriverShape1.driverMatrix;");
    for (rz, want) in [(0.0, 1.0), (10.0, 0.777778), (22.5, 0.5), (45.0, 0.0)] {
        mel(&mut c, &format!("setAttr piv.rz {rz};"));
        let got = nums(&mel(&mut c, "getAttr mGearWeightDriverShape1.outWeight;"))[0];
        assert!((got - want).abs() < 1e-5, "{rz}: {got}");
    }

    // RBF in matrix mode: the compute reads its driver's parent and joint orient from the scene, and each output is
    // a pose's weight.
    mel(&mut c, "file -f -new; createNode mGearWeightDriver -n wd; createNode joint -n par; createNode joint -n j -p par; setAttr j.tx 2;");
    mel(&mut c, "setAttr wd.type 1; connectAttr j.matrix wd.driverList[0].driverInput;");
    for (k, rz) in [(0, 0), (1, 90)] {
        mel(&mut c, &format!("setAttr j.rz {rz};"));
        let m: Vec<String> = nums(&mel(&mut c, "getAttr j.matrix;")).iter().map(f64::to_string).collect();
        mel(&mut c, &format!("setAttr wd.driverList[0].pose[{k}].poseMatrix -type \"matrix\" {};", m.join(" ")));
        mel(&mut c, &format!("setAttr wd.driverList[0].pose[{k}].poseParentMatrix -type \"matrix\" 1 0 0 0 0 1 0 0 0 0 1 0 0 0 0 1;"));
        mel(&mut c, &format!("setAttr wd.driverList[0].pose[{k}].controlPoseValues -type \"doubleArray\" 1 {k};"));
    }
    mel(&mut c, "setAttr wd.evaluate 1;");
    let weight = |c: &mut Core, rz: f64| {
        mel(c, &format!("setAttr j.rz {rz};"));
        nums(&mel(c, "getAttr wd.output[0];"))[0]
    };
    assert!((weight(&mut c, 0.0) - 1.0).abs() < 1e-9);
    assert!(weight(&mut c, 90.0).abs() < 1e-9);
    let mid = weight(&mut c, 45.0);
    assert!(mid > 0.05 && mid < 0.95, "{mid}");

    // Saved by short names, nested arrays and array data among them, and read back the same.
    save_and_open(&mut c, "weight-driver");
    assert_eq!(nums(&mel(&mut c, "getAttr wd.driverList[0].pose[1].controlPoseValues;")), vec![1.0]);
    assert!((weight(&mut c, 45.0) - mid).abs() < 1e-9);
    assert!((weight(&mut c, 0.0) - 1.0).abs() < 1e-9);

    // Generic RBF: two inputs, three poses, two outputs that exist once connected.
    mel(&mut c, "file -f -new; createNode mGearWeightDriver -n g; setAttr g.type 1; createNode transform -n out;");
    for (k, (pin, pv)) in [((0, 0), (0, 0)), ((1, 0), (10, 0)), ((0, 1), (0, 5))].iter().enumerate() {
        mel(&mut c, &format!("setAttr g.poses[{k}].poseInput[0] {}; setAttr g.poses[{k}].poseInput[1] {};", pin.0, pin.1));
        mel(&mut c, &format!("setAttr g.poses[{k}].poseValue[0] {}; setAttr g.poses[{k}].poseValue[1] {};", pv.0, pv.1));
    }
    mel(&mut c, "connectAttr g.output[0] out.tx; connectAttr g.output[1] out.ty; setAttr g.evaluate 1;");
    for ((x, y), want) in [((1, 0), [10.0, 0.0]), ((0, 1), [0.0, 5.0]), ((0, 0), [0.0, 0.0])] {
        mel(&mut c, &format!("setAttr g.input[0] {x}; setAttr g.input[1] {y};"));
        let got = [nums(&mel(&mut c, "getAttr out.tx;"))[0], nums(&mel(&mut c, "getAttr out.ty;"))[0]];
        assert!(near(&got, &want, 1e-6), "{x} {y}: {got:?}");
    }
}

#[test]
fn cvwrap_ships_and_works() {
    let Some(dir) = build() else {
        eprintln!("no CMake or C++ compiler: the shipped plug-ins not tested");
        return;
    };
    std::env::set_var("AZTEK_PLUG_IN_PATH", &dir);
    let mut c = Core::new();
    assert_eq!(mel(&mut c, "loadPlugin cvwrap;"), json!(["cvwrap"]));
    mel(&mut c, "polyCube -w 2 -h 2 -d 2 -n driver; polySphere -r 0.9 -sx 8 -sy 6 -n driven;");
    // The driven first, the driver last; the name's # numbered as the reference numbers it.
    assert_eq!(mel(&mut c, "cvWrap -r 0.5 driven driver;"), json!("cvWrap1"));
    assert_eq!(mel(&mut c, "ls \"cvWrap1Base\";"), json!(["cvWrap1Base"]));
    assert_eq!(mel(&mut c, "getAttr cvWrap1Base.visibility;"), json!(false));
    let bottom = |c: &mut Core| nums(&mel(c, "pointPosition driven.vtx[0];"));
    assert!(near(&bottom(&mut c), &[0.0, -0.9, 0.0], 1e-6));

    // The driver read in world space: moving its transform carries the wrapped mesh, scaling it keeps the offset.
    mel(&mut c, "move -r 0 1 0 driver;");
    assert!(near(&bottom(&mut c), &[0.0, 0.1, 0.0], 1e-6), "{:?}", bottom(&mut c));
    mel(&mut c, "move -r 0 -1 0 driver; scale 2 2 2 driver;");
    assert!(near(&bottom(&mut c), &[0.0, -1.9, 0.0], 1e-6), "{:?}", bottom(&mut c));

    // Saved in the reference's layout and read back still wrapping.
    save_and_open(&mut c, "cvwrap");
    assert!(near(&bottom(&mut c), &[0.0, -1.9, 0.0], 1e-6), "{:?}", bottom(&mut c));
    assert_eq!(mel(&mut c, "listConnections -p true cvWrap1.driver;"), json!(["driverShape.worldMesh[0]"]));
    mel(&mut c, "scale 1 1 1 driver;");
    assert!(near(&bottom(&mut c), &[0.0, -0.9, 0.0], 1e-6), "{:?}", bottom(&mut c));

    // Rebinding to some of the driver's faces: the bind mesh is copied and the other faces deleted off the copy.
    mel(&mut c, "select driver.f[3]; cvWrap -rb cvWrap1;");
    mel(&mut c, "move -r 0 1 0 driver;");
    assert!(near(&bottom(&mut c), &[0.0, 0.1, 0.0], 1e-6), "{:?}", bottom(&mut c));
}
