//! Light commands: directionalLight, pointLight, spotLight, ambientLight with create, query and edit; shadingNode -asLight;
//! the Create menu's default*Light procedures and Create*Light runtime commands; listLights; lightlink.
//!
//! Links live on a lightLinker1 node made when first needed, in hidden strings of node ids, so undo restores them and a rename
//! doesn't break them. A light lights an object if a link says so and no ignore says otherwise, or else if it is in
//! defaultLightSet.

use std::sync::Arc;

use aztek_ops::{fail, ArgType, Args, CmdResult, CommandDef, Commands, FlagDef};
use glam::DVec3;
use serde_json::{json, Value};

use crate::graph::{Graph, SelectMode};
use crate::lights as li;
use crate::ref_core::extend;
use crate::node::{types, EvalContext, NodeBehavior, NodeData, NodeType};
use crate::plug::{flags, AttrDef, NodeId, PlugRef, PlugType, PlugValue, NO_NODE};
use crate::rig::plug;
use crate::session::Session;
use crate::transform::Transform;

type Cmd = CommandDef<Session>;

const LINKER: &str = "lightLinker1";
pub const LINKER_TYPE: &str = "lightLinker";

fn f(name: &str, short: &str, ty: ArgType, doc: &str) -> FlagDef {
    FlagDef::new(name, short, ty, Value::Null, doc)
}

fn asked(a: &Args, name: &str) -> bool {
    a.bool(name) || a.get("queried").and_then(Value::as_array).is_some_and(|q| q.iter().any(|x| x == name))
}

/// A command flag and the shape attribute it stands for.
const COMMON: &[(&str, &str, &str, ArgType)] = &[("intensity", "i", "intensity", ArgType::Float), ("shadowSamples", "sh", "shadowRays", ArgType::Int), ("useRayTraceShadows", "rs", "useRayTraceShadows", ArgType::Bool)];
const NON_AMBIENT: &[(&str, &str, &str, ArgType)] = &[("decayRate", "d", "decayRate", ArgType::Int), ("softShadow", "ss", "castSoftShadows", ArgType::Bool), ("discRadius", "drs", "lightRadius", ArgType::Float)];
const SPOT: &[(&str, &str, &str, ArgType)] = &[
    ("coneAngle", "ca", "coneAngle", ArgType::Float),
    ("penumbra", "p", "penumbraAngle", ArgType::Float),
    ("dropOff", "do", "dropoff", ArgType::Float),
    ("barnDoors", "bd", "barnDoors", ArgType::Bool),
    ("leftBarnDoorAngle", "lbd", "leftBarnDoor", ArgType::Float),
    ("rightBarnDoorAngle", "rbd", "rightBarnDoor", ArgType::Float),
    ("topBarnDoorAngle", "tbd", "topBarnDoor", ArgType::Float),
    ("bottomBarnDoorAngle", "bbd", "bottomBarnDoor", ArgType::Float),
];
const AMBIENT: &[(&str, &str, &str, ArgType)] = &[("ambientShade", "ambientShade", "ambientShade", ArgType::Float), ("discRadius", "drs", "shadowRadius", ArgType::Float)];

fn table_for(ty: &str) -> Vec<&'static (&'static str, &'static str, &'static str, ArgType)> {
    let mut v: Vec<_> = COMMON.iter().collect();
    match ty {
        "ambientLight" => v.extend(AMBIENT),
        "spotLight" => {
            v.extend(NON_AMBIENT);
            v.extend(SPOT);
        }
        _ => v.extend(NON_AMBIENT),
    }
    v
}

fn attr_plug(g: &Graph, id: NodeId, name: &str) -> Option<PlugRef> {
    g.node(id)?.ty().attr_index(name).map(|i| PlugRef::new(id, i as i32))
}

fn set_num(g: &mut Graph, id: NodeId, name: &str, v: f64) -> CmdResult<()> {
    let Some(p) = attr_plug(g, id, name) else { return Ok(()) };
    let val = match g.attr(&p).ty {
        PlugType::Bool => PlugValue::Bool(v != 0.0),
        PlugType::Int | PlugType::Enum => PlugValue::Int(v as i64),
        _ => PlugValue::Float(v),
    };
    g.set_value(p, val)?;
    Ok(())
}

