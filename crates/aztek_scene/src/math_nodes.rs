//! The reference's math nodes (its 2024 and later releases): one table, each node made twice. The plain node's values
//! have no unit; its "DL" twin's are distances (`doubleLinear`), which the reference keeps for older scenes and
//! deprecates in favour of the plain one. Angles are angles in both. Both compute the same way: values are stored in
//! centimetres and radians, so only what scripts, the interface and files see differs.
//!
//! Attributes, defaults, limits and classifications are the 2027 release's (PARITY.md says where they were read).

use std::sync::OnceLock;

use aztek_ops::units::Unit;
use glam::DVec3;

use super::*;

/// A value attr: plain, or a distance in the DL twin.
type U = Unit;

fn unit(mut a: AttrDef, u: U) -> AttrDef {
    a.unit = u;
    a
}
fn sin_(name: &str, short: &str, d: f64, u: U) -> AttrDef {
    unit(fin(name, short, d), u)
}
fn sout(name: &str, short: &str, u: U) -> AttrDef {
    unit(fout(name, short), u)
}
fn limited(mut a: AttrDef, lo: Option<f64>, hi: Option<f64>) -> AttrDef {
    a.min_value = lo;
    a.max_value = hi;
    a
}
fn vin_(name: &str, short: &str, u: U) -> AttrDef {
    unit(vin(name, short, [0.0; 3]), u)
}
fn vout_(name: &str, short: &str, u: U) -> AttrDef {
    unit(vout(name, short), u)
}
fn matrix_in(name: &str, short: &str) -> AttrDef {
    base(name, short, PlugType::Matrix, PlugValue::Matrix(glam::DMat4::IDENTITY), IN)
}
/// Four doubles, X Y Z W, under one name (a row or column of a matrix).
fn four_out(name: &str, short: &str, u: U) -> Vec<AttrDef> {
    quat(name, short, OUT).into_iter().map(|a| unit(a, u)).collect()
}
fn order_in() -> AttrDef {
    let mut a = en("rotateOrder", "ro", 0, &ORDERS);
    a.flags = STORE;
    a
}

fn unary(u: U) -> Vec<AttrDef> {
    vec![sin_("input", "i", 0.0, u), sout("output", "o", u)]
}
fn binary(u: U) -> Vec<AttrDef> {
    vec![sin_("input1", "i1", 0.0, u), sin_("input2", "i2", 0.0, u), sout("output", "o", u)]
}
fn many(u: U) -> Vec<AttrDef> {
    vec![multi(sin_("input", "i", 0.0, u)), sout("output", "o", u)]
}
fn compare(u: U) -> Vec<AttrDef> {
    vec![sin_("input1", "i1", 0.0, u), sin_("input2", "i2", 0.0, u), bout("output", "o")]
}
fn compare_many(u: U) -> Vec<AttrDef> {
    many(u)
}
/// An angle in, a plain number (or a distance) out: sin, cos, tan.
fn trig(u: U) -> Vec<AttrDef> {
    vec![sin_("input", "i", 0.0, Unit::Angle), sout("output", "o", u)]
}
/// A number in, an angle out: acos, asin, atan. acos and asin take -1 to 1.
fn arc(u: U) -> Vec<AttrDef> {
    vec![sin_("input", "i", 0.0, u), sout("output", "o", Unit::Angle)]
}
fn arc_limited(u: U) -> Vec<AttrDef> {
    vec![limited(sin_("input", "i", 0.0, u), Some(-1.0), Some(1.0)), sout("output", "o", Unit::Angle)]
}
fn from_matrix_vec(u: U) -> Vec<AttrDef> {
    vec![matrix_in("input", "i"), vout_("output", "o", u)]
}
fn index_of_matrix(u: U) -> Vec<AttrDef> {
    let mut a = vec![limited(iin("input", "i", 0), Some(0.0), Some(3.0)), matrix_in("matrix", "m")];
    a.extend(four_out("output", "o", u));
    a
}
fn two_vectors(u: U) -> Vec<AttrDef> {
    vec![vin_("input1", "i1", u), vin_("input2", "i2", u)]
}
fn by_matrix(u: U) -> Vec<AttrDef> {
    vec![kids_named(vin_("input", "i", u), ["x", "y", "z"]), matrix_in("matrix", "m"), vout_("output", "o", u)]
}

