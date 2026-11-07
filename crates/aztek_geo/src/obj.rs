//! Wavefront OBJ reading and writing.

use std::collections::HashMap;
use std::hash::Hash;

use glam::DVec3;
use rayon::prelude::*;

use crate::attribute::{names, AttrValue, AttributeSet, Index, INVALID};
use crate::colors;
use crate::io_common::{append_number, format_blocks, parse_double, parse_i64};
use crate::mesh::Mesh;
use crate::normals::{corner_normals, CornerNormalOptions};
use crate::primitives::mark_uv_seams;
use crate::topology::face_components;

#[derive(Clone, Debug)]
pub struct ObjReadOptions {
    /// One mesh per "o" statement; otherwise everything goes into one mesh.
    pub split_objects: bool,
    /// Turn smoothing groups into hard edges (when the file has "s" statements).
    pub smoothing_groups_to_hard_edges: bool,
    /// Keep file normals as the corner "normal" attribute.
    pub keep_normals: bool,
}

impl Default for ObjReadOptions {
    fn default() -> Self {
        Self { split_objects: true, smoothing_groups_to_hard_edges: true, keep_normals: true }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ObjObject {
    pub name: String,
    pub mesh: Mesh,
    /// Names for the face "group" int attribute, from "g" statements.
    pub group_names: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub struct ObjReadResult {
    pub objects: Vec<ObjObject>,
    pub mtllibs: Vec<String>,
    pub error: String,
}

impl ObjReadResult {
    pub fn ok(&self) -> bool {
        self.error.is_empty()
    }
}

#[derive(Clone, Debug)]
pub struct ObjWriteOptions {
    pub normals: bool,
    pub uvs: bool,
    /// Write "s" statements derived from hard edges.
    pub smoothing_groups: bool,
    /// Significant digits; 0 writes the shortest text that reads back exactly.
    pub precision: i32,
}

impl Default for ObjWriteOptions {
    fn default() -> Self {
        Self { normals: true, uvs: true, smoothing_groups: true, precision: 0 }
    }
}

const NO_REF: i64 = i64::MIN;

#[derive(Clone, Copy, PartialEq, Eq)]
enum EventKind {
    Object,
    Group,
    Material,
    Smooth,
}

struct Event {
    kind: EventKind,
    /// Applies to faces from this one on.
    face_index: usize,
    name: String,
    value: i32,
}

#[derive(Default)]
struct Chunk {
    v: Vec<DVec3>,
    v_color: Vec<[f32; 4]>,
    any_color: bool,
    vt: Vec<[f32; 2]>,
    vn: Vec<[f32; 3]>,
    face_size: Vec<u32>,
    /// 3 per corner: v, vt, vn.
    refs: Vec<i64>,
    events: Vec<Event>,
    mtllibs: Vec<String>,
    lines: usize,
}

struct Cursor<'a> {
    s: &'a [u8],
    p: usize,
}

impl<'a> Cursor<'a> {
    fn at(&self) -> Option<u8> {
        self.s.get(self.p).copied()
    }
    fn skip_space(&mut self) {
        while matches!(self.at(), Some(b' ' | b'\t')) {
            self.p += 1;
        }
    }
    fn at_line_end(&self) -> bool {
        matches!(self.at(), None | Some(b'\n' | b'\r' | b'#'))
    }
    fn next_line(&mut self) {
        while self.p < self.s.len() && self.s[self.p] != b'\n' {
            self.p += 1;
        }
        if self.p < self.s.len() {
            self.p += 1;
        }
    }
    fn word(&mut self) -> &'a [u8] {
        self.skip_space();
        let st = self.p;
        while !matches!(self.at(), None | Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.p += 1;
        }
        &self.s[st..self.p]
    }
    fn rest(&mut self) -> String {
        self.skip_space();
        let st = self.p;
        while !matches!(self.at(), None | Some(b'\n' | b'\r' | b'#')) {
            self.p += 1;
        }
        let mut e = self.p;
        while e > st && matches!(self.s[e - 1], b' ' | b'\t') {
            e -= 1;
        }
        String::from_utf8_lossy(&self.s[st..e]).into_owned()
    }
    fn number(&mut self) -> Option<f64> {
        self.skip_space();
        if self.at() == Some(b'+') {
            self.p += 1;
        }
        let (v, n) = parse_double(&self.s[self.p..])?;
        self.p += n;
        Some(v)
    }
    fn integer(&mut self) -> Option<i64> {
        let (v, n) = parse_i64(&self.s[self.p..])?;
        self.p += n;
        Some(v)
    }
}

/// OBJ index to 0-based global, or a negative value encoding an index local to the chunk.
fn encode_ref(raw: i64, local_count: usize) -> i64 {
    if raw > 0 {
        raw - 1
    } else if raw < 0 {
        -(local_count as i64 + raw) - 1
    } else {
        NO_REF
    }
}

fn parse_chunk(text: &[u8]) -> Chunk {
    let mut c = Chunk::default();
    let mut cur = Cursor { s: text, p: 0 };
    while cur.p < text.len() {
        cur.skip_space();
        if cur.at_line_end() {
            cur.next_line();
            continue;
        }
        match cur.word() {
            b"v" => {
                let x = cur.number().unwrap_or(0.0);
                let y = cur.number().unwrap_or(0.0);
                let z = cur.number().unwrap_or(0.0);
                c.v.push(DVec3::new(x, y, z));
                if let (Some(r), Some(g), Some(b)) = (cur.number(), cur.number(), cur.number()) {
                    c.v_color.resize(c.v.len(), [1.0; 4]);
                    *c.v_color.last_mut().unwrap() = [r as f32, g as f32, b as f32, 1.0];
                    c.any_color = true;
                }
            }
            b"vt" => {
                let u = cur.number().unwrap_or(0.0);
                let v = cur.number().unwrap_or(0.0);
                c.vt.push([u as f32, v as f32]);
            }
            b"vn" => {
                let x = cur.number().unwrap_or(0.0);
                let y = cur.number().unwrap_or(0.0);
                let z = cur.number().unwrap_or(0.0);
                c.vn.push([x as f32, y as f32, z as f32]);
            }
            b"f" => {
                let mut n = 0u32;
                loop {
                    cur.skip_space();
                    if cur.at_line_end() {
                        break;
                    }
                    let Some(vi) = cur.integer() else {
                        cur.word();
                        continue;
                    };
                    let (mut ti, mut ni) = (0, 0);
                    if cur.at() == Some(b'/') {
                        cur.p += 1;
                        if cur.at().is_some_and(|ch| ch != b'/') {
                            ti = cur.integer().unwrap_or(0);
                        }
                        if cur.at() == Some(b'/') {
                            cur.p += 1;
                            ni = cur.integer().unwrap_or(0);
                        }
                    }
                    c.refs.push(encode_ref(vi, c.v.len()));
                    c.refs.push(encode_ref(ti, c.vt.len()));
                    c.refs.push(encode_ref(ni, c.vn.len()));
                    n += 1;
                }
                c.face_size.push(n);
            }
            b"o" => c.events.push(Event { kind: EventKind::Object, face_index: c.face_size.len(), name: cur.rest(), value: 0 }),
            b"g" => c.events.push(Event { kind: EventKind::Group, face_index: c.face_size.len(), name: cur.rest(), value: 0 }),
            b"usemtl" => {
                c.events.push(Event { kind: EventKind::Material, face_index: c.face_size.len(), name: cur.rest(), value: 0 })
            }
            b"s" => {
                let s = cur.rest();
                let value = if s != "off" { parse_i64(s.as_bytes()).map(|(v, _)| v as i32).unwrap_or(0) } else { 0 };
                c.events.push(Event { kind: EventKind::Smooth, face_index: c.face_size.len(), name: String::new(), value });
            }
            b"mtllib" => {
                let r = cur.rest();
                c.mtllibs.push(r);
            }
            b"l" => c.lines += 1,
            _ => {}
        }
        cur.next_line();
    }
    c
}

/// Splits text at line starts into roughly equal pieces.
fn split_lines(text: &[u8], pieces: usize) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut p = 0;
    let step = (text.len() / pieces).max(1);
    while p < text.len() {
        let mut q = text.len().min(p + step);
        while q < text.len() && text[q - 1] != b'\n' {
            q += 1;
        }
        out.push((p, q));
        p = q;
    }
    out
}

