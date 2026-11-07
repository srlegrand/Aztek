//! How an object draws: drawing overrides, templating, display layers, object colours and the display colour tables.
//! The viewport asks here, so the decisions can be tested without a GPU.
//!
//! A display layer works by connection: layer.drawInfo drives node.drawOverride, so while connected the node's own
//! override values are replaced by the layer's. `override_on` gives the values in force either way.
//!
//! The order a wire colour is decided in, first match wins:
//!   1. Selected: lead and active colours; a selected template has its own active colour.
//!   2. Under something selected: the child highlight, unless selectionChildHighlighting is off.
//!   3. Display type, from the node up: template draws in the template colour, reference in black.
//!   4. Drawing override colour, from the node up. The nearest enabled one with a colour wins, so a shape beats its
//!      transform, and a transform beats its group. Index 0 is no colour, and the search goes on above it.
//!   5. useObjectColor on the shape, then on its transform: objectColor (a user colour) or wireColorRGB.
//!   6. The type's default, which displayColor and displayRGBColor can change.
//!
//! The reference's node reference gives the attributes, and says template and visibility are inherited. It does not give the
//! order above. Steps 3 to 5, and treating shading, texturing and level of detail as "off anywhere on the path", are the
//! best reading without a copy of the reference to check against. PARITY.md lists them as unverified.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;

use aztek_ops::{fail, objects, ArgType, Args, CmdResult, CommandDef, Commands, FlagDef};
use glam::DVec3;
use serde_json::Value;

use crate::display_layers as dl;
use crate::graph::Graph;
use crate::plug::{NodeId, PlugRef, PlugValue, NO_NODE};
use crate::session::Session;
use crate::transform::DagNode;

/// Palette entries 0 to 31, the factory colours. 0 means no colour; `palette` lets colorIndex change them.
pub const INDEX_COLORS: [[u8; 3]; 32] = [
    [120, 120, 120], [0, 0, 0], [64, 64, 64], [153, 153, 153], [155, 0, 40], [0, 4, 96], [0, 0, 255], [0, 70, 25],
    [38, 0, 67], [200, 0, 200], [138, 72, 51], [63, 35, 31], [153, 38, 0], [255, 0, 0], [0, 255, 0], [0, 65, 153],
    [255, 255, 255], [255, 255, 0], [100, 220, 255], [67, 255, 163], [255, 176, 176], [228, 172, 121], [255, 255, 99], [0, 153, 84],
    [161, 106, 48], [158, 161, 48], [104, 161, 48], [48, 161, 93], [48, 161, 161], [48, 103, 161], [111, 48, 161], [161, 48, 106],
];

/// objectColor 0 to 7 is this palette entry and on. Unverified: the eight user colours are taken to be entries 24 to 31.
pub const USER_COLOR_BASE: i64 = 24;

/// (name, dormant index, active index).
pub const DISPLAY_COLORS: &[(&str, i64, i64)] = &[
    ("lead", 19, 19), ("background", 2, 2), ("curve", 5, 18), ("cv", 8, 18), ("hull", 25, 18), ("joint", 28, 20), ("locator", 5, 20),
    ("nurbsCurve", 1, 18), ("nurbsSurface", 1, 19), ("polymesh", 5, 19), ("template", 3, 20), ("light", 16, 18), ("camera", 5, 18), ("cluster", 5, 18), ("lattice", 5, 18),
    ("ikHandle", 5, 18), ("dimension", 5, 18), ("referenceLayer", 1, 1),
];

pub const LEAD: [u8; 4] = [66, 255, 163, 255];
pub const SELECTED: [u8; 4] = [255, 255, 255, 255];
/// Something above this object is selected. Unverified: a dimmer lead colour, not the reference's measured value.
pub const CHILD_HIGHLIGHT: [u8; 4] = [46, 178, 114, 255];

/// What the colour commands change. They are preferences rather than scene data, so they outlive a new scene; a new
/// session starts from the factory colours.
#[derive(Default)]
struct Colors {
    palette: BTreeMap<i64, [f64; 3]>,
    display: BTreeMap<(String, bool), i64>,
    rgb: BTreeMap<String, [f64; 4]>,
}

thread_local! {
    static COLORS: RefCell<Colors> = RefCell::new(Colors::default());
    /// Which way the animation plays: 0 stopped. Set with the play command; hiding on playback reads it.
    static PLAY_DIRECTION: Cell<i32> = const { Cell::new(0) };
}

