//! A mesh's UV sets: their names, their order, which one is current, and each one's seams.
//!
//! The current set is always corner "uv0" with edge "seam", which is what every UV tool and the views work on. The
//! others sit in "uv1", "uv2", ... in set order, as the geometry operations already carry them, each with its seams
//! parked in edge "uvseam:<name>". Names and order are kept in a face attribute's name ("uvnames:a|b|c") and the
//! current one in another ("uvcurrent:a"): attributes with no values that copies and edits of the mesh keep. A mesh
//! with neither is one set, map1, plus map2 and on for any "uv1" and on it carries.

use aztek_geo::primitives::mark_uv_seams;
use aztek_geo::{names, AttrType, Mesh};

const NAMES: &str = "uvnames:";
const CURRENT: &str = "uvcurrent:";
const SEAM_OF: &str = "uvseam:";
const HOLD: &str = "uvhold:";

fn marker(m: &Mesh, prefix: &str) -> Option<String> {
    m.face_attrs().names().into_iter().find_map(|n| n.strip_prefix(prefix).map(str::to_string))
}

fn set_marker(m: &mut Mesh, prefix: &str, value: &str) {
    for n in m.face_attrs().names() {
        if n.starts_with(prefix) {
            m.face_attrs_mut().remove(&n);
        }
    }
    m.face_attrs_mut().add(&format!("{prefix}{value}"), AttrType::Bool);
}

/// The name of the set the mesh's uv0 holds.
pub fn current(m: &Mesh) -> String {
    marker(m, CURRENT).unwrap_or_else(|| "map1".to_string())
}

/// Every set's name, in set order.
pub fn all(m: &Mesh) -> Vec<String> {
    let cur = current(m);
    let mut out: Vec<String> = marker(m, NAMES).map(|t| t.split('|').filter(|n| !n.is_empty()).map(str::to_string).collect()).unwrap_or_default();
    if !out.contains(&cur) {
        out.insert(0, cur);
    }
    // Layers nothing names: a mesh from a file or a tool that knows only the numbered layers.
    let mut k = out.len() as i32;
    while m.corner_attrs().has(&names::uv(k)) {
        let mut n = k + 1;
        while out.contains(&format!("map{n}")) {
            n += 1;
        }
        out.push(format!("map{n}"));
        k += 1;
    }
    out
}

/// The corner layer a set's UVs are in.
pub fn layer(m: &Mesh, set: &str) -> Option<String> {
    let cur = current(m);
    if set == cur {
        return Some(names::uv(0));
    }
    let mut slot = 1;
    for n in all(m) {
        if n == cur {
            continue;
        }
        if n == set {
            return Some(names::uv(slot));
        }
        slot += 1;
    }
    None
}

/// A name no set has, counting up from the one asked for.
pub fn unique(m: &Mesh, wanted: &str) -> String {
    let have = all(m);
    if !have.contains(&wanted.to_string()) {
        return wanted.to_string();
    }
    let stem = wanted.trim_end_matches(|c: char| c.is_ascii_digit());
    let mut n: u32 = wanted[stem.len()..].parse().unwrap_or(0) + 1;
    loop {
        let name = format!("{stem}{n}");
        if !have.contains(&name) {
            return name;
        }
        n += 1;
    }
}

/// Writes the names and the current one, moving every layer to where the new order puts it.
fn arrange(m: &mut Mesh, order: &[String], cur: &str) {
    let before: Vec<(String, String)> = all(m).into_iter().filter_map(|n| layer(m, &n).map(|l| (n, l))).collect();
    for (name, at) in &before {
        if m.corner_attrs().has(at) {
            m.corner_attrs_mut().rename(at, &format!("{HOLD}{name}"));
        }
    }
    set_marker(m, NAMES, &order.join("|"));
    set_marker(m, CURRENT, cur);
    for name in order {
        let to = layer(m, name).unwrap_or_else(|| names::uv(0));
        let held = format!("{HOLD}{name}");
        if m.corner_attrs().has(&held) {
            m.corner_attrs_mut().rename(&held, &to);
        } else {
            m.corner_attrs_mut().get_or_add::<[f32; 2]>(&to);
        }
    }
    // A set no longer named goes with its layer.
    for n in m.corner_attrs().names() {
        if n.starts_with(HOLD) {
            m.corner_attrs_mut().remove(&n);
        }
    }
}

fn park_seams(m: &mut Mesh, set: &str) {
    let parked = format!("{SEAM_OF}{set}");
    m.edge_attrs_mut().remove(&parked);
    if m.edge_attrs().has(names::SEAM) {
        m.edge_attrs_mut().rename(names::SEAM, &parked);
    }
}