/// A layer holding exactly values, one per element of the set.
fn fill_layer<T: AttrValue + Send + Sync>(set: &mut AttributeSet, name: &str, values: &[T]) {
    let dst = set.add(name, T::TYPE).values_mut::<T>();
    let n = dst.len().min(values.len());
    dst[..n].par_iter_mut().zip(values[..n].par_iter()).for_each(|(d, s)| *d = *s);
}

#[derive(Default)]
struct ObjectBuilder {
    obj: ObjObject,
    face_size: Vec<u32>,
    /// 3 per corner, resolved global 0-based, -1 when absent.
    refs: Vec<i64>,
    material: Vec<i32>,
    group: Vec<i32>,
    smooth: Vec<i32>,
}

/// Smoothing group per face from hard edges: faces joined by soft edges share a group, faces with only hard edges
/// get 0. Groups are numbered from 1.
pub fn smoothing_groups_from_hard_edges(mesh: &Mesh) -> Vec<i32> {
    let hard = mesh.edge_attrs().get::<bool>(names::HARD);
    let block: Vec<u8> = (0..mesh.edge_capacity())
        .map(|e| (!mesh.edge_valid(e) || !mesh.is_manifold_edge(e) || (!hard.is_empty() && hard[e as usize])) as u8)
        .collect();
    let mut comp = Vec::new();
    face_components(mesh, &mut comp, &block);
    let mut groups = vec![0i32; mesh.face_capacity() as usize];
    let mut id: Vec<i32> = Vec::new();
    let mut next = 1;
    for f in 0..mesh.face_capacity() {
        if !mesh.face_valid(f) {
            continue;
        }
        let mut any_soft = false;
        mesh.for_each_face_corner(f, |c| any_soft |= block[mesh.corner_edge(c) as usize] == 0);
        if !any_soft {
            continue;
        }
        let k = comp[f as usize] as usize;
        if k >= id.len() {
            id.resize(k + 1, 0);
        }
        if id[k] == 0 {
            id[k] = next;
            next += 1;
        }
        groups[f as usize] = id[k];
    }
    groups
}

