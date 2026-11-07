//! Scene snapshots for the panels, as JSON: outliner, channel box, attribute editor, layers, state.

use aztek_scene::plug::PlugRef;
use aztek_scene::components::{self, ComponentMode};
use aztek_scene::graph::Graph;
use aztek_scene::plug::{flags, AttrDef, NodeId, PlugType, PlugValue, NO_NODE};
use aztek_scene::transform::DagNode;
use serde_json::{json, Value};

use crate::core::Core;

/// camelCase to "Camel Case", as the reference's nice names.
pub fn nice(name: &str) -> String {
    let mut out = String::new();
    for (i, ch) in name.chars().enumerate() {
        if i == 0 {
            out.extend(ch.to_uppercase());
        } else if ch.is_uppercase() && !out.ends_with(' ') {
            out.push(' ');
            out.push(ch);
        } else {
            out.push(ch);
        }
    }
    out
}

fn attr_nice(a: &AttrDef) -> String {
    if !a.nice_name.is_empty() && a.nice_name != "json" {
        a.nice_name.clone()
    } else {
        nice(&a.name)
    }
}

fn value_json(v: &PlugValue) -> Value {
    match v {
        PlugValue::Bool(b) => json!(b),
        PlugValue::Int(i) => json!(i),
        PlugValue::Float(f) => json!(f),
        PlugValue::Vec3(v) => json!([v.x, v.y, v.z]),
        PlugValue::String(s) => json!(s),
        PlugValue::Matrix(m) => json!(m.to_cols_array()),
        _ => Value::Null,
    }
}

fn type_name(t: PlugType) -> &'static str {
    match t {
        PlugType::Bool => "bool",
        PlugType::Int => "int",
        PlugType::Float => "float",
        PlugType::Vec3 => "vec3",
        PlugType::Matrix => "matrix",
        PlugType::String => "string",
        PlugType::Enum => "enum",
        PlugType::Mesh => "mesh",
        PlugType::Data => "data",
    }
}

fn is_shape(g: &Graph, id: NodeId) -> bool {
    g.node(id).is_some_and(|n| n.ty().is_a("shape"))
}

/// A transform with shapes reads as what its shapes are, since those are hidden in the usual Outliner view.
fn outliner_display(g: &Graph, id: NodeId) -> i32 {
    use aztek_scene::draw_style::{display_kind, DisplayKind};
    let own = display_kind(g, id);
    if own != DisplayKind::Normal || !g.node(id).is_some_and(|n| n.ty().is_a("transform")) {
        return own as i32;
    }
    let kinds: Vec<DisplayKind> = aztek_scene::dag::shapes(g, id, false).iter().map(|&s| display_kind(g, s)).collect();
    match kinds.first() {
        Some(&k) if kinds.iter().all(|&x| x == k) => k as i32,
        _ => own as i32,
    }
}

fn outliner_node(g: &Graph, id: NodeId, shapes: bool, show_intermediate: bool, sel: &[NodeId]) -> Option<Value> {
    let n = g.node(id)?;
    let shape = n.ty().is_a("shape");
    let intermediate = shape && n.bool(DagNode::INTERMEDIATE_OBJECT);
    if (shape && !shapes) || (intermediate && !show_intermediate) {
        return None;
    }
    if n.ty().dag && n.bool(DagNode::HIDDEN_IN_OUTLINER) {
        return None;
    }
    let kids: Vec<Value> = g.children(id).iter().filter_map(|&c| outliner_node(g, c, shapes, show_intermediate, sel)).collect();
    // What the icon shows: a transform over a mesh is a mesh to the user.
    let icon = if n.ty().is_a("joint") {
        "joint".into()
    } else if n.ty().is_a("ikHandle") {
        "ik".into()
    } else if n.ty().is_a("deformHandle") {
        "deformHandle".into()
    } else if n.ty().is_a("transform") {
        match aztek_scene::dag::shapes(g, id, false).first().and_then(|&s| g.node(s)) {
            Some(s) => s.ty().name.clone(),
            None if g.children(id).is_empty() => "transform".into(),
            None => "group".into(),
        }
    } else {
        n.ty().name.clone()
    };
    Some(json!({
        "name": n.name(),
        "path": g.path(id),
        "type": n.ty().name,
        "icon": icon,
        "usdShape": usd_shape_of(g, id),
        "referenced": n.reference_id() != 0,
        "selected": sel.contains(&id),
        "visible": !n.ty().dag || n.bool(DagNode::VISIBILITY),
        "intermediate": intermediate,
        // 0 normal, 1 template, 2 reference, inherited; the name's colour when useOutlinerColor is on.
        "display": outliner_display(g, id),
        "outlinerColor": (n.ty().dag && n.bool(DagNode::USE_OUTLINER_COLOR)).then(|| n.data().vec3(DagNode::OUTLINER_COLOR).to_array()),
        "children": kids,
    }))
}

