//! What the interactive modelling tools run: polySlideEdge (Slide Edge Tool), polyAppendVertex (Create Polygon, Append to
//! Polygon, Quad Draw), polyCrease (Crease Tool), makeLive (Make Live, the surface Quad Draw draws on).

use std::collections::{BTreeMap, HashMap, HashSet};

use aztek_geo::symmetry::SymmetryMap;
use aztek_geo::{Index, Mesh, MeshRemap};
use aztek_ops::{fail, ArgType, Args, CmdResult, CommandDef, Commands, FlagDef};
use glam::DVec3;
use serde_json::{json, Value};

use super::{base_mesh, base_plug, edges, edit_selected_op, mesh_flag, store_edited, EditResult};
use crate::components::{self, ComponentMode as K, Ids};
use crate::session::Session;
use crate::transform::Transform;

type Cmd = CommandDef<Session>;

fn f(name: &str, short: &str, ty: ArgType, doc: &str) -> FlagDef {
    FlagDef::new(name, short, ty, Value::Null, doc)
}

/// The edge a selected edge's point slides along within one of the edge's faces: the other edge at the point in that face.
fn side_edge(m: &Mesh, e: Index, face: Index, p: Index) -> Option<Index> {
    for c in m.face_corners(face) {
        let x = m.corner_edge(c);
        if x != e && (m.edge_point(x, 0) == p || m.edge_point(x, 1) == p) {
            return Some(x);
        }
    }
    None
}

/// Per point of the selected edges, its slide targets to each side: (point, toward side A, toward side B). Sides stay
/// consistent along a connected run of edges.
pub fn slide_targets(m: &Mesh, sel: &[Index]) -> Vec<(Index, Option<Index>, Option<Index>)> {
    let mut face_of: HashMap<Index, [Option<Index>; 2]> = HashMap::new();
    let set: HashSet<Index> = sel.iter().copied().collect();
    let mut seen = HashSet::new();
    // Starting from each unvisited edge: its faces in order give sides A and B; neighbours take theirs from the shared
    // slide edge.
    for &start in sel {
        if !seen.insert(start) {
            continue;
        }
        let mut faces = Vec::new();
        m.for_each_edge_corner(start, |c| faces.push(m.corner_face(c)));
        face_of.insert(start, [faces.first().copied(), faces.get(1).copied()]);
        let mut todo = vec![start];
        while let Some(e) = todo.pop() {
            let sides = face_of[&e];
            for i in 0..2 {
                let p = m.edge_point(e, i);
                let mut next = None;
                m.for_each_point_edge(p, |x| {
                    if x != e && set.contains(&x) && !seen.contains(&x) {
                        next = Some(x);
                    }
                });
                let Some(n) = next else { continue };
                seen.insert(n);
                let mut nf = Vec::new();
                m.for_each_edge_corner(n, |c| nf.push(m.corner_face(c)));
                // A neighbour's side A is the face sharing side A's slide edge at p.
                let via = sides[0].and_then(|fa| side_edge(m, e, fa, p));
                let a_first = match (via, nf.first()) {
                    (Some(v), Some(&f0)) => side_edge(m, n, f0, p) == Some(v),
                    _ => true,
                };
                let pair = if a_first { [nf.first().copied(), nf.get(1).copied()] } else { [nf.get(1).copied(), nf.first().copied()] };
                face_of.insert(n, pair);
                todo.push(n);
            }
        }
    }
    let mut out: BTreeMap<Index, (Option<Index>, Option<Index>)> = BTreeMap::new();
    for &e in sel {
        let sides = face_of[&e];
        for i in 0..2 {
            let p = m.edge_point(e, i);
            let side = |face: Option<Index>| -> Option<Index> {
                let x = side_edge(m, e, face?, p)?;
                // Sliding along another selected edge would fold the loop.
                if set.contains(&x) {
                    return None;
                }
                Some(m.edge_other_point(x, p))
            };
            let (a, b) = (side(sides[0]), side(sides[1]));
            let entry = out.entry(p).or_default();
            entry.0 = entry.0.or(a);
            entry.1 = entry.1.or(b);
        }
    }
    out.into_iter().map(|(p, (a, b))| (p, a, b)).collect()
}

