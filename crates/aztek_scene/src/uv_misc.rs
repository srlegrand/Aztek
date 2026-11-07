//! The remaining UV commands: polyMapDel, polyEditUVShell, polyOptUvs, polyUVRectangle, polyUVOverlap, polyUVCoverage,
//! polyUVStackSimilarShells, polyStraightenUVBorder, u3dAutoSeam, u3dOptimize, untangleUV, uvSnapshot, and polyEvaluate's
//! UV bounding boxes.
//!
//! Differs from the reference: corners always hold a UV here, so polyMapDel leaves them at 0 joined up, and a whole mesh ends with
//! one UV a point. polyUVRectangle squares the shell's border on its bounding box rather than working from two edges.
//! polyUVCoverage rasterizes at 512 a side, so it's good to about a pixel's area. uvSnapshot writes PNG, or PPM when asked.

use std::collections::{BTreeMap, BTreeSet};

use aztek_geo::{Index, Mesh, INVALID};
use aztek_ops::{fail, ArgType, Args, CmdResult, CommandDef, Commands};
use glam::DVec2;
use serde_json::{json, Value};

use crate::ref_core::{extend, f};
use crate::plug::NodeId;
use crate::session::Session;
use crate::uv_ops::{self, each, Target, UvTopo, UV};

type Cmd = CommandDef<Session>;
use ArgType::{Bool as B, Float as Fl, Int as I, String as S};

fn uv_at(m: &Mesh, c: Index) -> DVec2 {
    m.corner_attrs().get::<[f32; 2]>(UV).get(c as usize).map_or(DVec2::ZERO, |u| DVec2::new(u[0] as f64, u[1] as f64))
}

fn set_uv_at(m: &mut Mesh, c: Index, p: DVec2) {
    if let Some(x) = m.corner_attrs_mut().get_or_add::<[f32; 2]>(UV).get_mut(c as usize) {
        *x = [p.x as f32, p.y as f32];
    }
}

fn all_faces(m: &Mesh) -> Vec<Index> {
    (0..m.face_capacity()).filter(|&f| m.face_valid(f)).collect()
}

/// The faces a target names, directly or through its maps and edges; the whole mesh when it says so.
fn target_faces(m: &Mesh, t: &Target, tp: &UvTopo) -> Vec<Index> {
    if t.whole {
        return all_faces(m);
    }
    let mut out: BTreeSet<Index> = t.faces.iter().copied().filter(|&f| m.face_valid(f)).collect();
    for &mp in &t.maps {
        if let Some(cs) = tp.maps.get(mp as usize) {
            out.extend(cs.iter().map(|&c| m.corner_face(c)));
        }
    }
    for &e in &t.edges {
        if m.edge_valid(e) {
            m.for_each_edge_corner(e, |c| {
                out.insert(m.corner_face(c));
            });
        }
    }
    out.into_iter().collect()
}

/// The shells the target's faces are in, as lists of faces.
fn shells_of(m: &Mesh, t: &Target, tp: &UvTopo) -> Vec<Vec<Index>> {
    let mut ids: BTreeSet<Index> = BTreeSet::new();
    for f in target_faces(m, t, tp) {
        if let Some(&s) = tp.face_shell.get(f as usize) {
            if s != INVALID {
                ids.insert(s);
            }
        }
    }
    ids.into_iter().filter_map(|s| tp.shells.get(s as usize).cloned()).collect()
}

fn corners_of_faces(m: &Mesh, list: &[Index]) -> Vec<Index> {
    list.iter().filter(|&&f| m.face_valid(f)).flat_map(|&f| m.face_corners(f)).collect()
}

fn bounds_of(m: &Mesh, corners: &[Index]) -> (DVec2, DVec2) {
    let (mut lo, mut hi) = (DVec2::splat(f64::MAX), DVec2::splat(f64::MIN));
    for &c in corners {
        let p = uv_at(m, c);
        lo = lo.min(p);
        hi = hi.max(p);
    }
    (lo, hi)
}

/// Two corners that meet across an edge of the shell and share a UV count as one map; for a map's neighbours in UV space.
fn map_neighbours(m: &Mesh, tp: &UvTopo, faces: &[Index]) -> BTreeMap<Index, BTreeSet<Index>> {
    let mut out: BTreeMap<Index, BTreeSet<Index>> = BTreeMap::new();
    for c in corners_of_faces(m, faces) {
        let (a, b) = (tp.corner_map[c as usize], tp.corner_map[m.corner_next(c) as usize]);
        if a != INVALID && b != INVALID && a != b {
            out.entry(a).or_default().insert(b);
            out.entry(b).or_default().insert(a);
        }
    }
    out
}

