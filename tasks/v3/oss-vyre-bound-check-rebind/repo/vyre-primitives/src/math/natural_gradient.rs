//! Natural-gradient preconditioner application.
//!
//! Natural gradient (Amari 1998) preconditions a gradient `g` by the
//! inverse Fisher information `F^{-1}`. For exponential-family models,
//! `F` is block-diagonal with closed-form blocks; for empirical-Fisher
//! approximations (KFAC, Shampoo, Sophia), each block is a small PSD
//! matrix that needs `M^{-1/2}` via Newton-Schulz (#16
//! [`crate::math::preconditioner`]).
//!
//! This file ships the **block-apply** primitive  -  apply a
//! preconditioner block to one slice of the gradient. Composes with
//! [`crate::math::preconditioner`] and [`crate::math::semiring_gemm`]
//! for the full preconditioned-gradient pipeline.
//!
//! # Composition pipeline
//!
//! ```text
//!   1. M = empirical_fisher_block(activations, grads)     // user-supplied
//!   2. M_inv_sqrt = newton_schulz_inverse_sqrt(M, iters)  // #16
//!   3. for each block:
//!        natural_gradient_block_apply(M_inv_sqrt, g, g_nat)  // this primitive
//!   4. apply g_nat to weights via SGD step                // user-supplied
//! ```
//!
//! # Why this primitive is dual-use
//!
//! | Consumer | Use |
//! |---|---|
//! | future `vyre-libs::optim::kfac` | K-FAC natural gradient |
//! | future `vyre-libs::optim::shampoo` | Shampoo per-block preconditioning |
//! | future `vyre-libs::optim::natural_sgd` | NG-SGD for exponential-family models |
//!
//! Self-consumer is weak; revisit when optimizer-aware dispatch
//! scheduling appears.

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Op id.
pub const OP_ID: &str = "vyre-primitives::math::natural_gradient_block_apply";

/// Apply a precomputed `M^{-1/2}` block to a gradient slice.
///
/// `g_nat[i] = Σ_j M_inv_sqrt[i, j] · g[j]`
///
/// This is one matrix-vector product, isomorphic to a single
/// [`crate::math::semiring_gemm`] call with shape `n × n · n × 1`. We
/// ship a focused primitive (rather than just calling semiring_gemm
/// directly) because the natural-gradient pipeline composes many of
/// these calls in sequence and giving them a stable op id makes
/// region-chain audits readable.
#[must_use]
pub fn natural_gradient_block_apply(
    m_inv_sqrt: &str,
    grad: &str,
    grad_nat: &str,
    n: u32,
) -> Program {
    match try_natural_gradient_block_apply(m_inv_sqrt, grad, grad_nat, n) {
        Ok(program) => program,
        Err(error) => crate::invalid_output_program(OP_ID, grad_nat, DataType::U32, error),
    }
}

/// Apply a precomputed `M^{-1/2}` block to a gradient slice with checked shape.
pub fn try_natural_gradient_block_apply(
    m_inv_sqrt: &str,
    grad: &str,
    grad_nat: &str,
    n: u32,
) -> Result<Program, String> {
    if n == 0 {
        return Err(format!(
            "Fix: natural_gradient_block_apply requires n > 0, got {n}."
        ));
    }
    let matrix_cells = n.checked_mul(n).ok_or_else(|| {
        format!(
            "natural_gradient_block_apply n={n} overflows preconditioner block cell count. Fix: shard the gradient block before GPU dispatch."
        )
    })?;

    let t = Expr::InvocationId { axis: 0 };

    let body = vec![Node::if_then(
        Expr::lt(t.clone(), Expr::u32(n)),
        vec![
            Node::let_bind("acc", Expr::u32(0)),
            Node::let_bind("row_base", Expr::mul(t.clone(), Expr::u32(n))),
            Node::loop_for(
                "j",
                Expr::u32(0),
                Expr::u32(n),
                vec![Node::assign(
                    "acc",
                    Expr::add(
                        Expr::var("acc"),
                        crate::fixed_mul_16_16_expr(
                            Expr::load(
                                m_inv_sqrt,
                                Expr::add(Expr::var("row_base"), Expr::var("j")),
                            ),
                            Expr::load(grad, Expr::var("j")),
                        ),
                    ),
                )],
            ),
            Node::store(grad_nat, t, Expr::var("acc")),
        ],
    )];

    Ok(Program::wrapped(
        vec![
            BufferDecl::storage(m_inv_sqrt, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(matrix_cells),
            BufferDecl::storage(grad, 1, BufferAccess::ReadOnly, DataType::U32).with_count(n),
            BufferDecl::storage(grad_nat, 2, BufferAccess::ReadWrite, DataType::U32).with_count(n),
        ],
        [256, 1, 1],
        vec![Node::Region {
            generator: Ident::from(OP_ID),
            source_region: None,
            body: Arc::new(body),
        }],
    ))
}

/// CPU reference: `g_nat = M_inv_sqrt · g` in f64.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn natural_gradient_block_apply_cpu(m_inv_sqrt: &[f64], grad: &[f64], n: u32) -> Vec<f64> {
    let mut out = Vec::new();
    try_natural_gradient_block_apply_cpu_into(m_inv_sqrt, grad, n, &mut out)
        .unwrap_or_else(|error| panic!("{error}"));
    out
}

