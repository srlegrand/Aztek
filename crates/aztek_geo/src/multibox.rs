//! Splitting a mesh into chunks that largely conform to boxes.

use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::sync::OnceLock;

use glam::{DMat3, DVec3, IVec3};
use rayon::prelude::*;

use crate::attribute::Index;
use crate::bounding::{box_in_frame, convex_hull_mesh, minimum_volume_box, OrientedBox};
use crate::bvh::MeshBvh;
use crate::mesh::{Bounds, Mesh};

#[derive(Clone, Debug)]
pub struct MultiBoxOptions {
    pub max_boxes: i32,
    /// Occupied volume over box volume a chunk should reach before it stops being split and may be merged.
    pub min_fill: f64,
    /// Smallest extent a split may leave, in scene units; 0 means two voxels.
    pub min_box_size: f64,
    /// Voxels along the longest side of the mesh.
    pub resolution: i32,
    /// Chunks get their own minimum-volume frame; off keeps every box on the world axes.
    pub oriented: bool,
    /// Pushed out on every side of each chunk.
    pub padding: f64,
    /// box, hull, or auto (a box unless the chunk's convex hull is clearly smaller).
    pub shape: String,
    pub max_hull_points: i32,
}

impl Default for MultiBoxOptions {
    fn default() -> Self {
        Self {
            max_boxes: 8,
            min_fill: 0.85,
            min_box_size: 0.0,
            resolution: 48,
            oriented: true,
            padding: 0.0,
            shape: "auto".into(),
            max_hull_points: 32,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct BoxChunk {
    pub box_: OrientedBox,
    pub hull: bool,
    /// The chunk's convex hull in world space, when hull is set.
    pub hull_mesh: Mesh,
    /// Occupied volume over the chunk's volume, 0..1.
    pub fill: f64,
    pub volume: f64,
}

#[derive(Clone, Debug, Default)]
pub struct MultiBoxResult {
    pub chunks: Vec<BoxChunk>,
    pub average_fill: f64,
    /// Share of the mesh's occupied volume that lies inside a box.
    pub coverage: f64,
}

const SURFACE: u8 = 1;
const EXTERIOR: u8 = 2;
const STEP6: [[i32; 3]; 6] = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]];

#[derive(Default)]
struct Grid {
    origin: DVec3,
    cell: f64,
    n: [i32; 3],
    state: Vec<u8>,
    filled: Vec<u8>,
}

impl Grid {
    fn size(&self) -> usize {
        self.n[0] as usize * self.n[1] as usize * self.n[2] as usize
    }
    fn index(&self, x: i32, y: i32, z: i32) -> usize {
        (z as usize * self.n[1] as usize + y as usize) * self.n[0] as usize + x as usize
    }
    fn coords(&self, i: usize) -> IVec3 {
        let nx = self.n[0] as usize;
        let ny = self.n[1] as usize;
        IVec3::new((i % nx) as i32, ((i / nx) % ny) as i32, (i / (nx * ny)) as i32)
    }
    fn center(&self, x: i32, y: i32, z: i32) -> DVec3 {
        self.origin + (DVec3::new(x as f64, y as f64, z as f64) + 0.5) * self.cell
    }
    fn cell_of(&self, p: DVec3) -> IVec3 {
        let mut c = IVec3::ZERO;
        for k in 0..3 {
            c[k] = (((p[k] - self.origin[k]) / self.cell).floor() as i32).clamp(0, self.n[k] - 1);
        }
        c
    }
    fn inside(&self, x: i32, y: i32, z: i32) -> bool {
        x >= 0 && y >= 0 && z >= 0 && x < self.n[0] && y < self.n[1] && z < self.n[2]
    }
}

fn min3(a: f64, b: f64, c: f64) -> f64 {
    a.min(b).min(c)
}

fn max3(a: f64, b: f64, c: f64) -> f64 {
    a.max(b).max(c)
}

fn separated(axis: DVec3, h: DVec3, a: DVec3, b: DVec3, c: DVec3) -> bool {
    let (p0, p1, p2) = (a.dot(axis), b.dot(axis), c.dot(axis));
    let r = h.x * axis.x.abs() + h.y * axis.y.abs() + h.z * axis.z.abs();
    min3(p0, p1, p2) > r || max3(p0, p1, p2) < -r
}

fn triangle_overlaps_box(center: DVec3, h: DVec3, a: DVec3, b: DVec3, c: DVec3) -> bool {
    let (a, b, c) = (a - center, b - center, c - center);
    let edges = [b - a, c - b, a - c];
    for e in edges {
        for k in 0..3 {
            let mut unit = DVec3::ZERO;
            unit[k] = 1.0;
            let axis = unit.cross(e);
            if axis.dot(axis) > 1e-24 && separated(axis, h, a, b, c) {
                return false;
            }
        }
    }
    for k in 0..3 {
        if min3(a[k], b[k], c[k]) > h[k] || max3(a[k], b[k], c[k]) < -h[k] {
            return false;
        }
    }
    !separated(edges[0].cross(edges[1]), h, a, b, c)
}

struct Sample {
    p: DVec3,
    voxel: usize,
    shell: i32,
}

#[derive(Default)]
struct Voxelized {
    grid: Grid,
    samples: Vec<Sample>,
    shells: i32,
}

/// Connected pieces of the mesh, numbered by their lowest point; points on no face get their own.
fn shells_of(mesh: &Mesh) -> (Vec<i32>, i32) {
    let mut parent: Vec<Index> = (0..mesh.point_capacity()).collect();
    fn find(parent: &mut [Index], mut i: Index) -> Index {
        while parent[i as usize] != i {
            parent[i as usize] = parent[parent[i as usize] as usize];
            i = parent[i as usize];
        }
        i
    }
    for f in 0..mesh.face_capacity() {
        if mesh.face_valid(f) {
            let pts = mesh.face_points(f);
            for &q in &pts[1..] {
                let a = find(&mut parent, pts[0]);
                let b = find(&mut parent, q);
                if a != b {
                    parent[a.max(b) as usize] = a.min(b);
                }
            }
        }
    }
    let pc = mesh.point_capacity() as usize;
    let mut id = vec![-1i32; pc];
    let mut out = vec![-1i32; pc];
    let mut count = 0;
    for p in 0..mesh.point_capacity() {
        if mesh.point_valid(p) {
            let r = find(&mut parent, p) as usize;
            if id[r] < 0 {
                id[r] = count;
                count += 1;
            }
            out[p as usize] = id[r];
        }
    }
    (out, count)
}

/// Surface cells by triangle overlap, outside by flood fill, and any cell an odd ray count calls inside on two axes.
fn voxelize(mesh: &Mesh, resolution: i32) -> Voxelized {
    let mut out = Voxelized::default();
    let mut b = Bounds::default();
    for p in 0..mesh.point_capacity() {
        if mesh.point_valid(p) {
            b.extend(mesh.position(p));
        }
    }
    if !b.valid() {
        return out;
    }
    let ext = b.size();
    let longest = max3(ext.x, ext.y, ext.z);
    if longest <= 0.0 {
        return out;
    }
    let g = &mut out.grid;
    g.cell = longest / resolution.clamp(4, 256) as f64;
    g.origin = b.min - DVec3::splat(g.cell);
    for k in 0..3 {
        g.n[k] = (ext[k] / g.cell).ceil() as i32 + 3;
    }
    g.state = vec![0; g.size()];

    let (shell_of, shells) = shells_of(mesh);
    out.shells = shells;
    let bvh = MeshBvh::new(mesh);
    let tris = bvh.triangles();
    let half = DVec3::splat(g.cell * 0.5 * (1.0 + 1e-9));

    for p in 0..mesh.point_capacity() {
        if mesh.point_valid(p) {
            let c = g.cell_of(mesh.position(p));
            let i = g.index(c.x, c.y, c.z);
            g.state[i] |= SURFACE;
            out.samples.push(Sample { p: mesh.position(p), voxel: i, shell: shell_of[p as usize] });
        }
    }

    let mut t = 0;
    while t * 3 + 2 < tris.corners.len() {
        let v: [DVec3; 3] = std::array::from_fn(|k| mesh.position(mesh.corner_point(tris.corners[t * 3 + k])));
        let shell = shell_of[mesh.corner_point(tris.corners[t * 3]) as usize];
        let mut tb = Bounds::default();
        for &p in &v {
            tb.extend(p);
        }
        let lo = g.cell_of(tb.min);
        let hi = g.cell_of(tb.max);
        for z in lo.z..=hi.z {
            for y in lo.y..=hi.y {
                for x in lo.x..=hi.x {
                    if triangle_overlaps_box(g.center(x, y, z), half, v[0], v[1], v[2]) {
                        let i = g.index(x, y, z);
                        g.state[i] |= SURFACE;
                    }
                }
            }
        }

        let longest_edge = max3((v[1] - v[0]).length(), (v[2] - v[1]).length(), (v[0] - v[2]).length());
        let div = ((longest_edge / (g.cell * 0.5)).ceil() as i32).clamp(1, 32);
        for i in 0..=div {
            for j in 0..=div - i {
                let k = div - i - j;
                if i == div || j == div || k == div {
                    continue;
                }
                let p = (v[0] * i as f64 + v[1] * j as f64 + v[2] * k as f64) / div as f64;
                let c = g.cell_of(p);
                out.samples.push(Sample { p, voxel: g.index(c.x, c.y, c.z), shell });
            }
        }
        t += 1;
    }

    let mut stack = vec![0usize];
    g.state[0] |= EXTERIOR;
    while let Some(top) = stack.pop() {
        let c = g.coords(top);
        for s in &STEP6 {
            let (x, y, z) = (c.x + s[0], c.y + s[1], c.z + s[2]);
            if !g.inside(x, y, z) {
                continue;
            }
            let i = g.index(x, y, z);
            if g.state[i] & (SURFACE | EXTERIOR) != 0 {
                continue;
            }
            g.state[i] |= EXTERIOR;
            stack.push(i);
        }
    }

    let mut votes = vec![0u8; g.size()];
    let eps = g.cell * 1e-6;
    for axis in 0..3usize {
        let u = (axis + 1) % 3;
        let w = (axis + 2) % 3;
        let ju = g.cell * (0.0713 + 0.0191 * axis as f64);
        let jw = g.cell * (0.0397 + 0.0263 * axis as f64);
        let mut dir = DVec3::ZERO;
        dir[axis] = 1.0;
        let span = g.cell * (g.n[axis] + 2) as f64;
        let g: &Grid = g;
        let rows: Vec<Vec<usize>> = (0..g.n[w])
            .into_par_iter()
            .map(|cw| {
                let mut out = Vec::new();
                let mut hits: Vec<f64> = Vec::new();
                for cu in 0..g.n[u] {
                    let mut o = DVec3::ZERO;
                    o[u] = g.origin[u] + (cu as f64 + 0.5) * g.cell + ju;
                    o[w] = g.origin[w] + (cw as f64 + 0.5) * g.cell + jw;
                    o[axis] = g.origin[axis] - g.cell;
                    hits.clear();
                    for _ in 0..4096 {
                        let Some(hit) = bvh.raycast_with(o, dir, span, true) else { break };
                        hits.push(hit.point[axis]);
                        o[axis] = hit.point[axis] + eps;
                    }
                    let mut h = 0;
                    for ca in 0..g.n[axis] {
                        let c = g.origin[axis] + (ca as f64 + 0.5) * g.cell;
                        while h < hits.len() && hits[h] < c {
                            h += 1;
                        }
                        if h & 1 != 0 {
                            let mut xyz = [0i32; 3];
                            xyz[axis] = ca;
                            xyz[u] = cu;
                            xyz[w] = cw;
                            out.push(g.index(xyz[0], xyz[1], xyz[2]));
                        }
                    }
                }
                out
            })
            .collect();
        for row in rows {
            for i in row {
                votes[i] += 1;
            }
        }
    }

    let g = &mut out.grid;
    g.filled = (0..g.size())
        .map(|i| ((g.state[i] & SURFACE != 0) || (g.state[i] & EXTERIOR == 0) || votes[i] >= 2) as u8)
        .collect();
    out
}

#[derive(Clone, Copy)]
struct IBox {
    lo: [i32; 3],
    hi: [i32; 3],
}

struct Counts {
    nx: usize,
    ny: usize,
    sum: Vec<i32>,
}

impl Counts {
    fn new(g: &Grid) -> Self {
        let nx = g.n[0] as usize + 1;
        let ny = g.n[1] as usize + 1;
        let mut c = Counts { nx, ny, sum: vec![0; nx * ny * (g.n[2] as usize + 1)] };
        for z in 0..g.n[2] {
            for y in 0..g.n[1] {
                for x in 0..g.n[0] {
                    let v = g.filled[g.index(x, y, z)] as i32 + c.get(x, y + 1, z + 1) + c.get(x + 1, y, z + 1)
                        + c.get(x + 1, y + 1, z)
                        - c.get(x, y, z + 1)
                        - c.get(x, y + 1, z)
                        - c.get(x + 1, y, z)
                        + c.get(x, y, z);
                    let i = c.at(x + 1, y + 1, z + 1);
                    c.sum[i] = v;
                }
            }
        }
        c
    }

