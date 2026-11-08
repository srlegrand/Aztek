//! The one solver. Every body is particles; each frame is split into substeps, and each substep predicts positions,
//! projects the constraints (cloth, links, contacts, rigid shapes) and takes the velocity from what moved: extended
//! position based dynamics with one projection per substep (small steps rather than many iterations).
//!
//! Hard limits, checked every substep: velocity and displacement are bounded, a particle whose value stops being finite
//! is put back where it was and stopped, and a constraint whose correction is not finite is skipped, so a bad value
//! never spreads to neighbours. What cannot be solved (pushed through, squeezed, stretched far past its limits,
//! oscillating) is handled per vertex by `resolve`.

use std::sync::Arc;

use glam::{DMat4, DVec3};
use rayon::prelude::*;

use crate::util::PAR_CHUNK;

use crate::body::{Body, BodyKind, BodySettings, ClothData, Fallback, Phase, RigidData, RigidShape};
use crate::fields::{Field, FieldKind};
use crate::geom::Aabb;
use crate::topology::{colour, Topology};
use crate::util::{clamp_len, finite, smoothstep, Rng};

#[derive(Clone, Debug)]
pub struct SolverSettings {
    pub enabled: bool,
    pub substeps: u32,
    /// Passes over the cloth's stretch constraints in each substep; bending and area take one.
    pub iterations: u32,
    /// Substeps between searches for contacts.
    pub contact_every: u32,
    /// Acceleration, world units per second squared.
    pub gravity: DVec3,
    pub wind: DVec3,
    pub air_density: f64,
    /// No particle moves faster, metres per second.
    pub max_velocity: f64,
    /// A frame longer than this is solved as this long.
    pub max_frame_time: f64,
    /// A ground plane at this height, when on.
    pub ground: bool,
    pub ground_height: f64,
    pub ground_friction: f64,
    pub seed: u64,
}

impl Default for SolverSettings {
    fn default() -> Self {
        SolverSettings {
            enabled: true,
            substeps: 12,
            iterations: 2,
            contact_every: 4,
            gravity: DVec3::new(0.0, -9.8, 0.0),
            wind: DVec3::ZERO,
            air_density: 1.0,
            max_velocity: 50.0,
            max_frame_time: 1.0 / 12.0,
            ground: false,
            ground_height: 0.0,
            ground_friction: 0.5,
            seed: 0,
        }
    }
}

/// One end of a link.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Anchor {
    /// A particle of a body, by its index in the body.
    Particle(u32, u32),
    /// A fixed place.
    World(DVec3),
    /// A place on a triangle of a body (collider or cloth), by barycentric weights.
    Surface(u32, u32, DVec3),
}

/// A constraint between two places: weld, component to component, point to surface, and the rigid pin, nail, hinge
/// and spring are all links.
#[derive(Clone, Debug)]
pub struct Link {
    pub name: String,
    pub a: Anchor,
    pub b: Anchor,
    pub rest: f64,
    pub compliance: f64,
    /// Breaks once stretched further than this past its rest length; 0 never.
    pub tear: f64,
    pub broken: bool,
    pub enabled: bool,
}

/// Where a vertex stands with the impossible state handling.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Sim,
    Leaving,
    Out,
    Returning,
}

#[derive(Clone, Copy, Debug)]
pub struct Resolve {
    /// 1 simulated, 0 following the fallback.
    pub w: f64,
    pub w_prev: f64,
    pub trouble: f64,
    pub calm: u32,
    pub out_frames: u32,
    pub mode: Mode,
    /// Frames since it last left the simulation, for the recovery time.
    pub left_at: u64,
}

impl Default for Resolve {
    fn default() -> Self {
        Resolve { w: 1.0, w_prev: 1.0, trouble: 0.0, calm: 0, out_frames: 0, mode: Mode::Sim, left_at: 0 }
    }
}

/// What the last substep found wrong with a particle.
#[derive(Clone, Copy, Debug, Default)]
pub struct Signal {
    /// Left inside a collider after the solve, in thicknesses.
    pub pen: f64,
    /// Through a collider further than it could be pushed back.
    pub deep: bool,
    /// Pressed from opposite sides.
    pub squeeze: bool,
    pub nan: bool,
    /// The animated position is itself inside something.
    pub anim_bad: bool,
    /// Contact normal this substep, for bounce.
    pub normal: DVec3,
}

/// Which side of each nearby collider a particle belongs on: (collider body, +1 or -1). Four at most.
#[derive(Clone, Copy, Debug, Default)]
pub struct Sides {
    pub body: [u32; 4],
    pub side: [i8; 4],
    pub n: u8,
}