/// The proxy shape a node is or holds: the outliner lists prims under it.
fn usd_shape_of(g: &Graph, id: NodeId) -> Value {
    let shape = if aztek_usd::stages::is_proxy(g, id) { Some(id) } else { g.children(id).iter().copied().find(|&c| aztek_usd::stages::is_proxy(g, c)) };
    shape.map_or(Value::Null, |s| json!(g.full_path(s)))
}

/// `arg` holds the words shapes and intermediate. Deformers' sets and original shapes show only with intermediate, as
/// The reference's Display > Intermediate Objects does.
pub fn outliner(core: &Core, arg: &str) -> Value {
    let show_intermediate = arg.contains("intermediate");
    let shapes = arg.contains("shapes") || show_intermediate;
    let g = &core.s.graph;
    let sel = g.selection().to_vec();
    let mut out = Vec::new();
    for &r in g.roots() {
        if g.node(r).is_some_and(|n| n.ty().dag) {
            if let Some(v) = outliner_node(g, r, shapes, show_intermediate, &sel) {
                out.push(v);
            }
        }
    }
    // Sets and layers at the end, as the reference lists its default sets.
    for id in g.nodes() {
        let Some(n) = g.node(id) else { continue };
        // Shading groups stay in the Hypershade unless everything is asked for, as the reference's outliner leaves them out.
        if n.ty().is_a("objectSet") && (show_intermediate || (aztek_scene::deform_sets::deformer_of_set(g, id).is_none() && !n.ty().is_a(aztek_scene::shading::SG_TYPE))) {
            out.push(json!({"name": n.name(), "path": g.path(id), "type": n.ty().name, "icon": "set", "selected": sel.contains(&id), "visible": true, "intermediate": false, "children": []}));
        }
    }
    Value::Array(out)
}

/// How the channel box colours a channel: keyed, keyed on this frame, driven by another connection, locked, or "".
pub(crate) fn channel_state(g: &Graph, p: &PlugRef) -> &'static str {
    if g.source(p).is_some_and(|x| aztek_scene::anim_filter::is_mute(g, x.node)) {
        return "muted";
    }
    if let Some(c) = aztek_scene::anim::curve_of(g, p) {
        let now = aztek_scene::anim::frame(g);
        if g.is_held(p) {
            return "changed";
        }
        return if aztek_scene::anim::keys_of(g, c).keys.iter().any(|k| (k.t - now).abs() < 1e-6) { "keyOnFrame" } else { "keyed" };
    }
    if !aztek_scene::anim_layers::chain(g, p).is_empty() {
        return "layered";
    }
    if g.source(p).is_some() || (p.component >= 0 && g.source(&p.whole()).is_some()) {
        return "connected";
    }
    if g.is_locked(p) {
        return "locked";
    }
    ""
}

