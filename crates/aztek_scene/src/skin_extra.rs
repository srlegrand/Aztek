//! The rest of what a skin does in the reference: heat map binding, deformerWeights export and import, Hammer Skin Weights,
//! transferring weight between influences, geometry influences, Substitute Geometry, Remove Unused Influences.

use std::collections::HashMap;

use aztek_geo::bvh::MeshBvh;
use aztek_geo::Mesh;
use aztek_ops::{execute, fail, objects, ArgType, Args, CallSource, CmdResult, Commands};
use glam::{DMat4, DVec3};
use rayon::prelude::*;
use serde_json::{json, Value};

use crate::deform::{self, f, floats_of, set_floats, Cmd, WEIGHTS};
use crate::graph::Graph;
use crate::plug::NodeId;
use crate::session::Session;
use crate::skin::{bind_of, commit_paint, influences, normalize_point, selected_points, skin_geometry, skin_named, SkinBind};

// ---------------------------------------------------------------------------------------------------------------------
// Heat map bind

/// Solves A x = b for the mesh's Laplacian plus a diagonal, by conjugate gradients with the diagonal as preconditioner.
fn solve(adj: &[Vec<(u32, f64)>], diag: &[f64], b: &[f64]) -> Vec<f64> {
    let n = b.len();
    let apply = |x: &[f64]| -> Vec<f64> { (0..n).map(|i| diag[i] * x[i] - adj[i].iter().map(|&(j, w)| w * x[j as usize]).sum::<f64>()).collect() };
    let dot = |a: &[f64], b: &[f64]| a.iter().zip(b).map(|(x, y)| x * y).sum::<f64>();
    let mut x = vec![0.0; n];
    let mut r = b.to_vec();
    let mut z: Vec<f64> = r.iter().zip(diag).map(|(a, d)| a / d.max(1e-12)).collect();
    let mut p = z.clone();
    let mut rz = dot(&r, &z);
    let goal = dot(b, b).sqrt() * 1e-7;
    for _ in 0..(n * 4).clamp(64, 4000) {
        if dot(&r, &r).sqrt() <= goal {
            break;
        }
        let ap = apply(&p);
        let denom = dot(&p, &ap);
        if denom.abs() < 1e-30 {
            break;
        }
        let alpha = rz / denom;
        for i in 0..n {
            x[i] += alpha * p[i];
            r[i] -= alpha * ap[i];
        }
        z = r.iter().zip(diag).map(|(a, d)| a / d.max(1e-12)).collect();
        let next = dot(&r, &z);
        let beta = next / rz.max(1e-30);
        rz = next;
        for i in 0..n {
            p[i] = z[i] + beta * p[i];
        }
    }
    x
}

fn nearest_on_bone(p: DVec3, a: DVec3, b: DVec3) -> DVec3 {
    let ab = b - a;
    let t = if ab.length_squared() < 1e-12 { 0.0 } else { ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) };
    a + ab * t
}

