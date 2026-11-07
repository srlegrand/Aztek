//! NURBS curves as the reference stores them (non-rational): degree, form, control vertices and a knot vector of
//! `cvs + degree - 1` values. Periodic curves repeat their first `degree` CVs at the end. Plus the operations the reference's curve
//! commands build on: evaluation, edit points, interpolation, rebuild, reverse, closing, knot insertion, splitting.

use glam::DVec3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Form {
    #[default]
    Open,
    Closed,
    Periodic,
}

impl Form {
    pub fn index(self) -> i64 {
        match self {
            Form::Open => 0,
            Form::Closed => 1,
            Form::Periodic => 2,
        }
    }
    pub fn from_index(i: i64) -> Form {
        match i {
            1 => Form::Closed,
            2 => Form::Periodic,
            _ => Form::Open,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct NurbsCurve {
    pub degree: usize,
    pub form: Form,
    pub cvs: Vec<DVec3>,
    /// `cvs.len() + degree - 1` values, as the reference writes them.
    pub knots: Vec<f64>,
}

/// The reference's default knots for an open curve: clamped ends, a unit apart.
pub fn open_knots(n_cvs: usize, degree: usize) -> Vec<f64> {
    let mut k = Vec::with_capacity(n_cvs + degree - 1);
    for _ in 0..degree {
        k.push(0.0);
    }
    let spans = n_cvs.saturating_sub(degree);
    for i in 1..spans {
        k.push(i as f64);
    }
    for _ in 0..degree {
        k.push(spans as f64);
    }
    k
}

/// Knots of a periodic curve of `spans` spans: uniform, running from `1 - degree`.
pub fn periodic_knots(spans: usize, degree: usize) -> Vec<f64> {
    let n = spans + degree + degree - 1;
    (0..n).map(|i| i as f64 + 1.0 - degree as f64).collect()
}

impl NurbsCurve {
    pub fn new(degree: usize, form: Form, cvs: Vec<DVec3>, knots: Vec<f64>) -> NurbsCurve {
        NurbsCurve { degree, form, cvs, knots }
    }

    /// An open curve through its control vertices with default knots. Too few CVs for the degree lowers the degree.
    pub fn open(degree: usize, cvs: Vec<DVec3>) -> NurbsCurve {
        let degree = degree.min(cvs.len().saturating_sub(1)).max(1);
        let knots = open_knots(cvs.len(), degree);
        NurbsCurve { degree, form: Form::Open, cvs, knots }
    }

    /// A periodic curve from one lap of CVs; the wrapped ones are added.
    pub fn periodic(degree: usize, lap: Vec<DVec3>) -> NurbsCurve {
        let degree = degree.max(1);
        let spans = lap.len();
        let mut cvs = lap;
        for i in 0..degree {
            cvs.push(cvs[i % spans]);
        }
        NurbsCurve { degree, form: Form::Periodic, cvs, knots: periodic_knots(spans, degree) }
    }

    pub fn is_valid(&self) -> bool {
        self.degree >= 1 && self.cvs.len() > self.degree && self.knots.len() == self.cvs.len() + self.degree - 1
    }

    pub fn spans(&self) -> usize {
        self.cvs.len().saturating_sub(self.degree)
    }

    /// (minValue, maxValue) of the parameter.
    pub fn domain(&self) -> (f64, f64) {
        if !self.is_valid() {
            return (0.0, 0.0);
        }
        (self.knots[self.degree - 1], self.knots[self.cvs.len() - 1])
    }

    /// One lap of CVs: the wrapped ones of a periodic curve left off.
    pub fn distinct_cvs(&self) -> &[DVec3] {
        if self.form == Form::Periodic {
            &self.cvs[..self.spans()]
        } else {
            &self.cvs
        }
    }

    fn ext(&self, i: usize) -> f64 {
        // The phantom knots at each end of the reference's knot vector don't affect evaluation in the domain.
        let m = self.knots.len();
        if i == 0 {
            self.knots[0]
        } else if i > m {
            self.knots[m - 1]
        } else {
            self.knots[i - 1]
        }
    }

    fn span_at(&self, u: f64) -> usize {
        // Largest k with ext(k) <= u < ext(k+1), within the domain.
        let n = self.cvs.len();
        let d = self.degree;
        let mut k = d;
        for i in d..n {
            if self.ext(i) <= u {
                k = i;
            }
            if self.ext(i + 1) > u && self.ext(i) <= u {
                k = i;
                break;
            }
        }
        // At the very end the last non-empty span applies.
        let mut k = k.min(n - 1);
        while k > d && self.ext(k + 1) <= self.ext(k) {
            k -= 1;
        }
        k
    }

    /// The point at a parameter (clamped to the domain), by de Boor.
    pub fn eval(&self, u: f64) -> DVec3 {
        if !self.is_valid() {
            return self.cvs.first().copied().unwrap_or(DVec3::ZERO);
        }
        let (lo, hi) = self.domain();
        let u = u.clamp(lo, hi);
        let d = self.degree;
        let k = self.span_at(u);
        let mut pts: Vec<DVec3> = (0..=d).map(|j| self.cvs[k - d + j]).collect();
        for r in 1..=d {
            for j in (r..=d).rev() {
                let i = k - d + j;
                let (a, b) = (self.ext(i), self.ext(i + d - r + 1));
                let den = b - a;
                let t = if den.abs() < 1e-12 { 0.0 } else { (u - a) / den };
                pts[j] = pts[j - 1].lerp(pts[j], t);
            }
        }
        pts[d]
    }

    /// The tangent (derivative with respect to the parameter).
    pub fn tangent(&self, u: f64) -> DVec3 {
        let (lo, hi) = self.domain();
        let h = (hi - lo).max(1e-6) * 1e-4;
        let (a, b) = ((u - h).max(lo), (u + h).min(hi));
        if b - a < 1e-12 {
            return DVec3::ZERO;
        }
        (self.eval(b) - self.eval(a)) / (b - a)
    }

    /// Distinct knots in the domain: the parameters of the edit points.
    pub fn distinct_params(&self) -> Vec<f64> {
        if !self.is_valid() {
            return Vec::new();
        }
        let (lo, hi) = self.domain();
        let mut v = Vec::new();
        for &k in &self.knots {
            if k >= lo - 1e-9 && k <= hi + 1e-9 && v.last().is_none_or(|l: &f64| (k - *l).abs() > 1e-9) {
                v.push(k);
            }
        }
        v
    }

    pub fn edit_points(&self) -> Vec<DVec3> {
        self.distinct_params().into_iter().map(|u| self.eval(u)).collect()
    }

    /// Points along the curve, `per_span` to a span and a straight run for degree 1.
    pub fn tessellate(&self, per_span: usize) -> Vec<DVec3> {
        if !self.is_valid() {
            return self.cvs.clone();
        }
        let params = self.distinct_params();
        let steps = if self.degree == 1 { 1 } else { per_span.max(1) };
        let mut out = Vec::with_capacity(params.len() * steps + 1);
        for w in params.windows(2) {
            for s in 0..steps {
                out.push(self.eval(w[0] + (w[1] - w[0]) * s as f64 / steps as f64));
            }
        }
        out.push(self.eval(*params.last().unwrap_or(&0.0)));
        out
    }

    /// Arc length.
    pub fn length(&self) -> f64 {
        let p = self.tessellate(32);
        p.windows(2).map(|w| (w[1] - w[0]).length()).sum()
    }

    /// The parameter a fraction of the way along the arc.
    pub fn param_at_fraction(&self, f: f64) -> f64 {
        let (lo, hi) = self.domain();
        let n = (self.spans() * 32).max(32);
        let mut acc = vec![0.0];
        let mut last = self.eval(lo);
        for i in 1..=n {
            let p = self.eval(lo + (hi - lo) * i as f64 / n as f64);
            acc.push(acc[i - 1] + (p - last).length());
            last = p;
        }
        let target = f.clamp(0.0, 1.0) * acc[n];
        let i = acc.partition_point(|&a| a < target).clamp(1, n);
        let seg = acc[i] - acc[i - 1];
        let t = if seg > 1e-12 { (target - acc[i - 1]) / seg } else { 0.0 };
        lo + (hi - lo) * (i as f64 - 1.0 + t) / n as f64
    }

    /// The parameter of the closest point on the curve.
    pub fn closest_param(&self, p: DVec3) -> f64 {
        let (lo, hi) = self.domain();
        let n = (self.spans() * 16).max(16);
        let mut best = (f64::MAX, lo);
        for i in 0..=n {
            let u = lo + (hi - lo) * i as f64 / n as f64;
            let d = (self.eval(u) - p).length_squared();
            if d < best.0 {
                best = (d, u);
            }
        }
        // Refine by golden-ish bisection around the sample.
        let step = (hi - lo) / n as f64;
        let (mut a, mut b) = ((best.1 - step).max(lo), (best.1 + step).min(hi));
        for _ in 0..40 {
            let (m1, m2) = (a + (b - a) / 3.0, b - (b - a) / 3.0);
            if (self.eval(m1) - p).length_squared() < (self.eval(m2) - p).length_squared() {
                b = m2;
            } else {
                a = m1;
            }
        }
        (a + b) * 0.5
    }

    pub fn transformed(&self, m: &glam::DMat4) -> NurbsCurve {
        let mut c = self.clone();
        for p in c.cvs.iter_mut() {
            *p = m.transform_point3(*p);
        }
        c
    }

    pub fn reversed(&self) -> NurbsCurve {
        let (lo, hi) = self.domain();
        let mut k: Vec<f64> = self.knots.iter().rev().map(|x| lo + hi - x).collect();
        if self.form == Form::Periodic {
            k = self.knots.clone();
        }
        let mut cvs: Vec<DVec3> = self.cvs.iter().rev().copied().collect();
        if self.form == Form::Periodic {
            let lap: Vec<DVec3> = self.distinct_cvs().iter().rev().copied().collect();
            cvs = lap.clone();
            for i in 0..self.degree {
                cvs.push(lap[i % lap.len()]);
            }
        }
        NurbsCurve { degree: self.degree, form: self.form, cvs, knots: k }
    }

    /// Boehm's knot insertion.
    pub fn insert_knot(&self, u: f64) -> NurbsCurve {
        if !self.is_valid() {
            return self.clone();
        }
        let d = self.degree;
        let k = self.span_at(u);
        let mut cvs = Vec::with_capacity(self.cvs.len() + 1);
        for i in 0..=self.cvs.len() {
            if i + d <= k {
                cvs.push(self.cvs[i]);
            } else if i > k {
                cvs.push(self.cvs[i - 1]);
            } else {
                let (a, b) = (self.ext(i), self.ext(i + d));
                let t = if (b - a).abs() < 1e-12 { 0.0 } else { (u - a) / (b - a) };
                cvs.push(self.cvs[i - 1].lerp(self.cvs[i], t));
            }
        }
        let mut knots = self.knots.clone();
        let at = knots.partition_point(|&x| x <= u);
        knots.insert(at, u);
        NurbsCurve { degree: d, form: self.form, cvs, knots }
    }

    /// Two open curves meeting at u.
    pub fn split(&self, u: f64) -> (NurbsCurve, NurbsCurve) {
        let (lo, hi) = self.domain();
        let u = u.clamp(lo, hi);
        // Sampling-free when the curve allows: raise u's multiplicity to the degree.
        let mut c = self.clone();
        let mult = c.knots.iter().filter(|x| (**x - u).abs() < 1e-9).count();
        for _ in mult..self.degree {
            c = c.insert_knot(u);
        }
        let d = self.degree;
        // ext index of the first u; the CV there is on the curve.
        let first = c.knots.iter().position(|x| (*x - u).abs() < 1e-9).unwrap_or(0);
        let split_cv = first + 1 - 1;
        let left_cvs: Vec<DVec3> = c.cvs[..=split_cv.min(c.cvs.len() - 1)].to_vec();
        let right_cvs: Vec<DVec3> = c.cvs[split_cv.min(c.cvs.len() - 1)..].to_vec();
        let mut left_knots: Vec<f64> = c.knots[..first + d].to_vec();
        left_knots.truncate(left_cvs.len() + d - 1);
        let mut right_knots: Vec<f64> = c.knots[first..].to_vec();
        right_knots.truncate(right_cvs.len() + d - 1);
        let fix = |cvs: Vec<DVec3>, knots: Vec<f64>| {
            if cvs.len() > d && knots.len() == cvs.len() + d - 1 {
                NurbsCurve { degree: d, form: Form::Open, cvs, knots }
            } else {
                NurbsCurve::open(d, cvs)
            }
        };
        (fix(left_cvs, left_knots), fix(right_cvs, right_knots))
    }

    /// The curve closed: periodic, its CVs one lap (the reference's closeCurve with preserve shape off).
    pub fn closed(&self) -> NurbsCurve {
        if self.form == Form::Periodic {
            return self.clone();
        }
        NurbsCurve::periodic(self.degree, self.cvs.clone())
    }

    /// The curve opened: clamped ends with the same CVs, one lap.
    pub fn opened(&self) -> NurbsCurve {
        if self.form != Form::Periodic {
            return self.clone();
        }
        NurbsCurve::open(self.degree, self.distinct_cvs().to_vec())
    }

    /// An open curve interpolating points, chord-length parameterised (global interpolation).
    pub fn through(degree: usize, pts: &[DVec3]) -> NurbsCurve {
        let n = pts.len();
        if n < 2 {
            return NurbsCurve::open(1, pts.to_vec());
        }
        let degree = degree.min(n - 1).max(1);
        if degree == 1 {
            return NurbsCurve::open(1, pts.to_vec());
        }
        let total: f64 = pts.windows(2).map(|w| (w[1] - w[0]).length()).sum::<f64>().max(1e-9);
        let mut u = vec![0.0; n];
        for i in 1..n {
            u[i] = u[i - 1] + (pts[i] - pts[i - 1]).length() / total;
        }
        u[n - 1] = 1.0;
        // Averaged knots: the full vector has n + degree + 1 values.
        let mut full = vec![0.0; n + degree + 1];
        for k in full.iter_mut().skip(n) {
            *k = 1.0;
        }
        for j in 1..n - degree {
            full[j + degree] = u[j..j + degree].iter().sum::<f64>() / degree as f64;
        }
        // Basis matrix, solved directly: curves here have few points.
        let mut a = vec![vec![0.0; n]; n];
        for (r, &ur) in u.iter().enumerate() {
            let b = basis_row(&full, degree, n, ur);
            a[r] = b;
        }
        let cvs = solve(&a, pts);
        let knots = full[1..full.len() - 1].to_vec();
        // Knots scaled to whole spans over the distinct range, as the reference's EP tool shows.
        let scale = (n - degree) as f64;
        NurbsCurve { degree, form: Form::Open, cvs, knots: knots.iter().map(|x| x * scale).collect() }
    }

    /// Rebuilt to uniform spans and a degree: points sampled by arc length, then interpolated.
    pub fn rebuilt(&self, spans: usize, degree: usize) -> NurbsCurve {
        let spans = spans.max(1);
        let degree = degree.max(1);
        if self.form == Form::Periodic {
            let lap: Vec<DVec3> = (0..spans).map(|i| self.eval(self.param_at_fraction(i as f64 / spans as f64))).collect();
            return NurbsCurve::periodic(degree, lap);
        }
        let n = spans + degree;
        let pts: Vec<DVec3> = (0..n).map(|i| self.eval(self.param_at_fraction(i as f64 / (n - 1) as f64))).collect();
        let c = NurbsCurve::through(degree, &pts);
        // Uniform knots with the fitted CVs' count.
        NurbsCurve::new(c.degree, Form::Open, c.cvs.clone(), open_knots(c.cvs.len(), c.degree))
    }

    /// Laplacian smoothing of the CVs; the ends stay on open curves.
    pub fn smoothed(&self, amount: f64, iterations: usize) -> NurbsCurve {
        let mut c = self.clone();
        let lap = c.distinct_cvs().len();
        let per = c.form == Form::Periodic;
        for _ in 0..iterations {
            let old = c.cvs.clone();
            for i in 0..lap {
                if !per && (i == 0 || i == lap - 1) {
                    continue;
                }
                let (a, b) = (old[(i + lap - 1) % lap], old[(i + 1) % lap]);
                c.cvs[i] = old[i].lerp((a + b) * 0.5, amount);
            }
            if per {
                for i in 0..c.degree {
                    c.cvs[lap + i] = c.cvs[i % lap];
                }
            }
        }
        c
    }

    /// This curve followed by another, joined with a straight connection between its end and the other's start.
    pub fn attached(&self, other: &NurbsCurve) -> NurbsCurve {
        let d = self.degree.max(other.degree);
        let mut pts = self.tessellate(8);
        let more = other.tessellate(8);
        if pts.last().zip(more.first()).is_some_and(|(a, b)| (*a - *b).length() < 1e-9) {
            pts.extend(more.into_iter().skip(1));
        } else {
            pts.extend(more);
        }
        let want = self.spans() + other.spans() + 1;
        NurbsCurve::open(d, pts).rebuilt(want, d)
    }

    /// Offset within the plane of the curve (or `normal`), by sampling and interpolating.
    pub fn offset(&self, distance: f64, normal: Option<DVec3>) -> NurbsCurve {
        let (lo, hi) = self.domain();
        let params: Vec<f64> = self.distinct_params();
        let n = normal.unwrap_or_else(|| self.plane_normal());
        let samples = (self.spans() * 4).max(8);
        let at = |u: f64| {
            let t = self.tangent(u).normalize_or_zero();
            self.eval(u) + t.cross(n).normalize_or_zero() * distance
        };
        let _ = params;
        let pts: Vec<DVec3> = (0..=samples).map(|i| at(lo + (hi - lo) * i as f64 / samples as f64)).collect();
        let c = NurbsCurve::through(self.degree, &pts);
        c.rebuilt(self.spans().max(1), self.degree)
    }

    /// The best plane's normal through the curve's CVs.
    pub fn plane_normal(&self) -> DVec3 {
        let c: DVec3 = self.cvs.iter().copied().sum::<DVec3>() / self.cvs.len().max(1) as f64;
        let mut n = DVec3::ZERO;
        for w in self.cvs.windows(2) {
            n += (w[0] - c).cross(w[1] - c);
        }
        if n.length() < 1e-9 {
            DVec3::Y
        } else {
            n.normalize()
        }
    }

    /// Lengthened straight along the end tangents by distance.
    pub fn extended(&self, distance: f64, at_start: bool, at_end: bool) -> NurbsCurve {
        let (lo, hi) = self.domain();
        let mut pts = self.tessellate(8);
        if at_start {
            let t = self.tangent(lo).normalize_or_zero();
            pts.insert(0, pts[0] - t * distance);
        }
        if at_end {
            let t = self.tangent(hi).normalize_or_zero();
            let last = *pts.last().unwrap();
            pts.push(last + t * distance);
        }
        NurbsCurve::through(self.degree, &pts).rebuilt(self.spans() + 1, self.degree)
    }
}

fn basis_row(full: &[f64], degree: usize, n: usize, u: f64) -> Vec<f64> {
    // Cox-de Boor over the full clamped knot vector.
    let mut b = vec![0.0; full.len() - 1];
    for i in 0..full.len() - 1 {
        if u >= full[i] && u < full[i + 1] {
            b[i] = 1.0;
        }
    }
    if u >= 1.0 {
        // The last basis function takes the end.
        for x in b.iter_mut() {
            *x = 0.0;
        }
        b[n - 1] = 1.0;
    }
    for d in 1..=degree {
        for i in 0..full.len() - 1 - d {
            let l = if (full[i + d] - full[i]).abs() < 1e-12 { 0.0 } else { (u - full[i]) / (full[i + d] - full[i]) * b[i] };
            let r = if (full[i + d + 1] - full[i + 1]).abs() < 1e-12 { 0.0 } else { (full[i + d + 1] - u) / (full[i + d + 1] - full[i + 1]) * b[i + 1] };
            b[i] = l + r;
        }
    }
    b.truncate(n);
    b.resize(n, 0.0);
    b
}

fn solve(a: &[Vec<f64>], rhs: &[DVec3]) -> Vec<DVec3> {
    let n = rhs.len();
    let mut m: Vec<Vec<f64>> = a.to_vec();
    let mut x: Vec<DVec3> = rhs.to_vec();
    for c in 0..n {
        let mut p = c;
        for r in c + 1..n {
            if m[r][c].abs() > m[p][c].abs() {
                p = r;
            }
        }
        m.swap(c, p);
        x.swap(c, p);
        let d = m[c][c];
        if d.abs() < 1e-14 {
            continue;
        }
        for r in c + 1..n {
            let f = m[r][c] / d;
            if f == 0.0 {
                continue;
            }
            for k in c..n {
                let v = m[c][k];
                m[r][k] -= f * v;
            }
            let v = x[c];
            x[r] -= v * f;
        }
    }
    for c in (0..n).rev() {
        let mut v = x[c];
        for k in c + 1..n {
            v -= x[k] * m[c][k];
        }
        x[c] = if m[c][c].abs() < 1e-14 { v } else { v / m[c][c] };
    }
    x
}

/// The reference's `circle`: a periodic curve whose points hit the radius at the section ends. Sections are counted from the first
/// point axis around the normal; with sweep below 360 the curve is an open arc.
pub fn circle(center: DVec3, normal: DVec3, first: DVec3, radius: f64, degree: usize, sections: usize, sweep_deg: f64) -> NurbsCurve {
    let n = normal.normalize_or_zero();
    let n = if n == DVec3::ZERO { DVec3::Z } else { n };
    let mut x = first - n * first.dot(n);
    if x.length() < 1e-9 {
        x = n.any_orthonormal_vector();
    }
    let x = x.normalize();
    let y = n.cross(x);
    let sections = sections.max(3);
    let step = sweep_deg.to_radians() / sections as f64;
    if (sweep_deg - 360.0).abs() < 1e-6 {
        // For a uniform B-spline a point sits at (P- + 4P + P+)/6, so the CVs lie a little outside the radius.
        let r = if degree >= 3 { radius * 6.0 / (4.0 + 2.0 * step.cos()) } else if degree == 2 { radius * 2.0 / (1.0 + step.cos()) } else { radius };
        let lap: Vec<DVec3> = (0..sections).map(|i| center + (x * (step * (i + 1) as f64).cos() + y * (step * (i + 1) as f64).sin()) * r).collect();
        return NurbsCurve::periodic(degree, lap);
    }
    let pts: Vec<DVec3> = (0..=sections).map(|i| center + (x * (step * i as f64).cos() + y * (step * i as f64).sin()) * radius).collect();
    NurbsCurve::through(degree, &pts).rebuilt(sections, degree)
}

// ---------------------------------------------------------------------------------------------------------------------
// Curve helpers the surfaces build on

fn knot_ext(k: &[f64], i: usize) -> f64 {
    let m = k.len();
    if i == 0 {
        k[0]
    } else if i > m {
        k[m - 1]
    } else {
        k[i - 1]
    }
}

fn find_span(k: &[f64], n: usize, d: usize, u: f64) -> usize {
    let mut s = d;
    for i in d..n {
        if knot_ext(k, i) <= u {
            s = i;
        } else {
            break;
        }
    }
    while s > d && knot_ext(k, s + 1) <= knot_ext(k, s) {
        s -= 1;
    }
    s
}

/// The p + 1 basis functions non-zero in a span: for the CVs span - p to span.
fn basis_funs(k: &[f64], span: usize, u: f64, p: usize) -> Vec<f64> {
    let mut n = vec![0.0; p + 1];
    n[0] = 1.0;
    let (mut left, mut right) = (vec![0.0; p + 1], vec![0.0; p + 1]);
    for j in 1..=p {
        left[j] = u - knot_ext(k, span + 1 - j);
        right[j] = knot_ext(k, span + j) - u;
        let mut saved = 0.0;
        for r in 0..j {
            let den = right[r + 1] + left[j - r];
            let t = if den.abs() < 1e-14 { 0.0 } else { n[r] / den };
            n[r] = saved + right[r + 1] * t;
            saved = left[j - r] * t;
        }
        n[j] = saved;
    }
    n
}

/// Basis functions and their first derivatives.
fn basis_ders(k: &[f64], span: usize, u: f64, p: usize) -> (Vec<f64>, Vec<f64>) {
    let n = basis_funs(k, span, u, p);
    let mut d = vec![0.0; p + 1];
    if p == 0 {
        return (n, d);
    }
    let low = basis_funs(k, span, u, p - 1);
    for j in 0..=p {
        let i = span - p + j;
        let mut v = 0.0;
        if j > 0 {
            let den = knot_ext(k, i + p) - knot_ext(k, i);
            if den.abs() > 1e-14 {
                v += low[j - 1] * p as f64 / den;
            }
        }
        if j < p {
            let den = knot_ext(k, i + p + 1) - knot_ext(k, i + 1);
            if den.abs() > 1e-14 {
                v -= low[j] * p as f64 / den;
            }
        }
        d[j] = v;
    }
    (n, d)
}

fn wrap_param(form: Form, u: f64, lo: f64, hi: f64) -> f64 {
    if form == Form::Periodic && hi > lo {
        if u >= lo && u <= hi {
            u
        } else {
            lo + (u - lo).rem_euclid(hi - lo)
        }
    } else {
        u.clamp(lo, hi)
    }
}

/// Where each CV has most say (Greville abscissae): one per CV of a lap.
fn cv_params(k: &[f64], d: usize, count: usize) -> Vec<f64> {
    (0..count).map(|i| k[i..i + d].iter().sum::<f64>() / d as f64).collect()
}

/// The distinct knots of a domain, with `per` steps to a span.
fn span_params(k: &[f64], d: usize, n: usize, per: usize) -> Vec<f64> {
    let (lo, hi) = (k[d - 1], k[n - 1]);
    let mut knots: Vec<f64> = Vec::new();
    for &x in k {
        if x >= lo - 1e-9 && x <= hi + 1e-9 && knots.last().is_none_or(|l| (x - *l).abs() > 1e-9) {
            knots.push(x);
        }
    }
    let per = if d == 1 { 1 } else { per.max(1) };
    let mut out = Vec::with_capacity((knots.len().max(1) - 1) * per + 1);
    for w in knots.windows(2) {
        for s in 0..per {
            out.push(w[0] + (w[1] - w[0]) * s as f64 / per as f64);
        }
    }
    out.extend(knots.last());
    out
}

impl NurbsCurve {
    /// Knot insertion for any form: a periodic curve stays periodic with one more span.
    pub fn insert_knot_any(&self, u: f64) -> NurbsCurve {
        if self.form != Form::Periodic {
            return self.insert_knot(u);
        }
        let (d, lap) = (self.degree, self.spans());
        let (lo, hi) = self.domain();
        let u = wrap_param(Form::Periodic, u, lo, hi);
        if lap < d || u <= lo || u >= hi {
            return self.clone();
        }
        let period = hi - lo;
        let domain = &self.knots[d - 1..d + lap];
        let t = |i: i64| -> f64 {
            let rel = i - d as i64;
            domain[rel.rem_euclid(lap as i64) as usize] + period * rel.div_euclid(lap as i64) as f64
        };
        let p = |i: i64| self.cvs[i.rem_euclid(lap as i64) as usize];
        let k = find_span(&self.knots, self.cvs.len(), d, u) as i64;
        let new_lap = lap + 1;
        let mut cvs = vec![DVec3::ZERO; new_lap];
        let di = d as i64;
        for i in k - di + 1..=k {
            let (a, b) = (t(i), t(i + di));
            let f = if (b - a).abs() < 1e-12 { 0.0 } else { (u - a) / (b - a) };
            cvs[i.rem_euclid(new_lap as i64) as usize] = p(i - 1).lerp(p(i), f);
        }
        for i in k..k + (lap as i64) - di + 1 {
            cvs[(i + 1).rem_euclid(new_lap as i64) as usize] = p(i);
        }
        let mut new_domain: Vec<f64> = domain[..lap].to_vec();
        new_domain.insert(new_domain.partition_point(|&x| x <= u), u);
        let t2 = |i: i64| -> f64 {
            let rel = i - d as i64;
            new_domain[rel.rem_euclid(new_lap as i64) as usize] + period * rel.div_euclid(new_lap as i64) as f64
        };
        let total = new_lap + d;
        let knots: Vec<f64> = (1..=(total + d - 1) as i64).map(t2).collect();
        for i in 0..d {
            let v = cvs[i % new_lap];
            cvs.push(v);
        }
        NurbsCurve { degree: d, form: Form::Periodic, cvs, knots }
    }

    /// The same shape with clamped ends: a periodic curve becomes Closed, with its seam at the start of the domain.
    pub fn clamped(&self) -> NurbsCurve {
        if self.form != Form::Periodic || !self.is_valid() {
            return self.clone();
        }
        let (lo, hi) = self.domain();
        let open = NurbsCurve { form: Form::Open, ..self.clone() };
        let (_, right) = open.split(lo);
        let (mut c, _) = right.split(hi);
        c.form = Form::Closed;
        c
    }

    /// The knots moved linearly to a new domain.
    pub fn with_domain(&self, a: f64, b: f64) -> NurbsCurve {
        let (lo, hi) = self.domain();
        let scale = if hi > lo { (b - a) / (hi - lo) } else { 1.0 };
        NurbsCurve { knots: self.knots.iter().map(|k| a + (k - lo) * scale).collect(), ..self.clone() }
    }

    /// A straight line of even spans.
    pub fn line(a: DVec3, b: DVec3, spans: usize, degree: usize) -> NurbsCurve {
        let (spans, degree) = (spans.max(1), degree.max(1));
        let knots = open_knots(spans + degree, degree);
        let cvs = cv_params(&knots, degree, spans + degree).into_iter().map(|t| a.lerp(b, t / spans as f64)).collect();
        NurbsCurve { degree, form: Form::Open, cvs, knots }
    }

    /// A clamped curve through points at the parameters `ts`, with first derivatives given at the ends: cubic takes
    /// both, quadratic the start's, linear neither.
    pub fn interpolated(degree: usize, pts: &[DVec3], ts: &[f64], d0: DVec3, d1: DVec3) -> NurbsCurve {
        let n = pts.len();
        let degree = degree.clamp(1, 3);
        if degree == 1 || n < 2 {
            return NurbsCurve { degree: 1, form: Form::Open, cvs: pts.to_vec(), knots: ts.to_vec() };
        }
        let count = n + degree - 1;
        let mut knots = Vec::with_capacity(count + degree - 1);
        for _ in 0..degree {
            knots.push(ts[0]);
        }
        knots.extend_from_slice(&ts[1..n - 1]);
        for _ in 0..degree {
            knots.push(ts[n - 1]);
        }
        let row = |t: f64, der: bool| {
            let s = find_span(&knots, count, degree, t);
            let (b, db) = basis_ders(&knots, s, t, degree);
            let mut r = vec![0.0; count];
            for (j, v) in (if der { db } else { b }).iter().enumerate() {
                r[s - degree + j] = *v;
            }
            r
        };
        let mut a = Vec::with_capacity(count);
        let mut rhs = Vec::with_capacity(count);
        a.push(row(ts[0], false));
        rhs.push(pts[0]);
        a.push(row(ts[0], true));
        rhs.push(d0);
        for i in 1..n - 1 {
            a.push(row(ts[i], false));
            rhs.push(pts[i]);
        }
        if degree == 3 {
            a.push(row(ts[n - 1], true));
            rhs.push(d1);
        }
        a.push(row(ts[n - 1], false));
        rhs.push(pts[n - 1]);
        let cvs = solve(&a, &rhs);
        NurbsCurve { degree, form: Form::Open, cvs, knots }
    }

    /// A periodic curve through a lap of points, one to a span.
    pub fn interpolated_periodic(degree: usize, pts: &[DVec3]) -> NurbsCurve {
        let n = pts.len();
        let degree = degree.clamp(1, 3);
        if degree == 1 || n < 3 {
            return NurbsCurve::periodic(1, pts.to_vec());
        }
        let knots = periodic_knots(n, degree);
        let shift = if degree % 2 == 0 { 0.5 } else { 0.0 };
        let total = n + degree;
        let mut a = vec![vec![0.0; n]; n];
        for (r, row) in a.iter_mut().enumerate() {
            let t = r as f64 + shift;
            let s = find_span(&knots, total, degree, t);
            for (j, v) in basis_funs(&knots, s, t, degree).iter().enumerate() {
                row[(s - degree + j) % n] += *v;
            }
        }
        NurbsCurve::periodic(degree, solve(&a, pts))
    }
}

/// An arc of the unit circle in the XY plane, from start to end degrees in `sections` spans: periodic for a full turn,
/// otherwise an interpolating curve whose end derivatives are a span's chord long.
pub fn unit_arc(start_deg: f64, end_deg: f64, sections: usize, degree: usize) -> NurbsCurve {
    let sections = sections.max(1);
    let step = (end_deg - start_deg).to_radians() / sections as f64;
    let at = |a: f64| DVec3::new(a.cos(), a.sin(), 0.0);
    let first = start_deg.to_radians();
    if ((end_deg - start_deg).abs() - 360.0).abs() < 1e-6 && sections >= 3 {
        let degree = degree.clamp(1, 3);
        let scale = match degree {
            3 => 6.0 / (4.0 + 2.0 * step.cos()),
            2 => 2.0 / (1.0 + step.cos()),
            _ => 1.0,
        };
        let offset = (1.0 - degree as f64) * 0.5;
        let lap: Vec<DVec3> = (0..sections).map(|j| at(first + step * (j as f64 + offset)) * scale).collect();
        return NurbsCurve::periodic(degree, lap);
    }
    let pts: Vec<DVec3> = (0..=sections).map(|i| at(first + step * i as f64)).collect();
    let ts: Vec<f64> = (0..=sections).map(|i| i as f64).collect();
    let chord = 2.0 * (step * 0.5).sin();
    let tangent = |a: f64| DVec3::new(-a.sin(), a.cos(), 0.0) * chord;
    NurbsCurve::interpolated(degree, &pts, &ts, tangent(first), tangent(first + step * sections as f64))
}

/// Curves brought to the same degree and knots, so their CVs line up.
pub fn compatible_curves(curves: &[NurbsCurve]) -> Vec<NurbsCurve> {
    let Some(first) = curves.first() else { return Vec::new() };
    if curves.iter().all(|c| c.degree == first.degree && c.form == first.form && c.cvs.len() == first.cvs.len() && c.knots == first.knots) {
        return curves.to_vec();
    }
    let degree = curves.iter().map(|c| c.degree).max().unwrap_or(1);
    let (lo, hi) = (0.0, 1.0);
    let mut lifted: Vec<NurbsCurve> = curves
        .iter()
        .map(|c| {
            let c = if c.degree == degree { c.clamped() } else { c.clamped().rebuilt(c.spans().max(1), degree) };
            let c = c.clamped();
            c.with_domain(lo, hi)
        })
        .collect();
    let mut union: Vec<f64> = Vec::new();
    for c in &lifted {
        let mut left = union.clone();
        for &k in &c.knots {
            if k <= lo + 1e-9 || k >= hi - 1e-9 {
                continue;
            }
            match left.iter().position(|x| (x - k).abs() < 1e-9) {
                Some(i) => {
                    left.remove(i);
                }
                None => union.push(k),
            }
        }
    }
    union.sort_by(f64::total_cmp);
    for c in lifted.iter_mut() {
        let mut have: Vec<f64> = c.knots.iter().copied().filter(|&k| k > lo + 1e-9 && k < hi - 1e-9).collect();
        for &k in &union {
            match have.iter().position(|x| (x - k).abs() < 1e-9) {
                Some(i) => {
                    have.remove(i);
                }
                None => *c = c.insert_knot(k),
            }
        }
    }
    lifted
}

// ---------------------------------------------------------------------------------------------------------------------
// Surfaces

/// A non-rational NURBS surface as the reference stores one: CVs row-major, `cvs[u * nv + v]`, wrapped CVs of a periodic direction
/// included, and `count + degree - 1` knots each way.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NurbsSurface {
    pub degree_u: usize,
    pub degree_v: usize,
    pub form_u: Form,
    pub form_v: Form,
    pub nu: usize,
    pub nv: usize,
    pub cvs: Vec<DVec3>,
    pub knots_u: Vec<f64>,
    pub knots_v: Vec<f64>,
    /// Trims, in parameter space; a point shows when every one keeps it.
    pub trim: Vec<TrimRegion>,
}

/// One trim: closed loops in (u, v). A point counts as inside by how many it's in, odd or even, so a loop within a loop is
/// a hole. keep_odd keeps what's inside; otherwise what's outside.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TrimRegion {
    pub loops: Vec<Vec<glam::DVec2>>,
    pub keep_odd: bool,
}

impl TrimRegion {
    pub fn keeps(&self, p: glam::DVec2) -> bool {
        let mut odd = false;
        for l in &self.loops {
            let n = l.len();
            for i in 0..n {
                let (a, b) = (l[i], l[(i + 1) % n]);
                if (a.y > p.y) != (b.y > p.y) && p.x < a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x) {
                    odd = !odd;
                }
            }
        }
        odd == self.keep_odd
    }
}

