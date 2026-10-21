//! Probabilistic dispatch cost model via #10 sum_product_circuit (#28).
//!
//! Models per-Program runtime as a probabilistic circuit. Calibrated
//! intervals come from #41 conformal prediction over historical
//! latency samples. Output feeds #22 megakernel scheduler as soft
//! constraints.

use crate::dispatch_buffers::{
    decode_u32_output_exact, ensure_input_slots, write_u32_slice_le_bytes, write_zero_bytes,
};
use crate::optimizer::dispatcher::{DispatchError, OptimizerDispatcher};
use vyre_primitives::graph::sum_product_circuit::sum_product_evaluate;
#[cfg(test)]
use vyre_primitives::graph::sum_product_circuit::sum_product_evaluate_cpu;
#[cfg(test)]
use vyre_primitives::math::conformal::conformal_threshold_cpu;
use vyre_primitives::math::conformal::{conformal_threshold, try_conformal_rank};

/// Caller-owned dispatch scratch for probabilistic runtime prediction.
#[derive(Debug, Default)]
pub struct CostModelGpuScratch {
    inputs: Vec<Vec<u8>>,
    circuit_out: Vec<u32>,
    conformal_out: Vec<u32>,
}

/// Predict expected runtime for a Program using a sum-product circuit
/// over its features. Returns (point_estimate, conformal_upper_bound).
#[must_use]
#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub fn reference_predict_runtime(
    feature_circuit_kinds: &[u32],
    feature_circuit_offsets: &[u32],
    feature_circuit_counts: &[u32],
    feature_circuit_children: &[u32],
    feature_circuit_weights: &[f64],
    feature_values: &[f64],
    historical_residuals: &[u32],
    alpha: f64,
) -> (f64, u32) {
    use crate::observability::{bump, cost_model_calls};
    bump(&cost_model_calls);
    let topo: Vec<u32> = (0..feature_circuit_kinds.len() as u32).collect();
    let result = sum_product_evaluate_cpu(
        feature_circuit_kinds,
        feature_circuit_offsets,
        feature_circuit_counts,
        feature_circuit_children,
        feature_circuit_weights,
        feature_values,
        &topo,
    );
    let point_estimate = *result.last().unwrap_or(&0.0);
    let upper_bound = conformal_threshold_cpu(historical_residuals, alpha);
    (point_estimate, upper_bound)
}

/// Predict runtime with primitive-native fixed-point buffers through the active backend.
///
/// `feature_circuit_weights_fixed` and `feature_values_fixed` use 16.16 fixed-point u32 lanes.
/// `historical_residuals_sorted` must already be sorted ascending so this path does not introduce
/// host-side sorting into the release cost model.
///
/// # Errors
///
/// Returns [`DispatchError`] for malformed circuit buffers, unsorted residuals, invalid alpha,
/// dispatch rejection, or malformed backend output.
#[allow(clippy::too_many_arguments)]
pub fn predict_runtime_fixed_via(
    dispatcher: &impl OptimizerDispatcher,
    feature_circuit_kinds: &[u32],
    feature_circuit_offsets: &[u32],
    feature_circuit_counts: &[u32],
    feature_circuit_children: &[u32],
    feature_circuit_weights_fixed: &[u32],
    feature_values_fixed: &[u32],
    historical_residuals_sorted: &[u32],
    alpha: f64,
) -> Result<(u32, u32), DispatchError> {
    let mut scratch = CostModelGpuScratch::default();
    predict_runtime_fixed_via_with_scratch(
        dispatcher,
        feature_circuit_kinds,
        feature_circuit_offsets,
        feature_circuit_counts,
        feature_circuit_children,
        feature_circuit_weights_fixed,
        feature_values_fixed,
        historical_residuals_sorted,
        alpha,
        &mut scratch,
    )
}

