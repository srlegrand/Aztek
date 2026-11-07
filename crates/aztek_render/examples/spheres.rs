//! A test scene with no scene graph: spheres of several materials on a floor under a sky and a key light.
//! `cargo run --release -p aztek_render --example spheres -- out.png [aa]`

use std::sync::Arc;

use aztek_render::camera::Camera;
use aztek_render::geom::{quad, sphere, Instance};
use aztek_render::lights::{Light, Shape};
use aztek_render::scene::{Options, Scene};
use aztek_render::shader::{Input, Material, Standard};
use glam::{Mat4, Vec3};

fn main() {
    let path = std::env::args().nth(1).unwrap_or("spheres.png".into());
    let aa: u32 = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(4);
    let mut s = Scene::new(Options { width: 640, height: 360, aa_samples: aa, aovs: vec!["diffuse".into(), "specular".into(), "transmission".into(), "albedo".into(), "N".into(), "Z".into()], ..Default::default() });
    s.camera = Camera::looking(Vec3::new(0.0, 2.0, 9.0), Vec3::new(0.0, 0.8, 0.0), 40.0);
    s.geo.meshes.push(Arc::new(sphere(Vec3::ZERO, 1.0, 64, 128)));
    s.geo.meshes.push(Arc::new(quad(Vec3::new(-50.0, 0.0, 50.0), Vec3::new(50.0, 0.0, 50.0), Vec3::new(50.0, 0.0, -50.0), Vec3::new(-50.0, 0.0, -50.0))));
    let std = |f: &dyn Fn(&mut Standard)| {
        let mut m = Standard::default();
        f(&mut m);
        Material::Standard(Box::new(m))
    };
    let floor = s.add_material("floor", std(&|m| {
        m.base_color = Input::c(0.5);
        m.specular_roughness = Input::c(0.5);
    }));
    let mats = [
        s.add_material("red", std(&|m| m.base_color = Input::rgb(0.8, 0.1, 0.1))),
        s.add_material("gold", std(&|m| {
            m.base = Input::c(1.0);
            m.metalness = Input::c(1.0);
            m.base_color = Input::rgb(1.0, 0.77, 0.34);
            m.specular_roughness = Input::c(0.25);
        })),
        s.add_material("glass", std(&|m| {
            m.base = Input::c(0.0);
            m.transmission = Input::c(1.0);
            m.specular_roughness = Input::c(0.0);
            m.transmission_color = Input::rgb(0.8, 0.95, 0.9);
        })),
        s.add_material("skin", std(&|m| {
            m.base = Input::c(1.0);
            m.subsurface = Input::c(1.0);
            m.subsurface_color = Input::rgb(0.9, 0.6, 0.5);
            m.subsurface_radius = Input::rgb(1.0, 0.35, 0.2);
            m.subsurface_scale = Input::c(0.3);
            m.specular_roughness = Input::c(0.35);
        })),
        s.add_material("car paint", std(&|m| {
            m.base_color = Input::rgb(0.05, 0.1, 0.5);
            m.coat = Input::c(1.0);
            m.coat_roughness = Input::c(0.02);
            m.specular_roughness = Input::c(0.4);
            m.metalness = Input::c(0.6);
        })),
    ];
    for (k, m) in mats.iter().enumerate() {
        let x = (k as f32 - 2.0) * 2.3;
        s.geo.instances.push(Instance::new("ball", 0, Mat4::from_translation(Vec3::new(x, 1.0, 0.0)), vec![*m]));
    }
    s.geo.instances.push(Instance::new("floor", 1, Mat4::IDENTITY, vec![floor]));
    s.lights.push(Light::new("sky", Shape::Dome { to_local: Mat4::IDENTITY, image: None, dist: None }, Vec3::new(0.4, 0.5, 0.7)));
    let mut key = Light::new("key", Shape::Disk { center: Vec3::new(4.0, 7.0, 5.0), frame: aztek_render::math::Frame::from_n(Vec3::new(-4.0, -7.0, -5.0).normalize()), radius: 1.0, two_sided: false }, Vec3::splat(30.0));
    key.samples = 2;
    s.lights.push(key);
    s.prepare();
    let t = std::time::Instant::now();
    let (film, st) = aztek_render::render::render(Arc::new(s), 0);
    eprintln!("{:.2}s, {} rays, {:.1} Mrays/s", t.elapsed().as_secs_f64(), st.rays, st.rays_per_second / 1e6);
    let p = std::path::Path::new(&path);
    let files = aztek_render::output::write_film(&film, p, aztek_render::output::Format::of_path(p).unwrap(), false, &[]).unwrap();
    eprintln!("{files:?}");
}
