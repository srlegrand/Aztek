//! The reference's binary scene files (`.mb`), read by writing them out as `.ma` text for the `.ma` reader.
//!
//! An `.mb` file is an IFF tree. Groups (`FOR4`/`FOR8`, `LIS4`/`LIS8`, `CAT4`/`CAT8`, `PROP`/`PRO8`) hold a
//! four-character form and child chunks; leaves hold data. The `8` kinds have a 16-byte header (tag, 4 spare
//! bytes, 64-bit size) and pad leaf data to 8 bytes; the `4` kinds have an 8-byte header and pad to 4. Groups pad
//! to 4 either way. Numbers are big-endian.
//!
//! Under the `Maya` root: a `HEAD` group (version, units, fileInfo, plug-ins), `FREF`/`FRDI` (references and their
//! load state), one group per node whose form is the node type's four-byte id and whose first child (`CREA`) names
//! it, `SLCT` groups (`select -ne` on a node the file didn't make, then its values) and `CONS` (connections).
//!
//! Each value chunk's tag says the kind of data; its data is the attribute path, a flag byte, an optional size and
//! then the value, in internal units (centimetres, radians, ticks for times). The text written here says so with
//! `currentUnit -l centimeter -a radian`, and the caller gives the scene the file's own units afterwards.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::sync::OnceLock;

/// Four-character type ids, from maya-scenefile-parser (MIT, see `data/mb/LICENSE`).
const TYPE_IDS: &str = include_str!("../data/mb/typeids.dat");

/// Types later than that table, seen in files the reference wrote.
const MORE_TYPE_IDS: &[(&str, &str)] = &[("STDS", "standardSurface"), ("NGEI", "nodeGraphEditorInfo"), ("PSDM", "poseInterpolatorManager"), ("SDML", "shapeEditorManager")];

/// The default number of ticks in a second.
const TICKS: f64 = 141_120_000.0;

/// A binary file read into `.ma` text.
pub struct Converted {
    pub text: String,
    /// What couldn't be carried over.
    pub warnings: Vec<String>,
    /// The file's linear and angle units as it names them ("cm", "m", "deg", "rad"), for the scene to take on.
    pub linear: String,
    pub angle: String,
}

/// True for data that starts as a binary scene does.
pub fn is_binary(bytes: &[u8]) -> bool {
    bytes.starts_with(b"FOR4") || bytes.starts_with(b"FOR8")
}

/// The file's contents as `.ma` text.
pub fn convert(bytes: &[u8]) -> Result<Converted, String> {
    if !is_binary(bytes) {
        return Err("not a binary scene file".into());
    }
    let mut truncated = false;
    let top = children(bytes, bytes[3] == b'8', &mut truncated);
    let root = top.into_iter().find(|c| c.form.is_some()).ok_or("a binary scene file with nothing in it")?;
    if root.form != Some(*b"Maya") {
        return Err("a binary file, but not a scene".into());
    }
    let mut w = Writer::default();
    w.read(&root);
    if truncated || w.truncated {
        w.warnings.push("the file is cut short; what could be read was kept".into());
    }
    Ok(w.finish())
}

// ---------------------------------------------------------------------------------------------------------------------
// The IFF tree

const GROUPS: [&[u8; 4]; 8] = [b"FOR4", b"FOR8", b"LIS4", b"LIS8", b"CAT4", b"CAT8", b"PROP", b"PRO8"];

struct Chunk<'a> {
    tag: [u8; 4],
    /// A group's form; None for a leaf.
    form: Option<[u8; 4]>,
    /// A leaf's data, or a group's children.
    data: &'a [u8],
    /// Whether this chunk's children use the 16-byte header.
    wide: bool,
}

fn children<'a>(bytes: &'a [u8], wide: bool, truncated: &mut bool) -> Vec<Chunk<'a>> {
    let header = if wide { 16 } else { 8 };
    let mut out = Vec::new();
    let mut pos = 0usize;
    while pos + header <= bytes.len() {
        let tag: [u8; 4] = bytes[pos..pos + 4].try_into().unwrap_or_default();
        let size = if wide { u64::from_be_bytes(bytes[pos + 8..pos + 16].try_into().unwrap_or_default()) } else { u64::from(u32::from_be_bytes(bytes[pos + 4..pos + 8].try_into().unwrap_or_default())) };
        let body = pos + header;
        let size = usize::try_from(size).unwrap_or(usize::MAX);
        let end = body.saturating_add(size);
        if end > bytes.len() {
            *truncated = true;
        }
        let end = end.min(bytes.len());
        if GROUPS.contains(&&tag) {
            if end < body + 4 {
                *truncated = true;
                break;
            }
            let form: [u8; 4] = bytes[body..body + 4].try_into().unwrap_or_default();
            let wide = tag[3] == b'8' || &tag == b"PRO8";
            out.push(Chunk { tag, form: Some(form), data: &bytes[body + 4..end], wide });
            pos = body.saturating_add((size + 3) & !3);
        } else {
            out.push(Chunk { tag, form: None, data: &bytes[body..end], wide });
            let align = if wide { 8 } else { 4 };
            pos = body.saturating_add(size.div_ceil(align) * align);
        }
    }
    out
}

/// Reads big-endian values off the front of a slice. Reading past the end gives zeros and clears `ok`.
struct Cursor<'a> {
    b: &'a [u8],
    at: usize,
    ok: bool,
}

