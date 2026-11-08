//! Dynamics in real units: a centimetre of geometry is a centimetre of cloth. The same real object behaves the same
//! in millimetre, centimetre and metre scenes; the same cloth folds alike at two resolutions; silk drapes in finer folds
//! than denim; things fall as gravity says. One test function, as the scene unit is shared by the process.

use aztek_scene::{PlugRef, Session};
use glam::DVec3;
use serde_json::{json, Value};

fn run(s: &mut Session, name: &str, call: Value) -> Value {
    s.run(name, call).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn frame(s: &mut Session, f: f64) {
    run(s, "currentTime", json!({"args": [f]}));
    s.graph.evaluate();
}

fn points(s: &mut Session, name: &str) -> Vec<DVec3> {
    let id = s.graph.find(name).unwrap().unwrap();
    let shape = aztek_scene::components::as_mesh(&s.graph, id).unwrap();
    let out = s.graph.node(shape).unwrap().ty().attr_index("outMesh").unwrap();
    let m = s.graph.value(&PlugRef::new(shape, out as i32)).unwrap();
    let mesh = m.as_mesh().unwrap().clone();
    let xf = aztek_scene::dag::transform_of(&s.graph, shape);
    let w = aztek_scene::rig::world(&mut s.graph, xf);
    mesh.positions().iter().map(|p| w.transform_point3(*p)).collect()
}

/// A square of `size` metres, `res` quads a side, a metre up over a ball `ball` metres across, in a scene whose unit is
/// `per` metres; after `frames` frames, its points in metres.
fn drape(unit: &str, per: f64, size: f64, res: i64, ball: f64, preset: &str, frames: i64) -> Vec<DVec3> {
    let mut s = Session::new();
    run(&mut s, "currentUnit", json!({"linear": unit}));
    run(&mut s, "playbackOptions", json!({"minTime": 1, "maxTime": 200}));
    let u = |metres: f64| metres / per;
    run(&mut s, "polySphere", json!({"name": "ball", "radius": u(ball / 2.0), "sx": 32, "sy": 16}));
    run(&mut s, "polyPlane", json!({"name": "sheet", "width": u(size), "height": u(size), "sx": res, "sy": res}));
    run(&mut s, "move", json!({"args": [0, u(ball / 2.0 + 0.3), 0, "sheet"]}));
    frame(&mut s, 1.0);
    run(&mut s, "createCloth", json!({"args": ["sheet"]}));
    run(&mut s, "collider", json!({"args": ["ball"]}));
    run(&mut s, "clothPreset", json!({"args": ["sheet"], "preset": preset}));
    for f in 1..=frames {
        frame(&mut s, f as f64);
    }
    let p = points(&mut s, "sheet").into_iter().map(|p| p * per).collect();
    run(&mut s, "currentUnit", json!({"linear": "cm"}));
    p
}

/// How many folds hang around the ball: local peaks in how far the outer edge reaches, going round.
fn folds(p: &[DVec3], res: usize) -> usize {
    let w = res + 1;
    let mut edge: Vec<(f64, f64)> = Vec::new();
    for j in 0..w {
        for i in 0..w {
            if i == 0 || j == 0 || i == res || j == res {
                let q = p[j * w + i];
                edge.push((q.z.atan2(q.x), (q.x * q.x + q.z * q.z).sqrt()));
            }
        }
    }
    edge.sort_by(|a, b| a.0.total_cmp(&b.0));
    let n = edge.len();
    // Smoothed over a few samples to drop contact jitter, then peaks that stand 2 cm above everything within four
    // samples either side: folds, not noise.
    let r: Vec<f64> = (0..n).map(|k| (0..7).map(|d| edge[(k + n + d - 3) % n].1).sum::<f64>() / 7.0).collect();
    (0..n)
        .filter(|&k| (1..=4).all(|d| r[k] > r[(k + d) % n] && r[k] > r[(k + n - d) % n]) && (1..=4).any(|d| r[k] - r[(k + d) % n] > 0.02 || r[k] - r[(k + n - d) % n] > 0.02))
        .count()
}

fn fall(unit: &str, per: f64) -> f64 {
    let mut s = Session::new();
    run(&mut s, "currentUnit", json!({"linear": unit}));
    run(&mut s, "playbackOptions", json!({"minTime": 1, "maxTime": 60}));
    let made = run(&mut s, "particle", json!({"position": [[0, 0, 0]]}));
    for f in 1..=25 {
        frame(&mut s, f as f64);
    }
    let y = run(&mut s, "getParticleAttr", json!({"args": [made[1].clone()], "attribute": "position"}))[1].as_f64().unwrap();
    run(&mut s, "currentUnit", json!({"linear": "cm"}));
    y * per
}

#[test]
fn real_units() {
    // Free fall: half g t squared, in every unit.
    for (unit, per) in [("mm", 0.001), ("cm", 0.01), ("m", 1.0)] {
        let y = fall(unit, per);
        assert!((y + 0.5 * 9.81).abs() < 0.05, "{unit}: fell {y} m in a second");
    }

    // The same real drape in millimetres, centimetres and metres: the same, point for point.
    let in_cm = drape("cm", 0.01, 2.0, 16, 1.0, "cotton", 24);
    for (unit, per) in [("mm", 0.001), ("m", 1.0)] {
        let other = drape(unit, per, 2.0, 16, 1.0, "cotton", 24);
        let worst = in_cm.iter().zip(&other).map(|(a, b)| (*a - *b).length()).fold(0.0, f64::max);
        assert!(worst < 1e-6, "{unit} against cm: {worst} m apart");
    }
    assert!(in_cm.iter().map(|p| p.y).fold(f64::INFINITY, f64::min) < 0.0, "it fell over the ball");

    // Two resolutions of the same cotton square settle to the same shape: hanging as low, reaching as wide.
    let coarse = drape("cm", 0.01, 2.0, 20, 1.0, "cotton", 60);
    let fine = drape("cm", 0.01, 2.0, 40, 1.0, "cotton", 60);
    let low = |p: &[DVec3]| p.iter().map(|q| q.y).fold(f64::INFINITY, f64::min);
    let wide = |p: &[DVec3]| p.iter().map(|q| (q.x * q.x + q.z * q.z).sqrt()).fold(0.0, f64::max);
    assert!((low(&coarse) - low(&fine)).abs() < 0.08, "lowest {} against {}", low(&coarse), low(&fine));
    assert!((wide(&coarse) - wide(&fine)).abs() < 0.1, "widest {} against {}", wide(&coarse), wide(&fine));

    // A 3 m square of silk over a ball drapes in fine folds; denim in fewer, bigger ones.
    let silk = drape("cm", 0.01, 3.0, 50, 1.0, "silk", 72);
    let denim = drape("cm", 0.01, 3.0, 50, 1.0, "heavyDenim", 72);
    let (fs, fd) = (folds(&silk, 50), folds(&denim, 50));
    eprintln!("folds: silk {fs}, denim {fd}; silk lowest {:.2} m, denim {:.2} m", low(&silk), low(&denim));
    assert!(fs >= 6, "silk folds {fs}");
    assert!(fs > fd, "silk {fs} folds against denim {fd}");
}
