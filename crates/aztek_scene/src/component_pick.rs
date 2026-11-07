//! Screen projection, picking components near the cursor, topology
//! callouts and the object selection masks.

use aztek_core::{PrefScope, PrefStore};
use aztek_geo::triangulate::{triangulate_face, TriangulateOptions};
use aztek_geo::{Index, Mesh};
use glam::{DMat4, DVec2, DVec3, DVec4};
use rayon::prelude::*;
use serde_json::json;

use crate::components::ComponentMode;
use crate::node::Node;
use crate::plug::{NodeId, NO_NODE};

/// World to widget pixels the way the viewport maps them: clip space scaled to the widget, no y flip.
#[derive(Debug, Clone, Copy)]
pub struct Screen {
    pub view_proj: DMat4,
    pub width: f64,
    pub height: f64,
}

impl Default for Screen {
    fn default() -> Self {
        Self { view_proj: DMat4::IDENTITY, width: 1.0, height: 1.0 }
    }
}

/// Signed distance to the near plane in clip space, positive in front: reversed-Z puts the near plane at z = w.
fn in_front(clip: DVec4) -> f64 {
    clip.w - clip.z
}

fn near_crossing(a: DVec4, b: DVec4) -> DVec4 {
    let (da, db) = (in_front(a), in_front(b));
    a.lerp(b, da / (da - db))
}

fn to_pixels(clip: DVec4, width: f64, height: f64) -> DVec2 {
    DVec2::new((clip.x / clip.w * 0.5 + 0.5) * width, (clip.y / clip.w * 0.5 + 0.5) * height)
}

impl Screen {
    /// None behind the camera.
    pub fn project(&self, world: DVec3) -> Option<DVec2> {
        let clip = self.view_proj * world.extend(1.0);
        if clip.w <= 1e-9 {
            return None;
        }
        Some(to_pixels(clip, self.width, self.height))
    }

    /// The part of segment a-b in front of the near plane, in pixels; None when none of it is.
    pub fn project_segment(&self, a: DVec3, b: DVec3) -> Option<(DVec2, DVec2)> {
        let (mut ca, mut cb) = (self.view_proj * a.extend(1.0), self.view_proj * b.extend(1.0));
        let (a_in, b_in) = (in_front(ca) >= 0.0, in_front(cb) >= 0.0);
        if !a_in && !b_in {
            return None;
        }
        if !a_in {
            ca = near_crossing(ca, cb);
        } else if !b_in {
            cb = near_crossing(cb, ca);
        }
        if ca.w <= 1e-12 || cb.w <= 1e-12 {
            return None;
        }
        Some((to_pixels(ca, self.width, self.height), to_pixels(cb, self.width, self.height)))
    }

    /// The part of the polygon in front of the near plane, in pixels; empty when none of it is.
    pub fn project_polygon(&self, corners: &[DVec3]) -> Vec<DVec2> {
        let clip: Vec<DVec4> = corners.iter().map(|c| self.view_proj * c.extend(1.0)).collect();
        let mut out = Vec::new();
        for i in 0..clip.len() {
            let (a, b) = (clip[i], clip[(i + 1) % clip.len()]);
            let (a_in, b_in) = (in_front(a) >= 0.0, in_front(b) >= 0.0);
            if a_in && a.w > 1e-12 {
                out.push(to_pixels(a, self.width, self.height));
            }
            if a_in != b_in {
                let x = near_crossing(a, b);
                if x.w > 1e-12 {
                    out.push(to_pixels(x, self.width, self.height));
                }
            }
        }
        out
    }
}