fn slide_op(a: &Args, m: &mut Mesh, sel: &Ids, _sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    let v = a.float("value", 0.0).clamp(-1.0, 1.0);
    let targets = slide_targets(m, sel);
    let before = m.positions().to_vec();
    for (p, side_a, side_b) in targets {
        let to = if v >= 0.0 { side_a } else { side_b };
        let Some(t) = to else { continue };
        let at = before[p as usize].lerp(before[t as usize], v.abs());
        m.set_position(p, at);
    }
    Ok(edges(sel.clone()))
}

#[derive(Clone, Debug)]
pub enum FaceItem {
    Vertex(Index),
    /// World.
    Position(DVec3),
}

/// Builds a face from existing vertices and new positions on a mesh, winding it to match any neighbour sharing an edge.
fn append_face(m: &mut Mesh, items: &[FaceItem], to_local: &glam::DMat4) -> Option<Index> {
    let mut pts = Vec::with_capacity(items.len());
    for it in items {
        pts.push(match it {
            FaceItem::Vertex(v) => {
                if !m.point_valid(*v) {
                    return None;
                }
                *v
            }
            FaceItem::Position(p) => m.add_point(to_local.transform_point3(*p)),
        });
    }
    pts.dedup();
    if pts.first() == pts.last() && pts.len() > 1 {
        pts.pop();
    }
    if pts.len() < 3 {
        return None;
    }
    // An existing edge already running a -> b means this face must run b -> a.
    let mut flip = false;
    for i in 0..pts.len() {
        let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
        let e = m.find_edge(a, b);
        if e == aztek_geo::INVALID {
            continue;
        }
        let mut same = false;
        m.for_each_edge_corner(e, |c| {
            if m.corner_point(c) == a && m.corner_next_point(c) == b {
                same = true;
            }
        });
        if same {
            flip = true;
            break;
        }
    }
    if flip {
        pts.reverse();
    }
    let f = m.add_face(&pts);
    (f != aztek_geo::INVALID).then_some(f)
}

fn parse_items(a: &Args) -> CmdResult<Vec<FaceItem>> {
    let mut items = Vec::new();
    if let Some(list) = a.get("items").and_then(Value::as_array) {
        for it in list {
            if let Some(v) = it.get("vertex").and_then(Value::as_u64) {
                items.push(FaceItem::Vertex(v as Index));
            } else if let Some(p) = it.get("position") {
                items.push(FaceItem::Position(super::vec(p)));
            } else {
                fail!("polyAppendVertex: each item is {{vertex:}} or {{position:}}");
            }
        }
    }
    // The reference's -a and -p in order: all vertices, then all points, when given apart.
    if let Some(v) = a.get("append") {
        let list: Vec<i64> = match v {
            Value::Array(x) => x.iter().filter_map(Value::as_i64).collect(),
            x => x.as_i64().into_iter().collect(),
        };
        items.extend(list.into_iter().map(|i| FaceItem::Vertex(i as Index)));
    }
    if let Some(Value::Array(x)) = a.get("point") {
        let mut nums = Vec::new();
        fn walk(v: &Value, out: &mut Vec<f64>) {
            match v {
                Value::Array(a) => a.iter().for_each(|x| walk(x, out)),
                y => out.extend(y.as_f64()),
            }
        }
        x.iter().for_each(|v| walk(v, &mut nums));
        items.extend(nums.chunks(3).filter(|c| c.len() == 3).map(|c| FaceItem::Position(DVec3::new(c[0], c[1], c[2]))));
    }
    Ok(items)
}

