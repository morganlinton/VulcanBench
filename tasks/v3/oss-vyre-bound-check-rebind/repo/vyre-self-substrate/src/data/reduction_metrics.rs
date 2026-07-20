//! GPU reduction metrics for self-substrate scheduling and telemetry.
//!
//! Optimizer passes repeatedly need scalar summaries: total active work,
//! maximum queue depth, minimum remaining budget, all/any convergence flags,
//! per-segment pressure, and occupancy histograms. This module routes those
//! summaries through `vyre-primitives::reduce` programs instead of open-coding
//! host loops in each pass.

use crate::dispatch_buffers::{
    ceil_div_u32, decode_u32_output_exact, ensure_input_slots, write_u32_slice_le_bytes,
    write_zero_bytes,
};
use crate::optimizer::dispatcher::{DispatchError, OptimizerDispatcher};
use vyre_primitives::reduce::{
    all::reduce_all, any::reduce_any, count_non_zero::reduce_count_non_zero,
    histogram::histogram_atomic_scatter, max::reduce_max, min::reduce_min,
    segment_reduce::segment_reduce_sum, sum::reduce_sum,
};

#[cfg(any(test, feature = "cpu-parity"))]
use vyre_primitives::reduce::{
    all::cpu_ref as primitive_all, any::cpu_ref as primitive_any,
    count_non_zero::cpu_ref as primitive_count_non_zero, histogram::cpu_ref as primitive_histogram,
    max::cpu_ref as primitive_max, min::cpu_ref as primitive_min,
    segment_reduce::cpu_ref as primitive_segment_reduce_sum, sum::cpu_ref as primitive_sum,
};

/// Caller-owned scratch for reduction metric dispatches.
#[derive(Debug, Default)]
pub struct ReductionMetricsGpuScratch {
    inputs: Vec<Vec<u8>>,
}

/// Scalar reduction selector.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReductionMetric {
    /// Wrapping unsigned sum.
    Sum,
    /// Unsigned maximum.
    Max,
    /// Unsigned minimum.
    Min,
    /// Count non-zero lanes.
    CountNonZero,
    /// Any non-zero lane.
    Any,
    /// Every lane non-zero.
    All,
}

/// Dispatch one scalar reduction metric over a u32 value set.
///
/// # Errors
///
/// Returns [`DispatchError`] when input length exceeds the primitive index
/// space, dispatch fails, or scalar readback is malformed.
pub fn reduce_metric_via(
    dispatcher: &dyn OptimizerDispatcher,
    metric: ReductionMetric,
    values: &[u32],
) -> Result<u32, DispatchError> {
    let mut scratch = ReductionMetricsGpuScratch::default();
    reduce_metric_via_with_scratch(dispatcher, metric, values, &mut scratch)
}

/// Dispatch one scalar reduction metric using caller-owned scratch.
///
/// # Errors
///
/// Returns [`DispatchError`] when validation, dispatch, or readback fails.
pub fn reduce_metric_via_with_scratch(
    dispatcher: &dyn OptimizerDispatcher,
    metric: ReductionMetric,
    values: &[u32],
    scratch: &mut ReductionMetricsGpuScratch,
) -> Result<u32, DispatchError> {
    use crate::observability::{bump, reduction_metrics_calls};
    bump(&reduction_metrics_calls);

    let count = checked_len(values.len(), "reduce_metric_via")?;
    let program = match metric {
        ReductionMetric::Sum => reduce_sum("values", "out", count),
        ReductionMetric::Max => reduce_max("values", "out", count),
        ReductionMetric::Min => reduce_min("values", "out", count),
        ReductionMetric::CountNonZero => reduce_count_non_zero("values", "out", count),
        ReductionMetric::Any => reduce_any("values", "out", count),
        ReductionMetric::All => reduce_all("values", "out", count),
    };
    ensure_input_slots(&mut scratch.inputs, 2);
    write_u32_slice_le_bytes(&mut scratch.inputs[0], values);
    write_zero_bytes(&mut scratch.inputs[1], std::mem::size_of::<u32>());
    let outputs = dispatcher.dispatch(
        &program,
        &scratch.inputs,
        Some(grid_for_metric(metric, count)),
    )?;
    decode_scalar(&outputs, "reduce_metric_via")
}

