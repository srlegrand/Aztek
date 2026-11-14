//! The reference's newer rigging nodes: parentMatrix, uvPin, proximityPin, remapColor, remapHsv and symmetryConstraint. Attributes,
//! defaults and classifications are the 2027 release's (PARITY.md says where they were read).

use aztek_geo::Mesh;
use aztek_ops::units::Unit;
use glam::{DMat3, DMat4, DQuat, DVec2, DVec3};

use super::*;

// ---------------------------------------------------------------------------------------------------------------------
// Attribute helpers

fn unit(mut a: AttrDef, u: Unit) -> AttrDef {
    a.unit = u;
    a
}
fn with_flags(mut a: AttrDef, fl: u32) -> AttrDef {
    a.flags = fl;
    a
}
fn geometry_in(name: &str, short: &str) -> AttrDef {
    base(name, short, PlugType::Mesh, PlugValue::Mesh(None), flags::INPUT)
}
fn matrix_store(name: &str, short: &str) -> AttrDef {
    base(name, short, PlugType::Matrix, PlugValue::Matrix(DMat4::IDENTITY), STORE)
}
const AXES: [&str; 6] = ["X", "Y", "Z", "-X", "-Y", "-Z"];
const SPACES: [&str; 3] = ["World", "Local", "Custom"];
const INTERPS: [&str; 4] = ["None", "Linear", "Smooth", "Spline"];

fn store_enum(name: &str, short: &str, d: i64, names: &[&str]) -> AttrDef {
    with_flags(en(name, short, d, names), STORE)
}

/// A ramp of floats on a multi compound: name[i].name_Position, name_FloatValue, name_Interp.
fn float_ramp(name: &str, short: &str) -> Vec<AttrDef> {
    vec![
        kid(name, short, fin(&format!("{name}_Position"), &format!("{short}p"), 0.0)),
        kid(name, short, fin(&format!("{name}_FloatValue"), &format!("{short}fv"), 0.0)),
        kid(name, short, en(&format!("{name}_Interp"), &format!("{short}i"), 1, &INTERPS)),
    ]
}
fn float_ramp_at(d: &NodeData, name: &str, t: f64) -> f64 {
    let pts = ramp(d, &format!("{name}.{name}_Position"), &format!("{name}.{name}_FloatValue"), &format!("{name}.{name}_Interp"));
    // With no points the ramp is a straight line, as a new node's is.
    ramp_at(&pts, t).map_or(t.clamp(0.0, 1.0), |x| x.x)
}

// ---------------------------------------------------------------------------------------------------------------------
// Matrices: the reference's A * B is B * A here (its rows are these columns).

fn rmul(a: DMat4, b: DMat4) -> DMat4 {
    b * a
}

/// Weighted blend of matrices, taken apart into translation, rotation, scale and shear: a normalized average.
fn blend(ms: &[(DMat4, f64)]) -> Option<DMat4> {
    let total: f64 = ms.iter().map(|(_, w)| *w).sum();
    if total <= 0.0 {
        return None;
    }
    let (mut t, mut s, mut sh) = (DVec3::ZERO, DVec3::ZERO, DVec3::ZERO);
    let mut q: Option<DQuat> = None;
    let mut acc = 0.0;
    for (m, w) in ms.iter().filter(|(_, w)| *w > 0.0) {
        let (mt, mr, ms_, msh) = decompose_full(m);
        let k = w / total;
        t += mt * k;
        s += ms_ * k;
        sh += msh * k;
        acc += w;
        let mq = DQuat::from_mat3(&mr);
        q = Some(match q {
            None => mq,
            Some(p) => p.slerp(mq, w / acc),
        });
    }
    Some(compose(t, DMat3::from_quat(q.unwrap_or(DQuat::IDENTITY)), sh, s))
}

fn lerp_matrix(a: DMat4, b: DMat4, w: f64) -> DMat4 {
    if w <= 0.0 {
        return a;
    }
    if w >= 1.0 {
        return b;
    }
    blend(&[(a, 1.0 - w), (b, w)]).unwrap_or(a)
}

