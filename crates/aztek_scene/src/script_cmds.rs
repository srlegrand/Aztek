//! Commands and procedures rig and scene scripts call: affects, fileInfo, requires, isParentOf, lsType,
//! findRelatedDeformer, getChain, getAllChains, gotoBindPose, setConstraintRestPosition and moveIKtoFK. In the reference
//! the last eight are MEL procedures; here they are commands MEL calls the same way, kept out of `maya.cmds` as the
//! reference keeps them out.

use aztek_ops::{fail, objects, ArgType, Args, CmdResult, Commands};
use serde_json::{json, Value};

use crate::deform::{f, Cmd};
use crate::graph::Graph;
use crate::plug::{flags, NodeId, PlugRef, NO_NODE};
use crate::session::Session;
use crate::transform::Transform;

/// The procedures the reference has only in MEL: `maya.cmds` leaves them out.
pub const MEL_ONLY: &[&str] = &["isParentOf", "lsType", "listTransforms", "findRelatedDeformer", "getChain", "getAllChains", "gotoBindPose", "setConstraintRestPosition", "moveIKtoFK"];

fn find(g: &Graph, name: &str, cmd: &str) -> CmdResult<NodeId> {
    crate::rig::find(g, name, cmd)
}

// ---------------------------------------------------------------------------------------------------------------------
// affects

/// What a node's attribute depends on, or what depends on it. A node here computes all its outputs from all its inputs,
/// so every input affects every output; an attribute that is both affects nothing on the node.
fn affects(s: &mut Session, a: &Args) -> CmdResult {
    let Some(attr) = a.pos_str(0).map(str::to_string) else { fail!("affects: name an attribute") };
    let g = &s.graph;
    let ty = match (a.has("type"), a.pos_str(1)) {
        (true, _) => {
            let t = a.string("type", "");
            crate::node::types::find(&t).ok_or_else(|| aztek_ops::CommandError::new(format!("affects: no node type {t}")))?
        }
        (false, Some(n)) => g.node(find(g, n, "affects")?).unwrap().type_arc().clone(),
        (false, None) => {
            let Some(&n) = g.selection().first() else { fail!("affects: name or select a node, or give -type") };
            g.node(n).unwrap().type_arc().clone()
        }
    };
    // A child (translateX) answers as its compound.
    let child = || ty.attrs.iter().position(|d| d.child_names.iter().zip(&d.child_short).any(|(n, s)| !n.is_empty() && (*n == attr || *s == attr)));
    let Some(i) = ty.attr_index(&attr).or_else(child) else { fail!("affects: {} has no attribute {attr}", ty.name) };
    let def = &ty.attrs[i];
    let is_in = |d: &crate::plug::AttrDef| d.has(flags::INPUT) && !d.has(flags::OUTPUT) && !d.has(flags::HIDDEN);
    let is_out = |d: &crate::plug::AttrDef| d.has(flags::OUTPUT) && !d.has(flags::INPUT);
    let names: Vec<String> = if a.bool("by") {
        if !is_in(def) {
            Vec::new()
        } else {
            ty.attrs.iter().filter(|d| is_out(d)).map(|d| d.name.clone()).collect()
        }
    } else if !is_out(def) {
        Vec::new()
    } else {
        ty.attrs.iter().filter(|d| is_in(d)).map(|d| d.name.clone()).collect()
    };
    Ok(json!(names))
}

// ---------------------------------------------------------------------------------------------------------------------
// fileInfo and requires

/// fileInfo: keyword and value pairs saved with the scene. The application and version Aztek writes itself.
fn file_info(s: &mut Session, a: &Args) -> CmdResult {
    let info = &mut s.extras.file_info;
    if a.has("remove") {
        let k = a.string("remove", "");
        info.retain(|(x, _)| *x != k);
        return Ok(Value::Null);
    }
    if a.bool("query") {
        if a.has("referenceNode") {
            return Ok(json!([]));
        }
        return Ok(match a.pos_str(0) {
            Some(k) => json!(info.iter().filter(|(x, _)| x == k).map(|(_, v)| v.clone()).collect::<Vec<_>>()),
            None => json!(info.iter().flat_map(|(k, v)| [k.clone(), v.clone()]).collect::<Vec<_>>()),
        });
    }
    let (Some(k), Some(v)) = (a.pos_str(0).map(str::to_string), a.pos_str(1).map(str::to_string)) else { fail!("fileInfo: give a keyword and its value") };
    match info.iter_mut().find(|(x, _)| *x == k) {
        Some(e) => e.1 = v,
        None => info.push((k, v)),
    }
    Ok(Value::Null)
}

