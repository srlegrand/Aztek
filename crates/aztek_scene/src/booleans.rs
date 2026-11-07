//! polyBoolean and the live boolean modifier.

use std::sync::Arc;

use aztek_geo::Mesh;
use aztek_ops::{fail, objects, ArgType as T, CmdResult, CommandDef, Commands, FlagDef};
use glam::DMat4;
use serde_json::{json, Value};

use crate::attr_defs::{matrix_attr, mesh_attr, param, param_enum};
use crate::graph::{Graph, SelectMode};
use crate::mesh_boolean::{boolean_op_from_name, mesh_boolean, BooleanOp, BooleanOperand, BooleanOptions};
use crate::mesh_nodes::MeshNode;
use crate::modifiers::{self, ModifierNode};
use crate::node::{types, NodeData, NodeType};
use crate::plug::{flags, AttrDef, NodeId, PlugRef, PlugType, PlugValue};
use crate::session::Session;
use crate::transform::Transform;

const OPERATIONS: [&str; 3] = ["difference", "union", "intersection"];

fn attr(d: &NodeData, name: &str) -> usize {
    d.ty().attr_index(name).expect(name)
}

fn run_modifier(input: &Mesh, d: &NodeData) -> Result<Mesh, String> {
    let Some(cutter) = d.mesh(attr(d, "cutterMesh")).cloned() else {
        return Err("no cutter: connect one with polyBoolean(live=True)".into());
    };
    let world = d.matrix(attr(d, "targetMatrix"));
    let operands = [
        BooleanOperand { mesh: input, to_world: world },
        BooleanOperand { mesh: &cutter, to_world: d.matrix(attr(d, "cutterMatrix")) },
    ];
    let op = d.int(attr(d, "operation")).clamp(0, 2);
    let o = BooleanOptions {
        op: match op {
            0 => BooleanOp::Difference,
            1 => BooleanOp::Union,
            _ => BooleanOp::Intersection,
        },
        cleanup: d.bool(attr(d, "cleanup")),
        split_ngons: d.bool(attr(d, "splitNgons")),
        quad_band: d.bool(attr(d, "quadBand")),
        band_rings: d.int(attr(d, "bandRings")).clamp(0, 8) as i32,
        band_timeout: d.float(attr(d, "timeout")).max(0.0),
        fillet_width: d.float(attr(d, "filletWidth")).max(0.0),
        fillet_segments: d.int(attr(d, "filletSegments")).clamp(1, 64) as i32,
        hard_angle: d.float(attr(d, "hardAngle")),
        ..Default::default()
    };
    let r = mesh_boolean(&operands, &world.inverse(), &o);
    if !r.ok() {
        let who = match r.bad_operand {
            Some(1) => "the cutter ",
            Some(0) => "this mesh ",
            _ => "",
        };
        return Err(format!("{who}{}", r.error));
    }
    Ok(r.mesh)
}

pub fn register_types() {
    if types::find("booleanModifier").is_some() {
        return;
    }
    let mut own: Vec<AttrDef> = vec![
        mesh_attr("inMesh", "", flags::INPUT | flags::HIDDEN, ""),
        param("enabled", PlugType::Bool, true, "Turn the modifier off without removing it.", None, None),
        AttrDef {
            name: "exportEnabled".into(),
            ty: PlugType::Bool,
            default_value: PlugValue::Bool(true),
            flags: flags::INPUT | flags::STORABLE,
            doc: "Apply the modifier when exporting.".into(),
            ..Default::default()
        },
        mesh_attr("cutterMesh", "", flags::INPUT, "The cutter's outMesh, connected by polyBoolean(live=True)."),
        matrix_attr("cutterMatrix", flags::INPUT | flags::HIDDEN, "The cutter's worldMatrix."),
        matrix_attr("targetMatrix", flags::INPUT | flags::STORABLE | flags::HIDDEN, "This mesh's world matrix when the cutter was connected."),
        param_enum("operation", 0, "difference, union or intersection.", &OPERATIONS),
        param("cleanup", PlugType::Bool, true, "Weld slivers and stray points along the seam.", None, None),
        param("splitNgons", PlugType::Bool, true, "Cut the n-gons along the seam into quads that carry on the edge loops.", None, None),
        param(
            "quadBand",
            PlugType::Bool,
            false,
            "Remesh a band around the seam into quads that follow it. Slower: it reruns as the cutter moves.",
            None,
            None,
        ),
        param("bandRings", PlugType::Int, 1i64, "Rings of faces past the seam the quad band takes in.", Some(0.0), Some(8.0)),
        param("timeout", PlugType::Float, 2.0, "Seconds the quad band may take before the n-gon split runs instead.", Some(0.0), None),
        param("filletWidth", PlugType::Float, 0.0, "Width of a rounded strip along the seam, cm; 0 keeps it sharp.", Some(0.0), None),
        param("filletSegments", PlugType::Int, 2i64, "Rows of faces across the fillet.", Some(1.0), Some(64.0)),
        param("hardAngle", PlugType::Float, 30.0, "New edges sharper than this many degrees are marked hard.", Some(0.0), Some(180.0)),
    ];
    own.push(mesh_attr("outMesh", "", flags::OUTPUT | flags::HIDDEN, ""));
    let out = own.len() - 1;
    let t = NodeType {
        name: "booleanModifier".into(),
        icon: "boolean".into(),
        doc: "A boolean kept live: cuts, joins or intersects this mesh with a cutter mesh, so moving or editing the cutter \
              updates the result. Stack one per cutter. Apply bakes it."
            .into(),
        own_attrs: own,
        create: Some(Arc::new(move || Box::new(ModifierNode::new(Arc::new(run_modifier), out)))),
        ..Default::default()
    };
    types::register_type(t).expect("booleanModifier");
}

