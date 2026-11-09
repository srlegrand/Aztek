//! Stepping the solvers with time. Each solver node holds an `aztek_dyn::Solver`; an evaluate hook, before the graph
//! computes, reads what changed, steps, and hands each body its result. As the old systems do: nothing is solved going
//! backwards; going to the start frame starts again; a jump forward solves one frame. Frames already solved are kept in
//! memory, so scrubbing back shows them; a disk cache (`dynCache`) plays instead of solving where it has the frame.

use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use aztek_dyn::{BodySettings, Emitter, EmitterKind, Fallback, Field, FieldKind, Lifespan, RigidShape, Solver, SolverSettings};
use glam::{DMat4, DVec3};

use crate::dynamics::{self as dy, ClothNode, ClothOut, ParticleState, ParticlesNode, RigidNode, SolverNode};
use crate::graph::Graph;
use crate::plug::{NodeId, PlugRef, PlugValue};

/// Solver nodes ever made: nothing to do until one is.
pub(crate) static SOLVERS_MADE: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Cloth,
    Particles,
    Rigid,
    /// A passive rigid body: collides like a collider.
    Passive,
    Collider,
}

#[derive(Clone, Debug)]
pub struct BodyRef {
    pub node: NodeId,
    pub kind: Kind,
    /// Its index in the solver.
    pub index: usize,
}

#[derive(Clone)]
pub enum Out {
    Cloth(Option<ClothOut>),
    Particles(Arc<ParticleState>),
    Rigid(Option<DMat4>),
}

pub type Frame = Vec<(NodeId, Out)>;

#[derive(Default)]
pub struct State {
    pub solver: Option<Solver>,
    /// What the solver was built from: body, kind, point count, topology.
    pub key: Vec<(NodeId, Kind, usize, u64)>,
    pub bodies: Vec<BodyRef>,
    /// The frame the solver's state is at.
    pub last: Option<f64>,
    /// Solved frames, by frame times 1000.
    pub cache: BTreeMap<i64, Arc<Frame>>,
    pub out_count: usize,
    pub solve_ms: f64,
    /// Disk caches read, by path.
    pub disk: HashMap<String, Arc<aztek_dyn::cache::Cache>>,
    /// What the bodies were last given: frame and whether it was a fresh result. Unchanged, nothing is given again,
    /// so edits elsewhere do not recompute the bodies.
    pub shown: Option<(i64, usize)>,
    pub results: usize,
}

/// A hash of which points each face uses: the same for a mesh rebuilt the same way, as a smooth under a skin is
/// every frame, where its topology id is new each time.
fn connectivity(m: &aztek_geo::Mesh) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325 ^ m.point_capacity() as u64;
    for f in 0..m.face_capacity() {
        if !m.face_valid(f) {
            continue;
        }
        m.for_each_face_corner(f, |c| {
            h = (h ^ m.corner_point(c) as u64).wrapping_mul(0x100_0000_01b3);
        });
        h = (h ^ 0xff).wrapping_mul(0x100_0000_01b3);
    }
    h
}

fn key(frame: f64) -> i64 {
    (frame * 1000.0).round() as i64
}

pub fn install(g: &mut Graph) {
    g.add_evaluate_hook(Arc::new(|g: &mut Graph, after: bool| {
        if after || SOLVERS_MADE.load(Ordering::Relaxed) == 0 {
            return;
        }
        for s in solvers(g) {
            run(g, s);
        }
    }));
}

pub fn solvers(g: &Graph) -> Vec<NodeId> {
    g.nodes().into_iter().filter(|&id| g.node(id).is_some_and(|n| n.ty().name == dy::SOLVER)).collect()
}

fn attr(g: &Graph, id: NodeId, name: &str) -> Option<PlugValue> {
    let n = g.node(id)?;
    n.ty().attr_index(name).map(|a| n.value(a).clone())
}

pub(crate) fn num(g: &Graph, id: NodeId, name: &str, default: f64) -> f64 {
    attr(g, id, name).and_then(|v| v.scalar()).filter(|x| x.is_finite()).unwrap_or(default)
}

fn flag(g: &Graph, id: NodeId, name: &str, default: bool) -> bool {
    attr(g, id, name).map_or(default, |v| v.as_bool())
}

fn vec(g: &Graph, id: NodeId, name: &str, default: DVec3) -> DVec3 {
    attr(g, id, name).map_or(default, |v| v.as_vec3())
}