impl<'a> Cursor<'a> {
    fn new(b: &'a [u8]) -> Self {
        Cursor { b, at: 0, ok: true }
    }
    fn left(&self) -> usize {
        self.b.len().saturating_sub(self.at)
    }
    fn take(&mut self, n: usize) -> &'a [u8] {
        if self.left() < n {
            self.ok = false;
            self.at = self.b.len();
            return &[];
        }
        let s = &self.b[self.at..self.at + n];
        self.at += n;
        s
    }
    fn u8(&mut self) -> u8 {
        self.take(1).first().copied().unwrap_or(0)
    }
    fn u32(&mut self) -> u32 {
        self.take(4).try_into().map(u32::from_be_bytes).unwrap_or(0)
    }
    fn u64(&mut self) -> u64 {
        self.take(8).try_into().map(u64::from_be_bytes).unwrap_or(0)
    }
    fn f32(&mut self) -> f32 {
        f32::from_bits(self.u32())
    }
    fn f64(&mut self) -> f64 {
        f64::from_bits(self.u64())
    }
    /// A count of items of `size` bytes each; zero, and not ok, when there aren't that many bytes left.
    fn count(&mut self, size: usize) -> usize {
        let n = self.u32() as usize;
        if n.saturating_mul(size) > self.left() {
            self.ok = false;
            return 0;
        }
        n
    }
    /// A NUL-ended string (or the rest of the data when it has no NUL).
    fn str(&mut self) -> String {
        let rest = &self.b[self.at.min(self.b.len())..];
        let n = rest.iter().position(|&c| c == 0).unwrap_or(rest.len());
        if rest.is_empty() {
            self.ok = false;
        }
        self.at += (n + 1).min(rest.len());
        String::from_utf8_lossy(&rest[..n]).into_owned()
    }
}

fn tag_name(t: [u8; 4]) -> String {
    if t.iter().all(|c| c.is_ascii_graphic() || *c == b' ') {
        String::from_utf8_lossy(&t).trim_end().to_string()
    } else {
        format!("{:08X}", u32::from_be_bytes(t))
    }
}

fn type_table() -> &'static HashMap<[u8; 4], &'static str> {
    static TABLE: OnceLock<HashMap<[u8; 4], &'static str>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut m = HashMap::new();
        for line in TYPE_IDS.lines() {
            if let (Some(tag), Some(name)) = (line.get(..4), line.get(5..)) {
                if let Ok(t) = <[u8; 4]>::try_from(tag.as_bytes()) {
                    m.insert(t, name.trim());
                }
            }
        }
        // The matrix nodes' ids are numbers, not four letters.
        m.insert([0x58, 0x00, 0x03, 0x00], "decomposeMatrix");
        m.insert([0x58, 0x00, 0x03, 0x01], "composeMatrix");
        for (tag, name) in MORE_TYPE_IDS {
            if let Ok(t) = <[u8; 4]>::try_from(tag.as_bytes()) {
                m.insert(t, *name);
            }
        }
        m
    })
}

// ---------------------------------------------------------------------------------------------------------------------
// Writing .ma text

const TIME_UNITS: &[(&str, f64)] = &[("game", 15.0), ("film", 24.0), ("pal", 25.0), ("ntsc", 30.0), ("show", 48.0), ("palf", 50.0), ("ntscf", 60.0)];

fn fps_of(unit: &str) -> Option<f64> {
    TIME_UNITS.iter().find(|(n, _)| *n == unit).map(|(_, f)| *f).or_else(|| unit.strip_suffix("fps").and_then(|n| n.parse().ok()))
}

struct Reference {
    path: String,
    ns: String,
    node: String,
    ty: String,
}

#[derive(Default)]
struct Writer {
    version: String,
    linear: String,
    angle: String,
    time: String,
    ticks: f64,
    requires: Vec<String>,
    file_info: Vec<(String, String)>,
    refs: Vec<Reference>,
    unloaded: Vec<String>,
    plugin_types: HashMap<[u8; 4], String>,
    body: String,
    conns: String,
    warnings: Vec<String>,
    unknown_types: BTreeMap<String, usize>,
    unknown_values: BTreeMap<String, usize>,
    unknown_sections: BTreeMap<String, usize>,
    truncated: bool,
}

impl Writer {
    fn read(&mut self, root: &Chunk) {
        let kids = children(root.data, root.wide, &mut self.truncated);
        // The header and references come first in the text whatever their place in the file.
        for c in &kids {
            match c.form.as_ref() {
                Some(b"HEAD") => self.header(c),
                Some(b"FREF") => self.references(c),
                Some(b"FRDI") => self.load_state(c),
                _ => {}
            }
        }
        if self.ticks <= 0.0 {
            self.ticks = TICKS;
        }
        for c in &kids {
            match c.form.as_ref() {
                None | Some(b"HEAD") | Some(b"FREF") | Some(b"FRDI") => {}
                Some(b"SLCT") => self.select(c),
                Some(b"CONS") | Some(b"CONN") => self.connections(c),
                Some(form) => self.node(*form, c),
            }
        }
    }

    fn finish(mut self) -> Converted {
        let mut t = String::new();
        let _ = writeln!(t, "//Maya ASCII scene");
        let _ = writeln!(t, "//Converted from a binary scene file by Aztek");
        for r in &self.refs {
            let _ = writeln!(t, "file -rdi 1 -ns {} -rfn {} -typ {} {};", q(&r.ns), q(&r.node), q(&r.ty), q(&r.path));
        }
        for r in &self.refs {
            let dr = if self.unloaded.contains(&r.node) { " -dr 1" } else { "" };
            let _ = writeln!(t, "file -r -ns {}{dr} -rfn {} -typ {} {};", q(&r.ns), q(&r.node), q(&r.ty), q(&r.path));
        }
        let _ = writeln!(t, "requires maya {};", q(if self.version.is_empty() { "2026" } else { &self.version }));
        for r in &self.requires {
            let _ = writeln!(t, "{r}");
        }
        let time = if fps_of(&self.time).is_some() { self.time.clone() } else { "film".to_string() };
        let _ = writeln!(t, "currentUnit -l centimeter -a radian -t {time};");
        for (k, v) in &self.file_info {
            let _ = writeln!(t, "fileInfo {} {};", q(k), q(v));
        }
        t.push_str(&self.body);
        t.push_str(&self.conns);
        let _ = writeln!(t, "// End of converted scene");
        for (ty, n) in &self.unknown_types {
            self.warnings.push(format!("{n} node(s) of binary type id {ty} skipped: the type isn't known"));
        }
        for (ty, n) in &self.unknown_values {
            self.warnings.push(format!("{n} value(s) of binary kind {ty} skipped"));
        }
        for (ty, n) in &self.unknown_sections {
            self.warnings.push(format!("{n} section(s) {ty} skipped"));
        }
        let linear = if self.linear.is_empty() { "cm".into() } else { self.linear };
        let angle = if self.angle.is_empty() { "deg".into() } else { self.angle };
        Converted { text: t, warnings: self.warnings, linear, angle }
    }

