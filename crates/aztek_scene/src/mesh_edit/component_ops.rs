//! Extrude, inset, bevel, delete, loops, merge, weld, bridge and the
//! other edits of selected components.

use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use aztek_geo::loop_tools::{edit_edge_flow, offset_edge_loops, EdgeFlowOptions, OffsetEnds, OffsetLoopOptions, OffsetSide};
use aztek_geo::ops::*;
use aztek_geo::topology::faces_of_points;
use aztek_geo::triangulate::{QuadSplit, TriangulateOptions};
use aztek_geo::{Index, Mesh, MeshRemap, INVALID};
use aztek_ops::{execute, fail, ArgType, Args, CallSource, CmdResult, CommandDef, Commands, FlagDef};
use serde_json::{json, Value};

use super::*;
use crate::components::{self, ComponentMode as K, Ids};
use crate::isolation;
use crate::mesh_nodes::MeshNode;
use crate::modifiers;
use crate::plug::{NodeId, PlugRef, PlugValue};
use crate::session::Session;
use crate::transform::Transform;

type T = ArgType;
type Cmd = CommandDef<Session>;

fn flag(name: &str, ty: ArgType, default: Value, doc: &str) -> FlagDef {
    FlagDef::new(name, "", ty, default, doc)
}

/// The reference's default for a component edit run in object mode: every component of kind on the selected meshes gets
/// selected and kind becomes the mode, so Extrude on an object extrudes all its faces.
fn select_whole_in_object_mode(g: &mut crate::graph::Graph, kind: K) {
    if components::mode(g) != K::Object {
        return;
    }
    g.evaluate();
    let mut all = BTreeMap::new();
    for sel in g.selection().to_vec() {
        let Some(id) = components::as_mesh(g, sel) else { continue };
        if let Some(cage) = modifiers::cage(g, id) {
            let ids = components::all_ids(&cage, kind);
            if !ids.is_empty() {
                all.insert(id, ids);
            }
        }
    }
    if all.is_empty() {
        return;
    }
    let pick = |k: K| if k == kind { all.clone() } else { BTreeMap::new() };
    components::set_all(g, pick(K::Vertex), pick(K::Edge), pick(K::Face));
    components::set_mode_undoable(g, kind);
}

/// The named curve's first spline as a polyline in the selected mesh's space, for polyBridge's Path.
fn bridge_path(g: &mut crate::graph::Graph, name: &str) -> CmdResult<Vec<glam::DVec3>> {
    if name.is_empty() {
        return Ok(Vec::new());
    }
    g.evaluate();
    let Some(c) = g.find(name)? else { fail!("polyBridge: no curve {name}") };
    let pts = curve_path(g, c).map_err(|e| aztek_ops::CommandError::new(format!("polyBridge: {e}")))?;
    if pts.is_empty() {
        fail!("polyBridge: {name} has no curve");
    }
    let mut to_local = glam::DMat4::IDENTITY;
    let sel = components::selected_all(g, components::mode(g));
    if sel.len() == 1 {
        let node = *sel.keys().next().unwrap();
        if let Some(n) = g.node(node) {
            to_local = n.matrix(Transform::WORLD_MATRIX).inverse();
        }
    }
    Ok(pts.into_iter().map(|p| to_local.transform_point3(p)).collect())
}

/// Edges with exactly one of the faces on them: the outline of each region the faces make.
fn region_border_edges(m: &Mesh, faces: &[Index]) -> Ids {
    let sel: BTreeSet<Index> = faces.iter().copied().collect();
    let mut out = BTreeSet::new();
    for &f in faces {
        if !m.face_valid(f) {
            continue;
        }
        m.for_each_face_corner(f, |c| {
            let e = m.corner_edge(c);
            let mut count = 0;
            m.for_each_edge_corner(e, |ec| count += sel.contains(&m.corner_face(ec)) as i32);
            if count == 1 {
                out.insert(e);
            }
        });
    }
    out.into_iter().collect()
}

const PLACEMENT: [usize; 8] = [
    Transform::TRANSLATE,
    Transform::ROTATE,
    Transform::SCALE,
    Transform::ROTATE_PIVOT,
    Transform::SCALE_PIVOT,
    Transform::ROTATE_PIVOT_TRANSLATE,
    Transform::SCALE_PIVOT_TRANSLATE,
    Transform::ROTATE_ORDER,
];

/// A new mesh beside `like` (same type, parent and local transform) holding `mesh`.
/// A new mesh (transform and shape) placed as like's transform is, holding mesh. Returns the shape.
pub(crate) fn mesh_beside(g: &mut crate::graph::Graph, like: NodeId, mesh: Mesh) -> CmdResult<NodeId> {
    let like_xf = crate::dag::transform_of(g, like);
    let name = g.node(like_xf).unwrap().name().to_string();
    let parent = g.parent(like_xf);
    let (xf, piece) = crate::dag::create_with_shape(g, "mesh", &name, parent)?;
    if like_xf != like {
        for attr in PLACEMENT {
            let v = g.stored_value(&PlugRef::new(like_xf, attr as i32)).clone();
            g.set_value(PlugRef::new(xf, attr as i32), v)?;
        }
    }
    g.set_value(PlugRef::new(piece, MeshNode::IN_MESH as i32), PlugValue::Mesh(Some(Arc::new(mesh))))?;
    Ok(piece)
}

