//! The reference's transform commands with their full flag sets: xform, move, rotate, scale, makeIdentity, transformLimits.
//!
//! move, rotate and scale keep their `by=` and `to=` flags, which the MEL and Python layers fill from the positional
//! values: `to` with `relative` on means `by`. Values given fewer than three (`move 5 obj -x`) stay positional.

use aztek_ops::{fail, objects, ArgType, Args, CmdResult, Commands, FlagDef, RunFn};
use glam::{DMat3, DMat4, DVec3};
use serde_json::{json, Value};

use crate::graph::Graph;
use crate::ref_core::{extend, f};
use crate::plug::{to_json, NodeId, PlugRef, PlugValue};
use crate::session::Session;
use crate::transform::{decompose_full, euler_to_matrix, matrix_to_euler, shear_matrix, RotateOrder, Transform};
use crate::transform_ops::{self as xf, Space};

const ORDERS: [&str; 6] = ["xyz", "yzx", "zxy", "xzy", "yxz", "zyx"];

fn vec3_of(v: &Value) -> Option<DVec3> {
    let a = v.as_array()?;
    (a.len() == 3).then(|| Some(DVec3::new(a[0].as_f64()?, a[1].as_f64()?, a[2].as_f64()?)))?
}

/// A flag given as a value on a set: (x, y, z).
fn set_vec(a: &Args, flag: &str) -> CmdResult<Option<DVec3>> {
    match a.get(flag) {
        None | Some(Value::Bool(_)) => Ok(None),
        Some(v) => vec3_of(v).map(Some).ok_or_else(|| aztek_ops::CommandError::new(format!("xform: {flag} needs (x, y, z)"))),
    }
}

fn put(g: &mut Graph, id: NodeId, attr: usize, v: DVec3) -> CmdResult<()> {
    Ok(g.set_value(PlugRef::new(id, attr as i32), v.into())?)
}

fn j3(v: DVec3) -> Value {
    json!([v.x, v.y, v.z])
}

fn order_of(g: &Graph, id: NodeId) -> RotateOrder {
    RotateOrder::from_index(g.node(id).unwrap().int(Transform::ROTATE_ORDER))
}

/// Terms of the local matrix around the pivots: R * Ra as the rotate pivot sees it, shear * S as the scale pivot does.
fn pivot_terms(g: &Graph, id: NodeId) -> (DMat4, DMat4) {
    let n = g.node(id).unwrap();
    let d = n.data();
    let r = euler_to_matrix(d.vec3(Transform::ROTATE), order_of(g, id));
    let axis = euler_to_matrix(d.vec3(Transform::ROTATE_AXIS), RotateOrder::Xyz);
    let s = shear_matrix(d.vec3(Transform::SHEAR)) * DMat4::from_scale(d.vec3(Transform::SCALE));
    (r * axis, s)
}

/// Moves one pivot in object space, with its compensation so the object stays put.
fn set_pivot(g: &mut Graph, id: NodeId, rotate: Option<DVec3>, scale: Option<DVec3>) -> CmdResult<()> {
    let (r, s) = pivot_terms(g, id);
    let n = g.node(id).unwrap();
    let (rp, sp, rpt, spt) = (n.vec3(Transform::ROTATE_PIVOT), n.vec3(Transform::SCALE_PIVOT), n.vec3(Transform::ROTATE_PIVOT_TRANSLATE), n.vec3(Transform::SCALE_PIVOT_TRANSLATE));
    if let Some(new) = scale {
        let delta = sp - new;
        let fix = spt + delta - s.transform_vector3(delta);
        put(g, id, Transform::SCALE_PIVOT_TRANSLATE, fix)?;
        put(g, id, Transform::SCALE_PIVOT, new)?;
    }
    if let Some(new) = rotate {
        let delta = rp - new;
        let fix = rpt + delta - r.transform_vector3(delta);
        put(g, id, Transform::ROTATE_PIVOT_TRANSLATE, fix)?;
        put(g, id, Transform::ROTATE_PIVOT, new)?;
    }
    Ok(())
}