    fn fps(&self) -> f64 {
        fps_of(&self.time).unwrap_or(24.0)
    }

    fn header(&mut self, c: &Chunk) {
        for k in children(c.data, c.wide, &mut self.truncated) {
            let mut cur = Cursor::new(k.data);
            match &k.tag {
                b"VERS" => self.version = cur.str(),
                b"LUNI" => self.linear = cur.str(),
                b"AUNI" => self.angle = cur.str(),
                b"TUNI" => self.time = cur.str(),
                b"TDUR" => self.ticks = cur.str().trim().parse().unwrap_or(TICKS),
                b"FINF" => {
                    let key = cur.str();
                    let value = cur.str();
                    if !key.is_empty() && !self.file_info.iter().any(|(k, _)| *k == key) {
                        self.file_info.push((key, value));
                    }
                }
                b"PLUG" => {
                    // Name, version, the requires flags, then (node type, id) pairs.
                    let name = cur.str();
                    let version = cur.str();
                    let flags = cur.str();
                    while cur.ok && cur.left() > 4 {
                        let ty = cur.str();
                        let id = cur.take(4);
                        if let (true, Ok(id)) = (!ty.is_empty(), <[u8; 4]>::try_from(id)) {
                            self.plugin_types.insert(id, ty);
                        }
                    }
                    if !name.is_empty() {
                        let flags = flags.trim();
                        let sp = if flags.is_empty() { "" } else { " " };
                        self.requires.push(format!("requires{sp}{flags} {} {};", q(&name), q(&version)));
                    }
                }
                _ => {}
            }
        }
    }

    fn references(&mut self, c: &Chunk) {
        for k in children(c.data, c.wide, &mut self.truncated) {
            if &k.tag != b"FREF" {
                continue;
            }
            let parts: Vec<String> = k.data.split(|&b| b == 0).map(|s| String::from_utf8_lossy(s).into_owned()).collect();
            let Some(path) = parts.first().filter(|p| !p.is_empty()) else { continue };
            let ns = parts.get(1).cloned().unwrap_or_default();
            // Flag bytes run straight into the reference node's name.
            let node = parts.get(2).map(|s| s.trim_start_matches(|c: char| (c as u32) < 32).to_string()).unwrap_or_default();
            let ty = parts.iter().skip(3).rev().find(|s| matches!(s.as_str(), "mayaAscii" | "mayaBinary")).cloned().unwrap_or_else(|| if path.to_ascii_lowercase().ends_with(".mb") { "mayaBinary".into() } else { "mayaAscii".into() });
            self.refs.push(Reference { path: path.clone(), ns, node, ty });
        }
    }

    /// Depth, path, namespace, two flag bytes (the second set when unloaded) and the reference node.
    fn load_state(&mut self, c: &Chunk) {
        for k in children(c.data, c.wide, &mut self.truncated) {
            if &k.tag != b"FRDI" {
                continue;
            }
            let mut cur = Cursor::new(k.data);
            let depth = cur.u32();
            cur.str();
            cur.str();
            cur.u8();
            let unloaded = cur.u8() != 0;
            let node = cur.str();
            if cur.ok && depth == 1 && unloaded {
                self.unloaded.push(node);
            }
        }
    }

    fn select(&mut self, c: &Chunk) {
        let kids = children(c.data, c.wide, &mut self.truncated);
        let Some(first) = kids.first().filter(|k| &k.tag == b"SLCT") else { return };
        let name = String::from_utf8_lossy(first.data).trim_end_matches('\0').to_string();
        if name.is_empty() {
            return;
        }
        let _ = writeln!(self.body, "select -ne {name};");
        for k in &kids[1..] {
            self.value(k, "");
        }
    }

    fn connections(&mut self, c: &Chunk) {
        for k in children(c.data, c.wide, &mut self.truncated) {
            if k.form.is_some() {
                self.connections(&k);
                continue;
            }
            let mut cur = Cursor::new(k.data);
            match &k.tag {
                b"CWFL" => {
                    let flags = cur.u8();
                    let src = cur.str();
                    let dst = cur.str();
                    if cur.ok {
                        let na = if flags & 1 != 0 { " -na" } else { "" };
                        let _ = writeln!(self.conns, "connectAttr {} {}{na};", q(&src), q(&dst));
                    }
                }
                b"RELA" => {
                    let kind = cur.str();
                    let node = cur.str();
                    let n = cur.count(1);
                    let mut line = format!("relationship {} {}", q(&kind), q(&node));
                    for _ in 0..n {
                        let _ = write!(line, " {}", q(&cur.str()));
                    }
                    if cur.ok {
                        let _ = writeln!(self.conns, "{line};");
                    }
                }
                other => *self.unknown_values.entry(tag_name(*other)).or_default() += 1,
            }
        }
    }

    fn node(&mut self, form: [u8; 4], c: &Chunk) {
        let kids = children(c.data, c.wide, &mut self.truncated);
        let Some(crea) = kids.first().filter(|k| &k.tag == b"CREA") else {
            *self.unknown_sections.entry(tag_name(form)).or_default() += 1;
            return;
        };
        let mut cur = Cursor::new(crea.data);
        let flags = cur.u8();
        let name = cur.str();
        let parent = if flags & 0x80 == 0 && cur.left() > 16 { Some(cur.str()) } else { None };
        let uid = (cur.left() >= 16).then(|| uuid(cur.take(16)));
        let known = self.plugin_types.get(&form).cloned().or_else(|| type_table().get(&form).map(|s| s.to_string()));
        let (ty, keep_values) = match known {
            Some(t) => (t, true),
            None => {
                *self.unknown_types.entry(tag_name(form)).or_default() += 1;
                // A transform stands in for an unknown one, so what is under it isn't lost.
                if flags & 0x08 != 0 {
                    ("transform".to_string(), false)
                } else {
                    return;
                }
            }
        };
        let shared = if flags & 0x01 != 0 { " -s" } else { "" };
        let mut line = format!("createNode {ty}{shared} -n {}", q(&name));
        if let Some(p) = parent.filter(|p| !p.is_empty()) {
            let _ = write!(line, " -p {}", q(&p));
        }
        let _ = writeln!(self.body, "{line};");
        if let Some(u) = uid {
            let _ = writeln!(self.body, "\trename -uid {};", q(&u));
        }
        if keep_values {
            for k in &kids[1..] {
                self.value(k, &ty);
            }
        }
    }

