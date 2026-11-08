//! What a render needs, gathered: geometry, materials and their networks, lights, the camera and the settings. Built by
//! the scene translator, then `prepare`d once before rendering; immutable while a render runs and shared between threads.

use std::sync::Arc;

use glam::{Mat4, Vec3};

use crate::bvh::Aabb;
use crate::camera::Camera;
use crate::geom::Geometry;
use crate::lights::{Light, Shape};
use crate::shader::{Material, Network, Node};
use crate::texture::TextureCache;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FilterKind {
    Box,
    Triangle,
    #[default]
    Gaussian,
    BlackmanHarris,
    Mitchell,
    CatmullRom,
    Sinc,
    /// One sample at the pixel centre, unweighted: Arnold's closest for data AOVs.
    Closest,
}

impl FilterKind {
    pub fn parse(s: &str) -> FilterKind {
        match s.trim_end_matches("_filter").to_ascii_lowercase().as_str() {
            "box" => FilterKind::Box,
            "triangle" => FilterKind::Triangle,
            "blackman_harris" | "blackmanharris" => FilterKind::BlackmanHarris,
            "mitnet" | "mitchell" | "mitchell_netravali" => FilterKind::Mitchell,
            "catrom" | "catmull_rom" => FilterKind::CatmullRom,
            "sinc" => FilterKind::Sinc,
            "closest" => FilterKind::Closest,
            _ => FilterKind::Gaussian,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            FilterKind::Box => "box",
            FilterKind::Triangle => "triangle",
            FilterKind::Gaussian => "gaussian",
            FilterKind::BlackmanHarris => "blackman_harris",
            FilterKind::Mitchell => "mitnet",
            FilterKind::CatmullRom => "catrom",
            FilterKind::Sinc => "sinc",
            FilterKind::Closest => "closest",
        }
    }
    /// The filter's value at an offset in pixels, for a filter `width` pixels across.
    pub fn eval(self, x: f32, width: f32) -> f32 {
        let r = (width * 0.5).max(1e-3);
        let t = x.abs() / r;
        if t > 1.0 {
            return 0.0;
        }
        match self {
            FilterKind::Box | FilterKind::Closest => 1.0,
            FilterKind::Triangle => 1.0 - t,
            FilterKind::Gaussian => (-4.0 * t * t).exp() - (-4.0f32).exp(),
            FilterKind::BlackmanHarris => {
                let a = std::f32::consts::PI * (t + 1.0);
                0.35875 - 0.48829 * a.cos() + 0.14128 * (2.0 * a).cos() - 0.01168 * (3.0 * a).cos()
            }
            FilterKind::Mitchell | FilterKind::CatmullRom => {
                let (b, c) = if self == FilterKind::Mitchell { (1.0 / 3.0, 1.0 / 3.0) } else { (0.0, 0.5) };
                let x = t * 2.0;
                if x < 1.0 {
                    ((12.0 - 9.0 * b - 6.0 * c) * x * x * x + (-18.0 + 12.0 * b + 6.0 * c) * x * x + (6.0 - 2.0 * b)) / 6.0
                } else {
                    ((-b - 6.0 * c) * x * x * x + (6.0 * b + 30.0 * c) * x * x + (-12.0 * b - 48.0 * c) * x + (8.0 * b + 24.0 * c)) / 6.0
                }
            }
            FilterKind::Sinc => {
                let x = t * r * std::f32::consts::PI;
                let s = if x.abs() < 1e-4 { 1.0 } else { x.sin() / x };
                let wnd = std::f32::consts::PI * t;
                let l = if wnd.abs() < 1e-4 { 1.0 } else { wnd.sin() / wnd };
                s * l
            }
        }
    }
}

/// A homogeneous fog filling the scene's bounds: Arnold's atmosphere volume, simplified.
#[derive(Clone, Copy, Debug)]
pub struct Atmosphere {
    pub density: f32,
    pub color: Vec3,
    pub anisotropy: f32,
}

#[derive(Clone, Debug)]
pub struct Options {
    pub width: u32,
    pub height: u32,
    /// Pixel aspect ratio.
    pub pixel_aspect: f32,
    /// x0, y0, x1, y1 in pixels, inclusive-exclusive.
    pub region: Option<[u32; 4]>,
    pub aa_samples: u32,
    pub diffuse_samples: u32,
    pub specular_samples: u32,
    pub transmission_samples: u32,
    pub sss_samples: u32,
    pub volume_samples: u32,
    pub total_depth: u32,
    pub diffuse_depth: u32,
    pub specular_depth: u32,
    pub transmission_depth: u32,
    pub volume_depth: u32,
    pub transparency_depth: u32,
    pub low_light_threshold: f32,
    /// Clamp of each camera sample's value, when set.
    pub aa_clamp: Option<f32>,
    pub clamp_aovs: bool,
    /// Clamp of indirect samples.
    pub indirect_clamp: f32,
    pub adaptive: bool,
    pub aa_samples_max: u32,
    pub adaptive_threshold: f32,
    pub filter: FilterKind,
    pub filter_width: f32,
    pub seed: u32,
    /// 0 for every core.
    pub threads: usize,
    pub bucket_size: u32,
    pub ignore_textures: bool,
    pub ignore_shadows: bool,
    pub ignore_sss: bool,
    pub ignore_atmosphere: bool,
    pub ignore_lights: bool,
    pub ignore_dof: bool,
    pub ignore_motion_blur: bool,
    pub denoise: bool,
    /// AOV names to keep beside the beauty.
    pub aovs: Vec<String>,
    /// Light group names, by group index; 0 is "default".
    pub light_groups: Vec<String>,
    pub texture_max_memory_mb: usize,
    /// What camera rays that meet nothing see, where no sky dome is.
    pub background: Vec3,
}

