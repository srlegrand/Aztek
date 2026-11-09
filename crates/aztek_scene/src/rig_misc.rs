//! Smaller rigging commands: rigid bind, geomBind, jointCluster, pairBlend, matchTransform, inheritTransform, container,
//! controller, the dimension and annotation shapes, dropoffLocator, findDeformers and copyDeformerWeights.
//!
//! Differs from the reference: bindSkin makes a skinCluster with one influence per point, the weights a rigid bind has, rather than
//! The reference's clusters per joint. jointCluster records its settings on a node and softens nothing; dropoffLocator marks the
//! place on a wire and doesn't change the wire's falloff. flexor, combinationShape, skeletonEmbed and weightsColor are in
//! rig_extra.

use std::any::Any;
use std::sync::Arc;

use aztek_ops::{fail, objects, ArgType, Args, CmdResult, CommandDef, Commands};
use glam::{DMat4, DVec3};
use serde_json::{json, Value};

use crate::graph::{Graph, SelectMode};
use crate::ref_core::{asked, f};
use crate::node::{types, vec3_attr, EvalContext, NodeBehavior, NodeData, NodeType};
use crate::plug::{flags, AttrDef, NodeId, PlugRef, PlugType, PlugValue, NO_NODE};
use crate::rig::{find, plug};
use crate::session::Session;
use crate::system_cmds::q_flags;
use crate::transform::{DagNode, Locator, Shape, Transform};

type Cmd = CommandDef<Session>;
use ArgType::{Bool as B, Float as F, Int as I, List as L, String as S};

const IN_STORE: u32 = flags::INPUT | flags::STORABLE;

fn attr(name: &str, short: &str, ty: PlugType, def: PlugValue, fl: u32, doc: &str) -> AttrDef {
    AttrDef { name: name.into(), short_name: short.into(), ty, default_value: def, flags: fl, doc: doc.into(), ..Default::default() }
}
fn float_in(name: &str, short: &str, def: f64) -> AttrDef {
    attr(name, short, PlugType::Float, PlugValue::Float(def), IN_STORE | flags::KEYABLE, "A setting.")
}
fn float_out(name: &str, short: &str) -> AttrDef {
    attr(name, short, PlugType::Float, PlugValue::Float(0.0), flags::OUTPUT, "Worked out from the inputs.")
}
fn str_in(name: &str, short: &str) -> AttrDef {
    attr(name, short, PlugType::String, PlugValue::String(String::new()), IN_STORE, "A setting.")
}

// ---------------------------------------------------------------------------------------------------------------------
// Node types

/// distanceDimShape: the distance between its two points.
struct DistanceDim;

const DD_START: usize = Shape::COUNT;
const DD_END: usize = Shape::COUNT + 1;
const DD_DISTANCE: usize = Shape::COUNT + 2;

impl NodeBehavior for DistanceDim {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        Shape::compute_matrices(d);
        let v = (d.vec3(DD_END) - d.vec3(DD_START)).length();
        d.set_output(DD_DISTANCE, PlugValue::Float(v));
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// A shape that only keeps its matrices current.
struct PlainShape;

impl NodeBehavior for PlainShape {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        Shape::compute_matrices(d);
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

const AN_TARGET: usize = Shape::COUNT + 2;

pub fn register_types() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let shape = types::find("shape").expect("shape");
        let reg = |t: NodeType| {
            let name = t.name.clone();
            types::register_type(t).expect(&name);
        };
        reg(NodeType {
            name: "distanceDimShape".into(),
            base: Some(shape.clone()),
            dag: true,
            icon: "ruler".into(),
            doc: "A measured distance between two points, drawn as a line.".into(),
            own_attrs: vec![
                vec3_attr("startPoint", "sp", DVec3::ZERO, IN_STORE, "One end, world space."),
                vec3_attr("endPoint", "ep", DVec3::ZERO, IN_STORE, "The other end, world space."),
                float_out("distance", "dist"),
            ],
            create: Some(Arc::new(|| Box::new(DistanceDim))),
            ..Default::default()
        });
        reg(NodeType {
            name: "paramDimension".into(),
            base: Some(shape.clone()),
            dag: true,
            icon: "ruler".into(),
            doc: "A point on a curve or surface, labelled with its parameter.".into(),
            own_attrs: vec![
                str_in("object", "obj"),
                float_in("uParamValue", "upv", 0.0),
                float_in("vParamValue", "vpv", 0.0),
            ],
            create: Some(Arc::new(|| Box::new(PlainShape))),
            ..Default::default()
        });
        reg(NodeType {
            name: "annotationShape".into(),
            base: Some(shape.clone()),
            dag: true,
            icon: "text".into(),
            doc: "A note in the viewport with a line to what it's about.".into(),
            own_attrs: vec![
                str_in("text", "txt"),
                attr("displayArrow", "daro", PlugType::Bool, PlugValue::Bool(true), IN_STORE, "Draw the line to the object."),
                vec3_attr("targetPosition", "tp", DVec3::ZERO, IN_STORE, "What the line points at, world space."),
                attr("dagObjectMatrix", "dom", PlugType::Matrix, PlugValue::Matrix(glam::DMat4::IDENTITY), flags::INPUT | flags::HIDDEN | flags::ARRAY, "The object the line points at: its world matrix."),
            ],
            create: Some(Arc::new(|| Box::new(PlainShape))),
            ..Default::default()
        });
        reg(NodeType {
            name: "dropoffLocator".into(),
            base: Some(shape.clone()),
            dag: true,
            icon: "locator".into(),
            doc: "A place along a wire with a dropoff percentage of its own.".into(),
            own_attrs: vec![float_in("param", "pr", 0.0), float_in("percent", "pc", 100.0), float_in("dropoff", "dp", 1.0), str_in("wire", "wr")],
            create: Some(Arc::new(|| Box::new(PlainShape))),
            ..Default::default()
        });
        for (name, doc, attrs) in [
            ("container", "Groups nodes so they read as one thing.", vec![attr("member", "mb", PlugType::String, PlugValue::String(String::new()), IN_STORE | flags::ARRAY, "Each node in it."), attr("isAsset", "isa", PlugType::Bool, PlugValue::Bool(false), IN_STORE, "Made with -asset.")]),
            ("controller", "Tags an object as an animation control, with its place among the controls.", {
                let link = |name: &str, short: &str, array: u32, doc: &str| attr(name, short, PlugType::Int, PlugValue::Int(0), flags::INPUT | flags::MESSAGE | flags::HIDDEN | array, doc);
                vec![
                    link("controllerObject", "act", 0, "The object this tags: its message."),
                    link("parent", "pare", 0, "Connected to the parent tag's children."),
                    link("children", "child", flags::ARRAY, "The tags under this one."),
                    attr("prepopulate", "prep", PlugType::Bool, PlugValue::Bool(true), IN_STORE, "Evaluate the controls below ahead of use."),
                    attr("parentprepopulate", "ppp", PlugType::Bool, PlugValue::Bool(true), IN_STORE, "Take prepopulate from the parent."),
                    attr("cycleWalkSibling", "cwsb", PlugType::Bool, PlugValue::Bool(false), IN_STORE, "Pick walking cycles among siblings."),
                    attr("visibilityMode", "vism", PlugType::Enum, PlugValue::Int(0), IN_STORE, "When the control shows."),
                ]
            }),
            (
                "jointCluster",
                "A joint's rigid bind settings. Kept for scripts: nothing softens here.",
                vec![
                    float_in("aboveBound", "ab", 10.0),
                    float_in("belowBound", "bb", 10.0),
                    float_in("aboveValue", "av", 0.5),
                    float_in("belowValue", "bv", 0.5),
                    str_in("joint", "jt"),
                ],
            ),
        ] {
            reg(NodeType { name: name.into(), doc: doc.into(), own_attrs: attrs, create: Some(Arc::new(|| Box::new(crate::node::Plain))), ..Default::default() });
        }
    });
}

