//! The `expression` node and command: the MEL expression subset with direct attribute access.
//!
//! An expression's attributes reach it as real connections: read ones arrive on `input[k]`, assigned ones leave from
//! `output[k]`, k counting first appearances in the source, so the same text always binds the same way and scene files
//! need only save the text and the connections. The parser and evaluator are private to this module.
//!
//! All numbers are doubles: unlike MEL, `1/2` is 0.5 unless an `int` variable holds it.

use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;

use aztek_ops::{fail, ArgType, Args, CmdResult, CommandDef, Commands, FlagDef};
use glam::DVec3;
use serde_json::{json, Value};

use crate::graph::Graph;
use crate::node::{types, EvalContext, NodeBehavior, NodeData, NodeType};
use crate::plug::{flags, AttrDef, NodeId, PlugRef, PlugType, PlugValue, NO_NODE};
use crate::session::Session;

// ---------------------------------------------------------------------------------------------------------------------
// Tokens

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Str(String),
    Word(String),
    Var(String),
    Op(&'static str),
    End,
}

const OPS: &[&str] = &[
    "<<", ">>", "==", "!=", "<=", ">=", "&&", "||", "+=", "-=", "*=", "/=", "++", "--", "+", "-", "*", "/", "%", "^", "<", ">", "=", "!", "?", ":", ";", ",", "(", ")", "{", "}",
];

/// Tokens with the byte span each came from.
fn lex(src: &str) -> Result<Vec<(Tok, usize, usize)>, String> {
    let b = src.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    let word_start = |c: u8| c.is_ascii_alphabetic() || c == b'_' || c == b'|';
    let word_char = |c: u8| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'|' | b'.' | b'[' | b']');
    while i < b.len() {
        let c = b[i];
        if c.is_ascii_whitespace() {
            i += 1;
        } else if src[i..].starts_with("//") {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
        } else if src[i..].starts_with("/*") {
            match src[i + 2..].find("*/") {
                Some(e) => i += e + 4,
                None => return Err("unterminated comment".into()),
            }
        } else if c.is_ascii_digit() || (c == b'.' && b.get(i + 1).is_some_and(u8::is_ascii_digit)) {
            let start = i;
            while i < b.len() && (b[i].is_ascii_digit() || b[i] == b'.') {
                i += 1;
            }
            if i < b.len() && matches!(b[i], b'e' | b'E') {
                let mut k = i + 1;
                if k < b.len() && matches!(b[k], b'+' | b'-') {
                    k += 1;
                }
                if k < b.len() && b[k].is_ascii_digit() {
                    i = k;
                    while i < b.len() && b[i].is_ascii_digit() {
                        i += 1;
                    }
                }
            }
            let v: f64 = src[start..i].parse().map_err(|_| format!("bad number '{}'", &src[start..i]))?;
            out.push((Tok::Num(v), start, i));
        } else if c == b'"' {
            let start = i;
            i += 1;
            let mut s = String::new();
            loop {
                match b.get(i) {
                    None => return Err("unterminated string".into()),
                    Some(b'"') => break,
                    Some(b'\\') => {
                        i += 1;
                        s.push(match b.get(i) {
                            Some(b'n') => '\n',
                            Some(b't') => '\t',
                            Some(&o) => o as char,
                            None => return Err("unterminated string".into()),
                        });
                        i += 1;
                    }
                    Some(&o) => {
                        s.push(o as char);
                        i += 1;
                    }
                }
            }
            i += 1;
            out.push((Tok::Str(s), start, i));
        } else if c == b'$' {
            let start = i;
            i += 1;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            if i == start + 1 {
                return Err("'$' with no variable name".into());
            }
            out.push((Tok::Var(src[start..i].to_string()), start, i));
            // $v.x: the component is an operator and a word, not an attribute token.
            if b.get(i) == Some(&b'.') {
                out.push((Tok::Op("."), i, i + 1));
                i += 1;
            }
        } else if (word_start(c) && !src[i..].starts_with("||")) || (c == b'.' && b.get(i + 1).is_some_and(|x| x.is_ascii_alphabetic())) {
            let start = i;
            i += 1;
            while i < b.len() && !src[i..].starts_with("||") && (word_char(b[i]) || (b[i] == b':' && b.get(i + 1).is_some_and(|x| x.is_ascii_alphabetic() || *x == b'_') && b.get(i - 1).is_some_and(|x| x.is_ascii_alphanumeric()) && !space_before_colon(src, i))) {
                i += 1;
            }
            out.push((Tok::Word(src[start..i].to_string()), start, i));
        } else if c == b'.' {
            // A variable's component: $v.x. Kept as an operator followed by a word.
            let start = i;
            i += 1;
            out.push((Tok::Op("."), start, i));
        } else {
            let Some(op) = OPS.iter().find(|o| src[i..].starts_with(**o)) else {
                return Err(format!("unexpected '{}'", &src[i..].chars().next().unwrap()));
            };
            out.push((Tok::Op(op), i, i + op.len()));
            i += op.len();
        }
    }
    out.push((Tok::End, src.len(), src.len()));
    Ok(out)
}

/// `a ? b:c` uses the colon as an operator; a namespace colon joins a word only when an attribute follows, ns:obj.tx.
fn space_before_colon(src: &str, colon: usize) -> bool {
    let rest = &src[colon + 1..];
    let end = rest.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == ':' || c == '|')).unwrap_or(rest.len());
    !rest[end..].starts_with('.')
}

// ---------------------------------------------------------------------------------------------------------------------
// Syntax

