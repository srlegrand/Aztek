//! Tests: soft select.

use std::sync::Mutex;

use aztek_core::{PrefScope, PrefStore};
use aztek_geo::{Index, Mesh};
use aztek_scene::mesh_edit::transform::soft_weights;
use aztek_scene::soft_select::*;
use aztek_scene::{Graph, Session};
use glam::{DMat4, DVec2, DVec3};
use serde_json::json;

static SERIAL: Mutex<()> = Mutex::new(());

fn approx(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-5 * (1.0 + a.abs().max(b.abs()))
}

fn approx_eps(a: f64, b: f64, eps: f64) -> bool {
    (a - b).abs() < eps * (1.0 + a.abs().max(b.abs()))
}

fn prefs() -> &'static PrefStore {
    std::env::set_var("AZTEK_CONFIG_DIR", std::env::temp_dir().join("aztek_test_config"));
    PrefStore::get()
}

/// A row of points 1 cm apart along X, joined by edges.
fn row(n: u32) -> Mesh {
    let mut m = Mesh::default();
    for i in 0..n {
        m.add_point(DVec3::new(i as f64, 0.0, 0.0));
    }
    for i in 0..n - 1 {
        m.add_edge(i as Index, i as Index + 1);
    }
    m
}

fn reset_prefs() {
    let p = prefs();
    p.remove("general", CURVE, PrefScope::User);
    p.remove("general", FALLOFF_MODE, PrefScope::User);
    p.remove("general", USER_PRESETS, PrefScope::User);
    p.set("general", ENABLED, json!(true), PrefScope::User);
    p.set("general", RADIUS, json!(10.0), PrefScope::User);
}

fn k(pos: f64, value: f64, interp: Interp) -> Key {
    Key { pos, value, interp }
}

#[test]
fn default_matches_smoothstep_and_holds_its_ends() {
    let c = Curve::default();
    for t in [0.0, 0.1, 0.3, 0.5, 0.77, 1.0] {
        assert!(approx(c.evaluate(t), 1.0 - t * t * (3.0 - 2.0 * t)));
    }
    assert!(approx(c.evaluate(-1.0), 1.0));
    assert!(approx(c.evaluate(2.0), 0.0));
    assert_eq!(Curve::new(Vec::new()), Curve::default());
}

#[test]
fn interpolation_modes() {
    let lin = Curve::new(vec![k(0.0, 1.0, Interp::Linear), k(1.0, 0.0, Interp::Linear)]);
    assert!(approx(lin.evaluate(0.25), 0.75));

    let flat = Curve::new(vec![k(0.0, 0.8, Interp::Flat), k(0.5, 0.2, Interp::Flat), k(1.0, 0.0, Interp::Flat)]);
    assert!(approx(flat.evaluate(0.49), 0.8));
    assert!(approx(flat.evaluate(0.51), 0.2));

    let smooth = Curve::new(vec![k(0.0, 1.0, Interp::Smooth), k(0.5, 0.0, Interp::Smooth), k(1.0, 1.0, Interp::Smooth)]);
    assert!(approx(smooth.evaluate(0.25), 0.5));
    assert!(approx(smooth.evaluate(0.75), 0.5));

    // Each segment uses the interpolation of the key it starts at.
    let mixed = Curve::new(vec![k(0.0, 1.0, Interp::Linear), k(0.5, 0.5, Interp::Flat), k(1.0, 0.0, Interp::Linear)]);
    assert!(approx(mixed.evaluate(0.25), 0.75));
    assert!(approx(mixed.evaluate(0.75), 0.5));

    // Keys are sorted and clamped into the unit square.
    let messy = Curve::new(vec![k(1.5, -2.0, Interp::Linear), k(-1.0, 3.0, Interp::Linear)]);
    assert_eq!(messy.keys().len(), 2);
    assert_eq!(messy.keys()[0].pos, 0.0);
    assert_eq!(messy.keys()[0].value, 1.0);
    assert_eq!(messy.keys()[1].pos, 1.0);
    assert_eq!(messy.keys()[1].value, 0.0);
}

