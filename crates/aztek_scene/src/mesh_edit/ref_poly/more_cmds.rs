//! The remaining polygon commands: polyCut, polySewEdge, polyClean, polyOutput, polyCheck, polyCompare, blind data, colour
//! edits, moveVertexAlongDirection, symmetricModelling, polyDuplicateAndConnect.
//!
//! Differs from the reference: blind data holds numbers only (booleans and ints as numbers), per component as a mesh attribute, and
//! blindDataType isn't needed first. polyGeoSampler has no lights to sample: it fills the colour set from the mesh's own
//! colours, scaled. polyBlendColor's -bfn numbers aren't checked against the reference.

use std::collections::BTreeMap;

use aztek_geo::ops::*;
use aztek_geo::symmetry::SymmetryMap;
use aztek_geo::{colors, AttrType, Index, Mesh};
use aztek_ops::{fail, objects, ArgType as T, Args, CmdResult, Commands};
use glam::DVec3;
use serde_json::{json, Value};

use super::*;
use crate::components::{self, ComponentMode as K, Ids};
use crate::history::{register_op, HistoryOp};
use crate::mesh_edit::{faces, points, EditResult};

// ---------------------------------------------------------------------------------------------------------------------
// polyCut

/// The plane polyCut's flags give, in object space: a point on it and its normal. +Z before -ro turns it.
fn cut_plane(a: &Args) -> (DVec3, DVec3) {
    let c = vec_flag(a, "cutPlaneCenter", DVec3::ZERO);
    let r = vec_flag(a, "cutPlaneRotate", DVec3::ZERO);
    let turn = glam::DMat3::from_euler(glam::EulerRot::ZYX, r.z, r.y, r.x);
    let n = turn * DVec3::Z;
    let w = object_to_world(a);
    let inv = w.inverse();
    (inv.transform_point3(c), inv.transpose().transform_vector3(n).normalize_or_zero())
}

/// Cuts the faces with a plane. Returns the faces on its positive side, among those cut or not.
fn cut_faces(m: &mut Mesh, sel: &[Index], at: DVec3, n: DVec3) -> Vec<Index> {
    let eps = 1e-7 * m.bounds().size().length().max(1.0);
    let dist = |m: &Mesh, p: Index| (m.position(p) - at).dot(n);
    let mut edges: Vec<Index> = sel.iter().filter(|&&f| m.face_valid(f)).flat_map(|&f| m.face_corners(f)).map(|c| m.corner_edge(c)).collect();
    edges.sort_unstable();
    edges.dedup();
    for e in edges {
        let (d0, d1) = (dist(m, m.edge_point(e, 0)), dist(m, m.edge_point(e, 1)));
        if d0.abs() > eps && d1.abs() > eps && (d0 > 0.0) != (d1 > 0.0) {
            m.split_edge(e, d0 / (d0 - d1));
        }
    }
    // The line the plane leaves across a face runs through its corners on the plane.
    let mut all: Vec<Index> = sel.to_vec();
    for &f in sel {
        if !m.face_valid(f) {
            continue;
        }
        let mut on: Vec<Index> = m.face_points(f).into_iter().filter(|&p| dist(m, p).abs() <= eps).collect();
        if on.len() < 2 {
            continue;
        }
        let dir = n.cross(m.face_normal(f)).normalize_or_zero();
        on.sort_by(|&x, &y| m.position(x).dot(dir).total_cmp(&m.position(y).dot(dir)));
        for pair in on.chunks_exact(2) {
            let owner = all.iter().copied().find(|&g| m.face_valid(g) && m.find_corner(g, pair[0]) != aztek_geo::INVALID && m.find_corner(g, pair[1]) != aztek_geo::INVALID);
            let Some(g) = owner else { continue };
            let (ca, cb) = (m.find_corner(g, pair[0]), m.find_corner(g, pair[1]));
            if m.corner_next(ca) == cb || m.corner_next(cb) == ca {
                continue;
            }
            let made = m.split_face(g, ca, cb);
            all.push(made);
        }
    }
    all.into_iter().filter(|&f| m.face_valid(f) && (m.face_centroid(f) - at).dot(n) > 0.0).collect()
}

fn cut_op(a: &Args, m: &mut Mesh, sel: &Ids, _sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    let (at, n) = cut_plane(a);
    if n == DVec3::ZERO {
        fail!("polyCut: the cut plane has no direction");
    }
    let side = cut_faces(m, sel, at, n);
    if a.bool("deleteFaces") {
        delete_faces(m, &side, None);
        return Ok(EditResult::default());
    }
    if a.bool("extractFaces") {
        let mut part = extract_faces(m, &side);
        part.compact(None);
        let first_face = m.face_capacity();
        let first_point = m.point_capacity();
        m.append(&part);
        delete_faces(m, &side, None);
        let off = vec_flag(a, "extractOffset", DVec3::ZERO);
        for p in first_point..m.point_capacity() {
            if m.point_valid(p) {
                let q = m.position(p) + off;
                m.set_position(p, q);
            }
        }
        return Ok(faces((first_face..m.face_capacity()).filter(|&f| m.face_valid(f)).collect()));
    }
    Ok(EditResult::default())
}

// ---------------------------------------------------------------------------------------------------------------------
// Sew and clean

fn sew_op(a: &Args, m: &mut Mesh, sel: &Ids, sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    let mut pts: Vec<Index> = sel.iter().filter(|&&e| m.edge_valid(e) && m.is_boundary_edge(e)).flat_map(|&e| [m.edge_point(e, 0), m.edge_point(e, 1)]).collect();
    pts.sort_unstable();
    pts.dedup();
    let o = MergeOptions { distance: a.float("tolerance", 0.0).max(1e-9), average: true, symmetry: sym };
    Ok(points(merge_by_distance(m, &pts, &o)))
}

fn clean_op(a: &Args, m: &mut Mesh, _sel: &Ids, _sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    if on(a, "cleanEdges", true) || on(a, "cleanVertices", true) {
        m.remove_all_loose();
    }
    if on(a, "cleanPartialUVMapping", true) {
        // Every face has corners in the UV layer; a face with no UVs set shows as NaNs in files read from elsewhere.
        let uv = m.corner_attrs_mut().get_or_add::<[f32; 2]>(aztek_geo::names::uv(0).as_str());
        for x in uv.iter_mut() {
            if !x[0].is_finite() || !x[1].is_finite() {
                *x = [0.0, 0.0];
            }
        }
    }
    Ok(EditResult::default())
}

// ---------------------------------------------------------------------------------------------------------------------
// Colours

fn color_set_of(a: &Args, m: &Mesh) -> String {
    let n = a.string("baseColorName", &a.string("colorSetName", ""));
    if n.is_empty() {
        colors::current(m)
    } else {
        n
    }
}