/// Wrapping sum of active work items through the reduce primitive.
///
/// # Errors
///
/// Returns [`DispatchError`] when dispatch or readback fails.
pub fn reduce_sum_via(
    dispatcher: &dyn OptimizerDispatcher,
    values: &[u32],
) -> Result<u32, DispatchError> {
    reduce_metric_via(dispatcher, ReductionMetric::Sum, values)
}

/// Maximum queue depth through the reduce primitive.
///
/// # Errors
///
/// Returns [`DispatchError`] when dispatch or readback fails.
pub fn reduce_max_via(
    dispatcher: &dyn OptimizerDispatcher,
    values: &[u32],
) -> Result<u32, DispatchError> {
    reduce_metric_via(dispatcher, ReductionMetric::Max, values)
}

/// Minimum remaining budget through the reduce primitive.
///
/// # Errors
///
/// Returns [`DispatchError`] when dispatch or readback fails.
pub fn reduce_min_via(
    dispatcher: &dyn OptimizerDispatcher,
    values: &[u32],
) -> Result<u32, DispatchError> {
    reduce_metric_via(dispatcher, ReductionMetric::Min, values)
}

/// Non-zero lane count through the reduce primitive.
///
/// # Errors
///
/// Returns [`DispatchError`] when dispatch or readback fails.
pub fn reduce_count_non_zero_via(
    dispatcher: &dyn OptimizerDispatcher,
    values: &[u32],
) -> Result<u32, DispatchError> {
    reduce_metric_via(dispatcher, ReductionMetric::CountNonZero, values)
}

/// Any-lane convergence predicate through the reduce primitive.
///
/// # Errors
///
/// Returns [`DispatchError`] when dispatch or readback fails.
pub fn reduce_any_via(
    dispatcher: &dyn OptimizerDispatcher,
    values: &[u32],
) -> Result<bool, DispatchError> {
    Ok(reduce_metric_via(dispatcher, ReductionMetric::Any, values)? != 0)
}

/// All-lanes convergence predicate through the reduce primitive.
///
/// # Errors
///
/// Returns [`DispatchError`] when dispatch or readback fails.
pub fn reduce_all_via(
    dispatcher: &dyn OptimizerDispatcher,
    values: &[u32],
) -> Result<bool, DispatchError> {
    Ok(reduce_metric_via(dispatcher, ReductionMetric::All, values)? != 0)
}

/// Per-segment wrapping sum through the segment reduction primitive.
///
/// # Errors
///
/// Returns [`DispatchError`] when offsets are malformed, segment count is
/// unsupported by the primitive, dispatch fails, or readback is malformed.
pub fn segment_reduce_sum_via(
    dispatcher: &dyn OptimizerDispatcher,
    input: &[u32],
    segment_offsets: &[u32],
) -> Result<Vec<u32>, DispatchError> {
    let mut out = Vec::new();
    let mut scratch = ReductionMetricsGpuScratch::default();
    segment_reduce_sum_via_with_scratch_into(
        dispatcher,
        input,
        segment_offsets,
        &mut scratch,
        &mut out,
    )?;
    Ok(out)
}

/// Per-segment wrapping sum into caller-owned storage.
///
/// # Errors
///
/// Returns [`DispatchError`] when validation, dispatch, or readback fails.
pub fn segment_reduce_sum_via_with_scratch_into(
    dispatcher: &dyn OptimizerDispatcher,
    input: &[u32],
    segment_offsets: &[u32],
    scratch: &mut ReductionMetricsGpuScratch,
    out: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    use crate::observability::{bump, reduction_metrics_calls};
    bump(&reduction_metrics_calls);

    let num_segments = validate_segment_offsets(input, segment_offsets)?;
    let program = segment_reduce_sum("input", "segment_offsets", "output", num_segments);
    ensure_input_slots(&mut scratch.inputs, 3);
    write_u32_slice_le_bytes(&mut scratch.inputs[0], input);
    write_u32_slice_le_bytes(&mut scratch.inputs[1], segment_offsets);
    write_zero_bytes(
        &mut scratch.inputs[2],
        num_segments as usize * std::mem::size_of::<u32>(),
    );
    let outputs = dispatcher.dispatch(&program, &scratch.inputs, Some([1, 1, 1]))?;
    decode_first_output(
        &outputs,
        num_segments as usize,
        "segment_reduce_sum_via",
        out,
    )
}

