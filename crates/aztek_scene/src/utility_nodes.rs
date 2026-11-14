//! The reference's utility nodes, from one table: each has its attributes, with the reference's long and short names, and a compute.
//! They create, connect, key, undo and save like any other node. Angles are degrees, as everywhere in Aztek, so
//! `angleBetween.angle` and `quatToEuler.outputRotate` read in degrees and no `unitConversion` is ever inserted.

use std::any::Any;
use std::sync::Arc;

use glam::{DMat3, DMat4, DQuat, DVec3};

use crate::graph::Graph;
use crate::node::{types, EvalContext, NodeBehavior, NodeData, NodeType};
use crate::plug::{flags, AttrDef, NodeId, PlugRef, PlugType, PlugValue};
use crate::transform::{decompose_full, euler_to_matrix, matrix_to_euler, shear_matrix, RotateOrder};

const IN: u32 = flags::INPUT | flags::STORABLE | flags::KEYABLE;
const STORE: u32 = flags::INPUT | flags::STORABLE;
const OUT: u32 = flags::OUTPUT;

const XYZ: [(&str, &str); 3] = [("X", "x"), ("Y", "y"), ("Z", "z")];
const RGB: [(&str, &str); 3] = [("R", "r"), ("G", "g"), ("B", "b")];
const HSV: [(&str, &str); 3] = [("H", "h"), ("S", "s"), ("V", "v")];
const LOWER: [(&str, &str); 3] = [("x", "x"), ("y", "y"), ("z", "z")];

fn base(name: &str, short: &str, ty: PlugType, default: PlugValue, fl: u32) -> AttrDef {
    AttrDef { name: name.into(), short_name: short.into(), ty, default_value: default, flags: fl, ..Default::default() }
}

fn generic(mut a: AttrDef) -> AttrDef {
    a.flags |= flags::GENERIC;
    a
}

fn fin(name: &str, short: &str, d: f64) -> AttrDef {
    base(name, short, PlugType::Float, PlugValue::Float(d), IN)
}
fn fout(name: &str, short: &str) -> AttrDef {
    base(name, short, PlugType::Float, PlugValue::Float(0.0), OUT)
}
fn bin(name: &str, short: &str, d: bool) -> AttrDef {
    base(name, short, PlugType::Bool, PlugValue::Bool(d), IN)
}
fn bout(name: &str, short: &str) -> AttrDef {
    base(name, short, PlugType::Bool, PlugValue::Bool(false), OUT)
}
fn iin(name: &str, short: &str, d: i64) -> AttrDef {
    base(name, short, PlugType::Int, PlugValue::Int(d), IN)
}
fn iout(name: &str, short: &str) -> AttrDef {
    base(name, short, PlugType::Int, PlugValue::Int(0), OUT)
}
fn min_in(name: &str, short: &str) -> AttrDef {
    base(name, short, PlugType::Matrix, PlugValue::Matrix(DMat4::IDENTITY), STORE)
}
fn mout(name: &str, short: &str) -> AttrDef {
    base(name, short, PlugType::Matrix, PlugValue::Matrix(DMat4::IDENTITY), OUT)
}
fn en(name: &str, short: &str, d: i64, names: &[&str]) -> AttrDef {
    let mut a = base(name, short, PlugType::Enum, PlugValue::Int(d), STORE | flags::KEYABLE);
    a.enum_names = names.iter().map(|s| s.to_string()).collect();
    a
}
const PAIR_MODES: [&str; 3] = ["Blend Inputs", "Input 1 only", "Input 2 only"];
const ORDERS: [&str; 6] = ["xyz", "yzx", "zxy", "xzy", "yxz", "zyx"];

fn vec_kids(name: &str, short: &str, d: [f64; 3], fl: u32, kids: [(&str, &str); 3]) -> AttrDef {
    let mut a = base(name, short, PlugType::Vec3, PlugValue::Vec3(DVec3::from_array(d)), fl);
    for (i, (l, s)) in kids.into_iter().enumerate() {
        a.child_names[i] = format!("{name}{l}");
        a.child_short[i] = format!("{short}{s}");
    }
    a
}
/// pairBlend's inputs, whose children carry the number last: inTranslateX1, itx1.
fn numbered(name: &str, short: &str, k: u32) -> AttrDef {
    let mut a = vec_kids(&format!("{name}{k}"), &format!("{short}{k}"), [0.0; 3], IN, XYZ);
    for (i, axis) in ["X", "Y", "Z"].into_iter().enumerate() {
        a.child_names[i] = format!("{name}{axis}{k}");
        a.child_short[i] = format!("{short}{}{k}", axis.to_lowercase());
    }
    a
}
fn vin(name: &str, short: &str, d: [f64; 3]) -> AttrDef {
    vec_kids(name, short, d, IN, XYZ)
}
fn vout(name: &str, short: &str) -> AttrDef {
    vec_kids(name, short, [0.0; 3], OUT, XYZ)
}
fn cin(name: &str, short: &str, d: [f64; 3]) -> AttrDef {
    vec_kids(name, short, d, IN | flags::COLOR, RGB)
}
fn cout(name: &str, short: &str) -> AttrDef {
    vec_kids(name, short, [0.0; 3], OUT | flags::COLOR, RGB)
}
/// A Vec3 whose children's short names don't follow from its own.
fn kids_named(mut a: AttrDef, short: [&str; 3]) -> AttrDef {
    a.child_short = short.map(str::to_string);
    a
}
/// Two children: input2Dx, input2Dy. Held in a Vec3 whose third child has no name.
fn two(mut a: AttrDef) -> AttrDef {
    a.child_names[2].clear();
    a.child_short[2].clear();
    a
}
fn multi(mut a: AttrDef) -> AttrDef {
    a.flags |= flags::ARRAY;
    a
}
/// A child of a multi compound: value[3].value_Position.
fn kid(group: &str, gshort: &str, mut a: AttrDef) -> AttrDef {
    a.name = format!("{group}.{}", a.name);
    a.short_name = format!("{gshort}.{}", a.short_name);
    a.flags |= flags::ARRAY;
    a
}
/// A four part compound (quaternions): its children are plain floats sharing the compound's name as their category.
fn quat(name: &str, short: &str, fl: u32) -> Vec<AttrDef> {
    ["X", "Y", "Z", "W"]
        .into_iter()
        .map(|c| {
            let lower = c.to_lowercase();
            let mut a = base(&format!("{name}{c}"), &format!("{short}{lower}"), PlugType::Float, PlugValue::Float(if c == "W" { 1.0 } else { 0.0 }), fl);
            a.category = name.to_string();
            a
        })
        .collect()
}

fn at(d: &NodeData, n: &str) -> usize {
    d.ty().attr_index(n).unwrap_or_else(|| panic!("{} has no {n}", d.ty().name))
}
fn f(d: &NodeData, n: &str) -> f64 {
    d.value(at(d, n)).scalar().unwrap_or(0.0)
}
fn i(d: &NodeData, n: &str) -> i64 {
    f(d, n) as i64
}
fn b(d: &NodeData, n: &str) -> bool {
    f(d, n) != 0.0
}
fn v(d: &NodeData, n: &str) -> DVec3 {
    d.vec3(at(d, n))
}
fn m(d: &NodeData, n: &str) -> DMat4 {
    d.matrix(at(d, n))
}
fn set(d: &mut NodeData, n: &str, x: impl Into<PlugValue>) {
    let a = at(d, n);
    d.set_output(a, x);
}
fn get_quat(d: &NodeData, n: &str) -> DQuat {
    DQuat::from_xyzw(f(d, &format!("{n}X")), f(d, &format!("{n}Y")), f(d, &format!("{n}Z")), f(d, &format!("{n}W")))
}
fn set_quat(d: &mut NodeData, n: &str, q: DQuat) {
    for (c, x) in [("X", q.x), ("Y", q.y), ("Z", q.z), ("W", q.w)] {
        set(d, &format!("{n}{c}"), x);
    }
}
fn scalars(d: &NodeData, n: &str) -> Vec<f64> {
    d.elements(at(d, n)).iter().filter_map(|(_, x)| x.scalar()).collect()
}
fn vecs(d: &NodeData, n: &str) -> Vec<DVec3> {
    d.elements(at(d, n)).iter().map(|(_, x)| x.as_vec3()).collect()
}
fn mats(d: &NodeData, n: &str) -> Vec<DMat4> {
    d.elements(at(d, n)).iter().map(|(_, x)| x.as_matrix()).collect()
}
fn order(d: &NodeData, n: &str) -> RotateOrder {
    RotateOrder::from_index(i(d, n))
}
fn rotation_quat(euler: DVec3, ord: RotateOrder) -> DQuat {
    DQuat::from_mat4(&euler_to_matrix(euler, ord))
}
fn quat_euler(q: DQuat, ord: RotateOrder) -> DVec3 {
    matrix_to_euler(&DMat3::from_quat(q), ord)
}
fn per(a: DVec3, c: DVec3, g: impl Fn(f64, f64) -> f64) -> DVec3 {
    DVec3::new(g(a.x, c.x), g(a.y, c.y), g(a.z, c.z))
}
fn divide(a: f64, c: f64) -> f64 {
    if c == 0.0 {
        0.0
    } else {
        a / c
    }
}

