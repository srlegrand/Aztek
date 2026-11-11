//! Per-mesh data commands: vertex normals, colours and colour sets, UV sets, the display options table, the clipboard.

use std::collections::BTreeMap;

use aztek_geo::normals::{corner_normals, update_corner_normals, CornerNormalOptions};
use aztek_geo::symmetry::SymmetryMap;
use aztek_geo::{colors, names, Index, Mesh};
use aztek_ops::{fail, ArgType as T, Args, CmdResult, Commands};
use glam::{DVec3, Vec3};
use serde_json::{json, Value};

use super::queries::{parse_item, Item};
use super::*;
use crate::components::{ComponentMode as K, Ids};
use crate::history::{register_op, HistoryOp};
use crate::mesh_edit::EditResult;

use crate::uv_sets;

fn uv_set_op(a: &Args, m: &mut Mesh, _sel: &Ids, _sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    let set = a.string("uvSet", "");
    let new = a.string("newUVSet", "");
    let named = |m: &Mesh| if set.is_empty() { uv_sets::current(m) } else { set.clone() };
    if a.bool("create") {
        uv_sets::create(m, if !set.is_empty() { &set } else { &new });
    } else if a.bool("delete") {
        let name = named(m);
        if let Err(e) = uv_sets::delete(m, &name) {
            fail!("polyUVSet: {e}");
        }
    } else if a.bool("rename") {
        if new.is_empty() {
            fail!("polyUVSet: -rename needs -newUVSet");
        }
        let name = named(m);
        if let Err(e) = uv_sets::rename(m, &name, &new) {
            fail!("polyUVSet: {e}");
        }
    } else if a.bool("copy") {
        let from = named(m);
        if uv_sets::copy(m, &from, if new.is_empty() { "uvSet1" } else { &new }).is_none() {
            fail!("polyUVSet: no UV set {from}");
        }
    } else if a.bool("currentUVSet") {
        let name = named(m);
        if !uv_sets::set_current(m, &name) {
            fail!("polyUVSet: no UV set {name}");
        }
    }
    Ok(EditResult::default())
}

fn color_set_op(a: &Args, m: &mut Mesh, _sel: &Ids, _sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    let set = a.string("colorSet", "");
    let new = a.string("newColorSet", "");
    if a.bool("create") {
        let name = if !set.is_empty() { set } else if !new.is_empty() { new } else { colors::unique_name(m, "colorSet1") };
        colors::add_white(m, &name);
    } else if a.bool("delete") {
        let name = if set.is_empty() { colors::current(m) } else { set };
        colors::remove(m, &name);
    } else if a.bool("rename") {
        let name = if set.is_empty() { colors::current(m) } else { set };
        colors::rename(m, &name, &new);
    } else if a.bool("copy") {
        let name = if set.is_empty() { colors::current(m) } else { set };
        let to = if new.is_empty() { colors::unique_name(m, "colorSet1") } else { new };
        colors::copy(m, &name, &to);
    } else if a.bool("currentColorSet") && !set.is_empty() && !colors::set_current(m, &set) {
        fail!("polyColorSet: no colour set {set}");
    }
    Ok(EditResult::default())
}

const UNSET_COLOR: f64 = -1.0;

fn color_value(a: &Args, was: [f32; 4]) -> [f32; 4] {
    let rel = a.bool("relative");
    let rgb = a.vec3("colorRGB");
    // -1 is how the node holds a channel the call didn't set.
    let pick = |i: usize, single: &str| -> f32 {
        let unset = |v: &f64| *v != UNSET_COLOR;
        let given = a.get(single).and_then(Value::as_f64).filter(unset).or(rgb.filter(|_| i < 3).map(|c| c[i]).filter(unset));
        match given {
            Some(v) if rel => was[i] + v as f32,
            Some(v) => v as f32,
            None => was[i],
        }
    };
    let mut out = [pick(0, "colorR"), pick(1, "colorG"), pick(2, "colorB"), pick(3, "alpha")];
    if a.bool("clamped") {
        for v in &mut out {
            *v = v.clamp(0.0, 1.0);
        }
    }
    out
}