impl Default for Options {
    /// Arnold's defaults.
    fn default() -> Self {
        Options {
            width: 960,
            height: 540,
            pixel_aspect: 1.0,
            region: None,
            aa_samples: 3,
            diffuse_samples: 2,
            specular_samples: 2,
            transmission_samples: 2,
            sss_samples: 2,
            volume_samples: 2,
            total_depth: 10,
            diffuse_depth: 1,
            specular_depth: 1,
            transmission_depth: 8,
            volume_depth: 0,
            transparency_depth: 10,
            low_light_threshold: 0.001,
            aa_clamp: None,
            clamp_aovs: false,
            indirect_clamp: 10.0,
            adaptive: false,
            aa_samples_max: 20,
            adaptive_threshold: 0.015,
            filter: FilterKind::Gaussian,
            filter_width: 2.0,
            seed: 0,
            threads: 0,
            bucket_size: 64,
            ignore_textures: false,
            ignore_shadows: false,
            ignore_sss: false,
            ignore_atmosphere: false,
            ignore_lights: false,
            ignore_dof: false,
            ignore_motion_blur: false,
            denoise: false,
            aovs: Vec::new(),
            light_groups: vec!["default".into()],
            texture_max_memory_mb: 4096,
            background: Vec3::ZERO,
        }
    }
}

/// A material's ambient occlusion or curvature node, which needs rays at the shading point.
#[derive(Clone, Copy, Debug)]
pub struct TracedInput {
    pub kind: i64,
    pub samples: u32,
    pub distance: f32,
}

pub struct Scene {
    pub geo: Geometry,
    pub network: Network,
    pub materials: Vec<Material>,
    pub material_names: Vec<String>,
    pub lights: Vec<Light>,
    pub camera: Camera,
    pub options: Options,
    pub atmosphere: Option<Atmosphere>,
    pub textures: Arc<TextureCache>,
    /// Per material, the traced input it reads, if any.
    pub traced: Vec<Option<TracedInput>>,
    /// Per instance, its world to object matrix at mid shutter, for object space lookups.
    pub bounds: Aabb,
    /// Warnings from building the scene, for the render log.
    pub warnings: Vec<String>,
}

impl Scene {
    pub fn new(options: Options) -> Scene {
        let budget = options.texture_max_memory_mb;
        Scene { geo: Geometry::default(), network: Network::default(), materials: vec![Material::default()], material_names: vec!["default".into()], lights: Vec::new(), camera: Camera::default(), options, atmosphere: None, textures: Arc::new(TextureCache::new(budget)), traced: Vec::new(), bounds: Aabb::EMPTY, warnings: Vec::new() }
    }

    pub fn add_material(&mut self, name: &str, m: Material) -> u32 {
        self.materials.push(m);
        self.material_names.push(name.into());
        (self.materials.len() - 1) as u32
    }

    /// Builds the hierarchy over instances, mesh light tables and the dome's distribution. Call after any change.
    pub fn prepare(&mut self) {
        self.geo.build();
        self.bounds = self.geo.bounds();
        // Mesh lights' tables need the hierarchy's instances in place.
        for i in 0..self.lights.len() {
            if let Shape::Mesh { inst, tris, .. } = &self.lights[i].shape {
                let (inst, tris) = (*inst, tris.clone());
                if (inst as usize) < self.geo.instances.len() {
                    self.lights[i].shape = crate::lights::mesh_light_shape(&self.geo, inst, if tris.is_empty() { None } else { Some(tris) });
                    self.geo.instances[inst as usize].light = Some(i as u32);
                }
            }
            if let Shape::Dome { to_local, image, dist } = &self.lights[i].shape {
                if let (Some(img), None) = (image, dist) {
                    let d = Arc::new(crate::lights::dome_distribution(img));
                    self.lights[i].shape = Shape::Dome { to_local: *to_local, image: Some(img.clone()), dist: Some(d) };
                }
            }
        }
        self.traced = self
            .materials
            .iter()
            .map(|m| {
                let crate::shader::Material::Standard(s) = m else {
                    if let crate::shader::Material::Flat { color, .. } = m {
                        return self.traced_of(*color);
                    }
                    return None;
                };
                [s.base_color, s.emission_color, s.specular_color, s.subsurface_color].iter().find_map(|i| self.traced_of(*i))
            })
            .collect();
    }

    fn traced_of(&self, i: crate::shader::Input) -> Option<TracedInput> {
        let n = self.network.traced(i)?;
        match &self.network.nodes[n as usize] {
            Node::Traced { kind, samples, distance, .. } => Some(TracedInput { kind: *kind, samples: *samples, distance: *distance }),
            _ => None,
        }
    }

    pub fn aspect(&self) -> f32 {
        self.options.width as f32 / self.options.height.max(1) as f32 * self.options.pixel_aspect
    }

    pub fn memory_bytes(&self) -> usize {
        self.geo.meshes.iter().map(|m| m.memory_bytes()).sum::<usize>() + self.geo.instances.len() * 200 + self.textures.bytes.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn group_index(&mut self, name: &str) -> u16 {
        if name.is_empty() || name == "default" {
            return 0;
        }
        if let Some(i) = self.options.light_groups.iter().position(|g| g == name) {
            return i as u16;
        }
        self.options.light_groups.push(name.into());
        (self.options.light_groups.len() - 1) as u16
    }
}

/// A transform from a translation and Euler rotation in degrees, for tests and examples.
pub fn xform(t: Vec3, r_deg: Vec3) -> Mat4 {
    Mat4::from_translation(t) * Mat4::from_euler(glam::EulerRot::ZYX, r_deg.z.to_radians(), r_deg.y.to_radians(), r_deg.x.to_radians())
}