fn xform_query(g: &mut Graph, id: NodeId, a: &Args) -> CmdResult {
    let ws = a.bool("worldSpace");
    g.evaluate();
    let n = g.node(id).unwrap();
    if !n.ty().dag {
        fail!("xform: {} is not in the hierarchy", g.path(id));
    }
    let transform = n.ty().is_a("transform");
    let world = n.matrix(Transform::WORLD_MATRIX);
    let frame = n.matrix(Transform::PARENT_MATRIX);
    // -parentSpace: pivots and bounding boxes in the parent's space, through the object's own matrix.
    let ps = a.bool("parentSpace") && !ws;
    let local_m = n.matrix(Transform::MATRIX);
    if a.has("matrix") {
        return Ok(to_json(&PlugValue::Matrix(if ws { world } else { n.matrix(Transform::MATRIX) })));
    }
    if !transform {
        fail!("xform: {} is not a transform", g.path(id));
    }
    let bound_of = |g: &mut Graph, invisible: bool| -> Value {
        let (mut lo, mut hi) = (DVec3::splat(f64::MAX), DVec3::splat(f64::MIN));
        let mut under = g.descendants(id);
        under.push(id);
        let inv = world.inverse();
        for u in under {
            if !invisible && !crate::transform::effectively_visible(g, u) {
                continue;
            }
            if !crate::dag::is_shape(g, u) {
                continue;
            }
            let local = crate::node_bounds::object_bounds(g, u);
            if !local.valid() {
                continue;
            }
            let w = g.node(u).unwrap().matrix(Transform::WORLD_MATRIX);
            let b = crate::node_bounds::transformed(&local, &if ws { w } else if ps { frame.inverse() * w } else { inv * w });
            lo = lo.min(b.min);
            hi = hi.max(b.max);
        }
        if lo.x > hi.x {
            return json!([0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
        }
        json!([lo.x, lo.y, lo.z, hi.x, hi.y, hi.z])
    };
    if a.has("boundingBox") {
        return Ok(bound_of(g, false));
    }
    if a.has("boundingBoxInvisible") {
        return Ok(bound_of(g, true));
    }
    let n = g.node(id).unwrap();
    let order = order_of(g, id);
    let (t, r, sc, sh) = decompose_full(&world);
    let pivot = |v: DVec3, ws: bool, m: DMat4| if ws { m.transform_point3(v) } else if ps { local_m.transform_point3(v) } else { v };
    let (rp, sp) = (n.vec3(Transform::ROTATE_PIVOT), n.vec3(Transform::SCALE_PIVOT));
    if a.has("translation") {
        return Ok(if ws { j3(t) } else { to_json(n.value(Transform::TRANSLATE)) });
    }
    if a.has("rotation") {
        return Ok(if ws { j3(matrix_to_euler(&r, order)) } else { to_json(n.value(Transform::ROTATE)) });
    }
    if a.has("scale") {
        return Ok(if ws { j3(sc) } else { to_json(n.value(Transform::SCALE)) });
    }
    if a.has("shear") {
        return Ok(if ws { j3(sh) } else { to_json(n.value(Transform::SHEAR)) });
    }
    if a.has("rotatePivot") {
        return Ok(j3(pivot(rp, ws, world)));
    }
    if a.has("scalePivot") {
        return Ok(j3(pivot(sp, ws, world)));
    }
    if a.has("pivots") {
        let (x, y) = (pivot(rp, ws, world), pivot(sp, ws, world));
        return Ok(json!([x.x, x.y, x.z, y.x, y.y, y.z]));
    }
    if a.has("rotateAxis") {
        return Ok(to_json(n.value(Transform::ROTATE_AXIS)));
    }
    if a.has("rotateOrder") {
        return Ok(json!(ORDERS[n.int(Transform::ROTATE_ORDER).clamp(0, 5) as usize]));
    }
    if a.has("rotateTranslation") {
        return Ok(j3(if ws { frame.transform_vector3(n.vec3(Transform::ROTATE_PIVOT_TRANSLATE)) } else { n.vec3(Transform::ROTATE_PIVOT_TRANSLATE) }));
    }
    if a.has("scaleTranslation") {
        return Ok(to_json(n.value(Transform::SCALE_PIVOT_TRANSLATE)));
    }
    fail!("xform: query what?")
}

fn xform(s: &mut Session, a: &Args, prev: &RunFn<Session>) -> CmdResult {
    let named = objects(s, a);
    if named.iter().any(|n| crate::components::is_component_string(n)) {
        return xform_components(s, a, prev);
    }
    let mut ids = Vec::new();
    for n in &named {
        let Some(id) = s.graph.find(n)? else { fail!("xform: no object named {n}") };
        ids.push(id);
    }
    if a.bool("query") {
        let Some(&id) = ids.first() else { fail!("xform: no object") };
        return xform_query(&mut s.graph, id, a);
    }
    let (ws, rel, preserve) = (a.bool("worldSpace"), a.bool("relative"), a.bool("preserve"));
    let ps = a.bool("parentSpace") && !ws;
    for id in ids {
        let g = &mut s.graph;
        if !g.node(id).is_some_and(|n| n.ty().dag) {
            continue;
        }
        g.evaluate();
        if let Some(Value::String(name)) = a.get("rotateOrder") {
            let Some(k) = ORDERS.iter().position(|o| o == name) else { fail!("xform: bad rotate order {name}") };
            // With -p the orientation stays and the angles change to suit.
            if preserve {
                let n = g.node(id).unwrap();
                let m = DMat3::from_mat4(euler_to_matrix(n.vec3(Transform::ROTATE), order_of(g, id)));
                put(g, id, Transform::ROTATE, matrix_to_euler(&m, RotateOrder::from_index(k as i64)))?;
            }
            g.set_value(PlugRef::new(id, Transform::ROTATE_ORDER as i32), PlugValue::Int(k as i64))?;
        }
        if let Some(mj) = a.get("matrix").filter(|v| v.is_array()) {
            let def = crate::plug::AttrDef { ty: crate::plug::PlugType::Matrix, ..Default::default() };
            let mut m = crate::plug::from_json(&def, mj)?.as_matrix();
            if ws {
                m = g.node(id).unwrap().matrix(Transform::PARENT_MATRIX).inverse() * m;
            }
            let (_, _, _, shear) = decompose_full(&m);
            Transform::set_local_matrix(g, id, &m)?;
            put(g, id, Transform::SHEAR, shear)?;
        }
        if let Some(v) = set_vec(a, "rotateAxis")? {
            put(g, id, Transform::ROTATE_AXIS, v)?;
        }
        if let Some(v) = set_vec(a, "rotation")? {
            let n = g.node(id).unwrap();
            let order = order_of(g, id);
            if rel {
                if ws {
                    xf::rotate_by(g, &[id], &DMat3::from_mat4(euler_to_matrix(v, RotateOrder::Xyz)), Space::World)?;
                } else if a.bool("euler") {
                    // -euler: the values add to the XYZ angles there are.
                    let cur = n.vec3(Transform::ROTATE);
                    put(g, id, Transform::ROTATE, cur + v)?;
                } else {
                    // A turn by the angles given: about the object's own axes, or the parent's with -parentSpace.
                    let cur = DMat3::from_mat4(euler_to_matrix(n.vec3(Transform::ROTATE), order));
                    let by = DMat3::from_mat4(euler_to_matrix(v, order));
                    let m = if ps { by * cur } else { cur * by };
                    // Of the angles that give the turn, those nearest the plain sum, as the reference keeps them.
                    let near = n.vec3(Transform::ROTATE) + v;
                    put(g, id, Transform::ROTATE, nearest_euler(&m, order, near))?;
                }
            } else if ws {
                let frame = DMat3::from_mat4(n.matrix(Transform::PARENT_MATRIX));
                let (_, parent_rot, _, _) = decompose_full(&DMat4::from_mat3(frame));
                let world = DMat3::from_mat4(euler_to_matrix(v, order));
                put(g, id, Transform::ROTATE, matrix_to_euler(&(parent_rot.transpose() * world), order))?;
            } else {
                put(g, id, Transform::ROTATE, v)?;
            }
        }
        if let Some(v) = set_vec(a, "scale")? {
            let cur = g.node(id).unwrap().vec3(Transform::SCALE);
            put(g, id, Transform::SCALE, if rel { cur * v } else { v })?;
        }
        if let Some(v) = set_vec(a, "shear")? {
            let cur = g.node(id).unwrap().vec3(Transform::SHEAR);
            put(g, id, Transform::SHEAR, if rel { cur + v } else { v })?;
        }
        if let Some(v) = set_vec(a, "translation")? {
            if rel {
                xf::move_by(g, &[id], v, if ws { Space::World } else if ps { Space::Parent } else { Space::Object })?;
            } else if ws {
                g.evaluate();
                let n = g.node(id).unwrap();
                let at = n.matrix(Transform::WORLD_MATRIX).w_axis.truncate();
                let local = n.matrix(Transform::PARENT_MATRIX).inverse().transform_vector3(v - at);
                let cur = n.vec3(Transform::TRANSLATE);
                put(g, id, Transform::TRANSLATE, cur + local)?;
            } else {
                put(g, id, Transform::TRANSLATE, v)?;
            }
        }
        // Pivots read in object space, or world with -ws.
        let to_local = |g: &mut Graph, v: DVec3| -> DVec3 {
            if ws {
                g.evaluate();
                g.node(id).unwrap().matrix(Transform::WORLD_MATRIX).inverse().transform_point3(v)
            } else if ps {
                g.evaluate();
                g.node(id).unwrap().matrix(Transform::MATRIX).inverse().transform_point3(v)
            } else {
                v
            }
        };
        if let Some(Value::Array(p)) = a.get("pivots") {
            let vals: Vec<f64> = p.iter().filter_map(Value::as_f64).collect();
            if vals.len() >= 3 {
                let at = to_local(g, DVec3::new(vals[0], vals[1], vals[2]));
                set_pivot(g, id, Some(at), Some(at))?;
            }
        }
        if let Some(v) = set_vec(a, "rotatePivot")? {
            let at = to_local(g, v);
            set_pivot(g, id, Some(at), None)?;
        }
        if let Some(v) = set_vec(a, "scalePivot")? {
            let at = to_local(g, v);
            set_pivot(g, id, None, Some(at))?;
        }
        if let Some(v) = set_vec(a, "rotateTranslation")? {
            put(g, id, Transform::ROTATE_PIVOT_TRANSLATE, v)?;
        }
        if let Some(v) = set_vec(a, "scaleTranslation")? {
            put(g, id, Transform::SCALE_PIVOT_TRANSLATE, v)?;
        }
        if a.bool("centerPivots") || a.bool("centerPivotsOnComponents") {
            let path = g.path(id);
            aztek_ops::run(s, "centerPivot", json!({"args": [path]}))?;
        } else if a.bool("zeroTransformPivots") {
            let path = g.path(id);
            aztek_ops::run(s, "resetPivot", json!({"args": [path]}))?;
        }
    }
    Ok(Value::Null)
}

/// The Euler angles for a rotation nearest to `near`: of the two solutions, each angle taken a whole number of turns
/// towards it.
fn nearest_euler(m: &DMat3, order: RotateOrder, near: DVec3) -> DVec3 {
    use std::f64::consts::{PI, TAU};
    let e = matrix_to_euler(m, order);
    let a = order.axes();
    let mut alt = e;
    alt[a[0]] += PI;
    alt[a[1]] = PI - alt[a[1]];
    alt[a[2]] += PI;
    let wrap = |v: DVec3| DVec3::new(v.x + ((near.x - v.x) / TAU).round() * TAU, v.y + ((near.y - v.y) / TAU).round() * TAU, v.z + ((near.z - v.z) / TAU).round() * TAU);
    let (e, alt) = (wrap(e), wrap(alt));
    if (alt - near).length_squared() < (e - near).length_squared() {
        alt
    } else {
        e
    }
}

/// xform on components. With -preserveUV or -reflection (-reflectionAbout*, -reflectionTolerance) a translation goes
/// through move, which carries both out; everything else the older command does.
fn xform_components(s: &mut Session, a: &Args, prev: &RunFn<Session>) -> CmdResult {
    let options = ["preserveUV", "reflection"].iter().any(|k| a.bool(k));
    let Some(t) = a.vec3("translation").filter(|_| options && !a.bool("query")) else { return prev(s, a) };
    let named: Vec<Value> = a.named().into_iter().filter(|n| crate::components::is_component_string(n)).map(Value::String).collect();
    let mut call = serde_json::Map::new();
    call.insert("args".into(), Value::Array(named));
    call.insert(if a.bool("relative") { "by" } else { "to" }.into(), json!(t));
    call.insert(if a.bool("worldSpace") { "worldSpace" } else { "objectSpace" }.into(), json!(true));
    for k in ["preserveUV", "reflection", "reflectionAboutBBox", "reflectionAboutOrigin", "reflectionAboutX", "reflectionAboutY", "reflectionAboutZ", "reflectionTolerance"] {
        if let Some(v) = a.get(k) {
            call.insert(k.into(), v.clone());
        }
    }
    s.run("move", Value::Object(call))?;
    Ok(Value::Null)
}

/// World matrices of a node's transform children, to put back after it moves (preserveChildPosition).
fn child_worlds(g: &mut Graph, ids: &[NodeId]) -> Vec<(NodeId, DMat4)> {
    g.evaluate();
    let mut out = Vec::new();
    for &id in ids {
        for &c in g.children(id) {
            if crate::dag::is_transform(g, c) {
                out.push((c, g.node(c).unwrap().matrix(Transform::WORLD_MATRIX)));
            }
        }
    }
    out
}

fn restore_worlds(g: &mut Graph, saved: Vec<(NodeId, DMat4)>) -> CmdResult<()> {
    g.evaluate();
    for (c, world) in saved {
        let frame = g.node(c).unwrap().matrix(Transform::PARENT_MATRIX);
        Transform::set_local_matrix(g, c, &(frame.inverse() * world))?;
    }
    Ok(())
}

/// One move, rotate or scale call, read the way the reference reads it.
struct Motion {
    values: DVec3,
    /// Which axes the values are for.
    mask: [bool; 3],
    relative: bool,
}

/// Reads the values and which axes they are for, from by=, to=, x=, the axis switches and positional numbers.
fn read_motion(a: &Args, cmd: &str, base: f64) -> CmdResult<Option<Motion>> {
    let relative = a.bool("relative") || a.has("by");
    let axis_switch = |flag: &str| a.get(flag).is_some_and(|v| v == &Value::Bool(true));
    let mut mask = [axis_switch("x"), axis_switch("y"), axis_switch("z")];
    let long = |suffix: &str| a.bool(&format!("{cmd}{suffix}"));
    if long("XY") {
        mask = [true, true, false];
    } else if long("XZ") {
        mask = [true, false, true];
    } else if long("YZ") {
        mask = [false, true, true];
    } else if long("XYZ") {
        mask = [true, true, true];
    }
    for (i, l) in ["X", "Y", "Z"].iter().enumerate() {
        if long(l) {
            mask[i] = true;
        }
    }
    // x=5 and friends, the older form and MEL's -x 5.
    let mut own = [None, None, None];
    for (i, flag) in ["x", "y", "z"].into_iter().enumerate() {
        own[i] = a.get(flag).and_then(Value::as_f64);
    }
    let any_own = own.iter().any(Option::is_some);
    let given: Vec<f64> = match a.get("to").or(a.get("by")) {
        Some(Value::Array(v)) => v.iter().filter_map(Value::as_f64).collect(),
        _ => a.positional.iter().filter_map(Value::as_f64).collect(),
    };
    if any_own {
        let mut values = DVec3::splat(if relative { base } else { f64::NAN });
        for i in 0..3 {
            if let Some(x) = own[i] {
                values[i] = x;
                mask[i] = true;
            }
        }
        return Ok(Some(Motion { values, mask, relative }));
    }
    if given.is_empty() {
        return Ok(None);
    }
    if mask == [false; 3] {
        if given.len() != 3 {
            fail!("{cmd}: give three values, or say which axes with -x, -y, -z");
        }
        mask = [true; 3];
    }
    let mut values = DVec3::splat(base);
    let mut it = given.into_iter();
    for i in 0..3 {
        if mask[i] {
            let Some(x) = it.next() else { fail!("{cmd}: too few values for the axes named") };
            values[i] = x;
        }
    }
    Ok(Some(Motion { values, mask, relative }))
}

/// What the older command understands: only names as positionals.
fn plain_args(a: &Args, flags: Value) -> Args {
    let mut out = Args { positional: a.named().into_iter().map(Value::String).collect(), flags: Default::default() };
    if let Value::Object(o) = flags {
        out.flags = o;
    }
    out
}

/// makeIdentity -jointOrient: each joint named and below, parents first, turns to line up with world space (rotate,
/// rotateAxis and jointOrient set so), its children kept where they are.
fn joint_orient_to_world(s: &mut Session, a: &Args) -> CmdResult<()> {
    let mut joints: Vec<NodeId> = Vec::new();
    for id in transforms_named(s, a, "makeIdentity")? {
        for n in std::iter::once(id).chain(s.graph.descendants(id)) {
            if crate::rig::is_joint(&s.graph, n) && !joints.contains(&n) {
                joints.push(n);
            }
        }
    }
    // Where everything was, before any joint turns.
    s.graph.evaluate();
    let mut was: std::collections::HashMap<NodeId, DMat4> = std::collections::HashMap::new();
    for &j in &joints {
        for c in s.graph.children(j).to_vec() {
            if s.graph.node(c).is_some_and(|n| n.ty().is_a("transform")) {
                let w = crate::rig::world(&mut s.graph, c);
                was.insert(c, w);
            }
        }
    }
    for j in joints {
        s.graph.evaluate();
        let kids: Vec<(NodeId, DMat4)> = s.graph.children(j).iter().filter_map(|c| was.get(c).map(|w| (*c, *w))).collect();
        let parent = crate::rig::parent_world(&mut s.graph, j);
        let (_, prot, _) = parent.to_scale_rotation_translation();
        let orient = matrix_to_euler(&DMat3::from_quat(prot.inverse()), RotateOrder::from_index(0));
        put(&mut s.graph, j, Transform::ROTATE, DVec3::ZERO)?;
        put(&mut s.graph, j, Transform::ROTATE_AXIS, DVec3::ZERO)?;
        put(&mut s.graph, j, crate::rig::Joint::JOINT_ORIENT, orient)?;
        s.graph.evaluate();
        let inv = crate::rig::world(&mut s.graph, j).inverse();
        for (c, w) in kids {
            if crate::rig::is_joint(&s.graph, c) {
                let t = inv.transform_point3(w.w_axis.truncate());
                put(&mut s.graph, c, Transform::TRANSLATE, t)?;
            } else {
                let path = s.graph.path(c);
                let m: Vec<f64> = w.to_cols_array().to_vec();
                s.run("xform", json!({"args": [path], "worldSpace": true, "matrix": m}))?;
            }
        }
    }
    s.graph.evaluate();
    Ok(())
}

fn transforms_named(s: &Session, a: &Args, cmd: &str) -> CmdResult<Vec<NodeId>> {
    let mut ids = Vec::new();
    for name in objects(s, a) {
        let Some(id) = s.graph.find(&name)? else { fail!("{cmd}: no node named '{name}'") };
        if crate::dag::is_transform(&s.graph, id) {
            ids.push(id);
        }
    }
    if ids.is_empty() {
        fail!("{cmd}: nothing to transform");
    }
    Ok(ids)
}

fn space_of(a: &Args) -> Space {
    if a.bool("objectSpace") {
        Space::Object
    } else if a.bool("localSpace") || a.bool("parent") {
        Space::Parent
    } else {
        match a.string("space", "world").as_str() {
            "object" => Space::Object,
            "parent" => Space::Parent,
            _ => Space::World,
        }
    }
}

fn space_name(sp: Space) -> &'static str {
    match sp {
        Space::Object => "object",
        Space::Parent => "parent",
        _ => "world",
    }
}

fn is_component_call(a: &Args) -> bool {
    a.named().iter().any(|n| crate::components::is_component_string(n))
}

/// -reflection on move, rotate and scale: the meshes named or picked take the symmetry asked for (-reflectionAboutX, Y or Z
/// through the origin or the bounding box, -reflectionTolerance, -symNegative) for this one edit. Returns what to put
/// back.
fn reflection_on(s: &mut Session, a: &Args) -> CmdResult<Vec<(NodeId, i64, i64, f64)>> {
    let mut saved: Vec<(NodeId, i64, i64, f64)> = Vec::new();
    if a.bool("reflection") {
        use crate::mesh_nodes::MeshNode;
        let axis = if a.bool("reflectionAboutY") {
            1
        } else if a.bool("reflectionAboutZ") {
            2
        } else {
            0
        };
        let about_box = a.bool("reflectionAboutBBox") && !a.bool("reflectionAboutOrigin");
        let mut meshes: Vec<NodeId> = Vec::new();
        for n in a.named() {
            if let Some(m) = s.graph.find(n.split('.').next().unwrap_or(&n)).ok().flatten().and_then(|id| crate::components::as_mesh(&s.graph, id)) {
                meshes.push(m);
            }
        }
        if a.named().is_empty() {
            for (&node, _) in crate::components::selected_all(&s.graph, crate::components::ComponentMode::Vertex) {
                meshes.push(node);
            }
            for &id in s.graph.selection() {
                if let Some(m) = crate::components::as_mesh(&s.graph, id) {
                    meshes.push(m);
                }
            }
        }
        meshes.sort_unstable();
        meshes.dedup();
        for m in meshes {
            let g = &mut s.graph;
            let n = g.node(m).unwrap();
            saved.push((m, n.int(MeshNode::SYMMETRY), n.int(MeshNode::SYMMETRY_PLANE), n.float(MeshNode::SYMMETRY_OFFSET)));
            let (plane, offset) = if about_box {
                let b = crate::modifiers::cage(g, m).map(|x| x.bounds()).unwrap_or_default();
                (1, b.center()[axis])
            } else {
                (crate::mesh_nodes::SYMMETRY_PLANE_WORLD, 0.0)
            };
            g.set_value(PlugRef::new(m, MeshNode::SYMMETRY as i32), PlugValue::Int(axis as i64 + 1))?;
            g.set_value(PlugRef::new(m, MeshNode::SYMMETRY_PLANE as i32), PlugValue::Int(plane))?;
            g.set_value(PlugRef::new(m, MeshNode::SYMMETRY_OFFSET as i32), PlugValue::Float(offset))?;
            crate::components::set_picked_wedge(g, m, a.bool("symNegative") as i32);
        }
        if let Some(t) = a.get("reflectionTolerance").and_then(Value::as_f64) {
            crate::mesh_edit::set_symmetry_tolerance(Some(t));
        }
    }
    Ok(saved)
}

/// The meshes' own symmetry back after a -reflection edit.
fn reflection_off(s: &mut Session, saved: Vec<(NodeId, i64, i64, f64)>) -> CmdResult<()> {
    crate::mesh_edit::set_symmetry_tolerance(None);
    for (m, sym, plane, offset) in saved {
        use crate::mesh_nodes::MeshNode;
        let g = &mut s.graph;
        g.set_value(PlugRef::new(m, MeshNode::SYMMETRY as i32), PlugValue::Int(sym))?;
        g.set_value(PlugRef::new(m, MeshNode::SYMMETRY_PLANE as i32), PlugValue::Int(plane))?;
        g.set_value(PlugRef::new(m, MeshNode::SYMMETRY_OFFSET as i32), PlugValue::Float(offset))?;
    }
    Ok(())
}

/// The middle of the world bounding box of some objects.
fn box_centre(s: &mut Session, ids: &[NodeId]) -> CmdResult<DVec3> {
    let paths: Vec<Value> = ids.iter().map(|&i| json!(s.graph.path(i))).collect();
    let b = s.run("exactWorldBoundingBox", json!({"args": paths}))?;
    let v: Vec<f64> = b.as_array().into_iter().flatten().filter_map(Value::as_f64).collect();
    if v.len() < 6 {
        return Ok(DVec3::ZERO);
    }
    Ok(DVec3::new(v[0] + v[3], v[1] + v[4], v[2] + v[5]) * 0.5)
}

/// The axes -orientAxes names, as a turn from the world's.
fn orient_of(a: &Args) -> Option<DMat3> {
    a.vec3("orientAxes").map(|o| DMat3::from_mat4(euler_to_matrix(DVec3::from_array(o), RotateOrder::Xyz)))
}

/// An object moved round a pivot by `delta` (world): into translate with -translate, else into the rotate pivot's
/// translation, as the reference's rotate does.
fn pivot_offset(g: &mut Graph, id: NodeId, delta: DVec3, into_translate: bool) -> CmdResult<()> {
    if into_translate {
        xf::move_by(g, &[id], delta, Space::World)?;
        return Ok(());
    }
    g.evaluate();
    let n = g.node(id).unwrap();
    let local = (n.matrix(Transform::PARENT_MATRIX).inverse() * glam::DVec4::from((delta, 0.0))).truncate();
    let rpt = n.vec3(Transform::ROTATE_PIVOT_TRANSLATE) + local;
    put(g, id, Transform::ROTATE_PIVOT_TRANSLATE, rpt)
}

/// rotate and scale on components named, with their component options: -reflection, -preserveUV, -pivot (or the
/// components' middle) and -orientAxes. The components named are picked for the edit and the selection put back.
fn component_motion(s: &mut Session, a: &Args, cmd: &str, values: DVec3) -> CmdResult {
    let saved = reflection_on(s, a)?;
    let keep_uv = s.graph.move_opts.preserve_uv;
    if a.bool("preserveUV") {
        s.graph.move_opts.preserve_uv = true;
    }
    let named: Vec<String> = a.named().into_iter().filter(|n| crate::components::is_component_string(n)).collect();
    let before: Vec<String> = s.graph.selection().iter().map(|&id| s.graph.path(id)).collect();
    let before_components = crate::components::component_strings(&s.graph, false);
    let r = (|| -> CmdResult {
        if !named.is_empty() {
            s.run("select", json!({"args": named}))?;
        }
        let mut call = json!({"by": j3(values)});
        if let Some(p) = a.vec3("pivot") {
            call["pivot"] = json!(p);
        }
        if let Some(o) = a.vec3("orientAxes") {
            call["orient"] = json!(o);
        }
        s.run(if cmd == "rotate" { "rotateComponents" } else { "scaleComponents" }, call)?;
        Ok(Value::Null)
    })();
    if !named.is_empty() {
        let back: Vec<String> = if before_components.is_empty() { before } else { before_components };
        let _ = if back.is_empty() { s.run("select", json!({"clear": true})) } else { s.run("select", json!({"args": back})) };
    }
    s.graph.move_opts.preserve_uv = keep_uv;
    reflection_off(s, saved)?;
    r
}

/// move with the reference's component options around the move itself: -preserveUV, -reflection (with -reflectionAbout*,
/// -reflectionTolerance and -symNegative), -parameter, and -orientJoint for joints moved.
fn run_move(s: &mut Session, a: &Args, prev: &RunFn<Session>) -> CmdResult {
    if a.bool("parameter") {
        return move_parametric(s, a);
    }
    let reflect = a.bool("reflection");
    let keep_uv = s.graph.move_opts.preserve_uv;
    if a.bool("preserveUV") {
        s.graph.move_opts.preserve_uv = true;
    }
    let saved = reflection_on(s, a)?;
    // Symmetry follows a move of the components picked: those named are picked for it, and the selection put back after.
    let named_components: Vec<String> = a.named().into_iter().filter(|n| crate::components::is_component_string(n)).collect();
    let r = if reflect && !named_components.is_empty() {
        let before: Vec<String> = s.graph.selection().iter().map(|&id| s.graph.path(id)).collect();
        let before_components = crate::components::component_strings(&s.graph, false);
        s.run("select", json!({"args": named_components}))?;
        let mut b = a.clone();
        let is_comp = |v: &Value| v.as_str().is_some_and(crate::components::is_component_string);
        b.positional.retain(|v| !is_comp(v));
        for v in b.positional.iter_mut() {
            if let Value::Array(list) = v {
                list.retain(|x| !is_comp(x));
            }
        }
        b.positional.retain(|v| !v.as_array().is_some_and(Vec::is_empty));
        // The components picked move, by the older command on the selection: not their objects.
        let r = match read_motion(&b, "move", 0.0) {
            Ok(Some(m)) => {
                let key = if m.relative { "by" } else { "to" };
                prev(s, &plain_args(a, json!({key: j3(m.values)})))
            }
            Ok(None) => run_move_inner(s, &b, prev),
            Err(e) => Err(e),
        };
        let back: Vec<String> = if before_components.is_empty() { before } else { before_components };
        let _ = if back.is_empty() { s.run("select", json!({"clear": true})) } else { s.run("select", json!({"args": back})) };
        r
    } else {
        run_move_inner(s, a, prev)
    };
    s.graph.move_opts.preserve_uv = keep_uv;
    reflection_off(s, saved)?;
    let r = r?;
    // -orientJoint: joints moved turn to aim along the order given, and so does the joint above each.
    if let Some(order) = a.get("orientJoint").and_then(Value::as_str).map(str::to_string) {
        let joints: Vec<NodeId> = transforms_named(s, a, "move").unwrap_or_default().into_iter().filter(|&j| crate::rig::is_joint(&s.graph, j)).collect();
        let mut done: Vec<NodeId> = Vec::new();
        for j in joints {
            let p = s.graph.parent(j);
            for x in [p, j] {
                if x == crate::plug::NO_NODE || !crate::rig::is_joint(&s.graph, x) || done.contains(&x) {
                    continue;
                }
                done.push(x);
                let mut call = json!({"args": [s.graph.path(x)], "edit": true, "orientJoint": order, "children": false});
                if let Some(sao) = a.get("secondaryAxisOrient") {
                    call["secondaryAxisOrient"] = sao.clone();
                }
                if a.bool("autoOrientSecondaryAxis") {
                    call["autoOrientSecondaryAxis"] = json!(true);
                }
                s.run("joint", call)?;
            }
        }
    }
    Ok(r)
}

/// move -parameter: curve points moved along the curve in parameter space. Each CV or edit point named moves by the
/// first value in parameter (along the curve, the distance a parameter step covers there), keeping on the curve.
fn move_parametric(s: &mut Session, a: &Args) -> CmdResult {
    let Some(m) = read_motion(a, "move", 0.0)? else { fail!("move: -parameter takes the amount to move by") };
    let names: Vec<String> = if a.named().is_empty() { crate::components::component_strings(&s.graph, false) } else { a.named() };
    s.graph.open_chunk("move -parameter");
    let r = (|| -> CmdResult<()> {
        for n in names {
            let Some((obj, rest)) = n.split_once('.') else { continue };
            // curve.u[p] or curve.ep[i] / curve.cv[i]: the point slides by the value along the curve.
            let Some(id) = s.graph.find(obj)? else { continue };
            let Some(shape) = crate::curves::as_curve(&s.graph, id) else { fail!("move: -parameter works on curve points; {n} isn't one") };
            let Some(c) = crate::curves::local_curve(&mut s.graph, shape) else { continue };
            let (lo, hi) = c.domain();
            let idx = |t: &str| t.trim_end_matches(']').split('[').nth(1).and_then(|x| x.parse::<f64>().ok());
            let Some(i) = idx(rest) else { continue };
            let path = format!("{obj}.{rest}");
            // A CV slides by how far its place on the curve (the mean of its knots) moves; an edit point or curve point
            // moves to the curve at its new parameter.
            let (at, cv) = if rest.starts_with("cv[") {
                let k = &c.knots;
                let d = c.degree.max(1);
                let i = i as usize;
                let g = if i + d <= k.len() { k[i..i + d].iter().sum::<f64>() / d as f64 } else { hi };
                (g, true)
            } else if rest.starts_with("ep[") {
                (c.distinct_params().get(i as usize).copied().unwrap_or(lo), false)
            } else {
                (i, false)
            };
            let to = (at + m.values.x).clamp(lo, hi);
            if cv {
                let d = c.eval(to) - c.eval(at);
                s.run("move", json!({"args": [path], "relative": true, "objectSpace": true, "x": d.x, "y": d.y, "z": d.z}))?;
            } else {
                let p = c.eval(to);
                s.run("move", json!({"args": [path], "absolute": true, "objectSpace": true, "x": p.x, "y": p.y, "z": p.z}))?;
            }
        }
        Ok(())
    })();
    s.graph.close_chunk();
    r?;
    Ok(Value::Null)
}

fn run_move_inner(s: &mut Session, a: &Args, prev: &RunFn<Session>) -> CmdResult {
    let Some(m) = read_motion(a, "move", 0.0)? else { return prev(s, a) };
    // -componentOffset with -localSpace: each component in its own space, the first value along its normal, as the
    // reference's Ctrl+middle drag echoes it (#102).
    if a.bool("componentOffset") && m.relative && (a.bool("localSpace") || a.bool("objectSpace")) {
        let named: Vec<String> = a.named().into_iter().filter(|n| crate::components::is_component_string(n)).collect();
        let before: Vec<String> = s.graph.selection().iter().map(|&id| s.graph.path(id)).collect();
        let before_components = crate::components::component_strings(&s.graph, false);
        if !named.is_empty() {
            s.run("select", json!({"args": named}))?;
        }
        let r = s.run("moveComponents", json!({"normal": m.values.x}));
        if !named.is_empty() {
            let back: Vec<String> = if before_components.is_empty() { before } else { before_components };
            let _ = if back.is_empty() { s.run("select", json!({"clear": true})) } else { s.run("select", json!({"args": back})) };
        }
        return r.map(|_| Value::Null);
    }
    if is_component_call(a) && !m.relative {
        return move_components_to(s, a, &m);
    }
    if is_component_call(a) {
        let key = if m.relative { "by" } else { "to" };
        return prev(s, &plain_args(a, json!({key: j3(m.values)})));
    }
    let mut saved = Vec::new();
    if a.bool("preserveChildPosition") {
        let ids = transforms_named(s, a, "move")?;
        saved = child_worlds(&mut s.graph, &ids);
    }
    let space = space_of(a);
    if m.relative {
        // Axes not named don't move.
        let by: Vec<f64> = (0..3).map(|i| if m.mask[i] { m.values[i] } else { 0.0 }).collect();
        let moved = transforms_named(s, a, "move").unwrap_or_default();
        let followers = soft_followers(s, &moved);
        prev(s, &plain_args(a, json!({"by": by, "space": space_name(space)})))?;
        for (id, w, _) in followers {
            xf::move_by(&mut s.graph, &[id], DVec3::new(by[0], by[1], by[2]) * w, Space::World)?;
        }
    } else if a.bool("localSpace") || a.bool("objectSpace") || a.bool("parent") {
        for id in transforms_named(s, a, "move")? {
            let mut t = s.graph.node(id).unwrap().vec3(Transform::TRANSLATE);
            for i in 0..3 {
                if m.mask[i] {
                    t[i] = m.values[i];
                }
            }
            put(&mut s.graph, id, Transform::TRANSLATE, t)?;
        }
    } else if m.mask == [true; 3] {
        prev(s, &plain_args(a, json!({"to": j3(m.values)})))?;
    } else {
        let mut flags = serde_json::Map::new();
        for (i, k) in ["x", "y", "z"].into_iter().enumerate() {
            if m.mask[i] {
                flags.insert(k.into(), json!(m.values[i]));
            }
        }
        prev(s, &plain_args(a, Value::Object(flags)))?;
    }
    if !saved.is_empty() {
        restore_worlds(&mut s.graph, saved)?;
    }
    Ok(Value::Null)
}

/// An absolute move of components: every point named goes to the position given, on the axes given, in world space
/// or with -objectSpace (or -localSpace) in its object's space.
fn move_components_to(s: &mut Session, a: &Args, m: &Motion) -> CmdResult {
    let local = a.bool("objectSpace") || a.bool("localSpace");
    let named: Vec<String> = a.named().into_iter().filter(|n| crate::components::is_component_string(n)).collect();
    let mut points: Vec<String> = Vec::new();
    for n in &named {
        let l = s.run("ls", json!({"args": [n], "flatten": true}))?;
        points.extend(l.as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_string));
    }
    let space = if local { "objectSpace" } else { "worldSpace" };
    s.graph.open_chunk("move");
    let r = (|| -> CmdResult<()> {
        if m.mask == [true; 3] {
            s.run("xform", json!({"args": points, space: true, "translation": j3(m.values)}))?;
            return Ok(());
        }
        for p in &points {
            let at = s.run("xform", json!({"args": [p], "query": true, space: true, "translation": true}))?;
            let mut v: Vec<f64> = at.as_array().into_iter().flatten().filter_map(Value::as_f64).collect();
            if v.len() < 3 {
                continue;
            }
            for i in 0..3 {
                if m.mask[i] {
                    v[i] = m.values[i];
                }
            }
            s.run("xform", json!({"args": [p], space: true, "translation": [v[0], v[1], v[2]]}))?;
        }
        Ok(())
    })();
    s.graph.close_chunk();
    r?;
    Ok(Value::Null)
}

