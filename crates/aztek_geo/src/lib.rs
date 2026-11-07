//! The geometry module (`src/geo`) in Rust: the mesh, its attributes, and the operations on them.

pub mod array;
pub mod attribute;
pub mod bounding;
pub mod bvh;
pub mod colors;
pub mod component_paths;
pub mod conform;
pub mod cross_section;
pub mod fold_edges;
pub mod mesh;
pub mod heightfield;
pub mod io_common;
pub mod lasso;
pub mod loop_tools;
pub mod mix;
pub mod multibox;
pub mod normals;
pub mod nurbs;
pub mod obj;
pub mod ops;
pub mod par;
pub mod ply;
pub mod point_weld;
pub mod primitives;
pub mod reduce;
pub mod render_mesh;
pub mod smart_join;
pub mod stl;
pub mod stroke_font;
pub mod symmetry;
pub mod topology;
pub mod triangulate;

pub use attribute::{names, AttrType, AttrValue, Attribute, AttributeSet, Domain, Index, INVALID};
pub use mesh::{validate, Bounds, ColorSetInfo, Mesh, MeshRemap};
