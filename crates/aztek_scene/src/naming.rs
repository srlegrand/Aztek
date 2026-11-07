
use serde_json::{json, Value};

use crate::node::NodeType;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaseStyle {
    Any,
    /// MyCube
    Pascal,
    /// myCube
    Camel,
    /// my_cube
    Snake,
    /// MY_CUBE
    UpperSnake,
}

fn is_digit(c: char) -> bool {
    c.is_ascii_digit()
}
fn is_upper(c: char) -> bool {
    c.is_ascii_uppercase()
}
fn is_lower(c: char) -> bool {
    c.is_ascii_lowercase()
}

/// Sides and positions: left, right, centre, middle, front, back, top, and the corners.
const DEFAULT_SUFFIXES: &str = "L, R, C, M, F, B, T, LF, RF, LB, RB, FL, FR, BL, BR";

/// "L, R" or ["L", "R"].
fn parse_suffixes(j: &Value) -> Vec<String> {
    let mut out = Vec::new();
    let mut add = |s: &str| {
        let s = s.trim_matches(|c| c == ' ' || c == '_');
        if !s.is_empty() {
            out.push(s.to_string());
        }
    };
    if let Some(a) = j.as_array() {
        for s in a.iter().filter_map(Value::as_str) {
            add(s);
        }
    }
    if let Some(s) = j.as_str() {
        for part in s.split([',', ' ']) {
            add(part);
        }
    }
    out
}

/// Splits any of myCube, MyCube, my_cube, my-cube, "my cube", HTTPServer2 into words.
pub fn split_words(name: &str) -> Vec<String> {
    let chars: Vec<char> = name.chars().collect();
    let mut words = Vec::new();
    let mut cur = String::new();
    for (i, &c) in chars.iter().enumerate() {
        if matches!(c, '_' | '-' | ' ' | '.') {
            if !cur.is_empty() {
                words.push(std::mem::take(&mut cur));
            }
            continue;
        }
        if let Some(prev) = cur.chars().last() {
            let next_lower = i + 1 < chars.len() && is_lower(chars[i + 1]);
            // myCube -> my|Cube, HTTPServer -> HTTP|Server, digits stay with the word before them.
            if is_upper(c) && (is_lower(prev) || is_digit(prev) || (is_upper(prev) && next_lower)) {
                words.push(std::mem::take(&mut cur));
            }
        }
        cur.push(c);
    }
    if !cur.is_empty() {
        words.push(cur);
    }
    words
}

/// Which underscores PascalCase and camelCase keep.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Underscores {
    pub allow: bool,
    /// Only the ones before trailing suffixes (or numbers): Arm_L and Wheel_LF stay, Dog_Leg_B becomes DogLeg_B.
    pub suffixes_only: bool,
    /// With suffixes_only, listed suffixes may also lead the name: L_Arm.
    pub prefixes: bool,
    /// Matched ignoring case and written as listed.
    pub suffixes: Vec<String>,
}

impl Default for Underscores {
    fn default() -> Self {
        Self {
            allow: false,
            suffixes_only: true,
            prefixes: false,
            suffixes: Vec::new(),
        }
    }
}

fn upper_first(w: &mut String) {
    if let Some(f) = w.chars().next() {
        let up = f.to_ascii_uppercase();
        w.replace_range(0..f.len_utf8(), &up.to_string());
    }
}

