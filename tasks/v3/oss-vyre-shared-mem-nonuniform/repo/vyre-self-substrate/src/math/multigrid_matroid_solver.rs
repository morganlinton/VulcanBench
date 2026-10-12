//! Sparse linear-system solver for matroid intersection via #50
//! algebraic multigrid (#50 self-consumer).
//!
//! Closes the recursion thesis for #50  -  Algebraic Multigrid (AMG)
//! Jacobi smoothing ships to user dialects (any PDE / sparse-system
//! workload) AND solves the inner linear systems that
//! Chakrabarty-Lee-Sidford (2021) reduces matroid intersection to.
//!
//! # The self-use
//!
//! Modern matroid-intersection algorithms (CLS-2021) reduce each
//! augmenting iteration to O(n²) iterations of solving sparse linear
//! systems M·x = b where M is the matroid-cover Laplacian.
//! Naive Gauss-Seidel takes O(n²) per step → O(n⁴) overall  -
//! intractable at workspace scale.
//!
//! AMG V-cycle drops this to O(n) per step → O(n³) overall, AND
//! the V-cycle structure is GPU-shaped: each level's smoothing is
//! one Jacobi-iteration dispatch, the coarsening between levels
//! is one prolongation/restriction sparse-matmul.
//!
//! Combined with the matroid_megakernel_scheduler self-consumer:
//!
//! ```text
//! megakernel scheduler                  matroid intersection
//!         |                                      ^
//!         v                                      |
//!   homotopy continuous solver --rounds--> matroid intersection
//!                                                 |
//!                                                 v
//!                                     CLS-2021 sparse linear solve
//!                                                 |
//!                                                 v
//!                                     AMG V-cycle (this self-consumer)
//! ```
//!
//! Three-deep recursive substrate: scheduler → matroid → AMG.
//! Each layer's Tier-2.5 primitive is the substrate for the layer
//! above. The recursion thesis at its limit.
//!
//! # Algorithm
//!
//! This module owns the per-level Jacobi-smoothing step and the
//! host-side tolerance loop used by matroid-intersection callers. A
//! full V-cycle composes this step with explicit restriction and
//! prolongation primitives.

use crate::dispatch_buffers::{
    ceil_div_u32, checked_square_cells, decode_u32_output_exact, ensure_input_slots,
    write_u32_slice_le_bytes, write_zero_bytes,
};
use crate::optimizer::dispatcher::{DispatchError, OptimizerDispatcher};
#[cfg(test)]
use vyre_foundation::pass_substrate::multigrid_matroid_solver as foundation_multigrid;
use vyre_primitives::math::multigrid::jacobi_smooth_step;

/// Caller-owned dispatch scratch for fixed-point multigrid Jacobi smoothing.
#[derive(Debug, Default)]
pub struct MultigridMatroidGpuScratch {
    inputs: Vec<Vec<u8>>,
    omega: Vec<u32>,
}

/// Apply one weighted-Jacobi smoothing step to the matroid linear
/// system. `a` is the n*n cover-Laplacian matrix; `b` is the rhs;
/// `x_in` is the current iterate; `omega` is the relaxation weight
/// (0.66 is the standard choice for pure Jacobi convergence on
/// Laplacian systems).
///
/// # Panics
///
/// Panics on size mismatches.
#[must_use]
#[cfg(test)]
pub fn reference_matroid_solve_step(
    a: &[f64],
    b: &[f64],
    x_in: &[f64],
    omega: f64,
    n: u32,
) -> Vec<f64> {
    use crate::observability::{bump, multigrid_matroid_solver_calls};
    bump(&multigrid_matroid_solver_calls);
    foundation_multigrid::matroid_solve_step(a, b, x_in, omega, n)
}

/// Apply one weighted-Jacobi smoothing step into caller-owned storage.
///
/// This is the hot path for tolerance loops; it avoids allocating a new
/// solution vector for every relaxation iteration.
#[cfg(test)]
pub fn reference_matroid_solve_step_into(
    a: &[f64],
    b: &[f64],
    x_in: &[f64],
    omega: f64,
    n: u32,
    out: &mut Vec<f64>,
) {
    use crate::observability::{bump, multigrid_matroid_solver_calls};
    bump(&multigrid_matroid_solver_calls);
    foundation_multigrid::matroid_solve_step_into(a, b, x_in, omega, n, out);
}

