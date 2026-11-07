//! What the Animation Layer Editor's menus do beyond the animLayer command: merging, extracting, selecting objects, the key
//! modes, and the snapshot the editor draws from. The reference's items run MEL procedures whose signatures aren't documented, so
//! `animLayerMerge` and the `layerEditor*` commands follow what the user guide says the items do.

use std::sync::atomic::Ordering;

use aztek_ops::{fail, ArgType, Args, CmdResult, Commands};
use glam::DVec3;
use serde_json::{json, Value};

use crate::anim::{self, f, Cmd, Key, Keys};
use crate::anim_layer_cmds::{copy_layer, create_layer, need_layer, remove_plugs, selected_nodes, split_axes, transfer};
use crate::anim_layers::*;
use crate::graph::Graph;
use crate::plug::{NodeId, PlugRef, PlugValue};
use crate::session::Session;

fn descendants(g: &Graph, l: NodeId, out: &mut Vec<NodeId>) {
    for c in layer_children(g, l) {
        if !out.contains(&c) {
            out.push(c);
        }
        descendants(g, c, out);
    }
}

/// One channel's result at each time, sampled before anything changes. A scalar's value is in x.
struct Sampled {
    plug: PlugRef,
    top: NodeId,
    times: Vec<f64>,
    values: Vec<DVec3>,
}

fn key_times_in(g: &Graph, p: &PlugRef, merged: &[NodeId]) -> Vec<f64> {
    let mut out: Vec<f64> = Vec::new();
    for (l, c) in plug_layer_curves(g, p) {
        if !merged.contains(&l) {
            continue;
        }
        if let Some(c) = c {
            out.extend(anim::keys_of(g, c).keys.iter().map(|k| k.t));
        }
        if let Some(w) = curve_in(g, &PlugRef::new(l, L_WEIGHT as i32)) {
            out.extend(anim::keys_of(g, w).keys.iter().map(|k| k.t));
        }
    }
    out.sort_by(f64::total_cmp);
    out.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
    out
}

