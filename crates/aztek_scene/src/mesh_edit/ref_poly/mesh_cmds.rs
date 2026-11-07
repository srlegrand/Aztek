//! The Mesh menu: combine, separate, booleans, smooth, reduce, remesh, retopo, mirror, transfer, cleanup, and the
//! commands that build faces.

use std::any::Any;
use std::collections::BTreeSet;
use std::sync::Arc;

use aztek_geo::bvh::MeshBvh;
use aztek_geo::normals::{corner_normals, CornerNormalOptions};
use aztek_geo::ops::*;
use aztek_geo::reduce::{reduce_mesh, ReduceOptions};
use aztek_geo::symmetry::{remove_symmetry_layers, SymmetryMap, SymmetryPlane};
use aztek_geo::{colors, names, Index, Mesh, INVALID};
use aztek_ops::{execute, fail, ArgType as T, Args, CallSource, CmdResult, Commands};
use glam::{DMat3, DMat4, DVec3};
use serde_json::{json, Value};

use super::*;
use crate::attr_defs::{matrix_attr, mesh_attr};
use crate::components::{self, ComponentMode as K, Ids};
use crate::graph::SelectMode;
use crate::history::{register_op, HistoryOp};
use crate::mesh_boolean::{mesh_boolean, BooleanOp, BooleanOperand, BooleanOptions};
use crate::mesh_edit::{faces, EditResult};
use crate::mesh_nodes::MeshNode;
use crate::node::{types, EvalContext, NodeBehavior, NodeData, NodeType};
use crate::plug::{flags, AttrDef, PlugRef, PlugType, PlugValue, NO_NODE};
use crate::transform::DagNode;

fn plug(n: NodeId, attr: usize) -> PlugRef {
    PlugRef::new(n, attr as i32)
}

// ---------------------------------------------------------------------------------------------------------------------
// Whole-mesh ops

fn smooth_op(a: &Args, m: &mut Mesh, sel: &Ids, _sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    let levels = a.int("divisions", 1).clamp(0, 6) as i32;
    if levels == 0 {
        return Ok(EditResult::default());
    }
    let whole = sel.is_empty() || sel.len() as Index >= m.num_faces();
    let linear = a.int("method", 0) == 1;
    if whole && !linear {
        let o = SubdivideOptions { levels, sharp_boundary: on(a, "keepBorder", true), keep_hard_edges: a.bool("keepHardEdge") };
        *m = subdivide(m, &o, None);
        return Ok(EditResult::default());
    }
    let list = if sel.is_empty() { components::all_ids(m, K::Face) } else { sel.clone() };
    let per_level = if linear { a.int("divisionsPerEdge", 1).max(1) as i32 } else { 1 };
    let made = subdivide_facets(m, &list, levels * per_level, false);
    if !linear {
        // Part of a mesh: the points strictly inside the faces relax toward their neighbours.
        let inside: BTreeSet<Index> = made.iter().filter(|&&f| m.face_valid(f)).flat_map(|&f| m.face_points(f)).collect();
        let keep_border = on(a, "keepSelectionBorder", true);
        let set: BTreeSet<Index> = made.iter().copied().collect();
        let moved: Vec<(Index, DVec3)> = inside
            .into_iter()
            .filter(|&p| !m.is_boundary_point(p) && (!keep_border || m.point_faces(p).iter().all(|f| set.contains(f))))
            .map(|p| {
                let mut sum = DVec3::ZERO;
                let mut n = 0.0;
                m.for_each_point_edge(p, |e| {
                    sum += m.position(m.edge_other_point(e, p));
                    n += 1.0;
                });
                (p, m.position(p) * 0.5 + sum / f64::max(n, 1.0) * 0.5)
            })
            .collect();
        for (p, at) in moved {
            m.set_position(p, at);
        }
    }
    Ok(faces(made))
}

fn reduce_op(a: &Args, m: &mut Mesh, _sel: &Ids, _sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    let tris_now: i64 = (0..m.face_capacity()).filter(|&f| m.face_valid(f)).map(|f| m.face_size(f) as i64 - 2).sum();
    let term = a.int("termination", 0);
    let percent = a.float("percentage", 0.0);
    let mut o = ReduceOptions {
        ratio: if percent > 0.0 { (1.0 - percent / 100.0).clamp(0.0, 1.0) } else { a.float("ratio", 0.5).clamp(0.0, 1.0) },
        keep_borders: on(a, "keepBorder", true) && on(a, "keepBorders", true),
        keep_hard_edges: on(a, "keepHardEdge", true) && on(a, "keepHardEdges", true),
        quads: a.bool("quads") || !on(a, "triangulate", true),
        ..Default::default()
    };
    match term {
        1 => o.target_triangles = (a.int("vertexCount", 0).max(0) * 2).clamp(1, tris_now.max(1)) as Index,
        2 => o.target_triangles = a.int("triangleCount", 0).clamp(1, tris_now.max(1)) as Index,
        _ => o.target_triangles = a.int("targetTriangles", 0).max(0) as Index,
    }
    if a.get("useVirtualSymmetry").and_then(Value::as_i64).unwrap_or(0) != 0 {
        let n = DVec3::new(a.float("symmetryPlaneX", 1.0), a.float("symmetryPlaneY", 0.0), a.float("symmetryPlaneZ", 0.0)).abs();
        o.symmetry_axis = (0..3).max_by(|&i, &j| n[i].total_cmp(&n[j])).unwrap_or(0) as i32;
        o.symmetry_offset = a.float("symmetryPlaneW", 0.0);
    }
    let r = reduce_mesh(m, &o);
    *m = r.mesh;
    Ok(EditResult::default())
}

fn average_edge(m: &Mesh, list: &[Index]) -> f64 {
    let mut seen = BTreeSet::new();
    for &f in list {
        if m.face_valid(f) {
            for c in m.face_corners(f) {
                seen.insert(m.corner_edge(c));
            }
        }
    }
    if seen.is_empty() {
        return 1.0;
    }
    seen.iter().map(|&e| m.edge_length(e)).sum::<f64>() / seen.len() as f64
}

fn remesh_op(a: &Args, m: &mut Mesh, sel: &Ids, _sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    let list = if sel.is_empty() { components::all_ids(m, K::Face) } else { sel.clone() };
    let mel = a.float("maxEdgeLength", 1.0).max(1e-6);
    let max_edge = if on(a, "useRelativeValues", true) { mel * average_edge(m, &list) } else { mel };
    let o = RemeshOptions {
        max_edge,
        collapse: (a.float("collapseThreshold", 20.0) / 100.0).clamp(0.0, 0.9),
        smooth: (a.float("smoothStrength", 0.0) / 100.0).clamp(0.0, 1.0),
        tessellate_borders: on(a, "tessellateBorders", true),
        passes: 4,
    };
    Ok(faces(remesh(m, &list, &o)))
}

fn retopo_op(a: &Args, m: &mut Mesh, sel: &Ids, _sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    let list = if sel.is_empty() { components::all_ids(m, K::Face) } else { sel.clone() };
    Ok(faces(retopo(m, &list, a.int("targetFaceCount", 1000), 60.0)))
}

fn mirror_op(a: &Args, m: &mut Mesh, _sel: &Ids, _sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    let mut axis = a.get("axis").and_then(Value::as_i64).unwrap_or(0).clamp(0, 2) as usize;
    let mut positive = a.int("axisDirection", 1) != 0;
    let d = a.get("direction").and_then(Value::as_i64).unwrap_or(0);
    if d > 0 {
        axis = (d / 2).clamp(0, 2) as usize;
        positive = d % 2 == 0;
    }
    let mut normal = DVec3::ZERO;
    normal[axis] = 1.0;
    let world = object_to_world(a);
    let mps = a.float("mirrorPosition", 0.0);
    let (offset, space) = match a.int("mirrorAxis", 2) {
        // Bounding box: the side it mirrors toward.
        0 => {
            let b = m.bounds();
            ((if positive { b.max[axis] } else { b.min[axis] }) + mps, DMat4::IDENTITY)
        }
        1 => (vec_flag(a, "pivot", DVec3::ZERO)[axis] + mps, DMat4::IDENTITY),
        _ => (mps, stored_world(a)),
    };
    let _ = world;
    let cut = a.bool("cutMesh");
    let o = MirrorOptions {
        plane: SymmetryPlane { normal, offset },
        // Cutting keeps the side away from the direction and rebuilds the other from it.
        direction: match (cut, positive) {
            (false, _) => MirrorDirection::Both,
            (true, true) => MirrorDirection::Negative,
            (true, false) => MirrorDirection::Positive,
        },
        space,
        weld: a.int("mergeMode", 1) != 0,
        tolerance: a.float("mergeThreshold", 0.001).max(0.0),
    };
    let made = mirror_mesh(m, &o);
    remove_symmetry_layers(m);
    if a.bool("flipUVs") {
        let corners: Vec<Index> = made.iter().filter(|&&f| m.face_valid(f)).flat_map(|&f| m.face_corners(f)).collect();
        if m.corner_attrs().has(names::uv(0).as_str()) {
            let uv = m.corner_attrs_mut().get_mut::<[f32; 2]>(names::uv(0).as_str());
            for c in corners {
                uv[c as usize][0] = 1.0 - uv[c as usize][0];
            }
        }
    }
    Ok(faces(made))
}

