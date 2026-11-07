
use std::any::Any;
use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, OnceLock, RwLock};

use glam::{DMat4, DVec3};
use rayon::prelude::*;
use serde_json::{json, Value};

use crate::error::GraphError;

/// Meshes flow through the graph as shared immutable snapshots; an edit makes a new one.
pub type MeshPtr = Arc<aztek_geo::Mesh>;

/// Any other immutable payload a plug carries (skin weights, influence palettes), shared like meshes. A kind registered
/// with [`data::register_kind`] can be stored; `to_json` must write a "kind" key holding that name.
pub trait Data: Send + Sync + fmt::Debug + Any {
    fn kind(&self) -> &str;
    fn to_json(&self) -> Value;
    /// Approximate heap bytes, for the undo memory budget.
    fn bytes(&self) -> usize {
        256
    }
    fn as_any(&self) -> &dyn Any;
    /// to_json when the data refers to nodes and needs the graph to write them (object set members by name).
    fn to_json_in(&self, _g: &crate::graph::Graph) -> Value {
        self.to_json()
    }
}
pub type DataPtr = Arc<dyn Data>;

pub type NodeId = u64;
pub const NO_NODE: NodeId = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlugType {
    Bool,
    Int,
    Float,
    Vec3,
    Matrix,
    String,
    /// Stored as Int.
    Enum,
    Mesh,
    Data,
}

#[derive(Clone)]
pub enum PlugValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    Vec3(DVec3),
    Matrix(DMat4),
    String(String),
    Mesh(Option<MeshPtr>),
    Data(Option<DataPtr>),
}

impl fmt::Debug for PlugValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlugValue::Bool(v) => write!(f, "Bool({v})"),
            PlugValue::Int(v) => write!(f, "Int({v})"),
            PlugValue::Float(v) => write!(f, "Float({v})"),
            PlugValue::Vec3(v) => write!(f, "Vec3({v})"),
            PlugValue::Matrix(v) => write!(f, "Matrix({v})"),
            PlugValue::String(v) => write!(f, "String({v:?})"),
            PlugValue::Mesh(m) => write!(f, "Mesh({})", if m.is_some() { "..." } else { "null" }),
            PlugValue::Data(d) => write!(
                f,
                "Data({})",
                d.as_ref()
                    .map(|d| d.kind().to_string())
                    .unwrap_or("null".into())
            ),
        }
    }
}