/// Heat map weights: each influence heats the points that see it nearest, and the heat spreads over the mesh's edges to
/// equilibrium, (L + H) w = H p. Because it spreads along the surface, a point takes weight from the bone inside its own
/// part of the mesh, even with another part's bone nearer across a gap. `falloff` from 1 (as solved) down to 0 tightens the
/// result; points go to their strongest `max_inf` and sum to 1.
pub(crate) fn heat_weights(m: &Mesh, geom: &DMat4, bones: &[(DVec3, DVec3)], max_inf: usize, falloff: f64) -> Vec<Vec<(u32, f64)>> {
    let cap = m.point_capacity() as usize;
    let mut out = vec![Vec::new(); cap];
    if bones.is_empty() {
        return out;
    }
    let world_mesh = {
        let mut w = m.clone();
        for p in 0..w.point_capacity() {
            if w.point_valid(p) {
                w.set_position(p, geom.transform_point3(m.position(p)));
            }
        }
        w
    };
    let bvh = MeshBvh::new(&world_mesh);
    let pts: Vec<DVec3> = world_mesh.positions().to_vec();
    let mut adj: Vec<Vec<(u32, f64)>> = vec![Vec::new(); cap];
    let mut laplace = vec![0.0; cap];
    let (mut total_len, mut edges) = (0.0, 0.0);
    for p in 0..cap as u32 {
        if !m.point_valid(p) {
            continue;
        }
        m.for_each_point_edge(p, |e| {
            let q = m.edge_other_point(e, p);
            let len = (pts[p as usize] - pts[q as usize]).length().max(1e-9);
            let w = 1.0 / len;
            adj[p as usize].push((q, w));
            laplace[p as usize] += w;
            total_len += len;
            edges += 1.0;
        });
    }
    let mean_len = if edges > 0.0 { total_len / edges } else { 1.0 };
    let mean_diag = laplace.iter().sum::<f64>() / cap.max(1) as f64;
    // Each point's heat, from its nearest bones it can see.
    let heat: Vec<(f64, Vec<usize>)> = (0..cap as u32)
        .into_par_iter()
        .map(|p| {
            if !m.point_valid(p) {
                return (0.0, Vec::new());
            }
            let at = pts[p as usize];
            let mut by: Vec<(usize, f64, DVec3)> = bones
                .iter()
                .enumerate()
                .map(|(i, (a, b))| {
                    let q = nearest_on_bone(at, *a, *b);
                    (i, (q - at).length(), q)
                })
                .collect();
            by.sort_by(|a, b| a.1.total_cmp(&b.1));
            let seen = by.iter().find(|(_, d, q)| {
                if *d < 1e-9 {
                    return true;
                }
                // A touch off the exact line, which on a regular mesh can run along its edges and through its points.
                let aim = *q + DVec3::new(0.317, 0.571, 0.757) * (*d * 1e-3);
                let dir = (aim - at).normalize_or_zero();
                let start = at + dir * (*d * 1e-3).min(mean_len * 1e-2);
                !bvh.occluded(start, dir, (aim - start).length())
            });
            match seen {
                Some(&(_, d, _)) => {
                    let near: Vec<usize> = by.iter().filter(|x| (x.1 - d).abs() <= d * 1e-4 + 1e-9).map(|x| x.0).collect();
                    (mean_diag * (mean_len / d.max(mean_len * 0.05)).powi(2), near)
                }
                // Hidden from every bone: it takes its weight from around it alone.
                None => (0.0, Vec::new()),
            }
        })
        .collect();
    let diag: Vec<f64> = (0..cap).map(|i| (laplace[i] + heat[i].0).max(1e-9)).collect();
    let solved: Vec<Vec<f64>> = (0..bones.len())
        .into_par_iter()
        .map(|j| {
            let b: Vec<f64> = heat.iter().map(|(h, near)| if near.contains(&j) { h / near.len() as f64 } else { 0.0 }).collect();
            solve(&adj, &diag, &b)
        })
        .collect();
    let power = 1.0 + 2.0 * (1.0 - falloff.clamp(0.0, 1.0));
    for p in 0..cap {
        if !m.point_valid(p as u32) {
            continue;
        }
        let mut w: Vec<(u32, f64)> = solved.iter().enumerate().map(|(j, s)| (j as u32, s[p].max(0.0).powf(power))).filter(|x| x.1 > 1e-6).collect();
        w.sort_by(|a, b| b.1.total_cmp(&a.1));
        w.truncate(max_inf.max(1));
        let sum: f64 = w.iter().map(|x| x.1).sum();
        if sum <= 1e-9 {
            // Nothing reached it, a shell no bone can see: the nearest bone has it.
            let at = pts[p];
            let j = (0..bones.len()).min_by(|&a, &b| (nearest_on_bone(at, bones[a].0, bones[a].1) - at).length_squared().total_cmp(&(nearest_on_bone(at, bones[b].0, bones[b].1) - at).length_squared())).unwrap_or(0);
            out[p] = vec![(j as u32, 1.0)];
            continue;
        }
        for x in w.iter_mut() {
            x.1 /= sum;
        }
        w.sort_by_key(|x| x.0);
        out[p] = w;
    }
    out
}

// ---------------------------------------------------------------------------------------------------------------------
// deformerWeights

#[derive(Default, Debug, Clone)]
struct WeightLayer {
    deformer: String,
    source: String,
    shape: String,
    default: f64,
    points: Vec<(usize, f64)>,
}

#[derive(Default, Debug)]
struct WeightDoc {
    shape_points: Vec<(usize, DVec3)>,
    layers: Vec<WeightLayer>,
}

/// One tag's attributes.
fn xml_attrs(tag: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let mut rest = tag;
    while let Some(eq) = rest.find('=') {
        let key = rest[..eq].trim().rsplit(char::is_whitespace).next().unwrap_or("").to_string();
        let after = &rest[eq + 1..];
        let Some(open) = after.find(['"', '\'']) else { break };
        let quote = after.as_bytes()[open] as char;
        let Some(close) = after[open + 1..].find(quote) else { break };
        out.insert(key, after[open + 1..open + 1 + close].to_string());
        rest = &after[open + close + 2..];
    }
    out
}

fn parse_xml(text: &str) -> CmdResult<WeightDoc> {
    if !text.contains("<deformerWeight") {
        fail!("deformerWeights: not a deformer weights file");
    }
    let mut doc = WeightDoc::default();
    let mut in_shape = false;
    for raw in text.split('<').skip(1) {
        let tag = raw.split('>').next().unwrap_or("");
        let name = tag.split_whitespace().next().unwrap_or("");
        let attrs = xml_attrs(tag);
        match name {
            "shape" => in_shape = true,
            "/shape" => in_shape = false,
            "weights" => {
                in_shape = false;
                doc.layers.push(WeightLayer {
                    deformer: attrs.get("deformer").cloned().unwrap_or_default(),
                    source: attrs.get("source").cloned().unwrap_or_default(),
                    shape: attrs.get("shape").cloned().unwrap_or_default(),
                    default: attrs.get("defaultValue").and_then(|x| x.parse().ok()).unwrap_or(0.0),
                    points: Vec::new(),
                });
            }
            "point" => {
                let index = attrs.get("index").and_then(|x| x.parse().ok()).unwrap_or(0usize);
                let value = attrs.get("value").cloned().unwrap_or_default();
                if in_shape {
                    let v: Vec<f64> = value.split_whitespace().filter_map(|x| x.parse().ok()).collect();
                    if v.len() >= 3 {
                        doc.shape_points.push((index, DVec3::new(v[0], v[1], v[2])));
                    }
                } else if let Some(l) = doc.layers.last_mut() {
                    l.points.push((index, value.trim().parse().unwrap_or(0.0)));
                }
            }
            _ => {}
        }
    }
    Ok(doc)
}

