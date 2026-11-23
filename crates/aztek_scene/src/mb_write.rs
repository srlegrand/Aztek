//! Writing binary scene files (`.mb`): the scene's `.ma` text, written in internal units (centimetres, radians),
//! turned into the IFF chunks `mb_file` reads. The layout follows the reference's own files (see `mb_file`): an
//! `FOR8` `Maya` root, a `HEAD` group, one group per node whose first child (`CREA`) names it, `SLCT` groups for
//! nodes the file didn't make and a `CONS` list of connections.
//!
//! What the text holds that the binary layout here has no chunk for is left out, with a warning: added compound
//! attributes, some data types (nurbsSurface, lattice, vectorArray and plug-in data), node locks and nested
//! references.

use std::collections::{BTreeMap, HashSet};

use crate::ma_file::{Lexer, Tok};

/// Ticks in a second, as the reference counts time.
const TICKS: f64 = 141_120_000.0;

/// What the text alone doesn't say.
pub struct Hints {
    /// The scene's own units, as the header names them ("cm", "deg", "film").
    pub linear: String,
    pub angle: String,
    pub time: String,
    pub fps: f64,
    /// DAG nodes with no parent, by name.
    pub dag_roots: HashSet<String>,
    /// Node types that are transforms.
    pub transform_types: HashSet<String>,
}

enum Item {
    Leaf([u8; 4], Vec<u8>),
    Group([u8; 4], [u8; 4], Vec<Item>),
}

fn emit(out: &mut Vec<u8>, item: &Item) {
    match item {
        Item::Leaf(tag, data) => {
            out.extend(tag);
            out.extend([0u8; 4]);
            out.extend((data.len() as u64).to_be_bytes());
            out.extend(data);
            out.extend(std::iter::repeat(0u8).take((8 - data.len() % 8) % 8));
        }
        Item::Group(tag, form, kids) => {
            let mut body = form.to_vec();
            for k in kids {
                emit(&mut body, k);
            }
            out.extend(tag);
            out.extend([0u8; 4]);
            out.extend((body.len() as u64).to_be_bytes());
            out.extend(&body);
            out.extend(std::iter::repeat(0u8).take((4 - body.len() % 4) % 4));
        }
    }
}

/// Big-endian data for one chunk.
#[derive(Default)]
struct Data(Vec<u8>);

impl Data {
    fn str(&mut self, s: &str) -> &mut Self {
        self.0.extend(s.as_bytes());
        self.0.push(0);
        self
    }
    fn u8(&mut self, v: u8) -> &mut Self {
        self.0.push(v);
        self
    }
    fn u32(&mut self, v: u32) -> &mut Self {
        self.0.extend(v.to_be_bytes());
        self
    }
    fn f32(&mut self, v: f32) -> &mut Self {
        self.0.extend(v.to_be_bytes());
        self
    }
    fn f64(&mut self, v: f64) -> &mut Self {
        self.0.extend(v.to_be_bytes());
        self
    }
    fn raw(&mut self, b: &[u8]) -> &mut Self {
        self.0.extend(b);
        self
    }
}

/// A mesh's geometry gathered from its `.vt`, `.ed`, `.n`, `.uvst`, `.clst` and `.fc` lines, for one MESH chunk.
#[derive(Default)]
struct MeshAcc {
    points: Vec<[f32; 3]>,
    edges: Vec<(u32, u32, bool)>,
    normals: Vec<f32>,
    uv_names: BTreeMap<usize, String>,
    uvs: BTreeMap<usize, Vec<[f32; 2]>>,
    colour_names: BTreeMap<usize, String>,
    colours: BTreeMap<usize, Vec<[f32; 4]>>,
    /// Each face's loops, each loop its edges and per set its uv and colour ids.
    faces: Vec<Vec<Loop>>,
    any: bool,
}

#[derive(Default)]
struct Loop {
    edges: Vec<i64>,
    uv: BTreeMap<usize, Vec<i64>>,
    colour: BTreeMap<usize, Vec<i64>>,
}

fn put<T: Default + Clone>(v: &mut Vec<T>, at: usize, x: T) {
    if v.len() <= at {
        v.resize(at + 1, T::default());
    }
    v[at] = x;
}

