//! Harold: a physically based, unidirectional path tracer. Brute force Monte Carlo with next event estimation and
//! multiple importance sampling, one standard surface material, sample counts per ray type, progressive rendering.
//! Pure Rust on the CPU; it knows nothing of the scene graph, which translates into `Scene`.

pub mod bsdf;
pub mod bvh;
pub mod camera;
pub mod geom;
pub mod math;
pub mod shader;
pub mod texture;
pub mod first_hit;
pub mod integrator;
pub mod lights;
pub mod path;
pub mod scene;
pub mod shade;
pub mod film;
pub mod procedural;
pub mod render;
pub mod denoise;
pub mod output;
