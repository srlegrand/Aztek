//! Tests: triangulate.

mod common;
use common::*;

use std::collections::BTreeSet;
use std::f64::consts::PI;

use aztek_geo::triangulate::*;
use aztek_geo::{Index, Mesh};
use glam::{DVec2, DVec3};
use rand_mt::Mt;

fn cross2(a: DVec2, b: DVec2, c: DVec2) -> f64 {
    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
}

/// Segments ab and cd cross at a point interior to both.
fn proper_cross(a: DVec2, b: DVec2, c: DVec2, d: DVec2, eps: f64) -> bool {
    let (d1, d2, d3, d4) = (
        cross2(a, b, c),
        cross2(a, b, d),
        cross2(c, d, a),
        cross2(c, d, b),
    );
    ((d1 > eps && d2 < -eps) || (d1 < -eps && d2 > eps))
        && ((d3 > eps && d4 < -eps) || (d3 < -eps && d4 > eps))
}

fn poly_area(p: &[DVec2]) -> f64 {
    0.5 * (0..p.len())
        .map(|i| p[i].x * p[(i + 1) % p.len()].y - p[(i + 1) % p.len()].x * p[i].y)
        .sum::<f64>()
}

fn uniform(rng: &mut Mt, lo: f64, hi: f64) -> f64 {
    lo + (hi - lo) * (rng.next_u32() as f64 / 4294967296.0)
}

/// Star-shaped around the origin, so simple; counter-clockwise. Sharp spikes when `jag`.
fn random_star(rng: &mut Mt, n: usize, jag: bool) -> Vec<DVec2> {
    let rlo = if jag { 0.15 } else { 0.6 };
    (0..n)
        .map(|i| {
            let t = (i as f64 + 0.5 + uniform(rng, -0.3, 0.3)) * 2.0 * PI / n as f64;
            let rad = if jag && i % 2 == 1 {
                0.2 * uniform(rng, rlo, 1.0)
            } else {
                uniform(rng, rlo, 1.0)
            };
            DVec2::new(rad * t.cos(), rad * t.sin())
        })
        .collect()
}

/// Splits random edges with collinear points, like bevels and loop inserts leave.
fn add_collinear(rng: &mut Mt, input: &[DVec2], extra: usize) -> Vec<DVec2> {
    let mut p = input.to_vec();
    for _ in 0..extra {
        let i = rng.next_u32() as usize % p.len();
        let (a, b) = (p[i], p[(i + 1) % p.len()]);
        let pieces = 1 + rng.next_u32() as usize % 3;
        let mids: Vec<DVec2> = (1..=pieces)
            .map(|j| a.lerp(b, j as f64 / (pieces + 1) as f64))
            .collect();
        p.splice(i + 1..i + 1, mids);
    }
    p
}

/// Comb: deep teeth with collinear points along the spine. Counter-clockwise.
fn comb(teeth: i32) -> Vec<DVec2> {
    let mut p: Vec<DVec2> = (0..teeth * 2).map(|x| DVec2::new(x as f64, 0.0)).collect();
    for k in (0..teeth).rev() {
        p.push(DVec2::new(2.0 * k as f64 + 1.0, 10.0));
        p.push(DVec2::new(2.0 * k as f64, 10.0));
        if k > 0 {
            p.push(DVec2::new(2.0 * k as f64, 1.0));
            p.push(DVec2::new(2.0 * k as f64 - 1.0, 1.0));
        }
    }
    p
}

/// One-face mesh of a 2D polygon placed on a tilted plane, optionally bent out of it.
fn polygon_mesh(poly: &[DVec2], bend: f64) -> Mesh {
    let u = DVec3::new(1.0, 0.3, -0.2).normalize();
    let v = DVec3::new(0.2, 0.5, 1.0).cross(u).normalize();
    let n = u.cross(v);
    let mut m = Mesh::new();
    let pts: Vec<Index> = poly
        .iter()
        .enumerate()
        .map(|(i, p)| {
            m.add_point(
                DVec3::new(500.0, -200.0, 80.0)
                    + u * p.x * 100.0
                    + v * p.y * 100.0
                    + n * (bend * 100.0 * (i as f64 * 1.7).sin()),
            )
        })
        .collect();
    m.add_face(&pts);
    m
}