impl Sides {
    pub fn get(&self, b: u32) -> Option<f64> {
        (0..self.n as usize).find(|&k| self.body[k] == b).map(|k| self.side[k] as f64)
    }
    pub fn set(&mut self, b: u32, s: f64) {
        let s = if s < 0.0 { -1 } else { 1 };

        if let Some(k) = (0..self.n as usize).find(|&k| self.body[k] == b) {
            self.side[k] = s;
        } else if (self.n as usize) < 4 {
            self.body[self.n as usize] = b;
            self.side[self.n as usize] = s;
            self.n += 1;
        }
    }
    pub fn forget(&mut self, b: u32) {

        if let Some(k) = (0..self.n as usize).find(|&k| self.body[k] == b) {
            let last = self.n as usize - 1;
            self.body[k] = self.body[last];
            self.side[k] = self.side[last];
            self.n -= 1;
        }
    }
}

/// Every particle in the solver, by array.
#[derive(Clone, Debug, Default)]
pub struct Particles {
    pub x: Vec<DVec3>,
    pub v: Vec<DVec3>,
    pub p: Vec<DVec3>,
    /// Where each was at the start of the frame.
    pub x0: Vec<DVec3>,
    pub v_prev_frame: Vec<DVec3>,
    pub inv_mass: Vec<f64>,
    pub radius: Vec<f64>,
    pub body: Vec<u32>,
    pub local: Vec<u32>,
    pub phase: Vec<Phase>,
    pub rs: Vec<Resolve>,
    pub sig: Vec<Signal>,
    pub sides: Vec<Sides>,
    /// Particles only: age and lifespan in seconds, colour, opacity, an id that stays with the particle.
    pub age: Vec<f64>,
    pub life: Vec<f64>,
    pub rgb: Vec<DVec3>,
    pub opacity: Vec<f64>,
    pub id: Vec<u64>,
}

impl Particles {
    pub fn len(&self) -> usize {
        self.x.len()
    }
    pub fn is_empty(&self) -> bool {
        self.x.is_empty()
    }
}

#[derive(Clone, Debug, Default)]
pub struct Stats {
    pub frame: u64,
    /// Vertices not fully simulated (blending out, out, or blending back).
    pub out: usize,
    pub left: usize,
    pub returned: usize,
    /// Values that stopped being finite and were put back.
    pub fixed: usize,
    /// Velocities cut to the limit.
    pub clamped: usize,
    pub contacts: usize,
    pub pairs: usize,
    /// The longest a vertex has stayed out, in frames, since the last reset.
    pub longest_out: u32,
    /// Milliseconds: frame preparation (contacts found), constraints, collisions, the rest.
    pub ms_prepare: f64,
    pub ms_constraints: f64,
    pub ms_collide: f64,
    pub ms_other: f64,
    /// Milliseconds by part, summed over the frame's substeps.
    pub detail: Vec<(&'static str, f64)>,
}

impl Stats {
    pub(crate) fn add(&mut self, name: &'static str, since: std::time::Instant) {
        let ms = since.elapsed().as_secs_f64() * 1e3;
        match self.detail.iter_mut().find(|d| d.0 == name) {
            Some(d) => d.1 += ms,
            None => self.detail.push((name, ms)),
        }
    }
}

/// Lets later systems (liquids, smoke, fire) take part in each substep of the same solve.
pub trait Extension: Send + Sync {
    fn name(&self) -> &str;
    /// After the cloth and link constraints, before contacts: may move particles it owns.
    fn substep(&mut self, view: &mut SubstepView);
}

pub struct SubstepView<'a> {
    pub x: &'a [DVec3],
    pub p: &'a mut [DVec3],
    pub phase: &'a [Phase],
    pub inv_mass: &'a [f64],
    pub h: f64,
}

pub struct Solver {
    pub settings: SolverSettings,
    pub bodies: Vec<Body>,
    pub fields: Vec<Field>,
    pub links: Vec<Link>,
    pub pt: Particles,
    pub stats: Stats,
    pub extensions: Vec<Box<dyn Extension>>,
    /// Seconds solved since the reset.
    pub time: f64,
    pub frame: u64,
    pub(crate) frame_data: crate::contacts::FrameData,
    pub(crate) bvh: crate::contacts::BvhCache,
    /// Effective inverse masses this substep: base times the simulation weight.
    pub(crate) im: Vec<f64>,
    /// Animated positions this substep.
    pub(crate) anim_now: Vec<DVec3>,
    pub(crate) has_anim: Vec<bool>,
    pub(crate) normals: Vec<DVec3>,
    pub(crate) v_int: Vec<DVec3>,
    pub(crate) rest_now: Vec<Option<Vec<f64>>>,
    /// Each particle's weight in the simulation this substep: what blending to a fallback leaves of its speed.
    pub(crate) blend_ws: Vec<f64>,
    /// With DYN_JUMP: the largest move each phase made this frame, and by which particle.
    pub jumps: std::collections::BTreeMap<&'static str, (usize, f64)>,
}

impl Default for Solver {
    fn default() -> Self {
        Self::new(SolverSettings::default())
    }
}

