//! What happens to a view's picture between the scene being drawn and the window showing it: ambient occlusion and
//! bounce light from the depth and colour the scene left. One full-screen pass works those out; another reads them with
//! the scene's colour and writes the window's image, with the camera's depth of field when the view asks for it. The
//! view's guides (gates, heads up display, ViewCube) are drawn on a layer of their own and laid over that last (#300).

use ash::vk;
use glam::Mat4;

use crate::gpu::Gpu;
use crate::render::compile;

const POST_VS: &str = r#"#version 450
layout(location = 0) out vec2 uv;
void main() {
    // One triangle over the whole view.
    vec2 p = vec2(float((gl_VertexIndex << 1) & 2), float(gl_VertexIndex & 2));
    uv = p;
    gl_Position = vec4(p * 2.0 - 1.0, 0.0, 1.0);
}
"#;

/// The parts both fragment shaders share. DEPTH_TEXTURE and DEPTH_FETCH are filled in for a multisampled depth or a
/// plain one.
const POST_HEAD: &str = r#"#version 450
layout(location = 0) in vec2 uv;
layout(location = 0) out vec4 o;
layout(set = 0, binding = 0) uniform texture2D t_scene;
layout(set = 0, binding = 1) uniform DEPTH_TEXTURE t_depth;
layout(set = 0, binding = 2) uniform sampler s_point;
layout(set = 0, binding = 3) uniform texture2D t_open;
layout(set = 0, binding = 5) uniform texture2D t_albedo;
// The view's guides, drawn after the scene onto a clear layer, premultiplied by their alpha (#300).
layout(set = 0, binding = 6) uniform texture2D t_overlay;
layout(push_constant) uniform P {
    // Clip space back to the camera's.
    mat4 inv_proj;
    // Reach in scene units, strength, on (1) or off (0), orthographic (1) or not.
    vec4 ao;
    // Width, height, and one over each.
    vec4 size;
    // The background's gradient, top and bottom colours; top.w is 1 with the gradient on, bottom.w the flat grey the
    // scene was cleared to. The occlusion passes have no background: there top.x is 1 in the bounce light's second.
    vec4 bg_top;
    vec4 bg_bottom;
} pc;
// The camera's projection and the contact shadow, in a uniform buffer: the push block stays what every device allows.
layout(set = 0, binding = 4, std140) uniform Params {
    mat4 proj;
    // The direction in view space towards the shadow-casting light; w is how far its contact shadow reaches.
    vec4 contact_dir;
    // Contact shadow strength, and whether it is on (0 off).
    vec4 contact;
    // x: 1 when the Realtime renderer asks for the finer occlusion (more taps, a wider blur). y: how strong the bounce
    // light is, 0 with it off.
    vec4 quality;
    // Depth of field. x: the lens's radius in pixels at unit distance (blur = x * |1/distance - y|); y: one over the
    // distance in focus; z: the widest blur in pixels, 0 with it off; w: 1 when only shaded surfaces blur (the
    // Realtime renderer marks them in the albedo's alpha).
    vec4 dof;
} pp;

float depth_at(vec2 px) {
    ivec2 at = clamp(ivec2(px), ivec2(0), ivec2(pc.size.xy) - 1);
    return DEPTH_FETCH;
}

// The camera-space point a pixel shows, from its depth. Depth runs 1 at the eye to 0 far away.
vec3 point_at(vec2 px, float d) {
    vec4 p = pc.inv_proj * vec4(px * pc.size.zw * 2.0 - 1.0, d, 1.0);
    return p.xyz / p.w;
}

// The surface's normal from its neighbours, taking on each axis the side nearer in depth so an edge doesn't bend it.
// It faces the eye.
vec3 normal_at(vec2 c, float d, vec3 p) {
    float dl = depth_at(c - vec2(1.0, 0.0)), dr = depth_at(c + vec2(1.0, 0.0));
    float du = depth_at(c - vec2(0.0, 1.0)), dd = depth_at(c + vec2(0.0, 1.0));
    vec3 ax = abs(dr - d) < abs(d - dl) ? point_at(c + vec2(1.0, 0.0), dr) - p : p - point_at(c - vec2(1.0, 0.0), dl);
    vec3 ay = abs(dd - d) < abs(d - du) ? point_at(c + vec2(0.0, 1.0), dd) - p : p - point_at(c - vec2(0.0, 1.0), du);
    vec3 n = normalize(cross(ax, ay));
    vec3 to_eye = pc.ao.w > 0.5 ? vec3(0.0, 0.0, 1.0) : normalize(-p);
    return dot(n, to_eye) < 0.0 ? -n : n;
}
"#;

/// How open each pixel is (alpha): 1 with nothing near it, less in creases and where things meet. And the light that
/// reaches it bounced off what the view shows round about (rgb), before the surface's own colour takes its share.
const AO_MAIN: &str = r#"
// How far a camera-space point lies behind the shaded surface the view shows at its pixel: negative in front of it.
// Off the view it is -1e6; where no shaded surface shows, 1e6.
float behind_at(vec3 at) {
    vec4 clip = pp.proj * vec4(at, 1.0);
    if (clip.w <= 0.0) return -1e6;
    vec2 sx = clip.xy / clip.w * 0.5 + 0.5;
    if (sx.x < 0.0 || sx.x > 1.0 || sx.y < 0.0 || sx.y > 1.0) return -1e6;
    vec2 q = floor(sx * pc.size.xy) + 0.5;
    float dq = depth_at(q);
    // Only shaded surfaces send light back: the grid, icons and wireframes leave no surface colour.
    if (dq <= 0.0 || texelFetch(sampler2D(t_albedo, s_point), ivec2(q), 0).a <= 0.0) return 1e6;
    return point_at(q, dq).z - at.z;
}

