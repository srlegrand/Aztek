//! Node types every scene of the reference may hold that are not about one feature: the unknown nodes that keep what a
//! file held for a plug-in that isn't here, `network`, and the grouping nodes deformers and sets are built from.
//!
//! - `unknown`, `unknownDag`, `unknownTransform`: a node whose type nobody here knows, read from a file. It keeps its
//!   type, the plug-in that made it, its setAttr lines and its connections, and is written back as it came. It computes
//!   nothing. `unknownTransform` is a transform (it has children), `unknownDag` a shape under one.
//! - `network`: an empty node joined to others by `affects` and `affectedBy`; rigs keep added attributes on it.
//! - `groupId`, `groupParts`: which part of a geometry a set or a deformer works on. Files are read through them, as
//!   before; these let scripts make them.

use std::any::Any;
use std::sync::Arc;

use aztek_ops::{fail, ArgType, Args, CommandDef, Commands, FlagDef};
use serde_json::{json, Value};

use crate::attr_defs::{mesh_attr, STORED_FLAGS};
use crate::node::{types, EvalContext, NodeBehavior, NodeData, NodeType};
use crate::plug::{flags, AttrDef, PlugType, PlugValue};
use crate::session::Session;
use crate::transform::Shape;

pub const UNKNOWN: &str = "unknown";
pub const UNKNOWN_DAG: &str = "unknownDag";
pub const UNKNOWN_TRANSFORM: &str = "unknownTransform";

/// The three kinds of unknown node.
pub fn is_unknown(ty: &str) -> bool {
    matches!(ty, UNKNOWN | UNKNOWN_DAG | UNKNOWN_TRANSFORM)
}

fn text(name: &str, short: &str, doc: &str) -> AttrDef {
    AttrDef { name: name.into(), short_name: short.into(), ty: PlugType::String, default_value: PlugValue::String(String::new()), flags: STORED_FLAGS | flags::HIDDEN, doc: doc.into(), ..Default::default() }
}

fn int(name: &str, short: &str, def: i64, fl: u32, doc: &str) -> AttrDef {
    AttrDef { name: name.into(), short_name: short.into(), ty: PlugType::Int, default_value: PlugValue::Int(def), flags: fl, doc: doc.into(), ..Default::default() }
}

fn message(name: &str, short: &str, fl: u32, doc: &str) -> AttrDef {
    AttrDef { name: name.into(), short_name: short.into(), ty: PlugType::Int, default_value: PlugValue::Int(0), flags: fl | flags::MESSAGE, doc: doc.into(), ..Default::default() }
}

/// What an unknown node keeps of the file. The same names as a held dynamics node's, so one writer serves both.
fn file_attrs() -> Vec<AttrDef> {
    vec![
        text("fileType", "fty", "Its type in the file: the type of a plug-in that isn't here."),
        text("fileAttributes", "fat", "Its setAttr lines, as the file had them."),
        text("fileConnections", "fcn", "Its connections, as the file had them; @ stands for the node itself."),
        text("filePlugin", "fpl", "The plug-in the file said its type comes from."),
        text("filePluginVersion", "fpv", "That plug-in's version, as the file had it."),
    ]
}

/// A node that computes nothing of its own.
#[derive(Default)]
struct Inert;

