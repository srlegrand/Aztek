//! Number parsing and formatting and colour conversion the mesh file readers and writers share.

/// A float type the writers format the way C++ `std::to_chars` does.
pub trait IoFloat: Copy + std::fmt::LowerExp {
    fn to_f64(self) -> f64;
    fn is_finite_(self) -> bool;
}
impl IoFloat for f64 {
    fn to_f64(self) -> f64 {
        self
    }
    fn is_finite_(self) -> bool {
        self.is_finite()
    }
}
impl IoFloat for f32 {
    fn to_f64(self) -> f64 {
        self as f64
    }
    fn is_finite_(self) -> bool {
        self.is_finite()
    }
}

/// Splits `{:e}` output ("-1.2345e-7") into sign, digit string without the point, and decimal exponent.
fn sci_parts(s: &str) -> (bool, String, i32) {
    let (neg, s) = match s.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, s),
    };
    let (mant, exp) = s.split_once('e').unwrap();
    let digits: String = mant.chars().filter(|c| *c != '.').collect();
    (neg, digits, exp.parse().unwrap())
}

fn non_finite(v: f64) -> &'static str {
    if v.is_nan() {
        if v.is_sign_negative() {
            "-nan"
        } else {
            "nan"
        }
    } else if v < 0.0 {
        "-inf"
    } else {
        "inf"
    }
}

/// Fixed notation from digits d1 d2 ... dn and the exponent of d1.
fn fixed(neg: bool, digits: &str, exp: i32) -> String {
    let mut s = String::new();
    if neg {
        s.push('-');
    }
    let n = digits.len() as i32;
    if exp < 0 {
        s.push_str("0.");
        for _ in 0..(-exp - 1) {
            s.push('0');
        }
        s.push_str(digits);
    } else if exp + 1 >= n {
        s.push_str(digits);
        for _ in 0..(exp + 1 - n) {
            s.push('0');
        }
    } else {
        s.push_str(&digits[..(exp + 1) as usize]);
        s.push('.');
        s.push_str(&digits[(exp + 1) as usize..]);
    }
    s
}

/// C style scientific notation: one digit, optional fraction, exponent of at least two digits.
fn scientific(neg: bool, digits: &str, exp: i32) -> String {
    let mut s = String::new();
    if neg {
        s.push('-');
    }
    s.push_str(&digits[..1]);
    if digits.len() > 1 {
        s.push('.');
        s.push_str(&digits[1..]);
    }
    s.push('e');
    s.push(if exp < 0 { '-' } else { '+' });
    s.push_str(&format!("{:02}", exp.abs()));
    s
}

/// Shortest text that reads back exactly, as `std::to_chars(v)`: fixed or scientific, whichever is shorter (fixed on
/// a tie).
pub fn shortest<T: IoFloat>(v: T) -> String {
    if !v.is_finite_() {
        return non_finite(v.to_f64()).to_string();
    }
    if v.to_f64() == 0.0 {
        return if v.to_f64().is_sign_negative() { "-0".into() } else { "0".into() };
    }
    let (neg, digits, exp) = sci_parts(&format!("{v:e}"));
    let f = fixed(neg, &digits, exp);
    let e = scientific(neg, &digits, exp);
    if e.len() < f.len() {
        e
    } else {
        f
    }
}

/// printf "%.{precision}g".
pub fn general<T: IoFloat>(v: T, precision: i32) -> String {
    let x = v.to_f64();
    if !x.is_finite() {
        return non_finite(x).to_string();
    }
    let p = precision.max(1);
    let (neg, mut digits, mut exp) = sci_parts(&format!("{:.*e}", (p - 1) as usize, x));
    if x == 0.0 {
        exp = 0;
    }
    let sci = !(exp >= -4 && exp < p);
    // Without '#', trailing zeros go.
    while digits.len() > 1 && digits.ends_with('0') {
        digits.pop();
    }
    if x == 0.0 {
        digits = "0".into();
    }
    if sci {
        scientific(neg, &digits, exp)
    } else {
        fixed(neg, &digits, exp)
    }
}

/// Appends v in the shortest form that reads back exactly, or with precision significant digits when above 0.
pub fn append_number<T: IoFloat>(s: &mut String, v: T, precision: i32) {
    if precision > 0 {
        s.push_str(&general(v, precision));
    } else {
        s.push_str(&shortest(v));
    }
}