impl MeshAcc {
    fn chunk(&self) -> Vec<u8> {
        let mut d = Data::default();
        d.str("ci").u8(0x20);
        d.u32((self.points.len() * 3) as u32);
        for p in &self.points {
            p.iter().for_each(|x| {
                d.f32(*x);
            });
        }
        d.u32((self.edges.len() * 2) as u32);
        for (a, b, hard) in &self.edges {
            d.u32(a | if *hard { 0x8000_0000 } else { 0 }).u32(*b);
        }
        let words: usize = self.faces.iter().flat_map(|f| f.iter().map(|l| l.edges.len())).sum();
        d.u32(words as u32);
        for f in &self.faces {
            for (li, l) in f.iter().enumerate() {
                for (k, e) in l.edges.iter().enumerate() {
                    let mut w = if *e < 0 { (!*e as u32) | 0x8000_0000 } else { *e as u32 };
                    if k + 1 == l.edges.len() {
                        w |= 0x4000_0000;
                        if li + 1 == f.len() {
                            w |= 0x2000_0000;
                        }
                    }
                    d.u32(w);
                }
            }
        }
        d.u32(self.normals.len() as u32);
        self.normals.iter().for_each(|x| {
            d.f32(*x);
        });
        if !self.normals.is_empty() {
            d.u32(0);
        }
        // Per face-vertex ids, every loop in face order; -1 where a loop has none in that set.
        let ids = |set: usize, colour: bool| -> Vec<i64> {
            let mut out = Vec::new();
            for l in self.faces.iter().flatten() {
                let m = if colour { &l.colour } else { &l.uv };
                match m.get(&set).filter(|v| v.len() == l.edges.len()) {
                    Some(v) => out.extend(v),
                    None => out.extend(std::iter::repeat(-1).take(l.edges.len())),
                }
            }
            out
        };
        let uv_sets: Vec<usize> = self.uv_names.keys().chain(self.uvs.keys()).copied().collect::<std::collections::BTreeSet<_>>().into_iter().collect();
        d.u32(uv_sets.len() as u32);
        if !uv_sets.is_empty() {
            d.u32(0);
            for s in &uv_sets {
                d.str(self.uv_names.get(s).map_or("map1", String::as_str));
                let uvs = self.uvs.get(s).map(Vec::as_slice).unwrap_or(&[]);
                d.u32((uvs.len() * 2) as u32);
                uvs.iter().for_each(|uv| {
                    d.f32(uv[0]).f32(uv[1]);
                });
                let idx = ids(*s, false);
                d.u32(idx.len() as u32);
                idx.iter().for_each(|i| {
                    d.u32(*i as i32 as u32);
                });
            }
        }
        let colour_sets: Vec<usize> = self.colour_names.keys().chain(self.colours.keys()).copied().collect::<std::collections::BTreeSet<_>>().into_iter().collect();
        d.u32(colour_sets.len() as u32);
        if !colour_sets.is_empty() {
            d.u32(0);
            for s in &colour_sets {
                d.str(self.colour_names.get(s).map_or("colorSet1", String::as_str)).u32(0);
                let c = self.colours.get(s).map(Vec::as_slice).unwrap_or(&[]);
                d.u32((c.len() * 4) as u32);
                c.iter().for_each(|x| {
                    x.iter().for_each(|v| {
                        d.f32(*v);
                    });
                });
                let idx = ids(*s, true);
                d.u32(idx.len() as u32);
                idx.iter().for_each(|i| {
                    d.u32(*i as i32 as u32);
                });
            }
        }
        // The reference's mesh data ends with a 16-byte geometry-tag header, then a gtag count.
        // Keep the empty table so Maya can read meshes with baked construction history.
        d.raw(&[0; 16]).str("gtag").u32(0);
        d.0
    }
}

/// One node being written: its group's children, and its mesh when it is one.
struct NodeOut {
    form: [u8; 4],
    ty: String,
    kids: Vec<Item>,
    mesh: Option<MeshAcc>,
    select: bool,
}

#[derive(Default)]
struct Writer {
    version: String,
    file_info: Vec<(String, String)>,
    plugins: Vec<(String, String, String)>,
    refs: Vec<Item>,
    ref_states: Vec<Item>,
    body: Vec<Item>,
    conns: Vec<Item>,
    current: Option<NodeOut>,
    made_ids: BTreeMap<String, [u8; 4]>,
    warnings: BTreeMap<String, usize>,
    uid_seed: u64,
}

/// The text of a setAttr: its flags, plug and values.
#[derive(Default)]
struct Set {
    plug: String,
    size: Option<u32>,
    lock: bool,
    keyable_off: bool,
    ty: Option<String>,
    values: Vec<Tok>,
}

fn parse_set(st: &[Tok]) -> Option<Set> {
    let mut s = Set::default();
    let mut i = 1;
    while i < st.len() {
        let t = &st[i];
        if t.is_flag() {
            let flag = t.text().unwrap_or("");
            let arg = st.get(i + 1);
            match flag {
                "-s" | "-size" | "-ch" | "-capacityHint" => {
                    if flag.starts_with("-s") {
                        s.size = arg.and_then(Tok::num).map(|v| v as u32);
                    }
                    i += 2;
                }
                "-l" | "-lock" => {
                    s.lock = arg.and_then(Tok::num).unwrap_or(1.0) != 0.0;
                    i += 2;
                }
                "-k" | "-keyable" => {
                    s.keyable_off = arg.and_then(Tok::num) == Some(0.0);
                    i += 2;
                }
                "-cb" | "-channelBox" | "-c" | "-clamp" => i += 2,
                "-type" | "-typ" => {
                    s.ty = arg.and_then(Tok::text).map(str::to_string);
                    i += 2;
                }
                _ => i += 1,
            }
            continue;
        }
        if s.plug.is_empty() {
            if let Tok::Str(p) = t {
                s.plug = p.trim_start_matches('.').to_string();
                i += 1;
                continue;
            }
        }
        s.values.push(t.clone());
        i += 1;
    }
    (!s.plug.is_empty()).then_some(s)
}

fn nums(v: &[Tok]) -> Option<Vec<f64>> {
    v.iter().map(Tok::num).collect()
}

/// "wl[0:19].w": a range of elements whose last part is itself a whole multi.
fn sparse_path(attr: &str) -> bool {
    let parts: Vec<&str> = attr.split('.').collect();
    let ranged = |p: &str| p.split_once('[').and_then(|(_, r)| r.strip_suffix(']')).is_some_and(|r| r.contains(':'));
    parts.len() > 1 && parts[..parts.len() - 1].iter().any(|p| ranged(p)) && !parts[parts.len() - 1].contains('[')
}

/// "uvst[2].uvsp[0:9]" → ("uvst", Some(2), "uvsp", start 0).
fn indexed(path: &str) -> Vec<(String, Option<(usize, usize)>)> {
    path.split('.')
        .map(|p| match p.split_once('[') {
            Some((name, rest)) => {
                let r = rest.trim_end_matches(']');
                let (a, b) = r.split_once(':').unwrap_or((r, r));
                (name.to_string(), a.parse().ok().zip(b.parse().ok()))
            }
            None => (p.to_string(), None),
        })
        .collect()
}