/// The object's world matrix stored with the edit, whether or not worldSpace was asked for.
fn stored_world(a: &Args) -> DMat4 {
    let Some(list) = a.get("worldInverse").and_then(Value::as_array) else { return DMat4::IDENTITY };
    let v: Vec<f64> = list.iter().filter_map(Value::as_f64).collect();
    <[f64; 16]>::try_from(v).map_or(DMat4::IDENTITY, |c| DMat4::from_cols_array(&c).inverse())
}

fn shell_op(a: &Args, m: &mut Mesh, _sel: &Ids, _sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    let parts = separate(m);
    let i = a.int("shell", 0).max(0) as usize;
    let mut part = parts.into_iter().nth(i).unwrap_or_default();
    part.compact(None);
    *m = part;
    Ok(EditResult::default())
}

pub(super) fn register_ops() {
    let op = |node_type: &'static str, command: &'static str, label: &'static str, kind: K, run: crate::history::OpFn, doc: &'static str| {
        register_op(HistoryOp { node_type, command, label, kind, run, skip_flags: &[], doc })
    };
    op("polySmoothFace", "polySmooth", "Smooth", K::Face, smooth_op, "Subdivides and smooths faces.");
    op("polyReduce", "polyReduce", "Reduce", K::Face, reduce_op, "Lowers the polygon count, keeping the shape.");
    op("polyRemesh", "polyRemesh", "Remesh", K::Face, remesh_op, "Rebuilds faces as even triangles.");
    op("polyRetopo", "polyRetopo", "Retopologize", K::Face, retopo_op, "Rebuilds the surface as even, quad-dominant faces.");
    op("polyMirror", "polyMirrorFace", "Mirror", K::Face, mirror_op, "Adds a mirrored copy, welded along the plane.");
    op("polySeparate", "polySeparate", "Separate", K::Face, shell_op, "Keeps one shell of its input mesh.");
}

// ---------------------------------------------------------------------------------------------------------------------
// Nodes with several meshes in

/// polyUnite, polyCBoolOp, polyBoolOp: their output is a function of their inputPoly and inputMat arrays.
struct MultiInput {
    boolean: bool,
}

const IN_POLY: usize = crate::history::COUNT;
const IN_MAT: usize = crate::history::COUNT + 1;
const OPERATION: usize = crate::history::COUNT + 2;

impl NodeBehavior for MultiInput {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        let meshes: Vec<(i32, crate::plug::MeshPtr)> = d.elements(IN_POLY).iter().filter_map(|(i, v)| v.as_mesh().map(|m| (*i, m.clone()))).collect();
        let matrix = |d: &NodeData, i: i32| match d.element(IN_MAT, i) {
            Some(PlugValue::Matrix(m)) => *m,
            _ => DMat4::IDENTITY,
        };
        if self.boolean {
            let operands: Vec<BooleanOperand> = meshes.iter().map(|(i, m)| BooleanOperand { mesh: m, to_world: matrix(d, *i) }).collect();
            if operands.is_empty() {
                d.set_output(crate::history::OUTPUT, PlugValue::Mesh(None));
                return;
            }
            let o = BooleanOptions {
                op: match d.int(OPERATION) {
                    2 => BooleanOp::Difference,
                    3 => BooleanOp::Intersection,
                    _ => BooleanOp::Union,
                },
                ..Default::default()
            };
            let r = mesh_boolean(&operands, &DMat4::IDENTITY, &o);
            if !r.ok() {
                log::warn!("polyCBoolOp: {}", r.error);
            }
            d.set_output(crate::history::OUTPUT, PlugValue::Mesh(Some(Arc::new(r.mesh))));
            return;
        }
        let mut world = Vec::new();
        for (i, m) in &meshes {
            let w = matrix(d, *i);
            let mut c = (**m).clone();
            for p in 0..c.point_capacity() {
                if c.point_valid(p) {
                    c.set_position(p, w.transform_point3(c.position(p)));
                }
            }
            if DMat3::from_mat4(w).determinant() < 0.0 {
                let all = components::all_ids(&c, K::Face);
                flip_normals(&mut c, &all);
            }
            world.push(c);
        }
        let refs: Vec<&Mesh> = world.iter().collect();
        d.set_output(crate::history::OUTPUT, PlugValue::Mesh(Some(Arc::new(combine(&refs)))));
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn multi_input_type(name: &str, boolean: bool, doc: &str) -> NodeType {
    let int_attr = |n: &str, short: &str, def: i64| AttrDef {
        name: n.into(),
        short_name: short.into(),
        ty: PlugType::Int,
        default_value: PlugValue::Int(def),
        flags: flags::INPUT | flags::STORABLE | flags::KEYABLE,
        ..Default::default()
    };
    NodeType {
        name: name.into(),
        base: types::find("polyModifier"),
        doc: doc.into(),
        own_attrs: vec![
            AttrDef { flags: flags::INPUT | flags::ARRAY | flags::HIDDEN, ..mesh_attr("inputPoly", "ipo", flags::INPUT, "The meshes.") },
            AttrDef { flags: flags::INPUT | flags::ARRAY | flags::HIDDEN, ..matrix_attr("inputMat", flags::INPUT, "Each mesh's world matrix.") },
            int_attr("operation", "op", 1),
            int_attr("mergeUVSets", "muv", 1),
        ],
        create: Some(Arc::new(move || Box::new(MultiInput { boolean }))),
        ..Default::default()
    }
}

/// The node types here that aren't history ops. Registered once polyModifier exists.
pub(crate) fn register_types() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        crate::history::ensure_base_type();
        for (name, boolean, doc) in [
            ("polyUnite", false, "Combines its input meshes into one."),
            ("polyCBoolOp", true, "A boolean of its input meshes."),
            ("polyBoolOp", true, "A boolean of its input meshes."),
        ] {
            types::register_type(multi_input_type(name, boolean, doc)).expect(name);
        }
        register_transfer_type();
        types::register_type(gen_type("polyCreateFace", "A face through its points.")).expect("polyCreateFace");
    });
}

/// A new mesh fed by a multi-input node, the sources kept as the hidden shapes feeding it. Returns [transform, node].
fn multi_input(s: &mut Session, a: &Args, node_type: &str, shapes: &[NodeId], operation: i64, default_name: &str) -> CmdResult {
    let name = a.string("name", default_name);
    let g = &mut s.graph;
    g.evaluate();
    g.open_chunk(node_type);
    let r = (|| -> CmdResult {
        let t = types::find(node_type).ok_or_else(|| aztek_ops::CommandError::new(format!("no node type {node_type}")))?;
        let h = g.create_node(&t, "", NO_NODE, true)?;
        g.set_value(plug(h, OPERATION), PlugValue::Int(operation))?;
        for (i, &m) in shapes.iter().enumerate() {
            let xf = crate::dag::transform_of(g, m);
            g.connect(&plug(m, MeshNode::OUT_MESH), &PlugRef::element(h, IN_POLY as i32, i as i32), false)?;
            g.connect(&plug(xf, Transform::WORLD_MATRIX), &PlugRef::element(h, IN_MAT as i32, i as i32), false)?;
            g.set_value(plug(m, DagNode::INTERMEDIATE_OBJECT), PlugValue::Bool(true))?;
        }
        let (xf, shape) = crate::mesh_nodes::create_mesh_node(g, &name)?;
        g.connect(&plug(h, crate::history::OUTPUT), &plug(shape, MeshNode::IN_MESH), false)?;
        g.select(&[xf], SelectMode::Replace);
        components::clear(g);
        Ok(json!([g.path(xf), g.path(h)]))
    })();
    g.close_chunk();
    r
}

fn shapes_named(s: &Session, a: &Args, cmd: &str) -> CmdResult<Vec<NodeId>> {
    let names = crate::mesh_nodes::mesh_objects(s, a, cmd)?;
    let mut out = Vec::new();
    for n in names {
        out.push(crate::mesh_edit::mesh_arg(&s.graph, &n, cmd)?);
    }
    Ok(out)
}

fn boolean_cmd(name: &'static str, node_type: &'static str, doc: &str) -> Cmd {
    ref_cmd(name, doc, &[("operation", json!(1))], move |s, a| {
        let shapes = shapes_named(s, a, name)?;
        if shapes.len() < 2 {
            fail!("{name}: give at least two meshes");
        }
        let op = a.get("operation").and_then(|v| v.as_i64().or_else(|| v.as_array().and_then(|l| l.first()).and_then(Value::as_i64))).unwrap_or(1);
        multi_input(s, a, node_type, &shapes, op, "polySurface1")
    })
}

// ---------------------------------------------------------------------------------------------------------------------
// transferAttributes

const SOURCE: usize = crate::history::COUNT;
const SOURCE_MAT: usize = crate::history::COUNT + 1;
const TARGET_MAT: usize = crate::history::COUNT + 2;
const POS: usize = crate::history::COUNT + 3;
const NML: usize = crate::history::COUNT + 4;
const UVS: usize = crate::history::COUNT + 5;
const COL: usize = crate::history::COUNT + 6;
const SPACE: usize = crate::history::COUNT + 7;
const METHOD: usize = crate::history::COUNT + 8;
const FLIP_UV: usize = crate::history::COUNT + 9;

