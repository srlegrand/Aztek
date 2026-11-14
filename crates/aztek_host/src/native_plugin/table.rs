//! The table of functions a plug-in gets: `AzHost` in abi.h, field for field.

use std::ffi::{c_char, c_void};
use std::sync::Arc;

use aztek_geo::{Index, Mesh};
use aztek_scene::node::types;
use aztek_scene::plug::MeshPtr;
use glam::DVec3;
use serde_json::{Map, Value};

use super::nodes::*;
use super::*;
use crate::python_api::{self, MeshArrays};

pub(crate) type Jump = unsafe extern "C" fn(*mut c_void, i32);
pub(crate) type CallbackFn = unsafe extern "C" fn(*mut c_void, *const c_char);

#[repr(C)]
pub(crate) struct AzMeshArrays {
    pub shape: i64,
    pub points: *const f64,
    pub point_count: usize,
    pub counts: *const i64,
    pub face_count: usize,
    pub connects: *const i64,
    pub corner_count: usize,
    pub corner_edges: *const i64,
    pub edges: *const i64,
    pub edge_count: usize,
    pub corner_normals: *const f64,
    pub point_normals: *const f64,
    pub has_uvs: i32,
    pub corner_maps: *const i64,
    pub map_uvs: *const f64,
    pub map_count: usize,
    pub point_ids: *const i64,
    pub point_id_count: usize,
    pub edge_ids: *const i64,
    pub edge_id_count: usize,
    pub face_ids: *const i64,
    pub face_id_count: usize,
    pub owner: *mut c_void,
}

#[repr(C)]
pub(crate) struct AzHost {
    pub abi_version: u32,
    pub size: u32,
    pub string_new: unsafe extern "C" fn(*const c_char) -> *mut c_char,
    pub string_free: unsafe extern "C" fn(*mut c_char),
    pub log: unsafe extern "C" fn(*const c_char, i32),
    pub query: unsafe extern "C" fn(*const c_char, *const c_char) -> *mut c_char,
    pub command: unsafe extern "C" fn(*const c_char, *const c_char, *const c_char) -> *mut c_char,
    pub mel: unsafe extern "C" fn(*const c_char) -> *mut c_char,
    pub python: unsafe extern "C" fn(*const c_char) -> *mut c_char,
    pub scene_version: unsafe extern "C" fn() -> u64,
    pub plugin_info: unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char, *const c_char) -> *mut c_char,
    pub register_command: unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char, i32, *const AzCommandClass) -> *mut c_char,
    pub deregister_command: unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_char,
    pub register_node: unsafe extern "C" fn(*mut c_void, *const c_char, u32, i32, *const c_char, *const c_char, *const AzNodeClass) -> *mut c_char,
    pub deregister_node: unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_char,
    pub attr_index: unsafe extern "C" fn(*const c_char, *const c_char) -> i32,
    pub callback_add: unsafe extern "C" fn(*mut c_void, Option<CallbackFn>, *mut c_void) -> i64,
    pub callback_remove: unsafe extern "C" fn(i64),
    pub mesh_arrays: unsafe extern "C" fn(i64, *mut AzMeshArrays, *mut *mut c_char) -> i32,
    pub mesh_arrays_of: unsafe extern "C" fn(*const c_void, *mut AzMeshArrays) -> i32,
    pub mesh_arrays_free: unsafe extern "C" fn(*mut AzMeshArrays),
    pub mesh_of_node: unsafe extern "C" fn(i64, *mut *mut c_char) -> *mut c_void,
    pub mesh_build: unsafe extern "C" fn(*const f64, usize, *const i64, usize, *const i64, usize) -> *mut c_void,
    pub mesh_with_points: unsafe extern "C" fn(*const c_void, *const f64, usize) -> *mut c_void,
    pub mesh_retain: unsafe extern "C" fn(*const c_void) -> *mut c_void,
    pub mesh_release: unsafe extern "C" fn(*mut c_void),
    pub block_node: unsafe extern "C" fn(*mut Block) -> i64,
    pub block_time: unsafe extern "C" fn(*mut Block) -> f64,
    pub block_number: unsafe extern "C" fn(*mut Block, i32, i32, i32) -> f64,
    pub block_vec3: unsafe extern "C" fn(*mut Block, i32, i32, *mut f64),
    pub block_matrix: unsafe extern "C" fn(*mut Block, i32, i32, *mut f64),
    pub block_string: unsafe extern "C" fn(*mut Block, i32, i32) -> *const c_char,
    pub block_mesh: unsafe extern "C" fn(*mut Block, i32, i32) -> *mut c_void,
    pub block_element_count: unsafe extern "C" fn(*mut Block, i32) -> i32,
    pub block_element_at: unsafe extern "C" fn(*mut Block, i32, i32) -> i32,
    pub block_set_number: unsafe extern "C" fn(*mut Block, i32, i32, i32, f64),
    pub block_set_vec3: unsafe extern "C" fn(*mut Block, i32, i32, *const f64),
    pub block_set_matrix: unsafe extern "C" fn(*mut Block, i32, i32, *const f64),
    pub block_set_string: unsafe extern "C" fn(*mut Block, i32, i32, *const c_char),
    pub block_set_mesh: unsafe extern "C" fn(*mut Block, i32, i32, *const c_void),
    pub block_weights: unsafe extern "C" fn(*mut Block, *mut usize) -> *const f64,
    pub guard_enter: unsafe extern "C" fn(Option<Jump>, *mut c_void) -> *mut c_void,
    pub guard_leave: unsafe extern "C" fn(*mut c_void),
    pub block_curve: unsafe extern "C" fn(*mut Block, i32, i32) -> *mut c_void,
    pub curve_release: unsafe extern "C" fn(*mut c_void),
    pub curve_info: unsafe extern "C" fn(*const c_void, *mut i32, *mut i32, *mut usize, *mut usize, *mut f64),
    pub curve_cvs: unsafe extern "C" fn(*const c_void, *mut f64),
    pub curve_eval: unsafe extern "C" fn(*const c_void, f64, *mut f64, *mut f64),
    pub curve_length: unsafe extern "C" fn(*const c_void) -> f64,
    pub curve_param_at_length: unsafe extern "C" fn(*const c_void, f64) -> f64,
    pub curve_length_at_param: unsafe extern "C" fn(*const c_void, f64) -> f64,
    pub curve_closest_param: unsafe extern "C" fn(*const c_void, *const f64) -> f64,
    pub curve_with_cvs: unsafe extern "C" fn(*const c_void, *const f64, usize) -> *mut c_void,
    pub block_set_curve: unsafe extern "C" fn(*mut Block, i32, i32, *const c_void),
    pub register_data: unsafe extern "C" fn(*mut c_void, *const c_char, u32, *const AzDataClass) -> *mut c_char,
    pub deregister_data: unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_char,
    pub block_data: unsafe extern "C" fn(*mut Block, i32, i32) -> *mut c_void,
    pub block_set_data: unsafe extern "C" fn(*mut Block, i32, i32, *const c_char, *mut c_void),
}