/// requires, run by hand: the reference reads it from files; a plug-in named here is loaded if it can be.
fn requires(s: &mut Session, a: &Args) -> CmdResult {
    let (Some(product), Some(_)) = (a.pos_str(0).map(str::to_string), a.pos_str(1)) else { fail!("requires: give the product and its version") };
    if product != "maya" && s.commands.find("loadPlugin").is_some() {
        let _ = s.run("loadPlugin", json!({"args": [product], "quiet": true}));
    }
    Ok(Value::Null)
}

// ---------------------------------------------------------------------------------------------------------------------
// The MEL procedures

fn is_parent_of(s: &mut Session, a: &Args) -> CmdResult {
    let (p, c) = (a.pos_str(0).unwrap_or_default().to_string(), a.pos_str(1).unwrap_or_default().to_string());
    let g = &s.graph;
    if p.is_empty() || c.is_empty() {
        return Ok(json!(0));
    }
    let (Ok(Some(p)), Ok(Some(c))) = (g.find(&p), g.find(&c)) else { return Ok(json!(0)) };
    let mut at = g.parent(c);
    while at != NO_NODE {
        if at == p {
            return Ok(json!(1));
        }
        at = g.parent(at);
    }
    Ok(json!(0))
}

/// lsType: the nodes of a type, ended by "<done>" as the reference's procedure ends them.
fn ls_type(s: &mut Session, a: &Args) -> CmdResult {
    let Some(t) = a.pos_str(0).map(str::to_string) else { fail!("lsType: name a type") };
    let mut out: Vec<Value> = s.run("ls", json!({"type": t}))?.as_array().cloned().unwrap_or_default();
    out.push(json!("<done>"));
    Ok(Value::Array(out))
}

/// The shapes a name stands for: a shape, or a transform's shapes.
fn shapes_of(g: &Graph, id: NodeId) -> Vec<NodeId> {
    if g.node(id).is_some_and(|n| n.ty().is_a("transform")) {
        crate::dag::shapes(g, id, false)
    } else {
        vec![id]
    }
}

/// Deformers and skins in a shape's history, nearest the shape first, tweaks included.
fn deformers_of(g: &Graph, shape: NodeId, to_next_shape: bool) -> Vec<NodeId> {
    let mut out = Vec::new();
    for h in crate::history::history_of(g, shape) {
        let Some(n) = g.node(h) else { break };
        let name = n.ty().name.clone();
        if to_next_shape && n.ty().is_a("mesh") {
            break;
        }
        if crate::rig::is_rig_chain_node(&name) || name == crate::history::TWEAK_TYPE {
            out.push(h);
        }
    }
    out
}

fn find_related_deformer(s: &mut Session, a: &Args) -> CmdResult {
    let names = objects(s, a);
    let g = &s.graph;
    let mut out: Vec<String> = Vec::new();
    for n in &names {
        let id = find(g, n, "findRelatedDeformer")?;
        for sh in shapes_of(g, id) {
            for d in deformers_of(g, sh, false) {
                let p = g.path(d);
                if !out.contains(&p) {
                    out.push(p);
                }
            }
        }
    }
    Ok(json!(out))
}

fn get_chain(s: &mut Session, a: &Args) -> CmdResult {
    let Some(n) = a.pos_str(0).map(str::to_string) else { fail!("getChain: name a shape") };
    let g = &s.graph;
    let id = find(g, &n, "getChain")?;
    let out: Vec<String> = shapes_of(g, id).into_iter().flat_map(|sh| deformers_of(g, sh, true)).map(|d| g.path(d)).collect();
    Ok(json!(out))
}

fn get_all_chains(s: &mut Session, _a: &Args) -> CmdResult {
    let g = &s.graph;
    let mut out: Vec<String> = Vec::new();
    for id in g.nodes() {
        let Some(n) = g.node(id) else { continue };
        if !n.ty().is_a("mesh") || n.bool(crate::transform::DagNode::INTERMEDIATE_OBJECT) {
            continue;
        }
        let chain = deformers_of(g, id, true);
        if chain.is_empty() {
            continue;
        }
        let entry = chain.iter().map(|&d| g.path(d)).collect::<Vec<_>>().join(" ");
        if !out.contains(&entry) {
            out.push(entry);
        }
    }
    Ok(json!(out))
}

fn goto_bind_pose(s: &mut Session, _a: &Args) -> CmdResult {
    let sel: Vec<String> = s.graph.selection().iter().map(|&i| s.graph.path(i)).collect();
    if sel.is_empty() {
        fail!("gotoBindPose: select a joint of each skeleton");
    }
    s.run("dagPose", json!({"args": sel, "restore": true, "global": true, "bindPose": true}))?;
    Ok(Value::Null)
}

