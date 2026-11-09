
use std::any::Any;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, OnceLock, RwLock};

use glam::{DMat4, DVec3};

use crate::error::{GraphError, Result};
use crate::plug::{flags, AttrDef, MeshPtr, NodeId, PlugValue, NO_NODE};

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct EvalContext {
    /// Seconds.
    pub time: f64,
}

pub type CreateFn = Arc<dyn Fn() -> Box<dyn NodeBehavior> + Send + Sync>;

#[derive(Clone, Default)]
pub struct NodeType {
    pub name: String,
    pub base: Option<Arc<NodeType>>,
    /// Lives in the transform hierarchy.
    pub dag: bool,
    /// Recomputed whenever the graph's time changes.
    pub time_dependent: bool,
    /// Extension that registered it; empty = built in.
    pub owner: String,
    /// What the node does and when to use it, one or two sentences.
    pub doc: String,
    /// Tabler icon name for the outliner and other node lists; empty inherits the base type's.
    pub icon: String,
    pub own_attrs: Vec<AttrDef>,
    pub create: Option<CreateFn>,
    /// Base attrs first, then own. Filled by `register_type`.
    pub attrs: Vec<AttrDef>,
    /// Set on a node's private copy of its type, which adds its user attrs after the type's own: the type it copies.
    pub user_of: Option<Arc<NodeType>>,
    /// Attr names older scene files may hold that no longer mean anything; loading skips them without a warning.
    pub retired_attrs: Vec<String>,
    /// Its outputs don't depend on where its parent is, so they may drive that parent: a follicle's outTranslate moves the
    /// transform above it.
    pub free_of_parent: bool,
    /// Its compute reads only its declared attrs, so one added to a node of this type orders nothing. DAG nodes are like this anyway.
    pub ignores_added_attrs: bool,
}

impl fmt::Debug for NodeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NodeType")
            .field("name", &self.name)
            .field("attrs", &self.attrs.len())
            .finish()
    }
}

impl NodeType {
    pub fn attr_index(&self, n: &str) -> Option<usize> {
        self.attrs
            .iter()
            .position(|a| a.name == n || a.short_name == n || a.aliases.iter().any(|x| x == n))
    }

    /// `attr_index`, falling back to the attrs' `file_alias`: for reading scene files. Also reads "<alias><n>", an
    /// array's numbered predecessor, returning n as the element.
    pub fn file_attr_index(&self, n: &str) -> Option<(usize, Option<i32>)> {
        if let Some(i) = self.attr_index(n) {
            return Some((i, None));
        }
        if let Some(i) = self
            .attrs
            .iter()
            .position(|a| !a.file_alias.is_empty() && a.file_alias == n)
        {
            return Some((i, None));
        }
        for (i, a) in self.attrs.iter().enumerate() {
            if !a.is_array() || a.file_alias.is_empty() {
                continue;
            }
            let (prefix, suffix) = match a.file_alias.find('#') {
                Some(h) => (&a.file_alias[..h], &a.file_alias[h + 1..]),
                None => (a.file_alias.as_str(), ""),
            };
            if n.len() <= prefix.len() + suffix.len()
                || !n.starts_with(prefix)
                || !n.ends_with(suffix)
            {
                continue;
            }
            let digits = &n[prefix.len()..n.len() - suffix.len()];
            if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) {
                if let Ok(k) = digits.parse::<i32>() {
                    return Some((i, Some(k)));
                }
            }
        }
        None
    }

    pub fn is_a(&self, type_name: &str) -> bool {
        let mut t = Some(self);
        while let Some(ty) = t {
            if ty.name == type_name {
                return true;
            }
            t = ty.base.as_deref();
        }
        false
    }

    /// The children of a grouped array ("group.child" attrs), in declaration order.
    pub fn group_children(&self, group: &str) -> Vec<usize> {
        self.attrs
            .iter()
            .enumerate()
            .filter(|(_, a)| {
                a.name.len() > group.len()
                    && a.name.starts_with(group)
                    && a.name.as_bytes()[group.len()] == b'.'
            })
            .map(|(i, _)| i)
            .collect()
    }

    /// True when this type or a base retired the attr name.
    pub fn is_retired(&self, name: &str) -> bool {
        let mut t = Some(self);
        while let Some(ty) = t {
            if ty.retired_attrs.iter().any(|a| a == name) {
                return true;
            }
            t = ty.base.as_deref();
        }
        false
    }
}

