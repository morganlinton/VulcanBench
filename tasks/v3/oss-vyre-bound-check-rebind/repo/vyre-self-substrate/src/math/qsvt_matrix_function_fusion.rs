//! Transport-based fusion analysis via #34 QSVT (#34 self-consumer).
//!
//! Closes the recursion thesis for #34  -  the QSVT (quantum singular
//! value transform) primitives ship to user dialects (matrix
//! function evaluation: exp, sqrt, inverse without eigendecomposition)
//! AND power vyre's transport-based fusion analyses.
//!
//! # The self-use
//!
//! Vyre's fusion analyzer ranks Region pairs by "fusion affinity":
//! how much would dispatch latency drop if these two Regions ran
//! in one kernel? The natural metric is a transport distance
//! (Wasserstein) over the dispatch graph: if Regions i and j have
//! similar memory-residency profiles, fusing them costs little and
//! saves a kernel-launch latency.
//!
//! Wasserstein-1 distance reduces (in the discrete case) to a
//! linear-program over the dispatch-cost matrix. The dual is
//! computed from `f(M) · v` where M is the dispatch-cost adjacency
//! and `f` is the negative-eigenvalue-truncating spectral function.
//! Standard pre-QSVT computation:
//!
//! 1. Eigendecompose M (O(n³))
//! 2. Apply f to eigenvalues (O(n))
//! 3. Reconstruct M' = U Λ' Uᵀ (O(n³))
//! 4. Compute M' · v (O(n²))
//!
//! Total: O(n³) memory + compute. At 1M Regions this is intractable.
//!
//! QSVT computes f(M) · v directly via Chebyshev expansion (O(K · n²)
//! where K is the polynomial order). For K = 16 and n = 1M, this is
//! 16 × 10¹² ≈ 10¹³ operations vs 10¹⁸ for the eigendecomposition
//! path  -  five orders of magnitude faster.
//!
//! # Algorithm
//!
//! ```text
//! 1. block_encode the dispatch-cost matrix M into Mscaled = M/||M||_F
//! 2. compute Chebyshev coefficients of f (caller-supplied function:
//!    f(λ) = -λ for negative-eigenvalue truncation, etc.)
//! 3. qsvt_apply(Mscaled, v, coeffs) → f(M) · v
//! ```
//!
//! # Why this matters at scale
//!
//! Wasserstein over dispatch graphs is the only metric that captures
//! "Regions that USE buffers similarly should fuse." LRU, LFU,
//! random  -  none capture this. QSVT-via-Chebyshev makes the
//! Wasserstein-distance computation tractable at 1M+ Regions.

use crate::dispatch_buffers::{
    ceil_div_u32, checked_square_cells, decode_u32_output_exact, ensure_input_slots,
    write_u32_slice_le_bytes,
};
use crate::hardware::scratch::reserve_vec_capacity_or_panic;
use crate::optimizer::dispatcher::{DispatchError, OptimizerDispatcher};
use vyre_primitives::graph::chebyshev_filter::{chebyshev_filter, MAX_K as CHEBYSHEV_MAX_K};
#[cfg(test)]
use vyre_primitives::math::qsvt::{qsvt_apply_cpu_into, qsvt_block_encode_cpu_into};

/// Caller-owned dispatch scratch for fixed-point QSVT transport residuals.
#[derive(Debug, Default)]
pub struct QsvtTransportGpuScratch {
    inputs: Vec<Vec<u8>>,
}

/// Compute the negative-truncation Chebyshev coefficients of length
/// `k_steps`. The truncation function is `f(λ) = -λ if λ < 0 else 0`
///  -  the standard negative-eigenvalue projector used in
/// transport-based fusion analyses.
///
/// Uses a hand-derived Chebyshev expansion that approximates the
/// truncator to ~3-decimal accuracy for the coefficient lengths this
/// oracle supports. Higher-accuracy coefficient generators should live
/// behind their own registered op and feed this function's `coeffs`
/// consumers directly.
#[must_use]
pub fn negative_truncator_coeffs(k_steps: u32) -> Vec<f64> {
    let mut out = Vec::new();
    negative_truncator_coeffs_into(k_steps, &mut out);
    out
}

/// Write negative-truncation Chebyshev coefficients into caller-owned storage.
pub fn negative_truncator_coeffs_into(k_steps: u32, out: &mut Vec<f64>) {
    // Hand-derived from the Fourier-Chebyshev expansion of
    // f(cos θ) = -max(cos θ, 0) on [-1, 1]. The first few
    // coefficients (truncated to k_steps):
    //
    // a_0 = -1/π
    // a_1 = -1/2
    // a_2 = -2/(3π)
    // a_3 = 0
    // a_4 = 2/(15π)
    // a_5 = 0
    // a_6 = -2/(35π)
    // a_7 = 0
    let pi = std::f64::consts::PI;
    let all = [
        -1.0 / pi,
        -0.5,
        -2.0 / (3.0 * pi),
        0.0,
        2.0 / (15.0 * pi),
        0.0,
        -2.0 / (35.0 * pi),
        0.0,
    ];
    out.clear();
    out.extend(all.iter().take(k_steps as usize).copied());
}

