//! Sculpting Tools: brushes that push a mesh's points about. Sculpt, Smooth, Relax, Grab, Pinch, Flatten, Foamy, Spray,
//! Repeat, Imprint, Wax, Scrape, Fill, Knife, Smear, Bulge, Amplify and Freeze, with the settings of the reference's
//! sculptMeshCacheCtx. A stroke's points go in as one polyTweak step.

use std::collections::HashMap;

use aztek_geo::Mesh;
use aztek_ops::{fail, objects, ArgType, CmdResult, CommandDef, Commands, FlagDef};
use glam::{DMat4, DVec3};
use serde_json::{json, Value};

use crate::components;
use crate::plug::NodeId;
use crate::session::Session;

#[derive(Clone, Debug)]
pub struct SculptCtx {
    pub tool: String,
    /// Brush radius, world units.
    pub size: f64,
    /// 0 to 1.
    pub strength: f64,
    pub invert: bool,
    /// Mirror across the object's YZ plane.
    pub symmetry: bool,
    /// Per mesh, per point: 1 frozen.
    pub frozen: HashMap<NodeId, Vec<f32>>,
    /// The mesh's own symmetry, for this stroke: its plane rather than the object's YZ.
    pub mirror: Option<aztek_geo::symmetry::SymmetrySpec>,
}

impl Default for SculptCtx {
    fn default() -> Self {
        SculptCtx { tool: "sculpt".into(), size: 1.0, strength: 0.25, invert: false, symmetry: false, mirror: None, frozen: HashMap::new() }
    }
}

pub const TOOLS: &[&str] = &["sculpt", "smooth", "relax", "grab", "pinch", "flatten", "foamy", "spray", "repeat", "imprint", "wax", "scrape", "fill", "knife", "smear", "bulge", "amplify", "freeze"];

/// A smooth falloff: 1 at the centre, 0 at the rim.
fn falloff(t: f64) -> f64 {
    if t >= 1.0 {
        0.0
    } else {
        let u = 1.0 - t * t;
        u * u * (3.0 - 2.0 * u).max(0.0)
    }
}

fn neighbour_average(m: &Mesh, pos: &[DVec3], p: u32) -> Option<DVec3> {
    let mut sum = DVec3::ZERO;
    let mut n = 0.0;
    m.for_each_point_edge(p, |e| {
        sum += pos[m.edge_other_point(e, p) as usize];
        n += 1.0;
    });
    (n > 0.0).then(|| sum / n)
}

fn point_normal(m: &Mesh, pos: &[DVec3], p: u32) -> DVec3 {
    let mut n = DVec3::ZERO;
    for f in m.point_faces(p) {
        let pts = m.face_points(f);
        for i in 0..pts.len() {
            let (a, b) = (pos[pts[i] as usize], pos[pts[(i + 1) % pts.len()] as usize]);
            n += a.cross(b);
        }
    }
    n.normalize_or_zero()
}

/// One dab on object-space positions. `center` and `normal` are world; `pull` is the world-space drag since the last dab
/// (Grab and Smear). `base` holds the positions at the stroke's start for the brushes that work from them.
#[allow(clippy::too_many_arguments)]
pub fn dab(pos: &mut [DVec3], base: &[DVec3], m: &Mesh, world: &DMat4, center: DVec3, normal: DVec3, pull: DVec3, ctx: &SculptCtx, frozen: Option<&[f32]>, invert_key: bool, smooth_key: bool) -> usize {
    let inv = world.inverse();
    let c = inv.transform_point3(center);
    let n = inv.transform_vector3(normal).normalize_or_zero();
    let radius = ctx.size / world.transform_vector3(DVec3::X).length().max(1e-9);
    let n1 = dab_one(pos, base, m, c, n, inv.transform_vector3(pull), radius, ctx, frozen, invert_key, smooth_key);
    if let Some(spec) = &ctx.mirror {
        let pull_m = spec.apply_vector(1, inv.transform_vector3(pull));
        dab_one(pos, base, m, spec.apply(1, c), spec.apply_vector(1, n), pull_m, radius, ctx, frozen, invert_key, smooth_key);
    } else if ctx.symmetry {
        let flip = DVec3::new(-1.0, 1.0, 1.0);
        let pull_m = inv.transform_vector3(pull) * flip;
        dab_one(pos, base, m, c * flip, n * flip, pull_m, radius, ctx, frozen, invert_key, smooth_key);
    }
    n1
}

