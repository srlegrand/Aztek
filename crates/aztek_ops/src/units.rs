//! The current units and the boundary between them and what's stored. Everything inside is centimetres and radians, as
//! in the reference. A command called from a script or the interface is in the current units: `execute` converts its
//! flags and results by their unit tags. Commands called by other commands, and the Python API, are in stored units.

use std::cell::Cell;
use std::f64::consts::PI;

use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum Unit {
    #[default]
    None,
    /// Centimetres stored; shown in `currentUnit -linear`.
    Linear,
    /// Radians stored; shown in `currentUnit -angle`.
    Angle,
    /// An area: stored in square centimetres.
    Area,
    /// A volume: stored in cubic centimetres.
    Volume,
}

thread_local! {
    static CM_PER_UNIT: Cell<f64> = const { Cell::new(1.0) };
    static RAD_PER_UNIT: Cell<f64> = const { Cell::new(PI / 180.0) };
    static DEPTH: Cell<u32> = const { Cell::new(0) };
    static USER_CALL: Cell<bool> = const { Cell::new(false) };
}

pub const LINEAR_UNITS: &[(&str, &str, f64)] = &[
    ("mm", "millimeter", 0.1),
    ("cm", "centimeter", 1.0),
    ("m", "meter", 100.0),
    ("km", "kilometer", 100000.0),
    ("in", "inch", 2.54),
    ("ft", "foot", 30.48),
    ("yd", "yard", 91.44),
    ("mi", "mile", 160934.4),
];

/// Centimetres in a linear unit, by either of its names.
pub fn linear_factor(name: &str) -> Option<f64> {
    LINEAR_UNITS.iter().find(|u| u.0 == name || u.1 == name).map(|u| u.2)
}

/// Radians in an angle unit, by any of its names.
pub fn angle_factor(name: &str) -> Option<f64> {
    match name {
        "deg" | "degree" => Some(PI / 180.0),
        "rad" | "radian" => Some(1.0),
        "min" | "minute" => Some(PI / 10800.0),
        "sec" | "second" => Some(PI / 648000.0),
        _ => None,
    }
}

pub fn set_linear(cm_per_unit: f64) {
    CM_PER_UNIT.with(|c| c.set(cm_per_unit));
}

pub fn set_angle(rad_per_unit: f64) {
    RAD_PER_UNIT.with(|c| c.set(rad_per_unit));
}

pub fn cm_per_unit() -> f64 {
    CM_PER_UNIT.with(Cell::get)
}

pub fn rad_per_unit() -> f64 {
    RAD_PER_UNIT.with(Cell::get)
}

/// The short name of the current linear unit.
pub fn linear_name() -> &'static str {
    let f = cm_per_unit();
    LINEAR_UNITS.iter().find(|u| (u.2 - f).abs() < 1e-12).map_or("cm", |u| u.0)
}

/// The short name of the current angle unit.
pub fn angle_name() -> &'static str {
    let f = rad_per_unit();
    if (f - 1.0).abs() < 1e-15 {
        "rad"
    } else if (f - PI / 10800.0).abs() < 1e-15 {
        "min"
    } else if (f - PI / 648000.0).abs() < 1e-15 {
        "sec"
    } else {
        "deg"
    }
}

pub fn linear_full_name() -> &'static str {
    let n = linear_name();
    LINEAR_UNITS.iter().find(|u| u.0 == n).map_or("centimeter", |u| u.1)
}

pub fn angle_full_name() -> &'static str {
    match angle_name() {
        "rad" => "radian",
        "min" => "minute",
        "sec" => "second",
        _ => "degree",
    }
}

/// What one current unit is in stored units.
pub fn factor(unit: Unit) -> f64 {
    match unit {
        Unit::None => 1.0,
        Unit::Linear => cm_per_unit(),
        Unit::Angle => rad_per_unit(),
        Unit::Area => cm_per_unit().powi(2),
        Unit::Volume => cm_per_unit().powi(3),
    }
}

pub fn to_stored(unit: Unit, v: f64) -> f64 {
    v * factor(unit)
}

