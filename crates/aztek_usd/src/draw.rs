//! What a stage draws, worked out with no reference to the GPU: geometry shared between prims with the same data, and one
//! instance per place it appears. The work is done in slices so opening a big stage doesn't stall the UI; the viewport
//! draws what is ready and asks again.

use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::rc::Rc;
use std::time::{Duration, Instant};

use glam::{DMat4, DVec3, Vec3};
use openusd::gf;
use openusd::sdf;
use openusd::tf::Token;
use openusd::usd::{Prim, Stage, TimeCode};
use openusd_schemas::geom::XformQuery;
use openusd_schemas::shade;

use crate::mesh_read::{get_vec, read_gprim, token_of};
use crate::stages::Changes;
use crate::util::*;

/// One corner of a triangle list: 40 bytes, what the viewport's pipeline reads.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Default)]
#[allow(dead_code)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub nrm: [f32; 3],
    pub uv: [f32; 2],
    pub color: [u8; 4],
    pub _pad: [f32; 1],
}

/// Triangles ready for the GPU, shared by every prim with the same data.
pub struct Geom {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub lo: Vec3,
    pub hi: Vec3,
    /// Vertices carry their own colour: the instance's colour doesn't tint them.
    pub vertex_colors: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Look {
    pub color: [f32; 3],
    pub opacity: f32,
    pub texture: Option<String>,
}

impl Default for Look {
    fn default() -> Self {
        Look { color: [0.5, 0.5, 0.5], opacity: 1.0, texture: None }
    }
}

/// One thing to draw: geometry, where, and how it looks.
#[derive(Clone)]
pub struct Item {
    pub geom: usize,
    pub look: usize,
    pub world: DMat4,
    /// The prim, for picking and selection: the instance root for anything inside an instance.
    pub prim: Rc<str>,
    pub depends_on_time: bool,
}

/// What a stage's walk found under a prototype, relative to it, to repeat at every instance.
#[derive(Clone, Default)]
struct Template {
    items: Vec<(usize, usize, DMat4)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Purposes {
    pub render: bool,
    pub proxy: bool,
    pub guide: bool,
}

impl Default for Purposes {
    fn default() -> Self {
        Purposes { render: false, proxy: true, guide: false }
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Filter {
    pub root: String,
    pub exclude: Vec<String>,
    pub purposes: Purposes,
}

struct Task {
    path: sdf::Path,
    parent: DMat4,
    varying: bool,
    purpose: Rc<str>,
    /// Inside a template being recorded: skip visibility of the instance root's ancestors.
    inherited_look: Option<usize>,
}

/// A stage's drawing state, updated in slices.
pub struct Scene {
    pub geoms: Vec<Geom>,
    pub looks: Vec<Look>,
    pub items: Vec<Item>,
    geom_by_hash: HashMap<u64, usize>,
    look_by_path: HashMap<String, usize>,
    look_by_value: HashMap<String, usize>,
    templates: HashMap<String, Template>,
    queue: Vec<Task>,
    /// While a new pass runs over one already showing, its items go here and replace the old ones when it's done.
    building: Option<Vec<Item>>,
    /// Vertices held, to bound what animated points leave behind.
    stored_vertices: usize,
    /// Bumped when geoms were thrown away: the GPU's copies go too.
    pub generation: u64,
    /// A skinned mesh draws in its skeleton's space: where the last mesh read stands, if not where its prim does.
    posed_world: Option<DMat4>,
    /// Bumped whenever geoms or looks gain entries the GPU hasn't seen.
    pub geom_version: u64,
    /// Bumped whenever items change.
    pub item_version: u64,
    stage_version: u64,
    filter: Filter,
    time: f64,
    pub has_varying: bool,
    pub prims_visited: usize,
}

impl Default for Scene {
    fn default() -> Self {
        let mut looks = Vec::new();
        looks.push(Look::default());
        Scene {
            geoms: Vec::new(),
            looks,
            items: Vec::new(),
            geom_by_hash: HashMap::new(),
            look_by_path: HashMap::new(),
            look_by_value: HashMap::new(),
            templates: HashMap::new(),
            queue: Vec::new(),
            building: None,
            stored_vertices: 0,
            generation: 0,
            posed_world: None,
            geom_version: 0,
            item_version: 0,
            stage_version: 0,
            filter: Filter::default(),
            time: f64::NAN,
            has_varying: false,
            prims_visited: 0,
        }
    }
}

fn world_of_ops(prim: &Prim, t: f64) -> (DMat4, bool, bool) {
    let Ok(q) = XformQuery::for_prim(prim) else { return (DMat4::IDENTITY, false, false) };
    let m = q.local_transformation(TimeCode::new(t)).map(|m| from_gf(&m)).unwrap_or(DMat4::IDENTITY);
    (m, q.resets_xform_stack(), q.transform_might_be_time_varying().unwrap_or(false))
}

fn v(p: &gf::Vec3f) -> Vec3 {
    Vec3::new(p.x, p.y, p.z)
}

fn hash_geom(pts: &[gf::Vec3f], counts: &[i32], fvi: &[i32], uv: &[[f32; 2]], col: &[[u8; 4]], normals: &[gf::Vec3f]) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for p in pts {
        (p.x.to_bits(), p.y.to_bits(), p.z.to_bits()).hash(&mut h);
    }
    counts.hash(&mut h);
    fvi.hash(&mut h);
    for u in uv {
        (u[0].to_bits(), u[1].to_bits()).hash(&mut h);
    }
    col.hash(&mut h);
    for n in normals {
        (n.x.to_bits(), n.y.to_bits(), n.z.to_bits()).hash(&mut h);
    }
    h.finish()
}

/// Flattens a primvar to one value per face-vertex.
fn corner_values<T: Clone>(prim: &Prim, name: &str, values: &[T], fvi: &[i32], counts: &[i32], t: Option<f64>) -> Option<Vec<T>> {
    let a = prim.attribute(name);
    let interp = a.get_metadata::<Token>("interpolation").ok().flatten().map_or_else(|| "constant".to_string(), |x| x.as_str().to_string());
    let ix = get_vec::<i32>(&prim.attribute(format!("{name}:indices").as_str()), t);
    let fetch = |i: usize| -> Option<T> {
        let k = match &ix {
            Some(l) => usize::try_from(*l.get(i)?).ok()?,
            None => i,
        };
        values.get(k).cloned()
    };
    let mut out = Vec::with_capacity(fvi.len());
    match interp.as_str() {
        "faceVarying" => {
            for i in 0..fvi.len() {
                out.push(fetch(i)?);
            }
        }
        "vertex" | "varying" => {
            for &p in fvi {
                out.push(fetch(p.max(0) as usize)?);
            }
        }
        "uniform" => {
            for (f, &c) in counts.iter().enumerate() {
                let x = fetch(f)?;
                for _ in 0..c {
                    out.push(x.clone());
                }
            }
        }
        _ => {
            let x = fetch(0)?;
            out.resize(fvi.len(), x);
        }
    }
    Some(out)
}

/// The nearest skeleton a prim is bound to, inherited down namespace.
fn bound_skeleton(prim: &Prim) -> Option<sdf::Path> {
    let mut p = Some(prim.clone());
    while let Some(q) = p {
        if let Some(first) = q.relationship("skel:skeleton").targets().ok().and_then(|t| t.first().cloned()) {
            return Some(first);
        }
        p = q.parent();
    }
    None
}

fn tokens(prim: &Prim, name: &str) -> Vec<String> {
    prim.attribute(name).get::<Vec<Token>>().ok().flatten().map(|v| v.iter().map(|x| x.as_str().to_string()).collect()).unwrap_or_default()
}

fn stack_world(stage: &Stage, path: &sdf::Path, t: f64) -> DMat4 {
    let mut chain: Vec<sdf::Path> = path.ancestors_below_root().collect();
    chain.reverse();
    let mut m = DMat4::IDENTITY;
    for a in chain {
        if let Ok(p) = stage.prim(a) {
            let (l, reset, _) = world_of_ops(&p, t);
            m = if reset { l } else { m * l };
        }
    }
    m
}

/// A mesh's points posed by its skeleton and blend shapes, with where they then stand: UsdSkel's points are in the skeleton's
/// space. None when nothing poses it.
fn pose(stage: &Stage, prim: &Prim, rest: &[gf::Vec3f], t: f64) -> Option<(Vec<gf::Vec3f>, DMat4, bool)> {
    let skel_path = bound_skeleton(prim)?;
    let skel = stage.prim(skel_path.clone()).ok()?;
    let anim = skel.relationship("skel:animationSource").targets().ok().and_then(|x| x.first().cloned()).and_then(|a| stage.prim(a).ok());
    let tc = Some(t);
    let mut varying = false;
    let mut pts: Vec<Vec3> = rest.iter().map(v).collect();

    // Blend shapes first: offsets weighted by the animation's channels.
    let shapes = tokens(prim, "skel:blendShapes");
    let targets = prim.relationship("skel:blendShapeTargets").targets().unwrap_or_default();
    if let (Some(a), false) = (&anim, shapes.is_empty()) {
        let order = tokens(a, "blendShapes");
        let wa = a.attribute("blendShapeWeights");
        varying |= wa.value_might_be_time_varying().unwrap_or(false);
        let weights = get_vec::<f32>(&wa, tc).unwrap_or_default();
        for (name, target) in shapes.iter().zip(&targets) {
            let Some(w) = order.iter().position(|x| x == name).and_then(|k| weights.get(k).copied()) else { continue };
            if w == 0.0 {
                continue;
            }
            let Ok(tp) = stage.prim(target.clone()) else { continue };
            let off = get_vec::<gf::Vec3f>(&tp.attribute("offsets"), None).unwrap_or_default();
            let ids = get_vec::<i32>(&tp.attribute("pointIndices"), None);
            for (k, o) in off.iter().enumerate() {
                let at = ids.as_ref().map_or(k, |l| l.get(k).copied().unwrap_or(-1) as usize);
                if let Some(q) = pts.get_mut(at) {
                    *q += v(o) * w;
                }
            }
        }
    }

    let joints = tokens(&skel, "joints");
    let ji_attr = prim.attribute("primvars:skel:jointIndices");
    let weights = (get_vec::<i32>(&ji_attr, None), get_vec::<f32>(&prim.attribute("primvars:skel:jointWeights"), None));
    let skel_world = stack_world(stage, &skel_path, t);
    let (Some(ji), Some(jw)) = weights else {
        // No skin: blended points stand where the prim does.
        if shapes.is_empty() {
            return None;
        }
        let out: Vec<gf::Vec3f> = pts.iter().map(|p| gf::vec3f(p.x, p.y, p.z)).collect();
        return Some((out, stack_world(stage, prim.path(), t), varying));
    };
    if joints.is_empty() {
        return None;
    }
    let bind: Vec<DMat4> = get_vec::<gf::Matrix4d>(&skel.attribute("bindTransforms"), None)?.iter().map(from_gf).collect();
    let rest_local: Vec<DMat4> = get_vec::<gf::Matrix4d>(&skel.attribute("restTransforms"), None).map(|m| m.iter().map(from_gf).collect()).unwrap_or_default();

    // Each joint's local matrix: the animation's where it has one, else the rest pose.
    let mut local: Vec<DMat4> = (0..joints.len()).map(|k| rest_local.get(k).copied().unwrap_or(DMat4::IDENTITY)).collect();
    if let Some(a) = &anim {
        let order = tokens(a, "joints");
        let (ta, ra, sa) = (a.attribute("translations"), a.attribute("rotations"), a.attribute("scales"));
        varying |= ta.value_might_be_time_varying().unwrap_or(false) || ra.value_might_be_time_varying().unwrap_or(false) || sa.value_might_be_time_varying().unwrap_or(false);
        let (tr, ro, sc) = (get_vec::<gf::Vec3f>(&ta, tc).unwrap_or_default(), get_vec::<gf::Quatf>(&ra, tc).unwrap_or_default(), get_vec::<gf::Vec3h>(&sa, tc).unwrap_or_default());
        for (k, name) in order.iter().enumerate() {
            let Some(j) = joints.iter().position(|x| x == name) else { continue };
            if let (Some(p), Some(q)) = (tr.get(k), ro.get(k)) {
                let s = sc.get(k).map_or(DVec3::ONE, |s| DVec3::new(s.x.to_f64(), s.y.to_f64(), s.z.to_f64()));
                let q = glam::DQuat::from_xyzw(q.x as f64, q.y as f64, q.z as f64, q.w as f64).normalize();
                local[j] = DMat4::from_scale_rotation_translation(s, q, DVec3::new(p.x as f64, p.y as f64, p.z as f64));
            }
        }
    }
    // Joints in the skeleton's space, parents first as the token paths order them.
    let mut world = vec![DMat4::IDENTITY; joints.len()];
    for k in 0..joints.len() {
        let up = joints[k].rsplit_once('/').and_then(|(u, _)| joints.iter().position(|x| x == u));
        world[k] = up.map_or(local[k], |u| world[u] * local[k]);
    }
    let skin: Vec<DMat4> = world.iter().enumerate().map(|(k, w)| *w * bind.get(k).copied().unwrap_or(DMat4::IDENTITY).inverse()).collect();

    let own = tokens(prim, "skel:joints");
    let order: Vec<usize> = if own.is_empty() { (0..joints.len()).collect() } else { own.iter().filter_map(|n| joints.iter().position(|x| x == n)).collect() };
    let width = ji_attr.get_metadata::<i32>("elementSize").ok().flatten().unwrap_or(1).max(1) as usize;
    let rigid = ji_attr.get_metadata::<Token>("interpolation").ok().flatten().is_some_and(|x| x.as_str() == "constant");
    let geom = prim.attribute("primvars:skel:geomBindTransform").get::<gf::Matrix4d>().ok().flatten().map_or(DMat4::IDENTITY, |m| from_gf(&m));
    let mut out = Vec::with_capacity(pts.len());
    for (i, p) in pts.iter().enumerate() {
        let base = if rigid { 0 } else { i * width };
        let bound = geom.transform_point3(p.as_dvec3());
        let mut sum = DVec3::ZERO;
        let mut total = 0.0;
        for w in 0..width {
            let (Some(&ix), Some(&wt)) = (ji.get(base + w), jw.get(base + w)) else { continue };
            if wt <= 0.0 {
                continue;
            }
            let Some(m) = order.get(ix.max(0) as usize).and_then(|k| skin.get(*k)) else { continue };
            sum += m.transform_point3(bound) * wt as f64;
            total += wt as f64;
        }
        let q = if total > 0.0 { sum / total } else { bound };
        out.push(v3f(q));
    }
    Some((out, skel_world, varying))
}

fn srgb8(x: f32) -> u8 {
    (x.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

/// Triangles for a set of faces of a mesh, or all of them.
#[allow(clippy::too_many_arguments)]
fn triangles(pts: &[gf::Vec3f], counts: &[i32], fvi: &[i32], uv: &[[f32; 2]], col: &[[u8; 4]], normals: Option<&[gf::Vec3f]>, left: bool, only: Option<&HashSet<u32>>) -> Geom {
    // Smooth vertex normals unless the file gives its own.
    let mut smooth = vec![Vec3::ZERO; pts.len()];
    if normals.is_none() {
        let mut s = 0usize;
        for &c in counts {
            let c = c.max(0) as usize;
            if s + c > fvi.len() {
                break;
            }
            let f = &fvi[s..s + c];
            let mut n = Vec3::ZERO;
            for i in 0..c {
                let (Some(a), Some(b)) = (pts.get(f[i] as usize), pts.get(f[(i + 1) % c] as usize)) else { continue };
                n += v(a).cross(v(b));
            }
            if left {
                n = -n;
            }
            for &i in f {
                if let Some(x) = smooth.get_mut(i as usize) {
                    *x += n;
                }
            }
            s += c;
        }
        for n in &mut smooth {
            *n = n.normalize_or_zero();
        }
    }
    let mut g = Geom { vertices: Vec::new(), indices: Vec::new(), lo: Vec3::splat(f32::MAX), hi: Vec3::splat(f32::MIN), vertex_colors: !col.is_empty() };
    let mut start = 0usize;
    for (fi, &c) in counts.iter().enumerate() {
        let c = c.max(0) as usize;
        if start + c > fvi.len() {
            break;
        }
        if c >= 3 && only.is_none_or(|o| o.contains(&(fi as u32))) {
            let base = g.vertices.len() as u32;
            for k in 0..c {
                let corner = start + k;
                let Some(p) = pts.get(fvi[corner] as usize) else { continue };
                let pos = v(p);
                let nrm = match normals {
                    Some(n) => n.get(corner).map_or(Vec3::Y, v),
                    None => smooth[fvi[corner] as usize],
                };
                g.lo = g.lo.min(pos);
                g.hi = g.hi.max(pos);
                g.vertices.push(Vertex { pos: pos.to_array(), nrm: nrm.to_array(), uv: uv.get(corner).copied().unwrap_or([0.0, 0.0]), color: col.get(corner).copied().unwrap_or([255, 255, 255, 255]), _pad: [0.0] });
            }
            if g.vertices.len() as u32 == base + c as u32 {
                for k in 1..c as u32 - 1 {
                    if left {
                        g.indices.extend_from_slice(&[base, base + k + 1, base + k]);
                    } else {
                        g.indices.extend_from_slice(&[base, base + k, base + k + 1]);
                    }
                }
            }
        }
        start += c;
    }
    if g.vertices.is_empty() {
        g.lo = Vec3::ZERO;
        g.hi = Vec3::ZERO;
    }
    g
}

impl Scene {
    fn add_geom(&mut self, hash: u64, make: impl FnOnce() -> Geom) -> usize {
        if let Some(&i) = self.geom_by_hash.get(&hash) {
            return i;
        }
        let g = make();
        self.stored_vertices += g.vertices.len();
        self.geoms.push(g);
        self.geom_version += 1;
        let i = self.geoms.len() - 1;
        self.geom_by_hash.insert(hash, i);
        i
    }

    fn emit(&mut self, it: Item) {
        match &mut self.building {
            Some(b) => b.push(it),
            None => {
                self.items.push(it);
                self.item_version += 1;
            }
        }
    }

    fn look_value(&mut self, l: Look) -> usize {
        let key = format!("{:?}", l);
        if let Some(&i) = self.look_by_value.get(&key) {
            return i;
        }
        self.looks.push(l);
        self.geom_version += 1;
        let i = self.looks.len() - 1;
        self.look_by_value.insert(key, i);
        i
    }

    fn material_look(&mut self, stage: &Stage, m: &sdf::Path) -> Option<usize> {
        let key = m.as_str().to_string();
        if let Some(&i) = self.look_by_path.get(&key) {
            return Some(i);
        }
        let surf = shade::read_preview_surface(stage, m).ok().flatten()?;
        let mut l = Look::default();
        if let Some(c) = surf.diffuse_color.value() {
            l.color = [c.x, c.y, c.z];
        }
        l.texture = surf.diffuse_color.texture().map(|authored| {
            // The asset as resolved, which the channel doesn't always say: follow the shader's connection.
            let shader = stage.prim(surf.shader.as_str()).ok();
            let source = shader.and_then(|sh| sh.attribute("inputs:diffuseColor").connections().ok()).and_then(|c| c.first().cloned());
            let tex = source.and_then(|c| c.split_property().map(|x| x.0)).and_then(|p| stage.prim(p).ok());
            let resolved = tex.and_then(|tp| tp.attribute("inputs:file").get::<sdf::AssetPath>().ok().flatten()).and_then(|a| a.resolved_path().map(str::to_string));
            crate::mesh_read::asset_on_disk(resolved.as_deref().unwrap_or(authored))
        });
        if l.texture.is_some() {
            l.color = [1.0, 1.0, 1.0];
        }
        if let Some(o) = surf.opacity.value() {
            l.opacity = *o;
        }
        let i = self.look_value(l);
        self.look_by_path.insert(key, i);
        Some(i)
    }

    /// The look of a prim: its material, inherited, else its display colour.
    fn prim_look(&mut self, stage: &Stage, prim: &Prim, inherited: Option<usize>) -> usize {
        let direct = prim.relationship("material:binding").targets().ok().and_then(|t| t.first().cloned());
        if let Some(m) = direct {
            if let Some(i) = self.material_look(stage, &m) {
                return i;
            }
        }
        if let Some(i) = inherited {
            return i;
        }
        if let Some(col) = crate::mesh_read::constant_display_color(prim) {
            return self.look_value(Look { color: col, opacity: 1.0, texture: None });
        }
        0
    }

    fn mesh_items(&mut self, stage: &Stage, prim: &Prim, t: f64, look: usize) -> Vec<(usize, usize)> {
        let tc = Some(t);
        let Some(mut points) = get_vec::<gf::Vec3f>(&prim.attribute("points"), tc) else { return Vec::new() };
        if let Some((posed, at_world, varying)) = pose(stage, prim, &points, t) {
            points = posed;
            self.posed_world = Some(at_world);
            self.has_varying |= varying;
        }
        let counts = get_vec::<i32>(&prim.attribute("faceVertexCounts"), tc).unwrap_or_default();
        let fvi = get_vec::<i32>(&prim.attribute("faceVertexIndices"), tc).unwrap_or_default();
        let left = token_of(prim, "orientation").as_deref() == Some("leftHanded");
        // st, or the first set of coordinates.
        let mut uv: Vec<[f32; 2]> = Vec::new();
        for name in ["primvars:st", "primvars:st0", "primvars:UVMap", "primvars:uv", "primvars:map1"] {
            let a = prim.attribute(name);
            if let Some(vals) = get_vec::<gf::Vec2f>(&a, tc) {
                let list: Vec<[f32; 2]> = vals.iter().map(|x| [x.x, x.y]).collect();
                if let Some(c) = corner_values(prim, name, &list, &fvi, &counts, tc) {
                    uv = c;
                    break;
                }
            }
        }
        let mut col: Vec<[u8; 4]> = Vec::new();
        if let Some(vals) = get_vec::<gf::Vec3f>(&prim.attribute("primvars:displayColor"), tc) {
            let interp = prim.attribute("primvars:displayColor").get_metadata::<Token>("interpolation").ok().flatten();
            if interp.is_some_and(|i| i.as_str() != "constant") {
                let list: Vec<[u8; 4]> = vals.iter().map(|c| [srgb8(c.x), srgb8(c.y), srgb8(c.z), 255]).collect();
                if let Some(c) = corner_values(prim, "primvars:displayColor", &list, &fvi, &counts, tc) {
                    col = c;
                }
            }
        }
        let normals_owned: Option<Vec<gf::Vec3f>> = get_vec::<gf::Vec3f>(&prim.attribute("normals"), tc).and_then(|n| {
            let a = prim.attribute("normals");
            let interp = a.get_metadata::<Token>("interpolation").ok().flatten().map_or_else(|| "vertex".to_string(), |x| x.as_str().to_string());
            match interp.as_str() {
                "faceVarying" if n.len() == fvi.len() => Some(n),
                "vertex" | "varying" if n.len() == points.len() => Some(fvi.iter().map(|&i| n[i.max(0) as usize]).collect()),
                _ => None,
            }
        });
        let normals = normals_owned.as_deref();
        let base_hash = hash_geom(&points, &counts, &fvi, &uv, &col, normals.unwrap_or(&[]));

        // Subsets bound to their own material each get their own triangles.
        let mut parts: Vec<(HashSet<u32>, usize, u64)> = Vec::new();
        let mut covered: HashSet<u32> = HashSet::new();
        for k in prim.children().unwrap_or_default() {
            if k.type_name().ok().flatten().is_none_or(|n| n.as_str() != "GeomSubset") {
                continue;
            }
            let Some(m) = k.relationship("material:binding").targets().ok().and_then(|x| x.first().cloned()) else { continue };
            let Some(mlook) = self.material_look(stage, &m) else { continue };
            let ids = get_vec::<i32>(&k.attribute("indices"), tc).unwrap_or_default();
            let set: HashSet<u32> = ids.iter().map(|&i| i as u32).collect();
            let mut h = std::collections::hash_map::DefaultHasher::new();
            base_hash.hash(&mut h);
            ids.hash(&mut h);
            covered.extend(set.iter().copied());
            parts.push((set, mlook, h.finish()));
        }
        let mut out = Vec::new();
        if parts.is_empty() {
            let i = self.add_geom(base_hash, || triangles(&points, &counts, &fvi, &uv, &col, normals, left, None));
            out.push((i, look));
            return out;
        }
        for (set, mlook, hash) in parts {
            let i = self.add_geom(hash, || triangles(&points, &counts, &fvi, &uv, &col, normals, left, Some(&set)));
            out.push((i, mlook));
        }
        let rest: HashSet<u32> = (0..counts.len() as u32).filter(|f| !covered.contains(f)).collect();
        if !rest.is_empty() {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            base_hash.hash(&mut h);
            0xfeed_u32.hash(&mut h);
            let i = self.add_geom(h.finish(), || triangles(&points, &counts, &fvi, &uv, &col, normals, left, Some(&rest)));
            out.push((i, look));
        }
        out
    }

    /// A small shape standing for a prim with no surface: a light or a camera.
    fn glyph(&mut self, _prim: &Prim, _t: f64, color: [f32; 3], size: f32) -> Vec<(usize, usize)> {
        let s = size;
        let pts = [(s, 0.0, 0.0), (-s, 0.0, 0.0), (0.0, s, 0.0), (0.0, -s, 0.0), (0.0, 0.0, s), (0.0, 0.0, -s)].map(|p| gf::vec3f(p.0, p.1, p.2));
        let fvi = [0, 2, 4, 2, 1, 4, 1, 3, 4, 3, 0, 4, 2, 0, 5, 1, 2, 5, 3, 1, 5, 0, 3, 5];
        let counts = [3; 8];
        let hash = hash_geom(&pts, &counts, &fvi, &[], &[], &[]);
        let g = self.add_geom(hash, || triangles(&pts, &counts, &fvi, &[], &[], None, false, None));
        let l = self.look_value(Look { color, opacity: 1.0, texture: None });
        vec![(g, l)]
    }

    fn gprim_items(&mut self, prim: &Prim, ty: &str, t: f64, look: usize) -> Vec<(usize, usize)> {
        let Some(m) = read_gprim(prim, ty, Some(t)) else { return Vec::new() };
        let pts: Vec<gf::Vec3f> = m.positions().iter().map(|p| v3f(*p)).collect();
        let mut counts = Vec::new();
        let mut fvi = Vec::new();
        for f in 0..m.num_faces() {
            let p = m.face_points(f);
            counts.push(p.len() as i32);
            fvi.extend(p.iter().map(|&i| i as i32));
        }
        // Implicit shapes have their hard edges: a cube's corners don't average.
        let by_corner = aztek_geo::normals::corner_normals(&m, &aztek_geo::normals::CornerNormalOptions::default());
        let mut normals = Vec::with_capacity(fvi.len());
        for f in 0..m.num_faces() {
            for c in m.face_corners(f) {
                let n = by_corner[c as usize];
                normals.push(gf::vec3f(n.x, n.y, n.z));
            }
        }
        let hash = hash_geom(&pts, &counts, &fvi, &[], &[], &normals);
        let i = self.add_geom(hash, || triangles(&pts, &counts, &fvi, &[], &[], Some(&normals), false, None));
        vec![(i, look)]
    }

    /// Everything a prim of this subtree contributes relative to `root`, for repeating under instances.
    fn record_template(&mut self, stage: &Stage, proto: &sdf::Path, t: f64) -> Template {
        let mut out = Template::default();
        let mut stack: Vec<(sdf::Path, DMat4, Option<usize>)> = vec![(proto.clone(), DMat4::IDENTITY, None)];
        while let Some((path, parent, inherited)) = stack.pop() {
            let Ok(prim) = stage.prim(path.clone()) else { continue };
            let ty = prim.type_name().ok().flatten().map_or(String::new(), |x| x.as_str().to_string());
            let local = if path == *proto { DMat4::IDENTITY } else { world_of_ops(&prim, t).0 };
            let world = parent * local;
            if token_of(&prim, "visibility").as_deref() == Some("invisible") {
                continue;
            }
            let look = self.prim_look(stage, &prim, inherited);
            let found = match ty.as_str() {
                "Mesh" => self.mesh_items(stage, &prim, t, look),
                "Cube" | "Sphere" | "Cylinder" | "Cone" | "Capsule" | "Plane" => self.gprim_items(&prim, &ty, t, look),
                _ => Vec::new(),
            };
            for (g, l) in found {
                out.items.push((g, l, world));
            }
            for k in prim.children().unwrap_or_default() {
                stack.push((k.path().clone(), world, Some(look).filter(|l| *l != 0)));
            }
        }
        out
    }

    /// Starts over for a changed stage, filter or time.
    fn restart(&mut self, stage: &Stage, filter: &Filter, t: f64) {
        // What animated points leave behind is bounded: past this the geometry starts afresh.
        const VERTEX_LIMIT: usize = 20_000_000;
        if self.stored_vertices > VERTEX_LIMIT {
            self.geoms.clear();
            self.geom_by_hash.clear();
            self.stored_vertices = 0;
            self.generation += 1;
            self.items.clear();
            self.item_version += 1;
        }
        // What's showing stays until the new pass has finished, so scrubbing a big stage doesn't flicker.
        self.building = (!self.items.is_empty()).then(Vec::new);
        self.queue.clear();
        self.has_varying = false;
        self.prims_visited = 0;
        self.filter = filter.clone();
        self.time = t;
        self.look_by_path.clear();
        self.templates.clear();
        let root = if filter.root.is_empty() { None } else { sdf::Path::new(&filter.root).ok() };
        match root {
            Some(r) => {
                // The shown prim's ancestors still place it.
                let mut parent = DMat4::IDENTITY;
                let mut chain: Vec<sdf::Path> = r.ancestors_below_root().collect();
                chain.reverse();
                for a in chain {
                    if a != r {
                        if let Ok(p) = stage.prim(a) {
                            parent *= world_of_ops(&p, t).0;
                        }
                    }
                }
                self.queue.push(Task { path: r, parent, varying: false, purpose: Rc::from("default"), inherited_look: None });
            }
            None => {
                if let Ok(names) = stage.root_prims() {
                    for n in names.iter().rev() {
                        if let Ok(path) = sdf::Path::new(&format!("/{}", n.as_str())) {
                            self.queue.push(Task { path, parent: DMat4::IDENTITY, varying: false, purpose: Rc::from("default"), inherited_look: None });
                        }
                    }
                }
            }
        }
    }

    fn purpose_shown(&self, purpose: &str) -> bool {
        match purpose {
            "render" => self.filter.purposes.render,
            "proxy" => self.filter.purposes.proxy,
            "guide" => self.filter.purposes.guide,
            _ => true,
        }
    }

    fn step(&mut self, stage: &Stage, task: Task) {
        let t = self.time;
        self.prims_visited += 1;
        let Ok(prim) = stage.prim(task.path.clone()) else { return };
        if !prim.is_valid().unwrap_or(false) || !prim.is_active().unwrap_or(true) || !prim.is_defined().unwrap_or(true) || prim.is_abstract().unwrap_or(false) {
            return;
        }
        if !prim.is_loaded().unwrap_or(true) {
            return;
        }
        if self.filter.exclude.iter().any(|e| e == task.path.as_str()) {
            return;
        }
        let ty = prim.type_name().ok().flatten().map_or(String::new(), |x| x.as_str().to_string());
        if matches!(ty.as_str(), "Material" | "Shader" | "NodeGraph" | "Skeleton" | "SkelAnimation" | "BlendShape" | "GeomSubset") {
            return;
        }
        let vis = prim.attribute("visibility");
        if vis.get_at::<Token>(TimeCode::new(t)).ok().flatten().is_some_and(|x| x.as_str() == "invisible") {
            if vis.value_might_be_time_varying().unwrap_or(false) {
                self.has_varying = true;
            }
            return;
        }
        // Purpose carries down until a prim says otherwise.
        let own = prim.attribute("purpose").get::<Token>().ok().flatten().filter(|p| p.as_str() != "default").map(|p| p.as_str().to_string());
        let purpose: Rc<str> = own.map_or_else(|| task.purpose.clone(), |p| Rc::from(p.as_str()));
        if !self.purpose_shown(&purpose) {
            return;
        }
        let (local, reset, xv) = world_of_ops(&prim, t);
        let world = if reset { local } else { task.parent * local };
        let varying = task.varying || xv;
        if varying {
            self.has_varying = true;
        }
        let look = self.prim_look(stage, &prim, task.inherited_look);
        let prim_name: Rc<str> = Rc::from(task.path.as_str());

        // An instance repeats its prototype's template.
        if prim.is_instance().unwrap_or(false) {
            if let Ok(Some(proto)) = prim.prototype() {
                let key = proto.as_str().to_string();
                if !self.templates.contains_key(&key) {
                    let tm = self.record_template(stage, &proto, t);
                    self.templates.insert(key.clone(), tm);
                }
                let list = self.templates[&key].items.clone();
                for (g, l, rel) in list {
                    self.emit(Item { geom: g, look: if l == 0 { look } else { l }, world: world * rel, prim: prim_name.clone(), depends_on_time: varying });
                }
            }
            return;
        }

        let found = match ty.as_str() {
            "Mesh" => {
                let pa = prim.attribute("points");
                if pa.value_might_be_time_varying().unwrap_or(false) {
                    self.has_varying = true;
                }
                self.mesh_items(stage, &prim, t, look)
            }
            "Cube" | "Sphere" | "Cylinder" | "Cone" | "Capsule" | "Plane" => self.gprim_items(&prim, &ty, t, look),
            "PointInstancer" => {
                self.instancer(stage, &prim, world, t, &prim_name);
                return;
            }
            "Camera" => self.glyph(&prim, t, [0.45, 0.6, 0.8], 0.4),
            x if x.ends_with("Light") => {
                let c = prim.attribute("inputs:color").get_at::<gf::Vec3f>(TimeCode::new(t)).ok().flatten().map_or([1.0, 0.95, 0.7], |c| [c.x, c.y, c.z]);
                self.glyph(&prim, t, c, 0.25)
            }
            _ => Vec::new(),
        };
        let shown_at = self.posed_world.take().unwrap_or(world);
        for (g, l) in found {
            self.emit(Item { geom: g, look: l, world: shown_at, prim: prim_name.clone(), depends_on_time: varying });
        }
        let kids = prim.child_names().unwrap_or_default();
        for n in kids.iter().rev() {
            if let Ok(path) = task.path.append_path(n.as_str()) {
                self.queue.push(Task { path, parent: world, varying, purpose: purpose.clone(), inherited_look: Some(look).filter(|l| *l != 0) });
            }
        }
    }

    fn instancer(&mut self, stage: &Stage, prim: &Prim, world: DMat4, t: f64, name: &Rc<str>) {
        let protos = prim.relationship("prototypes").targets().unwrap_or_default();
        let idx = get_vec::<i32>(&prim.attribute("protoIndices"), Some(t)).unwrap_or_default();
        let pos = get_vec::<gf::Vec3f>(&prim.attribute("positions"), Some(t)).unwrap_or_default();
        let ori = get_vec::<gf::Quath>(&prim.attribute("orientations"), Some(t)).unwrap_or_default();
        let scl = get_vec::<gf::Vec3f>(&prim.attribute("scales"), Some(t)).unwrap_or_default();
        for target in &protos {
            let key = target.as_str().to_string();
            if !self.templates.contains_key(&key) {
                let tm = self.record_template(stage, target, t);
                self.templates.insert(key, tm);
            }
        }
        for (i, &k) in idx.iter().enumerate() {
            let Some(target) = protos.get(k.max(0) as usize) else { continue };
            let p = pos.get(i).map_or(DVec3::ZERO, |p| DVec3::new(p.x as f64, p.y as f64, p.z as f64));
            let q = ori.get(i).map_or(glam::DQuat::IDENTITY, |q| glam::DQuat::from_xyzw(q.x.to_f64(), q.y.to_f64(), q.z.to_f64(), q.w.to_f64()));
            let s = scl.get(i).map_or(DVec3::ONE, |s| DVec3::new(s.x as f64, s.y as f64, s.z as f64));
            let m = world * DMat4::from_scale_rotation_translation(s, q, p);
            let list = self.templates[target.as_str()].items.clone();
            for (g, l, rel) in list {
                self.emit(Item { geom: g, look: l, world: m * rel, prim: name.clone(), depends_on_time: false });
            }
        }
    }

    /// A prim's own items again after an edit: what it and its subtree draw, nothing else.
    fn refresh_prim(&mut self, stage: &Stage, path: &sdf::Path) {
        let p = path.as_str().to_string();
        let under = |q: &str| q == p || (q.starts_with(&p) && q.as_bytes().get(p.len()) == Some(&b'/'));
        self.items.retain(|it| !under(&it.prim));
        self.item_version += 1;
        if !self.filter.root.is_empty() && !under(&self.filter.root) && !self.filter.root.starts_with(&format!("{p}/")) && !p.starts_with(&format!("{}/", self.filter.root)) {
            return;
        }
        // What's above says where it stands and whether it shows at all.
        let t = self.time;
        let mut parent = DMat4::IDENTITY;
        let mut purpose: Rc<str> = Rc::from("default");
        let mut look = None;
        let mut varying = false;
        let mut chain: Vec<sdf::Path> = path.ancestors_below_root().collect();
        chain.retain(|a| a != path);
        chain.reverse();
        for a in chain {
            if self.filter.exclude.iter().any(|e| e == a.as_str()) {
                return;
            }
            let Ok(pr) = stage.prim(a) else { return };
            if !pr.is_valid().unwrap_or(false) || !pr.is_active().unwrap_or(true) || !pr.is_loaded().unwrap_or(true) {
                return;
            }
            if pr.attribute("visibility").get_at::<Token>(TimeCode::new(t)).ok().flatten().is_some_and(|x| x.as_str() == "invisible") {
                return;
            }
            if let Some(x) = pr.attribute("purpose").get::<Token>().ok().flatten().filter(|x| x.as_str() != "default") {
                purpose = Rc::from(x.as_str());
            }
            if !self.purpose_shown(&purpose) {
                return;
            }
            let (l, reset, v) = world_of_ops(&pr, t);
            parent = if reset { l } else { parent * l };
            varying |= v;
            let found = self.prim_look(stage, &pr, look);
            look = Some(found).filter(|x| *x != 0);
        }
        self.queue.push(Task { path: path.clone(), parent, varying, purpose, inherited_look: look });
    }

    /// Whether the changed paths can be handled one prim at a time. False sends a full pass.
    fn apply_changes(&mut self, stage: &Stage, version_seen: u64, changes: &Changes) -> bool {
        if changes.everything_at > version_seen || version_seen == 0 || !self.queue.is_empty() || self.building.is_some() {
            return false;
        }
        let mut prims: Vec<sdf::Path> = Vec::new();
        for (v, path) in &changes.log {
            if *v <= version_seen {
                continue;
            }
            if path.is_abs_root() || path.as_str().starts_with("/__") {
                return false;
            }
            let prim_path = if path.is_property_path() { path.prim_path() } else { path.clone() };
            // Materials reach whatever is bound to them.
            let ty = stage.prim(prim_path.clone()).ok().and_then(|p| p.type_name().ok().flatten());
            if ty.is_some_and(|x| matches!(x.as_str(), "Material" | "Shader" | "NodeGraph" | "Skeleton" | "SkelAnimation" | "BlendShape")) {
                return false;
            }
            if !prims.contains(&prim_path) {
                prims.push(prim_path);
            }
            if prims.len() > 256 {
                return false;
            }
        }
        // Only the highest of nested edits.
        let tops: Vec<sdf::Path> = prims.iter().filter(|p| !prims.iter().any(|q| q != *p && p.has_prefix(q))).cloned().collect();
        for p in tops {
            self.refresh_prim(stage, &p);
        }
        true
    }

    /// Brings the scene up to date with the stage, working for at most `budget`. Returns true when finished.
    pub fn update(&mut self, stage: &Stage, stage_version: u64, changes: &Changes, filter: &Filter, t: f64, budget: Duration) -> bool {
        let moved = t != self.time && (self.has_varying || self.time.is_nan());
        if *filter != self.filter || moved {
            self.stage_version = stage_version;
            self.restart(stage, filter, t);
        } else if stage_version != self.stage_version {
            let seen = self.stage_version;
            self.stage_version = stage_version;
            self.time = t;
            if !self.apply_changes(stage, seen, changes) {
                self.restart(stage, filter, t);
            }
        }
        self.time = t;
        let start = Instant::now();
        let mut n = 0u32;
        while let Some(task) = self.queue.pop() {
            self.step(stage, task);
            n += 1;
            if n % 16 == 0 && start.elapsed() >= budget {
                return self.queue.is_empty() && self.finish_pass();
            }
        }
        self.finish_pass()
    }

    fn finish_pass(&mut self) -> bool {
        if let Some(b) = self.building.take() {
            self.items = b;
            self.item_version += 1;
        }
        true
    }

    /// The nearest item a world-space ray hits: (prim path, distance along the ray, world point).
    pub fn raycast(&self, origin: DVec3, dir: DVec3) -> Option<(Rc<str>, f64, DVec3)> {
        let mut best: Option<(Rc<str>, f64, DVec3)> = None;
        for it in &self.items {
            let g = &self.geoms[it.geom];
            let inv = it.world.inverse();
            let (o, d) = (inv.transform_point3(origin), inv.transform_vector3(dir));
            if !ray_box(o, d, g.lo.as_dvec3(), g.hi.as_dvec3()) {
                continue;
            }
            for tri in g.indices.chunks_exact(3) {
                let p = |k: usize| Vec3::from_array(g.vertices[tri[k] as usize].pos).as_dvec3();
                let Some(t) = ray_tri(o, d, p(0), p(1), p(2)) else { continue };
                let hit = it.world.transform_point3(o + d * t);
                let dist = (hit - origin).dot(dir);
                if dist > 0.0 && best.as_ref().is_none_or(|b| dist < b.1) {
                    best = Some((it.prim.clone(), dist, hit));
                }
            }
        }
        best
    }

    /// The world bounds of everything at or under a prim, or of the whole scene with no prim.
    pub fn bounds(&self, under: Option<&str>) -> Option<(DVec3, DVec3)> {
        let (mut lo, mut hi) = (DVec3::splat(f64::MAX), DVec3::splat(f64::MIN));
        let mut any = false;
        for it in &self.items {
            if let Some(u) = under {
                let p: &str = &it.prim;
                if !(p == u || p.starts_with(u) && p.as_bytes().get(u.len()) == Some(&b'/')) {
                    continue;
                }
            }
            let g = &self.geoms[it.geom];
            for c in 0..8 {
                let x = if c & 1 == 0 { g.lo.x } else { g.hi.x } as f64;
                let y = if c & 2 == 0 { g.lo.y } else { g.hi.y } as f64;
                let z = if c & 4 == 0 { g.lo.z } else { g.hi.z } as f64;
                let w = it.world.transform_point3(DVec3::new(x, y, z));
                lo = lo.min(w);
                hi = hi.max(w);
            }
            any = true;
        }
        any.then_some((lo, hi))
    }
}

fn ray_box(o: DVec3, d: DVec3, lo: DVec3, hi: DVec3) -> bool {
    let (mut t0, mut t1) = (f64::MIN, f64::MAX);
    for i in 0..3 {
        if d[i].abs() < 1e-12 {
            if o[i] < lo[i] || o[i] > hi[i] {
                return false;
            }
        } else {
            let (a, b) = ((lo[i] - o[i]) / d[i], (hi[i] - o[i]) / d[i]);
            t0 = t0.max(a.min(b));
            t1 = t1.min(a.max(b));
        }
    }
    t1 >= t0 && t1 >= 0.0
}

fn ray_tri(o: DVec3, d: DVec3, a: DVec3, b: DVec3, c: DVec3) -> Option<f64> {
    let (e1, e2) = (b - a, c - a);
    let p = d.cross(e2);
    let det = e1.dot(p);
    if det.abs() < 1e-14 {
        return None;
    }
    let inv = 1.0 / det;
    let s = o - a;
    let u = s.dot(p) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = s.cross(e1);
    let w = d.dot(q) * inv;
    if w < 0.0 || u + w > 1.0 {
        return None;
    }
    let t = e2.dot(q) * inv;
    (t > 0.0).then_some(t)
}
