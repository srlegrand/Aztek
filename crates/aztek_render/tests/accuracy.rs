//! Numerical checks of the renderer against known answers. Small images, few threads, fast.

use std::sync::Arc;

use aztek_render::camera::Camera;
use aztek_render::geom::{quad, sphere, Instance};
use aztek_render::lights::{Light, Shape};
use aztek_render::scene::{Options, Scene};
use aztek_render::shader::{Input, Material, Standard};
use glam::{Mat4, Vec3, Vec4};

fn opts(w: u32, h: u32, aa: u32) -> Options {
    Options { width: w, height: h, aa_samples: aa, diffuse_samples: 2, specular_samples: 2, diffuse_depth: 16, specular_depth: 16, total_depth: 32, low_light_threshold: 0.0, indirect_clamp: 1e9, filter_width: 1.0, ..Default::default() }
}

fn white(spec: f32) -> Material {
    Material::Standard(Box::new(Standard { base: Input::c(1.0), base_color: Input::c(1.0), specular: Input::c(spec), ..Default::default() }))
}

fn mean(img: &[Vec4], mask: impl Fn(usize) -> bool) -> Vec3 {
    let (mut s, mut n) = (Vec3::ZERO, 0);
    for (i, p) in img.iter().enumerate() {
        if mask(i) {
            s += p.truncate();
            n += 1;
        }
    }
    s / n.max(1) as f32
}

fn render(scene: Scene) -> (u32, Vec<Vec4>) {
    let w = scene.options.width;
    let mut scene = scene;
    scene.prepare();
    let (film, _) = aztek_render::render::render(Arc::new(scene), 2);
    let i = film.index_of("RGBA").unwrap();
    (w, film.pixels(i))
}

/// A white sphere in a uniform white environment returns the environment: nothing is lost or made.
#[test]
fn furnace_diffuse() {
    let mut s = Scene::new(opts(32, 32, 4));
    s.camera = Camera::looking(Vec3::new(0.0, 0.0, 5.0), Vec3::ZERO, 50.0);
    s.geo.meshes.push(Arc::new(sphere(Vec3::ZERO, 1.0, 48, 96)));
    let m = s.add_material("white", white(0.0));
    s.geo.instances.push(Instance::new("ball", 0, Mat4::IDENTITY, vec![m]));
    s.lights.push(Light::new("dome", Shape::Dome { to_local: Mat4::IDENTITY, image: None, dist: None }, Vec3::ONE));
    let (w, img) = render(s);
    // The middle of the image is all sphere.
    let c = mean(&img, |i| {
        let (x, y) = ((i as u32 % w) as i32 - 16, (i as u32 / w) as i32 - 16);
        x * x + y * y < 36
    });
    assert!((c.x - 1.0).abs() < 0.03, "furnace: {c}");
}

/// The same with the full white standard surface: specular over diffuse keeps nearly everything.
#[test]
fn furnace_standard() {
    let mut s = Scene::new(opts(32, 32, 4));
    s.camera = Camera::looking(Vec3::new(0.0, 0.0, 5.0), Vec3::ZERO, 50.0);
    s.geo.meshes.push(Arc::new(sphere(Vec3::ZERO, 1.0, 48, 96)));
    let m = s.add_material("white", white(1.0));
    s.geo.instances.push(Instance::new("ball", 0, Mat4::IDENTITY, vec![m]));
    s.lights.push(Light::new("dome", Shape::Dome { to_local: Mat4::IDENTITY, image: None, dist: None }, Vec3::ONE));
    let (w, img) = render(s);
    let c = mean(&img, |i| {
        let (x, y) = ((i as u32 % w) as i32 - 16, (i as u32 / w) as i32 - 16);
        x * x + y * y < 36
    });
    assert!(c.x > 0.95 && c.x < 1.03, "furnace standard: {c}");
}