/// Segments to draw for the shapes here, world space. The viewport calls this.
pub fn shape_lines(n: &crate::node::Node) -> Option<Vec<(DVec3, DVec3)>> {
    let t = n.ty();
    if t.name == "distanceDimShape" {
        let (a, b) = (n.data().vec3(DD_START), n.data().vec3(DD_END));
        let dir = (b - a).normalize_or_zero();
        let side = if dir.y.abs() > 0.9 { dir.cross(DVec3::X) } else { dir.cross(DVec3::Y) }.normalize_or_zero() * 0.1;
        return Some(vec![(a, b), (a - side, a + side), (b - side, b + side)]);
    }
    if t.name == "annotationShape" {
        let o = n.data().matrix(DagNode::WORLD_MATRIX).w_axis.truncate();
        let target = n.data().elements(Shape::COUNT + 3).first().map_or(n.data().vec3(AN_TARGET), |(_, m)| m.as_matrix().w_axis.truncate());
        return Some(if n.data().bool(Shape::COUNT + 1) { vec![(o, target)] } else { Vec::new() });
    }
    if t.name == "paramDimension" || t.name == "dropoffLocator" {
        let o = n.data().matrix(DagNode::WORLD_MATRIX).w_axis.truncate();
        let k = 0.15;
        return Some(vec![(o - DVec3::X * k, o + DVec3::X * k), (o - DVec3::Y * k, o + DVec3::Y * k), (o - DVec3::Z * k, o + DVec3::Z * k)]);
    }
    None
}

/// The label a shape shows, with where it goes. The viewport calls this.
pub fn shape_label(n: &crate::node::Node) -> Option<(String, DVec3)> {
    let t = n.ty();
    if t.name == "distanceDimShape" {
        return Some((format!("{:.3}", n.data().float(DD_DISTANCE)), (n.data().vec3(DD_START) + n.data().vec3(DD_END)) * 0.5));
    }
    if t.name == "annotationShape" {
        return Some((n.data().string(Shape::COUNT).to_string(), n.data().matrix(DagNode::WORLD_MATRIX).w_axis.truncate()));
    }
    if t.name == "paramDimension" {
        let d = n.data();
        return Some((format!("{:.3}, {:.3}", d.float(Shape::COUNT + 1), d.float(Shape::COUNT + 2)), d.matrix(DagNode::WORLD_MATRIX).w_axis.truncate()));
    }
    None
}

// ---------------------------------------------------------------------------------------------------------------------
// Binding

fn bind_skin(s: &mut Session, a: &Args) -> CmdResult {
    let names = objects(s, a);
    if a.bool("unbind") || a.bool("unbindKeepHistory") || a.bool("delete") {
        let mut call = json!({"args": names, "edit": true});
        call[if a.bool("unbindKeepHistory") { "unbindKeepHistory" } else { "unbind" }] = json!(true);
        s.run("skinCluster", call)?;
        return Ok(json!(1));
    }
    if names.len() < 2 {
        fail!("bindSkin: name or select a skeleton and geometry");
    }
    let mut call = json!({"args": names, "maximumInfluences": 1, "toSelectedBones": a.bool("toSelectedBones") || a.bool("doNotDescend"), "bindMethod": 0, "dropoffRate": 10.0});
    if a.has("name") {
        call["name"] = json!(a.string("name", ""));
    }
    s.run("skinCluster", call)?;
    Ok(json!(1))
}

fn geom_bind(s: &mut Session, a: &Args) -> CmdResult {
    let names = objects(s, a);
    let Some(first) = names.first() else { fail!("geomBind: name a skinCluster") };
    let g = &mut s.graph;
    let id = find(g, first, "geomBind")?;
    let sk = if g.node(id).is_some_and(|n| n.ty().is_a(crate::skin::SKIN_TYPE)) { Some(id) } else { crate::components::as_mesh(g, id).and_then(|m| crate::skin::skin_of(g, m)) };
    let Some(sk) = sk else { fail!("geomBind: {first} is not bound") };
    // The reference's geodesic voxel bind (3) is the heat bind here; both follow the surface rather than straight lines.
    let method = match a.int("bindMethod", 3) {
        3 | 2 => 2,
        m => m,
    };
    crate::skin::rebind_weights(g, sk, method, a.int("maxInfluences", 5).max(1) as usize, a.float("falloff", 0.2) * 20.0)?;
    Ok(Value::Null)
}

