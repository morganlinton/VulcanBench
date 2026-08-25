//! Differentiable autotuner via #7 differentiable softmax / argmax (#27).
//!
//! Picks workgroup-size / tile-shape / fusion-threshold via gradient
//! descent over a smoothed argmax of cost-model scores. Same softmax
//! primitive that user attention dialects use; here it picks the
//! best dispatch configuration.

use super::natural_gradient_autotuner::{
    precondition_autotune_gradient_fixed_via_with_scratch_into, NaturalGradientGpuScratch,
};
use crate::dispatch_buffers::{
    ceil_div_u32, decode_u32_output_exact, ensure_input_slots, write_u32_slice_le_bytes,
    write_zero_bytes,
};
#[cfg(test)]
use crate::hardware::scratch::reserve_vec_capacity_or_panic;
use crate::optimizer::dispatcher::{DispatchError, OptimizerDispatcher};
use vyre_primitives::math::differentiable::softmax_step;
#[cfg(test)]
use vyre_primitives::math::differentiable::{differentiable_argmax_cpu_into, softmax_cpu_into};

/// Caller-owned scratch for fixed-point differentiable-autotune dispatch.
#[derive(Debug, Default)]
pub struct DifferentiableAutotuneGpuScratch {
    inputs: Vec<Vec<u8>>,
}

/// Caller-owned scratch for Fisher-preconditioned differentiable autotuning.
#[derive(Debug, Default)]
pub struct NaturalDifferentiableAutotuneGpuScratch {
    softmax: DifferentiableAutotuneGpuScratch,
    fisher: NaturalGradientGpuScratch,
    probabilities: Vec<u32>,
}

/// Soft-pick configuration probabilities from pre-exponentiated fixed-point
/// costs through the dispatch backend.
///
/// `pre_exp_fixed[i]` is `exp(-cost[i] / temperature)` in 16.16 fixed-point.
/// The temperature/exp stage is intentionally composed before this primitive
/// so CUDA callers can fuse it with their own cost-model kernel.
///
/// # Errors
///
/// Returns [`DispatchError`] when there are no candidates, the candidate count
/// cannot be represented by the primitive, or the backend returns malformed
/// output.
pub fn pick_config_pre_exp_fixed_via(
    dispatcher: &impl OptimizerDispatcher,
    pre_exp_fixed: &[u32],
) -> Result<Vec<u32>, DispatchError> {
    let mut scratch = DifferentiableAutotuneGpuScratch::default();
    let mut out = Vec::new();
    pick_config_pre_exp_fixed_via_with_scratch_into(
        dispatcher,
        pre_exp_fixed,
        &mut scratch,
        &mut out,
    )?;
    Ok(out)
}

/// Soft-pick configuration probabilities into caller-owned output storage.
///
/// # Errors
///
/// Returns [`DispatchError`] when there are no candidates, the candidate count
/// cannot be represented by the primitive, or the backend returns malformed
/// output.
pub fn pick_config_pre_exp_fixed_via_with_scratch_into(
    dispatcher: &impl OptimizerDispatcher,
    pre_exp_fixed: &[u32],
    scratch: &mut DifferentiableAutotuneGpuScratch,
    out: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    if pre_exp_fixed.is_empty() {
        return Err(DispatchError::BadInputs(
            "Fix: pick_config_pre_exp_fixed_via requires at least one candidate.".to_string(),
        ));
    }
    let n = u32::try_from(pre_exp_fixed.len()).map_err(|_| {
        DispatchError::BadInputs(format!(
            "Fix: pick_config_pre_exp_fixed_via candidate count {} exceeds u32::MAX.",
            pre_exp_fixed.len()
        ))
    })?;
    let output_bytes = pre_exp_fixed
        .len()
        .checked_mul(std::mem::size_of::<u32>())
        .ok_or_else(|| {
            DispatchError::BadInputs(format!(
                "Fix: pick_config_pre_exp_fixed_via output byte count overflows usize for {} candidates.",
                pre_exp_fixed.len()
            ))
        })?;

    let program = softmax_step("pre_exp", "out", n);
    ensure_input_slots(&mut scratch.inputs, 2);
    write_u32_slice_le_bytes(&mut scratch.inputs[0], pre_exp_fixed);
    write_zero_bytes(&mut scratch.inputs[1], output_bytes);
    let outputs = dispatcher.dispatch(
        &program,
        &scratch.inputs[..2],
        Some([ceil_div_u32(n, 256), 1, 1]),
    )?;
    if outputs.is_empty() {
        return Err(DispatchError::BackendError(format!(
            "Fix: pick_config_pre_exp_fixed_via expected one output buffer, got {}.",
            outputs.len()
        )));
    }
    decode_u32_output_exact(
        &outputs[0],
        pre_exp_fixed.len(),
        "pick_config_pre_exp_fixed_via",
        out,
    )
}

