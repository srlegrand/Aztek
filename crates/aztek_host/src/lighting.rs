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
    let mut out = GpuLight { pos: f4(pos, kind), dir: f4(dir, l.decay as f32), color: [l.color[0], l.color[1], l.color[2], emits], params: [0.0; 4], a: [0.0; 4], b: [0.0; 4], c: [0.0; 4] };
    match l.kind {
        Kind::Ambient => out.params[0] = l.ambient_shade as f32,
        Kind::Spot => {
            let half = l.cone_deg * 0.5;
            // A positive penumbra widens the edge outside the cone, a negative one inside it.
            let (inner, outer) = if l.penumbra_deg >= 0.0 { (half, half + l.penumbra_deg) } else { (half + l.penumbra_deg, half) };
            out.params = [outer.clamp(0.0, 180.0).to_radians().cos() as f32, inner.clamp(0.0, 180.0).to_radians().cos() as f32, l.dropoff as f32, 0.0];
        }
        Kind::Area => {
            let (u, v) = (vw.x_axis.truncate(), vw.y_axis.truncate());
            out.a = f4(u, 0.0);
            out.b = f4(v, 0.0);
            if l.by_area {
                let k = (u.length() * v.length()) as f32;
                for x in &mut out.color[..3] {
                    *x *= k;
                }
            }
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

/// A view's lighting buffers and descriptor set.
pub struct ViewLights {
    block: Buffer,
    speculars: Buffer,
    pool: vk::DescriptorPool,
    pub set: vk::DescriptorSet,
    /// Indices into the highlight table, by shading group. 0 is no highlight.
    pub index: HashMap<NodeId, u32>,
    /// The shapes whose mask isn't all lights.
    pub masks: HashMap<NodeId, u32>,
    mask_key: Option<(u64, Vec<NodeId>)>,
    spec_version: Option<u64>,
    written_default: bool,
}

fn write<T: Pod>(b: &Buffer, data: &[T]) {
    let bytes: &[u8] = bytemuck::cast_slice(data);
    if let Some(ptr) = b.alloc.as_ref().and_then(|a| a.mapped_ptr()) {
        unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr.as_ptr() as *mut u8, bytes.len()) };
    }
}

impl ViewLights {
    pub fn new(gpu: &Gpu, r: &Renderer) -> ViewLights {
        let start = LightBlock { inv_proj: DMat4::IDENTITY.as_mat4().to_cols_array(), head: [0; 4], lights: [GpuLight::zeroed(); MAX_LIGHTS] };
        let block = gpu.buffer_with(vk::BufferUsageFlags::UNIFORM_BUFFER, std::slice::from_ref(&start));
        let speculars = gpu.buffer_with(vk::BufferUsageFlags::UNIFORM_BUFFER, &vec![[[0.0f32; 4]; 2]; MAX_SPECULARS]);
        unsafe {
            let d = &gpu.device;
            let sizes = [vk::DescriptorPoolSize::default().ty(vk::DescriptorType::UNIFORM_BUFFER).descriptor_count(2)];
            let pool = d.create_descriptor_pool(&vk::DescriptorPoolCreateInfo::default().max_sets(1).pool_sizes(&sizes), None).expect("light pool");
            let layouts = [r.light_layout];
            let set = d.allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo::default().descriptor_pool(pool).set_layouts(&layouts)).expect("light set")[0];
            let infos = [
                [vk::DescriptorBufferInfo::default().buffer(block.buffer).offset(0).range(std::mem::size_of::<LightBlock>() as u64)],
                [vk::DescriptorBufferInfo::default().buffer(speculars.buffer).offset(0).range((MAX_SPECULARS * 32) as u64)],
            ];
            let writes = [
                vk::WriteDescriptorSet::default().dst_set(set).dst_binding(0).descriptor_type(vk::DescriptorType::UNIFORM_BUFFER).buffer_info(&infos[0]),
                vk::WriteDescriptorSet::default().dst_set(set).dst_binding(1).descriptor_type(vk::DescriptorType::UNIFORM_BUFFER).buffer_info(&infos[1]),
            ];
            d.update_descriptor_sets(&writes, &[]);
            ViewLights { block, speculars, pool, set, index: HashMap::new(), masks: HashMap::new(), mask_key: None, spec_version: None, written_default: true }
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

    /// This frame's block. `lights` are what the mode chose.
    pub fn write_block(&mut self, mode: LightMode, lights: &[SceneLight], view: &DMat4, proj: &DMat4, two_sided: bool, ortho: bool) {
        let path = mode.shader_mode();
        // The default path reads nothing of the block but its head.
        if path == 0 {
            if !self.written_default {
                write(&self.block, std::slice::from_ref(&LightBlock { inv_proj: DMat4::IDENTITY.as_mat4().to_cols_array(), head: [0; 4], lights: [GpuLight::zeroed(); MAX_LIGHTS] }));
                self.written_default = true;
            }
            return;
        }
        self.written_default = false;
        let mut block = LightBlock { inv_proj: proj.inverse().as_mat4().to_cols_array(), head: [path, lights.len().min(MAX_LIGHTS) as i32, two_sided as i32, ortho as i32], lights: [GpuLight::zeroed(); MAX_LIGHTS] };
        for (i, l) in lights.iter().take(MAX_LIGHTS).enumerate() {
            block.lights[i] = gpu_light(l, view);
        }
        write(&self.block, std::slice::from_ref(&block));
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

    pub fn release(self, gpu: &mut Gpu) {
        unsafe { gpu.device.destroy_descriptor_pool(self.pool, None) };
        gpu.drop_buffer(self.block);
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