#[derive(Debug, Clone)]
enum Expr {
    Num(f64),
    Str(String),
    Vector(Box<[Expr; 3]>),
    Var(String),
    Attr(usize),
    Time,
    Frame,
    Neg(Box<Expr>),
    Not(Box<Expr>),
    Bin(&'static str, Box<Expr>, Box<Expr>),
    Ternary(Box<Expr>, Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>),
    Member(Box<Expr>, usize),
}

#[derive(Debug, Clone)]
enum Target {
    Var(String),
    Member(String, usize),
    Attr(usize),
}

#[derive(Debug, Clone)]
enum Stmt {
    Decl { kind: Kind, name: String, init: Option<Expr>, global: bool },
    Assign(Target, Option<&'static str>, Expr),
    Expr(Expr),
    If(Expr, Vec<Stmt>, Vec<Stmt>),
    While(Expr, Vec<Stmt>),
    DoWhile(Vec<Stmt>, Expr),
    For(Vec<Stmt>, Option<Expr>, Vec<Stmt>, Vec<Stmt>),
    Break,
    Continue,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Kind {
    Int,
    Float,
    Vector,
    Str,
}

/// An attribute the source names: as typed, where, and how it is used.
#[derive(Debug, Clone, Default)]
pub struct AttrRef {
    pub token: String,
    pub spans: Vec<(usize, usize)>,
    pub read: bool,
    pub written: bool,
}

#[derive(Debug, Default)]
pub struct Program {
    stmts: Vec<Stmt>,
    pub refs: Vec<AttrRef>,
}

impl Program {
    /// Slot of each ref: its input element when only read, its output element when assigned.
    fn slots(&self) -> Vec<(bool, i32)> {
        let (mut ins, mut outs) = (0, 0);
        self.refs
            .iter()
            .map(|r| {
                // The reference's files spell an expression's plugs by slot: .O[2] = .I[0] * 2;
                let slot = |prefix: &str| r.token.strip_prefix(prefix).and_then(|x| x.strip_suffix(']')).and_then(|x| x.parse::<i32>().ok());
                if let Some(k) = slot(".I[") {
                    return (false, k);
                }
                if let Some(k) = slot(".O[") {
                    return (true, k);
                }
                if r.written {
                    outs += 1;
                    (true, outs - 1)
                } else {
                    ins += 1;
                    (false, ins - 1)
                }
            })
            .collect()
    }
}

/// name, fewest and most arguments.
const FUNCTIONS: &[(&str, usize, usize)] = &[
    ("abs", 1, 1), ("sign", 1, 1), ("floor", 1, 1), ("ceil", 1, 1), ("trunc", 1, 1), ("round", 1, 1), ("frac", 1, 1),
    ("sqrt", 1, 1), ("exp", 1, 1), ("log", 1, 1), ("log10", 1, 1), ("pow", 2, 2), ("fmod", 2, 2), ("hypot", 2, 2),
    ("sin", 1, 1), ("cos", 1, 1), ("tan", 1, 1), ("asin", 1, 1), ("acos", 1, 1), ("atan", 1, 1), ("atan2", 2, 2),
    ("sinh", 1, 1), ("cosh", 1, 1), ("tanh", 1, 1), ("sind", 1, 1), ("cosd", 1, 1), ("tand", 1, 1), ("asind", 1, 1),
    ("acosd", 1, 1), ("atand", 1, 1), ("atan2d", 2, 2), ("deg_to_rad", 1, 1), ("rad_to_deg", 1, 1),
    ("min", 2, 2), ("max", 2, 2), ("clamp", 3, 3), ("linstep", 3, 3), ("smoothstep", 3, 3), ("step", 2, 2),
    ("mag", 1, 1), ("unit", 1, 1), ("cross", 2, 2), ("dot", 2, 2), ("angle", 2, 2),
    ("noise", 1, 3), ("dnoise", 1, 1), ("rand", 1, 2), ("sphrand", 1, 1), ("gauss", 1, 2), ("seed", 1, 1),
    ("print", 1, 1),
];

struct Parser<'a> {
    toks: Vec<(Tok, usize, usize)>,
    at: usize,
    refs: Vec<AttrRef>,
    declared: std::collections::HashSet<String>,
    #[allow(dead_code)]
    src: &'a str,
}

impl Parser<'_> {
    fn peek(&self) -> &Tok {
        &self.toks[self.at].0
    }
    fn next(&mut self) -> Tok {
        let t = self.toks[self.at].0.clone();
        if self.at + 1 < self.toks.len() {
            self.at += 1;
        }
        t
    }
    fn is_op(&self, op: &str) -> bool {
        matches!(self.peek(), Tok::Op(o) if *o == op)
    }
    fn eat(&mut self, op: &str) -> bool {
        if self.is_op(op) {
            self.next();
            true
        } else {
            false
        }
    }
    fn expect(&mut self, op: &str) -> Result<(), String> {
        if self.eat(op) {
            Ok(())
        } else {
            Err(format!("syntax error: expected '{op}', found {}", self.describe()))
        }
    }
    fn describe(&self) -> String {
        match self.peek() {
            Tok::Num(n) => format!("'{n}'"),
            Tok::Str(s) => format!("\"{s}\""),
            Tok::Word(w) => format!("'{w}'"),
            Tok::Var(w) => format!("'{w}'"),
            Tok::Op(o) => format!("'{o}'"),
            Tok::End => "the end of the expression".into(),
        }
    }
    fn is_word(&self, w: &str) -> bool {
        matches!(self.peek(), Tok::Word(x) if x == w)
    }

    fn attr_ref(&mut self, token: &str, span: (usize, usize), write: bool) -> usize {
        let k = match self.refs.iter().position(|r| r.token == token) {
            Some(k) => k,
            None => {
                self.refs.push(AttrRef { token: token.to_string(), ..Default::default() });
                self.refs.len() - 1
            }
        };
        let r = &mut self.refs[k];
        r.spans.push(span);
        if write {
            r.written = true;
        } else {
            r.read = true;
        }
        k
    }

    fn block(&mut self) -> Result<Vec<Stmt>, String> {
        if self.eat("{") {
            let mut out = Vec::new();
            while !self.is_op("}") {
                if *self.peek() == Tok::End {
                    return Err("syntax error: missing '}'".into());
                }
                self.statement(&mut out)?;
            }
            self.next();
            Ok(out)
        } else {
            let mut out = Vec::new();
            self.statement(&mut out)?;
            Ok(out)
        }
    }

    fn kind_word(&self) -> Option<Kind> {
        match self.peek() {
            Tok::Word(w) => match w.as_str() {
                "int" => Some(Kind::Int),
                "float" => Some(Kind::Float),
                "vector" => Some(Kind::Vector),
                "string" => Some(Kind::Str),
                _ => None,
            },
            _ => None,
        }
    }

    fn statement(&mut self, out: &mut Vec<Stmt>) -> Result<(), String> {
        if self.eat(";") {
            return Ok(());
        }
        if self.is_word("if") {
            self.next();
            self.expect("(")?;
            let c = self.expression()?;
            self.expect(")")?;
            let yes = self.block()?;
            let no = if self.is_word("else") {
                self.next();
                self.block()?
            } else {
                Vec::new()
            };
            out.push(Stmt::If(c, yes, no));
            return Ok(());
        }
        if self.is_word("while") {
            self.next();
            self.expect("(")?;
            let c = self.expression()?;
            self.expect(")")?;
            out.push(Stmt::While(c, self.block()?));
            return Ok(());
        }
        if self.is_word("do") {
            self.next();
            let body = self.block()?;
            if !self.is_word("while") {
                return Err("syntax error: do needs while".into());
            }
            self.next();
            self.expect("(")?;
            let c = self.expression()?;
            self.expect(")")?;
            self.eat(";");
            out.push(Stmt::DoWhile(body, c));
            return Ok(());
        }
        if self.is_word("for") {
            self.next();
            self.expect("(")?;
            let mut init = Vec::new();
            if !self.is_op(";") {
                self.simple(&mut init)?;
            }
            self.expect(";")?;
            let cond = if self.is_op(";") { None } else { Some(self.expression()?) };
            self.expect(";")?;
            let mut step = Vec::new();
            if !self.is_op(")") {
                self.simple(&mut step)?;
            }
            self.expect(")")?;
            out.push(Stmt::For(init, cond, step, self.block()?));
            return Ok(());
        }
        if self.is_op("{") {
            out.extend(self.block()?);
            return Ok(());
        }
        // A MEL command among the statements, which the reference runs as it evaluates. A compute can't set attributes: the value is worked out and dropped.
        if self.is_word("setAttr") {
            self.next();
            self.next();
            self.expression()?;
            self.eat(";");
            return Ok(());
        }
        if self.is_word("break") || self.is_word("continue") {
            out.push(if self.is_word("break") { Stmt::Break } else { Stmt::Continue });
            self.next();
            self.eat(";");
            return Ok(());
        }
        self.simple(out)?;
        if *self.peek() != Tok::End && !self.is_op("}") {
            self.expect(";")?;
        }
        Ok(())
    }