impl NodeBehavior for Inert {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// A shape that only follows its transform.
#[derive(Default)]
struct InertShape;

impl NodeBehavior for InertShape {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        Shape::compute_matrices(d);
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// groupParts: the geometry passes through; which components the group holds is kept for the set or deformer that
/// reads it.
#[derive(Default)]
struct GroupParts;

impl NodeBehavior for GroupParts {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        let ty = d.ty().clone();
        let (Some(i), Some(o)) = (ty.attr_index("inputGeometry"), ty.attr_index("outputGeometry")) else { return };
        let m = d.mesh(i).cloned();
        d.set_output(o, PlugValue::Mesh(m));
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub fn register_types() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let reg = |t: NodeType| {
            if types::find(&t.name).is_none() {
                let name = t.name.clone();
                types::register_type(t).expect(&name);
            }
        };
        reg(NodeType {
            name: UNKNOWN.into(),
            icon: "help".into(),
            doc: "A node from a file whose type isn't known here: a plug-in's that isn't loaded. Keeps what the file had and writes it back; computes nothing.".into(),
            own_attrs: file_attrs(),
            create: Some(Arc::new(|| Box::<Inert>::default())),
            ..Default::default()
        });
        if let Some(shape) = types::find("shape") {
            reg(NodeType {
                name: UNKNOWN_DAG.into(),
                base: Some(shape),
                dag: true,
                icon: "help".into(),
                doc: "A shape from a file whose type isn't known here. Keeps what the file had and writes it back; draws and computes nothing.".into(),
                own_attrs: file_attrs(),
                create: Some(Arc::new(|| Box::<InertShape>::default())),
                ..Default::default()
            });
        }
        if let Some(tt) = types::find("transform") {
            reg(NodeType {
                name: UNKNOWN_TRANSFORM.into(),
                base: Some(tt.clone()),
                dag: true,
                icon: "help".into(),
                doc: "A transform from a file whose type isn't known here. Moves its children as a transform does, keeps what else the file had and writes it back.".into(),
                own_attrs: file_attrs(),
                create: tt.create.clone(),
                ..Default::default()
            });
        }
        reg(NodeType {
            name: "network".into(),
            icon: "affiliate".into(),
            doc: "Joins nodes by connection: A.affects into B.affectedBy says A affects B. Rigs keep added attributes on one.".into(),
            own_attrs: vec![
                message("affects", "a", flags::OUTPUT | flags::INPUT, "Connect to another network's affectedBy: this one affects it."),
                message("affectedBy", "ab", flags::INPUT, "From another network's affects."),
            ],
            create: Some(Arc::new(|| Box::<Inert>::default())),
            ..Default::default()
        });
        reg(NodeType {
            name: "groupId".into(),
            icon: "hash".into(),
            doc: "The number that names a group of components, for a set or a deformer.".into(),
            own_attrs: vec![int("groupId", "id", 0, flags::INPUT | flags::OUTPUT | flags::STORABLE, "The group's number.")],
            create: Some(Arc::new(|| Box::<Inert>::default())),
            ..Default::default()
        });
        reg(NodeType {
            name: "groupParts".into(),
            icon: "components".into(),
            doc: "Marks components of the geometry passing through as a group, named by groupId.".into(),
            own_attrs: vec![
                mesh_attr("inputGeometry", "ig", flags::INPUT | flags::HIDDEN, "The geometry."),
                text("inputComponents", "ic", "The components added to the group, as a selection: vtx[0:7]."),
                text("inputRemoveComponent", "irc", "The components taken out of it."),
                mesh_attr("outputGeometry", "og", flags::OUTPUT | flags::HIDDEN, "The geometry, unchanged."),
                int("groupId", "gi", -1, flags::INPUT | flags::STORABLE, "The group's number, from a groupId node."),
            ],
            create: Some(Arc::new(|| Box::<GroupParts>::default())),
            ..Default::default()
        });
    });
}

fn asked(a: &Args, name: &str) -> bool {
    a.bool(name) || a.get("queried").and_then(Value::as_array).is_some_and(|q| q.iter().any(|x| x == name))
}

pub fn register(c: &mut Commands<Session>) {
    register_types();
    let b = |name: &str, short: &str, doc: &str| FlagDef::new(name, short, ArgType::Bool, Value::Null, doc);
    c.register(
        CommandDef::<Session>::new(
            "unknownNode",
            "About an unknown node, one a file held for a plug-in that isn't here. -q -realClassName: the type the file gave \
             it; -q -plugin: the plug-in that type comes from.",
            |s, a| {
                let Some(name) = a.pos_str(0) else { fail!("unknownNode: name an unknown node") };
                let g = &s.graph;
                let Some(id) = g.find(name)? else { fail!("unknownNode: no object named {name}") };
                let n = g.node(id).unwrap();
                if !is_unknown(&n.ty().name) {
                    fail!("unknownNode: {name} is not an unknown node");
                }
                let get = |attr: &str| n.ty().attr_index(attr).map(|i| n.value(i).as_str().to_string()).unwrap_or_default();
                if asked(&a, "plugin") {
                    return Ok(json!(get("filePlugin")));
                }
                Ok(json!(get("fileType")))
            },
        )
        .positional("the unknown node")
        .undoable(false)
        .flag(b("query", "q", "Query mode."))
        .flag(FlagDef::new("queried", "", ArgType::List, Value::Null, "Flags asked about in query mode."))
        .flag(b("realClassName", "rcn", "Query: the node's type in the file."))
        .flag(b("plugin", "p", "Query: the plug-in the file said that type comes from.")),
    );
}
