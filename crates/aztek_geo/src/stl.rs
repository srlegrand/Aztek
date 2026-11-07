//! STL reading (ASCII and binary, with VisCAM and Materialise colors) and writing.

use glam::{DVec3, Vec3};
use rayon::prelude::*;

use crate::attribute::{Index, INVALID};
use crate::colors;
use crate::io_common::{append_number, format_blocks, linear_to_srgb, srgb_to_linear};
use crate::mesh::Mesh;
use crate::normals::set_hard_edges_by_angle;
use crate::point_weld::weld_identical_f32;
use crate::triangulate::{triangulate, TriangulateOptions};

const HEADER: usize = 80;
const TRIANGLE: usize = 50;

#[derive(Clone, Debug)]
pub struct StlReadOptions {
    /// Merge corners at bit-identical positions into shared points, so the mesh is connected.
    pub weld: bool,
    /// Edges between faces bent more than this many degrees are hard; negative leaves every edge soft.
    pub hard_angle: f64,
}

impl Default for StlReadOptions {
    fn default() -> Self {
        Self { weld: true, hard_angle: 30.0 }
    }
}

#[derive(Clone, Debug, Default)]
pub struct StlSolid {
    pub name: String,
    pub mesh: Mesh,
}

#[derive(Clone, Debug, Default)]
pub struct StlReadResult {
    /// One per ASCII "solid"; a binary file is always one.
    pub solids: Vec<StlSolid>,
    pub binary: bool,
    /// Binary files whose triangles carry VisCAM or Materialise colors, read into a color set.
    pub colors: bool,
    pub error: String,
}

impl StlReadResult {
    pub fn ok(&self) -> bool {
        self.error.is_empty()
    }
}

#[derive(Clone, Debug)]
pub struct StlWriteOptions {
    pub binary: bool,
    /// Binary only: the current color set, averaged per triangle, in the VisCAM 15-bit convention.
    pub colors: bool,
    /// ASCII significant digits; 0 writes the shortest text that reads back exactly.
    pub precision: i32,
}

impl Default for StlWriteOptions {
    fn default() -> Self {
        Self { binary: true, colors: false, precision: 0 }
    }
}

fn u32_at(d: &[u8], p: usize) -> u32 {
    u32::from_le_bytes(d[p..p + 4].try_into().unwrap())
}
fn u16_at(d: &[u8], p: usize) -> u16 {
    u16::from_le_bytes(d[p..p + 2].try_into().unwrap())
}
fn f32_at(d: &[u8], p: usize) -> f32 {
    f32::from_le_bytes(d[p..p + 4].try_into().unwrap())
}

fn is_space(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c)
}

/// Corner positions and polygon sizes as read, before welding.
#[derive(Default)]
struct Soup {
    corners: Vec<Vec3>,
    sizes: Vec<Index>,
    /// Per polygon when the file has colors, else empty.
    face_color: Vec<[f32; 4]>,
}