/// The registered type: this one, or the one a node's private copy (see `user_of`) was made from.
pub fn registered(t: &Arc<NodeType>) -> Arc<NodeType> {
    t.user_of.clone().unwrap_or_else(|| t.clone())
}

const IMPLICIT_MESSAGE_DOC: &str = "Links this node to others by connection; carries no value.";

/// The `message` every node type has.
pub fn message_attr() -> AttrDef {
    AttrDef {
        name: "message".into(),
        short_name: "msg".into(),
        ty: crate::plug::PlugType::Int,
        default_value: PlugValue::Int(0),
        flags: flags::OUTPUT | flags::HIDDEN | flags::MESSAGE,
        doc: IMPLICIT_MESSAGE_DOC.into(),
        ..Default::default()
    }
}

fn is_implicit_message(a: &AttrDef) -> bool {
    a.name == "message" && a.doc == IMPLICIT_MESSAGE_DOC
}

pub mod types {
    use super::*;

    #[derive(Default)]
    struct Registry {
        types: BTreeMap<String, Arc<NodeType>>,
        aliases: BTreeMap<String, String>,
    }

    fn registry() -> &'static RwLock<Registry> {
        static R: OnceLock<RwLock<Registry>> = OnceLock::new();
        R.get_or_init(Default::default)
    }

    /// Fills `attrs` from the base and own attrs and adds the type. Errors when the name is taken.
    pub fn register_type(mut ty: NodeType) -> Result<Arc<NodeType>> {
        let mut r = registry().write().unwrap();
        if r.types.contains_key(&ty.name) {
            return Err(GraphError::invalid(format!(
                "node type '{}' already registered",
                ty.name
            )));
        }
        ty.attrs = ty
            .base
            .as_ref()
            .map(|b| b.attrs.clone())
            .unwrap_or_default();
        // The base's message stays last: it isn't counted in the attr indices a type's own attrs follow on from.
        if ty.attrs.last().is_some_and(is_implicit_message) {
            ty.attrs.pop();
        }
        let base_count = ty.attrs.len();
        for own in ty.own_attrs.clone() {
            // An own attr named like a base one overrides it in place (a different default, say), keeping indices.
            match ty.attrs.iter().position(|a| a.name == own.name) {
                Some(i) if ty.base.is_some() && i < base_count => ty.attrs[i] = own,
                _ => ty.attrs.push(own),
            }
        }
        // Every node in the reference has a message plug, whatever its type declares.
        if !ty.attrs.iter().any(|a| a.name == "message") {
            ty.attrs.push(message_attr());
        }
        let type_name = ty.name.clone();
        for a in &mut ty.attrs {
            if a.unit == aztek_ops::units::Unit::None && matches!(a.ty, crate::plug::PlugType::Float | crate::plug::PlugType::Vec3) {
                a.unit = crate::unit_tables::attr_unit(&type_name, &a.name);
                // Types declare angles in degrees, as people read them; what's stored is radians.
                if a.unit == aztek_ops::units::Unit::Angle {
                    a.degrees_to_radians();
                }
            }
        }
        let ty = Arc::new(ty);
        r.types.insert(ty.name.clone(), ty.clone());
        Ok(ty)
    }

    /// For extensions: owned by `owner`. None, with a log message, if the name is taken.
    pub fn register_owned_type(owner: &str, mut ty: NodeType) -> Option<Arc<NodeType>> {
        ty.owner = owner.to_string();
        if ty.doc.is_empty() {
            log::warn!("extension {owner}: node type '{}' has no doc, so menus and tooltips can't say what it does", ty.name);
        }
        match register_type(ty) {
            Ok(t) => Some(t),
            Err(e) => {
                log::error!("extension {owner}: {e}");
                None
            }
        }
    }

    /// Drops a type from lookup. Nodes still holding it keep it alive, but it can no longer create nodes.
    pub fn unregister_type(name: &str) {
        registry().write().unwrap().types.remove(name);
    }

    /// Removes every type `owner` registered. Returns how many.
    pub fn remove_owned(owner: &str) -> usize {
        let mut r = registry().write().unwrap();
        let before = r.types.len();
        r.types.retain(|_, t| t.owner != owner);
        before - r.types.len()
    }

    pub fn find(name: &str) -> Option<Arc<NodeType>> {
        let r = registry().read().unwrap();
        if let Some(t) = r.types.get(name) {
            return Some(t.clone());
        }
        r.aliases.get(name).and_then(|n| r.types.get(n)).cloned()
    }

    /// Makes `find(old_name)` return the type named `ty`, for scene files that store a type under an older name.
    pub fn add_file_alias(old_name: &str, ty: &str) {
        registry()
            .write()
            .unwrap()
            .aliases
            .insert(old_name.to_string(), ty.to_string());
    }

    pub fn all() -> Vec<Arc<NodeType>> {
        registry().read().unwrap().types.values().cloned().collect()
    }

    /// The first icon set on the type or one of its bases, else fallback.
    pub fn icon_for<'a>(ty: &'a NodeType, fallback: &'a str) -> &'a str {
        let mut t = Some(ty);
        while let Some(x) = t {
            if !x.icon.is_empty() {
                return &x.icon;
            }
            t = x.base.as_deref();
        }
        fallback
    }

    /// The built-in types implemented so far: transform and locator. Meshes, modifiers, materials,
    /// rigs, deformers, cameras, lights, animation and the rest register from their own modules.
    pub fn register_builtins() {
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| {
            if find("transform").is_some() {
                return;
            }
            let dag = register_type(crate::transform::DagNode::make_type()).expect("dagNode");
            let shape = register_type(crate::transform::Shape::make_type(dag.clone())).expect("shape");
            register_type(crate::transform::Transform::make_type(dag)).expect("transform");
            register_type(crate::transform::Locator::make_type(shape)).expect("locator");
            crate::mesh_nodes::register_types();
            crate::modifiers::register_types();
            crate::object_sets::register_types();
            crate::display_layers::register_types();
            crate::booleans::register_types();
            crate::extrude_modifier::register_types();
            crate::split_rings_modifier::register_types();
            crate::auto_bevel::register_types();
            crate::utility_nodes::register_types();
            crate::shading::register_types();
            crate::lights::register_types();
            crate::harold_nodes::register_types();
            crate::expression::register_types();
        });
    }
}

