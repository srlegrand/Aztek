//! MeshInfo's statistics, meshCheck, findComponents, selectComponentsWhere
//! and componentInfo.

use aztek_geo::topology::{boundary_loops, face_components};
use aztek_geo::{Index, Mesh, INVALID};
use aztek_ops::{execute, fail, objects, ArgType, Args, CallSource, CmdResult, CommandDef, Commands, FlagDef};
use glam::{DMat3, DMat4, DVec3};
use serde_json::{json, Value};

use crate::components::{self, ids_arg, ComponentMode, Ids, Op};
use crate::graph::Graph;
use crate::mesh_nodes::{MeshNode, SYMMETRY_RADIAL};
use crate::modifiers;
use crate::plug::NodeId;
use crate::session::Session;
use crate::transform::Transform;

type T = ArgType;
type Cmd = CommandDef<Session>;

fn component_mesh(g: &Graph, name: &str, cmd: &str) -> CmdResult<NodeId> {
    let Some(id) = g.find(name)? else { fail!("{cmd}: no node named '{name}'") };
    match crate::components::as_mesh(g, id) {
        Some(m) => Ok(m),
        None => fail!("{cmd}: {name} is not a mesh"),
    }
}

/// The mesh the command works on: the named one, else the one with selected components, else the selected one.
fn target_mesh(s: &Session, a: &Args, cmd: &str) -> CmdResult<NodeId> {
    let g = &s.graph;
    if let Some(last) = a.positional.last().and_then(Value::as_str) {
        return component_mesh(g, last, cmd);
    }
    let mode = components::mode(g);
    if mode != ComponentMode::Object {
        if let Some(&node) = components::selected_all(g, mode).keys().find(|&&id| g.node(id).is_some()) {
            return Ok(node);
        }
    }
    let objs = objects(s, a);
    let Some(last) = objs.last() else { fail!("{cmd}: no mesh") };
    component_mesh(g, last, cmd)
}

fn kind_arg(a: &Args, cmd: &str) -> CmdResult<ComponentMode> {
    match components::mode_from_name(&a.string("kind", "face")) {
        Ok(ComponentMode::Object) => fail!("{cmd}: kind must be vertex, edge or face"),
        Ok(k) => Ok(k),
        Err(e) => fail!("{cmd}: {e}"),
    }
}

fn vec(a: &Args, flag: &str) -> DVec3 {
    DVec3::from_array(a.vec3(flag).unwrap_or([0.0; 3]))
}

fn round3(v: DVec3) -> Value {
    let r = |x: f64| (x * 1e4).round() / 1e4;
    json!([r(v.x), r(v.y), r(v.z)])
}

/// Everything a predicate looks at for one component, in object space.
struct Sample {
    id: Index,
    pos: DVec3,
    normal: DVec3,
    border: bool,
    /// Edges only: unit vector along the edge.
    direction: DVec3,
}

fn unit_or_zero(n: DVec3) -> DVec3 {
    let len = n.length();
    if len > 1e-12 {
        n / len
    } else {
        n
    }
}

fn point_normal(m: &Mesh, p: Index) -> DVec3 {
    unit_or_zero(m.point_faces(p).into_iter().map(|f| m.face_area_vector(f)).sum())
}

fn edge_normal(m: &Mesh, e: Index) -> DVec3 {
    let mut n = DVec3::ZERO;
    m.for_each_edge_corner(e, |c| n += m.face_normal(m.corner_face(c)));
    unit_or_zero(n)
}