fn joint_cluster(s: &mut Session, a: &Args) -> CmdResult {
    let joint = a.get("joint").and_then(Value::as_str).map(str::to_string).or_else(|| objects(s, a).into_iter().next());
    let Some(joint) = joint else { fail!("jointCluster: give -j, or select a joint") };
    let g = &mut s.graph;
    let j = find(g, &joint, "jointCluster")?;
    if !crate::rig::is_joint(g, j) {
        fail!("jointCluster: {joint} is not a joint");
    }
    let t = types::find("jointCluster").unwrap();
    let n = g.create_node(&t, &a.string("name", "jointCluster1"), NO_NODE, true)?;
    let at = |k: &str| PlugRef::new(n, t.attr_index(k).unwrap() as i32);
    for flag in ["aboveBound", "belowBound", "aboveValue", "belowValue"] {
        if a.has(flag) {
            g.set_value(at(flag), PlugValue::Float(a.float(flag, 0.0)))?;
        }
    }
    let path = g.path(j);
    g.set_value(at("joint"), PlugValue::String(path))?;
    Ok(json!(g.path(n)))
}

// ---------------------------------------------------------------------------------------------------------------------
// pairBlend

/// Which pair of pairBlend inputs an attribute goes through, and which of its parts.
fn blend_slot(g: &Graph, p: &PlugRef) -> Option<(&'static str, &'static str, &'static str)> {
    match g.attr(p).name.as_str() {
        "translate" => Some(("inTranslate1", "inTranslate2", "outTranslate")),
        "rotate" => Some(("inRotate1", "inRotate2", "outRotate")),
        _ => None,
    }
}

fn pair_blend(s: &mut Session, a: &Args) -> CmdResult {
    let attrs: Vec<String> = match a.get("attribute") {
        Some(Value::Array(v)) => v.iter().filter_map(|x| x.as_str().map(str::to_string)).collect(),
        Some(Value::String(t)) => vec![t.clone()],
        _ => Vec::new(),
    };
    let g = &mut s.graph;
    if a.bool("query") {
        let Some(pb) = objects_of(a).first().and_then(|n| g.find(n).ok().flatten()) else { fail!("pairBlend: name a pairBlend") };
        let t = g.node(pb).unwrap().type_arc().clone();
        let list = |g: &Graph, names: &[&str]| -> Vec<Value> {
            let mut out = Vec::new();
            for n in names {
                let at = t.attr_index(n).unwrap() as i32;
                for (d, sr) in g.connections() {
                    if d.node == pb && d.attr == at {
                        out.push(json!(g.plug_name(&sr)));
                    }
                }
            }
            out
        };
        if asked(a, "input1") {
            return Ok(Value::Array(list(g, &["inTranslate1", "inRotate1"])));
        }
        if asked(a, "input2") {
            return Ok(Value::Array(list(g, &["inTranslate2", "inRotate2"])));
        }
        if asked(a, "attribute") {
            let mut out = Vec::new();
            for n in ["outTranslate", "outRotate"] {
                let at = t.attr_index(n).unwrap() as i32;
                for (d, sr) in g.connections() {
                    if sr.node == pb && sr.attr == at {
                        out.push(json!(g.plug_name(&d)));
                    }
                }
            }
            return Ok(Value::Array(out));
        }
        fail!("pairBlend: query what?");
    }
    let Some(node_name) = a.get("node").and_then(Value::as_str) else { fail!("pairBlend: give -node") };
    if attrs.is_empty() {
        fail!("pairBlend: give -attribute");
    }
    let t = types::find("pairBlend").expect("pairBlend");
    g.open_chunk("pairBlend");
    let r = (|| -> CmdResult<NodeId> {
        let pb = g.create_node(&t, &a.string("name", "pairBlend1"), NO_NODE, true)?;
        for at in &attrs {
            let dst = g.plug(&format!("{node_name}.{at}"))?;
            let Some((in1, _, out)) = blend_slot(g, &dst) else { fail!("pairBlend: {at} isn't a translate or rotate") };
            let part = |attr: &str, comp: i32| PlugRef { node: pb, attr: t.attr_index(attr).unwrap() as i32, component: comp, element: -1 };
            // What drives the attribute now feeds the first input, or its value does.
            match g.source(&dst) {
                Some(src) => {
                    g.connect(&src, &part(in1, dst.component), true)?;
                }
                None => {
                    let v = g.value(&dst)?;
                    g.set_value(part(in1, dst.component), v)?;
                }
            }
            g.connect(&part(out, dst.component), &dst, true)?;
        }
        Ok(pb)
    })();
    g.close_chunk();
    let pb = r?;
    g.evaluate();
    Ok(json!(g.path(pb)))
}

fn objects_of(a: &Args) -> Vec<String> {
    a.named()
}

// ---------------------------------------------------------------------------------------------------------------------
// matchTransform and inheritTransform

fn xq(s: &mut Session, name: &str, flag: &str) -> CmdResult<DVec3> {
    let v = s.run("xform", json!({"args": [name], "query": true, "worldSpace": true, flag: true}))?;
    let list: Vec<f64> = v.as_array().map(|x| x.iter().filter_map(Value::as_f64).collect()).unwrap_or_default();
    if list.len() < 3 {
        fail!("matchTransform: can't read {flag} of {name}");
    }
    Ok(DVec3::new(list[0], list[1], list[2]))
}

fn match_transform(s: &mut Session, a: &Args) -> CmdResult {
    let names = objects(s, a);
    if names.len() < 2 {
        fail!("matchTransform: name or select the objects, then the target");
    }
    let target = names.last().unwrap().clone();
    let (pos, rot, scl, piv) = (a.bool("position"), a.bool("rotation"), a.bool("scale"), a.bool("pivots"));
    let none = !(pos || rot || scl || piv);
    s.graph.open_chunk("Match Transform");
    let r = (|| -> CmdResult<()> {
        for name in &names[..names.len() - 1] {
            if rot || none {
                let ro = xq(s, &target, "rotation")?;
                s.run("xform", json!({"args": [name], "worldSpace": true, "rotation": [ro.x, ro.y, ro.z]}))?;
            }
            if scl || none {
                let sc = xq(s, &target, "scale")?;
                s.run("xform", json!({"args": [name], "worldSpace": true, "scale": [sc.x, sc.y, sc.z]}))?;
            }
            if piv {
                let rp = xq(s, &target, "rotatePivot")?;
                let sp = xq(s, &target, "scalePivot")?;
                s.run("xform", json!({"args": [name], "worldSpace": true, "rotatePivot": [rp.x, rp.y, rp.z], "scalePivot": [sp.x, sp.y, sp.z]}))?;
            }
            if pos || none {
                // The pivots line up, as Match Translation does.
                let (want, have) = (xq(s, &target, "rotatePivot")?, xq(s, name, "rotatePivot")?);
                let d = want - have;
                s.run("xform", json!({"args": [name], "worldSpace": true, "relative": true, "translation": [d.x, d.y, d.z]}))?;
            }
        }
        Ok(())
    })();
    s.graph.close_chunk();
    r?;
    s.graph.evaluate();
    Ok(Value::Null)
}