pub(crate) static TABLE: AzHost = AzHost {
    abi_version: ABI_VERSION,
    size: std::mem::size_of::<AzHost>() as u32,
    string_new,
    string_free,
    log,
    query,
    command,
    mel,
    python,
    scene_version,
    plugin_info,
    register_command,
    deregister_command,
    register_node,
    deregister_node,
    attr_index,
    callback_add,
    callback_remove,
    mesh_arrays,
    mesh_arrays_of,
    mesh_arrays_free,
    mesh_of_node,
    mesh_build,
    mesh_with_points,
    mesh_retain,
    mesh_release,
    block_node,
    block_time,
    block_number,
    block_vec3,
    block_matrix,
    block_string,
    block_mesh,
    block_element_count,
    block_element_at,
    block_set_number,
    block_set_vec3,
    block_set_matrix,
    block_set_string,
    block_set_mesh,
    block_weights,
    guard_enter: guard::enter,
    guard_leave: guard::leave,
    block_curve,
    curve_release,
    curve_info,
    curve_cvs,
    curve_eval,
    curve_length,
    curve_param_at_length,
    curve_length_at_param,
    curve_closest_param,
    curve_with_cvs,
    block_set_curve,
    register_data,
    deregister_data,
    block_data,
    block_set_data,
};

// ---------------------------------------------------------------------------------------------------------------------
// Curves: a handle is a boxed curve, the plug-in's until it releases it. The numbers are worked out here, so a
// plug-in's answers are the ones the rest of Aztek gives.

type Curve = aztek_geo::nurbs::NurbsCurve;

pub(crate) fn curve_handle(c: Curve) -> *mut c_void {
    Box::into_raw(Box::new(c)) as *mut c_void
}

pub(crate) unsafe fn curve_ref<'a>(h: *const c_void) -> Option<&'a Curve> {
    (h as *const Curve).as_ref().filter(|c| c.is_valid())
}