// ---------------------------------------------------------------------------------------------------------------------
// Computes

fn absolute(d: &mut NodeData) {
    let o = f(d, "input").abs();
    set(d, "output", o);
}
fn negate(d: &mut NodeData) {
    let o = -f(d, "input");
    set(d, "output", o);
}
fn ceil(d: &mut NodeData) {
    let o = f(d, "input").ceil();
    set(d, "output", o);
}
fn floor(d: &mut NodeData) {
    let o = f(d, "input").floor();
    set(d, "output", o);
}
/// To the nearest whole number; a half rounds up, as the reference's page says (0.5 gives 1).
fn round(d: &mut NodeData) {
    let o = (f(d, "input") + 0.5).floor();
    set(d, "output", o);
}
fn truncate(d: &mut NodeData) {
    let o = f(d, "input").trunc();
    set(d, "output", o);
}
fn sum(d: &mut NodeData) {
    let o: f64 = scalars(d, "input").iter().sum();
    set(d, "output", o);
}
fn product(d: &mut NodeData) {
    let o: f64 = scalars(d, "input").iter().product();
    set(d, "output", o);
}
fn average(d: &mut NodeData) {
    let x = scalars(d, "input");
    let o = if x.is_empty() { 0.0 } else { x.iter().sum::<f64>() / x.len() as f64 };
    set(d, "output", o);
}
fn max(d: &mut NodeData) {
    let o = scalars(d, "input").into_iter().reduce(f64::max).unwrap_or(0.0);
    set(d, "output", o);
}
fn min(d: &mut NodeData) {
    let o = scalars(d, "input").into_iter().reduce(f64::min).unwrap_or(0.0);
    set(d, "output", o);
}
fn subtract(d: &mut NodeData) {
    let o = f(d, "input1") - f(d, "input2");
    set(d, "output", o);
}
fn quotient(d: &mut NodeData) {
    let o = divide(f(d, "input1"), f(d, "input2"));
    set(d, "output", o);
}
fn add(d: &mut NodeData) {
    let o = f(d, "input1") + f(d, "input2");
    set(d, "output", o);
}
fn mult(d: &mut NodeData) {
    let o = f(d, "input1") * f(d, "input2");
    set(d, "output", o);
}
fn power(d: &mut NodeData) {
    let o = f(d, "input").powf(f(d, "exponent"));
    set(d, "output", if o.is_finite() { o } else { 0.0 });
}
/// The input wrapped into the modulus: the result has the modulus's sign. A modulus of 0 gives 0.
fn modulo(d: &mut NodeData) {
    let (x, m) = (f(d, "input"), f(d, "modulus"));
    let o = if m == 0.0 { 0.0 } else { x - m * (x / m).floor() };
    set(d, "output", o);
}
fn log(d: &mut NodeData) {
    let (x, b) = (f(d, "input"), f(d, "base"));
    let o = if x > 0.0 && b > 0.0 && b != 1.0 { x.ln() / b.ln() } else { 0.0 };
    set(d, "output", o);
}
fn lerp(d: &mut NodeData) {
    let (a, c, w) = (f(d, "input1"), f(d, "input2"), f(d, "weight"));
    set(d, "output", a + (c - a) * w);
}
/// Where `interpolation` sits from input1 (0) to input2 (1).
fn inverse_lerp(d: &mut NodeData) {
    let (a, c, x) = (f(d, "input1"), f(d, "input2"), f(d, "interpolation"));
    let o = if c == a { 0.0 } else { (x - a) / (c - a) };
    set(d, "output", o);
}
fn clamp_range(d: &mut NodeData) {
    let o = f(d, "input").max(f(d, "minimum")).min(f(d, "maximum"));
    set(d, "output", o);
}
/// 0 at or below the left edge, 1 at or above the right, the Hermite curve between. A right edge below the left is the
/// left.
fn smooth_step(d: &mut NodeData) {
    let (x, l) = (f(d, "input"), f(d, "leftEdge"));
    let r = f(d, "rightEdge").max(l);
    let o = if x <= l {
        0.0
    } else if x >= r {
        1.0
    } else {
        smooth((x - l) / (r - l))
    };
    set(d, "output", o);
}
fn equal(d: &mut NodeData) {
    let o = (f(d, "input1") - f(d, "input2")).abs() <= f(d, "epsilon");
    set(d, "output", o);
}
fn greater_than(d: &mut NodeData) {
    let o = f(d, "input1") > f(d, "input2");
    set(d, "output", o);
}
fn less_than(d: &mut NodeData) {
    let o = f(d, "input1") < f(d, "input2");
    set(d, "output", o);
}
fn and(d: &mut NodeData) {
    let o = b(d, "input1") && b(d, "input2");
    set(d, "output", o);
}
fn or(d: &mut NodeData) {
    let o = b(d, "input1") || b(d, "input2");
    set(d, "output", o);
}
fn not(d: &mut NodeData) {
    let o = !b(d, "input");
    set(d, "output", o);
}
fn sine(d: &mut NodeData) {
    let o = f(d, "input").sin();
    set(d, "output", o);
}
fn cosine(d: &mut NodeData) {
    let o = f(d, "input").cos();
    set(d, "output", o);
}
fn tangent(d: &mut NodeData) {
    let o = f(d, "input").tan();
    set(d, "output", o);
}
fn arc_cosine(d: &mut NodeData) {
    let o = f(d, "input").clamp(-1.0, 1.0).acos();
    set(d, "output", o);
}
fn arc_sine(d: &mut NodeData) {
    let o = f(d, "input").clamp(-1.0, 1.0).asin();
    set(d, "output", o);
}
fn arc_tangent(d: &mut NodeData) {
    let o = f(d, "input").atan();
    set(d, "output", o);
}
/// The angle of (input2, input1): atan2(input1, input2).
fn arc_tangent2(d: &mut NodeData) {
    let o = f(d, "input1").atan2(f(d, "input2"));
    set(d, "output", o);
}
fn pi(d: &mut NodeData) {
    set(d, "output", std::f64::consts::PI);
}
fn cross(d: &mut NodeData) {
    let o = v(d, "input1").cross(v(d, "input2"));
    set(d, "output", o);
}
fn dot(d: &mut NodeData) {
    let o = v(d, "input1").dot(v(d, "input2"));
    set(d, "output", o);
}
fn length(d: &mut NodeData) {
    let o = v(d, "input").length();
    set(d, "output", o);
}
fn normalize(d: &mut NodeData) {
    let o = v(d, "input").normalize_or_zero();
    set(d, "output", o);
}
fn determinant(d: &mut NodeData) {
    let o = m(d, "input").determinant();
    set(d, "output", o);
}
// A matrix here holds the reference's rows as its columns: row k is col(k), column k is row(k).
fn axis_from_matrix(d: &mut NodeData) {
    let k = i(d, "axis").clamp(0, 5) as usize;
    let a = m(d, "input").col(k % 3).truncate();
    // + 0.0 keeps a reversed zero from reading -0.
    set(d, "output", if k >= 3 { -a + DVec3::ZERO } else { a });
}
fn row_from_matrix(d: &mut NodeData) {
    let r = m(d, "matrix").col(i(d, "input").clamp(0, 3) as usize);
    set_quat(d, "output", glam::DQuat::from_xyzw(r.x, r.y, r.z, r.w));
}
fn column_from_matrix(d: &mut NodeData) {
    let c = m(d, "matrix").row(i(d, "input").clamp(0, 3) as usize);
    set_quat(d, "output", glam::DQuat::from_xyzw(c.x, c.y, c.z, c.w));
}
fn translation_from_matrix(d: &mut NodeData) {
    let o = m(d, "input").w_axis.truncate();
    set(d, "output", o);
}
fn scale_from_matrix(d: &mut NodeData) {
    let (_, _, s, _) = decompose_full(&m(d, "input"));
    set(d, "output", s);
}
fn rotation_from_matrix(d: &mut NodeData) {
    let (_, r, _, _) = decompose_full(&m(d, "input"));
    set(d, "output", matrix_to_euler(&r, RotateOrder::Xyz));
}
fn point_by_matrix(d: &mut NodeData) {
    let o = m(d, "matrix").transform_point3(v(d, "input"));
    set(d, "output", o);
}
fn vector_by_matrix(d: &mut NodeData) {
    let o = m(d, "matrix").transform_vector3(v(d, "input"));
    set(d, "output", o);
}
fn rotate_vector(d: &mut NodeData) {
    let r = euler_to_matrix(v(d, "rotate"), order(d, "rotateOrder"));
    let o = r.transform_vector3(v(d, "input"));
    set(d, "output", o);
}
fn point_matrix_mult(d: &mut NodeData) {
    let (p, mm) = (v(d, "inPoint"), m(d, "inMatrix"));
    let o = if b(d, "vectorMultiply") { mm.transform_vector3(p) } else { mm.transform_point3(p) };
    set(d, "output", o);
}
fn distance_between(d: &mut NodeData) {
    let x = m(d, "inMatrix1").transform_point3(v(d, "point1"));
    let y = m(d, "inMatrix2").transform_point3(v(d, "point2"));
    set(d, "distance", x.distance(y));
}
fn angle_between(d: &mut NodeData) {
    let (x, y) = (v(d, "vector1").normalize_or_zero(), v(d, "vector2").normalize_or_zero());
    let angle = x.angle_between(y);
    let axis = x.cross(y).normalize_or_zero();
    set(d, "axis", axis);
    set(d, "angle", angle);
    let q = if axis == DVec3::ZERO { DQuat::IDENTITY } else { DQuat::from_axis_angle(axis, angle) };
    set(d, "euler", quat_euler(q, RotateOrder::Xyz));
}