fn inherit_transform(s: &mut Session, a: &Args) -> CmdResult {
    let names = objects(s, a);
    let g = &mut s.graph;
    let mut ids = Vec::new();
    for n in &names {
        let id = find(g, n, "inheritTransform")?;
        if !g.node(id).is_some_and(|x| x.ty().is_a("transform")) {
            fail!("inheritTransform: {n} isn't a transform");
        }
        ids.push(id);
    }
    if a.bool("query") {
        return Ok(Value::Array(ids.iter().map(|&i| json!(g.node(i).unwrap().bool(Transform::INHERITS_TRANSFORM))).collect()));
    }
    g.open_chunk("inheritTransform");
    let r = (|| -> CmdResult<()> {
        for &id in &ids {
            g.evaluate();
            let was = g.node(id).unwrap().bool(Transform::INHERITS_TRANSFORM);
            let world = g.node(id).unwrap().matrix(Transform::WORLD_MATRIX);
            let on = match a.get("toggle") {
                Some(v) if !v.is_boolean() || !v.as_bool().unwrap_or(false) => v.as_bool().unwrap_or(v.as_i64().unwrap_or(0) != 0) ^ true,
                _ => !was,
            };
            let on = if a.has("on") { a.bool("on") } else { on };
            g.set_value(plug(id, Transform::INHERITS_TRANSFORM), PlugValue::Bool(on))?;
            if a.bool("preserve") {
                g.evaluate();
                let parent = g.node(id).unwrap().matrix(Transform::PARENT_MATRIX);
                let local = if on { parent.inverse() * world } else { world };
                let (sc, q, t) = local.to_scale_rotation_translation();
                let euler = q.to_euler(glam::EulerRot::ZYX);
                g.set_value(plug(id, Transform::TRANSLATE), PlugValue::Vec3(t))?;
                g.set_value(plug(id, Transform::ROTATE), PlugValue::Vec3(DVec3::new(euler.2, euler.1, euler.0)))?;
                g.set_value(plug(id, Transform::SCALE), PlugValue::Vec3(sc))?;
            }
        }
        Ok(())
    })();
    g.close_chunk();
    r?;
    Ok(Value::Null)
}

// ---------------------------------------------------------------------------------------------------------------------
// Containers and controllers: references to nodes held as "uuid|name"

fn node_ref(g: &Graph, id: NodeId) -> String {
    let n = g.node(id).unwrap();
    format!("{}|{}", n.uuid(), g.path(id))
}

fn resolve_ref(g: &Graph, text: &str) -> Option<NodeId> {
    let (uuid, name) = text.split_once('|').unwrap_or(("", text));
    if !uuid.is_empty() {
        if let Some(id) = g.nodes().into_iter().find(|&i| g.node(i).is_some_and(|n| n.uuid() == uuid)) {
            return Some(id);
        }
    }
    g.find(name).ok().flatten()
}

const CT_MEMBER: usize = 0;

fn is_type(g: &Graph, id: NodeId, ty: &str) -> bool {
    g.node(id).is_some_and(|n| n.ty().name == ty)
}

fn container_members(g: &Graph, c: NodeId) -> Vec<(i32, NodeId)> {
    let n = g.node(c).unwrap();
    n.elements(CT_MEMBER).iter().filter_map(|(i, v)| if let PlugValue::String(t) = v { resolve_ref(g, t).map(|x| (*i, x)) } else { None }).collect()
}

fn str_list(a: &Args, flag: &str) -> Vec<String> {
    match a.get(flag) {
        Some(Value::Array(v)) => v.iter().filter_map(|x| x.as_str().map(str::to_string)).collect(),
        Some(Value::String(t)) => vec![t.clone()],
        _ => Vec::new(),
    }
}

fn container_cmd(s: &mut Session, a: &Args) -> CmdResult {
    let g = &mut s.graph;
    let target = a.named().first().and_then(|n| g.find(n).ok().flatten());
    if a.bool("query") {
        if asked(a, "findContainer") || a.has("findContainer") {
            let names = if a.has("findContainer") { str_list(a, "findContainer") } else { a.named() };
            let Some(n) = names.first() else { fail!("container: name a node") };
            let id = find(g, n, "container")?;
            for c in g.nodes() {
                if is_type(g, c, "container") && container_members(g, c).iter().any(|(_, m)| *m == id) {
                    return Ok(json!(g.path(c)));
                }
            }
            return Ok(json!(""));
        }
        let Some(c) = target.filter(|&x| is_type(g, x, "container")) else { fail!("container: name a container") };
        if asked(a, "nodeList") {
            return Ok(Value::Array(container_members(g, c).iter().map(|(_, m)| json!(g.path(*m))).collect()));
        }
        if asked(a, "isContainer") {
            return Ok(json!(true));
        }
        fail!("container: query what?");
    }
    let add: Vec<String> = str_list(a, "addNode").into_iter().chain(str_list(a, "add")).collect();
    let rm: Vec<String> = str_list(a, "removeNode").into_iter().chain(str_list(a, "rm")).collect();
    g.open_chunk("container");
    let r = (|| -> CmdResult<NodeId> {
        let c = if a.bool("edit") {
            let Some(c) = target.filter(|&x| is_type(g, x, "container")) else { fail!("container: name a container") };
            c
        } else {
            let t = types::find("container").unwrap();
            let c = g.create_node(&t, &a.string("name", "container1"), NO_NODE, true)?;
            if a.has("asset") {
                g.set_value(plug(c, 1), PlugValue::Bool(true))?;
            }
            c
        };
        let mut next = g.node(c).unwrap().elements(CT_MEMBER).iter().map(|e| e.0 + 1).max().unwrap_or(0);
        for name in &add {
            let id = find(g, name, "container")?;
            if container_members(g, c).iter().any(|(_, m)| *m == id) {
                continue;
            }
            let text = node_ref(g, id);
            g.set_value(PlugRef::element(c, CT_MEMBER as i32, next), PlugValue::String(text))?;
            next += 1;
        }
        for name in &rm {
            let id = find(g, name, "container")?;
            for (i, m) in container_members(g, c) {
                if m == id {
                    g.remove_element(&PlugRef::element(c, CT_MEMBER as i32, i))?;
                }
            }
        }
        Ok(c)
    })();
    g.close_chunk();
    let c = r?;
    Ok(json!(g.path(c)))
}