impl Solver {
    pub fn new(settings: SolverSettings) -> Solver {
        Solver {
            settings,
            bodies: Vec::new(),
            fields: Vec::new(),
            links: Vec::new(),
            pt: Particles::default(),
            stats: Stats::default(),
            extensions: Vec::new(),
            time: 0.0,
            frame: 0,
            frame_data: Default::default(),
            bvh: Default::default(),
            im: Vec::new(),
            anim_now: Vec::new(),
            has_anim: Vec::new(),
            normals: Vec::new(),
            v_int: Vec::new(),
            rest_now: Vec::new(),
            blend_ws: Vec::new(),
            jumps: Default::default(),
        }
    }

    fn add_body(&mut self, kind: BodyKind, name: &str, positions: Vec<DVec3>, polygons: &[Vec<u32>], settings: BodySettings) -> usize {
        let positions: Vec<DVec3> = positions.into_iter().map(|p| if finite(p) { p } else { DVec3::ZERO }).collect();
        let topo = Arc::new(Topology::new(positions.len(), polygons));
        let body = Body {
            kind,
            name: name.into(),
            settings,
            topo,
            start: 0,
            count: 0,
            anim_prev: positions.clone(),
            anim: positions.clone(),
            rest: positions,
            cloth: Arc::new(ClothData::default()),
            torn: Vec::new(),
            emitters: Vec::new(),
            rigid: None,
            reset_pending: true,
            mean_edge: 1.0,
            radii: Vec::new(),
            outside: None,
            next_id: 0,
            pending: Vec::new(),
        };
        self.bodies.push(body);
        let b = self.bodies.len() - 1;
        if kind == BodyKind::Cloth {
            self.build_cloth(b);
        }
        b
    }

    /// Cloth from a mesh in world space.
    pub fn add_cloth(&mut self, name: &str, positions: Vec<DVec3>, polygons: &[Vec<u32>], settings: BodySettings) -> usize {
        self.add_body(BodyKind::Cloth, name, positions, polygons, settings)
    }

    /// An animated mesh the others collide with.
    pub fn add_collider(&mut self, name: &str, positions: Vec<DVec3>, polygons: &[Vec<u32>], settings: BodySettings) -> usize {
        let b = self.add_body(BodyKind::Collider, name, positions, polygons, settings);
        let me = self.bodies[b].topo.mean_edge(&self.bodies[b].rest);
        self.bodies[b].mean_edge = me;
        // A closed mesh has one outside, whichever side a particle starts on: one that starts inside is pushed out,
        // or, deep in, handled as impossible.
        let body = &mut self.bodies[b];
        let t = &body.topo;
        if !t.triangles.is_empty() && !t.border.iter().any(|&x| x) {
            let volume: f64 = t.triangles.iter().map(|tr| {
                let [a, bb, c] = tr.map(|k| body.rest[k as usize]);
                a.dot(bb.cross(c))
            }).sum();
            body.outside = Some(if volume < 0.0 { -1.0 } else { 1.0 });
        }
        b
    }

    /// An empty particle body; emitters and `add_particles_now` fill it.
    pub fn add_particles(&mut self, name: &str, settings: BodySettings) -> usize {
        self.add_body(BodyKind::Particles, name, Vec::new(), &[], settings)
    }

    /// A rigid body from a mesh in world space, its object's world matrix `matrix`.
    pub fn add_rigid(&mut self, name: &str, positions: Vec<DVec3>, polygons: &[Vec<u32>], matrix: DMat4, settings: BodySettings) -> usize {
        let b = self.add_body(BodyKind::Rigid, name, positions, polygons, settings);
        let samples = crate::rigid::sample(&self.bodies[b], matrix);
        let body = &mut self.bodies[b];
        body.rest = samples.points.clone();
        body.anim_prev = samples.points.clone();
        body.anim = samples.points;
        body.settings.radius = samples.radius;
        body.radii = samples.radii;
        body.rigid = Some(RigidData { matrix0: matrix, rot: glam::DQuat::IDENTITY, ..Default::default() });
        b
    }

    /// Builds a cloth's constraints again, after its pins or tearing changed.
    pub fn rebuild_cloth(&mut self, b: usize) {
        if self.bodies.get(b).is_some_and(|x| x.kind == BodyKind::Cloth) {
            self.build_cloth(b);
        }
    }

    /// Lays the particles out again now: particles added with `add_particles_now` appear at once.
    pub fn layout_now(&mut self) {
        self.layout(true, 0.0);
        self.learn_all_sides();
    }

