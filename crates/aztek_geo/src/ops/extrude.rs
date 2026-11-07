//! Extrude, inset and edge extrude.

use std::collections::HashMap;

use glam::{DVec2, DVec3};

use super::basic::extract_faces;
use super::common::*;
use crate::attribute::{names, AttrType, Index, INVALID};
use crate::mesh::Mesh;
use crate::symmetry::{apply_symmetry_snap, SymmetryMap};
use crate::topology::unique_sorted;

#[derive(Clone, Copy, Debug)]
pub struct ExtrudeOptions<'a> {
    /// Distance along the averaged face normals.
    pub offset: f64,
    /// Each face separately instead of as connected regions.
    pub individual: bool,
    pub symmetry: Option<&'a SymmetryMap>,
    /// Inward distance of the extruded cap's border in its plane (the reference's offset); negative grows it.
    pub inset: f64,
    /// A region bordered only by open edges, like a whole open shell, keeps a reversed copy of its faces at the base.
    pub keep_back: bool,
}

impl Default for ExtrudeOptions<'_> {
    fn default() -> Self {
        Self { offset: 0.0, individual: false, symmetry: None, inset: 0.0, keep_back: true }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ExtrudeResult {
    /// The extruded faces, same ids as the input.
    pub faces: Vec<Index>,
    /// New side faces.
    pub sides: Vec<Index>,
    /// New faces closing the base of open regions.
    pub back: Vec<Index>,
}

#[derive(Clone, Debug, Default)]
pub struct ExtrudeCopy {
    pub mesh: Mesh,
    /// The extruded faces in mesh.
    pub cap: Vec<Index>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct EdgeExtrudeOptions<'a> {
    /// Distance along the normals of the faces each edge borders.
    pub thickness: f64,
    /// Distance outward from those faces, in their plane.
    pub offset: f64,
    pub symmetry: Option<&'a SymmetryMap>,
}

#[derive(Clone, Debug, Default)]
pub struct EdgeExtrudeResult {
    /// The new outer edges.
    pub edges: Vec<Index>,
    /// The new strip faces.
    pub faces: Vec<Index>,
}

#[derive(Clone, Copy, Debug)]
pub struct InsetOptions<'a> {
    /// Inward distance in the face plane.
    pub thickness: f64,
    /// Distance along the normals after the inset.
    pub depth: f64,
    pub individual: bool,
    pub symmetry: Option<&'a SymmetryMap>,
    /// Shrink the thickness where it would carry the inner border across itself. On a face narrower than twice the
    /// thickness the inner border closes on the medial line: its points weld and the inner face goes.
    pub clamp: bool,
}

impl Default for InsetOptions<'_> {
    fn default() -> Self {
        Self { thickness: 10.0, depth: 0.0, individual: false, symmetry: None, clamp: true }
    }
}

#[derive(Clone, Copy, Default)]
struct RegionMove {
    /// Along normals, every region point.
    offset: f64,
    /// Inward, boundary points only.
    thickness: f64,
    /// Shrink the thickness so no face's border crosses itself.
    clamp: bool,
    /// Close regions bordered only by open edges with their faces reversed at the base.
    keep_back: bool,
}

/// Per face (by position in faces): whether its connected region's border is all open mesh edges.
fn open_regions(mesh: &Mesh, faces: &[Index]) -> Vec<bool> {
    let slot = |f: Index| faces.binary_search(&f).ok();
    let mut region = vec![usize::MAX; faces.len()];
    let mut open = Vec::new();
    let mut stack = Vec::new();
    for seed in 0..faces.len() {
        if region[seed] != usize::MAX {
            continue;
        }
        let id = open.len();
        open.push(true);
        region[seed] = id;
        stack.clear();
        stack.push(seed);
        while let Some(i) = stack.pop() {
            for c in mesh.face_corners(faces[i]) {
                let mut corners = 0;
                let mut selected = 0;
                mesh.for_each_edge_corner(mesh.corner_edge(c), |r| {
                    corners += 1;
                    if let Some(j) = slot(mesh.corner_face(r)) {
                        selected += 1;
                        if region[j] == usize::MAX {
                            region[j] = id;
                            stack.push(j);
                        }
                    }
                });
                if selected == 1 && corners > 1 {
                    open[id] = false;
                }
            }
        }
    }
    region.iter().map(|&r| open[r]).collect()
}