/// Time attributes, which a binary file keeps in ticks.
fn is_time(node_type: &str, leaf: &str) -> bool {
    match node_type {
        "time" => matches!(leaf, "o" | "outTime" | "unw" | "unwarpedTime"),
        "animClip" => matches!(leaf, "sf" | "startFrame" | "ss" | "sourceStart" | "se" | "sourceEnd" | "h" | "hold" | "st" | "start" | "du" | "duration"),
        "clipScheduler" => matches!(leaf, "st" | "start" | "ss" | "sourceStart" | "se" | "sourceEnd" | "h" | "hold"),
        "character" => matches!(leaf, "tcs" | "timelineClipStart" | "tce" | "timelineClipEnd"),
        _ => false,
    }
}

fn component_kind(name: &str) -> Option<&'static [u8; 4]> {
    match name {
        "f" => Some(b"CMDF"),
        "e" => Some(b"CMDE"),
        "vtx" => Some(b"CMDV"),
        "map" => Some(b"CMDM"),
        _ => None,
    }
}

fn uid_bytes(s: &str) -> Option<[u8; 16]> {
    let hex: String = s.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if hex.len() != 32 {
        return None;
    }
    let word = |a: usize, b: usize| u32::from_str_radix(&hex[a..b], 16).ok();
    let w0 = word(0, 8)?;
    let w1 = (word(8, 12)? << 16) | word(12, 16)?;
    let w2 = (word(16, 20)? << 16) | word(20, 24)?;
    let w3 = word(24, 32)?;
    let mut out = [0u8; 16];
    for (i, w) in [w0, w1, w2, w3].into_iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&w.to_le_bytes());
    }
    Some(out)
}

impl Writer {
    fn warn(&mut self, what: impl Into<String>) {
        *self.warnings.entry(what.into()).or_default() += 1;
    }

    fn type_id(&mut self, ty: &str) -> [u8; 4] {
        if let Some(id) = crate::mb_file::id_of(ty) {
            return id;
        }
        if let Some(id) = self.made_ids.get(ty) {
            return *id;
        }
        // Types the table has no id for get one of Aztek's own, declared in the header.
        let mut n = self.made_ids.len() as u32;
        loop {
            let id = (0x7A7A_0000u32 + n).to_be_bytes();
            if !crate::mb_file::id_known(id) {
                self.made_ids.insert(ty.to_string(), id);
                return id;
            }
            n += 1;
        }
    }

    fn next_uid(&mut self, name: &str) -> [u8; 16] {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325 ^ self.uid_seed;
        for b in name.bytes() {
            h = (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3);
        }
        self.uid_seed = self.uid_seed.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mix = |mut z: u64| {
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            z ^ (z >> 31)
        };
        let (a, b) = (mix(h), mix(h ^ 0x5555_5555_5555_5555));
        let mut out = [0u8; 16];
        out[..8].copy_from_slice(&a.to_le_bytes());
        out[8..].copy_from_slice(&b.to_le_bytes());
        out
    }

    fn finish_node(&mut self) {
        let Some(mut n) = self.current.take() else { return };
        if let Some(m) = n.mesh.take().filter(|m| m.any) {
            n.kids.push(Item::Leaf(*b"MESH", m.chunk()));
        }
        let tag = if n.select { *b"SLCT" } else { n.form };
        self.body.push(Item::Group(*b"FOR8", tag, n.kids));
    }

    fn create(&mut self, st: &[Tok], h: &Hints) {
        self.finish_node();
        let Some(ty) = st.get(1).and_then(Tok::text).map(str::to_string) else { return };
        let mut name = String::new();
        let mut parent: Option<String> = None;
        let mut shared = false;
        let mut i = 2;
        while i < st.len() {
            match st[i].text() {
                Some("-n" | "-name") => {
                    name = st.get(i + 1).and_then(Tok::text).unwrap_or("").to_string();
                    i += 2;
                }
                Some("-p" | "-parent") => {
                    parent = st.get(i + 1).and_then(Tok::text).map(str::to_string);
                    i += 2;
                }
                Some("-s" | "-shared") => {
                    shared = true;
                    i += 1;
                }
                _ => i += 1,
            }
        }
        let dag = parent.is_some() || h.dag_roots.contains(&name);
        let mut flags = 0u8;
        if parent.is_none() {
            flags |= 0x80;
        }
        if dag {
            flags |= 0x04;
        }
        if h.transform_types.contains(&ty) {
            flags |= 0x08;
        }
        if shared {
            flags |= 0x01;
        }
        let mut d = Data::default();
        d.u8(flags).str(&name);
        if let Some(p) = &parent {
            d.str(p);
        }
        let uid = self.next_uid(&name);
        d.raw(&uid);
        let form = self.type_id(&ty);
        let mesh = (ty == "mesh").then(MeshAcc::default);
        self.current = Some(NodeOut { form, ty, kids: vec![Item::Leaf(*b"CREA", d.0)], mesh, select: false });
    }

    fn select(&mut self, st: &[Tok]) {
        self.finish_node();
        if !st.iter().any(|t| matches!(t.text(), Some("-ne" | "-noExpand"))) {
            self.warn("select without -ne");
            return;
        }
        let Some(name) = st.iter().skip(1).rev().find(|t| !t.is_flag()).and_then(Tok::text) else { return };
        self.current = Some(NodeOut { form: *b"SLCT", ty: String::new(), kids: vec![Item::Leaf(*b"SLCT", name.as_bytes().to_vec())], mesh: None, select: true });
    }

    fn rename_uid(&mut self, st: &[Tok]) {
        let Some(u) = st.iter().skip(1).rev().find(|t| !t.is_flag()).and_then(Tok::text).and_then(uid_bytes) else { return };
        if let Some(Item::Leaf(_, d)) = self.current.as_mut().and_then(|n| n.kids.first_mut()) {
            if d.len() >= 16 {
                let at = d.len() - 16;
                d[at..].copy_from_slice(&u);
            }
        }
    }