unsafe extern "C" fn curve_release(h: *mut c_void) {
    if !h.is_null() {
        drop(Box::from_raw(h as *mut Curve));
    }
}

unsafe extern "C" fn curve_info(h: *const c_void, degree: *mut i32, form: *mut i32, cvs: *mut usize, knots: *mut usize, domain: *mut f64) {
    let Some(c) = curve_ref(h) else { return };
    if !degree.is_null() {
        *degree = c.degree as i32;
    }
    if !form.is_null() {
        *form = c.form.index() as i32;
    }
    if !cvs.is_null() {
        *cvs = c.cvs.len();
    }
    if !knots.is_null() {
        *knots = c.knots.len();
    }
    if !domain.is_null() {
        let (a, b) = c.domain();
        *domain = a;
        *domain.add(1) = b;
    }
}

unsafe extern "C" fn curve_cvs(h: *const c_void, xyz: *mut f64) {
    let Some(c) = curve_ref(h) else { return };
    if xyz.is_null() {
        return;
    }
    for (k, p) in c.cvs.iter().enumerate() {
        *xyz.add(k * 3) = p.x;
        *xyz.add(k * 3 + 1) = p.y;
        *xyz.add(k * 3 + 2) = p.z;
    }
}

unsafe extern "C" fn curve_eval(h: *const c_void, u: f64, point: *mut f64, tangent: *mut f64) {
    let Some(c) = curve_ref(h) else { return };
    let (a, b) = c.domain();
    let u = if a <= b { u.clamp(a, b) } else { a };
    let (p, t) = (c.eval(u), c.tangent(u));
    for k in 0..3 {
        if !point.is_null() {
            *point.add(k) = p[k];
        }
        if !tangent.is_null() {
            *tangent.add(k) = t[k];
        }
    }
}

unsafe extern "C" fn curve_length(h: *const c_void) -> f64 {
    curve_ref(h).map_or(0.0, |c| c.length())
}

unsafe extern "C" fn curve_param_at_length(h: *const c_void, length: f64) -> f64 {
    let Some(c) = curve_ref(h) else { return 0.0 };
    let whole = c.length();
    c.param_at_fraction(if whole > 1e-12 { length / whole } else { 0.0 })
}

unsafe extern "C" fn curve_length_at_param(h: *const c_void, u: f64) -> f64 {
    let Some(c) = curve_ref(h) else { return 0.0 };
    let (a, b) = c.domain();
    let u = if a <= b { u.clamp(a, b) } else { a };
    // The same steps the whole length is measured in, as far as the parameter.
    let n = (c.spans() * 32).max(32);
    let step = (b - a) / n as f64;
    let (mut sum, mut last, mut at) = (0.0, c.eval(a), a);
    while at + step < u {
        at += step;
        let p = c.eval(at);
        sum += (p - last).length();
        last = p;
    }
    sum + (c.eval(u) - last).length()
}

unsafe extern "C" fn curve_closest_param(h: *const c_void, point: *const f64) -> f64 {
    let Some(c) = curve_ref(h) else { return 0.0 };
    if point.is_null() {
        return 0.0;
    }
    c.closest_param(glam::DVec3::new(*point, *point.add(1), *point.add(2)))
}

unsafe extern "C" fn curve_with_cvs(h: *const c_void, xyz: *const f64, count: usize) -> *mut c_void {
    let Some(c) = curve_ref(h) else { return std::ptr::null_mut() };
    if xyz.is_null() || count != c.cvs.len() {
        return std::ptr::null_mut();
    }
    let mut out = c.clone();
    for (k, p) in out.cvs.iter_mut().enumerate() {
        *p = glam::DVec3::new(*xyz.add(k * 3), *xyz.add(k * 3 + 1), *xyz.add(k * 3 + 2));
    }
    curve_handle(out)
}

unsafe extern "C" fn string_new(t: *const c_char) -> *mut c_char {
    give(text(t))
}

unsafe extern "C" fn string_free(t: *mut c_char) {
    drop(take(t));
}

unsafe extern "C" fn log(t: *const c_char, kind: i32) {
    let k = match kind {
        1 => LineKind::Result,
        2 => LineKind::Error,
        4 => LineKind::Warning,
        _ => LineKind::Info,
    };
    defer(text(t), k);
}

fn json_arg(p: *const c_char) -> Value {
    serde_json::from_str(text(p)).unwrap_or(Value::Null)
}

