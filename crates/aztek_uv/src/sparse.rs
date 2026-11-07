//! Sparse symmetric solves, standing in for Eigen's `SimplicialLDLT` (AMD ordering, simplicial LDLT). Built on faer.

use faer::dyn_stack::{MemBuffer, MemStack, StackReq};
use faer::linalg::cholesky::ldlt::factor::LdltRegularization;
use faer::reborrow::*;
use faer::sparse::linalg::amd;
use faer::sparse::linalg::cholesky::simplicial;
use faer::sparse::{SparseColMat, SymbolicSparseColMat, Triplet as FTriplet};

/// One entry of a sparse matrix; entries at the same position add up.
#[derive(Clone, Copy, Debug)]
pub struct Triplet {
    pub row: usize,
    pub col: usize,
    pub value: f64,
}

impl Triplet {
    pub fn new(row: usize, col: usize, value: f64) -> Self {
        Self { row, col, value }
    }
}

/// The upper triangle of a symmetric matrix given by all its entries.
fn upper(n: usize, entries: &[Triplet]) -> Option<SparseColMat<usize, f64>> {
    let t: Vec<FTriplet<usize, usize, f64>> = entries
        .iter()
        .filter(|e| e.row <= e.col && e.row < n && e.col < n)
        .map(|e| FTriplet::new(e.row, e.col, e.value))
        .collect();
    SparseColMat::try_new_from_triplets(n, n, &t).ok()
}

/// A symmetric matrix factored as P^T L D L^T P. Like Eigen's SimplicialLDLT: no pivoting, so it suits the
/// symmetric positive (semi)definite systems the uv solvers build.
pub struct Ldlt {
    n: usize,
    perm: Vec<usize>,
    perm_inv: Vec<usize>,
    symbolic: Option<simplicial::SymbolicSimplicialCholesky<usize>>,
    values: Vec<f64>,
    ok: bool,
}

impl Ldlt {
    /// The ordering and symbolic factor of the matrix's pattern (Eigen's analyzePattern).
    pub fn analyze(n: usize, entries: &[Triplet]) -> Self {
        let mut s = Self { n, perm: (0..n).collect(), perm_inv: (0..n).collect(), symbolic: None, values: Vec::new(), ok: false };
        let Some(a) = upper(n, entries) else { return s };
        if n == 0 {
            return s;
        }
        let nnz = a.compute_nnz();
        let mut mem = MemBuffer::new(amd::order_scratch::<usize>(n, nnz));
        if amd::order(&mut s.perm, &mut s.perm_inv, a.symbolic(), amd::Control::default(), MemStack::new(&mut mem)).is_err() {
            return s;
        }
        let Some(ap) = s.permuted(&a) else { return s };
        let mut mem = MemBuffer::new(StackReq::any_of(&[
            simplicial::prefactorize_symbolic_cholesky_scratch::<usize>(n, nnz),
            simplicial::factorize_simplicial_symbolic_cholesky_scratch::<usize>(n),
        ]));
        let stack = MemStack::new(&mut mem);
        let mut etree = vec![0isize; n];
        let mut col_counts = vec![0usize; n];
        simplicial::prefactorize_symbolic_cholesky(&mut etree, &mut col_counts, ap.symbolic(), stack);
        s.symbolic = simplicial::factorize_simplicial_symbolic_cholesky(
            ap.symbolic(),
            // SAFETY: etree was filled by prefactorize_symbolic_cholesky.
            unsafe { simplicial::EliminationTreeRef::from_inner(&etree) },
            &col_counts,
            stack,
        )
        .ok();
        s
    }

    /// Analyze and factorize (Eigen's compute).
    pub fn new(n: usize, entries: &[Triplet]) -> Self {
        let mut s = Self::analyze(n, entries);
        s.factorize(entries);
        s
    }

    fn permuted(&self, a: &SparseColMat<usize, f64>) -> Option<SparseColMat<usize, f64>> {
        let n = self.n;
        let nnz = a.compute_nnz();
        let perm = unsafe { faer::perm::PermRef::new_unchecked(&self.perm, &self.perm_inv, n) };
        let (mut values, mut col_ptrs, mut row_idx) = (vec![0.0; nnz], vec![0usize; n + 1], vec![0usize; nnz]);
        let mut mem = MemBuffer::new(faer::sparse::utils::permute_self_adjoint_scratch::<usize>(n));
        faer::sparse::utils::permute_self_adjoint_to_unsorted(
            &mut values,
            &mut col_ptrs,
            &mut row_idx,
            a.rb(),
            perm,
            faer::Side::Upper,
            faer::Side::Upper,
            MemStack::new(&mut mem),
        );
        Some(SparseColMat::new(unsafe { SymbolicSparseColMat::new_unchecked(n, n, col_ptrs, None, row_idx) }, values))
    }