/// Side data `compute` publishes outside the node's plugs, kept by cached playback along with its values.
pub struct PlaybackExtra {
    pub data: Option<Arc<dyn Any + Send + Sync>>,
    pub bytes: usize,
    /// False when what `compute` just produced is a placeholder that will be recomputed, never to be kept.
    pub keep: bool,
}

impl Default for PlaybackExtra {
    fn default() -> Self {
        Self {
            data: None,
            bytes: 0,
            keep: true,
        }
    }
}

pub mod traits {
    /// A ModifierNode.
    pub const MODIFIER: u32 = 1;
    /// A DeformerNode.
    pub const DEFORMER: u32 = 2;
}

/// A node's values. Kept apart from its behaviour so `compute` can borrow both.
#[derive(Clone)]
pub struct NodeData {
    pub(crate) ty: Arc<NodeType>,
    pub(crate) values: Vec<PlugValue>,
    /// Per attr, empty until an array gets an element.
    pub(crate) elements: Vec<Vec<(i32, PlugValue)>>,
}

impl NodeData {
    pub fn ty(&self) -> &NodeType {
        &self.ty
    }
    pub fn value(&self, attr: usize) -> &PlugValue {
        &self.values[attr]
    }
    pub fn bool(&self, attr: usize) -> bool {
        self.values[attr].as_bool()
    }
    pub fn int(&self, attr: usize) -> i64 {
        self.values[attr].as_int()
    }
    pub fn float(&self, attr: usize) -> f64 {
        self.values[attr].as_float()
    }
    pub fn vec3(&self, attr: usize) -> DVec3 {
        self.values[attr].as_vec3()
    }
    pub fn matrix(&self, attr: usize) -> DMat4 {
        self.values[attr].as_matrix()
    }
    pub fn string(&self, attr: usize) -> &str {
        self.values[attr].as_str()
    }
    pub fn mesh(&self, attr: usize) -> Option<&MeshPtr> {
        self.values[attr].as_mesh()
    }
    /// An array's elements (index, value), by ascending index: connected ones carry what was pulled.
    pub fn elements(&self, attr: usize) -> &[(i32, PlugValue)] {
        self.elements.get(attr).map(Vec::as_slice).unwrap_or(&[])
    }
    /// One element's value, or None when the array has no such element.
    pub fn element(&self, attr: usize, index: i32) -> Option<&PlugValue> {
        self.elements(attr)
            .iter()
            .find(|(i, _)| *i == index)
            .map(|(_, v)| v)
    }
    /// An element's value, or the attr's default when it has no such element.
    pub fn element_or(&self, attr: usize, index: i32) -> &PlugValue {
        self.element(attr, index)
            .unwrap_or(&self.ty.attrs[attr].default_value)
    }
    /// Writes an output.
    pub fn set_output(&mut self, attr: usize, v: impl Into<PlugValue>) {
        self.values[attr] = v.into();
    }
    /// Writes an array output's elements, by ascending index.
    pub fn set_output_elements(&mut self, attr: usize, els: Vec<(i32, PlugValue)>) {
        if self.elements.len() <= attr {
            self.elements.resize(attr + 1, Vec::new());
        }
        self.elements[attr] = els;
    }
}

