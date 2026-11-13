//! Reduce, quadrangulate, subdivide, mirror, separate, combine, extract,
//! soft edges, duplicate faces and connect.

use std::sync::Arc;

use aztek_geo::normals::{set_hard_edges_by_angle, set_soft_edges, update_corner_normals, CornerNormalOptions};
use aztek_geo::ops::*;
use aztek_geo::reduce::{reduce_mesh, ReduceOptions};
use aztek_geo::symmetry::{remove_symmetry_layers, SymmetryMap, SymmetryPlane};
use aztek_geo::{names, Index, Mesh};
use aztek_ops::{execute, fail, ArgType, CallSource, CmdResult, CommandDef, Commands, FlagDef};
use glam::{DMat4, DVec3};
use serde_json::{json, Map, Value};

use super::component_ops::mesh_beside;
use super::*;
use crate::components::{self, ComponentMode as K, Ids};
use crate::graph::SelectMode;
use crate::isolation;
use crate::mesh_nodes::{mesh_objects, symmetry_offset, symmetry_spec, MeshNode};
use crate::modifiers;

use crate::plug::{PlugRef, PlugValue};
use crate::session::Session;
use crate::transform::Transform;

type T = ArgType;
type Cmd = CommandDef<Session>;

fn flag(name: &str, ty: ArgType, default: Value, doc: &str) -> FlagDef {
    FlagDef::new(name, "", ty, default, doc)
}

fn set_mesh(g: &mut crate::graph::Graph, p: PlugRef, m: Mesh) -> CmdResult<()> {
    g.set_value(p, PlugValue::Mesh(Some(Arc::new(m))))?;
    Ok(())
}

fn soft_edge_op(a: &Args, m: &mut Mesh, sel: &Ids, sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    let angle = a.float("angle", std::f64::consts::PI).to_degrees();
    let edges = with_mirror(m, sym, sel, K::Edge);
    if angle >= 180.0 {
        set_soft_edges(m, &edges);
    } else if angle <= 0.0 {
        let valid: Vec<bool> = (0..m.edge_capacity()).map(|e| m.edge_valid(e)).collect();
        let hard = m.edge_attrs_mut().get_or_add::<bool>(names::HARD);
        if edges.is_empty() {
            for (e, v) in valid.iter().enumerate() {
                if *v {
                    hard[e] = true;
                }
            }
        }
        for &e in &edges {
            if valid.get(e as usize).copied().unwrap_or(false) {
                hard[e as usize] = true;
            }
        }
    } else {
        set_hard_edges_by_angle(m, angle, &edges);
    }
    // Stored split normals would hide the change.
    if m.corner_attrs().has(names::NORMAL) {
        update_corner_normals(m, &CornerNormalOptions::default());
    }
    Ok(EditResult::default())
}

fn duplicate_faces_op(_a: &Args, m: &mut Mesh, sel: &Ids, sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    let mut part = extract_faces(m, &with_mirror(m, sym, sel, K::Face));
    part.compact(None);
    let first = m.face_capacity();
    m.append(&part);
    Ok(faces((first..m.face_capacity()).filter(|&f| m.face_valid(f)).collect()))
}

fn connect_with(m: &mut Mesh, sel: &Ids, sym: Option<&SymmetryMap>, kind: K) -> CmdResult<EditResult> {
    let all = with_mirror(m, sym, sel, kind);
    let pts: Ids = if kind == K::Edge {
        let mut pts = Vec::new();
        for e in all {
            if m.edge_valid(e) {
                pts.push(m.split_edge(e, 0.5));
            }
        }
        pts
    } else {
        all
    };
    Ok(edges(connect_points(m, &pts)))
}

fn connect_vertex_op(_a: &Args, m: &mut Mesh, sel: &Ids, sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    connect_with(m, sel, sym, K::Vertex)
}

fn connect_edge_op(_a: &Args, m: &mut Mesh, sel: &Ids, sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    connect_with(m, sel, sym, K::Edge)
}