    /// One attribute chunk on the current node, as setAttr or addAttr.
    fn value(&mut self, k: &Chunk, node_type: &str) {
        if k.form.is_some() {
            *self.unknown_sections.entry(tag_name(k.form.unwrap_or_default())).or_default() += 1;
            return;
        }
        let mut c = Cursor::new(k.data);
        match &k.tag {
            b"ATTR" => return self.add_attr(&mut c),
            b"REFE" => return self.ref_edits(&mut c),
            _ => {}
        }
        let attr = c.str();
        let flags = c.u8();
        let size = if flags & 0x08 != 0 { Some(c.u32()) } else { None };
        if !c.ok || attr.is_empty() {
            return;
        }
        let mut head = String::from("\tsetAttr");
        if flags & 0x01 != 0 {
            head.push_str(" -l on");
        }
        // Visibility is keyable by default; the bit is clear where it was made not keyable.
        if attr == "v" && flags & 0x04 == 0 && flags & 0x20 != 0 {
            head.push_str(" -k off");
        }
        if let Some(n) = size {
            let _ = write!(head, " -s {n}");
        }
        let plug = q(&format!(".{attr}"));
        let mut vals = String::new();
        let mut ty: Option<&str> = None;
        match &k.tag {
            b"FLGS" => {}
            b"DBLE" if sparse_path(&attr) => {
                // A range of multis whose elements are sparse: count, then (index, value) pairs, for each.
                while c.ok && c.left() >= 4 {
                    let n = c.count(12);
                    let _ = write!(vals, " {n}");
                    for _ in 0..n {
                        let _ = write!(vals, " {} {}", c.u32(), num(c.f64()));
                    }
                }
            }
            b"DBLE" | b"DBL2" | b"DBL3" | b"CMPD" => {
                ty = match &k.tag {
                    b"DBL2" => Some("double2"),
                    b"DBL3" => Some("double3"),
                    _ => None,
                };
                if &k.tag == b"CMPD" && node_type.starts_with("animCurveT") && (attr == "ktv" || attr.starts_with("ktv[")) {
                    // Key times are ticks; the text has frames.
                    let per = self.fps() / self.ticks;
                    while c.left() >= 16 {
                        let t = c.u64() as i64 as f64 * per;
                        let _ = write!(vals, " {} {}", num(t), num(c.f64()));
                    }
                } else if matches!(&k.tag, b"DBLE" | b"CMPD") {
                    let per = self.fps() / self.ticks;
                    while c.left() >= 8 {
                        let _ = write!(vals, " {}", num(double_or_ticks(c.u64(), per)));
                    }
                } else {
                    while c.left() >= 8 {
                        let _ = write!(vals, " {}", num(c.f64()));
                    }
                }
            }
            b"FLT2" | b"FLT3" => {
                ty = Some(if &k.tag == b"FLT2" { "float2" } else { "float3" });
                while c.left() >= 4 {
                    let _ = write!(vals, " {}", num32(c.f32()));
                }
            }
            b"STR " => {
                ty = Some("string");
                let _ = write!(vals, " {}", q(&c.str()));
            }
            b"STR#" | b"ATAL" => {
                let n = c.count(1);
                let items: Vec<String> = (0..n).map(|_| c.str()).collect();
                if &k.tag == b"STR#" {
                    ty = Some("stringArray");
                    let _ = write!(vals, " {n}");
                    items.iter().for_each(|s| {
                        let _ = write!(vals, " {}", q(s));
                    });
                } else {
                    ty = Some("attributeAlias");
                    let list: Vec<String> = items.iter().map(|s| q(s)).collect();
                    let _ = write!(vals, " {{{}}}", list.join(","));
                }
            }
            b"I32#" => {
                ty = Some("Int32Array");
                let n = c.count(4);
                let _ = write!(vals, " {n}");
                for _ in 0..n {
                    let _ = write!(vals, " {}", c.u32() as i32);
                }
            }
            b"DBL#" => {
                ty = Some("doubleArray");
                let n = c.count(8);
                let _ = write!(vals, " {n}");
                for _ in 0..n {
                    let _ = write!(vals, " {}", num(c.f64()));
                }
            }
            b"PNT#" => {
                // x, y, z, w for each point, with no count ahead of them.
                ty = Some("pointArray");
                let n = c.left() / 32;
                let _ = write!(vals, " {n}");
                for _ in 0..n * 4 {
                    let _ = write!(vals, " {}", num(c.f64()));
                }
            }
            b"MATR" => {
                ty = Some("matrix");
                if c.left() == 128 {
                    for _ in 0..16 {
                        let _ = write!(vals, " {}", num(c.f64()));
                    }
                } else if let Some(x) = xform(&mut c) {
                    vals = x;
                } else {
                    *self.unknown_values.entry("MATR (transformation)".into()).or_default() += 1;
                    return;
                }
            }
            b"CMP#" => {
                ty = Some("componentList");
                let mut items = Vec::new();
                let n = c.count(8);
                for _ in 0..n {
                    let kind = component_kind(c.take(4));
                    let ranges = c.count(8);
                    for _ in 0..ranges {
                        let (lo, hi) = (c.u32() as i32, c.u32() as i32);
                        items.push(if lo < 0 {
                            format!("{kind}[*]")
                        } else if lo == hi {
                            format!("{kind}[{lo}]")
                        } else {
                            format!("{kind}[{lo}:{hi}]")
                        });
                    }
                }
                let _ = write!(vals, " {}", items.len());
                items.iter().for_each(|s| {
                    let _ = write!(vals, " {}", q(s));
                });
            }
            b"NRBC" => {
                match nurbs_curve(c.take(c.left())) {
                    Some(text) => {
                        ty = Some("nurbsCurve");
                        vals = text;
                    }
                    None => {
                        *self.unknown_values.entry("NRBC (layout not recognised)".into()).or_default() += 1;
                        return;
                    }
                }
            }
            b"MESH" => {
                if attr != "o" && attr != "ci" {
                    *self.unknown_values.entry(format!("MESH on .{attr}")).or_default() += 1;
                    return;
                }
                match mesh(&mut c) {
                    Some(text) => self.body.push_str(&text),
                    None => self.warnings.push("a mesh's data is cut short; it was left out".into()),
                }
                return;
            }
            other => {
                *self.unknown_values.entry(tag_name(*other)).or_default() += 1;
                return;
            }
        }
        if vals.is_empty() && (head == "\tsetAttr") && ty.is_none() {
            return;
        }
        let typed = ty.map(|t| format!(" -type {}", q(t))).unwrap_or_default();
        let _ = writeln!(self.body, "{head} {plug}{typed}{vals};");
    }