fn parse_json(text: &str) -> CmdResult<WeightDoc> {
    let v: Value = serde_json::from_str(text).map_err(|e| aztek_ops::CommandError::new(format!("deformerWeights: {e}")))?;
    let Some(root) = v.get("deformerWeight") else { fail!("deformerWeights: not a deformer weights file") };
    let mut doc = WeightDoc::default();
    for sh in root.get("shapes").and_then(Value::as_array).into_iter().flatten() {
        for p in sh.get("points").and_then(Value::as_array).into_iter().flatten() {
            let c = |k: usize| p.get("value").and_then(|x| x.get(k)).and_then(Value::as_f64).unwrap_or(0.0);
            doc.shape_points.push((p.get("index").and_then(Value::as_u64).unwrap_or(0) as usize, DVec3::new(c(0), c(1), c(2))));
        }
    }
    for w in root.get("weights").and_then(Value::as_array).into_iter().flatten() {
        let s = |k: &str| w.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        doc.layers.push(WeightLayer {
            deformer: s("deformer"),
            source: s("source"),
            shape: s("shape"),
            default: w.get("defaultValue").and_then(Value::as_f64).unwrap_or(0.0),
            points: w.get("points").and_then(Value::as_array).into_iter().flatten().map(|p| (p.get("index").and_then(Value::as_u64).unwrap_or(0) as usize, p.get("value").and_then(Value::as_f64).unwrap_or(0.0))).collect(),
        });
    }
    Ok(doc)
}

/// The layers a deformer has: each influence of a skin, the weights of anything else.
fn layers_of(g: &Graph, n: NodeId, tolerance: f64, default: f64) -> CmdResult<(NodeId, Vec<WeightLayer>)> {
    let name = g.node(n).map(|x| x.name().to_string()).unwrap_or_default();
    if let Some(b) = g.node(n).filter(|x| x.ty().name == crate::skin::SKIN_TYPE).and_then(|_| bind_of(g, n)) {
        let Some(mesh) = skin_geometry(g, n) else { fail!("deformerWeights: {name} has no geometry") };
        let shape = g.node(mesh).map(|x| x.name().to_string()).unwrap_or_default();
        let mut out = Vec::new();
        for (i, j) in influences(g, n) {
            let points: Vec<(usize, f64)> = b.weights.iter().enumerate().filter_map(|(p, w)| w.iter().find(|x| x.0 == i as u32).map(|x| (p, x.1))).filter(|x| x.1 > tolerance).collect();
            out.push(WeightLayer { deformer: name.clone(), source: g.node(j).map(|x| x.name().to_string()).unwrap_or_default(), shape: shape.clone(), default, points });
        }
        return Ok((mesh, out));
    }
    let Some(mesh) = deform::mesh_fed_by(g, n) else { fail!("deformerWeights: {name} has no geometry") };
    let shape = g.node(mesh).map(|x| x.name().to_string()).unwrap_or_default();
    let points: Vec<(usize, f64)> = match g.node(n).and_then(|x| floats_of(x.data(), WEIGHTS)) {
        Some(w) => w.0.iter().copied().enumerate().filter(|x| x.1 > tolerance).collect(),
        None => g.node(n).and_then(|x| x.data().mesh(deform::IN)).map(|m| (0..m.point_capacity() as usize).filter(|&p| m.point_valid(p as u32)).map(|p| (p, 1.0)).collect()).unwrap_or_default(),
    };
    Ok((mesh, vec![WeightLayer { deformer: name.clone(), source: name, shape, default, points }]))
}

fn write_xml(file: &str, shape: &str, wm: &DMat4, pts: &[(usize, DVec3)], layers: &[WeightLayer], precision: usize) -> String {
    let matrix: Vec<String> = wm.to_cols_array().iter().map(|x| format!("{x}")).collect();
    let mut s = String::from("<?xml version=\"1.0\"?>\n<deformerWeight>\n");
    s += &format!("  <headerInfo fileName=\"{file}\" worldMatrix=\"{}\"/>\n", matrix.join(" "));
    let max = pts.last().map_or(0, |x| x.0);
    s += &format!("  <shape name=\"{shape}\" group=\"0\" stride=\"3\" size=\"{}\" max=\"{max}\">\n", pts.len());
    for (i, p) in pts {
        s += &format!("    <point index=\"{i}\" value=\" {:.*} {:.*} {:.*}\"/>\n", precision, p.x, precision, p.y, precision, p.z);
    }
    s += "  </shape>\n";
    for l in layers {
        let max = l.points.last().map_or(0, |x| x.0);
        s += &format!("  <weights deformer=\"{}\" source=\"{}\" shape=\"{}\" layer=\"0\" defaultValue=\"{:.*}\" size=\"{}\" max=\"{max}\">\n", l.deformer, l.source, l.shape, precision, l.default, l.points.len());
        for (i, v) in &l.points {
            s += &format!("    <point index=\"{i}\" value=\"{:.*}\"/>\n", precision, v);
        }
        s += "  </weights>\n";
    }
    s += "</deformerWeight>\n";
    s
}

