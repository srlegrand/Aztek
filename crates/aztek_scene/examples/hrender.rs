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
    let t = Instant::now();
    let mut tr = Translator::default();
    let aovs: Vec<String> = std::env::var("HAROLD_AOVS").map(|v| v.split(',').map(str::to_string).collect()).unwrap_or_default();
    let (scene, _) = harold::translate(&mut s, &mut tr, &Request { camera, width: w, height: h, aovs, ..Default::default() }).expect("translate");
    let tris: usize = scene.geo.instances.iter().map(|i| scene.geo.meshes[i.mesh as usize].tris.len()).sum();
    eprintln!("translate {:.2}s: {} instances, {} triangles, {} lights, {} materials, camera {}", t.elapsed().as_secs_f64(), scene.geo.instances.len(), tris, scene.lights.len(), scene.materials.len(), scene.camera.name);
    for w in &scene.warnings {
        eprintln!("  {w}");
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
    let (film, st) = aztek_render::render::render(Arc::new(scene), 0);
    eprintln!("render {:.2}s, {} rays, {:.1} M rays/s, {:.0} MB", st.elapsed.as_secs_f64(), st.rays, st.rays_per_second / 1e6, st.memory_bytes as f64 / 1048576.0);
    for w in &st.warnings {
        eprintln!("  {w}");
    }
    let p = std::path::Path::new(out);
    let files = aztek_render::output::write_film(&film, p, aztek_render::output::Format::of_path(p).unwrap(), false, &[]).unwrap();
    eprintln!("{files:?}");
}
