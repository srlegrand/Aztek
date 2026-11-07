//! MEL's grammar. A statement starting with a name is command syntax (`polyCube -w 2;`) unless it reads as one call
//! closed by the end of the statement (`foo(1, "a");`).

use std::sync::Arc;

use super::lexer::{Lexer, Token};
use super::value::Ty;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Cross,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Debug)]
pub enum Expr {
    Int(i32),
    Float(f64),
    Str(String),
    Var(String),
    Vector(Vec<Expr>),
    Matrix(Vec<Vec<Expr>>),
    Array(Vec<Expr>),
    Index(Box<Expr>, Box<Expr>),
    /// `.x`, `.y`, `.z` as 0, 1, 2.
    Member(Box<Expr>, usize),
    Neg(Box<Expr>),
    Not(Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    Ternary(Box<Expr>, Box<Expr>, Box<Expr>),
    /// target, the operator of `+=` and the like (None for `=`), value.
    Assign(Box<Expr>, Option<BinOp>, Box<Expr>),
    /// target, +1 or -1, prefix.
    Step(Box<Expr>, i32, bool),
    Cast(Ty, Box<Expr>),
    /// `foo(1, "a")`
    Call(String, Vec<Expr>),
    /// `foo 1 "a"`, alone or in backticks.
    Command(String, Vec<CmdArg>),
}

#[derive(Debug)]
pub enum CmdArg {
    /// A bare word: a flag, or a string or number by its look.
    Word(String),
    /// A quoted string, variable, or parenthesised expression.
    Expr(Expr),
}

#[derive(Debug)]
pub struct Decl {
    pub name: String,
    pub base: Ty,
    pub array: bool,
    /// `[n]`, and `[r][c]` for matrices.
    pub dims: Vec<Expr>,
    pub init: Option<Expr>,
}

#[derive(Debug)]
pub struct Proc {
    pub name: String,
    pub global: bool,
    pub ret: Option<Ty>,
    pub params: Vec<(Ty, String)>,
    pub body: Vec<Stmt>,
}

#[derive(Debug)]
pub enum Stmt {
    Empty,
    Expr(Expr),
    Decl(bool, Vec<Decl>),
    Block(Vec<Stmt>),
    If(Expr, Box<Stmt>, Option<Box<Stmt>>),
    While(Expr, Box<Stmt>),
    DoWhile(Box<Stmt>, Expr),
    For(Vec<Stmt>, Option<Expr>, Vec<Expr>, Box<Stmt>),
    ForIn(Option<Ty>, String, Expr, Box<Stmt>),
    /// subject, then (case value or None for default, statements).
    Switch(Expr, Vec<(Option<Expr>, Vec<Stmt>)>),
    Break,
    Continue,
    Return(Option<Expr>),
    Proc(Arc<Proc>),
}

/// A top-level statement and its source, for the echo.
#[derive(Debug)]
pub struct Top {
    pub stmt: Stmt,
    pub text: String,
}

/// Deep nesting past this is refused: the tree it would make has to be walked and dropped.
const MAX_DEPTH: usize = 400;
const MAX_CHAIN: usize = 2000;

struct Parser {
    lx: Lexer,
    pos: usize,
    depth: usize,
    /// Backticks open in the command being read.
    ticks: usize,
}

type P<T> = Result<T, String>;

fn type_keyword(s: &str) -> Option<Ty> {
    Some(match s {
        "int" => Ty::Int,
        "float" => Ty::Float,
        "string" => Ty::Str,
        "vector" => Ty::Vector,
        "matrix" => Ty::Matrix,
        _ => return None,
    })
}

/// Parses a whole source. On failure, the line and message: nothing of the source has run, as in the reference.
pub fn parse_program(src: &str) -> Result<Vec<Top>, (usize, String)> {
    let mut p = Parser { lx: Lexer::new(src), pos: 0, depth: 0, ticks: 0 };
    let mut out = Vec::new();
    loop {
        let r = (|| -> P<bool> {
            let start = p.lx.skip_space(p.pos)?;
            p.pos = start;
            if p.peek()? == Token::Eof {
                return Ok(false);
            }
            let stmt = p.statement(true)?;
            let t = p.lx.src[start..p.pos].iter().collect::<String>();
            let t = t.trim();
            if !matches!(stmt, Stmt::Empty) {
                let simple = matches!(stmt, Stmt::Expr(_) | Stmt::Decl(..) | Stmt::Break | Stmt::Continue | Stmt::Return(_));
                let text = if simple { format!("{};", t.strip_suffix(';').unwrap_or(t).trim_end()) } else { t.to_string() };
                out.push(Top { stmt, text });
            }
            Ok(true)
        })();
        match r {
            Ok(true) => {}
            Ok(false) => return Ok(out),
            Err(e) => {
                let line = p.lx.line_of(p.pos);
                return Err((line, e));
            }
        }
    }
}

impl Parser {
    fn peek(&self) -> P<Token> {
        Ok(self.lx.token(self.pos)?.0)
    }
    fn bump(&mut self) -> P<Token> {
        let (t, e) = self.lx.token(self.pos)?;
        self.pos = e;
        Ok(t)
    }
    fn is(&self, p: &str) -> P<bool> {
        Ok(matches!(self.peek()?, Token::Punct(x) if x == p))
    }
    fn eat(&mut self, p: &str) -> P<bool> {
        if self.is(p)? {
            self.bump()?;
            return Ok(true);
        }
        Ok(false)
    }
    fn expect(&mut self, p: &str) -> P<()> {
        if self.eat(p)? {
            Ok(())
        } else {
            self.syntax()
        }
    }
    fn syntax<T>(&mut self) -> P<T> {
        // Point at what could not be read.
        if let Ok(i) = self.lx.skip_space(self.pos) {
            self.pos = i;
        }
        Err("Syntax error".into())
    }
    fn is_word(&self, w: &str) -> P<bool> {
        Ok(matches!(self.peek()?, Token::Ident(x) if x == w))
    }

