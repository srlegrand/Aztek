//! The reference's display layers. A `displayLayer` node (non-DAG) holds the layer's
//! switches; an object joins one by having the layer's drawInfo connected to its drawOverride plug. While connected,
//! the layer's values replace the object's own drawing overrides (`draw_style::override_on`). An object is in at most
//! one layer, and a layer on a group governs everything under it unless a child has a layer of its own.
//!
//!   visibility       off hides the members, like their own visibility.
//!   hideOnPlayback   hides the members only while the animation plays (the panel's P).
//!   displayType      normal; template: grey wire that can't be picked; reference: drawn as usual but can't be picked.
//!   color            palette index for the members' wireframe; 0 is no colour.
//!   overrideRGBColors, overrideColorRGB   an RGB colour in place of the index.
//!   levelOfDetail, shading, texturing, playback, enabled   as the drawing overrides they drive.
//!   displayOrder     row order in the Display Layers panel, lowest first.

use std::any::Any;
use std::sync::Arc;

use aztek_ops::{fail, objects, ArgType as T, Args, CmdResult, CommandDef, CommandError, Commands, FlagDef};
use glam::DVec3;
use serde_json::{json, Value};

use crate::graph::{Graph, SelectMode};
use crate::node::{types, EvalContext, Node, NodeBehavior, NodeData, NodeType};
use crate::plug::{flags, AttrDef, NodeId, PlugRef, PlugType, PlugValue, NO_NODE};
use crate::session::Session;
use crate::transform::Transform;

pub const VISIBILITY: usize = 0;
pub const HIDE_ON_PLAYBACK: usize = 1;
pub const DISPLAY_TYPE: usize = 2;
pub const COLOR: usize = 3;
pub const DISPLAY_ORDER: usize = 4;
pub const DRAW_INFO: usize = 5;
pub const LEVEL_OF_DETAIL: usize = 6;
pub const SHADING: usize = 7;
pub const TEXTURING: usize = 8;
pub const PLAYBACK: usize = 9;
pub const ENABLED: usize = 10;
pub const OVERRIDE_RGB_COLORS: usize = 11;
pub const OVERRIDE_COLOR_RGB: usize = 12;
pub const OVERRIDE_COLOR_A: usize = 13;
pub const IDENTIFICATION: usize = 14;
pub const COUNT: usize = 15;

pub const TYPE_NAME: &str = "displayLayer";
/// The reference's implicit layer: naming it as the target of a membership edit takes objects out of their layer.
pub const DEFAULT_LAYER: &str = "defaultLayer";

const DISPLAY_TYPE_NAMES: [&str; 3] = ["normal", "template", "reference"];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DisplayType {
    #[default]
    Normal,
    Template,
    Reference,
}

impl DisplayType {
    fn from_index(i: i64) -> Self {
        match i {
            1 => Self::Template,
            2 => Self::Reference,
            _ => Self::Normal,
        }
    }
}

pub use crate::draw_style::set_play_direction;

struct DisplayLayerNode;

impl NodeBehavior for DisplayLayerNode {
    fn compute(&mut self, _ctx: &EvalContext, _d: &mut NodeData) {}
    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn has_layer_plug(n: &Node) -> bool {
    n.ty().dag
}

pub fn is_layer(n: &Node) -> bool {
    n.ty().is_a(TYPE_NAME)
}

/// Every layer in display order (then name).
pub fn all(g: &Graph) -> Vec<NodeId> {
    let mut out: Vec<NodeId> = g.nodes().into_iter().filter(|&id| g.node(id).is_some_and(is_layer)).collect();
    out.sort_by(|&a, &b| {
        let (na, nb) = (g.node(a).unwrap(), g.node(b).unwrap());
        na.int(DISPLAY_ORDER).cmp(&nb.int(DISPLAY_ORDER)).then_with(|| na.name().cmp(nb.name()))
    });
    out
}

/// The layer the node itself is in.
pub fn layer_of(g: &Graph, id: NodeId) -> Option<NodeId> {
    let n = g.node(id)?;
    if !has_layer_plug(n) {
        return None;
    }
    let src = g.source(&PlugRef::new(id, Transform::DISPLAY_LAYER as i32))?;
    g.node(src.node).filter(|l| is_layer(l)).map(|_| src.node)
}

/// Nodes whose own layer is this one, in node order.
pub fn members(g: &Graph, layer: NodeId) -> Vec<NodeId> {
    let mut out: Vec<NodeId> = g
        .destinations(&PlugRef::new(layer, DRAW_INFO as i32))
        .into_iter()
        .filter(|d| d.attr == Transform::DISPLAY_LAYER as i32)
        .map(|d| d.node)
        .collect();
    out.sort();
    out
}

/// How the layer governing a node (its own, else the nearest ancestor's) draws it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Display {
    pub ty: DisplayType,
    /// The layer's wire colour, 0 to 1; None when it has none.
    pub color: Option<[f64; 3]>,
}

