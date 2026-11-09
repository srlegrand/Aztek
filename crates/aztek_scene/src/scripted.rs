//! Node types defined by a script plugin. The scene knows nothing of the language: computing calls a hook the host sets.

use std::any::Any;
use std::collections::{BTreeMap, BTreeSet};
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
/// enum, nice}: type is bool, int, float, vec3, matrix, string, enum or mesh.
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
        other => {
            let (ty, fallback) = match other {
                "bool" => (PlugType::Bool, PlugValue::Bool(false)),
                "int" => (PlugType::Int, PlugValue::Int(0)),
                "enum" => (PlugType::Enum, PlugValue::Int(0)),
                "string" => (PlugType::String, PlugValue::String(String::new())),
                "matrix" => (PlugType::Matrix, PlugValue::Matrix(DMat4::IDENTITY)),
                "mesh" => (PlugType::Mesh, PlugValue::Mesh(None)),
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

/// Plug-in deformer types.
static DEFORMERS: RwLock<BTreeSet<String>> = RwLock::new(BTreeSet::new());
/// Every plug-in node type and the plug-in it came from, for the file's requires lines.
static TYPE_OWNERS: RwLock<BTreeMap<String, String>> = RwLock::new(BTreeMap::new());
/// Plug-in versions, as they say in their requires lines.
static VERSIONS: RwLock<BTreeMap<String, String>> = RwLock::new(BTreeMap::new());

/// Adds a node type from a compiled plug-in, whose behaviour the host supplies: a deformer type goes on geometryFilter.
pub fn register_native_type(name: &str, owner: &str, attrs: &[Value], deformer: bool, create: crate::node::CreateFn) -> Result<()> {
    let base = if deformer { types::find("geometryFilter") } else { None };
    types::register_type(NodeType {
        name: name.to_string(),
        owner: owner.to_string(),
        base,
        doc: format!("A node from the plug-in {owner}."),
        own_attrs: attrs.iter().map(attr_from_json).collect(),
        create: Some(create),
        ..Default::default()
    })?;
    TYPE_OWNERS.write().unwrap().insert(name.to_string(), owner.to_string());
    if deformer {
        DEFORMERS.write().unwrap().insert(name.to_string());
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

