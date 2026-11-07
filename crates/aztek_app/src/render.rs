//! Viewport drawing: shaded triangles, lines and points, compiled from GLSL at start-up.

use ash::vk;
use bytemuck::{Pod, Zeroable};

use crate::gpu::{Buffer, Gpu};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct ShadedVertex {
    pub pos: [f32; 3],
    pub nrm: [f32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct LineVertex {
    pub pos: [f32; 3],
    pub color: [f32; 4],
}

/// 8-bit colour to the vertex's float colour.
pub fn c8(c: [u8; 4]) -> [f32; 4] {
    c.map(|x| x as f32 / 255.0)
}

/// 128 bytes: the minimum every device allows.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Push {
    pub mvp: [f32; 16],
    /// Rows of the normal matrix into view space; n0.w is the depth pull toward the camera, n1.w the point size.
    pub n0: [f32; 4],
    pub n1: [f32; 4],
    pub n2: [f32; 4],
    pub color: [f32; 4],
}

const SHADED_VS: &str = r#"#version 450
layout(location = 0) in vec3 a_pos;
layout(location = 1) in vec3 a_nrm;
layout(push_constant) uniform PC { mat4 mvp; vec4 n0; vec4 n1; vec4 n2; vec4 color; } pc;
layout(location = 0) out vec3 v_nrm;
void main() {
    gl_Position = pc.mvp * vec4(a_pos, 1.0);
    gl_Position.z *= 1.0 + pc.n0.w;
    v_nrm = vec3(dot(pc.n0.xyz, a_nrm), dot(pc.n1.xyz, a_nrm), dot(pc.n2.xyz, a_nrm));
}
"#;

const SHADED_FS: &str = r#"#version 450
layout(location = 0) in vec3 v_nrm;
layout(push_constant) uniform PC { mat4 mvp; vec4 n0; vec4 n1; vec4 n2; vec4 color; } pc;
layout(location = 0) out vec4 out_color;
void main() {
    vec3 n = normalize(v_nrm);
    // A key from above and left of the camera plus a weaker fill, both sides lit.
    vec3 k = normalize(vec3(-0.45, 0.6, 0.65));
    vec3 f = normalize(vec3(0.6, -0.2, 0.75));
    float d = 0.75 * abs(dot(n, k)) + 0.3 * abs(dot(n, f));
    vec3 lin = pc.color.rgb * (0.12 + 0.95 * d);
    out_color = vec4(pow(lin, vec3(1.0 / 2.2)), pc.color.a);
}
"#;

const LINE_VS: &str = r#"#version 450
layout(location = 0) in vec3 a_pos;
layout(location = 1) in vec4 a_color;
layout(push_constant) uniform PC { mat4 mvp; vec4 n0; vec4 n1; vec4 n2; vec4 color; } pc;
layout(location = 0) out vec4 v_color;
void main() {
    gl_Position = pc.mvp * vec4(a_pos, 1.0);
    gl_Position.z *= 1.0 + pc.n0.w;
    gl_PointSize = pc.n1.w;
    v_color = a_color * pc.color;
}
"#;

const LINE_FS: &str = r#"#version 450
layout(location = 0) in vec4 v_color;
layout(location = 0) out vec4 out_color;
void main() {
    out_color = v_color;
}
"#;

fn compile(src: &str, stage: naga::ShaderStage) -> Vec<u32> {
    let mut front = naga::front::glsl::Frontend::default();
    let module = front.parse(&naga::front::glsl::Options::from(stage), src).unwrap_or_else(|e| panic!("shader parse: {e:?}"));
    let info = naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all())
        .validate(&module)
        .unwrap_or_else(|e| panic!("shader validate: {e:?}"));
    let mut opts = naga::back::spv::Options { lang_version: (1, 0), ..Default::default() };
    // GLSL here is written for Vulkan already: no y flip.
    opts.flags.remove(naga::back::spv::WriterFlags::ADJUST_COORDINATE_SPACE);
    let pipe = naga::back::spv::PipelineOptions { shader_stage: stage, entry_point: "main".into() };
    naga::back::spv::write_vec(&module, &info, &opts, Some(&pipe)).expect("spv")
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Shaded,
    /// Shaded, alpha blended, no depth write: selected-face tint.
    Tint,
    Lines,
    /// Lines over everything: manipulators.
    LinesOnTop,
    Points,
}

pub struct Renderer {
    pub layout: vk::PipelineLayout,
    pipelines: Vec<(Kind, vk::Pipeline)>,
    pub line_width: bool,
}

