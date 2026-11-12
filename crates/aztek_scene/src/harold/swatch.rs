//! Swatches: a shading node as a small picture, rendered by Harold so that what the Hypershade and the views show of
//! a network is what a render gives. A material is seen on a ball under a key light; anything else with a colour
//! (a texture, a utility) is seen flat, over the 0 to 1 UV square. The views use the flat picture as the texture of
//! a material whose colour comes from a network that is not one image file.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex, OnceLock};

use aztek_render::geom::{Instance, MeshData};
use aztek_render::lights::{Light, Shape};
use aztek_render::scene::{Options, Scene};
use aztek_render::shader::{Input, Network, Out, ShadeCtx};
use aztek_render::texture::TextureCache;
use glam::{Mat4, Vec2, Vec3, Vec4};

use super::materials::MatBuilder;
use super::network::NetBuilder;
use crate::graph::Graph;
use crate::plug::{NodeId, PlugRef};

/// A picture: 8 bits a channel, sRGB, top row first.
#[derive(Debug)]
pub struct Picture {
    /// What it was made from: the same key is the same picture.
    pub key: String,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    /// The mean colour, linear.
    pub mean: [f32; 3],
}

impl PartialEq for Picture {
    fn eq(&self, other: &Picture) -> bool {
        self.key == other.key
    }
}

fn textures() -> Arc<TextureCache> {
    static CACHE: OnceLock<Arc<TextureCache>> = OnceLock::new();
    CACHE.get_or_init(|| Arc::new(TextureCache::new(256))).clone()
}

/// A number that changes when anything a node's look depends on does: the node, and everything connected into it.
pub fn signature(g: &mut Graph, node: NodeId) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    let mut seen: Vec<NodeId> = Vec::new();
    let mut todo = vec![node];
    while let Some(id) = todo.pop() {
        if seen.contains(&id) {
            continue;
        }
        seen.push(id);
        let Some(n) = g.node(id) else { continue };
        let ty = n.type_arc().clone();
        n.name().hash(&mut h);
        ty.name.hash(&mut h);
        // A node outside shading (a shape, a control) is taken at its value where it is read, not walked into.
        let shading = crate::shading::classification(&ty.name).is_some() || crate::utility_nodes::classification(&ty.name).is_some() || crate::shading::is_material_type(&ty);
        if id != node && !shading {
            continue;
        }
        for (i, def) in ty.attrs.iter().enumerate() {
            let p = PlugRef::new(id, i as i32);
            let mut connected = false;
            for q in std::iter::once(p).chain((0..3).map(|k| PlugRef::component(id, i as i32, k))) {
                if let Some(src) = g.source(&q) {
                    connected = true;
                    (i, q.component, src.attr, src.component, src.element).hash(&mut h);
                    todo.push(src.node);
                }
            }
            if def.is_array() {
                // Elements are not walked one by one: any change to the scene counts.
                g.version().hash(&mut h);
                continue;
            }
            if !connected {
                if let Ok(v) = g.value(&p) {
                    format!("{v:?}").hash(&mut h);
                }
            }
        }
    }
    h.finish()
}

fn encode(c: f32) -> u8 {
    let c = c.clamp(0.0, 1.0);
    let s = if c <= 0.003_130_8 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
    (s * 255.0 + 0.5) as u8
}

fn builder<'a>(g: &'a mut Graph, net: &'a mut Network, scene: Option<&str>, project: Option<&str>, textures: Arc<TextureCache>) -> NetBuilder<'a> {
    NetBuilder { g, net, built: HashMap::new(), warnings: Vec::new(), scene_path: scene.map(str::to_string), project: project.map(str::to_string), ignore_textures: false, textures }
}