    /// addAttr: a four-letter code, four flag bytes, a word, the data type, long and short names, an enum's
    /// fields, a word, then the minimum, maximum and default where the flags say so.
    fn add_attr(&mut self, c: &mut Cursor) {
        let code = String::from_utf8_lossy(c.take(4)).into_owned();
        let f = c.take(4).to_vec();
        c.u32();
        let data_tag = c.take(4).to_vec();
        let long = c.str();
        let short = c.str();
        if !c.ok || long.is_empty() {
            return;
        }
        let at = match code.as_str() {
            "aTYP" => None,
            "aBOL" => Some("bool"),
            "aDBL" => Some("double"),
            "aFLT" => Some("float"),
            "aLI1" | "aLNG" => Some("long"),
            "aSHT" => Some("short"),
            "aBYT" => Some("byte"),
            "aCHR" => Some("char"),
            "aENM" => Some("enum"),
            "aMSG" => Some("message"),
            "aTIM" => Some("time"),
            "aANG" => Some("doubleAngle"),
            "aDIS" => Some("doubleLinear"),
            other => {
                self.warnings.push(format!("added attribute {long}: type code {} isn't known; left out", other.escape_debug()));
                return;
            }
        };
        let fields = if code == "aENM" { Some(c.str()) } else { None };
        c.u32();
        let (show, has) = (f.get(2).copied().unwrap_or(0), f.get(3).copied().unwrap_or(0));
        let mut line = String::from("\taddAttr -ci true");
        if show & 0x40 != 0 && code != "aMSG" {
            line.push_str(" -k true");
        }
        if show & 0x20 != 0 {
            line.push_str(" -h true");
        }
        let _ = write!(line, " -sn {} -ln {}", q(&short), q(&long));
        let finite = |x: f64| x.is_finite().then_some(x);
        let min = if has & 0x02 != 0 && c.left() >= 8 { finite(c.f64()) } else { None };
        let max = if has & 0x04 != 0 && c.left() >= 8 { finite(c.f64()) } else { None };
        let dv = if has & 0x08 != 0 && c.left() >= 12 && c.take(4) == b"DBLE" { finite(c.f64()) } else { None };
        // Limits only on a number, and only a pair that makes sense.
        let numeric = !matches!(code.as_str(), "aTYP" | "aMSG" | "aENM" | "aBOL");
        let (min, max) = match (min, max) {
            (Some(a), Some(b)) if a > b => (None, None),
            other => other,
        };
        if numeric {
            if let Some(v) = min {
                let _ = write!(line, " -min {}", num(v));
            }
            if let Some(v) = max {
                let _ = write!(line, " -max {}", num(v));
            }
        }
        if let (Some(v), true) = (dv, code != "aMSG" && code != "aTYP") {
            let _ = write!(line, " -dv {}", num(v));
        }
        match at {
            Some(at) => {
                let _ = write!(line, " -at {}", q(at));
                if let Some(en) = fields.filter(|e| !e.is_empty()) {
                    let _ = write!(line, " -en {}", q(&en));
                }
            }
            None => {
                let dt = match <[u8; 4]>::try_from(data_tag.as_slice()).unwrap_or_default() {
                    [b'S', b'T', b'R', b' '] => "string",
                    [b'S', b'T', b'R', b'#'] => "stringArray",
                    [b'M', b'A', b'T', b'R'] => "matrix",
                    [b'D', b'B', b'L', b'#'] => "doubleArray",
                    [b'I', b'3', b'2', b'#'] => "Int32Array",
                    [b'P', b'N', b'T', b'#'] => "pointArray",
                    [b'V', b'C', b'T', b'#'] => "vectorArray",
                    [b'M', b'E', b'S', b'H'] => "mesh",
                    [b'N', b'C', b'R', b'V'] => "nurbsCurve",
                    [b'C', b'M', b'P', b'#'] => "componentList",
                    other => {
                        self.warnings.push(format!("added attribute {long}: data type {} isn't known; left out", tag_name(other)));
                        return;
                    }
                };
                let _ = write!(line, " -dt {}", q(dt));
            }
        }
        let _ = writeln!(self.body, "{line};");
    }

