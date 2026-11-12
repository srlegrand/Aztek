//! The reference's attribute commands with their full flag sets: getAttr, setAttr, addAttr, connectAttr and friends. The older
//! commands in `commands.rs` keep doing what they did; these add to them.

use aztek_ops::{fail, objects, ArgType, Args, CmdResult, CommandDef, Commands};
use glam::{DMat4, DVec3};
use serde_json::{json, Value};

use crate::graph::Graph;
use crate::ref_core::{extend, f, glob};
use crate::node::NodeType;
use crate::plug::{flags, from_json, to_json, AttrDef, NodeId, PlugRef, PlugType, PlugValue};
use crate::session::Session;
use crate::transform::{DagNode, Transform};

type Cmd = CommandDef<Session>;

/// The node and text after its first dot.
fn split_plug(path: &str) -> Option<(&str, &str)> {
    path.split_once('.')
}

/// Children of a compound with no value of its own: quaternions (by category) and addAttr's plain compounds.
pub(crate) fn compound_children(ty: &NodeType, name: &str) -> Vec<usize> {
    if ty.attr_index(name).is_some() {
        return Vec::new();
    }
    let mut out: Vec<usize> = ty.attrs.iter().enumerate().filter(|(_, a)| a.category == name && a.name.starts_with(name)).map(|(i, _)| i).collect();
    if out.is_empty() {
        out = ty.group_children(name);
    }
    out
}

fn compound_of(g: &Graph, path: &str) -> Option<(NodeId, Vec<usize>)> {
    let (node, attr) = split_plug(path)?;
    if attr.contains('.') || attr.contains('[') {
        return None;
    }
    let id = g.find(node).ok().flatten()?;
    let n = g.node(id)?;
    // drawOverride and drawInfo are one plug for connections, and read and set as compounds.
    if n.ty().dag && matches!(attr, "do" | "drawOverride") {
        return Some((id, crate::draw_style::OVERRIDE_CHILDREN.to_vec()));
    }
    if crate::display_layers::is_layer(n) && matches!(attr, "di" | "drawInfo") {
        return Some((id, crate::draw_style::LAYER_CHILDREN.to_vec()));
    }
    let kids = compound_children(n.ty(), attr);
    (!kids.is_empty()).then_some((id, kids))
}

/// node.group[3]: the element's children. None when that isn't what the path names.
fn group_element(g: &Graph, path: &str) -> Option<Vec<PlugRef>> {
    let (node, rest) = split_plug(path)?;
    let open = rest.rfind('[')?;
    if !rest.ends_with(']') || rest.contains('.') {
        return None;
    }
    let index: i32 = rest[open + 1..rest.len() - 1].parse().ok()?;
    let id = g.find(node).ok().flatten()?;
    let n = g.node(id)?;
    let group = &rest[..open];
    if n.ty().attr_index(group).is_some() {
        return None;
    }
    let mut kids: Vec<usize> = n.ty().group_children(group);
    if kids.is_empty() {
        // The short spelling: vl[0] for value[0].
        let prefix = format!("{group}.");
        kids = n.ty().attrs.iter().enumerate().filter(|(_, a)| a.short_name.starts_with(&prefix)).map(|(i, _)| i).collect();
    }
    (!kids.is_empty()).then(|| kids.into_iter().map(|a| PlugRef::element(id, a as i32, index)).collect())
}

/// The reference's name for an attribute's type.
pub(crate) fn ref_type(a: &AttrDef, component: bool) -> &'static str {
    use aztek_ops::units::Unit;
    match a.ty {
        PlugType::Float if a.unit == Unit::Angle => "doubleAngle",
        PlugType::Float if a.unit == Unit::Linear => "doubleLinear",
        PlugType::Vec3 if component && a.unit == Unit::Angle => "doubleAngle",
        PlugType::Vec3 if component && a.unit == Unit::Linear => "doubleLinear",
        PlugType::Vec3 if component => "double",
        PlugType::Vec3 if !a.child_names[0].is_empty() && a.child_names[2].is_empty() => "double2",
        PlugType::Vec3 if a.has(flags::COLOR) => "float3",
        PlugType::Vec3 => "double3",
        PlugType::Bool => "bool",
        PlugType::Int => "long",
        PlugType::Float => "double",
        PlugType::Enum => "enum",
        PlugType::Matrix => "matrix",
        PlugType::String => "string",
        PlugType::Mesh => "mesh",
        PlugType::Data => "TdataCompound",
    }
}

fn number_list(v: &[Value]) -> Vec<Value> {
    let mut out = Vec::new();
    for x in v {
        match x {
            Value::Array(a) => out.extend(number_list(a)),
            o => out.push(o.clone()),
        }
    }
    out
}

/// Bounding box attributes are answered from the geometry when asked: transforms in their parent's space, as the reference has them.
fn bounds_value(g: &mut Graph, p: &PlugRef) -> Option<DVec3> {
    let attr = p.attr as usize;
    if !matches!(attr, DagNode::BOUNDING_BOX_MIN | DagNode::BOUNDING_BOX_MAX | DagNode::BOUNDING_BOX_SIZE | DagNode::CENTER) {
        return None;
    }
    if !g.node(p.node).is_some_and(|n| n.ty().dag) {
        return None;
    }
    g.evaluate();
    let local = crate::node_bounds::object_bounds(g, p.node);
    let b = if crate::dag::is_transform(g, p.node) {
        crate::node_bounds::transformed(&local, &g.node(p.node)?.matrix(Transform::MATRIX))
    } else {
        local
    };
    if !b.valid() {
        return Some(DVec3::ZERO);
    }
    Some(match attr {
        DagNode::BOUNDING_BOX_MIN => b.min,
        DagNode::BOUNDING_BOX_MAX => b.max,
        DagNode::BOUNDING_BOX_SIZE => b.max - b.min,
        _ => (b.min + b.max) * 0.5,
    })
}

fn read(g: &mut Graph, p: &PlugRef) -> CmdResult {
    if let Some(v) = crate::draw_style::through_layer(g, p) {
        return Ok(to_json(&v));
    }
    if let Some(v) = bounds_value(g, p) {
        return Ok(if p.component >= 0 { json!(v[p.component as usize]) } else { json!([v.x, v.y, v.z]) });
    }
    // parentMatrix holds the frame the node lives in, offsetParentMatrix included; the reference's leaves that out.
    if p.attr as usize == DagNode::PARENT_MATRIX && g.node(p.node).is_some_and(|n| n.ty().dag) {
        let pim = g.value(&PlugRef::new(p.node, DagNode::PARENT_INVERSE_MATRIX as i32))?.as_matrix();
        let m: DMat4 = if pim.determinant() != 0.0 { pim.inverse() } else { DMat4::IDENTITY };
        return Ok(to_json(&PlugValue::Matrix(m)));
    }
    let v = g.value(p)?;
    let a = g.attr(p);
    if p.component < 0 && a.ty == PlugType::Vec3 && !a.child_names[0].is_empty() && a.child_names[2].is_empty() {
        let x = v.as_vec3();
        return Ok(json!([x.x, x.y]));
    }
    Ok(to_json(&v))
}

/// What a plug named by a script measures, when it measures something.
fn plug_unit(s: &Session, name: &str) -> Option<aztek_ops::units::Unit> {
    let p = s.graph.plug(name).ok()?;
    let unit = s.graph.attr(&p).unit;
    (unit != aztek_ops::units::Unit::None).then_some(unit)
}

