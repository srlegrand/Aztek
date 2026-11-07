
use aztek_geo::mesh::Bounds;
use glam::DVec3;

use crate::components;
use crate::graph::Graph;
use crate::mesh_nodes::MeshNode;
use crate::plug::NodeId;
use crate::transform::Transform;

/// Object-space bounds of a node's own geometry: a mesh's points, or a point provider's components (a curve's points
/// and handles). A transform has its shapes' bounds. Invalid for nodes with none, such as groups and locators. Reads
/// evaluated values.
pub fn object_bounds(g: &mut Graph, id: NodeId) -> Bounds {
    let mut b = Bounds::default();
    if crate::dag::is_transform(g, id) {
        for sh in crate::dag::shapes(g, id, false) {
            let s = object_bounds(g, sh);
            if s.valid() {
                b.extend(s.min);
                b.extend(s.max);
            }
        }
        return b;
    }
    let Some(n) = g.node(id) else { return b };
    if let Some(points) = components::point_provider(n).and_then(|pp| pp.points) {
        for (_, p) in points(g, id) {
            b.extend(p);
        }
        return b;
    }
    if n.ty().is_a("mesh") {
        if let Some(m) = n.data().mesh(MeshNode::OUT_MESH) {
            if m.num_points() > 0 {
                b = m.bounds();
            }
        }
    }
    b
}

/// The eight corners of `local` through `m`.
pub fn transformed(local: &Bounds, m: &glam::DMat4) -> Bounds {
    let mut out = Bounds::default();
    if !local.valid() {
        return out;
    }
    for c in 0..8 {
        let corner = DVec3::new(
            if c & 1 != 0 { local.max.x } else { local.min.x },
            if c & 2 != 0 { local.max.y } else { local.min.y },
            if c & 4 != 0 { local.max.z } else { local.min.z },
        );
        out.extend(m.transform_point3(corner));
    }
    out
}

/// object_bounds' eight corners through the node's world matrix.
pub fn world_bounds(g: &mut Graph, id: NodeId) -> Bounds {
    let local = object_bounds(g, id);
    if !local.valid() {
        return Bounds::default();
    }
    let w = g.node(id).unwrap().matrix(Transform::WORLD_MATRIX);
    transformed(&local, &w)
}
