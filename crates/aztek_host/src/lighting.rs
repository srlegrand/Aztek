//! A view's lights for the material shader: which lights its Lighting menu asks for, as a uniform block in view space, the
//! table of highlights the materials index, and which lights each shape is linked to.
//!
//! Each view has its own two buffers, written between frames: a view has one frame in flight, and begin_frame has waited
//! for it. With Use Default Lighting the shader takes its old path and nothing here is written per frame.

use std::collections::HashMap;

use ash::vk;
use aztek_scene::graph::Graph;
use aztek_scene::light_cmds::LinkTable;
use aztek_scene::lights::{self, Kind, SceneLight};
use aztek_scene::plug::NodeId;
use aztek_scene::shading::{DrawMaterial, SpecularModel};
use bytemuck::{Pod, Zeroable};
use glam::{DMat4, DVec3};

use crate::gpu::{Buffer, Gpu};
use crate::render::Renderer;

/// Lights a view draws with. The reference's own default limit is 8; past this the first ones in the scene are used.
pub const MAX_LIGHTS: usize = 16;
/// Highlights held per view: the most a uniform block is sure to allow is 16 KiB. Past it, a material has no highlight.
pub const MAX_SPECULARS: usize = 512;
/// IES profile layers in the viewport's atlas: one per photometric light it can bake.
pub const MAX_IES: usize = 16;
/// Every light: the bits of the mask a shape draws with.
pub const ALL_LIGHTS: u32 = 0xFFFF;

/// The Lighting menu, `modelEditor -displayLights`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum LightMode {
    #[default]
    Default,
    All,
    /// "active" and "selected": the selected lights, as the selection changes.
    Selected,
    Flat,
    None,
}

impl LightMode {
    pub fn parse(s: &str) -> Option<LightMode> {
        Some(match s {
            "default" => LightMode::Default,
            "all" => LightMode::All,
            "selected" | "active" => LightMode::Selected,
            "flat" => LightMode::Flat,
            "none" => LightMode::None,
            _ => return None,
        })
    }
    pub fn name(self) -> &'static str {
        match self {
            LightMode::Default => "default",
            LightMode::All => "all",
            LightMode::Selected => "active",
            LightMode::Flat => "flat",
            LightMode::None => "none",
        }
    }
    /// The shader's path: 0 the fixed two lights, 1 the block's lights, 2 no lights, 3 flat.
    fn shader_mode(self) -> i32 {
        match self {
            LightMode::Default => 0,
            LightMode::All | LightMode::Selected => 1,
            LightMode::None => 2,
            LightMode::Flat => 3,
        }
    }
}

/// std140: all vec4, so it matches the shader's struct by construction.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct GpuLight {
    /// Position in view space; w the kind (0 ambient, 1 directional, 2 point, 3 spot, 4 area, 5 volume).
    pub pos: [f32; 4],
    /// Which way it shines in view space; w the decay rate.
    pub dir: [f32; 4],
    /// Colour times intensity; w: 1 emits diffuse, 2 emits specular.
    pub color: [f32; 4],
    /// Spot: cos outer, cos inner, dropoff. Ambient: x ambientShade. Volume: shape, direction, emits ambient, cone end radius.
    pub params: [f32; 4],
    /// Area: half its width and height in view space. Volume: the first rows from view space to the light's unit space.
    pub a: [f32; 4],
    pub b: [f32; 4],
    /// Volume: the third row.
    pub c: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct LightBlock {
    /// Clip space to view space: gives the shader each vertex's position in view space from its clip position.
    pub inv_proj: [f32; 16],
    /// Path, light count, two sided, orthographic.
    pub head: [i32; 4],
    /// The realtime renderer's one shadow map: world to the light's clip space.
    pub shadow_matrix: [f32; 16],
    /// Shadow bias (a world-space normal offset), one over its size, the light it shadows, and the map's kind.
    pub shadow: [f32; 4],
    /// A point light's view-space position; unused by a 2D map.
    pub shadow_pos: [f32; 4],
    /// A cube map's near and far planes, for turning its depth back into a distance.
    pub shadow_range: [f32; 4],
    /// The sky dome's colour and strength; w is 1 when an image feeds it.
    pub env: [f32; 4],
    /// World to the dome's space, so a latlong image turns with it.
    pub env_rot: [f32; 4],
    /// View space back to world, so a reflection can be looked up in a world-space environment.
    pub view_to_world: [f32; 16],
    pub lights: [GpuLight; MAX_LIGHTS],
}

