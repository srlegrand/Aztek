//! Transforms and the shapes under them, as the reference pairs them: `locator1` over `locatorShape1`, `pCube1` over
//! `pCubeShape1`.

use aztek_ops::CmdResult;

use crate::graph::Graph;
use crate::node::types;
use crate::plug::{NodeId, NO_NODE};

/// The shape name the reference gives under a transform: `pCube1` -> `pCubeShape1`, `foo` -> `fooShape`.
pub fn shape_name(transform: &str) -> String {
    // The namespace stays: ns:pCube1 -> ns:pCubeShape1.
    let short = transform.rsplit('|').next().unwrap_or(transform);
    let digits = short.len() - short.bytes().rev().take_while(u8::is_ascii_digit).count();
    format!("{}Shape{}", &short[..digits], &short[digits..])
}

/// The transform stem a shape type is created under when nothing names it.
pub fn transform_stem(shape_type: &str) -> &str {
    match shape_type {
        "mesh" => "polySurface",
        "nurbsCurve" => "curve",
        "nurbsSurface" => "nurbsSurface",
        other => other,
    }
}

pub fn is_shape(g: &Graph, id: NodeId) -> bool {
    g.node(id).is_some_and(|n| n.ty().is_a("shape"))
}

pub fn is_transform(g: &Graph, id: NodeId) -> bool {
    g.node(id).is_some_and(|n| n.ty().is_a("transform"))
}

/// The shapes directly under a transform, intermediate objects included when asked.
pub fn shapes(g: &Graph, transform: NodeId, intermediate: bool) -> Vec<NodeId> {
    g.children(transform)
        .iter()
        .copied()
        .filter(|&c| {
            g.node(c).is_some_and(|n| {
                n.ty().is_a("shape") && (intermediate || !n.bool(crate::transform::DagNode::INTERMEDIATE_OBJECT))
            })
        })
        .collect()
}

/// The first visible shape of a given type under a transform, or the node itself when it is such a shape.
pub fn shape_of_type(g: &Graph, id: NodeId, ty: &str) -> Option<NodeId> {
    let n = g.node(id)?;
    if n.ty().is_a(ty) {
        return Some(id);
    }
    if !n.ty().is_a("transform") {
        return None;
    }
    shapes(g, id, false).into_iter().find(|&s| g.node(s).is_some_and(|n| n.ty().is_a(ty)))
}

/// The transform a shape hangs under, or the node itself when it is not a shape.
pub fn transform_of(g: &Graph, id: NodeId) -> NodeId {
    if is_shape(g, id) {
        let p = g.parent(id);
        if p != NO_NODE {
            return p;
        }
    }
    id
}

/// Creates a transform (named `name`, or after the shape type when empty) under `parent`, and a shape of
/// `shape_type` under it named after the transform. Returns (transform, shape).
pub fn create_with_shape(g: &mut Graph, shape_type: &str, name: &str, parent: NodeId) -> CmdResult<(NodeId, NodeId)> {
    let tt = types::find("transform").expect("transform");
    let st = types::find(shape_type).ok_or_else(|| aztek_ops::CommandError::new(format!("unknown node type '{shape_type}'")))?;
    let wanted = if name.is_empty() { format!("{}1", transform_stem(shape_type)) } else { name.to_string() };
    // A name the scene already uses takes its next free number.
    let tr = g.create_node(&tt, &wanted, parent, true)?;
    let sh_name = shape_name(&g.node(tr).unwrap().name().to_string());
    let sh = g.create_node(&st, &sh_name, tr, false)?;
    Ok((tr, sh))
}