pub fn set_play_direction(d: i32) {
    PLAY_DIRECTION.with(|p| p.set(d));
}

pub fn reset_colors() {
    COLORS.with(|c| *c.borrow_mut() = Colors::default());
    set_play_direction(0);
}

/// A palette entry as colorIndex holds it, 0 to 1.
pub fn palette(i: i64) -> [f64; 3] {
    let i = i.clamp(0, 31);
    COLORS.with(|c| c.borrow().palette.get(&i).copied()).unwrap_or_else(|| INDEX_COLORS[i as usize].map(|x| x as f64 / 255.0))
}

pub fn set_palette(i: i64, rgb: [f64; 3]) {
    COLORS.with(|c| {
        c.borrow_mut().palette.insert(i, rgb);
    });
}

pub fn reset_palette() {
    COLORS.with(|c| c.borrow_mut().palette.clear());
}

/// displayColor's index for a name, set or factory. None for a name it doesn't know.
pub fn display_index(name: &str, active: bool) -> Option<i64> {
    let set = COLORS.with(|c| c.borrow().display.get(&(name.to_string(), active)).copied());
    set.or_else(|| DISPLAY_COLORS.iter().find(|c| c.0 == name).map(|c| if active { c.2 } else { c.1 }))
}

pub fn set_display_index(name: &str, active: bool, index: i64) {
    COLORS.with(|c| {
        c.borrow_mut().display.insert((name.to_string(), active), index);
    });
}

pub fn reset_display_indices() {
    COLORS.with(|c| c.borrow_mut().display.clear());
}

/// A colour displayRGBColor was given.
pub fn display_rgb(name: &str) -> Option<[f64; 4]> {
    COLORS.with(|c| c.borrow().rgb.get(name).copied())
}

pub fn set_display_rgb(name: &str, rgba: [f64; 4]) {
    COLORS.with(|c| {
        c.borrow_mut().rgb.insert(name.to_string(), rgba);
    });
}

pub fn reset_display_rgb() {
    COLORS.with(|c| c.borrow_mut().rgb.clear());
}

fn to_bytes(c: [f64; 3]) -> [u8; 4] {
    let b = |x: f64| (x.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
    [b(c[0]), b(c[1]), b(c[2]), 255]
}

/// A palette entry for drawing.
pub fn palette_bytes(i: i64) -> [u8; 4] {
    to_bytes(palette(i))
}

/// The drawn colour for a display colour name: what the commands set, else `factory`. The viewport's own defaults stay
/// until a script changes one, so the factory look is held in one place per type.
pub fn dormant_color(name: &str, factory: [u8; 4]) -> [u8; 4] {
    if let Some(c) = display_rgb(name) {
        return to_bytes([c[0], c[1], c[2]]);
    }
    match COLORS.with(|c| c.borrow().display.get(&(name.to_string(), false)).copied()) {
        Some(i) => palette_bytes(i),
        None => factory,
    }
}

/// The lead selection's colour.
pub fn lead_color() -> [u8; 4] {
    if let Some(c) = display_rgb("lead") {
        return to_bytes([c[0], c[1], c[2]]);
    }
    match COLORS.with(|c| c.borrow().display.get(&("lead".to_string(), true)).copied()) {
        Some(i) => palette_bytes(i),
        None => LEAD,
    }
}

fn template_color(active: bool) -> [u8; 4] {
    palette_bytes(display_index("template", active).unwrap_or(3))
}

/// The drawing override values in force on one node.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Override {
    pub enabled: bool,
    pub display_type: i64,
    pub level_of_detail: i64,
    pub shading: bool,
    pub texturing: bool,
    pub playback: bool,
    pub visible: bool,
    pub hide_on_playback: bool,
    pub use_rgb: bool,
    pub color: i64,
    pub rgb: DVec3,
    /// They came from a display layer rather than the node.
    pub from_layer: bool,
}