/// The colour a layer gives: RGB when overrideRGBColors is on, else its palette index, where 0 is none.
pub fn layer_color(l: &Node) -> Option<[f64; 3]> {
    if l.bool(OVERRIDE_RGB_COLORS) {
        return Some(l.vec3(OVERRIDE_COLOR_RGB).to_array());
    }
    let i = l.int(COLOR);
    (i > 0).then(|| crate::draw_style::palette(i))
}

pub fn display(g: &Graph, id: NodeId) -> Display {
    let mut n = id;
    while n != NO_NODE {
        if let Some(layer) = layer_of(g, n) {
            let l = g.node(layer).unwrap();
            return Display {
                ty: DisplayType::from_index(l.int(DISPLAY_TYPE)),
                color: layer_color(l),
            };
        }
        n = g.parent(n);
    }
    Display::default()
}

/// False for template and reference members: the viewport doesn't pick them, though lists and scripts still can.
pub fn pickable(g: &Graph, id: NodeId) -> bool {
    display(g, id).ty == DisplayType::Normal
}

/// Undoable. Takes the nodes out of any layer they were in.
pub fn create(g: &mut Graph, name: &str, nodes: &[NodeId]) -> CmdResult<NodeId> {
    let order = all(g).iter().map(|&l| g.node(l).unwrap().int(DISPLAY_ORDER) + 1).max().unwrap_or(0);
    let ty = types::find(TYPE_NAME).expect("displayLayer registered");
    let layer = g.create_node(&ty, if name.is_empty() { "layer1" } else { name }, NO_NODE, true)?;
    if order != 0 {
        g.set_value(PlugRef::new(layer, DISPLAY_ORDER as i32), PlugValue::Int(order))?;
    }
    add_members(g, layer, nodes)?;
    Ok(layer)
}

pub fn add_members(g: &mut Graph, layer: NodeId, nodes: &[NodeId]) -> CmdResult<()> {
    for &id in nodes {
        if !g.node(id).is_some_and(has_layer_plug) {
            continue;
        }
        let dst = PlugRef::new(id, Transform::DISPLAY_LAYER as i32);
        if g.source(&dst).is_some_and(|s| s.node == layer) {
            continue;
        }
        g.connect(&PlugRef::new(layer, DRAW_INFO as i32), &dst, true)?;
    }
    Ok(())
}

pub fn remove_members(g: &mut Graph, nodes: &[NodeId]) {
    for &id in nodes {
        if layer_of(g, id).is_some() {
            g.disconnect(&PlugRef::new(id, Transform::DISPLAY_LAYER as i32));
        }
    }
}

/// Members go back to no layer; the node is deleted.
pub fn remove(g: &mut Graph, layer: NodeId) -> CmdResult<()> {
    let m = members(g, layer);
    remove_members(g, &m);
    g.delete_node(layer)?;
    Ok(())
}

/// Moves the layer to index among all() and renumbers every layer's displayOrder 0, 1, 2, ...
pub fn reorder(g: &mut Graph, layer: NodeId, index: usize) -> CmdResult<()> {
    let mut layers = all(g);
    let Some(at) = layers.iter().position(|&l| l == layer) else { return Ok(()) };
    let moved = layers.remove(at);
    let index = index.min(layers.len());
    layers.insert(index, moved);
    for (i, &l) in layers.iter().enumerate() {
        if g.node(l).unwrap().int(DISPLAY_ORDER) != i as i64 {
            g.set_value(PlugRef::new(l, DISPLAY_ORDER as i32), PlugValue::Int(i as i64))?;
        }
    }
    Ok(())
}

fn names_of(g: &Graph, ids: &[NodeId]) -> Value {
    Value::Array(ids.iter().map(|&id| Value::String(g.path(id))).collect())
}

