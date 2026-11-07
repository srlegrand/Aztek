//! Prims as things the scene's commands act on. A prim is named `|stage1|stageShape1,/World/thing`, as the reference's USD plug-in writes
//! them; `select`, `ls`, `move`, `rotate`, `scale` and `delete` are wrapped here so they take such names.

use std::cell::RefCell;
use std::rc::Rc;

use aztek_ops::{Args, CmdResult, CommandDef, CommandError, Commands};
use aztek_scene::graph::Graph;
use aztek_scene::plug::NodeId;
use aztek_scene::session::Session;
use aztek_scene::transform::{decompose_full, euler_near, RotateOrder};
use aztek_undo::FnAction;
use glam::{DMat3, DMat4, DVec3};
use openusd::gf;
use openusd::sdf::{self, Value};
use openusd::tf::Token;
use openusd::usd::{Prim, Stage, TimeCode};
use openusd_schemas::geom::XformQuery;
use serde_json::{json, Value as J};

use crate::stages::{self, Entry};
use crate::util::*;

thread_local! {
    static SELECTED: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

pub fn selected() -> Vec<String> {
    SELECTED.with(|s| s.borrow().clone())
}

fn set_selected(v: Vec<String>) {
    SELECTED.with(|s| *s.borrow_mut() = v);
}

/// Splits `shape,/prim` or `shape,/prim.attr`. None for an ordinary name.
pub fn split_name(name: &str) -> Option<(&str, &str)> {
    let i = name.find(",/")?;
    Some((&name[..i], &name[i + 1..]))
}

/// A prim's stage, the proxy showing it and its path.
pub struct Target {
    pub shape: NodeId,
    pub entry: Rc<Entry>,
    pub path: sdf::Path,
}

pub fn resolve(g: &mut Graph, name: &str) -> Result<Target, String> {
    let (shape_name, prim) = split_name(name).ok_or_else(|| format!("'{name}' is not a prim: it needs proxy,/path"))?;
    let shape = stages::proxy_named(g, shape_name).ok_or_else(|| format!("no proxy shape named '{shape_name}'"))?;
    let entry = stages::entry_of(g, shape)?;
    let path = sdf::Path::new(prim).map_err(|e| format!("{prim}: {e}"))?;
    Ok(Target { shape, entry, path })
}

/// The name of a prim under a proxy, as selection lists write it.
pub fn name_of(g: &Graph, shape: NodeId, path: &str) -> String {
    format!("{},{path}", g.full_path(shape))
}

pub fn err(e: impl std::fmt::Display) -> CommandError {
    CommandError::new(e.to_string())
}

fn names_in(a: &Args) -> Vec<String> {
    let mut out = Vec::new();
    for v in &a.positional {
        match v {
            J::String(s) => out.push(s.clone()),
            J::Array(l) => out.extend(l.iter().filter_map(|x| x.as_str().map(str::to_string))),
            _ => {}
        }
    }
    out
}

/// A prim's transform to the world, proxy's transform included, without its own ops: (parent to world).
pub fn parent_world(g: &mut Graph, t: &Target, time: f64) -> DMat4 {
    let mut m = g.node(t.shape).map_or(DMat4::IDENTITY, |n| n.matrix(aztek_scene::transform::DagNode::WORLD_MATRIX));
    let mut chain: Vec<sdf::Path> = t.path.ancestors_below_root().collect();
    chain.retain(|p| *p != t.path);
    chain.reverse();
    let mut local_chain = Vec::new();
    for a in chain {
        if let Ok(p) = t.entry.stage.prim(a) {
            let (l, reset) = local_of(&p, time);
            if reset {
                local_chain.clear();
            }
            local_chain.push(l);
        }
    }
    for l in local_chain {
        m *= l;
    }
    m
}

pub fn local_of(prim: &Prim, time: f64) -> (DMat4, bool) {
    match XformQuery::for_prim(prim) {
        Ok(q) => (q.local_transformation(TimeCode::new(time)).map(|m| from_gf(&m)).unwrap_or(DMat4::IDENTITY), q.resets_xform_stack()),
        Err(_) => (DMat4::IDENTITY, false),
    }
}

pub fn world_of(g: &mut Graph, t: &Target, time: f64) -> DMat4 {
    let parent = parent_world(g, t, time);
    match t.entry.stage.prim(t.path.clone()) {
        Ok(p) => {
            let (l, reset) = local_of(&p, time);
            if reset {
                g.node(t.shape).map_or(DMat4::IDENTITY, |n| n.matrix(aztek_scene::transform::DagNode::WORLD_MATRIX)) * l
            } else {
                parent * l
            }
        }
        Err(_) => parent,
    }
}

fn order_tokens(prim: &Prim) -> Vec<String> {
    prim.attribute("xformOpOrder").get::<Vec<Token>>().ok().flatten().map(|v| v.iter().map(|t| t.as_str().to_string()).collect()).unwrap_or_default()
}

/// Where each op sits in the reference's stack; used to put a new op in its place.
fn rank(op: &str) -> Option<usize> {
    let bare = op.strip_prefix("!invert!").unwrap_or(op);
    Some(match bare {
        "xformOp:translate" => 0,
        "xformOp:translate:rotatePivotTranslate" => 1,
        "xformOp:translate:rotatePivot" if op.starts_with('!') => 5,
        "xformOp:translate:rotatePivot" => 2,
        "xformOp:rotateXYZ:rotateAxis" => 4,
        "xformOp:translate:scalePivotTranslate" => 6,
        "xformOp:translate:scalePivot" if op.starts_with('!') => 10,
        "xformOp:translate:scalePivot" => 7,
        "xformOp:scale" => 9,
        _ if bare.starts_with("xformOp:rotate") && bare.len() == "xformOp:rotateXYZ".len() => 3,
        _ => return None,
    })
}

/// Sets an attribute at the time it's sampled at, if it's sampled: editing an animated value keys it, as USD tools do.
fn set_value(prim: &Prim, name: &str, ty: &str, v: Value, time: f64) -> Result<(), String> {
    let a = prim.attribute(name);
    if a.num_time_samples().unwrap_or(0) > 0 {
        a.set_at(v, TimeCode::new(time)).map_err(|e| e.to_string())?;
    } else {
        let stage = prim.stage().clone();
        let path = a.path().clone();
        stage.create_attribute(path, ty).map_err(|e| e.to_string())?.set(v).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn d3(v: DVec3) -> Value {
    Value::Vec3d(gf::vec3d(v.x, v.y, v.z))
}

fn f3(v: DVec3) -> Value {
    Value::Vec3f(v3f(v))
}

fn ensure_op(prim: &Prim, ops: &mut Vec<String>, op: &str) {
    if ops.iter().any(|o| o == op) {
        return;
    }
    let r = rank(op).unwrap_or(0);
    let at = ops.iter().position(|o| rank(o).is_some_and(|x| x > r)).unwrap_or(ops.len());
    ops.insert(at, op.to_string());
    let _ = prim;
}

fn write_order(prim: &Prim, ops: &[String]) -> Result<(), String> {
    let stage = prim.stage().clone();
    let path = prim.attribute("xformOpOrder").path().clone();
    let tokens: Vec<Token> = ops.iter().map(|o| o.as_str().into()).collect();
    let a = stage.create_attribute(path, "token[]").map_err(|e| e.to_string())?;
    let a = a.set_variability(sdf::Variability::Uniform).map_err(|e| e.to_string())?;
    a.set(Value::TokenVec(tokens)).map_err(|e| e.to_string())?;
    Ok(())
}

/// The rotate op in use and its order.
fn rotate_op(ops: &[String]) -> Option<(String, RotateOrder)> {
    for o in ops {
        let Some(rest) = o.strip_prefix("xformOp:rotate") else { continue };
        let i = match rest {
            "XYZ" => 0,
            "YZX" => 1,
            "ZXY" => 2,
            "XZY" => 3,
            "YXZ" => 4,
            "ZYX" => 5,
            _ => continue,
        };
        return Some((o.clone(), RotateOrder::from_index(i)));
    }
    None
}

/// The order a prim's rotate op turns in, for building its matrix.
pub fn rotation_order(prim: &Prim) -> RotateOrder {
    rotate_op(&order_tokens(prim)).map_or(RotateOrder::Xyz, |x| x.1)
}

/// Writes the angles asked for, after set_local has written the nearest it could read from a matrix: 360 stays 360.
pub fn keep_euler(prim: &Prim, e: DVec3, time: f64) -> Result<(), String> {
    let ops = order_tokens(prim);
    if let Some((op, _)) = rotate_op(&ops) {
        set_value(prim, &op, "float3", f3(e), time)?;
    }
    Ok(())
}

fn all_ops_known(ops: &[String]) -> bool {
    ops.iter().all(|o| o == "!resetXformStack!" || rank(o).is_some())
}

/// Authors a prim's new local matrix on the edit target, keeping its op stack when the stack is one the reference's channels can
/// hold, and otherwise writing a single matrix op over it.
pub fn set_local(prim: &Prim, m: &DMat4, time: f64) -> Result<(), String> {
    let mut ops = order_tokens(prim);
    if !all_ops_known(&ops) {
        let keep_reset = ops.first().is_some_and(|o| o == "!resetXformStack!");
        set_value(prim, "xformOp:transform", "matrix4d", Value::Matrix4d(to_gf(m)), time)?;
        let mut order = vec!["xformOp:transform".to_string()];
        if keep_reset {
            order.insert(0, "!resetXformStack!".into());
        }
        return write_order(prim, &order);
    }
    // Pivots stay; the channels around them absorb the change.
    let read = |name: &str| -> DVec3 {
        let a = prim.attribute(name);
        if let Ok(Some(v)) = a.get_at::<gf::Vec3d>(TimeCode::new(time)) {
            return DVec3::new(v.x, v.y, v.z);
        }
        a.get_at::<gf::Vec3f>(TimeCode::new(time)).ok().flatten().map_or(DVec3::ZERO, |v| DVec3::new(v.x as f64, v.y as f64, v.z as f64))
    };
    let (rp, sp) = (read("xformOp:translate:rotatePivot"), read("xformOp:translate:scalePivot"));
    let (rpt, spt) = (read("xformOp:translate:rotatePivotTranslate"), read("xformOp:translate:scalePivotTranslate"));
    let (t, r, s, _) = decompose_full(m);
    let (op, order) = rotate_op(&ops).unwrap_or(("xformOp:rotateXYZ".to_string(), RotateOrder::Xyz));
    let old = {
        let a = prim.attribute(op.as_str());
        a.get_at::<gf::Vec3f>(TimeCode::new(time)).ok().flatten().map_or(DVec3::ZERO, |v| DVec3::new(v.x as f64, v.y as f64, v.z as f64))
    };
    let e = euler_near(&r, order, old);
    // m = T Rpt Rp R Rp^-1 Spt Sp S Sp^-1, so the translation channel is what's left of m's translation.
    let tr = DMat4::from_translation(rpt + rp) * DMat4::from_mat3(r) * DMat4::from_translation(-rp + spt + sp) * DMat4::from_scale(s) * DMat4::from_translation(-sp);
    let translate = t - tr.w_axis.truncate();

    let near = |a: DVec3, b: DVec3| (a - b).length_squared() < 1e-18;
    if !near(translate, DVec3::ZERO) || ops.iter().any(|o| o == "xformOp:translate") {
        set_value(prim, "xformOp:translate", "double3", d3(translate), time)?;
        ensure_op(prim, &mut ops, "xformOp:translate");
    }
    if !near(e, DVec3::ZERO) || ops.iter().any(|o| *o == op) {
        set_value(prim, &op, "float3", f3(e), time)?;
        ensure_op(prim, &mut ops, &op);
    }
    if !near(s, DVec3::ONE) || ops.iter().any(|o| o == "xformOp:scale") {
        set_value(prim, "xformOp:scale", "float3", f3(s), time)?;
        ensure_op(prim, &mut ops, "xformOp:scale");
    }
    write_order(prim, &ops)
}

fn vec3_of(v: Option<&J>) -> Option<DVec3> {
    let a = v?.as_array()?;
    Some(DVec3::new(a.first()?.as_f64()?, a.get(1)?.as_f64()?, a.get(2)?.as_f64()?))
}

#[derive(Clone, Copy, PartialEq)]
enum Motion {
    Move,
    Rotate,
    Scale,
}

/// (numbers given, prims named): `move 1 2 3 "shape,/p"`, with `-by`, or the selected prims.
fn motion_of(a: &Args, kind: Motion) -> Option<(DVec3, bool)> {
    if let Some(v) = vec3_of(a.get("by")) {
        return Some((v, true));
    }
    let nums: Vec<f64> = a.positional.iter().filter_map(J::as_f64).collect();
    let d = if kind == Motion::Scale { 1.0 } else { 0.0 };
    if nums.is_empty() {
        return None;
    }
    let relative = a.bool("relative");
    Some((DVec3::new(nums[0], nums.get(1).copied().unwrap_or(d), nums.get(2).copied().unwrap_or(d)), relative))
}

fn apply_motion(s: &mut Session, name: &str, v: DVec3, relative: bool, kind: Motion) -> Result<(), String> {
    let t = resolve(&mut s.graph, name)?;
    t.entry.check_target()?;
    let time = stages::time_of(&mut s.graph, t.shape);
    let parent = parent_world(&mut s.graph, &t, time);
    let world = world_of(&mut s.graph, &t, time);
    let (wt, wr, ws, _) = decompose_full(&world);
    let new_world = match kind {
        Motion::Move => {
            let p = if relative { wt + v } else { v };
            DMat4::from_translation(p) * DMat4::from_mat3(DMat3::from_cols(world.x_axis.truncate(), world.y_axis.truncate(), world.z_axis.truncate()))
        }
        Motion::Rotate => {
            let rot = aztek_scene::transform::euler_to_matrix(v, RotateOrder::Xyz);
            let m3 = if relative { DMat3::from_mat4(rot) * wr } else { DMat3::from_mat4(rot) };
            DMat4::from_translation(wt) * DMat4::from_mat3(m3) * DMat4::from_scale(ws)
        }
        Motion::Scale => {
            let sc = if relative { ws * v } else { v };
            DMat4::from_translation(wt) * DMat4::from_mat3(wr) * DMat4::from_scale(sc)
        }
    };
    let local = parent.inverse() * new_world;
    let entry = t.entry.clone();
    let path = t.path.clone();
    entry.edit(s, "transform", |st: &Stage| {
        let prim = st.prim(path.clone()).map_err(|e| e.to_string())?;
        set_local(&prim, &local, time)
    })
}

fn wrap(c: &mut Commands<Session>, name: &str, f: impl Fn(&mut Session, &Args, &aztek_ops::RunFn<Session>) -> CmdResult + Send + Sync + 'static) {
    let Some(old) = c.find(name).cloned() else { return };
    let prev = old.run.clone();
    let mut d: CommandDef<Session> = old;
    d.run = std::sync::Arc::new(move |s, a| f(s, a, &prev));
    c.register(d);
}

fn record_selection(s: &mut Session, before: Vec<String>, after: Vec<String>) {
    if before == after {
        return;
    }
    set_selected(after.clone());
    s.graph.undo_stack_mut().record(FnAction::new(move |_g: &mut Graph| set_selected(before.clone()), move |_g: &mut Graph| set_selected(after.clone())).selection(true));
}

fn run_select(s: &mut Session, a: &Args, prev: &aztek_ops::RunFn<Session>) -> CmdResult {
    let names = names_in(a);
    let (usd, plain): (Vec<String>, Vec<String>) = names.into_iter().partition(|n| split_name(n).is_some());
    let adds = a.bool("add") || a.bool("addFirst");
    let removes = a.bool("deselect");
    let toggles = a.bool("toggle");
    let before = selected();
    let mut after = if adds || removes || toggles { before.clone() } else { Vec::new() };
    if usd.is_empty() {
        let r = prev(s, a);
        // Asking for nodes alone replaces prims too; a filter or a query does not change what's selected.
        if r.is_ok() && !a.bool("query") {
            if a.bool("clear") || (!adds && !removes && !toggles) {
                record_selection(s, before, Vec::new());
            }
        }
        return r;
    }
    for n in &usd {
        let t = resolve(&mut s.graph, n).map_err(err)?;
        let canon = name_of(&s.graph, t.shape, t.path.as_str());
        let there = after.iter().position(|x| *x == canon);
        match there {
            Some(i) if removes || toggles => {
                after.remove(i);
            }
            None if !removes => after.push(canon),
            _ => {}
        }
    }
    if adds || removes || toggles {
        if !plain.is_empty() {
            let mut call = a.clone();
            call.positional = plain.iter().map(|x| json!(x)).collect();
            prev(s, &call)?;
        }
    } else {
        let mut call = Args::default();
        if plain.is_empty() {
            call.flags.insert("clear".into(), json!(true));
        } else {
            call.positional = plain.iter().map(|x| json!(x)).collect();
        }
        prev(s, &call)?;
    }
    record_selection(s, before, after);
    Ok(J::Null)
}

fn run_ls(s: &mut Session, a: &Args, prev: &aztek_ops::RunFn<Session>) -> CmdResult {
    let names = names_in(a);
    let mut extra: Vec<J> = Vec::new();
    let mut plain = Vec::new();
    for n in names {
        if split_name(&n).is_some() {
            extra.push(json!(n));
        } else {
            plain.push(n);
        }
    }
    let typed = a.has("type") || a.has("dagObjects") || a.has("transforms") || a.has("shapes");
    if a.bool("selection") && !typed {
        extra.extend(selected().into_iter().map(|x| json!(x)));
    }
    if extra.is_empty() {
        return prev(s, a);
    }
    let mut call = a.clone();
    call.positional = plain.iter().map(|x| json!(x)).collect();
    let had_names = !a.positional.is_empty();
    let r = if had_names && plain.is_empty() { json!([]) } else { prev(s, &call)? };
    let mut list = r.as_array().cloned().unwrap_or_default();
    list.extend(extra);
    Ok(J::Array(list))
}

fn run_motion(s: &mut Session, a: &Args, prev: &aztek_ops::RunFn<Session>, kind: Motion) -> CmdResult {
    let named: Vec<String> = names_in(a);
    let usd: Vec<String> = named.iter().filter(|n| split_name(n).is_some()).cloned().collect();
    let targets = if !usd.is_empty() {
        usd
    } else if named.is_empty() {
        selected()
    } else {
        Vec::new()
    };
    let Some((v, relative)) = (!targets.is_empty()).then(|| motion_of(a, kind)).flatten() else { return prev(s, a) };
    for name in &targets {
        apply_motion(s, name, v, relative, kind).map_err(err)?;
    }
    // Nodes selected alongside prims move with them.
    let plain: Vec<J> = named.iter().filter(|n| split_name(n).is_none()).map(|x| json!(x)).collect();
    let others = !s.graph.selection().is_empty() && named.is_empty();
    if others || !plain.is_empty() {
        let mut call = a.clone();
        call.positional.retain(|p| p.as_str().is_none_or(|n| split_name(n).is_none()));
        prev(s, &call)?;
    }
    Ok(J::Null)
}

pub fn install_wrappers(c: &mut Commands<Session>) {
    // A scene being replaced takes its stages and prim selection with it.
    wrap(c, "file", |s, a, p| {
        if a.bool("new") || a.bool("open") {
            let r = p(s, a);
            set_selected(Vec::new());
            stages::forget_all();
            return r;
        }
        p(s, a)
    });
    wrap(c, "select", run_select);
    wrap(c, "ls", run_ls);
    wrap(c, "move", |s, a, p| run_motion(s, a, p, Motion::Move));
    wrap(c, "rotate", |s, a, p| run_motion(s, a, p, Motion::Rotate));
    wrap(c, "scale", |s, a, p| run_motion(s, a, p, Motion::Scale));
}
