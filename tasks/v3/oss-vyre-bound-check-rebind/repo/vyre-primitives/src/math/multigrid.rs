//! Algebraic Multigrid V-cycle smoothing primitive (#50).
//!
//! AMG (Brandt 1986, Ruge-Stüben 1987) solves elliptic PDEs in O(n)
//! by alternating SMOOTHING (relax error on the current level) with
//! coarsening / restriction. Each level's smoother is GPU-parallel  -
//! the recursive structure is the part that's historically hard to
//! schedule, but with `level_wave_program` (already in vyre) the
//! V-cycle becomes a straightforward dispatch sequence.
//!
//! This file ships the **Jacobi smoother step** primitive  -  one
//! weighted-Jacobi relaxation on a single level. The full V-cycle
//! pipeline:
//!
//! ```text
//!   pre_smooth          : N iterations of jacobi_smooth_step on level k
//!   restrict            : project residual to coarser level (caller; matvec)
//!   recurse             : V-cycle on level k-1
//!   prolong             : interpolate correction back to level k (matvec)
//!   correct + post_smooth: jacobi_smooth_step a few more times
//! ```
//!
//! # Why this primitive is dual-use
//!
//! | Consumer | Use |
//! |---|---|
//! | future `vyre-libs::sci::poisson` | Poisson / Laplace solvers |
//! | future `vyre-libs::sci::diffusion` | diffusion / heat equation |
//! | future `vyre-libs::ml::pde_emulator` | physics-informed NN training |
//! | `vyre-driver` multilevel scheduling | IR-graph contraction levels match V-cycle levels; apply the same smoother as the dispatch scheduler smoother |

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Op id.
pub const OP_ID: &str = "vyre-primitives::math::amg_jacobi_step";

/// One weighted Jacobi smoothing step:
///
/// ```text
///   x_new[i] = x[i] + ω · (b[i] - Σ_j A[i,j] · x[j]) / A[i,i]
/// ```
///
/// Inputs:
/// - `a_matrix`: row-major `n × n` u32 (16.16). Symmetric PSD (caller-
///   supplied; the V-cycle works on any positive-definite system but
///   classical AMG assumes structure).
/// - `b`: length-`n` u32 right-hand side.
/// - `x_in`: length-`n` u32 current iterate.
/// - `omega_scaled`: 1-element u32 buffer, ω in 16.16 (typically 2/3).
///
/// Output:
/// - `x_out`: length-`n` u32 next iterate.
#[must_use]
pub fn jacobi_smooth_step(
    a_matrix: &str,
    b: &str,
    x_in: &str,
    omega_scaled: &str,
    x_out: &str,
    n: u32,
) -> Program {
    match try_jacobi_smooth_step(a_matrix, b, x_in, omega_scaled, x_out, n) {
        Ok(program) => program,
        Err(error) => crate::invalid_output_program(OP_ID, x_out, DataType::U32, error),
    }
}