/// The colour an output of a node has over the UV square, `size` pixels a side. None when the node is nothing the
/// renderer shades.
pub fn flat(g: &mut Graph, src: &PlugRef, size: u32, scene: Option<&str>, project: Option<&str>) -> Option<Picture> {
    let sig = signature(g, src.node);
    let key = format!("swatch:{}:{}:{}:{size}:{sig:x}", g.path(src.node), src.attr, src.component);
    let tex = textures();
    let mut net = Network::default();
    let input = {
        let mut nb = builder(g, &mut net, scene, project, tex.clone());
        let n = nb.node(src.node)?;
        let a = nb.g.attr(src).clone();
        let out = if src.component >= 0 {
            [Out::R, Out::G, Out::B][src.component.clamp(0, 2) as usize]
        } else if a.name.ends_with("Alpha") {
            Out::A
        } else {
            Out::Rgb
        };
        Input::Link(n, out)
    };
    let size = size.clamp(1, 1024);
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    let mut sum = Vec3::ZERO;
    for y in 0..size {
        for x in 0..size {
            let uv = Vec2::new((x as f32 + 0.5) / size as f32, 1.0 - (y as f32 + 0.5) / size as f32);
            // A slice through the middle of the unit cube stands for the object, for textures that fill space.
            let p = Vec3::new(uv.x * 2.0 - 1.0, uv.y * 2.0 - 1.0, 0.0);
            let ctx = ShadeCtx { uv, p, p_obj: p, footprint: 1.0 / size as f32, ..Default::default() };
            let mut c = net.eval(input, &ctx, &tex);
            if !c.is_finite() {
                c = Vec3::ZERO;
            }
            sum += c.clamp(Vec3::ZERO, Vec3::ONE);
            rgba.extend([encode(c.x), encode(c.y), encode(c.z), 255]);
        }
    }
    let mean = sum / (size * size) as f32;
    Some(Picture { key, width: size, height: size, rgba, mean: mean.to_array() })
}

/// As `flat`, kept: the same network gives the same picture without working it out again.
pub fn flat_cached(g: &mut Graph, src: &PlugRef, size: u32, scene: Option<&str>, project: Option<&str>) -> Option<Arc<Picture>> {
    static KEPT: Mutex<Vec<Arc<Picture>>> = Mutex::new(Vec::new());
    let sig = signature(g, src.node);
    let key = format!("swatch:{}:{}:{}:{size}:{sig:x}", g.path(src.node), src.attr, src.component);
    if let Some(p) = KEPT.lock().unwrap().iter().find(|p| p.key == key) {
        return Some(p.clone());
    }
    let made = Arc::new(flat(g, src, size, scene, project)?);
    let mut kept = KEPT.lock().unwrap();
    // Older pictures of the same output are of no more use.
    let stem = key.rsplit_once(':').map(|x| x.0.to_string()).unwrap_or_default();
    kept.retain(|p| !p.key.starts_with(&stem));
    if kept.len() >= 128 {
        kept.remove(0);
    }
    kept.push(made.clone());
    Some(made)
}

fn ball_mesh() -> MeshData {
    let (cols, rows) = (64u32, 32u32);
    let (mut pos, mut nrm, mut uvs, mut tris) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for r in 0..=rows {
        let v = r as f32 / rows as f32;
        let phi = (v - 0.5) * std::f32::consts::PI;
        for c in 0..=cols {
            let u = c as f32 / cols as f32;
            let th = u * std::f32::consts::TAU;
            let n = Vec3::new(phi.cos() * th.sin(), phi.sin(), phi.cos() * th.cos());
            pos.push(n);
            nrm.push(n);
            uvs.push(Vec2::new(u, v));
        }
    }
    for r in 0..rows {
        for c in 0..cols {
            let a = r * (cols + 1) + c;
            let b = a + cols + 1;
            if r > 0 {
                tris.push([a, a + 1, b + 1]);
            }
            if r + 1 < rows {
                tris.push([a, b + 1, b]);
            }
        }
    }
    let n = tris.len();
    MeshData::new("swatchBall", pos, nrm, uvs, tris, vec![0; n])
}