struct TransferNode;

/// One sample of the source: the corners of a triangle with their weights.
fn triangle_at(bvh: &MeshBvh, tri: Index, bary: DVec3) -> [(Index, f64); 3] {
    let c = &bvh.triangles().corners;
    let t = tri as usize * 3;
    [(c[t], bary.x), (c[t + 1], bary.y), (c[t + 2], bary.z)]
}

/// Where a point lands on the source: nearest, or along the normal both ways with method 0.
fn locate(bvh: &MeshBvh, at: DVec3, normal: DVec3, method: i64) -> Option<(Index, DVec3, DVec3)> {
    if method == 0 && normal != DVec3::ZERO {
        let ahead = bvh.raycast(at, normal);
        let behind = bvh.raycast(at, -normal);
        let best = match (ahead, behind) {
            (Some(x), Some(y)) => Some(if x.t <= y.t { x } else { y }),
            (x, y) => x.or(y),
        };
        if let Some(h) = best {
            return Some((h.triangle, h.bary, h.point));
        }
    }
    bvh.closest_point(at).map(|h| (h.triangle, h.bary, h.point))
}

fn transfer(input: &Mesh, source: &Mesh, to_common: DMat4, from_common: DMat4, a: [i64; 4], space: i64, method: i64, flip: bool) -> Mesh {
    let [pos, nml, uvs, col] = a;
    let mut m = input.clone();
    let by_component = space == 4 || space == 5;
    let mut src = source.clone();
    src.compact(None);
    for p in 0..src.point_capacity() {
        src.set_position(p, to_common.transform_point3(src.position(p)));
    }
    let bvh = MeshBvh::new(&src);
    if bvh.is_empty() {
        return m;
    }
    let uv_name = names::uv(0);
    let src_uv: Vec<[f32; 2]> = src.corner_attrs().get::<[f32; 2]>(uv_name.as_str()).to_vec();
    let src_normals = corner_normals(&src, &CornerNormalOptions::default());
    // UV space: points sample the source where the same UV lies, read through its triangles.
    let uv_lookup = |uv: [f32; 2]| -> Option<[(Index, f64); 3]> {
        let c = &bvh.triangles().corners;
        for t in c.chunks(3) {
            let at = |i: usize| glam::DVec2::new(src_uv[t[i] as usize][0] as f64, src_uv[t[i] as usize][1] as f64);
            let (p0, p1, p2) = (at(0), at(1), at(2));
            let q = glam::DVec2::new(uv[0] as f64, uv[1] as f64);
            let den = (p1 - p0).perp_dot(p2 - p0);
            if den.abs() < 1e-14 {
                continue;
            }
            let v = (q - p0).perp_dot(p2 - p0) / den;
            let w = (p1 - p0).perp_dot(q - p0) / den;
            let u = 1.0 - v - w;
            if u >= -1e-6 && v >= -1e-6 && w >= -1e-6 {
                return Some([(t[0], u), (t[1], v), (t[2], w)]);
            }
        }
        None
    };
    let blend = |weights: &[(Index, f64); 3], f: &dyn Fn(Index) -> DVec3| weights.iter().map(|&(c, w)| f(c) * w).sum::<DVec3>();
    let target_uv: Vec<[f32; 2]> = m.corner_attrs().get::<[f32; 2]>(uv_name.as_str()).to_vec();

    // Per corner of the target: the source corners it reads from.
    let cap = m.corner_capacity() as usize;
    let mut samples: Vec<Option<[(Index, f64); 3]>> = vec![None; cap];
    let mut hits: Vec<Option<DVec3>> = vec![None; m.point_capacity() as usize];
    for p in 0..m.point_capacity() {
        if !m.point_valid(p) {
            continue;
        }
        let at = to_common.transform_point3(input.position(p));
        let n = point_normal(input, p);
        if by_component {
            continue;
        }
        if let Some((_, _, q)) = locate(&bvh, at, n, method) {
            hits[p as usize] = Some(q);
        }
    }
    for c in 0..m.corner_capacity() {
        if !m.corner_valid(c) || !m.face_valid(m.corner_face(c)) {
            continue;
        }
        let p = m.corner_point(c);
        samples[c as usize] = if space == 3 {
            target_uv.get(c as usize).and_then(|&uv| uv_lookup(uv))
        } else if by_component {
            None
        } else {
            let at = to_common.transform_point3(input.position(p));
            locate(&bvh, at, point_normal(input, p), method).map(|(t, b, _)| triangle_at(&bvh, t, b))
        };
    }
    if by_component {
        if pos != 0 {
            for p in 0..m.point_capacity().min(src.point_capacity()) {
                if m.point_valid(p) && src.point_valid(p) {
                    m.set_position(p, from_common.transform_point3(src.position(p)));
                }
            }
        }
        if uvs != 0 && !src_uv.is_empty() {
            let n = m.corner_capacity().min(src.corner_capacity()) as usize;
            let layer = m.corner_attrs_mut().get_or_add::<[f32; 2]>(uv_name.as_str());
            layer[..n].copy_from_slice(&src_uv[..n]);
        }
        return m;
    }
    if space == 3 && pos != 0 {
        let mut sum = vec![(DVec3::ZERO, 0.0); m.point_capacity() as usize];
        for c in 0..m.corner_capacity() {
            if let Some(w) = samples[c as usize] {
                let v = blend(&w, &|k| src.position(src.corner_point(k)));
                let e = &mut sum[m.corner_point(c) as usize];
                e.0 += v;
                e.1 += 1.0;
            }
        }
        for p in 0..m.point_capacity() {
            if m.point_valid(p) && sum[p as usize].1 > 0.0 {
                m.set_position(p, from_common.transform_point3(sum[p as usize].0 / sum[p as usize].1));
            }
        }
    } else if pos != 0 {
        for p in 0..m.point_capacity() {
            if let Some(q) = hits[p as usize] {
                m.set_position(p, from_common.transform_point3(q));
            }
        }
    }
    if uvs != 0 && !src_uv.is_empty() {
        let layer = m.corner_attrs_mut().get_or_add::<[f32; 2]>(uv_name.as_str());
        for (c, s) in samples.iter().enumerate() {
            if let Some(w) = s {
                let u: f64 = w.iter().map(|&(k, x)| src_uv[k as usize][0] as f64 * x).sum();
                let v: f64 = w.iter().map(|&(k, x)| src_uv[k as usize][1] as f64 * x).sum();
                layer[c] = [if flip { 1.0 - u } else { u } as f32, v as f32];
            }
        }
    }
    if nml != 0 && !src_normals.is_empty() {
        let layer = m.corner_attrs_mut().get_or_add::<[f32; 3]>(names::NORMAL);
        let turn = from_common;
        for (c, s) in samples.iter().enumerate() {
            if let Some(w) = s {
                let sum = blend(w, &|k| src_normals[k as usize].as_dvec3());
                layer[c] = turn.transform_vector3(sum).normalize_or_zero().as_vec3().to_array();
            }
        }
    }
    if col != 0 {
        for set in colors::sets(&src) {
            let values: Vec<[f32; 4]> = colors::values(&src, &set).to_vec();
            if !colors::has(&m, &set) {
                colors::add_white(&mut m, &set);
            }
            let layer = colors::values_mut(&mut m, &set);
            for (c, s) in samples.iter().enumerate() {
                if let Some(w) = s {
                    let mut out = [0f32; 4];
                    for &(k, x) in w {
                        for i in 0..4 {
                            out[i] += values[k as usize][i] * x as f32;
                        }
                    }
                    layer[c] = out;
                }
            }
        }
    }
    m
}