    fn at(&self, x: i32, y: i32, z: i32) -> usize {
        (z as usize * self.ny + y as usize) * self.nx + x as usize
    }

    fn get(&self, x: i32, y: i32, z: i32) -> i32 {
        self.sum[self.at(x, y, z)]
    }

    fn count_in(&self, b: &IBox) -> i64 {
        let (x0, y0, z0) = (b.lo[0], b.lo[1], b.lo[2]);
        let (x1, y1, z1) = (b.hi[0] + 1, b.hi[1] + 1, b.hi[2] + 1);
        self.get(x1, y1, z1) as i64 - self.get(x0, y1, z1) as i64 - self.get(x1, y0, z1) as i64
            - self.get(x1, y1, z0) as i64
            + self.get(x0, y0, z1) as i64
            + self.get(x0, y1, z0) as i64
            + self.get(x1, y0, z0) as i64
            - self.get(x0, y0, z0) as i64
    }
}

fn volume_of(b: &IBox) -> i64 {
    (b.hi[0] - b.lo[0] + 1) as i64 * (b.hi[1] - b.lo[1] + 1) as i64 * (b.hi[2] - b.lo[2] + 1) as i64
}

fn union_of(a: &IBox, b: &IBox) -> IBox {
    let mut r = *a;
    for k in 0..3 {
        r.lo[k] = a.lo[k].min(b.lo[k]);
        r.hi[k] = a.hi[k].max(b.hi[k]);
    }
    r
}

fn near_each_other(a: &IBox, b: &IBox) -> bool {
    (0..3).all(|k| (b.lo[k] - a.hi[k] - 1).max(a.lo[k] - b.hi[k] - 1) <= 1)
}

/// Exact cover of the filled cells by axis-aligned boxes, grown greedily along x, then y, then z.
fn greedy_cover(g: &Grid, box_of: &mut Vec<i32>) -> Vec<IBox> {
    let mut boxes = Vec::new();
    *box_of = vec![-1; g.size()];
    for z in 0..g.n[2] {
        for y in 0..g.n[1] {
            for x in 0..g.n[0] {
                let free = |box_of: &Vec<i32>, x: i32, y: i32, z: i32| {
                    let i = g.index(x, y, z);
                    g.filled[i] != 0 && box_of[i] < 0
                };
                if !free(box_of, x, y, z) {
                    continue;
                }
                let mut x1 = x;
                while x1 + 1 < g.n[0] && free(box_of, x1 + 1, y, z) {
                    x1 += 1;
                }
                let mut y1 = y;
                let mut ok = true;
                while ok && y1 + 1 < g.n[1] {
                    let mut i = x;
                    while i <= x1 && ok {
                        ok = free(box_of, i, y1 + 1, z);
                        i += 1;
                    }
                    y1 += ok as i32;
                }
                let mut z1 = z;
                ok = true;
                while ok && z1 + 1 < g.n[2] {
                    let mut j = y;
                    while j <= y1 && ok {
                        let mut i = x;
                        while i <= x1 && ok {
                            ok = free(box_of, i, j, z1 + 1);
                            i += 1;
                        }
                        j += 1;
                    }
                    z1 += ok as i32;
                }
                let id = boxes.len() as i32;
                boxes.push(IBox { lo: [x, y, z], hi: [x1, y1, z1] });
                for k in z..=z1 {
                    for j in y..=y1 {
                        for i in x..=x1 {
                            box_of[g.index(i, j, k)] = id;
                        }
                    }
                }
            }
        }
    }
    boxes
}

/// A merge candidate in a max-heap: highest fill first, then lowest i, then lowest j.
struct Cand<T> {
    fill: f64,
    i: usize,
    j: usize,
    data: T,
}

impl<T> PartialEq for Cand<T> {
    fn eq(&self, o: &Self) -> bool {
        self.cmp(o) == Ordering::Equal
    }
}
impl<T> Eq for Cand<T> {}
impl<T> PartialOrd for Cand<T> {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl<T> Ord for Cand<T> {
    fn cmp(&self, o: &Self) -> Ordering {
        self.fill.total_cmp(&o.fill).then(o.i.cmp(&self.i)).then(o.j.cmp(&self.j))
    }
}

/// Merges boxes whose union stays full, best first. Returns each box's surviving group (a node id).
fn merge_regions(boxes: &[IBox], counts: &Counts, min_fill: f64) -> (Vec<usize>, usize) {
    let mut nodes: Vec<IBox> = boxes.to_vec();
    let mut up: Vec<usize> = (0..boxes.len()).collect();
    let mut alive = vec![true; boxes.len()];
    let mut queue: BinaryHeap<Cand<()>> = BinaryHeap::new();
    let consider = |queue: &mut BinaryHeap<Cand<()>>, nodes: &[IBox], i: usize, j: usize| {
        let u = union_of(&nodes[i], &nodes[j]);
        let fill = counts.count_in(&u) as f64 / volume_of(&u) as f64;
        if fill >= min_fill {
            queue.push(Cand { fill, i: i.min(j), j: i.max(j), data: () });
        }
    };

    let mut order: Vec<usize> = (0..boxes.len()).collect();
    order.sort_by(|&a, &b| boxes[a].lo[0].cmp(&boxes[b].lo[0]).then(a.cmp(&b)));
    for a in 0..order.len() {
        for b in a + 1..order.len() {
            let (pa, pb) = (&boxes[order[a]], &boxes[order[b]]);
            if pb.lo[0] - pa.hi[0] - 1 > 1 {
                break;
            }
            if near_each_other(pa, pb) {
                consider(&mut queue, &nodes, order[a], order[b]);
            }
        }
    }

    while let Some(c) = queue.pop() {
        if !alive[c.i] || !alive[c.j] {
            continue;
        }
        let k = nodes.len();
        nodes.push(union_of(&nodes[c.i], &nodes[c.j]));
        alive.push(true);
        up.push(k);
        alive[c.i] = false;
        alive[c.j] = false;
        up[c.i] = k;
        up[c.j] = k;
        for o in 0..k {
            if alive[o] && near_each_other(&nodes[k], &nodes[o]) {
                consider(&mut queue, &nodes, o, k);
            }
        }
    }
    let root = (0..boxes.len())
        .map(|i| {
            let mut r = i;
            while up[r] != r {
                r = up[r];
            }
            r
        })
        .collect();
    (root, nodes.len())
}

fn probe_directions() -> &'static [DVec3] {
    static DIRS: OnceLock<Vec<DVec3>> = OnceLock::new();
    DIRS.get_or_init(|| {
        let mut d = vec![DVec3::X, -DVec3::X, DVec3::Y, -DVec3::Y, DVec3::Z, -DVec3::Z];
        let n = 90;
        let golden = 3.14159265358979323846 * (3.0 - 5f64.sqrt());
        for i in 0..n {
            let z = 1.0 - 2.0 * (i as f64 + 0.5) / n as f64;
            let r = (1.0 - z * z).sqrt();
            d.push(DVec3::new(r * (golden * i as f64).cos(), r * (golden * i as f64).sin(), z));
        }
        d
    })
}

#[derive(Clone)]
struct Node {
    pts: Vec<DVec3>,
    /// Per probe direction, the point farthest along it: a small stand-in for the hull when solving frames.
    ext: Vec<DVec3>,
    vox: Vec<u32>,
    box_: OrientedBox,
    fill: f64,
    alive: bool,
    unsplittable: bool,
}

impl Default for Node {
    fn default() -> Self {
        Self {
            pts: Vec::new(),
            ext: Vec::new(),
            vox: Vec::new(),
            box_: OrientedBox::default(),
            fill: 0.0,
            alive: true,
            unsplittable: false,
        }
    }
}

#[derive(Clone, Copy)]
struct Cut {
    axis: i32,
    bin: i32,
}

#[derive(Clone, Copy)]
struct Bin {
    count: i32,
    lo: DVec3,
    hi: DVec3,
}

impl Default for Bin {
    fn default() -> Self {
        Self { count: 0, lo: DVec3::splat(f64::MAX), hi: DVec3::splat(-f64::MAX) }
    }
}

fn absorb(a: &mut Bin, b: &Bin) {
    a.count += b.count;
    a.lo = a.lo.min(b.lo);
    a.hi = a.hi.max(b.hi);
}

fn probe(n: &mut Node) {
    let dirs = probe_directions();
    let first = n.pts.first().copied().unwrap_or(DVec3::ZERO);
    n.ext = vec![first; dirs.len()];
    for (d, dir) in dirs.iter().enumerate() {
        let mut best = n.ext[d].dot(*dir);
        for &p in &n.pts {
            let t = p.dot(*dir);
            if t > best {
                best = t;
                n.ext[d] = p;
            }
        }
    }
}

fn merged_extremes(a: &Node, b: &Node) -> Vec<DVec3> {
    probe_directions()
        .iter()
        .enumerate()
        .map(|(d, dir)| if b.ext[d].dot(*dir) > a.ext[d].dot(*dir) { b.ext[d] } else { a.ext[d] })
        .collect()
}

fn union_in_frame(a: &OrientedBox, b: &OrientedBox) -> OrientedBox {
    let mut r = OrientedBox { axes: a.axes, ..Default::default() };
    for i in 0..3 {
        let ax = a.axes.col(i);
        let lo = (a.center.dot(ax) - a.size[i] * 0.5).min(b.center.dot(ax) - b.size[i] * 0.5);
        let hi = (a.center.dot(ax) + a.size[i] * 0.5).max(b.center.dot(ax) + b.size[i] * 0.5);
        r.size[i] = hi - lo;
        r.center += ax * ((lo + hi) * 0.5);
    }
    r
}

struct Ctx<'a> {
    opts: &'a MultiBoxOptions,
    g: &'a Grid,
    counts: &'a Counts,
}