fn f4(v: DVec3, w: f32) -> [f32; 4] {
    let a = v.as_vec3().to_array();
    [a[0], a[1], a[2], w]
}

/// A light in view space, as the shader wants it.
pub fn gpu_light(l: &SceneLight, view: &DMat4) -> GpuLight {
    let vw = *view * l.world;
    let pos = vw.w_axis.truncate();
    let dir = (-vw.z_axis.truncate()).normalize_or_zero();
    let kind = match l.kind {
        Kind::Ambient => 0.0,
        Kind::Directional => 1.0,
        Kind::Point => 2.0,
        Kind::Spot => 3.0,
        Kind::Area => 4.0,
        Kind::Volume => 5.0,
    };
    let emits = l.diffuse as i32 as f32 + 2.0 * (l.specular as i32 as f32);
    // aiExposure, as Harold applies it too: the same light settings give the same result in both.
    let gain = 2f64.powf(l.exposure) as f32;
    let mut out = GpuLight { pos: f4(pos, kind), dir: f4(dir, l.decay as f32), color: [l.color[0] * gain, l.color[1] * gain, l.color[2] * gain, emits], params: [0.0; 4], a: [0.0; 4], b: [0.0; 4], c: [0.0; 4] };
    match l.kind {
        Kind::Ambient => out.params[0] = l.ambient_shade as f32,
        Kind::Spot => {
            let half = l.cone_deg * 0.5;
            // A positive penumbra widens the edge outside the cone, a negative one inside it.
            let (inner, outer) = if l.penumbra_deg >= 0.0 { (half, half + l.penumbra_deg) } else { (half + l.penumbra_deg, half) };
            out.params = [outer.clamp(0.0, 180.0).to_radians().cos() as f32, inner.clamp(0.0, 180.0).to_radians().cos() as f32, l.dropoff as f32, 0.0];
            // A photometric light's IES profile as 12 vertical samples, averaged around each ring: the viewport's
            // compact form of the profile. Harold samples the full two-dimensional table.
            if let Some(ies) = l.ies.as_ref() {
                let mut s = [0.0f32; 12];
                for (k, value) in s.iter_mut().enumerate() {
                    let vert = k as f32 / 11.0 * std::f32::consts::PI;
                    let mut sum = 0.0f32;
                    const H: usize = 32;
                    for h in 0..H {
                        let horiz = h as f32 / H as f32 * std::f32::consts::TAU;
                        let d = glam::Vec3::new(vert.sin() * horiz.cos(), vert.sin() * horiz.sin(), -vert.cos());
                        sum += ies.at(d);
                    }
                    *value = sum / H as f32;
                }
                out.a = [s[0], s[1], s[2], s[3]];
                out.b = [s[4], s[5], s[6], s[7]];
                out.c = [s[8], s[9], s[10], s[11]];
            }
        }
        Kind::Area => {
            let (u, v) = (vw.x_axis.truncate(), vw.y_axis.truncate());
            out.a = f4(u, 0.0);
            out.b = f4(v, 0.0);
            // u and v are half its sides. x: what its radiance takes of the colour, spread over the area when the light
            // is normalised, as Harold's. y: the nearest-point light's scale when it isn't normalised.
            let area = 4.0 * u.cross(v).length() as f32;
            out.params[0] = if l.by_area { 1.0 } else { 1.0 / area.max(1e-6) };
            out.params[1] = if l.by_area { (u.length() * v.length()) as f32 } else { 1.0 };
        }
        Kind::Volume => {
            let to_unit = vw.inverse().transpose();
            let row = |c: glam::DVec4| [c.x as f32, c.y as f32, c.z as f32, c.w as f32];
            out.a = row(to_unit.x_axis);
            out.b = row(to_unit.y_axis);
            out.c = row(to_unit.z_axis);
            out.params = [l.volume_shape as f32, l.volume_dir as f32, l.volume_ambient as i32 as f32, 0.0];
        }
        _ => {}
    }
    out
}