/// Soft select's Object mode: objects near the moved ones, as (object, its share, the moved pivot nearest it). They go along
/// without deforming.
fn soft_followers(s: &mut Session, moved: &[NodeId]) -> Vec<(NodeId, f64, DVec3)> {
    let (on, radius) = crate::soft_select::state();
    if !on || radius <= 0.0 || crate::soft_select::falloff_name() != "object" || moved.is_empty() {
        return Vec::new();
    }
    let g = &mut s.graph;
    let curve = crate::soft_select::current_curve();
    let seeds: Vec<DVec3> = moved.iter().map(|&id| xf::world_pivot(g, id)).collect();
    let mut out = Vec::new();
    for id in g.nodes() {
        let Some(n) = g.node(id) else { continue };
        if !n.ty().is_a("transform") || moved.contains(&id) || crate::cameras::is_default_camera(g, id) {
            continue;
        }
        // What moves with a parent already goes with it, and a parent of what moved stays.
        let parent = g.parent(id);
        if parent != crate::plug::NO_NODE {
            continue;
        }
        if moved.iter().any(|&m| g.descendants(id).contains(&m)) {
            continue;
        }
        let at = xf::world_pivot(g, id);
        let Some(near) = seeds.iter().copied().min_by(|a, b| (*a - at).length().total_cmp(&(*b - at).length())) else { continue };
        let d = (near - at).length();
        if d < radius {
            let w = curve.evaluate(d / radius);
            if w > 0.0 {
                out.push((id, w, near));
            }
        }
    }
    out
}