fn samples(m: &Mesh, kind: ComponentMode) -> Vec<Sample> {
    let mut out = Vec::new();
    match kind {
        ComponentMode::Face => {
            for f in 0..m.face_capacity() {
                if !m.face_valid(f) {
                    continue;
                }
                let mut border = false;
                m.for_each_face_corner(f, |c| border |= m.is_boundary_edge(m.corner_edge(c)));
                out.push(Sample { id: f, pos: m.face_centroid(f), normal: m.face_normal(f), border, direction: DVec3::ZERO });
            }
        }
        ComponentMode::Edge => {
            for e in 0..m.edge_capacity() {
                if !m.edge_valid(e) {
                    continue;
                }
                let (a, b) = (m.position(m.edge_point(e, 0)), m.position(m.edge_point(e, 1)));
                let len = (b - a).length();
                out.push(Sample {
                    id: e,
                    pos: (a + b) * 0.5,
                    normal: edge_normal(m, e),
                    border: m.is_boundary_edge(e),
                    direction: if len > 1e-12 { (b - a) / len } else { DVec3::ZERO },
                });
            }
        }
        _ => {
            for p in 0..m.point_capacity() {
                if m.point_valid(p) {
                    out.push(Sample {
                        id: p,
                        pos: m.position(p),
                        normal: point_normal(m, p),
                        border: m.is_boundary_point(p),
                        direction: DVec3::ZERO,
                    });
                }
            }
        }
    }
    out
}

#[derive(Clone)]
struct Shell {
    faces: usize,
    /// Object space.
    lo: DVec3,
    hi: DVec3,
}

/// Connected shells, largest first, and each face's shell number in that order (INVALID for unused slots).
fn shells_of(m: &Mesh, face_shell: &mut Vec<Index>) -> Vec<Shell> {
    let count = face_components(m, face_shell, &[]) as usize;
    let mut shells = vec![Shell { faces: 0, lo: DVec3::splat(1e300), hi: DVec3::splat(-1e300) }; count];
    for f in 0..m.face_capacity() {
        if m.face_valid(f) {
            let s = &mut shells[face_shell[f as usize] as usize];
            s.faces += 1;
            m.for_each_face_corner(f, |c| {
                let p = m.position(m.corner_point(c));
                s.lo = s.lo.min(p);
                s.hi = s.hi.max(p);
            });
        }
    }
    let mut order: Vec<usize> = (0..count).collect();
    order.sort_by(|&a, &b| shells[b].faces.cmp(&shells[a].faces));
    let mut rank = vec![0; count];
    let mut sorted = Vec::new();
    for (i, &o) in order.iter().enumerate() {
        rank[o] = i as Index;
        sorted.push(shells[o].clone());
    }
    for s in face_shell.iter_mut() {
        if *s != INVALID {
            *s = rank[*s as usize];
        }
    }
    sorted
}

/// Points whose faces do not form one fan: two parts touching at a single vertex (a bowtie).
fn non_manifold_points(m: &Mesh) -> usize {
    let mut bad = 0;
    for p in 0..m.point_capacity() {
        if !m.point_valid(p) {
            continue;
        }
        let faces = m.point_faces(p);
        if faces.len() < 2 {
            continue;
        }
        let mut parent: Vec<usize> = (0..faces.len()).collect();
        fn root(parent: &mut [usize], mut i: usize) -> usize {
            while parent[i] != i {
                parent[i] = parent[parent[i]];
                i = parent[i];
            }
            i
        }
        let slot = |f: Index| faces.iter().position(|&x| x == f).unwrap_or(faces.len());
        m.for_each_point_edge(p, |e| {
            let mut first = faces.len();
            m.for_each_edge_corner(e, |c| {
                let s = slot(m.corner_face(c));
                if s == faces.len() {
                    return;
                }
                if first == faces.len() {
                    first = s;
                } else {
                    let (rs, rf) = (root(&mut parent, s), root(&mut parent, first));
                    parent[rs] = rf;
                }
            });
        });
        let fans = (0..faces.len()).filter(|&i| root(&mut parent, i) == i).count();
        bad += (fans > 1) as usize;
    }
    bad
}

/// "+x", "-y", ... as axis index and sign; "center" as sign 0 on the axis given, x by default.
fn side_arg(s: &str, cmd: &str) -> CmdResult<(usize, i32)> {
    let bad = || aztek_ops::CommandError::new(format!("{cmd}: side must look like '+x', '-z' or 'center'"));
    let axis_of = |c: u8| if (b'x'..=b'z').contains(&c) { Ok((c - b'x') as usize) } else { Err(bad()) };
    let b = s.as_bytes();
    if matches!(s, "center" | "centre" | "middle") {
        return Ok((0, 0));
    }
    if b.len() == 2 && (b[0] == b'+' || b[0] == b'-') {
        return Ok((axis_of(b[1])?, if b[0] == b'+' { 1 } else { -1 }));
    }
    if b.len() == 1 {
        return Ok((axis_of(b[0])?, 1));
    }
    if b.len() == 7 && s.starts_with("center") {
        return Ok((axis_of(b[6])?, 0));
    }
    Err(bad())
}