/// polyExtrude duplicate=True: each mesh's selected faces extruded into a new mesh beside it, which ends up selected
/// with its cap.
fn extrude_duplicate(s: &mut Session, a: &Args, keep_sides: bool) -> CmdResult {
    let g = &mut s.graph;
    g.evaluate();
    let mode = components::mode(g);
    let mut targets = BTreeMap::new();
    for (node, ids) in components::selected_all(g, mode).clone() {
        if !components::is_mesh(g, node) {
            continue;
        }
        if let Some(cage) = modifiers::cage(g, node) {
            let f = components::convert(&cage, mode, &ids, K::Face, false);
            if !f.is_empty() {
                targets.insert(node, f);
            }
        }
    }
    if targets.is_empty() {
        fail!("polyExtrude: no faces selected");
    }

    g.open_chunk("Extrude");
    let r = (|| -> CmdResult {
        let mut out = Vec::new();
        let (mut made, mut sources) = (Vec::new(), Vec::new());
        let mut caps = BTreeMap::new();
        for (node, sel) in targets {
            let base = base_plug(g, node)?;
            let mut m = base_mesh(g, &base);
            let sym = symmetry_for(g, node, &mut m);
            let o = ExtrudeOptions {
                offset: a.float("thickness", 0.0),
                individual: a.bool("individual"),
                symmetry: sym.as_ref(),
                inset: a.float("offset", 0.0),
                keep_back: a.bool("keepBack"),
            };
            let mut copy = extrude_copy(&m, &sel, &o, keep_sides);
            if copy.cap.is_empty() {
                continue;
            }
            copy.mesh.compact(None);
            let piece = mesh_beside(g, node, copy.mesh)?;
            out.push(json!(components::display_path(g, piece)));
            made.push(piece);
            sources.push(node);
            caps.insert(piece, copy.cap);
        }
        isolation::inherit(g, &made, &sources);
        let made_xf: Vec<NodeId> = made.iter().map(|&m| crate::dag::transform_of(g, m)).collect();
        g.select(&made_xf, crate::graph::SelectMode::Replace);
        // The cap stays selected in face mode, so the extrude handles and the last-op panel keep working on the copy.
        components::keep_mode_after_create(g);
        components::set_all(g, BTreeMap::new(), BTreeMap::new(), caps);
        if components::mode(g) != K::Face {
            components::set_mode_undoable(g, K::Face);
        }
        Ok(Value::Array(out))
    })();
    g.close_chunk();
    r
}

fn all_faces(m: &Mesh) -> Ids {
    (0..m.face_capacity()).filter(|&f| m.face_valid(f)).collect()
}

fn inset_op(a: &Args, m: &mut Mesh, sel: &Ids, sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    let o = InsetOptions {
        thickness: a.float("thickness", 5.0),
        depth: a.float("depth", 0.0),
        individual: a.bool("individual"),
        symmetry: sym,
        clamp: a.bool("clamp"),
    };
    Ok(faces(inset_faces(m, sel, &o).faces))
}


fn split_rings_with(a: &Args, m: &mut Mesh, picked: &Ids, sym: Option<&SymmetryMap>, kind: K) -> CmdResult<EditResult> {
    let count = a.int("count", 1).max(1) as i32;
    let across = a.string("direction", "along") == "across";
    let sel = if picked.is_empty() { all_faces(m) } else { picked.clone() };
    let ring_edges = if kind == K::Face { region_ring_edges(m, &sel, across) } else { sel };
    let made = split_rings(m, &ring_edges, count, sym);
    if a.bool("edgeFlow") && !made.is_empty() {
        for mv in edit_edge_flow(m, &made, &[], &EdgeFlowOptions::default()) {
            m.set_position(mv.point, mv.position);
        }
    }
    Ok(edges(made))
}

fn split_rings_face_op(a: &Args, m: &mut Mesh, sel: &Ids, sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    split_rings_with(a, m, sel, sym, K::Face)
}

fn split_rings_edge_op(a: &Args, m: &mut Mesh, sel: &Ids, sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    split_rings_with(a, m, sel, sym, K::Edge)
}

fn offset_loop_op(a: &Args, m: &mut Mesh, sel: &Ids, sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    let o = OffsetLoopOptions {
        distance: a.float("distance", 0.25).max(0.0),
        absolute: a.bool("absolute"),
        side: match a.string("side", "both").as_str() {
            "left" => OffsetSide::Left,
            "right" => OffsetSide::Right,
            _ => OffsetSide::Both,
        },
        ends: if a.string("ends", "converge") == "wrap" { OffsetEnds::Wrap } else { OffsetEnds::Converge },
        symmetry: sym,
    };
    Ok(edges(offset_edge_loops(m, sel, &o, a.bool("deleteEdge"))))
}

fn merge_with(a: &Args, m: &mut Mesh, sel: &Ids, sym: Option<&SymmetryMap>, pair: bool) -> CmdResult<EditResult> {
    let mut o = MergeOptions { distance: a.float("distance", 0.01), average: a.bool("average") || sym.is_some(), symmetry: sym };
    // Just past the pair's own gap, so a mirrored pair welds too without reaching further.
    if pair && sel.len() == 2 && m.point_valid(sel[0]) && m.point_valid(sel[1]) {
        o.distance = o.distance.max(m.position(sel[0]).distance(m.position(sel[1])) * 1.001 + 1e-6);
    }
    Ok(points(merge_by_distance(m, sel, &o)))
}

fn merge_all_op(a: &Args, m: &mut Mesh, sel: &Ids, sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    merge_with(a, m, sel, sym, false)
}

fn merge_center_op(_a: &Args, m: &mut Mesh, sel: &Ids, _sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    let p = merge_to_center(m, sel);
    Ok(points(if p == INVALID { Vec::new() } else { vec![p] }))
}

fn collapse_op(_a: &Args, m: &mut Mesh, sel: &Ids, sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    Ok(points(collapse_edges(m, &with_mirror(m, sym, sel, K::Edge))))
}

fn dissolve_vertex_op(_a: &Args, m: &mut Mesh, sel: &Ids, sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    let all = with_mirror(m, sym, sel, K::Vertex);
    Ok(faces(dissolve_points(m, &all)))
}

fn dissolve_edge_op(_a: &Args, m: &mut Mesh, sel: &Ids, sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    let all = with_mirror(m, sym, sel, K::Edge);
    Ok(faces(dissolve_edges(m, &all, true)))
}