#[test]
fn spline_is_monotone_between_keys_and_passes_through_them() {
    let c = Curve::new(vec![
        k(0.0, 1.0, Interp::Spline),
        k(0.1, 0.95, Interp::Spline),
        k(0.15, 0.2, Interp::Spline),
        k(0.8, 0.15, Interp::Spline),
        k(1.0, 0.0, Interp::Spline),
    ]);
    for key in c.keys() {
        assert!(approx(c.evaluate(key.pos), key.value));
    }
    let mut prev = c.evaluate(0.0);
    for i in 1..=1000 {
        let v = c.evaluate(i as f64 / 1000.0);
        assert!(v <= prev + 1e-12);
        assert!((0.0..=1.0).contains(&v));
        prev = v;
    }
    // A local extremum key stays the extremum: no overshoot past it.
    let bump = Curve::new(vec![k(0.0, 0.0, Interp::Spline), k(0.5, 1.0, Interp::Spline), k(1.0, 0.0, Interp::Spline)]);
    for i in 0..=100 {
        assert!(bump.evaluate(i as f64 / 100.0) <= 1.0);
    }
}

#[test]
fn editing_and_json() {
    let mut c = Curve::default();
    let i = c.add_key(k(0.5, 0.25, Interp::Linear));
    assert_eq!(i, 1);
    assert!(approx(c.evaluate(0.5), 0.25));
    let i = c.move_key(1, 0.2, 0.9);
    assert_eq!(i, 1);
    assert!(approx(c.keys()[1].pos, 0.2));
    c.set_interp(1, Interp::Flat);
    assert!(approx(c.evaluate(0.6), 0.9));

    assert_eq!(Curve::from_json(&c.to_json()), c);
    let j: serde_json::Value = serde_json::from_str(r#"[{"pos": 0, "value": 0.5, "interp": "linear"}, {"pos": 1, "value": 0.5}]"#).unwrap();
    assert!(approx(Curve::from_json(&j).evaluate(0.3), 0.5));
    assert_eq!(Curve::from_json(&json!("nonsense")), Curve::default());

    c.remove_key(1);
    assert_eq!(c.keys().len(), 2);
    c.remove_key(0);
    assert_eq!(c.keys().len(), 2);
}

#[test]
fn rdp_simplification() {
    let line: Vec<DVec2> = (0..=10).map(|i| DVec2::new(i as f64 / 10.0, 1.0 - i as f64 / 10.0)).collect();
    let s = simplify(&line, 0.01);
    assert_eq!(s.len(), 2);
    assert_eq!(s[0], line[0]);
    assert_eq!(*s.last().unwrap(), *line.last().unwrap());

    let corner = [DVec2::new(0.0, 0.0), DVec2::new(0.25, 0.25), DVec2::new(0.5, 0.5), DVec2::new(0.75, 0.25), DVec2::new(1.0, 0.0)];
    let kept = simplify(&corner, 0.01);
    assert_eq!(kept.len(), 3);
    assert_eq!(kept[1], DVec2::new(0.5, 0.5));
    assert_eq!(simplify(&corner, 1.0).len(), 2);

    // A drawn stroke, backwards and short of the edges, still makes a curve over the whole range.
    let stroke: Vec<DVec2> = (10..=90).rev().map(|i| DVec2::new(i as f64 / 100.0, 1.0 - i as f64 / 100.0)).collect();
    let c = curve_from_stroke(&stroke, 0.01);
    assert_eq!(c.keys()[0].pos, 0.0);
    assert_eq!(c.keys().last().unwrap().pos, 1.0);
    assert!(c.keys().len() <= 4);
    assert!(approx(c.evaluate(0.0), 0.9));
    assert!(approx_eps(c.evaluate(0.5), 0.5, 0.02));
    assert!(approx(c.evaluate(1.0), 0.1));
}

#[test]
fn falloff_presets() {
    let expect = [
        ("Smooth", 1.0, 0.5, 0.0),
        ("Linear", 1.0, 0.5, 0.0),
        ("Sharp", 1.0, 0.12, 0.0),
        ("Dome", 1.0, 0.87, 0.0),
        ("Bell", 1.0, 0.3, 0.0),
        ("Spike", 1.0, 0.03, 0.0),
        ("Plateau", 1.0, 1.0, 0.0),
        ("Inverse", 0.0, 0.5, 1.0),
        ("Ring", 1.0, 0.7, 0.0),
    ];
    for (name, at0, half, at1) in expect {
        let p = find_builtin(name).unwrap();
        assert!(approx(p.curve.evaluate(0.0), at0), "{name}");
        assert!(approx(p.curve.evaluate(0.5), half), "{name}");
        assert!(approx(p.curve.evaluate(1.0), at1), "{name}");
    }
    assert_eq!(builtin_presets().len(), 9);
    assert_eq!(builtin_presets()[0].curve, Curve::default());
}

#[test]
fn soft_weights_follows_the_curve_and_falloff_mode() {
    let _l = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    reset_prefs();
    let g = Graph::new();
    let mut m = row(21);
    let world = DMat4::IDENTITY;
    let sel = [0];

    let w = soft_weights(&g, &m, &world, &sel);
    assert_eq!(w[0], 1.0);
    assert!(approx(w[5] as f64, 0.5));
    assert_eq!(w[10], 0.0);

    set_current_curve(&find_builtin("Plateau").unwrap().curve);
    let w = soft_weights(&g, &m, &world, &sel);
    assert!(approx(w[5] as f64, 1.0));
    assert_eq!(w[0], 1.0);

    // Bend the row back on itself: point 20 sits 1 cm from point 0 in space but 20 cm away along the edges.
    m.set_position(20, DVec3::new(0.0, 1.0, 0.0));
    set_current_curve(&find_builtin("Linear").unwrap().curve);
    let w = soft_weights(&g, &m, &world, &sel);
    assert!(approx(w[20] as f64, 0.9));
    prefs().set("general", FALLOFF_MODE, json!("surface"), PrefScope::User);
    let w = soft_weights(&g, &m, &world, &sel);
    assert_eq!(w[20], 0.0);
    assert!(approx(w[3] as f64, 0.7));

    prefs().set("general", ENABLED, json!(false), PrefScope::User);
    let w = soft_weights(&g, &m, &world, &sel);
    assert_eq!(w[3], 0.0);
    reset_prefs();
    prefs().set("general", ENABLED, json!(false), PrefScope::User);
}

#[test]
fn soft_select_curve_command_and_user_presets() {
    let _l = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    reset_prefs();
    let mut s = Session::new();
    let r = s.run("softSelectCurve", json!({"preset": "Linear"})).unwrap();
    assert_eq!(Curve::from_json(&r["keys"]), find_builtin("Linear").unwrap().curve);
    assert_eq!(r["falloffMode"], "volume");

    s.run("softSelectCurve", json!({"keys": [[0, 1, "flat"], [0.5, 0.25, "linear"], [1, 0, "linear"]], "save": "Mine"})).unwrap();
    assert!(approx(current_curve().evaluate(0.25), 1.0));
    assert_eq!(user_presets().len(), 1);
    assert_eq!(user_presets()[0].name, "Mine");

    s.run("softSelectCurve", json!({"preset": "Smooth"})).unwrap();
    assert_eq!(current_curve(), Curve::default());
    s.run("softSelectCurve", json!({"preset": "Mine"})).unwrap();
    assert!(approx(current_curve().evaluate(0.75), 0.125));

    let r = s.run("softSelectCurve", json!({"rename": "Mine", "to": "Ours"})).unwrap();
    assert_eq!(r["userPresets"], json!(["Ours"]));
    assert!(s.run("softSelectCurve", json!({"preset": "Mine"})).is_err());
    assert!(s.run("softSelectCurve", json!({"save": "Smooth"})).is_err());
    s.run("softSelectCurve", json!({"remove": "Ours"})).unwrap();
    assert!(user_presets().is_empty());
    assert!(s.run("softSelectCurve", json!({"falloffMode": "sideways"})).is_err());
    s.run("softSelectCurve", json!({"falloffMode": "surface"})).unwrap();
    assert!(surface_falloff());
    reset_prefs();
    prefs().set("general", ENABLED, json!(false), PrefScope::User);
}