/// Validates one face's triangles against its 2D outline: count, coverage, orientation, no overlaps.
fn check_triangles(poly: &[DVec2], local: &[Index]) -> bool {
    let n = poly.len();
    if local.len() != 3 * (n - 2) {
        eprintln!("expected {} triangles, got {}", n - 2, local.len() / 3);
        return false;
    }
    let ext = poly
        .iter()
        .fold(0.0f64, |e, p| e.max(p.x.abs()).max(p.y.abs()));
    let eps = 1e-9 * ext * ext;
    let mut segs = BTreeSet::new();
    let mut area = 0.0;
    let mut ok = true;
    for t in local.chunks(3) {
        let a = cross2(
            poly[t[0] as usize],
            poly[t[1] as usize],
            poly[t[2] as usize],
        );
        if a < eps {
            eprintln!("triangle has signed area {a}");
            ok = false;
        }
        area += 0.5 * a;
        for k in 0..3 {
            let (p, q) = (t[k], t[(k + 1) % 3]);
            segs.insert((p.min(q), p.max(q)));
        }
    }
    let list: Vec<(Index, Index)> = segs.into_iter().collect();
    'outer: for i in 0..list.len() {
        if !ok {
            break;
        }
        for j in i + 1..list.len() {
            let ((a, b), (c, d)) = (list[i], list[j]);
            if a == c || a == d || b == c || b == d {
                continue;
            }
            if proper_cross(
                poly[a as usize],
                poly[b as usize],
                poly[c as usize],
                poly[d as usize],
                eps,
            ) {
                eprintln!("edges {a}-{b} and {c}-{d} cross");
                ok = false;
                break 'outer;
            }
        }
    }
    if (area - poly_area(poly)).abs() > 1e-9 * poly_area(poly).abs().max(1.0) {
        eprintln!("area {area} vs polygon {}", poly_area(poly));
        ok = false;
    }
    ok
}

/// Face triangles as local corner indices, via the mesh path that display and export use.
fn mesh_triangles(m: &Mesh, opts: &TriangulateOptions) -> Vec<Index> {
    let mut corners = Vec::new();
    triangulate_face(m, 0, &mut corners, opts);
    let order = m.face_corners(0);
    corners
        .iter()
        .map(|c| order.iter().position(|o| o == c).unwrap() as Index)
        .collect()
}

fn min_angle(poly: &[DVec2], local: &[Index]) -> f64 {
    let mut best = 1e9f64;
    for t in local.chunks(3) {
        for k in 0..3 {
            let (a, b, c) = (
                poly[t[k] as usize],
                poly[t[(k + 1) % 3] as usize],
                poly[t[(k + 2) % 3] as usize],
            );
            best = best.min(
                (b - a)
                    .normalize()
                    .dot((c - a).normalize())
                    .clamp(-1.0, 1.0)
                    .acos(),
            );
        }
    }
    best
}

fn d() -> TriangulateOptions {
    TriangulateOptions::default()
}

#[test]
fn random_simple_polygons() {
    let mut rng = Mt::new(1234);
    let mut checked = 0;
    for iter in 0..400 {
        let n = 4 + rng.next_u32() as usize % 40;
        let mut poly = random_star(&mut rng, n, iter % 3 == 1);
        if iter % 4 == 0 {
            for (i, p) in poly.iter_mut().enumerate() {
                *p = DVec2::new(
                    (i as f64 * 2.0 * PI / n as f64).cos(),
                    (i as f64 * 2.0 * PI / n as f64).sin(),
                );
            }
        }
        if iter % 2 == 0 {
            let extra = 1 + rng.next_u32() as usize % 6;
            poly = add_collinear(&mut rng, &poly, extra);
        }
        for beauty in [false, true] {
            let o = TriangulateOptions { beauty, ..d() };
            let m = polygon_mesh(&poly, 0.0);
            let local = mesh_triangles(&m, &o);
            assert!(
                check_triangles(&poly, &local),
                "iteration {iter}, {} points",
                poly.len()
            );
            assert_eq!(mesh_triangles(&m, &o), local);
            checked += 1;
        }
    }
    assert_eq!(checked, 800);
}

#[test]
fn hard_shapes() {
    let c = comb(6);
    assert!(check_triangles(
        &c,
        &mesh_triangles(&polygon_mesh(&c, 0.0), &d())
    ));

    // Square whose sides carry many collinear points, the bevel n-gon.
    let corners = [
        DVec2::new(-1.0, -1.0),
        DVec2::new(1.0, -1.0),
        DVec2::new(1.0, 1.0),
        DVec2::new(-1.0, 1.0),
    ];
    let mut sq = Vec::new();
    for side in 0..4 {
        for k in 0..7 {
            sq.push(corners[side].lerp(corners[(side + 1) % 4], k as f64 / 7.0));
        }
    }
    let local = mesh_triangles(&polygon_mesh(&sq, 0.0), &d());
    assert!(check_triangles(&sq, &local));
    // A fan from one corner would leave slivers along its two sides.
    assert!(min_angle(&sq, &local) > 5f64.to_radians());
    // Same, starting mid-side so the first corner is flat.
    sq.rotate_left(3);
    assert!(check_triangles(
        &sq,
        &mesh_triangles(&polygon_mesh(&sq, 0.0), &d())
    ));

    // Nearly degenerate: a long thin rectangle with collinear points.
    let thin: Vec<DVec2> = [
        (0.0, 0.0),
        (1.0, 0.0),
        (2.0, 0.0),
        (3.0, 0.0),
        (3.0, 1e-4),
        (1.5, 1e-4),
        (0.0, 1e-4),
    ]
    .iter()
    .map(|&(x, y)| DVec2::new(x, y))
    .collect();
    assert!(check_triangles(
        &thin,
        &mesh_triangles(&polygon_mesh(&thin, 0.0), &d())
    ));

    // Non-planar n-gon: triangles still cover the projected outline.
    let mut rng = Mt::new(7);
    for i in 0..20 {
        let s = random_star(&mut rng, 12, i % 2 == 0);
        let star = add_collinear(&mut rng, &s, 3);
        let m = polygon_mesh(&star, 0.02);
        assert_eq!(mesh_triangles(&m, &d()).len(), 3 * (star.len() - 2));
        let mut cs = Vec::new();
        triangulate_face(&m, 0, &mut cs, &d());
        let n = m.face_normal(0);
        for t in cs.chunks(3) {
            let p = |k: usize| m.position(m.corner_point(t[k]));
            assert!((p(1) - p(0)).cross(p(2) - p(0)).dot(n) > 0.0);
        }
    }

    // Beauty flips improve the smallest angle of a regular-ish polygon fan.
    let oval: Vec<DVec2> = (0..24)
        .map(|i| {
            DVec2::new(
                3.0 * (i as f64 * 2.0 * PI / 24.0).cos(),
                (i as f64 * 2.0 * PI / 24.0).sin(),
            )
        })
        .collect();
    let om = polygon_mesh(&oval, 0.0);
    let plain = TriangulateOptions {
        beauty: false,
        ..d()
    };
    assert!(
        min_angle(&oval, &mesh_triangles(&om, &d()))
            >= min_angle(&oval, &mesh_triangles(&om, &plain)) - 1e-12
    );
}

