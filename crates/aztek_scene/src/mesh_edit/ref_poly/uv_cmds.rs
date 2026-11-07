//! UV commands under their reference names: polyMergeUV, polyMoveUV, polyLayoutUV, polyCopyUV, polyForceUV, polyPinUV,
//! polyContourProjection, and polyTransfer.

use std::collections::BTreeSet;

use aztek_geo::{colors, names, Index, Mesh};
use aztek_ops::{execute, fail, ArgType as T, CallSource, CmdResult, Commands};
use glam::DVec2;
use serde_json::{json, Value};

use super::*;
use crate::uv_ops::{self, Target, UvTopo};

/// The corners a UV target covers, with its faces' and its maps'.
fn corners_of(m: &Mesh, t: &Target, tp: &UvTopo) -> Vec<Index> {
    let faces = if t.whole { components::all_ids(m, K::Face) } else { t.faces.clone() };
    let mut out: BTreeSet<Index> = faces.iter().filter(|&&f| m.face_valid(f)).flat_map(|&f| m.face_corners(f)).collect();
    for &mp in &t.maps {
        if let Some(cs) = tp.maps.get(mp as usize) {
            out.extend(cs.iter().copied());
        }
    }
    out.into_iter().collect()
}

pub(super) fn register(c: &mut Commands<Session>) {
    c.register(with_table(
        Cmd::new(
            "polyMoveUV",
            "Move UVs: -t -tu -tv, -ra degrees, -s -su -sv, about -pvt. Returns the node.",
            |s, a| {
                let two = |flag: &str, x: &str, y: &str, def: f64| -> DVec2 {
                    let list: Vec<f64> = a.get(flag).and_then(Value::as_array).map(|v| v.iter().filter_map(Value::as_f64).collect()).unwrap_or_default();
                    DVec2::new(a.float(x, list.first().copied().unwrap_or(def)), a.float(y, list.get(1).copied().unwrap_or(def)))
                };
                let (t, sc, pv) = (two("translate", "translateU", "translateV", 0.0), two("scale", "scaleU", "scaleV", 1.0), two("pivot", "pivotU", "pivotV", 0.0));
                let angle = a.float("rotationAngle", 0.0);
                uv_ops::each(s, a, "polyMoveUV", "Move UVs", "polyMoveUV", |m, target, tp| {
                    let corners = corners_of(m, target, tp);
                    uv_ops::transform(m, &corners, t, sc, angle, pv);
                })
            },
        )
        .flag(fl("translate", "t", T::List, Value::Null, "U and V offset."))
        .flag(fl("scale", "s", T::List, Value::Null, "U and V scale."))
        .flag(fl("pivot", "pvt", T::List, Value::Null, "U and V pivot.")),
        "polyMoveUV",
        &[],
    ));
    c.register(ref_cmd(
        "polyMergeUV",
        "Merge UVs closer than -d into one, where they share a vertex. Returns the node.",
        &[("distance", json!(0.0))],
        |s, a| {
            let d = a.float("distance", 0.0).max(1e-7);
            uv_ops::each(s, a, "polyMergeUV", "Merge UVs", "polyMergeUV", move |m, target, tp| {
                let corners = corners_of(m, target, tp);
                let mut by_point: std::collections::BTreeMap<Index, Vec<Index>> = Default::default();
                for &c in &corners {
                    by_point.entry(m.corner_point(c)).or_default().push(c);
                }
                let name = names::uv(0);
                let uv = m.corner_attrs_mut().get_or_add::<[f32; 2]>(name.as_str());
                for group in by_point.values() {
                    let mut used = vec![false; group.len()];
                    for i in 0..group.len() {
                        if used[i] {
                            continue;
                        }
                        let (mut su, mut sv, mut n) = (0.0f64, 0.0f64, 0.0f64);
                        let mut members = Vec::new();
                        let base = uv[group[i] as usize];
                        for (j, &c) in group.iter().enumerate() {
                            let q = uv[c as usize];
                            let dist = ((q[0] - base[0]) as f64).hypot((q[1] - base[1]) as f64);
                            if !used[j] && dist <= d {
                                used[j] = true;
                                su += q[0] as f64;
                                sv += q[1] as f64;
                                n += 1.0;
                                members.push(c);
                            }
                        }
                        for c in members {
                            uv[c as usize] = [(su / n) as f32, (sv / n) as f32];
                        }
                    }
                }
            })
        },
    ));
    c.register(ref_cmd(
        "polyLayoutUV",
        "Pack UV shells into 0 to 1: -ps percent space between them, -rbf 0 keeps them unturned. Returns the node.",
        &[("percentageSpace", json!(0.2)), ("rotateForBestFit", json!(1))],
        |s, a| {
            let spacing = (a.float("percentageSpace", 0.2) / 100.0).max(0.002);
            let rotate = a.int("rotateForBestFit", 1) != 0;
            uv_ops::each(s, a, "polyLayoutUV", "Layout", "polyLayoutUV", move |m, target, tp| {
                let faces = uv_ops::target_faces(m, target, tp);
                let shells = uv_ops::shells_of(tp, &faces).concat();
                uv_ops::layout(m, &shells, spacing * 2.0, rotate);
            })
        },
    ));
    extend(c, "polyFlipUV", "polyFlipUV", &[], |_, _| None);
    c.register(ref_cmd(
        "polyCopyUV",
        "Copy one UV set onto the current UVs: -uvi the set to read. Returns the node.",
        &[],
        |s, a| {
            let from = a.string("uvSetNameInput", "");
            if from.is_empty() {
                fail!("polyCopyUV: name the set to copy with -uvi");
            }
            let layer = format!("uvset:{from}");
            uv_ops::each(s, a, "polyCopyUV", "Copy UVs", "polyCopyUV", move |m, target, tp| {
                let corners = corners_of(m, target, tp);
                let src: Vec<[f32; 2]> = m.corner_attrs().get::<[f32; 2]>(&layer).to_vec();
                let name = names::uv(0);
                let uv = m.corner_attrs_mut().get_or_add::<[f32; 2]>(name.as_str());
                for c in corners {
                    if let Some(v) = src.get(c as usize) {
                        uv[c as usize] = *v;
                    }
                }
            })
        },
    ));
    c.register(ref_cmd(
        "polyForceUV",
        "Change UVs in place: -fh -fv flip about their middle, -uni gives each face all of 0 to 1, -nor \"transform\" or \"unitize\" fits them. Returns the node.",
        &[],
        |s, a| {
            let (fh, fv) = (a.bool("flipHorizontal"), a.bool("flipVertical"));
            let unitize = a.bool("unitize") || a.string("normalize", "") == "unitize";
            let fit = a.string("normalize", "") == "transform";
            uv_ops::each(s, a, "polyForceUV", "Force UVs", "polyForceUV", move |m, target, tp| {
                let corners = corners_of(m, target, tp);
                let name = names::uv(0);
                if unitize {
                    let faces = uv_ops::target_faces(m, target, tp);
                    for f in faces {
                        let cs = m.face_corners(f);
                        let n = cs.len() as f64;
                        let uv = m.corner_attrs_mut().get_or_add::<[f32; 2]>(name.as_str());
                        for (i, &k) in cs.iter().enumerate() {
                            let a = std::f64::consts::TAU * (i as f64 / n) + std::f64::consts::FRAC_PI_4 * 5.0;
                            uv[k as usize] = [(0.5 + a.cos() * std::f64::consts::FRAC_1_SQRT_2) as f32, (0.5 + a.sin() * std::f64::consts::FRAC_1_SQRT_2) as f32];
                        }
                    }
                    return;
                }
                let uv = m.corner_attrs_mut().get_or_add::<[f32; 2]>(name.as_str());
                let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
                for &k in &corners {
                    for i in 0..2 {
                        lo[i] = lo[i].min(uv[k as usize][i]);
                        hi[i] = hi[i].max(uv[k as usize][i]);
                    }
                }
                for &k in &corners {
                    let q = &mut uv[k as usize];
                    if fh {
                        q[0] = lo[0] + hi[0] - q[0];
                    }
                    if fv {
                        q[1] = lo[1] + hi[1] - q[1];
                    }
                    if fit {
                        for i in 0..2 {
                            q[i] = (q[i] - lo[i]) / (hi[i] - lo[i]).max(1e-6);
                        }
                    }
                }
            })
        },
    ));
    c.register(ref_cmd(
        "polyContourProjection",
        "Contour stretch: fits UVs of a run of quads across 0 to 1 by their arc lengths along the rows, from the faces' own order. -ps accepted. Returns the node.",
        &[],
        |s, a| {
            uv_ops::each(s, a, "polyContourProjection", "Contour Projection", "polyContourProj", move |m, target, tp| {
                let faces = uv_ops::target_faces(m, target, tp);
                uv_ops::project_faces(m, &faces, aztek_uv::project::Projection::Planar, aztek_uv::project::ProjectAxis::BestFit);
                uv_ops::unfold(m, &faces);
            })
        },
    ));
    c.register(ref_cmd(
        "polyPinUV",
        "Pin UVs so Unfold holds them: -v 1 pins, 0 unpins; -op 1 adds, 2 clears everything. -unp works on the rest. Changes the mesh directly.",
        &[],
        |s, a| {
            let value = match a.get("value") {
                Some(Value::Array(l)) => l.first().and_then(Value::as_f64).unwrap_or(1.0),
                Some(v) => v.as_f64().unwrap_or(1.0),
                None => 1.0,
            } as f32;
            let targets = uv_ops::targets(s, a, "polyPinUV")?;
            let clear = a.int("operation", 0) == 2;
            let unpinned = a.bool("unpinned");
            let g = &mut s.graph;
            g.open_chunk("Pin UVs");
            let r = (|| -> CmdResult<()> {
                for (n, t) in targets {
                    let base = crate::mesh_edit::base_plug(g, n)?;
                    let mut m = crate::mesh_edit::base_mesh(g, &base);
                    let tp = uv_ops::topo(&m);
                    let corners = corners_of(&m, &t, &tp);
                    let layer = m.corner_attrs_mut().get_or_add::<f32>("uvPin");
                    if clear {
                        layer.iter_mut().for_each(|x| *x = 0.0);
                    } else if unpinned {
                        let set: BTreeSet<Index> = corners.iter().copied().collect();
                        for (i, x) in layer.iter_mut().enumerate() {
                            if !set.contains(&(i as Index)) {
                                *x = value;
                            }
                        }
                    } else {
                        for k in corners {
                            layer[k as usize] = value;
                        }
                    }
                    g.set_value(base, crate::plug::PlugValue::Mesh(Some(std::sync::Arc::new(m))))?;
                }
                Ok(())
            })();
            g.close_chunk();
            r?;
            Ok(Value::Null)
        },
    ));
    c.register(ref_cmd(
        "polyTransfer",
        "Copy from another mesh of the same topology, -ao names it: -uv UVs (on by default), -v vertices, -vc colours. UVs go through a polyTransfer node; vertices and \
         colours change the mesh directly. Returns the node when there is one.",
        &[("uvSets", json!(true)), ("vertices", json!(false)), ("vertexColor", json!(false))],
        |s, a| {
            let from = a.string("alternateObject", "");
            if from.is_empty() {
                fail!("polyTransfer: name the other mesh with -ao");
            }
            let g = &mut s.graph;
            let source = crate::mesh_edit::mesh_arg(g, &from, "polyTransfer")?;
            let Some(src) = crate::modifiers::cage(g, source) else { fail!("polyTransfer: {from} has no mesh") };
            let (want_v, want_c, want_uv) = (a.bool("vertices"), a.bool("vertexColor"), on(a, "uvSets", true));
            let name = names::uv(0);
            let src_uv: Vec<[f32; 2]> = src.corner_attrs().get::<[f32; 2]>(name.as_str()).to_vec();
            let out = if want_uv {
                let src_uv = src_uv.clone();
                uv_ops::each(s, a, "polyTransfer", "Transfer", "polyTransfer", move |m, _t, _tp| {
                    let uv = m.corner_attrs_mut().get_or_add::<[f32; 2]>(names::uv(0).as_str());
                    let n = uv.len().min(src_uv.len());
                    uv[..n].copy_from_slice(&src_uv[..n]);
                })?
            } else {
                Value::Null
            };
            if want_v || want_c {
                let targets = uv_ops::targets(s, a, "polyTransfer")?;
                for n in targets.into_keys() {
                    if want_v {
                        let pts: Vec<Value> = (0..src.point_capacity()).filter(|&p| src.point_valid(p)).map(|p| {
                            let q = src.position(p);
                            json!([q.x, q.y, q.z])
                        }).collect();
                        let path = components::display_path(&s.graph, n);
                        execute(s, "setMeshPoints", &json!({"args": [path], "points": pts}), CallSource::Script)?;
                    }
                    if want_c {
                        let g = &mut s.graph;
                        let base = crate::mesh_edit::base_plug(g, n)?;
                        let mut m = crate::mesh_edit::base_mesh(g, &base);
                        for set in colors::sets(&src) {
                            let vals = colors::values(&src, &set).to_vec();
                            colors::add_white(&mut m, &set);
                            let layer = colors::values_mut(&mut m, &set);
                            let k = layer.len().min(vals.len());
                            layer[..k].copy_from_slice(&vals[..k]);
                        }
                        g.set_value(base, crate::plug::PlugValue::Mesh(Some(std::sync::Arc::new(m))))?;
                    }
                }
            }
            Ok(out)
        },
    ));
}