fn run_rotate(s: &mut Session, a: &Args, prev: &RunFn<Session>) -> CmdResult {
    let Some(m) = read_motion(a, "rotate", 0.0)? else { return prev(s, a) };
    let orient = orient_of(a);
    if is_component_call(a) {
        if ["reflection", "preserveUV", "orientAxes"].iter().any(|k| a.has(k)) {
            let values = DVec3::new(if m.mask[0] { m.values.x } else { 0.0 }, if m.mask[1] { m.values.y } else { 0.0 }, if m.mask[2] { m.values.z } else { 0.0 });
            return component_motion(s, a, "rotate", values);
        }
        return prev(s, &plain_args(a, json!({"by": j3(m.values)})));
    }
    let ids = transforms_named(s, a, "rotate")?;
    let saved = if a.bool("preserveChildPosition") { child_worlds(&mut s.graph, &ids) } else { Vec::new() };
    // -pivot, or -centerPivot the middle of everything's bounding box; -objectCenterPivot each object's own middle.
    let mut pivot = a.vec3("pivot").map(DVec3::from_array);
    if a.bool("centerPivot") {
        pivot = Some(box_centre(s, &ids)?);
    }
    let per_object = a.bool("objectCenterPivot");
    let into_translate = a.bool("translate");
    let values = DVec3::new(if m.mask[0] { m.values.x } else { 0.0 }, if m.mask[1] { m.values.y } else { 0.0 }, if m.mask[2] { m.values.z } else { 0.0 });
    if m.relative {
        // About a point: the object's place turns around it too. -orientAxes: the angles are about those axes.
        let own = DMat3::from_mat4(euler_to_matrix(values, RotateOrder::Xyz));
        let turn = orient.map_or(own, |o| o * own * o.transpose());
        // Only the tops of what is picked travel round the point; those under them go with them.
        for id in xf::top_level(&s.graph, &ids) {
            let p = if per_object { Some(box_centre(s, &[id])?) } else { pivot };
            if let Some(p) = p {
                let at = xf::world_pivot(&mut s.graph, id);
                let moved = turn * (at - p) + p - at;
                pivot_offset(&mut s.graph, id, moved, into_translate)?;
            }
        }
        let followers = soft_followers(s, &ids);
        if orient.is_some() {
            xf::rotate_by(&mut s.graph, &ids, &turn, Space::World)?;
        } else {
            prev(s, &plain_args(a, json!({"by": j3(values), "space": space_name(space_of(a))})))?;
        }
        for (id, w, near) in followers {
            let share = DMat3::from_mat4(euler_to_matrix(values * w, RotateOrder::Xyz));
            let at = xf::world_pivot(&mut s.graph, id);
            xf::move_by(&mut s.graph, &[id], share * (at - near) + near - at, Space::World)?;
            xf::rotate_by(&mut s.graph, &[id], &share, Space::World)?;
        }
    } else {
        for &id in &ids {
            let g = &mut s.graph;
            g.evaluate();
            let n = g.node(id).unwrap();
            let order = order_of(g, id);
            let mut r = n.vec3(Transform::ROTATE);
            if a.bool("worldSpace") {
                // The object's world orientation becomes these angles.
                let (_, parent, _, _) = decompose_full(&n.matrix(Transform::PARENT_MATRIX));
                let (_, now, _, _) = decompose_full(&n.matrix(Transform::WORLD_MATRIX));
                let mut world = matrix_to_euler(&now, order);
                for i in 0..3 {
                    if m.mask[i] {
                        world[i] = m.values[i];
                    }
                }
                r = matrix_to_euler(&(parent.transpose() * DMat3::from_mat4(euler_to_matrix(world, order))), order);
            } else {
                for i in 0..3 {
                    if m.mask[i] {
                        r[i] = m.values[i];
                    }
                }
            }
            let p = PlugRef::new(id, Transform::ROTATE as i32);
            let v = g.keep_locked(&p, r.into());
            g.set_value(p, v)?;
        }
    }
    if !saved.is_empty() {
        restore_worlds(&mut s.graph, saved)?;
    }
    Ok(Value::Null)
}

