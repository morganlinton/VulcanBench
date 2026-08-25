//! Tensor-train chain fusion analyzer (#6 substrate).
//!
//! Frames a sequence of Regions as a Tensor Train where:
//! - Each Region $R_i$ is a TT-core $G_i$.
//! - The bond dimension $r_i$ is the rank (element count) of the
//!   shared buffer between $R_i$ and $R_{i+1}$.
//! - The contraction $G_1 \cdot G_2 \cdot \dots \cdot G_n$ computes
//!   the "fusion pressure" or "total shared volume" across the chain.
//!
//! This module uses `vyre-primitives::math::tensor_train::tt_contract_step`
//! (the same Program shipped to users) to analyze Vyre's own IR.

use crate::dispatch_buffers::{
    ceil_div_u32, decode_u32_output_exact, ensure_input_slots, write_u32_slice_le_bytes,
    write_zero_bytes,
};
use crate::hardware::scratch::reserve_vec_capacity;
use crate::optimizer::dispatcher::{DispatchError, OptimizerDispatcher};
use vyre_primitives::math::tensor_train::tt_contract_step;
#[cfg(test)]
use vyre_primitives::math::tensor_train::tt_contract_step_cpu;

const FIXED_ONE: u32 = 1 << 16;
const MAX_TT_DISPATCH_CELLS: u32 = 1 << 20;

/// Caller-owned GPU dispatch scratch for tensor-train fusion pressure.
#[derive(Debug, Default)]
pub struct TensorTrainFusionGpuScratch {
    acc: Vec<u32>,
    core_slice: Vec<u32>,
    step_out: Vec<u32>,
    inputs: Vec<Vec<u8>>,
}

/// Parity-only CPU oracle for the fusion pressure of a chain of Regions.
///
/// Production callers must use [`fusion_pressure_via`] or [`should_fuse_chain_via`], which dispatch
/// the same TT contraction primitive through the selected backend.
#[cfg(test)]
#[must_use]
pub fn reference_fusion_pressure(shared_buffer_ranks: &[u32]) -> f64 {
    use crate::observability::{bump, tensor_train_chain_fusion_calls};
    bump(&tensor_train_chain_fusion_calls);
    if shared_buffer_ranks.is_empty() {
        return 0.0;
    }

    // Initial accumulator for r_0 = 1.
    let mut acc = vec![1.0];

    for &r_next in shared_buffer_ranks {
        let r_prev = acc.len() as u32;
        // Skip zero-rank buffers as they indicate no dataflow.
        if r_next == 0 {
            continue;
        }

        // Use a "unit core" - all ones.
        // acc_out[b] = Σ_a acc_in[a] · core[a, b] = Σ_a 1 · 1 = r_prev.
        // Result: acc_out is a vector of length r_next containing r_prev.
        let core_slice = vec![1.0; (r_prev * r_next) as usize];
        acc = tt_contract_step_cpu(&acc, &core_slice, r_prev, r_next);
    }

    // Final contraction to scalar (last bond is 1).
    let r_last = acc.len() as u32;
    let core_last = vec![1.0; r_last as usize];
    let result = tt_contract_step_cpu(&acc, &core_last, r_last, 1);

    result[0]
}

/// Compute fusion pressure through the GPU-dispatchable TT contraction primitive.
///
/// This is the production path for callers that have a concrete backend dispatcher. It uses the
/// same unit-core model as [`fusion_pressure`]. Accumulator lanes carry the integer rank product;
/// unit-core lanes are encoded as 16.16 fixed-point `1.0`, matching
/// `vyre_primitives::math::tensor_train::tt_contract_step`'s multiply-then-shift contract without
/// overflowing `1.0 * 1.0` in u32 space.
///
/// # Errors
///
/// Returns [`DispatchError::BadInputs`] when a rank is too large for buffer sizing, the fixed-point
/// result would overflow the primitive's u32 lanes, dispatch fails, or the backend returns a
/// truncated output buffer.
pub fn fusion_pressure_via(
    dispatcher: &dyn OptimizerDispatcher,
    shared_buffer_ranks: &[u32],
) -> Result<f64, DispatchError> {
    let mut scratch = TensorTrainFusionGpuScratch::default();
    fusion_pressure_via_with_scratch(dispatcher, shared_buffer_ranks, &mut scratch)
}