pub fn to_user(unit: Unit, v: f64) -> f64 {
    tidy(unit, v / factor(unit))
}

/// A number leaving radians picks up noise in its last digits (29.999999999999996 for 30): people read degrees.
fn tidy(unit: Unit, v: f64) -> f64 {
    if unit == Unit::Angle {
        let r = (v * 1e9).round() / 1e9;
        if (r - v).abs() < 1e-12 {
            return r;
        }
    }
    v
}

/// Scales every number in a value, through arrays; strings and the rest stay.
pub fn scale_json(v: &mut Value, by: f64) {
    if by == 1.0 {
        return;
    }
    match v {
        Value::Number(n) => {
            if let Some(x) = n.as_f64() {
                if let Some(y) = serde_json::Number::from_f64(x * by) {
                    *n = y;
                }
            }
        }
        Value::Array(a) => a.iter_mut().for_each(|x| scale_json(x, by)),
        _ => {}
    }
}

pub fn json_to_stored(unit: Unit, v: &mut Value) {
    scale_json(v, factor(unit));
}

pub fn json_to_user(unit: Unit, v: &mut Value) {
    if unit == Unit::None {
        return;
    }
    match v {
        Value::Number(n) => {
            if let Some(y) = n.as_f64().and_then(|x| serde_json::Number::from_f64(to_user(unit, x))) {
                *n = y;
            }
        }
        Value::Array(a) => a.iter_mut().for_each(|x| json_to_user(unit, x)),
        _ => {}
    }
}

/// Enters a command. True when this one takes and gives current units: nothing outside it but a script.
pub(crate) fn enter() -> (bool, u32, bool) {
    let d = DEPTH.with(Cell::get);
    DEPTH.with(|c| c.set(d + 1));
    let user = d == 0;
    (user, d, USER_CALL.with(|c| c.replace(user)))
}

pub(crate) fn leave(depth: u32, was: bool) {
    DEPTH.with(|c| c.set(depth));
    USER_CALL.with(|c| c.set(was));
}

/// Whether the running command was called from a script or the interface, and so answers in current units.
pub fn user_call() -> bool {
    USER_CALL.with(Cell::get)
}

/// Commands in `f` are a script's: in current units, even when a command is running around them (a plug-in command's
/// body, a callback).
pub fn as_script<R>(f: impl FnOnce() -> R) -> R {
    let d = DEPTH.with(|c| c.replace(0));
    let was = USER_CALL.with(Cell::get);
    let r = f();
    leave(d, was);
    r
}

/// Commands in `f` are in stored units: the Python API, viewport tools, the file reader.
pub fn stored<R>(f: impl FnOnce() -> R) -> R {
    let d = DEPTH.with(|c| {
        let d = c.get();
        c.set(d + 1);
        d
    });
    let was = USER_CALL.with(|c| c.replace(false));
    let r = f();
    leave(d, was);
    r
}

/// Whether commands called now are in stored units, rather than a script's.
pub fn in_stored() -> bool {
    DEPTH.with(Cell::get) > 0
}

thread_local! {
    // The units a new scene starts in: File > New Scene's options, centimetres and degrees unless changed.
    static DEFAULT_CM: Cell<f64> = const { Cell::new(1.0) };
    static DEFAULT_RAD: Cell<f64> = const { Cell::new(PI / 180.0) };
}

/// The units a new scene starts in, as factors (centimetres per unit, radians per unit). A factor of 0 leaves that one.
pub fn set_defaults(cm_per_unit: f64, rad_per_unit: f64) {
    if cm_per_unit > 0.0 {
        DEFAULT_CM.with(|c| c.set(cm_per_unit));
    }
    if rad_per_unit > 0.0 {
        DEFAULT_RAD.with(|c| c.set(rad_per_unit));
    }
}

pub fn default_cm_per_unit() -> f64 {
    DEFAULT_CM.with(Cell::get)
}

pub fn default_rad_per_unit() -> f64 {
    DEFAULT_RAD.with(Cell::get)
}

/// Back to the defaults, for a new scene.
pub fn reset() {
    set_linear(default_cm_per_unit());
    set_angle(default_rad_per_unit());
}