/// smoothPreview, Aztek's old name: the levels showing, 0 for none, over displaySmoothMesh and smoothLevel.
fn old_smooth_plug(s: &Session, name: &str) -> Option<NodeId> {
    let (node, attr) = name.rsplit_once('.')?;
    if attr != "smoothPreview" {
        return None;
    }
    let id = s.graph.find(node).ok().flatten()?;
    s.graph.node(id).is_some_and(|n| n.ty().is_a("mesh")).then_some(id)
}

fn get_attr(s: &mut Session, a: &Args, prev: &aztek_ops::RunFn<Session>) -> CmdResult {
    let Some(name) = a.pos_str(0).map(str::to_string) else { fail!("getAttr: expected a plug") };
    if let Some(id) = old_smooth_plug(s, &name) {
        return Ok(json!(crate::mesh_nodes::MeshNode::preview_level(s.graph.node(id).unwrap())));
    }
    // A deformer's per-point weights, kept together rather than as plugs.
    if let Some(r) = crate::deform_paint::get_weights(&mut s.graph, &name) {
        return r;
    }
    if a.has("time") {
        let t = a.float("time", 0.0) / crate::anim::fps();
        let mut rest = a.clone();
        rest.flags.remove("time");
        return s.graph.at_time(t, |g| read_args(g, &rest));
    }
    let g = &mut s.graph;
    // Components: the local position, as the reference gives for vtx and cv. A multi attribute reads the same way, so plugs first.
    if g.plug(&name).is_err() && crate::components::is_component_string(&name) {
        let mut out = Vec::new();
        for (mesh, ids) in crate::skin::points_named(g, std::slice::from_ref(&name)) {
            let Some(m) = crate::components::mesh_of(g, mesh) else { continue };
            for p in ids {
                let q = m.position(p);
                out.extend([json!(q.x), json!(q.y), json!(q.z)]);
            }
        }
        if !out.is_empty() {
            return Ok(Value::Array(out));
        }
    }
    let _ = prev;
    read_args(g, a)
}

fn read_args(g: &mut Graph, a: &Args) -> CmdResult {
    let name = a.pos_str(0).unwrap_or_default().to_string();
    if let Some((id, kids)) = compound_of(g, &name) {
        if a.bool("type") {
            return Ok(json!("TdataCompound"));
        }
        if a.bool("size") {
            return Ok(json!(1));
        }
        let mut out = Vec::new();
        for k in kids {
            out.push(read(g, &PlugRef::new(id, k as i32))?);
        }
        return Ok(Value::Array(out));
    }
    if let Some(kids) = group_element(g, &name) {
        let mut out = Vec::new();
        for k in kids {
            out.push(read(g, &k)?);
        }
        return Ok(Value::Array(out));
    }
    let p = g.plug(&name)?;
    let def = g.attr(&p).clone();
    if a.bool("lock") {
        return Ok(json!(g.is_locked(&p)));
    }
    if a.bool("multiIndices") {
        let id = p.node;
        let group = def.name.split('.').next().unwrap_or("").to_string();
        if def.name.contains('.') {
            return Ok(json!(g.group_indices(id, &group)));
        }
        return Ok(json!(g.elements(&p).iter().map(|e| e.element).collect::<Vec<_>>()));
    }
    if a.bool("type") {
        return Ok(json!(ref_type(&def, p.component >= 0)));
    }
    if a.bool("keyable") {
        return Ok(json!(crate::anim::is_keyable(g, &p)));
    }
    if a.bool("channelBox") {
        return Ok(json!(g.channel_state(&p).1));
    }
    if a.bool("settable") {
        let driven = g.source(&p).is_some() || (p.component >= 0 && g.source(&PlugRef { component: -1, ..p }).is_some()) || crate::draw_style::driven_by_layer(g, &p);
        return Ok(json!(def.has(flags::INPUT) && !g.is_locked(&p) && !driven));
    }
    if a.bool("size") {
        return Ok(json!(if def.is_array() && p.element < 0 { g.elements(&p).len() } else { 1 }));
    }
    if def.is_array() && p.element < 0 {
        let mut out = Vec::new();
        for e in g.elements(&p) {
            out.push(read(g, &e)?);
        }
        return Ok(Value::Array(out));
    }
    read(g, &p)
}

/// setAttr, on a shape's instances as well: they're one shape.
fn set_attr(s: &mut Session, a: &Args, prev: &aztek_ops::RunFn<Session>) -> CmdResult {
    let r = set_attr_one(s, a, prev)?;
    let Some(name) = a.pos_str(0).map(str::to_string) else { return Ok(r) };
    let Some((node, attr)) = name.split_once('.') else { return Ok(r) };
    let Some(id) = s.graph.find(node).ok().flatten() else { return Ok(r) };
    let others = crate::instancing::one_shape(&s.graph, id);
    for other in others.into_iter().filter(|&o| o != id) {
        let mut call = a.clone();
        call.positional[0] = json!(format!("{}.{attr}", s.graph.path(other)));
        set_attr_one(s, &call, prev)?;
    }
    Ok(r)
}

/// A curve as setAttr -type "nurbsCurve" writes it: degree, spans, form, rational, dimension, then the knots with
/// their count (the count first in MEL and scene files, the list first from Python), the number of CVs, and the CVs.
fn curve_from_values(values: &[Value]) -> Option<aztek_geo::nurbs::NurbsCurve> {
    let num = |v: &Value| v.as_f64().or_else(|| v.as_bool().map(|b| b as i64 as f64)).or_else(|| v.as_str().and_then(|t| if t == "no" || t == "false" { Some(0.0) } else if t == "yes" || t == "true" { Some(1.0) } else { t.parse().ok() }));
    if values.len() < 7 {
        return None;
    }
    let (degree, form, dim) = (num(&values[0])? as usize, num(&values[2])? as i64, num(&values[4])? as usize);
    let mut at = 5;
    let knots: Vec<f64> = match &values[at] {
        Value::Array(list) => {
            at += 2; // the list, then its count
            list.iter().filter_map(num).collect()
        }
        count => {
            let n = num(count)? as usize;
            at += 1;
            let k: Vec<f64> = values.get(at..at + n)?.iter().filter_map(num).collect();
            at += n;
            k
        }
    };
    let n_cvs = num(values.get(at)?)? as usize;
    at += 1;
    let mut flat: Vec<f64> = Vec::new();
    for v in &values[at..] {
        match v {
            Value::Array(p) => {
                let mut c: Vec<f64> = p.iter().filter_map(num).collect();
                c.resize(3, 0.0);
                flat.extend(&c[..3]);
            }
            other => flat.push(num(other)?),
        }
    }
    let step = if values[at..].iter().any(Value::is_array) { 3 } else { dim.clamp(2, 4) };
    let cvs: Vec<DVec3> = flat.chunks(step).filter(|c| c.len() == step).map(|c| DVec3::new(c[0], c[1], if step > 2 { c[2] } else { 0.0 })).collect();
    if cvs.len() != n_cvs || degree == 0 {
        return None;
    }
    let c = aztek_geo::nurbs::NurbsCurve::new(degree, aztek_geo::nurbs::Form::from_index(form), cvs, knots);
    c.is_valid().then_some(c)
}