/// Meshes and data compare by identity.
impl PartialEq for PlugValue {
    fn eq(&self, other: &Self) -> bool {
        use PlugValue::*;
        match (self, other) {
            (Bool(a), Bool(b)) => a == b,
            (Int(a), Int(b)) => a == b,
            (Float(a), Float(b)) => a == b,
            (Vec3(a), Vec3(b)) => a == b,
            (Matrix(a), Matrix(b)) => a == b,
            (String(a), String(b)) => a == b,
            (Mesh(a), Mesh(b)) => match (a, b) {
                (None, None) => true,
                (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                _ => false,
            },
            (Data(a), Data(b)) => match (a, b) {
                (None, None) => true,
                (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                _ => false,
            },
            _ => false,
        }
    }
}

impl From<bool> for PlugValue {
    fn from(v: bool) -> Self {
        PlugValue::Bool(v)
    }
}
impl From<i64> for PlugValue {
    fn from(v: i64) -> Self {
        PlugValue::Int(v)
    }
}
impl From<f64> for PlugValue {
    fn from(v: f64) -> Self {
        PlugValue::Float(v)
    }
}
impl From<DVec3> for PlugValue {
    fn from(v: DVec3) -> Self {
        PlugValue::Vec3(v)
    }
}
impl From<DMat4> for PlugValue {
    fn from(v: DMat4) -> Self {
        PlugValue::Matrix(v)
    }
}
impl From<&str> for PlugValue {
    fn from(v: &str) -> Self {
        PlugValue::String(v.to_string())
    }
}
impl From<String> for PlugValue {
    fn from(v: String) -> Self {
        PlugValue::String(v)
    }
}

impl PlugValue {
    pub fn as_bool(&self) -> bool {
        match self {
            PlugValue::Bool(v) => *v,
            other => panic!("expected bool, got {other:?}"),
        }
    }
    pub fn as_int(&self) -> i64 {
        match self {
            PlugValue::Int(v) => *v,
            other => panic!("expected int, got {other:?}"),
        }
    }
    pub fn as_float(&self) -> f64 {
        match self {
            PlugValue::Float(v) => *v,
            other => panic!("expected float, got {other:?}"),
        }
    }
    pub fn as_vec3(&self) -> DVec3 {
        match self {
            PlugValue::Vec3(v) => *v,
            other => panic!("expected vec3, got {other:?}"),
        }
    }
    pub fn as_matrix(&self) -> DMat4 {
        match self {
            PlugValue::Matrix(v) => *v,
            other => panic!("expected matrix, got {other:?}"),
        }
    }
    pub fn as_str(&self) -> &str {
        match self {
            PlugValue::String(v) => v,
            other => panic!("expected string, got {other:?}"),
        }
    }
    pub fn as_mesh(&self) -> Option<&MeshPtr> {
        match self {
            PlugValue::Mesh(v) => v.as_ref(),
            other => panic!("expected mesh, got {other:?}"),
        }
    }
    pub fn as_data(&self) -> Option<&DataPtr> {
        match self {
            PlugValue::Data(v) => v.as_ref(),
            other => panic!("expected data, got {other:?}"),
        }
    }

    /// A bool, int or float as a double.
    pub fn scalar(&self) -> Option<f64> {
        match self {
            PlugValue::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            PlugValue::Int(i) => Some(*i as f64),
            PlugValue::Float(f) => Some(*f),
            _ => None,
        }
    }

    /// Writes a double into a bool, int or float slot, converting; other kinds are left alone.
    pub fn assign_scalar(&mut self, s: f64) {
        match self {
            PlugValue::Bool(b) => *b = s != 0.0,
            PlugValue::Int(i) => *i = s as i64,
            PlugValue::Float(f) => *f = s,
            _ => {}
        }
    }

    pub fn holds(&self, ty: PlugType) -> bool {
        matches!(
            (ty, self),
            (PlugType::Bool, PlugValue::Bool(_))
                | (PlugType::Int | PlugType::Enum, PlugValue::Int(_))
                | (PlugType::Float, PlugValue::Float(_))
                | (PlugType::Vec3, PlugValue::Vec3(_))
                | (PlugType::Matrix, PlugValue::Matrix(_))
                | (PlugType::String, PlugValue::String(_))
                | (PlugType::Mesh, PlugValue::Mesh(_))
                | (PlugType::Data, PlugValue::Data(_))
        )
    }
}

pub mod flags {
    pub const INPUT: u32 = 1 << 0;
    pub const OUTPUT: u32 = 1 << 1;
    pub const KEYABLE: u32 = 1 << 2;
    pub const STORABLE: u32 = 1 << 3;
    pub const HIDDEN: u32 = 1 << 4;
    /// String holding a file path: editors offer a file browser.
    pub const FILE: u32 = 1 << 5;
    /// Only meaningful while driven: losing the connection puts the default back.
    pub const RESET_ON_DISCONNECT: u32 = 1 << 6;
    /// Vec3 holding a linear RGB colour: editors show a swatch.
    pub const COLOR: u32 = 1 << 7;
    /// An identity: duplicates don't inherit it.
    pub const UNIQUE: u32 = 1 << 8;
    /// Any number of elements (node.attr[3]); with STORABLE they hold values, else only connections.
    pub const ARRAY: u32 = 1 << 9;
    /// Added to one node by add_attr rather than declared by its type.
    pub const USER: u32 = 1 << 10;
    /// Shown in the channel box although not keyable.
    pub const CHANNEL_BOX: u32 = 1 << 11;
    /// String of several lines: editors offer a text box.
    pub const MULTILINE: u32 = 1 << 12;
    /// String naming a font: editors offer font pickers.
    pub const FONT: u32 = 1 << 13;
    /// String of source code: editors offer a monospace box that applies on Ctrl+Enter.
    pub const CODE: u32 = 1 << 14;
    /// With FILE, an image: editors offer thumbnails and the image library tied to the attr.
    pub const IMAGE: u32 = 1 << 15;
    /// A message plug: a link between nodes that carries no value, so it orders nothing and dirties nothing.
    pub const MESSAGE: u32 = 1 << 16;
}

#[derive(Clone, Debug)]
pub struct AttrDef {
    pub name: String,
    pub short_name: String,
    pub ty: PlugType,
    pub default_value: PlugValue,
    pub flags: u32,
    /// Enum only.
    pub enum_names: Vec<String>,
    /// Enum only: each name's own doc, by index.
    pub enum_docs: Vec<String>,
    /// Vec3: long names, e.g. translateX.
    pub child_names: [String; 3],
    /// Vec3: tx, ty, tz.
    pub child_short: [String; 3],
    pub doc: String,
    /// What a scene file means by leaving this attr out, when that differs from `default_value`.
    pub file_default: Option<PlugValue>,
    /// An older name scene files may store this attr under. On an array, `#` stands for the element number.
    pub file_alias: String,
    /// Other names getAttr, setAttr and commands reach this attr by (polyCube's subdivisionsX is subdivisionsWidth).
    pub aliases: Vec<String>,
    pub nice_name: String,
    pub min_value: Option<f64>,
    pub max_value: Option<f64>,
    pub soft_min: Option<f64>,
    pub soft_max: Option<f64>,
    pub step: Option<f64>,
    /// Groups attrs under a heading in editors; empty is the type's own section.
    pub category: String,
}

impl Default for AttrDef {
    fn default() -> Self {
        Self {
            name: String::new(),
            short_name: String::new(),
            ty: PlugType::Float,
            default_value: PlugValue::Float(0.0),
            flags: flags::INPUT | flags::STORABLE,
            enum_names: Vec::new(),
            enum_docs: Vec::new(),
            child_names: Default::default(),
            child_short: Default::default(),
            doc: String::new(),
            file_default: None,
            file_alias: String::new(),
            aliases: Vec::new(),
            nice_name: String::new(),
            min_value: None,
            max_value: None,
            soft_min: None,
            soft_max: None,
            step: None,
            category: String::new(),
        }
    }
}

impl AttrDef {
    /// The value scene files omit and assume when the attr is absent.
    pub fn stored_default(&self) -> &PlugValue {
        self.file_default.as_ref().unwrap_or(&self.default_value)
    }
    pub fn is_array(&self) -> bool {
        self.flags & flags::ARRAY != 0
    }
    pub fn has(&self, flag: u32) -> bool {
        self.flags & flag != 0
    }

    /// Holds a value inside the attr's hard limits; each component of a Vec3.
    pub fn clamp(&self, v: &mut PlugValue) {
        if self.min_value.is_none() && self.max_value.is_none() {
            return;
        }
        let lo = self.min_value.unwrap_or(f64::NEG_INFINITY);
        let hi = self.max_value.unwrap_or(f64::INFINITY);
        match v {
            PlugValue::Float(d) => *d = d.clamp(lo, hi),
            PlugValue::Int(i) => *i = (*i as f64).clamp(lo.ceil(), hi.floor()) as i64,
            PlugValue::Vec3(x) => *x = x.clamp(DVec3::splat(lo), DVec3::splat(hi)),
            _ => {}
        }
    }
}

/// node.attr, optionally one component of a Vec3 (node.tx) or one element of an array (node.attr[3]). An attr named
/// "group.child" is a child of a grouped array: its elements read as node.group[3].child.
///
/// Field order matters: ordering compares node, attr, component, element.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlugRef {
    pub node: NodeId,
    pub attr: i32,
    pub component: i32,
    pub element: i32,
}

impl Default for PlugRef {
    fn default() -> Self {
        Self {
            node: NO_NODE,
            attr: -1,
            component: -1,
            element: -1,
        }
    }
}

impl PlugRef {
    pub fn new(node: NodeId, attr: i32) -> Self {
        Self {
            node,
            attr,
            component: -1,
            element: -1,
        }
    }
    pub fn component(node: NodeId, attr: i32, component: i32) -> Self {
        Self {
            node,
            attr,
            component,
            element: -1,
        }
    }
    pub fn element(node: NodeId, attr: i32, element: i32) -> Self {
        Self {
            node,
            attr,
            component: -1,
            element,
        }
    }
    pub fn valid(&self) -> bool {
        self.node != NO_NODE && self.attr >= 0
    }
    /// The whole attr this plug is part of.
    pub fn whole(&self) -> Self {
        Self::new(self.node, self.attr)
    }
}

pub fn type_name(ty: PlugType) -> &'static str {
    match ty {
        PlugType::Bool => "bool",
        PlugType::Int => "int",
        PlugType::Float => "float",
        PlugType::Vec3 => "vec3",
        PlugType::Matrix => "matrix",
        PlugType::String => "string",
        PlugType::Enum => "enum",
        PlugType::Mesh => "mesh",
        PlugType::Data => "data",
    }
}