/// A point light above a lambert plane: radiance is albedo / pi * I cos / d^2.
#[test]
fn point_light_on_plane() {
    let mut s = Scene::new(opts(16, 16, 2));
    s.camera = Camera::looking(Vec3::new(0.0, 0.0, 3.0), Vec3::ZERO, 200.0);
    let q = quad(Vec3::new(-10.0, -10.0, 0.0), Vec3::new(10.0, -10.0, 0.0), Vec3::new(10.0, 10.0, 0.0), Vec3::new(-10.0, 10.0, 0.0));
    s.geo.meshes.push(Arc::new(q));
    let m = s.add_material("grey", Material::Standard(Box::new(Standard { base: Input::c(1.0), base_color: Input::c(0.5), specular: Input::c(0.0), ..Default::default() })));
    s.geo.instances.push(Instance::new("plane", 0, Mat4::IDENTITY, vec![m]));
    s.lights.push(Light::new("pt", Shape::Point { pos: Vec3::new(0.0, 0.0, 2.0), radius: 0.0 }, Vec3::splat(10.0)));
    s.options.diffuse_depth = 0;
    let (_, img) = render(s);
    let c = mean(&img, |_| true);
    // Telephoto: the view is a small patch at the centre, cos about 1 and d = 2.
    let expect = 0.5 / std::f32::consts::PI * 10.0 / 4.0;
    assert!((c.x - expect).abs() / expect < 0.02, "{} vs {expect}", c.x);
}

fn sphere_scene(m: Material, aa: u32) -> Scene {
    let mut s = Scene::new(opts(32, 32, aa));
    s.options.transmission_depth = 32;
    s.camera = Camera::looking(Vec3::new(0.0, 0.0, 5.0), Vec3::ZERO, 50.0);
    s.geo.meshes.push(Arc::new(sphere(Vec3::ZERO, 1.0, 64, 128)));
    let m = s.add_material("m", m);
    s.geo.instances.push(Instance::new("ball", 0, Mat4::IDENTITY, vec![m]));
    s.lights.push(Light::new("dome", Shape::Dome { to_local: Mat4::IDENTITY, image: None, dist: None }, Vec3::ONE));
    s
}

fn middle(w: u32, img: &[Vec4]) -> Vec3 {
    mean(img, |i| {
        let (x, y) = ((i as u32 % w) as i32 - 16, (i as u32 / w) as i32 - 16);
        x * x + y * y < 36
    })
}

fn std_with(f: impl Fn(&mut Standard)) -> Material {
    let mut m = Standard::default();
    f(&mut m);
    Material::Standard(Box::new(m))
}

/// A smooth white metal in a white environment reflects all of it.
#[test]
fn furnace_mirror() {
    let (w, img) = render(sphere_scene(std_with(|m| {
        m.base = Input::c(1.0);
        m.metalness = Input::c(1.0);
        m.specular_roughness = Input::c(0.0);
    }), 2));
    let c = middle(w, &img);
    assert!((c.x - 1.0).abs() < 0.01, "mirror: {c}");
}

/// Clear glass in a white environment: what isn't reflected goes through and comes out; nothing is lost.
#[test]
fn furnace_glass() {
    let (w, img) = render(sphere_scene(std_with(|m| {
        m.base = Input::c(0.0);
        m.transmission = Input::c(1.0);
        m.specular_roughness = Input::c(0.0);
    }), 3));
    let c = middle(w, &img);
    assert!((c.x - 1.0).abs() < 0.03, "glass: {c}");
}

/// Rough white metal: single scattering GGX loses energy at high roughness; the multiple scattering term returns most.
/// The figure printed is what's left of the environment (1 is perfect); at roughness 1 about a tenth is still lost.
#[test]
fn furnace_rough_metal() {
    for rough in [0.3f32, 0.6, 1.0] {
        let (w, img) = render(sphere_scene(std_with(|m| {
            m.base = Input::c(1.0);
            m.metalness = Input::c(1.0);
            m.specular_roughness = Input::c(rough);
        }), 4));
        let c = middle(w, &img);
        eprintln!("rough metal {rough}: {:.4}", c.x);
        // Measured: 1.001 at 0.3, 0.999 at 0.6, 0.90 at 1.0, where the compensation's table runs out.
        let floor = if rough < 0.9 { 0.97 } else { 0.88 };
        assert!(c.x > floor && c.x < 1.03, "rough metal {rough}: {c}");
    }
}