fn set_attr_one(s: &mut Session, a: &Args, prev: &aztek_ops::RunFn<Session>) -> CmdResult {
    let Some(name) = a.pos_str(0).map(str::to_string) else { fail!("setAttr: expected (plug, value)") };
    if a.get("type").and_then(Value::as_str) == Some("nurbsCurve") {
        let Some(curve) = curve_from_values(&a.positional[1..]) else { fail!("setAttr: {name}: the values do not make a curve (degree, spans, form, rational, dimension, knots, CVs)") };
        let p = s.graph.plug(&name)?;
        let data: crate::plug::DataPtr = std::sync::Arc::new(crate::curves::CurveData(curve));
        s.graph.set_value(p, PlugValue::Data(Some(data)))?;
        return Ok(Value::Null);
    }
    if let (Some(id), Some(level)) = (old_smooth_plug(s, &name), a.positional.get(1).and_then(Value::as_i64)) {
        crate::mesh_nodes::MeshNode::set_preview(&mut s.graph, id, level)?;
        return Ok(Value::Null);
    }
    // A child that's driven makes the whole plug driven.
    if let Ok(p) = s.graph.plug(&name) {
        if aztek_ops::units::user_call() && p.component < 0 && s.graph.attr(&p).ty == PlugType::Vec3 && (0..3).any(|k| s.graph.source(&PlugRef::component(p.node, p.attr, k)).is_some()) {
            fail!("setAttr: {name} is connected: its children are driven");
        }
    }
    // A script's number is in the current unit.
    let converted;
    let a = match plug_unit(s, &name).filter(|_| aztek_ops::units::user_call()) {
        Some(unit) => {
            let mut c = a.clone();
            for v in c.positional.iter_mut().skip(1) {
                aztek_ops::units::json_to_stored(unit, v);
            }
            converted = c;
            &converted
        }
        None => a,
    };
    if let Some(r) = crate::deform_paint::set_weights(&mut s.graph, &name, &a.positional[1..]) {
        return r;
    }
    // Mesh points by component.
    if s.graph.plug(&name).is_err() && crate::components::is_component_string(&name) && a.positional.len() > 1 {
        let vals: Vec<f64> = number_list(&a.positional[1..]).iter().filter_map(Value::as_f64).collect();
        let g = &mut s.graph;
        for (mesh, ids) in crate::skin::points_named(g, std::slice::from_ref(&name)) {
            let moves: Vec<(u32, DVec3)> = ids.into_iter().zip(vals.chunks(3)).filter(|(_, c)| c.len() == 3).map(|(p, c)| (p, DVec3::new(c[0], c[1], c[2]))).collect();
            crate::mesh_edit::transform::set_points(g, mesh, &moves)?;
        }
        return Ok(Value::Null);
    }
    let values = &a.positional[1..];
    // Compounds with no value of their own take one value per child.
    let compound = compound_of(&s.graph, &name).map(|(id, kids)| kids.into_iter().map(|k| PlugRef::new(id, k as i32)).collect::<Vec<_>>()).or_else(|| group_element(&s.graph, &name));
    if let Some(kids) = compound {
        let vals = number_list(values);
        if kids.iter().any(|p| crate::draw_style::driven_by_layer(&s.graph, p)) {
            fail!("setAttr: The attribute '{name}' is locked or connected and cannot be modified.");
        }
        for (p, v) in kids.iter().zip(vals.iter()) {
            let def = s.graph.attr(p).clone();
            s.graph.set_value(*p, from_json(&def, v)?)?;
        }
        return Ok(Value::Null);
    }
    let p = s.graph.plug(&name)?;
    if !values.is_empty() && crate::draw_style::driven_by_layer(&s.graph, &p) {
        fail!("setAttr: The attribute '{name}' is locked or connected and cannot be modified.");
    }
    if a.has("keyable") {
        crate::anim::set_keyable(&mut s.graph, &p, a.bool("keyable"))?;
        // A keyable channel is in the channel box as that; -cb is for the ones that aren't.
        if a.bool("keyable") {
            s.graph.set_channel_state(&p, None, Some(false));
        }
    }
    if a.has("channelBox") {
        s.graph.set_channel_state(&p, None, Some(a.bool("channelBox")));
    }
    if a.has("lock") && !a.bool("lock") {
        s.graph.set_locked(&p, false)?;
    }
    if !values.is_empty() {
        let ty = a.string("type", "");
        let def = s.graph.attr(&p).clone();
        let flat = number_list(values);
        let mut call = a.clone();
        call.flags.remove("lock");
        call.flags.remove("keyable");
        call.flags.remove("channelBox");
        match ty.as_str() {
            "stringArray" | "doubleArray" | "floatArray" | "Int32Array" | "int32Array" | "vectorArray" | "pointArray" => {
                // MEL and the reference's Python put the count first in some forms: setAttr n.a -type stringArray 2 "x" "y".
                let mut items = flat;
                let counted = items.first().and_then(Value::as_u64).map(|c| c as usize);
                let per = if matches!(ty.as_str(), "vectorArray" | "pointArray") { 3 } else { 1 };
                if counted.is_some_and(|c| c * per == items.len() - 1) {
                    items.remove(0);
                }
                let list: Vec<Value> = if per == 3 { items.chunks(3).map(|c| json!(c)).collect() } else { items };
                let vals = list.iter().map(|v| from_json(&def, v)).collect::<Result<Vec<_>, _>>()?;
                s.graph.set_elements(&p, &vals)?;
            }
            "string" | "" if def.ty == PlugType::String => {
                let text = values[0].as_str().map(str::to_string).unwrap_or_else(|| values[0].to_string());
                s.graph.set_value(p, PlugValue::String(text))?;
            }
            _ => {
                // double3, double2, matrix and plain values: gather what the attr holds.
                let v = match def.ty {
                    _ if p.component >= 0 => flat[0].clone(),
                    PlugType::Vec3 if flat.len() == 2 => {
                        json!([flat[0], flat[1], 0.0])
                    }
                    PlugType::Vec3 | PlugType::Matrix if !def.is_array() || p.element >= 0 => Value::Array(flat.clone()),
                    _ if def.is_array() && p.element < 0 => Value::Array(flat.clone()),
                    _ => flat[0].clone(),
                };
                call.positional = vec![json!(name), v];
                prev(s, &call)?;
            }
        }
    }
    if a.has("lock") && a.bool("lock") {
        s.graph.set_locked(&p, true)?;
    }
    Ok(Value::Null)
}

fn pairs_for_compound(g: &Graph, src: &str, dst: &str) -> Option<Vec<(PlugRef, PlugRef)>> {
    // A layer's drawInfo goes to drawOverride as one connection.
    if split_plug(src).is_some_and(|(_, a)| matches!(a, "di" | "drawInfo")) {
        return None;
    }
    let (si, sk) = compound_of(g, src)?;
    let (di, dk) = compound_of(g, dst)?;
    (sk.len() == dk.len()).then(|| sk.into_iter().zip(dk).map(|(x, y)| (PlugRef::new(si, x as i32), PlugRef::new(di, y as i32))).collect())
}


