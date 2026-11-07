//! The polygon commands under their reference names and flags: polyExtrudeFacet, polySplitRing, polySmooth and so on,
//! each sharing its geometry with Aztek's own commands where there is one. Component arguments are read here, not
//! selected first, so the selection stays as it was. Each command returns its history node, or for meshes it makes
//! [transform, node].

use std::collections::BTreeMap;

use aztek_geo::{Index, Mesh};
use aztek_ops::{fail, ArgType, Args, CmdResult, CommandDef, Commands, FlagDef};
use glam::{DMat4, DVec3};
use serde_json::{json, Map, Value};

use super::{apply_targets, names_of};
use crate::components::{self, ComponentMode as K, Ids};
use crate::graph::Graph;
use crate::history::fold_vectors;
use crate::plug::NodeId;
use crate::session::Session;
use crate::transform::Transform;

pub use more_cmds::SymmetryState;
pub mod flag_table;
mod edit_ops;
mod mesh_cmds;
mod more_cmds;
mod queries;
mod state_cmds;
mod uv_cmds;

pub(crate) type Cmd = CommandDef<Session>;

/// What the state-holding polygon commands remember.
#[derive(Default)]
pub struct PolyState {
    /// polyOptions: display settings kept for scripts to read back.
    pub options: BTreeMap<String, Value>,
    /// polySelectConstraint: its flags by long name.
    pub constraint: Map<String, Value>,
    /// polyClipboard: the UVs and colours copied, one list per corner of the copied face.
    pub clip_uvs: Vec<[f32; 2]>,
    pub clip_colors: Vec<[f32; 4]>,
}

/// Commands that read their component arguments themselves, so the host must not select them first.
pub fn reads_components(name: &str) -> bool {
    NATIVE.contains(&name)
}

const NATIVE: &[&str] = &[
    "polyAppend", "polyAverageNormal", "polyAverageVertex", "polyBevel3", "polyBridgeEdge", "polyCBoolOp", "polyBooleanCmd", "polyBoolOp",
    "polyChamferVtx", "polyChipOff", "polyCircularize", "polyCleanup", "polyCleanupArgList", "polyClipboard", "polyCloseBorder",
    "polyCollapseEdge", "polyCollapseFacet", "polyColorPerVertex", "polyColorSet", "polyConnectComponents", "polyContourProjection",
    "polyCopyUV", "polyCreateFacet", "polyDelEdge", "polyDelFacet", "polyDelVertex", "polyDetachEdges", "polyDuplicateEdge",
    "polyEditEdgeFlow", "polyExtrudeEdge", "polyExtrudeFacet", "polyExtrudeVertex", "polyFlipEdge", "polyFlipUV", "polyForceUV", "polyHole",
    "polyInfo", "polyLayoutUV", "polyListComponentConversion", "polyMergeEdge", "polyMergeFacet", "polyMergeUV", "polyMergeVertex",
    "polyMirrorFace", "polyMoveEdge", "polyMoveFacet", "polyMoveUV", "polyMoveVertex", "polyNormal", "polyNormalPerVertex", "polyOptions",
    "polyPasteFace", "polyPinUV", "polyPoke", "polyProjectCurve", "polyQuad", "polyReduce", "polyRemesh", "polyRetopo", "polySelect",
    "polySelectConstraint", "polySelectSp", "polySeparate", "polySetToFaceNormal", "polySmooth", "polySoftEdge", "polySplit", "polySplitEdge",
    "polyCut", "polySewEdge", "polyClean", "polyColorMod", "polyColorDel", "polyAverageColor", "polyBlendColor", "polyGeoSampler", "polyBlindData", "polyQueryBlindData",
    "polyColorBlindData", "polyCheck", "polyCompare", "polyOutput", "polyDuplicateAndConnect", "polySmartBevel", "polyMoveFacetUV", "polyMapDel", "polyEditUVShell", "polyOptUvs",
    "polyUVRectangle", "polyUVOverlap", "polyUVCoverage", "polyUVStackSimilarShells", "polyStraightenUVBorder",
    "polySplitFacet", "polySplitRing", "polySplitVertex", "polySubdivideEdge", "polySubdivideFacet", "polySymmetrize", "polyTransfer",
    "polyTriangulate", "polyUVSet", "polyUnite", "polyWedgeFace", "polyMirror", "polyCrease",
];

