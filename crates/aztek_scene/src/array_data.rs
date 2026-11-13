//! Array data held in one plug, as the reference's typed attributes hold it: a doubleArray, Int32Array, stringArray,
//! pointArray or vectorArray value (a plug-in's `MFnDoubleArrayData`, mGear's `controlPoseValues`). Not to be confused
//! with an array attribute, whose elements are plugs of their own.

use std::any::Any;
use std::sync::Arc;

use serde_json::{json, Value};

use crate::plug::{Data, PlugValue};

/// The kinds, by the names files give them after `-type`, with the attribute type a plug-in names them by.
pub const KINDS: &[(&str, &str)] = &[
    ("doubleArray", "doubleArray"),
    ("floatArray", "floatArray"),
    ("Int32Array", "intArray"),
    ("stringArray", "stringArray"),
    ("pointArray", "pointArray"),
    ("vectorArray", "vectorArray"),
];

#[derive(Debug, Clone, PartialEq)]
pub struct ArrayData {
    /// The file's name for the kind: doubleArray, Int32Array, stringArray, pointArray, vectorArray or floatArray.
    pub kind: String,
    /// Every number, flat: a point's four, a vector's three. Empty for strings.
    pub nums: Vec<f64>,
    pub strings: Vec<String>,
}

impl ArrayData {
    pub fn empty(kind: &str) -> Self {
        ArrayData { kind: kind.to_string(), nums: Vec::new(), strings: Vec::new() }
    }

    /// The numbers each item has.
    pub fn width(&self) -> usize {
        match self.kind.as_str() {
            "pointArray" => 4,
            "vectorArray" => 3,
            _ => 1,
        }
    }

    pub fn len(&self) -> usize {
        if self.kind == "stringArray" {
            self.strings.len()
        } else {
            self.nums.len() / self.width()
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The values as a JSON list: numbers flat, or strings.
    pub fn values_json(&self) -> Value {
        if self.kind == "stringArray" {
            json!(self.strings)
        } else if self.kind == "Int32Array" {
            json!(self.nums.iter().map(|x| *x as i64).collect::<Vec<_>>())
        } else {
            json!(self.nums)
        }
    }

    /// From a JSON list, as `values_json` gives it.
    pub fn with_values(&self, v: &Value) -> Self {
        let mut out = ArrayData::empty(&self.kind);
        for x in v.as_array().into_iter().flatten() {
            match x {
                Value::String(s) => out.strings.push(s.clone()),
                Value::Array(a) => out.nums.extend(a.iter().filter_map(Value::as_f64)),
                other => out.nums.extend(other.as_f64()),
            }
        }
        out
    }

    /// As getAttr gives it: a list of numbers or strings, or of tuples for points and vectors.
    pub fn get_json(&self) -> Value {
        let w = self.width();
        if w == 1 {
            return self.values_json();
        }
        Value::Array(self.nums.chunks(w).map(|c| json!(c)).collect())
    }

    pub fn value(self) -> PlugValue {
        PlugValue::Data(Some(Arc::new(self)))
    }
}

impl Data for ArrayData {
    fn kind(&self) -> &str {
        "arrayData"
    }
    fn to_json(&self) -> Value {
        json!({"kind": "arrayData", "type": self.kind, "values": self.values_json()})
    }
    fn bytes(&self) -> usize {
        self.nums.len() * 8 + self.strings.iter().map(|s| s.len() + 24).sum::<usize>() + 64
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// The array data a plug value holds.
pub fn of(v: &PlugValue) -> Option<&ArrayData> {
    match v {
        PlugValue::Data(Some(d)) => d.as_any().downcast_ref::<ArrayData>(),
        _ => None,
    }
}

/// The file's name for a kind a plug-in or `addAttr` names (intArray is Int32Array).
pub fn file_kind(name: &str) -> Option<&'static str> {
    KINDS.iter().find(|k| k.0 == name || k.1 == name).map(|k| k.0)
}

pub fn register() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        crate::plug::data::register_kind(
            "arrayData",
            Arc::new(|j| {
                let kind = j.get("type").and_then(Value::as_str).unwrap_or("doubleArray");
                Ok(Arc::new(ArrayData::empty(kind).with_values(j.get("values").unwrap_or(&Value::Null))) as Arc<dyn Data>)
            }),
        );
    });
}

/// From a file's setAttr values after `-type kind`: the count, then the items.
pub fn from_tokens(kind: &str, count_and_items: &[Value]) -> Option<ArrayData> {
    let mut out = ArrayData::empty(kind);
    let n = count_and_items.first()?.as_f64()? as usize;
    let rest = &count_and_items[1..];
    if kind == "stringArray" {
        out.strings = rest.iter().take(n).map(|v| v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string())).collect();
    } else {
        let w = out.width();
        out.nums = rest.iter().take(n * w).filter_map(Value::as_f64).collect();
    }
    Some(out)
}

/// As a file's setAttr writes it: `-type "doubleArray" 3 1 2 3`.
pub fn file_text(d: &ArrayData, num: impl Fn(f64) -> String, quote: impl Fn(&str) -> String) -> String {
    let mut out = format!("-type \"{}\" {}", d.kind, d.len());
    if d.kind == "stringArray" {
        for s in &d.strings {
            out.push(' ');
            out.push_str(&quote(s));
        }
    } else {
        for x in &d.nums {
            out.push(' ');
            out.push_str(&num(*x));
        }
    }
    out
}