fn query_flags() -> Vec<FlagDef> {
    let f = |name: &str, short: &str, ty, def: Value, doc: &str| FlagDef::new(name, short, ty, def, doc);
    vec![
        f("kind", "k", T::String, json!("face"), "vertex, edge or face.").choices(&["vertex", "edge", "face"]),
        f("normal", "n", T::Vec3, Value::Null, "Keep components whose normal is within angle of this direction (world space)."),
        f("angle", "", T::Float, json!(30.0), "Tolerance for normal=, degrees.").min(0.0).max(180.0),
        f(
            "side",
            "s",
            T::String,
            Value::Null,
            "'+x', '-x', '+y', ... keeps components whose centre is on that side of the object-space origin plane; 'center' \
             (or 'centerx', 'centery', 'centerz') keeps those on the plane.",
        ),
        f(
            "box",
            "b",
            T::List,
            Value::Null,
            "[[minX, minY, minZ], [maxX, maxY, maxZ]], world space: keep components whose centre is inside. None leaves that \
             side open, so [[None, 20, None], [None, None, None]] is everything above y=20.",
        ),
        f(
            "direction",
            "dir",
            T::Vec3,
            Value::Null,
            "Edges only: keep edges running along this world direction (either way) within angle; (0, 1, 0) finds vertical \
             edges, whose ring polyInsertLoop cuts with a horizontal loop.",
        ),
        f("near", "", T::Vec3, Value::Null, "World point; with radius= keeps components within it, else keeps the count nearest."),
        f("radius", "r", T::Float, Value::Null, "Distance for near=, cm.").min(0.0),
        f(
            "extreme",
            "x",
            T::String,
            Value::Null,
            "'+y', '-z', ...: keep the count components furthest along that world direction (the top face is extreme='+y').",
        ),
        f("count", "c", T::Int, json!(1), "How many to keep for extreme= or near= without radius.").min(1.0),
        f("border", "", T::Bool, Value::Null, "True keeps only components on an open border, False only interior ones."),
        f(
            "shell",
            "",
            T::Int,
            Value::Null,
            "Keep components of one connected shell: 0 is the largest, numbered as meshCheck lists them.",
        )
        .min(0.0),
        f("fromSelection", "fs", T::Bool, json!(false), "Only consider the currently selected components of that kind."),
        f(
            "tolerance",
            "t",
            T::Float,
            Value::Null,
            "Distance counted as on a plane for side=, cm (default scales with the mesh).",
        )
        .min(0.0),
    ]
}

fn normal_matrix(w: &DMat4) -> DMat3 {
    DMat3::from_mat4(*w).inverse().transpose()
}

