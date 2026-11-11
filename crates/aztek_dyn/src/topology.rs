//! A body's connectivity: triangles from its polygons, edges, bending stencils and neighbours, and the colouring that
//! lets the constraints of one colour be solved at once.

use std::collections::HashMap;

use glam::DVec3;

/// Constraints sorted by colour; `ranges[c]` holds colour c. No two constraints of one colour share a particle, except
/// in the last range when `serial` is set: those are solved one after another.
#[derive(Clone, Debug, Default)]
pub struct Colouring {
    pub ranges: Vec<std::ops::Range<usize>>,
    pub serial: bool,
}

/// Colours constraints given the particles each touches, reordering them in place. Deterministic: greedy in order.
pub fn colour<const N: usize>(items: &mut Vec<[u32; N]>, extra: &mut [&mut Vec<f64>], particles: usize) -> Colouring {
    let mut used = vec![0u64; particles];
    let mut colour_of = vec![0u8; items.len()];
    let mut most = 0usize;
    let mut serial = false;
    for (k, it) in items.iter().enumerate() {
        let mut mask = 0u64;
        for &p in it {
            mask |= used[p as usize];
        }
        let c = (!mask).trailing_zeros() as usize;
        let c = if c >= 63 {
            serial = true;
            63
        } else {
            for &p in it {
                used[p as usize] |= 1 << c;
            }
            c
        };
        colour_of[k] = c as u8;
        most = most.max(c + 1);
    }
    let mut order: Vec<usize> = (0..items.len()).collect();
    order.sort_by_key(|&k| (colour_of[k], k));
    *items = order.iter().map(|&k| items[k]).collect();
    for e in extra.iter_mut() {
        let v: Vec<f64> = order.iter().map(|&k| e[k]).collect();
        **e = v;
    }
    let mut ranges = Vec::new();
    let mut at = 0;
    for c in 0..most {
        let n = colour_of.iter().filter(|&&x| x as usize == c).count();
        ranges.push(at..at + n);
        at += n;
    }
    Colouring { ranges, serial }
}

/// A mesh's connectivity, built once per body.
#[derive(Clone, Debug, Default)]
pub struct Topology {
    pub points: usize,
    pub polygons: Vec<Vec<u32>>,
    pub triangles: Vec<[u32; 3]>,
    /// Polygon each triangle came from.
    pub tri_polygon: Vec<u32>,
    /// Unique edges of the triangulation: polygon edges and the diagonals that hold quads in shape.
    pub edges: Vec<[u32; 2]>,
    /// Which edges are polygon edges (not diagonals).
    pub edge_is_border_of_polygon: Vec<bool>,
    /// Bending stencils: the shared edge (0, 1) and the two opposite corners (2, 3).
    pub bends: Vec<[u32; 4]>,
    pub neighbours: Vec<Vec<u32>>,
    /// Triangles touching each point.
    pub point_tris: Vec<Vec<u32>>,
    /// Points on an open border: an edge with one triangle.
    pub border: Vec<bool>,
}