/// One highlight: colour and model, then the shininess and roll off.
pub fn specular_entry(m: &DrawMaterial) -> Option<[[f32; 4]; 2]> {
    let s = m.specular.as_ref()?;
    let model = match s.model {
        SpecularModel::Phong => 1.0,
        SpecularModel::Blinn => 2.0,
    };
    Some([[s.color[0], s.color[1], s.color[2], model], [s.shininess, s.roll_off, 0.0, 0.0]])
}

/// Which of the drawn lights light a shape: a bit for each, in the block's order.
pub fn mask_for(g: &Graph, links: &LinkTable, used: &[NodeId], shape: NodeId) -> u32 {
    let mut mask = 0;
    for (i, &light) in used.iter().enumerate().take(MAX_LIGHTS) {
        if links.lights(g, light, shape) {
            mask |= 1 << i;
        }
    }
    mask
}

/// The lights a mode draws, in scene order, up to the limit. Hidden lights light nothing.
pub fn chosen(g: &mut Graph, mode: LightMode, shown: impl Fn(&Graph, NodeId) -> bool) -> Vec<SceneLight> {
    if !matches!(mode, LightMode::All | LightMode::Selected) {
        return Vec::new();
    }
    let sel = g.selection().to_vec();
    let mut out = Vec::new();
    for sh in lights::all_shapes(g) {
        if !shown(g, sh) {
            continue;
        }
        if mode == LightMode::Selected {
            let tr = aztek_scene::dag::transform_of(g, sh);
            if !sel.contains(&sh) && !sel.contains(&tr) {
                continue;
            }
        }
        if let Some(l) = lights::scene_light(g, sh) {
            out.push(l);
        }
        if out.len() == MAX_LIGHTS {
            break;
        }
    }
    out
}

/// The sky dome the light mode draws, read as the Realtime renderer's environment.
pub fn chosen_environment(g: &mut Graph, mode: LightMode, shown: impl Fn(&Graph, NodeId) -> bool, scene: Option<&str>, project: Option<&str>) -> Option<lights::EnvironmentLight> {
    if !matches!(mode, LightMode::All | LightMode::Selected) {
        return None;
    }
    let sel = g.selection().to_vec();
    for sh in lights::all_shapes(g) {
        if g.node(sh).is_none_or(|n| n.ty().name != "aiSkyDomeLight") || !shown(g, sh) {
            continue;
        }
        if mode == LightMode::Selected {
            let tr = aztek_scene::dag::transform_of(g, sh);
            if !sel.contains(&sh) && !sel.contains(&tr) {
                continue;
            }
        }
        if let Some(e) = lights::environment_of(g, sh, scene, project) {
            return Some(e);
        }
    }
    None
}

/// One light's shadow: which light, how its map is drawn, and how far a surface is offset along its normal to keep a
/// caster from shadowing itself.
#[derive(Clone, Copy, Debug)]
pub struct ShadowPlan {
    pub light: usize,
    pub camera: ShadowCamera,
    pub bias: f32,
}

#[derive(Clone, Copy, Debug)]
pub enum ShadowCamera {
    /// A directional or spot light: one map, world to the light's clip space.
    Map2D(DMat4),
    /// A point, area or volume light: six faces looking out along the world's axes. The light's position and the far
    /// plane are kept for the shader.
    Cube { faces: [DMat4; 6], position: DVec3, near: f32, far: f32 },
}

/// What `ViewLights` writes for a frame's shadow map.
#[derive(Clone, Copy, Debug)]
pub struct ShadowBlock {
    pub light: usize,
    pub matrix: [f32; 16],
    pub pos: [f32; 4],
    pub mode: f32,
    pub bias: f32,
    pub near: f32,
    pub far: f32,
}

impl ShadowPlan {
    /// The block's fields, in the camera's view space.
    pub fn block(&self, view: &DMat4) -> ShadowBlock {
        match self.camera {
            ShadowCamera::Map2D(matrix) => ShadowBlock {
                light: self.light,
                matrix: (matrix * view.inverse()).as_mat4().to_cols_array(),
                pos: [0.0; 4],
                mode: 1.0,
                bias: self.bias,
                near: 0.0,
                far: 0.0,
            },
            ShadowCamera::Cube { position, near, far, .. } => {
                let p = view.transform_point3(position);
                ShadowBlock { light: self.light, matrix: DMat4::IDENTITY.as_mat4().to_cols_array(), pos: [p.x as f32, p.y as f32, p.z as f32, far], near, far, mode: 2.0, bias: self.bias }
            }
        }
    }
}

