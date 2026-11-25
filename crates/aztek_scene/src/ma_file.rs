//! .ma read and write, and the `file` command.
//!
//! Reading runs in two passes. The statements become a plain list of nodes, setAttr data and connections (`Doc`), with
//! no graph involved; then what Aztek can represent is built from that list, and the rest is reported by node type.
//! Files are read tolerantly: unknown commands and attributes are skipped, unknown node types become a warning.
//!
//! A mesh with construction history is read and written as its history, the creator and every edit as the reference's
//! nodes (see `history_io`); one with none, or with history that can't go in a file, as stored geometry (.vt .ed .fc). A
//! deformed mesh gets an intermediate Orig shape holding, or fed by the history making, what its deformers read, as the
//! reference does (see `deformers`). Aztek's internal nodes (ikSolve, its
//! matrix-fed constraints) are written as the nodes the reference expects: ikEffector and a solver, constraints with their
//! target compounds.
//!
//! Numbers that are easy to get wrong, all as the reference writes them: signed edges in .fc use ~e for reversed, so -2 is edge
//! 1 reversed; matrices list Aztek's columns in order, which are the reference's rows; .kix is in seconds and .kiy in internal
//! units (radians on angle curves); tangent codes in .kit and .kot are the file's own (see `tangent_code`).

mod deformers;
mod dynamics;
mod unknown;
mod lights;
mod shading;
mod nurbs;
mod characters;
mod history_io;

use aztek_ops::units;
use std::cell::Cell;

thread_local! {
    /// Centimetres and radians per unit of the file being read.
    static FILE_UNITS: Cell<(f64, f64)> = const { Cell::new((1.0, std::f64::consts::PI / 180.0)) };
}

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt::Write as _;
use std::sync::Arc;

use aztek_geo::{names, Index, Mesh, INVALID};
use aztek_ops::{fail, ArgType, Args, CmdResult, CommandDef, Commands, FlagDef};
use glam::{DMat4, DQuat, DVec3};
use serde_json::{json, Value};

use crate::anim::{self, Key, Keys, Tangent};
use crate::graph::{Graph, SelectMode};
use crate::mesh_nodes::MeshNode;
use crate::node::{types, EvalContext, NodeBehavior, NodeData, NodeType};
use crate::plug::{flags, AttrDef, NodeId, PlugRef, PlugType, PlugValue, NO_NODE};
use crate::rig::{self, Joint};
use crate::session::Session;
use crate::skin::{self, SkinBind};
use crate::transform::{euler_to_matrix, DagNode, RotateOrder, Transform};

// ---------------------------------------------------------------------------------------------------------------------
// Tokens

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Tok {
    Num(f64),
    Word(String),
    Str(String),
}

impl Tok {
    pub(crate) fn text(&self) -> Option<&str> {
        match self {
            Tok::Word(s) | Tok::Str(s) => Some(s),
            Tok::Num(_) => None,
        }
    }
    pub(crate) fn num(&self) -> Option<f64> {
        match self {
            Tok::Num(v) => Some(*v),
            Tok::Word(s) => truth_word(s).map(|b| b as i32 as f64),
            Tok::Str(_) => None,
        }
    }
    fn truth(&self) -> Option<bool> {
        match self {
            Tok::Num(v) => Some(*v != 0.0),
            Tok::Word(s) | Tok::Str(s) => truth_word(s),
        }
    }
    pub(crate) fn is_flag(&self) -> bool {
        matches!(self, Tok::Word(s) if s.starts_with('-'))
    }
}

fn truth_word(s: &str) -> Option<bool> {
    match s {
        "yes" | "on" | "true" => Some(true),
        "no" | "off" | "false" => Some(false),
        _ => None,
    }
}

/// Splits source into statements: whitespace-separated tokens up to a semicolon, spanning any number of lines.
pub(crate) struct Lexer<'a> {
    text: &'a str,
    i: usize,
}

impl<'a> Lexer<'a> {
    pub(crate) fn new(text: &'a str) -> Self {
        Lexer { text, i: 0 }
    }

    fn skip_space(&mut self) {
        let b = self.text.as_bytes();
        loop {
            while self.i < b.len() && b[self.i].is_ascii_whitespace() {
                self.i += 1;
            }
            if b[self.i..].starts_with(b"//") {
                while self.i < b.len() && b[self.i] != b'\n' {
                    self.i += 1;
                }
            } else if b[self.i..].starts_with(b"/*") {
                match self.text[self.i..].find("*/") {
                    Some(end) => self.i += end + 2,
                    None => self.i = b.len(),
                }
            } else {
                return;
            }
        }
    }

    fn string(&mut self) -> String {
        let b = self.text.as_bytes();
        self.i += 1;
        let mut out = Vec::new();
        while self.i < b.len() && b[self.i] != b'"' {
            if b[self.i] == b'\\' && self.i + 1 < b.len() {
                self.i += 1;
                out.push(match b[self.i] {
                    b'n' => b'\n',
                    b't' => b'\t',
                    b'r' => b'\r',
                    c => c,
                });
            } else {
                out.push(b[self.i]);
            }
            self.i += 1;
        }
        self.i += 1;
        String::from_utf8_lossy(&out).into_owned()
    }

    /// The next statement into out; false at the end of the text.
    pub(crate) fn statement(&mut self, out: &mut Vec<Tok>) -> bool {
        out.clear();
        let b = self.text.as_bytes();
        let mut joining = false;
        loop {
            self.skip_space();
            if self.i >= b.len() {
                return !out.is_empty();
            }
            match b[self.i] {
                b';' => {
                    self.i += 1;
                    return true;
                }
                b'"' => {
                    let s = self.string();
                    match (joining, out.last_mut()) {
                        (true, Some(Tok::Str(last))) => last.push_str(&s),
                        _ => out.push(Tok::Str(s)),
                    }
                    joining = false;
                }
                _ => {
                    let start = self.i;
                    while self.i < b.len() && !b[self.i].is_ascii_whitespace() && !matches!(b[self.i], b';' | b'"') {
                        self.i += 1;
                    }
                    let w = &self.text[start..self.i];
                    // Long strings are written ("abc" + "def").
                    if w == "(" || w == ")" {
                        continue;
                    }
                    if w == "+" && matches!(out.last(), Some(Tok::Str(_))) {
                        joining = true;
                        continue;
                    }
                    out.push(word_token(w));
                }
            }
        }
    }
}

fn word_token(w: &str) -> Tok {
    let b = w.as_bytes();
    let numeric = match b[0] {
        b'0'..=b'9' => true,
        b'-' | b'+' | b'.' => b.len() > 1 && (b[1].is_ascii_digit() || (b[0] != b'.' && b[1] == b'.' && b.len() > 2)),
        _ => false,
    };
    if numeric {
        if let Ok(v) = w.parse::<f64>() {
            return Tok::Num(v);
        }
    }
    Tok::Word(w.to_string())
}

// ---------------------------------------------------------------------------------------------------------------------
// The document

#[derive(Clone, Debug, PartialEq)]
struct Seg {
    name: String,
    range: Option<(i64, i64)>,
}

/// A setAttr as the file wrote it: the plug (`dli[1:5]`), and what follows the plug.
fn kept_line(set: &SetAttr) -> (String, String) {
    let key: Vec<String> = set
        .path
        .iter()
        .map(|seg| match seg.range {
            Some((a, b)) if a == b => format!("{}[{a}]", seg.name),
            Some((a, b)) => format!("{}[{a}:{b}]", seg.name),
            None => seg.name.clone(),
        })
        .collect();
    let mut text = String::new();
    if let Some(t) = &set.ty {
        text.push_str(&format!("-type {}", quoted(t)));
    }
    for v in &set.values {
        if !text.is_empty() {
            text.push(' ');
        }
        match v {
            Tok::Num(x) => text.push_str(&fmt_num(*x)),
            Tok::Word(w) => text.push_str(w),
            Tok::Str(s) => text.push_str(&quoted(s)),
        }
    }
    (key.join("."), text)
}

/// Indices past this are not data; a bad file should not ask for that much memory.
const MAX_INDEX: i64 = 1 << 28;

/// ".uvst[0].uvsp[0:13]" as segments.
fn parse_path(s: &str) -> Vec<Seg> {
    let s = s.strip_prefix('.').unwrap_or(s);
    s.split('.')
        .map(|part| {
            let (name, range) = match part.find('[') {
                Some(open) if part.ends_with(']') => {
                    let inner = &part[open + 1..part.len() - 1];
                    let r = match inner.split_once(':') {
                        Some((a, b)) => a.parse().ok().zip(b.parse().ok()),
                        None => inner.parse().ok().map(|a: i64| (a, a)),
                    };
                    (&part[..open], r.filter(|&(a, b): &(i64, i64)| 0 <= a && a <= b && b < MAX_INDEX))
                }
                _ => (part, None),
            };
            Seg { name: name.to_string(), range }
        })
        .collect()
}

#[derive(Debug)]
struct SetAttr {
    path: Vec<Seg>,
    lock: Option<bool>,
    values: Vec<Tok>,
    /// The -type given, when one was.
    ty: Option<String>,
}

#[derive(Debug, Default)]
struct AddAttr {
    long: String,
    short: String,
    at: String,
    dt: String,
    dv: Option<f64>,
    min: Option<f64>,
    max: Option<f64>,
    en: String,
    parent: String,
    keyable: bool,
    multi: bool,
    nice: String,
    category: Vec<String>,
    world_space: bool,
    not_unique: bool,
}

#[derive(Debug, Default)]
struct MaNode {
    ty: String,
    name: String,
    parent: Option<usize>,
    /// Named with `select -ne :time1` or `":time1.o"`: a node every scene has, not one the file creates.
    default: bool,
    sets: Vec<SetAttr>,
    adds: Vec<AddAttr>,
    /// lockNode -l 1.
    locked: bool,
    /// -p naming a node this file doesn't hold: one from a reference.
    ext_parent: Option<String>,
}

impl MaNode {
    /// The last non-empty setAttr on this plain attribute.
    fn set_of(&self, name: &[&str]) -> Option<&SetAttr> {
        self.sets
            .iter()
            .rev()
            .find(|s| s.path.len() == 1 && s.path[0].range.is_none() && name.contains(&s.path[0].name.as_str()) && !s.values.is_empty())
    }
    fn num(&self, name: &[&str]) -> Option<f64> {
        self.set_of(name)?.values.first()?.num()
    }
    fn truth(&self, name: &[&str]) -> Option<bool> {
        self.set_of(name)?.values.first()?.truth()
    }
    fn text(&self, name: &[&str]) -> Option<&str> {
        self.set_of(name)?.values.first()?.text()
    }
    fn vec3(&self, name: &[&str]) -> Option<DVec3> {
        let v = &self.set_of(name)?.values;
        Some(DVec3::new(v.first()?.num()?, v.get(1)?.num()?, v.get(2)?.num()?))
    }
    /// Elements of an array attribute, stride values each: `.kit[0:2]  1 2 3` and `.kit[4] 1`, in file order.
    fn elements(&self, name: &[&str], stride: usize) -> Vec<(usize, &[Tok])> {
        let mut out = Vec::new();
        for s in &self.sets {
            if s.path.len() != 1 || !name.contains(&s.path[0].name.as_str()) || s.values.is_empty() {
                continue;
            }
            let start = s.path[0].range.map_or(0, |r| r.0.max(0) as usize);
            for (k, c) in s.values.chunks(stride).enumerate() {
                if c.len() == stride {
                    out.push((start + k, c));
                }
            }
        }
        out
    }
    /// A single matrix element given as 16 numbers: `.pm[0]`.
    fn matrix_element(&self, name: &[&str], element: i64) -> Option<DMat4> {
        let s = self.sets.iter().rev().find(|s| {
            s.path.len() == 1 && name.contains(&s.path[0].name.as_str()) && s.path[0].range.is_some_and(|r| r.0 <= element && element <= r.1) && !s.values.is_empty()
        })?;
        let skip = (element - s.path[0].range?.0) as usize * 16;
        matrix_from(s.values.get(skip..skip + 16)?)
    }
}

fn matrix_from(v: &[Tok]) -> Option<DMat4> {
    if v.len() < 16 {
        return None;
    }
    let mut a = [0.0; 16];
    for (d, t) in a.iter_mut().zip(v) {
        *d = t.num()?;
    }
    Some(DMat4::from_cols_array(&a))
}

#[derive(Clone, Debug)]
struct Conn {
    src: usize,
    src_attr: String,
    dst: usize,
    dst_attr: String,
}

/// An attribute path without its indices: "ip[0].ig" is "ip.ig".
fn plain_attr(a: &str) -> String {
    let mut out = String::with_capacity(a.len());
    let mut depth = 0;
    for c in a.chars() {
        match c {
            '[' => depth += 1,
            ']' => depth -= 1,
            c if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out
}

/// The first index in an attribute path: 3 in "tg[3].tpm".
fn first_index(a: &str) -> Option<i64> {
    let open = a.find('[')?;
    let close = a[open..].find(']')? + open;
    a[open + 1..close].parse().ok().filter(|e| (0..MAX_INDEX).contains(e))
}

#[derive(Debug, Clone)]
struct RefLine {
    src: String,
    dst: String,
    src_ref: bool,
    dst_ref: bool,
    owner: Option<String>,
}

/// The connections into and out of each node, and each node's children, by index in the document.
#[derive(Debug, Default)]
struct DocIndex {
    ins: Vec<Vec<usize>>,
    outs: Vec<Vec<usize>>,
    kids: Vec<Vec<usize>>,
}

#[derive(Debug)]
struct Doc {
    /// Plug-ins the file's requires lines name.
    /// Plug-ins the file names, each with the node types it says come from it.
    requires: Vec<(String, Vec<String>)>,
    /// The data types each requires line gives its plug-in.
    plugin_data_types: HashMap<String, Vec<String>>,
    /// The version each requires line gives its plug-in.
    plugin_versions: HashMap<String, String>,
    /// fileInfo lines, in order.
    file_info: Vec<(String, String)>,
    nodes: Vec<MaNode>,
    by_name: HashMap<String, Vec<usize>>,
    conns: Vec<Conn>,
    /// Made when the build starts (make_index), once the document no longer changes.
    index: std::sync::OnceLock<DocIndex>,
    /// Centimetres per file unit, degrees per file unit.
    lin: f64,
    ang: f64,
    fps: Option<f64>,
    /// The time unit is 29.97df.
    drop_frame: bool,
    warnings: Vec<String>,
    /// file -r lines.
    refs: Vec<crate::references::FileRef>,
    /// file -rdi lines: reference node, starts unloaded.
    ref_load: Vec<(String, bool)>,
    /// parent -s -add: the shape and the transform it's also under.
    instances: Vec<(String, String)>,
    /// What the file connects to :defaultLightSet.dsm.
    light_set: Vec<String>,
    /// relationship lines: the kind, the object's plug, the light's plug.
    relationships: Vec<(String, String, String)>,
    /// Connections with a node of a reference: source, destination, whether each is in one, and the reference node
    /// when a placeholder named it.
    ref_conns: Vec<RefLine>,
    up_z: bool,
    /// Characters and their clips, taken out of the nodes and connections (see characters.rs).
    characters: Vec<characters::CharFile>,
}

fn linear_unit(s: &str) -> Option<f64> {
    Some(match s {
        "mm" | "millimeter" => 0.1,
        "cm" | "centimeter" => 1.0,
        "m" | "meter" => 100.0,
        "km" | "kilometer" => 100000.0,
        "in" | "inch" => 2.54,
        "ft" | "foot" => 30.48,
        "yd" | "yard" => 91.44,
        "mi" | "mile" => 160934.4,
        _ => return None,
    })
}

use crate::anim_time::{time_unit_fps as time_unit, time_unit_name};

impl Doc {
    fn find(&self, path: &str) -> Option<usize> {
        let path = path.strip_prefix(':').unwrap_or(path);
        let parts: Vec<&str> = path.split('|').collect();
        let last = parts.last()?;
        let list = self.by_name.get(*last)?;
        for &i in list.iter().rev() {
            if parts.len() == 1 {
                return Some(i);
            }
            let rooted = parts[0].is_empty();
            let mut at = self.nodes[i].parent;
            let mut ok = true;
            for want in parts[..parts.len() - 1].iter().rev() {
                if want.is_empty() {
                    break;
                }
                match at {
                    Some(p) if self.nodes[p].name == *want => at = self.nodes[p].parent,
                    _ => {
                        ok = false;
                        break;
                    }
                }
            }
            if ok && (!rooted || at.is_none()) {
                return Some(i);
            }
        }
        None
    }

    fn add_node(&mut self, n: MaNode) -> usize {
        let i = self.nodes.len();
        self.by_name.entry(n.name.clone()).or_default().push(i);
        self.nodes.push(n);
        i
    }

    /// A node by name; a `:name` the file never created is one of the scene's default nodes.
    fn find_or_default(&mut self, path: &str) -> Option<usize> {
        if let Some(i) = self.find(path) {
            return Some(i);
        }
        let name = path.strip_prefix(':')?;
        Some(self.add_node(MaNode { name: name.to_string(), default: true, ..Default::default() }))
    }

    fn rename(&mut self, i: usize, new: &str) {
        let old = std::mem::replace(&mut self.nodes[i].name, new.to_string());
        if let Some(v) = self.by_name.get_mut(&old) {
            v.retain(|&x| x != i);
        }
        self.by_name.entry(new.to_string()).or_default().push(i);
    }

    fn parse(text: &str) -> Doc {
        FILE_UNITS.with(|u| u.set((1.0, std::f64::consts::PI / 180.0)));
        let mut d = Doc { light_set: Vec::new(), relationships: Vec::new(), nodes: Vec::new(), by_name: HashMap::new(), conns: Vec::new(), index: std::sync::OnceLock::new(), lin: 1.0, ang: std::f64::consts::PI / 180.0, fps: None, drop_frame: false, warnings: Vec::new(), refs: Vec::new(), ref_load: Vec::new(), instances: Vec::new(), ref_conns: Vec::new(), up_z: false, requires: Vec::new(), plugin_versions: HashMap::new(), plugin_data_types: HashMap::new(), file_info: Vec::new(), characters: Vec::new() };
        let mut lex = Lexer::new(text);
        let mut st = Vec::new();
        let mut current: Option<usize> = None;
        let mut raw_conns: Vec<(String, String)> = Vec::new();
        while lex.statement(&mut st) {
            let Some(cmd) = st.first().and_then(Tok::text).map(str::to_string) else { continue };
            match cmd.as_str() {
                "createNode" => current = d.create_node(&st).or(current),
                "requires" => {
                    let plain = plain_args(&st, &[]);
                    if let Some(name) = plain.first().filter(|n| **n != "maya") {
                        let types: Vec<String> = st.windows(2).filter(|w| matches!(w[0].text(), Some("-nodeType") | Some("-nt"))).filter_map(|w| w[1].text().map(str::to_string)).collect();
                        if let Some(v) = plain.get(1) {
                            d.plugin_versions.insert(name.to_string(), v.to_string());
                        }
                        let data: Vec<String> = st.windows(2).filter(|w| matches!(w[0].text(), Some("-dataType") | Some("-dt"))).filter_map(|w| w[1].text().map(str::to_string)).collect();
                        if !data.is_empty() {
                            d.plugin_data_types.insert(name.to_string(), data);
                        }
                        d.requires.push((name.to_string(), types));
                    }
                }
                "fileInfo" => {
                    let plain = plain_args(&st, &[]);
                    if let [k, v, ..] = plain.as_slice() {
                        d.file_info.push((k.to_string(), v.to_string()));
                    }
                }
                "relationship" => {
                    if let [_, Some(kind), _, Some(object), Some(light)] = st.iter().map(Tok::text).collect::<Vec<_>>().as_slice() {
                        d.relationships.push((kind.to_string(), object.to_string(), light.to_string()));
                    }
                }
                "setAttr" => d.set_attr(&st, current),
                "addAttr" => d.add_attr(&st, current),
                "connectAttr" => {
                    let plain = plain_args(&st, &["-l", "-lock", "-na", "-nextAvailable", "-f", "-force", "-rd", "-referenceDest"]);
                    if let [a, b, ..] = plain.as_slice() {
                        raw_conns.push((a.to_string(), b.to_string()));
                    }
                }
                "disconnectAttr" => {
                    let plain = plain_args(&st, &["-na", "-nextAvailable"]);
                    if let [a, b, ..] = plain.as_slice() {
                        raw_conns.retain(|(x, y)| !(x == a && y == b));
                    }
                }
                "rename" => {
                    if !st.iter().any(|t| t.text() == Some("-uid")) {
                        let plain = plain_args(&st, &["-is", "-ignoreShape"]);
                        match plain.as_slice() {
                            [new] => {
                                if let Some(i) = current {
                                    d.rename(i, new);
                                }
                            }
                            [old, new, ..] => {
                                if let Some(i) = d.find(old) {
                                    d.rename(i, new);
                                }
                            }
                            _ => {}
                        }
                    }
                }
                "select" => {
                    if st.iter().any(|t| matches!(t.text(), Some("-ne") | Some("-noExpand"))) {
                        if let Some(name) = plain_args(&st, &["-ne", "-noExpand", "-r", "-add"]).last() {
                            if let Some(i) = d.find_or_default(name) {
                                current = Some(i);
                            } else if name.contains(':') {
                                // A node from a reference: its edits are applied once the reference is in.
                                current = Some(d.add_node(MaNode { name: name.to_string(), default: true, ..Default::default() }));
                            }
                        }
                    }
                }
                "parent" => d.parent(&st),
                "currentUnit" => d.current_unit(&st),
                "file" => d.file_line(&st),
                "lockNode" => {
                    let off = st.iter().position(|t| matches!(t, Tok::Word(w) if w == "-l" || w == "-lock")).and_then(|i| st.get(i + 1)).and_then(Tok::num) == Some(0.0);
                    if let Some(i) = current {
                        d.nodes[i].locked = !off;
                    }
                }
                "upAxis" => {
                    if st.iter().any(|t| t.text() == Some("z") || t.text() == Some("Z")) {
                        d.up_z = true;
                    }
                }
                _ => {}
            }
        }
        let placeholders = d.placeholders();
        for (a, b) in raw_conns {
            // Through a reference's placeholder list: what it stands for is in the reference's edits.
            let (held_a, held_b) = (placeholders.get(&a), placeholders.get(&b));
            if held_a.is_some() || held_b.is_some() {
                let owner = held_a.or(held_b).map(|h| h.1.clone());
                d.ref_conns.push(RefLine { src: held_a.map_or(a.clone(), |h| h.0.clone()), dst: held_b.map_or(b.clone(), |h| h.0.clone()), src_ref: held_a.is_some(), dst_ref: held_b.is_some(), owner });
                continue;
            }
            // The sharedReferenceNode and the like hold nothing to connect.
            if a.split_once('.').is_some_and(|x| x.1 == "sr" || x.1 == "sharedReference") {
                continue;
            }
            let (Some((sn, sa)), Some((dn, da))) = (a.split_once('.'), b.split_once('.')) else { continue };
            // Lists Aztek keeps no node for: the set's members are read later, the light list is every light.
            if dn.trim_start_matches(':') == crate::lights::DEFAULT_SET {
                if matches!(da, "dsm" | "dagSetMembers") || da.starts_with("dsm[") || da.starts_with("dagSetMembers[") {
                    d.light_set.push(sn.to_string());
                }
                continue;
            }
            if dn.trim_start_matches(':') == "lightList1" {
                continue;
            }
            // Into the lists every scene has (renderPartition, defaultShaderList1, the layer managers): what they list
            // is written again on saving.
            let listing = |n: &str, attr: &str| {
                let first = attr.split(['.', '[']).next().unwrap_or("");
                crate::defaults::LISTS.iter().any(|&(ln, la)| ln == n.trim_start_matches(':') && la == first)
            };
            if listing(dn, da) || listing(sn, sa) {
                continue;
            }
            // Plugs on default nodes the file never touched are of no interest: only nodes it knows connect.
            let from_ref = |d: &Doc, n: &str| n.trim_start_matches(':').contains(':') && !shading::is_named_default(n) && d.find(n).is_none_or(|i| d.nodes[i].default);
            if from_ref(&d, sn) || from_ref(&d, dn) {
                let (src_ref, dst_ref) = (from_ref(&d, sn), from_ref(&d, dn));
                d.ref_conns.push(RefLine { src: a.clone(), dst: b.clone(), src_ref, dst_ref, owner: None });
                continue;
            }
            // A connection to lambert1 or initialShadingGroup holds even if the file never selected it.
            let dst = match d.find(dn) {
                None if shading::is_named_default(dn) => d.find_or_default(dn),
                found => found,
            };
            let src = match d.find(sn) {
                None if shading::is_named_default(sn) => d.find_or_default(sn),
                found => found,
            };
            let (Some(src), Some(dst)) = (src, dst) else { continue };
            d.conns.push(Conn { src, src_attr: sa.to_string(), dst, dst_attr: da.to_string() });
        }
        // The script node the reference keeps a Z up scene's axis in.
        if let Some(n) = d.by_name.get("upAxisScriptNode").and_then(|v| v.first()).map(|&i| &d.nodes[i]) {
            if n.text(&["b", "before"]).is_some_and(|b| b.contains("upAxis -ax \"z\"") || b.contains("!= \"z\"")) {
                d.up_z = true;
            }
        }
        d
    }

    /// The `rigRN.phl[1]` plugs a file's connections name, to what they stand for and their reference node. The reference writes
    /// `5 side reference plug placeholder flags` in `.ed`.
    fn placeholders(&self) -> HashMap<String, (String, String)> {
        let mut out = HashMap::new();
        for (rfn, tokens) in self.reference_edits_text() {
            let (sections, _) = crate::references::parse_edits(&tokens);
            for (_, list) in sections {
                for e in list.iter().filter(|e| e.kind == 5) {
                    let [_, _, plug, holder, ..] = e.args.as_slice() else { continue };
                    let Some(index) = holder.split_once('.').map(|x| x.1) else { continue };
                    let index = index.trim_start_matches("placeHolderList").trim_start_matches("phl");
                    for name in ["phl", "placeHolderList"] {
                        out.insert(format!("{rfn}.{name}{index}"), (plug.clone(), rfn.clone()));
                    }
                }
            }
        }
        out
    }

    /// `file -rdi`, where the reference keeps a reference's load state, and `file -r`, which always says -dr 1 in its own files.
    fn file_line(&mut self, st: &[Tok]) {
        let word = |t: &Tok, names: &[&str]| matches!(t, Tok::Word(w) if names.contains(&w.as_str()));
        let after = |names: &[&str]| st.iter().position(|t| word(t, names)).and_then(|i| st.get(i + 1)).and_then(Tok::text).map(str::to_string);
        let on = |names: &[&str]| st.iter().position(|t| word(t, names)).is_some_and(|i| st.get(i + 1).and_then(Tok::num).is_none_or(|v| v != 0.0));
        let is_rdi = st.iter().any(|t| word(t, &["-rdi", "-referenceDepthInfo"]));
        let rfn = after(&["-rfn", "-referenceNode"]);
        let deferred = on(&["-dr", "-deferReference"]);
        if is_rdi {
            if let Some(rfn) = rfn {
                self.ref_load.push((rfn, deferred));
            }
            return;
        }
        if !st.iter().any(|t| word(t, &["-r", "-reference"])) {
            return;
        }
        let path = st.iter().rev().find_map(|t| match t {
            Tok::Str(s) => Some(s.clone()),
            _ => None,
        });
        if let Some(path) = path {
            self.refs.push(crate::references::FileRef {
                path,
                ns: after(&["-ns", "-namespace"]).unwrap_or_default(),
                rfn,
                deferred,
                rpr: after(&["-rpr", "-renamingPrefix"]),
                file_type: after(&["-typ", "-type"]).unwrap_or_default(),
                options: after(&["-op", "-options"]).unwrap_or_default(),
                locked: on(&["-lck", "-lockReference"]),
                merge: false,
            });
        }
    }

    /// What each reference node's `.ed` holds, as text.
    fn reference_edits_text(&self) -> Vec<(String, Vec<String>)> {
        let mut out = Vec::new();
        for n in self.nodes.iter().filter(|n| n.ty == crate::references::REF_NODE_TYPE) {
            for set in &n.sets {
                if matches!(set.path.as_slice(), [seg] if seg.name == "ed" || seg.name == "edits") {
                    let tok = set.values.iter().map(|t| match t {
                        Tok::Num(v) => v.to_string(),
                        Tok::Word(w) | Tok::Str(w) => w.clone(),
                    });
                    out.push((n.name.clone(), tok.collect()));
                }
            }
        }
        out
    }

    fn create_node(&mut self, st: &[Tok]) -> Option<usize> {
        let ty = st.get(1)?.text()?.to_string();
        let mut name = String::new();
        let mut parent = None;
        let mut ext_parent = None;
        let mut k = 2;
        while k < st.len() {
            match st[k].text() {
                Some("-n") | Some("-name") => {
                    name = st.get(k + 1)?.text()?.to_string();
                    k += 1;
                }
                Some("-p") | Some("-parent") => {
                    let named = st.get(k + 1)?.text()?;
                    parent = self.find(named);
                    // A parent the file doesn't make: a node of a reference.
                    if parent.is_none() && named.contains(':') {
                        ext_parent = Some(named.to_string());
                    }
                    k += 1;
                }
                _ => {}
            }
            k += 1;
        }
        // A name the reference would not take: said, unless disableIncorrectNameWarning has turned that off.
        if !name.is_empty() && crate::db_cmds::name_warning_on() {
            let leaf = name.rsplit('|').next().unwrap_or(&name);
            if !crate::graph::Graph::valid_name(leaf) {
                self.warnings.push(format!("Incorrect node name \"{name}\": it is read as \"{}\"", crate::graph::Graph::legal_name(leaf)));
            }
        }
        if name.is_empty() {
            let n = self.nodes.iter().filter(|n| n.ty == ty).count() + 1;
            name = format!("{ty}{n}");
        }
        Some(self.add_node(MaNode { ty, name, parent, ext_parent, ..Default::default() }))
    }

    fn set_attr(&mut self, st: &[Tok], current: Option<usize>) {
        let mut plug = None;
        let mut lock = None;
        let mut ty = None;
        let mut values = Vec::with_capacity(st.len().saturating_sub(2));
        let mut k = 1;
        while k < st.len() {
            let t = &st[k];
            if plug.is_none() || t.is_flag() {
                match t.text() {
                    Some("-s") | Some("-size") | Some("-ch") | Some("-capacityHint") | Some("-ca") | Some("-caching") | Some("-k") | Some("-keyable")
                    | Some("-cb") | Some("-channelBox") | Some("-av") | Some("-alteredValue") | Some("-c") | Some("-clamp") => {
                        // These take a value, or (-av, -c, a bare -k) nothing; a plug or another flag next means nothing.
                        if st.get(k + 1).is_some_and(|n| n.num().is_some() || n.truth().is_some()) {
                            k += 1;
                        }
                    }
                    Some("-l") | Some("-lock") => {
                        lock = st.get(k + 1).and_then(Tok::truth).or(Some(true));
                        if st.get(k + 1).is_some_and(|n| n.truth().is_some()) {
                            k += 1;
                        }
                    }
                    Some("-type") | Some("-typ") => {
                        ty = st.get(k + 1).and_then(Tok::text).map(str::to_string);
                        k += 1;
                    }
                    Some(p) if !t.is_flag() && plug.is_none() => plug = Some(p.to_string()),
                    _ => {}
                }
            } else {
                values.push(t.clone());
            }
            k += 1;
        }
        let Some(plug) = plug else { return };
        let (node, attr) = if let Some(rel) = plug.strip_prefix('.') {
            (current, rel.to_string())
        } else {
            match plug.split_once('.') {
                Some((n, a)) => (self.find_or_default(n), a.to_string()),
                None => (None, String::new()),
            }
        };
        let Some(node) = node else { return };
        self.nodes[node].sets.push(SetAttr { path: parse_path(&attr), lock, values, ty });
    }

    fn add_attr(&mut self, st: &[Tok], current: Option<usize>) {
        let Some(node) = current else { return };
        let mut a = AddAttr::default();
        let mut k = 1;
        while k < st.len() {
            let val = st.get(k + 1);
            let text = || val.and_then(Tok::text).unwrap_or("").to_string();
            let mut used = true;
            match st[k].text().unwrap_or("") {
                "-ln" | "-longName" => a.long = text(),
                "-sn" | "-shortName" => a.short = text(),
                "-at" | "-attributeType" => a.at = text(),
                "-dt" | "-dataType" => a.dt = text(),
                "-en" | "-enumName" => a.en = text(),
                "-nn" | "-niceName" => a.nice = text(),
                "-p" | "-parent" => a.parent = text(),
                "-dv" | "-defaultValue" => a.dv = val.and_then(Tok::num),
                "-min" | "-minValue" => a.min = val.and_then(Tok::num),
                "-max" | "-maxValue" => a.max = val.and_then(Tok::num),
                "-k" | "-keyable" => a.keyable = val.and_then(Tok::truth).unwrap_or(true),
                "-ct" | "-category" => a.category.push(text()),
                "-ws" | "-worldSpace" => a.world_space = val.and_then(Tok::truth).unwrap_or(true),
                "-eun" | "-enforcingUniqueName" => a.not_unique = !val.and_then(Tok::truth).unwrap_or(true),
                "-m" | "-multi" => {
                    a.multi = true;
                    used = false;
                }
                _ => used = false,
            }
            k += if used { 2 } else { 1 };
        }
        if a.long.is_empty() {
            a.long = a.short.clone();
        }
        if !a.long.is_empty() {
            self.nodes[node].adds.push(a);
        }
    }

    fn parent(&mut self, st: &[Tok]) {
        let flagged = |names: &[&str]| st.iter().any(|t| t.text().is_some_and(|x| names.contains(&x)));
        if flagged(&["-add", "-addObject"]) || flagged(&["-s", "-shape"]) {
            let plain = plain_args(st, &["-s", "-shape", "-add", "-addObject", "-r", "-relative", "-a", "-absolute", "-nc", "-noConnections"]);
            if let [shape, parent, ..] = plain.as_slice() {
                self.instances.push((shape.to_string(), parent.to_string()));
            }
            return;
        }
        let plain = plain_args(st, &["-r", "-relative", "-a", "-absolute", "-nc", "-noConnections", "-w", "-world"]);
        if flagged(&["-w", "-world"]) {
            if let Some(i) = plain.first().and_then(|n| self.find(n)) {
                self.nodes[i].parent = None;
            }
        } else if let [child, parent, ..] = plain.as_slice() {
            if let (Some(c), Some(p)) = (self.find(child), self.find(parent)) {
                self.nodes[c].parent = Some(p);
            }
        }
    }

    fn current_unit(&mut self, st: &[Tok]) {
        let mut k = 1;
        while k + 1 < st.len() {
            let v = st[k + 1].text().unwrap_or("");
            match st[k].text().unwrap_or("") {
                "-l" | "-linear" => {
                    self.lin = linear_unit(v).unwrap_or(1.0);
                    FILE_UNITS.with(|u| u.set((self.lin, u.get().1)));
                }
                "-a" | "-angle" => {
                    self.ang = if v.starts_with("rad") { 1.0 } else { std::f64::consts::PI / 180.0 };
                    FILE_UNITS.with(|u| u.set((u.get().0, self.ang)));
                }
                "-t" | "-time" => {
                    self.fps = time_unit(v);
                    self.drop_frame = v == "29.97df";
                }
                _ => {}
            }
            k += 2;
        }
    }

    /// The one connection into node's attribute (indices ignored unless the name holds them), and its source.
    fn input(&self, node: usize, attrs: &[&str]) -> Option<&Conn> {
        self.conns_in(node).find(|c| c.dst == node && attrs.contains(&plain_attr(&c.dst_attr).as_str()))
    }

    /// The connections into a node, from the index once the build has made it, else by going through them all.
    fn conns_in(&self, node: usize) -> impl Iterator<Item = &Conn> + '_ {
        self.by_index(node, |x| &x.ins, move |c: &&Conn| c.dst == node)
    }

    /// The connections out of a node, as conns_in.
    fn conns_out(&self, node: usize) -> impl Iterator<Item = &Conn> + '_ {
        self.by_index(node, |x| &x.outs, move |c: &&Conn| c.src == node)
    }

    fn by_index<'a>(&'a self, node: usize, list: impl Fn(&'a DocIndex) -> &'a Vec<Vec<usize>>, test: impl Fn(&&Conn) -> bool + 'a) -> impl Iterator<Item = &'a Conn> + 'a {
        let indexed = self.index.get().map(|x| list(x).get(node).map_or(&[][..], |v| v.as_slice()).iter().map(move |&k| &self.conns[k]));
        let scanned = self.index.get().is_none().then(|| self.conns.iter().filter(test));
        indexed.into_iter().flatten().chain(scanned.into_iter().flatten())
    }

    /// Makes the index of connections and children the build reads them through: a scan of the whole document for each
    /// node it builds made a big scene's read take time in the square of its size. Once nothing changes the document.
    fn make_index(&self) {
        self.index.get_or_init(|| {
            let mut x = DocIndex { ins: vec![Vec::new(); self.nodes.len()], outs: vec![Vec::new(); self.nodes.len()], kids: vec![Vec::new(); self.nodes.len()] };
            for (k, c) in self.conns.iter().enumerate() {
                if let Some(v) = x.ins.get_mut(c.dst) {
                    v.push(k);
                }
                if let Some(v) = x.outs.get_mut(c.src) {
                    v.push(k);
                }
            }
            for (i, n) in self.nodes.iter().enumerate() {
                if let Some(v) = n.parent.and_then(|p| x.kids.get_mut(p)) {
                    v.push(i);
                }
            }
            x
        });
    }

    fn index_of(&self, n: &MaNode) -> usize {
        (n as *const MaNode as usize - self.nodes.as_ptr() as usize) / std::mem::size_of::<MaNode>()
    }

    fn children(&self, node: usize) -> impl Iterator<Item = usize> + '_ {
        let indexed = self.index.get().map(|x| x.kids.get(node).map_or(&[][..], |v| v.as_slice()).iter().copied());
        let scanned = self.index.get().is_none().then(|| self.nodes.iter().enumerate().filter(move |(_, n)| n.parent == Some(node)).map(|(i, _)| i));
        indexed.into_iter().flatten().chain(scanned.into_iter().flatten())
    }
}

/// Text and number tokens after the command, without flags. Flags in `bare` take no value; the rest take one.
fn plain_args(st: &[Tok], bare: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    let mut k = 1;
    while k < st.len() {
        let t = &st[k];
        if t.is_flag() {
            if !t.text().is_some_and(|x| bare.contains(&x)) {
                k += 1;
            }
        } else if let Some(s) = t.text() {
            out.push(s.to_string());
        }
        k += 1;
    }
    out
}

// ---------------------------------------------------------------------------------------------------------------------
// Meshes in the file

#[derive(Debug, Default)]
struct MaFace {
    edges: Vec<i64>,
    /// (set index, UV per corner).
    uvs: Vec<(i64, Vec<i64>)>,
}

#[derive(Debug, Default)]
struct UvSet {
    name: String,
    uvs: Vec<[f32; 2]>,
}

#[derive(Debug, Default)]
struct MaMesh {
    points: Vec<DVec3>,
    tweaks: BTreeMap<usize, DVec3>,
    edges: Vec<[i64; 3]>,
    faces: Vec<MaFace>,
    holes: usize,
    uv_sets: BTreeMap<i64, UvSet>,
    current_uv: String,
}

impl MaMesh {
    fn from_node(n: &MaNode) -> MaMesh {
        let mut m = MaMesh::default();
        // Points are distances, in the file's linear unit as the reference writes them.
        let lin = FILE_UNITS.with(Cell::get).0;
        for s in &n.sets {
            let Some(first) = s.path.first() else { continue };
            let start = first.range.map_or(0, |r| r.0.max(0) as usize);
            match (first.name.as_str(), s.path.get(1).map(|x| x.name.as_str())) {
                ("vt" | "vrts", None) => {
                    for (k, c) in s.values.chunks_exact(3).enumerate() {
                        put(&mut m.points, start + k, tok_vec3(c) * lin);
                    }
                }
                ("pt" | "pnts", None) => {
                    for (k, c) in s.values.chunks_exact(3).enumerate() {
                        m.tweaks.insert(start + k, tok_vec3(c) * lin);
                    }
                }
                ("pt" | "pnts", Some(child)) => {
                    if let (Some(v), Some(axis)) = (s.values.first().and_then(Tok::num), axis_of(child)) {
                        m.tweaks.entry(start).or_default()[axis] = v * lin;
                    }
                }
                ("ed" | "edge", None) => {
                    for (k, c) in s.values.chunks_exact(3).enumerate() {
                        let e = [c[0].num().unwrap_or(0.0) as i64, c[1].num().unwrap_or(0.0) as i64, c[2].num().unwrap_or(1.0) as i64];
                        put(&mut m.edges, start + k, e);
                    }
                }
                ("fc" | "face", None) => m.read_faces(start, s),
                ("uvst" | "uvSet", Some(child)) => {
                    let set = m.uv_sets.entry(first.range.map_or(0, |r| r.0)).or_default();
                    match child {
                        "uvsn" | "uvSetName" => set.name = s.values.first().and_then(Tok::text).unwrap_or("").to_string(),
                        "uvsp" | "uvSetPoints" => {
                            let at = s.path[1].range.map_or(0, |r| r.0.max(0) as usize);
                            for (k, c) in s.values.chunks_exact(2).enumerate() {
                                let uv = [c[0].num().unwrap_or(0.0) as f32, c[1].num().unwrap_or(0.0) as f32];
                                put(&mut set.uvs, at + k, uv);
                            }
                        }
                        _ => {}
                    }
                }
                ("cuvs" | "currentUVSet", None) => m.current_uv = s.values.first().and_then(Tok::text).unwrap_or("").to_string(),
                _ => {}
            }
        }
        m
    }

    fn read_faces(&mut self, start: usize, s: &SetAttr) {
        let v = &s.values;
        let mut at = start;
        let mut k = 0;
        let mut started = false;
        // A list of up to n ints after a count.
        let list = |k: &mut usize| -> Vec<i64> {
            let n = v.get(*k).and_then(Tok::num).unwrap_or(0.0) as usize;
            *k += 1;
            let mut out = Vec::with_capacity(n);
            for _ in 0..n {
                match v.get(*k).and_then(Tok::num) {
                    Some(x) => out.push(x as i64),
                    None => break,
                }
                *k += 1;
            }
            out
        };
        while k < v.len() {
            let word = v[k].text().unwrap_or("");
            k += 1;
            match word {
                "f" => {
                    if started {
                        at += 1;
                    }
                    started = true;
                    let edges = list(&mut k);
                    if self.faces.len() <= at {
                        self.faces.resize_with(at + 1, MaFace::default);
                    }
                    self.faces[at].edges = edges;
                }
                "h" => {
                    let _ = list(&mut k);
                    self.holes += 1;
                }
                "mu" | "mf" => {
                    let set = if word == "mu" {
                        let s = v.get(k).and_then(Tok::num).unwrap_or(0.0) as i64;
                        k += 1;
                        s
                    } else {
                        0
                    };
                    let uvs = list(&mut k);
                    if let Some(f) = self.faces.get_mut(at) {
                        f.uvs.push((set, uvs));
                    }
                }
                // Colour assignments: fc n c..., mc set n c...
                "mc" => {
                    k += 1;
                    let _ = list(&mut k);
                }
                "fc" => {
                    let _ = list(&mut k);
                }
                _ => {}
            }
        }
    }

    fn has_geometry(&self) -> bool {
        !self.points.is_empty() && !self.faces.is_empty()
    }

    /// The Aztek mesh: points in file order, faces in file order, the current UV set as uv0.
    fn build(&self, name: &str, warnings: &mut Vec<String>) -> Mesh {
        let mut sizes = Vec::with_capacity(self.faces.len());
        let mut indices = Vec::new();
        let mut kept: Vec<usize> = Vec::with_capacity(self.faces.len());
        let mut bad = 0;
        for (fi, f) in self.faces.iter().enumerate() {
            let start = indices.len();
            let mut ok = f.edges.len() >= 3;
            for &e in &f.edges {
                let (index, reversed) = if e < 0 { (!e, true) } else { (e, false) };
                match self.edges.get(index as usize) {
                    Some(ed) => indices.push(if reversed { ed[1] } else { ed[0] } as Index),
                    None => {
                        ok = false;
                        break;
                    }
                }
            }
            let pts = &indices[start..];
            let repeats = pts.iter().enumerate().any(|(i, p)| pts[..i].contains(p));
            if !ok || repeats || pts.iter().any(|&p| p as usize >= self.points.len()) {
                indices.truncate(start);
                bad += 1;
                continue;
            }
            sizes.push(f.edges.len() as Index);
            kept.push(fi);
        }
        if bad > 0 {
            warnings.push(format!("{name}: {bad} unusable faces dropped"));
        }
        if self.holes > 0 {
            warnings.push(format!("{name}: {} face holes dropped", self.holes));
        }
        let points: Vec<DVec3> = self.points.iter().enumerate().map(|(i, p)| *p + self.tweaks.get(&i).copied().unwrap_or(DVec3::ZERO)).collect();
        // The edges numbered as the file numbers them, when every one is a side of a face: the reference's history
        // names edges by number, and replaying it needs the same ones (#180). Very large meshes are built in bulk.
        let mut used = vec![false; self.edges.len()];
        for &fi in &kept {
            for &e in &self.faces[fi].edges {
                if let Some(u) = used.get_mut((if e < 0 { !e } else { e }) as usize) {
                    *u = true;
                }
            }
        }
        let in_range = |p: i64| 0 <= p && (p as usize) < points.len();
        let file_order = indices.len() < (1 << 16) && points.len() < (1 << 16) && used.iter().all(|&u| u) && self.edges.iter().all(|e| in_range(e[0]) && in_range(e[1]) && e[0] != e[1]);
        let mut m = if file_order {
            let mut m = Mesh::new();
            for &p in &points {
                m.add_point(p);
            }
            for e in &self.edges {
                m.add_edge(e[0] as Index, e[1] as Index);
            }
            let mut at = 0;
            for &n in &sizes {
                m.add_face(&indices[at..at + n as usize]);
                at += n as usize;
            }
            m
        } else {
            Mesh::from_polygons(&points, &sizes, &indices)
        };

        let mut hard = Vec::new();
        for e in &self.edges {
            let in_range = |p: i64| 0 <= p && (p as usize) < self.points.len();
            if e[2] == 0 && in_range(e[0]) && in_range(e[1]) {
                let id = m.find_edge(e[0] as Index, e[1] as Index);
                if id != INVALID {
                    hard.push(id);
                }
            }
        }
        if !hard.is_empty() {
            let a = m.edge_attrs_mut().get_or_add::<bool>(names::HARD);
            for id in hard {
                a[id as usize] = true;
            }
        }

        // Every set the file has, under its name and in its order. The current one goes in uv0, where Aztek's tools
        // work, and the others after it, each with the seams its own UVs make.
        let mut order: Vec<i64> = self.uv_sets.keys().copied().collect();
        order.retain(|k| !self.uv_sets[k].uvs.is_empty());
        let mut set_names: Vec<String> = Vec::new();
        for (i, k) in order.iter().enumerate() {
            let given = self.uv_sets[k].name.replace('|', "_");
            let mut name = if given.is_empty() { format!("map{}", i + 1) } else { given };
            while set_names.contains(&name) {
                name.push('_');
            }
            set_names.push(name);
        }
        let current = set_names.iter().position(|n| *n == self.current_uv).unwrap_or(0);
        if !set_names.is_empty() {
            crate::uv_sets::name_layers(&mut m, &set_names, &set_names[current]);
        }
        for (at, set_id) in order.iter().enumerate() {
            let slot = if at == current { 0 } else if at < current { at + 1 } else { at };
            let set = &self.uv_sets[set_id];
            let mut ids = vec![-1i64; m.corner_capacity() as usize];
            let mut uv = vec![[0.0f32; 2]; m.corner_capacity() as usize];
            for (face, &fi) in kept.iter().enumerate() {
                let f = &self.faces[fi];
                let Some((_, list)) = f.uvs.iter().find(|(s, _)| s == set_id) else { continue };
                // Corners by the point they sit on: the file's order starts wherever its first edge starts.
                for c in m.face_corners(face as Index) {
                    let p = m.corner_point(c);
                    let at = f.edges.iter().position(|&e| {
                        let (index, reversed) = if e < 0 { (!e, true) } else { (e, false) };
                        self.edges.get(index as usize).is_some_and(|ed| (if reversed { ed[1] } else { ed[0] }) as Index == p)
                    });
                    if let Some(id) = at.and_then(|k| list.get(k)).copied() {
                        ids[c as usize] = id;
                        if let Some(v) = set.uvs.get(id.max(0) as usize) {
                            uv[c as usize] = *v;
                        }
                    }
                }
            }
            m.corner_attrs_mut().get_or_add::<[f32; 2]>(&names::uv(slot as i32)).copy_from_slice(&uv);
            if slot == 0 {
                mark_seams(&mut m, &ids);
            } else {
                let mut seams = vec![false; m.edge_capacity() as usize];
                for e in seam_edges(&m, &ids) {
                    seams[e as usize] = true;
                }
                crate::uv_sets::set_seams(&mut m, &set_names[at], &seams);
            }
        }
        m
    }
}

fn put<T: Default + Clone>(v: &mut Vec<T>, at: usize, x: T) {
    if v.len() <= at {
        v.resize(at + 1, T::default());
    }
    v[at] = x;
}

fn tok_vec3(c: &[Tok]) -> DVec3 {
    DVec3::new(c[0].num().unwrap_or(0.0), c[1].num().unwrap_or(0.0), c[2].num().unwrap_or(0.0))
}

fn axis_of(child: &str) -> Option<usize> {
    match child.chars().last()? {
        'x' => Some(0),
        'y' => Some(1),
        'z' => Some(2),
        _ => None,
    }
}

/// Edges whose two sides use different UVs at one spot get the seam flag, so coincident UVs stay separate.
fn mark_seams(m: &mut Mesh, ids: &[i64]) {
    let seams = seam_edges(m, ids);
    if !seams.is_empty() {
        let a = m.edge_attrs_mut().get_or_add::<bool>("seam");
        for e in seams {
            a[e as usize] = true;
        }
    }
}

/// The edges whose two sides use different UV ids at either end.
fn seam_edges(m: &Mesh, ids: &[i64]) -> Vec<Index> {
    let mut seams = Vec::new();
    for c in 0..m.corner_capacity() {
        if !m.corner_valid(c) {
            continue;
        }
        let r = m.corner_radial(c);
        if r == c || r == INVALID || m.corner_radial(r) != c {
            continue;
        }
        let (p, q) = (m.corner_point(c), m.corner_next_point(c));
        let other = |point: Index| if m.corner_point(r) == point { r } else { m.corner_next(r) };
        let (cq, rp, rq) = (m.corner_next(c), other(p), other(q));
        if ids[c as usize] != ids[rp as usize] || ids[cq as usize] != ids[rq as usize] {
            seams.push(m.corner_edge(c));
        }
    }
    seams
}

// ---------------------------------------------------------------------------------------------------------------------
// Attribute names

const XFORM_CHANNELS: &[&str] = &[
    "visibility", "translate", "rotate", "scale", "rotateOrder", "rotatePivot", "scalePivot", "rotatePivotTranslate", "scalePivotTranslate", "rotateAxis", "shear", "offsetParentMatrix",
    "inheritsTransform", "displayHandle", "displayRotatePivot", "displayScalePivot", "displayLocalAxis", "selectHandle", "lodVisibility",
    "minTransLimit", "maxTransLimit", "minTransLimitEnable", "maxTransLimitEnable", "minRotLimit", "maxRotLimit", "minRotLimitEnable", "maxRotLimitEnable", "minScaleLimit", "maxScaleLimit", "minScaleLimitEnable", "maxScaleLimitEnable",
];

/// What every DAG node reads and writes: transforms, shapes, joints, handles.
const DRAW_CHANNELS: &[&str] = &[
    "overrideEnabled", "overrideDisplayType", "overrideLevelOfDetail", "overrideShading", "overrideTexturing", "overridePlayback", "overrideVisibility", "hideOnPlayback",
    "overrideRGBColors", "overrideColor", "overrideColorRGB", "overrideColorA", "template", "useObjectColor", "objectColor", "objectColorRGB", "wireColorRGB", "useOutlinerColor", "outlinerColor",
    "hiddenInOutliner", "selectionChildHighlighting",
];

/// Rigging and animation nodes that are all attributes and connections.
const PLAIN_NODES: &[&str] = &["timeSliderBookmark", "audio", "xgmSplineBase", "xgmModifierScale", "xgmModifierSculpt", "xgmModifierClump", "xgmModifierNoise", "xgmModifierCut", "xgmModifierCollide", "xgmModifierDisplace", "xgmModifierGuide", "xgmModifierLinearWire", "xgmCurveToSpline", "xgmModifierSplineCache", "AlembicNode", "collider", "rigidBody", "dynamicConstraint", "heldNode", "unknown", "network", "blendWeighted", "makeNurbCircle", "makeNurbPlane", "makeNurbsSquare", "motionPath", "ramp", "extrude", "reverseSurface", "transformGeometry", "pointOnCurveInfo", "curveInfo", "nearestPointOnCurve", "container", "controller", "timeWarp", "hierarchyTreeNode", "curveFromMeshEdge", "characterMap", "keyingGroup", "filter", "filterEuler", "filterResample", "filterSimplify", "filterClosestSample", "characterOffset"];
/// The same under a transform.
const PLAIN_SHAPES: &[&str] = &["hairSystem", "imagePlane", "poseInterpolator", "xgmSplineDescription", "solver", "particles", "field", "emitter", "follicle", "distanceDimShape", "paramDimension", "annotationShape", "dropoffLocator", "lattice", "baseLattice", "softModHandle", "mayaUsdProxyShape", "AL_usdmaya_ProxyShape", "ProxyUsdGeomCamera"];

/// Nodes written and read attribute by attribute, with their connections: utility nodes and expressions.
/// What a file's unitConversion does: values are in stored units, as in the reference, so its factor means what it says.
fn conversion_factor(n: &MaNode) -> f64 {
    n.num(&["cf", "conversionFactor"]).unwrap_or(1.0)
}

/// A unitConversion that changes a value, and so has to exist here.
fn scales(n: &MaNode) -> bool {
    n.ty == "unitConversion" && conversion_factor(n) != 1.0
}

fn is_plain_node(ty: &str) -> bool {
    crate::scripted::plugin_of_type(ty).is_some_and(|_| !crate::deform::is_deformer(ty)) || ty == "animLayer" || ty.starts_with("animBlendNode") || ty.starts_with("HIK") || ty == "CustomRigRetargeterNode" || ty == "expression" || ty == crate::script_node::TYPE || (ty != "unitConversion" && crate::utility_nodes::classification(ty).is_some()) || PLAIN_NODES.contains(&ty) || PLAIN_SHAPES.contains(&ty)
}
/// Matrices a file connection may read: they are never set, so they are no channels to write.
const MATRIX_OUTPUTS: &[&str] = &["matrix", "inverseMatrix", "xformMatrix", "worldMatrix", "worldInverseMatrix", "parentMatrix", "parentInverseMatrix"];
const LINEAR_CHANNELS: &[&str] = &["translate", "rotatePivot", "scalePivot", "rotatePivotTranslate", "scalePivotTranslate"];
const ANGLE_CHANNELS: &[&str] = &["rotate", "rotateAxis", "jointOrient", "preferredAngle"];

/// Creator nodes: (type, [(the reference short name, Aztek attribute)]).
const CREATORS: &[(&str, &[(&str, &str)])] = &[
    ("polyCube", &[("w", "width"), ("h", "height"), ("d", "depth"), ("sw", "subdivisionsWidth"), ("sh", "subdivisionsHeight"), ("sd", "subdivisionsDepth")]),
    ("polySphere", &[("r", "radius"), ("sa", "subdivisionsAxis"), ("sh", "subdivisionsHeight")]),
    ("polyCylinder", &[("r", "radius"), ("h", "height"), ("sa", "subdivisionsAxis"), ("sh", "subdivisionsHeight"), ("sc", "subdivisionsCaps")]),
    ("polyCone", &[("r", "radius"), ("h", "height"), ("sa", "subdivisionsAxis"), ("sh", "subdivisionsHeight"), ("sc", "subdivisionsCap")]),
    ("polyPlane", &[("w", "width"), ("h", "height"), ("sw", "subdivisionsWidth"), ("sh", "subdivisionsHeight")]),
    ("polyTorus", &[("r", "radius"), ("sr", "sectionRadius"), ("sa", "subdivisionsAxis"), ("sh", "subdivisionsHeight")]),
];

fn creator_attrs(ty: &str) -> Option<&'static [(&'static str, &'static str)]> {
    CREATORS.iter().find(|(t, _)| *t == ty).map(|(_, a)| *a)
}

fn channels_of(t: &NodeType) -> Vec<&'static str> {
    let mut v: Vec<&'static str> = Vec::new();
    if t.dag {
        v.extend(DRAW_CHANNELS);
    }
    // A deformer handle's shape: its settings, as the reference writes them there.
    if let Some((_, _, ps)) = crate::deform::NONLINEAR_PARAMS.iter().find(|k| t.name == k.1) {
        v.extend(ps.iter().map(|p| p.0));
        v.push("handleWidth");
    }
    if t.name == "clusterHandle" {
        v.push("origin");
    }
    if t.name == "implicitSphere" {
        v.push("radius");
    }
    if t.is_a("transform") {
        v.extend(XFORM_CHANNELS);
    }
    if t.is_a("joint") {
        v.extend(["jointOrient", "radius", "segmentScaleCompensate", "preferredAngle"]);
    }
    if t.is_a("ikHandle") {
        v.extend(["poleVector", "twist", "roll", "ikBlend", "offset", "weight", "poWeight", "priority", "stickiness", "snapEnable", "solverEnable", "rootOnCurve", "dTwistControlEnable", "dWorldUpType", "dForwardAxis", "dWorldUpAxis", "dWorldUpVector", "dWorldUpVectorEnd", "dWorldUpMatrix", "dWorldUpMatrixEnd", "dTwistValueType", "dTwistStart", "dTwistEnd", "dTwistRamp", "dTwistRampMult"]);
    }
    if t.is_a("locator") {
        v.extend(["visibility", "localPosition", "localScale"]);
    }
    if t.name == "ProxyUsdGeomCamera" {
        v.extend(["stage", "path"]);
    }
    if t.is_a("mesh") || t.is_a("nurbsCurve") || t.is_a("nurbsSurface") {
        v.extend(["visibility", "intermediateObject"]);
    }
    if t.is_a("mesh") || t.is_a("nurbsSurface") {
        v.extend(crate::harold_nodes::SHAPE_RENDER_NAMES);
    }
    if let Some(attrs) = creator_attrs(&t.name) {
        v.extend(attrs.iter().map(|(_, a)| *a));
    }
    v
}

/// The Aztek plug a reference attribute name stands for on a node of this type: long or short, a vector or one component.
fn channel(t: &NodeType, attr: &str) -> Option<(usize, i32)> {
    let attr = attr.split('[').next().unwrap_or(attr);
    let mut attr = attr.to_string();
    if t.is_a("locator") {
        if let Some(rest) = attr.strip_prefix("los") {
            attr = format!("ls{rest}");
        }
    }
    if t.is_a("joint") && attr == "ssc" {
        attr = "segmentScaleCompensate".into();
    }
    if let Some(table) = creator_attrs(&t.name) {
        if let Some((_, ours)) = table.iter().find(|(m, _)| *m == attr) {
            attr = ours.to_string();
        }
    }
    let allowed = channels_of(t);
    for (i, a) in t.attrs.iter().enumerate() {
        if !a.has(flags::USER) && !a.has(flags::MESSAGE) && !allowed.contains(&a.name.as_str()) {
            continue;
        }
        if a.name == attr || (!a.short_name.is_empty() && a.short_name == attr) {
            return Some((i, -1));
        }
        if a.ty == PlugType::Vec3 {
            for c in 0..3 {
                if a.child_names[c] == attr || a.child_short[c] == attr {
                    return Some((i, c as i32));
                }
            }
        }
    }
    None
}

/// `channel`, or a matrix a connection reads, for what a matrix input on another node takes.
fn source_channel(t: &NodeType, attr: &str) -> Option<(usize, i32)> {
    channel(t, attr).or_else(|| {
        let plain = attr.split('[').next().unwrap_or(attr);
        t.attrs.iter().position(|a| MATRIX_OUTPUTS.contains(&a.name.as_str()) && (a.name == plain || a.short_name == plain)).map(|i| (i, -1))
    })
}

/// The reference name of an Aztek plug on a written node, for setAttr and connectAttr.
fn ref_name(a: &AttrDef, component: i32) -> String {
    let short = match a.name.as_str() {
        "localScale" => "los",
        "segmentScaleCompensate" => "ssc",
        _ if a.short_name.is_empty() => &a.name,
        _ => &a.short_name,
    };
    match component {
        c if c >= 0 => format!("{short}{}", ["x", "y", "z"][c as usize]),
        _ => short.to_string(),
    }
}

fn plug_ref(id: NodeId, attr: usize, component: i32) -> PlugRef {
    if component >= 0 {
        PlugRef::component(id, attr as i32, component)
    } else {
        PlugRef::new(id, attr as i32)
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Curves not driven by time

const DRIVEN_CURVES: &[&str] = &["animCurveUL", "animCurveUA", "animCurveUU", "animCurveUT"];

/// A curve whose input is a plug instead of time: a set driven key.
struct DrivenCurve;

impl NodeBehavior for DrivenCurve {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        let ty = d.ty().clone();
        let at = |n: &str| ty.attr_index(n).unwrap();
        let v = match d.value(at("keys")) {
            PlugValue::Data(Some(k)) => k.as_any().downcast_ref::<Keys>().map(|k| k.eval(d.float(at("input")), infinity_of(d.int(at("preInfinity"))), infinity_of(d.int(at("postInfinity"))))),
            _ => None,
        };
        d.set_output(at("output"), PlugValue::Float(v.unwrap_or(0.0)));
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

fn infinity_of(i: i64) -> anim::Infinity {
    use anim::Infinity::*;
    match i {
        1 => Linear,
        3 => Cycle,
        4 => CycleRelative,
        5 => Oscillate,
        _ => Constant,
    }
}

fn register_types() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        anim::register_types();
        for name in DRIVEN_CURVES {
            if types::find(name).is_some() {
                continue;
            }
            let curve = types::find("animCurveTU").expect("animCurveTU");
            let mut own: Vec<AttrDef> = curve.attrs.iter().filter(|a| matches!(a.name.as_str(), "keys" | "preInfinity" | "postInfinity" | "output")).cloned().collect();
            own.push(AttrDef {
                name: "input".into(),
                short_name: "i".into(),
                ty: PlugType::Float,
                default_value: PlugValue::Float(0.0),
                flags: flags::INPUT | flags::STORABLE,
                doc: "The driver's value.".into(),
                ..Default::default()
            });
            let t = NodeType {
                name: name.to_string(),
                icon: "chart-line".into(),
                doc: "Keys over a driving plug's value: a set driven key.".into(),
                own_attrs: own,
                create: Some(Arc::new(|| Box::new(DrivenCurve))),
                ..Default::default()
            };
            types::register_type(t).expect("driven curve");
        }
    });
}

// ---------------------------------------------------------------------------------------------------------------------
// Tangents

/// Tangent codes in .kit and .kot and .tan, as files hold them. 0 is global, 4 the old smooth, 6 and 7 slow and fast.
fn tangent_from_code(i: i64) -> Tangent {
    match i {
        1 => Tangent::Fixed,
        2 => Tangent::Linear,
        3 => Tangent::Flat,
        6 => Tangent::Slow,
        7 => Tangent::Fast,
        4 | 9 => Tangent::Spline,
        5 => Tangent::Step,
        8 | 10 => Tangent::Clamped,
        16 => Tangent::Plateau,
        17 => Tangent::StepNext,
        _ => Tangent::Auto,
    }
}

fn tangent_code(t: Tangent) -> i64 {
    match t {
        Tangent::Fixed => 1,
        Tangent::Linear => 2,
        Tangent::Flat => 3,
        Tangent::Step => 5,
        Tangent::Spline => 9,
        Tangent::Slow => 6,
        Tangent::Fast => 7,
        Tangent::Clamped => 10,
        Tangent::Plateau => 16,
        Tangent::StepNext => 17,
        Tangent::Auto | Tangent::AutoMix | Tangent::AutoEase | Tangent::AutoCustom => 18,
    }
}

fn curve_keys(n: &MaNode, doc: &Doc, fps: f64) -> Keys {
    let linear = n.ty.ends_with('L');
    let angle = n.ty.ends_with('A');
    let vscale = if linear {
        doc.lin
    } else if angle {
        doc.ang
    } else {
        1.0
    };
    let default = n.num(&["tan", "tangentType"]).map_or(Tangent::Auto, |v| tangent_from_code(v as i64));
    let mut keys: Vec<Key> = Vec::new();
    for (at, c) in n.elements(&["ktv", "keyTimeValue"], 2) {
        let (Some(t), Some(v)) = (c[0].num(), c[1].num()) else { continue };
        if keys.len() <= at {
            keys.resize(at + 1, Key::new(0.0, 0.0, default, default));
        }
        keys[at] = Key::new(t, v * vscale, default, default);
    }
    for (at, c) in n.elements(&["kit", "keyTanInType"], 1) {
        if let (Some(k), Some(v)) = (keys.get_mut(at), c[0].num()) {
            k.itt = tangent_from_code(v as i64);
        }
    }
    for (at, c) in n.elements(&["kot", "keyTanOutType"], 1) {
        if let (Some(k), Some(v)) = (keys.get_mut(at), c[0].num()) {
            k.ott = tangent_from_code(v as i64);
        }
    }
    for (at, c) in n.elements(&["kbd", "keyBreakdown"], 1) {
        if let Some(k) = keys.get_mut(at) {
            k.breakdown = c[0].truth().unwrap_or(false);
        }
    }
    for (at, c) in n.elements(&["ktl", "keyTanLocked"], 1) {
        if let Some(k) = keys.get_mut(at) {
            k.tan_lock = c[0].truth().unwrap_or(true);
        }
    }
    for (at, c) in n.elements(&["kwl", "keyWeightLocked"], 1) {
        if let Some(k) = keys.get_mut(at) {
            k.weight_lock = c[0].truth().unwrap_or(true);
        }
    }
    // A weighted curve's tangent weights: the length of the stored vector.
    let weighted = n.truth(&["wgt", "weightedTangents"]).unwrap_or(false);
    let lengths = |xs: &[&str], ys: &[&str]| -> HashMap<usize, f64> {
        let x: HashMap<usize, f64> = n.elements(xs, 1).into_iter().filter_map(|(i, c)| Some((i, c[0].num()?))).collect();
        n.elements(ys, 1).into_iter().filter_map(|(i, c)| Some((i, (c[0].num()?.powi(2) + x.get(&i)?.powi(2)).sqrt()))).collect()
    };
    if weighted {
        for (i, w) in lengths(&["kix", "keyTanInX"], &["kiy", "keyTanInY"]) {
            if let Some(k) = keys.get_mut(i) {
                k.in_weight = w;
            }
        }
        for (i, w) in lengths(&["kox", "keyTanOutX"], &["koy", "keyTanOutY"]) {
            if let Some(k) = keys.get_mut(i) {
                k.out_weight = w;
            }
        }
    }
    // A fixed tangent's slope: y over x, x in seconds, y in internal units (radians on angle curves).
    let slopes = |xs: &[&str], ys: &[&str]| -> HashMap<usize, f64> {
        let x: HashMap<usize, f64> = n.elements(xs, 1).into_iter().filter_map(|(i, c)| Some((i, c[0].num()?))).collect();
        let mut out = HashMap::new();
        for (i, c) in n.elements(ys, 1) {
            let (Some(y), Some(&dx)) = (c[0].num(), x.get(&i)) else { continue };
            // A time curve's value is in seconds too: frames here.
            let y = if n.ty.ends_with('T') { y * fps } else { y };
            let per_second = if dx.abs() < 1e-12 { y.signum() * 1e6 } else { y / dx };
            out.insert(i, per_second / fps);
        }
        out
    };
    for (i, s) in slopes(&["kix", "keyTanInX"], &["kiy", "keyTanInY"]) {
        if let Some(k) = keys.get_mut(i) {
            k.in_slope = s;
        }
    }
    for (i, s) in slopes(&["kox", "keyTanOutX"], &["koy", "keyTanOutY"]) {
        if let Some(k) = keys.get_mut(i) {
            k.out_slope = s;
        }
    }
    let mut k = Keys { keys, ..Default::default() };
    k.sort();
    k
}

// ---------------------------------------------------------------------------------------------------------------------
// Building the scene

/// Node types read without comment: the reference's scene furniture, and nodes whose data Aztek takes from elsewhere.
const QUIET_TYPES: &[&str] = &[
    "",
    "createColorSet",
    "deleteComponent",
    "polyDelEdge",
    "groupId",
    "groupParts",
    "shadingEngine",
    "lambert",
    "blinn",
    "phong",
    "phongE",
    "anisotropic",
    "surfaceShader",
    "lightLinker",
    "materialInfo",
    "partition",
    "renderLayerManager",
    "renderLayer",
    "displayLayerManager",
    "script",
    "nodeGraphEditorInfo",
    "hyperGraphInfo",
    "hyperLayout",
    "hyperView",
    "sequenceManager",
    "sequencer",
    "shapeEditorManager",
    "poseInterpolatorManager",
    "objectSet",
    "tweak",
    "time",
    "brush",
    "reference",
    "unitConversion",
    "ikRPsolver",
    "ikSCsolver",
    "ikSplineSolver",
    "ikEffector",
    "ikSystem",
    "camera",
    "renderGlobals",
    "defaultRenderingList",
    "polyBlindData",
    "blindDataTemplate",
];

/// A renderer's nodes, which rendering being out of scope leaves behind on saving: RenderMan's, Arnold's, and the plug-ins' display helpers.
fn is_render_type(ty: &str) -> bool {
    let arnold = ty.strip_prefix("ai").is_some_and(|r| r.starts_with(|c: char| c.is_ascii_uppercase()));
    arnold || ty.starts_with("rman") || ty.starts_with("Pxr") || ty.starts_with("d_") || matches!(ty, "displayPoints" | "stereoRigCamera" | "stereoRigTransform")
}

const DEFAULT_CAMERAS: &[&str] = &["persp", "top", "front", "side", "perspShape", "topShape", "frontShape", "sideShape"];

/// What feeds a mesh shape in the file, walking its input back.
#[derive(Debug, Default)]
struct Chain {
    creator: Option<usize>,
    /// The nearest mesh upstream with stored geometry.
    base: Option<usize>,
    /// polyTweak nodes between that source and the shape, source first.
    tweaks: Vec<usize>,
    /// tweak nodes among the deformers.
    vertex_tweaks: Vec<usize>,
    /// Deformers and skins with the geometry index they output, source first.
    deformers: Vec<(usize, i64)>,
    /// Nodes on the way that cannot be replayed.
    lost: Vec<usize>,
    /// Faces and edges deleted from the stored mesh, source first.
    deletions: Vec<usize>,
    /// The reference's polySplit nodes before the deformers, source first: replayed onto the stored mesh (#180).
    pre_edits: Vec<usize>,
    /// Those between the deformers and the shape, source first: made again as history after the deformers.
    post_edits: Vec<usize>,
}

/// A polySplit node's cut as polySplit's points here: `.edge` holds each point's fraction and `.desc` the edge it is on,
/// with the top bit set (-2147483648 + the edge's number). None for a cut with a point inside a face, which isn't read.
fn file_split_points(n: &MaNode) -> Option<Vec<Value>> {
    let fractions = n.elements(&["e", "edge"], 1);
    let descs = n.elements(&["d", "desc"], 1);
    if descs.is_empty() || descs.len() != fractions.len() {
        return None;
    }
    let mut pts = Vec::new();
    for ((_, d), (_, f)) in descs.iter().zip(&fractions) {
        let code = d[0].num()? as i64;
        if code >= 0 {
            return None;
        }
        pts.push(json!({"edge": code + (1i64 << 31), "t": f[0].num()?}));
    }
    Some(pts)
}

/// Replays a file's polySplit on a mesh, as the reference made it.
fn apply_file_split(n: &MaNode, m: &mut Mesh) -> Result<(), String> {
    let pts = file_split_points(n).ok_or_else(|| format!("{}: a cut through a face isn't read", n.name))?;
    let op = crate::history::op("polySplit").ok_or("no polySplit")?;
    let mut flags = serde_json::Map::new();
    flags.insert("points".into(), Value::Array(pts));
    let args = aztek_ops::Args { positional: Vec::new(), flags };
    let run = op.run;
    crate::history::apply_edit(m, None, &Default::default(), op.kind, &mut |m, sel, sym| run(&args, m, sel, sym)).map(|_| ()).map_err(|e| format!("{}: {e}", n.name))
}

const INPUT_ATTRS: &[&str] = &["i", "inMesh", "ip", "inputPolymesh", "ig", "inputGeometry", "ip.ig", "input.inputGeometry"];

/// A deleteComponent's faces or a polyDelEdge's edges, off a mesh the file stored before them.
fn apply_deletion(n: &MaNode, m: &mut Mesh) {
    let Some(list) = n.set_of(&["dc", "deleteComponents", "ics", "inputComponents"]) else { return };
    let (mut faces, mut edges) = (Vec::new(), Vec::new());
    for t in &list.values {
        let Some(text) = t.text() else { continue };
        let (out, inner) = if let Some(x) = text.strip_prefix("f[") {
            (&mut faces, x)
        } else if let Some(x) = text.strip_prefix("e[") {
            (&mut edges, x)
        } else {
            continue;
        };
        let inner = inner.trim_end_matches(']');
        let (a, b) = inner.split_once(':').unwrap_or((inner, inner));
        if let (Ok(a), Ok(b)) = (a.parse::<u32>(), b.parse::<u32>()) {
            out.extend(a..=b);
        }
    }
    faces.retain(|&f| m.face_valid(f));
    edges.retain(|&e| e < m.edge_capacity() && m.edge_valid(e));
    aztek_geo::ops::basic::delete_faces(m, &faces, None);
    aztek_geo::ops::basic::delete_edges(m, &edges, None);
}

/// Whether a file's plug-in deformer deforms something other than curves and surfaces: its output goes anywhere but a
/// NURBS shape's create.
fn mesh_plugin_deformer(doc: &Doc, i: usize) -> bool {
    doc.conns_out(i).any(|c| {
        c.src == i && matches!(plain_attr(&c.src_attr).as_str(), "og" | "outputGeometry") && !matches!(doc.nodes[c.dst].ty.as_str(), "nurbsCurve" | "nurbsSurface")
    })
}

fn walk_chain(doc: &Doc, shape: usize, stored: &HashMap<usize, MaMesh>) -> Chain {
    let mut c = Chain::default();
    let mut seen = HashSet::new();
    let mut at = shape;
    // Which geometry of a deformer the walk came out of: that index is the way back.
    let mut through = 0;
    while seen.insert(at) {
        let plain = |c: &&Conn| c.dst == at && INPUT_ATTRS.contains(&plain_attr(&c.dst_attr).as_str());
        let by_index = doc.conns_in(at).filter(plain).find(|c| first_index(&c.dst_attr).unwrap_or(0) == through);
        let Some(conn) = by_index.or_else(|| doc.conns_in(at).find(plain)) else { break };
        let next = conn.src;
        let index = first_index(&conn.src_attr).unwrap_or(0);
        // A plug-in's deformer on a mesh is in the chain like any other.
        let file_deformer = |t: &str| deformers::is_file_deformer(t) || crate::scripted::is_plugin_deformer(t);
        through = if file_deformer(&doc.nodes[next].ty) || doc.nodes[next].ty == "skinCluster" || doc.nodes[next].ty == "tweak" { index } else { 0 };
        let n = &doc.nodes[next];
        match n.ty.as_str() {
            "mesh" => {
                if stored.get(&next).is_some_and(MaMesh::has_geometry) {
                    c.base = Some(next);
                    break;
                }
            }
            // Under a deformer it is in the chain; `walk_chain` moves any with no deformer upstream back to the base.
            "polyTweak" => c.deformers.insert(0, (next, index)),
            "skinCluster" => c.deformers.insert(0, (next, index)),
            // Freeze Transformations' move on a mesh with history.
            "transformGeometry" => c.deformers.insert(0, (next, 0)),
            t if file_deformer(t) => c.deformers.insert(0, (next, index)),
            "tweak" => c.vertex_tweaks.push(next),
            // A colour set changes no points.
            "groupParts" | "groupId" | "unitConversion" | "createColorSet" => {}
            "deleteComponent" | "polyDelEdge" => c.deletions.insert(0, next),
            // Cuts are replayed: before the deformers onto the stored mesh, after them as history (#180).
            "polySplit" if file_split_points(n).is_some() => {
                if c.deformers.is_empty() {
                    c.post_edits.insert(0, next)
                } else {
                    c.pre_edits.insert(0, next)
                }
            }
            t if history_io::creator_type(t) => {
                c.creator = Some(next);
                break;
            }
            _ => c.lost.push(next),
        }
        at = next;
    }
    while c.deformers.first().is_some_and(|d| doc.nodes[d.0].ty == "polyTweak") {
        let (t, _) = c.deformers.remove(0);
        c.tweaks.push(t);
    }
    c
}

/// A chain node made for one geometry index of a file's deformer.
#[derive(Clone, Copy)]
struct FileDeformer {
    doc: usize,
    geometry: i64,
    id: NodeId,
    shape: NodeId,
}

/// Whether an objectSet in a file is one Aztek reads as a set of its own: not a deformer's set (one `usedBy` names) and
/// not the scene's default sets.
fn plain_object_set(doc: &Doc, i: usize) -> bool {
    let n = &doc.nodes[i];
    n.ty == crate::object_sets::TYPE_NAME
        && !matches!(n.name.trim_start_matches(':'), "defaultLightSet" | "defaultObjectSet")
        && !doc.conns_in(i).any(|c| c.dst == i && matches!(plain_attr(&c.dst_attr).as_str(), "ub" | "usedBy"))
}

struct Build<'a> {
    deformers: Vec<FileDeformer>,
    /// Shapes with the file's polySplit nodes after their deformers, replayed once the scene is read.
    post_edits: Vec<(NodeId, Vec<usize>)>,
    s: &'a mut Session,
    doc: &'a Doc,
    ids: Vec<Option<NodeId>>,
    /// Handled here: not reported as unrepresented.
    used: Vec<bool>,
    meshes: HashMap<usize, MaMesh>,
    warnings: Vec<String>,
}

impl Build<'_> {
    fn g(&mut self) -> &mut Graph {
        &mut self.s.graph
    }

    /// A plug-in's deformer on a curve, as the reference writes one: `deformer.og[k]` into the curve's create, and its
    /// `ip[k].ig` fed by the original shape (an intermediate curve), maybe through the groupParts and tweak nodes the
    /// reference puts between. Here the deformer keeps the original curve itself, so the original shape is removed.
    fn curve_deformers(&mut self) {
        let doc = self.doc;
        let feeding = |node: usize, attrs: &[&str]| doc.conns_in(node).find(|c| c.dst == node && attrs.contains(&plain_attr(&c.dst_attr).as_str())).map(|c| (c.src, plain_attr(&c.src_attr)));
        for c in &doc.conns {
            if !matches!(plain_attr(&c.src_attr).as_str(), "og" | "outputGeometry") || !matches!(plain_attr(&c.dst_attr).as_str(), "cr" | "create") {
                continue;
            }
            let (Some(def), Some(shape)) = (self.id(c.src), self.id(c.dst)) else { continue };
            let g = &self.s.graph;
            let Some(dt) = g.node(def).map(|n| n.type_arc().clone()) else { continue };
            if !crate::scripted::is_plugin_deformer(&dt.name) || g.node(shape).is_none_or(|n| n.ty().name != crate::curves::SHAPE_TYPE) {
                continue;
            }
            let (Some(inp), Some(out)) = (dt.attr_index("inputCurve"), dt.attr_index("outputCurve")) else { continue };
            // Up from the deformer's input to a curve shape, through groupParts and tweak nodes.
            let mut at = feeding(c.src, &["ip.ig", "input.inputGeometry"]);
            let mut orig = None;
            for _ in 0..8 {
                let Some((n, _)) = at else { break };
                if doc.nodes[n].ty == crate::curves::SHAPE_TYPE {
                    orig = Some(n);
                    break;
                }
                at = feeding(n, &["ig", "inputGeometry", "ip.ig", "input.inputGeometry"]);
            }
            let Some(orig) = orig.and_then(|o| self.id(o)) else {
                self.warnings.push(format!("{}: no original curve found for it", doc.nodes[c.src].name));
                continue;
            };
            let g = &mut self.s.graph;
            let Some(curve) = crate::curves::local_curve(g, orig) else { continue };
            let done = g.set_value(PlugRef::new(def, inp as i32), crate::curves::to_value(curve)).and_then(|_| g.connect(&PlugRef::new(def, out as i32), &PlugRef::new(shape, crate::curves::CurveShape::CREATE as i32), true));
            if let Err(e) = done {
                self.warnings.push(format!("{}: {e}", doc.nodes[c.src].name));
                continue;
            }
            if orig != shape && g.node(orig).is_some_and(|n| n.bool(DagNode::INTERMEDIATE_OBJECT)) && g.connections().iter().all(|(_, s)| s.node != orig) {
                let _ = g.delete_node(orig);
            }
        }
    }

    /// The file's object sets and what each holds: objects through `dagSetMembers` (a DAG node's instObjGroups),
    /// other nodes and sets through `dnSetMembers`. Made empty first, so a set can hold one written after it. A quick
    /// select set is one annotated gCharacterSet, as the reference marks them.
    fn read_object_sets(&mut self) {
        let doc = self.doc;
        for i in 0..doc.nodes.len() {
            if self.ids[i].is_some() || !plain_object_set(doc, i) {
                continue;
            }
            let quick = doc.nodes[i].text(&["an", "annotation"]) == Some("gCharacterSet");
            match crate::object_sets::create(&mut self.s.graph, doc.nodes[i].name.trim_start_matches(':'), &[], quick) {
                Ok(id) => {
                    self.ids[i] = Some(id);
                    self.used[i] = true;
                }
                Err(e) => self.warnings.push(format!("{}: {e}", doc.nodes[i].name)),
            }
        }
        let mut held: BTreeMap<NodeId, Vec<crate::object_sets::Member>> = BTreeMap::new();
        for c in &doc.conns {
            if !plain_object_set(doc, c.dst) {
                continue;
            }
            let into = plain_attr(&c.dst_attr);
            if !matches!(into.as_str(), "dsm" | "dagSetMembers" | "dnsm" | "dnSetMembers") {
                continue;
            }
            let (Some(set), Some(member)) = (self.id(c.dst), self.id(c.src)) else { continue };
            if !self.s.graph.node(set).is_some_and(crate::object_sets::is_set) {
                continue;
            }
            let list = held.entry(set).or_default();
            if !list.iter().any(|m| m.node == member) {
                list.push(crate::object_sets::Member::object(member));
            }
        }
        for (set, list) in held {
            if let Err(e) = crate::object_sets::add(&mut self.s.graph, set, &list) {
                self.warnings.push(format!("{}: {e}", self.s.graph.path(set)));
            }
        }
    }

    fn id(&self, node: usize) -> Option<NodeId> {
        self.ids[node]
    }

    fn nearest_parent(&mut self, node: usize) -> NodeId {
        if let Some(p) = &self.doc.nodes[node].ext_parent {
            let prefix = crate::references::reading_namespace(self.s);
            let named = crate::references::prefix_path(p, &prefix);
            if let Ok(Some(id)) = self.s.graph.find(&named) {
                return id;
            }
        }
        let mut at = self.doc.nodes[node].parent;
        while let Some(p) = at {
            if self.ids[p].is_none() && self.keeps(p) {
                self.create_dag(p);
            }
            if let Some(id) = self.ids[p] {
                return id;
            }
            at = self.doc.nodes[p].parent;
        }
        NO_NODE
    }

    /// DAG nodes built as themselves. Meshes marked intermediate are consumed by the mesh pass.
    fn keeps(&self, i: usize) -> bool {
        let n = &self.doc.nodes[i];
        if self.used[i] && (n.ty == "transform" || n.ty == "camera") {
            return false;
        }
        match n.ty.as_str() {
            "transform" | "joint" | "locator" | "nurbsCurve" | "nurbsSurface" | "camera" => true,
            crate::core_nodes::UNKNOWN_DAG | crate::core_nodes::UNKNOWN_TRANSFORM => true,
            // A deformer's handle shape.
            ty if crate::deform::handle_shape_kind(ty).is_some() => true,
            ty if PLAIN_SHAPES.contains(&ty) || crate::lights::is_light_type(ty) => true,
            "mesh" => !n.truth(&["io", "intermediateObject"]).unwrap_or(false),
            // A plug-in's locator (mGearWeightDriver).
            ty => crate::scripted::plugin_of_type(ty).is_some() && types::find(ty).is_some_and(|t| t.is_a("locator")),
        }
    }

    fn create_dag(&mut self, i: usize) {
        if self.ids[i].is_some() || !self.keeps(i) {
            return;
        }
        let parent = self.nearest_parent(i);
        let n = &self.doc.nodes[i];
        let Some(t) = types::find(&n.ty) else { return };
        let name = if Graph::valid_name(&n.name) { n.name.clone() } else { String::new() };
        let is_shape = t.is_a("shape");
        let r = (|| -> Result<NodeId, String> {
            let g = &mut self.s.graph;
            // A shape with no transform above it gets one, as the reference gives it.
            let parent = if is_shape && parent == NO_NODE {
                let tt = types::find("transform").unwrap();
                let stem = crate::dag::transform_stem(&n.ty).to_string();
                g.create_node(&tt, &format!("{stem}1"), NO_NODE, true).map_err(|e| e.to_string())?
            } else {
                parent
            };
            g.create_node(&t, &name, parent, false).map_err(|e| e.to_string())
        })();
        match r {
            Ok(id) => {
                crate::scripted::post_create_named(self.s, id);
                self.ids[i] = Some(id);
                self.used[i] = true;
            }
            Err(e) => self.warnings.push(format!("{}: {e}", n.name)),
        }
    }

    fn add_user_attrs(&mut self, i: usize) {
        let Some(id) = self.id(i) else { return };
        let n = &self.doc.nodes[i];
        for a in &n.adds {
            // The trim is read with the surface.
            if !a.parent.is_empty() || a.long == "aztekTrim" {
                continue;
            }
            // The reference adds a joint's Character on the joint, and a quick rig's info on the character.
            if (a.long == "Character" && self.s.graph.node(id).is_some_and(|x| x.ty().is_a("joint"))) || (n.ty == crate::hik::TYPE && a.long == "quickRigInfo") {
                continue;
            }
            // A default node of Aztek's may have it already (defaultRenderGlobals.defaultSurfaceShader).
            // A nonLinear has its kind's settings already, as the reference's addAttr lines give them.
            // A history node or creator has its own settings already: written as added attributes so the reference keeps them.
            let own_settings = history_io::history_type(&n.ty).is_some() || history_io::creator_type(&n.ty);
            if (crate::defaults::is_scene_node(&n.name) || n.ty == "nonLinear" || own_settings) && self.s.graph.node(id).is_some_and(|x| x.ty().attr_index(&a.long).is_some()) {
                continue;
            }
            let Some(def) = user_attr_def(a) else {
                self.warnings.push(format!("{}.{}: attribute type {}{} skipped", n.name, a.long, a.at, a.dt));
                continue;
            };
            if let Err(e) = self.g().add_attr(id, def) {
                self.warnings.push(format!("{}.{}: {e}", n.name, a.long));
            }
        }
    }

    /// The setAttr values on a built node's channels, unit-converted.
    fn apply_channels(&mut self, i: usize) {
        let Some(id) = self.id(i) else { return };
        let n = &self.doc.nodes[i];
        let t = self.s.graph.node(id).unwrap().type_arc().clone();
        for set in &n.sets {
            if set.path.len() != 1 || set.path[0].range.is_some() || set.values.is_empty() {
                continue;
            }
            let Some((attr, comp)) = channel(&t, &set.path[0].name) else { continue };
            let def = &t.attrs[attr];
            let factor = if LINEAR_CHANNELS.contains(&def.name.as_str()) {
                self.doc.lin
            } else if ANGLE_CHANNELS.contains(&def.name.as_str()) {
                self.doc.ang
            } else {
                1.0
            };
            let Some(v) = plug_value(def, comp, &set.values, factor) else { continue };
            if let Err(e) = self.s.graph.set_value(plug_ref(id, attr, comp), v) {
                self.warnings.push(format!("{}.{}: {e}", n.name, set.path[0].name));
            }
        }
    }

    /// Whether a setAttr names an attribute the node has, with a value Aztek reads.
    fn models(&self, node_path: &str, set: &SetAttr) -> bool {
        let text: Vec<String> = set
            .path
            .iter()
            .map(|seg| match seg.range {
                Some(r) => format!("{}[{}]", seg.name, r.0),
                None => seg.name.clone(),
            })
            .collect();
        let Ok(p) = self.s.graph.plug(&format!("{node_path}.{}", text.join("."))) else { return false };
        let def = self.s.graph.attr(&p);
        if !def.has(flags::INPUT) {
            // An output a file saves (time1.outTime): computed here, nothing to keep.
            return true;
        }
        let (lo, hi) = set.path.last().and_then(|s| s.range).or(set.path.first().and_then(|s| s.range)).unwrap_or((-1, -1));
        let stride = (set.values.len() / (hi - lo + 1).max(1) as usize).max(1);
        plug_value(def, p.component, &set.values[..stride.min(set.values.len())], 1.0).is_some()
    }

    /// An element of a typed array attribute that is itself an array in the reference (a poseInterpolator's
    /// pose[i].poseRotation[driver]), held here in one array: each element's numbers in its place, in index order.
    fn typed_array_element(&mut self, node_path: &str, text: &[String], chunk: &[Tok]) -> bool {
        let Some(last) = text.last() else { return false };
        let Some(open) = last.rfind('[') else { return false };
        let Ok(j) = last[open + 1..last.len().saturating_sub(1)].parse::<usize>() else { return false };
        let mut head = text.to_vec();
        *head.last_mut().unwrap() = last[..open].to_string();
        let Ok(p) = self.s.graph.plug(&format!("{node_path}.{}", head.join("."))) else { return false };
        let def = self.s.graph.attr(&p).clone();
        let Some(kind) = crate::array_data::of(&def.default_value).map(|d| d.kind.clone()) else { return false };
        let Some(PlugValue::Data(Some(nd))) = plug_value(&def, p.component, chunk, 1.0) else { return false };
        let Some(new) = nd.as_any().downcast_ref::<crate::array_data::ArrayData>() else { return false };
        let mut have = self.s.graph.value(&p).ok().and_then(|v| crate::array_data::of(&v).cloned()).unwrap_or_else(|| crate::array_data::ArrayData::empty(&kind));
        let w = new.nums.len();
        if w == 0 {
            return true;
        }
        if have.nums.len() < (j + 1) * w {
            have.nums.resize((j + 1) * w, 0.0);
        }
        have.nums[j * w..(j + 1) * w].copy_from_slice(&new.nums);
        self.s.graph.set_value(p, have.value()).is_ok()
    }

    /// A utility node or expression: made as it is, its setAttrs applied by name. Attributes Aztek's node doesn't have
    /// (The reference's nodeState and the like) are passed over.
    fn create_plain(&mut self, i: usize) {
        let n = &self.doc.nodes[i];
        // A plug-in's deformer on a mesh is made with the mesh's chain; on a curve it is a plain node.
        if crate::scripted::is_plugin_deformer(&n.ty) && self.ids[i].is_none() && mesh_plugin_deformer(self.doc, i) {
            return;
        }
        self.used[i] = true;
        let id = match self.ids[i] {
            // A shape create_dag made under its transform, or a default node.
            Some(id) => id,
            None => {
                let Some(t) = types::find(&n.ty) else { return };
                match self.s.graph.create_node(&t, &n.name, NO_NODE, false) {
                    Ok(id) => {
                        // A compiled plug-in's node: its postConstructor before the file sets anything, under the file's name.
                        crate::scripted::post_create_named(self.s, id);
                        id
                    }
                    Err(e) => {
                        self.warnings.push(format!("{}: {e}", n.name));
                        return;
                    }
                }
            }
        };
        self.ids[i] = Some(id);
        let node_path = self.s.graph.path(id);
        let keeps = crate::defaults::is_scene_node(&n.name);
        for set in &n.sets {
            if set.values.is_empty() {
                continue;
            }
            if keeps && !self.models(&node_path, set) {
                let (key, text) = kept_line(set);
                self.s.graph.keep_default_line(&n.name, &key, &text);
                continue;
            }
            // One range may hold several elements: .i[0:2] 1 2 3.
            let (lo, hi) = set.path.last().and_then(|s| s.range).or(set.path.first().and_then(|s| s.range)).unwrap_or((-1, -1));
            let count = (hi - lo + 1).max(1) as usize;
            let stride = (set.values.len() / count).max(1);
            for (k, chunk) in set.values.chunks(stride).enumerate().take(count) {
                let text: Vec<String> = set
                    .path
                    .iter()
                    .map(|seg| match seg.range {
                        Some(r) => format!("{}[{}]", seg.name, r.0 + k as i64),
                        None => seg.name.clone(),
                    })
                    .collect();
                let Ok(p) = self.s.graph.plug(&format!("{node_path}.{}", text.join("."))) else {
                    if !self.typed_array_element(&node_path, &text, chunk) {
                        self.note_drop(&n.ty, &text.join("."), "setAttr");
                    }
                    continue;
                };
                let def = self.s.graph.attr(&p).clone();
                // The reference saves what a layer's blend last put out; here it is computed.
                // The reference saves outputs' last values too (a composeMatrix's outputMatrix); here they are computed.
                if !def.has(flags::INPUT) {
                    continue;
                }
                let Some(v) = plug_value(&def, p.component, chunk, 1.0) else { continue };
                if n.ty == "expression" && matches!(self.s.graph.attr(&p).name.as_str(), "expression") {
                    if let PlugValue::String(src) = &v {
                        if let Err(e) = crate::expression::parse(src) {
                            self.warnings.push(format!("{}: {e}", n.name));
                        }
                    }
                }
                if let Err(e) = self.s.graph.set_value(p, v) {
                    self.warnings.push(format!("{}.{}: {e}", n.name, text.join(".")));
                }
            }
        }
    }

    /// A connection with a utility node or expression at either end.
    /// With AZTEK_MA_DROPS set, names what a file held that was not read. For finding what a rig needs.
    fn note_drop(&self, ty: &str, attr: &str, how: &str) {
        if std::env::var_os("AZTEK_MA_DROPS").is_some() {
            let attr: String = attr.chars().map(|c| if c.is_ascii_digit() { '#' } else { c }).collect();
            eprintln!("DROP {how} {ty}.{attr}");
        }
    }

    fn plain_connection(&mut self, c: &Conn, sn: usize, sa: &str, src: NodeId, dst: NodeId) {
        // A nonlinear deformer's settings into its handle's shape, which here are one node: nothing to connect.
        if src == dst && plain_attr(sa) == plain_attr(&c.dst_attr) {
            return;
        }
        // An extrude's profile and path are the first two of its curves here.
        let to = match (self.doc.nodes[c.dst].ty.as_str(), c.dst_attr.as_str()) {
            ("extrude", "pr" | "profile") => "ic[0]",
            ("extrude", "pt" | "path") => "ic[1]",
            (_, attr) => attr,
        };
        let (a, b) = (format!("{}.{sa}", self.s.graph.path(src)), format!("{}.{to}", self.s.graph.path(dst)));
        let plugs = self.s.graph.plug(&a).and_then(|x| self.s.graph.plug(&b).map(|y| (x, y)));
        let r = plugs.and_then(|(x, y)| self.s.graph.connect(&x, &y, true));
        if let Err(e) = r {
            self.warnings.push(format!("{}.{sa} -> {}.{}: {e}", self.doc.nodes[sn].name, self.doc.nodes[c.dst].name, c.dst_attr));
        }
    }

    fn apply_locks(&mut self, i: usize) {
        let Some(id) = self.id(i) else { return };
        let n = &self.doc.nodes[i];
        let t = self.s.graph.node(id).unwrap().type_arc().clone();
        for set in &n.sets {
            let (Some(lock), [seg]) = (set.lock, set.path.as_slice()) else { continue };
            if let Some((attr, comp)) = channel(&t, &seg.name) {
                let _ = self.s.graph.set_locked(&plug_ref(id, attr, comp), lock);
            }
        }
    }

    fn create_generator(&mut self, i: usize) -> Option<NodeId> {
        if let Some(id) = self.id(i) {
            return Some(id);
        }
        let n = &self.doc.nodes[i];
        let t = types::find(&n.ty)?;
        let name = if Graph::valid_name(&n.name) { n.name.clone() } else { String::new() };
        let id = self.s.graph.create_node(&t, &name, NO_NODE, false).ok()?;
        self.ids[i] = Some(id);
        self.used[i] = true;
        self.apply_channels(i);
        self.history_settings(i, id, &t);
        if let Some(ax) = n.vec3(&["ax", "axis"]) {
            let k = (0..3).max_by(|&a, &b| ax[a].abs().total_cmp(&ax[b].abs())).unwrap();
            let index = if ax[k] < 0.0 { k + 3 } else { k } as i64;
            if let Some(a) = t.attr_index("upAxis") {
                let _ = self.s.graph.set_value(PlugRef::new(id, a as i32), PlugValue::Int(index));
            }
        }
        Some(id)
    }

    /// Geometry for a mesh shape: its source plug for a skinCluster or the shape, or a mesh to store.
    /// A curve from `.cc`, or from the makeNurbCircle feeding it when it has no stored curve.
    fn build_curve(&mut self, i: usize) {
        use aztek_geo::nurbs::{self, Form, NurbsCurve};
        let Some(shape) = self.id(i) else { return };
        if self.chain_nurbs(i, crate::curves::CurveShape::LOCAL, crate::curves::CurveShape::CREATE) {
            return;
        }
        // Fed straight by another curve, as an original with its deformers taken off leaves it.
        if let Some(src) = self.doc.input(i, &["cr", "create"]).filter(|c| self.doc.nodes[c.src].ty == "nurbsCurve").and_then(|c| self.id(c.src)) {
            let _ = self.s.graph.connect(&PlugRef::new(src, crate::curves::CurveShape::LOCAL as i32), &PlugRef::new(shape, crate::curves::CurveShape::CREATE as i32), false);
        }
        let doc = self.doc;
        let n = &doc.nodes[i];
        let mut curve = None;
        if let Some(cc) = n.set_of(&["cc", "cached"]) {
            // The rational flag is a word: only the numbers count here.
            let nums: Vec<f64> = cc.values.iter().filter_map(|t| if let Tok::Num(v) = t { Some(*v) } else { None }).collect();
            // degree spans form [rational, a word] dimension knotCount knots... cvCount cvs...
            if nums.len() >= 6 {
                let (degree, form) = (nums[0] as usize, nums[2] as i64);
                let dim = nums[3] as usize;
                let nk = nums[4] as usize;
                if nums.len() > 5 + nk {
                    let knots = nums[5..5 + nk].to_vec();
                    let nc = nums[5 + nk] as usize;
                    let start = 6 + nk;
                    let stride = dim.max(2);
                    let mut cvs = Vec::with_capacity(nc);
                    for k in 0..nc {
                        let at = start + k * stride;
                        if at + stride > nums.len() {
                            break;
                        }
                        cvs.push(if stride == 2 { DVec3::new(nums[at], nums[at + 1], 0.0) } else { DVec3::new(nums[at], nums[at + 1], nums[at + 2]) });
                    }
                    curve = Some(NurbsCurve::new(degree, Form::from_index(form), cvs, knots));
                }
            }
        }
        if curve.is_none() {
            if let Some(c) = doc.input(i, &["cr", "create"]) {
                let g = &doc.nodes[c.src];
                // One in the scene by now feeds the shape itself.
                if g.ty == "makeNurbCircle" && self.id(c.src).is_none() {
                    let nr = g.vec3(&["nr", "normal"]).unwrap_or(DVec3::Z);
                    let cen = g.vec3(&["c", "center"]).unwrap_or(DVec3::ZERO);
                    let fp = g.vec3(&["fp", "firstPoint"]).unwrap_or(DVec3::X);
                    let r = g.num(&["r", "radius"]).unwrap_or(1.0);
                    let deg = g.num(&["d", "degree"]).unwrap_or(3.0) as usize;
                    let sec = g.num(&["s", "sections"]).unwrap_or(8.0) as usize;
                    let sw = g.num(&["sw", "sweep"]).unwrap_or(360.0);
                    curve = Some(nurbs::circle(cen, nr, fp, r, deg, sec, sw));
                }
            }
        }
        let Some(c) = curve else {
            // History that is in the scene gives the curve by connection.
            // A file can hold a curve shape with no geometry at all, and the reference keeps it that way: that is nothing to warn about.
            if doc.input(i, &["cr", "create"]).is_some_and(|c| self.id(c.src).is_none()) {
                self.warnings.push(format!("{}: no stored curve", n.name));
            }
            return;
        };
        let at = crate::curves::CurveShape::CACHED;
        let _ = self.s.graph.set_value(PlugRef::new(shape, at as i32), PlugValue::Data(Some(Arc::new(crate::curves::CurveData(c)))));
    }

    /// A surface from `.cc`. History with no stored surface is a warning: the reference always writes one for a shape it can't rebuild.
    fn build_surface(&mut self, i: usize) {
        use aztek_geo::nurbs::{Form, NurbsSurface};
        let Some(shape) = self.id(i) else { return };
        if self.chain_nurbs(i, crate::surfaces::SurfaceShape::LOCAL, crate::surfaces::SurfaceShape::CREATE) {
            return;
        }
        let n = &self.doc.nodes[i];
        let mut made = None;
        if let Some(cc) = n.set_of(&["cc", "cached"]) {
            let v: Vec<f64> = cc.values.iter().filter_map(|t| if let Tok::Num(x) = t { Some(*x) } else { None }).collect();
            // du dv fu fv [rational, a word] nku ku... nkv kv... ncv cvs...
            if v.len() > 5 {
                let nku = v[4] as usize;
                let kv_at = 5 + nku;
                if v.len() > kv_at {
                    let nkv = v[kv_at] as usize;
                    let cv_at = kv_at + 1 + nkv;
                    if v.len() > cv_at {
                        let (du, dv) = (v[0] as usize, v[1] as usize);
                        let (nu, nv) = (nku + 1 - du, nkv + 1 - dv);
                        let count = v[cv_at] as usize;
                        let stride = if count > 0 && v.len() >= cv_at + 1 + count * 3 { 3 } else { 0 };
                        if stride == 3 && count == nu * nv {
                            // U is the outer loop in the file, as in NurbsSurface.
                            let cvs: Vec<DVec3> = v[cv_at + 1..cv_at + 1 + count * 3].chunks(3).map(|c| DVec3::new(c[0], c[1], c[2])).collect();
                            let trim = n.text(&["aztekTrim"]).and_then(|t| serde_json::from_str::<Value>(t).ok()).map(|j| crate::surfaces::trim_from_json(&j)).unwrap_or_default();
                            made = Some(NurbsSurface { degree_u: du, degree_v: dv, form_u: Form::from_index(v[2] as i64), form_v: Form::from_index(v[3] as i64), nu, nv, cvs, knots_u: v[5..kv_at].to_vec(), knots_v: v[kv_at + 1..cv_at].to_vec(), trim });
                        }
                    }
                }
            }
        }
        let Some(c) = made else {
            if !self.doc.input(i, &["cr", "create"]).is_some_and(|c| self.id(c.src).is_some()) {
                self.warnings.push(format!("{}: no stored surface", n.name));
            }
            return;
        };
        let at = crate::surfaces::SurfaceShape::CACHED;
        let _ = self.s.graph.set_value(PlugRef::new(shape, at as i32), PlugValue::Data(Some(Arc::new(crate::surfaces::SurfaceData(c)))));
    }

    fn build_camera(&mut self, i: usize) {
        use crate::cameras as cam;
        let Some(shape) = self.id(i) else { return };
        let n = &self.doc.nodes[i];
        for (names, at) in [(["fl", "focalLength"], cam::FOCAL_LENGTH), (["hfa", "horizontalFilmAperture"], cam::H_APERTURE), (["vfa", "verticalFilmAperture"], cam::V_APERTURE), (["ncp", "nearClipPlane"], cam::NEAR_CLIP), (["fcp", "farClipPlane"], cam::FAR_CLIP), (["coi", "centerOfInterest"], cam::COI), (["ow", "orthographicWidth"], cam::ORTHO_WIDTH)] {
            if let Some(v) = n.num(&names) {
                let _ = self.s.graph.set_value(PlugRef::new(shape, at as i32), PlugValue::Float(v));
            }
        }
        if let Some(o) = n.truth(&["o", "orthographic"]) {
            let _ = self.s.graph.set_value(PlugRef::new(shape, cam::ORTHOGRAPHIC as i32), PlugValue::Bool(o));
        }
        let t = self.s.graph.node(shape).unwrap().type_arc().clone();
        for (at, short) in cam::plain_attrs(&self.s.graph, shape) {
            let def = &t.attrs[at];
            let Some(set) = n.set_of(&[short.as_str(), def.name.as_str()]) else { continue };
            if let Some(v) = plug_value(def, -1, &set.values, 1.0) {
                let _ = self.s.graph.set_value(PlugRef::new(shape, at as i32), v);
            }
        }
    }

    fn build_mesh(&mut self, i: usize) {
        let Some(shape) = self.id(i) else { return };
        let doc = self.doc;
        let n = &doc.nodes[i];
        // History the file keeps is made as history, every step of it (#209).
        if let Some(o) = history_io::walk_ordered(doc, i, &self.meshes) {
            self.build_ordered(i, shape, o);
            return;
        }
        let chain = walk_chain(doc, i, &self.meshes);
        let own = self.meshes.get(&i).is_some_and(MaMesh::has_geometry);
        let in_mesh = PlugRef::new(shape, MeshNode::IN_MESH as i32);

        if !chain.lost.is_empty() && !own {
            let names: Vec<String> = chain.lost.iter().map(|&l| format!("{} ({})", doc.nodes[l].name, doc.nodes[l].ty)).collect();
            self.warnings.push(format!("{}: no stored geometry past {}; shown as before them", n.name, names.join(", ")));
        }

        // What the mesh is built from.
        enum Source {
            Stored(Mesh),
            Creator(usize),
            Nothing,
        }
        let tweaked = |this: &mut Self, base: usize, tweaks: &[usize]| -> Mesh {
            this.used[base] = true;
            let mut warnings = Vec::new();
            let mut m = this.meshes[&base].build(&doc.nodes[base].name, &mut warnings);
            apply_file_tags(&doc.nodes[base], &mut m);
            if base != i {
                apply_file_tags(&doc.nodes[i], &mut m);
            }
            this.warnings.append(&mut warnings);
            apply_tweaks(doc, &mut m, tweaks);
            for &t in tweaks {
                this.used[t] = true;
            }
            m
        };
        let chained = !chain.deformers.is_empty();
        let source = if !chained && own {
            Source::Stored(tweaked(self, i, &[]))
        } else if let Some(base) = chain.base {
            let mut m = tweaked(self, base, &chain.tweaks);
            for &t in &chain.vertex_tweaks {
                deformers::apply_vertex_tweaks(doc, &mut m, t);
            }
            for &k in &chain.deletions {
                apply_deletion(&doc.nodes[k], &mut m);
            }
            // The cuts before the deformers; with no deformers every cut, since the shape shows them all (#180).
            let baked = if chained { chain.pre_edits.clone() } else { chain.post_edits.clone() };
            for e in baked {
                self.used[e] = true;
                if let Err(w) = apply_file_split(&doc.nodes[e], &mut m) {
                    self.warnings.push(w);
                }
            }
            Source::Stored(m)
        } else if let Some(c) = chain.creator {
            if !chain.tweaks.is_empty() {
                let names: Vec<&str> = chain.tweaks.iter().map(|&t| doc.nodes[t].name.as_str()).collect();
                self.warnings.push(format!("{}: {} not applied to a {}, whose point order may differ", n.name, names.join(", "), doc.nodes[c].ty));
            }
            Source::Creator(c)
        } else if own {
            Source::Stored(tweaked(self, i, &[]))
        } else {
            Source::Nothing
        };

        if !chained {
            match source {
                Source::Stored(m) => {
                    let _ = self.s.graph.set_value(in_mesh, PlugValue::Mesh(Some(Arc::new(m))));
                }
                Source::Creator(c) => {
                    if let Some(gen) = self.create_generator(c) {
                        let out = self.s.graph.node(gen).unwrap().ty().attr_index("outMesh").unwrap();
                        if let Err(e) = self.s.graph.connect(&PlugRef::new(gen, out as i32), &in_mesh, false) {
                            self.warnings.push(format!("{}: {e}", n.name));
                        }
                    }
                }
                Source::Nothing => self.warnings.push(format!("{}: no geometry found", n.name)),
            }
            return;
        }

        // Deformed: what the deformers read goes on an intermediate shape (or stays a creator) that feeds the first.
        let src = match source {
            Source::Stored(m) => {
                let parent = self.s.graph.parent(shape);
                let name = chain.base.map_or_else(|| format!("{}Orig", n.name), |b| doc.nodes[b].name.clone());
                let mt = types::find("mesh").unwrap();
                let Ok(orig) = self.s.graph.create_node(&mt, &name, parent, false) else { return };
                if let Some(b) = chain.base {
                    self.ids[b] = Some(orig);
                }
                let g = &mut self.s.graph;
                let _ = g.set_value(PlugRef::new(orig, DagNode::INTERMEDIATE_OBJECT as i32), PlugValue::Bool(true));
                let _ = g.set_value(PlugRef::new(orig, MeshNode::IN_MESH as i32), PlugValue::Mesh(Some(Arc::new(m))));
                Some(PlugRef::new(orig, MeshNode::OUT_MESH as i32))
            }
            Source::Creator(c) => self.create_generator(c).map(|gen| {
                let out = self.s.graph.node(gen).unwrap().ty().attr_index("outMesh").unwrap();
                PlugRef::new(gen, out as i32)
            }),
            Source::Nothing => {
                self.warnings.push(format!("{}: no bind geometry found", n.name));
                None
            }
        };
        let Some(mut prev) = src else { return };
        for &(d, k) in &chain.deformers {
            let made = match doc.nodes[d].ty.as_str() {
                skin::SKIN_TYPE => self.create_skin(d),
                crate::history::TWEAK_TYPE => self.create_chain_tweak(d),
                // Made with the plain nodes, matrix and all.
                "transformGeometry" => {
                    self.used[d] = true;
                    self.ids[d]
                }
                _ => self.create_deformer(d, k, shape),
            };
            let Some(id) = made else { continue };
            let t = self.s.graph.node(id).unwrap().type_arc().clone();
            let (ip, out) = if t.name == "transformGeometry" {
                (t.attr_index("inputGeometry").unwrap(), t.attr_index("outputGeometry").unwrap())
            } else {
                (t.attr_index("inputPolymesh").unwrap(), t.attr_index("output").unwrap())
            };
            let _ = self.s.graph.connect(&prev, &PlugRef::new(id, ip as i32), false);
            prev = PlugRef::new(id, out as i32);
        }
        let _ = self.s.graph.connect(&prev, &in_mesh, false);
        // The cuts made after the deformers, history after them again once everything is read (#180).
        if !chain.post_edits.is_empty() {
            for &e in &chain.post_edits {
                self.used[e] = true;
            }
            self.post_edits.push((shape, chain.post_edits.clone()));
        }
    }

    /// The reference's cuts after a mesh's deformers, as polySplit history on the mesh, named as in the file.
    fn replay_post_edits(&mut self) {
        let doc = self.doc;
        for (shape, edits) in std::mem::take(&mut self.post_edits) {
            for e in edits {
                let n = &doc.nodes[e];
                let Some(pts) = file_split_points(n) else { continue };
                let path = self.s.graph.path(shape);
                match self.s.run("polySplit", json!({"args": [path], "points": pts})) {
                    Ok(made) => {
                        let made = made.as_array().and_then(|a| a.first()).and_then(Value::as_str).map(str::to_string).or_else(|| made.as_str().map(str::to_string));
                        if let Some(id) = made.and_then(|m| self.s.graph.find(&m).ok().flatten()) {
                            self.ids[e] = Some(id);
                            if Graph::valid_name(&n.name) {
                                let _ = self.s.graph.rename(id, &n.name);
                            }
                        }
                    }
                    Err(err) => self.warnings.push(format!("{}: {err}", n.name)),
                }
            }
        }
    }

    fn create_skin(&mut self, sk: usize) -> Option<NodeId> {
        let doc = self.doc;
        let n = &doc.nodes[sk];
        let t = types::find(skin::SKIN_TYPE).unwrap();
        let at = |name: &str| t.attr_index(name).unwrap();
        let name = if Graph::valid_name(&n.name) { n.name.clone() } else { String::new() };
        let id = self.s.graph.create_node(&t, &name, NO_NODE, false).ok()?;
        self.ids[sk] = Some(id);
        self.used[sk] = true;
        let g = &mut self.s.graph;
        if let Some(v) = n.num(&["en", "envelope"]) {
            let _ = g.set_value(PlugRef::new(id, at("envelope") as i32), PlugValue::Float(v));
        }
        if let Some(v) = n.num(&["skm", "skinningMethod"]) {
            // -1 is "classic linear, weights blended" to the reference; here linear.
            let _ = g.set_value(PlugRef::new(id, at("skinningMethod") as i32), PlugValue::Int((v as i64).max(0)));
        }
        if let Some(v) = n.num(&["mi", "maxInfluences"]) {
            let _ = g.set_value(PlugRef::new(id, at("maxInfluences") as i32), PlugValue::Int(v as i64));
        }
        if let Some(v) = n.truth(&["mmi", "maintainMaxInfluences"]) {
            let _ = g.set_value(PlugRef::new(id, at("maintainMaxInfluences") as i32), PlugValue::Bool(v));
        }
        if let Some(v) = n.num(&["dr", "dropoffRate"]) {
            let _ = g.set_value(PlugRef::new(id, at("dropoffRate") as i32), PlugValue::Float(v));
        }
        if let Some(v) = n.num(&["nw", "normalizeWeights"]) {
            let _ = g.set_value(PlugRef::new(id, at("normalizeWeights") as i32), PlugValue::Int(v as i64));
        }
        Some(id)
    }

    /// Influences, bind matrices and weights: after the skeleton exists and has its values.
    fn fill_skin(&mut self, sk: usize) {
        let Some(id) = self.id(sk) else { return };
        let doc = self.doc;
        let n = &doc.nodes[sk];
        let t = types::find(skin::SKIN_TYPE).unwrap();
        let ma = t.attr_index("matrix").unwrap();
        let mut infs: BTreeMap<i64, usize> = BTreeMap::new();
        for c in doc.conns_in(sk).filter(|c| c.dst == sk && matches!(plain_attr(&c.dst_attr).as_str(), "ma" | "matrix")) {
            if let Some(e) = first_index(&c.dst_attr).filter(|&e| e < 1 << 16) {
                infs.insert(e, c.src);
            }
        }
        let count = infs.keys().copied().max().map_or(0, |m| m as usize + 1);
        let mut bind = SkinBind { bind_pre: vec![DMat4::IDENTITY; count], geom: n.matrix_of(&["gm", "geomMatrix"]).unwrap_or(DMat4::IDENTITY), weights: Vec::new(), locks: vec![false; count], raw_edit: false };
        for (&e, &src) in &infs {
            let Some(j) = self.id(src) else {
                self.warnings.push(format!("{}: influence {} is not in the scene", n.name, doc.nodes[src].name));
                continue;
            };
            let _ = self.s.graph.connect(&PlugRef::new(j, Transform::WORLD_MATRIX as i32), &PlugRef::element(id, ma as i32, e as i32), false);
            bind.bind_pre[e as usize] = match n.matrix_element(&["pm", "bindPreMatrix"], e) {
                Some(m) => m,
                None => rig::world(&mut self.s.graph, j).inverse(),
            };
            bind.locks[e as usize] = doc.nodes[src].truth(&["liw", "lockInfluenceWeights"]).unwrap_or(false);
        }
        // A bind pre-matrix the file connects (a skin riding a local rig) stays live.
        let pm = t.attr_index("bindPreMatrix").unwrap();
        for c in doc.conns_in(sk).filter(|c| c.dst == sk && matches!(plain_attr(&c.dst_attr).as_str(), "pm" | "bindPreMatrix")) {
            let (Some(e), Some(src)) = (first_index(&c.dst_attr).filter(|&e| e < 1 << 16), self.id(c.src)) else { continue };
            let Ok(from) = self.s.graph.plug(&format!("{}.{}", self.s.graph.path(src), c.src_attr)) else {
                self.warnings.push(format!("{}.{}: no such attribute for {}.pm[{e}]", doc.nodes[c.src].name, c.src_attr, n.name));
                continue;
            };
            let _ = self.s.graph.connect(&from, &PlugRef::element(id, pm as i32, e as i32), false);
            if let Ok(PlugValue::Matrix(m)) = self.s.graph.value(&from) {
                if (e as usize) < bind.bind_pre.len() {
                    bind.bind_pre[e as usize] = m;
                }
            }
        }
        bind.weights = read_weights(n);
        for w in &mut bind.weights {
            w.retain(|x| x.1 != 0.0 && (x.0 as usize) < count);
            w.sort_by_key(|x| x.0);
        }
        // One list per point of the bind mesh: the tools index it by point.
        let input = PlugRef::new(id, t.attr_index("inputPolymesh").unwrap() as i32);
        if let Some(m) = self.s.graph.value(&input).ok().and_then(|v| v.as_mesh().cloned()) {
            bind.weights.resize(m.point_capacity() as usize, Vec::new());
        }
        let bd = t.attr_index("bindData").unwrap();
        let _ = self.s.graph.set_value(PlugRef::new(id, bd as i32), PlugValue::Data(Some(Arc::new(bind))));
    }

    fn create_ik(&mut self, i: usize) {
        let doc = self.doc;
        let n = &doc.nodes[i];
        self.used[i] = true;
        let src_of = |attrs: &[&str]| doc.input(i, attrs).map(|c| c.src);
        let start = src_of(&["hsj", "startJoint"]);
        let effector = src_of(&["hee", "endEffector"]);
        let solver_ty = src_of(&["hsv", "ikSolver"]).map_or("ikRPsolver", |s| doc.nodes[s].ty.as_str());
        if !matches!(solver_ty, "ikRPsolver" | "ikSCsolver" | "ikSplineSolver") {
            self.warnings.push(format!("{}: {solver_ty} is not represented; handle skipped", n.name));
            return;
        }
        let (Some(start), Some(effector)) = (start, effector) else {
            self.warnings.push(format!("{}: ikHandle has no start joint or effector; skipped", n.name));
            return;
        };
        self.used[effector] = true;
        // The end joint drives the effector's translate; otherwise it is the effector's sibling joint.
        let end = doc
            .input(effector, &["tx", "ty", "tz", "t", "translate", "translateX"])
            .map(|c| c.src)
            .or_else(|| doc.nodes[effector].parent.and_then(|p| doc.children(p).find(|&k| doc.nodes[k].ty == "joint")));
        let (Some(end), Some(sid)) = (end, self.id(start)) else {
            self.warnings.push(format!("{}: can't find the chain; handle skipped", n.name));
            return;
        };
        let Some(eid) = self.id(end) else { return };
        let (sp, ep) = (self.s.graph.path(sid), self.s.graph.path(eid));
        let name = if Graph::valid_name(&n.name) { n.name.as_str() } else { "ikHandle1" };
        let mut call = json!({"startJoint": sp, "endEffector": ep, "solver": solver_ty, "name": name});
        // Spline IK: the curve wired to the handle's inCurve, if it came through.
        if solver_ty == "ikSplineSolver" {
            let curve = doc.input(i, &["ic", "inCurve"]).and_then(|c| self.id(c.src));
            match curve {
                Some(c) => {
                    call["curve"] = json!(self.s.graph.path(crate::dag::transform_of(&self.s.graph, c)));
                    call["createCurve"] = json!(false);
                }
                None => self.warnings.push(format!("{}: spline IK's curve is not in the file; one is made through the joints", n.name)),
            }
        }
        match self.s.run("ikHandle", call) {
            Ok(v) => {
                let Some(h) = v.get(0).and_then(Value::as_str).and_then(|p| self.s.graph.find(p).ok().flatten()) else { return };
                self.ids[i] = Some(h);
                // The effector keeps its name from the file, for what it connects to.
                if let Some(e) = crate::ik::effector_of(&self.s.graph, h) {
                    let name = doc.nodes[effector].name.clone();
                    if Graph::valid_name(&name) {
                        let _ = self.s.graph.rename(e, &name);
                    }
                    self.ids[effector] = Some(e);
                }
                let parent = self.nearest_parent(i);
                if parent != NO_NODE {
                    let _ = self.s.graph.set_parent(h, parent, false, usize::MAX);
                }
                // A new handle sits at the end joint; the file says where it is, and leaves out zeros.
                let g = &mut self.s.graph;
                let _ = g.set_value(PlugRef::new(h, Transform::TRANSLATE as i32), PlugValue::Vec3(DVec3::ZERO));
                let _ = g.set_value(PlugRef::new(h, Transform::ROTATE as i32), PlugValue::Vec3(DVec3::ZERO));
                self.apply_channels(i);
                // The rest reference: the constraint's rest value when one drives it, else the stored vector.
                let driven_by = doc.input(i, &["pvx", "poleVectorX", "pv", "poleVector"]).map(|c| &doc.nodes[c.src]);
                let rest = driven_by.and_then(|c| c.vec3(&["rst", "restTranslate"])).or_else(|| n.vec3(&["pv", "poleVector"]));
                if let Some(p) = rest {
                    crate::ik::set_rest_pole(&mut self.s.graph, h, p);
                }
            }
            Err(e) => self.warnings.push(format!("{}: {e}", n.name)),
        }
    }

    /// What each single chain handle's rotation means, from where the file has it and its start joint.
    fn single_chain_rests(&mut self) {
        let doc = self.doc;
        let single = |n: &MaNode| n.ty == "ikHandle" && doc.input(doc.index_of(n), &["hsv", "ikSolver"]).is_some_and(|c| doc.nodes[c.src].ty == "ikSCsolver");
        if !doc.nodes.iter().any(single) {
            return;
        }
        let handles: Vec<(NodeId, DVec3)> = (0..doc.nodes.len())
            .filter(|&i| single(&doc.nodes[i]))
            .filter_map(|i| {
                let start = doc.input(i, &["hsj", "startJoint"])?.src;
                Some((self.id(i)?, doc.nodes[start].vec3(&["r", "rotate"]).unwrap_or(DVec3::ZERO) * doc.ang))
            })
            .collect();
        // A handle can hang under a chain another of them solves: each pass settles one more link.
        for _ in 0..4 {
            self.s.graph.evaluate();
            for &(h, r) in &handles {
                crate::ik::set_rest_turn(&mut self.s.graph, h, r);
            }
        }
    }

    fn create_constraint(&mut self, i: usize, _kind: usize) {
        let doc = self.doc;
        let n = &doc.nodes[i];
        let mut targets: BTreeMap<i64, usize> = BTreeMap::new();
        for c in doc.conns_in(i).filter(|c| c.dst == i && c.dst_attr.starts_with("tg[")) {
            let leaf = plain_attr(&c.dst_attr);
            if leaf != "tg.tw" {
                if let Some(e) = first_index(&c.dst_attr) {
                    targets.entry(e).or_insert(c.src);
                }
            }
        }
        // A pole vector constraint drives its handle, whose parent inverse it reads, even when its output goes through
        // other nodes first (a blendColors switching between two pole vectors).
        let pole_handle = (n.ty == "poleVectorConstraint").then(|| doc.input(i, &["cpim", "constraintParentInverseMatrix"]).map(|c| c.src)).flatten().filter(|&h| doc.nodes[h].ty == "ikHandle");
        // The driven object: where the outputs go, else what its parent matrix came from, else its DAG parent.
        let driven = pole_handle.or_else(|| doc
            .conns_out(i)
            .find(|c| c.src == i && matches!(plain_attr(&c.src_attr).as_str(), "ctx" | "cty" | "ctz" | "ct" | "crx" | "cry" | "crz" | "cr" | "csx" | "csy" | "csz" | "cs"))
            .map(|c| c.dst)
            // A constraint blended with animation goes through a pairBlend; what it drives is what that drives.
            .map(|d| if doc.nodes[d].ty == "pairBlend" { doc.conns_out(d).find(|c| c.src == d && matches!(plain_attr(&c.src_attr).as_str(), "ot" | "otx" | "oty" | "otz" | "or" | "orx" | "ory" | "orz" | "outTranslate" | "outTranslateX" | "outTranslateY" | "outTranslateZ" | "outRotate" | "outRotateX" | "outRotateY" | "outRotateZ")).map_or(d, |c| c.dst) } else { d })
            .or_else(|| doc.input(i, &["cpim"]).map(|c| c.src))
            .or(n.parent));
        let Some(driven_id) = driven.and_then(|d| self.id(d)) else {
            self.warnings.push(format!("{}: {} drives nothing in the scene; skipped", n.name, n.ty));
            return;
        };
        self.used[i] = true;

        let mut ids: Vec<(i64, NodeId)> = Vec::new();
        let mut weights = Vec::new();
        for (&e, &src) in &targets {
            let Some(t) = self.id(src) else {
                self.warnings.push(format!("{}: target {} is not in the scene", n.name, doc.nodes[src].name));
                continue;
            };
            ids.push((e as i64, t));
            // The weight: a value on the element, or the user attr (w0) wired to it.
            let wired = doc.conns_in(i).find(|c| c.dst == i && c.src == i && first_index(&c.dst_attr) == Some(e) && plain_attr(&c.dst_attr).ends_with("tw"));
            let w = wired
                .and_then(|c| n.num(&[c.src_attr.as_str()]).or_else(|| n.adds.iter().find(|a| a.short == c.src_attr || a.long == c.src_attr).and_then(|a| n.num(&[a.long.as_str(), a.short.as_str()]))))
                .or_else(|| {
                    n.sets.iter().rev().find_map(|s| {
                        let [a, b] = s.path.as_slice() else { return None };
                        (a.range.is_some_and(|r| r.0 == e) && matches!(b.name.as_str(), "tw" | "targetWeight")).then(|| s.values.first()?.num()).flatten()
                    })
                })
                .unwrap_or(1.0);
            weights.push(w);
        }
        if ids.is_empty() {
            self.warnings.push(format!("{}: {} has no target in the scene; skipped", n.name, n.ty));
            return;
        }
        let cmd = n.ty.clone();
        let g = &self.s.graph;
        let mut args: Vec<Value> = ids.iter().map(|x| json!(g.path(x.1))).collect();
        args.push(json!(g.path(driven_id)));
        let mut call = json!({"args": args});
        if Graph::valid_name(&n.name) {
            call["name"] = json!(n.name);
        }
        // Only the channels the file wires: a rig can hold a translate's Y out of a point constraint.
        let wired = |whole: &str, parts: [&str; 3]| -> Option<Vec<&'static str>> {
            let mut on = [false; 3];
            let mut any = false;
            for c in doc.conns_out(i).filter(|c| c.src == i) {
                let a = plain_attr(&c.src_attr);
                if a == whole {
                    return Some(Vec::new());
                }
                if let Some(k) = parts.iter().position(|p| *p == a) {
                    on[k] = true;
                    any = true;
                }
            }
            any.then(|| (0..3).filter(|&k| !on[k]).map(|k| ["x", "y", "z"][k]).collect())
        };
        let groups = [("skipTranslate", wired("ct", ["ctx", "cty", "ctz"])), ("skipRotate", wired("cr", ["crx", "cry", "crz"])), ("skipScale", wired("cs", ["csx", "csy", "csz"]))];
        if groups.iter().any(|g| g.1.is_some()) && cmd != "poleVectorConstraint" {
            for (flag, skip) in groups {
                call[flag] = json!(skip.unwrap_or_else(|| vec!["x", "y", "z"]));
            }
        }
        let made = match self.s.run(&cmd, call) {
            Ok(v) => v,
            Err(e) => {
                self.warnings.push(format!("{}: {e}", n.name));
                return;
            }
        };
        let Some(cname) = made.get(0).and_then(Value::as_str).map(str::to_string) else { return };
        let Ok(Some(c)) = self.s.graph.find(&cname) else { return };
        self.ids[i] = Some(c);
        // The handle's pole vector comes from somewhere else in the file: the constraint's output goes where the file
        // sends it, by the connections read later.
        if let Some(h) = pole_handle {
            let into_handle = doc.conns_out(i).any(|x| x.src == i && x.dst == h);
            if !into_handle {
                let pv = PlugRef::new(driven_id, crate::ik::IkHandle::POLE_VECTOR as i32);
                let g = &mut self.s.graph;
                if g.source(&pv).is_some_and(|s| s.node == c) {
                    g.disconnect(&pv);
                }
                for k in 0..3 {
                    let pk = PlugRef { component: k, ..pv };
                    if g.source(&pk).is_some_and(|s| s.node == c) {
                        g.disconnect(&pk);
                    }
                }
            }
        }
        let t = self.s.graph.node(c).unwrap().type_arc().clone();
        let g = &mut self.s.graph;
        let set_vec = |g: &mut Graph, attr: &str, element: Option<i32>, v: DVec3| {
            if let Some(a) = t.attr_index(attr) {
                let p = match element {
                    Some(e) => PlugRef::element(c, a as i32, e),
                    None => PlugRef::new(c, a as i32),
                };
                let _ = g.set_value(p, PlugValue::Vec3(v));
            }
        };
        for (k, &(e, _)) in ids.iter().enumerate() {
            // The whole offset (".tg[0].tot" x y z) or one axis of it (".tg[0].toty" y), in the file's order.
            let element_vec = |leaf: &[&str]| -> Option<DVec3> {
                let mut out: Option<DVec3> = None;
                for s in &n.sets {
                    let [a, b] = s.path.as_slice() else { continue };
                    if !(matches!(a.name.as_str(), "tg" | "target") && a.range.is_some_and(|r| r.0 as i64 == e)) {
                        continue;
                    }
                    let v = &s.values;
                    if leaf.contains(&b.name.as_str()) {
                        if let (Some(x), Some(y), Some(z)) = (v.first().and_then(Tok::num), v.get(1).and_then(Tok::num), v.get(2).and_then(Tok::num)) {
                            out = Some(DVec3::new(x, y, z));
                        }
                        continue;
                    }
                    let axis = leaf.iter().find_map(|l| b.name.strip_prefix(l).and_then(|rest| ["x", "y", "z"].iter().chain(["X", "Y", "Z"].iter()).position(|c| *c == rest)));
                    if let (Some(axis), Some(x)) = (axis, v.first().and_then(Tok::num)) {
                        let o = out.get_or_insert(DVec3::ZERO);
                        o[axis % 3] = x;
                    }
                }
                out
            };
            if let Some(v) = element_vec(&["tot", "targetOffsetTranslate"]) {
                set_vec(g, "target.targetOffsetTranslate", Some(k as i32), v * doc.lin);
            }
            if let Some(v) = element_vec(&["tor", "targetOffsetRotate"]) {
                set_vec(g, "target.targetOffsetRotate", Some(k as i32), v * doc.ang);
            }
        }
        // Rotation-like offsets are stored in the file's angle unit; a point constraint's in its linear unit; scale has none.
        if let Some(o) = n.vec3(&["o", "offset"]) {
            let unit = match cmd.as_str() {
                "pointConstraint" => doc.lin,
                "orientConstraint" | "aimConstraint" => doc.ang,
                _ => 1.0,
            };
            set_vec(g, "offset", None, o * unit);
        }
        for (names, attr) in [(["a", "aimVector"], "aimVector"), (["u", "upVector"], "upVector"), (["wu", "worldUpVector"], "worldUpVector")] {
            if let Some(v) = n.vec3(&names) {
                set_vec(g, attr, None, v);
            }
        }
        // Where it rests with its weights at zero. A pole vector's rest stays what the handle gave.
        if cmd != "poleVectorConstraint" {
            if let Some(v) = n.vec3(&["rst", "restTranslate"]) {
                let _ = crate::constraint::set_rest(g, c, Some(v * doc.lin), None);
            }
            if let Some(v) = n.vec3(&["rsrr", "restRotate"]) {
                let _ = crate::constraint::set_rest(g, c, None, Some(v * doc.ang));
            }
        }
        if let (Some(on), Some(at)) = (n.truth(&["erp", "enableRestPosition"]), t.attr_index("enableRestPosition")) {
            let _ = g.set_value(PlugRef::new(c, at as i32), PlugValue::Bool(on));
        }
        if let (Some(w), Some(at)) = (n.num(&["wut", "worldUpType"]), t.attr_index("worldUpType")) {
            let _ = g.set_value(PlugRef::new(c, at as i32), PlugValue::Int(w as i64));
        }
        if let (Some(w), Some(at)) = (n.num(&["int", "interpType"]), t.attr_index("interpType")) {
            let _ = g.set_value(PlugRef::new(c, at as i32), PlugValue::Int(w as i64));
        }
        let aliases = self.s.run(&cmd, json!({"args": [cname.clone()], "query": true, "queried": ["weightAliasList"]})).unwrap_or(Value::Null);
        for (alias, w) in aliases.as_array().into_iter().flatten().filter_map(Value::as_str).zip(&weights) {
            if (*w - 1.0).abs() > 1e-12 {
                let _ = self.s.run("setAttr", json!({"args": [format!("{cname}.{alias}"), *w]}));
            }
        }
    }

    fn create_pose(&mut self, i: usize) {
        let doc = self.doc;
        let n = &doc.nodes[i];
        self.used[i] = true;
        let mut members: BTreeMap<i64, usize> = BTreeMap::new();
        for c in doc.conns_in(i).filter(|c| c.dst == i && matches!(plain_attr(&c.dst_attr).as_str(), "m" | "members")) {
            if let Some(e) = first_index(&c.dst_attr) {
                if doc.nodes[c.src].ty == "joint" {
                    members.insert(e, c.src);
                }
            }
        }
        let mut data = Vec::new();
        let mut ids = Vec::new();
        for (&e, &src) in &members {
            let Some(j) = self.id(src) else { continue };
            let xm = n.sets.iter().rev().find(|s| matches!(s.path.as_slice(), [a] if matches!(a.name.as_str(), "xm" | "xformMatrix") && a.range.is_some_and(|r| r.0 == e)));
            let local = xm.and_then(|s| xform_values(&s.values));
            let v = |g: &Graph, a: usize| g.node(j).map_or(DVec3::ZERO, |x| x.data().vec3(a));
            let g = &self.s.graph;
            let (t, r, s) = local.unwrap_or((v(g, Transform::TRANSLATE), v(g, Transform::ROTATE), v(g, Transform::SCALE)));
            data.push(json!({"t": [t.x, t.y, t.z], "r": [r.x, r.y, r.z], "s": [s.x, s.y, s.z]}));
            ids.push(j);
        }
        if ids.is_empty() {
            return;
        }
        let t = types::find("dagPose").unwrap();
        let name = if Graph::valid_name(&n.name) { n.name.clone() } else { String::new() };
        let g = &mut self.s.graph;
        let Ok(p) = g.create_node(&t, &name, NO_NODE, false) else { return };
        self.ids[i] = Some(p);
        let at = |a: &str| t.attr_index(a).unwrap();
        let _ = rig::set_json(g, p, at("poseData"), json!(data));
        let _ = g.set_value(PlugRef::new(p, at("bindPose") as i32), PlugValue::Bool(n.truth(&["bp", "bindPose"]).unwrap_or(false)));
        for (e, j) in ids.into_iter().enumerate() {
            let _ = g.connect(&PlugRef::new(j, Transform::WORLD_MATRIX as i32), &PlugRef::element(p, at("worldMatrix") as i32, e as i32), false);
        }
    }

    fn create_layer(&mut self, i: usize) {
        use crate::display_layers as dl;
        let n = &self.doc.nodes[i];
        self.used[i] = true;
        if n.name == dl::DEFAULT_LAYER {
            return;
        }
        let t = types::find(dl::TYPE_NAME).unwrap();
        let name = if Graph::valid_name(&n.name) { n.name.clone() } else { String::new() };
        let g = &mut self.s.graph;
        let Ok(id) = g.create_node(&t, &name, NO_NODE, false) else { return };
        self.ids[i] = Some(id);
        if let Some(v) = n.truth(&["v", "visibility"]) {
            let _ = g.set_value(PlugRef::new(id, dl::VISIBILITY as i32), PlugValue::Bool(v));
        }
        if let Some(v) = n.num(&["dt", "displayType"]) {
            let _ = g.set_value(PlugRef::new(id, dl::DISPLAY_TYPE as i32), PlugValue::Int(v as i64));
        }
        if let Some(v) = n.num(&["do", "displayOrder"]) {
            let _ = g.set_value(PlugRef::new(id, dl::DISPLAY_ORDER as i32), PlugValue::Int(v as i64));
        }
        for (names, attr) in [(["lod", "levelOfDetail"], dl::LEVEL_OF_DETAIL), (["c", "color"], dl::COLOR), (["id", "identification"], dl::IDENTIFICATION)] {
            if let Some(v) = n.num(&names) {
                let _ = g.set_value(PlugRef::new(id, attr as i32), PlugValue::Int(v as i64));
            }
        }
        for (names, attr) in [
            (["s", "shading"], dl::SHADING),
            (["t", "texturing"], dl::TEXTURING),
            (["p", "playback"], dl::PLAYBACK),
            (["e", "enabled"], dl::ENABLED),
            (["hpb", "hideOnPlayback"], dl::HIDE_ON_PLAYBACK),
            (["ovrgbf", "overrideRGBColors"], dl::OVERRIDE_RGB_COLORS),
        ] {
            if let Some(v) = n.truth(&names) {
                let _ = g.set_value(PlugRef::new(id, attr as i32), PlugValue::Bool(v));
            }
        }
        // Binary files give the colour a channel at a time.
        let channels = [["ovcr", "overrideColorR"], ["ovcg", "overrideColorG"], ["ovcb", "overrideColorB"]];
        let parts = || -> Option<DVec3> { Some(DVec3::new(n.num(&channels[0])?, n.num(&channels[1])?, n.num(&channels[2])?)) };
        if let Some(v) = n.vec3(&["ovrgb", "overrideColorRGB"]).or_else(parts) {
            let _ = g.set_value(PlugRef::new(id, dl::OVERRIDE_COLOR_RGB as i32), PlugValue::Vec3(v));
        }
        if let Some(v) = n.num(&["ovca", "overrideColorA"]) {
            let _ = g.set_value(PlugRef::new(id, dl::OVERRIDE_COLOR_A as i32), PlugValue::Float(v));
        }
    }

    fn create_curve(&mut self, i: usize, fps: f64) {
        let doc = self.doc;
        let n = &doc.nodes[i];
        let Some(t) = types::find(&n.ty) else { return };
        let name = if Graph::valid_name(&n.name) { n.name.clone() } else { String::new() };
        let g = &mut self.s.graph;
        let Ok(id) = g.create_node(&t, &name, NO_NODE, false) else { return };
        self.ids[i] = Some(id);
        self.used[i] = true;
        let at = |a: &str| t.attr_index(a).unwrap();
        let keys = curve_keys(n, doc, fps);
        let _ = g.set_value(PlugRef::new(id, at("keys") as i32), PlugValue::Data(Some(Arc::new(keys))));
        if n.truth(&["wgt", "weightedTangents"]) == Some(true) {
            let _ = g.set_value(PlugRef::new(id, at("weightedTangents") as i32), PlugValue::Bool(true));
        }
        if let Some(v) = n.num(&["pre", "preInfinity"]) {
            let _ = g.set_value(PlugRef::new(id, at("preInfinity") as i32), PlugValue::Int(v as i64));
        }
        if let Some(v) = n.num(&["pst", "postInfinity"]) {
            let _ = g.set_value(PlugRef::new(id, at("postInfinity") as i32), PlugValue::Int(v as i64));
        }
    }

    /// A connection's source, seen through unitConversion nodes.
    fn real_source(&self, c: &Conn) -> (usize, String) {
        let (mut src, mut attr) = (c.src, c.src_attr.clone());
        for _ in 0..8 {
            if self.doc.nodes[src].ty != "unitConversion" || scales(&self.doc.nodes[src]) {
                break;
            }
            let Some(up) = self.doc.input(src, &["i", "input"]) else { break };
            src = up.src;
            attr = up.src_attr.clone();
        }
        (src, attr)
    }

    fn channel_connections(&mut self) {
        // A joint's inverseScale, which undoes the parent joint's scale.
        for c in &self.doc.conns {
            if matches!(c.dst_attr.as_str(), "is" | "inverseScale") {
                if let (Some(src), Some(dst)) = (self.ids[c.src], self.ids[c.dst]) {
                    let g = &mut self.s.graph;
                    if rig::is_joint(g, dst) {
                        if let Ok(from) = g.plug(&format!("{}.{}", g.path(src), c.src_attr)) {
                            let _ = g.connect(&from, &PlugRef::new(dst, rig::Joint::INVERSE_SCALE as i32), true);
                        }
                    }
                }
            }
        }
        let doc = self.doc;
        for c in &doc.conns {
            let Some(dst) = self.id(c.dst) else { continue };
            let dt = self.s.graph.node(dst).unwrap().type_arc().clone();
            // The handle's command wired the effector to the end joint.
            if dt.name == "ikEffector" {
                continue;
            }
            let (sn, sa) = self.real_source(c);
            let Some(src) = self.id(sn) else { continue };
            let st = self.s.graph.node(src).unwrap().type_arc().clone();
            // A plug-in's deformer on a curve: read by `curve_deformers`.
            if crate::scripted::is_plugin_deformer(&st.name) && dt.name == crate::curves::SHAPE_TYPE && matches!(plain_attr(&sa).as_str(), "og" | "outputGeometry") {
                continue;
            }
            if crate::scripted::is_plugin_deformer(&dt.name) && matches!(plain_attr(&c.dst_attr).as_str(), "ip.ig" | "input.inputGeometry" | "ip.gi" | "input.groupId") {
                continue;
            }
            // These go through `deformer_connections`.
            // A plug-in's deformer goes that way only on a mesh; on a curve its connections are plain ones.
            let chained = |ty: &str, id: NodeId| crate::deform::is_deformer(ty) || (crate::scripted::is_plugin_deformer(ty) && self.deformers.iter().any(|d| d.id == id));
            if chained(&dt.name, dst) || chained(&st.name, src) {
                continue;
            }
            // What HIKState2SK reads from each joint in the reference: here the definition's rig information holds it.
            if dt.name == "HIKState2SK" && ["PGX", "PreR", "PostR", "SC", "IS", "ROrder"].iter().any(|x| c.dst_attr.ends_with(x)) {
                continue;
            }
            if dt.name == crate::hik::TYPE && matches!(sa.as_str(), "ch" | "Character") {
                let _ = crate::hik::ch_plug(&mut self.s.graph, src);
            }
            if dt.name == crate::hik::TYPE && matches!(plain_attr(&c.dst_attr).as_str(), "meshes" | "guides" | "skeleton" | "quickRigInfo") {
                continue;
            }
            // Keying groups inside keying groups: the set here holds objects.
            if dt.name == "keyingGroup" && matches!(plain_attr(&c.dst_attr).as_str(), "dnsm" | "dnSetMembers") {
                continue;
            }
            // A layer's members are the plugs its blends drive: an edge from them would be a cycle here.
            if dt.name == "animLayer" && matches!(plain_attr(&c.dst_attr).as_str(), "dsm" | "dagSetMembers" | "dnsm" | "dnSetMembers") {
                continue;
            }
            // A CV driven by a locator or anything else, as an eyelid rig does.
            let into_cv = dt.name == crate::curves::SHAPE_TYPE && matches!(plain_attr(&c.dst_attr).as_str(), "cp" | "controlPoints");
            let from_layer = st.name == crate::display_layers::TYPE_NAME;
            if !from_layer && (into_cv || is_plain_node(&dt.name) || is_plain_node(&st.name) || dt.name == "unitConversion" || st.name == "unitConversion") {
                self.plain_connection(c, sn, &sa, src, dst);
                continue;
            }
            let plain_dst = plain_attr(&c.dst_attr);
            let is_curve = st.attr_index("keys").is_some() && st.attr_index("output").is_some();
            // Into a setting of a history node or creator, by the reference's names; the mesh and the world matrix
            // went in with the chain.
            if history_io::history_type(&dt.name).is_some() || history_io::creator_type(&dt.name) {
                if matches!(plain_dst.as_str(), "ip" | "inputPolymesh" | "mp" | "manipMatrix" | "ix" | "inputMatrix") {
                    continue;
                }
                let to = history_io::setting_index(&dt, &plain_dst).map(|a| match first_index(&c.dst_attr) {
                    Some(e) if dt.attrs[a].is_array() => PlugRef::element(dst, a as i32, e as i32),
                    _ => PlugRef::new(dst, a as i32),
                });
                let from = if is_curve { st.attr_index("output").map(|o| PlugRef::new(src, o as i32)) } else { self.s.graph.plug(&format!("{}.{sa}", self.s.graph.path(src))).ok() };
                match (from, to) {
                    (Some(a), Some(b)) => {
                        if let Err(e) = self.s.graph.connect(&a, &b, false) {
                            self.warnings.push(format!("{}.{} -> {}.{}: {e}", doc.nodes[sn].name, sa, doc.nodes[c.dst].name, c.dst_attr));
                        }
                    }
                    _ => self.note_drop(&dt.name, &plain_dst, "connection to"),
                }
                continue;
            }
            // Into a driven curve, from a channel.
            let dst_plug = if DRIVEN_CURVES.contains(&dt.name.as_str()) {
                if !matches!(plain_dst.as_str(), "i" | "input") {
                    continue;
                }
                PlugRef::new(dst, dt.attr_index("input").unwrap() as i32)
            } else if st.name == crate::display_layers::TYPE_NAME {
                if !matches!(plain_dst.as_str(), "do" | "drawOverride") {
                    continue;
                }
                PlugRef::new(dst, DagNode::DISPLAY_LAYER as i32)
            } else {
                match channel(&dt, &plain_dst) {
                    // An added multi takes the element the file names.
                    Some((a, comp)) if comp < 0 && dt.attrs[a].is_array() && first_index(&c.dst_attr).is_some() => PlugRef::element(dst, a as i32, first_index(&c.dst_attr).unwrap() as i32),
                    Some((a, comp)) => plug_ref(dst, a, comp),
                    None => {
                        self.note_drop(&dt.name, &plain_dst, "connection to");
                        continue;
                    }
                }
            };
            let src_plug = if is_curve {
                if !matches!(sa.as_str(), "o" | "output") {
                    continue;
                }
                PlugRef::new(src, st.attr_index("output").unwrap() as i32)
            } else if st.name == crate::display_layers::TYPE_NAME {
                if !matches!(sa.as_str(), "di" | "drawInfo") {
                    continue;
                }
                PlugRef::new(src, crate::display_layers::DRAW_INFO as i32)
            } else {
                match source_channel(&st, &plain_attr(&sa)) {
                    Some((a, comp)) => plug_ref(src, a, comp),
                    None => {
                        self.note_drop(&st.name, &plain_attr(&sa), "connection from");
                        continue;
                    }
                }
            };
            // A nonlinear deformer's settings into its handle's shape, which here are one node.
            if src_plug == dst_plug {
                continue;
            }
            if let Err(e) = self.s.graph.connect(&src_plug, &dst_plug, false) {
                self.warnings.push(format!("{}.{} -> {}.{}: {e}", doc.nodes[sn].name, sa, doc.nodes[c.dst].name, c.dst_attr));
            }
        }
    }

    /// Connections with a constraint at one end, which `channel_connections` could not make because the constraints come later:
    /// what drives a target's weight, usually a set driven key for a space switch, and a constraint feeding a pairBlend.
    fn constraint_connections(&mut self) {
        let doc = self.doc;
        let is_constraint = |i: usize| rig::CONSTRAINT_TYPES.contains(&doc.nodes[i].ty.as_str());
        let mut blends: Vec<usize> = Vec::new();
        for c in &doc.conns {
            if is_constraint(c.dst) && matches!(plain_attr(&c.dst_attr).as_str(), "wum" | "worldUpMatrix") {
                let (Some(src), Some(dst)) = (self.id(c.src), self.id(c.dst)) else { continue };
                self.plain_connection(c, c.src, &c.src_attr, src, dst);
            } else if is_constraint(c.dst) && c.src != c.dst {
                let Some(e) = plain_attr(&c.dst_attr).strip_prefix('w').and_then(|x| x.parse::<usize>().ok()) else {
                    // What else a rig drives on one: whether it runs, and its offset.
                    if matches!(plain_attr(&c.dst_attr).as_str(), "nds" | "nodeState" | "o" | "offset" | "ox" | "oy" | "oz" | "offsetX" | "offsetY" | "offsetZ") {
                        let (sn, sa) = self.real_source(c);
                        if let (Some(src), Some(dst)) = (self.id(sn), self.id(c.dst)) {
                            self.plain_connection(c, sn, &sa, src, dst);
                        }
                    }
                    continue;
                };
                let (sn, sa) = self.real_source(c);
                let (Some(src), Some(dst)) = (self.id(sn), self.id(c.dst)) else { continue };
                let t = self.s.graph.node(dst).unwrap().type_arc().clone();
                let suffix = format!("W{e}");
                let Some(a) = t.attrs.iter().position(|a| a.has(flags::USER) && a.name.ends_with(&suffix)) else { continue };
                let st = self.s.graph.node(src).unwrap().type_arc().clone();
                // A curve's output, a channel, or by name on a utility node: a condition or plusMinusAverage switching spaces.
                let out = if st.attr_index("keys").is_some() { st.attr_index("output").map(|x| plug_ref(src, x, -1)) } else { channel(&st, &plain_attr(&sa)).map(|(oa, oc)| plug_ref(src, oa, oc)) };
                let out = out.or_else(|| self.s.graph.plug(&format!("{}.{sa}", self.s.graph.path(src))).ok());
                let Some(out) = out else {
                    self.note_drop(&st.name, &plain_attr(&sa), "weight from");
                    continue;
                };
                if let Err(e) = self.s.graph.connect(&out, &PlugRef::new(dst, a as i32), true) {
                    self.warnings.push(format!("{}.{sa} -> {}.{}: {e}", doc.nodes[sn].name, doc.nodes[c.dst].name, c.dst_attr));
                }
            } else if is_constraint(c.src) && doc.nodes[c.dst].ty == "pairBlend" {
                let (Some(src), Some(dst)) = (self.id(c.src), self.id(c.dst)) else { continue };
                self.plain_connection(c, c.src, &c.src_attr, src, dst);
                if !blends.contains(&c.dst) {
                    blends.push(c.dst);
                }
            }
        }
        // The constraint took the channels when it was made; they belong to the pairBlend.
        for &b in &blends {
            for c in doc.conns_out(b).filter(|c| c.src == b) {
                let (Some(src), Some(dst)) = (self.id(c.src), self.id(c.dst)) else { continue };
                self.plain_connection(c, c.src, &c.src_attr, src, dst);
            }
        }
    }

    /// Curves with something left to drive; the rest are dropped with what they animated.
    fn curve_is_needed(&self, i: usize) -> bool {
        // Keys on a scene node the file doesn't hold (an animation export), connected by name as it is imported.
        if OUTSIDE_SOURCES.with(|o| o.borrow().contains(&self.doc.nodes[i].name)) {
            return true;
        }
        // Keys on a node of a reference, through its placeholder or by name.
        let start = format!("{}.", self.doc.nodes[i].name);
        if self.doc.ref_conns.iter().any(|c| c.src.starts_with(&start)) {
            return true;
        }
        self.doc.conns_out(i).any(|c| {
            c.src == i && {
                let d = &self.doc.nodes[c.dst];
                let plain = plain_attr(&c.dst_attr);
                match self.id(c.dst) {
                    Some(_) if is_plain_node(&d.ty) || deformers::is_file_deformer(&d.ty) => true,
                    Some(id) if history_io::history_type(&d.ty).is_some() || history_io::creator_type(&d.ty) => history_io::setting_index(self.s.graph.node(id).unwrap().ty(), &plain).is_some(),
                    Some(id) => channel(self.s.graph.node(id).unwrap().ty(), &plain).is_some(),
                    None => d.ty == "unitConversion" || d.ty == "ikHandle" || d.ty.starts_with("deform") || rig::CONSTRAINT_TYPES.contains(&d.ty.as_str()),
                }
            }
        })
    }

    /// Shapes under a second transform: a mesh of its own there, an instance of the first.
    fn instances(&mut self) {
        for (shape, parent) in &self.doc.instances {
            let g = &mut self.s.graph;
            let found = (g.find(shape).ok().flatten(), g.find(parent).ok().flatten());
            let (Some(src), Some(tr)) = found else {
                self.warnings.push(format!("instance of {shape} under {parent}: no such object"));
                continue;
            };
            if !crate::components::is_mesh(g, src) {
                self.warnings.push(format!("instance of {shape}: only meshes are instanced here"));
                continue;
            }
            let name = crate::dag::shape_name(&g.node(tr).unwrap().name().to_string());
            let t = types::find("mesh").unwrap();
            let made = g.create_node(&t, &name, tr, false).map_err(|e| e.to_string()).and_then(|m| crate::instancing::connect_instance(g, src, m).map_err(|e| e.to_string()));
            if let Err(e) = made {
                self.warnings.push(format!("instance of {shape}: {e}"));
            }
        }
    }

    /// The lines after `file -r` that change a referenced node.
    fn reference_edits(&mut self) {
        for n in self.doc.nodes.iter().filter(|n| n.default && n.name.contains(':')) {
            let Ok(Some(id)) = self.s.graph.find(&n.name) else { continue };
            let path = self.s.graph.path(id);
            for set in &n.sets {
                if set.values.is_empty() {
                    continue;
                }
                let text: Vec<String> = set.path.iter().map(|seg| seg.name.clone()).collect();
                let Ok(p) = self.s.graph.plug(&format!("{path}.{}", text.join("."))) else { continue };
                let def = self.s.graph.attr(&p).clone();
                if let Some(v) = plug_value(&def, p.component, &set.values, 1.0) {
                    let _ = self.s.graph.set_value(p, v);
                }
            }
        }
        for c in &self.doc.ref_conns {
            crate::references::file_connection(self.s, &c.src, &c.dst, c.src_ref, c.dst_ref, c.owner.as_deref());
        }
    }

    fn report_unused(&mut self) {
        let mut by_type: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for (i, n) in self.doc.nodes.iter().enumerate() {
            if self.used[i] || n.default || QUIET_TYPES.contains(&n.ty.as_str()) || is_render_type(&n.ty) {
                continue;
            }
            // Intermediate meshes and their creators nobody used were superseded by stored geometry.
            if n.ty == "mesh" || creator_attrs(&n.ty).is_some() {
                continue;
            }
            by_type.entry(n.ty.as_str()).or_default().push(n.name.as_str());
        }
        for (ty, names) in by_type {
            let shown: Vec<&str> = names.iter().take(4).copied().collect();
            let more = if names.len() > shown.len() { format!(" and {} more", names.len() - shown.len()) } else { String::new() };
            self.warnings.push(format!("{ty} not represented: {}{more}", shown.join(", ")));
        }
    }
}

/// .wl in its forms: `.wl[3].w[2]  v`, `.wl[3].w[0:1]  a b`, and `.wl[0:7].w  n i v i v ...` for each point in turn.
fn read_weights(n: &MaNode) -> Vec<Vec<(u32, f64)>> {
    let mut out: Vec<Vec<(u32, f64)>> = Vec::new();
    for s in &n.sets {
        let [a, b] = s.path.as_slice() else { continue };
        if !matches!(a.name.as_str(), "wl" | "weightList") || !matches!(b.name.as_str(), "w" | "weights") || s.values.is_empty() {
            continue;
        }
        let Some((p0, p1)) = a.range else { continue };
        if out.len() <= p1 as usize {
            out.resize(p1 as usize + 1, Vec::new());
        }
        match b.range {
            Some((w0, _)) => {
                for (k, t) in s.values.iter().enumerate() {
                    if let Some(v) = t.num() {
                        out[p0 as usize].push(((w0 + k as i64) as u32, v));
                    }
                }
            }
            None => {
                let mut k = 0;
                for p in p0..=p1 {
                    let count = s.values.get(k).and_then(Tok::num).unwrap_or(0.0) as usize;
                    k += 1;
                    for _ in 0..count {
                        let (Some(i), Some(v)) = (s.values.get(k).and_then(Tok::num), s.values.get(k + 1).and_then(Tok::num)) else { break };
                        out[p as usize].push((i as u32, v));
                        k += 2;
                    }
                }
            }
        }
    }
    out
}

/// A matrix attribute's plain value.
trait MatrixOf {
    fn matrix_of(&self, name: &[&str]) -> Option<DMat4>;
}

impl MatrixOf for MaNode {
    fn matrix_of(&self, name: &[&str]) -> Option<DMat4> {
        matrix_from(&self.set_of(name)?.values)
    }
}

/// "xform" s[3] r[3] order t[3] ...: scale, rotate in degrees and translate.
fn xform_values(v: &[Tok]) -> Option<(DVec3, DVec3, DVec3)> {
    if v.first()?.text()? != "xform" {
        return None;
    }
    let n = |i: usize| v.get(i).and_then(Tok::num);
    let s = DVec3::new(n(1)?, n(2)?, n(3)?);
    let r = DVec3::new(n(4)?, n(5)?, n(6)?);
    let t = DVec3::new(n(8)?, n(9)?, n(10)?);
    Some((t, r, s))
}

/// An added attribute as the graph holds it. None for a type that isn't kept.
fn user_attr_def(a: &AddAttr) -> Option<AttrDef> {
    let at = a.at.as_str();
    let ty = match (at, a.dt.as_str()) {
        ("double" | "float" | "doubleLinear" | "doubleAngle" | "time", _) => PlugType::Float,
        ("long" | "short" | "byte" | "char", _) => PlugType::Int,
        ("bool", _) => PlugType::Bool,
        ("enum", _) => PlugType::Enum,
        ("double3" | "float3", _) => PlugType::Vec3,
        (_, "string") => PlugType::String,
        ("message", _) => PlugType::Int,
        _ => return None,
    };
    let default_value = match ty {
        PlugType::Float => PlugValue::Float(a.dv.unwrap_or(0.0)),
        PlugType::Int | PlugType::Enum => PlugValue::Int(a.dv.unwrap_or(0.0) as i64),
        PlugType::Bool => PlugValue::Bool(a.dv.unwrap_or(0.0) != 0.0),
        PlugType::Vec3 => PlugValue::Vec3(DVec3::ZERO),
        _ => PlugValue::String(String::new()),
    };
    let mut flags_on = flags::INPUT | flags::STORABLE | flags::USER;
    if a.keyable {
        flags_on |= flags::KEYABLE;
    }
    if at == "message" {
        flags_on = flags::INPUT | flags::USER | flags::MESSAGE;
    }
    if a.multi {
        flags_on |= flags::ARRAY;
    }
    if a.world_space {
        flags_on |= flags::WORLD_SPACE;
    }
    if a.not_unique {
        flags_on |= flags::NAME_NOT_UNIQUE;
    }
    let mut def = AttrDef {
        name: a.long.clone(),
        short_name: if a.short == a.long { String::new() } else { a.short.clone() },
        ty,
        default_value,
        flags: flags_on,
        min_value: a.min,
        max_value: a.max,
        enum_names: a.en.split(':').filter(|s| !s.is_empty()).map(|s| s.split('=').next().unwrap_or(s).to_string()).collect(),
        ..Default::default()
    };
    if ty == PlugType::Vec3 {
        def = AttrDef { ..crate::node::vec3_attr(&def.name, &def.short_name, DVec3::ZERO, flags_on, "") };
    }
    def.nice_name = a.nice.clone();
    def.category = a.category.join(";");
    Some(def)
}

/// What became of a reference edit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum EditResult {
    Done,
    /// The values are of a type that isn't read: kept as written.
    NotRead,
    Failed,
}

fn lex_one(text: &str) -> Vec<Tok> {
    let mut lex = Lexer::new(text);
    let mut st = Vec::new();
    lex.statement(&mut st);
    st
}

/// The setAttr a reference edit's text holds, against a stand-in node.
fn edit_set(args: &str) -> Option<SetAttr> {
    let mut d = Doc::parse("");
    let i = d.add_node(MaNode::default());
    d.set_attr(&lex_one(&format!("setAttr \".x\"{args};")), Some(i));
    d.nodes[i].sets.pop()
}

/// Whether a setAttr edit holds only flags, as `" -av"` and `" -k 1"` do.
pub(crate) fn edit_is_flags_only(args: &str) -> bool {
    edit_set(args).is_some_and(|s| s.values.is_empty())
}

pub(crate) fn set_attr_edit(g: &mut Graph, p: PlugRef, args: &str) -> EditResult {
    let Some(set) = edit_set(args) else { return EditResult::Failed };
    if !set.values.is_empty() {
        let def = g.attr(&p).clone();
        let Some(v) = plug_value(&def, p.component, &set.values, 1.0) else { return EditResult::NotRead };
        if g.set_value(p, v).is_err() {
            return EditResult::Failed;
        }
    }
    if let Some(lock) = set.lock {
        let _ = g.set_locked(&p, lock);
    }
    EditResult::Done
}

pub(crate) fn add_attr_edit(g: &mut Graph, n: NodeId, long: &str, short: &str, args: &str) -> bool {
    let mut d = Doc::parse("");
    let i = d.add_node(MaNode::default());
    d.add_attr(&lex_one(&format!("addAttr{args} -ln \"{long}\" -sn \"{short}\";")), Some(i));
    let Some(a) = d.nodes[i].adds.pop() else { return false };
    if !a.parent.is_empty() {
        return true;
    }
    let Some(def) = user_attr_def(&a) else { return false };
    if g.node(n).is_some_and(|x| x.ty().attr_index(&def.name).is_some()) {
        return true;
    }
    g.add_attr(n, def).is_ok()
}

fn plug_value(def: &AttrDef, comp: i32, values: &[Tok], _factor: f64) -> Option<PlugValue> {
    // The file's numbers are in the units its header names; what's stored is centimetres and radians.
    let factor = match def.unit {
        units::Unit::Linear => FILE_UNITS.with(Cell::get).0,
        units::Unit::Angle => FILE_UNITS.with(Cell::get).1,
        _ => 1.0,
    };
    let f0 = values.first()?;
    Some(match def.ty {
        _ if comp >= 0 => PlugValue::Float(f0.num()? * factor),
        PlugType::Float => PlugValue::Float(f0.num()? * factor),
        PlugType::Int | PlugType::Enum => PlugValue::Int(f0.num()? as i64),
        PlugType::Bool => PlugValue::Bool(f0.truth()?),
        PlugType::Vec3 => {
            // A compound of two floats: repeatUV, offset.
            if values.len() == 2 {
                return Some(PlugValue::Vec3(DVec3::new(values[0].num()?, values[1].num()?, 0.0) * factor));
            }
            if values.len() < 3 {
                return None;
            }
            PlugValue::Vec3(tok_vec3(values) * factor)
        }
        PlugType::String => PlugValue::String(f0.text()?.to_string()),
        PlugType::Matrix => PlugValue::Matrix(matrix_from(values)?),
        // A plug-in's data: what its readASCII takes, the words after -type and its name.
        PlugType::Data if def.category.starts_with(crate::scripted::PLUGIN_DATA) => {
            let kind = &def.category[crate::scripted::PLUGIN_DATA.len()..];
            let text: Vec<String> = values.iter().map(|t| match t {
                Tok::Num(x) => fmt_num(*x),
                Tok::Word(w) => w.clone(),
                Tok::Str(w) => quoted(w),
            }).collect();
            // A value its type can't read is said, not dropped quietly: the reference warns on a setAttr that fails.
            let Some(load) = crate::plug::data::find(kind) else {
                log::warn!("setAttr: no data type {kind} is registered; is its plug-in loaded?");
                return None;
            };
            match load(&json!({"kind": kind, "pluginData": true, "text": text.join(" ")})) {
                Ok(d) => PlugValue::Data(Some(d)),
                Err(e) => {
                    log::warn!("setAttr: {kind}: {e}");
                    return None;
                }
            }
        }
        // Array data: the count, then the items.
        PlugType::Data => {
            let kind = crate::array_data::of(&def.default_value)?.kind.clone();
            let toks: Vec<Value> = values.iter().map(|t| match t {
                Tok::Num(x) => json!(x),
                Tok::Word(w) | Tok::Str(w) => json!(w),
            }).collect();
            crate::array_data::from_tokens(&kind, &toks)?.value()
        }
        _ => return None,
    })
}

fn apply_tweaks(doc: &Doc, m: &mut Mesh, tweaks: &[usize]) {
    for &t in tweaks {
        for (at, c) in doc.nodes[t].elements(&["tk", "tweak"], 3) {
            if at < m.point_capacity() as usize && m.point_valid(at as Index) {
                let p = m.position(at as Index) + tok_vec3(c);
                m.set_position(at as Index, p);
            }
        }
    }
}

thread_local! {
    static LAST_READ_NAMES: std::cell::RefCell<Vec<(NodeId, String)>> = const { std::cell::RefCell::new(Vec::new()) };
    /// -exportSelectedStrict: export writes the nodes given and none of what they need.
    pub(crate) static EXPORT_STRICT: Cell<bool> = const { Cell::new(false) };
    /// The nodes of a file being imported that connect to nodes it doesn't hold.
    static OUTSIDE_SOURCES: std::cell::RefCell<HashSet<String>> = std::cell::RefCell::new(HashSet::new());
    /// file -saveReference: the file written holds none of the scene's references.
    pub(crate) static NO_REFERENCES: Cell<bool> = const { Cell::new(false) };
    /// An animation export: curves are written though what they drive isn't.
    static ANIM_EXPORT: Cell<bool> = const { Cell::new(false) };
    /// -importFrameRate on the next import: the file's frame rate becomes the scene's.
    pub(crate) static IMPORT_FRAME_RATE: Cell<bool> = const { Cell::new(false) };
    /// -importTimeRange on the next import: keep, override or combine the playback range with the file's.
    pub(crate) static IMPORT_TIME_RANGE: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
    /// file -defaultExtensions: a name with no extension takes the type's on save and export.
    pub(crate) static DEFAULT_EXTENSIONS: Cell<bool> = const { Cell::new(true) };
}

/// The nodes the last read made, with the names the file gave them.
pub(crate) fn last_read_names() -> Vec<(NodeId, String)> {
    LAST_READ_NAMES.with(|n| n.borrow().clone())
}

/// Builds the document in the session. Everything goes in one pass; the graph evaluates on demand for world matrices.
fn build(s: &mut Session, doc: &Doc, import: bool) -> Vec<String> {
    doc.make_index();
    let prior: Vec<NodeId> = s.graph.selection().to_vec();
    let take_rate = import && IMPORT_FRAME_RATE.with(Cell::get);
    let fps = if import && !take_rate { anim::fps() } else { doc.fps.unwrap_or_else(anim::fps) };
    if !import || take_rate {
        if let Some(f) = doc.fps {
            let _ = s.run("playbackOptions", json!({"framesPerSecond": f}));
            anim::set_drop_frame(doc.drop_frame);
        }
    }
    let count = doc.nodes.len();
    let mut meshes = HashMap::new();
    for (i, n) in doc.nodes.iter().enumerate() {
        if n.ty == "mesh" {
            meshes.insert(i, MaMesh::from_node(n));
        }
    }
    let mut b = Build { s, doc, ids: vec![None; count], used: vec![false; count], meshes, warnings: doc.warnings.clone(), deformers: Vec::new(), post_edits: Vec::new() };

    // Cameras are skipped with the transforms that exist only to hold them.
    for i in 0..count {
        if doc.nodes[i].ty != "camera" {
            continue;
        }
        let n = &doc.nodes[i];
        if !DEFAULT_CAMERAS.contains(&n.name.as_str()) {
            // A camera of the scene's own is made like any other shape.
            continue;
        }
        b.used[i] = true;
        // The scene's own cameras take what the file says; importing leaves them alone.
        if !import {
            b.ids[i] = b.s.graph.find(&n.name).ok().flatten();
        }
        if let Some(p) = n.parent {
            let only_cameras = doc.children(p).all(|k| doc.nodes[k].ty == "camera");
            if only_cameras {
                b.used[p] = true;
                if !import {
                    b.ids[p] = b.s.graph.find(&doc.nodes[p].name).ok().flatten();
                }
            }
        }
    }

    for i in 0..count {
        b.create_dag(i);
    }
    b.create_shading_groups(import);
    for i in 0..count {
        // The reference's own configuration scripts stay out: their content is read above and their MEL is for its interface.
        // The up axis script node is run as the file is read and written again on save.
        if is_plain_node(&doc.nodes[i].ty) && !matches!(doc.nodes[i].name.as_str(), "sceneConfigurationScriptNode" | "uiConfigurationScriptNode" | "upAxisScriptNode") {
            b.create_plain(i);
        }
        if scales(&doc.nodes[i]) {
            b.create_plain(i);
            if let Some(id) = b.ids[i] {
                let path = format!("{}.cf", b.s.graph.path(id));
                if let Ok(p) = b.s.graph.plug(&path) {
                    let _ = b.s.graph.set_value(p, PlugValue::Float(conversion_factor(&doc.nodes[i])));
                }
            }
        }
    }
    for i in 0..count {
        b.add_user_attrs(i);
        if doc.nodes[i].ty != "mesh" {
            b.apply_channels(i);
        } else if b.ids[i].is_some() {
            b.apply_channels(i);
        }
    }
    for i in 0..count {
        // An intermediate mesh with an id is an Orig already filled from the mesh it feeds, tweaks and all.
        if doc.nodes[i].ty == "mesh" && b.ids[i].is_some() && !doc.nodes[i].truth(&["io", "intermediateObject"]).unwrap_or(false) {
            b.build_mesh(i);
            // Smooth mesh preview: displaySmoothMesh 1 or 2 shows smoothLevel levels (2 when not written).
            let dsm = doc.nodes[i].num(&["dsm", "displaySmoothMesh"]).unwrap_or(0.0);
            // An older Aztek file: smoothPreview held the level.
            let old = doc.nodes[i].num(&["smoothPreview"]).unwrap_or(0.0);
            if let Some(id) = b.ids[i] {
                let path = b.s.graph.path(id);
                let g = &mut b.s.graph;
                let mut set = |attr: &str, v: PlugValue| {
                    if let Ok(p) = g.plug(&format!("{path}.{attr}")) {
                        let _ = g.set_value(p, v);
                    }
                };
                if dsm > 0.0 {
                    set("displaySmoothMesh", PlugValue::Int(dsm as i64));
                } else if old > 0.0 {
                    set("displaySmoothMesh", PlugValue::Int(2));
                    set("smoothLevel", PlugValue::Int(old as i64));
                }
                if let Some(level) = doc.nodes[i].num(&["smoothLevel", "lev"]) {
                    set("smoothLevel", PlugValue::Int(level as i64));
                }
                if let Some(level) = doc.nodes[i].num(&["renderSmoothLevel", "rsl"]) {
                    set("renderSmoothLevel", PlugValue::Int(level as i64));
                }
                if doc.nodes[i].num(&["useSmoothPreviewForRender", "uspr"]).is_some() {
                    set("useSmoothPreviewForRender", PlugValue::Bool(doc.nodes[i].truth(&["useSmoothPreviewForRender", "uspr"]).unwrap_or(true)));
                }
                // Polygon Display switches.
                for (attr, short, _) in crate::mesh_nodes::DISPLAY_SWITCHES {
                    if let Some(on) = doc.nodes[i].truth(&[attr, short]) {
                        set(attr, PlugValue::Bool(on));
                    }
                }
            }
        }
        if doc.nodes[i].ty == "nurbsSurface" && b.ids[i].is_some() {
            b.build_surface(i);
        }
        if doc.nodes[i].ty == "nurbsCurve" && b.ids[i].is_some() {
            b.build_curve(i);
        }
        if doc.nodes[i].ty == "camera" && b.ids[i].is_some() {
            b.build_camera(i);
        }
    }
    // Generators can be animated too; their channels were set as they were made.

    for i in 0..count {
        if doc.nodes[i].ty == "ikHandle" {
            b.create_ik(i);
        }
    }
    for i in 0..count {
        if doc.nodes[i].ty == crate::display_layers::TYPE_NAME {
            b.create_layer(i);
        }
    }
    for i in 0..count {
        let ty = doc.nodes[i].ty.as_str();
        if (anim::CURVE_TYPES.contains(&ty) || DRIVEN_CURVES.contains(&ty)) && b.curve_is_needed(i) {
            b.create_curve(i, fps);
        } else if anim::CURVE_TYPES.contains(&ty) || DRIVEN_CURVES.contains(&ty) {
            b.used[i] = true;
        }
    }
    b.channel_connections();
    b.read_lattices();
    b.deformer_connections();
    b.read_dynamics();

    // The time before anything that reads world matrices at this frame.
    if !import {
        if let Some(t) = doc.by_name.get("time1").and_then(|v| v.first()).and_then(|&i| doc.nodes[i].num(&["o", "outTime"])) {
            anim::set_frame(&mut b.s.graph, t);
        }
        if let Some(script) = doc.nodes.iter().find(|n| n.ty == "script" && n.name == "sceneConfigurationScriptNode") {
            playback_from_script(b.s, script, "override");
        }
    } else {
        // -importTimeRange: keep (the default) leaves the playback range alone.
        let mode = IMPORT_TIME_RANGE.with(|m| m.borrow().clone());
        if mode == "override" || mode == "combine" {
            if let Some(script) = doc.nodes.iter().find(|n| n.ty == "script" && n.name == "sceneConfigurationScriptNode") {
                playback_from_script(b.s, script, &mode);
            }
        }
    }

    b.s.graph.evaluate();
    for i in 0..count {
        if doc.nodes[i].ty == skin::SKIN_TYPE {
            b.fill_skin(i);
        }
    }
    b.fill_deformers();
    for i in 0..count {
        if let Some(kind) = rig::CONSTRAINT_TYPES.iter().position(|t| *t == doc.nodes[i].ty) {
            // Nothing reads a result until they are all in, except a point on poly constraint looking for its place on a mesh.
            b.s.graph.evaluation_suspended = doc.nodes[i].ty != "pointOnPolyConstraint";
            b.create_constraint(i, kind);
        }
    }
    b.s.graph.evaluation_suspended = false;
    b.constraint_connections();
    for i in 0..count {
        if doc.nodes[i].ty == "dagPose" {
            b.create_pose(i);
        }
    }
    for i in 0..count {
        b.apply_locks(i);
    }
    b.single_chain_rests();
    b.instances();
    b.read_shading_members();
    b.read_object_sets();
    b.curve_deformers();
    b.read_lights();
    b.reference_edits();
    for i in 0..count {
        if doc.nodes[i].locked {
            if let Some(id) = b.ids[i] {
                b.s.graph.set_node_locked(id, true);
            }
        }
    }
    crate::hik::after_load(&mut b.s.graph);
    crate::scripted::link_geom_matrices(&mut b.s.graph);
    b.replay_post_edits();
    // Keys on a joint an IK handle drives: on the joint in the file, on the solver's FK rotate here.
    for c in &doc.conns {
        let (Some(src), Some(joint)) = (b.ids[c.src], b.ids[c.dst]) else { continue };
        if !anim::is_curve(&b.s.graph, src) {
            continue;
        }
        let channel = match plain_attr(&c.dst_attr).as_str() {
            "rx" | "rotateX" => 0,
            "ry" | "rotateY" => 1,
            "rz" | "rotateZ" => 2,
            _ => continue,
        };
        let rotate = PlugRef::component(joint, Transform::ROTATE as i32, channel);
        if let Some(fk) = crate::ik::fk_plug_for(&b.s.graph, &rotate) {
            let out = PlugRef::new(src, b.s.graph.node(src).unwrap().ty().attr_index("output").unwrap() as i32);
            if b.s.graph.source(&fk).is_none() {
                let _ = b.s.graph.connect(&out, &fk, true);
                b.s.graph.disconnect(&rotate);
            }
        }
    }
    b.read_characters(&doc.characters, fps);
    b.hold_unused();
    b.report_unused();
    let warnings = std::mem::take(&mut b.warnings);
    // Each node as the file names it, for an import that renames what it brought in (-renamingPrefix).
    let named: Vec<(NodeId, String)> = (0..count).filter_map(|i| Some((b.ids[i]?, doc.nodes[i].name.clone()))).collect();
    LAST_READ_NAMES.with(|n| *n.borrow_mut() = named);
    let g = &mut s.graph;
    if import {
        g.select(&prior, SelectMode::Replace);
    } else {
        g.select(&[], SelectMode::Replace);
    }
    warnings
}

/// The playback range the reference keeps in a script node: `playbackOptions -min 1 -max 120 -ast 1 -aet 200 `.
fn playback_from_script(s: &mut Session, script: &MaNode, mode: &str) {
    let Some(text) = script.text(&["b", "before"]) else { return };
    let Some(at) = text.find("playbackOptions") else { return };
    let mut call = serde_json::Map::new();
    let words: Vec<&str> = text[at..].split(|c: char| c.is_whitespace() || c == ';').filter(|w| !w.is_empty()).collect();
    for w in words[1..].chunks(2) {
        let [flag, value] = w else { break };
        let Ok(v) = value.parse::<f64>() else { continue };
        let key = match *flag {
            "-min" => "minTime",
            "-max" => "maxTime",
            "-ast" => "animationStartTime",
            "-aet" => "animationEndTime",
            _ => continue,
        };
        call.insert(key.into(), json!(v));
    }
    // -importTimeRange combine: the scene's range widened to take in the file's.
    if mode == "combine" {
        for (k, wider) in [("minTime", -1.0), ("maxTime", 1.0), ("animationStartTime", -1.0), ("animationEndTime", 1.0)] {
            let now = s.run("playbackOptions", json!({"query": true, "queried": [k]})).ok().and_then(|v| v.as_f64());
            if let (Some(now), Some(f)) = (now, call.get(k).and_then(Value::as_f64)) {
                call.insert(k.into(), json!(if wider < 0.0 { now.min(f) } else { now.max(f) }));
            }
        }
    }
    if !call.is_empty() {
        let _ = s.run("playbackOptions", Value::Object(call));
    }
}

/// True while a scene file is being read: nodes may compute before all of their connections are made, which plug-ins
/// can take as a failure that isn't one (a node reading arrays that are not filled yet).
pub static READING: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

pub fn reading() -> bool {
    READING.load(std::sync::atomic::Ordering::Relaxed) > 0
}

struct ReadingGuard;
impl ReadingGuard {
    fn on() -> Self {
        READING.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        ReadingGuard
    }
}
impl Drop for ReadingGuard {
    fn drop(&mut self) {
        READING.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
    }
}

/// Reads .ma text into the session. `import` adds to the scene; otherwise the scene should already be empty.
pub fn read_text(s: &mut Session, text: &str, import: bool) -> Vec<String> {
    let _reading = ReadingGuard::on();
    register_types();
    let mut doc = Doc::parse(text);
    if !import {
        s.extras.file_info = doc.file_info.clone();
    }
    let dyn_warnings = dynamics::translate(&mut doc);
    doc.warnings.extend(dyn_warnings);
    doc.characters = characters::take(&mut doc);
    let mut before = Vec::new();
    // The plug-ins the file names, before its nodes. One that can't be found only matters if the file holds a type nobody knows.
    if s.commands.find("loadPlugin").is_some() {
        for (name, its_types) in &doc.requires {
            // Part of Aztek, not a plug-in: files name it so the reference knows what they want.
            if name == "aztekDynamics" {
                continue;
            }
            let loaded = s.run("loadPlugin", json!({"args": [name], "quiet": true}));
            let found = loaded.as_ref().is_ok_and(|v| v.as_array().is_some_and(|l| !l.is_empty()));
            // One that can't be loaded is kept as the scene's unknown plug-in (unknownPlugin), and written back.
            if !found {
                let e = s.extras.unknown_plugins.entry(name.clone()).or_default();
                e.version = doc.plugin_versions.get(name).cloned().unwrap_or_default();
                e.node_types = its_types.clone();
                e.data_types = doc.plugin_data_types.get(name).cloned().unwrap_or_default();
            }
            // A plug-in that names its types matters only when one of them is used and unknown here; one that names none,
            // when an unknown type is used that no plug-in claims.
            let claimed = |ty: &String| doc.requires.iter().any(|(_, t)| t.contains(ty));
            let missing = |n: &MaNode| types::find(&n.ty).is_none() && if its_types.is_empty() { !claimed(&n.ty) } else { its_types.contains(&n.ty) };
            if !found && doc.nodes.iter().any(missing) {
                match loaded {
                    Err(e) => before.push(format!("plug-in {name} did not load: {e}")),
                    // mGear's come from Help > Install mGear, built for Aztek.
                    Ok(_) if ["mgear_solvers", "mGearWeightDriver", "weightDriver", "cvwrap"].contains(&name.as_str()) => {
                        before.push(format!("plug-in {name} was not found on the plug-in path: it is mGear's, which Help > Install mGear installs"))
                    }
                    Ok(_) => before.push(format!("plug-in {name} was not found on the plug-in path")),
                }
            }
        }
    }
    // With the plug-ins loaded: what is still of a type nobody knows is kept as an unknown node, as the reference keeps it.
    before.extend(unknown::hold_dag(&mut doc));
    crate::references::begin_read(s, &doc.reference_edits_text(), &doc.ref_load, &mut before);
    for r in &doc.refs {
        if let Err(e) = crate::references::from_ma(s, r) {
            before.push(format!("reference {} not loaded: {e}", r.path));
        }
    }
    // A Z up scene's script node: `upAxis -ax "z"; viewSet -home persp;` when the session is Y up. Run here rather than
    // kept, so it works without the interface and is written afresh on save.
    let switch_up = doc.up_z && !import && !crate::world::up_z();
    if switch_up {
        crate::world::set_up_z(true);
    }
    // The file holds its default cameras' values that differ from the defaults of the axis it was saved in: start them
    // there.
    if !import {
        for name in ["persp", "top", "front", "side"] {
            reset_camera(s, name, doc.up_z);
        }
    }
    let mut warnings = build(s, &doc, import);
    if switch_up {
        home_camera(s, "persp");
    }
    crate::references::end_read(s, &mut warnings);
    crate::shading::scene_was_read(s);
    warnings.extend(before);
    for w in &warnings {
        log::warn!("{w}");
    }
    warnings
}

// ---------------------------------------------------------------------------------------------------------------------
// Writing

fn quoted(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

const VT_CHUNK: usize = 166;
const UV_CHUNK: usize = 250;
const FACE_CHUNK: usize = 250;

/// A mesh as the reference stores it: dense points, edges and faces, shared UVs per set.
struct Stored {
    points: Vec<DVec3>,
    /// Dense index per point id.
    point_of: Vec<i64>,
    /// (a, b, smooth).
    edges: Vec<(i64, i64, i64)>,
    /// Per face: signed edges and each set's UV per corner.
    faces: Vec<(Vec<i64>, Vec<Vec<i64>>)>,
    corners: usize,
    uv_sets: Vec<Vec<[f32; 2]>>,
    /// The sets' names, in the order of `uv_sets`, and the current one's.
    uv_names: Vec<String>,
    uv_current: String,
    /// The geometry's component tags: (name, components as written).
    tags: Vec<(String, Vec<String>)>,
}

/// A mesh node's componentTags as the file sets them: `.gtag[0].gtagnm` and `.gtag[0].gtagcmp`, into the geometry.
fn apply_file_tags(n: &MaNode, m: &mut Mesh) {
    let mut tags: BTreeMap<i64, (String, String)> = BTreeMap::new();
    for set in &n.sets {
        let [g, field] = set.path.as_slice() else { continue };
        if !matches!(g.name.as_str(), "gtag" | "componentTags") {
            continue;
        }
        let Some((k, _)) = g.range else { continue };
        let e = tags.entry(k).or_default();
        match field.name.as_str() {
            "gtagnm" | "componentTagName" => e.0 = set.values.first().and_then(Tok::text).unwrap_or_default().to_string(),
            "gtagcmp" | "componentTagContents" => e.1 = set.values.iter().filter_map(|v| if let Tok::Str(x) = v { Some(x.as_str()) } else { None }).collect::<Vec<_>>().join(" "),
            _ => {}
        }
    }
    for (_, (name, contents)) in tags {
        if name.is_empty() {
            continue;
        }
        let sel = crate::component_tags::parse(m, &contents).unwrap_or_else(|| crate::component_tags::Sel { cat: crate::component_tags::Cat::V, on: vec![false; m.point_capacity() as usize] });
        crate::component_tags::set(m, &name, &sel);
    }
}

fn stored_mesh(m: &Mesh) -> Stored {
    let mut point_of = vec![-1i64; m.point_capacity() as usize];
    let mut points = Vec::with_capacity(m.num_points() as usize);
    for p in 0..m.point_capacity() {
        if m.point_valid(p) {
            point_of[p as usize] = points.len() as i64;
            points.push(m.position(p));
        }
    }
    // Edges numbered and directed by first use around the faces, so writing what was read gives the same text.
    let hard = m.edge_attrs().get::<bool>(names::HARD);
    let mut edge_of = vec![-1i64; m.edge_capacity() as usize];
    let mut edges = Vec::with_capacity(m.num_edges() as usize);
    let smooth = |e: Index| !hard.get(e as usize).copied().unwrap_or(false) as i64 != 0;
    for f in 0..m.face_capacity() {
        if !m.face_valid(f) {
            continue;
        }
        for c in m.face_corners(f) {
            let e = m.corner_edge(c);
            if edge_of[e as usize] < 0 {
                edge_of[e as usize] = edges.len() as i64;
                edges.push((point_of[m.corner_point(c) as usize], point_of[m.corner_next_point(c) as usize], smooth(e) as i64));
            }
        }
    }
    for e in 0..m.edge_capacity() {
        if m.edge_valid(e) && edge_of[e as usize] < 0 {
            edge_of[e as usize] = edges.len() as i64;
            edges.push((point_of[m.edge_point(e, 0) as usize], point_of[m.edge_point(e, 1) as usize], smooth(e) as i64));
        }
    }
    // UV sets: uv0 through the same maps the UV Editor shows; others shared where point and value agree.
    let uv_current = crate::uv_sets::current(m);
    let uv_names: Vec<String> = crate::uv_sets::all(m).into_iter().filter(|n| crate::uv_sets::layer(m, n).is_some_and(|l| m.corner_attrs().has(&l))).collect();
    let mut uv_sets: Vec<Vec<[f32; 2]>> = Vec::new();
    let mut corner_uv: Vec<Vec<i64>> = Vec::new();
    for name in &uv_names {
        let uv = m.corner_attrs().get::<[f32; 2]>(&crate::uv_sets::layer(m, name).unwrap());
        let mut map = vec![-1i64; m.corner_capacity() as usize];
        let mut vals: Vec<[f32; 2]> = Vec::new();
        if *name == uv_current {
            let topo = crate::uv_ops::topo(m);
            let mut dense: HashMap<Index, i64> = HashMap::new();
            for f in 0..m.face_capacity() {
                if !m.face_valid(f) {
                    continue;
                }
                for c in m.face_corners(f) {
                    let id = topo.corner_map[c as usize];
                    let d = *dense.entry(id).or_insert_with(|| {
                        vals.push(uv[c as usize]);
                        (vals.len() - 1) as i64
                    });
                    map[c as usize] = d;
                }
            }
        } else {
            let mut seen: HashMap<(Index, [u32; 2]), i64> = HashMap::new();
            for f in 0..m.face_capacity() {
                if !m.face_valid(f) {
                    continue;
                }
                for c in m.face_corners(f) {
                    let key = (m.corner_point(c), [uv[c as usize][0].to_bits(), uv[c as usize][1].to_bits()]);
                    let d = *seen.entry(key).or_insert_with(|| {
                        vals.push(uv[c as usize]);
                        (vals.len() - 1) as i64
                    });
                    map[c as usize] = d;
                }
            }
        }
        uv_sets.push(vals);
        corner_uv.push(map);
    }
    let mut faces = Vec::with_capacity(m.num_faces() as usize);
    let mut corners = 0;
    for f in 0..m.face_capacity() {
        if !m.face_valid(f) {
            continue;
        }
        let cs = m.face_corners(f);
        corners += cs.len();
        let signed: Vec<i64> = cs
            .iter()
            .map(|&c| {
                let e = m.corner_edge(c);
                let d = edge_of[e as usize];
                if edges[d as usize].0 == point_of[m.corner_point(c) as usize] {
                    d
                } else {
                    !d
                }
            })
            .collect();
        let uvs: Vec<Vec<i64>> = corner_uv.iter().map(|map| cs.iter().map(|&c| map[c as usize]).collect()).collect();
        faces.push((signed, uvs));
    }
    let mut face_of = vec![-1i64; m.face_capacity() as usize];
    let mut k = 0;
    for f in 0..m.face_capacity() {
        if m.face_valid(f) {
            face_of[f as usize] = k;
            k += 1;
        }
    }
    let tags = crate::component_tags::names(m)
        .into_iter()
        .filter_map(|name| {
            let sel = crate::component_tags::get(m, &name)?;
            let map = match sel.cat {
                crate::component_tags::Cat::E => &edge_of,
                crate::component_tags::Cat::F => &face_of,
                _ => &point_of,
            };
            let mut on = vec![false; map.iter().filter(|&&x| x >= 0).count()];
            for i in sel.indices() {
                if let Some(&d) = map.get(i as usize).filter(|d| **d >= 0) {
                    on[d as usize] = true;
                }
            }
            Some((name, crate::component_tags::text(&crate::component_tags::Sel { cat: sel.cat, on })))
        })
        .collect();
    Stored { points, point_of, edges, faces, corners, uv_sets, uv_names, uv_current, tags }
}

fn write_stored(out: &mut String, m: &Stored) {
    if !m.tags.is_empty() {
        let _ = writeln!(out, "\tsetAttr -s {} \".gtag\";", m.tags.len());
        for (k, (name, comps)) in m.tags.iter().enumerate() {
            let _ = writeln!(out, "\tsetAttr \".gtag[{k}].gtagnm\" -type \"string\" {};", quoted(name));
            let list: Vec<String> = comps.iter().map(|c| quoted(c)).collect();
            let _ = writeln!(out, "\tsetAttr \".gtag[{k}].gtagcmp\" -type \"componentList\" {} {};", list.len(), list.join(" "));
        }
    }
    let _ = writeln!(out, "\tsetAttr \".vir\" yes;");
    let _ = writeln!(out, "\tsetAttr \".vif\" yes;");
    if m.uv_sets.is_empty() {
        let _ = writeln!(out, "\tsetAttr \".uvst[0].uvsn\" -type \"string\" \"map1\";");
    }
    for (k, set) in m.uv_sets.iter().enumerate() {
        let name = m.uv_names.get(k).cloned().unwrap_or_else(|| format!("map{}", k + 1)).replace('\\', "\\\\").replace('"', "\\\"");
        let _ = writeln!(out, "\tsetAttr \".uvst[{k}].uvsn\" -type \"string\" \"{name}\";");
        let _ = writeln!(out, "\tsetAttr -s {} \".uvst[{k}].uvsp\";", set.len());
        for (c, chunk) in set.chunks(UV_CHUNK).enumerate() {
            let a = c * UV_CHUNK;
            let _ = write!(out, "\tsetAttr \".uvst[{k}].uvsp[{}]\" -type \"float2\"", range_text(a, a + chunk.len() - 1));
            for uv in chunk {
                let _ = write!(out, " {} {}", uv[0], uv[1]);
            }
            let _ = writeln!(out, " ;");
        }
    }
    let current = if m.uv_names.contains(&m.uv_current) { m.uv_current.replace('\\', "\\\\").replace('"', "\\\"") } else { "map1".to_string() };
    let _ = writeln!(out, "\tsetAttr \".cuvs\" -type \"string\" \"{current}\";");
    let _ = writeln!(out, "\tsetAttr \".dcc\" -type \"string\" \"Ambient+Diffuse\";");
    let _ = writeln!(out, "\tsetAttr \".covm[0]\"  0 1 1;");
    let _ = writeln!(out, "\tsetAttr \".cdvm[0]\"  0 1 1;");
    let _ = writeln!(out, "\tsetAttr -s {} \".vt\";", m.points.len());
    for (c, chunk) in m.points.chunks(VT_CHUNK).enumerate() {
        let a = c * VT_CHUNK;
        let _ = write!(out, "\tsetAttr \".vt[{}]\" ", range_text(a, a + chunk.len() - 1));
        for p in chunk {
            let _ = write!(out, " {}", vec_text(file_vec(units::Unit::Linear, *p)));
        }
        let _ = writeln!(out, ";");
    }
    let _ = writeln!(out, "\tsetAttr -s {} \".ed\";", m.edges.len());
    for (c, chunk) in m.edges.chunks(VT_CHUNK).enumerate() {
        let a = c * VT_CHUNK;
        let _ = write!(out, "\tsetAttr \".ed[{}]\" ", range_text(a, a + chunk.len() - 1));
        for e in chunk {
            let _ = write!(out, " {} {} {}", e.0, e.1, e.2);
        }
        let _ = writeln!(out, ";");
    }
    for (c, chunk) in m.faces.chunks(FACE_CHUNK).enumerate() {
        let a = c * FACE_CHUNK;
        let head = if c == 0 { format!("-s {} -ch {} ", m.faces.len(), m.corners) } else { String::new() };
        let _ = write!(out, "\tsetAttr {head}\".fc[{}]\" -type \"polyFaces\"", range_text(a, a + chunk.len() - 1));
        for (edges, uvs) in chunk {
            let _ = write!(out, " \n\t\tf {}", edges.len());
            for e in edges {
                let _ = write!(out, " {e}");
            }
            for (k, list) in uvs.iter().enumerate() {
                let _ = write!(out, " mu {k} {}", list.len());
                for u in list {
                    let _ = write!(out, " {u}");
                }
            }
        }
        let _ = writeln!(out, " ;");
    }
}

fn matrix_text(m: &DMat4) -> String {
    m.to_cols_array().iter().map(|&v| if v == 0.0 { 0.0 } else { v }.to_string()).collect::<Vec<_>>().join(" ")
}

/// An index range as the reference prints it: [3] or [3:7].
fn range_text(a: usize, b: usize) -> String {
    if a == b {
        format!("{a}")
    } else {
        format!("{a}:{b}")
    }
}

/// Three numbers, with no negative zeros.
/// A vector as the file holds it: in the current units, which its header names.
fn file_vec(unit: units::Unit, v: DVec3) -> DVec3 {
    DVec3::new(units::to_user(unit, v.x), units::to_user(unit, v.y), units::to_user(unit, v.z))
}

/// A default node's plug as the reference writes it, `:time1.o`.
fn default_colon(plug: &str) -> String {
    let node = plug.split(['.', '[']).next().unwrap_or("");
    if crate::defaults::is_default_node(node) {
        format!(":{plug}")
    } else {
        plug.to_string()
    }
}

fn vec_text(v: DVec3) -> String {
    let n = |x: f64| if x == 0.0 { 0.0 } else { x };
    format!("{} {} {}", n(v.x), n(v.y), n(v.z))
}

struct Writer<'a> {
    /// Deformers written with their mesh.
    chain_written: HashSet<NodeId>,
    /// Creators, edits and intermediate meshes written as a mesh's history: their mesh connections are written with them.
    history_written: HashSet<NodeId>,
    s: &'a mut Session,
    out: String,
    /// Connections, sorted when written.
    conns: Vec<String>,
    /// Lines with -na, written after the others.
    shading: Vec<String>,
    warnings: Vec<String>,
    /// Names the file uses, so made-up nodes (effectors, Orig shapes) don't clash.
    used: HashSet<String>,
    /// What gets written; None is everything.
    keep: HashSet<NodeId>,
    /// Node text written after the DAG: skins, constraints, effectors, poses.
    pending: Vec<String>,
    /// Solver nodes already written.
    solvers: HashSet<String>,
    /// Shading groups the light linker has to name, and its lines.
    needs_links: Vec<String>,
    links_text: String,
    /// A character member, and the character's value that drives it (`hero.lv[1]`).
    char_driven: HashMap<PlugRef, String>,
    /// A clip channel, and the character's value its member's keys go to.
    char_channels: HashMap<NodeId, String>,
}

impl Writer<'_> {
    fn fresh(&mut self, wanted: &str) -> String {
        let mut name = wanted.to_string();
        let mut k = 1;
        while self.used.contains(&name) {
            name = format!("{wanted}{k}");
            k += 1;
        }
        self.used.insert(name.clone());
        name
    }

    fn conn(&mut self, a: &str, b: &str) {
        self.conns.push(format!("connectAttr {} {};", quoted(&default_colon(a)), quoted(&default_colon(b))));
    }

    /// The node's values that differ from their defaults, then its locks.
    fn write_channels(&mut self, id: NodeId) {
        let t = self.s.graph.node(id).unwrap().type_arc().clone();
        let channels = channels_of(&t);
        let is_shape = t.is_a("shape");
        if is_shape {
            let _ = writeln!(self.out, "\tsetAttr -k off \".v\";");
        }
        for (i, a) in t.attrs.iter().enumerate() {
            if !channels.contains(&a.name.as_str()) && !a.has(flags::USER) {
                continue;
            }
            let p = PlugRef::new(id, i as i32);
            let v = match self.s.graph.value(&p) {
                Ok(v) => v,
                Err(_) => continue,
            };
            let name = ref_name(a, -1);
            if a.has(flags::USER) && !a.has(flags::EXTENSION) {
                let line = attr_add_lines(a);
                self.out.push_str(&line);
            }
            if v == a.default_value {
                continue;
            }
            let kflag = if is_shape && a.name == "visibility" { "-k off " } else { "" };
            match v {
                PlugValue::Vec3(x) => {
                    let ty = if a.has(flags::COLOR) { "float3" } else { "double3" };
                    let _ = writeln!(self.out, "\tsetAttr \".{name}\" -type \"{ty}\" {} ;", vec_text(file_vec(a.unit, x)));
                }
                PlugValue::Float(x) => {
                    let x = units::to_user(a.unit, x);
                    let _ = writeln!(self.out, "\tsetAttr {kflag}\".{name}\" {x};");
                }
                PlugValue::Int(x) => {
                    let _ = writeln!(self.out, "\tsetAttr {kflag}\".{name}\" {x};");
                }
                PlugValue::Bool(x) => {
                    let _ = writeln!(self.out, "\tsetAttr {kflag}\".{name}\" {};", if x { "yes" } else { "no" });
                }
                PlugValue::String(x) => {
                    let _ = writeln!(self.out, "\tsetAttr \".{name}\" -type \"string\" {};", quoted(&x));
                }
                PlugValue::Matrix(m) => {
                    let _ = writeln!(self.out, "\tsetAttr \".{name}\" -type \"matrix\" {};", matrix_text(&m));
                }
                _ => {}
            }
        }
        for (i, a) in t.attrs.iter().enumerate() {
            if !channels.contains(&a.name.as_str()) && !a.has(flags::USER) {
                continue;
            }
            let mask = self.s.graph.node(id).unwrap().lock_mask(i);
            if mask == 0 {
                continue;
            }
            if a.ty == PlugType::Vec3 {
                for c in 0..3 {
                    if mask & (1 << c) != 0 {
                        let _ = writeln!(self.out, "\tsetAttr -l on \".{}\";", ref_name(a, c));
                    }
                }
            } else {
                let _ = writeln!(self.out, "\tsetAttr -l on \".{}\";", ref_name(a, -1));
            }
        }
    }

    fn create_line(&mut self, ty: &str, name: &str, parent: NodeId) {
        if parent == NO_NODE {
            let _ = writeln!(self.out, "createNode {ty} -n {};", quoted(name));
        } else {
            let referenced = self.s.graph.node(parent).is_some_and(|n| n.reference_id() != 0);
            let p = if referenced { self.s.graph.full_path(parent) } else { self.s.graph.path(parent) };
            let _ = writeln!(self.out, "createNode {ty} -n {} -p {};", quoted(name), quoted(&p));
        }
    }

    /// The written ancestor of a node, skipping nodes that aren't written.
    fn written_parent(&self, id: NodeId) -> NodeId {
        let g = &self.s.graph;
        let mut p = g.parent(id);
        while p != NO_NODE {
            // A node of a reference is there when the file is read.
            if self.dag_written(p) || g.node(p).is_some_and(|n| n.reference_id() != 0) {
                return p;
            }
            p = g.parent(p);
        }
        NO_NODE
    }

    fn dag_written(&self, id: NodeId) -> bool {
        let Some(n) = self.s.graph.node(id) else { return false };
        if !self.keep.contains(&id) {
            return false;
        }
        let t = n.ty();
        // A curve or surface's original holds what its deformers read; a mesh's is baked into its shape's history.
        if t.is_a("mesh") {
            return !n.bool(DagNode::INTERMEDIATE_OBJECT);
        }
        if t.is_a("nurbsCurve") || t.is_a("nurbsSurface") {
            return true;
        }
        // The default cameras are the application's own; every scene has them.
        if crate::cameras::is_default_camera(&self.s.graph, id) {
            return false;
        }
        t.name == "transform" || crate::deform::handle_shape_kind(&t.name).is_some() || matches!(t.name.as_str(), crate::core_nodes::UNKNOWN_DAG | crate::core_nodes::UNKNOWN_TRANSFORM) || t.is_a("joint") || t.is_a("locator") || t.is_a("ikHandle") || t.is_a("camera") || t.is_a("light") || PLAIN_SHAPES.contains(&t.name.as_str())
    }

    fn write_dag(&mut self, id: NodeId) {
        if self.dag_written(id) {
            let parent = self.written_parent(id);
            let (name, ty) = {
                let n = self.s.graph.node(id).unwrap();
                (n.name().to_string(), n.ty().name.clone())
            };
            // An instance is its source's shape under one more transform, as the reference has it.
            if ty == "mesh" && crate::instancing::kind(&self.s.graph, id) == crate::instancing::Kind::Instance {
                let src = crate::instancing::source(&self.s.graph, id);
                if src != id && self.dag_written(src) {
                    let (a, b) = (self.s.graph.full_path(src), self.s.graph.path(parent));
                    self.pending.push(format!("parent -s -nc -r -add {} {};\n", quoted(&a), quoted(&b)));
                    return;
                }
            }
            if crate::core_nodes::is_unknown(&ty) {
                self.write_unknown_dag(id, &name, parent);
                for c in self.s.graph.children(id).to_vec() {
                    self.write_dag(c);
                }
                return;
            }
            self.create_line(&ty, &name, parent);
            // Influence locks are dynamic attributes in the file, even when the scene stores them on the skin.
            let g = &self.s.graph;
            let node_type = g.node(id).unwrap().ty();
            if node_type.is_a("transform") && node_type.attr_index("liw").is_none()
                && g.destinations(&PlugRef::new(id, Transform::WORLD_MATRIX as i32)).iter().any(|d| {
                    d.element >= 0 && self.keep.contains(&d.node) && g.node(d.node).is_some_and(|n| {
                        n.ty().name == skin::SKIN_TYPE && n.ty().attr_index("matrix") == Some(d.attr as usize)
                    })
                })
            {
                self.out.push_str("\taddAttr -ci true -sn \"liw\" -ln \"lockInfluenceWeights\" -at \"bool\";\n");
            }
            self.write_channels(id);
            if is_plain_node(&ty) {
                let text = self.plain_attr_lines(id);
                self.out.push_str(&text);
            }
            if ty == "volumeLight" {
                self.write_ramps(id);
            }
            if self.s.graph.node(id).is_some_and(|n| n.is_locked()) {
                let _ = writeln!(self.out, "\tlockNode -l 1 ;");
            }
            if ty == "mesh" {
                // Smooth mesh preview as the reference writes it.
                let path = self.s.graph.path(id);
                let read = |g: &mut Graph, attr: &str| g.plug(&format!("{path}.{attr}")).ok().and_then(|p| g.value(&p).ok());
                let g = &mut self.s.graph;
                let (dsm, level, render, use_preview) = (read(g, "displaySmoothMesh"), read(g, "smoothLevel"), read(g, "renderSmoothLevel"), read(g, "useSmoothPreviewForRender"));
                if let Some(PlugValue::Int(d)) = dsm.filter(|d| d.as_int() != 0) {
                    let _ = writeln!(self.out, "\tsetAttr \".displaySmoothMesh\" {d};");
                }
                if let Some(PlugValue::Int(l)) = level.filter(|l| l.as_int() != 2) {
                    let _ = writeln!(self.out, "\tsetAttr \".smoothLevel\" {l};");
                }
                if let Some(PlugValue::Int(l)) = render.filter(|l| l.as_int() != 2) {
                    let _ = writeln!(self.out, "\tsetAttr \".renderSmoothLevel\" {l};");
                }
                if matches!(use_preview, Some(PlugValue::Bool(false))) {
                    let _ = writeln!(self.out, "\tsetAttr \".useSmoothPreviewForRender\" no;");
                }
                // Polygon Display switches that are on.
                for (attr, short, _) in crate::mesh_nodes::DISPLAY_SWITCHES {
                    if matches!(read(g, attr), Some(PlugValue::Bool(true))) {
                        let _ = writeln!(self.out, "\tsetAttr \".{short}\" yes;");
                    }
                }
                self.write_mesh(id, &name, parent);
            }
            if crate::deform::handle_shape_kind(&ty).is_some() {
                self.write_handle_shape(id);
            }
            if ty == "lattice" {
                self.write_lattice_points(id);
            }
            if ty == "nurbsCurve" || ty == "nurbsSurface" {
                if self.s.graph.node(id).is_some_and(|n| n.bool(DagNode::INTERMEDIATE_OBJECT)) {
                    let _ = writeln!(self.out, "\tsetAttr \".io\" yes;");
                }
            }
            if ty == "nurbsCurve" {
                self.write_nurbs_curve(id);
                self.write_nurbs_chain(id, crate::curves::CurveShape::CREATE);
                self.write_curve_deformer_input(id, &name, parent);
            }
            if ty == "nurbsSurface" {
                self.write_nurbs_surface(id);
                self.write_nurbs_chain(id, crate::surfaces::SurfaceShape::CREATE);
                if !self.s.graph.node(id).is_some_and(|n| n.bool(DagNode::INTERMEDIATE_OBJECT)) {
                    let path = self.s.graph.path(id);
                    self.write_membership(id, &path, 0);
                }
            }
            if ty == crate::xjen::DESCRIPTION {
                let path = self.s.graph.path(id);
                self.write_membership(id, &path, 0);
            }
            if ty == "camera" {
                self.write_camera(id);
            }
            if ty == "ikHandle" {
                let _ = writeln!(self.out, "\tsetAttr \".roc\" yes;");
            }
            // The parent's scale into inverseScale, where the scene has it: a rig may take it away (mGear does).
            let is_link = PlugRef::new(id, rig::Joint::INVERSE_SCALE as i32);
            if ty == "joint" && parent != NO_NODE && self.s.graph.node(parent).is_some_and(|n| n.ty().is_a("joint")) && self.s.graph.source(&is_link).is_some_and(|s| s.node == parent) {
                let (a, b) = (self.s.graph.path(parent), self.s.graph.path(id));
                self.conn(&format!("{a}.s"), &format!("{b}.is"));
            }
        }
        for c in self.s.graph.children(id).to_vec() {
            self.write_dag(c);
        }
    }

    /// A plug-in's deformer on this curve, as the reference has one: the curve the deformer keeps written as the
    /// shape's original (an intermediate curve beside it), into the deformer's input, and the deformer's output into
    /// the shape.
    fn write_curve_deformer_input(&mut self, shape: NodeId, name: &str, parent: NodeId) {
        let g = &self.s.graph;
        let Some(src) = g.source(&PlugRef::new(shape, crate::curves::CurveShape::CREATE as i32)) else { return };
        let Some(dn) = g.node(src.node) else { return };
        let dt = dn.type_arc().clone();
        if !crate::scripted::is_plugin_deformer(&dt.name) || dt.attr_index("outputCurve") != Some(src.attr as usize) || !self.keep.contains(&src.node) {
            return;
        }
        let Some(inp) = dt.attr_index("inputCurve") else { return };
        let curve = match dn.data().value(inp) {
            PlugValue::Data(Some(d)) => d.as_any().downcast_ref::<crate::curves::CurveData>().map(|c| c.0.clone()),
            _ => None,
        };
        let Some(c) = curve else { return };
        let orig = format!("{name}Orig");
        let p = if parent == NO_NODE { String::new() } else { self.s.graph.path(parent) };
        let (def, shape_path) = (self.s.graph.path(src.node), self.s.graph.path(shape));
        let orig_path = if p.is_empty() { orig.clone() } else { format!("{p}|{orig}") };
        if p.is_empty() {
            let _ = writeln!(self.out, "createNode nurbsCurve -n {};", quoted(&orig));
        } else {
            let _ = writeln!(self.out, "createNode nurbsCurve -n {} -p {};", quoted(&orig), quoted(&p));
        }
        let _ = writeln!(self.out, "\tsetAttr -k off \".v\";");
        let _ = writeln!(self.out, "\tsetAttr \".io\" yes;");
        let _ = writeln!(self.out, "\tsetAttr \".cc\" -type \"nurbsCurve\" ");
        let _ = writeln!(self.out, "\t\t{} {} {} no 3", c.degree, c.spans(), c.form.index());
        let knots: Vec<String> = c.knots.iter().map(|k| fmt_num(*k)).collect();
        let _ = writeln!(self.out, "\t\t{} {}", c.knots.len(), knots.join(" "));
        let _ = writeln!(self.out, "\t\t{}", c.cvs.len());
        for v in &c.cvs {
            let _ = writeln!(self.out, "\t\t{} {} {}", fmt_num(v.x), fmt_num(v.y), fmt_num(v.z));
        }
        let _ = writeln!(self.out, "\t\t;");
        self.conn(&format!("{orig_path}.ws"), &format!("{def}.ip[0].ig"));
        self.conn(&format!("{def}.og[0]"), &format!("{shape_path}.cr"));
    }

    /// The curve as `.cc`: degree, spans, form, rational, dimension, knots, CVs.
    fn write_nurbs_curve(&mut self, shape: NodeId) {
        let Some(c) = crate::curves::local_curve(&mut self.s.graph, shape) else { return };
        let _ = writeln!(self.out, "\tsetAttr \".cc\" -type \"nurbsCurve\" ");
        let _ = writeln!(self.out, "\t\t{} {} {} no 3", c.degree, c.spans(), c.form.index());
        let knots: Vec<String> = c.knots.iter().map(|k| fmt_num(*k)).collect();
        let _ = writeln!(self.out, "\t\t{} {}", c.knots.len(), knots.join(" "));
        let _ = writeln!(self.out, "\t\t{}", c.cvs.len());
        for p in &c.cvs {
            let _ = writeln!(self.out, "\t\t{} {} {}", fmt_num(p.x), fmt_num(p.y), fmt_num(p.z));
        }
        let _ = writeln!(self.out, "\t\t;");
    }

    /// The surface as `.cc`: degrees, forms, rational, each knot vector, then CVs with U the outer loop.
    fn write_nurbs_surface(&mut self, shape: NodeId) {
        let Some(c) = crate::surfaces::local_surface(&mut self.s.graph, shape) else { return };
        let _ = writeln!(self.out, "\tsetAttr \".cc\" -type \"nurbsSurface\" ");
        let _ = writeln!(self.out, "\t\t{} {} {} {} no ", c.degree_u, c.degree_v, c.form_u.index(), c.form_v.index());
        for k in [&c.knots_u, &c.knots_v] {
            let t: Vec<String> = k.iter().map(|x| fmt_num(*x)).collect();
            let _ = writeln!(self.out, "\t\t{} {}", k.len(), t.join(" "));
        }
        let _ = writeln!(self.out, "\t\t{}", c.nu * c.nv);
        for u in 0..c.nu {
            for v in 0..c.nv {
                let p = c.cv(u, v);
                let _ = writeln!(self.out, "\t\t{} {} {}", fmt_num(p.x), fmt_num(p.y), fmt_num(p.z));
            }
        }
        let _ = writeln!(self.out, "\t\t;");
        // The reference's own trim format isn't written: an attribute of ours holds the regions, which the reference keeps but doesn't draw.
        if !c.trim.is_empty() {
            let _ = writeln!(self.out, "\taddAttr -ci true -sn \"aztekTrim\" -ln \"aztekTrim\" -dt \"string\";");
            let _ = writeln!(self.out, "\tsetAttr \".aztekTrim\" -type \"string\" {};", quoted(&crate::surfaces::trim_to_json(&c.trim).to_string()));
        }
    }

    /// The four default cameras, marked -s as the reference marks them: every scene has them and a file only says where they are.
    fn write_default_cameras(&mut self) {
        for name in ["persp", "top", "front", "side"] {
            let Ok(Some(tr)) = self.s.graph.find(name) else { continue };
            let Some(shape) = crate::cameras::shape_of(&self.s.graph, tr) else { continue };
            if !crate::cameras::is_default_camera(&self.s.graph, tr) {
                continue;
            }
            let shape_name = self.s.graph.node(shape).map_or(String::new(), |n| n.name().to_string());
            let _ = writeln!(self.out, "createNode transform -s -n {};", quoted(name));
            self.write_channels(tr);
            let _ = writeln!(self.out, "createNode camera -s -n {} -p {};", quoted(&shape_name), quoted(name));
            self.write_camera(shape);
        }
    }

    fn write_camera(&mut self, shape: NodeId) {
        use crate::cameras as cam;
        let Some(n) = self.s.graph.node(shape) else { return };
        let d = n.data();
        for (attr, at, def) in [(".fl", cam::FOCAL_LENGTH, 35.0), (".hfa", cam::H_APERTURE, 1.41732), (".vfa", cam::V_APERTURE, 0.94488), (".ncp", cam::NEAR_CLIP, 0.1), (".fcp", cam::FAR_CLIP, 10000.0), (".coi", cam::COI, 5.0), (".ow", cam::ORTHO_WIDTH, 30.0)] {
            let v = d.float(at);
            if (v - def).abs() > 1e-9 {
                let _ = writeln!(self.out, "\tsetAttr \"{attr}\" {};", fmt_num(v));
            }
        }
        if d.bool(cam::ORTHOGRAPHIC) {
            let _ = writeln!(self.out, "\tsetAttr \".o\" yes;");
        }
        let t = n.type_arc().clone();
        for (at, short) in cam::plain_attrs(&self.s.graph, shape) {
            let def = &t.attrs[at];
            let Some(n) = self.s.graph.node(shape) else { return };
            match n.value(at).clone() {
                PlugValue::Bool(v) if !matches!(def.default_value, PlugValue::Bool(x) if x == v) => {
                    let _ = writeln!(self.out, "\tsetAttr \".{short}\" {};", if v { "yes" } else { "no" });
                }
                PlugValue::Float(v) if !matches!(def.default_value, PlugValue::Float(x) if (x - v).abs() < 1e-12) => {
                    let _ = writeln!(self.out, "\tsetAttr \".{short}\" {};", fmt_num(v));
                }
                PlugValue::String(v) if !v.is_empty() => {
                    let _ = writeln!(self.out, "\tsetAttr \".{short}\" -type \"string\" {};", quoted(&v));
                }
                _ => {}
            }
        }
    }

    fn write_mesh(&mut self, shape: NodeId, name: &str, parent: NodeId) {
        let g = &mut self.s.graph;
        let path = g.path(shape);
        let out_plug = PlugRef::new(shape, MeshNode::OUT_MESH as i32);
        let chain = self.deformer_chain(shape);
        self.write_membership(shape, &path, chain.len());
        match chain.first().copied() {
            Some(first) => {
                let t = self.s.graph.node(first).unwrap().type_arc().clone();
                let ip = t.attr_index("inputPolymesh").unwrap();
                let base = self.s.graph.value(&PlugRef::new(first, ip as i32)).ok().and_then(|v| v.as_mesh().cloned());
                let Some(base) = base else { return };
                let stored = stored_mesh(&base);
                // The history before the deformers feeds the original, as the reference has it, when it reads back
                // numbered as it is here: the deformers' points are the original's.
                let into = PlugRef::new(first, ip as i32);
                let start = match self.s.graph.source(&into) {
                    Some(s) if self.s.graph.node(s.node).is_some_and(|n| n.ty().is_a("mesh") && n.bool(DagNode::INTERMEDIATE_OBJECT)) => PlugRef::new(s.node, MeshNode::IN_MESH as i32),
                    _ => into,
                };
                let history = match self.history_chain(start) {
                    Ok(Some(h)) if base.num_points() == base.point_capacity() => Some(h),
                    Ok(Some(_)) => {
                        self.warnings.push(format!("{path}: history baked into its original: its points are numbered with gaps, which the deformers' weights would not survive"));
                        None
                    }
                    Ok(None) => None,
                    Err(why) => {
                        self.warnings.push(format!("{path}: history baked into its original: {why}"));
                        None
                    }
                };
                // The scene's own original isn't written as itself: this one takes its place and its name.
                let wanted = format!("{name}Orig");
                if self.s.graph.find(&wanted).ok().flatten().is_some_and(|o| self.s.graph.node(o).is_some_and(|x| x.bool(DagNode::INTERMEDIATE_OBJECT))) {
                    self.used.remove(&wanted);
                }
                let orig = self.fresh(&wanted);
                self.create_line("mesh", &orig, parent);
                let _ = writeln!(self.out, "\tsetAttr -k off \".v\";");
                let _ = writeln!(self.out, "\tsetAttr \".io\" yes;");
                match history {
                    Some(h) => {
                        // The original's own tags, which the deformers' sets may name.
                        if let Some(m) = self.s.graph.source(&into).map(|s| s.node).filter(|&m| self.s.graph.node(m).is_some_and(|n| n.ty().is_a("mesh"))) {
                            self.node_tag_lines(m);
                        }
                        self.write_history(&h, &path, parent, &format!("{orig}.i"));
                    }
                    None => write_stored(&mut self.out, &stored),
                }
                self.write_chain(&chain, &path, &orig, &stored);
            }
            None => {
                // A mesh with history is written as its history, the shape holding no geometry of its own (#209).
                match self.history_chain(PlugRef::new(shape, MeshNode::IN_MESH as i32)) {
                    Ok(Some(h)) => {
                        self.node_tag_lines(shape);
                        self.write_history(&h, &path, parent, &format!("{path}.i"));
                        return;
                    }
                    Ok(None) => {}
                    Err(why) => self.warnings.push(format!("{path}: history baked: {why}")),
                }
                let m = self.s.graph.value(&out_plug).ok().and_then(|v| v.as_mesh().cloned());
                if let Some(m) = m {
                    write_stored(&mut self.out, &stored_mesh(&m));
                }
            }
        }
    }

    fn pending_nodes_push(&mut self, text: String) {
        self.pending.push(text);
    }

    fn skin_text(&mut self, sk: NodeId, stored: &Stored) -> String {
        let mut o = String::new();
        let name = self.s.graph.path(sk);
        let _ = writeln!(o, "createNode skinCluster -n {};", quoted(&name));
        let Some(bind) = skin::bind_of(&self.s.graph, sk) else { return o };
        // Weights per written point.
        let mut per_point: Vec<&[(u32, f64)]> = vec![&[]; stored.points.len()];
        for (p, w) in bind.weights.iter().enumerate() {
            if let Some(&d) = stored.point_of.get(p) {
                if d >= 0 {
                    per_point[d as usize] = w;
                }
            }
        }
        let _ = writeln!(o, "\tsetAttr -s {} \".wl\";", per_point.len());
        for (p, w) in per_point.iter().enumerate() {
            let runs = weight_runs(w);
            if runs.len() > 1 {
                let _ = writeln!(o, "\tsetAttr -s {} \".wl[{p}].w\";", w.len());
            }
            for run in runs {
                let (first, last) = (run[0].0, run[run.len() - 1].0);
                let vals = run.iter().map(|x| x.1.to_string()).collect::<Vec<_>>().join(" ");
                if first == last {
                    let _ = writeln!(o, "\tsetAttr \".wl[{p}].w[{first}]\" {vals};");
                } else {
                    let _ = writeln!(o, "\tsetAttr -s {} \".wl[{p}].w[{first}:{last}]\"  {vals};", run.len());
                }
            }
        }
        let infs = skin::influences(&self.s.graph, sk);
        let _ = writeln!(o, "\tsetAttr -s {} \".pm\";", infs.len());
        for &(e, _) in &infs {
            let m = bind.bind_pre.get(e as usize).copied().unwrap_or(DMat4::IDENTITY);
            let _ = writeln!(o, "\tsetAttr \".pm[{e}]\" -type \"matrix\" {};", matrix_text(&m));
        }
        let _ = writeln!(o, "\tsetAttr \".gm\" -type \"matrix\" {};", matrix_text(&bind.geom));
        let _ = writeln!(o, "\tsetAttr -s {} \".ma\";", infs.len());
        let t = self.s.graph.node(sk).unwrap().type_arc().clone();
        let n = self.s.graph.node(sk).unwrap();
        let (mi, mmi, dr, en, sm, nw) = (
            n.data().int(t.attr_index("maxInfluences").unwrap()),
            n.bool(t.attr_index("maintainMaxInfluences").unwrap()),
            n.data().float(t.attr_index("dropoffRate").unwrap()),
            n.data().float(t.attr_index("envelope").unwrap()),
            n.data().int(t.attr_index("skinningMethod").unwrap()),
            n.data().int(t.attr_index("normalizeWeights").unwrap()),
        );
        if !infs.is_empty() {
            let _ = writeln!(o, "\tsetAttr -s {} \".dpf[0:{}]\"  {};", infs.len(), infs.iter().map(|x| x.0).max().unwrap_or(0), vec![dr.to_string(); infs.iter().map(|x| x.0).max().unwrap_or(0) as usize + 1].join(" "));
        }
        let _ = writeln!(o, "\tsetAttr -s {} \".lw\";", infs.len());
        if en != 1.0 {
            let _ = writeln!(o, "\tsetAttr \".en\" {en};");
        }
        let _ = writeln!(o, "\tsetAttr \".mmi\" {};", if mmi { "yes" } else { "no" });
        let _ = writeln!(o, "\tsetAttr \".mi\" {mi};");
        let _ = writeln!(o, "\tsetAttr \".dr\" {dr};");
        let _ = writeln!(o, "\tsetAttr \".skm\" {sm};");
        let _ = writeln!(o, "\tsetAttr \".nw\" {nw};");
        let _ = writeln!(o, "\tsetAttr \".ucm\" yes;");
        for &(e, j) in &infs {
            let jp = self.s.graph.path(j);
            if bind.locks.get(e as usize).copied().unwrap_or(false) {
                let _ = writeln!(o, "setAttr {} yes;", quoted(&format!("{jp}.liw")));
            }
            self.conn(&format!("{jp}.wm"), &format!("{name}.ma[{e}]"));
            self.conn(&format!("{jp}.liw"), &format!("{name}.lw[{e}]"));
            self.conn(&format!("{jp}.obcc"), &format!("{name}.ifcl[{e}]"));
        }
        // Bind pre-matrices a connection drives (a skin on a local rig).
        let pm = t.attr_index("bindPreMatrix").unwrap() as i32;
        let into: Vec<(PlugRef, PlugRef)> = self.s.graph.connections().into_iter().filter(|(d, _)| d.node == sk && d.attr == pm).collect();
        for (dst, src) in into {
            let from = self.s.graph.plug_name(&src);
            self.conn(&from, &format!("{name}.pm[{}]", dst.element));
        }
        o
    }

    fn write_curve(&mut self, id: NodeId) {
        let n = self.s.graph.node(id).unwrap();
        let t = n.type_arc().clone();
        let name = n.name().to_string();
        let keys = anim::keys_of(&self.s.graph, id);
        let n = self.s.graph.node(id).unwrap();
        let (pre, post) = (n.data().int(t.attr_index("preInfinity").unwrap()), n.data().int(t.attr_index("postInfinity").unwrap()));
        let text = curve_lines(&t.name, &name, &keys, pre, post);
        self.out.push_str(&text);
    }

    fn write_constraint(&mut self, id: NodeId, kind: usize) {
        let g = &self.s.graph;
        let n = g.node(id).unwrap();
        let t = n.type_arc().clone();
        let name = n.name().to_string();
        let targets = crate::constraint::target_nodes(g, id);
        let Some(driven) = crate::constraint::driven_of(g, id).filter(|d| self.keep.contains(d)) else { return };
        if targets.iter().any(|(_, t)| !self.keep.contains(t)) {
            self.warnings.push(format!("{name}: a target is not exported; constraint omitted"));
            return;
        }
        let path = g.path(driven);
        let aliases = self.s.run(&t.name, json!({"args": [name.clone()], "query": true, "queried": ["weightAliasList"]})).unwrap_or(Value::Null);
        let mut weights: Vec<f64> = Vec::new();
        for alias in aliases.as_array().into_iter().flatten().filter_map(Value::as_str) {
            weights.push(self.s.run("getAttr", json!({"args": [format!("{name}.{alias}")]})).ok().and_then(|v| v.as_f64()).unwrap_or(1.0));
        }
        let g = &self.s.graph;
        let mut body = String::new();
        let mut conns: Vec<(String, String)> = Vec::new();
        let _ = writeln!(body, "createNode {} -n {} -p {};", t.name, quoted(&name), quoted(&path));
        let c = g.path(driven) + "|" + &name;
        let _ = writeln!(body, "\tsetAttr -s {} \".tg\";", targets.len());
        for &(e, target) in &targets {
            let tp = g.path(target);
            let tname = g.node(target).unwrap().name().rsplit(':').next().unwrap_or("").to_string();
            let w = weights.get(e as usize).copied().unwrap_or(1.0);
            let _ = writeln!(body, "\taddAttr -dcb 0 -ci true -k true -sn \"w{e}\" -ln \"{tname}W{e}\" -dv 1 -min 0 -at \"double\";");
            if w != 1.0 {
                let _ = writeln!(body, "\tsetAttr -k on \".w{e}\" {w};");
            }
            conns.push((format!("{c}.w{e}"), format!("{c}.tg[{e}].tw")));
            let is_joint = g.node(target).unwrap().ty().is_a("joint");
            let leaves: &[(&str, &str)] = match kind {
                0 => &[("t", "tt"), ("rp", "trp"), ("rpt", "trt"), ("r", "tr"), ("ro", "tro"), ("s", "ts"), ("pm", "tpm")],
                1 | 4 => &[("t", "tt"), ("rp", "trp"), ("rpt", "trt"), ("pm", "tpm")],
                2 => &[("r", "tr"), ("ro", "tro"), ("pm", "tpm")],
                _ => &[("s", "ts"), ("pm", "tpm")],
            };
            for (a, b) in leaves {
                conns.push((format!("{tp}.{a}"), format!("{c}.tg[{e}].{b}")));
            }
            if is_joint && matches!(kind, 0 | 2) {
                conns.push((format!("{tp}.jo"), format!("{c}.tg[{e}].tjo")));
                conns.push((format!("{tp}.ssc"), format!("{c}.tg[{e}].tsc")));
                conns.push((format!("{tp}.is"), format!("{c}.tg[{e}].tis")));
            }
            if t.name == "parentConstraint" {
                let n = g.node(id).unwrap();
                let at = |a: &str, e: i32| -> DVec3 {
                    t.attr_index(a).and_then(|i| n.data().element(i, e).cloned()).and_then(|v| if let PlugValue::Vec3(x) = v { Some(x) } else { None }).unwrap_or(DVec3::ZERO)
                };
                let (tot, tor) = (at("target.targetOffsetTranslate", e), at("target.targetOffsetRotate", e));
                if tot != DVec3::ZERO {
                    let _ = writeln!(body, "\tsetAttr \".tg[{e}].tot\" -type \"double3\" {} ;", vec_text(file_vec(units::Unit::Linear, tot)));
                }
                if tor != DVec3::ZERO {
                    let _ = writeln!(body, "\tsetAttr \".tg[{e}].tor\" -type \"double3\" {} ;", vec_text(file_vec(units::Unit::Angle, tor)));
                }
            }
        }
        // The other kinds hold one offset for the node.
        if let Some(i) = t.attr_index("offset").filter(|_| t.name != "parentConstraint") {
            let v = g.node(id).unwrap().data().vec3(i);
            let default = if t.name == "scaleConstraint" { DVec3::ONE } else { DVec3::ZERO };
            if v != default {
                let _ = writeln!(body, "\tsetAttr \".o\" -type \"double3\" {} ;", vec_text(file_vec(t.attrs[i].unit, v)));
            }
        }
        if let Some(i) = t.attr_index("interpType") {
            let v = g.node(id).unwrap().data().int(i);
            if v != 1 {
                let _ = writeln!(body, "\tsetAttr \".int\" {v};");
            }
        }
        if matches!(t.name.as_str(), "aimConstraint" | "tangentConstraint" | "normalConstraint") {
            let n = g.node(id).unwrap();
            for (attr, short) in [("aimVector", "a"), ("upVector", "u"), ("worldUpVector", "wu")] {
                if let Some(i) = t.attr_index(attr) {
                    let _ = writeln!(body, "\tsetAttr \".{short}\" -type \"double3\" {} ;", vec_text(n.data().vec3(i)));
                }
            }
            if let Some(i) = t.attr_index("worldUpType") {
                let _ = writeln!(body, "\tsetAttr \".wut\" {};", n.data().int(i));
            }
        }
        conns.push((format!("{path}.pim"), format!("{c}.cpim")));
        if kind != 3 {
            conns.push((format!("{path}.ro"), format!("{c}.cro")));
        }
        if matches!(kind, 0 | 1 | 4) {
            conns.push((format!("{path}.rp"), format!("{c}.crp")));
            conns.push((format!("{path}.rpt"), format!("{c}.crt")));
        }
        let outs: &[(&str, &str)] = match kind {
            0 => &[("ct", "t"), ("cr", "r")],
            1 => &[("ct", "t")],
            3 => &[("cs", "s")],
            _ => &[("cr", "r")],
        };
        // Blended with animation, the outputs feed a pairBlend, and it drives the object.
        let mut blended = false;
        for (attr, short) in [("constraintTranslate", "ct"), ("constraintRotate", "cr")] {
            let Some(at) = t.attr_index(attr) else { continue };
            for k in 0..3 {
                let axis = ["x", "y", "z"][k];
                let mut out = g.destinations(&PlugRef::component(id, at as i32, k as i32));
                for d in g.destinations(&PlugRef::new(id, at as i32)) {
                    out.push(PlugRef::component(d.node, d.attr, k as i32));
                }
                for d in out {
                    if let Some(n) = g.node(d.node).filter(|n| n.ty().name == "pairBlend") {
                        let child = &n.ty().attrs[d.attr as usize].child_short[d.component.max(0) as usize];
                        conns.push((format!("{c}.{short}{axis}"), format!("{}.{child}", g.path(d.node))));
                        blended = true;
                    }
                }
            }
        }
        if !blended {
            for (o, a) in outs {
                let Some(at) = t.attr_index(match *o {
                    "ct" => "constraintTranslate",
                    "cr" => "constraintRotate",
                    _ => "constraintScale",
                }) else { continue };
                // Only the channels it drives: a rig can hold some out.
                let whole = !g.destinations(&PlugRef::new(id, at as i32)).is_empty();
                for (k, axis) in ["x", "y", "z"].into_iter().enumerate() {
                    if whole || !g.destinations(&PlugRef::component(id, at as i32, k as i32)).is_empty() {
                        conns.push((format!("{c}.{o}{axis}"), format!("{path}.{a}{axis}")));
                    }
                }
            }
        }
        if let Some(src) = t.attr_index("worldUpMatrix").and_then(|a| g.source(&PlugRef::new(id, a as i32))) {
            conns.push((format!("{}.wm", g.path(src.node)), format!("{c}.wum")));
        }
        {
            let n = g.node(id).unwrap();
            // In the file's units, as the reader takes them: restRotate in degrees, restTranslate in its length unit.
            for (attr, short, unit) in [("restTranslate", "rst", units::Unit::Linear), ("restRotate", "rsrr", units::Unit::Angle)] {
                if let Some(i) = t.attr_index(attr) {
                    let _ = writeln!(body, "\tsetAttr \".{short}\" -type \"double3\" {} ;", vec_text(file_vec(unit, n.data().vec3(i))));
                }
            }
            if let Some(i) = t.attr_index("enableRestPosition") {
                let _ = writeln!(body, "\tsetAttr \".erp\" {};", if n.bool(i) { "yes" } else { "no" });
            }
        }
        self.pending.push(body);
        for (a, b) in conns {
            self.conn(&a, &b);
        }
    }

    fn write_ik(&mut self, handle: NodeId) {
        let g = &self.s.graph;
        let t = g.node(handle).unwrap().type_arc().clone();
        let _ = t;
        let wm = PlugRef::new(handle, Transform::WORLD_MATRIX as i32);
        let Some(sv) = g.destinations(&wm).into_iter().map(|d| d.node).find(|&d| g.node(d).is_some_and(|n| n.ty().name == "ikSolve")) else { return };
        let st = g.node(sv).unwrap().type_arc().clone();
        let mut chain = crate::ik::joints_of(g, sv);
        let Some(end) = crate::ik::end_of(g, handle) else { return };
        // The solver drives the chain above the end joint; the end joint belongs to it all the same.
        if chain.last() != Some(&end) {
            chain.push(end);
        }
        // A one bone chain whose joint a constraint took over from the solver still starts at that joint.
        if chain.len() == 1 && g.parent(end) != NO_NODE {
            chain.insert(0, g.parent(end));
        }
        let Some(&start) = chain.first() else { return };
        if chain.iter().any(|j| !self.keep.contains(j)) {
            let name = g.path(handle);
            self.warnings.push(format!("{name}: its joints are not exported; effector and solver omitted"));
            return;
        }
        let data = rig::json_of(g.node(sv).unwrap().data(), st.attr_index("chain").unwrap());
        let _ = data;
        let solver = st.attr_index("solverType").map_or(0, |a| g.node(sv).unwrap().data().int(a) as usize);
        let solver = crate::ik::SOLVER_TYPES.get(solver).copied().unwrap_or("ikRPsolver").to_string();
        let spline_curve = st.attr_index("inCurve").and_then(|a| g.source(&PlugRef::new(sv, a as i32))).map(|c| g.path(c.node));
        let parent = g.parent(end);
        if parent == NO_NODE {
            return;
        }
        let eff_name = crate::ik::effector_of(&self.s.graph, handle).and_then(|e| self.s.graph.node(e).map(|n| n.name().to_string())).unwrap_or_else(|| self.fresh("effector1"));
        let g = &self.s.graph;
        let hp = g.path(handle);
        let (sp, ep, pp) = (g.path(start), g.path(end), g.path(parent));
        let eff_path = format!("{pp}|{eff_name}");
        let end_t = g.node(end).unwrap().data().vec3(Transform::TRANSLATE);
        let mut body = String::new();
        let _ = writeln!(body, "createNode ikEffector -n {} -p {};", quoted(&eff_name), quoted(&pp));
        let _ = writeln!(body, "\tsetAttr \".v\" no;");
        let _ = writeln!(body, "\tsetAttr \".t\" -type \"double3\" {} ;", vec_text(file_vec(units::Unit::Linear, end_t)));
        let _ = writeln!(body, "\tsetAttr \".hd\" yes;");
        self.pending.push(body);
        if self.solvers.insert(solver.clone()) {
            self.pending.push(format!("createNode {solver} -n {};\n", quoted(&solver)));
            self.shading.push(format!("connectAttr {} \":ikSystem.sol\" -na;", quoted(&format!("{solver}.msg"))));
        }
        self.conn(&format!("{sp}.msg"), &format!("{hp}.hsj"));
        self.conn(&format!("{eff_path}.hp"), &format!("{hp}.hee"));
        self.conn(&format!("{solver}.msg"), &format!("{hp}.hsv"));
        if let Some(curve) = spline_curve {
            self.conn(&format!("{curve}.ws"), &format!("{hp}.ic"));
        }
        for axis in ["x", "y", "z"] {
            self.conn(&format!("{ep}.t{axis}"), &format!("{eff_path}.t{axis}"));
        }
        self.conn(&format!("{ep}.ro"), &format!("{eff_path}.ro"));
        self.conn(&format!("{ep}.jo"), &format!("{eff_path}.jo"));
        self.conn(&format!("{pp}.s"), &format!("{eff_path}.is"));
    }

    fn write_pose(&mut self, id: NodeId) {
        let g = &self.s.graph;
        let n = g.node(id).unwrap();
        let t = n.type_arc().clone();
        let name = n.name().to_string();
        let wm = t.attr_index("worldMatrix").unwrap() as i32;
        let data = rig::json_of(n.data(), t.attr_index("poseData").unwrap());
        let bind = n.bool(t.attr_index("bindPose").unwrap());
        let mut members: Vec<(i32, NodeId)> = g.connections().into_iter().filter(|(d, _)| d.node == id && d.attr == wm && d.element >= 0).map(|(d, s)| (d.element, s.node)).collect();
        members.sort();
        if members.is_empty() || members.iter().any(|(_, j)| !self.keep.contains(j)) {
            return;
        }
        let mut body = String::new();
        let _ = writeln!(body, "createNode dagPose -n {};", quoted(&name));
        let _ = writeln!(body, "\tsetAttr -s {} \".wm\";", members.len());
        let _ = writeln!(body, "\tsetAttr -s {} \".xm\";", members.len());
        let v = |e: &Value, k: &str, d: f64| {
            let x = e.get(k).and_then(Value::as_array);
            let f = |i: usize| x.and_then(|x| x.get(i)).and_then(Value::as_f64).unwrap_or(d);
            DVec3::new(f(0), f(1), f(2))
        };
        for &(e, j) in &members {
            let item = data.get(e as usize).cloned().unwrap_or(Value::Null);
            let (tr, r, s) = (v(&item, "t", 0.0), v(&item, "r", 0.0), v(&item, "s", 1.0));
            let jn = g.node(j).unwrap();
            // A pose may hold transforms as well as joints (dagPose -save -selection): only a joint has an orient.
            let jo = if jn.ty().is_a("joint") { DQuat::from_mat4(&euler_to_matrix(jn.data().vec3(Joint::JOINT_ORIENT), RotateOrder::Xyz)) } else { DQuat::IDENTITY };
            let ro = jn.data().int(Transform::ROTATE_ORDER);
            let rad = r;
            let _ = writeln!(
                body,
                "\tsetAttr \".xm[{e}]\" -type \"matrix\" \"xform\" {} {} {ro} {} 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 1 {} {} {} {} 1 1 1 yes;",
                vec_text(s),
                vec_text(rad),
                vec_text(tr),
                jo.x,
                jo.y,
                jo.z,
                jo.w,
            );
        }
        let _ = writeln!(body, "\tsetAttr -s {} \".m\";", members.len());
        let _ = writeln!(body, "\tsetAttr -s {} \".p\";", members.len());
        let _ = writeln!(body, "\tsetAttr \".bp\" {};", if bind { "yes" } else { "no" });
        let members_at: HashMap<NodeId, i32> = members.iter().map(|&(e, j)| (j, e)).collect();
        let mut lines = Vec::new();
        for &(e, j) in &members {
            let jp = g.path(j);
            lines.push((format!("{jp}.msg"), format!("{name}.m[{e}]")));
            lines.push((format!("{jp}.bps"), format!("{name}.wm[{e}]")));
            match members_at.get(&g.parent(j)) {
                Some(p) => lines.push((format!("{name}.m[{p}]"), format!("{name}.p[{e}]"))),
                None => lines.push((format!("{name}.w"), format!("{name}.p[{e}]"))),
            }
        }
        // The skinCluster on these joints points back at its bind pose.
        if bind {
            let skins: Vec<NodeId> = g.nodes().into_iter().filter(|&s| g.node(s).is_some_and(|n| n.ty().name == skin::SKIN_TYPE) && self.keep.contains(&s) && skin::influences(g, s).iter().any(|(_, j)| members_at.contains_key(j))).collect();
            for sk in skins {
                lines.push((format!("{name}.msg"), format!("{}.bp", g.path(sk))));
            }
        }
        self.pending.push(body);
        for (a, b) in lines {
            self.conn(&a, &b);
        }
    }

    /// The display and render layer managers and their default layers, as every file from the reference has them.
    fn write_scene_nodes(&mut self) {
        let g = &self.s.graph;
        let layers: Vec<String> = crate::display_layers::all(g).into_iter().filter(|l| self.keep.contains(l)).map(|l| g.path(l)).collect();
        let n = layers.len();
        let _ = writeln!(self.out, "createNode lightLinker -s -n \"lightLinker1\";");
        let _ = writeln!(self.out, "createNode displayLayerManager -n \"layerManager\";");
        if n > 0 {
            let ids: Vec<String> = (1..=n).map(|k| k.to_string()).collect();
            let range = if n == 1 { "1".to_string() } else { format!("1:{n}") };
            let _ = writeln!(self.out, "\tsetAttr -s {} \".dli[{range}]\"  {};", n + 1, ids.join(" "));
            let _ = writeln!(self.out, "\tsetAttr -s {} \".dli\";", n + 1);
        }
        let _ = writeln!(self.out, "createNode displayLayer -n \"defaultLayer\";\n\tsetAttr \".ufem\" -type \"stringArray\" 0  ;");
        let _ = writeln!(self.out, "createNode renderLayerManager -n \"renderLayerManager\";");
        let _ = writeln!(self.out, "createNode renderLayer -n \"defaultRenderLayer\";\n\tsetAttr \".g\" yes;");
        self.conn("layerManager.dli[0]", "defaultLayer.id");
        self.conn("renderLayerManager.rlmi[0]", "defaultRenderLayer.rlid");
        for (k, l) in layers.iter().enumerate() {
            self.conn(&format!("layerManager.dli[{}]", k + 1), &format!("{l}.id"));
        }
        self.shading.push("connectAttr \"defaultRenderLayer.msg\" \":defaultRenderingList1.r\" -na;".into());
    }

    /// The default nodes, `select -ne :name` and what differs from a new scene's, in the reference's order: those its
    /// files always name, and any other that something changed.
    fn write_default_nodes(&mut self) {
        let into = |w: &Self, plug: &str| w.shading.iter().chain(w.conns.iter()).filter(|l| l.contains(plug)).count();
        let frame = anim::frame(&self.s.graph);
        for &(name, _) in crate::defaults::DEFAULT_NODES {
            // Written with the lights.
            if matches!(name, "lightList1" | "defaultLightSet") {
                continue;
            }
            let Ok(Some(id)) = self.s.graph.find(name) else { continue };
            let mut lines = String::new();
            let count = |lines: &mut String, attr: &str, n: usize| {
                if n > 1 {
                    let _ = writeln!(lines, "\tsetAttr -s {n} \".{attr}\";");
                }
            };
            match name {
                "time1" => {
                    let _ = writeln!(lines, "\tsetAttr \".o\" {frame};\n\tsetAttr \".unw\" {frame};");
                }
                "renderPartition" => count(&mut lines, "st", 2 + into(self, ":renderPartition.st")),
                "defaultShaderList1" => count(&mut lines, "s", 6 + into(self, ":defaultShaderList1.s")),
                "postProcessList1" => count(&mut lines, "p", 2),
                "defaultRenderUtilityList1" => count(&mut lines, "u", into(self, ":defaultRenderUtilityList1.u")),
                "defaultTextureList1" => count(&mut lines, "tx", into(self, ":defaultTextureList1.tx")),
                "initialShadingGroup" => {
                    let n = into(self, ":initialShadingGroup.dsm");
                    if n > 0 {
                        let _ = writeln!(lines, "\tsetAttr -s {n} \".dsm\";");
                    }
                    let _ = writeln!(lines, "\tsetAttr \".ro\" yes;");
                }
                "initialParticleSE" => {
                    let _ = writeln!(lines, "\tsetAttr \".ro\" yes;");
                }
                _ => {}
            }
            let n = self.s.graph.node(id).unwrap();
            for a in n.ty().attrs.iter().filter(|a| a.has(flags::USER) && !a.has(flags::EXTENSION)) {
                lines.push_str(&attr_add_lines(a));
            }
            let t = n.ty().clone();
            if !t.is_a(crate::shading::SG_TYPE) && t.name != "materialInfo" && name != "time1" {
                lines.push_str(&self.plain_attr_lines(id));
            }
            if let Some(kept) = self.s.graph.default_lines.get(name) {
                for (attr, text) in kept {
                    let _ = writeln!(lines, "\tsetAttr \".{attr}\" {text};");
                }
            }
            if !lines.is_empty() || crate::defaults::ALWAYS_WRITTEN.contains(&name) {
                let _ = write!(self.out, "select -ne :{name};\n{lines}");
            }
        }
    }

    fn write_layer(&mut self, id: NodeId) {
        use crate::display_layers as dl;
        let g = &self.s.graph;
        let n = g.node(id).unwrap();
        let name = n.name().to_string();
        let members: Vec<NodeId> = dl::members(g, id).into_iter().filter(|m| self.keep.contains(m)).collect();
        let mut body = String::new();
        let _ = writeln!(body, "createNode displayLayer -n {};", quoted(&name));
        if !n.bool(dl::VISIBILITY) {
            let _ = writeln!(body, "\tsetAttr \".v\" no;");
        }
        let dt = n.data().int(dl::DISPLAY_TYPE);
        if dt != 0 {
            let _ = writeln!(body, "\tsetAttr \".dt\" {dt};");
        }
        let _ = writeln!(body, "\tsetAttr \".ufem\" -type \"stringArray\" 0  ;");
        let _ = writeln!(body, "\tsetAttr \".do\" {};", n.data().int(dl::DISPLAY_ORDER));
        let lod = n.data().int(dl::LEVEL_OF_DETAIL);
        if lod != 0 {
            let _ = writeln!(body, "\tsetAttr \".lod\" {lod};");
        }
        for (name, attr, default) in [("s", dl::SHADING, true), ("t", dl::TEXTURING, true), ("p", dl::PLAYBACK, true), ("e", dl::ENABLED, true), ("hpb", dl::HIDE_ON_PLAYBACK, false), ("ovrgbf", dl::OVERRIDE_RGB_COLORS, false)] {
            if n.bool(attr) != default {
                let _ = writeln!(body, "\tsetAttr \".{name}\" {};", if default { "no" } else { "yes" });
            }
        }
        let color = n.data().int(dl::COLOR);
        if color != 0 {
            let _ = writeln!(body, "\tsetAttr \".c\" {color};");
        }
        let rgb = n.vec3(dl::OVERRIDE_COLOR_RGB);
        if rgb != DVec3::ZERO {
            let _ = writeln!(body, "\tsetAttr \".ovrgb\" -type \"float3\" {} ;", vec_text(rgb));
        }
        let lines: Vec<String> = members.iter().map(|&m| g.path(m)).collect();
        self.pending.push(body);
        for m in lines {
            self.conn(&format!("{name}.di"), &format!("{m}.do"));
        }
    }

    /// An object set as the reference writes one: whole objects through their instObjGroups into `dagSetMembers`, other
    /// nodes and sets through their message into `dnSetMembers`. A quick select set is annotated gCharacterSet. Members
    /// that are components are left out, with a warning.
    fn write_object_set(&mut self, id: NodeId) {
        use crate::object_sets as os;
        let g = &self.s.graph;
        let n = g.node(id).unwrap();
        let name = n.name().to_string();
        let quick = n.bool(os::QUICK_SELECT);
        let mut body = String::new();
        let _ = writeln!(body, "createNode objectSet -n {};", quoted(&name));
        let _ = writeln!(body, "\tsetAttr \".ihi\" 0;");
        if quick {
            let _ = writeln!(body, "\tsetAttr \".an\" -type \"string\" \"gCharacterSet\";");
        }
        let (mut dag, mut dg, mut parts) = (Vec::new(), Vec::new(), 0usize);
        for m in os::members(g, id) {
            if !self.keep.contains(&m.node) || g.node(m.node).is_none() {
                continue;
            }
            if m.kind != crate::components::ComponentMode::Object {
                parts += 1;
                continue;
            }
            if g.node(m.node).is_some_and(|x| x.ty().dag) {
                dag.push(g.path(m.node));
            } else {
                dg.push(g.path(m.node));
            }
        }
        self.pending.push(body);
        for (k, m) in dag.iter().enumerate() {
            self.conn(&format!("{m}.iog"), &format!("{name}.dsm[{k}]"));
        }
        for (k, m) in dg.iter().enumerate() {
            self.conn(&format!("{m}.msg"), &format!("{name}.dnsm[{k}]"));
        }
        if parts > 0 {
            self.warnings.push(format!("{name}: {parts} component member(s) not written"));
        }
    }

    /// A utility node or expression: its type, name and every stored value that isn't the default or driven.
    fn write_plain(&mut self, id: NodeId) {
        if self.s.graph.node(id).is_some_and(|n| n.ty().name == "heldNode") {
            return self.write_held(id);
        }
        if self.s.graph.node(id).is_some_and(|n| n.ty().name == crate::core_nodes::UNKNOWN) {
            return self.write_unknown(id);
        }
        let g = &self.s.graph;
        let n = g.node(id).unwrap();
        let mut text = format!("createNode {} -n {};\n", n.ty().name, quoted(&g.path(id)));
        if n.is_locked() {
            text.push_str("\tlockNode -l 1 ;\n");
        }
        // Added attributes come first: the values and connections below can name them.
        for a in n.ty().attrs.iter().filter(|a| a.has(flags::USER) && !a.has(flags::EXTENSION)) {
            text.push_str(&attr_add_lines(a));
        }
        text.push_str(&self.plain_attr_lines(id));
        // The reference's HIKState2SK reads each joint it drives.
        if n.ty().name == "HIKState2SK" {
            let k = id;
            let path = g.path(k);
            let mut lines: Vec<String> = Vec::new();
            for (i, slot) in crate::hik_slots::slot_names().iter().enumerate() {
                let Some(d) = g.destinations(&PlugRef::component(k, crate::hik_nodes::k_slot(i) as i32 + 1, 0)).first().copied() else { continue };
                let j = g.path(d.node);
                for (from, to) in [("pm", "PGX"), ("jo", "PreR"), ("ssc", "SC"), ("is", "IS"), ("ro", "ROrder"), ("ra", "PostR")] {
                    lines.push(format!("connectAttr {} {};", quoted(&format!("{j}.{from}")), quoted(&format!("{path}.{slot}{to}"))));
                }
            }
            self.conns.extend(lines);
        }
        // A layer is a set of the plugs its blends drive, and counts them as the reference does.
        if n.ty().name == "animLayer" {
            let blends = crate::anim_layers::layer_blends(g, id);
            let members: Vec<PlugRef> = blends.iter().flat_map(|&b| crate::anim_layers::blend_plugs(g, b)).filter(|p| self.keep.contains(&p.node)).collect();
            if !members.is_empty() {
                let _ = writeln!(text, "\tsetAttr -s {} \".dsm\";", members.len());
                let _ = writeln!(text, "\tsetAttr -s {} \".bnds\";", blends.len());
            }
            let layer = g.path(id);
            let lines: Vec<String> = members.iter().map(|p| format!("connectAttr {} {} -na;", quoted(&format!("{}.{}", g.path(p.node), ref_name(g.attr(p), p.component))), quoted(&format!("{layer}.dsm")))).collect();
            self.conns.extend(lines);
        }
        self.out.push_str(&text);
    }

    /// A node's stored attributes that differ from their defaults, as setAttr lines.
    fn plain_attr_lines(&self, id: NodeId) -> String {
        self.attr_lines(id, |_, _| true)
    }

    /// The same for the attributes `wanted` allows, given each attribute's index and definition.
    fn attr_lines(&self, id: NodeId, wanted: impl Fn(usize, &AttrDef) -> bool) -> String {
        let g = &self.s.graph;
        let n = g.node(id).unwrap();
        let t = n.type_arc().clone();
        let mut text = String::new();
        let line = |p: &PlugRef, v: &PlugValue, out: &mut String| {
            let name = if shading::writes_short(g, p.node) { shading::short_plug(g, p) } else { g.plug_name(p) };
            let attr = name.split_once('.').map_or("", |x| x.1);
            match v {
                PlugValue::Vec3(x) => {
                    let def = g.attr(p);
                    if !def.child_names[0].is_empty() && def.child_names[2].is_empty() {
                        let _ = writeln!(out, "\tsetAttr \".{attr}\" -type \"float2\" {} {} ;", fmt_num(x.x), fmt_num(x.y));
                    } else {
                        let ty = if def.has(flags::COLOR) { "float3" } else { "double3" };
                        let _ = writeln!(out, "\tsetAttr \".{attr}\" -type \"{ty}\" {} ;", vec_text(file_vec(def.unit, *x)));
                    }
                }
                PlugValue::Float(x) => {
                    let x = units::to_user(g.attr(p).unit, *x);
                    let _ = writeln!(out, "\tsetAttr \".{attr}\" {x};");
                }
                PlugValue::Int(x) => {
                    let _ = writeln!(out, "\tsetAttr \".{attr}\" {x};");
                }
                PlugValue::Bool(x) => {
                    let _ = writeln!(out, "\tsetAttr \".{attr}\" {};", if *x { "yes" } else { "no" });
                }
                PlugValue::String(x) => {
                    let _ = writeln!(out, "\tsetAttr \".{attr}\" -type \"string\" {};", quoted(x));
                }
                PlugValue::Matrix(m) => {
                    let _ = writeln!(out, "\tsetAttr \".{attr}\" -type \"matrix\" {};", matrix_text(m));
                }
                v @ PlugValue::Data(_) => {
                    if let Some(d) = crate::array_data::of(v) {
                        let _ = writeln!(out, "\tsetAttr \".{attr}\" {};", crate::array_data::file_text(d, fmt_num, quoted));
                    } else if let Some((kind, text)) = crate::scripted::plugin_data_text(v) {
                        let _ = writeln!(out, "\tsetAttr \".{attr}\" -type {} {text};", quoted(&kind));
                    }
                }
                _ => {}
            }
        };
        for (i, a) in t.attrs.iter().enumerate() {
            if !a.has(flags::INPUT) || !a.has(flags::STORABLE) || !wanted(i, a) {
                continue;
            }
            // A HumanIK node's vectors go by axis: the reference's files name .HipsTx and never .HipsT.
            if t.name.starts_with("HIK") && a.ty == PlugType::Vec3 {
                let (value, rest) = (n.value(i).as_vec3(), a.default_value.as_vec3());
                for c in 0..3 {
                    if value[c] != rest[c] {
                        let _ = writeln!(text, "\tsetAttr \".{}\" {};", a.child_short[c], fmt_num(value[c]));
                    }
                }
                continue;
            }
            if a.is_array() {
                for (k, v) in n.elements(i) {
                    let p = PlugRef::element(id, i as i32, *k);
                    if g.source(&p).is_none() {
                        line(&p, v, &mut text);
                    }
                }
            } else {
                let p = PlugRef::new(id, i as i32);
                if g.source(&p).is_none() && n.value(i) != &a.default_value {
                    line(&p, n.value(i), &mut text);
                }
            }
        }
        text
    }

    fn write_connections(&mut self) {
        let list = self.s.graph.connections();
        for (dst, src) in list {
            let g = &self.s.graph;
            let (Some(dn), Some(sn)) = (g.node(dst.node), g.node(src.node)) else { continue };
            // A plug-in deformer's world matrix is linked here; the reference hands it over without a connection.
            if crate::scripted::linked_geom_attr(g, dst.node) == Some(dst.attr as usize) {
                continue;
            }
            // Keys on a joint an IK solver drives live on the solver's FK rotate here; in the file they go on the
            // joint, as the reference keeps them.
            if dn.ty().name == "ikSolve" {
                if let Some(joint) = crate::ik::joint_plug_for_fk(g, &dst) {
                    if self.keep.contains(&joint.node) && self.keep.contains(&src.node) && anim::is_curve(g, src.node) {
                        let (from, to) = (format!("{}.o", g.path(src.node)), g.plug_name(&joint));
                        self.conn(&from, &to);
                    }
                }
                continue;
            }
            if self.history_written.contains(&dst.node) || self.history_written.contains(&src.node) {
                self.history_connection(src, dst);
                continue;
            }
            if !self.keep.contains(&dst.node) || !self.keep.contains(&src.node) {
                continue;
            }
            // A character and its clips: written by write_character, with a member's keys going to the character's value.
            let clip_kind = |ty: &str| matches!(ty, "character" | "clipLibrary" | "clipScheduler" | "clipChannel");
            if clip_kind(&dn.ty().name) || clip_kind(&sn.ty().name) {
                if let Some(target) = self.char_channels.get(&dst.node).cloned() {
                    if anim::is_curve(g, src.node) && crate::characters::is_channel_base(g, &dst) {
                        let from = format!("{}.o", g.path(src.node));
                        self.conn(&from, &target);
                    }
                }
                continue;
            }
            if let Some(target) = self.char_driven.get(&dst).cloned() {
                if anim::is_curve(g, src.node) {
                    let from = format!("{}.o", g.path(src.node));
                    self.conn(&from, &target);
                    continue;
                }
            }
            // A plug-in's deformer into a curve: written with the curve, in the reference's form.
            if crate::scripted::is_plugin_deformer(&sn.ty().name) && sn.ty().attr_index("outputCurve") == Some(src.attr as usize) && dn.ty().name == crate::curves::SHAPE_TYPE {
                continue;
            }
            // A plug-in's deformer is written with a mesh's chain; on a curve its other connections are plain ones.
            let chained = |ty: &str, id: NodeId| crate::deform::is_deformer(ty) || self.chain_written.contains(&id);
            let deforms = chained(&dn.ty().name, dst.node) || chained(&sn.ty().name, src.node);
            if deforms {
                self.deformer_connection(src, dst);
                continue;
            }
            if self.shading_connection(src, dst) {
                continue;
            }
            let g = &self.s.graph;
            let (dt, st) = (g.node(dst.node).unwrap().ty(), g.node(src.node).unwrap().ty());
            // What drives a constraint's target weight: its short name is w and the target's index.
            if rig::CONSTRAINT_TYPES.contains(&dt.name.as_str()) {
                let attr = &dt.attrs[dst.attr as usize];
                let index = attr.has(flags::USER).then(|| attr.name.rsplit('W').next().and_then(|x| x.parse::<usize>().ok())).flatten();
                if let Some(e) = index {
                    let from = if st.attr_index("keys").is_some() { "o".to_string() } else { g.plug_name(&src).rsplit('.').next().unwrap_or("").to_string() };
                    let (a, b) = (g.path(src.node), g.path(dst.node));
                    self.conn(&format!("{a}.{from}"), &format!("{b}.w{e}"));
                }
                continue;
            }
            // Written by write_constraint, which knows the reference's short names.
            if rig::CONSTRAINT_TYPES.contains(&st.name.as_str()) && dt.name == "pairBlend" {
                continue;
            }
            // A curve fed straight by another curve.
            if dt.name == crate::curves::SHAPE_TYPE && st.name == crate::curves::SHAPE_TYPE && dst.attr as usize == crate::curves::CurveShape::CREATE {
                let (a, b) = (g.path(src.node), g.path(dst.node));
                self.conn(&format!("{a}.ws"), &format!("{b}.cr"));
                continue;
            }
            // A CV something drives.
            if dt.name == crate::curves::SHAPE_TYPE && dst.attr as usize == crate::curves::CurveShape::CONTROL_POINTS {
                let (a, b) = (g.plug_name(&src), g.path(dst.node));
                self.conn(&a, &format!("{b}.cp[{}]", dst.element));
                continue;
            }
            // Layers and their blends, with the short names the reference's files use.
            let layered = |n: &str| n == "animLayer" || n.starts_with("animBlendNode") || n.starts_with("HIK");
            if layered(&dt.name) || layered(&st.name) {
                let name = |p: &PlugRef, t: &NodeType| {
                    if layered(&t.name) || anim::is_curve(g, p.node) {
                        shading::short_plug(g, p)
                    } else {
                        let a = &t.attrs[p.attr as usize];
                        format!("{}.{}", g.path(p.node), ref_name(a, p.component))
                    }
                };
                let (a, b) = (name(&src, st), name(&dst, dt));
                self.conn(&a, &b);
                continue;
            }
            let plain = |t: &NodeType| is_plain_node(&t.name) || t.name == "unitConversion";
            if plain(dt) || plain(st) {
                // By full attribute name. The reference's matrix outputs are arrays: worldMatrix[0].
                let name = |p: &PlugRef| {
                    // A plug-in's node by its short names, as the reference writes them.
                    if g.node(p.node).is_some_and(|n| !n.ty().owner.is_empty()) {
                        return shading::short_plug(g, p);
                    }
                    let text = g.plug_name(p);
                    let one_of = matches!(g.attr(p).name.as_str(), "worldMatrix" | "worldInverseMatrix" | "parentMatrix" | "parentInverseMatrix");
                    if one_of && p.element < 0 {
                        format!("{text}[0]")
                    } else {
                        text
                    }
                };
                let (a, b) = (name(&src), name(&dst));
                self.conn(&a, &b);
                continue;
            }
            let curve = anim::is_curve(g, src.node) || DRIVEN_CURVES.contains(&st.name.as_str());
            let driven_curve = DRIVEN_CURVES.contains(&dt.name.as_str());
            if !(curve || self.dag_written(src.node)) || !(driven_curve || self.dag_written(dst.node)) {
                continue;
            }
            let plug_of = |t: &NodeType, p: &PlugRef| -> Option<String> {
                let a = &t.attrs[p.attr as usize];
                (channels_of(t).contains(&a.name.as_str()) || a.has(flags::USER) || a.has(flags::MESSAGE) || MATRIX_OUTPUTS.contains(&a.name.as_str())).then(|| {
                    let name = ref_name(a, p.component);
                    if p.element >= 0 { format!("{name}[{}]", p.element) } else { name }
                })
            };
            let from = if curve {
                (src.attr as usize == st.attr_index("output").unwrap_or(usize::MAX)).then(|| "o".to_string())
            } else {
                plug_of(st, &src)
            };
            let to = if driven_curve {
                (dst.attr as usize == dt.attr_index("input").unwrap_or(usize::MAX)).then(|| "i".to_string())
            } else {
                plug_of(dt, &dst)
            };
            let (Some(from), Some(to)) = (from, to) else { continue };
            let (a, b) = (g.path(src.node), g.path(dst.node));
            self.conn(&format!("{a}.{from}"), &format!("{b}.{to}"));
        }
    }
}

/// addAttr lines for a user attribute.
fn attr_add_lines(a: &AttrDef) -> String {
    let short = if a.short_name.is_empty() { a.name.as_str() } else { a.short_name.as_str() };
    let mut k = if a.has(flags::KEYABLE) { " -k true".to_string() } else { String::new() };
    for c in crate::commands::categories(a) {
        let _ = write!(k, " -ct {}", quoted(&c));
    }
    if a.has(flags::WORLD_SPACE) {
        k.push_str(" -ws true");
    }
    if a.has(flags::NAME_NOT_UNIQUE) {
        k.push_str(" -eun false");
    }
    let range = |o: &mut String| {
        if let Some(m) = a.min_value {
            let _ = write!(o, " -min {m}");
        }
        if let Some(m) = a.max_value {
            let _ = write!(o, " -max {m}");
        }
    };
    let mut o = String::new();
    let m = if a.is_array() { " -m" } else { "" };
    if a.has(flags::MESSAGE) {
        let _ = writeln!(o, "\taddAttr -s false -ci true{m} -sn {} -ln {} -at \"message\";", quoted(short), quoted(&a.name));
        return o;
    }
    match a.ty {
        PlugType::Float | PlugType::Int | PlugType::Bool => {
            let at = match a.ty {
                PlugType::Float => "double",
                PlugType::Int => "long",
                _ => "bool",
            };
            let _ = write!(o, "\taddAttr -ci true{k}{m} -sn {} -ln {}", quoted(short), quoted(&a.name));
            let dv = a.default_value.scalar().unwrap_or(0.0);
            if dv != 0.0 {
                let _ = write!(o, " -dv {dv}");
            }
            range(&mut o);
            let _ = writeln!(o, " -at {};", quoted(at));
        }
        PlugType::Enum => {
            let _ = writeln!(o, "\taddAttr -ci true{k} -sn {} -ln {} -min 0 -max {} -en {} -at \"enum\";", quoted(short), quoted(&a.name), a.enum_names.len().saturating_sub(1), quoted(&a.enum_names.join(":")));
        }
        PlugType::String => {
            let _ = writeln!(o, "\taddAttr -ci true{k} -sn {} -ln {} -dt \"string\";", quoted(short), quoted(&a.name));
        }
        PlugType::Vec3 => {
            let _ = writeln!(o, "\taddAttr -ci true{k} -sn {} -ln {} -at \"double3\" -nc 3;", quoted(short), quoted(&a.name));
            for c in 0..3 {
                let child_short = if a.child_short[c].is_empty() { &a.child_names[c] } else { &a.child_short[c] };
                let _ = writeln!(o, "\taddAttr -ci true -sn {} -ln {} -at \"double\" -p {};", quoted(child_short), quoted(&a.child_names[c]), quoted(&a.name));
            }
        }
        _ => {}
    }
    o
}

/// Contiguous runs of influence indices in a point's sorted weights.
fn weight_runs(w: &[(u32, f64)]) -> Vec<&[(u32, f64)]> {
    let mut out = Vec::new();
    let mut start = 0;
    for i in 1..=w.len() {
        if i == w.len() || w[i].0 != w[i - 1].0 + 1 {
            if start < i {
                out.push(&w[start..i]);
            }
            start = i;
        }
    }
    out
}

/// What export selected takes: the nodes, what lies below them, their parents, and the skeletons their skins use.
fn closure(g: &Graph, roots: &[NodeId]) -> HashSet<NodeId> {
    let mut keep: HashSet<NodeId> = HashSet::new();
    let add_dag = |g: &Graph, keep: &mut HashSet<NodeId>, id: NodeId| {
        keep.insert(id);
        keep.extend(g.descendants(id));
        let mut p = g.parent(id);
        while p != NO_NODE {
            keep.insert(p);
            p = g.parent(p);
        }
    };
    for &r in roots {
        add_dag(g, &mut keep, r);
    }
    // Skins on the meshes, with their influences.
    let shapes: Vec<NodeId> = keep.iter().copied().filter(|&n| g.node(n).is_some_and(|x| x.ty().is_a("mesh"))).collect();
    for shape in shapes {
        if let Some(sk) = skin::skin_of(g, shape) {
            keep.insert(sk);
            for (_, j) in skin::influences(g, sk) {
                add_dag(g, &mut keep, j);
            }
        }
        // Other deformers, and the handles and curves that drive them.
        for h in crate::history::history_of(g, shape) {
            let Some(n) = g.node(h) else { continue };
            if !crate::deform::is_deformer(n.ty().name.as_str()) && !crate::scripted::is_plugin_deformer(n.ty().name.as_str()) {
                continue;
            }
            keep.insert(h);
            for (dst, src) in g.connections() {
                if dst.node == h && g.node(src.node).is_some_and(|x| x.ty().dag) {
                    add_dag(g, &mut keep, src.node);
                }
            }
        }
    }
    shading::keep_shading(g, &mut keep);
    // Everything else that only serves what is kept.
    for id in g.nodes() {
        let Some(n) = g.node(id) else { continue };
        let t = n.ty();
        if t.is_a("animCurve") || DRIVEN_CURVES.contains(&t.name.as_str()) {
            if anim::driven_by(g, id).iter().any(|d| keep.contains(&d.node)) {
                keep.insert(id);
            }
        } else if t.name == crate::display_layers::TYPE_NAME {
            if crate::display_layers::members(g, id).iter().any(|m| keep.contains(m)) {
                keep.insert(id);
            }
        } else if rig::CONSTRAINT_TYPES.contains(&t.name.as_str()) || t.name == "dagPose" {
            keep.insert(id);
        }
    }
    keep
}

/// The scene as .ma text. `only` limits it to some objects (export selected). Returns the text and warnings.
pub fn write_text(s: &mut Session, file_name: &str, only: Option<&[NodeId]>) -> (String, Vec<String>) {
    register_types();
    // Every scene has them; one that never needed them writes them too.
    crate::shading::make_defaults(&mut s.graph);
    crate::defaults::make(&mut s.graph);
    // A deformed mesh with edits after its deformers is written as Delete Non-Deformer History would leave it: the edits
    // go before the deformers, into the original the file holds, and the skin's weights follow the new points. Written
    // as it is, the file lost its skin (#180). The scene is put back once the file is written.
    // Only when the edits can be taken back: within whatever command is open, back to where this started.
    let undo = s.graph.undo_stack();
    let put_back = (undo.enabled() && !undo.replaying()).then(|| {
        s.graph.open_chunk("Write scene");
        let mark = s.graph.undo_stack().open_edits();
        crate::history::move_post_edits_everywhere(&mut s.graph);
        mark
    });
    s.graph.evaluate();
    // Before anything's borrowed: this reads the scene against what the files gave.
    let (ref_head, ref_tail) = if NO_REFERENCES.with(Cell::get) { (String::new(), String::new()) } else { crate::references::ma_statements(s) };
    let g = &s.graph;
    let mut keep = match only {
        // -exportSelectedStrict: what is selected and nothing else.
        Some(list) if EXPORT_STRICT.with(Cell::get) => list.iter().copied().collect(),
        Some(list) => closure(g, list),
        None => g.nodes().into_iter().collect::<HashSet<NodeId>>(),
    };
    // What a reference brought in is written as the reference.
    keep.retain(|&id| g.node(id).is_some_and(|n| n.reference_id() == 0));
    // A mesh's history is written as the geometry it makes: Freeze Transformations' move on one goes with the rest.
    keep.retain(|&id| {
        !g.node(id).is_some_and(|n| n.ty().name == "transformGeometry")
            || !g.destinations(&PlugRef::new(id, 3)).iter().any(|d| g.node(d.node).is_some_and(|n| n.ty().is_a("mesh") || crate::modifiers::is_modifier(g, d.node) || g.node(d.node).is_some_and(|n| n.ty().is_a(crate::history::BASE_TYPE))))
    });
    let used: HashSet<String> = g.nodes().iter().filter_map(|&id| g.node(id).map(|n| n.name().to_string())).collect();
    let roots = g.roots().to_vec();
    let mut w = Writer { s, out: String::new(), conns: Vec::new(), shading: Vec::new(), warnings: Vec::new(), used, keep, pending: Vec::new(), solvers: HashSet::new(), chain_written: HashSet::new(), history_written: HashSet::new(), needs_links: Vec::new(), links_text: String::new(), char_driven: HashMap::new(), char_channels: HashMap::new() };
    w.plan_characters();

    let fps = anim::fps();
    let name = std::path::Path::new(file_name).file_name().and_then(|n| n.to_str()).unwrap_or(file_name).to_string();
    let _ = writeln!(w.out, "//Maya ASCII 2024 scene");
    let _ = writeln!(w.out, "//Name: {name}");
    let _ = writeln!(w.out, "//Codeset: UTF-8");
    let _ = writeln!(w.out, "requires maya \"2024\";");
    // The reference reads the file, keeping these nodes as unknown ones from a plug-in it does not have.
    if w.s.graph.nodes().iter().any(|&id| w.keep.contains(&id) && w.s.graph.node(id).is_some_and(|n| crate::dynamics::is_dynamics_type(&n.ty().name) || n.ty().name == crate::hair::HAIR_SYSTEM)) {
        let _ = writeln!(w.out, "requires \"aztekDynamics\" \"1.0\";");
    }
    // The plug-ins whose nodes this file holds, as the reference lists them; reading loads them first.
    let mut plugins: BTreeSet<String> = BTreeSet::new();
    for &id in &w.keep {
        if let Some(p) = w.s.graph.node(id).and_then(|n| crate::scripted::plugin_of_type(&n.ty().name)) {
            plugins.insert(p);
        }
    }
    // And those pluginInfo -writeRequires names.
    plugins.extend(crate::scripted::write_requires());
    let mut unknown_plugins = unknown::plugins(w.s, &w.keep);
    unknown_plugins.retain(|p, _| !plugins.contains(p));
    for p in &plugins {
        let _ = writeln!(w.out, "requires {} {};", quoted(p), quoted(&crate::scripted::plugin_version(p)));
    }
    // The plug-ins of nodes kept as unknown, as the file they came from named them, and the unknown plug-ins the scene
    // named with no nodes kept.
    let mut unknown_plugins: BTreeMap<String, (String, Vec<String>, Vec<String>)> = unknown_plugins.into_iter().map(|(p, (v, tys))| (p, (v, tys.into_iter().collect(), Vec::new()))).collect();
    for (p, u) in &w.s.extras.unknown_plugins {
        if plugins.contains(p) {
            continue;
        }
        let e = unknown_plugins.entry(p.clone()).or_insert_with(|| (u.version.clone(), Vec::new(), Vec::new()));
        for t in &u.node_types {
            if !e.1.contains(t) {
                e.1.push(t.clone());
            }
        }
        e.2 = u.data_types.clone();
    }
    for (p, (v, tys, dts)) in unknown_plugins {
        let nt: String = tys.iter().map(|t| format!("-nodeType {} ", quoted(t))).collect();
        let dt: String = dts.iter().map(|t| format!("-dataType {} ", quoted(t))).collect();
        let _ = writeln!(w.out, "requires {nt}{dt}{} {};", quoted(&p), quoted(&v));
    }
    let _ = writeln!(w.out, "currentUnit -l {} -a {} -t {};", units::linear_full_name(), units::angle_full_name(), time_unit_name(fps));
    let _ = writeln!(w.out, "fileInfo \"application\" \"aztek\";");
    let _ = writeln!(w.out, "fileInfo \"version\" \"{}\";", env!("CARGO_PKG_VERSION"));
    for (k, v) in w.s.extras.file_info.clone() {
        if k != "application" && k != "version" {
            let _ = writeln!(w.out, "fileInfo {} {};", quoted(&k), quoted(&v));
        }
    }
    w.out.push_str(&ref_head);

    if only.is_none() {
        w.write_default_cameras();
    }
    for r in roots {
        w.write_dag(r);
    }
    w.write_scene_nodes();

    // The nodes hanging off the DAG, in creation order.
    let mut unwritten: BTreeMap<String, usize> = BTreeMap::new();
    for id in w.s.graph.nodes() {
        if !w.keep.contains(&id) {
            continue;
        }
        // Every scene has these: written on their own, below.
        if w.s.graph.node(id).is_some_and(|n| n.namespace().is_empty() && crate::defaults::is_scene_node(n.name())) {
            continue;
        }
        if w.s.graph.node(id).is_some() && !w.dag_written(id) && w.write_shading_node(id) {
            continue;
        }
        let Some(n) = w.s.graph.node(id) else { continue };
        let t = n.type_arc().clone();
        let ty = t.name.as_str();
        if w.dag_written(id) {
            if ty == "ikHandle" {
                w.write_ik(id);
            }
            continue;
        }
        if crate::characters::is_character(&w.s.graph, id) {
            w.write_character(id);
            continue;
        }
        if matches!(ty, "clipLibrary" | "clipScheduler" | "clipChannel") {
            // Written with their character.
            continue;
        }
        if t.is_a("animCurve") || DRIVEN_CURVES.contains(&ty) {
            let output = PlugRef::new(id, t.attr_index("output").unwrap() as i32);
            let to_ik_joint = |d: &PlugRef| crate::ik::joint_plug_for_fk(&w.s.graph, d).is_some_and(|j| w.keep.contains(&j.node));
            let drives = w.s.graph.destinations(&output).iter().any(|d| w.char_channels.contains_key(&d.node) || to_ik_joint(d)) || w.s.graph.destinations(&output).iter().any(|d| {
                let dt = w.s.graph.node(d.node).unwrap().ty().name.as_str();
                w.s.graph.node(d.node).unwrap().reference_id() != 0 || w.history_written.contains(&d.node) || (w.keep.contains(&d.node) && (rig::CONSTRAINT_TYPES.contains(&dt) || w.dag_written(d.node) || DRIVEN_CURVES.contains(&dt) || is_plain_node(dt) || crate::deform::is_deformer(dt)))
            });
            if drives || ANIM_EXPORT.with(Cell::get) || crate::references::held_by_unloaded(w.s, n.name()) {
                w.write_curve(id);
            } else {
                w.warnings.push(format!("{}: keys on what is not written; omitted", n.name()));
            }
        } else if let Some(kind) = rig::CONSTRAINT_TYPES.iter().position(|c| *c == ty) {
            w.write_constraint(id, kind);
        } else if (is_plain_node(ty) && !w.chain_written.contains(&id)) || ty == "unitConversion" {
            w.write_plain(id);
        } else if ty == "dagPose" {
            w.write_pose(id);
        } else if ty == crate::display_layers::TYPE_NAME {
            w.write_layer(id);
        } else if ty == crate::references::REF_NODE_TYPE {
            let _ = writeln!(w.out, "createNode reference -n {};", quoted(n.name()));
        } else if ty == skin::SKIN_TYPE || ty == "ikSolve" {
            // Written with their mesh and handle.
        } else if crate::deform::is_deformer(ty) || w.chain_written.contains(&id) {
            if !w.chain_written.contains(&id) {
                *unwritten.entry(ty.to_string()).or_default() += 1;
            }
        } else if crate::deform_sets::deformer_of_set(&w.s.graph, id).is_some() {
            // Goes with its deformer.
        } else if ty == crate::object_sets::TYPE_NAME && !matches!(n.name(), "defaultLightSet" | "defaultObjectSet") {
            w.write_object_set(id);
        } else if t.is_a("mesh") {
            // Intermediate shapes: geometry is baked.
        } else if matches!(ty, crate::nurbs_nodes::TO_POINTS | crate::nurbs_nodes::FROM_POINTS | "ikEffector" | "ikRPsolver" | "ikSCsolver" | "ikSplineSolver" | "ikSystem") || ty == crate::history::TWEAK_TYPE {
            // Written as the reference's connections and tweak nodes with their shape, and with each IK handle.
        } else if ty == crate::light_cmds::LINKER_TYPE || matches!(n.name(), "defaultLightSet" | "defaultObjectSet") || crate::cameras::is_default_camera(&w.s.graph, id) {
            // Every scene has these; the default cameras are written on their own.
        } else if !n_is_baked(&t) {
            *unwritten.entry(ty.to_string()).or_default() += 1;
        }
    }
    let pending = std::mem::take(&mut w.pending);
    for text in pending {
        w.out.push_str(&text);
    }
    for (ty, count) in unwritten {
        w.warnings.push(format!("{ty}: {count} not written; meshes are baked, other nodes omitted"));
    }

    w.write_light_links();
    w.write_render_settings();
    w.write_lights();
    w.write_connections();
    w.out.push_str(&ref_tail);

    // The reference keeps a Z up scene's axis in this script node.
    if crate::world::up_z() {
        let _ = writeln!(w.out, "createNode script -n \"upAxisScriptNode\";");
        let _ = writeln!(w.out, "\tsetAttr \".b\" -type \"string\" \"string $currentAxis = `upAxis -q -ax`; if ($currentAxis != \\\"z\\\") {{ upAxis -ax \\\"z\\\"; viewSet -home persp; }}\";");
        let _ = writeln!(w.out, "\tsetAttr \".st\" 2;");
    }

    // The reference keeps the playback range in this script node.
    let r = anim::range();
    let _ = writeln!(w.out, "createNode script -n \"sceneConfigurationScriptNode\";");
    let _ = writeln!(w.out, "\tsetAttr \".b\" -type \"string\" \"playbackOptions -min {} -max {} -ast {} -aet {} \";", r.min, r.max, r.anim_start, r.anim_end);
    let _ = writeln!(w.out, "\tsetAttr \".st\" 6;");

    w.write_default_nodes();

    let links = std::mem::take(&mut w.links_text);
    w.out.push_str(&links);
    let mut lines = std::mem::take(&mut w.conns);
    lines.sort();
    lines.dedup();
    for l in lines {
        let _ = writeln!(w.out, "{l}");
    }
    let mut lines = std::mem::take(&mut w.shading);
    lines.sort();
    for l in lines {
        let _ = writeln!(w.out, "{l}");
    }
    let _ = writeln!(w.out, "// End of {name}");
    if let Some(mark) = put_back {
        w.s.graph.rollback_to(mark);
        w.s.graph.close_chunk();
        w.s.graph.evaluate();
    }
    (w.out, w.warnings)
}

/// An animCurve's lines: its node, tangents and keys, and its infinities.
fn curve_lines(ty: &str, name: &str, keys: &Keys, pre: i64, post: i64) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "createNode {} -n {};", ty, quoted(&name));
    let _ = writeln!(out, "\tsetAttr \".tan\" 18;");
    let _ = writeln!(out, "\tsetAttr \".wgt\" {};", if keys.weighted { "yes" } else { "no" });
    let count = keys.keys.len();
    if count > 0 {
        let last = range_text(0, count - 1);
        let value_unit = if ty.ends_with('A') { units::Unit::Angle } else if ty.ends_with('L') { units::Unit::Linear } else { units::Unit::None };
        let pairs = keys.keys.iter().map(|k| format!("{} {}", k.t, units::to_user(value_unit, k.v))).collect::<Vec<_>>().join(" ");
        let _ = writeln!(out, "\tsetAttr -s {count} \".ktv[{last}]\"  {pairs};");
        let list = |f: &dyn Fn(&Key) -> String| keys.keys.iter().map(f).collect::<Vec<_>>().join(" ");
        if keys.keys.iter().any(|k| k.itt != Tangent::Auto) {
            let _ = writeln!(out, "\tsetAttr -s {count} \".kit[{last}]\"  {};", list(&|k| tangent_code(k.itt).to_string()));
        }
        if keys.keys.iter().any(|k| k.ott != Tangent::Auto) {
            let _ = writeln!(out, "\tsetAttr -s {count} \".kot[{last}]\"  {};", list(&|k| tangent_code(k.ott).to_string()));
        }
        let broken = |k: &Key| !k.tan_lock || ((k.itt == Tangent::Fixed || k.ott == Tangent::Fixed) && k.in_slope != k.out_slope);
        if keys.keys.iter().any(broken) {
            let _ = writeln!(out, "\tsetAttr -s {count} \".ktl[{last}]\"  {};", list(&|k| if broken(k) { "no".into() } else { "yes".to_string() }));
        }
        if keys.weighted && keys.keys.iter().any(|k| !k.weight_lock) {
            let _ = writeln!(out, "\tsetAttr -s {count} \".kwl[{last}]\"  {};", list(&|k| if k.weight_lock { "yes".into() } else { "no".to_string() }));
        }
        if keys.keys.iter().any(|k| k.breakdown) {
            let _ = writeln!(out, "\tsetAttr -s {count} \".kbd[{last}]\"  {};", list(&|k| if k.breakdown { "yes".into() } else { "no".to_string() }));
        }
        // Fixed slopes as vectors one frame long. A time curve's value goes in seconds.
        let dx = 1.0 / anim::fps();
        let unit = if ty.ends_with('T') { dx } else { 1.0 };
        // On a weighted curve the vector's length is the tangent's weight.
        let vector = |slope: f64, weight: f64| {
            let (x, y) = (dx, slope * unit);
            let len = (x * x + y * y).sqrt();
            if keys.weighted && len > 0.0 {
                (x * weight / len, y * weight / len)
            } else {
                (x, y)
            }
        };
        for (i, k) in keys.keys.iter().enumerate() {
            if k.itt == Tangent::Fixed {
                let (x, y) = vector(k.in_slope, k.in_weight);
                let _ = writeln!(out, "\tsetAttr \".kix[{i}]\" {x};");
                let _ = writeln!(out, "\tsetAttr \".kiy[{i}]\" {y};");
            }
            if k.ott == Tangent::Fixed {
                let (x, y) = vector(k.out_slope, k.out_weight);
                let _ = writeln!(out, "\tsetAttr \".kox[{i}]\" {x};");
                let _ = writeln!(out, "\tsetAttr \".koy[{i}]\" {y};");
            }
        }
    }
    if pre != 0 {
        let _ = writeln!(out, "\tsetAttr \".pre\" {pre};");
    }
    if post != 0 {
        let _ = writeln!(out, "\tsetAttr \".pst\" {post};");
    }
    out
}

/// Nodes whose result the mesh holds once baked: anything that outputs a mesh.
fn n_is_baked(t: &NodeType) -> bool {
    // Curve history bakes into the curve's .cc as mesh history does into .vt; the info nodes are written below.
    if t.name.starts_with("makeNurb") || matches!(t.name.as_str(), "loft" | "revolve" | "extrude" | "boundary" | "rebuildSurface" | "reverseSurface" | "insertKnotSurface" | "offsetSurface" | "closeSurface" | "squareSrf" | "planarTrim" | "boundaryBase") {
        return true;
    }
    const CURVE_HISTORY: &[&str] = &["makeNurbCircle", "makeNurbsSquare", "rebuildCurve", "reverseCurve", "closeCurve", "smoothCurve", "fitBspline", "extendCurve", "offsetCurve", "insertKnotCurve"];
    CURVE_HISTORY.contains(&t.name.as_str()) || t.name == "camera" || t.attrs.iter().any(|a| a.ty == PlugType::Mesh && a.has(flags::OUTPUT))
}

/// A number as short as it stays exact.
fn fmt_num(v: f64) -> String {
    format!("{v}")
}

// ---------------------------------------------------------------------------------------------------------------------
// Files

/// The scene file types: mayaAscii (`.ma`) and mayaBinary (`.mb`), as the reference has them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Ma,
    Mb,
}

impl Format {
    /// mayaBinary for `.mb`, mayaAscii for anything else.
    pub fn of_path(path: &str) -> Format {
        if extension(path) == "mb" {
            Format::Mb
        } else {
            Format::Ma
        }
    }
    pub fn of_type(ty: &str) -> CmdResult<Format> {
        match ty {
            "mayaAscii" => Ok(Format::Ma),
            "mayaBinary" => Ok(Format::Mb),
            other => fail!("file: unknown file type {other}"),
        }
    }
    pub fn type_name(self) -> &'static str {
        match self {
            Format::Ma => "mayaAscii",
            Format::Mb => "mayaBinary",
        }
    }
    pub fn extension(self) -> &'static str {
        match self {
            Format::Ma => "ma",
            Format::Mb => "mb",
        }
    }
}

fn extension(path: &str) -> String {
    std::path::Path::new(path).extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase()
}

/// A scene file's text; a binary `.mb` file comes as `.ma` text, with its own units and what it couldn't carry.
struct SceneText {
    text: String,
    binary: Option<crate::mb_file::Converted>,
}

fn read_scene_text(path: &str) -> Result<SceneText, String> {
    let mut bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    // A file saved with -compress is gzipped.
    if bytes.starts_with(&[0x1f, 0x8b]) {
        bytes = crate::file_more::gunzip(&bytes).map_err(|e| format!("{path}: {e}"))?;
    }
    if crate::mb_file::is_binary(&bytes) {
        let mut c = crate::mb_file::convert(&bytes).map_err(|e| format!("{path}: {e}"))?;
        let text = std::mem::take(&mut c.text);
        return Ok(SceneText { text, binary: Some(c) });
    }
    if extension(path) == "mb" {
        return Err(format!("{path}: not a binary scene file"));
    }
    let text = String::from_utf8_lossy(&bytes).into_owned();
    if text.trim_start().starts_with('{') && text.contains("\"aztek\"") {
        return Err(format!("{path}: a scene in Aztek's old .aztek format, which is no longer read; open it in an earlier Aztek and save it as .ma or .mb"));
    }
    Ok(SceneText { text, binary: None })
}

/// Writes the scene to path in the format given. Returns warnings; does not change the scene's name.
pub fn write_file(s: &mut Session, path: &str, format: Format, only: Option<&[NodeId]>) -> Result<Vec<String>, String> {
    if s.commands.find("aztekUsdBeforeSave").is_some() {
        let _ = s.run("aztekUsdBeforeSave", json!({}));
    }
    let warnings = match format {
        Format::Ma => {
            let (text, warnings) = write_text(s, path, only);
            std::fs::write(path, text).map_err(|e| format!("{path}: {e}"))?;
            warnings
        }
        Format::Mb => {
            let (bytes, warnings) = binary_bytes(s, path, only);
            std::fs::write(path, bytes).map_err(|e| format!("{path}: {e}"))?;
            warnings
        }
    };
    for w in &warnings {
        log::warn!("{w}");
    }
    Ok(warnings)
}

/// file -exportAnim and the like: the curves given and what they drive, written as connections to nodes the file
/// doesn't hold, which an import makes to the nodes of those names.
pub(crate) fn write_anim_file(s: &mut Session, path: &str, format: Format, curves: &[NodeId]) -> Result<(), String> {
    let mut lines = String::new();
    for &c in curves {
        let g = &s.graph;
        let Some(n) = g.node(c) else { continue };
        let Some(out) = n.ty().attr_index("output") else { continue };
        for d in g.destinations(&PlugRef::new(c, out as i32)) {
            if curves.contains(&d.node) {
                continue;
            }
            let Some(dn) = g.node(d.node) else { continue };
            let attr = ref_name(&dn.ty().attrs[d.attr as usize], d.component);
            let _ = writeln!(lines, "connectAttr {} {};", quoted(&format!("{}.o", g.path(c))), quoted(&format!("{}.{attr}", g.path(d.node))));
        }
    }
    let write = |s: &mut Session| -> (String, Vec<String>) {
        ANIM_EXPORT.with(|c| c.set(true));
        let (mut text, w) = write_text(s, path, Some(curves));
        ANIM_EXPORT.with(|c| c.set(false));
        let at = text.rfind("// End of").unwrap_or(text.len());
        text.insert_str(at, &lines);
        (text, w)
    };
    match format {
        Format::Ma => {
            let (text, _) = write(s);
            std::fs::write(path, text).map_err(|e| format!("{path}: {e}"))
        }
        Format::Mb => {
            let (lin, ang) = (units::cm_per_unit(), units::rad_per_unit());
            let (linear, angle) = (units::linear_name().to_string(), units::angle_name().to_string());
            units::set_linear(1.0);
            units::set_angle(1.0);
            let (text, _) = write(s);
            units::set_linear(lin);
            units::set_angle(ang);
            let fps = anim::fps();
            let hints = crate::mb_write::Hints { linear, angle, time: time_unit_name(fps), fps, dag_roots: Default::default(), transform_types: Default::default() };
            let (bytes, _) = crate::mb_write::encode(&text, &hints);
            std::fs::write(path, bytes).map_err(|e| format!("{path}: {e}"))
        }
    }
}

/// What export would write for these nodes: what export selected takes with them, or just them when strict.
pub(crate) fn export_closure(s: &Session, only: &[NodeId], strict: bool) -> Vec<NodeId> {
    let g = &s.graph;
    let keep: HashSet<NodeId> = if strict { only.iter().copied().collect() } else { closure(g, only) };
    let mut v: Vec<NodeId> = keep.into_iter().filter(|&id| g.node(id).is_some_and(|n| n.reference_id() == 0)).collect();
    v.sort();
    v
}

/// The scene as a binary file: its `.ma` text in internal units (values in a binary file are centimetres and
/// radians whatever the scene's units), then as chunks. The header keeps the scene's own units.
fn binary_bytes(s: &mut Session, path: &str, only: Option<&[NodeId]>) -> (Vec<u8>, Vec<String>) {
    let (lin, ang) = (units::cm_per_unit(), units::rad_per_unit());
    let (linear, angle) = (units::linear_name().to_string(), units::angle_name().to_string());
    units::set_linear(1.0);
    units::set_angle(1.0);
    let (text, mut warnings) = write_text(s, path, only);
    units::set_linear(lin);
    units::set_angle(ang);
    let g = &s.graph;
    let dag_roots = g.roots().iter().filter_map(|&r| g.node(r).map(|n| n.name().to_string())).collect();
    let transform_types = g.nodes().iter().filter_map(|&id| g.node(id)).filter(|n| n.ty().is_a("transform")).map(|n| n.ty().name.clone()).collect();
    let fps = anim::fps();
    let hints = crate::mb_write::Hints { linear, angle, time: time_unit_name(fps), fps, dag_roots, transform_types };
    let (bytes, more) = crate::mb_write::encode(&text, &hints);
    warnings.extend(more);
    (bytes, warnings)
}

/// Saves under path and makes it the scene's name.
pub fn save_scene(s: &mut Session, path: &str, format: Format) -> Result<Vec<String>, String> {
    let w = write_file(s, path, format, None)?;
    s.scene_path = Some(path.to_string());
    s.scene_type = Some(format);
    s.mark_saved();
    Ok(w)
}

/// Replaces the scene with the file's, `.ma` or `.mb`. Undo restarts.
pub fn open_scene(s: &mut Session, path: &str) -> Result<Vec<String>, String> {
    let SceneText { text, binary } = read_scene_text(path)?;
    let _ = s.run("newScene", json!({}));
    // Here and not at the next command: a script can use a camera straight after opening a file.
    crate::cameras::make_defaults(&mut s.graph);
    crate::references::note_scene_folder(s, path);
    let g = &mut s.graph;
    g.begin_batch();
    g.undo_stack_mut().set_enabled(false);
    let mut warnings = binary_warnings(&binary);
    warnings.extend(read_text(s, &text, false));
    // The scene takes on the file's units, as it does its frame rate. A binary file's values are in internal
    // units, so its text says centimetres and radians and its own units come from its header.
    let (lin, ang) = match &binary {
        Some(b) => (linear_unit(&b.linear).unwrap_or(1.0), if b.angle.starts_with("rad") { 1.0 } else { std::f64::consts::PI / 180.0 }),
        None => FILE_UNITS.with(Cell::get),
    };
    units::set_linear(lin);
    units::set_angle(ang);
    let g = &mut s.graph;
    g.undo_stack_mut().set_enabled(true);
    g.undo_stack_mut().clear();
    g.end_batch();
    let _ = s.run("flushUndo", json!({}));
    s.scene_path = Some(path.to_string());
    s.scene_type = Some(if binary.is_some() { Format::Mb } else { Format::Ma });
    crate::shading::scene_was_read(s);
    s.mark_saved();
    Ok(warnings)
}

/// What a binary file couldn't carry over, logged as read_text logs its own.
fn binary_warnings(binary: &Option<crate::mb_file::Converted>) -> Vec<String> {
    let w = binary.as_ref().map(|b| b.warnings.clone()).unwrap_or_default();
    w.iter().for_each(|x| log::warn!("{x}"));
    w
}

/// The file's nodes with connections into nodes it doesn't hold.
fn outside_sources(text: &str) -> HashSet<String> {
    let mut made: HashSet<String> = HashSet::new();
    let mut pairs = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("createNode ") {
            if let Some(n) = rest.split("-n \"").nth(1).and_then(|x| x.split('"').next()) {
                made.insert(n.to_string());
            }
        } else if let Some(rest) = t.strip_prefix("connectAttr ") {
            let q: Vec<&str> = rest.split('"').collect();
            if q.len() >= 4 {
                if let (Some((a, _)), Some((b, _))) = (q[1].split_once('.'), q[3].split_once('.')) {
                    pairs.push((a.to_string(), b.trim_start_matches(':').to_string()));
                }
            }
        }
    }
    pairs.into_iter().filter(|(a, b)| made.contains(a) && !made.contains(b) && !b.is_empty()).map(|x| x.0).collect()
}

/// The file's connections from its own nodes into nodes it doesn't hold, made to the scene's nodes of those names: what
/// an animation export (file -exportAnim) leaves for the import to connect.
fn connect_to_scene(s: &mut Session, text: &str) {
    let read = LAST_READ_NAMES.with(|n| n.borrow().clone());
    if read.is_empty() || !text.contains("connectAttr") {
        return;
    }
    let short = |n: &str| n.rsplit('|').next().unwrap_or(n).to_string();
    for line in text.lines() {
        let Some(rest) = line.trim().strip_prefix("connectAttr ") else { continue };
        let q: Vec<&str> = rest.split('"').collect();
        if q.len() < 4 {
            continue;
        }
        let (a, b) = (q[1], q[3]);
        let (Some((an, aa)), Some((bn, ba))) = (a.split_once('.'), b.split_once('.')) else { continue };
        if read.iter().any(|(_, n)| short(n) == short(bn)) {
            continue;
        }
        let Some((src_id, _)) = read.iter().find(|(_, n)| short(n) == short(an)) else { continue };
        let g = &mut s.graph;
        // Only animation: anything else the file connects outside itself is the reader's.
        if !g.node(*src_id).is_some_and(|n| n.ty().is_a("animCurve") || DRIVEN_CURVES.contains(&n.ty().name.as_str())) {
            continue;
        }
        let from = format!("{}.{aa}", g.path(*src_id));
        if let (Ok(f), Ok(t)) = (g.plug(&from), g.plug(&format!("{bn}.{ba}"))) {
            if g.source(&t).is_none() {
                let _ = g.connect(&f, &t, true);
            }
        }
    }
}

/// Adds a `.ma` or `.mb` scene file's contents to the scene. One undo step.
pub fn import_scene(s: &mut Session, path: &str) -> Result<Vec<String>, String> {
    let SceneText { text, binary } = read_scene_text(path)?;
    crate::references::note_scene_folder(s, path);
    let mut w = binary_warnings(&binary);
    OUTSIDE_SOURCES.with(|o| *o.borrow_mut() = outside_sources(&text));
    w.extend(read_text(s, &text, true));
    OUTSIDE_SOURCES.with(|o| o.borrow_mut().clear());
    connect_to_scene(s, &text);
    // An edits file (exportEdits): its reference edits go onto the nodes they name.
    if text.contains("\n//edit ") {
        crate::references::apply_edit_lines(s, &text);
    }
    Ok(w)
}

// ---------------------------------------------------------------------------------------------------------------------
// The file command

fn file_command(s: &mut Session, a: &Args) -> CmdResult {
    let path_arg = a.pos_str(0).map(str::to_string);
    let type_flag = a.get("type").and_then(Value::as_str).filter(|t| !t.is_empty()).map(str::to_string);

    if a.bool("query") {
        if let Some(v) = crate::references::file_query(s, a, path_arg.as_deref()) {
            return Ok(v);
        }
        return Ok(file_query(s, a, path_arg.as_deref()));
    }
    // -returnNewNodes on a translator's import: the nodes it made, as the reference returns them, not the file.
    let before: Option<std::collections::HashSet<_>> = (a.bool("returnNewNodes") && (a.bool("import") || a.bool("open"))).then(|| s.graph.nodes().into_iter().collect());
    let routes: [fn(&mut Session, &Args, Option<&str>, Option<&str>) -> Option<CmdResult>; 7] = [plugin_translator_file, fbx_file, usd_file, obj_file, abc_file, gltf_file, mesh_file];
    for route in routes {
        if let Some(r) = route(s, a, path_arg.as_deref(), type_flag.as_deref()) {
            return match (r, &before) {
                (Ok(_), Some(old)) => {
                    let g = &s.graph;
                    Ok(json!(g.nodes().into_iter().filter(|id| !old.contains(id)).map(|id| g.path(id)).collect::<Vec<_>>()))
                }
                (r, _) => r,
            };
        }
    }
    if let Some(r) = crate::references::file_flags(s, a) {
        return r;
    }
    if a.bool("reference") {
        let Some(path) = path_arg else { fail!("file: -reference needs a path") };
        return crate::references::file_reference(s, a, &path);
    }
    if let Some(t) = &type_flag {
        // A file is read by what's in it; the type is what the next save writes.
        let reading = ["open", "import", "reference"].iter().any(|f| a.bool(f));
        let format = Format::of_type(t)?;
        if !reading {
            s.scene_type = Some(format);
        }
    }
    let format_for = |s: &Session, path: &str| -> Format {
        match s.scene_type {
            Some(f) if extension(path).is_empty() || type_flag.is_some() => f,
            _ => Format::of_path(path),
        }
    };
    // As the reference: a script has to say -force to throw changes away.
    if (a.bool("new") || a.bool("open")) && !a.bool("force") && s.is_modified() {
        fail!("file: Unsaved changes.");
    }
    if a.bool("new") {
        let _ = s.run("newScene", json!({}))?;
        s.scene_path = None;
        s.scene_type = None;
        return Ok(json!("untitled"));
    }
    if let Some(name) = a.get("rename").and_then(Value::as_str) {
        s.scene_path = Some(name.to_string());
        return Ok(json!(name));
    }
    if a.bool("open") {
        let Some(path) = path_arg else { fail!("file: -open needs a path") };
        open_scene(s, &path).map_err(aztek_ops::CommandError::new)?;
        return Ok(json!(path));
    }
    if a.bool("import") {
        let Some(path) = path_arg else { fail!("file: -import needs a path") };
        let before: std::collections::HashSet<NodeId> = s.graph.nodes().into_iter().collect();
        s.graph.open_chunk("Import");
        let r = import_scene(s, &path).map_err(aztek_ops::CommandError::new).and_then(|_| {
            // -renamingPrefix (with -renameAll, or on its own): prefix_name, as the reference names pasted__pCube1.
            let new: Vec<NodeId> = s.graph.nodes().into_iter().filter(|id| !before.contains(id)).collect();
            let prefix = a.string("renamingPrefix", "");
            // -namespace: under that namespace, or the next free numbered one when it is in use and
            // -mergeNamespacesOnClash is off (ns1, ns2...), as the reference does. -renamingPrefix when there is none.
            let mut ns = a.string("namespace", "").trim_start_matches(':').to_string();
            if ns == ":" {
                ns.clear();
            }
            if !ns.is_empty() && !a.bool("mergeNamespacesOnClash") && crate::ref_scene::namespace_exists(&s.graph, &ns) {
                let stem = ns.trim_end_matches(|c: char| c.is_ascii_digit()).to_string();
                let mut k = 1;
                while crate::ref_scene::namespace_exists(&s.graph, &format!("{stem}{k}")) {
                    k += 1;
                }
                ns = format!("{stem}{k}");
            }
            if !ns.is_empty() || !prefix.is_empty() {
                // From the name in the file, not the one a clash gave it: box pasted beside box is pasted__box.
                let file_names = LAST_READ_NAMES.with(|n| n.borrow().clone());
                if !ns.is_empty() {
                    s.graph.set_namespace_known(&ns, true);
                }
                for &id in &new {
                    let Some(now) = s.graph.node(id).map(|n| n.name().to_string()) else { continue };
                    let name = file_names.iter().find(|x| x.0 == id).map_or(now, |x| x.1.rsplit(':').next().unwrap_or(&x.1).to_string());
                    let to = if ns.is_empty() { format!("{prefix}_{name}") } else { format!("{ns}:{name}") };
                    s.graph.rename(id, &to)?;
                }
            }
            Ok(new)
        });
        s.graph.close_chunk();
        let new = r?;
        if a.bool("returnNewNodes") {
            return Ok(json!(new.iter().map(|&id| s.graph.path(id)).collect::<Vec<_>>()));
        }
        return Ok(json!(path));
    }
    if a.bool("exportSelected") || a.bool("exportAll") {
        let Some(path) = path_arg else { fail!("file: export needs a path") };
        let path = with_type_extension(&path, type_flag.as_deref());
        let format = format_for(s, &path);
        let only: Option<Vec<NodeId>> = a.bool("exportSelected").then(|| s.graph.selection().to_vec());
        if only.as_ref().is_some_and(Vec::is_empty) {
            fail!("file: nothing selected to export");
        }
        write_file(s, &path, format, only.as_deref()).map_err(aztek_ops::CommandError::new)?;
        return Ok(json!(path));
    }
    if a.bool("save") {
        let Some(path) = path_arg.or_else(|| s.scene_path.clone()) else { fail!("file: the scene has no name; use file -rename first") };
        let path = with_type_extension(&path, s.scene_type.map(Format::type_name));
        let format = format_for(s, &path);
        save_scene(s, &path, format).map_err(aztek_ops::CommandError::new)?;
        return Ok(json!(path));
    }
    // -type or -options alone only set what the next save uses.
    Ok(Value::Null)
}

/// FBX files go to the FBX plug-in's commands, as the reference's file command hands them to its translator: the
/// types FBX (reading) and "FBX export" (writing), or a .fbx name with no type. The plug-in's settings apply, not
/// -options. Without the plug-in the type is not known.
fn fbx_file(s: &mut Session, a: &Args, path: Option<&str>, ty: Option<&str>) -> Option<CmdResult> {
    let path = path?;
    let moving = ["import", "open", "exportAll", "exportSelected"].iter().find(|f| a.bool(f))?;
    let named = matches!(ty, Some(t) if t.eq_ignore_ascii_case("FBX") || t.eq_ignore_ascii_case("FBX export"));
    if !named && !(ty.is_none() && extension(path) == "fbx") {
        return None;
    }
    let loaded = s.commands.find("FBXImport").is_some();
    if !loaded {
        let t = ty.unwrap_or("FBX");
        return Some(Err(aztek_ops::CommandError::new(format!("file: unknown file type {t}: the fbx plug-in is not loaded"))));
    }
    Some(match *moving {
        "open" => {
            if !a.bool("force") && s.is_modified() {
                return Some(Err(aztek_ops::CommandError::new("file: Unsaved changes.")));
            }
            let _ = s.run("newScene", json!({}));
            s.scene_path = None;
            s.run("FBXImport", json!({"file": path})).map(|_| json!(path))
        }
        "import" => s.run("FBXImport", json!({"file": path})).map(|_| json!(path)),
        sel => s.run("FBXExport", json!({"file": path, "selected": sel == "exportSelected"})).map(|_| json!(path)),
    })
}

/// USD files go to the USD plug-in's command (mayaUsdPlugin). Without the plug-in the types are not known.
fn usd_file(s: &mut Session, a: &Args, path: Option<&str>, ty: Option<&str>) -> Option<CmdResult> {
    let path = path?;
    let moving = ["import", "open", "exportAll", "exportSelected"].iter().find(|f| a.bool(f))?;
    let by_type = matches!(ty, Some("USD Import" | "USD Export"));
    let by_extension = ty.is_none() && matches!(extension(path).as_str(), "usd" | "usda" | "usdc" | "usdz");
    if !(by_type || by_extension) {
        return None;
    }
    if s.commands.find("aztekUsdFile").is_none() {
        let t = ty.unwrap_or("USD Import");
        return Some(Err(aztek_ops::CommandError::new(format!("file: unknown file type {t}: the mayaUsdPlugin plug-in is not loaded"))));
    }
    let mut call = json!({"file": path, "options": a.string("options", "")});
    call[*moving] = json!(true);
    Some(s.run("aztekUsdFile", call))
}

/// Alembic files go to the AbcImport plug-in, as the reference's "Alembic" translator: read only (writing is
/// AbcExport's). A .abc name with no type is read as Alembic. Without the plug-in the type is not known.
fn abc_file(s: &mut Session, a: &Args, path: Option<&str>, ty: Option<&str>) -> Option<CmdResult> {
    let path = path?;
    let moving = ["import", "open", "exportAll", "exportSelected"].iter().find(|f| a.bool(f))?;
    let named = matches!(ty, Some(t) if t.eq_ignore_ascii_case("Alembic"));
    if !named && !(ty.is_none() && extension(path) == "abc") {
        return None;
    }
    if s.commands.find("AbcImport").is_none() {
        return Some(Err(aztek_ops::CommandError::new("file: unknown file type Alembic: the AbcImport plug-in is not loaded")));
    }
    Some(match *moving {
        "open" => {
            if !a.bool("force") && s.is_modified() {
                return Some(Err(aztek_ops::CommandError::new("file: Unsaved changes.")));
            }
            let _ = s.run("newScene", json!({}));
            s.scene_path = None;
            s.run("AbcImport", json!({"args": [path]})).map(|_| json!(path))
        }
        "import" => s.run("AbcImport", json!({"args": [path]})).map(|_| json!(path)),
        _ => Err(aztek_ops::CommandError::new("file: the Alembic type reads only; write with AbcExport")),
    })
}

/// glTF files go to the gltf plug-in's importGltf, read only (exportGltf writes). A .glb or .gltf name with no type
/// is read as glTF. Without the plug-in the type is not known.
fn gltf_file(s: &mut Session, a: &Args, path: Option<&str>, ty: Option<&str>) -> Option<CmdResult> {
    let path = path?;
    let moving = ["import", "open", "exportAll", "exportSelected"].iter().find(|f| a.bool(f))?;
    let named = matches!(ty, Some(t) if t.eq_ignore_ascii_case("glTF") || t.eq_ignore_ascii_case("glTF 2.0"));
    if !named && !(ty.is_none() && matches!(extension(path).as_str(), "glb" | "gltf")) {
        return None;
    }
    if s.commands.find("importGltf").is_none() {
        return Some(Err(aztek_ops::CommandError::new("file: unknown file type glTF: the gltf plug-in is not loaded")));
    }
    Some(match *moving {
        "open" => {
            if !a.bool("force") && s.is_modified() {
                return Some(Err(aztek_ops::CommandError::new("file: Unsaved changes.")));
            }
            let _ = s.run("newScene", json!({}));
            s.scene_path = None;
            s.run("importGltf", json!({"args": [path]})).map(|_| json!(path))
        }
        "import" => s.run("importGltf", json!({"args": [path]})).map(|_| json!(path)),
        _ => Err(aztek_ops::CommandError::new("file: the glTF type reads only; write with exportGltf")),
    })
}

/// PLY and STL meshes read with importPly and importStl, and are written with exportPly and exportStl.
fn mesh_file(s: &mut Session, a: &Args, path: Option<&str>, ty: Option<&str>) -> Option<CmdResult> {
    let path = path?;
    let moving = ["import", "open", "exportAll", "exportSelected"].iter().find(|f| a.bool(f))?;
    let ext = match ty {
        Some(t) if t.eq_ignore_ascii_case("PLY") => "ply".to_string(),
        Some(t) if t.eq_ignore_ascii_case("STL") => "stl".to_string(),
        None => extension(path),
        _ => return None,
    };
    let (read, write) = match ext.as_str() {
        "ply" => ("importPly", "exportPly"),
        "stl" => ("importStl", "exportStl"),
        _ => return None,
    };
    s.commands.find(read)?;
    Some(match *moving {
        "open" => {
            if !a.bool("force") && s.is_modified() {
                return Some(Err(aztek_ops::CommandError::new("file: Unsaved changes.")));
            }
            let _ = s.run("newScene", json!({}));
            s.scene_path = None;
            s.run(read, json!({"args": [path]})).map(|_| json!(path))
        }
        "import" => s.run(read, json!({"args": [path]})).map(|_| json!(path)),
        sel => {
            let mut args = vec![json!(path)];
            if sel == "exportAll" {
                let g = &s.graph;
                args.extend(g.nodes().into_iter().filter(|&id| crate::dag::is_transform(g, id) && crate::components::as_mesh(g, id).is_some()).map(|id| json!(g.path(id))));
            }
            s.run(write, json!({"args": args})).map(|_| json!(path))
        }
    })
}

/// The file types `file -type` knows, as the reference's translators: (name, extension, read, write, the command a
/// plug-in brings when the type is its).
pub const TRANSLATORS: &[(&str, &str, bool, bool, Option<&str>)] = &[
    ("mayaAscii", "ma", true, true, None),
    ("mayaBinary", "mb", true, true, None),
    ("OBJ", "obj", true, false, Some("importObj")),
    ("OBJexport", "obj", false, true, Some("exportObj")),
    ("FBX", "fbx", true, false, Some("FBXImport")),
    ("FBX export", "fbx", false, true, Some("FBXExport")),
    ("USD Import", "usd", true, false, Some("mayaUSDImport")),
    ("USD Export", "usd", false, true, Some("mayaUSDExport")),
    ("Alembic", "abc", true, false, Some("AbcImport")),
    ("glTF", "glb", true, false, Some("importGltf")),
    ("PLY", "ply", true, true, None),
    ("STL", "stl", true, true, None),
];

/// A file translator a plug-in registered (MPxFileTranslator): its name, extension, and whether it reads and writes.
#[derive(Clone, Debug)]
pub struct PluginTranslator {
    pub name: String,
    pub extension: String,
    pub read: bool,
    pub write: bool,
    pub owner: String,
    pub default_options: String,
    /// The MEL procedure that posts its options in the file dialogs, or "".
    pub options_script: String,
}

/// What runs a plug-in's translator: (session, translator, "open" | "import" | "reference" | "exportAll" |
/// "exportSelected" | "save", path, options).
pub type TranslatorHook = std::sync::Arc<dyn Fn(&mut Session, &str, &str, &str, &str) -> CmdResult + Send + Sync>;

static PLUGIN_TRANSLATORS: std::sync::RwLock<Vec<PluginTranslator>> = std::sync::RwLock::new(Vec::new());
static TRANSLATOR_HOOK: std::sync::RwLock<Option<TranslatorHook>> = std::sync::RwLock::new(None);

pub fn set_translator_hook(h: TranslatorHook) {
    *TRANSLATOR_HOOK.write().unwrap() = Some(h);
}

pub fn register_plugin_translator(t: PluginTranslator) {
    let mut l = PLUGIN_TRANSLATORS.write().unwrap();
    l.retain(|x| x.name != t.name);
    l.push(t);
}

pub fn deregister_plugin_translator(name: &str) {
    PLUGIN_TRANSLATORS.write().unwrap().retain(|x| x.name != name);
}

pub fn plugin_translators() -> Vec<PluginTranslator> {
    PLUGIN_TRANSLATORS.read().unwrap().clone()
}

/// A plug-in translator's file: named by -type, or a file with its extension when no type is given.
fn plugin_translator_file(s: &mut Session, a: &Args, path: Option<&str>, ty: Option<&str>) -> Option<CmdResult> {
    let mode = ["open", "import", "exportAll", "exportSelected"].into_iter().find(|f| a.bool(f)).or_else(|| a.bool("save").then_some("save"))?;
    let owned = match path {
        Some(p) => p.to_string(),
        None if mode == "save" => s.scene_path.clone()?,
        None => return None,
    };
    let path = owned.as_str();
    let writing = mode.starts_with("export") || mode == "save";
    let list = plugin_translators();
    let t = match ty {
        Some(t) => list.into_iter().find(|x| x.name.eq_ignore_ascii_case(t))?,
        None => {
            let ext = extension(path);
            list.into_iter().find(|x| !x.extension.is_empty() && x.extension.eq_ignore_ascii_case(&ext) && if writing { x.write } else { x.read })?
        }
    };
    if (writing && !t.write) || (!writing && !t.read) {
        return Some(Err(aztek_ops::CommandError::new(format!("file: the {} type can't {}", t.name, if writing { "write" } else { "read" }))));
    }
    let hook = TRANSLATOR_HOOK.read().unwrap().clone()?;
    if mode == "open" {
        if !a.bool("force") && s.is_modified() {
            return Some(Err(aztek_ops::CommandError::new("file: Unsaved changes.")));
        }
        let _ = s.run("newScene", json!({}));
        s.scene_path = None;
    }
    let options = a.get("options").and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| t.default_options.clone());
    Some(hook(s, &t.name, mode, path, &options).map(|_| json!(path)))
}

/// OBJ files go to the objExport plug-in's commands, as the reference's file command hands the OBJ (reading) and
/// OBJexport (writing) types to that plug-in's translator; a .obj name with no type is read as OBJ. Without the
/// plug-in the types are not known.
fn obj_file(s: &mut Session, a: &Args, path: Option<&str>, ty: Option<&str>) -> Option<CmdResult> {
    let path = path?;
    let moving = ["import", "open", "exportAll", "exportSelected"].iter().find(|f| a.bool(f))?;
    let named = matches!(ty, Some(t) if t.eq_ignore_ascii_case("OBJ") || t.eq_ignore_ascii_case("OBJexport"));
    if !named && !(ty.is_none() && extension(path) == "obj") {
        return None;
    }
    if s.commands.find("importObj").is_none() {
        let t = ty.unwrap_or("OBJ");
        return Some(Err(aztek_ops::CommandError::new(format!("file: unknown file type {t}: the objExport plug-in is not loaded"))));
    }
    Some(match *moving {
        "open" => {
            if !a.bool("force") && s.is_modified() {
                return Some(Err(aztek_ops::CommandError::new("file: Unsaved changes.")));
            }
            let _ = s.run("newScene", json!({}));
            s.scene_path = None;
            s.run("importObj", json!({"args": [path]})).map(|_| json!(path))
        }
        "import" => s.run("importObj", json!({"args": [path]})).map(|_| json!(path)),
        sel => {
            let mut args = vec![json!(path)];
            if sel == "exportAll" {
                let g = &s.graph;
                args.extend(g.nodes().into_iter().filter(|&id| crate::dag::is_transform(g, id) && crate::components::as_mesh(g, id).is_some()).map(|id| json!(g.path(id))));
            }
            s.run("exportObj", json!({"args": args})).map(|_| json!(path))
        }
    })
}

/// A bare name takes the extension of the type asked for, as the reference does.
fn with_type_extension(path: &str, ty: Option<&str>) -> String {
    if extension(path).is_empty() && DEFAULT_EXTENSIONS.with(Cell::get) {
        match ty {
            Some("mayaAscii") => return format!("{path}.ma"),
            Some("mayaBinary") => return format!("{path}.mb"),
            _ => {}
        }
    }
    path.to_string()
}

fn file_query(s: &mut Session, a: &Args, path: Option<&str>) -> Value {
    // -list: the scene's file, then the files it uses (references, textures, image planes, sounds, caches), where
    // they were found or as written when they weren't. -withoutCopyNumber and -unresolvedName are references' own.
    if a.bool("list") || crate::ref_core::asked(a, "list") {
        let mut out: Vec<String> = s.scene_path.iter().cloned().collect();
        for e in crate::external_files::list(s) {
            out.push(e.found.map_or(e.written, |p| p.to_string_lossy().replace('\\', "/")));
        }
        return json!(out);
    }
    if a.bool("exists") {
        return json!(path.map(str::to_string).or_else(|| s.scene_path.clone()).is_some_and(|p| std::path::Path::new(&p).exists()));
    }
    if a.has("type") || crate::ref_core::asked(a, "type") {
        let t = s.scene_type.or(s.scene_path.as_deref().map(Format::of_path)).unwrap_or(Format::Ma);
        return json!([t.type_name()]);
    }
    if a.bool("modified") {
        return json!(s.is_modified());
    }
    if a.has("options") || crate::ref_core::asked(a, "options") {
        return json!("");
    }
    let name = s.scene_path.clone().unwrap_or_default();
    if a.bool("shortName") {
        return json!(std::path::Path::new(&name).file_name().and_then(|n| n.to_str()).unwrap_or(""));
    }
    json!(name)
}

pub fn register(c: &mut Commands<Session>) {
    register_types();
    let b = |name: &str, short: &str, doc: &str| FlagDef::new(name, short, ArgType::Bool, Value::Null, doc);
    let t = |name: &str, short: &str, doc: &str| FlagDef::new(name, short, ArgType::String, Value::Null, doc);
    c.register(
        CommandDef::<Session>::new(
            "file",
            "Scene files: -new, -open path, -save, -rename name, -import path, -exportAll path, -exportSelected path; -type \
             mayaAscii or mayaBinary. In query mode -sceneName (with -shortName for the file alone), -exists, -type, -modified.",
            file_command,
        )
        .positional("the file")
        .undoable(false)
        .flag(b("new", "new", "A new, empty scene."))
        .flag(b("list", "l", "With -q: the scene's file and every file it uses."))
        .flag(b("open", "o", "Replace the scene with the file."))
        .flag(b("save", "s", "Write the scene under its name."))
        .flag(t("rename", "rn", "Set the scene's name, for the next save."))
        .flag(b("import", "i", "Add the file's contents to the scene."))
        .flag(b("exportAll", "ea", "Write the whole scene to the file."))
        .flag(b("exportSelected", "es", "Write the selection, what lies below it, its parents and the skeletons of its skins."))
        .flag(FlagDef::new("type", "typ", ArgType::Any, Value::Null, "mayaAscii or mayaBinary."))
        .flag(FlagDef::new("options", "op", ArgType::Any, Value::Null, "File translator options. Accepted for existing scripts."))
        .flag(b("force", "f", "Don't ask about unsaved changes. Nothing asks here."))
        .flag(b("query", "q", "Query mode."))
        .flag(FlagDef::new("queried", "", ArgType::List, Value::Null, "Flags asked about in query mode."))
        .flag(b("sceneName", "sn", "Query: the scene's path; empty when untitled."))
        .flag(b("shortName", "shn", "Query: with -sceneName, the file name alone."))
        .flag(b("exists", "ex", "Query: whether the file (or the scene's file) exists."))
        .flag(b("modified", "mf", "Query: whether the scene has edits."))
        .flag(b("reference", "r", "Bring the file in as a reference under -ns. Query: the referenced files."))
        .flag(FlagDef::new("loadReference", "lr", ArgType::Any, Value::Null, "Load a reference, by its reference node (or the path argument); loaded already, it reloads."))
        .flag(FlagDef::new("unloadReference", "ur", ArgType::Any, Value::Null, "Unload a reference, by its reference node (or the path argument)."))
        .flag(b("removeReference", "rr", "Remove the reference named by the path argument."))
        .flag(t("referenceNode", "rfn", "The reference node's name, or with -q ask for it."))
        .flag(FlagDef::new("deferReference", "dr", ArgType::Any, Value::Null, "Add the reference unloaded. Query: whether it is."))
        .flag(b("importReference", "ir", "Make the reference's contents part of the scene."))
        .flag(FlagDef::new("cleanReference", "cr", ArgType::Any, Value::Null, "Remove the edits of an unloaded reference, or -editCommand's kind."))
        .flag(FlagDef::new("editCommand", "ec", ArgType::Any, Value::Null, "With -cleanReference, which edits."))
        .flag(FlagDef::new("flushReference", "fr", ArgType::Any, Value::Null, "Unload, keeping the reference nodes."))
        .flag(FlagDef::new("loadReferencePreview", "lrp", ArgType::Any, Value::Null, "Loads the reference; there's no partial load here."))
        .flag(t("loadReferenceDepth", "lrd", "Accepted for scripts: all, none, topOnly, asPrefs."))
        .flag(b("groupReference", "gr", "Group the reference's top nodes under one transform."))
        .flag(t("groupName", "gn", "The group's name."))
        .flag(b("groupLocator", "gl", "Make the group a locator."))
        .flag(FlagDef::new("sharedNodes", "shd", ArgType::Any, Value::Null, "Accepted for scripts: nothing's shared yet."))
        .flag(b("sharedReferenceFile", "srf", "Accepted for scripts: nothing's shared yet."))
        .flag(b("mergeNamespacesOnClash", "mnc", "Use the namespace even if it's there already."))
        .flag(t("renamingPrefix", "rpr", "A prefix instead of a namespace. Query: what it is."))
        .flag(b("renameAll", "ra", "With -renamingPrefix on import: every imported node takes the prefix."))
        .flag(b("lockReference", "lck", "Lock the reference's nodes and attributes."))
        .flag(b("usingNamespaces", "uns", "Query: whether it uses a namespace."))
        .flag(b("unresolvedName", "un", "Query: the path as written."))
        .flag(b("withoutCopyNumber", "wcn", "Query: the path without its {n}."))
        .flag(b("expandName", "exn", "Query: the file's path."))
        .flag(FlagDef::new("swapNamespace", "sns", ArgType::Any, Value::Null, "Accepted for scripts."))
        .flag(b("edit", "e", "Edit mode: -namespace renames a reference's."))
        .flag(b("saveReferencesUnloaded", "sru", "Accepted for scripts."))
        .flag(b("exportUnloadedReferences", "eur", "Accepted for scripts."))
        .flag(FlagDef::new("referenceDepthInfo", "rdi", ArgType::Int, Value::Null, "Accepted for existing scripts."))
        .flag(b("prompt", "pmt", "Accepted for existing scripts."))
        .flag(b("ignoreVersion", "iv", "Accepted for existing scripts."))
        .flag(b("preserveReferences", "pr", "Accepted for existing scripts."))
        .flag(b("constructionHistory", "ch", "Accepted for existing scripts; a mesh's history is written with it."))
        .flag(b("channels", "chn", "Accepted for existing scripts."))
        .flag(b("constraints", "con", "Accepted for existing scripts."))
        .flag(b("expressions", "exp", "Accepted for existing scripts."))
        .flag(b("shader", "sh", "Accepted for existing scripts."))
        .flag(b("returnNewNodes", "rnn", "With a reference, return the new nodes."))
        .flag(t("namespace", "ns", "The namespace a reference or an import goes under.")),
    );
}

/// `viewSet -home` for a default camera, as a Z up scene's script node asks for when it switches the axis.
fn home_camera(s: &mut Session, name: &str) {
    reset_camera(s, name, crate::world::up_z());
}

/// A default camera to its home for an up axis.
fn reset_camera(s: &mut Session, name: &str, z_up: bool) {
    let g = &mut s.graph;
    let Ok(Some(tr)) = g.find(name) else { return };
    let Some(sh) = crate::cameras::shape_of(g, tr) else { return };
    let (t, r, coi) = crate::cameras::home_for(name, z_up);
    g.without_undo(|g| {
        let _ = g.set_value(PlugRef::new(tr, Transform::TRANSLATE as i32), PlugValue::Vec3(t));
        let _ = g.set_value(PlugRef::new(tr, Transform::ROTATE as i32), PlugValue::Vec3(r));
        let _ = g.set_value(PlugRef::new(sh, crate::cameras::COI as i32), PlugValue::Float(coi));
    });
}