fn write_json(file: &str, shape: &str, wm: &DMat4, pts: &[(usize, DVec3)], layers: &[WeightLayer]) -> String {
    let weights: Vec<Value> = layers
        .iter()
        .map(|l| {
            json!({
                "deformer": l.deformer, "source": l.source, "shape": l.shape, "layer": 0, "defaultValue": l.default,
                "size": l.points.len(), "max": l.points.last().map_or(0, |x| x.0),
                "points": l.points.iter().map(|(i, v)| json!({"index": i, "value": v})).collect::<Vec<_>>(),
            })
        })
        .collect();
    let doc = json!({"deformerWeight": {
        "headerInfo": {"fileName": file, "worldMatrix": wm.to_cols_array().to_vec()},
        "shapes": [{"name": shape, "group": 0, "stride": 3, "size": pts.len(), "max": pts.last().map_or(0, |x| x.0),
            "points": pts.iter().map(|(i, p)| json!({"index": i, "value": [p.x, p.y, p.z]})).collect::<Vec<_>>()}],
        "weights": weights,
    }});
    serde_json::to_string_pretty(&doc).unwrap_or_default()
}

/// How each destination point reads the file's: (stored index, share).
fn mapping(m: &Mesh, wm: &DMat4, doc: &WeightDoc, method: &str) -> CmdResult<Vec<Vec<(usize, f64)>>> {
    let cap = m.point_capacity() as usize;
    let nearest_count = match method {
        "index" | "over" => 0,
        "nearest" => 1,
        "barycentric" => 3,
        "bilinear" => 4,
        o => fail!("deformerWeights: method {o} is not index, nearest, barycentric, bilinear or over"),
    };
    if nearest_count == 0 {
        return Ok((0..cap).map(|p| vec![(p, 1.0)]).collect());
    }
    if doc.shape_points.is_empty() {
        fail!("deformerWeights: the file has no point positions for -method {method}");
    }
    Ok((0..cap as u32)
        .into_par_iter()
        .map(|p| {
            if !m.point_valid(p) {
                return Vec::new();
            }
            let at = wm.transform_point3(m.position(p));
            let mut by: Vec<(usize, f64)> = doc.shape_points.iter().map(|(i, q)| (*i, (*q - at).length())).collect();
            by.sort_by(|a, b| a.1.total_cmp(&b.1));
            by.truncate(nearest_count);
            if by[0].1 < 1e-9 {
                return vec![(by[0].0, 1.0)];
            }
            let total: f64 = by.iter().map(|x| 1.0 / x.1).sum();
            by.iter().map(|x| (x.0, 1.0 / x.1 / total)).collect()
        })
        .collect())
}

fn deformers_named(s: &Session, a: &Args) -> CmdResult<Vec<NodeId>> {
    let g = &s.graph;
    let mut list: Vec<String> = match a.get("deformer") {
        Some(Value::String(x)) => vec![x.clone()],
        Some(Value::Array(v)) => v.iter().filter_map(|x| x.as_str().map(str::to_string)).collect(),
        _ => Vec::new(),
    };
    if list.is_empty() {
        if let Some(sh) = a.get("shape").and_then(Value::as_str) {
            let id = crate::rig::find(g, sh, "deformerWeights")?;
            if let Some(m) = crate::components::as_mesh(g, id) {
                list = crate::history::history_of(g, m).into_iter().filter(|&h| g.node(h).is_some_and(|n| crate::rig::is_rig_chain_node(n.ty().name.as_str()))).map(|h| g.path(h)).collect();
            }
        }
    }
    if list.is_empty() {
        fail!("deformerWeights: give -deformer or -shape");
    }
    list.iter().map(|n| crate::rig::find(g, n, "deformerWeights")).collect()
}