fn add_attr_query(s: &mut Session, a: &Args) -> CmdResult {
    let g = &s.graph;
    let plug = plug_for_add_attr(s, a)?;
    let def = g.attr(&plug).clone();
    let what = a.get("queried").and_then(Value::as_array).and_then(|v| v.first()).and_then(Value::as_str).unwrap_or("").to_string();
    Ok(match what.as_str() {
        "attributeType" => json!(ref_type(&def, plug.component >= 0)),
        "dataType" => json!(if def.ty == PlugType::String { "string" } else { "" }),
        "longName" => json!(def.name.rsplit('.').next().unwrap_or("")),
        "shortName" => json!(def.short_name.rsplit('.').next().unwrap_or("")),
        "niceName" => json!(if def.nice_name.is_empty() { def.name.clone() } else { def.nice_name.clone() }),
        "defaultValue" => {
            let mut v = to_json(&def.default_value);
            aztek_ops::units::json_to_user(def.unit, &mut v);
            v
        }
        "minValue" => json!(def.min_value.map(|x| aztek_ops::units::to_user(def.unit, x))),
        "maxValue" => json!(def.max_value.map(|x| aztek_ops::units::to_user(def.unit, x))),
        "softMinValue" => json!(def.soft_min.map(|x| aztek_ops::units::to_user(def.unit, x))),
        "softMaxValue" => json!(def.soft_max.map(|x| aztek_ops::units::to_user(def.unit, x))),
        "hasMinValue" => json!(def.min_value.is_some()),
        "hasMaxValue" => json!(def.max_value.is_some()),
        "enumName" => json!(def.enum_names.join(":")),
        "keyable" => json!(crate::anim::is_keyable(g, &plug)),
        "multi" => json!(def.is_array()),
        "parent" => json!(def.name.split_once('.').map(|x| x.0).unwrap_or("")),
        "exists" => json!(true),
        _ => fail!("addAttr: nothing to query"),
    })
}

/// addAttr's -q and -e name the attribute as node.attr or as -ln on the node.
fn plug_for_add_attr(s: &Session, a: &Args) -> CmdResult<PlugRef> {
    let objs = objects(s, a);
    let Some(first) = objs.first() else { fail!("addAttr: no attribute named") };
    if first.contains('.') {
        return Ok(s.graph.plug(first)?);
    }
    for key in ["longName", "shortName"] {
        if a.has(key) {
            return Ok(s.graph.plug(&format!("{first}.{}", a.string(key, "")))?);
        }
    }
    fail!("addAttr: name the attribute: node.attr or -ln")
}

fn add_attr_edit(s: &mut Session, a: &Args) -> CmdResult {
    let plug = plug_for_add_attr(s, a)?;
    let g = &mut s.graph;
    let mut def = g.attr(&plug).clone();
    if a.has("niceName") {
        def.nice_name = a.string("niceName", "");
    }
    if a.has("minValue") {
        def.min_value = Some(a.float("minValue", 0.0));
    }
    if a.has("maxValue") {
        def.max_value = Some(a.float("maxValue", 0.0));
    }
    if a.has("hasMinValue") && !a.bool("hasMinValue") {
        def.min_value = None;
    }
    if a.has("hasMaxValue") && !a.bool("hasMaxValue") {
        def.max_value = None;
    }
    if a.has("softMinValue") {
        def.soft_min = Some(a.float("softMinValue", 0.0));
    }
    if a.has("softMaxValue") {
        def.soft_max = Some(a.float("softMaxValue", 0.0));
    }
    if a.has("enumName") {
        def.enum_names = a.string("enumName", "").split(':').filter(|n| !n.is_empty()).map(str::to_string).collect();
    }
    if let Some(dv) = a.get("defaultValue") {
        def.default_value = from_json(&def, dv)?;
    }
    if a.has("keyable") {
        if a.bool("keyable") {
            def.flags |= flags::KEYABLE;
        } else {
            def.flags &= !flags::KEYABLE;
        }
    }
    if !def.has(flags::USER) {
        fail!("addAttr: {} belongs to the node's type and can't be edited", g.plug_name(&plug));
    }
    g.edit_attr(&PlugRef { component: -1, element: -1, ..plug }, def)?;
    Ok(Value::Null)
}

/// A child of a reference style double3: addAttr -ln fooR -at double -p foo names one of foo's three children.
fn add_vec3_child(s: &mut Session, a: &Args) -> CmdResult<Option<Value>> {
    if !a.has("parent") {
        return Ok(None);
    }
    let group = a.string("parent", "");
    let name = a.string("longName", "");
    let mut out = Vec::new();
    for o in objects(s, a) {
        let Some(id) = s.graph.find(&o)? else { fail!("addAttr: no object named {o}") };
        let n = s.graph.node(id).unwrap();
        let Some(i) = n.ty().attr_index(&group) else { return Ok(None) };
        let mut def = n.ty().attrs[i].clone();
        if def.ty != PlugType::Vec3 {
            return Ok(None);
        }
        if !def.child_names.contains(&name) {
            // Still at their made up names: this child takes the first of those.
            let auto = |c: usize| def.child_names[c] == format!("{group}{}", ["X", "Y", "Z"][c]);
            let Some(c) = (0..3).find(|&c| auto(c)) else { fail!("addAttr: {group} already has its three children") };
            def.child_names[c] = name.clone();
            def.child_short[c] = a.string("shortName", "");
            s.graph.edit_attr(&PlugRef::new(id, i as i32), def)?;
        } else if a.has("shortName") {
            let c = def.child_names.iter().position(|x| *x == name).unwrap();
            def.child_short[c] = a.string("shortName", "");
            s.graph.edit_attr(&PlugRef::new(id, i as i32), def)?;
        }
        out.push(json!(format!("{}.{name}", s.graph.path(id))));
    }
    Ok(Some(Value::Array(out)))
}

/// The unitConversion node feeding a plug.
fn conversion_into(g: &Graph, p: &PlugRef) -> Option<NodeId> {
    g.source(p).map(|c| c.node).filter(|&n| g.node(n).is_some_and(|x| x.ty().name == "unitConversion"))
}

/// As attribute_query, with defaults and limits in the current unit.
fn attribute_query_shown(s: &mut Session, a: &Args) -> CmdResult {
    let mut v = attribute_query(s, a)?;
    let numeric = ["listDefault", "minimum", "maximum", "softMin", "softMax", "range", "softRange", "softRangeAll"].iter().any(|f| a.bool(f));
    if numeric && aztek_ops::units::user_call() {
        let node = a.string("node", "");
        let attr = a.pos_str(0).unwrap_or_default().to_string();
        if let Ok(p) = s.graph.plug(&format!("{node}.{attr}")) {
            aztek_ops::units::json_to_user(s.graph.attr(&p).unit, &mut v);
        }
    }
    Ok(v)
}

