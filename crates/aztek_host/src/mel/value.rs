//! MEL's values and its conversions between types.

use serde_json::{json, Value as J};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ty {
    Int,
    Float,
    Str,
    Vector,
    Matrix,
    IntArr,
    FloatArr,
    StrArr,
    VecArr,
}

impl Ty {
    pub fn array_of(self) -> Ty {
        match self {
            Ty::Int | Ty::IntArr => Ty::IntArr,
            Ty::Float | Ty::FloatArr => Ty::FloatArr,
            Ty::Str | Ty::StrArr => Ty::StrArr,
            Ty::Vector | Ty::VecArr => Ty::VecArr,
            Ty::Matrix => Ty::Matrix,
        }
    }
    pub fn element(self) -> Ty {
        match self {
            Ty::IntArr => Ty::Int,
            Ty::FloatArr => Ty::Float,
            Ty::StrArr => Ty::Str,
            Ty::VecArr => Ty::Vector,
            t => t,
        }
    }
    pub fn is_array(self) -> bool {
        matches!(self, Ty::IntArr | Ty::FloatArr | Ty::StrArr | Ty::VecArr)
    }
    pub fn name(self) -> &'static str {
        match self {
            Ty::Int => "int",
            Ty::Float => "float",
            Ty::Str => "string",
            Ty::Vector => "vector",
            Ty::Matrix => "matrix",
            Ty::IntArr => "int[]",
            Ty::FloatArr => "float[]",
            Ty::StrArr => "string[]",
            Ty::VecArr => "vector[]",
        }
    }
    pub fn default_value(self) -> Value {
        match self {
            Ty::Int => Value::Int(0),
            Ty::Float => Value::Float(0.0),
            Ty::Str => Value::Str(String::new()),
            Ty::Vector => Value::Vector([0.0; 3]),
            Ty::Matrix => Value::Matrix(0, 0, Vec::new()),
            Ty::IntArr => Value::IntArr(Vec::new()),
            Ty::FloatArr => Value::FloatArr(Vec::new()),
            Ty::StrArr => Value::StrArr(Vec::new()),
            Ty::VecArr => Value::VecArr(Vec::new()),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Void,
    Int(i32),
    Float(f64),
    Str(String),
    Vector([f64; 3]),
    /// Rows, columns, values row by row.
    Matrix(usize, usize, Vec<f64>),
    IntArr(Vec<i32>),
    FloatArr(Vec<f64>),
    StrArr(Vec<String>),
    VecArr(Vec<[f64; 3]>),
}

/// A float as the reference prints it: six decimals at most, trailing zeros dropped; very small or large in exponent form.
pub fn fmt_float(f: f64) -> String {
    if f.is_nan() {
        return "nan".into();
    }
    if f.is_infinite() {
        return if f > 0.0 { "inf".into() } else { "-inf".into() };
    }
    let a = f.abs();
    if a != 0.0 && !(1e-4..1e15).contains(&a) {
        let t = format!("{f:.5e}");
        let (m, e) = t.split_once('e').unwrap_or((&t, "0"));
        let m = m.trim_end_matches('0').trim_end_matches('.');
        let (sign, digits) = match e.strip_prefix('-') {
            Some(d) => ('-', d),
            None => ('+', e),
        };
        return format!("{m}e{sign}{digits:0>2}");
    }
    let t = format!("{f:.6}");
    let t = t.trim_end_matches('0').trim_end_matches('.');
    if t == "-0" || t.is_empty() {
        "0".into()
    } else {
        t.to_string()
    }
}

/// C's atoi: the leading digits, 0 when there are none.
fn str_to_int(s: &str) -> i32 {
    let s = s.trim_start();
    let (neg, rest) = match s.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, s.strip_prefix('+').unwrap_or(s)),
    };
    let mut v: i64 = 0;
    for c in rest.chars() {
        let Some(d) = c.to_digit(10) else { break };
        v = (v * 10 + d as i64).min(i32::MAX as i64 + 1);
    }
    (if neg { -v } else { v }) as i32
}