fn register_history_ops() {
    use crate::history::{register_op, HistoryOp};
    let op = |node_type: &'static str, command: &'static str, label: &'static str, kind: K, run: crate::history::OpFn, doc: &'static str| HistoryOp {
        node_type,
        command,
        label,
        kind,
        run,
        skip_flags: &[],
        doc,
    };
    register_op(op("polySoftEdge", "polySoftEdge", "Soften/Harden Edges", K::Edge, soft_edge_op, "Softens or hardens edges by angle."));
    register_op(op("polyDuplicateFaces", "polyDuplicateFaces", "Duplicate Faces", K::Face, duplicate_faces_op, "Copies faces into a new shell."));
    register_op(op("polyConnectVertex", "polyConnect", "Connect", K::Vertex, connect_vertex_op, "Connects vertices with edges."));
    register_op(op("polyConnectEdge", "polyConnect", "Connect", K::Edge, connect_edge_op, "Connects edge midpoints across faces."));
}

pub fn register(c: &mut Commands<Session>) {
    register_history_ops();
    c.register(
        Cmd::new(
            "polyReduce",
            "Reduce the polygon count of whole meshes while keeping their shape, silhouette, UVs and normals. The result is \
             triangles (or quads with quads=True). A mesh with modifiers gets a decimate modifier on top instead. Honours \
             the mesh's symmetry: one half (or one radial wedge) is reduced and mirrored (or repeated).",
            |s, a| {
                let names = mesh_objects(s, a, "polyReduce")?;
                let mut nodes = Vec::new();
                for name in &names {
                    nodes.push(s.graph.find(name)?.and_then(|id| components::as_mesh(&s.graph, id)).unwrap());
                }
                let sym = a.string("symmetry", "auto");
                if !["auto", "off", "x", "y", "z"].contains(&sym.as_str()) {
                    fail!("polyReduce: symmetry must be auto, off, x, y or z");
                }
                let mut o = ReduceOptions {
                    ratio: a.float("ratio", 0.5).clamp(0.0, 1.0),
                    target_triangles: a.int("targetTriangles", 0).max(0) as Index,
                    keep_borders: a.bool("keepBorders"),
                    keep_hard_edges: a.bool("keepHardEdges"),
                    quads: a.bool("quads"),
                    ..Default::default()
                };
                s.graph.open_chunk("Reduce");
                let r = (|| -> CmdResult {
                    let mut out = Map::new();
                    for id in nodes {
                        let path = components::display_path(&s.graph, id);
                        let axis = match sym.as_str() {
                            "auto" => symmetry_axis(&s.graph, id),
                            "off" => -1,
                            x => (x.as_bytes()[0] - b'x') as i32,
                        };
                        if !modifiers::stack(&s.graph, id).is_empty() {
                            let params = json!({
                                "ratio": o.ratio,
                                "targetTriangles": o.target_triangles,
                                "keepBorders": o.keep_borders,
                                "keepHardEdges": o.keep_hard_edges,
                                "symmetry": axis + 1,
                                "quads": o.quads,
                            });
                            let r = execute(s, "addModifier", &json!({"args": [path], "kind": "decimate", "params": params}), CallSource::Script)?;
                            out.insert(path, r);
                            continue;
                        }
                        let g = &mut s.graph;
                        let base = base_plug(g, id)?;
                        let mut mesh = base_mesh(g, &base);
                        let spec = symmetry_spec(g, id);
                        if let Some(radial) = spec.as_ref().and_then(|sp| sp.radial.clone()).filter(|_| sym == "auto") {
                            // Radial: reduce one wedge with its seams held, then repeat it round the axis.
                            let tol = symmetry_tolerance(&mesh);
                            cut_to_wedge(&mut mesh, &radial, 0, tol);
                            mesh.compact(None);
                            let wo = ReduceOptions {
                                keep_borders: true,
                                symmetry_axis: -1,
                                target_triangles: o.target_triangles / radial.order() as Index,
                                ..o.clone()
                            };
                            let mut r = reduce_mesh(&mesh, &wo);
                            symmetrize_radial_topology(&mut r.mesh, &radial, 0, tol);
                            out.insert(path, json!(r.mesh.num_faces()));
                            set_mesh(g, base, r.mesh)?;
                            continue;
                        }
                        o.symmetry_axis = axis;
                        o.symmetry_offset = symmetry_offset(g, id, axis);
                        o.symmetry_tolerance = symmetry_tolerance(&mesh);
                        let mut r = reduce_mesh(&mesh, &o);
                        if r.symmetry_ignored {
                            log::warn!("polyReduce {path}: faces straddle the symmetry plane, so it was reduced without symmetry");
                        }
                        if axis >= 0 && axis == symmetry_axis(g, id) {
                            remove_symmetry_layers(&mut r.mesh);
                            let tol = symmetry_tolerance(&r.mesh);
                            SymmetryMap::persistent_axis(&mut r.mesh, axis, tol, None, symmetry_offset(g, id, axis));
                        }
                        out.insert(path, json!(r.mesh.num_faces()));
                        set_mesh(g, base, r.mesh)?;
                    }
                    components::clear(&mut s.graph);
                    Ok(Value::Object(out))
                })();
                s.graph.close_chunk();
                r
            },
        )
        .positional("objects")
        .flag(flag("ratio", T::Float, json!(0.5), "Fraction of triangles to keep, 0 to 1.").min(0.0).max(1.0))
        .flag(flag("targetTriangles", T::Int, json!(0), "Triangles to keep; above 0 it overrides ratio.").min(0.0))
        .flag(flag("keepBorders", T::Bool, json!(true), "Open borders stay exactly where they are."))
        .flag(flag(
            "keepHardEdges",
            T::Bool,
            json!(true),
            "Hard edges stay intact, as UV seams and material borders always do; off lets them collapse for a lower count.",
        ))
        .flag(
            flag("symmetry", T::String, json!("auto"), "auto (the mesh's symmetry setting), off, x, y or z.")
                .choices(&["auto", "off", "x", "y", "z"]),
        )
        .flag(flag("quads", T::Bool, json!(false), "Join triangle pairs back into quads where they make good ones.")),
    );

    c.register(
        Cmd::new("polyQuadrangulate", "Join neighbouring triangles into quads.", |s, a| {
            let mut a2 = a.clone();
            a2.flags.insert("angle".into(), json!(a.float("maxAngle", 40_f64.to_radians())));
            edit_selected_op(s, "polyQuad", &a2, true, None)
        })
        .flag(flag("maxAngle", T::Float, json!(40.0), "Largest angle between the two triangles, degrees.").min(0.0).max(180.0)),
    );

    c.register(
        Cmd::new("polySubdivide", "Catmull-Clark subdivide whole meshes (crease-aware).", |s, a| {
            let names = mesh_objects(s, a, "polySubdivide")?;
            let g = &mut s.graph;
            g.open_chunk("Subdivide");
            let r = (|| -> CmdResult {
                for name in names {
                    let n = mesh_arg(g, &name, "polySubdivide")?;
                    let base = base_plug(g, n)?;
                    let o = SubdivideOptions {
                        levels: a.int("levels", 1).clamp(1, 5) as i32,
                        keep_hard_edges: a.bool("keepHardEdges"),
                        ..Default::default()
                    };
                    let m = subdivide(&base_mesh(g, &base), &o, None);
                    set_mesh(g, base, m)?;
                }
                components::clear(g);
                Ok(Value::Null)
            })();
            g.close_chunk();
            r
        })
        .positional("objects")
        .flag(flag("levels", T::Int, json!(1), "Subdivision levels.").min(1.0).max(5.0))
        .flag(flag("keepHardEdges", T::Bool, json!(false), "Keep normals split along hard edges; off smooths the shading across them.")),
    );

    c.register(
        Cmd::new(
            "polyMirror",
            "Mirror whole meshes across the plane through the object's or the world's origin facing an axis, welding along \
             the seam. direction positive keeps the + side and rebuilds the - side from it, cutting faces that cross the \
             plane; negative does the reverse; both adds a mirrored copy of the whole mesh. flip=True reflects the mesh \
             across the plane instead, keeping nothing of the original.",
            |s, a| {
                let axis = a.string("axis", "x");
                if axis.len() != 1 || !(b'x'..=b'z').contains(&axis.as_bytes()[0]) {
                    fail!("polyMirror: axis must be x, y or z");
                }
                let ax = (axis.as_bytes()[0] - b'x') as usize;
                let direction = a.string("direction", "positive");
                if !["positive", "negative", "both"].contains(&direction.as_str()) {
                    fail!("polyMirror: direction must be positive, negative or both");
                }
                let space = a.string("space", "object");
                if space != "object" && space != "world" {
                    fail!("polyMirror: space must be object or world");
                }
                let names = mesh_objects(s, a, "polyMirror")?;
                let g = &mut s.graph;
                if space == "world" {
                    g.evaluate();
                }
                g.open_chunk("Mirror");
                let r = (|| -> CmdResult {
                    for name in names {
                        let n = mesh_arg(g, &name, "polyMirror")?;
                        let base = base_plug(g, n)?;
                        let mut m = base_mesh(g, &base);
                        let world = g.node(n).unwrap().matrix(Transform::WORLD_MATRIX);
                        if a.bool("flip") {
                            let mut reflect = DMat4::IDENTITY;
                            reflect.col_mut(ax)[ax] = -1.0;
                            if space == "world" {
                                reflect = world.inverse() * reflect * world;
                            }
                            for p in 0..m.point_capacity() {
                                if m.point_valid(p) {
                                    m.set_position(p, reflect.transform_point3(m.position(p)));
                                }
                            }
                            for f in 0..m.face_capacity() {
                                if m.face_valid(f) {
                                    m.flip_face(f);
                                }
                            }
                            set_mesh(g, base, m)?;
                            continue;
                        }
                        let mut normal = DVec3::ZERO;
                        normal[ax] = 1.0;
                        let o = MirrorOptions {
                            plane: SymmetryPlane { normal, offset: 0.0 },
                            direction: match direction.as_str() {
                                "positive" => MirrorDirection::Positive,
                                "negative" => MirrorDirection::Negative,
                                _ => MirrorDirection::Both,
                            },
                            space: if space == "world" { world } else { DMat4::IDENTITY },
                            weld: a.bool("weld"),
                            tolerance: a.float("tolerance", 0.001).max(0.0),
                        };
                        mirror_mesh(&mut m, &o);
                        // The result is exactly symmetric, so its pairs are re-found by position on the next edit.
                        remove_symmetry_layers(&mut m);
                        m.compact(None);
                        set_mesh(g, base, m)?;
                    }
                    Ok(Value::Null)
                })();
                g.close_chunk();
                r
            },
        )
        .positional("objects")
        .flag(flag("axis", T::Any, json!("x"), "x, y or z; or 0, 1 or 2 with the reference's other flags."))
        .flag(
            flag("direction", T::String, json!("positive"), "positive (+ to -), negative (- to +) or both.")
                .choices(&["positive", "negative", "both"]),
        )
        .flag(flag("space", T::String, json!("object"), "object or world: whose origin and axes the plane uses.").choices(&["object", "world"]))
        .flag(flag("weld", T::Bool, json!(true), "Weld points on the plane."))
        .flag(flag("tolerance", T::Float, json!(0.001), "Points this close to the plane are cut onto it and welded, cm.").min(0.0))
        .flag(flag(
            "flip",
            T::Bool,
            json!(false),
            "Reflect the mesh across the plane rather than adding a mirrored copy: the original is not kept.",
        )),
    );

    c.register(
        Cmd::new("polySeparate", "Split meshes into one object per connected part. Returns the new names.", |s, a| {
            let names = mesh_objects(s, a, "polySeparate")?;
            let g = &mut s.graph;
            let mut out = Vec::new();
            let mut made = Vec::new();
            for name in names {
                let n = mesh_arg(g, &name, "polySeparate")?;
                g.evaluate();
                let Some(m) = g.node(n).unwrap().data().mesh(MeshNode::OUT_MESH).cloned() else { continue };
                let parts = separate(&m);
                if parts.len() < 2 {
                    continue;
                }
                let first = made.len();
                for part in parts {
                    let piece = mesh_beside(g, n, part)?;
                    out.push(json!(components::display_path(g, piece)));
                    made.push(piece);
                }
                isolation::inherit(g, &made[first..].to_vec(), &[n]);
                let gone = crate::dag::transform_of(g, n);
                g.delete_node(gone)?;
            }
            g.select(&made, SelectMode::Replace);
            Ok(Value::Array(out))
        })
        .positional("objects"),
    );

    c.register(
        Cmd::new(
            "polyCombine",
            "Merge meshes into one object, world space kept, as the reference's Mesh > Combine: a live polyUnite node fed by the \
             meshes, which stay as hidden shapes under their transforms (with -ch 0 the result is plain and the meshes go). \
             Returns the new object; polyUnite returns it with its node.",
            |s, a| {
                let names = mesh_objects(s, a, "polyCombine")?;
                if names.len() < 2 {
                    fail!("polyCombine: select at least two meshes, or a group holding them");
                }
                let mut shapes = Vec::new();
                for name in &names {
                    shapes.push(mesh_arg(&s.graph, name, "polyCombine")?);
                }
                let made = crate::mesh_edit::ref_poly::mesh_cmds::multi_input(s, a, "polyUnite", &shapes, 1, "polySurface1")?;
                Ok(made[0].clone())
            },
        )
        .positional("objects")
        .flag(flag("name", T::String, Value::Null, "Name of the combined mesh.")),
    );

    c.register(Cmd::new(
        "polyExtract",
        "Move the selected faces out into objects of their own, as the reference's Edit Mesh > Extract: a polyChipOff node, \
         then polySeparate. Returns the new objects.",
        |s, _a| chip_and_separate(s, "polyExtract", false),
    ));

    c.register(
        Cmd::new(
            "polySoftEdge",
            "Make the selected edges soft or hard by the angle their faces meet at: sharper than angle turns hard, the rest \
             soft. 180 softens every edge, 0 hardens every edge. In object mode, whole selected meshes.",
            |s, a| {
                edit_selected_op(s, "polySoftEdge", a, true, None)
            },
        )
        .flag(flag("angle", T::Float, json!(180.0), "Edges whose faces meet sharper than this turn hard, degrees.").min(0.0).max(180.0)),
    );

    c.register(Cmd::new(
        "polyDuplicateFaces",
        "Copy the selected faces into objects of their own, as the reference's Edit Mesh > Duplicate on faces: a polyChipOff \
         node with duplicate on, then polySeparate. Returns the new objects.",
        |s, _a| chip_and_separate(s, "polyDuplicateFaces", true),
    ));

    c.register(Cmd::new(
        "polyConnect",
        "Connect selected vertices that share a face with new edges, or the midpoints of selected edges across their faces.",
        |s, _a| {
            let ty = if components::mode(&s.graph) == K::Vertex { "polyConnectVertex" } else { "polyConnectEdge" };
            edit_selected_op(s, ty, _a, false, None)
        },
    ));

}