/// The values in force on a DAG node: its layer's while it is in one, else its own.
pub fn override_on(g: &Graph, id: NodeId) -> Option<Override> {
    let n = g.node(id)?;
    if !n.ty().dag {
        return None;
    }
    if let Some(layer) = dl::layer_of(g, id) {
        let l = g.node(layer)?;
        return Some(Override {
            enabled: l.bool(dl::ENABLED),
            display_type: l.int(dl::DISPLAY_TYPE),
            level_of_detail: l.int(dl::LEVEL_OF_DETAIL),
            shading: l.bool(dl::SHADING),
            texturing: l.bool(dl::TEXTURING),
            playback: l.bool(dl::PLAYBACK),
            visible: l.bool(dl::VISIBILITY),
            hide_on_playback: l.bool(dl::HIDE_ON_PLAYBACK),
            use_rgb: l.bool(dl::OVERRIDE_RGB_COLORS),
            color: l.int(dl::COLOR),
            rgb: l.vec3(dl::OVERRIDE_COLOR_RGB),
            from_layer: true,
        });
    }
    Some(Override {
        enabled: n.bool(DagNode::OVERRIDE_ENABLED),
        display_type: n.int(DagNode::OVERRIDE_DISPLAY_TYPE),
        level_of_detail: n.int(DagNode::OVERRIDE_LEVEL_OF_DETAIL),
        shading: n.bool(DagNode::OVERRIDE_SHADING),
        texturing: n.bool(DagNode::OVERRIDE_TEXTURING),
        playback: n.bool(DagNode::OVERRIDE_PLAYBACK),
        visible: n.bool(DagNode::OVERRIDE_VISIBILITY),
        hide_on_playback: n.bool(DagNode::HIDE_ON_PLAYBACK),
        use_rgb: n.bool(DagNode::OVERRIDE_RGB_COLORS),
        color: n.int(DagNode::OVERRIDE_COLOR),
        rgb: n.data().vec3(DagNode::OVERRIDE_COLOR_RGB),
        from_layer: false,
    })
}

/// True when this node's own override hides it: overrideVisibility off, or hidden while the animation plays.
pub fn hidden_by_override(g: &Graph, id: NodeId) -> bool {
    let Some(o) = override_on(g, id) else { return false };
    if !o.enabled {
        return false;
    }
    !o.visible || (PLAY_DIRECTION.with(Cell::get) != 0 && (o.hide_on_playback || !o.playback))
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum DisplayKind {
    #[default]
    Normal,
    Template,
    Reference,
}

/// The display type on a node, from it up. A template flag or a non-normal override applies to everything below. A
/// layer that says normal ends the search, so a child in its own layer follows that layer.
pub fn display_kind(g: &Graph, id: NodeId) -> DisplayKind {
    let mut n = id;
    while n != NO_NODE {
        let Some(node) = g.node(n) else { break };
        if node.ty().dag {
            if node.bool(DagNode::TEMPLATE) {
                return DisplayKind::Template;
            }
            if let Some(o) = override_on(g, n).filter(|o| o.enabled) {
                match o.display_type {
                    1 => return DisplayKind::Template,
                    2 => return DisplayKind::Reference,
                    _ if o.from_layer => return DisplayKind::Normal,
                    _ => {}
                }
            }
        }
        n = g.parent(n);
    }
    DisplayKind::Normal
}

/// False for what the viewport must not pick: templates and references, on the node or above it. The Outliner and
/// scripts still select them.
pub fn pickable(g: &Graph, id: NodeId) -> bool {
    display_kind(g, id) == DisplayKind::Normal
}

/// The nearest enabled override colour from the node up.
pub fn override_color(g: &Graph, id: NodeId) -> Option<[u8; 4]> {
    let mut n = id;
    while n != NO_NODE {
        if let Some(o) = override_on(g, n).filter(|o| o.enabled) {
            if o.use_rgb {
                return Some(to_bytes(o.rgb.to_array()));
            }
            if o.color > 0 {
                return Some(palette_bytes(o.color));
            }
        }
        n = g.parent(n);
    }
    None
}

/// useObjectColor on the node, then on its transform.
pub fn object_color(g: &Graph, id: NodeId) -> Option<[u8; 4]> {
    let own = |n: NodeId| -> Option<[u8; 4]> {
        let node = g.node(n).filter(|x| x.ty().dag)?;
        match node.int(DagNode::USE_OBJECT_COLOR) {
            1 => Some(palette_bytes(USER_COLOR_BASE + node.int(DagNode::OBJECT_COLOR).clamp(0, 7))),
            2 => Some(to_bytes(node.data().vec3(DagNode::WIRE_COLOR_RGB).to_array())),
            _ => None,
        }
    };
    own(id).or_else(|| {
        let xf = crate::dag::transform_of(g, id);
        if xf != id {
            own(xf)
        } else {
            None
        }
    })
}

/// How a shape draws apart from selection.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct DrawStyle {
    pub kind: DisplayKind,
    /// The colour the object's settings give; None leaves the type's default.
    pub color: Option<[u8; 4]>,
    /// False draws a wireframe in shaded display.
    pub shaded: bool,
    pub textured: bool,
    pub bounding_box: bool,
}