fn spin_op(a: &Args, m: &mut Mesh, sel: &Ids, sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    let forward = a.int("offset", 1) >= 0;
    Ok(edges(spin_mirrored(m, sel, sym, forward)))
}

/// Spins edges one step, the mirror side the other way so the two sides stay images of each other. Backward goes round
/// the polygon the two faces make, all but one of the way.
pub(crate) fn spin_mirrored(m: &mut Mesh, sel: &Ids, sym: Option<&SymmetryMap>, forward: bool) -> Vec<Index> {
    let ids = with_mirror(m, sym, sel, K::Edge);
    let wedge = sym.map(|s| s.majority_wedge(&components::convert(m, K::Edge, sel, K::Vertex, false)));
    let sides: Vec<bool> = ids
        .iter()
        .map(|&e| match (sym, wedge) {
            (Some(s), Some(w)) => m.edge_valid(e) && s.wedge(m.edge_point(e, 0)) != w && s.wedge(m.edge_point(e, 1)) != w,
            _ => false,
        })
        .collect();
    let mut out = Vec::new();
    for (e, mirror_side) in ids.into_iter().zip(sides) {
        if forward != mirror_side {
            out.extend(spin_edges(m, &[e]));
            continue;
        }
        let mut count = 0;
        m.for_each_edge_corner(e, |c| count += m.face_points(m.corner_face(c)).len());
        let around = count.saturating_sub(2);
        let period = if around % 2 == 0 { around / 2 } else { around };
        let mut at = vec![e];
        for _ in 1..period.max(2) {
            at = spin_edges(m, &at);
        }
        out.extend(at);
    }
    out
}

fn fill_hole_op(_a: &Args, m: &mut Mesh, sel: &Ids, _sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    Ok(faces(fill_holes(m, sel)))
}

fn conform_op(_a: &Args, m: &mut Mesh, sel: &Ids, _sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    conform_normals(m, sel);
    Ok(EditResult::default())
}

fn triangulate_op(a: &Args, m: &mut Mesh, sel: &Ids, sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    let o = TriangulateOptions {
        beauty: a.bool("beauty"),
        quads: if a.string("quadSplit", "shortest") == "first" { QuadSplit::FirstCorner } else { QuadSplit::ShortestDiagonal },
    };
    let mut all = with_mirror(m, sym, sel, K::Face);
    if all.is_empty() {
        all = all_faces(m);
    }
    Ok(faces(triangulate_faces(m, &all, &o)))
}

fn bevel_with(a: &Args, m: &mut Mesh, picked: &Ids, sym: Option<&SymmetryMap>, from_faces: bool) -> CmdResult<EditResult> {
    let offset = a.float("offset", 2.0);
    let sel = if from_faces { region_border_edges(m, &with_mirror(m, sym, picked, K::Face)) } else { picked.clone() };
    let applied = Cell::new(offset);
    let o = BevelOptions {
        offset,
        segments: a.int("segments", 1).max(1) as i32,
        profile: a.float("profile", 0.5),
        clamp: a.bool("clamp"),
        smoothing_angle: a.float("smoothingAngle", 30.0),
        symmetry: sym,
        applied_offset: Some(&applied),
    };
    let mut r = faces(bevel_edges(m, &sel, &o));
    r.info = json!(applied.get());
    Ok(r)
}

fn bevel_face_op(a: &Args, m: &mut Mesh, sel: &Ids, sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    bevel_with(a, m, sel, sym, true)
}

fn grid_fill_with(a: &Args, m: &mut Mesh, sel: &Ids, sym: Option<&SymmetryMap>, kind: K) -> CmdResult<EditResult> {
    let o = GridFillOptions { grid_size: a.float("gridSize", 0.4).clamp(0.05, 1.0), rotate: a.int("rotate", 0) as i32 };
    let all = with_mirror(m, sym, sel, kind);
    if kind == K::Edge {
        return Ok(faces(grid_fill_holes(m, &all, &o, sym)));
    }
    let fs = if kind == K::Vertex { faces_of_points(m, &all) } else { all };
    Ok(faces(grid_fill_faces(m, &fs, &o, sym)))
}

fn grid_fill_vertex_op(a: &Args, m: &mut Mesh, sel: &Ids, sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    grid_fill_with(a, m, sel, sym, K::Vertex)
}

fn grid_fill_edge_op(a: &Args, m: &mut Mesh, sel: &Ids, sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    grid_fill_with(a, m, sel, sym, K::Edge)
}

fn grid_fill_face_op(a: &Args, m: &mut Mesh, sel: &Ids, sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    grid_fill_with(a, m, sel, sym, K::Face)
}