/// Predict runtime with caller-owned dispatch scratch.
///
/// # Errors
///
/// Returns [`DispatchError`] for malformed circuit buffers, unsorted residuals, invalid alpha,
/// dispatch rejection, or malformed backend output.
#[allow(clippy::too_many_arguments)]
pub fn predict_runtime_fixed_via_with_scratch(
    dispatcher: &impl OptimizerDispatcher,
    feature_circuit_kinds: &[u32],
    feature_circuit_offsets: &[u32],
    feature_circuit_counts: &[u32],
    feature_circuit_children: &[u32],
    feature_circuit_weights_fixed: &[u32],
    feature_values_fixed: &[u32],
    historical_residuals_sorted: &[u32],
    alpha: f64,
    scratch: &mut CostModelGpuScratch,
) -> Result<(u32, u32), DispatchError> {
    use crate::observability::{bump, cost_model_calls};
    bump(&cost_model_calls);

    let n_nodes = validate_circuit(
        feature_circuit_kinds,
        feature_circuit_offsets,
        feature_circuit_counts,
        feature_circuit_children,
        feature_circuit_weights_fixed,
        feature_values_fixed,
    )?;
    let n_edges = u32::try_from(feature_circuit_children.len()).map_err(|_| {
        DispatchError::BadInputs(format!(
            "Fix: predict_runtime_fixed_via edge count {} exceeds u32::MAX.",
            feature_circuit_children.len()
        ))
    })?;
    let residual_count = u32::try_from(historical_residuals_sorted.len()).map_err(|_| {
        DispatchError::BadInputs(format!(
            "Fix: predict_runtime_fixed_via residual count {} exceeds u32::MAX.",
            historical_residuals_sorted.len()
        ))
    })?;
    let k = try_conformal_rank(residual_count, alpha).ok_or_else(|| {
        DispatchError::BadInputs(format!(
            "Fix: predict_runtime_fixed_via requires residual_count > 0 and 0 < alpha < 1, got residual_count={residual_count}, alpha={alpha}."
        ))
    })?;
    validate_sorted_residuals(historical_residuals_sorted)?;

    let node_bytes = feature_circuit_kinds
        .len()
        .checked_mul(std::mem::size_of::<u32>())
        .ok_or_else(|| {
            DispatchError::BadInputs(format!(
                "Fix: predict_runtime_fixed_via node output byte count overflows usize for n_nodes={n_nodes}."
            ))
        })?;
    ensure_input_slots(&mut scratch.inputs, 7);
    write_u32_slice_le_bytes(&mut scratch.inputs[0], feature_circuit_kinds);
    write_u32_slice_le_bytes(&mut scratch.inputs[1], feature_circuit_offsets);
    write_u32_slice_le_bytes(&mut scratch.inputs[2], feature_circuit_counts);
    write_u32_slice_le_bytes(&mut scratch.inputs[3], feature_circuit_children);
    write_u32_slice_le_bytes(&mut scratch.inputs[4], feature_circuit_weights_fixed);
    write_u32_slice_le_bytes(&mut scratch.inputs[5], feature_values_fixed);
    write_zero_bytes(&mut scratch.inputs[6], node_bytes);
    let circuit = sum_product_evaluate(
        "kinds",
        "child_offsets",
        "child_counts",
        "children",
        "weights",
        "leaf_values",
        "out",
        n_nodes,
        n_edges,
    );
    let circuit_outputs = dispatcher.dispatch(&circuit, &scratch.inputs, Some([1, 1, 1]))?;
    let circuit_output = circuit_outputs.first().ok_or_else(|| {
        DispatchError::BackendError(format!(
            "Fix: predict_runtime_fixed_via expected one sum-product output, got {}.",
            circuit_outputs.len()
        ))
    })?;
    decode_u32_output_exact(
        circuit_output,
        n_nodes as usize,
        "predict_runtime_fixed_via sum_product",
        &mut scratch.circuit_out,
    )?;

    ensure_input_slots(&mut scratch.inputs, 2);
    write_u32_slice_le_bytes(&mut scratch.inputs[0], historical_residuals_sorted);
    write_zero_bytes(&mut scratch.inputs[1], std::mem::size_of::<u32>());
    let conformal = conformal_threshold("scores_sorted", "q_hat", residual_count, k);
    let conformal_outputs =
        dispatcher.dispatch(&conformal, &scratch.inputs[..2], Some([1, 1, 1]))?;
    let conformal_output = conformal_outputs.first().ok_or_else(|| {
        DispatchError::BackendError(format!(
            "Fix: predict_runtime_fixed_via expected one conformal output, got {}.",
            conformal_outputs.len()
        ))
    })?;
    decode_u32_output_exact(
        conformal_output,
        1,
        "predict_runtime_fixed_via conformal",
        &mut scratch.conformal_out,
    )?;

    let point_estimate = scratch.circuit_out.last().copied().ok_or_else(|| {
        DispatchError::BackendError(
            "Fix: predict_runtime_fixed_via sum-product output was unexpectedly empty.".to_string(),
        )
    })?;
    Ok((point_estimate, scratch.conformal_out[0]))
}

