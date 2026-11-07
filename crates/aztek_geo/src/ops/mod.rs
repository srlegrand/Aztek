//! Modeling operations. Each works on a component selection and returns the new selection. Ids of untouched elements
//! stay stable; removed elements are only marked deleted, so call Mesh::compact() when convenient. Attributes are
//! carried along: corner uvs and colours are copied or interpolated, edge "hard"/"seam"/"crease" follow the edges they
//! came from, face attributes follow the face.
//!
//! Ops taking a SymmetryMap run on the selection plus its mirror (every radial image) and keep on-plane points on the
//! plane (on the radial axis or a mirror plane).

pub mod basic;
pub mod bevel;
pub mod bridge;
pub mod common;
pub mod extrude;
pub mod grid_fill;
pub mod knife;
pub mod loops;
pub mod poly_tools;
pub mod split_ngons;
pub mod subdivide;

pub use basic::*;
pub use bevel::*;
pub use bridge::*;
pub use extrude::*;
pub use grid_fill::*;
pub use knife::*;
pub use loops::*;
pub use poly_tools::*;
pub use split_ngons::*;
pub use subdivide::*;