fn register_history_ops() {
    use crate::history::{register_op, HistoryOp};
    for (node_type, kind, run) in [
        ("polyGridFillVertex", K::Vertex, grid_fill_vertex_op as crate::history::OpFn),
        ("polyGridFillEdge", K::Edge, grid_fill_edge_op),
        ("polyGridFillFace", K::Face, grid_fill_face_op),
    ] {
        register_op(HistoryOp { node_type, command: "polyGridFill", label: "Grid Fill", kind, run, skip_flags: &[], doc: "Rebuilds patches as a quad grid." });
    }
    register_op(HistoryOp {
        node_type: "polyBevelFace",
        command: "polyBevel",
        label: "Bevel",
        kind: K::Face,
        run: bevel_face_op,
        skip_flags: &[],
        doc: "Bevels the borders of face regions of its input mesh.",
    });
    register_op(HistoryOp {
        node_type: "polyBridgeEdge",
        command: "polyBridge",
        label: "Bridge",
        kind: K::Edge,
        run: bridge_edge_op,
        skip_flags: &["curve"],
        doc: "Bridges border edges of its input mesh.",
    });
    register_op(HistoryOp {
        node_type: "polyBridgeFace",
        command: "polyBridge",
        label: "Bridge",
        kind: K::Face,
        run: bridge_face_op,
        skip_flags: &["curve"],
        doc: "Deletes faces of its input mesh and bridges the holes.",
    });
    let op = |node_type: &'static str, command: &'static str, label: &'static str, kind: K, run: crate::history::OpFn, doc: &'static str| HistoryOp {
        node_type,
        command,
        label,
        kind,
        run,
        skip_flags: &[],
        doc,
    };
    register_op(op("polySplitRingsFace", "polySplitRings", "Split Rings", K::Face, split_rings_face_op, "Splits the rings across faces."));
    register_op(op("polySplitRingsEdge", "polySplitRings", "Split Rings", K::Edge, split_rings_edge_op, "Splits the rings through edges."));
    register_op(op("polyOffsetLoop", "polyOffsetLoop", "Offset Edge Loop", K::Edge, offset_loop_op, "Inserts loops either side of edges."));
    register_op(op("polyMergeVertAll", "polyMerge", "Merge", K::Vertex, merge_all_op, "Welds vertices within a distance."));
    register_op(op("polyMergeCenter", "polyMergeCenter", "Merge to Center", K::Vertex, merge_center_op, "Welds vertices at their centre."));
    register_op(op("polyCollapseEdge", "polyCollapse", "Collapse", K::Edge, collapse_op, "Collapses edges to their centres."));
    register_op(op("polyDissolveVertex", "polyDissolve", "Dissolve", K::Vertex, dissolve_vertex_op, "Dissolves vertices."));
    register_op(op("polyDissolveEdge", "polyDissolve", "Dissolve", K::Edge, dissolve_edge_op, "Dissolves edges."));
    register_op(op("polySpinEdge", "polySpinEdge", "Spin Edge", K::Edge, spin_op, "Spins edges within their faces."));
    register_op(op("polyCloseBorder", "polyFillHole", "Fill Hole", K::Edge, fill_hole_op, "Caps holes."));
    register_op(op("polyConform", "polyConform", "Conform", K::Face, conform_op, "Makes face winding consistent."));
    register_op(op("polyTriangulate", "polyTriangulate", "Triangulate", K::Face, triangulate_op, "Triangulates faces."));
    register_op(HistoryOp {
        node_type: "polyInset",
        command: "polyInset",
        label: "Inset",
        kind: K::Face,
        run: inset_op,
        skip_flags: &[],
        doc: "Insets faces of its input mesh.",
    });
}