impl Topology {
    /// From polygons over `points` points. Polygons with fewer than three distinct points or out of range indices
    /// are skipped.
    pub fn new(points: usize, polygons: &[Vec<u32>]) -> Topology {
        let mut t = Topology { points, ..Default::default() };
        for (fi, poly) in polygons.iter().enumerate() {
            let mut p: Vec<u32> = Vec::with_capacity(poly.len());
            for &v in poly {
                if (v as usize) < points && p.last() != Some(&v) {
                    p.push(v);
                }
            }
            if p.len() > 1 && p.first() == p.last() {
                p.pop();
            }
            if p.len() < 3 {
                continue;
            }
            t.polygons.push(p.clone());
            for k in 1..p.len() - 1 {
                let tri = [p[0], p[k], p[k + 1]];
                if tri[0] != tri[1] && tri[1] != tri[2] && tri[0] != tri[2] {
                    t.triangles.push(tri);
                    t.tri_polygon.push(fi as u32);
                }
            }
        }
        // Edges and the triangles on each.
        let mut edge_index: HashMap<(u32, u32), usize> = HashMap::new();
        let mut edge_tris: Vec<Vec<(u32, u32)>> = Vec::new();
        let mut poly_edges = std::collections::HashSet::new();
        for p in &t.polygons {
            for k in 0..p.len() {
                let (a, b) = (p[k], p[(k + 1) % p.len()]);
                poly_edges.insert((a.min(b), a.max(b)));
            }
        }
        for (ti, tri) in t.triangles.iter().enumerate() {
            for k in 0..3 {
                let (a, b) = (tri[k], tri[(k + 1) % 3]);
                let key = (a.min(b), a.max(b));
                let e = *edge_index.entry(key).or_insert_with(|| {
                    t.edges.push([key.0, key.1]);
                    edge_tris.push(Vec::new());
                    t.edges.len() - 1
                });
                edge_tris[e].push((ti as u32, tri[(k + 2) % 3]));
            }
        }
        t.border = vec![false; points];
        for (e, tris) in edge_tris.iter().enumerate() {
            if tris.len() == 1 {
                t.border[t.edges[e][0] as usize] = true;
                t.border[t.edges[e][1] as usize] = true;
            }
        }
        t.edge_is_border_of_polygon = t.edges.iter().map(|e| poly_edges.contains(&(e[0], e[1]))).collect();
        for (e, tris) in edge_tris.iter().enumerate() {
            // Bend across each pair on a manifold edge; non-manifold edges bend between consecutive pairs.
            for w in tris.windows(2) {
                let (o2, o3) = (w[0].1, w[1].1);
                if o2 != o3 {
                    t.bends.push([t.edges[e][0], t.edges[e][1], o2, o3]);
                }
            }
        }
        t.neighbours = vec![Vec::new(); points];
        for e in &t.edges {
            t.neighbours[e[0] as usize].push(e[1]);
            t.neighbours[e[1] as usize].push(e[0]);
        }
        for n in &mut t.neighbours {
            n.sort_unstable();
            n.dedup();
        }
        t.point_tris = vec![Vec::new(); points];
        for (ti, tri) in t.triangles.iter().enumerate() {
            for &p in tri {
                t.point_tris[p as usize].push(ti as u32);
            }
        }
        t
    }

    /// The average length of the polygon edges at these positions, a scale for defaults.
    pub fn mean_edge(&self, pos: &[DVec3]) -> f64 {
        let mut sum = 0.0;
        let mut n = 0;
        for (e, &border) in self.edges.iter().zip(&self.edge_is_border_of_polygon) {
            if border {
                let l = (pos[e[0] as usize] - pos[e[1] as usize]).length();
                if l.is_finite() {
                    sum += l;
                    n += 1;
                }
            }
        }
        if n == 0 {
            1.0
        } else {
            sum / n as f64
        }
    }
}