fn rgb_hsv(c: [f32; 4]) -> (f64, f64, f64) {
    let (r, g, b) = (c[0] as f64, c[1] as f64, c[2] as f64);
    let (hi, lo) = (r.max(g).max(b), r.min(g).min(b));
    let d = hi - lo;
    let h = if d < 1e-12 {
        0.0
    } else if hi == r {
        60.0 * ((g - b) / d).rem_euclid(6.0)
    } else if hi == g {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    (h, if hi > 0.0 { d / hi } else { 0.0 }, hi)
}

fn hsv_rgb(h: f64, s: f64, v: f64) -> (f64, f64, f64) {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0).rem_euclid(2.0) - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match (h.rem_euclid(360.0) / 60.0) as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    (r + m, g + m, b + m)
}

/// The corners at the points named.
fn corners_at(m: &Mesh, pts: &[Index]) -> Vec<Index> {
    let mut out = Vec::new();
    for &p in pts {
        m.for_each_point_corner(p, |c| out.push(c));
    }
    out
}

fn color_mod_op(a: &Args, m: &mut Mesh, sel: &Ids, _sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    let set = color_set_of(a, m);
    let list = corners_at(m, sel);
    if !colors::has(m, &set) {
        colors::add_white(m, &set);
    }
    let (dh, ms, mv) = (a.float("huev", 0.0), a.float("satv", 1.0), a.float("value", 1.0));
    let vals = colors::values_mut(m, &set);
    for c in list {
        let Some(x) = vals.get_mut(c as usize) else { continue };
        let (h, s, v) = rgb_hsv(*x);
        let (r, g, b) = hsv_rgb(h + dh, (s * ms).clamp(0.0, 1.0), (v * mv).max(0.0));
        *x = [r as f32, g as f32, b as f32, x[3]];
    }
    Ok(EditResult::default())
}

fn color_del_op(a: &Args, m: &mut Mesh, _sel: &Ids, _sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    let set = color_set_of(a, m);
    colors::remove(m, &set);
    Ok(EditResult::default())
}

fn average_color_op(a: &Args, m: &mut Mesh, sel: &Ids, _sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    let set = color_set_of(a, m);
    if !colors::has(m, &set) {
        fail!("polyAverageColor: no colour set {set}");
    }
    for _ in 0..a.int("iterations", 1).max(1) {
        // Each point's colour: the mean of its own corners, then with its neighbours'.
        let vals = colors::values(m, &set).to_vec();
        let mean_at = |p: Index| -> [f32; 4] {
            let mut sum = [0f32; 4];
            let mut k = 0f32;
            m.for_each_point_corner(p, |c| {
                if let Some(v) = vals.get(c as usize) {
                    (0..4).for_each(|i| sum[i] += v[i]);
                    k += 1.0;
                }
            });
            sum.map(|x| x / k.max(1.0))
        };
        let mut next: Vec<(Index, [f32; 4])> = Vec::new();
        for &p in sel {
            if !m.point_valid(p) {
                continue;
            }
            let mut sum = mean_at(p);
            let mut count = 1f32;
            m.for_each_point_edge(p, |e| {
                let v = mean_at(m.edge_other_point(e, p));
                (0..4).for_each(|i| sum[i] += v[i]);
                count += 1.0;
            });
            next.push((p, sum.map(|x| x / count)));
        }
        let lists: Vec<(Vec<Index>, [f32; 4])> = next.iter().map(|(p, c)| (corners_at(m, &[*p]), *c)).collect();
        let out = colors::values_mut(m, &set);
        for (cs, c) in lists {
            for k in cs {
                if let Some(x) = out.get_mut(k as usize) {
                    *x = c;
                }
            }
        }
    }
    Ok(EditResult::default())
}

/// -bfn: 0 custom (A*src + B*dst, with C and D for alpha), 1 add, 2 subtract, 3 multiply, 4 average, 5 replace.
fn blend_color_op(a: &Args, m: &mut Mesh, sel: &Ids, _sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    let (src, dst) = (a.string("baseColorName", ""), a.string("dstColorName", ""));
    let src = if src.is_empty() { colors::current(m) } else { src };
    if !colors::has(m, &src) {
        fail!("polyBlendColor: no colour set {src}");
    }
    let other = a.string("srcColorName", "");
    let other_set = if other.is_empty() { src.clone() } else { other };
    if !colors::has(m, &other_set) {
        fail!("polyBlendColor: no colour set {other_set}");
    }
    let dst = if dst.is_empty() { src.clone() } else { dst };
    if !colors::has(m, &dst) {
        colors::add_white(m, &dst);
    }
    let (x, y) = (colors::values(m, &src).to_vec(), colors::values(m, &other_set).to_vec());
    let w = [a.float("blendWeightA", 0.5) as f32, a.float("blendWeightB", 0.5) as f32, a.float("blendWeightC", 0.0) as f32, a.float("blendWeightD", 1.0) as f32];
    let func = a.int("blendFunc", 0);
    let list = corners_at(m, sel);
    let out = colors::values_mut(m, &dst);
    for c in list {
        let i = c as usize;
        let (Some(p), Some(q), Some(o)) = (x.get(i), y.get(i), out.get_mut(i)) else { continue };
        for k in 0..3 {
            o[k] = match func {
                1 => p[k] + q[k],
                2 => p[k] - q[k],
                3 => p[k] * q[k],
                4 => (p[k] + q[k]) * 0.5,
                5 => q[k],
                _ => w[0] * p[k] + w[1] * q[k],
            };
        }
        o[3] = if func == 0 { w[2] * p[3] + w[3] * q[3] } else { p[3] };
    }
    Ok(EditResult::default())
}

fn geo_sampler_op(a: &Args, m: &mut Mesh, _sel: &Ids, _sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    let set = color_set_of(a, m);
    if !colors::has(m, &set) {
        colors::add_white(m, &set);
    }
    let (sc, off, lo, hi) = (a.float("scaleFactor", 1.0) as f32, 0.0f32, a.float("clampMin", 0.0) as f32, a.float("clampMax", 1.0) as f32);
    for x in colors::values_mut(m, &set).iter_mut() {
        for k in 0..3 {
            x[k] = (x[k] * sc + off).clamp(lo, hi);
        }
    }
    Ok(EditResult::default())
}

// ---------------------------------------------------------------------------------------------------------------------
// Blind data

fn blind_layer(a: &Args) -> String {
    let long = a.string("longDataName", "");
    format!("blind:{}:{}", a.int("typeId", 0), if long.is_empty() { a.string("shortDataName", "") } else { long })
}