/// Return fixed-point gradient magnitudes for the soft-picked cost.
///
/// The mathematical gradient with respect to each cost is `-softmax_i`.
/// This unsigned fixed-point path returns the magnitudes; callers that need a
/// signed representation attach the negative sign at the consuming fused stage.
///
/// # Errors
///
/// Returns [`DispatchError`] under the same conditions as
/// [`pick_config_pre_exp_fixed_via`].
pub fn config_gradient_magnitude_pre_exp_fixed_via(
    dispatcher: &impl OptimizerDispatcher,
    pre_exp_fixed: &[u32],
) -> Result<Vec<u32>, DispatchError> {
    pick_config_pre_exp_fixed_via(dispatcher, pre_exp_fixed)
}

/// Return fixed-point gradient magnitudes into caller-owned output storage.
///
/// # Errors
///
/// Returns [`DispatchError`] under the same conditions as
/// [`pick_config_pre_exp_fixed_via_with_scratch_into`].
pub fn config_gradient_magnitude_pre_exp_fixed_via_with_scratch_into(
    dispatcher: &impl OptimizerDispatcher,
    pre_exp_fixed: &[u32],
    scratch: &mut DifferentiableAutotuneGpuScratch,
    out: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    pick_config_pre_exp_fixed_via_with_scratch_into(dispatcher, pre_exp_fixed, scratch, out)
}

/// Compute the Fisher-preconditioned fixed-point autotune gradient.
///
/// This composes the differentiable policy primitive with the natural-gradient
/// primitive in the release path:
///
/// `pre_exp_fixed -> softmax policy gradient -> M_inv_sqrt * gradient`
///
/// `m_inv_sqrt_fixed` is an `n x n` row-major 16.16 inverse-Fisher square-root
/// matrix and `pre_exp_fixed.len() == n`.
///
/// # Errors
///
/// Returns [`DispatchError`] when candidate counts overflow, the Fisher block
/// has the wrong shape, or backend execution/readback fails.
pub fn natural_config_gradient_magnitude_pre_exp_fixed_via(
    dispatcher: &impl OptimizerDispatcher,
    pre_exp_fixed: &[u32],
    m_inv_sqrt_fixed: &[u32],
) -> Result<Vec<u32>, DispatchError> {
    let mut scratch = NaturalDifferentiableAutotuneGpuScratch::default();
    let mut out = Vec::new();
    natural_config_gradient_magnitude_pre_exp_fixed_via_with_scratch_into(
        dispatcher,
        pre_exp_fixed,
        m_inv_sqrt_fixed,
        &mut scratch,
        &mut out,
    )?;
    Ok(out)
}

/// Compute the Fisher-preconditioned fixed-point autotune gradient into
/// caller-owned output storage.
///
/// # Errors
///
/// Returns [`DispatchError`] when validation or backend execution fails.
pub fn natural_config_gradient_magnitude_pre_exp_fixed_via_with_scratch_into(
    dispatcher: &impl OptimizerDispatcher,
    pre_exp_fixed: &[u32],
    m_inv_sqrt_fixed: &[u32],
    scratch: &mut NaturalDifferentiableAutotuneGpuScratch,
    out: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    let n = u32::try_from(pre_exp_fixed.len()).map_err(|_| {
        DispatchError::BadInputs(format!(
            "Fix: natural_config_gradient_magnitude_pre_exp_fixed_via candidate count {} exceeds u32::MAX.",
            pre_exp_fixed.len()
        ))
    })?;
    pick_config_pre_exp_fixed_via_with_scratch_into(
        dispatcher,
        pre_exp_fixed,
        &mut scratch.softmax,
        &mut scratch.probabilities,
    )?;
    precondition_autotune_gradient_fixed_via_with_scratch_into(
        dispatcher,
        m_inv_sqrt_fixed,
        &scratch.probabilities,
        n,
        &mut scratch.fisher,
        out,
    )
}