    /// A reference node's edits: "ed", a flag, the owner, then groups of (reference node, count, edits).
    fn ref_edits(&mut self, c: &mut Cursor) {
        let attr = c.str();
        c.u8();
        let owner = c.str();
        let mut body = format!("\t\t{}\n", q(&owner));
        while c.ok && c.left() >= 4 {
            let groups = c.count(5);
            for _ in 0..groups {
                let node = c.str();
                let n = c.count(4);
                let _ = writeln!(body, "\t\t{} {n}", q(&node));
                for _ in 0..n {
                    let kind = c.u32();
                    let mut line = format!("\t\t{kind}");
                    let strings = match kind {
                        0 | 2 => 3,
                        1 => 4,
                        3 | 7 | 8 => 2,
                        5 => {
                            let _ = write!(line, " {}", c.u32());
                            4
                        }
                        other => {
                            self.warnings.push(format!("{owner}: a reference edit of kind {other} isn't known; the rest were left out"));
                            return self.finish_edits(&attr, &body);
                        }
                    };
                    for _ in 0..strings {
                        let _ = write!(line, " {}", q(&c.str()));
                    }
                    if c.ok {
                        let _ = writeln!(body, "{line}");
                    }
                }
            }
        }
        self.finish_edits(&attr, &body);
    }

    fn finish_edits(&mut self, attr: &str, body: &str) {
        let _ = writeln!(self.body, "\tsetAttr {} -type \"dataReferenceEdits\" \n{};", q(&format!(".{attr}")), body.trim_end_matches('\n'));
    }
}

/// "wl[0:19].w": a range of elements whose last part is itself a whole multi.
fn sparse_path(attr: &str) -> bool {
    let parts: Vec<&str> = attr.split('.').collect();
    let ranged = |p: &str| p.split_once('[').and_then(|(_, r)| r.strip_suffix(']')).is_some_and(|r| r.contains(':'));
    parts.len() > 1 && parts[..parts.len() - 1].iter().any(|p| ranged(p)) && !parts[parts.len() - 1].contains('[')
}

/// A double, or a time in ticks (a whole number the file keeps in the same eight bytes) given as frames.
fn double_or_ticks(bits: u64, frames_per_tick: f64) -> f64 {
    // As a double it would be denormal, or a NaN with its sign set; -0 and 0 are themselves.
    if (bits >> 52 == 0 && bits != 0) || bits >> 52 == 0xfff {
        return bits as i64 as f64 * frames_per_tick;
    }
    f64::from_bits(bits)
}

fn component_kind(t: &[u8]) -> &'static str {
    const KINDS: [(&[u8], &str); 5] = [(b"CMDF", "f"), (b"CMDE", "e"), (b"CMDV", "vtx"), (b"CMDM", "map"), (b"CMDS", "vtxFace")];
    KINDS.iter().find(|(k, _)| *k == t).map_or("f", |(_, n)| n)
}

/// A transformation matrix: scale, rotate, rotate order, then translate, shear, the pivots and their
/// translations, rotate axis and joint orient as quaternions, inverse parent scale, a spare quaternion and
/// whether parent scale is compensated.
fn xform(c: &mut Cursor) -> Option<String> {
    if c.left() < 41 * 8 {
        return None;
    }
    let mut out = String::from(" \"xform\"");
    for _ in 0..6 {
        let _ = write!(out, " {}", num(c.f64()));
    }
    let order = c.u32();
    c.u32();
    let _ = write!(out, " {order}");
    for _ in 0..29 {
        let _ = write!(out, " {}", num(c.f64()));
    }
    for _ in 0..4 {
        c.f64();
    }
    let _ = write!(out, " {}", if c.u8() != 0 { "yes" } else { "no" });
    c.ok.then_some(out)
}

/// A mesh's geometry chunk as the `.vt`, `.ed`, `.n` and `.fc` lines (and its uv and colour sets) of `.ma`.
/// A curve's shape: degree, spans, form, rational, dimension, the knots, then the CVs, as the text has them.
/// Read only when every count agrees and nothing is left over; otherwise None and the curve is left out.
fn nurbs_curve(b: &[u8]) -> Option<String> {
    // Where the rational flag is a byte or a word.
    for rational_width in [4usize, 1] {
        let mut c = Cursor::new(b);
        let (degree, spans, form) = (c.u32() as usize, c.u32() as usize, c.u32());
        let rational = if rational_width == 4 { c.u32() } else { u32::from(c.u8()) };
        let dim = c.u32() as usize;
        if !(1..=7).contains(&degree) || spans == 0 || spans > 1 << 20 || form > 2 || rational > 1 || !(2..=3).contains(&dim) {
            continue;
        }
        let nk = c.count(8);
        if !c.ok || nk != spans + 2 * degree - 1 {
            continue;
        }
        let knots: Vec<f64> = (0..nk).map(|_| c.f64()).collect();
        if knots.iter().any(|k| !k.is_finite()) || knots.windows(2).any(|w| w[1] < w[0]) {
            continue;
        }
        let per = dim + rational as usize;
        let ncv = c.count(8 * per);
        if !c.ok || ncv != spans + degree || c.left() != ncv * per * 8 {
            continue;
        }
        let cvs: Vec<f64> = (0..ncv * per).map(|_| c.f64()).collect();
        if cvs.iter().any(|x| !x.is_finite()) {
            continue;
        }
        let mut out = format!(" {degree} {spans} {form} {} {dim} {nk}", if rational == 1 { "yes" } else { "no" });
        for k in &knots {
            let _ = write!(out, " {}", num(*k));
        }
        let _ = write!(out, " {ncv}");
        for x in &cvs {
            let _ = write!(out, " {}", num(*x));
        }
        return Some(out);
    }
    None
}