fn deformer_weights(s: &mut Session, a: &Args) -> CmdResult {
    let Some(file) = a.named().into_iter().next() else { fail!("deformerWeights: name the file") };
    let path = match a.get("path").and_then(Value::as_str) {
        Some(dir) => std::path::Path::new(dir).join(&file),
        None => std::path::PathBuf::from(&file),
    };
    let json_format = a.get("format").and_then(Value::as_str).map_or_else(|| path.extension().is_some_and(|e| e == "json"), |x| x.eq_ignore_ascii_case("json"));
    let targets = deformers_named(s, a)?;
    let default_value = a.float("defaultValue", 0.0);
    let g = &mut s.graph;
    g.evaluate();
    if a.bool("export") {
        let tolerance = a.float("weightTolerance", 0.0);
        let precision = a.int("weightPrecision", 3).clamp(0, 15) as usize;
        let mut layers = Vec::new();
        let mut shape = None;
        for &n in &targets {
            let (mesh, l) = layers_of(g, n, tolerance, default_value)?;
            shape.get_or_insert(mesh);
            layers.extend(l);
        }
        let mesh = shape.unwrap();
        let Some(m) = crate::modifiers::cage(g, mesh) else { fail!("deformerWeights: no geometry") };
        let xf = crate::dag::transform_of(g, mesh);
        let wm = crate::rig::world(g, xf);
        // Points as they were before deforming, by way of the original where there is one.
        let rest = crate::deform_sets::orig_of(g, mesh).and_then(|o| g.value(&crate::deform::plug(o, crate::mesh_nodes::MeshNode::OUT_MESH)).ok()).and_then(|v| v.as_mesh().cloned());
        let from = rest.as_deref().unwrap_or(&m);
        let pts: Vec<(usize, DVec3)> = (0..from.point_capacity()).filter(|&p| from.point_valid(p)).map(|p| (p as usize, wm.transform_point3(from.position(p)))).collect();
        let shape_name = g.node(mesh).map(|x| x.name().to_string()).unwrap_or_default();
        let name = path.file_name().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
        let text = if json_format { write_json(&name, &shape_name, &wm, &pts, &layers) } else { write_xml(&name, &shape_name, &wm, &pts, &layers, precision) };
        if let Err(e) = std::fs::write(&path, text) {
            fail!("deformerWeights: can't write {}: {e}", path.display());
        }
        return Ok(json!(path.to_string_lossy()));
    }
    if !a.bool("import") {
        fail!("deformerWeights: give -ex or -im");
    }
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) => fail!("deformerWeights: can't read {}: {e}", path.display()),
    };
    let doc = if json_format { parse_json(&text)? } else { parse_xml(&text)? };
    let method = a.string("method", "index");
    let ignore_name = a.bool("ignoreName");
    g.open_chunk("Deformer Weights");
    let r = (|| -> CmdResult<()> {
        for &n in &targets {
            let name = g.node(n).map(|x| x.name().to_string()).unwrap_or_default();
            let of_this: Vec<&WeightLayer> = doc.layers.iter().filter(|l| ignore_name || l.deformer == name).collect();
            if of_this.is_empty() {
                fail!("deformerWeights: the file has nothing for {name}");
            }
            let is_skin = g.node(n).is_some_and(|x| x.ty().name == crate::skin::SKIN_TYPE);
            let mesh = if is_skin { skin_geometry(g, n) } else { deform::mesh_fed_by(g, n) };
            let Some(mesh) = mesh else { fail!("deformerWeights: {name} has no geometry") };
            let Some(m) = crate::modifiers::cage(g, mesh) else { fail!("deformerWeights: no geometry") };
            let xf = crate::dag::transform_of(g, mesh);
            let wm = crate::rig::world(g, xf);
            let map = mapping(&m, &wm, &doc, &method)?;
            let over = method == "over";
            let value_at = |l: &WeightLayer, p: usize, lookup: &HashMap<usize, f64>| -> Option<f64> {
                let def = if a.has("defaultValue") { default_value } else { l.default };
                let mut sum = 0.0;
                let mut any = false;
                for &(i, share) in &map[p] {
                    match lookup.get(&i) {
                        Some(v) => {
                            sum += v * share;
                            any = true;
                        }
                        None => sum += def * share,
                    }
                }
                (any || !over).then_some(sum)
            };
            if is_skin {
                let mut b: SkinBind = (*bind_of(g, n).unwrap_or_default()).clone();
                let list = influences(g, n);
                let known: Vec<(u32, String)> = list.iter().map(|&(i, j)| (i as u32, g.node(j).map(|x| x.name().to_string()).unwrap_or_default())).collect();
                for (k, l) in of_this.iter().enumerate() {
                    let inf = if ignore_name { known.get(k).map(|x| x.0) } else { known.iter().find(|x| x.1 == l.source).map(|x| x.0) };
                    let Some(inf) = inf else { continue };
                    let lookup: HashMap<usize, f64> = l.points.iter().copied().collect();
                    for p in 0..b.weights.len() {
                        let Some(v) = value_at(l, p, &lookup) else { continue };
                        b.weights[p].retain(|x| x.0 != inf);
                        if v > 1e-12 {
                            b.weights[p].push((inf, v));
                            b.weights[p].sort_by_key(|x| x.0);
                        }
                    }
                }
                commit_paint(g, n, b)?;
            } else {
                let l = of_this[0];
                let lookup: HashMap<usize, f64> = l.points.iter().copied().collect();
                let size = m.point_capacity() as usize;
                let mut w = g.node(n).and_then(|x| floats_of(x.data(), WEIGHTS)).map(|x| x.0.clone()).unwrap_or_else(|| vec![1.0; size]);
                w.resize(size, 0.0);
                for (p, slot) in w.iter_mut().enumerate() {
                    if let Some(v) = value_at(l, p, &lookup) {
                        *slot = v;
                    }
                }
                set_floats(g, n, WEIGHTS, w)?;
            }
        }
        Ok(())
    })();
    g.close_chunk();
    r?;
    Ok(json!(path.to_string_lossy()))
}

// ---------------------------------------------------------------------------------------------------------------------
// Editing weights

/// Hammer Skin Weights: each point takes the average of its neighbours outside what is being hammered; a point with none
/// outside takes the average of all of them.
fn hammer(s: &mut Session, a: &Args) -> CmdResult {
    let names = objects(s, a);
    let g = &mut s.graph;
    let mut targets = crate::skin::points_named(g, &names);
    if targets.iter().all(|t| t.1.is_empty()) {
        targets.clear();
        for n in &names {
            let id = crate::rig::find(g, n.split('.').next().unwrap_or(n), "weightHammerVerts")?;
            if let Some(m) = crate::components::as_mesh(g, id) {
                let ids = selected_points(g, m);
                targets.push((m, ids));
            }
        }
    }
    for (mesh, ids) in targets {
        let Some(sk) = crate::skin::skin_of(g, mesh) else { fail!("weightHammerVerts: {} has no skin", g.path(mesh)) };
        let Some(m) = crate::modifiers::cage(g, mesh) else { continue };
        let mut b = (*bind_of(g, sk).unwrap_or_default()).clone();
        let old = b.weights.clone();
        let locks = SkinBind { locks: b.locks.clone(), ..Default::default() };
        for &p in &ids {
            let mut around = Vec::new();
            m.for_each_point_edge(p, |e| around.push(m.edge_other_point(e, p)));
            let outside: Vec<u32> = around.iter().copied().filter(|q| !ids.contains(q)).collect();
            let from = if outside.is_empty() { around } else { outside };
            if from.is_empty() {
                continue;
            }
            let mut acc: HashMap<u32, f64> = HashMap::new();
            for q in &from {
                for &(i, w) in old.get(*q as usize).map(Vec::as_slice).unwrap_or(&[]) {
                    *acc.entry(i).or_default() += w / from.len() as f64;
                }
            }
            let Some(own) = old.get(p as usize) else { continue };
            let held: Vec<(u32, f64)> = own.iter().copied().filter(|x| locks.locks.get(x.0 as usize).copied().unwrap_or(false)).collect();
            let mut next: Vec<(u32, f64)> = acc.into_iter().filter(|x| !locks.locks.get(x.0 as usize).copied().unwrap_or(false)).collect();
            next.extend(held);
            next.sort_by_key(|x| x.0);
            normalize_point(&mut next, &locks);
            b.weights[p as usize] = next;
        }
        commit_paint(g, sk, b)?;
    }
    Ok(Value::Null)
}