    /// A declaration, assignment or bare expression, with no terminator.
    fn simple(&mut self, out: &mut Vec<Stmt>) -> Result<(), String> {
        let global = self.is_word("global");
        if global {
            self.next();
        }
        if let Some(kind) = self.kind_word() {
            self.next();
            loop {
                let Tok::Var(name) = self.next() else { return Err("syntax error: expected a $variable".into()) };
                let init = if self.eat("=") { Some(self.expression()?) } else { None };
                self.declared.insert(name.clone());
                out.push(Stmt::Decl { kind, name, init, global });
                if !self.eat(",") {
                    return Ok(());
                }
            }
        }
        if global {
            return Err("syntax error: global needs a type".into());
        }
        // Assignment: look ahead past the target.
        let start = self.at;
        if let Some(target) = self.target()? {
            for (op, base) in [("=", None), ("+=", Some("+")), ("-=", Some("-")), ("*=", Some("*")), ("/=", Some("/"))] {
                if self.eat(op) {
                    let e = self.expression()?;
                    if base.is_some() {
                        if let Target::Attr(k) = target {
                            self.refs[k].read = true;
                        }
                    }
                    out.push(Stmt::Assign(self.assigned(target), base, e));
                    return Ok(());
                }
            }
            for (op, base) in [("++", "+"), ("--", "-")] {
                if self.eat(op) {
                    out.push(Stmt::Assign(self.assigned(target), Some(base), Expr::Num(1.0)));
                    return Ok(());
                }
            }
            self.at = start;
        }
        out.push(Stmt::Expr(self.expression()?));
        Ok(())
    }

    /// Marks the variable declared, or the attribute written, once an assignment is certain.
    fn assigned(&mut self, t: Target) -> Target {
        match &t {
            Target::Var(n) | Target::Member(n, _) => {
                self.declared.insert(n.clone());
            }
            Target::Attr(k) => self.refs[*k].written = true,
        }
        t
    }

    fn target(&mut self) -> Result<Option<Target>, String> {
        let (tok, s, e) = self.toks[self.at].clone();
        match tok {
            Tok::Var(name) => {
                self.next();
                if self.eat(".") {
                    let Tok::Word(w) = self.next() else { return Err("syntax error: expected x, y or z".into()) };
                    let Some(c) = component(&w) else { return Err(format!("syntax error: bad vector component '{w}'")) };
                    return Ok(Some(Target::Member(name, c)));
                }
                Ok(Some(Target::Var(name)))
            }
            Tok::Word(w) if !is_keyword(&w) && !matches!(self.toks[self.at + 1].0, Tok::Op("(")) => {
                let assign = matches!(self.toks[self.at + 1].0, Tok::Op("=" | "+=" | "-=" | "*=" | "/=" | "++" | "--"));
                if !assign {
                    return Ok(None);
                }
                self.next();
                if w == "time" || w == "frame" {
                    return Err(format!("'{w}' can't be assigned to"));
                }
                // Recorded as neither read nor written until the caller has seen the operator.
                let k = match self.refs.iter().position(|r| r.token == w) {
                    Some(k) => k,
                    None => {
                        self.refs.push(AttrRef { token: w.clone(), ..Default::default() });
                        self.refs.len() - 1
                    }
                };
                self.refs[k].spans.push((s, e));
                Ok(Some(Target::Attr(k)))
            }
            _ => Ok(None),
        }
    }

    fn expression(&mut self) -> Result<Expr, String> {
        let c = self.binary(0)?;
        if self.eat("?") {
            let y = self.expression()?;
            self.expect(":")?;
            let n = self.expression()?;
            return Ok(Expr::Ternary(Box::new(c), Box::new(y), Box::new(n)));
        }
        Ok(c)
    }

    fn binary(&mut self, level: usize) -> Result<Expr, String> {
        const LEVELS: &[&[&str]] = &[&["||"], &["&&"], &["==", "!="], &["<", "<=", ">", ">="], &["+", "-"], &["*", "/", "%", "^"]];
        if level == LEVELS.len() {
            return self.unary();
        }
        let mut left = self.binary(level + 1)?;
        loop {
            let Some(op) = LEVELS[level].iter().find(|o| self.is_op(o)).copied() else { return Ok(left) };
            self.next();
            let right = self.binary(level + 1)?;
            left = Expr::Bin(op, Box::new(left), Box::new(right));
        }
    }

    fn unary(&mut self) -> Result<Expr, String> {
        if self.eat("-") {
            return Ok(Expr::Neg(Box::new(self.unary()?)));
        }
        if self.eat("+") {
            return self.unary();
        }
        if self.eat("!") {
            return Ok(Expr::Not(Box::new(self.unary()?)));
        }
        let mut e = self.primary()?;
        while self.eat(".") {
            let Tok::Word(w) = self.next() else { return Err("syntax error: expected x, y or z".into()) };
            let Some(c) = component(&w) else { return Err(format!("syntax error: bad vector component '{w}'")) };
            e = Expr::Member(Box::new(e), c);
        }
        Ok(e)
    }