fn text(g: &Graph, id: NodeId, name: &str) -> String {
    attr(g, id, name).map(|v| v.as_str().to_string()).unwrap_or_default()
}

fn floats(g: &Graph, id: NodeId, name: &str) -> Option<Vec<f64>> {
    let n = g.node(id)?;
    let a = n.ty().attr_index(name)?;
    crate::deform::floats_of(n.data(), a).map(|f| f.0.clone())
}

/// What arrives at an input: its source's value, evaluated, or the stored value.
fn pulled(g: &mut Graph, id: NodeId, name: &str) -> Option<PlugValue> {
    let a = g.node(id)?.ty().attr_index(name)?;
    match g.source(&PlugRef::new(id, a as i32)) {
        Some(src) => g.value(&src).ok(),
        None => Some(g.node(id)?.value(a).clone()),
    }
}

/// The bodies and constraints connected to a solver, by node, in creation order.
pub fn members(g: &Graph, s: NodeId) -> Vec<NodeId> {
    let Some(msg) = g.node(s).and_then(|n| n.ty().attr_index("message")) else { return Vec::new() };
    let mut out: Vec<NodeId> = g
        .destinations(&PlugRef::new(s, msg as i32))
        .into_iter()
        .filter(|d| g.attr(d).name == "solver")
        .map(|d| d.node)
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

fn kind_of(g: &Graph, id: NodeId) -> Option<Kind> {
    match g.node(id)?.ty().name.as_str() {
        dy::CLOTH => Some(Kind::Cloth),
        dy::PARTICLES => Some(Kind::Particles),
        dy::COLLIDER => Some(Kind::Collider),
        dy::RIGID => Some(if flag(g, id, "active", true) { Kind::Rigid } else { Kind::Passive }),
        _ => None,
    }
}

/// A body's mesh in world space now: (points, faces, topology id).
/// A matrix with its translation in metres.
fn to_metres(mx: DMat4, mpu: f64) -> DMat4 {
    let mut r = mx;
    r.w_axis = (mx.w_axis.truncate() * mpu).extend(1.0);
    r
}

/// A body's mesh in metres now (scene units times `mpu`): (points, faces, connectivity, its world matrix in scene units).
fn world_mesh(g: &mut Graph, id: NodeId, kind: Kind, mpu: f64) -> Option<(Vec<DVec3>, Vec<Vec<u32>>, u64, DMat4)> {
    let (mesh, matrix) = match kind {
        Kind::Cloth => {
            let m = pulled(g, id, "inputPolymesh")?.as_mesh()?.clone();
            (m, pulled(g, id, "geomMatrix").map_or(DMat4::IDENTITY, |v| v.as_matrix()))
        }
        Kind::Collider => {
            let m = pulled(g, id, "inputMesh")?.as_mesh()?.clone();
            (m, pulled(g, id, "inputMatrix").map_or(DMat4::IDENTITY, |v| v.as_matrix()))
        }
        Kind::Rigid | Kind::Passive => {
            let xf = rigid_transform(g, id)?;
            // Read from a file, it holds no mesh: the object's own.
            let m = match pulled(g, id, "inputMesh").and_then(|v| v.as_mesh().cloned()) {
                Some(m) => m,
                None => {
                    let shape = crate::components::as_mesh(g, xf)?;
                    let out = g.node(shape)?.ty().attr_index("outMesh")?;
                    g.value(&PlugRef::new(shape, out as i32)).ok()?.as_mesh()?.clone()
                }
            };
            (m, crate::rig::world(g, xf))
        }
        Kind::Particles => return None,
    };
    let (points, faces) = dy::mesh_parts(&mesh);
    let world = points.iter().map(|p| matrix.transform_point3(*p) * mpu).collect();
    Some((world, faces, connectivity(&mesh), matrix))
}

/// The transform a rigid body drives: named by its object attribute, not connected, since it drives that transform.
pub(crate) fn rigid_transform(g: &Graph, id: NodeId) -> Option<NodeId> {
    g.find(&text(g, id, "object")).ok().flatten()
}

/// The material in physical units: stretch and compression resistance 1 is 100 N/m, bend resistance 1 is 1e-4 N m,
/// drag 0.05 is a drag coefficient of 1 (a plate across the flow). Lengths in scene units become metres by `m`.
fn cloth_settings(g: &Graph, id: NodeId, m: f64) -> BodySettings {
    let th = num(g, id, "thickness", 0.1) * m;
    BodySettings {
        enabled: flag(g, id, "isDynamic", true),
        mass: num(g, id, "pointMass", 1.0),
        density: num(g, id, "density", 0.15),
        thickness: th,
        self_thickness: th * num(g, id, "selfCollideWidthScale", 1.0),
        stretch_stiffness: num(g, id, "stretchResistance", 20.0).max(0.0) * 100.0,
        compression_stiffness: num(g, id, "compressionResistance", 10.0).max(0.0) * 100.0,
        bend_stiffness: num(g, id, "bendResistance", 0.1).max(0.0) * 1e-4,
        area_stiffness: Some(num(g, id, "deformResistance", 0.0)).filter(|&r| r > 0.0).map(|r| r * 100.0),
        damping: num(g, id, "damp", 0.0),
        friction: num(g, id, "friction", 0.1),
        drag: num(g, id, "drag", 0.05) * 20.0,
        tangential_drag: num(g, id, "tangentialDrag", 0.0) * 20.0,
        attract: num(g, id, "inputMeshAttract", 0.0),
        attract_map: floats(g, id, "inputAttractPerVertex"),
        pin_map: floats(g, id, "pinPerVertex"),
        mass_map: floats(g, id, "massPerVertex"),
        collide: flag(g, id, "collide", true),
        self_collide: flag(g, id, "selfCollide", true),
        rest_from_input: flag(g, id, "restFromInput", false),
        tear_ratio: num(g, id, "tearRatio", 0.0),
        gravity_scale: num(g, id, "gravityScale", 1.0),
        bounce: num(g, id, "bounce", 0.0),
        ..resolve_settings(g, id)
    }
}

fn resolve_settings(g: &Graph, id: NodeId) -> BodySettings {
    BodySettings {
        resolve: flag(g, id, "resolveImpossible", true),
        fallback: match num(g, id, "fallback", 0.0) as i64 {
            1 => Fallback::Animation,
            2 => Fallback::Neighbours,
            _ => Fallback::Auto,
        },
        blend_frames: num(g, id, "blendFrames", 4.0),
        return_frames: num(g, id, "returnFrames", 4.0).max(1.0) as u32,
        most_frames_out: num(g, id, "mostFramesOut", 48.0).max(1.0) as u32,
        stretch_limit: num(g, id, "stretchLimit", 3.0),
        ..Default::default()
    }
}

fn particle_settings(g: &Graph, id: NodeId, m: f64) -> BodySettings {
    let most = num(g, id, "maxCount", -1.0);
    let r = num(g, id, "radius", 0.1) * m;
    BodySettings {
        enabled: flag(g, id, "isDynamic", true),
        mass: num(g, id, "pointMass", 1.0),
        density: 0.0,
        radius: r,
        thickness: r,
        self_thickness: r,
        friction: num(g, id, "friction", 0.1),
        bounce: num(g, id, "bounce", 0.0),
        damping: num(g, id, "damp", 0.0),
        drag: num(g, id, "drag", 0.0) * 20.0,
        collide: flag(g, id, "collide", true),
        self_collide: flag(g, id, "selfCollide", false),
        gravity_scale: num(g, id, "gravityScale", 1.0),
        lifespan_mode: match num(g, id, "lifespanMode", 0.0) as i64 {
            1 => Lifespan::Constant,
            2 => Lifespan::Random,
            3 => Lifespan::PerParticle,
            _ => Lifespan::Forever,
        },
        lifespan: num(g, id, "lifespan", 1.0),
        lifespan_random: num(g, id, "lifespanRandom", 0.0),
        max_count: if most < 0.0 { 10_000_000 } else { most as usize },
        ..resolve_settings(g, id)
    }
}

fn rigid_settings(g: &Graph, id: NodeId, m: f64) -> BodySettings {
    BodySettings {
        enabled: flag(g, id, "isDynamic", true),
        mass: num(g, id, "mass", 1.0),
        friction: 0.5 * (num(g, id, "staticFriction", 0.2) + num(g, id, "dynamicFriction", 0.2)),
        bounce: num(g, id, "bounciness", 0.6),
        thickness: 0.0,
        shape: match num(g, id, "standInObject", 0.0) as i64 {
            1 => RigidShape::Box,
            2 => RigidShape::Sphere,
            3 => RigidShape::Capsule,
            _ => RigidShape::Mesh,
        },
        resolution: num(g, id, "particleResolution", 6.0).max(2.0) as usize,
        initial_velocity: vec(g, id, "initialVelocity", DVec3::ZERO) * m,
        initial_spin: vec(g, id, "initialSpin", DVec3::ZERO) * std::f64::consts::PI / 180.0,
        gravity_scale: 1.0,
        drag: 0.0,
        density: 0.0,
        resolve: false,
        ..Default::default()
    }
}

fn collider_settings(g: &Graph, id: NodeId, kind: Kind, m: f64) -> BodySettings {
    if kind == Kind::Passive {
        return BodySettings { friction: 0.5 * (num(g, id, "staticFriction", 0.2) + num(g, id, "dynamicFriction", 0.2)), thickness: 0.0, ..Default::default() };
    }
    BodySettings {
        enabled: flag(g, id, "isDynamic", true),
        thickness: num(g, id, "thickness", 0.0) * m,
        friction: num(g, id, "friction", 0.1),
        collide: flag(g, id, "collide", true),
        ..Default::default()
    }
}

/// Metres in one scene unit for this solver: its spaceScale where a file set one, otherwise the scene's unit
/// (currentUnit -linear), so a centimetre of geometry is a centimetre of cloth.
pub fn metres(g: &Graph, s: NodeId) -> f64 {
    let ss = num(g, s, "spaceScale", 0.0);
    if ss > 0.0 {
        ss
    } else {
        crate::anim_time::metres_per_unit()
    }
}

/// The solver works in metres, kilograms and seconds: gravity in m/s^2, wind and speed limit in m/s.
fn solver_settings(g: &Graph, s: NodeId) -> SolverSettings {
    let m = metres(g, s);
    let ground = vec(g, s, "planeOrigin", DVec3::ZERO) * m;
    SolverSettings {
        enabled: flag(g, s, "enable", true),
        substeps: num(g, s, "subSteps", 12.0).clamp(1.0, 64.0) as u32,
        iterations: num(g, s, "iterations", 2.0).clamp(1.0, 16.0) as u32,
        contact_every: num(g, s, "contactEvery", 4.0).clamp(1.0, 64.0) as u32,
        gravity: vec(g, s, "gravityDirection", DVec3::NEG_Y).normalize_or_zero() * num(g, s, "gravity", 9.81),
        wind: vec(g, s, "windDirection", DVec3::X).normalize_or_zero() * num(g, s, "windSpeed", 0.0),
        air_density: num(g, s, "airDensity", 1.2),
        max_velocity: num(g, s, "maxVelocity", 50.0),
        ground: flag(g, s, "usePlane", false),
        ground_height: ground.y,
        ground_friction: num(g, s, "planeFriction", 0.1),
        ..Default::default()
    }
}

/// Settings that may change between frames without starting again.
fn apply_settings(g: &Graph, s: NodeId, st: &mut State) {
    let Some(solver) = st.solver.as_mut() else { return };
    solver.settings = solver_settings(g, s);
    let m = metres(g, s);
    for b in &st.bodies {
        let new = match b.kind {
            Kind::Cloth => cloth_settings(g, b.node, m),
            Kind::Particles => particle_settings(g, b.node, m),
            Kind::Rigid => rigid_settings(g, b.node, m),
            Kind::Collider | Kind::Passive => collider_settings(g, b.node, b.kind, m),
        };
        let body = &mut solver.bodies[b.index];
        // Pins and masses are built into the cloth's constraints: changing them builds those again.
        let rebuild = b.kind == Kind::Cloth && (new.pin_map != body.settings.pin_map || new.tear_ratio != body.settings.tear_ratio || new.area_stiffness.is_some() != body.settings.area_stiffness.is_some());
        body.settings = BodySettings { radius: if b.kind == Kind::Rigid { body.settings.radius } else { new.radius }, ..new };
        if rebuild {
            solver.rebuild_cloth(b.index);
        }
    }
}

/// Emitters feeding a particle body, as the solver takes them, keeping what each carries over between frames.
fn update_emitters(g: &mut Graph, node: NodeId, old: &[Emitter], mpu: f64) -> Vec<Emitter> {
    let Some(a) = g.node(node).and_then(|n| n.ty().attr_index("newParticles")) else { return Vec::new() };
    let sources: Vec<NodeId> = g.elements(&PlugRef::new(node, a as i32)).iter().filter_map(|e| g.source(e)).map(|s| s.node).collect();
    let mut out = Vec::new();
    for em in sources {
        if g.node(em).is_none_or(|n| n.ty().name != dy::EMITTER) {
            continue;
        }
        let _ = g.value(&PlugRef::new(em, crate::transform::DagNode::WORLD_MATRIX as i32));
        let m = g.node(em).unwrap().matrix(crate::transform::DagNode::WORLD_MATRIX);
        let kind = match num(g, em, "emitterType", 1.0) as i64 {
            0 => EmitterKind::Directional,
            2 => EmitterKind::Surface,
            3 => EmitterKind::Curve,
            4 => EmitterKind::Volume,
            _ => EmitterKind::Omni,
        };
        let name = g.path(em);
        let mut e = old.iter().find(|o| o.name == name).cloned().unwrap_or_else(|| Emitter::new(kind));
        let was = e.matrix.w_axis.truncate();
        e.name = name;
        e.kind = kind;
        e.rate = num(g, em, "rate", 100.0);
        e.speed = num(g, em, "speed", 1.0) * mpu;
        e.speed_random = num(g, em, "speedRandom", 0.0);
        e.spread = num(g, em, "spread", 0.0);
        e.direction = vec(g, em, "direction", DVec3::X);
        e.sphere = num(g, em, "volumeShape", 0.0) as i64 == 1;
        e.inherit_velocity = num(g, em, "inheritVelocity", 0.0);
        // Seeded by name, which a file keeps, so a saved scene emits the same particles.
        e.seed = num(g, em, "seed", 0.0) as u64 ^ e.name.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, c| (h ^ c as u64).wrapping_mul(0x100_0000_01b3));
        let m = to_metres(m, mpu);
        e.velocity = (m.w_axis.truncate() - was) * crate::anim::fps();
        e.matrix = m;
        e.points.clear();
        e.triangles.clear();
        if kind == EmitterKind::Surface {
            if let Some(mesh) = pulled(g, em, "inputMesh").and_then(|v| v.as_mesh().cloned()) {
                let mx = pulled(g, em, "inputMatrix").map_or(DMat4::IDENTITY, |v| v.as_matrix());
                let (pts, faces) = dy::mesh_parts(&mesh);
                e.points = pts.iter().map(|p| mx.transform_point3(*p) * mpu).collect();
                for f in faces {
                    for k in 1..f.len().saturating_sub(1) {
                        e.triangles.push([f[0], f[k], f[k + 1]]);
                    }
                }
            }
        }
        out.push(e);
    }
    out
}