/// How a surface becomes a mesh.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MeshOptions {
    pub quads: bool,
    /// Corner normals from the surface, for smooth shading.
    pub normals: bool,
}

/// A direction of a rebuild: on or off, span count (0 keeps it), degree and form.
#[derive(Clone, Copy, Debug)]
pub struct RebuildDir {
    pub on: bool,
    pub spans: usize,
    pub degree: usize,
    pub form: Form,
}

impl NurbsSurface {
    /// From columns: one curve along U per CV in V, all of one degree and knots.
    pub fn from_columns(columns: &[NurbsCurve], degree_v: usize, form_v: Form, knots_v: Vec<f64>) -> NurbsSurface {
        let Some(first) = columns.first() else { return NurbsSurface::default() };
        let (nu, nv) = (first.cvs.len(), columns.len());
        let mut cvs = Vec::with_capacity(nu * nv);
        for u in 0..nu {
            for c in columns {
                cvs.push(c.cvs[u]);
            }
        }
        NurbsSurface { degree_u: first.degree, degree_v, form_u: first.form, form_v, nu, nv, cvs, knots_u: first.knots.clone(), knots_v, trim: Vec::new() }
    }

    /// The tensor product of a curve along U and a curve along V, with CVs from a function of their indices.
    pub fn from_curves(cu: &NurbsCurve, cv: &NurbsCurve, f: impl Fn(usize, usize) -> DVec3) -> NurbsSurface {
        let (nu, nv) = (cu.cvs.len(), cv.cvs.len());
        let mut cvs = Vec::with_capacity(nu * nv);
        for u in 0..nu {
            for v in 0..nv {
                cvs.push(f(u, v));
            }
        }
        let mut s = NurbsSurface { degree_u: cu.degree, degree_v: cv.degree, form_u: cu.form, form_v: cv.form, nu, nv, cvs, knots_u: cu.knots.clone(), knots_v: cv.knots.clone(), trim: Vec::new() };
        s.rewrap();
        s
    }