fn attribute_query(s: &mut Session, a: &Args) -> CmdResult {
    let Some(attr) = a.pos_str(0).map(str::to_string) else { fail!("attributeQuery: name an attribute") };
    let ty: std::sync::Arc<NodeType>;
    let mut node = None;
    if a.has("node") {
        let name = a.string("node", "");
        let Some(id) = s.graph.find(&name)? else { fail!("attributeQuery: no node named {name}") };
        ty = s.graph.node(id).unwrap().type_arc().clone();
        node = Some(id);
    } else if a.has("type") {
        let Some(t) = crate::node::types::find(&a.string("type", "")) else { fail!("attributeQuery: unknown type") };
        ty = t;
    } else {
        fail!("attributeQuery: give -n or -type");
    }
    // Which attr, and which child of it.
    let mut found: Option<(usize, i32)> = ty.attr_index(&attr).map(|i| (i, -1));
    if found.is_none() {
        for (i, d) in ty.attrs.iter().enumerate() {
            if d.ty == PlugType::Vec3 {
                if let Some(c) = (0..3).find(|&c| !attr.is_empty() && (d.child_names[c] == attr || d.child_short[c] == attr)) {
                    found = Some((i, c as i32));
                    break;
                }
            }
            if d.name.rsplit_once('.').is_some_and(|x| x.1 == attr) || d.short_name.rsplit_once('.').is_some_and(|x| x.1 == attr) {
                found = Some((i, -1));
                break;
            }
        }
    }
    if found.is_none() {
        if let (Some(id), true) = (node, a.has("node")) {
            if let Some(t) = s.graph.node(id).unwrap().aliases().get(&attr).cloned() {
                found = ty.attr_index(&t).map(|i| (i, -1));
            }
        }
    }
    let kids = compound_children(&ty, &attr);
    if a.bool("exists") {
        return Ok(json!(found.is_some() || !kids.is_empty()));
    }
    let part = |i: usize, c: i32| node.map(|id| PlugRef { node: id, attr: i as i32, component: c, element: -1 });
    if !kids.is_empty() {
        if a.bool("listChildren") {
            return Ok(json!(kids.iter().map(|&k| ty.attrs[k].name.rsplit('.').next().unwrap().to_string()).collect::<Vec<_>>()));
        }
        if a.bool("attributeType") {
            return Ok(json!("compound"));
        }
        if a.bool("longName") || a.bool("shortName") || a.bool("niceName") {
            return Ok(json!(attr));
        }
        fail!("attributeQuery: nothing to say about compound {attr}");
    }
    let Some((i, comp)) = found else { fail!("attributeQuery: no attribute {attr}") };
    let d = &ty.attrs[i];
    let leaf = d.name.rsplit('.').next().unwrap_or("").to_string();
    let long = if comp >= 0 { d.child_names[comp as usize].clone() } else { leaf.clone() };
    let short = if comp >= 0 { d.child_short[comp as usize].clone() } else { d.short_name.rsplit('.').next().unwrap_or("").to_string() };
    let g = &s.graph;
    let pair = |lo: Option<f64>, hi: Option<f64>| {
        let mut out = Vec::new();
        out.extend(lo.map(|x| json!(x)));
        out.extend(hi.map(|x| json!(x)));
        Value::Array(out)
    };
    let one = |x: Option<f64>| Value::Array(x.map(|v| json!(v)).into_iter().collect());
    Ok(if a.bool("keyable") {
        json!(part(i, comp).map_or(d.has(flags::KEYABLE), |p| crate::anim::is_keyable(g, &p)))
    } else if a.bool("channelBox") {
        json!(part(i, comp).map_or(d.has(flags::CHANNEL_BOX), |p| g.channel_state(&p).1))
    } else if a.bool("hidden") {
        json!(d.has(flags::HIDDEN))
    } else if a.bool("writable") {
        json!(d.has(flags::INPUT))
    } else if a.bool("readable") {
        json!(true)
    } else if a.bool("connectable") {
        json!(true)
    } else if a.bool("storable") {
        json!(d.has(flags::STORABLE))
    } else if a.bool("multi") {
        json!(d.is_array())
    } else if a.bool("indexMatters") {
        json!(d.is_array())
    } else if a.bool("usedAsColor") {
        json!(d.has(flags::COLOR))
    } else if a.bool("usedAsFilename") {
        json!(d.has(flags::FILE))
    } else if a.bool("attributeType") {
        json!(if d.ty == PlugType::Vec3 && comp < 0 { "double3" } else if comp >= 0 { "double" } else { ref_type(d, false) })
    } else if a.bool("typeExact") {
        json!(ref_type(d, comp >= 0))
    } else if a.bool("longName") {
        json!(long)
    } else if a.bool("shortName") {
        json!(short)
    } else if a.bool("niceName") {
        json!(crate::ref_attrs::nice(if d.nice_name.is_empty() || comp >= 0 { &long } else { &d.nice_name }))
    } else if a.bool("listEnum") {
        json!([d.enum_names.join(":")])
    } else if a.bool("listParent") {
        if comp >= 0 {
            json!([d.name])
        } else if let Some((grp, _)) = d.name.split_once('.') {
            json!([grp])
        } else if !d.category.is_empty() && d.name.starts_with(&d.category) {
            json!([d.category])
        } else {
            Value::Null
        }
    } else if a.bool("listChildren") {
        if d.ty == PlugType::Vec3 && comp < 0 {
            json!(d.child_names.iter().filter(|c| !c.is_empty()).collect::<Vec<_>>())
        } else {
            Value::Null
        }
    } else if a.bool("listSiblings") {
        if comp >= 0 {
            json!(d.child_names.iter().enumerate().filter(|(c, n)| *c as i32 != comp && !n.is_empty()).map(|x| x.1).collect::<Vec<_>>())
        } else {
            Value::Null
        }
    } else if a.bool("listDefault") {
        match &d.default_value {
            PlugValue::Vec3(v) if comp >= 0 => json!([v[comp as usize]]),
            PlugValue::Vec3(v) => json!([v.x, v.y, v.z]),
            o => o.scalar().map(|x| json!([x])).unwrap_or(json!([to_json(o)])),
        }
    } else if a.bool("minExists") {
        json!(d.min_value.is_some())
    } else if a.bool("maxExists") {
        json!(d.max_value.is_some())
    } else if a.bool("softMinExists") {
        json!(d.soft_min.is_some())
    } else if a.bool("softMaxExists") {
        json!(d.soft_max.is_some())
    } else if a.bool("minimum") || a.bool("mn") {
        one(d.min_value)
    } else if a.bool("maximum") || a.bool("mx") {
        one(d.max_value)
    } else if a.bool("softMin") {
        one(d.soft_min)
    } else if a.bool("softMax") {
        one(d.soft_max)
    } else if a.bool("range") {
        pair(d.min_value, d.max_value)
    } else if a.bool("softRange") {
        pair(d.soft_min, d.soft_max)
    } else {
        fail!("attributeQuery: nothing asked")
    })
}

/// "translateX" -> "Translate X".
pub(crate) fn nice(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if i == 0 {
            out.extend(c.to_uppercase());
        } else {
            if c.is_ascii_uppercase() {
                out.push(' ');
            }
            out.push(c);
        }
    }
    out
}

