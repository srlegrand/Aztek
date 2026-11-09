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
layout(location = 1) out vec3 v_rot;
void main() {
    gl_Position = pc.mvp * vec4(a_pos, 1.0);
    gl_Position.z *= 1.0 + pc.n0.w;
    v_nrm = vec3(dot(pc.n0.xyz, a_nrm), dot(pc.n1.xyz, a_nrm), dot(pc.n2.xyz, a_nrm));
    v_rot = vec3(dot(pc.n0.xyz, a_pos), dot(pc.n1.xyz, a_pos), dot(pc.n2.xyz, a_pos));
}
"#;

const SHADED_FS: &str = r#"#version 450
layout(location = 0) in vec3 v_nrm;
layout(location = 1) in vec3 v_rot;
layout(push_constant) uniform PC { mat4 mvp; vec4 n0; vec4 n1; vec4 n2; vec4 color; } pc;
layout(location = 0) out vec4 out_color;
void main() {
    vec3 n = normalize(v_nrm);
    // Flat shading: n2.w set, the facet's own normal.
    if (pc.n2.w > 0.5) {
        n = normalize(cross(dFdx(v_rot), dFdy(v_rot)));
    }
    // A key from above and left of the camera plus a weaker fill, both sides lit.
    vec3 k = normalize(vec3(-0.45, 0.6, 0.65));
    vec3 f = normalize(vec3(0.6, -0.2, 0.75));
    float d = 0.75 * abs(dot(n, k)) + 0.3 * abs(dot(n, f));
    vec3 lin = pc.color.rgb * (0.12 + 0.95 * d);
    out_color = vec4(pow(lin, vec3(1.0 / 2.2)), pc.color.a);
}
"#;

/// Push constants of the material pipelines. Asks more than the 128 bytes every device allows: see `Renderer::materials`.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct MatPush {
    pub base: Push,
    /// repeatU, repeatV, offsetU, offsetV.
    pub uv: [f32; 4],
    /// coverageU, coverageV, translateFrameU, translateFrameV.
    pub frame: [f32; 4],
    /// rotateUV and rotateFrame in radians, flags (1 wrapU, 2 wrapV, 4 mirrorU, 8 mirrorV, 16 has a colour map), alpha
    /// mode (0 base.color.a, 1 the alpha map's alpha, 2 its luminance).
    pub misc: [f32; 4],
    /// colorGain; w set when lit.
    pub gain: [f32; 4],
    /// colorOffset; w is diffuse.
    pub offs: [f32; 4],
    pub ambient: [f32; 4],
    pub incandescence: [f32; 4],
}

const MATERIAL_VS: &str = r#"#version 450
struct Light { vec4 pos; vec4 dir; vec4 color; vec4 params; vec4 a; vec4 b; vec4 c; };
layout(location = 0) in vec3 a_pos;
layout(location = 1) in vec3 a_nrm;
layout(location = 2) in vec2 a_uv;
layout(push_constant) uniform PC { mat4 mvp; vec4 n0; vec4 n1; vec4 n2; vec4 color; vec4 uv; vec4 frame; vec4 misc; vec4 gain; vec4 offs; vec4 ambient; vec4 incandescence; } pc;
layout(set = 2, binding = 0, std140) uniform Lights { mat4 inv_proj; ivec4 head; Light lights[16]; } lt;
layout(location = 0) out vec3 v_nrm;
layout(location = 1) out vec3 v_rot;
layout(location = 2) out vec2 v_uv;
layout(location = 3) out vec3 v_view;
void main() {
    gl_Position = pc.mvp * vec4(a_pos, 1.0);
    // What the lights need: the block's inverse projection takes the clip position back to view space.
    vec4 e = lt.inv_proj * gl_Position;
    v_view = e.xyz / e.w;
    gl_Position.z *= 1.0 + pc.n0.w;
    v_nrm = vec3(dot(pc.n0.xyz, a_nrm), dot(pc.n1.xyz, a_nrm), dot(pc.n2.xyz, a_nrm));
    v_rot = vec3(dot(pc.n0.xyz, a_pos), dot(pc.n1.xyz, a_pos), dot(pc.n2.xyz, a_pos));
    v_uv = a_uv;
}
"#;

