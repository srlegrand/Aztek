//! Grid fill: quad grids inside a ring of quads, for holes and for patches of faces such as the fan round a pole.

use std::collections::HashMap;

use glam::DVec3;

use super::common::{mix_corners, remove_faces_clean};
use crate::attribute::{Index, INVALID};
use crate::mesh::Mesh;
use crate::symmetry::SymmetryMap;
use crate::topology::{boundary_loops, make_mask, region_boundary_edges, unique_sorted};

#[derive(Clone, Copy, Debug)]
pub struct GridFillOptions {
    /// The grid's size against the hole; the rest is a ring of quads out to the border. 1 grids right to the border.
    pub grid_size: f64,
    /// Steps the grid's corners round the border.
    pub rotate: i32,
}

impl Default for GridFillOptions {
    fn default() -> Self {
        Self { grid_size: 0.4, rotate: 0 }
    }
}

/// Weighted sources for a fill point's attributes, deduplicated so blends of blends stay small.
#[derive(Clone, Debug, Default)]
struct Blend {
    src: Vec<Index>,
    w: Vec<f32>,
}

impl Blend {
    fn add(&mut self, o: &Blend, k: f64) {
        for i in 0..o.src.len() {
            match self.src.iter().position(|&s| s == o.src[i]) {
                None => {
                    self.src.push(o.src[i]);
                    self.w.push((o.w[i] as f64 * k) as f32);
                }
                Some(at) => self.w[at] += (o.w[i] as f64 * k) as f32,
            }
        }
    }
}

#[derive(Clone, Debug)]
struct FillPoint {
    id: Index,
    pos: DVec3,
    points: Blend,
    corners: Blend,
}

impl Default for FillPoint {
    fn default() -> Self {
        Self { id: INVALID, pos: DVec3::ZERO, points: Blend::default(), corners: Blend::default() }
    }
}

fn border_point(m: &Mesh, p: Index, corner: Index) -> FillPoint {
    FillPoint {
        id: p,
        pos: m.position(p),
        points: Blend { src: vec![p], w: vec![1.0] },
        corners: if corner != INVALID { Blend { src: vec![corner], w: vec![1.0] } } else { Blend::default() },
    }
}

/// A new point weighted from others (weights summing to 1), added to the mesh.
fn blend_point(m: &mut Mesh, parts: &[(&FillPoint, f64)]) -> FillPoint {
    let mut fp = FillPoint::default();
    for &(src, k) in parts {
        fp.pos += src.pos * k;
        fp.points.add(&src.points, k);
        fp.corners.add(&src.corners, k);
    }
    fp.id = m.add_point(fp.pos);
    m.point_attrs_mut().mix(fp.id as usize, &fp.points.src, &fp.points.w);
    fp
}

struct Filler<'a> {
    mesh: &'a mut Mesh,
    src_face: Index,
    /// Winds every face the other way, for a border walked against the faces it fills.
    flip: bool,
    out: Vec<Index>,
}