/// Compute fusion pressure through the GPU-dispatchable TT contraction
/// primitive using caller-owned dispatch and intermediate storage.
pub fn fusion_pressure_via_with_scratch(
    dispatcher: &dyn OptimizerDispatcher,
    shared_buffer_ranks: &[u32],
    scratch: &mut TensorTrainFusionGpuScratch,
) -> Result<f64, DispatchError> {
    use crate::observability::{bump, tensor_train_chain_fusion_calls};
    bump(&tensor_train_chain_fusion_calls);
    if shared_buffer_ranks.is_empty() {
        return Ok(0.0);
    }

    scratch.acc.clear();
    scratch.acc.push(1);
    let max_rank = shared_buffer_ranks
        .iter()
        .copied()
        .filter(|&rank| rank != 0)
        .max()
        .unwrap_or(1) as usize;
    reserve_vec_capacity(
        &mut scratch.acc,
        max_rank,
        "tensor-train accumulator scratch",
    )?;
    reserve_vec_capacity(
        &mut scratch.step_out,
        max_rank,
        "tensor-train output scratch",
    )?;
    let mut exact_product = 1u128;
    for &r_next in shared_buffer_ranks {
        if r_next == 0 {
            continue;
        }
        exact_product = exact_product.checked_mul(r_next as u128).ok_or_else(|| {
            DispatchError::BadInputs(
                "Fix: fusion_pressure_via rank product overflowed u128.".to_string(),
            )
        })?;
        if exact_product > u32::MAX as u128 {
            return Err(DispatchError::BadInputs(format!(
                "Fix: fusion_pressure_via rank product would overflow u32 lanes; product={exact_product}, max={}.",
                u32::MAX
            )));
        }
        let r_prev = u32::try_from(scratch.acc.len()).map_err(|_| {
            DispatchError::BadInputs(
                "Fix: fusion_pressure_via accumulator length exceeds u32.".to_string(),
            )
        })?;
        let core_len = checked_cells(r_prev, r_next, "r_prev*r_next")?;
        scratch.core_slice.clear();
        scratch.core_slice.resize(core_len, FIXED_ONE);
        dispatch_tt_step_with_scratch_into(
            dispatcher,
            &scratch.acc,
            &scratch.core_slice,
            r_prev,
            r_next,
            &mut scratch.inputs,
            &mut scratch.step_out,
        )?;
        std::mem::swap(&mut scratch.acc, &mut scratch.step_out);
    }

    let r_last = u32::try_from(scratch.acc.len()).map_err(|_| {
        DispatchError::BadInputs("Fix: fusion_pressure_via final rank exceeds u32.".to_string())
    })?;
    scratch.core_slice.clear();
    scratch.core_slice.resize(r_last as usize, FIXED_ONE);
    dispatch_tt_step_with_scratch_into(
        dispatcher,
        &scratch.acc,
        &scratch.core_slice,
        r_last,
        1,
        &mut scratch.inputs,
        &mut scratch.step_out,
    )?;
    Ok(scratch.step_out.first().copied().unwrap_or(0) as f64)
}

/// Decide whether to fuse a chain using the GPU-dispatchable TT contraction primitive.
///
/// # Errors
///
/// Propagates [`fusion_pressure_via`] dispatch and input-shape failures.
pub fn should_fuse_chain_via(
    dispatcher: &dyn OptimizerDispatcher,
    shared_buffer_ranks: &[u32],
    threshold_per_link: f64,
) -> Result<bool, DispatchError> {
    if shared_buffer_ranks.is_empty() {
        return Ok(false);
    }
    let pressure = fusion_pressure_via(dispatcher, shared_buffer_ranks)?;
    let n = shared_buffer_ranks.len() as f64;
    Ok(pressure.ln() / n <= threshold_per_link.ln())
}

