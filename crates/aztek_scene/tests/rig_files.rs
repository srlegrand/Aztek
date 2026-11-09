//! Wiring real rigs use, read from .ma text laid out as the reference writes it: constraints to pivots and with driven weights, follicles,
//! deformed curves and surfaces, mirrored setups. Each of these was found wrong opening a production rig.

use aztek_scene::ma_file::read_text;
use aztek_scene::Session;
use serde_json::{json, Value};

const HEAD: &str = "//Maya ASCII 2020 scene\nrequires maya \"2020\";\ncurrentUnit -l centimeter -a degree -t film;\n";

fn load(body: &str) -> (Session, Vec<String>) {
    let mut s = Session::new();
    let w = read_text(&mut s, &format!("{HEAD}{body}"), false);
    s.graph.evaluate();
    (s, w)
}

fn get(s: &mut Session, plug: &str) -> Vec<f64> {
    match s.run("getAttr", json!({"args": [plug]})).unwrap_or_else(|e| panic!("{plug}: {e}")) {
        Value::Array(a) => a.iter().map(|x| x.as_f64().unwrap()).collect(),
        v => vec![v.as_f64().unwrap_or_else(|| panic!("{plug}: {v}"))],
    }
}

fn near(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-3)
}

fn assert_near(got: Vec<f64>, want: &[f64], what: &str) {
    assert!(near(&got, want), "{what}: got {got:?}, want {want:?}");
}

/// The fourth column of a world matrix.
fn world_at(s: &mut Session, node: &str) -> Vec<f64> {
    get(s, &format!("{node}.worldMatrix"))[12..15].to_vec()
}

fn constraint(name: &str, parent: &str, target: &str, alias: &str, driven_outputs: bool) -> String {
    let mut t = format!(
        "createNode parentConstraint -n \"{name}\" -p \"{parent}\";\n\taddAttr -dcb 0 -ci true -k true -sn \"w0\" -ln \"{alias}\" -dv 1 -min 0 -at \"double\";\n\tsetAttr \".erp\" yes;\n"
    );
    t.push_str(&format!("connectAttr \"{target}.t\" \"{name}.tg[0].tt\";\nconnectAttr \"{target}.rp\" \"{name}.tg[0].trp\";\nconnectAttr \"{target}.pm\" \"{name}.tg[0].tpm\";\nconnectAttr \"{name}.w0\" \"{name}.tg[0].tw\";\n"));
    if driven_outputs {
        t.push_str(&format!("connectAttr \"{name}.ctx\" \"{parent}.tx\";\nconnectAttr \"{name}.cty\" \"{parent}.ty\";\nconnectAttr \"{name}.ctz\" \"{parent}.tz\";\nconnectAttr \"{name}.crx\" \"{parent}.rx\";\nconnectAttr \"{name}.cry\" \"{parent}.ry\";\nconnectAttr \"{name}.crz\" \"{parent}.rz\";\n"));
    }
    t
}

#[test]
fn constraints_go_to_the_targets_rotate_pivot() {
    let body = format!(
        "createNode transform -n \"tgt\";\n\tsetAttr \".rp\" -type \"double3\" 1 2 3 ;\n\tsetAttr \".sp\" -type \"double3\" 1 2 3 ;\ncreateNode transform -n \"obj\";\n{}",
        constraint("obj_parentConstraint1", "obj", "tgt", "tgtW0", true)
    );
    let (mut s, w) = load(&body);
    assert!(w.is_empty(), "{w:?}");
    assert_near(get(&mut s, "obj.translate"), &[1.0, 2.0, 3.0], "at the pivot");
}

#[test]
fn a_set_driven_key_on_a_target_weight_is_kept() {
    let body = format!(
        "createNode transform -n \"a\";\n\tsetAttr \".t\" -type \"double3\" 5 0 0 ;\ncreateNode transform -n \"sw\";\n\taddAttr -ci true -sn \"Follow\" -ln \"Follow\" -at \"double\";\n\tsetAttr -k on \".Follow\" 0;\ncreateNode transform -n \"obj\";\n\tsetAttr \".t\" -type \"double3\" 0 7 0 ;\n{}\tsetAttr \".rst\" -type \"double3\" 0 7 0 ;\ncreateNode animCurveUU -n \"w_curve\";\n\tsetAttr -s 2 \".ktv[0:1]\"  0 0 1 1;\nconnectAttr \"sw.Follow\" \"w_curve.i\";\nconnectAttr \"w_curve.o\" \"obj_parentConstraint1.w0\";\n",
        constraint("obj_parentConstraint1", "obj", "a", "aW0", true)
    );
    let (mut s, _) = load(&body);
    assert_near(get(&mut s, "obj.translate"), &[0.0, 7.0, 0.0], "weight 0: at rest");
    s.run("setAttr", json!({"args": ["sw.Follow", 1.0]})).unwrap();
    assert_near(get(&mut s, "obj.translate"), &[5.0, 0.0, 0.0], "weight 1: on the target");
}

#[test]
fn a_constraint_stays_on_its_target_under_a_mirrored_parent() {
    // Rotation part of mirror X is 180 about X with scale -1 on every axis, which is how the offset was worked out.
    let body = format!(
        "createNode transform -n \"mirror\";\n\tsetAttr \".s\" -type \"double3\" -1 1 1 ;\ncreateNode transform -n \"tgt\";\ncreateNode transform -n \"grp\" -p \"mirror\";\n{}\tsetAttr \".tg[0].tor\" -type \"double3\" 180 0 0 ;\n",
        constraint("grp_parentConstraint1", "grp", "tgt", "tgtW0", true)
    );
    let (mut s, _) = load(&body);
    assert_near(get(&mut s, "grp.rotate"), &[0.0, 0.0, 0.0], "rest under a mirror");
}

#[test]
fn aim_constraints_work_in_the_parents_frame() {
    // The same setup with and without a mirror gives the same rotate: the mirror cancels out.
    let aim = |parent: &str| {
        format!(
            "createNode transform -n \"holder\";\n\tsetAttr \".s\" -type \"double3\" {parent} ;\ncreateNode transform -n \"tgt\" -p \"holder\";\n\tsetAttr \".t\" -type \"double3\" 0 0 5 ;\ncreateNode transform -n \"obj\" -p \"holder\";\ncreateNode aimConstraint -n \"obj_aimConstraint1\" -p \"obj\";\n\taddAttr -dcb 0 -ci true -sn \"w0\" -ln \"tgtW0\" -dv 1 -at \"double\";\n\tsetAttr \".a\" -type \"double3\" 1 0 0 ;\n\tsetAttr \".u\" -type \"double3\" 0 1 0 ;\n\tsetAttr \".wu\" -type \"double3\" 0 1 0 ;\n\tsetAttr \".wut\" 3;\nconnectAttr \"tgt.t\" \"obj_aimConstraint1.tg[0].tt\";\nconnectAttr \"tgt.pm\" \"obj_aimConstraint1.tg[0].tpm\";\nconnectAttr \"obj_aimConstraint1.w0\" \"obj_aimConstraint1.tg[0].tw\";\nconnectAttr \"holder.pim\" \"obj_aimConstraint1.cpim\";\nconnectAttr \"obj_aimConstraint1.crx\" \"obj.rx\";\nconnectAttr \"obj_aimConstraint1.cry\" \"obj.ry\";\nconnectAttr \"obj_aimConstraint1.crz\" \"obj.rz\";\n"
        )
    };
    let (mut plain, _) = load(&aim("1 1 1"));
    let (mut mirrored, _) = load(&aim("-1 1 1"));
    let a = get(&mut plain, "obj.rotate");
    assert_near(get(&mut mirrored, "obj.rotate"), &a, "same under a mirror");
    assert!((a[1] + 90.0).abs() < 1e-3, "aims along Z: {a:?}");
}

#[test]
fn a_world_up_object_is_connected() {
    let body = "createNode transform -n \"tgt\";\n\tsetAttr \".t\" -type \"double3\" 5 0 0 ;\ncreateNode transform -n \"up\";\n\tsetAttr \".r\" -type \"double3\" 90 0 0 ;\ncreateNode transform -n \"obj\";\ncreateNode aimConstraint -n \"obj_aimConstraint1\" -p \"obj\";\n\taddAttr -dcb 0 -ci true -sn \"w0\" -ln \"tgtW0\" -dv 1 -at \"double\";\n\tsetAttr \".a\" -type \"double3\" 1 0 0 ;\n\tsetAttr \".u\" -type \"double3\" 0 1 0 ;\n\tsetAttr \".wu\" -type \"double3\" 0 1 0 ;\n\tsetAttr \".wut\" 2;\nconnectAttr \"tgt.t\" \"obj_aimConstraint1.tg[0].tt\";\nconnectAttr \"obj_aimConstraint1.w0\" \"obj_aimConstraint1.tg[0].tw\";\nconnectAttr \"up.wm\" \"obj_aimConstraint1.wum\";\nconnectAttr \"obj_aimConstraint1.crx\" \"obj.rx\";\nconnectAttr \"obj_aimConstraint1.cry\" \"obj.ry\";\nconnectAttr \"obj_aimConstraint1.crz\" \"obj.rz\";\n";
    let (mut s, _) = load(body);
    assert_near(get(&mut s, "obj.rotate"), &[90.0, 0.0, 0.0], "up follows the object's rotation");
}

const PLANE: &str = "createNode transform -n \"sheet\";\ncreateNode nurbsSurface -n \"sheetShape\" -p \"sheet\";\ncreateNode makeNurbPlane -n \"makeNurbPlane1\";\n\tsetAttr \".ax\" -type \"double3\" 0 1 0 ;\n\tsetAttr \".w\" 2;\nconnectAttr \"makeNurbPlane1.os\" \"sheetShape.cr\";\n";