// The pixel round about `q` that the surface covers most. A surface's edge pixel is part background in the scene's
// colour, and the background is no light (#301): the bounce takes the colour from a pixel the surface fills instead.
ivec2 covered_near(ivec2 q) {
    ivec2 best = q;
    float most = texelFetch(sampler2D(t_albedo, s_point), q, 0).a;
    for (int y = -1; y <= 1; ++y) {
        for (int x = -1; x <= 1; ++x) {
            ivec2 at = clamp(q + ivec2(x, y), ivec2(0), ivec2(pc.size.xy) - 1);
            float a = texelFetch(sampler2D(t_albedo, s_point), at, 0).a;
            if (a > most + 0.01 && depth_at(vec2(at) + 0.5) > 0.0) {
                most = a;
                best = at;
            }
        }
    }
    return best;
}

// Noise that differs from pixel to pixel with no pattern the eye picks out.
float noise(vec2 px) {
    return fract(52.9829189 * fract(dot(px, vec2(0.06711056, 0.00583715))));
}

void main() {
    vec2 c = floor(gl_FragCoord.xy) + 0.5;
    float d = depth_at(c);
    if (d <= 0.0) {
        o = vec4(0.0, 0.0, 0.0, 1.0);
        return;
    }
    vec3 p = point_at(c, d);
    vec3 n = normal_at(c, d, p);
    // The bounce light's second pass keeps the first's occlusion and sends its bounce round once more.
    bool second = pc.bg_top.x > 0.5;

    float reach = pc.ao.x;
    // How far that is on screen at this depth, kept to a size worth sampling.
    float per_px = length(point_at(c + vec2(1.0, 0.0), d) - p);
    float reach_px = clamp(reach / max(per_px, 1e-9), 3.0, 64.0);
    // On screen the reach is held to 64 pixels: close up, what is sampled is nearer than the reach asked for.
    float r = min(reach, reach_px * per_px);
    float turn = noise(c) * 6.2831853;
    float sum = 0.0;
    // Realtime takes twice the taps: a smoother term and less of the sampling's grain.
    int taps = pp.quality.x > 0.5 ? 32 : 16;
    for (int i = 0; i < 32; ++i) {
        if (i >= taps) break;
        // A spiral out from the pixel.
        float t = (float(i) + 0.5) / float(taps);
        float a = turn + t * 6.2831853 * 7.0;
        vec2 q = floor(c + vec2(cos(a), sin(a)) * reach_px * t) + 0.5;
        float dq = depth_at(q);
        if (dq <= 0.0) continue;
        vec3 v = point_at(q, dq) - p;
        float vv = dot(v, v);
        // A flat-faced mesh's neighbouring faces sit a little proud of each other: that much is not occlusion.
        float vn = dot(v, n) - 0.06 * r;
        // What is past the reach hides nothing: something far in front (a handle, another object) casts no halo.
        float f = max(1.0 - vv / (r * r), 0.0);
        sum += f * f * f * max(vn, 0.0) * r / (vv + 0.0001 * r * r);
    }
    // Contact shadows: march from the point towards the light and see whether the surface stands in the way. This is the
    // short-range shadow a shadow map's texels are too coarse for, as EEVEE's contact shadows are.
    float contact = 1.0;
    if (pp.contact.x > 0.0 && pp.contact_dir.w > 0.0) {
        vec3 dir = normalize(pp.contact_dir.xyz);
        float far = pp.contact_dir.w;
        float step_len = far / 16.0;
        float t = step_len;
        for (int i = 0; i < 16; ++i) {
            vec4 clip = pp.proj * vec4(p + dir * t, 1.0);
            if (clip.w <= 0.0) break;
            vec2 sx = clip.xy / clip.w * 0.5 + 0.5;
            if (sx.x < 0.0 || sx.x > 1.0 || sx.y < 0.0 || sx.y > 1.0) break;
            vec2 q = floor(sx * pc.size.xy) + 0.5;
            float dq = depth_at(q);
            if (dq > 0.0) {
                float zq = point_at(q, dq).z;
                float zr = (p + dir * t).z;
                if (zq - zr > far * 0.02 && zq - zr < far * 0.75) {
                    contact = 1.0 - pp.contact.x;
                    break;
                }
            }
            t += step_len;
        }
    }
    float open = pc.ao.z > 0.5 ? clamp(1.0 - pc.ao.y * sum / float(taps), 0.0, 1.0) : 1.0;
    if (second) {
        open = texelFetch(sampler2D(t_open, s_point), ivec2(c), 0).a;
        contact = 1.0;
    }
    // Bounce light, as screen-space global illumination does it: rays leave the point over the hemisphere round its
    // normal, more of them near the normal as the cosine has it, and march across what the view shows until one passes
    // behind a surface; that surface's lit colour is the light it sends back. Their mean is the radiance arriving here,
    // weighted by the cosine, so the surface's colour times it is the light it bounces: rays that leave the view bring
    // none. Only what the view shows gives light, so it costs the same however the lights move.
    vec3 gi = vec3(0.0);
    if (pp.quality.y > 0.0 && texelFetch(sampler2D(t_albedo, s_point), ivec2(c), 0).a > 0.0) {
        vec3 tn = normalize(cross(n, abs(n.y) < 0.9 ? vec3(0.0, 1.0, 0.0) : vec3(1.0, 0.0, 0.0)));
        vec3 bn = cross(n, tn);
        // How far a ray goes: as far as the point is from the eye, which spans what the view shows.
        float reach = pc.ao.w > 0.5 ? 2.0 * pc.ao.x * 8.0 : length(p);
        float jitter = noise(c.yx + vec2(17.0, 5.0));
        // Rays leave a little off the surface, so those that skim along it don't meet it again in its own depth.
        vec3 from = p + n * (0.01 * reach);
        const int RAYS = 12;
        vec3 hit_albedo = vec3(0.0);
        float hits = 0.0, gone = 0.0;
        const int STEPS = 16;
        for (int i = 0; i < RAYS; ++i) {
            float u1 = fract((float(i) + jitter) / float(RAYS));
            float u2 = fract(float(i) * 0.618034 + noise(c + vec2(float(i) * 7.0, 3.0)));
            float phi = 6.2831853 * u1;
            float r = sqrt(u2);
            vec3 dir = tn * (r * cos(phi)) + bn * (r * sin(phi)) + n * sqrt(max(1.0 - u2, 0.0));
            float step_jit = noise(c + vec2(3.0, float(i) * 11.0));
            float t_last = 0.0;
            bool was_behind = false;
            for (int k = 0; k < STEPS; ++k) {
                // Steps grow along the ray: fine near the point, where what stands close matters most.
                float f = (float(k) + step_jit) / float(STEPS);
                float t = reach * f * f + 0.01 * reach;
                float gap = behind_at(from + dir * t);
                if (gap < -1e5) {
                    gone += 1.0;
                    break;
                }
                if (gap > 1e5) continue;
                bool behind = gap > 0.0;
                if (behind && k == 0) break;
                if (behind && !was_behind) {
                    // It went behind the surface the view shows there since the last step. Narrow down where: when the
                    // gap closes, the ray met that surface, however edge-on it is to the eye; when it stays open, the
                    // ray only passed behind something standing in front, and goes on.
                    float lo = t_last, hi = t;
                    for (int b = 0; b < 5; ++b) {
                        float mid = 0.5 * (lo + hi);
                        if (behind_at(from + dir * mid) > 0.0) hi = mid; else lo = mid;
                    }
                    vec3 at = from + dir * hi;
                    if (behind_at(at) < 0.05 * hi + 0.02 * reach) {
                        vec4 clip = pp.proj * vec4(at, 1.0);
                        ivec2 q = covered_near(ivec2(clip.xy / clip.w * 0.5 * pc.size.xy + 0.5 * pc.size.xy));
                        vec3 hit_colour = texelFetch(sampler2D(t_albedo, s_point), q, 0).rgb;
                        gi += pow(texelFetch(sampler2D(t_scene, s_point), q, 0).rgb, vec3(2.2));
                        // The second time round, the surface met also sends back what the first bounce brought it.
                        if (second) gi += hit_colour * texelFetch(sampler2D(t_open, s_point), q, 0).rgb;
                        hit_albedo += hit_colour;
                        hits += 1.0;
                        break;
                    }
                }
                was_behind = behind;
                t_last = t;
            }
        }
        // Light keeps bouncing: what came back goes out again off the surfaces the rays met, which send back the same
        // share again, and so on. The first pass saw one bounce; the series of the rest is that over one less the share
        // each bounce keeps, the hits' surface colour times how much of the view round about they cover. The second
        // pass met those surfaces with that on them already.
        // Rays that leave the view meet what it doesn't show, which may be more of the room or open space: taken to be
        // half as bright as what the others met. Rays that cross open space that the view shows bring nothing.
        if (hits > 0.5) {
            gi += gi / hits * gone * 0.5;
            hit_albedo += hit_albedo / hits * gone * 0.5;
        }
        vec3 keep = second ? vec3(0.0) : min(hit_albedo / float(RAYS), vec3(0.9));
        gi *= pp.quality.y / float(RAYS) / (vec3(1.0) - keep);
    }
    o = vec4(gi, clamp(open * contact, 0.0, 1.0));
}
"#;

