//! Transform edits shared by the manipulators and the move/rotate/scale
//! commands. All record undo through the graph.
//!
//! `rotate_by` and `rotated_euler` take the rotation as a matrix rather than glm's quaternion. `bakeIntoGeometry`
//! waits for meshes.

use std::collections::HashSet;

use glam::{DMat3, DVec3, DVec4};

use crate::error::Result;
use crate::graph::Graph;
use crate::plug::{NodeId, PlugRef, NO_NODE};
use crate::transform::{euler_to_matrix, matrix_to_euler, RotateOrder, Transform};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Space {
    World,
    Object,
    Parent,
}

fn v3(g: &Graph, id: NodeId, attr: usize) -> DVec3 {
    g.node(id).unwrap().vec3(attr)
}

fn orthonormal(m: &DMat3) -> DMat3 {
    let x = m.x_axis.normalize();
    let y = m.y_axis - x * x.dot(m.y_axis);
    let y = if y.length() > 1e-12 {
        y.normalize()
    } else {
        DVec3::Y
    };
    let mut z = x.cross(y);
    // Mirrored frames (negative scale) keep their handedness out of the rotation.
    if z.dot(m.z_axis) < 0.0 {
        z = -z;
    }
    DMat3::from_cols(x, y, z)
}

fn wrap_toward(a: f64, hint: f64) -> f64 {
    a + 360.0 * ((hint - a) / 360.0).round()
}

fn rotation_of(g: &Graph, id: NodeId) -> DMat3 {
    let n = g.node(id).unwrap();
    DMat3::from_mat4(euler_to_matrix(
        n.vec3(Transform::ROTATE),
        RotateOrder::from_index(n.int(Transform::ROTATE_ORDER)),
    ))
}

pub(crate) fn pre_rotation(g: &Graph, id: NodeId) -> DMat3 {
    let n = g.node(id).unwrap();
    n.behavior()
        .as_transform()
        .map(|t| DMat3::from_mat4(t.pre_rotation(n.data())))
        .unwrap_or(DMat3::IDENTITY)
}

pub(crate) fn rotate_axis(g: &Graph, id: NodeId) -> DMat3 {
    let n = g.node(id).unwrap();
    n.behavior()
        .as_transform()
        .map(|t| DMat3::from_mat4(t.rotate_axis_matrix(n.data())))
        .unwrap_or(DMat3::IDENTITY)
}

fn plug(id: NodeId, attr: usize) -> PlugRef {
    PlugRef::new(id, attr as i32)
}

/// Drops ids whose ancestor is also in the list, so a selected parent and child move once.
pub fn top_level(g: &Graph, ids: &[NodeId]) -> Vec<NodeId> {
    let set: HashSet<NodeId> = ids.iter().copied().collect();
    ids.iter()
        .copied()
        .filter(|&id| {
            if !g.node(id).is_some_and(|n| n.ty().is_a("transform")) {
                return false;
            }
            let mut p = g.parent(id);
            while p != NO_NODE {
                if set.contains(&p) {
                    return false;
                }
                p = g.parent(p);
            }
            true
        })
        .collect()
}

/// World position of the rotate pivot.
pub fn world_pivot(g: &mut Graph, id: NodeId) -> DVec3 {
    g.evaluate();
    let n = g.node(id).unwrap();
    let local = n.vec3(Transform::TRANSLATE)
        + n.vec3(Transform::ROTATE_PIVOT_TRANSLATE)
        + n.vec3(Transform::ROTATE_PIVOT);
    (n.matrix(Transform::PARENT_MATRIX) * DVec4::from((local, 1.0))).truncate()
}

/// Orthonormal world-space axes (columns) for a space.
pub fn frame(g: &mut Graph, id: NodeId, space: Space) -> DMat3 {
    if space == Space::World {
        return DMat3::IDENTITY;
    }
    g.evaluate();
    let n = g.node(id).unwrap();
    let attr = if space == Space::Parent {
        Transform::PARENT_MATRIX
    } else {
        Transform::WORLD_MATRIX
    };
    orthonormal(&DMat3::from_mat4(n.matrix(attr)))
}