/// Each selected object's constraints rest where the object is now: its translate and rotate.
fn set_constraint_rest_position(s: &mut Session, _a: &Args) -> CmdResult {
    let sel: Vec<NodeId> = s.graph.selection().to_vec();
    if sel.is_empty() {
        fail!("setConstraintRestPosition: select a constrained object");
    }
    s.graph.evaluate();
    for obj in sel {
        let t = s.graph.value(&PlugRef::new(obj, Transform::TRANSLATE as i32)).map(|v| v.as_vec3()).ok();
        let r = s.graph.value(&PlugRef::new(obj, Transform::ROTATE as i32)).map(|v| v.as_vec3()).ok();
        let constraints: Vec<NodeId> = s
            .graph
            .connections()
            .iter()
            .filter(|(d, sp)| d.node == obj && s.graph.node(sp.node).is_some_and(|n| crate::rig::CONSTRAINT_TYPES.contains(&n.ty().name.as_str())))
            .map(|(_, sp)| sp.node)
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        for c in constraints {
            crate::constraint::set_rest(&mut s.graph, c, t, r)?;
        }
    }
    Ok(Value::Null)
}

/// The selected IK handle, or what drives one, moves to its end joint.
fn move_ik_to_fk(s: &mut Session, _a: &Args) -> CmdResult {
    let sel: Vec<NodeId> = s.graph.selection().to_vec();
    if sel.is_empty() {
        fail!("moveIKtoFK: select an IK handle or what drives one");
    }
    s.graph.evaluate();
    for obj in sel {
        let g = &s.graph;
        let is_handle = |n: NodeId| g.node(n).is_some_and(|x| x.ty().name == "ikHandle");
        let handle = if is_handle(obj) {
            Some(obj)
        } else {
            // A handle whose translate this drives, directly or through a constraint.
            g.connections().iter().find_map(|(d, sp)| {
                if !is_handle(d.node) {
                    return None;
                }
                let src = sp.node;
                let by_constraint = g.node(src).is_some_and(|n| crate::rig::CONSTRAINT_TYPES.contains(&n.ty().name.as_str())) && g.connections().iter().any(|(cd, cs)| cd.node == src && cs.node == obj);
                (src == obj || by_constraint).then_some(d.node)
            })
        };
        let Some(h) = handle else { continue };
        let Some(end) = crate::ik::end_of(g, h) else { continue };
        let at = crate::rig::world(&mut s.graph, end).w_axis.truncate();
        let p = s.graph.path(obj);
        s.run("xform", json!({"args": [p], "worldSpace": true, "translation": [at.x, at.y, at.z]}))?;
    }
    Ok(Value::Null)
}

// ---------------------------------------------------------------------------------------------------------------------
// Sorting strings, and matrices

/// The strings given, as one list: positional arguments, or a list among them.
fn strings_in(a: &Args) -> Vec<String> {
    let mut out = Vec::new();
    for v in &a.positional {
        match v {
            Value::Array(l) => out.extend(l.iter().map(|x| x.as_str().map(str::to_string).unwrap_or_else(|| x.to_string()))),
            Value::String(x) => out.push(x.clone()),
            Value::Null => {}
            x => out.push(x.to_string()),
        }
    }
    out
}

/// Natural order: runs of digits compare as numbers, so file9 comes before file10.
fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let (mut x, mut y) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (x.peek().copied(), y.peek().copied()) {
            (None, None) => return std::cmp::Ordering::Equal,
            (None, _) => return std::cmp::Ordering::Less,
            (_, None) => return std::cmp::Ordering::Greater,
            (Some(p), Some(q)) if p.is_ascii_digit() && q.is_ascii_digit() => {
                let mut na = String::new();
                while let Some(c) = x.peek().copied().filter(char::is_ascii_digit) {
                    na.push(c);
                    x.next();
                }
                let mut nb = String::new();
                while let Some(c) = y.peek().copied().filter(char::is_ascii_digit) {
                    nb.push(c);
                    y.next();
                }
                let (ta, tb) = (na.trim_start_matches('0'), nb.trim_start_matches('0'));
                let o = ta.len().cmp(&tb.len()).then(ta.cmp(tb));
                if o != std::cmp::Ordering::Equal {
                    return o;
                }
            }
            (Some(p), Some(q)) => {
                if p != q {
                    return p.cmp(&q);
                }
                x.next();
                y.next();
            }
        }
    }
}

