//! Edit Mesh: extrude, bevel, merge, move, split, chip off and the rest, each a history node whose attributes are its
//! command's flags.

use std::cell::Cell;
use std::collections::BTreeSet;

use aztek_geo::loop_tools::{edit_edge_flow, offset_edge_loops, EdgeFlowOptions, OffsetEnds, OffsetLoopOptions, OffsetSide};
use aztek_geo::ops::*;
use aztek_geo::symmetry::SymmetryMap;
use aztek_geo::topology::{chain_edges, region_boundary_edges};
use aztek_geo::{Index, Mesh};
use aztek_ops::{fail, ArgType as T, Args, CmdResult, Commands, FlagDef};
use glam::{DQuat, DVec3};
use serde_json::{json, Value};

use super::*;
use crate::components::{self, ComponentMode as K, Ids};
use crate::history::{register_op, HistoryOp};
use crate::mesh_edit::{edges, faces, points, with_mirror, EditResult};

/// The settings every polyMove and extrude node shares, as flags.
pub(crate) fn move_spec(a: &Args) -> MoveSpec {
    MoveSpec {
        translate: vec_flag(a, "translate", DVec3::ZERO),
        rotate: vec_flag(a, "rotate", DVec3::ZERO),
        scale: vec_flag(a, "scale", DVec3::ONE),
        pivot: vec_flag(a, "pivot", DVec3::ZERO),
        local_translate: vec_flag(a, "localTranslate", DVec3::ZERO),
        local_rotate: vec_flag(a, "localRotate", DVec3::ZERO),
        local_scale: vec_flag(a, "localScale", DVec3::ONE),
        local_direction: vec_flag(a, "localDirection", DVec3::ZERO),
        gain: a.float("gain", 1.0),
        random: a.float("random", 0.0),
        gravity: vec_flag(a, "gravity", DVec3::ZERO),
        weight: a.float("weight", 0.0),
        magnet: vec_flag(a, "magnet", DVec3::ZERO),
        attraction: a.float("attraction", 0.0),
        to_world: object_to_world(a),
    }
}

fn moves_nothing(s: &MoveSpec) -> bool {
    s.translate == DVec3::ZERO
        && s.rotate == DVec3::ZERO
        && s.scale == DVec3::ONE
        && s.local_translate == DVec3::ZERO
        && s.local_rotate == DVec3::ZERO
        && s.local_scale == DVec3::ONE
        && (s.gravity == DVec3::ZERO || s.weight == 0.0)
        && (s.attraction == 0.0 || s.weight == 0.0)
}

/// A share of a move: one of steps equal steps of it.
fn step_of(s: &MoveSpec, steps: f64) -> MoveSpec {
    let root = |v: DVec3| DVec3::new(v.x.max(1e-9).powf(1.0 / steps), v.y.max(1e-9).powf(1.0 / steps), v.z.max(1e-9).powf(1.0 / steps));
    MoveSpec {
        translate: s.translate / steps,
        rotate: s.rotate / steps,
        scale: if s.scale == DVec3::ONE { DVec3::ONE } else { root(s.scale) },
        local_translate: s.local_translate / steps,
        local_rotate: s.local_rotate / steps,
        local_scale: if s.local_scale == DVec3::ONE { DVec3::ONE } else { root(s.local_scale) },
        ..s.clone()
    }
}

/// The centre of the points of some components: the pivot a move turns and scales about when it is given none.
fn set_pivot(g: &crate::graph::Graph, node: NodeId, m: &Mesh, ids: &Ids, kind: K, a: &mut Args) {
    add_world(g, node, a);
    if a.has("pivot") {
        return;
    }
    let pts = components::convert(m, kind, ids, K::Vertex, false);
    let world = object_to_world(a);
    let c = pts.iter().filter(|&&p| m.point_valid(p)).map(|&p| world.transform_point3(m.position(p))).sum::<DVec3>() / pts.len().max(1) as f64;
    a.flags.insert("pivot".into(), json!([c.x, c.y, c.z]));
}

fn curve_samples(a: &Args) -> Vec<DVec3> {
    a.get("curvePoints").and_then(Value::as_array).map(|v| v.iter().map(crate::mesh_edit::vec).collect()).unwrap_or_default()
}

/// The samples of a curve along inputCurve, in the mesh's space, stored for the node: a curve can't be read from the
/// node's evaluation.
fn add_curve(g: &mut crate::graph::Graph, node: NodeId, a: &mut Args) -> CmdResult<()> {
    let name = a.string("inputCurve", "");
    if name.is_empty() {
        return Ok(());
    }
    let Some(c) = g.find(&name)? else { fail!("no curve named {name}") };
    let Some(shape) = crate::curves::as_curve(g, c) else { fail!("{name} is not a curve") };
    let Some(curve) = crate::curves::world_curve(g, shape) else { fail!("{name} has no curve") };
    let to_local = g.node(node).map_or(glam::DMat4::IDENTITY, |n| n.matrix(Transform::WORLD_MATRIX)).inverse();
    let steps = (a.int("divisions", 1).max(1) as usize * 8).max(32);
    let mut pts = Vec::new();
    let (lo, hi) = curve.domain();
    for i in 0..=steps {
        let p = to_local.transform_point3(curve.eval(lo + (hi - lo) * i as f64 / steps as f64));
        pts.push(json!([p.x, p.y, p.z]));
    }
    a.flags.insert("curvePoints".into(), Value::Array(pts));
    Ok(())
}

fn sample(path: &[DVec3], t: f64) -> (DVec3, DVec3) {
    let x = t.clamp(0.0, 1.0) * (path.len() - 1) as f64;
    let i = (x.floor() as usize).min(path.len() - 2);
    let d = path[i + 1] - path[i];
    (path[i] + d * (x - i as f64), d.normalize_or_zero())
}

fn extrude_face_op(a: &Args, m: &mut Mesh, sel: &Ids, sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    let spec = move_spec(a);
    let together = on(a, "keepFacesTogether", true) && on(a, "keepFacetTogether", true) && !a.bool("individual");
    let steps = a.int("divisions", 1).max(1);
    let n = steps as f64;
    let distance = a.float("thickness", 0.0) + spec.local_translate.z * spec.gain;
    let inset = a.float("offset", 0.0);
    let (taper, twist) = (a.float("taper", 1.0), a.float("twist", 0.0));
    let mut step = step_of(&MoveSpec { local_translate: DVec3::new(spec.local_translate.x, spec.local_translate.y, 0.0), ..spec.clone() }, n);
    step.local_rotate.z += twist / n;
    if taper != 1.0 {
        step.local_scale *= taper.max(1e-6).powf(1.0 / n);
    }
    let path = curve_samples(a);
    let mut cur = sel.clone();
    for i in 0..steps {
        let o = ExtrudeOptions {
            offset: if path.len() >= 2 { 0.0 } else { distance / n },
            individual: !together,
            symmetry: sym,
            inset: inset / n,
            keep_back: on(a, "keepBack", true),
        };
        cur = extrude_faces(m, &cur, &o).faces;
        let groups = face_groups(m, &cur, together);
        if path.len() >= 2 {
            let (from, along0) = sample(&path, i as f64 / n);
            let (to, along1) = sample(&path, (i + 1) as f64 / n);
            let turn = if along0 != DVec3::ZERO && along1 != DVec3::ZERO { DQuat::from_rotation_arc(along0, along1) } else { DQuat::IDENTITY };
            for g in &groups {
                for &p in &g.points {
                    m.set_position(p, g.center + (to - from) + turn * (m.position(p) - g.center));
                }
            }
        }
        if !moves_nothing(&step) {
            let groups = face_groups(m, &cur, together);
            move_groups(m, &groups, &step);
        }
    }
    Ok(faces(cur))
}