/// With underscores allowed, PascalCase and camelCase case each kept underscore-separated part on its own.
pub fn apply_case(name: &str, style: CaseStyle, underscores: &Underscores) -> String {
    if style == CaseStyle::Any {
        return name.to_string();
    }
    if underscores.allow
        && matches!(style, CaseStyle::Pascal | CaseStyle::Camel)
        && name.contains('_')
    {
        let parts: Vec<&str> = name.split('_').collect();
        let listed = |i: usize| -> Option<String> {
            underscores
                .suffixes
                .iter()
                .find(|s| s.eq_ignore_ascii_case(parts[i]))
                .cloned()
        };
        // The part as a listed suffix, or nothing. Numbers count, and so does the empty part before uniqueness digits.
        let suffix = |i: usize| -> Option<String> {
            let p = parts[i];
            let numeric = if p.is_empty() {
                i + 1 == parts.len()
            } else {
                p.chars().all(is_digit)
            };
            if numeric {
                return Some(p.to_string());
            }
            listed(i)
        };
        let mut first = 0;
        let mut body = 1;
        if underscores.suffixes_only {
            body = parts.len();
            while body > 1 && suffix(body - 1).is_some() {
                body -= 1;
            }
            if underscores.prefixes {
                while first + 1 < body && listed(first).is_some() {
                    first += 1;
                }
            }
        }
        let mut out = String::new();
        for i in 0..first {
            out += &listed(i).unwrap();
            out.push('_');
        }
        let mut head = String::new();
        for p in &parts[first..body] {
            head += p;
            head.push(' ');
        }
        out += &apply_case(
            &head,
            if first > 0 { CaseStyle::Pascal } else { style },
            &Underscores::default(),
        );
        for i in body..parts.len() {
            let s = suffix(i);
            let mut part = match &s {
                Some(s) => s.clone(),
                None => apply_case(parts[i], CaseStyle::Pascal, &Underscores::default()),
            };
            // A part that starts with a digit keeps it in front: arm_2b is Arm_2b, not Arm__2b.
            if s.is_none() && part.starts_with('_') && parts[i].chars().next().is_some_and(is_digit)
            {
                part.remove(0);
            }
            out.push('_');
            out += &part;
        }
        return out;
    }
    let words = split_words(name);
    let mut out = String::new();
    for (i, w) in words.iter().enumerate() {
        let mut w: String = w
            .chars()
            .map(|c| {
                if style == CaseStyle::UpperSnake {
                    c.to_ascii_uppercase()
                } else {
                    c.to_ascii_lowercase()
                }
            })
            .collect();
        if style == CaseStyle::Pascal || (style == CaseStyle::Camel && i > 0) {
            upper_first(&mut w);
        }
        if matches!(style, CaseStyle::Snake | CaseStyle::UpperSnake)
            && i > 0
            && !w.chars().next().is_some_and(is_digit)
        {
            out.push('_');
        }
        out += &w;
    }
    // A name can't start with a digit.
    if out.chars().next().is_some_and(is_digit) {
        out.insert(0, '_');
    }
    out
}

pub fn parse_case_style(s: &str) -> CaseStyle {
    match s {
        "PascalCase" | "pascal" => CaseStyle::Pascal,
        "camelCase" | "camel" => CaseStyle::Camel,
        "snake_case" | "snake" => CaseStyle::Snake,
        "UPPER_SNAKE" | "upper_snake" => CaseStyle::UpperSnake,
        _ => CaseStyle::Any,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NamingMode {
    /// Rewrite to match.
    Fix,
    Warn,
    Block,
}

/// Per-workspace naming rules (prefs file "naming", usually in .aztek/naming.json).
/// Namespaces and trailing digits are left alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamingPolicy {
    pub enabled: bool,
    pub mode: NamingMode,
    /// "*" last.
    pub rules: Vec<(String, CaseStyle)>,
    pub keep_prefixes: Vec<String>,
    /// Names unique across the whole scene; off, only among siblings (paths tell them apart).
    pub unique_names: bool,
    /// "*" last.
    pub allow_underscores: Vec<(String, bool)>,
    pub underscore_suffixes_only: bool,
    pub underscore_prefixes: bool,
    pub underscore_suffixes: Vec<String>,
}

impl Default for NamingPolicy {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: NamingMode::Fix,
            rules: vec![
                ("joint".into(), CaseStyle::Snake),
                ("*".into(), CaseStyle::Pascal),
            ],
            keep_prefixes: Vec::new(),
            unique_names: false,
            allow_underscores: vec![("joint".into(), true), ("*".into(), true)],
            underscore_suffixes_only: true,
            underscore_prefixes: false,
            underscore_suffixes: Vec::new(),
        }
    }
}

/// RFC 7386 merge patch, as nlohmann's `merge_patch`.
fn merge_patch(target: &mut Value, patch: &Value) {
    if let Some(p) = patch.as_object() {
        if !target.is_object() {
            *target = json!({});
        }
        let t = target.as_object_mut().unwrap();
        for (k, v) in p {
            if v.is_null() {
                t.remove(k);
            } else {
                merge_patch(t.entry(k.clone()).or_insert(Value::Null), v);
            }
        }
    } else {
        *target = patch.clone();
    }
}