const CTL_OBJECT: usize = 0;
const CTL_PARENT: usize = 1;
const CTL_CHILDREN: usize = 2;

fn message_of(g: &Graph, id: NodeId) -> Option<PlugRef> {
    let a = g.node(id)?.ty().attr_index("message")?;
    Some(plug(id, a))
}

fn tag_of(g: &Graph, obj: NodeId) -> Option<NodeId> {
    g.destinations(&message_of(g, obj)?).into_iter().find(|d| d.attr as usize == CTL_OBJECT && is_type(g, d.node, "controller")).map(|d| d.node)
}

fn object_of_tag(g: &Graph, tag: NodeId) -> Option<NodeId> {
    g.source(&plug(tag, CTL_OBJECT)).map(|s| s.node)
}

/// As the reference wires it: a tag's `parent` goes into its parent's `children`.
fn parent_tag(g: &Graph, tag: NodeId) -> Option<NodeId> {
    g.destinations(&plug(tag, CTL_PARENT)).into_iter().find(|d| d.attr as usize == CTL_CHILDREN).map(|d| d.node)
}

fn set_parent_tag(g: &mut Graph, tag: NodeId, parent: Option<NodeId>) -> CmdResult<()> {
    for d in g.destinations(&plug(tag, CTL_PARENT)) {
        g.disconnect(&d);
    }
    if let Some(p) = parent {
        g.connect(&plug(tag, CTL_PARENT), &plug(p, CTL_CHILDREN), true)?;
    }
    Ok(())
}

fn controller_cmd(s: &mut Session, a: &Args) -> CmdResult {
    let names = objects(s, a);
    let g = &mut s.graph;
    if asked(a, "allControllers") {
        return Ok(Value::Array(g.nodes().into_iter().filter(|&c| is_type(g, c, "controller")).filter_map(|c| object_of_tag(g, c)).map(|o| json!(g.path(o))).collect()));
    }
    let tag_named = |g: &Graph, n: &str| -> CmdResult<Option<NodeId>> {
        let id = find(g, n, "controller")?;
        Ok(if is_type(g, id, "controller") { Some(id) } else { tag_of(g, id) })
    };
    if a.bool("query") || asked(a, "isController") {
        let Some(n) = names.first() else { fail!("controller: name an object") };
        let tag = tag_named(g, n)?;
        if asked(a, "isController") {
            return Ok(json!(tag.is_some()));
        }
        let Some(tag) = tag else { fail!("controller: {n} is not a controller") };
        if asked(a, "parent") {
            return Ok(match parent_tag(g, tag).and_then(|p| object_of_tag(g, p)) {
                Some(o) => json!(g.path(o)),
                None => Value::Null,
            });
        }
        if asked(a, "children") {
            let kids: Vec<Value> = g.nodes().into_iter().filter(|&c| is_type(g, c, "controller") && parent_tag(g, c) == Some(tag)).filter_map(|c| object_of_tag(g, c)).map(|o| json!(g.path(o))).collect();
            return Ok(Value::Array(kids));
        }
        fail!("controller: query what?");
    }
    if a.bool("unparent") {
        for n in &names {
            if let Some(tag) = tag_named(g, n)? {
                set_parent_tag(g, tag, None)?;
            }
        }
        return Ok(Value::Null);
    }
    let t = types::find("controller").unwrap();
    let parent_name = a.get("parent").and_then(Value::as_str).map(str::to_string);
    g.open_chunk("controller");
    let r = (|| -> CmdResult<Vec<Value>> {
        let mut made = Vec::new();
        let mut tagged = |g: &mut Graph, n: &str| -> CmdResult<NodeId> {
            let id = find(g, n, "controller")?;
            if let Some(tag) = tag_of(g, id) {
                return Ok(tag);
            }
            let short = g.node(id).unwrap().name().to_string();
            let tag = g.create_node(&t, &format!("{short}_tag"), NO_NODE, true)?;
            let Some(msg) = message_of(g, id) else { fail!("controller: {n} has no message") };
            g.connect(&msg, &plug(tag, CTL_OBJECT), true)?;
            made.push(json!(g.path(tag)));
            Ok(tag)
        };
        let parent = match &parent_name {
            Some(p) => Some(tagged(g, p)?),
            None => None,
        };
        for n in &names {
            let tag = tagged(g, n)?;
            if let Some(p) = parent {
                set_parent_tag(g, tag, Some(p))?;
            }
        }
        Ok(made)
    })();
    g.close_chunk();
    Ok(Value::Array(r?))
}

// ---------------------------------------------------------------------------------------------------------------------
// Dimensions

fn vec_flag(a: &Args, flag: &str) -> Option<DVec3> {
    a.vec3(flag).map(DVec3::from_array)
}

/// A locator exactly there, or a new one.
fn locator_at(g: &mut Graph, at: DVec3) -> CmdResult<NodeId> {
    g.evaluate();
    for id in g.nodes() {
        let Some(n) = g.node(id) else { continue };
        if n.ty().is_a("locator") && n.data().vec3(Locator::WORLD_POSITION).abs_diff_eq(at, 1e-9) {
            return Ok(id);
        }
    }
    let (tr, sh) = crate::dag::create_with_shape(g, "locator", "locator1", NO_NODE)?;
    g.set_value(plug(tr, Transform::TRANSLATE), PlugValue::Vec3(at))?;
    Ok(sh)
}

fn distance_dimension(s: &mut Session, a: &Args) -> CmdResult {
    let (Some(sp), Some(ep)) = (vec_flag(a, "startPoint"), vec_flag(a, "endPoint")) else { fail!("distanceDimension: give -sp and -ep") };
    let g = &mut s.graph;
    g.open_chunk("Distance Tool");
    let r = (|| -> CmdResult<NodeId> {
        let (l0, l1) = (locator_at(g, sp)?, locator_at(g, ep)?);
        let (tr, sh) = crate::dag::create_with_shape(g, "distanceDimShape", "distanceDimension1", NO_NODE)?;
        let _ = tr;
        g.connect(&plug(l0, Locator::WORLD_POSITION), &plug(sh, DD_START), false)?;
        g.connect(&plug(l1, Locator::WORLD_POSITION), &plug(sh, DD_END), false)?;
        Ok(sh)
    })();
    g.close_chunk();
    let sh = r?;
    g.evaluate();
    g.select(&[crate::dag::transform_of(g, sh)], SelectMode::Replace);
    Ok(json!(g.path(sh)))
}

