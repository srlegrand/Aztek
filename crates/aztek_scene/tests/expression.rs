//! The expression command and node.

use aztek_scene::{scene_file, Session};
use serde_json::{json, Value};

/// The frame rate is process-wide, as in the reference: tests that lean on it take turns.
static TIME: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn run(s: &mut Session, name: &str, call: Value) -> Value {
    s.run(name, call).unwrap_or_else(|e| panic!("{name}: {e}"))
}
fn get(s: &mut Session, plug: &str) -> f64 {
    let v = run(s, "getAttr", json!({"args": [plug]}));
    v.as_f64().unwrap_or_else(|| panic!("{plug}: {v}"))
}
fn close(got: f64, want: f64) {
    assert!((got - want).abs() < 1e-6, "got {got}, wanted {want}");
}
fn frame(s: &mut Session, f: f64) {
    run(s, "currentTime", json!({"args": [f]}));
}
fn cube() -> Session {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({}));
    s
}
fn expr(s: &mut Session, src: &str) -> String {
    run(s, "expression", json!({"string": src})).as_str().unwrap().to_string()
}

#[test]
fn drives_with_time_and_frame() {
    let _t = TIME.lock().unwrap();
    let mut s = cube();
    let n = run(&mut s, "expression", json!({"string": "pCube1.tx = sin(time);", "object": "pCube1", "alwaysEvaluate": 1, "unitConversion": "all", "name": "wobble"}));
    assert_eq!(n, "wobble");
    assert_eq!(run(&mut s, "nodeType", json!({"args": ["wobble"]})), "expression");
    let fps = aztek_scene::anim::fps();
    frame(&mut s, 12.0);
    close(get(&mut s, "pCube1.tx"), (12.0f64 / fps).sin());
    frame(&mut s, 48.0);
    close(get(&mut s, "pCube1.tx"), (48.0f64 / fps).sin());
    expr(&mut s, "pCube1.ty = frame * 2;");
    close(get(&mut s, "pCube1.ty"), 96.0);
    // It is a real connection, and reaches the matrix.
    assert_eq!(run(&mut s, "listConnections", json!({"args": ["pCube1.tx"], "source": true, "destination": false})), json!(["wobble"]));
    let m = run(&mut s, "getAttr", json!({"args": ["pCube1.worldMatrix"]}));
    close(m[13].as_f64().unwrap(), 96.0);
}

#[test]
fn reads_one_attribute_and_drives_another_on_the_same_object() {
    let mut s = cube();
    expr(&mut s, "pCube1.ry = pCube1.tx * 10;");
    run(&mut s, "setAttr", json!({"args": ["pCube1.tx", 3.0]}));
    close(get(&mut s, "pCube1.ry"), 30.0);
    run(&mut s, "setAttr", json!({"args": ["pCube1.tx", 4.0]}));
    close(get(&mut s, "pCube1.ry"), 40.0);
}

#[test]
fn object_flag_and_short_names() {
    let mut s = cube();
    run(&mut s, "expression", json!({"string": ".tx = 2; ty = .tx + 1; scaleZ = 3;", "object": "pCube1"}));
    close(get(&mut s, "pCube1.tx"), 2.0);
    close(get(&mut s, "pCube1.ty"), 3.0);
    close(get(&mut s, "pCube1.sz"), 3.0);
}

#[test]
fn the_language() {
    let mut s = cube();
    expr(
        &mut s,
        "float $a = 2.5;\n\
         int $i = 3.9; // a comment\n\
         vector $v = <<1, 2, 3>>;\n\
         /* block */\n\
         pCube1.tx = $a * $i + $v.y;\n\
         if ($a > 2) { pCube1.ty = 1; } else { pCube1.ty = -1; }\n\
         if ($a < 2) pCube1.tz = 1; else pCube1.tz = abs(-7);\n\
         float $sum = 0;\n\
         for ($k = 0; $k < 4; $k++) { $sum += $k; }\n\
         pCube1.sx = $sum;\n\
         int $n = 0; while ($n < 5) { $n++; if ($n == 2) break; }\n\
         pCube1.sy = $n;\n\
         pCube1.sz = ($a == 2.5) ? pow(2, 3) : 0;\n\
         vector $c = <<1, 0, 0>> ^ <<0, 1, 0>>;\n\
         pCube1.rx = $c.z + clamp(0, 1, 5) + min(4, 2) + max(4, 2) + floor(1.5) + ceil(1.5) + sqrt(16) + mag(<<3, 4, 0>>);\n\
         pCube1.ry = deg_to_rad(180) + rad_to_deg(0) + sind(90) + cosd(0) + hypot(3, 4) + fmod(7, 4) + linstep(0, 10, 5) + smoothstep(0, 10, 5);\n",
    );
    close(get(&mut s, "pCube1.tx"), 2.5 * 3.0 + 2.0);
    close(get(&mut s, "pCube1.ty"), 1.0);
    close(get(&mut s, "pCube1.tz"), 7.0);
    close(get(&mut s, "pCube1.sx"), 6.0);
    close(get(&mut s, "pCube1.sy"), 2.0);
    close(get(&mut s, "pCube1.sz"), 8.0);
    close(get(&mut s, "pCube1.rx"), 1.0 + 1.0 + 2.0 + 4.0 + 1.0 + 2.0 + 4.0 + 5.0);
    close(get(&mut s, "pCube1.ry"), std::f64::consts::PI + 0.0 + 1.0 + 1.0 + 5.0 + 3.0 + 0.5 + 0.5);
}

