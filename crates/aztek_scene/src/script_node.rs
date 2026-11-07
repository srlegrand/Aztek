//! The `script` node: scene-level scripts that travel with the file. The `scriptNode` command that makes and runs them
//! lives with the script engines in the host; the scene only stores them.

use std::any::Any;
use std::sync::Arc;

use crate::node::{types, EvalContext, NodeBehavior, NodeData, NodeType};
use crate::plug::{flags, AttrDef, PlugType, PlugValue};

pub const TYPE: &str = "script";

struct ScriptNode;

impl NodeBehavior for ScriptNode {
    fn compute(&mut self, _ctx: &EvalContext, _d: &mut NodeData) {}
    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub fn register_types() {
    // Sessions may start on several threads at once.
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(register_type);
}

fn register_type() {
    let attr = |name: &str, short: &str, ty: PlugType, v: PlugValue| AttrDef { name: name.into(), short_name: short.into(), ty, default_value: v, flags: flags::INPUT | flags::STORABLE, ..Default::default() };
    types::register_type(NodeType {
        name: TYPE.into(),
        doc: "Scripts kept in the scene: one to run before, one after, when the script type says.".into(),
        own_attrs: vec![
            AttrDef { flags: flags::INPUT | flags::STORABLE | flags::CODE, ..attr("before", "b", PlugType::String, PlugValue::String(String::new())) },
            AttrDef { flags: flags::INPUT | flags::STORABLE | flags::CODE, ..attr("after", "a", PlugType::String, PlugValue::String(String::new())) },
            // 0 demand, 1 open/close, 2 GUI open/close, 3 UI configuration, 4 software render, 5 software frame render,
            // 6 scene configuration, 7 time changed.
            attr("scriptType", "st", PlugType::Int, PlugValue::Int(0)),
            // 0 MEL, 1 Python.
            attr("sourceType", "stp", PlugType::Int, PlugValue::Int(0)),
        ],
        create: Some(Arc::new(|| Box::new(ScriptNode))),
        ..Default::default()
    })
    .expect(TYPE);
}