/// A material on a ball: a soft key light from the upper left, a dim sky all round, a dark ground behind.
pub fn ball(g: &mut Graph, material: NodeId, size: u32, scene: Option<&str>, project: Option<&str>) -> Picture {
    let sig = signature(g, material);
    let key = format!("ball:{}:{size}:{sig:x}", g.path(material));
    let size = size.clamp(8, 1024);
    let small = size <= 96;
    let options = Options {
        width: size,
        height: size,
        aa_samples: if small { 3 } else { 4 },
        diffuse_samples: 1,
        specular_samples: 1,
        transmission_samples: 1,
        sss_samples: 1,
        total_depth: 4,
        diffuse_depth: 1,
        specular_depth: 2,
        transmission_depth: 4,
        adaptive: false,
        ..Default::default()
    };
    let mut sc = Scene::new(options);
    sc.textures = textures();
    let mut net = std::mem::take(&mut sc.network);
    let mut mats = std::mem::take(&mut sc.materials);
    let mut names = std::mem::take(&mut sc.material_names);
    let shader = {
        let mut nb = builder(g, &mut net, scene, project, sc.textures.clone());
        let mut mb = MatBuilder { nb: &mut nb, materials: &mut mats, names: &mut names, built: HashMap::new() };
        mb.material(material)
    };
    sc.network = net;
    sc.materials = mats;
    sc.material_names = names;
    sc.geo.meshes.push(Arc::new(ball_mesh()));
    // Turned so the seam of the UVs is at the back.
    sc.geo.instances.push(Instance::new("swatchBall", 0, Mat4::from_rotation_y(std::f32::consts::PI), vec![shader]));
    sc.camera.xforms = vec![Mat4::from_translation(Vec3::new(0.0, 0.0, 4.4))];
    sc.camera.focal_mm = 50.0;
    sc.camera.h_aperture = 1.0;
    sc.camera.v_aperture = 1.0;
    let centre = Vec3::new(-3.2, 4.2, 4.6);
    let n = (-centre).normalize();
    let u = n.cross(Vec3::Y).normalize() * 3.0;
    let v = u.cross(n).normalize() * 3.0;
    sc.lights.push(Light::new("swatchKey", Shape::Quad { corner: centre - u * 0.5 - v * 0.5, u, v, n, two_sided: false, spread: 1.0 }, Vec3::splat(9.0)));
    let mut sky = Light::new("swatchSky", Shape::Dome { to_local: Mat4::IDENTITY, image: None, dist: None }, Vec3::splat(0.22));
    sky.camera = 0.0;
    sc.lights.push(sky);
    super::lights::emissive(&mut sc);
    sc.prepare();
    let (film, _) = aztek_render::render::render(Arc::new(sc), 0);
    let px: Vec<Vec4> = film.index_of("RGBA").map(|i| film.pixels(i)).unwrap_or_default();
    let ground = 20.0 / 255.0;
    let mut rgba = Vec::with_capacity(px.len() * 4);
    let mut sum = Vec3::ZERO;
    for p in &px {
        let a = if p.w.is_finite() { p.w.clamp(0.0, 1.0) } else { 0.0 };
        let c = if p.is_finite() { Vec3::new(p.x, p.y, p.z).max(Vec3::ZERO) } else { Vec3::ZERO };
        sum += c.min(Vec3::ONE);
        // The render is premultiplied: the ground shows through what it does not cover.
        for k in 0..3 {
            rgba.push(((encode(c[k]) as f32) + ground * 255.0 * (1.0 - a)).min(255.0) as u8);
        }
        rgba.push(255);
    }
    if rgba.len() != (size * size * 4) as usize {
        rgba = vec![20; (size * size * 4) as usize];
    }
    let mean = sum / (size * size).max(1) as f32;
    Picture { key, width: size, height: size, rgba, mean: mean.to_array() }
}

/// The picture of a node: a material on a ball, anything else with a colour flat.
pub fn of_node(g: &mut Graph, node: NodeId, size: u32, scene: Option<&str>, project: Option<&str>) -> Option<Picture> {
    let ty = g.node(node)?.type_arc().clone();
    if crate::shading::is_material_type(&ty) {
        return Some(ball(g, node, size, scene, project));
    }
    let attr = ["outColor", "output", "outValue", "outAlpha", "outFloat", "outUV"].iter().find_map(|a| ty.attr_index(a))?;
    flat(g, &PlugRef::new(node, attr as i32), size, scene, project)
}