fn set_vec(g: &mut Graph, id: NodeId, name: &str, v: [f64; 3]) -> CmdResult<()> {
    if let Some(p) = attr_plug(g, id, name) {
        g.set_value(p, PlugValue::Vec3(DVec3::from_array(v)))?;
    }
    Ok(())
}

/// A transform and light shape, in defaultLightSet.
pub fn make(g: &mut Graph, ty: &str, name: &str, parent: NodeId) -> CmdResult<(NodeId, NodeId)> {
    let (tr, sh) = crate::dag::create_with_shape(g, ty, name, parent)?;
    li::set_illuminates_by_default(g, tr, true)?;
    Ok((tr, sh))
}

fn apply(g: &mut Graph, a: &Args, ty: &str, tr: NodeId, sh: NodeId) -> CmdResult<()> {
    for (flag, _, attr, kind) in table_for(ty) {
        if !a.has(flag) {
            continue;
        }
        let v = match kind {
            ArgType::Bool => a.bool(flag) as i32 as f64,
            _ => a.float(flag, 0.0),
        };
        set_num(g, sh, attr, v)?;
    }
    if let Some(c) = a.vec3("rgb") {
        set_vec(g, sh, "color", c)?;
    }
    if let Some(c) = a.vec3("shadowColor") {
        set_vec(g, sh, "shadowColor", c)?;
    }
    if let Some(p) = a.vec3("position") {
        g.set_value(plug(tr, Transform::TRANSLATE), PlugValue::Vec3(DVec3::from_array(p)))?;
    }
    if let Some(r) = a.vec3("rotation") {
        g.set_value(plug(tr, Transform::ROTATE), PlugValue::Vec3(DVec3::from_array(r)))?;
    }
    if a.has("exclusive") {
        li::set_illuminates_by_default(g, tr, !a.bool("exclusive"))?;
    }
    Ok(())
}

fn query(g: &mut Graph, a: &Args, ty: &str, tr: NodeId, sh: NodeId) -> CmdResult {
    for (flag, _, attr, _) in table_for(ty) {
        if asked(a, flag) {
            let Some(p) = attr_plug(g, sh, attr) else { return Ok(Value::Null) };
            return Ok(match g.value(&p)? {
                PlugValue::Bool(b) => json!(b),
                PlugValue::Int(i) => json!(i),
                v => json!(v.scalar().unwrap_or(0.0)),
            });
        }
    }
    let vec_of = |g: &mut Graph, p: PlugRef| -> CmdResult {
        Ok(match g.value(&p)? {
            PlugValue::Vec3(v) => json!([v.x, v.y, v.z]),
            _ => Value::Null,
        })
    };
    for (flag, attr) in [("rgb", "color"), ("shadowColor", "shadowColor")] {
        if asked(a, flag) {
            let Some(p) = attr_plug(g, sh, attr) else { return Ok(Value::Null) };
            return vec_of(g, p);
        }
    }
    if asked(a, "position") {
        return vec_of(g, plug(tr, Transform::TRANSLATE));
    }
    if asked(a, "rotation") {
        return vec_of(g, plug(tr, Transform::ROTATE));
    }
    if asked(a, "exclusive") {
        return Ok(json!(!li::illuminates_by_default(g, tr)));
    }
    if asked(a, "name") {
        return Ok(json!(g.path(sh)));
    }
    Ok(Value::Null)
}

fn light_command(s: &mut Session, a: &Args, ty: &str) -> CmdResult {
    let g = &mut s.graph;
    if a.bool("query") || a.bool("edit") {
        let names = a.named();
        let n = match names.first() {
            Some(n) => n.clone(),
            None => match g.selection().last() {
                Some(&i) => g.path(i),
                None => fail!("{ty}: name a light"),
            },
        };
        let id = crate::rig::find(g, &n, ty)?;
        let Some(sh) = li::shape_of(g, id).filter(|&sh| g.node(sh).is_some_and(|x| x.ty().is_a(ty))) else { fail!("{ty}: {n} is not a {ty}") };
        let tr = crate::dag::transform_of(g, sh);
        if a.bool("query") {
            return query(g, a, ty, tr, sh);
        }
        apply(g, a, ty, tr, sh)?;
        return Ok(json!(g.path(sh)));
    }
    let (tr, sh) = make(g, ty, &a.string("name", ""), NO_NODE)?;
    apply(g, a, ty, tr, sh)?;
    g.select(&[tr], SelectMode::Replace);
    Ok(json!(g.path(sh)))
}

