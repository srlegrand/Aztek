//! Builds one layer's specs directly, then writes them as usda, usdc or usdz. Going to `sdf::Data` rather than through a
//! stage costs no composition per attribute, which matters for large exports.

use std::collections::{BTreeMap, HashMap};

use openusd::sdf::{self, ChildrenKey, FieldKey, ListOp, SpecType, Specifier, Value, Variability};
use openusd::tf::Token;

pub fn p(s: &str) -> Result<sdf::Path, String> {
    sdf::Path::new(s).map_err(|e| format!("bad USD path '{s}': {e}"))
}

/// One attribute's value over the export: a default, samples, or both empty for a declaration only.
#[derive(Clone, Default)]
pub struct Track {
    pub samples: Vec<(f64, Value)>,
    pub default: Option<Value>,
}

impl Track {
    /// Samples all alike become the default, as the reference exporter writes static values.
    pub fn settle(&mut self) {
        if self.samples.len() >= 1 && self.samples.iter().all(|(_, v)| *v == self.samples[0].1) {
            self.default = Some(self.samples[0].1.clone());
            self.samples.clear();
        }
    }
}

#[derive(Default)]
pub struct Out {
    pub data: sdf::Data,
    prim_children: BTreeMap<String, Vec<Token>>,
    props: HashMap<String, Vec<Token>>,
    made: HashMap<String, ()>,
}

impl Out {
    pub fn new() -> Out {
        let mut o = Out::default();
        o.data.create_spec(sdf::Path::abs_root(), SpecType::PseudoRoot);
        o
    }

    pub fn layer_meta(&mut self, key: impl AsRef<str>, v: impl Into<Value>) {
        if let Some(s) = self.data.spec_mut(&sdf::Path::abs_root()) {
            s.add(key, v);
        }
    }

    pub fn has_prim(&self, path: &str) -> bool {
        self.made.contains_key(path)
    }

    /// Defines a prim; its parents must exist already. Defining one again changes its type only.
    pub fn prim(&mut self, path: &str, spec: Specifier, ty: &str) -> Result<(), String> {
        let sp = p(path)?;
        if self.made.contains_key(path) {
            if !ty.is_empty() {
                if let Some(s) = self.data.spec_mut(&sp) {
                    s.add(FieldKey::TypeName, Value::Token(ty.into()));
                }
            }
            return Ok(());
        }
        let (parent, name) = path.rsplit_once('/').ok_or_else(|| format!("bad prim path {path}"))?;
        let parent = if parent.is_empty() { "/" } else { parent };
        self.prim_children.entry(parent.to_string()).or_default().push(name.into());
        self.made.insert(path.to_string(), ());
        let s = self.data.create_spec(sp, SpecType::Prim);
        s.add(FieldKey::Specifier, Value::Specifier(spec));
        if !ty.is_empty() {
            s.add(FieldKey::TypeName, Value::Token(ty.into()));
        }
        Ok(())
    }

    pub fn prim_field(&mut self, path: &str, key: impl AsRef<str>, v: impl Into<Value>) -> Result<(), String> {
        let sp = p(path)?;
        if let Some(s) = self.data.spec_mut(&sp) {
            s.add(key, v);
        }
        Ok(())
    }

    /// Adds to the prim's applied API schemas.
    pub fn apply_api(&mut self, path: &str, api: &str) -> Result<(), String> {
        let sp = p(path)?;
        if let Some(s) = self.data.spec_mut(&sp) {
            let mut items: Vec<Token> = Vec::new();
            for (k, v) in s.fields.iter() {
                if k == "apiSchemas" {
                    if let Value::TokenListOp(l) = v {
                        items = l.prepended_items.clone();
                    }
                }
            }
            if !items.iter().any(|t| t.as_str() == api) {
                items.push(api.into());
            }
            s.add(FieldKey::ApiSchemas, Value::TokenListOp(ListOp::prepended(items)));
        }
        Ok(())
    }

    fn property(&mut self, prim: &str, name: &str, kind: SpecType) -> Result<sdf::Path, String> {
        let pp = p(prim)?.append_property(name).map_err(|e| e.to_string())?;
        let list = self.props.entry(prim.to_string()).or_default();
        if !list.iter().any(|t| t.as_str() == name) {
            list.push(name.into());
        }
        self.data.create_spec(pp.clone(), kind);
        Ok(pp)
    }

    /// An attribute with a default.
    pub fn attr(&mut self, prim: &str, name: &str, ty: &str, v: impl Into<Value>) -> Result<sdf::Path, String> {
        let t = Track { default: Some(v.into()), samples: Vec::new() };
        self.attr_track(prim, name, ty, &t)
    }

    pub fn uniform(&mut self, prim: &str, name: &str, ty: &str, v: impl Into<Value>) -> Result<sdf::Path, String> {
        let pp = self.attr(prim, name, ty, v)?;
        if let Some(s) = self.data.spec_mut(&pp) {
            s.add(FieldKey::Variability, Value::Variability(Variability::Uniform));
        }
        Ok(pp)
    }