    pub fn is_valid(&self) -> bool {
        self.degree_u >= 1
            && self.degree_v >= 1
            && self.nu > self.degree_u
            && self.nv > self.degree_v
            && self.cvs.len() == self.nu * self.nv
            && self.knots_u.len() == self.nu + self.degree_u - 1
            && self.knots_v.len() == self.nv + self.degree_v - 1
    }

    pub fn spans_u(&self) -> usize {
        self.nu.saturating_sub(self.degree_u)
    }
    pub fn spans_v(&self) -> usize {
        self.nv.saturating_sub(self.degree_v)
    }

    /// CVs of one lap in U: the count of `cv[u][]`.
    pub fn distinct_u(&self) -> usize {
        if self.form_u == Form::Periodic {
            self.spans_u()
        } else {
            self.nu
        }
    }
    /// CVs of one lap in V: the count of `cv[][v]`.
    pub fn distinct_v(&self) -> usize {
        if self.form_v == Form::Periodic {
            self.spans_v()
        } else {
            self.nv
        }
    }

    pub fn domain_u(&self) -> (f64, f64) {
        if !self.is_valid() {
            return (0.0, 0.0);
        }
        (self.knots_u[self.degree_u - 1], self.knots_u[self.nu - 1])
    }
    pub fn domain_v(&self) -> (f64, f64) {
        if !self.is_valid() {
            return (0.0, 0.0);
        }
        (self.knots_v[self.degree_v - 1], self.knots_v[self.nv - 1])
    }