#[test]
fn a_follicle_drives_its_own_transform_with_u_v_and_the_normal_as_axes() {
    let body = format!(
        "{PLANE}createNode transform -n \"fol\";\n\tsetAttr \".it\" no;\ncreateNode follicle -n \"folShape\" -p \"fol\";\n\tsetAttr \".pu\" 0.75;\n\tsetAttr \".pv\" 0.5;\nconnectAttr \"sheetShape.l\" \"folShape.is\";\nconnectAttr \"sheetShape.wm\" \"folShape.iwm\";\nconnectAttr \"folShape.ot\" \"fol.t\";\nconnectAttr \"folShape.or\" \"fol.r\";\n"
    );
    let (mut s, w) = load(&body);
    assert!(w.is_empty(), "{w:?}");
    assert_near(get(&mut s, "fol.translate"), &[0.5, 0.0, 0.0], "at the UV");
    let m = get(&mut s, "fol.worldMatrix");
    assert_near(m[0..3].to_vec(), &[1.0, 0.0, 0.0], "X along U");
    assert_near(m[8..11].to_vec(), &[0.0, 1.0, 0.0], "Z the normal");
    s.run("setAttr", json!({"args": ["sheet.ty", 4.0]})).unwrap();
    assert_near(get(&mut s, "fol.translate"), &[0.5, 4.0, 0.0], "follows the surface");
}

#[test]
fn a_skinned_curve_follows_its_joint_through_the_deformer_chain() {
    let cv = "\t\t1 3 0 no 3\n\t\t4 0 1 2 3\n\t\t4\n\t\t0 0 0\n\t\t1 0 0\n\t\t2 0 0\n\t\t3 0 0\n\t\t;\n";
    let body = format!(
        "createNode joint -n \"j\";\ncreateNode transform -n \"c\";\ncreateNode nurbsCurve -n \"cShape\" -p \"c\";\ncreateNode nurbsCurve -n \"cShapeOrig\" -p \"c\";\n\tsetAttr \".io\" yes;\n\tsetAttr \".cc\" -type \"nurbsCurve\" \n{cv}createNode skinCluster -n \"sk\";\n\tsetAttr -s 4 \".wl\";\n\tsetAttr \".wl[0:3].w\"\n\t\t1 0 1\n\t\t1 0 1\n\t\t1 0 1\n\t\t1 0 1;\n\tsetAttr \".pm[0]\" -type \"matrix\" 1 0 0 0 0 1 0 0 0 0 1 0 0 0 0 1;\n\tsetAttr \".gm\" -type \"matrix\" 1 0 0 0 0 1 0 0 0 0 1 0 0 0 0 1;\n\tsetAttr \".mi\" 1;\ncreateNode tweak -n \"tw\";\n\tsetAttr -s 1 \".pl[0].cp[1]\" -type \"double3\" 0 0.5 0;\nconnectAttr \"j.wm\" \"sk.ma[0]\";\nconnectAttr \"cShapeOrig.ws\" \"tw.ip[0].ig\";\nconnectAttr \"tw.og[0]\" \"sk.ip[0].ig\";\nconnectAttr \"sk.og[0]\" \"cShape.cr\";\ncreateNode pointOnCurveInfo -n \"poc\";\n\tsetAttr \".pr\" 1;\ncreateNode transform -n \"loc\";\nconnectAttr \"cShape.ws\" \"poc.ic\";\nconnectAttr \"poc.p\" \"loc.t\";\n"
    );
    let (mut s, w) = load(&body);
    assert!(w.iter().all(|x| !x.contains("cShape")), "{w:?}");
    assert_near(get(&mut s, "loc.translate"), &[1.0, 0.5, 0.0], "the tweak applies");
    s.run("setAttr", json!({"args": ["j.ty", 2.0]})).unwrap();
    assert_near(get(&mut s, "loc.translate"), &[1.0, 2.5, 0.0], "follows the joint");
}

#[test]
fn something_can_drive_a_curves_control_points() {
    let cv = "\t\t1 1 0 no 3\n\t\t2 0 1\n\t\t2\n\t\t0 0 0\n\t\t1 0 0\n\t\t;\n";
    let body = format!(
        "createNode transform -n \"c\";\ncreateNode nurbsCurve -n \"cShape\" -p \"c\";\n\tsetAttr \".cc\" -type \"nurbsCurve\" \n{cv}createNode transform -n \"l\";\n\tsetAttr \".t\" -type \"double3\" 3 3 3 ;\ncreateNode locator -n \"lShape\" -p \"l\";\nconnectAttr \"lShape.wp\" \"cShape.cp[1]\";\ncreateNode pointOnCurveInfo -n \"poc\";\n\tsetAttr \".pr\" 1;\ncreateNode transform -n \"at\";\nconnectAttr \"cShape.ws\" \"poc.ic\";\nconnectAttr \"poc.p\" \"at.t\";\n"
    );
    let (mut s, _) = load(&body);
    assert_near(get(&mut s, "at.translate"), &[3.0, 3.0, 3.0], "the CV goes where the locator is");
}

#[test]
fn a_transform_geometry_moves_a_curve_with_no_stored_points() {
    let body = "createNode transform -n \"c\";\ncreateNode nurbsCurve -n \"cShape\" -p \"c\";\ncreateNode makeNurbCircle -n \"mc\";\n\tsetAttr \".nr\" -type \"double3\" 0 1 0 ;\ncreateNode transformGeometry -n \"tg\";\n\tsetAttr \".txf\" -type \"matrix\" 2 0 0 0 0 2 0 0 0 0 2 0 0 5 0 1;\nconnectAttr \"mc.oc\" \"tg.ig\";\nconnectAttr \"tg.og\" \"cShape.cr\";\ncreateNode curveInfo -n \"ci\";\nconnectAttr \"cShape.ws\" \"ci.ic\";\n";
    let (mut s, w) = load(body);
    assert!(w.iter().all(|x| !x.contains("no stored")), "{w:?}");
    let len = get(&mut s, "ci.arcLength")[0];
    assert!((len - 4.0 * std::f64::consts::PI).abs() < 0.2, "radius 2: {len}");
}

const LEG: &str = "createNode joint -n \"a\" -p \"mirror\";\n\tsetAttr \".t\" -type \"double3\" 1 0 0 ;\n\tsetAttr \".jo\" -type \"double3\" 0 0 -90 ;\ncreateNode joint -n \"b\" -p \"a\";\n\tsetAttr \".t\" -type \"double3\" 2 0 0 ;\n\tsetAttr \".pa\" -type \"double3\" 0 0 0 ;\n\tsetAttr \".jo\" -type \"double3\" 0 -20 0 ;\ncreateNode joint -n \"c\" -p \"b\";\n\tsetAttr \".t\" -type \"double3\" 2 0 0 ;\ncreateNode ikEffector -n \"eff\" -p \"b\";\ncreateNode ikRPsolver -n \"ikRPsolver\";\nconnectAttr \"c.tx\" \"eff.tx\";\nconnectAttr \"a.msg\" \"h.hsj\";\nconnectAttr \"eff.hp\" \"h.hee\";\nconnectAttr \"ikRPsolver.msg\" \"h.hsv\";\n";

#[test]
fn a_handle_with_no_stored_translate_sits_at_its_parent() {
    let body = format!(
        "createNode transform -n \"mirror\";\ncreateNode transform -n \"ctl\";\n\tsetAttr \".t\" -type \"double3\" 1 -3.9 0.5 ;\ncreateNode ikHandle -n \"h\" -p \"ctl\";\n{LEG}"
    );
    let (mut s, w) = load(&body);
    assert!(w.is_empty(), "{w:?}");
    assert_near(world_at(&mut s, "h"), &[1.0, -3.9, 0.5], "a handle's translate left out of the file is zero");
}

#[test]
fn a_chain_under_a_mirror_matches_its_unmirrored_twin() {
    // The pole vector is held in the start joint's parent space, so a mirrored parent leaves the local pose alone.
    let rig = |scale: &str, x: f64| {
        format!(
            "createNode transform -n \"mirror\";\n\tsetAttr \".s\" -type \"double3\" {scale} ;\ncreateNode transform -n \"pole\";\n\tsetAttr \".t\" -type \"double3\" {} -2 5 ;\ncreateNode transform -n \"ctl\";\n\tsetAttr \".t\" -type \"double3\" {} -3.5 1 ;\ncreateNode ikHandle -n \"h\" -p \"ctl\";\ncreateNode poleVectorConstraint -n \"pv1\" -p \"h\";\n\taddAttr -dcb 0 -ci true -k true -sn \"w0\" -ln \"poleW0\" -dv 1 -min 0 -at \"double\";\nconnectAttr \"pole.t\" \"pv1.tg[0].tt\";\nconnectAttr \"pv1.w0\" \"pv1.tg[0].tw\";\nconnectAttr \"pv1.ctx\" \"h.pvx\";\nconnectAttr \"pv1.cty\" \"h.pvy\";\nconnectAttr \"pv1.ctz\" \"h.pvz\";\n{LEG}",
            x * 1.0,
            x * 1.0
        )
    };
    let (mut l, _) = load(&rig("1 1 1", 1.0));
    let (mut r, _) = load(&rig("-1 1 1", -1.0));
    assert_near(get(&mut r, "h.poleVector"), &get(&mut l, "h.poleVector"), "same in the parent's space");
    assert_near(get(&mut r, "a.rotate"), &get(&mut l, "a.rotate"), "start joint");
    assert_near(get(&mut r, "b.rotate"), &get(&mut l, "b.rotate"), "middle joint");
    let (lw, rw) = (world_at(&mut l, "c"), world_at(&mut r, "c"));
    assert_near(rw.clone(), &[-lw[0], lw[1], lw[2]], "the end is the mirror image");
}