    pub fn attr_track(&mut self, prim: &str, name: &str, ty: &str, t: &Track) -> Result<sdf::Path, String> {
        let pp = self.property(prim, name, SpecType::Attribute)?;
        let s = self.data.spec_mut(&pp).expect("just made");
        s.add(FieldKey::TypeName, Value::Token(ty.into()));
        s.add(FieldKey::Custom, Value::Bool(false));
        s.add(FieldKey::Variability, Value::Variability(Variability::Varying));
        if let Some(d) = &t.default {
            s.add(FieldKey::Default, d.clone());
        }
        if !t.samples.is_empty() {
            s.add(FieldKey::TimeSamples, Value::TimeSamples(sdf::normalize_time_samples(t.samples.clone())));
        }
        Ok(pp)
    }

    /// A field on a property: interpolation, elementSize, custom, colorSpace.
    pub fn attr_field(&mut self, attr: &sdf::Path, key: impl AsRef<str>, v: impl Into<Value>) {
        if let Some(s) = self.data.spec_mut(attr) {
            s.add(key, v);
        }
    }

    /// A primvar with interpolation, indices when given and an element size above one.
    #[allow(clippy::too_many_arguments)]
    pub fn primvar(&mut self, prim: &str, name: &str, ty: &str, values: &Track, interp: &str, indices: Option<Vec<i32>>, elements: i32) -> Result<(), String> {
        let full = format!("primvars:{name}");
        let pp = self.attr_track(prim, &full, ty, values)?;
        self.attr_field(&pp, "interpolation", Value::Token(interp.into()));
        if elements > 1 {
            self.attr_field(&pp, "elementSize", Value::Int(elements));
        }
        if let Some(ix) = indices {
            self.attr(prim, &format!("{full}:indices"), "int[]", Value::IntVec(ix))?;
        }
        Ok(())
    }

    pub fn rel(&mut self, prim: &str, name: &str, targets: &[String]) -> Result<(), String> {
        let pp = self.property(prim, name, SpecType::Relationship)?;
        let mut paths = Vec::with_capacity(targets.len());
        for t in targets {
            paths.push(p(t)?);
        }
        let s = self.data.spec_mut(&pp).expect("just made");
        s.add(FieldKey::Custom, Value::Bool(false));
        s.add(FieldKey::Variability, Value::Variability(Variability::Uniform));
        s.add(FieldKey::TargetPaths, Value::PathListOp(ListOp::explicit(paths.clone())));
        s.add(ChildrenKey::RelationshipTargetChildren, Value::PathVec(paths));
        Ok(())
    }

    /// A connection on an attribute: `inputs:x.connect = <target>`.
    pub fn connect(&mut self, prim: &str, name: &str, ty: &str, target: &str) -> Result<(), String> {
        let pp = self.attr_track(prim, name, ty, &Track::default())?;
        let tp = p(target)?;
        let s = self.data.spec_mut(&pp).expect("just made");
        s.add(FieldKey::ConnectionPaths, Value::PathListOp(ListOp::explicit(vec![tp.clone()])));
        s.add(ChildrenKey::ConnectionChildren, Value::PathVec(vec![tp]));
        Ok(())
    }

    /// Writes the child lists every spec needs. Call once, before writing.
    pub fn finish(&mut self) -> Result<(), String> {
        for (path, kids) in std::mem::take(&mut self.prim_children) {
            let sp = p(&path)?;
            if let Some(s) = self.data.spec_mut(&sp) {
                s.add(ChildrenKey::PrimChildren, Value::TokenVec(kids));
            }
        }
        for (path, names) in std::mem::take(&mut self.props) {
            let sp = p(&path)?;
            if let Some(s) = self.data.spec_mut(&sp) {
                s.add(ChildrenKey::PropertyChildren, Value::TokenVec(names));
            }
        }
        Ok(())
    }
}

/// The extension a file is written with, `-defaultUSDFormat` deciding for `.usd`.
pub fn resolve_format(path: &str, default_format: &str) -> &'static str {
    let ext = std::path::Path::new(path).extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "usda" => "usda",
        "usdc" => "usdc",
        "usdz" => "usdz",
        _ => {
            if default_format == "usda" {
                "usda"
            } else {
                "usdc"
            }
        }
    }
}

/// Writes finished specs to `path`. A `.usdz` is packed from a temporary crate file beside it, with the textures it names.
pub fn write(data: &sdf::Data, path: &str, default_format: &str) -> Result<(), String> {
    match resolve_format(path, default_format) {
        "usda" => openusd::usda::TextWriter::write_to_file(data, path).map_err(|e| format!("{path}: {e}")),
        "usdc" => openusd::usdc::CrateWriter::write_to_file(data, path).map_err(|e| format!("{path}: {e}")),
        _ => {
            let temp = format!("{path}.root.usdc");
            openusd::usdc::CrateWriter::write_to_file(data, &temp).map_err(|e| format!("{temp}: {e}"))?;
            let result = (|| -> Result<(), String> {
                let stage = openusd::usd::Stage::open(&temp).map_err(|e| format!("{temp}: {e}"))?;
                openusd::usd_utils::create_new_usdz_package(&stage, &temp, path, None).map_err(|e| format!("{path}: {e}"))?;
                Ok(())
            })();
            let _ = std::fs::remove_file(&temp);
            result
        }
    }
}