fn number(v: Option<&Value>) -> Option<f64> {
    match v? {
        Value::String(x) => x.parse().ok(),
        Value::Bool(b) => Some(*b as i32 as f64),
        x => x.as_f64(),
    }
}

/// The Create menu's procedures, default*Light(intensity, r, g, b, ...). Their arguments as the Script Editor echoes them;
/// the node reference doesn't document these. What they mean after the colour is read for spot and ambient lights only.
fn default_light(s: &mut Session, a: &Args, ty: &str) -> CmdResult {
    let g = &mut s.graph;
    let (tr, sh) = make(g, ty, "", NO_NODE)?;
    let at = |i: usize| number(a.positional.get(i));
    if let Some(i) = at(0) {
        set_num(g, sh, "intensity", i)?;
    }
    let first_colour = if ty == "ambientLight" {
        if let Some(x) = at(1) {
            set_num(g, sh, "ambientShade", x)?;
        }
        2
    } else {
        1
    };
    if let (Some(r), Some(gr), Some(b)) = (at(first_colour), at(first_colour + 1), at(first_colour + 2)) {
        set_vec(g, sh, "color", [r, gr, b])?;
    }
    if ty == "spotLight" {
        for (i, attr) in [(5, "coneAngle"), (6, "penumbraAngle"), (7, "dropoff")] {
            if let Some(x) = at(i) {
                set_num(g, sh, attr, x)?;
            }
        }
    }
    g.select(&[tr], SelectMode::Replace);
    Ok(json!(g.path(tr)))
}

// ---------------------------------------------------------------------------------------------------------------------
// Links

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LinkKind {
    Link,
    Ignore,
    ShadowLink,
    ShadowIgnore,
}

impl LinkKind {
    pub const ALL: [LinkKind; 4] = [LinkKind::Link, LinkKind::Ignore, LinkKind::ShadowLink, LinkKind::ShadowIgnore];
    pub fn attr(self) -> &'static str {
        match self {
            LinkKind::Link => "aztekLink",
            LinkKind::Ignore => "aztekIgnore",
            LinkKind::ShadowLink => "aztekShadowLink",
            LinkKind::ShadowIgnore => "aztekShadowIgnore",
        }
    }
    /// The word in a file's `relationship` line.
    pub fn word(self) -> &'static str {
        match self {
            LinkKind::Link => "link",
            LinkKind::Ignore => "ignore",
            LinkKind::ShadowLink => "shadowLink",
            LinkKind::ShadowIgnore => "shadowIgnore",
        }
    }
    pub fn from_word(w: &str) -> Option<LinkKind> {
        LinkKind::ALL.into_iter().find(|k| k.word() == w)
    }
}

struct Linker;

impl NodeBehavior for Linker {
    fn compute(&mut self, _ctx: &EvalContext, _d: &mut NodeData) {}
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

pub fn register_types() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(register_linker);
}

fn register_linker() {
    let held = |name: &str| AttrDef { name: name.into(), ty: PlugType::String, default_value: PlugValue::String(String::new()), flags: flags::INPUT | flags::HIDDEN, ..Default::default() };
    types::register_type(NodeType {
        name: LINKER_TYPE.into(),
        doc: "Which lights light which objects, beyond what defaultLightSet says.".into(),
        own_attrs: LinkKind::ALL.iter().map(|k| held(k.attr())).collect(),
        create: Some(Arc::new(|| Box::new(Linker) as Box<dyn NodeBehavior>)),
        ..Default::default()
    })
    .expect(LINKER_TYPE);
}

/// (light or light set, object or object set) pairs of one kind.
pub fn pairs(g: &Graph, kind: LinkKind) -> Vec<(NodeId, NodeId)> {
    let Some(id) = g.find(LINKER).ok().flatten() else { return Vec::new() };
    let Some(n) = g.node(id) else { return Vec::new() };
    let Some(i) = n.ty().attr_index(kind.attr()) else { return Vec::new() };
    let PlugValue::String(text) = n.value(i) else { return Vec::new() };
    text.split(';')
        .filter_map(|p| {
            let (a, b) = p.split_once(' ')?;
            Some((a.parse().ok()?, b.parse().ok()?))
        })
        .filter(|(a, b)| g.node(*a).is_some() && g.node(*b).is_some())
        .collect()
}