pub fn style(g: &Graph, id: NodeId) -> DrawStyle {
    let kind = display_kind(g, id);
    let color = match kind {
        DisplayKind::Template => Some(template_color(false)),
        DisplayKind::Reference => Some(palette_bytes(display_index("referenceLayer", false).unwrap_or(1))),
        DisplayKind::Normal => override_color(g, id).or_else(|| object_color(g, id)),
    };
    let (mut shaded, mut textured, mut bounding_box) = (kind != DisplayKind::Template, true, false);
    let mut n = id;
    while n != NO_NODE {
        if let Some(o) = override_on(g, n).filter(|o| o.enabled) {
            shaded &= o.shading;
            textured &= o.texturing;
            bounding_box |= o.level_of_detail == 1;
        }
        n = g.parent(n);
    }
    DrawStyle { kind, color, shaded, textured, bounding_box }
}

/// How the selection touches an object.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SelectState {
    None,
    Selected,
    Lead,
    /// Something above it is selected.
    Child,
}

/// A shape counts as selected when it or its transform is; further up is the child highlight.
pub fn select_state(g: &Graph, id: NodeId, sel: &[NodeId]) -> SelectState {
    if sel.is_empty() || id == NO_NODE {
        return SelectState::None;
    }
    let xf = crate::dag::transform_of(g, id);
    let lead = sel.last().copied();
    if lead == Some(id) || lead == Some(xf) {
        return SelectState::Lead;
    }
    if sel.contains(&id) || sel.contains(&xf) {
        return SelectState::Selected;
    }
    if !g.node(id).is_some_and(|n| n.ty().dag && n.bool(DagNode::SELECTION_CHILD_HIGHLIGHTING)) {
        return SelectState::None;
    }
    let mut n = g.parent(xf);
    while n != NO_NODE {
        if sel.contains(&n) {
            return SelectState::Child;
        }
        n = g.parent(n);
    }
    SelectState::None
}

/// The wire colour of an object: see the order at the top of this file. `factory` is the type's default look and `kind`
/// its display colour name.
pub fn wire_color(g: &Graph, id: NodeId, sel: &[NodeId], kind: &str, factory: [u8; 4]) -> [u8; 4] {
    selected_color(g, id, sel).unwrap_or_else(|| style(g, id).color.unwrap_or_else(|| dormant_color(kind, factory)))
}

/// The colour the selection gives an object, if it touches it.
pub fn selected_color(g: &Graph, id: NodeId, sel: &[NodeId]) -> Option<[u8; 4]> {
    let state = select_state(g, id, sel);
    if matches!(state, SelectState::Lead | SelectState::Selected) && display_kind(g, id) == DisplayKind::Template {
        return Some(template_color(true));
    }
    match state {
        SelectState::Lead => Some(lead_color()),
        SelectState::Selected => Some(SELECTED),
        SelectState::Child => Some(CHILD_HIGHLIGHT),
        SelectState::None => None,
    }
}

/// Which layer attribute a drawOverride child reads from while the node is in a layer.
fn layer_attr(dag_attr: usize) -> Option<usize> {
    Some(match dag_attr {
        DagNode::OVERRIDE_DISPLAY_TYPE => dl::DISPLAY_TYPE,
        DagNode::OVERRIDE_LEVEL_OF_DETAIL => dl::LEVEL_OF_DETAIL,
        DagNode::OVERRIDE_SHADING => dl::SHADING,
        DagNode::OVERRIDE_TEXTURING => dl::TEXTURING,
        DagNode::OVERRIDE_PLAYBACK => dl::PLAYBACK,
        DagNode::OVERRIDE_ENABLED => dl::ENABLED,
        DagNode::OVERRIDE_VISIBILITY => dl::VISIBILITY,
        DagNode::HIDE_ON_PLAYBACK => dl::HIDE_ON_PLAYBACK,
        DagNode::OVERRIDE_RGB_COLORS => dl::OVERRIDE_RGB_COLORS,
        DagNode::OVERRIDE_COLOR => dl::COLOR,
        DagNode::OVERRIDE_COLOR_RGB => dl::OVERRIDE_COLOR_RGB,
        DagNode::OVERRIDE_COLOR_A => dl::OVERRIDE_COLOR_A,
        _ => return None,
    })
}