/// Moves a fraction of one influence's weight to another on some points.
fn transfer(s: &mut Session, a: &Args) -> CmdResult {
    let names = a.named();
    let g = &mut s.graph;
    let Some(skin_name) = names.first() else { fail!("transferBetweenInfluences: name the skinCluster") };
    let Some(sk) = skin_named(g, skin_name) else { fail!("transferBetweenInfluences: no skinCluster {skin_name}") };
    let (Some(src), Some(dst)) = (a.get("source").and_then(Value::as_str), a.get("destination").and_then(Value::as_str)) else { fail!("transferBetweenInfluences: give -src and -dst") };
    let list = influences(g, sk);
    let inf = |g: &Graph, n: &str| -> CmdResult<u32> {
        let j = crate::rig::find(g, n, "transferBetweenInfluences")?;
        match list.iter().find(|x| x.1 == j) {
            Some(e) => Ok(e.0 as u32),
            None => fail!("transferBetweenInfluences: {n} is not an influence of {skin_name}"),
        }
    };
    let (from, to) = (inf(g, src)?, inf(g, dst)?);
    let mut b = (*bind_of(g, sk).unwrap_or_default()).clone();
    if b.locks.get(from as usize).copied().unwrap_or(false) || b.locks.get(to as usize).copied().unwrap_or(false) {
        fail!("transferBetweenInfluences: unlock both influences first");
    }
    let ids: Vec<u32> = if names.len() > 1 {
        crate::skin::points_named(g, &names[1..]).into_iter().flat_map(|x| x.1).collect()
    } else {
        match skin_geometry(g, sk) {
            Some(m) => selected_points(g, m),
            None => Vec::new(),
        }
    };
    let amount = a.float("value", 1.0).clamp(0.0, 1.0);
    for p in ids {
        let Some(w) = b.weights.get_mut(p as usize) else { continue };
        let Some(i) = w.iter().position(|x| x.0 == from) else { continue };
        let moved = w[i].1 * amount;
        w[i].1 -= moved;
        match w.iter_mut().find(|x| x.0 == to) {
            Some(x) => x.1 += moved,
            None => w.push((to, moved)),
        }
        w.retain(|x| x.1 > 1e-9);
        w.sort_by_key(|x| x.0);
    }
    commit_paint(g, sk, b)?;
    Ok(Value::Null)
}

/// Weights on a skin for a mesh influence: by distance from its surface, each point keeping what it had in proportion.
pub(crate) fn geometry_influence_weights(b: &mut SkinBind, m: &Mesh, inf: u32, influence_mesh: &Mesh, influence_world: &DMat4, dropoff: f64) {
    let mut w_mesh = influence_mesh.clone();
    for p in 0..w_mesh.point_capacity() {
        if w_mesh.point_valid(p) {
            w_mesh.set_position(p, influence_world.transform_point3(influence_mesh.position(p)));
        }
    }
    let bvh = MeshBvh::new(&w_mesh);
    let locks = SkinBind { locks: b.locks.clone(), ..Default::default() };
    let geom = b.geom;
    for p in 0..m.point_capacity().min(b.weights.len() as u32) {
        if !m.point_valid(p) {
            continue;
        }
        let at = geom.transform_point3(m.position(p));
        let Some(c) = bvh.closest_point(at) else { continue };
        let others: f64 = b.weights[p as usize].iter().map(|x| x.1).sum();
        let w = (1.0 / (1.0 + c.distance)).powf(dropoff) * others.max(1e-3);
        let pw = &mut b.weights[p as usize];
        pw.retain(|x| x.0 != inf);
        pw.push((inf, w));
        pw.sort_by_key(|x| x.0);
        normalize_point(pw, &locks);
    }
}

