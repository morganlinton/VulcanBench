//! Information-geometric preconditioner primitives  -  Newton-Schulz
//! matrix inverse-square-root (Shampoo / Sophia core kernel).
//!
//! KFAC (Martens 2015), Shampoo (Gupta 2018), Sophia (Liu 2024)
//! preconditioned optimizers all need `M^{-1/2}` for some block-
//! diagonal Fisher-style matrix `M`. Newton-Schulz iteration
//! computes it without an SVD:
//!
//! ```text
//!   Y_0 = M / ||M||           (normalize so spectrum lies in [0, 1])
//!   Y_{k+1} = (1/2) Y_k (3·I - Y_k² Y_k^{-1?})  (one variant)
//! ```
//!
//! This file ships the **Newton-Schulz iteration step** primitive that
//! takes the current iterate and one matmul output and emits the next
//! iterate. The matrix-matrix multiplies inside the iteration are
//! [`crate::math::semiring_gemm`] composed by the caller.
//!
//! # Why this primitive is dual-use
//!
//! | Consumer | Use |
//! |---|---|
//! | future `vyre-libs::optim::shampoo` | Shampoo / Sophia preconditioned SGD |
//! | future `vyre-libs::optim::kfac` | K-FAC natural gradient |
//! | future `vyre-libs::math::matrix_function` | general matrix-function family (sqrt, inv-sqrt, log, exp via QSVT  -  composes with #34) |
//!
//! Self-consumer is currently weak; revisit once optimizer-aware
//! dispatch scheduling lands (#22 megakernel auto-scheduler may use
//! preconditioned SGD on its own ILP relaxation).
//!
//! # Newton-Schulz variant
//!
//! For the inverse square root `Y = M^{-1/2}`, the standard variant is
//! the **coupled iteration**:
//!
//! ```text
//!   Y_{k+1} = (1/2) Y_k (3·I - Z_k Y_k)
//!   Z_{k+1} = (1/2) (3·I - Z_k Y_k) Z_k
//!   Z_0 = M / ||M||,  Y_0 = I / sqrt(||M||)
//! ```
//!
//! converging to `Z_k → M / sqrt(M·M) = sqrt(M)/||M||·M = sqrt(M^2)/||M||`
//! and `Y_k → 1/sqrt(M)·sqrt(||M||)`. After `k` iterations, the caller
//! rescales by the saved norm to recover `M^{-1/2}`.
//!
//! This file ships the **Y update step**: given `(Y_k, Z_k Y_k)`,
//! emit `Y_{k+1}`. The matrix product `Z_k Y_k` is the caller's job
//! (one `semiring_gemm` dispatch per step).

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Op id.
pub const OP_ID: &str = "vyre-primitives::math::newton_schulz_y_step";
/// Primitive op id for the fused f32 Newton-Schulz scalar polynomial.
pub const POLY5_F32_OP_ID: &str = "vyre-primitives::math::newton_schulz_poly5_f32";

/// Emit `y_next = (1/2) y_curr (3·I - zy)` per cell.
///
/// Inputs:
/// - `y_curr`: `n × n` u32 buffer (current Y_k iterate, 16.16 fp).
/// - `zy`: `n × n` u32 buffer = `Z_k · Y_k` (caller-precomputed via
///   one `semiring_gemm`).
///
/// Output:
/// - `y_next`: `n × n` u32 buffer.
///
/// Computation per cell `(i, j)`:
///   `y_next[i,j] = 0.5 · (3 · y_curr[i,j] - Σ_k y_curr[i,k] · zy[k,j])`
///
/// Wait  -  `Y · (3·I - Z·Y)` involves another matmul. Decomposing:
///   `Y · (3·I - Z·Y) = 3·Y - Y·Z·Y`
///
/// The caller pre-computes `YZY = Y·Z·Y` via TWO `semiring_gemm`s
/// (Y·Z then result·Y) and supplies the full `n × n` buffer. This
/// primitive then does the cheap elementwise `0.5 (3·Y - YZY)`.
#[must_use]
pub fn newton_schulz_y_step(y_curr: &str, yzy: &str, y_next: &str, n: u32) -> Program {
    if n == 0 {
        return crate::invalid_output_program(
            OP_ID,
            y_next,
            DataType::U32,
            format!("Fix: newton_schulz_y_step requires n > 0, got {n}."),
        );
    }

    let cells = n * n;
    let t = Expr::InvocationId { axis: 0 };

    // value = (3 * y_curr[t] - yzy[t]) / 2
    let three_y = Expr::mul(Expr::u32(3), Expr::load(y_curr, t.clone()));
    let diff = Expr::sub(three_y, Expr::load(yzy, t.clone()));
    let half = Expr::shr(diff, Expr::u32(1));

    let body = vec![Node::if_then(
        Expr::lt(t.clone(), Expr::u32(cells)),
        vec![Node::store(y_next, t, half)],
    )];

    Program::wrapped(
        vec![
            BufferDecl::storage(y_curr, 0, BufferAccess::ReadOnly, DataType::U32).with_count(cells),
            BufferDecl::storage(yzy, 1, BufferAccess::ReadOnly, DataType::U32).with_count(cells),
            BufferDecl::storage(y_next, 2, BufferAccess::ReadWrite, DataType::U32)
                .with_count(cells),
        ],
        [256, 1, 1],
        vec![Node::Region {
            generator: Ident::from(OP_ID),
            source_region: None,
            body: Arc::new(body),
        }],
    )
}