/// The children of drawOverride, in the reference's order.
pub const OVERRIDE_CHILDREN: [usize; 12] = [
    DagNode::OVERRIDE_DISPLAY_TYPE, DagNode::OVERRIDE_LEVEL_OF_DETAIL, DagNode::OVERRIDE_SHADING, DagNode::OVERRIDE_TEXTURING, DagNode::OVERRIDE_PLAYBACK, DagNode::OVERRIDE_ENABLED,
    DagNode::OVERRIDE_VISIBILITY, DagNode::HIDE_ON_PLAYBACK, DagNode::OVERRIDE_RGB_COLORS, DagNode::OVERRIDE_COLOR, DagNode::OVERRIDE_COLOR_RGB, DagNode::OVERRIDE_COLOR_A,
];

/// The children of a layer's drawInfo, in the same order.
pub const LAYER_CHILDREN: [usize; 12] = [
    dl::DISPLAY_TYPE, dl::LEVEL_OF_DETAIL, dl::SHADING, dl::TEXTURING, dl::PLAYBACK, dl::ENABLED, dl::VISIBILITY, dl::HIDE_ON_PLAYBACK, dl::OVERRIDE_RGB_COLORS, dl::COLOR, dl::OVERRIDE_COLOR_RGB,
    dl::OVERRIDE_COLOR_A,
];

/// True for a drawOverride child a layer is driving: reading goes to the layer, and setAttr refuses.
pub fn driven_by_layer(g: &Graph, p: &PlugRef) -> bool {
    layer_attr(p.attr as usize).is_some() && g.node(p.node).is_some_and(|n| n.ty().dag) && dl::layer_of(g, p.node).is_some()
}

/// The value a drawOverride child has through its node's layer.
pub fn through_layer(g: &Graph, p: &PlugRef) -> Option<PlugValue> {
    if !driven_by_layer(g, p) {
        return None;
    }
    let layer = dl::layer_of(g, p.node)?;
    let attr = layer_attr(p.attr as usize)?;
    let v = g.node(layer)?.value(attr).clone();
    Some(match v {
        PlugValue::Vec3(x) if p.component >= 0 => PlugValue::Float(x[p.component as usize]),
        o => o,
    })
}

/// Sets the dormant wireframe colour: -ud 1 to 8, -rgb r g b, or neither for the class colour. A transform's shapes take
/// it too, as they are what draws. Unverified: which of the two the reference writes it on.
fn color_cmd(s: &mut Session, a: &Args) -> CmdResult {
    let ud = a.has("userDefined").then(|| a.int("userDefined", 1));
    if ud.is_some_and(|u| !(1..=8).contains(&u)) {
        fail!("color: userDefined must be 1 to 8");
    }
    let rgb = if a.has("rgbColor") {
        let Some(c) = a.vec3("rgbColor") else { fail!("color: rgbColor takes r g b") };
        Some(DVec3::from_array(c))
    } else {
        None
    };
    let names = objects(s, a);
    if names.is_empty() {
        fail!("color: name an object or select one");
    }
    for name in names {
        let g = &mut s.graph;
        let Some(id) = g.find(&name)?.filter(|&i| g.node(i).is_some_and(|n| n.ty().dag)) else { fail!("color: {name} is not a DAG object") };
        let mut targets = vec![id];
        if crate::dag::is_transform(g, id) {
            targets.extend(crate::dag::shapes(g, id, false));
        }
        for t in targets {
            let mode = if rgb.is_some() { 2 } else if ud.is_some() { 1 } else { 0 };
            g.set_value(PlugRef::new(t, DagNode::USE_OBJECT_COLOR as i32), PlugValue::Int(mode))?;
            if let Some(u) = ud {
                g.set_value(PlugRef::new(t, DagNode::OBJECT_COLOR as i32), PlugValue::Int(u - 1))?;
            }
            if let Some(c) = rgb {
                g.set_value(PlugRef::new(t, DagNode::WIRE_COLOR_RGB as i32), PlugValue::Vec3(c))?;
            }
        }
    }
    Ok(Value::Null)
}

pub fn register(c: &mut Commands<Session>) {
    c.register(
        CommandDef::<Session>::new("color", "Set the dormant wireframe colour of objects (the selection by default): -ud 1 to 8, -rgb r g b, or neither for the class colour.", color_cmd)
            .positional("objects")
            .flag(FlagDef::new("rgbColor", "rgb", ArgType::Vec3, Value::Null, "An RGB colour."))
            .flag(FlagDef::new("userDefined", "ud", ArgType::Int, Value::Null, "One of the eight user colours, 1 to 8.")),
    );
}
