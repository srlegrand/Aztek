//! Object duplication shared by duplicate, Duplicate with Transform, instance,
//! the gizmo's clone drag and the array tools.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock, RwLock};

use aztek_ops::{fail, objects, ArgType as T, Args, CmdResult, CommandDef, Commands, FlagDef};
use glam::{DMat3, DMat4, DVec3};
use serde_json::{json, Value};

use crate::components;
use crate::display_layers;
use crate::graph::{Change, Graph, SelectMode};
use crate::instancing;
use crate::node::types;
use crate::plug::{flags, NodeId, PlugRef, PlugType, PlugValue, NO_NODE};
use crate::repeat::{self, Step};
use crate::session::Session;
use crate::transform::Transform;
use crate::transform_ops::{self as xf, Space};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Mode {
    /// A mesh gets its own live copy of the stack.
    #[default]
    Copy,
    /// A mesh shares its source's data (instancing::connect_instance).
    Instance,
    /// A mesh shares its source's base and gets its own modifiers (instancing::connect_reference).
    Reference,
}

pub fn mode_name(m: Mode) -> &'static str {
    match m {
        Mode::Instance => "instance",
        Mode::Reference => "reference",
        Mode::Copy => "copy",
    }
}

/// "copy", "instance" or "reference".
pub fn parse_mode(s: &str) -> CmdResult<Mode> {
    match s {
        "copy" => Ok(Mode::Copy),
        "instance" => Ok(Mode::Instance),
        "reference" => Ok(Mode::Reference),
        _ => fail!("unknown duplicate mode '{s}' (copy, instance or reference)"),
    }
}

/// A source and its newest copy, as Duplicate with Transform repeats them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pair {
    pub source: NodeId,
    pub copy: NodeId,
    pub mode: Mode,
}

/// The pairs the last duplicate made, one per top-level object; dropped as soon as either node of any pair is deleted.
pub type Pairs = Arc<Mutex<Vec<Pair>>>;

/// Runs after a duplicate with every node it copied, source to copy, so a node type can rewire what copying values
/// alone misses (connections between copied nodes, a container to resync). Once per copy for a run of copies.
pub type Fixup = Arc<dyn Fn(&mut Graph, &HashMap<NodeId, NodeId>) + Send + Sync>;

fn fixups() -> &'static RwLock<Vec<Fixup>> {
    static F: OnceLock<RwLock<Vec<Fixup>>> = OnceLock::new();
    F.get_or_init(Default::default)
}

pub fn add_fixup(f: Fixup) {
    fixups().write().unwrap().push(f);
}

fn run_fixups(g: &mut Graph, copied: &HashMap<NodeId, NodeId>) {
    if copied.is_empty() {
        return;
    }
    let all = fixups().read().unwrap().clone();
    for f in all {
        f(g, copied);
    }
}

/// Drops the pairs when the scene is cleared or either node of one is deleted.
pub fn install(g: &mut Graph) {
    let pairs = g.duplicate_pairs.clone();
    g.add_listener(
        Arc::new(move |_g, c, id| match c {
            Change::Cleared => pairs.lock().unwrap().clear(),
            Change::Deleted => pairs.lock().unwrap().retain(|p| p.source != id && p.copy != id),
            _ => {}
        }),
        true,
    );
}

pub fn last_pairs(g: &Graph) -> Vec<Pair> {
    g.duplicate_pairs.lock().unwrap().clone()
}

pub fn set_last_pairs(g: &Graph, pairs: Vec<Pair>) {
    *g.duplicate_pairs.lock().unwrap() = pairs;
}

pub fn clear_last_pairs(g: &Graph) {
    g.duplicate_pairs.lock().unwrap().clear();
}

fn plug(id: NodeId, attr: usize) -> PlugRef {
    PlugRef::new(id, attr as i32)
}