impl NodeBehavior for TransferNode {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        let Some(input) = d.mesh(crate::history::INPUT_POLYMESH).cloned() else {
            d.set_output(crate::history::OUTPUT, PlugValue::Mesh(None));
            return;
        };
        let Some(source) = d.mesh(SOURCE).cloned() else {
            d.set_output(crate::history::OUTPUT, PlugValue::Mesh(Some(input)));
            return;
        };
        let space = d.int(SPACE);
        let matrix = |attr: usize| match d.value(attr) {
            PlugValue::Matrix(m) => *m,
            _ => DMat4::IDENTITY,
        };
        // World space samples through both objects' matrices; every other space compares them as they stand.
        let (to_common, from_common) = if space == 0 {
            let (sm, tm) = (matrix(SOURCE_MAT), matrix(TARGET_MAT));
            (tm.inverse() * sm, DMat4::IDENTITY)
        } else {
            (DMat4::IDENTITY, DMat4::IDENTITY)
        };
        let out = transfer(&input, &source, to_common, from_common, [d.int(POS), d.int(NML), d.int(UVS), d.int(COL)], space, d.int(METHOD), d.int(FLIP_UV) != 0);
        d.set_output(crate::history::OUTPUT, PlugValue::Mesh(Some(Arc::new(out))));
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn register_transfer_type() {
    let int_attr = |n: &str, short: &str, def: i64| AttrDef {
        name: n.into(),
        short_name: short.into(),
        ty: PlugType::Int,
        default_value: PlugValue::Int(def),
        flags: flags::INPUT | flags::STORABLE | flags::KEYABLE,
        ..Default::default()
    };
    let _ = types::register_type(NodeType {
        name: "transferAttributes".into(),
        base: types::find("polyModifier"),
        doc: "Copies positions, normals, UVs or colours from another mesh.".into(),
        own_attrs: vec![
            mesh_attr("sourcePolymesh", "spm", flags::INPUT | flags::HIDDEN, "The mesh sampled."),
            matrix_attr("sourceMatrix", flags::INPUT | flags::HIDDEN, "Its world matrix."),
            matrix_attr("targetMatrix", flags::INPUT | flags::HIDDEN, "The target's world matrix."),
            int_attr("transferPositions", "pos", 0),
            int_attr("transferNormals", "nml", 0),
            int_attr("transferUVs", "uvs", 0),
            int_attr("transferColors", "col", 0),
            int_attr("sampleSpace", "spa", 0),
            int_attr("searchMethod", "sm", 3),
            int_attr("flipUVs", "fuv", 0),
        ],
        create: Some(Arc::new(|| Box::new(TransferNode))),
        ..Default::default()
    });
}

// ---------------------------------------------------------------------------------------------------------------------
// polyCleanup

struct Cleanup {
    quads: bool,
    nsided: bool,
    concave: bool,
    holed: bool,
    non_planar: bool,
    zero_geometry: Option<f64>,
    zero_edge: Option<f64>,
    non_manifold: bool,
    lamina: bool,
}

fn face_is_concave(m: &Mesh, f: Index) -> bool {
    let pts: Vec<DVec3> = m.face_points(f).iter().map(|&p| m.position(p)).collect();
    if pts.len() < 4 {
        return false;
    }
    let n = m.face_normal(f);
    (0..pts.len()).any(|i| {
        let (a, b, c) = (pts[i], pts[(i + 1) % pts.len()], pts[(i + 2) % pts.len()]);
        (b - a).cross(c - b).dot(n) < -1e-9
    })
}

fn face_is_non_planar(m: &Mesh, f: Index) -> bool {
    let pts: Vec<DVec3> = m.face_points(f).iter().map(|&p| m.position(p)).collect();
    if pts.len() < 4 {
        return false;
    }
    let n = m.face_normal(f);
    let tol = (m.face_area(f).sqrt() * 1e-4).max(1e-9);
    pts.iter().any(|p| (*p - pts[0]).dot(n).abs() > tol)
}

/// Faces with the same points as another: all but the first of each group.
pub(crate) fn lamina_faces(m: &Mesh) -> Vec<Index> {
    let mut seen: std::collections::HashMap<Vec<Index>, Index> = std::collections::HashMap::new();
    let mut out = Vec::new();
    for f in 0..m.face_capacity() {
        if !m.face_valid(f) {
            continue;
        }
        let mut key = m.face_points(f);
        key.sort_unstable();
        if seen.contains_key(&key) {
            out.push(f);
        } else {
            seen.insert(key, f);
        }
    }
    out
}

pub(crate) fn non_manifold_edges(m: &Mesh) -> Vec<Index> {
    (0..m.edge_capacity()).filter(|&e| m.edge_valid(e) && m.edge_face_count(e) > 2).collect()
}

/// Points whose faces don't make one fan: bow ties.
pub(crate) fn non_manifold_points(m: &Mesh) -> Vec<Index> {
    let mut out = Vec::new();
    for p in 0..m.point_capacity() {
        if !m.point_valid(p) {
            continue;
        }
        let faces = m.point_faces(p);
        if faces.len() < 2 {
            continue;
        }
        let mut seen = BTreeSet::from([faces[0]]);
        let mut todo = vec![faces[0]];
        while let Some(f) = todo.pop() {
            for c in m.face_corners(f) {
                let e = m.corner_edge(c);
                if m.edge_point(e, 0) != p && m.edge_point(e, 1) != p {
                    continue;
                }
                m.for_each_edge_corner(e, |r| {
                    let o = m.corner_face(r);
                    if seen.insert(o) {
                        todo.push(o);
                    }
                });
            }
        }
        if seen.len() != faces.len() {
            out.push(p);
        }
    }
    out
}

/// What cleanup finds: (faces, edges, points).
fn find_problems(m: &Mesh, c: &Cleanup) -> (Vec<Index>, Vec<Index>, Vec<Index>) {
    let mut fs = BTreeSet::new();
    let mut es = BTreeSet::new();
    let mut ps = BTreeSet::new();
    for f in 0..m.face_capacity() {
        if !m.face_valid(f) {
            continue;
        }
        let n = m.face_size(f);
        if (c.quads && n == 4) || (c.nsided && n > 4) || (c.concave && face_is_concave(m, f)) || (c.non_planar && face_is_non_planar(m, f)) {
            fs.insert(f);
        }
        if c.holed && m.face_attrs().get::<bool>("hole").get(f as usize).copied().unwrap_or(false) {
            fs.insert(f);
        }
        if c.zero_geometry.is_some_and(|t| m.face_area(f) <= t) {
            fs.insert(f);
        }
    }
    if let Some(t) = c.zero_edge {
        for e in 0..m.edge_capacity() {
            if m.edge_valid(e) && m.edge_length(e) <= t {
                es.insert(e);
            }
        }
    }
    if c.lamina {
        fs.extend(lamina_faces(m));
    }
    if c.non_manifold {
        es.extend(non_manifold_edges(m));
        ps.extend(non_manifold_points(m));
    }
    (fs.into_iter().collect(), es.into_iter().collect(), ps.into_iter().collect())
}

fn fix_problems(m: &mut Mesh, c: &Cleanup) {
    let (fs, es, _) = find_problems(m, c);
    let tri = aztek_geo::triangulate::TriangulateOptions::default();
    if c.zero_edge.is_some() {
        let short: Vec<Index> = es.iter().copied().filter(|&e| m.edge_valid(e) && m.edge_face_count(e) <= 2).collect();
        collapse_edges(m, &short);
    }
    if c.lamina {
        let l = lamina_faces(m);
        delete_faces(m, &l, None);
    }
    if c.non_manifold {
        let bad: Vec<Index> = non_manifold_edges(m);
        for e in bad {
            // The third face and beyond leave the edge.
            let mut around = Vec::new();
            m.for_each_edge_corner(e, |r| around.push(m.corner_face(r)));
            delete_faces(m, &around[2..], None);
        }
        let bow = non_manifold_points(m);
        split_vertices(m, &bow);
    }
    let live: Vec<Index> = fs.into_iter().filter(|&f| m.face_valid(f)).collect();
    if c.zero_geometry.is_some() {
        let tiny: Vec<Index> = live.iter().copied().filter(|&f| m.face_area(f) <= c.zero_geometry.unwrap()).collect();
        delete_faces(m, &tiny, None);
    }
    let rest: Vec<Index> = live.into_iter().filter(|&f| m.face_valid(f)).collect();
    if !rest.is_empty() {
        triangulate_faces(m, &rest, &tri);
    }
}

fn cleanup_cmd(s: &mut Session, a: &Args, c: &Cleanup, select_only: bool, all_meshes: bool, keep_history: bool) -> CmdResult {
    let meshes: Vec<NodeId> = if all_meshes {
        s.graph.nodes().into_iter().filter(|&n| components::is_mesh(&s.graph, n) && !s.graph.node(n).is_some_and(|x| x.bool(DagNode::INTERMEDIATE_OBJECT))).collect()
    } else {
        gather(s, a, K::Face)?.0.into_keys().collect()
    };
    let mut found = Vec::new();
    for n in meshes {
        let g = &mut s.graph;
        let Some(cage) = crate::modifiers::cage(g, n) else { continue };
        let (fs, es, ps) = find_problems(&cage, c);
        found.extend(component_list(g, n, "f", &fs, false));
        found.extend(component_list(g, n, "e", &es, false));
        found.extend(component_list(g, n, "vtx", &ps, false));
        if !select_only && !(fs.is_empty() && es.is_empty() && ps.is_empty()) {
            let base = if keep_history { None } else { Some(crate::mesh_edit::base_plug(g, n)?) };
            let mut m = (*cage).clone();
            fix_problems(&mut m, c);
            m.compact(None);
            let base = match base {
                Some(b) => b,
                None => crate::mesh_edit::base_plug(g, n)?,
            };
            g.set_value(base, PlugValue::Mesh(Some(Arc::new(m))))?;
        }
    }
    if select_only {
        if found.is_empty() {
            components::clear(&mut s.graph);
        } else {
            execute(s, "select", &json!({"args": found.clone()}), CallSource::Script)?;
        }
    }
    Ok(json!(found))
}

// ---------------------------------------------------------------------------------------------------------------------
// Faces from points

fn gen_type(name: &str, doc: &str) -> NodeType {
    NodeType {
        name: name.into(),
        icon: "cube-plus".into(),
        doc: doc.into(),
        own_attrs: vec![
            AttrDef {
                name: "points".into(),
                ty: PlugType::String,
                default_value: PlugValue::String(String::new()),
                flags: flags::INPUT | flags::STORABLE,
                doc: "The face's points, as JSON [[x, y, z], ...].".into(),
                ..Default::default()
            },
            mesh_attr("output", "out", flags::OUTPUT | flags::HIDDEN, "The face."),
        ],
        create: Some(Arc::new(|| Box::new(CreateFaceNode))),
        ..Default::default()
    }
}

struct CreateFaceNode;

impl NodeBehavior for CreateFaceNode {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        let pts: Vec<DVec3> = serde_json::from_str::<Value>(d.string(0)).ok().and_then(|v| v.as_array().map(|l| l.iter().map(crate::mesh_edit::vec).collect())).unwrap_or_default();
        let idx: Vec<Index> = (0..pts.len() as Index).collect();
        let m = Mesh::from_polygons(&pts, &[pts.len() as Index], &idx);
        d.set_output(1, PlugValue::Mesh(Some(Arc::new(m))));
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn points_of(a: &Args, flag: &str) -> Vec<DVec3> {
    fn walk(v: &Value, out: &mut Vec<f64>) {
        match v {
            Value::Array(x) => x.iter().for_each(|y| walk(y, out)),
            y => out.extend(y.as_f64()),
        }
    }
    let mut n = Vec::new();
    if let Some(v) = a.get(flag) {
        walk(v, &mut n);
    }
    n.chunks(3).filter(|c| c.len() == 3).map(|c| DVec3::new(c[0], c[1], c[2])).collect()
}

fn append_op(a: &Args, m: &mut Mesh, _sel: &Ids, _sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    // The face runs along its edges in the order given, then through its new points; winding follows the border.
    let mut order: Vec<Index> = Vec::new();
    let list: Vec<i64> = match a.get("edge") {
        Some(Value::Array(v)) => v.iter().filter_map(Value::as_i64).collect(),
        Some(v) => v.as_i64().into_iter().collect(),
        None => Vec::new(),
    };
    let mut flat = Vec::new();
    if let Some(v) = a.get("append") {
        fn walk(v: &Value, out: &mut Vec<f64>) {
            match v {
                Value::Array(x) => x.iter().for_each(|y| walk(y, out)),
                y => out.extend(y.as_f64()),
            }
        }
        walk(v, &mut flat);
    }
    let mut edges_given = list;
    if flat.len() % 3 != 0 || flat.len() < 3 {
        edges_given.extend(flat.iter().map(|&x| x as i64));
        flat.clear();
    }
    for e in edges_given {
        if e < 0 || !m.edge_valid(e as Index) {
            fail!("polyAppend: no edge {e}");
        }
        let (a0, b0) = (m.edge_point(e as Index, 0), m.edge_point(e as Index, 1));
        match order.last().copied() {
            None => {
                // A border edge's own face runs one way; the new face runs the other.
                let mut forward = false;
                m.for_each_edge_corner(e as Index, |c| forward = forward || m.corner_point(c) == b0);
                if forward {
                    order.extend([a0, b0]);
                } else {
                    order.extend([b0, a0]);
                }
            }
            Some(last) if last == a0 => order.push(b0),
            Some(last) if last == b0 => order.push(a0),
            Some(_) => {
                let mut forward = false;
                m.for_each_edge_corner(e as Index, |c| forward = forward || m.corner_point(c) == b0);
                if forward {
                    order.extend([a0, b0]);
                } else {
                    order.extend([b0, a0]);
                }
            }
        }
    }
    let mut pts = points_of(a, "point");
    for c in flat.chunks(3) {
        pts.push(DVec3::new(c[0], c[1], c[2]));
    }
    for p in pts {
        order.push(m.add_point(p));
    }
    if order.first() == order.last() && order.len() > 1 {
        order.pop();
    }
    order.dedup();
    if order.len() < 3 {
        fail!("polyAppend: a face needs three points");
    }
    let f = m.add_face(&order);
    if f == INVALID {
        fail!("polyAppend: these points don't make a face");
    }
    Ok(faces(vec![f]))
}

pub(super) fn register(c: &mut Commands<Session>) {
    register_types();

    register_op(HistoryOp { node_type: "polyAppend", command: "polyAppend", label: "Append to Polygon", kind: K::Edge, run: append_op, skip_flags: &[], doc: "Adds a face." });

    c.register(with_table(
        Cmd::new(
            "polySeparate",
            "Separate a mesh into one mesh per shell, children of its transform, each fed by a polySeparate node. -ch 0 makes them independent \
             objects and removes the original. Returns the new transforms, then the nodes.",
            |s, a| {
                let ch = a.get("constructionHistory").and_then(Value::as_bool).unwrap_or(true) && crate::history::enabled(&s.graph);
                if !ch {
                    return separate_plain(s, a);
                }
                let shapes = shapes_named(s, a, "polySeparate")?;
                let op = crate::history::op("polySeparate").unwrap();
                let g = &mut s.graph;
                g.open_chunk("Separate");
                let r = (|| -> CmdResult {
                    let (mut xfs, mut nodes, mut made) = (Vec::new(), Vec::new(), Vec::new());
                    for shape in shapes {
                        g.evaluate();
                        let Some(m) = g.node(shape).unwrap().data().mesh(MeshNode::OUT_MESH).cloned() else { continue };
                        let count = separate(&m).len();
                        if count < 2 {
                            fail!("polySeparate: {} has only one shell", components::display_path(g, shape));
                        }
                        let parent = crate::dag::transform_of(g, shape);
                        for i in 0..count {
                            let t = types::find(op.node_type).unwrap();
                            let h = g.create_node(&t, "", NO_NODE, true)?;
                            g.set_value(plug(h, t.attr_index("shell").unwrap()), PlugValue::Int(i as i64))?;
                            g.connect(&plug(shape, MeshNode::OUT_MESH), &plug(h, crate::history::INPUT_POLYMESH), false)?;
                            let (xf, piece) = crate::dag::create_with_shape(g, "mesh", "polySurface1", parent)?;
                            g.connect(&plug(h, crate::history::OUTPUT), &plug(piece, MeshNode::IN_MESH), false)?;
                            xfs.push(json!(g.path(xf)));
                            nodes.push(json!(g.path(h)));
                            made.push(xf);
                        }
                        g.set_value(plug(shape, DagNode::INTERMEDIATE_OBJECT), PlugValue::Bool(true))?;
                    }
                    g.select(&made, SelectMode::Replace);
                    components::clear(g);
                    xfs.extend(nodes);
                    Ok(Value::Array(xfs))
                })();
                g.close_chunk();
                r
            },
        )
        .flag(fl("shell", "", T::Int, json!(0), "Which shell this node keeps.")),
        "polySeparate",
        &[],
    ));

    c.register(ref_cmd(
        "polyUnite",
        "Combine meshes into one, world positions kept, faces in the order named. The originals stay as the hidden shapes feeding it, so moving \
         them moves the result, unless -ch 0. Returns [transform, node].",
        &[("mergeUVSets", json!(1))],
        |s, a| {
            let shapes = shapes_named(s, a, "polyUnite")?;
            if shapes.len() < 2 {
                fail!("polyUnite: give at least two meshes");
            }
            let ch = a.get("constructionHistory").and_then(Value::as_bool).unwrap_or(true) && crate::history::enabled(&s.graph);
            if !ch {
                let names: Vec<String> = shapes.iter().map(|&n| components::display_path(&s.graph, n)).collect();
                let mut call = json!({"args": names});
                if a.has("name") {
                    call["name"] = json!(a.string("name", ""));
                }
                let r = execute(s, "polyCombine", &call, CallSource::Script)?;
                return Ok(json!([r]));
            }
            let r = multi_input(s, a, "polyUnite", &shapes, 0, "polySurface1")?;
            if a.bool("centerPivot") {
                execute(s, "centerPivot", &json!({"args": [r[0].clone()]}), CallSource::Script)?;
            }
            Ok(r)
        },
    ));
    c.register(boolean_cmd("polyCBoolOp", "polyCBoolOp", "A boolean of meshes: -op 1 union, 2 difference, 3 intersection. Returns [transform, node]."));
    c.register(boolean_cmd("polyBoolOp", "polyBoolOp", "A boolean of two meshes: -op 1 union, 2 difference, 3 intersection. Returns [transform, node]."));
    c.register(boolean_cmd("polyBooleanCmd", "polyCBoolOp", "A boolean of meshes: -op 1 union, 2 difference, 3 intersection. Returns [transform, node]."));

    c.register(ref_cmd(
        "polySmooth",
        "Smooth meshes or faces: -mth 0 Catmull-Clark (whole meshes), 1 linear; -dv levels; -kb keep borders sharp, -khe hard edges. Returns the \
         polySmoothFace node.",
        &[
            ("divisions", json!(1)),
            ("method", json!(0)),
            ("keepBorder", json!(true)),
            ("continuity", json!(1.0)),
            ("smoothUVs", json!(true)),
            ("keepTessellation", json!(true)),
            ("subdivisionLevels", json!(1)),
            ("divisionsPerEdge", json!(1)),
            ("pushStrength", json!(0.1)),
            ("roundness", json!(1.0)),
        ],
        |s, a| run_op(s, "polySmoothFace", a),
    ));
    c.register(ref_cmd(
        "polyRemesh",
        "Isotropic remesh of faces or meshes: edges over -mel are split, under -cot percent of it collapsed, -smt relaxes. -urv reads -mel against \
         the average edge length. Returns the node.",
        &[("maxEdgeLength", json!(1.0)), ("collapseThreshold", json!(20.0)), ("smoothStrength", json!(0.0)), ("tessellateBorders", json!(true)), ("useRelativeValues", json!(true))],
        |s, a| run_op(s, "polyRemesh", a),
    ));
    c.register(with_table(
        Cmd::new(
            "polyRetopo",
            "Automatic retopology: rebuilds meshes as even quad-dominant faces, about -targetFaceCount of them. Returns the node.",
            |s, a| run_op(s, "polyRetopo", a),
        )
        .flag(fl("targetFaceCount", "tfc", T::Int, json!(1000), "About how many faces."))
        .flag(fl("targetFaceCountTolerance", "tft", T::Float, json!(10.0), "Accepted."))
        .flag(fl("preserveHardEdges", "phe", T::Bool, json!(false), "Accepted."))
        .flag(fl("symmetry", "sym", T::Bool, json!(false), "Accepted.")),
        "polyRetopo",
        &[],
    ));

    let mirror_run = |s: &mut Session, a: &Args| -> CmdResult {
        run_op_with(s, "polyMirror", a, &mut |g, n, _m, _ids, args| {
            add_world(g, n, args);
            Ok(())
        })
    };
    c.register(with_table(
        Cmd::new(
            "polyMirrorFace",
            "Mirror a mesh in place, welding along the plane: -a axis, -ad direction, -ma 0 bounding box, 1 object, 2 world, -mps plane position, \
             -cutMesh removes what crosses it, -mm 0 leaves borders open, -mt how close welds. Returns the node.",
            mirror_run,
        )
        .flag(world_flag())
        .flag(fl("cutMesh", "cm", T::Bool, Value::Null, "Cut off what lies past the plane first."))
        .flag(fl("flipUVs", "fuv", T::Bool, Value::Null, "Reflect the new faces' U."))
        .flag(fl("smoothingAngle", "sma", T::Float, json!(30.0), "Accepted.")),
        "polyMirrorFace",
        &[("axis", json!(0)), ("axisDirection", json!(1)), ("mergeMode", json!(1)), ("mergeThreshold", json!(0.001)), ("mirrorAxis", json!(2))],
    ));
    // Every form leaves a node in the history; only reflecting in place, which adds no geometry, is left to the plain command.
    extend(c, "polyMirror", "polyMirrorFace", &[], move |s, a| {
        if a.bool("flip") {
            return None;
        }
        let mut a2 = a.clone();
        if a2.get("axis").is_some_and(Value::is_string) {
            let i = a2.string("axis", "x").bytes().next().map_or(0, |b| (b - b'x') as i64);
            a2.flags.insert("axis".into(), json!(i));
        }
        if let Some(direction) = a2.get("direction").and_then(Value::as_str).map(str::to_string) {
            // Positive keeps the + side and rebuilds the other from it; both adds a copy of everything.
            a2.flags.insert("axisDirection".into(), json!(if direction == "negative" { 0 } else { 1 }));
            if !a2.has("cutMesh") {
                a2.flags.insert("cutMesh".into(), json!(direction != "both"));
            }
            a2.flags.remove("direction");
        } else if !a2.has("cutMesh") && !a2.has("axisDirection") {
            a2.flags.insert("cutMesh".into(), json!(true));
        }
        match a2.get("space").and_then(Value::as_str) {
            Some("world") => {
                a2.flags.insert("mirrorAxis".into(), json!(2));
            }
            Some(_) => {
                a2.flags.insert("mirrorAxis".into(), json!(1));
            }
            None => {}
        }
        a2.flags.remove("space");
        Some(mirror_run(s, &a2))
    });
    // The same smooth as polySmooth, which is a node of its own in the history.
    extend(c, "polySubdivide", "polySmooth", &[], |s, a| {
        let mut a2 = a.clone();
        a2.flags.insert("divisions".into(), json!(a.int("levels", 1).clamp(1, 5)));
        a2.flags.insert("method".into(), json!(0));
        Some(run_op(s, "polySmoothFace", &a2))
    });
    if let Some(mut d) = c.find("polyMirror").cloned() {
        if !d.flags.iter().any(|f| f.name == "cutMesh") {
            d = d.flag(fl("cutMesh", "cm", T::Bool, Value::Null, "Cut off what lies past the plane first."));
        }
        if !d.flags.iter().any(|f| f.name == "flipUVs") {
            d = d.flag(fl("flipUVs", "fuv", T::Bool, Value::Null, "Reflect the new faces' U."));
        }
        c.register(d);
    }

    extend(
        c,
        "polyReduce",
        "polyReduce",
        &[
            ("termination", json!(0)),
            ("version", json!(0)),
            ("keepBorder", json!(true)),
            ("keepHardEdge", json!(true)),
            ("keepMapBorder", json!(true)),
            ("keepColorBorder", json!(true)),
            ("keepFaceGroupBorder", json!(true)),
            ("keepCreaseEdge", json!(true)),
            ("triangulate", json!(true)),
            ("preserveTopology", json!(true)),
        ],
        |s, a| {
        // The reference's flags go through the node; the ratio form keeps the modifier and radial handling it always had.
        // Symmetric reduction mirrors a half and has no node form.
        if a.get("symmetry").and_then(Value::as_str).is_some_and(|x| !x.is_empty() && x != "auto") {
            return None;
        }
        let mut a2 = a.clone();
        if a.has("percentage") && a.float("percentage", 0.0) == 0.0 {
            a2.flags.insert("ratio".into(), json!(1.0));
        }
        Some(run_op(s, "polyReduce", &a2))
        },
    );
    extend(c, "polyTriangulate", "polyTriangulate", &[], |s, a| {
        if a.named().is_empty() {
            return None;
        }
        Some(run_op(s, "polyTriangulate", a))
    });
    extend(c, "polySoftEdge", "polySoftEdge", &[], |s, a| {
        if a.named().is_empty() {
            return None;
        }
        Some(run_op(s, "polySoftEdge", a))
    });
    extend(c, "polyCircularize", "polyCircularize", &[("evenlyDistribute", json!(true)), ("smoothingAngle", json!(30.0))], |s, a| {
        let the_reference = !a.named().is_empty() || a.has("radialOffset") || a.has("alignment") || a.has("normalOrientation") || a.has("constructionHistory");
        if !the_reference {
            return None;
        }
        Some(circularize_node(s, a))
    });
    extend(c, "polyEditEdgeFlow", "polyEditEdgeFlow", &[], |s, a| {
        let names = a.named();
        if names.is_empty() {
            return None;
        }
        // The positional form reads like the selection's, but leaves the selection as it was.
        let before = crate::mesh_edit::current_selection(&s.graph);
        let mode = components::mode(&s.graph);
        if let Err(e) = execute(s, "select", &json!({"args": names}), CallSource::Script) {
            return Some(Err(e));
        }
        let mut a2 = a.clone();
        a2.positional.clear();
        let r = edge_flow_inner(s, &a2);
        let [p, e, f] = before;
        components::set_all(&mut s.graph, p, e, f);
        components::set_mode(&mut s.graph, mode);
        Some(r)
    });

    c.register(with_table(
        Cmd::new("polyCreateFacet", "Create one face through -p points as a new mesh. Returns [transform, node].", |s, a| {
            let pts = points_of(a, "point");
            if pts.len() < 3 {
                fail!("polyCreateFacet: a face needs three points");
            }
            let name = a.string("name", "polySurface1");
            let ch = a.get("constructionHistory").and_then(Value::as_bool).unwrap_or(true);
            let g = &mut s.graph;
            g.open_chunk("Create Facet");
            let r = (|| -> CmdResult {
                let (xf, shape) = crate::mesh_nodes::create_mesh_node(g, &name)?;
                let list: Vec<Value> = pts.iter().map(|p| json!([p.x, p.y, p.z])).collect();
                let out = if ch {
                    let t = types::find("polyCreateFace").unwrap();
                    let h = g.create_node(&t, "", NO_NODE, true)?;
                    g.set_value(plug(h, 0), PlugValue::String(Value::Array(list).to_string()))?;
                    g.connect(&plug(h, 1), &plug(shape, MeshNode::IN_MESH), false)?;
                    json!([g.path(xf), g.path(h)])
                } else {
                    let idx: Vec<Index> = (0..pts.len() as Index).collect();
                    let m = Mesh::from_polygons(&pts, &[pts.len() as Index], &idx);
                    g.set_value(plug(shape, MeshNode::IN_MESH), PlugValue::Mesh(Some(Arc::new(m))))?;
                    json!([g.path(xf)])
                };
                g.select(&[xf], SelectMode::Replace);
                Ok(out)
            })();
            g.close_chunk();
            r
        })
        .flag(fl("point", "p", T::List, Value::Null, "A point of the face: x y z. Repeat it.").multi(3)),
        "polyCreateFacet",
        &[],
    ));
    c.register(with_table(
        Cmd::new(
            "polyAppend",
            "Add a face to a mesh along border edges and through new points: -ed edge (repeat; -a takes edges too), -p x y z (repeat). Returns the node.",
            |s, a| {
                let g = &mut s.graph;
                g.evaluate();
                let names = a.named();
                let mesh = match names.first() {
                    Some(n) => crate::mesh_edit::mesh_arg(g, n.split('.').next().unwrap_or(n), "polyAppend")?,
                    None => crate::mesh_edit::mesh_flag(g, &Args::default(), "polyAppend")?,
                };
                let op = crate::history::op("polyAppend").unwrap();
                g.open_chunk("Append");
                let r = apply_targets(g, K::Edge, std::collections::BTreeMap::from([(mesh, Vec::new())]), &mut |m, sel, sym| append_op(a, m, sel, sym), Some((&*op, a)), None);
                g.close_chunk();
                r.map(|made| names_of(g, &made))
            },
        )
        .flag(fl("point", "p", T::List, Value::Null, "A new point: x y z. Repeat it.").multi(3))
        .flag(fl("edge", "ed", T::List, Value::Null, "A border edge. Repeat it.").multi(1))
        .flag(fl("append", "a", T::List, Value::Null, "A border edge, or x y z. Repeat it.").multi(1)),
        "polyAppend",
        &[],
    ));

    c.register(with_table(
        Cmd::new(
            "polyProjectCurve",
            "Project a curve onto a mesh along -d: makes a curve lying on the surface, for polySplit -pc. Takes the curve, then the mesh. Returns the \
             new curve's transform.",
            |s, a| {
                let names = a.named();
                let [curve_name, mesh_name] = names.as_slice() else { fail!("polyProjectCurve: give a curve and a mesh") };
                let g = &mut s.graph;
                let Some(cid) = g.find(curve_name)? else { fail!("polyProjectCurve: no curve {curve_name}") };
                let Some(shape) = crate::curves::as_curve(g, cid) else { fail!("polyProjectCurve: {curve_name} is not a curve") };
                let Some(curve) = crate::curves::world_curve(g, shape) else { fail!("polyProjectCurve: {curve_name} has no curve") };
                let mesh = crate::mesh_edit::mesh_arg(g, mesh_name, "polyProjectCurve")?;
                let Some(cage) = crate::modifiers::cage(g, mesh) else { fail!("polyProjectCurve: no mesh") };
                let world = g.node(mesh).unwrap().matrix(Transform::WORLD_MATRIX);
                let inverse = world.inverse();
                let dir = crate::history::fold_vectors(a).vec3("direction").map_or(DVec3::NEG_Y, DVec3::from_array);
                let dir = if dir == DVec3::ZERO { DVec3::NEG_Y } else { dir.normalize() };
                let local_dir = inverse.transform_vector3(dir).normalize_or_zero();
                let bvh = MeshBvh::new(&cage);
                let reach = cage.bounds().size().length() * 2.0 + 1.0;
                let mut pts: Vec<Value> = Vec::new();
                for p in curve.tessellate(8) {
                    let at = inverse.transform_point3(p);
                    let hit = bvh.raycast_with(at - local_dir * reach, local_dir, reach * 3.0, true);
                    if let Some(h) = hit {
                        let w = world.transform_point3(h.point);
                        pts.push(json!([w.x, w.y, w.z]));
                    }
                }
                if pts.len() < 2 {
                    fail!("polyProjectCurve: {curve_name} misses {mesh_name} along that direction");
                }
                let r = execute(s, "curve", &json!({"degree": 1, "point": pts}), CallSource::Script)?;
                Ok(json!([r]))
            },
        )
        .positional("the curve, then the mesh"),
        "polyProjectCurve",
        &[],
    ));

    c.register(
        Cmd::new(
            "meshReorder",
            "Reorder Vertices: renumbers a mesh's vertices outward from the given ones, so meshes of the same topology come to match when the same \
             three vertices of a face are named on each.",
            |s, a| {
                let members = named_members(s, a)?;
                let Some(first) = members.first() else { fail!("meshReorder: name a mesh's vertices") };
                let mesh = first.node;
                let start: Vec<Index> = a
                    .named()
                    .iter()
                    .filter_map(|n| n.rfind('[').and_then(|o| n[o + 1..n.len() - 1].parse().ok()))
                    .collect();
                let g = &mut s.graph;
                g.open_chunk("Reorder Vertices");
                let r = (|| -> CmdResult {
                    let base = crate::mesh_edit::base_plug(g, mesh)?;
                    let mut m = crate::mesh_edit::base_mesh(g, &base);
                    m.compact(None);
                    let order = reorder_from(&m, &start);
                    m.reorder_points(&order);
                    g.set_value(base, PlugValue::Mesh(Some(Arc::new(m))))?;
                    Ok(json!(g.path(crate::dag::transform_of(g, mesh))))
                })();
                g.close_chunk();
                r
            },
        )
        .positional("three vertices of a face")
        .flag(fl("intermediate", "i", T::Bool, Value::Null, "Accepted.")),
    );

    c.register(with_table(
        Cmd::new(
            "polySymmetrize",
            "Symmetrize meshes across an axis: -ma 0 x, 1 y, 2 z, the -d side rebuilt from the other. Runs symmetrize.",
            |s, a| {
                let axis = ["x", "y", "z"][a.int("axis", 0).clamp(0, 2) as usize];
                let shapes = shapes_named(s, a, "polySymmetrize")?;
                let side = if a.int("axisDirection", 1) == 0 { "negative" } else { "positive" };
                for shape in shapes {
                    let path = components::display_path(&s.graph, shape);
                    let before = s.graph.node(shape).map_or(0, |n| n.int(MeshNode::SYMMETRY));
                    s.graph.set_value(plug(shape, MeshNode::SYMMETRY), PlugValue::Int(1 + axis.as_bytes()[0] as i64 - b'x' as i64))?;
                    execute(s, "symmetrize", &json!({"args": [path], "side": side}), CallSource::Script)?;
                    s.graph.set_value(plug(shape, MeshNode::SYMMETRY), PlugValue::Int(before))?;
                }
                Ok(Value::Null)
            },
        )
        .flag(fl("axis", "a", T::Int, json!(0), "0 x, 1 y, 2 z."))
        .flag(fl("axisDirection", "ad", T::Int, json!(1), "The side kept: 1 positive, 0 negative.")),
        "polySymmetrize",
        &[],
    ));

    c.register(with_table(
        Cmd::new(
            "polyBridgeEdge",
            "Bridge border edges: -dv divisions, -ctp 0 linear, 1 blend, 2 along -inc, -twt twist, -tp taper, -bo offset, -sv1 -sv2 starting points. \
             Returns the node.",
            |s, a| bridge_edge(s, a),
        ),
        "polyBridgeEdge",
        &[],
    ));

    c.register(with_table(
        Cmd::new(
            "transferAttributes",
            "Copy attributes from the first mesh to the others: -pos positions, -nml normals, -uvs UVs, -col colours, 1 or 2 on; -spa 0 world, 1 model, \
             3 UV, 4 component, 5 topology; -sm 0 along the normal, 3 nearest. Live: changing the source changes the target. Returns the nodes.",
            |s, a| {
                let names = a.named();
                let (source, targets): (String, Vec<String>) = if names.len() >= 2 {
                    (names[0].clone(), names[1..].to_vec())
                } else {
                    let sel = aztek_ops::CommandHost::selection_paths(s);
                    if sel.len() < 2 {
                        fail!("transferAttributes: give a source and a target");
                    }
                    (sel[0].clone(), sel[1..].to_vec())
                };
                let g = &mut s.graph;
                let src = crate::mesh_edit::mesh_arg(g, &source, "transferAttributes")?;
                let t = types::find("transferAttributes").unwrap();
                g.open_chunk("Transfer Attributes");
                let r = (|| -> CmdResult {
                    let mut made = Vec::new();
                    for name in &targets {
                        let dst = crate::mesh_edit::mesh_arg(g, name, "transferAttributes")?;
                        let (base, up) = crate::history::upstream(g, dst)?;
                        let h = g.create_node(&t, "", NO_NODE, true)?;
                        for (flag, attr) in [
                            ("transferPositions", POS),
                            ("transferNormals", NML),
                            ("transferUVs", UVS),
                            ("transferColors", COL),
                            ("sampleSpace", SPACE),
                            ("searchMethod", METHOD),
                            ("flipUVs", FLIP_UV),
                        ] {
                            if let Some(v) = a.get(flag).and_then(Value::as_i64) {
                                g.set_value(plug(h, attr), PlugValue::Int(v))?;
                            }
                        }
                        g.disconnect(&base);
                        g.connect(&up, &plug(h, crate::history::INPUT_POLYMESH), false)?;
                        g.connect(&plug(src, MeshNode::OUT_MESH), &plug(h, SOURCE), false)?;
                        g.connect(&plug(crate::dag::transform_of(g, src), Transform::WORLD_MATRIX), &plug(h, SOURCE_MAT), false)?;
                        g.connect(&plug(crate::dag::transform_of(g, dst), Transform::WORLD_MATRIX), &plug(h, TARGET_MAT), false)?;
                        g.connect(&plug(h, crate::history::OUTPUT), &base, false)?;
                        made.push(json!(g.path(h)));
                    }
                    Ok(Value::Array(made))
                })();
                g.close_chunk();
                r
            },
        ),
        "transferAttributes",
        &[],
    ));

    let cleanup_flags = |cmd: Cmd| -> Cmd {
        cmd.positional("meshes (the selection when none)")
            .flag(fl("allMeshes", "am", T::Bool, json!(false), "Every mesh in the scene."))
            .flag(fl("selectOnly", "so", T::Bool, json!(false), "Select what it finds rather than fixing it."))
            .flag(fl("keepHistory", "kh", T::Bool, json!(true), "Accepted."))
            .flag(fl("quads", "q4", T::Bool, json!(false), "Four-sided faces."))
            .flag(fl("nsided", "ns", T::Bool, json!(false), "Faces of more than four sides."))
            .flag(fl("concave", "cv", T::Bool, json!(false), "Concave faces."))
            .flag(fl("holed", "ho", T::Bool, json!(false), "Faces with holes."))
            .flag(fl("nonPlanar", "np", T::Bool, json!(false), "Non-planar faces."))
            .flag(fl("zeroGeometry", "zg", T::Bool, json!(false), "Faces of no area."))
            .flag(fl("zeroGeometryTolerance", "zgt", T::Float, json!(1e-5), "How small."))
            .flag(fl("zeroEdge", "ze", T::Bool, json!(false), "Edges of no length."))
            .flag(fl("zeroEdgeTolerance", "zet", T::Float, json!(1e-5), "How short."))
            .flag(fl("nonManifold", "nm", T::Bool, json!(false), "Non-manifold edges and vertices."))
            .flag(fl("lamina", "lm", T::Bool, json!(false), "Faces sharing all their edges."))
    };
    c.register(cleanup_flags(Cmd::new(
        "polyCleanup",
        "Find or fix problem geometry: -selectOnly selects it, otherwise it is fixed. Returns the components found.",
        |s, a| {
            let c = Cleanup {
                quads: a.bool("quads"),
                nsided: a.bool("nsided"),
                concave: a.bool("concave"),
                holed: a.bool("holed"),
                non_planar: a.bool("nonPlanar"),
                zero_geometry: a.bool("zeroGeometry").then(|| a.float("zeroGeometryTolerance", 1e-5)),
                zero_edge: a.bool("zeroEdge").then(|| a.float("zeroEdgeTolerance", 1e-5)),
                non_manifold: a.bool("nonManifold"),
                lamina: a.bool("lamina"),
            };
            cleanup_cmd(s, a, &c, a.bool("selectOnly"), a.bool("allMeshes"), true)
        },
    )));
    c.register(
        Cmd::new(
            "polyCleanupArgList",
            "polyCleanup's positional form: version, then the list: all meshes, select only (1) or fix (0), history, quads, n-sided, concave, holed, \
             non-planar, zero geometry and its tolerance, zero edge and its tolerance, zero map and its tolerance, shared UVs, non-manifold, lamina, \
             invalid components.",
            |s, a| {
                let list: Vec<String> = a
                    .positional
                    .iter()
                    .filter_map(Value::as_array)
                    .flatten()
                    .map(|v| v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string()))
                    .collect();
                let at = |i: usize| list.get(i).map_or(0.0, |v| v.parse::<f64>().unwrap_or(0.0));
                let c = Cleanup {
                    quads: at(3) != 0.0,
                    nsided: at(4) != 0.0,
                    concave: at(5) != 0.0,
                    holed: at(6) != 0.0,
                    non_planar: at(7) != 0.0,
                    zero_geometry: (at(8) != 0.0).then(|| at(9)),
                    zero_edge: (at(10) != 0.0).then(|| at(11)),
                    non_manifold: at(15) > 0.0,
                    lamina: at(16) > 0.0,
                };
                let a2 = Args::default();
                cleanup_cmd(s, &a2, &c, at(1) == 2.0 || at(1) == 1.0, at(0) != 0.0, at(2) != 0.0)
            },
        )
        .positional("version, then the argument list"),
    );
}

/// polySeparate without history: one independent mesh per shell, the original gone.
fn separate_plain(s: &mut Session, a: &Args) -> CmdResult {
    let names = crate::mesh_nodes::mesh_objects(s, a, "polySeparate")?;
    let g = &mut s.graph;
    let mut out = Vec::new();
    let mut made = Vec::new();
    for name in names {
        let n = crate::mesh_edit::mesh_arg(g, &name, "polySeparate")?;
        g.evaluate();
        let Some(m) = g.node(n).unwrap().data().mesh(MeshNode::OUT_MESH).cloned() else { continue };
        let parts = separate(&m);
        if parts.len() < 2 {
            continue;
        }
        let first = made.len();
        for part in parts {
            let piece = crate::mesh_edit::component_ops::mesh_beside(g, n, part)?;
            out.push(json!(components::display_path(g, piece)));
            made.push(piece);
        }
        crate::isolation::inherit(g, &made[first..].to_vec(), &[n]);
        let gone = crate::dag::transform_of(g, n);
        g.delete_node(gone)?;
    }
    g.select(&made, SelectMode::Replace);
    Ok(Value::Array(out))
}

/// polyCircularize's node: the edges of each named or selected loop, the border of each face region, the edges between vertices.
fn circularize_node(s: &mut Session, a: &Args) -> CmdResult {
    let members = named_members(s, a)?;
    let picked: Vec<(NodeId, K, Ids)> = if !members.is_empty() {
        members.into_iter().filter(|m| m.kind != K::Object).map(|m| (m.node, m.kind, m.ids)).collect()
    } else {
        let mode = components::mode(&s.graph);
        if mode == K::Object {
            Vec::new()
        } else {
            components::selected_all(&s.graph, mode).iter().map(|(&n, ids)| (n, mode, ids.clone())).collect()
        }
    };
    if picked.is_empty() {
        fail!("polyCircularize: select edge loops, faces or vertices");
    }
    let op = crate::history::op("polyCircularize").unwrap();
    let g = &mut s.graph;
    let mut targets: std::collections::BTreeMap<NodeId, Ids> = Default::default();
    for (n, kind, ids) in picked {
        let Some(cage) = crate::modifiers::cage(g, n) else { continue };
        let list = match kind {
            K::Face => aztek_geo::topology::region_boundary_edges(&cage, &ids),
            k => components::convert(&cage, k, &ids, K::Edge, false),
        };
        targets.entry(n).or_default().extend(list);
    }
    g.open_chunk("Circularize");
    let run = op.run;
    let r = apply_targets(g, K::Edge, targets, &mut |m, sel, sym| run(a, m, sel, sym), Some((&*op, a)), None);
    g.close_chunk();
    r.map(|made| names_of(g, &made))
}

/// polyEditEdgeFlow once its components are selected.
fn edge_flow_inner(s: &mut Session, a: &Args) -> CmdResult {
    let mut call = serde_json::Map::new();
    for key in ["adjustEdgeFlow", "edgeFlow", "tension", "loopSmooth", "iterations"] {
        if let Some(v) = a.get(key) {
            call.insert(key.to_string(), v.clone());
        }
    }
    execute(s, "polyEditEdgeFlow", &Value::Object(call), CallSource::Script)
}

/// polyBridgeEdge's flags as polyBridge's.
fn bridge_edge(s: &mut Session, a: &Args) -> CmdResult {
    let mut call = serde_json::Map::new();
    call.insert("divisions".into(), json!(a.int("divisions", 1)));
    let curve = match a.int("curveType", 0) {
        1 => "blend",
        2 => "path",
        _ => "linear",
    };
    call.insert("curveType".into(), json!(curve));
    let inc = a.string("inputCurve", "");
    if !inc.is_empty() {
        call.insert("curve".into(), json!(inc));
        call.insert("curveType".into(), json!("path"));
    }
    if let Some(v) = a.get("twist") {
        call.insert("twist".into(), v.clone());
    }
    if let Some(v) = a.get("taper") {
        call.insert("taper".into(), v.clone());
    }
    if let Some(v) = a.get("bridgeOffset") {
        call.insert("bridgeOffset".into(), v.clone());
    }
    if let Some(v) = a.get("smoothingAngle") {
        call.insert("smoothingAngle".into(), v.clone());
    }
    if let (Some(x), Some(y)) = (a.get("startVert1"), a.get("startVert2")) {
        call.insert("startSource".into(), x.clone());
        call.insert("startTarget".into(), y.clone());
    }
    let name = a.string("name", "");
    let names = a.named();
    let before = crate::mesh_edit::current_selection(&s.graph);
    let mode = components::mode(&s.graph);
    if !names.is_empty() {
        execute(s, "select", &json!({"args": names}), CallSource::Script)?;
    }
    let r = execute(s, "polyBridge", &Value::Object(call), CallSource::Script)?;
    if !names.is_empty() {
        let [p, e, f] = before;
        components::set_all(&mut s.graph, p, e, f);
        components::set_mode(&mut s.graph, mode);
    }
    if !name.is_empty() {
        if let Some(first) = r.as_array().and_then(|l| l.first()).and_then(Value::as_str) {
            if let Ok(Some(id)) = s.graph.find(first) {
                s.graph.rename(id, &name)?;
                return Ok(json!([s.graph.path(id)]));
            }
        }
    }
    Ok(r)
}