/// Primitive-native fixed-point production path for one weighted-Jacobi
/// matroid solve step.
///
/// Inputs are 16.16 u32 buffers. This path is intended for systems already
/// represented in the primitive's nonnegative fixed-point domain; signed f64
/// systems stay on the reference compatibility helpers above until a signed
/// primitive lands.
///
/// # Errors
///
/// Returns [`DispatchError`] when shapes are invalid, lane counts exceed the
/// primitive range, or the backend returns malformed output.
pub fn matroid_solve_step_fixed_via(
    dispatcher: &impl OptimizerDispatcher,
    a_fixed: &[u32],
    b_fixed: &[u32],
    x_in_fixed: &[u32],
    omega_fixed: u32,
    n: u32,
) -> Result<Vec<u32>, DispatchError> {
    let mut out = Vec::new();
    matroid_solve_step_fixed_via_into(
        dispatcher,
        a_fixed,
        b_fixed,
        x_in_fixed,
        omega_fixed,
        n,
        &mut out,
    )?;
    Ok(out)
}

/// Primitive-native fixed-point weighted-Jacobi step into caller-owned output.
///
/// # Errors
///
/// Returns [`DispatchError`] when shape checks or backend execution fail.
pub fn matroid_solve_step_fixed_via_into(
    dispatcher: &impl OptimizerDispatcher,
    a_fixed: &[u32],
    b_fixed: &[u32],
    x_in_fixed: &[u32],
    omega_fixed: u32,
    n: u32,
    out: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    let mut scratch = MultigridMatroidGpuScratch::default();
    matroid_solve_step_fixed_via_with_scratch_into(
        dispatcher,
        a_fixed,
        b_fixed,
        x_in_fixed,
        omega_fixed,
        n,
        &mut scratch,
        out,
    )
}

/// Primitive-native fixed-point weighted-Jacobi step using caller-owned dispatch scratch.
///
/// # Errors
///
/// Returns [`DispatchError`] when shape checks or backend execution fail.
pub fn matroid_solve_step_fixed_via_with_scratch_into(
    dispatcher: &impl OptimizerDispatcher,
    a_fixed: &[u32],
    b_fixed: &[u32],
    x_in_fixed: &[u32],
    omega_fixed: u32,
    n: u32,
    scratch: &mut MultigridMatroidGpuScratch,
    out: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    use crate::observability::{bump, multigrid_matroid_solver_calls};
    bump(&multigrid_matroid_solver_calls);

    let cells = checked_square_cells(n, "matroid_solve_step_fixed_via")?;
    let cells_u32 = u32::try_from(cells).map_err(|_| {
        DispatchError::BadInputs(format!(
            "Fix: matroid_solve_step_fixed_via n*n exceeds the primitive u32 lane limit for n={n}."
        ))
    })?;
    if a_fixed.len() != cells {
        return Err(DispatchError::BadInputs(format!(
            "Fix: matroid_solve_step_fixed_via requires a_fixed.len() == n*n, got len={}, n={n}, n*n={cells}.",
            a_fixed.len()
        )));
    }
    if b_fixed.len() != n as usize {
        return Err(DispatchError::BadInputs(format!(
            "Fix: matroid_solve_step_fixed_via requires b_fixed.len() == n, got len={}, n={n}.",
            b_fixed.len()
        )));
    }
    if x_in_fixed.len() != n as usize {
        return Err(DispatchError::BadInputs(format!(
            "Fix: matroid_solve_step_fixed_via requires x_in_fixed.len() == n, got len={}, n={n}.",
            x_in_fixed.len()
        )));
    }

    let program = jacobi_smooth_step("a", "b", "x_in", "omega", "x_out", n);
    let out_bytes = (n as usize)
        .checked_mul(std::mem::size_of::<u32>())
        .ok_or_else(|| {
            DispatchError::BadInputs(format!(
                "Fix: matroid_solve_step_fixed_via n={n} overflows output byte count."
            ))
        })?;
    scratch.omega.clear();
    scratch.omega.push(omega_fixed);
    ensure_input_slots(&mut scratch.inputs, 5);
    write_u32_slice_le_bytes(&mut scratch.inputs[0], a_fixed);
    write_u32_slice_le_bytes(&mut scratch.inputs[1], b_fixed);
    write_u32_slice_le_bytes(&mut scratch.inputs[2], x_in_fixed);
    write_u32_slice_le_bytes(&mut scratch.inputs[3], &scratch.omega);
    write_zero_bytes(&mut scratch.inputs[4], out_bytes);
    let outputs = dispatcher.dispatch(
        &program,
        &scratch.inputs,
        Some([ceil_div_u32(cells_u32, 256), 1, 1]),
    )?;
    if outputs.is_empty() {
        return Err(DispatchError::BackendError(format!(
            "Fix: matroid_solve_step_fixed_via expected at least one output buffer, got {}.",
            outputs.len()
        )));
    }
    decode_u32_output_exact(&outputs[0], n as usize, "matroid_solve_step_fixed_via", out)
}