#[allow(clippy::too_many_arguments)]
fn dab_one(pos: &mut [DVec3], base: &[DVec3], m: &Mesh, center: DVec3, normal: DVec3, pull: DVec3, radius: f64, ctx: &SculptCtx, frozen: Option<&[f32]>, invert_key: bool, smooth_key: bool) -> usize {
    // Shift gives the Smooth brush in any tool, as in the reference.
    let tool = if smooth_key { "smooth" } else { ctx.tool.as_str() };
    let sign = if ctx.invert != invert_key { -1.0 } else { 1.0 };
    let strength = ctx.strength;
    let mut hits = Vec::new();
    let mut avg_pos = DVec3::ZERO;
    let mut avg_n = DVec3::ZERO;
    let mut total = 0.0;
    for p in 0..m.point_capacity() {
        if !m.point_valid(p) {
            continue;
        }
        let d = (pos[p as usize] - center).length();
        if d >= radius {
            continue;
        }
        let w = falloff(d / radius) * (1.0 - frozen.and_then(|f| f.get(p as usize)).copied().unwrap_or(0.0) as f64);
        if w <= 0.0 {
            continue;
        }
        hits.push((p, w));
        avg_pos += pos[p as usize] * w;
        avg_n += point_normal(m, pos, p) * w;
        total += w;
    }
    if hits.is_empty() {
        return 0;
    }
    let plane = avg_pos / total;
    let pn = if avg_n.length() > 1e-9 { avg_n.normalize() } else { normal };
    let snapshot: Vec<DVec3> = hits.iter().map(|&(p, _)| pos[p as usize]).collect();
    let step = radius * strength * 0.25 * sign;
    for (k, &(p, w)) in hits.iter().enumerate() {
        let here = snapshot[k];
        let delta = match tool {
            "sculpt" | "foamy" | "spray" => normal * step * w,
            "bulge" => point_normal(m, pos, p) * step * w,
            "smooth" | "relax" => {
                let t = neighbour_average(m, pos, p).unwrap_or(here) - here;
                if tool == "relax" {
                    let nrm = point_normal(m, pos, p);
                    (t - nrm * t.dot(nrm)) * strength * w
                } else {
                    t * strength * w
                }
            }
            "grab" | "smear" => pull * w,
            "pinch" => {
                let to = center - here;
                (to - normal * to.dot(normal)) * strength * 0.25 * w * sign
            }
            "flatten" => -pn * (here - plane).dot(pn) * strength * w * sign,
            // Only what's below (Fill, Wax) or above (Scrape) the plane through the dab moves.
            "fill" | "wax" => {
                let h = (here - plane).dot(pn);
                if h < 0.0 {
                    -pn * h * strength * w
                } else if tool == "wax" {
                    pn * step * 0.25 * w
                } else {
                    DVec3::ZERO
                }
            }
            "scrape" => {
                let h = (here - plane).dot(pn);
                if h > 0.0 {
                    -pn * h * strength * w
                } else {
                    DVec3::ZERO
                }
            }
            // A crease along the stroke.
            "knife" => {
                let to = center - here;
                let side = to - normal * to.dot(normal);
                side * strength * 0.5 * w - normal * step * w * w
            }
            "imprint" => normal * step * if w > 0.5 { 1.0 } else { 0.0 },
            "repeat" => normal * step * (0.5 + 0.5 * (here.dot(DVec3::ONE) * 8.0 / radius).sin()) * w,
            // Away from the neighbours' average: details stand out.
            "amplify" => (here - neighbour_average(m, pos, p).unwrap_or(here)) * strength * w * sign,
            _ => DVec3::ZERO,
        };
        pos[p as usize] = here + delta;
    }
    let _ = base;
    hits.len()
}

/// Paints the Freeze mask: what's frozen doesn't move under the other brushes.
pub fn paint_freeze(frozen: &mut Vec<f32>, m: &Mesh, world: &DMat4, center: DVec3, ctx: &SculptCtx, unfreeze: bool) {
    frozen.resize(m.point_capacity() as usize, 0.0);
    for p in 0..m.point_capacity() {
        if !m.point_valid(p) {
            continue;
        }
        let d = (world.transform_point3(m.position(p)) - center).length();
        if d < ctx.size {
            let w = (falloff(d / ctx.size) * ctx.strength * 2.0) as f32;
            let f = &mut frozen[p as usize];
            *f = if unfreeze { (*f - w).max(0.0) } else { (*f + w).min(1.0) };
        }
    }
}

/// The stroke's points written as one undoable step.
pub fn commit(s: &mut Session, shape: NodeId, pos: &[DVec3], m: &Mesh) -> CmdResult<()> {
    let moves: Vec<(u32, DVec3)> = (0..m.point_capacity()).filter(|&p| m.point_valid(p)).map(|p| (p, pos[p as usize])).collect();
    crate::mesh_edit::transform::set_points(&mut s.graph, shape, &moves)
}

type Cmd = CommandDef<Session>;

fn f(name: &str, short: &str, ty: ArgType, doc: &str) -> FlagDef {
    FlagDef::new(name, short, ty, Value::Null, doc)
}