/// parentMatrix: each enabled target's offsetMatrix * targetMatrix, blended by weight and normalized, then mixed with
/// inputMatrix by the envelope. Every matrix is taken into preSpaceMatrix first and the result into postSpaceMatrix.
fn parent_matrix(d: &mut NodeData) {
    let input = m(d, "inputMatrix");
    let env = f(d, "envelope").clamp(0.0, 1.0);
    if !b(d, "enable") || env == 0.0 {
        set(d, "outputMatrix", input);
        return;
    }
    let (pre, post) = (m(d, "preSpaceMatrix"), m(d, "postSpaceMatrix"));
    let (ta, oa, wa, ea) = (at(d, "target.targetMatrix"), at(d, "target.offsetMatrix"), at(d, "target.weight"), at(d, "target.enableTarget"));
    let mut idx: Vec<i32> = [ta, oa, wa, ea].iter().flat_map(|&a| d.elements(a).iter().map(|(k, _)| *k)).collect();
    idx.sort_unstable();
    idx.dedup();
    let mut parts = Vec::new();
    for k in idx {
        if d.element_or(ea, k).scalar().unwrap_or(1.0) == 0.0 {
            continue;
        }
        let w = d.element_or(wa, k).scalar().unwrap_or(1.0).clamp(0.0, 1.0);
        let c = rmul(rmul(d.element_or(oa, k).as_matrix(), d.element_or(ta, k).as_matrix()), pre);
        parts.push((c, w));
    }
    let start = rmul(input, pre);
    let blended = blend(&parts).unwrap_or(start);
    set(d, "outputMatrix", rmul(lerp_matrix(start, blended, env), post));
}

// ---------------------------------------------------------------------------------------------------------------------
// Pins: a place on a mesh (a face's triangle and its weights), found once on the original and read on the deformed.

#[derive(Clone, Copy)]
struct Spot {
    corners: [u32; 3],
    w: [f64; 3],
}

fn uv_of(mesh: &Mesh, c: u32) -> DVec2 {
    let uv = mesh.corner_attrs().get::<[f32; 2]>(aztek_geo::names::uv(0).as_str());
    uv.get(c as usize).map_or(DVec2::ZERO, |u| DVec2::new(u[0] as f64, u[1] as f64))
}

fn triangles(mesh: &Mesh) -> impl Iterator<Item = [u32; 3]> + '_ {
    (0..mesh.face_capacity()).filter(|&f| mesh.face_valid(f)).flat_map(move |f| {
        let c = mesh.face_corners(f);
        (1..c.len().saturating_sub(1)).map(move |k| [c[0], c[k], c[k + 1]]).collect::<Vec<_>>()
    })
}

fn bary2(p: DVec2, a: DVec2, b: DVec2, c: DVec2) -> Option<[f64; 3]> {
    let (v0, v1, v2) = (b - a, c - a, p - a);
    let den = v0.x * v1.y - v1.x * v0.y;
    if den.abs() < 1e-20 {
        return None;
    }
    let w1 = (v2.x * v1.y - v1.x * v2.y) / den;
    let w2 = (v0.x * v2.y - v2.x * v0.y) / den;
    Some([1.0 - w1 - w2, w1, w2])
}

/// The spot at a UV: the triangle that holds it, or the nearest one.
fn spot_at_uv(mesh: &Mesh, uv: DVec2) -> Option<Spot> {
    let mut best: Option<(f64, Spot)> = None;
    for t in triangles(mesh) {
        let Some(w) = bary2(uv, uv_of(mesh, t[0]), uv_of(mesh, t[1]), uv_of(mesh, t[2])) else { continue };
        let miss = w.iter().map(|x| (-x).max(0.0)).sum::<f64>();
        if best.as_ref().is_none_or(|b| miss < b.0) {
            let wc = if miss > 0.0 {
                let c = w.map(|x| x.max(0.0));
                let s = c.iter().sum::<f64>().max(1e-12);
                c.map(|x| x / s)
            } else {
                w
            };
            best = Some((miss, Spot { corners: t, w: wc }));
        }
        if miss == 0.0 {
            break;
        }
    }
    best.map(|b| b.1)
}