pub(crate) fn fl(name: &str, short: &str, ty: ArgType, default: Value, doc: &str) -> FlagDef {
    FlagDef::new(name, short, ty, default, doc)
}

fn arg_type(c: char) -> ArgType {
    match c {
        'b' => ArgType::Bool,
        'i' => ArgType::Int,
        'f' => ArgType::Float,
        's' => ArgType::String,
        'v' => ArgType::Vec3,
        'l' => ArgType::List,
        _ => ArgType::Any,
    }
}

/// Adds the reference's flags to a command, skipping any it already has. Vectors default to zero, or one for scales;
/// defaults gives the others. pivot stays unset, so the command can tell when none was given.
pub(crate) fn with_table(mut cmd: Cmd, table_of: &str, defaults: &[(&str, Value)]) -> Cmd {
    for &(long, short, ty) in flag_table::table(table_of) {
        // A flag the command already has takes the reference's short name if it has none.
        if let Some(at) = cmd.flags.iter().position(|f| f.name == long) {
            if cmd.flags[at].short_name.is_empty() && !cmd.flags.iter().any(|f| f.short_name == short) {
                cmd.flags[at].short_name = short.to_string();
            }
            continue;
        }
        let mut def = defaults.iter().find(|(n, _)| *n == long).map(|(_, v)| v.clone()).unwrap_or(Value::Null);
        if def.is_null() && ty == 'v' && !long.contains("ivot") {
            def = if long.to_lowercase().ends_with("scale") { json!([1.0, 1.0, 1.0]) } else { json!([0.0, 0.0, 0.0]) };
        }
        // A short name another flag of the command holds would take that flag's calls.
        let short = if cmd.flags.iter().any(|f| f.short_name == short) { "" } else { short };
        let mut flag = fl(long, short, arg_type(ty), def, "See the command reference.");
        // Multi-use flags with a set size, so their values stop there and arguments may follow.
        flag.arity = match long {
            "edgepoint" | "edgeLoopPath" | "edgeRingPath" | "edgeBorderPath" | "edgeLoopPattern" | "edgeRingPattern" | "edgeBorderPattern"
            | "edgeLoopOrBorderPattern" | "shortestEdgePath" | "shortestEdgePathUV" | "shortestFacePath" => 2,
            "facepoint" => 4,
            "point" => 3,
            _ => 0,
        };
        cmd = cmd.flag(flag);
    }
    for (long, short, ty) in [
        ("constructionHistory", "ch", ArgType::Bool),
        ("name", "n", ArgType::String),
        ("caching", "cch", ArgType::Bool),
        ("nodeState", "nds", ArgType::Int),
        ("frozen", "fzn", ArgType::Bool),
    ] {
        if !cmd.flags.iter().any(|f| f.name == long) {
            cmd = cmd.flag(fl(long, short, ty, Value::Null, "Accepted; see the command reference."));
        }
    }
    if cmd.positional_doc.is_empty() {
        cmd = cmd.positional("components or meshes (the selection when none)");
    }
    cmd
}

/// A command with the reference's flags for its name.
pub(crate) fn ref_cmd(name: &str, doc: &str, defaults: &[(&str, Value)], run: impl Fn(&mut Session, &Args) -> CmdResult + Send + Sync + 'static) -> Cmd {
    with_table(Cmd::new(name, doc, run), name, defaults)
}