    /// A mesh line taken into the node's MESH chunk; false when it isn't one.
    fn mesh_line(&mut self, s: &Set) -> bool {
        let Some(m) = self.current.as_mut().and_then(|n| n.mesh.as_mut()) else { return false };
        let parts = indexed(&s.plug);
        let start = |k: usize| parts.get(k).and_then(|p| p.1).map_or(0, |r| r.0);
        let f32s = |v: &[Tok]| v.iter().map(|t| t.num().unwrap_or(0.0) as f32).collect::<Vec<f32>>();
        match (parts.len(), parts[0].0.as_str()) {
            (1, "vt") => {
                if !s.values.is_empty() {
                    for (k, p) in f32s(&s.values).chunks_exact(3).enumerate() {
                        put(&mut m.points, start(0) + k, [p[0], p[1], p[2]]);
                    }
                    m.any = true;
                }
                true
            }
            (1, "ed") => {
                if !s.values.is_empty() {
                    let v: Vec<f64> = s.values.iter().map(|t| t.num().unwrap_or(0.0)).collect();
                    for (k, e) in v.chunks_exact(3).enumerate() {
                        put(&mut m.edges, start(0) + k, (e[0] as u32, e[1] as u32, e[2] == 0.0));
                    }
                    m.any = true;
                }
                true
            }
            (1, "n") => {
                if !s.values.is_empty() {
                    let at = start(0) * 3;
                    let v = f32s(&s.values);
                    if m.normals.len() < at + v.len() {
                        m.normals.resize(at + v.len(), 0.0);
                    }
                    m.normals[at..at + v.len()].copy_from_slice(&v);
                    m.any = true;
                }
                true
            }
            (2, "uvst") | (2, "clst") => {
                let set = start(0);
                let colour = parts[0].0 == "clst";
                match parts[1].0.as_str() {
                    "uvsn" | "clsn" => {
                        let name = s.values.first().and_then(Tok::text).unwrap_or("").to_string();
                        if colour {
                            m.colour_names.insert(set, name);
                        } else {
                            m.uv_names.insert(set, name);
                        }
                        // Kept as its own chunk too, as the reference writes it.
                        false
                    }
                    "uvsp" if !colour => {
                        if !s.values.is_empty() {
                            let list = m.uvs.entry(set).or_default();
                            for (k, p) in f32s(&s.values).chunks_exact(2).enumerate() {
                                put(list, start(1) + k, [p[0], p[1]]);
                            }
                            m.any = true;
                        }
                        true
                    }
                    "clsp" if colour => {
                        if !s.values.is_empty() {
                            let list = m.colours.entry(set).or_default();
                            for (k, p) in f32s(&s.values).chunks_exact(4).enumerate() {
                                put(list, start(1) + k, [p[0], p[1], p[2], p[3]]);
                            }
                            m.any = true;
                        }
                        true
                    }
                    _ => false,
                }
            }
            (1, "fc") => {
                if s.values.is_empty() {
                    return true;
                }
                let mut faces: Vec<Vec<Loop>> = Vec::new();
                let mut it = s.values.iter().peekable();
                let read_ids = |it: &mut std::iter::Peekable<std::slice::Iter<Tok>>| -> Vec<i64> {
                    let n = it.next().and_then(Tok::num).unwrap_or(0.0) as usize;
                    (0..n).map(|_| it.next().and_then(Tok::num).unwrap_or(-1.0) as i64).collect()
                };
                while let Some(t) = it.next() {
                    match t.text() {
                        Some("f") => {
                            let edges = read_ids(&mut it);
                            faces.push(vec![Loop { edges, ..Default::default() }]);
                        }
                        Some("h") => {
                            let edges = read_ids(&mut it);
                            if let Some(f) = faces.last_mut() {
                                f.push(Loop { edges, ..Default::default() });
                            }
                        }
                        Some(k @ ("mu" | "mc")) => {
                            let set = it.next().and_then(Tok::num).unwrap_or(0.0) as usize;
                            let ids = read_ids(&mut it);
                            if let Some(l) = faces.last_mut().and_then(|f| f.last_mut()) {
                                if k == "mu" {
                                    l.uv.insert(set, ids);
                                } else {
                                    l.colour.insert(set, ids);
                                }
                            }
                        }
                        _ => {}
                    }
                }
                let at = start(0);
                for (k, f) in faces.into_iter().enumerate() {
                    if m.faces.len() <= at + k {
                        m.faces.resize_with(at + k + 1, Vec::new);
                    }
                    m.faces[at + k] = f;
                }
                m.any = true;
                true
            }
            _ => false,
        }
    }

    fn set_attr(&mut self, st: &[Tok], h: &Hints) {
        let Some(s) = parse_set(st) else { return };
        if self.current.is_none() {
            self.warn("setAttr with no node before it");
            return;
        }
        if self.mesh_line(&s) {
            return;
        }
        let node_type = self.current.as_ref().map(|n| n.ty.clone()).unwrap_or_default();
        let mut flags = 0x20u8;
        if !s.keyable_off {
            flags |= 0x04;
        }
        if s.lock {
            flags |= 0x01;
        }
        if s.size.is_some() {
            flags |= 0x08;
        }
        let mut d = Data::default();
        d.str(&s.plug).u8(flags);
        if let Some(n) = s.size {
            d.u32(n);
        }
        let leaf = match self.value(&node_type, &s, &mut d, h) {
            Ok(tag) => tag,
            Err(why) => {
                self.warn(why);
                return;
            }
        };
        let Some(tag) = leaf else { return };
        if let Some(n) = self.current.as_mut() {
            n.kids.push(Item::Leaf(tag, d.0));
        }
    }