/// The six faces of a cube shadow map, in the order the shader selects them: +X, -X, +Y, -Y, +Z, -Z.
fn cube_faces(eye: DVec3, near: f64, far: f64) -> [DMat4; 6] {
    // (forward, up): the up axis is chosen so the face's u runs right and v runs down, as the shader looks them up.
    let faces = [
        (DVec3::X, -DVec3::Y),
        (-DVec3::X, -DVec3::Y),
        (DVec3::Y, DVec3::Z),
        (-DVec3::Y, -DVec3::Z),
        (DVec3::Z, -DVec3::Y),
        (-DVec3::Z, -DVec3::Y),
    ];
    // Each face sees a quarter turn from the light, as it does: the shader turns the depth back into the distance along
    // the face's axis to compare.
    let proj = DMat4::perspective_rh(std::f64::consts::FRAC_PI_2, 1.0, near, far);
    let mut out = [DMat4::IDENTITY; 6];
    for (i, (f, up)) in faces.iter().enumerate() {
        out[i] = proj * DMat4::look_to_rh(eye, *f, *up);
    }
    out
}

/// The shadow camera for the first light of the frame that casts one. A directional light or spot gets one map; a
/// point, area or volume light gets six, from its position.
pub fn shadow_plan(lights: &[SceneLight], items: &[crate::viewport::DrawItem]) -> Option<ShadowPlan> {
    let index = lights.iter().position(|l| l.cast_shadows && !matches!(l.kind, Kind::Ambient))?;
    let light = &lights[index];
    let (lo, hi) = scene_bounds(items);
    let center = (lo + hi) * 0.5;
    let radius = ((hi - lo).length() * 0.5).max(1e-3) as f64;
    let bias = (radius * 0.01).clamp(0.001, 1.0) as f32;
    let axis = (-light.world.z_axis.truncate()).normalize_or_zero();
    match light.kind {
        Kind::Directional => {
            let eye = center - axis * (radius * 3.0 + 1.0);
            let up = if axis.dot(DVec3::Y).abs() > 0.95 { DVec3::Z } else { DVec3::Y };
            let view = DMat4::look_at_rh(eye, center, up);
            let r = radius * 1.15;
            let proj = DMat4::orthographic_rh(-r, r, -r, r, 0.01, radius * 8.0 + 2.0);
            Some(ShadowPlan { light: index, camera: ShadowCamera::Map2D(proj * view), bias })
        }
        Kind::Spot => {
            let eye = light.world.w_axis.truncate();
            let (near, far) = scene_distance_range(items, eye);
            let up = if axis.dot(DVec3::Y).abs() > 0.95 { DVec3::Z } else { DVec3::Y };
            let view = DMat4::look_at_rh(eye, eye + axis, up);
            // A map is a flat window on the cone: a very wide spot (a photometric light's 180 degrees) is held to 170,
            // where the projection is still finite.
            let half = ((light.cone_deg + light.penumbra_deg.abs()) * 0.5).clamp(1.0, 85.0).to_radians();
            // The near plane as close to the scene as it can be: perspective depth has most of its precision near it, and
            // a far plane far past the scene leaves too little to tell a caster from what it shadows.
            let proj = DMat4::perspective_rh(2.0 * half, 1.0, near.max(0.01), far.max(near + 0.1));
            Some(ShadowPlan { light: index, camera: ShadowCamera::Map2D(proj * view), bias })
        }
        Kind::Point | Kind::Area | Kind::Volume => {
            let position = light.world.w_axis.truncate();
            let (near, far) = scene_distance_range(items, position);
            let faces = cube_faces(position, near.max(0.01), far.max(near + 0.1));
            Some(ShadowPlan { light: index, camera: ShadowCamera::Cube { faces, position, near: near.max(0.01) as f32, far: far.max(near + 0.1) as f32 }, bias })
        }
        Kind::Ambient => None,
    }
}