    /// Rebuilds the cloth constraints from the body's rest positions.
    pub(crate) fn build_cloth(&mut self, b: usize) {
        let body = &self.bodies[b];
        let t = &body.topo;
        let rest = &body.rest;
        let n = rest.len();
        let mut c = ClothData { dist: t.edges.clone(), ..Default::default() };
        // Each triangle's area, shared out to its corners and its edges.
        let mut point_area = vec![0.0; n];
        let mut edge_area: std::collections::HashMap<(u32, u32), f64> = std::collections::HashMap::new();
        for tr in &t.triangles {
            let a = crate::constraints::area(rest, tr);
            if !a.is_finite() {
                continue;
            }
            for k in 0..3 {
                point_area[tr[k] as usize] += a / 3.0;
                let (p, q) = (tr[k], tr[(k + 1) % 3]);
                *edge_area.entry((p.min(q), p.max(q))).or_default() += a / 3.0;
            }
        }
        // Points on degenerate seams have no area of their own; each weighs at least a quarter of the average point, or
        // the slightest contact flings it.
        let used: Vec<f64> = point_area.iter().copied().filter(|&a| a > 0.0).collect();
        let mean_area = if used.is_empty() { 0.0 } else { used.iter().sum::<f64>() / used.len() as f64 };
        c.point_area = point_area.into_iter().map(|a| a.max(if crate::util::off("massfloor") { 0.0 } else { mean_area * 0.25 })).collect();
        c.dist_rest = c.dist.iter().map(|e| (rest[e[0] as usize] - rest[e[1] as usize]).length()).collect();
        if std::env::var("DYN_WHY").is_ok() {
            let mut l = c.dist_rest.clone();
            l.sort_by(|a, b| a.total_cmp(b));
            let q = |f: f64| l[((l.len() - 1) as f64 * f) as usize];
            let zero_area = c.point_area.iter().filter(|&&a| a <= 0.0).count();
            eprintln!("edges {}: min {:.2e} 1% {:.2e} 10% {:.2e} median {:.2e} max {:.2e} m; points without area {zero_area}; triangles {}", l.len(), q(0.0), q(0.01), q(0.1), q(0.5), q(1.0), t.triangles.len());
        }
        // An edge's stiffness is the material's times its share of the surface over its length squared: the same cloth
        // stretches the same at any resolution.
        // Seam edges of almost no length still hold their points together: no edge is weaker than a regular mesh's
        // (an equilateral triangle's share, about 0.29).
        let mean = t.mean_edge(rest).max(1e-9);
        c.dist_geo = c.dist.iter().zip(&c.dist_rest).map(|(e, &l)| (edge_area.get(&(e[0], e[1])).copied().unwrap_or(0.0) / l.max(mean * 0.25).powi(2)).clamp(if crate::util::off("geofloor") { 0.0 } else { 0.29 }, 4.0)).collect();
        c.dist_colour = colour(&mut c.dist, &mut [&mut c.dist_rest, &mut c.dist_geo], n);
        c.bend = t.bends.clone();
        c.bend_rest = c.bend.iter().map(|s| crate::constraints::dihedral_angle(rest, s)).collect();
        // A hinge's stiffness from the bending stiffness: 3 |e|^2 / (A1 + A2), as for discrete shells.
        c.bend_geo = c
            .bend
            .iter()
            .map(|s| {
                let e = (rest[s[0] as usize] - rest[s[1] as usize]).length_squared();
                let (a1, a2) = (crate::constraints::area(rest, &[s[0], s[1], s[2]]), crate::constraints::area(rest, &[s[0], s[1], s[3]]));
                // A sliver's fold angle is noise: no bending across one. Otherwise capped, so no hinge is stiffer than
                // a well shaped mesh's would be many times over.
                if a1.min(a2) > 0.02 * e && e.is_finite() && e > 0.0 { (3.0 * e / (a1 + a2)).min(20.0) } else { 0.0 }
            })
            .collect();
        c.bend_colour = colour(&mut c.bend, &mut [&mut c.bend_rest, &mut c.bend_geo], n);
        if body.settings.area_stiffness.is_some() {
            c.area = t.triangles.clone();
            c.area_rest = c.area.iter().map(|tr| crate::constraints::area(rest, tr)).collect();
            c.area_colour = colour(&mut c.area, &mut [&mut c.area_rest], n);
        }
        if body.settings.tear_ratio <= 0.0 {
            let (path, start, rest_len) = crate::topology::stretch_paths(t, rest);
            c.path = path;
            c.path_start = start;
            c.path_rest = rest_len;
        }
        let anchors: Vec<u32> = (0..n).filter(|&l| body.map(&body.settings.pin_map, l, 0.0) >= 0.5).map(|l| l as u32).collect();
        if !anchors.is_empty() && body.settings.tear_ratio <= 0.0 {
            c.tether = crate::topology::tethers(t, rest, &anchors);
        }
        let torn = vec![false; c.dist.len()];
        let mean_edge = t.mean_edge(rest);
        let body = &mut self.bodies[b];
        body.mean_edge = mean_edge;
        body.cloth = Arc::new(c);
        body.torn = torn;
    }

    /// The animated input for the frame about to be solved, world space. False when the count is wrong for the body:
    /// the topology changed and the body must be made again.
    pub fn set_input(&mut self, b: usize, positions: &[DVec3]) -> bool {
        let Some(body) = self.bodies.get_mut(b) else { return false };
        if body.kind == BodyKind::Rigid {
            return true;
        }
        if positions.len() != body.anim.len() {
            return false;
        }
        let mut next: Vec<DVec3> = positions.iter().zip(&body.anim).map(|(&p, &old)| if finite(p) { p } else { old }).collect();
        // A jump further than the body's own size in one frame is a teleport: no sweep through what lies between.
        let size = Aabb::of(&body.anim).size().length().max(1e-6);
        let moved = next.iter().zip(&body.anim).fold(0.0f64, |m, (a, b)| m.max((*a - *b).length()));
        std::mem::swap(&mut body.anim, &mut next);
        body.anim_prev = next;
        if moved > size.max(body.settings.thickness * 10.0) {
            body.anim_prev = body.anim.clone();
            if body.kind != BodyKind::Collider {
                body.reset_pending = true;
            }
        }
        true
    }