/// Histogram with input-parallel atomic scatter semantics.
///
/// # Errors
///
/// Returns [`DispatchError`] when count/bin dimensions are zero or too large,
/// dispatch fails, or readback is malformed.
pub fn histogram_atomic_scatter_via(
    dispatcher: &dyn OptimizerDispatcher,
    input: &[u32],
    num_bins: u32,
) -> Result<Vec<u32>, DispatchError> {
    use crate::observability::{bump, reduction_metrics_calls};
    bump(&reduction_metrics_calls);

    let count = checked_nonzero_len(input.len(), "histogram_atomic_scatter_via")?;
    if num_bins == 0 {
        return Err(DispatchError::BadInputs(
            "Fix: histogram_atomic_scatter_via requires num_bins > 0.".to_string(),
        ));
    }
    let bin_count = num_bins as usize;
    let program = histogram_atomic_scatter("input", "output", count, num_bins);
    let mut scratch = ReductionMetricsGpuScratch::default();
    ensure_input_slots(&mut scratch.inputs, 2);
    write_u32_slice_le_bytes(&mut scratch.inputs[0], input);
    write_zero_bytes(
        &mut scratch.inputs[1],
        bin_count * std::mem::size_of::<u32>(),
    );
    let outputs = dispatcher.dispatch(
        &program,
        &scratch.inputs,
        Some([ceil_div_u32(count, 256), 1, 1]),
    )?;
    let mut out = Vec::new();
    decode_first_output(
        &outputs,
        bin_count,
        "histogram_atomic_scatter_via",
        &mut out,
    )?;
    Ok(out)
}

#[cfg(any(test, feature = "cpu-parity"))]
#[must_use]
pub fn reference_reduce_sum(values: &[u32]) -> u32 {
    primitive_sum(values)
}

#[cfg(any(test, feature = "cpu-parity"))]
#[must_use]
pub fn reference_reduce_max(values: &[u32]) -> u32 {
    primitive_max(values)
}

#[cfg(any(test, feature = "cpu-parity"))]
#[must_use]
pub fn reference_reduce_min(values: &[u32]) -> u32 {
    primitive_min(values)
}

#[cfg(any(test, feature = "cpu-parity"))]
#[must_use]
pub fn reference_reduce_count_non_zero(values: &[u32]) -> u32 {
    primitive_count_non_zero(values)
}

#[cfg(any(test, feature = "cpu-parity"))]
#[must_use]
pub fn reference_reduce_any(values: &[u32]) -> bool {
    primitive_any(values) != 0
}

#[cfg(any(test, feature = "cpu-parity"))]
#[must_use]
pub fn reference_reduce_all(values: &[u32]) -> bool {
    primitive_all(values) != 0
}

#[cfg(any(test, feature = "cpu-parity"))]
#[must_use]
pub fn reference_segment_reduce_sum(input: &[u32], segment_offsets: &[u32]) -> Vec<u32> {
    primitive_segment_reduce_sum(input, segment_offsets)
}

#[cfg(any(test, feature = "cpu-parity"))]
#[must_use]
pub fn reference_histogram_atomic_scatter(input: &[u32], num_bins: u32) -> Vec<u32> {
    primitive_histogram(input, num_bins)
}

fn grid_for_metric(metric: ReductionMetric, _count: u32) -> [u32; 3] {
    match metric {
        ReductionMetric::Sum
        | ReductionMetric::Max
        | ReductionMetric::Min
        | ReductionMetric::CountNonZero
        | ReductionMetric::Any
        | ReductionMetric::All => [1, 1, 1],
    }
}

