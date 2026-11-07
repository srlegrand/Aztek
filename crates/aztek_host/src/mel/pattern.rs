//! The two pattern languages: regular expressions for match and substitute, globs for gmatch.

enum Node {
    Char(char),
    Any,
    Set(bool, Vec<(char, char)>),
    Start,
    End,
    Group(Vec<Vec<Node>>),
    Repeat(Box<Node>, usize, usize),
}

struct Compiler {
    p: Vec<char>,
    i: usize,
    depth: usize,
}

impl Compiler {
    fn alternatives(&mut self) -> Result<Vec<Vec<Node>>, String> {
        self.depth += 1;
        if self.depth > 100 {
            return Err("regular expression nested too deeply".into());
        }
        let mut alts = vec![Vec::new()];
        while let Some(&c) = self.p.get(self.i) {
            match c {
                ')' => break,
                '|' => {
                    self.i += 1;
                    alts.push(Vec::new());
                }
                _ => {
                    let atom = self.atom()?;
                    let atom = self.quantified(atom);
                    alts.last_mut().unwrap().push(atom);
                }
            }
        }
        self.depth -= 1;
        Ok(alts)
    }

    fn quantified(&mut self, atom: Node) -> Node {
        let range = match self.p.get(self.i) {
            Some('*') => (0, usize::MAX),
            Some('+') => (1, usize::MAX),
            Some('?') => (0, 1),
            _ => return atom,
        };
        self.i += 1;
        Node::Repeat(Box::new(atom), range.0, range.1)
    }

    fn atom(&mut self) -> Result<Node, String> {
        let c = self.p[self.i];
        self.i += 1;
        Ok(match c {
            '.' => Node::Any,
            '^' => Node::Start,
            '$' => Node::End,
            '(' => {
                let g = self.alternatives()?;
                if self.p.get(self.i) != Some(&')') {
                    return Err("unbalanced parenthesis in regular expression".into());
                }
                self.i += 1;
                Node::Group(g)
            }
            '[' => self.set()?,
            '\\' => {
                let Some(&n) = self.p.get(self.i) else { return Err("trailing backslash in regular expression".into()) };
                self.i += 1;
                match n {
                    'd' => Node::Set(false, vec![('0', '9')]),
                    'w' => Node::Set(false, vec![('a', 'z'), ('A', 'Z'), ('0', '9'), ('_', '_')]),
                    's' => Node::Set(false, vec![(' ', ' '), ('\t', '\t'), ('\n', '\n'), ('\r', '\r')]),
                    n => Node::Char(n),
                }
            }
            c => Node::Char(c),
        })
    }

    fn set(&mut self) -> Result<Node, String> {
        let neg = self.p.get(self.i) == Some(&'^');
        if neg {
            self.i += 1;
        }
        let mut ranges = Vec::new();
        let mut first = true;
        loop {
            let Some(&c) = self.p.get(self.i) else { return Err("unbalanced bracket in regular expression".into()) };
            self.i += 1;
            if c == ']' && !first {
                return Ok(Node::Set(neg, ranges));
            }
            first = false;
            let c = if c == '\\' {
                let Some(&n) = self.p.get(self.i) else { return Err("trailing backslash in regular expression".into()) };
                self.i += 1;
                n
            } else {
                c
            };
            if self.p.get(self.i) == Some(&'-') && self.p.get(self.i + 1).is_some_and(|n| *n != ']') {
                ranges.push((c, self.p[self.i + 1]));
                self.i += 2;
            } else {
                ranges.push((c, c));
            }
        }
    }
}

/// Steps a match may take before it gives up: a pathological pattern fails instead of hanging.
const STEP_LIMIT: usize = 2_000_000;

struct Matcher<'a> {
    s: &'a [char],
    steps: usize,
}

