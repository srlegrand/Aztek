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

/// A corner of a line drawn as a band of triangles (the grid, #203): both ends of the line, its colour, which end this
/// corner is at (0 or 1) and which side (-1 or 1).
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct SmoothLineVertex {
    pub p0: [f32; 3],
    pub p1: [f32; 3],
    pub color: [f32; 4],
    pub tv: [f32; 2],
}

/// Line pairs as bands of two triangles each, for Kind::SmoothLines.
pub fn smooth_lines(lines: &[LineVertex]) -> Vec<SmoothLineVertex> {
    let mut out = Vec::with_capacity(lines.len() * 3);
    for pair in lines.chunks_exact(2) {
        let (a, b) = (pair[0], pair[1]);
        let v = |t: f32, side: f32| SmoothLineVertex { p0: a.pos, p1: b.pos, color: if t < 0.5 { a.color } else { b.color }, tv: [t, side] };
        out.extend([v(0.0, -1.0), v(1.0, -1.0), v(1.0, 1.0), v(0.0, -1.0), v(1.0, 1.0), v(0.0, 1.0)]);
    }
    out
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

/// The Realtime renderer's push constants: the material's, with its OpenPBR coat and transmission added. 256 bytes is the
/// most many devices allow; a device with less draws Viewport 2.0 instead.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct RtPush {
    pub base: Push,
    /// repeatU, repeatV, offsetU, offsetV; the realtime shader reads the wrap and mirror flags in `misc`.
    pub uv: [f32; 4],
    /// metalness, specular roughness, diffuse roughness, specular weight.
    pub frame: [f32; 4],
    /// rotation UV and frame, flags (1 wrapU, 2 wrapV, 4 mirrorU, 8 mirrorV, 16 has a colour map), alpha mode.
    pub misc: [f32; 4],
    /// colourGain; w set when lit.
    pub gain: [f32; 4],
    /// colourOffset; w is the base weight.
    pub offs: [f32; 4],
    /// The specular colour; w is the highlight to add from the material's own model (unused here).
    pub ambient: [f32; 4],
    /// The emitted colour; w is the specular IOR.
    pub incandescence: [f32; 4],
    /// coat weight, coat roughness, coat IOR, transmission weight.
    pub coat: [f32; 4],
}

const MATERIAL_VS: &str = r#"#version 450
struct Light { vec4 pos; vec4 dir; vec4 color; vec4 params; vec4 a; vec4 b; vec4 c; };
layout(location = 0) in vec3 a_pos;
layout(location = 1) in vec3 a_nrm;
layout(location = 2) in vec2 a_uv;
layout(push_constant) uniform PC { mat4 mvp; vec4 n0; vec4 n1; vec4 n2; vec4 color; vec4 uv; vec4 frame; vec4 misc; vec4 gain; vec4 offs; vec4 ambient; vec4 incandescence; } pc;
layout(set = 2, binding = 0, std140) uniform LightsBlock { mat4 inv_proj; ivec4 head; mat4 shadow_matrix; vec4 shadow; vec4 shadow_pos; vec4 shadow_range; vec4 env; vec4 env_rot; mat4 view_to_world; Light lights[16]; } lt;
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
layout(set = 2, binding = 0, std140) uniform LightsBlock { mat4 inv_proj; ivec4 head; mat4 shadow_matrix; vec4 shadow; vec4 shadow_pos; vec4 shadow_range; vec4 env; vec4 env_rot; mat4 view_to_world; Light lights[16]; } lt;
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
            c *= decay(dir.w, d) * max(dot(-l, dir.xyz), 0.0) * par.y;
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

const REALTIME_VS: &str = r#"#version 450
struct Light { vec4 pos; vec4 dir; vec4 color; vec4 params; vec4 a; vec4 b; vec4 c; };
layout(location = 0) in vec3 a_pos;
layout(location = 1) in vec3 a_nrm;
layout(location = 2) in vec2 a_uv;
layout(push_constant) uniform PC { mat4 mvp; vec4 n0; vec4 n1; vec4 n2; vec4 color; vec4 uv; vec4 frame; vec4 misc; vec4 gain; vec4 offs; vec4 ambient; vec4 incandescence; vec4 coat; } pc;
layout(set = 2, binding = 0, std140) uniform LightsBlock { mat4 inv_proj; ivec4 head; mat4 shadow_matrix; vec4 shadow; vec4 shadow_pos; vec4 shadow_range; vec4 env; vec4 env_rot; mat4 view_to_world; Light lights[16]; } lt;
layout(location = 0) out vec3 v_nrm;
layout(location = 1) out vec3 v_rot;
layout(location = 2) out vec2 v_uv;
layout(location = 3) out vec3 v_view;
void main() {
    gl_Position = pc.mvp * vec4(a_pos, 1.0);
    vec4 e = lt.inv_proj * gl_Position;
    v_view = e.xyz / e.w;
    gl_Position.z *= 1.0 + pc.n0.w;
    v_nrm = vec3(dot(pc.n0.xyz, a_nrm), dot(pc.n1.xyz, a_nrm), dot(pc.n2.xyz, a_nrm));
    v_rot = vec3(dot(pc.n0.xyz, a_pos), dot(pc.n1.xyz, a_pos), dot(pc.n2.xyz, a_pos));
    v_uv = a_uv;
}
"#;

// The OpenPBR surface as the Realtime renderer draws it: GGX specular and coat, Lambert base, an image or a sky dome
// for image-based lighting, and the scene's one shadow map. The light block, all six light types and the falloffs are the
// same as the legacy material shader's, so a light lights the same in either renderer.
/// The colour attachments' blending for a pipeline in the scene pass: the picture, the surface colour bounce light reads,
/// which only the Realtime renderer's materials write, and the two material images the screen-space reflection pass reads
/// (#302).
pub(crate) fn scene_blend(albedo: bool, mat: bool) -> [vk::PipelineColorBlendAttachmentState; 4] {
    let blend = vk::PipelineColorBlendAttachmentState::default()
        .blend_enable(true)
        .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA)
        .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
        .color_blend_op(vk::BlendOp::ADD)
        .src_alpha_blend_factor(vk::BlendFactor::ONE)
        .dst_alpha_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
        .alpha_blend_op(vk::BlendOp::ADD)
        .color_write_mask(vk::ColorComponentFlags::RGBA);
    let none = blend.color_write_mask(vk::ColorComponentFlags::empty());
    let material = vk::PipelineColorBlendAttachmentState::default().blend_enable(false).color_write_mask(vk::ColorComponentFlags::RGBA);
    [blend, if albedo { blend } else { none }, if mat { material } else { none }, if mat { material } else { none }]
}

