//! Named vertex color sets, the reference style: each set is a corner float4 RGBA layer named "color:<set>", linear values.
//! Being ordinary corner layers, they follow every op that carries attributes. One set is current (displayed and
//! painted); sets marked half precision keep their values rounded to 16-bit floats. The first set is what exporters
//! write as the primary vertex color.

use rayon::prelude::*;

use crate::attribute::{AttrType, Index};
use crate::mesh::Mesh;

pub const LAYER_PREFIX: &str = "color:";

pub fn layer_name(set: &str) -> String {
    format!("{LAYER_PREFIX}{set}")
}

/// The set a layer holds, empty when the layer is not a color set.
pub fn set_of(layer: &str) -> &str {
    if layer.len() <= LAYER_PREFIX.len() || !layer.starts_with(LAYER_PREFIX) {
        return "";
    }
    &layer[LAYER_PREFIX.len()..]
}

/// Set names in layer order.
pub fn sets(mesh: &Mesh) -> Vec<String> {
    mesh.corner_attrs()
        .layers()
        .iter()
        .filter(|l| l.ty() == AttrType::Float4)
        .map(|l| set_of(l.name()))
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

pub fn has(mesh: &Mesh, set: &str) -> bool {
    !set.is_empty()
        && mesh
            .corner_attrs()
            .find(&layer_name(set))
            .is_some_and(|a| a.ty() == AttrType::Float4)
}

/// Empty when missing.
pub fn values<'a>(mesh: &'a Mesh, set: &str) -> &'a [[f32; 4]] {
    if set.is_empty() {
        return &[];
    }
    mesh.corner_attrs().get::<[f32; 4]>(&layer_name(set))
}

pub fn values_mut<'a>(mesh: &'a mut Mesh, set: &str) -> &'a mut [[f32; 4]] {
    if set.is_empty() {
        return &mut [];
    }
    mesh.corner_attrs_mut().get_mut::<[f32; 4]>(&layer_name(set))
}

/// Adds a set filled with fill (new faces get fill too), or returns the existing one. The first set becomes current.
pub fn add<'a>(mesh: &'a mut Mesh, set: &str, fill: [f32; 4]) -> &'a mut [[f32; 4]] {
    if set.is_empty() {
        return &mut [];
    }
    let name = layer_name(set);
    if !has(mesh, set) {
        let a = mesh.corner_attrs_mut().add(&name, AttrType::Float4);
        a.set_default(fill);
        a.values_mut::<[f32; 4]>().fill(fill);
        if sets(mesh).len() == 1 {
            mesh.color_set_info_mut().current = set.to_string();
        }
    }
    mesh.corner_attrs_mut().get_mut::<[f32; 4]>(&name)
}

pub fn add_white<'a>(mesh: &'a mut Mesh, set: &str) -> &'a mut [[f32; 4]] {
    add(mesh, set, [1.0; 4])
}

pub fn remove(mesh: &mut Mesh, set: &str) -> bool {
    if !mesh.corner_attrs_mut().remove(&layer_name(set)) {
        return false;
    }
    mesh.color_set_info_mut().half.retain(|h| h != set);
    if mesh.color_set_info().current == set {
        let rest = sets(mesh);
        mesh.color_set_info_mut().current = rest.into_iter().next().unwrap_or_default();
    }
    true
}

pub fn rename(mesh: &mut Mesh, from: &str, to: &str) -> bool {
    if to.is_empty() || !mesh.corner_attrs_mut().rename(&layer_name(from), &layer_name(to)) {
        return false;
    }
    let info = mesh.color_set_info_mut();
    if info.current == from {
        info.current = to.to_string();
    }
    for h in &mut info.half {
        if h == from {
            *h = to.to_string();
        }
    }
    true
}

/// Copies from into to (created or overwritten).
pub fn copy(mesh: &mut Mesh, from: &str, to: &str) -> bool {
    let Some(src) = mesh.corner_attrs().find(&layer_name(from)) else {
        return false;
    };
    if src.ty() != AttrType::Float4 || to.is_empty() || from == to {
        return false;
    }
    let d = src.default_raw();
    let fill: [f32; 4] = std::array::from_fn(|i| f32::from_ne_bytes(d[i * 4..i * 4 + 4].try_into().unwrap()));
    let copied = src.values::<[f32; 4]>().to_vec();
    add(mesh, to, fill).copy_from_slice(&copied);
    let half = is_half(mesh, from);
    set_half(mesh, to, half);
    true
}

/// Moves a set to position index among the sets (0 is the primary, exported first).
pub fn move_to(mesh: &mut Mesh, set: &str, index: usize) -> bool {
    let a = mesh.corner_attrs();
    let mut layers = Vec::new();
    let mut at = None;
    for (i, l) in a.layers().iter().enumerate() {
        let s = set_of(l.name());
        if l.ty() == AttrType::Float4 && !s.is_empty() {
            if s == set {
                at = Some(i);
            }
            layers.push(i);
        }
    }
    let Some(at) = at else {
        return false;
    };
    let to = layers[index.min(layers.len() - 1)];
    mesh.corner_attrs_mut().move_layer(at, to);
    true
}

/// The current set: the stored one when it exists, otherwise the first set; empty when the mesh has none.
pub fn current(mesh: &Mesh) -> String {
    let stored = &mesh.color_set_info().current;
    if !stored.is_empty() && mesh.corner_attrs().has(&layer_name(stored)) {
        return stored.clone();
    }
    sets(mesh).into_iter().next().unwrap_or_default()
}