pub fn register(c: &mut Commands<Session>) {
    crate::history::register_op(crate::history::HistoryOp {
        node_type: "polySlideEdge",
        command: "polySlideEdge",
        label: "Slide Edge",
        kind: K::Edge,
        run: slide_op,
        skip_flags: &[],
        doc: "Slides edges along the edges beside them.",
    });
    c.register(
        Cmd::new(
            "polySlideEdge",
            "Slide the selected edges within the faces beside them: -v -1 to 1, a fraction of the way along the neighbouring edges to \
             one side or the other.",
            |s, a| edit_selected_op(s, "polySlideEdge", a, false, None),
        )
        .flag(f("value", "v", ArgType::Float, "How far: -1 to 1."))
        .flag(f("absolute", "a", ArgType::Bool, "Accepted for existing scripts."))
        .flag(f("direction", "d", ArgType::Int, "Accepted for existing scripts."))
        .flag(f("edgeFlow", "ef", ArgType::Bool, "Accepted for existing scripts."))
        .flag(f("symmetry", "sym", ArgType::Bool, "Accepted for existing scripts.")),
    );

    c.register(
        Cmd::new(
            "polyAppendVertex",
            "Add a face to a mesh from existing vertices and new points: -a vertex (repeat), -p x y z (repeat), or -items [{vertex:}, \
             {position:}] in order. With no mesh named or selected and only points, makes a new mesh. Returns the new face and the mesh.",
            |s, a| {
                let items = parse_items(a)?;
                if items.len() < 3 {
                    fail!("polyAppendVertex: a face needs three points");
                }
                let g = &mut s.graph;
                g.evaluate();
                let target = mesh_flag(g, a, "polyAppendVertex").ok();
                let (n, fresh) = match target {
                    Some(n) => (n, false),
                    None => {
                        if items.iter().any(|i| matches!(i, FaceItem::Vertex(_))) {
                            fail!("polyAppendVertex: name a mesh to use its vertices");
                        }
                        let (_tr, sh) = crate::mesh_nodes::create_mesh_node(g, "polySurface1")?;
                        (sh, true)
                    }
                };
                g.open_chunk("Append to Polygon");
                let r = (|| -> CmdResult {
                    let base = base_plug(g, n)?;
                    let mut mesh = base_mesh(g, &base);
                    let world = g.node(n).unwrap().matrix(Transform::WORLD_MATRIX);
                    // With symmetry the face's image is made too, wound the other way.
                    let spec = if fresh { None } else { crate::mesh_nodes::symmetry_spec(g, n) };
                    let sym = spec.as_ref().filter(|s| !s.is_radial()).and_then(|_| crate::mesh_edit::symmetry_for(g, n, &mut mesh));
                    let to_local = world.inverse();
                    let image: Option<Vec<FaceItem>> = match (&spec, &sym) {
                        (Some(sp), Some(map)) => {
                            let mut out = Vec::new();
                            for i in items.iter().rev() {
                                out.push(match i {
                                    FaceItem::Vertex(v) => FaceItem::Vertex(map.image(1, *v)),
                                    FaceItem::Position(p) => FaceItem::Position(world.transform_point3(sp.apply(1, to_local.transform_point3(*p)))),
                                });
                            }
                            let lies_on_plane = items.iter().zip(out.iter().rev()).all(|(a, b)| match (a, b) {
                                (FaceItem::Vertex(x), FaceItem::Vertex(y)) => x == y,
                                (FaceItem::Position(x), FaceItem::Position(y)) => (*x - *y).length() < 1e-9,
                                _ => false,
                            });
                            let usable = out.iter().all(|i| !matches!(i, FaceItem::Vertex(v) if *v == aztek_geo::INVALID));
                            (usable && !lies_on_plane).then_some(out)
                        }
                        _ => None,
                    };
                    let Some(face) = append_face(&mut mesh, &items, &world.inverse()) else { fail!("polyAppendVertex: these points don't make a face") };
                    if let Some(other) = &image {
                        let _ = append_face(&mut mesh, other, &to_local);
                    }
                    store_edited(g, n, base, mesh, &MeshRemap::default())?;
                    let path = components::display_path(g, n);
                    if fresh {
                        let tr = crate::dag::transform_of(g, n);
                        g.select(&[tr], crate::graph::SelectMode::Replace);
                    }
                    Ok(json!([path, format!("f[{face}]")]))
                })();
                g.close_chunk();
                r
            },
        )
        .flag(f("mesh", "", ArgType::String, "The mesh; the selected one by default."))
        .flag(f("append", "a", ArgType::Any, "An existing vertex. Repeat it.").multi(1))
        .flag(f("point", "p", ArgType::List, "A new point: x y z. Repeat it.").multi(3))
        .flag(f("items", "", ArgType::List, "In order: {vertex: id} or {position: [x, y, z]}."))
        .flag(f("constructionHistory", "ch", ArgType::Bool, "Accepted for existing scripts.")),
    );

    c.register(
        Cmd::new(
            "polyCrease",
            "Set the crease of the selected edges (-v 0 to 10): it holds them sharper under smoothing. -q -v asks.",
            |s, a| {
                let g = &mut s.graph;
                let names = a.named();
                if !names.is_empty() {
                    components::select_strings(g, &names, crate::graph::SelectMode::Replace).map_err(aztek_ops::CommandError::new)?;
                    components::set_mode(g, K::Edge);
                }
                let mode = components::mode(g);
                let sel = components::selected_all(g, mode).clone();
                if sel.is_empty() {
                    fail!("polyCrease: select or name edges");
                }
                let mut out = Vec::new();
                g.open_chunk("Crease");
                let r = (|| -> CmdResult<()> {
                    for (n, ids) in sel {
                        let Some(cage) = crate::modifiers::cage(g, n) else { continue };
                        let list = components::convert(&cage, mode, &ids, K::Edge, false);
                        if a.bool("query") {
                            let cr = cage.edge_attrs().get::<f32>(aztek_geo::attribute::names::CREASE);
                            out.extend(list.iter().map(|&e| json!(cr.get(e as usize).copied().unwrap_or(0.0) as f64)));
                            continue;
                        }
                        let v = a.float("value", 0.0).clamp(0.0, 10.0) as f32;
                        let tweak = crate::history::tweak_node(g, n)?;
                        let _ = tweak;
                        let base = base_plug(g, n)?;
                        let mut mesh = base_mesh(g, &base);
                        mesh.edge_attrs_mut().add(aztek_geo::attribute::names::CREASE, aztek_geo::attribute::AttrType::Float);
                        let cr = mesh.edge_attrs_mut().get_mut::<f32>(aztek_geo::attribute::names::CREASE);
                        for &e in &list {
                            if let Some(x) = cr.get_mut(e as usize) {
                                *x = v;
                            }
                        }
                        store_edited(g, n, base, mesh, &MeshRemap::default())?;
                    }
                    Ok(())
                })();
                g.close_chunk();
                r?;
                Ok(if a.bool("query") { Value::Array(out) } else { Value::Null })
            },
        )
        .positional("edges (the selection when none)")
        .flag(f("value", "v", ArgType::Float, "Crease value: 0 to 10."))
        .flag(f("relativeValue", "rv", ArgType::Float, "Accepted for existing scripts."))
        .flag(f("query", "q", ArgType::Bool, "Query mode."))
        .flag(f("queried", "", ArgType::List, "Flags asked about in query mode.")),
    );

    c.register(
        Cmd::new(
            "makeLive",
            "Make a mesh the live surface: Quad Draw and the snap to live draw on it. -none clears it. -q asks.",
            |s, a| {
                if a.bool("query") {
                    return Ok(json!(s.live_surface.iter().map(|&n| s.graph.path(n)).collect::<Vec<_>>()));
                }
                if a.bool("none") {
                    s.live_surface.clear();
                    return Ok(Value::Null);
                }
                let names = if a.named().is_empty() { s.graph.selection().iter().map(|&i| s.graph.path(i)).collect() } else { a.named() };
                let mut list = Vec::new();
                for n in names {
                    let Some(id) = s.graph.find(&n)? else { fail!("makeLive: no object named {n}") };
                    let Some(m) = components::as_mesh(&s.graph, id) else { fail!("makeLive: {n} is not a mesh") };
                    list.push(m);
                }
                s.live_surface = list;
                Ok(Value::Null)
            },
        )
        .undoable(false)
        .positional("meshes (the selection when none)")
        .flag(f("none", "n", ArgType::Bool, "Clear the live surface."))
        .flag(f("query", "q", ArgType::Bool, "Query mode.")),
    );
}
