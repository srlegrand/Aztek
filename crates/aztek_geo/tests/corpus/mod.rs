//! Adversarial meshes and the invariants every reshaping op must keep, shared by the triangulate, reduce, relax and
//! remesh tests.
#![allow(dead_code)]

use std::collections::BTreeSet;
use std::f64::consts::PI;

use aztek_geo::bvh::MeshBvh;
use aztek_geo::Bounds;
use aztek_geo::normals::{point_normals, set_hard_edges_by_angle, NormalWeight};
use aztek_geo::obj::{read_obj, write_obj, ObjReadOptions, ObjWriteOptions};
use aztek_geo::ops::{bevel_edges, delete_faces, BevelOptions};
use aztek_geo::primitives::*;
use aztek_geo::triangulate::{triangulate, TriangulateOptions};
use aztek_geo::{validate, Index, Mesh};
use glam::{DVec2, DVec3};
use rand_mt::Mt;

pub struct Case {
    pub name: String,
    pub mesh: Mesh,
}

/// std::uniform_real_distribution<double> over std::mt19937, as libstdc++ draws it.
pub fn uniform(rng: &mut Mt, lo: f64, hi: f64) -> f64 {
    let a = rng.next_u32() as f64;
    let b = rng.next_u32() as f64;
    let mut x = (a + b * 4294967296.0) / 18446744073709551616.0;
    if x >= 1.0 {
        x = 1.0 - f64::EPSILON / 2.0;
    }
    lo + (hi - lo) * x
}

pub fn all_faces(m: &Mesh) -> Vec<Index> {
    (0..m.face_capacity()).filter(|&f| m.face_valid(f)).collect()
}

pub fn all_edges(m: &Mesh) -> Vec<Index> {
    (0..m.edge_capacity()).filter(|&e| m.edge_valid(e)).collect()
}

/// A 2D outline as one face on a tilted plane, optionally bent out of it, extruded into a prism when depth > 0.
pub fn polygon(poly: &[DVec2], bend: f64, depth: f64) -> Mesh {
    let u = DVec3::new(1.0, 0.3, -0.2).normalize();
    let v = DVec3::new(0.2, 0.5, 1.0).cross(u).normalize();
    let n = u.cross(v);
    let mut m = Mesh::new();
    let top: Vec<Index> = poly
        .iter()
        .enumerate()
        .map(|(i, p)| {
            m.add_point(
                DVec3::new(30.0, -20.0, 8.0) + u * p.x * 50.0 + v * p.y * 50.0 + n * (bend * 50.0 * (i as f64 * 1.7).sin()),
            )
        })
        .collect();
    if depth <= 0.0 {
        m.add_face(&top);
        return m;
    }
    let bottom: Vec<Index> = top.iter().map(|&t| m.add_point(m.position(t) - n * depth)).collect();
    m.add_face(&top);
    let rev: Vec<Index> = bottom.iter().rev().copied().collect();
    m.add_face(&rev);
    for i in 0..poly.len() {
        let j = (i + 1) % poly.len();
        m.add_face(&[top[j], top[i], bottom[i], bottom[j]]);
    }
    m
}

pub fn star(points: i32, inner: f64) -> Vec<DVec2> {
    (0..2 * points)
        .map(|i| {
            let t = i as f64 * 3.14159265358979323846 / points as f64;
            let r = if i % 2 != 0 { inner } else { 1.0 };
            DVec2::new(r * t.cos(), r * t.sin())
        })
        .collect()
}

pub fn comb(teeth: i32) -> Vec<DVec2> {
    let mut p = Vec::new();
    for x in 0..teeth * 2 {
        p.push(DVec2::new(0.2 * x as f64, 0.0));
    }
    for k in (0..teeth).rev() {
        p.push(DVec2::new(0.2 * (2 * k + 1) as f64, 1.5));
        p.push(DVec2::new(0.2 * (2 * k) as f64, 1.5));
        if k > 0 {
            p.push(DVec2::new(0.2 * (2 * k) as f64, 0.2));
            p.push(DVec2::new(0.2 * (2 * k - 1) as f64, 0.2));
        }
    }
    p
}