    /// The value's chunk kind, its data written after the plug and flags; None for nothing to write.
    fn value(&mut self, node_type: &str, s: &Set, d: &mut Data, h: &Hints) -> Result<Option<[u8; 4]>, String> {
        let v = &s.values;
        let ty = s.ty.as_deref();
        let leaf = s.plug.rsplit('.').next().unwrap_or("").split('[').next().unwrap_or("");
        let ticks_per_frame = TICKS / h.fps.max(1e-9);
        let numbers = || nums(v).ok_or_else(|| format!("values that aren't numbers on .{}", leaf));
        match ty {
            None => {
                if v.is_empty() {
                    let says = s.lock || s.size.is_some() || s.keyable_off;
                    return Ok(says.then_some(*b"FLGS"));
                }
                let x = numbers()?;
                if node_type.starts_with("animCurveT") && leaf == "ktv" {
                    for p in x.chunks_exact(2) {
                        d.raw(&((p[0] * ticks_per_frame).round() as i64).to_be_bytes()).f64(p[1]);
                    }
                    return Ok(Some(*b"CMPD"));
                }
                if is_time(node_type, leaf) {
                    x.iter().for_each(|f| {
                        d.raw(&((f * ticks_per_frame).round() as i64).to_be_bytes());
                    });
                    return Ok(Some(*b"DBLE"));
                }
                if sparse_path(&s.plug) {
                    let mut k = 0;
                    while k < x.len() {
                        let n = x[k] as usize;
                        d.u32(n as u32);
                        for j in 0..n {
                            let (i, val) = (x.get(k + 1 + 2 * j).copied().unwrap_or(0.0), x.get(k + 2 + 2 * j).copied().unwrap_or(0.0));
                            d.u32(i as u32).f64(val);
                        }
                        k += 1 + 2 * n;
                    }
                    return Ok(Some(*b"DBLE"));
                }
                x.iter().for_each(|f| {
                    d.f64(*f);
                });
                let ranged = s.plug.rsplit('.').next().is_some_and(|p| p.contains(':'));
                Ok(Some(if x.len() == 1 || ranged { *b"DBLE" } else { *b"CMPD" }))
            }
            Some("double2" | "double3" | "float2" | "float3") => {
                let x = numbers()?;
                let float = ty.is_some_and(|t| t.starts_with("float"));
                x.iter().for_each(|f| {
                    if float {
                        d.f32(*f as f32);
                    } else {
                        d.f64(*f);
                    }
                });
                Ok(Some(match ty {
                    Some("double2") => *b"DBL2",
                    Some("double3") => *b"DBL3",
                    Some("float2") => *b"FLT2",
                    _ => *b"FLT3",
                }))
            }
            Some("string") => {
                d.str(v.first().and_then(Tok::text).unwrap_or(""));
                Ok(Some(*b"STR "))
            }
            Some("stringArray") => {
                let items: Vec<&str> = v.iter().skip(1).filter_map(Tok::text).collect();
                d.u32(items.len() as u32);
                items.iter().for_each(|x| {
                    d.str(x);
                });
                Ok(Some(*b"STR#"))
            }
            Some("Int32Array") => {
                let x = numbers()?;
                let items = x.get(1..).unwrap_or(&[]);
                d.u32(items.len() as u32);
                items.iter().for_each(|i| {
                    d.u32(*i as i32 as u32);
                });
                Ok(Some(*b"I32#"))
            }
            Some("doubleArray") => {
                let x = numbers()?;
                let items = x.get(1..).unwrap_or(&[]);
                d.u32(items.len() as u32);
                items.iter().for_each(|f| {
                    d.f64(*f);
                });
                Ok(Some(*b"DBL#"))
            }
            Some("pointArray") => {
                let x = numbers()?;
                x.get(1..).unwrap_or(&[]).iter().for_each(|f| {
                    d.f64(*f);
                });
                Ok(Some(*b"PNT#"))
            }
            Some("matrix") => {
                if v.first().and_then(Tok::text) == Some("xform") {
                    let x: Vec<f64> = v[1..].iter().map(|t| t.num().unwrap_or(0.0)).collect();
                    if x.len() < 37 {
                        return Err("a transformation matrix cut short".into());
                    }
                    x[..6].iter().for_each(|f| {
                        d.f64(*f);
                    });
                    d.u32(x[6] as u32).u32(0);
                    x[7..36].iter().for_each(|f| {
                        d.f64(*f);
                    });
                    d.f64(0.0).f64(0.0).f64(0.0).f64(1.0);
                    // The flag fills a word of its own.
                    d.u8(u8::from(x[36] != 0.0)).raw(&[0; 7]);
                    return Ok(Some(*b"MATR"));
                }
                let x = numbers()?;
                if x.len() != 16 {
                    return Err("a matrix without 16 values".into());
                }
                x.iter().for_each(|f| {
                    d.f64(*f);
                });
                Ok(Some(*b"MATR"))
            }
            Some("componentList") => {
                let items: Vec<&str> = v.iter().skip(1).filter_map(Tok::text).collect();
                let mut groups: Vec<([u8; 4], Vec<(i32, i32)>)> = Vec::new();
                for it in items {
                    let Some((kind, rest)) = it.split_once('[') else { return Err(format!("component {it}")) };
                    let Some(k) = component_kind(kind) else { return Err(format!("{kind} components in a component list")) };
                    let r = rest.trim_end_matches(']');
                    if r.contains('[') {
                        return Err(format!("{kind} components in a component list"));
                    }
                    let range = if r == "*" {
                        (-1, -1)
                    } else {
                        let (a, b) = r.split_once(':').unwrap_or((r, r));
                        (a.parse().map_err(|_| format!("component {it}"))?, b.parse().map_err(|_| format!("component {it}"))?)
                    };
                    match groups.last_mut() {
                        Some((g, list)) if g == k => list.push(range),
                        _ => groups.push((*k, vec![range])),
                    }
                }
                d.u32(groups.len() as u32);
                for (k, list) in &groups {
                    d.raw(k).u32(list.len() as u32);
                    for (a, b) in list {
                        d.u32(*a as u32).u32(*b as u32);
                    }
                }
                Ok(Some(*b"CMP#"))
            }
            Some("nurbsCurve") => {
                let t: Vec<f64> = v.iter().map(|t| t.num().unwrap_or(0.0)).collect();
                let (degree, spans, form, rational, dim) = (*t.first().unwrap_or(&0.0) as u32, *t.get(1).unwrap_or(&0.0) as u32, *t.get(2).unwrap_or(&0.0) as u32, *t.get(3).unwrap_or(&0.0) as u32, *t.get(4).unwrap_or(&0.0) as u32);
                let nk = *t.get(5).unwrap_or(&0.0) as usize;
                let knots = t.get(6..6 + nk).ok_or("a curve cut short")?;
                let ncv = *t.get(6 + nk).unwrap_or(&0.0) as usize;
                let per = (dim + rational) as usize;
                let cvs = t.get(7 + nk..7 + nk + ncv * per).ok_or("a curve cut short")?;
                d.u32(degree).u32(spans).u32(form).u32(rational).u32(dim).u32(nk as u32);
                knots.iter().for_each(|k| {
                    d.f64(*k);
                });
                d.u32(ncv as u32);
                cvs.iter().for_each(|x| {
                    d.f64(*x);
                });
                Ok(Some(*b"NRBC"))
            }
            Some("characterMapping") => {
                // Count, then each member's plug, kind (a byte) and index, as the reference's files hold it.
                let n = v.first().and_then(Tok::num).unwrap_or(0.0) as u32;
                d.u32(n);
                for e in v.get(1..).unwrap_or(&[]).chunks(3) {
                    if let [plug, kind, idx] = e {
                        d.str(plug.text().unwrap_or("")).u8(kind.num().unwrap_or(0.0) as u8).u32(idx.num().unwrap_or(0.0) as u32);
                    }
                }
                Ok(Some(*b"CHMP"))
            }
            Some("attributeAlias") => {
                let items: Vec<&str> = v.iter().filter(|t| matches!(t, Tok::Str(_))).filter_map(Tok::text).collect();
                d.u32(items.len() as u32);
                items.iter().for_each(|x| {
                    d.str(x);
                });
                Ok(Some(*b"ATAL"))
            }
            Some("dataReferenceEdits") => self.ref_edits(s, d).map(|_| Some(*b"REFE")),
            Some(other) => Err(format!("values of type {other}")),
        }
    }

