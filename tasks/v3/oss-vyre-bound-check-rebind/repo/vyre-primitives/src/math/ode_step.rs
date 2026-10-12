//! Classical fourth-order Runge-Kutta ODE step combiner.
//!
//! Given the four stage derivatives `k1..k4` (computed by the caller's
//! own `f(t, y)` Programs between dispatches) and a pre-scaled step
//! `h_scaled`, emit the RK4 next-state combiner:
//!
//! ```text
//! y_next[i] = y_prev[i] + h_scaled * (k1[i] + 2*k2[i] + 2*k3[i] + k4[i])
//! ```
//!
//! `h_scaled` is the caller-precomputed `h / 6` (in their fixed-point
//! convention). Keeping the divide on the host side avoids per-lane
//! precision loss; the GPU does only multiply-adds.
//!
//! # Why this primitive is dual-use
//!
//! | Consumer | Use |
//! |---|---|
//! | `vyre-libs::nn::neural_ode` consumers | continuous-time NN training |
//! | `vyre-libs::physics::flow` consumers | Lipschitz-bounded flows / sim |
//! | `vyre-primitives::opt::homotopy_continuation` (#9) | **path-tracking step** in homotopy methods uses RK4 to follow zeros of `H(x, t) = 0`; same Program serves user-dialect ODE *and* vyre's own combinatorial-optimization substrate |
//!
//! # Separate ODE-Step Ops
//!
//! - `dormand_prince_step` (DP5(4)) owns adaptive step control.
//! - Multi-shooting variants own independent segment dispatch plus a
//!   final stitch step that reconciles boundary continuity.

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Canonical op id.
pub const OP_ID: &str = "vyre-primitives::math::ode_rk4_step";

/// Emit the RK4 next-state combiner Program.
///
/// All buffers are `u32` length-`n` (caller's fixed-point convention).
/// `h_scaled_buffer` is a single-element u32 buffer holding `h/6` in
/// the caller's scale.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn rk4_step(
    y_prev: &str,
    k1: &str,
    k2: &str,
    k3: &str,
    k4: &str,
    h_scaled: &str,
    y_next: &str,
    n: u32,
) -> Program {
    if n == 0 {
        return crate::invalid_output_program(
            OP_ID,
            y_next,
            DataType::U32,
            format!("Fix: rk4_step requires n > 0, got {n}."),
        );
    }

    let t = Expr::InvocationId { axis: 0 };
    // weighted = k1 + 2*k2 + 2*k3 + k4
    let weighted = Expr::add(
        Expr::add(
            Expr::add(
                Expr::load(k1, t.clone()),
                Expr::mul(Expr::u32(2), Expr::load(k2, t.clone())),
            ),
            Expr::mul(Expr::u32(2), Expr::load(k3, t.clone())),
        ),
        Expr::load(k4, t.clone()),
    );
    let increment = Expr::mul(Expr::load(h_scaled, Expr::u32(0)), weighted);
    let next = Expr::add(Expr::load(y_prev, t.clone()), increment);

    let body = vec![Node::if_then(
        Expr::lt(t.clone(), Expr::u32(n)),
        vec![Node::store(y_next, t, next)],
    )];

    Program::wrapped(
        vec![
            BufferDecl::storage(y_prev, 0, BufferAccess::ReadOnly, DataType::U32).with_count(n),
            BufferDecl::storage(k1, 1, BufferAccess::ReadOnly, DataType::U32).with_count(n),
            BufferDecl::storage(k2, 2, BufferAccess::ReadOnly, DataType::U32).with_count(n),
            BufferDecl::storage(k3, 3, BufferAccess::ReadOnly, DataType::U32).with_count(n),
            BufferDecl::storage(k4, 4, BufferAccess::ReadOnly, DataType::U32).with_count(n),
            BufferDecl::storage(h_scaled, 5, BufferAccess::ReadOnly, DataType::U32).with_count(1),
            BufferDecl::storage(y_next, 6, BufferAccess::ReadWrite, DataType::U32).with_count(n),
        ],
        [256, 1, 1],
        vec![Node::Region {
            generator: Ident::from(OP_ID),
            source_region: None,
            body: Arc::new(body),
        }],
    )
}