fn validate_circuit(
    kinds: &[u32],
    offsets: &[u32],
    counts: &[u32],
    children: &[u32],
    weights: &[u32],
    leaf_values: &[u32],
) -> Result<u32, DispatchError> {
    if kinds.is_empty() {
        return Err(DispatchError::BadInputs(
            "Fix: predict_runtime_fixed_via requires at least one circuit node.".to_string(),
        ));
    }
    if children.is_empty() {
        return Err(DispatchError::BadInputs(
            "Fix: predict_runtime_fixed_via requires at least one circuit edge for the dispatchable sum-product primitive.".to_string(),
        ));
    }
    let n_nodes = kinds.len();
    if offsets.len() != n_nodes || counts.len() != n_nodes || leaf_values.len() != n_nodes {
        return Err(DispatchError::BadInputs(format!(
            "Fix: predict_runtime_fixed_via requires kinds/offsets/counts/leaf_values to have equal node length, got kinds={}, offsets={}, counts={}, leaf_values={}.",
            kinds.len(),
            offsets.len(),
            counts.len(),
            leaf_values.len()
        )));
    }
    if weights.len() != children.len() {
        return Err(DispatchError::BadInputs(format!(
            "Fix: predict_runtime_fixed_via requires one weight per child edge, got weights={}, children={}.",
            weights.len(),
            children.len()
        )));
    }
    for (node, (&offset, &count)) in offsets.iter().zip(counts).enumerate() {
        let end = offset.checked_add(count).ok_or_else(|| {
            DispatchError::BadInputs(format!(
                "Fix: predict_runtime_fixed_via child range overflows u32 for node {node}."
            ))
        })?;
        if end as usize > children.len() {
            return Err(DispatchError::BadInputs(format!(
                "Fix: predict_runtime_fixed_via node {node} child range [{offset}..{end}) exceeds children length {}.",
                children.len()
            )));
        }
    }
    for (idx, &child) in children.iter().enumerate() {
        if child as usize >= n_nodes {
            return Err(DispatchError::BadInputs(format!(
                "Fix: predict_runtime_fixed_via children[{idx}]={child} is outside node count {n_nodes}."
            )));
        }
    }
    u32::try_from(n_nodes).map_err(|_| {
        DispatchError::BadInputs(format!(
            "Fix: predict_runtime_fixed_via node count {n_nodes} exceeds u32::MAX."
        ))
    })
}