#[test]
fn spline_ik_keeps_its_root_on_the_curve() {
    let cv = "\t\t3 1 0 no 3\n\t\t6 0 0 0 1 1 1\n\t\t4\n\t\t0 5 0\n\t\t0 6 0\n\t\t0 7 0\n\t\t0 8 0\n\t\t;\n";
    let body = format!(
        "createNode joint -n \"a\";\n\tsetAttr \".jo\" -type \"double3\" 0 0 90 ;\ncreateNode joint -n \"b\" -p \"a\";\n\tsetAttr \".t\" -type \"double3\" 3 0 0 ;\ncreateNode ikEffector -n \"eff\" -p \"a\";\ncreateNode transform -n \"c\";\ncreateNode nurbsCurve -n \"cShape\" -p \"c\";\n\tsetAttr \".cc\" -type \"nurbsCurve\" \n{cv}createNode ikSplineSolver -n \"ikSplineSolver\";\ncreateNode ikHandle -n \"h\";\n\tsetAttr \".roc\" yes;\nconnectAttr \"b.tx\" \"eff.tx\";\nconnectAttr \"a.msg\" \"h.hsj\";\nconnectAttr \"eff.hp\" \"h.hee\";\nconnectAttr \"ikSplineSolver.msg\" \"h.hsv\";\nconnectAttr \"cShape.ws\" \"h.ic\";\n"
    );
    let (mut s, w) = load(&body);
    assert!(w.is_empty(), "{w:?}");
    assert_near(world_at(&mut s, "a"), &[0.0, 5.0, 0.0], "the root on the curve's start");
    // Anything that dirties the chain runs the sync, which must not lose the setting.
    s.run("setAttr", json!({"args": ["c.ty", 1.0]})).unwrap();
    assert_near(world_at(&mut s, "a"), &[0.0, 6.0, 0.0], "still on it after a sync");
}

#[test]
fn utility_names_are_the_ones_the_reference_writes() {
    let body = "createNode transform -n \"o\";\ncreateNode angleBetween -n \"ab\";\n\tsetAttr \".v2\" -type \"double3\" 0 1 0 ;\ncreateNode pairBlend -n \"pb\";\n\tsetAttr \".w\" 0;\n\tsetAttr \".itx1\" 4;\nconnectAttr \"pb.otx\" \"o.tx\";\nconnectAttr \"ab.a\" \"o.ry\";\ncreateNode transform -n \"sum\";\ncreateNode transform -n \"x\";\n\tsetAttr \".t\" -type \"double3\" 2 3 0 ;\ncreateNode blendWeighted -n \"bw\";\nconnectAttr \"x.tx\" \"bw.i[0]\";\nconnectAttr \"x.ty\" \"bw.i[1]\";\nconnectAttr \"bw.o\" \"sum.tx\";\n";
    let (mut s, w) = load(body);
    assert!(w.iter().all(|x| !x.contains("->")), "{w:?}");
    assert_near(get(&mut s, "o.rotateY"), &[90.0], "angleBetween.a is the angle");
    assert_near(get(&mut s, "o.translateX"), &[4.0], "pairBlend's numbered children");
    assert_near(get(&mut s, "sum.translateX"), &[5.0], "blendWeighted comes through");
}

#[test]
fn a_live_mesh_target_follows_what_deforms_it() {
    let cube = |n: &str| format!("createNode transform -n \"{n}\";\ncreateNode mesh -n \"{n}Shape\" -p \"{n}\";\ncreateNode polyPlane -n \"{n}Make\";\n\tsetAttr \".sw\" 1;\n\tsetAttr \".sh\" 1;\nconnectAttr \"{n}Make.out\" \"{n}Shape.i\";\n");
    let body = format!(
        "{}{}createNode blendShape -n \"bs\";\n\tsetAttr -s 1 \".w[0]\" 1;\nconnectAttr \"tgtShape.w\" \"bs.it[0].itg[0].iti[6000].igt\";\nconnectAttr \"bsParts.og\" \"bs.ip[0].ig\";\ncreateNode groupParts -n \"bsParts\";\nconnectAttr \"baseMake.out\" \"bsParts.ig\";\nconnectAttr \"bs.og[0]\" \"baseShape.i\";\n",
        cube("base"),
        cube("tgt")
    );
    let (mut s, w) = load(&body.replace("connectAttr \"baseMake.out\" \"baseShape.i\";\n", ""));
    assert!(w.iter().all(|x| !x.contains("nothing to read")), "{w:?}");
    let before = s.run("pointPosition", json!({"args": ["base.vtx[0]"]})).unwrap();
    s.run("move", json!({"args": [0.0, 2.0, 0.0, "tgt.vtx[0]"], "r": true})).unwrap();
    let after = s.run("pointPosition", json!({"args": ["base.vtx[0]"]})).unwrap();
    assert_near(vec![after[1].as_f64().unwrap() - before[1].as_f64().unwrap()], &[2.0], "the base follows the target");
}

#[test]
fn a_blend_weight_with_a_driver_follows_it_not_the_value_in_the_file() {
    let body = "createNode transform -n \"base\";\ncreateNode mesh -n \"baseShape\" -p \"base\";\ncreateNode polyPlane -n \"make\";\ncreateNode transform -n \"sw\";\n\tsetAttr \".tx\" 0.25;\ncreateNode blendShape -n \"bs\";\n\tsetAttr -s 1 \".w[0]\" 0.75;\ncreateNode groupParts -n \"gp\";\nconnectAttr \"make.out\" \"gp.ig\";\nconnectAttr \"gp.og\" \"bs.ip[0].ig\";\nconnectAttr \"bs.og[0]\" \"baseShape.i\";\nconnectAttr \"sw.tx\" \"bs.w[0]\";\n";
    let (mut s, _) = load(body);
    assert_near(get(&mut s, "bs.weight[0]"), &[0.25], "the driver");
    s.run("setAttr", json!({"args": ["sw.tx", 0.5]})).unwrap();
    assert_near(get(&mut s, "bs.weight[0]"), &[0.5], "and it keeps following");
}

/// What matters of a rig after saving: it still moves the same.
fn after_saving(body: &str) -> Session {
    let (mut s, _) = load(body);
    let (text, _) = aztek_scene::ma_file::write_text(&mut s, "rig.ma", None);
    let mut again = Session::new();
    let w = read_text(&mut again, &text, false);
    assert!(w.is_empty(), "{w:?}\n{text}");
    again.graph.evaluate();
    again
}

#[test]
fn spline_ik_survives_saving() {
    let cv = "\t\t3 1 0 no 3\n\t\t6 0 0 0 1 1 1\n\t\t4\n\t\t0 5 0\n\t\t0 6 0\n\t\t0 7 0\n\t\t0 8 0\n\t\t;\n";
    let body = format!(
        "createNode joint -n \"a\";\n\tsetAttr \".jo\" -type \"double3\" 0 0 90 ;\ncreateNode joint -n \"b\" -p \"a\";\n\tsetAttr \".t\" -type \"double3\" 3 0 0 ;\ncreateNode ikEffector -n \"eff\" -p \"a\";\ncreateNode transform -n \"c\";\ncreateNode nurbsCurve -n \"cShape\" -p \"c\";\n\tsetAttr \".cc\" -type \"nurbsCurve\" \n{cv}createNode ikSplineSolver -n \"ikSplineSolver\";\ncreateNode ikHandle -n \"h\";\n\tsetAttr \".roc\" yes;\nconnectAttr \"b.tx\" \"eff.tx\";\nconnectAttr \"a.msg\" \"h.hsj\";\nconnectAttr \"eff.hp\" \"h.hee\";\nconnectAttr \"ikSplineSolver.msg\" \"h.hsv\";\nconnectAttr \"cShape.ws\" \"h.ic\";\n"
    );
    let mut s = after_saving(&body);
    s.run("setAttr", json!({"args": ["c.ty", 1.0]})).unwrap();
    assert_near(world_at(&mut s, "a"), &[0.0, 6.0, 0.0], "still follows the curve");
}

#[test]
fn a_skinned_curve_and_its_tweak_survive_saving() {
    let cv = "\t\t1 3 0 no 3\n\t\t4 0 1 2 3\n\t\t4\n\t\t0 0 0\n\t\t1 0 0\n\t\t2 0 0\n\t\t3 0 0\n\t\t;\n";
    let body = format!(
        "createNode joint -n \"j\";\ncreateNode transform -n \"c\";\ncreateNode nurbsCurve -n \"cShape\" -p \"c\";\ncreateNode nurbsCurve -n \"cShapeOrig\" -p \"c\";\n\tsetAttr \".io\" yes;\n\tsetAttr \".cc\" -type \"nurbsCurve\" \n{cv}createNode skinCluster -n \"sk\";\n\tsetAttr -s 4 \".wl\";\n\tsetAttr \".wl[0:3].w\"\n\t\t1 0 1\n\t\t1 0 1\n\t\t1 0 1\n\t\t1 0 1;\n\tsetAttr \".pm[0]\" -type \"matrix\" 1 0 0 0 0 1 0 0 0 0 1 0 0 0 0 1;\n\tsetAttr \".gm\" -type \"matrix\" 1 0 0 0 0 1 0 0 0 0 1 0 0 0 0 1;\n\tsetAttr \".mi\" 1;\ncreateNode tweak -n \"tw\";\n\tsetAttr -s 1 \".pl[0].cp[1]\" -type \"double3\" 0 0.5 0;\nconnectAttr \"j.wm\" \"sk.ma[0]\";\nconnectAttr \"cShapeOrig.ws\" \"tw.ip[0].ig\";\nconnectAttr \"tw.og[0]\" \"sk.ip[0].ig\";\nconnectAttr \"sk.og[0]\" \"cShape.cr\";\ncreateNode pointOnCurveInfo -n \"poc\";\n\tsetAttr \".pr\" 1;\ncreateNode transform -n \"loc\";\nconnectAttr \"cShape.ws\" \"poc.ic\";\nconnectAttr \"poc.p\" \"loc.t\";\n"
    );
    let mut s = after_saving(&body);
    assert_near(get(&mut s, "loc.translate"), &[1.0, 0.5, 0.0], "the tweak");
    s.run("setAttr", json!({"args": ["j.ty", 2.0]})).unwrap();
    assert_near(get(&mut s, "loc.translate"), &[1.0, 2.5, 0.0], "the skin");
}