    /// A reference node's edits: the owner, then for each node it edits, the count and the edits.
    fn ref_edits(&mut self, s: &Set, d: &mut Data) -> Result<(), String> {
        // REFE data starts with the plug and a flag byte, written again here in its own order.
        let v = &s.values;
        let mut out = Data::default();
        out.str(&s.plug).u8(0);
        out.str(v.first().and_then(Tok::text).unwrap_or(""));
        let mut groups = Data::default();
        let mut count = 0u32;
        let mut i = 1;
        let text = |i: usize| v.get(i).and_then(Tok::text).ok_or("reference edits cut short".to_string());
        let num = |i: usize| v.get(i).and_then(Tok::num).ok_or("reference edits cut short".to_string());
        while i < v.len() {
            let node = text(i)?;
            let n = num(i + 1)? as usize;
            i += 2;
            groups.str(node).u32(n as u32);
            for _ in 0..n {
                let kind = num(i)? as u32;
                i += 1;
                groups.u32(kind);
                let strings = match kind {
                    0 | 2 => 3,
                    1 => 4,
                    3 | 7 | 8 => 2,
                    5 => {
                        groups.u32(num(i)? as u32);
                        i += 1;
                        4
                    }
                    other => return Err(format!("reference edits of kind {other}")),
                };
                for _ in 0..strings {
                    groups.str(text(i)?);
                    i += 1;
                }
            }
            count += 1;
        }
        out.u32(count).raw(&groups.0);
        d.0 = out.0;
        Ok(())
    }