/// Substitute Geometry: a new mesh takes over from a skinned one, with the same influences and the weights copied across.
fn substitute(s: &mut Session, a: &Args) -> CmdResult {
    let names = objects(s, a);
    let (old, new) = match (a.get("oldGeometry").and_then(Value::as_str), a.get("newGeometry").and_then(Value::as_str)) {
        (Some(o), Some(n)) => (o.to_string(), n.to_string()),
        _ => {
            let [old, new] = &names[..] else { fail!("substituteGeometry: select the skinned mesh, then its replacement") };
            (old.clone(), new.clone())
        }
    };
    let g = &mut s.graph;
    let Some(old_skin) = skin_named(g, &old) else { fail!("substituteGeometry: {old} has no skin") };
    let joints: Vec<String> = influences(g, old_skin).into_iter().map(|x| g.path(x.1)).collect();
    let max_inf = g.node(old_skin).map(|n| n.data().int(crate::skin::SK_MAX_INF)).unwrap_or(5);
    let mut args: Vec<Value> = joints.iter().map(|j| json!(j)).collect();
    args.push(json!(new));
    let made = execute(s, "skinCluster", &json!({"args": args, "toSelectedBones": true, "maximumInfluences": max_inf}), CallSource::Script)?;
    let new_skin = made.as_array().and_then(|v| v.first()).and_then(Value::as_str).unwrap_or("").to_string();
    let old_skin_path = s.graph.path(old_skin);
    execute(s, "copySkinWeights", &json!({"sourceSkin": old_skin_path, "destinationSkin": new_skin, "noMirror": true, "surfaceAssociation": "closestPoint", "influenceAssociation": ["oneToOne"]}), CallSource::Script)?;
    if !a.bool("retainOldGeometry") {
        execute(s, "delete", &json!({"args": [old]}), CallSource::Script)?;
    }
    Ok(json!([new_skin]))
}

fn remove_unused(s: &mut Session, a: &Args) -> CmdResult {
    let names = objects(s, a);
    let mut removed = Vec::new();
    for n in names {
        let Some(sk) = skin_named(&s.graph, &n) else { fail!("removeUnusedInfluences: {n} has no skin") };
        let g = &s.graph;
        let b = bind_of(g, sk).unwrap_or_default();
        for (i, j) in influences(g, sk) {
            if !b.weights.iter().any(|w| w.iter().any(|x| x.0 == i as u32 && x.1 > 0.0)) {
                removed.push(g.path(j));
            }
        }
        let path = s.graph.path(sk);
        execute(s, "skinCluster", &json!({"args": [path], "edit": true, "removeUnusedInfluence": true}), CallSource::Script)?;
    }
    Ok(json!(removed))
}

pub(crate) fn register(c: &mut Commands<Session>) {
    c.register(
        aztek_ops::CommandDef::<Session>::new(
            "artAttrSkinWeightCopy",
            "Copy Vertex Weights: holds the weights of the selected vertex, or their mean when several are selected.",
            |s, _| {
                let names = s.run("filterExpand", serde_json::json!({"selectionMask": 31, "expand": true}))?;
                let Some(list) = names.as_array().filter(|l| !l.is_empty()) else { aztek_ops::fail!("artAttrSkinWeightCopy: select vertices") };
                let first = list[0].as_str().unwrap_or("").split('.').next().unwrap_or("").to_string();
                let skin = s.run("findRelatedSkinCluster", serde_json::json!({"args": [first]}))?;
                let Some(skin) = skin.as_str().map(str::to_string) else { aztek_ops::fail!("artAttrSkinWeightCopy: {first} has no skin") };
                let infs = s.run("skinCluster", serde_json::json!({"args": [skin], "query": true, "queried": ["influence"]}))?;
                let mut held = Vec::new();
                for inf in infs.as_array().into_iter().flatten() {
                    let inf = inf.as_str().unwrap_or("").to_string();
                    let mut sum = 0.0;
                    for v in list {
                        let w = s.run("skinPercent", serde_json::json!({"args": [skin, v], "query": true, "queried": ["transform"], "transform": inf}))?;
                        sum += w.as_f64().unwrap_or(0.0);
                    }
                    held.push((inf, sum / list.len() as f64));
                }
                s.extras.more.copied_weights = held;
                Ok(serde_json::Value::Null)
            },
        )
        .undoable(false),
    );
    c.register(aztek_ops::CommandDef::<Session>::new(
        "artAttrSkinWeightPaste",
        "Paste Vertex Weights: gives the selected vertices the weights last copied.",
        |s, _| {
            let held = s.extras.more.copied_weights.clone();
            if held.is_empty() {
                aztek_ops::fail!("artAttrSkinWeightPaste: copy some weights first");
            }
            let names = s.run("filterExpand", serde_json::json!({"selectionMask": 31, "expand": true}))?;
            let Some(list) = names.as_array().filter(|l| !l.is_empty()).cloned() else { aztek_ops::fail!("artAttrSkinWeightPaste: select vertices") };
            let first = list[0].as_str().unwrap_or("").split('.').next().unwrap_or("").to_string();
            let skin = s.run("findRelatedSkinCluster", serde_json::json!({"args": [first]}))?;
            let Some(skin) = skin.as_str().map(str::to_string) else { aztek_ops::fail!("artAttrSkinWeightPaste: {first} has no skin") };
            let tv: Vec<serde_json::Value> = held.iter().map(|(j, w)| serde_json::json!([j, w])).collect();
            for v in list {
                s.run("skinPercent", serde_json::json!({"args": [skin, v], "transformValue": tv}))?;
            }
            Ok(serde_json::Value::Null)
        },
    ));

    c.register(
        Cmd::new(
            "deformerWeights",
            "Export or import a deformer's weights in the reference's layout: deformerWeights name.xml -ex -deformer skinCluster1 -path dir; -im with -method \
             index, nearest, barycentric, bilinear or over. A .json file, or -fm JSON, goes as JSON. Returns the file's path.",
            deformer_weights,
        )
        .undoable(true)
        .positional("the file")
        .flag(f("export", "ex", ArgType::Bool, "Write the file."))
        .flag(f("import", "im", ArgType::Bool, "Read the file."))
        .flag(f("deformer", "df", ArgType::Any, "The deformers. -shape takes all those on a mesh."))
        .flag(f("shape", "sh", ArgType::String, "The deformed shape."))
        .flag(f("path", "p", ArgType::String, "The file's folder."))
        .flag(f("method", "m", ArgType::String, "index (the default), nearest, barycentric, bilinear or over: over leaves points the file doesn't list."))
        .flag(f("defaultValue", "dv", ArgType::Float, "The weight of a point the file doesn't list."))
        .flag(f("format", "fm", ArgType::String, "XML or JSON."))
        .flag(f("weightPrecision", "wp", ArgType::Int, "Decimal places in XML (3)."))
        .flag(f("weightTolerance", "wt", ArgType::Float, "Leave out weights at or below this."))
        .flag(f("ignoreName", "ig", ArgType::Bool, "Match deformers and sources by order, not name.")),
    );
    c.register(
        Cmd::new("weightHammerVerts", "Hammer Skin Weights: the points named or selected take the average weights of the points around them.", hammer)
            .positional("points (the selection when none)"),
    );
    c.register(
        Cmd::new("transferBetweenInfluences", "Moves weight from one influence to another on the points named or selected: transferBetweenInfluences skinCluster1 -src joint1 -dst joint2 -v 0.5.", transfer)
            .positional("the skinCluster, then points")
            .flag(f("source", "src", ArgType::String, "The influence giving weight."))
            .flag(f("destination", "dst", ArgType::String, "The influence taking it."))
            .flag(f("value", "v", ArgType::Float, "How much of it moves (1).")),
    );
    c.register(
        Cmd::new("substituteGeometry", "Substitute Geometry: the second mesh takes over from the skinned first, with its influences and weights. The old mesh goes unless -rog. Returns the new skinCluster.", substitute)
            .positional("the skinned mesh, then its replacement (the selection when none)")
            .flag(f("oldGeometry", "og", ArgType::String, "The skinned mesh."))
            .flag(f("newGeometry", "ng", ArgType::String, "Its replacement."))
            .flag(f("retainOldGeometry", "rog", ArgType::Bool, "Keep the old mesh.")),
    );
    c.register(
        Cmd::new("removeUnusedInfluences", "Takes influences holding no weight off a skin. Returns those taken off.", remove_unused).positional("skinned meshes or skinClusters (the selection when none)"),
    );
}