/// The picture: the scene's colour, darkened by how closed in each pixel is, with the bounce light the surface's colour
/// sends back. Both are smoothed over the pixels round about that lie on the same surface, which takes the sampling's
/// grain out and keeps edges sharp.
const COMPOSITE_MAIN: &str = r#"
// Keep the displayed scene in range while the bounce pass reads its full brightness.
vec3 display_color(ivec2 at) {
    return clamp(texelFetch(sampler2D(t_scene, s_point), at, 0).rgb, 0.0, 1.0);
}

// The view's background where nothing was drawn: no depth, and still the grey it was cleared to.
bool cleared(vec3 color, float d) {
    return d <= 0.0 && all(lessThan(abs(color - vec3(pc.bg_bottom.w)), vec3(0.5 / 255.0)));
}

bool effects_off() {
    return pc.ao.z < 0.5 && pp.contact.x <= 0.0 && pp.quality.y <= 0.0;
}

// The scene's colour with the occlusion and bounce light (`got`, rgb bounce and alpha openness) on it.
vec3 lit(vec3 color, vec4 got, ivec2 at) {
    float open = got.a;
    vec3 shaded = color * open;
    if (pp.quality.y > 0.0) {
        // The bounce is added in linear light, then the picture goes back to the display's encoding.
        vec3 albedo = texelFetch(sampler2D(t_albedo, s_point), at, 0).rgb;
        shaded = pow(pow(shaded, vec3(2.2)) + albedo * got.rgb * open, vec3(1.0 / 2.2));
    }
    return shaded;
}