/// Ids of one mesh's components passing every predicate given.
fn query(g: &mut Graph, n: NodeId, a: &Args, cmd: &str) -> CmdResult<Ids> {
    let kind = kind_arg(a, cmd)?;
    g.evaluate();
    let Some(mesh) = modifiers::cage(g, n) else { return Ok(Vec::new()) };
    let m = &*mesh;
    let world = g.node(n).unwrap().matrix(Transform::WORLD_MATRIX);
    let normal_world = normal_matrix(&world);
    let to_world = |p: DVec3| world.transform_point3(p);
    let dir_world = |d: DVec3| unit_or_zero(normal_world * d);

    let mut all = samples(m, kind);
    if a.bool("fromSelection") {
        let sel = components::selected(g, n, kind);
        all.retain(|s| sel.binary_search(&s.id).is_ok());
    }
    if a.has("normal") {
        let want = vec(a, "normal");
        if want.length() < 1e-12 {
            fail!("{cmd}: normal= must not be zero");
        }
        let want = want.normalize();
        let cos_max = a.float("angle", 30.0).clamp(0.0, 180.0).to_radians().cos();
        all.retain(|s| dir_world(s.normal).dot(want) >= cos_max - 1e-9);
    }
    if a.has("side") {
        let (axis, sign) = side_arg(&a.string("side", ""), cmd)?;
        let tol = if a.has("tolerance") { a.float("tolerance", 0.0) } else { (m.bounds().size().length() * 1e-4).max(1e-3) };
        all.retain(|s| {
            let v = s.pos[axis];
            !(if sign == 0 {
                v.abs() > tol
            } else if sign > 0 {
                v <= tol
            } else {
                v >= -tol
            })
        });
    }
    if a.has("direction") {
        if kind != ComponentMode::Edge {
            fail!("{cmd}: direction= is for kind='edge'");
        }
        let want = vec(a, "direction");
        if want.length() < 1e-12 {
            fail!("{cmd}: direction= must not be zero");
        }
        let want = want.normalize();
        let cos_max = a.float("angle", 30.0).clamp(0.0, 90.0).to_radians().cos();
        all.retain(|s| {
            let d = world.transform_vector3(s.direction);
            let len = d.length();
            !(len < 1e-12 || (d / len).dot(want).abs() < cos_max - 1e-9)
        });
    }
    if a.has("box") {
        let b = a.get("box").unwrap();
        let ok = b.as_array().is_some_and(|b| {
            b.len() == 2 && b.iter().all(|c| c.as_array().is_some_and(|c| c.len() == 3))
        });
        if !ok {
            fail!("{cmd}: box= is [[minX, minY, minZ], [maxX, maxY, maxZ]]");
        }
        let bound = |v: &Value, open: f64| v.as_f64().unwrap_or(open);
        let mut lo = DVec3::ZERO;
        let mut hi = DVec3::ZERO;
        for i in 0..3 {
            lo[i] = bound(&b[0][i], -1e300);
            hi[i] = bound(&b[1][i], 1e300);
        }
        all.retain(|s| {
            let p = to_world(s.pos);
            !(p.cmplt(lo).any() || p.cmpgt(hi).any())
        });
    }
    if a.has("border") {
        let want = a.bool("border");
        all.retain(|s| s.border == want);
    }
    if a.has("shell") {
        let mut face_shell = Vec::new();
        shells_of(m, &mut face_shell);
        let want = a.int("shell", 0) as Index;
        let shell_of = |id: Index| {
            let mut f = INVALID;
            match kind {
                ComponentMode::Face => f = id,
                ComponentMode::Edge => m.for_each_edge_corner(id, |c| f = m.corner_face(c)),
                _ => {
                    if let Some(&first) = m.point_faces(id).first() {
                        f = first;
                    }
                }
            }
            if f == INVALID {
                INVALID
            } else {
                face_shell[f as usize]
            }
        };
        all.retain(|s| shell_of(s.id) == want);
    }
    let count = a.int("count", 1).max(1) as usize;
    if a.has("near") {
        let at = vec(a, "near");
        if a.has("radius") {
            let r = a.float("radius", 0.0);
            all.retain(|s| (to_world(s.pos) - at).length() <= r);
        } else {
            all.sort_by(|x, y| (to_world(x.pos) - at).length().total_cmp(&(to_world(y.pos) - at).length()));
            all.truncate(count);
        }
    }
    if a.has("extreme") {
        let (axis, sign) = side_arg(&a.string("extreme", ""), cmd)?;
        if sign == 0 {
            fail!("{cmd}: extreme= is a direction like '+y'");
        }
        let sign = sign as f64;
        all.sort_by(|x, y| (to_world(y.pos)[axis] * sign).total_cmp(&(to_world(x.pos)[axis] * sign)));
        all.truncate(count);
    }
    let mut out: Ids = all.iter().map(|s| s.id).collect();
    out.sort_unstable();
    Ok(out)
}