fn blind_value(a: &Args) -> Option<f32> {
    a.get("doubleData").and_then(Value::as_f64).or(a.get("intData").and_then(Value::as_f64)).or(a.get("int64Data").and_then(Value::as_f64)).or(a.get("booleanData").map(|v| if v.as_bool().unwrap_or(v.as_i64().unwrap_or(0) != 0) { 1.0 } else { 0.0 })).map(|v| v as f32)
}

fn blind_domain(ty: &str) -> K {
    match ty {
        "vertex" | "vtx" => K::Vertex,
        "edge" => K::Edge,
        _ => K::Face,
    }
}

fn blind_op(a: &Args, m: &mut Mesh, _sel: &Ids, _sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    let layer = blind_layer(a);
    let list: Vec<Index> = a.get("blindIds").and_then(Value::as_array).map(|l| l.iter().filter_map(Value::as_u64).map(|x| x as Index).collect()).unwrap_or_default();
    let v = if a.bool("reset") { f32::NAN } else { a.float("blindValue", f64::NAN) as f32 };
    let attrs = match blind_domain(&a.string("associationType", "face")) {
        K::Vertex => m.point_attrs_mut(),
        K::Edge => m.edge_attrs_mut(),
        _ => m.face_attrs_mut(),
    };
    if !attrs.has(&layer) {
        attrs.add(&layer, AttrType::Float);
        for x in attrs.get_mut::<f32>(&layer).iter_mut() {
            *x = f32::NAN;
        }
    }
    let data = attrs.get_mut::<f32>(&layer);
    for i in list {
        if let Some(x) = data.get_mut(i as usize) {
            *x = v;
        }
    }
    Ok(EditResult::default())
}

fn poly_blind_data(s: &mut Session, a: &Args) -> CmdResult {
    if !a.has("typeId") {
        fail!("polyBlindData: give -id");
    }
    if !a.has("longDataName") && !a.has("shortDataName") {
        fail!("polyBlindData: give -ldn or -sdn");
    }
    if a.has("stringData") || a.has("binaryData") {
        fail!("polyBlindData: only numbers are held here");
    }
    if blind_value(a).is_none() && !a.bool("reset") {
        fail!("polyBlindData: give -dbd, -ind or -bd");
    }
    let kind = blind_domain(&a.string("associationType", "face"));
    // The ids of the domain asked for, carried to the op, which works on faces.
    let (targets, _) = gather(s, a, kind)?;
    let value = blind_value(a);
    run_op_with(s, "polyBlindData", a, &mut |_, node, _, _, args| {
        let list = targets.get(&node).cloned().unwrap_or_default();
        args.flags.insert("blindIds".into(), json!(list));
        if let Some(v) = value {
            args.flags.insert("blindValue".into(), json!(v));
        }
        Ok(())
    })
}

fn poly_query_blind_data(s: &mut Session, a: &Args) -> CmdResult {
    let kind = blind_domain(&a.string("associationType", "face"));
    let (targets, _) = gather(s, a, kind)?;
    let id = a.int("typeId", 0);
    let wanted = {
        let long = a.string("longDataName", "");
        if long.is_empty() { a.string("shortDataName", "") } else { long }
    };
    let sub = a.string("subString", "");
    let prefix = match kind {
        K::Vertex => "vtx",
        K::Edge => "e",
        _ => "f",
    };
    let mut out = Vec::new();
    for (mesh, ids) in targets {
        let Some(m) = crate::modifiers::cage(&mut s.graph, mesh) else { continue };
        let attrs = match kind {
            K::Vertex => m.point_attrs(),
            K::Edge => m.edge_attrs(),
            _ => m.face_attrs(),
        };
        let path = components::display_path(&s.graph, crate::dag::transform_of(&s.graph, mesh));
        for name in attrs.names() {
            let Some(rest) = name.strip_prefix(&format!("blind:{id}:")) else { continue };
            if (!wanted.is_empty() && rest != wanted) || (!sub.is_empty() && !rest.contains(&sub)) {
                continue;
            }
            let data = attrs.get::<f32>(&name);
            for &i in &ids {
                let Some(&v) = data.get(i as usize) else { continue };
                if v.is_nan() {
                    continue;
                }
                if a.bool("showComp") {
                    out.push(json!(format!("{path}.{prefix}[{i}]")));
                }
                out.push(json!(v as f64));
            }
        }
    }
    Ok(Value::Array(out))
}

/// polyColorBlindData: colours the components carrying the data. -mode 0 is binary: the colour where there's data, the
/// no-colour elsewhere; other modes shade between the -mnv and -mxv values.
fn color_blind_op(a: &Args, m: &mut Mesh, _sel: &Ids, _sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    let layer = blind_layer(a);
    let kind = blind_domain(&a.string("associationType", "face"));
    let (hit, none) = (
        [a.float("colorRed", 1.0) as f32, a.float("colorGreen", 0.0) as f32, a.float("colorBlue", 0.0) as f32, 1.0],
        [a.float("noColorRed", 0.2) as f32, a.float("noColorGreen", 0.2) as f32, a.float("noColorBlue", 0.2) as f32, 1.0],
    );
    let (lo, hi) = (a.float("minValue", 0.0) as f32, a.float("maxValue", 1.0) as f32);
    let mode = a.int("mode", 0);
    let set = "blindData".to_string();
    if !colors::has(m, &set) {
        colors::add_white(m, &set);
    }
    let attrs = match kind {
        K::Vertex => m.point_attrs(),
        K::Edge => m.edge_attrs(),
        _ => m.face_attrs(),
    };
    let data: Vec<f32> = attrs.get::<f32>(&layer).to_vec();
    let mut corner_values: Vec<(Index, f32)> = Vec::new();
    for c in 0..m.corner_capacity() {
        if !m.corner_valid(c) {
            continue;
        }
        let i = match kind {
            K::Vertex => m.corner_point(c),
            K::Edge => m.corner_edge(c),
            _ => m.corner_face(c),
        };
        corner_values.push((c, data.get(i as usize).copied().unwrap_or(f32::NAN)));
    }
    let out = colors::values_mut(m, &set);
    for (c, v) in corner_values {
        let Some(x) = out.get_mut(c as usize) else { continue };
        *x = if v.is_nan() {
            none
        } else if mode == 0 {
            hit
        } else {
            let t = ((v - lo) / (hi - lo).max(1e-6)).clamp(0.0, 1.0);
            [none[0] + (hit[0] - none[0]) * t, none[1] + (hit[1] - none[1]) * t, none[2] + (hit[2] - none[2]) * t, 1.0]
        };
    }
    colors::set_current(m, &set);
    Ok(EditResult::default())
}