    fn primary(&mut self) -> Result<Expr, String> {
        let (tok, s, e) = self.toks[self.at].clone();
        match tok {
            Tok::Num(n) => {
                self.next();
                Ok(Expr::Num(n))
            }
            Tok::Str(t) => {
                self.next();
                Ok(Expr::Str(t))
            }
            Tok::Var(name) => {
                self.next();
                // $v.x lexes as one variable, a dot and a word, handled by unary.
                if !self.declared.contains(&name) {
                    return Err(format!("Attribute not found or variable missing '$': {name}"));
                }
                Ok(Expr::Var(name))
            }
            Tok::Op("(") => {
                self.next();
                let inner = self.expression()?;
                self.expect(")")?;
                Ok(inner)
            }
            Tok::Op("<<") => {
                self.next();
                let x = self.binary(4)?;
                self.expect(",")?;
                let y = self.binary(4)?;
                self.expect(",")?;
                let z = self.binary(4)?;
                self.expect(">>")?;
                Ok(Expr::Vector(Box::new([x, y, z])))
            }
            Tok::Word(w) => {
                self.next();
                if self.eat("(") {
                    let Some(&(_, lo, hi)) = FUNCTIONS.iter().find(|f| f.0 == w) else {
                        return Err(format!("Cannot find procedure \"{w}\"."));
                    };
                    let mut args = Vec::new();
                    if !self.is_op(")") {
                        loop {
                            args.push(self.expression()?);
                            if !self.eat(",") {
                                break;
                            }
                        }
                    }
                    self.expect(")")?;
                    if args.len() < lo || args.len() > hi {
                        return Err(format!("{w}: wrong number of arguments ({})", args.len()));
                    }
                    return Ok(Expr::Call(w, args));
                }
                match w.as_str() {
                    "time" => Ok(Expr::Time),
                    "frame" => Ok(Expr::Frame),
                    k if is_keyword(k) => Err(format!("syntax error near '{k}'")),
                    _ => Ok(Expr::Attr(self.attr_ref(&w, (s, e), false))),
                }
            }
            _ => Err(format!("syntax error: unexpected {}", self.describe())),
        }
    }
}

fn is_keyword(w: &str) -> bool {
    matches!(w, "if" | "else" | "while" | "do" | "for" | "break" | "continue" | "global" | "int" | "float" | "vector" | "string")
}

fn component(w: &str) -> Option<usize> {
    match w {
        "x" => Some(0),
        "y" => Some(1),
        "z" => Some(2),
        _ => None,
    }
}

/// The source as a program, or the error to report.
pub fn parse(src: &str) -> Result<Program, String> {
    let toks = lex(src)?;
    let mut p = Parser { toks, at: 0, refs: Vec::new(), declared: Default::default(), src };
    let mut stmts = Vec::new();
    while *p.peek() != Tok::End {
        p.statement(&mut stmts)?;
    }
    Ok(Program { stmts, refs: p.refs })
}

// ---------------------------------------------------------------------------------------------------------------------
// Evaluation

#[derive(Debug, Clone, PartialEq)]
enum Val {
    F(f64),
    V(DVec3),
    S(String),
}

impl Val {
    fn f(&self) -> f64 {
        match self {
            Val::F(x) => *x,
            Val::V(v) => v.length(),
            Val::S(s) => s.trim().parse().unwrap_or(0.0),
        }
    }
    fn v(&self) -> DVec3 {
        match self {
            Val::V(v) => *v,
            o => DVec3::splat(o.f()),
        }
    }
    fn truth(&self) -> bool {
        match self {
            Val::F(x) => *x != 0.0,
            Val::V(v) => *v != DVec3::ZERO,
            Val::S(s) => !s.is_empty(),
        }
    }
    fn text(&self) -> String {
        match self {
            Val::F(x) => x.to_string(),
            Val::V(v) => format!("{} {} {}", v.x, v.y, v.z),
            Val::S(s) => s.clone(),
        }
    }
}

fn hash3(x: i64, y: i64, z: i64) -> f64 {
    let mut h = (x as u64).wrapping_mul(0x9E3779B97F4A7C15) ^ (y as u64).wrapping_mul(0xC2B2AE3D27D4EB4F) ^ (z as u64).wrapping_mul(0x165667B19E3779F9);
    h ^= h >> 29;
    h = h.wrapping_mul(0xBF58476D1CE4E5B9);
    h ^= h >> 32;
    (h >> 11) as f64 / (1u64 << 53) as f64
}

fn gradient(x: i64, y: i64, z: i64) -> DVec3 {
    let (u, w) = (hash3(x, y, z) * 2.0 - 1.0, hash3(x + 101, y - 37, z + 59) * std::f64::consts::TAU);
    let r = (1.0 - u * u).max(0.0).sqrt();
    DVec3::new(r * w.cos(), r * w.sin(), u)
}

/// Smooth noise in -1 to 1, 0 at whole numbers.
fn noise(p: DVec3) -> f64 {
    let base = p.floor();
    let t = p - base;
    let (x, y, z) = (base.x as i64, base.y as i64, base.z as i64);
    let fade = |a: f64| a * a * a * (a * (a * 6.0 - 15.0) + 10.0);
    let u = DVec3::new(fade(t.x), fade(t.y), fade(t.z));
    let corner = |i: i64, j: i64, k: i64| gradient(x + i, y + j, z + k).dot(t - DVec3::new(i as f64, j as f64, k as f64));
    let lerp = |a: f64, c: f64, s: f64| a + (c - a) * s;
    let n = lerp(
        lerp(lerp(corner(0, 0, 0), corner(1, 0, 0), u.x), lerp(corner(0, 1, 0), corner(1, 1, 0), u.x), u.y),
        lerp(lerp(corner(0, 0, 1), corner(1, 0, 1), u.x), lerp(corner(0, 1, 1), corner(1, 1, 1), u.x), u.y),
        u.z,
    );
    (n * 1.5).clamp(-1.0, 1.0)
}

enum Flow {
    Normal,
    Break,
    Continue,
}

/// How many statements one evaluation may run: a loop that never ends stops here rather than hanging the scene.
const STEP_LIMIT: u64 = 2_000_000;

struct Run<'a> {
    locals: HashMap<String, (Val, Kind)>,
    globals: &'a mut HashMap<String, (Val, Kind)>,
    global_names: Vec<String>,
    rng: &'a mut u64,
    time: f64,
    frame: f64,
    /// Current value per ref: inputs as they arrived, outputs as last assigned.
    attrs: Vec<f64>,
    steps: u64,
}