#[test]
fn noise_and_rand_stay_in_range_and_are_smooth() {
    let _t = TIME.lock().unwrap();
    let mut s = cube();
    expr(&mut s, "pCube1.tx = noise(frame * 0.37); pCube1.ty = rand(5); pCube1.tz = rand(2, 3); vector $d = dnoise(<<frame, 1.5, 2.5>>); pCube1.sx = $d.x; vector $r = sphrand(2); pCube1.sy = mag($r); pCube1.sz = gauss(1);");
    let mut prev = None;
    let mut varied = false;
    for f in 1..40 {
        frame(&mut s, f as f64);
        let n = get(&mut s, "pCube1.tx");
        assert!((-1.0..=1.0).contains(&n), "noise {n}");
        if let Some(p) = prev {
            assert!((n - p as f64).abs() < 0.8, "noise jumped from {p} to {n}");
            varied |= n != p;
        }
        prev = Some(n);
        assert!((0.0..5.0).contains(&get(&mut s, "pCube1.ty")));
        assert!((2.0..3.0).contains(&get(&mut s, "pCube1.tz")));
        assert!(get(&mut s, "pCube1.sy") <= 2.0 + 1e-9);
    }
    assert!(varied);
}

#[test]
fn errors_refuse_the_command_and_leave_nothing() {
    let mut s = cube();
    for bad in [
        "pCube1.tx = ;",
        "pCube1.tx = sin(;",
        "pCube1.tx = nosuchfunction(1);",
        "pCube1.nosuchattr = 1;",
        "nosuchobject.tx = 1;",
        "pCube1.tx = $missing;",
        "tx = 1;",
        "pCube1.tx = sin(1, 2);",
        "time = 3;",
        "if (1 { pCube1.tx = 1; }",
        "pCube1.translate = 1;",
        "pCube1.worldMatrix = 1;",
    ] {
        assert!(s.run("expression", json!({"string": bad})).is_err(), "{bad} should fail");
    }
    assert_eq!(run(&mut s, "ls", json!({"type": "expression"})), json!([]));
    // A channel already driven can't be taken.
    expr(&mut s, "pCube1.tx = 1;");
    assert!(s.run("expression", json!({"string": "pCube1.tx = 2;"})).is_err());
    assert_eq!(run(&mut s, "ls", json!({"type": "expression"})), json!(["expression1"]));
    // A locked one neither.
    run(&mut s, "setAttr", json!({"args": ["pCube1.ty"], "lock": true}));
    assert!(s.run("expression", json!({"string": "pCube1.ty = 2;"})).is_err());
}

#[test]
fn query_edit_and_rename() {
    let mut s = cube();
    let src = "pCube1.tx = 5;";
    run(&mut s, "expression", json!({"string": src, "object": "pCube1", "name": "e1", "unitConversion": "none", "alwaysEvaluate": 0}));
    let q = |s: &mut Session, what: &str| run(s, "expression", json!({"args": ["e1"], "query": true, "queried": [what]}));
    assert_eq!(q(&mut s, "string"), src);
    assert_eq!(q(&mut s, "object"), "pCube1");
    assert_eq!(q(&mut s, "alwaysEvaluate"), 0);
    assert_eq!(q(&mut s, "unitConversion"), "none");

    run(&mut s, "expression", json!({"args": ["e1"], "edit": true, "string": "pCube1.ty = 6;"}));
    close(get(&mut s, "pCube1.ty"), 6.0);
    // The old channel is let go.
    assert_eq!(run(&mut s, "listConnections", json!({"args": ["pCube1.tx"]})), json!([]));
    assert!(s.run("expression", json!({"args": ["e1"], "edit": true, "string": "pCube1.ty = ;"})).is_err());
    assert_eq!(q(&mut s, "string"), "pCube1.ty = 6;");
    close(get(&mut s, "pCube1.ty"), 6.0);

    // Source follows a rename, as the reference's does.
    run(&mut s, "rename", json!({"args": ["pCube1", "box"]}));
    assert_eq!(q(&mut s, "string"), "box.ty = 6;");
    close(get(&mut s, "box.ty"), 6.0);
}