fn extrude_edge_op(a: &Args, m: &mut Mesh, sel: &Ids, sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    let spec = move_spec(a);
    let steps = a.int("divisions", 1).max(1);
    let n = steps as f64;
    let thickness = a.float("thickness", 0.0) + spec.local_translate.z * spec.gain;
    let step = step_of(&MoveSpec { local_translate: DVec3::new(spec.local_translate.x, spec.local_translate.y, 0.0), ..spec.clone() }, n);
    let mut cur = sel.clone();
    for _ in 0..steps {
        let o = EdgeExtrudeOptions { thickness: thickness / n, offset: a.float("offset", 0.0) / n, symmetry: sym };
        cur = extrude_edges(m, &cur, &o).edges;
        if !moves_nothing(&step) {
            let groups = edge_groups(m, &cur);
            move_groups(m, &groups, &step);
        }
    }
    Ok(edges(cur))
}

fn extrude_vertex_op(a: &Args, m: &mut Mesh, sel: &Ids, _sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    Ok(faces(extrude_vertices(m, sel, a.float("width", 0.0), a.float("length", 0.0), a.int("divisions", 1) as i32)))
}

fn chamfer_op(a: &Args, m: &mut Mesh, sel: &Ids, _sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    Ok(faces(chamfer_vertices(m, sel, a.float("width", 0.25))))
}

/// The shortest edge round the selected edges' faces: what a fraction offset is a fraction of.
fn shortest_near(m: &Mesh, sel: &Ids) -> f64 {
    let mut short = f64::MAX;
    for &e in sel {
        if !m.edge_valid(e) {
            continue;
        }
        short = short.min(m.edge_length(e));
        let mut near = Vec::new();
        m.for_each_edge_corner(e, |c| near.push(m.corner_face(c)));
        for f in near {
            for c in m.face_corners(f) {
                short = short.min(m.edge_length(m.corner_edge(c)));
            }
        }
    }
    if short == f64::MAX {
        1.0
    } else {
        short
    }
}

fn bevel_op(a: &Args, m: &mut Mesh, sel: &Ids, sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    let mut offset = a.float("offset", 0.5);
    // A fraction reaches the middle of the shortest edge at 1.
    if a.bool("offsetAsFraction") {
        offset = a.float("fraction", 0.5) * shortest_near(m, sel) * 0.5;
    }
    let applied = Cell::new(offset);
    let o = BevelOptions {
        offset,
        segments: a.int("segments", 1).max(1) as i32,
        profile: if on(a, "chamfer", false) { 0.25 } else { a.float("profile", 0.5) },
        clamp: on(a, "clamp", true) && on(a, "autoFit", true),
        smoothing_angle: a.float("smoothingAngle", 30.0),
        symmetry: sym,
        applied_offset: Some(&applied),
    };
    let mut r = faces(bevel_edges(m, sel, &o));
    if a.bool("mergeVertices") {
        let all = components::all_ids(m, K::Vertex);
        merge_by_distance(m, &all, &MergeOptions { distance: a.float("mergeVertexTolerance", 0.0001), average: false, symmetry: sym });
    }
    r.info = json!(applied.get());
    Ok(r)
}

fn merge_vert_op(a: &Args, m: &mut Mesh, sel: &Ids, sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    let mut o = MergeOptions { distance: a.float("distance", 0.0), average: a.bool("average") || sym.is_some(), symmetry: sym };
    if a.bool("alwaysMergeTwoVertices") && sel.len() == 2 && m.point_valid(sel[0]) && m.point_valid(sel[1]) {
        o.distance = o.distance.max(m.position(sel[0]).distance(m.position(sel[1])) * 1.001 + 1e-6);
    }
    // Exactly coincident points weld at distance 0.
    o.distance = o.distance.max(1e-9);
    Ok(points(merge_by_distance(m, sel, &o)))
}

fn merge_edge_op(a: &Args, m: &mut Mesh, sel: &Ids, _sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    let (mut first, mut second) = (a.int("firstEdge", -1), a.int("secondEdge", -1));
    if first < 0 || second < 0 {
        if sel.len() != 2 {
            fail!("polyMergeEdge: give two edges");
        }
        (first, second) = (sel[0] as i64, sel[1] as i64);
    }
    let (first, second) = (first as Index, second as Index);
    if !m.edge_valid(first) || !m.edge_valid(second) || first == second {
        fail!("polyMergeEdge: no edges {first} and {second}");
    }
    let mode = a.int("mergeMode", 1);
    // Pairs move source onto target: mode 0 goes to the first edge, 2 to the second, 1 halfway.
    let (source, target) = if mode == 2 { (first, second) } else { (second, first) };
    let pairs = edge_weld_pairs(m, source, target);
    let welded = target_weld(m, &pairs, mode == 1);
    Ok(points(welded))
}

fn merge_face_op(a: &Args, m: &mut Mesh, sel: &Ids, _sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    let (mut first, mut second) = (a.int("firstFacet", -1), a.int("secondFacet", -1));
    if first < 0 || second < 0 {
        if sel.len() != 2 {
            fail!("polyMergeFacet: give two faces");
        }
        (first, second) = (sel[0] as i64, sel[1] as i64);
    }
    // merge_facets keeps the first's points at mode 0, which here moves the second to the first.
    let done = merge_facets(m, first as Index, second as Index, a.int("mergeMode", 6));
    if done.is_empty() {
        fail!("polyMergeFacet: faces {first} and {second} can't merge");
    }
    Ok(points(done))
}

fn move_vertex_op(a: &Args, m: &mut Mesh, sel: &Ids, _sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    let groups = vertex_groups(m, sel);
    move_groups(m, &groups, &move_spec(a));
    Ok(points(sel.clone()))
}

fn move_edge_op(a: &Args, m: &mut Mesh, sel: &Ids, _sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    let groups = edge_groups(m, sel);
    move_groups(m, &groups, &move_spec(a));
    Ok(edges(sel.clone()))
}

fn move_face_op(a: &Args, m: &mut Mesh, sel: &Ids, _sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    let groups = face_groups(m, sel, false);
    move_groups(m, &groups, &move_spec(a));
    Ok(faces(sel.clone()))
}