    pub fn cv(&self, u: usize, v: usize) -> DVec3 {
        self.cvs[u * self.nv + v]
    }

    /// Wrapped CVs of a periodic direction follow their first lap again.
    pub fn rewrap(&mut self) {
        if self.form_u == Form::Periodic {
            let lap = self.spans_u();
            for i in lap..self.nu {
                for v in 0..self.nv {
                    self.cvs[i * self.nv + v] = self.cvs[(i % lap) * self.nv + v];
                }
            }
        }
        if self.form_v == Form::Periodic {
            let lap = self.spans_v();
            for u in 0..self.nu {
                for i in lap..self.nv {
                    self.cvs[u * self.nv + i] = self.cvs[u * self.nv + i % lap];
                }
            }
        }
    }

    pub fn transformed(&self, m: &glam::DMat4) -> NurbsSurface {
        let mut s = self.clone();
        for p in s.cvs.iter_mut() {
            *p = m.transform_point3(*p);
        }
        s
    }

    /// U and V exchanged.
    pub fn swapped(&self) -> NurbsSurface {
        let mut cvs = Vec::with_capacity(self.cvs.len());
        for v in 0..self.nv {
            for u in 0..self.nu {
                cvs.push(self.cv(u, v));
            }
        }
        NurbsSurface {
            degree_u: self.degree_v,
            degree_v: self.degree_u,
            form_u: self.form_v,
            form_v: self.form_u,
            nu: self.nv,
            nv: self.nu,
            cvs,
            knots_u: self.knots_v.clone(),
            knots_v: self.knots_u.clone(),
            trim: self.trim.iter().map(|r| TrimRegion { keep_odd: r.keep_odd, loops: r.loops.iter().map(|l| l.iter().map(|p| glam::DVec2::new(p.y, p.x)).collect()).collect() }).collect(),
        }
    }

    /// One curve along U for each CV in V, wrapped ones included.
    pub fn columns(&self) -> Vec<NurbsCurve> {
        (0..self.nv).map(|v| NurbsCurve { degree: self.degree_u, form: self.form_u, cvs: (0..self.nu).map(|u| self.cv(u, v)).collect(), knots: self.knots_u.clone() }).collect()
    }

    /// Each column changed alike.
    pub fn map_u(&self, f: impl Fn(&NurbsCurve) -> NurbsCurve) -> NurbsSurface {
        let cols: Vec<NurbsCurve> = self.columns().iter().map(f).collect();
        NurbsSurface::from_columns(&cols, self.degree_v, self.form_v, self.knots_v.clone())
    }

