//! Aztek's dynamics: one solver for cloth, particles and rigid bodies, which all collide with and push each other.
//!
//! Everything is particles under extended position based dynamics, stepped in small substeps. Cloth is particles held
//! by stretch, bend and area constraints; particles are free; rigid bodies are particles held in their shape;
//! colliders are animated meshes. Fields push any of them. The solver never takes a step it cannot bound, and a
//! vertex it cannot put anywhere sensible leaves the simulation for a while (`resolve`).
//!
//! Phases leave room for liquids, smoke and fire to join the same solve through `Extension`.

pub mod body;
pub mod cache;
pub mod constraints;
pub mod contacts;
pub mod emit;
pub mod fields;
pub mod geom;
pub mod hash;
pub mod resolve;
pub mod rigid;
pub mod solver;
pub mod topology;
pub mod util;

pub use body::{Body, BodyKind, BodySettings, Fallback, Lifespan, Phase, RigidShape};
pub use emit::{Emitter, EmitterKind};
pub use fields::{Field, FieldKind};
pub use solver::{Anchor, Extension, Link, Mode, Solver, SolverSettings, Stats, SubstepView};