/// Copies every storable input value that isn't driven by a connection, then the locks, as the reference's duplicate keeps
/// them.
pub fn copy_values(g: &mut Graph, from: NodeId, to: NodeId) -> CmdResult<()> {
    let ty = g.node(from).unwrap().type_arc().clone();
    for (i, def) in ty.attrs.iter().enumerate() {
        if def.flags & flags::INPUT == 0 || def.flags & flags::UNIQUE != 0 {
            continue;
        }
        if def.flags & flags::ARRAY != 0 {
            g.copy_elements(from, to, i as i32, false)?;
        } else if g.source(&plug(from, i)).is_none() {
            let v = g.node(from).unwrap().value(i).clone();
            g.set_value(plug(to, i), v)?;
        }
    }
    for (i, def) in ty.attrs.iter().enumerate() {
        if def.ty != PlugType::Vec3 {
            if g.is_locked(&plug(from, i)) {
                g.set_locked(&plug(to, i), true)?;
            }
            continue;
        }
        for c in 0..3 {
            let p = PlugRef { component: c, ..plug(from, i) };
            if g.is_locked(&p) {
                g.set_locked(&PlugRef { component: c, ..plug(to, i) }, true)?;
            }
        }
    }
    Ok(())
}

/// Driven inputs of a shape without a mesh stack (curves) take their current value, so the copy keeps its shape
/// without the history.
fn copy_evaluated_inputs(g: &mut Graph, from: NodeId, to: NodeId) -> CmdResult<()> {
    let ty = g.node(from).unwrap().type_arc().clone();
    for (i, def) in ty.attrs.iter().enumerate() {
        if def.flags & flags::INPUT != 0 && g.source(&plug(from, i)).is_some() {
            let v = g.value(&plug(from, i))?;
            g.set_value(plug(to, i), v)?;
        }
    }
    Ok(())
}

fn copy_node(
    g: &mut Graph,
    s: NodeId,
    p: NodeId,
    top: bool,
    mode: Mode,
    name: &str,
    copies: &mut HashMap<NodeId, NodeId>,
    record: &mut HashMap<NodeId, NodeId>,
) -> CmdResult<NodeId> {
    let n = g.node(s).unwrap();
    let ty = n.type_arc().clone();
    let named = top && !name.is_empty();
    // A shape's copy takes the next free shape name in the scene (pCubeShape2), not its source's under a new parent.
    let is_shape = crate::dag::is_shape(g, s);
    let copy_name = if named {
        name.to_string()
    } else if is_shape && mode != Mode::Instance {
        let source = n.name().to_string();
        g.next_scene_name(&source)
    } else {
        n.name().to_string()
    };
    let c = g.create_node(&ty, &copy_name, p, named)?;
    record.insert(s, c);
    copy_values(g, s, c)?;
    if let Some(layer) = display_layers::layer_of(g, s) {
        display_layers::add_members(g, layer, &[c])?;
    }
    let n = g.node(s).unwrap();
    if ty.is_a("mesh") && !ty.is_a("lattice") {
        match mode {
            Mode::Instance => instancing::connect_instance(g, s, c)?,
            Mode::Reference => instancing::connect_reference(g, s, c)?,
            Mode::Copy if instancing::instance_parent(g, s) != NO_NODE => {
                // A copy of an instance is a mesh of its own, not one more instance.
                let mut own = HashMap::new();
                let root = instancing::root(g, s);
                instancing::copy_history(g, root, c, &mut own)?;
            }
            // As the reference copies a deformed mesh: what it looks like now, with no deformers.
            Mode::Copy if crate::deform_sets::is_deformed(g, s) => {
                g.evaluate();
                let v = g.value(&PlugRef::new(s, crate::mesh_nodes::MeshNode::OUT_MESH as i32))?;
                g.set_value(PlugRef::new(c, crate::mesh_nodes::MeshNode::IN_MESH as i32), v)?;
            }
            Mode::Copy => instancing::copy_history(g, s, c, copies)?,
        }
    } else if ty.is_a("mesh") {
        instancing::copy_history(g, s, c, copies)?;
    } else if components::point_provider(n).is_some() {
        copy_evaluated_inputs(g, s, c)?;
    }
    for child in g.children(s).to_vec() {
        // A deformed mesh's original stays behind: the copy has no deformers for it to feed.
        if crate::deform_sets::is_original(g, child) {
            continue;
        }
        copy_node(g, child, c, false, mode, name, copies, record)?;
    }
    Ok(c)
}