/// The spot closest to a point.
fn spot_near(mesh: &Mesh, p: DVec3) -> Option<Spot> {
    let mut best: Option<(f64, Spot)> = None;
    for t in triangles(mesh) {
        let [a, bb, c] = t.map(|k| mesh.position(mesh.corner_point(k)));
        let q = closest_on_triangle(p, a, bb, c);
        let dist = q.distance_squared(p);
        if best.as_ref().is_some_and(|x| x.0 <= dist) {
            continue;
        }
        let (v0, v1, v2) = (bb - a, c - a, q - a);
        let (d00, d01, d11, d20, d21) = (v0.dot(v0), v0.dot(v1), v1.dot(v1), v2.dot(v0), v2.dot(v1));
        let den = d00 * d11 - d01 * d01;
        if den.abs() < 1e-20 {
            continue;
        }
        let w1 = (d11 * d20 - d01 * d21) / den;
        let w2 = (d00 * d21 - d01 * d20) / den;
        best = Some((dist, Spot { corners: t, w: [1.0 - w1 - w2, w1, w2] }));
    }
    best.map(|b| b.1)
}

fn valid(mesh: &Mesh, s: &Spot) -> bool {
    s.corners.iter().all(|&c| (mesh.corner_point(c) as usize) < mesh.positions().len())
}

/// Position, normal (the points' smooth normals blended) and a tangent at a spot: along U, or along the triangle's
/// first edge.
fn frame_at(mesh: &Mesh, s: &Spot, normals: &[glam::Vec3], along_uv: bool) -> (DVec3, DVec3, DVec3) {
    let pts = s.corners.map(|c| mesh.corner_point(c));
    let p = pts.map(|k| mesh.position(k));
    let pos = p[0] * s.w[0] + p[1] * s.w[1] + p[2] * s.w[2];
    let face_n = (p[1] - p[0]).cross(p[2] - p[0]).normalize_or_zero();
    let mut n = DVec3::ZERO;
    for (k, &i) in pts.iter().enumerate() {
        if let Some(v) = normals.get(i as usize) {
            n += v.as_dvec3() * s.w[k];
        }
    }
    let n = if n.length_squared() > 1e-20 { n.normalize() } else { face_n };
    let mut t = p[1] - p[0];
    if along_uv {
        let uv = s.corners.map(|c| uv_of(mesh, c));
        let (e1, e2) = (p[1] - p[0], p[2] - p[0]);
        let (d1, d2) = (uv[1] - uv[0], uv[2] - uv[0]);
        let den = d1.x * d2.y - d2.x * d1.y;
        if den.abs() > 1e-20 {
            t = (e1 * d2.y - e2 * d1.y) / den;
        }
    }
    let t = (t - n * t.dot(n)).normalize_or_zero();
    (pos, n, t)
}

fn axis(k: i64) -> (usize, f64) {
    ((k.rem_euclid(3)) as usize, if k >= 3 { -1.0 } else { 1.0 })
}

/// A matrix whose normalAxis row is the normal and tangentAxis row the tangent (tangentAxis 6, None, leaves the
/// tangent to the remaining row), the third row completing a right-handed frame, at the position.
fn pin_matrix(pos: DVec3, n: DVec3, t: DVec3, normal_axis: i64, tangent_axis: i64) -> DMat4 {
    let (na, ns) = axis(normal_axis);
    let mut rows = [DVec3::ZERO; 3];
    rows[na] = n * ns;
    let (ta, ts) = if tangent_axis == 6 || axis(tangent_axis).0 == na { ((na + 1) % 3, 1.0) } else { axis(tangent_axis) };
    rows[ta] = t * ts;
    let r = 3 - na - ta;
    rows[r] = rows[(r + 1) % 3].cross(rows[(r + 2) % 3]);
    DMat4::from_cols(rows[0].extend(0.0), rows[1].extend(0.0), rows[2].extend(0.0), pos.extend(1.0))
}