fn plug_named(g: &Graph, id: NodeId, name: &str) -> PlugRef {
    PlugRef::new(id, g.node(id).unwrap().ty().attr_index(name).expect(name) as i32)
}

/// polyBoolean(live=True): a boolean modifier per cutter, wired to it, and the cutters moved to a template layer.
fn live_boolean(s: &mut Session, nodes: &[NodeId], a: &aztek_ops::Args) -> CmdResult {
    let base = crate::components::display_path(&s.graph, nodes[0]);
    let world = s.graph.node(nodes[0]).unwrap().matrix(Transform::WORLD_MATRIX);
    let operation = match a.string("operation", "difference").as_str() {
        "union" => 1,
        "intersection" => 2,
        _ => 0,
    };
    let params = json!({
        "operation": operation,
        "cleanup": a.get("cleanup").and_then(Value::as_bool).unwrap_or(true),
        "splitNgons": a.get("splitNgons").and_then(Value::as_bool).unwrap_or(true),
        "quadBand": a.bool("quadBand"),
        "bandRings": a.int("bandRings", 1),
        "filletWidth": a.float("filletWidth", 0.0),
        "filletSegments": a.int("filletSegments", 2),
        "hardAngle": a.float("hardAngle", 30.0),
    });
    let mut cutters = Vec::new();
    for &cutter in &nodes[1..] {
        let made = s.run("addModifier", json!({"args": [base], "kind": "boolean", "params": params}))?;
        let name = made.as_str().map(str::to_string).unwrap_or_else(|| made[0].as_str().unwrap_or_default().to_string());
        let Some(m) = s.graph.find(&name)? else { fail!("polyBoolean: no modifier {name}") };
        let g = &mut s.graph;
        let (cm, cmat, tmat) = (plug_named(g, m, "cutterMesh"), plug_named(g, m, "cutterMatrix"), plug_named(g, m, "targetMatrix"));
        g.connect(&PlugRef::new(cutter, MeshNode::OUT_MESH as i32), &cm, true)?;
        g.connect(&PlugRef::new(cutter, Transform::WORLD_MATRIX as i32), &cmat, true)?;
        g.set_value(tmat, PlugValue::Matrix(world))?;
        cutters.push(Value::String(crate::components::display_path(g, cutter)));
    }
    let layer = "BooleanCutters";
    if s.graph.find(layer)?.is_none() {
        s.run("createDisplayLayer", json!({"name": layer, "empty": true}))?;
    }
    let mut members = vec![Value::String(layer.into())];
    members.extend(cutters);
    s.run("editDisplayLayerMembers", json!({"args": members}))?;
    s.run("setAttr", json!({"args": [format!("{layer}.displayType"), 1]}))?;
    s.graph.select(&[nodes[0]], SelectMode::Replace);
    Ok(Value::String(base))
}

fn flag(name: &str, ty: T, default: Value, doc: &str) -> FlagDef {
    FlagDef::new(name, "", ty, default, doc)
}