/// One weighted Jacobi smoothing step with checked dense matrix sizing.
pub fn try_jacobi_smooth_step(
    a_matrix: &str,
    b: &str,
    x_in: &str,
    omega_scaled: &str,
    x_out: &str,
    n: u32,
) -> Result<Program, String> {
    if n == 0 {
        return Err(format!("Fix: jacobi_smooth_step requires n > 0, got {n}."));
    }
    let matrix_cells = checked_jacobi_cells(n)?;

    let t = Expr::InvocationId { axis: 0 };

    let body = vec![Node::if_then(
        Expr::lt(t.clone(), Expr::u32(n)),
        vec![
            // residual = b[i] - Σ_j A[i,j] · x_in[j]
            Node::let_bind("res", Expr::load(b, t.clone())),
            Node::let_bind("row_base", Expr::mul(t.clone(), Expr::u32(n))),
            Node::loop_for(
                "j",
                Expr::u32(0),
                Expr::u32(n),
                vec![Node::assign(
                    "res",
                    Expr::sub(
                        Expr::var("res"),
                        crate::fixed_mul_16_16_expr(
                            Expr::load(a_matrix, Expr::add(Expr::var("row_base"), Expr::var("j"))),
                            Expr::load(x_in, Expr::var("j")),
                        ),
                    ),
                )],
            ),
            // diag = A[i, i]; safe = max(diag, 1)
            Node::let_bind(
                "diag",
                Expr::load(a_matrix, Expr::add(Expr::var("row_base"), t.clone())),
            ),
            Node::let_bind(
                "diag_safe",
                Expr::select(
                    Expr::eq(Expr::var("diag"), Expr::u32(0)),
                    Expr::u32(1),
                    Expr::var("diag"),
                ),
            ),
            Node::let_bind(
                "diag_units",
                Expr::select(
                    Expr::lt(Expr::var("diag_safe"), Expr::u32(1 << 16)),
                    Expr::u32(1),
                    Expr::shr(Expr::var("diag_safe"), Expr::u32(16)),
                ),
            ),
            // delta = (omega · res) / diag_safe (16.16 throughout).
            // `(omega * res) >> 16` leaves a 16.16 numerator. Divide by the
            // integer diagonal scale to avoid the overflowing `(num << 16)`
            // rescale on 32-bit target lanes.
            Node::let_bind(
                "delta",
                Expr::div(
                    crate::fixed_mul_16_16_expr(
                        Expr::load(omega_scaled, Expr::u32(0)),
                        Expr::var("res"),
                    ),
                    Expr::var("diag_units"),
                ),
            ),
            Node::store(
                x_out,
                t.clone(),
                Expr::add(Expr::load(x_in, t), Expr::var("delta")),
            ),
        ],
    )];

    Ok(Program::wrapped(
        vec![
            BufferDecl::storage(a_matrix, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(matrix_cells),
            BufferDecl::storage(b, 1, BufferAccess::ReadOnly, DataType::U32).with_count(n),
            BufferDecl::storage(x_in, 2, BufferAccess::ReadOnly, DataType::U32).with_count(n),
            BufferDecl::storage(omega_scaled, 3, BufferAccess::ReadOnly, DataType::U32)
                .with_count(1),
            BufferDecl::storage(x_out, 4, BufferAccess::ReadWrite, DataType::U32).with_count(n),
        ],
        [256, 1, 1],
        vec![Node::Region {
            generator: Ident::from(OP_ID),
            source_region: None,
            body: Arc::new(body),
        }],
    ))
}

fn checked_jacobi_cells(n: u32) -> Result<u32, String> {
    n.checked_mul(n).ok_or_else(|| {
        format!(
            "jacobi_smooth_step n={n} overflows dense matrix cell count. Fix: shard or sparsify the AMG level before GPU dispatch."
        )
    })
}

/// CPU reference: one weighted Jacobi step in f64.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn jacobi_smooth_step_cpu(a: &[f64], b: &[f64], x_in: &[f64], omega: f64, n: u32) -> Vec<f64> {
    try_jacobi_smooth_step_cpu(a, b, x_in, omega, n).unwrap_or_else(|error| panic!("{error}"))
}

/// Fallible CPU reference: one weighted Jacobi step in f64.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_jacobi_smooth_step_cpu(
    a: &[f64],
    b: &[f64],
    x_in: &[f64],
    omega: f64,
    n: u32,
) -> Result<Vec<f64>, String> {
    let mut out = Vec::new();
    try_jacobi_smooth_step_cpu_into(a, b, x_in, omega, n, &mut out)?;
    Ok(out)
}

/// CPU reference: one weighted Jacobi step in f64, writing into caller-owned storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn jacobi_smooth_step_cpu_into(
    a: &[f64],
    b: &[f64],
    x_in: &[f64],
    omega: f64,
    n: u32,
    out: &mut Vec<f64>,
) {
    try_jacobi_smooth_step_cpu_into(a, b, x_in, omega, n, out)
        .unwrap_or_else(|error| panic!("{error}"));
}