    /// Particles added now, as a script's emit would: positions and velocities.
    pub fn add_particles_now(&mut self, b: usize, points: &[(DVec3, DVec3)]) {
        if let Some(body) = self.bodies.get_mut(b) {
            body.pending.extend(points.iter().copied().filter(|(p, v)| finite(*p) && finite(*v)));
        }
    }

    pub fn positions(&self, b: usize) -> &[DVec3] {
        let body = &self.bodies[b];
        if body.kind == BodyKind::Collider {
            return &body.anim;
        }
        &self.pt.x[body.start..body.start + body.count]
    }

    pub fn velocities(&self, b: usize) -> &[DVec3] {
        let body = &self.bodies[b];
        &self.pt.v[body.start..body.start + body.count]
    }

    /// Each vertex's simulation weight: 1 simulated, 0 out, between while blending.
    pub fn sim_weights(&self, b: usize) -> Vec<f64> {
        let body = &self.bodies[b];
        self.pt.rs[body.start..body.start + body.count].iter().map(|r| r.w).collect()
    }

    /// How many of a body's vertices are not fully simulated now.
    pub fn out_count(&self, b: usize) -> usize {
        self.sim_weights(b).iter().filter(|&&w| w < 1.0).count()
    }

    /// The object's world matrix for a rigid body.
    pub fn rigid_matrix(&self, b: usize) -> Option<DMat4> {
        self.bodies.get(b)?.rigid.as_ref().map(RigidData::matrix)
    }

    /// Back to the start: every body at its input, at rest, particles gone.
    pub fn reset(&mut self) {
        self.time = 0.0;
        self.frame = 0;
        self.stats = Stats::default();
        for b in 0..self.bodies.len() {
            let body = &mut self.bodies[b];
            body.anim_prev = body.anim.clone();
            body.reset_pending = true;
            body.next_id = 0;
            body.pending.clear();
            for e in &mut body.emitters {
                e.carry = 0.0;
            }
            for t in &mut body.torn {
                *t = false;
            }
            if body.kind == BodyKind::Particles {
                body.count = 0;
            }
        }
        for l in &mut self.links {
            l.broken = false;
        }
        self.layout(true, 0.0);
        self.learn_all_sides();
    }

    /// Rebuilds the particle arrays: bodies in order, particle bodies with their living particles and new ones.
    /// `dt` is the frame being solved, for emitters.
    pub(crate) fn layout(&mut self, reset: bool, dt: f64) {
        let old = std::mem::take(&mut self.pt);
        let mut pt = Particles::default();
        let seed = self.settings.seed;
        for (bi, body) in self.bodies.iter_mut().enumerate() {
            let start = pt.len();
            let was = body.start..body.start + body.count;
            let fresh = reset || body.reset_pending;
            let s = &body.settings;
            match body.kind {
                BodyKind::Collider => {
                    body.start = start;
                    body.count = 0;
                    body.reset_pending = false;
                    continue;
                }
                BodyKind::Cloth | BodyKind::Rigid => {
                    let n = body.anim.len();
                    for l in 0..n {
                        let keep = !fresh && was.len() == n;
                        let i = was.start + l;
                        let mass = if body.kind == BodyKind::Rigid {
                            s.mass / n.max(1) as f64
                        } else if s.density > 0.0 {
                            // Areal density over the point's share of the surface.
                            s.density * body.cloth.point_area.get(l).copied().unwrap_or(0.0).max(1e-12) * body.map(&s.mass_map, l, 1.0)
                        } else {
                            s.mass * body.map(&s.mass_map, l, 1.0)
                        };
                        let pinned = body.map(&s.pin_map, l, 0.0) >= 0.5;
                        let inv = if mass > 0.0 && mass.is_finite() && !pinned { 1.0 / mass } else { 0.0 };
                        let radius = if body.kind == BodyKind::Rigid { body.radii.get(l).copied().unwrap_or(s.radius) } else { s.thickness };
                        let v0 = if body.kind == BodyKind::Rigid {
                            s.initial_velocity + s.initial_spin.cross(body.anim[l] - body.anim.iter().sum::<DVec3>() / n as f64)
                        } else {
                            DVec3::ZERO
                        };
                        push(&mut pt, if keep { Some((&old, i)) } else { None }, body.anim[l], v0, inv, radius, bi as u32, l as u32, body.phase());
                    }
                    if body.kind == BodyKind::Rigid && fresh {
                        let com = body.anim.iter().sum::<DVec3>() / n.max(1) as f64;
                        if let Some(r) = &mut body.rigid {
                            r.com0 = com;
                            r.com = com;
                            r.rot = glam::DQuat::IDENTITY;
                            r.offsets = body.anim.iter().map(|p| *p - com).collect();
                        }
                    }
                }
                BodyKind::Particles => {
                    let mass = if s.mass > 0.0 { s.mass } else { 0.0 };
                    let inv = if mass > 0.0 { 1.0 / mass } else { 0.0 };
                    if !fresh {
                        for i in was.clone() {
                            if i >= old.len() {
                                break;
                            }
                            let dead = s.lifespan_mode != crate::body::Lifespan::Forever && old.age[i] >= old.life[i];
                            if !dead {
                                push(&mut pt, Some((&old, i)), old.x[i], old.v[i], old.inv_mass[i], old.radius[i], bi as u32, 0, Phase::Particle);
                            }
                        }
                    }
                    let mut born: Vec<(DVec3, DVec3)> = std::mem::take(&mut body.pending);
                    if !reset {
                        for e in &mut body.emitters {
                            born.extend(e.emit(self.frame as i64, dt));
                        }
                    }
                    let mut rng = Rng::new(crate::util::hash3(seed, bi as u64, self.frame));
                    for (p, v) in born {
                        if pt.len() - start >= s.max_count {
                            break;
                        }
                        push(&mut pt, None, p, v, inv, s.radius, bi as u32, 0, Phase::Particle);
                        let k = pt.len() - 1;
                        pt.life[k] = match s.lifespan_mode {
                            crate::body::Lifespan::Forever | crate::body::Lifespan::PerParticle => s.lifespan.max(0.0),
                            crate::body::Lifespan::Constant => s.lifespan,
                            crate::body::Lifespan::Random => (s.lifespan + s.lifespan_random * rng.signed() * 0.5).max(0.0),
                        };
                        pt.id[k] = body.next_id;
                        body.next_id += 1;
                    }
                    for (l, k) in (start..pt.len()).enumerate() {
                        pt.local[k] = l as u32;
                    }
                }
            }
            body.start = start;
            body.count = pt.len() - start;
            body.reset_pending = false;
        }
        self.pt = pt;
    }

