//! Repeating an object by a fixed step, for duplicate count=, Duplicate with
//! Transform and the array tools. The step and run maths are aztek_geo::array's, shared with the array modifier and
//! the live array group. Matrices are world space, column vectors.

use aztek_geo::array::{about_frame, pivot_frame, repeat_matrices, trs_matrix};
use glam::{DMat3, DMat4, DVec3};

use crate::error::Result;
use crate::graph::Graph;
use crate::plug::{NodeId, PlugRef, PlugValue, NO_NODE};
use crate::transform::{decompose, RotateOrder, Transform};
use crate::transform_ops::{closest_euler, pre_rotation, rotate_axis};

/// One step: translate (cm), rotate (degrees, XYZ order) and scale, applied scale first, then rotate, then translate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Step {
    pub translate: DVec3,
    pub rotate: DVec3,
    pub scale: DVec3,
}

impl Default for Step {
    fn default() -> Self {
        Self { translate: DVec3::ZERO, rotate: DVec3::ZERO, scale: DVec3::ONE }
    }
}

impl Step {
    pub fn identity(&self) -> bool {
        self.translate == DVec3::ZERO && self.rotate == DVec3::ZERO && self.scale == DVec3::ONE
    }
}

/// The world matrix of one step about `pivot`, its axes along the columns of `frame` (identity for world axes).
pub fn step_matrix(s: &Step, pivot: DVec3, frame: &DMat3) -> DMat4 {
    about_frame(&pivot_frame(pivot, frame), &trs_matrix(s.translate, s.rotate * (180.0 / std::f64::consts::PI), s.scale))
}

/// World matrices of copies 1..count of an object whose world matrix is `base`: copy i is step^i * base.
pub fn copy_matrices(base: &DMat4, step: &DMat4, count: i32) -> Vec<DMat4> {
    repeat_matrices(base, step, count)
}

/// The step that takes world matrix `from` to `to`: to = delta * from.
pub fn delta(from: &DMat4, to: &DMat4) -> DMat4 {
    *to * from.inverse()
}

/// Sets translate, rotate and scale so the node's world matrix becomes `world`, keeping its pivots, locked channels
/// included. The rotate channels come out nearest `rotate_hint` (default: their current value). Undoable.
pub fn set_world_matrix(g: &mut Graph, id: NodeId, world: &DMat4, rotate_hint: Option<DVec3>) -> Result<()> {
    if !g.node(id).is_some_and(|n| n.ty().is_a("transform")) {
        return Ok(());
    }
    let parent = g.parent(id);
    let parent_world = if parent == NO_NODE {
        DMat4::IDENTITY
    } else {
        match g.value(&PlugRef::new(parent, Transform::WORLD_MATRIX as i32))? {
            PlugValue::Matrix(m) => m,
            _ => DMat4::IDENTITY,
        }
    };
    let m = parent_world.inverse() * *world;

    // Placing a copy is not a channel edit, so locked channels (copied from its source) take the new value too.
    let mut relock = Vec::new();
    for attr in [Transform::TRANSLATE, Transform::ROTATE, Transform::SCALE] {
        for c in 0..3 {
            let p = PlugRef { component: c, ..PlugRef::new(id, attr as i32) };
            if g.is_locked(&p) {
                g.set_locked(&p, false)?;
                relock.push(p);
            }
        }
    }

    let (scale, rotation, _) = decompose(&m);
    let n = g.node(id).unwrap();
    let order = RotateOrder::from_index(n.int(Transform::ROTATE_ORDER));
    // Nearest the expected channels, so stepping past 180 keeps counting instead of flipping.
    let hint = rotate_hint.unwrap_or(n.vec3(Transform::ROTATE));
    let (pre, axis) = (pre_rotation(g, id), rotate_axis(g, id));
    let euler = closest_euler(&(pre.transpose() * rotation * axis.transpose()), order, hint);
    g.set_value(PlugRef::new(id, Transform::ROTATE as i32), euler.into())?;
    g.set_value(PlugRef::new(id, Transform::SCALE as i32), scale.into())?;

    let n = g.node(id).unwrap();
    let without_t = Transform::local_matrix(n);
    let current = n.vec3(Transform::TRANSLATE);
    let offset = without_t.w_axis.truncate() - current;
    g.set_value(PlugRef::new(id, Transform::TRANSLATE as i32), (m.w_axis.truncate() - offset).into())?;
    for p in relock {
        g.set_locked(&p, true)?;
    }
    Ok(())
}
