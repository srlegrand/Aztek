//! NURBS surfaces as the reference's own echoes spell them.

use aztek_host::Core;
use serde_json::json;

fn errors(c: &mut Core) -> Vec<String> {
    c.take_log().as_array().unwrap().iter().filter(|l| l["kind"] == 2).map(|l| l["text"].as_str().unwrap().to_string()).collect()
}

#[test]
fn the_reference_echoes_run() {
    let mut c = Core::new();
    let r = c.mel("sphere -p 0 0 0 -ax 0 1 0 -ssw 0 -esw 360 -r 1 -d 3 -ut 0 -tol 0.01 -s 8 -nsp 4 -ch 1;");
    assert_eq!(r, json!(["nurbsSphere1", "makeNurbSphere1"]));
    c.mel("nurbsCube -p 0 0 0 -ax 0 1 0 -w 1 -lr 1 -hr 1 -d 3 -u 1 -v 1 -ch 1;");
    c.mel("cylinder -p 0 0 0 -ax 0 1 0 -ssw 0 -esw 360 -r 1 -hr 2 -d 3 -ut 0 -tol 0.01 -s 8 -nsp 1 -ch 1;");
    c.mel("cone -p 0 0 0 -ax 0 1 0 -ssw 0 -esw 360 -r 1 -hr 2 -d 3 -ut 0 -tol 0.01 -s 8 -nsp 1 -ch 1;");
    c.mel("torus -p 0 0 0 -ax 0 1 0 -ssw 0 -esw 360 -msw 360 -r 1 -hr 0.5 -d 3 -ut 0 -tol 0.01 -s 8 -nsp 4 -ch 1;");
    c.mel("nurbsPlane -p 0 0 0 -ax 0 1 0 -w 1 -lr 1 -d 3 -u 1 -v 1 -ch 1;");
    assert_eq!(errors(&mut c), Vec::<String>::new());

    c.mel("select -r nurbsSphere1.cv[1][2];");
    assert_eq!(errors(&mut c), Vec::<String>::new());
    assert_eq!(c.s.run("ls", json!({"selection": true})).unwrap(), json!(["nurbsSphere1.cv[1][2]"]));
    let before = c.s.run("xform", json!({"args": ["nurbsSphere1.cv[1][2]"], "query": true, "worldSpace": true, "translation": true})).unwrap();
    c.mel("move -r 0 2 0 nurbsSphere1.cv[1][2];");
    assert_eq!(errors(&mut c), Vec::<String>::new());
    let after = c.s.run("xform", json!({"args": ["nurbsSphere1.cv[1][2]"], "query": true, "worldSpace": true, "translation": true})).unwrap();
    assert!((after[1].as_f64().unwrap() - before[1].as_f64().unwrap() - 2.0).abs() < 1e-9, "{before} {after}");
    c.mel("select -r nurbsSphere1.cv[0:2][*];");
    assert_eq!(errors(&mut c), Vec::<String>::new());
    assert_eq!(c.s.run("ls", json!({"selection": true})).unwrap(), json!(["nurbsSphere1.cv[0:2][0:7]"]));

    c.mel("select -r nurbsSphere1; selectType -cv true;");
    assert_eq!(errors(&mut c), Vec::<String>::new());
    assert_eq!(c.s.run("selectMode", json!({})).unwrap(), json!("vertex"));
    assert_eq!(c.mel("selectType -q -cv;"), json!(true));
    assert_eq!(c.mel("selectType -q -polymeshFace;"), json!(false));
    assert_eq!(errors(&mut c), Vec::<String>::new());
    c.s.run("selectMode", json!({"args": ["object"]})).unwrap();

    let r = c.mel("nurbsToPoly -mnd 1 -ch 1 -f 2 -pt 1 -pc 200 -chr 0.9 -ft 0.01 -mel 0.001 -d 0.1 -ut 3 -un 3 -vt 3 -vn 3 -uch 0 -ucr 0 -cht 0.2 -es 0 -ntr 0 -mrt 0 -uss 1 nurbsSphere1;");
    assert_eq!(r, json!(["nurbsToPoly1", "nurbsTessellate1"]));
    assert_eq!(errors(&mut c), Vec::<String>::new());
    assert_eq!(c.s.run("meshInfo", json!({"args": ["nurbsToPoly1"]})).unwrap()["faces"], json!(288));

    c.mel("rebuildSurface -ch 1 -rpo 1 -rt 0 -end 1 -kr 0 -kcp 0 -kc 0 -su 4 -du 3 -sv 4 -dv 3 -tol 0.01 -fr 0 -dir 2 nurbsPlane1;");
    c.mel("reverseSurface -d 3 -ch 1 -rpo 1 nurbsPlane1;");
    c.mel("insertKnotSurface -ch 1 -d 1 -p 0.25 -nk 1 -add 1 -ib 0 -rpo 1 nurbsPlane1;");
    c.mel("offsetSurface -d 1 -m 0 -ch 1 -rpo 1 nurbsPlane1;");
    assert_eq!(errors(&mut c), Vec::<String>::new());

    c.mel("curve -d 1 -p 0 0 0 -p 1 0 0 -n c1; curve -d 1 -p 0 1 0 -p 1 1 0 -n c2;");
    let r = c.mel("loft -ch 1 -u 1 -c 0 -ar 1 -d 3 -ss 1 -rn 0 -po 0 -rsn 1 c1 c2;");
    assert_eq!(r, json!(["loftedSurface1", "loft1"]));
    let r = c.mel("revolve -ch 1 -po 0 -rn 0 -ssw 0 -esw 360 -ut 0 -tol 0.01 -d 3 -s 8 -ax 0 1 0 -p 0 0 0 c1;");
    assert_eq!(r, json!(["revolvedSurface1", "revolve1"]));
    let r = c.mel("extrude -ch 1 -rn 0 -po 0 -et 2 -ucp 1 -fpt 1 -upn 1 -rotation 0 -scale 1 -rsp 1 c1 c2;");
    assert_eq!(r, json!(["extrudedSurface1", "extrude1"]));
    let r = c.mel("loft -ch 1 -po 1 c1 c2;");
    assert_eq!(r[1], json!("loft2"));
    assert_eq!(errors(&mut c), Vec::<String>::new());
    let r = c.mel("duplicateCurve -ch 1 -rn 0 -local 0 \"loftedSurface1.u[0.5]\";");
    assert_eq!(r[1], json!("curveFromSurfaceIso1"));
    assert_eq!(errors(&mut c), Vec::<String>::new());
    let p = c.mel("pointOnSurface -u 0.5 -v 0.5 -p loftedSurface1;");
    assert_eq!(errors(&mut c), Vec::<String>::new());
    assert!((p[1].as_f64().unwrap() - 0.5).abs() < 1e-9, "{p}");

    c.mel("undo;");
    let _ = errors(&mut c);
}
