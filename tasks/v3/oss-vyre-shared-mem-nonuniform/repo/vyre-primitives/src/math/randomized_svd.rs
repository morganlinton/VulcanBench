//! Randomized SVD primitives  -  Halko-Martinsson-Tropp 2011 algorithm.
//!
//! Randomized SVD computes a rank-`k` approximation of an `m × n`
//! matrix in `O(mnk)` time vs. classical `O(mn²)` SVD. The algorithm:
//!
//! ```text
//!   1. Y = A · Ω        (random projection, Ω is n×(k+p) Gaussian)
//!   2. Q = qr(Y).Q       (orthonormalize the column space)
//!   3. B = Qᵀ A          (project A into the small (k+p)×n basis)
//!   4. SVD on B (small)  (cheap deterministic SVD on k+p×n)
//!   5. U = Q · U_b       (lift back to m-row space)
//! ```
//!
//! Provable bounds (Theorem 10.7 of HMT): with oversampling `p ≥ 2`,
//! `||A - QQᵀA||_2 ≤ (1 + 11√(k+p)·√(min(m,n))) · σ_{k+1}`. The
//! constant 11 looks bad but is rarely tight in practice  -  randomized
//! SVD is 10-100× faster than full SVD with negligible accuracy loss.
//!
//! This file ships the **random-projection step** primitive  -  `Y = A·Ω`
//! where Ω is a random Gaussian matrix already supplied by the caller
//! (typically generated host-side from a fixed seed for reproducibility).
//! The QR and small-SVD steps compose with future Householder + small-
//! matrix-SVD primitives.
//!
//! # Why this primitive is dual-use
//!
//! | Consumer | Use |
//! |---|---|
//! | future `vyre-libs::ml::low_rank` | low-rank attention, weight compression |
//! | future `vyre-libs::sci::pca` | PCA / spectral analysis at scale |
//! | future `vyre-libs::security::anomaly` | covariance-based anomaly detection |
//! | `vyre-foundation::transform` dispatch compression | randomized SVD compresses huge low-rank dispatch dependency matrices for polyhedral fusion analysis at workspace scale |

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Op id.
pub const OP_ID: &str = "vyre-primitives::math::randomized_projection_step";

/// Emit `Y = A · Ω` where:
/// - `A` is `m × n` row-major u32.
/// - `Ω` is `n × l` row-major u32 (l = k + oversample, typical p = 5 to 10).
/// - `Y` is `m × l` row-major u32.
///
/// This is one matrix-matrix multiply, isomorphic to a single
/// [`crate::math::semiring_gemm`] call. Shipped as a focused
/// primitive so randomized-SVD region-chains read clearly.
#[must_use]
pub fn randomized_projection_step(
    a: &str,
    omega: &str,
    y: &str,
    m: u32,
    n: u32,
    l: u32,
) -> Program {
    match try_randomized_projection_step(a, omega, y, m, n, l) {
        Ok(program) => program,
        Err(error) => crate::invalid_output_program(OP_ID, y, DataType::U32, error),
    }
}

/// Emit `Y = A · Ω` with checked matrix dimensions.
pub fn try_randomized_projection_step(
    a: &str,
    omega: &str,
    y: &str,
    m: u32,
    n: u32,
    l: u32,
) -> Result<Program, String> {
    if m == 0 {
        return Err("Fix: randomized_projection_step requires m > 0, got 0.".to_string());
    }
    if n == 0 {
        return Err("Fix: randomized_projection_step requires n > 0, got 0.".to_string());
    }
    if l == 0 {
        return Err("Fix: randomized_projection_step requires l > 0, got 0.".to_string());
    }

    let a_cells = checked_randomized_svd_cells("A input", m, n)?;
    let omega_cells = checked_randomized_svd_cells("omega input", n, l)?;
    let cells = checked_randomized_svd_cells("projection output", m, l)?;
    let t = Expr::InvocationId { axis: 0 };

    // i = t / l, j = t % l
    let i_expr = Expr::div(t.clone(), Expr::u32(l));
    let j_expr = Expr::rem(t.clone(), Expr::u32(l));

    let body = vec![Node::if_then(
        Expr::lt(t.clone(), Expr::u32(cells)),
        vec![
            Node::let_bind("i", i_expr),
            Node::let_bind("j", j_expr),
            Node::let_bind("acc", Expr::u32(0)),
            Node::loop_for(
                "k",
                Expr::u32(0),
                Expr::u32(n),
                vec![Node::assign(
                    "acc",
                    Expr::add(
                        Expr::var("acc"),
                        crate::fixed_mul_16_16_expr(
                            Expr::load(
                                a,
                                Expr::add(Expr::mul(Expr::var("i"), Expr::u32(n)), Expr::var("k")),
                            ),
                            Expr::load(
                                omega,
                                Expr::add(Expr::mul(Expr::var("k"), Expr::u32(l)), Expr::var("j")),
                            ),
                        ),
                    ),
                )],
            ),
            Node::store(y, t, Expr::var("acc")),
        ],
    )];

    Ok(Program::wrapped(
        vec![
            BufferDecl::storage(a, 0, BufferAccess::ReadOnly, DataType::U32).with_count(a_cells),
            BufferDecl::storage(omega, 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(omega_cells),
            BufferDecl::storage(y, 2, BufferAccess::ReadWrite, DataType::U32).with_count(cells),
        ],
        [256, 1, 1],
        vec![Node::Region {
            generator: Ident::from(OP_ID),
            source_region: None,
            body: Arc::new(body),
        }],
    ))
}

fn checked_randomized_svd_cells(context: &str, lhs: u32, rhs: u32) -> Result<u32, String> {
    lhs.checked_mul(rhs).ok_or_else(|| {
        format!(
            "randomized_projection_step {context} shape {lhs}x{rhs} overflows cell count. Fix: shard the randomized SVD matrix before GPU dispatch."
        )
    })
}

/// CPU reference: `Y = A · Ω` in f64.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn randomized_projection_step_cpu(
    a: &[f64],
    omega: &[f64],
    m: u32,
    n: u32,
    l: u32,
) -> Vec<f64> {
    try_randomized_projection_step_cpu(a, omega, m, n, l).unwrap_or_else(|error| panic!("{error}"))
}

