//! Quantum singular value transform (classical)  -  block-encoded matrix
//! function via Chebyshev polynomial of singular values (#34).
//!
//! QSVT (Gilyen-Su-Low-Wiebe 2018) gives a unified framework for
//! matrix functions: inverse, sqrt, exp, all without eigendecomposition.
//! The classical "dequantized" form (Tang 2019) computes
//! `f(A) · v` via:
//!
//! ```text
//!   f(A) · v ≈ Σ_k c_k T_k(A/||A||) · v
//! ```
//!
//! where `T_k` are Chebyshev polynomials of the first kind. Composes
//! with #5 chebyshev_filter (already on graph Laplacians)  -  same
//! recurrence, applied here to a generic matrix.
//!
//! This file ships the **block-encoding scaling step** primitive  -
//! given matrix `A` and Frobenius norm `||A||`, produce the scaled
//! `A / ||A||` whose singular values lie in `[0, 1]`. Caller composes
//! with #5 chebyshev_filter and a coefficient buffer to evaluate
//! `f(A) · v` for any analytic `f`.
//!
//! # Why this primitive is dual-use
//!
//! | Consumer | Use |
//! |---|---|
//! | future `vyre-libs::math::matrix_function` | unified matrix-function family |
//! | future `vyre-libs::sci::quantum_sim` | classical simulation of quantum circuits |
//! | `vyre-foundation::transform` Wasserstein dispatch analysis | matrix-function evaluation (matrix log, exp) for transport-based fusion-cost analyses |

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Op id.
pub const OP_ID: &str = "vyre-primitives::math::qsvt_block_encode";

/// Emit `A_scaled[i, j] = A[i, j] / norm` for `n × n` matrix `A`.
///
/// The norm is supplied as a single-element u32 buffer in 16.16 fp;
/// caller precomputes (typically as Frobenius norm via reduce::sum
/// then sqrt). After scaling, `A_scaled` has spectral norm `≤ 1`, the
/// requirement for QSVT block encoding.
#[must_use]
pub fn qsvt_block_encode(a: &str, norm: &str, a_scaled: &str, n: u32) -> Program {
    match try_qsvt_block_encode(a, norm, a_scaled, n) {
        Ok(program) => program,
        Err(error) => crate::invalid_output_program(OP_ID, a_scaled, DataType::U32, error),
    }
}

/// Emit `A_scaled[i, j] = A[i, j] / norm` with checked dense matrix sizing.
pub fn try_qsvt_block_encode(
    a: &str,
    norm: &str,
    a_scaled: &str,
    n: u32,
) -> Result<Program, String> {
    if n == 0 {
        return Err(format!("Fix: qsvt_block_encode requires n > 0, got {n}."));
    }

    let cells = checked_qsvt_cells(n)?;
    let t = Expr::InvocationId { axis: 0 };
    let n_v = Expr::load(norm, Expr::u32(0));
    let safe_norm = Expr::select(Expr::eq(n_v.clone(), Expr::u32(0)), Expr::u32(1), n_v);
    let value = Expr::div(
        Expr::shl(Expr::load(a, t.clone()), Expr::u32(16)),
        safe_norm,
    );

    let body = vec![Node::if_then(
        Expr::lt(t.clone(), Expr::u32(cells)),
        vec![Node::store(a_scaled, t, value)],
    )];

    Ok(Program::wrapped(
        vec![
            BufferDecl::storage(a, 0, BufferAccess::ReadOnly, DataType::U32).with_count(cells),
            BufferDecl::storage(norm, 1, BufferAccess::ReadOnly, DataType::U32).with_count(1),
            BufferDecl::storage(a_scaled, 2, BufferAccess::ReadWrite, DataType::U32)
                .with_count(cells),
        ],
        [256, 1, 1],
        vec![Node::Region {
            generator: Ident::from(OP_ID),
            source_region: None,
            body: Arc::new(body),
        }],
    ))
}

fn checked_qsvt_cells(n: u32) -> Result<u32, String> {
    n.checked_mul(n).ok_or_else(|| {
        format!(
            "qsvt_block_encode n={n} overflows dense matrix cell count. Fix: shard the matrix before GPU dispatch."
        )
    })
}

/// CPU reference: scale `A` by `1 / ||A||_F`.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn qsvt_block_encode_cpu(a: &[f64], n: u32) -> (Vec<f64>, f64) {
    let mut out = Vec::new();
    let frob =
        try_qsvt_block_encode_cpu_into(a, n, &mut out).unwrap_or_else(|error| panic!("{error}"));
    (out, frob)
}