fn face_info(m: &Mesh, f: Index, w: &DMat4, nw: &DMat3) -> Value {
    let (mut pts, mut edges, mut nbrs) = (Vec::new(), Vec::new(), Vec::new());
    m.for_each_face_corner(f, |c| {
        pts.push(m.corner_point(c));
        edges.push(m.corner_edge(c));
        let o = m.opposite_face(m.corner_edge(c), f);
        if o != INVALID {
            nbrs.push(o);
        }
    });
    let n = *nw * m.face_normal(f);
    json!({
        "id": f,
        "center": round3(w.transform_point3(m.face_centroid(f))),
        "normal": round3(unit_or_zero(n)),
        "area": (m.face_area(f) * 1e3).round() / 1e3,
        "points": pts,
        "edges": edges,
        "neighbors": nbrs,
    })
}

fn edge_info(m: &Mesh, e: Index, w: &DMat4) -> Value {
    let mut faces = Vec::new();
    m.for_each_edge_corner(e, |c| faces.push(m.corner_face(c)));
    let (a, b) = (m.position(m.edge_point(e, 0)), m.position(m.edge_point(e, 1)));
    json!({
        "id": e,
        "points": [m.edge_point(e, 0), m.edge_point(e, 1)],
        "center": round3(w.transform_point3((a + b) * 0.5)),
        "length": (m.edge_length(e) * 1e3).round() / 1e3,
        "faces": faces,
        "border": m.is_boundary_edge(e),
    })
}

fn point_info(m: &Mesh, p: Index, w: &DMat4, nw: &DMat3) -> Value {
    let (mut edges, mut nbrs) = (Vec::new(), Vec::new());
    m.for_each_point_edge(p, |e| {
        edges.push(e);
        nbrs.push(m.edge_other_point(e, p));
    });
    let n = *nw * point_normal(m, p);
    json!({
        "id": p,
        "position": round3(w.transform_point3(m.position(p))),
        "normal": round3(unit_or_zero(n)),
        "edges": edges,
        "neighbors": nbrs,
        "faces": m.point_faces(p),
    })
}

fn face_counts(m: &Mesh) -> (usize, usize, usize, usize) {
    let (mut tris, mut tri_faces, mut quads, mut ngons) = (0, 0, 0, 0);
    for f in 0..m.face_capacity() {
        if m.face_valid(f) {
            let k = m.face_size(f) as usize;
            tris += k - 2;
            match k {
                3 => tri_faces += 1,
                4 => quads += 1,
                _ => ngons += 1,
            }
        }
    }
    (tris, tri_faces, quads, ngons)
}

fn edge_counts(m: &Mesh) -> (usize, usize) {
    let (mut border, mut non_manifold) = (0, 0);
    for e in 0..m.edge_capacity() {
        if m.edge_valid(e) {
            let c = m.edge_face_count(e);
            border += (c == 1) as usize;
            non_manifold += (c > 2) as usize;
        }
    }
    (border, non_manifold)
}

fn world_corners(w: &DMat4, lo: DVec3, hi: DVec3) -> (DVec3, DVec3) {
    let mut a = DVec3::splat(1e300);
    let mut b = DVec3::splat(-1e300);
    for i in 0..8 {
        let c = w.transform_point3(DVec3::new(
            if i & 1 != 0 { hi.x } else { lo.x },
            if i & 2 != 0 { hi.y } else { lo.y },
            if i & 4 != 0 { hi.z } else { lo.z },
        ));
        a = a.min(c);
        b = b.max(c);
    }
    (a, b)
}