fn mesh(c: &mut Cursor) -> Option<String> {
    let mut out = String::new();
    let nf = c.count(4);
    let points: Vec<f32> = (0..nf - nf % 3).map(|_| c.f32()).collect();
    for _ in 0..nf % 3 {
        c.f32();
    }
    let ne = c.count(4);
    let mut edges = Vec::with_capacity(ne / 2);
    for _ in 0..ne / 2 {
        let (a, b) = (c.u32(), c.u32());
        edges.push((a & 0x7fff_ffff, b & 0x7fff_ffff, a & 0x8000_0000 != 0));
    }
    if ne % 2 == 1 {
        c.u32();
    }
    // Each face's loops, each loop its edges (~e where the edge runs backwards).
    let nfe = c.count(4);
    let mut faces: Vec<Vec<Vec<i64>>> = Vec::new();
    let (mut face, mut lp): (Vec<Vec<i64>>, Vec<i64>) = (Vec::new(), Vec::new());
    for _ in 0..nfe {
        let w = c.u32();
        let e = i64::from(w & 0x1fff_ffff);
        lp.push(if w & 0x8000_0000 != 0 { !e } else { e });
        if w & 0x4000_0000 != 0 {
            face.push(std::mem::take(&mut lp));
        }
        if w & 0x2000_0000 != 0 {
            if !lp.is_empty() {
                face.push(std::mem::take(&mut lp));
            }
            faces.push(std::mem::take(&mut face));
        }
    }
    if !c.ok {
        return None;
    }
    let nn = c.count(4);
    let normals: Vec<f32> = (0..nn - nn % 3).map(|_| c.f32()).collect();
    for _ in 0..nn % 3 {
        c.f32();
    }
    if nn > 0 {
        let k = c.count(4);
        c.take(4 * k);
    }
    // Per face-vertex index lists run over every loop in face order.
    let loops: Vec<usize> = faces.iter().flat_map(|f| f.iter().map(Vec::len)).collect();
    let fv: usize = loops.iter().sum();
    let mut uv_sets: Vec<(String, Vec<f32>, Vec<i32>)> = Vec::new();
    let nsets = c.count(1);
    if nsets > 0 {
        c.u32();
        for _ in 0..nsets {
            let name = c.str();
            let n = c.count(4);
            let uvs: Vec<f32> = (0..n).map(|_| c.f32()).collect();
            let k = c.count(4);
            let idx: Vec<i32> = (0..k).map(|_| c.u32() as i32).collect();
            uv_sets.push((name, uvs, idx));
        }
    }
    let mut colour_sets: Vec<(String, Vec<f32>, Vec<i32>)> = Vec::new();
    let ncol = c.count(1);
    if ncol > 0 && c.ok {
        c.u32();
        for _ in 0..ncol {
            let name = c.str();
            c.u32();
            let n = c.count(4);
            let rgba: Vec<f32> = (0..n).map(|_| c.f32()).collect();
            let k = c.count(4);
            let idx: Vec<i32> = (0..k).map(|_| c.u32() as i32).collect();
            colour_sets.push((name, rgba, idx));
        }
    }

    let np = points.len() / 3;
    if np > 0 {
        let _ = write!(out, "\tsetAttr -s {np} \".vt[0:{}]\"", np - 1);
        points.iter().for_each(|v| {
            let _ = write!(out, " {}", num32(*v));
        });
        out.push_str(";\n");
    }
    if !edges.is_empty() {
        let _ = write!(out, "\tsetAttr -s {} \".ed[0:{}]\"", edges.len(), edges.len() - 1);
        for (a, b, hard) in &edges {
            let _ = write!(out, " {a} {b} {}", if *hard { 0 } else { 1 });
        }
        out.push_str(";\n");
    }
    let nnorm = normals.len() / 3;
    if nnorm > 0 {
        let _ = write!(out, "\tsetAttr -s {nnorm} \".n[0:{}]\" -type \"float3\"", nnorm - 1);
        normals.iter().for_each(|v| {
            let _ = write!(out, " {}", num32(*v));
        });
        out.push_str(";\n");
    }
    for (i, (name, uvs, _)) in uv_sets.iter().enumerate() {
        let _ = writeln!(out, "\tsetAttr \".uvst[{i}].uvsn\" -type \"string\" {};", q(name));
        let n = uvs.len() / 2;
        if n > 0 {
            let _ = write!(out, "\tsetAttr -s {n} \".uvst[{i}].uvsp[0:{}]\" -type \"float2\"", n - 1);
            uvs.iter().for_each(|v| {
                let _ = write!(out, " {}", num32(*v));
            });
            out.push_str(";\n");
        }
    }
    for (i, (name, rgba, _)) in colour_sets.iter().enumerate() {
        let _ = writeln!(out, "\tsetAttr \".clst[{i}].clsn\" -type \"string\" {};", q(name));
        let n = rgba.len() / 4;
        if n > 0 {
            let _ = write!(out, "\tsetAttr -s {n} \".clst[{i}].clsp[0:{}]\"", n - 1);
            rgba.iter().for_each(|v| {
                let _ = write!(out, " {}", num32(*v));
            });
            out.push_str(";\n");
        }
    }
    if !faces.is_empty() {
        let usable = |idx: &Vec<i32>| idx.len() == fv;
        let _ = write!(out, "\tsetAttr -s {} -ch {fv} \".fc[0:{}]\" -type \"polyFaces\"", faces.len(), faces.len() - 1);
        let mut at = 0usize;
        for f in &faces {
            for (li, lp) in f.iter().enumerate() {
                let _ = write!(out, "\n\t\t{} {}", if li == 0 { "f" } else { "h" }, lp.len());
                lp.iter().for_each(|e| {
                    let _ = write!(out, " {e}");
                });
                for (s, (_, _, idx)) in uv_sets.iter().enumerate() {
                    if usable(idx) {
                        let ids = &idx[at..at + lp.len()];
                        if ids.iter().all(|&i| i >= 0) {
                            let _ = write!(out, "\n\t\tmu {s} {}", ids.len());
                            ids.iter().for_each(|i| {
                                let _ = write!(out, " {i}");
                            });
                        }
                    }
                }
                for (s, (_, _, idx)) in colour_sets.iter().enumerate() {
                    if usable(idx) {
                        let ids = &idx[at..at + lp.len()];
                        let _ = write!(out, "\n\t\tmc {s} {}", ids.len());
                        ids.iter().for_each(|i| {
                            let _ = write!(out, " {i}");
                        });
                    }
                }
                at += lp.len();
            }
        }
        out.push_str(";\n");
    }
    Some(out)
}

/// The 16 bytes of a node's id as `rename -uid` writes it: four little-endian words.
fn uuid(b: &[u8]) -> String {
    let w = |i: usize| u32::from_le_bytes(b[i * 4..i * 4 + 4].try_into().unwrap_or_default());
    format!("{:08X}-{:04X}-{:04X}-{:04X}-{:04X}{:08X}", w(0), w(1) >> 16, w(1) & 0xffff, w(2) >> 16, w(2) & 0xffff, w(3))
}