fn list_attr(s: &mut Session, a: &Args) -> CmdResult {
    let objs = objects(s, a);
    let Some(first) = objs.first() else { fail!("listAttr: no object") };
    let patterns: Vec<String> = match a.get("string") {
        Some(Value::String(t)) => vec![t.clone()],
        Some(Value::Array(v)) => v.iter().filter_map(|x| x.as_str().map(str::to_string)).collect(),
        _ => Vec::new(),
    };
    let g = &s.graph;
    let (node_name, only) = match split_plug(first) {
        Some((n, attr)) => (n.to_string(), Some(g.plug(first).map(|p| p.attr as usize).map_err(|_| aztek_ops::CommandError::new(format!("listAttr: no attribute {attr}")))?)),
        None => (first.clone(), None),
    };
    let Some(id) = g.find(&node_name)? else { fail!("listAttr: no object named {node_name}") };
    let n = g.node(id).unwrap();
    // The reference's filters, each narrowing: a flag not given lets everything through.
    let (keyable, locked, unlocked, user, cb, settable, multi, write, visible, leaf, array, scalar, short) = (
        a.bool("keyable"), a.bool("locked"), a.bool("unlocked"), a.bool("userDefined"), a.bool("channelBox"), a.bool("settable"), a.bool("multi"), a.bool("write"), a.bool("visible"), a.bool("leaf"), a.bool("array"), a.bool("scalar"), a.bool("shortNames"),
    );
    let scalar_and_array = a.bool("scalarAndArray");
    let mut out: Vec<String> = Vec::new();
    for (i, d) in n.ty().attrs.iter().enumerate() {
        if only.is_some_and(|o| o != i) {
            continue;
        }
        if d.has(flags::HIDDEN) && !d.has(flags::MESSAGE) && patterns.is_empty() && only.is_none() {
            continue;
        }
        if (user && !d.has(flags::USER)) || (write && !d.has(flags::INPUT)) || (settable && !d.has(flags::INPUT)) || false {
            continue;
        }
        if (multi || array) && !d.is_array() {
            continue;
        }
        if (scalar || scalar_and_array) && matches!(d.ty, PlugType::Matrix | PlugType::Mesh | PlugType::Data | PlugType::String) {
            continue;
        }
        if scalar && d.is_array() {
            continue;
        }
        if visible && d.has(flags::HIDDEN) {
            continue;
        }
        let consider = |name: String, sn: String, part: PlugRef, is_leaf: bool, out: &mut Vec<String>| {
            let (k, c) = (crate::anim::is_keyable(g, &part), g.channel_state(&part).1);
            if (keyable && !k) || (cb && !c) || (leaf && !is_leaf) {
                return;
            }
            let is_locked = n.lock_mask(i) != 0 && g.is_locked(&part);
            if (locked && !is_locked) || (unlocked && is_locked) {
                return;
            }
            if settable && (is_locked || g.source(&part).is_some()) {
                return;
            }
            if !patterns.is_empty() && !patterns.iter().any(|p| glob(p, &name) || glob(p, &sn)) {
                return;
            }
            out.push(if short && !sn.is_empty() { sn } else { name });
        };
        let whole = PlugRef::new(id, i as i32);
        if d.ty == PlugType::Vec3 {
            // Keyable and locked filters look at the children; a compound with all three children locked or keyable is
            // not named on its own, as the reference lists translateX for -k rather than translate.
            if !keyable && !cb && !locked && !leaf {
                consider(d.name.clone(), d.short_name.clone(), whole, false, &mut out);
            }
            if !d.is_array() {
                for c in 0..3 {
                    if d.child_names[c].is_empty() {
                        continue;
                    }
                    consider(d.child_names[c].clone(), d.child_short[c].clone(), PlugRef::component(id, i as i32, c as i32), true, &mut out);
                }
            }
        } else {
            // listAttr -m node.multi gives the elements alone, by their aliases where they have them.
            if !(multi && only.is_some()) {
                consider(d.name.clone(), d.short_name.clone(), whole, !d.is_array(), &mut out);
            }
        }
        if multi && d.is_array() {
            for (k, _) in n.elements(i) {
                out.push(g.alias_of(&PlugRef::element(id, i as i32, *k)).unwrap_or_else(|| format!("{}[{k}]", d.name)));
            }
        }
    }
    Ok(json!(out))
}