fn color_vertex_op(a: &Args, m: &mut Mesh, sel: &Ids, _sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    let mut set = colors::current(m);
    if a.bool("remove") {
        if !set.is_empty() {
            colors::remove(m, &set);
        }
        return Ok(EditResult::default());
    }
    if set.is_empty() {
        set = "colorSet1".to_string();
        colors::add_white(m, &set);
        colors::set_current(m, &set);
    }
    let mut corners = Vec::new();
    if let Some(list) = a.get("corners").and_then(Value::as_array) {
        // vtxFace: only those corners.
        for pr in list {
            if let (Some(v), Some(f)) = (pr.get(0).and_then(Value::as_u64), pr.get(1).and_then(Value::as_u64)) {
                let c = m.find_corner(f as Index, v as Index);
                if c != aztek_geo::INVALID {
                    corners.push(c);
                }
            }
        }
    } else {
        for &p in sel {
            if m.point_valid(p) {
                m.for_each_point_corner(p, |c| corners.push(c));
            }
        }
    }
    let values = colors::values_mut(m, &set);
    for c in corners {
        values[c as usize] = color_value(a, values[c as usize]);
    }
    colors::settle(m, &set);
    Ok(EditResult::default())
}

fn normal_vertex_op(a: &Args, m: &mut Mesh, sel: &Ids, _sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    if a.bool("unFreezeNormal") {
        m.corner_attrs_mut().remove(names::NORMAL);
        return Ok(EditResult::default());
    }
    if !m.corner_attrs().has(names::NORMAL) {
        update_corner_normals(m, &CornerNormalOptions::default());
    }
    let mut xyz = vec_flag(a, "normalXYZ", DVec3::ZERO);
    if xyz.length_squared() < 1e-18 {
        xyz = DVec3::new(a.float("normalX", 0.0), a.float("normalY", 0.0), a.float("normalZ", 0.0));
    }
    if xyz.length_squared() < 1e-18 {
        return Ok(EditResult::default());
    }
    let rel = a.bool("relative");
    let mut corners = Vec::new();
    for &p in sel {
        if m.point_valid(p) {
            m.for_each_point_corner(p, |c| corners.push(c));
        }
    }
    let n = m.corner_attrs_mut().get_mut::<[f32; 3]>(names::NORMAL);
    for c in corners {
        let v = if rel { Vec3::from_array(n[c as usize]).as_dvec3() + xyz } else { xyz };
        n[c as usize] = v.normalize_or_zero().as_vec3().to_array();
    }
    Ok(EditResult::default())
}

fn to_face_normal_op(_a: &Args, m: &mut Mesh, sel: &Ids, _sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    if !m.corner_attrs().has(names::NORMAL) {
        update_corner_normals(m, &CornerNormalOptions::default());
    }
    let mut writes = Vec::new();
    for &f in sel {
        if m.face_valid(f) {
            let n = m.face_normal(f).as_vec3().to_array();
            for c in m.face_corners(f) {
                writes.push((c, n));
            }
        }
    }
    let layer = m.corner_attrs_mut().get_mut::<[f32; 3]>(names::NORMAL);
    for (c, n) in writes {
        layer[c as usize] = n;
    }
    Ok(EditResult::default())
}

fn average_normal_op(a: &Args, m: &mut Mesh, sel: &Ids, _sym: Option<&SymmetryMap>) -> CmdResult<EditResult> {
    if !m.corner_attrs().has(names::NORMAL) {
        update_corner_normals(m, &CornerNormalOptions::default());
    }
    let distance = a.float("distance", 0.0);
    let replace = vec_flag(a, "replaceNormalXYZ", DVec3::X);
    // Vertices closer than distance share: chain them by position.
    let mut groups: Vec<Vec<Index>> = Vec::new();
    for &p in sel {
        if !m.point_valid(p) {
            continue;
        }
        let at = m.position(p);
        match groups.iter_mut().find(|g| g.iter().any(|&q| m.position(q).distance(at) <= distance)) {
            Some(g) => g.push(p),
            None => groups.push(vec![p]),
        }
    }
    let values: Vec<Vec3> = m.corner_attrs().get::<[f32; 3]>(names::NORMAL).iter().map(|v| Vec3::from_array(*v)).collect();
    let mut writes = Vec::new();
    for g in groups {
        let mut corners = Vec::new();
        for p in g {
            m.for_each_point_corner(p, |c| corners.push(c));
        }
        let sum: DVec3 = corners.iter().map(|&c| values[c as usize].as_dvec3()).sum();
        let avg = if sum.length_squared() < 1e-18 && !on(a, "allowZeroNormal", false) { replace } else { sum };
        let n = if on(a, "postnormalize", true) { avg.normalize_or_zero() } else { avg };
        for c in corners {
            writes.push((c, n.as_vec3().to_array()));
        }
    }
    let layer = m.corner_attrs_mut().get_mut::<[f32; 3]>(names::NORMAL);
    for (c, n) in writes {
        layer[c as usize] = n;
    }
    Ok(EditResult::default())
}