/// Fallible CPU reference: `Y = A · Ω` in f64.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_randomized_projection_step_cpu(
    a: &[f64],
    omega: &[f64],
    m: u32,
    n: u32,
    l: u32,
) -> Result<Vec<f64>, String> {
    let mut y = Vec::new();
    try_randomized_projection_step_cpu_into(a, omega, m, n, l, &mut y)?;
    Ok(y)
}

/// CPU reference: `Y = A · Ω` in caller-owned storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn randomized_projection_step_cpu_into(
    a: &[f64],
    omega: &[f64],
    m: u32,
    n: u32,
    l: u32,
    y: &mut Vec<f64>,
) {
    try_randomized_projection_step_cpu_into(a, omega, m, n, l, y)
        .unwrap_or_else(|error| panic!("{error}"));
}

/// Fallible CPU reference: `Y = A · Ω` in caller-owned storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_randomized_projection_step_cpu_into(
    a: &[f64],
    omega: &[f64],
    m: u32,
    n: u32,
    l: u32,
    y: &mut Vec<f64>,
) -> Result<(), String> {
    let m = m as usize;
    let n = n as usize;
    let l = l as usize;
    m.checked_mul(n).ok_or_else(|| {
        format!(
            "randomized_projection_step CPU oracle A shape {m}x{n} overflows indexing. Fix: shard the randomized SVD matrix before parity evaluation."
        )
    })?;
    n.checked_mul(l).ok_or_else(|| {
        format!(
            "randomized_projection_step CPU oracle omega shape {n}x{l} overflows indexing. Fix: shard the randomized SVD matrix before parity evaluation."
        )
    })?;
    let out_cells = m.checked_mul(l).ok_or_else(|| {
        format!(
            "randomized_projection_step CPU oracle output shape {m}x{l} overflows indexing. Fix: shard the randomized SVD matrix before parity evaluation."
        )
    })?;
    if out_cells > y.capacity() {
        crate::graph::scratch::reserve_graph_items(
            y,
            out_cells - y.len(),
            "randomized SVD CPU oracle",
            "randomized_projection_step output",
        )?;
    }
    y.clear();
    y.resize(out_cells, 0.0);
    for i in 0..m {
        for j in 0..l {
            let mut acc = 0.0;
            for k in 0..n {
                let a_value = a.get(i * n + k).copied().unwrap_or(0.0);
                let omega_value = omega.get(k * l + j).copied().unwrap_or(0.0);
                acc += a_value * omega_value;
            }
            y[i * l + j] = acc;
        }
    }
    Ok(())
}

/// Modified Gram-Schmidt orthonormalization (CPU-only convenience for
/// the QR step). Operates on `m × l` matrix Y in-place, returns Q
/// (same shape, columns orthonormal).
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn modified_gram_schmidt_cpu(y: &[f64], m: u32, l: u32) -> Vec<f64> {
    try_modified_gram_schmidt_cpu(y, m, l).unwrap_or_else(|error| panic!("{error}"))
}

/// Fallible Modified Gram-Schmidt orthonormalization.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_modified_gram_schmidt_cpu(y: &[f64], m: u32, l: u32) -> Result<Vec<f64>, String> {
    let mut q = Vec::new();
    try_modified_gram_schmidt_cpu_into(y, m, l, &mut q)?;
    Ok(q)
}