/// Copies `src` and its children under `parent` and returns the copy. Meshes follow `mode`; other shapes copy their
/// values. `name` names the top copy (made unique), empty keeps the source's. `copies` maps history nodes already
/// copied in this operation, so shared upstream nodes are copied once.
pub fn copy_tree(
    g: &mut Graph,
    src: NodeId,
    parent: NodeId,
    mode: Mode,
    name: &str,
    copies: &mut HashMap<NodeId, NodeId>,
) -> CmdResult<NodeId> {
    let mut own = HashMap::new();
    let made = copy_node(g, src, parent, true, mode, name, copies, &mut own)?;
    run_fixups(g, &own);
    Ok(made)
}

fn world_of(g: &mut Graph, id: NodeId) -> CmdResult<DMat4> {
    match g.value(&plug(id, Transform::WORLD_MATRIX))? {
        PlugValue::Matrix(m) => Ok(m),
        _ => Ok(DMat4::IDENTITY),
    }
}

fn rotate_of(g: &Graph, id: NodeId) -> DVec3 {
    g.node(id).unwrap().vec3(Transform::ROTATE)
}

fn top_level(s: &Session, a: &Args, cmd: &str) -> CmdResult<Vec<NodeId>> {
    let mut ids = Vec::new();
    for n in objects(s, a) {
        let Some(id) = s.graph.find(&n)? else { fail!("{cmd}: no node named '{n}'") };
        ids.push(id);
    }
    Ok(xf::top_level(&s.graph, &ids))
}

fn vec_flag(a: &Args, flag: &str, fallback: f64) -> CmdResult<DVec3> {
    let Some(v) = a.get(flag) else { return Ok(DVec3::splat(fallback)) };
    if let Some(x) = v.as_f64() {
        return Ok(DVec3::splat(x));
    }
    match a.vec3(flag) {
        Some(v) => Ok(DVec3::from_array(v)),
        None => fail!("duplicate: {flag} takes three numbers"),
    }
}

fn paths_of(g: &Graph, ids: &[NodeId]) -> Value {
    Value::Array(ids.iter().map(|&id| Value::String(g.path(id))).collect())
}

struct Request {
    count: i32,
    mode: Mode,
    step: Step,
    pivot: String,
    pivot_point: DVec3,
    space: String,
    group: bool,
    name: String,
}

impl Default for Request {
    fn default() -> Self {
        Self {
            count: 1,
            mode: Mode::Copy,
            step: Step::default(),
            pivot: "object".into(),
            pivot_point: DVec3::ZERO,
            space: "world".into(),
            group: false,
            name: String::new(),
        }
    }
}

/// The copies of every root, in order, with the pairs Duplicate with Transform repeats.
fn duplicate_roots(g: &mut Graph, roots: &[NodeId], r: &Request) -> CmdResult<Vec<NodeId>> {
    g.evaluate();
    // One map per repetition: history shared by several roots is copied once per copy, not shared by every copy.
    let count = r.count as usize;
    let mut copies: Vec<HashMap<NodeId, NodeId>> = vec![HashMap::new(); count];
    let mut copied: Vec<HashMap<NodeId, NodeId>> = vec![HashMap::new(); count];
    let mut made = Vec::new();
    let mut recorded = Vec::new();
    for &root in roots {
        let base = world_of(g, root)?;
        let pivot = match r.pivot.as_str() {
            "world" => DVec3::ZERO,
            "point" => r.pivot_point,
            _ => xf::world_pivot(g, root),
        };
        let frame = if r.space == "object" { xf::frame(g, root, Space::Object) } else { DMat3::IDENTITY };
        let placed = repeat::copy_matrices(&base, &repeat::step_matrix(&r.step, pivot, &frame), r.count);
        let (mut prev, mut last) = (root, root);
        for i in 0..count {
            let parent = g.parent(root);
            let c = copy_node(g, root, parent, true, r.mode, &r.name, &mut copies[i], &mut copied[i])?;
            if !r.step.identity() {
                let hint = rotate_of(g, root) + r.step.rotate * (i + 1) as f64;
                repeat::set_world_matrix(g, c, &placed[i], Some(hint))?;
            }
            made.push(c);
            prev = last;
            last = c;
        }
        recorded.push(Pair { source: prev, copy: last, mode: r.mode });
    }
    for c in &copied {
        run_fixups(g, c);
    }
    if r.group {
        let mut parent = g.parent(roots[0]);
        for &root in roots {
            if g.parent(root) != parent {
                parent = NO_NODE;
            }
        }
        let ty = types::find("transform").expect("transform");
        let grp = g.create_node(&ty, "group1", parent, true)?;
        for &c in &made {
            g.set_parent(c, grp, true, usize::MAX)?;
        }
    }
    set_last_pairs(g, recorded);
    Ok(made)
}