fn run_scale(s: &mut Session, a: &Args, prev: &RunFn<Session>) -> CmdResult {
    let Some(m) = read_motion(a, "scale", 1.0)? else { return prev(s, a) };
    let orient = orient_of(a);
    if is_component_call(a) {
        if ["reflection", "preserveUV", "orientAxes"].iter().any(|k| a.has(k)) {
            let by = DVec3::new(if m.mask[0] { m.values.x } else { 1.0 }, if m.mask[1] { m.values.y } else { 1.0 }, if m.mask[2] { m.values.z } else { 1.0 });
            return component_motion(s, a, "scale", by);
        }
        return prev(s, &plain_args(a, json!({"by": j3(m.values)})));
    }
    let ids = transforms_named(s, a, "scale")?;
    let saved = if a.bool("preserveChildPosition") { child_worlds(&mut s.graph, &ids) } else { Vec::new() };
    let mut pivot = a.vec3("pivot").map(DVec3::from_array);
    if a.bool("centerPivot") {
        pivot = Some(box_centre(s, &ids)?);
    }
    let per_object = a.bool("objectCenterPivot");
    if m.relative {
        let by = DVec3::new(if m.mask[0] { m.values.x } else { 1.0 }, if m.mask[1] { m.values.y } else { 1.0 }, if m.mask[2] { m.values.z } else { 1.0 });
        // -orientAxes: a scale along other axes than the objects' own, set as each object's whole world matrix.
        if let Some(o) = orient {
            let sc = o * DMat3::from_diagonal(by) * o.transpose();
            for id in xf::top_level(&s.graph, &ids) {
                let p = if per_object { box_centre(s, &[id])? } else { pivot.unwrap_or_else(|| xf::world_pivot(&mut s.graph, id)) };
                s.graph.evaluate();
                let w = s.graph.node(id).unwrap().matrix(Transform::WORLD_MATRIX);
                let change = DMat4::from_translation(p) * DMat4::from_mat3(sc) * DMat4::from_translation(-p);
                let next = change * w;
                let path = s.graph.path(id);
                s.run("xform", json!({"args": [path], "worldSpace": true, "matrix": next.to_cols_array().to_vec()}))?;
            }
            if !saved.is_empty() {
                restore_worlds(&mut s.graph, saved)?;
            }
            return Ok(Value::Null);
        }
        for id in xf::top_level(&s.graph, &ids) {
            let p = if per_object { Some(box_centre(s, &[id])?) } else { pivot };
            if let Some(p) = p {
                let at = xf::world_pivot(&mut s.graph, id);
                xf::move_by(&mut s.graph, &[id], (at - p) * (by - DVec3::ONE), Space::World)?;
            }
        }
        let followers = soft_followers(s, &ids);
        prev(s, &plain_args(a, json!({"by": j3(by)})))?;
        for (id, w, near) in followers {
            let share = DVec3::ONE + (by - DVec3::ONE) * w;
            let at = xf::world_pivot(&mut s.graph, id);
            xf::move_by(&mut s.graph, &[id], (at - near) * (share - DVec3::ONE), Space::World)?;
            xf::scale_by(&mut s.graph, &[id], share)?;
        }
    } else {
        for &id in &ids {
            let g = &mut s.graph;
            let mut v = g.node(id).unwrap().vec3(Transform::SCALE);
            for i in 0..3 {
                if m.mask[i] {
                    v[i] = m.values[i];
                }
            }
            let p = PlugRef::new(id, Transform::SCALE as i32);
            let v = g.keep_locked(&p, v.into());
            g.set_value(p, v)?;
        }
    }
    if !saved.is_empty() {
        restore_worlds(&mut s.graph, saved)?;
    }
    Ok(Value::Null)
}