/// Sets "hard" on edges whose faces have different smoothing groups, or group 0.
pub fn hard_edges_from_smoothing_groups(mesh: &mut Mesh, groups: &[i32]) {
    let vals: Vec<Option<bool>> = (0..mesh.edge_capacity())
        .into_par_iter()
        .map(|e| {
            if !mesh.edge_valid(e) || !mesh.is_manifold_edge(e) {
                return None;
            }
            let c = mesh.edge_corner(e);
            let a = groups[mesh.corner_face(c) as usize];
            let b = groups[mesh.corner_face(mesh.corner_radial(c)) as usize];
            Some(a == 0 || b == 0 || a != b)
        })
        .collect();
    let hard = mesh.edge_attrs_mut().get_or_add::<bool>(names::HARD);
    for (h, v) in hard.iter_mut().zip(vals) {
        if let Some(v) = v {
            *h = v;
        }
    }
}

fn name_id(names: &mut Vec<String>, name: &str) -> i32 {
    if let Some(i) = names.iter().position(|n| n == name) {
        return i as i32;
    }
    names.push(name.to_string());
    (names.len() - 1) as i32
}

/// Parses in parallel chunks. Faces referencing missing data are skipped.
pub fn read_obj(text: &[u8], opts: &ObjReadOptions) -> ObjReadResult {
    let mut result = ObjReadResult::default();
    let pieces = if text.len() < (1 << 20) {
        1
    } else {
        (text.len() >> 20).min(4 * rayon::current_num_threads())
    };
    let ranges = split_lines(text, pieces);
    let mut chunks: Vec<Chunk> = ranges.par_iter().map(|&(a, b)| parse_chunk(&text[a..b])).collect();

    let nc = chunks.len();
    let mut v_base = vec![0usize; nc + 1];
    let mut t_base = vec![0usize; nc + 1];
    let mut n_base = vec![0usize; nc + 1];
    let mut any_color = false;
    let mut any_smooth_statement = false;
    let mut lines = 0;
    for (i, ch) in chunks.iter().enumerate() {
        lines += ch.lines;
        v_base[i + 1] = v_base[i] + ch.v.len();
        t_base[i + 1] = t_base[i] + ch.vt.len();
        n_base[i + 1] = n_base[i] + ch.vn.len();
        any_color |= ch.any_color;
        any_smooth_statement |= ch.events.iter().any(|e| e.kind == EventKind::Smooth);
        result.mtllibs.extend(ch.mtllibs.iter().cloned());
    }
    let mut v = Vec::with_capacity(v_base[nc]);
    let mut vt = Vec::with_capacity(t_base[nc]);
    let mut vn = Vec::with_capacity(n_base[nc]);
    let mut v_color = Vec::new();
    for ch in &chunks {
        v.extend_from_slice(&ch.v);
        vt.extend_from_slice(&ch.vt);
        vn.extend_from_slice(&ch.vn);
        if any_color {
            let mut col = ch.v_color.clone();
            col.resize(ch.v.len(), [1.0; 4]);
            v_color.extend_from_slice(&col);
        }
    }

    // References made global in parallel: a chunk's relative indices depend only on the counts before it. A face
    // with a missing point is dropped below; missing UVs or normals stay absent.
    let face_bad: Vec<Vec<bool>> = chunks
        .par_iter_mut()
        .enumerate()
        .map(|(ci, ch)| {
            let resolve = |r: i64, base: &[usize], count: usize| -> i64 {
                if r == NO_REF {
                    return -1;
                }
                let g = if r >= 0 { r } else { base[ci] as i64 + (-r - 1) };
                if g >= 0 && (g as usize) < count {
                    g
                } else {
                    -2
                }
            };
            let mut bad_out = vec![false; ch.face_size.len()];
            let mut at = 0;
            for (f, bad_f) in bad_out.iter_mut().enumerate() {
                let n = ch.face_size[f] as usize;
                let refs = &mut ch.refs[at * 3..(at + n) * 3];
                at += n;
                let mut bad = n < 3;
                for k in 0..n {
                    refs[k * 3] = resolve(refs[k * 3], &v_base, v.len());
                    refs[k * 3 + 1] = resolve(refs[k * 3 + 1], &t_base, vt.len());
                    refs[k * 3 + 2] = resolve(refs[k * 3 + 2], &n_base, vn.len());
                    bad = bad || refs[k * 3] < 0;
                }
                *bad_f = bad;
            }
            bad_out
        })
        .collect();

    // Sorting faces into objects follows the state statements in file order.
    let mut builders: Vec<ObjectBuilder> = Vec::new();
    let mut material_name = String::new();
    let mut group_name = String::new();
    let mut smooth = 0;
    let (mut material, mut group) = (-1i32, -1i32);
    let begin_object = |builders: &mut Vec<ObjectBuilder>, name: &str, material: &mut i32, group: &mut i32| {
        let mut b = ObjectBuilder::default();
        b.obj.name = name.to_string();
        builders.push(b);
        *material = -1;
        *group = -1;
    };
    for (ci, ch) in chunks.iter().enumerate() {
        let mut ev = 0;
        let mut at = 0;
        for f in 0..=ch.face_size.len() {
            while ev < ch.events.len() && ch.events[ev].face_index == f {
                let e = &ch.events[ev];
                match e.kind {
                    EventKind::Object => {
                        if opts.split_objects {
                            if builders.last().is_some_and(|b| b.face_size.is_empty()) {
                                builders.last_mut().unwrap().obj.name = e.name.clone();
                            } else {
                                begin_object(&mut builders, &e.name, &mut material, &mut group);
                            }
                        }
                    }
                    EventKind::Group => {
                        group_name = e.name.clone();
                        group = -1;
                    }
                    EventKind::Material => {
                        material_name = e.name.clone();
                        material = -1;
                    }
                    EventKind::Smooth => smooth = e.value,
                }
                ev += 1;
            }
            if f == ch.face_size.len() {
                break;
            }
            if builders.is_empty() {
                begin_object(&mut builders, "default", &mut material, &mut group);
            }
            let n = ch.face_size[f] as usize;
            let refs = &ch.refs[at * 3..(at + n) * 3];
            at += n;
            if face_bad[ci][f] {
                continue;
            }
            let b = builders.last_mut().unwrap();
            if material < 0 && !material_name.is_empty() {
                material = name_id(b.obj.mesh.material_names_mut(), &material_name);
            }
            if group < 0 && !group_name.is_empty() {
                group = name_id(&mut b.obj.group_names, &group_name);
            }
            b.refs.extend_from_slice(refs);
            b.face_size.push(n as u32);
            b.material.push(material.max(0));
            b.group.push(group.max(0));
            b.smooth.push(smooth);
        }
    }
    drop(chunks);

    let mut point_of = vec![INVALID; v.len()];
    let builder_count = builders.len();
    for mut b in builders {
        if b.face_size.is_empty() && builder_count > 1 {
            continue;
        }

        // Points keep the file's vertex order: flag the referenced ones, then walk the flags in order.
        let mut referenced = vec![false; v.len()];
        for i in 0..b.refs.len() / 3 {
            referenced[b.refs[i * 3] as usize] = true;
        }
        let used: Vec<usize> = (0..v.len()).filter(|&g| referenced[g]).collect();
        let corners = b.refs.len() / 3;
        let positions: Vec<DVec3> = used.iter().map(|&g| v[g]).collect();
        for (i, &g) in used.iter().enumerate() {
            point_of[g] = i as Index;
        }
        let pts: Vec<Index> = (0..corners).into_par_iter().map(|i| point_of[b.refs[i * 3] as usize]).collect();
        let mat_names = std::mem::take(b.obj.mesh.material_names_mut());
        let mut m = Mesh::from_polygons(&positions, &b.face_size, &pts);
        // Every face made it, so faces and corners line up with the file's; otherwise build again face by face.
        let all_faces = m.face_capacity() as usize == b.face_size.len();
        if !all_faces {
            m = Mesh::new();
            m.reserve(used.len(), corners, b.face_size.len(), corners);
            for &pos in &positions {
                m.add_point(pos);
            }
        }
        *m.material_names_mut() = mat_names;
        if any_color {
            let col = m.point_attrs_mut().get_or_add::<[f32; 4]>(names::COLOR);
            for &g in &used {
                col[point_of[g] as usize] = v_color[g];
            }
        }

        let mut uv: Vec<[f32; 2]> = Vec::with_capacity(corners);
        let mut normal: Vec<[f32; 3]> = Vec::with_capacity(corners);
        let mut face_mat = Vec::new();
        let mut face_group = Vec::new();
        let mut face_smooth = Vec::new();
        let (mut any_uv, mut any_normal) = (false, false);
        if all_faces {
            face_mat = std::mem::take(&mut b.material);
            face_group = std::mem::take(&mut b.group);
            face_smooth = std::mem::take(&mut b.smooth);
            uv = (0..corners)
                .into_par_iter()
                .map(|i| {
                    let ti = b.refs[i * 3 + 1];
                    if ti >= 0 {
                        vt[ti as usize]
                    } else {
                        [0.0; 2]
                    }
                })
                .collect();
            normal = (0..corners)
                .into_par_iter()
                .map(|i| {
                    let ni = b.refs[i * 3 + 2];
                    if ni >= 0 {
                        vn[ni as usize]
                    } else {
                        [0.0; 3]
                    }
                })
                .collect();
            any_uv = (0..corners).into_par_iter().any(|i| b.refs[i * 3 + 1] >= 0);
            any_normal = (0..corners).into_par_iter().any(|i| b.refs[i * 3 + 2] >= 0);
        } else {
            let mut at = 0;
            let mut fp = Vec::new();
            for f in 0..b.face_size.len() {
                let n = b.face_size[f] as usize;
                let refs = &b.refs[at * 3..(at + n) * 3];
                at += n;
                fp.clear();
                fp.extend((0..n).map(|k| point_of[refs[k * 3] as usize]));
                if m.add_face(&fp) == INVALID {
                    continue;
                }
                face_mat.push(b.material[f]);
                face_group.push(b.group[f]);
                face_smooth.push(b.smooth[f]);
                for k in 0..n {
                    let (ti, ni) = (refs[k * 3 + 1], refs[k * 3 + 2]);
                    uv.push(if ti >= 0 { vt[ti as usize] } else { [0.0; 2] });
                    normal.push(if ni >= 0 { vn[ni as usize] } else { [0.0; 3] });
                    any_uv |= ti >= 0;
                    any_normal |= ni >= 0;
                }
            }
        }
        for &g in &used {
            point_of[g] = INVALID;
        }
        // Points only used by rejected faces.
        m.remove_all_loose();
        if m.has_deleted() {
            m.compact(None);
        }

        // Faces and corners were added in order, so ids match the vectors.
        if !m.material_names().is_empty() {
            fill_layer(m.face_attrs_mut(), names::MATERIAL, &face_mat);
        }
        if !b.obj.group_names.is_empty() {
            fill_layer(m.face_attrs_mut(), "group", &face_group);
        }
        if any_uv {
            fill_layer(m.corner_attrs_mut(), &names::uv(0), &uv);
        }
        if any_normal && opts.keep_normals {
            fill_layer(m.corner_attrs_mut(), names::NORMAL, &normal);
        }

        if any_smooth_statement {
            fill_layer(m.face_attrs_mut(), names::SMOOTH_GROUP, &face_smooth);
            if opts.smoothing_groups_to_hard_edges {
                hard_edges_from_smoothing_groups(&mut m, &face_smooth);
            }
        } else if any_normal {
            // Without smoothing groups, edges whose file normals disagree are hard.
            let mm = &m;
            let vals: Vec<Option<bool>> = (0..mm.edge_capacity())
                .into_par_iter()
                .map(|e| {
                    if !mm.is_manifold_edge(e) {
                        return None;
                    }
                    let c = mm.edge_corner(e);
                    let r = mm.corner_radial(c);
                    let same = mm.corner_point(r) == mm.corner_point(c);
                    let ra = if same { r } else { mm.corner_next(r) };
                    let rb = if same { mm.corner_next(r) } else { r };
                    let close = |x: Index, y: Index| {
                        let (a, b) = (normal[x as usize], normal[y as usize]);
                        a[0] * b[0] + a[1] * b[1] + a[2] * b[2] > 0.9999
                    };
                    Some(!close(c, ra) || !close(mm.corner_next(c), rb))
                })
                .collect();
            let hard = m.edge_attrs_mut().get_or_add::<bool>(names::HARD);
            for (h, v) in hard.iter_mut().zip(vals) {
                if let Some(v) = v {
                    *h = v;
                }
            }
        }
        if any_uv {
            mark_uv_seams(&mut m, &names::uv(0));
        }
        if any_color {
            colors::adopt(&mut m);
        }
        b.obj.mesh = m;
        result.objects.push(b.obj);
    }
    if result.objects.is_empty() && lines > 0 {
        result.error = format!("{lines} line elements (l) and no faces; OBJ lines are not imported");
    }
    result
}