/// Fields acting on the solver's bodies now.
fn gather_fields(g: &mut Graph, st: &State, mpu: f64) -> Vec<Field> {
    let mut by_node: BTreeMap<NodeId, Vec<u32>> = BTreeMap::new();
    for b in &st.bodies {
        let Some(a) = g.node(b.node).and_then(|n| n.ty().attr_index("inputForce")) else { continue };
        for e in g.elements(&PlugRef::new(b.node, a as i32)) {
            if let Some(src) = g.source(&e) {
                by_node.entry(src.node).or_default().push(b.index as u32);
            }
        }
    }
    for id in g.nodes() {
        if g.node(id).is_some_and(|n| n.ty().name == dy::FIELD) && flag(g, id, "applyToAll", false) {
            by_node.entry(id).or_default();
        }
    }
    let mut out = Vec::new();
    for (id, bodies) in by_node {
        if g.node(id).is_none_or(|n| n.ty().name != dy::FIELD) {
            continue;
        }
        let _ = g.value(&PlugRef::new(id, crate::transform::DagNode::WORLD_MATRIX as i32));
        let m = g.node(id).unwrap().matrix(crate::transform::DagNode::WORLD_MATRIX);
        let kind = match num(g, id, "fieldType", 0.0) as i64 {
            1 => FieldKind::Air,
            2 => FieldKind::Drag,
            3 => FieldKind::Newton,
            4 => FieldKind::Radial,
            5 => FieldKind::Turbulence,
            6 => FieldKind::Uniform,
            7 => FieldKind::Vortex,
            8 => FieldKind::VolumeAxis,
            _ => FieldKind::Gravity,
        };
        let mut f = Field::new(kind);
        f.name = g.path(id);
        f.all = flag(g, id, "applyToAll", false);
        f.bodies = bodies;
        f.magnitude = num(g, id, "magnitude", f.magnitude);
        f.attenuation = num(g, id, "attenuation", f.attenuation);
        f.max_distance = num(g, id, "maxDistance", f.max_distance) * mpu;
        f.use_max_distance = flag(g, id, "useMaxDistance", false);
        f.matrix = to_metres(m, mpu);
        f.direction = m.transform_vector3(vec(g, id, "direction", f.direction));
        f.speed = num(g, id, "speed", f.speed);
        f.inherit_velocity = num(g, id, "inheritVelocity", 0.0);
        f.use_direction = flag(g, id, "useDirection", false);
        f.min_distance = num(g, id, "minDistance", f.min_distance) * mpu;
        f.frequency = num(g, id, "frequency", f.frequency);
        f.phase = vec(g, id, "phase", DVec3::ZERO);
        f.away_from_axis = num(g, id, "awayFromAxis", 1.0);
        f.along_axis = num(g, id, "alongAxis", 0.0);
        f.around_axis = num(g, id, "aroundAxis", 0.0);
        out.push(f);
    }
    out
}