fn chip_off_op(a: &Args, m: &mut Mesh, sel: &Ids, sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    let all = with_mirror(m, sym, sel, K::Face);
    let mut part = extract_faces(m, &all);
    part.compact(None);
    let first = m.face_capacity();
    m.append(&part);
    let new: Ids = (first..m.face_capacity()).filter(|&f| m.face_valid(f)).collect();
    if !on(a, "duplicate", true) {
        delete_faces(m, &all, None);
    }
    let spec = move_spec(a);
    let together = on(a, "keepFacesTogether", true) && on(a, "keepFacetTogether", true);
    if !moves_nothing(&spec) {
        let groups = face_groups(m, &new, together);
        move_groups(m, &groups, &spec);
    }
    Ok(faces(new))
}

fn del_vertex_op(a: &Args, m: &mut Mesh, sel: &Ids, sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    if a.bool("withFaces") {
        delete_points(m, sel, sym);
    } else {
        dissolve_points(m, &with_mirror(m, sym, sel, K::Vertex));
    }
    Ok(EditResult::default())
}

fn del_edge_op(a: &Args, m: &mut Mesh, sel: &Ids, sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    if a.bool("withFaces") {
        delete_edges(m, sel, sym);
    } else {
        dissolve_edges(m, &with_mirror(m, sym, sel, K::Edge), a.bool("cleanVertices"));
    }
    Ok(EditResult::default())
}

fn del_face_op(_a: &Args, m: &mut Mesh, sel: &Ids, sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    delete_faces(m, sel, sym);
    Ok(EditResult::default())
}

fn split_ring_op(a: &Args, m: &mut Mesh, sel: &Ids, sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    let root = a.int("rootEdge", -1);
    let Some(&first) = (if root >= 0 && m.edge_valid(root as Index) { Some(&(root as Index)) } else { sel.first() }) else {
        return Ok(edges(Vec::new()));
    };
    let first: Index = first;
    let weight = a.float("weight", 0.5);
    let fraction = if (weight - 0.5).abs() > 1e-12 { weight } else { a.float("fraction", 0.5) };
    let count = if a.int("splitType", 0) == 2 { a.int("divisions", 2).max(1) } else { a.int("count", 1).max(1) };
    let spread = if a.int("splitType", 0) == 2 { 1.0 } else { a.float("spacing", 1.0) };
    let o = EdgeLoopOptions { fraction, count: count as i32, spacing: spread };
    let made = match sym {
        None => insert_edge_loop(m, first, &o),
        Some(sym) => {
            let mut plan = KnifePlan::default();
            for t in edge_loop_fractions(&o) {
                plan.segments.extend(knife_loop_plan(m, first, t, Some(sym)).segments);
            }
            knife_apply(m, &plan)
        }
    };
    if a.bool("insertWithEdgeFlow") && !made.is_empty() {
        let f = EdgeFlowOptions { amount: a.float("adjustEdgeFlow", 1.0), ..Default::default() };
        for mv in edit_edge_flow(m, &made, &[], &f) {
            m.set_position(mv.point, mv.position);
        }
    }
    Ok(edges(made))
}

fn normal_op(a: &Args, m: &mut Mesh, sel: &Ids, sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    let all = if sel.is_empty() { components::all_ids(m, K::Face) } else { sel.clone() };
    match a.int("normalMode", 0) {
        1 => conform_normals(m, &all),
        2 => conform_normals(m, &all),
        3 | 4 => {
            // Reverse and cut: the faces come away from the rest along their border, then turn over.
            let border = region_boundary_edges(m, &all);
            detach_edges(m, &border);
            flip_normals(m, &all);
            if a.int("normalMode", 0) == 4 {
                conform_normals(m, &components::all_ids(m, K::Face));
            }
        }
        _ => flip_normals(m, &with_mirror(m, sym, &all, K::Face)),
    }
    Ok(if sel.is_empty() { EditResult::default() } else { faces(sel.clone()) })
}

fn flip_edge_op(_a: &Args, m: &mut Mesh, sel: &Ids, sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    Ok(edges(crate::mesh_edit::component_ops::spin_mirrored(m, sel, sym, true)))
}

fn quad_op(a: &Args, m: &mut Mesh, sel: &Ids, _sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    let o = QuadrangulateOptions {
        max_face_angle: a.float("angle", 30.0),
        keep_hard_edges: on(a, "keepHardEdges", true),
        keep_seams: on(a, "keepTextureBorders", true),
        keep_materials: on(a, "keepGroupBorder", true),
    };
    Ok(faces(quadrangulate_faces(m, sel, &o)))
}

fn poke_op(a: &Args, m: &mut Mesh, sel: &Ids, sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    let first_new = m.point_capacity();
    let all = with_mirror(m, sym, sel, K::Face);
    let normals: Vec<DVec3> = all.iter().filter(|&&f| m.face_valid(f)).map(|&f| m.face_normal(f)).collect();
    let made = poke_faces(m, &all);
    let spec = move_spec(a);
    if !moves_nothing(&spec) {
        let groups: Vec<MoveGroup> = (first_new..m.point_capacity())
            .enumerate()
            .filter(|(_, p)| m.point_valid(*p))
            .map(|(i, p)| MoveGroup {
                points: vec![p],
                center: m.position(p),
                normal: normals.get(i).copied().unwrap_or(DVec3::Y),
                tangent: normals.get(i).copied().unwrap_or(DVec3::Y).any_orthonormal_vector(),
            })
            .collect();
        move_groups(m, &groups, &spec);
    }
    Ok(faces(made))
}

fn wedge_op(a: &Args, m: &mut Mesh, sel: &Ids, _sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    let steps = a.int("divisions", 1).max(1);
    let angle = a.float("wedgeAngle", 0.0);
    // The hinge: a given edge, else the axis and centre flags.
    let hinge_edge = a.get("edge").and_then(|v| v.as_i64().or_else(|| v.as_array().and_then(|l| l.first()).and_then(Value::as_i64)));
    let (center, axis) = match hinge_edge {
        Some(e) if m.edge_valid(e as Index) => {
            let (p, q) = (m.position(m.edge_point(e as Index, 0)), m.position(m.edge_point(e as Index, 1)));
            (p, (q - p).normalize_or_zero())
        }
        _ => (vec_flag(a, "center", DVec3::ZERO), vec_flag(a, "axis", DVec3::Y).normalize_or_zero()),
    };
    if axis == DVec3::ZERO {
        fail!("polyWedgeFace: give a hinge with -ed, or -ax and -cen");
    }
    let turn = DQuat::from_axis_angle(axis, (angle / steps as f64).to_radians());
    let mut cur = sel.clone();
    for _ in 0..steps {
        cur = extrude_faces(m, &cur, &ExtrudeOptions { offset: 0.0, individual: false, symmetry: None, inset: 0.0, keep_back: false }).faces;
        let pts: BTreeSet<Index> = cur.iter().filter(|&&f| m.face_valid(f)).flat_map(|&f| m.face_points(f)).collect();
        for p in pts {
            m.set_position(p, center + turn * (m.position(p) - center));
        }
    }
    // Points on the hinge stayed where they were: the walls end there.
    let all = components::all_ids(m, K::Vertex);
    merge_by_distance(m, &all, &MergeOptions { distance: 1e-7, average: false, symmetry: None });
    Ok(faces(cur.into_iter().filter(|&f| m.face_valid(f)).collect()))
}