fn flag(name: &str, short: &str, ty: T, default: Value, doc: &str) -> FlagDef {
    FlagDef::new(name, short, ty, default, doc)
}

pub fn register(c: &mut Commands<Session>) {
    c.register(
        CommandDef::<Session>::new(
            "duplicate",
            "Duplicate objects with their children, keeping each mesh's generator and modifier stack live. Returns the new \
             top-level names.\n\ncount makes a run of copies, copy i placed by the translate/rotate/scale step applied i \
             times about the pivot: a row, a ring or a spiral in one call. mode instance shares the mesh (edits to one \
             change all); reference shares the base mesh but gives each copy its own modifier stack.",
            |s, a| {
                let roots = top_level(s, a, "duplicate")?;
                if roots.is_empty() {
                    fail!("duplicate: nothing to duplicate");
                }
                let mut r = Request {
                    count: a.int("count", 1).clamp(1, 10000) as i32,
                    mode: parse_mode(&a.string("mode", "copy"))?,
                    step: Step {
                        translate: vec_flag(a, "translate", 0.0)?,
                        rotate: vec_flag(a, "rotate", 0.0)?,
                        scale: vec_flag(a, "scale", 1.0)?,
                    },
                    pivot: a.string("pivot", "object"),
                    pivot_point: vec_flag(a, "pivotPoint", 0.0)?,
                    space: a.string("space", "world"),
                    group: a.bool("group"),
                    name: String::new(),
                };
                if !["object", "world", "point"].contains(&r.pivot.as_str()) {
                    fail!("duplicate: pivot is object, world or point");
                }
                if r.space != "world" && r.space != "object" {
                    fail!("duplicate: space is world or object");
                }
                if a.has("name") && roots.len() == 1 {
                    r.name = a.string("name", "");
                }
                let made = duplicate_roots(&mut s.graph, &roots, &r)?;
                s.graph.select(&made, SelectMode::Replace);
                Ok(paths_of(&s.graph, &made))
            },
        )
        .positional("objects")
        .flag(flag("name", "n", T::String, Value::Null, "Name for a single duplicate."))
        .flag(flag("count", "c", T::Int, json!(1), "Copies of each object, 1 to 10000.").min(1.0).max(10000.0))
        .flag(flag("translate", "t", T::Vec3, json!([0.0, 0.0, 0.0]), "Move from one copy to the next, cm."))
        .flag(flag("rotate", "r", T::Vec3, json!([0.0, 0.0, 0.0]), "Rotation from one copy to the next about the pivot, degrees (XYZ)."))
        .flag(flag("scale", "s", T::Vec3, json!([1.0, 1.0, 1.0]), "Scale from one copy to the next about the pivot."))
        .flag(flag("pivot", "", T::String, json!("object"), "What the step rotates and scales about.").choices(&["object", "world", "point"]))
        .flag(flag("pivotPoint", "", T::Vec3, json!([0.0, 0.0, 0.0]), "World point rotated and scaled about when pivot is point, cm."))
        .flag(
            flag("space", "", T::String, json!("world"), "Axes the step is measured along: the world's or each object's own.")
                .choices(&["world", "object"]),
        )
        .flag(
            flag(
                "mode",
                "m",
                T::String,
                json!("copy"),
                "What a duplicated mesh shares with its source: copy shares nothing. instance shares the mesh and its whole \
                 modifier stack: every copy looks the same and exports as one mesh. reference shares only the base mesh and \
                 keeps a modifier stack of its own (same shape, a different bevel). Component edits on a shared mesh reach \
                 every copy.",
            )
            .choices(&["copy", "instance", "reference"]),
        )
        .flag(flag("group", "g", T::Bool, json!(false), "Put the copies under a new group."))
        .examples(&[
            "duplicate(\"Pillar\", count=7, translate=[200, 0, 0])",
            "duplicate(\"Spoke\", count=11, rotate=[0, 30, 0], pivot=\"world\", mode=\"instance\")",
            "duplicate(\"Step\", count=20, translate=[0, 18, 0], rotate=[0, 15, 0], pivot=\"point\", pivotPoint=[0, 0, 0])",
        ]),
    );
    c.set_adjustable("duplicate", true);

    c.register(
        CommandDef::<Session>::new(
            "duplicateWithTransform",
            "Duplicate the last duplicated object again, placed by how its copy now sits relative to its source. Duplicate \
             an object, move, rotate or scale the copy, then repeat this to continue the run. Returns the new names.\n\n\
             The step is measured from the current world matrices of the last source and copy, so it follows any later \
             edit to the copy and survives selection changes. It is forgotten when either object is deleted.",
            |s, a| {
                let g = &mut s.graph;
                let from = last_pairs(g);
                if from.is_empty() {
                    fail!("duplicateWithTransform: nothing to repeat; duplicate an object first");
                }
                let count = a.int("count", 1).clamp(1, 10000) as usize;
                g.evaluate();
                let mut copies = HashMap::new();
                let mut copied: Vec<HashMap<NodeId, NodeId>> = vec![HashMap::new(); count];
                let mut made = Vec::new();
                let mut recorded = Vec::new();
                for p in from {
                    let step = repeat::delta(&world_of(g, p.source)?, &world_of(g, p.copy)?);
                    let turn = rotate_of(g, p.copy) - rotate_of(g, p.source);
                    let (mut prev, mut last) = (p.source, p.copy);
                    for record in copied.iter_mut() {
                        let parent = g.parent(last);
                        let c = copy_node(g, last, parent, true, p.mode, "", &mut copies, record)?;
                        let world = step * world_of(g, last)?;
                        let hint = rotate_of(g, last) + turn;
                        repeat::set_world_matrix(g, c, &world, Some(hint))?;
                        made.push(c);
                        prev = last;
                        last = c;
                    }
                    recorded.push(Pair { source: prev, copy: last, mode: p.mode });
                }
                for each in &copied {
                    run_fixups(g, each);
                }
                set_last_pairs(g, recorded);
                g.select(&made, SelectMode::Replace);
                Ok(paths_of(g, &made))
            },
        )
        .flag(flag("count", "c", T::Int, json!(1), "Copies to add, 1 to 10000.").min(1.0).max(10000.0))
        .examples(&["duplicate(\"Fence\")\nmove(\"Fence1\", by=[120, 0, 0])\nduplicateWithTransform(count=8)"]),
    );

    c.register(
        CommandDef::<Session>::new(
            "instance",
            "Make instances of objects: meshes that share their source's mesh, so an edit to any of them changes all. \
             Returns the new names.\n\nreference=True makes references instead, which share the base mesh under the \
             source's modifiers but keep a modifier stack of their own (same base, a different bevel).",
            |s, a| {
                let roots = top_level(s, a, "instance")?;
                if roots.is_empty() {
                    fail!("instance: nothing to instance");
                }
                let mut r = Request {
                    mode: if a.bool("reference") { Mode::Reference } else { Mode::Instance },
                    ..Default::default()
                };
                if a.has("name") && roots.len() == 1 {
                    r.name = a.string("name", "");
                }
                let made = duplicate_roots(&mut s.graph, &roots, &r)?;
                s.graph.select(&made, SelectMode::Replace);
                Ok(paths_of(&s.graph, &made))
            },
        )
        .positional("objects")
        .flag(flag("name", "n", T::String, Value::Null, "Name for a single instance."))
        .flag(flag(
            "reference",
            "",
            T::Bool,
            json!(false),
            "Share only the base mesh and keep a modifier stack of its own (same shape, a different bevel). Off, instances \
             share the modifier stack too, so every copy looks the same and exports as one mesh.",
        ))
        .examples(&["instance(\"Bolt\")", "instance(\"Panel\", reference=True)"]),
    );
    // Option layouts (ops::options overrides) are not implemented.
}