fn dispatch_tt_step_with_scratch_into(
    dispatcher: &dyn OptimizerDispatcher,
    acc_in: &[u32],
    core_slice: &[u32],
    r_prev: u32,
    r_next: u32,
    inputs: &mut Vec<Vec<u8>>,
    out: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    let program = tt_contract_step("acc_in", "core_slice", "acc_out", r_prev, r_next);
    let output_len = r_next as usize;
    ensure_input_slots(inputs, 3);
    write_u32_slice_le_bytes(&mut inputs[0], acc_in);
    write_u32_slice_le_bytes(&mut inputs[1], core_slice);
    write_zero_bytes(&mut inputs[2], output_len * std::mem::size_of::<u32>());
    let grid_x = ceil_div_u32(r_next, 256);
    let outputs = dispatcher.dispatch(&program, inputs, Some([grid_x, 1, 1]))?;
    if outputs.is_empty() {
        return Err(DispatchError::BackendError(format!(
            "Fix: fusion_pressure_via TT expected at least one output buffer, got {}.",
            outputs.len()
        )));
    }
    decode_u32_output_exact(&outputs[0], output_len, "fusion_pressure_via TT", out)
}

fn checked_cells(left: u32, right: u32, label: &str) -> Result<usize, DispatchError> {
    let value = left.checked_mul(right).ok_or_else(|| {
        DispatchError::BadInputs(format!(
            "Fix: fusion_pressure_via buffer size overflows {label}: {left} * {right}."
        ))
    })?;
    if value > MAX_TT_DISPATCH_CELLS {
        return Err(DispatchError::BadInputs(format!(
            "Fix: fusion_pressure_via refuses to allocate {value} TT core cells for {label}; max is {MAX_TT_DISPATCH_CELLS}. Shard the chain or lower ranks before dispatch."
        )));
    }
    Ok(value as usize)
}