/// What a node type does. `compute` runs on a worker thread and may only read this node's values (inputs are already
/// pulled from their sources) and write its outputs.
pub trait NodeBehavior: Send + Sync + 'static {
    fn compute(&mut self, _ctx: &EvalContext, _data: &mut NodeData) {}

    /// Called on a worker thread right after `compute`.
    fn playback_extra(&self, _ctx: &EvalContext, _data: &NodeData) -> PlaybackExtra {
        PlaybackExtra::default()
    }
    /// Republishes what `playback_extra` returned, in place of a `compute`. Called on a worker thread.
    fn restore_playback_extra(&mut self, _data: Option<&Arc<dyn Any + Send + Sync>>) {}

    /// Told the node's id once it is created.
    fn set_node_id(&mut self, _id: NodeId) {}

    /// Told the node's name just before `compute`.
    fn set_node_name(&mut self, _name: &str) {}

    /// Trait bits, so code can tell a node's kind without downcasting.
    fn traits(&self) -> u32 {
        0
    }

    /// Transforms (and joints, later) answer with their matrix terms; other nodes don't.
    fn as_transform(&self) -> Option<&dyn crate::transform::TransformBehavior> {
        None
    }

    /// Modifiers answer with their plug layout and state; other nodes don't.
    fn as_modifier(&self) -> Option<&dyn crate::modifiers::ModifierBehavior> {
        None
    }

    fn as_any(&self) -> &dyn Any;
}

/// A node with no behaviour of its own: computes nothing.
#[derive(Clone, Default)]
pub struct Plain;

impl NodeBehavior for Plain {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// A fresh identity in the reference's form: 8-4-4-4-12 upper case hex.
fn new_uuid() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    fn mix(mut x: u64) -> u64 {
        x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
        x ^ (x >> 31)
    }
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(0);
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    let (a, b) = (mix(now ^ count.wrapping_mul(0x9e3779b97f4a7c15)), mix(count ^ now.rotate_left(32) ^ std::process::id() as u64));
    format!("{:08X}-{:04X}-{:04X}-{:04X}-{:012X}", a >> 32, (a >> 16) & 0xffff, a & 0xffff, b >> 48, b & 0xffff_ffff_ffff)
}

/// A DG node. Values live here; connections and hierarchy live in `Graph`.
pub struct Node {
    pub(crate) id: NodeId,
    pub(crate) name: String,
    pub(crate) namespace: String,
    pub(crate) uuid: String,
    /// setAttr -k / -cb on one plug: (attr, component) -> (keyable, channel box).
    pub(crate) channels: BTreeMap<(i32, i32), (bool, bool)>,
    /// aliasAttr: alias -> attribute.
    pub(crate) aliases: BTreeMap<String, String>,
    pub(crate) reference: u32,
    /// lockNode: a locked node can't be deleted or renamed.
    pub(crate) locked: bool,
    pub(crate) data: NodeData,
    /// True once `data.ty` is this node's private copy holding user attrs.
    pub(crate) own_type: bool,
    /// Per attr, empty until something is locked.
    pub(crate) locks: Vec<u8>,
    pub(crate) dirty: bool,
    /// In the pass it last evaluated in, with cached playback on: 0 restored, 1 computed and kept, 2 computed, not kept.
    pub(crate) playback: u8,
    pub(crate) evaluated_pass: u64,
    pub(crate) behavior: Box<dyn NodeBehavior>,
}