/// The animation curves on the selected objects, for the graph editor: keys with their actual slopes, and the curve
/// sampled over [start, end].
pub fn anim_curves(core: &Core, arg: &str) -> Value {
    use aztek_scene::anim;
    let g = &core.s.graph;
    let mut it = arg.split(':').filter_map(|x| x.parse::<f64>().ok());
    let (start, end, step) = (it.next().unwrap_or(0.0), it.next().unwrap_or(100.0), it.next().unwrap_or(1.0).max(1e-3));
    let mut out = Vec::new();
    let mut nodes: Vec<NodeId> = Vec::new();
    for &id in g.selection() {
        let id = if g.node(id).is_some_and(|n| n.ty().dag && !n.ty().is_a("transform")) { aztek_scene::dag::transform_of(g, id) } else { id };
        if !nodes.contains(&id) {
            nodes.push(id);
        }
    }
    for id in nodes {
        let mut curves = Vec::new();
        // A channel in layers shows a curve per layer, by the layer editor's Show filter.
        let mut shown: Vec<(PlugRef, NodeId, Option<String>)> = Vec::new();
        for (p, c) in anim::curves_on(g, id) {
            if aztek_scene::anim_layers::chain(g, &p).is_empty() {
                shown.push((p, c, None));
            }
        }
        for p in aztek_scene::anim_layers::layered_plugs(g, id) {
            for (l, x) in aztek_scene::anim_layers::shown_curves(g, &p) {
                shown.push((p, x, g.node(l).map(|n| n.name().to_string())));
            }
        }
        for (p, c, layer) in shown {
            let k = anim::keys_of(g, c);
            let (pre, post) = (g.node(c).map(|n| n.data().int(anim::PRE)).unwrap_or(0), g.node(c).map(|n| n.data().int(anim::POST)).unwrap_or(0));
            let inf = |i: i64| match i {
                1 => anim::Infinity::Linear,
                3 => anim::Infinity::Cycle,
                4 => anim::Infinity::CycleRelative,
                5 => anim::Infinity::Oscillate,
                _ => anim::Infinity::Constant,
            };
            let keys: Vec<Value> = (0..k.keys.len())
                .map(|i| {
                    let (si, so) = k.slopes(i);
                    let key = &k.keys[i];
                    // The handles, in frames and value: a third of the tangents.
                    let f = anim::fps() / 3.0;
                    let (ix, iy) = k.tangent_xy(i, false);
                    let (ox, oy) = k.tangent_xy(i, true);
                    json!([key.t, key.v, si, so, key.itt.name(), key.ott.name(), key.breakdown, ix * f, iy / 3.0, ox * f, oy / 3.0, key.tan_lock, key.weight_lock])
                })
                .collect();
            let mut samples = Vec::new();
            let mut t = start;
            while t <= end + 1e-9 {
                samples.push(json!([t, k.eval(t, inf(pre), inf(post))]));
                t += step;
            }
            let a = g.attr(&p);
            let channel = if p.component >= 0 { a.child_names[p.component as usize].clone() } else { a.name.clone() };
            let channel = match &layer {
                Some(l) => format!("{channel} ({l})"),
                None => channel,
            };
            // What the buffer holds, sampled the same way.
            let mut buffer = Vec::new();
            if let Some(b) = core.s.buffer_curves.get(&c) {
                let mut held = b.clone();
                held.weighted = k.weighted;
                let mut t = start;
                while !held.keys.is_empty() && t <= end + 1e-9 {
                    buffer.push(json!([t, held.eval(t, inf(pre), inf(post))]));
                    t += step;
                }
            }
            curves.push(json!({"curve": g.path(c), "channel": channel, "plug": g.plug_name(&p), "component": p.component, "keys": keys, "samples": samples, "weighted": k.weighted, "buffer": buffer}));
        }
        if !curves.is_empty() {
            out.push(json!({"node": g.path(id), "curves": curves}));
        }
    }
    json!({"time": anim::frame(g), "objects": out})
}