    /// Learns which side of each collider every particle is on, from where they are now.
    pub(crate) fn learn_all_sides(&mut self) {
        let fd = crate::contacts::FrameData::colliders_only(self);
        let sides: Vec<Sides> = (0..self.pt.len())
            .into_par_iter()
            .with_min_len(PAR_CHUNK)
            .map(|i| crate::contacts::sides_at(self, &fd, i, self.pt.x[i]))
            .collect();
        self.pt.sides = sides;
    }

    /// Solves one frame of dt seconds against the inputs set for it.
    pub fn step(&mut self, dt: f64) {
        if !self.settings.enabled || !(dt > 0.0) || !dt.is_finite() {
            return;
        }
        let dt = dt.min(self.settings.max_frame_time.max(1e-4));
        let t0 = std::time::Instant::now();
        self.frame += 1;
        self.stats = Stats { frame: self.frame, longest_out: self.stats.longest_out, ..Default::default() };
        // Particles age, die and are born; bodies whose input jumped start again from it.
        let any_reset = self.bodies.iter().any(|b| b.reset_pending);
        let particles = self.bodies.iter().any(|b| b.kind == BodyKind::Particles);
        if particles || any_reset {
            for i in 0..self.pt.len() {
                if self.pt.phase[i] == Phase::Particle {
                    self.pt.age[i] += dt;
                }
            }
            self.layout(false, dt);
            if any_reset {
                self.learn_all_sides();
            }
        }
        let n = self.pt.len();
        self.pt.x0.clone_from(&self.pt.x);
        self.rest_now = self.bodies.iter().map(|b| crate::constraints::rest_from_input(b)).collect();
        self.has_anim = (0..n).map(|i| matches!(self.pt.phase[i], Phase::Cloth)).collect();
        let t = std::time::Instant::now();
        self.normals = crate::constraints::vertex_normals(self);
        self.stats.add("normals", t);
        self.stats.ms_prepare = t0.elapsed().as_secs_f64() * 1e3;
        let substeps = self.settings.substeps.clamp(1, 64);
        let every = self.settings.contact_every.clamp(1, substeps);
        let h = dt / substeps as f64;
        for s in 0..substeps {
            let a0 = s as f64 / substeps as f64;
            let a1 = (s + 1) as f64 / substeps as f64;
            if s % every == 0 {
                // Contacts found again over the next few substeps: short motion keeps the search small.
                let t = std::time::Instant::now();
                let span = every.min(substeps - s);
                let a_end = (s + span) as f64 / substeps as f64;
                self.frame_data = crate::contacts::FrameData::build(self, h * span as f64, a0, a_end);
                self.stats.pairs = self.stats.pairs.max(self.frame_data.pairs());
                self.stats.ms_prepare += t.elapsed().as_secs_f64() * 1e3;
            }
            self.substep(h, a0, a1, s + 1 == substeps);
        }
        let t1 = std::time::Instant::now();
        crate::constraints::tear(self);
        crate::resolve::end_frame(self, dt);
        for b in 0..self.bodies.len() {
            crate::rigid::publish(self, b);
        }
        self.pt.v_prev_frame.clone_from(&self.pt.v);
        self.time += dt;
        self.stats.ms_other += t1.elapsed().as_secs_f64() * 1e3;
    }