/// C's atof: the longest leading number, 0 when there is none.
fn str_to_float(s: &str) -> f64 {
    let s = s.trim_start();
    let b = s.as_bytes();
    let mut e = 0;
    if e < b.len() && (b[e] == b'-' || b[e] == b'+') {
        e += 1;
    }
    let digits = |e: &mut usize| {
        let start = *e;
        while *e < b.len() && b[*e].is_ascii_digit() {
            *e += 1;
        }
        *e > start
    };
    let mut any = digits(&mut e);
    if e < b.len() && b[e] == b'.' {
        e += 1;
        any |= digits(&mut e);
    }
    if !any {
        return 0.0;
    }
    if e < b.len() && (b[e] == b'e' || b[e] == b'E') {
        let mut x = e + 1;
        if x < b.len() && (b[x] == b'-' || b[x] == b'+') {
            x += 1;
        }
        if digits(&mut x) {
            e = x;
        }
    }
    s[..e].parse().unwrap_or(0.0)
}

fn mag(v: [f64; 3]) -> f64 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

impl Value {
    pub fn ty(&self) -> Option<Ty> {
        Some(match self {
            Value::Void => return None,
            Value::Int(_) => Ty::Int,
            Value::Float(_) => Ty::Float,
            Value::Str(_) => Ty::Str,
            Value::Vector(_) => Ty::Vector,
            Value::Matrix(..) => Ty::Matrix,
            Value::IntArr(_) => Ty::IntArr,
            Value::FloatArr(_) => Ty::FloatArr,
            Value::StrArr(_) => Ty::StrArr,
            Value::VecArr(_) => Ty::VecArr,
        })
    }
    pub fn type_name(&self) -> &'static str {
        self.ty().map(Ty::name).unwrap_or("void")
    }
    pub fn is_array(&self) -> bool {
        self.ty().is_some_and(Ty::is_array)
    }
    pub fn len(&self) -> usize {
        match self {
            Value::IntArr(a) => a.len(),
            Value::FloatArr(a) => a.len(),
            Value::StrArr(a) => a.len(),
            Value::VecArr(a) => a.len(),
            Value::Str(s) => s.chars().count(),
            _ => 0,
        }
    }
    /// The elements of an array, one value each; a scalar is its own only element.
    pub fn elements(&self) -> Vec<Value> {
        match self {
            Value::IntArr(a) => a.iter().map(|x| Value::Int(*x)).collect(),
            Value::FloatArr(a) => a.iter().map(|x| Value::Float(*x)).collect(),
            Value::StrArr(a) => a.iter().map(|x| Value::Str(x.clone())).collect(),
            Value::VecArr(a) => a.iter().map(|x| Value::Vector(*x)).collect(),
            Value::Void => Vec::new(),
            v => vec![v.clone()],
        }
    }
    pub fn to_int(&self) -> i32 {
        match self {
            Value::Int(i) => *i,
            Value::Float(f) => *f as i32,
            Value::Str(s) => str_to_int(s),
            Value::Vector(v) => mag(*v) as i32,
            a if a.is_array() => a.elements().first().map(Value::to_int).unwrap_or(0),
            _ => 0,
        }
    }
    pub fn to_float(&self) -> f64 {
        match self {
            Value::Int(i) => *i as f64,
            Value::Float(f) => *f,
            Value::Str(s) => str_to_float(s),
            Value::Vector(v) => mag(*v),
            a if a.is_array() => a.elements().first().map(Value::to_float).unwrap_or(0.0),
            _ => 0.0,
        }
    }
    pub fn to_vector(&self) -> [f64; 3] {
        match self {
            Value::Vector(v) => *v,
            Value::Str(s) => {
                let mut o = [0.0; 3];
                for (i, p) in s.split_whitespace().take(3).enumerate() {
                    o[i] = str_to_float(p);
                }
                o
            }
            Value::FloatArr(a) if a.len() >= 3 => [a[0], a[1], a[2]],
            v => [v.to_float(); 3],
        }
    }
    /// The value as text: what string + value and string($value) give. Arrays have no string form in MEL, but this is
    /// also what print and whatever else needs a display form use.
    pub fn to_text(&self) -> String {
        match self {
            Value::Void => String::new(),
            Value::Int(i) => i.to_string(),
            Value::Float(f) => fmt_float(*f),
            Value::Str(s) => s.clone(),
            Value::Vector(v) => format!("{} {} {}", fmt_float(v[0]), fmt_float(v[1]), fmt_float(v[2])),
            Value::Matrix(r, c, d) => {
                let rows: Vec<String> = (0..*r).map(|i| (0..*c).map(|j| fmt_float(d[i * c + j])).collect::<Vec<_>>().join(", ")).collect();
                format!("<<{}>>", rows.join("; "))
            }
            a => a.elements().iter().map(Value::to_text).collect::<Vec<_>>().join(" "),
        }
    }
    pub fn truth(&self) -> bool {
        match self {
            Value::Void => false,
            Value::Int(i) => *i != 0,
            Value::Float(f) => *f != 0.0,
            Value::Str(s) => !s.is_empty(),
            Value::Vector(v) => v.iter().any(|x| *x != 0.0),
            Value::Matrix(_, _, d) => d.iter().any(|x| *x != 0.0),
            a => a.len() > 0,
        }
    }

    /// The conversion on assigning to a variable or passing to a parameter of type `t`. An array of one element goes to
    /// a scalar, since some commands here return a list where the reference's return one value; longer ones refuse, as the reference.
    pub fn coerce(&self, t: Ty) -> Result<Value, String> {
        if self.ty() == Some(t) {
            return Ok(self.clone());
        }
        if matches!(self, Value::Void) {
            return Ok(t.default_value());
        }
        if self.is_array() && !t.is_array() {
            let e = self.elements();
            if e.len() == 1 {
                return e[0].coerce(t);
            }
            return Err(format!("Cannot convert data of type {} to type {}.", self.type_name(), t.name()));
        }
        if let Value::Matrix(..) = self {
            return Err(format!("Cannot convert data of type matrix to type {}.", t.name()));
        }
        Ok(match t {
            Ty::Int => Value::Int(self.to_int()),
            Ty::Float => Value::Float(self.to_float()),
            Ty::Str => Value::Str(self.to_text()),
            Ty::Vector => Value::Vector(self.to_vector()),
            Ty::Matrix => return Err(format!("Cannot convert data of type {} to type matrix.", self.type_name())),
            Ty::IntArr => Value::IntArr(self.elements().iter().map(Value::to_int).collect()),
            Ty::FloatArr => Value::FloatArr(self.elements().iter().map(Value::to_float).collect()),
            Ty::StrArr => Value::StrArr(self.elements().iter().map(Value::to_text).collect()),
            Ty::VecArr => Value::VecArr(self.elements().iter().map(Value::to_vector).collect()),
        })
    }

    pub fn to_json(&self) -> J {
        match self {
            Value::Void => J::Null,
            Value::Int(i) => json!(i),
            Value::Float(f) => float_json(*f),
            Value::Str(s) => json!(s),
            Value::Vector(v) => json!(v),
            Value::Matrix(r, c, d) => J::Array((0..*r).map(|i| json!(d[i * c..(i + 1) * c])).collect()),
            Value::IntArr(a) => json!(a),
            Value::FloatArr(a) => J::Array(a.iter().map(|f| float_json(*f)).collect()),
            Value::StrArr(a) => json!(a),
            Value::VecArr(a) => json!(a),
        }
    }

    /// A command's result: lists become string[], float[] or int[], nested lists flattened; scalars stay scalars.
    pub fn from_json(v: &J) -> Value {
        match v {
            J::Null => Value::Void,
            J::Bool(b) => Value::Int(*b as i32),
            J::Number(n) => match n.as_i64() {
                Some(i) => Value::Int(i as i32),
                None => Value::Float(n.as_f64().unwrap_or(0.0)),
            },
            J::String(s) => Value::Str(s.clone()),
            J::Object(_) => Value::Str(v.to_string()),
            J::Array(_) => {
                let mut flat = Vec::new();
                flatten(v, &mut flat);
                let numeric = |x: &&J| x.is_number() || x.is_boolean();
                if flat.is_empty() || !flat.iter().all(|x| numeric(&x)) {
                    Value::StrArr(flat.iter().map(|x| Value::from_json(x).to_text()).collect())
                } else if flat.iter().all(|x| x.is_i64() || x.is_u64() || x.is_boolean()) {
                    Value::IntArr(flat.iter().map(|x| Value::from_json(x).to_int()).collect())
                } else {
                    Value::FloatArr(flat.iter().map(|x| Value::from_json(x).to_float()).collect())
                }
            }
        }
    }
}

/// serde_json has no NaN or infinity: those come back as null.
fn float_json(f: f64) -> J {
    serde_json::Number::from_f64(f).map(J::Number).unwrap_or(J::Null)
}

fn flatten<'a>(v: &'a J, out: &mut Vec<&'a J>) {
    match v {
        J::Array(a) => a.iter().for_each(|x| flatten(x, out)),
        x => out.push(x),
    }
}