    /// The point, and derivatives along U and V. Parameters clamp to the domain; a periodic direction wraps.
    pub fn eval_ders(&self, u: f64, v: f64) -> (DVec3, DVec3, DVec3) {
        if !self.is_valid() {
            return (self.cvs.first().copied().unwrap_or(DVec3::ZERO), DVec3::ZERO, DVec3::ZERO);
        }
        let (ulo, uhi) = self.domain_u();
        let (vlo, vhi) = self.domain_v();
        let u = wrap_param(self.form_u, u, ulo, uhi);
        let v = wrap_param(self.form_v, v, vlo, vhi);
        let (du, dv) = (self.degree_u, self.degree_v);
        let su = find_span(&self.knots_u, self.nu, du, u);
        let sv = find_span(&self.knots_v, self.nv, dv, v);
        let (nu, dnu) = basis_ders(&self.knots_u, su, u, du);
        let (nv, dnv) = basis_ders(&self.knots_v, sv, v, dv);
        let (mut p, mut pu, mut pv) = (DVec3::ZERO, DVec3::ZERO, DVec3::ZERO);
        for i in 0..=du {
            let row = (su - du + i) * self.nv;
            for j in 0..=dv {
                let c = self.cvs[row + sv - dv + j];
                p += c * (nu[i] * nv[j]);
                pu += c * (dnu[i] * nv[j]);
                pv += c * (nu[i] * dnv[j]);
            }
        }
        (p, pu, pv)
    }

    pub fn eval(&self, u: f64, v: f64) -> DVec3 {
        self.eval_ders(u, v).0
    }

    /// The unit normal, dU x dV. Where a derivative vanishes, at a pole, it comes from just beside the point.
    pub fn normal(&self, u: f64, v: f64) -> DVec3 {
        let (_, pu, pv) = self.eval_ders(u, v);
        let n = pu.cross(pv);
        if n.length_squared() > 1e-16 {
            return n.normalize();
        }
        let (ulo, uhi) = self.domain_u();
        let (vlo, vhi) = self.domain_v();
        let (eu, ev) = ((uhi - ulo).max(1e-9) * 1e-4, (vhi - vlo).max(1e-9) * 1e-4);
        let nudged_u = if u + eu <= uhi { u + eu } else { u - eu };
        let nudged_v = if v + ev <= vhi { v + ev } else { v - ev };
        for (a, b) in [(nudged_u, v), (u, nudged_v), (nudged_u, nudged_v)] {
            let (_, pu, pv) = self.eval_ders(a, b);
            let n = pu.cross(pv);
            if n.length_squared() > 1e-24 {
                return n.normalize();
            }
        }
        DVec3::ZERO
    }

    /// The curve along V at a U parameter: the reference's `surface.u[x]`.
    pub fn iso_u(&self, u: f64) -> NurbsCurve {
        if !self.is_valid() {
            return NurbsCurve::default();
        }
        let (lo, hi) = self.domain_u();
        let u = wrap_param(self.form_u, u, lo, hi);
        let d = self.degree_u;
        let s = find_span(&self.knots_u, self.nu, d, u);
        let n = basis_funs(&self.knots_u, s, u, d);
        let cvs = (0..self.nv).map(|v| (0..=d).map(|i| self.cv(s - d + i, v) * n[i]).sum()).collect();
        NurbsCurve { degree: self.degree_v, form: self.form_v, cvs, knots: self.knots_v.clone() }
    }

    /// The curve along U at a V parameter: the reference's `surface.v[x]`.
    pub fn iso_v(&self, v: f64) -> NurbsCurve {
        self.swapped().iso_u(v)
    }

    /// Isoparm polylines for display: one at each knot and `div` more within each span, `per_span` points to a span.
    pub fn isoparm_lines(&self, div_u: usize, div_v: usize, per_span: usize) -> Vec<Vec<DVec3>> {
        if !self.is_valid() {
            return Vec::new();
        }
        let mut us = span_params(&self.knots_u, self.degree_u, self.nu, div_u + 1);
        let mut vs = span_params(&self.knots_v, self.degree_v, self.nv, div_v + 1);
        if self.form_u != Form::Open && us.len() > 1 {
            us.pop();
        }
        if self.form_v != Form::Open && vs.len() > 1 {
            vs.pop();
        }
        let mut out = Vec::with_capacity(us.len() + vs.len());
        if self.trim.is_empty() {
            for u in us {
                out.push(self.iso_u(u).tessellate(per_span));
            }
            for v in vs {
                out.push(self.iso_v(v).tessellate(per_span));
            }
            return out;
        }
        // Trimmed: the runs of each isoparm inside the trim, and the trim's own edges.
        let n = (per_span.max(1) * 24).max(48);
        let (ulo, uhi) = self.domain_u();
        let (vlo, vhi) = self.domain_v();
        let mut runs = |param: &dyn Fn(f64) -> glam::DVec2| {
            let mut run: Vec<DVec3> = Vec::new();
            for k in 0..=n {
                let p = param(k as f64 / n as f64);
                if self.trim_keeps(p) {
                    run.push(self.eval(p.x, p.y));
                } else if !run.is_empty() {
                    out.push(std::mem::take(&mut run));
                }
            }
            if run.len() > 1 {
                out.push(run);
            }
        };
        for u in us {
            runs(&|t| glam::DVec2::new(u, vlo + (vhi - vlo) * t));
        }
        for v in vs {
            runs(&|t| glam::DVec2::new(ulo + (uhi - ulo) * t, v));
        }
        for region in &self.trim {
            for l in &region.loops {
                let mut line: Vec<DVec3> = l.iter().map(|p| self.eval(p.x, p.y)).collect();
                if let Some(&f) = line.first() {
                    line.push(f);
                }
                out.push(line);
            }
        }
        out
    }

    /// Whether every trim keeps this point of parameter space.
    pub fn trim_keeps(&self, p: glam::DVec2) -> bool {
        self.trim.iter().all(|r| r.keeps(p))
    }

    /// The mesh of what the trims keep: a grid of cells, each cut where a trim's edge crosses it.
    fn trimmed_mesh(&self, us: &[f64], vs: &[f64], opts: MeshOptions) -> crate::Mesh {
        use crate::attribute::names;
        use glam::DVec2;
        let refine = |list: &[f64], want: usize| -> Vec<f64> {
            let per = (want / list.len().max(2)).max(1);
            let mut out = Vec::new();
            for w in list.windows(2) {
                for k in 0..per {
                    out.push(w[0] + (w[1] - w[0]) * k as f64 / per as f64);
                }
            }
            out.extend(list.last());
            out
        };
        let (us, vs) = (refine(us, 64), refine(vs, 64));
        let mut m = crate::Mesh::default();
        let mut ids: std::collections::HashMap<(i64, i64), u32> = std::collections::HashMap::new();
        let (ulo, uhi) = self.domain_u();
        let (vlo, vhi) = self.domain_v();
        let mut corner_uv: Vec<[f32; 2]> = Vec::new();
        let mut corner_n: Vec<[f32; 3]> = Vec::new();
        for i in 0..us.len() - 1 {
            for j in 0..vs.len() - 1 {
                let corners = [DVec2::new(us[i], vs[j]), DVec2::new(us[i + 1], vs[j]), DVec2::new(us[i + 1], vs[j + 1]), DVec2::new(us[i], vs[j + 1])];
                let inside = corners.map(|c| self.trim_keeps(c));
                if inside.iter().all(|x| !*x) {
                    continue;
                }
                // Round the cell: the corners kept, and where each edge crosses a trim.
                let mut poly: Vec<DVec2> = Vec::with_capacity(6);
                for k in 0..4 {
                    let (a, b) = (corners[k], corners[(k + 1) % 4]);
                    if inside[k] {
                        poly.push(a);
                    }
                    if inside[k] != inside[(k + 1) % 4] {
                        let (mut lo, mut hi) = (0.0, 1.0);
                        for _ in 0..24 {
                            let mid = (lo + hi) * 0.5;
                            if self.trim_keeps(a.lerp(b, mid)) == inside[k] {
                                lo = mid;
                            } else {
                                hi = mid;
                            }
                        }
                        poly.push(a.lerp(b, (lo + hi) * 0.5));
                    }
                }
                if poly.len() < 3 {
                    continue;
                }
                let mut face = Vec::with_capacity(poly.len());
                for p in &poly {
                    let key = ((p.x * 1e8).round() as i64, (p.y * 1e8).round() as i64);
                    let id = *ids.entry(key).or_insert_with(|| m.add_point(self.eval(p.x, p.y)));
                    if !face.contains(&id) {
                        face.push(id);
                    }
                }
                if face.len() < 3 {
                    continue;
                }
                let tris: Vec<Vec<u32>> = if opts.quads { vec![face.clone()] } else { (1..face.len() - 1).map(|k| vec![face[0], face[k], face[k + 1]]).collect() };
                let mut uv_of: std::collections::HashMap<u32, DVec2> = std::collections::HashMap::new();
                for p in &poly {
                    let key = ((p.x * 1e8).round() as i64, (p.y * 1e8).round() as i64);
                    uv_of.insert(ids[&key], *p);
                }
                for t in tris {
                    m.add_face(&t);
                    for id in &t {
                        let p = uv_of[id];
                        corner_uv.push([((p.x - ulo) / (uhi - ulo).max(1e-12)) as f32, ((p.y - vlo) / (vhi - vlo).max(1e-12)) as f32]);
                        if opts.normals {
                            corner_n.push(self.normal(p.x, p.y).as_vec3().to_array());
                        }
                    }
                }
            }
        }
        for (dst, src) in m.corner_attrs_mut().get_or_add::<[f32; 2]>(&names::uv(0)).iter_mut().zip(&corner_uv) {
            *dst = *src;
        }
        if opts.normals {
            for (dst, src) in m.corner_attrs_mut().get_or_add::<[f32; 3]>(names::NORMAL).iter_mut().zip(&corner_n) {
                *dst = *src;
            }
        }
        m
    }

    /// The default display tessellation: quads with corner normals and `uv0` from the parameters.
    pub fn tessellate(&self, per_u: usize, per_v: usize) -> crate::Mesh {
        if !self.is_valid() {
            return crate::Mesh::default();
        }
        self.mesh_at(&self.params_u(per_u), &self.params_v(per_v), MeshOptions { quads: true, normals: true })
    }

    /// Parameters at `per` steps to a span along U.
    pub fn params_u(&self, per: usize) -> Vec<f64> {
        span_params(&self.knots_u, self.degree_u, self.nu, per)
    }
    /// Parameters at `per` steps to a span along V.
    pub fn params_v(&self, per: usize) -> Vec<f64> {
        span_params(&self.knots_v, self.degree_v, self.nv, per)
    }
    /// Parameters at each CV of a lap along U; a closed lap comes back to its start.
    pub fn cv_params_u(&self) -> Vec<f64> {
        grid_cv_params(&self.knots_u, self.degree_u, self.distinct_u(), self.form_u, self.domain_u())
    }
    /// Parameters at each CV of a lap along V; a closed lap comes back to its start.
    pub fn cv_params_v(&self) -> Vec<f64> {
        grid_cv_params(&self.knots_v, self.degree_v, self.distinct_v(), self.form_v, self.domain_v())
    }

