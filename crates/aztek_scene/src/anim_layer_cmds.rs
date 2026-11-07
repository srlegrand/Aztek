//! The animLayer command, with every flag on its page, and what the Animation Layer Editor's menus do. The reference's menu items run
//! MEL procedures whose signatures aren't documented; `animLayerMerge` and the `layerEditor*` commands here do what the
//! documentation says those items do.

use aztek_ops::{fail, ArgType, Args, CmdResult, Commands};
use serde_json::{json, Value};

use crate::anim::{self, f, query_flags, queried, Cmd};
use crate::anim_layers::*;
use crate::graph::Graph;
use crate::node::types;
use crate::plug::{flags, NodeId, PlugRef, PlugType, PlugValue, NO_NODE};
use crate::session::Session;

pub(crate) fn split_axes(g: &Graph, plugs: Vec<PlugRef>) -> Vec<PlugRef> {
    plugs
        .into_iter()
        .flat_map(|p| {
            if p.component < 0 && g.attr(&p).ty == PlugType::Vec3 {
                (0..3).map(|c| PlugRef::component(p.node, p.attr, c)).collect::<Vec<_>>()
            } else {
                vec![p]
            }
        })
        .collect()
}

fn layer_named(g: &Graph, n: &str) -> Option<NodeId> {
    g.find(n).ok().flatten().filter(|&id| is_layer(g, id))
}

pub(crate) fn need_layer(g: &Graph, n: &str, cmd: &str) -> CmdResult<NodeId> {
    match layer_named(g, n) {
        Some(l) => Ok(l),
        None => fail!("{cmd}: no layer named {n}"),
    }
}

fn layer_arg(g: &Graph, a: &Args) -> Option<NodeId> {
    a.named().iter().find_map(|n| layer_named(g, n))
}

/// A flag's text value, when it was given one.
fn text_flag(a: &Args, flag: &str) -> Option<String> {
    a.get(flag).and_then(Value::as_str).map(str::to_string)
}

fn excluded(g: &Graph, p: &PlugRef, a: &Args) -> bool {
    let at = g.attr(p);
    (a.bool("excludeBoolean") && at.ty == PlugType::Bool)
        || (a.bool("excludeEnum") && at.ty == PlugType::Enum)
        || (a.bool("excludeRotate") && at.name == "rotate")
        || (a.bool("excludeScale") && at.name == "scale")
        || (a.bool("excludeTranslate") && at.name == "translate")
        || (a.bool("excludeVisibility") && at.name == "visibility")
        || (a.bool("excludeDynamic") && at.has(flags::USER))
}

fn attr_list(a: &Args, flag: &str) -> Vec<String> {
    match a.get(flag) {
        Some(Value::String(x)) => vec![x.clone()],
        Some(Value::Array(v)) => v.iter().filter_map(|x| x.as_str().map(str::to_string)).collect(),
        _ => Vec::new(),
    }
}

/// The transforms of the selection, with the keying groups they are in when asked.
pub(crate) fn selected_nodes(s: &Session, related: bool) -> Vec<NodeId> {
    let g = &s.graph;
    let mut out: Vec<NodeId> = Vec::new();
    for &id in g.selection() {
        let t = crate::dag::transform_of(g, id);
        if !out.contains(&t) {
            out.push(t);
        }
    }
    if related {
        for set in g.nodes().into_iter().filter(|&x| g.node(x).is_some_and(|n| n.ty().name == "keyingGroup")) {
            let members: Vec<NodeId> = crate::object_sets::members(g, set).iter().map(|m| m.node).collect();
            if members.iter().any(|m| out.contains(m)) {
                for m in members {
                    if !out.contains(&m) {
                        out.push(m);
                    }
                }
            }
        }
    }
    out
}

fn plugs_of_nodes(s: &Session, nodes: &[NodeId], a: &Args) -> Vec<PlugRef> {
    let g = &s.graph;
    let mut out: Vec<PlugRef> = Vec::new();
    for &id in nodes {
        out.extend(anim::keyable_plugs(g, id));
    }
    out.retain(|p| !excluded(g, p, a));
    out
}

/// Plain channels of a layer's blend for this node's plugs.
pub(crate) fn remove_plugs(g: &mut Graph, layer: NodeId, plugs: &[PlugRef]) -> CmdResult<()> {
    for p in plugs {
        if let Some(&b) = chain(g, p).iter().find(|&&b| blend_layer(g, b) == Some(layer)) {
            remove_blend(g, b)?;
        }
    }
    Ok(())
}

