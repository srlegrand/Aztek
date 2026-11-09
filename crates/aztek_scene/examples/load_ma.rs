//! Load a .ma file with no UI and report timing and warnings: `cargo run --release -p aztek_scene --example load_ma -- file.ma`.

use aztek_scene::plug::{PlugRef, PlugValue};
use aztek_scene::skin::SkinBind;
use aztek_scene::{ma_file, Session};
use std::collections::BTreeMap;
use std::time::Instant;

fn main() {
    let path = std::env::args().nth(1).expect("usage: load_ma file.ma");
    let mut s = Session::new();
    let t = Instant::now();
    let warnings = ma_file::open_scene(&mut s, &path).expect("open");
    eprintln!("open {:.2}s, {} warnings", t.elapsed().as_secs_f64(), warnings.len());
    // AZTEK_SET="node.attr=value,...": values set before the checks, to find which part of a rig is wrong.
    for item in std::env::var("AZTEK_SET").unwrap_or_default().split(',').filter(|x| !x.is_empty()) {
        if let Some((plug, v)) = item.split_once('=') {
            let r = s.run("setAttr", serde_json::json!({"args": [plug, v.parse::<f64>().unwrap_or(0.0)]}));
            eprintln!("setAttr {plug} {v}: {}", if r.is_ok() { "ok".to_string() } else { format!("{r:?}") });
        }
    }
    let t = Instant::now();
    s.graph.evaluate();
    // AZTEK_GET="node.attr,...": values printed after evaluating, then the checks are skipped.
    let gets = std::env::var("AZTEK_GET").unwrap_or_default();
    if !gets.is_empty() {
        for plug in gets.split(',') {
            println!("{plug} = {}", s.run("getAttr", serde_json::json!({"args": [plug]})).map(|v| v.to_string()).unwrap_or_else(|e| e.to_string()));
        }
        return;
    }
    eprintln!("evaluate {:.2}s", t.elapsed().as_secs_f64());
    let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
    for w in &warnings {
        let key: String = w.chars().map(|c| if c.is_ascii_digit() { '#' } else { c }).collect();
        *kinds.entry(key).or_default() += 1;
    }
    for (k, n) in kinds {
        println!("{n:6} {k}");
    }
    let stuck = s.graph.stuck_nodes();
    println!("stuck after evaluate: {}", stuck.len());
    let mut names: Vec<String> = stuck.iter().filter_map(|&i| s.graph.node(i)).map(|n| format!("{} ({})", n.name(), n.ty().name)).collect();
    names.sort();
    for n in names.iter().take(40) {
        println!("  {n}");
    }
    bind_check(&mut s);
    stored_check(&mut s, &path);
    last_rotation_check(&mut s, &path);
    rest_check(&mut s);
    for name in std::env::args().skip(2) {
        chain(&mut s, &name);
    }
}