    fn add_attr(&mut self, st: &[Tok]) {
        let mut long = String::new();
        let mut short = String::new();
        let mut nice: Option<String> = None;
        let (mut at, mut dt, mut en): (Option<String>, Option<String>, Option<String>) = (None, None, None);
        let (mut min, mut max, mut dv): (Option<f64>, Option<f64>, Option<f64>) = (None, None, None);
        let (mut keyable, mut hidden, mut multi, mut child) = (false, false, false, false);
        let mut i = 1;
        while i < st.len() {
            let arg = st.get(i + 1);
            let text = || arg.and_then(Tok::text).unwrap_or("").to_string();
            let step = match st[i].text().unwrap_or("") {
                "-ln" | "-longName" => {
                    long = text();
                    2
                }
                "-sn" | "-shortName" => {
                    short = text();
                    2
                }
                "-nn" | "-niceName" => {
                    nice = Some(text());
                    2
                }
                "-at" | "-attributeType" => {
                    at = Some(text());
                    2
                }
                "-dt" | "-dataType" => {
                    dt = Some(text());
                    2
                }
                "-en" | "-enumName" => {
                    en = Some(text());
                    2
                }
                "-min" | "-minValue" => {
                    min = arg.and_then(Tok::num);
                    2
                }
                "-max" | "-maxValue" => {
                    max = arg.and_then(Tok::num);
                    2
                }
                "-dv" | "-defaultValue" => {
                    dv = arg.and_then(Tok::num);
                    2
                }
                "-k" | "-keyable" => {
                    keyable = arg.and_then(Tok::num).unwrap_or(1.0) != 0.0;
                    2
                }
                "-h" | "-hidden" => {
                    hidden = arg.and_then(Tok::num).unwrap_or(1.0) != 0.0;
                    2
                }
                "-m" | "-multi" => {
                    multi = true;
                    1
                }
                "-p" | "-parent" => {
                    child = true;
                    2
                }
                "-ci" | "-cachedInternally" | "-dcb" | "-defaultChannelBox" | "-smn" | "-softMinValue" | "-smx" | "-softMaxValue" | "-nc" | "-numberOfChildren" | "-uac" | "-usedAsColor" | "-im" | "-indexMatters" | "-r" | "-readable" | "-w" | "-writable" | "-s" | "-storable" | "-hsn" | "-hasSoftMinValue" | "-hsx" | "-hasSoftMaxValue" | "-hnv" | "-hxv" => {
                    if arg.is_some_and(|t| !t.is_flag() && t.num().is_some()) {
                        2
                    } else {
                        1
                    }
                }
                _ => 1,
            };
            i += step;
        }
        if long.is_empty() {
            return;
        }
        if short.is_empty() {
            short = long.clone();
        }
        let code: &[u8; 4] = match (at.as_deref(), dt.as_deref()) {
            (_, Some(_)) => b"aTYP",
            (Some("bool"), _) => b"aBOL",
            (Some("double"), _) => b"aDBL",
            (Some("float"), _) => b"aFLT",
            (Some("long"), _) => b"aLNG",
            (Some("short"), _) => b"aSHT",
            (Some("byte"), _) => b"aBYT",
            (Some("char"), _) => b"aCHR",
            (Some("enum"), _) => b"aENM",
            (Some("message"), _) => b"aMSG",
            (Some("time"), _) => b"aTIM",
            (Some("doubleAngle"), _) => b"aANG",
            (Some("doubleLinear"), _) => b"aDIS",
            (Some(other), _) => {
                self.warn(format!("added attributes of type {other}"));
                return;
            }
            (None, None) => b"aDBL",
        };
        if child {
            self.warn("added attributes inside a compound");
            return;
        }
        let data_tag: [u8; 4] = match dt.as_deref() {
            None => [0; 4],
            Some("string") => *b"STR ",
            Some("stringArray") => *b"STR#",
            Some("matrix") => *b"MATR",
            Some("doubleArray") => *b"DBL#",
            Some("Int32Array") => *b"I32#",
            Some("pointArray") => *b"PNT#",
            Some("vectorArray") => *b"VCT#",
            Some("mesh") => *b"MESH",
            Some("nurbsCurve") => *b"NCRV",
            Some("componentList") => *b"CMP#",
            Some("attributeAlias") => *b"ATAL",
            Some(other) => {
                self.warn(format!("added attributes of data type {other}"));
                return;
            }
        };
        let mut show = 0u8;
        if keyable {
            show |= 0x40;
        }
        if hidden {
            show |= 0x20;
        }
        if nice.is_some() {
            show |= 0x80;
        }
        let mut has = 0u8;
        if multi {
            has |= 0x01;
        }
        if min.is_some() {
            has |= 0x02;
        }
        if max.is_some() {
            has |= 0x04;
        }
        if dv.is_some() {
            has |= 0x08;
        }
        let mut d = Data::default();
        d.raw(code).raw(&[0, 0xf8, show, has]).u32(1).raw(&data_tag).str(&long).str(&short);
        if let Some(n) = &nice {
            d.str(n);
        }
        if code == b"aENM" {
            d.str(en.as_deref().unwrap_or(""));
        }
        d.u32(0);
        if let Some(x) = min {
            d.f64(x);
        }
        if let Some(x) = max {
            d.f64(x);
        }
        if let Some(x) = dv {
            d.raw(b"DBLE").f64(x);
        }
        match self.current.as_mut() {
            Some(n) => n.kids.push(Item::Leaf(*b"ATTR", d.0)),
            None => self.warn("addAttr with no node before it"),
        }
    }

    fn connect(&mut self, st: &[Tok]) {
        let plain: Vec<&str> = st.iter().skip(1).filter(|t| !t.is_flag()).filter_map(Tok::text).collect();
        let na = st.iter().any(|t| matches!(t.text(), Some("-na" | "-nextAvailable")));
        if let [a, b, ..] = plain.as_slice() {
            let mut d = Data::default();
            d.u8(u8::from(na)).str(a).str(b);
            self.conns.push(Item::Leaf(*b"CWFL", d.0));
        }
    }

    fn relationship(&mut self, st: &[Tok]) {
        let plain: Vec<&str> = st.iter().skip(1).filter_map(Tok::text).collect();
        if let [kind, node, rest @ ..] = plain.as_slice() {
            let mut d = Data::default();
            d.str(kind).str(node).u32(rest.len() as u32);
            rest.iter().for_each(|x| {
                d.str(x);
            });
            self.conns.push(Item::Leaf(*b"RELA", d.0));
        }
    }

    fn requires(&mut self, st: &[Tok]) {
        let mut flags = String::new();
        let mut plain = Vec::new();
        let mut i = 1;
        while i < st.len() {
            if st[i].is_flag() {
                let f = st[i].text().unwrap_or("");
                if let Some(arg) = st.get(i + 1).and_then(Tok::text) {
                    flags.push_str(&format!(" {f} \"{arg}\""));
                }
                i += 2;
            } else {
                plain.push(st[i].text().map(str::to_string).unwrap_or_else(|| st[i].num().map(|v| v.to_string()).unwrap_or_default()));
                i += 1;
            }
        }
        match plain.as_slice() {
            [m, v, ..] if m == "maya" => self.version = v.clone(),
            [name, version, ..] => self.plugins.push((name.clone(), version.clone(), flags)),
            [name] => self.plugins.push((name.clone(), String::new(), flags)),
            [] => {}
        }
    }