/// Fallible CPU reference: one weighted Jacobi step in f64, writing into caller-owned storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_jacobi_smooth_step_cpu_into(
    a: &[f64],
    b: &[f64],
    x_in: &[f64],
    omega: f64,
    n: u32,
    out: &mut Vec<f64>,
) -> Result<(), String> {
    let n = n as usize;
    n.checked_mul(n).ok_or_else(|| {
        format!(
            "jacobi_smooth_step CPU oracle n={n} overflows dense matrix indexing. Fix: shard or sparsify the AMG level before parity evaluation."
        )
    })?;
    if n > out.capacity() {
        crate::graph::scratch::reserve_graph_items(
            out,
            n - out.len(),
            "AMG Jacobi CPU oracle",
            "jacobi_smooth_step output",
        )?;
    }
    out.clear();
    for i in 0..n {
        let mut ax_i = 0.0;
        for j in 0..n {
            let a_ij = a.get(i * n + j).copied().unwrap_or(0.0);
            let x_j = x_in.get(j).copied().unwrap_or(0.0);
            ax_i += a_ij * x_j;
        }
        let res = b.get(i).copied().unwrap_or(0.0) - ax_i;
        let diag_value = a.get(i * n + i).copied().unwrap_or(0.0);
        let diag = if diag_value.abs() > 1e-30 {
            diag_value
        } else {
            1.0
        };
        out.push(x_in.get(i).copied().unwrap_or(0.0) + omega * res / diag);
    }
    Ok(())
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || jacobi_smooth_step("a", "b", "x", "omega", "out", 1),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![
                to_bytes(&[1u32 << 16]),
                to_bytes(&[3u32 << 16]),
                to_bytes(&[1u32 << 16]),
                to_bytes(&[1u32 << 16]),
                to_bytes(&[0]),
            ]]
        }),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![to_bytes(&[3u32 << 16])]]
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
    fn cpu_zero_residual_holds_solution() {
        // If A x = b exactly, Jacobi update should leave x unchanged.
        let a = vec![1.0, 0.0, 0.0, 1.0];
        let b = vec![3.0, 5.0];
        let x = vec![3.0, 5.0];
        let new_x = jacobi_smooth_step_cpu(&a, &b, &x, 1.0, 2);
        assert!(approx_eq(new_x[0], 3.0));
        assert!(approx_eq(new_x[1], 5.0));
    }

    #[test]
    fn cpu_iterations_converge_to_solution() {
        // Solve [[2, -1], [-1, 2]] x = [1, 1]; exact x = [1, 1].
        let a = vec![2.0, -1.0, -1.0, 2.0];
        let b = vec![1.0, 1.0];
        let mut x = vec![0.0, 0.0];
        for _ in 0..50 {
            x = jacobi_smooth_step_cpu(&a, &b, &x, 2.0 / 3.0, 2);
        }
        assert!(approx_eq(x[0], 1.0));
        assert!(approx_eq(x[1], 1.0));
    }

    #[test]
    fn cpu_omega_one_matches_classical_jacobi() {
        // ω = 1 reduces to vanilla Jacobi.
        let a = vec![4.0, 1.0, 1.0, 3.0];
        let b = vec![1.0, 2.0];
        let x_in = vec![0.0, 0.0];
        let x = jacobi_smooth_step_cpu(&a, &b, &x_in, 1.0, 2);
        // Classical Jacobi: x[0] = b[0]/a[0,0] = 0.25; x[1] = b[1]/a[1,1] = 2/3
        assert!(approx_eq(x[0], 0.25));
        assert!(approx_eq(x[1], 2.0 / 3.0));
    }

    #[test]
    fn cpu_short_inputs_are_zero_padded() {
        let out = jacobi_smooth_step_cpu(&[2.0], &[], &[], 1.0, 2);
        assert_eq!(out.len(), 2);
        assert!(out.iter().all(|&v| approx_eq(v, 0.0)));
    }

    #[test]
    fn cpu_into_reuses_output_and_truncates_stale_tail() {
        let a = vec![4.0, 1.0, 1.0, 3.0];
        let b = vec![1.0, 2.0];
        let x_in = vec![0.0, 0.0];
        let mut out = Vec::with_capacity(4);
        out.extend_from_slice(&[99.0, 98.0, 97.0, 96.0]);
        let ptr = out.as_ptr();
        let capacity = out.capacity();

        try_jacobi_smooth_step_cpu_into(&a, &b, &x_in, 1.0, 2, &mut out)
            .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - Jacobi CPU oracle should reuse caller-owned output");

        assert_eq!(out.len(), 2);
        assert!(approx_eq(out[0], 0.25));
        assert!(approx_eq(out[1], 2.0 / 3.0));
        assert_eq!(out.as_ptr(), ptr);
        assert_eq!(out.capacity(), capacity);

        try_jacobi_smooth_step_cpu_into(&[2.0], &[4.0], &[1.0], 1.0, 1, &mut out)
            .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - Jacobi CPU oracle should truncate stale output");

        assert_eq!(out, vec![2.0]);
        assert_eq!(out.as_ptr(), ptr);
        assert_eq!(out.capacity(), capacity);
    }

    #[test]
    fn generated_jacobi_cpu_matches_independent_reference() {
        for case in 0..512usize {
            let n = case % 9 + 1;
            let a_len = (case * 5) % (n * n + 1);
            let b_len = (case * 7) % (n + 1);
            let x_len = (case * 11) % (n + 1);
            let omega = ((case % 17) as f64 - 8.0) / 5.0;
            let a: Vec<f64> = (0..a_len)
                .map(|idx| ((idx * 13 + case) % 23) as f64 / 7.0 - 1.0)
                .collect();
            let b: Vec<f64> = (0..b_len)
                .map(|idx| ((idx * 17 + case) % 29) as f64 / 11.0 - 1.0)
                .collect();
            let x: Vec<f64> = (0..x_len)
                .map(|idx| ((idx * 19 + case) % 31) as f64 / 13.0 - 1.0)
                .collect();
            let actual = try_jacobi_smooth_step_cpu(&a, &b, &x, omega, n as u32)
                .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - generated Jacobi CPU oracle should evaluate");
            let expected = independent_jacobi(&a, &b, &x, omega, n);

            assert_eq!(actual.len(), n, "case {case}: output length");
            for idx in 0..n {
                assert!(
                    approx_eq(actual[idx], expected[idx]),
                    "case {case} idx {idx}: expected {}, got {}",
                    expected[idx],
                    actual[idx]
                );
            }
        }
    }

    fn independent_jacobi(a: &[f64], b: &[f64], x_in: &[f64], omega: f64, n: usize) -> Vec<f64> {
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            let mut ax_i = 0.0;
            for j in 0..n {
                ax_i +=
                    a.get(i * n + j).copied().unwrap_or(0.0) * x_in.get(j).copied().unwrap_or(0.0);
            }
            let res = b.get(i).copied().unwrap_or(0.0) - ax_i;
            let diag_value = a.get(i * n + i).copied().unwrap_or(0.0);
            let diag = if diag_value.abs() > 1e-30 {
                diag_value
            } else {
                1.0
            };
            out.push(x_in.get(i).copied().unwrap_or(0.0) + omega * res / diag);
        }
        out
    }

    #[test]
    fn ir_program_buffer_layout() {
        let p = jacobi_smooth_step("A", "b", "xi", "om", "xo", 4);
        assert_eq!(p.workgroup_size, [256, 1, 1]);
        let names: Vec<&str> = p.buffers.iter().map(|b| b.name()).collect();
        assert_eq!(names, vec!["A", "b", "xi", "om", "xo"]);
        assert_eq!(p.buffers[0].count(), 16);
        assert_eq!(p.buffers[1].count(), 4);
        assert_eq!(p.buffers[2].count(), 4);
        assert_eq!(p.buffers[3].count(), 1);
        assert_eq!(p.buffers[4].count(), 4);
    }

    #[test]
    fn zero_n_traps() {
        let p = jacobi_smooth_step("A", "b", "xi", "om", "xo", 0);
        assert!(p.stats().trap());
    }

    #[test]
    fn checked_builder_rejects_dense_matrix_overflow() {
        let error = try_jacobi_smooth_step("A", "b", "xi", "om", "xo", u32::MAX)
            .expect_err("checked Jacobi builder must reject dense matrix overflow");

        assert!(
            error.contains("overflows dense matrix cell count"),
            "error should describe dense matrix overflow: {error}"
        );
    }
}