unsafe extern "C" fn query(op: *const c_char, args: *const c_char) -> *mut c_char {
    let (op, args) = (text(op).to_string(), json_arg(args));
    if let Some(r) = aztek_scene::graph::in_compute(|g| python_api::read_query(g, &op, &args)).flatten() {
        return answer(r);
    }
    answer(with_core(|c| python_api::query(c, &op, &args)).and_then(|r| r))
}

unsafe extern "C" fn command(name: *const c_char, args: *const c_char, flags: *const c_char) -> *mut c_char {
    let name = text(name).to_string();
    let args = match json_arg(args) {
        Value::Array(a) => a,
        Value::Null => Vec::new(),
        v => vec![v],
    };
    let flags = match json_arg(flags) {
        Value::Object(m) => m,
        _ => Map::new(),
    };
    // Inside a compute, reading another node's stored value is all a command may do.
    if name == "getAttr" && flags.is_empty() && args.len() == 1 {
        if let Some(r) = aztek_scene::graph::in_compute(|g| {
            let p = g.plug(args[0].as_str().unwrap_or("")).map_err(|e| e.to_string())?;
            g.peek(&p).map(|v| aztek_scene::array_data::of(&v).map_or_else(|| aztek_scene::plug::to_json(&v), |d| d.get_json())).ok_or_else(|| "(kInvalidParameter): Object does not exist".to_string())
        }) {
            return answer(r);
        }
    }
    answer(with_core(|c| c.py_call(&name, args, flags)).and_then(|r| r))
}

unsafe extern "C" fn mel(src: *const c_char) -> *mut c_char {
    let src = text(src).to_string();
    answer(with_core(|c| c.mel_checked(&src)).and_then(|r| r))
}

unsafe extern "C" fn python(src: *const c_char) -> *mut c_char {
    let src = text(src).to_string();
    #[cfg(feature = "python")]
    let r = with_core(|c| crate::python::eval_from_mel(c, &src)).and_then(|r| r);
    #[cfg(not(feature = "python"))]
    let r: Result<Value, String> = Err(format!("(kFailure): this build has no Python: {src}"));
    answer(r)
}

unsafe extern "C" fn scene_version() -> u64 {
    with_core(|c| c.s.graph.version()).unwrap_or(0)
}

unsafe extern "C" fn plugin_info(plugin: *mut c_void, vendor: *const c_char, version: *const c_char, api: *const c_char) -> *mut c_char {
    let Some(p) = (plugin as *mut PluginState).as_mut() else { return give("no plug-in") };
    for (to, from) in [(&mut p.vendor, vendor), (&mut p.version, version), (&mut p.api, api)] {
        let t = text(from);
        if !t.is_empty() {
            *to = t.to_string();
        }
    }
    aztek_scene::scripted::set_plugin_version(&p.name, &p.version);
    std::ptr::null_mut()
}

unsafe extern "C" fn attr_index(ty: *const c_char, attr: *const c_char) -> i32 {
    types::find(text(ty)).and_then(|t| t.attr_index(text(attr))).map_or(-1, |i| i as i32)
}

unsafe extern "C" fn callback_add(plugin: *mut c_void, func: Option<CallbackFn>, user: *mut c_void) -> i64 {
    let (Some(p), Some(func)) = ((plugin as *mut PluginState).as_mut(), func) else { return 0 };
    let id = NEXT_CALLBACK.with(|n| n.replace(n.get() + 1));
    CALLBACKS.with(|c| c.borrow_mut().insert(id, Callback { func, user, _lib: p.lib.clone() }));
    p.callbacks.push(id);
    id
}

unsafe extern "C" fn callback_remove(id: i64) {
    CALLBACKS.with(|c| c.borrow_mut().remove(&id));
    super::messages::unwatch(id);
}