fn duplicate_edge_op(a: &Args, m: &mut Mesh, sel: &Ids, sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    let o = OffsetLoopOptions {
        distance: a.float("offset", 0.5).abs(),
        absolute: a.int("splitType", 0) == 1,
        side: OffsetSide::Both,
        ends: OffsetEnds::Converge,
        symmetry: sym,
    };
    Ok(edges(offset_edge_loops(m, sel, &o, a.bool("deleteEdge"))))
}

fn connect_op(a: &Args, m: &mut Mesh, sel: &Ids, sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    let mut pts: Ids = a.get("vertices").and_then(Value::as_array).map(|v| v.iter().filter_map(Value::as_u64).map(|p| p as Index).collect()).unwrap_or_default();
    for e in with_mirror(m, sym, sel, K::Edge) {
        if m.edge_valid(e) {
            pts.push(m.split_edge(e, 0.5));
        }
    }
    let made = connect_points(m, &pts);
    if a.bool("insertWithEdgeFlow") && !made.is_empty() {
        let f = EdgeFlowOptions { amount: a.float("adjustEdgeFlow", 1.0), ..Default::default() };
        for mv in edit_edge_flow(m, &made, &[], &f) {
            m.set_position(mv.point, mv.position);
        }
    }
    Ok(edges(made))
}

fn subd_face_op(a: &Args, m: &mut Mesh, sel: &Ids, _sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    Ok(faces(subdivide_facets(m, sel, a.int("divisions", 1) as i32, a.int("mode", 0) == 1)))
}

fn subd_edge_op(a: &Args, m: &mut Mesh, sel: &Ids, _sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    subdivide_edges(m, sel, a.int("divisions", 1).max(0) as i32);
    Ok(EditResult::default())
}

fn split_vertex_op(_a: &Args, m: &mut Mesh, sel: &Ids, _sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    split_vertices(m, sel);
    Ok(EditResult::default())
}

fn split_edge_op(_a: &Args, m: &mut Mesh, sel: &Ids, _sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    detach_edges(m, sel);
    Ok(EditResult::default())
}

fn collapse_face_op(a: &Args, m: &mut Mesh, sel: &Ids, _sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    let threshold = a.bool("useAreaThreshold").then(|| a.float("areaThreshold", 0.1));
    let mut made = Vec::new();
    for &f in sel {
        if !m.face_valid(f) || threshold.is_some_and(|t| m.face_area(f) > t) {
            continue;
        }
        let es: Vec<Index> = m.face_corners(f).into_iter().map(|c| m.corner_edge(c)).collect();
        made.extend(collapse_edges(m, &es));
    }
    Ok(points(made))
}

fn circularize_op(a: &Args, m: &mut Mesh, sel: &Ids, _sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    for chain in chain_edges(m, sel) {
        if chain.closed {
            circularize_loop(m, &chain.points, a.float("radialOffset", 0.0), on(a, "evenlyDistribute", true), a.float("twist", 0.0));
        }
    }
    Ok(edges(sel.clone()))
}

fn hole_op(a: &Args, m: &mut Mesh, sel: &Ids, _sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    let assign = on(a, "assignHole", true);
    let valid: Vec<Index> = sel.iter().copied().filter(|&f| m.face_valid(f)).collect();
    let holes = m.face_attrs_mut().get_or_add::<bool>("hole");
    for f in valid {
        holes[f as usize] = assign;
    }
    Ok(EditResult::default())
}

fn average_vertex_op(a: &Args, m: &mut Mesh, sel: &Ids, _sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    let cap = m.point_capacity() as usize;
    let mut pos: Vec<DVec3> = (0..cap).map(|p| if m.point_valid(p as Index) { m.position(p as Index) } else { DVec3::ZERO }).collect();
    for _ in 0..a.int("iterations", 1).max(1) {
        let prev = pos.clone();
        for &p in sel {
            if !m.point_valid(p) {
                continue;
            }
            let mut sum = DVec3::ZERO;
            let mut n = 0.0;
            m.for_each_point_edge(p, |e| {
                sum += prev[m.edge_other_point(e, p) as usize];
                n += 1.0;
            });
            if n > 0.0 {
                pos[p as usize] = prev[p as usize] * 0.5 + sum / n * 0.5;
            }
        }
    }
    for &p in sel {
        if m.point_valid(p) {
            m.set_position(p, pos[p as usize]);
        }
    }
    Ok(EditResult::default())
}

/// Cuts along points: {edge, t}, {vertex} or {face, position} in the mesh's space, or across a ring with loopEdge.
pub(crate) fn split_op(a: &Args, m: &mut Mesh, _sel: &Ids, _sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    let loop_edge = a.int("loopEdge", -1);
    if loop_edge >= 0 {
        if !m.edge_valid(loop_edge as Index) {
            fail!("polySplit: no edge {loop_edge}");
        }
        let plan = knife_loop_plan(m, loop_edge as Index, a.float("fraction", 0.5), None);
        return Ok(edges(knife_apply(m, &plan)));
    }
    let mut pts = Vec::new();
    for p in a.get("points").and_then(Value::as_array).into_iter().flatten() {
        let id = |k: &str| p.get(k).and_then(Value::as_u64).map(|v| v as Index);
        if let Some(v) = id("vertex") {
            pts.push(KnifePoint::point(v));
        } else if let Some(e) = id("edge") {
            pts.push(KnifePoint::edge(e, p.get("t").and_then(Value::as_f64).unwrap_or(0.5).clamp(0.0, 1.0)));
        } else if let (Some(f), Some(at)) = (id("face"), p.get("position")) {
            pts.push(KnifePoint::face(f, crate::mesh_edit::vec(at)));
        }
    }
    if pts.len() == 1 && pts[0].kind == KnifeKind::Edge {
        if !m.edge_valid(pts[0].id) {
            fail!("polySplit: no edge {}", pts[0].id);
        }
        m.split_edge(pts[0].id, pts[0].t);
        return Ok(edges(vec![m.last_split_edge()]));
    }
    if pts.len() < 2 {
        fail!("polySplit: give at least two points, or one on an edge");
    }
    let mut o = KnifeOptions { eye: None, view_dir: DVec3::ZERO, closed: a.bool("closed"), through: a.bool("through"), symmetry: None };
    if let Some(e) = a.vec3("eye") {
        if a.bool("useEye") {
            o.eye = Some(DVec3::from_array(e));
        }
    }
    if let Some(d) = a.vec3("direction") {
        if a.bool("useDirection") {
            o.view_dir = DVec3::from_array(d);
        }
    }
    let plan = knife_plan(m, &pts, &o);
    Ok(edges(knife_apply(m, &plan)))
}