/// The reference's Extract and Duplicate on faces: polyChipOff (keeping faces together) on each mesh's selected faces,
/// then polySeparate on that mesh. Returns the objects polySeparate made.
fn chip_and_separate(s: &mut Session, cmd: &str, duplicate: bool) -> CmdResult {
    let g = &mut s.graph;
    g.evaluate();
    let from = components::mode(g);
    if from == K::Object {
        fail!("{cmd}: no faces selected");
    }
    let mut targets: Vec<(String, Vec<String>)> = Vec::new();
    for (node, ids) in components::selected_all(g, from).clone() {
        if !components::is_mesh(g, node) {
            continue;
        }
        let Some(cage) = modifiers::cage(g, node) else { continue };
        let f = components::convert(&cage, from, &ids, K::Face, false);
        if f.is_empty() {
            continue;
        }
        let path = components::display_path(g, node);
        let list = crate::history::format_components(K::Face, &f).split_whitespace().map(|x| format!("{path}.{x}")).collect();
        targets.push((path, list));
    }
    if targets.is_empty() {
        fail!("{cmd}: no faces selected");
    }
    let mut out = Vec::new();
    for (path, faces) in targets {
        s.run("polyChipOff", json!({"args": faces, "duplicate": duplicate, "keepFacesTogether": true}))?;
        let made = s.run("polySeparate", json!({"args": [path]}))?;
        for v in made.as_array().into_iter().flatten() {
            if let Some(n) = v.as_str() {
                if s.graph.find(n)?.is_some_and(|id| s.graph.node(id).is_some_and(|x| x.ty().name == "transform")) {
                    out.push(json!(n));
                }
            }
        }
    }
    let g = &mut s.graph;
    components::set_mode(g, K::Object);
    let ids: Vec<NodeId> = out.iter().filter_map(|v| v.as_str()).filter_map(|n| g.find(n).ok().flatten()).collect();
    g.select(&ids, SelectMode::Replace);
    Ok(Value::Array(out))
}
