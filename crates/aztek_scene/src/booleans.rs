//! polyBoolean and the live boolean modifier.

use std::sync::Arc;

use aztek_geo::Mesh;
use aztek_ops::{fail, objects, ArgType as T, CommandDef, Commands, FlagDef};
use serde_json::{json, Value};

use crate::attr_defs::{matrix_attr, mesh_attr, param, param_enum};
use crate::graph::SelectMode;
use crate::mesh_boolean::{mesh_boolean, BooleanOp, BooleanOperand, BooleanOptions};
use crate::modifiers::ModifierNode;
use crate::node::{types, NodeData, NodeType};
use crate::plug::{flags, AttrDef, PlugType, PlugValue};
use crate::session::Session;

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

fn flag(name: &str, ty: T, default: Value, doc: &str) -> FlagDef {
    FlagDef::new(name, "", ty, default, doc)
}

pub fn register(c: &mut Commands<Session>) {
    c.register(
        CommandDef::<Session>::new(
            "polyBoolean",
            "Union, difference or intersection of meshes as a live polyCBoolOp node, as the reference's Mesh > Booleans: \
             moving or editing an input updates the result, and the inputs stay as hidden shapes feeding it. The seam is \
             cleaned up (Aztek's addition, settings on the node): slivers welded, the n-gons along the cut split into quads \
             that carry on the edge loops, an optional rounded fillet, and weighted normals (a polyWeightedNormals node). \
             Returns [transform, polyCBoolOp].",
            |s, a| {
                let objs = objects(s, a);
                if objs.len() < 2 {
                    fail!("polyBoolean: give a base and at least one cutter");
                }
                let op = match a.string("operation", "difference").as_str() {
                    "union" => 1,
                    "difference" => 2,
                    "intersection" => 3,
                    _ => fail!("polyBoolean: operation must be union, difference or intersection"),
                };
                let shapes = crate::mesh_edit::ref_poly::mesh_cmds::shapes_named(s, a, "polyBoolean")?;
                let made = crate::mesh_edit::ref_poly::mesh_cmds::multi_input(s, a, "polyCBoolOp", &shapes, op, "polySurface1")?;
                if a.get("weightedNormals").and_then(Value::as_bool).unwrap_or(true) {
                    if let Some(id) = made.get(0).and_then(Value::as_str).and_then(|xf| s.graph.find(xf).ok().flatten()) {
                        s.graph.select(&[id], SelectMode::Replace);
                        s.run("polyWeightedNormals", json!({}))?;
                        s.graph.select(&[id], SelectMode::Replace);
                    }
                }
                Ok(made)
            },
        )
        .positional("base cutters...")
        .flag(flag("operation", T::String, json!("difference"), "union, difference or intersection.").choices(&["union", "difference", "intersection"]))
        .flag(flag("keepCutters", T::Bool, json!(false), "Accepted for existing scripts: the inputs are always kept, hidden, feeding the node."))
        .flag(flag("live", T::Bool, json!(true), "Accepted for existing scripts: the result is always live."))
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