/// A double the way `std::from_chars` reads it: the longest prefix in decimal floating syntax (optional minus,
/// digits with an optional point, optional exponent), or inf/infinity/nan. Returns the value and the bytes used.
pub fn parse_double(s: &[u8]) -> Option<(f64, usize)> {
    let mut i = 0;
    if i < s.len() && s[i] == b'-' {
        i += 1;
    }
    let rest = &s[i..];
    let lower = |n: usize| rest.len() >= n && rest[..n].eq_ignore_ascii_case(&b"infinity"[..n]);
    if lower(8) {
        let v = f64::INFINITY;
        return Some((if i == 1 { -v } else { v }, i + 8));
    }
    if lower(3) {
        let v = f64::INFINITY;
        return Some((if i == 1 { -v } else { v }, i + 3));
    }
    if rest.len() >= 3 && rest[..3].eq_ignore_ascii_case(b"nan") {
        let mut j = i + 3;
        if j < s.len() && s[j] == b'(' {
            let mut k = j + 1;
            while k < s.len() && (s[k].is_ascii_alphanumeric() || s[k] == b'_') {
                k += 1;
            }
            if k < s.len() && s[k] == b')' {
                j = k + 1;
            }
        }
        let v = f64::NAN;
        return Some((if i == 1 { -v } else { v }, j));
    }
    let start = i;
    while i < s.len() && s[i].is_ascii_digit() {
        i += 1;
    }
    let whole = i > start;
    let mut frac = false;
    if i < s.len() && s[i] == b'.' {
        let mut j = i + 1;
        while j < s.len() && s[j].is_ascii_digit() {
            j += 1;
        }
        frac = j > i + 1;
        if whole || frac {
            i = j;
        }
    }
    if !whole && !frac {
        return None;
    }
    if i < s.len() && (s[i] == b'e' || s[i] == b'E') {
        let mut j = i + 1;
        if j < s.len() && (s[j] == b'+' || s[j] == b'-') {
            j += 1;
        }
        let ds = j;
        while j < s.len() && s[j].is_ascii_digit() {
            j += 1;
        }
        if j > ds {
            i = j;
        }
    }
    // The slice is ASCII by construction.
    let text = std::str::from_utf8(&s[..i]).ok()?;
    text.parse::<f64>().ok().map(|v| (v, i))
}

/// An integer the way `std::from_chars` reads it: optional minus, then digits.
pub fn parse_i64(s: &[u8]) -> Option<(i64, usize)> {
    let mut i = 0;
    let neg = !s.is_empty() && s[0] == b'-';
    if neg {
        i = 1;
    }
    let start = i;
    let mut v: i64 = 0;
    while i < s.len() && s[i].is_ascii_digit() {
        v = v.checked_mul(10)?.checked_add((s[i] - b'0') as i64)?;
        i += 1;
    }
    if i == start {
        return None;
    }
    Some((if neg { -v } else { v }, i))
}

pub fn srgb_to_linear(c: f32) -> f32 {
    let c = c.max(0.0);
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

pub fn linear_to_srgb(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// Formats [0, n) with f into one string, in parallel blocks.
pub fn format_blocks(n: usize, f: impl Fn(&mut String, usize) + Sync) -> String {
    use rayon::prelude::*;
    const BLOCK: usize = 16384;
    let blocks = n.div_ceil(BLOCK);
    let parts: Vec<String> = (0..blocks)
        .into_par_iter()
        .map(|b| {
            let mut s = String::new();
            for i in b * BLOCK..n.min((b + 1) * BLOCK) {
                f(&mut s, i);
            }
            s
        })
        .collect();
    parts.concat()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_chars_forms() {
        assert_eq!(shortest(1.0f64), "1");
        assert_eq!(shortest(0.5f64), "0.5");
        assert_eq!(shortest(100.0f64), "100");
        assert_eq!(shortest(1e20f64), "1e+20");
        assert_eq!(shortest(1e-5f64), "1e-05");
        assert_eq!(shortest(0.001f64), "0.001");
        assert_eq!(shortest(123456.0f64), "123456");
        assert_eq!(shortest(-0.0f64), "-0");
        assert_eq!(shortest(0.1f32), "0.1");
        assert_eq!(general(0.1f64, 3), "0.1");
        assert_eq!(general(1234567.0f64, 3), "1.23e+06");
        assert_eq!(general(0.0001234f64, 2), "0.00012");
        assert_eq!(general(100.0f64, 6), "100");
        assert_eq!(general(0.0f64, 6), "0");
    }

    #[test]
    fn from_chars_forms() {
        assert_eq!(parse_double(b"1."), Some((1.0, 2)));
        assert_eq!(parse_double(b".5x"), Some((0.5, 2)));
        assert_eq!(parse_double(b"1e"), Some((1.0, 1)));
        assert_eq!(parse_double(b"1e+"), Some((1.0, 1)));
        assert_eq!(parse_double(b"-.25"), Some((-0.25, 4)));
        assert_eq!(parse_double(b"."), None);
        assert_eq!(parse_double(b"+1"), None);
        assert_eq!(parse_double(b"inf"), Some((f64::INFINITY, 3)));
        assert_eq!(parse_i64(b"-12/"), Some((-12, 3)));
    }
}