fn store(g: &mut Graph, kind: LinkKind, list: &[(NodeId, NodeId)]) -> CmdResult<()> {
    let id = match g.find(LINKER)? {
        Some(id) => id,
        None => g.create_node(&types::find(LINKER_TYPE).unwrap(), LINKER, NO_NODE, false)?,
    };
    let Some(p) = attr_plug(g, id, kind.attr()) else { return Ok(()) };
    let text: Vec<String> = list.iter().map(|(a, b)| format!("{a} {b}")).collect();
    g.set_value(p, PlugValue::String(text.join(";")))?;
    Ok(())
}

pub fn add_pair(g: &mut Graph, kind: LinkKind, light: NodeId, object: NodeId) -> CmdResult<()> {
    let mut list = pairs(g, kind);
    if !list.contains(&(light, object)) {
        list.push((light, object));
        store(g, kind, &list)?;
    }
    Ok(())
}

fn remove_pair(g: &mut Graph, kind: LinkKind, light: NodeId, object: NodeId) -> CmdResult<()> {
    let mut list = pairs(g, kind);
    let before = list.len();
    list.retain(|p| *p != (light, object));
    if list.len() != before {
        store(g, kind, &list)?;
    }
    Ok(())
}

/// A pair's light end names this light: it, its transform or shape, or a set holding it.
fn names_light(g: &Graph, end: NodeId, tr: NodeId, sh: NodeId) -> bool {
    end == tr || end == sh || (crate::object_sets::is_set(g.node(end).unwrap()) && crate::object_sets::nodes_of(g, end).iter().any(|m| *m == tr || *m == sh))
}

/// A pair's object end names this shape: it, something above it, or a set or shading group holding one of those.
fn names_object(g: &Graph, end: NodeId, shape: NodeId) -> bool {
    let mut chain = Vec::new();
    let mut at = shape;
    while at != NO_NODE {
        chain.push(at);
        at = g.parent(at);
    }
    chain.contains(&end) || (crate::object_sets::is_set(g.node(end).unwrap()) && crate::object_sets::nodes_of(g, end).iter().any(|m| chain.contains(m)))
}

/// What the links say, read once for many questions.
pub struct LinkTable {
    pub links: Vec<(NodeId, NodeId)>,
    pub ignores: Vec<(NodeId, NodeId)>,
}

impl LinkTable {
    pub fn read(g: &Graph) -> LinkTable {
        LinkTable { links: pairs(g, LinkKind::Link), ignores: pairs(g, LinkKind::Ignore) }
    }
    pub fn is_empty(&self) -> bool {
        self.links.is_empty() && self.ignores.is_empty()
    }
    /// Whether a light lights a shape.
    pub fn lights(&self, g: &Graph, light_shape: NodeId, shape: NodeId) -> bool {
        let tr = crate::dag::transform_of(g, light_shape);
        let says = |list: &[(NodeId, NodeId)]| list.iter().any(|(l, o)| names_light(g, *l, tr, light_shape) && names_object(g, *o, shape));
        if says(&self.ignores) {
            return false;
        }
        says(&self.links) || li::illuminates_by_default(g, tr)
    }
}

fn shaded_shapes(g: &Graph) -> Vec<NodeId> {
    g.nodes().into_iter().filter(|&id| g.node(id).is_some_and(|n| n.ty().is_a("mesh") || n.ty().is_a("nurbsSurface"))).collect()
}

fn name_list(a: &Args, flag: &str) -> Vec<String> {
    match a.get(flag) {
        Some(Value::String(x)) => vec![x.clone()],
        Some(Value::Array(v)) => v.iter().filter_map(|x| x.as_str().map(str::to_string)).collect(),
        _ => Vec::new(),
    }
}