fn copy_settings(g: &mut Graph, from: NodeId, to: NodeId) -> CmdResult<()> {
    for at in [L_OVERRIDE, L_PASSTHROUGH, L_RAM, L_SAM, L_WEIGHT, L_GHOST_COLOR] {
        let v = g.node(from).map(|n| n.value(at).clone());
        if let Some(v) = v {
            g.set_value(PlugRef::new(to, at as i32), v)?;
        }
    }
    Ok(())
}

/// Copies or moves the curves of channels both layers hold.
pub(crate) fn transfer(g: &mut Graph, from: NodeId, to: NodeId, take: bool) -> CmdResult<()> {
    for b in layer_blends(g, from) {
        for p in split_axes(g, blend_plugs(g, b)) {
            if !chain(g, &p).iter().any(|&x| blend_layer(g, x) == Some(to)) {
                continue;
            }
            let Some(fs) = slot(g, &p, Some(from)) else { continue };
            let Some(c) = curve_in(g, &fs) else { continue };
            let k = anim::keys_of(g, c);
            if let Some(n) = curve_for(g, &p, Some(to))? {
                anim::set_keys(g, n, k)?;
            }
            if take {
                g.delete_node(c)?;
                rest_input(g, b, from)?;
            }
        }
    }
    Ok(())
}

/// A layer's channels on another, with their curves when asked.
pub(crate) fn copy_layer(g: &mut Graph, from: NodeId, to: NodeId, curves: bool) -> CmdResult<()> {
    copy_settings(g, from, to)?;
    for b in layer_blends(g, from) {
        for p in blend_plugs(g, b) {
            add_plug(g, to, &p)?;
        }
    }
    if curves {
        transfer(g, from, to, false)?;
    }
    Ok(())
}

fn apply_flags(s: &mut Session, layer: NodeId, a: &Args) -> CmdResult<()> {
    let g = &mut s.graph;
    for (flag, at) in [("mute", L_MUTE), ("lock", L_LOCK), ("solo", L_SOLO), ("selected", L_SELECTED), ("preferred", L_PREFERRED), ("passthrough", L_PASSTHROUGH), ("collapse", L_COLLAPSE), ("ghost", L_GHOST)] {
        if let Some(b) = a.get(flag).and_then(Value::as_bool) {
            g.set_value(PlugRef::new(layer, at as i32), PlugValue::Bool(b))?;
        }
    }
    if let Some(c) = a.get("ghostColor").and_then(Value::as_i64) {
        g.set_value(PlugRef::new(layer, L_GHOST_COLOR as i32), PlugValue::Int(c))?;
    }
    if let Some(w) = a.get("weight").and_then(Value::as_f64) {
        g.set_value(PlugRef::new(layer, L_WEIGHT as i32), PlugValue::Float(w.clamp(0.0, 1.0)))?;
    }
    if let Some(b) = a.get("override").and_then(Value::as_bool) {
        g.set_value(PlugRef::new(layer, L_OVERRIDE as i32), PlugValue::Bool(b))?;
        g.evaluate();
        for blend in layer_blends(g, layer) {
            rest_input(g, blend, layer)?;
        }
    }
    if let Some(mode) = a.get("rotationAccumulationMode") {
        let v = mode.as_i64().or(mode.as_str().map(|x| i64::from(x == "layer"))).unwrap_or(0);
        g.set_value(PlugRef::new(layer, L_RAM as i32), PlugValue::Int(v))?;
    }
    if let Some(mode) = a.get("scaleAccumulationMode") {
        let v = mode.as_i64().or(mode.as_str().map(|x| i64::from(x == "multiply"))).unwrap_or(1);
        g.set_value(PlugRef::new(layer, L_SAM as i32), PlugValue::Int(v))?;
        for blend in layer_blends(g, layer) {
            if blend_kind(g, blend) == BlendKind::Scale {
                rest_input(g, blend, layer)?;
            }
        }
    }
    if let Some(n) = text_flag(a, "parent") {
        let p = need_layer(g, &n, "animLayer")?;
        set_parent(g, layer, p)?;
    }
    for (flag, after) in [("moveLayerAfter", true), ("moveLayerBefore", false)] {
        if let Some(n) = text_flag(a, flag) {
            let o = need_layer(g, &n, "animLayer")?;
            move_layer(g, layer, o, after)?;
        }
    }
    // Channels.
    let mut plugs: Vec<PlugRef> = Vec::new();
    for at in attr_list(a, "attribute") {
        plugs.push(s.graph.plug(&at)?);
    }
    if a.bool("addSelectedObjects") {
        let nodes = selected_nodes(s, a.bool("addRelatedKG"));
        plugs.extend(plugs_of_nodes(s, &nodes, a));
    }
    let mut gone: Vec<PlugRef> = Vec::new();
    for at in attr_list(a, "removeAttribute") {
        let p = s.graph.plug(&at)?;
        gone.extend(split_axes(&s.graph, vec![p]));
    }
    if a.bool("removeSelectedObjects") {
        let nodes = selected_nodes(s, a.bool("addRelatedKG"));
        gone.extend(plugs_of_nodes(s, &nodes, a));
    }
    let g = &mut s.graph;
    for p in &plugs {
        add_plug(g, layer, p)?;
    }
    remove_plugs(g, layer, &gone)?;
    if a.bool("removeAllAttributes") {
        for b in layer_blends(g, layer) {
            remove_blend(g, b)?;
        }
    }
    if let Some(n) = text_flag(a, "copyAnimation") {
        let from = need_layer(g, &n, "animLayer")?;
        transfer(g, from, layer, false)?;
    }
    if let Some(n) = text_flag(a, "extractAnimation") {
        let from = need_layer(g, &n, "animLayer")?;
        transfer(g, from, layer, true)?;
    }
    if a.bool("writeBlendnodeDestinations") {
        for b in layer_blends(g, layer) {
            let names: Vec<String> = blend_plugs(g, b).iter().map(|p| g.plug_name(p)).collect();
            if let Some(first) = names.first() {
                let text = if is_rotation(g, b) { first.rsplit_once('.').map(|(n, at)| format!("{n}.{}", at.trim_end_matches(['X', 'Y', 'Z']))).unwrap_or_default() } else { first.clone() };
                g.set_value(PlugRef::new(b, BLEND_DEST as i32), PlugValue::String(text))?;
            }
        }
    }
    Ok(())
}