/// Builds the solver afresh from what is connected now.
fn build(g: &mut Graph, s: NodeId, st: &mut State) {
    let mut solver = Solver::new(solver_settings(g, s));
    let m = metres(g, s);
    st.bodies.clear();
    st.key.clear();
    let ids = members(g, s);
    for &id in &ids {
        let Some(kind) = kind_of(g, id) else { continue };
        let name = g.path(id);
        let index = match kind {
            Kind::Particles => {
                let b = solver.add_particles(&name, particle_settings(g, id, m));
                st.key.push((id, kind, 0, 0));
                b
            }
            _ => {
                let Some((points, faces, topo, _)) = world_mesh(g, id, kind, m) else { continue };
                st.key.push((id, kind, points.len(), topo));
                match kind {
                    Kind::Cloth => solver.add_cloth(&name, points, &faces, cloth_settings(g, id, m)),
                    Kind::Rigid => {
                        let xf = rigid_transform(g, id).unwrap_or(id);
                        let wm = to_metres(crate::rig::world(g, xf), m);
                        solver.add_rigid(&name, points, &faces, wm, rigid_settings(g, id, m))
                    }
                    _ => solver.add_collider(&name, points, &faces, collider_settings(g, id, kind, m)),
                }
            }
        };
        st.bodies.push(BodyRef { node: id, kind, index });
    }
    solver.links = crate::dyn_cmds::links_of(g, s, &st.bodies, &solver);
    st.solver = Some(solver);
    st.last = None;
    st.cache.clear();
}