/// Paths of edges between vertices several edges apart, and their lengths on the rest mesh, on levels 3, 9, 27...
/// edges apart until few vertices are left. Returns the flat path list, where each starts, and the rest lengths.
/// Limits between the ends carry tension across a fine mesh in one step; the rest length along the mesh keeps them
/// off folds.
pub fn stretch_paths(t: &Topology, rest: &[DVec3]) -> (Vec<u32>, Vec<u32>, Vec<f64>) {
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;
    #[derive(PartialEq)]
    struct D(f64);
    impl Eq for D {}
    impl PartialOrd for D {
        fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
            Some(self.cmp(o))
        }
    }
    impl Ord for D {
        fn cmp(&self, o: &Self) -> std::cmp::Ordering {
            self.0.total_cmp(&o.0)
        }
    }
    let n = t.points;
    let mut path = Vec::new();
    let mut starts = vec![0u32];
    let mut lengths = Vec::new();
    let mut radius = 3usize;
    let mut hops = vec![u32::MAX; n];
    let mut dist = vec![f64::INFINITY; n];
    let mut prev = vec![u32::MAX; n];
    let mut touched: Vec<u32> = Vec::new();
    while n / (radius * radius) >= 4 {
        // Centres at least `radius` edges apart, picked in order.
        let mut covered = vec![false; n];
        let mut centre = vec![false; n];
        for v in 0..n {
            if covered[v] || t.neighbours[v].is_empty() {
                continue;
            }
            centre[v] = true;
            let mut frontier = vec![v as u32];
            covered[v] = true;
            for _ in 0..radius {
                let mut next = Vec::new();
                for &u in &frontier {
                    for &w in &t.neighbours[u as usize] {
                        if !covered[w as usize] {
                            covered[w as usize] = true;
                            next.push(w);
                        }
                    }
                }
                frontier = next;
            }
        }
        // Each centre to the centres within twice the radius, by shortest path.
        let reach = (radius * 2 + 1) as u32;
        for c in 0..n {
            if !centre[c] {
                continue;
            }
            let mut heap = BinaryHeap::new();
            dist[c] = 0.0;
            hops[c] = 0;
            touched.push(c as u32);
            heap.push(Reverse((D(0.0), c as u32)));
            while let Some(Reverse((D(d), u))) = heap.pop() {
                let u = u as usize;
                if d > dist[u] {
                    continue;
                }
                if centre[u] && u > c {
                    let mut chain = vec![u as u32];
                    let mut at = u;
                    while at != c {
                        at = prev[at] as usize;
                        chain.push(at as u32);
                    }
                    path.extend(chain);
                    starts.push(path.len() as u32);
                    lengths.push(d);
                }
                if hops[u] >= reach {
                    continue;
                }
                for &w in &t.neighbours[u] {
                    let w = w as usize;
                    let nd = d + (rest[u] - rest[w]).length();
                    if nd < dist[w] {
                        if dist[w].is_infinite() {
                            touched.push(w as u32);
                        }
                        dist[w] = nd;
                        hops[w] = hops[u] + 1;
                        prev[w] = u as u32;
                        heap.push(Reverse((D(nd), w as u32)));
                    }
                }
            }
            for &k in &touched {
                dist[k as usize] = f64::INFINITY;
                hops[k as usize] = u32::MAX;
                prev[k as usize] = u32::MAX;
            }
            touched.clear();
        }
        radius *= 3;
    }
    (path, starts, lengths)
}

/// For every vertex reachable from an anchor, its nearest anchor and the shortest distance to it along the rest
/// mesh's edges. Anchors themselves are left out.
pub fn tethers(t: &Topology, rest: &[DVec3], anchors: &[u32]) -> Vec<(u32, u32, f64)> {
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;
    #[derive(PartialEq)]
    struct D(f64);
    impl Eq for D {}
    impl PartialOrd for D {
        fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
            Some(self.cmp(o))
        }
    }
    impl Ord for D {
        fn cmp(&self, o: &Self) -> std::cmp::Ordering {
            self.0.total_cmp(&o.0)
        }
    }
    let n = t.points;
    let mut dist = vec![f64::INFINITY; n];
    let mut from = vec![u32::MAX; n];
    let mut heap = BinaryHeap::new();
    for &a in anchors {
        if (a as usize) < n {
            dist[a as usize] = 0.0;
            from[a as usize] = a;
            heap.push(Reverse((D(0.0), a)));
        }
    }
    while let Some(Reverse((D(d), u))) = heap.pop() {
        let u = u as usize;
        if d > dist[u] {
            continue;
        }
        for &w in &t.neighbours[u] {
            let w = w as usize;
            let nd = d + (rest[u] - rest[w]).length();
            if nd < dist[w] {
                dist[w] = nd;
                from[w] = from[u];
                heap.push(Reverse((D(nd), w as u32)));
            }
        }
    }
    (0..n).filter(|&v| dist[v].is_finite() && dist[v] > 0.0).map(|v| (v as u32, from[v], dist[v])).collect()
}