/// curve1.u[0.5], curve1.ep[2], surface1.uv[0.5][1], surface1.u[1]: the object's shape and parameters.
fn param_of(g: &mut Graph, text: &str) -> Option<(NodeId, f64, f64)> {
    let (head, rest) = text.split_once('[')?;
    let (obj, kind) = head.rsplit_once('.')?;
    let id = g.find(obj).ok()??;
    let inner = rest.trim_end_matches(']');
    let (a, b) = inner.split_once("][").unwrap_or((inner, "0"));
    let (x, y): (f64, f64) = (a.parse().ok()?, b.parse().ok()?);
    if let Some(c) = crate::curves::as_curve(g, id) {
        let u = if kind == "ep" { crate::curves::local_curve(g, c)?.distinct_params().get(x as usize).copied()? } else { x };
        return Some((c, u, 0.0));
    }
    let s = crate::surfaces::as_surface(g, id)?;
    Some((s, x, y))
}

fn point_on(g: &mut Graph, shape: NodeId, u: f64, v: f64) -> Option<DVec3> {
    if g.node(shape)?.ty().is_a("nurbsCurve") {
        return crate::curves::world_curve(g, shape).map(|c| c.eval(u));
    }
    crate::surfaces::world_surface(g, shape).map(|s| s.eval(u, v))
}

fn param_dimension(s: &mut Session, a: &Args) -> CmdResult {
    let names = objects(s, a);
    let g = &mut s.graph;
    let Some(text) = names.first() else { fail!("paramDimension: name a point: curve1.u[0.5] or surface1.uv[0.5][0.5]") };
    let Some((shape, u, v)) = param_of(g, text) else { fail!("paramDimension: {text} isn't a point on a curve or surface") };
    let Some(at) = point_on(g, shape, u, v) else { fail!("paramDimension: {text} has no geometry") };
    g.open_chunk("paramDimension");
    let r = (|| -> CmdResult<NodeId> {
        let (tr, sh) = crate::dag::create_with_shape(g, "paramDimension", "paramDimension1", NO_NODE)?;
        g.set_value(plug(tr, Transform::TRANSLATE), PlugValue::Vec3(at))?;
        g.set_value(plug(sh, Shape::COUNT), PlugValue::String(text.clone()))?;
        g.set_value(plug(sh, Shape::COUNT + 1), PlugValue::Float(u))?;
        g.set_value(plug(sh, Shape::COUNT + 2), PlugValue::Float(v))?;
        Ok(sh)
    })();
    g.close_chunk();
    Ok(json!(g.path(r?)))
}

fn annotate(s: &mut Session, a: &Args) -> CmdResult {
    let names = objects(s, a);
    let g = &mut s.graph;
    let at = vec_flag(a, "point");
    let text = a.string("text", "");
    let target = names.first().map(|n| find(g, n, "annotate")).transpose()?;
    g.evaluate();
    let target_pos = target.and_then(|t| g.node(t)).filter(|n| n.ty().dag).map(|n| n.matrix(DagNode::WORLD_MATRIX).w_axis.truncate());
    let at = at.or(target_pos).unwrap_or(DVec3::ZERO);
    g.open_chunk("annotate");
    let r = (|| -> CmdResult<NodeId> {
        let (tr, sh) = crate::dag::create_with_shape(g, "annotationShape", "annotation1", NO_NODE)?;
        g.set_value(plug(tr, Transform::TRANSLATE), PlugValue::Vec3(at))?;
        g.set_value(plug(sh, Shape::COUNT), PlugValue::String(text))?;
        match target {
            Some(t) if g.node(t).is_some_and(|n| n.ty().is_a("locator")) => {
                g.connect(&plug(t, Locator::WORLD_POSITION), &plug(sh, AN_TARGET), false)?;
            }
            _ => {
                g.set_value(plug(sh, AN_TARGET), PlugValue::Vec3(target_pos.unwrap_or(at)))?;
                if target.is_none() {
                    g.set_value(plug(sh, Shape::COUNT + 1), PlugValue::Bool(false))?;
                }
            }
        }
        Ok(sh)
    })();
    g.close_chunk();
    Ok(json!(g.path(r?)))
}

fn dropoff_locator(s: &mut Session, a: &Args) -> CmdResult {
    let names = a.named();
    let nums: Vec<f64> = a.positional.iter().filter_map(Value::as_f64).collect();
    let [dropoff, percent] = nums.as_slice() else { fail!("dropoffLocator: dropoff percent curve.u[...]") };
    let g = &mut s.graph;
    let Some(text) = names.first() else { fail!("dropoffLocator: name a wire curve's point") };
    let Some((shape, u, v)) = param_of(g, text) else { fail!("dropoffLocator: {text} isn't a point on a curve") };
    let Some(at) = point_on(g, shape, u, v) else { fail!("dropoffLocator: {text} has no geometry") };
    g.open_chunk("dropoffLocator");
    let r = (|| -> CmdResult<NodeId> {
        let (tr, sh) = crate::dag::create_with_shape(g, "dropoffLocator", "dropoffLocator1", NO_NODE)?;
        g.set_value(plug(tr, Transform::TRANSLATE), PlugValue::Vec3(at))?;
        g.set_value(plug(sh, Shape::COUNT), PlugValue::Float(u))?;
        g.set_value(plug(sh, Shape::COUNT + 1), PlugValue::Float(*percent))?;
        g.set_value(plug(sh, Shape::COUNT + 2), PlugValue::Float(*dropoff))?;
        g.set_value(plug(sh, Shape::COUNT + 3), PlugValue::String(text.clone()))?;
        Ok(tr)
    })();
    g.close_chunk();
    Ok(json!([g.path(r?)]))
}

// ---------------------------------------------------------------------------------------------------------------------
// Asking about and copying deformers