impl Renderer {
    pub fn new(gpu: &Gpu) -> Renderer {
        unsafe {
            let d = &gpu.device;
            let range = [vk::PushConstantRange::default()
                .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT)
                .size(std::mem::size_of::<Push>() as u32)];
            let layout = d.create_pipeline_layout(&vk::PipelineLayoutCreateInfo::default().push_constant_ranges(&range), None).unwrap();
            let module = |src: &str, stage| {
                let code = compile(src, stage);
                d.create_shader_module(&vk::ShaderModuleCreateInfo::default().code(&code), None).unwrap()
            };
            let svs = module(SHADED_VS, naga::ShaderStage::Vertex);
            let sfs = module(SHADED_FS, naga::ShaderStage::Fragment);
            let lvs = module(LINE_VS, naga::ShaderStage::Vertex);
            let lfs = module(LINE_FS, naga::ShaderStage::Fragment);
            let feats = gpu.instance.get_physical_device_features(gpu.phys);
            let mut pipelines = Vec::new();
            for kind in [Kind::Shaded, Kind::Tint, Kind::Lines, Kind::LinesOnTop, Kind::Points] {
                let shaded = matches!(kind, Kind::Shaded | Kind::Tint);
                let (vs, fs) = if shaded { (svs, sfs) } else { (lvs, lfs) };
                let stages = [
                    vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::VERTEX).module(vs).name(c"main"),
                    vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::FRAGMENT).module(fs).name(c"main"),
                ];
                let (stride, attrs): (u32, Vec<vk::VertexInputAttributeDescription>) = if shaded {
                    (
                        24,
                        vec![
                            vk::VertexInputAttributeDescription::default().location(0).format(vk::Format::R32G32B32_SFLOAT).offset(0),
                            vk::VertexInputAttributeDescription::default().location(1).format(vk::Format::R32G32B32_SFLOAT).offset(12),
                        ],
                    )
                } else {
                    (
                        28,
                        vec![
                            vk::VertexInputAttributeDescription::default().location(0).format(vk::Format::R32G32B32_SFLOAT).offset(0),
                            vk::VertexInputAttributeDescription::default().location(1).format(vk::Format::R32G32B32A32_SFLOAT).offset(12),
                        ],
                    )
                };
                let binding = [vk::VertexInputBindingDescription::default().stride(stride).input_rate(vk::VertexInputRate::VERTEX)];
                let vin = vk::PipelineVertexInputStateCreateInfo::default().vertex_binding_descriptions(&binding).vertex_attribute_descriptions(&attrs);
                let topo = match kind {
                    Kind::Shaded | Kind::Tint => vk::PrimitiveTopology::TRIANGLE_LIST,
                    Kind::Lines | Kind::LinesOnTop => vk::PrimitiveTopology::LINE_LIST,
                    Kind::Points => vk::PrimitiveTopology::POINT_LIST,
                };
                let ia = vk::PipelineInputAssemblyStateCreateInfo::default().topology(topo);
                let vp = vk::PipelineViewportStateCreateInfo::default().viewport_count(1).scissor_count(1);
                let rs = vk::PipelineRasterizationStateCreateInfo::default()
                    .polygon_mode(vk::PolygonMode::FILL)
                    .cull_mode(vk::CullModeFlags::NONE)
                    .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
                    .line_width(1.0);
                let ms = vk::PipelineMultisampleStateCreateInfo::default().rasterization_samples(vk::SampleCountFlags::TYPE_1);
                let on_top = kind == Kind::LinesOnTop;
                let ds = vk::PipelineDepthStencilStateCreateInfo::default()
                    .depth_test_enable(!on_top)
                    .depth_write_enable(kind == Kind::Shaded || kind == Kind::Lines)
                    .depth_compare_op(vk::CompareOp::GREATER_OR_EQUAL);
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
                let mut dyns = vec![vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
                if feats.wide_lines != 0 && !shaded {
                    dyns.push(vk::DynamicState::LINE_WIDTH);
                }
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
                let p = d.create_graphics_pipelines(vk::PipelineCache::null(), &[info], None).map_err(|e| e.1).unwrap()[0];
                pipelines.push((kind, p));
            }
            for m in [svs, sfs, lvs, lfs] {
                d.destroy_shader_module(m, None);
            }
            Renderer { layout, pipelines, line_width: feats.wide_lines != 0 }
        }
    }

    pub fn bind(&self, gpu: &Gpu, kind: Kind) {
        let p = self.pipelines.iter().find(|(k, _)| *k == kind).unwrap().1;
        unsafe { gpu.device.cmd_bind_pipeline(gpu.cmd, vk::PipelineBindPoint::GRAPHICS, p) };
    }

    pub fn push(&self, gpu: &Gpu, p: &Push) {
        unsafe {
            gpu.device.cmd_push_constants(
                gpu.cmd,
                self.layout,
                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                0,
                bytemuck::bytes_of(p),
            )
        };
    }

    pub fn set_line_width(&self, gpu: &Gpu, w: f32) {
        if self.line_width {
            unsafe { gpu.device.cmd_set_line_width(gpu.cmd, w) };
        }
    }

    pub fn draw(&self, gpu: &Gpu, vb: &Buffer, ib: Option<(&Buffer, u32)>, count: u32) {
        unsafe {
            gpu.device.cmd_bind_vertex_buffers(gpu.cmd, 0, &[vb.buffer], &[0]);
            match ib {
                Some((ib, n)) => {
                    gpu.device.cmd_bind_index_buffer(gpu.cmd, ib.buffer, 0, vk::IndexType::UINT32);
                    gpu.device.cmd_draw_indexed(gpu.cmd, n, 1, 0, 0, 0);
                }
                None => gpu.device.cmd_draw(gpu.cmd, count, 1, 0, 0),
            }
        }
    }

    pub fn set_viewport(&self, gpu: &Gpu, x: f32, y: f32, w: f32, h: f32) {
        unsafe {
            gpu.device.cmd_set_viewport(gpu.cmd, 0, &[vk::Viewport { x, y, width: w, height: h, min_depth: 0.0, max_depth: 1.0 }]);
            gpu.device.cmd_set_scissor(
                gpu.cmd,
                0,
                &[vk::Rect2D {
                    offset: vk::Offset2D { x: x.max(0.0) as i32, y: y.max(0.0) as i32 },
                    extent: vk::Extent2D { width: w.max(1.0) as u32, height: h.max(1.0) as u32 },
                }],
            );
        }
    }
}
