//! What the solver moves: cloth, particles and rigid bodies are all particles with different constraints; colliders
//! are animated meshes that push them.

use std::sync::Arc;

use glam::{DMat3, DMat4, DQuat, DVec3};

use crate::emit::Emitter;
use crate::topology::{Colouring, Topology};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodyKind {
    Cloth,
    Particles,
    Rigid,
    Collider,
}

/// What a particle belongs to. Fluid is held for liquids, smoke and fire, which will join the same solve.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Cloth,
    Particle,
    Rigid,
    Fluid,
}

/// Where a vertex goes while it is out of the simulation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fallback {
    /// The animation where that is itself a possible place, else the neighbours.
    Auto,
    /// The animated input position.
    Animation,
    /// The average of its neighbours.
    Neighbours,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RigidShape {
    /// The mesh's own volume, filled with particles.
    Mesh,
    Box,
    Sphere,
    Capsule,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lifespan {
    Forever,
    Constant,
    Random,
    /// Each particle's own, set through lifespanPP.
    PerParticle,
}

/// One settings block for every kind of body; each kind reads what applies to it.
#[derive(Clone, Debug)]
pub struct BodySettings {
    pub enabled: bool,
    /// Mass of each point (cloth, particles) or of the whole body (rigid).
    pub mass: f64,
    /// Collision thickness: the soft zone kept between the body and what it touches.
    pub thickness: f64,
    /// Self collision thickness, for cloth against itself and particles against each other.
    pub self_thickness: f64,
    /// Compliance: 0 is as stiff as the steps allow; larger gives more.
    pub stretch_compliance: f64,
    pub compression_compliance: f64,
    pub bend_compliance: f64,
    /// Bending also keeps the rest fold, rather than pulling towards flat when there is one.
    pub area_compliance: Option<f64>,
    /// Fraction of velocity lost per second.
    pub damping: f64,
    pub friction: f64,
    /// Contact softness: compliance of the soft zone. 0 resolves inside the step.
    pub collide_compliance: f64,
    /// Drag on motion along the surface normal through still air, and along the surface.
    pub drag: f64,
    pub tangential_drag: f64,
    /// How strongly points follow the animated input, 0 to 1, per point by `attract_map`.
    pub attract: f64,
    pub attract_map: Option<Vec<f64>>,
    /// Points with a weight of 1 here are pinned to the input.
    pub pin_map: Option<Vec<f64>>,
    /// Per point mass scale.
    pub mass_map: Option<Vec<f64>>,
    pub collide: bool,
    pub self_collide: bool,
    /// Rest lengths and angles follow the animated input each frame, as when a skinned mesh drives the cloth.
    pub rest_from_input: bool,
    /// Edges tear when stretched past this ratio of their rest length; 0 never.
    pub tear_ratio: f64,
    /// Gravity scale for this body (0 floats).
    pub gravity_scale: f64,
    /// Particles bounce off colliders keeping this much of their normal speed.
    pub bounce: f64,
    // Impossible states.
    pub resolve: bool,
    pub fallback: Fallback,
    /// Frames to blend out to the fallback and back in.
    pub blend_frames: f64,
    /// Frames a vertex must be in a possible state before it comes back.
    pub return_frames: u32,
    /// Out longer than this, it comes back anyway from where it is, learning its sides afresh.
    pub most_frames_out: u32,
    /// Stretch past this ratio of rest length counts as impossible.
    pub stretch_limit: f64,
    // Particles.
    pub radius: f64,
    pub lifespan_mode: Lifespan,
    pub lifespan: f64,
    pub lifespan_random: f64,
    pub max_count: usize,
    // Rigid bodies.
    pub active: bool,
    pub shape: RigidShape,
    /// Particles across the rigid body's longest side.
    pub resolution: usize,
    pub initial_velocity: DVec3,
    pub initial_spin: DVec3,
}

impl Default for BodySettings {
    fn default() -> Self {
        BodySettings {
            enabled: true,
            mass: 1.0,
            thickness: 0.1,
            self_thickness: 0.1,
            stretch_compliance: 0.0,
            compression_compliance: 1e-6,
            bend_compliance: 1e-3,
            area_compliance: None,
            damping: 0.1,
            friction: 0.3,
            collide_compliance: 1e-7,
            drag: 0.05,
            tangential_drag: 0.0,
            attract: 0.0,
            attract_map: None,
            pin_map: None,
            mass_map: None,
            collide: true,
            self_collide: true,
            rest_from_input: false,
            tear_ratio: 0.0,
            gravity_scale: 1.0,
            bounce: 0.0,
            resolve: true,
            fallback: Fallback::Auto,
            blend_frames: 4.0,
            return_frames: 4,
            most_frames_out: 48,
            stretch_limit: 3.0,
            radius: 0.1,
            lifespan_mode: Lifespan::Forever,
            lifespan: 1.0,
            lifespan_random: 0.0,
            max_count: 1_000_000,
            active: true,
            shape: RigidShape::Mesh,
            resolution: 6,
            initial_velocity: DVec3::ZERO,
            initial_spin: DVec3::ZERO,
        }
    }
}

/// Cloth constraints, local to the body.
#[derive(Clone, Debug, Default)]
pub struct ClothData {
    pub dist: Vec<[u32; 2]>,
    pub dist_rest: Vec<f64>,
    pub dist_colour: Colouring,
    pub bend: Vec<[u32; 4]>,
    pub bend_rest: Vec<f64>,
    pub bend_colour: Colouring,
    pub area: Vec<[u32; 3]>,
    pub area_rest: Vec<f64>,
    pub area_colour: Colouring,
    /// Stretch limits along paths of edges between vertices several edges apart: path k is
    /// path[path_start[k]..path_start[k + 1]], its rest length along the mesh path_rest[k]. Each acts only when its
    /// two ends are further apart than that.
    pub path: Vec<u32>,
    pub path_start: Vec<u32>,
    pub path_rest: Vec<f64>,
    /// Long range attachments: each free vertex's nearest pinned vertex and the distance between them along the
    /// rest mesh; the vertex is kept within it. (vertex, anchor, distance).
    pub tether: Vec<(u32, u32, f64)>,
}

/// A rigid body's particles in its own frame, and where it is.
#[derive(Clone, Debug, Default)]
pub struct RigidData {
    /// Offsets from the centre of mass at the start.
    pub offsets: Vec<DVec3>,
    pub com0: DVec3,
    pub com: DVec3,
    pub rot: DQuat,
    /// The object's world matrix at the start.
    pub matrix0: DMat4,
}

impl RigidData {
    /// The object's world matrix now.
    pub fn matrix(&self) -> DMat4 {
        let r = DMat3::from_quat(self.rot);
        let m0 = self.matrix0;
        let t0 = m0.w_axis.truncate();
        let lin = r * DMat3::from_mat4(m0);
        let t = self.com + r * (t0 - self.com0);
        DMat4::from_cols(lin.x_axis.extend(0.0), lin.y_axis.extend(0.0), lin.z_axis.extend(0.0), t.extend(1.0))
    }
}

/// A body in the solver.
#[derive(Clone, Debug)]
pub struct Body {
    pub kind: BodyKind,
    pub name: String,
    pub settings: BodySettings,
    pub topo: Arc<Topology>,
    /// Its particles' range in the solver's arrays.
    pub start: usize,
    pub count: usize,
    /// The animated input at the start and end of the frame being solved, world space, one per mesh point.
    pub anim_prev: Vec<DVec3>,
    pub anim: Vec<DVec3>,
    /// Rest positions: the input at the start frame.
    pub rest: Vec<DVec3>,
    pub cloth: Arc<ClothData>,
    /// Torn edges, by distance constraint.
    pub torn: Vec<bool>,
    pub emitters: Vec<Emitter>,
    pub rigid: Option<RigidData>,
    /// The input jumped or changed topology: start again from it.
    pub reset_pending: bool,
    /// Average polygon edge at rest.
    pub mean_edge: f64,
    /// Rigid bodies: each particle's radius.
    pub radii: Vec<f64>,
    /// A closed collider's outside: +1 along its normals, -1 when they point in. None for an open mesh, whose
    /// sides each particle learns.
    pub outside: Option<f64>,
    pub next_id: u64,
    /// Particles added by script for the next frame: position, velocity.
    pub pending: Vec<(DVec3, DVec3)>,
}

impl Body {
    pub fn is_dynamic(&self) -> bool {
        self.kind != BodyKind::Collider
    }
    pub fn phase(&self) -> Phase {
        match self.kind {
            BodyKind::Cloth => Phase::Cloth,
            BodyKind::Rigid => Phase::Rigid,
            _ => Phase::Particle,
        }
    }
    pub(crate) fn map(&self, m: &Option<Vec<f64>>, local: usize, default: f64) -> f64 {
        m.as_ref().and_then(|v| v.get(local).copied()).filter(|x| x.is_finite()).unwrap_or(default)
    }
}