impl Run<'_> {
    fn random(&mut self) -> f64 {
        *self.rng = self.rng.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = *self.rng;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        ((z ^ (z >> 31)) >> 11) as f64 / (1u64 << 53) as f64
    }

    fn get_var(&self, n: &str) -> Val {
        let slot = if self.global_names.iter().any(|g| g == n) { self.globals.get(n) } else { self.locals.get(n) };
        slot.map(|s| s.0.clone()).unwrap_or(Val::F(0.0))
    }

    fn set_var(&mut self, n: &str, v: Val) {
        let global = self.global_names.iter().any(|g| g == n);
        let store = if global { &mut *self.globals } else { &mut self.locals };
        match store.get_mut(n) {
            Some(slot) => slot.0 = coerce(v, slot.1),
            None => {
                let kind = match v {
                    Val::V(_) => Kind::Vector,
                    Val::S(_) => Kind::Str,
                    Val::F(_) => Kind::Float,
                };
                store.insert(n.to_string(), (v, kind));
            }
        }
    }

    fn eval(&mut self, e: &Expr) -> Val {
        match e {
            Expr::Num(n) => Val::F(*n),
            Expr::Str(s) => Val::S(s.clone()),
            Expr::Vector(c) => Val::V(DVec3::new(self.eval(&c[0]).f(), self.eval(&c[1]).f(), self.eval(&c[2]).f())),
            Expr::Var(n) => self.get_var(n),
            Expr::Attr(k) => Val::F(self.attrs[*k]),
            Expr::Time => Val::F(self.time),
            Expr::Frame => Val::F(self.frame),
            Expr::Neg(x) => match self.eval(x) {
                Val::V(v) => Val::V(-v),
                o => Val::F(-o.f()),
            },
            Expr::Not(x) => Val::F(if self.eval(x).truth() { 0.0 } else { 1.0 }),
            Expr::Bin("&&", a, c) => Val::F((self.eval(a).truth() && self.eval(c).truth()) as i32 as f64),
            Expr::Bin("||", a, c) => Val::F((self.eval(a).truth() || self.eval(c).truth()) as i32 as f64),
            Expr::Bin(op, a, c) => {
                let (x, y) = (self.eval(a), self.eval(c));
                binary(op, &x, &y)
            }
            Expr::Ternary(c, y, n) => {
                if self.eval(c).truth() {
                    self.eval(y)
                } else {
                    self.eval(n)
                }
            }
            Expr::Member(x, c) => Val::F(self.eval(x).v()[*c]),
            Expr::Call(name, args) => {
                let vals: Vec<Val> = args.iter().map(|a| self.eval(a)).collect();
                self.call(name, &vals)
            }
        }
    }

    fn call(&mut self, name: &str, a: &[Val]) -> Val {
        let x = a.first().map(Val::f).unwrap_or(0.0);
        let y = a.get(1).map(Val::f).unwrap_or(0.0);
        let z = a.get(2).map(Val::f).unwrap_or(0.0);
        let f = match name {
            "abs" => match &a[0] {
                Val::V(v) => return Val::V(v.abs()),
                _ => x.abs(),
            },
            "sign" => {
                if x > 0.0 {
                    1.0
                } else if x < 0.0 {
                    -1.0
                } else {
                    0.0
                }
            }
            "floor" => x.floor(),
            "ceil" => x.ceil(),
            "trunc" => x.trunc(),
            "round" => x.round(),
            "frac" => x - x.floor(),
            "sqrt" => x.sqrt(),
            "exp" => x.exp(),
            "log" => x.ln(),
            "log10" => x.log10(),
            "pow" => x.powf(y),
            "fmod" => x % y,
            "hypot" => x.hypot(y),
            "sin" => x.sin(),
            "cos" => x.cos(),
            "tan" => x.tan(),
            "asin" => x.asin(),
            "acos" => x.acos(),
            "atan" => x.atan(),
            "atan2" => x.atan2(y),
            "sinh" => x.sinh(),
            "cosh" => x.cosh(),
            "tanh" => x.tanh(),
            "sind" => x.to_radians().sin(),
            "cosd" => x.to_radians().cos(),
            "tand" => x.to_radians().tan(),
            "asind" => x.asin().to_degrees(),
            "acosd" => x.acos().to_degrees(),
            "atand" => x.atan().to_degrees(),
            "atan2d" => x.atan2(y).to_degrees(),
            "deg_to_rad" => x.to_radians(),
            "rad_to_deg" => x.to_degrees(),
            "min" => x.min(y),
            "max" => x.max(y),
            // MEL's order: clamp(low, high, value).
            "clamp" => z.max(x).min(y),
            "linstep" => {
                if y == x {
                    0.0
                } else {
                    ((z - x) / (y - x)).clamp(0.0, 1.0)
                }
            }
            "smoothstep" => {
                if y == x {
                    0.0
                } else {
                    let t = ((z - x) / (y - x)).clamp(0.0, 1.0);
                    t * t * (3.0 - 2.0 * t)
                }
            }
            "step" => (y >= x) as i32 as f64,
            "mag" => a[0].v().length(),
            "dot" => a[0].v().dot(a[1].v()),
            "angle" => a[0].v().angle_between(a[1].v()),
            "unit" => return Val::V(a[0].v().normalize_or_zero()),
            "cross" => return Val::V(a[0].v().cross(a[1].v())),
            "noise" => match (a.len(), &a[0]) {
                (1, Val::V(v)) => noise(*v),
                _ => noise(DVec3::new(x, y, z)),
            },
            "dnoise" => {
                let p = a[0].v();
                return Val::V(DVec3::new(noise(p), noise(p + DVec3::new(31.4, 17.3, 5.9)), noise(p - DVec3::new(11.7, 43.1, 23.2))));
            }
            "rand" => {
                if let (1, Val::V(v)) = (a.len(), &a[0]) {
                    return Val::V(DVec3::new(self.random(), self.random(), self.random()) * *v);
                }
                let r = self.random();
                if a.len() == 1 {
                    r * x
                } else {
                    x + r * (y - x)
                }
            }
            "sphrand" => {
                let radius = a[0].v();
                loop {
                    let p = DVec3::new(self.random(), self.random(), self.random()) * 2.0 - DVec3::ONE;
                    if p.length_squared() <= 1.0 {
                        return Val::V(p * radius);
                    }
                }
            }
            "gauss" => {
                let (u, w) = (self.random().max(1e-12), self.random());
                (-2.0 * u.ln()).sqrt() * (std::f64::consts::TAU * w).cos() * x
            }
            "seed" => {
                *self.rng = x as i64 as u64;
                x
            }
            "print" => {
                log::info!("{}", a[0].text());
                0.0
            }
            _ => 0.0,
        };
        Val::F(f)
    }

    fn block(&mut self, stmts: &[Stmt]) -> Flow {
        for s in stmts {
            self.steps += 1;
            if self.steps > STEP_LIMIT {
                return Flow::Break;
            }
            match self.stmt(s) {
                Flow::Normal => {}
                other => return other,
            }
        }
        Flow::Normal
    }

    fn stmt(&mut self, s: &Stmt) -> Flow {
        match s {
            Stmt::Decl { kind, name, init, global } => {
                let init_val = init.as_ref().map(|e| coerce(self.eval(e), *kind));
                if *global {
                    if !self.global_names.contains(name) {
                        self.global_names.push(name.clone());
                    }
                    // A global keeps its value across evaluations; its initial value is given once.
                    self.globals.entry(name.clone()).or_insert_with(|| (init_val.unwrap_or_else(|| zero(*kind)), *kind));
                } else {
                    self.locals.insert(name.clone(), (init_val.unwrap_or_else(|| zero(*kind)), *kind));
                }
            }
            Stmt::Assign(t, op, e) => {
                let right = self.eval(e);
                let current = |r: &Self| match t {
                    Target::Var(n) => r.get_var(n),
                    Target::Member(n, c) => Val::F(r.get_var(n).v()[*c]),
                    Target::Attr(k) => Val::F(r.attrs[*k]),
                };
                let v = match op {
                    Some(o) => binary(o, &current(self), &right),
                    None => right,
                };
                match t {
                    Target::Var(n) => self.set_var(n, v),
                    Target::Member(n, c) => {
                        let mut x = self.get_var(n).v();
                        x[*c] = v.f();
                        self.set_var(n, Val::V(x));
                    }
                    Target::Attr(k) => self.attrs[*k] = v.f(),
                }
            }
            Stmt::Expr(e) => {
                self.eval(e);
            }
            Stmt::If(c, yes, no) => {
                let branch = if self.eval(c).truth() { yes } else { no };
                return self.block(branch);
            }
            Stmt::While(c, body) => {
                while self.steps <= STEP_LIMIT && self.eval(c).truth() {
                    self.steps += 1;
                    if matches!(self.block(body), Flow::Break) {
                        break;
                    }
                }
            }
            Stmt::DoWhile(body, c) => loop {
                self.steps += 1;
                if matches!(self.block(body), Flow::Break) || self.steps > STEP_LIMIT || !self.eval(c).truth() {
                    break;
                }
            },
            Stmt::For(init, cond, step, body) => {
                self.block(init);
                while self.steps <= STEP_LIMIT && cond.as_ref().is_none_or(|c| self.eval(c).truth()) {
                    self.steps += 1;
                    if matches!(self.block(body), Flow::Break) {
                        break;
                    }
                    self.block(step);
                }
            }
            Stmt::Break => return Flow::Break,
            Stmt::Continue => return Flow::Continue,
        }
        Flow::Normal
    }
}