/// The type `type_name` names.
pub fn type_named(name: &str) -> Result<PlugType, GraphError> {
    use PlugType::*;
    for t in [Bool, Int, Float, Vec3, Matrix, String, Enum, Mesh, Data] {
        if name == type_name(t) {
            return Ok(t);
        }
    }
    Err(GraphError::invalid(format!(
        "unknown attribute type '{name}'"
    )))
}

pub fn default_for(ty: PlugType) -> PlugValue {
    match ty {
        PlugType::Bool => PlugValue::Bool(false),
        PlugType::Int | PlugType::Enum => PlugValue::Int(0),
        PlugType::Float => PlugValue::Float(0.0),
        PlugType::Vec3 => PlugValue::Vec3(DVec3::ZERO),
        PlugType::Matrix => PlugValue::Matrix(DMat4::IDENTITY),
        PlugType::String => PlugValue::String(String::new()),
        PlugType::Mesh => PlugValue::Mesh(None),
        PlugType::Data => PlugValue::Data(None),
    }
}

fn num(v: f64) -> Value {
    serde_json::Number::from_f64(v)
        .map(Value::Number)
        .unwrap_or(Value::Null)
}

pub fn to_json(v: &PlugValue) -> Value {
    match v {
        PlugValue::Bool(b) => Value::Bool(*b),
        PlugValue::Int(i) => json!(*i),
        PlugValue::Float(f) => num(*f),
        PlugValue::Vec3(x) => json!([num(x.x), num(x.y), num(x.z)]),
        PlugValue::Matrix(m) => {
            // Column-major, as glm stores it.
            let cols = m.to_cols_array();
            Value::Array(cols.iter().map(|&d| num(d)).collect())
        }
        PlugValue::String(s) => Value::String(s.clone()),
        PlugValue::Mesh(m) => m
            .as_ref()
            .map(|m| crate::mesh_data::mesh_to_json(m))
            .unwrap_or(Value::Null),
        PlugValue::Data(d) => d.as_ref().map(|d| d.to_json()).unwrap_or(Value::Null),
    }
}