/// The pins' outputs in the space relativeSpaceMode asks for: as the geometry is (World, Local), or relative to
/// relativeSpaceMatrix (Custom).
fn in_space(d: &NodeData, mx: DMat4) -> DMat4 {
    if i(d, "relativeSpaceMode") == 2 {
        rmul(mx, m(d, "relativeSpaceMatrix").inverse())
    } else {
        mx
    }
}

fn uv_pin(d: &mut NodeData) {
    let (deformed, original) = (d.mesh(at(d, "deformedGeometry")).cloned(), d.mesh(at(d, "originalGeometry")).cloned());
    let Some(def) = deformed else { return };
    let orig = original.unwrap_or_else(|| def.clone());
    let normals = aztek_geo::normals::point_normals(&def, aztek_geo::normals::NormalWeight::Angle);
    let (na, ta) = (i(d, "normalAxis"), i(d, "tangentAxis"));
    let coords: Vec<(i32, DVec2)> = d.elements(at(d, "coordinate")).iter().map(|(k, v)| (*k, v.as_vec3().truncate())).collect();
    let (mut mats, mut trans) = (Vec::new(), Vec::new());
    for (k, uv) in coords {
        let Some(s) = spot_at_uv(&orig, uv).filter(|s| valid(&def, s)) else { continue };
        let (pos, n, t) = frame_at(&def, &s, &normals, true);
        let mx = in_space(d, pin_matrix(pos, n, t, na, ta));
        trans.push((k, PlugValue::Vec3(mx.w_axis.truncate())));
        mats.push((k, PlugValue::Matrix(mx)));
    }
    let (ma, tr) = (at(d, "outputMatrix"), at(d, "outputTranslate"));
    d.set_output_elements(ma, mats);
    d.set_output_elements(tr, trans);
}

fn proximity_pin(d: &mut NodeData) {
    let (deformed, original) = (d.mesh(at(d, "deformedGeometry")).cloned(), d.mesh(at(d, "originalGeometry")).cloned());
    let inputs: Vec<(i32, DMat4)> = d.elements(at(d, "inputMatrix")).iter().map(|(k, v)| (*k, v.as_matrix())).collect();
    let oa = at(d, "outputMatrix");
    let Some(def) = deformed else {
        d.set_output_elements(oa, inputs.into_iter().map(|(k, x)| (k, PlugValue::Matrix(x))).collect());
        return;
    };
    let orig = original.unwrap_or_else(|| def.clone());
    let (n0, n1) = (aztek_geo::normals::point_normals(&orig, aztek_geo::normals::NormalWeight::Angle), aztek_geo::normals::point_normals(&def, aztek_geo::normals::NormalWeight::Angle));
    let (na, ta, by_uv) = (i(d, "normalAxis"), i(d, "tangentAxis"), i(d, "coordMode") == 1);
    let (env, ot, oo) = (f(d, "envelope"), f(d, "offsetTranslation").clamp(0.0, 1.0), f(d, "offsetOrientation").clamp(0.0, 1.0));
    let mut out = Vec::new();
    for (k, input) in inputs {
        let at_p = input.w_axis.truncate();
        let Some(s) = spot_near(&orig, at_p).filter(|s| valid(&def, s)) else {
            out.push((k, PlugValue::Matrix(input)));
            continue;
        };
        let (p0, nn0, t0) = frame_at(&orig, &s, &n0, by_uv);
        let (p1, nn1, t1) = frame_at(&def, &s, &n1, by_uv);
        let (f0, f1) = (pin_matrix(p0, nn0, t0, na, ta), pin_matrix(p1, nn1, t1, na, ta));
        // The bind offset, all of it at 1; at 0 the matrix sits on the surface in the surface's frame.
        let full = rmul(rmul(input, f0.inverse()), f1);
        let (ft, fr, fs, fsh) = decompose_full(&full);
        let (_, sr, _, _) = decompose_full(&f1);
        let pos = p1.lerp(ft, ot);
        let rot = DQuat::from_mat3(&sr).slerp(DQuat::from_mat3(&fr), oo);
        let pinned = compose(pos, DMat3::from_quat(rot), fsh, fs);
        out.push((k, PlugValue::Matrix(in_space(d, lerp_matrix(input, pinned, env)))));
    }
    d.set_output_elements(oa, out);
}

