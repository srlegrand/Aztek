//! One light model: Harold's direct light on a white matte surface equals the viewport's lighting formula
//! (`lights::viewport_diffuse`, which the viewport's shader mirrors) for every light type, decay rate and setting.
//! Lights with a surface are compared far enough away that one point of them stands for all of it, as the viewport takes
//! it; closer in, the soft light is what only the renderer has.

use std::sync::Arc;

use aztek_render::bsdf::{Bsdf, Category};
use aztek_render::geom::{quad, Instance, SurfacePoint};
use aztek_render::integrator::Tracer;
use aztek_render::math::Sampler;
use aztek_render::scene::{Options, Scene};
use aztek_scene::harold::lights::viewport_light;
use aztek_scene::lights::{scene_light, viewport_diffuse};
use aztek_scene::Session;
use glam::{DVec3, Mat4, Vec2, Vec3};
use serde_json::{json, Value};

fn run(s: &mut Session, name: &str, call: Value) -> Value {
    s.run(name, call).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn set(s: &mut Session, plug: &str, v: Value) {
    let mut args = vec![json!(plug)];
    match v {
        Value::Array(a) => args.extend(a),
        other => args.push(other),
    }
    run(s, "setAttr", json!({ "args": args }));
}

fn session() -> Session {
    let mut s = Session::new();
    aztek_scene::cameras::make_defaults(&mut s.graph);
    s
}

/// Points around the origin and normals turned various ways: what is compared.
fn probes() -> Vec<(DVec3, DVec3)> {
    let mut out = Vec::new();
    for (k, p) in [DVec3::new(0.0, 0.0, 0.0), DVec3::new(1.5, -0.5, 0.7), DVec3::new(-2.0, 0.3, -1.0), DVec3::new(0.4, 1.2, 2.5), DVec3::new(-0.8, -1.7, 0.2)].iter().enumerate() {
        for n in [DVec3::Y, DVec3::new(0.3, 0.8, -0.5), DVec3::new(-0.6, 0.2, 0.77), DVec3::new(0.1, -0.2, 1.0), DVec3::new(0.9, 0.4, 0.1)] {
            out.push((*p + DVec3::splat(k as f64 * 0.01), n.normalize()));
        }
    }
    out
}

/// Harold's direct light from one light on a white matte surface at p with normal n, averaged over samples.
fn harold_at(scene: &Scene, p: DVec3, n: DVec3, samples: u32) -> Vec3 {
    let t = Tracer::new(scene);
    let (pf, nf) = (p.as_vec3(), n.as_vec3());
    let sp = SurfacePoint { p: pf, ng: nf, n: nf, uv: Vec2::ZERO, dpdu: Vec3::X, dpdv: Vec3::Z, front: true, uv_density: 1.0, inst: 0, prim: 0, shader: 0 };
    let b = Bsdf::lambert(Vec3::ONE, nf, nf);
    let mut sum = Vec3::ZERO;
    for k in 0..samples {
        let mut sampler = Sampler::new(7, 11, k, 3);
        let mut groups = [Vec3::ZERO; aztek_render::integrator::MAX_GROUPS];
        // No BSDF samples are taken here, so the light's own carry full weight: cont is empty.
        let d = t.direct(&sp, &b, nf, Category::Diffuse.bit(), 0, &[1.0; 6], false, 0, 0.5, &mut sampler, &mut groups);
        sum += d[0];
    }
    sum / samples as f32
}

/// Compares the renderer and the viewport for the light shape `shape` in the session, at every probe.
fn compare(s: &mut Session, shape: &str, samples: u32, tol: f32, label: &str) {
    let id = s.graph.find(shape).unwrap().unwrap();
    s.graph.evaluate();
    let sl = scene_light(&mut s.graph, id).unwrap();
    let mut scene = Scene::new(Options { low_light_threshold: 0.0, ..Default::default() });
    // Something to hit far off, for the shadow rays' hierarchy; nothing in the way.
    scene.geo.meshes.push(Arc::new(quad(Vec3::new(1e5, 1e5, 1e5), Vec3::new(1e5 + 1.0, 1e5, 1e5), Vec3::new(1e5 + 1.0, 1e5 + 1.0, 1e5), Vec3::new(1e5, 1e5 + 1.0, 1e5))));
    scene.geo.instances.push(Instance::new("far", 0, Mat4::IDENTITY, vec![0]));
    scene.lights.push(viewport_light(&sl, "light", 0.0, 0.0, 0.0));
    scene.prepare();
    let mut worst = 0.0f32;
    for (p, n) in probes() {
        let want = viewport_diffuse(&sl, p, n).as_vec3();
        let got = harold_at(&scene, p, n, samples);
        let scale = want.max_element().max(1e-3);
        let err = (got - want).abs().max_element() / scale;
        worst = worst.max(err);
        assert!(err <= tol, "{label}: at {p} n {n}: renderer {got}, viewport {want}");
    }
    eprintln!("{label}: worst relative difference {worst:.5}");
}

#[test]
fn point_and_spot_lights_every_decay_rate() {
    for rate in 0..4 {
        let mut s = session();
        run(&mut s, "pointLight", json!({"name": "pt", "intensity": 3.0, "rgb": [1.0, 0.5, 0.25], "position": [0.5, 4.0, 1.0]}));
        set(&mut s, "ptShape.decayRate", json!(rate));
        compare(&mut s, "ptShape", 1, 1e-3, &format!("point, decay {rate}"));
        let mut s = session();
        run(&mut s, "spotLight", json!({"name": "sp", "intensity": 2.0, "rgb": [0.2, 1.0, 0.3], "coneAngle": 70.0}));
        set(&mut s, "spShape.decayRate", json!(rate));
        set(&mut s, "spShape.penumbraAngle", json!(10.0));
        set(&mut s, "spShape.dropoff", json!(2.0));
        run(&mut s, "move", json!({"args": [0.0, 5.0, 1.0, "sp"], "absolute": true}));
        run(&mut s, "rotate", json!({"args": [-80.0, 0.0, 0.0, "sp"], "absolute": true}));
        compare(&mut s, "spShape", 1, 1e-3, &format!("spot, decay {rate}"));
        // A negative penumbra narrows the edge inside the cone.
        set(&mut s, "spShape.penumbraAngle", json!(-15.0));
        compare(&mut s, "spShape", 1, 1e-3, &format!("spot, inner penumbra, decay {rate}"));
    }
}

#[test]
fn directional_and_ambient_lights() {
    let mut s = session();
    run(&mut s, "directionalLight", json!({"name": "sun", "intensity": 0.7, "rgb": [1.0, 0.95, 0.85]}));
    run(&mut s, "rotate", json!({"args": [-50.0, -30.0, 0.0, "sun"], "absolute": true}));
    compare(&mut s, "sunShape", 1, 1e-3, "directional");
    for shade in [0.0, 0.45, 1.0] {
        let mut s = session();
        run(&mut s, "ambientLight", json!({"name": "fill", "intensity": 0.6}));
        run(&mut s, "move", json!({"args": [2.0, 6.0, 1.0, "fill"], "absolute": true}));
        set(&mut s, "fillShape.ambientShade", json!(shade));
        // All round is sampled: a few thousand samples and a looser bound.
        compare(&mut s, "fillShape", 4096, 0.03, &format!("ambient, shade {shade}"));
    }
}

#[test]
fn emit_diffuse_off_takes_nothing() {
    let mut s = session();
    run(&mut s, "pointLight", json!({"name": "pt", "position": [0.0, 3.0, 0.0]}));
    set(&mut s, "ptShape.emitDiffuse", json!(false));
    compare(&mut s, "ptShape", 1, 1e-3, "point, emitDiffuse off");
}

#[test]
fn area_lights_far_off_every_decay_rate() {
    for rate in 0..4 {
        for normalize in [true, false] {
            let mut s = session();
            run(&mut s, "shadingNode", json!({"args": ["areaLight"], "asLight": true}));
            set(&mut s, "areaLightShape1.decayRate", json!(rate));
            set(&mut s, "areaLightShape1.normalize", json!(normalize));
            set(&mut s, "areaLightShape1.intensity", json!(50.0));
            // Small and far: the viewport lights from the rectangle's nearest point, the renderer from all of it; 200 units
            // off they agree to a fraction of a percent. Closer, the difference is the soft light only a renderer has.
            run(&mut s, "move", json!({"args": [0.0, 200.0, 0.0, "areaLight1"], "absolute": true}));
            run(&mut s, "rotate", json!({"args": [-90.0, 0.0, 0.0, "areaLight1"], "absolute": true}));
            run(&mut s, "scale", json!({"args": [0.2, 0.3, 1.0, "areaLight1"], "absolute": true}));
            compare(&mut s, "areaLightShape1", 64, 0.01, &format!("area, decay {rate}, normalize {normalize}"));
        }
    }
}

#[test]
fn volume_lights_every_shape_and_direction() {
    for shape in 0..4 {
        for dir in 0..3 {
            for ambient in [false, true] {
                let mut s = session();
                run(&mut s, "shadingNode", json!({"args": ["volumeLight"], "asLight": true}));
                set(&mut s, "volumeLightShape1.lightShape", json!(shape));
                set(&mut s, "volumeLightShape1.volumeLightDir", json!(dir));
                set(&mut s, "volumeLightShape1.emitAmbient", json!(ambient));
                set(&mut s, "volumeLightShape1.intensity", json!(2.0));
                run(&mut s, "move", json!({"args": [0.3, 1.0, 0.2, "volumeLight1"], "absolute": true}));
                run(&mut s, "scale", json!({"args": [4.0, 3.0, 5.0, "volumeLight1"], "absolute": true}));
                let samples = if ambient { 4096 } else { 1 };
                let tol = if ambient { 0.03 } else { 1e-3 };
                compare(&mut s, "volumeLightShape1", samples, tol, &format!("volume, shape {shape}, direction {dir}, ambient {ambient}"));
            }
        }
    }
}