    /// A mesh of the surface at given parameters. A row or column that collapses to a point is one vertex, as at a
    /// sphere's poles, and a closed direction shares its seam's points; each corner keeps its own UV.
    pub fn mesh_at(&self, us: &[f64], vs: &[f64], opts: MeshOptions) -> crate::Mesh {
        use crate::attribute::names;
        let mut m = crate::Mesh::default();
        if !self.is_valid() || us.len() < 2 || vs.len() < 2 {
            return m;
        }
        if !self.trim.is_empty() {
            return self.trimmed_mesh(us, vs, opts);
        }
        let (cu, cv) = (us.len(), vs.len());
        let pos: Vec<DVec3> = us.iter().flat_map(|&u| vs.iter().map(move |&v| (u, v))).map(|(u, v)| self.eval(u, v)).collect();
        let at = |i: usize, j: usize| pos[i * cv + j];
        let size = self.cvs.iter().fold(0.0_f64, |a, p| a.max(p.abs().max_element())).max(1e-6);
        let tol = size * 1e-9;
        let close = |a: DVec3, b: DVec3| (a - b).length() <= tol;
        let row_collapsed: Vec<bool> = (0..cu).map(|i| (1..cv).all(|j| close(at(i, j), at(i, 0)))).collect();
        let col_collapsed: Vec<bool> = (0..cv).map(|j| (1..cu).all(|i| close(at(i, j), at(0, j)))).collect();
        let wrap_u = self.form_u != Form::Open && (0..cv).all(|j| close(at(cu - 1, j), at(0, j)));
        let wrap_v = self.form_v != Form::Open && (0..cu).all(|i| close(at(i, cv - 1), at(i, 0)));
        let mut ids = vec![u32::MAX; cu * cv];
        for i in 0..cu {
            for j in 0..cv {
                let (mut a, mut b) = (i, j);
                if wrap_u && a == cu - 1 {
                    a = 0;
                }
                if wrap_v && b == cv - 1 {
                    b = 0;
                }
                if row_collapsed[a] {
                    b = 0;
                }
                if col_collapsed[b] {
                    a = 0;
                }
                let id = ids[a * cv + b];
                let id = if id != u32::MAX { id } else { m.add_point(at(a, b)) };
                ids[a * cv + b] = id;
                ids[i * cv + j] = id;
            }
        }
        let (ulo, uhi) = self.domain_u();
        let (vlo, vhi) = self.domain_v();
        let uv = |i: usize, j: usize| [((us[i] - ulo) / (uhi - ulo).max(1e-12)) as f32, ((vs[j] - vlo) / (vhi - vlo).max(1e-12)) as f32];
        let mut corner_uv: Vec<[f32; 2]> = Vec::new();
        let mut corner_n: Vec<[f32; 3]> = Vec::new();
        let mut emit = |m: &mut crate::Mesh, ring: &[(usize, usize)]| {
            let mut pts: Vec<(u32, (usize, usize))> = Vec::with_capacity(4);
            for &(i, j) in ring {
                let id = ids[i * cv + j];
                if !pts.iter().any(|p| p.0 == id) {
                    pts.push((id, (i, j)));
                }
            }
            if pts.len() < 3 {
                return;
            }
            let idx: Vec<u32> = pts.iter().map(|p| p.0).collect();
            m.add_face(&idx);
            for &(_, (i, j)) in &pts {
                corner_uv.push(uv(i, j));
                if opts.normals {
                    corner_n.push(self.normal(us[i], vs[j]).as_vec3().to_array());
                }
            }
        };
        for i in 0..cu - 1 {
            for j in 0..cv - 1 {
                let q = [(i, j), (i + 1, j), (i + 1, j + 1), (i, j + 1)];
                if opts.quads {
                    emit(&mut m, &q);
                } else {
                    emit(&mut m, &[q[0], q[1], q[2]]);
                    emit(&mut m, &[q[0], q[2], q[3]]);
                }
            }
        }
        for (dst, src) in m.corner_attrs_mut().get_or_add::<[f32; 2]>(&names::uv(0)).iter_mut().zip(&corner_uv) {
            *dst = *src;
        }
        if opts.normals {
            for (dst, src) in m.corner_attrs_mut().get_or_add::<[f32; 3]>(names::NORMAL).iter_mut().zip(&corner_n) {
                *dst = *src;
            }
        }
        m
    }

    /// The parameters of the point on the surface closest to `p`.
    pub fn closest_point(&self, p: DVec3) -> (f64, f64) {
        if !self.is_valid() {
            return (0.0, 0.0);
        }
        let us = self.params_u(6);
        let vs = self.params_v(6);
        let mut best = (f64::MAX, us[0], vs[0]);
        for &u in &us {
            for &v in &vs {
                let d = (self.eval(u, v) - p).length_squared();
                if d < best.0 {
                    best = (d, u, v);
                }
            }
        }
        let (ulo, uhi) = self.domain_u();
        let (vlo, vhi) = self.domain_v();
        let (mut u, mut v) = (best.1, best.2);
        for _ in 0..24 {
            let (q, pu, pv) = self.eval_ders(u, v);
            let r = q - p;
            let (a, b, c) = (pu.dot(pu), pu.dot(pv), pv.dot(pv));
            let (e, f) = (pu.dot(r), pv.dot(r));
            let det = a * c - b * b;
            let (mut su, mut sv) = if det.abs() > 1e-14 {
                ((c * e - b * f) / det, (a * f - b * e) / det)
            } else {
                (if a > 1e-14 { e / a } else { 0.0 }, if c > 1e-14 { f / c } else { 0.0 })
            };
            // Damped until it gets closer.
            let now = r.length_squared();
            let mut accepted = false;
            for _ in 0..6 {
                let nu = wrap_param(self.form_u, u - su, ulo, uhi);
                let nv = wrap_param(self.form_v, v - sv, vlo, vhi);
                if (self.eval(nu, nv) - p).length_squared() <= now {
                    if (nu - u).abs() + (nv - v).abs() < 1e-12 {
                        return (nu, nv);
                    }
                    u = nu;
                    v = nv;
                    accepted = true;
                    break;
                }
                su *= 0.5;
                sv *= 0.5;
            }
            if !accepted {
                break;
            }
        }
        (u, v)
    }

    /// A knot added along U; the shape doesn't change.
    pub fn insert_knot_u(&self, u: f64) -> NurbsSurface {
        let (lo, hi) = self.domain_u();
        if !self.is_valid() || (self.form_u != Form::Periodic && (u <= lo || u >= hi)) {
            return self.clone();
        }
        let mult = self.knots_u.iter().filter(|k| (**k - u).abs() < 1e-9).count();
        if mult >= self.degree_u {
            return self.clone();
        }
        self.map_u(|c| c.insert_knot_any(u))
    }
    /// A knot added along V; the shape doesn't change.
    pub fn insert_knot_v(&self, v: f64) -> NurbsSurface {
        self.swapped().insert_knot_u(v).swapped()
    }

    /// Clamped along U: a periodic direction becomes Closed with no change of shape.
    pub fn clamped_u(&self) -> NurbsSurface {
        if self.form_u != Form::Periodic {
            return self.clone();
        }
        self.map_u(|c| c.clamped())
    }

    /// Two surfaces meeting at a U parameter. A periodic direction opens at its seam first.
    pub fn split_u(&self, u: f64) -> (NurbsSurface, NurbsSurface) {
        let base = self.clamped_u();
        let (lo, hi) = base.domain_u();
        if !base.is_valid() || u <= lo + 1e-9 || u >= hi - 1e-9 {
            return (base.clone(), base);
        }
        let mut open = base;
        open.form_u = Form::Open;
        let pairs: Vec<(NurbsCurve, NurbsCurve)> = open.columns().iter().map(|c| c.split(u)).collect();
        let (left, right): (Vec<NurbsCurve>, Vec<NurbsCurve>) = pairs.into_iter().unzip();
        (
            NurbsSurface::from_columns(&left, self.degree_v, self.form_v, self.knots_v.clone()),
            NurbsSurface::from_columns(&right, self.degree_v, self.form_v, self.knots_v.clone()),
        )
    }
    /// Two surfaces meeting at a V parameter.
    pub fn split_v(&self, v: f64) -> (NurbsSurface, NurbsSurface) {
        let (a, b) = self.swapped().split_u(v);
        (a.swapped(), b.swapped())
    }

    /// Reversed along U, the knot spacing mirrored.
    pub fn reversed_u(&self) -> NurbsSurface {
        self.map_u(|c| c.reversed())
    }
    /// Reversed along V.
    pub fn reversed_v(&self) -> NurbsSurface {
        self.swapped().reversed_u().swapped()
    }

    /// Closed along U: periodic, with the CVs one lap. A last row on top of the first is dropped.
    pub fn closed_u(&self) -> NurbsSurface {
        if self.form_u == Form::Periodic || !self.is_valid() {
            return self.clone();
        }
        let shared = (0..self.nv).all(|v| (self.cv(0, v) - self.cv(self.nu - 1, v)).length() < 1e-9);
        self.map_u(|c| {
            let mut lap = c.cvs.clone();
            if shared && lap.len() > 2 {
                lap.pop();
            }
            NurbsCurve::periodic(c.degree, lap)
        })
    }
    /// Closed along V.
    pub fn closed_v(&self) -> NurbsSurface {
        self.swapped().closed_u().swapped()
    }