/// Emit the fused f32 Newton-Schulz quintic polynomial for each matrix cell.
#[must_use]
pub fn newton_schulz_poly5_f32(mat: &str, output: &str, rows: u32, cols: u32) -> Program {
    let total = rows * cols;
    let i = Expr::var("i");
    let mut iter_body = vec![Node::let_bind("x0", Expr::load(mat, i.clone()))];
    for step in 0..5 {
        let x = Expr::var(format!("x{step}"));
        let x2 = format!("x{step}_2");
        let x3 = format!("x{step}_3");
        let x5 = format!("x{step}_5");
        let next = format!("x{}", step + 1);
        iter_body.push(Node::let_bind(&x2, Expr::mul(x.clone(), x.clone())));
        iter_body.push(Node::let_bind(&x3, Expr::mul(Expr::var(&x2), x.clone())));
        iter_body.push(Node::let_bind(
            &x5,
            Expr::mul(Expr::var(&x3), Expr::var(&x2)),
        ));
        iter_body.push(Node::let_bind(
            &next,
            Expr::add(
                Expr::add(
                    Expr::mul(Expr::f32(3.4445), x),
                    Expr::mul(Expr::f32(-4.7750), Expr::var(&x3)),
                ),
                Expr::mul(Expr::f32(2.0315), Expr::var(&x5)),
            ),
        ));
    }
    iter_body.push(Node::Store {
        buffer: output.into(),
        index: i.clone(),
        value: Expr::var("x5"),
    });

    let body = vec![
        Node::let_bind("i", Expr::InvocationId { axis: 0 }),
        Node::if_then(Expr::lt(i.clone(), Expr::u32(total)), iter_body),
    ];

    Program::wrapped(
        vec![
            BufferDecl::storage(mat, 0, BufferAccess::ReadOnly, DataType::F32).with_count(total),
            BufferDecl::output(output, 1, DataType::F32).with_count(total),
        ],
        [64, 1, 1],
        vec![Node::Region {
            generator: Ident::from(POLY5_F32_OP_ID),
            source_region: None,
            body: Arc::new(body),
        }],
    )
}

#[cfg(feature = "inventory-registry")]
fn fixture_f32(values: &[f32]) -> Vec<u8> {
    crate::wire::pack_f32_slice(values)
}

#[cfg(feature = "inventory-registry")]
fn poly5_fixture_expected(values: &[f32]) -> Vec<f32> {
    values
        .iter()
        .copied()
        .map(|mut x| {
            for _ in 0..5 {
                let x2 = x * x;
                let x3 = x2 * x;
                let x5 = x3 * x2;
                x = 3.4445 * x + -4.7750 * x3 + 2.0315 * x5;
            }
            x
        })
        .collect()
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        POLY5_F32_OP_ID,
        || newton_schulz_poly5_f32("mat", "output", 2, 2),
        Some(|| vec![vec![
            fixture_f32(&[0.25, 0.5, 0.75, 1.0]),
            fixture_f32(&[0.0; 4]),
        ]]),
        Some(|| {
            let expected = poly5_fixture_expected(&[0.25, 0.5, 0.75, 1.0]);
            vec![vec![fixture_f32(&expected)]]
        }),
    )
}