// ---------------------------------------------------------------------------------------------------------------------
// Colour remaps

fn remap_index(d: &NodeData, x: f64) -> f64 {
    let (lo, hi) = (f(d, "inputMin"), f(d, "inputMax"));
    if hi != lo {
        (x - lo) / (hi - lo)
    } else {
        0.0
    }
}
fn remap_out(d: &NodeData, v: f64) -> f64 {
    let (a, c) = (f(d, "outputMin"), f(d, "outputMax"));
    a + v * (c - a)
}

fn remap_color(d: &mut NodeData) {
    let c = v(d, "color");
    let o = DVec3::new(
        remap_out(d, float_ramp_at(d, "red", remap_index(d, c.x))),
        remap_out(d, float_ramp_at(d, "green", remap_index(d, c.y))),
        remap_out(d, float_ramp_at(d, "blue", remap_index(d, c.z))),
    );
    set(d, "outColor", o);
}

/// The colour taken to hue (0 to 1 around the circle), saturation and value, each through its ramp, and back.
fn remap_hsv(d: &mut NodeData) {
    let hsv = hsv_of(v(d, "color"));
    let h = remap_out(d, float_ramp_at(d, "hue", remap_index(d, hsv.x / 360.0)));
    let s = remap_out(d, float_ramp_at(d, "saturation", remap_index(d, hsv.y)));
    let val = remap_out(d, float_ramp_at(d, "value", remap_index(d, hsv.z)));
    set(d, "outColor", rgb_of(DVec3::new(h * 360.0, s, val)));
}

fn remap_attrs(ramps: [(&str, &str); 3]) -> Vec<AttrDef> {
    let mut a = vec![kids_named(cin("color", "cl", [0.5; 3]), ["cr", "cg", "cb"])];
    for (n, s) in ramps {
        a.extend(float_ramp(n, s));
    }
    a.extend([fin("inputMin", "imn", 0.0), fin("inputMax", "imx", 1.0), fin("outputMin", "omn", 0.0), fin("outputMax", "omx", 1.0)]);
    a.push(en("renderPassMode", "arp", 1, &["Pass through", "Apply to Render Passes", "No Contribution", "Write Shader Result to Beauty Passes"]));
    a.push(cout("outColor", "oc"));
    a
}