/// Counts and health of a mesh for meshInfo: topology, open borders, non-manifold edges, shells, stack and symmetry.
pub fn mesh_stats(g: &mut Graph, n: NodeId) -> Value {
    g.evaluate();
    let node = g.node(n).unwrap();
    let Some(m) = node.data().mesh(MeshNode::OUT_MESH).cloned() else {
        return json!({"points": 0, "edges": 0, "faces": 0, "triangles": 0});
    };
    let (tris, tri_faces, quads, ngons) = face_counts(&m);
    let (border, non_manifold) = edge_counts(&m);
    let mut comp = Vec::new();
    let shells = face_components(&m, &mut comp, &[]);
    let b = m.bounds();
    let w = node.matrix(Transform::WORLD_MATRIX);
    let (lo, hi) = world_corners(&w, b.min, b.max);
    let mods: Vec<String> = modifiers::stack(g, n)
        .into_iter()
        .filter_map(|id| g.node(id).map(|md| format!("{} ({})", md.name(), md.ty().name)))
        .collect();
    let axes = ["off", "x", "y", "z", "radial"];
    let sym = node.int(MeshNode::SYMMETRY).clamp(0, SYMMETRY_RADIAL) as usize;
    let smooth = node.int(MeshNode::SMOOTH_PREVIEW);
    let mut out = json!({
        "points": m.num_points(),
        "edges": m.num_edges(),
        "faces": m.num_faces(),
        "triangles": tris,
        "triFaces": tri_faces,
        "quads": quads,
        "ngons": ngons,
        "borderEdges": border,
        "boundaryLoops": boundary_loops(&m).len(),
        "nonManifoldEdges": non_manifold,
        "shells": shells,
        "closed": border == 0 && non_manifold == 0,
        "boundsMin": [b.min.x, b.min.y, b.min.z],
        "boundsMax": [b.max.x, b.max.y, b.max.z],
        "worldBoundsMin": round3(lo),
        "worldBoundsMax": round3(hi),
        "symmetry": axes[sym],
        "smoothPreview": smooth,
        "modifiers": mods,
        "materialSlots": m.material_names(),
    });
    if let Some(cage) = modifiers::cage(g, n) {
        if !std::sync::Arc::ptr_eq(&cage, &m) {
            out["cage"] = json!({"points": cage.num_points(), "edges": cage.num_edges(), "faces": cage.num_faces()});
        }
    }
    out
}

fn world_box(w: &DMat4, lo: DVec3, hi: DVec3) -> Value {
    let (a, b) = world_corners(w, lo, hi);
    let r = |v: DVec3| json!([(v.x * 100.0).round() / 100.0, (v.y * 100.0).round() / 100.0, (v.z * 100.0).round() / 100.0]);
    json!([r(a), r(b)])
}