/// { name, visibility, hideOnPlayback, displayType, color (RGB, null without one), colorIndex, order, members }.
pub fn info(g: &Graph, layer: NodeId) -> Value {
    let n = g.node(layer).unwrap();
    let color = layer_color(n).map_or(Value::Null, |c| json!(c));
    json!({
        "name": g.path(layer),
        "visibility": n.bool(VISIBILITY),
        "hideOnPlayback": n.bool(HIDE_ON_PLAYBACK),
        "displayType": DISPLAY_TYPE_NAMES[n.int(DISPLAY_TYPE).clamp(0, 2) as usize],
        "color": color,
        "colorIndex": n.int(COLOR),
        "order": n.int(DISPLAY_ORDER),
        "members": names_of(g, &members(g, layer)),
    })
}

fn layer_attrs() -> Vec<AttrDef> {
    let stored = flags::INPUT | flags::STORABLE;
    let attr = |name: &str, short: &str, ty: PlugType, default: PlugValue, f: u32, doc: &str| AttrDef {
        name: name.into(),
        short_name: short.into(),
        ty,
        default_value: default,
        flags: f,
        doc: doc.into(),
        ..Default::default()
    };
    vec![
        attr("visibility", "v", PlugType::Bool, PlugValue::Bool(true), stored | flags::KEYABLE, "Off hides the layer's objects (V)."),
        attr(
            "hideOnPlayback",
            "hpb",
            PlugType::Bool,
            PlugValue::Bool(false),
            stored,
            "Hide the layer's objects while the animation plays (P).",
        ),
        AttrDef {
            enum_names: DISPLAY_TYPE_NAMES.iter().map(|s| s.to_string()).collect(),
            ..attr(
                "displayType",
                "dt",
                PlugType::Enum,
                PlugValue::Int(0),
                stored,
                "normal; template: grey wireframe, not pickable in the viewport; reference: drawn as usual, not pickable.",
            )
        },
        AttrDef {
            min_value: Some(0.0),
            max_value: Some(31.0),
            ..attr("color", "c", PlugType::Int, PlugValue::Int(0), stored, "Palette index the objects draw in, 0 to 31; 0 is no colour.")
        },
        AttrDef {
            min_value: Some(0.0),
            ..attr("displayOrder", "do", PlugType::Int, PlugValue::Int(0), stored, "Row in the Display Layers panel, lowest first.")
        },
        attr(
            "drawInfo",
            "di",
            PlugType::Int,
            PlugValue::Int(0),
            flags::OUTPUT | flags::HIDDEN,
            "All the layer's drawing settings as one plug: connected to each member's drawOverride.",
        ),
        AttrDef {
            enum_names: vec!["Full".into(), "Bounding Box".into()],
            ..attr("levelOfDetail", "lod", PlugType::Enum, PlugValue::Int(0), stored, "Full, or drawn as a bounding box.")
        },
        attr("shading", "s", PlugType::Bool, PlugValue::Bool(true), stored, "Off draws a wireframe in shaded display."),
        attr("texturing", "t", PlugType::Bool, PlugValue::Bool(true), stored, "Off leaves textures out in shaded display."),
        attr("playback", "p", PlugType::Bool, PlugValue::Bool(true), stored, "Off leaves the objects out of the view during playback."),
        attr("enabled", "e", PlugType::Bool, PlugValue::Bool(true), stored, "Off turns the effect of the other attributes off."),
        attr("overrideRGBColors", "ovrgbf", PlugType::Bool, PlugValue::Bool(false), stored, "Take the colour from overrideColorRGB instead of the index."),
        AttrDef {
            child_names: ["overrideColorR".into(), "overrideColorG".into(), "overrideColorB".into()],
            child_short: ["ovcr".into(), "ovcg".into(), "ovcb".into()],
            min_value: Some(0.0),
            max_value: Some(1.0),
            ..attr("overrideColorRGB", "ovrgb", PlugType::Vec3, PlugValue::Vec3(DVec3::ZERO), stored | flags::COLOR, "The override colour, 0 to 1 per channel.")
        },
        AttrDef {
            min_value: Some(0.0),
            max_value: Some(1.0),
            ..attr("overrideColorA", "ovca", PlugType::Float, PlugValue::Float(1.0), stored, "The override colour's alpha, if supported.")
        },
        attr("identification", "id", PlugType::Int, PlugValue::Int(0), stored, "Display layer identification number."),
    ]
}

pub fn register_types() {
    if types::find(TYPE_NAME).is_some() {
        return;
    }
    let t = NodeType {
        name: TYPE_NAME.into(),
        icon: "stack-2".into(),
        doc: "A display layer: shows, hides, templates or references its objects together. Edit it in Windows > Display \
              Layers."
            .into(),
        own_attrs: layer_attrs(),
        create: Some(Arc::new(|| Box::new(DisplayLayerNode))),
        ..Default::default()
    };
    types::register_type(t).expect("displayLayer");
}