/// The key the solver would be built with now.
fn current_key(g: &mut Graph, s: NodeId) -> Vec<(NodeId, Kind, usize, u64)> {
    let mut k = Vec::new();
    for id in members(g, s) {
        let Some(kind) = kind_of(g, id) else { continue };
        if kind == Kind::Particles {
            k.push((id, kind, 0, 0));
            continue;
        }
        if matches!(kind, Kind::Rigid | Kind::Passive) {
            if let Some((points, _, topo, _)) = world_mesh(g, id, kind, 1.0) {
                k.push((id, kind, points.len(), topo));
            }
            continue;
        }
        let mesh = match kind {
            Kind::Cloth => pulled(g, id, "inputPolymesh"),
            _ => pulled(g, id, "inputMesh"),
        };
        let Some(m) = mesh.and_then(|v| v.as_mesh().cloned()) else { continue };
        k.push((id, kind, m.point_capacity() as usize, connectivity(&m)));
    }
    k
}

/// Sets every body's input for the frame about to be solved.
fn feed(g: &mut Graph, st: &mut State, mpu: f64) {
    let bodies = st.bodies.clone();
    for b in &bodies {
        match b.kind {
            Kind::Cloth | Kind::Collider | Kind::Passive => {
                if let Some((points, _, _, _)) = world_mesh(g, b.node, b.kind, mpu) {
                    let solver = st.solver.as_mut().unwrap();
                    if !solver.set_input(b.index, &points) {
                        // The topology changed: build again next time.
                        st.key.clear();
                    }
                }
            }
            Kind::Particles => {
                let old = st.solver.as_ref().unwrap().bodies[b.index].emitters.clone();
                let em = update_emitters(g, b.node, &old, mpu);
                st.solver.as_mut().unwrap().bodies[b.index].emitters = em;
            }
            Kind::Rigid => {}
        }
    }
    let fields = gather_fields(g, st, mpu);
    st.solver.as_mut().unwrap().fields = fields;
}