pub fn register(c: &mut Commands<Session>) {
    c.register(
        Cmd::new(
            "sculptMeshCacheCtx",
            "The sculpting tools' settings. -st the brush (sculpt, smooth, relax, grab, pinch, flatten, foamy, spray, repeat, imprint, wax, \
             scrape, fill, knife, smear, bulge, amplify, freeze), -sz size, -str strength, -inv invert, -sym symmetry, -q to ask. \
             -cf clears the freeze.",
            |s, a| {
                let asked = |n: &str| a.bool(n) || a.get("queried").and_then(Value::as_array).is_some_and(|q| q.iter().any(|x| x == n));
                if a.bool("query") {
                    let c = &s.sculpt_ctx;
                    if asked("sculptTool") {
                        return Ok(json!(c.tool));
                    }
                    if asked("size") {
                        return Ok(json!(c.size));
                    }
                    if asked("strength") {
                        return Ok(json!(c.strength));
                    }
                    if asked("invert") {
                        return Ok(json!(c.invert));
                    }
                    if asked("symmetry") {
                        return Ok(json!(c.symmetry));
                    }
                    return Ok(Value::Null);
                }
                if let Some(t) = a.get("sculptTool").and_then(Value::as_str) {
                    if !TOOLS.contains(&t) {
                        fail!("sculptMeshCacheCtx: unknown brush {t}");
                    }
                    s.sculpt_ctx.tool = t.to_string();
                }
                if a.has("size") {
                    s.sculpt_ctx.size = a.float("size", 1.0).max(1e-4);
                }
                if a.has("strength") {
                    s.sculpt_ctx.strength = a.float("strength", 0.25).clamp(0.0, 1.0);
                }
                if a.has("invert") {
                    s.sculpt_ctx.invert = a.bool("invert");
                }
                if a.has("symmetry") {
                    s.sculpt_ctx.symmetry = a.bool("symmetry");
                }
                if a.bool("clearFreeze") {
                    s.sculpt_ctx.frozen.clear();
                }
                Ok(Value::Null)
            },
        )
        .undoable(false)
        .positional("the tool's name")
        .flag(f("sculptTool", "st", ArgType::String, "The brush."))
        .flag(f("size", "sz", ArgType::Float, "Brush radius, world units."))
        .flag(f("strength", "str", ArgType::Float, "Strength, 0 to 1."))
        .flag(f("invert", "inv", ArgType::Bool, "Push the other way."))
        .flag(f("symmetry", "sym", ArgType::Bool, "Mirror across the object's YZ plane."))
        .flag(f("clearFreeze", "cf", ArgType::Bool, "Unfreeze everything."))
        .flag(f("query", "q", ArgType::Bool, "Query mode."))
        .flag(f("edit", "e", ArgType::Bool, "Edit mode."))
        .flag(f("queried", "", ArgType::List, "Flags asked about in query mode.")),
    );

    c.register(
        Cmd::new(
            "sculptStamp",
            "One dab of the sculpting brush at a world point on a mesh: for scripts and tests. -p the point, -n the surface normal, -d the \
             drag for Grab and Smear. Returns how many points it moved.",
            |s, a| {
                let names = objects(s, a);
                let Some(n) = names.first() else { fail!("sculptStamp: name a mesh") };
                let Some(id) = s.graph.find(n)? else { fail!("sculptStamp: no object named {n}") };
                let Some(shape) = components::as_mesh(&s.graph, id) else { fail!("sculptStamp: {n} is not a mesh") };
                let Some(at) = a.vec3("position") else { fail!("sculptStamp: -p x y z") };
                s.graph.evaluate();
                let Some(m) = crate::modifiers::cage(&mut s.graph, shape) else { fail!("sculptStamp: no mesh") };
                let xf = crate::dag::transform_of(&s.graph, shape);
                let world = crate::rig::world(&mut s.graph, xf);
                let ctx = s.sculpt_ctx.clone();
                let nrm = a.vec3("normal").map(DVec3::from_array).unwrap_or(DVec3::Y);
                let drag = a.vec3("drag").map(DVec3::from_array).unwrap_or(DVec3::ZERO);
                let center = DVec3::from_array(at);
                if ctx.tool == "freeze" {
                    let entry = s.sculpt_ctx.frozen.entry(shape).or_default();
                    let mut fr = std::mem::take(entry);
                    paint_freeze(&mut fr, &m, &world, center, &ctx, a.bool("invert"));
                    s.sculpt_ctx.frozen.insert(shape, fr);
                    return Ok(json!(0));
                }
                let mut pos = m.positions().to_vec();
                let base = pos.clone();
                let frozen = ctx.frozen.get(&shape).map(|v| v.as_slice());
                let n2 = dab(&mut pos, &base, &m, &world, center, nrm, drag, &ctx, frozen, a.bool("invert"), a.bool("smooth"));
                commit(s, shape, &pos, &m)?;
                Ok(json!(n2))
            },
        )
        .positional("a mesh")
        .flag(f("position", "p", ArgType::Vec3, "World centre."))
        .flag(f("normal", "n", ArgType::Vec3, "World normal."))
        .flag(f("drag", "d", ArgType::Vec3, "World drag since the last dab."))
        .flag(f("invert", "inv", ArgType::Bool, "As Ctrl."))
        .flag(f("smooth", "sm", ArgType::Bool, "As Shift.")),
    );
}