fn f64_of(j: &Value) -> Option<f64> {
    j.as_f64()
}

/// Converts `j` to the attr's type.
pub fn from_json(def: &AttrDef, j: &Value) -> Result<PlugValue, GraphError> {
    match def.ty {
        PlugType::Bool => {
            if let Some(b) = j.as_bool() {
                return Ok(PlugValue::Bool(b));
            }
            if let Some(d) = f64_of(j) {
                return Ok(PlugValue::Bool(d != 0.0));
            }
        }
        PlugType::Int => {
            if let Some(i) = j.as_i64() {
                return Ok(PlugValue::Int(i));
            }
            if let Some(d) = f64_of(j) {
                return Ok(PlugValue::Int(d as i64));
            }
            if let Some(b) = j.as_bool() {
                return Ok(PlugValue::Int(b as i64));
            }
        }
        PlugType::Enum => {
            if let Some(i) = j.as_i64() {
                return Ok(PlugValue::Int(i));
            }
            if let Some(s) = j.as_str() {
                if let Some(i) = def.enum_names.iter().position(|n| n == s) {
                    return Ok(PlugValue::Int(i as i64));
                }
            }
        }
        PlugType::Float => {
            if let Some(d) = f64_of(j) {
                return Ok(PlugValue::Float(d));
            }
            if let Some(b) = j.as_bool() {
                return Ok(PlugValue::Float(if b { 1.0 } else { 0.0 }));
            }
        }
        PlugType::Vec3 => {
            if let Some(a) = j.as_array() {
                if a.len() == 3 {
                    if let (Some(x), Some(y), Some(z)) =
                        (f64_of(&a[0]), f64_of(&a[1]), f64_of(&a[2]))
                    {
                        return Ok(PlugValue::Vec3(DVec3::new(x, y, z)));
                    }
                }
            }
        }
        PlugType::Matrix => {
            if let Some(a) = j.as_array() {
                if a.len() == 16 {
                    let mut m = [0.0; 16];
                    let mut ok = true;
                    for (i, v) in a.iter().enumerate() {
                        match f64_of(v) {
                            Some(d) => m[i] = d,
                            None => ok = false,
                        }
                    }
                    if ok {
                        return Ok(PlugValue::Matrix(DMat4::from_cols_array(&m)));
                    }
                }
            }
        }
        PlugType::String => {
            if let Some(s) = j.as_str() {
                return Ok(PlugValue::String(s.to_string()));
            }
        }
        PlugType::Mesh => {
            if j.is_null() {
                return Ok(PlugValue::Mesh(None));
            }
            if j.is_object() {
                return crate::mesh_data::mesh_from_json(j).map(|m| PlugValue::Mesh(Some(m)));
            }
        }
        PlugType::Data => {
            if j.is_null() {
                return Ok(PlugValue::Data(None));
            }
            if let Some(kind) = j.get("kind").and_then(Value::as_str) {
                let loader = data::find(kind)
                    .ok_or_else(|| GraphError::invalid(format!("unknown data kind '{kind}'")))?;
                return loader(j).map(|d| PlugValue::Data(Some(d)));
            }
            // A bare value for a Data attr whose default names its kind: that kind's loader gets it as {"kind", "value"}.
            if let PlugValue::Data(Some(d)) = &def.default_value {
                if let Some(loader) = data::find(d.kind()) {
                    return loader(&json!({"kind": d.kind(), "value": j}))
                        .map(|d| PlugValue::Data(Some(d)));
                }
            }
        }
    }
    Err(GraphError::invalid(format!(
        "expected {}, got {}",
        type_name(def.ty),
        j
    )))
}

