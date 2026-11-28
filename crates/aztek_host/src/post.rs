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

// The sRGB transfer the final renderer writes its images with: one curve on both paths, so a radiance is the same
// number whichever renderer shows it.
vec3 to_srgb(vec3 c) {
    c = clamp(c, 0.0, 1.0);
    return mix(12.92 * c, 1.055 * pow(c, vec3(1.0 / 2.4)) - 0.055, step(vec3(0.0031308), c));
}
vec3 to_linear(vec3 c) {
    c = clamp(c, 0.0, 1.0);
    return mix(c / 12.92, pow((c + 0.055) / 1.055, vec3(2.4)), step(vec3(0.04045), c));
}
layout(location = 0) in vec2 uv;
layout(location = 0) out vec4 o;
layout(set = 0, binding = 0) uniform texture2D t_scene;
layout(set = 0, binding = 1) uniform DEPTH_TEXTURE t_depth;
layout(set = 0, binding = 2) uniform sampler s_point;
layout(set = 0, binding = 3) uniform texture2D t_open;
layout(set = 0, binding = 5) uniform texture2D t_albedo;
// The view's guides, drawn after the scene onto a clear layer, premultiplied by their alpha (#300).
layout(set = 0, binding = 6) uniform texture2D t_overlay;
// The Realtime renderer's material (#302): reflection colour and roughness, then transmission, IOR and metalness.
layout(set = 0, binding = 7) uniform texture2D t_mat;
layout(set = 0, binding = 8) uniform texture2D t_mat_aux;
// The reflections and refraction the reflection pass worked out from these.
layout(set = 0, binding = 9) uniform texture2D t_refl;
// The scene mirrored below the ground plane, for the floor's reflection (#302).
layout(set = 0, binding = 10) uniform texture2D t_planar;
// The scene without its transmissive surfaces, for glass to see what it lets through (#302).
layout(set = 0, binding = 11) uniform texture2D t_back;
// The reflection image low-passed over a few pixels, so one bright texel does not speckle (#302).
layout(set = 0, binding = 12) uniform texture2D t_refl_soft;
layout(set = 0, binding = 13) uniform sampler s_refl;
// The far side of the transmissive bodies: view normal in rgb, view distance in a.
layout(set = 0, binding = 14) uniform texture2D t_backface;
// The view's sky dome, which reflections fall back on where the view shows nothing to reflect.
layout(set = 1, binding = 0) uniform texture2D t_env;
layout(set = 1, binding = 1) uniform sampler s_env;
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
    // light is, 0 with it off. z: 1 when the screen-space reflections are on.
    vec4 quality;
    // Depth of field. x: the lens's radius in pixels at unit distance (blur = x * |1/distance - y|); y: one over the
    // distance in focus; z: the widest blur in pixels, 0 with it off; w: 1 when only shaded surfaces blur (the
    // Realtime renderer marks them in the albedo's alpha).
    vec4 dof;
    // View space back to world, for a reflection looked up in the environment.
    mat4 view_to_world;
    // The sky dome's colour and strength, w 1 when an image feeds it; and its world-to-dome rotation.
    vec4 env;
    vec4 env_rot;
    // x: 1 with a ground plane's reflection, yzw: the world's up.
    vec4 extra;
    // The scene's sky dome as the view's background: colour in rgb, w 0 none, 1 colour, 2 image.
    vec4 bg_env;
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

