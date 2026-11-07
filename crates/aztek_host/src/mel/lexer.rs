//! MEL's tokens, read on demand: command syntax reads bare words from the same source, so the parser moves between
//! tokens and raw text at will.

#[derive(Clone, Debug, PartialEq)]
pub enum Token {
    Int(i32),
    Float(f64),
    Str(String),
    Var(String),
    Ident(String),
    Punct(&'static str),
    Eof,
}

/// Longest first.
const PUNCT: &[&str] = &[
    "<<", ">>", "==", "!=", "<=", ">=", "&&", "||", "++", "--", "+=", "-=", "*=", "/=", "%=", "^=", "+", "-", "*", "/", "%", "^", "<", ">", "=", "!", "?", ":", ";", ",", ".", "(",
    ")", "[", "]", "{", "}", "`",
];

pub struct Lexer {
    pub src: Vec<char>,
}

impl Lexer {
    pub fn new(src: &str) -> Lexer {
        Lexer { src: src.chars().collect() }
    }

    pub fn at(&self, i: usize) -> Option<char> {
        self.src.get(i).copied()
    }

    /// The position after whitespace and comments.
    pub fn skip_space(&self, mut i: usize) -> Result<usize, String> {
        loop {
            match self.at(i) {
                Some(c) if c.is_whitespace() => i += 1,
                Some('/') if self.at(i + 1) == Some('/') => {
                    while self.at(i).is_some_and(|c| c != '\n') {
                        i += 1;
                    }
                }
                Some('/') if self.at(i + 1) == Some('*') => {
                    i += 2;
                    loop {
                        match self.at(i) {
                            None => return Err("Unterminated comment".into()),
                            Some('*') if self.at(i + 1) == Some('/') => {
                                i += 2;
                                break;
                            }
                            _ => i += 1,
                        }
                    }
                }
                _ => return Ok(i),
            }
        }
    }

    pub fn line_of(&self, i: usize) -> usize {
        1 + self.src[..i.min(self.src.len())].iter().filter(|c| **c == '\n').count()
    }

    /// The token at or after `i`, and the position after it.
    pub fn token(&self, i: usize) -> Result<(Token, usize), String> {
        let i = self.skip_space(i)?;
        let Some(c) = self.at(i) else { return Ok((Token::Eof, i)) };
        if c == '"' {
            return self.string(i);
        }
        if c == '$' {
            let mut e = i + 1;
            while self.at(e).is_some_and(|c| c.is_ascii_alphanumeric() || c == '_') {
                e += 1;
            }
            if e == i + 1 {
                return Err("Syntax error".into());
            }
            return Ok((Token::Var(self.src[i + 1..e].iter().collect()), e));
        }
        if c.is_ascii_alphabetic() || c == '_' {
            let mut e = i;
            while self.at(e).is_some_and(|c| c.is_ascii_alphanumeric() || c == '_') {
                e += 1;
            }
            return Ok((Token::Ident(self.src[i..e].iter().collect()), e));
        }
        if c.is_ascii_digit() || (c == '.' && self.at(i + 1).is_some_and(|d| d.is_ascii_digit())) {
            return self.number(i);
        }
        for p in PUNCT {
            if p.chars().enumerate().all(|(k, pc)| self.at(i + k) == Some(pc)) {
                return Ok((Token::Punct(p), i + p.len()));
            }
        }
        Err("Syntax error".into())
    }

    fn number(&self, i: usize) -> Result<(Token, usize), String> {
        let mut e = i;
        if self.at(i) == Some('0') && matches!(self.at(i + 1), Some('x' | 'X')) {
            e = i + 2;
            while self.at(e).is_some_and(|c| c.is_ascii_hexdigit()) {
                e += 1;
            }
            let t: String = self.src[i + 2..e].iter().collect();
            return Ok((Token::Int(i64::from_str_radix(&t, 16).unwrap_or(0) as i32), e));
        }
        let mut float = false;
        while self.at(e).is_some_and(|c| c.is_ascii_digit()) {
            e += 1;
        }
        if self.at(e) == Some('.') {
            float = true;
            e += 1;
            while self.at(e).is_some_and(|c| c.is_ascii_digit()) {
                e += 1;
            }
        }
        if matches!(self.at(e), Some('e' | 'E')) {
            let mut x = e + 1;
            if matches!(self.at(x), Some('+' | '-')) {
                x += 1;
            }
            if self.at(x).is_some_and(|c| c.is_ascii_digit()) {
                while self.at(x).is_some_and(|c| c.is_ascii_digit()) {
                    x += 1;
                }
                e = x;
                float = true;
            }
        }
        let t: String = self.src[i..e].iter().collect();
        if float {
            Ok((Token::Float(t.parse().unwrap_or(0.0)), e))
        } else {
            Ok((Token::Int(t.parse::<i64>().unwrap_or(i64::MAX) as i32), e))
        }
    }

    fn string(&self, i: usize) -> Result<(Token, usize), String> {
        let mut s = String::new();
        let mut e = i + 1;
        loop {
            match self.at(e) {
                None => return Err("Unterminated string".into()),
                Some('"') => return Ok((Token::Str(s), e + 1)),
                Some('\\') => {
                    match self.at(e + 1) {
                        None => return Err("Unterminated string".into()),
                        Some('n') => s.push('\n'),
                        Some('t') => s.push('\t'),
                        Some('r') => s.push('\r'),
                        // A backslash at the end of a line continues the string on the next.
                        Some('\n') => {}
                        Some(c) => s.push(c),
                    }
                    e += 2;
                }
                Some(c) => {
                    s.push(c);
                    e += 1;
                }
            }
        }
    }
}