pub fn move_by(g: &mut Graph, ids: &[NodeId], delta: DVec3, space: Space) -> Result<()> {
    g.evaluate();
    for id in top_level(g, ids) {
        let world = frame(g, id, space) * delta;
        let n = g.node(id).unwrap();
        let local =
            (n.matrix(Transform::PARENT_MATRIX).inverse() * DVec4::from((world, 0.0))).truncate();
        let p = plug(id, Transform::TRANSLATE);
        let v = g.keep_locked(&p, (n.vec3(Transform::TRANSLATE) + local).into());
        g.set_value(p, v)?;
    }
    Ok(())
}

/// The rotate channels `rotate_by` would give one object, before its locked channels are kept.
pub fn rotated_euler(g: &mut Graph, id: NodeId, r: &DMat3, space: Space) -> DVec3 {
    g.evaluate();
    let n = g.node(id).unwrap();
    let parent = orthonormal(&DMat3::from_mat4(n.matrix(Transform::PARENT_MATRIX)));
    let current = rotation_of(g, id);
    let next = if space == Space::Object {
        // The object's own axes include rotateAxis, which sits after R.
        let ra = rotate_axis(g, id);
        current * ra * *r * ra.transpose()
    } else {
        // Changing R alone turns the object about its rotate pivot, which is exactly what we want.
        let world_r = if space == Space::Parent {
            parent * *r * parent.transpose()
        } else {
            *r
        };
        // Joints rotate inside their jointOrient, so that frame sits between the parent and R.
        let f = parent * pre_rotation(g, id);
        f.transpose() * world_r * f * current
    };
    let order = RotateOrder::from_index(n.int(Transform::ROTATE_ORDER));
    closest_euler(&next, order, n.vec3(Transform::ROTATE))
}

/// Rotates each object about its own pivot. `r` is in `space` (world, or each object's own frame).
pub fn rotate_by(g: &mut Graph, ids: &[NodeId], r: &DMat3, space: Space) -> Result<()> {
    g.evaluate();
    for id in top_level(g, ids) {
        let e = rotated_euler(g, id, r, space);
        let p = plug(id, Transform::ROTATE);
        let v = g.keep_locked(&p, e.into());
        g.set_value(p, v)?;
    }
    Ok(())
}

/// Multiplies local scale; scaling is always along the object's own axes.
pub fn scale_by(g: &mut Graph, ids: &[NodeId], factor: DVec3) -> Result<()> {
    for id in top_level(g, ids) {
        let p = plug(id, Transform::SCALE);
        let v = g.keep_locked(&p, (v3(g, id, Transform::SCALE) * factor).into());
        g.set_value(p, v)?;
    }
    Ok(())
}

/// Moves the rotate and scale pivots to a world point without moving the object.
pub fn set_world_pivot(g: &mut Graph, id: NodeId, world: DVec3) -> Result<()> {
    g.evaluate();
    let t = v3(g, id, Transform::TRANSLATE);
    let rpt = v3(g, id, Transform::ROTATE_PIVOT_TRANSLATE);
    let rp = v3(g, id, Transform::ROTATE_PIVOT);
    let sp = v3(g, id, Transform::SCALE_PIVOT);
    let spt = v3(g, id, Transform::SCALE_PIVOT_TRANSLATE);
    let s = v3(g, id, Transform::SCALE);
    let r = pre_rotation(g, id) * rotation_of(g, id) * rotate_axis(g, id);

    let pm = g.node(id).unwrap().matrix(Transform::PARENT_MATRIX);
    let target = (pm.inverse() * DVec4::from((world, 1.0))).truncate();
    // Pivot sits at T + Rpt + Rp in parent space; solve for Rp with Rpt compensating so the matrix is unchanged.
    let new_rp = r.transpose() * (target - t - rpt - rp + r * rp);
    let new_rpt = rpt + (rp - new_rp) - r * (rp - new_rp);

    // The scale pivot is the pre-scale point that scales onto the new rotate pivot, so scaling holds the pivot still.
    let mut new_sp = sp;
    for i in 0..3 {
        if s[i].abs() > 1e-12 {
            new_sp[i] = sp[i] + (new_rp[i] - sp[i] - spt[i]) / s[i];
        }
    }
    let new_spt = spt + (sp - new_sp) - s * (sp - new_sp);

    g.set_value(plug(id, Transform::ROTATE_PIVOT), new_rp.into())?;
    g.set_value(plug(id, Transform::ROTATE_PIVOT_TRANSLATE), new_rpt.into())?;
    g.set_value(plug(id, Transform::SCALE_PIVOT), new_sp.into())?;
    g.set_value(plug(id, Transform::SCALE_PIVOT_TRANSLATE), new_spt.into())
}