/// animLayerMerge: the merged layers' result goes on one of them, the rest give up those channels.
fn merge_layers(s: &mut Session, a: &Args) -> CmdResult {
    let g = &mut s.graph;
    let order = ordered_layers(g);
    let mut layers: Vec<NodeId> = Vec::new();
    let named = a.named();
    if named.is_empty() {
        layers.extend(order.iter().copied().filter(|&l| layer_flag(g, l, L_SELECTED)));
        // One layer selected merges down into the base.
        if layers.len() == 1 {
            layers.extend(base_layer(g));
        }
    }
    for n in named {
        layers.push(need_layer(g, &n, "animLayerMerge")?);
    }
    let include_children = a.get("includeChildren").and_then(Value::as_bool).unwrap_or(true);
    if include_children {
        for l in layers.clone() {
            if Some(l) != base_layer(g) {
                descendants(g, l, &mut layers);
            }
        }
    }
    layers.sort_by_key(|l| order.iter().position(|x| x == l).unwrap_or(0));
    layers.dedup();
    if layers.len() < 2 {
        fail!("animLayerMerge: name two or more layers");
    }
    let base = base_layer(g);
    let to_top = a.string("mergeTo", "bottom") == "top";
    let target = if to_top { *layers.last().unwrap() } else { layers[0] };
    let target_name = g.path(target);
    // The base gives up nothing unless it is what is merged into.
    let sources: Vec<NodeId> = layers.iter().copied().filter(|&l| l != target && Some(l) != base).collect();
    let by = a.float("sampleBy", 1.0).max(1e-3);
    let smart = a.bool("smart");
    let tolerance = a.float("fidelity", 0.01).max(1e-6);
    let mode = a.string("resultLayerMode", "automatic");
    let override_result = match mode.as_str() {
        "additive" => false,
        "override" => true,
        _ => layers.iter().any(|&l| Some(l) != base && layer_flag(g, l, L_OVERRIDE)),
    };

    // Every channel in the layers going away, and what it comes to now.
    let mut plugs: Vec<PlugRef> = Vec::new();
    for &l in &sources {
        for b in layer_blends(g, l) {
            for p in split_axes(g, blend_plugs(g, b)) {
                if !plugs.contains(&p) {
                    plugs.push(p);
                }
            }
        }
    }
    let none = Skip::new();
    let mut sampled: Vec<Sampled> = Vec::new();
    for &p in &plugs {
        let c = chain(g, &p);
        let Some(&top) = c.iter().find(|&&b| blend_layer(g, b).is_some_and(|l| layers.contains(&l))) else { continue };
        let keys = key_times_in(g, &p, &layers);
        let (Some(&t0), Some(&t1)) = (keys.first(), keys.last()) else { continue };
        let mut times: Vec<f64> = Vec::new();
        let n = ((t1 - t0) / by).ceil() as usize;
        for i in 0..=n {
            times.push((t0 + i as f64 * by).min(t1));
        }
        times.dedup_by(|x, y| (*x - *y).abs() < 1e-9);
        let values = times
            .iter()
            .map(|&t| if is_rotation(g, top) { blend_vec_at(g, top, t, &none) } else { DVec3::new(blend_at(g, top, &p, t, &none), 0.0, 0.0) })
            .collect();
        sampled.push(Sampled { plug: p, top, times, values });
    }

    g.open_chunk("Merge Layers");
    let result = (|| -> CmdResult<()> {
        let g = &mut s.graph;
        if Some(target) != base {
            g.set_value(PlugRef::new(target, L_OVERRIDE as i32), PlugValue::Bool(override_result))?;
            g.set_value(PlugRef::new(target, L_MUTE as i32), PlugValue::Bool(false))?;
            g.disconnect(&PlugRef::new(target, L_WEIGHT as i32));
            g.set_value(PlugRef::new(target, L_WEIGHT as i32), PlugValue::Float(1.0))?;
            g.set_value(PlugRef::new(target, L_SOLO as i32), PlugValue::Bool(false))?;
        }
        for x in &sampled {
            let p = x.plug;
            add_plug(g, target, &p)?;
            let chain_now = chain(g, &p);
            let skip: Skip = chain_now.iter().copied().filter(|&b| blend_layer(g, b).is_some_and(|l| sources.contains(&l))).collect();
            let solved: Vec<(f64, f64)> = x
                .times
                .iter()
                .zip(&x.values)
                .map(|(&t, v)| {
                    let value = if is_rotation(g, x.top) {
                        resolve_vec(g, &p, Some(target), *v, t, &skip, Some(x.top))[p.component.max(0) as usize]
                    } else {
                        resolve(g, &p, Some(target), v.x, t, &skip, Some(x.top))
                    };
                    (t, value)
                })
                .collect();
            let r = anim::range();
            let keyed = |list: &[(f64, f64)]| {
                let mut k = Keys { keys: list.iter().map(|&(t, v)| Key::new(t, v, r.default_in, r.default_out)).collect(), weighted: false };
                k.sort();
                k
            };
            let baked = if smart {
                // Keys where the sources have them, more only where the curve strays past the tolerance.
                let at = key_times_in(g, &p, &layers);
                let mut kept: Vec<(f64, f64)> = solved.iter().copied().filter(|(t, _)| at.iter().any(|y| (y - t).abs() < 1e-6)).collect();
                let k = keyed(&kept);
                let (pre, post) = (anim::infinity(0), anim::infinity(0));
                kept.extend(solved.iter().copied().filter(|(t, v)| (k.eval(*t, pre, post) - v).abs() > tolerance));
                kept.sort_by(|x, y| x.0.total_cmp(&y.0));
                kept.dedup_by(|x, y| (x.0 - y.0).abs() < 1e-9);
                keyed(&kept)
            } else {
                keyed(&solved)
            };
            if let Some(c) = curve_for(g, &p, Some(target))? {
                anim::set_keys(g, c, baked)?;
            }
        }
        let delete = a.get("deleteBaked").and_then(Value::as_bool).unwrap_or(true);
        for &l in &sources {
            // Without Include Children a parent keeps its place for the children left under it.
            let has_kids = !layer_children(g, l).is_empty() && !include_children;
            if delete && !has_kids {
                before_delete(g, l)?;
                g.delete_node(l)?;
            } else {
                for b in layer_blends(g, l) {
                    remove_blend(g, b)?;
                }
            }
        }
        after_delete(g)?;
        Ok(())
    })();
    s.graph.close_chunk();
    result?;
    Ok(json!(target_name))
}