impl Filler<'_> {
    fn face(&mut self, list: &[&FillPoint]) {
        let mut pts: Vec<&FillPoint> = list.to_vec();
        if self.flip {
            pts.reverse();
        }
        let ids: Vec<Index> = pts.iter().map(|p| p.id).collect();
        let f = self.mesh.add_face_from(&ids, self.src_face, &[]);
        if f == INVALID {
            return;
        }
        let mut c = self.mesh.face_corner(f);
        for p in pts {
            if !p.corners.src.is_empty() {
                mix_corners(self.mesh, c, &p.corners.src, &p.corners.w);
            }
            c = self.mesh.corner_next(c);
        }
        self.out.push(f);
    }

    /// Coons grid over an even loop of 4 or more, sides a and b with 2(a + b) = loop size.
    fn grid(&mut self, lp: &[FillPoint]) {
        let n = lp.len() as i32;
        if n == 4 {
            self.face(&[&lp[0], &lp[1], &lp[2], &lp[3]]);
            return;
        }
        let a = n / 4;
        let b = n / 2 - a;
        let (au, bu) = (a as usize, b as usize);
        let mut g = vec![vec![FillPoint::default(); bu + 1]; au + 1];
        for i in 0..=au {
            g[i][0] = lp[i].clone();
        }
        for j in 0..=bu {
            g[au][j] = lp[au + j].clone();
        }
        for i in 0..=au {
            g[i][bu] = lp[au + bu + (au - i)].clone();
        }
        for j in 0..=bu {
            g[0][j] = lp[(2 * au + bu + (bu - j)) % n as usize].clone();
        }
        for i in 1..au {
            for j in 1..bu {
                let u = i as f64 / a as f64;
                let v = j as f64 / b as f64;
                let p = blend_point(
                    self.mesh,
                    &[
                        (&g[i][0], 1.0 - v),
                        (&g[i][bu], v),
                        (&g[0][j], 1.0 - u),
                        (&g[au][j], u),
                        (&g[0][0], -(1.0 - u) * (1.0 - v)),
                        (&g[au][0], -u * (1.0 - v)),
                        (&g[0][bu], -(1.0 - u) * v),
                        (&g[au][bu], -u * v),
                    ],
                );
                g[i][j] = p;
            }
        }
        for i in 0..au {
            for j in 0..bu {
                self.face(&[&g[i][j], &g[i + 1][j], &g[i + 1][j + 1], &g[i][j + 1]]);
            }
        }
    }
}

/// The border point to start the grid from: the one whose four grid corners turn the most, so a rectangular hole
/// keeps its corners as the grid's, and a mirrored hole picks the same corners whatever point its border was walked
/// from.
fn corner_start(lp: &[FillPoint], grid_loop: i32) -> i32 {
    let n = lp.len();
    let mut turn = vec![0.0f64; n];
    for i in 0..n {
        let inn = lp[i].pos - lp[(i + n - 1) % n].pos;
        let out = lp[(i + 1) % n].pos - lp[i].pos;
        let len = inn.length() * out.length();
        if len > 0.0 {
            turn[i] = (inn.dot(out) / len).clamp(-1.0, 1.0).acos();
        }
    }
    let a = (grid_loop / 4) as usize;
    let b = (grid_loop / 2) as usize - a;
    let mut best = 0;
    let mut best_score = -1.0;
    for s in 0..n {
        let score = turn[s] + turn[(s + a) % n] + turn[(s + a + b) % n] + turn[(s + 2 * a + b) % n];
        if score > best_score + 1e-9 {
            best_score = score;
            best = s as i32;
        }
    }
    best
}

/// The border point the grid starts from, the given rotation applied.
fn fill_start(outer: &[FillPoint], o: &GridFillOptions) -> i32 {
    let n = outer.len() as i32;
    let start = if n >= 4 { corner_start(outer, if n % 2 == 0 { n } else { n + 1 }) } else { 0 };
    ((start + o.rotate) % n + n) % n
}

/// Fills a closed border round the centre point, the grid starting at outer[0]. The border runs in the winding the
/// new faces take, or against it with flip.
fn fill_loop(m: &mut Mesh, outer: &[FillPoint], centre: &FillPoint, src_face: Index, o: &GridFillOptions, flip: bool) -> Vec<Index> {
    let n = outer.len();
    if n < 3 {
        return Vec::new();
    }
    let s = o.grid_size.clamp(0.05, 1.0);
    let mut fill = Filler { mesh: m, src_face, flip, out: Vec::new() };
    if s >= 1.0 {
        if n % 2 == 0 {
            fill.grid(outer);
        } else {
            let mut ids: Vec<Index> = outer.iter().map(|p| p.id).collect();
            let mut corners: Vec<Index> = outer.iter().map(|p| p.corners.src.first().copied().unwrap_or(INVALID)).collect();
            if flip {
                ids.reverse();
                corners.reverse();
            }
            let f = fill.mesh.add_face_from(&ids, src_face, &corners);
            if f != INVALID {
                fill.out.push(f);
            }
        }
        return fill.out;
    }

    let mut inner: Vec<FillPoint> = outer.iter().map(|p| blend_point(fill.mesh, &[(centre, 1.0 - s), (p, s)])).collect();
    let quads = if n % 2 == 0 { n } else { n - 1 };
    for i in 0..quads {
        fill.face(&[&outer[i], &outer[(i + 1) % n], &inner[(i + 1) % n], &inner[i]]);
    }
    if n % 2 == 1 {
        // An odd border gets one extra inner point, so the grid's loop is even, at the cost of one triangle.
        let last = &outer[n - 1];
        let extra = blend_point(fill.mesh, &[(centre, 1.0 - s), (last, s / 2.0), (&outer[0], s / 2.0)]);
        inner.push(extra);
        fill.face(&[last, &outer[0], &inner[0], inner.last().unwrap()]);
        fill.face(&[last, inner.last().unwrap(), &inner[n - 1]]);
    }
    fill.grid(&inner);
    fill.out
}