/// Channels of a node as the channel box lists them: keyable scalar attributes, vectors split per axis.
fn channels(g: &Graph, id: NodeId, all_inputs: bool) -> Vec<Value> {
    let Some(n) = g.node(id) else { return Vec::new() };
    let path = g.path(id);
    let ty = n.ty();
    let mut out = Vec::new();
    for (i, a) in ty.attrs.iter().enumerate() {
        if a.flags & (flags::HIDDEN | flags::OUTPUT) != 0 {
            continue;
        }
        // A multi with aliases shows each element under its alias: a blend shape's target weights.
        if a.is_array() {
            for e in g.elements(&PlugRef::new(id, i as i32)) {
                let Some(alias) = g.alias_of(&e) else { continue };
                out.push(json!({
                    "attr": alias,
                    "plug": format!("{path}.{alias}"),
                    "nice": alias,
                    "type": "float",
                    "value": n.data().element(i, e.element).map_or(0.0, |v| v.as_float()),
                    "enum": [],
                    "connected": g.source(&e).is_some(),
                    "state": channel_state(g, &e),
                }));
            }
            continue;
        }
        // Keyable as this node has it (setAttr -k), or shown by setAttr -cb.
        let whole = PlugRef::new(id, i as i32);
        let vector = a.ty == PlugType::Vec3;
        let shown = |p: &PlugRef| aztek_scene::anim::is_keyable(g, p) || g.channel_state(p).1;
        let keyable = !vector && shown(&whole);
        if !vector && !keyable && !(all_inputs && matches!(a.ty, PlugType::Bool | PlugType::Int | PlugType::Float | PlugType::Enum | PlugType::String)) {
            continue;
        }
        let v = n.value(i);
        match (a.ty, v) {
            (PlugType::Vec3, PlugValue::Vec3(v)) => {
                for k in 0..3 {
                    if !shown(&PlugRef::component(id, i as i32, k as i32)) {
                        continue;
                    }
                    let child = if a.child_names[k].is_empty() { format!("{}{}", a.name, ["X", "Y", "Z"][k]) } else { a.child_names[k].clone() };
                    out.push(json!({
                        "attr": child,
                        "plug": format!("{path}.{child}"),
                        "nice": format!("{} {}", attr_nice(a), if a.has(flags::COLOR) { ["R", "G", "B"][k] } else { ["X", "Y", "Z"][k] }),
                        "type": "float",
                        "value": v[k],
                        "connected": g.source(&aztek_scene::plug::PlugRef::new(id, i as i32)).is_some(),
                        "state": channel_state(g, &PlugRef::component(id, i as i32, k as i32)),
                    }));
                }
            }
            (t, v) if matches!(t, PlugType::Bool | PlugType::Int | PlugType::Float | PlugType::Enum | PlugType::String) => {
                out.push(json!({
                    "attr": a.name,
                    "plug": format!("{path}.{}", a.name),
                    "nice": attr_nice(a),
                    "type": type_name(t),
                    "value": value_json(v),
                    "enum": a.enum_names,
                    "connected": g.source(&aztek_scene::plug::PlugRef::new(id, i as i32)).is_some(),
                    "state": channel_state(g, &PlugRef::new(id, i as i32)),
                }));
            }
            _ => {}
        }
    }
    out
}

pub fn channel_box(core: &Core) -> Value {
    let g = &core.s.graph;
    let Some(&lead) = g.selection().last() else { return Value::Null };
    let Some(n) = g.node(lead) else { return Value::Null };
    let is_xf = n.ty().is_a("transform");
    let shapes: Vec<NodeId> = if is_xf { aztek_scene::dag::shapes(g, lead, false) } else if is_shape(g, lead) { vec![lead] } else { vec![] };
    let mut inputs = Vec::new();
    for &sh in &shapes {
        if g.node(sh).is_some_and(|n| n.ty().is_a("mesh")) {
            for h in aztek_scene::history::history_of(g, sh) {
                let Some(hn) = g.node(h) else { continue };
                if hn.ty().dag {
                    continue;
                }
                inputs.push(json!({"name": hn.name(), "path": g.path(h), "type": hn.ty().name, "channels": channels(g, h, true)}));
            }
        }
    }
    let shape_list: Vec<Value> = shapes
        .iter()
        .filter_map(|&s| g.node(s).map(|n| json!({"name": n.name(), "path": g.path(s), "type": n.ty().name, "channels": channels(g, s, false)})))
        .collect();
    json!({
        "name": n.name(),
        "path": g.path(lead),
        "type": n.ty().name,
        "channels": if is_xf || !is_shape(g, lead) { channels(g, lead, !n.ty().dag) } else { Vec::new() },
        "shapes": shape_list,
        "inputs": inputs,
        "count": g.selection().len(),
    })
}