fn lightlink(s: &mut Session, a: &Args) -> CmdResult {
    let g = &mut s.graph;
    let is_light = |g: &Graph, id: NodeId| li::shape_of(g, id).is_some() || g.node(id).is_some_and(crate::object_sets::is_set);
    let mut light_names = name_list(a, "light");
    let mut object_names = name_list(a, "object");
    if a.bool("useActiveLights") || a.bool("useActiveObjects") {
        for &id in g.selection().to_vec().iter() {
            let n = g.path(id);
            if a.bool("useActiveLights") && li::shape_of(g, id).is_some() {
                light_names.push(n);
            } else if a.bool("useActiveObjects") && li::shape_of(g, id).is_none() {
                object_names.push(n);
            }
        }
    }
    // Names with no flag: lights one way, objects the other.
    for n in a.named() {
        match g.find(&n)? {
            Some(id) if is_light(g, id) => light_names.push(n),
            Some(_) => object_names.push(n),
            None => fail!("lightlink: No object matches name: {n}"),
        }
    }
    let ids = |g: &Graph, names: &[String]| -> CmdResult<Vec<NodeId>> {
        names
            .iter()
            .map(|n| match g.find(n) {
                Ok(Some(id)) => Ok(id),
                _ => Err(aztek_ops::CommandError::new(format!("lightlink: No object matches name: {n}"))),
            })
            .collect()
    };
    let lights = ids(g, &light_names)?;
    let objects = ids(g, &object_names)?;
    let shadow = a.bool("shadow");
    let breaking = a.bool("b");
    let making = a.bool("make");
    let querying = a.bool("query") || (!breaking && !making && (lights.is_empty() || objects.is_empty()));
    if querying {
        let want = |flag: &str| a.get(flag).and_then(Value::as_bool).unwrap_or(true);
        let t = LinkTable::read(g);
        let mut out: Vec<String> = Vec::new();
        let mut push = |x: String| {
            if !out.contains(&x) {
                out.push(x);
            }
        };
        if let Some(&l) = lights.first() {
            let (tr, sh) = match li::shape_of(g, l) {
                Some(sh) => (crate::dag::transform_of(g, sh), sh),
                None => (l, l),
            };
            if li::shape_of(g, l).is_some() {
                if want("sets") {
                    for (e, o) in &t.links {
                        if names_light(g, *e, tr, sh) && g.node(*o).is_some_and(crate::object_sets::is_set) {
                            push(g.path(*o));
                        }
                    }
                }
                for shape in shaded_shapes(g) {
                    if t.lights(g, sh, shape) {
                        if want("transforms") {
                            push(g.path(crate::dag::transform_of(g, shape)));
                        }
                        if want("shapes") {
                            push(g.path(shape));
                        }
                    }
                }
            } else {
                // A set of lights: what's linked to it by name, and all objects if it's the default set.
                for (e, o) in &t.links {
                    if *e == l {
                        push(g.path(*o));
                    }
                }
            }
        } else if let Some(&o) = objects.first() {
            let shapes: Vec<NodeId> = if crate::dag::is_shape(g, o) { vec![o] } else { crate::dag::shapes(g, o, false) };
            let mut by_default = false;
            for light in li::all_shapes(g) {
                let tr = crate::dag::transform_of(g, light);
                if shapes.iter().any(|&sh| t.lights(g, light, sh)) {
                    by_default |= li::illuminates_by_default(g, tr);
                    push(g.path(tr));
                }
            }
            if by_default && want("sets") {
                push(li::DEFAULT_SET.to_string());
            }
        } else {
            fail!("lightlink: name a light or an object");
        }
        return Ok(json!(out));
    }
    if lights.is_empty() || objects.is_empty() {
        fail!("lightlink: give both -light and -object to make or break a link");
    }
    let (link, ignore) = if shadow { (LinkKind::ShadowLink, LinkKind::ShadowIgnore) } else { (LinkKind::Link, LinkKind::Ignore) };
    for &l in &lights {
        // A link is held on the light's transform, whichever was named.
        let l = li::shape_of(g, l).map_or(l, |sh| crate::dag::transform_of(g, sh));
        for &o in &objects {
            if breaking {
                remove_pair(g, link, l, o)?;
                add_pair(g, ignore, l, o)?;
            } else {
                remove_pair(g, ignore, l, o)?;
                add_pair(g, link, l, o)?;
            }
        }
    }
    Ok(Value::Null)
}