pub fn register(c: &mut Commands<Session>) {
    register_history_ops();
    c.register(
        Cmd::new(
            "polyExtrude",
            "Extrude the selected faces along their normals, or in edge mode grow a strip of faces off the selected edges. \
             Connected faces extrude as regions unless individual=True.",
            |s, a| {
                select_whole_in_object_mode(&mut s.graph, K::Face);
                let mode = components::mode(&s.graph);
                if a.bool("duplicate") && mode != K::Edge {
                    return extrude_duplicate(s, a, a.bool("keepSides"));
                }
                if mode == K::Edge {
                    return edit_selected_op(s, "polyExtrudeEdge", a, false, None);
                }
                edit_selected_op(s, "polyExtrudeFace", a, false, None)
            },
        )
        .flag(flag("thickness", T::Float, json!(0.0), "Distance along the normals, cm."))
        .flag(FlagDef::new("keepFacesTogether", "kft", T::Bool, Value::Null, "Faces that touch extrude as one."))
        .flag(FlagDef::new("divisions", "d", T::Int, Value::Null, "Segments along the extrusion."))
        .flag(FlagDef::new("smoothingAngle", "sma", T::Float, Value::Null, "New edges sharper than this are hard."))
        .flag(FlagDef::new("taper", "tp", T::Float, Value::Null, "Scale at the far end."))
        .flag(FlagDef::new("twist", "twt", T::Float, Value::Null, "Turn at the far end, degrees."))
        .flag(flag(
            "offset",
            T::Float,
            json!(0.0),
            "Faces: inward distance of the extruded border, cm; negative grows them. Edges: outward distance from their \
             faces, cm.",
        ))
        .flag(flag("individual", T::Bool, json!(false), "Each face separately."))
        .flag(flag(
            "keepBack",
            T::Bool,
            json!(true),
            "Faces: where the extruded faces only border open edges, as a whole open shell, keep a copy facing back at the base.",
        ))
        .flag(flag("duplicate", T::Bool, json!(false), "Faces: leave the mesh as it is and put the extruded faces in a new mesh."))
        .flag(flag("keepSides", T::Bool, json!(false), "With duplicate, the new mesh also keeps the side faces.")),
    );

    c.register(
        Cmd::new("polyInset", "Inset the selected faces.", |s, a| {
            select_whole_in_object_mode(&mut s.graph, K::Face);
            edit_selected_op(s, "polyInset", a, false, None)
        })
        .flag(flag("thickness", T::Float, json!(5.0), "Inward distance, cm."))
        .flag(flag("depth", T::Float, json!(0.0), "Push along the normals afterwards, cm."))
        .flag(flag("individual", T::Bool, json!(false), "Each face separately."))
        .flag(flag("clamp", T::Bool, json!(true), "Shrink the thickness where the inset would cross itself, as on narrow faces.")),
    );

    c.register(
        Cmd::new("polyBevel", "Bevel the selected edges (vertex or face selections convert to their edges).", |s, a| {
            select_whole_in_object_mode(&mut s.graph, K::Edge);
            let offset = a.float("offset", 2.0);
            // Faces bevel the border of each selected region, not the edges between selected faces (the reference).
            let from_faces = components::mode(&s.graph) == K::Face;
            let mut a2 = a.clone();
            if a.get("chamfer").is_none() {
                a2.flags.insert("chamfer".into(), json!(false));
            }
            if from_faces && a.get("offset").is_none() {
                a2.flags.insert("offset".into(), json!(2.0));
            }
            // Named with no width, edges bevel by the reference's default: half way to the shortest edge's middle.
            if !from_faces && a.get("offset").is_none() && a.get("offsetAsFraction").is_none() {
                a2.flags.insert("offsetAsFraction".into(), json!(true));
                a2.flags.insert("fraction".into(), json!(0.5));
            }
            let (out, infos) = edit_selected_op_info(s, if from_faces { "polyBevelFace" } else { "polyBevel3" }, &a2, false, None)?;
            let used = infos.iter().filter_map(|i| i.as_f64()).fold(offset, f64::min);
            if a.bool("clamp") && used < offset - 1e-9 {
                crate::last_op::limit_flag(s, "offset", json!((used * 1000.0).floor() / 1000.0));
            }
            Ok(out)
        })
        .flag(FlagDef::new("offset", "o", T::Float, Value::Null, "Width, cm.").min(0.0))
        .flag(FlagDef::new("fraction", "f", T::Float, Value::Null, "Width as a fraction, with offsetAsFraction."))
        .flag(FlagDef::new("offsetAsFraction", "oaf", T::Bool, Value::Null, "Width by fraction rather than distance."))
        .flag(FlagDef::new("depth", "dp", T::Float, Value::Null, "How far the bevel rounds out; held for the node."))
        .flag(FlagDef::new("chamfer", "c", T::Bool, Value::Null, "Flat across rather than round."))
        .flag(flag("segments", T::Int, json!(1), "Segments across the bevel.").min(1.0).max(64.0))
        .flag(flag("profile", T::Float, json!(0.5), "0.5 round, 0.25 straight, toward 1 square.").min(0.0).max(1.0))
        .flag(flag("clamp", T::Bool, json!(true), "Keep widths from overlapping."))
        .flag(flag("smoothingAngle", T::Float, json!(30.0), "New edges sharper than this are hard.").min(0.0).max(180.0)),
    );

    c.register(Cmd::new("deleteComponents", "Delete the selected vertices, edges or faces (with whatever depends on them).", |s, _a| {
        let ty = match components::mode(&s.graph) {
            K::Vertex => "polyDelVertex",
            K::Edge => "polyDelEdge",
            _ => "polyDelFacet",
        };
        let mut a2 = _a.clone();
        a2.flags.insert("withFaces".into(), json!(true));
        edit_selected_op(s, ty, &a2, false, None)
    }));

    c.register(
        Cmd::new(
            "polyInsertLoop",
            "Insert edge loops across the ring of the first selected edge. Several loops are centred on fraction, spacing apart.",
            |s, a| {
                edit_selected_op(s, "polySplitRing", a, false, None)
            },
        )
        .flag(flag("fraction", T::Float, json!(0.5), "Where along the ring, 0 to 1; with several loops, where their middle goes.").min(0.0).max(1.0))
        .flag(flag("count", T::Int, json!(1), "Number of loops.").min(1.0))
        .flag(
            flag(
                "spacing",
                T::Float,
                json!(1.0),
                "Several loops: gap between neighbours, 1 even spacing across the edge, smaller packs them together.",
            )
            .min(0.0),
        ),
    );

    c.register(
        Cmd::new(
            "polySplitRings",
            "Subdivide one way: insert loops across every ring through the selected edges, or across the selected faces in \
             one direction, without the perpendicular cuts a full subdivide adds. Returns the new edges.",
            |s, a| {
                // Object mode splits whole meshes, read as every face so direction still picks the way.
                let mode = components::mode(&s.graph);
                let kind = if matches!(mode, K::Face | K::Object) { K::Face } else { K::Edge };
                edit_selected_op(s, if kind == K::Face { "polySplitRingsFace" } else { "polySplitRingsEdge" }, a, true, None)
            },
        )
        .flag(flag("count", T::Int, json!(1), "Loops per ring, evenly spaced.").min(1.0).max(32.0))
        .flag(
            flag("direction", T::String, json!("along"), "Faces: across adds lines across the selection's short way, along the long way.")
                .choices(&["across", "along"]),
        )
        .flag(flag("edgeFlow", T::Bool, json!(true), "Curve the new loops to follow the surrounding surface rather than lie flat.")),
    );

    c.register(
        Cmd::new(
            "polyOffsetLoop",
            "Insert an edge loop either side of the selected edges, parallel to them (the reference's Offset Edge Loop). Partial loops \
             end on a border, or inside the mesh by meeting at the end vertex or wrapping round it. Returns the new edges.",
            |s, a| {
                edit_selected_op(s, "polyOffsetLoop", a, false, None)
            },
        )
        .flag(flag("distance", T::Float, json!(0.25), "How far out: a fraction of each crossing edge's length, or cm with absolute.").min(0.0))
        .flag(flag("absolute", T::Bool, json!(false), "distance is in cm, square to the selected edges."))
        .flag(flag("side", T::String, json!("both"), "both, left or right of the edges' direction.").choices(&["both", "left", "right"]))
        .flag(
            flag("ends", T::String, json!("converge"), "Open ends inside the mesh: converge (meet at the end vertex) or wrap (round it).")
                .choices(&["converge", "wrap"]),
        )
        .flag(flag("deleteEdge", T::Bool, json!(false), "Dissolve the selected edges afterward, leaving the two new loops.")),
    );

    c.register(
        Cmd::new(
            "polyMerge",
            "Weld selected vertices closer than a distance (all of a mesh's points if it has no component selection). \
             Exactly two selected vertices weld whatever the distance.",
            |s, a| {
                let pair = components::mode(&s.graph) == K::Vertex;
                let mut a2 = a.clone();
                a2.flags.insert("alwaysMergeTwoVertices".into(), json!(true));
                edit_selected_op(s, if pair { "polyMergeVert" } else { "polyMergeVertAll" }, &a2, true, None)
            },
        )
        .flag(flag("distance", T::Float, json!(0.01), "Weld distance, cm.").min(0.0))
        .flag(flag("average", T::Bool, json!(false), "Move merged points to their average.")),
    );

    c.register(Cmd::new("polyMergeCenter", "Weld the selected vertices into one at their centre.", |s, _a| {
        edit_selected_op(s, "polyMergeCenter", _a, false, None)
    }));

    c.register(
        Cmd::new(
            "polyTargetWeld",
            "Target weld: merge one vertex into another, which keeps its place (or both meet halfway with center=), or an \
             edge into another, end to nearest end. Follows the mesh's symmetry; a weld across the symmetry plane meets on \
             it. Returns the welded vertices.",
            target_weld_cmd,
        )
        .flag(flag("mesh", T::String, Value::Null, "Mesh to weld; default: the selected or hilited mesh."))
        .flag(flag("source", T::Int, json!(-1), "Vertex (or edge) that moves.").min(-1.0))
        .flag(flag("target", T::Int, json!(-1), "Vertex (or edge) it merges into.").min(-1.0))
        .flag(flag("edge", T::Bool, json!(false), "source and target are edges."))
        .flag(flag("center", T::Bool, json!(false), "Meet halfway instead of at the target.")),
    );

    c.register(Cmd::new("polyCollapse", "Collapse each connected group of selected edges to its centre.", |s, _a| {
        edit_selected_op(s, "polyCollapseEdge", _a, false, None)
    }));

    c.register(Cmd::new("polyDissolve", "Remove selected edges or vertices by merging the faces around them, cleanly.", |s, _a| {
        let ty = if components::mode(&s.graph) == K::Vertex { "polyDissolveVertex" } else { "polyDissolveEdge" };
        edit_selected_op(s, ty, _a, false, None)
    }));

    register_bridge(c);

    c.register(Cmd::new("polyPoke", "Fan each selected face into triangles around its centre.", |s, _a| {
        edit_selected_op(s, "polyPoke", _a, false, None)
    }));

    c.register(Cmd::new("polySpinEdge", "Turn each selected edge one step within its two faces: -of 1 forward, -of -1 backward.", |s, _a| {
        edit_selected_op(s, "polySpinEdge", _a, false, None)
    }).flag(FlagDef::new("offset", "of", T::Int, json!(1), "1 forward, -1 backward.")));

    c.register(Cmd::new("polyFillHole", "Cap the holes bordered by the selected edges (every hole when nothing is selected).", |s, _a| {
        edit_selected_op(s, "polyCloseBorder", _a, true, None)
    }));

    c.register(
        Cmd::new(
            "polyGridFill",
            "Rebuild triangle fans and other patches as a quad grid inside a ring of quads: select the fans' centre vertices \
             or the faces. With edges selected, fills the holes they border the same way.",
            |s, a| {
                let ty = match components::mode(&s.graph) {
                    K::Vertex => "polyGridFillVertex",
                    K::Edge => "polyGridFillEdge",
                    _ => "polyGridFillFace",
                };
                edit_selected_op(s, ty, a, false, None)
            },
        )
        .flag(flag("gridSize", T::Float, json!(0.4), "The grid's size against the hole; 1 grids right to the border with no ring.").min(0.05).max(1.0))
        .flag(flag("rotate", T::Int, json!(0), "Steps the grid's corners round the border.")),
    );

    c.register(Cmd::new("polyFlip", "Reverse the winding of the selected faces (the whole mesh in object mode).", |s, _a| {
        edit_selected_op(s, "polyNormal", _a, true, None)
    }));

    c.register(Cmd::new("polyConform", "Make face winding consistent; closed parts face outward.", |s, _a| {
        edit_selected_op(s, "polyConform", _a, true, None)
    }));

    c.register(
        Cmd::new(
            "polyTriangulate",
            "Triangulate the selected faces (the whole mesh in object mode), deterministically, as Unreal will. Concave and \
             non-planar faces, collinear points and faces touching themselves split without overlaps or slivers, and no \
             diagonal doubles an edge another face already has. With symmetry on, the mirrored faces split the mirrored way. \
             A whole mesh with modifiers gets a triangulate modifier on top instead, so the result is triangulated rather \
             than the cage.",
            |s, a| {
                let split = a.string("quadSplit", "shortest");
                if split != "shortest" && split != "first" {
                    fail!("polyTriangulate: quadSplit must be shortest or first");
                }
                s.graph.open_chunk("Triangulate");
                let mut on_stack = |s: &mut Session, mesh: NodeId| -> CmdResult<bool> {
                    let stack = modifiers::stack(&s.graph, mesh);
                    let Some(&top) = stack.last() else { return Ok(false) };
                    if s.graph.node(top).unwrap().ty().name != "triangulateModifier" {
                        let path = components::display_path(&s.graph, mesh);
                        execute(s, "addModifier", &json!({"args": [path], "kind": "triangulate"}), CallSource::Script)?;
                    }
                    Ok(true)
                };
                let r = edit_selected_op(s, "polyTriangulate", a, true, Some(&mut on_stack));
                s.graph.close_chunk();
                r
            },
        )
        .flag(flag("beauty", T::Bool, json!(true), "Flip n-gon diagonals toward the largest smallest angle (what display and export use)."))
        .flag(
            flag(
                "quadSplit",
                T::String,
                json!("shortest"),
                "Quads split along the shorter diagonal (what display, export and MikkTSpace use) or always from the first corner.",
            )
            .choices(&["shortest", "first"]),
        ),
    );
}