fn zero(k: Kind) -> Val {
    match k {
        Kind::Vector => Val::V(DVec3::ZERO),
        Kind::Str => Val::S(String::new()),
        _ => Val::F(0.0),
    }
}

fn coerce(v: Val, k: Kind) -> Val {
    match k {
        Kind::Int => Val::F(v.f().trunc()),
        Kind::Float => Val::F(v.f()),
        Kind::Vector => Val::V(v.v()),
        Kind::Str => Val::S(v.text()),
    }
}

fn binary(op: &str, x: &Val, y: &Val) -> Val {
    if let (Val::S(_), "+") | (_, "+") = (x, op) {
        if matches!(x, Val::S(_)) || matches!(y, Val::S(_)) {
            return Val::S(x.text() + &y.text());
        }
    }
    let flag = |b: bool| Val::F(b as i32 as f64);
    match op {
        "==" => return flag(if matches!(x, Val::S(_)) { x.text() == y.text() } else { x == y || x.f() == y.f() && !matches!(x, Val::V(_)) }),
        "!=" => return flag(if matches!(x, Val::S(_)) { x.text() != y.text() } else { !(x == y || x.f() == y.f() && !matches!(x, Val::V(_))) }),
        "<" => return flag(x.f() < y.f()),
        "<=" => return flag(x.f() <= y.f()),
        ">" => return flag(x.f() > y.f()),
        ">=" => return flag(x.f() >= y.f()),
        _ => {}
    }
    if matches!(x, Val::V(_)) || matches!(y, Val::V(_)) {
        let (a, c) = (x.v(), y.v());
        let both = matches!(x, Val::V(_)) && matches!(y, Val::V(_));
        return match op {
            "+" => Val::V(a + c),
            "-" => Val::V(a - c),
            "*" if both => Val::F(a.dot(c)),
            "*" => Val::V(a * c),
            "/" => Val::V(a / c),
            "^" => Val::V(a.cross(c)),
            _ => Val::V(DVec3::new(a.x % c.x, a.y % c.y, a.z % c.z)),
        };
    }
    let (a, c) = (x.f(), y.f());
    Val::F(match op {
        "+" => a + c,
        "-" => a - c,
        "*" => a * c,
        "/" => {
            if c == 0.0 {
                0.0
            } else {
                a / c
            }
        }
        "^" => a.powf(c),
        _ => a % c,
    })
}

// ---------------------------------------------------------------------------------------------------------------------
// The node

pub const EXPRESSION: usize = 0;
pub const OBJECT: usize = 1;
pub const ALWAYS_EVALUATE: usize = 2;
pub const UNIT_OPTION: usize = 3;
pub const INPUT: usize = 4;
pub const OUTPUT: usize = 5;
pub const RADIANS: usize = 6;
pub const ANIMATED: usize = 7;

#[derive(Default)]
struct Expression {
    compiled: Option<(String, Arc<Program>)>,
    globals: HashMap<String, (Val, Kind)>,
    rng: u64,
}