/// Pass/fail validation of a finished model, small enough to print after every block of edits.
fn mesh_check(s: &mut Session, n: NodeId, a: &Args) -> CmdResult {
    let g = &mut s.graph;
    g.evaluate();
    let path = g.path(n);
    let node = g.node(n).unwrap();
    let Some(mesh) = node.data().mesh(MeshNode::OUT_MESH).cloned() else { fail!("meshCheck: {path} has no mesh") };
    let m = &*mesh;
    let w = node.matrix(Transform::WORLD_MATRIX);
    let symmetry_on = node.int(MeshNode::SYMMETRY) > 0;

    let (tris, tri_faces, quads, ngons) = face_counts(m);
    let scale = m.bounds().size().length();
    let degenerate =
        (0..m.face_capacity()).filter(|&f| m.face_valid(f) && m.face_area(f) < 1e-8 * scale * scale).count();
    let (border, non_manifold) = edge_counts(m);
    let bowties = non_manifold_points(m);
    let mut face_shell = Vec::new();
    let shells = shells_of(m, &mut face_shell);
    let shell_list: Vec<Value> =
        shells.iter().take(12).map(|s| json!({"faces": s.faces, "box": world_box(&w, s.lo, s.hi)})).collect();

    let expect = a.int("expectShells", 1);
    let mut issues = Vec::new();
    let mut notes = Vec::new();
    if expect > 0 && shells.len() as i64 != expect {
        issues.push(format!(
            "{} shells, expected {}: parts float separately instead of growing out of one body. Rebuild them by extruding \
             faces of the body, or join each to the body: polyCombine, delete the facing faces of both parts, select the two \
             open border loops (selectComponentsWhere(kind='edge', border=True)) and polyBridge. findComponents(shell=i) \
             selects one.",
            shells.len(),
            expect
        ));
    }
    if border > 0 {
        issues.push(format!(
            "{border} open border edges in {} holes: select them (kind='edge', border=True) and polyFillHole, or polyBridge \
             two loops.",
            boundary_loops(m).len()
        ));
    }
    if non_manifold > 0 {
        issues.push(format!(
            "{non_manifold} non-manifold edges (3+ faces on one edge), usually an extrude into existing faces or merged \
             overlapping parts: undo it, or delete the extra faces."
        ));
    }
    if bowties > 0 {
        issues.push(format!("{bowties} non-manifold vertices (parts touching at one point): polyMerge / polyBridge them properly."));
    }
    if degenerate > 0 {
        issues.push(format!("{degenerate} zero-area faces: polyMerge(distance=0.01) on the mesh."));
    }
    if a.has("maxTriangles") && tris as i64 > a.int("maxTriangles", 0) {
        issues.push(format!("{tris} triangles, over the budget of {}.", a.int("maxTriangles", 0)));
    }
    if ngons > 0 {
        notes.push(format!("{ngons} ngons (fine flat-shaded; triangulate or split them before subdividing)."));
    }

    let mut out = json!({
        "mesh": path,
        "ok": issues.is_empty(),
        "faces": m.num_faces(),
        "triangles": tris,
        "quads": quads,
        "tris": tri_faces,
        "ngons": ngons,
        "shells": shells.len(),
        "shellList": shell_list,
        "borderEdges": border,
        "nonManifoldEdges": non_manifold,
        "nonManifoldVerts": bowties,
        "box": world_box(&w, m.bounds().min, m.bounds().max),
    });
    if symmetry_on && s.commands.find("symmetryCheck").is_some() {
        if let Ok(r) = execute(s, "symmetryCheck", &json!({"args": [path]}), CallSource::Script) {
            let sc = &r[&path];
            out["symmetry"] = json!({"axis": sc["axis"], "maxAsymmetry": sc["maxAsymmetry"], "unmatched": sc["unmatched"]});
            if sc["unmatched"].as_i64().unwrap_or(0) > 0 || sc["maxAsymmetry"].as_f64().unwrap_or(0.0) > 1e-3 * scale.max(1.0) {
                notes.push("halves differ: symmetrize(mesh) if that is not intended.".into());
            }
        }
    }
    out["issues"] = json!(issues);
    if !notes.is_empty() {
        out["notes"] = json!(notes);
    }
    Ok(out)
}