pub fn spiral(turns_times8: i32) -> Vec<DVec2> {
    let mut outer = Vec::new();
    let mut inner = Vec::new();
    for i in 0..=turns_times8 {
        let t = i as f64 * PI / 4.0;
        let r = 0.3 + 0.08 * t;
        outer.push(DVec2::new((r + 0.12) * t.cos(), (r + 0.12) * t.sin()));
        inner.push(DVec2::new(r * t.cos(), r * t.sin()));
    }
    outer.extend(inner.into_iter().rev());
    outer
}

/// Square with a square hole, joined by a bridge whose two sides are distinct points at the same positions.
pub fn square_with_hole() -> Vec<DVec2> {
    [
        [-1.0, -1.0], [0.0, -1.0], [0.0, -0.4], [-0.4, -0.4], [-0.4, 0.4], [0.4, 0.4], [0.4, -0.4], [0.0, -0.4],
        [0.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0],
    ]
    .iter()
    .map(|p| DVec2::new(p[0], p[1]))
    .collect()
}

/// Rectangle whose sides carry many collinear points.
pub fn collinear_rect(per_side: i32) -> Vec<DVec2> {
    let c = [DVec2::new(-1.5, -0.5), DVec2::new(1.5, -0.5), DVec2::new(1.5, 0.5), DVec2::new(-1.5, 0.5)];
    let mut p = Vec::new();
    for s in 0..4 {
        for k in 0..per_side {
            let t = k as f64 / per_side as f64;
            p.push(c[s] * (1.0 - t) + c[(s + 1) % 4] * t);
        }
    }
    p
}

pub fn bevelled_cube(segments: i32) -> Mesh {
    let mut m = make_cube(&CubeOptions::default());
    let edges = all_edges(&m);
    bevel_edges(&mut m, &edges, &BevelOptions { offset: 10.0, segments, ..Default::default() });
    m.compact(None);
    m
}

pub fn noisy_sphere(subdiv: i32, amount: f64, seed: u32) -> Mesh {
    let mut m = make_sphere(&SphereOptions { subdiv_axis: subdiv, subdiv_height: subdiv, ..Default::default() });
    let mut rng = Mt::new(seed);
    for p in 0..m.point_capacity() {
        if m.point_valid(p) {
            let s = 1.0 + uniform(&mut rng, -amount, amount);
            m.set_position(p, m.position(p) * s);
        }
    }
    m
}

/// Plane bent into a saddle with jittered heights, so its quads are strongly non-planar.
pub fn twisted_grid(n: i32) -> Mesh {
    let mut m = make_plane(&PlaneOptions { subdiv_x: n, subdiv_z: n, ..Default::default() });
    let mut rng = Mt::new(3);
    for p in 0..m.point_capacity() {
        if m.point_valid(p) {
            let mut q = m.position(p);
            q.y = q.x * q.z / 60.0 + uniform(&mut rng, -6.0, 6.0);
            m.set_position(p, q);
        }
    }
    m
}

pub fn hemisphere() -> Mesh {
    let mut m = make_sphere(&SphereOptions { subdiv_axis: 24, subdiv_height: 16, ..Default::default() });
    let low: Vec<Index> = all_faces(&m).into_iter().filter(|&f| m.face_centroid(f).y < 0.0).collect();
    delete_faces(&mut m, &low, None);
    m.compact(None);
    m
}

pub fn hard_cube() -> Mesh {
    let mut m = make_cube(&CubeOptions { subdiv_x: 3, subdiv_y: 3, subdiv_z: 3, ..Default::default() });
    set_hard_edges_by_angle(&mut m, 30.0, &[]);
    mark_uv_seams(&mut m, "uv0");
    m
}

/// Two quads sharing two edges at a valence-2 point, where both would like the same diagonal.
pub fn tent() -> Mesh {
    let mut m = Mesh::new();
    let a = m.add_point(DVec3::new(-50.0, 0.0, 0.0));
    let b = m.add_point(DVec3::new(50.0, 0.0, 0.0));
    let p = m.add_point(DVec3::new(0.0, 0.0, 10.0));
    let x = m.add_point(DVec3::new(0.0, -150.0, 0.0));
    let y = m.add_point(DVec3::new(0.0, 150.0, 0.0));
    m.add_face(&[a, p, b, x]);
    m.add_face(&[b, p, a, y]);
    m
}