fn take_seams(m: &mut Mesh, set: &str) {
    let parked = format!("{SEAM_OF}{set}");
    m.edge_attrs_mut().remove(names::SEAM);
    if m.edge_attrs().has(&parked) {
        m.edge_attrs_mut().rename(&parked, names::SEAM);
    } else {
        // Never the current set before: its seams are where its UVs part.
        mark_uv_seams(m, &names::uv(0));
    }
}

/// Makes a set the one the tools and views work on. False when there is no set of that name.
pub fn set_current(m: &mut Mesh, set: &str) -> bool {
    let order = all(m);
    if !order.iter().any(|n| n == set) {
        return false;
    }
    let cur = current(m);
    if cur == set {
        // Still worth writing down, for a mesh that had no names yet.
        arrange(m, &order, set);
        return true;
    }
    park_seams(m, &cur);
    arrange(m, &order, set);
    take_seams(m, set);
    true
}

/// A new, empty set. Returns its name, made unique.
pub fn create(m: &mut Mesh, wanted: &str) -> String {
    let name = unique(m, if wanted.is_empty() { "uvSet1" } else { wanted });
    let mut order = all(m);
    order.push(name.clone());
    let cur = current(m);
    arrange(m, &order, &cur);
    name
}

/// A copy of a set under a new name, or over an existing set of that name. Returns the name; None when there is no
/// set to copy.
pub fn copy(m: &mut Mesh, from: &str, to: &str) -> Option<String> {
    let src_layer = layer(m, from)?;
    let src: Vec<[f32; 2]> = m.corner_attrs().get::<[f32; 2]>(&src_layer).to_vec();
    let name = if all(m).iter().any(|n| n == to) { to.to_string() } else { create(m, to) };
    if name == from {
        return Some(name);
    }
    let dst_layer = layer(m, &name)?;
    let dst = m.corner_attrs_mut().get_or_add::<[f32; 2]>(&dst_layer);
    let n = dst.len().min(src.len());
    dst[..n].copy_from_slice(&src[..n]);
    // The copy's seams are the source's.
    let cur = current(m);
    let seam_of = |set: &str| if set == cur { names::SEAM.to_string() } else { format!("{SEAM_OF}{set}") };
    let seams: Vec<bool> = m.edge_attrs().get::<bool>(&seam_of(from)).to_vec();
    if name == cur {
        m.edge_attrs_mut().remove(names::SEAM);
        if seams.is_empty() {
            mark_uv_seams(m, &names::uv(0));
        }
    }
    if !seams.is_empty() {
        let to_attr = seam_of(&name);
        let dst = m.edge_attrs_mut().get_or_add::<bool>(&to_attr);
        let n = dst.len().min(seams.len());
        dst[..n].copy_from_slice(&seams[..n]);
    } else if name != cur {
        m.edge_attrs_mut().remove(&seam_of(&name));
    }
    Some(name)
}

/// Removes a set; the first of the others becomes current if it was. An error names why it can't.
pub fn delete(m: &mut Mesh, set: &str) -> Result<(), String> {
    let order = all(m);
    if !order.iter().any(|n| n == set) {
        return Err(format!("no UV set {set}"));
    }
    if order.len() == 1 {
        return Err("can't delete the only UV set".into());
    }
    if current(m) == set {
        let next = order.iter().find(|n| *n != set).unwrap().clone();
        set_current(m, &next);
    }
    let rest: Vec<String> = order.into_iter().filter(|n| n != set).collect();
    let cur = current(m);
    m.edge_attrs_mut().remove(&format!("{SEAM_OF}{set}"));
    arrange(m, &rest, &cur);
    Ok(())
}

pub fn rename(m: &mut Mesh, set: &str, to: &str) -> Result<(), String> {
    let order = all(m);
    if !order.iter().any(|n| n == set) {
        return Err(format!("no UV set {set}"));
    }
    if to.is_empty() || to.contains('|') {
        return Err("give the new name".into());
    }
    if set == to {
        return Ok(());
    }
    if order.iter().any(|n| n == to) {
        return Err(format!("there is already a UV set {to}"));
    }
    let cur = current(m);
    // The layers stay where they are: only the names change.
    let renamed: Vec<String> = order.iter().map(|n| if n == set { to.to_string() } else { n.clone() }).collect();
    let parked = format!("{SEAM_OF}{set}");
    if m.edge_attrs().has(&parked) {
        m.edge_attrs_mut().remove(&format!("{SEAM_OF}{to}"));
        m.edge_attrs_mut().rename(&parked, &format!("{SEAM_OF}{to}"));
    }
    set_marker(m, NAMES, &renamed.join("|"));
    set_marker(m, CURRENT, if cur == set { to } else { &cur });
    Ok(())
}