fn validate_sorted_residuals(residuals: &[u32]) -> Result<(), DispatchError> {
    for (idx, pair) in residuals.windows(2).enumerate() {
        if pair[0] > pair[1] {
            return Err(DispatchError::BadInputs(format!(
                "Fix: predict_runtime_fixed_via requires sorted residuals, but residuals[{idx}]={} > residuals[{}]={}.",
                pair[0],
                idx + 1,
                pair[1]
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch_buffers::u32_slice_to_le_bytes;
    use vyre_primitives::graph::sum_product_circuit::{KIND_LEAF, KIND_PRODUCT, KIND_SUM};

    #[test]
    fn predict_returns_point_plus_conformal_interval() {
        let kinds = vec![KIND_LEAF, KIND_LEAF, KIND_SUM];
        let offsets = vec![0, 0, 0];
        let counts = vec![0, 0, 2];
        let children = vec![0, 1];
        let weights = vec![0.5, 0.5];
        let values = vec![10.0, 20.0, 0.0];
        let residuals = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

        let (point, upper) = reference_predict_runtime(
            &kinds, &offsets, &counts, &children, &weights, &values, &residuals, 0.1,
        );
        // 0.5·10 + 0.5·20 = 15
        assert!((point - 15.0).abs() < 1e-10);
        // Upper bound = 90th percentile of residuals = 10
        assert_eq!(upper, 10);
    }

    #[test]
    fn product_node_predict_works() {
        let kinds = vec![KIND_LEAF, KIND_LEAF, KIND_PRODUCT];
        let offsets = vec![0, 0, 0];
        let counts = vec![0, 0, 2];
        let children = vec![0, 1];
        let weights = vec![0.0, 0.0];
        let values = vec![3.0, 5.0, 0.0];
        let residuals = vec![1u32];
        let (point, _) = reference_predict_runtime(
            &kinds, &offsets, &counts, &children, &weights, &values, &residuals, 0.5,
        );
        assert!((point - 15.0).abs() < 1e-10);
    }

    #[test]
    fn predict_runtime_fixed_via_dispatches_sum_product_and_conformal() {
        let kinds = vec![KIND_LEAF, KIND_LEAF, KIND_SUM];
        let offsets = vec![0, 0, 0];
        let counts = vec![0, 0, 2];
        let children = vec![0, 1];
        let weights = vec![32768, 32768];
        let values = vec![10 << 16, 20 << 16, 0];
        let residuals = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

        let (point, upper) = predict_runtime_fixed_via(
            &CostModelDispatcher,
            &kinds,
            &offsets,
            &counts,
            &children,
            &weights,
            &values,
            &residuals,
            0.1,
        )
        .unwrap();

        assert_eq!(point, 15 << 16);
        assert_eq!(upper, 10);
    }

    #[test]
    fn predict_runtime_fixed_via_rejects_unsorted_residuals() {
        let kinds = vec![KIND_LEAF, KIND_LEAF, KIND_SUM];
        let offsets = vec![0, 0, 0];
        let counts = vec![0, 0, 2];
        let children = vec![0, 1];
        let weights = vec![32768, 32768];
        let values = vec![10 << 16, 20 << 16, 0];
        let residuals = vec![1, 3, 2];

        let err = predict_runtime_fixed_via(
            &CostModelDispatcher,
            &kinds,
            &offsets,
            &counts,
            &children,
            &weights,
            &values,
            &residuals,
            0.1,
        )
        .unwrap_err();

        assert!(err.to_string().contains("requires sorted residuals"));
    }

    #[test]
    fn production_source_keeps_cpu_cost_model_helpers_out_of_via_path() {
        let source = include_str!("cost_model.rs");
        let via_section = source
            .split("pub fn predict_runtime_fixed_via")
            .nth(1)
            .expect("Fix: via section should exist")
            .split("#[cfg(test)]\nmod tests")
            .next()
            .expect("Fix: test module marker should exist");

        assert!(!via_section.contains("_cpu"));
        assert!(!via_section.contains("reference_predict_runtime"));
    }

    struct CostModelDispatcher;

    impl OptimizerDispatcher for CostModelDispatcher {
        fn dispatch(
            &self,
            _program: &vyre_foundation::ir::Program,
            inputs: &[Vec<u8>],
            _grid_override: Option<[u32; 3]>,
        ) -> Result<Vec<Vec<u8>>, DispatchError> {
            match inputs.len() {
                7 => dispatch_sum_product(inputs),
                2 => dispatch_conformal(inputs),
                other => Err(DispatchError::BadInputs(format!(
                    "Fix: cost-model test dispatcher expected 7 or 2 buffers, got {other}."
                ))),
            }
        }
    }

    fn dispatch_sum_product(inputs: &[Vec<u8>]) -> Result<Vec<Vec<u8>>, DispatchError> {
        let kinds = crate::hardware::dispatch_buffers::read_u32s(&inputs[0]);
        let offsets = crate::hardware::dispatch_buffers::read_u32s(&inputs[1]);
        let counts = crate::hardware::dispatch_buffers::read_u32s(&inputs[2]);
        let children = crate::hardware::dispatch_buffers::read_u32s(&inputs[3]);
        let weights = crate::hardware::dispatch_buffers::read_u32s(&inputs[4]);
        let values = crate::hardware::dispatch_buffers::read_u32s(&inputs[5]);
        let mut out = values.clone();
        for node in 0..kinds.len() {
            if kinds[node] == KIND_SUM {
                let mut acc = 0_u32;
                for edge in offsets[node] as usize..(offsets[node] + counts[node]) as usize {
                    let child = children[edge] as usize;
                    acc += ((out[child] as u64 * weights[edge] as u64) >> 16) as u32;
                }
                out[node] = acc;
            } else if kinds[node] == KIND_PRODUCT {
                let mut acc = 1_u32 << 16;
                for edge in offsets[node] as usize..(offsets[node] + counts[node]) as usize {
                    let child = children[edge] as usize;
                    acc = ((acc as u64 * out[child] as u64) >> 16) as u32;
                }
                out[node] = acc;
            }
        }
        Ok(vec![u32_slice_to_le_bytes(&out)])
    }

    fn dispatch_conformal(inputs: &[Vec<u8>]) -> Result<Vec<Vec<u8>>, DispatchError> {
        let scores = crate::hardware::dispatch_buffers::read_u32s(&inputs[0]);
        let out_len = inputs[1].len() / std::mem::size_of::<u32>();
        if out_len != 1 {
            return Err(DispatchError::BadInputs(format!(
                "Fix: conformal test dispatcher expected one output word, got {out_len}."
            )));
        }
        Ok(vec![u32_slice_to_le_bytes(&[*scores.last().unwrap_or(&0)])])
    }
}
