//! Duplicate of an object whose channels are driven (#190): the copy takes the values they have now, with no
//! connection, as the reference's does.

use aztek_host::Core;
use std::path::PathBuf;

fn num(c: &mut Core, mel: &str) -> f64 {
    let v = c.mel_checked(mel).unwrap_or_else(|e| panic!("{mel}: {e}"));
    format!("{v:?}").trim_start_matches("Number(").trim_end_matches(')').parse().unwrap_or_else(|_| panic!("{mel}: {v:?}"))
}

#[test]
fn a_brick_moved_by_its_rigid_body_is_copied_where_it_is() {
    let scene = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/dynamics/rigid_wall_breaking.ma");
    let mut c = Core::new();
    c.mel_checked(&format!("file -o -f \"{}\";", scene.display().to_string().replace('\\', "/"))).unwrap();
    c.mel_checked("currentTime 20;").unwrap();
    let (x, y, rz) = (num(&mut c, "getAttr brick_0_0.tx"), num(&mut c, "getAttr brick_0_0.ty"), num(&mut c, "getAttr brick_0_0.rz"));
    c.mel_checked("select -r brick_0_0; string $d[] = `duplicate -rr`; global string $copy; $copy = $d[0];").unwrap();
    for (attr, want) in [("tx", x), ("ty", y), ("rz", rz)] {
        let got = num(&mut c, &format!("global string $copy; getAttr ($copy + \".{attr}\")"));
        assert!((got - want).abs() < 1e-6, "{attr}: {got} vs {want}");
    }
    let inputs = c.mel_checked("global string $copy; size(`listConnections -s 1 -d 0 $copy`)").unwrap();
    assert_eq!(format!("{inputs:?}"), "Number(0)");
}
