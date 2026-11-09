//! Drawing proxy shapes: no node per prim, geometry shared on the GPU, one instance per place it appears. The work of
//! reading a stage comes in slices from `aztek_usd::draw`, so a big stage fills in rather than stalling the UI.

use std::cell::RefCell;
use std::collections::HashMap;
use std::time::{Duration, Instant};

use ash::vk;
use aztek_scene::graph::Graph;
use aztek_scene::plug::NodeId;
use aztek_scene::session::Session;
use aztek_scene::transform::DagNode;
use aztek_usd::draw::{Filter, Purposes, Scene, Vertex};
use aztek_usd::{prims, stages, util};
use bytemuck::{Pod, Zeroable};
use glam::{DMat3, DMat4, DVec2, DVec3};

use crate::gpu::{Buffer, Gpu};
use crate::render::{compile, Kind, LineVertex, Push, Renderer};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuVertex {
    pos: [f32; 3],
    nrm: [f32; 3],
    uv: [f32; 2],
    color: [u8; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Instance {
    m: [f32; 16],
    color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct UsdPush {
    mvp: [f32; 16],
    n0: [f32; 4],
    n1: [f32; 4],
    n2: [f32; 4],
}

const VS: &str = r#"#version 450
layout(location = 0) in vec3 a_pos;
layout(location = 1) in vec3 a_nrm;
layout(location = 2) in vec2 a_uv;
layout(location = 3) in vec4 a_col;
layout(location = 4) in vec4 i_m0;
layout(location = 5) in vec4 i_m1;
layout(location = 6) in vec4 i_m2;
layout(location = 7) in vec4 i_m3;
layout(location = 8) in vec4 i_col;
layout(push_constant) uniform PC { mat4 mvp; vec4 n0; vec4 n1; vec4 n2; } pc;
layout(location = 0) out vec3 v_nrm;
layout(location = 1) out vec2 v_uv;
layout(location = 2) out vec4 v_col;
void main() {
    mat4 m = mat4(i_m0, i_m1, i_m2, i_m3);
    gl_Position = pc.mvp * (m * vec4(a_pos, 1.0));
    vec3 n = normalize(mat3(i_m0.xyz, i_m1.xyz, i_m2.xyz) * a_nrm);
    v_nrm = vec3(dot(pc.n0.xyz, n), dot(pc.n1.xyz, n), dot(pc.n2.xyz, n));
    v_uv = a_uv;
    v_col = a_col * i_col;
}
"#;

const FS: &str = r#"#version 450
layout(location = 0) in vec3 v_nrm;
layout(location = 1) in vec2 v_uv;
layout(location = 2) in vec4 v_col;
layout(set = 0, binding = 0) uniform texture2D t_color;
layout(set = 0, binding = 1) uniform sampler s_color;
layout(location = 0) out vec4 out_color;
void main() {
    vec3 n = normalize(v_nrm);
    vec3 k = normalize(vec3(-0.45, 0.6, 0.65));
    vec3 f = normalize(vec3(0.6, -0.2, 0.75));
    float d = 0.75 * abs(dot(n, k)) + 0.3 * abs(dot(n, f));
    vec4 tex = texture(sampler2D(t_color, s_color), vec2(v_uv.x, 1.0 - v_uv.y));
    vec3 lin = v_col.rgb * tex.rgb * (0.12 + 0.95 * d);
    out_color = vec4(pow(lin, vec3(1.0 / 2.2)), v_col.a * tex.a);
}
"#;

struct Pipes {
    layout: vk::PipelineLayout,
    solid: vk::Pipeline,
    blend: vk::Pipeline,
}

fn make_pipes(gpu: &Gpu) -> Pipes {
    unsafe {
        let d = &gpu.device;
        let range = [vk::PushConstantRange::default().stage_flags(vk::ShaderStageFlags::VERTEX).size(std::mem::size_of::<UsdPush>() as u32)];
        let sets = [crate::textures::set_layout(gpu)];
        let layout = d.create_pipeline_layout(&vk::PipelineLayoutCreateInfo::default().push_constant_ranges(&range).set_layouts(&sets), None).unwrap();
        let module = |src: &str, stage| {
            let code = compile(src, stage);
            d.create_shader_module(&vk::ShaderModuleCreateInfo::default().code(&code), None).unwrap()
        };
        let (vs, fs) = (module(VS, naga::ShaderStage::Vertex), module(FS, naga::ShaderStage::Fragment));
        let mut made = Vec::new();
        for blending in [false, true] {
            let stages = [
                vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::VERTEX).module(vs).name(c"main"),
                vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::FRAGMENT).module(fs).name(c"main"),
            ];
            let bindings = [
                vk::VertexInputBindingDescription::default().binding(0).stride(std::mem::size_of::<GpuVertex>() as u32).input_rate(vk::VertexInputRate::VERTEX),
                vk::VertexInputBindingDescription::default().binding(1).stride(std::mem::size_of::<Instance>() as u32).input_rate(vk::VertexInputRate::INSTANCE),
            ];
            let attr = |loc, binding, format, offset| vk::VertexInputAttributeDescription::default().location(loc).binding(binding).format(format).offset(offset);
            let attrs = [
                attr(0, 0, vk::Format::R32G32B32_SFLOAT, 0),
                attr(1, 0, vk::Format::R32G32B32_SFLOAT, 12),
                attr(2, 0, vk::Format::R32G32_SFLOAT, 24),
                attr(3, 0, vk::Format::R8G8B8A8_UNORM, 32),
                attr(4, 1, vk::Format::R32G32B32A32_SFLOAT, 0),
                attr(5, 1, vk::Format::R32G32B32A32_SFLOAT, 16),
                attr(6, 1, vk::Format::R32G32B32A32_SFLOAT, 32),
                attr(7, 1, vk::Format::R32G32B32A32_SFLOAT, 48),
                attr(8, 1, vk::Format::R32G32B32A32_SFLOAT, 64),
            ];
            let vin = vk::PipelineVertexInputStateCreateInfo::default().vertex_binding_descriptions(&bindings).vertex_attribute_descriptions(&attrs);
            let ia = vk::PipelineInputAssemblyStateCreateInfo::default().topology(vk::PrimitiveTopology::TRIANGLE_LIST);
            let vp = vk::PipelineViewportStateCreateInfo::default().viewport_count(1).scissor_count(1);
            let rs = vk::PipelineRasterizationStateCreateInfo::default().polygon_mode(vk::PolygonMode::FILL).cull_mode(vk::CullModeFlags::NONE).front_face(vk::FrontFace::CLOCKWISE).line_width(1.0);
            let ms = vk::PipelineMultisampleStateCreateInfo::default().rasterization_samples(gpu.samples);
            let ds = vk::PipelineDepthStencilStateCreateInfo::default().depth_test_enable(true).depth_write_enable(!blending).depth_compare_op(vk::CompareOp::GREATER_OR_EQUAL);
            let blend = [vk::PipelineColorBlendAttachmentState::default()
                .blend_enable(true)
                .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA)
                .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
                .color_blend_op(vk::BlendOp::ADD)
                .src_alpha_blend_factor(vk::BlendFactor::ONE)
                .dst_alpha_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
                .alpha_blend_op(vk::BlendOp::ADD)
                .color_write_mask(vk::ColorComponentFlags::RGBA)];
            let cb = vk::PipelineColorBlendStateCreateInfo::default().attachments(&blend);
            let dyns = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
            let dy = vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dyns);
            let info = vk::GraphicsPipelineCreateInfo::default()
                .stages(&stages)
                .vertex_input_state(&vin)
                .input_assembly_state(&ia)
                .viewport_state(&vp)
                .rasterization_state(&rs)
                .multisample_state(&ms)
                .depth_stencil_state(&ds)
                .color_blend_state(&cb)
                .dynamic_state(&dy)
                .layout(layout)
                .render_pass(gpu.render_pass)
                .subpass(0);
            made.push(d.create_graphics_pipelines(vk::PipelineCache::null(), &[info], None).map_err(|e| e.1).unwrap()[0]);
        }
        d.destroy_shader_module(vs, None);
        d.destroy_shader_module(fs, None);
        Pipes { layout, solid: made[0], blend: made[1] }
    }
}