/// Extract Selected Objects: off each selected layer onto a new one named `<layer>_extract`.
fn extract_objects(s: &mut Session, a: &Args) -> CmdResult {
    let mut sources: Vec<NodeId> = Vec::new();
    for n in a.named() {
        sources.push(need_layer(&s.graph, &n, "layerEditorExtractObjects")?);
    }
    if sources.is_empty() {
        let g = &s.graph;
        sources = ordered_layers(g).into_iter().filter(|&l| layer_flag(g, l, L_SELECTED) && Some(l) != base_layer(g)).collect();
    }
    let nodes = selected_nodes(s, false);
    let mut made = Vec::new();
    s.graph.open_chunk("Extract Selected Objects");
    let r = (|| -> CmdResult<()> {
        for from in sources {
            let name = format!("{}_extract", s.graph.node(from).map(|n| n.name.clone()).unwrap_or_default());
            let l = create_layer(s, &name)?;
            let g = &mut s.graph;
            for key in [L_OVERRIDE, L_PASSTHROUGH, L_RAM, L_SAM] {
                let v = g.node(from).map(|n| n.value(key).clone());
                if let Some(v) = v {
                    g.set_value(PlugRef::new(l, key as i32), v)?;
                }
            }
            let mine: Vec<PlugRef> = layer_blends(g, from).into_iter().flat_map(|b| blend_plugs(g, b)).filter(|p| nodes.contains(&p.node)).collect();
            for p in &mine {
                add_plug(g, l, &p)?;
            }
            for p in &mine {
                if let Some(c) = slot(g, p, Some(from)).and_then(|sl| curve_in(g, &sl)) {
                    let k = anim::keys_of(g, c);
                    if let Some(n) = curve_for(g, p, Some(l))? {
                        anim::set_keys(g, n, k)?;
                    }
                }
            }
            remove_plugs(g, from, &mine)?;
            made.push(g.path(l));
        }
        Ok(())
    })();
    s.graph.close_chunk();
    r?;
    Ok(json!(made))
}

/// Extract Non-Layered Animation: the objects' curves go from the base onto a new layer. The guide doesn't say what the base
/// keeps or the layer's mode; here the layer overrides, so the result stays as it was.
fn extract_non_layered(s: &mut Session, a: &Args) -> CmdResult {
    let nodes: Vec<NodeId> = if a.bool("all") {
        let g = &s.graph;
        g.nodes().into_iter().filter(|&n| !anim::curves_on(g, n).is_empty() && g.node(n).is_some_and(|x| x.ty().dag)).collect()
    } else {
        selected_nodes(s, false)
    };
    s.graph.open_chunk("Extract Non-Layered Animation");
    let r = (|| -> CmdResult<NodeId> {
        let l = create_layer(s, "AnimLayer1")?;
        let g = &mut s.graph;
        g.set_value(PlugRef::new(l, L_OVERRIDE as i32), PlugValue::Bool(true))?;
        for &n in &nodes {
            for (p, c) in anim::curves_on(g, n) {
                if !chain(g, &p).is_empty() && slot(g, &p, None).and_then(|x| curve_in(g, &x)) != Some(c) {
                    continue;
                }
                let k = anim::keys_of(g, c);
                add_plug(g, l, &p)?;
                if let Some(to) = curve_for(g, &p, Some(l))? {
                    anim::set_keys(g, to, k)?;
                }
                let v = g.value(&p)?.scalar().unwrap_or(0.0);
                g.delete_node(c)?;
                if let Some(sl) = slot(g, &p, None) {
                    set_num(g, sl, v)?;
                }
            }
        }
        Ok(l)
    })();
    s.graph.close_chunk();
    let l = r?;
    Ok(json!(s.graph.path(l)))
}

fn the_layers(s: &Session, a: &Args, cmd: &str) -> CmdResult<Vec<NodeId>> {
    let mut out = Vec::new();
    for n in a.named() {
        out.push(need_layer(&s.graph, &n, cmd)?);
    }
    if out.is_empty() {
        let g = &s.graph;
        out = ordered_layers(g).into_iter().filter(|&l| layer_flag(g, l, L_SELECTED)).collect();
    }
    Ok(out)
}