fn average_point(pts: &[FillPoint]) -> FillPoint {
    let mut c = FillPoint::default();
    let k = 1.0 / pts.len() as f64;
    for p in pts {
        c.pos += p.pos * k;
        c.points.add(&p.points, k);
        c.corners.add(&p.corners, k);
    }
    c
}

/// Faces of sel in groups joined through shared edges.
fn patches(m: &Mesh, sel: &[Index]) -> Vec<Vec<Index>> {
    let inside = make_mask(m.face_capacity() as usize, sel);
    let mut seen = vec![false; m.face_capacity() as usize];
    let mut out = Vec::new();
    for &start in sel {
        if seen[start as usize] {
            continue;
        }
        let mut group = Vec::new();
        let mut stack = vec![start];
        seen[start as usize] = true;
        while let Some(f) = stack.pop() {
            group.push(f);
            m.for_each_face_corner(f, |c| {
                m.for_each_edge_corner(m.corner_edge(c), |r| {
                    let g = m.corner_face(r);
                    if inside[g as usize] != 0 && !seen[g as usize] {
                        seen[g as usize] = true;
                        stack.push(g);
                    }
                });
            });
        }
        out.push(group);
    }
    out
}

/// A border to fill: its points in the winding of the faces that fill it, and the faces it replaces, if any.
#[derive(Default)]
struct Patch {
    border: Vec<FillPoint>,
    centre: FillPoint,
    src_face: Index,
    faces: Vec<Index>,
}

/// Fills each patch. A patch that is a symmetry image of one already filled is filled from that one's border mapped
/// across, so both get the same grid point for point and the symmetry still pairs them.
fn fill_patches(mesh: &mut Mesh, patches: &[Patch], o: &GridFillOptions, sym: Option<&SymmetryMap>) -> Vec<Index> {
    let mut owner: HashMap<Index, usize> = HashMap::new();
    for (i, p) in patches.iter().enumerate() {
        for b in &p.border {
            owner.insert(b.id, i);
        }
    }
    let mut done = vec![false; patches.len()];
    let mut out = Vec::new();
    let fill = |mesh: &mut Mesh, out: &mut Vec<Index>, patch: &Patch, border: &[FillPoint], flip: bool| {
        remove_faces_clean(mesh, &patch.faces);
        out.extend(fill_loop(mesh, border, &patch.centre, patch.src_face, o, flip));
    };
    for i in 0..patches.len() {
        if done[i] {
            continue;
        }
        done[i] = true;
        let mut border = patches[i].border.clone();
        let n = border.len();
        if n >= 3 {
            let s = fill_start(&border, o) as usize;
            border.rotate_left(s);
        }
        fill(mesh, &mut out, &patches[i], &border, false);
        let Some(sym) = sym else { continue };
        if n < 3 {
            continue;
        }
        for g in 1..sym.order() {
            let image: Vec<Index> = border.iter().map(|p| sym.image(g, p.id)).collect();
            let Some(&oi) = owner.get(&image[0]) else { continue };
            if done[oi] || patches[oi].border.len() != n {
                continue;
            }
            let other = &patches[oi];
            let at: HashMap<Index, usize> = other.border.iter().enumerate().map(|(k, b)| (b.id, k)).collect();
            let mapped: Vec<FillPoint> =
                image.iter().filter_map(|q| at.get(q).map(|&k| other.border[k].clone())).collect();
            if mapped.len() != n {
                continue;
            }
            done[oi] = true;
            let flip = other.border[(at[&image[0]] + 1) % n].id != image[1];
            fill(mesh, &mut out, other, &mapped, flip);
        }
    }
    out
}