/// Adds a command's flags to a command already registered, and runs f first: f returns Some to answer instead.
pub(crate) fn extend(
    c: &mut Commands<Session>,
    name: &str,
    table_of: &str,
    defaults: &[(&str, Value)],
    f: impl Fn(&mut Session, &Args) -> Option<CmdResult> + Send + Sync + 'static,
) {
    let Some(old) = c.find(name).cloned() else {
        log::warn!("{name}: nothing to extend");
        return;
    };
    let run = old.run.clone();
    let mut cmd = with_table(old, table_of, defaults);
    cmd.run = std::sync::Arc::new(move |s: &mut Session, a: &Args| match f(s, a) {
        Some(r) => r,
        None => run(s, a),
    });
    c.register(cmd);
}

// ---------------------------------------------------------------------------------------------------------------------
// What a call works on

/// Each mesh's named components, unconverted. Empty when the call named none.
pub(crate) fn named_members(s: &Session, a: &Args) -> CmdResult<Vec<crate::object_sets::Member>> {
    let names: Vec<String> = a.named();
    if names.is_empty() {
        return Ok(Vec::new());
    }
    crate::object_sets::parse(&s.graph, &names).map_err(aztek_ops::CommandError::new)
}

/// The components a call works on as kind, per mesh: those named, else those selected, else the whole of each named or
/// selected mesh. The second value is the mode they were selected from, None when they were named.
pub(crate) fn gather(s: &mut Session, a: &Args, kind: K) -> CmdResult<(BTreeMap<NodeId, Ids>, Option<K>)> {
    let members = named_members(s, a)?;
    let g = &mut s.graph;
    g.evaluate();
    let mut out: BTreeMap<NodeId, Ids> = BTreeMap::new();
    if !members.is_empty() {
        for m in members {
            let Some(mesh) = components::as_mesh(g, m.node) else { continue };
            let Some(cage) = crate::modifiers::cage(g, mesh) else { continue };
            let ids = if m.kind == K::Object { components::all_ids(&cage, kind) } else { components::convert(&cage, m.kind, &m.ids, kind, false) };
            out.entry(mesh).or_default().extend(ids);
        }
        return Ok((out, None));
    }
    let mode = components::mode(g);
    if mode != K::Object {
        for (node, ids) in components::selected_all(g, mode).clone() {
            if !components::is_mesh(g, node) {
                continue;
            }
            if let Some(cage) = crate::modifiers::cage(g, node) {
                out.insert(node, components::convert(&cage, mode, &ids, kind, false));
            }
        }
    }
    if out.is_empty() {
        for id in g.selection().to_vec() {
            let Some(mesh) = components::as_mesh(g, id) else { continue };
            if let Some(cage) = crate::modifiers::cage(g, mesh) {
                out.insert(mesh, components::all_ids(&cage, kind));
            }
        }
    }
    Ok((out, Some(mode)))
}

/// Runs a registered edit through its history node for each mesh the call names or has selected. prep may add values
/// worked out per mesh (a pivot, a matrix) to its copy of the flags. Returns the nodes made.
pub(crate) fn run_op_with(s: &mut Session, node_type: &str, a: &Args, prep: &mut dyn FnMut(&mut Graph, NodeId, &Mesh, &Ids, &mut Args) -> CmdResult<()>) -> CmdResult {
    let op = crate::history::op(node_type).ok_or_else(|| aztek_ops::CommandError::new(format!("no history op {node_type}")))?;
    let (targets, follow) = gather(s, a, op.kind)?;
    if targets.is_empty() {
        fail!("{}: nothing to work on", op.command);
    }
    let folded = fold_vectors(a);
    let g = &mut s.graph;
    g.open_chunk(op.label);
    let r = (|| -> CmdResult<Vec<NodeId>> {
        let mut made = Vec::new();
        for (node, ids) in targets {
            let cage = crate::modifiers::cage(g, node).map(|m| (*m).clone()).unwrap_or_default();
            let mut args = folded.clone();
            prep(g, node, &cage, &ids, &mut args)?;
            let run = op.run;
            let (a1, op1) = (&args, &*op);
            let got = apply_targets(g, op.kind, BTreeMap::from([(node, ids)]), &mut |m, sel, sym| run(a1, m, sel, sym), Some((op1, a1)), follow)?;
            made.extend(got);
        }
        Ok(made)
    })();
    g.close_chunk();
    r.map(|made| names_of(g, &made))
}