pub fn read_obj_file(path: &str, opts: &ObjReadOptions) -> ObjReadResult {
    match std::fs::read(path) {
        Ok(text) => read_obj(&text, opts),
        Err(_) => ObjReadResult { error: format!("cannot open {path}"), ..Default::default() },
    }
}

/// Bit-pattern key for pooling float values exactly.
trait Bits: Copy {
    type Key: Hash + Eq;
    fn bits(&self) -> Self::Key;
}
impl Bits for [f32; 2] {
    type Key = [u32; 2];
    fn bits(&self) -> [u32; 2] {
        self.map(f32::to_bits)
    }
}
impl Bits for [f32; 3] {
    type Key = [u32; 3];
    fn bits(&self) -> [u32; 3] {
        self.map(f32::to_bits)
    }
}

/// Pools unique values, returning 0-based ids per corner.
fn pool_values<T: Bits>(m: &Mesh, values: &[T], pool: &mut Vec<T>) -> Vec<usize> {
    let mut id = vec![0usize; m.corner_capacity() as usize];
    let mut map: HashMap<T::Key, usize> = HashMap::with_capacity(m.num_corners() as usize);
    for f in 0..m.face_capacity() {
        if !m.face_valid(f) {
            continue;
        }
        m.for_each_face_corner(f, |c| {
            let v = values[c as usize];
            let next = pool.len();
            let i = *map.entry(v.bits()).or_insert_with(|| {
                pool.push(v);
                next
            });
            id[c as usize] = i;
        });
    }
    id
}