/// The switches every one of move, rotate, scale takes, and its axis spellings.
fn motion_flags(cmd: &str) -> Vec<FlagDef> {
    let mut v = vec![
        f("relative", "r", ArgType::Bool, "The values add to what is there."),
        f("absolute", "a", ArgType::Bool, "The values replace what is there (the default)."),
        f("worldSpace", "ws", ArgType::Bool, "World axes (the default)."),
        f("objectSpace", "os", ArgType::Bool, "The object's own axes."),
        f("localSpace", "ls", ArgType::Bool, "The parent's axes."),
        f("parent", "pa", ArgType::Bool, "The parent's axes."),
        f("worldSpaceDistance", "wd", ArgType::Bool, "Accepted: distances are in world units already."),
        f("preserveChildPosition", "pcp", ArgType::Bool, "Children stay where they are in the world."),
        f("preserveGeometryPosition", "pgp", ArgType::Bool, "Accepted for existing scripts."),
        f("rotatePivotRelative", "rpr", ArgType::Bool, "Accepted for existing scripts."),
        f("scalePivotRelative", "spr", ArgType::Bool, "Accepted for existing scripts."),
        f("deletePriorHistory", "dph", ArgType::Bool, "Accepted for existing scripts."),
        f("xformConstraint", "xc", ArgType::String, "Accepted for existing scripts."),
        f("constrainAlongNormal", "xn", ArgType::Bool, "Accepted for existing scripts."),
        f("orientJoint", "oj", ArgType::String, "Accepted for existing scripts."),
        f("secondaryAxisOrient", "sao", ArgType::String, "Accepted for existing scripts."),
        f("componentSpace", "cs", ArgType::Bool, "Accepted for existing scripts."),
        f("componentOffset", "co", ArgType::Bool, "move -r -co -ls: components moved each in its own space, the first value along its normal."),
        f("x", "", ArgType::Any, "This axis: a value, or on to read the positional value."),
        f("y", "", ArgType::Any, "This axis."),
        f("z", "", ArgType::Any, "This axis."),
    ];
    for (suffix, short) in [("X", ""), ("Y", ""), ("Z", ""), ("XY", "xy"), ("XZ", "xz"), ("YZ", "yz"), ("XYZ", "xyz")] {
        v.push(f(&format!("{cmd}{suffix}"), short, ArgType::Bool, "The axes the values are for."));
    }
    v
}