impl Matcher<'_> {
    fn one(&self, n: &Node, at: usize) -> Option<bool> {
        let c = self.s.get(at).copied();
        Some(match n {
            Node::Char(x) => c == Some(*x),
            Node::Any => c.is_some(),
            Node::Set(neg, r) => c.is_some_and(|c| r.iter().any(|(a, b)| *a <= c && c <= *b) != *neg),
            _ => return None,
        })
    }

    fn seq(&mut self, nodes: &[Node], at: usize, k: &mut dyn FnMut(&mut Self, usize) -> bool) -> bool {
        self.steps += 1;
        if self.steps > STEP_LIMIT {
            return false;
        }
        let Some((n, rest)) = nodes.split_first() else { return k(self, at) };
        match n {
            Node::Start => at == 0 && self.seq(rest, at, k),
            Node::End => at == self.s.len() && self.seq(rest, at, k),
            Node::Group(alts) => {
                for a in alts {
                    let ok = stacker::maybe_grow(64 * 1024, 1024 * 1024, || self.seq(a, at, &mut |m, e| m.seq(rest, e, k)));
                    if ok {
                        return true;
                    }
                }
                false
            }
            Node::Repeat(x, min, max) => {
                if self.one(x, at).is_some() {
                    // Single characters: count how far it reaches, then back off.
                    let mut e = at;
                    let mut count = 0;
                    while count < *max && self.one(x, e) == Some(true) {
                        e += 1;
                        count += 1;
                    }
                    loop {
                        if count < *min {
                            return false;
                        }
                        if self.seq(rest, at + count, k) {
                            return true;
                        }
                        if count == 0 {
                            return false;
                        }
                        count -= 1;
                    }
                }
                self.repeat(x, *min, *max, 0, at, rest, k)
            }
            n => self.one(n, at) == Some(true) && self.seq(rest, at + 1, k),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn repeat(&mut self, x: &Node, min: usize, max: usize, count: usize, at: usize, rest: &[Node], k: &mut dyn FnMut(&mut Self, usize) -> bool) -> bool {
        self.steps += 1;
        if self.steps > STEP_LIMIT {
            return false;
        }
        if count < max {
            let tried = stacker::maybe_grow(64 * 1024, 1024 * 1024, || {
                self.seq(std::slice::from_ref(x), at, &mut |m, e| e != at && m.repeat(x, min, max, count + 1, e, rest, k))
            });
            if tried {
                return true;
            }
        }
        count >= min && self.seq(rest, at, k)
    }
}

/// The first match of a regular expression: (start, end) in characters.
pub fn regex_find(pattern: &str, text: &str) -> Result<Option<(usize, usize)>, String> {
    let mut c = Compiler { p: pattern.chars().collect(), i: 0, depth: 0 };
    let alts = c.alternatives()?;
    if c.i != c.p.len() {
        return Err("unbalanced parenthesis in regular expression".into());
    }
    let prog = [Node::Group(alts)];
    let s: Vec<char> = text.chars().collect();
    let mut m = Matcher { s: &s, steps: 0 };
    for start in 0..=s.len() {
        let mut end = None;
        if m.seq(&prog, start, &mut |_, e| {
            end = Some(e);
            true
        }) {
            return Ok(end.map(|e| (start, e)));
        }
        if m.steps > STEP_LIMIT {
            break;
        }
    }
    Ok(None)
}

/// gmatch's glob: `*`, `?`, `[abc]`, `[a-z]`, `[!a]`, and backslash for a literal.
pub fn glob(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let s: Vec<char> = text.chars().collect();
    let (mut pi, mut si) = (0, 0);
    // The last star: where in the pattern, and how far it has taken the text.
    let mut star: Option<(usize, usize)> = None;
    loop {
        let step = if pi < p.len() {
            match p[pi] {
                '*' => {
                    star = Some((pi + 1, si));
                    pi += 1;
                    continue;
                }
                _ if si >= s.len() => None,
                '?' => Some(pi + 1),
                '[' => match glob_set(&p, pi, s[si]) {
                    Some((true, next)) => Some(next),
                    Some((false, _)) => None,
                    None if s[si] == '[' => Some(pi + 1),
                    None => None,
                },
                '\\' if pi + 1 < p.len() => (p[pi + 1] == s[si]).then_some(pi + 2),
                c => (c == s[si]).then_some(pi + 1),
            }
        } else {
            None
        };
        match step {
            Some(next) => {
                pi = next;
                si += 1;
            }
            None => {
                if pi >= p.len() && si >= s.len() {
                    return true;
                }
                match star {
                    Some((sp, ss)) if ss < s.len() => {
                        star = Some((sp, ss + 1));
                        pi = sp;
                        si = ss + 1;
                    }
                    _ => return false,
                }
            }
        }
    }
}

/// Whether the set at `p[start]` holds `c`, and the position after it. None when it isn't closed.
fn glob_set(p: &[char], start: usize, c: char) -> Option<(bool, usize)> {
    let mut i = start + 1;
    let neg = matches!(p.get(i), Some('!' | '^'));
    if neg {
        i += 1;
    }
    let mut hit = false;
    let mut first = true;
    loop {
        let &x = p.get(i)?;
        if x == ']' && !first {
            return Some((hit != neg, i + 1));
        }
        first = false;
        if p.get(i + 1) == Some(&'-') && p.get(i + 2).is_some_and(|y| *y != ']') {
            hit |= x <= c && c <= p[i + 2];
            i += 3;
        } else {
            hit |= x == c;
            i += 1;
        }
    }
}