impl NodeBehavior for Expression {
    fn compute(&mut self, ctx: &EvalContext, d: &mut NodeData) {
        let src = d.string(EXPRESSION);
        if self.compiled.as_ref().is_none_or(|c| c.0 != src) {
            self.compiled = parse(src).ok().map(|p| (src.to_string(), Arc::new(p)));
        }
        let Some((_, program)) = &self.compiled else { return };
        let program = program.clone();
        let slots = program.slots();
        // Refs whose slot is angular, spelled i3 or o2: the script sees them in the current unit, except under -uc none.
        let radians: Vec<String> = d.string(RADIANS).split_whitespace().map(str::to_string).collect();
        let shown_unit = |out: bool, k: i32| {
            let slot = format!("{}{k}", if out { "o" } else { "i" });
            if radians.iter().any(|r| *r == slot) {
                aztek_ops::units::Unit::Angle
            } else if radians.iter().any(|r| r.strip_prefix('L') == Some(slot.as_str())) {
                aztek_ops::units::Unit::Linear
            } else {
                aztek_ops::units::Unit::None
            }
        };
        let attrs: Vec<f64> = slots
            .iter()
            .map(|&(out, k)| {
                let v = d.element_or(if out { OUTPUT } else { INPUT }, k).scalar().unwrap_or(0.0);
                aztek_ops::units::to_user(shown_unit(out, k), v)
            })
            .collect();
        if self.rng == 0 {
            self.rng = 0x5EED_1234_ABCD_0001;
        }
        let mut run = Run {
            locals: HashMap::new(),
            globals: &mut self.globals,
            global_names: Vec::new(),
            rng: &mut self.rng,
            time: ctx.time,
            frame: {
                let f = ctx.time * crate::anim::fps();
                if (f - f.round()).abs() < 1e-6 {
                    f.round()
                } else {
                    f
                }
            },
            attrs,
            steps: 0,
        };
        run.block(&program.stmts);
        let values = std::mem::take(&mut run.attrs);
        if d.elements.len() <= OUTPUT {
            d.elements.resize_with(d.values.len(), Vec::new);
        }
        let mut out: Vec<(i32, PlugValue)> = Vec::new();
        for (&(is_out, k), v) in slots.iter().zip(values) {
            if is_out {
                let v = aztek_ops::units::to_stored(shown_unit(true, k), v);
                out.push((k, PlugValue::Float(v)));
            }
        }
        out.sort_by_key(|e| e.0);
        d.elements[OUTPUT] = out;
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub fn register_types() {
    if types::find("expression").is_some() {
        return;
    }
    let attr = |name: &str, short: &str, ty: PlugType, v: PlugValue, fl: u32| AttrDef { name: name.into(), short_name: short.into(), ty, default_value: v, flags: fl, ..Default::default() };
    let store = flags::INPUT | flags::STORABLE;
    let mut unit = attr("unitOption", "uno", PlugType::Enum, PlugValue::Int(0), store);
    unit.enum_names = vec!["all".into(), "none".into(), "angularOnly".into()];
    types::register_type(NodeType {
        name: "expression".into(),
        doc: "Runs a small program that reads and assigns attributes, whenever its inputs or the time change.".into(),
        time_dependent: true,
        own_attrs: vec![
            attr("expression", "ixp", PlugType::String, PlugValue::String(String::new()), store | flags::CODE),
            attr("object", "obj", PlugType::String, PlugValue::String(String::new()), store),
            attr("alwaysEvaluate", "ae", PlugType::Int, PlugValue::Int(1), store),
            unit,
            attr("input", "in", PlugType::Float, PlugValue::Float(0.0), store | flags::ARRAY | flags::HIDDEN),
            attr("output", "out", PlugType::Float, PlugValue::Float(0.0), flags::OUTPUT | flags::ARRAY | flags::HIDDEN),
            attr("radianSlots", "rds", PlugType::String, PlugValue::String(String::new()), store | flags::HIDDEN),
            attr("animated", "ani", PlugType::Int, PlugValue::Int(0), store),
            attr("objectMsg", "obm", PlugType::Int, PlugValue::Int(0), flags::INPUT | flags::HIDDEN | flags::MESSAGE),
        ],
        create: Some(Arc::new(|| Box::new(Expression::default()))),
        ..Default::default()
    })
    .expect("expression");
}

// ---------------------------------------------------------------------------------------------------------------------
// The command

/// The plug name a token stands for, given the expression's object.
fn plug_path(token: &str, object: &str) -> Result<String, String> {
    if token.starts_with('.') {
        if object.is_empty() {
            return Err(format!("{token}: no object given with -o"));
        }
        return Ok(format!("{object}{token}"));
    }
    if token.contains('.') {
        return Ok(token.to_string());
    }
    if object.is_empty() {
        return Err(format!("Attribute not found or variable missing '$': {token}"));
    }
    Ok(format!("{object}.{token}"))
}

struct Bound {
    plug: PlugRef,
    out: bool,
    slot: i32,
}

/// Resolves every attribute the program names, refusing what an expression can't use.
fn bind(g: &Graph, program: &Program, object: &str) -> Result<Vec<Bound>, String> {
    let mut out = Vec::new();
    for (r, (is_out, slot)) in program.refs.iter().zip(program.slots()) {
        let path = plug_path(&r.token, object)?;
        let plug = g.plug(&path).map_err(|_| format!("Attribute not found or variable missing '$': {path}"))?;
        let a = g.attr(&plug);
        let numeric = plug.component >= 0 || matches!(a.ty, PlugType::Bool | PlugType::Int | PlugType::Float | PlugType::Enum);
        if !numeric || (a.is_array() && plug.element < 0) {
            return Err(format!("{path}: an expression reads and assigns single numbers; name one channel"));
        }
        if is_out {
            if !a.has(flags::INPUT) {
                return Err(format!("{path} is an output and can't be assigned"));
            }
            if g.is_locked(&plug) {
                return Err(format!("{path} is locked and can't be assigned"));
            }
            let whole = PlugRef { component: -1, ..plug };
            if g.source(&plug).is_some() || (plug.component >= 0 && g.source(&whole).is_some()) {
                return Err(format!("{path} is already driven by a connection"));
            }
        }
        out.push(Bound { plug, out: is_out, slot });
    }
    Ok(out)
}

fn is_angular(g: &Graph, p: &PlugRef) -> bool {
    g.node(p.node).is_some_and(|n| n.ty().attrs[p.attr as usize].unit == aztek_ops::units::Unit::Angle)
}

/// Replaces what the node drives and reads with what `src` needs. Errors leave the caller to undo.
fn apply(g: &mut Graph, id: NodeId, src: &str, object: &str, unit: i64) -> CmdResult<()> {
    let program = parse(src).map_err(aztek_ops::CommandError::new)?;
    disconnect_all(g, id);
    let bound = bind(g, &program, object).map_err(aztek_ops::CommandError::new)?;
    let mut radians = Vec::new();
    for b in &bound {
        let side = if b.out { "o" } else { "i" };
        if unit != 1 && is_angular(g, &b.plug) {
            radians.push(format!("{side}{}", b.slot));
        } else if unit == 0 && g.node(b.plug.node).is_some_and(|n| n.ty().attrs[b.plug.attr as usize].unit == aztek_ops::units::Unit::Linear) {
            radians.push(format!("L{side}{}", b.slot));
        }
    }
    g.set_value(PlugRef::new(id, RADIANS as i32), PlugValue::String(radians.join(" ")))?;
    g.set_value(PlugRef::new(id, EXPRESSION as i32), PlugValue::String(src.to_string()))?;
    g.set_value(PlugRef::new(id, OBJECT as i32), PlugValue::String(object.to_string()))?;
    let has_time = src.contains("time") || src.contains("frame");
    g.set_value(PlugRef::new(id, ANIMATED as i32), PlugValue::Int(has_time as i64))?;
    for b in &bound {
        if b.out {
            g.connect(&PlugRef::element(id, OUTPUT as i32, b.slot), &b.plug, false)?;
        } else {
            g.connect(&b.plug, &PlugRef::element(id, INPUT as i32, b.slot), false)?;
        }
    }
    Ok(())
}

fn disconnect_all(g: &mut Graph, id: NodeId) {
    for (d, s) in g.connections() {
        if (d.node == id && d.attr == INPUT as i32) || (s.node == id && s.attr == OUTPUT as i32) {
            g.disconnect(&d);
        }
    }
}

/// The source with its attribute tokens naming today's nodes, as renames leave them.
pub fn current_source(g: &Graph, id: NodeId) -> String {
    let Some(n) = g.node(id) else { return String::new() };
    let src = n.data().string(EXPRESSION).to_string();
    let Ok(program) = parse(&src) else { return src };
    let conns = g.connections();
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    for (r, (is_out, slot)) in program.refs.iter().zip(program.slots()) {
        let Some((node_part, attr_part)) = r.token.split_once('.') else { continue };
        if node_part.is_empty() {
            continue;
        }
        let plug = if is_out {
            conns.iter().find(|(_, s)| s.node == id && s.attr == OUTPUT as i32 && s.element == slot).map(|(d, _)| *d)
        } else {
            conns.iter().find(|(d, _)| d.node == id && d.attr == INPUT as i32 && d.element == slot).map(|(_, s)| *s)
        };
        let Some(p) = plug else { continue };
        let text = format!("{}.{attr_part}", g.path(p.node));
        edits.extend(r.spans.iter().map(|&(s, e)| (s, e, text.clone())));
    }
    edits.sort_by_key(|e| std::cmp::Reverse(e.0));
    let mut out = src;
    for (s, e, t) in edits {
        out.replace_range(s..e, &t);
    }
    out
}

/// The object an expression was made with, following a rename of the node its attributes are on.
fn current_object(g: &Graph, id: NodeId) -> String {
    let Some(n) = g.node(id) else { return String::new() };
    let stored = n.data().string(OBJECT).to_string();
    if stored.is_empty() || g.find(&stored).ok().flatten().is_some() {
        return stored;
    }
    // Renamed: anything it drives or reads through a bare or dotted token is on that node.
    let Ok(program) = parse(n.data().string(EXPRESSION)) else { return stored };
    let conns = g.connections();
    for (r, (is_out, slot)) in program.refs.iter().zip(program.slots()) {
        if r.token.split_once('.').is_some_and(|(o, _)| !o.is_empty()) {
            continue;
        }
        let plug = if is_out {
            conns.iter().find(|(_, s)| s.node == id && s.attr == OUTPUT as i32 && s.element == slot).map(|(d, _)| *d)
        } else {
            conns.iter().find(|(d, _)| d.node == id && d.attr == INPUT as i32 && d.element == slot).map(|(_, s)| *s)
        };
        if let Some(p) = plug {
            return g.path(p.node);
        }
    }
    stored
}

/// Expressions that drove something and, once `doomed` goes, drive nothing: the reference deletes them with their objects.
pub fn orphans(g: &Graph, doomed: &[NodeId]) -> Vec<NodeId> {
    let mut driving: HashMap<NodeId, bool> = HashMap::new();
    for (d, s) in g.connections() {
        if s.attr == OUTPUT as i32 && g.node(s.node).is_some_and(|n| n.ty().name == "expression") {
            let stays = !doomed.contains(&d.node);
            let e = driving.entry(s.node).or_insert(false);
            *e = *e || stays;
        }
    }
    driving.into_iter().filter(|(id, alive)| !*alive && !doomed.contains(id)).map(|(id, _)| id).collect()
}

fn run_command(s: &mut Session, a: &Args) -> CmdResult {
    let target = |s: &Session| -> CmdResult<NodeId> {
        let name = a.pos_str(0).map(str::to_string).or_else(|| a.get("name").and_then(Value::as_str).map(str::to_string));
        let id = match name {
            Some(n) => s.graph.find(&n)?,
            None => s.graph.selection().iter().rev().copied().find(|&i| s.graph.node(i).is_some_and(|n| n.ty().name == "expression")),
        };
        match id {
            Some(i) if s.graph.node(i).is_some_and(|n| n.ty().name == "expression") => Ok(i),
            _ => fail!("expression: no expression node given"),
        }
    };
    let unit_of = |name: &str| ["all", "none", "angularOnly"].iter().position(|u| *u == name).unwrap_or(0) as i64;
    if a.bool("query") {
        let id = target(s)?;
        let g = &s.graph;
        let n = g.node(id).unwrap();
        let asked: Vec<String> = a.get("queried").and_then(Value::as_array).map(|v| v.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default();
        return Ok(match asked.first().map(String::as_str) {
            Some("object") => json!(current_object(g, id)),
            Some("alwaysEvaluate") => json!(n.int(ALWAYS_EVALUATE)),
            Some("unitConversion") => json!(["all", "none", "angularOnly"][n.int(UNIT_OPTION).clamp(0, 2) as usize]),
            Some("name") => json!(g.path(id)),
            Some("timeDependent") => json!(n.int(ANIMATED) != 0),
            _ => json!(current_source(g, id)),
        });
    }
    if a.bool("edit") {
        let id = target(s)?;
        let g = &mut s.graph;
        if a.has("alwaysEvaluate") {
            g.set_value(PlugRef::new(id, ALWAYS_EVALUATE as i32), PlugValue::Int(a.int("alwaysEvaluate", 1)))?;
        }
        if a.has("unitConversion") {
            g.set_value(PlugRef::new(id, UNIT_OPTION as i32), PlugValue::Int(unit_of(&a.string("unitConversion", "all"))))?;
        }
        let object = if a.has("object") { a.string("object", "") } else { current_object(g, id) };
        let src = if a.has("string") { a.string("string", "") } else { current_source(g, id) };
        let unit = g.node(id).unwrap().int(UNIT_OPTION);
        if a.has("string") || a.has("object") || a.has("unitConversion") {
            // Refuse a bad edit before anything changes.
            parse(&src).map_err(aztek_ops::CommandError::new)?;
            let mark = current_source(g, id);
            let old_object = current_object(g, id);
            if let Err(e) = apply(g, id, &src, &object, unit) {
                let _ = apply(g, id, &mark, &old_object, unit);
                return Err(e);
            }
        }
        if a.has("name") {
            g.rename(id, &a.string("name", ""))?;
        }
        return Ok(json!(g.path(id)));
    }
    if !a.has("string") {
        fail!("expression: give the source with -s");
    }
    let src = a.string("string", "");
    let object = a.string("object", "");
    let g = &mut s.graph;
    let program = parse(&src).map_err(aztek_ops::CommandError::new)?;
    bind(g, &program, &object).map_err(aztek_ops::CommandError::new)?;
    let ty = types::find("expression").unwrap();
    let id = g.create_node(&ty, &a.string("name", ""), NO_NODE, true)?;
    let unit = unit_of(&a.string("unitConversion", "all"));
    g.set_value(PlugRef::new(id, UNIT_OPTION as i32), PlugValue::Int(unit))?;
    g.set_value(PlugRef::new(id, ALWAYS_EVALUATE as i32), PlugValue::Int(a.int("alwaysEvaluate", 1)))?;
    if let Err(e) = apply(g, id, &src, &object, unit) {
        disconnect_all(g, id);
        g.delete_node(id)?;
        return Err(e);
    }
    Ok(json!(g.path(id)))
}

pub fn register(c: &mut Commands<Session>) {
    let f = |n: &str, sh: &str, ty: ArgType, doc: &str| FlagDef::new(n, sh, ty, Value::Null, doc);
    c.register(
        CommandDef::<Session>::new(
            "expression",
            "Create, edit or query an expression node: expression -s \"pCube1.tx = sin(time);\" -o pCube1 -ae 1 -uc all -n name. \
             Errors in the source fail the command. Returns the node's name.",
            run_command,
        )
        .positional("[expression node, for -q and -e]")
        .flag(f("query", "q", ArgType::Bool, "Query."))
        .flag(f("edit", "e", ArgType::Bool, "Edit."))
        .flag(f("queried", "", ArgType::List, "What -q asks for."))
        .flag(f("string", "s", ArgType::String, "The source."))
        .flag(f("object", "o", ArgType::String, "The object bare attribute names (tx, .tx) are on."))
        .flag(f("alwaysEvaluate", "ae", ArgType::Int, "Evaluate whenever asked (1, the default)."))
        .flag(f("unitConversion", "uc", ArgType::String, "all, none or angularOnly. With none, angles read and assign in radians.").choices(&["all", "none", "angularOnly"]))
        .flag(f("name", "n", ArgType::String, "The node's name."))
        .flag(f("shortNames", "sn", ArgType::Bool, "Accepted for existing scripts."))
        .flag(f("safe", "sf", ArgType::Bool, "Accepted for existing scripts."))
        .flag(f("timeDependent", "td", ArgType::Bool, "With -q: whether it reads time or frame.")),
    );
}
