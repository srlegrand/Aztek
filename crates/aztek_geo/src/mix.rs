//! glm's `mix`, x * (1 - a) + y * a, kept to that form so results are stable to the last bit.

use glam::{DVec3, Vec2};

pub fn mixd(x: f64, y: f64, a: f64) -> f64 {
    x * (1.0 - a) + y * a
}
pub fn mixf(x: f32, y: f32, a: f32) -> f32 {
    x * (1.0 - a) + y * a
}
pub fn mix3(x: DVec3, y: DVec3, a: f64) -> DVec3 {
    x * (1.0 - a) + y * a
}
pub fn mix2(x: Vec2, y: Vec2, a: f32) -> Vec2 {
    x * (1.0 - a) + y * a
}