fn names(g: &Graph, ids: impl IntoIterator<Item = NodeId>) -> Value {
    json!(ids.into_iter().map(|i| g.path(i)).collect::<Vec<_>>())
}

fn layers_of_plug(g: &Graph, p: &PlugRef) -> Vec<NodeId> {
    plug_layer_curves(g, p).into_iter().map(|(l, _)| l).collect()
}

/// A flag asked about in query mode, however it arrived: MEL gives a string flag the next word as its value.
fn asked(a: &Args, flag: &str) -> bool {
    queried(a, &[flag]) || a.has(flag)
}

fn query(s: &Session, a: &Args) -> CmdResult {
    let g = &s.graph;
    let word = ["attribute", "children", "parent", "root", "copy", "layeredPlug", "findCurveForPlug"].iter().find_map(|fl| attr_list(a, fl).iter().find_map(|n| layer_named(g, n)));
    let layer = layer_arg(g, a).or(word);
    let named_plugs = |extra: &[String]| -> CmdResult<Vec<PlugRef>> {
        let mut out = Vec::new();
        let mut list: Vec<String> = a.named().into_iter().filter(|n| layer_named(g, n).is_none()).collect();
        list.extend(extra.iter().cloned());
        if list.is_empty() {
            list = g.selection().iter().map(|&i| g.path(i)).collect();
        }
        for n in list {
            if n.contains('.') {
                out.extend(split_axes(g, vec![g.plug(&n)?]));
            } else if let Some(id) = g.find(&n)? {
                out.extend(anim::keyable_plugs(g, crate::dag::transform_of(g, id)));
            }
        }
        Ok(out)
    };
    if asked(a, "exists") {
        return Ok(json!(layer.is_some()));
    }
    if asked(a, "root") && word.is_none() {
        return Ok(base_layer(g).map(|b| json!(g.path(b))).unwrap_or(Value::Null));
    }
    if asked(a, "maxLayers") {
        // The page gives no number; this product sets no limit of its own.
        return Ok(json!(i32::MAX));
    }
    if asked(a, "layeredHierarchy") {
        return Ok(names(g, ordered_layers(g)));
    }
    if asked(a, "affectedLayers") {
        let mut out: Vec<NodeId> = Vec::new();
        for p in named_plugs(&[])? {
            for l in layers_of_plug(g, &p) {
                if !out.contains(&l) {
                    out.push(l);
                }
            }
        }
        return Ok(names(g, out));
    }
    if asked(a, "bestLayer") {
        let at = attr_list(a, "attribute");
        let plugs = named_plugs(&at)?;
        let best = plugs.first().and_then(|p| key_target(g, p).or_else(|| base_layer(g))).or_else(|| base_layer(g));
        return Ok(best.map(|b| json!(g.path(b))).unwrap_or(Value::Null));
    }
    if asked(a, "bestAnimLayer") {
        let at = attr_list(a, "attribute");
        let mut out: Vec<NodeId> = Vec::new();
        for p in named_plugs(&at)? {
            if let Some(l) = key_target(g, &p) {
                if !out.contains(&l) {
                    out.push(l);
                }
            }
        }
        return Ok(names(g, out));
    }
    if asked(a, "findCurveForPlug") {
        let plugs = match text_flag(a, "findCurveForPlug").filter(|n| n.contains('.')) {
            Some(n) => split_axes(g, vec![g.plug(&n)?]),
            None => named_plugs(&[])?,
        };
        let mut out = Vec::new();
        for p in plugs {
            let l = layer.or_else(|| key_target(g, &p));
            out.extend(match slot(g, &p, l) {
                Some(sl) => curve_in(g, &sl),
                None => anim::curve_of(g, &p),
            });
        }
        return Ok(names(g, out));
    }
    if asked(a, "layeredPlug") {
        let plugs = match text_flag(a, "layeredPlug").filter(|n| n.contains('.')) {
            Some(n) => split_axes(g, vec![g.plug(&n)?]),
            None => named_plugs(&[])?,
        };
        let mut out = Vec::new();
        for p in plugs {
            if let Some(sl) = slot(g, &p, layer) {
                out.push(g.plug_name(&sl));
            }
        }
        return Ok(if out.len() == 1 { json!(out[0]) } else { json!(out) });
    }
    let Some(l) = layer.or_else(|| base_layer(g)) else { fail!("animLayer: no layers") };
    let flag = |at: usize| json!(layer_flag(g, l, at));
    let d = g.node(l).map(|n| n.data());
    for (name, at) in [("mute", L_MUTE), ("lock", L_LOCK), ("solo", L_SOLO), ("override", L_OVERRIDE), ("selected", L_SELECTED), ("preferred", L_PREFERRED), ("passthrough", L_PASSTHROUGH), ("collapse", L_COLLAPSE), ("ghost", L_GHOST)] {
        if asked(a, name) {
            return Ok(flag(at));
        }
    }
    Ok(if asked(a, "weight") {
        json!(d.map(|d| d.float(L_WEIGHT)).unwrap_or(1.0))
    } else if asked(a, "ghostColor") {
        json!(d.map(|d| d.int(L_GHOST_COLOR)).unwrap_or(5))
    } else if asked(a, "rotationAccumulationMode") {
        json!(d.map(|d| d.int(L_RAM)).unwrap_or(0))
    } else if asked(a, "scaleAccumulationMode") {
        json!(d.map(|d| d.int(L_SAM)).unwrap_or(1))
    } else if asked(a, "parent") {
        layer_parent(g, l).map(|p| json!(g.path(p))).unwrap_or(Value::Null)
    } else if asked(a, "children") {
        names(g, layer_children(g, l))
    } else if asked(a, "attribute") {
        let mut out: Vec<String> = Vec::new();
        for b in layer_blends(g, l) {
            for p in split_axes(g, blend_plugs(g, b)) {
                out.push(g.plug_name(&p));
            }
        }
        json!(out)
    } else if asked(a, "animCurves") {
        let mut out: Vec<NodeId> = Vec::new();
        if Some(l) == base_layer(g) {
            for b in layers_blends_all(g) {
                for p in split_axes(g, blend_plugs(g, b)) {
                    if let Some(c) = slot(g, &p, None).and_then(|x| curve_in(g, &x)) {
                        if !out.contains(&c) {
                            out.push(c);
                        }
                    }
                }
            }
        }
        for b in layer_blends(g, l) {
            out.extend(inputs_b(g, b).iter().filter_map(|i| curve_in(g, i)));
        }
        names(g, out)
    } else if asked(a, "baseAnimCurves") {
        let mut out: Vec<NodeId> = Vec::new();
        for b in layer_blends(g, l) {
            for p in split_axes(g, blend_plugs(g, b)) {
                if let Some(c) = slot(g, &p, None).and_then(|x| curve_in(g, &x)) {
                    if !out.contains(&c) {
                        out.push(c);
                    }
                }
            }
        }
        names(g, out)
    } else if asked(a, "blendNodes") {
        names(g, layer_blends(g, l))
    } else {
        json!(g.path(l))
    })
}