/// How near and far the scene is from a light: the closest and farthest mesh point.
fn scene_distance_range(items: &[crate::viewport::DrawItem], eye: DVec3) -> (f64, f64) {
    let mut near = f64::MAX;
    let mut far = 0.0f64;
    for it in items {
        let m = &it.mesh;
        for p in 0..m.point_capacity() {
            if m.point_valid(p) {
                let d = (it.world.transform_point3(m.position(p)) - eye).length();
                near = near.min(d);
                far = far.max(d);
            }
        }
    }
    if near > far {
        (0.1, 100.0)
    } else {
        ((near * 0.5).max(0.01), (far * 1.05 + 0.1).max(near + 0.1))
    }
}

/// The world bounds of what the view draws, for a shadow camera to cover.
fn scene_bounds(items: &[crate::viewport::DrawItem]) -> (DVec3, DVec3) {
    let mut lo = DVec3::splat(f64::MAX);
    let mut hi = DVec3::splat(f64::MIN);
    for it in items {
        let m = &it.mesh;
        for p in 0..m.point_capacity() {
            if m.point_valid(p) {
                let w = it.world.transform_point3(m.position(p));
                lo = lo.min(w);
                hi = hi.max(w);
            }
        }
    }
    if lo.x > hi.x {
        (DVec3::splat(-1.0), DVec3::splat(1.0))
    } else {
        (lo, hi)
    }
}

/// A view's lighting buffers and descriptor set.
pub struct ViewLights {
    block: Buffer,
    planar_block: Buffer,
    speculars: Buffer,
    pool: vk::DescriptorPool,
    pub set: vk::DescriptorSet,
    pub planar_set: vk::DescriptorSet,
    /// Indices into the highlight table, by shading group. 0 is no highlight.
    pub index: HashMap<NodeId, u32>,
    /// The shapes whose mask isn't all lights.
    pub masks: HashMap<NodeId, u32>,
    mask_key: Option<(u64, Vec<NodeId>)>,
    spec_version: Option<u64>,
    written_default: bool,
    /// The sky dome for the Realtime renderer: its loaded image set, colour, rotation and strength.
    pub env_set: Option<vk::DescriptorSet>,
    env_key: Option<String>,
    env: [f32; 4],
    env_rot: [f32; 4],
}

fn write<T: Pod>(b: &Buffer, data: &[T]) {
    let bytes: &[u8] = bytemuck::cast_slice(data);
    if let Some(ptr) = b.alloc.as_ref().and_then(|a| a.mapped_ptr()) {
        unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr.as_ptr() as *mut u8, bytes.len()) };
    }
}