fn limits_set(g: &mut Graph, id: NodeId, attr: usize, axis: Option<usize>, values: &[f64]) -> CmdResult<()> {
    let mut lo = g.node(id).unwrap().vec3(attr);
    let mut hi = g.node(id).unwrap().vec3(attr + 1);
    match axis {
        Some(i) => {
            if values.len() < 2 {
                fail!("transformLimits: give the low and high value");
            }
            lo[i] = values[0];
            hi[i] = values[1];
        }
        None => {
            if values.len() < 6 {
                fail!("transformLimits: give three pairs of values");
            }
            for i in 0..3 {
                lo[i] = values[i * 2];
                hi[i] = values[i * 2 + 1];
            }
        }
    }
    put(g, id, attr, lo)?;
    put(g, id, attr + 1, hi)
}

fn numbers(v: &Value) -> Vec<f64> {
    match v {
        Value::Array(a) => a.iter().filter_map(|x| x.as_f64().or_else(|| x.as_bool().map(|b| b as i32 as f64))).collect(),
        o => o.as_f64().into_iter().collect(),
    }
}

/// (flag, limit block 0 translate 1 rotate 2 scale, axis, enable). Every transformLimits value flag.
fn limit_flags() -> Vec<(String, String, usize, Option<usize>, bool)> {
    let mut out = Vec::new();
    for (k, (word, letter)) in [("translation", "t"), ("rotation", "r"), ("scale", "s")].into_iter().enumerate() {
        out.push((word.to_string(), letter.to_string(), k, None, false));
        let cap = word[..1].to_uppercase() + &word[1..];
        out.push((format!("enable{cap}"), format!("e{letter}"), k, None, true));
        for (i, ax) in ["X", "Y", "Z"].iter().enumerate() {
            out.push((format!("{word}{ax}"), format!("{letter}{}", ax.to_lowercase()), k, Some(i), false));
            out.push((format!("enable{cap}{ax}"), format!("e{letter}{}", ax.to_lowercase()), k, Some(i), true));
        }
    }
    out
}

fn transform_limits(s: &mut Session, a: &Args) -> CmdResult {
    let ids = transforms_named(s, a, "transformLimits")?;
    let g = &mut s.graph;
    if a.bool("query") {
        let Some(what) = a.get("queried").and_then(Value::as_array).and_then(|v| v.first()).and_then(Value::as_str).map(str::to_string) else { fail!("transformLimits: query what?") };
        let Some((_, _, k, axis, enable)) = limit_flags().into_iter().find(|x| x.0 == what) else { fail!("transformLimits: can't query {what}") };
        let n = g.node(ids[0]).unwrap();
        let base = Transform::LIMITS + k * 4 + if enable { 2 } else { 0 };
        let (lo, hi) = (n.vec3(base), n.vec3(base + 1));
        let val = |x: f64| if enable { json!(x != 0.0) } else { json!(x) };
        return Ok(match axis {
            Some(i) => json!([val(lo[i]), val(hi[i])]),
            None => Value::Array((0..3).flat_map(|i| [val(lo[i]), val(hi[i])]).collect()),
        });
    }
    for id in ids {
        if a.bool("remove") {
            for k in 0..3 {
                let base = Transform::LIMITS + k * 4;
                put(g, id, base + 2, DVec3::ZERO)?;
                put(g, id, base + 3, DVec3::ZERO)?;
            }
        }
        for (flag, _, k, axis, enable) in limit_flags() {
            let Some(v) = a.get(&flag) else { continue };
            let base = Transform::LIMITS + k * 4 + if enable { 2 } else { 0 };
            limits_set(g, id, base, axis, &numbers(v))?;
        }
    }
    Ok(Value::Null)
}

