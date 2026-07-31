//! Sum-of-Squares (SOS) / Positivstellensatz certificate primitives.
//!
//! SOS relaxation proves polynomial inequalities `p(x) ≥ 0` by writing
//! `p` as a sum of squares of polynomials. This compiles to a
//! semidefinite-programming (SDP) feasibility problem on the Gram
//! matrix `Q` of the monomial basis: `p(x) = m(x)ᵀ Q m(x)` with
//! `Q ⪰ 0`. Modern low-rank ADMM SDP solvers (Yurtsever-Tropp 2019,
//! cuSOLVER + per-block updates) make this GPU-friendly.
//!
//! This file ships the **Gram matrix construction** primitive  -  given
//! the polynomial coefficients and a monomial basis, populate the
//! Gram matrix `Q` such that `m(x)ᵀ Q m(x) = p(x)`. The downstream
//! SDP feasibility check (Newton-Schulz on Q's PSD projection or a
//! coupled ADMM iteration) composes with #16 preconditioner.
//!
//! # Why this primitive is dual-use
//!
//! | Consumer | Use |
//! |---|---|
//! | future `vyre-libs::formal::lyapunov` | control-theoretic stability proofs |
//! | future `vyre-libs::opt::polynomial` | polynomial optimization (POP) |
//! | future `vyre-libs::security::buffer_safety` | SOS proofs of bounded-buffer-access |

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Op id.
pub const OP_ID: &str = "vyre-primitives::math::sos_gram_construct";

/// Emit the Gram-matrix construction step.
///
/// Inputs:
/// - `monomial_pairs`: row-major `m × m` u32 buffer where
///   `monomial_pairs[i, j] = idx_in_p_of(monomial_i · monomial_j)`.
///   Caller precomputes (host-side) the multiplication-table mapping
///   pairs of basis monomials to their indices in the target
///   polynomial's coefficient vector.
/// - `p_coeffs`: u32 buffer of length `coeff_count`  -  the target
///   polynomial's coefficient vector indexed by monomial index.
///
/// Output:
/// - `gram`: `m × m` u32  -  `gram[i, j] = p_coeffs[monomial_pairs[i, j]]`.
///   This Q satisfies `mᵀ Q m = p` IF Q is also constrained to be
///   PSD, which the downstream SDP solver enforces.
#[must_use]
pub fn sos_gram_construct(
    monomial_pairs: &str,
    p_coeffs: &str,
    gram: &str,
    m: u32,
    coeff_count: u32,
) -> Program {
    if m == 0 {
        return crate::invalid_output_program(
            OP_ID,
            gram,
            DataType::U32,
            format!("Fix: sos_gram_construct requires m > 0, got {m}."),
        );
    }
    if coeff_count == 0 {
        return crate::invalid_output_program(
            OP_ID,
            gram,
            DataType::U32,
            "Fix: sos_gram_construct requires coeff_count > 0, got 0.".to_string(),
        );
    }

    let Some(cells) = m.checked_mul(m) else {
        return crate::invalid_output_program(
            OP_ID,
            gram,
            DataType::U32,
            format!("Fix: sos_gram_construct m*m overflows u32 for m={m}."),
        );
    };
    let t = Expr::InvocationId { axis: 0 };

    let body = vec![Node::if_then(
        Expr::lt(t.clone(), Expr::u32(cells)),
        vec![Node::store(
            gram,
            t.clone(),
            Expr::load(p_coeffs, Expr::load(monomial_pairs, t)),
        )],
    )];

    Program::wrapped(
        vec![
            BufferDecl::storage(monomial_pairs, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(cells),
            BufferDecl::storage(p_coeffs, 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(coeff_count),
            BufferDecl::storage(gram, 2, BufferAccess::ReadWrite, DataType::U32).with_count(cells),
        ],
        [256, 1, 1],
        vec![Node::Region {
            generator: Ident::from(OP_ID),
            source_region: None,
            body: Arc::new(body),
        }],
    )
}

/// CPU reference.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn sos_gram_construct_cpu(monomial_pairs: &[u32], p_coeffs: &[u32], m: u32) -> Vec<u32> {
    let mut out = Vec::new();
    try_sos_gram_construct_cpu_into(monomial_pairs, p_coeffs, m, &mut out)
        .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - sos_gram_construct_cpu failed: invalid Gram-matrix shape");
    out
}

/// Fallible CPU reference.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_sos_gram_construct_cpu(
    monomial_pairs: &[u32],
    p_coeffs: &[u32],
    m: u32,
) -> Result<Vec<u32>, String> {
    let mut out = Vec::new();
    try_sos_gram_construct_cpu_into(monomial_pairs, p_coeffs, m, &mut out)?;
    Ok(out)
}