/// The flags of polySplit a knife cut records; polyKnife fills them too.
pub(crate) fn split_flags(cmd: Cmd) -> Cmd {
    let t = |n: &str, ty: T, doc: &str| -> FlagDef { fl(n, "", ty, Value::Null, doc) };
    with_table(
        cmd.flag(t("points", T::List, "The cut's points: {edge, t}, {vertex} or {face, position}, object space."))
            .flag(t("closed", T::Bool, "Also cut from the last point back to the first."))
            .flag(t("through", T::Bool, "Cut every face under the line, hidden faces too."))
            .flag(t("eye", T::Vec3, "Object-space eye the cut was seen from."))
            .flag(t("useEye", T::Bool, "eye is set."))
            .flag(t("direction", T::Vec3, "Object-space direction the cut was seen along."))
            .flag(t("useDirection", T::Bool, "direction is set."))
            .flag(fl("loopEdge", "", T::Int, json!(-1), "Cut across this edge's ring instead."))
            .flag(fl("fraction", "", T::Float, json!(0.5), "Where along loopEdge.")),
        "polySplit",
        &[],
    )
}

/// polySplit's -ep, -fp and -ip as the cut's points: ip as they come, then ep, then fp.
fn split_points(a: &Args) -> Vec<Value> {
    fn flat(v: &Value, out: &mut Vec<f64>) {
        match v {
            Value::Array(x) => x.iter().for_each(|y| flat(y, out)),
            y => out.extend(y.as_f64()),
        }
    }
    let nums = |flag: &str| {
        let mut out = Vec::new();
        if let Some(v) = a.get(flag) {
            flat(v, &mut out);
        }
        out
    };
    let mut pts = Vec::new();
    // -ip e t, or -ip f x y z when the list is one face point: pairs unless the rest doesn't divide by two.
    let ip = nums("insertpoint");
    if ip.len() % 2 == 0 {
        for c in ip.chunks(2) {
            pts.push(json!({"edge": c[0] as u64, "t": c[1]}));
        }
    } else {
        for c in ip.chunks(4) {
            if c.len() == 4 {
                pts.push(json!({"face": c[0] as u64, "position": [c[1], c[2], c[3]]}));
            }
        }
    }
    for c in nums("edgepoint").chunks(2) {
        if c.len() == 2 {
            pts.push(json!({"edge": c[0] as u64, "t": c[1]}));
        }
    }
    for c in nums("facepoint").chunks(4) {
        if c.len() == 4 {
            pts.push(json!({"face": c[0] as u64, "position": [c[1], c[2], c[3]]}));
        }
    }
    pts
}

/// The points along a curve as faces of a mesh it lies on, for polySplit -pc.
fn projected_points(g: &mut crate::graph::Graph, mesh: NodeId, curve_name: &str) -> CmdResult<Vec<Value>> {
    let Some(c) = g.find(curve_name)? else { fail!("polySplit: no curve {curve_name}") };
    let Some(shape) = crate::curves::as_curve(g, c) else { fail!("polySplit: {curve_name} is not a curve") };
    let Some(curve) = crate::curves::world_curve(g, shape) else { fail!("polySplit: {curve_name} has no curve") };
    let Some(cage) = crate::modifiers::cage(g, mesh) else { fail!("polySplit: no mesh") };
    let to_local = g.node(mesh).map_or(glam::DMat4::IDENTITY, |n| n.matrix(Transform::WORLD_MATRIX)).inverse();
    let bvh = aztek_geo::bvh::MeshBvh::new(&cage);
    let hits: Vec<(Index, DVec3)> = curve.tessellate(8).into_iter().filter_map(|p| bvh.closest_point(to_local.transform_point3(p))).map(|h| (h.face, h.point)).collect();
    let mut out: Vec<Value> = Vec::new();
    for (i, &(face, at)) in hits.iter().enumerate() {
        // Where the curve meets an edge it cuts the edge; inside a face only the ends and the turns between faces count.
        let on_edge = cage.face_corners(face).into_iter().find_map(|c| {
            let e = cage.corner_edge(c);
            let (p, q) = (cage.position(cage.edge_point(e, 0)), cage.position(cage.edge_point(e, 1)));
            let t = ((at - p).dot(q - p) / (q - p).length_squared().max(1e-18)).clamp(0.0, 1.0);
            (p.lerp(q, t).distance(at) < 1e-6).then_some((e, t))
        });
        if let Some((e, t)) = on_edge {
            out.push(json!({"edge": e, "t": t}));
        } else if i == 0 || i + 1 == hits.len() || hits[i - 1].0 != face {
            out.push(json!({"face": face, "position": [at.x, at.y, at.z]}));
        }
    }
    Ok(out)
}

/// Keeps a polyMoveVertex-style call's move in one attribute per vector and sets its pivot.
fn pivot_prep(kind: K) -> impl FnMut(&mut crate::graph::Graph, NodeId, &Mesh, &Ids, &mut Args) -> CmdResult<()> {
    move |g, n, m, ids, a| {
        set_pivot(g, n, m, ids, kind, a);
        Ok(())
    }
}

pub(super) fn register_ops() {
    let op = |node_type: &'static str, command: &'static str, label: &'static str, kind: K, run: crate::history::OpFn, doc: &'static str| {
        register_op(HistoryOp { node_type, command, label, kind, run, skip_flags: &[], doc })
    };
    op("polyExtrudeFace", "polyExtrudeFacet", "Extrude", K::Face, extrude_face_op, "Extrudes faces of its input mesh.");
    op("polyExtrudeEdge", "polyExtrudeEdge", "Extrude", K::Edge, extrude_edge_op, "Extrudes edges of its input mesh into strips of faces.");
    op("polyExtrudeVertex", "polyExtrudeVertex", "Extrude Vertex", K::Vertex, extrude_vertex_op, "Extrudes vertices into pyramids.");
    op("polyChamfer", "polyChamferVtx", "Chamfer Vertex", K::Vertex, chamfer_op, "Cuts vertices off, leaving a face each.");
    op("polyBevel3", "polyBevel3", "Bevel", K::Edge, bevel_op, "Bevels edges of its input mesh.");
    op("polyMergeVert", "polyMergeVertex", "Merge", K::Vertex, merge_vert_op, "Welds vertices within a distance.");
    op("polyMergeEdge", "polyMergeEdge", "Merge Edges", K::Edge, merge_edge_op, "Joins two border edges.");
    op("polyMergeFace", "polyMergeFacet", "Merge Faces", K::Face, merge_face_op, "Joins two faces of different shells.");
    op("polyMoveVertex", "polyMoveVertex", "Move Vertices", K::Vertex, move_vertex_op, "Moves vertices.");
    op("polyMoveEdge", "polyMoveEdge", "Move Edges", K::Edge, move_edge_op, "Moves edges.");
    op("polyMoveFace", "polyMoveFacet", "Move Faces", K::Face, move_face_op, "Moves faces.");
    op("polyChipOff", "polyChipOff", "Chip Off", K::Face, chip_off_op, "Copies or moves faces out into their own shell.");
    op("polyDelVertex", "polyDelVertex", "Delete Vertices", K::Vertex, del_vertex_op, "Deletes vertices of its input mesh.");
    op("polyDelEdge", "polyDelEdge", "Delete Edges", K::Edge, del_edge_op, "Deletes edges of its input mesh.");
    op("polyDelFacet", "polyDelFacet", "Delete Faces", K::Face, del_face_op, "Deletes faces of its input mesh.");
    op("polySplitRing", "polySplitRing", "Insert Edge Loop", K::Edge, split_ring_op, "Inserts edge loops across a ring.");
    op("polyNormal", "polyNormal", "Normals", K::Face, normal_op, "Reverses or conforms face winding.");
    op("polyFlipEdge", "polyFlipEdge", "Flip Edge", K::Edge, flip_edge_op, "Flips triangle edges.");
    op("polyQuad", "polyQuad", "Quadrangulate", K::Face, quad_op, "Joins triangles into quads.");
    op("polyPoke", "polyPoke", "Poke", K::Face, poke_op, "Fans faces into triangles round their centres.");
    op("polyWedgeFace", "polyWedgeFace", "Wedge", K::Face, wedge_op, "Extrudes faces in an arc round a hinge.");
    op("polyDuplicateEdge", "polyDuplicateEdge", "Offset Edge Loop", K::Edge, duplicate_edge_op, "Inserts loops either side of edges.");
    op("polyConnectComponents", "polyConnectComponents", "Connect", K::Edge, connect_op, "Connects components with edges.");
    op("polySubdFace", "polySubdivideFacet", "Add Divisions", K::Face, subd_face_op, "Divides faces.");
    op("polySubdEdge", "polySubdivideEdge", "Add Divisions", K::Edge, subd_edge_op, "Divides edges.");
    op("polySplitVert", "polySplitVertex", "Split Vertex", K::Vertex, split_vertex_op, "Gives each face its own copy of a vertex.");
    op("polySplitEdge", "polySplitEdge", "Split Edge", K::Edge, split_edge_op, "Parts the faces along edges.");
    op("polyCollapseF", "polyCollapseFacet", "Collapse Faces", K::Face, collapse_face_op, "Collapses faces to their centres.");
    op("polyCircularize", "polyCircularize", "Circularize", K::Edge, circularize_op, "Fits edge loops to circles.");
    op("polyHole", "polyHole", "Assign Invisible Faces", K::Face, hole_op, "Marks faces as invisible.");
    op("polyAverageVertex", "polyAverageVertex", "Average Vertices", K::Vertex, average_vertex_op, "Moves vertices toward the middle of their neighbours.");
    op("polySplit", "polySplit", "Split", K::Edge, split_op, "Cuts faces along a path of points.");
}