/// CPU reference: one Newton-Schulz Y step. f64 for clarity.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn newton_schulz_y_step_cpu(y_curr: &[f64], yzy: &[f64]) -> Vec<f64> {
    let mut out = Vec::new();
    try_newton_schulz_y_step_cpu_into(y_curr, yzy, &mut out)
        .unwrap_or_else(|error| panic!("{error}"));
    out
}

/// CPU reference: one Newton-Schulz Y step into caller-owned storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn newton_schulz_y_step_cpu_into(y_curr: &[f64], yzy: &[f64], out: &mut Vec<f64>) {
    try_newton_schulz_y_step_cpu_into(y_curr, yzy, out).unwrap_or_else(|error| panic!("{error}"));
}

/// Fallible CPU reference: one Newton-Schulz Y step into caller-owned storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_newton_schulz_y_step_cpu_into(
    y_curr: &[f64],
    yzy: &[f64],
    out: &mut Vec<f64>,
) -> Result<(), String> {
    let n = y_curr.len().min(yzy.len());
    if n > out.capacity() {
        crate::graph::scratch::reserve_graph_items(
            out,
            n - out.len(),
            "Newton-Schulz preconditioner CPU oracle",
            "newton_schulz_y_step output",
        )?;
    }
    out.clear();
    for (&y, &yzy_v) in y_curr.iter().zip(yzy.iter()).take(n) {
        out.push(0.5 * (3.0 * y - yzy_v));
    }
    Ok(())
}

/// Helper: f64 matrix-matrix multiply (for the CPU reference test
/// driver below). Not an op  -  testing convenience.
#[cfg(test)]
fn matmul_dense(a: &[f64], b: &[f64], n: usize) -> Vec<f64> {
    let mut c = Vec::new();
    matmul_dense_into(a, b, n, &mut c);
    c
}

#[cfg(any(test, feature = "cpu-parity"))]
fn matmul_dense_into(a: &[f64], b: &[f64], n: usize, c: &mut Vec<f64>) {
    c.clear();
    c.resize(n * n, 0.0);
    for i in 0..n {
        for j in 0..n {
            let mut acc = 0.0;
            for k in 0..n {
                acc += a[i * n + k] * b[k * n + j];
            }
            c[i * n + j] = acc;
        }
    }
}

/// Scratch workspace for repeated Newton-Schulz inverse-square-root references.
#[derive(Debug, Default)]
#[cfg(any(test, feature = "cpu-parity"))]
pub struct NewtonSchulzScratch {
    y: Vec<f64>,
    z: Vec<f64>,
    zy: Vec<f64>,
    three_i_minus_zy: Vec<f64>,
    y_times: Vec<f64>,
    z_times: Vec<f64>,
}

#[cfg(any(test, feature = "cpu-parity"))]
impl NewtonSchulzScratch {
    /// Construct empty Newton-Schulz scratch.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            y: Vec::new(),
            z: Vec::new(),
            zy: Vec::new(),
            three_i_minus_zy: Vec::new(),
            y_times: Vec::new(),
            z_times: Vec::new(),
        }
    }
}

/// CPU reference: full Newton-Schulz coupled iteration for `M^{-1/2}`.
///
/// Algorithm (Higham 2008, eq. 6.20):
/// ```text
///   Scale: M' = M / c, c chosen so spectrum(M') ⊂ (0, 1]
///   Y_0 = M',  Z_0 = I
///   Y_{k+1} = 0.5 Y_k (3 I - Z_k Y_k)
///   Z_{k+1} = 0.5 (3 I - Z_k Y_k) Z_k
///   Y_k → sqrt(M'),  Z_k → 1/sqrt(M')
///   Return Z_∞ / sqrt(c) as M^{-1/2}.
/// ```
///
/// Convergence is quadratic  -  ~10 iterations gives ~30 digits of
/// accuracy when the spectrum is close to 1.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn newton_schulz_inverse_sqrt_cpu(m: &[f64], n: usize, iters: u32) -> Vec<f64> {
    let mut out = Vec::new();
    let mut scratch = NewtonSchulzScratch::new();
    newton_schulz_inverse_sqrt_cpu_into(m, n, iters, &mut out, &mut scratch);
    out
}