/// A 1 x 1 quad light of radiance L, 1 above a lambert plane, facing it: radiance off the plane is albedo * L * F, F
/// the point to rectangle form factor. Light and BSDF samples are combined by MIS; wrong weights would miss this.
#[test]
fn quad_light_form_factor() {
    let corner = |a: f32, b: f32, c: f32| {
        let (x, y) = (a / c, b / c);
        let (sx, sy) = ((1.0 + x * x).sqrt(), (1.0 + y * y).sqrt());
        (x / sx * (y / sx).atan() + y / sy * (x / sy).atan()) / (2.0 * std::f32::consts::PI)
    };
    let f = 4.0 * corner(0.5, 0.5, 1.0);
    let mut s = Scene::new(opts(8, 8, 8));
    s.options.diffuse_depth = 0;
    let mut cam = Camera::looking(Vec3::new(0.0, 0.0, 0.5), Vec3::ZERO, 50.0);
    cam.orthographic = true;
    cam.ortho_width = 0.01;
    s.camera = cam;
    let q = quad(Vec3::new(-10.0, -10.0, 0.0), Vec3::new(10.0, -10.0, 0.0), Vec3::new(10.0, 10.0, 0.0), Vec3::new(-10.0, 10.0, 0.0));
    s.geo.meshes.push(Arc::new(q));
    let m = s.add_material("grey", std_with(|m| {
        m.base = Input::c(1.0);
        m.base_color = Input::c(0.5);
        m.specular = Input::c(0.0);
    }));
    s.geo.instances.push(Instance::new("plane", 0, Mat4::IDENTITY, vec![m]));
    s.lights.push(Light::new("quad", Shape::Quad { corner: Vec3::new(-0.5, -0.5, 1.0), u: Vec3::X, v: Vec3::Y, n: -Vec3::Z, two_sided: false, spread: 1.0 }, Vec3::splat(4.0)));
    let (_, img) = render(s);
    let c = mean(&img, |_| true);
    let expect = 0.5 * 4.0 * f;
    assert!((c.x - expect).abs() / expect < 0.01, "{} vs {expect} (F = {f})", c.x);
}

/// With the sample clamp on, no camera sample's value passes it: a tiny, very bright light makes no fireflies.
#[test]
fn sample_clamp_holds() {
    let mut s = sphere_scene(white(1.0), 2);
    s.lights.clear();
    s.lights.push(Light::new("tiny", Shape::Point { pos: Vec3::new(3.0, 3.0, 3.0), radius: 0.01 }, Vec3::splat(1e5)));
    s.options.aa_clamp = Some(1.5);
    s.options.filter = aztek_render::scene::FilterKind::Box;
    let (_, img) = render(s);
    let max = img.iter().map(|p| p.truncate().max_element()).fold(0.0f32, f32::max);
    assert!(max <= 1.5 + 1e-4, "{max}");
}

fn cornell(aa: u32, seed: u32) -> Scene {
    let mut s = Scene::new(opts(16, 16, aa));
    s.options.seed = seed;
    s.options.diffuse_depth = 4;
    s.options.filter = aztek_render::scene::FilterKind::Box;
    s.camera = Camera::looking(Vec3::new(0.0, 1.0, 3.9), Vec3::new(0.0, 1.0, 0.0), 35.0);
    let wall = |s: &mut Scene, a: Vec3, b: Vec3, c: Vec3, d: Vec3, col: Vec3| {
        let mi = s.geo.meshes.len() as u32;
        s.geo.meshes.push(Arc::new(quad(a, b, c, d)));
        let m = s.add_material("wall", std_with(|m| {
            m.base = Input::c(1.0);
            m.base_color = Input::Const(col);
            m.specular = Input::c(0.0);
        }));
        s.geo.instances.push(Instance::new("wall", mi, Mat4::IDENTITY, vec![m]));
    };
    let white = Vec3::splat(0.73);
    let red = Vec3::new(0.63, 0.065, 0.05);
    let green = Vec3::new(0.14, 0.45, 0.091);
    let (l, r, f, b, t) = (-1.0, 1.0, 0.0, -1.0, 2.0);
    wall(&mut s, Vec3::new(l, f, 1.0), Vec3::new(r, f, 1.0), Vec3::new(r, f, b), Vec3::new(l, f, b), white);
    wall(&mut s, Vec3::new(l, t, b), Vec3::new(r, t, b), Vec3::new(r, t, 1.0), Vec3::new(l, t, 1.0), white);
    wall(&mut s, Vec3::new(l, f, b), Vec3::new(r, f, b), Vec3::new(r, t, b), Vec3::new(l, t, b), white);
    wall(&mut s, Vec3::new(l, f, 1.0), Vec3::new(l, f, b), Vec3::new(l, t, b), Vec3::new(l, t, 1.0), red);
    wall(&mut s, Vec3::new(r, f, b), Vec3::new(r, f, 1.0), Vec3::new(r, t, 1.0), Vec3::new(r, t, b), green);
    let mut light = Light::new("lamp", Shape::Quad { corner: Vec3::new(-0.25, 1.99, -0.25), u: Vec3::X * 0.5, v: Vec3::Z * 0.5, n: -Vec3::Y, two_sided: false, spread: 1.0 }, Vec3::new(17.0, 12.0, 4.0));
    light.samples = 1;
    s.lights.push(light);
    s
}