/// `to_json` of each value, in order; meshes and data convert in parallel. `packed` writes meshes in their packed form.
pub fn to_json_all(values: &[&PlugValue], packed: bool) -> Vec<Value> {
    values
        .par_iter()
        .map(|v| match v {
            // The packed form belongs to the binary scene file, not implemented yet; plain JSON holds the same data.
            other => {
                let _ = packed;
                to_json(other)
            }
        })
        .collect()
}

pub struct FromJsonResult {
    pub value: Option<PlugValue>,
    /// `from_json`'s message when it failed; value is then None.
    pub error: String,
}

/// `from_json` of each (attr, json), in order; meshes decode in parallel.
pub fn from_json_all(items: &[(&AttrDef, &Value)]) -> Vec<FromJsonResult> {
    items
        .par_iter()
        .map(|(def, j)| match from_json(def, j) {
            Ok(v) => FromJsonResult {
                value: Some(v),
                error: String::new(),
            },
            Err(e) => FromJsonResult {
                value: None,
                error: e.to_string(),
            },
        })
        .collect()
}

/// A user attr's definition as scene files store it: name, type, flags and the editor hints set on it.
pub fn attr_def_to_json(d: &AttrDef) -> Value {
    let mut j = serde_json::Map::new();
    j.insert("name".into(), json!(d.name));
    j.insert("type".into(), json!(type_name(d.ty)));
    if !d.short_name.is_empty() {
        j.insert("short".into(), json!(d.short_name));
    }
    for (flag, key) in [
        (flags::ARRAY, "array"),
        (flags::KEYABLE, "keyable"),
        (flags::HIDDEN, "hidden"),
        (flags::COLOR, "color"),
        (flags::UNIQUE, "unique"),
        (flags::FILE, "file"),
        (flags::CHANNEL_BOX, "channelBox"),
        (flags::OUTPUT, "output"),
    ] {
        if d.flags & flag != 0 {
            j.insert(key.into(), json!(true));
        }
    }
    if d.default_value != default_for(d.ty) {
        j.insert("default".into(), to_json(&d.default_value));
    }
    if !d.enum_names.is_empty() {
        j.insert("enum".into(), json!(d.enum_names));
    }
    if d.ty == PlugType::Vec3 {
        j.insert("children".into(), json!(d.child_names));
    }
    for (key, v) in [
        ("min", d.min_value),
        ("max", d.max_value),
        ("softMin", d.soft_min),
        ("softMax", d.soft_max),
    ] {
        if let Some(v) = v {
            j.insert(key.into(), num(v));
        }
    }
    if !d.nice_name.is_empty() {
        j.insert("niceName".into(), json!(d.nice_name));
    }
    if !d.doc.is_empty() {
        j.insert("doc".into(), json!(d.doc));
    }
    if !d.category.is_empty() {
        j.insert("category".into(), json!(d.category));
    }
    Value::Object(j)
}