/// CPU reference using f64 internally for precision; callers convert
/// to/from their fixed-point convention at the boundary.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn rk4_step_cpu(
    y_prev: &[f64],
    k1: &[f64],
    k2: &[f64],
    k3: &[f64],
    k4: &[f64],
    h: f64,
) -> Vec<f64> {
    let n = y_prev
        .len()
        .min(k1.len())
        .min(k2.len())
        .min(k3.len())
        .min(k4.len());

    let mut out = Vec::with_capacity(n);
    rk4_step_cpu_into(y_prev, k1, k2, k3, k4, h, &mut out);
    out
}

/// CPU reference into caller-owned storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn rk4_step_cpu_into(
    y_prev: &[f64],
    k1: &[f64],
    k2: &[f64],
    k3: &[f64],
    k4: &[f64],
    h: f64,
    out: &mut Vec<f64>,
) {
    try_rk4_step_cpu_into(y_prev, k1, k2, k3, k4, h, out)
        .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - rk4_step_cpu_into failed: output allocation failed");
}

/// Fallible CPU reference into caller-owned storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_rk4_step_cpu_into(
    y_prev: &[f64],
    k1: &[f64],
    k2: &[f64],
    k3: &[f64],
    k4: &[f64],
    h: f64,
    out: &mut Vec<f64>,
) -> Result<(), String> {
    let n = y_prev
        .len()
        .min(k1.len())
        .min(k2.len())
        .min(k3.len())
        .min(k4.len());
    if n > out.capacity() {
        crate::graph::scratch::reserve_graph_items(
            out,
            n - out.len(),
            "RK4 CPU oracle",
            "next-state output",
        )?;
    }
    out.clear();
    for i in 0..n {
        out.push(y_prev[i] + (h / 6.0) * (k1[i] + 2.0 * k2[i] + 2.0 * k3[i] + k4[i]));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f64 = 1e-10;

    fn approx_eq(a: f64, b: f64) -> bool {
        (a - b).abs() < EPS * (1.0 + a.abs() + b.abs())
    }

    #[test]
    fn cpu_zero_derivative_holds_state() {
        let y = vec![1.0, 2.0, 3.0];
        let k = vec![0.0, 0.0, 0.0];
        let next = rk4_step_cpu(&y, &k, &k, &k, &k, 0.1);
        for i in 0..y.len() {
            assert!(approx_eq(next[i], y[i]));
        }
    }

    #[test]
    fn cpu_constant_derivative_linear_advance() {
        // dy/dt = 1 (constant), all stages = 1, RK4 reduces to Euler.
        // y(t+h) = y(t) + h * 1
        let y = vec![5.0];
        let k = vec![1.0];
        let h = 0.5;
        let next = rk4_step_cpu(&y, &k, &k, &k, &k, h);
        assert!(approx_eq(next[0], 5.5));
    }

    #[test]
    fn cpu_classical_rk4_weights_recovered() {
        // Verify the (1, 2, 2, 1)/6 weighting explicitly. With distinct
        // stage values, the result must match the formula.
        let y = vec![0.0];
        let k1 = vec![1.0];
        let k2 = vec![2.0];
        let k3 = vec![3.0];
        let k4 = vec![4.0];
        let h = 6.0;
        // expected: 0 + 6/6 * (1 + 4 + 6 + 4) = 15
        let next = rk4_step_cpu(&y, &k1, &k2, &k3, &k4, h);
        assert!(approx_eq(next[0], 15.0));
    }

    #[test]
    fn cpu_mismatched_stage_lengths_truncate_to_valid_prefix() {
        let y = vec![1.0, 2.0];
        let k1 = vec![1.0];
        let k2 = vec![1.0, 1.0];
        let k3 = vec![1.0, 1.0];
        let k4 = vec![1.0, 1.0];
        let next = rk4_step_cpu(&y, &k1, &k2, &k3, &k4, 0.5);
        assert_eq!(next.len(), 1);
        assert!(approx_eq(next[0], 1.5));
    }

    #[test]
    fn cpu_into_reuses_output_and_truncates_stale_tail() {
        let y = vec![1.0, 2.0, 3.0];
        let k = vec![1.0, 1.0, 1.0];
        let mut out = Vec::with_capacity(8);
        out.extend([99.0; 8]);
        let ptr = out.as_ptr();

        try_rk4_step_cpu_into(&y, &k[..1], &k, &k, &k, 0.5, &mut out).unwrap();

        assert_eq!(out.len(), 1);
        assert!(approx_eq(out[0], 1.5));
        assert_eq!(out.as_ptr(), ptr);
    }

    #[test]
    fn generated_cpu_matches_independent_rk4_reference() {
        for case in 0..96 {
            let n = 1 + (case % 9);
            let h = 0.01 * ((case % 11) as f64 - 5.0);
            let y: Vec<f64> = (0..n).map(|i| i as f64 * 0.25 - 0.5).collect();
            let k1: Vec<f64> = (0..n).map(|i| (i as f64 + case as f64) * 0.01).collect();
            let k2: Vec<f64> = (0..n).map(|i| (i as f64 - case as f64) * 0.02).collect();
            let k3: Vec<f64> = (0..n).map(|i| (case as f64 - i as f64) * 0.03).collect();
            let k4: Vec<f64> = (0..n).map(|i| (i as f64 * i as f64) * 0.001).collect();
            let mut out = Vec::with_capacity(n + 3);

            try_rk4_step_cpu_into(&y, &k1, &k2, &k3, &k4, h, &mut out).unwrap();

            for i in 0..n {
                let expected = y[i] + (h / 6.0) * (k1[i] + 2.0 * k2[i] + 2.0 * k3[i] + k4[i]);
                assert!(
                    approx_eq(out[i], expected),
                    "case {case} idx {i}: expected {expected}, got {}",
                    out[i]
                );
            }
        }
    }

    #[test]
    fn cpu_solves_dy_eq_y_one_step() {
        // dy/dt = y, y(0) = 1. Exact: y(h) = e^h. RK4 error is O(h^5).
        // For h=0.1, RK4 should match e^0.1 ≈ 1.10517 to ~6 digits.
        let h = 0.1;
        let y0 = vec![1.0];
        // For this simple ODE, k_i = y_i where y_i is the value at the
        // sub-step. Compute manually:
        let k1: Vec<f64> = y0.clone();
        let k2: Vec<f64> = y0.iter().map(|&y| y + (h / 2.0) * y).collect(); // y + h/2 * k1
        let k3: Vec<f64> = y0
            .iter()
            .zip(k2.iter())
            .map(|(&y, &k2v)| y + (h / 2.0) * k2v)
            .collect();
        let k4: Vec<f64> = y0
            .iter()
            .zip(k3.iter())
            .map(|(&y, &k3v)| y + h * k3v)
            .collect();
        let next = rk4_step_cpu(&y0, &k1, &k2, &k3, &k4, h);
        let exact = (0.1f64).exp();
        assert!(
            (next[0] - exact).abs() < 1e-6,
            "got {} expected {}",
            next[0],
            exact
        );
    }

    #[test]
    fn ir_program_buffer_layout() {
        let p = rk4_step("y", "k1", "k2", "k3", "k4", "h", "yn", 32);
        assert_eq!(p.workgroup_size, [256, 1, 1]);
        let names: Vec<&str> = p.buffers.iter().map(|b| b.name()).collect();
        assert_eq!(names, vec!["y", "k1", "k2", "k3", "k4", "h", "yn"]);
        assert_eq!(p.buffers[5].count(), 1); // h_scaled is single-element
        for i in [0, 1, 2, 3, 4, 6] {
            assert_eq!(p.buffers[i].count(), 32);
        }
    }

    #[test]
    fn zero_n_traps() {
        let p = rk4_step("y", "k1", "k2", "k3", "k4", "h", "yn", 0);
        assert!(p.stats().trap());
    }
}