// ---------------------------------------------------------------------------------------------------------------------
// Asking about meshes

fn mesh_of_arg(s: &mut Session, name: &str, cmd: &str) -> CmdResult<(NodeId, crate::plug::MeshPtr)> {
    let g = &mut s.graph;
    let Some(id) = g.find(name)? else { fail!("{cmd}: no object {name}") };
    let Some(mesh) = components::as_mesh(g, id) else { fail!("{cmd}: {name} is not a mesh") };
    let Some(m) = crate::modifiers::cage(g, mesh) else { fail!("{cmd}: {name} has no geometry") };
    Ok((mesh, m))
}

fn poly_check(s: &mut Session, a: &Args) -> CmdResult {
    let names = objects(s, a);
    let mut problems = 0;
    let mut lines = Vec::new();
    for n in &names {
        let (_, m) = mesh_of_arg(s, n, "polyCheck")?;
        let found = m.validate(u32::MAX);
        problems += found.len();
        lines.extend(found.into_iter().map(|l| format!("{n}: {l}")));
    }
    if let Some(p) = a.get("openFile").and_then(Value::as_str) {
        std::fs::write(p, lines.join("\n")).map_err(|e| aztek_ops::CommandError::new(format!("polyCheck: {p}: {e}")))?;
    }
    for l in &lines {
        log::warn!("{l}");
    }
    Ok(json!(problems))
}

/// polyCompare's bits: 1 vertices, 2 edges, 4 face description, 8 UV sets, 16 UV indices, 32 colour sets, 64 colour
/// indices, 128 user normals.
fn poly_compare(s: &mut Session, a: &Args) -> CmdResult {
    let names = objects(s, a);
    let [x, y] = names.as_slice() else { fail!("polyCompare: name or select two meshes") };
    let (_, p) = mesh_of_arg(s, x, "polyCompare")?;
    let (_, q) = mesh_of_arg(s, y, "polyCompare")?;
    let flags = ["vertices", "edges", "faceDesc", "uvSets", "uvSetIndices", "colorSets", "colorSetIndices", "userNormals"];
    let all = !flags.iter().any(|f| a.bool(f));
    let on = |f: &str| all || a.bool(f);
    let mut bits = 0;
    let (mut mp, mut mq) = ((*p).clone(), (*q).clone());
    mp.compact(None);
    mq.compact(None);
    if on("vertices") && (mp.num_points() != mq.num_points() || (0..mp.num_points()).any(|i| mp.position(i).distance(mq.position(i)) > 1e-5)) {
        bits |= 1;
    }
    let edge_list = |m: &Mesh| -> Vec<(Index, Index)> {
        let mut v: Vec<(Index, Index)> = (0..m.edge_capacity()).filter(|&e| m.edge_valid(e)).map(|e| { let (i, j) = (m.edge_point(e, 0), m.edge_point(e, 1)); (i.min(j), i.max(j)) }).collect();
        v.sort_unstable();
        v
    };
    if on("edges") && edge_list(&mp) != edge_list(&mq) {
        bits |= 2;
    }
    let face_list = |m: &Mesh| -> Vec<Vec<Index>> { (0..m.face_capacity()).filter(|&f| m.face_valid(f)).map(|f| m.face_points(f)).collect() };
    if on("faceDesc") && face_list(&mp) != face_list(&mq) {
        bits |= 4;
    }
    let uv_name = aztek_geo::names::uv(0);
    let uvs = |m: &Mesh| -> Vec<[f32; 2]> { (0..m.corner_capacity()).filter(|&c| m.corner_valid(c)).filter_map(|c| m.corner_attrs().get::<[f32; 2]>(uv_name.as_str()).get(c as usize).copied()).collect() };
    let same = |a: &[[f32; 2]], b: &[[f32; 2]]| a.len() == b.len() && a.iter().zip(b).all(|(p, q)| (p[0] - q[0]).abs() < 1e-5 && (p[1] - q[1]).abs() < 1e-5);
    if on("uvSets") && !same(&uvs(&mp), &uvs(&mq)) {
        bits |= 8;
    }
    if on("uvSetIndices") {
        let (tp, tq) = (crate::uv_ops::topo(&mp), crate::uv_ops::topo(&mq));
        if tp.maps.len() != tq.maps.len() {
            bits |= 16;
        }
    }
    if on("colorSets") || on("colorSetIndices") {
        let (cp, cq) = (colors::sets(&mp), colors::sets(&mq));
        if on("colorSets") && cp != cq {
            bits |= 32;
        }
        if on("colorSetIndices") && cp == cq {
            let differ = cp.iter().any(|n| {
                let (a, b) = (colors::values(&mp, n), colors::values(&mq, n));
                a.len() != b.len() || a.iter().zip(b).any(|(x, y)| (0..4).any(|i| (x[i] - y[i]).abs() > 1e-5))
            });
            if differ {
                bits |= 64;
            }
        }
    }
    Ok(json!(bits))
}

/// polyOutput: the mesh's description as text lines, written to -of when given.
fn poly_output(s: &mut Session, a: &Args) -> CmdResult {
    let names = objects(s, a);
    let flags = ["vert", "edge", "face", "faceNorm", "vertNorm", "triangle", "uvValue", "color", "group"];
    let all = a.bool("allValues") || !flags.iter().any(|f| a.bool(f));
    let on = |f: &str| all || a.bool(f);
    let mut out: Vec<String> = Vec::new();
    for n in &names {
        let (_, m) = mesh_of_arg(s, n, "polyOutput")?;
        out.push(format!("{n}: {} vertices, {} edges, {} faces", m.num_points(), m.num_edges(), m.num_faces()));
        if on("vert") {
            for p in (0..m.point_capacity()).filter(|&p| m.point_valid(p)) {
                let q = m.position(p);
                out.push(format!("vtx {p}: {} {} {}", q.x, q.y, q.z));
            }
        }
        if on("edge") {
            for e in (0..m.edge_capacity()).filter(|&e| m.edge_valid(e)) {
                out.push(format!("edge {e}: {} {}", m.edge_point(e, 0), m.edge_point(e, 1)));
            }
        }
        if on("face") {
            for f in (0..m.face_capacity()).filter(|&f| m.face_valid(f)) {
                out.push(format!("face {f}: {}", m.face_points(f).iter().map(|x| x.to_string()).collect::<Vec<_>>().join(" ")));
            }
        }
        if on("faceNorm") {
            for f in (0..m.face_capacity()).filter(|&f| m.face_valid(f)) {
                let q = m.face_normal(f);
                out.push(format!("faceNormal {f}: {} {} {}", q.x, q.y, q.z));
            }
        }
    }
    if let Some(p) = a.get("outputFile").and_then(Value::as_str) {
        std::fs::write(p, out.join("\n")).map_err(|e| aztek_ops::CommandError::new(format!("polyOutput: {p}: {e}")))?;
    }
    Ok(json!(out))
}