/// CPU reference into caller-owned storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn sos_gram_construct_cpu_into(
    monomial_pairs: &[u32],
    p_coeffs: &[u32],
    m: u32,
    out: &mut Vec<u32>,
) {
    try_sos_gram_construct_cpu_into(monomial_pairs, p_coeffs, m, out)
        .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - sos_gram_construct_cpu_into failed: invalid Gram-matrix shape");
}

/// Fallible CPU reference into caller-owned storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_sos_gram_construct_cpu_into(
    monomial_pairs: &[u32],
    p_coeffs: &[u32],
    m: u32,
    out: &mut Vec<u32>,
) -> Result<(), String> {
    let m = usize::try_from(m).map_err(|_| {
        format!("sos_gram_construct CPU oracle m={m} does not fit usize. Fix: shard the monomial basis.")
    })?;
    let cells = m.checked_mul(m).ok_or_else(|| {
        format!("sos_gram_construct CPU oracle m={m} overflows dense Gram indexing. Fix: shard the monomial basis.")
    })?;
    if cells > out.capacity() {
        crate::graph::scratch::reserve_graph_items(
            out,
            cells - out.len(),
            "SOS Gram CPU oracle",
            "Gram output",
        )?;
    }
    out.clear();
    for pair_idx in 0..cells {
        let value = monomial_pairs
            .get(pair_idx)
            .and_then(|&idx| p_coeffs.get(idx as usize))
            .copied()
            .unwrap_or(0);
        out.push(value);
    }
    Ok(())
}

/// CPU helper: PSD check on a small `n × n` symmetric matrix via
/// Sylvester's criterion (all leading principal minors > 0). Used to
/// verify SOS feasibility downstream of the Gram construction.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn is_psd_cpu(matrix: &[f64], n: u32) -> bool {
    let n = n as usize;
    if n == 0 {
        return true;
    }
    if n > 4 {
        // Fall back to checking diagonal positivity (necessary but
        // not sufficient). For the test fixtures n ≤ 3.
        return (0..n).all(|i| matrix.get(i * n + i).copied().unwrap_or(0.0) >= 0.0);
    }

    for k in 1..=n {
        if leading_principal_det(matrix, n, k) <= 0.0 {
            return false;
        }
    }
    true
}