fn layer_arg(g: &Graph, name: &str, cmd: &str) -> CmdResult<NodeId> {
    match g.find(name) {
        Ok(Some(id)) if g.node(id).is_some_and(is_layer) => Ok(id),
        _ => fail!("{cmd}: {name} is not a display layer"),
    }
}

fn object_ids(g: &Graph, names: &[String], cmd: &str) -> CmdResult<Vec<NodeId>> {
    let mut out = Vec::new();
    for name in names {
        let Some(id) = g.find(name)? else { fail!("{cmd}: no object named {name}") };
        if !has_layer_plug(g.node(id).unwrap()) {
            fail!("{cmd}: {name} is not a DAG object");
        }
        out.push(id);
    }
    Ok(out)
}

fn display_type_arg(v: &str, cmd: &str) -> CmdResult<i64> {
    match DISPLAY_TYPE_NAMES.iter().position(|&n| n == v) {
        Some(i) => Ok(i as i64),
        None => fail!("{cmd}: displayType must be normal, template or reference"),
    }
}

fn first_positional(a: &Args, cmd: &str, what: &str) -> CmdResult<String> {
    match a.positional.first().and_then(Value::as_str) {
        Some(s) => Ok(s.to_string()),
        None => fail!("{cmd}: {what}"),
    }
}

fn positional_strings(a: &Args) -> Vec<String> {
    a.positional.iter().filter_map(Value::as_str).map(str::to_string).collect()
}

fn flag(name: &str, short: &str, ty: T, default: Value, doc: &str) -> FlagDef {
    FlagDef::new(name, short, ty, default, doc)
}