fn change_type(s: &mut Session, a: &Args) -> CmdResult {
    let (Some(n), Some(ty)) = (a.pos_str(0), a.pos_str(1)) else { fail!("name a light and a type") };
    if !li::is_light_type(ty) {
        fail!("{ty} is not a light type");
    }
    let g = &mut s.graph;
    let id = crate::rig::find(g, n, "aztekChangeLightType")?;
    let Some(old) = li::shape_of(g, id) else { fail!("{n} is not a light") };
    if g.node(old).unwrap().ty().name == ty {
        return Ok(json!(g.path(old)));
    }
    let tr = crate::dag::transform_of(g, old);
    let old_node = g.node(old).unwrap();
    let name = old_node.name().to_string();
    let t = old_node.type_arc().clone();
    let held: Vec<(String, PlugValue)> = t.attrs.iter().enumerate().filter(|(_, x)| x.has(flags::STORABLE) && x.has(flags::INPUT) && !x.is_array()).map(|(i, x)| (x.name.clone(), old_node.value(i).clone())).collect();
    let selected = g.selection().to_vec();
    g.delete_node(old)?;
    let new = g.create_node(&types::find(ty).unwrap(), &name, tr, false)?;
    for (attr, v) in held {
        if let Some(p) = attr_plug(g, new, &attr) {
            if std::mem::discriminant(&g.attr(&p).default_value) == std::mem::discriminant(&v) {
                let _ = g.set_value(p, v);
            }
        }
    }
    let keep: Vec<NodeId> = selected.into_iter().filter(|&x| g.node(x).is_some()).collect();
    g.select(&keep, SelectMode::Replace);
    Ok(json!(g.path(new)))
}

// ---------------------------------------------------------------------------------------------------------------------