// The picture at a pixel centre.
vec3 picture(vec2 c) {
    vec3 color = display_color(ivec2(c));
    float d = depth_at(c);
    // Nothing drawn here: the gradient, top of the view to bottom. What was drawn without depth keeps its colour.
    if (pc.bg_top.w > 0.5 && cleared(color, d)) return mix(pc.bg_top.rgb, pc.bg_bottom.rgb, c.y * pc.size.w);
    if (effects_off() || d <= 0.0) return color;
    float z = point_at(c, d).z;
    float per_px = length(point_at(c + vec2(1.0, 0.0), d) - point_at(c, d));
    vec4 sum = vec4(0.0);
    float weight = 0.0;
    // Realtime smooths over a wider square, which takes the finer sampling's remaining grain out.
    int blur = pp.quality.x > 0.5 ? 4 : 3;
    for (int y = -blur; y <= blur; ++y) {
        for (int x = -blur; x <= blur; ++x) {
            vec2 q = c + vec2(float(x), float(y));
            ivec2 at = clamp(ivec2(q), ivec2(0), ivec2(pc.size.xy) - 1);
            float dq = depth_at(q);
            if (dq <= 0.0) continue;
            // Pixels much nearer or farther are another surface: a slope across the square is not.
            float dz = abs(point_at(q, dq).z - z) / (per_px * float(blur * 2) + 1e-9);
            float w = exp(-float(x * x + y * y) / float(blur * blur * 2)) * max(1.0 - dz, 0.0);
            sum += texelFetch(sampler2D(t_open, s_point), at, 0) * w;
            weight += w;
        }
    }
    return lit(color, weight > 0.0 ? sum / weight : vec4(0.0, 0.0, 0.0, 1.0), ivec2(c));
}

// The picture at a pixel the depth of field gathers from: the same, without the occlusion's smoothing, which the
// gathering itself stands in for.
vec3 picture_rough(ivec2 at, vec3 color, float d) {
    if (pc.bg_top.w > 0.5 && cleared(color, d)) return mix(pc.bg_top.rgb, pc.bg_bottom.rgb, (float(at.y) + 0.5) * pc.size.w);
    if (effects_off() || d <= 0.0) return color;
    return lit(color, texelFetch(sampler2D(t_open, s_point), at, 0), at);
}

// What depth of field blurs: the background, and what has depth (only shaded surfaces when the renderer marks them).
// The grid, wireframes, icons and handles drawn over it stay sharp.
bool blurs(ivec2 at, vec3 color, float d) {
    if (cleared(color, d)) return true;
    if (d <= 0.0) return false;
    return pp.dof.w < 0.5 || texelFetch(sampler2D(t_albedo, s_point), at, 0).a > 0.0;
}

// Distance from the eye along the view, and the blur's radius in pixels there, as Harold's thin lens has it.
float distance_at(vec2 px, float d) {
    return d <= 0.0 ? 1e30 : max(-point_at(px, d).z, 1e-6);
}

float blur_at(float z) {
    return min(pp.dof.x * abs(1.0 / z - pp.dof.y), pp.dof.z);
}

// The finished picture with the guides laid over it: the gates, the heads up display, the ViewCube and the rest
// stay as sharp and as bright as they were drawn, whatever the depth of field and the lighting did underneath (#300).
vec4 with_guides(vec3 picture, ivec2 at) {
    vec4 g = texelFetch(sampler2D(t_overlay, s_point), at, 0);
    return vec4(clamp(g.rgb + picture * (1.0 - clamp(g.a, 0.0, 1.0)), 0.0, 1.0), 1.0);
}

void main() {
    vec2 c = floor(gl_FragCoord.xy) + 0.5;
    vec3 here = picture(c);
    ivec2 at = ivec2(c);
    vec3 color = display_color(at);
    float d = depth_at(c);
    if (pp.dof.z < 1.0 || !blurs(at, color, d)) {
        o = with_guides(here, at);
        return;
    }
    // Gathered along a spiral out to the widest blur. A pixel counts where its own blur reaches this one; one behind
    // this pixel reaches no further than twice this pixel's blur, so a sharp surface keeps its edge over a soft
    // background while a soft one in front spreads over it.
    float z = distance_at(c, d);
    float size = blur_at(z);
    float widest = pp.dof.z;
    // Spacing that puts about 96 taps inside the widest blur.
    float spacing = widest * widest / 192.0;
    vec3 sum = here;
    float total = 1.0;
    float r = spacing;
    float angle = 0.0;
    for (int i = 0; i < 160 && r < widest; ++i) {
        vec2 q = c + vec2(cos(angle), sin(angle)) * r;
        ivec2 qa = clamp(ivec2(q), ivec2(0), ivec2(pc.size.xy) - 1);
        vec2 qc = vec2(qa) + 0.5;
        vec3 qcolor = display_color(qa);
        float dq = depth_at(qc);
        if (blurs(qa, qcolor, dq)) {
            float zq = distance_at(qc, dq);
            float sq = blur_at(zq);
            if (zq > z) sq = min(sq, size * 2.0);
            float m = smoothstep(r - 0.5, r + 0.5, sq);
            sum += mix(sum / total, picture_rough(qa, qcolor, dq), m);
            total += 1.0;
        }
        // The golden angle: taps spread evenly over the disc.
        angle += 2.39996323;
        r += spacing / r;
    }
    o = with_guides(sum / total, at);
}
"#;