pub fn register(c: &mut Commands<Session>) {
    c.register(
        CommandDef::<Session>::new(
            "createDisplayLayer",
            "Create a display layer holding the objects (the selection by default), taking them out of any layer they were \
             in. Returns its name.",
            |s, a| {
                let ids = if a.bool("empty") { Vec::new() } else { object_ids(&s.graph, &objects(s, a), "createDisplayLayer")? };
                let layer = create(&mut s.graph, &a.string("name", ""), &ids)
                    .map_err(|e| CommandError::new(format!("createDisplayLayer: {e}")))?;
                Ok(Value::String(s.graph.path(layer)))
            },
        )
        .positional("objects")
        .flag(flag("name", "n", T::String, json!(""), "Name; layer1 when empty, made unique."))
        .flag(flag("empty", "e", T::Bool, json!(false), "Make it with no members, whatever is selected.")),
    );

    c.register(
        CommandDef::<Session>::new(
            "editDisplayLayerMembers",
            "Put objects (the selection by default) in a display layer, moving them out of their old one. Naming defaultLayer \
             takes them out of any layer. Returns the layer's members.",
            |s, a| {
                let layer_name = first_positional(a, "editDisplayLayerMembers", "name the layer first")?;
                let mut rest = a.clone();
                rest.positional.remove(0);
                let ids = object_ids(&s.graph, &objects(s, &rest), "editDisplayLayerMembers")?;
                if layer_name == DEFAULT_LAYER {
                    remove_members(&mut s.graph, &ids);
                    return Ok(json!([]));
                }
                let layer = layer_arg(&s.graph, &layer_name, "editDisplayLayerMembers")?;
                add_members(&mut s.graph, layer, &ids)?;
                Ok(names_of(&s.graph, &members(&s.graph, layer)))
            },
        )
        .positional("layer, objects"),
    );

    c.register(
        CommandDef::<Session>::new("removeFromDisplayLayer", "Take objects (the selection by default) out of their display layers.", |s, a| {
            let ids = object_ids(&s.graph, &objects(s, a), "removeFromDisplayLayer")?;
            remove_members(&mut s.graph, &ids);
            Ok(Value::Null)
        })
        .positional("objects"),
    );

    c.register(
        CommandDef::<Session>::new("deleteDisplayLayer", "Delete display layers; their objects stay, in no layer.", |s, a| {
            let mut layers = Vec::new();
            for name in positional_strings(a) {
                layers.push(layer_arg(&s.graph, &name, "deleteDisplayLayer")?);
            }
            if layers.is_empty() {
                fail!("deleteDisplayLayer: name a layer");
            }
            for id in layers {
                remove(&mut s.graph, id)?;
            }
            Ok(Value::Null)
        })
        .positional("layers"),
    );

    c.register(
        CommandDef::<Session>::new(
            "displayLayer",
            "Edit a display layer's switches (V, P, display type, colour, name); with no flags, query it. Returns its info: \
             name, visibility, hideOnPlayback, displayType, color (RGB, null for none), colorIndex, order and members.",
            |s, a| {
                let name = first_positional(a, "displayLayer", "name the layer")?;
                let id = layer_arg(&s.graph, &name, "displayLayer")?;
                let g = &mut s.graph;
                let set = |g: &mut Graph, attr: usize, v: PlugValue| -> CmdResult<()> {
                    if *g.node(id).unwrap().value(attr) != v {
                        g.set_value(PlugRef::new(id, attr as i32), v)?;
                    }
                    Ok(())
                };
                if a.has("visibility") {
                    set(g, VISIBILITY, PlugValue::Bool(a.bool("visibility")))?;
                }
                if a.has("hideOnPlayback") {
                    set(g, HIDE_ON_PLAYBACK, PlugValue::Bool(a.bool("hideOnPlayback")))?;
                }
                if a.has("displayType") {
                    set(g, DISPLAY_TYPE, PlugValue::Int(display_type_arg(&a.string("displayType", ""), "displayLayer")?))?;
                }
                if a.has("color") {
                    let i = a.int("color", 0);
                    if !(0..=31).contains(&i) {
                        fail!("displayLayer: color is a palette index, 0 to 31");
                    }
                    set(g, COLOR, PlugValue::Int(i))?;
                    set(g, OVERRIDE_RGB_COLORS, PlugValue::Bool(false))?;
                }
                if a.has("rgbColor") {
                    let Some(c) = a.vec3("rgbColor") else { fail!("displayLayer: rgbColor takes [r, g, b]") };
                    set(g, OVERRIDE_COLOR_RGB, PlugValue::Vec3(DVec3::from_array(c)))?;
                    set(g, OVERRIDE_RGB_COLORS, PlugValue::Bool(true))?;
                }
                if a.has("name") {
                    g.rename(id, &a.string("name", "")).map_err(|e| CommandError::new(format!("displayLayer: {e}")))?;
                }
                Ok(info(g, id))
            },
        )
        .positional("layer")
        .flag(flag("visibility", "v", T::Bool, Value::Null, "Show or hide its objects."))
        .flag(flag("hideOnPlayback", "hpb", T::Bool, Value::Null, "Hide its objects while the animation plays."))
        .flag(flag("displayType", "dt", T::String, Value::Null, "normal, template or reference.").choices(&DISPLAY_TYPE_NAMES))
        .flag(flag("color", "c", T::Int, Value::Null, "Palette index for its objects' wireframe, 0 to 31; 0 is none. Turns RGB off."))
        .flag(flag("rgbColor", "rgb", T::Vec3, Value::Null, "An RGB wireframe colour; turns RGB on."))
        .flag(flag("name", "n", T::String, Value::Null, "Rename the layer; made unique.")),
    );

    c.register(
        CommandDef::<Session>::new("listDisplayLayers", "Every display layer in panel order, each as displayLayer returns it.", |s, _a| {
            Ok(Value::Array(all(&s.graph).into_iter().map(|l| info(&s.graph, l)).collect()))
        })
        .undoable(false),
    );

    c.register(
        CommandDef::<Session>::new("reorderDisplayLayer", "Move a display layer to a row of the Display Layers panel (0 is the top).", |s, a| {
            let name = first_positional(a, "reorderDisplayLayer", "name the layer")?;
            let id = layer_arg(&s.graph, &name, "reorderDisplayLayer")?;
            reorder(&mut s.graph, id, a.int("index", 0).max(0) as usize)?;
            Ok(names_of(&s.graph, &all(&s.graph)))
        })
        .positional("layer")
        .flag(flag("index", "i", T::Int, json!(0), "Row it moves to.").min(0.0)),
    );

    c.register(
        CommandDef::<Session>::new("selectDisplayLayerMembers", "Select the objects in display layers.", |s, a| {
            let mut ids = Vec::new();
            for name in positional_strings(a) {
                let layer = layer_arg(&s.graph, &name, "selectDisplayLayerMembers")?;
                ids.extend(members(&s.graph, layer));
            }
            let mode = if a.bool("add") { SelectMode::Add } else { SelectMode::Replace };
            s.graph.select(&ids, mode);
            Ok(names_of(&s.graph, &ids))
        })
        .positional("layers")
        .flag(flag("add", "", T::Bool, json!(false), "Add them to the selection instead of replacing it.")),
    );
}
