//! Batch rendering a scene file with no interface:
//! `cargo run --release -p aztek_scene --example hrender -- file.ma out.png [camera] [width height] [aa] [frame]`

use aztek_scene::harold::{self, Request, Translator};
use aztek_scene::{ma_file, Session};
use std::sync::Arc;
use std::time::Instant;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let (file, out) = (&a[1], &a[2]);
    let camera = a.get(3).filter(|c| c.as_str() != "-").cloned();
    let w = a.get(4).and_then(|x| x.parse().ok());
    let h = a.get(5).and_then(|x| x.parse().ok());
    let aa: Option<i64> = a.get(6).and_then(|x| x.parse().ok());
    let frame: Option<f64> = a.get(7).and_then(|x| x.parse().ok());
    let mut s = Session::new();
    let t = Instant::now();
    let warnings = ma_file::open_scene(&mut s, file).expect("open");
    eprintln!("open {:.2}s, {} file warnings", t.elapsed().as_secs_f64(), warnings.len());
    if let Some(fr) = frame {
        s.graph.set_time(fr / aztek_scene::anim::fps());
    }
    if let Some(aa) = aa {
        let _ = s.run("setAttr", serde_json::json!({"args": ["defaultArnoldRenderOptions.AASamples", aa]}));
    }
    if std::env::var_os("HAROLD_LIGHTS").is_some() {
        // A sky and an angled key, for rigs that come with no lights.
        let run = |s: &mut Session, n: &str, v: serde_json::Value| {
            if let Err(e) = s.run(n, v) {
                eprintln!("{n}: {e}");
            }
        };
        run(&mut s, "createNode", serde_json::json!({"args": ["transform"], "name": "hSky"}));
        run(&mut s, "createNode", serde_json::json!({"args": ["aiSkyDomeLight"], "name": "hSkyShape", "parent": "hSky"}));
        run(&mut s, "setAttr", serde_json::json!({"args": ["hSkyShape.color", 0.55, 0.6, 0.7]}));
        run(&mut s, "directionalLight", serde_json::json!({"name": "hKey", "intensity": 2.5}));
        run(&mut s, "rotate", serde_json::json!({"args": [-40.0, 35.0, 0.0, "hKey"], "absolute": true}));
        run(&mut s, "setAttr", serde_json::json!({"args": ["hKeyShape.aiAngle", 3.0]}));
    }
    let t = Instant::now();
    let mut tr = Translator::default();
    let aovs: Vec<String> = std::env::var("HAROLD_AOVS").map(|v| v.split(',').map(str::to_string).collect()).unwrap_or_default();
    let (scene, _) = harold::translate(&mut s, &mut tr, &Request { camera, width: w, height: h, aovs, ..Default::default() }).expect("translate");
    let tris: usize = scene.geo.instances.iter().map(|i| scene.geo.meshes[i.mesh as usize].tris.len()).sum();
    eprintln!("translate {:.2}s: {} instances, {} triangles, {} lights, {} materials, camera {}", t.elapsed().as_secs_f64(), scene.geo.instances.len(), tris, scene.lights.len(), scene.materials.len(), scene.camera.name);
    for w in &scene.warnings {
        eprintln!("  {w}");
    }
    if let Ok(at) = std::env::var("HAROLD_PICK") {
        // Every surface a camera ray through pixel x,y meets, nearest first, with its material.
        let v: Vec<f32> = at.split(',').filter_map(|x| x.parse().ok()).collect();
        let (w, h) = (scene.options.width as f32, scene.options.height as f32);
        let r = scene.camera.ray(glam::Vec2::new(v[0] / w, v[1] / h), scene.aspect(), glam::Vec2::splat(0.5), 0.5);
        let mut hits = Vec::new();
        scene.geo.intersect_all(&r, f32::INFINITY, 255, |hh| {
            hits.push(hh);
            true
        });
        hits.sort_by(|a, b| a.t.total_cmp(&b.t));
        let tr2 = aztek_render::integrator::Tracer::new(&scene);
        for k in 0..4 {
            let mut smp = aztek_render::math::Sampler::new(v[0] as u32, v[1] as u32, k, 0);
            let o = tr2.camera_sample(glam::Vec2::new(v[0] / w, v[1] / h), &mut smp);
            eprintln!("sample {k}: colour {:.3} albedo {:.3} z {:.2} material {}", o.color, o.slots[aztek_render::first_hit::ALBEDO], o.z, scene.material_names[o.material as usize]);
        }
        for hh in hits {
            let sp = scene.geo.surface(&r, &hh);
            let inst = &scene.geo.instances[hh.inst as usize];
            eprintln!("t {:.3} {} material {} front {} n.wo {:.2} opaque {} uv {:.4} {:.4}", hh.t, inst.name, scene.material_names[sp.shader as usize], sp.front, sp.n.dot(-r.d), inst.opaque, sp.uv.x, sp.uv.y);
        }
        return;
    }
    if std::env::var_os("HAROLD_RAYBENCH").is_some() {
        // Intersection alone: camera rays through every pixel, many times, on every core.
        use rayon::prelude::*;
        let (w, h) = (scene.options.width, scene.options.height);
        let aspect = scene.aspect();
        let t = Instant::now();
        let reps = 8u32;
        let hits: usize = (0..h * reps)
            .into_par_iter()
            .map(|row| {
                let y = row % h;
                (0..w).filter(|&x| {
                    let r = scene.camera.ray(glam::Vec2::new((x as f32 + 0.5) / w as f32, (y as f32 + 0.5) / h as f32), aspect, glam::Vec2::splat(0.5), 0.5);
                    scene.geo.intersect(&r, f32::INFINITY, 255).is_some()
                }).count()
            })
            .sum();
        let n = (w * h * reps) as f64;
        eprintln!("raybench: {:.1} M camera rays/s, {:.0}% hit", n / t.elapsed().as_secs_f64() / 1e6, hits as f64 / n * 100.0);
        return;
    }
    if std::env::var_os("HAROLD_IPR").is_some() {
        // Interactive: time to the first coarse image, then after an edit the translation (meshes cached) and the
        // first coarse image again.
        use aztek_render::render::{JobOptions, Render};
        let wait_preview = |r: &Render| {
            let t = Instant::now();
            while !(r.status().preview || r.status().pass > 0) && t.elapsed().as_secs() < 60 {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            t.elapsed().as_secs_f64()
        };
        let t0 = Instant::now();
        let mut r = Render::start(Arc::new(scene), JobOptions { preview: true, threads: 0 });
        let first = wait_preview(&r);
        let to_pass = {
            let t = Instant::now();
            while r.status().pass == 0 && t.elapsed().as_secs() < 120 {
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            t0.elapsed().as_secs_f64()
        };
        let t = Instant::now();
        r.cancel();
        let cancel = t.elapsed().as_secs_f64();
        // An edit: a light's intensity.
        let _ = s.run("setAttr", serde_json::json!({"args": ["defaultArnoldRenderOptions.AASamples", 3]}));
        let t = Instant::now();
        let (scene2, _) = harold::translate(&mut s, &mut tr, &Request { camera: None, width: w, height: h, ..Default::default() }).expect("translate");
        let tr_s = t.elapsed().as_secs_f64();
        let r2 = Render::start(Arc::new(scene2), JobOptions { preview: true, threads: 0 });
        let again = wait_preview(&r2);
        eprintln!("ipr: first coarse image {first:.3}s, first full pass {to_pass:.2}s; restart: cancel {cancel:.3}s + translate {tr_s:.3}s (meshes reused {}, rebuilt {}) + coarse image {again:.3}s = {:.3}s", tr.reused, tr.built, cancel + tr_s + again);
        return;
    }
    let (film, st) = aztek_render::render::render(Arc::new(scene), 0);
    eprintln!("render {:.2}s, {} rays, {:.1} M rays/s, {:.0} MB", st.elapsed.as_secs_f64(), st.rays, st.rays_per_second / 1e6, st.memory_bytes as f64 / 1048576.0);
    for w in &st.warnings {
        eprintln!("  {w}");
    }
    let p = std::path::Path::new(out);
    let files = aztek_render::output::write_film(&film, p, aztek_render::output::Format::of_path(p).unwrap(), false, &[]).unwrap();
    eprintln!("{files:?}");
}