#[test]
fn a_driven_weight_survives_saving() {
    let body = format!(
        "createNode transform -n \"a\";\n\tsetAttr \".t\" -type \"double3\" 5 0 0 ;\ncreateNode transform -n \"sw\";\n\taddAttr -ci true -sn \"Follow\" -ln \"Follow\" -at \"double\";\n\tsetAttr -k on \".Follow\" 0;\ncreateNode transform -n \"obj\";\n\tsetAttr \".t\" -type \"double3\" 0 7 0 ;\n{}\tsetAttr \".rst\" -type \"double3\" 0 7 0 ;\ncreateNode animCurveUU -n \"w_curve\";\n\tsetAttr -s 2 \".ktv[0:1]\"  0 0 1 1;\nconnectAttr \"sw.Follow\" \"w_curve.i\";\nconnectAttr \"w_curve.o\" \"obj_parentConstraint1.w0\";\n",
        constraint("obj_parentConstraint1", "obj", "a", "aW0", true)
    );
    let mut s = after_saving(&body);
    assert_near(get(&mut s, "obj.translate"), &[0.0, 7.0, 0.0], "weight 0");
    s.run("setAttr", json!({"args": ["sw.Follow", 1.0]})).unwrap();
    assert_near(get(&mut s, "obj.translate"), &[5.0, 0.0, 0.0], "weight 1");
}