/// Sum, subtract, average or the first value, over any values.
fn pma<T: Copy + std::ops::Add<Output = T> + std::ops::Sub<Output = T> + std::ops::Div<f64, Output = T>>(op: i64, vals: &[T], zero: T) -> T {
    let Some(&first) = vals.first() else { return zero };
    match op {
        1 => vals[1..].iter().fold(first, |a, &x| a + x),
        2 => vals[1..].iter().fold(first, |a, &x| a - x),
        3 => vals[1..].iter().fold(first, |a, &x| a + x) / vals.len() as f64,
        _ => first,
    }
}

fn smooth(t: f64) -> f64 {
    t * t * (3.0 - 2.0 * t)
}

/// Ramp points (position, value, interp) sorted by position, gathered from a multi compound's three children.
fn ramp(d: &NodeData, pos: &str, val: &str, interp: &str) -> Vec<(f64, DVec3, i64)> {
    let (pa, va, ia) = (at(d, pos), at(d, val), at(d, interp));
    let mut idx: Vec<i32> = d.elements(pa).iter().chain(d.elements(va)).chain(d.elements(ia)).map(|(k, _)| *k).collect();
    idx.sort_unstable();
    idx.dedup();
    let mut out: Vec<(f64, DVec3, i64)> = idx
        .into_iter()
        .map(|k| {
            let x = match d.element_or(va, k) {
                PlugValue::Vec3(x) => *x,
                o => DVec3::splat(o.scalar().unwrap_or(0.0)),
            };
            (d.element_or(pa, k).scalar().unwrap_or(0.0), x, d.element_or(ia, k).scalar().unwrap_or(1.0) as i64)
        })
        .collect();
    out.sort_by(|a, c| a.0.total_cmp(&c.0));
    out
}

/// A ramp at t: None 0 holds, Linear 1, Smooth 2, Spline 3. Each span takes its interpolation from its left point.
fn ramp_at(pts: &[(f64, DVec3, i64)], t: f64) -> Option<DVec3> {
    let first = pts.first()?;
    if t <= first.0 {
        return Some(first.1);
    }
    let last = pts.last().unwrap();
    if t >= last.0 {
        return Some(last.1);
    }
    let k = pts.iter().rposition(|p| p.0 <= t).unwrap_or(0);
    let (a, c) = (&pts[k], &pts[k + 1]);
    let span = c.0 - a.0;
    let u = if span > 0.0 { (t - a.0) / span } else { 0.0 };
    Some(match a.2 {
        0 => a.1,
        2 => a.1.lerp(c.1, smooth(u)),
        3 => {
            // Catmull-Rom with tangents measured over the neighbouring spans.
            let before = if k > 0 { pts[k - 1].1 } else { a.1 };
            let after = if k + 2 < pts.len() { pts[k + 2].1 } else { c.1 };
            let (t0, t1) = ((c.1 - before) * 0.5, (after - a.1) * 0.5);
            let (u2, u3) = (u * u, u * u * u);
            a.1 * (2.0 * u3 - 3.0 * u2 + 1.0) + t0 * (u3 - 2.0 * u2 + u) + c.1 * (-2.0 * u3 + 3.0 * u2) + t1 * (u3 - u2)
        }
        _ => a.1.lerp(c.1, u),
    })
}

fn compose(t: DVec3, r: DMat3, shear: DVec3, s: DVec3) -> DMat4 {
    DMat4::from_translation(t) * DMat4::from_mat3(r) * shear_matrix(shear) * DMat4::from_scale(s)
}

fn hsv_of(c: DVec3) -> DVec3 {
    let (hi, lo) = (c.max_element(), c.min_element());
    let delta = hi - lo;
    let h = if delta <= 0.0 {
        0.0
    } else if hi == c.x {
        60.0 * ((c.y - c.z) / delta).rem_euclid(6.0)
    } else if hi == c.y {
        60.0 * ((c.z - c.x) / delta + 2.0)
    } else {
        60.0 * ((c.x - c.y) / delta + 4.0)
    };
    DVec3::new(h, if hi > 0.0 { delta / hi } else { 0.0 }, hi)
}

fn rgb_of(c: DVec3) -> DVec3 {
    let h = (c.x / 60.0).rem_euclid(6.0);
    let chroma = c.z * c.y;
    let x = chroma * (1.0 - (h.rem_euclid(2.0) - 1.0).abs());
    let (r, g, bl) = match h as i32 {
        0 => (chroma, x, 0.0),
        1 => (x, chroma, 0.0),
        2 => (0.0, chroma, x),
        3 => (0.0, x, chroma),
        4 => (x, 0.0, chroma),
        _ => (chroma, 0.0, x),
    };
    DVec3::new(r, g, bl) + DVec3::splat(c.z - chroma)
}