    fn substep(&mut self, h: f64, a0: f64, a1: f64, last: bool) {
        let n = self.pt.len();
        let tc = std::time::Instant::now();
        // Animated positions and the weights the impossible state handling sets.
        let mut anim_now = vec![DVec3::ZERO; n];
        let mut im = vec![0.0; n];
        for (bi, body) in self.bodies.iter().enumerate() {
            if body.kind == BodyKind::Collider {
                continue;
            }
            let _ = bi;
            for l in 0..body.count {
                let i = body.start + l;
                if body.kind == BodyKind::Cloth {
                    anim_now[i] = body.anim_prev[l].lerp(body.anim[l], a1);
                } else {
                    anim_now[i] = self.pt.x0[i];
                }
                let r = &self.pt.rs[i];
                let w = smoothstep(r.w_prev + (r.w - r.w_prev) * a1);
                im[i] = self.pt.inv_mass[i] * w;
            }
        }
        self.anim_now = anim_now;
        self.im = im;
        self.stats.add("weights", tc);
        let t = std::time::Instant::now();
        let snap = |s: &Solver| if std::env::var("DYN_JUMP").is_ok() { s.pt.p.clone() } else { Vec::new() };
        let jump = |s: &mut Solver, name: &'static str, before: &[DVec3]| {
            if before.len() == s.pt.p.len() && !before.is_empty() {
                let (i, d) = s.pt.p.iter().zip(before).map(|(a, b)| (*a - *b).length()).enumerate().fold((0, 0.0f64), |m, (i, d)| if d > m.1 { (i, d) } else { m });
                let e = s.jumps.entry(name).or_insert((0, 0.0));
                if d > e.1 {
                    *e = (i, d);
                }
            }
        };
        let b0: Vec<DVec3> = self.pt.x.clone();
        crate::constraints::integrate(self, h);
        if std::env::var("DYN_JUMP").is_ok() {
            jump(self, "integrate", &b0);
        }
        self.stats.add("integrate", t);
        let t = std::time::Instant::now();
        let b0 = snap(self);
        crate::constraints::cloth(self, h, self.settings.iterations.clamp(1, 16));
        jump(self, "cloth", &b0);
        self.stats.add("cloth", t);
        let t = std::time::Instant::now();
        let b0 = snap(self);
        crate::constraints::attract(self, h);
        crate::constraints::links(self, h, a1);
        jump(self, "attract", &b0);
        self.stats.add("attract+links", t);
        if !self.extensions.is_empty() {
            let mut ext = std::mem::take(&mut self.extensions);
            let mut view = SubstepView { x: &self.pt.x, p: &mut self.pt.p, phase: &self.pt.phase, inv_mass: &self.im, h };
            for e in &mut ext {
                e.substep(&mut view);
            }
            self.extensions = ext;
        }
        let tk = std::time::Instant::now();
        self.stats.ms_constraints += (tk - tc).as_secs_f64() * 1e3;
        let b0 = snap(self);
        crate::contacts::solve_pairs(self, h, a0, a1);
        jump(self, "pairs", &b0);
        self.stats.add("pairs", tk);
        let t = std::time::Instant::now();
        let b0 = snap(self);
        crate::contacts::solve_colliders(self, h, a0, a1, last);
        jump(self, "colliders", &b0);
        self.stats.add("colliders", t);
        crate::rigid::shape_match(self);
        let tr = std::time::Instant::now();
        self.stats.ms_collide += (tr - tk).as_secs_f64() * 1e3;
        let b0 = snap(self);
        crate::resolve::blend(self, a1, h);
        jump(self, "blend", &b0);
        self.finish_substep(h);
        self.stats.ms_other += tr.elapsed().as_secs_f64() * 1e3;
    }

    /// Velocities from what moved, inside the limits; anything not finite goes back where it was.
    fn finish_substep(&mut self, h: f64) {
        let vmax = self.settings.max_velocity.max(1e-6);
        let bounce: Vec<f64> = self.bodies.iter().map(|b| b.settings.bounce).collect();
        let out: Vec<(DVec3, DVec3, bool, bool)> = (0..self.pt.len())
            .into_par_iter()
            .with_min_len(PAR_CHUNK)
            .map(|i| {
                let x = self.pt.x[i];
                let mut p = self.pt.p[i];
                let mut fixed = false;
                if !finite(p) {
                    p = if finite(x) { x } else { self.anim_now[i] };
                    if !finite(p) {
                        p = DVec3::ZERO;
                    }
                    fixed = true;
                }
                // Kinematic points (pinned, fully out) may jump with their target; the rest move a bounded amount.
                let free = self.im[i] > 0.0;
                if free {
                    let most = (vmax * h * 2.0).max(self.pt.radius[i] * 2.0);
                    p = x + clamp_len(p - x, most);
                }
                // Out of the simulation, a point has none of its own speed: what blending moved it by is not motion
                // to carry back in.
                let keep = self.blend_ws.get(i).copied().unwrap_or(1.0);
                let mut v = if fixed { DVec3::ZERO } else { (p - x) / h * keep };
                let clamped = v.length_squared() > vmax * vmax;
                v = clamp_len(v, vmax);
                let nrm = self.pt.sig[i].normal;
                let e = bounce[self.pt.body[i] as usize];
                if e > 0.0 && nrm != DVec3::ZERO {
                    let vin = self.v_int[i].dot(nrm);
                    if vin < 0.0 {
                        v += nrm * (-e * vin - v.dot(nrm)).max(0.0);
                    }
                }
                if !finite(v) {
                    v = DVec3::ZERO;
                }
                (p, v, fixed, clamped)
            })
            .collect();
        for (i, (p, v, fixed, clamped)) in out.into_iter().enumerate() {
            self.pt.x[i] = p;
            self.pt.p[i] = p;
            self.pt.v[i] = v;
            if fixed {
                self.pt.sig[i].nan = true;
                self.stats.fixed += 1;
            }
            if clamped {
                self.stats.clamped += 1;
            }
        }
    }

    /// Fields acting on body b at a point.
    pub(crate) fn field_accel(&self, b: u32, x: DVec3, v: DVec3, mass: f64) -> DVec3 {
        let mut a = DVec3::ZERO;
        for f in &self.fields {
            if !f.acts_on(b) {
                continue;
            }
            let (force, accel) = f.force(x, v, mass, self.time);
            if !finite(force) {
                continue;
            }
            if accel || f.kind == FieldKind::Gravity {
                a += force;
            } else if mass > 0.0 {
                a += force / mass;
            }
        }
        a
    }

    /// The fallback chosen for a vertex now.
    pub(crate) fn fallback_of(&self, i: usize) -> Fallback {
        let body = &self.bodies[self.pt.body[i] as usize];
        match body.settings.fallback {
            Fallback::Auto => {
                // The input is a sensible place only near where the vertex is: cloth on a moving body, not a drape
                // far from its starting shape.
                let near = (self.anim_target_end(i) - self.pt.x[i]).length() < body.mean_edge * 4.0 + body.settings.thickness * 4.0;
                if body.kind == BodyKind::Cloth && !self.pt.sig[i].anim_bad && near {
                    Fallback::Animation
                } else {
                    Fallback::Neighbours
                }
            }
            f => f,
        }
    }

    /// The rigid shape a body was given.
    pub fn rigid_shape(&self, b: usize) -> RigidShape {
        self.bodies[b].settings.shape
    }

    /// A body's triangle in world space now.
    pub(crate) fn tri_points(&self, b: usize, t: usize) -> Option<[DVec3; 3]> {
        let body = self.bodies.get(b)?;
        let tri = body.topo.triangles.get(t)?;
        Some(tri.map(|k| if body.kind == BodyKind::Collider { body.anim[k as usize] } else { self.pt.x[body.start + k as usize] }))
    }
}