    fn nested<T>(&mut self, f: impl FnOnce(&mut Parser) -> P<T>) -> P<T> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err("Script nested too deeply".into());
        }
        let r = stacker::maybe_grow(128 * 1024, 2 * 1024 * 1024, || f(self));
        self.depth -= 1;
        r
    }

    /// `;` closes a statement; the last one before `}` or the end may leave it out.
    fn end_of_statement(&mut self) -> P<()> {
        if self.eat(";")? || self.is("}")? || self.peek()? == Token::Eof {
            Ok(())
        } else {
            self.syntax()
        }
    }

    fn statement(&mut self, top: bool) -> P<Stmt> {
        self.nested(|p| p.statement_inner(top))
    }

    fn statement_inner(&mut self, top: bool) -> P<Stmt> {
        match self.peek()? {
            Token::Punct(";") => {
                self.bump()?;
                Ok(Stmt::Empty)
            }
            Token::Punct("{") => Ok(Stmt::Block(self.block()?)),
            Token::Ident(w) => match w.as_str() {
                "if" => self.if_stmt(),
                "while" => {
                    self.bump()?;
                    self.expect("(")?;
                    let c = self.expr()?;
                    self.expect(")")?;
                    Ok(Stmt::While(c, Box::new(self.statement(false)?)))
                }
                "do" => {
                    self.bump()?;
                    let body = self.statement(false)?;
                    if !self.is_word("while")? {
                        return self.syntax();
                    }
                    self.bump()?;
                    self.expect("(")?;
                    let c = self.expr()?;
                    self.expect(")")?;
                    self.end_of_statement()?;
                    Ok(Stmt::DoWhile(Box::new(body), c))
                }
                "for" => self.for_stmt(),
                "switch" => self.switch_stmt(),
                "break" => {
                    self.bump()?;
                    self.end_of_statement()?;
                    Ok(Stmt::Break)
                }
                "continue" => {
                    self.bump()?;
                    self.end_of_statement()?;
                    Ok(Stmt::Continue)
                }
                "return" => {
                    self.bump()?;
                    let v = if self.is(";")? || self.is("}")? || self.peek()? == Token::Eof { None } else { Some(self.expr()?) };
                    self.end_of_statement()?;
                    Ok(Stmt::Return(v))
                }
                "global" => {
                    self.bump()?;
                    if self.is_word("proc")? {
                        return self.proc_stmt(true, top);
                    }
                    let d = self.declaration()?;
                    self.end_of_statement()?;
                    Ok(Stmt::Decl(true, d))
                }
                "proc" => self.proc_stmt(false, top),
                "else" | "case" | "default" | "in" => self.syntax(),
                _ if type_keyword(&w).is_some() => {
                    // string($x) as a statement is a cast; anything else declares.
                    let save = self.pos;
                    self.bump()?;
                    let cast = self.is("(")?;
                    self.pos = save;
                    if cast {
                        let e = self.expr()?;
                        self.end_of_statement()?;
                        return Ok(Stmt::Expr(e));
                    }
                    let d = self.declaration()?;
                    self.end_of_statement()?;
                    Ok(Stmt::Decl(false, d))
                }
                "true" | "false" | "on" | "off" | "yes" | "no" => {
                    let e = self.expr()?;
                    self.end_of_statement()?;
                    Ok(Stmt::Expr(e))
                }
                _ => {
                    let e = self.named_statement(&w)?;
                    self.end_of_statement()?;
                    Ok(Stmt::Expr(e))
                }
            },
            _ => {
                let e = self.expr()?;
                self.end_of_statement()?;
                Ok(Stmt::Expr(e))
            }
        }
    }

    fn block(&mut self) -> P<Vec<Stmt>> {
        self.expect("{")?;
        let mut out = Vec::new();
        while !self.is("}")? {
            if self.peek()? == Token::Eof {
                return self.syntax();
            }
            let s = self.statement(false)?;
            if !matches!(s, Stmt::Empty) {
                out.push(s);
            }
        }
        self.bump()?;
        Ok(out)
    }

    fn if_stmt(&mut self) -> P<Stmt> {
        self.bump()?;
        self.expect("(")?;
        let c = self.expr()?;
        self.expect(")")?;
        let then = self.statement(false)?;
        let other = if self.is_word("else")? {
            self.bump()?;
            Some(Box::new(self.statement(false)?))
        } else {
            None
        };
        Ok(Stmt::If(c, Box::new(then), other))
    }

    fn for_stmt(&mut self) -> P<Stmt> {
        self.bump()?;
        self.expect("(")?;
        // for ($x in $arr), for (string $x in $arr)
        let save = self.pos;
        let ty = match self.peek()? {
            Token::Ident(w) => {
                let t = type_keyword(&w);
                if t.is_some() {
                    self.bump()?;
                }
                t
            }
            _ => None,
        };
        if let Token::Var(v) = self.peek()? {
            self.bump()?;
            if self.is_word("in")? {
                self.bump()?;
                let it = self.expr()?;
                self.expect(")")?;
                return Ok(Stmt::ForIn(ty, v, it, Box::new(self.statement(false)?)));
            }
        }
        self.pos = save;
        let mut init = Vec::new();
        if !self.is(";")? {
            if matches!(self.peek()?, Token::Ident(w) if type_keyword(&w).is_some()) {
                init.push(Stmt::Decl(false, self.declaration()?));
            } else {
                loop {
                    init.push(Stmt::Expr(self.expr()?));
                    if !self.eat(",")? {
                        break;
                    }
                }
            }
        }
        self.expect(";")?;
        let cond = if self.is(";")? { None } else { Some(self.expr()?) };
        self.expect(";")?;
        let mut step = Vec::new();
        if !self.is(")")? {
            loop {
                step.push(self.expr()?);
                if !self.eat(",")? {
                    break;
                }
            }
        }
        self.expect(")")?;
        Ok(Stmt::For(init, cond, step, Box::new(self.statement(false)?)))
    }

    fn switch_stmt(&mut self) -> P<Stmt> {
        self.bump()?;
        self.expect("(")?;
        let subject = self.expr()?;
        self.expect(")")?;
        self.expect("{")?;
        let mut cases = Vec::new();
        loop {
            if self.eat("}")? {
                break;
            }
            let label = if self.is_word("case")? {
                self.bump()?;
                Some(self.expr()?)
            } else if self.is_word("default")? {
                self.bump()?;
                None
            } else {
                return self.syntax();
            };
            self.expect(":")?;
            let mut body = Vec::new();
            while !self.is("}")? && !self.is_word("case")? && !self.is_word("default")? {
                if self.peek()? == Token::Eof {
                    return self.syntax();
                }
                let s = self.statement(false)?;
                if !matches!(s, Stmt::Empty) {
                    body.push(s);
                }
            }
            cases.push((label, body));
        }
        Ok(Stmt::Switch(subject, cases))
    }

    fn proc_stmt(&mut self, global: bool, top: bool) -> P<Stmt> {
        if !top {
            return self.syntax();
        }
        self.bump()?; // proc
        let Token::Ident(first) = self.bump()? else { return self.syntax() };
        let mut ret = None;
        let mut name = first.clone();
        if let Some(t) = type_keyword(&first) {
            // proc string[] foo(...), proc int foo(...)
            let mut t = t;
            if self.eat("[")? {
                self.expect("]")?;
                t = t.array_of();
            }
            ret = Some(t);
            let Token::Ident(n) = self.bump()? else { return self.syntax() };
            name = n;
        }
        self.expect("(")?;
        let mut params = Vec::new();
        if !self.is(")")? {
            loop {
                let Token::Ident(tw) = self.bump()? else { return self.syntax() };
                let Some(mut t) = type_keyword(&tw) else { return self.syntax() };
                let Token::Var(v) = self.bump()? else { return self.syntax() };
                while self.eat("[")? {
                    if !self.is("]")? {
                        self.expr()?;
                    }
                    self.expect("]")?;
                    t = t.array_of();
                }
                params.push((t, v));
                if !self.eat(",")? {
                    break;
                }
            }
        }
        self.expect(")")?;
        let body = self.block()?;
        Ok(Stmt::Proc(Arc::new(Proc { name, global, ret, params, body })))
    }

    fn declaration(&mut self) -> P<Vec<Decl>> {
        let Token::Ident(tw) = self.bump()? else { return self.syntax() };
        let Some(base) = type_keyword(&tw) else { return self.syntax() };
        let mut out = Vec::new();
        loop {
            let Token::Var(name) = self.bump()? else { return self.syntax() };
            let mut array = false;
            let mut dims = Vec::new();
            while self.eat("[")? {
                array = true;
                if !self.is("]")? {
                    dims.push(self.expr()?);
                }
                self.expect("]")?;
            }
            let init = if self.eat("=")? { Some(self.assignment()?) } else { None };
            out.push(Decl { name, base, array, dims, init });
            if !self.eat(",")? {
                return Ok(out);
            }
        }
    }

    /// A statement starting with a name: `foo(...)` closed by the statement's end is a call, as is one an operator
    /// carries on (`size($a) + 1`); anything else is command syntax, so `setAttr ("a" + $b) -1;` works.
    fn named_statement(&mut self, name: &str) -> P<Expr> {
        let start = self.lx.skip_space(self.pos)?;
        self.bump()?;
        if self.is("(")? {
            let save = self.pos;
            let mut depth = 0;
            loop {
                match self.bump()? {
                    Token::Punct("(") => depth += 1,
                    Token::Punct(")") => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    Token::Eof => return self.syntax(),
                    _ => {}
                }
            }
            let closes = matches!(self.peek(), Ok(Token::Punct(";" | "}" | "`")) | Ok(Token::Eof));
            if !closes && self.operator_follows() {
                self.pos = start;
                return self.expr();
            }
            self.pos = save;
            if closes {
                let args = self.call_args()?;
                return Ok(Expr::Call(name.to_string(), args));
            }
        }
        Ok(Expr::Command(name.to_string(), self.command_args()?))
    }

    /// Whether what follows a closing parenthesis is an operator rather than the next command argument. Comparisons
    /// always are. `+ - * / % ^` could start a word (`-1`, `*x`), so they count when they touch the parenthesis or stand
    /// clear of what comes next.
    fn operator_follows(&self) -> bool {
        let Ok((Token::Punct(p), end)) = self.lx.token(self.pos) else { return false };
        match p {
            "==" | "!=" | "<=" | ">=" | "<" | ">" | "&&" | "||" | "?" => true,
            "+" | "-" | "*" | "/" | "%" | "^" => {
                let touches = self.lx.at(self.pos).is_some_and(|c| !c.is_whitespace());
                touches || self.lx.at(end).is_none_or(char::is_whitespace)
            }
            _ => false,
        }
    }

    fn call_args(&mut self) -> P<Vec<Expr>> {
        self.expect("(")?;
        let mut args = Vec::new();
        if !self.is(")")? {
            loop {
                args.push(self.expr()?);
                if !self.eat(",")? {
                    break;
                }
            }
        }
        self.expect(")")?;
        Ok(args)
    }

    /// Reads command arguments up to `;`, `}`, a closing backtick or `)`, which are left unread.
    fn command_args(&mut self) -> P<Vec<CmdArg>> {
        let mut out = Vec::new();
        loop {
            let i = self.lx.skip_space(self.pos)?;
            self.pos = i;
            let Some(c) = self.lx.at(i) else { return Ok(out) };
            match c {
                ';' | '}' | ')' => return Ok(out),
                '`' => {
                    if !self.backtick_opens(i) {
                        return Ok(out);
                    }
                    out.push(CmdArg::Expr(self.backtick()?));
                }
                '"' | '{' => out.push(CmdArg::Expr(self.primary()?)),
                '(' => {
                    self.bump()?;
                    let e = self.expr()?;
                    self.expect(")")?;
                    out.push(CmdArg::Expr(e));
                }
                '$' => {
                    let v = self.bump()?;
                    let Token::Var(name) = v else { return self.syntax() };
                    out.push(CmdArg::Expr(self.postfix_adjacent(Expr::Var(name))?));
                }
                '<' if self.lx.at(i + 1) == Some('<') => out.push(CmdArg::Expr(self.primary()?)),
                _ => {
                    let mut e = i;
                    while let Some(c) = self.lx.at(e) {
                        if c.is_whitespace() || matches!(c, ';' | '`' | '"' | '(' | ')' | '{' | '}') {
                            break;
                        }
                        e += 1;
                    }
                    if e == i {
                        return self.syntax();
                    }
                    out.push(CmdArg::Word(self.lx.src[i..e].iter().collect()));
                    self.pos = e;
                }
            }
        }
    }

    /// Outside a capture a backtick opens one. Inside, it closes, unless it stands after a space right before a name:
    /// `listRelatives `ls -sl``.
    fn backtick_opens(&self, i: usize) -> bool {
        self.ticks == 0 || (i > 0 && self.lx.src[i - 1].is_whitespace() && self.lx.at(i + 1).is_some_and(|c| c.is_ascii_alphabetic()))
    }

    fn backtick(&mut self) -> P<Expr> {
        self.expect("`")?;
        self.ticks += 1;
        let r = self.nested(|p| {
            if let Token::Ident(w) = p.peek()? {
                if type_keyword(&w).is_none() {
                    return p.named_statement(&w);
                }
            }
            p.expr()
        });
        self.ticks -= 1;
        let e = r?;
        self.eat(";")?;
        self.expect("`")?;
        Ok(e)
    }

    // Expressions, lowest binding first.

    pub fn expr(&mut self) -> P<Expr> {
        self.nested(|p| {
            // An expression's own backticks nest freely: `size(`ls`)` inside a capture opens a new one.
            let saved = std::mem::take(&mut p.ticks);
            let r = p.assignment();
            p.ticks = saved;
            r
        })
    }

    fn assignment(&mut self) -> P<Expr> {
        let left = self.ternary()?;
        let op = match self.peek()? {
            Token::Punct("=") => None,
            Token::Punct("+=") => Some(BinOp::Add),
            Token::Punct("-=") => Some(BinOp::Sub),
            Token::Punct("*=") => Some(BinOp::Mul),
            Token::Punct("/=") => Some(BinOp::Div),
            Token::Punct("%=") => Some(BinOp::Mod),
            Token::Punct("^=") => Some(BinOp::Cross),
            _ => return Ok(left),
        };
        if !assignable(&left) {
            return self.syntax();
        }
        self.bump()?;
        let right = self.nested(|p| p.assignment())?;
        Ok(Expr::Assign(Box::new(left), op, Box::new(right)))
    }

    fn ternary(&mut self) -> P<Expr> {
        let c = self.or()?;
        if !self.eat("?")? {
            return Ok(c);
        }
        let a = self.nested(|p| p.assignment())?;
        self.expect(":")?;
        let b = self.nested(|p| p.assignment())?;
        Ok(Expr::Ternary(Box::new(c), Box::new(a), Box::new(b)))
    }

    fn or(&mut self) -> P<Expr> {
        let mut l = self.and()?;
        let mut n = 0;
        while self.eat("||")? {
            l = Expr::Or(Box::new(l), Box::new(self.and()?));
            self.chain(&mut n)?;
        }
        Ok(l)
    }

    fn and(&mut self) -> P<Expr> {
        let mut l = self.binary(0)?;
        let mut n = 0;
        while self.eat("&&")? {
            l = Expr::And(Box::new(l), Box::new(self.binary(0)?));
            self.chain(&mut n)?;
        }
        Ok(l)
    }

    fn chain(&mut self, n: &mut usize) -> P<()> {
        *n += 1;
        if *n > MAX_CHAIN {
            return Err("Expression too long".into());
        }
        Ok(())
    }

    fn binary(&mut self, level: usize) -> P<Expr> {
        const LEVELS: &[&[(&str, BinOp)]] = &[
            &[("==", BinOp::Eq), ("!=", BinOp::Ne)],
            &[("<", BinOp::Lt), ("<=", BinOp::Le), (">", BinOp::Gt), (">=", BinOp::Ge)],
            &[("+", BinOp::Add), ("-", BinOp::Sub)],
            &[("*", BinOp::Mul), ("/", BinOp::Div), ("%", BinOp::Mod), ("^", BinOp::Cross)],
        ];
        let Some(ops) = LEVELS.get(level) else { return self.unary() };
        let mut l = self.binary(level + 1)?;
        let mut n = 0;
        loop {
            let t = self.peek()?;
            let Token::Punct(p) = t else { return Ok(l) };
            let Some((_, op)) = ops.iter().find(|(s, _)| *s == p) else { return Ok(l) };
            self.bump()?;
            let r = self.binary(level + 1)?;
            l = Expr::Binary(*op, Box::new(l), Box::new(r));
            self.chain(&mut n)?;
        }
    }

    fn unary(&mut self) -> P<Expr> {
        match self.peek()? {
            Token::Punct("-") => {
                self.bump()?;
                let e = self.nested(|p| p.unary())?;
                Ok(match e {
                    Expr::Int(i) => Expr::Int(i.wrapping_neg()),
                    Expr::Float(f) => Expr::Float(-f),
                    e => Expr::Neg(Box::new(e)),
                })
            }
            Token::Punct("+") => {
                self.bump()?;
                self.nested(|p| p.unary())
            }
            Token::Punct("!") => {
                self.bump()?;
                Ok(Expr::Not(Box::new(self.nested(|p| p.unary())?)))
            }
            Token::Punct(t @ ("++" | "--")) => {
                self.bump()?;
                let e = self.nested(|p| p.unary())?;
                if !assignable(&e) {
                    return self.syntax();
                }
                Ok(Expr::Step(Box::new(e), if t == "++" { 1 } else { -1 }, true))
            }
            _ => {
                let p = self.primary()?;
                self.postfix(p)
            }
        }
    }

    fn postfix(&mut self, mut e: Expr) -> P<Expr> {
        let mut n = 0;
        loop {
            match self.peek()? {
                Token::Punct("[") => {
                    self.bump()?;
                    let i = self.expr()?;
                    self.expect("]")?;
                    e = Expr::Index(Box::new(e), Box::new(i));
                }
                Token::Punct(".") => {
                    self.bump()?;
                    e = self.member(e)?;
                }
                Token::Punct(t @ ("++" | "--")) => {
                    if !assignable(&e) {
                        return self.syntax();
                    }
                    self.bump()?;
                    e = Expr::Step(Box::new(e), if t == "++" { 1 } else { -1 }, false);
                }
                _ => return Ok(e),
            }
            self.chain(&mut n)?;
        }
    }

    /// In command syntax `[` and `.x` belong to a variable only when they touch it.
    fn postfix_adjacent(&mut self, mut e: Expr) -> P<Expr> {
        let mut n = 0;
        loop {
            match self.lx.at(self.pos) {
                Some('[') => {
                    self.bump()?;
                    let i = self.expr()?;
                    self.expect("]")?;
                    e = Expr::Index(Box::new(e), Box::new(i));
                }
                Some('.') if matches!(self.lx.at(self.pos + 1), Some('x' | 'y' | 'z')) => {
                    self.bump()?;
                    e = self.member(e)?;
                }
                _ => return Ok(e),
            }
            self.chain(&mut n)?;
        }
    }

    fn member(&mut self, e: Expr) -> P<Expr> {
        let Token::Ident(w) = self.bump()? else { return self.syntax() };
        let axis = match w.as_str() {
            "x" => 0,
            "y" => 1,
            "z" => 2,
            _ => return self.syntax(),
        };
        Ok(Expr::Member(Box::new(e), axis))
    }

    fn primary(&mut self) -> P<Expr> {
        match self.peek()? {
            Token::Int(i) => {
                self.bump()?;
                Ok(Expr::Int(i))
            }
            Token::Float(f) => {
                self.bump()?;
                Ok(Expr::Float(f))
            }
            Token::Str(s) => {
                self.bump()?;
                Ok(Expr::Str(s))
            }
            Token::Var(v) => {
                self.bump()?;
                Ok(Expr::Var(v))
            }
            Token::Punct("(") => {
                self.bump()?;
                // (int)$x
                if let Token::Ident(w) = self.peek()? {
                    if let Some(t) = type_keyword(&w) {
                        let save = self.pos;
                        self.bump()?;
                        let mut t = t;
                        if self.eat("[")? {
                            self.expect("]")?;
                            t = t.array_of();
                        }
                        if self.eat(")")? {
                            let e = self.nested(|p| p.unary())?;
                            return Ok(Expr::Cast(t, Box::new(e)));
                        }
                        self.pos = save;
                    }
                }
                let e = self.expr()?;
                self.expect(")")?;
                Ok(e)
            }
            Token::Punct("{") => {
                self.bump()?;
                let mut items = Vec::new();
                while !self.is("}")? {
                    items.push(self.expr()?);
                    if !self.eat(",")? {
                        break;
                    }
                }
                self.expect("}")?;
                Ok(Expr::Array(items))
            }
            Token::Punct("<<") => {
                self.bump()?;
                let mut rows = vec![Vec::new()];
                loop {
                    rows.last_mut().unwrap().push(self.expr()?);
                    if self.eat(",")? {
                        continue;
                    }
                    if self.eat(";")? {
                        // A closing semicolon is allowed: <<1, 2; 3, 4;>>
                        if self.is(">>")? {
                            break;
                        }
                        rows.push(Vec::new());
                        continue;
                    }
                    break;
                }
                self.expect(">>")?;
                if rows.len() == 1 && rows[0].len() <= 3 {
                    return Ok(Expr::Vector(rows.pop().unwrap()));
                }
                let w = rows[0].len();
                if rows.iter().any(|r| r.len() != w) {
                    return self.syntax();
                }
                Ok(Expr::Matrix(rows))
            }
            Token::Punct("`") => self.backtick(),
            Token::Ident(w) => {
                let constant = match w.as_str() {
                    "true" | "on" | "yes" => Some(1),
                    "false" | "off" | "no" => Some(0),
                    _ => None,
                };
                if let Some(c) = constant {
                    self.bump()?;
                    return Ok(Expr::Int(c));
                }
                if let Some(t) = type_keyword(&w) {
                    self.bump()?;
                    if !self.is("(")? {
                        return self.syntax();
                    }
                    self.bump()?;
                    let e = self.expr()?;
                    self.expect(")")?;
                    return Ok(Expr::Cast(t, Box::new(e)));
                }
                self.bump()?;
                if !self.is("(")? {
                    return self.syntax();
                }
                let args = self.call_args()?;
                Ok(Expr::Call(w, args))
            }
            _ => self.syntax(),
        }
    }
}

fn assignable(e: &Expr) -> bool {
    match e {
        Expr::Var(_) => true,
        Expr::Index(b, _) => assignable(b),
        _ => false,
    }
}