#[test]
fn opening_a_file_keeps_the_scenes_cameras_where_it_says() {
    let dir = std::env::temp_dir().join(format!("aztek_rig_files_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("cam.ma");
    let text = format!("{HEAD}createNode transform -s -n \"persp\";\n\tsetAttr \".t\" -type \"double3\" 1 2 3 ;\ncreateNode camera -s -n \"perspShape\" -p \"persp\";\n\tsetAttr \".fl\" 50;\n\tsetAttr \".coi\" 7;\n");
    std::fs::write(&path, text).unwrap();
    let mut s = Session::new();
    aztek_scene::ma_file::open_scene(&mut s, path.to_str().unwrap()).unwrap();
    assert_near(get(&mut s, "persp.translate"), &[1.0, 2.0, 3.0], "where the file has it");
    assert_near(get(&mut s, "perspShape.focalLength"), &[50.0], "the lens");
    assert!(s.graph.find("top").unwrap().is_some(), "the others exist too");
    // And it goes round.
    let (saved, _) = aztek_scene::ma_file::write_text(&mut s, "cam.ma", None);
    assert!(saved.contains("createNode transform -s -n \"persp\""), "{saved}");
    std::fs::write(&path, saved).unwrap();
    let mut again = Session::new();
    aztek_scene::ma_file::open_scene(&mut again, path.to_str().unwrap()).unwrap();
    assert_near(get(&mut again, "persp.translate"), &[1.0, 2.0, 3.0], "after saving");
    assert_near(get(&mut again, "perspShape.focalLength"), &[50.0], "the lens after saving");
    std::fs::remove_dir_all(dir).ok();
}

// Faults found opening two more production rigs, written small.

#[test]
fn every_node_has_a_message() {
    let body = "createNode transform -n \"obj\";\ncreateNode multiplyDivide -n \"md\";\ncreateNode transform -n \"hub\";\n\taddAttr -s false -ci true -m -sn \"links\" -ln \"links\" -at \"message\";\nconnectAttr \"obj.msg\" \"hub.links[0]\";\nconnectAttr \"md.msg\" \"hub.links[1]\";\n";
    let (mut s, w) = load(body);
    assert!(w.is_empty(), "{w:?}");
    let listed = s.run("listAttr", json!({"args": ["md"]})).unwrap();
    assert!(listed.as_array().unwrap().iter().any(|a| a == "message"), "{listed}");
    let c = s.run("listConnections", json!({"args": ["hub.links"], "source": true, "destination": false})).unwrap();
    assert_eq!(c, json!(["obj", "md"]));
    let (out, _) = aztek_scene::ma_file::write_text(&mut s, "m.ma", None);
    assert!(out.contains("-m -sn \"links\" -ln \"links\" -at \"message\""), "{out}");
    assert!(out.contains("connectAttr \"md.message\" \"hub.links[1]\";"), "{out}");
    let mut b = Session::new();
    let w = read_text(&mut b, &out, false);
    assert!(w.is_empty(), "{w:?}");
    assert_eq!(b.run("listConnections", json!({"args": ["hub.links"], "source": true, "destination": false})).unwrap(), json!(["obj", "md"]));
}

#[test]
fn a_node_may_read_its_own_parent_inverse_matrix() {
    let body = "createNode transform -n \"grp\";\n\tsetAttr \".t\" -type \"double3\" 1 2 3 ;\ncreateNode transform -n \"src\";\n\tsetAttr \".t\" -type \"double3\" 5 5 5 ;\ncreateNode joint -n \"jnt\" -p \"grp\";\ncreateNode multMatrix -n \"mm\";\ncreateNode decomposeMatrix -n \"dm\";\nconnectAttr \"src.wm\" \"mm.i[0]\";\nconnectAttr \"jnt.pim\" \"mm.i[1]\";\nconnectAttr \"mm.o\" \"dm.imat\";\nconnectAttr \"dm.ot\" \"jnt.t\";\n";
    let (mut s, w) = load(body);
    assert!(w.is_empty(), "{w:?}");
    assert_near(world_at(&mut s, "jnt"), &[5.0, 5.0, 5.0], "on the source");
    s.run("setAttr", json!({"args": ["grp.tx", 3.0]})).unwrap();
    assert_near(world_at(&mut s, "jnt"), &[5.0, 5.0, 5.0], "after the parent moved");
}

#[test]
fn a_point_constraint_reads_no_rotation_from_its_target() {
    // The target turns as what the constraint drives moves, as an IK chain's start joint does in a stretch setup. No cycle in the reference.
    let body = "createNode transform -n \"tgt\";\n\tsetAttr \".t\" -type \"double3\" 2 0 0 ;\ncreateNode transform -n \"obj\";\ncreateNode pointConstraint -n \"pc\" -p \"obj\";\n\taddAttr -dcb 0 -ci true -k true -sn \"w0\" -ln \"tgtW0\" -dv 1 -min 0 -at \"double\";\nconnectAttr \"tgt.t\" \"pc.tg[0].tt\";\nconnectAttr \"tgt.rp\" \"pc.tg[0].trp\";\nconnectAttr \"tgt.rpt\" \"pc.tg[0].trt\";\nconnectAttr \"tgt.pm\" \"pc.tg[0].tpm\";\nconnectAttr \"pc.w0\" \"pc.tg[0].tw\";\nconnectAttr \"pc.ctx\" \"obj.tx\";\nconnectAttr \"pc.cty\" \"obj.ty\";\nconnectAttr \"pc.ctz\" \"obj.tz\";\ncreateNode locator -n \"reader\";\ncreateNode multiplyDivide -n \"md\";\nconnectAttr \"obj.t\" \"md.i1\";\nconnectAttr \"md.o\" \"tgt.r\";\n";
    let (mut s, w) = load(body);
    assert!(w.is_empty(), "{w:?}");
    assert_near(get(&mut s, "obj.t"), &[2.0, 0.0, 0.0], "on the target");
    // A file's connection joins stored values: 2 from a translate is 2 radians with no conversion node between.
    assert_near(get(&mut s, "tgt.r"), &[2_f64.to_degrees(), 0.0, 0.0], "and what it drives turns the target");
}

#[test]
fn an_added_attribute_driven_from_below_is_no_cycle() {
    // A control showing a length measured from the joints it moves.
    let body = "createNode transform -n \"ctrl\";\n\taddAttr -ci true -sn \"len\" -ln \"len\" -at \"double\";\ncreateNode transform -n \"obj\" -p \"ctrl\";\n\tsetAttr \".t\" -type \"double3\" 4 0 0 ;\ncreateNode decomposeMatrix -n \"dm\";\nconnectAttr \"obj.wm\" \"dm.imat\";\nconnectAttr \"dm.otx\" \"ctrl.len\";\n";
    let (mut s, w) = load(body);
    assert!(w.is_empty(), "{w:?}");
    assert_near(get(&mut s, "ctrl.len"), &[4.0], "read after everything computed");
    s.run("setAttr", json!({"args": ["ctrl.tx", 3.0]})).unwrap();
    assert_near(get(&mut s, "ctrl.len"), &[7.0], "follows");
    // The same on a utility node, whose added attribute a save keeps.
    let body = "createNode distanceBetween -n \"db\";\n\taddAttr -ci true -k true -sn \"extra\" -ln \"extra\" -at \"double\";\ncreateNode addDoubleLinear -n \"adl\";\nconnectAttr \"db.d\" \"adl.i1\";\nconnectAttr \"adl.o\" \"db.extra\";\n";
    let (mut s, w) = load(body);
    assert!(w.is_empty(), "{w:?}");
    let (out, _) = aztek_scene::ma_file::write_text(&mut s, "e.ma", None);
    let mut b = Session::new();
    let w = read_text(&mut b, &out, false);
    assert!(w.is_empty(), "{w:?}");
}

#[test]
fn a_motion_path_drives_translate_with_trans_minus_rotate_pivot() {
    let body = "createNode transform -n \"curve1\";\ncreateNode nurbsCurve -n \"curveShape1\" -p \"curve1\";\n\tsetAttr -k off \".v\";\n\tsetAttr \".cc\" -type \"nurbsCurve\" \n\t\t1 1 0 no 3\n\t\t2 0 1\n\t\t2\n\t\t0 0 0\n\t\t10 0 0\n\t\t;\ncreateNode transform -n \"obj\";\n\tsetAttr \".rp\" -type \"double3\" 0 1 0 ;\ncreateNode motionPath -n \"mp\";\n\tsetAttr \".u\" 0.5;\n\tsetAttr \".fm\" yes;\ncreateNode addDoubleLinear -n \"ax\";\ncreateNode addDoubleLinear -n \"ay\";\nconnectAttr \"curveShape1.ws\" \"mp.gp\";\nconnectAttr \"obj.tmrx\" \"ax.i1\";\nconnectAttr \"mp.xc\" \"ax.i2\";\nconnectAttr \"obj.tmry\" \"ay.i1\";\nconnectAttr \"mp.yc\" \"ay.i2\";\nconnectAttr \"ax.o\" \"obj.tx\";\nconnectAttr \"ay.o\" \"obj.ty\";\nconnectAttr \"mp.msg\" \"obj.sml\";\n";
    let (mut s, w) = load(body);
    assert!(w.is_empty(), "{w:?}");
    assert_near(get(&mut s, "obj.t"), &[5.0, -1.0, 0.0], "its pivot on the curve");
    assert_near(get(&mut s, "obj.tmrp"), &[0.0, -1.0, 0.0], "minus the pivot");
    let (out, _) = aztek_scene::ma_file::write_text(&mut s, "m.ma", None);
    let mut b = Session::new();
    let w = read_text(&mut b, &out, false);
    assert!(w.is_empty(), "{w:?}");
    assert_near(get(&mut b, "obj.t"), &[5.0, -1.0, 0.0], "after a save");
}

#[test]
fn make_nurbs_square_gives_a_curve_per_side() {
    let body = "createNode makeNurbsSquare -n \"sq\";\ncreateNode transformGeometry -n \"tg\";\n\tsetAttr \".txf\" -type \"matrix\" 2 0 0 0 0 2 0 0 0 0 2 0 0 5 0 1;\ncreateNode transform -n \"side\";\ncreateNode nurbsCurve -n \"sideShape\" -p \"side\";\nconnectAttr \"sq.oc1\" \"tg.ig\";\nconnectAttr \"tg.og\" \"sideShape.cr\";\ncreateNode curveInfo -n \"ci\";\nconnectAttr \"sideShape.ws\" \"ci.ic\";\n";
    let (mut s, w) = load(body);
    assert!(w.is_empty(), "{w:?}");
    assert_near(get(&mut s, "ci.al"), &[2.0], "one unit side, scaled by 2");
}

#[test]
fn controller_tags_are_wired_as_the_reference_wires_them() {
    let body = "createNode transform -n \"a\";\ncreateNode transform -n \"b\";\ncreateNode controller -n \"a_tag\";\ncreateNode controller -n \"b_tag\";\nconnectAttr \"a.msg\" \"a_tag.act\";\nconnectAttr \"b.msg\" \"b_tag.act\";\nconnectAttr \"b_tag.pare\" \"a_tag.child[0]\";\nconnectAttr \"a_tag.prep\" \"b_tag.prep\";\n";
    let (mut s, w) = load(body);
    assert!(w.is_empty(), "{w:?}");
    assert_eq!(s.run("controller", json!({"args": ["b"], "query": true, "parent": true})).unwrap(), json!("a"));
    assert_eq!(s.run("controller", json!({"args": ["a"], "query": true, "children": true})).unwrap(), json!(["b"]));
    s.run("controller", json!({"args": ["b", "a"], "parent": "a"})).ok();
    let (out, _) = aztek_scene::ma_file::write_text(&mut s, "c.ma", None);
    assert!(out.contains("connectAttr \"b_tag.parent\" \"a_tag.children[0]\";"), "{out}");
    assert!(out.contains("connectAttr \"a.message\" \"a_tag.controllerObject\";"), "{out}");
}

#[test]
fn renderer_nodes_are_quiet() {
    let body = "createNode rmanGlobals -s -n \"rmanGlobals\";\ncreateNode PxrPathTracer -n \"tracer\";\ncreateNode d_openexr -n \"exr\";\ncreateNode rmanDisplay -n \"disp\";\ncreateNode aiOptions -s -n \"defaultArnoldRenderOptions\";\ncreateNode aiAOVFilter -n \"filter\";\ncreateNode displayPoints -n \"dp\";\n";
    let (_, w) = load(body);
    assert!(w.is_empty(), "{w:?}");
}

#[test]
fn a_long_expression_is_read_whole() {
    let body = "createNode transform -n \"a\";\ncreateNode transform -n \"b\";\ncreateNode expression -n \"ex\";\n\tsetAttr -s 1 \".in\";\n\tsetAttr -s 1 \".out\";\n\tsetAttr \".ixp\" -type \"string\" (\n\t\t\"float $v = .I[0];\\n\"\n\t\t+ \".O[0] = $v + 1;\");\nconnectAttr \"a.tx\" \"ex.in[0]\";\nconnectAttr \"ex.out[0]\" \"b.tx\";\n";
    let (mut s, w) = load(body);
    assert!(w.is_empty(), "{w:?}");
    assert_near(get(&mut s, "b.tx"), &[1.0], "ran");
    s.run("setAttr", json!({"args": ["a.tx", 2.0]})).unwrap();
    assert_near(get(&mut s, "b.tx"), &[3.0], "follows");
}

#[test]
fn an_expression_may_hold_or_and_a_set_attr() {
    let body = "createNode transform -n \"a\";\ncreateNode transform -n \"b\";\ncreateNode expression -n \"ex\";\n\tsetAttr \".ixp\" -type \"string\" \"if((.I[0]>0)||(.I[0]<-1)){\\nsetAttr a.ty (.I[0]*2);}\\n.O[0] = .I[0]+1;\";\nconnectAttr \"a.tx\" \"ex.in[0]\";\nconnectAttr \"ex.out[0]\" \"b.tx\";\n";
    let (mut s, w) = load(body);
    assert!(w.is_empty(), "{w:?}");
    assert_near(get(&mut s, "b.tx"), &[1.0], "ran");
}

#[test]
fn a_cluster_on_some_of_a_curves_cvs_moves_only_those() {
    let body = "createNode transform -n \"c\";\ncreateNode nurbsCurve -n \"cShape\" -p \"c\";\n\tsetAttr -k off \".v\";\ncreateNode nurbsCurve -n \"cShapeOrig\" -p \"c\";\n\tsetAttr -k off \".v\";\n\tsetAttr \".io\" yes;\n\tsetAttr \".cc\" -type \"nurbsCurve\" \n\t\t1 2 0 no 3\n\t\t3 0 1 2\n\t\t3\n\t\t0 0 0\n\t\t1 0 0\n\t\t2 0 0\n\t\t;\ncreateNode transform -n \"h\";\n\tsetAttr \".t\" -type \"double3\" 0 5 0 ;\ncreateNode clusterHandle -n \"hShape\" -p \"h\";\ncreateNode cluster -n \"cl\";\ncreateNode groupParts -n \"gp\";\n\tsetAttr \".ic\" -type \"componentList\" 1 \"cv[2]\";\nconnectAttr \"cShapeOrig.ws\" \"gp.ig\";\nconnectAttr \"gp.og\" \"cl.ip[0].ig\";\nconnectAttr \"cl.og[0]\" \"cShape.cr\";\nconnectAttr \"h.wm\" \"cl.ma\";\nconnectAttr \"hShape.x\" \"cl.x\";\n";
    let (mut s, w) = load(body);
    assert!(w.is_empty(), "{w:?}");
    let id = s.graph.find("cShape").unwrap().unwrap();
    let c = aztek_scene::curves::world_curve(&mut s.graph, id).unwrap();
    assert_near(c.cvs[2].to_array().to_vec(), &[2.0, 5.0, 0.0], "the member moved");
    assert_near(c.cvs[0].to_array().to_vec(), &[0.0, 0.0, 0.0], "the others did not");
}

#[test]
fn a_constraint_drives_only_the_channels_the_file_wires() {
    let body = "createNode transform -n \"tgt\";\n\tsetAttr \".t\" -type \"double3\" 1 2 3 ;\ncreateNode transform -n \"obj\";\n\tsetAttr \".t\" -type \"double3\" 0 9 0 ;\ncreateNode pointConstraint -n \"pc\" -p \"obj\";\n\taddAttr -dcb 0 -ci true -k true -sn \"w0\" -ln \"tgtW0\" -dv 1 -min 0 -at \"double\";\nconnectAttr \"tgt.t\" \"pc.tg[0].tt\";\nconnectAttr \"tgt.pm\" \"pc.tg[0].tpm\";\nconnectAttr \"pc.w0\" \"pc.tg[0].tw\";\nconnectAttr \"pc.ctx\" \"obj.tx\";\nconnectAttr \"pc.ctz\" \"obj.tz\";\n";
    let (mut s, w) = load(body);
    assert!(w.is_empty(), "{w:?}");
    assert_near(get(&mut s, "obj.t"), &[1.0, 9.0, 3.0], "Y as stored");
    let (out, _) = aztek_scene::ma_file::write_text(&mut s, "p.ma", None);
    let mut b = Session::new();
    let w = read_text(&mut b, &out, false);
    assert!(w.is_empty(), "{w:?}");
    assert_near(get(&mut b, "obj.t"), &[1.0, 9.0, 3.0], "and after a save");
}

#[test]
fn a_blocked_constraint_holds_its_rest() {
    let body = "createNode transform -n \"tgt\";\n\tsetAttr \".t\" -type \"double3\" 1 2 3 ;\ncreateNode transform -n \"sw\";\n\taddAttr -ci true -k true -sn \"st\" -ln \"st\" -dv 2 -at \"double\";\ncreateNode transform -n \"obj\";\n\tsetAttr \".t\" -type \"double3\" 4 5 6 ;\ncreateNode pointConstraint -n \"pc\" -p \"obj\";\n\taddAttr -dcb 0 -ci true -k true -sn \"w0\" -ln \"tgtW0\" -dv 1 -min 0 -at \"double\";\n\tsetAttr \".rst\" -type \"double3\" 4 5 6 ;\nconnectAttr \"tgt.t\" \"pc.tg[0].tt\";\nconnectAttr \"tgt.pm\" \"pc.tg[0].tpm\";\nconnectAttr \"pc.w0\" \"pc.tg[0].tw\";\nconnectAttr \"pc.ctx\" \"obj.tx\";\nconnectAttr \"pc.cty\" \"obj.ty\";\nconnectAttr \"pc.ctz\" \"obj.tz\";\nconnectAttr \"sw.st\" \"pc.nds\";\n";
    let (mut s, w) = load(body);
    assert!(w.is_empty(), "{w:?}");
    assert_near(get(&mut s, "obj.t"), &[4.0, 5.0, 6.0], "blocked");
    s.run("setAttr", json!({"args": ["sw.st", 0.0]})).unwrap();
    assert_near(get(&mut s, "obj.t"), &[1.0, 2.0, 3.0], "running");
}

#[test]
fn an_extrude_and_a_colour_set_come_from_a_file() {
    let curve = |n: &str, pts: &str| format!("createNode transform -n \"{n}\";\ncreateNode nurbsCurve -n \"{n}Shape\" -p \"{n}\";\n\tsetAttr -k off \".v\";\n\tsetAttr \".cc\" -type \"nurbsCurve\" \n\t\t1 1 0 no 3\n\t\t2 0 1\n\t\t2\n\t\t{pts}\n\t\t;\n");
    let body = format!("{}{}createNode extrude -n \"ex\";\n\tsetAttr \".fpt\" yes;\ncreateNode reverseSurface -n \"rs\";\ncreateNode transform -n \"srf\";\ncreateNode nurbsSurface -n \"srfShape\" -p \"srf\";\nconnectAttr \"prof1Shape.ws\" \"ex.pr\";\nconnectAttr \"path1Shape.ws\" \"ex.pt\";\nconnectAttr \"ex.os\" \"rs.is\";\nconnectAttr \"rs.os\" \"srfShape.cr\";\ncreateNode transform -n \"m\";\ncreateNode mesh -n \"mShape\" -p \"m\";\ncreateNode mesh -n \"mShapeOrig\" -p \"m\";\n\tsetAttr \".io\" yes;\n\tsetAttr -s 3 \".vt[0:2]\" 0 0 0 1 0 0 0 1 0;\n\tsetAttr -s 3 \".ed[0:2]\" 0 1 0 1 2 0 2 0 0;\n\tsetAttr -ch 3 \".fc[0]\" -type \"polyFaces\" f 3 0 1 2;\ncreateNode createColorSet -n \"cc\";\nconnectAttr \"mShapeOrig.w\" \"cc.ig\";\nconnectAttr \"cc.og\" \"mShape.i\";\n", curve("prof1", "0 0 0\n\t\t1 0 0"), curve("path1", "0 0 0\n\t\t0 5 0"));
    let (mut s, w) = load(&body);
    assert!(w.is_empty(), "{w:?}");
    let id = s.graph.find("srfShape").unwrap().unwrap();
    assert!(s.graph.source(&aztek_scene::plug::PlugRef::new(id, aztek_scene::surfaces::SurfaceShape::CREATE as i32)).is_some(), "wired");
    assert_eq!(s.run("polyEvaluate", json!({"args": ["mShape"], "vertex": true})).unwrap(), json!(3));
}

#[test]
fn a_ramp_reads_its_entries() {
    let body = "createNode ramp -n \"rp\";\n\tsetAttr -s 2 \".cel\";\n\tsetAttr \".cel[0].ep\" 0;\n\tsetAttr \".cel[0].ec\" -type \"float3\" 0 0 0 ;\n\tsetAttr \".cel[1].ep\" 1;\n\tsetAttr \".cel[1].ec\" -type \"float3\" 4 0 0 ;\n\tsetAttr \".v\" 0.25;\ncreateNode transform -n \"o\";\nconnectAttr \"rp.ocr\" \"o.tx\";\n";
    let (mut s, w) = load(body);
    assert!(w.is_empty(), "{w:?}");
    assert_near(get(&mut s, "o.tx"), &[1.0], "a quarter of the way");
}

#[test]
fn a_curve_fed_by_its_original_has_its_points_and_percentage_is_of_the_range() {
    let body = "createNode transform -n \"c\";\ncreateNode nurbsCurve -n \"cShape\" -p \"c\";\n\tsetAttr -k off \".v\";\ncreateNode nurbsCurve -n \"cShapeOrig\" -p \"c\";\n\tsetAttr -k off \".v\";\n\tsetAttr \".io\" yes;\n\tsetAttr \".cc\" -type \"nurbsCurve\" \n\t\t1 2 0 no 3\n\t\t3 0 1 2\n\t\t3\n\t\t0 0 0\n\t\t1 0 0\n\t\t11 0 0\n\t\t;\nconnectAttr \"cShapeOrig.ws\" \"cShape.cr\";\ncreateNode pointOnCurveInfo -n \"pc\";\n\tsetAttr \".pr\" 0.5;\n\tsetAttr \".top\" yes;\nconnectAttr \"cShape.ws\" \"pc.ic\";\ncreateNode transform -n \"o\";\nconnectAttr \"pc.p\" \"o.t\";\n";
    let (mut s, w) = load(body);
    assert!(w.is_empty(), "{w:?}");
    assert_near(get(&mut s, "o.t"), &[1.0, 0.0, 0.0], "half the range is the middle CV, a sixth of the length");
}

#[test]
fn a_transform_limit_can_be_driven() {
    let body = "createNode transform -n \"sw\";\n\taddAttr -ci true -k true -sn \"on\" -ln \"on\" -min 0 -max 1 -at \"bool\";\ncreateNode transform -n \"o\";\n\tsetAttr \".t\" -type \"double3\" 5 0 0 ;\n\tsetAttr \".mxtl\" -type \"double3\" 2 0 0 ;\nconnectAttr \"sw.on\" \"o.xtxe\";\n";
    let (mut s, w) = load(body);
    assert!(w.is_empty(), "{w:?}");
    assert_near(world_at(&mut s, "o"), &[5.0, 0.0, 0.0], "off");
    s.run("setAttr", json!({"args": ["sw.on", true]})).unwrap();
    assert_near(world_at(&mut s, "o"), &[2.0, 0.0, 0.0], "held");
}

#[test]
fn a_control_point_on_a_curve_with_history_is_a_move() {
    let body = "createNode transform -n \"c\";\ncreateNode nurbsCurve -n \"cShape\" -p \"c\";\n\tsetAttr -k off \".v\";\n\tsetAttr \".tw\" yes;\ncreateNode nurbsCurve -n \"cShapeOrig\" -p \"c\";\n\tsetAttr -k off \".v\";\n\tsetAttr \".io\" yes;\n\tsetAttr \".cc\" -type \"nurbsCurve\" \n\t\t1 1 0 no 3\n\t\t2 0 1\n\t\t2\n\t\t1 0 0\n\t\t2 0 0\n\t\t;\ncreateNode transform -n \"j\";\n\tsetAttr \".t\" -type \"double3\" 0 3 0 ;\nconnectAttr \"cShapeOrig.ws\" \"cShape.cr\";\nconnectAttr \"j.t\" \"cShape.cp[1]\";\n";
    let (mut s, w) = load(body);
    assert!(w.is_empty(), "{w:?}");
    let id = s.graph.find("cShape").unwrap().unwrap();
    let c = aztek_scene::curves::world_curve(&mut s.graph, id).unwrap();
    assert_near(c.cvs[1].to_array().to_vec(), &[2.0, 3.0, 0.0], "on top of the original");
    let (out, _) = aztek_scene::ma_file::write_text(&mut s, "p.ma", None);
    let mut b = Session::new();
    let w = read_text(&mut b, &out, false);
    assert!(w.is_empty(), "{w:?}");
    let id = b.graph.find("cShape").unwrap().unwrap();
    let c = aztek_scene::curves::world_curve(&mut b.graph, id).unwrap();
    assert_near(c.cvs[1].to_array().to_vec(), &[2.0, 3.0, 0.0], "and after a save");
}

#[test]
fn an_offset_rotation_is_in_the_driven_objects_rotate_order() {
    // One turn in YZX order. Read in XYZ it is a different turn, so the object would miss the stored rotation.
    let body = format!(
        "createNode transform -n \"tgt\";\ncreateNode transform -n \"obj\";\n\tsetAttr \".ro\" 1;\n{}\tsetAttr \".tg[0].tor\" -type \"double3\" 30 40 50 ;\nconnectAttr \"obj.ro\" \"obj_parentConstraint1.cro\";\n",
        constraint("obj_parentConstraint1", "obj", "tgt", "tgtW0", true)
    );
    let (mut s, w) = load(&body);
    assert!(w.is_empty(), "{w:?}");
    assert_near(get(&mut s, "obj.r"), &[30.0, 40.0, 50.0], "the same three angles");
}

fn spline_chain(extra: &str) -> String {
    format!("createNode joint -n \"j1\";\ncreateNode joint -n \"j2\" -p \"j1\";\n\tsetAttr \".t\" -type \"double3\" 2 0 0 ;\ncreateNode joint -n \"j3\" -p \"j2\";\n\tsetAttr \".t\" -type \"double3\" 2 0 0 ;\ncreateNode transform -n \"c\";\ncreateNode nurbsCurve -n \"cShape\" -p \"c\";\n\tsetAttr \".cc\" -type \"nurbsCurve\" \n\t\t1 1 0 no 3\n\t\t2 0 1\n\t\t2\n\t\t0 0 0\n\t\t4 0 0\n\t\t;\ncreateNode transform -n \"up\";\n\tsetAttr \".r\" -type \"double3\" 90 0 0 ;\ncreateNode ikHandle -n \"h\";\n\tsetAttr \".roc\" yes;\n\tsetAttr \".dtce\" yes;\n\tsetAttr \".dwut\" 4;\n{extra}createNode ikEffector -n \"e\" -p \"j2\";\ncreateNode ikSplineSolver -n \"ikSplineSolver\";\nconnectAttr \"j1.msg\" \"h.hsj\";\nconnectAttr \"e.hp\" \"h.hee\";\nconnectAttr \"ikSplineSolver.msg\" \"h.hsv\";\nconnectAttr \"cShape.ws\" \"h.ic\";\nconnectAttr \"j3.tx\" \"e.tx\";\nconnectAttr \"up.wm\" \"h.dwum\";\nconnectAttr \"up.wm\" \"h.dwue\";\n")
}

#[test]
fn spline_ik_advanced_twist_follows_its_up_objects() {
    // Up, Y in the up object's space, is world Z there, so the joints roll 90 about X.
    let (mut s, w) = load(&spline_chain(""));
    assert!(w.is_empty(), "{w:?}");
    assert_near(get(&mut s, "j1.r"), &[90.0, 0.0, 0.0], "start");
    assert_near(world_at(&mut s, "j3"), &[4.0, 0.0, 0.0], "on the curve");
}

#[test]
fn spline_ik_twist_can_come_from_a_ramp() {
    let body = format!("{}createNode ramp -n \"rp\";\n\tsetAttr -s 2 \".cel\";\n\tsetAttr \".cel[0].ep\" 0;\n\tsetAttr \".cel[0].ec\" -type \"float3\" 0 0 0 ;\n\tsetAttr \".cel[1].ep\" 1;\n\tsetAttr \".cel[1].ec\" -type \"float3\" 1 0 0 ;\nconnectAttr \"rp.oc\" \"h.dtra\";\n", spline_chain("\tsetAttr \".dtvt\" 2;\n\tsetAttr \".dtrm\" 40;\n"));
    let (mut s, w) = load(&body);
    assert!(w.is_empty(), "{w:?}");
    assert_near(get(&mut s, "j1.r"), &[90.0, 0.0, 0.0], "none at the start");
    assert_near(get(&mut s, "j2.r"), &[40.0, 0.0, 0.0], "all of the ramp at the last joint, on top of the first");
}

#[test]
fn a_unit_conversion_with_a_factor_of_its_own_scales() {
    let body = "createNode transform -n \"a\";\n\tsetAttr \".tx\" 10;\ncreateNode transform -n \"b\";\ncreateNode unitConversion -n \"tenth\";\n\tsetAttr \".cf\" 0.1;\nconnectAttr \"a.tx\" \"tenth.i\";\nconnectAttr \"tenth.o\" \"b.tx\";\ncreateNode transform -n \"c\";\ncreateNode unitConversion -n \"toDegrees\";\n\tsetAttr \".cf\" 0.017453292519943295;\nconnectAttr \"a.tx\" \"toDegrees.i\";\nconnectAttr \"toDegrees.o\" \"c.rx\";\ncreateNode transform -n \"d\";\ncreateNode unitConversion -n \"twoAndAHalf\";\n\tsetAttr \".cf\" 0.043633231299858237;\nconnectAttr \"a.tx\" \"twoAndAHalf.i\";\nconnectAttr \"twoAndAHalf.o\" \"d.rx\";\n";
    let (mut s, w) = load(body);
    assert!(w.is_empty(), "{w:?}");
    assert_near(get(&mut s, "b.tx"), &[1.0], "a tenth");
    assert_near(get(&mut s, "c.rx"), &[10.0], "only a change of units");
    assert_near(get(&mut s, "d.rx"), &[25.0], "2.5 degrees per unit");
    let (out, _) = aztek_scene::ma_file::write_text(&mut s, "u.ma", None);
    let mut b = Session::new();
    let w = read_text(&mut b, &out, false);
    assert!(w.is_empty(), "{w:?}");
    assert_near(get(&mut b, "b.tx"), &[1.0], "after a save");
    assert_near(get(&mut b, "d.rx"), &[25.0], "after a save");
}

#[test]
fn a_preferred_angle_comes_from_the_file_and_goes_back_to_it() {
    let body = "createNode joint -n \"a\";\ncreateNode joint -n \"b\" -p \"a\";\n\tsetAttr \".t\" -type \"double3\" 2 0 0 ;\n\tsetAttr \".pa\" -type \"double3\" 0 -60 0 ;\ncreateNode joint -n \"c\" -p \"b\";\n\tsetAttr \".t\" -type \"double3\" 2 0 0 ;\ncreateNode ikHandle -n \"h\";\n\tsetAttr \".t\" -type \"double3\" 3 0 0 ;\ncreateNode ikEffector -n \"e\" -p \"b\";\ncreateNode ikRPsolver -n \"ikRPsolver\";\nconnectAttr \"a.msg\" \"h.hsj\";\nconnectAttr \"e.hp\" \"h.hee\";\nconnectAttr \"ikRPsolver.msg\" \"h.hsv\";\nconnectAttr \"c.tx\" \"e.tx\";\n";
    let (mut s, w) = load(body);
    assert!(w.is_empty(), "{w:?}");
    assert_near(get(&mut s, "b.pa"), &[0.0, -60.0, 0.0], "read");
    let first = get(&mut s, "b.worldMatrix");
    let (out, _) = aztek_scene::ma_file::write_text(&mut s, "p.ma", None);
    assert!(out.contains("setAttr \".pa\" -type \"double3\" 0 -60 0 ;"), "{out}");
    let mut b = Session::new();
    let w = read_text(&mut b, &out, false);
    assert!(w.is_empty(), "{w:?}");
    assert_near(get(&mut b, "b.worldMatrix"), &first, "solves the same way after a save");
}

#[test]
fn an_aim_with_a_zero_up_vector_turns_the_shortest_way() {
    let body = "createNode transform -n \"tgt\";\n\tsetAttr \".t\" -type \"double3\" 1 0 1 ;\ncreateNode transform -n \"obj\";\ncreateNode aimConstraint -n \"ac\" -p \"obj\";\n\taddAttr -dcb 0 -ci true -sn \"w0\" -ln \"tgtW0\" -dv 1 -at \"double\";\n\tsetAttr \".a\" -type \"double3\" 0 0 1 ;\n\tsetAttr \".wu\" -type \"double3\" 0 0 0 ;\nconnectAttr \"tgt.t\" \"ac.tg[0].tt\";\nconnectAttr \"tgt.pm\" \"ac.tg[0].tpm\";\nconnectAttr \"ac.w0\" \"ac.tg[0].tw\";\nconnectAttr \"obj.t\" \"ac.ct\";\nconnectAttr \"ac.crx\" \"obj.rx\";\nconnectAttr \"ac.cry\" \"obj.ry\";\nconnectAttr \"ac.crz\" \"obj.rz\";\n";
    let (mut s, w) = load(body);
    assert!(w.is_empty(), "{w:?}");
    assert_near(get(&mut s, "obj.r"), &[0.0, 45.0, 0.0], "about Y alone");
}

#[test]
fn a_target_weight_can_come_from_a_utility_node() {
    let body = format!(
        "createNode transform -n \"tgt\";\n\tsetAttr \".t\" -type \"double3\" 5 0 0 ;\ncreateNode transform -n \"sw\";\n\taddAttr -ci true -sn \"on\" -ln \"on\" -at \"double\";\ncreateNode condition -n \"cd\";\n\tsetAttr \".ct\" -type \"float3\" 1 0 0 ;\n\tsetAttr \".cf\" -type \"float3\" 0 0 0 ;\n\tsetAttr \".st\" 1;\ncreateNode transform -n \"obj\";\n\tsetAttr \".t\" -type \"double3\" 0 7 0 ;\n{}\tsetAttr \".rst\" -type \"double3\" 0 7 0 ;\nconnectAttr \"sw.on\" \"cd.ft\";\nconnectAttr \"cd.ocr\" \"obj_parentConstraint1.w0\";\n",
        constraint("obj_parentConstraint1", "obj", "tgt", "tgtW0", true)
    );
    let (mut s, w) = load(&body);
    assert!(w.is_empty(), "{w:?}");
    assert_near(get(&mut s, "obj.t"), &[0.0, 7.0, 0.0], "off: at rest");
    s.run("setAttr", json!({"args": ["sw.on", 1.0]})).unwrap();
    assert_near(get(&mut s, "obj.t"), &[5.0, 0.0, 0.0], "on: on the target");
}

#[test]
fn a_single_chain_handle_keeps_the_rotation_it_was_stored_with() {
    let body = "createNode joint -n \"j1\";\n\tsetAttr \".r\" -type \"double3\" 0 0 0 ;\ncreateNode joint -n \"j2\" -p \"j1\";\n\tsetAttr \".t\" -type \"double3\" 2 0 0 ;\ncreateNode transform -n \"grp\";\ncreateNode ikHandle -n \"h\" -p \"grp\";\n\tsetAttr \".t\" -type \"double3\" 2 0 0 ;\n\tsetAttr \".r\" -type \"double3\" 30 0 0 ;\ncreateNode ikEffector -n \"e\" -p \"j1\";\ncreateNode ikSCsolver -n \"ikSCsolver\";\nconnectAttr \"j1.msg\" \"h.hsj\";\nconnectAttr \"e.hp\" \"h.hee\";\nconnectAttr \"ikSCsolver.msg\" \"h.hsv\";\nconnectAttr \"j2.tx\" \"e.tx\";\nconnectAttr \"j2.ty\" \"e.ty\";\nconnectAttr \"j2.tz\" \"e.tz\";\n";
    let (mut s, w) = load(body);
    assert!(w.is_empty(), "{w:?}");
    assert_near(get(&mut s, "j1.r"), &[0.0, 0.0, 0.0], "as stored, the handle at 30 degrees");
    // Turning the handle about the chain turns the chain by as much.
    s.run("setAttr", json!({"args": ["h.rx", 75.0]})).unwrap();
    assert_near(world_at(&mut s, "j2"), &[2.0, 0.0, 0.0], "still reaches the handle");
    assert_near(get(&mut s, "j1.r"), &[45.0, 0.0, 0.0], "turned as the handle did");
}

#[test]
fn a_scale_constraint_on_a_mirrored_target_has_z_negative() {
    let body = "createNode transform -n \"mirror\";\n\tsetAttr \".s\" -type \"double3\" 1 1 -1 ;\ncreateNode transform -n \"tgt\" -p \"mirror\";\ncreateNode transform -n \"obj\";\ncreateNode scaleConstraint -n \"sc\" -p \"obj\";\n\taddAttr -dcb 0 -ci true -k true -sn \"w0\" -ln \"tgtW0\" -dv 1 -min 0 -at \"double\";\n\tsetAttr \".o\" -type \"double3\" 1 1 -1 ;\nconnectAttr \"tgt.s\" \"sc.tg[0].ts\";\nconnectAttr \"tgt.pm\" \"sc.tg[0].tpm\";\nconnectAttr \"sc.w0\" \"sc.tg[0].tw\";\nconnectAttr \"sc.csx\" \"obj.sx\";\nconnectAttr \"sc.csy\" \"obj.sy\";\nconnectAttr \"sc.csz\" \"obj.sz\";\n";
    let (mut s, w) = load(body);
    assert!(w.is_empty(), "{w:?}");
    assert_near(get(&mut s, "obj.s"), &[1.0, 1.0, 1.0], "the offset cancels the mirror");
}

#[test]
fn matrix_nodes_have_the_reference_short_names() {
    let body = "createNode transform -n \"src\";\n\tsetAttr \".t\" -type \"double3\" 3 0 0 ;\n\tsetAttr \".r\" -type \"double3\" 0 0 90 ;\ncreateNode pickMatrix -n \"pick\";\n\tsetAttr \".rot\" no;\ncreateNode blendMatrix -n \"bm\";\n\tsetAttr \".tgt[0].wgt\" 0.5;\ncreateNode aimMatrix -n \"am\";\n\tsetAttr \".sm\" 0;\ncreateNode transform -n \"aimAt\";\n\tsetAttr \".t\" -type \"double3\" 0 0 4 ;\ncreateNode setRange -n \"sr\";\n\tsetAttr \".m\" -type \"float3\" 10 0 0 ;\n\tsetAttr \".om\" -type \"float3\" 2 0 0 ;\n\tsetAttr \".v\" -type \"float3\" 1 0 0 ;\ncreateNode decomposeMatrix -n \"dp\";\ncreateNode decomposeMatrix -n \"db\";\ncreateNode decomposeMatrix -n \"da\";\ncreateNode transform -n \"out\";\nconnectAttr \"src.wm\" \"pick.imat\";\nconnectAttr \"pick.tmat\" \"dp.imat\";\nconnectAttr \"pick.tmat\" \"bm.tgt[0].tmat\";\nconnectAttr \"bm.omat\" \"db.imat\";\nconnectAttr \"aimAt.wm\" \"am.pmat\";\nconnectAttr \"am.tmat\" \"da.imat\";\nconnectAttr \"sr.ox\" \"out.tx\";\n";
    let (mut s, w) = load(body);
    assert!(w.is_empty(), "{w:?}");
    assert_near(get(&mut s, "dp.or"), &[0.0, 0.0, 0.0], "no rotation picked");
    assert_near(get(&mut s, "dp.ot"), &[3.0, 0.0, 0.0], "translation picked");
    assert_near(get(&mut s, "db.ot"), &[1.5, 0.0, 0.0], "half way");
    assert_near(get(&mut s, "da.or"), &[0.0, -90.0, 0.0], "X aimed along Z");
    assert_near(get(&mut s, "out.tx"), &[5.0], "setRange's outValueX");
}

#[test]
fn a_pair_blend_rotate_mode_takes_one_input() {
    let body = "createNode pairBlend -n \"pb\";\n\tsetAttr \".w\" 0.25;\n\tsetAttr \".rm\" 2;\n\tsetAttr \".txm\" 1;\n\tsetAttr \".ir1\" -type \"double3\" 10 0 0 ;\n\tsetAttr \".ir2\" -type \"double3\" 20 0 0 ;\n\tsetAttr \".it1\" -type \"double3\" 1 1 0 ;\n\tsetAttr \".it2\" -type \"double3\" 3 3 0 ;\ncreateNode transform -n \"obj\";\nconnectAttr \"pb.or\" \"obj.r\";\nconnectAttr \"pb.ot\" \"obj.t\";\n";
    let (mut s, w) = load(body);
    assert!(w.is_empty(), "{w:?}");
    assert_near(get(&mut s, "obj.r"), &[20.0, 0.0, 0.0], "input 2 only");
    assert_near(get(&mut s, "obj.t"), &[1.0, 1.5, 0.0], "input 1 only on X, blended on Y");
}

#[test]
fn ik_blend_can_be_driven_by_a_switch() {
    let body = "createNode joint -n \"j1\";\ncreateNode joint -n \"j2\" -p \"j1\";\n\tsetAttr \".t\" -type \"double3\" 2 0 0 ;\ncreateNode transform -n \"sw\";\n\taddAttr -ci true -k true -sn \"ik\" -ln \"ik\" -dv 1 -at \"double\";\ncreateNode ikHandle -n \"h\";\n\tsetAttr \".t\" -type \"double3\" 0 2 0 ;\ncreateNode ikEffector -n \"e\" -p \"j1\";\ncreateNode ikRPsolver -n \"ikRPsolver\";\nconnectAttr \"j1.msg\" \"h.hsj\";\nconnectAttr \"e.hp\" \"h.hee\";\nconnectAttr \"ikRPsolver.msg\" \"h.hsv\";\nconnectAttr \"j2.tx\" \"e.tx\";\nconnectAttr \"sw.ik\" \"h.ikb\";\n";
    let (mut s, w) = load(body);
    assert!(w.is_empty(), "{w:?}");
    assert_near(world_at(&mut s, "j2"), &[0.0, 2.0, 0.0], "IK on");
    s.run("setAttr", json!({"args": ["sw.ik", 0.0]})).unwrap();
    assert_near(world_at(&mut s, "j2"), &[2.0, 0.0, 0.0], "IK off");
}

/// A file's `connectAttr "a.s" "b.is"` holds: a joint doesn't take on its parent joint's scale.
#[test]
fn a_joint_compensates_for_its_parents_scale() {
    let (mut s, _) = load(
        "createNode joint -n \"a\";\n\tsetAttr \".s\" -type \"double3\" 2 2 2 ;\n\
         createNode joint -n \"b\" -p \"a\";\n\tsetAttr \".t\" -type \"double3\" 1 0 0 ;\n\
         connectAttr \"a.s\" \"b.is\";\n",
    );
    assert_near(get(&mut s, "b.is"), &[2.0, 2.0, 2.0], "inverseScale");
    let m = get(&mut s, "b.worldMatrix");
    assert_near(m[0..3].to_vec(), &[1.0, 0.0, 0.0], "b keeps its own size");
    assert_near(m[12..15].to_vec(), &[2.0, 0.0, 0.0], "and sits where the scaled parent puts it");
}