pub fn register(c: &mut Commands<Session>) {
    c.register(
        CommandDef::<Session>::new(
            "polyBoolean",
            "Union, difference or intersection of meshes (Manifold: robust, watertight output), with the seam cleaned up: \
             slivers welded, the n-gons along the cut split into quads that carry on the edge loops, an optional rounded \
             fillet, and weighted normals. The first object keeps the result; the others are deleted unless \
             keepCutters=True.",
            |s, a| {
                let objs = objects(s, a);
                if objs.len() < 2 {
                    fail!("polyBoolean: give a base and at least one cutter");
                }
                let Some(op) = boolean_op_from_name(&a.string("operation", "difference")) else {
                    fail!("polyBoolean: operation must be union, difference or intersection");
                };
                s.graph.evaluate();
                let mut nodes = Vec::new();
                for name in &objs {
                    let id = s.graph.find(name)?.and_then(|id| crate::components::as_mesh(&s.graph, id));
                    let ok = id.and_then(|id| s.graph.node(id)).is_some_and(|n| n.ty().is_a("mesh") && n.data().mesh(MeshNode::OUT_MESH).is_some());
                    if !ok {
                        fail!("polyBoolean: {name} is not a mesh");
                    }
                    nodes.push(id.unwrap());
                }
                let opts = BooleanOptions {
                    op,
                    cleanup: a.get("cleanup").and_then(Value::as_bool).unwrap_or(true),
                    snap_distance: a.float("snapDistance", -1.0),
                    split_ngons: a.get("splitNgons").and_then(Value::as_bool).unwrap_or(true),
                    quad_band: a.bool("quadBand"),
                    band_rings: a.int("bandRings", 1).clamp(0, 8) as i32,
                    band_timeout: a.float("timeout", 5.0).max(0.0),
                    fillet_width: a.float("filletWidth", 0.0).max(0.0),
                    fillet_segments: a.int("filletSegments", 2).clamp(1, 64) as i32,
                    hard_angle: a.float("hardAngle", 30.0),
                };
                if a.bool("live") {
                    return live_boolean(s, &nodes, a);
                }

                let r = {
                    let g = &s.graph;
                    let meshes: Vec<_> = nodes.iter().map(|&id| g.node(id).unwrap().data().mesh(MeshNode::OUT_MESH).unwrap().clone()).collect();
                    let worlds: Vec<DMat4> = nodes.iter().map(|&id| g.node(id).unwrap().matrix(Transform::WORLD_MATRIX)).collect();
                    let operands: Vec<BooleanOperand> =
                        meshes.iter().zip(&worlds).map(|(m, w)| BooleanOperand { mesh: m, to_world: *w }).collect();
                    mesh_boolean(&operands, &worlds[0].inverse(), &opts)
                };
                if !r.ok() {
                    let who = r.bad_operand.map(|i| format!("{} ", crate::components::display_path(&s.graph, nodes[i]))).unwrap_or_default();
                    fail!("polyBoolean: {who}{}", r.error);
                }

                let g = &mut s.graph;
                let base = modifiers::edit_plug(g, nodes[0])?;
                g.set_value(base, PlugValue::Mesh(Some(Arc::new(r.mesh))))?;
                for &n in &nodes[1..] {
                    if a.bool("keepCutters") {
                        g.set_value(PlugRef::new(n, Transform::VISIBILITY as i32), PlugValue::Bool(false))?;
                    } else {
                        g.delete_node(n)?;
                    }
                }
                if a.get("weightedNormals").and_then(Value::as_bool).unwrap_or(true) {
                    let has = modifiers::stack(g, nodes[0]).iter().any(|&m| g.node(m).is_some_and(|n| n.ty().name == "weightedNormalsModifier"));
                    if !has {
                        let path = crate::components::display_path(g, nodes[0]);
                        s.run("addModifier", json!({"args": [path], "kind": "weightedNormals"}))?;
                    }
                }
                s.graph.select(&[nodes[0]], SelectMode::Replace);
                Ok(Value::String(crate::components::display_path(&s.graph, nodes[0])))
            },
        )
        .positional("base cutters...")
        .flag(flag("operation", T::String, json!("difference"), "union, difference or intersection.").choices(&["union", "difference", "intersection"]))
        .flag(flag("keepCutters", T::Bool, json!(false), "Keep the cutter objects (hidden)."))
        .flag(flag(
            "live",
            T::Bool,
            json!(false),
            "Keep it live instead of baking: a boolean modifier per cutter, so moving or editing a cutter updates the cut. \
             The cutters go in a template display layer (BooleanCutters): grey wire you select from the outliner.",
        ))
        .flag(flag("cleanup", T::Bool, json!(true), "Weld slivers and stray points along the seam."))
        .flag(flag("snapDistance", T::Float, json!(-1.0), "Weld distance for cleanup, cm; negative is a tenth of the median seam edge."))
        .flag(flag("splitNgons", T::Bool, json!(true), "Cut the n-gons along the seam into quads (and triangles) that carry on the edge loops."))
        .flag(flag(
            "quadBand",
            T::Bool,
            json!(false),
            "Remesh a band of faces around the seam into quads that follow it, welded back into the faces around it. Falls \
             back to splitting n-gons where the band's border can't be kept.",
        ))
        .flag(flag("bandRings", T::Int, json!(1), "Rings of faces past the seam the quad band takes in.").min(0.0).max(8.0))
        .flag(flag("timeout", T::Float, json!(5.0), "Seconds the quad band remesh may take before the n-gon split runs instead; 0 waits forever.").min(0.0))
        .flag(flag("filletWidth", T::Float, json!(0.0), "Width of a rounded strip along the seam, cm; 0 keeps it sharp.").min(0.0))
        .flag(flag("filletSegments", T::Int, json!(2), "Rows of faces across the fillet.").min(1.0).max(64.0))
        .flag(flag("hardAngle", T::Float, json!(30.0), "New edges sharper than this many degrees are marked hard.").min(0.0).max(180.0))
        .flag(flag("weightedNormals", T::Bool, json!(true), "Add a weighted normals modifier, if the mesh has none, so the n-gons shade flat.")),
    );
    c.set_adjustable("polyBoolean", true);
}
