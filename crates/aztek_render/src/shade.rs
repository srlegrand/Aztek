//! A material at a surface point: its network evaluated into the standard surface's parameters, the normal bent by
//! bump or normal map, and the choice between the two sides of a mix.

use glam::{Vec2, Vec3};

use crate::geom::SurfacePoint;
use crate::math::Pcg;
use crate::scene::Scene;
use crate::shader::{Input, Material, NormalMod, Params, ShadeCtx, Standard};

/// What a surface point turned out to be after mixes are resolved.
pub enum Resolved<'a> {
    Standard(&'a Standard),
    Flat { color: Vec3, opacity: Vec3 },
    ShadowMatte { shadow_color: Vec3, background: bool },
}

pub fn ctx_of(scene: &Scene, sp: &SurfacePoint, wo: Vec3, footprint: f32, time: f32) -> ShadeCtx {
    let inst = &scene.geo.instances[sp.inst as usize];
    let (_, inv) = inst.xform_at(time);
    ShadeCtx { p: sp.p, p_obj: inv.transform_point3(sp.p), n: sp.n, ng: sp.ng, uv: sp.uv, dpdu: sp.dpdu, dpdv: sp.dpdv, footprint, wo, front: sp.front, object_id: inst.id }
}

/// Picks one side of each mix by chance, in proportion to its share: unbiased on average, and cheaper than shading both.
pub fn resolve<'a>(scene: &'a Scene, mut id: u32, ctx: &ShadeCtx, rng: &mut Pcg) -> (Resolved<'a>, f32) {
    let mut weight = 1.0;
    for _ in 0..16 {
        let Some(m) = scene.materials.get(id as usize) else { break };
        match m {
            Material::Standard(s) => return (Resolved::Standard(s), weight),
            Material::Flat { color, opacity } => {
                let net = &scene.network;
                return (Resolved::Flat { color: net.eval(*color, ctx, &scene.textures), opacity: net.eval(*opacity, ctx, &scene.textures) }, weight);
            }
            Material::ShadowMatte { shadow_color, background } => return (Resolved::ShadowMatte { shadow_color: scene.network.eval(*shadow_color, ctx, &scene.textures), background: *background }, weight),
            Material::Mix { a, b, mix, add } => {
                if *add {
                    id = if rng.next_f32() < 0.5 { *a } else { *b };
                    weight *= 2.0;
                } else {
                    let t = scene.network.eval_f(*mix, ctx, &scene.textures).clamp(0.0, 1.0);
                    id = if rng.next_f32() < t { *b } else { *a };
                }
            }
        }
    }
    (Resolved::Standard(match &scene.materials[0] {
        Material::Standard(s) => s,
        _ => unreachable!("material 0 is the default standard surface"),
    }), weight)
}

fn bent_normal(scene: &Scene, m: &NormalMod, ctx: &ShadeCtx, n: Vec3) -> Vec3 {
    let net = &scene.network;
    let tex = &scene.textures;
    match *m {
        NormalMod::None => n,
        NormalMod::Bump { height, depth } => {
            if depth == 0.0 {
                return n;
            }
            let step = (ctx.footprint * 0.5).clamp(2e-4, 5e-3);
            let h0 = net.eval_f(height, ctx, tex);
            let mut cu = *ctx;
            cu.uv += Vec2::new(step, 0.0);
            cu.p += ctx.dpdu * step;
            let mut cv = *ctx;
            cv.uv += Vec2::new(0.0, step);
            cv.p += ctx.dpdv * step;
            let dhdu = (net.eval_f(height, &cu, tex) - h0) / step * depth;
            let dhdv = (net.eval_f(height, &cv, tex) - h0) / step * depth;
            // The displaced surface's tangents, crossed.
            let pu = ctx.dpdu + n * dhdu;
            let pv = ctx.dpdv + n * dhdv;
            let mut nn = pu.cross(pv).normalize_or_zero();
            if nn.dot(n) < 0.0 {
                nn = -nn;
            }
            if nn.length_squared() < 0.5 {
                n
            } else {
                nn
            }
        }
        NormalMod::Map { map, strength, flip_r, flip_g, swap, tangent, signed } => {
            let c = net.eval(map, ctx, tex);
            let mut t = if signed { c } else { c * 2.0 - Vec3::ONE };
            if flip_r {
                t.x = -t.x;
            }
            if flip_g {
                t.y = -t.y;
            }
            if swap {
                t = Vec3::new(t.y, t.x, t.z);
            }
            t = Vec3::new(t.x * strength, t.y * strength, t.z.max(1e-3));
            if !tangent {
                let nn = t.normalize_or_zero();
                return if nn.length_squared() < 0.5 { n } else { nn };
            }
            let tu = (ctx.dpdu - n * n.dot(ctx.dpdu)).normalize_or_zero();
            if tu.length_squared() < 0.5 {
                return n;
            }
            let mut b = n.cross(tu);
            if b.dot(ctx.dpdv) < 0.0 {
                b = -b;
            }
            let mut nn = (tu * t.x + b * t.y + n * t.z).normalize_or_zero();
            if nn.length_squared() < 0.5 {
                return n;
            }
            // Keep the bent normal on the viewer's side, so the surface doesn't go black at grazing angles.
            if nn.dot(ctx.wo) < 0.0 {
                nn = (nn - ctx.wo * (nn.dot(ctx.wo) - 1e-3)).normalize();
            }
            nn
        }
    }
}