/// Every attribute of a node, for the attribute editor.
fn node_attrs(g: &Graph, id: NodeId) -> Value {
    let Some(n) = g.node(id) else { return Value::Null };
    let path = g.path(id);
    let mut attrs = Vec::new();
    for (i, a) in n.ty().attrs.iter().enumerate() {
        if a.flags & flags::HIDDEN != 0 || matches!(a.ty, PlugType::Mesh | PlugType::Data) {
            continue;
        }
        // A shading node's results aren't settings.
        if a.flags & flags::OUTPUT != 0 && aztek_scene::shading::classification(&n.ty().name).is_some() {
            continue;
        }
        attrs.push(json!({
            "attr": a.name,
            "plug": format!("{path}.{}", a.name),
            "nice": attr_nice(a),
            "type": type_name(a.ty),
            "value": value_json(&aztek_scene::draw_style::through_layer(g, &aztek_scene::plug::PlugRef::new(id, i as i32)).unwrap_or_else(|| n.value(i).clone())),
            "enum": a.enum_names,
            "children": a.child_names,
            "output": a.flags & flags::OUTPUT != 0,
            "keyable": a.flags & flags::KEYABLE != 0,
            "category": a.category,
            "doc": a.doc,
            "min": a.soft_min.or(a.min_value),
            "max": a.soft_max.or(a.max_value),
            "connected": g.source(&aztek_scene::plug::PlugRef::new(id, i as i32)).is_some() || aztek_scene::draw_style::driven_by_layer(g, &aztek_scene::plug::PlugRef::new(id, i as i32)),
            "source": g.source(&aztek_scene::plug::PlugRef::new(id, i as i32)).map(|s| g.path(s.node)),
            // Wire and interface colours are shown as they are stored; materials' are linear.
            "displayColor": matches!(a.name.as_str(), "overrideColorRGB" | "objectColorRGB" | "wireColorRGB" | "outlinerColor"),
            "color": a.flags & flags::COLOR != 0,
            "pair": !a.child_names[0].is_empty() && a.child_names[2].is_empty(),
            "file": a.flags & flags::FILE != 0,
            "mappable": a.flags & flags::COLOR != 0 && a.flags & flags::INPUT != 0 && aztek_scene::shading::is_material_type(n.ty()),
        }));
    }
    json!({"name": n.name(), "path": path, "type": n.ty().name, "doc": n.ty().doc, "attrs": attrs})
}

/// The attribute editor's tabs for a node: the transform, its shape, and the shape's history, as the reference shows them.
pub fn attribute_editor(core: &Core, path: &str) -> Value {
    let g = &core.s.graph;
    let id = if path.is_empty() {
        match g.selection().last() {
            Some(&id) => id,
            None => return Value::Null,
        }
    } else {
        match g.find(path) {
            Ok(Some(id)) => id,
            _ => return Value::Null,
        }
    };
    let mut tabs = vec![id];
    let Some(n) = g.node(id) else { return Value::Null };
    if n.ty().is_a("transform") {
        for s in aztek_scene::dag::shapes(g, id, false) {
            tabs.push(s);
            if g.node(s).is_some_and(|n| n.ty().is_a("mesh")) {
                tabs.extend(aztek_scene::history::history_of(g, s).into_iter().filter(|&h| g.node(h).is_some_and(|n| !n.ty().dag)));
            }
            // Its shading groups and their materials, the last tabs as in the reference.
            for sg in aztek_scene::shading::groups_of(g, s) {
                if !tabs.contains(&sg) {
                    tabs.push(sg);
                }
                if let Some(m) = aztek_scene::shading::material_of_group(g, sg).filter(|m| !tabs.contains(m)) {
                    tabs.push(m);
                }
            }
        }
    } else if is_shape(g, id) {
        let xf = aztek_scene::dag::transform_of(g, id);
        if xf != id && xf != NO_NODE {
            tabs.insert(0, xf);
        }
    }
    json!({"focus": g.path(id), "tabs": tabs.iter().map(|&t| node_attrs(g, t)).collect::<Vec<_>>()})
}