/// The ramps a new remapColor or remapHsv starts with, as remapValue's do: (0, 0) to (1, 1), linear.
pub(super) fn seed_ramps(ty: &str) -> &'static [&'static str] {
    match ty {
        "remapColor" => &["red", "green", "blue"],
        "remapHsv" => &["hue", "saturation", "value"],
        _ => &[],
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// symmetryConstraint: a joint mirrored from another, as the joint tool's symmetry makes them.

const ORDER_NAMES: [&str; 6] = ["xyz", "yzx", "zxy", "xzy", "yxz", "zyx"];

fn vec_store(name: &str, short: &str, u: Unit, d: [f64; 3]) -> AttrDef {
    unit(vec_kids(name, short, d, STORE, XYZ), u)
}
fn vec_out(name: &str, short: &str, u: Unit) -> AttrDef {
    unit(vout(name, short), u)
}
fn in_target(mut a: AttrDef) -> AttrDef {
    a.category = "target".into();
    a
}
fn rotation(m: &DMat4) -> DMat3 {
    let m3 = DMat3::from_mat4(*m);
    DMat3::from_cols(m3.x_axis.normalize_or_zero(), m3.y_axis.normalize_or_zero(), m3.z_axis.normalize_or_zero())
}

/// The target's place mirrored across the root's planes (at symmetryRootOffset, in the root's frame), into the
/// constrained object's parent space. Rotate, scale and rotate order are the target's; the jointOrient is the target's
/// frame mirrored with every axis reversed, so the same rotate values make mirrored motion.
fn symmetry(d: &mut NodeData) {
    let flip = DVec3::new(
        if b(d, "xAxis") { -1.0 } else { 1.0 },
        if b(d, "yAxis") { -1.0 } else { 1.0 },
        if b(d, "zAxis") { -1.0 } else { 1.0 },
    );
    let root = m(d, "symmetryRootWorldMatrix");
    let origin = v(d, "symmetryRootOffset");
    let cipm = m(d, "constraintInverseParentWorldMatrix");
    let target = m(d, "targetWorldMatrix").w_axis.truncate();
    let at = root.inverse().transform_point3(target);
    let mirrored = origin + (at - origin) * flip;
    set(d, "constraintTranslate", cipm.transform_point3(root.transform_point3(mirrored)));
    set(d, "symmetryMiddlePoint", (at + mirrored) * 0.5);
    set(d, "constraintRotate", v(d, "targetRotate"));
    set(d, "constraintScale", v(d, "targetScale"));
    let ro = i(d, "targetRotateOrder");
    set(d, "constraintRotateOrder", PlugValue::Int(ro));
    let rr = rotation(&root);
    let reflect = rr * DMat3::from_diagonal(flip) * rr.transpose();
    let sign = if flip.x * flip.y * flip.z < 0.0 { -1.0 } else { 1.0 };
    let frame = rotation(&m(d, "targetParentMatrix")) * DMat3::from_mat4(euler_to_matrix(v(d, "targetJointOrient"), RotateOrder::Xyz));
    let orient = rotation(&cipm) * (reflect * frame * sign);
    set(d, "constraintJointOrient", matrix_to_euler(&orient, RotateOrder::Xyz));
}

fn symmetry_attrs() -> Vec<AttrDef> {
    let order = |n: &str, s: &str| store_enum(n, s, 0, &ORDER_NAMES);
    vec![
        in_target(vec_store("targetTranslate", "tt", Unit::Linear, [0.0; 3])),
        in_target(vec_store("targetRotate", "tr", Unit::Angle, [0.0; 3])),
        in_target(vec_store("targetScale", "ts", Unit::Linear, [1.0; 3])),
        in_target(order("targetRotateOrder", "tro")),
        in_target(order("targetJointOrientType", "tjt")),
        in_target(kids_named(vec_store("targetJointOrient", "tjo", Unit::Angle, [0.0; 3]), ["tjx", "tjy", "tjz"])),
        in_target(kids_named(vec_store("targetChildTranslate", "tct", Unit::Linear, [0.0; 3]), ["tcx", "tcy", "tcz"])),
        in_target(matrix_store("targetWorldMatrix", "twm")),
        in_target(matrix_store("targetParentMatrix", "tpm")),
        matrix_store("constraintInverseParentWorldMatrix", "cipm"),
        matrix_store("symmetryRootWorldMatrix", "cpim"),
        kids_named(vec_store("symmetryRootOffset", "srof", Unit::Linear, [0.0; 3]), ["srox", "sroy", "sroz"]),
        with_flags(bin("xAxis", "syx", true), STORE),
        with_flags(bin("yAxis", "syy", false), STORE),
        with_flags(bin("zAxis", "syz", false), STORE),
        with_flags(bin("xChildAxis", "cyx", true), STORE),
        with_flags(bin("yChildAxis", "cyy", false), STORE),
        with_flags(bin("zChildAxis", "cyz", false), STORE),
        with_flags(bin("enableRestPosition", "erp", false), STORE),
        vec_out("constraintTranslate", "ct", Unit::Linear),
        vec_out("constraintRotate", "cr", Unit::Angle),
        unit(vec_kids("constraintScale", "cs", [1.0; 3], OUT, XYZ), Unit::Linear),
        with_flags(en("constraintRotateOrder", "cro", 0, &ORDER_NAMES), OUT),
        kids_named(vec_out("constraintJointOrient", "cjo", Unit::Angle), ["cjx", "cjy", "cjz"]),
        kids_named(vec_out("symmetryMiddlePoint", "cmwp", Unit::Linear), ["cmpx", "cmpy", "cmpz"]),
    ]
}

// ---------------------------------------------------------------------------------------------------------------------
// timeFunction and sampler: a value as a function of time, and samples of one.

/// A function of time: here the value its timeFunction held when evaluated, the same at every time.
#[derive(Debug, Clone)]
pub struct TimeFunction(pub f64);

impl crate::plug::Data for TimeFunction {
    fn kind(&self) -> &str {
        "function"
    }
    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({"kind": "function", "value": self.0})
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

fn time_function(d: &mut NodeData) {
    let x = f(d, "input");
    set(d, "output", PlugValue::Data(Some(std::sync::Arc::new(TimeFunction(x)))));
}

/// The function sampled from minimum to maximum by step (the other way with invert): its connected function, else
/// function_Default.
fn sampler(d: &mut NodeData) {
    let value = match d.value(at(d, "function_Raw")) {
        PlugValue::Data(Some(x)) => x.as_any().downcast_ref::<TimeFunction>().map(|t| t.0),
        _ => None,
    }
    .unwrap_or_else(|| f(d, "function_Default"));
    let (lo, hi, step) = (f(d, "minimum"), f(d, "maximum"), f(d, "step"));
    let count = if step > 0.0 && hi >= lo { ((hi - lo) / step + 1e-9).floor() as usize + 1 } else { 1 };
    let count = count.min(100_000);
    let out = (0..count as i32).map(|k| (k, PlugValue::Float(value))).collect();
    let a = at(d, "value");
    d.set_output_elements(a, out);
}

fn function_attrs() -> Vec<AttrDef> {
    let in_function = |mut a: AttrDef| {
        a.category = "function".into();
        a
    };
    let short = |name: &str, s: &str| with_flags(base(name, s, PlugType::Int, PlugValue::Int(0), STORE), STORE);
    vec![
        in_function(with_flags(fin("function_Default", "fd", 0.0), STORE)),
        in_function(base("function_Hidden", "fh", PlugType::Data, PlugValue::Data(None), flags::HIDDEN)),
        in_function(base("function_Raw", "fr", PlugType::Data, PlugValue::Data(None), flags::INPUT | flags::HIDDEN)),
        kid("function_Inmap", "fi", short("function_InmapFrom", "fif")),
        kid("function_Inmap", "fi", short("function_InmapTo", "fit")),
        kid("function_Outmap", "fo", short("function_OutmapFrom", "fof")),
        kid("function_Outmap", "fo", short("function_OutmapTo", "fot")),
        with_flags(bin("invert", "i", false), STORE),
        with_flags(fin("maximum", "max", 0.0), STORE),
        with_flags(fin("minimum", "min", 0.0), STORE),
        with_flags(fin("step", "s", 0.0), STORE),
        multi(fout("value", "v")),
    ]
}

pub(super) fn specs() -> Vec<Spec> {
    vec![
        Spec("parentMatrix", "animation", "Blends its targets, each with its offset, into the input matrix: a parent constraint as a matrix.",
            vec![
                with_flags(min_in("inputMatrix", "imat"), IN),
                kid("target", "tgt", matrix_store("targetMatrix", "tmat")),
                kid("target", "tgt", matrix_store("offsetMatrix", "ofm")),
                kid("target", "tgt", limited(with_flags(fin("weight", "wgt", 1.0), STORE), 0.0, 1.0)),
                kid("target", "tgt", with_flags(bin("enableTarget", "umt", true), STORE)),
                with_flags(bin("enable", "enb", true), STORE),
                limited(with_flags(fin("envelope", "env", 1.0), STORE), 0.0, 1.0),
                matrix_store("preSpaceMatrix", "premat"),
                matrix_store("postSpaceMatrix", "pstmat"),
                mout("outputMatrix", "omat"),
            ],
            parent_matrix),
        Spec("uvPin", "geometry", "A matrix for each UV coordinate, on the surface of a deforming mesh.",
            vec![
                geometry_in("deformedGeometry", "curgeom"),
                geometry_in("originalGeometry", "orggeom"),
                multi(kids_named(two(vec_kids("coordinate", "coord", [0.0; 3], IN, [("U", "u"), ("V", "v"), ("", "")])), ["cu", "cv", ""])),
                store_enum("normalAxis", "nrm", 0, &AXES),
                store_enum("tangentAxis", "tng", 2, &AXES),
                store_enum("normalOverride", "novr", 0, &["Auto", "Rail Curve"]),
                with_flags(bin("normalizedIsoParms", "nrmip", true), STORE),
                geometry_in("railCurve", "rlcrv"),
                store_enum("relativeSpaceMode", "rsmd", 0, &SPACES),
                matrix_store("relativeSpaceMatrix", "rsmat"),
                base("uvSetName", "msn", PlugType::String, PlugValue::String(String::new()), STORE),
                multi(mout("outputMatrix", "omat")),
                unit(multi(vout("outputTranslate", "ot")), Unit::Linear),
            ],
            uv_pin),
        Spec("proximityPin", "geometry", "Matrices bound to the closest place on a mesh, following it as it deforms.",
            vec![
                geometry_in("deformedGeometry", "curgeom"),
                geometry_in("originalGeometry", "orggeom"),
                multi(matrix_store("inputMatrix", "imat")),
                fin("envelope", "en", 1.0),
                limited(fin("offsetTranslation", "ostr", 1.0), 0.0, 1.0),
                limited(fin("offsetOrientation", "osor", 1.0), 0.0, 1.0),
                store_enum("coordMode", "crdm", 0, &["Edge", "UV"]),
                store_enum("normalAxis", "nrm", 0, &AXES),
                store_enum("tangentAxis", "tng", 2, &["X", "Y", "Z", "-X", "-Y", "-Z", "None"]),
                store_enum("normalOverride", "novr", 0, &["Auto", "Rail Curve"]),
                geometry_in("railCurve", "rlcrv"),
                geometry_in("originalRailCurve", "orlcrv"),
                store_enum("relativeSpaceMode", "rsmd", 0, &SPACES),
                matrix_store("relativeSpaceMatrix", "rsmat"),
                base("uvSetName", "msn", PlugType::String, PlugValue::String(String::new()), STORE),
                multi(mout("outputMatrix", "omat")),
            ],
            proximity_pin),
        Spec("remapColor", "utility/color:drawdb/shader/operation/remapColor", "Remaps red, green and blue, each through its own ramp.",
            remap_attrs([("red", "r"), ("green", "g"), ("blue", "b")]), remap_color),
        Spec("remapHsv", "utility/color:drawdb/shader/operation/remapHsv", "Remaps hue, saturation and value, each through its own ramp.",
            remap_attrs([("hue", "h"), ("saturation", "s"), ("value", "v")]), remap_hsv),
        Spec("timeFunction", "math/conversion", "Turns a number into a function of time, for constraints and expressions blended in clips.",
            vec![with_flags(fin("input", "i", 0.0), STORE), base("output", "o", PlugType::Data, PlugValue::Data(None), OUT | flags::HIDDEN)], time_function),
        Spec("sampler", "math/rounding", "Samples a function over an interval: value[k] at minimum + k step.", function_attrs(), sampler),
        Spec("symmetryConstraint", "animation", "Mirrors a joint: the target's place across the symmetry planes, its rotate as it is.",
            symmetry_attrs(), symmetry),
    ]
}

fn limited(mut a: AttrDef, lo: f64, hi: f64) -> AttrDef {
    a.min_value = Some(lo);
    a.max_value = Some(hi);
    a
}