pub fn register(c: &mut Commands<Session>) {
    extend(
        c,
        "getAttr",
        vec![
            f("time", "t", ArgType::Float, "The value at this frame."),
            f("type", "typ", ArgType::Bool, "The attribute's type name."),
            f("keyable", "k", ArgType::Bool, "Whether it is keyable."),
            f("channelBox", "cb", ArgType::Bool, "Whether it shows in the channel box without being keyable."),
            f("settable", "se", ArgType::Bool, "Whether setAttr would take: not locked, not driven, an input."),
            f("size", "s", ArgType::Bool, "The element count of a multi attribute; 1 otherwise."),
            f("silent", "sl", ArgType::Bool, "Accepted for existing scripts."),
            f("asString", "as", ArgType::Bool, "An enum's label."),
            f("expandEnvironmentVariables", "x", ArgType::Bool, "Accepted for existing scripts."),
            f("lock", "l", ArgType::Bool, "Whether it is locked."),
            f("multiIndices", "mi", ArgType::Bool, "The element indices of a multi attribute."),
        ],
        |s, a, prev| {
            if a.bool("asString") {
                let name = a.pos_str(0).unwrap_or_default().to_string();
                let p = s.graph.plug(&name)?;
                let d = s.graph.attr(&p).clone();
                let v = s.graph.value(&p)?;
                if d.ty == PlugType::Enum {
                    return Ok(json!(d.enum_names.get(v.as_int() as usize).cloned().unwrap_or_default()));
                }
                return Ok(to_json(&v));
            }
            let mut v = get_attr(s, a, prev)?;
            if aztek_ops::units::user_call() && !a.bool("type") && !a.bool("size") && !a.bool("multiIndices") {
                if let Some(unit) = plug_unit(s, a.pos_str(0).unwrap_or_default()) {
                    aztek_ops::units::json_to_user(unit, &mut v);
                }
            }
            Ok(v)
        },
    );
    extend(
        c,
        "setAttr",
        vec![
            f("keyable", "k", ArgType::Bool, "Make the plug keyable or not."),
            f("channelBox", "cb", ArgType::Bool, "Show the plug in the channel box without keying."),
            f("lock", "l", ArgType::Bool, "Lock or unlock the plug."),
            f("type", "typ", ArgType::String, "double3, string, matrix, stringArray, doubleArray and so on."),
            f("clamp", "c", ArgType::Bool, "Accepted: values are always held inside an attribute's limits."),
            f("size", "s", ArgType::Int, "Accepted for existing scripts."),
            f("alteredValue", "av", ArgType::Bool, "Accepted for existing scripts."),
            f("caching", "ca", ArgType::Bool, "Accepted for existing scripts."),
        ],
        set_attr,
    );
    extend(
        c,
        "connectAttr",
        vec![
            f("force", "f", ArgType::Bool, "Replace an existing input connection."),
            f("nextAvailable", "na", ArgType::Bool, "The next free element of a multi attribute."),
            f("lock", "l", ArgType::Bool, "Lock the destination afterwards."),
            f("referenceDest", "rd", ArgType::String, "Accepted for existing scripts."),
        ],
        |s, a, prev| {
            let (Some(src), Some(dst)) = (a.pos_str(0).map(str::to_string), a.pos_str(1).map(str::to_string)) else { fail!("connectAttr: expected (src, dst)") };
            let g = &mut s.graph;
            if let Some(pairs) = pairs_for_compound(g, &src, &dst) {
                for (x, y) in pairs {
                    g.connect(&x, &y, a.bool("force"))?;
                }
                return Ok(json!(dst));
            }
            let mut d = g.plug(&dst)?;
            if a.bool("nextAvailable") && d.element >= 0 {
                while g.source(&d).is_some() {
                    d.element += 1;
                }
                let sp = g.plug(&src)?;
                let made = g.connect(&sp, &d, a.bool("force"))?;
                return Ok(json!(g.plug_name(&made)));
            }
            let r = prev(s, a)?;
            if a.bool("lock") {
                let p = s.graph.plug(&dst)?;
                s.graph.set_locked(&p, true)?;
            }
            Ok(r)
        },
    );
    extend(
        c,
        "disconnectAttr",
        vec![f("nextAvailable", "na", ArgType::Bool, "Disconnect the next element src drives.")],
        |s, a, prev| {
            if let (Some(src), Some(dst)) = (a.pos_str(0).map(str::to_string), a.pos_str(1).map(str::to_string)) {
                let g = &mut s.graph;
                if let Some(pairs) = pairs_for_compound(g, &src, &dst) {
                    for (_, y) in pairs {
                        let conversion = conversion_into(&s.graph, &y);
                        s.graph.disconnect(&y);
                        if let Some(c) = conversion {
                            let name = s.graph.path(c);
                            s.run("delete", json!({"args": [name]}))?;
                        }
                    }
                    return Ok(Value::Null);
                }
                let (sp, d) = (g.plug(&src)?, g.plug(&dst)?);
                if !g.attr(&d).is_array() || d.element >= 0 {
                    let through = conversion_into(g, &d).and_then(|c| {
                        let input = g.node(c)?.ty().attr_index("input")?;
                        g.source(&PlugRef::new(c, input as i32))
                    });
                    if g.source(&d) != Some(sp) && through != Some(sp) {
                        fail!("disconnectAttr: there is no connection from '{src}' to '{dst}'");
                    }
                } else if a.bool("nextAvailable") {
                    if let Some(e) = g.elements(&d).into_iter().find(|e| g.source(e) == Some(sp)) {
                        g.disconnect(&e);
                    }
                    return Ok(Value::Null);
                }
            }
            prev(s, a)
        },
    );
    extend(
        c,
        "addAttr",
        vec![
            f("query", "q", ArgType::Bool, "Query."),
            f("edit", "e", ArgType::Bool, "Edit."),
            f("queried", "", ArgType::List, "What -q asks for."),
            f("exists", "ex", ArgType::Bool, "With -q: whether it exists."),
            f("hasMinValue", "hnv", ArgType::Bool, "Whether it has a hard minimum."),
            f("hasMaxValue", "hxv", ArgType::Bool, "Whether it has a hard maximum."),
            f("hasSoftMinValue", "hsn", ArgType::Bool, "Accepted for existing scripts."),
            f("hasSoftMaxValue", "hsx", ArgType::Bool, "Accepted for existing scripts."),
            f("indexMatters", "im", ArgType::Bool, "Accepted: a multi's indices always matter here."),
            f("readable", "r", ArgType::Bool, "Accepted for existing scripts."),
            f("writable", "w", ArgType::Bool, "Accepted for existing scripts."),
            f("storable", "s", ArgType::Bool, "Accepted for existing scripts."),
            f("cachedInternally", "ci", ArgType::Bool, "Accepted for existing scripts."),
            f("internalSet", "is", ArgType::Bool, "Accepted for existing scripts."),
            f("disconnectBehaviour", "dcb", ArgType::Int, "Accepted for existing scripts."),
            f("fromPlugin", "fp", ArgType::Bool, "Accepted for existing scripts."),
            f("binaryTag", "bt", ArgType::String, "Accepted for existing scripts."),
            f("proxy", "pxy", ArgType::String, "Accepted for existing scripts."),
            f("usedAsProxy", "uap", ArgType::Bool, "Accepted for existing scripts."),
        ],
        |s, a, prev| {
            if a.bool("query") {
                if a.get("queried").and_then(Value::as_array).is_some_and(|v| v.iter().any(|x| x.as_str() == Some("exists"))) {
                    return Ok(json!(plug_for_add_attr(s, a).is_ok()));
                }
                return add_attr_query(s, a);
            }
            if a.bool("edit") {
                return add_attr_edit(s, a);
            }
            if let Some(v) = add_vec3_child(s, a)? {
                return Ok(v);
            }
            prev(s, a)
        },
    );
    c.register(
        Cmd::new("renameAttr", "renameAttr('node.attr', 'newName'): renames an attribute added with addAttr.", |s, a| {
            let (Some(path), Some(name)) = (a.pos_str(0), a.pos_str(1)) else { fail!("renameAttr: expected (plug, newName)") };
            let g = &mut s.graph;
            let p = g.plug(path)?;
            let mut def = g.attr(&p).clone();
            if !def.has(flags::USER) {
                fail!("renameAttr: {path} belongs to the node's type");
            }
            def.name = match def.name.rsplit_once('.') {
                Some((grp, _)) => format!("{grp}.{name}"),
                None => name.to_string(),
            };
            if def.ty == PlugType::Vec3 {
                for (c, axis) in ["X", "Y", "Z"].into_iter().enumerate() {
                    def.child_names[c] = format!("{name}{axis}");
                }
            }
            g.edit_attr(&PlugRef { component: -1, element: -1, ..p }, def)?;
            Ok(json!(name))
        })
        .positional("plug newName"),
    );
    c.register(
        Cmd::new("aliasAttr", "aliasAttr('alias', 'node.attr') names an attribute another way. -q lists a node's aliases and their attributes; -rm removes.", |s, a| {
            if a.bool("query") {
                let Some(first) = a.pos_str(0).map(str::to_string) else { fail!("aliasAttr: name a node") };
                let g = &s.graph;
                if let Some((node, attr)) = split_plug(&first) {
                    let Some(id) = g.find(node)? else { fail!("aliasAttr: no node {node}") };
                    let n = g.node(id).unwrap();
                    return Ok(json!(n.aliases().iter().find(|(al, t)| t.as_str() == attr || al.as_str() == attr).map(|x| x.0.clone()).unwrap_or_default()));
                }
                let Some(id) = g.find(&first)? else { fail!("aliasAttr: no node {first}") };
                let out: Vec<Value> = g.node(id).unwrap().aliases().iter().flat_map(|(al, t)| [json!(al), json!(t)]).collect();
                return Ok(Value::Array(out));
            }
            let named = a.named();
            if a.bool("remove") {
                for path in named {
                    let Some((node, attr)) = split_plug(&path).map(|(x, y)| (x.to_string(), y.to_string())) else { fail!("aliasAttr: expected node.alias") };
                    let Some(id) = s.graph.find(&node)? else { fail!("aliasAttr: no node {node}") };
                    let n = s.graph.node(id).unwrap();
                    let alias = n.aliases().iter().find(|(al, t)| al.as_str() == attr || t.as_str() == attr).map(|x| x.0.clone());
                    if let Some(al) = alias {
                        s.graph.set_alias(id, &al, None);
                    }
                }
                return Ok(Value::Null);
            }
            let (Some(alias), Some(path)) = (named.first(), named.get(1)) else { fail!("aliasAttr: expected (alias, plug)") };
            let p = s.graph.plug(path)?;
            let Some((_, attr)) = split_plug(path) else { fail!("aliasAttr: expected node.attr") };
            if s.graph.node(p.node).unwrap().ty().attr_index(alias).is_some() {
                fail!("aliasAttr: '{alias}' is already an attribute");
            }
            s.graph.set_alias(p.node, alias, Some(attr.to_string()));
            Ok(json!(true))
        })
        .positional("alias plug, or a node with -q")
        .flag(f("query", "q", ArgType::Bool, "List the aliases."))
        .flag(f("remove", "rm", ArgType::Bool, "Remove the alias.")),
    );
    let mut q = Cmd::new("attributeQuery", "Facts about an attribute: attributeQuery -n node -k attr, or -type nodeType.", attribute_query_shown).positional("attribute").undoable(false);
    for (name, short, doc) in [
        ("exists", "ex", "Whether it exists."), ("keyable", "k", "Keyable."), ("channelBox", "ch", "Shown in the channel box."), ("hidden", "h", "Hidden."),
        ("writable", "w", "Takes values."), ("readable", "r", "Can be read."), ("connectable", "c", "Can connect."), ("storable", "s", "Saved."), ("multi", "m", "A multi attribute."),
        ("indexMatters", "im", "A multi's indices matter."), ("usedAsColor", "uac", "A colour."), ("usedAsFilename", "uaf", "A file name."), ("attributeType", "at", "Its type."),
        ("typeExact", "tex", "Its exact type."), ("longName", "ln", "The long name."), ("shortName", "sn", "The short name."), ("niceName", "nn", "The nice name."),
        ("listEnum", "le", "An enum's labels."), ("listParent", "lp", "The compound it is in."), ("listChildren", "lc", "A compound's children."), ("listSiblings", "ls", "The other children."),
        ("listDefault", "ld", "The default."), ("minExists", "mne", "Has a hard minimum."), ("maxExists", "mxe", "Has a hard maximum."), ("softMinExists", "sme", "Has a soft minimum."),
        ("softMaxExists", "sxe", "Has a soft maximum."), ("minimum", "min", "The hard minimum."), ("maximum", "max", "The hard maximum."), ("mn", "", "The hard minimum."), ("mx", "", "The hard maximum."),
        ("softMin", "smn", "The soft minimum."), ("softMax", "smx", "The soft maximum."), ("range", "rg", "The hard range."), ("softRange", "sr", "The soft range."),
    ] {
        q = q.flag(f(name, short, ArgType::Bool, doc));
    }
    c.register(q.flag(f("node", "n", ArgType::String, "The node.")).flag(f("type", "typ", ArgType::String, "A node type.")));

    extend(c, "listAttr", {
        let mut v = Vec::new();
        for (name, short, doc) in [
            ("keyable", "k", "Only keyable."), ("unlocked", "u", "Only unlocked."), ("connectable", "c", "Only connectable."), ("settable", "s", "Only settable."), ("multi", "m", "Only multi attributes, with their elements."),
            ("userDefined", "ud", "Only added ones."), ("locked", "l", "Only locked."), ("channelBox", "cb", "Only those in the channel box without being keyable."), ("read", "r", "Only readable."), ("write", "w", "Only writable."),
            ("visible", "v", "Only visible."), ("leaf", "lf", "Only leaves of compounds."), ("scalarAndArray", "sa", "Only scalars and arrays of them."), ("scalar", "sc", "Only scalars."), ("shortNames", "sn", "Short names."), ("array", "a", "Only arrays."), ("output", "o", "Only outputs."), ("hasData", "hd", "Accepted for existing scripts."), ("inUse", "iu", "Accepted for existing scripts."), ("changedSinceFileOpen", "cfo", "Accepted for existing scripts."), ("nodeName", "nn", "Accepted for existing scripts."),
        ] {
            v.push(f(name, short, ArgType::Bool, doc));
        }
        v.push(f("string", "st", ArgType::Any, "Only names matching this pattern, or these."));
        v.push(f("category", "ct", ArgType::String, "Accepted for existing scripts."));
        v
    }, |s, a, _prev| list_attr(s, a));

    c.register(
        Cmd::new("isConnected", "isConnected(src, dst): whether src drives dst.", |s, a| {
            let (Some(x), Some(y)) = (a.pos_str(0), a.pos_str(1)) else { fail!("isConnected: expected (src, dst)") };
            let g = &s.graph;
            if let Some(pairs) = pairs_for_compound(g, x, y) {
                let source_through = |q: &PlugRef| match conversion_into(g, q) {
                    Some(c) => g.node(c).and_then(|n| n.ty().attr_index("input")).and_then(|i| g.source(&PlugRef::new(c, i as i32))),
                    None => g.source(q),
                };
                return Ok(json!(pairs.iter().all(|(p, q)| source_through(q).as_ref() == Some(p) || g.source(q) == Some(*p))));
            }
            let (sp, dp) = (g.plug(x)?, g.plug(y)?);
            // A conversion node between them doesn't part them.
            let through = g.source(&dp).filter(|c| g.node(c.node).is_some_and(|n| n.ty().name == "unitConversion")).and_then(|c| {
                let input = g.node(c.node)?.ty().attr_index("input")?;
                g.source(&PlugRef::new(c.node, input as i32))
            });
            let exact = g.source(&dp) == Some(sp) || through == Some(sp);
            // A whole vector joined through conversion nodes is joined axis by axis.
            let axes = dp.component < 0
                && sp.component < 0
                && g.attr(&dp).ty == PlugType::Vec3
                && (0..3).all(|k| {
                    let part = PlugRef::component(dp.node, dp.attr, k);
                    conversion_into(g, &part)
                        .and_then(|c| g.source(&PlugRef::new(c, g.node(c)?.ty().attr_index("input")? as i32)))
                        == Some(PlugRef::component(sp.node, sp.attr, k))
                });
            if axes {
                return Ok(json!(true));
            }
            // A whole Vec3 drives its children, child by child.
            let parts = dp.component >= 0 && sp.component == dp.component && g.source(&PlugRef { component: -1, ..dp }) == Some(PlugRef { component: -1, ..sp });
            Ok(json!(exact || parts))
        })
        .positional("src dst")
        .flag(f("ignoreUnitConversion", "iuc", ArgType::Bool, "Accepted: no conversion nodes are inserted here."))
        .undoable(false),
    );
    let mut ci = Cmd::new("connectionInfo", "Facts about a plug's connections.", |s, a| {
        let Some(name) = a.pos_str(0) else { fail!("connectionInfo: name a plug") };
        let g = &s.graph;
        let p = g.plug(name)?;
        let whole = PlugRef { component: -1, ..p };
        let source = g.source(&p).or_else(|| if p.component >= 0 { g.source(&whole).map(|x| PlugRef { component: p.component, ..x }) } else { None });
        let mut dests = g.destinations(&p);
        if p.component >= 0 {
            dests.extend(g.destinations(&whole));
        }
        let name_of = |q: &PlugRef| json!(g.plug_name(q));
        Ok(if a.bool("sourceFromDestination") || a.bool("getExactSource") {
            source.as_ref().map(name_of).unwrap_or(json!(""))
        } else if a.bool("destinationFromSource") || a.bool("getExactDestination") {
            Value::Array(dests.iter().map(name_of).collect())
        } else if a.bool("isDestination") {
            json!(source.is_some())
        } else if a.bool("isExactDestination") {
            json!(g.source(&p).is_some())
        } else if a.bool("isSource") {
            json!(!dests.is_empty())
        } else if a.bool("isExactSource") {
            json!(!g.destinations(&p).is_empty())
        } else if a.bool("isLocked") {
            json!(g.is_locked(&p))
        } else {
            fail!("connectionInfo: nothing asked")
        })
    })
    .positional("plug")
    .undoable(false);
    for (name, short, doc) in [
        ("sourceFromDestination", "sfd", "The plug driving this one, or \"\"."), ("destinationFromSource", "dfs", "The plugs this one drives."), ("getExactSource", "ges", "The plug driving exactly this one."), ("getExactDestination", "ged", "The plugs driven by exactly this one."),
        ("isDestination", "id", "Driven."), ("isExactDestination", "ied", "Driven directly."), ("isSource", "is", "Drives something."), ("isExactSource", "ies", "Drives something directly."), ("isLocked", "il", "Locked."),
    ] {
        ci = ci.flag(f(name, short, ArgType::Bool, doc));
    }
    c.register(ci);
}
