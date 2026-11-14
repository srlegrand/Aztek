//! Curves and surfaces with deformers on them, which rigs lean on: a ribbon skinned to joints, a spline IK curve skinned to its
//! controls, eyelid curves with blend shapes and wires. See `nurbs_nodes` for how they go through the deformers.

use super::*;
use crate::nurbs_nodes as nn;

/// What feeds a curve or surface shape, walking back from `.cr` through its deformers.
#[derive(Default)]
struct NurbsChain {
    /// The shape holding the geometry the deformers read: the reference's Orig.
    base: Option<usize>,
    /// Deformers with the geometry index they output, source first; a tweak among them is CV moves.
    deformers: Vec<(usize, i64)>,
}

fn walk(doc: &Doc, shape: usize) -> NurbsChain {
    const IN: &[&str] = &["cr", "create", "ip.ig", "input.inputGeometry", "ig", "inputGeometry"];
    let mut c = NurbsChain::default();
    let mut seen = HashSet::new();
    let mut at = shape;
    let mut through = 0;
    while seen.insert(at) {
        let plain = |k: &&Conn| k.dst == at && IN.contains(&plain_attr(&k.dst_attr).as_str());
        let by_index = doc.conns.iter().filter(plain).find(|k| first_index(&k.dst_attr).unwrap_or(0) == through);
        let Some(conn) = by_index.or_else(|| doc.conns.iter().find(plain)) else { break };
        let next = conn.src;
        let index = first_index(&conn.src_attr).unwrap_or(0);
        let ty = doc.nodes[next].ty.as_str();
        match ty {
            "nurbsCurve" | "nurbsSurface" => {
                c.base = Some(next);
                break;
            }
            "skinCluster" => c.deformers.insert(0, (next, index)),
            // A plug-in's deformer on a curve keeps the curve itself: read by `curve_deformers`.
            t if crate::scripted::is_plugin_deformer(t) => break,
            t if deformers::is_file_deformer(t) => c.deformers.insert(0, (next, index)),
            "tweak" => c.deformers.insert(0, (next, index)),
            "groupParts" | "groupId" => {}
            // A creator or anything else: not a deformed shape as far as this walk goes.
            _ => break,
        }
        through = if ty == "skinCluster" || ty == "tweak" || deformers::is_file_deformer(ty) { index } else { through };
        at = next;
    }
    c
}