pub(super) fn register(c: &mut Commands<Session>) {
    let legacy = |name: &str, ty: T, default: Value| fl(name, "", ty, default, "Also accepted under this command's other name.");

    let extrude_defaults = [
        ("keepFacesTogether", json!(true)),
        ("keepFacetTogether", json!(true)),
        ("divisions", json!(1)),
        ("gain", json!(1.0)),
        ("taper", json!(1.0)),
        ("smoothingAngle", json!(30.0)),
    ];
    c.register(with_table(
        Cmd::new(
            "polyExtrudeFacet",
            "Extrude faces: -ltz along their normals, -thickness too, -off inward, -ls -lr about each region, -t -r -s about the pivot, \
             -d divisions, -inc along a curve with -tp taper and -twt twist. -kft 0 extrudes each face alone. Returns the node.",
            |s, a| {
                run_op_with(s, "polyExtrudeFace", a, &mut |g, n, m, ids, args| {
                    set_pivot(g, n, m, ids, K::Face, args);
                    add_curve(g, n, args)
                })
            },
        )
        .flag(legacy("individual", T::Bool, json!(false)))
        .flag(legacy("keepBack", T::Bool, json!(true)))
        .flag(world_flag())
        .flag(fl("curvePoints", "", T::List, Value::Null, "Samples of inputCurve when the edit was made, object space.")),
        "polyExtrudeFacet",
        &extrude_defaults,
    ));
    c.register(with_table(
        Cmd::new(
            "polyExtrudeEdge",
            "Extrude edges into strips of faces: -ltz out from their faces, -off outward, -t -r -s about the pivot. Returns the node.",
            |s, a| {
                run_op_with(s, "polyExtrudeEdge", a, &mut |g, n, m, ids, args| {
                    set_pivot(g, n, m, ids, K::Edge, args);
                    Ok(())
                })
            },
        )
        .flag(world_flag()),
        "polyExtrudeEdge",
        &extrude_defaults,
    ));
    c.register(ref_cmd(
        "polyExtrudeVertex",
        "Extrude vertices: each becomes a pyramid's tip, -w wide at the base, -l long along its normal, -d rings. Returns the node.",
        &[("divisions", json!(1))],
        |s, a| run_op(s, "polyExtrudeVertex", a),
    ));
    c.register(
        Cmd::new(
            "polyChamferVtx",
            "Chamfer Vertices: cut each vertex off -w along its edges, leaving one face. Also takes the positional form: history, width, \
             delete face.",
            |s, a| {
                // polyChamferVtx 1 0.25 0 pCube1.vtx[0]: the numbers come first.
                let mut a2 = a.clone();
                let nums: Vec<f64> = a.positional.iter().filter_map(Value::as_f64).collect();
                a2.positional.retain(|v| !v.is_number());
                if let Some(w) = nums.get(1) {
                    a2.flags.insert("width".into(), json!(w));
                }
                run_op(s, "polyChamfer", &a2)
            },
        )
        .positional("history, width, delete face, then vertices")
        .flag(fl("width", "w", T::Float, json!(0.25), "How far along each edge, cm."))
        .flag(fl("constructionHistory", "ch", T::Bool, Value::Null, "Off: no history node."))
        .flag(fl("name", "n", T::String, Value::Null, "Name of the node.")),
    );

    c.register(with_table(
        Cmd::new(
            "polyBevel3",
            "Bevel edges: -o offset, -oaf as a fraction of the shortest edge, -sg segments, -c chamfer (flat) or round, -sa smoothing angle, \
             -mv -mvt merge the result. Returns the node.",
            |s, a| run_op(s, "polyBevel3", a),
        )
        .flag(legacy("profile", T::Float, json!(0.5)))
        .flag(legacy("clamp", T::Bool, json!(true))),
        "polyBevel3",
        &[
            ("offset", json!(0.5)),
            ("fraction", json!(0.5)),
            ("offsetAsFraction", json!(false)),
            ("segments", json!(1)),
            ("smoothingAngle", json!(30.0)),
            ("autoFit", json!(true)),
            ("chamfer", json!(true)),
            ("depth", json!(1.0)),
            ("angleTolerance", json!(20.0)),
        ],
    ));

    c.register(with_table(
        Cmd::new(
            "polyMergeVertex",
            "Weld vertices closer than -d; -am welds exactly two whatever the distance. A mesh means all its vertices. Returns the node.",
            |s, a| run_op(s, "polyMergeVert", a),
        )
        .flag(legacy("average", T::Bool, json!(false))),
        "polyMergeVertex",
        &[("distance", json!(0.0))],
    ));
    c.register(ref_cmd(
        "polyMergeEdge",
        "Join two border edges: -fe -se name them, or give two. -mm 0 at the first, 1 halfway, 2 at the second. Returns the node.",
        &[("mergeMode", json!(1)), ("firstEdge", json!(-1)), ("secondEdge", json!(-1))],
        |s, a| run_op(s, "polyMergeEdge", a),
    ));
    c.register(ref_cmd(
        "polyMergeFacet",
        "Join two faces of different shells with the same number of corners: -ff -sf name them. -mm 0 moves the second to the first, 1 both \
         to the average, 2 the first to the second, 6 nothing. Returns the node.",
        &[("mergeMode", json!(6)), ("firstFacet", json!(-1)), ("secondFacet", json!(-1))],
        |s, a| run_op(s, "polyMergeFace", a),
    ));

    let move_defaults = [("gain", json!(1.0))];
    c.register(with_table(
        Cmd::new("polyMoveVertex", "Move vertices: -t -r -s about the pivot, -ltz along each normal. Returns the node.", |s, a| {
            let mut prep = pivot_prep(K::Vertex);
            run_op_with(s, "polyMoveVertex", a, &mut prep)
        })
        .flag(world_flag()),
        "polyMoveVertex",
        &move_defaults,
    ));
    c.register(with_table(
        Cmd::new("polyMoveEdge", "Move edges: -t -r -s about the pivot, -lt -ls -lr each in its own frame. Returns the node.", |s, a| {
            let mut prep = pivot_prep(K::Edge);
            run_op_with(s, "polyMoveEdge", a, &mut prep)
        })
        .flag(world_flag()),
        "polyMoveEdge",
        &move_defaults,
    ));
    c.register(with_table(
        Cmd::new("polyMoveFacet", "Move faces: -t -r -s about the pivot, -ltz -ls -lr each in its own frame. Returns the node.", |s, a| {
            let mut prep = pivot_prep(K::Face);
            run_op_with(s, "polyMoveFace", a, &mut prep)
        })
        .flag(world_flag()),
        "polyMoveFacet",
        &move_defaults,
    ));
    c.register(with_table(
        Cmd::new(
            "polyChipOff",
            "Chip faces off into their own shell of the mesh, moved by -ltz -t and the rest. -dup 0 removes them from where they were. Returns \
             the node.",
            |s, a| {
                let mut prep = pivot_prep(K::Face);
                run_op_with(s, "polyChipOff", a, &mut prep)
            },
        )
        .flag(world_flag()),
        "polyChipOff",
        &[("duplicate", json!(true)), ("keepFacesTogether", json!(true)), ("keepFacetTogether", json!(true)), ("gain", json!(1.0))],
    ));

    c.register(
        Cmd::new("polyDelVertex", "Delete vertices, merging the faces round them. Returns the node.", |s, a| run_op(s, "polyDelVertex", a))
            .flag(fl("withFaces", "", T::Bool, json!(false), "Delete the faces round them instead."))
            .positional("vertices (the selection when none)")
            .flag(fl("constructionHistory", "ch", T::Bool, Value::Null, "Off: no history node."))
            .flag(fl("name", "n", T::String, Value::Null, "Name of the node.")),
    );
    c.register(with_table(
        Cmd::new("polyDelEdge", "Delete edges, merging the faces beside them. -cv also removes the vertices that leaves redundant. Returns the node.", |s, a| {
            run_op(s, "polyDelEdge", a)
        })
        .flag(fl("withFaces", "", T::Bool, json!(false), "Delete the faces beside them instead.")),
        "polyDelEdge",
        &[],
    ));
    c.register(ref_cmd("polyDelFacet", "Delete faces. Returns the node.", &[], |s, a| run_op(s, "polyDelFacet", a)));

    c.register(with_table(
        Cmd::new(
            "polySplitRing",
            "Insert edge loops across the ring through the first edge (or -re). -wt where across, 0 to 1. -stp 2 with -div inserts that many \
             evenly. -ief -aef follow the surface. Returns the node.",
            |s, a| run_op(s, "polySplitRing", a),
        )
        .flag(legacy("fraction", T::Float, json!(0.5)))
        .flag(legacy("count", T::Int, json!(1)))
        .flag(legacy("spacing", T::Float, json!(1.0))),
        "polySplitRing",
        &[("weight", json!(0.5)), ("divisions", json!(2)), ("rootEdge", json!(-1)), ("smoothingAngle", json!(30.0)), ("adjustEdgeFlow", json!(1.0))],
    ));

    c.register(ref_cmd(
        "polyNormal",
        "Face winding: -nm 0 reverse, 1 propagate, 2 conform, 3 reverse and cut, 4 reverse and propagate. Returns the node.",
        &[("normalMode", json!(0)), ("userNormalMode", json!(true))],
        |s, a| run_op(s, "polyNormal", a),
    ));
    c.register(ref_cmd("polyFlipEdge", "Flip edges between triangles to join their other corners. Returns the node.", &[], |s, a| run_op(s, "polyFlipEdge", a)));
    c.register(ref_cmd(
        "polyQuad",
        "Join triangles into quads where they meet at less than -a degrees. Returns the node.",
        &[("angle", json!(30.0)), ("keepGroupBorder", json!(true)), ("keepHardEdges", json!(true)), ("keepTextureBorders", json!(true))],
        |s, a| run_op(s, "polyQuad", a),
    ));
    c.register(with_table(
        Cmd::new("polyPoke", "Fan faces into triangles round a new point, moved by -lt along each face's frame or -t. Returns the node.", |s, a| {
            let mut prep = pivot_prep(K::Face);
            run_op_with(s, "polyPoke", a, &mut prep)
        })
        .flag(world_flag()),
        "polyPoke",
        &[],
    ));
    c.register(ref_cmd(
        "polyWedgeFace",
        "Extrude faces in an arc: -wa degrees, -d divisions, round the edge -ed or the -ax axis through -cen. Returns the node.",
        &[("divisions", json!(1))],
        |s, a| run_op(s, "polyWedgeFace", a),
    ));
    c.register(ref_cmd(
        "polyDuplicateEdge",
        "Offset Edge Loop's node: insert a loop either side of edges, -of along each crossing edge. -de also removes the edges. Returns the node.",
        &[("offset", json!(0.5))],
        |s, a| run_op(s, "polyDuplicateEdge", a),
    ));
    c.register(
        with_table(
            Cmd::new(
                "polyConnectComponents",
                "Connect vertices, edges or faces with new edges through the faces between them: vertices directly, edges at their midpoints, \
                 faces along the edges they share. -ief -aef follow the surface. Returns the node.",
                |s, a| {
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
                        fail!("polyConnectComponents: select vertices, edges or faces");
                    }
                    let mut per: BTreeMap<NodeId, (Vec<Index>, Vec<Index>)> = BTreeMap::new();
                    for (n, kind, ids) in picked {
                        let Some(cage) = crate::modifiers::cage(&mut s.graph, n) else { continue };
                        let e = per.entry(n).or_default();
                        match kind {
                            K::Vertex => e.0.extend(ids),
                            K::Edge => e.1.extend(ids),
                            _ => {
                                let set: BTreeSet<Index> = ids.iter().copied().collect();
                                let all = components::convert(&cage, K::Face, &ids, K::Edge, false);
                                e.1.extend(all.into_iter().filter(|&x| {
                                    let mut both = 0;
                                    cage.for_each_edge_corner(x, |c| both += set.contains(&cage.corner_face(c)) as i32);
                                    both == 2
                                }));
                            }
                        }
                    }
                    let op = crate::history::op("polyConnectComponents").unwrap();
                    let follow = components::mode(&s.graph);
                    let folded = fold_vectors(a);
                    let g = &mut s.graph;
                    g.open_chunk("Connect");
                    let r = (|| -> CmdResult<Vec<NodeId>> {
                        let mut made = Vec::new();
                        for (n, (verts, es)) in per {
                            let mut args = folded.clone();
                            if !verts.is_empty() {
                                args.flags.insert("vertices".into(), json!(verts));
                            }
                            let (a1, op1) = (&args, &*op);
                            made.extend(apply_targets(
                                g,
                                K::Edge,
                                BTreeMap::from([(n, es)]),
                                &mut |m, sel, sym| connect_op(a1, m, sel, sym),
                                Some((op1, a1)),
                                a.named().is_empty().then_some(follow),
                            )?);
                        }
                        Ok(made)
                    })();
                    g.close_chunk();
                    r.map(|made| names_of(g, &made))
                },
            )
            .flag(fl("vertices", "", T::List, Value::Null, "Vertices the edit also connects.")),
            "polyConnectComponents",
            &[("adjustEdgeFlow", json!(1.0))],
        ),
    );
    c.register(ref_cmd(
        "polySubdivideFacet",
        "Add Divisions to faces: -dv levels, -m 0 quads, 1 triangles. Returns the node.",
        &[("divisions", json!(1)), ("mode", json!(0))],
        |s, a| run_op(s, "polySubdFace", a),
    ));
    c.register(ref_cmd(
        "polySubdivideEdge",
        "Add Divisions to edges: -dv new points on each. Returns the node.",
        &[("divisions", json!(1))],
        |s, a| run_op(s, "polySubdEdge", a),
    ));
    c.register(ref_cmd("polySplitVertex", "Give each face at each vertex its own copy of it. Returns the node.", &[], |s, a| run_op(s, "polySplitVert", a)));
    c.register(ref_cmd(
        "polySplitEdge",
        "Part the faces along edges, wherever the edges divide them at a vertex. Returns the node.",
        &[],
        |s, a| run_op(s, "polySplitEdge", a),
    ));
    c.register(ref_cmd(
        "polyDetachEdges",
        "Detach Components: vertices split at each face, edges part the faces beside them. Returns the node.",
        &[],
        |s, a| {
            let vertices = match named_members(s, a)?.first() {
                Some(m) => m.kind == K::Vertex,
                None => components::mode(&s.graph) == K::Vertex,
            };
            run_op(s, if vertices { "polySplitVert" } else { "polySplitEdge" }, a)
        },
    ));
    c.register(
        Cmd::new("DetachComponent", "Detach the selected vertices or edges: polyDetachEdges on the selection.", |s, _a| {
            aztek_ops::execute(s, "polyDetachEdges", &json!({}), aztek_ops::CallSource::Script)
        }),
    );
    extend(c, "polyCrease", "polyCrease", &[], |_, _| None);
    extend(c, "polyAppendVertex", "polyAppendVertex", &[], |_, _| None);
    c.register(ref_cmd(
        "polyCollapseFacet",
        "Collapse faces to their centres; with -uat only those under -at in area. Returns the node.",
        &[("areaThreshold", json!(0.1))],
        |s, a| run_op(s, "polyCollapseF", a),
    ));
    c.register(ref_cmd("polyCollapseEdge", "Collapse each connected group of edges to its centre. Returns the node.", &[], |s, a| run_op(s, "polyCollapseEdge", a)));
    c.register(ref_cmd("polyCloseBorder", "Fill the holes whose borders the edges lie on; a mesh means all its holes. Returns the node.", &[], |s, a| {
        run_op(s, "polyCloseBorder", a)
    }));
    c.register(with_table(
        query_flags(Cmd::new("polyHole", "Assign Invisible Faces: -ah 1 marks faces invisible, 0 clears them. -q -ah tells whether all are. Returns the node.", |s, a| {
            if a.bool("query") {
                let (targets, _) = gather(s, a, K::Face)?;
                let mut all = true;
                for (n, ids) in targets {
                    let Some(m) = crate::modifiers::cage(&mut s.graph, n) else { continue };
                    let hole = m.face_attrs().get::<bool>("hole");
                    all = all && !ids.is_empty() && ids.iter().all(|&f| hole.get(f as usize).copied().unwrap_or(false));
                }
                return Ok(json!(all));
            }
            run_op(s, "polyHole", a)
        })),
        "polyHole",
        &[("assignHole", json!(true))],
    ));
    c.register(ref_cmd(
        "polyAverageVertex",
        "Average Vertices: each vertex moves toward the middle of its neighbours, -i times. Returns the node.",
        &[("iterations", json!(1))],
        |s, a| run_op(s, "polyAverageVertex", a),
    ));

    c.register(split_flags(
        Cmd::new(
            "polySplit",
            "Split faces along a path: -ep edge fraction, -fp face x y z, -ip the same in order. -pc splits along a curve lying on the mesh. \
             Returns the node.",
            |s, a| {
                let mut a2 = a.clone();
                let mut pts = split_points(a);
                if let Some(curves) = a.get("projectedCurve") {
                    let names: Vec<String> = match curves {
                        Value::Array(v) => v.iter().filter_map(|x| x.as_str().map(str::to_string)).collect(),
                        v => v.as_str().map(str::to_string).into_iter().collect(),
                    };
                    let (targets, _) = gather(s, a, K::Edge)?;
                    let Some(&mesh) = targets.keys().next() else { fail!("polySplit: name the mesh to split") };
                    for n in names {
                        pts.extend(projected_points(&mut s.graph, mesh, &n)?);
                    }
                }
                if !pts.is_empty() {
                    a2.flags.insert("points".into(), Value::Array(pts));
                }
                run_split(s, &a2)
            },
        ),
    ));
    c.register(split_flags(Cmd::new("polySplitFacet", "polySplit's older name.", |s, a| {
        let mut a2 = a.clone();
        a2.flags.insert("points".into(), Value::Array(split_points(a)));
        run_split(s, &a2)
    })));
}

/// polySplit on one mesh: its ids hold nothing, the path being in its flags.
fn run_split(s: &mut Session, a: &Args) -> CmdResult {
    let g = &mut s.graph;
    g.evaluate();
    let names = a.named();
    let mesh = match names.first() {
        Some(n) => {
            let head = n.split('.').next().unwrap_or(n);
            crate::mesh_edit::mesh_arg(g, head, "polySplit")?
        }
        None => crate::mesh_edit::mesh_flag(g, &Args::default(), "polySplit")?,
    };
    let op = crate::history::op("polySplit").unwrap();
    g.open_chunk("Split");
    let r = apply_targets(g, K::Edge, BTreeMap::from([(mesh, Vec::new())]), &mut |m, sel, sym| split_op(a, m, sel, sym), Some((&*op, a)), None);
    g.close_chunk();
    r.map(|made| names_of(g, &made))
}