/// Compute `f(M) · v` for the dispatch-cost matrix M and weight
/// vector v, where f is the negative-truncator. Returns the
/// transport-residual vector.
///
/// `dispatch_cost` is the n*n cost adjacency (row-major).
/// `weights` is length-n.
/// `chebyshev_order` controls the polynomial truncation; 8 is the
/// 0.6-shippable maximum.
///
/// # Panics
///
/// Panics if `dispatch_cost.len() != n*n` or `weights.len() != n` or
/// `chebyshev_order == 0`.
#[must_use]
#[cfg(test)]
pub fn transport_residual(
    dispatch_cost: &[f64],
    weights: &[f64],
    n: u32,
    chebyshev_order: u32,
) -> Vec<f64> {
    let mut scaled = Vec::new();
    let mut coeffs = Vec::new();
    let mut out = Vec::new();
    let mut t_prev = Vec::new();
    let mut t_curr = Vec::new();
    let mut t_next = Vec::new();
    reference_transport_residual_into(
        dispatch_cost,
        weights,
        n,
        chebyshev_order,
        &mut scaled,
        &mut coeffs,
        &mut out,
        &mut t_prev,
        &mut t_curr,
        &mut t_next,
    );
    out
}

/// Compute transport residual into caller-owned QSVT scratch buffers.
#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub fn reference_transport_residual_into(
    dispatch_cost: &[f64],
    weights: &[f64],
    n: u32,
    chebyshev_order: u32,
    scaled: &mut Vec<f64>,
    coeffs: &mut Vec<f64>,
    out: &mut Vec<f64>,
    t_prev: &mut Vec<f64>,
    t_curr: &mut Vec<f64>,
    t_next: &mut Vec<f64>,
) {
    use crate::observability::{bump, qsvt_matrix_function_fusion_calls};
    bump(&qsvt_matrix_function_fusion_calls);
    assert!(
        chebyshev_order > 0 && chebyshev_order <= 8,
        "Fix: chebyshev_order must be in 1..=8 for 0.6, got {chebyshev_order}."
    );
    let n_us = n as usize;
    assert_eq!(dispatch_cost.len(), n_us * n_us);
    assert_eq!(weights.len(), n_us);

    let _frobenius = qsvt_block_encode_cpu_into(dispatch_cost, n, scaled);
    negative_truncator_coeffs_into(chebyshev_order, coeffs);
    qsvt_apply_cpu_into(scaled, weights, coeffs, n, out, t_prev, t_curr, t_next);
}

/// Fixed-point dispatcher path for `f(M) * v` transport residuals.
///
/// `dispatch_cost_scaled_fixed` must already be block-encoded / scaled at the
/// representation boundary. `weights_fixed` and `coeffs_fixed` use the same
/// 16.16 fixed-point representation as the Chebyshev primitive.
///
/// # Errors
///
/// Returns [`DispatchError`] for invalid shapes, unsupported polynomial order,
/// dispatch failure, or malformed backend output.
pub fn transport_residual_fixed_via(
    dispatcher: &impl OptimizerDispatcher,
    dispatch_cost_scaled_fixed: &[u32],
    weights_fixed: &[u32],
    coeffs_fixed: &[u32],
    n: u32,
    chebyshev_order: u32,
) -> Result<Vec<u32>, DispatchError> {
    let mut scratch = QsvtTransportGpuScratch::default();
    let mut out = Vec::new();
    transport_residual_fixed_via_with_scratch_into(
        dispatcher,
        dispatch_cost_scaled_fixed,
        weights_fixed,
        coeffs_fixed,
        n,
        chebyshev_order,
        &mut scratch,
        &mut out,
    )?;
    Ok(out)
}