const REALTIME_FS: &str = r#"#version 450
struct Light { vec4 pos; vec4 dir; vec4 color; vec4 params; vec4 a; vec4 b; vec4 c; };
layout(location = 0) in vec3 v_nrm;
layout(location = 1) in vec3 v_rot;
layout(location = 2) in vec2 v_uv;
layout(location = 3) in vec3 v_view;
layout(set = 2, binding = 0, std140) uniform LightsBlock { mat4 inv_proj; ivec4 head; mat4 shadow_matrix; vec4 shadow; vec4 shadow_pos; vec4 shadow_range; vec4 env; vec4 env_rot; mat4 view_to_world; Light lights[16]; } lt;
layout(push_constant) uniform PC { mat4 mvp; vec4 n0; vec4 n1; vec4 n2; vec4 color; vec4 uv; vec4 frame; vec4 misc; vec4 gain; vec4 offs; vec4 ambient; vec4 incandescence; vec4 coat; } pc;
layout(set = 0, binding = 0) uniform texture2D t_color;
layout(set = 0, binding = 1) uniform sampler s_color;
layout(set = 1, binding = 0) uniform texture2D t_alpha;
layout(set = 1, binding = 1) uniform sampler s_alpha;
layout(set = 3, binding = 0) uniform texture2D t_env;
layout(set = 3, binding = 1) uniform sampler s_env;
layout(set = 4, binding = 0) uniform texture2D t_shadow;
layout(set = 4, binding = 1) uniform sampler s_shadow;
layout(set = 4, binding = 2) uniform textureCube t_shadow_cube;
layout(set = 4, binding = 3) uniform sampler s_shadow_cube;
layout(location = 0) out vec4 out_color;
// The surface's diffuse colour, linear: what bounce light arriving here is tinted by.
layout(location = 1) out vec4 out_albedo;
// The screen-space reflection pass's material (#302): the fresnel-weighted reflection colour in rgb, the roughness it
// looks the environment up with in a. Zero where nothing is drawn.
layout(location = 2) out vec4 out_mat;
// Transmission, IOR as (IOR - 1) / 2, metalness, and 1 where this shader drew a material.
layout(location = 3) out vec4 out_aux;

const float PI = 3.141592653589793;
const float INV_PI = 0.3183098861837907;
const float TWO_PI = 6.283185307179586;
vec2 turned(vec2 p, float a) {
    float c = cos(a);
    float s = sin(a);
    vec2 q = p - vec2(0.5);
    return vec2(c * q.x - s * q.y, s * q.x + c * q.y) + vec2(0.5);
}
float decay(float rate, float d) {
    return rate < 0.5 ? 1.0 : 1.0 / pow(max(d, 0.001), rate);
}
float volume_reach(vec3 u, float shape, float cone_end) {
    if (shape < 0.5) return max(abs(u.x), max(abs(u.y), abs(u.z)));
    if (shape < 1.5) return length(u);
    float radius = shape < 2.5 ? 1.0 : mix(1.0, cone_end, clamp((u.z + 1.0) * 0.5, 0.0, 1.0));
    return max(length(u.xy) / max(radius, 0.001), abs(u.z));
}
vec3 qrot(vec4 q, vec3 v) {
    return v + 2.0 * cross(q.xyz, cross(q.xyz, v) + q.w * v);
}
float distribution_ggx(float nh, float rough) {
    float a = rough * rough;
    float a2 = a * a;
    float d = nh * nh * (a2 - 1.0) + 1.0;
    return a2 / (PI * d * d);
}
float geometry_smith(float nv, float nl, float rough) {
    float k = (rough + 1.0) * (rough + 1.0) / 8.0;
    return (nv / (nv * (1.0 - k) + k)) * (nl / (nl * (1.0 - k) + k));
}
vec3 fresnel(float u, vec3 f0) {
    return f0 + (vec3(1.0) - f0) * pow(1.0 - u, 5.0);
}
float f_ior(float ior) {
    float f = (ior - 1.0) / (ior + 1.0);
    return f * f;
}