impl Ctx<'_> {
    fn cell3(&self) -> f64 {
        self.g.cell * self.g.cell * self.g.cell
    }

    fn reach(&self, b: &OrientedBox, pad: f64) -> DVec3 {
        let mut reach = DVec3::ZERO;
        for i in 0..3 {
            for k in 0..3 {
                reach[k] += b.axes.col(i)[k].abs() * (b.size[i] * 0.5 + pad);
            }
        }
        reach
    }

    fn padded_volume(&self, b: &OrientedBox) -> f64 {
        let h = self.g.cell * 0.5;
        b.size.x.max(h) * b.size.y.max(h) * b.size.z.max(h)
    }

    /// Most the box could hold: every filled voxel in its axis-aligned extent.
    fn fill_ceiling(&self, b: &OrientedBox) -> f64 {
        let reach = self.reach(b, self.g.cell);
        let lo = self.g.cell_of(b.center - reach);
        let hi = self.g.cell_of(b.center + reach);
        self.counts.count_in(&IBox { lo: lo.to_array(), hi: hi.to_array() }) as f64 * self.cell3() / self.padded_volume(b)
    }

    /// The box around a node (or two): frame solved from the extreme points, extents measured over every point.
    fn fit(&self, a: &Node, b: Option<&Node>) -> OrientedBox {
        let mut frame = DMat3::IDENTITY;
        if self.opts.oriented {
            let e = match b {
                Some(b) => merged_extremes(a, b),
                None => a.ext.clone(),
            };
            if let Some(ob) = minimum_volume_box(&e) {
                frame = ob.axes;
            }
        }
        let r = box_in_frame(&a.pts, &frame);
        match b {
            Some(b) => union_in_frame(&r, &box_in_frame(&b.pts, &frame)),
            None => r,
        }
    }

    /// Filled-voxel volume inside the box; each voxel counts by how much of it the box covers.
    fn occupied(&self, b: &OrientedBox) -> f64 {
        let g = self.g;
        let reach = self.reach(b, g.cell);
        let width: [f64; 3] = std::array::from_fn(|i| {
            let a = b.axes.col(i);
            g.cell * (a.x.abs() + a.y.abs() + a.z.abs())
        });
        let lo = g.cell_of(b.center - reach);
        let hi = g.cell_of(b.center + reach);
        let mut cells = 0.0;
        for z in lo.z..=hi.z {
            for y in lo.y..=hi.y {
                for x in lo.x..=hi.x {
                    if g.filled[g.index(x, y, z)] == 0 {
                        continue;
                    }
                    let d = g.center(x, y, z) - b.center;
                    let mut share = 1.0;
                    for i in 0..3 {
                        if share <= 0.0 {
                            break;
                        }
                        share *= ((b.size[i] * 0.5 - d.dot(b.axes.col(i)).abs()) / width[i] + 0.5).clamp(0.0, 1.0);
                    }
                    cells += share;
                }
            }
        }
        cells * self.cell3()
    }

    fn fill_of(&self, b: &OrientedBox) -> f64 {
        (self.occupied(b) / self.padded_volume(b)).min(1.0)
    }

    fn refit(&self, n: &mut Node) {
        probe(n);
        n.box_ = self.fit(n, None);
        n.fill = self.fill_of(&n.box_);
    }

    fn world_bounds(&self, b: &OrientedBox) -> (DVec3, DVec3) {
        let reach = self.reach(b, self.g.cell * 0.5);
        (b.center - reach, b.center + reach)
    }

    fn adjacent(&self, a: &Node, b: &Node) -> bool {
        let (alo, ahi) = self.world_bounds(&a.box_);
        let (blo, bhi) = self.world_bounds(&b.box_);
        (0..3).all(|k| !(blo[k] > ahi[k] + self.g.cell || alo[k] > bhi[k] + self.g.cell))
    }

    fn binned_volume(&self, b: &Bin) -> f64 {
        let c = self.g.cell;
        (b.hi.x - b.lo.x + c) * (b.hi.y - b.lo.y + c) * (b.hi.z - b.lo.z + c)
    }

    /// The plane along the box's frame that leaves the two voxel halves the least combined volume, if it beats the
    /// whole.
    fn best_cut(&self, n: &Node) -> Cut {
        let g = self.g;
        let b = &n.box_;
        let min_extent = if self.opts.min_box_size > 0.0 { self.opts.min_box_size } else { 2.0 * g.cell };
        let nb: [i32; 3] = std::array::from_fn(|i| (((b.size[i] + g.cell) / g.cell - 1e-9).ceil() as i32).max(1));
        let mut bins: [Vec<Bin>; 3] = std::array::from_fn(|i| vec![Bin::default(); nb[i] as usize]);
        let mut whole = Bin::default();
        for &v in &n.vox {
            let c = g.coords(v as usize);
            let d = g.center(c.x, c.y, c.z) - b.center;
            let u = DVec3::new(d.dot(b.axes.x_axis), d.dot(b.axes.y_axis), d.dot(b.axes.z_axis));
            let one = Bin { count: 1, lo: u, hi: u };
            absorb(&mut whole, &one);
            for i in 0..3 {
                let k = (((u[i] + (b.size[i] + g.cell) * 0.5) / g.cell).floor() as i32).clamp(0, nb[i] - 1);
                absorb(&mut bins[i][k as usize], &one);
            }
        }
        let mut best = self.binned_volume(&whole) * 0.97;
        let mut cut = Cut { axis: -1, bin: 0 };
        for i in 0..3 {
            let nbi = nb[i] as usize;
            let mut right = vec![Bin::default(); nbi + 1];
            for k in (0..nbi).rev() {
                right[k] = right[k + 1];
                absorb(&mut right[k], &bins[i][k]);
            }
            let mut left = Bin::default();
            for k in 0..nbi.saturating_sub(1) {
                absorb(&mut left, &bins[i][k]);
                let r = &right[k + 1];
                if left.count == 0 || r.count == 0 {
                    continue;
                }
                if left.hi[i] - left.lo[i] + g.cell < min_extent || r.hi[i] - r.lo[i] + g.cell < min_extent {
                    continue;
                }
                let cost = self.binned_volume(&left) + self.binned_volume(r);
                if cost < best {
                    best = cost;
                    cut = Cut { axis: i as i32, bin: k as i32 };
                }
            }
        }
        cut
    }

    fn add_corners(&self, pts: &mut Vec<DVec3>, v: u32) {
        let c = self.g.coords(v as usize).as_dvec3();
        for k in 0..8 {
            let o = DVec3::new((k & 1) as f64, ((k >> 1) & 1) as f64, ((k >> 2) & 1) as f64);
            pts.push(self.g.origin + (c + o) * self.g.cell);
        }
    }
}