fn sort_string_array(_s: &mut Session, a: &Args) -> CmdResult {
    let items = strings_in(a);
    let fold = a.bool("ignoreCase");
    let natural = a.string("sortOrder", "natural") != "alphabetic";
    let key = |t: &String| if fold { t.to_lowercase() } else { t.clone() };
    let mut idx: Vec<usize> = (0..items.len()).collect();
    // A stable sort throughout: -stable only says so.
    idx.sort_by(|&i, &j| {
        let (p, q) = (key(&items[i]), key(&items[j]));
        let o = if natural { natural_cmp(&p, &q) } else { p.cmp(&q) };
        if a.bool("reverse") {
            o.reverse()
        } else {
            o
        }
    });
    if a.bool("indexArray") {
        return Ok(json!(idx));
    }
    Ok(json!(idx.into_iter().map(|i| items[i].clone()).collect::<Vec<_>>()))
}

fn sort_case_insensitive(_s: &mut Session, a: &Args) -> CmdResult {
    let mut items = strings_in(a);
    items.sort_by_key(|x| x.to_lowercase());
    Ok(json!(items))
}

fn floats(v: Option<&Value>) -> Vec<f64> {
    match v {
        Some(Value::Array(l)) => l.iter().filter_map(Value::as_f64).collect(),
        Some(x) => x.as_f64().into_iter().collect(),
        None => Vec::new(),
    }
}

/// matrixUtil: a matrix from translation, rotation (degrees, xyz), scale, shear or a quaternion; with a matrix given,
/// -q reads one of them back, -iv and -tp invert and transpose it, and the parts given set its own (add to them with
/// -rt). Matrices are 16 numbers, row by row as the reference writes them.
fn matrix_util(_s: &mut Session, a: &Args) -> CmdResult {
    use glam::{DMat3, DMat4, DQuat, DVec3};
    let given: Vec<f64> = a.positional.iter().flat_map(|v| floats(Some(v))).collect();
    let m = if given.len() >= 16 { DMat4::from_cols_array(&given[..16].try_into().unwrap()) } else { DMat4::IDENTITY };
    let (t, r, s, sh) = crate::transform::decompose_full(&m);
    let v3 = |flag: &str| {
        let f = floats(a.get(flag));
        (f.len() >= 3).then(|| DVec3::new(f[0], f[1], f[2]))
    };
    let to_euler = |r: &DMat3| crate::transform::matrix_to_euler(r, crate::transform::RotateOrder::Xyz) * (180.0 / std::f64::consts::PI);
    if a.bool("query") {
        let out: Vec<f64> = if a.has("translation") {
            t.to_array().to_vec()
        } else if a.has("rotation") {
            to_euler(&r).to_array().to_vec()
        } else if a.has("scale") {
            s.to_array().to_vec()
        } else if a.has("shear") {
            sh.to_array().to_vec()
        } else if a.has("quaternion") {
            let q = DQuat::from_mat3(&r);
            vec![q.x, q.y, q.z, q.w]
        } else if a.bool("inverse") {
            m.inverse().to_cols_array().to_vec()
        } else if a.bool("transpose") {
            m.transpose().to_cols_array().to_vec()
        } else {
            m.to_cols_array().to_vec()
        };
        return Ok(json!(out));
    }
    if a.bool("inverse") {
        return Ok(json!(m.inverse().to_cols_array().to_vec()));
    }
    if a.bool("transpose") {
        return Ok(json!(m.transpose().to_cols_array().to_vec()));
    }
    let rel = a.bool("relative");
    let pick = |now: DVec3, flag: &str| v3(flag).map_or(now, |x| if rel { now + x } else { x });
    let t = pick(t, "translation");
    let s = pick(s, "scale");
    let sh = pick(sh, "shear");
    let mut rot = r;
    if let Some(e) = v3("rotation") {
        let e = e * (std::f64::consts::PI / 180.0);
        let given = DMat3::from_mat4(crate::transform::euler_to_matrix(e, crate::transform::RotateOrder::Xyz));
        rot = if rel { given * r } else { given };
    }
    let q = floats(a.get("quaternion"));
    if q.len() >= 4 {
        let given = DMat3::from_quat(DQuat::from_xyzw(q[0], q[1], q[2], q[3]).normalize());
        rot = if rel { given * r } else { given };
    }
    let out = DMat4::from_translation(t) * DMat4::from_mat3(rot) * crate::transform::shear_matrix(sh) * DMat4::from_scale(s);
    Ok(json!(out.to_cols_array().to_vec()))
}