impl ViewLights {
    pub fn new(gpu: &Gpu, r: &Renderer) -> ViewLights {
        let start = LightBlock {
            inv_proj: DMat4::IDENTITY.as_mat4().to_cols_array(),
            head: [0; 4],
            shadow_matrix: DMat4::IDENTITY.as_mat4().to_cols_array(),
            shadow: [-1.0; 4],
            shadow_pos: [0.0; 4],
            shadow_range: [0.0; 4],
            env: [0.0; 4],
            env_rot: [0.0, 0.0, 0.0, 1.0],
            view_to_world: DMat4::IDENTITY.as_mat4().to_cols_array(),
            lights: [GpuLight::zeroed(); MAX_LIGHTS],
        };
        let block = gpu.buffer_with(vk::BufferUsageFlags::UNIFORM_BUFFER, std::slice::from_ref(&start));
        let planar_block = gpu.buffer_with(vk::BufferUsageFlags::UNIFORM_BUFFER, std::slice::from_ref(&start));
        let speculars = gpu.buffer_with(vk::BufferUsageFlags::UNIFORM_BUFFER, &vec![[[0.0f32; 4]; 2]; MAX_SPECULARS]);
        unsafe {
            let d = &gpu.device;
            let sizes = [vk::DescriptorPoolSize::default().ty(vk::DescriptorType::UNIFORM_BUFFER).descriptor_count(4)];
            let pool = d.create_descriptor_pool(&vk::DescriptorPoolCreateInfo::default().max_sets(2).pool_sizes(&sizes), None).expect("light pool");
            let layouts = [r.light_layout, r.light_layout];
            let sets = d.allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo::default().descriptor_pool(pool).set_layouts(&layouts));
            let (set, planar_set) = (sets.as_ref().expect("light sets")[0], sets.as_ref().expect("light sets")[1]);
            let infos = [
                [vk::DescriptorBufferInfo::default().buffer(block.buffer).offset(0).range(std::mem::size_of::<LightBlock>() as u64)],
                [vk::DescriptorBufferInfo::default().buffer(speculars.buffer).offset(0).range((MAX_SPECULARS * 32) as u64)],
                [vk::DescriptorBufferInfo::default().buffer(planar_block.buffer).offset(0).range(std::mem::size_of::<LightBlock>() as u64)],
            ];
            let writes = [
                vk::WriteDescriptorSet::default().dst_set(set).dst_binding(0).descriptor_type(vk::DescriptorType::UNIFORM_BUFFER).buffer_info(&infos[0]),
                vk::WriteDescriptorSet::default().dst_set(set).dst_binding(1).descriptor_type(vk::DescriptorType::UNIFORM_BUFFER).buffer_info(&infos[1]),
                vk::WriteDescriptorSet::default().dst_set(planar_set).dst_binding(0).descriptor_type(vk::DescriptorType::UNIFORM_BUFFER).buffer_info(&infos[2]),
                vk::WriteDescriptorSet::default().dst_set(planar_set).dst_binding(1).descriptor_type(vk::DescriptorType::UNIFORM_BUFFER).buffer_info(&infos[1]),
            ];
            d.update_descriptor_sets(&writes, &[]);
            ViewLights {
                block,
                planar_block,
                speculars,
                pool,
                set,
                planar_set,
                index: HashMap::new(),
                masks: HashMap::new(),
                mask_key: None,
                spec_version: None,
                written_default: true,
                env_set: None,
                env_key: None,
                env: [0.0; 4],
                env_rot: [0.0, 0.0, 0.0, 1.0],
            }
        }
    }

    /// The highlights, when the materials have been read again.
    pub fn write_speculars<'a>(&mut self, version: u64, materials: impl Iterator<Item = (&'a NodeId, &'a DrawMaterial)>) {
        if self.spec_version == Some(version) {
            return;
        }
        self.spec_version = Some(version);
        self.index.clear();
        let mut table = vec![[[0.0f32; 4]; 2]; MAX_SPECULARS];
        let mut next = 1;
        for (sg, m) in materials {
            let Some(e) = specular_entry(m) else { continue };
            if next < MAX_SPECULARS {
                table[next] = e;
                self.index.insert(*sg, next as u32);
                next += 1;
            }
        }
        write(&self.speculars, &table);
    }

    /// The sky dome image for this frame, or white when there is none. The colour and rotation go in the block.
    pub fn write_environment(&mut self, gpu: &Gpu, env: Option<&aztek_scene::lights::EnvironmentLight>) {
        self.env = [0.0; 4];
        self.env_rot = [0.0, 0.0, 0.0, 1.0];
        let mut key = None;
        if let Some(e) = env {
            self.env = [e.color[0], e.color[1], e.color[2], 1.0];
            let q = e.rotation;
            self.env_rot = [q.x as f32, q.y as f32, q.z as f32, q.w as f32];
            if let Some(t) = &e.image {
                let path = t.resolved.clone();
                key = path.clone();
                if self.env_key != path || self.env_set.is_none() {
                    self.env_set = path.as_ref().and_then(|p| crate::textures::with_cache(gpu, |c| c.get(gpu, p, t.srgb)).map(|l| l.set));
                }
            }
        }
        if self.env_key != key {
            self.env_key = key;
            if self.env_key.is_none() {
                self.env_set = None;
            }
        }
    }

    /// This frame's block. `lights` are what the mode chose; `shadow` the light and its world-to-clip matrix when the
    /// Realtime renderer draws one.
    pub fn write_block(&mut self, mode: LightMode, lights: &[SceneLight], view: &DMat4, proj: &DMat4, two_sided: bool, ortho: bool, shadow: Option<ShadowBlock>) {
        self.write_block_at(false, mode, lights, view, proj, two_sided, ortho, shadow);
    }

    /// The block the planar reflection draws with, in the planar buffer: the main view and lights, and no shadow map.
    pub fn write_planar_block(&mut self, mode: LightMode, lights: &[SceneLight], view: &DMat4, proj: &DMat4, two_sided: bool, ortho: bool) {
        self.write_block_at(true, mode, lights, view, proj, two_sided, ortho, None);
    }

    fn write_block_at(&mut self, planar: bool, mode: LightMode, lights: &[SceneLight], view: &DMat4, proj: &DMat4, two_sided: bool, ortho: bool, shadow: Option<ShadowBlock>) {
        let target = if planar { &self.planar_block } else { &self.block };
        let path = mode.shader_mode();
        // The default path reads only its head and the inverse projection, which the view's position and size change
        // every frame; writing it is cheap.
        if path == 0 {
            write(
                target,
                std::slice::from_ref(&LightBlock {
                    inv_proj: proj.inverse().as_mat4().to_cols_array(),
                    head: [0; 4],
                    shadow_matrix: DMat4::IDENTITY.as_mat4().to_cols_array(),
                    shadow: [-1.0; 4],
                    shadow_pos: [0.0; 4],
                    shadow_range: [0.0; 4],
                    env: [0.0; 4],
                    env_rot: [0.0, 0.0, 0.0, 1.0],
                    view_to_world: view.inverse().as_mat4().to_cols_array(),
                    lights: [GpuLight::zeroed(); MAX_LIGHTS],
                }),
            );
            self.written_default = true;
            return;
        }
        self.written_default = false;
        let mut block = LightBlock {
            inv_proj: proj.inverse().as_mat4().to_cols_array(),
            head: [path, lights.len().min(MAX_LIGHTS) as i32, two_sided as i32, ortho as i32],
            shadow_matrix: DMat4::IDENTITY.as_mat4().to_cols_array(),
            shadow: [-1.0; 4],
            shadow_pos: [0.0; 4],
            shadow_range: [0.0; 4],
            env: self.env,
            env_rot: self.env_rot,
            view_to_world: view.inverse().as_mat4().to_cols_array(),
            lights: [GpuLight::zeroed(); MAX_LIGHTS],
        };
        if let Some(s) = shadow {
            block.shadow_matrix = s.matrix;
            block.shadow = [s.bias, 1.0 / crate::realtime::SHADOW_SIZE as f32, s.light as f32, s.mode];
            block.shadow_pos = s.pos;
            block.shadow_range = [s.near, s.far, 0.0, 0.0];
        }
        for (i, l) in lights.iter().take(MAX_LIGHTS).enumerate() {
            let mut g = gpu_light(l, view);
            if l.ies.is_some() {
                g.params[3] = 1.0;
            }
            block.lights[i] = g;
        }
        write(target, std::slice::from_ref(&block));
    }

    /// Which lights each drawn shape takes, when the scene or the lights have changed. Empty, and cheap, while every light
    /// is in the default set and nothing is linked.
    pub fn update_masks(&mut self, g: &Graph, used: &[NodeId], shapes: impl Iterator<Item = NodeId>) {
        let key = (g.version(), used.to_vec());
        if self.mask_key.as_ref() == Some(&key) {
            return;
        }
        self.mask_key = Some(key);
        self.masks.clear();
        let links = LinkTable::read(g);
        if links.is_empty() && used.iter().all(|&l| lights::illuminates_by_default(g, l)) {
            return;
        }
        for sh in shapes {
            let m = mask_for(g, &links, used, sh);
            if m != low_bits(used.len()) {
                self.masks.insert(sh, m);
            }
        }
    }

    pub fn mask(&self, shape: NodeId) -> u32 {
        self.masks.get(&shape).copied().unwrap_or(ALL_LIGHTS)
    }

    /// The sky dome the frame's reflection pass falls back on: its colour and strength (w 1 with an image), its rotation,
    /// and the descriptor set holding the image.
    pub fn environment(&self) -> ([f32; 4], [f32; 4], Option<vk::DescriptorSet>) {
        (self.env, self.env_rot, self.env_set)
    }

    pub fn release(self, gpu: &mut Gpu) {
        unsafe { gpu.device.destroy_descriptor_pool(self.pool, None) };
        gpu.drop_buffer(self.block);
        gpu.drop_buffer(self.planar_block);
        gpu.drop_buffer(self.speculars);
    }
}