/// How close, in pixels, the cursor must be to a vertex or edge for a click to take it.
pub fn radius(mode: ComponentMode) -> f64 {
    if mode == ComponentMode::Vertex {
        12.0
    } else {
        8.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Candidate {
    pub px: f64,
    pub id: u32,
}

fn sort_candidates(all: &mut [Candidate]) {
    all.sort_by(|a, b| a.px.total_cmp(&b.px).then(a.id.cmp(&b.id)));
}

fn seg_distance(p: DVec2, a: DVec2, b: DVec2) -> f64 {
    let d = b - a;
    let len2 = d.dot(d);
    let t = if len2 > 0.0 { ((p - a).dot(d) / len2).clamp(0.0, 1.0) } else { 0.0 };
    (a + d * t - p).length()
}

/// Vertices (by their point) or edges (by the projected segment) of the mesh closer than max_px to the cursor,
/// nearest first. Face mode has none: faces are picked by raycast.
pub fn nearby(m: &Mesh, world: &DMat4, mode: ComponentMode, screen: &Screen, cursor: DVec2, max_px: f64) -> Vec<Candidate> {
    if mode != ComponentMode::Vertex && mode != ComponentMode::Edge {
        return Vec::new();
    }
    let world_of = |p: Index| world.transform_point3(m.position(p));
    let count = if mode == ComponentMode::Vertex { m.point_capacity() } else { m.edge_capacity() };
    let mut all: Vec<Candidate> = (0..count)
        .into_par_iter()
        .filter_map(|id| {
            let d = if mode == ComponentMode::Vertex {
                if !m.point_valid(id) {
                    return None;
                }
                (screen.project(world_of(id))? - cursor).length()
            } else {
                if !m.edge_valid(id) {
                    return None;
                }
                let (a, b) = screen.project_segment(world_of(m.edge_point(id, 0)), world_of(m.edge_point(id, 1)))?;
                seg_distance(cursor, a, b)
            };
            (d < max_px).then_some(Candidate { px: d, id })
        })
        .collect();
    sort_candidates(&mut all);
    all
}

/// "general" prefs: whether a click picks edges or faces by their center point instead of their line or area.
pub const EDGES_BY_CENTER: &str = "/select/byCenter/edges";
pub const FACES_BY_CENTER: &str = "/select/byCenter/faces";

pub fn select_by_center_key(mode: ComponentMode) -> &'static str {
    if mode == ComponentMode::Face {
        FACES_BY_CENTER
    } else {
        EDGES_BY_CENTER
    }
}

pub fn select_by_center(mode: ComponentMode) -> bool {
    matches!(mode, ComponentMode::Edge | ComponentMode::Face) && PrefStore::get().bool("general", select_by_center_key(mode), false)
}

/// How close, in pixels, the cursor must be to a center for a click to take its edge or face.
pub fn center_radius() -> f64 {
    12.0
}

/// Edges or faces of the mesh whose center, center_of(id) in world space, is closer than max_px to the cursor,
/// nearest first.
pub fn nearby_centers(
    m: &Mesh,
    mode: ComponentMode,
    center_of: impl Fn(u32) -> DVec3,
    screen: &Screen,
    cursor: DVec2,
    max_px: f64,
) -> Vec<Candidate> {
    if mode != ComponentMode::Edge && mode != ComponentMode::Face {
        return Vec::new();
    }
    let count = if mode == ComponentMode::Edge { m.edge_capacity() } else { m.face_capacity() };
    let mut all = Vec::new();
    for i in 0..count {
        let valid = if mode == ComponentMode::Edge { m.edge_valid(i) } else { m.face_valid(i) };
        if !valid {
            continue;
        }
        if let Some(s) = screen.project(center_of(i)) {
            let d = (s - cursor).length();
            if d < max_px {
                all.push(Candidate { px: d, id: i });
            }
        }
    }
    sort_candidates(&mut all);
    all
}

/// The world point that must be unoccluded for a candidate to count: the vertex, or the edge's midpoint.
pub fn anchor(m: &Mesh, world: &DMat4, mode: ComponentMode, id: u32) -> DVec3 {
    let world_of = |p: Index| world.transform_point3(m.position(p));
    if mode == ComponentMode::Edge {
        return (world_of(m.edge_point(id, 0)) + world_of(m.edge_point(id, 1))) * 0.5;
    }
    world_of(id)
}

/// What a click under the cursor would select, kept while hovering. Object mode leaves id unused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hover {
    pub mode: ComponentMode,
    pub node: NodeId,
    pub id: u32,
}

impl Default for Hover {
    fn default() -> Self {
        Self { mode: ComponentMode::Object, node: NO_NODE, id: 0 }
    }
}

impl Hover {
    pub fn empty(&self) -> bool {
        self.node == NO_NODE
    }
}

