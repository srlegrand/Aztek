//! The plug-ins that ship with Aztek (plugins/), built with CMake against devkit/ as the program's build builds them,
//! and loaded by name as a scene that requires them loads them. Without CMake and a C++ compiler it says so and passes.

use std::path::{Path, PathBuf};
use std::process::Command;

use aztek_host::Core;
use serde_json::{json, Value};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn build() -> Option<PathBuf> {
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
}