struct Decomposer<'a> {
    ctx: Ctx<'a>,
    nodes: Vec<Node>,
}

impl Decomposer<'_> {
    fn alive_count(&self) -> i32 {
        self.nodes.iter().filter(|n| n.alive).count() as i32
    }

    /// Merges the best neighbouring pair while its refit box stays at or above the threshold; past a count limit it
    /// keeps going regardless.
    fn merge(&mut self, threshold: f64, limit: i32) {
        let mut queue: BinaryHeap<Cand<OrientedBox>> = BinaryHeap::new();
        let mut any_pair = false;
        let mut floor = threshold;
        fn consider(
            ctx: &Ctx,
            nodes: &[Node],
            queue: &mut BinaryHeap<Cand<OrientedBox>>,
            any_pair: bool,
            floor: f64,
            pairs: &[(usize, usize)],
        ) {
            let todo: Vec<(usize, usize)> =
                pairs.iter().copied().filter(|&(i, j)| any_pair || ctx.adjacent(&nodes[i], &nodes[j])).collect();
            let cands: Vec<Option<Cand<OrientedBox>>> = todo
                .par_iter()
                .map(|&(i, j)| {
                    let box_ = ctx.fit(&nodes[i], Some(&nodes[j]));
                    if floor > 0.0 && ctx.fill_ceiling(&box_) < floor {
                        return None;
                    }
                    let fill = ctx.fill_of(&box_);
                    (fill >= floor).then_some(Cand { fill, i, j, data: box_ })
                })
                .collect();
            for c in cands.into_iter().flatten() {
                queue.push(c);
            }
        }
        let seed = |nodes: &[Node], queue: &mut BinaryHeap<Cand<OrientedBox>>, any_pair: bool, floor: f64| {
            queue.clear();
            let mut pairs = Vec::new();
            for i in 0..nodes.len() {
                for j in i + 1..nodes.len() {
                    if nodes[i].alive && nodes[j].alive {
                        pairs.push((i, j));
                    }
                }
            }
            consider(&self.ctx, nodes, queue, any_pair, floor, &pairs);
        };
        seed(&self.nodes, &mut queue, any_pair, floor);
        let mut count = self.alive_count();
        while count > 1 && (limit < 0 || count > limit) {
            if queue.is_empty() {
                if limit < 0 || any_pair {
                    break;
                }
                any_pair = true;
                floor = 0.0;
                seed(&self.nodes, &mut queue, any_pair, floor);
                if queue.is_empty() {
                    break;
                }
            }
            let c = queue.pop().unwrap();
            if !self.nodes[c.i].alive || !self.nodes[c.j].alive {
                continue;
            }
            let ext = merged_extremes(&self.nodes[c.i], &self.nodes[c.j]);
            let mut pts = std::mem::take(&mut self.nodes[c.i].pts);
            pts.extend(std::mem::take(&mut self.nodes[c.j].pts));
            let mut vox = std::mem::take(&mut self.nodes[c.i].vox);
            vox.extend_from_slice(&self.nodes[c.j].vox);
            self.nodes[c.i].alive = false;
            self.nodes[c.j].alive = false;
            let k = self.nodes.len();
            self.nodes.push(Node { pts, ext, vox, box_: c.data, fill: c.fill, ..Default::default() });
            count -= 1;
            let pairs: Vec<(usize, usize)> = (0..k).filter(|&o| self.nodes[o].alive).map(|o| (o, k)).collect();
            consider(&self.ctx, &self.nodes, &mut queue, any_pair, floor, &pairs);
        }
    }

    fn split(&mut self, index: usize, cut: Cut) {
        let parent = std::mem::take(&mut self.nodes[index]);
        self.nodes[index].alive = false;
        let ctx = &self.ctx;
        let g = ctx.g;
        let b = &parent.box_;
        let a = cut.axis as usize;
        let plane = -(b.size[a] + g.cell) * 0.5 + (cut.bin + 1) as f64 * g.cell;
        let mut halves = [Node::default(), Node::default()];
        let axis = b.axes.col(a);
        let side = |p: DVec3| if (p - b.center).dot(axis) < plane { 0 } else { 1 };
        for &p in &parent.pts {
            halves[side(p)].pts.push(p);
        }
        for &v in &parent.vox {
            let c = g.coords(v as usize);
            let center = g.center(c.x, c.y, c.z);
            let s = side(center);
            halves[s].vox.push(v);
            if g.state[v as usize] & SURFACE == 0 && ((center - b.center).dot(axis) - plane).abs() <= g.cell {
                ctx.add_corners(&mut halves[s].pts, v);
            }
        }
        for mut h in halves {
            if h.pts.is_empty() {
                let vox = h.vox.clone();
                for v in vox {
                    ctx.add_corners(&mut h.pts, v);
                }
            }
            ctx.refit(&mut h);
            self.nodes.push(h);
        }
    }

    fn split_worst(&mut self) {
        while self.alive_count() < self.ctx.opts.max_boxes.max(1) {
            let g = self.ctx.g;
            let mut pick: Option<usize> = None;
            let mut worst = 0.0;
            for (i, n) in self.nodes.iter().enumerate() {
                if !n.alive || n.unsplittable || n.fill >= self.ctx.opts.min_fill {
                    continue;
                }
                let s = n.box_.size;
                let waste = (s.x + g.cell) * (s.y + g.cell) * (s.z + g.cell) * (1.0 - n.fill);
                if waste > worst {
                    worst = waste;
                    pick = Some(i);
                }
            }
            let Some(pick) = pick else { return };
            let cut = self.ctx.best_cut(&self.nodes[pick]);
            if cut.axis < 0 {
                self.nodes[pick].unsplittable = true;
                continue;
            }
            self.split(pick, cut);
        }
    }
}

