//! Stanford PLY reading (ASCII, binary little and big endian) and writing.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use glam::DVec3;
use rayon::prelude::*;

use crate::attribute::{names, Index, INVALID};
use crate::colors;
use crate::io_common::{append_number, linear_to_srgb, parse_double, srgb_to_linear};
use crate::mesh::Mesh;
use crate::normals::{corner_normals, CornerNormalOptions};
use crate::par::Disjoint;
use crate::point_weld::number_distinct;
use crate::primitives::mark_uv_seams;

#[derive(Clone, Debug)]
pub struct PlyReadOptions {
    /// Keep file normals as the corner "normal" attribute (point "normal" on a point cloud).
    pub keep_normals: bool,
}

impl Default for PlyReadOptions {
    fn default() -> Self {
        Self { keep_normals: true }
    }
}

#[derive(Clone, Debug, Default)]
pub struct PlyReadResult {
    /// Polygons with corner normals, uv0 and color sets ("colorSet" from vertex colors, "faceColor" from face
    /// colors). Other numeric vertex and face properties become float attributes of the same name. A file without
    /// faces is a point cloud: loose points whose normals, colors and uvs are point attributes.
    pub mesh: Mesh,
    pub point_cloud: bool,
    /// From "comment TextureFile" lines (MeshLab), as written.
    pub texture_files: Vec<String>,
    /// Elements and properties that had no Aztek equivalent.
    pub skipped: Vec<String>,
    pub error: String,
}

impl PlyReadResult {
    pub fn ok(&self) -> bool {
        self.error.is_empty()
    }
}

#[derive(Clone, Debug)]
pub struct PlyWriteOptions {
    pub binary: bool,
    pub normals: bool,
    /// uv0 as per-vertex s, t.
    pub uvs: bool,
    /// The current color set as 8-bit sRGB red, green, blue, alpha.
    pub colors: bool,
    /// ASCII significant digits; 0 writes the shortest text that reads back exactly.
    pub precision: i32,
}