/// For a reader: names the sets a mesh was just given as uv0, uv1, ... in `order`, with `cur` in uv0. `order` holds
/// every name once, `cur` among them.
pub fn name_layers(m: &mut Mesh, order: &[String], cur: &str) {
    set_marker(m, NAMES, &order.join("|"));
    set_marker(m, CURRENT, cur);
}

/// For a reader: a set's seams, the current one's or one that is parked.
pub fn set_seams(m: &mut Mesh, set: &str, seams: &[bool]) {
    let attr = if current(m) == set { names::SEAM.to_string() } else { format!("{SEAM_OF}{set}") };
    let dst = m.edge_attrs_mut().get_or_add::<bool>(&attr);
    let n = dst.len().min(seams.len());
    dst[..n].copy_from_slice(&seams[..n]);
}

/// How many UV points a set has: corners of a point that share a value count once.
pub fn uv_count(m: &Mesh, set: &str) -> usize {
    let Some(l) = layer(m, set) else { return 0 };
    let uv = m.corner_attrs().get::<[f32; 2]>(&l);
    let mut seen = std::collections::HashSet::new();
    for c in 0..m.corner_capacity() {
        if m.corner_valid(c) {
            if let Some(v) = uv.get(c as usize) {
                seen.insert((m.corner_point(c), v[0].to_bits(), v[1].to_bits()));
            }
        }
    }
    seen.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cube() -> Mesh {
        aztek_geo::primitives::make_cube(&Default::default())
    }

    fn seam_count(m: &Mesh) -> usize {
        m.edge_attrs().get::<bool>(names::SEAM).iter().filter(|s| **s).count()
    }

    #[test]
    fn sets_keep_their_uvs_seams_names_and_order() {
        let mut m = cube();
        assert_eq!(all(&m), ["map1"]);
        let seams = seam_count(&m);
        let first = m.corner_attrs().get::<[f32; 2]>("uv0").to_vec();
        assert_eq!(create(&mut m, "bake"), "bake");
        assert_eq!(create(&mut m, ""), "uvSet1");
        assert_eq!(create(&mut m, ""), "uvSet2");
        assert_eq!(copy(&mut m, "map1", "bake").as_deref(), Some("bake"));
        assert_eq!(all(&m), ["map1", "bake", "uvSet1", "uvSet2"]);
        assert_eq!(layer(&m, "bake").as_deref(), Some("uv1"));

        // The empty set has no seams of its own; map1's come back with it.
        assert!(set_current(&mut m, "uvSet1"));
        assert_eq!(current(&m), "uvSet1");
        assert_eq!(seam_count(&m), 0);
        assert_eq!(all(&m), ["map1", "bake", "uvSet1", "uvSet2"], "the order does not follow the current set");
        assert_eq!(layer(&m, "map1").as_deref(), Some("uv1"));
        assert_eq!(layer(&m, "bake").as_deref(), Some("uv2"));
        assert!(set_current(&mut m, "bake"));
        assert_eq!(seam_count(&m), seams, "a copy has its source's seams");
        assert!(set_current(&mut m, "map1"));
        assert_eq!(seam_count(&m), seams);
        assert_eq!(m.corner_attrs().get::<[f32; 2]>("uv0"), &first[..]);

        rename(&mut m, "bake", "lightmap").unwrap();
        assert!(rename(&mut m, "nope", "x").is_err());
        assert!(rename(&mut m, "uvSet1", "map1").is_err());
        rename(&mut m, "map1", "base").unwrap();
        assert_eq!(current(&m), "base");
        assert_eq!(all(&m), ["base", "lightmap", "uvSet1", "uvSet2"]);

        delete(&mut m, "base").unwrap();
        assert_eq!(current(&m), "lightmap");
        assert_eq!(m.corner_attrs().get::<[f32; 2]>("uv0"), &first[..], "lightmap was a copy of the first set");
        assert_eq!(seam_count(&m), seams);
        delete(&mut m, "uvSet1").unwrap();
        delete(&mut m, "uvSet2").unwrap();
        assert!(delete(&mut m, "lightmap").is_err());
        assert!(!m.corner_attrs().has("uv1"));
        assert!(!set_current(&mut m, "gone"));
    }

    #[test]
    fn numbered_layers_without_names_are_sets() {
        let mut m = cube();
        m.corner_attrs_mut().get_or_add::<[f32; 2]>("uv1");
        assert_eq!(all(&m), ["map1", "map2"]);
        assert!(set_current(&mut m, "map2"));
        assert_eq!(all(&m), ["map1", "map2"]);
        assert_eq!(layer(&m, "map1").as_deref(), Some("uv1"));
    }
}