/// `attr_def_to_json` read back.
pub fn attr_def_from_json(j: &Value) -> Result<AttrDef, GraphError> {
    let s = |k: &str| j.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let b = |k: &str| j.get(k).and_then(Value::as_bool).unwrap_or(false);
    let name = j
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| GraphError::invalid("attr def has no name"))?;
    let ty = type_named(
        j.get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| GraphError::invalid("attr def has no type"))?,
    )?;
    let mut d = AttrDef {
        name: name.to_string(),
        short_name: s("short"),
        ty,
        default_value: default_for(ty),
        flags: flags::INPUT | flags::STORABLE | flags::USER,
        ..Default::default()
    };
    for (flag, key) in [
        (flags::ARRAY, "array"),
        (flags::KEYABLE, "keyable"),
        (flags::HIDDEN, "hidden"),
        (flags::COLOR, "color"),
        (flags::UNIQUE, "unique"),
        (flags::FILE, "file"),
        (flags::CHANNEL_BOX, "channelBox"),
        (flags::OUTPUT, "output"),
    ] {
        if b(key) {
            d.flags |= flag;
        }
    }
    if let Some(e) = j.get("enum").and_then(Value::as_array) {
        d.enum_names = e
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect();
    }
    if let Some(c) = j.get("children").and_then(Value::as_array) {
        for (i, v) in c.iter().take(3).enumerate() {
            d.child_names[i] = v.as_str().unwrap_or("").to_string();
        }
    }
    if let Some(def) = j.get("default") {
        d.default_value = from_json(&d, def)?;
    }
    d.min_value = j.get("min").and_then(Value::as_f64);
    d.max_value = j.get("max").and_then(Value::as_f64);
    d.soft_min = j.get("softMin").and_then(Value::as_f64);
    d.soft_max = j.get("softMax").and_then(Value::as_f64);
    d.nice_name = s("niceName");
    d.doc = s("doc");
    d.category = s("category");
    Ok(d)
}

pub mod data {
    use super::*;

    pub type FromJson = Arc<dyn Fn(&Value) -> Result<DataPtr, GraphError> + Send + Sync>;

    struct Kind {
        load: FromJson,
        owner: String,
    }

    fn kinds() -> &'static RwLock<HashMap<String, Kind>> {
        static K: OnceLock<RwLock<HashMap<String, Kind>>> = OnceLock::new();
        K.get_or_init(Default::default)
    }

    /// Lets a Data kind load from scene files. Registering a kind again replaces its loader.
    pub fn register_kind(kind: &str, load: FromJson) {
        kinds().write().unwrap().insert(
            kind.to_string(),
            Kind {
                load,
                owner: String::new(),
            },
        );
    }

    /// For extensions: false, with a log message, if the kind is taken.
    pub fn register_owned_kind(owner: &str, kind: &str, load: FromJson) -> bool {
        let mut k = kinds().write().unwrap();
        if k.contains_key(kind) {
            log::error!("extension {owner}: data kind '{kind}' is already registered");
            return false;
        }
        k.insert(
            kind.to_string(),
            Kind {
                load,
                owner: owner.to_string(),
            },
        );
        true
    }

    /// Removes every kind `owner` registered. Returns how many.
    pub fn remove_owned(owner: &str) -> usize {
        let mut k = kinds().write().unwrap();
        let before = k.len();
        k.retain(|_, v| v.owner != owner);
        before - k.len()
    }

    pub fn find(kind: &str) -> Option<FromJson> {
        kinds().read().unwrap().get(kind).map(|k| k.load.clone())
    }
}