/// The mesh from a soup: welded (or a point per corner), degenerate polygons dropped, unused points removed.
fn build_mesh(soup: Soup, opts: &StlReadOptions) -> (Mesh, bool) {
    let nf = soup.sizes.len();
    let (point_of, mut points) = if opts.weld {
        weld_identical_f32(&soup.corners, false)
    } else {
        ((0..soup.corners.len() as Index).collect(), soup.corners.iter().map(|p| p.as_dvec3()).collect())
    };
    let mut start = vec![0usize; nf];
    let mut sum = 0;
    for (s, &n) in start.iter_mut().zip(&soup.sizes) {
        *s = sum;
        sum += n as usize;
    }
    let keep: Vec<bool> = (0..nf)
        .into_par_iter()
        .map(|f| {
            let n = soup.sizes[f] as usize;
            let p = &point_of[start[f]..start[f] + n];
            n >= 3 && (0..n).all(|i| (i + 1..n).all(|j| p[i] != p[j]))
        })
        .collect();
    let dropped = keep.iter().any(|k| !k);
    let (sizes, indices, face_color) = if !dropped {
        (soup.sizes, point_of, soup.face_color)
    } else {
        let mut sizes = Vec::new();
        let mut indices = Vec::new();
        let mut face_color = Vec::new();
        for f in 0..nf {
            if !keep[f] {
                continue;
            }
            sizes.push(soup.sizes[f]);
            indices.extend_from_slice(&point_of[start[f]..start[f] + soup.sizes[f] as usize]);
            if !soup.face_color.is_empty() {
                face_color.push(soup.face_color[f]);
            }
        }
        // Points only the dropped polygons used go, the rest keep their order.
        let mut remap = vec![INVALID; points.len()];
        for &p in &indices {
            remap[p as usize] = 0;
        }
        let mut next = 0;
        for p in 0..points.len() {
            if remap[p] == 0 {
                points[next] = points[p];
                remap[p] = next as Index;
                next += 1;
            }
        }
        points.truncate(next);
        for i in indices.iter_mut() {
            *i = remap[*i as usize];
        }
        (sizes, indices, face_color)
    };

    let mut mesh = Mesh::from_polygons(&points, &sizes, &indices);
    let any_color = !face_color.is_empty();
    if any_color {
        let face_of: Vec<Index> = (0..mesh.corner_capacity()).map(|c| mesh.corner_face(c)).collect();
        let col = colors::add_white(&mut mesh, "colorSet");
        col.par_iter_mut().enumerate().for_each(|(c, v)| *v = face_color[face_of[c] as usize]);
    }
    if opts.hard_angle >= 0.0 {
        set_hard_edges_by_angle(&mut mesh, opts.hard_angle, &[]);
    }
    (mesh, any_color)
}

/// VisCAM sets bit 15 on colored triangles with red high; Materialise ("COLOR=" header) clears it with red low.
fn triangle_color(attr: u16, materialise: bool) -> Option<[f32; 4]> {
    let has = if materialise { attr & 0x8000 == 0 } else { attr & 0x8000 != 0 };
    if !has {
        return None;
    }
    let lo = (attr & 31) as f32 / 31.0;
    let mid = ((attr >> 5) & 31) as f32 / 31.0;
    let hi = ((attr >> 10) & 31) as f32 / 31.0;
    let (r, b) = if materialise { (lo, hi) } else { (hi, lo) };
    Some([srgb_to_linear(r), srgb_to_linear(mid), srgb_to_linear(b), 1.0])
}

fn read_binary(data: &[u8], opts: &StlReadOptions, r: &mut StlReadResult) {
    let n = u32_at(data, HEADER) as usize;
    let header = &data[..HEADER];
    let materialise = header.windows(6).any(|w| w == b"COLOR=");
    let tris = HEADER + 4;
    // Most writers leave the attribute bytes zero. Trust them as colors when they vary or all carry the VisCAM flag.
    let first_attr = if n > 0 { u16_at(data, tris + 48) } else { 0 };
    let attr = |t: usize| u16_at(data, tris + t * TRIANGLE + 48);
    let differ = (0..n).into_par_iter().any(|t| attr(t) != first_attr);
    let all_flagged = (0..n).into_par_iter().all(|t| attr(t) & 0x8000 != 0);
    let use_colors = n > 0 && (differ || all_flagged || materialise);

    let mut soup = Soup {
        corners: (0..n * 3)
            .into_par_iter()
            .map(|i| {
                let p = tris + (i / 3) * TRIANGLE + 12 + (i % 3) * 12;
                Vec3::new(f32_at(data, p), f32_at(data, p + 4), f32_at(data, p + 8))
            })
            .collect(),
        sizes: vec![3; n],
        face_color: Vec::new(),
    };
    if use_colors {
        let found: Vec<Option<[f32; 4]>> = (0..n).into_par_iter().map(|t| triangle_color(attr(t), materialise)).collect();
        if found.iter().any(Option::is_some) {
            soup.face_color = found.into_iter().map(|c| c.unwrap_or([1.0; 4])).collect();
        }
    }
    r.binary = true;
    let end = header.iter().position(|&c| c == 0).unwrap_or(HEADER);
    let mut name = &header[..end];
    if name.starts_with(b"solid ") {
        name = &name[6..];
    }
    while name.last().is_some_and(|&c| is_space(c)) {
        name = &name[..name.len() - 1];
    }
    let (mesh, colors) = build_mesh(soup, opts);
    r.colors = colors;
    r.solids.push(StlSolid { name: String::from_utf8_lossy(name).into_owned(), mesh });
}