fn target_weld_cmd(s: &mut Session, a: &Args) -> CmdResult {
    let g = &mut s.graph;
    g.evaluate();
    let n = mesh_flag(g, a, "polyTargetWeld")?;
    let base = base_plug(g, n)?;
    let mut mesh = base_mesh(g, &base);
    let edge = a.bool("edge");
    let (si, ti) = (a.int("source", -1), a.int("target", -1));
    let (source, target) = (si as Index, ti as Index);
    let valid = si >= 0
        && ti >= 0
        && if edge { mesh.edge_valid(source) && mesh.edge_valid(target) } else { mesh.point_valid(source) && mesh.point_valid(target) };
    if !valid || source == target {
        fail!(
            "polyTargetWeld: {} has no {} {si} and {ti} to weld",
            components::display_path(g, n),
            if edge { "edges" } else { "vertices" }
        );
    }
    let sym = symmetry_for(g, n, &mut mesh);
    let mut pairs = Vec::new();
    let add = |pairs: &mut Vec<WeldPair>, mesh: &Mesh, s: Index, t: Index| {
        if edge {
            pairs.extend(edge_weld_pairs(mesh, s, t));
        } else {
            pairs.push(WeldPair { source: s, target: t });
        }
    };
    add(&mut pairs, &mesh, source, target);
    let mut center = a.bool("center");
    if let Some(sym) = &sym {
        let mut added = BTreeSet::from([(source, target)]);
        for k in 1..sym.order() {
            let ms = if edge { sym.image_edge(&mesh, k, source) } else { sym.image(k, source) };
            let mt = if edge { sym.image_edge(&mesh, k, target) } else { sym.image(k, target) };
            if ms == target {
                center = true;
            } else if ms != INVALID && mt != INVALID && added.insert((ms, mt)) {
                add(&mut pairs, &mesh, ms, mt);
            }
        }
    }

    g.open_chunk("Target Weld");
    let r = (|| -> CmdResult {
        let mut before = Vec::new();
        let mut drive = 0;
        if let Some(sym) = &sym {
            before = mesh.positions().to_vec();
            let pts = if edge { components::convert(&mesh, K::Edge, &[source], K::Vertex, false) } else { vec![source] };
            drive = sym.majority_wedge(&pts);
        }
        let welded = target_weld(&mut mesh, &pairs, center);
        if welded.is_empty() {
            fail!("polyTargetWeld: nothing to weld");
        }
        if sym.is_some() {
            maintain_symmetry(g, n, &mut mesh, &before, drive);
        }
        let mut remap = MeshRemap::default();
        mesh.compact(Some(&mut remap));
        let mut sel = current_selection(g);
        remap_selection(&mut sel, n, &remap);
        let mapped = remap_ids(&welded, &remap.points);
        store_edited(g, n, base, mesh, &remap)?;
        let [p, e, f] = sel;
        components::set_all(g, p, e, f);
        Ok(json!(mapped))
    })();
    g.close_chunk();
    r
}