pub(crate) fn run_op(s: &mut Session, node_type: &str, a: &Args) -> CmdResult {
    run_op_with(s, node_type, a, &mut |_, _, _, _, _| Ok(()))
}

/// Stores the object's matrix for ops whose world-space values need it.
pub(crate) fn add_world(g: &Graph, node: NodeId, a: &mut Args) {
    let world = g.node(node).map_or(DMat4::IDENTITY, |n| n.matrix(Transform::WORLD_MATRIX));
    a.flags.insert("worldInverse".into(), json!(world.inverse().to_cols_array().to_vec()));
}

/// The object's matrix an op was given by add_world; identity when worldSpace is off or it has none.
pub(crate) fn object_to_world(a: &Args) -> DMat4 {
    if !a.bool("worldSpace") {
        return DMat4::IDENTITY;
    }
    let Some(list) = a.get("worldInverse").and_then(Value::as_array) else { return DMat4::IDENTITY };
    let v: Vec<f64> = list.iter().filter_map(Value::as_f64).collect();
    match <[f64; 16]>::try_from(v) {
        Ok(cols) => DMat4::from_cols_array(&cols).inverse(),
        Err(_) => DMat4::IDENTITY,
    }
}

/// The hidden flag ops that need the object's matrix store it in.
pub(crate) fn world_flag() -> FlagDef {
    fl("worldInverse", "", ArgType::List, Value::Null, "The object's inverse world matrix when the edit was made.")
}

/// A flag read as on or off, with its default when the call left it out.
pub(crate) fn on(a: &Args, flag: &str, default: bool) -> bool {
    match a.get(flag) {
        Some(Value::Bool(b)) => *b,
        Some(v) => v.as_i64().map_or(default, |i| i != 0),
        None => default,
    }
}

pub(crate) fn vec_flag(a: &Args, flag: &str, default: DVec3) -> DVec3 {
    a.vec3(flag).map_or(default, DVec3::from_array)
}

/// A flag of a query: on when query mode asked for it.
pub(crate) fn asked(a: &Args, flag: &str) -> bool {
    a.bool(flag) || a.get("queried").and_then(Value::as_array).is_some_and(|q| q.iter().any(|x| x == flag))
}

pub(crate) fn query_flags(cmd: Cmd) -> Cmd {
    cmd.flag(fl("query", "q", ArgType::Bool, Value::Null, "Query mode."))
        .flag(fl("edit", "e", ArgType::Bool, Value::Null, "Edit mode."))
        .flag(fl("queried", "", ArgType::List, Value::Null, "Flags asked about in query mode."))
}

/// Ids as component strings on a mesh, one per range or one per id.
pub(crate) fn component_list(g: &Graph, mesh: NodeId, prefix: &str, ids: &[Index], flat: bool) -> Vec<String> {
    let path = components::display_path(g, mesh);
    let mut v = ids.to_vec();
    v.sort_unstable();
    v.dedup();
    if flat {
        return v.into_iter().map(|i| format!("{path}.{prefix}[{i}]")).collect();
    }
    let mut out = Vec::new();
    let mut i = 0;
    while i < v.len() {
        let mut j = i;
        while j + 1 < v.len() && v[j + 1] == v[j] + 1 {
            j += 1;
        }
        out.push(if i == j { format!("{path}.{prefix}[{}]", v[i]) } else { format!("{path}.{prefix}[{}:{}]", v[i], v[j]) });
        i = j + 1;
    }
    out
}

/// Registered last of the scene's commands, since these replace or extend ones registered before them.
pub fn register(c: &mut Commands<Session>) {
    edit_ops::register_ops();
    mesh_cmds::register_ops();
    state_cmds::register_ops();
    more_cmds::register_ops();
    edit_ops::register(c);
    mesh_cmds::register(c);
    queries::register(c);
    state_cmds::register(c);
    uv_cmds::register(c);
    more_cmds::register(c);
}