struct Tokens<'a> {
    text: &'a [u8],
    at: usize,
}

impl<'a> Tokens<'a> {
    fn next(&mut self) -> &'a [u8] {
        while self.at < self.text.len() && is_space(self.text[self.at]) {
            self.at += 1;
        }
        let start = self.at;
        while self.at < self.text.len() && !is_space(self.text[self.at]) {
            self.at += 1;
        }
        &self.text[start..self.at]
    }
    /// The rest of the line, trimmed.
    fn line(&mut self) -> &'a [u8] {
        while self.at < self.text.len() && matches!(self.text[self.at], b' ' | b'\t') {
            self.at += 1;
        }
        let start = self.at;
        while self.at < self.text.len() && !matches!(self.text[self.at], b'\n' | b'\r') {
            self.at += 1;
        }
        let mut s = &self.text[start..self.at];
        while s.last().is_some_and(|&c| is_space(c)) {
            s = &s[..s.len() - 1];
        }
        s
    }
    /// A whole token as from_chars reads a float.
    fn number(&mut self) -> Option<f32> {
        let mut t = self.next();
        if t.first() == Some(&b'+') {
            t = &t[1..];
        }
        if t.first() == Some(&b'+') {
            return None;
        }
        std::str::from_utf8(t).ok()?.parse::<f32>().ok()
    }
}

/// One solid's polygons as read: vertex ranges of the loops, from the file's whole vertex list.
#[derive(Default)]
struct AsciiSolid {
    name: String,
    loops: Vec<(usize, Index)>,
}