/// A rig at rest has every influence where it was bound: its world matrix times its bindPreMatrix is the identity. Reports the
/// influences that are not, worst first.
fn bind_check(s: &mut Session) {
    let g = &mut s.graph;
    g.evaluate();
    let mut worst: BTreeMap<String, (f64, f64)> = BTreeMap::new();
    let (mut total, mut skins) = (0, 0);
    for id in g.nodes() {
        let Some(n) = g.node(id) else { continue };
        if n.ty().name != "skinCluster" {
            continue;
        }
        let t = n.type_arc().clone();
        let (ma, bd) = (t.attr_index("matrix").unwrap(), t.attr_index("bindData").unwrap());
        let PlugValue::Data(Some(d)) = n.data().value(bd).clone() else { continue };
        let Some(b) = d.as_any().downcast_ref::<SkinBind>().cloned() else { continue };
        skins += 1;
        for (i, pre) in b.bind_pre.iter().enumerate() {
            let p = PlugRef::element(id, ma as i32, i as i32);
            let Some(src) = g.source(&p) else { continue };
            let Ok(PlugValue::Matrix(w)) = g.value(&p) else { continue };
            total += 1;
            let m = w * *pre;
            let move_err = m.w_axis.truncate().length();
            let rot_err = (m.x_axis.truncate() - glam::DVec3::X).length().max((m.y_axis.truncate() - glam::DVec3::Y).length());
            let name = g.node(src.node).map_or(String::new(), |n| n.name().to_string());
            if std::env::var("BIND_SHOW").is_ok_and(|v| v.split(',').any(|x| x == name)) {
                let want = pre.inverse();
                let show = |m: &glam::DMat4| format!("x{:.2} y{:.2} z{:.2} w{:.2}", m.x_axis.truncate(), m.y_axis.truncate(), m.z_axis.truncate(), m.w_axis.truncate());
                println!("{name}\n   want {}\n   got  {}", show(&want), show(&w));
            }
            let e = worst.entry(name).or_insert((0.0, 0.0));
            e.0 = e.0.max(move_err);
            e.1 = e.1.max(rot_err);
        }
    }
    let mut bad: Vec<_> = worst.into_iter().filter(|(_, e)| e.0 > 0.01 || e.1 > 0.01).collect();
    bad.sort_by(|a, b| b.1 .0.total_cmp(&a.1 .0));
    println!("bind check: {skins} skins, {total} influences, {} joints off", bad.len());
    let limit = std::env::var("BIND_ALL").map_or(60, |_| usize::MAX);
    for (name, (m, r)) in bad.iter().take(limit) {
        println!("  {m:10.3} {r:7.3}  {name}");
    }
    // Where it starts: wrong influences whose ancestors are all right (or aren't influences).
    let off: std::collections::HashSet<&str> = bad.iter().map(|(n, _)| n.as_str()).collect();
    let mut roots = Vec::new();
    for (name, e) in &bad {
        let Ok(Some(id)) = g.find(name) else { continue };
        let mut up = g.parent(id);
        let mut clean = true;
        while up != aztek_scene::plug::NO_NODE {
            if g.node(up).is_some_and(|n| off.contains(n.name())) {
                clean = false;
                break;
            }
            up = g.parent(up);
        }
        if clean {
            roots.push((name.clone(), *e));
        }
    }
    println!("first wrong: {}", roots.len());
    for (name, (m, r)) in roots.iter().take(limit) {
        println!("  {m:10.3} {r:7.3}  {name}");
    }
}

/// A node and everything above it: type, world position, local translate and rotate, and what drives each.
fn chain(s: &mut Session, name: &str) {
    let g = &mut s.graph;
    let Ok(Some(mut id)) = g.find(name) else {
        println!("{name}: not found");
        return;
    };
    println!("{name}:");
    loop {
        let Some(n) = g.node(id) else { break };
        let t = n.type_arc().clone();
        let wm = t.attr_index("worldMatrix").and_then(|a| match n.data().value(a) {
            PlugValue::Matrix(m) => Some(m.w_axis.truncate()),
            _ => None,
        });
        let tr = t.attr_index("translate").map(|a| n.data().vec3(a));
        let ro = t.attr_index("rotate").map(|a| n.data().vec3(a));
        println!("  {:32} {:14} w={:?} t={:?} r={:?}", n.name(), t.name, wm.map(|v| (v * 100.0).round() / 100.0), tr.map(|v| (v * 100.0).round() / 100.0), ro.map(|v| (v * 100.0).round() / 100.0));
        for (d, src) in g.connections() {
            if d.node == id {
                let sn = g.node(src.node).map_or(String::new(), |x| format!("{}.{} ({})", x.name(), x.ty().attrs[src.attr as usize].name, x.ty().name));
                println!("      {:24} <- {sn}", t.attrs[d.attr as usize].name);
            }
        }
        let p = g.parent(id);
        if p == aztek_scene::plug::NO_NODE {
            break;
        }
        id = p;
    }
}