/// CPU reference: scale `A` by `1 / ||A||_F` using caller-owned output.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn qsvt_block_encode_cpu_into(a: &[f64], n: u32, out: &mut Vec<f64>) -> f64 {
    try_qsvt_block_encode_cpu_into(a, n, out).unwrap_or_else(|error| panic!("{error}"))
}

/// Fallible CPU reference: scale `A` by `1 / ||A||_F` using caller-owned output.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_qsvt_block_encode_cpu_into(
    a: &[f64],
    n: u32,
    out: &mut Vec<f64>,
) -> Result<f64, String> {
    let n = n as usize;
    let cells = n.checked_mul(n).ok_or_else(|| {
        format!(
            "qsvt_block_encode CPU oracle n={n} overflows dense matrix indexing. Fix: shard the matrix before parity evaluation."
        )
    })?;
    if cells > out.capacity() {
        crate::graph::scratch::reserve_graph_items(
            out,
            cells - out.len(),
            "QSVT CPU oracle",
            "qsvt_block_encode output",
        )?;
    }
    let frob: f64 = a.iter().map(|&v| v * v).sum::<f64>().sqrt();
    let safe = frob.max(1e-30);
    out.clear();
    out.extend((0..cells).map(|idx| a.get(idx).copied().unwrap_or(0.0) / safe));
    Ok(frob)
}

/// CPU reference: evaluate `f(A) · v` via Chebyshev expansion. `coeffs[k]`
/// is the k-th Chebyshev coefficient of `f` (caller computes via numerical
/// integration). Operates on already-scaled `A`.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn qsvt_apply_cpu(a_scaled: &[f64], v: &[f64], coeffs: &[f64], n: u32) -> Vec<f64> {
    let mut out = Vec::new();
    let mut t_prev = Vec::new();
    let mut t_curr = Vec::new();
    let mut t_next = Vec::new();
    qsvt_apply_cpu_into(
        a_scaled,
        v,
        coeffs,
        n,
        &mut out,
        &mut t_prev,
        &mut t_curr,
        &mut t_next,
    );
    out
}

/// Fallible CPU reference: evaluate `f(A) · v` via Chebyshev expansion.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_qsvt_apply_cpu(
    a_scaled: &[f64],
    v: &[f64],
    coeffs: &[f64],
    n: u32,
) -> Result<Vec<f64>, String> {
    let mut out = Vec::new();
    let mut t_prev = Vec::new();
    let mut t_curr = Vec::new();
    let mut t_next = Vec::new();
    try_qsvt_apply_cpu_into(
        a_scaled,
        v,
        coeffs,
        n,
        &mut out,
        &mut t_prev,
        &mut t_curr,
        &mut t_next,
    )?;
    Ok(out)
}

/// CPU reference: evaluate `f(A) · v` using caller-owned recurrence buffers.
#[allow(clippy::too_many_arguments)]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn qsvt_apply_cpu_into(
    a_scaled: &[f64],
    v: &[f64],
    coeffs: &[f64],
    n: u32,
    out: &mut Vec<f64>,
    t_prev: &mut Vec<f64>,
    t_curr: &mut Vec<f64>,
    t_next: &mut Vec<f64>,
) {
    // Clearing the buffers on failure makes a GPU-vs-CPU parity assertion pass
    // on empty==empty, silently masking a divergence (Law 10 / Law 6). Fail
    // loud; callers use try_qsvt_apply_cpu_into.
    if let Err(error) =
        try_qsvt_apply_cpu_into(a_scaled, v, coeffs, n, out, t_prev, t_curr, t_next)
    {
        panic!("vyre-primitives QSVT apply CPU reference failed: {error}");
    }
}