// ---------------------------------------------------------------------------------------------------------------------
// The table

/// Which twins a node has: the plain one, the DL one, or both.
#[derive(Clone, Copy, PartialEq)]
enum Has {
    Both,
    Plain,
    Dl,
}

/// name, its group in the classification, what it does, which twins, attributes for a unit, compute.
type Row = (&'static str, &'static str, &'static str, Has, fn(U) -> Vec<AttrDef>, Compute);

fn rows() -> Vec<Row> {
    use Has::*;
    vec![
        // operation
        ("absolute", "operation", "The absolute value of the input.", Both, unary, absolute),
        ("negate", "operation", "The input negated.", Both, unary, negate),
        ("sum", "operation", "The sum of its inputs.", Both, many, sum),
        ("multiply", "operation", "The product of its inputs.", Both, many, product),
        ("average", "operation", "The mean of its inputs.", Both, many, average),
        ("subtract", "operation", "input1 - input2.", Both, binary, subtract),
        ("divide", "operation", "input1 / input2 (0 when input2 is 0).", Both, |u| vec![sin_("input1", "i1", 0.0, u), sin_("input2", "i2", 1.0, u), sout("output", "o", u)], quotient),
        ("power", "operation", "The input to the power of the exponent.", Both, |u| vec![sin_("input", "i", 0.0, u), sin_("exponent", "e", 2.0, u), sout("output", "o", u)], power),
        ("modulo", "operation", "The input wrapped around the modulus.", Both, |u| vec![sin_("input", "i", 0.0, u), sin_("modulus", "m", 1.0, u), sout("output", "o", u)], modulo),
        ("log", "operation", "The logarithm of the input in the given base.", Both,
            |u| vec![limited(sin_("base", "e", 2.0, u), Some(0.0), None), limited(sin_("input", "i", 0.0, u), Some(0.0), None), sout("output", "o", u)], log),
        ("lerp", "operation", "From input1 to input2 by the weight.", Both, |u| vec![sin_("input1", "i1", 0.0, u), sin_("input2", "i2", 0.0, u), sin_("weight", "w", 0.0, u), sout("output", "o", u)], lerp),
        ("inverseLerp", "operation", "The weight that gives the interpolated value from input1 to input2.", Both,
            |u| vec![sin_("input1", "i1", 0.0, u), sin_("input2", "i2", 0.0, u), sin_("interpolation", "i", 0.0, u), sout("output", "o", u)], inverse_lerp),
        // comparison
        ("equal", "comparison", "Whether input1 and input2 are within epsilon of each other.", Both,
            |u| vec![sin_("epsilon", "e", 0.0, u), sin_("input1", "i1", 0.0, u), sin_("input2", "i2", 0.0, u), bout("output", "o")], equal),
        ("greaterThan", "comparison", "Whether input1 is greater than input2.", Both, compare, greater_than),
        ("lessThan", "comparison", "Whether input1 is less than input2.", Both, compare, less_than),
        ("max", "comparison", "The largest of its inputs.", Both, compare_many, max),
        ("min", "comparison", "The smallest of its inputs.", Both, compare_many, min),
        ("and", "comparison", "Whether both inputs are on.", Plain, |_| vec![bin("input1", "i1", false), bin("input2", "i2", false), bout("output", "o")], and),
        ("or", "comparison", "Whether either input is on.", Plain, |_| vec![bin("input1", "i1", false), bin("input2", "i2", false), bout("output", "o")], or),
        ("not", "comparison", "The input switched.", Plain, |_| vec![bin("input", "i", false), bout("output", "o")], not),
        // rounding
        ("ceil", "rounding", "The input rounded up.", Both, unary, ceil),
        ("floor", "rounding", "The input rounded down.", Both, unary, floor),
        ("round", "rounding", "The input to the nearest whole number; a half rounds up.", Both, unary, round),
        ("truncate", "rounding", "The input with its fraction dropped.", Both, unary, truncate),
        ("clampRange", "rounding", "The input kept between minimum and maximum.", Both,
            |u| vec![sin_("input", "i", 0.0, u), sin_("maximum", "max", 1.0, u), sin_("minimum", "min", 0.0, u), sout("output", "o", u)], clamp_range),
        ("smoothStep", "rounding", "0 to 1 along a smooth curve from the left edge to the right.", Both,
            |u| vec![sin_("input", "i", 0.0, u), sin_("leftEdge", "le", 0.0, u), sin_("rightEdge", "re", 1.0, u), sout("output", "o", u)], smooth_step),
        // trigonometry
        ("sin", "trigonometry", "The sine of an angle.", Both, trig, sine),
        ("cos", "trigonometry", "The cosine of an angle.", Both, trig, cosine),
        ("tan", "trigonometry", "The tangent of an angle.", Both, trig, tangent),
        ("acos", "trigonometry", "The angle whose cosine is the input.", Both, arc_limited, arc_cosine),
        ("asin", "trigonometry", "The angle whose sine is the input.", Both, arc_limited, arc_sine),
        ("atan", "trigonometry", "The angle whose tangent is the input.", Both, arc, arc_tangent),
        ("atan2", "trigonometry", "The angle of input1 over input2, in the right quadrant.", Both,
            |u| vec![sin_("input1", "i1", 0.0, u), sin_("input2", "i2", 0.0, u), sout("output", "o", Unit::Angle)], arc_tangent2),
        ("pi", "trigonometry", "Pi, as an angle.", Plain, |_| vec![unit(base("output", "o", PlugType::Float, PlugValue::Float(std::f64::consts::PI), OUT), Unit::Angle)], pi),
        // transformation
        ("crossProduct", "transformation", "input1 crossed with input2.", Both, |u| {
            let mut a = two_vectors(u);
            a.push(vout_("output", "o", u));
            a
        }, cross),
        ("dotProduct", "transformation", "input1 dotted with input2.", Both, |u| {
            let mut a = two_vectors(u);
            a.push(sout("output", "o", u));
            a
        }, dot),
        ("length", "transformation", "The length of a vector.", Both, |u| vec![vin_("input", "i", u), sout("output", "o", u)], length),
        ("normalize", "transformation", "A vector made one long.", Both, |u| vec![vin_("input", "i", u), vout_("output", "o", u)], normalize),
        ("determinant", "transformation", "A matrix's determinant.", Both, |u| vec![matrix_in("input", "i"), sout("output", "o", u)], determinant),
        ("axisFromMatrix", "transformation", "A matrix's X, Y or Z axis, or one of them reversed.", Both, |u| {
            let mut ax = en("axis", "op", 0, &["X", "Y", "Z", "-X", "-Y", "-Z"]);
            ax.flags = STORE;
            vec![matrix_in("input", "i"), ax, vout_("output", "o", u)]
        }, axis_from_matrix),
        ("rowFromMatrix", "transformation", "The row of a matrix at the input index.", Both, index_of_matrix, row_from_matrix),
        ("columnFromMatrix", "transformation", "The column of a matrix at the input index.", Both, index_of_matrix, column_from_matrix),
        ("translationFromMatrix", "transformation", "A matrix's translation.", Both, from_matrix_vec, translation_from_matrix),
        ("scaleFromMatrix", "transformation", "A matrix's scale.", Both, from_matrix_vec, scale_from_matrix),
        ("rotationFromMatrix", "transformation", "A matrix's rotation, as XYZ Euler angles.", Plain, |_| vec![matrix_in("input", "i"), vout_("output", "o", Unit::Angle)], rotation_from_matrix),
        ("multiplyPointByMatrix", "transformation", "A point through a matrix.", Both, by_matrix, point_by_matrix),
        ("multiplyVectorByMatrix", "transformation", "A vector through a matrix: its translation left out.", Both, by_matrix, vector_by_matrix),
        ("rotateVector", "transformation", "A vector turned by a rotation, in a rotation order.", Both, |u| {
            vec![vin_("input", "i", u), unit(vin("rotate", "r", [0.0; 3]), Unit::Angle), vout_("output", "o", u), order_in()]
        }, rotate_vector),
        ("distanceBetween", "transformation", "The distance between two points, each through its matrix.", Both, |u| {
            let p = |n: &str, s: &str| {
                let mut a = vin_(n, s, u);
                a.flags = STORE;
                a
            };
            let mm = |n: &str, s: &str| base(n, s, PlugType::Matrix, PlugValue::Matrix(glam::DMat4::IDENTITY), STORE);
            vec![p("point1", "p1"), mm("inMatrix1", "im1"), p("point2", "p2"), mm("inMatrix2", "im2"), sout("distance", "d", u)]
        }, distance_between),
        ("angleBetween", "operation", "The rotation from vector1 to vector2: axis and angle, and as Euler angles.", Both, |u| {
            let vec = |n: &str, s: &str, dd: [f64; 3]| {
                let mut a = unit(vin(n, s, dd), u);
                a.flags = STORE;
                a
            };
            vec![
                vec("vector1", "v1", [0.0, 1.0, 0.0]),
                vec("vector2", "v2", [0.0, 0.0, 1.0]),
                unit(vec_kids("axis", "ax", [0.0, 0.0, 1.0], OUT, XYZ), u),
                unit(fout("angle", "a"), Unit::Angle),
                unit(vout("euler", "eu"), Unit::Angle),
            ]
        }, angle_between),
        // DL only: the twins of the classic nodes.
        ("add", "operation", "input1 + input2.", Dl, |u| {
            let i = |n: &str, s: &str| {
                let mut a = sin_(n, s, 0.0, u);
                a.flags = STORE;
                a
            };
            vec![i("input1", "i1"), i("input2", "i2"), sout("output", "o", u)]
        }, add),
        ("mult", "operation", "input1 * input2.", Dl, |u| {
            let i = |n: &str, s: &str| {
                let mut a = sin_(n, s, 0.0, u);
                a.flags = STORE;
                a
            };
            vec![i("input1", "i1"), i("input2", "i2"), sout("output", "o", u)]
        }, mult),
        ("pointMatrixMult", "transformation", "A point (or a vector) through a matrix.", Dl, |u| {
            let mut p = kids_named(vin_("inPoint", "ip", u), ["ipx", "ipy", "ipz"]);
            p.flags = IN;
            let mut vm = bin("vectorMultiply", "vm", false);
            vm.flags = STORE;
            vec![matrix_in("inMatrix", "im"), p, vm, vout_("output", "o", u)]
        }, point_matrix_mult),
    ]
}

/// The DL twins' classification lines up with the plain one's: `drawdb/shader/operation/<plain>` and its group.
/// addDL and multDL draw as the classic nodes they twin; pointMatrixMultDL has no drawdb entry.
fn classification_of(plain: &str, group: &str, dl: bool) -> String {
    let math = if dl { "mathLinear" } else { "math" };
    let draw = match plain {
        "add" => Some("addDoubleLinear"),
        "mult" => Some("multDoubleLinear"),
        "pointMatrixMult" => None,
        p => Some(p),
    };
    match draw {
        Some(p) => format!("{math}/{group}:drawdb/shader/operation/{p}"),
        None => format!("{math}/{group}"),
    }
}

struct Made {
    name: &'static str,
    class: &'static str,
    doc: &'static str,
    attrs: fn(U) -> Vec<AttrDef>,
    unit: U,
    compute: Compute,
}

fn made() -> &'static [Made] {
    static MADE: OnceLock<Vec<Made>> = OnceLock::new();
    MADE.get_or_init(|| {
        let leak = |s: String| -> &'static str { Box::leak(s.into_boxed_str()) };
        let mut out = Vec::new();
        for (plain, group, doc, has, attrs, compute) in rows() {
            if has != Has::Dl {
                out.push(Made { name: plain, class: leak(classification_of(plain, group, false)), doc, attrs, unit: Unit::None, compute });
            }
            if has != Has::Plain {
                let name = leak(format!("{plain}DL"));
                let doc = leak(format!("{doc} In distances: the reference keeps it for older scenes."));
                out.push(Made { name, class: leak(classification_of(plain, group, true)), doc, attrs, unit: Unit::Linear, compute });
            }
        }
        out
    })
}

/// Every math node, plain and DL.
pub(super) fn specs() -> Vec<Spec> {
    made().iter().map(|m| Spec(m.name, m.class, m.doc, (m.attrs)(m.unit), m.compute)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_node_and_its_twin() {
        let s = specs();
        assert_eq!(s.len(), 98);
        let names: std::collections::HashSet<&str> = s.iter().map(|x| x.0).collect();
        assert_eq!(names.len(), s.len());
        for n in ["sumDL", "addDL", "multDL", "pointMatrixMultDL", "distanceBetweenDL", "angleBetweenDL", "and", "pi", "rotationFromMatrix"] {
            assert!(names.contains(n), "{n}");
        }
        let dl = s.iter().find(|x| x.0 == "clampRangeDL").unwrap();
        assert!(dl.3.iter().all(|a| a.unit == Unit::Linear));
        assert_eq!(dl.1, "mathLinear/rounding:drawdb/shader/operation/clampRange");
    }
}