#[allow(clippy::too_many_arguments)]
fn push(pt: &mut Particles, from: Option<(&Particles, usize)>, x: DVec3, v: DVec3, inv: f64, radius: f64, body: u32, local: u32, phase: Phase) {
    let from = from.filter(|(o, i)| *i < o.len());
    pt.x.push(from.map_or(x, |(o, i)| o.x[i]));
    pt.v.push(from.map_or(v, |(o, i)| o.v[i]));
    pt.p.push(from.map_or(x, |(o, i)| o.x[i]));
    pt.x0.push(from.map_or(x, |(o, i)| o.x0.get(i).copied().unwrap_or(o.x[i])));
    pt.v_prev_frame.push(from.map_or(v, |(o, i)| o.v_prev_frame.get(i).copied().unwrap_or(o.v[i])));
    pt.inv_mass.push(inv);
    pt.radius.push(radius);
    pt.body.push(body);
    pt.local.push(local);
    pt.phase.push(phase);
    pt.rs.push(from.map_or(Resolve::default(), |(o, i)| o.rs[i]));
    pt.sig.push(Signal::default());
    pt.sides.push(from.map_or(Sides::default(), |(o, i)| o.sides[i]));
    pt.age.push(from.map_or(0.0, |(o, i)| o.age[i]));
    pt.life.push(from.map_or(f64::INFINITY, |(o, i)| o.life[i]));
    pt.rgb.push(from.map_or(DVec3::ONE, |(o, i)| o.rgb[i]));
    pt.opacity.push(from.map_or(1.0, |(o, i)| o.opacity[i]));
    pt.id.push(from.map_or(0, |(o, i)| o.id[i]));
}