/// What the editor draws: every layer, base first, with its state and how it relates to the selection.
pub fn layer_info(g: &mut Graph) -> Value {
    let order = ordered_layers(g);
    let base = base_layer(g);
    let selection: Vec<NodeId> = g.selection().iter().map(|&i| crate::dag::transform_of(g, i)).collect();
    let plugs: Vec<PlugRef> = selection.iter().flat_map(|&n| anim::keyable_plugs(g, n)).collect();
    let now = anim::frame(g);
    let mut rows = Vec::new();
    for &l in &order {
        let weight_plug = PlugRef::new(l, L_WEIGHT as i32);
        let weight_curve = curve_in(g, &weight_plug);
        let weight = weight_curve.map(|c| curve_at(g, c, now)).unwrap_or_else(|| g.stored_value(&weight_plug).scalar().unwrap_or(1.0));
        let weight_key = weight_curve.is_some_and(|c| anim::keys_of(g, c).keys.iter().any(|k| (k.t - now).abs() < 1e-6));
        let in_layer = plugs.iter().any(|p| plug_layer_curves(g, p).iter().any(|(x, _)| *x == l));
        let active = plugs.iter().any(|p| key_target(g, p) == Some(l));
        let n = g.node(l).unwrap();
        let mut depth = 0;
        let mut up = layer_parent(g, l);
        while let Some(x) = up {
            depth += 1;
            up = layer_parent(g, x);
        }
        rows.push(json!({
            "name": n.name, "base": Some(l) == base, "depth": depth, "parent": layer_parent(g, l).and_then(|p| g.node(p)).map(|p| p.name.clone()),
            "children": layer_children(g, l).len(), "mute": layer_flag(g, l, L_MUTE), "solo": layer_flag(g, l, L_SOLO), "lock": layer_flag(g, l, L_LOCK),
            "ghost": layer_flag(g, l, L_GHOST), "ghostColor": g.stored_value(&PlugRef::new(l, L_GHOST_COLOR as i32)).scalar().unwrap_or(5.0),
            "override": layer_flag(g, l, L_OVERRIDE), "passthrough": layer_flag(g, l, L_PASSTHROUGH), "selected": layer_flag(g, l, L_SELECTED),
            "collapse": layer_flag(g, l, L_COLLAPSE), "ram": g.stored_value(&PlugRef::new(l, L_RAM as i32)).scalar().unwrap_or(0.0),
            "sam": g.stored_value(&PlugRef::new(l, L_SAM as i32)).scalar().unwrap_or(1.0), "weight": weight, "weightAnimated": weight_curve.is_some(), "weightKey": weight_key,
            "active": active, "affected": in_layer && !active, "channels": layer_blends(g, l).len(),
        }));
    }
    json!({"layers": rows, "keyMode": KEY_MODE.load(Ordering::Relaxed), "lockMuted": LOCK_MUTED.load(Ordering::Relaxed)})
}