#[test]
fn quads() {
    // Shorter diagonal 1-3.
    let kite = [
        DVec2::new(0.0, 0.0),
        DVec2::new(2.0, -0.5),
        DVec2::new(4.0, 0.0),
        DVec2::new(2.0, 0.5),
    ];
    assert_eq!(
        mesh_triangles(&polygon_mesh(&kite, 0.0), &d()),
        vec![1, 2, 3, 1, 3, 0]
    );
    let first = TriangulateOptions {
        quads: QuadSplit::FirstCorner,
        ..d()
    };
    assert_eq!(
        mesh_triangles(&polygon_mesh(&kite, 0.0), &first),
        vec![0, 1, 2, 0, 2, 3]
    );

    // Concave dart: the shorter diagonal 1-3 runs outside, so the split takes 0-2.
    let mut dart = vec![
        DVec2::new(0.0, 0.0),
        DVec2::new(2.0, 1.0),
        DVec2::new(0.0, 0.2),
        DVec2::new(-2.0, 1.0),
    ];
    dart.rotate_left(1);
    assert!(check_triangles(
        &dart,
        &mesh_triangles(&polygon_mesh(&dart, 0.0), &d())
    ));

    // Non-planar quad folded: both triangles still face the quad's normal.
    let mut m = Mesh::new();
    let a = m.add_point(DVec3::new(0.0, 0.0, 0.0));
    let b = m.add_point(DVec3::new(100.0, 0.0, 0.0));
    let c = m.add_point(DVec3::new(100.0, 100.0, 30.0));
    let e = m.add_point(DVec3::new(0.0, 100.0, -30.0));
    m.add_face(&[a, b, c, e]);
    let mut corners = Vec::new();
    triangulate_face(&m, 0, &mut corners, &d());
    assert_eq!(corners.len(), 6);
    for t in corners.chunks(3) {
        let p = |k: usize| m.position(m.corner_point(t[k]));
        assert!((p(1) - p(0)).cross(p(2) - p(0)).dot(m.face_normal(0)) > 0.0);
    }
}

#[test]
fn large_ngons() {
    let mut rng = Mt::new(99);
    let circle: Vec<DVec2> = (0..2000)
        .map(|i| {
            DVec2::new(
                (i as f64 * 2.0 * PI / 2000.0).cos(),
                (i as f64 * 2.0 * PI / 2000.0).sin(),
            )
        })
        .collect();
    let s = random_star(&mut rng, 400, true);
    let star = add_collinear(&mut rng, &s, 50);
    for poly in [&circle, &star] {
        let m = polygon_mesh(poly, 0.0);
        let mut corners = Vec::new();
        triangulate_face(&m, 0, &mut corners, &d());
        assert_eq!(corners.len(), 3 * (poly.len() - 2));
    }
    assert!(check_triangles(
        &star,
        &mesh_triangles(&polygon_mesh(&star, 0.0), &d())
    ));
}

#[test]
fn whole_meshes_keep_every_face() {
    use aztek_geo::primitives::*;
    for m in [
        make_cylinder(&CylinderOptions {
            cap_fill: CapFill::Ngon,
            ..Default::default()
        }),
        make_sphere(&SphereOptions::default()),
        make_disc(&DiscOptions {
            subdiv_rings: 0,
            ..Default::default()
        }),
    ] {
        let t = triangulate(&m, &d());
        let budget: Index = (0..m.face_capacity())
            .filter(|&f| m.face_valid(f))
            .map(|f| m.face_size(f) - 2)
            .sum();
        assert_eq!(t.triangle_count(), budget);
        assert!(approx(signed_volume(&m), {
            let mut v = 0.0;
            for c in t.corners.chunks(3) {
                let p = |k: usize| m.position(m.corner_point(c[k]));
                v += p(0).dot(p(1).cross(p(2))) / 6.0;
            }
            v
        }));
    }
}
