//! Node types defined by a script plugin. The scene knows nothing of the language: computing calls a hook the host sets.

use std::any::Any;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;
use std::sync::{Arc, RwLock};

use glam::{DMat4, DVec3};
use serde_json::Value;

use crate::error::Result;
use crate::node::{types, vec3_attr, EvalContext, NodeBehavior, NodeData, NodeType};
use crate::plug::{flags, AttrDef, NodeId, PlugType, PlugValue, NO_NODE};

/// Called with the node's type name, its id, its name, the time in seconds and its values (inputs pulled); writes its
/// outputs. May run on a worker thread.
pub type ComputeHook = Arc<dyn Fn(&str, NodeId, &str, f64, &mut NodeData) + Send + Sync>;

static HOOK: RwLock<Option<ComputeHook>> = RwLock::new(None);

pub fn set_compute_hook(hook: ComputeHook) {
    *HOOK.write().unwrap() = Some(hook);
}

struct ScriptedNode {
    type_name: Arc<str>,
    id: NodeId,
    name: String,
}

impl NodeBehavior for ScriptedNode {
    fn compute(&mut self, ctx: &EvalContext, data: &mut NodeData) {
        let hook = HOOK.read().unwrap().clone();
        if let Some(h) = hook {
            h(&self.type_name, self.id, &self.name, ctx.time, data);
        }
    }
    fn set_node_id(&mut self, id: NodeId) {
        self.id = id;
    }
    fn set_node_name(&mut self, name: &str) {
        if self.name != name {
            self.name = name.to_string();
        }
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn numbers(v: &Value) -> Vec<f64> {
    v.as_array().map(|a| a.iter().filter_map(Value::as_f64).collect()).unwrap_or_default()
}

/// A plug value of the type from JSON: a number, a string, [x, y, z] or 16 numbers row by row.
pub fn value_from_json(ty: PlugType, v: &Value) -> Option<PlugValue> {
    Some(match ty {
        PlugType::Bool => PlugValue::Bool(v.as_bool().or_else(|| v.as_f64().map(|f| f != 0.0))?),
        PlugType::Int | PlugType::Enum => PlugValue::Int(v.as_i64().or_else(|| v.as_f64().map(|f| f as i64))?),
        PlugType::Float => PlugValue::Float(v.as_f64()?),
        PlugType::String => PlugValue::String(v.as_str()?.to_string()),
        PlugType::Vec3 => {
            let n = numbers(v);
            if n.len() != 3 {
                return None;
            }
            PlugValue::Vec3(DVec3::new(n[0], n[1], n[2]))
        }
        PlugType::Matrix => {
            let n: [f64; 16] = numbers(v).try_into().ok()?;
            PlugValue::Matrix(DMat4::from_cols_array(&n))
        }
        PlugType::Mesh | PlugType::Data => return None,
    })
}

/// The JSON of a plain plug value; null for meshes and data.
pub fn value_to_json(v: &PlugValue) -> Value {
    match v {
        PlugValue::Bool(b) => Value::from(*b),
        PlugValue::Int(i) => Value::from(*i),
        PlugValue::Float(f) => Value::from(*f),
        PlugValue::String(s) => Value::from(s.clone()),
        PlugValue::Vec3(x) => Value::from(vec![x.x, x.y, x.z]),
        PlugValue::Matrix(m) => Value::from(m.to_cols_array().to_vec()),
        PlugValue::Mesh(_) | PlugValue::Data(_) => Value::Null,
    }
}

/// An attr from {name, short, type, default, input, keyable, storable, hidden, array, color, min, max, softMin, softMax,
/// enum, nice, nest}: type is bool, int, float, vec3, matrix, string, enum or mesh.
pub fn attr_from_json(j: &Value) -> AttrDef {
    let text = |k: &str| j.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let on = |k: &str, d: bool| j.get(k).and_then(Value::as_bool).unwrap_or(d);
    let num = |k: &str| j.get(k).and_then(Value::as_f64);
    let (name, short) = (text("name"), text("short"));
    let input = on("input", true);
    let mut fl = if input { flags::INPUT } else { flags::OUTPUT };
    if on("storable", input) {
        fl |= flags::STORABLE;
    }
    if on("keyable", false) {
        fl |= flags::KEYABLE;
    }
    if on("hidden", false) {
        fl |= flags::HIDDEN;
    }
    if on("array", false) {
        fl |= flags::ARRAY;
    }
    if on("color", false) {
        fl |= flags::COLOR;
    }
    let default = j.get("default").cloned().unwrap_or(Value::Null);
    let mut a = match text("type").as_str() {
        "vec3" => {
            let n = numbers(&default);
            let d = if n.len() == 3 { DVec3::new(n[0], n[1], n[2]) } else { DVec3::ZERO };
            let mut a = vec3_attr(&name, &short, d, fl, "");
            if let Some(kids) = j.get("children").and_then(Value::as_array) {
                for (i, k) in kids.iter().take(3).enumerate() {
                    a.child_names[i] = k.get("name").and_then(Value::as_str).unwrap_or("").to_string();
                    a.child_short[i] = k.get("short").and_then(Value::as_str).unwrap_or("").to_string();
                }
            }
            a
        }
        kind if crate::array_data::file_kind(kind).is_some() => {
            crate::array_data::register();
            AttrDef {
                name: name.clone(),
                short_name: short.clone(),
                ty: PlugType::Data,
                default_value: crate::array_data::ArrayData::empty(crate::array_data::file_kind(kind).unwrap()).value(),
                flags: fl,
                ..Default::default()
            }
        }
        other => {
            let (ty, fallback) = match other {
                "bool" => (PlugType::Bool, PlugValue::Bool(false)),
                "int" => (PlugType::Int, PlugValue::Int(0)),
                "enum" => (PlugType::Enum, PlugValue::Int(0)),
                "string" => (PlugType::String, PlugValue::String(String::new())),
                "matrix" => (PlugType::Matrix, PlugValue::Matrix(DMat4::IDENTITY)),
                "mesh" => (PlugType::Mesh, PlugValue::Mesh(None)),
                "curve" => (PlugType::Data, PlugValue::Data(None)),
                // A plug-in's own data type: its values flow as that type's objects.
                "data" => (PlugType::Data, PlugValue::Data(None)),
                _ => (PlugType::Float, PlugValue::Float(0.0)),
            };
            AttrDef {
                name: name.clone(),
                short_name: short.clone(),
                ty,
                default_value: value_from_json(ty, &default).unwrap_or(fallback),
                flags: fl,
                ..Default::default()
            }
        }
    };
    a.unit = match text("unit").as_str() {
        "angle" => aztek_ops::units::Unit::Angle,
        "distance" => aztek_ops::units::Unit::Linear,
        _ => aztek_ops::units::Unit::None,
    };
    a.min_value = num("min");
    a.max_value = num("max");
    a.soft_min = num("softMin");
    a.soft_max = num("softMax");
    a.nice_name = text("nice");
    if text("type") == "data" && !text("dataType").is_empty() {
        a.category = format!("{PLUGIN_DATA}{}", text("dataType"));
    }
    // Arrays inside arrays: which parts of the name are arrays (see AttrDef::nest).
    a.nest = j.get("nest").and_then(Value::as_u64).unwrap_or(0) as u8;
    a.enum_names = j.get("enum").and_then(Value::as_array).map(|e| e.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default();
    a
}

/// Adds a dependency node type whose attrs are described by JSON (see `attr_from_json`). Errors when the name is taken.
pub fn register_type(name: &str, owner: &str, attrs: &[Value]) -> Result<()> {
    let type_name: Arc<str> = Arc::from(name);
    types::register_type(NodeType {
        name: name.to_string(),
        owner: owner.to_string(),
        doc: format!("A dependency node from the plug-in {owner}."),
        own_attrs: attrs.iter().map(attr_from_json).collect(),
        create: Some(Arc::new(move || Box::new(ScriptedNode { type_name: type_name.clone(), id: NO_NODE, name: String::new() }))),
        ..Default::default()
    })?;
    TYPE_OWNERS.write().unwrap().insert(name.to_string(), owner.to_string());
    Ok(())
}

pub fn deregister_type(name: &str) {
    types::unregister_type(name);
    DEFORMERS.write().unwrap().remove(name);
    TYPE_OWNERS.write().unwrap().remove(name);
}

/// What runs the postConstructors of compiled plug-ins' nodes just made, set by the host: at the end of a command and
/// as the file reader goes, when the scene can be reached.
static POST_CREATE: OnceLock<fn(&mut crate::Session)> = OnceLock::new();

pub fn set_post_create_hook(f: fn(&mut crate::Session)) {
    let _ = POST_CREATE.set(f);
}

pub fn run_post_create(s: &mut crate::Session) {
    if let Some(f) = POST_CREATE.get() {
        f(s);
    }
}

/// `run_post_create` for a node made with a name: the name back if its postConstructor gave it another, as the
/// reference's createNode names a node after its postConstructor has run (mGearWeightDriver's sets its own).
pub fn post_create_named(s: &mut crate::Session, id: crate::plug::NodeId) {
    let Some(given) = s.graph.node(id).map(|n| n.name().to_string()) else { return };
    run_post_create(s);
    if s.graph.node(id).is_some_and(|n| n.name() != given) {
        let _ = s.graph.rename(id, &given);
    }
}

/// Plug-in deformer types.
static DEFORMERS: RwLock<BTreeSet<String>> = RwLock::new(BTreeSet::new());
/// Every plug-in node type and the plug-in it came from, for the file's requires lines.
static TYPE_OWNERS: RwLock<BTreeMap<String, String>> = RwLock::new(BTreeMap::new());
/// Plug-in versions, as they say in their requires lines.
static VERSIONS: RwLock<BTreeMap<String, String>> = RwLock::new(BTreeMap::new());

/// What a compiled plug-in's node type is built on.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NativeKind {
    Depend,
    /// On geometryFilter.
    Deformer,
    /// On locator: a shape in the DAG.
    Locator,
    /// On transform: a transform whose matrix the plug-in may make.
    Transform,
    /// On transform: a constraint, in the DAG under what it drives.
    Constraint,
    /// On shape: geometry of the plug-in's own, with its bounding box.
    Shape,
    /// An IK solver: a handle names it with ikHandle -solver.
    IkSolver,
}

/// The category an attribute of a plug-in data type has: this, then the type's name.
pub const PLUGIN_DATA: &str = "pluginData:";

static SHAPES: RwLock<BTreeSet<String>> = RwLock::new(BTreeSet::new());
static IK_SOLVERS: RwLock<BTreeSet<String>> = RwLock::new(BTreeSet::new());

/// A compiled plug-in's shape type: its bounds are its own.
pub fn is_plugin_shape(name: &str) -> bool {
    SHAPES.read().unwrap().contains(name)
}

/// A compiled plug-in's IK solver type.
pub fn is_plugin_ik_solver(name: &str) -> bool {
    IK_SOLVERS.read().unwrap().contains(name)
}

/// A plug-in data value as a scene file has it: the type's name and the text after -type.
pub fn plugin_data_text(v: &PlugValue) -> Option<(String, String)> {
    let PlugValue::Data(Some(d)) = v else { return None };
    let j = d.to_json();
    if j.get("pluginData").and_then(Value::as_bool) != Some(true) {
        return None;
    }
    Some((d.kind().to_string(), j.get("text").and_then(Value::as_str).unwrap_or("").to_string()))
}

/// Adds a node type from a compiled plug-in, whose behaviour the host supplies: a deformer type goes on geometryFilter,
/// a locator on locator.
pub fn register_native_type(name: &str, owner: &str, attrs: &[Value], kind: NativeKind, create: crate::node::CreateFn) -> Result<()> {
    let deformer = kind == NativeKind::Deformer;
    let base = match kind {
        NativeKind::Deformer => types::find("geometryFilter"),
        NativeKind::Locator => types::find("locator"),
        NativeKind::Transform | NativeKind::Constraint => types::find("transform"),
        NativeKind::Shape => types::find("shape"),
        NativeKind::Depend | NativeKind::IkSolver => None,
    };
    let dag = matches!(kind, NativeKind::Locator | NativeKind::Transform | NativeKind::Constraint | NativeKind::Shape);
    types::register_type(NodeType {
        name: name.to_string(),
        owner: owner.to_string(),
        dag,
        icon: if dag { "crosshair".into() } else { String::new() },
        base,
        doc: format!("A node from the plug-in {owner}."),
        own_attrs: {
            let mut own: Vec<AttrDef> = attrs.iter().map(attr_from_json).collect();
            if deformer {
                // A plug-in's deformer on a curve: the curve it starts from is kept on the node, and what it makes
                // of it goes into the shape.
                own.push(crate::curves::curve_attr("inputCurve", "icv", flags::INPUT | flags::STORABLE | flags::HIDDEN, "The curve the deformer starts from, when it deforms a curve."));
                own.push(crate::curves::curve_attr("outputCurve", "ocv", flags::OUTPUT | flags::HIDDEN, "The deformed curve."));
            }
            own
        },
        create: Some(create),
        // A constraint reads what it drives through its connections, not its parent's matrix, as the reference's do.
        free_of_parent: kind == NativeKind::Constraint,
        ..Default::default()
    })?;
    TYPE_OWNERS.write().unwrap().insert(name.to_string(), owner.to_string());
    if deformer {
        DEFORMERS.write().unwrap().insert(name.to_string());
    }
    if kind == NativeKind::Shape {
        SHAPES.write().unwrap().insert(name.to_string());
    }
    if kind == NativeKind::IkSolver {
        IK_SOLVERS.write().unwrap().insert(name.to_string());
    }
    Ok(())
}

pub fn is_plugin_deformer(name: &str) -> bool {
    DEFORMERS.read().unwrap().contains(name)
}

/// The plug-in a node type came from.
pub fn plugin_of_type(name: &str) -> Option<String> {
    TYPE_OWNERS.read().unwrap().get(name).cloned()
}

pub fn set_plugin_version(owner: &str, version: &str) {
    VERSIONS.write().unwrap().insert(owner.to_string(), version.to_string());
}

pub fn plugin_version(owner: &str) -> String {
    VERSIONS.read().unwrap().get(owner).cloned().unwrap_or_else(|| "Unknown".to_string())
}

/// A deformer's per-point weights: none means 1 everywhere.
pub fn deformer_weights(d: &NodeData) -> Option<&[f64]> {
    crate::deform::floats_of(d, crate::deform::WEIGHTS).map(|f| f.0.as_slice())
}