fn find_deformers(s: &mut Session, a: &Args) -> CmdResult {
    let names = objects(s, a);
    let g = &mut s.graph;
    let mut out = Vec::new();
    for n in names {
        let id = find(g, &n, "findDeformers")?;
        let Some(m) = crate::components::as_mesh(g, id) else { continue };
        for h in crate::history::history_of(g, m) {
            let ty = g.node(h).map(|x| x.ty().name.clone()).unwrap_or_default();
            if crate::deform::is_deformer(&ty) || ty == crate::skin::SKIN_TYPE {
                out.push(json!(g.path(h)));
            }
        }
    }
    Ok(Value::Array(out))
}

fn copy_deformer_weights(s: &mut Session, a: &Args) -> CmdResult {
    let (ss, ds) = (a.string("sourceShape", ""), a.string("destinationShape", ""));
    let (sd, dd) = (a.string("sourceDeformer", ""), a.string("destinationDeformer", ""));
    if sd.is_empty() || dd.is_empty() {
        fail!("copyDeformerWeights: give -sd and -dd");
    }
    let g = &mut s.graph;
    let (src, dst) = (crate::deform_cmds::deformer_named(g, &sd, "copyDeformerWeights")?, crate::deform_cmds::deformer_named(g, &dd, "copyDeformerWeights")?);
    g.evaluate();
    let Some(input_s) = crate::deform::input_mesh(g, src) else { fail!("copyDeformerWeights: {sd} has no mesh") };
    let Some(input_d) = crate::deform::input_mesh(g, dst) else { fail!("copyDeformerWeights: {dd} has no mesh") };
    let (ns, nd) = (input_s.point_capacity() as usize, input_d.point_capacity() as usize);
    let mut from = crate::deform::floats_of(g.node(src).unwrap().data(), crate::deform::WEIGHTS).map(|f| f.0.clone()).unwrap_or_else(|| vec![1.0; ns]);
    from.resize(ns, 0.0);
    let geom = |g: &mut Graph, name: &str, fallback: NodeId| -> DMat4 {
        let id = if name.is_empty() { None } else { g.find(name).ok().flatten() };
        let id = id.or_else(|| crate::deform::geometry_names(g, fallback).first().and_then(|n| g.find(n).ok().flatten()));
        match id {
            Some(i) => crate::rig::world(g, crate::dag::transform_of(g, i)),
            None => DMat4::IDENTITY,
        }
    };
    let (ws, wd) = (geom(g, &ss, src), geom(g, &ds, dst));
    let mirror = a.get("mirrorMode").and_then(Value::as_str).map(str::to_string).filter(|_| !a.bool("noMirror"));
    let flip = |p: DVec3| -> DVec3 {
        match mirror.as_deref() {
            Some("XY") => DVec3::new(p.x, p.y, -p.z),
            Some("XZ") => DVec3::new(p.x, -p.y, p.z),
            Some("YZ") => DVec3::new(-p.x, p.y, p.z),
            _ => p,
        }
    };
    // Closest point: the source points in the destination's world.
    let spots: Vec<DVec3> = (0..ns as u32).filter(|&p| input_s.point_valid(p)).map(|p| ws.transform_point3(input_s.position(p))).collect();
    let keys: Vec<usize> = (0..ns as u32).filter(|&p| input_s.point_valid(p)).map(|p| p as usize).collect();
    let mut w = vec![0.0; nd];
    for p in 0..nd as u32 {
        if !input_d.point_valid(p) {
            continue;
        }
        let q = flip(wd.transform_point3(input_d.position(p)));
        let mut best = (f64::MAX, 0usize);
        for (i, sp) in spots.iter().enumerate() {
            let d = sp.distance_squared(q);
            if d < best.0 {
                best = (d, keys[i]);
            }
        }
        w[p as usize] = from.get(best.1).copied().unwrap_or(0.0);
    }
    crate::deform::set_floats(g, dst, crate::deform::WEIGHTS, w)?;
    Ok(Value::Null)
}