// ---------------------------------------------------------------------------------------------------------------------
// UV space

/// A point's UV: from its first corner.
pub(crate) fn uv_of_point(m: &Mesh, p: u32) -> Option<[f32; 2]> {
    let uv = m.corner_attrs().get::<[f32; 2]>(crate::uv_ops::UV);
    let face = m.point_faces(p).into_iter().next()?;
    let c = m.face_corners(face).into_iter().find(|&c| m.corner_point(c) == p)?;
    uv.get(c as usize).copied()
}

/// A mesh's triangles in UV space, to find what lies at a UV.
pub(crate) struct UvLookup {
    tris: Vec<([u32; 3], [[f32; 2]; 3])>,
}

impl UvLookup {
    pub(crate) fn new(m: &Mesh) -> UvLookup {
        let uv = m.corner_attrs().get::<[f32; 2]>(crate::uv_ops::UV);
        let mut tris = Vec::new();
        for face in (0..m.face_capacity()).filter(|&x| m.face_valid(x)) {
            let corners = m.face_corners(face);
            let at = |k: usize| -> (u32, [f32; 2]) { (m.corner_point(corners[k]), uv.get(corners[k] as usize).copied().unwrap_or([0.0, 0.0])) };
            for k in 1..corners.len().saturating_sub(1) {
                let (a, b, c) = (at(0), at(k), at(k + 1));
                tris.push(([a.0, b.0, c.0], [a.1, b.1, c.1]));
            }
        }
        UvLookup { tris }
    }

    /// The source points at a UV and their shares: the triangle holding it, or failing that the nearest corner.
    pub(crate) fn sample(&self, uv: [f32; 2]) -> Option<Vec<(u32, f64)>> {
        let (x, y) = (uv[0] as f64, uv[1] as f64);
        let mut nearest: Option<(f64, u32)> = None;
        for (pts, t) in &self.tris {
            let (ax, ay, bx, by, cx, cy) = (t[0][0] as f64, t[0][1] as f64, t[1][0] as f64, t[1][1] as f64, t[2][0] as f64, t[2][1] as f64);
            let det = (by - cy) * (ax - cx) + (cx - bx) * (ay - cy);
            if det.abs() > 1e-14 {
                let u = ((by - cy) * (x - cx) + (cx - bx) * (y - cy)) / det;
                let v = ((cy - ay) * (x - cx) + (ax - cx) * (y - cy)) / det;
                let w = 1.0 - u - v;
                if u >= -1e-9 && v >= -1e-9 && w >= -1e-9 {
                    return Some(vec![(pts[0], u.max(0.0)), (pts[1], v.max(0.0)), (pts[2], w.max(0.0))]);
                }
            }
            for k in 0..3 {
                let d = (t[k][0] as f64 - x).powi(2) + (t[k][1] as f64 - y).powi(2);
                if nearest.is_none_or(|n| d < n.0) {
                    nearest = Some((d, pts[k]));
                }
            }
        }
        nearest.map(|n| vec![(n.1, 1.0)])
    }
}