/// The standard surface's parameters at a point.
pub fn params(scene: &Scene, s: &Standard, ctx: &ShadeCtx, ao: f32) -> Params {
    let net = &scene.network;
    let tex = &scene.textures;
    let mut c = *ctx;
    // An occlusion or curvature node reads its result from here, computed by the integrator.
    c.footprint = ctx.footprint;
    let traced = |i: Input| -> Option<Vec3> {
        let n = net.traced(i)?;
        if let crate::shader::Node::Traced { white, black, .. } = &net.nodes[n as usize] {
            let (w, b) = (net.eval(*white, &c, tex), net.eval(*black, &c, tex));
            return Some(b.lerp(w, ao));
        }
        None
    };
    let v = |i: Input| traced(i).unwrap_or_else(|| net.eval(i, &c, tex));
    let f = |i: Input| match i {
        Input::Const(x) => x.x,
        _ => v(i).x,
    };
    let n = bent_normal(scene, &s.normal, ctx, ctx.n);
    let coat = f(s.coat);
    let coat_n = if coat > 0.0 { bent_normal(scene, &s.coat_normal, ctx, ctx.n) } else { n };
    let tangent = (ctx.dpdu - n * n.dot(ctx.dpdu)).normalize_or_zero();
    Params {
        base: f(s.base),
        base_color: v(s.base_color).max(Vec3::ZERO),
        diffuse_roughness: f(s.diffuse_roughness),
        metalness: f(s.metalness),
        specular: f(s.specular),
        specular_color: v(s.specular_color).max(Vec3::ZERO),
        specular_roughness: f(s.specular_roughness).clamp(0.0, 1.0),
        specular_ior: f(s.specular_ior),
        specular_anisotropy: f(s.specular_anisotropy).clamp(0.0, 1.0),
        specular_rotation: f(s.specular_rotation),
        transmission: f(s.transmission),
        transmission_color: v(s.transmission_color).max(Vec3::ZERO),
        transmission_depth: f(s.transmission_depth),
        transmission_scatter: v(s.transmission_scatter).max(Vec3::ZERO),
        transmission_scatter_anisotropy: f(s.transmission_scatter_anisotropy),
        transmission_extra_roughness: f(s.transmission_extra_roughness),
        subsurface: if scene.options.ignore_sss { 0.0 } else { f(s.subsurface) },
        subsurface_color: v(s.subsurface_color).max(Vec3::ZERO),
        subsurface_radius: v(s.subsurface_radius).max(Vec3::splat(1e-4)),
        subsurface_scale: f(s.subsurface_scale),
        subsurface_anisotropy: f(s.subsurface_anisotropy),
        sheen: f(s.sheen),
        sheen_color: v(s.sheen_color).max(Vec3::ZERO),
        sheen_roughness: f(s.sheen_roughness),
        coat,
        coat_color: v(s.coat_color).max(Vec3::ZERO),
        coat_roughness: f(s.coat_roughness).clamp(0.0, 1.0),
        coat_anisotropy: f(s.coat_anisotropy),
        coat_rotation: f(s.coat_rotation),
        coat_ior: f(s.coat_ior),
        coat_affect_color: f(s.coat_affect_color),
        coat_affect_roughness: f(s.coat_affect_roughness),
        thin_film_thickness: f(s.thin_film_thickness),
        thin_film_ior: f(s.thin_film_ior),
        emission: v(s.emission_color).max(Vec3::ZERO) * f(s.emission),
        opacity: v(s.opacity).clamp(Vec3::ZERO, Vec3::ONE),
        thin_walled: s.thin_walled,
        n,
        coat_n,
        tangent: if tangent.length_squared() < 0.5 { crate::math::Frame::from_n(n).t } else { tangent },
    }
}

/// Opacity alone, for shadow rays and transparency, without the rest of the material.
pub fn opacity(scene: &Scene, mat: u32, ctx: &ShadeCtx, rng: &mut Pcg) -> Vec3 {
    let (r, _) = resolve(scene, mat, ctx, rng);
    match r {
        Resolved::Standard(s) => match s.opacity {
            Input::Const(o) => o,
            i => scene.network.eval(i, ctx, &scene.textures).clamp(Vec3::ZERO, Vec3::ONE),
        },
        Resolved::Flat { opacity, .. } => opacity,
        Resolved::ShadowMatte { .. } => Vec3::ZERO,
    }
}