/// What each body shows now, from the solver.
fn outputs(g: &mut Graph, st: &State, rest: bool, mpu: f64) -> Frame {
    let solver = st.solver.as_ref().unwrap();
    let mut out = Vec::new();
    for b in &st.bodies {
        match b.kind {
            Kind::Cloth => {
                if rest {
                    out.push((b.node, Out::Cloth(None)));
                    continue;
                }
                let m = pulled(g, b.node, "geomMatrix").map_or(DMat4::IDENTITY, |v| v.as_matrix());
                let inv = m.inverse();
                let points: Vec<DVec3> = solver.positions(b.index).iter().map(|p| inv.transform_point3(*p / mpu)).collect();
                out.push((b.node, Out::Cloth(Some(ClothOut { points: Arc::new(points), weights: Arc::new(solver.sim_weights(b.index)) }))));
            }
            Kind::Particles => {
                let body = &solver.bodies[b.index];
                let r = body.start..body.start + body.count;
                let pt = &solver.pt;
                out.push((
                    b.node,
                    Out::Particles(Arc::new(ParticleState {
                        position: pt.x[r.clone()].iter().map(|p| *p / mpu).collect(),
                        velocity: pt.v[r.clone()].iter().map(|v| *v / mpu).collect(),
                        id: pt.id[r.clone()].to_vec(),
                        age: pt.age[r.clone()].to_vec(),
                        lifespan: pt.life[r.clone()].to_vec(),
                        rgb: pt.rgb[r.clone()].to_vec(),
                        opacity: pt.opacity[r.clone()].to_vec(),
                        radius: pt.radius[r.clone()].iter().map(|x| x / mpu).collect(),
                        weight: pt.rs[r].iter().map(|x| x.w).collect(),
                    })),
                ));
            }
            Kind::Rigid => {
                let world = solver.rigid_matrix(b.index).map(|w| to_metres(w, 1.0 / mpu));
                let local = world.map(|w| {
                    let xf = rigid_transform(g, b.node).unwrap_or(b.node);
                    let parent = g.parent(xf);
                    let pw = if parent == crate::plug::NO_NODE { DMat4::IDENTITY } else { crate::rig::world(g, parent) };
                    pw.inverse() * w
                });
                out.push((b.node, Out::Rigid(if rest { None } else { local })));
            }
            _ => {}
        }
    }
    out
}