/// The reference writes a channel's value when it saves, and leaves out zeros, so a transform's stored .t and .r say where it was in
/// The reference. Reports the nodes that came out somewhere else whose inputs and parents are all right: those are the causes, the rest
/// follow from them.
fn stored_check(s: &mut Session, path: &str) {
    use std::collections::{HashMap, HashSet};
    let text = std::fs::read(path).map(|b| String::from_utf8_lossy(&b).to_string()).unwrap_or_default();
    let mut stored: Vec<(String, glam::DVec3, glam::DVec3)> = Vec::new();
    let mut written: HashSet<(String, i32)> = HashSet::new();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("createNode ") {
            if rest.starts_with("transform ") || rest.starts_with("joint ") {
                stored.push((rest.split('"').nth(1).unwrap_or("").to_string(), glam::DVec3::ZERO, glam::DVec3::ZERO));
            } else {
                stored.push((String::new(), glam::DVec3::ZERO, glam::DVec3::ZERO));
            }
            continue;
        }
        let Some(last) = stored.last_mut() else { continue };
        if last.0.is_empty() {
            continue;
        }
        let l = line.trim_start();
        let t = l.starts_with("setAttr \".t\" -type \"double3\"");
        let r = l.starts_with("setAttr \".r\" -type \"double3\"");
        if t || r {
            written.insert((last.0.clone(), if t { 0 } else { 1 }));
            // Long values wrap onto the next line in the file; those channels are skipped by having too few numbers.
            let v: Vec<f64> = l.split_whitespace().filter_map(|x| x.parse().ok()).collect();
            if v.len() == 3 {
                let x = glam::DVec3::new(v[0], v[1], v[2]);
                if t {
                    last.1 = x;
                } else {
                    last.2 = x;
                }
            } else if t {
                last.1 = glam::DVec3::splat(f64::NAN);
            } else {
                last.2 = glam::DVec3::splat(f64::NAN);
            }
        }
    }
    let g = &mut s.graph;
    g.evaluate();
    let connected: HashSet<(aztek_scene::plug::NodeId, i32)> = g.connections().iter().map(|(d, _)| (d.node, d.attr)).collect();
    let mut bad: HashMap<aztek_scene::plug::NodeId, (String, f64, f64)> = HashMap::new();
    let mut checked = 0;
    for (name, t, r) in &stored {
        if name.is_empty() {
            continue;
        }
        let Ok(Some(id)) = g.find(name) else { continue };
        let Some(n) = g.node(id) else { continue };
        let (Some(ta), Some(ra)) = (n.ty().attr_index("translate"), n.ty().attr_index("rotate")) else { continue };
        checked += 1;
        // The reference leaves a driven channel out when it saves. Absent and driven means it says nothing.
        let driven = |attr: usize| connected.contains(&(id, attr as i32));
        let t = &if !written.contains(&(name.clone(), 0)) && driven(ta) { glam::DVec3::splat(f64::NAN) } else { *t };
        let r = &if !written.contains(&(name.clone(), 1)) && driven(ra) { glam::DVec3::splat(f64::NAN) } else { *r };
        let et = if t.x.is_nan() { 0.0 } else { (n.data().vec3(ta) - *t).abs().max_element() };
        // Rotations can be the same turn written differently: compare the matrices.
        let er = if r.x.is_nan() {
            0.0
        } else {
            let (a, b) = (n.data().vec3(ra), *r);
            let m = |v: glam::DVec3| glam::DQuat::from_euler(glam::EulerRot::ZYX, v.z.to_radians(), v.y.to_radians(), v.x.to_radians());
            m(a).dot(m(b)).abs().min(1.0).acos().to_degrees() * 2.0
        };
        if et > 0.01 || er > 0.05 {
            bad.insert(id, (name.clone(), et, er));
        }
    }
    let mut ups: HashMap<aztek_scene::plug::NodeId, Vec<aztek_scene::plug::NodeId>> = HashMap::new();
    for (d, src) in g.connections() {
        ups.entry(d.node).or_default().push(src.node);
    }
    let mut causes: Vec<&(String, f64, f64)> = Vec::new();
    for (&id, info) in &bad {
        let mut stack = ups.get(&id).cloned().unwrap_or_default();
        let p = g.parent(id);
        if p != aztek_scene::plug::NO_NODE {
            stack.push(p);
        }
        let mut seen = HashSet::new();
        let mut clean = true;
        while let Some(x) = stack.pop() {
            if x == id || !seen.insert(x) {
                continue;
            }
            if bad.contains_key(&x) {
                clean = false;
                break;
            }
            stack.extend(ups.get(&x).into_iter().flatten().copied());
            let p = g.parent(x);
            if p != aztek_scene::plug::NO_NODE {
                stack.push(p);
            }
        }
        if clean {
            causes.push(info);
        }
    }
    causes.sort_by(|a, b| a.0.cmp(&b.0));
    println!("stored check: {checked} nodes, {} differ, {} with nothing wrong upstream", bad.len(), causes.len());
    for (name, t, r) in causes.iter().take(usize::MAX) {
        println!("  t {t:9.3} r {r:9.3}  {name}");
    }
}