    fn file(&mut self, st: &[Tok]) {
        let mut depth: Option<u32> = None;
        let (mut ns, mut rfn, mut typ) = (String::new(), String::new(), String::from("mayaAscii"));
        let (mut deferred, mut prefix) = (false, false);
        let mut path = String::new();
        let mut i = 1;
        while i < st.len() {
            let arg = st.get(i + 1);
            let s = || arg.and_then(Tok::text).unwrap_or("").to_string();
            match st[i].text() {
                Some("-rdi" | "-referenceDepthInfo") => {
                    depth = arg.and_then(Tok::num).map(|v| v as u32);
                    i += 2;
                }
                Some("-ns" | "-namespace") => {
                    ns = s();
                    i += 2;
                }
                Some("-rpr" | "-referencePrefix") => {
                    prefix = true;
                    ns = s();
                    i += 2;
                }
                Some("-rfn" | "-referenceNode") => {
                    rfn = s();
                    i += 2;
                }
                Some("-typ" | "-type") => {
                    typ = s();
                    i += 2;
                }
                Some("-dr" | "-deferReference") => {
                    deferred = arg.and_then(Tok::num).unwrap_or(1.0) != 0.0;
                    i += 2;
                }
                Some("-op" | "-options") => i += 2,
                Some("-r" | "-reference") => i += 1,
                _ if !st[i].is_flag() => {
                    path = st[i].text().unwrap_or("").to_string();
                    i += 1;
                }
                _ => i += 1,
            }
        }
        if prefix {
            self.warn("reference prefixes (written as namespaces)");
        }
        match depth {
            Some(depth) => {
                if depth != 1 {
                    self.warn("nested references");
                    return;
                }
                let mut d = Data::default();
                d.u32(depth).str(&path).str(&ns).u8(0).u8(u8::from(deferred)).str(&rfn);
                self.ref_states.push(Item::Leaf(*b"FRDI", d.0));
            }
            None => {
                let mut d = Data::default();
                d.str(&path).str(&ns).str(&rfn).str(&typ);
                self.refs.push(Item::Leaf(*b"FREF", d.0));
            }
        }
    }

    fn header(&self, h: &Hints) -> Item {
        let s = |t: &[u8; 4], v: &str| Item::Leaf(*t, v.as_bytes().to_vec());
        let mut kids = vec![s(b"VERS", if self.version.is_empty() { "2024" } else { &self.version })];
        for t in [b"UVER", b"MADE", b"ICON", b"INFO", b"OBJN"] {
            kids.push(s(t, "undef"));
        }
        kids.push(s(b"LUNI", &h.linear));
        kids.push(s(b"TUNI", &h.time));
        kids.push(s(b"AUNI", &h.angle));
        kids.push(s(b"TDUR", &format!("{}", TICKS as u64)));
        for (k, v) in &self.file_info {
            let mut d = Data::default();
            d.str(k).str(v);
            kids.push(Item::Leaf(*b"FINF", d.0));
        }
        for (name, version, flags) in &self.plugins {
            let mut d = Data::default();
            d.str(name).str(version).str(flags.trim());
            kids.push(Item::Leaf(*b"PLUG", d.0));
        }
        if !self.made_ids.is_empty() {
            // Types with ids of Aztek's own: a plug-in entry with no name, which declares them and asks for nothing.
            let mut d = Data::default();
            d.str("").str("").str("");
            for (ty, id) in &self.made_ids {
                d.str(ty).raw(id);
            }
            kids.push(Item::Leaf(*b"PLUG", d.0));
        }
        Item::Group(*b"FOR8", *b"HEAD", kids)
    }
}

/// The `.ma` text as a binary scene file, and what couldn't be carried over.
pub fn encode(text: &str, h: &Hints) -> (Vec<u8>, Vec<String>) {
    let mut w = Writer::default();
    let mut lex = Lexer::new(text);
    let mut st = Vec::new();
    while lex.statement(&mut st) {
        let Some(cmd) = st.first().and_then(Tok::text).map(str::to_string) else { continue };
        match cmd.as_str() {
            "requires" => w.requires(&st),
            "currentUnit" => {}
            "fileInfo" => {
                let plain: Vec<&str> = st.iter().skip(1).filter_map(Tok::text).collect();
                if let [k, v, ..] = plain.as_slice() {
                    w.file_info.push((k.to_string(), v.to_string()));
                }
            }
            "file" => w.file(&st),
            "createNode" => w.create(&st, h),
            "select" => w.select(&st),
            "rename" if st.iter().any(|t| t.text() == Some("-uid")) => w.rename_uid(&st),
            "setAttr" => w.set_attr(&st, h),
            "addAttr" => w.add_attr(&st),
            "connectAttr" => w.connect(&st),
            "relationship" => w.relationship(&st),
            "lockNode" => w.warn("node locks"),
            other => w.warn(format!("{other} statements")),
        }
    }
    w.finish_node();
    let mut kids = vec![w.header(h)];
    if !w.refs.is_empty() {
        kids.push(Item::Group(*b"FOR8", *b"FREF", std::mem::take(&mut w.refs)));
    }
    if !w.ref_states.is_empty() {
        kids.push(Item::Group(*b"FOR8", *b"FRDI", std::mem::take(&mut w.ref_states)));
    }
    kids.append(&mut w.body);
    kids.push(Item::Group(*b"LIS8", *b"CONS", vec![Item::Group(*b"FOR8", *b"CONN", std::mem::take(&mut w.conns))]));
    let mut out = Vec::new();
    emit(&mut out, &Item::Group(*b"FOR8", *b"Maya", kids));
    let warnings = w.warnings.iter().map(|(what, n)| format!("{n} × {what}: left out of the binary file")).collect();
    (out, warnings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_round_trip() {
        let u = "B5354AAA-4AE9-9D9F-3FCD-8EAD69270BB0";
        let b = uid_bytes(u).unwrap();
        assert_eq!(b, [0xaa, 0x4a, 0x35, 0xb5, 0x9f, 0x9d, 0xe9, 0x4a, 0xad, 0x8e, 0xcd, 0x3f, 0xb0, 0x0b, 0x27, 0x69]);
    }
}