/// Border maps of a shell, in order round each loop.
fn border_loops(m: &Mesh, tp: &UvTopo, faces: &[Index]) -> Vec<Vec<Index>> {
    let list: BTreeSet<Index> = faces.iter().copied().collect();
    let mut next: BTreeMap<Index, Index> = BTreeMap::new();
    for c in corners_of_faces(m, faces) {
        let e = m.corner_edge(c);
        // A border: the UVs split here, or nothing lies across.
        let across = m.corner_radial(c);
        let open = across == c || across == INVALID || tp.split.get(e as usize).copied().unwrap_or(true) || !list.contains(&m.corner_face(across));
        if open {
            next.insert(tp.corner_map[c as usize], tp.corner_map[m.corner_next(c) as usize]);
        }
    }
    let mut seen = BTreeSet::new();
    let mut loops = Vec::new();
    for &start in next.keys() {
        if seen.contains(&start) {
            continue;
        }
        let mut one = Vec::new();
        let mut at = start;
        while seen.insert(at) {
            one.push(at);
            match next.get(&at) {
                Some(&n) => at = n,
                None => break,
            }
        }
        loops.push(one);
    }
    loops
}

fn map_uv(m: &Mesh, tp: &UvTopo, mp: Index) -> DVec2 {
    tp.maps.get(mp as usize).and_then(|c| c.first()).map_or(DVec2::ZERO, |&c| uv_at(m, c))
}