fn closest_on_triangle(p: DVec3, a: DVec3, c: DVec3, e: DVec3) -> DVec3 {
    let (ab, ac, ap) = (c - a, e - a, p - a);
    let (d1, d2) = (ab.dot(ap), ac.dot(ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }
    let bp = p - c;
    let (d3, d4) = (ab.dot(bp), ac.dot(bp));
    if d3 >= 0.0 && d4 <= d3 {
        return c;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return a + ab * (d1 / (d1 - d3));
    }
    let cp = p - e;
    let (d5, d6) = (ab.dot(cp), ac.dot(cp));
    if d6 >= 0.0 && d5 <= d6 {
        return e;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return a + ac * (d2 / (d2 - d6));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        return c + (e - c) * ((d4 - d3) / ((d4 - d3) + (d5 - d6)));
    }
    let denom = 1.0 / (va + vb + vc);
    a + ab * (vb * denom) + ac * (vc * denom)
}

/// The UV at a point on a face: interpolated in the triangle of the face's fan the point is in.
pub fn uv_at(mesh: &aztek_geo::Mesh, face: u32, q: DVec3) -> glam::DVec2 {
    let corners = mesh.face_corners(face);
    let uv = mesh.corner_attrs().get::<[f32; 2]>(aztek_geo::names::uv(0).as_str());
    let uv_of = |c: u32| uv.get(c as usize).map_or(glam::DVec2::ZERO, |u| glam::DVec2::new(u[0] as f64, u[1] as f64));
    let mut best = (f64::MAX, glam::DVec2::ZERO);
    for k in 1..corners.len().saturating_sub(1) {
        let (c0, c1, c2) = (corners[0], corners[k], corners[k + 1]);
        let (a, b, c) = (mesh.position(mesh.corner_point(c0)), mesh.position(mesh.corner_point(c1)), mesh.position(mesh.corner_point(c2)));
        let (v0, v1, v2) = (b - a, c - a, q - a);
        let (d00, d01, d11, d20, d21) = (v0.dot(v0), v0.dot(v1), v1.dot(v1), v2.dot(v0), v2.dot(v1));
        let den = d00 * d11 - d01 * d01;
        if den.abs() < 1e-20 {
            continue;
        }
        let (w1, w2) = ((d11 * d20 - d01 * d21) / den, (d00 * d21 - d01 * d20) / den);
        let w0 = 1.0 - w1 - w2;
        let miss = (a * w0 + b * w1 + c * w2 - q).length() + (-w0).max(0.0) + (-w1).max(0.0) + (-w2).max(0.0);
        if miss < best.0 {
            best = (miss, uv_of(c0) * w0 + uv_of(c1) * w1 + uv_of(c2) * w2);
        }
    }
    best.1
}

/// Closest surface point on a mesh: (point, face, nearest vertex of that face).
pub fn closest_point(mesh: &aztek_geo::Mesh, p: DVec3) -> Option<(DVec3, u32, u32)> {
    let mut best: Option<(f64, DVec3, u32)> = None;
    for face in 0..mesh.face_capacity() {
        if !mesh.face_valid(face) {
            continue;
        }
        let pts = mesh.face_points(face);
        for k in 1..pts.len().saturating_sub(1) {
            let q = closest_on_triangle(p, mesh.position(pts[0]), mesh.position(pts[k]), mesh.position(pts[k + 1]));
            let dist = q.distance_squared(p);
            if best.as_ref().is_none_or(|b| dist < b.0) {
                best = Some((dist, q, face));
            }
        }
    }
    let (_, q, face) = best?;
    let vert = mesh.face_points(face).into_iter().min_by(|&x, &y| mesh.position(x).distance_squared(q).total_cmp(&mesh.position(y).distance_squared(q)))?;
    Some((q, face, vert))
}

type Compute = fn(&mut NodeData);

struct Utility(Compute);

impl NodeBehavior for Utility {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        (self.0)(d)
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// name, classification, doc, attributes, compute.
/// A ramp's colour at a place along it, its entries held on the node.
pub(crate) fn colour_ramp_at(d: &NodeData, at_v: f64) -> DVec3 {
    let (pa, ca) = (at(d, "colorEntryList.position"), at(d, "colorEntryList.color"));
    let mut keys: Vec<(f64, DVec3)> = d.elements(pa).iter().map(|(k, p)| (p.scalar().unwrap_or(0.0), d.element_or(ca, *k).as_vec3())).collect();
    keys.sort_by(|a, b| a.0.total_cmp(&b.0));
    let (Some(first), Some(last)) = (keys.first(), keys.last()) else { return DVec3::ZERO };
    if at_v <= first.0 {
        return first.1;
    }
    if at_v >= last.0 {
        return last.1;
    }
    let none = i(d, "interpolation") == 0;
    for w in keys.windows(2) {
        if at_v <= w[1].0 {
            if none {
                return w[0].1;
            }
            let span = (w[1].0 - w[0].0).max(1e-12);
            return w[0].1.lerp(w[1].1, (at_v - w[0].0) / span);
        }
    }
    last.1
}

struct Spec(&'static str, &'static str, &'static str, Vec<AttrDef>, Compute);

fn binary_float(name: &'static str, doc: &'static str, c: Compute) -> Spec {
    Spec(name, "utility/general", doc, vec![fin("input1", "i1", 0.0), fin("input2", "i2", 0.0), fout("output", "o")], c)
}

/// Later releases of the reference add "DL" twins of these nodes, the same attributes and the same result.
const TWINS: &[(&str, &str, &str)] = &[
    ("addDL", "addDoubleLinear", "input1 + input2: the later twin of addDoubleLinear."),
    ("multDL", "multDoubleLinear", "input1 * input2: the later twin of multDoubleLinear."),
    ("distanceBetweenDL", "distanceBetween", "The distance between two points: the later twin of distanceBetween."),
    ("angleBetweenDL", "angleBetween", "The rotation from vector1 to vector2: the later twin of angleBetween."),
];

fn table() -> Vec<Spec> {
    let mut t = base_table();
    for (twin, of, doc) in TWINS {
        if let Some(Spec(_, class, _, attrs, compute)) = t.iter().find(|s| s.0 == *of).map(|s| Spec(s.0, s.1, s.2, s.3.clone(), s.4)) {
            t.push(Spec(twin, class, doc, attrs, compute));
        }
    }
    t
}

fn base_table() -> Vec<Spec> {
    vec![
        Spec("multiplyDivide", "utility/general", "Multiplies, divides or raises input1 by input2, per channel.",
            vec![en("operation", "op", 1, &["noOperation", "multiply", "divide", "power"]), vin("input1", "i1", [0.0; 3]), vin("input2", "i2", [1.0; 3]), vout("output", "o")],
            |d| {
                let (x, y) = (v(d, "input1"), v(d, "input2"));
                let o = match i(d, "operation") {
                    1 => x * y,
                    2 => per(x, y, divide),
                    3 => per(x, y, f64::powf),
                    _ => x,
                };
                set(d, "output", o);
            }),
        Spec("plusMinusAverage", "utility/general", "Adds, subtracts or averages its inputs.",
            vec![
                en("operation", "op", 1, &["none", "sum", "subtract", "average"]),
                multi(fin("input1D", "i1", 0.0)),
                multi(two(vec_kids("input2D", "i2", [0.0; 3], IN, LOWER))),
                multi(vec_kids("input3D", "i3", [0.0; 3], IN, LOWER)),
                fout("output1D", "o1"),
                two(vec_kids("output2D", "o2", [0.0; 3], OUT, LOWER)),
                vec_kids("output3D", "o3", [0.0; 3], OUT, LOWER),
            ],
            |d| {
                let op = i(d, "operation");
                let o1 = pma(op, &scalars(d, "input1D"), 0.0);
                let o2 = pma(op, &vecs(d, "input2D"), DVec3::ZERO);
                let o3 = pma(op, &vecs(d, "input3D"), DVec3::ZERO);
                set(d, "output1D", o1);
                set(d, "output2D", o2);
                set(d, "output3D", o3);
            }),
        binary_float("addDoubleLinear", "input1 + input2.", |d| {
            let o = f(d, "input1") + f(d, "input2");
            set(d, "output", o);
        }),
        binary_float("multDoubleLinear", "input1 * input2.", |d| {
            let o = f(d, "input1") * f(d, "input2");
            set(d, "output", o);
        }),
        Spec("condition", "utility/general", "Compares two terms and outputs one colour or the other.",
            vec![
                en("operation", "op", 0, &["Equal", "Not Equal", "Greater Than", "Greater or Equal", "Less Than", "Less or Equal"]),
                fin("firstTerm", "ft", 0.0),
                fin("secondTerm", "st", 0.0),
                cin("colorIfTrue", "ct", [0.0; 3]),
                cin("colorIfFalse", "cf", [1.0; 3]),
                cout("outColor", "oc"),
            ],
            |d| {
                let (x, y) = (f(d, "firstTerm"), f(d, "secondTerm"));
                let yes = match i(d, "operation") {
                    0 => x == y,
                    1 => x != y,
                    2 => x > y,
                    3 => x >= y,
                    4 => x < y,
                    _ => x <= y,
                };
                let o = v(d, if yes { "colorIfTrue" } else { "colorIfFalse" });
                set(d, "outColor", o);
            }),
        Spec("reverse", "utility/color", "1 - input, per channel.", vec![vin("input", "i", [0.0; 3]), vout("output", "o")], |d| {
            let o = DVec3::ONE - v(d, "input");
            set(d, "output", o);
        }),
        Spec("clamp", "utility/color", "Holds input between min and max, per channel.",
            vec![cin("min", "mn", [0.0; 3]), cin("max", "mx", [0.0; 3]), cin("input", "ip", [0.0; 3]), cout("output", "op")],
            |d| {
                let (lo, hi, x) = (v(d, "min"), v(d, "max"), v(d, "input"));
                let o = DVec3::new(x.x.max(lo.x).min(hi.x), x.y.max(lo.y).min(hi.y), x.z.max(lo.z).min(hi.z));
                set(d, "output", o);
            }),
        Spec("setRange", "utility/general", "Maps value from the old range to the new one, held inside the range.",
            vec![vin("value", "v", [0.0; 3]), vin("min", "n", [0.0; 3]), vin("max", "m", [0.0; 3]), vin("oldMin", "on", [0.0; 3]), vin("oldMax", "om", [0.0; 3]), vout("outValue", "o")],
            |d| {
                let (x, lo, hi, a, c) = (v(d, "value"), v(d, "min"), v(d, "max"), v(d, "oldMin"), v(d, "oldMax"));
                let mut o = DVec3::ZERO;
                for k in 0..3 {
                    let span = c[k] - a[k];
                    let t = if span != 0.0 { ((x[k] - a[k]) / span).clamp(0.0, 1.0) } else { 0.0 };
                    o[k] = lo[k] + t * (hi[k] - lo[k]);
                }
                set(d, "outValue", o);
            }),
        Spec("blendColors", "utility/color", "color1 * blender + color2 * (1 - blender).",
            vec![fin("blender", "b", 0.5), cin("color1", "c1", [1.0, 0.0, 0.0]), cin("color2", "c2", [0.0, 0.0, 1.0]), cout("output", "op")],
            |d| {
                let t = f(d, "blender");
                let o = v(d, "color1") * t + v(d, "color2") * (1.0 - t);
                set(d, "output", o);
            }),
        Spec("ramp", "texture/2d", "A colour along V or U, from entries each with a position and a colour.",
            vec![
                en("type", "t", 0, &["V Ramp", "U Ramp", "Diagonal Ramp", "Radial Ramp", "Circular Ramp", "Box Ramp", "UV Ramp", "Four Corner Ramp", "Tartan Ramp"]),
                en("interpolation", "in", 1, &["None", "Linear", "Exponential Up", "Exponential Down", "Smooth", "Bump", "Spike"]),
                fin("uCoord", "u", 0.0),
                fin("vCoord", "v", 0.0),
                kid("colorEntryList", "cel", fin("position", "ep", 0.0)),
                kid("colorEntryList", "cel", cin("color", "ec", [0.0, 0.0, 0.0])),
                two(vec_kids("uvCoord", "uv", [0.0; 3], IN, [("U", "u"), ("V", "v"), ("", "")])),
                two(vec_kids("uvFilterSize", "fs", [0.0; 3], IN, [("X", "x"), ("Y", "y"), ("", "")])),
                cout("outColor", "oc"),
                fout("outAlpha", "oa"),
                crate::rig::data_attr("rampKeys", "rkk", "Each entry's position and value, for what samples the ramp itself: spline IK's twist."),
            ],
            |d| {
                let (pa, ca) = (at(d, "colorEntryList.position"), at(d, "colorEntryList.color"));
                let mut keys: Vec<(f64, f64)> = d.elements(pa).iter().map(|(k, p)| (p.scalar().unwrap_or(0.0), d.element_or(ca, *k).as_vec3().x)).collect();
                keys.sort_by(|a, b| a.0.total_cmp(&b.0));
                let json: serde_json::Value = keys.iter().map(|k| serde_json::json!([k.0, k.1])).collect();
                set(d, "rampKeys", PlugValue::Data(Some(Arc::new(crate::rig::JsonData(json)))));
                let at_v = if i(d, "type") == 1 { f(d, "uCoord") } else { f(d, "vCoord") };
                let o = colour_ramp_at(d, at_v);
                set(d, "outColor", o);
                set(d, "outAlpha", (o.x + o.y + o.z) / 3.0);
            }),
        Spec("unitConversion", "utility/general", "input times conversionFactor. Angles are degrees everywhere here, so only a factor beyond the change of angle units needs one.",
            vec![generic(fin("input", "i", 0.0)), fin("conversionFactor", "cf", 1.0), generic(fout("output", "o"))],
            |d| {
                let (cf, a) = (f(d, "conversionFactor"), at(d, "input"));
                match d.value(a).clone() {
                    PlugValue::Vec3(v) => set(d, "output", PlugValue::Vec3(v * cf)),
                    other => set(d, "output", other.scalar().unwrap_or(0.0) * cf),
                }
            }),
        // The reference counts time inside these two in its own unit, 6000 to the second: a factor of 250 at 24 frames a
        // second is one frame to one unit. Time here is in frames.
        Spec("unitToTimeConversion", "utility/general", "A number into a time: input times conversionFactor, in the reference's internal time unit (6000 a second), read here as frames.",
            vec![fin("input", "i", 0.0), fin("conversionFactor", "cf", 1.0), fout("output", "o")],
            |d| {
                let ticks = f(d, "input") * f(d, "conversionFactor");
                set(d, "output", ticks * crate::anim::fps() / 6000.0);
            }),
        Spec("timeToUnitConversion", "utility/general", "A time into a number: the time in the reference's internal time unit (6000 a second) times conversionFactor.",
            vec![fin("input", "i", 0.0), fin("conversionFactor", "cf", 1.0), fout("output", "o")],
            |d| {
                let ticks = f(d, "input") * 6000.0 / crate::anim::fps().max(1e-9);
                set(d, "output", ticks * f(d, "conversionFactor"));
            }),
        Spec("blendTwoAttr", "utility/general", "Blends input[current] toward input[current + 1] by attributesBlender.",
            vec![iin("current", "c", 0), multi(fin("input", "i", 0.0)), fin("attributesBlender", "ab", 0.0), fout("output", "o")],
            |d| {
                let (c, t, ia) = (i(d, "current") as i32, f(d, "attributesBlender"), at(d, "input"));
                let (x, y) = (d.element_or(ia, c).scalar().unwrap_or(0.0), d.element_or(ia, c + 1).scalar().unwrap_or(0.0));
                set(d, "output", x * (1.0 - t) + y * t);
            }),
        // In this table so they go through .ma files as the utility nodes do. The node code is in rig_extra.
        Spec("jointLattice", "deformer", "A flexor's node: bulges a lattice around a joint as the joint bends.", crate::rig_extra::flexor_attrs(("creasing", "cr"), ("rounding", "ro")), crate::rig_extra::lattice_bulge),
        Spec("boneLattice", "deformer", "A flexor's node: bulges a lattice along a bone as the joint at its end bends.", crate::rig_extra::flexor_attrs(("bicep", "bi"), ("tricep", "tr")), crate::rig_extra::lattice_bulge),
        Spec("combinationShape", "utility/general", "Works out a blend shape target's weight from its drivers: multiplied, the lowest, or the lowest smoothed. The smoothing is a polynomial smooth minimum of this module's own.",
            vec![en("combinationMethod", "cm", 0, &["Multiplication", "Lowest Weighting", "Lowest Weighting (Smooth)"]), multi(fin("inputWeight", "iw", 0.0)), fout("outputWeight", "ow")],
            |d| {
                let v = scalars(d, "inputWeight");
                let out = match (v.is_empty(), i(d, "combinationMethod")) {
                    (true, _) => 0.0,
                    (_, 1) => v.iter().copied().fold(f64::MAX, f64::min),
                    (_, 2) => v.iter().copied().fold(f64::MAX, |a, b| {
                        if a == f64::MAX {
                            return b;
                        }
                        let h = (0.5 + 0.5 * (b - a) / 0.2).clamp(0.0, 1.0);
                        b + (a - b) * h - 0.2 * h * (1.0 - h)
                    }),
                    _ => v.iter().product(),
                };
                set(d, "outputWeight", out);
            }),
        Spec("remapValue", "utility/general", "Maps inputValue through a value ramp and a colour ramp.",
            vec![
                fin("inputValue", "i", 0.0),
                fin("inputMin", "imn", 0.0),
                fin("inputMax", "imx", 1.0),
                fin("outputMin", "omn", 0.0),
                fin("outputMax", "omx", 1.0),
                kid("value", "vl", fin("value_Position", "vlp", 0.0)),
                kid("value", "vl", fin("value_FloatValue", "vlfv", 0.0)),
                kid("value", "vl", en("value_Interp", "vli", 1, &["None", "Linear", "Smooth", "Spline"])),
                kid("color", "cl", fin("color_Position", "clp", 0.0)),
                kid("color", "cl", cin("color_Color", "clc", [0.0; 3])),
                kid("color", "cl", en("color_Interp", "cli", 1, &["None", "Linear", "Smooth", "Spline"])),
                fout("outValue", "ov"),
                cout("outColor", "oc"),
            ],
            |d| {
                let (lo, hi) = (f(d, "inputMin"), f(d, "inputMax"));
                let t = if hi != lo { (f(d, "inputValue") - lo) / (hi - lo) } else { 0.0 };
                // With no ramp points the ramps are straight lines, what a new node starts with.
                let tv = ramp_at(&ramp(d, "value.value_Position", "value.value_FloatValue", "value.value_Interp"), t).map_or(t.clamp(0.0, 1.0), |x| x.x);
                let tc = ramp_at(&ramp(d, "color.color_Position", "color.color_Color", "color.color_Interp"), t).unwrap_or(DVec3::splat(t.clamp(0.0, 1.0)));
                let (a, c) = (f(d, "outputMin"), f(d, "outputMax"));
                set(d, "outValue", a + tv * (c - a));
                set(d, "outColor", DVec3::splat(a) + tc * (c - a));
            }),
        Spec("distanceBetween", "utility/general", "The distance between two points, each through its matrix.",
            vec![vin("point1", "p1", [0.0; 3]), min_in("inMatrix1", "im1"), vin("point2", "p2", [0.0; 3]), min_in("inMatrix2", "im2"), fout("distance", "d")],
            |d| {
                let x = m(d, "inMatrix1").transform_point3(v(d, "point1"));
                let y = m(d, "inMatrix2").transform_point3(v(d, "point2"));
                set(d, "distance", x.distance(y));
            }),
        Spec("angleBetween", "utility/general", "The rotation from vector1 to vector2: axis, angle in degrees, and as Euler angles.",
            vec![vin("vector1", "v1", [1.0, 0.0, 0.0]), vin("vector2", "v2", [1.0, 0.0, 0.0]), vout("axis", "ax"), fout("angle", "a"), vout("euler", "eu")],
            |d| {
                let (x, y) = (v(d, "vector1").normalize_or_zero(), v(d, "vector2").normalize_or_zero());
                let angle = x.angle_between(y);
                let axis = x.cross(y).normalize_or_zero();
                set(d, "axis", axis);
                set(d, "angle", angle);
                let q = if axis == DVec3::ZERO { DQuat::IDENTITY } else { DQuat::from_axis_angle(axis, angle) };
                set(d, "euler", quat_euler(q, RotateOrder::Xyz));
            }),
        Spec("vectorProduct", "utility/general", "Dot or cross product, or a vector or point through a matrix.",
            vec![
                en("operation", "op", 1, &["noOperation", "dotProduct", "crossProduct", "vectorMatrixProduct", "pointMatrixProduct"]),
                vin("input1", "i1", [0.0; 3]),
                vin("input2", "i2", [0.0; 3]),
                min_in("matrix", "m"),
                bin("normalizeOutput", "no", false),
                vout("output", "o"),
            ],
            |d| {
                let (x, y, mm) = (v(d, "input1"), v(d, "input2"), m(d, "matrix"));
                let mut o = match i(d, "operation") {
                    1 => DVec3::splat(x.dot(y)),
                    2 => x.cross(y),
                    3 => mm.transform_vector3(x),
                    4 => mm.transform_point3(x),
                    _ => x,
                };
                if b(d, "normalizeOutput") && i(d, "operation") != 1 {
                    o = o.normalize_or_zero();
                }
                set(d, "output", o);
            }),
        Spec("unitConversion", "utility/general", "input * conversionFactor. Aztek inserts none of these itself.",
            vec![generic(fin("input", "i", 0.0)), fin("conversionFactor", "cf", 1.0), generic(fout("output", "o"))],
            |d| {
                let (cf, a) = (f(d, "conversionFactor"), at(d, "input"));
                match d.value(a).clone() {
                    PlugValue::Vec3(v) => set(d, "output", PlugValue::Vec3(v * cf)),
                    other => set(d, "output", other.scalar().unwrap_or(0.0) * cf),
                }
            }),
        Spec("pairBlend", "utility/general", "Blends two translations and rotations by weight: 0 is the first, 1 the second.",
            vec![
                fin("weight", "w", 1.0),
                numbered("inTranslate", "it", 1),
                numbered("inRotate", "ir", 1),
                numbered("inTranslate", "it", 2),
                numbered("inRotate", "ir", 2),
                en("rotInterpolation", "ri", 0, &["Euler Angles", "Quaternions"]),
                en("translateXMode", "txm", 0, &PAIR_MODES),
                en("translateYMode", "tym", 0, &PAIR_MODES),
                en("translateZMode", "tzm", 0, &PAIR_MODES),
                en("rotateMode", "rm", 0, &PAIR_MODES),
                en("rotateOrder", "ro", 0, &ORDERS),
                vout("outTranslate", "ot"),
                vout("outRotate", "or"),
            ],
            |d| {
                let w = f(d, "weight");
                let (t1, t2) = (v(d, "inTranslate1"), v(d, "inTranslate2"));
                let mut t = t1.lerp(t2, w);
                for (k, mode) in ["translateXMode", "translateYMode", "translateZMode"].into_iter().enumerate() {
                    match i(d, mode) {
                        1 => t[k] = t1[k],
                        2 => t[k] = t2[k],
                        _ => {}
                    }
                }
                let (r1, r2) = (v(d, "inRotate1"), v(d, "inRotate2"));
                let r = match i(d, "rotateMode") {
                    1 => r1,
                    2 => r2,
                    _ if i(d, "rotInterpolation") == 1 => {
                        let ord = order(d, "rotateOrder");
                        quat_euler(rotation_quat(r1, ord).slerp(rotation_quat(r2, ord), w), ord)
                    }
                    _ => r1.lerp(r2, w),
                };
                set(d, "outTranslate", t);
                set(d, "outRotate", r);
            }),
        Spec("decomposeMatrix", "utility/general", "Splits a matrix into translate, rotate, scale, shear and a quaternion.",
            {
                let mut a = vec![min_in("inputMatrix", "imat"), en("inputRotateOrder", "ro", 0, &ORDERS), vout("outputTranslate", "ot"), vout("outputRotate", "or"), vout("outputScale", "os"), vout("outputShear", "osh")];
                a.extend(quat("outputQuat", "oq", OUT));
                a
            },
            |d| {
                let (t, r, s, sh) = decompose_full(&m(d, "inputMatrix"));
                let ord = order(d, "inputRotateOrder");
                set(d, "outputTranslate", t);
                set(d, "outputRotate", matrix_to_euler(&r, ord));
                set(d, "outputScale", s);
                set(d, "outputShear", sh);
                set_quat(d, "outputQuat", DQuat::from_mat3(&r));
            }),
        Spec("composeMatrix", "utility/general", "Builds a matrix from translate, rotate (or a quaternion), scale and shear.",
            {
                let mut a = vec![vin("inputTranslate", "it", [0.0; 3]), vin("inputRotate", "ir", [0.0; 3]), vin("inputScale", "is", [1.0; 3]), vin("inputShear", "ish", [0.0; 3]), en("inputRotateOrder", "iro", 0, &ORDERS), bin("useEulerRotation", "uer", true)];
                a.extend(quat("inputQuat", "iq", IN));
                a.push(mout("outputMatrix", "omat"));
                a
            },
            |d| {
                let r = if b(d, "useEulerRotation") {
                    DMat3::from_mat4(euler_to_matrix(v(d, "inputRotate"), order(d, "inputRotateOrder")))
                } else {
                    DMat3::from_quat(get_quat(d, "inputQuat").normalize())
                };
                let o = compose(v(d, "inputTranslate"), r, v(d, "inputShear"), v(d, "inputScale"));
                set(d, "outputMatrix", o);
            }),
        // matrixIn[0] applies first, as in the reference's row vector product matrixIn[0] * matrixIn[1] * ...
        Spec("multMatrix", "utility/general", "Multiplies its matrices: matrixIn[0] applies first.",
            vec![multi(min_in("matrixIn", "i")), mout("matrixSum", "o")],
            |d| {
                let o = mats(d, "matrixIn").into_iter().fold(DMat4::IDENTITY, |acc, x| x * acc);
                set(d, "matrixSum", o);
            }),
        // The reference's choice takes any kind of value; rigs use it to pick a space's matrix, which is what this one
        // carries.
        Spec("choice", "utility/general", "Passes on the input the selector names: input[selector].",
            vec![iin("selector", "s", 0), multi(min_in("input", "i")), mout("output", "o")],
            |d| {
                let pick = i(d, "selector") as i32;
                let o = d.elements(at(d, "input")).iter().find(|(k, _)| *k == pick).map_or(DMat4::IDENTITY, |(_, x)| x.as_matrix());
                set(d, "output", o);
            }),
        Spec("addMatrix", "utility/general", "Adds its matrices.",
            vec![multi(min_in("matrixIn", "i")), mout("matrixSum", "o")],
            |d| {
                let o = mats(d, "matrixIn").into_iter().fold(DMat4::ZERO, |acc, x| acc + x);
                set(d, "matrixSum", o);
            }),
        Spec("inverseMatrix", "utility/general", "The inverse of a matrix.", vec![min_in("inputMatrix", "imat"), mout("outputMatrix", "omat")], |d| {
            let x = m(d, "inputMatrix");
            set(d, "outputMatrix", if x.determinant() == 0.0 { DMat4::IDENTITY } else { x.inverse() });
        }),
        Spec("transposeMatrix", "utility/general", "The transpose of a matrix.", vec![min_in("inputMatrix", "imat"), mout("outputMatrix", "omat")], |d| {
            let o = m(d, "inputMatrix").transpose();
            set(d, "outputMatrix", o);
        }),
        Spec("wtAddMatrix", "utility/general", "A weighted sum of matrices.",
            vec![kid("wtMatrix", "wm", min_in("matrixIn", "m")), kid("wtMatrix", "wm", fin("weightIn", "w", 1.0)), mout("matrixSum", "o")],
            |d| {
                let (ma, wa) = (at(d, "wtMatrix.matrixIn"), at(d, "wtMatrix.weightIn"));
                let mut idx: Vec<i32> = d.elements(ma).iter().chain(d.elements(wa)).map(|(k, _)| *k).collect();
                idx.sort_unstable();
                idx.dedup();
                let mut o = DMat4::ZERO;
                for k in idx {
                    o += d.element_or(ma, k).as_matrix() * d.element_or(wa, k).scalar().unwrap_or(1.0);
                }
                set(d, "matrixSum", o);
            }),
        Spec("holdMatrix", "utility/general", "Passes a matrix through.", vec![min_in("inMatrix", "i"), mout("outMatrix", "o")], |d| {
            let o = m(d, "inMatrix");
            set(d, "outMatrix", o);
        }),
        Spec("fourByFourMatrix", "utility/general", "A matrix from sixteen values; in30, in31 and in32 are the translation.",
            {
                let mut a = Vec::new();
                for r in 0..4 {
                    for c in 0..4 {
                        a.push(fin(&format!("in{r}{c}"), &format!("i{r}{c}"), if r == c { 1.0 } else { 0.0 }));
                    }
                }
                a.push(mout("output", "o"));
                a
            },
            |d| {
                let first = at(d, "in00");
                let mut cols = [0.0; 16];
                for (k, c) in cols.iter_mut().enumerate() {
                    *c = d.value(first + k).scalar().unwrap_or(0.0);
                }
                set(d, "output", DMat4::from_cols_array(&cols));
            }),
        Spec("quatToEuler", "utility/general", "A quaternion as Euler angles.",
            {
                let mut a = quat("inputQuat", "iq", IN);
                a.push(en("inputRotateOrder", "iro", 0, &ORDERS));
                a.push(vout("outputRotate", "or"));
                a
            },
            |d| {
                let e = quat_euler(get_quat(d, "inputQuat").normalize(), order(d, "inputRotateOrder"));
                set(d, "outputRotate", e);
            }),
        Spec("eulerToQuat", "utility/general", "Euler angles as a quaternion.",
            {
                let mut a = vec![vin("inputRotate", "ir", [0.0; 3]), en("inputRotateOrder", "iro", 0, &ORDERS)];
                a.extend(quat("outputQuat", "oq", OUT));
                a
            },
            |d| {
                let q = rotation_quat(v(d, "inputRotate"), order(d, "inputRotateOrder"));
                set_quat(d, "outputQuat", q);
            }),
        Spec("quatMultiply", "utility/general", "input1Quat then input2Quat.",
            {
                let mut a = quat("input1Quat", "i1q", IN);
                a.extend(quat("input2Quat", "i2q", IN));
                a.extend(quat("outputQuat", "oq", OUT));
                a
            },
            |d| {
                let q = get_quat(d, "input2Quat") * get_quat(d, "input1Quat");
                set_quat(d, "outputQuat", q);
            }),
        Spec("quatProd", "utility/general", "The product of two quaternions: input1Quat's rotation, then input2Quat's.",
            {
                let mut a = quat("input1Quat", "i1q", IN);
                a.extend(quat("input2Quat", "i2q", IN));
                a.extend(quat("outputQuat", "oq", OUT));
                a
            },
            |d| {
                let (a, b) = (get_quat(d, "input1Quat"), get_quat(d, "input2Quat"));
                set_quat(d, "outputQuat", b * a);
            }),
        Spec("quatAdd", "utility/general", "The sum of two quaternions, part by part.",
            {
                let mut a = quat("input1Quat", "i1q", IN);
                a.extend(quat("input2Quat", "i2q", IN));
                a.extend(quat("outputQuat", "oq", OUT));
                a
            },
            |d| {
                let (a, b) = (get_quat(d, "input1Quat"), get_quat(d, "input2Quat"));
                set_quat(d, "outputQuat", DQuat::from_xyzw(a.x + b.x, a.y + b.y, a.z + b.z, a.w + b.w));
            }),
        Spec("quatSub", "utility/general", "The difference of two quaternions, part by part.",
            {
                let mut a = quat("input1Quat", "i1q", IN);
                a.extend(quat("input2Quat", "i2q", IN));
                a.extend(quat("outputQuat", "oq", OUT));
                a
            },
            |d| {
                let (a, b) = (get_quat(d, "input1Quat"), get_quat(d, "input2Quat"));
                set_quat(d, "outputQuat", DQuat::from_xyzw(a.x - b.x, a.y - b.y, a.z - b.z, a.w - b.w));
            }),
        Spec("quatNegate", "utility/general", "Every part of the quaternion negated: the same rotation, the other way round the sphere.",
            {
                let mut a = quat("inputQuat", "iq", IN);
                a.extend(quat("outputQuat", "oq", OUT));
                a
            },
            |d| {
                let q = get_quat(d, "inputQuat");
                set_quat(d, "outputQuat", DQuat::from_xyzw(-q.x, -q.y, -q.z, -q.w));
            }),
        Spec("quatConjugate", "utility/general", "The conjugate: the vector part negated.",
            {
                let mut a = quat("inputQuat", "iq", IN);
                a.extend(quat("outputQuat", "oq", OUT));
                a
            },
            |d| {
                let q = get_quat(d, "inputQuat");
                set_quat(d, "outputQuat", DQuat::from_xyzw(-q.x, -q.y, -q.z, q.w));
            }),
        Spec("quatSlerp", "utility/general", "From input1Quat to input2Quat along the sphere, by inputT, the shortest way.",
            {
                let mut a = quat("input1Quat", "i1q", IN);
                a.extend(quat("input2Quat", "i2q", IN));
                a.push(fin("inputT", "it", 0.0));
                a.push(en("angleInterpolation", "ait", 0, &["Shortest", "Positive", "Negative"]));
                a.extend(quat("outputQuat", "oq", OUT));
                a
            },
            |d| {
                let (a, mut b) = (get_quat(d, "input1Quat").normalize(), get_quat(d, "input2Quat").normalize());
                let t = f(d, "inputT");
                // The same rotation is at b and at -b: the nearer of the two is aimed for. The positive and negative ways
                // round are not told apart yet: all three settings take the shortest.
                if a.dot(b) < 0.0 {
                    b = DQuat::from_xyzw(-b.x, -b.y, -b.z, -b.w);
                }
                let cos = a.dot(b).clamp(-1.0, 1.0);
                let angle = cos.acos();
                let q = if angle.abs() < 1e-9 {
                    a
                } else {
                    let s = angle.sin();
                    let (wa, wb) = (((1.0 - t) * angle).sin() / s, (t * angle).sin() / s);
                    DQuat::from_xyzw(a.x * wa + b.x * wb, a.y * wa + b.y * wb, a.z * wa + b.z * wb, a.w * wa + b.w * wb)
                };
                set_quat(d, "outputQuat", q);
            }),
        Spec("quatInvert", "utility/general", "The inverse quaternion.",
            {
                let mut a = quat("inputQuat", "iq", IN);
                a.extend(quat("outputQuat", "oq", OUT));
                a
            },
            |d| {
                let q = get_quat(d, "inputQuat").inverse();
                set_quat(d, "outputQuat", q);
            }),
        Spec("quatNormalize", "utility/general", "The unit quaternion.",
            {
                let mut a = quat("inputQuat", "iq", IN);
                a.extend(quat("outputQuat", "oq", OUT));
                a
            },
            |d| {
                let q = get_quat(d, "inputQuat").normalize();
                set_quat(d, "outputQuat", q);
            }),
        Spec("blendMatrix", "utility/general", "Blends inputMatrix toward each target in turn, per channel weight.",
            vec![
                min_in("inputMatrix", "imat"),
                fin("envelope", "env", 1.0),
                kid("target", "tgt", min_in("targetMatrix", "tmat")),
                kid("target", "tgt", fin("weight", "wgt", 1.0)),
                kid("target", "tgt", bin("useMatrix", "umt", true)),
                kid("target", "tgt", fin("translateWeight", "tra", 1.0)),
                kid("target", "tgt", fin("rotateWeight", "rot", 1.0)),
                kid("target", "tgt", fin("scaleWeight", "sca", 1.0)),
                kid("target", "tgt", fin("shearWeight", "she", 1.0)),
                mout("outputMatrix", "omat"),
            ],
            |d| {
                let (env, ma) = (f(d, "envelope"), at(d, "target.targetMatrix"));
                let names = ["target.weight", "target.translateWeight", "target.rotateWeight", "target.scaleWeight", "target.shearWeight"];
                let attrs = names.map(|n| at(d, n));
                let mut idx: Vec<i32> = d.elements(ma).iter().map(|(k, _)| *k).collect();
                for a in attrs {
                    idx.extend(d.elements(a).iter().map(|(k, _)| *k));
                }
                idx.sort_unstable();
                idx.dedup();
                let (mut t, r, mut s, mut sh) = decompose_full(&m(d, "inputMatrix"));
                let mut q = DQuat::from_mat3(&r);
                for k in idx {
                    // A target with useMatrix off is skipped.
                    if d.element_or(at(d, "target.useMatrix"), k).scalar().unwrap_or(1.0) == 0.0 {
                        continue;
                    }
                    let w = |a: usize| d.element_or(a, k).scalar().unwrap_or(1.0) * d.element_or(attrs[0], k).scalar().unwrap_or(1.0) * env;
                    let (tt, tr, ts, tsh) = decompose_full(&d.element_or(ma, k).as_matrix());
                    t = t.lerp(tt, w(attrs[1]));
                    q = q.slerp(DQuat::from_mat3(&tr), w(attrs[2]));
                    s = s.lerp(ts, w(attrs[3]));
                    sh = sh.lerp(tsh, w(attrs[4]));
                }
                set(d, "outputMatrix", compose(t, DMat3::from_quat(q), sh, s));
            }),
        Spec("pickMatrix", "utility/general", "Keeps only the chosen parts of a matrix.",
            vec![min_in("inputMatrix", "imat"), bin("useTranslate", "tra", true), bin("useRotate", "rot", true), bin("useScale", "sca", true), bin("useShear", "she", true), mout("outputMatrix", "tmat")],
            |d| {
                let (t, r, s, sh) = decompose_full(&m(d, "inputMatrix"));
                let o = compose(
                    if b(d, "useTranslate") { t } else { DVec3::ZERO },
                    if b(d, "useRotate") { r } else { DMat3::IDENTITY },
                    if b(d, "useShear") { sh } else { DVec3::ZERO },
                    if b(d, "useScale") { s } else { DVec3::ONE },
                );
                set(d, "outputMatrix", o);
            }),
        Spec("aimMatrix", "utility/general", "Turns inputMatrix so its axes follow targets.",
            vec![
                min_in("inputMatrix", "imat"),
                fin("envelope", "env", 1.0),
                kids_named(vin("primaryInputAxis", "pmi", [1.0, 0.0, 0.0]), ["pmx", "pmy", "pmz"]),
                en("primaryMode", "prmd", 1, &["Lock Axis", "Aim", "Align"]),
                kids_named(vin("primaryTargetVector", "pmiv", [0.0, 0.0, 0.0]), ["pmvx", "pmvy", "pmvz"]),
                min_in("primaryTargetMatrix", "pmat"),
                kids_named(vin("secondaryInputAxis", "smi", [0.0, 1.0, 0.0]), ["smx", "smy", "smz"]),
                en("secondaryMode", "sm", 0, &["None", "Aim", "Align"]),
                kids_named(vin("secondaryTargetVector", "smiv", [0.0, 0.0, 0.0]), ["smvx", "smvy", "smvz"]),
                min_in("secondaryTargetMatrix", "smat"),
                mout("outputMatrix", "tmat"),
            ],
            |d| {
                let input = m(d, "inputMatrix");
                let (t, r, s, sh) = decompose_full(&input);
                let dir = |mode: i64, tm: DMat4, vector: DVec3, fallback: DVec3| match mode {
                    1 => (tm.w_axis.truncate() - t).normalize_or(fallback),
                    2 => tm.transform_vector3(vector).normalize_or(fallback),
                    _ => fallback,
                };
                let (pa, sa) = (v(d, "primaryInputAxis").normalize_or(DVec3::X), v(d, "secondaryInputAxis").normalize_or(DVec3::Y));
                let p = dir(i(d, "primaryMode"), m(d, "primaryTargetMatrix"), v(d, "primaryTargetVector"), r * pa);
                let q = dir(i(d, "secondaryMode"), m(d, "secondaryTargetMatrix"), v(d, "secondaryTargetVector"), r * sa);
                let frame = |x: DVec3, y: DVec3| {
                    let z = x.cross(y).normalize_or_zero();
                    let z = if z == DVec3::ZERO { x.any_orthonormal_vector() } else { z };
                    DMat3::from_cols(x, z.cross(x), z)
                };
                let turned = frame(p, q) * frame(pa, sa).transpose();
                let env = f(d, "envelope");
                let blended = DQuat::from_mat3(&r).slerp(DQuat::from_mat3(&turned), env);
                set(d, "outputMatrix", compose(t, DMat3::from_quat(blended), sh, s));
            }),
        Spec("floatMath", "utility/general", "A math operation on two floats.",
            vec![fin("floatA", "fa", 0.0), fin("floatB", "fb", 1.0), en("operation", "op", 0, &["Add", "Subtract", "Multiply", "Divide", "Min", "Max", "Power"]), fout("outFloat", "of")],
            |d| {
                let (x, y) = (f(d, "floatA"), f(d, "floatB"));
                let o = match i(d, "operation") {
                    0 => x + y,
                    1 => x - y,
                    2 => x * y,
                    3 => divide(x, y),
                    4 => x.min(y),
                    5 => x.max(y),
                    _ => x.powf(y),
                };
                set(d, "outFloat", o);
            }),
        Spec("floatConstant", "utility/general", "A float.", vec![fin("inFloat", "if", 0.0), fout("outFloat", "of")], |d| {
            let o = f(d, "inFloat");
            set(d, "outFloat", o);
        }),
        Spec("floatLogic", "utility/general", "Compares two floats.",
            vec![fin("floatA", "fa", 0.0), fin("floatB", "fb", 0.0), en("operation", "op", 0, &["==", "!=", "<", "<=", ">", ">="]), bout("outBool", "ob")],
            |d| {
                let (x, y) = (f(d, "floatA"), f(d, "floatB"));
                let o = match i(d, "operation") {
                    0 => x == y,
                    1 => x != y,
                    2 => x < y,
                    3 => x <= y,
                    4 => x > y,
                    _ => x >= y,
                };
                set(d, "outBool", o);
            }),
        Spec("floatCondition", "utility/general", "floatA when condition is on, else floatB.",
            vec![bin("condition", "cd", true), fin("floatA", "fa", 0.0), fin("floatB", "fb", 0.0), fout("outFloat", "of")],
            |d| {
                let o = f(d, if b(d, "condition") { "floatA" } else { "floatB" });
                set(d, "outFloat", o);
            }),
        Spec("colorConstant", "utility/color", "A colour and alpha.",
            vec![cin("inColor", "ic", [0.0; 3]), fin("inAlpha", "ia", 1.0), cout("outColor", "oc"), fout("outAlpha", "oa")],
            |d| {
                let (c, a) = (v(d, "inColor"), f(d, "inAlpha"));
                set(d, "outColor", c);
                set(d, "outAlpha", a);
            }),
        Spec("sum", "utility/general", "The sum of its inputs.", vec![multi(fin("input", "i", 0.0)), fout("output", "o")], |d| {
            let o: f64 = scalars(d, "input").iter().sum();
            set(d, "output", o);
        }),
        Spec("multiply", "utility/general", "The product of its inputs.", vec![multi(fin("input", "i", 1.0)), fout("output", "o")], |d| {
            let o: f64 = scalars(d, "input").iter().product();
            set(d, "output", o);
        }),
        binary_float("divide", "input1 / input2.", |d| {
            let o = divide(f(d, "input1"), f(d, "input2"));
            set(d, "output", o);
        }),
        binary_float("subtract", "input1 - input2.", |d| {
            let o = f(d, "input1") - f(d, "input2");
            set(d, "output", o);
        }),
        Spec("rgbToHsv", "utility/color", "RGB to hue (degrees), saturation, value.", vec![cin("inRgb", "ir", [0.0; 3]), vec_kids("outHsv", "oh", [0.0; 3], OUT, HSV)], |d| {
            let o = hsv_of(v(d, "inRgb"));
            set(d, "outHsv", o);
        }),
        Spec("hsvToRgb", "utility/color", "Hue (degrees), saturation, value to RGB.", vec![vec_kids("inHsv", "ih", [0.0; 3], IN, HSV), cout("outRgb", "or")], |d| {
            let o = rgb_of(v(d, "inHsv"));
            set(d, "outRgb", o);
        }),
        Spec("luminance", "utility/color", "0.3 R + 0.59 G + 0.11 B.", vec![cin("value", "v", [0.0; 3]), fout("outValue", "ov")], |d| {
            let c = v(d, "value");
            set(d, "outValue", 0.3 * c.x + 0.59 * c.y + 0.11 * c.z);
        }),
        Spec("pointMatrixMult", "utility/general", "A point (or vector) through a matrix.",
            vec![vin("inPoint", "ip", [0.0; 3]), min_in("inMatrix", "im"), bin("vectorMultiply", "vm", false), vout("output", "o")],
            |d| {
                let (p, mm) = (v(d, "inPoint"), m(d, "inMatrix"));
                let o = if b(d, "vectorMultiply") { mm.transform_vector3(p) } else { mm.transform_point3(p) };
                set(d, "output", o);
            }),
        Spec("closestPointOnMesh", "utility/general", "The surface point of a mesh closest to a position.",
            vec![
                base("inMesh", "im", PlugType::Mesh, PlugValue::Mesh(None), flags::INPUT),
                vin("inPosition", "ip", [0.0; 3]),
                min_in("inputMatrix", "imat"),
                vout("position", "p"),
                vout("normal", "n"),
                fout("parameterU", "u"),
                fout("parameterV", "v"),
                iout("closestFaceIndex", "f"),
                iout("closestVertexIndex", "vt"),
            ],
            |d| {
                let w = m(d, "inputMatrix");
                let local = w.inverse().transform_point3(v(d, "inPosition"));
                let found = d.mesh(at(d, "inMesh")).and_then(|mesh| closest_point(mesh, local).map(|(q, face, vert)| (q, face, vert, mesh.face_normal(face))));
                if let Some((q, face, vert, normal)) = found {
                    set(d, "position", w.transform_point3(q));
                    set(d, "normal", w.inverse().transpose().transform_vector3(normal).normalize_or_zero());
                    let uv = d.mesh(at(d, "inMesh")).map_or(glam::DVec2::ZERO, |mesh| uv_at(mesh, face, q));
                    set(d, "parameterU", uv.x);
                    set(d, "parameterV", uv.y);
                    set(d, "closestFaceIndex", face as i64);
                    set(d, "closestVertexIndex", vert as i64);
                }
            }),
    ]
}

/// classification, by type: for shadingNode and getClassification.
pub fn classification(type_name: &str) -> Option<&'static str> {
    table().into_iter().find(|s| s.0 == type_name).map(|s| s.1).or_else(|| crate::shading::classification(type_name)).or_else(|| crate::lights::classification(type_name))
}

pub fn register_types() {
    for Spec(name, _, doc, own_attrs, compute) in table() {
        if types::find(name).is_some() {
            continue;
        }
        types::register_type(NodeType {
            name: name.into(),
            doc: doc.into(),
            own_attrs,
            create: Some(Arc::new(move || Box::new(Utility(compute)))),
            ignores_added_attrs: true,
            ..Default::default()
        })
        .expect(name);
    }
}

/// What createNode gives a new node beyond its defaults: remapValue starts with its ramps' two end points.
pub fn seed(g: &mut Graph, id: NodeId) -> crate::error::Result<()> {
    let Some(n) = g.node(id) else { return Ok(()) };
    if n.ty().name != "remapValue" {
        return Ok(());
    }
    let ty = n.type_arc().clone();
    let attr = |name: &str| ty.attr_index(name).unwrap() as i32;
    for (k, x) in [(0, 0.0), (1, 1.0)] {
        g.set_value(PlugRef::element(id, attr("value.value_Position"), k), PlugValue::Float(x))?;
        g.set_value(PlugRef::element(id, attr("value.value_FloatValue"), k), PlugValue::Float(x))?;
        g.set_value(PlugRef::element(id, attr("value.value_Interp"), k), PlugValue::Int(1))?;
        g.set_value(PlugRef::element(id, attr("color.color_Position"), k), PlugValue::Float(x))?;
        g.set_value(PlugRef::element(id, attr("color.color_Color"), k), PlugValue::Vec3(DVec3::splat(x)))?;
        g.set_value(PlugRef::element(id, attr("color.color_Interp"), k), PlugValue::Int(1))?;
    }
    Ok(())
}