/// Fallible CPU reference: evaluate `f(A) · v` using caller-owned recurrence buffers.
#[allow(clippy::too_many_arguments)]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_qsvt_apply_cpu_into(
    a_scaled: &[f64],
    v: &[f64],
    coeffs: &[f64],
    n: u32,
    out: &mut Vec<f64>,
    t_prev: &mut Vec<f64>,
    t_curr: &mut Vec<f64>,
    t_next: &mut Vec<f64>,
) -> Result<(), String> {
    let n = n as usize;
    let k_steps = coeffs.len();
    let cells = n.checked_mul(n).ok_or_else(|| {
        format!(
            "qsvt_apply CPU oracle n={n} overflows dense matrix indexing. Fix: shard the matrix before parity evaluation."
        )
    })?;
    if k_steps == 0 {
        return Err("qsvt_apply CPU oracle requires at least one Chebyshev coefficient. Fix: pass c_0 or use an explicit zero output.".to_string());
    }
    if a_scaled.len() != cells {
        return Err(format!(
            "qsvt_apply CPU oracle received a_scaled_len={} for n={n}; Fix: pass exactly n*n scaled matrix cells.",
            a_scaled.len()
        ));
    }
    if v.len() != n {
        return Err(format!(
            "qsvt_apply CPU oracle received vector_len={} for n={n}; Fix: pass exactly n vector entries.",
            v.len()
        ));
    }
    reserve_qsvt_cpu_vec(out, n, "qsvt_apply output")?;
    if k_steps > 1 {
        reserve_qsvt_cpu_vec(t_prev, n, "qsvt_apply T0")?;
        reserve_qsvt_cpu_vec(t_curr, n, "qsvt_apply T1")?;
    }
    if k_steps > 2 {
        reserve_qsvt_cpu_vec(t_next, n, "qsvt_apply T_next")?;
    }

    // T_0(A) v = v
    // T_1(A) v = A v
    // T_{k+1}(A) v = 2 A T_k v - T_{k-1} v
    out.clear();
    out.extend(v.iter().map(|&xi| coeffs[0] * xi));
    if k_steps == 1 {
        t_prev.clear();
        t_curr.clear();
        t_next.clear();
        return Ok(());
    }

    t_prev.clear();
    t_prev.extend_from_slice(v);
    t_curr.clear();
    t_curr.resize(n, 0.0);
    mat_vec_into(a_scaled, t_prev, n, t_curr);
    for i in 0..n {
        out[i] += coeffs[1] * t_curr[i];
    }

    for &c_k in coeffs.iter().take(k_steps).skip(2) {
        t_next.clear();
        t_next.resize(n, 0.0);
        mat_vec_into(a_scaled, t_curr, n, t_next);
        for i in 0..n {
            t_next[i] = 2.0 * t_next[i] - t_prev[i];
        }
        for i in 0..n {
            out[i] += c_k * t_next[i];
        }
        std::mem::swap(t_prev, t_curr);
        std::mem::swap(t_curr, t_next);
    }
    Ok(())
}

#[cfg(any(test, feature = "cpu-parity"))]
fn reserve_qsvt_cpu_vec<T>(out: &mut Vec<T>, len: usize, context: &str) -> Result<(), String> {
    if len > out.capacity() {
        crate::graph::scratch::reserve_graph_items(
            out,
            len - out.len(),
            "QSVT CPU oracle",
            context,
        )?;
    }
    Ok(())
}

