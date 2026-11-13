//! Commands and node types from compiled plug-ins, and the data block a node's compute reads and writes.

use std::any::Any;
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{c_char, c_void, CString};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use aztek_ops::{ArgType, Args, CmdResult, CommandDef, CommandError, FlagDef};
use aztek_scene::graph::Graph;
use aztek_scene::node::{EvalContext, NodeBehavior, NodeData};
use aztek_scene::node::types;
use aztek_scene::plug::{NodeId, PlugType, PlugValue, NO_NODE};
use aztek_scene::scripted;
use aztek_undo::FnAction;
use glam::{DMat4, DVec3};

use super::table::{mesh_handle, mesh_ref};
use super::*;

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct AzCommandClass {
    pub ctx: *mut c_void,
    pub create: Option<unsafe extern "C" fn(*mut c_void, *mut i32, *mut *mut c_char) -> *mut c_void>,
    pub destroy: Option<unsafe extern "C" fn(*mut c_void, *mut c_void)>,
    pub do_it: Option<unsafe extern "C" fn(*mut c_void, *mut c_void, *const c_char, *mut i32, *mut *mut c_char, *mut *mut c_char) -> i32>,
    pub undo_it: Option<unsafe extern "C" fn(*mut c_void, *mut c_void, *mut *mut c_char) -> i32>,
    pub redo_it: Option<unsafe extern "C" fn(*mut c_void, *mut c_void, *mut *mut c_char) -> i32>,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct AzNodeClass {
    pub ctx: *mut c_void,
    pub create: Option<unsafe extern "C" fn(*mut c_void, i64, *mut *mut c_char) -> *mut c_void>,
    pub destroy: Option<unsafe extern "C" fn(*mut c_void, *mut c_void)>,
    pub scheduling: Option<unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32>,
    pub compute: Option<unsafe extern "C" fn(*mut c_void, *mut c_void, i32, i32, *mut Block, *mut *mut c_char) -> i32>,
    /// ABI 2: the node's postConstructor, run once the scene can be reached (see `run_post_create`).
    pub post_create: Option<unsafe extern "C" fn(*mut c_void, *mut c_void)>,
}

/// The class as a plug-in built with ABI 1 gives it, without post_create.
#[repr(C)]
#[derive(Clone, Copy)]
struct AzNodeClassV1 {
    ctx: *mut c_void,
    create: Option<unsafe extern "C" fn(*mut c_void, i64, *mut *mut c_char) -> *mut c_void>,
    destroy: Option<unsafe extern "C" fn(*mut c_void, *mut c_void)>,
    scheduling: Option<unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32>,
    compute: Option<unsafe extern "C" fn(*mut c_void, *mut c_void, i32, i32, *mut Block, *mut *mut c_char) -> i32>,
}

/// Nodes just made whose postConstructor waits for the scene to be reachable: the end of the command, or the file
/// reader's next step.
struct PostCreate {
    func: unsafe extern "C" fn(*mut c_void, *mut c_void),
    ctx: *mut c_void,
    inst: *mut c_void,
    entry: Arc<NodeEntry>,
}
thread_local! {
    static PENDING_POST: std::cell::RefCell<Vec<PostCreate>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Runs the postConstructors waiting, each with the scene reachable.
pub(crate) fn run_post_create(s: &mut aztek_scene::Session) {
    loop {
        let next = PENDING_POST.with(|p| {
            let mut p = p.borrow_mut();
            (!p.is_empty()).then(|| p.remove(0))
        });
        let Some(pc) = next else { break };
        let _keep = pc.entry.clone();
        super::nested(s, || unsafe { (pc.func)(pc.ctx, pc.inst) });
    }
}

fn or_else(e: String, fallback: impl FnOnce() -> String) -> String {
    if e.is_empty() {
        fallback()
    } else {
        e
    }
}

fn crash_text(what: &str, owner: &str, why: &str) -> String {
    format!("{what}: the plug-in {owner} crashed ({why}). The scene may be damaged: save it under a new name and restart Aztek.")
}

// ---------------------------------------------------------------------------------------------------------------------
// Commands

pub(crate) struct CmdEntry {
    cls: AzCommandClass,
    name: String,
    owner: String,
    _lib: Arc<Lib>,
}
unsafe impl Send for CmdEntry {}
unsafe impl Sync for CmdEntry {}

/// One run of a command, kept for undo and redo.
struct CmdInst {
    entry: Arc<CmdEntry>,
    inst: *mut c_void,
}
unsafe impl Send for CmdInst {}
unsafe impl Sync for CmdInst {}

impl Drop for CmdInst {
    fn drop(&mut self) {
        if let Some(d) = self.entry.cls.destroy {
            unsafe { d(self.entry.cls.ctx, self.inst) };
        }
    }
}

pub(crate) unsafe extern "C" fn register_command(plugin: *mut c_void, name: *const c_char, flags: *const c_char, raw: i32, cls: *const AzCommandClass) -> *mut c_char {
    let (Some(p), Some(cls)) = ((plugin as *mut PluginState).as_mut(), cls.as_ref()) else { return give("registerCommand: no plug-in") };
    let name = text(name).to_string();
    let flags: Vec<Value> = serde_json::from_str(text(flags)).unwrap_or_default();
    let entry = Arc::new(CmdEntry { cls: *cls, name: name.clone(), owner: p.name.clone(), _lib: p.lib.clone() });
    let owner = p.name.clone();
    let r = with_core(|c| {
        if c.s.commands.find(&name).is_some() {
            return Err(format!("(kFailure): A command named {name} already exists"));
        }
        let e = entry.clone();
        let mut def = CommandDef::<Session>::new(&name, &format!("A command from the plug-in {owner}."), move |s, a| run_command(s, &e, a)).positional("arguments");
        def.owner = owner.clone();
        def.scripted = true;
        def.raw_args = raw != 0;
        for f in &flags {
            let t = |k: &str| f.get(k).and_then(Value::as_str).unwrap_or("").to_string();
            let ty = match t("type").as_str() {
                "bool" => ArgType::Bool,
                "int" => ArgType::Int,
                "float" => ArgType::Float,
                "string" => ArgType::String,
                "list" => ArgType::List,
                _ => ArgType::Any,
            };
            let mut fd = FlagDef::new(&t("name"), &t("short"), ty, Value::Null, "");
            // A flag of several values: MEL reads that many tokens after it.
            if let Some(n) = f.get("arity").and_then(Value::as_u64).filter(|&n| n > 1) {
                fd.arity = n.min(255) as u8;
            }
            def = def.flag(fd);
        }
        c.s.register(def);
        keep_commands(&c.s);
        Ok(())
    })
    .and_then(|r| r);
    match r {
        Ok(()) => {
            p.commands.push(name);
            std::ptr::null_mut()
        }
        Err(e) => give(&e),
    }
}

pub(crate) unsafe extern "C" fn deregister_command(plugin: *mut c_void, name: *const c_char) -> *mut c_char {
    let Some(p) = (plugin as *mut PluginState).as_mut() else { return give("deregisterCommand: no plug-in") };
    let name = text(name).to_string();
    if !p.commands.contains(&name) {
        return give(&format!("(kFailure): No plug-in command named {name}"));
    }
    let _ = with_core(|c| {
        c.s.commands.unregister(&name);
        keep_commands(&c.s);
    });
    p.commands.retain(|x| *x != name);
    std::ptr::null_mut()
}

fn run_command(s: &mut Session, e: &Arc<CmdEntry>, a: &Args) -> CmdResult {
    let input = CString::new(json!({"args": a.positional, "flags": a.flags}).to_string()).unwrap_or_default();
    let was_on = s.graph.undo_stack().enabled();
    let (Some(create), Some(do_it)) = (e.cls.create, e.cls.do_it) else { return Err(CommandError::new(format!("{}: the plug-in gave no doIt", e.name))) };
    let r: Result<(Value, Option<Arc<CmdInst>>), String> = nested(s, || unsafe {
        let mut undoable = 0;
        let mut err: *mut c_char = std::ptr::null_mut();
        let inst = create(e.cls.ctx, &mut undoable, &mut err);
        if inst.is_null() {
            return Err(or_else(take(err), || format!("{}: the plug-in could not make the command", e.name)));
        }
        let inst = Arc::new(CmdInst { entry: e.clone(), inst });
        // An undoable command answers for its whole effect through undoIt: what it runs inside isn't also recorded.
        if undoable != 0 {
            let _ = with_core(|c| c.s.graph.undo_stack_mut().set_enabled(false));
        }
        let mut after = undoable;
        let mut result: *mut c_char = std::ptr::null_mut();
        let rc = do_it(e.cls.ctx, inst.inst, input.as_ptr(), &mut after, &mut result, &mut err);
        if undoable != 0 {
            let _ = with_core(|c| c.s.graph.undo_stack_mut().set_enabled(was_on));
        }
        let result = take(result);
        let err = take(err);
        match rc {
            AZ_OK => {}
            AZ_CRASHED => return Err(crash_text(&e.name, &e.owner, &err)),
            _ => return Err(or_else(err, || format!("{}: the command failed", e.name))),
        }
        let v = if result.is_empty() { Value::Null } else { serde_json::from_str(&result).unwrap_or(Value::String(result)) };
        Ok((v, (undoable != 0).then_some(inst)))
    });
    let (v, inst) = r.map_err(CommandError::new)?;
    if let Some(inst) = inst {
        let redo = inst.clone();
        s.graph.undo_stack_mut().record(FnAction::new(move |g: &mut Graph| replay(g, &inst, false), move |g: &mut Graph| replay(g, &redo, true)));
    }
    Ok(v)
}

/// undoIt or redoIt. The stack hands over only the graph, so a session is put together around it.
fn replay(g: &mut Graph, inst: &Arc<CmdInst>, redo: bool) {
    let cls = inst.entry.cls;
    let Some(f) = (if redo { cls.redo_it } else { cls.undo_it }) else { return };
    let mut s = Session::hollow();
    COMMANDS.with(|c| {
        for d in c.borrow().iter() {
            s.commands.register(d.clone());
        }
    });
    s.commands.set_previewing(true);
    std::mem::swap(g, &mut s.graph);
    let mut err: *mut c_char = std::ptr::null_mut();
    let rc = nested(&mut s, || unsafe { f(cls.ctx, inst.inst, &mut err) });
    std::mem::swap(g, &mut s.graph);
    let err = take(err);
    let what = if redo { "redoIt" } else { "undoIt" };
    match rc {
        AZ_OK => {}
        AZ_CRASHED => defer(format!("// Error: {} //", crash_text(&format!("{} {what}", inst.entry.name), &inst.entry.owner, &err)), LineKind::Error),
        _ => defer(format!("// Error: {}: {what} failed: {err} //", inst.entry.name), LineKind::Error),
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Node types

/// Plug-in nodes that don't say they are safe in parallel take this, one at a time across every plug-in.
static GLOBAL: Mutex<()> = Mutex::new(());

pub(crate) struct NodeEntry {
    cls: AzNodeClass,
    name: String,
    owner: String,
    deformer: bool,
    /// A locator: a shape, whose world matrix and position come first.
    locator: bool,
    outputs: OnceLock<Vec<usize>>,
    /// For kSerial and kGloballySerial: one node of the type at a time.
    lock: Mutex<()>,
    crashed: AtomicBool,
    _lib: Arc<Lib>,
}
unsafe impl Send for NodeEntry {}
unsafe impl Sync for NodeEntry {}

struct NativeNode {
    entry: Arc<NodeEntry>,
    inst: *mut c_void,
    sched: i32,
    id: NodeId,
    name: String,
    /// A failure is reported once per node.
    failed: bool,
}
unsafe impl Send for NativeNode {}
unsafe impl Sync for NativeNode {}

impl Drop for NativeNode {
    fn drop(&mut self) {
        // After a crash the plug-in's memory can't be trusted: its objects are left as they are.
        if self.inst.is_null() || self.entry.crashed.load(Ordering::Relaxed) {
            return;
        }
        if let Some(d) = self.entry.cls.destroy {
            let _g = GLOBAL.lock().unwrap_or_else(|e| e.into_inner());
            unsafe { d(self.entry.cls.ctx, self.inst) };
        }
    }
}

struct ComputeFlag(bool);
impl ComputeFlag {
    fn on() -> Self {
        ComputeFlag(IN_COMPUTE.with(|c| c.replace(true)))
    }
}
impl Drop for ComputeFlag {
    fn drop(&mut self) {
        IN_COMPUTE.with(|c| c.set(self.0));
    }
}

impl NativeNode {
    fn report(&mut self, text: String, kind: LineKind) {
        if !self.failed {
            self.failed = true;
            defer(text, kind);
        }
    }
}

impl NativeNode {
    /// The plug-in's object for the node: made when the node gets its id, on the thread making the node, as the reference makes
    /// it with the node. Creators aren't known to be thread-safe, so under the global lock.
    fn make(&mut self) {
        if !self.inst.is_null() || self.id == NO_NODE || self.entry.crashed.load(Ordering::Relaxed) {
            return;
        }
        let e = self.entry.clone();
        let Some(create) = e.cls.create else { return };
        let _g = GLOBAL.lock().unwrap_or_else(|x| x.into_inner());
        let mut err: *mut c_char = std::ptr::null_mut();
        self.inst = unsafe { create(e.cls.ctx, self.id as i64, &mut err) };
        let err = take(err);
        if self.inst.is_null() {
            self.report(format!("// Error: the plug-in {} could not make a {} node: {err} //", e.owner, e.name), LineKind::Error);
            return;
        }
        self.sched = e.cls.scheduling.map_or(3, |f| unsafe { f(e.cls.ctx, self.inst) });
        if let Some(func) = e.cls.post_create {
            PENDING_POST.with(|p| p.borrow_mut().push(PostCreate { func, ctx: e.cls.ctx, inst: self.inst, entry: e.clone() }));
        }
    }
}

impl NodeBehavior for NativeNode {
    fn compute(&mut self, ctx: &EvalContext, d: &mut NodeData) {
        let e = self.entry.clone();
        if e.locator {
            aztek_scene::transform::Shape::compute_matrices(d);
            let (lp, wp) = (aztek_scene::transform::Locator::LOCAL_POSITION, aztek_scene::transform::Locator::WORLD_POSITION);
            let p = d.matrix(aztek_scene::transform::DagNode::WORLD_MATRIX).transform_point3(d.vec3(lp));
            d.set_output(wp, PlugValue::Vec3(p));
        }
        let (inp, out) = if e.deformer { (d.ty().attr_index("inputPolymesh"), d.ty().attr_index("output")) } else { (None, None) };
        let mut input = inp.and_then(|i| d.mesh(i).cloned());
        // A curve to deform: its CVs go to the plug-in as the points of a mesh with no faces, and come back as the
        // curve's CVs.
        let curve_attrs = if e.deformer { d.ty().attr_index("inputCurve").zip(d.ty().attr_index("outputCurve")) } else { None };
        let curve_in = match (input.is_none(), curve_attrs) {
            (true, Some((ci, _))) => match d.value(ci) {
                PlugValue::Data(Some(data)) => data.as_any().downcast_ref::<aztek_scene::curves::CurveData>().filter(|c| c.0.is_valid()).map(|c| c.0.clone()),
                _ => None,
            },
            _ => None,
        };
        let carrier = curve_in.as_ref().map(|c| Arc::new(aztek_geo::Mesh::from_polygons(&c.cvs, &[], &[])));
        if let Some(m) = &carrier {
            input = Some(m.clone());
        }
        let pass = |d: &mut NodeData| {
            if let Some(o) = out {
                d.set_output(o, PlugValue::Mesh(input.clone()));
            }
        };
        if e.deformer && input.is_none() {
            pass(d);
            return;
        }
        if e.crashed.load(Ordering::Relaxed) {
            pass(d);
            return;
        }
        let Some(compute) = e.cls.compute else { return };
        let _flag = ComputeFlag::on();
        self.make();
        if self.inst.is_null() {
            pass(d);
            return;
        }
        let _g = match self.sched {
            0 => None,
            1 | 2 => Some(e.lock.lock().unwrap_or_else(|x| x.into_inner())),
            _ => Some(GLOBAL.lock().unwrap_or_else(|x| x.into_inner())),
        };
        let outputs = e.outputs.get().cloned().unwrap_or_default();
        let mut block = Block { data: d, time: ctx.time, node: self.id, pending: BTreeMap::new(), strings: Vec::new(), stand_in: inp.zip(carrier.clone()) };
        let mut crashed = None;
        'outs: for out_attr in outputs {
            let mut els: Vec<i32> = if block.d().ty().attrs[out_attr].is_array() { block.ids(out_attr) } else { Vec::new() };
            if els.is_empty() {
                els.push(-1);
            }
            for el in els {
                let mut err: *mut c_char = std::ptr::null_mut();
                let rc = unsafe { compute(e.cls.ctx, self.inst, out_attr as i32, el, &mut block, &mut err) };
                let err = take(err);
                match rc {
                    AZ_OK | AZ_UNKNOWN_PARAMETER => {}
                    AZ_CRASHED => {
                        crashed = Some(err);
                        break 'outs;
                    }
                    // While a file is read a node may compute before its inputs are all connected.
                    _ if aztek_scene::ma_file::reading() => {}
                    _ => {
                        let attr = block.d().ty().attrs[out_attr].name.clone();
                        self.report(format!("// Warning: {}.{attr}: compute failed{} //", self.name, if err.is_empty() { String::new() } else { format!(": {err}") }), LineKind::Warning);
                    }
                }
            }
        }
        block.flush();
        drop(block);
        if let (Some(start), Some((_, co)), Some(o)) = (&curve_in, curve_attrs, out) {
            let moved = match d.value(o) {
                PlugValue::Mesh(Some(m)) if m.positions().len() >= start.cvs.len() => {
                    let mut c = start.clone();
                    for (k, p) in c.cvs.iter_mut().enumerate() {
                        *p = m.positions()[k];
                    }
                    c
                }
                _ => start.clone(),
            };
            d.set_output(co, aztek_scene::curves::to_value(moved));
        }
        if let Some(why) = crashed {
            if !e.crashed.swap(true, Ordering::Relaxed) {
                defer(
                    format!("// Error: {} Nodes of type {} no longer compute. //", crash_text(&format!("{} compute", self.name), &e.owner, &why), e.name),
                    LineKind::Error,
                );
            }
            pass(d);
        }
    }
    fn set_node_id(&mut self, id: NodeId) {
        self.id = id;
        self.make();
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

pub(crate) unsafe extern "C" fn register_node(plugin: *mut c_void, name: *const c_char, _type_id: u32, kind: i32, attrs: *const c_char, outputs: *const c_char, cls: *const AzNodeClass) -> *mut c_char {
    let (Some(p), false) = ((plugin as *mut PluginState).as_mut(), cls.is_null()) else { return give("registerNode: no plug-in") };
    // A plug-in built with ABI 1 gives a shorter class.
    let cls: AzNodeClass = if p.abi >= 2 {
        *cls
    } else {
        let v1 = *(cls as *const AzNodeClassV1);
        AzNodeClass { ctx: v1.ctx, create: v1.create, destroy: v1.destroy, scheduling: v1.scheduling, compute: v1.compute, post_create: None }
    };
    let cls = &cls;
    let name = text(name).to_string();
    let attrs: Vec<Value> = match serde_json::from_str(text(attrs)) {
        Ok(a) => a,
        Err(e) => return give(&format!("registerNode {name}: {e}")),
    };
    let outs: Vec<String> = serde_json::from_str(text(outputs)).unwrap_or_default();
    let deformer = kind == 1;
    let locator = kind == 2;
    let entry = Arc::new(NodeEntry {
        cls: *cls,
        name: name.clone(),
        owner: p.name.clone(),
        deformer,
        locator,
        outputs: OnceLock::new(),
        lock: Mutex::new(()),
        crashed: AtomicBool::new(false),
        _lib: p.lib.clone(),
    });
    let e = entry.clone();
    let create: aztek_scene::node::CreateFn = Arc::new(move || Box::new(NativeNode { entry: e.clone(), inst: std::ptr::null_mut(), sched: 3, id: NO_NODE, name: String::new(), failed: false }));
    let kind = if deformer { scripted::NativeKind::Deformer } else if locator { scripted::NativeKind::Locator } else { scripted::NativeKind::Depend };
    if let Err(err) = scripted::register_native_type(&name, &p.name, &attrs, kind, create) {
        return give(&format!("(kFailure): {err}"));
    }
    let t = types::find(&name).expect("just registered");
    let mut idx: Vec<usize> = outs.iter().filter_map(|o| t.attr_index(o)).collect();
    if deformer && idx.is_empty() {
        idx.extend(t.attr_index("output"));
    }
    let _ = entry.outputs.set(idx);
    p.nodes.push(name);
    std::ptr::null_mut()
}

pub(crate) unsafe extern "C" fn deregister_node(plugin: *mut c_void, name: *const c_char) -> *mut c_char {
    let Some(p) = (plugin as *mut PluginState).as_mut() else { return give("deregisterNode: no plug-in") };
    let name = text(name).to_string();
    if !p.nodes.contains(&name) {
        return give(&format!("(kFailure): No plug-in node type named {name}"));
    }
    scripted::deregister_type(&name);
    p.nodes.retain(|x| *x != name);
    std::ptr::null_mut()
}

// ---------------------------------------------------------------------------------------------------------------------
// The data block

pub(crate) struct Block {
    data: *mut NodeData,
    time: f64,
    node: NodeId,
    /// Array elements written, merged into the node's when the compute is done.
    pending: BTreeMap<usize, BTreeMap<i32, PlugValue>>,
    strings: Vec<CString>,
    /// A mesh standing in for an input's value during this compute: a curve's CVs as the points to deform.
    stand_in: Option<(usize, aztek_scene::plug::MeshPtr)>,
}

impl Block {
    fn d(&self) -> &NodeData {
        unsafe { &*self.data }
    }
    fn dm(&mut self) -> &mut NodeData {
        unsafe { &mut *self.data }
    }
    fn attr(&self, a: i32) -> Option<usize> {
        (a >= 0 && (a as usize) < self.d().ty().attrs.len()).then_some(a as usize)
    }
    fn read(&self, a: usize, el: i32) -> PlugValue {
        if let Some((at, m)) = &self.stand_in {
            if *at == a && el < 0 {
                return PlugValue::Mesh(Some(m.clone()));
            }
        }
        if el >= 0 {
            if let Some(v) = self.pending.get(&a).and_then(|m| m.get(&el)) {
                return v.clone();
            }
            return self.d().element(a, el).cloned().unwrap_or_else(|| self.d().ty().attrs[a].default_value.clone());
        }
        self.d().value(a).clone()
    }
    fn write(&mut self, a: usize, el: i32, v: PlugValue) {
        if el >= 0 {
            self.pending.entry(a).or_default().insert(el, v);
        } else {
            self.dm().set_output(a, v);
        }
    }
    fn ids(&self, a: usize) -> Vec<i32> {
        let mut s: BTreeSet<i32> = self.d().elements(a).iter().map(|x| x.0).collect();
        if let Some(m) = self.pending.get(&a) {
            s.extend(m.keys());
        }
        s.into_iter().collect()
    }
    fn flush(&mut self) {
        for (a, m) in std::mem::take(&mut self.pending) {
            let mut all: BTreeMap<i32, PlugValue> = self.d().elements(a).iter().cloned().collect();
            all.extend(m);
            self.dm().set_output_elements(a, all.into_iter().collect());
        }
    }
}

fn number_of(v: &PlugValue, comp: i32) -> f64 {
    match v {
        PlugValue::Vec3(x) => x[comp.clamp(0, 2) as usize],
        v => v.scalar().unwrap_or(0.0),
    }
}

fn typed(ty: PlugType, old: &PlugValue, comp: i32, x: f64) -> PlugValue {
    match ty {
        PlugType::Bool => PlugValue::Bool(x != 0.0),
        PlugType::Int | PlugType::Enum => PlugValue::Int(x.round() as i64),
        PlugType::Float => PlugValue::Float(x),
        PlugType::Vec3 => {
            let mut v = if let PlugValue::Vec3(v) = old { *v } else { DVec3::ZERO };
            if (0..3).contains(&comp) {
                v[comp as usize] = x;
            } else {
                v = DVec3::splat(x);
            }
            PlugValue::Vec3(v)
        }
        PlugType::String => PlugValue::String(x.to_string()),
        _ => old.clone(),
    }
}

pub(crate) unsafe extern "C" fn block_node(b: *mut Block) -> i64 {
    b.as_ref().map_or(-1, |b| b.node as i64)
}

pub(crate) unsafe extern "C" fn block_time(b: *mut Block) -> f64 {
    b.as_ref().map_or(0.0, |b| b.time)
}

pub(crate) unsafe extern "C" fn block_number(b: *mut Block, attr: i32, el: i32, comp: i32) -> f64 {
    let Some(b) = b.as_ref() else { return 0.0 };
    b.attr(attr).map_or(0.0, |a| number_of(&b.read(a, el), comp))
}

pub(crate) unsafe extern "C" fn block_vec3(b: *mut Block, attr: i32, el: i32, out: *mut f64) {
    let (Some(b), false) = (b.as_ref(), out.is_null()) else { return };
    let v = match b.attr(attr).map(|a| b.read(a, el)) {
        Some(PlugValue::Vec3(v)) => v,
        Some(v) => DVec3::splat(v.scalar().unwrap_or(0.0)),
        None => DVec3::ZERO,
    };
    std::ptr::copy_nonoverlapping(v.to_array().as_ptr(), out, 3);
}

pub(crate) unsafe extern "C" fn block_matrix(b: *mut Block, attr: i32, el: i32, out: *mut f64) {
    let (Some(b), false) = (b.as_ref(), out.is_null()) else { return };
    let m = match b.attr(attr).map(|a| b.read(a, el)) {
        Some(PlugValue::Matrix(m)) => m,
        _ => DMat4::IDENTITY,
    };
    std::ptr::copy_nonoverlapping(m.to_cols_array().as_ptr(), out, 16);
}

pub(crate) unsafe extern "C" fn block_string(b: *mut Block, attr: i32, el: i32) -> *const c_char {
    let Some(b) = b.as_mut() else { return std::ptr::null() };
    let s = match b.attr(attr).map(|a| b.read(a, el)) {
        Some(PlugValue::String(s)) => s,
        // Array data goes as a JSON list.
        Some(v @ PlugValue::Data(_)) if b.attr(attr).is_some_and(|a| aztek_scene::array_data::of(&b.d().ty().attrs[a].default_value).is_some()) => {
            aztek_scene::array_data::of(&v).map_or_else(|| "[]".to_string(), |d| d.values_json().to_string())
        }
        Some(PlugValue::Mesh(_)) | Some(PlugValue::Data(_)) | Some(PlugValue::Matrix(_)) | None => String::new(),
        Some(v) => number_of(&v, -1).to_string(),
    };
    b.strings.push(CString::new(s.replace('\0', " ")).unwrap());
    b.strings.last().unwrap().as_ptr()
}

pub(crate) unsafe extern "C" fn block_mesh(b: *mut Block, attr: i32, el: i32) -> *mut c_void {
    let Some(b) = b.as_ref() else { return std::ptr::null_mut() };
    match b.attr(attr).map(|a| b.read(a, el)) {
        Some(PlugValue::Mesh(Some(m))) => mesh_handle(m),
        _ => std::ptr::null_mut(),
    }
}

pub(crate) unsafe extern "C" fn block_element_count(b: *mut Block, attr: i32) -> i32 {
    let Some(b) = b.as_ref() else { return 0 };
    b.attr(attr).map_or(0, |a| b.ids(a).len() as i32)
}

pub(crate) unsafe extern "C" fn block_element_at(b: *mut Block, attr: i32, i: i32) -> i32 {
    let Some(b) = b.as_ref() else { return -1 };
    b.attr(attr).and_then(|a| b.ids(a).get(i.max(0) as usize).copied()).unwrap_or(-1)
}

pub(crate) unsafe extern "C" fn block_set_number(b: *mut Block, attr: i32, el: i32, comp: i32, x: f64) {
    let Some(b) = b.as_mut() else { return };
    let Some(a) = b.attr(attr) else { return };
    let ty = b.d().ty().attrs[a].ty;
    let v = typed(ty, &b.read(a, el), comp, x);
    b.write(a, el, v);
}

pub(crate) unsafe extern "C" fn block_set_vec3(b: *mut Block, attr: i32, el: i32, v: *const f64) {
    let (Some(b), false) = (b.as_mut(), v.is_null()) else { return };
    let Some(a) = b.attr(attr) else { return };
    let x = DVec3::new(*v, *v.add(1), *v.add(2));
    b.write(a, el, PlugValue::Vec3(x));
}

pub(crate) unsafe extern "C" fn block_set_matrix(b: *mut Block, attr: i32, el: i32, v: *const f64) {
    let (Some(b), false) = (b.as_mut(), v.is_null()) else { return };
    let Some(a) = b.attr(attr) else { return };
    let mut m = [0.0; 16];
    std::ptr::copy_nonoverlapping(v, m.as_mut_ptr(), 16);
    b.write(a, el, PlugValue::Matrix(DMat4::from_cols_array(&m)));
}

pub(crate) unsafe extern "C" fn block_set_string(b: *mut Block, attr: i32, el: i32, s: *const c_char) {
    let Some(b) = b.as_mut() else { return };
    let Some(a) = b.attr(attr) else { return };
    // Array data comes as a JSON list.
    if let Some(empty) = aztek_scene::array_data::of(&b.d().ty().attrs[a].default_value).cloned() {
        let v: serde_json::Value = serde_json::from_str(text(s)).unwrap_or(serde_json::Value::Null);
        b.write(a, el, empty.with_values(&v).value());
        return;
    }
    b.write(a, el, PlugValue::String(text(s).to_string()));
}

pub(crate) unsafe extern "C" fn block_set_mesh(b: *mut Block, attr: i32, el: i32, m: *const c_void) {
    let Some(b) = b.as_mut() else { return };
    let Some(a) = b.attr(attr) else { return };
    b.write(a, el, PlugValue::Mesh(mesh_ref(m).cloned()));
}

/// A curve input as a handle the plug-in releases; null when nothing valid is connected.
pub(crate) unsafe extern "C" fn block_curve(b: *mut Block, attr: i32, el: i32) -> *mut c_void {
    let Some(b) = b.as_ref() else { return std::ptr::null_mut() };
    match b.attr(attr).map(|a| b.read(a, el)) {
        Some(PlugValue::Data(Some(d))) => match d.as_any().downcast_ref::<aztek_scene::curves::CurveData>() {
            Some(c) if c.0.is_valid() => super::table::curve_handle(c.0.clone()),
            _ => std::ptr::null_mut(),
        },
        _ => std::ptr::null_mut(),
    }
}

pub(crate) unsafe extern "C" fn block_set_curve(b: *mut Block, attr: i32, el: i32, c: *const c_void) {
    let Some(b) = b.as_mut() else { return };
    let Some(a) = b.attr(attr) else { return };
    let data = super::table::curve_ref(c).map(|c| Arc::new(aztek_scene::curves::CurveData(c.clone())) as aztek_scene::plug::DataPtr);
    b.write(a, el, PlugValue::Data(data));
}

pub(crate) unsafe extern "C" fn block_weights(b: *mut Block, n: *mut usize) -> *const f64 {
    let Some(b) = b.as_ref() else { return std::ptr::null() };
    match scripted::deformer_weights(b.d()) {
        Some(w) => {
            if !n.is_null() {
                *n = w.len();
            }
            w.as_ptr()
        }
        None => std::ptr::null(),
    }
}