pub fn register(c: &mut Commands<Session>) {
    register_types();
    for name in ["bindSkin", "rigidBind"] {
        c.register(
            Cmd::new(name, "Rigid bind: each point follows one joint, a skinCluster of 1.0 weights. -tsb only the bones named, -ub unbinds, -ubk keeps history. Returns 1.", bind_skin)
                .positional("skeleton and geometry")
                .flag(f("byClosestPoint", "bcp", B, "Accepted: closest point is how it binds."))
                .flag(f("byPartition", "bp", B, "Accepted for existing scripts."))
                .flag(f("colorJoints", "cj", B, "Accepted for existing scripts."))
                .flag(f("delete", "del", B, "Remove the skin."))
                .flag(f("doNotDescend", "dnd", B, "Only the joints named."))
                .flag(f("enable", "en", B, "Accepted for existing scripts."))
                .flag(f("name", "n", S, "The skinCluster's name."))
                .flag(f("partition", "p", S, "Accepted for existing scripts."))
                .flag(f("toAll", "ta", B, "The whole skeleton (the default)."))
                .flag(f("toSelectedBones", "tsb", B, "Only the joints named."))
                .flag(f("unbind", "ub", B, "Remove the skin."))
                .flag(f("unbindKeepHistory", "ubk", B, "Stop skinning, keep the node.")),
        );
    }
    c.register(
        Cmd::new("geomBind", "Recomputes a skinCluster's weights following the surface: -bm 3 (the geodesic voxel method, which runs the heat bind here), -mi, -fo.", geom_bind)
            .positional("skinCluster or skinned mesh")
            .flag(f("bindMethod", "bm", I, "3 geodesic voxel; 2 heat map; 0 and 1 closest distance and hierarchy."))
            .flag(f("falloff", "fo", F, "0 to 1, how far influences reach."))
            .flag(f("maxInfluences", "mi", I, "Most influences on a point."))
            .flag(f("geodesicVoxelParams", "gvp", L, "Accepted for existing scripts."))
            .flag(f("postVoxelSmooth", "pvs", I, "Accepted for existing scripts.")),
    );
    c.register(
        Cmd::new("jointCluster", "Record a joint's rigid bind settings on a jointCluster node: -j, -ab, -bb, -av, -bv. Returns the node. It softens nothing here.", joint_cluster)
            .positional("joint")
            .flag(f("joint", "j", S, "The joint."))
            .flag(f("aboveBound", "ab", F, "How far above, percent."))
            .flag(f("belowBound", "bb", F, "How far below, percent."))
            .flag(f("aboveValue", "av", F, "Weight above."))
            .flag(f("belowValue", "bv", F, "Weight below."))
            .flag(f("aboveDropoffType", "adt", S, "Accepted for existing scripts."))
            .flag(f("belowDropoffType", "bdt", S, "Accepted for existing scripts."))
            .flag(f("aboveCluster", "ac", B, "Accepted for existing scripts."))
            .flag(f("belowCluster", "bc", B, "Accepted for existing scripts."))
            .flag(f("deformerTools", "dt", B, "Accepted for existing scripts."))
            .flag(f("name", "n", S, "The node's name.")),
    );
    c.register(
        q_flags(Cmd::new("pairBlend", "Put a pairBlend on translates or rotates: -nd node -at attr. What drove the attribute feeds input 1. Returns the node; -q -in1 -in2 -at read its connections.", pair_blend))
            .positional("pairBlend, with -q")
            .flag(f("node", "nd", S, "The node driven."))
            .flag(f("attribute", "at", ArgType::Any, "The attributes.").multi(1))
            .flag(f("input1", "i1", B, "With -q: what feeds the first pair."))
            .flag(f("input2", "i2", B, "With -q: what feeds the second pair."))
            .flag(f("name", "n", S, "The node's name.")),
    );
    c.register(
        Cmd::new("matchTransform", "Match objects to the last one: -pos, -rot, -scl, -piv; all three transforms when none.", match_transform)
            .positional("objects, then the target")
            .flag(f("position", "pos", B, "Line the pivots up."))
            .flag(f("rotation", "rot", B, "Match rotation."))
            .flag(f("scale", "scl", B, "Match scale."))
            .flag(f("pivots", "piv", B, "Match the pivots."))
            .flag(f("positionConstraint", "pcp", B, "Accepted for existing scripts.")),
    );
    c.register(
        q_flags(Cmd::new("inheritTransform", "Whether objects follow their parent's transform: toggles, -on sets, -p keeps them where they are; -q reads each.", inherit_transform))
            .positional("objects")
            .flag(f("toggle", "tgl", ArgType::Any, "Toggle (what it does anyway)."))
            .flag(f("on", "on", B, "Set on or off."))
            .flag(f("preserve", "p", B, "Keep the world position.")),
    );
    c.register(
        q_flags(Cmd::new("container", "Group nodes in a container node: -n, -an adds, -rn removes; with -e on an existing one. -q -nl lists, -q -fc finds the one holding a node.", container_cmd))
            .positional("container")
            .flag(f("name", "n", S, "The container's name."))
            .flag(f("addNode", "an", ArgType::Any, "Nodes to add."))
            .flag(f("add", "add", ArgType::Any, "Nodes to add."))
            .flag(f("removeNode", "rn", ArgType::Any, "Nodes to take out."))
            .flag(f("rm", "rm", ArgType::Any, "Nodes to take out."))
            .flag(f("nodeList", "nl", B, "With -q: its nodes."))
            .flag(f("findContainer", "fc", ArgType::Any, "With -q: the container a node is in."))
            .flag(f("isContainer", "isc", B, "With -q: whether it's a container."))
            .flag(f("asset", "a", S, "Marks it an asset."))
            .flag(f("force", "f", B, "Accepted for existing scripts."))
            .flag(f("type", "typ", S, "Accepted for existing scripts."))
            .flag(f("includeHierarchyBelow", "ihb", B, "Accepted for existing scripts.")),
    );
    c.register(
        q_flags(Cmd::new("controller", "Tag objects as animation controls: controller obj; -p parent makes them its children; -q -p, -q -c read the tree; -ac all; -ic is it one; -up unparents.", controller_cmd))
            .positional("objects")
            .flag(f("parent", "p", ArgType::Any, "The parent control, or with -q ask."))
            .flag(f("children", "c", B, "With -q: its children."))
            .flag(f("allControllers", "ac", B, "All the controls."))
            .flag(f("isController", "ic", B, "Whether the object is a control."))
            .flag(f("unparent", "unp", B, "Take from its parent."))
            .flag(f("group", "g", B, "Accepted for existing scripts."))
            .flag(f("index", "idx", I, "Accepted for existing scripts."))
            .flag(f("pickWalkDown", "pwd", B, "Accepted for existing scripts."))
            .flag(f("pickWalkUp", "pwu", B, "Accepted for existing scripts.")),
    );
    c.register(
        Cmd::new("annotate", "A note in the viewport: -tx text, -p where; the line points at the object named. Returns the shape.", annotate)
            .positional("object")
            .flag(f("text", "tx", S, "The note."))
            .flag(f("point", "p", ArgType::Vec3, "Where it goes.")),
    );
    c.register(
        Cmd::new("distanceDimension", "Measure between two points: -sp, -ep, with locators there (made when none is). Returns the shape, whose .distance reads the length.", distance_dimension)
            .flag(f("startPoint", "sp", ArgType::Vec3, "One end."))
            .flag(f("endPoint", "ep", ArgType::Vec3, "The other end.")),
    );
    c.register(Cmd::new("paramDimension", "Label a point with its parameter: curve1.u[0.5] or surface1.uv[0.5][0.5]. Returns the shape.", param_dimension).positional("point"));
    c.register(Cmd::new("dropoffLocator", "Mark a wire's dropoff at a point: dropoff percent wire.u[...]. Returns the locator. The wire's falloff stays as it is.", dropoff_locator).positional("dropoff percent point"));
    c.register(Cmd::new("findDeformers", "The deformers on a mesh, nearest the shape first.", find_deformers).positional("objects").undoable(false));
    c.register(
        Cmd::new("copyDeformerWeights", "Copy a deformer's weights to another by closest point in world space: -sd, -dd, -ss, -ds; -mm XY, XZ or YZ to mirror them.", copy_deformer_weights)
            .flag(f("sourceDeformer", "sd", S, "The deformer to copy from."))
            .flag(f("destinationDeformer", "dd", S, "The deformer to copy to."))
            .flag(f("sourceShape", "ss", S, "Its shape."))
            .flag(f("destinationShape", "ds", S, "Its shape."))
            .flag(f("mirrorMode", "mm", S, "XY, XZ or YZ."))
            .flag(f("mirrorInverse", "mi", B, "Accepted for existing scripts."))
            .flag(f("noMirror", "nm", B, "Don't mirror."))
            .flag(f("smooth", "sm", B, "Accepted for existing scripts."))
            .flag(f("surfaceAssociation", "sa", S, "Accepted: closest point is what's used."))
            .flag(f("uvSpace", "uv", L, "Accepted for existing scripts.")),
    );
}