/// The Cornell box converges: the image mean at low samples agrees with a high sample reference, and the error per pixel
/// shrinks as samples grow, about as one over the square root of their count.
#[test]
fn cornell_box_converges() {
    let (_, reference) = render(cornell(16, 99));
    let rmse = |img: &[Vec4]| (img.iter().zip(&reference).map(|(a, b)| (a.truncate() - b.truncate()).length_squared()).sum::<f32>() / img.len() as f32).sqrt();
    let (_, low) = render(cornell(2, 1));
    let (_, mid) = render(cornell(8, 2));
    let m_ref = mean(&reference, |_| true);
    let m_low = mean(&low, |_| true);
    assert!((m_low - m_ref).abs().max_element() / m_ref.max_element() < 0.04, "{m_low} vs {m_ref}");
    let (e_low, e_mid) = (rmse(&low), rmse(&mid));
    eprintln!("cornell: mean {m_ref}, rmse 4 spp {e_low:.4}, 64 spp {e_mid:.4}");
    // 16 times the samples: a quarter of the error in theory; allow for the reference's own noise.
    assert!(e_mid < e_low * 0.45, "{e_low} {e_mid}");
    // The red wall tints the white beside it: indirect light is there.
    let left = mean(&reference, |i| i % 16 < 3);
    assert!(left.x > left.y * 1.5, "red bleed {left}");
}

/// The surface an indirect ray reaches is lit by the lights even when no depth is left to go further: with diffuse
/// depth 1, a floor the light doesn't reach (unlinked) still shows the light a ceiling throws back.
#[test]
fn last_bounce_is_lit() {
    let mut s = Scene::new(opts(8, 8, 2));
    s.options.diffuse_depth = 1;
    let mut cam = Camera::looking(Vec3::new(0.0, 0.0, 0.5), Vec3::ZERO, 50.0);
    cam.orthographic = true;
    cam.ortho_width = 0.1;
    s.camera = cam;
    s.geo.meshes.push(Arc::new(quad(Vec3::new(-10.0, -10.0, 0.0), Vec3::new(10.0, -10.0, 0.0), Vec3::new(10.0, 10.0, 0.0), Vec3::new(-10.0, 10.0, 0.0))));
    s.geo.meshes.push(Arc::new(quad(Vec3::new(-10.0, -10.0, 2.0), Vec3::new(-10.0, 10.0, 2.0), Vec3::new(10.0, 10.0, 2.0), Vec3::new(10.0, -10.0, 2.0))));
    let m = s.add_material("grey", white(0.0));
    let mut floor = Instance::new("floor", 0, Mat4::IDENTITY, vec![m]);
    floor.light_mask = 0;
    s.geo.instances.push(floor);
    let mut ceiling = Instance::new("ceiling", 1, Mat4::IDENTITY, vec![m]);
    ceiling.visibility &= !aztek_render::geom::vis::CAMERA;
    s.geo.instances.push(ceiling);
    s.lights.push(Light::new("pt", Shape::Point { pos: Vec3::new(0.0, 0.0, 1.0), radius: 0.0 }, Vec3::splat(10.0)));
    let (_, img) = render(s);
    let c = mean(&img, |_| true);
    assert!(c.x > 0.1, "the floor sees the lit ceiling: {c}");
}