/// CPU reference: full Newton-Schulz coupled iteration into caller-owned storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn newton_schulz_inverse_sqrt_cpu_into(
    m: &[f64],
    n: usize,
    iters: u32,
    out: &mut Vec<f64>,
    scratch: &mut NewtonSchulzScratch,
) {
    // Frobenius norm bounds the spectral radius  -  safe choice of c.
    let frob_sq: f64 = m.iter().map(|&v| v * v).sum();
    let c = frob_sq.sqrt();
    out.clear();
    out.resize(n * n, 0.0);
    if c == 0.0 {
        for i in 0..n {
            out[i * n + i] = 1.0;
        }
        return;
    }

    // Y_0 = M / c
    scratch.y.clear();
    scratch.y.reserve(n * n);
    for idx in 0..(n * n) {
        scratch.y.push(m.get(idx).copied().unwrap_or(0.0) / c);
    }
    // Z_0 = I
    scratch.z.clear();
    scratch.z.resize(n * n, 0.0);
    for i in 0..n {
        scratch.z[i * n + i] = 1.0;
    }

    for _ in 0..iters {
        matmul_dense_into(&scratch.z, &scratch.y, n, &mut scratch.zy);
        // (3I - Z Y)
        scratch.three_i_minus_zy.clear();
        scratch.three_i_minus_zy.extend_from_slice(&scratch.zy);
        for k in 0..(n * n) {
            scratch.three_i_minus_zy[k] = -scratch.zy[k];
        }
        for i in 0..n {
            scratch.three_i_minus_zy[i * n + i] += 3.0;
        }
        // Y_{k+1} = 0.5 * Y * (3I - ZY)
        matmul_dense_into(
            &scratch.y,
            &scratch.three_i_minus_zy,
            n,
            &mut scratch.y_times,
        );
        for value in &mut scratch.y_times {
            *value *= 0.5;
        }
        // Z_{k+1} = 0.5 * (3I - ZY) * Z
        matmul_dense_into(
            &scratch.three_i_minus_zy,
            &scratch.z,
            n,
            &mut scratch.z_times,
        );
        for value in &mut scratch.z_times {
            *value *= 0.5;
        }
        std::mem::swap(&mut scratch.y, &mut scratch.y_times);
        std::mem::swap(&mut scratch.z, &mut scratch.z_times);
    }

    // Return Z_∞ / sqrt(c) as M^{-1/2}
    let inv_sqrt_c = 1.0 / c.sqrt();
    for (dst, &value) in out.iter_mut().zip(scratch.z.iter()) {
        *dst = value * inv_sqrt_c;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-3 * (1.0 + a.abs() + b.abs())
    }

    #[test]
    fn cpu_one_step_decreases_error_for_diagonal() {
        // Start with Y = 0, after one step with yzy = 0: y_next = 0.5 * (3*0 - 0) = 0.
        // Doesn't move. Use Y = 0.5, yzy = 0.25: y_next = 0.5 * (1.5 - 0.25) = 0.625.
        let y = vec![0.5];
        let yzy = vec![0.25];
        let yn = newton_schulz_y_step_cpu(&y, &yzy);
        assert!(approx_eq(yn[0], 0.625));
    }

    #[test]
    fn cpu_y_step_into_reuses_output_storage() {
        let y = vec![0.5, 0.25];
        let yzy = vec![0.25, 0.125];
        let expected = newton_schulz_y_step_cpu(&y, &yzy);
        let mut out = Vec::with_capacity(expected.len());
        out.extend_from_slice(&[99.0, 98.0]);

        newton_schulz_y_step_cpu_into(&y, &yzy, &mut out);
        let ptr = out.as_ptr();
        let capacity = out.capacity();
        newton_schulz_y_step_cpu_into(&y, &yzy, &mut out);

        assert_eq!(out, expected);
        assert_eq!(out.as_ptr(), ptr);
        assert_eq!(out.capacity(), capacity);

        try_newton_schulz_y_step_cpu_into(&[1.0], &[0.5], &mut out)
            .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - Newton-Schulz Y-step should truncate stale output");
        assert_eq!(out, vec![1.25]);
        assert_eq!(out.as_ptr(), ptr);
        assert_eq!(out.capacity(), capacity);
    }

    #[test]
    fn cpu_y_step_truncates_mismatched_inputs() {
        let out = newton_schulz_y_step_cpu(&[1.0, 2.0], &[0.5]);
        assert_eq!(out, vec![1.25]);
    }

    #[test]
    fn cpu_inverse_sqrt_recovers_identity_inverse() {
        // M = I → M^{-1/2} = I.
        let m = vec![1.0, 0.0, 0.0, 1.0];
        let result = newton_schulz_inverse_sqrt_cpu(&m, 2, 12);
        // Expect ~ identity.
        assert!(approx_eq(result[0], 1.0));
        assert!(approx_eq(result[1], 0.0));
        assert!(approx_eq(result[2], 0.0));
        assert!(approx_eq(result[3], 1.0));
    }

    #[test]
    fn cpu_inverse_sqrt_pads_short_matrix() {
        let out = newton_schulz_inverse_sqrt_cpu(&[1.0], 2, 1);
        assert_eq!(out.len(), 4);
    }

    #[test]
    fn cpu_inverse_sqrt_recovers_diagonal_two() {
        // M = diag(4, 4) → M^{-1/2} = diag(0.5, 0.5).
        let m = vec![4.0, 0.0, 0.0, 4.0];
        let result = newton_schulz_inverse_sqrt_cpu(&m, 2, 20);
        assert!(approx_eq(result[0], 0.5));
        assert!(approx_eq(result[1], 0.0));
        assert!(approx_eq(result[2], 0.0));
        assert!(approx_eq(result[3], 0.5));
    }

    #[test]
    fn cpu_inverse_sqrt_into_reuses_workspace() {
        let m = vec![4.0, 0.0, 0.0, 4.0];
        let expected = newton_schulz_inverse_sqrt_cpu(&m, 2, 8);
        let mut out = Vec::with_capacity(4);
        let mut scratch = NewtonSchulzScratch::new();

        newton_schulz_inverse_sqrt_cpu_into(&m, 2, 8, &mut out, &mut scratch);
        let out_ptr = out.as_ptr();
        let y_ptr = scratch.y.as_ptr();
        let z_ptr = scratch.z.as_ptr();
        let zy_ptr = scratch.zy.as_ptr();
        let three_i_ptr = scratch.three_i_minus_zy.as_ptr();
        newton_schulz_inverse_sqrt_cpu_into(&m, 2, 8, &mut out, &mut scratch);

        assert_eq!(out.as_ptr(), out_ptr);
        assert_eq!(scratch.y.as_ptr(), y_ptr);
        assert_eq!(scratch.z.as_ptr(), z_ptr);
        assert_eq!(scratch.zy.as_ptr(), zy_ptr);
        assert_eq!(scratch.three_i_minus_zy.as_ptr(), three_i_ptr);
        for (got, want) in out.iter().zip(expected.iter()) {
            assert!(approx_eq(*got, *want));
        }
    }

    #[test]
    fn cpu_inverse_sqrt_property_y_squared_times_m_is_identity() {
        // For any PSD M, after enough iterations: Y² · M ≈ I.
        let m = vec![2.0, 0.5, 0.5, 1.5];
        let y = newton_schulz_inverse_sqrt_cpu(&m, 2, 30);
        let y_sq = matmul_dense(&y, &y, 2);
        let prod = matmul_dense(&y_sq, &m, 2);
        // prod ≈ identity
        assert!(approx_eq(prod[0], 1.0));
        assert!(approx_eq(prod[3], 1.0));
        assert!(prod[1].abs() < 1e-3);
        assert!(prod[2].abs() < 1e-3);
    }

    #[test]
    fn ir_program_buffer_layout() {
        let p = newton_schulz_y_step("y", "yzy", "yn", 4);
        assert_eq!(p.workgroup_size, [256, 1, 1]);
        let names: Vec<&str> = p.buffers.iter().map(|b| b.name()).collect();
        assert_eq!(names, vec!["y", "yzy", "yn"]);
        for buf in p.buffers.iter() {
            assert_eq!(buf.count(), 16); // n*n = 4*4
        }
    }

    #[test]
    fn zero_n_traps() {
        let p = newton_schulz_y_step("y", "yzy", "yn", 0);
        assert!(p.stats().trap());
    }
}