/// Decide whether to fuse a chain based on its TT fusion pressure.
///
/// A chain should be fused if its total intermediate volume (pressure)
/// is below the threshold relative to the number of regions.
#[must_use]
pub fn should_fuse_chain(shared_buffer_ranks: &[u32], threshold_per_link: f64) -> bool {
    if shared_buffer_ranks.is_empty() {
        return false;
    }
    let n = shared_buffer_ranks.len() as f64;
    // Logarithmic scale because TT contraction of unit cores is multiplicative.
    // log(r1 * r2 * ... * rn) = Σ log(ri)
    // We compare average log-rank against the threshold.
    let log_sum = shared_buffer_ranks
        .iter()
        .copied()
        .filter(|&rank| rank != 0)
        .map(|rank| (rank as f64).ln())
        .sum::<f64>();
    let avg_log_rank = log_sum / n;
    avg_log_rank <= threshold_per_link.ln()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch_buffers::u32_slice_to_le_bytes;

    fn approx_eq(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-10 * (1.0 + a.abs() + b.abs())
    }

    struct ReferenceDispatcher;

    impl OptimizerDispatcher for ReferenceDispatcher {
        fn dispatch(
            &self,
            program: &vyre_foundation::ir::Program,
            inputs: &[Vec<u8>],
            _grid_override: Option<[u32; 3]>,
        ) -> Result<Vec<Vec<u8>>, DispatchError> {
            let [acc_bytes, core_bytes, out_bytes] = inputs else {
                return Err(DispatchError::BadInputs(format!(
                    "Fix: TT test dispatcher expected 3 buffers, got {}.",
                    inputs.len()
                )));
            };
            let acc = crate::hardware::dispatch_buffers::decode_u32_input_aligned(
                acc_bytes,
                "TT test dispatcher",
            )?;
            let core = crate::hardware::dispatch_buffers::decode_u32_input_aligned(
                core_bytes,
                "TT test dispatcher",
            )?;
            let out_len = out_bytes.len() / 4;
            if out_len == 0 || acc.is_empty() || core.len() != acc.len() * out_len {
                return Err(DispatchError::BadInputs(format!(
                    "Fix: invalid TT test-dispatch shape acc={}, core={}, out={out_len}.",
                    acc.len(),
                    core.len()
                )));
            }
            assert_eq!(program.workgroup_size(), [256, 1, 1]);
            let mut out = vec![0u32; out_len];
            for b in 0..out_len {
                let mut sum = 0u64;
                for a in 0..acc.len() {
                    sum =
                        sum.wrapping_add(((acc[a] as u64) * (core[a * out_len + b] as u64)) >> 16);
                }
                out[b] = sum as u32;
            }
            Ok(vec![u32_slice_to_le_bytes(&out)])
        }
    }

    #[test]
    fn pressure_of_single_link_is_rank() {
        let ranks = vec![64];
        assert!(approx_eq(reference_fusion_pressure(&ranks), 64.0));
    }

    #[test]
    fn pressure_is_multiplicative_product() {
        // r0=1, r1=4, r2=8 -> result = 1 * 4 * 8 = 32.
        let ranks = vec![4, 8];
        assert!(approx_eq(reference_fusion_pressure(&ranks), 32.0));
    }

    #[test]
    fn large_ranks_produce_high_pressure() {
        let ranks = vec![1024, 1024];
        assert!(approx_eq(
            reference_fusion_pressure(&ranks),
            1024.0 * 1024.0
        ));
    }

    #[test]
    fn should_fuse_small_chain() {
        let ranks = vec![8, 8, 8];
        // ln(8*8*8)/3 = ln(8)
        assert!(should_fuse_chain(&ranks, 16.0));
        assert!(!should_fuse_chain(&ranks, 4.0));
    }

    #[test]
    fn parity_with_raw_product() {
        let ranks = vec![2, 3, 5, 7];
        let pressure = reference_fusion_pressure(&ranks);
        let expected = (2 * 3 * 5 * 7) as f64;
        assert!(approx_eq(pressure, expected));
    }

    #[test]
    fn production_source_does_not_call_cpu_tensor_train_helpers() {
        let source = include_str!("tensor_train_chain_fusion.rs");
        let production_source = source
            .split("#[cfg(test)]")
            .next()
            .expect("Fix: source prefix exists");
        assert!(
            !production_source.contains("tt_contract_step_cpu")
                && !production_source.contains("reference_fusion_pressure(shared_buffer_ranks)"),
            "Fix: tensor-train chain fusion production paths must dispatch GPU-capable primitives or use exact metadata, not CPU helper calls."
        );
    }

    #[test]
    fn via_pressure_matches_unit_core_reference() {
        let dispatcher = ReferenceDispatcher;
        let ranks = vec![2, 3, 5];
        let pressure = fusion_pressure_via(&dispatcher, &ranks).expect("Fix: TT dispatch succeeds");
        assert!(approx_eq(pressure, reference_fusion_pressure(&ranks)));
    }

    #[test]
    fn via_pressure_with_scratch_reuses_acc_core_dispatch_and_step_storage() {
        let dispatcher = ReferenceDispatcher;
        let ranks = vec![2, 3, 5];
        let mut scratch = TensorTrainFusionGpuScratch::default();

        let pressure = fusion_pressure_via_with_scratch(&dispatcher, &ranks, &mut scratch).unwrap();
        assert!(approx_eq(pressure, reference_fusion_pressure(&ranks)));

        let acc_pool_capacity = scratch.acc.capacity() + scratch.step_out.capacity();
        let core_capacity = scratch.core_slice.capacity();
        let input_capacities = scratch.inputs.iter().map(Vec::capacity).collect::<Vec<_>>();

        let pressure = fusion_pressure_via_with_scratch(&dispatcher, &ranks, &mut scratch).unwrap();

        assert!(approx_eq(pressure, reference_fusion_pressure(&ranks)));
        assert_eq!(
            scratch.acc.capacity() + scratch.step_out.capacity(),
            acc_pool_capacity
        );
        assert_eq!(scratch.core_slice.capacity(), core_capacity);
        assert_eq!(
            scratch.inputs.iter().map(Vec::capacity).collect::<Vec<_>>(),
            input_capacities
        );
    }

    #[test]
    fn via_should_fuse_matches_reference_decision() {
        let dispatcher = ReferenceDispatcher;
        let ranks = vec![8, 8, 8];
        assert_eq!(
            should_fuse_chain_via(&dispatcher, &ranks, 16.0).expect("Fix: TT dispatch succeeds"),
            should_fuse_chain(&ranks, 16.0)
        );
        assert_eq!(
            should_fuse_chain_via(&dispatcher, &ranks, 4.0).expect("Fix: TT dispatch succeeds"),
            should_fuse_chain(&ranks, 4.0)
        );
    }

    #[test]
    fn via_rejects_fixed_point_overflow() {
        let dispatcher = ReferenceDispatcher;
        let error = fusion_pressure_via(&dispatcher, &[u32::MAX])
            .expect_err("oversized TT core must be rejected before allocation or dispatch");
        assert!(
            error.to_string().contains("refuses to allocate")
                || error.to_string().contains("overflow"),
            "expected allocation/overflow error, got {error}"
        );
    }
}