impl NamingPolicy {
    pub fn defaults() -> Value {
        json!({
            "enabled": false,
            "mode": "fix",
            "uniqueNames": false,
            "allowUnderscores": {"joint": true, "*": true},
            "underscoreSuffixesOnly": true,
            "underscorePrefixes": false,
            "underscoreSuffixes": DEFAULT_SUFFIXES,
            "rules": {"joint": "snake_case", "*": "PascalCase"},
            "keepPrefixes": ["UCX_", "UBX_", "USP_", "UCP_", "SOCKET_", "SM_", "SK_", "MI_", "M_", "T_"],
        })
    }

    pub fn from_json(input: &Value) -> Self {
        let mut j = Self::defaults();
        if input.is_object() {
            merge_patch(&mut j, input);
        }
        let b = |k: &str, d: bool| j.get(k).and_then(Value::as_bool).unwrap_or(d);
        let mut p = NamingPolicy {
            enabled: b("enabled", false),
            unique_names: b("uniqueNames", false),
            underscore_suffixes_only: b("underscoreSuffixesOnly", true),
            underscore_prefixes: b("underscorePrefixes", false),
            underscore_suffixes: parse_suffixes(
                j.get("underscoreSuffixes").unwrap_or(&Value::Null),
            ),
            allow_underscores: Vec::new(),
            ..Default::default()
        };
        let mut fallback_underscores = true;
        if let Some(o) = j.get("allowUnderscores").and_then(Value::as_object) {
            for (ty, allow) in o {
                let Some(allow) = allow.as_bool() else {
                    continue;
                };
                if ty == "*" {
                    fallback_underscores = allow;
                } else {
                    p.allow_underscores.push((ty.clone(), allow));
                }
            }
        }
        p.allow_underscores.push(("*".into(), fallback_underscores));
        p.mode = match j.get("mode").and_then(Value::as_str).unwrap_or("fix") {
            "warn" => NamingMode::Warn,
            "block" => NamingMode::Block,
            _ => NamingMode::Fix,
        };
        // The default rules are kept in front; the file's are appended.
        let mut fallback = ("*".to_string(), CaseStyle::Any);
        if let Some(o) = j.get("rules").and_then(Value::as_object) {
            for (ty, style) in o {
                let Some(style) = style.as_str() else {
                    continue;
                };
                if ty == "*" {
                    fallback = (ty.clone(), parse_case_style(style));
                } else {
                    p.rules.push((ty.clone(), parse_case_style(style)));
                }
            }
        }
        p.rules.push(fallback);
        if let Some(a) = j.get("keepPrefixes").and_then(Value::as_array) {
            p.keep_prefixes
                .extend(a.iter().filter_map(|v| v.as_str().map(str::to_string)));
        }
        p
    }

    pub fn underscores_for(&self, ty: &NodeType) -> Underscores {
        let mut u = Underscores {
            allow: true,
            suffixes_only: self.underscore_suffixes_only,
            prefixes: self.underscore_prefixes,
            suffixes: self.underscore_suffixes.clone(),
        };
        if let Some((_, allow)) = self
            .allow_underscores
            .iter()
            .find(|(t, _)| t == "*" || ty.is_a(t))
        {
            u.allow = *allow;
        }
        u
    }

    pub fn style_for(&self, ty: &NodeType) -> CaseStyle {
        self.rules
            .iter()
            .find(|(t, _)| t == "*" || ty.is_a(t))
            .map(|(_, s)| *s)
            .unwrap_or(CaseStyle::Any)
    }

    /// What the name should be under this policy (may equal name).
    pub fn conform(&self, ty: &NodeType, name: &str) -> String {
        if !self.enabled {
            return name.to_string();
        }
        let (ns, short) = match name.rfind(':') {
            Some(colon) => (&name[..=colon], &name[colon + 1..]),
            None => ("", name),
        };
        let prefix = self
            .keep_prefixes
            .iter()
            .filter(|p| short.starts_with(p.as_str()))
            .fold("", |best, p| {
                if p.len() > best.len() {
                    p.as_str()
                } else {
                    best
                }
            });
        let short = &short[prefix.len()..];
        // Uniqueness digits belong to the name as typed, not to a word.
        let digits = short.len()
            - short
                .chars()
                .rev()
                .take_while(|c| c.is_ascii_digit())
                .count();
        let body = apply_case(
            &short[..digits],
            self.style_for(ty),
            &self.underscores_for(ty),
        );
        format!("{ns}{prefix}{body}{}", &short[digits..])
    }
}