/// The file node's image found on disk, for its Attribute Editor: {resolved} or nothing.
pub fn texture_path(core: &mut Core, node: &str) -> Value {
    let project = aztek_scene::shading::project_dir(&mut core.s);
    let scene = core.s.scene_path.clone();
    let g = &mut core.s.graph;
    let Ok(p) = g.plug(&format!("{node}.fileTextureName")) else { return Value::Null };
    let written = match g.value(&p) {
        Ok(PlugValue::String(x)) => x,
        _ => String::new(),
    };
    let found = aztek_scene::shading::resolve_texture_uncached(&written, scene.as_deref(), project.as_deref());
    json!({"written": written, "resolved": found.map(|p| p.display().to_string())})
}

/// The Hypershade's lists: materials with their colour and groups, textures with their images, utilities.
pub fn shading(core: &mut Core) -> Value {
    let project = aztek_scene::shading::project_dir(&mut core.s);
    let scene = core.s.scene_path.clone();
    let g = &mut core.s.graph;
    let mut ids = g.nodes();
    ids.sort_unstable();
    let (mut materials, mut textures, mut utilities) = (Vec::new(), Vec::new(), Vec::new());
    for id in ids {
        let Some(n) = g.node(id) else { continue };
        let t = n.type_arc().clone();
        let name = g.path(id);
        if aztek_scene::shading::is_material_type(&t) {
            let sg = aztek_scene::shading::group_of_material(g, id, false).ok().flatten();
            let drawn = sg.and_then(|s| aztek_scene::shading::draw_material(g, s, scene.as_deref(), project.as_deref()));
            let color = drawn.as_ref().map(|d| d.color);
            let map = drawn.as_ref().and_then(|d| d.color_map.as_ref()).and_then(|m| m.resolved.clone());
            materials.push(json!({"name": name, "type": t.name, "group": sg.map(|s| g.path(s)), "color": color, "image": map}));
        } else if aztek_scene::shading::is_texture_type(&t) {
            let p = g.plug(&format!("{name}.fileTextureName"));
            let written = p.ok().and_then(|p| g.value(&p).ok()).and_then(|v| if let PlugValue::String(x) = v { Some(x) } else { None }).unwrap_or_default();
            let found = aztek_scene::shading::resolve_texture(&written, scene.as_deref(), project.as_deref());
            textures.push(json!({"name": name, "type": t.name, "written": written, "image": found.map(|p| p.display().to_string())}));
        } else if aztek_scene::shading::classification(&t.name).is_some_and(|c| c.starts_with("utility")) && t.name != "materialInfo" {
            utilities.push(json!({"name": name, "type": t.name}));
        }
    }
    json!({"materials": materials, "textures": textures, "utilities": utilities})
}

/// The image to draw behind the UV Editor's UVs: the first selected mesh's first material with a file on its colour.
pub fn uv_texture(core: &mut Core) -> Value {
    let project = aztek_scene::shading::project_dir(&mut core.s);
    let scene = core.s.scene_path.clone();
    let g = &mut core.s.graph;
    for id in g.selection().to_vec() {
        let Some(m) = components::as_mesh(g, id) else { continue };
        for sg in aztek_scene::shading::groups_of(g, m) {
            let Some(d) = aztek_scene::shading::draw_material(g, sg, scene.as_deref(), project.as_deref()) else { continue };
            if let Some(t) = d.color_map {
                if let Some(path) = t.resolved {
                    return json!({"path": path, "repeat": t.repeat, "offset": t.offset});
                }
            }
        }
    }
    Value::Null
}

pub fn layers(core: &Core) -> Value {
    let g = &core.s.graph;
    let mut out = Vec::new();
    for id in g.nodes() {
        let Some(n) = g.node(id) else { continue };
        if n.ty().name != aztek_scene::display_layers::TYPE_NAME || n.name() == aztek_scene::display_layers::DEFAULT_LAYER {
            continue;
        }
        let members: Vec<String> = aztek_scene::display_layers::members(g, id).iter().map(|&m| g.path(m)).collect();
        out.push(json!({
            "name": n.name(),
            "visible": n.bool(aztek_scene::display_layers::VISIBILITY),
            "displayType": n.int(aztek_scene::display_layers::DISPLAY_TYPE),
            "order": n.int(aztek_scene::display_layers::DISPLAY_ORDER),
            "playback": !n.bool(aztek_scene::display_layers::HIDE_ON_PLAYBACK),
            "color": aztek_scene::display_layers::layer_color(n).map_or(Value::Null, |c| json!(c)),
            "colorIndex": n.int(aztek_scene::display_layers::COLOR),
            "useRgb": n.bool(aztek_scene::display_layers::OVERRIDE_RGB_COLORS),
            "rgb": n.vec3(aztek_scene::display_layers::OVERRIDE_COLOR_RGB).to_array(),
            "members": members,
        }));
    }
    out.sort_by_key(|l| -l["order"].as_i64().unwrap_or(0));
    Value::Array(out)
}