    /// The other surface after this one along U, sharing a row when their ends meet and otherwise joined by a straight
    /// span. With `blend`, both ends move to a point between them instead, by `bias`. V knots are made to agree first.
    pub fn attached_u(&self, other: &NurbsSurface, blend: bool, bias: f64) -> NurbsSurface {
        let (a, b) = make_compatible_v(&self.clamped_u(), &other.clamped_u());
        if !a.is_valid() || !b.is_valid() {
            return self.clone();
        }
        let d = a.degree_u.max(b.degree_u);
        let lift = |s: &NurbsSurface| {
            if s.degree_u == d {
                s.clone()
            } else {
                s.rebuilt(RebuildDir { on: true, spans: s.spans_u().max(1), degree: d, form: s.form_u }, RebuildDir { on: false, spans: 0, degree: s.degree_v, form: s.form_v }, 1)
            }
        };
        let (a, mut b) = (lift(&a), lift(&b));
        let nv = a.nv;
        let gap = (0..nv).map(|v| (a.cv(a.nu - 1, v) - b.cv(0, v)).length()).fold(0.0, f64::max);
        let (_, ahi) = a.domain_u();
        let mut rows: Vec<Vec<DVec3>> = (0..a.nu).map(|u| (0..nv).map(|v| a.cv(u, v)).collect()).collect();
        let mut knots: Vec<f64> = a.knots_u.clone();
        let mut end = ahi;
        if gap > 1e-9 {
            if blend {
                for v in 0..nv {
                    let mid = a.cv(a.nu - 1, v).lerp(b.cv(0, v), bias);
                    rows[a.nu - 1][v] = mid;
                    b.cvs[v] = mid;
                }
            } else {
                // A straight span, its CVs evenly along; the last of them is the other surface's first row.
                for i in 1..d {
                    let f = i as f64 / d as f64;
                    rows.push((0..nv).map(|v| a.cv(a.nu - 1, v).lerp(b.cv(0, v), f)).collect());
                }
                rows.push((0..nv).map(|v| b.cv(0, v)).collect());
                end += 1.0;
                knots.extend(std::iter::repeat_n(end, d));
            }
        }
        let (blo, _) = b.domain_u();
        for u in 1..b.nu {
            rows.push((0..nv).map(|v| b.cv(u, v)).collect());
        }
        for &k in b.knots_u.iter().skip(d) {
            knots.push(end + k - blo);
        }
        let cvs = rows.into_iter().flatten().collect::<Vec<_>>();
        let nu = cvs.len() / nv.max(1);
        NurbsSurface { degree_u: d, degree_v: a.degree_v, form_u: Form::Open, form_v: a.form_v, nu, nv, cvs, knots_u: knots, knots_v: a.knots_v, trim: Vec::new() }
    }
    /// Attached along V.
    pub fn attached_v(&self, other: &NurbsSurface, blend: bool, bias: f64) -> NurbsSurface {
        self.swapped().attached_u(&other.swapped(), blend, bias).swapped()
    }

    /// A surface of given knots fitted through points of `f`, one point to a CV, at the CVs' parameters.
    pub fn fitted(cu: (usize, Form, Vec<f64>), cv: (usize, Form, Vec<f64>), f: impl Fn(f64, f64) -> DVec3) -> NurbsSurface {
        let (du, fu, ku) = cu;
        let (dv, fv, kv) = cv;
        let (nu, nv) = (ku.len() + 1 - du, kv.len() + 1 - dv);
        let lap_u = if fu == Form::Periodic { nu - du } else { nu };
        let lap_v = if fv == Form::Periodic { nv - dv } else { nv };
        let pu = cv_params(&ku, du, lap_u);
        let pv = cv_params(&kv, dv, lap_v);
        let ma = basis_matrix(&ku, du, nu, lap_u, fu, &pu);
        let mb = basis_matrix(&kv, dv, nv, lap_v, fv, &pv);
        // First across V for each sample row, then down U for each column.
        let rows: Vec<Vec<DVec3>> = pu.iter().map(|&u| solve(&mb, &pv.iter().map(|&v| f(u, v)).collect::<Vec<_>>())).collect();
        let mut grid = vec![DVec3::ZERO; nu * nv];
        for j in 0..lap_v {
            let col: Vec<DVec3> = rows.iter().map(|r| r[j]).collect();
            for (i, p) in solve(&ma, &col).into_iter().enumerate() {
                grid[i * nv + j] = p;
            }
        }
        let mut s = NurbsSurface { degree_u: du, degree_v: dv, form_u: fu, form_v: fv, nu, nv, cvs: grid, knots_u: ku, knots_v: kv, trim: Vec::new() };
        s.rewrap();
        s
    }

    /// Rebuilt with even spans. `range` is the reference's keepRange: 0 the parameters run 0 to 1, 1 they keep their range, 2 they
    /// run 0 to the span count. A direction that's off stays as it is.
    pub fn rebuilt(&self, u: RebuildDir, v: RebuildDir, range: i64) -> NurbsSurface {
        if !self.is_valid() {
            return self.clone();
        }
        let plan = |d: RebuildDir, knots: &[f64], old_deg: usize, old_spans: usize, dom: (f64, f64), old_form: Form| {
            if !d.on {
                return ((old_deg, old_form, knots.to_vec()), dom.0, dom.1);
            }
            let spans = if d.spans == 0 { old_spans } else { d.spans }.max(1);
            let deg = d.degree.max(1);
            let base = if d.form == Form::Periodic { periodic_knots(spans, deg) } else { open_knots(spans + deg, deg) };
            let (a, b) = match range {
                0 => (0.0, 1.0),
                2 => (0.0, spans as f64),
                _ => dom,
            };
            let scale = (b - a) / spans as f64;
            ((deg, d.form, base.into_iter().map(|k| a + k * scale).collect()), a, b)
        };
        let (du, dv) = (self.domain_u(), self.domain_v());
        let (cu, au, bu) = plan(u, &self.knots_u, self.degree_u, self.spans_u(), du, self.form_u);
        let (cv, av, bv) = plan(v, &self.knots_v, self.degree_v, self.spans_v(), dv, self.form_v);
        let map = |t: f64, a: f64, b: f64, dom: (f64, f64)| dom.0 + (t - a) / (b - a).max(1e-12) * (dom.1 - dom.0);
        NurbsSurface::fitted(cu, cv, |s, t| self.eval(map(s, au, bu, du), map(t, av, bv, dv)))
    }

    /// Offset along the normals, fitted with the same knots.
    pub fn offset(&self, distance: f64) -> NurbsSurface {
        if !self.is_valid() {
            return self.clone();
        }
        NurbsSurface::fitted((self.degree_u, self.form_u, self.knots_u.clone()), (self.degree_v, self.form_v, self.knots_v.clone()), |u, v| self.eval(u, v) + self.normal(u, v) * distance)
    }
}

fn grid_cv_params(k: &[f64], d: usize, lap: usize, form: Form, dom: (f64, f64)) -> Vec<f64> {
    let (lo, hi) = dom;
    let mut p = cv_params(k, d, lap);
    if form == Form::Periodic {
        for x in p.iter_mut() {
            *x = wrap_param(Form::Periodic, *x, lo, hi);
        }
        p.sort_by(f64::total_cmp);
        p.push(p[0] + (hi - lo));
    }
    p
}

fn basis_matrix(k: &[f64], d: usize, total: usize, lap: usize, form: Form, params: &[f64]) -> Vec<Vec<f64>> {
    let lo = k[d - 1];
    let hi = k[total - 1];
    params
        .iter()
        .map(|&t| {
            let t = wrap_param(form, t, lo, hi);
            let s = find_span(k, total, d, t);
            let mut row = vec![0.0; lap];
            for (j, v) in basis_funs(k, s, t, d).iter().enumerate() {
                row[(s - d + j) % lap] += *v;
            }
            row
        })
        .collect()
}

/// Two surfaces given the same degree and knots along V, so their rows line up.
fn make_compatible_v(a: &NurbsSurface, b: &NurbsSurface) -> (NurbsSurface, NurbsSurface) {
    if a.degree_v == b.degree_v && a.form_v == b.form_v && a.knots_v == b.knots_v {
        return (a.clone(), b.clone());
    }
    let d = a.degree_v.max(b.degree_v);
    let prep = |s: &NurbsSurface| {
        let t = s.swapped().clamped_u();
        let t = if t.degree_u == d {
            t
        } else {
            t.rebuilt(RebuildDir { on: true, spans: t.spans_u().max(1), degree: d, form: t.form_u }, RebuildDir { on: false, spans: 0, degree: t.degree_v, form: t.form_v }, 1)
        };
        // Parameters scaled to 0..1, so the two can be compared.
        let (lo, hi) = t.domain_u();
        let scale = 1.0 / (hi - lo).max(1e-12);
        NurbsSurface { knots_u: t.knots_u.iter().map(|k| (k - lo) * scale).collect(), ..t }
    };
    let (mut ta, mut tb) = (prep(a), prep(b));
    let interior = |s: &NurbsSurface| -> Vec<f64> { s.knots_u.iter().copied().filter(|&k| k > 1e-9 && k < 1.0 - 1e-9).collect() };
    let (ka, kb) = (interior(&ta), interior(&tb));
    let missing = |have: &[f64], want: &[f64]| -> Vec<f64> {
        let mut left = have.to_vec();
        let mut out = Vec::new();
        for &k in want {
            match left.iter().position(|x| (x - k).abs() < 1e-9) {
                Some(i) => {
                    left.remove(i);
                }
                None => out.push(k),
            }
        }
        out
    };
    for k in missing(&ka, &kb) {
        ta = ta.map_u(|c| c.insert_knot(k));
    }
    for k in missing(&kb, &ka) {
        tb = tb.map_u(|c| c.insert_knot(k));
    }
    (ta.swapped(), tb.swapped())
}

/// A curve turned about an axis: U along the curve, V around, the turn running from +X toward +Z about +Y.
pub fn revolved(profile: &NurbsCurve, axis: DVec3, pivot: DVec3, start_deg: f64, end_deg: f64, sections: usize, degree: usize) -> NurbsSurface {
    let axis = axis.normalize_or_zero();
    let axis = if axis == DVec3::ZERO { DVec3::Y } else { axis };
    let arc = unit_arc(start_deg, end_deg, sections, degree);
    NurbsSurface::from_curves(profile, &arc, |i, j| {
        let p = profile.cvs[i] - pivot;
        let foot = axis * axis.dot(p);
        let r = p - foot;
        let w = (-axis).cross(r);
        pivot + foot + r * arc.cvs[j].x + w * arc.cvs[j].y
    })
}

/// The reference's sphere with its axis along Y and radius 1: U from pole to pole in `spans` spans, V around in `sections`.
pub fn sphere_surface(start_deg: f64, end_deg: f64, sections: usize, spans: usize, degree: usize) -> NurbsSurface {
    let degree = if degree == 1 { 1 } else { 3 };
    let spans = spans.max(if degree == 1 { 2 } else { 1 });
    let profile = unit_arc(-90.0, 90.0, spans, degree);
    revolved(&profile, DVec3::Y, DVec3::ZERO, start_deg, end_deg, sections, degree)
}

