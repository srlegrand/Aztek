//! Short AttrDef builders the node type files share.

use glam::{DMat4, DVec3};

use crate::node::vec3_attr as node_vec3_attr;
use crate::plug::{flags, AttrDef, PlugType, PlugValue};

pub const STORED_FLAGS: u32 = flags::INPUT | flags::STORABLE;

/// A keyable float.
pub fn float_attr(name: &str, def: f64, doc: &str, lo: Option<f64>, hi: Option<f64>) -> AttrDef {
    AttrDef {
        name: name.into(),
        ty: PlugType::Float,
        default_value: PlugValue::Float(def),
        flags: STORED_FLAGS | flags::KEYABLE,
        doc: doc.into(),
        min_value: lo,
        max_value: hi,
        ..Default::default()
    }
}

/// A keyable float with a short name.
pub fn float_attr_short(name: &str, short: &str, def: f64, doc: &str, lo: Option<f64>, hi: Option<f64>) -> AttrDef {
    AttrDef { short_name: short.into(), ..float_attr(name, def, doc, lo, hi) }
}

pub fn bool_attr(name: &str, def: bool, doc: &str) -> AttrDef {
    AttrDef {
        name: name.into(),
        ty: PlugType::Bool,
        default_value: PlugValue::Bool(def),
        flags: STORED_FLAGS,
        doc: doc.into(),
        ..Default::default()
    }
}

pub fn enum_attr(name: &str, names: &[&str], def: i64, doc: &str) -> AttrDef {
    AttrDef {
        name: name.into(),
        ty: PlugType::Enum,
        default_value: PlugValue::Int(def),
        flags: STORED_FLAGS,
        enum_names: names.iter().map(|s| s.to_string()).collect(),
        doc: doc.into(),
        ..Default::default()
    }
}

/// A vector with X, Y and Z children; short may be empty.
pub fn vec3_attr(name: &str, short: &str, def: DVec3, flags: u32, doc: &str) -> AttrDef {
    node_vec3_attr(name, short, def, flags, doc)
}

pub fn matrix_attr(name: &str, flags: u32, doc: &str) -> AttrDef {
    AttrDef {
        name: name.into(),
        ty: PlugType::Matrix,
        default_value: PlugValue::Matrix(DMat4::IDENTITY),
        flags,
        doc: doc.into(),
        ..Default::default()
    }
}

/// A keyable modifier parameter.
pub fn param(name: &str, ty: PlugType, def: impl Into<PlugValue>, doc: &str, lo: Option<f64>, hi: Option<f64>) -> AttrDef {
    AttrDef {
        name: name.into(),
        ty,
        default_value: def.into(),
        flags: STORED_FLAGS | flags::KEYABLE,
        doc: doc.into(),
        min_value: lo,
        max_value: hi,
        ..Default::default()
    }
}

/// A keyable modifier parameter with enum names.
pub fn param_enum(name: &str, def: i64, doc: &str, options: &[&str]) -> AttrDef {
    AttrDef { enum_names: options.iter().map(|s| s.to_string()).collect(), ..param(name, PlugType::Enum, def, doc, None, None) }
}

/// def, shown under category in the Attribute Editor.
pub fn in_category(mut def: AttrDef, category: &str) -> AttrDef {
    def.category = category.into();
    def
}

/// A mesh plug.
pub fn mesh_attr(name: &str, short: &str, flags: u32, doc: &str) -> AttrDef {
    AttrDef {
        name: name.into(),
        short_name: short.into(),
        ty: PlugType::Mesh,
        default_value: PlugValue::Mesh(None),
        flags,
        doc: doc.into(),
        ..Default::default()
    }
}