pub fn state(core: &Core) -> Value {
    let g = &core.s.graph;
    let mode = match components::mode(g) {
        ComponentMode::Object => "object",
        ComponentMode::Vertex => "vertex",
        ComponentMode::Edge => "edge",
        ComponentMode::Face => "face",
    };
    let sel = components::component_strings(g, false);
    let sel = if sel.is_empty() { g.selection().iter().map(|&id| g.path(id)).collect() } else { sel };
    json!({
        "mode": mode,
        "selection": sel,
        "history": g.construction_history,
        "canUndo": g.undo_stack().can_undo(),
        "canRedo": g.undo_stack().can_redo(),
        "file": core.s.scene_path,
        "modified": core.s.is_modified(),
        "undoId": g.undo_stack().newest_id(),
        "undoLabel": g.undo_stack().undo_label(),
        "version": core.version,
    })
}

pub fn commands(core: &Core) -> Value {
    let mut v: Vec<Value> = core
        .s
        .commands
        .all()
        .map(|c| json!({"name": c.name, "doc": c.doc, "flags": c.flags.iter().map(|f| json!({"name": f.name, "short": f.short_name, "doc": f.doc})).collect::<Vec<_>>()}))
        .collect();
    v.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    Value::Array(v)
}

/// The UV editor's view of the selected and hilited meshes: maps, faces, borders, and the components selected.
pub fn uv_data(core: &mut Core) -> Value {
    let g = &mut core.s.graph;
    let mut meshes: Vec<NodeId> = Vec::new();
    for id in g.selection().to_vec() {
        if let Some(m) = components::as_mesh(g, id) {
            if !meshes.contains(&m) {
                meshes.push(m);
            }
        }
    }
    let mode = components::mode(g);
    for kind in [ComponentMode::Vertex, ComponentMode::Edge, ComponentMode::Face] {
        for (&node, _) in components::selected_all(g, kind).iter() {
            if let Some(m) = components::as_mesh(g, node) {
                if !meshes.contains(&m) {
                    meshes.push(m);
                }
            }
        }
    }
    let mut out = Vec::new();
    for m in meshes {
        let Some(mut d) = aztek_scene::uv_ops::editor_data(g, m) else { continue };
        let sel = |kind| -> Vec<u32> {
            let all = components::selected_all(g, kind);
            all.iter().filter(|(n, _)| components::as_mesh(g, **n) == Some(m)).flat_map(|(_, ids)| ids.iter().copied()).collect()
        };
        d["selFaces"] = json!(sel(ComponentMode::Face));
        d["selEdges"] = json!(sel(ComponentMode::Edge));
        d["selPoints"] = json!(sel(ComponentMode::Vertex));
        out.push(d);
    }
    let mode = match mode {
        ComponentMode::Object => "object",
        ComponentMode::Vertex => "vertex",
        ComponentMode::Edge => "edge",
        ComponentMode::Face => "face",
    };
    json!({"mode": mode, "meshes": out})
}