/// CPU reference: `g_nat = M_inv_sqrt · g` in f64 using caller-owned output.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn natural_gradient_block_apply_cpu_into(
    m_inv_sqrt: &[f64],
    grad: &[f64],
    n: u32,
    out: &mut Vec<f64>,
) {
    try_natural_gradient_block_apply_cpu_into(m_inv_sqrt, grad, n, out)
        .unwrap_or_else(|error| panic!("{error}"));
}

/// Fallible CPU reference: `g_nat = M_inv_sqrt · g` in f64 using caller-owned output.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_natural_gradient_block_apply_cpu_into(
    m_inv_sqrt: &[f64],
    grad: &[f64],
    n: u32,
    out: &mut Vec<f64>,
) -> Result<(), String> {
    let n = n as usize;
    n.checked_mul(n).ok_or_else(|| {
        format!(
            "natural_gradient_block_apply CPU oracle n={n} overflows preconditioner block indexing. Fix: shard the gradient block before parity evaluation."
        )
    })?;
    if n > out.capacity() {
        crate::graph::scratch::reserve_graph_items(
            out,
            n - out.len(),
            "natural-gradient CPU oracle",
            "natural_gradient_block_apply output",
        )?;
    }
    out.clear();
    out.resize(n, 0.0);
    for i in 0..n {
        let mut acc = 0.0;
        for j in 0..n {
            let Some(&m) = m_inv_sqrt.get(i * n + j) else {
                continue;
            };
            let Some(&g) = grad.get(j) else {
                continue;
            };
            acc += m * g;
        }
        out[i] = acc;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-10 * (1.0 + a.abs() + b.abs())
    }

    #[test]
    fn cpu_identity_passthrough() {
        // I · g = g
        let i_mat = vec![1.0, 0.0, 0.0, 1.0];
        let g = vec![3.0, 5.0];
        let out = natural_gradient_block_apply_cpu(&i_mat, &g, 2);
        assert!(approx_eq(out[0], 3.0));
        assert!(approx_eq(out[1], 5.0));
    }

    #[test]
    fn cpu_diagonal_scales_each_component() {
        // diag(0.5, 2.0) · g
        let m = vec![0.5, 0.0, 0.0, 2.0];
        let g = vec![4.0, 3.0];
        let out = natural_gradient_block_apply_cpu(&m, &g, 2);
        assert!(approx_eq(out[0], 2.0)); // 0.5 * 4
        assert!(approx_eq(out[1], 6.0)); // 2.0 * 3
    }

    #[test]
    fn cpu_off_diagonal_couples() {
        // [[0, 1], [1, 0]] · [a, b] = [b, a]
        let m = vec![0.0, 1.0, 1.0, 0.0];
        let g = vec![7.0, 11.0];
        let out = natural_gradient_block_apply_cpu(&m, &g, 2);
        assert!(approx_eq(out[0], 11.0));
        assert!(approx_eq(out[1], 7.0));
    }

    #[test]
    fn cpu_into_reuses_output_buffer() {
        let m = vec![0.5, 0.0, 0.0, 2.0];
        let g = vec![4.0, 3.0];
        let mut out = Vec::with_capacity(8);
        out.extend_from_slice(&[99.0, 98.0, 97.0, 96.0]);
        let ptr = out.as_ptr();
        let capacity = out.capacity();
        natural_gradient_block_apply_cpu_into(&m, &g, 2, &mut out);
        assert!(approx_eq(out[0], 2.0));
        assert!(approx_eq(out[1], 6.0));
        assert_eq!(out.len(), 2);
        assert_eq!(out.as_ptr(), ptr);
        assert_eq!(out.capacity(), capacity);

        natural_gradient_block_apply_cpu_into(&[3.0], &[5.0], 1, &mut out);
        assert_eq!(out, vec![15.0]);
        assert_eq!(out.as_ptr(), ptr);
        assert_eq!(out.capacity(), capacity);
    }

    #[test]
    fn cpu_malformed_inputs_treat_missing_entries_as_zero() {
        let out = natural_gradient_block_apply_cpu(&[2.0], &[3.0], 2);
        assert_eq!(out.len(), 2);
        assert!(approx_eq(out[0], 6.0));
        assert!(approx_eq(out[1], 0.0));
    }

    #[test]
    fn ir_program_buffer_layout() {
        let p = natural_gradient_block_apply("M", "g", "gn", 8);
        assert_eq!(p.workgroup_size, [256, 1, 1]);
        let names: Vec<&str> = p.buffers.iter().map(|b| b.name()).collect();
        assert_eq!(names, vec!["M", "g", "gn"]);
        assert_eq!(p.buffers[0].count(), 64); // n*n
        assert_eq!(p.buffers[1].count(), 8); // n
        assert_eq!(p.buffers[2].count(), 8); // n
    }

    #[test]
    fn zero_n_traps() {
        let p = natural_gradient_block_apply("M", "g", "gn", 0);
        assert!(p.stats().trap());
    }

    #[test]
    fn checked_builder_rejects_preconditioner_cell_overflow() {
        let error = try_natural_gradient_block_apply("M", "g", "gn", u32::MAX)
            .expect_err("checked natural-gradient builder must reject n*n overflow");

        assert!(
            error.contains("overflows preconditioner block cell count"),
            "error should describe preconditioner shape overflow: {error}"
        );
    }
}