/// Point ids of the triangles covering face f, 3 per triangle, split exactly as the shaded mesh splits it. Overlays
/// that fill a face use these so concave and non-planar n-gons cover the same area the surface does.
pub fn fill_triangles(m: &Mesh, f: u32) -> Vec<u32> {
    let mut corners = Vec::new();
    triangulate_face(m, f, &mut corners, &TriangulateOptions::default());
    corners.into_iter().map(|c| m.corner_point(c)).collect()
}

/// Topology that hover feedback calls out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Topology {
    Plain,
    /// A 3-sided face.
    Triangle,
    /// A face of 5 or more sides.
    Ngon,
    /// A vertex with 5 or more edges.
    Pole,
}

pub fn face_topology(m: &Mesh, f: u32) -> Topology {
    match m.face_size(f) {
        3 => Topology::Triangle,
        n if n >= 5 => Topology::Ngon,
        _ => Topology::Plain,
    }
}

pub fn point_topology(m: &Mesh, p: u32) -> Topology {
    let mut edges = 0;
    m.for_each_point_edge(p, |_| edges += 1);
    if edges >= 5 {
        Topology::Pole
    } else {
        Topology::Plain
    }
}

fn topology_name(t: Topology) -> &'static str {
    match t {
        Topology::Triangle => "triangles",
        Topology::Ngon => "ngons",
        Topology::Pole => "poles",
        Topology::Plain => "plain",
    }
}

/// Pref keys ("general") for a kind's hover callout: whether it is on and its sRGB "#rrggbb" color.
pub fn topology_enabled_key(t: Topology) -> String {
    format!("/viewport/topology/{}", topology_name(t))
}

pub fn topology_color_key(t: Topology) -> String {
    topology_enabled_key(t) + "Color"
}

pub fn topology_default_color(t: Topology) -> &'static str {
    if t == Topology::Ngon {
        "#e03c3c"
    } else {
        "#f0922b"
    }
}

/// The reference's object selection masks: which kinds of object clicks and marquees may pick. Each persists as a "general"
/// pref, on by default.
pub mod selection_mask {
    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Kind {
        Curve,
        Mesh,
        Joint,
        Locator,
        Other,
        Light,
    }

    pub const ALL: [Kind; 6] = [Kind::Curve, Kind::Mesh, Kind::Joint, Kind::Light, Kind::Locator, Kind::Other];

    /// "mesh", "joint", ...: the leaf of the pref key and of the "select.mask.<name>" action.
    pub fn name(k: Kind) -> &'static str {
        match k {
            Kind::Curve => "curve",
            Kind::Mesh => "mesh",
            Kind::Joint => "joint",
            Kind::Locator => "locator",
            Kind::Other => "other",
            Kind::Light => "light",
        }
    }

    pub fn pref_key(k: Kind) -> String {
        format!("/select/mask/{}", name(k))
    }

    pub fn enabled(k: Kind) -> bool {
        PrefStore::get().bool("general", &pref_key(k), true)
    }

    pub fn set_enabled(k: Kind, on: bool) {
        PrefStore::get().set("general", &pref_key(k), json!(on), PrefScope::User);
    }

    /// The reference's Show menu filters: whether a kind draws in the viewport at all. Hidden kinds can't be picked either.
    pub fn show_key(k: Kind) -> String {
        format!("/viewport/show/{}", name(k))
    }

    pub fn shown(k: Kind) -> bool {
        PrefStore::get().bool("general", &show_key(k), true)
    }

    pub fn set_shown(k: Kind, on: bool) {
        PrefStore::get().set("general", &show_key(k), json!(on), PrefScope::User);
    }

    /// Whether the kind node falls under is shown.
    pub fn node_shown(node: &Node) -> bool {
        shown(kind_of(node))
    }

    /// The mask an object falls under, by its node type.
    pub fn kind_of(node: &Node) -> Kind {
        let t = node.ty();
        // Cameras and lights derive from locator but are not what the locator mask means.
        if t.is_a("mesh") {
            Kind::Mesh
        } else if t.is_a("curve") {
            Kind::Curve
        } else if t.is_a("joint") {
            Kind::Joint
        } else if t.is_a("light") {
            Kind::Light
        } else if t.is_a("camera") {
            Kind::Other
        } else if t.is_a("locator") {
            Kind::Locator
        } else {
            Kind::Other
        }
    }

    pub fn pickable(node: &Node) -> bool {
        enabled(kind_of(node)) && shown(kind_of(node))
    }
}