pub fn register(c: &mut Commands<Session>) {
    let merge = |name: &str| {
        Cmd::new(
            name,
            "Merge layers into one, keeping the result: -mt top or bottom selected layer, -ic children too, -rlm automatic, additive or override, -db delete the merged layers, -sb sample by, -sm keys only where needed, -fd its tolerance. \
             With none named, the selected layers. Returns the layer merged into.",
            merge_layers,
        )
        .positional("the layers")
        .flag(f("mergeTo", "mt", ArgType::String, "top or bottom (the default)."))
        .flag(f("includeChildren", "ic", ArgType::Bool, "Merge the children of merged layers too (on by default)."))
        .flag(f("resultLayerMode", "rlm", ArgType::String, "automatic, additive or override."))
        .flag(f("deleteBaked", "db", ArgType::Bool, "Delete the merged layers (on by default)."))
        .flag(f("sampleBy", "sb", ArgType::Float, "Frames between samples (1)."))
        .flag(f("smart", "sm", ArgType::Bool, "Keep the sources' key times, adding keys only past the tolerance."))
        .flag(f("fidelity", "fd", ArgType::Float, "The smart bake's tolerance."))
    };
    c.register(merge("animLayerMerge"));
    c.register(merge("mergeAnimLayers"));
    c.register(Cmd::new("layerEditorExtractObjects", "Move the selected objects off the named (or selected) layers onto new `<layer>_extract` layers. Returns the new layers.", extract_objects).positional("the layers"));
    c.register(
        Cmd::new("layerEditorExtractNonLayered", "Move the non-layered animation of the selected objects, or -all objects, onto a new layer. Returns the layer.", extract_non_layered)
            .flag(f("all", "a", ArgType::Bool, "Every animated object.")),
    );
    c.register(
        Cmd::new("layerEditorSelectObjects", "Select every object with channels on the named (or selected) layers.", |s, a| {
            let layers = the_layers(s, a, "layerEditorSelectObjects")?;
            let g = &s.graph;
            let mut names: Vec<String> = Vec::new();
            for l in layers {
                for b in layer_blends(g, l) {
                    for p in blend_plugs(g, b) {
                        let n = g.path(p.node);
                        if !names.contains(&n) {
                            names.push(n);
                        }
                    }
                }
            }
            s.run("select", json!({"args": names.clone()}))?;
            Ok(json!(names))
        })
        .positional("the layers"),
    );
    c.register(
        Cmd::new("layerEditorCopyLayer", "Duplicate the named (or selected) layers: with their animation, or with -cna without. Returns the copies.", |s, a| {
            let layers = the_layers(s, a, "layerEditorCopyLayer")?;
            let mut out = Vec::new();
            s.graph.open_chunk("Copy Layer");
            let r = (|| -> CmdResult<()> {
                for from in layers {
                    let name = format!("{}_copy", s.graph.node(from).map(|n| n.name.clone()).unwrap_or_default());
                    let l = create_layer(s, &name)?;
                    copy_layer(&mut s.graph, from, l, !a.bool("noAnimation"))?;
                    out.push(s.graph.path(l));
                }
                Ok(())
            })();
            s.graph.close_chunk();
            r?;
            Ok(json!(out))
        })
        .positional("the layers")
        .flag(f("noAnimation", "cna", ArgType::Bool, "Channels only.")),
    );
    c.register(
        Cmd::new("layerEditorEmpty", "Remove every channel and curve from the named (or selected) layers, keeping the layers.", |s, a| {
            for l in the_layers(s, a, "layerEditorEmpty")? {
                for b in layer_blends(&s.graph, l) {
                    remove_blend(&mut s.graph, b)?;
                }
            }
            Ok(Value::Null)
        })
        .positional("the layers"),
    );
    c.register(Cmd::new("layerEditorDeleteEmpty", "Delete every layer with no channels. Returns how many.", |s, _a| {
        let g = &mut s.graph;
        let base = base_layer(g);
        let doomed: Vec<NodeId> = ordered_layers(g).into_iter().filter(|&l| Some(l) != base && layer_blends(g, l).is_empty() && layer_children(g, l).is_empty()).collect();
        for &l in &doomed {
            before_delete(g, l)?;
            g.delete_node(l)?;
        }
        after_delete(g)?;
        Ok(json!(doomed.len()))
    }));
    c.register(
        Cmd::new("layerEditorMoveToNewLayer", "Move the selected objects from the layers they are in to one new layer, curves included. Returns it.", |s, a| {
            let nodes = selected_nodes(s, false);
            s.graph.open_chunk("Move to New Layer");
            let r = (|| -> CmdResult<NodeId> {
                let name = a.named().first().cloned().unwrap_or_else(|| "AnimLayer1".into());
                let l = create_layer(s, &name)?;
                let g = &mut s.graph;
                let others: Vec<NodeId> = ordered_layers(g).into_iter().filter(|&x| x != l && Some(x) != base_layer(g)).collect();
                for from in others {
                    let mine: Vec<PlugRef> = layer_blends(g, from).into_iter().flat_map(|b| blend_plugs(g, b)).filter(|p| nodes.contains(&p.node)).collect();
                    for p in &mine {
                        add_plug(g, l, &p)?;
                    }
                    transfer(g, from, l, false)?;
                    remove_plugs(g, from, &mine)?;
                }
                Ok(l)
            })();
            s.graph.close_chunk();
            let l = r?;
            Ok(json!(s.graph.path(l)))
        })
        .positional("the new layer's name"),
    );
    c.register(
        Cmd::new("animLayerOptions", "The Animation Layer Editor's Options menu: -km selected, active or hybrid; -lm Lock Muted Layer. With nothing, the editor's snapshot.", |s, a| {
            if let Some(m) = a.get("keyMode").and_then(Value::as_str) {
                KEY_MODE.store(match m { "selected" => 0, "active" => 1, _ => 2 }, Ordering::Relaxed);
            }
            if let Some(m) = a.get("showFilter").and_then(Value::as_str) {
                SHOW_FILTER.store(match m { "active" => 1, "selected" => 2, "activeAndSelected" => 3, _ => 0 }, Ordering::Relaxed);
            }
            if let Some(b) = a.get("lockMuted").and_then(Value::as_bool) {
                LOCK_MUTED.store(b, Ordering::Relaxed);
            }
            Ok(layer_info(&mut s.graph))
        })
        .undoable(false)
        .flag(f("keyMode", "km", ArgType::String, "selected, active or hybrid."))
        .flag(f("showFilter", "sf", ArgType::String, "allAffecting, active, selected or activeAndSelected: the curves other tools show."))
        .flag(f("lockMuted", "lm", ArgType::Bool, "Muted layers take no keys.")),
    );
}