/// Soft-pick the best configuration index given per-config cost
/// scores (lower cost = better). Returns probabilities that sum to 1;
/// at low temperature the argmax dominates.
#[must_use]
#[cfg(test)]
pub fn pick_config(costs: &[f64], temperature: f64) -> Vec<f64> {
    let mut neg_costs = Vec::new();
    let mut scaled = Vec::new();
    let mut out = Vec::new();
    reference_pick_config_into(costs, temperature, &mut neg_costs, &mut scaled, &mut out);
    out
}

/// Soft-pick into caller-owned scratch and probability buffers.
#[cfg(test)]
pub fn reference_pick_config_into(
    costs: &[f64],
    temperature: f64,
    neg_costs: &mut Vec<f64>,
    scaled: &mut Vec<f64>,
    out: &mut Vec<f64>,
) {
    use crate::observability::{bump, differentiable_autotune_calls};
    bump(&differentiable_autotune_calls);
    // Negate costs so higher input = better config.
    neg_costs.clear();
    reserve_vec_capacity_or_panic(
        neg_costs,
        costs.len(),
        "differentiable autotune negated costs",
    );
    neg_costs.extend(costs.iter().map(|&c| -c));
    differentiable_argmax_cpu_into(neg_costs, temperature, scaled, out);
}

/// Hard pick the best configuration: take the argmax of cost scores.
#[must_use]
#[cfg(test)]
pub fn pick_best_config(costs: &[f64]) -> usize {
    assert!(
        !costs.is_empty(),
        "Fix: pick_best_config requires at least one candidate."
    );
    let mut best = 0usize;
    let mut best_cost = costs[0];
    for (i, &cost) in costs.iter().enumerate().skip(1) {
        if cost.total_cmp(&best_cost).is_lt() {
            best = i;
            best_cost = cost;
        }
    }
    best
}

/// Compute the gradient of the soft-picked cost w.r.t. each config
/// score. Useful for end-to-end training with the cost model. The
/// `temperature` parameter scales softmax sharpness; lower → gradient
/// concentrates on the argmin.
#[must_use]
#[cfg(test)]
pub fn config_gradient(costs: &[f64], temperature: f64) -> Vec<f64> {
    let mut neg_costs = Vec::new();
    let mut out = Vec::new();
    reference_config_gradient_into(costs, temperature, &mut neg_costs, &mut out);
    out
}