/// Runs a plug-in's callback: what aztekPluginCallback does.
pub(crate) fn fire_callback(s: &mut Session, id: i64, payload: &str) -> Result<(), String> {
    let found = CALLBACKS.with(|c| c.borrow().get(&id).map(|cb| (cb.func, cb.user, cb._lib.clone())));
    let Some((func, user, _lib)) = found else { return Err(format!("aztekPluginCallback: no callback {id}")) };
    let payload = CString::new(payload).unwrap_or_default();
    nested(s, || unsafe { func(user, payload.as_ptr()) });
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------------
// Meshes. A handle is a boxed MeshPtr.

pub(crate) fn mesh_handle(m: MeshPtr) -> *mut c_void {
    Box::into_raw(Box::new(m)) as *mut c_void
}

pub(crate) unsafe fn mesh_ref<'a>(h: *const c_void) -> Option<&'a MeshPtr> {
    (h as *const MeshPtr).as_ref()
}

fn fill(out: &mut AzMeshArrays, shape: i64, a: MeshArrays) {
    let b = Box::new(a);
    *out = AzMeshArrays {
        shape,
        points: b.points.as_ptr(),
        point_count: b.points.len() / 3,
        counts: b.counts.as_ptr(),
        face_count: b.counts.len(),
        connects: b.connects.as_ptr(),
        corner_count: b.connects.len(),
        corner_edges: b.corner_edges.as_ptr(),
        edges: b.edges.as_ptr(),
        edge_count: b.edges.len() / 2,
        corner_normals: b.corner_normals.as_ptr(),
        point_normals: b.point_normals.as_ptr(),
        has_uvs: b.has_uvs as i32,
        corner_maps: b.corner_maps.as_ptr(),
        map_uvs: b.map_uvs.as_ptr(),
        map_count: b.map_uvs.len() / 2,
        point_ids: b.point_ids.as_ptr(),
        point_id_count: b.point_ids.len(),
        edge_ids: b.edge_ids.as_ptr(),
        edge_id_count: b.edge_ids.len(),
        face_ids: b.face_ids.as_ptr(),
        face_id_count: b.face_ids.len(),
        owner: Box::into_raw(b) as *mut c_void,
    };
}

unsafe extern "C" fn mesh_arrays(node: i64, out: *mut AzMeshArrays, error: *mut *mut c_char) -> i32 {
    let r = with_core(|c| python_api::mesh_arrays(c, node as u64)).and_then(|r| r);
    match (r, out.as_mut()) {
        (Ok(a), Some(o)) => {
            let shape = a.shape as i64;
            fill(o, shape, a);
            AZ_OK
        }
        (Err(e), _) => {
            if !error.is_null() {
                *error = give(&e);
            }
            AZ_FAILURE
        }
        _ => AZ_FAILURE,
    }
}

unsafe extern "C" fn mesh_arrays_of(h: *const c_void, out: *mut AzMeshArrays) -> i32 {
    match (mesh_ref(h), out.as_mut()) {
        (Some(m), Some(o)) => {
            fill(o, -1, python_api::arrays_of(m));
            AZ_OK
        }
        _ => AZ_FAILURE,
    }
}

unsafe extern "C" fn mesh_arrays_free(a: *mut AzMeshArrays) {
    if let Some(a) = a.as_mut() {
        if !a.owner.is_null() {
            drop(Box::from_raw(a.owner as *mut MeshArrays));
            a.owner = std::ptr::null_mut();
        }
    }
}

unsafe extern "C" fn mesh_of_node(node: i64, error: *mut *mut c_char) -> *mut c_void {
    match with_core(|c| python_api::out_mesh_of(c, node as u64)).and_then(|r| r) {
        Ok(Some(m)) => mesh_handle(m),
        Ok(None) => std::ptr::null_mut(),
        Err(e) => {
            if !error.is_null() {
                *error = give(&e);
            }
            std::ptr::null_mut()
        }
    }
}

unsafe fn slice<'a, T>(p: *const T, n: usize) -> &'a [T] {
    if p.is_null() || n == 0 {
        &[]
    } else {
        std::slice::from_raw_parts(p, n)
    }
}

unsafe extern "C" fn mesh_build(points: *const f64, np: usize, counts: *const i64, nf: usize, connects: *const i64, nc: usize) -> *mut c_void {
    let pts: Vec<DVec3> = slice(points, np * 3).chunks(3).map(|c| DVec3::new(c[0], c[1], c[2])).collect();
    let counts = slice(counts, nf);
    let connects = slice(connects, nc);
    if counts.iter().any(|&c| c < 3) || counts.iter().sum::<i64>() as usize != connects.len() || connects.iter().any(|&v| v < 0 || v as usize >= pts.len()) {
        return std::ptr::null_mut();
    }
    let sizes: Vec<Index> = counts.iter().map(|&c| c as Index).collect();
    let idx: Vec<Index> = connects.iter().map(|&c| c as Index).collect();
    mesh_handle(Arc::new(Mesh::from_polygons(&pts, &sizes, &idx)))
}

unsafe extern "C" fn mesh_with_points(h: *const c_void, points: *const f64, n: usize) -> *mut c_void {
    let Some(m) = mesh_ref(h) else { return std::ptr::null_mut() };
    let mut out = (**m).clone();
    let mut k = 0;
    let p = slice(points, n * 3);
    for i in 0..out.point_capacity() {
        if !out.point_valid(i) {
            continue;
        }
        if k >= n {
            break;
        }
        out.set_position(i, DVec3::new(p[k * 3], p[k * 3 + 1], p[k * 3 + 2]));
        k += 1;
    }
    mesh_handle(Arc::new(out))
}

unsafe extern "C" fn mesh_retain(h: *const c_void) -> *mut c_void {
    mesh_ref(h).map_or(std::ptr::null_mut(), |m| mesh_handle(m.clone()))
}

unsafe extern "C" fn mesh_release(h: *mut c_void) {
    if !h.is_null() {
        drop(Box::from_raw(h as *mut MeshPtr));
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Crash guard. The kit wraps each call into a plug-in with sigsetjmp and enters a frame here; a fatal signal while a
// frame is entered jumps back to it. Outside one, the signal goes to whoever handled it before.

#[cfg(unix)]
pub(crate) mod guard {
    use std::cell::Cell;
    use std::ffi::c_void;
    use std::sync::Once;

    use super::Jump;

    #[derive(Clone, Copy)]
    struct Frame {
        jump: Option<Jump>,
        buf: *mut c_void,
    }

    thread_local! {
        static TOP: Cell<Frame> = const { Cell::new(Frame { jump: None, buf: std::ptr::null_mut() }) };
    }

    const SIGNALS: [i32; 5] = [libc::SIGSEGV, libc::SIGBUS, libc::SIGFPE, libc::SIGILL, libc::SIGABRT];
    static mut OLD: [std::mem::MaybeUninit<libc::sigaction>; 5] = [const { std::mem::MaybeUninit::uninit() }; 5];
    static INSTALL: Once = Once::new();

    fn install() {
        INSTALL.call_once(|| unsafe {
            for (i, &sig) in SIGNALS.iter().enumerate() {
                let mut sa: libc::sigaction = std::mem::zeroed();
                sa.sa_sigaction = handler as *const () as usize;
                sa.sa_flags = libc::SA_SIGINFO | libc::SA_ONSTACK | libc::SA_NODEFER;
                libc::sigemptyset(&mut sa.sa_mask);
                libc::sigaction(sig, &sa, (*std::ptr::addr_of_mut!(OLD))[i].as_mut_ptr());
            }
        });
    }

    extern "C" fn handler(sig: i32, info: *mut libc::siginfo_t, ctx: *mut c_void) {
        let top = TOP.try_with(|t| t.replace(Frame { jump: None, buf: std::ptr::null_mut() }));
        if let Ok(Frame { jump: Some(j), buf }) = top {
            if !buf.is_null() {
                unsafe { j(buf, sig) };
            }
        }
        let Some(i) = SIGNALS.iter().position(|&s| s == sig) else { return };
        unsafe {
            let old = (*std::ptr::addr_of!(OLD))[i].assume_init();
            if old.sa_sigaction == libc::SIG_DFL || old.sa_sigaction == libc::SIG_IGN {
                // Back to the default, so the fault happens again on return and ends the process as it would have.
                libc::sigaction(sig, &old, std::ptr::null_mut());
            } else if old.sa_flags & libc::SA_SIGINFO != 0 {
                let f: extern "C" fn(i32, *mut libc::siginfo_t, *mut c_void) = std::mem::transmute(old.sa_sigaction);
                f(sig, info, ctx);
            } else {
                let f: extern "C" fn(i32) = std::mem::transmute(old.sa_sigaction);
                f(sig);
            }
        }
    }

    pub(crate) unsafe extern "C" fn enter(jump: Option<Jump>, buf: *mut c_void) -> *mut c_void {
        install();
        let prev = TOP.with(|t| t.replace(Frame { jump, buf }));
        Box::into_raw(Box::new(prev)) as *mut c_void
    }

    pub(crate) unsafe extern "C" fn leave(prev: *mut c_void) {
        if prev.is_null() {
            return;
        }
        let f = *Box::from_raw(prev as *mut Frame);
        TOP.with(|t| t.set(f));
    }
}

/// On Windows the kit guards with structured exception handling on its own.
#[cfg(not(unix))]
pub(crate) mod guard {
    use std::ffi::c_void;

    pub(crate) unsafe extern "C" fn enter(_jump: Option<super::Jump>, _buf: *mut c_void) -> *mut c_void {
        std::ptr::null_mut()
    }

    pub(crate) unsafe extern "C" fn leave(_prev: *mut c_void) {}
}