pub(super) fn register_ops() {
    let op = |node_type: &'static str, command: &'static str, label: &'static str, kind: K, run: crate::history::OpFn, doc: &'static str| {
        register_op(HistoryOp { node_type, command, label, kind, run, skip_flags: &[], doc })
    };
    op("createUVSet", "polyUVSet", "UV Set", K::Face, uv_set_op, "Creates, removes, renames or copies a UV set.");
    op("createColorSet", "polyColorSet", "Color Set", K::Face, color_set_op, "Creates, removes, renames or copies a colour set.");
    op("polyColorPerVertex", "polyColorPerVertex", "Color Per Vertex", K::Vertex, color_vertex_op, "Sets vertex colours.");
    op("polyNormalPerVertex", "polyNormalPerVertex", "Normal Per Vertex", K::Vertex, normal_vertex_op, "Sets or frees vertex normals.");
    op("polySetToFaceNormal", "polySetToFaceNormal", "Set to Face", K::Face, to_face_normal_op, "Sets vertex normals to their faces' normals.");
    op("polyAverageNormal", "polyAverageNormal", "Average Normals", K::Vertex, average_normal_op, "Averages vertex normals.");
}

/// The first mesh a query names or has selected, with its cage.
fn query_mesh(s: &mut Session, a: &Args) -> CmdResult<(NodeId, crate::plug::MeshPtr)> {
    let pairs = vtx_face_pairs(s, a)?;
    let n = match pairs.first() {
        Some((n, _)) => *n,
        None => {
            let (targets, _) = gather(s, a, K::Vertex)?;
            let Some(&n) = targets.keys().next() else { fail!("name or select a mesh") };
            n
        }
    };
    let Some(m) = crate::modifiers::cage(&mut s.graph, n) else { fail!("no mesh") };
    Ok((n, m))
}

fn vtx_face_pairs(s: &mut Session, a: &Args) -> CmdResult<Vec<(NodeId, Vec<(Index, Index)>)>> {
    let mut out: Vec<(NodeId, Vec<(Index, Index)>)> = Vec::new();
    for n in a.named() {
        if let Item::VtxFace(mesh, v, f) = parse_item(&mut s.graph, &n)? {
            let mut pairs = Vec::new();
            for p in &v {
                for x in &f {
                    pairs.push((*p, *x));
                }
            }
            out.push((mesh, pairs));
        }
    }
    Ok(out)
}