const MATERIAL_FS: &str = r#"#version 450
struct Light { vec4 pos; vec4 dir; vec4 color; vec4 params; vec4 a; vec4 b; vec4 c; };
layout(location = 0) in vec3 v_nrm;
layout(location = 1) in vec3 v_rot;
layout(location = 2) in vec2 v_uv;
layout(location = 3) in vec3 v_view;
layout(set = 2, binding = 0, std140) uniform Lights { mat4 inv_proj; ivec4 head; Light lights[16]; } lt;
layout(set = 2, binding = 1, std140) uniform Speculars { vec4 spec[1024]; } sp;
layout(push_constant) uniform PC { mat4 mvp; vec4 n0; vec4 n1; vec4 n2; vec4 color; vec4 uv; vec4 frame; vec4 misc; vec4 gain; vec4 offs; vec4 ambient; vec4 incandescence; } pc;
layout(set = 0, binding = 0) uniform texture2D t_color;
layout(set = 0, binding = 1) uniform sampler s_color;
layout(set = 1, binding = 0) uniform texture2D t_alpha;
layout(set = 1, binding = 1) uniform sampler s_alpha;
layout(location = 0) out vec4 out_color;
vec2 turned(vec2 p, float a) {
    float c = cos(a);
    float s = sin(a);
    vec2 q = p - vec2(0.5);
    return vec2(c * q.x - s * q.y, s * q.x + c * q.y) + vec2(0.5);
}
float decay(float rate, float d) {
    return rate < 0.5 ? 1.0 : 1.0 / pow(max(d, 0.001), rate);
}
// How far through a volume light's shape a unit space point is: 0 at its middle, 1 at its boundary, past 1 outside.
float volume_reach(vec3 u, float shape, float cone_end) {
    if (shape < 0.5) {
        return max(abs(u.x), max(abs(u.y), abs(u.z)));
    }
    if (shape < 1.5) {
        return length(u);
    }
    float radius = shape < 2.5 ? 1.0 : mix(1.0, cone_end, clamp((u.z + 1.0) * 0.5, 0.0, 1.0));
    return max(length(u.xy) / max(radius, 0.001), abs(u.z));
}
// Every light on a point: x what diffuse takes, a highlight added to it. n faces the viewer's side already.
void shade(vec3 n, vec3 v, vec4 sa, vec4 sb, out vec3 diff, out vec3 high) {
    diff = vec3(0.0);
    high = vec3(0.0);
    int mask = int(pc.n1.w);
    for (int i = 0; i < lt.head.y; i += 1) {
        if (((mask >> i) & 1) == 0) {
            continue;
        }
        vec4 pos = lt.lights[i].pos;
        vec4 dir = lt.lights[i].dir;
        vec4 col = lt.lights[i].color;
        vec4 par = lt.lights[i].params;
        float kind = pos.w;
        vec3 c = col.rgb;
        vec3 l = vec3(0.0, 0.0, 1.0);
        if (kind < 0.5) {
            // Ambient: all round, ambientShade of it from the light's position.
            vec3 to = pos.xyz - v_view;
            l = normalize(to);
            diff += c * (1.0 - par.x);
            c *= par.x;
        } else if (kind < 1.5) {
            l = -dir.xyz;
        } else if (kind < 2.5) {
            vec3 to = pos.xyz - v_view;
            float d = length(to);
            l = to / max(d, 0.0001);
            c *= decay(dir.w, d);
        } else if (kind < 3.5) {
            vec3 to = pos.xyz - v_view;
            float d = length(to);
            l = to / max(d, 0.0001);
            float cone = dot(-l, dir.xyz);
            float edge = clamp((cone - par.x) / max(par.y - par.x, 0.00001), 0.0, 1.0);
            c *= decay(dir.w, d) * edge * pow(max(cone, 0.0), par.z);
        } else if (kind < 4.5) {
            // Area: the rectangle's nearest point to the surface stands for all of it, lit from one side.
            vec4 ua = lt.lights[i].a;
            vec4 ub = lt.lights[i].b;
            vec3 r = v_view - pos.xyz;
            float lu = max(dot(ua.xyz, ua.xyz), 0.000001);
            float lv = max(dot(ub.xyz, ub.xyz), 0.000001);
            vec3 q = pos.xyz + ua.xyz * clamp(dot(r, ua.xyz) / lu, -1.0, 1.0) + ub.xyz * clamp(dot(r, ub.xyz) / lv, -1.0, 1.0);
            vec3 to = q - v_view;
            float d = length(to);
            l = to / max(d, 0.0001);
            c *= decay(dir.w, d) * max(dot(-l, dir.xyz), 0.0);
        } else {
            // Volume: nothing outside its shape, fading from its middle to its boundary.
            vec4 p = vec4(v_view, 1.0);
            vec3 u = vec3(dot(lt.lights[i].a, p), dot(lt.lights[i].b, p), dot(lt.lights[i].c, p));
            float reach = volume_reach(u, par.x, par.w);
            float fade = clamp(1.0 - reach, 0.0, 1.0);
            c *= fade;
            vec3 to = pos.xyz - v_view;
            l = length(to) > 0.0001 ? normalize(to) : n;
            if (par.y > 1.5) {
                l = -dir.xyz;
            } else if (par.y > 0.5) {
                l = -l;
            }
            if (par.z > 0.5) {
                diff += c;
                c = vec3(0.0);
            }
        }
        float nl = dot(n, l);
        if (nl <= 0.0) {
            continue;
        }
        int emits = int(col.w);
        if ((emits & 1) != 0) {
            diff += c * nl;
        }
        if ((emits & 2) != 0 && sa.w > 0.5) {
            if (sa.w < 1.5) {
                vec3 r2 = reflect(-l, n);
                high += c * sa.rgb * pow(max(dot(r2, v), 0.0), sb.x);
            } else {
                vec3 h = normalize(l + v);
                float nh = max(dot(n, h), 0.0);
                float e2 = sb.x * sb.x;
                float d2 = e2 / (nh * nh * (e2 - 1.0) + 1.0);
                float rim = sb.y + (1.0 - sb.y) * pow(1.0 - max(dot(v, h), 0.0), 5.0);
                high += c * sa.rgb * d2 * d2 * rim;
            }
        }
    }
}
void main() {
    int flags = int(pc.misc.z);
    // As place2dTexture: the frame first, then repeat and offset inside it.
    vec2 p = (turned(v_uv, pc.misc.y) - pc.frame.zw) / pc.frame.xy;
    bool outside = ((flags & 1) == 0 && (p.x < 0.0 || p.x > 1.0)) || ((flags & 2) == 0 && (p.y < 0.0 || p.y > 1.0));
    vec2 q = turned(p * pc.uv.xy + pc.uv.zw, pc.misc.x);
    if ((flags & 4) != 0) {
        q.x = 1.0 - abs(mod(q.x, 2.0) - 1.0);
    }
    if ((flags & 8) != 0) {
        q.y = 1.0 - abs(mod(q.y, 2.0) - 1.0);
    }
    // V runs up from the image's bottom.
    vec2 st = vec2(q.x, 1.0 - q.y);
    vec4 texel = texture(sampler2D(t_color, s_color), st);
    vec4 mask = texture(sampler2D(t_alpha, s_alpha), st);
    vec3 base = pc.color.rgb;
    if ((flags & 16) != 0 && !outside) {
        base = texel.rgb * pc.gain.rgb + pc.offs.rgb;
    }
    float alpha = pc.color.a;
    if (pc.misc.w > 1.5) {
        alpha = dot(mask.rgb, vec3(0.3, 0.59, 0.11));
    } else if (pc.misc.w > 0.5) {
        alpha = mask.a;
    }
    vec3 lin = base + pc.incandescence.rgb;
    if (pc.gain.w > 0.5 && lt.head.x != 0) {
        vec3 own = pc.ambient.rgb * base + pc.incandescence.rgb;
        if (lt.head.x == 3) {
            lin = base + own;
        } else if (lt.head.x == 2) {
            lin = own;
        } else {
            vec3 v = lt.head.w != 0 ? vec3(0.0, 0.0, 1.0) : normalize(-v_view);
            // The facet as it faces the viewer says which side is seen, whatever way the triangles wind.
            vec3 facet = normalize(cross(dFdx(v_view), dFdy(v_view)));
            if (dot(facet, v) < 0.0) {
                facet = -facet;
            }
            vec3 n = normalize(v_nrm);
            bool back = dot(n, facet) < 0.0;
            if (pc.n2.w > 0.5) {
                n = facet;
            } else if (back) {
                n = -n;
            }
            lin = own;
            if (!back || lt.head.z != 0) {
                vec3 diff;
                vec3 high;
                int at = int(pc.ambient.w);
                shade(n, v, sp.spec[at * 2], sp.spec[at * 2 + 1], diff, high);
                lin += base * pc.offs.w * diff + high;
            }
        }
    } else if (pc.gain.w > 0.5) {
        vec3 n = normalize(v_nrm);
        if (pc.n2.w > 0.5) {
            n = normalize(cross(dFdx(v_rot), dFdy(v_rot)));
        }
        vec3 k = normalize(vec3(-0.45, 0.6, 0.65));
        vec3 f = normalize(vec3(0.6, -0.2, 0.75));
        float d = 0.75 * abs(dot(n, k)) + 0.3 * abs(dot(n, f));
        lin = base * pc.offs.w * (0.12 + 0.95 * d) + pc.ambient.rgb * base + pc.incandescence.rgb;
    }
    out_color = vec4(pow(max(lin, vec3(0.0)), vec3(1.0 / 2.2)), alpha);
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

pub(crate) fn compile(src: &str, stage: naga::ShaderStage) -> Vec<u32> {
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
    /// Points over everything: X-Ray Active Components.
    PointsOnTop,
    /// Shaded with the back faces left out: Backface Culling.
    ShadedCull,
    /// Triangles with a colour per vertex, unlit: weight painting's feedback.
    ColorTris,
    /// A material: its colour or file texture, with UVs in a second buffer.
    Material,
    MaterialCull,
    /// A see-through material: blended, no depth write. Drawn after everything solid.
    MaterialBlend,
    MaterialBlendCull,
}

pub struct Renderer {
    pub layout: vk::PipelineLayout,
    pipelines: Vec<(Kind, vk::Pipeline)>,
    pub line_width: bool,
    /// The device allows the material pipelines' push constants. Without them a material draws as a flat colour.
    pub materials: bool,
    material_layout: vk::PipelineLayout,
    /// A view's lights and highlights: set 2 of the material pipelines.
    pub light_layout: vk::DescriptorSetLayout,
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
            let feats = gpu.shared.features;
            let mut pipelines = Vec::new();
            for kind in [Kind::Shaded, Kind::Tint, Kind::Lines, Kind::LinesOnTop, Kind::Points, Kind::PointsOnTop, Kind::ShadedCull, Kind::ColorTris] {
                let shaded = matches!(kind, Kind::Shaded | Kind::Tint | Kind::ShadedCull);
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
                    Kind::Shaded | Kind::Tint | Kind::ShadedCull | Kind::ColorTris => vk::PrimitiveTopology::TRIANGLE_LIST,
                    Kind::Lines | Kind::LinesOnTop => vk::PrimitiveTopology::LINE_LIST,
                    Kind::Points | Kind::PointsOnTop => vk::PrimitiveTopology::POINT_LIST,
                    _ => unreachable!("made below"),
                };
                let ia = vk::PipelineInputAssemblyStateCreateInfo::default().topology(topo);
                let vp = vk::PipelineViewportStateCreateInfo::default().viewport_count(1).scissor_count(1);
                let rs = vk::PipelineRasterizationStateCreateInfo::default()
                    .polygon_mode(vk::PolygonMode::FILL)
                    .cull_mode(if kind == Kind::ShadedCull { vk::CullModeFlags::BACK } else { vk::CullModeFlags::NONE })
                    .front_face(vk::FrontFace::CLOCKWISE)
                    .line_width(1.0);
                let ms = vk::PipelineMultisampleStateCreateInfo::default().rasterization_samples(gpu.samples);
                let on_top = matches!(kind, Kind::LinesOnTop | Kind::PointsOnTop);
                let ds = vk::PipelineDepthStencilStateCreateInfo::default()
                    .depth_test_enable(!on_top)
                    .depth_write_enable(matches!(kind, Kind::Shaded | Kind::ShadedCull | Kind::Lines | Kind::ColorTris))
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
            let limits = gpu.shared.instance.get_physical_device_properties(gpu.shared.phys).limits;
            let materials = limits.max_push_constants_size as usize >= std::mem::size_of::<MatPush>();
            let mut material_layout = vk::PipelineLayout::null();
            let mut light_layout = vk::DescriptorSetLayout::null();
            if materials {
                let set = crate::textures::set_layout(gpu);
                let bindings = [
                    vk::DescriptorSetLayoutBinding::default().binding(0).descriptor_type(vk::DescriptorType::UNIFORM_BUFFER).descriptor_count(1).stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT),
                    vk::DescriptorSetLayoutBinding::default().binding(1).descriptor_type(vk::DescriptorType::UNIFORM_BUFFER).descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT),
                ];
                light_layout = d.create_descriptor_set_layout(&vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings), None).unwrap();
                let sets = [set, set, light_layout];
                let range = [vk::PushConstantRange::default()
                    .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT)
                    .size(std::mem::size_of::<MatPush>() as u32)];
                material_layout = d.create_pipeline_layout(&vk::PipelineLayoutCreateInfo::default().set_layouts(&sets).push_constant_ranges(&range), None).unwrap();
                let vs = module(MATERIAL_VS, naga::ShaderStage::Vertex);
                let fs = module(MATERIAL_FS, naga::ShaderStage::Fragment);
                for kind in [Kind::Material, Kind::MaterialCull, Kind::MaterialBlend, Kind::MaterialBlendCull] {
                    let stages = [
                        vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::VERTEX).module(vs).name(c"main"),
                        vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::FRAGMENT).module(fs).name(c"main"),
                    ];
                    let bindings = [
                        vk::VertexInputBindingDescription::default().binding(0).stride(24).input_rate(vk::VertexInputRate::VERTEX),
                        vk::VertexInputBindingDescription::default().binding(1).stride(8).input_rate(vk::VertexInputRate::VERTEX),
                    ];
                    let attrs = [
                        vk::VertexInputAttributeDescription::default().location(0).binding(0).format(vk::Format::R32G32B32_SFLOAT).offset(0),
                        vk::VertexInputAttributeDescription::default().location(1).binding(0).format(vk::Format::R32G32B32_SFLOAT).offset(12),
                        vk::VertexInputAttributeDescription::default().location(2).binding(1).format(vk::Format::R32G32_SFLOAT).offset(0),
                    ];
                    let vin = vk::PipelineVertexInputStateCreateInfo::default().vertex_binding_descriptions(&bindings).vertex_attribute_descriptions(&attrs);
                    let ia = vk::PipelineInputAssemblyStateCreateInfo::default().topology(vk::PrimitiveTopology::TRIANGLE_LIST);
                    let vp = vk::PipelineViewportStateCreateInfo::default().viewport_count(1).scissor_count(1);
                    let cull = matches!(kind, Kind::MaterialCull | Kind::MaterialBlendCull);
                    let rs = vk::PipelineRasterizationStateCreateInfo::default()
                        .polygon_mode(vk::PolygonMode::FILL)
                        .cull_mode(if cull { vk::CullModeFlags::BACK } else { vk::CullModeFlags::NONE })
                        .front_face(vk::FrontFace::CLOCKWISE)
                        .line_width(1.0);
                    let ms = vk::PipelineMultisampleStateCreateInfo::default().rasterization_samples(gpu.samples);
                    let ds = vk::PipelineDepthStencilStateCreateInfo::default()
                        .depth_test_enable(true)
                        .depth_write_enable(matches!(kind, Kind::Material | Kind::MaterialCull))
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
                        .layout(material_layout)
                        .render_pass(gpu.render_pass)
                        .subpass(0);
                    let p = d.create_graphics_pipelines(vk::PipelineCache::null(), &[info], None).map_err(|e| e.1).unwrap()[0];
                    pipelines.push((kind, p));
                }
                d.destroy_shader_module(vs, None);
                d.destroy_shader_module(fs, None);
            }
            Renderer { layout, pipelines, line_width: feats.wide_lines != 0, materials, material_layout, light_layout }
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

    /// Binds a material pipeline with its colour and alpha textures.
    pub fn bind_material(&self, gpu: &Gpu, kind: Kind, color: vk::DescriptorSet, alpha: vk::DescriptorSet, lights: vk::DescriptorSet) {
        self.bind(gpu, kind);
        unsafe { gpu.device.cmd_bind_descriptor_sets(gpu.cmd, vk::PipelineBindPoint::GRAPHICS, self.material_layout, 0, &[color, alpha, lights], &[]) };
    }

    pub fn push_material(&self, gpu: &Gpu, p: &MatPush) {
        unsafe {
            gpu.device.cmd_push_constants(gpu.cmd, self.material_layout, vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT, 0, bytemuck::bytes_of(p));
        }
    }

    /// A range of an index buffer, with UVs in a second vertex buffer.
    pub fn draw_material(&self, gpu: &Gpu, vb: &Buffer, uvs: &Buffer, ib: &Buffer, first: u32, count: u32) {
        unsafe {
            gpu.device.cmd_bind_vertex_buffers(gpu.cmd, 0, &[vb.buffer, uvs.buffer], &[0, 0]);
            gpu.device.cmd_bind_index_buffer(gpu.cmd, ib.buffer, 0, vk::IndexType::UINT32);
            gpu.device.cmd_draw_indexed(gpu.cmd, count, 1, first, 0, 0);
        }
    }

    /// A range of an index buffer with the plain vertex format.
    pub fn draw_range(&self, gpu: &Gpu, vb: &Buffer, ib: &Buffer, first: u32, count: u32) {
        unsafe {
            gpu.device.cmd_bind_vertex_buffers(gpu.cmd, 0, &[vb.buffer], &[0]);
            gpu.device.cmd_bind_index_buffer(gpu.cmd, ib.buffer, 0, vk::IndexType::UINT32);
            gpu.device.cmd_draw_indexed(gpu.cmd, count, 1, first, 0, 0);
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A bad shader stops the whole viewport at start-up, and shows only with a device.
    #[test]
    fn every_shader_compiles() {
        for (src, stage) in [(SHADED_VS, naga::ShaderStage::Vertex), (SHADED_FS, naga::ShaderStage::Fragment), (LINE_VS, naga::ShaderStage::Vertex), (LINE_FS, naga::ShaderStage::Fragment), (MATERIAL_VS, naga::ShaderStage::Vertex), (MATERIAL_FS, naga::ShaderStage::Fragment)] {
            assert!(!compile(src, stage).is_empty());
        }
    }

    #[test]
    fn the_light_block_is_what_the_shader_declares() {
        use crate::lighting::{GpuLight, LightBlock, MAX_LIGHTS};
        assert_eq!(std::mem::size_of::<GpuLight>(), 7 * 16);
        assert_eq!(std::mem::size_of::<LightBlock>(), 64 + 16 + 7 * 16 * MAX_LIGHTS);
        assert!(MATERIAL_FS.contains("Light lights[16]") && MAX_LIGHTS == 16);
    }

    /// The shader's lighting is `aztek_scene::lights::viewport_diffuse` in GLSL, and the renderer is tested against that
    /// (tests/light_parity.rs). These are the lines that formula mirrors: change one side, change the other.
    #[test]
    fn the_shader_lights_as_the_shared_formula() {
        for line in [
            "return rate < 0.5 ? 1.0 : 1.0 / pow(max(d, 0.001), rate);",
            "diff += c * (1.0 - par.x);",
            "float edge = clamp((cone - par.x) / max(par.y - par.x, 0.00001), 0.0, 1.0);",
            "c *= decay(dir.w, d) * edge * pow(max(cone, 0.0), par.z);",
            "c *= decay(dir.w, d) * max(dot(-l, dir.xyz), 0.0);",
            "float fade = clamp(1.0 - reach, 0.0, 1.0);",
            "diff += c * nl;",
        ] {
            assert!(MATERIAL_FS.contains(line), "the viewport's lighting changed: {line}");
        }
    }
}