fn mesh_volume(m: &Mesh) -> f64 {
    let mut sum = 0.0;
    for f in 0..m.face_capacity() {
        if !m.face_valid(f) {
            continue;
        }
        let pts = m.face_points(f);
        for i in 1..pts.len() - 1 {
            sum += m.position(pts[0]).dot(m.position(pts[i]).cross(m.position(pts[i + 1])));
        }
    }
    sum.abs() / 6.0
}

/// Splits a mesh into chunks that largely conform to boxes. The mesh is voxelized (interior filled by a flood fill
/// from outside plus ray parity, so open and overlapping shells work), the voxels are covered greedily by
/// axis-aligned boxes and merged into regions, then each region is refit, split where it is mostly empty and merged
/// with neighbours where the union is still full. Every point of the mesh lies in some chunk. Deterministic.
pub fn multi_box_decompose(mesh: &Mesh, opts: &MultiBoxOptions) -> MultiBoxResult {
    let mut result = MultiBoxResult::default();
    let vx = voxelize(mesh, opts.resolution);
    let g = &vx.grid;
    if g.state.is_empty() {
        return result;
    }

    let counts = Counts::new(g);
    let mut label = vec![-1i32; g.size()];
    let mut regions = 0i32;
    // Separate shells are the mesh's own parts, so they seed the regions when there are several.
    let by_shell = vx.shells > 1 && vx.shells <= 4096;
    if by_shell {
        regions = vx.shells;
        let mut frontier: Vec<usize> = Vec::new();
        for s in &vx.samples {
            if label[s.voxel] < 0 {
                label[s.voxel] = s.shell;
                frontier.push(s.voxel);
            }
        }
        let mut head = 0;
        while head < frontier.len() {
            let c = g.coords(frontier[head]);
            for st in &STEP6 {
                let (x, y, z) = (c.x + st[0], c.y + st[1], c.z + st[2]);
                if !g.inside(x, y, z) {
                    continue;
                }
                let i = g.index(x, y, z);
                if g.filled[i] != 0 && label[i] < 0 {
                    label[i] = label[frontier[head]];
                    frontier.push(i);
                }
            }
            head += 1;
        }
        for i in 0..g.size() {
            if g.filled[i] != 0 && label[i] < 0 {
                label[i] = 0;
            }
        }
    } else {
        let mut box_of = Vec::new();
        let cover = greedy_cover(g, &mut box_of);
        if cover.is_empty() {
            return result;
        }
        let (root, groups) = merge_regions(&cover, &counts, opts.min_fill);
        let mut region_of_root = vec![-1i32; groups];
        for &r in &root {
            if region_of_root[r] < 0 {
                region_of_root[r] = regions;
                regions += 1;
            }
        }
        for i in 0..g.size() {
            if box_of[i] >= 0 {
                label[i] = region_of_root[root[box_of[i] as usize]];
            }
        }
    }

    let mut d = Decomposer { ctx: Ctx { opts, g, counts: &counts }, nodes: vec![Node::default(); regions as usize] };
    for i in 0..g.size() {
        if label[i] < 0 {
            continue;
        }
        let l = label[i] as usize;
        d.nodes[l].vox.push(i as u32);
        if g.state[i] & SURFACE != 0 || by_shell {
            continue;
        }
        let c = g.coords(i);
        let rim = STEP6.iter().any(|s| {
            let (x, y, z) = (c.x + s[0], c.y + s[1], c.z + s[2]);
            !g.inside(x, y, z) || label[g.index(x, y, z)] != label[i]
        });
        if rim {
            let mut pts = std::mem::take(&mut d.nodes[l].pts);
            d.ctx.add_corners(&mut pts, i as u32);
            d.nodes[l].pts = pts;
        }
    }
    for s in &vx.samples {
        let mut l = if by_shell { s.shell } else { label[s.voxel] };
        if l < 0 {
            let c = g.coords(s.voxel);
            'search: for dz in -1..=1 {
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        let (x, y, z) = (c.x + dx, c.y + dy, c.z + dz);
                        if g.inside(x, y, z) {
                            l = label[g.index(x, y, z)];
                            if l >= 0 {
                                break 'search;
                            }
                        }
                    }
                }
            }
        }
        d.nodes[l.max(0) as usize].pts.push(s.p);
    }
    for n in &mut d.nodes {
        if n.pts.is_empty() {
            let vox = n.vox.clone();
            for v in vox {
                d.ctx.add_corners(&mut n.pts, v);
            }
        }
    }
    {
        let ctx = &d.ctx;
        d.nodes.par_iter_mut().for_each(|n| ctx.refit(n));
    }

    let limit = opts.max_boxes.max(1);
    d.merge(opts.min_fill, -1);
    d.merge(0.0, limit);
    d.split_worst();
    d.merge(opts.min_fill, -1);
    d.merge(0.0, limit);

    let mut live: Vec<&Node> = d.nodes.iter().filter(|n| n.alive).collect();
    live.sort_by(|a, b| {
        let (p, q) = (a.box_.center, b.box_.center);
        p.x.total_cmp(&q.x).then(p.y.total_cmp(&q.y)).then(p.z.total_cmp(&q.z))
    });

    let mut covered = vec![false; g.size()];
    let mut fill_sum = 0.0;
    for n in live {
        let mut chunk = BoxChunk { box_: n.box_, fill: n.fill, ..Default::default() };
        for i in 0..3 {
            chunk.box_.size[i] = chunk.box_.size[i].max(g.cell * 0.5);
        }
        chunk.volume = chunk.box_.size.x * chunk.box_.size.y * chunk.box_.size.z;

        let (lo, hi) = d.ctx.world_bounds(&n.box_);
        let clo = g.cell_of(lo);
        let chi = g.cell_of(hi);
        for z in clo.z..=chi.z {
            for y in clo.y..=chi.y {
                for x in clo.x..=chi.x {
                    let dv = g.center(x, y, z) - n.box_.center;
                    let inside = (0..3)
                        .all(|i| dv.dot(n.box_.axes.col(i)).abs() <= (n.box_.size[i] + g.cell) * 0.5 + 1e-9);
                    if inside {
                        covered[g.index(x, y, z)] = true;
                    }
                }
            }
        }

        if opts.shape != "box" {
            let hull = convex_hull_mesh(&n.pts, opts.max_hull_points, 0.0);
            let mut use_hull = hull.num_faces() > 0;
            if use_hull && opts.shape == "auto" {
                use_hull = mesh_volume(&hull) < 0.9 * chunk.volume;
            }
            if use_hull {
                chunk.hull = true;
                chunk.hull_mesh = if opts.padding > 0.0 {
                    convex_hull_mesh(&n.pts, opts.max_hull_points, opts.padding)
                } else {
                    hull
                };
            }
        }
        for i in 0..3 {
            chunk.box_.size[i] += 2.0 * opts.padding;
        }
        fill_sum += chunk.fill;
        result.chunks.push(chunk);
    }
    result.average_fill = fill_sum / result.chunks.len() as f64;
    let total = g.filled.iter().filter(|&&f| f != 0).count();
    let hit = (0..g.size()).filter(|&i| g.filled[i] != 0 && covered[i]).count();
    result.coverage = if total > 0 { hit as f64 / total as f64 } else { 0.0 };
    result
}