pub(super) fn register(c: &mut Commands<Session>) {
    c.register(with_table(
        query_flags(Cmd::new(
            "polyNormalPerVertex",
            "Set vertex normals: -xyz or -x -y -z, -rel adds. -fn freezes, -ufn lets them follow the surface again. -q -xyz gives each normal in turn; -q -fn \
             whether they are frozen. Also takes vtxFace components when querying.",
            |s, a| {
                if a.bool("query") {
                    let flat_pairs = vtx_face_pairs(s, a)?;
                    let (n, m) = query_mesh(s, a)?;
                    if asked(a, "freezeNormal") || asked(a, "allLocked") {
                        return Ok(json!(m.corner_attrs().has(names::NORMAL)));
                    }
                    let normals = if m.corner_attrs().has(names::NORMAL) {
                        m.corner_attrs().get::<[f32; 3]>(names::NORMAL).iter().map(|v| Vec3::from_array(*v)).collect()
                    } else {
                        corner_normals(&m, &CornerNormalOptions::default())
                    };
                    let mut out = Vec::new();
                    let mut push = |v: Vec3| {
                        if asked(a, "normalX") {
                            out.push(json!(v.x as f64));
                        } else if asked(a, "normalY") {
                            out.push(json!(v.y as f64));
                        } else if asked(a, "normalZ") {
                            out.push(json!(v.z as f64));
                        } else {
                            out.extend([json!(v.x as f64), json!(v.y as f64), json!(v.z as f64)]);
                        }
                    };
                    if !flat_pairs.is_empty() {
                        for (_, pairs) in &flat_pairs {
                            for &(p, f) in pairs {
                                let k = m.find_corner(f, p);
                                if k != aztek_geo::INVALID {
                                    push(normals[k as usize]);
                                }
                            }
                        }
                    } else {
                        let (targets, _) = gather(s, a, K::Vertex)?;
                        for &p in targets.get(&n).map(Vec::as_slice).unwrap_or(&[]) {
                            m.for_each_point_corner(p, |k| push(normals[k as usize]));
                        }
                    }
                    return Ok(Value::Array(out));
                }
                run_op(s, "polyNormalPerVertex", a)
            },
        )),
        "polyNormalPerVertex",
        &[],
    ));
    c.register(ref_cmd(
        "polySetToFaceNormal",
        "Set each vertex normal of the faces to its face's normal, freezing it. Returns the node.",
        &[],
        |s, a| run_op(s, "polySetToFaceNormal", a),
    ));
    c.register(ref_cmd(
        "polyAverageNormal",
        "Average the normals at each vertex and among vertices within -d. Returns the node.",
        &[("postnormalize", json!(true)), ("prenormalize", json!(true)), ("replaceNormalXYZ", json!([1.0, 0.0, 0.0]))],
        |s, a| run_op(s, "polyAverageNormal", a),
    ));

    c.register(with_table(
        query_flags(Cmd::new(
            "polyColorPerVertex",
            "Set vertex colours: -r -g -b -a or -rgb, -rel adds, -rem removes the set's colours. -cdo shows them. -q -r -g -b -a -rgb reads each in turn. Works on \
             vtx and vtxFace. Returns the node.",
            |s, a| {
                if a.bool("query") {
                    let pairs = vtx_face_pairs(s, a)?;
                    let (n, m) = query_mesh(s, a)?;
                    let set = colors::current(&m);
                    if asked(a, "colorDisplayOption") {
                        let g = &s.graph;
                        return Ok(json!(g.node(n).is_some_and(|x| x.ty().attr_index("displayColors").is_some_and(|i| x.bool(i)))));
                    }
                    let vals = colors::values(&m, &set).to_vec();
                    let mut out = Vec::new();
                    let mut push = |v: [f32; 4]| {
                        if asked(a, "colorRGB") {
                            out.extend([json!(v[0] as f64), json!(v[1] as f64), json!(v[2] as f64)]);
                        }
                        for (i, f) in ["colorR", "colorG", "colorB", "alpha"].iter().enumerate() {
                            if asked(a, f) {
                                out.push(json!(v[i] as f64));
                            }
                        }
                    };
                    if vals.is_empty() {
                        fail!("polyColorPerVertex: the mesh has no colour set");
                    }
                    if !pairs.is_empty() {
                        for (_, list) in &pairs {
                            for &(p, f) in list {
                                let k = m.find_corner(f, p);
                                if k != aztek_geo::INVALID {
                                    push(vals[k as usize]);
                                }
                            }
                        }
                    } else {
                        let (targets, _) = gather(s, a, K::Vertex)?;
                        for &p in targets.get(&n).map(Vec::as_slice).unwrap_or(&[]) {
                            // Shared colours read once per vertex, as colours set on the vertex are.
                            let mut first = None;
                            m.for_each_point_corner(p, |k| first = first.or(Some(k)));
                            if let Some(k) = first {
                                push(vals[k as usize]);
                            }
                        }
                    }
                    return Ok(Value::Array(out));
                }
                let pairs = vtx_face_pairs(s, a)?;
                if !pairs.is_empty() {
                    let mut last = Value::Null;
                    for (mesh, list) in pairs {
                        let mut a2 = fold_vectors(a);
                        a2.positional.clear();
                        a2.flags.insert("corners".into(), json!(list.iter().map(|(p, f)| json!([p, f])).collect::<Vec<_>>()));
                        let op = crate::history::op("polyColorPerVertex").unwrap();
                        let g = &mut s.graph;
                        g.open_chunk("Color Per Vertex");
                        let a3 = &a2;
                        let r = apply_targets(g, K::Vertex, BTreeMap::from([(mesh, Vec::new())]), &mut |m, sel, sym| color_vertex_op(a3, m, sel, sym), Some((&*op, a3)), None);
                        g.close_chunk();
                        last = names_of(g, &r?);
                    }
                    return Ok(last);
                }
                run_op(s, "polyColorPerVertex", a)
            },
        ))
        .flag(fl("corners", "", T::List, Value::Null, "vtxFace pairs the edit works on, when it isn't whole vertices.")),
        "polyColorPerVertex",
        &[
            ("colorR", json!(UNSET_COLOR)),
            ("colorG", json!(UNSET_COLOR)),
            ("colorB", json!(UNSET_COLOR)),
            ("alpha", json!(UNSET_COLOR)),
            ("colorRGB", json!([UNSET_COLOR, UNSET_COLOR, UNSET_COLOR])),
        ],
    ));
    c.register(with_table(
        query_flags(Cmd::new(
            "polyColorSet",
            "Colour sets: -cr -d -rn -cp with -cs and -nc, -ccs -cs makes one current. -q -acs lists them, -q -ccs names the current one.",
            |s, a| {
                if a.bool("query") {
                    let (_, m) = query_mesh(s, a)?;
                    if asked(a, "allColorSets") {
                        let all = colors::sets(&m);
                        return Ok(if all.is_empty() { Value::Null } else { json!(all) });
                    }
                    if asked(a, "currentColorSet") {
                        let cur = colors::current(&m);
                        return Ok(if cur.is_empty() { Value::Null } else { json!([cur]) });
                    }
                    fail!("polyColorSet: ask -acs or -ccs");
                }
                run_op(s, "createColorSet", a)
            },
        )),
        "polyColorSet",
        &[],
    ));
    c.register(with_table(
        query_flags(Cmd::new(
            "polyUVSet",
            "UV sets: -cr -d -rn -cp with -uvs and -nuv, -cuv -uvs makes one current. -q -auv lists them, -q -cuv names the current one.",
            |s, a| {
                if a.bool("query") {
                    let (_, m) = query_mesh(s, a)?;
                    if asked(a, "allUVSets") {
                        return Ok(json!(uv_sets::all(&m)));
                    }
                    if asked(a, "currentUVSet") {
                        return Ok(json!([uv_sets::current(&m)]));
                    }
                    fail!("polyUVSet: ask -auv or -cuv");
                }
                // A new set's name is settled here, so the node makes that one each time it runs, and it is what comes back.
                let (set, new) = (a.string("uvSet", ""), a.string("newUVSet", ""));
                if a.bool("create") || (a.bool("copy") && new.is_empty()) {
                    let (_, m) = query_mesh(s, a)?;
                    let wanted = if a.bool("copy") { "uvSet1".to_string() } else if !set.is_empty() { set } else if !new.is_empty() { new } else { "uvSet1".to_string() };
                    let name = uv_sets::unique(&m, &wanted);
                    let mut named = a.clone();
                    named.flags.insert(if a.bool("copy") { "newUVSet" } else { "uvSet" }.to_string(), json!(name));
                    run_op(s, "createUVSet", &named)?;
                    return Ok(json!([name]));
                }
                run_op(s, "createUVSet", a)?;
                Ok(if a.bool("copy") || a.bool("rename") { json!([new]) } else if set.is_empty() { Value::Null } else { json!([set]) })
            },
        )),
        "polyUVSet",
        &[],
    ));

    c.register(
        with_table(
            query_flags(Cmd::new(
                "polyOptions",
                "Display options for meshes: -bc -dn -sn -ds -dt -dv -dce and the rest are remembered and read back with -q. They set state for scripts; most don't change \
                 the viewport.",
                |s, a| {
                    let st = &mut s.poly_state.options;
                    if a.bool("query") {
                        let queried: Vec<String> = a.get("queried").and_then(Value::as_array).map(|q| q.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default();
                        let Some(name) = queried.first() else { fail!("polyOptions: ask about a flag") };
                        return Ok(json!([st.get(name).cloned().unwrap_or(json!(false))]));
                    }
                    for (k, v) in &a.flags {
                        if !matches!(k.as_str(), "query" | "edit" | "queried" | "activeObjects" | "allEdges") {
                            st.insert(k.clone(), v.clone());
                        }
                    }
                    Ok(Value::Null)
                },
            ))
            .undoable(false),
            "polyOptions",
            &[],
        ),
    );

    c.register(
        ref_cmd(
            "polyClipboard",
            "Copy attributes of one face and paste them onto others: -cp or -ps, with -uv and -clr; -sh accepted. -cl empties it.",
            &[],
            |s, a| {
                if a.bool("clear") {
                    s.poly_state.clip_uvs.clear();
                    s.poly_state.clip_colors.clear();
                    return Ok(Value::Null);
                }
                let (targets, _) = gather(s, a, K::Face)?;
                if a.bool("copy") {
                    let Some((&n, ids)) = targets.iter().next() else { fail!("polyClipboard: name a face to copy") };
                    let Some(&f) = ids.first() else { fail!("polyClipboard: name a face to copy") };
                    let Some(m) = crate::modifiers::cage(&mut s.graph, n) else { fail!("polyClipboard: no mesh") };
                    let corners = m.face_corners(f);
                    if a.bool("uvCoordinates") {
                        let uv = m.corner_attrs().get::<[f32; 2]>(names::uv(0).as_str());
                        s.poly_state.clip_uvs = corners.iter().map(|&c| uv.get(c as usize).copied().unwrap_or([0.0; 2])).collect();
                    }
                    if a.bool("color") {
                        let set = colors::current(&m);
                        let vals = colors::values(&m, &set);
                        s.poly_state.clip_colors = corners.iter().map(|&c| vals.get(c as usize).copied().unwrap_or([1.0; 4])).collect();
                    }
                    return Ok(Value::Null);
                }
                if a.bool("paste") {
                    let (uvs, cols) = (s.poly_state.clip_uvs.clone(), s.poly_state.clip_colors.clone());
                    let (want_uv, want_col) = (a.bool("uvCoordinates"), a.bool("color"));
                    let g = &mut s.graph;
                    g.open_chunk("Paste");
                    let r = (|| -> CmdResult<()> {
                        for (n, ids) in targets {
                            let base = crate::mesh_edit::base_plug(g, n)?;
                            let mut m = crate::mesh_edit::base_mesh(g, &base);
                            for f in ids {
                                if !m.face_valid(f) {
                                    continue;
                                }
                                let corners = m.face_corners(f);
                                if want_uv && !uvs.is_empty() {
                                    let layer = m.corner_attrs_mut().get_or_add::<[f32; 2]>(names::uv(0).as_str());
                                    for (i, &c) in corners.iter().enumerate() {
                                        layer[c as usize] = uvs[i % uvs.len()];
                                    }
                                }
                                if want_col && !cols.is_empty() {
                                    let mut set = colors::current(&m);
                                    if set.is_empty() {
                                        set = "colorSet1".into();
                                        colors::add_white(&mut m, &set);
                                        colors::set_current(&mut m, &set);
                                    }
                                    let layer = colors::values_mut(&mut m, &set);
                                    for (i, &c) in corners.iter().enumerate() {
                                        layer[c as usize] = cols[i % cols.len()];
                                    }
                                }
                            }
                            g.set_value(base, crate::plug::PlugValue::Mesh(Some(std::sync::Arc::new(m))))?;
                        }
                        Ok(())
                    })();
                    g.close_chunk();
                    r?;
                    return Ok(Value::Null);
                }
                fail!("polyClipboard: say -cp, -ps or -cl")
            },
        ),
    );
    c.register(
        Cmd::new("polyPasteFace", "Paste the clipboard's UVs and colours onto faces. Returns nothing.", |s, a| {
            let mut call = serde_json::Map::new();
            call.insert("paste".into(), json!(true));
            call.insert("uvCoordinates".into(), json!(true));
            call.insert("color".into(), json!(true));
            if !a.named().is_empty() {
                call.insert("args".into(), json!(a.named()));
            }
            aztek_ops::execute(s, "polyClipboard", &Value::Object(call), aztek_ops::CallSource::Script)
        })
        .positional("faces (the selection when none)")
        .flag(fl("constructionHistory", "ch", T::Bool, Value::Null, "Accepted."))
        .flag(fl("name", "n", T::String, Value::Null, "Accepted.")),
    );
}