// ---------------------------------------------------------------------------------------------------------------------
// moveVertexAlongDirection

fn move_vertex_along(s: &mut Session, a: &Args) -> CmdResult {
    let (targets, _) = gather(s, a, K::Vertex)?;
    if targets.is_empty() {
        fail!("moveVertexAlongDirection: name or select vertices");
    }
    // Multi-use flags: one value (or vector) per vertex, repeating the last.
    let scalars = |flag: &str| -> Vec<f64> {
        match a.get(flag) {
            Some(Value::Array(l)) => l.iter().filter_map(Value::as_f64).collect(),
            Some(x) => x.as_f64().into_iter().collect(),
            None => Vec::new(),
        }
    };
    let (ns, us, vs, ms) = (scalars("normalDirection"), scalars("uDirection"), scalars("vDirection"), scalars("magnitude"));
    let dirs = scalars("direction");
    let uvn = scalars("uvNormalDirection");
    let pick = |l: &[f64], i: usize| l.get(i).or(l.last()).copied().unwrap_or(0.0);
    let triple = |l: &[f64], i: usize| -> DVec3 {
        if l.len() < 3 {
            return DVec3::ZERO;
        }
        let at = (i * 3).min(l.len() - 3);
        DVec3::new(l[at], l[at + 1], l[at + 2])
    };
    let g = &mut s.graph;
    g.open_chunk("Move Along Direction");
    let r = (|| -> CmdResult<()> {
        for (mesh, ids) in targets {
            let Some(m) = crate::modifiers::cage(g, mesh) else { continue };
            let mut moved: BTreeMap<u32, DVec3> = BTreeMap::new();
            for (k, &p) in ids.iter().enumerate() {
                if !m.point_valid(p) {
                    continue;
                }
                // The point's normal and two tangents along its first edge, as the reference's u and v.
                let normal = m.point_faces(p).iter().map(|&f| m.face_area_vector(f)).sum::<DVec3>().normalize_or_zero();
                let mut first = DVec3::ZERO;
                m.for_each_point_edge(p, |e| {
                    if first == DVec3::ZERO {
                        first = m.position(m.edge_other_point(e, p)) - m.position(p);
                    }
                });
                let u = (first - normal * first.dot(normal)).normalize_or_zero();
                let v = normal.cross(u);
                let mut d = normal * pick(&ns, k) + u * pick(&us, k) + v * pick(&vs, k);
                if !dirs.is_empty() {
                    d += triple(&dirs, k).normalize_or_zero() * if ms.is_empty() { 1.0 } else { pick(&ms, k) };
                }
                if !uvn.is_empty() {
                    let t = triple(&uvn, k);
                    d += u * t.x + v * t.y + normal * t.z;
                }
                moved.insert(p, m.position(p) + d);
            }
            let pp = components::point_provider(g.node(mesh).unwrap()).and_then(|x| x.set_points);
            if let Some(set) = pp {
                let list: Vec<(u32, DVec3)> = moved.into_iter().collect();
                set(g, mesh, &list);
            } else {
                let base = crate::modifiers::edit_plug(g, mesh)?;
                let mut edit = crate::mesh_edit::base_mesh(g, &base);
                for (p, q) in moved {
                    edit.set_position(p, q);
                }
                g.set_value(base, crate::plug::PlugValue::Mesh(Some(std::sync::Arc::new(edit))))?;
            }
        }
        Ok(())
    })();
    g.close_chunk();
    r?;
    Ok(Value::Null)
}

// ---------------------------------------------------------------------------------------------------------------------
// symmetricModelling

fn sym_meshes(s: &Session) -> Vec<String> {
    let g = &s.graph;
    let selected: Vec<NodeId> = g.selection().iter().filter_map(|&id| components::as_mesh(g, id)).collect();
    let list = if selected.is_empty() { g.nodes().into_iter().filter(|&n| components::is_mesh(g, n)).collect() } else { selected };
    list.into_iter().map(|n| g.path(n)).collect()
}

fn symmetric_modelling(s: &mut Session, a: &Args) -> CmdResult {
    if a.bool("query") {
        let e = &s.extras.symmetry;
        if asked(a, "about") {
            return Ok(json!(e.about));
        }
        if asked(a, "axis") {
            return Ok(json!(e.axis));
        }
        if asked(a, "topoSymmetry") {
            return Ok(json!(e.about == "topo"));
        }
        if asked(a, "tolerance") {
            return Ok(json!(e.tolerance));
        }
        if asked(a, "preserveSeam") {
            return Ok(json!(if s.graph.move_opts.preserve_seam { 1 } else { 0 }));
        }
        return Ok(json!(e.on));
    }
    if a.bool("reset") {
        s.extras.symmetry = Default::default();
    }
    if a.has("tolerance") {
        s.extras.symmetry.tolerance = a.float("tolerance", 0.001);
    }
    if a.has("preserveSeam") {
        s.graph.move_opts.preserve_seam = a.int("preserveSeam", 1) != 0;
    }
    if let Some(v) = a.get("axis").and_then(Value::as_str) {
        if !["x", "y", "z"].contains(&v) {
            fail!("symmetricModelling: the axis must be x, y or z");
        }
        s.extras.symmetry.axis = v.to_string();
    }
    if let Some(v) = a.get("about").and_then(Value::as_str) {
        if !["world", "object", "topo"].contains(&v) {
            fail!("symmetricModelling: -about must be world, object or topo");
        }
        s.extras.symmetry.about = v.to_string();
    }
    if a.bool("worldSpace") {
        s.extras.symmetry.about = "world".to_string();
    }
    let want_topo = a.get("topoSymmetry").map(|v| v.as_bool().unwrap_or(v.as_i64().unwrap_or(0) != 0));
    if let Some(on) = want_topo {
        if on {
            s.extras.symmetry.about = "topo".to_string();
            s.extras.symmetry.on = true;
        } else if s.extras.symmetry.about == "topo" {
            s.extras.symmetry.about = "world".to_string();
        }
    }
    if let Some(v) = a.get("symmetry") {
        s.extras.symmetry.on = v.as_bool().unwrap_or(v.as_i64().unwrap_or(0) != 0);
    }
    let meshes = sym_meshes(s);
    let e = (s.extras.symmetry.on, s.extras.symmetry.axis.clone(), s.extras.symmetry.about.clone());
    s.graph.open_chunk("Symmetry");
    let r = (|| -> CmdResult<()> {
        let axis = if e.0 { e.1.as_str() } else { "off" };
        s.run("setSymmetry", json!({"args": meshes, "axis": axis}))?;
        // World goes by the world's axis wherever the object is; object and topology by the object's own.
        let plane = if e.0 && e.2 == "world" { crate::mesh_nodes::SYMMETRY_PLANE_WORLD } else { 0 };
        for name in &meshes {
            let Some(id) = s.graph.find(name)?.and_then(|id| components::as_mesh(&s.graph, id)) else { continue };
            s.graph.set_value(crate::plug::PlugRef::new(id, crate::mesh_nodes::MeshNode::SYMMETRY_PLANE as i32), crate::plug::PlugValue::Int(plane))?;
        }
        if e.0 && e.2 == "topo" {
            // The pairs come from the seam edge selected; without one they stay as they are by position.
            let edge_selected = components::mode(&s.graph) == K::Edge && !components::selected_all(&s.graph, K::Edge).is_empty();
            if want_topo == Some(true) && !edge_selected {
                fail!("symmetricModelling: select an edge on the seam for topological symmetry");
            }
            if edge_selected {
                s.run("symmetryRebuild", json!({"args": meshes, "pairsOnly": true, "topological": true, "axis": e.1}))?;
            }
        }
        Ok(())
    })();
    s.graph.close_chunk();
    r?;
    Ok(Value::Null)
}

