//! Homotopy continuation predictor-corrector step (#9).
//!
//! Continuation methods solve `F(x) = 0` (the hard problem) by
//! deforming from `G(x) = 0` (the easy problem) via the homotopy
//!
//! ```text
//!   H(x, t) = (1 - t) · G(x) + t · F(x)
//! ```
//!
//! tracking solutions as `t` advances from 0 to 1. Recent parallel
//! variants (Bertini, HomotopyContinuation.jl, Lazard 2023) follow
//! many independent paths simultaneously  -  embarrassingly GPU-shaped.
//!
//! This file ships the **Euler predictor step** primitive  -  given the
//! current solution `x_t` and the Jacobian-vector product `J · v`
//! (where `v = -∂H/∂t`), advance to `x_{t + Δt}`:
//!
//! ```text
//!   x_pred = x_t + Δt · v
//! ```
//!
//! The corrector (Newton) step is one `semiring_gemm` matvec + one
//! linear-system solve, which composes from existing primitives. Each
//! independent path is one lane group on the GPU.
//!
//! # Why this primitive is dual-use
//!
//! | Consumer | Use |
//! |---|---|
//! | future `vyre-libs::opt::polynomial_solve` | polynomial system solving |
//! | future `vyre-libs::opt::milp_relax` | MILP continuous relaxation |
//! | `vyre-runtime/src/megakernel/planner.rs` (#22 self-consumer) | **vyre's megakernel scheduler ILP** is solved by relaxing to a continuous family parameterized by `t ∈ [0, 1]` and following the homotopy path on GPU |

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Op id.
pub const OP_ID: &str = "vyre-primitives::opt::homotopy_euler_predictor";