// Noise that differs from pixel to pixel with no pattern the eye picks out. The integer hash spreads the low bits
// over both axes; the sine-free form avoids the diagonal bands a cheap dot-and-fract leaves on a screen grid.
float noise(vec2 px) {
    vec3 p3 = fract(vec3(px.xyx) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
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
    // Contact shadows: march a few of the map's texels into the scene from the point towards its light, for the one
    // band at a caster's base that the map's own texels are too coarse for.
    float contact = 1.0;
    if (pp.contact.x > 0.0 && pp.contact_dir.w > 0.0) {
        vec3 dir = normalize(pp.contact_dir.xyz);
        float far = pp.contact_dir.w;
        // Start clear of the surface, and take only a surface that is clearly between the marched point and the eye:
        // a floor seen at a grazing angle stands a hair in front of its own ray, and too thin a threshold read that as
        // a dark patch over the whole floor.
        float start = max(0.05 * far, 0.5 * per_px);
        float step_len = (far - start) / 12.0;
        float thick = max(0.15 * far, 1.2 * per_px);
        float t = start + step_len;
        float strength = 0.0;
        for (int i = 0; i < 12; ++i) {
            vec3 at = p + dir * t;
            vec4 clip = pp.proj * vec4(at, 1.0);
            if (clip.w <= 0.0) break;
            vec2 sx = clip.xy / clip.w * 0.5 + 0.5;
            if (sx.x < 0.0 || sx.x > 1.0 || sx.y < 0.0 || sx.y > 1.0) break;
            vec2 q = floor(sx * pc.size.xy) + 0.5;
            float dq = depth_at(q);
            if (dq > 0.0) {
                float diff = point_at(q, dq).z - at.z;
                if (diff > thick && diff < 0.9 * far) {
                    strength = 1.0 - smoothstep(0.6 * far, far, t);
                    break;
                }
            }
            t += step_len;
        }
        // A surface turned away from the light takes no extra darkening: the shadow map owns that side.
        contact = 1.0 - pp.contact.x * strength * clamp(dot(n, dir), 0.0, 1.0);
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
    // A metal has no diffuse bounce, and glass lets light through rather than bouncing it back.
    vec4 aux_here = texelFetch(sampler2D(t_mat_aux, s_point), ivec2(c), 0);
    bool diffuse_here = aux_here.b <= 0.5 && aux_here.r <= 0.5;
    if (pp.quality.y > 0.0 && diffuse_here && texelFetch(sampler2D(t_albedo, s_point), ivec2(c), 0).a > 0.0) {
        vec3 tn = normalize(cross(n, abs(n.y) < 0.9 ? vec3(0.0, 1.0, 0.0) : vec3(1.0, 0.0, 0.0)));
        vec3 bn = cross(n, tn);
        // How far a ray goes: as far as the point is from the eye, which spans what the view shows.
        float reach = pc.ao.w > 0.5 ? 2.0 * pc.ao.x * 8.0 : length(p);
        float jitter = noise(c.yx + vec2(17.0, 5.0));
        // Rays leave a little off the surface, so those that skim along it don't meet it again in its own depth.
        vec3 from = p + n * (0.01 * reach);
        // The bounce's grain is the few stochastic hits a pixel gets; the clamp and the filter carry the rest.
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
                    for (int b = 0; b < 8; ++b) {
                        float mid = 0.5 * (lo + hi);
                        if (behind_at(from + dir * mid) > 0.0) hi = mid; else lo = mid;
                    }
                    vec3 at = from + dir * hi;
                    if (behind_at(at) < 0.05 * hi + 0.02 * reach) {
                        vec4 clip = pp.proj * vec4(at, 1.0);
                        ivec2 q = covered_near(ivec2(clip.xy / clip.w * 0.5 * pc.size.xy + 0.5 * pc.size.xy));
                        vec3 hit_colour = texelFetch(sampler2D(t_albedo, s_point), q, 0).rgb;
                        // A ray that lands on a specular highlight must not carry it back as a bright texel: one such
                        // hit would light a whole square of the filter's footprint. The offline path clamps its
                        // indirect samples for the same reason.
                        gi += min(to_linear(max(texelFetch(sampler2D(t_scene, s_point), q, 0).rgb, vec3(0.0))), vec3(4.0));
                        // The second time round, the surface met also sends back what the first bounce brought it.
                        if (second) gi += min(hit_colour * texelFetch(sampler2D(t_open, s_point), q, 0).rgb, vec3(4.0));
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
        // Rays that leave the view meet what it does not show: with a sky dome, the room it lights, taken to be half as
        // bright as what the others met; without one, nothing. The fill was standing in for a dome even on an open floor
        // that has none, and invented light there. The whole bounce is multiplied by the receiver's openness where the
        // picture reads it, so the fill never lifts a contact or the floor behind an object.
        bool has_sky = pp.env.w > 0.5 || dot(pp.env.rgb, pp.env.rgb) > 0.0;
        if (has_sky && hits > 0.5) {
            gi += gi / hits * gone * 0.5;
            hit_albedo += hit_albedo / hits * gone * 0.5;
        }
        vec3 keep = second ? vec3(0.0) : min(hit_albedo / float(RAYS), vec3(0.9));
        gi *= pp.quality.y / float(RAYS) / (vec3(1.0) - keep);
    }
    o = vec4(gi, clamp(open * contact, 0.0, 1.0));
}
"#;

/// The bounce light low-passed along one axis: the rays that found it are stochastic, and indirect light is not, so a
/// separable Gaussian turns the grain into a soft field before the picture reads it (#302).
const BOUNCE_BLUR_MAIN: &str = r#"
void main() {
    vec2 c = floor(gl_FragCoord.xy) + 0.5;
    vec2 dir = pc.bg_top.xy;
    float sigma = max(pc.bg_top.z, 0.5);
    int radius = int(clamp(ceil(sigma * 2.5), 1.0, 16.0));
    vec4 sum = vec4(0.0);
    float total = 0.0;
    for (int i = -16; i <= 16; ++i) {
        if (i < -radius || i > radius) continue;
        float f = float(i);
        float w = exp(-f * f / (2.0 * sigma * sigma));
        ivec2 at = clamp(ivec2(c + dir * f), ivec2(0), ivec2(pc.size.xy) - 1);
        sum += textureLod(sampler2D(t_open, s_refl), (vec2(at) + 0.5) / pc.size.xy, 0.0) * w;
        total += w;
    }
    // The openness and contact shadow share the buffer and the rays' grain, so they are low-passed with them: a sharp
    // alpha against the soft light made the contact shadow's steps show on the sphere's dark side.
    o = sum / max(total, 1e-6);
}
"#;

/// Screen-space reflections and refraction (#302): what a material's reflection sees of the scene, and what glass lets
/// through, in linear light. The environment stands in where the view shows nothing to reflect.
const REFLECT_MAIN: &str = r#"
const float PI = 3.141592653589793;

float noise_at(vec2 px) {
    return fract(52.9829189 * fract(dot(px, vec2(0.06711056, 0.00583715))));
}
vec3 qrot(vec4 q, vec3 v) {
    return v + 2.0 * cross(q.xyz, cross(q.xyz, v) + q.w * v);
}
// The sky dome a reflection sees, at the roughness's blur.
vec3 env_lookup(vec3 dir, float rough) {
    if (pp.env.w < 0.5 && dot(pp.env.rgb, pp.env.rgb) <= 0.0) return vec3(0.0);
    vec3 wd = normalize((pp.view_to_world * vec4(dir, 0.0)).xyz);
    vec3 ld = normalize(qrot(pp.env_rot, wd));
    vec2 uv = vec2(atan(ld.z, ld.x) / (2.0 * PI) + 0.5, acos(clamp(ld.y, -1.0, 1.0)) / PI);
    vec3 env = pp.env.w > 0.5 ? textureLod(sampler2D(t_env, s_env), uv, rough * 6.0).rgb : vec3(1.0);
    return env * pp.env.rgb;
}
// The ground plane's reflection: the scene mirrored below the world's ground, read where the surface is that plane.
// cover comes back 1 there and 0 elsewhere; where the mirrored scene shows nothing the sky stands in.
vec3 planar_at(vec2 c, vec3 p, vec3 n, vec3 r, float rough, out float cover) {
    vec3 wp = (pp.view_to_world * vec4(p, 1.0)).xyz;
    vec3 wn = normalize((pp.view_to_world * vec4(n, 0.0)).xyz);
    vec3 up = normalize(pp.extra.yzw);
    cover = smoothstep(0.86, 0.98, dot(wn, up)) * (1.0 - smoothstep(0.04, 0.25, abs(dot(wp, up))));
    float planarity = cover;
    if (pp.extra.x < 0.5 || cover <= 0.001) return vec3(0.0);
    // A few pixels around the sample: the planar image's edge is a hard silhouette, and the weight that hands over to
    // the screen-space reflection has to cross it gradually.
    ivec2 at = ivec2(c);
    ivec2 top = ivec2(pc.size.xy) - 1;
    vec3 plane = vec3(0.0);
    // How much of the sample is mirrored scene: the planar pass clears to transparent black, so its alpha is the
    // coverage. Lightness is not coverage: a mirrored sphere's own shadowed side is dark and still a reflection.
    float data = 0.0;
    for (int y = -1; y <= 1; ++y) {
        for (int x = -1; x <= 1; ++x) {
            ivec2 q = clamp(at + ivec2(x, y), ivec2(0), top);
            // Linear filtering, so the mirrored silhouette hands over to the floor without a stepped edge.
            vec4 t = textureLod(sampler2D(t_planar, s_refl), (vec2(q) + 0.5) / pc.size.xy, 0.0);
            // A mirrored pass can leave a non-finite texel where a projected edge divides by zero; one of those blacked
            // a whole floor once it reached the composite. Drop it and let the sky stand in.
            if (!(t.r == t.r && t.g == t.g && t.b == t.b && t.a == t.a)) continue;
            if (abs(t.r) > 1e30 || abs(t.g) > 1e30 || abs(t.b) > 1e30 || abs(t.a) > 1e30) continue;
            plane += to_linear(max(t.rgb, vec3(0.0)));
            data += t.a;
        }
    }
    plane /= 9.0;
    // Where the planar image has nothing, the screen-space reflection stands in: the weight falls away at the image's
    // edge and with roughness, and the two are mixed by one minus the other in the caller.
    data = smoothstep(0.0, 0.5, data / 9.0);
    float rough_w = 1.0 - 0.5 * smoothstep(0.15, 0.7, rough);
    // The plane itself takes the planar image, fading to the sky where the image has nothing. The screen-space march
    // fills in on surfaces that are not the mirror, and only partly here where the surface is rough.
    cover = planarity * rough_w;
    return mix(env_lookup(r, rough), plane, data);
}
// Where a view-space ray first goes behind what the view shows. Glass is passed through: a refracted ray starts behind
// it, and a reflected one sees what is behind glass too. colour comes back in linear light.
bool scene_hit(vec3 from, vec3 dir, float jitter, out vec3 colour, out float travelled) {
    float scale = max(length(from), 1.0);
    float span = clamp(scale * 12.0, 4.0, 240.0);
    float thick = 0.01 * scale + 0.002;
    float start = 0.05 * scale;
    float prev = start;
    bool was_in_front = false;
    ivec2 top = ivec2(pc.size.xy) - 1;
    for (int i = 0; i < 64; ++i) {
        float f = (float(i) + jitter) / 64.0;
        float t = start + span * f * f;
        vec3 p = from + dir * t;
        vec4 clip = pp.proj * vec4(p, 1.0);
        if (clip.w <= 0.0) break;
        vec2 ndc = clip.xy / clip.w;
        if (abs(ndc.x) > 1.0 || abs(ndc.y) > 1.0) break;
        ivec2 at = clamp(ivec2((ndc * 0.5 + 0.5) * pc.size.xy), ivec2(0), top);
        vec2 c = vec2(at) + 0.5;
        float dq = depth_at(c);
        if (dq <= 0.0) {
            prev = t;
            was_in_front = true;
            continue;
        }
        // Glass does not stop the ray: what is behind it is what a reflection or a refraction sees.
        if (texelFetch(sampler2D(t_mat_aux, s_point), at, 0).r > 0.5) {
            prev = t;
            was_in_front = true;
            continue;
        }
        vec3 q = point_at(c, dq);
        bool behind = -p.z > -q.z + thick;
        if (behind && was_in_front) {
            float lo = prev;
            float hi = t;
            for (int b = 0; b < 8; ++b) {
                float mid = 0.5 * (lo + hi);
                vec3 mp = from + dir * mid;
                vec4 mc = pp.proj * vec4(mp, 1.0);
                if (mc.w <= 0.0) break;
                vec2 ms = mc.xy / mc.w * 0.5 + 0.5;
                float md = depth_at(ms * pc.size.xy);
                if (md > 0.0 && -mp.z > -point_at(ms * pc.size.xy, md).z + thick) hi = mid; else lo = mid;
            }
            vec3 hp = from + dir * hi;
            vec4 hc = pp.proj * vec4(hp, 1.0);
            if (hc.w > 0.0) {
                vec2 hs = hc.xy / hc.w * 0.5 + 0.5;
                ivec2 hat = clamp(ivec2(hs * pc.size.xy), ivec2(0), top);
                if (texelFetch(sampler2D(t_mat_aux, s_point), hat, 0).r <= 0.5) {
                    colour = to_linear(texelFetch(sampler2D(t_scene, s_point), hat, 0).rgb);
                    travelled = hi;
                    return true;
                }
            }
        }
        prev = t;
        was_in_front = !behind;
    }
    return false;
}

void main() {
    vec2 c = floor(gl_FragCoord.xy) + 0.5;
    ivec2 at = ivec2(c);
    vec4 m = texelFetch(sampler2D(t_mat, s_point), at, 0);
    vec4 aux = texelFetch(sampler2D(t_mat_aux, s_point), at, 0);
    float d = depth_at(c);
    if (aux.a < 0.5 || d <= 0.0) {
        o = vec4(0.0);
        return;
    }
    vec3 p = point_at(c, d);
    vec3 n = normal_at(c, d, p);
    vec3 v = normalize(p);
    float rough = clamp(m.a - 0.002, 0.02, 1.0);
    float transmission = clamp(aux.r, 0.0, 1.0);
    if (int(pp.quality.w) == 10) {
        o = vec4(pp.extra.x > 0.5 ? vec3(0.0, 1.0, 0.0) : vec3(1.0, 0.0, 0.0), 1.0);
        return;
    }
    float ior = 1.0 + clamp(aux.g, 0.0, 1.0) * 2.0;
    vec3 coeff = max(m.rgb, vec3(0.0));
    vec3 sum = vec3(0.0);
    float lift = 0.01 * length(p) + 0.002;
    // Glass: the two interfaces it really has. A solid body bends the ray at its front, carries it to its far side, and
    // bends it again there (or reflects it, at a grazing exit): the second bend is what inverts and magnifies what is
    // behind. A pane's interfaces are parallel, so the ray leaves along the way it came and the picture behind it is
    // only displaced. Where the view has nothing behind the glass, the dome stands in; glass is never black.
    if (transmission > 0.5) {
        vec3 tint = texelFetch(sampler2D(t_albedo, s_point), at, 0).rgb;
        vec3 dir_in = refract(v, n, 1.0 / ior);
        if (dot(dir_in, dir_in) < 1e-6) dir_in = reflect(v, n);
        // The far side of the body, from the pass that drew its back faces: where the ray leaves it, and which way that
        // surface faces. The second bend there is the one that inverts and magnifies what lies behind.
        vec4 far_side = texelFetch(sampler2D(t_backface, s_point), at, 0);
        vec3 hit;
        float travelled;
        if (far_side.a > 0.0) {
            vec3 exit_n = normalize(far_side.xyz);
            // The far side sits on the same pixel ray; lay it at its own view distance.
            vec3 exit_p = p * (far_side.a / max(-p.z, 1e-6));
            float path = length(exit_p - p);
            vec3 dir_out = refract(dir_in, -exit_n, ior);
            if (dot(dir_out, dir_out) < 1e-6) dir_out = reflect(dir_in, -exit_n);
            vec3 seen = scene_hit(p + n * lift, dir_out, 0.5, hit, travelled) ? hit : env_lookup(dir_out, rough);
            // The tint deepens with the path through the body.
            sum += seen * tint * exp(-path * 0.06);
        } else {
            // A single-faced pane: sample the view's own picture behind it, bent out towards the rim where the surface
            // turns away from the eye, and let the dome stand in where that picture is the empty background.
            vec2 radial = vec2(n.x, -n.y);
            float edge = 1.0 - clamp(abs(n.z), 0.0, 1.0);
            vec2 offs = normalize(radial + vec2(0.0001)) * (edge * edge * 200.0);
            vec2 q = clamp(c + offs, vec2(1.0), pc.size.xy - 2.0);
            vec3 behind = vec3(0.0);
            for (int y = -1; y <= 1; ++y) {
                for (int x = -1; x <= 1; ++x) {
                    ivec2 qi = clamp(ivec2(q) + ivec2(x, y), ivec2(0), ivec2(pc.size.xy) - 1);
                    behind += to_linear(max(texelFetch(sampler2D(t_back, s_point), qi, 0).rgb, vec3(0.0)));
                }
            }
            behind /= 9.0;
            float behind_lum = dot(behind, vec3(0.2126, 0.7152, 0.0722));
            behind = mix(env_lookup(reflect(v, n), rough), behind, smoothstep(0.0, 0.02, behind_lum));
            sum += behind * tint;
        }
    }
    // The reflection: the scene where the view shows it, the sky where it doesn't. Glossy surfaces are smoothed over
    // the reflection image in the composite, from their roughness.
    vec3 r = reflect(v, n);
    vec3 hit;
    float travelled;
    // A mirror marches at a fixed phase: the jitter that rougher surfaces need would only grain it.
    float jitter = rough < 0.08 ? 0.5 : noise_at(c);
    vec3 refl = scene_hit(p + n * lift, r, jitter, hit, travelled) ? hit : env_lookup(r, rough);
    float planar_cover;
    vec3 planar = planar_at(c, p, n, r, rough, planar_cover);
    if (int(pp.quality.w) == 11) {
        o = vec4(vec3(planar_cover), 1.0);
        return;
    }
    // The two tiers are mixed by one weight, so their sum is the one reflection. Both take the same brightness scale
    // and go through the same filtered copy by the surface's roughness.
    refl = mix(refl, planar, planar_cover) * 1.3;
    // The same guard at the last step: a bad sample in either tier becomes the sky rather than a black surface.
    if (!(refl.r == refl.r && refl.g == refl.g && refl.b == refl.b)) refl = env_lookup(r, rough);
    if (abs(refl.r) > 1e30 || abs(refl.g) > 1e30 || abs(refl.b) > 1e30) refl = env_lookup(r, rough);
    sum += refl * coeff;
    o = vec4(sum, 1.0);
}
"#;

/// A small low-pass of the reflection image: a bright texel from a mirrored specular highlight is spread over its
/// neighbours before a rough surface gathers it, so it does not come back as a speckle (#302).
const REFL_BLUR_MAIN: &str = r#"
void main() {
    vec2 c = floor(gl_FragCoord.xy) + 0.5;
    ivec2 at = ivec2(c);
    ivec2 top = ivec2(pc.size.xy) - 1;
    vec3 sum = vec3(0.0);
    for (int y = -1; y <= 1; ++y) {
        for (int x = -1; x <= 1; ++x) {
            ivec2 q = clamp(at + ivec2(x, y), ivec2(0), top);
            sum += clamp(texelFetch(sampler2D(t_refl, s_point), q, 0).rgb, vec3(0.0), vec3(8.0));
        }
    }
    o = vec4(sum / 9.0, 1.0);
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

// The reflection image at a pixel, smoothed over the surface round about as the material's roughness asks: a mirror
// keeps one ray, a rough metal gathers its neighbourhood. Neighbouring pixels on another surface are left out, so an
// edge stays an edge.
vec3 reflection_at(ivec2 at, float d) {
    float rough = texelFetch(sampler2D(t_mat, s_point), at, 0).a - 0.002;
    vec2 uv = (vec2(at) + 0.5) / pc.size.xy;
    if (d <= 0.0) return textureLod(sampler2D(t_refl_soft, s_refl), uv, 0.0).rgb;
    // The mip chain is the filtered copy: one bright texel of a mirrored highlight is spread over the level the
    // roughness asks for, so a rough gather is smooth and a mirror stays a mirror.
    // The radius is a physical blur, so it is measured against a 720-line view and grows with this one: a rough
    // surface keeps the same softness when the window or the display's pixel ratio changes.
    float res = max(pc.size.y / 720.0, 1.0);
    float radius = clamp(rough * rough * 120.0, 8.0, 32.0) * res;
    // The disc below is the gather itself; the level it starts from is only a prefilter, so take it two steps sharper
    // than the radius and let the disc reach the rest. Sampling the whole blur from the level made every reflection
    // step at an eighth of the view (#302).
    float lod = clamp(log2(radius) - 2.0, 1.0, 4.0);
    float z = -point_at(vec2(at) + 0.5, d).z;
    vec3 base = clamp(textureLod(sampler2D(t_refl_soft, s_refl), uv, lod).rgb, vec3(0.0), vec3(8.0));
    vec3 sum = base;
    float total = 1.0;
    ivec2 top = ivec2(pc.size.xy) - 1;
    // A fixed disc inside the reflection ray's footprint: golden-angle taps, no per-frame or per-pixel noise.
    for (int i = 0; i < 8; ++i) {
        float t = (float(i) + 0.5) / 8.0;
        float a = float(i) * 2.39996323;
        vec2 q = vec2(at) + 0.5 + vec2(cos(a), sin(a)) * sqrt(t) * radius * 0.5;
        ivec2 qa = clamp(ivec2(q), ivec2(0), top);
        float dq = depth_at(vec2(qa) + 0.5);
        if (dq <= 0.0) continue;
        float zq = -point_at(vec2(qa) + 0.5, dq).z;
        float w = exp(-abs(zq - z) / (0.02 * z + 0.1));
        vec2 quv = (vec2(qa) + 0.5) / pc.size.xy;
        sum += clamp(textureLod(sampler2D(t_refl_soft, s_refl), quv, lod).rgb, vec3(0.0), vec3(8.0)) * w;
        total += w;
    }
    return sum / total;
}

const float SKY_PI = 3.141592653589793;
vec3 sky_turn(vec4 q, vec3 v) {
    return v + 2.0 * cross(q.xyz, cross(q.xyz, v) + q.w * v);
}
// The view direction a pixel looks along: the eye's ray, or the view's forward when the rays are parallel.
vec3 view_dir(vec2 c) {
    vec4 a = pc.inv_proj * vec4(c * pc.size.zw * 2.0 - 1.0, 1.0, 1.0);
    vec4 b = pc.inv_proj * vec4(c * pc.size.zw * 2.0 - 1.0, 0.0, 1.0);
    vec3 d = a.xyz / a.w - b.xyz / b.w;
    return normalize(length(d) > 1e-6 ? d : vec3(0.0, 0.0, -1.0));
}
// The dome the scene lights with, in that direction: its image, or its own colour.
vec3 dome_at(vec3 dir_view) {
    vec3 wd = (pp.view_to_world * vec4(dir_view, 0.0)).xyz;
    if (pp.bg_env.w < 1.5) return pp.bg_env.rgb;
    vec3 d = normalize(sky_turn(pp.env_rot, wd));
    vec2 uv = vec2(atan(d.z, d.x) / (2.0 * SKY_PI) + 0.5, acos(clamp(d.y, -1.0, 1.0)) / SKY_PI);
    return textureLod(sampler2D(t_env, s_env), uv, 0.0).rgb;
}

// The scene's colour with the occlusion and bounce light (`got`, rgb bounce and alpha openness) on it, and the screen-
// space reflections and refraction added in linear light.
vec3 lit(vec3 color, vec4 got, ivec2 at, float d) {
    float open = got.a;
    vec3 shaded = color * open;
    if (pp.quality.y > 0.0) {
        // The bounce is added in linear light, then the picture goes back to the display's encoding.
        vec3 albedo = texelFetch(sampler2D(t_albedo, s_point), at, 0).rgb;
        shaded = to_srgb(to_linear(shaded) + albedo * got.rgb * open);
    }
    if (pp.quality.z > 0.5) {
        vec3 refl = reflection_at(at, d);
        int dbg = int(pp.quality.w);
        if (dbg == 1) return to_srgb(max(refl, vec3(0.0)));
        if (dbg == 2) return texelFetch(sampler2D(t_mat, s_point), at, 0).rgb;
        if (dbg == 3) return vec3(texelFetch(sampler2D(t_mat, s_point), at, 0).a - 0.002);
        if (dbg == 4) return vec3(texelFetch(sampler2D(t_mat_aux, s_point), at, 0).b);
        if (dbg == 5) return vec3(texelFetch(sampler2D(t_mat_aux, s_point), at, 0).g);
        if (dbg == 6) return texelFetch(sampler2D(t_planar, s_point), at, 0).rgb;
        if (dbg == 7) return vec3(texelFetch(sampler2D(t_mat_aux, s_point), at, 0).r);
        if (dbg == 8) return texelFetch(sampler2D(t_albedo, s_point), at, 0).rgb;
        if (dbg == 13) return texelFetch(sampler2D(t_refl_soft, s_point), at, 0).rgb;
        if (dbg == 14) return texelFetch(sampler2D(t_refl, s_point), at, 0).rgb;
        if (dbg == 15) return reflection_at(at, d);
        if (dbg == 16) return textureLod(sampler2D(t_refl_soft, s_refl), (vec2(at) + 0.5) / pc.size.xy, 3.0).rgb;
        if (dbg == 20) return to_srgb(max(got.rgb, vec3(0.0)));
        if (dbg == 21) return vec3(got.a);
        if (dbg == 22) return to_srgb(max(texelFetch(sampler2D(t_open, s_point), at, 0).rgb, vec3(0.0)));
        if (dbg == 23) return vec3(texelFetch(sampler2D(t_open, s_point), at, 0).a);
        if (dbg == 24) return texelFetch(sampler2D(t_back, s_point), at, 0).rgb;
        if (dbg == 25) return vec3(texelFetch(sampler2D(t_mat_aux, s_point), at, 0).r);
        shaded = to_srgb(to_linear(shaded) + refl);
    }
    return shaded;
}

// The picture at a pixel centre.
vec3 picture(vec2 c) {
    vec3 color = display_color(ivec2(c));
    float d = depth_at(c);
    // Nothing drawn here: the scene's own sky dome, which is what the final render shows; else the display gradient,
    // top of the view to bottom; else the grey the scene was cleared to. What was drawn without depth keeps its colour.
    if (pp.bg_env.w > 0.5 && cleared(color, d)) return to_srgb(max(dome_at(view_dir(c)), vec3(0.0)));
    if (pc.bg_top.w > 0.5 && cleared(color, d)) return mix(pc.bg_top.rgb, pc.bg_bottom.rgb, c.y * pc.size.w);
    if (effects_off() || d <= 0.0) return lit(color, vec4(0.0, 0.0, 0.0, 1.0), ivec2(c), d);
    // The bounce was low-passed along both axes before this pass, so one bilinear sample is already smooth: no
    // stochastic grain and no square footprint, and no gather cost here.
    ivec2 at = clamp(ivec2(c), ivec2(0), ivec2(pc.size.xy) - 1);
    vec4 got = textureLod(sampler2D(t_open, s_refl), (vec2(at) + 0.5) / pc.size.xy, 0.0);
    return lit(color, got, ivec2(c), d);
}

// The picture at a pixel the depth of field gathers from: the same, without the occlusion's smoothing, which the
// gathering itself stands in for.
vec3 picture_rough(ivec2 at, vec3 color, float d) {
    if (pc.bg_top.w > 0.5 && cleared(color, d)) return mix(pc.bg_top.rgb, pc.bg_bottom.rgb, (float(at.y) + 0.5) * pc.size.w);
    if (effects_off() || d <= 0.0) return lit(color, vec4(0.0, 0.0, 0.0, 1.0), at, d);
    return lit(color, textureLod(sampler2D(t_open, s_refl), (vec2(at) + 0.5) / pc.size.xy, 0.0), at, d);
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
    /// Screen-space reflections and refraction (#302), only with the Realtime renderer's material buffer.
    pub reflections: bool,
    /// Show the reflection buffer itself (1), the material's reflection colour (2) or roughness (3), for checking the
    /// pass.
    pub debug_reflection: u32,
    /// Whether a ground plane is being reflected, and which way the world's up is.
    pub planar_reflection: bool,
    pub up: [f32; 3],
    /// The camera's world-to-view matrix, for looking a reflection up in the environment.
    pub view: Mat4,
    /// The sky dome the reflections fall back on: its colour and strength, w 1 when an image feeds it; and its
    /// world-to-dome rotation.
    pub env: [f32; 4],
    pub env_rot: [f32; 4],
    /// The scene's sky dome as the view's background: its colour in rgb, and w 0 with none, 1 for a colour, 2 for an
    /// image.
    pub bg_env: [f32; 4],
}

impl Default for PostSettings {
    fn default() -> Self {
        PostSettings {
            ambient_occlusion: false,
            ao_radius: 2.0,
            ao_intensity: 1.5,
            gradient: None,
            clear_grey: 0.36,
            contact: None,
            high_quality: false,
            bounce: 0.0,
            dof: None,
            reflections: false,
            debug_reflection: 0,
            planar_reflection: false,
            up: [0.0, 1.0, 0.0],
            view: Mat4::IDENTITY,
            env: [0.0; 4],
            env_rot: [0.0, 0.0, 0.0, 1.0],
            bg_env: [0.0; 4],
        }
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
    /// View space back to world, for a reflection looked up in the environment.
    view_to_world: [f32; 16],
    /// The sky dome's colour and strength, w 1 when an image feeds it.
    env: [f32; 4],
    /// World to the dome's space.
    env_rot: [f32; 4],
    /// x: 1 with a ground plane's reflection, yzw: the world's up.
    extra: [f32; 4],
    /// The sky dome as the view's background: colour and kind, as `PostSettings::bg_env`.
    bg_env: [f32; 4],
}

pub struct Post {
    pipeline: vk::Pipeline,
    ao_pipeline: vk::Pipeline,
    reflection_pipeline: vk::Pipeline,
    reflection_blur_pipeline: vk::Pipeline,
    bounce_blur_pipeline: vk::Pipeline,
    layout: vk::PipelineLayout,
    set_layout: vk::DescriptorSetLayout,
    env_layout: vk::DescriptorSetLayout,
    pool: vk::DescriptorPool,
    set: vk::DescriptorSet,
    ao_set: vk::DescriptorSet,
    /// The bounce light's second pass, reading the first's; and the picture's, reading the second's.
    bounce_set: vk::DescriptorSet,
    bounced_set: vk::DescriptorSet,
    reflection_set: vk::DescriptorSet,
    /// The sky dome's textures, bound at set 1; white when the scene has none.
    env_set: vk::DescriptorSet,
    sampler: vk::Sampler,
    refl_sampler: vk::Sampler,
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
            let reflect_fs = module(&compile(&fragment_source(REFLECT_MAIN, multisampled), naga::ShaderStage::Fragment));
            let blur_fs = module(&compile(&fragment_source(REFL_BLUR_MAIN, multisampled), naga::ShaderStage::Fragment));
            let bounce_blur_fs = module(&compile(&fragment_source(BOUNCE_BLUR_MAIN, multisampled), naga::ShaderStage::Fragment));
            let bindings = vec![
                vk::DescriptorSetLayoutBinding::default().binding(0).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT),
                vk::DescriptorSetLayoutBinding::default().binding(1).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT),
                vk::DescriptorSetLayoutBinding::default().binding(2).descriptor_type(vk::DescriptorType::SAMPLER).descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT),
                vk::DescriptorSetLayoutBinding::default().binding(3).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT),
                vk::DescriptorSetLayoutBinding::default().binding(4).descriptor_type(vk::DescriptorType::UNIFORM_BUFFER).descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT),
                vk::DescriptorSetLayoutBinding::default().binding(5).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT),
                vk::DescriptorSetLayoutBinding::default().binding(6).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT),
                vk::DescriptorSetLayoutBinding::default().binding(7).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT),
                vk::DescriptorSetLayoutBinding::default().binding(8).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT),
                vk::DescriptorSetLayoutBinding::default().binding(9).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT),
                vk::DescriptorSetLayoutBinding::default().binding(10).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT),
                vk::DescriptorSetLayoutBinding::default().binding(11).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT),
                vk::DescriptorSetLayoutBinding::default().binding(12).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT),
                vk::DescriptorSetLayoutBinding::default().binding(13).descriptor_type(vk::DescriptorType::SAMPLER).descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT),
                vk::DescriptorSetLayoutBinding::default().binding(14).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT),
            ];
            let set_layout = d.create_descriptor_set_layout(&vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings), None).unwrap();
            // The sky dome is a texture set of its own, bound at set 1, as the material pipelines bind it.
            let env_layout = crate::textures::set_layout(gpu);
            let sizes = [
                vk::DescriptorPoolSize::default().ty(vk::DescriptorType::SAMPLED_IMAGE).descriptor_count(64),
                vk::DescriptorPoolSize::default().ty(vk::DescriptorType::SAMPLER).descriptor_count(10),
                vk::DescriptorPoolSize::default().ty(vk::DescriptorType::UNIFORM_BUFFER).descriptor_count(5),
            ];
            let pool = d.create_descriptor_pool(&vk::DescriptorPoolCreateInfo::default().max_sets(5).pool_sizes(&sizes), None).unwrap();
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
            let refl_sampler = d
                .create_sampler(
                    &vk::SamplerCreateInfo::default()
                        .mag_filter(vk::Filter::LINEAR)
                        .min_filter(vk::Filter::LINEAR)
                        .mipmap_mode(vk::SamplerMipmapMode::LINEAR)
                        .min_lod(0.0)
                        .max_lod(5.0)
                        .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                        .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                        .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE),
                    None,
                )
                .unwrap();
            let params = gpu.buffer_with(
                vk::BufferUsageFlags::UNIFORM_BUFFER,
                &[PostParams {
                    proj: glam::Mat4::IDENTITY.to_cols_array(),
                    contact_dir: [0.0; 4],
                    contact: [0.0; 4],
                    quality: [0.0; 4],
                    dof: [0.0; 4],
                    view_to_world: glam::Mat4::IDENTITY.to_cols_array(),
                    env: [0.0; 4],
                    env_rot: [0.0, 0.0, 0.0, 1.0],
                    extra: [0.0; 4],
                    bg_env: [0.0; 4],
                }],
            );
            let push = [vk::PushConstantRange::default().stage_flags(vk::ShaderStageFlags::FRAGMENT).offset(0).size(std::mem::size_of::<PostPush>() as u32)];
            let sets = [set_layout, env_layout];
            let layout = d.create_pipeline_layout(&vk::PipelineLayoutCreateInfo::default().set_layouts(&sets).push_constant_ranges(&push), None).unwrap();
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
            // The reflection pass: the same triangle, its own shader, into the reflection image.
            let reflect_stages = [
                vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::VERTEX).module(vs).name(c"main"),
                vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::FRAGMENT).module(reflect_fs).name(c"main"),
            ];
            let reflect_info = info.stages(&reflect_stages).render_pass(gpu.ao_pass);
            let reflection_pipeline = d.create_graphics_pipelines(vk::PipelineCache::null(), &[reflect_info], None).map_err(|e| e.1).unwrap()[0];
            // The low-pass of that image: the same triangle, its own shader.
            let blur_stages = [
                vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::VERTEX).module(vs).name(c"main"),
                vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::FRAGMENT).module(blur_fs).name(c"main"),
            ];
            let blur_info = info.stages(&blur_stages).render_pass(gpu.ao_pass);
            let reflection_blur_pipeline = d.create_graphics_pipelines(vk::PipelineCache::null(), &[blur_info], None).map_err(|e| e.1).unwrap()[0];
            // The bounce light's low-pass, along one axis at a time.
            let bounce_blur_stages = [
                vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::VERTEX).module(vs).name(c"main"),
                vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::FRAGMENT).module(bounce_blur_fs).name(c"main"),
            ];
            let bounce_blur_info = info.stages(&bounce_blur_stages).render_pass(gpu.ao_pass);
            let bounce_blur_pipeline = d.create_graphics_pipelines(vk::PipelineCache::null(), &[bounce_blur_info], None).map_err(|e| e.1).unwrap()[0];
            // The occlusion and reflection passes can't read the image they write: their sets have another image there.
            let allocate = || d.allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo::default().descriptor_pool(pool).set_layouts(&layouts)).unwrap()[0];
            let ao_set = allocate();
            let bounce_set = allocate();
            let bounced_set = allocate();
            let reflection_set = allocate();
            let env_set = crate::textures::with_cache(gpu, |c| c.white(gpu));
            d.destroy_shader_module(vs, None);
            d.destroy_shader_module(fs, None);
            d.destroy_shader_module(ao_fs, None);
            d.destroy_shader_module(reflect_fs, None);
            d.destroy_shader_module(blur_fs, None);
            d.destroy_shader_module(bounce_blur_fs, None);
            Post { pipeline, ao_pipeline, reflection_pipeline, reflection_blur_pipeline, bounce_blur_pipeline, layout, set_layout, env_layout, pool, set, ao_set, bounce_set, bounced_set, reflection_set, env_set, sampler, refl_sampler, params }
        }
    }

    /// Points the sets at the scene's images, and at the sky dome the reflections fall back on. Call with no frame in
    /// flight (before the frame draws).
    pub fn bind_images(&mut self, gpu: &Gpu, env: Option<vk::DescriptorSet>) {
        if gpu.scene_view == vk::ImageView::null() || gpu.overlay_view == vk::ImageView::null() {
            return;
        }
        self.env_set = env.unwrap_or_else(|| crate::textures::with_cache(gpu, |c| c.white(gpu)));
        // Written each frame: a view remade at another size can come back under the handle the old one had.
        let color = [vk::DescriptorImageInfo::default().image_view(gpu.scene_view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
        let depth = [vk::DescriptorImageInfo::default().image_view(gpu.depth_view).image_layout(vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL)];
        let point = [vk::DescriptorImageInfo::default().sampler(self.sampler)];
        let open = [vk::DescriptorImageInfo::default().image_view(gpu.ao_view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
        let bounced = [vk::DescriptorImageInfo::default().image_view(gpu.bounce_view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
        let albedo = [vk::DescriptorImageInfo::default().image_view(gpu.albedo_view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
        let overlay = [vk::DescriptorImageInfo::default().image_view(gpu.overlay_view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
        let mat = [vk::DescriptorImageInfo::default().image_view(gpu.mat_view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
        let mat_aux = [vk::DescriptorImageInfo::default().image_view(gpu.mat_aux_view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
        let reflection = [vk::DescriptorImageInfo::default().image_view(gpu.refl_view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
        let planar = [vk::DescriptorImageInfo::default().image_view(gpu.planar_view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
        let back = [vk::DescriptorImageInfo::default().image_view(gpu.back_view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
        let refl_soft = [vk::DescriptorImageInfo::default().image_view(gpu.refl_chain_view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
        let refl_sampler = [vk::DescriptorImageInfo::default().sampler(self.refl_sampler)];
        let backface = [vk::DescriptorImageInfo::default().image_view(gpu.backface_view).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
        let params = [vk::DescriptorBufferInfo::default().buffer(self.params.buffer).offset(0).range(std::mem::size_of::<PostParams>() as u64)];
        let mut writes = Vec::new();
        for (set, last, refl) in [
            (self.set, &open, &reflection),
            (self.ao_set, &color, &reflection),
            (self.bounce_set, &open, &reflection),
            (self.bounced_set, &bounced, &reflection),
            // The reflection pass can't read the image it writes: its binding has the bounce image, which it never reads.
            (self.reflection_set, &open, &bounced),
        ] {
            writes.push(vk::WriteDescriptorSet::default().dst_set(set).dst_binding(0).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).image_info(&color));
            writes.push(vk::WriteDescriptorSet::default().dst_set(set).dst_binding(1).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).image_info(&depth));
            writes.push(vk::WriteDescriptorSet::default().dst_set(set).dst_binding(2).descriptor_type(vk::DescriptorType::SAMPLER).image_info(&point));
            writes.push(vk::WriteDescriptorSet::default().dst_set(set).dst_binding(3).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).image_info(last));
            writes.push(vk::WriteDescriptorSet::default().dst_set(set).dst_binding(4).descriptor_type(vk::DescriptorType::UNIFORM_BUFFER).buffer_info(&params));
            writes.push(vk::WriteDescriptorSet::default().dst_set(set).dst_binding(5).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).image_info(&albedo));
            writes.push(vk::WriteDescriptorSet::default().dst_set(set).dst_binding(6).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).image_info(&overlay));
            writes.push(vk::WriteDescriptorSet::default().dst_set(set).dst_binding(7).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).image_info(&mat));
            writes.push(vk::WriteDescriptorSet::default().dst_set(set).dst_binding(8).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).image_info(&mat_aux));
            writes.push(vk::WriteDescriptorSet::default().dst_set(set).dst_binding(9).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).image_info(refl));
            writes.push(vk::WriteDescriptorSet::default().dst_set(set).dst_binding(10).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).image_info(&planar));
            writes.push(vk::WriteDescriptorSet::default().dst_set(set).dst_binding(11).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).image_info(&back));
            writes.push(vk::WriteDescriptorSet::default().dst_set(set).dst_binding(12).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).image_info(&refl_soft));
            writes.push(vk::WriteDescriptorSet::default().dst_set(set).dst_binding(13).descriptor_type(vk::DescriptorType::SAMPLER).image_info(&refl_sampler));
            writes.push(vk::WriteDescriptorSet::default().dst_set(set).dst_binding(14).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).image_info(&backface));
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
            quality: [if s.high_quality { 1.0 } else { 0.0 }, s.bounce.max(0.0), if s.reflections { 1.0 } else { 0.0 }, s.debug_reflection as f32],
            dof: dof_params(proj, s, w, h),
            view_to_world: s.view.inverse().to_cols_array(),
            env: s.env,
            env_rot: s.env_rot,
            extra: [if s.planar_reflection { 1.0 } else { 0.0 }, s.up[0], s.up[1], s.up[2]],
            bg_env: s.bg_env,
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
            d.cmd_bind_descriptor_sets(gpu.cmd, vk::PipelineBindPoint::GRAPHICS, self.layout, 0, &[set, self.env_set], &[]);
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

    /// One axis of the bounce light's low-pass, between the two passes the view begins for it.
    pub fn draw_bounce_blur(&self, gpu: &Gpu, proj: Mat4, orthographic: bool, s: &PostSettings, horizontal: bool) {
        if s.bounce <= 0.0 {
            return;
        }
        let mut push = self.push(gpu, proj, orthographic, s);
        // bg_top is unused by this pass otherwise: the axis, then the Gaussian's sigma in pixels.
        push.bg_top = [if horizontal { 1.0 } else { 0.0 }, if horizontal { 0.0 } else { 1.0 }, 3.5, 0.0];
        let set = if horizontal { self.bounced_set } else { self.bounce_set };
        self.pass(gpu, self.bounce_blur_pipeline, set, &push);
    }

    /// The screen-space reflections and refraction. Between `Gpu::begin_reflection` and `Gpu::begin_occlusion`, with
    /// the Realtime renderer's material buffer.
    pub fn draw_reflection(&self, gpu: &Gpu, proj: Mat4, orthographic: bool, s: &PostSettings) {
        if s.reflections {
            self.pass(gpu, self.reflection_pipeline, self.reflection_set, &self.push(gpu, proj, orthographic, s));
        }
    }

    /// The low-pass of the reflection image. Between `Gpu::begin_refl_blur` and `Gpu::end_refl_blur`.
    pub fn draw_reflection_blur(&self, gpu: &Gpu, proj: Mat4, orthographic: bool, s: &PostSettings) {
        if s.reflections {
            self.pass(gpu, self.reflection_blur_pipeline, self.set, &self.push(gpu, proj, orthographic, s));
        }
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
            d.destroy_pipeline(self.reflection_pipeline, None);
            d.destroy_pipeline(self.reflection_blur_pipeline, None);
            d.destroy_pipeline(self.bounce_blur_pipeline, None);
            d.destroy_pipeline_layout(self.layout, None);
            d.destroy_descriptor_pool(self.pool, None);
            d.destroy_descriptor_set_layout(self.set_layout, None);
            d.destroy_descriptor_set_layout(self.env_layout, None);
            d.destroy_sampler(self.sampler, None);
            d.destroy_sampler(self.refl_sampler, None);
            d.destroy_buffer(self.params.buffer, None);
            if let Some(a) = self.params.alloc.take() {
                let _ = gpu.shared.allocator.lock().unwrap().free(a);
            }
        }
    }
}

/// The reflection of `i` about `n`, as the shader's `reflect`: i and n unit vectors, i towards the surface.
#[cfg(test)]
fn reflected(i: [f32; 3], n: [f32; 3]) -> [f32; 3] {
    let d = i[0] * n[0] + i[1] * n[1] + i[2] * n[2];
    [i[0] - 2.0 * d * n[0], i[1] - 2.0 * d * n[1], i[2] - 2.0 * d * n[2]]
}

/// The refracted direction into a medium of `eta = n1 / n2`, or None at total internal reflection, as GLSL `refract`.
#[cfg(test)]
fn refracted(i: [f32; 3], n: [f32; 3], eta: f32) -> Option<[f32; 3]> {
    let d = i[0] * n[0] + i[1] * n[1] + i[2] * n[2];
    let k = 1.0 - eta * eta * (1.0 - d * d);
    if k < 0.0 {
        return None;
    }
    let s = eta * d + k.sqrt();
    Some([eta * i[0] - s * n[0], eta * i[1] - s * n[1], eta * i[2] - s * n[2]])
}

/// The radius in pixels a roughness blurs a reflection over, as `reflection_at` gathers it.
#[cfg(test)]
fn roughness_pixels(rough: f32) -> f32 {
    (rough * rough * 120.0).clamp(8.0, 32.0)
}

/// How much a surface is the ground plane's reflection, as `planar_at`: 1 flat on the ground, 0 upright or off it.
/// The mip level the reflection gather samples for a roughness, as `reflection_at`.
#[cfg(test)]
fn reflection_lod(rough: f32) -> f32 {
    (roughness_pixels(rough).log2() - 2.0).clamp(1.0, 4.0)
}

#[cfg(test)]
fn planar_cover(up: f32, height: f32) -> f32 {
    fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
        let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    }
    smoothstep(0.86, 0.98, up) * (1.0 - smoothstep(0.04, 0.25, height.abs()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_post_shaders_compile_for_one_sample_and_for_several() {
        assert!(!compile(POST_VS, naga::ShaderStage::Vertex).is_empty());
        for multisampled in [false, true] {
            for main in [AO_MAIN, REFLECT_MAIN, REFL_BLUR_MAIN, COMPOSITE_MAIN] {
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

    /// Glass is two interfaces: the entrance and the far side, where the ray bends out again (or reflects at a grazing
    /// exit), and what the view cannot show is the dome rather than black.
    #[test]
    fn glass_refracts_at_both_of_its_interfaces() {
        assert!(REFLECT_MAIN.contains("vec3 dir_in = refract(v, n, 1.0 / ior);"));
        assert!(REFLECT_MAIN.contains("vec3 dir_out = refract(dir_in, -exit_n, ior);"));
        assert!(REFLECT_MAIN.contains("if (dot(dir_out, dir_out) < 1e-6) dir_out = reflect(dir_in, -exit_n);"));
        assert!(REFLECT_MAIN.contains("env_lookup(dir_out, rough)"));
        assert!(REFLECT_MAIN.contains("vec4 far_side = texelFetch(sampler2D(t_backface, s_point), at, 0);"));
        assert!(REFLECT_MAIN.contains("vec3 exit_p = p * (far_side.a / max(-p.z, 1e-6));"));
        assert!(REFLECT_MAIN.contains("behind = mix(env_lookup(reflect(v, n), rough), behind,"));
        // The old transmission-colour floor, which kept a dark cap but made the glass read flat, is gone.
        assert!(!REFLECT_MAIN.contains("tint * (0.3 *"));
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
        // The reflection pass reads the material, the planar image and the behind-glass image: they are what the shader
        // declares, and what the post sets bind.
        for b in [7, 8, 9, 10, 11, 12, 13, 14] {
            assert!(POST_HEAD.contains(&format!("binding = {b}")), "binding {b}");
        }
        assert!(REFLECT_MAIN.contains("t_mat_aux"));
        assert!(REFLECT_MAIN.contains("t_planar"));
        assert!(REFLECT_MAIN.contains("t_back"));
        assert!(REFL_BLUR_MAIN.contains("t_refl"));
        assert!(COMPOSITE_MAIN.contains("t_refl_soft"));
    }

    #[test]
    fn a_reflection_goes_where_the_mirror_says() {
        // Straight down onto a floor whose normal is +y reflects straight up.
        let r = reflected([0.0, -1.0, 0.0], [0.0, 1.0, 0.0]);
        assert!((r[0]).abs() < 1e-6 && (r[1] - 1.0).abs() < 1e-6 && r[2].abs() < 1e-6, "{r:?}");
        // At 45 degrees it keeps the sideways part and turns the normal part over, the same angle.
        let i = [0.7071068, -0.7071068, 0.0];
        let r = reflected(i, [0.0, 1.0, 0.0]);
        assert!((r[0] - 0.7071068).abs() < 1e-5 && (r[1] - 0.7071068).abs() < 1e-5 && r[2].abs() < 1e-5, "{r:?}");
        // The shader's own line is this one.
        assert!(REFLECT_MAIN.contains("vec3 r = reflect(v, n);"));
    }

    #[test]
    fn glass_bends_a_ray_towards_its_normal() {
        // 45 degrees into glass of IOR 1.5: sin(t) = sin(i) / 1.5, so the ray turns towards the normal.
        let i = [0.7071068, 0.0, -0.7071068];
        let r = refracted(i, [0.0, 0.0, 1.0], 1.0 / 1.5).unwrap();
        let angle = r[0].atan2(-r[2]).abs();
        assert!(angle < std::f32::consts::FRAC_PI_4 - 0.1, "{angle}");
        assert!((r[0] * r[0] + r[1] * r[1] + r[2] * r[2] - 1.0).abs() < 1e-5);
        assert!(REFLECT_MAIN.contains("vec3 dir_in = refract(v, n, 1.0 / ior);"));
    }

    #[test]
    fn a_rougher_metal_blurs_its_reflection_more() {
        assert_eq!(roughness_pixels(0.02), 8.0);
        assert!(roughness_pixels(0.3) < roughness_pixels(0.5));
        assert!(roughness_pixels(0.2) < roughness_pixels(0.5));
        assert_eq!(roughness_pixels(1.0), 32.0);
        // The composite's gather is the same radius, scaled to the view's height, with a pixel and a half at least at
        // any roughness.
        assert!(COMPOSITE_MAIN.contains("float res = max(pc.size.y / 720.0, 1.0);"));
        assert!(COMPOSITE_MAIN.contains("clamp(rough * rough * 120.0, 8.0, 32.0) * res;"));
        // The gather reads the filtered copy at a level that follows the roughness, and clamps bright taps.
        assert!(COMPOSITE_MAIN.contains("textureLod(sampler2D(t_refl_soft, s_refl)"));
        assert!(COMPOSITE_MAIN.contains("vec3(0.0), vec3(8.0)"));
        // The prefilter starts two levels sharper than the gather's radius: a mirror reads the half-resolution mip,
        // not an eighth.
        assert_eq!(reflection_lod(0.02), 1.0);
        assert!(reflection_lod(0.2) < reflection_lod(0.5));
        assert_eq!(reflection_lod(1.0), 3.0);
    }

    #[test]
    fn the_ground_plane_takes_the_planar_reflection() {
        assert_eq!(planar_cover(1.0, 0.0), 1.0);
        assert_eq!(planar_cover(0.0, 0.0), 0.0);
        assert_eq!(planar_cover(1.0, 1.0), 0.0);
        // Rounding a sphere's side off the plane, the planar image fades to the screen-space one.
        assert!(planar_cover(0.9, 0.0) > 0.0 && planar_cover(0.9, 0.0) < 1.0);
        assert!(REFLECT_MAIN.contains("smoothstep(0.86, 0.98, dot(wn, up))"));
    }

    #[test]
    fn the_examples_have_the_parity_suite() {
        // Each lighting scene opens through shotCam and has a reference render and a viewport capture beside it.
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for name in [
            "reflections",
            "metal_roughness_sweep",
            "dielectric_glass",
            "tinted_glass",
            "emissive_object",
            "area_lights",
            "environment_hdri",
            "checker_sphere",
        ] {
            let ma = root.join(format!("examples/rendering/{name}.ma"));
            assert!(ma.is_file(), "{}", ma.display());
            let text = std::fs::read_to_string(&ma).unwrap();
            assert!(text.contains("shotCam"), "{name} has a shot camera");
            assert!(root.join(format!("examples/rendering/reference/{name}.png")).is_file(), "{name} reference");
            assert!(root.join(format!("examples/rendering/viewport/{name}.png")).is_file(), "{name} viewport");
        }
    }
}