pub fn register(c: &mut Commands<Session>) {
    c.register(
        Cmd::new(
            "meshCheck",
            "Validate a model in one compact report: ok (no issues), shells with their world boxes (largest first), open \
             border edges, non-manifold edges and vertices, zero-area faces, face/triangle counts, symmetry drift. issues says \
             what is wrong and how to fix it. Changes nothing.",
            |s, a| {
                let n = target_mesh(s, a, "meshCheck")?;
                mesh_check(s, n, a)
            },
        )
        .positional("[mesh]")
        .flag(FlagDef::new("expectShells", "", T::Int, json!(1), "Connected parts the model should have (0 skips the check).").min(0.0))
        .flag(FlagDef::new("maxTriangles", "", T::Int, Value::Null, "Triangle budget; more is an issue.").min(0.0))
        .undoable(false),
    );

    let mut find = Cmd::new(
        "findComponents",
        "Ids of a mesh's vertices, edges or faces matching every predicate given (normal direction, side of the symmetry \
         plane, box, distance, extreme along an axis, open border). Ids index the editable cage, like selectComponents. \
         Changes nothing; see selectComponentsWhere to select them.",
        |s, a| {
            let n = target_mesh(s, a, "findComponents")?;
            Ok(json!(query(&mut s.graph, n, a, "findComponents")?))
        },
    )
    .positional("[mesh]")
    .undoable(false);
    for f in query_flags() {
        find = find.flag(f);
    }
    c.register(find);

    let mut sel = Cmd::new(
        "selectComponentsWhere",
        "Select a mesh's components by predicates (the flags of findComponents) and switch to that component mode. With \
         symmetry on, edits still mirror; select one side. Returns the selected ids.",
        |s, a| {
            let n = target_mesh(s, a, "selectComponentsWhere")?;
            let kind = kind_arg(a, "selectComponentsWhere")?;
            let ids = query(&mut s.graph, n, a, "selectComponentsWhere")?;
            let op = if a.bool("add") {
                Op::Add
            } else if a.bool("deselect") {
                Op::Remove
            } else {
                Op::Replace
            };
            // Picking components hilites their mesh, as clicking it would.
            if !s.graph.is_selected(n) {
                let path = s.graph.path(n);
                execute(s, "select", &json!({"args": [path]}), CallSource::Script)?;
            }
            components::select(&mut s.graph, n, kind, &ids, op);
            components::set_mode(&mut s.graph, kind);
            Ok(json!(components::selected(&s.graph, n, kind)))
        },
    )
    .positional("[mesh]");
    for f in query_flags() {
        sel = sel.flag(f);
    }
    sel = sel
        .flag(FlagDef::new("add", "", T::Bool, json!(false), "Add to the selection of that kind instead of replacing it."))
        .flag(FlagDef::new("deselect", "", T::Bool, json!(false), "Remove the matches from the selection."));
    c.register(sel);

    c.register(
        Cmd::new(
            "componentInfo",
            "Positions, normals and adjacency of a mesh's components in bulk. Faces: center, normal, area, points, edges, \
             neighbors (faces across each edge). Edges: points, center, length, faces, border. Vertices: position, normal, \
             edges, neighbors (points), faces. World space unless space='object'. ids= picks some (default: the selected \
             ones of that kind, or all).",
            |s, a| {
                let n = target_mesh(s, a, "componentInfo")?;
                let kind = kind_arg(a, "componentInfo")?;
                let g = &mut s.graph;
                g.evaluate();
                let Some(mesh) = modifiers::cage(g, n) else { return Ok(json!([])) };
                let m = &*mesh;
                let w = if a.string("space", "world") == "object" {
                    DMat4::IDENTITY
                } else {
                    g.node(n).unwrap().matrix(Transform::WORLD_MATRIX)
                };
                let nw = normal_matrix(&w);
                let ids: Ids = if a.has("ids") {
                    ids_arg(a.get("ids").unwrap())?
                } else if !components::selected(g, n, kind).is_empty() {
                    components::selected(g, n, kind).to_vec()
                } else {
                    let cap = match kind {
                        ComponentMode::Face => m.face_capacity(),
                        ComponentMode::Edge => m.edge_capacity(),
                        _ => m.point_capacity(),
                    };
                    (0..cap).collect()
                };
                let mut out = Vec::new();
                for id in ids {
                    match kind {
                        ComponentMode::Face if m.face_valid(id) => out.push(face_info(m, id, &w, &nw)),
                        ComponentMode::Edge if m.edge_valid(id) => out.push(edge_info(m, id, &w)),
                        ComponentMode::Vertex if m.point_valid(id) => out.push(point_info(m, id, &w, &nw)),
                        _ => {}
                    }
                }
                if let Some(keep) = a.get("fields").and_then(Value::as_array) {
                    let keep: Vec<&str> = keep.iter().filter_map(Value::as_str).collect();
                    for c in &mut out {
                        let mut trimmed = serde_json::Map::new();
                        for k in &keep {
                            if let Some(v) = c.get(*k) {
                                trimmed.insert(k.to_string(), v.clone());
                            }
                        }
                        *c = Value::Object(trimmed);
                    }
                }
                Ok(Value::Array(out))
            },
        )
        .positional("[mesh]")
        .flag(FlagDef::new("kind", "k", T::String, json!("face"), "vertex, edge or face.").choices(&["vertex", "edge", "face"]))
        .flag(FlagDef::new("ids", "", T::List, Value::Null, "Component ids; [first, last] pairs are ranges."))
        .flag(FlagDef::new("space", "", T::String, json!("world"), "world or object.").choices(&["world", "object"]))
        .flag(FlagDef::new(
            "fields",
            "",
            T::List,
            Value::Null,
            "Keys to keep per component, e.g. ['id', 'center', 'normal'] (default all).",
        ))
        .undoable(false),
    );
}
