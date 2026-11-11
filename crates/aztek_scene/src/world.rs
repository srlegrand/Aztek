//! The world's up axis: Y or Z, as the reference's Preferences > Settings > World Coordinate System sets it. It is a
//! preference, not part of a scene: a new scene or an opened one keeps it (a Z up scene file switches it on, through the
//! script node the reference writes into Z up scenes).
//!
//! Code that thinks in Y up (camera tumbling, the default views, the ground plane) works in a Y up frame and turns it
//! to the world with `to_world`: the identity when Y is up, a quarter turn about X when Z is up, which takes +Y to +Z
//! and +Z to -Y, as the reference's views turn with `upAxis -rv`.

use std::cell::Cell;

use glam::{DMat3, DVec3};

thread_local! {
    // Per thread, as the current units are: the session's thread holds the application's.
    static UP_Z: Cell<bool> = const { Cell::new(false) };
}

/// Whether Z is up.
pub fn up_z() -> bool {
    UP_Z.with(Cell::get)
}

pub fn set_up_z(z: bool) {
    UP_Z.with(|c| c.set(z));
}

/// "y" or "z".
pub fn up_name() -> &'static str {
    if up_z() {
        "z"
    } else {
        "y"
    }
}

/// World up.
pub fn up() -> DVec3 {
    if up_z() {
        DVec3::Z
    } else {
        DVec3::Y
    }
}

/// The turn from the Y up frame to the world.
pub fn frame() -> DMat3 {
    if up_z() {
        DMat3::from_rotation_x(std::f64::consts::FRAC_PI_2)
    } else {
        DMat3::IDENTITY
    }
}

/// A Y up frame direction or point in the world: (x, y, z) is (x, -z, y) when Z is up.
pub fn to_world(v: DVec3) -> DVec3 {
    if up_z() {
        DVec3::new(v.x, -v.z, v.y)
    } else {
        v
    }
}

/// A world direction or point in the Y up frame.
pub fn from_world(v: DVec3) -> DVec3 {
    if up_z() {
        DVec3::new(v.x, v.z, -v.y)
    } else {
        v
    }
}

/// The index of the up axis in a vector: 1 or 2.
pub fn up_index() -> usize {
    if up_z() {
        2
    } else {
        1
    }
}