struct GpuGeom {
    vb: Buffer,
    ib: Buffer,
    count: u32,
}

#[derive(Clone, Copy)]
struct Batch {
    geom: usize,
    look: usize,
    first: u32,
    count: u32,
    blend: bool,
}

#[derive(Default)]
struct ProxyDraw {
    scene: Scene,
    entry_id: i64,
    geoms: Vec<Option<GpuGeom>>,
    instances: Option<Buffer>,
    batches: Vec<Batch>,
    built_item_version: u64,
    built_geoms: usize,
    world: DMat4,
    file_dir: String,
    finished: bool,
    wire: Option<(Buffer, u32)>,
    wire_key: String,
    generation: u64,
}

#[derive(Default)]
struct State {
    proxies: HashMap<NodeId, ProxyDraw>,
    pipes: HashMap<vk::RenderPass, Pipes>,
    busy: bool,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

thread_local! {
    static SEEN: RefCell<HashMap<i64, (u64, String, usize)>> = RefCell::new(HashMap::new());
}

/// What stages did since last asked, for scriptJob: usdStageChanged, usdLayerChanged, usdEditTargetChanged.
pub fn note_events() {
    for e in stages::open_entries() {
        let version = e.version.get();
        let target = e.stage.edit_target().layer_identifier().to_string();
        let layers = e.stage.layer_identifiers().len();
        SEEN.with(|s| {
            let mut s = s.borrow_mut();
            // A stage first seen here may have changed in the command that opened it.
            let was = s.insert(e.id, (version, target.clone(), layers)).unwrap_or((1, target.clone(), layers));
            if was.0 != version {
                crate::script_jobs::note_event("usdStageChanged");
            }
            if was.1 != target {
                crate::script_jobs::note_event("usdEditTargetChanged");
            }
            if was.2 != layers {
                crate::script_jobs::note_event("usdLayerChanged");
            }
        });
    }
}

/// Whether a stage is still being read: the UI should come back.
pub fn busy() -> bool {
    STATE.with(|s| s.borrow().busy)
}

fn budget() -> Duration {
    let ms = std::env::var("AZTEK_USD_BUDGET_MS").ok().and_then(|x| x.parse().ok()).unwrap_or(10u64);
    Duration::from_millis(ms)
}

fn filter_of(g: &mut Graph, shape: NodeId) -> Filter {
    let on = |g: &mut Graph, n: &str, d: bool| util::get_f64(g, shape, n).map_or(d, |v| v != 0.0);
    let mut p = Purposes { render: on(g, "drawRenderPurpose", false), proxy: on(g, "drawProxyPurpose", true), guide: on(g, "drawGuidePurpose", false) };
    p.guide |= on(g, "displayGuides", false);
    p.render |= on(g, "displayRenderGuides", false);
    let exclude = util::get_str(g, shape, "excludePrimPaths").unwrap_or_default().split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect();
    Filter { root: util::get_str(g, shape, "primPath").unwrap_or_default(), exclude, purposes: p }
}

/// Reads more of each stage, for at most `limit` in all. None means finish.
fn update_scenes(s: &mut Session, limit: Option<Duration>) {
    let shapes = stages::all_proxies(&s.graph);
    aztek_usd::translate::drive_prims(&mut s.graph);
    let start = Instant::now();
    let mut busy = false;
    for shape in &shapes {
        let shape = *shape;
        let Ok(entry) = stages::entry_of(&mut s.graph, shape) else { continue };
        let filter = filter_of(&mut s.graph, shape);
        let time = stages::time_of(&mut s.graph, shape);
        let world = s.graph.node(shape).map_or(DMat4::IDENTITY, |n| n.matrix(DagNode::WORLD_MATRIX));
        let left = limit.map(|l| l.saturating_sub(start.elapsed()));
        STATE.with(|st| {
            let mut st = st.borrow_mut();
            let d = st.proxies.entry(shape).or_default();
            if d.entry_id != entry.id {
                *d = ProxyDraw { entry_id: entry.id, ..Default::default() };
            }
            d.world = world;
            d.file_dir = std::path::Path::new(&entry.file).parent().map_or_else(String::new, |x| x.to_string_lossy().into_owned());
            let b = left.unwrap_or(Duration::from_secs(3600));
            d.finished = d.scene.update(&entry.stage, entry.version.get(), &entry.changes.borrow(), &filter, time, b);
            busy |= !d.finished;
        });
    }
    STATE.with(|st| {
        let mut st = st.borrow_mut();
        st.proxies.retain(|k, _| shapes.contains(k));
        st.busy = busy;
    });
}

/// Finishes reading every stage, for framing and tests.
pub fn sync(s: &mut Session) {
    update_scenes(s, None);
}

fn upload(gpu: &mut Gpu, d: &mut ProxyDraw, until: Instant) -> bool {
    if d.generation != d.scene.generation {
        d.generation = d.scene.generation;
        for g in std::mem::take(&mut d.geoms).into_iter().flatten() {
            gpu.drop_buffer(g.vb);
            gpu.drop_buffer(g.ib);
        }
    }
    d.geoms.resize_with(d.scene.geoms.len(), || None);
    let mut more = false;
    for i in 0..d.geoms.len() {
        if d.geoms[i].is_some() {
            continue;
        }
        if Instant::now() >= until {
            more = true;
            break;
        }
        let g = &d.scene.geoms[i];
        if g.indices.is_empty() {
            d.geoms[i] = Some(GpuGeom { vb: gpu.buffer_with(vk::BufferUsageFlags::VERTEX_BUFFER, &[0u8; 16]), ib: gpu.buffer_with(vk::BufferUsageFlags::INDEX_BUFFER, &[0u32; 4]), count: 0 });
            continue;
        }
        let verts: Vec<GpuVertex> = g.vertices.iter().map(|v: &Vertex| GpuVertex { pos: v.pos, nrm: v.nrm, uv: v.uv, color: v.color }).collect();
        d.geoms[i] = Some(GpuGeom {
            vb: gpu.buffer_with(vk::BufferUsageFlags::VERTEX_BUFFER, &verts),
            ib: gpu.buffer_with(vk::BufferUsageFlags::INDEX_BUFFER, &g.indices),
            count: g.indices.len() as u32,
        });
    }
    more
}

/// Sorts items into batches of one geometry and look, each one instanced draw.
fn rebuild_batches(gpu: &mut Gpu, d: &mut ProxyDraw) {
    let mut order: Vec<usize> = (0..d.scene.items.len()).collect();
    order.sort_by_key(|&i| {
        let it = &d.scene.items[i];
        (d.scene.looks[it.look].opacity < 1.0, d.scene.looks[it.look].texture.is_some(), it.look, it.geom)
    });
    let mut inst = Vec::with_capacity(order.len());
    let mut batches: Vec<Batch> = Vec::new();
    for i in order {
        let it = &d.scene.items[i];
        let look = &d.scene.looks[it.look];
        let g = &d.scene.geoms[it.geom];
        // Colours on the vertices stand as they are unless a material says otherwise.
        let c = if g.vertex_colors && it.look == 0 { [1.0, 1.0, 1.0] } else { look.color };
        let m = it.world.as_mat4().to_cols_array();
        inst.push(Instance { m, color: [c[0], c[1], c[2], look.opacity] });
        let n = inst.len() as u32 - 1;
        match batches.last_mut() {
            Some(b) if b.geom == it.geom && b.look == it.look => b.count += 1,
            _ => batches.push(Batch { geom: it.geom, look: it.look, first: n, count: 1, blend: look.opacity < 1.0 }),
        }
    }
    if let Some(old) = d.instances.take() {
        gpu.drop_buffer(old);
    }
    if !inst.is_empty() {
        d.instances = Some(gpu.buffer_with(vk::BufferUsageFlags::VERTEX_BUFFER, &inst));
    }
    d.batches = batches;
    d.built_item_version = d.scene.item_version;
    d.built_geoms = d.scene.geoms.len();
}

fn rows(m: DMat3) -> ([f32; 4], [f32; 4], [f32; 4]) {
    let r = m.transpose();
    let row = |v: DVec3| [v.x as f32, v.y as f32, v.z as f32, 0.0];
    (row(r.x_axis), row(r.y_axis), row(r.z_axis))
}

/// A line box around what's selected, in stage space.
fn selection_wire(d: &ProxyDraw, name: &str, selected: &[String], whole: bool) -> Vec<LineVertex> {
    let mut out = Vec::new();
    let col = [0.26, 1.0, 0.64, 1.0];
    let boxes: Vec<(DVec3, DVec3)> = if whole {
        d.scene.bounds(None).into_iter().collect()
    } else {
        selected.iter().filter_map(|s| s.strip_prefix(&format!("{name},"))).filter_map(|p| d.scene.bounds(Some(p))).collect()
    };
    for (lo, hi) in boxes {
        let c = |i: usize| DVec3::new(if i & 1 == 0 { lo.x } else { hi.x }, if i & 2 == 0 { lo.y } else { hi.y }, if i & 4 == 0 { lo.z } else { hi.z });
        for (a, b) in [(0, 1), (2, 3), (4, 5), (6, 7), (0, 2), (1, 3), (4, 6), (5, 7), (0, 4), (1, 5), (2, 6), (3, 7)] {
            out.push(LineVertex { pos: c(a).as_vec3().to_array(), color: col });
            out.push(LineVertex { pos: c(b).as_vec3().to_array(), color: col });
        }
    }
    out
}

/// Draws every visible proxy shape.
pub fn draw(gpu: &mut Gpu, r: &Renderer, s: &mut Session, view_proj: DMat4, view: DMat4) {
    let shapes = stages::all_proxies(&s.graph);
    if shapes.is_empty() {
        return;
    }
    update_scenes(s, Some(budget()));
    let selected = prims::selected();
    let names: HashMap<NodeId, (String, bool, bool)> = shapes
        .iter()
        .map(|&n| {
            let up = s.graph.parent(n);
            let picked = s.graph.selection().contains(&n) || s.graph.selection().contains(&up);
            (n, (s.graph.full_path(n), crate::viewport::visible(&s.graph, n), picked))
        })
        .collect();
    STATE.with(|st| {
        let mut st = st.borrow_mut();
        let pass = gpu.render_pass;
        if !st.pipes.contains_key(&pass) {
            let p = make_pipes(gpu);
            st.pipes.insert(pass, p);
        }
        let (layout, solid, blend) = {
            let p = &st.pipes[&pass];
            (p.layout, p.solid, p.blend)
        };
        let until = Instant::now() + budget();
        let mut more = false;
        let white = crate::textures::with_cache(gpu, |c| c.white(gpu));
        for (shape, d) in st.proxies.iter_mut() {
            let Some((name, shown, picked)) = names.get(shape) else { continue };
            if !shown {
                continue;
            }
            more |= upload(gpu, d, until);
            if d.built_item_version != d.scene.item_version || d.built_geoms != d.scene.geoms.len() {
                rebuild_batches(gpu, d);
            }
            let Some(ibuf) = d.instances.as_ref().map(|b| b.buffer) else { continue };
            let n = rows(DMat3::from_mat4(view * d.world));
            let pc = UsdPush { mvp: (view_proj * d.world).as_mat4().to_cols_array(), n0: n.0, n1: n.1, n2: n.2 };
            unsafe {
                gpu.device.cmd_push_constants(gpu.cmd, layout, vk::ShaderStageFlags::VERTEX, 0, bytemuck::bytes_of(&pc));
            }
            for phase_blend in [false, true] {
                let mut bound_look = usize::MAX;
                unsafe { gpu.device.cmd_bind_pipeline(gpu.cmd, vk::PipelineBindPoint::GRAPHICS, if phase_blend { blend } else { solid }) };
                for b in d.batches.iter().filter(|b| b.blend == phase_blend) {
                    let Some(Some(g)) = d.geoms.get(b.geom) else { continue };
                    if g.count == 0 {
                        continue;
                    }
                    if b.look != bound_look {
                        bound_look = b.look;
                        let set = d.scene.looks[b.look].texture.as_ref().and_then(|t| {
                            let path = if std::path::Path::new(t).exists() { t.clone() } else { format!("{}/{t}", d.file_dir) };
                            crate::textures::with_cache(gpu, |c| c.get(gpu, &path, true)).map(|l| l.set)
                        });
                        unsafe { gpu.device.cmd_bind_descriptor_sets(gpu.cmd, vk::PipelineBindPoint::GRAPHICS, layout, 0, &[set.unwrap_or(white)], &[]) };
                    }
                    unsafe {
                        gpu.device.cmd_bind_vertex_buffers(gpu.cmd, 0, &[g.vb.buffer, ibuf], &[0, 0]);
                        gpu.device.cmd_bind_index_buffer(gpu.cmd, g.ib.buffer, 0, vk::IndexType::UINT32);
                        gpu.device.cmd_draw_indexed(gpu.cmd, g.count, b.count, 0, 0, b.first);
                    }
                }
            }
            // What's selected: a box round the prim, or round the stage when the shape is.
            let has_prim = selected.iter().any(|x| x.starts_with(&format!("{name},")));
            let key = format!("{picked}|{}|{}|{}", d.scene.item_version, d.scene.geoms.len(), if has_prim { selected.join(";") } else { String::new() });
            if d.wire_key != key {
                if let Some((b, _)) = d.wire.take() {
                    gpu.drop_buffer(b);
                }
                let lines = selection_wire(d, name, &selected, *picked);
                if !lines.is_empty() {
                    d.wire = Some((gpu.buffer_with(vk::BufferUsageFlags::VERTEX_BUFFER, &lines), lines.len() as u32));
                }
                d.wire_key = key;
            }
            if let Some((b, n)) = &d.wire {
                r.bind(gpu, Kind::LinesOnTop);
                r.set_line_width(gpu, 1.5);
                let mvp = (view_proj * d.world).as_mat4().to_cols_array();
                r.push(gpu, &Push { mvp, n0: [0.0; 4], n1: [0.0; 4], n2: [0.0; 4], color: [1.0; 4] });
                r.draw(gpu, b, None, *n);
            }
        }
        st.busy |= more;
    });
}

/// The prim under a ray, if it's nearer than `limit`. Returns its name.
pub fn pick(g: &Graph, origin: DVec3, dir: DVec3, limit: Option<DVec3>) -> Option<String> {
    STATE.with(|st| {
        let st = st.borrow();
        let mut best: Option<(String, f64)> = None;
        for (shape, d) in st.proxies.iter() {
            if !crate::viewport::visible(g, *shape) {
                continue;
            }
            let inv = d.world.inverse();
            let (o, dd) = (inv.transform_point3(origin), inv.transform_vector3(dir));
            let Some((path, _, hit)) = d.scene.raycast(o, dd) else { continue };
            let world = d.world.transform_point3(hit);
            let dist = (world - origin).dot(dir);
            if best.as_ref().is_none_or(|b| dist < b.1) {
                best = Some((prims::name_of(g, *shape, &path), dist));
            }
        }
        let (name, dist) = best?;
        if let Some(l) = limit {
            if (l - origin).dot(dir) < dist {
                return None;
            }
        }
        Some(name)
    })
}

/// Prims whose centre a screen test accepts, for marquee and lasso.
pub fn pick_inside(g: &Graph, view_proj: DMat4, size: DVec2, inside: &dyn Fn(DVec2) -> bool) -> Vec<String> {
    STATE.with(|st| {
        let st = st.borrow();
        let mut out: Vec<String> = Vec::new();
        for (shape, d) in st.proxies.iter() {
            if !crate::viewport::visible(g, *shape) {
                continue;
            }
            let mut seen = std::collections::HashSet::new();
            for it in &d.scene.items {
                if !seen.insert(it.prim.clone()) {
                    continue;
                }
                let geom = &d.scene.geoms[it.geom];
                let c = (geom.lo + geom.hi).as_dvec3() * 0.5;
                let w = view_proj * d.world * it.world;
                let clip = w * c.extend(1.0);
                if clip.w <= 0.0 {
                    continue;
                }
                let ndc = clip.truncate() / clip.w;
                let px = DVec2::new((ndc.x + 1.0) * 0.5 * size.x, (ndc.y + 1.0) * 0.5 * size.y);
                if inside(px) {
                    out.push(prims::name_of(g, *shape, &it.prim));
                }
            }
        }
        out
    })
}

/// World bounds of the stages, or of what's selected in them. Finishes reading first.
pub fn bounds(s: &mut Session, all: bool) -> Option<(DVec3, DVec3)> {
    sync(s);
    let selected = prims::selected();
    let names: Vec<(NodeId, String, bool)> = stages::all_proxies(&s.graph)
        .into_iter()
        .map(|n| {
            let up = s.graph.parent(n);
            let picked = s.graph.selection().contains(&n) || s.graph.selection().contains(&up);
            (n, s.graph.full_path(n), picked)
        })
        .collect();
    let nothing_selected = selected.is_empty() && s.graph.selection().is_empty();
    STATE.with(|st| {
        let st = st.borrow();
        let (mut lo, mut hi) = (DVec3::splat(f64::MAX), DVec3::splat(f64::MIN));
        let mut any = false;
        for (shape, name, picked) in names {
            let Some(d) = st.proxies.get(&shape) else { continue };
            let mut boxes = Vec::new();
            if all || *&picked || nothing_selected {
                boxes.extend(d.scene.bounds(None));
            } else {
                for p in selected.iter().filter_map(|x| x.strip_prefix(&format!("{name},"))) {
                    boxes.extend(d.scene.bounds(Some(p)));
                }
            }
            for (l, h) in boxes {
                for i in 0..8 {
                    let c = DVec3::new(if i & 1 == 0 { l.x } else { h.x }, if i & 2 == 0 { l.y } else { h.y }, if i & 4 == 0 { l.z } else { h.z });
                    let w = d.world.transform_point3(c);
                    lo = lo.min(w);
                    hi = hi.max(w);
                    any = true;
                }
            }
        }
        any.then_some((lo, hi))
    })
}

/// Where the manipulator sits for selected prims.
pub fn manip_pivot(s: &mut Session) -> Option<DVec3> {
    let last = prims::selected().last().cloned()?;
    let t = prims::resolve(&mut s.graph, &last).ok()?;
    let time = stages::time_of(&mut s.graph, t.shape);
    Some(prims::world_of(&mut s.graph, &t, time).w_axis.truncate())
}

/// Forgets drawing state for stages replaced.
pub fn stats() -> Vec<(NodeId, usize, usize, usize, usize)> {
    STATE.with(|st| {
        st.borrow()
            .proxies
            .iter()
            .map(|(k, d)| (*k, d.scene.prims_visited, d.scene.items.len(), d.scene.geoms.len(), d.batches.len()))
            .collect()
    })
}
