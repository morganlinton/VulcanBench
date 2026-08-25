//! Autotune latency-trace smoothing via `vyre-primitives::math::conv1d`.
//!
//! Optimizer telemetry is noisy: isolated dispatch spikes, PCIe jitter, and
//! queue contention can cause a single bad sample to push the autotuner toward
//! a worse workgroup shape. This module smooths latency traces with the same
//! 1D convolution primitive shipped to users, keeping the recursion thesis
//! intact while giving the scheduler a stable signal.

use crate::dispatch_buffers::{
    ceil_div_u32, decode_u32_output_exact, ensure_input_slots, write_u32_slice_le_bytes,
    write_zero_bytes,
};
use crate::optimizer::dispatcher::{DispatchError, OptimizerDispatcher};
use vyre_foundation::ir::Node;
#[cfg(test)]
use vyre_primitives::math::conv1d::cpu_conv1d;
use vyre_primitives::math::conv1d::{conv1d_node, conv1d_program, gaussian_weights, pack_params};

/// Caller-owned scratch for latency-trace smoothing dispatches.
#[derive(Debug, Default)]
pub struct Conv1dLatencySmoothingScratch {
    inputs: Vec<Vec<u8>>,
    weights: Vec<u32>,
    params: Vec<u32>,
}

/// Return the primitive node used when another self-substrate analysis wants
/// to inline latency smoothing into a larger composed program.
#[must_use]
pub fn latency_smoothing_node(input: &str, output: &str, weights: &str, params: &str) -> Node {
    conv1d_node(input, output, weights, params)
}

/// Smooth a fixed-point latency trace with a Gaussian 1D convolution through
/// the selected backend.
///
/// `latency_fixed` is a 16.16 fixed-point latency signal. The returned values
/// are pre-normalization convolution accumulators, matching the primitive
/// contract exactly. Callers that need normalized values divide by `1 << 16`.
///
/// # Errors
///
/// Returns [`DispatchError`] when the input is too large for the primitive
/// buffer contract, `sigma` is not a positive finite value, backend dispatch
/// fails, or readback is malformed.
pub fn smooth_latency_trace_via(
    dispatcher: &impl OptimizerDispatcher,
    latency_fixed: &[u32],
    radius: u32,
    sigma: f32,
) -> Result<Vec<u32>, DispatchError> {
    let mut out = Vec::new();
    smooth_latency_trace_via_into(dispatcher, latency_fixed, radius, sigma, &mut out)?;
    Ok(out)
}

/// Smooth a fixed-point latency trace into caller-owned output storage.
///
/// # Errors
///
/// Returns [`DispatchError`] on invalid smoothing parameters, dispatch failure,
/// or malformed backend output.
pub fn smooth_latency_trace_via_into(
    dispatcher: &impl OptimizerDispatcher,
    latency_fixed: &[u32],
    radius: u32,
    sigma: f32,
    out: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    let mut scratch = Conv1dLatencySmoothingScratch::default();
    smooth_latency_trace_via_with_scratch_into(
        dispatcher,
        latency_fixed,
        radius,
        sigma,
        &mut scratch,
        out,
    )
}

/// Smooth a fixed-point latency trace using caller-owned dispatch scratch.
///
/// # Errors
///
/// Returns [`DispatchError`] on invalid smoothing parameters, dispatch failure,
/// or malformed backend output.
pub fn smooth_latency_trace_via_with_scratch_into(
    dispatcher: &impl OptimizerDispatcher,
    latency_fixed: &[u32],
    radius: u32,
    sigma: f32,
    scratch: &mut Conv1dLatencySmoothingScratch,
    out: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    use crate::observability::{bump, conv1d_latency_smoothing_calls};
    bump(&conv1d_latency_smoothing_calls);

    if latency_fixed.is_empty() {
        out.clear();
        return Ok(());
    }
    if !sigma.is_finite() || sigma <= 0.0 {
        return Err(DispatchError::BadInputs(format!(
            "Fix: smooth_latency_trace_via requires positive finite sigma, got {sigma}."
        )));
    }
    let count = u32::try_from(latency_fixed.len()).map_err(|_| {
        DispatchError::BadInputs(format!(
            "Fix: smooth_latency_trace_via input length {} exceeds u32 primitive count.",
            latency_fixed.len()
        ))
    })?;
    let out_bytes = latency_fixed
        .len()
        .checked_mul(std::mem::size_of::<u32>())
        .ok_or_else(|| {
            DispatchError::BadInputs(format!(
                "Fix: smooth_latency_trace_via input length {} overflows byte count.",
                latency_fixed.len()
            ))
        })?;

    scratch.weights.clear();
    scratch.weights.extend(gaussian_weights(radius, sigma));
    scratch.params.clear();
    scratch.params.extend(pack_params(count, 1, radius));

    ensure_input_slots(&mut scratch.inputs, 4);
    write_u32_slice_le_bytes(&mut scratch.inputs[0], latency_fixed);
    write_zero_bytes(&mut scratch.inputs[1], out_bytes);
    write_u32_slice_le_bytes(&mut scratch.inputs[2], &scratch.weights);
    write_u32_slice_le_bytes(&mut scratch.inputs[3], &scratch.params);

    let program = conv1d_program(count, radius);
    let outputs = dispatcher.dispatch(
        &program,
        &scratch.inputs,
        Some([ceil_div_u32(count, 256), 1, 1]),
    )?;
    if outputs.is_empty() {
        return Err(DispatchError::BackendError(format!(
            "Fix: smooth_latency_trace_via expected at least one output buffer, got {}.",
            outputs.len()
        )));
    }
    decode_u32_output_exact(
        &outputs[0],
        latency_fixed.len(),
        "smooth_latency_trace_via",
        out,
    )
}

/// CPU oracle for latency smoothing, enabled only for parity tests.
#[cfg(test)]
#[must_use]
pub fn reference_smooth_latency_trace(latency_fixed: &[u32], radius: u32, sigma: f32) -> Vec<u32> {
    if latency_fixed.is_empty() {
        return Vec::new();
    }
    let weights = gaussian_weights(radius, sigma);
    cpu_conv1d(latency_fixed, &weights, 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_foundation::ir::Node;

    #[test]
    fn smoothing_node_is_the_conv1d_primitive_node() {
        let node = latency_smoothing_node("latency", "smoothed", "weights", "params");
        match node {
            Node::Region { generator, .. } => {
                assert_eq!(generator.as_str(), vyre_primitives::math::conv1d::OP_ID);
            }
            other => panic!("expected conv1d region node, got {other:?}"),
        }
    }

    #[test]
    fn reference_smoothing_matches_conv1d_cpu_oracle() {
        let trace = [100u32, 1_000, 200, 900, 300];
        let weights = gaussian_weights(1, 1.0);
        let expected = cpu_conv1d(&trace, &weights, 1);
        assert_eq!(reference_smooth_latency_trace(&trace, 1, 1.0), expected);
    }

    #[test]
    fn pack_params_match_latency_axis_contract() {
        assert_eq!(pack_params(7, 1, 2), vec![7, 1, 2, 0]);
    }
}