pub fn register(c: &mut Commands<Session>) {
    use ArgType as T;
    register_types();
    for (cmd, ty, doc) in [
        ("directionalLight", "directionalLight", "Create a directional light, or query and edit one. Returns its shape."),
        ("pointLight", "pointLight", "Create a point light, or query and edit one. Returns its shape."),
        ("spotLight", "spotLight", "Create a spot light, or query and edit one. Returns its shape."),
        ("ambientLight", "ambientLight", "Create an ambient light, or query and edit one. Returns its shape."),
    ] {
        let t = ty.to_string();
        let mut d = crate::system_cmds::q_flags(Cmd::new(cmd, doc, move |s, a| light_command(s, a, &t)))
            .positional("the light, to edit or query")
            .flag(f("name", "n", T::String, "Its name."))
            .flag(f("rgb", "rgb", T::Vec3, "Colour."))
            .flag(f("shadowColor", "sc", T::Vec3, "Shadow colour."))
            .flag(f("position", "pos", T::Vec3, "Position."))
            .flag(f("rotation", "rot", T::Vec3, "Rotation."))
            .flag(f("exclusive", "exc", T::Bool, "Out of defaultLightSet: lights only what it's linked to."))
            .flag(f("shadowDither", "sd", T::Float, "Accepted for scripts."));
        for (flag, short, attr, kind) in table_for(ty) {
            d = d.flag(f(flag, short, kind.clone(), attr));
        }
        c.register(d);
    }
    for (cmd, ty) in [
        ("defaultDirectionalLight", "directionalLight"),
        ("defaultPointLight", "pointLight"),
        ("defaultSpotLight", "spotLight"),
        ("defaultAmbientLight", "ambientLight"),
        ("defaultAreaLight", "areaLight"),
        ("defaultVolumeLight", "volumeLight"),
    ] {
        let t = ty.to_string();
        c.register(Cmd::new(cmd, "Make a light as the Create menu does: intensity, then the colour. Returns its transform.", move |s, a| default_light(s, a, &t)).positional("intensity, r, g, b and the rest"));
    }
    for (cmd, ty) in [
        ("CreateDirectionalLight", "directionalLight"),
        ("CreatePointLight", "pointLight"),
        ("CreateSpotLight", "spotLight"),
        ("CreateAmbientLight", "ambientLight"),
        ("CreateAreaLight", "areaLight"),
        ("CreateVolumeLight", "volumeLight"),
    ] {
        let t = ty.to_string();
        let t2 = ty.to_string();
        let mut d = Cmd::new(cmd, "Make a light, selected: Create > Lights.", move |s, a| {
            let g = &mut s.graph;
            let (tr, sh) = make(g, &t2, &a.string("name", ""), NO_NODE)?;
            apply(g, a, &t2, tr, sh)?;
            g.select(&[tr], SelectMode::Replace);
            Ok(json!(g.path(tr)))
        })
        .flag(f("rgb", "rgb", T::Vec3, "Colour."));
        let _ = &t;
        for (flag, short, attr, kind) in table_for(ty) {
            d = d.flag(f(flag, short, kind.clone(), attr));
        }
        c.register(d);
    }
    extend(c, "shadingNode", vec![], |s, a, prev| {
        let Some(ty) = a.pos_str(0).filter(|t| li::is_light_type(t)).map(str::to_string) else { return prev(s, a) };
        if !a.bool("asLight") {
            fail!("shadingNode: {ty} is a light: give -asLight");
        }
        let g = &mut s.graph;
        let name = a.string("name", "");
        if a.bool("shared") && !name.is_empty() {
            if let Some(id) = g.find(&name)? {
                return Ok(json!(g.path(id)));
            }
        }
        let (tr, _) = make(g, &ty, &name, NO_NODE)?;
        if !a.bool("skipSelect") {
            g.select(&[tr], SelectMode::Replace);
        }
        Ok(json!(g.path(tr)))
    });
    c.register(
        Cmd::new("aztekChangeLightType", "The Attribute Editor's Type menu: the light's shape becomes another type under the same transform, with the same name, keeping what the two types share.", change_type)
            .positional("the light, then the type"),
    );
    c.register(
        Cmd::new("aztekIlluminatesByDefault", "The Attribute Editor's Illuminates by Default: whether a light is in defaultLightSet; -set puts it in or out.", |s, a| {
            let Some(n) = a.pos_str(0) else { fail!("name a light") };
            let g = &mut s.graph;
            let id = crate::rig::find(g, n, "aztekIlluminatesByDefault")?;
            if a.has("set") {
                li::set_illuminates_by_default(g, id, a.bool("set"))?;
            }
            Ok(json!(li::illuminates_by_default(g, id)))
        })
        .positional("the light")
        .flag(f("set", "set", T::Bool, "In or out.")),
    );
    c.register(
        Cmd::new("listLights", "The lights in the scene: their transforms, -s their shapes, -sl only the selected.", |s, a| {
            let g = &s.graph;
            let sel = g.selection();
            let mut out = Vec::new();
            for sh in li::all_shapes(g) {
                let tr = crate::dag::transform_of(g, sh);
                if a.bool("selection") && !sel.contains(&tr) && !sel.contains(&sh) {
                    continue;
                }
                out.push(g.path(if a.bool("shapes") { sh } else { tr }));
            }
            Ok(json!(out))
        })
        .flag(f("shapes", "s", T::Bool, "Their shapes."))
        .flag(f("selection", "sl", T::Bool, "Only the selected."))
        .undoable(false),
    );
    c.register(
        Cmd::new("lightlink", "Make, break and query which lights light which objects: -l lights -o objects, -m, -b, -q. With both and no mode it makes; with one it queries.", lightlink)
            .positional("lights and objects")
            .flag(f("b", "b", T::Bool, "Break the links."))
            .flag(f("make", "m", T::Bool, "Make the links."))
            .flag(f("query", "q", T::Bool, "Query."))
            .flag(f("light", "l", T::Any, "A light or set of lights.").multi(1))
            .flag(f("object", "o", T::Any, "An object or set of objects.").multi(1))
            .flag(f("hierarchy", "h", T::Bool, "Accepted: transforms above the first aren't listed."))
            .flag(f("sets", "set", T::Bool, "Query: include sets."))
            .flag(f("shapes", "shp", T::Bool, "Query: include shapes."))
            .flag(f("transforms", "t", T::Bool, "Query: include transforms."))
            .flag(f("shadow", "shd", T::Bool, "Link shadows."))
            .flag(f("useActiveLights", "ual", T::Bool, "Use the selected lights."))
            .flag(f("useActiveObjects", "uao", T::Bool, "Use the selected objects.")),
    );
}