/// The reference stores each rotating constraint's last result in `.lr`, so these are posed results it computed. Reports the ones that differ.
fn last_rotation_check(s: &mut Session, path: &str) {
    use aztek_scene::transform::{euler_to_matrix, RotateOrder};
    let text = std::fs::read(path).map(|b| String::from_utf8_lossy(&b).to_string()).unwrap_or_default();
    let mut current = String::new();
    let mut stored: Vec<(String, glam::DVec3)> = Vec::new();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("createNode ") {
            current = if rest.split_whitespace().next().is_some_and(|t| t.ends_with("Constraint")) { rest.split('"').nth(1).unwrap_or("").to_string() } else { String::new() };
            continue;
        }
        let l = line.trim_start();
        if !current.is_empty() && l.starts_with("setAttr \".lr\"") {
            let v: Vec<f64> = l.split_whitespace().filter_map(|x| x.parse().ok()).collect();
            if v.len() == 3 {
                stored.push((current.clone(), glam::DVec3::new(v[0], v[1], v[2])));
            }
        }
    }
    let g = &mut s.graph;
    g.evaluate();
    let mut bad = Vec::new();
    let mut checked = 0;
    for (name, lr) in &stored {
        let Ok(Some(id)) = g.find(name) else { continue };
        let Some(n) = g.node(id) else { continue };
        let (Some(out), Some(ro)) = (n.ty().attr_index("constraintRotate"), n.ty().attr_index("constraintRotateOrder")) else { continue };
        checked += 1;
        let order = RotateOrder::from_index(n.data().int(ro));
        let q = |v: glam::DVec3| glam::DQuat::from_mat4(&euler_to_matrix(v, order));
        let got = n.data().vec3(out);
        let angle = q(got).dot(q(*lr)).abs().min(1.0).acos().to_degrees() * 2.0;
        if angle > 0.05 {
            bad.push((angle, name.clone(), got, *lr));
        }
    }
    println!("last rotation check: {checked} constraints, {} differ", bad.len());
    let limit = std::env::var("BIND_ALL").map_or(40, |_| usize::MAX);
    for (angle, name, got, lr) in bad.iter().take(limit) {
        println!("  {angle:8.3}  {name}  got {:.2} stored {:.2}", got, lr);
    }
}

/// A rig at rest deforms nothing: every mesh sits on its original, the sibling intermediate shape. Reports how far each is off.
fn rest_check(s: &mut Session) {
    use aztek_scene::mesh_nodes::MeshNode;
    use aztek_scene::transform::DagNode;
    let g = &mut s.graph;
    g.evaluate();
    let mut rows = Vec::new();
    for id in g.nodes() {
        let Some(n) = g.node(id) else { continue };
        if n.ty().name != "mesh" || n.data().bool(DagNode::INTERMEDIATE_OBJECT) {
            continue;
        }
        let name = n.name().to_string();
        let Some(out) = n.data().value(MeshNode::OUT_MESH).as_mesh().cloned() else { continue };
        let parent = g.parent(id);
        let orig = g.children(parent).iter().copied().find(|&k| g.node(k).is_some_and(|x| x.ty().name == "mesh" && x.data().bool(DagNode::INTERMEDIATE_OBJECT)));
        let Some(orig) = orig.and_then(|k| g.node(k)).and_then(|x| x.data().value(MeshNode::OUT_MESH).as_mesh().cloned()) else { continue };
        if orig.point_capacity() != out.point_capacity() {
            rows.push((f64::INFINITY, 0.0, name));
            continue;
        }
        let d: Vec<f64> = out.positions().iter().zip(orig.positions()).map(|(a, b)| (*a - *b).length()).collect();
        let max = d.iter().copied().fold(0.0, f64::max);
        let avg = d.iter().sum::<f64>() / d.len().max(1) as f64;
        rows.push((max, avg, name));
    }
    rows.sort_by(|a, b| b.0.total_cmp(&a.0));
    println!("rest check: {} deformed meshes", rows.len());
    for (max, avg, name) in rows {
        println!("  max {max:10.3} avg {avg:9.3}  {name}");
    }
}