fn checked_len(len: usize, context: &'static str) -> Result<u32, DispatchError> {
    u32::try_from(len).map_err(|_| {
        DispatchError::BadInputs(format!(
            "Fix: {context} received {len} values, which exceeds the u32 GPU index space."
        ))
    })
}

fn checked_nonzero_len(len: usize, context: &'static str) -> Result<u32, DispatchError> {
    let count = checked_len(len, context)?;
    if count == 0 {
        return Err(DispatchError::BadInputs(format!(
            "Fix: {context} requires count > 0."
        )));
    }
    Ok(count)
}

fn validate_segment_offsets(input: &[u32], segment_offsets: &[u32]) -> Result<u32, DispatchError> {
    if segment_offsets.len() < 2 {
        return Err(DispatchError::BadInputs(
            "Fix: segment_reduce_sum_via requires at least two CSR offsets.".to_string(),
        ));
    }
    let num_segments = segment_offsets.len() - 1;
    if num_segments > 256 {
        return Err(DispatchError::BadInputs(format!(
            "Fix: segment_reduce_sum_via supports at most 256 segments per primitive dispatch, got {num_segments}."
        )));
    }
    for (segment, pair) in segment_offsets.windows(2).enumerate() {
        let start = pair[0] as usize;
        let end = pair[1] as usize;
        if start > end || end > input.len() {
            return Err(DispatchError::BadInputs(format!(
                "Fix: segment_reduce_sum_via received malformed segment {segment}: start={start}, end={end}, input_len={}.",
                input.len()
            )));
        }
    }
    Ok(num_segments as u32)
}

fn decode_scalar(outputs: &[Vec<u8>], context: &'static str) -> Result<u32, DispatchError> {
    let mut out = Vec::new();
    decode_first_output(outputs, 1, context, &mut out)?;
    Ok(out[0])
}