/// Emit the Euler predictor Program: `x_pred = x_curr + dt · v`
/// elementwise across `n_paths` independent paths × `n_dim` solution
/// dimensions per path.
///
/// Inputs:
/// - `x_curr`: `n_paths * n_dim` u32 (16.16 fp).
/// - `v`: `n_paths * n_dim` u32  -  Jacobian-vector product.
/// - `dt_scaled`: 1-element u32  -  `Δt` in 16.16 fp.
///
/// Output:
/// - `x_pred`: `n_paths * n_dim` u32.
#[must_use]
pub fn homotopy_euler_predictor(
    x_curr: &str,
    v: &str,
    dt_scaled: &str,
    x_pred: &str,
    n_paths: u32,
    n_dim: u32,
) -> Program {
    if n_paths == 0 {
        return crate::invalid_output_program(
            OP_ID,
            x_pred,
            DataType::U32,
            "Fix: homotopy_euler_predictor requires n_paths > 0, got 0.".to_string(),
        );
    }
    if n_dim == 0 {
        return crate::invalid_output_program(
            OP_ID,
            x_pred,
            DataType::U32,
            "Fix: homotopy_euler_predictor requires n_dim > 0, got 0.".to_string(),
        );
    }

    let cells = n_paths * n_dim;
    let t = Expr::InvocationId { axis: 0 };

    // x_pred[i] = x_curr[i] + fixed_mul_16_16(dt, v[i])
    let value = Expr::add(
        Expr::load(x_curr, t.clone()),
        crate::fixed_mul_16_16_expr(
            Expr::load(dt_scaled, Expr::u32(0)),
            Expr::load(v, t.clone()),
        ),
    );

    let body = vec![Node::if_then(
        Expr::lt(t.clone(), Expr::u32(cells)),
        vec![Node::store(x_pred, t, value)],
    )];

    Program::wrapped(
        vec![
            BufferDecl::storage(x_curr, 0, BufferAccess::ReadOnly, DataType::U32).with_count(cells),
            BufferDecl::storage(v, 1, BufferAccess::ReadOnly, DataType::U32).with_count(cells),
            BufferDecl::storage(dt_scaled, 2, BufferAccess::ReadOnly, DataType::U32).with_count(1),
            BufferDecl::storage(x_pred, 3, BufferAccess::ReadWrite, DataType::U32)
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

/// CPU reference: `x_pred[i] = x_curr[i] + dt · v[i]`.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn homotopy_euler_predictor_cpu(x_curr: &[f64], v: &[f64], dt: f64) -> Vec<f64> {
    x_curr
        .iter()
        .zip(v.iter())
        .map(|(&x, &dv)| x + dt * dv)
        .collect()
}

/// CPU helper: build the homotopy `H(x, t) = (1 - t) · G(x) + t · F(x)`
/// elementwise (parameter `t` in [0, 1]).
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn linear_homotopy_cpu(g_x: &[f64], f_x: &[f64], t: f64) -> Vec<f64> {
    let s = 1.0 - t;
    g_x.iter()
        .zip(f_x.iter())
        .map(|(&g, &f)| s * g + t * f)
        .collect()
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || homotopy_euler_predictor("x_curr", "v", "dt_scaled", "x_pred", 1, 2),
        Some(|| {
            vec![vec![
                crate::wire::pack_u32_slice(&[5u32 << 16, 7u32 << 16]),
                crate::wire::pack_u32_slice(&[2u32 << 16, 3u32 << 16]),
                crate::wire::pack_u32_slice(&[1u32 << 16]),
                crate::wire::pack_u32_slice(&[0, 0]),
            ]]
        }),
        Some(|| {
            vec![vec![crate::wire::pack_u32_slice(&[
                7u32 << 16,
                10u32 << 16,
            ])]]
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-10 * (1.0 + a.abs() + b.abs())
    }

    #[test]
    fn cpu_zero_dt_holds_state() {
        let x = vec![1.0, 2.0, 3.0];
        let v = vec![10.0, 20.0, 30.0];
        let out = homotopy_euler_predictor_cpu(&x, &v, 0.0);
        assert_eq!(out, x);
    }

    #[test]
    fn cpu_unit_dt_advances_by_v() {
        let x = vec![5.0];
        let v = vec![3.0];
        let out = homotopy_euler_predictor_cpu(&x, &v, 1.0);
        assert!(approx_eq(out[0], 8.0));
    }

    #[test]
    fn cpu_mismatched_inputs_truncate_to_complete_pairs() {
        assert_eq!(
            homotopy_euler_predictor_cpu(&[1.0, 2.0], &[3.0], 1.0),
            vec![4.0]
        );
        assert_eq!(linear_homotopy_cpu(&[1.0], &[3.0, 5.0], 0.5), vec![2.0]);
    }

    #[test]
    fn cpu_iterated_steps_track_known_path() {
        // For dx/dt = -x, exact solution is x(t) = x_0 · exp(-t).
        // Forward Euler with small dt should track it approximately.
        let mut x = vec![1.0];
        let dt = 0.01;
        for _ in 0..100 {
            let v: Vec<f64> = x.iter().map(|&xi| -xi).collect();
            x = homotopy_euler_predictor_cpu(&x, &v, dt);
        }
        // Exact at t=1: e^{-1} ≈ 0.3679. Forward Euler error ~ O(dt) so
        // accept ~10% tolerance.
        let exact = (-1.0f64).exp();
        assert!((x[0] - exact).abs() < 0.05);
    }

    #[test]
    fn cpu_linear_homotopy_endpoints_match() {
        let g = vec![1.0, 2.0];
        let f = vec![10.0, 20.0];
        let h0 = linear_homotopy_cpu(&g, &f, 0.0);
        let h1 = linear_homotopy_cpu(&g, &f, 1.0);
        assert_eq!(h0, g);
        assert_eq!(h1, f);
    }

    #[test]
    fn cpu_linear_homotopy_midpoint_averages() {
        let g = vec![0.0, 4.0];
        let f = vec![10.0, 0.0];
        let mid = linear_homotopy_cpu(&g, &f, 0.5);
        assert!(approx_eq(mid[0], 5.0));
        assert!(approx_eq(mid[1], 2.0));
    }

    #[test]
    fn ir_program_buffer_layout() {
        let p = homotopy_euler_predictor("xc", "v", "dt", "xp", 4, 8);
        assert_eq!(p.workgroup_size, [256, 1, 1]);
        let names: Vec<&str> = p.buffers.iter().map(|b| b.name()).collect();
        assert_eq!(names, vec!["xc", "v", "dt", "xp"]);
        assert_eq!(p.buffers[0].count(), 32);
        assert_eq!(p.buffers[1].count(), 32);
        assert_eq!(p.buffers[2].count(), 1);
        assert_eq!(p.buffers[3].count(), 32);
    }

    #[test]
    fn zero_n_paths_traps() {
        let p = homotopy_euler_predictor("xc", "v", "dt", "xp", 0, 1);
        assert!(p.stats().trap());
    }

    #[test]
    fn zero_n_dim_traps() {
        let p = homotopy_euler_predictor("xc", "v", "dt", "xp", 1, 0);
        assert!(p.stats().trap());
    }
}
