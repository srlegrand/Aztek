//! Weighted, locked and unlocked normals.

use aztek_geo::names;
use aztek_geo::normals::{apply_weighted_normals, WeightedNormalMode, WeightedNormalOptions};
use aztek_ops::{ArgType, CommandDef, Commands, FlagDef};
use serde_json::json;

use super::*;
use crate::components::ComponentMode as K;
use crate::session::Session;

type T = ArgType;
type Cmd = CommandDef<Session>;

fn weighted_op(a: &aztek_ops::Args, m: &mut aztek_geo::Mesh, sel: &crate::components::Ids, sym: Option<&aztek_geo::symmetry::SymmetryMap>) -> aztek_ops::CmdResult<EditResult> {
    let o = WeightedNormalOptions {
        mode: match a.string("mode", "area").as_str() {
            "angle" => WeightedNormalMode::CornerAngle,
            "areaAngle" => WeightedNormalMode::AreaAngle,
            _ => WeightedNormalMode::FaceArea,
        },
        dominance: a.float("dominance", 0.5),
        keep_sharp: a.bool("keepSharp"),
        use_face_strength: a.bool("faceStrength"),
    };
    let fs = with_mirror(m, sym, sel, K::Face);
    apply_weighted_normals(m, &o, &fs);
    Ok(EditResult::default())
}

fn unlock_op(_a: &aztek_ops::Args, m: &mut aztek_geo::Mesh, _sel: &crate::components::Ids, _sym: Option<&aztek_geo::symmetry::SymmetryMap>) -> aztek_ops::CmdResult<EditResult> {
    m.corner_attrs_mut().remove(names::NORMAL);
    Ok(EditResult::default())
}

pub fn register(c: &mut Commands<Session>) {
    use crate::history::{register_op, HistoryOp};
    let op = |node_type: &'static str, command: &'static str, label: &'static str, run: crate::history::OpFn, doc: &'static str| HistoryOp {
        node_type,
        command,
        label,
        kind: K::Face,
        run,
        skip_flags: &[],
        doc,
    };
    register_op(op("polyWeightedNormals", "polyWeightedNormals", "Weighted Normals", weighted_op, "Face-weighted locked normals."));
    register_op(op("polyUnlockNormals", "polyUnlockNormals", "Unlock Normals", unlock_op, "Drops stored split normals."));
    c.register(
        Cmd::new(
            "polyWeightedNormals",
            "Face-weighted normals on the selected faces (whole meshes in object mode): each smooth fan takes the normal of \
             its biggest faces, so chamfered or bevelled low poly shades its large faces flat and only the chamfers bend. \
             Written as locked split normals, which export to FBX. polyUnlockNormals goes back to computed ones.",
            |s, a| {
                edit_selected_op(s, "polyWeightedNormals", a, true, None)
            },
        )
        .flag(
            FlagDef::new("mode", "m", T::String, json!("area"), "What weighs a face: area, angle (its corner angle) or areaAngle.")
                .choices(&["area", "angle", "areaAngle"]),
        )
        .flag(
            FlagDef::new("dominance", "d", T::Float, json!(0.5), "Faces under this fraction of the heaviest face in a fan are left out; 0 keeps all.")
                .min(0.0)
                .max(1.0),
        )
        .flag(FlagDef::new("keepSharp", "ks", T::Bool, json!(true), "Keep hard edges split."))
        .flag(FlagDef::new(
            "faceStrength",
            "fs",
            T::Bool,
            json!(true),
            "Only the faces with the highest face int faceStrength count, when the mesh has it.",
        )),
    );

    c.register(Cmd::new(
        "polyLockNormals",
        "Store the current shading normals of the selected meshes as split normals, so later edits keep them.",
        |s, _a| {
            edit_selected_op(s, "polyNormalPerVertex", _a, true, None)
        },
    ));

    c.register(Cmd::new(
        "polyUnlockNormals",
        "Drop the stored split normals of the selected meshes, so shading comes from the hard and soft edges again.",
        |s, _a| {
            edit_selected_op(s, "polyUnlockNormals", _a, true, None)
        },
    ));
}