impl fmt::Debug for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Node")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("type", &self.data.ty.name)
            .finish()
    }
}

impl Node {
    pub(crate) fn new(ty: Arc<NodeType>, behavior: Box<dyn NodeBehavior>) -> Self {
        Self {
            id: NO_NODE,
            name: String::new(),
            namespace: String::new(),
            uuid: new_uuid(),
            channels: BTreeMap::new(),
            aliases: BTreeMap::new(),
            reference: 0,
            locked: false,
            data: NodeData {
                ty,
                values: Vec::new(),
                elements: Vec::new(),
            },
            own_type: false,
            locks: Vec::new(),
            dirty: true,
            playback: 0,
            evaluated_pass: 0,
            behavior,
        }
    }

    pub fn id(&self) -> NodeId {
        self.id
    }
    pub fn ty(&self) -> &NodeType {
        &self.data.ty
    }
    pub fn type_arc(&self) -> &Arc<NodeType> {
        &self.data.ty
    }
    /// The short name, unique among siblings. Use `Graph::path` to address the node.
    pub fn name(&self) -> &str {
        &self.name
    }
    /// "" for the root namespace.
    pub fn namespace(&self) -> &str {
        &self.namespace
    }
    /// aliasAttr's aliases: alias -> attribute.
    pub fn aliases(&self) -> &BTreeMap<String, String> {
        &self.aliases
    }
    /// The node's identity, as `ls -uuid` gives it.
    pub fn uuid(&self) -> &str {
        &self.uuid
    }
    /// Reference this node was loaded from; 0 for nodes owned by the scene.
    pub fn is_locked(&self) -> bool {
        self.locked
    }
    pub fn reference_id(&self) -> u32 {
        self.reference
    }
    pub fn data(&self) -> &NodeData {
        &self.data
    }
    pub fn value(&self, attr: usize) -> &PlugValue {
        &self.data.values[attr]
    }
    pub fn bool(&self, attr: usize) -> bool {
        self.data.bool(attr)
    }
    pub fn int(&self, attr: usize) -> i64 {
        self.data.int(attr)
    }
    pub fn float(&self, attr: usize) -> f64 {
        self.data.float(attr)
    }
    pub fn vec3(&self, attr: usize) -> DVec3 {
        self.data.vec3(attr)
    }
    pub fn matrix(&self, attr: usize) -> DMat4 {
        self.data.matrix(attr)
    }
    pub fn elements(&self, attr: usize) -> &[(i32, PlugValue)] {
        self.data.elements(attr)
    }
    pub fn element(&self, attr: usize, index: i32) -> Option<&PlugValue> {
        self.data.element(attr, index)
    }
    /// Locked parts of an attr: bit 0 for a scalar, bits 0-2 for a Vec3's components.
    pub fn lock_mask(&self, attr: usize) -> u8 {
        self.locks.get(attr).copied().unwrap_or(0)
    }
    /// `Graph::evaluation_pass()` when this node last computed; 0 before it ever has.
    pub fn evaluated_pass(&self) -> u64 {
        self.evaluated_pass
    }
    pub fn behavior(&self) -> &dyn NodeBehavior {
        self.behavior.as_ref()
    }
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }
}

/// Makes a Vec3 attr with child names `<name>X/Y/Z` and short names `<short>x/y/z`.
pub fn vec3_attr(name: &str, short: &str, default: DVec3, flags: u32, doc: &str) -> AttrDef {
    let mut a = AttrDef {
        name: name.into(),
        short_name: short.into(),
        ty: crate::plug::PlugType::Vec3,
        default_value: PlugValue::Vec3(default),
        flags,
        doc: doc.into(),
        ..Default::default()
    };
    for (i, (up, lo)) in [('X', 'x'), ('Y', 'y'), ('Z', 'z')].into_iter().enumerate() {
        a.child_names[i] = format!("{name}{up}");
        a.child_short[i] = format!("{short}{lo}");
    }
    a
}

/// Shorthand used by type definitions.
pub const IN_STORE: u32 = flags::INPUT | flags::STORABLE;