// The distance from the light, along its face's axis, to what the cube shadow map saw in that direction.
float cube_distance(vec3 d, float near, float far) {
    float depth = texture(samplerCube(t_shadow_cube, s_shadow_cube), d).r;
    return far * near / (far + depth * (near - far));
}
// A face's projection stores the distance along its own axis. Every comparison here is in the distance from the light
// instead, which is the same number on both sides of a face boundary: a tap that crosses one keeps its metric, and the
// compare cannot step at the seam. Taking the axis distance of the receiver, of a tap and of a blocker in different
// faces mixes three axes.
float radial_at(float face_distance, vec3 dir) {
    return face_distance / max(max(abs(dir.x), abs(dir.y)), abs(dir.z));
}
// How much of the light reaches this point from the shadow map, 0 in shadow to 1 lit. Outside the map it is lit.
float shadow_at(vec3 p, vec3 n) {
    if (lt.shadow.w < 0.5) return 1.0;
    // Two different quantities keep a surface off its own depth, and they are not interchangeable. The first is a
    // world-space offset along the normal, about one of the map's texels: it moves the point out of the caster's own
    // depth. The second is the compared tolerance, the depth a texel spans along the light on this surface, which
    // grows with the slope. A constant belongs in neither.
    if (lt.shadow.w > 1.5) {
        // The light's position and `p` are view space; the six faces look along the world's axes, so the sampled
        // direction and the depth's distance have to be world space.
        int li = int(lt.shadow.z);
        vec3 l = lt.lights[li].pos.xyz - p;
        float nl = max(dot(n, normalize(l + vec3(1e-6))), 0.0);
        float sin_theta = sqrt(max(1.0 - nl * nl, 0.0));
        float near = max(lt.shadow_range.x, 0.01);
        float far = max(lt.shadow_range.y, near + 0.02);
        vec3 to = p - lt.shadow_pos.xyz;
        float reach = length(to);
        if (reach < 1e-4) return 1.0;
        // The offset: a cube face's texel at the receiver, in world units, and a little more as the surface turns
        // away from the light, where one texel covers more of it.
        float texel_world = reach * 2.0 * lt.shadow.y;
        vec3 q3 = p + n * (texel_world * (sin_theta + 0.05));
        to = q3 - lt.shadow_pos.xyz;
        reach = length(to);
        vec3 wto = (lt.view_to_world * vec4(to, 0.0)).xyz;
        vec3 d = normalize(wto);
        if (reach < near || reach > far) return 1.0;
        float size = lt.lights[li].pos.w > 3.5 && lt.lights[li].pos.w < 4.5 ? 0.5 * (length(lt.lights[li].a.xyz) + length(lt.lights[li].b.xyz)) : 0.0;
        // The compared tolerance: the distance along the light that one texel spans on this surface, which is the
        // texel's footprint over the cosine to the light.
        float tan_theta = sin_theta / max(nl, 0.15);
        float tol = texel_world * (1.0 + 2.0 * tan_theta);
        vec3 t1 = normalize(cross(d, abs(d.y) < 0.9 ? vec3(0.0, 1.0, 0.0) : vec3(1.0, 0.0, 0.0)));
        vec3 t2 = cross(d, t1);
        // The penumbra's width follows the nearest blocker in the emitter's cone, and the disc that samples it is held
        // to a texel budget: taps spread over a large disc are so far apart that their hit pattern is a fan of hard
        // patches, which is what the floor showed as clouds. The nearest blocker is one number, so its distance does
        // not step as taps enter and leave the cone.
        float spread = 2.0 * lt.shadow.y * 4.0;
        if (size > 0.0) {
            float search = clamp(size / max(reach, 0.001), 0.0, 2.0 * lt.shadow.y * 48.0);
            float blocker = 1e20;
            for (int i = 0; i < 12; ++i) {
                float r = sqrt((float(i) + 0.5) / 12.0) * search;
                float a = float(i) * 2.39996323;
                vec3 dir = normalize(d + (t1 * cos(a) + t2 * sin(a)) * r);
                float stored = radial_at(cube_distance(dir, near, far), dir);
                if (stored + tol < reach) {
                    blocker = min(blocker, stored);
                }
            }
            if (blocker < 1e19) {
                float b = blocker;
                spread = clamp(size * (reach - b) / max(b * reach, 0.0001), 2.0 * lt.shadow.y, 2.0 * lt.shadow.y * 24.0);
            }
        }
        // The taps are fixed, never rotated per pixel or per frame.
        float lit = 0.0;
        float taps = 0.0;
        if (size <= 0.0) {
            // A point light has no size: its edge is the map's. A 3 by 3 gather of bilinear depths, one texel apart,
            // with the gradient compare, gives a continuous edge instead of the single fetch's texel-sized steps.
            for (int y = -1; y <= 1; ++y) {
                for (int x = -1; x <= 1; ++x) {
                    vec3 dir = normalize(d + (t1 * float(x) + t2 * float(y)) * 2.0 * lt.shadow.y);
                    float stored = radial_at(cube_distance(dir, near, far), dir);
                    lit += smoothstep(-tol, tol, stored + tol - reach);
                    taps += 1.0;
                }
            }
        } else {
            // An area light's penumbra is sampled on one fixed golden-angle disc, over thirty-two taps rather than
            // sixteen: with the gradient compare the levels and the disc's spokes read as a smooth edge, not a star.
            for (int i = 0; i < 32; ++i) {
                float t = (float(i) + 0.5) / 32.0;
                float a = float(i) * 2.39996323;
                float r = sqrt(t) * spread;
                vec3 dir = normalize(d + (t1 * cos(a) + t2 * sin(a)) * r);
                float stored = radial_at(cube_distance(dir, near, far), dir);
                lit += smoothstep(-tol, tol, stored + tol - reach);
                taps += 1.0;
            }
        }
        return lit / taps;
    }
    // A single map: a directional light's direction is the depth axis, a spot's points from its position.
    int li = int(lt.shadow.z);
    vec3 toward = lt.lights[li].pos.w < 1.5 ? -lt.lights[li].dir.xyz : lt.lights[li].pos.xyz - p;
    float nl = max(dot(n, normalize(toward + vec3(1e-6))), 0.0);
    float sin_theta = sqrt(max(1.0 - nl * nl, 0.0));
    float tan_theta = sin_theta / max(nl, 0.15);
    vec3 q3 = p + n * (lt.shadow.x * (sin_theta + 0.05));
    vec4 sc = lt.shadow_matrix * vec4(q3, 1.0);
    // Clip x and y run -1 to 1, so they go to 0 to 1; Vulkan's clip z already runs 0 to 1 and must not be halved.
    vec3 q = vec3(sc.xy / sc.w * 0.5 + 0.5, sc.z / sc.w);
    if (q.x < 0.0 || q.x > 1.0 || q.y < 0.0 || q.y > 1.0 || q.z > 1.0 || q.z < 0.0) return 1.0;
    // The tolerance in the map's own depth: where one texel's worth of world distance lands in clip space.
    float tol_world = lt.shadow.x * (1.0 + 2.0 * tan_theta);
    vec4 sc2 = lt.shadow_matrix * vec4(q3 - normalize(toward) * tol_world, 1.0);
    float tol = abs(q.z - sc2.z / sc2.w) + 1e-6;
    float texel = lt.shadow.y;
    float sum = 0.0;
    // A 5 by 5 gather, bilinearly read: the map's edge is chunky at a close view where one texel is many pixels, and
    // it crawls as the camera moves. The wider filtered kernel turns that into a soft penumbra, and each tap's compare
    // is a gradient over the tolerance so the edge has no steps.
    for (int y = -2; y <= 2; y += 1) {
        for (int x = -2; x <= 2; x += 1) {
            float d = texture(sampler2D(t_shadow, s_shadow), q.xy + vec2(float(x), float(y)) * texel).r;
            sum += smoothstep(-tol, tol, d + tol - q.z);
        }
    }
    // The map's border is where its coverage ends, not a shadow: fade it back to lit over a few texels.
    float border = min(min(q.x, 1.0 - q.x), min(q.y, 1.0 - q.y)) / texel;
    return mix(1.0, sum / 25.0, smoothstep(0.0, 4.0, border));
}
// What a rectangle of light sends to a matte surface at x facing n, as its radiance times this: the exact integral of
// the cosines over the distance squared across it (Lambert's polygon formula). Corners p0..p3 go round it so that
// cross(p1 - p0, p3 - p0) points away from the side it lights.
float rect_irradiance(vec3 x, vec3 n, vec3 p0, vec3 p1, vec3 p2, vec3 p3) {
    vec3 r0 = normalize(p0 - x), r1 = normalize(p1 - x), r2 = normalize(p2 - x), r3 = normalize(p3 - x);
    float s = 0.0;
    vec3 e;
    e = cross(r0, r1); s += acos(clamp(dot(r0, r1), -1.0, 1.0)) * dot(e, n) / max(length(e), 1e-7);
    e = cross(r1, r2); s += acos(clamp(dot(r1, r2), -1.0, 1.0)) * dot(e, n) / max(length(e), 1e-7);
    e = cross(r2, r3); s += acos(clamp(dot(r2, r3), -1.0, 1.0)) * dot(e, n) / max(length(e), 1e-7);
    e = cross(r3, r0); s += acos(clamp(dot(r3, r0), -1.0, 1.0)) * dot(e, n) / max(length(e), 1e-7);
    return max(0.5 * s, 0.0);
}
// One light: what it adds to the base layer, and to the coat. Mirrors `aztek_scene::lights::viewport_diffuse`, but for
// an area light that falls off with distance, whose diffuse is the rectangle's exact irradiance as Harold's.
void direct(vec3 base, vec3 n, vec3 v, out vec3 diff, out vec3 spec, out vec3 coat_spec) {
    diff = vec3(0.0);
    spec = vec3(0.0);
    coat_spec = vec3(0.0);
    int mask = int(pc.n1.w);
    float rough = max(pc.frame.y, 0.01);
    float coat_rough = max(pc.coat.y, 0.01);
    float nv = max(dot(n, v), 0.001);
    float f0d = f_ior(max(pc.incandescence.w, 1.0));
    vec3 f0 = mix(vec3(f0d) * pc.ambient.rgb * pc.frame.w, base, clamp(pc.frame.x, 0.0, 1.0));
    vec3 coat_f0 = vec3(f_ior(max(pc.coat.z, 1.0)));
    for (int i = 0; i < lt.head.y; i += 1) {
        if (((mask >> i) & 1) == 0) continue;
        vec4 pos = lt.lights[i].pos;
        vec4 dir = lt.lights[i].dir;
        vec4 col = lt.lights[i].color;
        vec4 par = lt.lights[i].params;
        float kind = pos.w;
        // Harold scales the viewport's lights by pi so a matte surface's /pi matches it. Scaling the direct terms here
        // gives the same radiance for the same colour and intensity, so both renderers need the same light settings.
        vec3 c = col.rgb * PI;
        vec3 l = vec3(0.0, 0.0, 1.0);
        // An area light's diffuse, worked out over its whole rectangle; negative while the light has none.
        vec3 area_diff = vec3(-1.0);
        // An area light's angular radius at this point. Its highlight is not a point's: the lobe has to carry the
        // emitter's size, or a large light close to a floor reflects as a small, blown plate at the nearest point.
        float area_angle = 0.0;
        if (kind < 0.5) {
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
            // A photometric light's IES profile: 12 vertical samples, averaged around each ring, interpolated.
            if (par.w > 0.5) {
                vec3 to = -l;
                float angle = acos(clamp(dot(to, dir.xyz), -1.0, 1.0)) / PI * 11.0;
                float k = floor(angle);
                float t = angle - k;
                vec4 A = lt.lights[i].a;
                vec4 B = lt.lights[i].b;
                vec4 C = lt.lights[i].c;
                float s0, s1;
                if (k < 1.0) { s0 = A.x; s1 = A.y; }
                else if (k < 2.0) { s0 = A.y; s1 = A.z; }
                else if (k < 3.0) { s0 = A.z; s1 = A.w; }
                else if (k < 4.0) { s0 = A.w; s1 = B.x; }
                else if (k < 5.0) { s0 = B.x; s1 = B.y; }
                else if (k < 6.0) { s0 = B.y; s1 = B.z; }
                else if (k < 7.0) { s0 = B.z; s1 = B.w; }
                else if (k < 8.0) { s0 = B.w; s1 = C.x; }
                else if (k < 9.0) { s0 = C.x; s1 = C.y; }
                else if (k < 10.0) { s0 = C.y; s1 = C.z; }
                else { s0 = C.z; s1 = C.w; }
                c *= mix(s0, s1, t);
            }
        } else if (kind < 4.5) {
            vec4 ua = lt.lights[i].a;
            vec4 ub = lt.lights[i].b;
            vec3 r = v_view - pos.xyz;
            float lu = max(dot(ua.xyz, ua.xyz), 0.000001);
            float lv = max(dot(ub.xyz, ub.xyz), 0.000001);
            vec3 q = pos.xyz + ua.xyz * clamp(dot(r, ua.xyz) / lu, -1.0, 1.0) + ub.xyz * clamp(dot(r, ub.xyz) / lv, -1.0, 1.0);
            vec3 to = q - v_view;
            float d = length(to);
            l = to / max(d, 0.0001);
            // The emitter's half size over its distance: how wide it looks from here.
            area_angle = 0.5 * (length(ua.xyz) + length(ub.xyz)) / max(d, 0.0001);
            if (dir.w > 1.5) {
                // Falling off as the reference renderer's: its radiance (colour, intensity and exposure, normalised over the area, as
                // Harold's: without the pi the viewport's own lights take) through the rectangle's exact irradiance for
                // the diffuse, and as a point of the same power at the nearest point for the base of the highlight.
                // The highlight's lobe is widened by the emitter's angular size below, as a real one is.
                vec3 radiance = col.rgb * par.x;
                vec3 su = ua.xyz, sv = ub.xyz;
                bool flip = dot(cross(su, sv), dir.xyz) > 0.0;
                vec3 p0 = pos.xyz - su - sv, p1 = pos.xyz + su - sv, p2 = pos.xyz + su + sv, p3 = pos.xyz - su + sv;
                float e = dot(v_view - pos.xyz, dir.xyz) > 0.0 ? (flip ? rect_irradiance(v_view, n, p0, p3, p2, p1) : rect_irradiance(v_view, n, p0, p1, p2, p3)) : 0.0;
                area_diff = radiance * e;
                c = radiance * 4.0 * length(cross(su, sv)) * max(dot(-l, dir.xyz), 0.0) / max(d * d, 0.000001);
            } else {
                c *= decay(dir.w, d) * max(dot(-l, dir.xyz), 0.0) * par.y;
            }
        } else {
            vec4 p = vec4(v_view, 1.0);
            vec3 u = vec3(dot(lt.lights[i].a, p), dot(lt.lights[i].b, p), dot(lt.lights[i].c, p));
            float reach = volume_reach(u, par.x, par.w);
            c *= clamp(1.0 - reach, 0.0, 1.0);
            vec3 to = pos.xyz - v_view;
            l = length(to) > 0.0001 ? normalize(to) : n;
            if (par.y > 1.5) l = -dir.xyz;
            else if (par.y > 0.5) l = -l;
            if (par.z > 0.5) {
                diff += c;
                c = vec3(0.0);
            }
        }
        int emits = int(col.w);
        float lit = i == int(lt.shadow.z) ? shadow_at(v_view, n) : 1.0;
        if (area_diff.x >= 0.0 && (emits & 1) != 0) {
            diff += area_diff * lit;
        }
        float nl = dot(n, l);
        if (nl <= 0.0) continue;
        if ((emits & 1) != 0 && area_diff.x < 0.0) {
            diff += c * nl * lit;
        }
        if ((emits & 2) != 0 && nl > 0.0) {
            // The lobe's width is the surface's own roughness combined with the emitter's angular radius: two widths
            // add in quadrature. A point light leaves the roughness alone; a broad emitter near the surface widens
            // the highlight until its peak sits inside the shoulder instead of clipping as a plate.
            float look_rough = min(sqrt(rough * rough + area_angle * area_angle), 1.0);
            float look_coat = min(sqrt(coat_rough * coat_rough + area_angle * area_angle), 1.0);
            vec3 h = normalize(l + v);
            float nh = max(dot(n, h), 0.0);
            vec3 f = fresnel(max(dot(h, v), 0.0), f0);
            float d = distribution_ggx(nh, look_rough);
            float g = geometry_smith(nv, nl, look_rough);
            spec += c * lit * (d * g * f / max(4.0 * nv * nl, 0.0001)) * nl;
            if (pc.coat.x > 0.0) {
                vec3 cf = fresnel(max(dot(h, v), 0.0), coat_f0);
                float cd = distribution_ggx(nh, look_coat);
                float cg = geometry_smith(nv, nl, look_coat);
                coat_spec += c * lit * pc.coat.x * (cd * cg * cf / max(4.0 * nv * nl, 0.0001)) * nl;
            }
        }
    }
}
// A direction in the sky image, turned with the dome. The diffuse probe below samples a few of these around the
// normal: the sky's brightness comes from the whole upper hemisphere, not just the point over the surface.
vec3 env_at(vec3 world_dir, float lod) {
    vec3 d = normalize(qrot(lt.env_rot, world_dir));
    vec2 uv = vec2(atan(d.z, d.x) / (2.0 * PI) + 0.5, acos(clamp(d.y, -1.0, 1.0)) / PI);
    return textureLod(sampler2D(t_env, s_env), uv, lod).rgb;
}
vec3 env_irradiance(vec3 n) {
    vec3 t = normalize(cross(n, abs(n.y) < 0.9 ? vec3(0.0, 1.0, 0.0) : vec3(1.0, 0.0, 0.0)));
    vec3 b = normalize(cross(n, t));
    // A coarse cosine-weighted hemisphere: the normal, a 45-degree ring and a 75-degree ring, which is where a sky's
    // bright horizon sits. Harold integrates the whole hemisphere; this reads its main bands.
    vec3 sum = env_at(n, 5.0) * 0.30;
    float s45 = sin(0.785398);
    float c45 = cos(0.785398);
    float s75 = sin(1.308997);
    float c75 = cos(1.308997);
    sum += (env_at(normalize(n * c45 + t * s45), 5.0) + env_at(normalize(n * c45 - t * s45), 5.0)
          + env_at(normalize(n * c45 + b * s45), 5.0) + env_at(normalize(n * c45 - b * s45), 5.0)) * 0.125;
    sum += (env_at(normalize(n * c75 + t * s75), 5.0) + env_at(normalize(n * c75 - t * s75), 5.0)
          + env_at(normalize(n * c75 + b * s75), 5.0) + env_at(normalize(n * c75 - b * s75), 5.0)) * 0.05;
    return sum;
}
void main() {
    int flags = int(pc.misc.z);
    vec2 p = v_uv;
    if ((flags & 4) != 0) p.x = 1.0 - abs(mod(p.x, 2.0) - 1.0);
    if ((flags & 8) != 0) p.y = 1.0 - abs(mod(p.y, 2.0) - 1.0);
    vec2 q = turned(p * pc.uv.xy + pc.uv.zw, pc.misc.x);
    bool outside = ((flags & 1) == 0 && (q.x < 0.0 || q.x > 1.0)) || ((flags & 2) == 0 && (q.y < 0.0 || q.y > 1.0));
    vec2 st = vec2(q.x, 1.0 - q.y);
    vec4 texel = texture(sampler2D(t_color, s_color), st);
    vec4 mask = texture(sampler2D(t_alpha, s_alpha), st);
    vec3 base = pc.color.rgb;
    if ((flags & 16) != 0 && !outside) base = texel.rgb * pc.gain.rgb + pc.offs.rgb;
    float alpha = pc.color.a;
    if (pc.misc.w > 1.5) alpha = dot(mask.rgb, vec3(0.3, 0.59, 0.11));
    else if (pc.misc.w > 0.5) alpha = mask.a;
    vec3 emission = pc.incandescence.rgb;
    if (pc.gain.w < 0.5) {
        out_color = vec4(pow(max(base + emission, vec3(0.0)), vec3(1.0 / 2.2)), alpha);
        out_albedo = vec4(0.0, 0.0, 0.0, alpha);
        out_mat = vec4(0.0);
        out_aux = vec4(0.0);
        return;
    }
    vec3 v = lt.head.w != 0 ? vec3(0.0, 0.0, 1.0) : normalize(-v_view);
    vec3 facet = normalize(cross(dFdx(v_view), dFdy(v_view)));
    if (dot(facet, v) < 0.0) facet = -facet;
    vec3 n = normalize(v_nrm);
    bool back = dot(n, facet) < 0.0;
    if (pc.n2.w > 0.5) n = facet;
    else if (back) n = -n;
    float transmission = clamp(pc.coat.w, 0.0, 1.0);
    // A transmissive surface's colour is its transmission colour, which the material already carries (#302): the
    // refraction pass tints what it lets through with it, so it is kept as it is.
    // A mapped colour comes from the image; an unmapped one already carries the base weight in the material's colour.
    bool mapped = (flags & 16) != 0 && !outside;
    vec3 layer = mapped ? base * clamp(pc.offs.w, 0.0, 1.0) : base;
    vec3 diff, spec, coat_spec;
    direct(layer, n, v, diff, spec, coat_spec);
    float metal = clamp(pc.frame.x, 0.0, 1.0);
    vec3 diffuse_color = layer * (1.0 - metal) * (1.0 - transmission);
    vec3 lin = emission + diff * diffuse_color * INV_PI + spec;
    // Glass keeps its transmission colour for the refraction pass; a metal's own colour is already in its reflection.
    out_albedo = vec4(clamp(transmission > 0.5 ? layer : diffuse_color, 0.0, 1.0), alpha);
    // Use Default Lighting: the viewport's key and fill, when no real light is in use.
    if (lt.head.x == 0) {
        vec3 k = normalize(vec3(-0.45, 0.6, 0.65));
        vec3 f = normalize(vec3(0.6, -0.2, 0.75));
        float d = 0.75 * abs(dot(n, k)) + 0.3 * abs(dot(n, f));
        lin += layer * (1.0 - metal) * (0.12 + 0.95 * d);
    }
    // Image-based lighting's diffuse half: the sky's irradiance, a few blurred samples around the normal, so a bright
    // horizon reaches the surface as it does through Harold's path tracing.
    float rough = clamp(pc.frame.y, 0.02, 1.0);
    vec3 wn = normalize((lt.view_to_world * vec4(n, 0.0)).xyz);
    vec3 env_d = lt.env.w > 0.5 ? env_irradiance(wn) : vec3(1.0);
    env_d *= lt.env.rgb;
    if (lt.env.w > 0.5 || dot(lt.env.rgb, lt.env.rgb) > 0.0) {
        lin += env_d * diffuse_color;
    }
    // The reflection is drawn by the screen-space pass from this material (#302): the environment feeds it where the
    // view has no scene to reflect, and the scene where it has.
    float nv = max(dot(n, v), 0.001);
    float ior = max(pc.incandescence.w, 1.0);
    vec3 f0 = mix(vec3(f_ior(ior)) * pc.ambient.rgb * pc.frame.w, layer, metal);
    vec3 fres = f0 + (max(vec3(1.0 - rough), f0) - f0) * pow(1.0 - nv, 5.0);
    vec3 coeff = fres * (1.0 - 0.7 * rough);
    float look_rough = rough;
    if (pc.coat.x > 0.0) {
        float cr = max(pc.coat.y, 0.02);
        vec3 cf0 = vec3(f_ior(max(pc.coat.z, 1.0)));
        vec3 cfres = cf0 + (max(vec3(1.0 - cr), cf0) - cf0) * pow(1.0 - nv, 5.0);
        coeff += cfres * pc.coat.x;
        look_rough = min(look_rough, cr);
    }
    out_mat = vec4(coeff, look_rough + 0.002);
    out_aux = vec4(transmission, clamp((ior - 1.0) * 0.5, 0.0, 1.0), metal, 1.0);
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

// Lines smoothed by their own coverage rather than the samples: each is a band a pixel wider than the line on each side,
// widened on screen, its edge fading over a pixel. n2 carries the view's size in pixels and the line's half width.
const SMOOTH_VS: &str = r#"#version 450
layout(location = 0) in vec3 a_p0;
layout(location = 1) in vec3 a_p1;
layout(location = 2) in vec4 a_color;
layout(location = 3) in vec2 a_tv;
layout(push_constant) uniform PC { mat4 mvp; vec4 n0; vec4 n1; vec4 n2; vec4 color; } pc;
layout(location = 0) out vec4 v_color;
layout(location = 1) out float v_dist;
void main() {
    vec4 c0 = pc.mvp * vec4(a_p0, 1.0);
    vec4 c1 = pc.mvp * vec4(a_p1, 1.0);
    float near = 1e-5;
    v_color = a_color * pc.color;
    v_dist = 0.0;
    if (c0.w < near && c1.w < near) {
        gl_Position = vec4(4.0, 4.0, 0.0, 1.0);
        return;
    }
    // An end behind the camera comes forward along the line to just in front of it.
    if (c0.w < near) {
        c0 = mix(c0, c1, (near - c0.w) / (c1.w - c0.w));
    }
    if (c1.w < near) {
        c1 = mix(c1, c0, (near - c1.w) / (c0.w - c1.w));
    }
    vec2 half_vp = 0.5 * pc.n2.xy;
    vec2 s0 = c0.xy / c0.w * half_vp;
    vec2 s1 = c1.xy / c1.w * half_vp;
    vec2 d = s1 - s0;
    float len = length(d);
    vec2 dir = len > 1e-6 ? d / len : vec2(1.0, 0.0);
    vec2 nrm = vec2(-dir.y, dir.x);
    float w = pc.n2.z + 1.0;
    vec4 c = mix(c0, c1, a_tv.x);
    c.xy += nrm * (a_tv.y * w) / half_vp * c.w;
    gl_Position = c;
    gl_Position.z *= 1.0 + pc.n0.w;
    v_dist = a_tv.y * w;
}
"#;

const SMOOTH_FS: &str = r#"#version 450
layout(location = 0) in vec4 v_color;
layout(location = 1) in float v_dist;
layout(push_constant) uniform PC { mat4 mvp; vec4 n0; vec4 n1; vec4 n2; vec4 color; } pc;
layout(location = 0) out vec4 out_color;
void main() {
    float cover = clamp(pc.n2.z + 0.5 - abs(v_dist), 0.0, 1.0);
    out_color = vec4(v_color.rgb, v_color.a * cover);
}
"#;

// The far side of a transmissive body: its view-space normal, and how far along the view it sits. Glass bends a ray
// again on its way out, and the exit is the only place that second interface can be read from.
const BACKFACE_VS: &str = r#"#version 450
layout(location = 0) in vec3 a_pos;
layout(location = 1) in vec3 a_nrm;
layout(push_constant) uniform PC { mat4 mvp; vec4 n0; vec4 n1; vec4 n2; vec4 color; } pc;
layout(location = 0) out vec3 v_nrm;
layout(location = 1) out vec3 v_pos;
void main() {
    gl_Position = pc.mvp * vec4(a_pos, 1.0);
    v_nrm = vec3(dot(pc.n0.xyz, a_nrm), dot(pc.n1.xyz, a_nrm), dot(pc.n2.xyz, a_nrm));
    v_pos = vec3(dot(pc.n0.xyz, a_pos), dot(pc.n1.xyz, a_pos), dot(pc.n2.xyz, a_pos));
}
"#;

const BACKFACE_FS: &str = r#"#version 450
layout(location = 0) in vec3 v_nrm;
layout(location = 1) in vec3 v_pos;
layout(location = 0) out vec4 out_color;
void main() {
    out_color = vec4(normalize(v_nrm), -v_pos.z);
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
    /// Coloured triangles over everything: the manipulators' solid handles.
    ColorTrisOnTop,
    /// A material: its colour or file texture, with UVs in a second buffer.
    Material,
    MaterialCull,
    /// A see-through material: blended, no depth write. Drawn after everything solid.
    MaterialBlend,
    MaterialBlendCull,
    /// Lines as bands of triangles that fade at their edges: smooth at any sample count (#203). SmoothLineVertex.
    SmoothLines,
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
    /// The Realtime renderer's pipelines, and whether the device has room for their constants (256 bytes).
    pub realtime: bool,
    rt_layout: vk::PipelineLayout,
    rt_pipelines: Vec<(Kind, vk::Pipeline)>,
    /// The Realtime renderer's pipelines for the ground plane's reflection pass: the picture alone (#302).
    rt_planar_pipelines: Vec<(Kind, vk::Pipeline)>,
    /// The Realtime shadow set: a 2D map, a cube map, and a sampler for each.
    pub rt_shadow_layout: vk::DescriptorSetLayout,
    /// The far side of the transmissive bodies: back faces, their view normal and view distance.
    backface_pipeline: vk::Pipeline,
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
            for kind in [Kind::Shaded, Kind::Tint, Kind::Lines, Kind::LinesOnTop, Kind::Points, Kind::PointsOnTop, Kind::ShadedCull, Kind::ColorTris, Kind::ColorTrisOnTop] {
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
                    Kind::Shaded | Kind::Tint | Kind::ShadedCull | Kind::ColorTris | Kind::ColorTrisOnTop => vk::PrimitiveTopology::TRIANGLE_LIST,
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
                // With anti-aliasing on, lines are smoothed too: wireframes, curves, the grid and the manipulators.
                // Rectangles the samples cover in part where the device has them: the pieces of a line (the grid's run
                // cell by cell) meet with nothing doubled or missed. A device's own smoothed edges fade each piece's
                // ends, which drew the grid dashed (#198), so they are only used when it has no rectangles.
                let (rect, smooth) = gpu.shared.line_modes;
                let mut line_state = vk::PipelineRasterizationLineStateCreateInfoKHR::default().line_rasterization_mode(if rect {
                    vk::LineRasterizationModeKHR::RECTANGULAR
                } else {
                    vk::LineRasterizationModeKHR::RECTANGULAR_SMOOTH
                });
                let rs = if matches!(kind, Kind::Lines | Kind::LinesOnTop) && gpu.samples != vk::SampleCountFlags::TYPE_1 && (rect || smooth) { rs.push_next(&mut line_state) } else { rs };
                let ms = vk::PipelineMultisampleStateCreateInfo::default().rasterization_samples(gpu.samples);
                let on_top = matches!(kind, Kind::LinesOnTop | Kind::PointsOnTop | Kind::ColorTrisOnTop);
                let ds = vk::PipelineDepthStencilStateCreateInfo::default()
                    .depth_test_enable(!on_top)
                    .depth_write_enable(matches!(kind, Kind::Shaded | Kind::ShadedCull | Kind::Lines | Kind::ColorTris))
                    .depth_compare_op(vk::CompareOp::GREATER_OR_EQUAL);
                let blend = scene_blend(false, false);
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
                    let blend = scene_blend(false, false);
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
            let rt_realtime = materials && limits.max_push_constants_size as usize >= std::mem::size_of::<RtPush>();
            let mut rt_layout = vk::PipelineLayout::null();
            let mut rt_pipelines = Vec::new();
            let mut rt_planar_pipelines = Vec::new();
            let mut rt_shadow_layout = vk::DescriptorSetLayout::null();
            if rt_realtime {
                let tex = crate::textures::set_layout(gpu);
                let shadow_bindings = [
                    vk::DescriptorSetLayoutBinding::default().binding(0).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT),
                    vk::DescriptorSetLayoutBinding::default().binding(1).descriptor_type(vk::DescriptorType::SAMPLER).descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT),
                    vk::DescriptorSetLayoutBinding::default().binding(2).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT),
                    vk::DescriptorSetLayoutBinding::default().binding(3).descriptor_type(vk::DescriptorType::SAMPLER).descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT),
                ];
                rt_shadow_layout = d.create_descriptor_set_layout(&vk::DescriptorSetLayoutCreateInfo::default().bindings(&shadow_bindings), None).unwrap();
                let sets = [tex, tex, light_layout, tex, rt_shadow_layout];
                let range = [vk::PushConstantRange::default()
                    .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT)
                    .size(std::mem::size_of::<RtPush>() as u32)];
                rt_layout = d.create_pipeline_layout(&vk::PipelineLayoutCreateInfo::default().set_layouts(&sets).push_constant_ranges(&range), None).unwrap();
                let vs = module(REALTIME_VS, naga::ShaderStage::Vertex);
                let fs = module(REALTIME_FS, naga::ShaderStage::Fragment);
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
                    let blend = scene_blend(true, true);
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
                        .layout(rt_layout)
                        .render_pass(gpu.render_pass)
                        .subpass(0);
                    let p = d.create_graphics_pipelines(vk::PipelineCache::null(), &[info], None).map_err(|e| e.1).unwrap()[0];
                    rt_pipelines.push((kind, p));
                    // The same draw for the ground plane's reflection pass: the picture alone, no material targets.
                    if matches!(kind, Kind::Material | Kind::MaterialCull) {
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
                        let ms = vk::PipelineMultisampleStateCreateInfo::default().rasterization_samples(vk::SampleCountFlags::TYPE_1);
                        // The mirror flips the winding: both sides are drawn, the shader turns the normal to the eye.
                        let rs = vk::PipelineRasterizationStateCreateInfo::default()
                            .polygon_mode(vk::PolygonMode::FILL)
                            .cull_mode(vk::CullModeFlags::NONE)
                            .front_face(vk::FrontFace::CLOCKWISE)
                            .line_width(1.0);
                        let info = info.color_blend_state(&cb).multisample_state(&ms).rasterization_state(&rs).render_pass(gpu.planar_pass);
                        let p = d.create_graphics_pipelines(vk::PipelineCache::null(), &[info], None).map_err(|e| e.1).unwrap()[0];
                        rt_planar_pipelines.push((kind, p));
                    }
                }
                d.destroy_shader_module(vs, None);
                d.destroy_shader_module(fs, None);
            }
            let backface_pipeline = {
                let vs = module(BACKFACE_VS, naga::ShaderStage::Vertex);
                let fs = module(BACKFACE_FS, naga::ShaderStage::Fragment);
                let stages = [
                    vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::VERTEX).module(vs).name(c"main"),
                    vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::FRAGMENT).module(fs).name(c"main"),
                ];
                let binding = [vk::VertexInputBindingDescription::default().stride(24).input_rate(vk::VertexInputRate::VERTEX)];
                let attrs = [
                    vk::VertexInputAttributeDescription::default().location(0).format(vk::Format::R32G32B32_SFLOAT).offset(0),
                    vk::VertexInputAttributeDescription::default().location(1).format(vk::Format::R32G32B32_SFLOAT).offset(12),
                ];
                let vin = vk::PipelineVertexInputStateCreateInfo::default().vertex_binding_descriptions(&binding).vertex_attribute_descriptions(&attrs);
                let ia = vk::PipelineInputAssemblyStateCreateInfo::default().topology(vk::PrimitiveTopology::TRIANGLE_LIST);
                let vp = vk::PipelineViewportStateCreateInfo::default().viewport_count(1).scissor_count(1);
                let rs = vk::PipelineRasterizationStateCreateInfo::default()
                    .polygon_mode(vk::PolygonMode::FILL)
                    .cull_mode(vk::CullModeFlags::FRONT)
                    .front_face(vk::FrontFace::CLOCKWISE)
                    .line_width(1.0);
                let ms = vk::PipelineMultisampleStateCreateInfo::default().rasterization_samples(vk::SampleCountFlags::TYPE_1);
                let ds = vk::PipelineDepthStencilStateCreateInfo::default().depth_test_enable(true).depth_write_enable(true).depth_compare_op(vk::CompareOp::GREATER_OR_EQUAL);
                let blend = [vk::PipelineColorBlendAttachmentState::default().blend_enable(false).color_write_mask(vk::ColorComponentFlags::RGBA)];
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
                    .render_pass(gpu.planar_pass)
                    .subpass(0);
                let p = d.create_graphics_pipelines(vk::PipelineCache::null(), &[info], None).map_err(|e| e.1).unwrap()[0];
                d.destroy_shader_module(vs, None);
                d.destroy_shader_module(fs, None);
                p
            };
            {
                let vs = module(SMOOTH_VS, naga::ShaderStage::Vertex);
                let fs = module(SMOOTH_FS, naga::ShaderStage::Fragment);
                let stages = [
                    vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::VERTEX).module(vs).name(c"main"),
                    vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::FRAGMENT).module(fs).name(c"main"),
                ];
                let attrs = [
                    vk::VertexInputAttributeDescription::default().location(0).format(vk::Format::R32G32B32_SFLOAT).offset(0),
                    vk::VertexInputAttributeDescription::default().location(1).format(vk::Format::R32G32B32_SFLOAT).offset(12),
                    vk::VertexInputAttributeDescription::default().location(2).format(vk::Format::R32G32B32A32_SFLOAT).offset(24),
                    vk::VertexInputAttributeDescription::default().location(3).format(vk::Format::R32G32_SFLOAT).offset(40),
                ];
                let binding = [vk::VertexInputBindingDescription::default().stride(std::mem::size_of::<SmoothLineVertex>() as u32).input_rate(vk::VertexInputRate::VERTEX)];
                let vin = vk::PipelineVertexInputStateCreateInfo::default().vertex_binding_descriptions(&binding).vertex_attribute_descriptions(&attrs);
                let ia = vk::PipelineInputAssemblyStateCreateInfo::default().topology(vk::PrimitiveTopology::TRIANGLE_LIST);
                let vp = vk::PipelineViewportStateCreateInfo::default().viewport_count(1).scissor_count(1);
                let rs = vk::PipelineRasterizationStateCreateInfo::default()
                    .polygon_mode(vk::PolygonMode::FILL)
                    .cull_mode(vk::CullModeFlags::NONE)
                    .front_face(vk::FrontFace::CLOCKWISE)
                    .line_width(1.0);
                let ms = vk::PipelineMultisampleStateCreateInfo::default().rasterization_samples(gpu.samples);
                // Tested against what's drawn, written to nothing: the faded edges stay see-through.
                let ds = vk::PipelineDepthStencilStateCreateInfo::default()
                    .depth_test_enable(true)
                    .depth_write_enable(false)
                    .depth_compare_op(vk::CompareOp::GREATER_OR_EQUAL);
                let blend = scene_blend(false, false);
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
                let p = d.create_graphics_pipelines(vk::PipelineCache::null(), &[info], None).map_err(|e| e.1).unwrap()[0];
                pipelines.push((Kind::SmoothLines, p));
                d.destroy_shader_module(vs, None);
                d.destroy_shader_module(fs, None);
            }
            Renderer { layout, pipelines, line_width: feats.wide_lines != 0, materials, material_layout, light_layout, realtime: rt_realtime, rt_layout, rt_pipelines, rt_planar_pipelines, rt_shadow_layout, backface_pipeline }
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

    /// Binds a Realtime material pipeline: its colour and alpha textures, the lights, the sky dome's image and the
    /// shadow map.
    pub fn bind_realtime(&self, gpu: &Gpu, kind: Kind, color: vk::DescriptorSet, alpha: vk::DescriptorSet, lights: vk::DescriptorSet, env: vk::DescriptorSet, shadow: vk::DescriptorSet) {
        let p = self.rt_pipelines.iter().find(|(k, _)| *k == kind).unwrap().1;
        unsafe {
            gpu.device.cmd_bind_pipeline(gpu.cmd, vk::PipelineBindPoint::GRAPHICS, p);
            gpu.device.cmd_bind_descriptor_sets(gpu.cmd, vk::PipelineBindPoint::GRAPHICS, self.rt_layout, 0, &[color, alpha, lights, env, shadow], &[]);
        }
    }

    /// Binds a Realtime material for the ground plane's reflection pass (#302).
    pub fn bind_realtime_planar(&self, gpu: &Gpu, kind: Kind, color: vk::DescriptorSet, alpha: vk::DescriptorSet, lights: vk::DescriptorSet, env: vk::DescriptorSet, shadow: vk::DescriptorSet) {
        let p = self.rt_planar_pipelines.iter().find(|(k, _)| *k == kind).unwrap().1;
        unsafe {
            gpu.device.cmd_bind_pipeline(gpu.cmd, vk::PipelineBindPoint::GRAPHICS, p);
            gpu.device.cmd_bind_descriptor_sets(gpu.cmd, vk::PipelineBindPoint::GRAPHICS, self.rt_layout, 0, &[color, alpha, lights, env, shadow], &[]);
        }
    }

    /// Binds the pass that records the transmissive bodies' far side.
    pub fn bind_backface(&self, gpu: &Gpu) {
        unsafe { gpu.device.cmd_bind_pipeline(gpu.cmd, vk::PipelineBindPoint::GRAPHICS, self.backface_pipeline) };
    }

    pub fn push_realtime(&self, gpu: &Gpu, p: &RtPush) {
        unsafe {
            gpu.device.cmd_push_constants(gpu.cmd, self.rt_layout, vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT, 0, bytemuck::bytes_of(p));
        }
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
        for (src, stage) in [
            (SHADED_VS, naga::ShaderStage::Vertex),
            (SHADED_FS, naga::ShaderStage::Fragment),
            (LINE_VS, naga::ShaderStage::Vertex),
            (LINE_FS, naga::ShaderStage::Fragment),
            (MATERIAL_VS, naga::ShaderStage::Vertex),
            (MATERIAL_FS, naga::ShaderStage::Fragment),
            (REALTIME_VS, naga::ShaderStage::Vertex),
            (REALTIME_FS, naga::ShaderStage::Fragment),
            (SMOOTH_VS, naga::ShaderStage::Vertex),
            (SMOOTH_FS, naga::ShaderStage::Fragment),
            (BACKFACE_VS, naga::ShaderStage::Vertex),
            (BACKFACE_FS, naga::ShaderStage::Fragment),
            (crate::realtime::SHADOW_VS, naga::ShaderStage::Vertex),
            (crate::realtime::SHADOW_FS, naga::ShaderStage::Fragment),
        ] {
            assert!(!compile(src, stage).is_empty());
        }
    }

    #[test]
    fn the_light_block_is_what_the_shader_declares() {
        use crate::lighting::{GpuLight, LightBlock, MAX_LIGHTS};
        assert_eq!(std::mem::size_of::<GpuLight>(), 7 * 16);
        // inv_proj, head, shadow_matrix, shadow, shadow_pos, shadow_range, env, env_rot, view_to_world, then lights.
        assert_eq!(std::mem::size_of::<LightBlock>(), 64 + 16 + 64 + 16 + 16 + 16 + 16 + 16 + 64 + 7 * 16 * MAX_LIGHTS);
        assert!(MATERIAL_FS.contains("Light lights[16]") && REALTIME_FS.contains("Light lights[16]") && MAX_LIGHTS == 16);
    }

    /// The cube shadow samples world-axis directions, and each face's depth image holds the distance along its own
    /// axis. Every value that is compared is turned back into the distance from the light, which is one number across
    /// all six faces; the receiver's axis distance is not compared with a tap's.
    #[test]
    fn the_cube_shadow_measures_the_distance_in_the_maps_own_space() {
        assert!(REALTIME_FS.contains("vec3 wto = (lt.view_to_world * vec4(to, 0.0)).xyz;"));
        assert!(REALTIME_FS.contains("float radial_at(float face_distance, vec3 dir)"));
        assert!(REALTIME_FS.contains("float stored = radial_at(cube_distance(dir, near, far), dir);"));
        assert!(REALTIME_FS.contains("lit += smoothstep(-tol, tol, stored + tol - reach);"));
        assert!(!REALTIME_FS.contains("axis <= stored"));
        // The offset and the compared tolerance are separate quantities: a texel-sized world offset along the normal,
        // and a depth tolerance that grows with the slope. Neither carries a constant floor.
        assert!(REALTIME_FS.contains("vec3 q3 = p + n * (texel_world * (sin_theta + 0.05));"));
        assert!(REALTIME_FS.contains("float tol = texel_world * (1.0 + 2.0 * tan_theta);"));
        assert!(!REALTIME_FS.contains("0.02 +"));
        assert!(!REALTIME_FS.contains("q.z - 0.0015"));
        // An area light's disc carries thirty-two taps, and the single map's border fades back to lit.
        assert!(REALTIME_FS.contains("for (int i = 0; i < 32; ++i)"));
        assert!(REALTIME_FS.contains("float border = min(min(q.x, 1.0 - q.x), min(q.y, 1.0 - q.y)) / texel;"));
    }

    /// A lit receiver must stay lit when the tap that reads its caster lies in another cube face: the receiver's axis
    /// distance and the tap's axis distance are measured along different axes, and comparing them mixes metrics.
    #[test]
    fn the_cube_shadow_compares_one_metric_across_its_faces() {
        let axis_of = |d: glam::DVec3| d.x.abs().max(d.y.abs()).max(d.z.abs());
        let radial = |face_distance: f64, d: glam::DVec3| face_distance / axis_of(d);
        // A receiver at 10 units nearly on +X, and a caster at 11 units read through a tap near the x/z boundary, in
        // the +Z face.
        let receiver = glam::DVec3::new(1.0, 0.0, 0.1).normalize();
        let tap = glam::DVec3::new(0.6, 0.0, 0.8).normalize();
        let (reach, caster) = (10.0, 11.0);
        let stored_face = caster * axis_of(tap);
        assert!((radial(stored_face, tap) - caster).abs() < 1e-9);
        // Axis against axis shadows the lit receiver; distance against distance does not.
        assert!(reach * axis_of(receiver) > stored_face + 1e-6);
        assert!(reach <= radial(stored_face, tap) + 1e-6);
    }

    /// A broad emitter's highlight is broad: the lobe carries the emitter's angular radius, widened in quadrature
    /// with the surface's roughness. Without it the nearest point acts as a point light of the whole power and the
    /// floor under an area light clips to a plate.
    #[test]
    fn an_area_lights_highlight_widens_with_the_emitter() {
        assert!(REALTIME_FS.contains("area_angle = 0.5 * (length(ua.xyz) + length(ub.xyz)) / max(d, 0.0001);"));
        assert!(REALTIME_FS.contains("float look_rough = min(sqrt(rough * rough + area_angle * area_angle), 1.0);"));
        assert!(REALTIME_FS.contains("float d = distribution_ggx(nh, look_rough);"));
        assert!(REALTIME_FS.contains("float g = geometry_smith(nv, nl, look_rough);"));
        assert!(!REALTIME_FS.contains("distribution_ggx(nh, rough)"));
    }

    #[test]
    fn the_realtime_constants_fit_a_device() {
        // 256 bytes is what the Realtime pipelines ask for: 128 for the draw's, then one vec4 a layer parameter group.
        assert_eq!(std::mem::size_of::<RtPush>(), 256);
        assert_eq!(std::mem::size_of::<MatPush>(), 240);
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
            // An area light's scale when it isn't normalised comes in par.y, as viewport_diffuse's half sides.
            "c *= decay(dir.w, d) * max(dot(-l, dir.xyz), 0.0) * par.y;",
            "float fade = clamp(1.0 - reach, 0.0, 1.0);",
            "diff += c * nl;",
        ] {
            assert!(MATERIAL_FS.contains(line), "the viewport's lighting changed: {line}");
        }
    }
}

#[cfg(test)]
mod smooth_line_tests {
    use super::*;

    #[test]
    fn each_line_becomes_a_band_of_two_triangles() {
        // #203: the grid's lines are drawn as bands, both sides of each end.
        let a = LineVertex { pos: [0.0, 0.0, 0.0], color: [1.0, 0.0, 0.0, 1.0] };
        let b = LineVertex { pos: [1.0, 0.0, 0.0], color: [0.0, 1.0, 0.0, 1.0] };
        let band = smooth_lines(&[a, b]);
        assert_eq!(band.len(), 6);
        for v in &band {
            assert_eq!((v.p0, v.p1), (a.pos, b.pos));
            assert_eq!(v.color, if v.tv[0] < 0.5 { a.color } else { b.color });
        }
        let corners: std::collections::BTreeSet<(i32, i32)> = band.iter().map(|v| (v.tv[0] as i32, v.tv[1] as i32)).collect();
        assert_eq!(corners.len(), 4);
        assert_eq!(std::mem::size_of::<SmoothLineVertex>(), 48);
    }
}