/// A view's post effects and how strong they are.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PostSettings {
    pub ambient_occlusion: bool,
    /// How far a surface is shadowed by what is near it, in scene units.
    pub ao_radius: f32,
    pub ao_intensity: f32,
    /// The background's gradient, top and bottom, when it is on.
    pub gradient: Option<([f32; 3], [f32; 3])>,
    /// The grey the scene was cleared to.
    pub clear_grey: f32,
    /// The contact shadow: the view-space direction towards its light, how far it reaches and how strong it is.
    pub contact: Option<([f32; 3], f32, f32)>,
    /// The Realtime renderer's finer occlusion: more taps and a wider blur.
    pub high_quality: bool,
    /// How strong the bounce light is, 0 with it off. Only the Realtime renderer's materials take it.
    pub bounce: f32,
    /// The camera's depth of field, as Harold renders it: the lens's radius in scene units and the distance in focus.
    pub dof: Option<(f32, f32)>,
}

impl Default for PostSettings {
    fn default() -> Self {
        PostSettings { ambient_occlusion: false, ao_radius: 2.0, ao_intensity: 1.5, gradient: None, clear_grey: 0.36, contact: None, high_quality: false, bounce: 0.0, dof: None }
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct PostPush {
    inv_proj: [f32; 16],
    ao: [f32; 4],
    size: [f32; 4],
    bg_top: [f32; 4],
    bg_bottom: [f32; 4],
}

/// The projection and contact shadow, in a uniform buffer so the push block stays at the 128 bytes every device gives.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct PostParams {
    proj: [f32; 16],
    contact_dir: [f32; 4],
    contact: [f32; 4],
    quality: [f32; 4],
    dof: [f32; 4],
}

pub struct Post {
    pipeline: vk::Pipeline,
    ao_pipeline: vk::Pipeline,
    layout: vk::PipelineLayout,
    set_layout: vk::DescriptorSetLayout,
    pool: vk::DescriptorPool,
    set: vk::DescriptorSet,
    ao_set: vk::DescriptorSet,
    /// The bounce light's second pass, reading the first's; and the picture's, reading the second's.
    bounce_set: vk::DescriptorSet,
    bounced_set: vk::DescriptorSet,
    sampler: vk::Sampler,
    params: crate::gpu::Buffer,
}

/// The depth of field's values for the shader: the lens's radius in pixels at unit distance, one over the distance in
/// focus, the widest blur in pixels (0 with it off) and whether only shaded surfaces blur. A point at distance z from the
/// eye blurs over a disc of radius x * |1/z - y| pixels: Harold's thin lens, seen through this view's projection.
fn dof_params(proj: Mat4, s: &PostSettings, w: f32, h: f32) -> [f32; 4] {
    match s.dof {
        Some((radius, focus)) if radius > 0.0 && focus > 0.0 => {
            // The projection's x scale is one over the half width at unit distance; the half width is w/2 pixels.
            let per_unit = proj.x_axis.x.abs() * w * 0.5;
            [radius * per_unit, 1.0 / focus, (h * 0.035).clamp(4.0, 48.0), if s.high_quality { 1.0 } else { 0.0 }]
        }
        _ => [0.0; 4],
    }
}

fn fragment_source(main: &str, multisampled: bool) -> String {
    let (texture, fetch) = if multisampled { ("texture2DMS", "texelFetch(sampler2DMS(t_depth, s_point), at, 0).r") } else { ("texture2D", "texelFetch(sampler2D(t_depth, s_point), at, 0).r") };
    format!("{}{main}", POST_HEAD.replace("DEPTH_TEXTURE", texture).replace("DEPTH_FETCH", fetch))
}

impl Post {
    pub fn new(gpu: &Gpu) -> Post {
        let d = &gpu.device;
        unsafe {
            let module = |code: &[u32]| d.create_shader_module(&vk::ShaderModuleCreateInfo::default().code(code), None).unwrap();
            let vs = module(&compile(POST_VS, naga::ShaderStage::Vertex));
            let multisampled = gpu.samples != vk::SampleCountFlags::TYPE_1;
            let fs = module(&compile(&fragment_source(COMPOSITE_MAIN, multisampled), naga::ShaderStage::Fragment));
            let ao_fs = module(&compile(&fragment_source(AO_MAIN, multisampled), naga::ShaderStage::Fragment));
            let bindings = [
                vk::DescriptorSetLayoutBinding::default().binding(0).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT),
                vk::DescriptorSetLayoutBinding::default().binding(1).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT),
                vk::DescriptorSetLayoutBinding::default().binding(2).descriptor_type(vk::DescriptorType::SAMPLER).descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT),
                vk::DescriptorSetLayoutBinding::default().binding(3).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT),
                vk::DescriptorSetLayoutBinding::default().binding(4).descriptor_type(vk::DescriptorType::UNIFORM_BUFFER).descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT),
                vk::DescriptorSetLayoutBinding::default().binding(5).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT),
                vk::DescriptorSetLayoutBinding::default().binding(6).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT),
            ];
            let set_layout = d.create_descriptor_set_layout(&vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings), None).unwrap();
            let sizes = [
                vk::DescriptorPoolSize::default().ty(vk::DescriptorType::SAMPLED_IMAGE).descriptor_count(24),
                vk::DescriptorPoolSize::default().ty(vk::DescriptorType::SAMPLER).descriptor_count(4),
                vk::DescriptorPoolSize::default().ty(vk::DescriptorType::UNIFORM_BUFFER).descriptor_count(4),
            ];
            let pool = d.create_descriptor_pool(&vk::DescriptorPoolCreateInfo::default().max_sets(4).pool_sizes(&sizes), None).unwrap();
            let layouts = [set_layout];
            let set = d.allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo::default().descriptor_pool(pool).set_layouts(&layouts)).unwrap()[0];
            let sampler = d
                .create_sampler(
                    &vk::SamplerCreateInfo::default()
                        .mag_filter(vk::Filter::NEAREST)
                        .min_filter(vk::Filter::NEAREST)
                        .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                        .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                        .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE),
                    None,
                )
                .unwrap();
            let params = gpu.buffer_with(vk::BufferUsageFlags::UNIFORM_BUFFER, &[PostParams { proj: glam::Mat4::IDENTITY.to_cols_array(), contact_dir: [0.0; 4], contact: [0.0; 4], quality: [0.0; 4], dof: [0.0; 4] }]);
            let push = [vk::PushConstantRange::default().stage_flags(vk::ShaderStageFlags::FRAGMENT).offset(0).size(std::mem::size_of::<PostPush>() as u32)];
            let layout = d.create_pipeline_layout(&vk::PipelineLayoutCreateInfo::default().set_layouts(&layouts).push_constant_ranges(&push), None).unwrap();
            let stages = [
                vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::VERTEX).module(vs).name(c"main"),
                vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::FRAGMENT).module(fs).name(c"main"),
            ];
            let vin = vk::PipelineVertexInputStateCreateInfo::default();
            let ia = vk::PipelineInputAssemblyStateCreateInfo::default().topology(vk::PrimitiveTopology::TRIANGLE_LIST);
            let vp = vk::PipelineViewportStateCreateInfo::default().viewport_count(1).scissor_count(1);
            let rs = vk::PipelineRasterizationStateCreateInfo::default().polygon_mode(vk::PolygonMode::FILL).cull_mode(vk::CullModeFlags::NONE).line_width(1.0);
            let ms = vk::PipelineMultisampleStateCreateInfo::default().rasterization_samples(vk::SampleCountFlags::TYPE_1);
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
                .color_blend_state(&cb)
                .dynamic_state(&dy)
                .layout(layout)
                .render_pass(gpu.post_pass)
                .subpass(0);
            let pipeline = d.create_graphics_pipelines(vk::PipelineCache::null(), &[info], None).map_err(|e| e.1).unwrap()[0];
            // The occlusion pass: the same triangle, its own fragment shader, into the occlusion image.
            let ao_stages = [
                vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::VERTEX).module(vs).name(c"main"),
                vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::FRAGMENT).module(ao_fs).name(c"main"),
            ];
            let ao_info = info.stages(&ao_stages).render_pass(gpu.ao_pass);
            let ao_pipeline = d.create_graphics_pipelines(vk::PipelineCache::null(), &[ao_info], None).map_err(|e| e.1).unwrap()[0];
            // The occlusion pass can't read the image it writes: its set has the scene's colour in that place.
            let ao_set = d.allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo::default().descriptor_pool(pool).set_layouts(&layouts)).unwrap()[0];
            let bounce_set = d.allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo::default().descriptor_pool(pool).set_layouts(&layouts)).unwrap()[0];
            let bounced_set = d.allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo::default().descriptor_pool(pool).set_layouts(&layouts)).unwrap()[0];
            d.destroy_shader_module(vs, None);
            d.destroy_shader_module(fs, None);
            d.destroy_shader_module(ao_fs, None);
            Post { pipeline, ao_pipeline, layout, set_layout, pool, set, ao_set, bounce_set, bounced_set, sampler, params }
        }
    }

    /// Points the sets at the scene's images. Call with no frame in flight (before the frame draws).
    pub fn bind_images(&self, gpu: &Gpu) {
        if gpu.scene_view == vk::ImageView::null() || gpu.overlay_view == vk::ImageView::null() {
            return;
        }
        // Written each frame: a view remade at another size can come back under the handle the old one had.
        let color = [vk::DescriptorImageInfo::default().image_view(gpu.scene_view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
        let depth = [vk::DescriptorImageInfo::default().image_view(gpu.depth_view).image_layout(vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL)];
        let point = [vk::DescriptorImageInfo::default().sampler(self.sampler)];
        let open = [vk::DescriptorImageInfo::default().image_view(gpu.ao_view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
        let bounced = [vk::DescriptorImageInfo::default().image_view(gpu.bounce_view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
        let albedo = [vk::DescriptorImageInfo::default().image_view(gpu.albedo_view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
        let overlay = [vk::DescriptorImageInfo::default().image_view(gpu.overlay_view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
        let params = [vk::DescriptorBufferInfo::default().buffer(self.params.buffer).offset(0).range(std::mem::size_of::<PostParams>() as u64)];
        let mut writes = Vec::new();
        for (set, last) in [(self.set, &open), (self.ao_set, &color), (self.bounce_set, &open), (self.bounced_set, &bounced)] {
            writes.push(vk::WriteDescriptorSet::default().dst_set(set).dst_binding(0).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).image_info(&color));
            writes.push(vk::WriteDescriptorSet::default().dst_set(set).dst_binding(1).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).image_info(&depth));
            writes.push(vk::WriteDescriptorSet::default().dst_set(set).dst_binding(2).descriptor_type(vk::DescriptorType::SAMPLER).image_info(&point));
            writes.push(vk::WriteDescriptorSet::default().dst_set(set).dst_binding(3).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).image_info(last));
            writes.push(vk::WriteDescriptorSet::default().dst_set(set).dst_binding(4).descriptor_type(vk::DescriptorType::UNIFORM_BUFFER).buffer_info(&params));
            writes.push(vk::WriteDescriptorSet::default().dst_set(set).dst_binding(5).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).image_info(&albedo));
            writes.push(vk::WriteDescriptorSet::default().dst_set(set).dst_binding(6).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).image_info(&overlay));
        }
        unsafe { gpu.device.update_descriptor_sets(&writes, &[]) };
    }

    fn push(&self, gpu: &Gpu, proj: Mat4, orthographic: bool, s: &PostSettings) -> PostPush {
        let (w, h) = (gpu.extent.width.max(1) as f32, gpu.extent.height.max(1) as f32);
        self.write_params(proj, s, w, h);
        PostPush {
            inv_proj: proj.inverse().to_cols_array(),
            ao: [s.ao_radius.max(1e-4), s.ao_intensity.max(0.0), if s.ambient_occlusion { 1.0 } else { 0.0 }, if orthographic { 1.0 } else { 0.0 }],
            size: [w, h, 1.0 / w, 1.0 / h],
            bg_top: match s.gradient { Some((t, _)) => [t[0], t[1], t[2], 1.0], None => [0.0; 4] },
            bg_bottom: match s.gradient { Some((_, b)) => [b[0], b[1], b[2], s.clear_grey], None => [0.0, 0.0, 0.0, s.clear_grey] },
        }
    }
}

/// The occlusion passes' constants. They have no use for the background, so its colours are cleared and its first value
/// says only which pass this is: 1 in the bounce light's second, 0 in the first. Left as the view's gradient, a top
/// colour with more than half red (the default gradient's has 0.54) made the first pass act as the second, and the
/// bounce light changed with the background (#301).
fn occlusion_push(mut push: PostPush, second: bool) -> PostPush {
    push.bg_top = [if second { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0];
    push.bg_bottom = [0.0; 4];
    push
}

impl Post {
    /// The projection, contact shadow and depth of field, into the uniform buffer the shaders read.
    fn write_params(&self, proj: Mat4, s: &PostSettings, w: f32, h: f32) {
        let data = PostParams {
            proj: proj.to_cols_array(),
            contact_dir: match s.contact { Some((d, reach, _)) => [d[0], d[1], d[2], reach.max(0.0)], None => [0.0; 4] },
            contact: match s.contact { Some((_, _, strength)) => [strength.max(0.0), 0.0, 0.0, 0.0], None => [0.0; 4] },
            quality: [if s.high_quality { 1.0 } else { 0.0 }, s.bounce.max(0.0), 0.0, 0.0],
            dof: dof_params(proj, s, w, h),
        };
        let bytes: &[u8] = bytemuck::bytes_of(&data);
        if let Some(p) = self.params.alloc.as_ref().and_then(|a| a.mapped_ptr()) {
            unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), p.as_ptr() as *mut u8, bytes.len()) };
        }
    }

    fn pass(&self, gpu: &Gpu, pipeline: vk::Pipeline, set: vk::DescriptorSet, push: &PostPush) {
        unsafe {
            let d = &gpu.device;
            d.cmd_bind_pipeline(gpu.cmd, vk::PipelineBindPoint::GRAPHICS, pipeline);
            d.cmd_bind_descriptor_sets(gpu.cmd, vk::PipelineBindPoint::GRAPHICS, self.layout, 0, &[set], &[]);
            d.cmd_push_constants(gpu.cmd, self.layout, vk::ShaderStageFlags::FRAGMENT, 0, bytemuck::bytes_of(push));
            d.cmd_draw(gpu.cmd, 3, 1, 0, 0);
        }
    }

    /// Works out the occlusion. Between `Gpu::begin_occlusion` and `Gpu::begin_post`. Nothing is drawn with it off.
    pub fn draw_occlusion(&self, gpu: &Gpu, proj: Mat4, orthographic: bool, s: &PostSettings) {
        if s.ambient_occlusion || s.contact.is_some() || s.bounce > 0.0 {
            self.pass(gpu, self.ao_pipeline, self.ao_set, &occlusion_push(self.push(gpu, proj, orthographic, s), false));
        }
    }

    /// The bounce light's second pass: the rays again, now meeting surfaces lit by the first bounce as well. Between
    /// `Gpu::begin_bounce` and `Gpu::begin_post`, with bounce light on.
    pub fn draw_bounce(&self, gpu: &Gpu, proj: Mat4, orthographic: bool, s: &PostSettings) {
        let push = occlusion_push(self.push(gpu, proj, orthographic, s), true);
        self.pass(gpu, self.ao_pipeline, self.bounce_set, &push);
    }

    /// Draws the scene's picture onto the window's image. Between `Gpu::begin_post` and `Gpu::end_frame`.
    pub fn draw(&self, gpu: &Gpu, proj: Mat4, orthographic: bool, s: &PostSettings) {
        let set = if s.bounce > 0.0 { self.bounced_set } else { self.set };
        self.pass(gpu, self.pipeline, set, &self.push(gpu, proj, orthographic, s));
    }

    pub fn destroy(&mut self, gpu: &Gpu) {
        unsafe {
            let d = &gpu.device;
            d.destroy_pipeline(self.pipeline, None);
            d.destroy_pipeline(self.ao_pipeline, None);
            d.destroy_pipeline_layout(self.layout, None);
            d.destroy_descriptor_pool(self.pool, None);
            d.destroy_descriptor_set_layout(self.set_layout, None);
            d.destroy_sampler(self.sampler, None);
            d.destroy_buffer(self.params.buffer, None);
            if let Some(a) = self.params.alloc.take() {
                let _ = gpu.shared.allocator.lock().unwrap().free(a);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_post_shaders_compile_for_one_sample_and_for_several() {
        assert!(!compile(POST_VS, naga::ShaderStage::Vertex).is_empty());
        for multisampled in [false, true] {
            for main in [AO_MAIN, COMPOSITE_MAIN] {
                assert!(!compile(&fragment_source(main, multisampled), naga::ShaderStage::Fragment).is_empty(), "multisampled {multisampled}");
            }
        }
    }

    #[test]
    fn the_blur_is_harolds_thin_lens_through_the_view() {
        // A 35 mm lens at f/2 focused at 10: a 0.875 cm radius. With a 90 degree view 1000 pixels wide, a point at
        // distance 5 blurs over 0.875 * |1/5 - 1/10| * 500 pixels.
        let proj = Mat4::perspective_infinite_reverse_rh(std::f32::consts::FRAC_PI_2, 1.0, 0.1);
        let s = PostSettings { dof: Some((0.875, 10.0)), ..Default::default() };
        let p = dof_params(proj, &s, 1000.0, 1000.0);
        assert!((p[0] * (1.0 / 5.0 - p[1]) - 0.875 * 0.1 * 500.0).abs() < 1e-3, "{p:?}");
        assert!(p[2] > 0.0);
        assert_eq!(dof_params(proj, &PostSettings::default(), 1000.0, 1000.0), [0.0; 4]);
    }

    #[test]
    fn the_background_never_reaches_the_bounce_light() {
        // #301: the default gradient's top colour has 0.54 red, which the first occlusion pass took for the flag of the
        // second. Whatever the background, the first pass is the first and the second the second.
        let push = PostPush { inv_proj: [0.0; 16], ao: [0.0; 4], size: [0.0; 4], bg_top: [0.54, 0.62, 0.70, 1.0], bg_bottom: [0.1, 0.1, 0.1, 0.63] };
        let first = occlusion_push(push, false);
        assert_eq!(first.bg_top, [0.0; 4]);
        assert_eq!(first.bg_bottom, [0.0; 4]);
        let white = PostPush { bg_top: [1.0; 4], ..push };
        assert_eq!(occlusion_push(white, false).bg_top[0], 0.0);
        assert_eq!(occlusion_push(push, true).bg_top, [1.0, 0.0, 0.0, 0.0]);
        // A hit on a surface's edge takes its colour from a pixel the surface covers, not the part background one.
        assert!(AO_MAIN.contains("ivec2 q = covered_near("));
    }

    #[test]
    fn the_guides_go_over_the_finished_picture() {
        // #300: whatever the picture's last step (sharp, or gathered by the depth of field), the guides' layer goes on
        // after it, so neither the blur nor the occlusion and bounce light ever reach them.
        let main = &COMPOSITE_MAIN[COMPOSITE_MAIN.find("void main()").unwrap()..];
        let outputs: Vec<&str> = main.lines().map(str::trim).filter(|l| l.starts_with("o = ")).collect();
        assert_eq!(outputs.len(), 2, "{outputs:?}");
        assert!(outputs.iter().all(|l| l.starts_with("o = with_guides(")), "{outputs:?}");
        // Nothing before the composite reads the guides' layer.
        assert!(!AO_MAIN.contains("t_overlay"));
    }

    #[test]
    fn the_push_block_is_what_the_shader_declares() {
        // In reverse projection, the occlusion, the size and the two background colours: the 128 bytes every device gives.
        // The projection and contact shadow ride in a uniform buffer, so this stays at that minimum.
        assert_eq!(std::mem::size_of::<PostPush>(), 128);
    }
}