/// World axes (columns) of the pivot: the object's axes turned by its pivotOrient.
pub fn pivot_frame(g: &mut Graph, id: NodeId) -> DMat3 {
    let object = frame(g, id, Space::Object);
    object
        * DMat3::from_mat4(euler_to_matrix(
            v3(g, id, Transform::PIVOT_ORIENT),
            RotateOrder::Xyz,
        ))
}

/// Sets pivotOrient so the pivot's world axes become `world` (orthonormal columns), without moving the object.
pub fn set_world_pivot_frame(g: &mut Graph, id: NodeId, world: &DMat3) -> Result<()> {
    let object = frame(g, id, Space::Object);
    let local = object.transpose() * *world;
    let e = closest_euler(&local, RotateOrder::Xyz, v3(g, id, Transform::PIVOT_ORIENT));
    g.set_value(plug(id, Transform::PIVOT_ORIENT), e.into())
}

/// Zeroes pivots, their compensation and the pivot orientation, keeping the object where it is.
pub fn reset_pivot(g: &mut Graph, id: NodeId) -> Result<()> {
    g.evaluate();
    let local = g.node(id).unwrap().matrix(Transform::MATRIX);
    g.set_value(plug(id, Transform::PIVOT_ORIENT), DVec3::ZERO.into())?;
    for a in [
        Transform::ROTATE_PIVOT,
        Transform::ROTATE_PIVOT_TRANSLATE,
        Transform::SCALE_PIVOT,
        Transform::SCALE_PIVOT_TRANSLATE,
    ] {
        g.set_value(plug(id, a), DVec3::ZERO.into())?;
    }
    g.evaluate();
    // With pivots gone the old matrix is plain TRS again, so translation is all that needs restoring.
    let now = Transform::local_matrix(g.node(id).unwrap());
    let t = v3(g, id, Transform::TRANSLATE) + local.w_axis.truncate() - now.w_axis.truncate();
    g.set_value(plug(id, Transform::TRANSLATE), t.into())
}

/// Euler angles for a rotation that are closest to `hint`, so keys don't flip by 360.
pub fn closest_euler(rotation: &DMat3, order: RotateOrder, hint: DVec3) -> DVec3 {
    let mut a = matrix_to_euler(rotation, order);
    // Every rotation has a second Euler solution: first and last axes +180, middle mirrored about 180.
    const AXES: [[usize; 3]; 6] = [
        [0, 1, 2],
        [1, 2, 0],
        [2, 0, 1],
        [0, 2, 1],
        [1, 0, 2],
        [2, 1, 0],
    ];
    let ax = AXES[order as usize];
    let mut b = a;
    b[ax[0]] += 180.0;
    b[ax[1]] = 180.0 - b[ax[1]];
    b[ax[2]] += 180.0;
    let fit = |mut e: DVec3| {
        for i in 0..3 {
            e[i] = wrap_toward(e[i], hint[i]);
        }
        e
    };
    a = fit(a);
    b = fit(b);
    let check = DMat3::from_mat4(euler_to_matrix(b, order));
    let err: f64 = (0..3)
        .map(|c| (check.col(c) - rotation.col(c)).length())
        .sum();
    if err > 1e-6 {
        return a;
    }
    if (b - hint).length() < (a - hint).length() {
        b
    } else {
        a
    }
}