fn layers_blends_all(g: &Graph) -> Vec<NodeId> {
    ordered_layers(g).into_iter().flat_map(|l| layer_blends(g, l)).collect()
}

/// A flag that holds a layer name, or for scripts that pass it true, the -copy layer.
pub(crate) fn create_layer(s: &mut Session, name: &str) -> CmdResult<NodeId> {
    let g = &mut s.graph;
    let base = ensure_base(g)?;
    let t = types::find("animLayer").unwrap();
    let l = g.create_node(&t, name, NO_NODE, true)?;
    set_parent(g, l, base)?;
    Ok(l)
}

fn anim_layer(s: &mut Session, a: &Args) -> CmdResult {
    if a.bool("query") {
        return query(s, a);
    }
    if a.bool("edit") {
        let Some(l) = layer_arg(&s.graph, a) else { fail!("animLayer: name the layer to edit") };
        s.graph.open_chunk("animLayer");
        let r = apply_flags(s, l, a);
        s.graph.close_chunk();
        r?;
        return Ok(json!(s.graph.path(l)));
    }
    if a.bool("forceUIRebuild") || a.bool("forceUIRefresh") {
        return Ok(Value::Null);
    }
    // Create. The reference's default name is AnimLayer1.
    let name = a.named().first().cloned().unwrap_or_else(|| "AnimLayer1".into());
    s.graph.open_chunk("animLayer");
    let r = (|| -> CmdResult<NodeId> {
        let l = create_layer(s, &name)?;
        let g = &mut s.graph;
        if let Some(n) = text_flag(a, "copy") {
            let from = need_layer(g, &n, "animLayer")?;
            // Scripts of older habit say -cp L1 -ca for the keys and -cp L1 -cna without.
            let without = a.get("copyNoAnimation").is_some_and(|v| v == &Value::Bool(true));
            copy_layer(g, from, l, !without)?;
        } else if let Some(n) = text_flag(a, "copyNoAnimation") {
            let from = need_layer(g, &n, "animLayer")?;
            copy_layer(g, from, l, false)?;
        }
        apply_flags(s, l, a)?;
        Ok(l)
    })();
    s.graph.close_chunk();
    let l = r?;
    Ok(json!(s.graph.path(l)))
}