fn decode_first_output(
    outputs: &[Vec<u8>],
    words: usize,
    context: &'static str,
    out: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    if outputs.is_empty() {
        return Err(DispatchError::BackendError(format!(
            "Fix: {context} expected at least one output buffer, got 0."
        )));
    }
    decode_u32_output_exact(&outputs[0], words, context, out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch_buffers::u32_slice_to_le_bytes;
    use vyre_foundation::ir::Program;

    struct ReduceDispatcher;

    impl OptimizerDispatcher for ReduceDispatcher {
        fn dispatch(
            &self,
            program: &Program,
            inputs: &[Vec<u8>],
            grid_override: Option<[u32; 3]>,
        ) -> Result<Vec<Vec<u8>>, DispatchError> {
            let op_id = program
                .entry
                .iter()
                .find_map(|node| match node {
                    vyre_foundation::ir::Node::Region { generator, .. } => Some(generator.as_str()),
                    _ => None,
                })
                .expect("Fix: reduction primitive should expose a region generator");
            let values = crate::hardware::dispatch_buffers::read_u32s(&inputs[0]);
            match op_id {
                vyre_primitives::reduce::sum::OP_ID => {
                    assert_scalar_metric_dispatch(
                        program,
                        grid_override,
                        ReductionMetric::Sum,
                        values.len() as u32,
                    );
                    scalar(primitive_sum(&values))
                }
                vyre_primitives::reduce::max::OP_ID => {
                    assert_scalar_metric_dispatch(
                        program,
                        grid_override,
                        ReductionMetric::Max,
                        values.len() as u32,
                    );
                    scalar(primitive_max(&values))
                }
                vyre_primitives::reduce::min::OP_ID => {
                    assert_scalar_metric_dispatch(
                        program,
                        grid_override,
                        ReductionMetric::Min,
                        values.len() as u32,
                    );
                    scalar(primitive_min(&values))
                }
                vyre_primitives::reduce::count_non_zero::OP_ID => {
                    assert_scalar_metric_dispatch(
                        program,
                        grid_override,
                        ReductionMetric::CountNonZero,
                        values.len() as u32,
                    );
                    scalar(primitive_count_non_zero(&values))
                }
                vyre_primitives::reduce::any::OP_ID => {
                    assert_scalar_metric_dispatch(
                        program,
                        grid_override,
                        ReductionMetric::Any,
                        values.len() as u32,
                    );
                    scalar(primitive_any(&values))
                }
                vyre_primitives::reduce::all::OP_ID => {
                    assert_scalar_metric_dispatch(
                        program,
                        grid_override,
                        ReductionMetric::All,
                        values.len() as u32,
                    );
                    scalar(primitive_all(&values))
                }
                vyre_primitives::reduce::segment_reduce::OP_ID => {
                    assert_eq!(grid_override, Some([1, 1, 1]));
                    assert_eq!(program.workgroup_size(), [256, 1, 1]);
                    let offsets = crate::hardware::dispatch_buffers::read_u32s(&inputs[1]);
                    Ok(vec![u32_slice_to_le_bytes(&primitive_segment_reduce_sum(
                        &values, &offsets,
                    ))])
                }
                vyre_primitives::reduce::histogram::OP_ID => {
                    assert_eq!(
                        grid_override,
                        Some([ceil_div_u32(values.len() as u32, 256), 1, 1]),
                        "Fix: histogram_atomic_scatter_via must launch one lane per input item."
                    );
                    assert_eq!(program.workgroup_size(), [256, 1, 1]);
                    let bins = (inputs[1].len() / std::mem::size_of::<u32>()) as u32;

                    Ok(vec![u32_slice_to_le_bytes(&primitive_histogram(
                        &values, bins,
                    ))])
                }
                other => panic!("unexpected reduction primitive op id {other}"),
            }
        }
    }

    fn scalar(value: u32) -> Result<Vec<Vec<u8>>, DispatchError> {
        Ok(vec![u32_slice_to_le_bytes(&[value])])
    }

    fn assert_scalar_metric_dispatch(
        program: &Program,
        grid_override: Option<[u32; 3]>,
        metric: ReductionMetric,
        count: u32,
    ) {
        assert_eq!(
            program.workgroup_size(),
            [256, 1, 1],
            "Fix: scalar reduction primitives must keep the 256-lane chunked reducer."
        );
        assert_eq!(
            grid_override,
            Some(grid_for_metric(metric, count)),
            "Fix: scalar reductions initialize their output inside the primitive, so the self-substrate wrapper must not split one scalar reduction across unsynchronized workgroups."
        );
    }

    #[test]
    fn reference_reductions_match_primitives_exactly() {
        let values = [1u32, 0, 7, u32::MAX];
        assert_eq!(reference_reduce_sum(&values), primitive_sum(&values));
        assert_eq!(reference_reduce_max(&values), primitive_max(&values));
        assert_eq!(reference_reduce_min(&values), primitive_min(&values));
        assert_eq!(
            reference_reduce_count_non_zero(&values),
            primitive_count_non_zero(&values)
        );
        assert_eq!(reference_reduce_any(&values), primitive_any(&values) != 0);
        assert_eq!(reference_reduce_all(&values), primitive_all(&values) != 0);
    }

    #[test]
    fn scalar_reductions_dispatch_through_primitives() {
        let values = [1u32, 0, 7, 3];
        assert_eq!(reduce_sum_via(&ReduceDispatcher, &values).unwrap(), 11);
        assert_eq!(reduce_max_via(&ReduceDispatcher, &values).unwrap(), 7);
        assert_eq!(reduce_min_via(&ReduceDispatcher, &values).unwrap(), 0);
        assert_eq!(
            reduce_count_non_zero_via(&ReduceDispatcher, &values).unwrap(),
            3
        );
        assert!(reduce_any_via(&ReduceDispatcher, &values).unwrap());
        assert!(!reduce_all_via(&ReduceDispatcher, &values).unwrap());
    }

    #[test]
    fn segment_and_histogram_dispatch_through_primitives() {
        assert_eq!(
            segment_reduce_sum_via(&ReduceDispatcher, &[1, 2, 3, 4, 5], &[0, 2, 5]).unwrap(),
            vec![3, 12]
        );
        assert_eq!(
            histogram_atomic_scatter_via(&ReduceDispatcher, &[0, 1, 2, 1, 9], 4).unwrap(),
            vec![1, 2, 1, 0]
        );
    }

    #[test]
    fn generated_large_scalar_reductions_match_oracles() {
        for case in 0..4096u32 {
            let len = 257 + (case.wrapping_mul(31) % 1024) as usize;
            let mut state = 0xA11C_E5CAu32 ^ case.wrapping_mul(0x9E37_79B9);
            let mut values = Vec::with_capacity(len);
            for index in 0..len {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                values.push(if (state.wrapping_add(index as u32)) % 13 == 0 {
                    0
                } else {
                    state
                });
            }

            assert_eq!(
                reduce_sum_via(&ReduceDispatcher, &values).unwrap(),
                reference_reduce_sum(&values),
                "case {case}: sum"
            );
            assert_eq!(
                reduce_max_via(&ReduceDispatcher, &values).unwrap(),
                reference_reduce_max(&values),
                "case {case}: max"
            );
            assert_eq!(
                reduce_min_via(&ReduceDispatcher, &values).unwrap(),
                reference_reduce_min(&values),
                "case {case}: min"
            );
            assert_eq!(
                reduce_count_non_zero_via(&ReduceDispatcher, &values).unwrap(),
                reference_reduce_count_non_zero(&values),
                "case {case}: count_non_zero"
            );
            assert_eq!(
                reduce_any_via(&ReduceDispatcher, &values).unwrap(),
                reference_reduce_any(&values),
                "case {case}: any"
            );
            assert_eq!(
                reduce_all_via(&ReduceDispatcher, &values).unwrap(),
                reference_reduce_all(&values),
                "case {case}: all"
            );
        }
    }

    #[test]
    fn generated_large_histograms_match_oracles() {
        for case in 0..4096u32 {
            let len = 257 + (case.wrapping_mul(17) % 1024) as usize;
            let bins = 1 + case.wrapping_mul(7) % 97;
            let mut state = 0xABCD_EF01u32 ^ case.wrapping_mul(0x85EB_CA6B);
            let mut input = Vec::with_capacity(len);
            for index in 0..len {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                let value = if index % 11 == 0 {
                    bins + (state % 19)
                } else {
                    state % bins
                };
                input.push(value);
            }

            assert_eq!(
                histogram_atomic_scatter_via(&ReduceDispatcher, &input, bins).unwrap(),
                reference_histogram_atomic_scatter(&input, bins),
                "case {case}: histogram"
            );
        }
    }

    #[test]
    fn scratch_path_reuses_buffers() {
        let mut scratch = ReductionMetricsGpuScratch::default();
        assert_eq!(
            reduce_metric_via_with_scratch(
                &ReduceDispatcher,
                ReductionMetric::CountNonZero,
                &[0, 1, 2],
                &mut scratch,
            )
            .unwrap(),
            2
        );
        let input_capacities = scratch.inputs.iter().map(Vec::capacity).collect::<Vec<_>>();
        assert_eq!(
            reduce_metric_via_with_scratch(
                &ReduceDispatcher,
                ReductionMetric::CountNonZero,
                &[0, 1, 2],
                &mut scratch,
            )
            .unwrap(),
            2
        );
        assert_eq!(
            scratch.inputs.iter().map(Vec::capacity).collect::<Vec<_>>(),
            input_capacities
        );
    }

    #[test]
    fn invalid_segment_offsets_are_actionable() {
        let err = segment_reduce_sum_via(&ReduceDispatcher, &[1, 2], &[0, 3]).unwrap_err();
        assert!(err
            .to_string()
            .contains("Fix: segment_reduce_sum_via received malformed segment"));
    }

    #[test]
    fn zero_bin_histogram_is_rejected_before_dispatch() {
        let err = histogram_atomic_scatter_via(&ReduceDispatcher, &[1], 0).unwrap_err();
        assert!(err
            .to_string()
            .contains("Fix: histogram_atomic_scatter_via requires num_bins > 0"));
    }
}