fn publish(g: &mut Graph, frame: &Frame) {
    for (id, o) in frame {
        let Some(n) = g.node(*id) else { continue };
        let any = n.behavior().as_any();
        match o {
            Out::Cloth(c) => {
                if let Some(cn) = any.downcast_ref::<ClothNode>() {
                    *cn.out.lock().unwrap() = c.clone();
                }
            }
            Out::Particles(p) => {
                if let Some(pn) = any.downcast_ref::<ParticlesNode>() {
                    *pn.state.lock().unwrap() = p.clone();
                }
            }
            Out::Rigid(m) => {
                if let Some(rn) = any.downcast_ref::<RigidNode>() {
                    *rn.local.lock().unwrap() = *m;
                }
            }
        }
        g.touch(*id);
    }
}

/// Plays a body from its disk cache where the cache has the frame: its output replaced, its part in the solve off.
fn from_disk(g: &mut Graph, st: &mut State, frame_no: f64, frame: &mut Frame) {
    for b in st.bodies.clone() {
        let path = text(g, b.node, "diskCache");
        if path.is_empty() || b.kind != Kind::Cloth {
            continue;
        }
        if !st.disk.contains_key(&path) {
            match aztek_dyn::cache::Cache::read(&path) {
                Ok(c) => {
                    st.disk.insert(path.clone(), Arc::new(c));
                }
                Err(_) => continue,
            }
        }
        let cache = st.disk[&path].clone();
        let name = g.path(b.node);
        if let Some(points) = cache.get(frame_no, &name) {
            let n = points.len();
            for (id, o) in frame.iter_mut() {
                if *id == b.node {
                    *o = Out::Cloth(Some(ClothOut { points: Arc::new(points.to_vec()), weights: Arc::new(vec![1.0; n]) }));
                }
            }
            if let Some(s) = st.solver.as_mut() {
                s.bodies[b.index].settings.enabled = false;
            }
        }
    }
}

/// Drops what was read from disk, so a cache file written again is read again.
pub(crate) fn forget_disk(g: &Graph) {
    for s in solvers(g) {
        if let Some(n) = g.node(s).and_then(|n| n.behavior().as_any().downcast_ref::<SolverNode>()) {
            n.state.lock().unwrap().disk.clear();
        }
    }
}

/// Starts a solver again at the next evaluation: after edits that change what it holds.
pub fn invalidate(g: &mut Graph, s: NodeId) {
    if let Some(n) = g.node(s).and_then(|n| n.behavior().as_any().downcast_ref::<SolverNode>()) {
        let mut st = n.state.lock().unwrap();
        st.key.clear();
        st.solver = None;
    }
    g.touch(s);
}