pub fn obj_round_trip(input: &Mesh) -> Mesh {
    let r = read_obj(write_obj(input, &ObjWriteOptions::default()).as_bytes(), &ObjReadOptions::default());
    if r.ok() && !r.objects.is_empty() {
        r.objects.into_iter().next().unwrap().mesh
    } else {
        Mesh::new()
    }
}

/// Every mesh the ops are run on. Closed solids, open sheets, n-gons, slivers, attribute seams.
pub fn all() -> Vec<Case> {
    let mut c: Vec<Case> = Vec::new();
    let mut add = |name: &str, mesh: Mesh| c.push(Case { name: name.into(), mesh });
    add("cube", make_cube(&CubeOptions::default()));
    add("cube4", make_cube(&CubeOptions { subdiv_x: 4, subdiv_y: 4, subdiv_z: 4, ..Default::default() }));
    add("plane1", make_plane(&PlaneOptions::default()));
    add("plane10", make_plane(&PlaneOptions { subdiv_x: 10, subdiv_z: 10, ..Default::default() }));
    add("sphere8x6", make_sphere(&SphereOptions { subdiv_axis: 8, subdiv_height: 6, ..Default::default() }));
    add("sphere32", make_sphere(&SphereOptions { subdiv_axis: 32, subdiv_height: 32, ..Default::default() }));
    add("cylinder", make_cylinder(&CylinderOptions { cap_fill: CapFill::Ngon, ..Default::default() }));
    add("openCylinder", make_cylinder(&CylinderOptions { caps: false, subdiv_height: 4, ..Default::default() }));
    add("cone", make_cone(&ConeOptions { cap_fill: CapFill::Ngon, ..Default::default() }));
    add("torus", make_torus(&TorusOptions::default()));
    add("disc", make_disc(&DiscOptions::default()));
    add("pipe", make_pipe(&PipeOptions::default()));
    add("quadSphere", make_quad_sphere(&QuadSphereOptions::default()));
    add("icoSphere", make_ico_sphere(&IcoSphereOptions::default()));
    add("starPrism", polygon(&star(7, 0.35), 0.0, 20.0));
    add("combPrism", polygon(&comb(5), 0.0, 10.0));
    add("spiralFace", polygon(&spiral(40), 0.0, 0.0));
    add("collinearPrism", polygon(&collinear_rect(6), 0.0, 15.0));
    add("holeFace", polygon(&square_with_hole(), 0.0, 0.0));
    add("bentStar", polygon(&star(9, 0.5), 0.15, 0.0));
    add("bentComb", polygon(&comb(4), 0.1, 0.0));
    add("twistedGrid", twisted_grid(6));
    add("slivers", make_plane(&PlaneOptions { width: 1000.0, depth: 0.5, subdiv_x: 40, subdiv_z: 1, ..Default::default() }));
    add("hardCube", hard_cube());
    add("objSphere", obj_round_trip(&make_sphere(&SphereOptions::default())));
    add("bevelCube", bevelled_cube(3));
    add("noisySphere", noisy_sphere(48, 0.03, 11));
    add("hemisphere", hemisphere());
    add("tent", tent());
    for k in &mut c {
        k.mesh.compact(None);
    }
    c
}

// Invariants

pub fn diagonal(m: &Mesh) -> f64 {
    m.bounds().size().length().max(1e-12)
}

/// Faces of (near) zero area relative to the mesh's size.
pub fn degenerate_faces(m: &Mesh, rel: f64) -> Index {
    let d = diagonal(m);
    let lim = rel * d * d;
    all_faces(m).into_iter().filter(|&f| m.face_area(f) <= lim).count() as Index
}

/// Faces on exactly the same point set as an earlier face.
pub fn duplicate_faces(m: &Mesh) -> Index {
    let mut seen = BTreeSet::new();
    let mut n = 0;
    for f in all_faces(m) {
        let mut p = m.face_points(f);
        p.sort();
        n += !seen.insert(p) as Index;
    }
    n
}

pub fn non_manifold_edges(m: &Mesh) -> Index {
    all_edges(m).into_iter().filter(|&e| m.edge_face_count(e) > 2).count() as Index
}

/// Manifold edges whose two faces run it the same way, i.e. inconsistent winding.
pub fn winding_errors(m: &Mesh) -> Index {
    all_edges(m)
        .into_iter()
        .filter(|&e| {
            if m.edge_face_count(e) != 2 {
                return false;
            }
            let c0 = m.edge_corner(e);
            let c1 = m.corner_radial(c0);
            m.corner_point(c0) == m.corner_point(c1)
        })
        .count() as Index
}