/// Token by token, for files that don't keep each keyword on its own line.
fn read_ascii_tokens(data: &[u8], verts: &mut Vec<Vec3>, solids: &mut Vec<AsciiSolid>) -> Result<(), String> {
    let mut tok = Tokens { text: data, at: 0 };
    loop {
        let t = tok.next();
        if t.is_empty() {
            break;
        }
        if t != b"solid" {
            return Err(format!("expected \"solid\", found \"{}\"", String::from_utf8_lossy(&t[..t.len().min(32)])));
        }
        let mut s = AsciiSolid { name: String::from_utf8_lossy(tok.line()).into_owned(), loops: Vec::new() };
        let mut loop_start = verts.len();
        loop {
            let t = tok.next();
            if t.is_empty() {
                break;
            }
            if t == b"endsolid" {
                tok.line();
                break;
            }
            if t == b"vertex" {
                match (tok.number(), tok.number(), tok.number()) {
                    (Some(x), Some(y), Some(z)) => verts.push(Vec3::new(x, y, z)),
                    _ => return Err(format!("bad vertex in solid \"{}\"", s.name)),
                }
            } else if t == b"endloop" {
                if verts.len() - loop_start >= 3 {
                    s.loops.push((loop_start, (verts.len() - loop_start) as Index));
                }
                loop_start = verts.len();
            }
            // facet normal, outer loop and endfacet carry nothing the mesh needs: normals are recomputed.
        }
        solids.push(s);
    }
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum LineKind {
    Solid,
    EndLoop,
    EndSolid,
}

struct LineEvent {
    kind: LineKind,
    /// Within the chunk.
    verts_before: usize,
    name: String,
}

#[derive(Default)]
struct LineChunk {
    verts: Vec<Vec3>,
    events: Vec<LineEvent>,
    bad: bool,
}

/// Line by line in parallel chunks, each keyword on its own line as writers put them. False when a line doesn't fit
/// that.
fn read_ascii_lines(data: &[u8], verts: &mut Vec<Vec3>, solids: &mut Vec<AsciiSolid>) -> bool {
    const CHUNK: usize = 4 << 20;
    let chunks = data.len().div_ceil(CHUNK);
    let find_nl = |from: usize| data[from..].iter().position(|&c| c == b'\n').map(|p| from + p);
    let parts: Vec<LineChunk> = (0..chunks)
        .into_par_iter()
        .map(|k| {
            let mut out = LineChunk::default();
            let end = data.len().min((k + 1) * CHUNK);
            let mut at = k * CHUNK;
            // A chunk owns the lines that start inside it.
            if at > 0 && data[at - 1] != b'\n' {
                at = find_nl(at).map_or(end, |p| p + 1);
            }
            while at < end && !out.bad {
                let eol = find_nl(at).unwrap_or(data.len());
                let mut tok = Tokens { text: &data[..eol], at };
                at = eol + 1;
                let t = tok.next();
                if t.is_empty() {
                    continue;
                }
                let ok = match t {
                    b"vertex" => {
                        let v = (tok.number(), tok.number(), tok.number());
                        let ok = v.0.is_some() && v.1.is_some() && v.2.is_some() && tok.next().is_empty();
                        out.verts.push(Vec3::new(v.0.unwrap_or(0.0), v.1.unwrap_or(0.0), v.2.unwrap_or(0.0)));
                        ok
                    }
                    b"endloop" | b"endfacet" => {
                        if t == b"endloop" {
                            out.events.push(LineEvent { kind: LineKind::EndLoop, verts_before: out.verts.len(), name: String::new() });
                        }
                        tok.next().is_empty()
                    }
                    b"solid" => {
                        let name = String::from_utf8_lossy(tok.line()).into_owned();
                        out.events.push(LineEvent { kind: LineKind::Solid, verts_before: out.verts.len(), name });
                        true
                    }
                    b"endsolid" => {
                        out.events.push(LineEvent { kind: LineKind::EndSolid, verts_before: out.verts.len(), name: String::new() });
                        true
                    }
                    _ => t == b"facet" || t == b"outer",
                };
                if !ok {
                    out.bad = true;
                }
            }
            out
        })
        .collect();
    if parts.iter().any(|p| p.bad) {
        return false;
    }
    let mut base = vec![0usize; chunks + 1];
    for k in 0..chunks {
        base[k + 1] = base[k] + parts[k].verts.len();
    }
    verts.clear();
    for p in &parts {
        verts.extend_from_slice(&p.verts);
    }
    let mut current: Option<usize> = None;
    let mut loop_start = 0;
    for (k, part) in parts.iter().enumerate() {
        for e in &part.events {
            let at = base[k] + e.verts_before;
            match (e.kind, current) {
                (LineKind::Solid, Some(_)) => return false,
                (LineKind::Solid, None) => {
                    solids.push(AsciiSolid { name: e.name.clone(), loops: Vec::new() });
                    current = Some(solids.len() - 1);
                    loop_start = at;
                }
                (_, None) => return false,
                (LineKind::EndSolid, Some(_)) => current = None,
                (LineKind::EndLoop, Some(s)) => {
                    if at - loop_start >= 3 {
                        solids[s].loops.push((loop_start, (at - loop_start) as Index));
                    }
                    loop_start = at;
                }
            }
        }
    }
    true
}

fn read_ascii(data: &[u8], opts: &StlReadOptions, r: &mut StlReadResult) {
    let mut verts = Vec::new();
    let mut solids = Vec::new();
    if !read_ascii_lines(data, &mut verts, &mut solids) {
        verts.clear();
        solids.clear();
        if let Err(e) = read_ascii_tokens(data, &mut verts, &mut solids) {
            r.error = e;
            return;
        }
    }
    if solids.is_empty() {
        r.error = "no solids".into();
        return;
    }
    for a in &solids {
        let mut soup = Soup::default();
        for &(first, n) in &a.loops {
            soup.sizes.push(n);
            soup.corners.extend_from_slice(&verts[first..first + n as usize]);
        }
        let (mesh, _) = build_mesh(soup, opts);
        r.solids.push(StlSolid { name: a.name.clone(), mesh });
    }
}

/// Detects binary from the size the triangle count implies, so binary files whose header starts "solid" still read.
pub fn read_stl(data: &[u8], opts: &StlReadOptions) -> StlReadResult {
    let mut r = StlReadResult::default();
    let implied = |d: &[u8]| HEADER + 4 + u32_at(d, HEADER) as usize * TRIANGLE;
    let long_enough = data.len() >= HEADER + 4;
    let trimmed = &data[data.iter().position(|&c| !is_space(c)).unwrap_or(data.len())..];
    if long_enough && implied(data) == data.len() {
        read_binary(data, opts, &mut r);
    } else if trimmed.starts_with(b"solid") {
        read_ascii(data, opts, &mut r);
    } else if long_enough && implied(data) <= data.len() {
        // Binary with trailing bytes.
        read_binary(data, opts, &mut r);
    } else {
        r.error = if long_enough { "not an STL file, or truncated" } else { "file too short" }.into();
    }
    r
}

pub fn read_stl_file(path: &str, opts: &StlReadOptions) -> StlReadResult {
    match std::fs::read(path) {
        Ok(d) => read_stl(&d, opts),
        Err(_) => StlReadResult { error: format!("cannot open {path}"), ..Default::default() },
    }
}

struct Triangle {
    p: [Vec3; 3],
    normal: Vec3,
    attr: u16,
}

fn triangles(m: &Mesh, want_colors: bool) -> Vec<Triangle> {
    let tris = triangulate(m, &TriangulateOptions::default());
    let set = if want_colors { colors::current(m) } else { String::new() };
    let col: &[[f32; 4]] = if set.is_empty() { &[] } else { colors::values(m, &set) };
    (0..tris.triangle_count() as usize)
        .into_par_iter()
        .map(|t| {
            let c: [Index; 3] = std::array::from_fn(|k| tris.corners[t * 3 + k]);
            let pd: [DVec3; 3] = c.map(|c| m.position(m.corner_point(c)));
            let mut sum = [0.0f32; 4];
            if !col.is_empty() {
                for &ci in &c {
                    for k in 0..4 {
                        sum[k] += col[ci as usize][k];
                    }
                }
            }
            let n = (pd[1] - pd[0]).cross(pd[2] - pd[0]);
            let len = n.length();
            let normal = if len > 0.0 { (n / len).as_vec3() } else { Vec3::ZERO };
            let attr = if col.is_empty() {
                0
            } else {
                let q = |v: f32| (linear_to_srgb(v) * 31.0).round() as u16;
                let s = sum.map(|x| x / 3.0);
                0x8000 | (q(s[0]) << 10) | (q(s[1]) << 5) | q(s[2])
            };
            Triangle { p: pd.map(|p| p.as_vec3()), normal, attr }
        })
        .collect()
}

/// Faces are triangulated; each solid becomes one ASCII "solid", and binary files merge them.
pub fn write_stl(solids: &[StlSolid], opts: &StlWriteOptions) -> Vec<u8> {
    if opts.binary {
        let mut all = Vec::new();
        for solid in solids {
            all.extend(triangles(&solid.mesh, opts.colors));
        }
        // Never "solid" first, or readers that sniff the header take it for ASCII.
        let mut header = if solids.len() == 1 && !solids[0].name.is_empty() {
            format!("Aztek STL: {}", solids[0].name).into_bytes()
        } else {
            b"Aztek STL".to_vec()
        };
        header.resize(HEADER, 0);
        let mut s = header;
        s.extend_from_slice(&(all.len() as u32).to_le_bytes());
        s.resize(HEADER + 4 + all.len() * TRIANGLE, 0);
        s[HEADER + 4..].par_chunks_mut(TRIANGLE).zip(all.par_iter()).for_each(|(out, t)| {
            let mut at = 0;
            let mut put = |b: &[u8]| {
                out[at..at + b.len()].copy_from_slice(b);
                at += b.len();
            };
            for k in 0..3 {
                put(&t.normal[k].to_le_bytes());
            }
            for p in &t.p {
                for k in 0..3 {
                    put(&p[k].to_le_bytes());
                }
            }
            put(&t.attr.to_le_bytes());
        });
        return s;
    }
    let mut s = String::new();
    let prec = opts.precision;
    for solid in solids {
        let name = if solid.name.is_empty() { "mesh" } else { &solid.name };
        s += &format!("solid {name}\n");
        let tris = triangles(&solid.mesh, false);
        s += &format_blocks(tris.len(), |out, i| {
            let t = &tris[i];
            out.push_str("  facet normal ");
            for k in 0..3 {
                append_number(out, t.normal[k], prec);
                out.push(if k < 2 { ' ' } else { '\n' });
            }
            out.push_str("    outer loop\n");
            for p in &t.p {
                out.push_str("      vertex ");
                for k in 0..3 {
                    append_number(out, p[k], prec);
                    out.push(if k < 2 { ' ' } else { '\n' });
                }
            }
            out.push_str("    endloop\n  endfacet\n");
        });
        s += &format!("endsolid {name}\n");
    }
    s.into_bytes()
}