fn run(g: &mut Graph, s: NodeId) {
    let Some(node) = g.node(s) else { return };
    if node.behavior().as_any().downcast_ref::<SolverNode>().is_none() {
        return;
    }
    let frame_no = crate::anim::frame(g);
    let start = num(g, s, "startFrame", 1.0);
    let mpu = metres(g, s);
    let wanted = current_key(g, s);
    // The state is taken out while the graph is used, and put back after.
    let mut st = {
        let n = g.node(s).unwrap().behavior().as_any().downcast_ref::<SolverNode>().unwrap();
        std::mem::take(&mut *n.state.lock().unwrap())
    };
    if st.solver.is_none() || st.key != wanted {
        build(g, s, &mut st);
        st.shown = None;
    }
    apply_settings(g, s, &mut st);
    let t0 = std::time::Instant::now();
    let mut stepped = false;
    let frame: Option<Arc<Frame>> = if !flag(g, s, "enable", true) {
        Some(Arc::new(st.bodies.iter().map(|b| (b.node, match b.kind {
            Kind::Particles => Out::Particles(Arc::default()),
            Kind::Rigid => Out::Rigid(None),
            _ => Out::Cloth(None),
        })).collect()))
    } else if frame_no <= start + 1e-6 || st.last.is_none() {
        // Built afresh at the start: the rest shapes, and the nails and other links taken from them, are where the
        // meshes are now. One built while a file was still being read had them before the meshes were placed.
        if st.last.is_some() || st.results == 0 {
            build(g, s, &mut st);
        }
        feed(g, &mut st, mpu);
        let solver = st.solver.as_mut().unwrap();
        solver.reset();
        // Particles made with the particle command are there from the start.
        for b in st.bodies.iter().filter(|b| b.kind == Kind::Particles) {
            if let Some(p) = floats(g, b.node, "startPositions") {
                let pts: Vec<(DVec3, DVec3)> = p.chunks_exact(3).map(|c| (DVec3::new(c[0], c[1], c[2]) * mpu, DVec3::ZERO)).collect();
                solver.add_particles_now(b.index, &pts);
                solver.layout_now();
            }
        }
        st.cache.clear();
        st.last = Some(frame_no);
        st.results += 1;
        let f = Arc::new(outputs(g, &st, true, mpu));
        st.cache.insert(key(frame_no), f.clone());
        Some(f)
    } else if let Some(f) = st.cache.get(&key(frame_no)) {
        Some(f.clone())
    } else if frame_no > st.last.unwrap() && !g.sampling() {
        feed(g, &mut st, mpu);
        let fps = crate::anim::fps().max(1e-6);
        let dt = (frame_no - st.last.unwrap()).min(1.0) / fps * num(g, s, "timeScale", 1.0);
        st.solver.as_mut().unwrap().step(dt);
        stepped = true;
        st.results += 1;
        st.last = Some(frame_no);
        let mut f = outputs(g, &st, false, mpu);
        from_disk(g, &mut st, frame_no, &mut f);
        let f = Arc::new(f);
        st.cache.insert(key(frame_no), f.clone());
        Some(f)
    } else {
        None
    };
    if stepped {
        st.solve_ms = t0.elapsed().as_secs_f64() * 1e3;
    }
    if let Some(f) = &frame {
        st.out_count = f
            .iter()
            .map(|(_, o)| match o {
                Out::Cloth(Some(c)) => c.weights.iter().filter(|&&w| w < 1.0).count(),
                Out::Particles(p) => p.weight.iter().filter(|&&w| w < 1.0).count(),
                _ => 0,
            })
            .sum();
    }
    let now = (key(frame_no), st.results);
    let fresh = frame.is_some() && st.shown != Some(now);
    if fresh {
        st.shown = Some(now);
    }
    if let Some(n) = g.node(s).and_then(|n| n.behavior().as_any().downcast_ref::<SolverNode>()) {
        *n.state.lock().unwrap() = st;
    }
    if let (Some(f), true) = (frame, fresh) {
        publish(g, &f);
        g.touch(s);
    }
}

/// Runs a solver's state through `f`, for commands that read it.
pub fn with_state<R>(g: &Graph, s: NodeId, f: impl FnOnce(&State) -> R) -> Option<R> {
    let n = g.node(s)?.behavior().as_any().downcast_ref::<SolverNode>()?;
    let st = n.state.lock().unwrap();
    Some(f(&st))
}