fn q(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The shortest text that reads back as the same number, with an exponent where it would be long without.
fn num(v: f64) -> String {
    if !v.is_finite() {
        return "0".into();
    }
    let a = v.abs();
    if a != 0.0 && !(1e-5..1e15).contains(&a) {
        format!("{v:e}")
    } else {
        format!("{v}")
    }
}

fn num32(v: f32) -> String {
    if !v.is_finite() {
        return "0".into();
    }
    let a = v.abs();
    if a != 0.0 && !(1e-5..1e15).contains(&a) {
        format!("{v:e}")
    } else {
        format!("{v}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_read_as_rename_writes_them() {
        let b = [0xaa, 0x4a, 0x35, 0xb5, 0x9f, 0x9d, 0xe9, 0x4a, 0xad, 0x8e, 0xcd, 0x3f, 0xb0, 0x0b, 0x27, 0x69];
        assert_eq!(uuid(&b), "B5354AAA-4AE9-9D9F-3FCD-8EAD69270BB0");
    }

    #[test]
    fn times_kept_as_ticks_read_as_frames() {
        let per = 24.0 / TICKS;
        assert_eq!(double_or_ticks(5_880_000, per), 1.0);
        assert_eq!(double_or_ticks((-5_880_000i64) as u64, per), -1.0);
        assert_eq!(double_or_ticks(2.5f64.to_bits(), per), 2.5);
        assert_eq!(double_or_ticks(0, per), 0.0);
        assert!(double_or_ticks((-0.0f64).to_bits(), per) == 0.0);
    }

    fn attr_record(code: &[u8; 4], flags: [u8; 4], name: &str, fields: Option<&str>, min: f64, max: f64, dv: f64) -> Vec<u8> {
        let mut b = code.to_vec();
        b.extend(flags);
        b.extend([0, 0, 0, 1, 0, 0, 0, 0]);
        for _ in 0..2 {
            b.extend(name.as_bytes());
            b.push(0);
        }
        if let Some(f) = fields {
            b.extend(f.as_bytes());
            b.push(0);
        }
        b.extend([0; 4]);
        if flags[3] & 0x02 != 0 {
            b.extend(min.to_be_bytes());
        }
        if flags[3] & 0x04 != 0 {
            b.extend(max.to_be_bytes());
        }
        if flags[3] & 0x08 != 0 {
            b.extend(b"DBLE");
            b.extend(dv.to_be_bytes());
        }
        b
    }

    fn added(b: &[u8]) -> (String, Vec<String>) {
        let mut w = Writer::default();
        w.add_attr(&mut Cursor::new(b));
        (w.body, w.warnings)
    }

    #[test]
    fn added_attributes_of_every_kind() {
        // The first flag byte set, as a rig's attributes have it: the code still ends after four letters.
        let (body, warn) = added(&attr_record(b"aDBL", [0x01, 0xf8, 0xc0, 0x4e], "blend", None, 0.0, 1.0, 0.5));
        assert!(warn.is_empty(), "{warn:?}");
        assert_eq!(body, "\taddAttr -ci true -k true -sn \"blend\" -ln \"blend\" -min 0 -max 1 -dv 0.5 -at \"double\";\n");
        let (body, _) = added(&attr_record(b"aENM", [0x00, 0xf8, 0x02, 0x4e], "space", Some("world:local"), 0.0, 1.0, 1.0));
        assert_eq!(body, "\taddAttr -ci true -sn \"space\" -ln \"space\" -dv 1 -at \"enum\" -en \"world:local\";\n");
        let (body, _) = added(&attr_record(b"aMSG", [0x00, 0xf8, 0x00, 0x00], "compRoot", None, 0.0, 0.0, 0.0));
        assert_eq!(body, "\taddAttr -ci true -sn \"compRoot\" -ln \"compRoot\" -at \"message\";\n");
        let (body, _) = added(&attr_record(b"aFLT", [0x00, 0xf8, 0x40, 0x08], "RBF_Multiplier", None, 0.0, 0.0, 2.0));
        assert!(body.contains("-dv 2 -at \"float\""), "{body}");
        // Limits that hold nothing are left off rather than read.
        let (body, _) = added(&attr_record(b"aDBL", [0x00, 0xf8, 0x00, 0x06], "roll", None, 1.044e208, 0.0, 0.0));
        assert!(!body.contains("-min") && !body.contains("-max"), "{body}");
    }

    #[test]
    fn curve_shapes_read_only_when_they_add_up() {
        let mut b = Vec::new();
        for w in [3u32, 1, 0, 0, 3, 6] {
            b.extend(w.to_be_bytes());
        }
        for k in [0.0f64, 0.0, 0.0, 1.0, 1.0, 1.0] {
            b.extend(k.to_be_bytes());
        }
        b.extend(4u32.to_be_bytes());
        for x in [0.0f64, 0.0, 0.0, 1.0, 0.0, 0.0, 2.0, 0.0, 0.0, 3.0, 0.0, 0.0] {
            b.extend(x.to_be_bytes());
        }
        assert_eq!(nurbs_curve(&b).as_deref(), Some(" 3 1 0 no 3 6 0 0 0 1 1 1 4 0 0 0 1 0 0 2 0 0 3 0 0"));
        b.push(0);
        assert_eq!(nurbs_curve(&b), None);
        assert_eq!(nurbs_curve(&[0xff, 0, 7]), None);
    }

    #[test]
    fn sparse_paths() {
        assert!(sparse_path("wl[0:19].w"));
        assert!(!sparse_path("wl[3].w"));
        assert!(!sparse_path("uvst[0].uvsp[0:13]"));
        assert!(!sparse_path("t"));
    }

    #[test]
    fn other_data_is_refused() {
        assert!(convert(b"//Maya ASCII").is_err());
        assert!(convert(b"FOR8 data").is_err());
    }
}