const PROFILE_SHAPES: [&str; 7] = ["smooth", "sphere", "root", "sharp", "linear", "inverseSquare", "constant"];

fn bridge_options(a: &Args) -> BridgeOptions {
    let mut o = BridgeOptions {
                    divisions: a.int("divisions", -1) as i32,
                    curve: match a.string("curveType", "auto").as_str() {
                        "linear" => BridgeCurve::Linear,
                        "blend" => BridgeCurve::Blend,
                        "path" => BridgeCurve::Path,
                        _ => BridgeCurve::Auto,
                    },
                    smoothness: a.float("smoothness", 1.0),
                    tension_source: a.float("tensionSource", 1.0),
                    tension_target: a.float("tensionTarget", 1.0),
                    offset: a.int("bridgeOffset", 0) as i32,
                    reverse_source: a.bool("directionSource"),
                    reverse_target: a.bool("directionTarget"),
                    twist: a.float("twist", 0.0),
                    taper: a.float("taper", 1.0),
                    bias: a.float("bias", 0.5),
                    profile_factor: a.float("profileFactor", 0.0),
                    transition: if a.string("transition", "quads") == "triangles" {
                        BridgeTransition::Triangles
                    } else {
                        BridgeTransition::Quads
                    },
                    weld_ends: a.bool("weldEnds"),
                    weld_tolerance: a.float("weldTolerance", 0.35),
                    relax: a.int("relax", 0).max(0) as i32,
                    smoothing_angle: a.float("smoothingAngle", 30.0),
                    merge: a.bool("merge"),
                    merge_factor: a.float("mergeFactor", 0.5),
                    connect: match a.string("connect", "pairs").as_str() {
                        "chain" => BridgeConnect::Chain,
                        "chainClosed" => BridgeConnect::ChainClosed,
                        _ => BridgeConnect::Pairs,
                    },
                    ..Default::default()
                };
                if let Some(list) = a.get("taperCurve").and_then(Value::as_array) {
                    o.taper_curve = list.iter().filter_map(Value::as_f64).collect();
                }
                let shape = a.string("profileShape", "smooth");
                if let Some(k) = PROFILE_SHAPES.iter().position(|&p| p == shape) {
                    o.profile_shape = [
                        BridgeProfile::Smooth,
                        BridgeProfile::Sphere,
                        BridgeProfile::Root,
                        BridgeProfile::Sharp,
                        BridgeProfile::Linear,
                        BridgeProfile::InverseSquare,
                        BridgeProfile::Constant,
                    ][k];
                }
                let (ss, st) = (a.int("startSource", -1), a.int("startTarget", -1));
                if ss >= 0 && st >= 0 {
                    o.pairs.push((ss as Index, st as Index));
                }
                if let Some(list) = a.get("pairs").and_then(Value::as_array) {
                    for pr in list {
                        if let (Some(x), Some(y)) = (pr.get(0).and_then(Value::as_u64), pr.get(1).and_then(Value::as_u64)) {
                            if pr.as_array().map_or(0, Vec::len) == 2 {
                                o.pairs.push((x as Index, y as Index));
                            }
                        }
                    }
                }
    o
}

fn run_bridge(m: &mut Mesh, sel: &Ids, sym: Option<&SymmetryMap>, o: &BridgeOptions, from_faces: bool) -> EditResult {
    let kind = if from_faces { K::Face } else { K::Edge };
    let all = with_mirror(m, sym, sel, kind);
    let r = if from_faces { bridge_faces(m, &all, o) } else { bridge(m, &all, o) };
    let mut out = faces(r.faces);
    out.info = json!({"pairsBridged": r.pairs_bridged, "unwelded": r.unwelded, "warnings": r.warnings});
    out
}