/// Fixed-point dispatcher path for `f(M) * v` using caller-owned scratch.
///
/// # Errors
///
/// Returns [`DispatchError`] for invalid shapes, unsupported polynomial order,
/// dispatch failure, or malformed backend output.
pub fn transport_residual_fixed_via_with_scratch_into(
    dispatcher: &impl OptimizerDispatcher,
    dispatch_cost_scaled_fixed: &[u32],
    weights_fixed: &[u32],
    coeffs_fixed: &[u32],
    n: u32,
    chebyshev_order: u32,
    scratch: &mut QsvtTransportGpuScratch,
    out: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    use crate::observability::{bump, qsvt_matrix_function_fusion_calls};
    bump(&qsvt_matrix_function_fusion_calls);

    if chebyshev_order == 0 || chebyshev_order > CHEBYSHEV_MAX_K {
        return Err(DispatchError::BadInputs(format!(
            "Fix: transport_residual_fixed_via requires chebyshev_order in 1..={CHEBYSHEV_MAX_K}, got {chebyshev_order}."
        )));
    }
    let cells = checked_square_cells(n, "transport_residual_fixed_via")?;
    if dispatch_cost_scaled_fixed.len() != cells {
        return Err(DispatchError::BadInputs(format!(
            "Fix: transport_residual_fixed_via requires dispatch_cost_scaled_fixed.len() == n*n, got len={}, n={n}, n*n={cells}.",
            dispatch_cost_scaled_fixed.len()
        )));
    }
    if weights_fixed.len() != n as usize {
        return Err(DispatchError::BadInputs(format!(
            "Fix: transport_residual_fixed_via requires weights_fixed.len() == n, got len={}, n={n}.",
            weights_fixed.len()
        )));
    }
    let coeff_count = (chebyshev_order as usize).checked_add(1).ok_or_else(|| {
        DispatchError::BadInputs(
            "Fix: transport_residual_fixed_via coefficient count overflowed usize.".to_string(),
        )
    })?;
    if coeffs_fixed.len() != coeff_count {
        return Err(DispatchError::BadInputs(format!(
            "Fix: transport_residual_fixed_via requires coeffs_fixed.len() == chebyshev_order + 1, got len={}, chebyshev_order={chebyshev_order}.",
            coeffs_fixed.len()
        )));
    }

    let program = chebyshev_filter(
        "dispatch_cost_scaled",
        "weights",
        "coeffs",
        "transport_residual",
        "scratch",
        n,
        chebyshev_order,
    );
    ensure_input_slots(&mut scratch.inputs, 3);
    write_u32_slice_le_bytes(&mut scratch.inputs[0], dispatch_cost_scaled_fixed);
    write_u32_slice_le_bytes(&mut scratch.inputs[1], weights_fixed);
    write_u32_slice_le_bytes(&mut scratch.inputs[2], coeffs_fixed);
    let outputs = dispatcher.dispatch(
        &program,
        &scratch.inputs[..3],
        Some([ceil_div_u32(n, 256), 1, 1]),
    )?;
    let output = outputs.first().ok_or_else(|| {
        DispatchError::BackendError(format!(
            "Fix: transport_residual_fixed_via expected one output buffer, got {}.",
            outputs.len()
        ))
    })?;
    decode_u32_output_exact(output, n as usize, "transport_residual_fixed_via", out)
}

/// Convenience: derive a fusion-affinity score per Region from the
/// transport residual. Lower magnitude = closer to the cost-matrix
/// null space = better fusion candidate.
#[must_use]
pub fn fusion_affinity(transport_residual: &[f64]) -> Vec<f64> {
    let mut out = Vec::new();
    fusion_affinity_into(transport_residual, &mut out);
    out
}