pub fn boundary_edges(m: &Mesh) -> Index {
    all_edges(m).into_iter().filter(|&e| m.edge_face_count(e) == 1).count() as Index
}

pub fn boundary_length(m: &Mesh) -> f64 {
    all_edges(m).into_iter().filter(|&e| m.edge_face_count(e) == 1).map(|e| m.edge_length(e)).sum()
}

/// Largest distance from the mesh's points and triangle centres to the other surface.
pub fn one_sided(from: &Mesh, to: &MeshBvh) -> f64 {
    let mut worst = 0.0f64;
    for p in 0..from.point_capacity() {
        if from.point_valid(p) {
            if let Some(c) = to.closest_point(from.position(p)) {
                worst = worst.max(c.distance);
            }
        }
    }
    let tris = triangulate(from, &TriangulateOptions::default());
    for t in (0..tris.corners.len()).step_by(3) {
        let mut centre = DVec3::ZERO;
        for k in 0..3 {
            centre += from.position(from.corner_point(tris.corners[t + k])) / 3.0;
        }
        if let Some(c) = to.closest_point(centre) {
            worst = worst.max(c.distance);
        }
    }
    worst
}

/// Symmetric Hausdorff distance (sampled at points and face centres) as a fraction of the source's diagonal.
pub fn hausdorff(a: &Mesh, b: &Mesh) -> f64 {
    let ba = MeshBvh::new(a);
    let bb = MeshBvh::new(b);
    one_sided(a, &bb).max(one_sided(b, &ba)) / diagonal(a)
}

/// Faces of out whose normal points against the source surface under their centre. Faces whose centre sits near a
/// sharp source edge are judged against the best of the faces around it, so bevels and corners don't false-positive.
pub fn flipped_faces(out: &Mesh, src: &Mesh, min_dot: f64) -> Index {
    let bvh = MeshBvh::new(src);
    let smooth = point_normals(src, NormalWeight::AreaAngle);
    let d = diagonal(src);
    let mut n = 0;
    for f in all_faces(out) {
        if out.face_area(f) <= 1e-12 * d * d {
            continue;
        }
        let fn_ = out.face_normal(f);
        let Some(c) = bvh.closest_point(out.face_centroid(f)) else { continue };
        // The smooth normal under the centre too, so crumpled source facets (sculpt noise) don't count against it.
        let mut interp = DVec3::ZERO;
        for k in 0..3 {
            let corner = bvh.triangles().corners[c.triangle as usize * 3 + k];
            interp += smooth[src.corner_point(corner) as usize].as_dvec3() * c.bary[k];
        }
        let mut best = fn_.dot(src.face_normal(c.face)).max(if interp.length() > 0.0 {
            fn_.dot(interp.normalize())
        } else {
            -1.0
        });
        if best < min_dot {
            let r = DVec3::splat(1e-3 * d);
            for t in bvh.overlapping(&Bounds { min: c.point - r, max: c.point + r }) {
                best = best.max(fn_.dot(src.face_normal(bvh.triangles().faces[t as usize])));
            }
        }
        n += (best < min_dot) as Index;
    }
    n
}

pub fn triangle_count(m: &Mesh) -> Index {
    all_faces(m).into_iter().map(|f| m.face_size(f) - 2).sum()
}

/// Problems an op introduced: things the output has more of than the input. Empty means clean.
pub fn introduced(input: &Mesh, out: &Mesh) -> Vec<String> {
    let mut p = Vec::new();
    let mut more = |what: &str, a: Index, b: Index| {
        if b > a {
            p.push(format!("{what} {a} -> {b}"));
        }
    };
    more("degenerate faces", degenerate_faces(input, 1e-9), degenerate_faces(out, 1e-9));
    more("duplicate faces", duplicate_faces(input), duplicate_faces(out));
    more("non-manifold edges", non_manifold_edges(input), non_manifold_edges(out));
    more("winding errors", winding_errors(input), winding_errors(out));
    for s in out.validate(validate::STRUCTURE) {
        p.push(format!("invalid: {s}"));
    }
    p
}

pub fn join(v: &[String]) -> String {
    v.iter().map(|s| format!("{s}; ")).collect()
}