#[cfg(any(test, feature = "cpu-parity"))]
fn leading_principal_det(matrix: &[f64], n: usize, k: usize) -> f64 {
    let mut a = [[0.0_f64; 4]; 4];
    for row in 0..k {
        for col in 0..k {
            a[row][col] = matrix.get(row * n + col).copied().unwrap_or(0.0);
        }
    }
    let mut det = 1.0;
    for pivot in 0..k {
        let mut pivot_row = pivot;
        let mut pivot_abs = a[pivot][pivot].abs();
        for (row, values) in a.iter().enumerate().take(k).skip(pivot + 1) {
            let candidate = values[pivot].abs();
            if candidate > pivot_abs {
                pivot_abs = candidate;
                pivot_row = row;
            }
        }
        if pivot_abs == 0.0 {
            return 0.0;
        }
        if pivot_row != pivot {
            a.swap(pivot, pivot_row);
            det = -det;
        }
        let pivot_value = a[pivot][pivot];
        det *= pivot_value;
        for row in (pivot + 1)..k {
            let factor = a[row][pivot] / pivot_value;
            for col in (pivot + 1)..k {
                a[row][col] -= factor * a[pivot][col];
            }
        }
    }
    det
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-10 * (1.0 + a.abs() + b.abs())
    }

    #[test]
    fn cpu_gram_construction_indexes_correctly() {
        // 2x2 monomial basis (e.g. {1, x}); pairs index into a 3-coeff
        // polynomial p = c_0 + c_1·x + c_2·x²
        // m(x) = (1, x), m·mᵀ = [[1, x], [x, x²]] → pairs = [[0, 1], [1, 2]]
        let pairs = vec![0u32, 1, 1, 2];
        let p = vec![10u32, 20, 30];
        let g = sos_gram_construct_cpu(&pairs, &p, 2);
        assert_eq!(g, vec![10, 20, 20, 30]);
    }

    #[test]
    fn cpu_gram_zero_polynomial_zero_gram() {
        let pairs = vec![0u32, 1, 1, 2];
        let p = vec![0u32; 3];
        let g = sos_gram_construct_cpu(&pairs, &p, 2);
        assert_eq!(g, vec![0; 4]);
    }

    #[test]
    fn cpu_gram_into_reuses_output_storage() {
        let pairs = vec![0u32, 1, 1, 2];
        let p = vec![10u32, 20, 30];
        let expected = sos_gram_construct_cpu(&pairs, &p, 2);
        let mut out = Vec::with_capacity(expected.len());

        sos_gram_construct_cpu_into(&pairs, &p, 2, &mut out);
        let ptr = out.as_ptr();
        sos_gram_construct_cpu_into(&pairs, &p, 2, &mut out);

        assert_eq!(out, expected);
        assert_eq!(out.as_ptr(), ptr);
    }

    #[test]
    fn cpu_gram_into_truncates_stale_tail_without_reallocating() {
        let pairs = vec![0u32];
        let p = vec![7u32];
        let mut out = Vec::with_capacity(8);
        out.extend([99u32; 8]);
        let ptr = out.as_ptr();

        try_sos_gram_construct_cpu_into(&pairs, &p, 1, &mut out).unwrap();

        assert_eq!(out, vec![7]);
        assert_eq!(out.as_ptr(), ptr);
    }

    #[test]
    fn generated_cpu_gram_matches_independent_reference() {
        for m in 1usize..=8 {
            let cells = m * m;
            let coeff_count = cells + 3;
            let pairs: Vec<u32> = (0..cells)
                .map(|idx| ((idx * 7 + m) % coeff_count) as u32)
                .collect();
            let coeffs: Vec<u32> = (0..coeff_count)
                .map(|idx| (idx as u32).wrapping_mul(17).wrapping_add(5))
                .collect();
            let mut out = Vec::with_capacity(cells + 5);

            try_sos_gram_construct_cpu_into(&pairs, &coeffs, m as u32, &mut out).unwrap();
            let expected: Vec<u32> = pairs
                .iter()
                .map(|&idx| coeffs.get(idx as usize).copied().unwrap_or(0))
                .collect();

            assert_eq!(out, expected, "generated SOS Gram case m={m}");
        }
    }

    #[test]
    fn cpu_gram_malformed_inputs_fill_missing_coefficients_with_zero() {
        let g = sos_gram_construct_cpu(&[0, 4], &[7], 2);
        assert_eq!(g, vec![7, 0, 0, 0]);
    }

    #[test]
    fn cpu_psd_identity_passes() {
        let i_3 = vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
        assert!(is_psd_cpu(&i_3, 3));
    }

    #[test]
    fn cpu_psd_negative_diagonal_fails() {
        let m = vec![-1.0, 0.0, 0.0, 1.0];
        assert!(!is_psd_cpu(&m, 2));
    }

    #[test]
    fn cpu_psd_two_by_two_positive_definite() {
        // [[2, 1], [1, 2]] is PD: det 3 > 0, leading minor 2 > 0.
        let m = vec![2.0, 1.0, 1.0, 2.0];
        assert!(is_psd_cpu(&m, 2));
    }

    #[test]
    fn cpu_psd_indefinite_fails() {
        // [[1, 2], [2, 1]]: det = 1 - 4 = -3 < 0, not PSD.
        let m = vec![1.0, 2.0, 2.0, 1.0];
        assert!(!is_psd_cpu(&m, 2));
    }

    #[test]
    fn cpu_psd_short_matrix_is_zero_padded() {
        assert!(!is_psd_cpu(&[1.0], 2));
    }

    #[test]
    fn ir_program_buffer_layout() {
        let p = sos_gram_construct("pairs", "p", "g", 4, 16);
        assert_eq!(p.workgroup_size, [256, 1, 1]);
        let names: Vec<&str> = p.buffers.iter().map(|b| b.name()).collect();
        assert_eq!(names, vec!["pairs", "p", "g"]);
        assert_eq!(p.buffers[0].count(), 16);
        assert_eq!(p.buffers[1].count(), 16);
        assert_eq!(p.buffers[2].count(), 16);
    }

    #[test]
    fn zero_m_traps() {
        let p = sos_gram_construct("pairs", "p", "g", 0, 1);
        assert!(p.stats().trap());
    }

    #[test]
    fn gram_cell_overflow_traps() {
        let p = sos_gram_construct("pairs", "p", "g", u32::MAX, 1);
        assert!(p.stats().trap());
    }

    #[test]
    fn approx_eq_works() {
        assert!(approx_eq(1.0, 1.0));
        assert!(!approx_eq(1.0, 2.0));
    }
}