#[cfg(any(test, feature = "cpu-parity"))]
fn mat_vec_into(matrix: &[f64], vector: &[f64], n: usize, out: &mut [f64]) {
    for i in 0..n {
        let mut sum = 0.0;
        for j in 0..n {
            sum += matrix[i * n + j] * vector[j];
        }
        out[i] = sum;
    }
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || qsvt_block_encode("a", "norm", "a_scaled", 4),
        Some(|| {
            vec![vec![
                crate::wire::pack_u32_slice(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]),
                crate::wire::pack_u32_slice(&[1]),
                crate::wire::pack_u32_slice(&[0; 16]),
            ]]
        }),
        Some(|| {
            vec![vec![crate::wire::pack_u32_slice(&[
                65_536, 131_072, 196_608, 262_144,
                327_680, 393_216, 458_752, 524_288,
                589_824, 655_360, 720_896, 786_432,
                851_968, 917_504, 983_040, 1_048_576,
            ])]]
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6 * (1.0 + a.abs() + b.abs())
    }

    #[test]
    fn cpu_block_encode_scales_correctly() {
        let a = vec![3.0, 0.0, 0.0, 4.0]; // ||A||_F = 5
        let (scaled, norm) = qsvt_block_encode_cpu(&a, 2);
        assert!(approx_eq(norm, 5.0));
        assert!(approx_eq(scaled[0], 0.6));
        assert!(approx_eq(scaled[3], 0.8));
    }

    #[test]
    fn cpu_block_encode_short_matrix_is_zero_padded() {
        let (scaled, norm) = qsvt_block_encode_cpu(&[2.0], 2);
        assert!(approx_eq(norm, 2.0));
        assert_eq!(scaled, vec![1.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn cpu_block_encode_into_reuses_output_and_truncates_stale_tail() {
        let mut out = Vec::with_capacity(4);
        out.extend_from_slice(&[99.0, 98.0, 97.0, 96.0]);
        let ptr = out.as_ptr();
        let capacity = out.capacity();

        let norm = try_qsvt_block_encode_cpu_into(&[3.0, 0.0, 0.0, 4.0], 2, &mut out)
            .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - QSVT block encode CPU oracle should reuse caller-owned output");

        assert!(approx_eq(norm, 5.0));
        assert!(approx_eq(out[0], 0.6));
        assert!(approx_eq(out[3], 0.8));
        assert_eq!(out.as_ptr(), ptr);
        assert_eq!(out.capacity(), capacity);

        let norm = try_qsvt_block_encode_cpu_into(&[2.0], 1, &mut out)
            .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - QSVT block encode CPU oracle should truncate stale output");

        assert!(approx_eq(norm, 2.0));
        assert_eq!(out, vec![1.0]);
        assert_eq!(out.as_ptr(), ptr);
        assert_eq!(out.capacity(), capacity);
    }

    #[test]
    fn cpu_qsvt_constant_function_passes() {
        // f(A) = c · I implemented as Chebyshev coeffs = [c]
        let a = vec![0.5, 0.0, 0.0, 0.5];
        let v = vec![1.0, 1.0];
        let out = qsvt_apply_cpu(&a, &v, &[3.0], 2);
        assert!(approx_eq(out[0], 3.0));
        assert!(approx_eq(out[1], 3.0));
    }

    #[test]
    fn cpu_qsvt_linear_function_recovers_av() {
        // f(A) = A → coeffs = [0, 1]; f(A) v = A v.
        let a = vec![0.5, 0.5, 0.5, 0.5];
        let v = vec![1.0, 0.0];
        let out = qsvt_apply_cpu(&a, &v, &[0.0, 1.0], 2);
        // A v = (0.5, 0.5)
        assert!(approx_eq(out[0], 0.5));
        assert!(approx_eq(out[1], 0.5));
    }

    #[test]
    fn cpu_qsvt_into_reuses_buffers() {
        let a = vec![0.5, 0.5, 0.5, 0.5];
        let v = vec![1.0, 0.0];
        let mut out = Vec::with_capacity(8);
        let mut prev = Vec::with_capacity(8);
        let mut curr = Vec::with_capacity(8);
        let mut next = Vec::with_capacity(8);
        out.extend_from_slice(&[99.0; 8]);
        prev.extend_from_slice(&[89.0; 8]);
        curr.extend_from_slice(&[79.0; 8]);
        next.extend_from_slice(&[69.0; 8]);
        let pointers = [out.as_ptr(), prev.as_ptr(), curr.as_ptr(), next.as_ptr()];
        let capacities = [
            out.capacity(),
            prev.capacity(),
            curr.capacity(),
            next.capacity(),
        ];
        qsvt_apply_cpu_into(
            &a,
            &v,
            &[0.0, 1.0],
            2,
            &mut out,
            &mut prev,
            &mut curr,
            &mut next,
        );
        assert!(approx_eq(out[0], 0.5));
        assert!(approx_eq(out[1], 0.5));
        for ptr in [out.as_ptr(), prev.as_ptr(), curr.as_ptr(), next.as_ptr()] {
            assert!(pointers.contains(&ptr));
        }
        assert_eq!(
            capacities,
            [
                out.capacity(),
                prev.capacity(),
                curr.capacity(),
                next.capacity()
            ]
        );

        try_qsvt_apply_cpu_into(
            &[1.0],
            &[3.0],
            &[2.0],
            1,
            &mut out,
            &mut prev,
            &mut curr,
            &mut next,
        )
        .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - QSVT apply CPU oracle should truncate stale output");

        assert_eq!(out, vec![6.0]);
        assert!(prev.is_empty());
        assert!(curr.is_empty());
        assert!(next.is_empty());
        assert_eq!(out.as_ptr(), pointers[0]);
        assert_eq!(out.capacity(), capacities[0]);
    }

    #[test]
    fn cpu_qsvt_zero_signal_zero_output() {
        let a = vec![1.0, 0.0, 0.0, 1.0];
        let v = vec![0.0; 2];
        let out = qsvt_apply_cpu(&a, &v, &[1.0, 0.5, 0.25], 2);
        assert!(approx_eq(out[0], 0.0));
        assert!(approx_eq(out[1], 0.0));
    }

    #[test]
    fn try_qsvt_apply_rejects_bad_shape_without_clobbering_buffers() {
        let mut out = vec![1.0, 2.0];
        let mut prev = vec![3.0];
        let mut curr = vec![4.0];
        let mut next = vec![5.0];

        let err = try_qsvt_apply_cpu_into(
            &[1.0],
            &[1.0, 2.0],
            &[1.0],
            2,
            &mut out,
            &mut prev,
            &mut curr,
            &mut next,
        )
        .expect_err("checked QSVT apply must reject malformed matrix shape");

        assert!(err.contains("a_scaled_len"));
        assert_eq!(out, vec![1.0, 2.0]);
        assert_eq!(prev, vec![3.0]);
        assert_eq!(curr, vec![4.0]);
        assert_eq!(next, vec![5.0]);
    }

    #[test]
    fn generated_qsvt_apply_matches_independent_chebyshev_reference() {
        let mut out = Vec::new();
        let mut prev = Vec::new();
        let mut curr = Vec::new();
        let mut next = Vec::new();
        for case in 0..1024usize {
            let n = case % 5 + 1;
            let coeff_len = case % 6 + 1;
            let a_scaled: Vec<f64> = (0..(n * n))
                .map(|idx| ((idx * 13 + case) % 19) as f64 / 23.0 - 0.4)
                .collect();
            let v: Vec<f64> = (0..n)
                .map(|idx| ((idx * 17 + case) % 29) as f64 / 11.0 - 1.0)
                .collect();
            let coeffs: Vec<f64> = (0..coeff_len)
                .map(|idx| ((idx * 7 + case) % 17) as f64 / 13.0 - 0.5)
                .collect();

            try_qsvt_apply_cpu_into(
                &a_scaled, &v, &coeffs, n as u32, &mut out, &mut prev, &mut curr, &mut next,
            )
            .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - generated QSVT apply CPU oracle should evaluate");
            let expected = independent_qsvt_apply(&a_scaled, &v, &coeffs, n);

            assert_eq!(out.len(), n, "case {case}: output length");
            for idx in 0..n {
                assert!(
                    approx_eq(out[idx], expected[idx]),
                    "case {case} idx {idx}: expected {}, got {}",
                    expected[idx],
                    out[idx]
                );
            }
        }
    }

    fn independent_qsvt_apply(a_scaled: &[f64], v: &[f64], coeffs: &[f64], n: usize) -> Vec<f64> {
        let mut out: Vec<f64> = v.iter().map(|&xi| coeffs[0] * xi).collect();
        if coeffs.len() == 1 {
            return out;
        }
        let mut prev = v.to_vec();
        let mut curr = vec![0.0; n];
        mat_vec_into(a_scaled, &prev, n, &mut curr);
        for i in 0..n {
            out[i] += coeffs[1] * curr[i];
        }
        for &coeff in coeffs.iter().skip(2) {
            let mut next = vec![0.0; n];
            mat_vec_into(a_scaled, &curr, n, &mut next);
            for i in 0..n {
                next[i] = 2.0 * next[i] - prev[i];
                out[i] += coeff * next[i];
            }
            prev = curr;
            curr = next;
        }
        out
    }

    #[test]
    fn ir_program_buffer_layout() {
        let p = qsvt_block_encode("A", "n", "As", 4);
        assert_eq!(p.workgroup_size, [256, 1, 1]);
        assert_eq!(p.buffers[0].count(), 16);
        assert_eq!(p.buffers[1].count(), 1);
        assert_eq!(p.buffers[2].count(), 16);
    }

    #[test]
    fn zero_n_traps() {
        let p = qsvt_block_encode("A", "n", "As", 0);
        assert!(p.stats().trap());
    }

    #[test]
    fn checked_builder_rejects_dense_matrix_overflow() {
        let error = try_qsvt_block_encode("A", "n", "As", u32::MAX)
            .expect_err("checked QSVT builder must reject n*n overflow");

        assert!(
            error.contains("overflows dense matrix cell count"),
            "error should describe dense matrix overflow: {error}"
        );
    }
}