/// The Node Editor's graph: the root nodes and what's connected to them, `depth` connections out. arg is JSON:
/// {"roots": [names], "depth": n}. Node attrs: name, input, output; edges: [source, attr, destination, attr].
pub fn node_graph(core: &Core, arg: &str) -> Value {
    let g = &core.s.graph;
    let parsed: Value = serde_json::from_str(arg).unwrap_or(json!({}));
    let depth = parsed.get("depth").and_then(Value::as_i64).unwrap_or(1).max(0);
    let mut set: Vec<NodeId> = Vec::new();
    for r in parsed.get("roots").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str) {
        if let Ok(Some(id)) = g.find(r) {
            if !set.contains(&id) {
                set.push(id);
            }
            // A transform brings its shape, as the editor graphs them.
            if g.node(id).is_some_and(|n| n.ty().is_a("transform")) {
                for s in aztek_scene::dag::shapes(g, id, false) {
                    if !set.contains(&s) {
                        set.push(s);
                    }
                }
            }
        }
    }
    let conns = g.connections();
    let touches = |a: NodeId, b: NodeId, set: &[NodeId]| set.contains(&a) || set.contains(&b);
    for _ in 0..depth {
        let mut more = Vec::new();
        for (d, s) in &conns {
            if touches(d.node, s.node, &set) {
                for id in [d.node, s.node] {
                    if !set.contains(&id) && !more.contains(&id) {
                        more.push(id);
                    }
                }
            }
        }
        set.extend(more);
    }
    let nodes: Vec<Value> = set
        .iter()
        .filter_map(|&id| {
            let n = g.node(id)?;
            let attrs: Vec<Value> = n
                .ty()
                .attrs
                .iter()
                .filter(|a| !a.has(flags::HIDDEN) || conns.iter().any(|(d, s)| (d.node == id && g.attr(d).name == a.name) || (s.node == id && g.attr(s).name == a.name)))
                .map(|a| json!({"name": a.name, "input": a.has(flags::INPUT), "output": a.has(flags::OUTPUT) || !a.has(flags::INPUT)}))
                .collect();
            Some(json!({"name": g.path(id), "type": n.ty().name, "attrs": attrs}))
        })
        .collect();
    let plug_attr = |p: &PlugRef| -> String {
        let a = g.attr(p);
        if p.component >= 0 && a.ty == PlugType::Vec3 {
            a.child_names[p.component as usize].clone()
        } else if p.element >= 0 {
            format!("{}[{}]", a.name, p.element)
        } else {
            a.name.clone()
        }
    };
    let edges: Vec<Value> = conns
        .iter()
        .filter(|(d, s)| set.contains(&d.node) && set.contains(&s.node))
        .map(|(d, s)| json!([g.path(s.node), plug_attr(s), g.path(d.node), plug_attr(d)]))
        .collect();
    json!({"nodes": nodes, "edges": edges})
}

/// Tables for the General Editors. arg is JSON with "what":
/// spreadSheet: the selection's keyable and shape attributes; skinWeights: weights of the selected vertices per influence;
/// connections: {"node"}'s attributes both ways; hypergraph: the DAG as nodes and parents; namespaces: names with a colon.
pub fn editors(core: &mut Core, arg: &str) -> Value {
    let parsed: Value = serde_json::from_str(arg).unwrap_or(json!({}));
    let what = parsed.get("what").and_then(Value::as_str).unwrap_or("");
    let g = &core.s.graph;
    match what {
        "spreadSheet" => {
            let rows: Vec<Value> = g
                .selection()
                .iter()
                .map(|&id| {
                    let shapes = aztek_scene::dag::shapes(g, id, false);
                    json!({"node": g.path(id), "channels": channels(g, id, false), "shape": shapes.first().map(|&s| channels(g, s, true)).unwrap_or_default()})
                })
                .collect();
            Value::Array(rows)
        }
        "hypergraph" => {
            let mut out = Vec::new();
            for id in g.nodes() {
                let Some(n) = g.node(id) else { continue };
                if !n.ty().dag {
                    continue;
                }
                let p = g.parent(id);
                out.push(json!({"name": n.name(), "path": g.path(id), "type": n.ty().name, "parent": if p == NO_NODE { Value::Null } else { json!(g.path(p)) }, "selected": g.selection().contains(&id)}));
            }
            Value::Array(out)
        }
        "namespaces" => {
            let mut names = std::collections::BTreeMap::<String, i64>::new();
            for id in g.nodes() {
                if let Some(n) = g.node(id) {
                    if let Some((ns, _)) = n.name().rsplit_once(':') {
                        *names.entry(ns.to_string()).or_default() += 1;
                    }
                }
            }
            json!(names)
        }
        _ => Value::Null,
    }
}