pub fn register(c: &mut Commands<Session>) {
    c.register(
        Cmd::new("sortStringArray", "Sort strings: natural order (file9 before file10) unless -ord alphabetic; -ic ignores case, -rev reverses, -ia returns the sorted indices.", sort_string_array)
            .positional("the strings")
            .flag(f("ignoreCase", "ic", ArgType::Bool, "Ignore case."))
            .flag(f("indexArray", "ia", ArgType::Bool, "Return the sorted indices instead."))
            .flag(f("reverse", "rev", ArgType::Bool, "Sort the other way."))
            .flag(f("sortOrder", "ord", ArgType::String, "natural (the default) or alphabetic."))
            .flag(f("stable", "st", ArgType::Bool, "Keep equal items in their order (they always are here)."))
            .undoable(false),
    );
    c.register(Cmd::new("sortCaseInsensitive", "Sort strings regardless of case.", sort_case_insensitive).positional("the strings").undoable(false));
    c.register(
        Cmd::new("matrixUtil", "Make a matrix from -t, -r (degrees), -s, -sh or -qt; with a matrix given: -q one of them, -iv or -tp the matrix, or set its parts (-rt adds to them). 16 numbers, row by row.", matrix_util)
            .positional("a matrix: 16 numbers")
            .flag(f("translation", "t", ArgType::Any, "Translation.").multi(3))
            .flag(f("rotation", "r", ArgType::Any, "Rotation, degrees, xyz.").multi(3))
            .flag(f("scale", "s", ArgType::Any, "Scale.").multi(3))
            .flag(f("shear", "sh", ArgType::Any, "Shear.").multi(3))
            .flag(f("quaternion", "qt", ArgType::Any, "Rotation as a quaternion x y z w.").multi(4))
            .flag(f("relative", "rt", ArgType::Bool, "Add to the matrix's own parts."))
            .flag(f("inverse", "iv", ArgType::Bool, "The inverse."))
            .flag(f("transpose", "tp", ArgType::Bool, "The transpose."))
            .flag(f("query", "q", ArgType::Bool, "Query."))
            .flag(f("edit", "e", ArgType::Bool, "Edit."))
            .undoable(false),
    );
    c.register(
        Cmd::new("affects", "The attributes of a node, or of a node type with -type, that affect the one named; -by those it affects. A node here computes every output from every input.", affects)
            .positional("an attribute, then a node (the selection when none)")
            .flag(f("by", "by", ArgType::Bool, "The attributes it affects."))
            .flag(f("type", "t", ArgType::String, "A node type instead of a node."))
            .undoable(false),
    );
    c.register(
        Cmd::new("fileInfo", "Keyword and value pairs saved with the scene: fileInfo key value sets one, -rm removes one, -q lists them (key value...) or gives one key's value.", file_info)
            .positional("keyword, value")
            .flag(f("remove", "rm", ArgType::String, "Remove this keyword."))
            .flag(f("referenceNode", "rfn", ArgType::String, "With -q: the fileInfo of a referenced file."))
            .flag(f("query", "q", ArgType::Bool, "Query."))
            .undoable(false),
    );
    c.register(
        Cmd::new("requires", "What a scene needs: the product and its version. A plug-in named loads if it can; the file's own lines are read when it opens.", requires)
            .positional("product, version")
            .flag(f("nodeType", "nt", ArgType::String, "A node type the plug-in makes.").multi(1))
            .flag(f("dataType", "dt", ArgType::String, "A data type the plug-in makes.").multi(1)),
    );
    c.register(Cmd::new("isParentOf", "Whether the first node is a DAG parent (at any depth) of the second.", is_parent_of).positional("parent, child").undoable(false));
    c.register(Cmd::new("lsType", "The nodes of a type, ended by \"<done>\".", ls_type).positional("a node type").undoable(false));
    c.register(Cmd::new("findRelatedDeformer", "The deformers and skins acting on a shape (or a transform's shapes), nearest the shape first.", find_related_deformer).positional("an object").undoable(false));
    c.register(Cmd::new("getChain", "The deformers between a shape and the shape upstream of them, nearest the shape first.", get_chain).positional("a shape").undoable(false));
    c.register(Cmd::new("getAllChains", "Each deformer chain in the scene, its deformers' names separated by spaces.", get_all_chains).undoable(false));
    c.register(Cmd::new("gotoBindPose", "The selected skeletons go to their bind pose.", goto_bind_pose));
    c.register(Cmd::new("setConstraintRestPosition", "The selected objects' constraints rest where the objects are now.", set_constraint_rest_position));
    c.register(Cmd::new("moveIKtoFK", "The selected IK handle, or what drives one, moves to the handle's end joint.", move_ik_to_fk));
}