/// A flat patch from a corner along two edges, of even spans.
pub fn patch_surface(origin: DVec3, du: DVec3, dv: DVec3, spans_u: usize, spans_v: usize, degree: usize) -> NurbsSurface {
    let cu = NurbsCurve::line(DVec3::ZERO, du, spans_u, degree);
    let cv = NurbsCurve::line(DVec3::ZERO, dv, spans_v, degree);
    NurbsSurface::from_curves(&cu, &cv, |i, j| origin + cu.cvs[i] + cv.cvs[j])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sphere_matches_the_reference_cvs() {
        let s = sphere_surface(0.0, 360.0, 8, 4, 3);
        assert_eq!((s.nu, s.nv, s.spans_u(), s.spans_v()), (7, 11, 4, 8));
        assert_eq!((s.distinct_u(), s.distinct_v()), (7, 8));
        assert_eq!(s.knots_u, vec![0.0, 0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 4.0, 4.0]);
        assert_eq!(s.knots_v, (-2..=10).map(f64::from).collect::<Vec<_>>());
        assert_eq!(s.form_v, Form::Periodic);
        assert_eq!(s.domain_u(), (0.0, 4.0));
        assert_eq!(s.domain_v(), (0.0, 8.0));
        // The numbers the reference writes for a unit sphere about Y.
        assert!((s.cv(1, 0) - DVec3::new(0.199917, -1.0, -0.199917)).length() < 1e-5, "{:?}", s.cv(1, 0));
        assert!((s.cv(1, 1) - DVec3::new(0.282725, -1.0, 0.0)).length() < 1e-5, "{:?}", s.cv(1, 1));
        assert!((s.cv(2, 0) - DVec3::new(0.616430, -0.783612, -0.616430)).length() < 1e-5, "{:?}", s.cv(2, 0));
        assert!((s.cv(2, 1) - DVec3::new(0.871764, -0.783612, 0.0)).length() < 1e-5, "{:?}", s.cv(2, 1));
        assert!((s.cv(0, 3) - DVec3::new(0.0, -1.0, 0.0)).length() < 1e-9);
        for i in 0..=16 {
            for j in 0..=16 {
                let (u, v) = (4.0 * i as f64 / 16.0, 8.0 * j as f64 / 16.0);
                let p = s.eval(u, v);
                assert!((p.length() - 1.0).abs() < 0.012, "{u} {v} {p:?}");
                // Normals point out.
                if i > 0 && i < 16 {
                    assert!(s.normal(u, v).dot(p.normalize()) > 0.99, "{u} {v}");
                }
            }
        }
        assert!((s.eval(2.0, 0.0) - DVec3::X).length() < 0.01);
    }

    #[test]
    fn derivatives_match_differences() {
        let s = sphere_surface(0.0, 360.0, 8, 4, 3);
        let h = 1e-6;
        let (u, v) = (1.3, 2.7);
        let (_, pu, pv) = s.eval_ders(u, v);
        let fu = (s.eval(u + h, v) - s.eval(u - h, v)) / (2.0 * h);
        let fv = (s.eval(u, v + h) - s.eval(u, v - h)) / (2.0 * h);
        assert!((pu - fu).length() < 1e-5 && (pv - fv).length() < 1e-5);
    }

    #[test]
    fn plane_is_flat_with_even_cvs() {
        let s = patch_surface(DVec3::new(-0.5, 0.0, 0.5), DVec3::X, -DVec3::Z, 1, 1, 3);
        assert_eq!((s.nu, s.nv), (4, 4));
        assert!((s.cv(1, 2) - DVec3::new(-1.0 / 6.0, 0.0, -1.0 / 6.0)).length() < 1e-9);
        assert!((s.eval(0.25, 0.75) - DVec3::new(-0.25, 0.0, -0.25)).length() < 1e-9);
        assert!((s.normal(0.5, 0.5) - DVec3::Y).length() < 1e-9);
    }

    #[test]
    fn surface_mesh_welds_poles_and_seams() {
        let s = sphere_surface(0.0, 360.0, 8, 4, 3);
        let m = s.mesh_at(&s.params_u(3), &s.params_v(3), MeshOptions { quads: true, normals: false });
        assert_eq!(m.num_faces(), 12 * 24);
        assert_eq!(m.num_points(), 11 * 24 + 2);
        let p = patch_surface(DVec3::ZERO, DVec3::X, DVec3::Z, 2, 3, 1);
        let m = p.tessellate(4, 4);
        assert_eq!((m.num_faces(), m.num_points()), (6, 12));
        assert!(m.corner_attrs().find("uv0").is_some());
    }

    #[test]
    fn surface_knot_insertion_and_split_keep_the_shape() {
        let s = sphere_surface(0.0, 360.0, 8, 4, 3);
        let a = s.insert_knot_u(1.5);
        assert_eq!((a.nu, a.spans_u()), (8, 5));
        let b = s.insert_knot_v(3.25);
        assert_eq!((b.distinct_v(), b.spans_v(), b.form_v), (9, 9, Form::Periodic));
        let (l, r) = s.split_u(2.0);
        for i in 0..=8 {
            for j in 0..=8 {
                let (u, v) = (4.0 * i as f64 / 8.0, 8.0 * j as f64 / 8.0);
                let want = s.eval(u, v);
                assert!((a.eval(u, v) - want).length() < 1e-9);
                assert!((b.eval(u, v) - want).length() < 1e-9, "{u} {v}");
                let got = if u <= 2.0 { l.eval(u, v) } else { r.eval(u, v) };
                assert!((got - want).length() < 1e-9, "{u} {v}");
            }
        }
        let c = s.swapped().clamped_u().swapped();
        assert_eq!(c.form_v, Form::Closed);
        assert!((c.eval(1.0, 3.3) - s.eval(1.0, 3.3)).length() < 1e-9);
    }

    #[test]
    fn iso_rebuild_reverse_closest() {
        let s = sphere_surface(0.0, 360.0, 8, 4, 3);
        let c = s.iso_u(1.0);
        assert!((c.eval(2.5) - s.eval(1.0, 2.5)).length() < 1e-9);
        let c = s.iso_v(1.0);
        assert!((c.eval(2.5) - s.eval(2.5, 1.0)).length() < 1e-9);
        let (u, v) = s.closest_point(DVec3::new(3.0, 0.0, 0.0));
        assert!((s.eval(u, v) - DVec3::X).length() < 0.02, "{u} {v}");
        let dir = |spans, form| RebuildDir { on: true, spans, degree: 3, form };
        let r = s.rebuilt(dir(s.spans_u(), s.form_u), dir(s.spans_v(), s.form_v), 1);
        for (a, b) in r.cvs.iter().zip(&s.cvs) {
            assert!((*a - *b).length() < 1e-6, "{a:?} {b:?}");
        }
        let finer = s.rebuilt(dir(8, s.form_u), dir(16, s.form_v), 0);
        assert_eq!(finer.domain_u(), (0.0, 1.0));
        assert!((finer.eval(0.5, 0.0).length() - 1.0).abs() < 0.01);
        let rv = s.reversed_u();
        assert!((rv.eval(0.5, 1.0) - s.eval(3.5, 1.0)).length() < 1e-9);
        assert!(s.swapped().normal(1.0, 2.0).dot(s.normal(2.0, 1.0)) < 0.0);
    }

    #[test]
    fn attach_joins_rows() {
        let a = patch_surface(DVec3::ZERO, DVec3::X, DVec3::Z, 1, 1, 3);
        let b = patch_surface(DVec3::X, DVec3::X, DVec3::Z, 1, 1, 3);
        let c = a.attached_u(&b, false, 0.5);
        assert_eq!((c.nu, c.spans_u()), (7, 4));
        assert!((c.eval(1.5, 0.5) - DVec3::new(1.5, 0.0, 0.5)).length() < 1e-9);
        let far = patch_surface(DVec3::new(3.0, 0.0, 0.0), DVec3::X, DVec3::Z, 1, 1, 3);
        let d = a.attached_u(&far, false, 0.5);
        assert_eq!((d.nu, d.domain_u()), (10, (0.0, 3.0)));
        assert!(d.is_valid());
        assert!((d.eval(d.domain_u().1, 0.0) - DVec3::new(4.0, 0.0, 0.0)).length() < 1e-9);
    }

    #[test]
    fn open_curves_hit_their_ends() {
        let c = NurbsCurve::open(3, vec![DVec3::ZERO, DVec3::X, DVec3::new(2.0, 1.0, 0.0), DVec3::new(3.0, 1.0, 0.0), DVec3::new(4.0, 0.0, 0.0)]);
        assert_eq!(c.knots, vec![0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0]);
        assert_eq!(c.domain(), (0.0, 2.0));
        assert!((c.eval(0.0) - DVec3::ZERO).length() < 1e-9);
        assert!((c.eval(2.0) - DVec3::new(4.0, 0.0, 0.0)).length() < 1e-9);
        assert_eq!(c.spans(), 2);
    }

    #[test]
    fn circle_matches_the_reference_cvs() {
        let c = circle(DVec3::ZERO, DVec3::Y, DVec3::X, 1.0, 3, 8, 360.0);
        assert_eq!(c.cvs.len(), 11);
        assert_eq!(c.form, Form::Periodic);
        // The reference's first CVs for a unit circle about Y.
        assert!((c.cvs[0] - DVec3::new(0.783612, 0.0, -0.783612)).length() < 1e-5, "{:?}", c.cvs[0]);
        assert!((c.cvs[1] - DVec3::new(0.0, 0.0, -1.108194)).length() < 1e-5, "{:?}", c.cvs[1]);
        for i in 0..=80 {
            let (lo, hi) = c.domain();
            let p = c.eval(lo + (hi - lo) * i as f64 / 80.0);
            assert!((p.length() - 1.0).abs() < 0.01, "{p:?}");
        }
    }

    #[test]
    fn knot_insertion_keeps_the_shape() {
        let c = NurbsCurve::open(3, vec![DVec3::ZERO, DVec3::X, DVec3::new(2.0, 1.0, 0.0), DVec3::new(3.0, 1.0, 0.0), DVec3::new(4.0, 0.0, 0.0)]);
        let d = c.insert_knot(0.5);
        assert_eq!(d.cvs.len(), 6);
        for i in 0..=20 {
            let u = 2.0 * i as f64 / 20.0;
            assert!((c.eval(u) - d.eval(u)).length() < 1e-9);
        }
    }

    #[test]
    fn split_and_interpolate() {
        let c = NurbsCurve::open(3, vec![DVec3::ZERO, DVec3::X, DVec3::new(2.0, 1.0, 0.0), DVec3::new(3.0, 1.0, 0.0), DVec3::new(4.0, 0.0, 0.0)]);
        let (a, b) = c.split(1.0);
        assert!((a.eval(a.domain().1) - c.eval(1.0)).length() < 1e-6, "{:?} {:?}", a, c.eval(1.0));
        assert!((b.eval(b.domain().0) - c.eval(1.0)).length() < 1e-6);
        assert!((b.eval(b.domain().1) - c.eval(2.0)).length() < 1e-6);
        let pts = [DVec3::ZERO, DVec3::new(1.0, 1.0, 0.0), DVec3::new(2.0, 0.0, 0.0), DVec3::new(3.0, 1.0, 0.0)];
        let t = NurbsCurve::through(3, &pts);
        for p in pts {
            let u = t.closest_param(p);
            assert!((t.eval(u) - p).length() < 1e-4, "{p:?} {u}");
        }
    }

    #[test]
    fn reverse_and_length() {
        let c = NurbsCurve::open(1, vec![DVec3::ZERO, DVec3::X, DVec3::new(1.0, 1.0, 0.0)]);
        assert!((c.length() - 2.0).abs() < 1e-9);
        let r = c.reversed();
        assert!((r.eval(r.domain().0) - DVec3::new(1.0, 1.0, 0.0)).length() < 1e-9);
        assert!((c.eval(c.param_at_fraction(0.5)) - DVec3::X).length() < 1e-6);
    }
}