fn set_map_uv(m: &mut Mesh, tp: &UvTopo, mp: Index, p: DVec2) {
    if let Some(list) = tp.maps.get(mp as usize) {
        for &c in list {
            set_uv_at(m, c, p);
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Editing

fn poly_map_del(s: &mut Session, a: &Args) -> CmdResult {
    each(s, a, "polyMapDel", "Delete UVs", "polyMapDel", |m, t, tp| {
        let faces = target_faces(m, t, tp);
        let corners = corners_of_faces(m, &faces);
        for c in corners {
            set_uv_at(m, c, DVec2::ZERO);
        }
    })
}

fn poly_edit_uv_shell(s: &mut Session, a: &Args) -> CmdResult {
    if a.bool("query") {
        let mut call = a.clone();
        call.flags.remove("angle");
        return s.run("polyEditUV", json!({"args": a.named(), "query": true}));
    }
    let rel = a.get("relative").and_then(Value::as_bool).unwrap_or(true);
    let (u, v) = (a.get("uValue").and_then(Value::as_f64), a.get("vValue").and_then(Value::as_f64));
    let scale = DVec2::new(a.float("scaleU", 1.0), a.float("scaleV", 1.0));
    let (angle, pivot) = (a.float("rotateRatio", a.float("angle", 0.0)), DVec2::new(a.float("pivotU", 0.0), a.float("pivotV", 0.0)));
    let pivot_given = a.has("pivotU") || a.has("pivotV");
    each(s, a, "polyEditUVShell", "Edit UV Shell", "polyTweakUV", |m, t, tp| {
        for shell in shells_of(m, t, tp) {
            let corners = corners_of_faces(m, &shell);
            let (lo, hi) = bounds_of(m, &corners);
            // Without a pivot a shell turns and scales about its own middle; with absolute values its middle goes there.
            let centre = (lo + hi) * 0.5;
            let at = if pivot_given { pivot } else { centre };
            let off = if rel { DVec2::new(u.unwrap_or(0.0), v.unwrap_or(0.0)) } else { DVec2::new(u.map_or(0.0, |x| x - centre.x), v.map_or(0.0, |x| x - centre.y)) };
            uv_ops::transform(m, &corners, off, scale, angle, at);
        }
    })
}

/// polyOptUvs and u3dOptimize: flatten again keeping each shell's place and size.
fn optimize(s: &mut Session, a: &Args, cmd: &str, label: &str, node: &str) -> CmdResult {
    each(s, a, cmd, label, node, |m, t, tp| {
        let faces = target_faces(m, t, tp);
        uv_ops::unfold(m, &faces);
    })
}

fn poly_uv_rectangle(s: &mut Session, a: &Args) -> CmdResult {
    each(s, a, "polyUVRectangle", "UV Rectangle", "polyUVRectangle", |m, t, tp| {
        for shell in shells_of(m, t, tp) {
            let corners = corners_of_faces(m, &shell);
            let (lo, hi) = bounds_of(m, &corners);
            let size = (hi - lo).max(DVec2::splat(1e-9));
            let loops = border_loops(m, tp, &shell);
            let Some(border) = loops.iter().max_by_key(|l| l.len()) else { continue };
            // The four corners: the border maps nearest each corner of the box; the runs between lie on its sides.
            let box_corners = [lo, DVec2::new(hi.x, lo.y), hi, DVec2::new(lo.x, hi.y)];
            let mut at = [0usize; 4];
            for (k, b) in box_corners.iter().enumerate() {
                at[k] = (0..border.len()).min_by(|&x, &y| map_uv(m, tp, border[x]).distance(*b).total_cmp(&map_uv(m, tp, border[y]).distance(*b))).unwrap_or(0);
            }
            let mut order: Vec<(usize, DVec2)> = at.iter().copied().zip(box_corners).collect();
            order.sort_by_key(|o| o.0);
            let n = border.len();
            let mut moved: Vec<(Index, DVec2)> = Vec::new();
            for k in 0..4 {
                let (i0, p0) = order[k];
                let (i1, p1) = order[(k + 1) % 4];
                let steps = (i1 + n - i0) % n;
                let steps = if steps == 0 { n } else { steps };
                let mut lens = vec![0.0];
                for j in 0..steps {
                    let (x, y) = (border[(i0 + j) % n], border[(i0 + j + 1) % n]);
                    lens.push(lens[j] + map_uv(m, tp, x).distance(map_uv(m, tp, y)));
                }
                let total = lens[steps].max(1e-12);
                for j in 0..steps {
                    moved.push((border[(i0 + j) % n], p0.lerp(p1, lens[j] / total)));
                }
            }
            for (mp, p) in &moved {
                set_map_uv(m, tp, *mp, *p);
            }
            tutte_interior(m, tp, &shell, &moved.iter().map(|x| x.0).collect());
            let _ = size;
        }
    })
}

/// Interior maps to the average of their neighbours, the border held.
fn tutte_interior(m: &mut Mesh, tp: &UvTopo, faces: &[Index], fixed: &BTreeSet<Index>) {
    let near = map_neighbours(m, tp, faces);
    let free: Vec<Index> = near.keys().copied().filter(|x| !fixed.contains(x)).collect();
    let mut at: BTreeMap<Index, DVec2> = near.keys().map(|&x| (x, map_uv(m, tp, x))).collect();
    for _ in 0..500 {
        let mut most = 0f64;
        for &x in &free {
            let list = &near[&x];
            let mean = list.iter().map(|y| at[y]).sum::<DVec2>() / list.len().max(1) as f64;
            most = most.max(mean.distance(at[&x]));
            at.insert(x, mean);
        }
        if most < 1e-7 {
            break;
        }
    }
    for x in free {
        set_map_uv(m, tp, x, at[&x]);
    }
}

fn untangle_uv(s: &mut Session, a: &Args) -> CmdResult {
    let border_shape = a.string("mapBorder", "none");
    if !["none", "circular", "square", "circle"].contains(&border_shape.as_str()) {
        fail!("untangleUV: -mapBorder must be circular, square or none");
    }
    each(s, a, "untangleUV", "Untangle UVs", "polyTweakUV", |m, t, tp| {
        for shell in shells_of(m, t, tp) {
            let corners = corners_of_faces(m, &shell);
            let (lo, hi) = bounds_of(m, &corners);
            let (centre, half) = ((lo + hi) * 0.5, ((hi - lo) * 0.5).max(DVec2::splat(1e-6)));
            let loops = border_loops(m, tp, &shell);
            let Some(border) = loops.iter().max_by_key(|l| l.len()).cloned() else { continue };
            let mut fixed: BTreeSet<Index> = border.iter().copied().collect();
            if border_shape != "none" {
                let n = border.len();
                let mut lens = vec![0.0];
                for j in 0..n {
                    lens.push(lens[j] + map_uv(m, tp, border[j]).distance(map_uv(m, tp, border[(j + 1) % n])));
                }
                let total = lens[n].max(1e-12);
                for j in 0..n {
                    let turn = lens[j] / total * std::f64::consts::TAU;
                    let dir = DVec2::new(turn.cos(), turn.sin());
                    let p = if border_shape == "square" {
                        let k = dir.x.abs().max(dir.y.abs()).max(1e-9);
                        centre + dir / k * half
                    } else {
                        centre + dir * half
                    };
                    set_map_uv(m, tp, border[j], p);
                }
            }
            if a.bool("pinSelected") {
                fixed.extend(t.maps.iter().copied());
            }
            tutte_interior(m, tp, &shell, &fixed);
        }
    })
}

fn stack_shells(s: &mut Session, a: &Args) -> CmdResult {
    let tolerance = a.float("tolerance", 0.01).max(1e-6);
    each(s, a, "polyUVStackSimilarShells", "Stack Shells", "polyTweakUV", |m, t, tp| {
        let shells = shells_of(m, t, tp);
        // Alike: the same counts of faces and points, and the same 3D area.
        let key = |m: &Mesh, sh: &[Index]| -> (usize, usize, f64) {
            let area: f64 = sh.iter().map(|&f| m.face_area(f)).sum();
            let corners = corners_of_faces(m, sh).len();
            (sh.len(), corners, area)
        };
        let mut groups: Vec<(usize, usize, f64, DVec2)> = Vec::new();
        for shell in shells {
            let (fc, cc, area) = key(m, &shell);
            let corners = corners_of_faces(m, &shell);
            let (lo, hi) = bounds_of(m, &corners);
            let centre = (lo + hi) * 0.5;
            if let Some(g) = groups.iter().find(|g| g.0 == fc && g.1 == cc && (g.2 - area).abs() <= tolerance * area.max(1e-9)) {
                uv_ops::transform(m, &corners, g.3 - centre, DVec2::ONE, 0.0, DVec2::ZERO);
            } else {
                groups.push((fc, cc, area, centre));
            }
        }
    })
}

fn straighten_border(s: &mut Session, a: &Args) -> CmdResult {
    let blend = a.float("blendOriginal", 0.0).clamp(0.0, 1.0);
    let curvature = a.float("curvature", 0.0);
    let keep_length = a.get("preserveLength").and_then(Value::as_f64).unwrap_or(1.0).clamp(0.0, 1.0);
    each(s, a, "polyStraightenUVBorder", "Straighten UV Border", "polyTweakUV", |m, t, tp| {
        let shells = shells_of(m, t, tp);
        let picked: BTreeSet<Index> = t.maps.iter().copied().collect();
        for shell in shells {
            for border in border_loops(m, tp, &shell) {
                // The run asked for: picked maps on this border in order, or the whole border's longest side.
                let n = border.len();
                let run: Vec<Index> = if picked.is_empty() {
                    border.clone()
                } else {
                    let Some(start) = (0..n).find(|&i| picked.contains(&border[i]) && !picked.contains(&border[(i + n - 1) % n])).or_else(|| (0..n).find(|&i| picked.contains(&border[i]))) else { continue };
                    (0..n).map(|k| border[(start + k) % n]).take_while(|x| picked.contains(x)).collect()
                };
                if run.len() < 3 {
                    continue;
                }
                let pts: Vec<DVec2> = run.iter().map(|&x| map_uv(m, tp, x)).collect();
                let (p0, p1) = (pts[0], pts[pts.len() - 1]);
                let chord = p1 - p0;
                let mut lens = vec![0.0];
                for w in pts.windows(2) {
                    lens.push(lens.last().unwrap() + w[0].distance(w[1]));
                }
                let total = lens.last().copied().unwrap_or(1.0).max(1e-12);
                let side = DVec2::new(-chord.y, chord.x);
                for (i, &mp) in run.iter().enumerate() {
                    let along = if keep_length > 0.5 { lens[i] / total } else { i as f64 / (run.len() - 1) as f64 };
                    let bow = curvature * (along * std::f64::consts::PI).sin() * 0.25;
                    let q = p0 + chord * along + side * bow;
                    set_map_uv(m, tp, mp, q.lerp(pts[i], blend));
                }
            }
        }
    })
}

/// Cuts seams where faces meet sharply. -s (0 to 1) lowers the angle that counts, from 90 down to 30 degrees.
fn auto_seam(s: &mut Session, a: &Args) -> CmdResult {
    let limit = 90.0 - a.float("splitShells", 0.5).clamp(0.0, 1.0) * 60.0;
    each(s, a, "u3dAutoSeam", "Auto Seams", "polyMapCut", |m, t, tp| {
        let faces: BTreeSet<Index> = target_faces(m, t, tp).into_iter().collect();
        let mut cuts = Vec::new();
        for e in 0..m.edge_capacity() {
            if !m.edge_valid(e) || m.edge_face_count(e) != 2 {
                continue;
            }
            let c = m.edge_corner(e);
            let (f0, f1) = (m.corner_face(c), m.corner_face(m.corner_radial(c)));
            if !faces.contains(&f0) && !faces.contains(&f1) {
                continue;
            }
            if m.face_normal(f0).angle_between(m.face_normal(f1)).to_degrees() >= limit - 1e-6 {
                cuts.push(e);
            }
        }
        uv_ops::cut(m, &cuts);
        // Each shell gets UVs of its own shape.
        let list: Vec<Index> = faces.into_iter().collect();
        uv_ops::unfold(m, &list);
    })
}

// ---------------------------------------------------------------------------------------------------------------------
// Asking

fn triangles_of(m: &Mesh, face: Index) -> Vec<[DVec2; 3]> {
    let c = m.face_corners(face);
    (1..c.len().saturating_sub(1)).map(|k| [uv_at(m, c[0]), uv_at(m, c[k]), uv_at(m, c[k + 1])]).collect()
}

fn cross(a: DVec2, b: DVec2) -> f64 {
    a.x * b.y - a.y * b.x
}

/// Do two triangles overlap with more than a touch? By separating axes along their edges, shrunk a little.
fn tris_overlap(p: &[DVec2; 3], q: &[DVec2; 3]) -> bool {
    let area = |t: &[DVec2; 3]| cross(t[1] - t[0], t[2] - t[0]).abs();
    if area(p) < 1e-14 || area(q) < 1e-14 {
        return false;
    }
    for tri in [p, q] {
        for k in 0..3 {
            let e = tri[(k + 1) % 3] - tri[k];
            let axis = DVec2::new(-e.y, e.x).normalize_or_zero();
            let span = |t: &[DVec2; 3]| {
                let d = [t[0].dot(axis), t[1].dot(axis), t[2].dot(axis)];
                (d[0].min(d[1]).min(d[2]), d[0].max(d[1]).max(d[2]))
            };
            let (a, b) = (span(p), span(q));
            if a.1 <= b.0 + 1e-7 || b.1 <= a.0 + 1e-7 {
                return false;
            }
        }
    }
    true
}

/// Faces named per mesh, for a query.
fn query_faces(s: &mut Session, a: &Args, cmd: &str) -> CmdResult<Vec<(NodeId, crate::plug::MeshPtr, Vec<Index>)>> {
    let targets = uv_ops::targets(s, a, cmd)?;
    let g = &mut s.graph;
    let mut out = Vec::new();
    for (mesh, t) in targets {
        let Some(m) = crate::modifiers::cage(g, mesh) else { continue };
        let tp = uv_ops::topo(&m);
        let faces = target_faces(&m, &t, &tp);
        out.push((mesh, m, faces));
    }
    Ok(out)
}

fn poly_uv_overlap(s: &mut Session, a: &Args) -> CmdResult {
    let want_free = a.bool("nonOverlappingComponents");
    let list = query_faces(s, a, "polyUVOverlap")?;
    let mut out = Vec::new();
    for (mesh, m, faces) in list {
        let tris: Vec<(Index, [DVec2; 3])> = faces.iter().flat_map(|&f| triangles_of(&m, f).into_iter().map(move |t| (f, t))).collect();
        let boxes: Vec<(DVec2, DVec2)> = tris.iter().map(|(_, t)| (t[0].min(t[1]).min(t[2]), t[0].max(t[1]).max(t[2]))).collect();
        let mut hit: BTreeSet<Index> = BTreeSet::new();
        for i in 0..tris.len() {
            for j in i + 1..tris.len() {
                if tris[i].0 == tris[j].0 || boxes[i].1.x < boxes[j].0.x || boxes[j].1.x < boxes[i].0.x || boxes[i].1.y < boxes[j].0.y || boxes[j].1.y < boxes[i].0.y {
                    continue;
                }
                if tris_overlap(&tris[i].1, &tris[j].1) {
                    hit.insert(tris[i].0);
                    hit.insert(tris[j].0);
                }
            }
        }
        let path = crate::components::display_path(&s.graph, crate::dag::transform_of(&s.graph, mesh));
        for f in faces {
            if hit.contains(&f) != want_free {
                out.push(json!(format!("{path}.f[{f}]")));
            }
        }
    }
    Ok(Value::Array(out))
}

fn poly_uv_coverage(s: &mut Session, a: &Args) -> CmdResult {
    let range: Vec<f64> = a.get("uvRange").and_then(Value::as_array).map(|l| l.iter().filter_map(Value::as_f64).collect()).unwrap_or_default();
    let (u0, v0, u1, v1) = if let [a, b, c, d] = range.as_slice() { (*a, *b, *c, *d) } else { (0.0, 0.0, 1.0, 1.0) };
    if u1 <= u0 || v1 <= v0 {
        fail!("polyUVCoverage: the range is empty");
    }
    let list = query_faces(s, a, "polyUVCoverage")?;
    const N: usize = 512;
    let mut out = Vec::new();
    for (_, m, faces) in list {
        let mut grid = vec![false; N * N];
        for f in faces {
            for t in triangles_of(&m, f) {
                let px = |p: DVec2| DVec2::new((p.x - u0) / (u1 - u0) * N as f64, (p.y - v0) / (v1 - v0) * N as f64);
                let (a, b, c) = (px(t[0]), px(t[1]), px(t[2]));
                let area = cross(b - a, c - a);
                if area.abs() < 1e-12 {
                    continue;
                }
                let lo = a.min(b).min(c);
                let hi = a.max(b).max(c);
                for y in (lo.y.floor().max(0.0) as usize)..(hi.y.ceil().min(N as f64) as usize) {
                    for x in (lo.x.floor().max(0.0) as usize)..(hi.x.ceil().min(N as f64) as usize) {
                        let q = DVec2::new(x as f64 + 0.5, y as f64 + 0.5);
                        let (w0, w1, w2) = (cross(b - a, q - a) / area, cross(c - b, q - b) / area, cross(a - c, q - c) / area);
                        if w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0 {
                            grid[y * N + x] = true;
                        }
                    }
                }
            }
        }
        out.push(json!(grid.iter().filter(|x| **x).count() as f64 / (N * N) as f64));
    }
    Ok(Value::Array(out))
}

// ---------------------------------------------------------------------------------------------------------------------
// uvSnapshot

fn crc32(data: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for &b in data {
        c ^= b as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
    }
    !c
}

/// A PNG with no compression: stored deflate blocks, RGB 8 bit.
fn png_bytes(w: usize, h: usize, rgb: &[u8]) -> Vec<u8> {
    let mut raw = Vec::with_capacity((w * 3 + 1) * h);
    for row in rgb.chunks(w * 3) {
        raw.push(0);
        raw.extend_from_slice(row);
    }
    let mut z = vec![0x78, 0x01];
    let mut left = raw.as_slice();
    loop {
        let n = left.len().min(65535);
        let last = n == left.len();
        z.push(last as u8);
        z.extend_from_slice(&(n as u16).to_le_bytes());
        z.extend_from_slice(&(!(n as u16)).to_le_bytes());
        z.extend_from_slice(&left[..n]);
        left = &left[n..];
        if last {
            break;
        }
    }
    let (mut s1, mut s2) = (1u32, 0u32);
    for &b in &raw {
        s1 = (s1 + b as u32) % 65521;
        s2 = (s2 + s1) % 65521;
    }
    z.extend_from_slice(&((s2 << 16) | s1).to_be_bytes());
    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut chunk = |kind: &[u8; 4], data: &[u8]| {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut body = kind.to_vec();
        body.extend_from_slice(data);
        out.extend_from_slice(&body);
        out.extend_from_slice(&crc32(&body).to_be_bytes());
    };
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&(w as u32).to_be_bytes());
    ihdr.extend_from_slice(&(h as u32).to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    chunk(b"IHDR", &ihdr);
    chunk(b"IDAT", &z);
    chunk(b"IEND", &[]);
    out
}

fn uv_snapshot(s: &mut Session, a: &Args) -> CmdResult {
    let Some(path) = a.get("name").and_then(Value::as_str).map(str::to_string) else { fail!("uvSnapshot: give -name") };
    if std::path::Path::new(&path).exists() && !a.bool("overwrite") {
        fail!("uvSnapshot: {path} exists; use -overwrite");
    }
    let (w, h) = (a.int("xResolution", 512).clamp(1, 8192) as usize, a.int("yResolution", 512).clamp(1, 8192) as usize);
    let (u0, u1, v0, v1) = (a.float("uMin", 0.0), a.float("uMax", 1.0), a.float("vMin", 0.0), a.float("vMax", 1.0));
    let ink = [a.int("redColor", 255).clamp(0, 255) as u8, a.int("greenColor", 255).clamp(0, 255) as u8, a.int("blueColor", 255).clamp(0, 255) as u8];
    let mut rgb = vec![0u8; w * h * 3];
    let list = query_faces(s, a, "uvSnapshot")?;
    if list.is_empty() {
        fail!("uvSnapshot: name or select a mesh");
    }
    for (_, m, faces) in &list {
        for &f in faces {
            let corners = m.face_corners(f);
            for k in 0..corners.len() {
                let (p, q) = (uv_at(m, corners[k]), uv_at(m, corners[(k + 1) % corners.len()]));
                let to = |x: DVec2| DVec2::new((x.x - u0) / (u1 - u0) * (w - 1) as f64, (1.0 - (x.y - v0) / (v1 - v0)) * (h - 1) as f64);
                let (p, q) = (to(p), to(q));
                let steps = (p.distance(q).ceil() as usize).max(1);
                for i in 0..=steps {
                    let d = p.lerp(q, i as f64 / steps as f64);
                    let (x, y) = (d.x.round(), d.y.round());
                    if x >= 0.0 && y >= 0.0 && (x as usize) < w && (y as usize) < h {
                        let at = (y as usize * w + x as usize) * 3;
                        rgb[at..at + 3].copy_from_slice(&ink);
                    }
                }
            }
        }
    }
    let ppm = a.get("fileFormat").and_then(Value::as_str).is_some_and(|x| x.eq_ignore_ascii_case("ppm")) || path.to_lowercase().ends_with(".ppm");
    let bytes = if ppm {
        let mut o = format!("P6\n{w} {h}\n255\n").into_bytes();
        o.extend_from_slice(&rgb);
        o
    } else {
        png_bytes(w, h, &rgb)
    };
    std::fs::write(&path, bytes).map_err(|e| aztek_ops::CommandError::new(format!("uvSnapshot: {path}: {e}")))?;
    Ok(json!(path))
}

// ---------------------------------------------------------------------------------------------------------------------

fn poly_flag(c: Cmd) -> Cmd {
    c.positional("UVs, faces or meshes (the selection when none)")
        .flag(f("constructionHistory", "ch", B, "Accepted: a node is made while history is on."))
        .flag(f("name", "n", S, "Accepted for existing scripts."))
        .flag(f("caching", "cch", B, "Accepted for existing scripts."))
        .flag(f("nodeState", "nds", I, "Accepted for existing scripts."))
}

pub fn register(c: &mut Commands<Session>) {
    c.register(poly_flag(Cmd::new("polyMapDel", "Delete UVs of faces. Corners always hold a UV here: they go to 0, joined. Returns the node.", poly_map_del)));
    c.register(
        poly_flag(Cmd::new("polyEditUVShell", "Move whole shells of the UVs named: -u -v, -su -sv, -a degrees, about -pu -pv or each shell's middle. -r 0 puts a shell's middle at -u -v. Returns the node.", poly_edit_uv_shell))
            .flag(f("uValue", "u", Fl, "U offset, or value with -r 0."))
            .flag(f("vValue", "v", Fl, "V offset, or value with -r 0."))
            .flag(f("scaleU", "su", Fl, "U scale."))
            .flag(f("scaleV", "sv", Fl, "V scale."))
            .flag(f("rotateRatio", "rr", Fl, "Accepted: the same as -a."))
            .flag(f("angle", "a", Fl, "Turn, degrees."))
            .flag(f("pivotU", "pu", Fl, "Pivot U."))
            .flag(f("pivotV", "pv", Fl, "Pivot V."))
            .flag(f("relative", "r", B, "Offsets (the default) or values."))
            .flag(f("rotation", "rot", B, "Accepted for existing scripts."))
            .flag(f("query", "q", B, "Return u, v of each UV.")),
    );
    c.register(
        poly_flag(Cmd::new("polyOptUvs", "Relax UVs: flattens each shell again, keeping its place and size. Returns the node.", |s, a| optimize(s, a, "polyOptUvs", "Optimize UVs", "polyOptUvs")))
            .flag(f("iterations", "i", I, "Accepted: the solver decides."))
            .flag(f("stoppingThreshold", "ss", Fl, "Accepted: the solver decides."))
            .flag(f("useScale", "us", B, "Accepted for existing scripts."))
            .flag(f("scale", "s", Fl, "Accepted for existing scripts."))
            .flag(f("areaWeight", "aw", Fl, "Accepted for existing scripts."))
            .flag(f("globalBlend", "gb", Fl, "Accepted for existing scripts."))
            .flag(f("globalMethodBlend", "gmb", Fl, "Accepted for existing scripts."))
            .flag(f("optimizeAxis", "oa", I, "Accepted for existing scripts."))
            .flag(f("pinSelected", "ps", B, "Accepted for existing scripts."))
            .flag(f("pinUvBorder", "pub", B, "Accepted for existing scripts."))
            .flag(f("applyToShell", "as", B, "Accepted: shells are what it works on.")),
    );
    c.register(poly_flag(Cmd::new("polyUVRectangle", "Square a shell's UVs: its border onto its bounding box, the inside evened between. Returns the node.", poly_uv_rectangle)));
    c.register(
        Cmd::new("polyUVOverlap", "Faces whose UVs overlap another's, among those named: -oc; -noc those that don't.", poly_uv_overlap)
            .positional("faces or meshes (the selection when none)")
            .flag(f("overlappingComponents", "oc", B, "The overlapping faces (the default)."))
            .flag(f("nonOverlappingComponents", "noc", B, "The ones that don't overlap."))
            .undoable(false),
    );
    c.register(
        Cmd::new("polyUVCoverage", "How much of a UV range each mesh's faces cover, 0 to 1: -ur u0 v0 u1 v1 (0 0 1 1).", poly_uv_coverage)
            .positional("faces or meshes (the selection when none)")
            .flag(f("uvRange", "ur", ArgType::List, "The range."))
            .undoable(false),
    );
    c.register(poly_flag(Cmd::new("polyUVStackSimilarShells", "Stack shells that are alike (same counts, same 3D area within -tol) on the first of each kind, by their middles. Returns the node.", stack_shells)).flag(f("tolerance", "tol", Fl, "How close the areas must be, as a fraction.")).flag(f("onlyMatch", "om", B, "Accepted for existing scripts.")));
    c.register(
        poly_flag(Cmd::new("polyStraightenUVBorder", "Straighten a run of border UVs between its ends: -bo blends back to where they were, -cur bows the line, -pl keeps their spacing. Whole borders without maps picked. Returns the node.", straighten_border))
            .flag(f("blendOriginal", "bo", Fl, "0 straight, 1 as it was."))
            .flag(f("curvature", "c", Fl, "How much to bow."))
            .flag(f("preserveLength", "pl", Fl, "1 keeps spacing, 0 evens it."))
            .flag(f("gapTolerance", "gt", I, "Accepted for existing scripts.")),
    );
    c.register(poly_flag(Cmd::new("u3dAutoSeam", "Cut seams where faces meet sharply: -s 0 to 1 how readily (90 to 30 degrees). Returns the node.", auto_seam)).flag(f("splitShells", "s", Fl, "How readily.")).flag(f("cutPipes", "p", I, "Accepted for existing scripts.")));
    c.register(
        poly_flag(Cmd::new("u3dOptimize", "Relax UVs, reducing distortion: each shell flattened again, keeping its place and size. Returns the node.", |s, a| optimize(s, a, "u3dOptimize", "Optimize UVs", "Unfold3DUnfold")))
            .flag(f("iterations", "ite", I, "Accepted: the solver decides."))
            .flag(f("power", "pow", I, "Accepted for existing scripts."))
            .flag(f("surfangle", "sa", Fl, "Accepted for existing scripts."))
            .flag(f("borderintersection", "bi", B, "Accepted for existing scripts."))
            .flag(f("triangleflip", "tf", B, "Accepted for existing scripts."))
            .flag(f("mapsize", "ms", I, "Accepted for existing scripts."))
            .flag(f("roomspace", "rs", I, "Accepted for existing scripts.")),
    );
    c.register(
        poly_flag(Cmd::new("untangleUV", "Untangle a shell: -mb circular or square maps its border to that shape, the inside evened so nothing folds. Returns the node.", untangle_uv))
            .flag(f("mapBorder", "mb", S, "circular, square or none."))
            .flag(f("pinSelected", "ps", B, "Hold the UVs named."))
            .flag(f("pinBorder", "pb", B, "Accepted: the border holds unless -mb moves it."))
            .flag(f("pinUnselected", "pu", B, "Accepted for existing scripts."))
            .flag(f("relax", "r", S, "Accepted: the inside is evened uniformly."))
            .flag(f("maxRelaxIterations", "mri", I, "Accepted for existing scripts."))
            .flag(f("relaxTolerance", "rt", Fl, "Accepted for existing scripts."))
            .flag(f("shapeDetail", "sd", Fl, "Accepted for existing scripts.")),
    );
    c.register(
        Cmd::new("uvSnapshot", "Write the UV wireframe to an image: -n file, -xr -yr, -r -g -b the line colour, -um -uM -vm -vM the range, -o overwrites. PNG, or PPM with -ff ppm.", uv_snapshot)
            .positional("faces or meshes (the selection when none)")
            .flag(f("name", "n", S, "The file."))
            .flag(f("xResolution", "xr", I, "Width (512)."))
            .flag(f("yResolution", "yr", I, "Height (512)."))
            .flag(f("redColor", "r", I, "0 to 255."))
            .flag(f("greenColor", "g", I, "0 to 255."))
            .flag(f("blueColor", "b", I, "0 to 255."))
            .flag(f("uMin", "umn", Fl, "Range."))
            .flag(f("uMax", "umx", Fl, "Range."))
            .flag(f("vMin", "vmn", Fl, "Range."))
            .flag(f("vMax", "vmx", Fl, "Range."))
            .flag(f("overwrite", "o", B, "Replace the file."))
            .flag(f("fileFormat", "ff", S, "png, or ppm."))
            .flag(f("antiAliased", "aa", B, "Accepted for existing scripts."))
            .flag(f("entireUVRange", "euv", B, "Accepted for existing scripts."))
            .undoable(false),
    );
    extend(
        c,
        "polyEvaluate",
        vec![f("boundingBox2d", "b2", B, "UV bounds: [[umin, umax], [vmin, vmax]]."), f("boundingBoxComponent2d", "bc2", B, "UV bounds of the components named.")],
        |s, a, prev| {
            if !a.bool("boundingBox2d") && !a.bool("boundingBoxComponent2d") {
                return prev(s, a);
            }
            let list = query_faces(s, a, "polyEvaluate")?;
            let (mut lo, mut hi) = (DVec2::splat(f64::MAX), DVec2::splat(f64::MIN));
            for (_, m, faces) in &list {
                let corners = corners_of_faces(m, faces);
                let (x, y) = bounds_of(m, &corners);
                lo = lo.min(x);
                hi = hi.max(y);
            }
            if lo.x > hi.x {
                fail!("polyEvaluate: no UVs to bound");
            }
            Ok(json!([[lo.x, hi.x], [lo.y, hi.y]]))
        },
    );
}