pub fn register(c: &mut Commands<Session>) {
    c.register(
        query_flags(Cmd::new(
            "animLayer",
            "Create (name optional), edit (-e layer) or query (-q layer) an animation layer. The first layer brings BaseAnimation with it. -aso or -at put \
             channels in the layer; setKeyframe -al keys them there.",
            anim_layer,
        ))
        .positional("the layer, or for -afl and -bl the objects or plugs")
        .flag(f("addRelatedKG", "akg", ArgType::Bool, "With -aso or -rso: the keying groups the objects are in too."))
        .flag(f("addSelectedObjects", "aso", ArgType::Bool, "Add the selected objects' keyable channels."))
        .flag(f("affectedLayers", "afl", ArgType::Bool, "Query: the layers the objects or plugs are in."))
        .flag(f("animCurves", "anc", ArgType::Bool, "Query: the layer's curves."))
        .flag(f("attribute", "at", ArgType::Any, "Plug(s) to add; query: the layer's plugs.").multi(1))
        .flag(f("baseAnimCurves", "bac", ArgType::Bool, "Query: the base curves of the layer's channels."))
        .flag(f("bestAnimLayer", "blr", ArgType::Bool, "Query: the best layers for keying the selected objects, or the -at plugs."))
        .flag(f("bestLayer", "bl", ArgType::Bool, "Query: the layer that will be keyed for the attribute."))
        .flag(f("blendNodes", "bld", ArgType::Bool, "Query: the layer's blend nodes."))
        .flag(f("children", "c", ArgType::String, "Query: the child layers."))
        .flag(f("collapse", "col", ArgType::Bool, "Collapsed in the layer editor."))
        .flag(f("copy", "cp", ArgType::String, "Copy from this layer."))
        .flag(f("copyAnimation", "ca", ArgType::Any, "Copy animation from this layer, on channels both layers hold."))
        .flag(f("copyNoAnimation", "cna", ArgType::Any, "Copy from this layer without the curves."))
        .flag(f("excludeBoolean", "ebl", ArgType::Bool, "With -aso or -rso: not boolean channels."))
        .flag(f("excludeDynamic", "edn", ArgType::Bool, "With -aso or -rso: not dynamic attributes."))
        .flag(f("excludeEnum", "een", ArgType::Bool, "With -aso or -rso: not enum channels."))
        .flag(f("excludeProxy", "epr", ArgType::Bool, "With -aso or -rso: not proxy attributes (there are none here)."))
        .flag(f("excludeRotate", "ert", ArgType::Bool, "With -aso or -rso: not rotate."))
        .flag(f("excludeScale", "esc", ArgType::Bool, "With -aso or -rso: not scale."))
        .flag(f("excludeTranslate", "etr", ArgType::Bool, "With -aso or -rso: not translate."))
        .flag(f("excludeVisibility", "evs", ArgType::Bool, "With -aso or -rso: not visibility."))
        .flag(f("exists", "ex", ArgType::Bool, "Query: whether the layer exists."))
        .flag(f("extractAnimation", "ea", ArgType::String, "Move animation from this layer, on channels both layers hold."))
        .flag(f("findCurveForPlug", "fcv", ArgType::String, "Query: the curve holding a plug's keys on the layer."))
        .flag(f("forceUIRebuild", "fur", ArgType::Bool, "Rebuild the layers interface."))
        .flag(f("forceUIRefresh", "uir", ArgType::Bool, "Refresh the layers interface."))
        .flag(f("ghost", "gh", ArgType::Bool, "The layer's ghost button."))
        .flag(f("ghostColor", "ghc", ArgType::Int, "The layer's ghost and key tick colour index."))
        .flag(f("layeredHierarchy", "lh", ArgType::Bool, "Query: all layers, bottom to top."))
        .flag(f("layeredPlug", "lp", ArgType::String, "Query: the plug on the blend node for this plug on the layer."))
        .flag(f("lock", "l", ArgType::Bool, "Lock the layer against keying."))
        .flag(f("maxLayers", "ml", ArgType::Bool, "Query: the most layers the product supports."))
        .flag(f("moveLayerAfter", "mva", ArgType::String, "Move the layer after this one."))
        .flag(f("moveLayerBefore", "mvb", ArgType::String, "Move the layer before this one."))
        .flag(f("mute", "m", ArgType::Bool, "Mute the layer."))
        .flag(f("override", "o", ArgType::Bool, "Override what is below instead of adding."))
        .flag(f("parent", "p", ArgType::String, "The parent layer."))
        .flag(f("passthrough", "pth", ArgType::Bool, "An override layer's weight lets the layers below show through."))
        .flag(f("preferred", "prf", ArgType::Bool, "The best layer algorithm tries this layer first."))
        .flag(f("removeAllAttributes", "raa", ArgType::Bool, "Remove every channel."))
        .flag(f("removeAttribute", "ra", ArgType::Any, "Plug(s) to remove.").multi(1))
        .flag(f("removeSelectedObjects", "rso", ArgType::Bool, "Remove the selected objects' channels."))
        .flag(f("root", "r", ArgType::String, "Query: the base layer."))
        .flag(f("rotationAccumulationMode", "ram", ArgType::Any, "component (0) or layer (1)."))
        .flag(f("scaleAccumulationMode", "sam", ArgType::Any, "additive (0) or multiply (1)."))
        .flag(f("selected", "sel", ArgType::Bool, "A selected layer shows in the Time Slider and Graph Editor and takes keys."))
        .flag(f("solo", "s", ArgType::Bool, "Solo the layer among its siblings."))
        .flag(f("weight", "w", ArgType::Float, "The layer's weight, 0 to 1."))
        .flag(f("writeBlendnodeDestinations", "wbd", ArgType::Bool, "Write the plugs the blends drive into the blends.")),
    );
    crate::anim_layer_menu::register(c);
}