/// Modified Gram-Schmidt into caller-owned storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn modified_gram_schmidt_cpu_into(y: &[f64], m: u32, l: u32, q: &mut Vec<f64>) {
    try_modified_gram_schmidt_cpu_into(y, m, l, q).unwrap_or_else(|error| panic!("{error}"));
}

/// Fallible Modified Gram-Schmidt into caller-owned storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_modified_gram_schmidt_cpu_into(
    y: &[f64],
    m: u32,
    l: u32,
    q: &mut Vec<f64>,
) -> Result<(), String> {
    let m = m as usize;
    let l = l as usize;
    let cells = m.checked_mul(l).ok_or_else(|| {
        format!(
            "modified_gram_schmidt CPU oracle shape {m}x{l} overflows indexing. Fix: shard the randomized SVD basis before parity evaluation."
        )
    })?;
    if cells > q.capacity() {
        crate::graph::scratch::reserve_graph_items(
            q,
            cells - q.len(),
            "randomized SVD CPU oracle",
            "modified_gram_schmidt output",
        )?;
    }
    q.clear();
    q.extend((0..cells).map(|idx| y.get(idx).copied().unwrap_or(0.0)));
    for j in 0..l {
        // Norm of column j
        let mut sq = 0.0;
        for i in 0..m {
            sq += q[i * l + j] * q[i * l + j];
        }
        let nrm = sq.sqrt().max(1e-30);
        for i in 0..m {
            q[i * l + j] /= nrm;
        }
        // Orthogonalize remaining columns against j.
        for jj in (j + 1)..l {
            let mut dot = 0.0;
            for i in 0..m {
                dot += q[i * l + j] * q[i * l + jj];
            }
            for i in 0..m {
                q[i * l + jj] -= dot * q[i * l + j];
            }
        }
    }
    Ok(())
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || randomized_projection_step("a", "omega", "y", 1, 2, 2),
        Some(|| {
            vec![vec![
                crate::wire::pack_u32_slice(&[2u32 << 16, 3u32 << 16]),
                crate::wire::pack_u32_slice(&[1u32 << 16, 0, 0, 1u32 << 16]),
                crate::wire::pack_u32_slice(&[0, 0]),
            ]]
        }),
        Some(|| {
            vec![vec![crate::wire::pack_u32_slice(&[
                2u32 << 16,
                3u32 << 16,
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
    fn cpu_projection_identity_omega_passthrough() {
        // m = n = 2, A = identity, Ω = identity → Y = A.
        let a = vec![1.0, 0.0, 0.0, 1.0];
        let omega = vec![1.0, 0.0, 0.0, 1.0];
        let y = randomized_projection_step_cpu(&a, &omega, 2, 2, 2);
        assert_eq!(y, a);
    }

    #[test]
    fn cpu_projection_zero_omega_zeros_out() {
        let a = vec![1.0, 2.0, 3.0, 4.0];
        let omega = vec![0.0; 4];
        let y = randomized_projection_step_cpu(&a, &omega, 2, 2, 2);
        for v in y {
            assert!(approx_eq(v, 0.0));
        }
    }

    #[test]
    fn cpu_projection_correct_shape_for_rectangular_a() {
        // m=3, n=4, l=2. Y should be 3x2.
        let a: Vec<f64> = (0..12).map(|i| i as f64).collect();
        let omega: Vec<f64> = (0..8).map(|i| (i % 2) as f64).collect();
        let y = randomized_projection_step_cpu(&a, &omega, 3, 4, 2);
        assert_eq!(y.len(), 6);
    }

    #[test]
    fn cpu_projection_into_reuses_output_storage() {
        let a: Vec<f64> = (0..12).map(|i| i as f64).collect();
        let omega: Vec<f64> = (0..8).map(|i| (i % 3) as f64).collect();
        let expected = randomized_projection_step_cpu(&a, &omega, 3, 4, 2);
        let mut y = Vec::with_capacity(expected.len());
        y.extend_from_slice(&[99.0, 98.0, 97.0, 96.0, 95.0, 94.0]);

        randomized_projection_step_cpu_into(&a, &omega, 3, 4, 2, &mut y);
        let ptr = y.as_ptr();
        let capacity = y.capacity();
        randomized_projection_step_cpu_into(&a, &omega, 3, 4, 2, &mut y);

        assert_eq!(y, expected);
        assert_eq!(y.as_ptr(), ptr);
        assert_eq!(y.capacity(), capacity);

        randomized_projection_step_cpu_into(&[2.0], &[3.0], 1, 1, 1, &mut y);
        assert_eq!(y, vec![6.0]);
        assert_eq!(y.as_ptr(), ptr);
        assert_eq!(y.capacity(), capacity);
    }

    #[test]
    fn cpu_modified_gram_schmidt_columns_orthonormal() {
        // Random-ish 3x2 matrix.
        let y = vec![1.0, 0.5, 0.3, 0.9, 0.7, 0.2];
        let q = modified_gram_schmidt_cpu(&y, 3, 2);
        // Column 0 norm = 1, column 1 norm = 1, dot(c0, c1) = 0.
        let n0_sq: f64 = (0..3).map(|i| q[i * 2] * q[i * 2]).sum();
        let n1_sq: f64 = (0..3).map(|i| q[i * 2 + 1] * q[i * 2 + 1]).sum();
        let dot: f64 = (0..3).map(|i| q[i * 2] * q[i * 2 + 1]).sum();
        assert!(approx_eq(n0_sq, 1.0));
        assert!(approx_eq(n1_sq, 1.0));
        assert!(approx_eq(dot, 0.0));
    }

    #[test]
    fn cpu_modified_gram_schmidt_into_reuses_output_storage() {
        let y = vec![1.0, 0.5, 0.3, 0.9, 0.7, 0.2];
        let expected = modified_gram_schmidt_cpu(&y, 3, 2);
        let mut q = Vec::with_capacity(expected.len());
        q.extend_from_slice(&[99.0, 98.0, 97.0, 96.0, 95.0, 94.0]);

        modified_gram_schmidt_cpu_into(&y, 3, 2, &mut q);
        let ptr = q.as_ptr();
        let capacity = q.capacity();
        modified_gram_schmidt_cpu_into(&y, 3, 2, &mut q);

        assert_eq!(q, expected);
        assert_eq!(q.as_ptr(), ptr);
        assert_eq!(q.capacity(), capacity);

        modified_gram_schmidt_cpu_into(&[4.0], 1, 1, &mut q);
        assert_eq!(q, vec![1.0]);
        assert_eq!(q.as_ptr(), ptr);
        assert_eq!(q.capacity(), capacity);
    }

    #[test]
    fn generated_projection_cpu_matches_independent_reference() {
        let mut state = 0x5A17_1234_u32;
        for case in 0..1024usize {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let m = (state % 9 + 1) as usize;
            let n = (state.rotate_left(5) % 11 + 1) as usize;
            let l = (state.rotate_left(9) % 7 + 1) as usize;
            let a_len = (case * 7) % (m * n + 1);
            let omega_len = (case * 11) % (n * l + 1);
            let a: Vec<f64> = (0..a_len)
                .map(|idx| ((idx * 13 + case) % 31) as f64 / 7.0 - 2.0)
                .collect();
            let omega: Vec<f64> = (0..omega_len)
                .map(|idx| ((idx * 17 + case) % 29) as f64 / 5.0 - 3.0)
                .collect();
            let actual =
                try_randomized_projection_step_cpu(&a, &omega, m as u32, n as u32, l as u32)
                    .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - generated randomized projection should evaluate");
            let expected = independent_projection(&a, &omega, m, n, l);

            assert_eq!(actual.len(), m * l, "case {case}: output shape");
            for idx in 0..actual.len() {
                assert!(
                    approx_eq(actual[idx], expected[idx]),
                    "case {case} idx {idx}: expected {}, got {}",
                    expected[idx],
                    actual[idx]
                );
            }
        }
    }

    fn independent_projection(a: &[f64], omega: &[f64], m: usize, n: usize, l: usize) -> Vec<f64> {
        let mut out = vec![0.0; m * l];
        for i in 0..m {
            for j in 0..l {
                for k in 0..n {
                    out[i * l + j] += a.get(i * n + k).copied().unwrap_or(0.0)
                        * omega.get(k * l + j).copied().unwrap_or(0.0);
                }
            }
        }
        out
    }

    #[test]
    fn ir_program_buffer_layout() {
        let p = randomized_projection_step("A", "O", "Y", 8, 4, 3);
        assert_eq!(p.workgroup_size, [256, 1, 1]);
        let names: Vec<&str> = p.buffers.iter().map(|b| b.name()).collect();
        assert_eq!(names, vec!["A", "O", "Y"]);
        assert_eq!(p.buffers[0].count(), 32);
        assert_eq!(p.buffers[1].count(), 12);
        assert_eq!(p.buffers[2].count(), 24);
    }

    #[test]
    fn zero_m_traps() {
        let p = randomized_projection_step("A", "O", "Y", 0, 1, 1);
        assert!(p.stats().trap());
    }

    #[test]
    fn checked_builder_rejects_projection_cell_overflow() {
        let error = try_randomized_projection_step("A", "O", "Y", u32::MAX, 1, 2)
            .expect_err("checked randomized projection builder must reject m*l overflow");

        assert!(
            error.contains("overflows cell count"),
            "error should describe projection shape overflow: {error}"
        );
    }
}