#[test]
fn radians_with_no_unit_conversion() {
    let mut s = cube();
    run(&mut s, "expression", json!({"string": "pCube1.rx = 3.14159265358979;", "unitConversion": "none"}));
    close(get(&mut s, "pCube1.rx"), 180.0);
    run(&mut s, "expression", json!({"string": "pCube1.ry = 90;", "unitConversion": "all"}));
    close(get(&mut s, "pCube1.ry"), 90.0);
}

#[test]
fn undo_delete_and_files() {
    let _t = TIME.lock().unwrap();
    let mut s = cube();
    run(&mut s, "polyCube", json!({"name": "other"}));
    run(&mut s, "setAttr", json!({"args": ["other.tz", 2.0]}));
    expr(&mut s, "pCube1.tx = other.tz + frame;");
    frame(&mut s, 5.0);
    close(get(&mut s, "pCube1.tx"), 7.0);

    run(&mut s, "undo", json!({})); // the time isn't on the undo stack; this takes the expression back
    assert_eq!(run(&mut s, "ls", json!({"type": "expression"})), json!([]));
    run(&mut s, "redo", json!({}));
    close(get(&mut s, "pCube1.tx"), 7.0);

    // Saved and loaded.
    let doc = scene_file::serialize(&s.graph, false);
    let mut t = Session::new();
    scene_file::deserialize(&mut t.graph, &doc).unwrap();
    frame(&mut t, 5.0);
    close(get(&mut t, "pCube1.tx"), 7.0);
    frame(&mut t, 6.0);
    close(get(&mut t, "pCube1.tx"), 8.0);
    assert_eq!(run(&mut t, "expression", json!({"args": ["expression1"], "query": true, "queried": ["string"]})), "pCube1.tx = other.tz + frame;");

    // Deleting the expression frees the channel at its value.
    run(&mut s, "delete", json!({"args": ["expression1"]}));
    assert_eq!(run(&mut s, "listConnections", json!({"args": ["pCube1.tx"]})), json!([]));
    run(&mut s, "setAttr", json!({"args": ["pCube1.tx", 1.0]}));
    close(get(&mut s, "pCube1.tx"), 1.0);

    // Deleting what it drives takes the expression too.
    expr(&mut s, "other.tx = pCube1.ty;");
    run(&mut s, "delete", json!({"args": ["other"]}));
    assert_eq!(run(&mut s, "ls", json!({"type": "expression"})), json!([]));
    close(get(&mut s, "pCube1.ty"), 0.0);
}

#[test]
fn chains_of_expressions_evaluate_in_order() {
    let mut s = cube();
    run(&mut s, "polyCube", json!({"name": "b"}));
    run(&mut s, "polyCube", json!({"name": "c"}));
    // Created downstream first, so creation order isn't what makes this right.
    expr(&mut s, "c.tx = b.tx + 1;");
    expr(&mut s, "b.tx = pCube1.tx * 2;");
    run(&mut s, "setAttr", json!({"args": ["pCube1.tx", 5.0]}));
    close(get(&mut s, "c.tx"), 11.0);
}

#[test]
fn assigned_attribute_reads_its_last_value() {
    let _t = TIME.lock().unwrap();
    let mut s = cube();
    expr(&mut s, "float $x = frame; pCube1.tx = pCube1.tx + 1; pCube1.ty = pCube1.tx;");
    // Reads before an assignment in the same run see the last output; the channel really moves.
    let first = get(&mut s, "pCube1.tx");
    frame(&mut s, 3.0);
    let second = get(&mut s, "pCube1.tx");
    assert!(second > first);
    close(get(&mut s, "pCube1.ty"), second);
}

#[test]
fn namespaced_targets() {
    let mut s = Session::new();
    run(&mut s, "namespace", json!({"addNamespace": "rig"}));
    run(&mut s, "polyCube", json!({"name": "rig:box"}));
    expr(&mut s, "rig:box.tx = 4;");
    close(get(&mut s, "rig:box.tx"), 4.0);
    // A colon in a ternary stays an operator.
    expr(&mut s, "rig:box.ty = 1 ? 2:3;");
    close(get(&mut s, "rig:box.ty"), 2.0);
}