impl Default for PlyWriteOptions {
    fn default() -> Self {
        Self { binary: true, normals: true, uvs: true, colors: true, precision: 0 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Type {
    I8,
    U8,
    I16,
    U16,
    I32,
    U32,
    F32,
    F64,
}

fn parse_type(s: &str) -> Option<Type> {
    Some(match s {
        "char" | "int8" => Type::I8,
        "uchar" | "uint8" => Type::U8,
        "short" | "int16" => Type::I16,
        "ushort" | "uint16" => Type::U16,
        "int" | "int32" => Type::I32,
        "uint" | "uint32" => Type::U32,
        "float" | "float32" => Type::F32,
        "double" | "float64" => Type::F64,
        _ => return None,
    })
}

fn type_size(t: Type) -> usize {
    match t {
        Type::I8 | Type::U8 => 1,
        Type::I16 | Type::U16 => 2,
        Type::I32 | Type::U32 | Type::F32 => 4,
        Type::F64 => 8,
    }
}

/// Integer color channels are normalized by their type's range; floats are already 0 to 1.
fn channel_scale(t: Type) -> f32 {
    match t {
        Type::U16 => 1.0 / 65535.0,
        Type::F32 | Type::F64 => 1.0,
        _ => 1.0 / 255.0,
    }
}

/// Values of a narrow type are kept as float, so they read back exactly as stored.
fn wide_type(t: Type) -> bool {
    matches!(t, Type::F64 | Type::I32 | Type::U32)
}

#[derive(Clone, Debug)]
struct Property {
    name: String,
    ty: Type,
    list: bool,
    count_type: Type,
}

#[derive(Clone, Debug, Default)]
struct Element {
    name: String,
    count: usize,
    props: Vec<Property>,
}

impl Element {
    fn find(&self, names: &[&str]) -> Option<usize> {
        names.iter().find_map(|n| self.props.iter().position(|p| p.name == *n))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Format {
    Ascii,
    Little,
    Big,
}

fn is_space(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c)
}

/// Reads values from one record of the body, in any of the three encodings, never past its end.
struct RecordReader<'a> {
    s: &'a [u8],
    at: usize,
    end: usize,
    format: Format,
    ok: bool,
}

impl<'a> RecordReader<'a> {
    fn new(s: &'a [u8], at: usize, end: usize, format: Format) -> Self {
        Self { s, at, end, format, ok: true }
    }

    fn value(&mut self, t: Type) -> f64 {
        if self.format == Format::Ascii {
            return self.ascii_value();
        }
        let n = type_size(t);
        if self.end - self.at < n {
            self.ok = false;
            return 0.0;
        }
        let mut b = [0u8; 8];
        b[..n].copy_from_slice(&self.s[self.at..self.at + n]);
        self.at += n;
        if self.format == Format::Big {
            b[..n].reverse();
        }
        match t {
            Type::I8 => b[0] as i8 as f64,
            Type::U8 => b[0] as f64,
            Type::I16 => i16::from_le_bytes([b[0], b[1]]) as f64,
            Type::U16 => u16::from_le_bytes([b[0], b[1]]) as f64,
            Type::I32 => i32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f64,
            Type::U32 => u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f64,
            Type::F32 => f32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f64,
            Type::F64 => f64::from_le_bytes(b),
        }
    }

    fn skip(&mut self, t: Type, count: usize) {
        if self.format != Format::Ascii {
            if (self.end - self.at) / type_size(t) < count {
                self.ok = false;
            } else {
                self.at += count * type_size(t);
            }
            return;
        }
        for _ in 0..count {
            if !self.ok {
                break;
            }
            self.ascii_value();
        }
    }

    fn ascii_value(&mut self) -> f64 {
        while self.at < self.end && is_space(self.s[self.at]) {
            self.at += 1;
        }
        let mut first = self.at;
        if first < self.end && self.s[first] == b'+' {
            first += 1;
        }
        match parse_double(&self.s[first..self.end]) {
            Some((v, n)) if n > 0 => {
                self.at = first + n;
                v
            }
            _ => {
                self.ok = false;
                0.0
            }
        }
    }
}

#[derive(Default)]
struct Header {
    format: Option<Format>,
    elements: Vec<Element>,
    texture_files: Vec<String>,
    body_start: usize,
    error: String,
}

fn parse_header(data: &[u8]) -> Header {
    let mut h = Header::default();
    let mut at = 0usize;
    let next_line = |at: &mut usize| -> Option<String> {
        if *at >= data.len() {
            return None;
        }
        let end = data[*at..].iter().position(|&c| c == b'\n').map_or(data.len(), |p| *at + p);
        let mut line = &data[*at..end];
        *at = (end + 1).min(data.len());
        if line.last() == Some(&b'\r') {
            line = &line[..line.len() - 1];
        }
        Some(String::from_utf8_lossy(line).into_owned())
    };
    let words = |line: &str| -> Vec<String> {
        line.split(|c: char| c.is_ascii() && is_space(c as u8)).filter(|w| !w.is_empty()).map(str::to_string).collect()
    };

    match next_line(&mut at) {
        Some(l) if l == "ply" => {}
        _ => {
            h.error = "not a PLY file".into();
            return h;
        }
    }
    let mut format = Format::Ascii;
    let mut saw_format = false;
    while let Some(line) = next_line(&mut at) {
        let w = words(&line);
        if w.is_empty() {
            continue;
        }
        match w[0].as_str() {
            "end_header" => {
                h.body_start = at;
                h.format = Some(format);
                if !saw_format {
                    h.error = "no format line".into();
                }
                return h;
            }
            "comment" | "obj_info" => {
                if w.len() >= 3 && (w[1] == "TextureFile" || w[1] == "texturefile") {
                    let pos = line.find(&w[1]).unwrap() + w[1].len();
                    let rest = line[pos..].trim_start_matches(|c: char| c.is_ascii() && is_space(c as u8));
                    h.texture_files.push(rest.to_string());
                }
            }
            "format" if w.len() >= 2 => {
                saw_format = true;
                format = match w[1].as_str() {
                    "ascii" => Format::Ascii,
                    "binary_little_endian" => Format::Little,
                    "binary_big_endian" => Format::Big,
                    other => {
                        h.error = format!("unknown format \"{other}\"");
                        return h;
                    }
                };
            }
            "element" if w.len() >= 3 => {
                let name = w[1].clone();
                let Ok(count) = w[2].parse::<usize>() else {
                    h.error = format!("bad element count for \"{name}\"");
                    return h;
                };
                h.elements.push(Element { name, count, props: Vec::new() });
            }
            "property" if !h.elements.is_empty() => {
                let p = if w.len() >= 5 && w[1] == "list" {
                    match (parse_type(&w[2]), parse_type(&w[3])) {
                        (Some(ct), Some(it)) => Property { name: w[4].clone(), ty: it, list: true, count_type: ct },
                        _ => {
                            h.error = format!("unknown property type in \"{line}\"");
                            return h;
                        }
                    }
                } else if w.len() >= 3 {
                    match parse_type(&w[1]) {
                        Some(t) => Property { name: w[2].clone(), ty: t, list: false, count_type: Type::U8 },
                        None => {
                            h.error = format!("unknown property type in \"{line}\"");
                            return h;
                        }
                    }
                } else {
                    h.error = format!("bad property line \"{line}\"");
                    return h;
                };
                h.elements.last_mut().unwrap().props.push(p);
            }
            _ => {
                h.error = format!("unexpected header line \"{line}\"");
                return h;
            }
        }
    }
    h.error = "no end_header".into();
    h
}

/// Values of one property, rounded through float unless the type needs double to stay exact.
#[derive(Default)]
struct Column {
    wide: bool,
    v: Vec<f64>,
}

impl Column {
    fn with_len(n: usize, t: Type) -> Self {
        Self { wide: wide_type(t), v: vec![0.0; n] }
    }
    fn get(&self, k: usize) -> f64 {
        self.v[k]
    }
}

/// One element's values: scalars as columns, lists flattened with offsets. Only wanted properties are kept.
#[derive(Default)]
struct Columns {
    scalar: Vec<Column>,
    list_values: Vec<Column>,
    /// count + 1 per list property kept.
    list_offsets: Vec<Vec<u64>>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Pass {
    /// Wanted scalars, and list lengths into list_offsets.
    Scalars,
    /// Wanted list values, at the offsets the lengths summed to.
    Lists,
    Skip,
}

/// Writable views of a Columns, for records decoded in parallel (each record writes its own slots).
struct ColumnsView<'a> {
    scalar: Vec<(bool, Disjoint<'a, f64>)>,
    list_values: Vec<(bool, Disjoint<'a, f64>)>,
    list_offsets: Vec<Disjoint<'a, u64>>,
}

impl<'a> ColumnsView<'a> {
    fn new(c: &'a mut Columns) -> Self {
        Self {
            scalar: c.scalar.iter_mut().map(|x| (x.wide, Disjoint::new(&mut x.v))).collect(),
            list_values: c.list_values.iter_mut().map(|x| (x.wide, Disjoint::new(&mut x.v))).collect(),
            list_offsets: c.list_offsets.iter_mut().map(|x| Disjoint::new(x)).collect(),
        }
    }
}

fn store(col: &(bool, Disjoint<f64>), k: usize, v: f64) {
    let v = if col.0 { v } else { v as f32 as f64 };
    // SAFETY: record k (or its list range) is decoded by one thread only.
    unsafe { col.1.write(k, v) };
}

fn read_record(e: &Element, want: &[bool], c: &mut RecordReader, k: usize, out: Option<&ColumnsView>, pass: Pass) -> bool {
    for (i, p) in e.props.iter().enumerate() {
        if !p.list {
            if pass == Pass::Scalars && want[i] {
                let v = c.value(p.ty);
                store(&out.unwrap().scalar[i], k, v);
            } else {
                c.skip(p.ty, 1);
            }
            continue;
        }
        let n = c.value(p.count_type);
        if !c.ok || !(0.0..=1e7).contains(&n) || n != n.floor() {
            return false;
        }
        let count = n as usize;
        if want[i] && pass == Pass::Scalars {
            // SAFETY: as above.
            unsafe { out.unwrap().list_offsets[i].write(k, count as u64) };
        } else if want[i] && pass == Pass::Lists {
            let o = out.unwrap();
            // SAFETY: offsets were summed before this pass and are only read.
            let at = unsafe { *o.list_offsets[i].read(k) } as usize;
            for j in 0..count {
                let v = c.value(p.ty);
                store(&o.list_values[i], at + j, v);
            }
            continue;
        }
        c.skip(p.ty, count);
    }
    c.ok
}

/// Where an element's records start in the body: every stride bytes from base, or listed with the end last.
#[derive(Default)]
struct Records<'a> {
    base: usize,
    stride: usize,
    table: Option<&'a [usize]>,
}

impl Records<'_> {
    fn start(&self, k: usize) -> usize {
        match self.table {
            Some(t) => t[k],
            None => self.base + k * self.stride,
        }
    }
}

/// Start of every line holding more than white space, then the body's end.
fn line_starts(body: &[u8]) -> Vec<usize> {
    let mut out = Vec::new();
    let mut at = 0;
    while at < body.len() {
        let mut q = at;
        while q < body.len() && body[q] != b'\n' && is_space(body[q]) {
            q += 1;
        }
        if q < body.len() && body[q] != b'\n' {
            out.push(at);
        }
        at = body[q..].iter().position(|&c| c == b'\n').map_or(body.len(), |p| q + p + 1);
    }
    out.push(body.len());
    out
}

/// Decodes an element's wanted properties from its records, in parallel.
fn read_element(body: &[u8], format: Format, e: &Element, rec: &Records, want: &[bool], out: &mut Columns) -> bool {
    let np = e.props.len();
    out.scalar = (0..np).map(|_| Column::default()).collect();
    out.list_values = (0..np).map(|_| Column::default()).collect();
    out.list_offsets = vec![Vec::new(); np];
    let mut lists = false;
    for i in 0..np {
        if !want[i] {
            continue;
        }
        if e.props[i].list {
            out.list_offsets[i] = vec![0; e.count + 1];
            lists = true;
        } else {
            out.scalar[i] = Column::with_len(e.count, e.props[i].ty);
        }
    }
    let run = |out: &mut Columns, pass: Pass| -> bool {
        let view = ColumnsView::new(out);
        let bad = AtomicBool::new(false);
        (0..e.count).into_par_iter().with_min_len(4096).for_each(|k| {
            if bad.load(Ordering::Relaxed) {
                return;
            }
            let mut c = RecordReader::new(body, rec.start(k), rec.start(k + 1), format);
            if !read_record(e, want, &mut c, k, Some(&view), pass) {
                bad.store(true, Ordering::Relaxed);
            }
        });
        !bad.load(Ordering::Relaxed)
    };
    if !run(out, Pass::Scalars) {
        return false;
    }
    if !lists {
        return true;
    }
    for i in 0..np {
        if out.list_offsets[i].is_empty() {
            continue;
        }
        let mut sum = 0u64;
        for o in out.list_offsets[i].iter_mut() {
            let n = *o;
            *o = sum;
            sum += n;
        }
        out.list_values[i] = Column::with_len(sum as usize, e.props[i].ty);
    }
    run(out, Pass::Lists)
}

/// A binary element's records from byte `at` of the body. Records without lists, or whose lists all have the first
/// record's lengths (a triangle mesh), sit at a fixed stride; otherwise each start is found by walking them.
fn binary_records(body: &[u8], format: Format, e: &Element, at: usize, table: &mut Vec<usize>) -> Option<(usize, usize, bool)> {
    if e.count == 0 {
        return Some((at, 0, false));
    }
    let nothing = vec![false; e.props.len()];
    // Byte ranges of the first record's list lengths.
    let mut lengths = Vec::new();
    let mut first = RecordReader::new(body, at, body.len(), format);
    for p in &e.props {
        if !first.ok {
            break;
        }
        if !p.list {
            first.skip(p.ty, 1);
            continue;
        }
        let offset = first.at - at;
        let n = first.value(p.count_type);
        if !(0.0..=1e7).contains(&n) {
            return None;
        }
        lengths.push((offset, type_size(p.count_type)));
        first.skip(p.ty, n as usize);
    }
    if !first.ok {
        return None;
    }
    let stride = first.at - at;
    let mut fixed = stride > 0 && (body.len() - at) / stride >= e.count;
    if fixed && !lengths.is_empty() {
        let differs = (1..e.count).into_par_iter().with_min_len(16384).any(|k| {
            lengths.iter().any(|&(offset, size)| {
                body[at + offset..at + offset + size] != body[at + k * stride + offset..at + k * stride + offset + size]
            })
        });
        fixed = !differs;
    }
    if fixed {
        return Some((at, stride, false));
    }
    table.resize(e.count + 1, 0);
    let mut c = RecordReader::new(body, at, body.len(), format);
    for k in 0..e.count {
        table[k] = c.at;
        if !read_record(e, &nothing, &mut c, k, None, Pass::Skip) {
            return None;
        }
    }
    table[e.count] = c.at;
    Some((at, 0, true))
}

/// An ASCII element's records walked one by one, for files that break records across lines.
fn ascii_records(body: &[u8], e: &Element, at: usize, table: &mut Vec<usize>) -> bool {
    let nothing = vec![false; e.props.len()];
    table.resize(e.count + 1, 0);
    let mut c = RecordReader::new(body, at, body.len(), Format::Ascii);
    for k in 0..e.count {
        table[k] = c.at;
        if !read_record(e, &nothing, &mut c, k, None, Pass::Skip) {
            return false;
        }
    }
    table[e.count] = c.at;
    true
}

fn is_builtin_vertex_property(n: &str) -> bool {
    const KNOWN: &[&str] = &[
        "x", "y", "z", "nx", "ny", "nz", "red", "green", "blue", "alpha", "r", "g", "b", "a", "diffuse_red",
        "diffuse_green", "diffuse_blue", "diffuse_alpha", "s", "t", "u", "v", "texture_u", "texture_v", "texture_s",
        "texture_t",
    ];
    KNOWN.contains(&n)
}

fn is_builtin_face_property(n: &str) -> bool {
    const KNOWN: &[&str] =
        &["vertex_indices", "vertex_index", "texcoord", "red", "green", "blue", "alpha", "r", "g", "b", "a"];
    KNOWN.contains(&n)
}

/// A property name usable as an attribute, prefixed when it would collide with a built-in layer.
fn extra_attribute_name(name: &str) -> String {
    if name == names::NORMAL
        || name == names::COLOR
        || name == names::MATERIAL
        || name == names::HARD
        || name.starts_with("uv")
        || name.starts_with(colors::LAYER_PREFIX)
    {
        format!("ply_{name}")
    } else {
        name.to_string()
    }
}

#[derive(Clone, Copy)]
struct ColorProps {
    r: Option<usize>,
    g: Option<usize>,
    b: Option<usize>,
    a: Option<usize>,
}

impl ColorProps {
    fn any(&self) -> bool {
        self.r.is_some() && self.g.is_some() && self.b.is_some()
    }
}

fn color_props(e: &Element) -> ColorProps {
    ColorProps {
        r: e.find(&["red", "r", "diffuse_red"]),
        g: e.find(&["green", "g", "diffuse_green"]),
        b: e.find(&["blue", "b", "diffuse_blue"]),
        a: e.find(&["alpha", "a", "diffuse_alpha"]),
    }
}

fn color_at(e: &Element, c: &Columns, p: &ColorProps, k: usize) -> [f32; 4] {
    let ch = |i: usize| c.scalar[i].get(k) as f32 * channel_scale(e.props[i].ty);
    [
        srgb_to_linear(ch(p.r.unwrap())),
        srgb_to_linear(ch(p.g.unwrap())),
        srgb_to_linear(ch(p.b.unwrap())),
        p.a.map_or(1.0, |a| ch(a).clamp(0.0, 1.0)),
    ]
}

/// Reads every element's wanted properties. Returns the element that failed, or nothing.
fn read_body(body: &[u8], h: &Header, want: &[Vec<bool>], into: &mut [Option<&mut Columns>]) -> Option<String> {
    let format = h.format.unwrap_or(Format::Ascii);
    let mut by_line = format == Format::Ascii;
    let lines = if by_line { line_starts(body) } else { Vec::new() };
    // ASCII writers put one record per line; when a file doesn't, its records are walked instead.
    loop {
        let mut at = 0;
        let mut line = 0;
        let mut failed = None;
        for (i, e) in h.elements.iter().enumerate() {
            let mut table = Vec::new();
            let mut rec = Records::default();
            let mut ok;
            if format != Format::Ascii {
                match binary_records(body, format, e, at, &mut table) {
                    Some((base, stride, tabled)) => {
                        ok = true;
                        rec.base = base;
                        rec.stride = stride;
                        if tabled {
                            rec.table = Some(&table);
                        }
                    }
                    None => ok = false,
                }
            } else if by_line {
                ok = line + e.count < lines.len();
                if ok {
                    rec.table = Some(&lines[line..line + e.count + 1]);
                }
                line += e.count;
            } else {
                ok = ascii_records(body, e, at, &mut table);
                rec.table = Some(&table);
            }
            if ok {
                if let Some(out) = into[i].as_deref_mut() {
                    ok = read_element(body, format, e, &rec, &want[i], out);
                }
            }
            if ok {
                at = rec.start(e.count);
            } else {
                failed = Some(e.name.clone());
                break;
            }
        }
        if failed.is_none() || !by_line {
            return failed;
        }
        by_line = false;
    }
}

/// File value to a point index, INVALID when it isn't one.
fn point_index(v: f64, points: usize) -> Index {
    if v >= 0.0 && v < points as f64 {
        v as Index
    } else {
        INVALID
    }
}

fn has_repeated_point(pts: &[Index]) -> bool {
    if pts.len() > 16 {
        let mut sorted = pts.to_vec();
        sorted.sort_unstable();
        return sorted.windows(2).any(|w| w[0] == w[1]);
    }
    (0..pts.len()).any(|i| (i + 1..pts.len()).any(|j| pts[i] == pts[j]))
}

fn bad_face(pts: &[Index]) -> bool {
    pts.len() < 3 || pts.contains(&INVALID) || has_repeated_point(pts)
}

pub fn read_ply(data: &[u8], opts: &PlyReadOptions) -> PlyReadResult {
    let mut r = PlyReadResult::default();
    let h = parse_header(data);
    if !h.error.is_empty() {
        r.error = h.error;
        return r;
    }
    r.texture_files = h.texture_files.clone();

    let mut vertex_k = None;
    let mut face_k = None;
    let mut edge_k = None;
    let mut want: Vec<Vec<bool>> = Vec::with_capacity(h.elements.len());
    for (k, e) in h.elements.iter().enumerate() {
        let mut w = vec![false; e.props.len()];
        if e.name == "vertex" && vertex_k.is_none() {
            vertex_k = Some(k);
            for (i, p) in e.props.iter().enumerate() {
                w[i] = !p.list;
            }
        } else if e.name == "face" && face_k.is_none() {
            face_k = Some(k);
            for (i, p) in e.props.iter().enumerate() {
                w[i] = if p.list {
                    p.name == "vertex_indices" || p.name == "vertex_index" || p.name == "texcoord"
                } else {
                    true
                };
                if p.list && !w[i] {
                    r.skipped.push(format!("face property {}", p.name));
                }
            }
        } else if e.name == "edge" && edge_k.is_none() {
            edge_k = Some(k);
            for (i, p) in e.props.iter().enumerate() {
                w[i] = p.name == "vertex1" || p.name == "vertex2";
            }
        } else if e.count > 0 {
            r.skipped.push(format!("element {}", e.name));
        }
        want.push(w);
    }
    let mut vertex = Columns::default();
    let mut face = Columns::default();
    let mut edge = Columns::default();
    {
        let mut into: Vec<Option<&mut Columns>> = (0..h.elements.len()).map(|_| None).collect();
        let (mut vr, mut fr, mut er) = (Some(&mut vertex), Some(&mut face), Some(&mut edge));
        for (k, slot) in into.iter_mut().enumerate() {
            if Some(k) == vertex_k {
                *slot = vr.take();
            } else if Some(k) == face_k {
                *slot = fr.take();
            } else if Some(k) == edge_k {
                *slot = er.take();
            }
        }
        if let Some(failed) = read_body(&data[h.body_start..], &h, &want, &mut into) {
            r.error = format!("truncated or malformed data in element \"{failed}\"");
            return r;
        }
    }
    let Some(vk) = vertex_k else {
        r.error = "no vertex element".into();
        return r;
    };
    let vertex_el = &h.elements[vk];
    let (Some(px), Some(py), Some(pz)) = (vertex_el.find(&["x"]), vertex_el.find(&["y"]), vertex_el.find(&["z"])) else {
        r.error = "vertex element has no x, y, z".into();
        return r;
    };

    let face_el = face_k.map(|k| &h.elements[k]);
    let mut kept: Vec<usize> = Vec::new();
    let nv = vertex_el.count;
    let f_idx = face_el.and_then(|f| f.find(&["vertex_indices", "vertex_index"]));
    let nf = if f_idx.is_some() { face_el.unwrap().count } else { 0 };
    let mut m = {
        let (x, y, z) = (&vertex.scalar[px], &vertex.scalar[py], &vertex.scalar[pz]);
        let points: Vec<DVec3> = (0..nv).into_par_iter().map(|k| DVec3::new(x.get(k), y.get(k), z.get(k))).collect();
        let mut sizes: Vec<Index> = Vec::new();
        let mut indices: Vec<Index> = Vec::new();
        if let Some(fi) = f_idx {
            let idx = &face.list_values[fi];
            let off = &face.list_offsets[fi];
            sizes = (0..nf).map(|f| (off[f + 1] - off[f]) as Index).collect();
            indices = (0..idx.v.len()).into_par_iter().map(|j| point_index(idx.get(j), nv)).collect();
            let face_pts = |f: usize, _: &[Index]| -> std::ops::Range<usize> { off[f] as usize..off[f + 1] as usize };
            let any_bad = (0..nf).into_par_iter().any(|f| bad_face(&indices[face_pts(f, &indices)]));
            // Faces from_polygons would refuse are dropped here, so the rest keep their order and kept maps them to
            // the file's.
            if any_bad {
                let mut good_sizes = Vec::new();
                let mut good_indices = Vec::new();
                for f in 0..nf {
                    let pts = &indices[face_pts(f, &indices)];
                    if bad_face(pts) {
                        continue;
                    }
                    kept.push(f);
                    good_sizes.push(sizes[f]);
                    good_indices.extend_from_slice(pts);
                }
                r.skipped.push(format!("{} bad faces", nf - kept.len()));
                sizes = good_sizes;
                indices = good_indices;
            }
        }
        Mesh::from_polygons(&points, &sizes, &indices)
    };
    let source = |f: Index| if kept.is_empty() { f as usize } else { kept[f as usize] };
    if let Some(ek) = edge_k {
        let edge_el = &h.elements[ek];
        if let (Some(a), Some(b)) = (edge_el.find(&["vertex1"]), edge_el.find(&["vertex2"])) {
            for k in 0..edge_el.count {
                let pa = point_index(edge.scalar[a].get(k), nv);
                let pb = point_index(edge.scalar[b].get(k), nv);
                if pa != INVALID && pb != INVALID && pa != pb {
                    m.add_edge(pa, pb);
                }
            }
        }
    }
    r.point_cloud = m.num_faces() == 0 && m.num_edges() == 0;

    let (nx, ny, nz) = (vertex_el.find(&["nx"]), vertex_el.find(&["ny"]), vertex_el.find(&["nz"]));
    let has_normals = opts.keep_normals && nx.is_some() && ny.is_some() && nz.is_some();
    let vcol = color_props(vertex_el);
    let ps = vertex_el.find(&["s", "u", "texture_u", "texture_s"]);
    let pt = vertex_el.find(&["t", "v", "texture_v", "texture_t"]);
    let has_uv = ps.is_some() && pt.is_some();
    let normal_at = |k: usize| {
        [
            vertex.scalar[nx.unwrap()].get(k) as f32,
            vertex.scalar[ny.unwrap()].get(k) as f32,
            vertex.scalar[nz.unwrap()].get(k) as f32,
        ]
    };
    let uv_at = |k: usize| [vertex.scalar[ps.unwrap()].get(k) as f32, vertex.scalar[pt.unwrap()].get(k) as f32];

    for (i, p) in vertex_el.props.iter().enumerate() {
        if p.list {
            r.skipped.push(format!("vertex property {}", p.name));
            continue;
        }
        if is_builtin_vertex_property(&p.name) {
            continue;
        }
        let layer = m.point_attrs_mut().get_or_add::<f32>(&extra_attribute_name(&p.name));
        layer.par_iter_mut().enumerate().for_each(|(k, v)| *v = vertex.scalar[i].get(k) as f32);
    }

    if m.num_faces() == 0 {
        if has_normals {
            let n = m.point_attrs_mut().get_or_add::<[f32; 3]>(names::NORMAL);
            n.par_iter_mut().enumerate().for_each(|(k, v)| *v = normal_at(k));
        }
        if vcol.any() {
            let c = m.point_attrs_mut().get_or_add::<[f32; 4]>(names::COLOR);
            c.par_iter_mut().enumerate().for_each(|(k, v)| *v = color_at(vertex_el, &vertex, &vcol, k));
        }
        if has_uv {
            let uv = m.point_attrs_mut().get_or_add::<[f32; 2]>(&names::uv(0));
            uv.par_iter_mut().enumerate().for_each(|(k, v)| *v = uv_at(k));
        }
        r.mesh = m;
        return r;
    }
    let face_el = face_el.unwrap();

    // Faces were added in order without deletions, so face and corner ids are contiguous.
    let corner_point: Vec<Index> = (0..m.corner_capacity()).map(|c| m.corner_point(c)).collect();
    if has_normals {
        let n = m.corner_attrs_mut().get_or_add::<[f32; 3]>(names::NORMAL);
        n.par_iter_mut().enumerate().for_each(|(c, v)| *v = normal_at(corner_point[c] as usize));
    }
    let tc = face_el.find(&["texcoord"]);
    if let Some(tc) = tc.filter(|&t| face_el.props[t].list) {
        let v = &face.list_values[tc];
        let off = &face.list_offsets[tc];
        let mut vals = vec![None; m.corner_capacity() as usize];
        for f in 0..m.face_capacity() {
            let src = source(f);
            if off[src + 1] - off[src] != m.face_size(f) as u64 * 2 {
                continue;
            }
            let mut j = off[src] as usize;
            m.for_each_face_corner(f, |c| {
                vals[c as usize] = Some([v.get(j) as f32, v.get(j + 1) as f32]);
                j += 2;
            });
        }
        let uv = m.corner_attrs_mut().get_or_add::<[f32; 2]>(&names::uv(0));
        for (d, s) in uv.iter_mut().zip(vals) {
            if let Some(s) = s {
                *d = s;
            }
        }
        mark_uv_seams(&mut m, &names::uv(0));
    } else if has_uv {
        let uv = m.corner_attrs_mut().get_or_add::<[f32; 2]>(&names::uv(0));
        uv.par_iter_mut().enumerate().for_each(|(c, v)| *v = uv_at(corner_point[c] as usize));
    }
    if vcol.any() {
        let point_colors: Vec<[f32; 4]> =
            (0..nv).into_par_iter().map(|k| color_at(vertex_el, &vertex, &vcol, k)).collect();
        colors::add_white(&mut m, "colorSet");
        colors::set_from_points(&mut m, "colorSet", &point_colors);
    }
    let fcol = color_props(face_el);
    if fcol.any() {
        let set = if vcol.any() { "faceColor" } else { "colorSet" };
        let face_of: Vec<Index> = (0..m.corner_capacity()).map(|c| m.corner_face(c)).collect();
        let face_color: Vec<[f32; 4]> =
            (0..m.face_capacity()).into_par_iter().map(|f| color_at(face_el, &face, &fcol, source(f))).collect();
        let col = colors::add_white(&mut m, set);
        col.par_iter_mut().enumerate().for_each(|(c, v)| *v = face_color[face_of[c] as usize]);
    }
    for (i, p) in face_el.props.iter().enumerate() {
        if p.list || is_builtin_face_property(&p.name) {
            continue;
        }
        let layer = m.face_attrs_mut().get_or_add::<f32>(&extra_attribute_name(&p.name));
        layer.par_iter_mut().enumerate().for_each(|(f, v)| *v = face.scalar[i].get(source(f as Index)) as f32);
    }

    // Points no face or edge uses.
    let loose: Vec<Index> = (0..m.point_capacity()).filter(|&p| m.point_edge(p) == INVALID).collect();
    if !loose.is_empty() {
        m.remove_loose(&[], &loose);
        m.compact(None);
    }
    r.mesh = m;
    r
}

pub fn read_ply_file(path: &str, opts: &PlyReadOptions) -> PlyReadResult {
    match std::fs::read(path) {
        Ok(d) => read_ply(&d, opts),
        Err(_) => PlyReadResult { error: format!("cannot open {path}"), ..Default::default() },
    }
}

enum Val {
    F(f32),
    B(u8),
}

/// A written vertex: its point and the attributes it carries (normal, uv and color bits), compared bit for bit.
type VertexKey = (Index, [u32; 9]);

/// Points are split where the written corner attributes differ (UV seams, hard edges when writing normals), since PLY
/// holds them per vertex. Loose points and edges are written too, so point clouds and wires round-trip.
pub fn write_ply(mesh: &Mesh, opts: &PlyWriteOptions) -> Vec<u8> {
    let computed: Vec<[f32; 3]>;
    let mut normals: &[[f32; 3]] = &[];
    if opts.normals && mesh.num_faces() > 0 {
        normals = mesh.corner_attrs().get::<[f32; 3]>(names::NORMAL);
        if normals.is_empty() {
            computed = corner_normals(mesh, &CornerNormalOptions::default()).iter().map(|n| n.to_array()).collect();
            normals = &computed;
        }
    }
    let uvs: &[[f32; 2]] = if opts.uvs { mesh.corner_attrs().get(&names::uv(0)) } else { &[] };
    let set = if opts.colors { colors::current(mesh) } else { String::new() };
    let cols: &[[f32; 4]] = if set.is_empty() { &[] } else { colors::values(mesh, &set) };
    // Point clouds carry their attributes per point.
    let point_normals: &[[f32; 3]] = if opts.normals { mesh.point_attrs().get(names::NORMAL) } else { &[] };
    let point_cols: &[[f32; 4]] = if opts.colors { mesh.point_attrs().get(names::COLOR) } else { &[] };
    let point_uvs: &[[f32; 2]] = if opts.uvs { mesh.point_attrs().get(&names::uv(0)) } else { &[] };
    let write_normals = !normals.is_empty() || !point_normals.is_empty();
    let write_uvs = !uvs.is_empty() || !point_uvs.is_empty();
    let write_colors = !cols.is_empty() || !point_cols.is_empty();

    // Corners in write order, faces in id order, then the points no face uses.
    let mut faces = Vec::with_capacity(mesh.num_faces() as usize);
    let mut max_face = 0usize;
    for f in 0..mesh.face_capacity() {
        if mesh.face_valid(f) {
            faces.push(f);
            max_face = max_face.max(mesh.face_size(f) as usize);
        }
    }
    let mut face_start = vec![0usize; faces.len() + 1];
    for (i, &f) in faces.iter().enumerate() {
        face_start[i + 1] = face_start[i] + mesh.face_size(f) as usize;
    }
    let corner_count = face_start[faces.len()];
    let used: Vec<AtomicU8> = (0..mesh.point_capacity()).map(|_| AtomicU8::new(0)).collect();
    let corners: Vec<Index> = faces
        .par_iter()
        .flat_map_iter(|&f| {
            let cs = mesh.face_corners(f);
            for &c in &cs {
                used[mesh.corner_point(c) as usize].store(1, Ordering::Relaxed);
            }
            cs
        })
        .collect();
    let unused: Vec<Index> = (0..mesh.point_capacity())
        .filter(|&p| mesh.point_valid(p) && used[p as usize].load(Ordering::Relaxed) == 0)
        .collect();

    let key_for = |p: Index, c: Index| -> VertexKey {
        let mut n = [0.0f32; 3];
        let mut uv = [0.0f32; 2];
        let mut col = [0.0f32; 4];
        if write_normals {
            n = if c != INVALID && !normals.is_empty() {
                normals[c as usize]
            } else if !point_normals.is_empty() {
                point_normals[p as usize]
            } else {
                [0.0; 3]
            };
        }
        if write_uvs {
            uv = if c != INVALID && !uvs.is_empty() {
                uvs[c as usize]
            } else if !point_uvs.is_empty() {
                point_uvs[p as usize]
            } else {
                [0.0; 2]
            };
        }
        if write_colors {
            col = if c != INVALID && !cols.is_empty() {
                cols[c as usize]
            } else if !point_cols.is_empty() {
                point_cols[p as usize]
            } else {
                [1.0; 4]
            };
        }
        let mut a = [0u32; 9];
        for k in 0..3 {
            a[k] = n[k].to_bits();
        }
        a[3] = uv[0].to_bits();
        a[4] = uv[1].to_bits();
        for k in 0..4 {
            a[5 + k] = col[k].to_bits();
        }
        (p, a)
    };
    let keys: Vec<VertexKey> = (0..corner_count + unused.len())
        .into_par_iter()
        .map(|i| {
            if i < corner_count {
                key_for(mesh.corner_point(corners[i]), corners[i])
            } else {
                key_for(unused[i - corner_count], INVALID)
            }
        })
        .collect();
    let (vertex_of, first_key) = number_distinct(&keys);
    let vertex_count = first_key.len();
    // A point's vertex, for loose edges: the first written for it.
    let mut point_vertex = vec![u32::MAX; mesh.point_capacity() as usize];
    for (i, k) in keys.iter().enumerate() {
        if point_vertex[k.0 as usize] == u32::MAX {
            point_vertex[k.0 as usize] = vertex_of[i];
        }
    }
    let mut loose_edges = Vec::new();
    for e in 0..mesh.edge_capacity() {
        if mesh.edge_valid(e) && mesh.edge_corner(e) == INVALID {
            loose_edges.push((point_vertex[mesh.edge_point(e, 0) as usize], point_vertex[mesh.edge_point(e, 1) as usize]));
        }
    }

    let mut s = String::from("ply\nformat ");
    s += if opts.binary { "binary_little_endian 1.0\n" } else { "ascii 1.0\n" };
    s += "comment Aztek\n";
    s += &format!("element vertex {vertex_count}\n");
    s += "property float x\nproperty float y\nproperty float z\n";
    if write_normals {
        s += "property float nx\nproperty float ny\nproperty float nz\n";
    }
    if write_uvs {
        s += "property float s\nproperty float t\n";
    }
    if write_colors {
        s += "property uchar red\nproperty uchar green\nproperty uchar blue\nproperty uchar alpha\n";
    }
    let wide_count = max_face > 255;
    if !faces.is_empty() {
        s += &format!("element face {}\n", faces.len());
        s += if wide_count { "property list int int vertex_indices\n" } else { "property list uchar int vertex_indices\n" };
    }
    if !loose_edges.is_empty() {
        s += &format!("element edge {}\nproperty int vertex1\nproperty int vertex2\n", loose_edges.len());
    }
    s += "end_header\n";

    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    let vertex_values = |v: usize, out: &mut dyn FnMut(Val)| {
        let k = &keys[first_key[v] as usize];
        let p = mesh.position(k.0);
        for i in 0..3 {
            out(Val::F(p[i] as f32));
        }
        let a: [f32; 9] = k.1.map(f32::from_bits);
        if write_normals {
            for &x in &a[..3] {
                out(Val::F(x));
            }
        }
        if write_uvs {
            out(Val::F(a[3]));
            out(Val::F(a[4]));
        }
        if write_colors {
            for i in 0..3 {
                out(Val::B(byte(linear_to_srgb(a[5 + i]))));
            }
            out(Val::B(byte(a[8])));
        }
    };

    if opts.binary {
        let stride = 12 + if write_normals { 12 } else { 0 } + if write_uvs { 8 } else { 0 } + if write_colors { 4 } else { 0 };
        let count_bytes = if wide_count { 4 } else { 1 };
        let head = s.len();
        let faces_at = head + vertex_count * stride;
        let edges_at = faces_at + faces.len() * count_bytes + corner_count * 4;
        let mut out = s.into_bytes();
        out.resize(edges_at + loose_edges.len() * 8, 0);
        let (head_part, rest) = out.split_at_mut(faces_at);
        head_part[head..].par_chunks_mut(stride).enumerate().for_each(|(v, chunk)| {
            let mut at = 0;
            vertex_values(v, &mut |x| match x {
                Val::F(f) => {
                    chunk[at..at + 4].copy_from_slice(&f.to_le_bytes());
                    at += 4;
                }
                Val::B(b) => {
                    chunk[at] = b;
                    at += 1;
                }
            });
        });
        let (face_part, edge_part) = rest.split_at_mut(edges_at - faces_at);
        {
            let fp = Disjoint::new(face_part);
            (0..faces.len()).into_par_iter().for_each(|i| {
                let mut at = i * count_bytes + face_start[i] * 4;
                let n = face_start[i + 1] - face_start[i];
                let mut put = |b: &[u8]| {
                    for &x in b {
                        // SAFETY: each face owns its byte range.
                        unsafe { fp.write(at, x) };
                        at += 1;
                    }
                };
                if wide_count {
                    put(&(n as i32).to_le_bytes());
                } else {
                    put(&[n as u8]);
                }
                for j in face_start[i]..face_start[i + 1] {
                    put(&(vertex_of[j] as i32).to_le_bytes());
                }
            });
        }
        for (k, &(a, b)) in loose_edges.iter().enumerate() {
            edge_part[k * 8..k * 8 + 4].copy_from_slice(&(a as i32).to_le_bytes());
            edge_part[k * 8 + 4..k * 8 + 8].copy_from_slice(&(b as i32).to_le_bytes());
        }
        return out;
    }
    let prec = opts.precision;
    s += &crate::io_common::format_blocks(vertex_count, |out, v| {
        let mut first = true;
        vertex_values(v, &mut |x| {
            if !first {
                out.push(' ');
            }
            first = false;
            match x {
                Val::F(f) => append_number(out, f, prec),
                Val::B(b) => out.push_str(&b.to_string()),
            }
        });
        out.push('\n');
    });
    s += &crate::io_common::format_blocks(faces.len(), |out, i| {
        out.push_str(&(face_start[i + 1] - face_start[i]).to_string());
        for j in face_start[i]..face_start[i + 1] {
            out.push(' ');
            out.push_str(&vertex_of[j].to_string());
        }
        out.push('\n');
    });
    for (a, b) in loose_edges {
        s += &format!("{a} {b}\n");
    }
    s.into_bytes()
}