fn bridge_face_op(a: &Args, m: &mut Mesh, sel: &Ids, sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    Ok(run_bridge(m, sel, sym, &bridge_options(a), true))
}

fn bridge_edge_op(a: &Args, m: &mut Mesh, sel: &Ids, sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    Ok(run_bridge(m, sel, sym, &bridge_options(a), false))
}

fn register_bridge(c: &mut Commands<Session>) {
    c.register(
        Cmd::new(
            "polyBridge",
            "Join selected border loops or chains with faces. Several pairs bridge at once; a single border round a slot or \
             sliver bridges its two long sides; borders of different lengths pair by arc length. Selected faces are deleted \
             and the holes bridged.",
            |s, a| {
                let from_faces = components::mode(&s.graph) == K::Face;
                let (out, infos) = if a.string("curveType", "auto") == "path" {
                    // A path reads a curve from the scene, which history can't replay: this one stays destructive.
                    let mut o = bridge_options(a);
                    o.path = bridge_path(&mut s.graph, &a.string("curve", ""))?;
                    let mut infos = Vec::new();
                    let kind = if from_faces { K::Face } else { K::Edge };
                    let out = edit_selected(
                        s,
                        "Bridge",
                        kind,
                        &mut |m, sel, sym| {
                            let r = run_bridge(m, sel, sym, &o, from_faces);
                            infos.push(r.info.clone());
                            Ok(r)
                        },
                        false,
                        None,
                    )?;
                    (out, infos)
                } else {
                    edit_selected_op_info(s, if from_faces { "polyBridgeFace" } else { "polyBridgeEdge" }, a, false, None)?
                };
                let mut warnings = Vec::new();
                let (mut bridged, mut unwelded) = (0, 0);
                for i in &infos {
                    bridged += i["pairsBridged"].as_i64().unwrap_or(0);
                    unwelded += i["unwelded"].as_i64().unwrap_or(0);
                    warnings.extend(i["warnings"].as_array().cloned().unwrap_or_default());
                }
                for w in &warnings {
                    log::warn!("polyBridge: {}", w.as_str().unwrap_or(""));
                }
                if bridged == 0 {
                    log::warn!("polyBridge: nothing to bridge; select two borders, or one border round a slot");
                } else if unwelded > 0 {
                    log::warn!("polyBridge: {unwelded} end border points fell between rows and stay as corners of the end faces");
                }
                Ok(out)
            },
        )
        .flag(
            flag("divisions", T::Int, json!(-1), "Rows between the borders; -1 picks them from the gap, the faces around it and the open ends.")
                .min(-1.0)
                .max(256.0),
        )
        .flag(flag("curveType", T::String, json!("auto"), "How the rows run between the borders.").choices(&["linear", "blend", "path", "auto"]))
        .flag(flag("smoothness", T::Float, json!(1.0), "0 straightens the curve, 1 is the full curve.").min(0.0).max(1.0))
        .flag(flag("tensionSource", T::Float, json!(1.0), "Handle length leaving the source border (Blend and Auto).").min(0.0).max(4.0))
        .flag(flag("tensionTarget", T::Float, json!(1.0), "Handle length entering the target border (Blend and Auto).").min(0.0).max(4.0))
        .flag(flag("bridgeOffset", T::Int, json!(0), "Steps the pairing round closed loops."))
        .flag(flag("directionSource", T::Bool, json!(false), "Read the source border the other way round."))
        .flag(flag("directionTarget", T::Bool, json!(false), "Read the target border the other way round."))
        .flag(flag("twist", T::Float, json!(0.0), "Turns the middle rows about the bridge's axis, degrees at the target."))
        .flag(flag("taper", T::Float, json!(1.0), "Scale of the middle rows about their centre.").min(0.0).max(10.0))
        .flag(flag("bias", T::Float, json!(0.5), "Where the taper peaks, 0 at the source, 1 at the target.").min(0.0).max(1.0))
        .flag(flag("taperCurve", T::List, Value::Null, "Taper strength along the bridge, values evenly spaced from source to target."))
        .flag(
            flag("profileFactor", T::Float, json!(0.0), "Bulge (positive) or pinch (negative) of the middle rows, as a fraction of the gap.")
                .min(-2.0)
                .max(2.0),
        )
        .flag(flag("profileShape", T::String, json!("smooth"), "Falloff of the profile across the bridge.").choices(&PROFILE_SHAPES))
        .flag(flag("curve", T::String, json!(""), "The curve Path follows."))
        .flag(
            flag("transition", T::String, json!("quads"), "How borders of different lengths make up the difference.")
                .choices(&["quads", "triangles"]),
        )
        .flag(flag("weldEnds", T::Bool, json!(true), "Open bridges land their end rows on the border between the two sides' ends."))
        .flag(
            flag(
                "weldTolerance",
                T::Float,
                json!(0.35),
                "How far a row may be from an end border point and still weld, as a fraction of a row.",
            )
            .min(0.0)
            .max(1.0),
        )
        .flag(flag("relax", T::Int, json!(0), "Smoothing passes over the new points.").min(0.0).max(50.0))
        .flag(flag("smoothingAngle", T::Float, json!(30.0), "New edges sharper than this are hard.").min(0.0).max(180.0))
        .flag(flag("merge", T::Bool, json!(false), "Merge the borders into one instead of bridging."))
        .flag(flag("mergeFactor", T::Float, json!(0.5), "Where merged points go: 0 the source, 1 the target.").min(0.0).max(1.0))
        .flag(
            flag("connect", T::String, json!("pairs"), "Pair loops up, or run one bridge through them all in order.")
                .choices(&["pairs", "chain", "chainClosed"]),
        )
        .flag(flag("startSource", T::Int, json!(-1), "Source point that pairs with startTarget."))
        .flag(flag("startTarget", T::Int, json!(-1), "Target point that pairs with startSource."))
        .flag(flag("pairs", T::List, Value::Null, "More point pairs to hold: [[source, target], ...].")),
    );
}