/// symmetricModelling's settings.
#[derive(Clone, Debug)]
pub struct SymmetryState {
    pub on: bool,
    pub about: String,
    pub axis: String,
    pub tolerance: f64,
}

impl Default for SymmetryState {
    fn default() -> Self {
        SymmetryState { on: false, about: "world".to_string(), axis: "x".to_string(), tolerance: 0.001 }
    }
}

// ---------------------------------------------------------------------------------------------------------------------

pub(super) fn register_ops() {
    let op = |node_type: &'static str, command: &'static str, label: &'static str, kind: K, run: crate::history::OpFn, doc: &'static str| {
        register_op(HistoryOp { node_type, command, label, kind, run, skip_flags: &[], doc })
    };
    op("polyCut", "polyCut", "Cut Faces", K::Face, cut_op, "Cuts faces with a plane; can delete or pull apart one side.");
    op("polySewEdge", "polySewEdge", "Sew Edges", K::Edge, sew_op, "Joins border edges that lie together.");
    op("polyClean", "polyClean", "Clean", K::Face, clean_op, "Removes edges and vertices no face uses.");
    op("polyColorMod", "polyColorMod", "Modify Colors", K::Vertex, color_mod_op, "Changes colours' hue, saturation and value.");
    op("polyColorDel", "polyColorDel", "Delete Colors", K::Face, color_del_op, "Removes a colour set.");
    op("polyAverageColor", "polyAverageColor", "Average Colors", K::Vertex, average_color_op, "Averages vertices' colours with their neighbours'.");
    op("polyBlendColor", "polyBlendColor", "Blend Colors", K::Vertex, blend_color_op, "Blends two colour sets.");
    op("polyGeoSampler", "polyGeoSampler", "Sample Colors", K::Face, geo_sampler_op, "Fills a colour set from the mesh's own colours, scaled and clamped.");
    op("polyBlindData", "polyBlindData", "Blind Data", K::Face, blind_op, "Numbers stored on components for tools to read.");
    op("polyColorBlindData", "polyColorBlindData", "Color Blind Data", K::Face, color_blind_op, "Shows blind data as colours.");
}