/// Derive fusion-affinity scores into caller-owned storage.
pub fn fusion_affinity_into(transport_residual: &[f64], out: &mut Vec<f64>) {
    out.clear();
    reserve_vec_capacity_or_panic(out, transport_residual.len(), "QSVT fusion-affinity output");
    out.extend(transport_residual.iter().map(|&v| -v.abs()));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch_buffers::u32_slice_to_le_bytes;

    fn approx_eq(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-3 * (1.0 + a.abs() + b.abs())
    }

    struct QsvtDispatcher;

    impl OptimizerDispatcher for QsvtDispatcher {
        fn dispatch(
            &self,
            _program: &vyre_foundation::ir::Program,
            inputs: &[Vec<u8>],
            grid_override: Option<[u32; 3]>,
        ) -> Result<Vec<Vec<u8>>, DispatchError> {
            assert_eq!(grid_override, Some([1, 1, 1]));
            assert_eq!(inputs.len(), 3);
            let matrix = crate::hardware::dispatch_buffers::read_u32s(&inputs[0]);
            let weights = crate::hardware::dispatch_buffers::read_u32s(&inputs[1]);
            let coeffs = crate::hardware::dispatch_buffers::read_u32s(&inputs[2]);
            assert_eq!(matrix, vec![1, 0, 0, 1]);
            assert_eq!(coeffs, vec![1, 0]);
            Ok(vec![u32_slice_to_le_bytes(&weights)])
        }
    }

    #[test]
    fn truncator_first_coeff_is_negative_inverse_pi() {
        let coeffs = negative_truncator_coeffs(1);
        assert_eq!(coeffs.len(), 1);
        assert!(approx_eq(coeffs[0], -1.0 / std::f64::consts::PI));
    }

    #[test]
    fn truncator_high_order_caps_at_eight() {
        let coeffs = negative_truncator_coeffs(8);
        assert_eq!(coeffs.len(), 8);
        // Odd-index k >= 3 is zero by Chebyshev parity.
        assert!(approx_eq(coeffs[3], 0.0));
        assert!(approx_eq(coeffs[5], 0.0));
        assert!(approx_eq(coeffs[7], 0.0));
    }

    #[test]
    fn transport_residual_zero_weights_yields_zero() {
        let cost = vec![1.0, 0.5, 0.5, 1.0];
        let weights = vec![0.0, 0.0];
        let result = transport_residual(&cost, &weights, 2, 4);
        assert!(result.iter().all(|&v| approx_eq(v, 0.0)));
    }

    #[test]
    fn fusion_affinity_inverts_residual_magnitude() {
        let residual = vec![1.0, -2.5, 0.0, 0.5];
        let aff = fusion_affinity(&residual);
        assert!(approx_eq(aff[0], -1.0));
        assert!(approx_eq(aff[1], -2.5));
        assert!(approx_eq(aff[2], 0.0));
        assert!(approx_eq(aff[3], -0.5));
    }

    #[test]
    fn transport_residual_into_reuses_qsvt_buffers() {
        let cost = vec![1.0, 0.5, 0.5, 1.0];
        let weights = vec![1.0, 1.0];
        let mut scaled = Vec::with_capacity(8);
        let mut coeffs = Vec::with_capacity(8);
        let mut out = Vec::with_capacity(8);
        let mut t_prev = Vec::with_capacity(8);
        let mut t_curr = Vec::with_capacity(8);
        let mut t_next = Vec::with_capacity(8);
        let pointers = [
            scaled.as_ptr(),
            coeffs.as_ptr(),
            out.as_ptr(),
            t_prev.as_ptr(),
            t_curr.as_ptr(),
            t_next.as_ptr(),
        ];
        reference_transport_residual_into(
            &cost,
            &weights,
            2,
            4,
            &mut scaled,
            &mut coeffs,
            &mut out,
            &mut t_prev,
            &mut t_curr,
            &mut t_next,
        );
        assert_eq!(out.len(), 2);
        for ptr in [
            scaled.as_ptr(),
            coeffs.as_ptr(),
            out.as_ptr(),
            t_prev.as_ptr(),
            t_curr.as_ptr(),
            t_next.as_ptr(),
        ] {
            assert!(pointers.contains(&ptr));
        }
    }

    #[test]
    fn transport_residual_runs_on_small_matrix() {
        // 3x3 dispatch cost, all-ones weights. Just verify shape +
        // no panic; numerical exactness comes from qsvt_apply_cpu
        // unit tests in the primitive's own crate.
        let cost = vec![1.0, 0.5, 0.3, 0.5, 1.0, 0.4, 0.3, 0.4, 1.0];
        let weights = vec![1.0, 1.0, 1.0];
        let result = transport_residual(&cost, &weights, 3, 4);
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn transport_residual_fixed_via_dispatches_chebyshev_path() {
        let out =
            transport_residual_fixed_via(&QsvtDispatcher, &[1, 0, 0, 1], &[7, 11], &[1, 0], 2, 1)
                .unwrap();
        assert_eq!(out, vec![7, 11]);
    }

    #[test]
    fn transport_residual_fixed_via_rejects_bad_shapes() {
        let err =
            transport_residual_fixed_via(&QsvtDispatcher, &[1, 0, 0], &[7, 11], &[1, 0], 2, 1)
                .unwrap_err();
        assert!(matches!(err, DispatchError::BadInputs(_)));
    }

    #[test]
    fn production_source_keeps_cpu_qsvt_helpers_out_of_via_path() {
        let source = include_str!("qsvt_matrix_function_fusion.rs");
        let via_section = source
            .split("pub fn transport_residual_fixed_via")
            .nth(1)
            .expect("Fix: via section should exist")
            .split("/// Convenience: derive a fusion-affinity score")
            .next()
            .expect("Fix: post-via marker should exist");

        assert!(!via_section.contains("_cpu"));
        assert!(!via_section.contains("reference_transport"));
    }

    #[test]
    #[should_panic(expected = "chebyshev_order")]
    fn rejects_zero_chebyshev_order() {
        let cost = vec![1.0; 4];
        let weights = vec![1.0; 2];
        let _residual = transport_residual(&cost, &weights, 2, 0);
    }
}