pub fn register(c: &mut Commands<Session>) {
    let mut xf_flags = vec![
        f("query", "q", ArgType::Bool, "Return a value of the first object."),
        f("worldSpace", "ws", ArgType::Bool, "World space."),
        f("objectSpace", "os", ArgType::Bool, "Object space (the default)."),
        f("absolute", "a", ArgType::Bool, "Values replace what is there (the default)."),
        f("relative", "r", ArgType::Bool, "Values add to what is there."),
        f("preserve", "p", ArgType::Bool, "With -roo: keep the orientation, changing the angles."),
    ];
    for (name, short, doc) in [
        ("translation", "t", "Position."), ("rotation", "ro", "Rotation, degrees."), ("scale", "s", "Scale."), ("shear", "sh", "Shear: xy, xz, yz."), ("matrix", "m", "The matrix, 16 values."),
        ("pivots", "piv", "Both pivots: a point to set, six values queried."), ("rotatePivot", "rp", "The rotate pivot."), ("scalePivot", "sp", "The scale pivot."), ("rotateAxis", "ra", "The rotate axis."),
        ("rotateOrder", "roo", "The rotate order, by name."), ("boundingBox", "bb", "With -q: xmin ymin zmin xmax ymax zmax."), ("boundingBoxInvisible", "bbi", "With -q: bounding box, hidden shapes included."),
        ("rotateTranslation", "rt", "rotatePivotTranslate."), ("scaleTranslation", "st", "scalePivotTranslate."),
    ] {
        xf_flags.push(f(name, short, ArgType::Any, doc));
    }
    for (name, short, doc) in [
        ("centerPivots", "cp", "Centre the pivots on the bounding box."), ("centerPivotsOnComponents", "cpc", "Centre the pivots on the bounding box."), ("zeroTransformPivots", "ztp", "Put the pivots back at the origin of the object."),
        ("worldSpaceDistance", "wd", "Accepted: distances are in world units already."), ("euler", "eu", "With -r: the rotation values add to the XYZ angles there are, rather than turning the object by them."),
        ("deletePriorHistory", "dph", "Accepted for existing scripts."), ("reflection", "rfl", "On components: move the symmetric components too."),
        ("disableReflection", "drp", "Accepted for existing scripts."), ("parentSpace", "ps", "Pivots, translations, rotations, the rotate axis, the matrix and the bounding box in the parent's space."),
        ("preserveUV", "puv", "On components: their UVs follow, projected across the change (off)."), ("reflectionAboutBBox", "rab", "With -rfl: the mirror plane at the middle of the object's bounding box."),
        ("reflectionAboutOrigin", "rao", "With -rfl: the mirror plane through the origin (the default)."), ("reflectionAboutX", "rax", "With -rfl: mirror across X = 0 (the default)."),
        ("reflectionAboutY", "ray", "With -rfl: mirror across Y = 0."), ("reflectionAboutZ", "raz", "With -rfl: mirror across Z = 0."),
    ] {
        xf_flags.push(f(name, short, ArgType::Bool, doc));
    }
    xf_flags.push(f("reflectionTolerance", "rft", ArgType::Float, "With -rfl: how far a point and its mirror image may be apart to pair."));
    extend(c, "xform", xf_flags, xform);

    let mut mv = motion_flags("move");
    mv.retain(|x| !matches!(x.name.as_str(), "orientJoint" | "secondaryAxisOrient"));
    mv.extend([
        f("orientJoint", "oj", ArgType::String, "Joints moved, and the joint above each, aim along this axis order (xyz...)."),
        f("secondaryAxisOrient", "sao", ArgType::String, "With -oj: where the second axis points (yup, zdown...)."),
        f("autoOrientSecondaryAxis", "aos", ArgType::Bool, "With -oj and -sao: the second axis from the plane of the joints around, else -sao alone."),
        f("parameter", "p", ArgType::Bool, "Move curve points along the curve, in parameter space."),
        f("preserveUV", "puv", ArgType::Bool, "UVs of the components moved follow, projected along the move."),
        f("reflection", "rfl", ArgType::Bool, "Move the symmetric components too."),
        f("reflectionAboutBBox", "rab", ArgType::Bool, "With -rfl: the mirror plane at the middle of the object's bounding box."),
        f("reflectionAboutOrigin", "rao", ArgType::Bool, "With -rfl: the mirror plane through the origin (the default)."),
        f("reflectionAboutX", "rax", ArgType::Bool, "With -rfl: mirror across X = 0 (the default)."),
        f("reflectionAboutY", "ray", ArgType::Bool, "With -rfl: mirror across Y = 0."),
        f("reflectionAboutZ", "raz", ArgType::Bool, "With -rfl: mirror across Z = 0."),
        f("reflectionTolerance", "rft", ArgType::Float, "With -rfl: how far a point and its mirror image may be apart to pair."),
        f("symNegative", "smn", ArgType::Bool, "With -rfl: the move is relative to the negative side of the plane."),
    ]);
    extend(c, "move", mv, run_move);

    // The component and pivot options rotate and scale share with move.
    let shared = || {
        vec![
            f("centerPivot", "cp", ArgType::Bool, "Turn or scale about the middle of the bounding box of everything named."),
            f("objectCenterPivot", "ocp", ArgType::Bool, "Each object about the middle of its own bounding box."),
            f("orientAxes", "oa", ArgType::Vec3, "The axes the values are about, as Euler angles from the world's."),
            f("preserveUV", "puv", ArgType::Bool, "UVs of the components changed follow, projected across the change."),
            f("reflection", "rfl", ArgType::Bool, "Change the symmetric components too."),
            f("reflectionAboutBBox", "rab", ArgType::Bool, "With -rfl: the mirror plane at the middle of the object's bounding box."),
            f("reflectionAboutOrigin", "rao", ArgType::Bool, "With -rfl: the mirror plane through the origin (the default)."),
            f("reflectionAboutX", "rax", ArgType::Bool, "With -rfl: mirror across X = 0 (the default)."),
            f("reflectionAboutY", "ray", ArgType::Bool, "With -rfl: mirror across Y = 0."),
            f("reflectionAboutZ", "raz", ArgType::Bool, "With -rfl: mirror across Z = 0."),
            f("reflectionTolerance", "rft", ArgType::Float, "With -rfl: how far a point and its mirror image may be apart to pair."),
            f("symNegative", "smn", ArgType::Bool, "With -rfl: the change is relative to the negative side of the plane."),
        ]
    };
    let mut rot = motion_flags("rotate");
    rot.push(f("pivot", "p", ArgType::Vec3, "The world point to turn about."));
    rot.push(f("euler", "eu", ArgType::Bool, "Accepted for existing scripts."));
    rot.push(f("forceOrderXYZ", "fo", ArgType::Bool, "Accepted for existing scripts."));
    rot.push(f("translate", "t", ArgType::Bool, "About a pivot other than the object's: the move goes into translate rather than the rotate pivot's translation."));
    rot.extend(shared());
    extend(c, "rotate", rot, run_rotate);

    let mut sc = motion_flags("scale");
    sc.push(f("pivot", "p", ArgType::Vec3, "The world point to scale about."));
    sc.push(f("distanceOnly", "dso", ArgType::Bool, "Accepted for existing scripts."));
    sc.extend(shared());
    extend(c, "scale", sc, run_scale);

    // Named channels alone: makeIdentity -a 1 -t 1 freezes translation and nothing else.
    extend(
        c,
        "freezeTransforms",
        vec![
            f("apply", "a", ArgType::Bool, "On: bake into the geometry. Off, when given: back to identity, the object moving."),
            f("translate", "t", ArgType::Bool, "Freeze translation."),
            f("rotate", "r", ArgType::Bool, "Freeze rotation."),
            f("scale", "s", ArgType::Bool, "Freeze scale."),
            f("jointOrient", "jo", ArgType::Bool, "Joints: their jointOrient is reset so they line up with world space."),
        ],
        |s, a, prev| {
            let r = freeze_channels(s, a, prev)?;
            if a.bool("jointOrient") {
                joint_orient_to_world(s, a)?;
            }
            Ok(r)
        },
    );
    fn freeze_channels(s: &mut Session, a: &Args, prev: &RunFn<Session>) -> CmdResult {
        {
            if a.has("apply") && !a.bool("apply") {
                let channels: Vec<&str> = ["translate", "rotate", "scale"].into_iter().filter(|c| a.has(c)).collect();
                for id in transforms_named(s, a, "makeIdentity")? {
                    let g = &mut s.graph;
                    for (ch, attr, value) in [("translate", Transform::TRANSLATE, DVec3::ZERO), ("rotate", Transform::ROTATE, DVec3::ZERO), ("scale", Transform::SCALE, DVec3::ONE)] {
                        if channels.is_empty() || a.bool(ch) {
                            put(g, id, attr, value)?;
                        }
                    }
                }
                return Ok(Value::Null);
            }
            let mut b = a.clone();
            if ["translate", "rotate", "scale"].iter().any(|c| a.bool(c)) {
                for c in ["translate", "rotate", "scale"] {
                    b.flags.entry(c.to_string()).or_insert(json!(false));
                }
            }
            prev(s, &b)
        }
    }
    let freeze = c.find("freezeTransforms").unwrap().clone();
    let mut identity = freeze.clone();
    identity.name = "makeIdentity".into();
    identity.doc = "makeIdentity -a 1 bakes translate, rotate and scale into the geometry; without -a they go back to identity and the object moves.".into();
    let run = freeze.run.clone();
    identity.run = std::sync::Arc::new(move |s: &mut Session, a: &Args| {
        let mut b = a.clone();
        if !a.has("apply") {
            b.flags.insert("apply".into(), json!(false));
        }
        run(s, &b)
    });
    identity.adjustable = false;
    c.register(identity);

    extend(
        c,
        "spaceLocator",
        vec![
            f("absolute", "a", ArgType::Bool, "The position is world, with the pivots at it."),
            f("relative", "r", ArgType::Bool, "The position is local (the default)."),
            f("query", "q", ArgType::Bool, "Query mode: -p reads the locator's position."),
            f("edit", "e", ArgType::Bool, "Edit mode: -p moves the locator shape in its transform, -n renames it."),
            f("queried", "", ArgType::List, "Flags asked about in query mode."),
        ],
        |s, a, prev| {
            if a.bool("query") || a.bool("edit") {
                return locator_query_edit(s, a);
            }
            let r = prev(s, a)?;
            if a.bool("absolute") {
                if let (Some(p), Some(name)) = (a.vec3("position"), r[0].as_str()) {
                    let g = &mut s.graph;
                    let Some(id) = g.find(name)? else { return Ok(r) };
                    let at = DVec3::from_array(p);
                    put(g, id, Transform::ROTATE_PIVOT, at)?;
                    put(g, id, Transform::SCALE_PIVOT, at)?;
                }
            }
            Ok(r)
        },
    );

    if let Some(mut d) = c.find("spaceLocator").cloned() {
        d.positional_doc = "with -q and -e: the locator".into();
        c.register(d);
    }
    fn locator_query_edit(s: &mut Session, a: &Args) -> CmdResult {
        let names = aztek_ops::objects(s, a);
        let Some(name) = names.first() else { fail!("spaceLocator: name a locator") };
        let g = &mut s.graph;
        let Some(id) = g.find(name)? else { fail!("spaceLocator: no object {name}") };
        // The transform or its locator shape.
        let shape = if g.node(id).is_some_and(|n| n.ty().is_a("locator")) { Some(id) } else { crate::dag::shapes(g, id, false).into_iter().find(|&x| g.node(x).is_some_and(|n| n.ty().is_a("locator"))) };
        let Some(shape) = shape else { fail!("spaceLocator: {name} is not a locator") };
        let lp = PlugRef::new(shape, crate::transform::Locator::LOCAL_POSITION as i32);
        if a.bool("query") {
            if crate::ref_core::asked(a, "position") {
                let v = g.node(shape).unwrap().data().vec3(crate::transform::Locator::LOCAL_POSITION);
                return Ok(j3(v));
            }
            fail!("spaceLocator: -q takes -p");
        }
        let tr = crate::dag::transform_of(g, shape);
        if let Some(p) = a.vec3("position") {
            let at = DVec3::from_array(p);
            g.open_chunk("Space Locator");
            let r = (|| -> CmdResult<()> {
                g.set_value(lp, at.into())?;
                // Absolute: the pivots go to the point too, as at creation.
                if a.bool("absolute") {
                    put(g, tr, Transform::ROTATE_PIVOT, at)?;
                    put(g, tr, Transform::SCALE_PIVOT, at)?;
                }
                Ok(())
            })();
            g.close_chunk();
            r?;
        }
        if let Some(n) = a.get("name").and_then(Value::as_str) {
            g.rename(tr, n)?;
        }
        Ok(json!([g.path(tr)]))
    }

    let mut tl = aztek_ops::CommandDef::<Session>::new("transformLimits", "Set or query the limits of translate, rotate and scale: -tx lo hi, -etx onLo onHi.", transform_limits)
        .positional("objects")
        .flag(f("query", "q", ArgType::Bool, "Query."))
        .flag(f("edit", "e", ArgType::Bool, "Edit."))
        .flag(f("queried", "", ArgType::List, "What -q asks for."))
        .flag(f("remove", "rm", ArgType::Bool, "Turn every limit off."));
    for (name, short, _, _, _) in limit_flags() {
        tl = tl.flag(f(&name, &short, ArgType::Any, "Low and high values, or whether each is on."));
    }
    c.register(tl);
}