impl Build<'_> {
    /// A tweak node's CV moves for one geometry, as a tweak in the chain.
    fn nurbs_tweak(&mut self, d: usize, geometry: i64) -> Option<NodeId> {
        let moves = deformers::tweak_moves(self.doc, d, geometry);
        self.used[d] = true;
        crate::history::register_tweak_type();
        let t = types::find(crate::history::TWEAK_TYPE)?;
        let id = self.s.graph.create_node(&t, "", NO_NODE, false).ok()?;
        let points: Vec<Value> = moves.into_iter().map(|(at, v)| json!([at, v.x, v.y, v.z])).collect();
        let _ = crate::history::set_tweaks_json(&mut self.s.graph, id, Some(&json!({"points": points})));
        Some(id)
    }

    /// What a blend shape reads from a live target: a curve or surface goes through nurbsToPoints.
    pub(super) fn target_source(&mut self, src: NodeId, from: String) -> String {
        let name = self.s.graph.node(src).map(|n| n.ty().name.clone()).unwrap_or_default();
        // A mesh's `.w` is its worldMesh in the file: the same points, read here from its output.
        if name == "mesh" {
            return format!("{}.outMesh", self.s.graph.path(src));
        }
        let local = match name.as_str() {
            "nurbsCurve" => crate::curves::CurveShape::LOCAL,
            "nurbsSurface" => crate::surfaces::SurfaceShape::LOCAL,
            _ => return from,
        };
        let Some(to) = types::find(nn::TO_POINTS) else { return from };
        let g = &mut self.s.graph;
        let Ok(id) = g.create_node(&to, "", NO_NODE, false) else { return from };
        let _ = g.connect(&PlugRef::new(src, local as i32), &PlugRef::new(id, 0), false);
        format!("{}.outMesh", g.path(id))
    }

    /// Wires a deformed curve or surface: base -> nurbsToPoints -> deformers -> pointsToNurbs -> shape. False when the shape
    /// has no deformers, which leaves it to the plain path.
    pub(super) fn chain_nurbs(&mut self, i: usize, local: usize, create: usize) -> bool {
        let doc = self.doc;
        let chain = walk(doc, i);
        if chain.deformers.is_empty() {
            return false;
        }
        let Some(shape) = self.id(i) else { return false };
        let Some(base_id) = chain.base.and_then(|b| self.id(b)) else {
            self.warnings.push(format!("{}: deformed, with no original shape in the file", doc.nodes[i].name));
            return false;
        };
        let (Some(to), Some(from)) = (types::find(nn::TO_POINTS), types::find(nn::FROM_POINTS)) else { return false };
        let g = &mut self.s.graph;
        let (Ok(to_id), Ok(from_id)) = (g.create_node(&to, "", NO_NODE, false), g.create_node(&from, "", NO_NODE, false)) else { return false };
        let base_out = PlugRef::new(base_id, local as i32);
        let _ = g.connect(&base_out, &PlugRef::new(to_id, 0), false);
        let _ = g.connect(&base_out, &PlugRef::new(from_id, 0), false);
        let mut prev = PlugRef::new(to_id, 1);
        for &(d, k) in &chain.deformers {
            let made = match doc.nodes[d].ty.as_str() {
                skin::SKIN_TYPE => self.create_skin(d),
                "tweak" => self.nurbs_tweak(d, k),
                _ => self.create_deformer(d, k, shape),
            };
            let Some(id) = made else { continue };
            let t = self.s.graph.node(id).unwrap().type_arc().clone();
            let (ip, out) = (t.attr_index("inputPolymesh").unwrap(), t.attr_index("output").unwrap());
            let _ = self.s.graph.connect(&prev, &PlugRef::new(id, ip as i32), false);
            prev = PlugRef::new(id, out as i32);
        }
        let g = &mut self.s.graph;
        let _ = g.connect(&prev, &PlugRef::new(from_id, 1), false);
        let _ = g.connect(&PlugRef::new(from_id, 2), &PlugRef::new(shape, create as i32), false);
        true
    }
}

impl Writer<'_> {
    /// A deformed curve or surface: its original's `.ws` through the deformers to `.cr`.
    pub(super) fn write_nurbs_chain(&mut self, shape: NodeId, create: usize) {
        let g = &self.s.graph;
        let Some(from) = g.source(&PlugRef::new(shape, create as i32)).filter(|s| g.node(s.node).is_some_and(|n| n.ty().name == nn::FROM_POINTS)) else { return };
        let mut chain = Vec::new();
        let mut at = PlugRef::new(from.node, 1);
        let to = loop {
            let Some(src) = g.source(&at) else { return };
            let Some(n) = g.node(src.node) else { return };
            if n.ty().name == nn::TO_POINTS {
                break src.node;
            }
            chain.insert(0, src.node);
            let Some(ip) = n.ty().attr_index("inputPolymesh") else { return };
            at = PlugRef::new(src.node, ip as i32);
        };
        let Some(orig) = g.source(&PlugRef::new(to, 0)) else { return };
        let count = g.node(to).and_then(|n| n.data().value(1).as_mesh().map(|m| m.point_capacity())).unwrap_or(0) as usize;
        let stored = Stored { points: vec![DVec3::ZERO; count], point_of: (0..count as i64).collect(), edges: Vec::new(), faces: Vec::new(), corners: 0, uv_sets: Vec::new(), uv_names: Vec::new(), uv_current: String::new(), tags: Vec::new() };
        let (first, shape_path) = (format!("{}.ws", g.path(orig.node)), g.path(shape));
        let last = self.write_chain_from(&chain, &shape_path, first, &stored, true);
        self.conn(&last, &format!("{shape_path}.cr"));
    }
}