pub fn set_current(mesh: &mut Mesh, set: &str) -> bool {
    if !mesh.corner_attrs().has(&layer_name(set)) {
        return false;
    }
    mesh.color_set_info_mut().current = set.to_string();
    true
}

pub fn is_half(mesh: &Mesh, set: &str) -> bool {
    mesh.color_set_info().half.iter().any(|h| h == set)
}

/// Marks the set half or full precision; going to half rounds its values.
pub fn set_half(mesh: &mut Mesh, set: &str, half: bool) {
    mesh.color_set_info_mut().half.retain(|h| h != set);
    if !half || !mesh.corner_attrs().has(&layer_name(set)) {
        return;
    }
    mesh.color_set_info_mut().half.push(set.to_string());
    round_slice_to_half(values_mut(mesh, set));
}

pub fn round_to_half(v: f32) -> f32 {
    half::f16::from_f32(v).to_f32()
}

pub fn round_slice_to_half(values: &mut [[f32; 4]]) {
    values.par_iter_mut().for_each(|v| {
        for x in v.iter_mut() {
            *x = round_to_half(*x);
        }
    });
}

/// Rounds a half set's values after they were written; does nothing to full-precision sets.
pub fn settle(mesh: &mut Mesh, set: &str) {
    if is_half(mesh, set) {
        round_slice_to_half(values_mut(mesh, set));
    }
}

/// A free set name: base, else base1, base2, ...
pub fn unique_name(mesh: &Mesh, base: &str) -> String {
    let b = if base.is_empty() { "colorSet" } else { base };
    if !mesh.corner_attrs().has(&layer_name(b)) {
        return b.to_string();
    }
    (1..)
        .map(|i| format!("{b}{i}"))
        .find(|n| !mesh.corner_attrs().has(&layer_name(n)))
        .unwrap()
}

/// Average of each point's corners in the set; points without corners get white.
pub fn point_colors(mesh: &Mesh, set: &str) -> Vec<[f32; 4]> {
    let mut out = vec![[1.0f32; 4]; mesh.point_capacity() as usize];
    let v = values(mesh, set);
    if v.is_empty() {
        return out;
    }
    out.par_iter_mut().enumerate().for_each(|(p, o)| {
        let p = p as Index;
        if !mesh.point_valid(p) {
            return;
        }
        let mut sum = [0.0f32; 4];
        let mut n = 0;
        mesh.for_each_point_corner(p, |c| {
            for k in 0..4 {
                sum[k] += v[c as usize][k];
            }
            n += 1;
        });
        if n > 0 {
            *o = sum.map(|s| s / n as f32);
        }
    });
    out
}

/// Writes per-point colors to every corner of each point.
pub fn set_from_points(mesh: &mut Mesh, set: &str, per_point: &[[f32; 4]]) {
    if values(mesh, set).is_empty() {
        add_white(mesh, set);
    }
    let cc = mesh.corner_capacity() as usize;
    let src: Vec<Option<Index>> = (0..cc as Index)
        .map(|c| {
            (mesh.corner_valid(c) && (mesh.corner_point(c) as usize) < per_point.len()).then(|| mesh.corner_point(c))
        })
        .collect();
    let v = values_mut(mesh, set);
    v.par_iter_mut().zip(src.par_iter()).for_each(|(o, p)| {
        if let Some(p) = p {
            *o = per_point[*p as usize];
        }
    });
    settle(mesh, set);
}

fn legacy_index(n: &str) -> Option<i32> {
    if n == "color" {
        return Some(0);
    }
    let rest = n.strip_prefix("color")?;
    if !rest.is_empty() && rest.bytes().all(|c| c.is_ascii_digit()) {
        return rest.parse().ok();
    }
    None
}

fn legacy_set_name(i: i32) -> String {
    if i == 0 {
        "colorSet".into()
    } else {
        format!("colorSet{i}")
    }
}

/// Turns plain "color" layers (point or corner, and "color1", "color2"... from older importers) into color sets named
/// colorSet, colorSet1, ... and restores the white default of every set (files keep values, not defaults).
pub fn adopt(mesh: &mut Mesh) {
    for s in sets(mesh) {
        mesh.corner_attrs_mut().find_mut(&layer_name(&s)).unwrap().set_default([1.0f32; 4]);
    }
    for n in mesh.corner_attrs().names() {
        let Some(i) = legacy_index(&n) else { continue };
        if mesh.corner_attrs().find(&n).unwrap().ty() != AttrType::Float4 {
            continue;
        }
        let base = legacy_set_name(i);
        let to = if has(mesh, &base) { unique_name(mesh, &base) } else { base };
        mesh.corner_attrs_mut().rename(&n, &layer_name(&to));
        mesh.corner_attrs_mut().find_mut(&layer_name(&to)).unwrap().set_default([1.0f32; 4]);
        if mesh.color_set_info().current.is_empty() {
            mesh.color_set_info_mut().current = to;
        }
    }
    for n in mesh.point_attrs().names() {
        let Some(i) = legacy_index(&n) else { continue };
        if mesh.point_attrs().find(&n).unwrap().ty() != AttrType::Float4 {
            continue;
        }
        let per_point = mesh.point_attrs().get::<[f32; 4]>(&n).to_vec();
        mesh.point_attrs_mut().remove(&n);
        let base = legacy_set_name(i);
        let to = if has(mesh, &base) { unique_name(mesh, &base) } else { base };
        add_white(mesh, &to);
        set_from_points(mesh, &to, &per_point);
    }
}