    /// Numeric factorization for new values on the analysed pattern (Eigen's factorize). False on a zero or
    /// non-finite pivot.
    pub fn factorize(&mut self, entries: &[Triplet]) -> bool {
        self.ok = false;
        let Some(symbolic) = &self.symbolic else { return false };
        let Some(a) = upper(self.n, entries) else { return false };
        let Some(ap) = self.permuted(&a) else { return false };
        self.values = vec![0.0; symbolic.len_val()];
        let mut mem = MemBuffer::new(simplicial::factorize_simplicial_numeric_ldlt_scratch::<usize, f64>(self.n));
        if simplicial::factorize_simplicial_numeric_ldlt::<usize, f64>(
            &mut self.values,
            ap.rb(),
            LdltRegularization::default(),
            symbolic,
            MemStack::new(&mut mem),
        )
        .is_err()
        {
            return false;
        }
        // The first entry of each column of the factor holds the pivot.
        let col_ptr = symbolic.col_ptr();
        self.ok = self.values.iter().all(|v| v.is_finite()) && (0..self.n).all(|j| self.values[col_ptr[j]] != 0.0);
        self.ok
    }

    /// Eigen's info() == Success.
    pub fn ok(&self) -> bool {
        self.ok
    }

    /// Solves A x = b. None when the factorization failed or the solution is not finite.
    pub fn solve(&self, b: &[f64]) -> Option<Vec<f64>> {
        if !self.ok || b.len() != self.n {
            return None;
        }
        let symbolic = self.symbolic.as_ref()?;
        let n = self.n;
        let perm = unsafe { faer::perm::PermRef::new_unchecked(&self.perm, &self.perm_inv, n) };
        let mut x = b.to_vec();
        let mut mem = MemBuffer::new(StackReq::all_of(&[
            faer::perm::permute_rows_in_place_scratch::<usize, f64>(n, 1),
            symbolic.solve_in_place_scratch::<f64>(n),
        ]));
        let stack = MemStack::new(&mut mem);
        {
            let mut sol = faer::MatMut::from_column_major_slice_mut(&mut x, n, 1);
            faer::perm::permute_rows_in_place(sol.rb_mut(), perm, stack);
            let ldlt = simplicial::SimplicialLdltRef::<'_, usize, f64>::new(symbolic, &self.values);
            ldlt.solve_in_place_with_conj(faer::Conj::No, sol.rb_mut(), faer::Par::Seq, stack);
            faer::perm::permute_rows_in_place(sol.rb_mut(), perm.inverse(), stack);
        }
        x.iter().all(|v| v.is_finite()).then_some(x)
    }
}

/// Dense symmetric 3x3 solve by LDLT with pivoting (Eigen's Matrix3d::ldlt().solve()).
pub fn solve3_ldlt(a: [[f64; 3]; 3], b: [f64; 3]) -> [f64; 3] {
    let m = faer::Mat::<f64>::from_fn(3, 3, |i, j| a[i][j]);
    let rhs = faer::Mat::<f64>::from_fn(3, 1, |i, _| b[i]);
    use faer::linalg::solvers::Solve;
    let x = m.lblt(faer::Side::Lower).solve(&rhs);
    [x[(0, 0)], x[(1, 0)], x[(2, 0)]]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solves_a_laplacian_with_duplicates() {
        // 1D Laplacian with the first point pinned (as a big diagonal), entries split into duplicates.
        let n = 5;
        let mut t = Vec::new();
        for i in 0..n {
            t.push(Triplet::new(i, i, 1.0));
            t.push(Triplet::new(i, i, 1.0));
            if i + 1 < n {
                t.push(Triplet::new(i, i + 1, -1.0));
                t.push(Triplet::new(i + 1, i, -1.0));
            }
        }
        t.push(Triplet::new(0, 0, 10.0));
        let s = Ldlt::new(n, &t);
        assert!(s.ok());
        let b = [1.0, 0.0, 0.0, 0.0, 1.0];
        let x = s.solve(&b).unwrap();
        // Check A x = b.
        let mut ax = vec![0.0; n];
        for e in &t {
            ax[e.row] += e.value * x[e.col];
        }
        for i in 0..n {
            assert!((ax[i] - b[i]).abs() < 1e-10, "{ax:?}");
        }
    }

    #[test]
    fn a_singular_matrix_fails() {
        let t = [Triplet::new(0, 0, 1.0), Triplet::new(0, 1, -1.0), Triplet::new(1, 0, -1.0), Triplet::new(1, 1, 1.0)];
        assert!(!Ldlt::new(2, &t).ok());
    }
}