struct Clamp {
    /// Of the thickness.
    share: f64,
    /// Some border edge shrinks to nothing at that share.
    collapses: bool,
}

/// The share of the thickness, at most 1, at which the border points moving by dirs (per unit thickness) leave every
/// face simple. An edge whose ends would pass each other stops where they meet, as the sides of a face narrower than
/// twice the thickness meeting on its medial line; points that would cross inside a face stop short of it.
fn clamp_thickness(mesh: &Mesh, faces: &[Index], dirs: &HashMap<Index, DVec3>, thickness: f64) -> Clamp {
    let dir_of = |p: Index| dirs.get(&p).map_or(DVec3::ZERO, |d| *d * thickness);
    let mut out = Clamp { share: 1.0, collapses: false };
    for &f in faces {
        mesh.for_each_face_corner(f, |c| {
            let a = mesh.position(mesh.corner_point(c));
            let b = mesh.position(mesh.corner_next_point(c));
            let len = a.distance(b);
            if len <= 0.0 {
                return;
            }
            let k = (dir_of(mesh.corner_point(c)) - dir_of(mesh.corner_next_point(c))).dot((b - a) / len);
            if k > 1e-12 && len / k < out.share {
                out.share = len / k;
                out.collapses = true;
            }
        });
    }
    let tol = WELD_SHARE * thickness.abs();
    // Collapsed edges count once, and a face that collapses to a line or a point is simple.
    let simple_at = |f: Index, at: f64| {
        let n = mesh.face_normal(f);
        let ax = n.cross(if n.x.abs() < 0.9 { DVec3::X } else { DVec3::Y }).normalize();
        let ay = n.cross(ax);
        let mut poly: Vec<DVec2> = Vec::new();
        mesh.for_each_face_corner(f, |c| {
            let q = mesh.position(mesh.corner_point(c)) + dir_of(mesh.corner_point(c)) * at;
            let uv = DVec2::new(q.dot(ax), q.dot(ay));
            if poly.last().is_none_or(|l| l.distance(uv) > tol) {
                poly.push(uv);
            }
        });
        while poly.len() > 1 && poly.last().unwrap().distance(poly[0]) <= tol {
            poly.pop();
        }
        poly.len() < 3 || polygon_is_simple(&poly)
    };
    let crossing: Vec<Index> = faces
        .iter()
        .copied()
        .filter(|&f| mesh.face_normal(f).length() > 0.5 && !simple_at(f, out.share) && simple_at(f, out.share * 1e-6))
        .collect();
    if crossing.is_empty() {
        return out;
    }
    let (mut lo, mut hi) = (0.0, out.share);
    for _ in 0..40 {
        let mid = 0.5 * (lo + hi);
        if crossing.iter().all(|&f| simple_at(f, mid)) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Clamp { share: lo * (1.0 - CLAMP_MARGIN), collapses: false }
}

/// Welds the border points a clamped inset brought together along the edges of the region's faces. Faces left with
/// under three points go, and result follows the faces that were rebuilt.
fn weld_collapsed(mesh: &mut Mesh, result: &mut ExtrudeResult, normal_sum: &mut HashMap<Index, DVec3>, tol: f64) {
    let mut target = vec![INVALID; mesh.point_capacity() as usize];
    let find = |target: &[Index], mut p: Index| {
        while target[p as usize] != INVALID {
            p = target[p as usize];
        }
        p
    };
    let mut any = false;
    for &f in &result.faces {
        for c in mesh.face_corners(f) {
            let a = find(&target, mesh.corner_point(c));
            let b = find(&target, mesh.corner_next_point(c));
            if a != b && mesh.position(a).distance(mesh.position(b)) <= tol {
                target[a.max(b) as usize] = a.min(b);
                any = true;
            }
        }
    }
    if !any {
        return;
    }
    for p in 0..target.len() as Index {
        if target[p as usize] != INVALID {
            let root = find(&target, p);
            target[p as usize] = root;
            if let Some(v) = normal_sum.remove(&p) {
                *normal_sum.entry(root).or_insert(DVec3::ZERO) += v;
            }
        }
    }
    let mut rebuilt = HashMap::new();
    weld_points_rebuilt(mesh, &target, Some(&mut rebuilt));
    let remap = |ids: &mut Vec<Index>| {
        for f in ids.iter_mut() {
            if let Some(&n) = rebuilt.get(f) {
                *f = n;
            }
        }
        ids.retain(|&f| f != INVALID);
    };
    remap(&mut result.faces);
    remap(&mut result.sides);
}

/// A cap corner on the region border, with its face's corner triangle before the thickness moved it.
struct CapCorner {
    c: Index,
    prev: Index,
    next: Index,
    o: DVec3,
    a: DVec3,
    b: DVec3,
}

/// Uvs of the moved border corners extrapolated from their corner triangles, so the cap shrinks in uv space as it
/// does in the face and the sides between get the uv band it gave up. The sides' top corners follow the cap.
fn inset_cap_uvs(mesh: &mut Mesh, caps: &[CapCorner], sides: &[Index], first_dup: Index) {
    // Barycentric weights of each cap corner's moved position in its corner triangle.
    let weights: Vec<Option<(f32, f32)>> = caps
        .iter()
        .map(|cc| {
            let d = mesh.position(mesh.corner_point(cc.c)) - cc.o;
            let aa = cc.a.dot(cc.a);
            let ab = cc.a.dot(cc.b);
            let bb = cc.b.dot(cc.b);
            let det = aa * bb - ab * ab;
            if det.abs() <= 1e-12 * aa * bb {
                return None;
            }
            let al = (bb * cc.a.dot(d) - ab * cc.b.dot(d)) / det;
            let be = (aa * cc.b.dot(d) - ab * cc.a.dot(d)) / det;
            Some((al as f32, be as f32))
        })
        .collect();
    // Side corners copying from cap corners: (side corner, source corner).
    let mut copies = Vec::new();
    for &sf in sides {
        for sc in mesh.face_corners(sf) {
            let r = mesh.corner_radial(sc);
            if r == sc || mesh.corner_point(sc) < first_dup || mesh.corner_next_point(sc) < first_dup {
                continue;
            }
            copies.push((sc, mesh.corner_next(r)));
            copies.push((mesh.corner_next(sc), r));
        }
    }
    let ca = mesh.corner_attrs_mut();
    for l in 0..ca.layer_count() {
        if ca.layer(l).ty() != AttrType::Float2 {
            continue;
        }
        let uv = ca.layer_mut(l).values_mut::<[f32; 2]>();
        let moved: Vec<[f32; 2]> = caps
            .iter()
            .zip(&weights)
            .map(|(cc, w)| match w {
                None => uv[cc.c as usize],
                Some((al, be)) => {
                    let (o, p, n) = (uv[cc.c as usize], uv[cc.prev as usize], uv[cc.next as usize]);
                    [o[0] + al * (p[0] - o[0]) + be * (n[0] - o[0]), o[1] + al * (p[1] - o[1]) + be * (n[1] - o[1])]
                }
            })
            .collect();
        for (cc, m) in caps.iter().zip(moved) {
            uv[cc.c as usize] = m;
        }
        for &(dst, src) in &copies {
            uv[dst as usize] = uv[src as usize];
        }
    }
}

struct BoundaryCorner {
    corner: Index,
    a: Index,
    b: Index,
    edge: Index,
}

fn extrude_region(mesh: &mut Mesh, faces: &[Index], mv: &RegionMove) -> ExtrudeResult {
    let mut result = ExtrudeResult::default();
    // faces is sorted; a lookup keeps per-face runs from allocating a mask the size of the mesh.
    let is_in = |f: Index| faces.binary_search(&f).is_ok();
    let face_n: HashMap<Index, DVec3> = faces.iter().map(|&f| (f, mesh.face_normal(f))).collect();

    // Boundary corners: corners of selected faces whose edge has no other selected face.
    let mut boundary = Vec::new();
    let mut old_edge = Vec::new();
    let mut sel_corners = Vec::new();
    for &f in faces {
        for c in mesh.face_corners(f) {
            sel_corners.push(c);
            old_edge.push(mesh.corner_edge(c));
            let mut selected = 0;
            mesh.for_each_edge_corner(mesh.corner_edge(c), |r| selected += is_in(mesh.corner_face(r)) as i32);
            if selected == 1 {
                boundary.push(BoundaryCorner {
                    corner: c,
                    a: mesh.corner_point(c),
                    b: mesh.corner_next_point(c),
                    edge: mesh.corner_edge(c),
                });
            }
        }
    }

    let mut dup: HashMap<Index, Index> = HashMap::new();
    let first_dup = mesh.point_capacity();
    for bc in &boundary {
        for p in [bc.a, bc.b] {
            if let std::collections::hash_map::Entry::Vacant(v) = dup.entry(p) {
                let np = mesh.add_point(mesh.position(p));
                mesh.point_attrs_mut().copy(np as usize, p as usize);
                v.insert(np);
            }
        }
    }
    let mapped = |p: Index| dup.get(&p).copied().unwrap_or(p);

    let backed = if mv.keep_back { open_regions(mesh, faces) } else { vec![false; faces.len()] };
    let mut back_points: Vec<Vec<Index>> = vec![Vec::new(); faces.len()];
    for i in 0..faces.len() {
        if backed[i] {
            back_points[i] = mesh.face_points(faces[i]);
        }
    }

    let edge_cap = mesh.edge_capacity();
    for &f in faces {
        let pts: Vec<Index> = mesh.face_points(f).into_iter().map(mapped).collect();
        mesh.set_face_points(f, &pts);
    }

    // The border points stay at the base with the sides; interior points get a copy there, since the cap takes them.
    let mut base_point: HashMap<Index, Index> = HashMap::new();
    for i in 0..faces.len() {
        if !backed[i] {
            continue;
        }
        let mut src = mesh.face_corners(faces[i]);
        for p in back_points[i].iter_mut() {
            if dup.contains_key(p) {
                continue;
            }
            let bp = match base_point.get(p) {
                Some(&b) => b,
                None => {
                    let b = mesh.add_point(mesh.position(*p));
                    mesh.point_attrs_mut().copy(b as usize, *p as usize);
                    base_point.insert(*p, b);
                    b
                }
            };
            *p = bp;
        }
        back_points[i].reverse();
        src.reverse();
        let bf = mesh.add_face_from(&back_points[i], faces[i], &src);
        if bf != INVALID {
            result.back.push(bf);
        }
    }
    for (i, &c) in sel_corners.iter().enumerate() {
        let e = mesh.corner_edge(c);
        if e >= edge_cap {
            mesh.edge_attrs_mut().copy(e as usize, old_edge[i] as usize);
        }
    }

    // Side edges going up from the base inherit hardness from the region border at their foot.
    let mut foot_hard: HashMap<Index, bool> = HashMap::new();
    {
        let hard = mesh.edge_attrs().get::<bool>(names::HARD);
        if !hard.is_empty() {
            for bc in &boundary {
                if hard[bc.edge as usize] {
                    foot_hard.insert(bc.a, true);
                    foot_hard.insert(bc.b, true);
                }
            }
        }
    }

    for bc in &boundary {
        let c = bc.corner;
        let n = mesh.corner_next(c);
        let quad = [bc.a, bc.b, dup[&bc.b], dup[&bc.a]];
        let src = [c, n, n, c];
        let sf = mesh.add_face_from(&quad, mesh.corner_face(c), &src);
        if sf != INVALID {
            result.sides.push(sf);
        }
    }
    // The sides share the cap's uvs, so a seam on the region border stays on the base edge only.
    if mesh.edge_attrs().has(names::SEAM) {
        let tops: Vec<Index> = boundary
            .iter()
            .map(|bc| mesh.find_edge(dup[&bc.a], dup[&bc.b]))
            .filter(|&e| e != INVALID && e >= edge_cap)
            .collect();
        let seam = mesh.edge_attrs_mut().get_mut::<bool>(names::SEAM);
        if !seam.is_empty() {
            for e in tops {
                seam[e as usize] = false;
            }
        }
    }
    if !foot_hard.is_empty() {
        let set: Vec<(Index, bool)> = foot_hard
            .iter()
            .map(|(&p, &h)| (mesh.find_edge(p, dup[&p]), h))
            .filter(|&(e, _)| e != INVALID)
            .collect();
        let hard = mesh.edge_attrs_mut().get_mut::<bool>(names::HARD);
        for (e, h) in set {
            hard[e as usize] = h;
        }
    }

    // Region points in their final ids, with the normals of the selected faces around them.
    let mut normal_sum: HashMap<Index, DVec3> = HashMap::new();
    for &f in faces {
        for p in mesh.face_points(f) {
            *normal_sum.entry(p).or_insert(DVec3::ZERO) += face_n[&f];
        }
    }

    let mut cap_corners = Vec::new();
    if mv.thickness != 0.0 {
        for &f in faces {
            for c in mesh.face_corners(f) {
                if mesh.corner_point(c) < first_dup {
                    continue;
                }
                let pc = mesh.corner_prev(c);
                let nc = mesh.corner_next(c);
                let o = mesh.position(mesh.corner_point(c));
                cap_corners.push(CapCorner {
                    c,
                    prev: pc,
                    next: nc,
                    o,
                    a: mesh.position(mesh.corner_point(pc)) - o,
                    b: mesh.position(mesh.corner_point(nc)) - o,
                });
            }
        }
    }

    let mut clamp = Clamp { share: 1.0, collapses: false };
    if mv.thickness != 0.0 {
        let mut perp: HashMap<Index, (DVec3, i32)> = HashMap::new();
        for bc in &boundary {
            let d = mesh.position(bc.b) - mesh.position(bc.a);
            let inward = face_n[&mesh.corner_face(bc.corner)].cross(d).normalize();
            for p in [dup[&bc.a], dup[&bc.b]] {
                let s = perp.entry(p).or_insert((DVec3::ZERO, 0));
                s.0 += inward;
                s.1 += 1;
            }
        }
        let mut dirs: HashMap<Index, DVec3> = HashMap::new();
        for (&p, s) in &perp {
            let len = s.0.length();
            if len < 1e-12 {
                continue;
            }
            let m = s.0 / len;
            // Miter: for two edges the offset along m is thickness over the cosine to each edge normal.
            let cos_half = if s.1 == 2 { (len * 0.5).max(0.1) } else { 1.0 };
            dirs.insert(p, m / cos_half);
        }
        if mv.clamp {
            clamp = clamp_thickness(mesh, faces, &dirs, mv.thickness);
        }
        for (&p, d) in &dirs {
            let v = mesh.position(p) + *d * (mv.thickness * clamp.share);
            mesh.set_position(p, v);
        }
    }
    if !cap_corners.is_empty() {
        inset_cap_uvs(mesh, &cap_corners, &result.sides, first_dup);
    }
    result.faces = faces.to_vec();
    if clamp.collapses {
        weld_collapsed(mesh, &mut result, &mut normal_sum, WELD_SHARE * mv.thickness.abs());
    }
    if mv.offset != 0.0 {
        for (&p, sum) in &normal_sum {
            let len = sum.length();
            if len > 1e-12 {
                let v = mesh.position(p) + *sum / len * mv.offset;
                mesh.set_position(p, v);
            }
        }
    }

    mesh.remove_loose(&old_edge, &[]);
    result
}

fn run_regions(mesh: &mut Mesh, faces: &[Index], individual: bool, symmetry: Option<&SymmetryMap>, mv: &RegionMove) -> ExtrudeResult {
    let mut sel = match symmetry {
        Some(s) => s.expand_faces(mesh, faces),
        None => unique_sorted(faces),
    };
    sel.retain(|&f| mesh.face_valid(f));
    let first_new = mesh.point_capacity();
    let mut out = ExtrudeResult::default();
    if individual {
        for &f in &sel {
            let r = extrude_region(mesh, &[f], mv);
            out.faces.extend(r.faces);
            out.sides.extend(r.sides);
            out.back.extend(r.back);
        }
    } else if !sel.is_empty() {
        out = extrude_region(mesh, &sel, mv);
    }
    apply_symmetry_snap(mesh, symmetry, first_new);
    out
}

pub fn extrude_faces(mesh: &mut Mesh, faces: &[Index], opts: &ExtrudeOptions) -> ExtrudeResult {
    let mv = RegionMove { offset: opts.offset, thickness: opts.inset, clamp: false, keep_back: opts.keep_back };
    run_regions(mesh, faces, opts.individual, opts.symmetry, &mv)
}

/// The faces extruded as a separate mesh, leaving the source untouched. With keep_sides it also holds the side
/// walls, open where they would have met the source.
pub fn extrude_copy(mesh: &Mesh, faces: &[Index], opts: &ExtrudeOptions, keep_sides: bool) -> ExtrudeCopy {
    let mut work = mesh.clone();
    let r = extrude_faces(&mut work, faces, opts);
    let mut keep = r.faces.clone();
    if keep_sides {
        keep.extend_from_slice(&r.sides);
        keep.extend_from_slice(&r.back);
    }
    let mut keep = unique_sorted(&keep);
    keep.retain(|&f| work.face_valid(f));
    let mut out = ExtrudeCopy { mesh: extract_faces(&work, &keep), cap: Vec::new() };
    // extract_faces adds the faces in ascending order, so a face's new id is its rank in keep.
    let cap = unique_sorted(&r.faces);
    for (i, f) in keep.iter().enumerate() {
        if cap.binary_search(f).is_ok() {
            out.cap.push(i as Index);
        }
    }
    out
}

/// Grows a strip of quads off each selected edge; edges that share a point share the new point.
pub fn extrude_edges(mesh: &mut Mesh, edges: &[Index], opts: &EdgeExtrudeOptions) -> EdgeExtrudeResult {
    let mut sel = match opts.symmetry {
        Some(s) => s.expand_edges(mesh, edges),
        None => unique_sorted(edges),
    };
    sel.retain(|&e| mesh.edge_valid(e) && mesh.edge_corner(e) != INVALID);
    let mut out = EdgeExtrudeResult::default();
    if sel.is_empty() {
        return out;
    }

    // Per point: the summed normals of the faces its selected edges border, and their summed outward directions.
    #[derive(Default)]
    struct Push {
        normal: DVec3,
        out: DVec3,
        count: i32,
    }
    let mut push: HashMap<Index, Push> = HashMap::new();
    let mut dup: HashMap<Index, Index> = HashMap::new();
    let first_new = mesh.point_capacity();
    let mut strips = Vec::new();
    for &e in &sel {
        let c = mesh.edge_corner(e);
        let a = mesh.corner_point(c);
        let b = mesh.corner_next_point(c);
        let n = mesh.face_normal(mesh.corner_face(c));
        let d = mesh.position(b) - mesh.position(a);
        let len = d.cross(n).length();
        let outward = if len > 1e-12 { d.cross(n) / len } else { DVec3::ZERO };
        for p in [a, b] {
            let s = push.entry(p).or_default();
            s.normal += n;
            s.out += outward;
            s.count += 1;
            if !dup.contains_key(&p) {
                let np = mesh.add_point(mesh.position(p));
                mesh.point_attrs_mut().copy(np as usize, p as usize);
                dup.insert(p, np);
            }
        }
        strips.push((c, a, b));
    }

    for &(corner, a, b) in &strips {
        // Wound against the face the edge borders, so the strip continues its surface.
        let n = mesh.corner_next(corner);
        let quad = [b, a, dup[&a], dup[&b]];
        let src = [n, corner, corner, n];
        let f = mesh.add_face_from(&quad, mesh.corner_face(corner), &src);
        if f != INVALID {
            out.faces.push(f);
        }
        let e = mesh.find_edge(dup[&a], dup[&b]);
        if e != INVALID {
            out.edges.push(e);
        }
    }

    for (&p, s) in &push {
        let mut mv = DVec3::ZERO;
        let nl = s.normal.length();
        if nl > 1e-12 {
            mv += s.normal / nl * opts.thickness;
        }
        let ol = s.out.length();
        if ol > 1e-12 {
            // Miter: two edges meeting at an angle push further so the strip keeps its width.
            let cos_half = if s.count == 2 { (ol * 0.5).max(0.1) } else { 1.0 };
            mv += s.out / ol * (opts.offset / cos_half);
        }
        let v = mesh.position(p) + mv;
        mesh.set_position(dup[&p], v);
    }
    apply_symmetry_snap(mesh, opts.symmetry, first_new);
    out
}

/// Returns the inner faces (the input ids, less any a clamp closed) and the new rim faces as sides.
pub fn inset_faces(mesh: &mut Mesh, faces: &[Index], opts: &InsetOptions) -> ExtrudeResult {
    let mv = RegionMove { offset: opts.depth, thickness: opts.thickness, clamp: opts.clamp, keep_back: false };
    run_regions(mesh, faces, opts.individual, opts.symmetry, &mv)
}