pub(super) fn register(c: &mut Commands<Session>) {
    c.register(ref_cmd("polyCut", "Cut Faces: -pc the plane's centre, -ro its rotation (world), -df deletes the side its normal points to, -ef pulls that side off by -eo. Returns the node.", &[], |s, a| {
        run_op_with(s, "polyCut", a, &mut |g, node, _, _, args| {
            args.flags.insert("worldSpace".into(), json!(true));
            add_world(g, node, args);
            Ok(())
        })
    })
    .flag(fl("cutPlaneCenter", "pc", T::Vec3, Value::Null, "The plane's centre."))
    .flag(fl("cutPlaneRotate", "ro", T::Vec3, Value::Null, "The plane's rotation."))
    .flag(fl("cutPlaneWidth", "pw", T::Float, json!(1.0), "Accepted: the plane has no edge here."))
    .flag(fl("cutPlaneHeight", "ph", T::Float, json!(1.0), "Accepted: the plane has no edge here."))
    .flag(fl("deleteFaces", "df", T::Bool, json!(false), "Delete the side the normal points to."))
    .flag(fl("extractFaces", "ef", T::Bool, json!(false), "Pull that side off."))
    .flag(fl("extractOffset", "eo", T::Vec3, json!([0.0, 0.0, 0.0]), "How far."))
    .flag(fl("onObject", "oo", T::Bool, Value::Null, "Accepted for existing scripts."))
    .flag(fl("worldSpace", "ws", T::Bool, Value::Null, "The plane is in world space (always)."))
    .flag(world_flag()));
    c.register(ref_cmd("polySewEdge", "Sew border edges together where their ends are within -tx. Returns the node.", &[], |s, a| run_op(s, "polySewEdge", a))
        .flag(fl("tolerance", "tx", T::Float, json!(0.0), "How close the ends must be."))
        .flag(fl("texture", "tx2", T::Bool, Value::Null, "Accepted for existing scripts."))
        .flag(fl("worldSpace", "ws", T::Bool, Value::Null, "Accepted for existing scripts.")));
    c.register(ref_cmd("polyClean", "Remove edges and vertices no face uses: -ce, -cv, -cuv, -cpm (each on by default). Returns the node.", &[], |s, a| run_op(s, "polyClean", a))
        .flag(fl("cleanEdges", "ce", T::Bool, json!(true), "Loose edges."))
        .flag(fl("cleanVertices", "cv", T::Bool, json!(true), "Loose vertices."))
        .flag(fl("cleanUVs", "cuv", T::Bool, Value::Null, "Accepted: UVs belong to corners here, so none are loose."))
        .flag(fl("cleanPartialUVMapping", "cpm", T::Bool, json!(true), "UVs that aren't numbers go to 0.")));
    let colour = |c: Cmd| c.flag(fl("baseColorName", "bcn", T::String, Value::Null, "The colour set (the current one)."));
    c.register(colour(ref_cmd("polyColorMod", "Change colours: -hv adds to hue, -sv and -v scale saturation and value. Returns the node.", &[], |s, a| run_op(s, "polyColorMod", a)))
        .flag(fl("huev", "hv", T::Float, json!(0.0), "Degrees of hue."))
        .flag(fl("satv", "sv", T::Float, json!(1.0), "Saturation scale."))
        .flag(fl("value", "v", T::Float, json!(1.0), "Value scale.")));
    c.register(ref_cmd("polyColorDel", "Remove a colour set: -cn (the current one). Returns the node.", &[], |s, a| run_op(s, "polyColorDel", a)).flag(fl("colorSetName", "cn", T::String, Value::Null, "The set.")));
    c.register(colour(ref_cmd("polyAverageColor", "Average vertices' colours with their neighbours': -i passes. Returns the node.", &[], |s, a| run_op(s, "polyAverageColor", a))).flag(fl("iterations", "i", T::Int, json!(1), "How many passes.")));
    c.register(ref_cmd("polyBlendColor", "Blend two colour sets into -dst: -bcn, -src, -bfn (0 by weights -bwa -bwb, 1 add, 2 subtract, 3 multiply, 4 average, 5 replace). Returns the node.", &[], |s, a| run_op(s, "polyBlendColor", a))
        .flag(fl("baseColorName", "bcn", T::String, Value::Null, "One set."))
        .flag(fl("srcColorName", "src", T::String, Value::Null, "The other set."))
        .flag(fl("dstColorName", "dst", T::String, Value::Null, "Where it goes."))
        .flag(fl("blendFunc", "bfn", T::Int, json!(0), "How they combine."))
        .flag(fl("blendWeightA", "bwa", T::Float, json!(0.5), "Weight of the first."))
        .flag(fl("blendWeightB", "bwb", T::Float, json!(0.5), "Weight of the second."))
        .flag(fl("blendWeightC", "bwc", T::Float, json!(0.0), "Alpha weight of the first."))
        .flag(fl("blendWeightD", "bwd", T::Float, json!(1.0), "Alpha weight of the second.")));
    c.register(colour(ref_cmd("polyGeoSampler", "Fill a colour set from the mesh's own colours, scaled by -sf and clamped; there are no lights to sample here. Returns the node.", &[], |s, a| run_op(s, "polyGeoSampler", a)))
        .flag(fl("scaleFactor", "sf", T::Float, json!(1.0), "Scale."))
        .flag(fl("clampMin", "cmn", T::Float, json!(0.0), "Lowest value."))
        .flag(fl("clampMax", "cmx", T::Float, json!(1.0), "Highest value."))
        .flag(fl("computeShadows", "cs", T::Bool, Value::Null, "Accepted for existing scripts."))
        .flag(fl("sampleByFace", "sbf", T::Bool, Value::Null, "Accepted for existing scripts.")));
    c.register(ref_cmd("polyBlindData", "Store a number on components: -id, -at face|vertex|edge, -ldn name, -dbd, -ind or -bd; -rst removes it. Returns the node.", &[], poly_blind_data)
        .flag(fl("typeId", "id", T::Int, Value::Null, "The data's type id."))
        .flag(fl("associationType", "at", T::String, json!("face"), "face, vertex or edge."))
        .flag(fl("longDataName", "ldn", T::String, Value::Null, "The name."))
        .flag(fl("shortDataName", "sdn", T::String, Value::Null, "The short name."))
        .flag(fl("doubleData", "dbd", T::Float, Value::Null, "A number."))
        .flag(fl("intData", "ind", T::Int, Value::Null, "A whole number."))
        .flag(fl("int64Data", "lng", T::Int, Value::Null, "A whole number."))
        .flag(fl("booleanData", "bd", T::Bool, Value::Null, "On or off."))
        .flag(fl("stringData", "sd", T::String, Value::Null, "Refused: numbers only."))
        .flag(fl("binaryData", "bnd", T::String, Value::Null, "Refused: numbers only."))
        .flag(fl("reset", "rst", T::Bool, Value::Null, "Remove the data."))
        .flag(fl("rescale", "rs", T::Bool, Value::Null, "Accepted for existing scripts."))
        .flag(fl("shape", "sh", T::Bool, Value::Null, "Accepted for existing scripts."))
        .flag(fl("blindIds", "", T::List, Value::Null, "The components, worked out from the arguments."))
        .flag(fl("blindValue", "", T::Float, Value::Null, "The value, worked out from the arguments.")));
    c.register(Cmd::new("polyQueryBlindData", "Read blind data back: -id, -at, -ldn or -ss a part of the name, -sc lists each component before its value.", poly_query_blind_data)
        .positional("components or meshes (the selection when none)")
        .flag(fl("typeId", "id", T::Int, Value::Null, "The data's type id."))
        .flag(fl("associationType", "at", T::String, json!("face"), "face, vertex or edge."))
        .flag(fl("longDataName", "ldn", T::String, Value::Null, "The name."))
        .flag(fl("shortDataName", "sdn", T::String, Value::Null, "The short name."))
        .flag(fl("subString", "ss", T::String, Value::Null, "A part of the name."))
        .flag(fl("showComp", "sc", T::Bool, Value::Null, "List the components too."))
        .undoable(false));
    c.register(ref_cmd("polyColorBlindData", "Show blind data as colours in the set \"blindData\": -id, -at, -ldn, -m 0 binary or shaded between -mnv and -mxv; -cr -cg -cb and -ncr -ncg -ncb the colours. Returns the node.", &[], |s, a| run_op(s, "polyColorBlindData", a))
        .flag(fl("typeId", "id", T::Int, Value::Null, "The data's type id."))
        .flag(fl("associationType", "at", T::String, json!("face"), "face, vertex or edge."))
        .flag(fl("longDataName", "ldn", T::String, Value::Null, "The name."))
        .flag(fl("shortDataName", "sdn", T::String, Value::Null, "The short name."))
        .flag(fl("mode", "m", T::Int, json!(0), "0 binary, others shaded."))
        .flag(fl("colorRed", "cr", T::Float, json!(1.0), "The colour."))
        .flag(fl("colorGreen", "cg", T::Float, json!(0.0), "The colour."))
        .flag(fl("colorBlue", "cb", T::Float, json!(0.0), "The colour."))
        .flag(fl("noColorRed", "ncr", T::Float, json!(0.2), "Where there's no data."))
        .flag(fl("noColorGreen", "ncg", T::Float, json!(0.2), "Where there's no data."))
        .flag(fl("noColorBlue", "ncb", T::Float, json!(0.2), "Where there's no data."))
        .flag(fl("minValue", "mnv", T::Float, json!(0.0), "Lowest of the shading."))
        .flag(fl("maxValue", "mxv", T::Float, json!(1.0), "Highest of the shading.")));
    c.register(Cmd::new("polyCheck", "Check a mesh's insides: returns how many problems, each logged; -of writes them to a file.", poly_check)
        .positional("meshes")
        .flag(fl("openFile", "of", T::String, Value::Null, "Write the problems here."))
        .flag(fl("edge", "e", T::Bool, Value::Null, "Accepted: every check runs."))
        .flag(fl("face", "f", T::Bool, Value::Null, "Accepted: every check runs."))
        .flag(fl("faceOffset", "fo", T::Bool, Value::Null, "Accepted: every check runs."))
        .undoable(false));
    c.register(Cmd::new("polyCompare", "Compare two meshes: 0 when alike, else the bits of what differs: 1 vertices, 2 edges, 4 faces, 8 UV sets, 16 UV indices, 32 colour sets, 64 colour values.", poly_compare)
        .positional("two meshes")
        .flag(fl("vertices", "v", T::Bool, Value::Null, "Compare vertices."))
        .flag(fl("edges", "e", T::Bool, Value::Null, "Compare edges."))
        .flag(fl("faceDesc", "fd", T::Bool, Value::Null, "Compare faces."))
        .flag(fl("uvSets", "uv", T::Bool, Value::Null, "Compare UVs."))
        .flag(fl("uvSetIndices", "iuv", T::Bool, Value::Null, "Compare how UVs are shared."))
        .flag(fl("colorSets", "c", T::Bool, Value::Null, "Compare colour sets."))
        .flag(fl("colorSetIndices", "ic", T::Bool, Value::Null, "Compare colour values."))
        .flag(fl("userNormals", "un", T::Bool, Value::Null, "Accepted for existing scripts."))
        .undoable(false));
    c.register(Cmd::new("polyOutput", "A mesh's description as text: -v, -ed, -f, -fn; -a all, the default; -of a file.", poly_output)
        .positional("meshes")
        .flag(fl("allValues", "a", T::Bool, Value::Null, "Everything."))
        .flag(fl("vert", "v", T::Bool, Value::Null, "Vertices."))
        .flag(fl("edge", "ed", T::Bool, Value::Null, "Edges."))
        .flag(fl("face", "f", T::Bool, Value::Null, "Faces."))
        .flag(fl("faceNorm", "fn", T::Bool, Value::Null, "Face normals."))
        .flag(fl("vertNorm", "vn", T::Bool, Value::Null, "Accepted for existing scripts."))
        .flag(fl("triangle", "t", T::Bool, Value::Null, "Accepted for existing scripts."))
        .flag(fl("uvValue", "uv", T::Bool, Value::Null, "Accepted for existing scripts."))
        .flag(fl("color", "c", T::Bool, Value::Null, "Accepted for existing scripts."))
        .flag(fl("group", "g", T::Bool, Value::Null, "Accepted for existing scripts."))
        .flag(fl("outputFile", "of", T::String, Value::Null, "Write here."))
        .undoable(false));
    c.register(Cmd::new("moveVertexAlongDirection", "Move vertices along their normal (-n), tangents (-u, -v), or -d with -m. One value per vertex, the last repeating.", move_vertex_along)
        .positional("vertices (the selection when none)")
        .flag(fl("normalDirection", "n", T::Any, Value::Null, "Along the normal.").multi(1))
        .flag(fl("uDirection", "u", T::Any, Value::Null, "Along the first tangent.").multi(1))
        .flag(fl("vDirection", "v", T::Any, Value::Null, "Along the second tangent.").multi(1))
        .flag(fl("direction", "d", T::Any, Value::Null, "A direction.").multi(3))
        .flag(fl("magnitude", "m", T::Any, Value::Null, "How far along it.").multi(1))
        .flag(fl("uvNormalDirection", "uvn", T::Any, Value::Null, "u, v and normal at once.").multi(3)));
    c.register(query_flags(Cmd::new("symmetricModelling", "Symmetry for component edits: -e -s on, -ax x|y|z, -ab world|object|topo, -ts on from the selected seam edge. On the selected meshes, or all. -q reads each.", symmetric_modelling))
        .flag(fl("symmetry", "s", T::Any, Value::Null, "On or off."))
        .flag(fl("axis", "ax", T::Any, Value::Null, "x, y or z."))
        .flag(fl("about", "ab", T::Any, Value::Null, "world, object or topo. All mirror in the object's axes here; topo pairs by topology."))
        .flag(fl("topoSymmetry", "ts", T::Any, Value::Null, "Pair by topology from the selected seam edge."))
        .flag(fl("worldSpace", "ws", T::Bool, Value::Null, "About world."))
        .flag(fl("tolerance", "tol", T::Any, Value::Null, "How close a pair must be."))
        .flag(fl("reset", "r", T::Bool, Value::Null, "Back to the defaults."))
        .flag(fl("allowPartial", "ap", T::Bool, Value::Null, "Accepted for existing scripts."))
        .flag(fl("preserveSeam", "ps", T::Int, Value::Null, "1 keeps points on the plane there; 0 lets them leave it."))
        .flag(fl("seamTolerance", "st", T::Float, Value::Null, "Accepted for existing scripts."))
        .undoable(false));
    c.register(Cmd::new("polyDuplicateAndConnect", "Duplicate a mesh with its inMesh fed by the original's outMesh, so edits to the first show on the second. Returns the new objects.", |s, a| {
        let names = objects(s, a);
        let mut out = Vec::new();
        s.graph.open_chunk("Duplicate and Connect");
        let r = (|| -> CmdResult<()> {
            for n in &names {
                let made = s.run("duplicate", json!({"args": [n]}))?;
                let Some(new_name) = made.get(0).and_then(Value::as_str).map(str::to_string) else { continue };
                let g = &mut s.graph;
                let (Some(src), Some(dst)) = (g.find(n)?, g.find(&new_name)?) else { continue };
                let (Some(ms), Some(md)) = (components::as_mesh(g, src), components::as_mesh(g, dst)) else { fail!("polyDuplicateAndConnect: {n} is not a mesh") };
                crate::instancing::connect_instance(g, ms, md)?;
                out.push(json!(g.path(dst)));
            }
            Ok(())
        })();
        s.graph.close_chunk();
        r?;
        Ok(Value::Array(out))
    })
    .positional("meshes")
    .flag(fl("removeOriginalFromShaders", "ros", T::Bool, Value::Null, "Accepted for existing scripts."))
    .flag(fl("renameChildren", "rc", T::Bool, Value::Null, "Accepted for existing scripts.")));
    // Under their other names.
    for (name, from, doc) in [("polySmartBevel", "polyBevel3", "Bevel: polyBevel3 under its other name."), ("polyMoveFacetUV", "polyMoveUV", "Move UVs of faces: polyMoveUV under its other name.")] {
        if let Some(mut def) = c.find(from).cloned() {
            def.name = name.to_string();
            def.doc = doc.to_string();
            c.register(def);
        }
    }
}