/// Iterate Jacobi smoothing until residual norm drops below `tol`
/// or `max_iters` reached. Returns `(x, iters_run)`.
///
/// The Tier-2.5 primitive ships the per-step kernel; the convergence
/// loop here is what production matroid-intersection callers want.
#[must_use]
#[cfg(test)]
pub fn solve_to_tolerance(
    a: &[f64],
    b: &[f64],
    x0: &[f64],
    omega: f64,
    n: u32,
    tol: f64,
    max_iters: u32,
) -> (Vec<f64>, u32) {
    let mut x = Vec::new();
    let mut next = Vec::new();
    let iters = solve_to_tolerance_into(a, b, x0, omega, n, tol, max_iters, &mut x, &mut next);
    (x, iters)
}

/// Iterate Jacobi smoothing into caller-owned buffers.
///
/// Returns the iteration count and leaves the final solution in `x`.
#[allow(clippy::too_many_arguments)]
#[cfg(test)]
pub fn solve_to_tolerance_into(
    a: &[f64],
    b: &[f64],
    x0: &[f64],
    omega: f64,
    n: u32,
    tol: f64,
    max_iters: u32,
    x: &mut Vec<f64>,
    next: &mut Vec<f64>,
) -> u32 {
    x.clear();
    x.extend_from_slice(x0);
    next.clear();
    let n_us = n as usize;
    for iter in 0..max_iters {
        reference_matroid_solve_step_into(a, b, x, omega, n, next);
        std::mem::swap(x, next);
        // Residual norm = ||Ax - b||_∞.
        let mut max_resid = 0.0_f64;
        for i in 0..n_us {
            let row_dot: f64 = (0..n_us).map(|j| a[i * n_us + j] * x[j]).sum();
            let r = (row_dot - b[i]).abs();
            if r > max_resid {
                max_resid = r;
            }
        }
        if max_resid < tol {
            return iter + 1;
        }
    }
    max_iters
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch_buffers::u32_slice_to_le_bytes;
    use vyre_foundation::ir::Program;

    fn approx_eq(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-4 * (1.0 + a.abs() + b.abs())
    }

    #[test]
    fn identity_system_converges_to_b() {
        // A = I, b = [1, 2, 3] → solution = b.
        let a = vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
        let b = vec![1.0, 2.0, 3.0];
        let x0 = vec![0.0; 3];
        let (x, iters) = solve_to_tolerance(&a, &b, &x0, 1.0, 3, 1e-6, 100);
        for (a, b) in x.iter().zip(b.iter()) {
            assert!(approx_eq(*a, *b));
        }
        assert!(iters <= 5, "identity converges in 1 step");
    }

    #[test]
    fn diagonally_dominant_system_converges() {
        // 2x2 system: 4x + y = 9, 2x + 5y = 8 → x=37/18 ≈ 2.0556, y=14/18 ≈ 0.7778.
        let a = vec![4.0, 1.0, 2.0, 5.0];
        let b = vec![9.0, 8.0];
        let x0 = vec![0.0, 0.0];
        let (x, _) = solve_to_tolerance(&a, &b, &x0, 0.66, 2, 1e-4, 1000);
        assert!(approx_eq(x[0], 37.0 / 18.0));
        assert!(approx_eq(x[1], 14.0 / 18.0));
    }

    #[test]
    fn zero_max_iters_returns_initial() {
        let a = vec![1.0, 0.0, 0.0, 1.0];
        let b = vec![5.0, 7.0];
        let x0 = vec![0.0, 0.0];
        let (x, iters) = solve_to_tolerance(&a, &b, &x0, 1.0, 2, 1e-6, 0);
        assert_eq!(x, x0);
        assert_eq!(iters, 0);
    }

    #[test]
    fn solve_to_tolerance_into_matches_owned_solver() {
        let a = vec![4.0, 1.0, 2.0, 5.0];
        let b = vec![9.0, 8.0];
        let x0 = vec![0.0, 0.0];
        let (owned, owned_iters) = solve_to_tolerance(&a, &b, &x0, 0.66, 2, 1e-4, 1000);
        let mut x = Vec::new();
        let mut next = Vec::new();
        let into_iters =
            solve_to_tolerance_into(&a, &b, &x0, 0.66, 2, 1e-4, 1000, &mut x, &mut next);
        assert_eq!(into_iters, owned_iters);
        assert_eq!(x.len(), owned.len());
        for (a, b) in x.iter().zip(owned.iter()) {
            assert!(approx_eq(*a, *b));
        }
    }

    #[test]
    fn matroid_solve_step_is_jacobi_iteration() {
        let a = vec![2.0, 0.0, 0.0, 2.0];
        let b = vec![6.0, 8.0];
        let x_in = vec![0.0, 0.0];
        let x_out = reference_matroid_solve_step(&a, &b, &x_in, 1.0, 2);
        // Pure Jacobi step with x_in = 0 yields x_out = b/diag.
        assert!(approx_eq(x_out[0], 3.0));
        assert!(approx_eq(x_out[1], 4.0));
    }

    #[test]
    fn reference_step_matches_foundation_authority_generated() {
        for n in 1..5usize {
            let mut a = vec![0.0; n * n];
            let mut b = vec![0.0; n];
            let mut x = vec![0.0; n];
            for i in 0..n {
                b[i] = (i as f64 + 1.0) * 1.5;
                x[i] = i as f64 * 0.25;
                for j in 0..n {
                    a[i * n + j] = if i == j {
                        (n + i + 2) as f64
                    } else {
                        ((i + j) % 3) as f64 * 0.125
                    };
                }
            }

            let reference = reference_matroid_solve_step(&a, &b, &x, 0.66, n as u32);
            let authority = foundation_multigrid::matroid_solve_step(&a, &b, &x, 0.66, n as u32);

            assert_eq!(reference.len(), authority.len());
            for (reference, authority) in reference.iter().zip(authority.iter()) {
                assert!(approx_eq(*reference, *authority));
            }
        }
    }

    #[test]
    fn reference_step_into_reuses_output() {
        let a = vec![2.0, 0.0, 0.0, 4.0];
        let b = vec![4.0, 8.0];
        let x = vec![0.0, 0.0];
        let mut out = Vec::with_capacity(16);
        out.extend_from_slice(&[99.0, 98.0, 97.0]);
        let capacity = out.capacity();

        reference_matroid_solve_step_into(&a, &b, &x, 0.5, 2, &mut out);

        assert_eq!(out, vec![1.0, 1.0]);
        assert_eq!(out.capacity(), capacity);
    }

    #[test]
    fn reference_step_handles_sparse_missing_entries_without_panicking() {
        let reference = reference_matroid_solve_step(&[2.0], &[4.0], &[1.0], 0.5, 2);
        let authority = foundation_multigrid::matroid_solve_step(&[2.0], &[4.0], &[1.0], 0.5, 2);

        assert_eq!(reference, authority);
        assert_eq!(reference, vec![1.5, 0.0]);
    }

    struct JacobiDispatcher;

    impl OptimizerDispatcher for JacobiDispatcher {
        fn dispatch(
            &self,
            _program: &Program,
            inputs: &[Vec<u8>],
            grid_override: Option<[u32; 3]>,
        ) -> Result<Vec<Vec<u8>>, DispatchError> {
            assert_eq!(grid_override, Some([1, 1, 1]));
            assert_eq!(inputs.len(), 5);
            let a = crate::hardware::dispatch_buffers::read_u32s(&inputs[0]);
            let b = crate::hardware::dispatch_buffers::read_u32s(&inputs[1]);
            let x_in = crate::hardware::dispatch_buffers::read_u32s(&inputs[2]);
            let omega = crate::hardware::dispatch_buffers::read_u32s(&inputs[3])[0];
            assert_eq!(inputs[4].len(), b.len() * std::mem::size_of::<u32>());
            let n = b.len();
            let mut out = Vec::with_capacity(n);
            for i in 0..n {
                let mut ax = 0u32;
                for j in 0..n {
                    ax = ax.saturating_add(((a[i * n + j] as u64 * x_in[j] as u64) >> 16) as u32);
                }
                let res = b[i].saturating_sub(ax);
                let diag = a[i * n + i].max(1);
                let omega_res = ((omega as u64 * res as u64) >> 16) as u32;
                out.push(x_in[i].saturating_add((((omega_res as u64) << 16) / diag as u64) as u32));
            }

            Ok(vec![u32_slice_to_le_bytes(&out)])
        }
    }

    #[test]
    fn fixed_via_dispatches_jacobi_step() {
        let one = 1u32 << 16;
        let out = matroid_solve_step_fixed_via(
            &JacobiDispatcher,
            &[one, 0, 0, one],
            &[3 * one, 4 * one],
            &[0, 0],
            one,
            2,
        )
        .unwrap();
        assert_eq!(out, vec![3 * one, 4 * one]);
    }

    #[test]
    fn fixed_via_rejects_bad_shapes() {
        let err =
            matroid_solve_step_fixed_via(&JacobiDispatcher, &[1, 0, 0], &[1, 1], &[0, 0], 1, 2)
                .unwrap_err();
        assert!(matches!(err, DispatchError::BadInputs(_)));
    }

    #[test]
    fn fixed_via_with_scratch_reuses_input_buffers() {
        let one = 1u32 << 16;
        let mut scratch = MultigridMatroidGpuScratch::default();
        let mut out = Vec::new();

        matroid_solve_step_fixed_via_with_scratch_into(
            &JacobiDispatcher,
            &[one, 0, 0, one],
            &[3 * one, 4 * one],
            &[0, 0],
            one,
            2,
            &mut scratch,
            &mut out,
        )
        .unwrap();
        let input_ptrs: Vec<*const u8> = scratch.inputs.iter().map(Vec::as_ptr).collect();
        matroid_solve_step_fixed_via_with_scratch_into(
            &JacobiDispatcher,
            &[one, 0, 0, one],
            &[2 * one, 5 * one],
            &[0, 0],
            one,
            2,
            &mut scratch,
            &mut out,
        )
        .unwrap();

        for (before, after) in input_ptrs
            .iter()
            .zip(scratch.inputs.iter().map(Vec::as_ptr))
        {
            assert_eq!(*before, after);
        }
    }

    #[test]
    fn production_source_keeps_cpu_multigrid_helpers_out_of_via_path() {
        let source = include_str!("multigrid_matroid_solver.rs");
        let via_section = source
            .split("pub fn matroid_solve_step_fixed_via")
            .nth(1)
            .expect("Fix: via section should exist")
            .split("/// Iterate Jacobi smoothing until residual norm drops below `tol`")
            .next()
            .expect("Fix: post-via marker should exist");

        assert!(!via_section.contains("_cpu"));
        assert!(!via_section.contains("reference_matroid"));
    }
}