fn write_meshes(objects: &[(&str, &Mesh)], opts: &ObjWriteOptions) -> String {
    let mut out = String::from("# Aztek\n");
    let (mut v_offset, mut t_offset, mut n_offset) = (0usize, 0usize, 0usize);
    let prec = opts.precision;
    for &(name, m) in objects {
        out += "o ";
        out += if name.is_empty() { "mesh" } else { name };
        out += "\n";

        let mut point_id = vec![0usize; m.point_capacity() as usize];
        let mut live = Vec::with_capacity(m.num_points() as usize);
        for p in 0..m.point_capacity() {
            if m.point_valid(p) {
                point_id[p as usize] = live.len();
                live.push(p);
            }
        }
        // The vertex color extension holds one color per point: the current set, averaged over each point's corners.
        let set = colors::current(m);
        let color = if set.is_empty() { Vec::new() } else { colors::point_colors(m, &set) };
        out += &format_blocks(live.len(), |s, i| {
            let p = m.position(live[i]);
            s.push_str("v ");
            append_number(s, p.x, prec);
            s.push(' ');
            append_number(s, p.y, prec);
            s.push(' ');
            append_number(s, p.z, prec);
            if !color.is_empty() {
                for k in 0..3 {
                    s.push(' ');
                    append_number(s, color[live[i] as usize][k], prec);
                }
            }
            s.push('\n');
        });

        let uv = m.corner_attrs().get::<[f32; 2]>(&names::uv(0));
        let mut uv_pool = Vec::new();
        let mut uv_id = Vec::new();
        if opts.uvs && !uv.is_empty() {
            uv_id = pool_values(m, uv, &mut uv_pool);
            out += &format_blocks(uv_pool.len(), |s, i| {
                s.push_str("vt ");
                append_number(s, uv_pool[i][0], prec);
                s.push(' ');
                append_number(s, uv_pool[i][1], prec);
                s.push('\n');
            });
        }

        let mut n_pool = Vec::new();
        let mut n_id = Vec::new();
        if opts.normals {
            let stored = m.corner_attrs().get::<[f32; 3]>(names::NORMAL);
            let computed: Vec<[f32; 3]>;
            let nrm: &[[f32; 3]] = if stored.len() == m.corner_capacity() as usize {
                stored
            } else {
                computed = corner_normals(m, &CornerNormalOptions::default()).iter().map(|n| n.to_array()).collect();
                &computed
            };
            n_id = pool_values(m, nrm, &mut n_pool);
            out += &format_blocks(n_pool.len(), |s, i| {
                s.push_str("vn ");
                append_number(s, n_pool[i][0], prec);
                s.push(' ');
                append_number(s, n_pool[i][1], prec);
                s.push(' ');
                append_number(s, n_pool[i][2], prec);
                s.push('\n');
            });
        }

        let smooth = if opts.smoothing_groups { smoothing_groups_from_hard_edges(m) } else { Vec::new() };
        let mat = m.face_attrs().get::<i32>(names::MATERIAL);
        let mat_names = m.material_names();
        let faces: Vec<Index> = (0..m.face_capacity()).filter(|&f| m.face_valid(f)).collect();

        // State statements are emitted where they change, so blocks start from the previous face's state.
        out += &format_blocks(faces.len(), |s, i| {
            let f = faces[i] as usize;
            let prev = if i > 0 { Some(faces[i - 1] as usize) } else { None };
            if !mat.is_empty() && !mat_names.is_empty() && prev.is_none_or(|p| mat[p] != mat[f]) {
                let id = mat[f];
                s.push_str("usemtl ");
                if id >= 0 && (id as usize) < mat_names.len() {
                    s.push_str(&mat_names[id as usize]);
                } else {
                    s.push_str(&format!("material{id}"));
                }
                s.push('\n');
            }
            if !smooth.is_empty() && prev.is_none_or(|p| smooth[p] != smooth[f]) {
                s.push_str("s ");
                if smooth[f] != 0 {
                    s.push_str(&smooth[f].to_string());
                } else {
                    s.push_str("off");
                }
                s.push('\n');
            }
            s.push('f');
            m.for_each_face_corner(f as Index, |c| {
                s.push(' ');
                s.push_str(&(v_offset + point_id[m.corner_point(c) as usize] + 1).to_string());
                if !uv_id.is_empty() || !n_id.is_empty() {
                    s.push('/');
                    if !uv_id.is_empty() {
                        s.push_str(&(t_offset + uv_id[c as usize] + 1).to_string());
                    }
                    if !n_id.is_empty() {
                        s.push('/');
                        s.push_str(&(n_offset + n_id[c as usize] + 1).to_string());
                    }
                }
            });
            s.push('\n');
        });
        v_offset += live.len();
        t_offset += uv_pool.len();
        n_offset += n_pool.len();
    }
    out
}

/// Writes one or more meshes, each as an "o" object, sharing one vertex pool.
pub fn write_obj_objects(objects: &[ObjObject], opts: &ObjWriteOptions) -> String {
    let named: Vec<(&str, &Mesh)> = objects.iter().map(|o| (o.name.as_str(), &o.mesh)).collect();
    write_meshes(&named, opts)
}

pub fn write_obj(mesh: &Mesh, opts: &ObjWriteOptions) -> String {
    write_meshes(&[("mesh", mesh)], opts)
}

pub fn write_obj_file(path: &str, mesh: &Mesh, opts: &ObjWriteOptions) -> bool {
    std::fs::write(path, write_obj(mesh, opts)).is_ok()
}