fn low_bits(n: usize) -> u32 {
    if n >= 16 {
        ALL_LIGHTS
    } else {
        (1u32 << n) - 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_point_lights_faces_look_out_at_the_scene() {
        let eye = DVec3::new(10.0, 18.0, 10.0);
        let faces = cube_faces(eye, 1.0, 100.0);
        // The scene is below and behind the light: the -Y face sees it.
        let p = DVec3::ZERO;
        let clip = faces[3] * p.extend(1.0);
        assert!(clip.w > 0.0, "in front: {clip:?}");
        let ndc = clip.truncate() / clip.w;
        assert!(ndc.x.abs() <= 1.0 && ndc.y.abs() <= 1.0 && (0.0..=1.0).contains(&ndc.z), "inside the -Y face: {ndc:?}");
        // The +Y face looks away: the point is behind its near plane.
        let up = faces[2] * p.extend(1.0);
        let ndc_up = up.truncate() / up.w;
        assert!(!(0.0..=1.0).contains(&ndc_up.z) || ndc_up.x.abs() > 1.0 || ndc_up.y.abs() > 1.0, "+Y looks away: {ndc_up:?}");
    }

    /// A cube face's depth image holds the distance along its own axis, not the ray's length: that is what the
    /// shader's `cube_distance` decodes, and what it must compare the receiver against.
    #[test]
    fn a_cube_face_stores_its_own_axis_distance() {
        let eye = DVec3::new(10.0, 18.0, 10.0);
        let (near, far) = (1.0f64, 100.0f64);
        let faces = cube_faces(eye, near, far);
        let at = eye + DVec3::new(5.0, 3.0, 0.0);
        let clip = faces[0] * at.extend(1.0);
        let decoded = far * near / (far + (clip.z / clip.w) * (near - far));
        assert!((decoded - 5.0).abs() < 1e-6, "{decoded}");
        // The ray is longer than the axis, so comparing against its length would shadow too little.
        assert!((at - eye).length() > 5.5);
        // The axis is a world-space maximum. A turned frame changes which component is largest, which is why the
        // shader must take it from the vector it sampled the cube with, not from the view-space one.
        let turned = DMat4::from_rotation_y(0.7) * (at - eye).extend(0.0);
        let (world, view) = (DVec3::new(5.0, 3.0, 0.0), turned.truncate());
        let axis = |v: DVec3| v.x.abs().max(v.y.abs()).max(v.z.abs());
        assert!((axis(world) - 5.0).abs() < 1e-6);
        assert!((axis(view) - 5.0).abs() > 1e-3, "the view-space maximum moved: {}", axis(view));
    }

    #[test]
    fn the_shadow_camera_covers_the_scene_in_vulkans_clip_space() {
        let mut s = aztek_scene::Session::new();
        aztek_scene::cameras::make_defaults(&mut s.graph);
        s.run("polyCube", json!({})).unwrap();
        s.run("directionalLight", json!({})).unwrap();
        let items = crate::viewport::gather(&mut s.graph);
        let light = aztek_scene::lights::all_shapes(&s.graph).into_iter().find_map(|id| aztek_scene::lights::scene_light(&mut s.graph, id)).unwrap();
        let plan = shadow_plan(&[light], &items).unwrap();
        let matrix = match plan.camera {
            ShadowCamera::Map2D(m) => m,
            ShadowCamera::Cube { .. } => panic!("a directional light has one map"),
        };
        let mut seen = false;
        for it in &items {
            for p in 0..it.mesh.point_capacity() {
                if !it.mesh.point_valid(p) {
                    continue;
                }
                let world = it.world.transform_point3(it.mesh.position(p));
                let clip = matrix * world.extend(1.0);
                // Vulkan: clip x and y in -w..w, clip z in 0..w.
                assert!(clip.z >= -1e-6 && clip.z <= clip.w + 1e-6, "clip {clip:?}");
                assert!(clip.x.abs() <= clip.w + 1e-6 && clip.y.abs() <= clip.w + 1e-6, "clip {clip:?}");
                seen = true;
            }
        }
        assert!(seen);
    }
}