/// Compute the config-score gradient into caller-owned storage.
#[cfg(test)]
pub fn reference_config_gradient_into(
    costs: &[f64],
    temperature: f64,
    neg_costs: &mut Vec<f64>,
    out: &mut Vec<f64>,
) {
    assert!(temperature > 0.0, "temperature must be positive");
    // d softmax / d cost_i = -softmax_i (since costs are negated).
    neg_costs.clear();
    reserve_vec_capacity_or_panic(
        neg_costs,
        costs.len(),
        "differentiable autotune gradient negated costs",
    );
    neg_costs.extend(costs.iter().map(|&c| -c / temperature));
    softmax_cpu_into(neg_costs, out);
    for value in out.iter_mut() {
        *value = -*value;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_foundation::ir::Program;

    fn u32_slice_to_le_bytes(values: &[u32]) -> Vec<u8> {
        vyre_primitives::wire::pack_u32_slice(values)
    }

    struct DifferentiableDispatcher;

    impl OptimizerDispatcher for DifferentiableDispatcher {
        fn dispatch(
            &self,
            _program: &Program,
            inputs: &[Vec<u8>],
            grid_override: Option<[u32; 3]>,
        ) -> Result<Vec<Vec<u8>>, DispatchError> {
            assert_eq!(grid_override, Some([1, 1, 1]));
            match inputs.len() {
                2 => {
                    let pre_exp = crate::hardware::dispatch_buffers::read_u32s(&inputs[0]);
                    let output_seed = crate::hardware::dispatch_buffers::read_u32s(&inputs[1]);
                    assert_eq!(output_seed, vec![0; pre_exp.len()]);
                    let sum: u64 = pre_exp.iter().map(|&value| u64::from(value)).sum();
                    let sum = sum.max(1);
                    let probabilities: Vec<u32> = pre_exp
                        .iter()
                        .map(|&value| ((u64::from(value) << 16) / sum) as u32)
                        .collect();
                    Ok(vec![u32_slice_to_le_bytes(&probabilities)])
                }
                3 => {
                    let matrix = crate::hardware::dispatch_buffers::read_u32s(&inputs[0]);
                    let grad = crate::hardware::dispatch_buffers::read_u32s(&inputs[1]);
                    assert_eq!(inputs[2].len(), grad.len() * std::mem::size_of::<u32>());
                    let n = grad.len();
                    assert_eq!(matrix.len(), n * n);
                    let mut out = vec![0u32; n];
                    for i in 0..n {
                        let mut acc = 0u64;
                        for j in 0..n {
                            acc = acc.wrapping_add(
                                ((u64::from(matrix[i * n + j])) * u64::from(grad[j])) >> 16,
                            );
                        }
                        out[i] = acc as u32;
                    }
                    Ok(vec![u32_slice_to_le_bytes(&out)])
                }
                other => {
                    panic!("Fix: unexpected differentiable autotune dispatch input count {other}.")
                }
            }
        }
    }

    fn approx_eq(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-4 * (1.0 + a.abs() + b.abs())
    }

    #[test]
    fn fixed_pick_config_normalizes_pre_exp_weights() {
        let out =
            pick_config_pre_exp_fixed_via(&DifferentiableDispatcher, &[65_536, 131_072, 65_536])
                .expect("Fix: dispatch should normalize pre-exp weights");
        assert_eq!(out, vec![16_384, 32_768, 16_384]);
    }

    #[test]
    fn fixed_pick_config_reuses_buffers() {
        let mut scratch = DifferentiableAutotuneGpuScratch {
            inputs: vec![Vec::with_capacity(64), Vec::with_capacity(64)],
        };
        let mut out = Vec::with_capacity(8);
        let first_input_ptr = scratch.inputs[0].as_ptr();
        let second_input_ptr = scratch.inputs[1].as_ptr();
        let out_ptr = out.as_ptr();
        pick_config_pre_exp_fixed_via_with_scratch_into(
            &DifferentiableDispatcher,
            &[65_536, 65_536],
            &mut scratch,
            &mut out,
        )
        .expect("Fix: dispatch should reuse caller-owned buffers");
        assert_eq!(out, vec![32_768, 32_768]);
        assert_eq!(scratch.inputs[0].as_ptr(), first_input_ptr);
        assert_eq!(scratch.inputs[1].as_ptr(), second_input_ptr);
        assert_eq!(out.as_ptr(), out_ptr);
    }

    #[test]
    fn fixed_gradient_magnitude_matches_probabilities() {
        let mut scratch = DifferentiableAutotuneGpuScratch::default();
        let mut out = Vec::new();
        config_gradient_magnitude_pre_exp_fixed_via_with_scratch_into(
            &DifferentiableDispatcher,
            &[65_536, 196_608],
            &mut scratch,
            &mut out,
        )
        .expect("Fix: dispatch should return unsigned gradient magnitudes");
        assert_eq!(out, vec![16_384, 49_152]);
    }

    #[test]
    fn fixed_natural_gradient_preconditions_policy_gradient() {
        let one = 1 << 16;
        let half = 1 << 15;
        let m_inv_sqrt = vec![one, 0, 0, half];

        let out = natural_config_gradient_magnitude_pre_exp_fixed_via(
            &DifferentiableDispatcher,
            &[one, one],
            &m_inv_sqrt,
        )
        .expect(
            "Fix: release autotuner should compose softmax gradient with Fisher preconditioning",
        );

        assert_eq!(out, vec![half, 1 << 14]);
    }

    #[test]
    fn fixed_natural_gradient_reuses_two_stage_scratch() {
        let one = 1 << 16;
        let m_inv_sqrt = vec![one, 0, 0, one];
        let mut scratch = NaturalDifferentiableAutotuneGpuScratch::default();
        let mut out = Vec::new();

        natural_config_gradient_magnitude_pre_exp_fixed_via_with_scratch_into(
            &DifferentiableDispatcher,
            &[one, one],
            &m_inv_sqrt,
            &mut scratch,
            &mut out,
        )
        .expect("Fix: first natural-gradient autotune dispatch should succeed");
        let softmax_ptrs: Vec<*const u8> = scratch.softmax.inputs.iter().map(Vec::as_ptr).collect();
        let probabilities_ptr = scratch.probabilities.as_ptr();
        let out_ptr = out.as_ptr();

        natural_config_gradient_magnitude_pre_exp_fixed_via_with_scratch_into(
            &DifferentiableDispatcher,
            &[one, one],
            &m_inv_sqrt,
            &mut scratch,
            &mut out,
        )
        .expect("Fix: second natural-gradient autotune dispatch should reuse buffers");

        for (before, after) in softmax_ptrs
            .iter()
            .zip(scratch.softmax.inputs.iter().map(Vec::as_ptr))
        {
            assert_eq!(*before, after);
        }
        assert_eq!(scratch.probabilities.as_ptr(), probabilities_ptr);
        assert_eq!(out.as_ptr(), out_ptr);
    }

    #[test]
    fn fixed_pick_config_rejects_empty_candidates() {
        let err = pick_config_pre_exp_fixed_via(&DifferentiableDispatcher, &[])
            .expect_err("empty candidate grids are invalid");
        match err {
            DispatchError::BadInputs(message) => {
                assert!(message.contains("requires at least one candidate"));
            }
            other => panic!("expected BadInputs, got {other:?}"),
        }
    }

    #[test]
    fn release_fixed_path_does_not_call_cpu_or_reference_helpers() {
        let source = include_str!("differentiable_autotune.rs");
        let start = source
            .find("pub fn pick_config_pre_exp_fixed_via")
            .expect("Fix: fixed path marker must exist");
        let end = source
            .find("\n/// Soft-pick the best configuration index")
            .expect("Fix: test-only CPU path marker must exist");
        let release_path = &source[start..end];
        assert!(!release_path.contains("_cpu"));
        assert!(!release_path.contains("reference_"));
        assert!(!release_path.contains("vec![0u32"));
    }

    #[test]
    fn pick_best_config_returns_minimum_cost_idx() {
        let costs = vec![3.0, 1.0, 4.0, 1.5, 9.0];
        assert_eq!(pick_best_config(&costs), 1); // cost 1.0
    }

    #[test]
    fn pick_config_low_temp_concentrates_on_best() {
        let costs = vec![5.0, 1.0, 5.0];
        let probs = pick_config(&costs, 0.01);
        assert!(probs[1] > 0.99);
        assert!(probs[0] < 0.01);
        assert!(probs[2] < 0.01);
    }

    #[test]
    fn pick_config_high_temp_uniform() {
        let costs = vec![1.0, 5.0, 9.0];
        let probs = pick_config(&costs, 1000.0);
        // High temperature flattens the distribution near uniform.
        // Tolerance loosened: cost spread/temperature is small, but
        // not zero, so probs ≈ 1/3 ± O(0.01).
        for p in probs {
            assert!((p - 1.0 / 3.0).abs() < 0.01);
        }
    }

    #[test]
    fn pick_config_into_reuses_buffers() {
        let costs = vec![5.0, 1.0, 5.0];
        let mut scratch = Vec::with_capacity(8);
        let mut scaled = Vec::with_capacity(8);
        let mut out = Vec::with_capacity(8);
        let scratch_ptr = scratch.as_ptr();
        let scaled_ptr = scaled.as_ptr();
        let out_ptr = out.as_ptr();
        reference_pick_config_into(&costs, 0.01, &mut scratch, &mut scaled, &mut out);
        assert!(out[1] > 0.99);
        assert_eq!(scratch.as_ptr(), scratch_ptr);
        assert_eq!(scaled.as_ptr(), scaled_ptr);
        assert_eq!(out.as_ptr(), out_ptr);
    }

    #[test]
    fn config_gradient_sum_is_negative_one() {
        let costs = vec![1.0, 2.0, 3.0];
        let grads = config_gradient(&costs, 1.0);
        let total: f64 = grads.iter().sum();
        assert!(approx_eq(total, -1.0)); // -Σ softmax = -1
    }

    #[test]
    fn config_gradient_into_reuses_buffers() {
        let costs = vec![1.0, 2.0, 3.0];
        let mut scratch = Vec::with_capacity(8);
        let mut out = Vec::with_capacity(8);
        let scratch_ptr = scratch.as_ptr();
        let out_ptr = out.as_ptr();
        reference_config_gradient_into(&costs, 1.0, &mut scratch, &mut out);
        let total: f64 = out.iter().sum();
        assert!(approx_eq(total, -1.0));
        assert_eq!(scratch.as_ptr(), scratch_ptr);
        assert_eq!(out.as_ptr(), out_ptr);
    }
}