/// Replaces each connected patch of faces, such as the triangle fan round a pole, with a quad grid inside a ring of
/// quads. A patch must be a disc with one border loop; others are left alone. Returns the new faces.
pub fn grid_fill_faces(mesh: &mut Mesh, faces: &[Index], opts: &GridFillOptions, sym: Option<&SymmetryMap>) -> Vec<Index> {
    let sel: Vec<Index> = unique_sorted(faces).into_iter().filter(|&f| mesh.face_valid(f)).collect();
    let mut found = Vec::new();
    for patch in patches(mesh, &sel) {
        let inside = make_mask(mesh.face_capacity() as usize, &patch);
        let rim = region_boundary_edges(mesh, &patch);
        // Each rim edge's corner inside the patch, keyed by the point it leaves; a point leaving twice is not a disc.
        let mut leaving: HashMap<Index, Index> = HashMap::new();
        let mut first = INVALID;
        let mut disc = !rim.is_empty();
        for &e in &rim {
            mesh.for_each_edge_corner(e, |c| {
                if inside[mesh.corner_face(c) as usize] != 0 {
                    if first == INVALID {
                        first = c;
                    }
                    disc &= leaving.insert(mesh.corner_point(c), c).is_none();
                }
            });
        }
        if !disc || leaving.len() != rim.len() {
            continue;
        }
        let mut p = Patch::default();
        let mut c = first;
        loop {
            p.border.push(border_point(mesh, mesh.corner_point(c), c));
            let Some(&next) = leaving.get(&mesh.corner_point(mesh.corner_next(c))) else { break };
            if p.border.len() > rim.len() {
                break;
            }
            c = next;
            if c == first {
                break;
            }
        }
        // Fewer points than rim edges means a second loop: a ring or a patch with holes.
        if c != first || p.border.len() != rim.len() {
            continue;
        }

        let mut inside_pts: Vec<FillPoint> = Vec::new();
        for &f in &patch {
            mesh.for_each_face_corner(f, |fc| {
                let q = mesh.corner_point(fc);
                if !leaving.contains_key(&q) && !inside_pts.iter().any(|r| r.id == q) {
                    inside_pts.push(border_point(mesh, q, fc));
                }
            });
        }
        p.centre = average_point(if inside_pts.is_empty() { &p.border } else { &inside_pts });
        p.src_face = mesh.corner_face(first);
        p.faces = patch;
        found.push(p);
    }
    fill_patches(mesh, &found, opts, sym)
}

/// Fills the holes bordered by the given edges the same way (all holes when empty). Returns the new faces. With sym,
/// a patch or hole that is the image of another is filled as that one's mirror, point for point.
pub fn grid_fill_holes(mesh: &mut Mesh, edges: &[Index], opts: &GridFillOptions, sym: Option<&SymmetryMap>) -> Vec<Index> {
    let sel = make_mask(mesh.edge_capacity() as usize, edges);
    let mut found = Vec::new();
    for lp in boundary_loops(mesh) {
        if !lp.closed || lp.points.len() < 3 {
            continue;
        }
        let wanted = edges.is_empty() || lp.edges.iter().any(|&e| sel[e as usize] != 0);
        if !wanted {
            continue;
        }
        let mut p = Patch::default();
        for i in 0..lp.points.len() {
            p.border.push(border_point(mesh, lp.points[i], mesh.corner_next(mesh.edge_corner(lp.edges[i]))));
        }
        p.centre = average_point(&p.border);
        p.src_face = mesh.corner_face(mesh.edge_corner(lp.edges[0]));
        found.push(p);
    }
    fill_patches(mesh, &found, opts, sym)
}
