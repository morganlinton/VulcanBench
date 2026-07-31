//! Bitset summary substrate consumer.
//!
//! Wires `vyre_primitives::bitset::popcount` and several companion bitset
//! operations into the dispatch path so the optimizer / cache invalidator can
//! summarize how saturated their reachability / alias / dirty-set bitsets are
//! without each pass re-implementing popcount inline.

#[cfg(any(test, feature = "cpu-parity"))]
use vyre_primitives::bitset::popcount::{
    cpu_ref as primitive_popcount, cpu_ref_into as primitive_popcount_into,
};

use crate::dispatch_buffers::{
    ceil_div_u32, decode_u32_output_exact, ensure_input_slots, write_u32_slice_le_bytes,
    write_zero_bytes,
};
use crate::optimizer::dispatcher::{DispatchError, OptimizerDispatcher};

/// Caller-owned GPU dispatch scratch for bitset-summary kernels.
#[derive(Debug, Default)]
pub struct BitsetSummaryGpuScratch {
    inputs: Vec<Vec<u8>>,
}

/// Per-word popcount via the bitset primitive. Bumps the
/// dataflow-fixpoint substrate counter so dispatch dashboards
/// register every summary.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn per_word_popcount(input: &[u32]) -> Vec<u32> {
    use crate::observability::{bump, dataflow_fixpoint_calls};
    bump(&dataflow_fixpoint_calls);
    primitive_popcount(input)
}

/// Per-word popcount into caller-owned storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn per_word_popcount_into(input: &[u32], out: &mut Vec<u32>) {
    use crate::observability::{bump, dataflow_fixpoint_calls};
    bump(&dataflow_fixpoint_calls);
    primitive_popcount_into(input, out);
}

/// Total set-bit count across the bitset. Saturating-summed so a
/// 32-billion-bit bitset doesn't overflow.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn total_set_bits(input: &[u32]) -> u64 {
    let mut total: u64 = 0;
    for word in input {
        total = total.saturating_add(u64::from(word.count_ones()));
    }
    total
}

/// Saturation ratio in `[0.0, 1.0]`: fraction of bits set across the
/// bitset's full word capacity. The dispatch-time tracker uses this
/// to detect "alias-set is becoming dense, switch to whole-program
/// reachability instead of per-region masks".
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn saturation_ratio(input: &[u32]) -> f64 {
    if input.is_empty() {
        return 0.0;
    }
    let capacity_bits = (input.len() as u64) * 32;
    if capacity_bits == 0 {
        return 0.0;
    }
    let set = total_set_bits(input);
    (set as f64) / (capacity_bits as f64)
}

/// GPU dispatch wrapper around the primitive per-word popcount program.
///
/// # Errors
///
/// Propagates dispatcher errors or malformed readback.
pub fn per_word_popcount_via(
    dispatcher: &dyn OptimizerDispatcher,
    input: &[u32],
) -> Result<Vec<u32>, DispatchError> {
    let mut out = Vec::new();
    per_word_popcount_via_into(dispatcher, input, &mut out)?;
    Ok(out)
}

/// GPU dispatch wrapper around the primitive per-word popcount program into
/// caller-owned output storage.
///
/// # Errors
///
/// Propagates dispatcher errors or malformed readback.
pub fn per_word_popcount_via_into(
    dispatcher: &dyn OptimizerDispatcher,
    input: &[u32],
    out: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    let mut scratch = BitsetSummaryGpuScratch::default();
    per_word_popcount_via_with_scratch_into(dispatcher, input, &mut scratch, out)
}

/// GPU dispatch wrapper around the primitive per-word popcount program into
/// caller-owned dispatch and output storage.
///
/// # Errors
///
/// Propagates dispatcher errors or malformed readback.
pub fn per_word_popcount_via_with_scratch_into(
    dispatcher: &dyn OptimizerDispatcher,
    input: &[u32],
    scratch: &mut BitsetSummaryGpuScratch,
    out: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    if input.is_empty() {
        out.clear();
        return Ok(());
    }
    let word_count = u32::try_from(input.len()).map_err(|_| {
        DispatchError::BadInputs(format!(
            "Fix: per_word_popcount_via input has {} words, which exceeds the u32 GPU index space.",
            input.len()
        ))
    })?;
    let program =
        vyre_primitives::bitset::popcount::bitset_popcount("input", "count_words", word_count);
    ensure_input_slots(&mut scratch.inputs, 2);
    write_u32_slice_le_bytes(&mut scratch.inputs[0], input);
    write_zero_bytes(
        &mut scratch.inputs[1],
        input.len() * std::mem::size_of::<u32>(),
    );
    let grid_x = ceil_div_u32(word_count, 256);
    let outputs = dispatcher.dispatch(&program, &scratch.inputs, Some([grid_x, 1, 1]))?;
    if outputs.is_empty() {
        return Err(DispatchError::BackendError(format!(
            "Fix: per_word_popcount_via expected at least one output buffer, got {}.",
            outputs.len()
        )));
    }
    decode_u32_output_exact(&outputs[0], input.len(), "per_word_popcount_via", out)
}

/// GPU-backed total set-bit count.
///
/// # Errors
///
/// Propagates popcount dispatch errors.
pub fn total_set_bits_via(
    dispatcher: &dyn OptimizerDispatcher,
    input: &[u32],
) -> Result<u64, DispatchError> {
    let counts = per_word_popcount_via(dispatcher, input)?;
    let mut total = 0_u64;
    for count in counts {
        total = total.checked_add(u64::from(count)).ok_or_else(|| {
            DispatchError::BackendError(
                "Fix: total_set_bits_via popcount sum overflowed u64; shard the bitset before summarizing."
                    .to_string(),
            )
        })?;
    }
    Ok(total)
}

/// GPU-backed saturation ratio.
///
/// # Errors
///
/// Propagates popcount dispatch errors.
pub fn saturation_ratio_via(
    dispatcher: &dyn OptimizerDispatcher,
    input: &[u32],
) -> Result<f64, DispatchError> {
    if input.is_empty() {
        return Ok(0.0);
    }
    let capacity_bits = (input.len() as u64) * 32;
    let set = total_set_bits_via(dispatcher, input)?;
    Ok((set as f64) / (capacity_bits as f64))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch_buffers::u32_slice_to_le_bytes;
    use vyre_foundation::ir::Program;

    struct PopcountDispatcher;

    impl OptimizerDispatcher for PopcountDispatcher {
        fn dispatch(
            &self,
            _program: &Program,
            inputs: &[Vec<u8>],
            grid_override: Option<[u32; 3]>,
        ) -> Result<Vec<Vec<u8>>, DispatchError> {
            assert_eq!(grid_override, Some([1, 1, 1]));
            assert_eq!(inputs.len(), 2);
            let input = crate::hardware::dispatch_buffers::read_u32s(&inputs[0]);
            assert_eq!(inputs[1].len(), input.len() * std::mem::size_of::<u32>());
            let out: Vec<u32> = input.iter().map(|word| word.count_ones()).collect();
            Ok(vec![u32_slice_to_le_bytes(&out)])
        }
    }

    #[test]
    fn empty_input_yields_empty_summary() {
        let v = per_word_popcount(&[]);
        assert!(v.is_empty());
        assert_eq!(total_set_bits(&[]), 0);
        assert_eq!(saturation_ratio(&[]), 0.0);
    }

    #[test]
    fn full_word_is_thirty_two_bits() {
        let v = per_word_popcount(&[0xFFFF_FFFFu32]);
        assert_eq!(v, vec![32u32]);
        assert_eq!(total_set_bits(&[0xFFFF_FFFF]), 32);
        assert!((saturation_ratio(&[0xFFFF_FFFF]) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn mixed_words_count_correctly() {
        // 0b1111 = 4 bits, 0b101 = 2 bits.
        let v = per_word_popcount(&[0b1111u32, 0b101]);
        assert_eq!(v, vec![4, 2]);
        assert_eq!(total_set_bits(&[0b1111, 0b101]), 6);
    }

    #[test]
    fn popcount_into_reuses_capacity() {
        let mut out = Vec::with_capacity(8);
        per_word_popcount_into(&[0b1111u32, 0xFFFF_FFFF], &mut out);
        let capacity = out.capacity();
        assert_eq!(out, vec![4, 32]);

        per_word_popcount_into(&[0b1010u32], &mut out);
        assert_eq!(out.capacity(), capacity);
        assert_eq!(out, vec![2]);
    }

    /// Closure-bar: substrate output equals primitive output exactly.
    #[test]
    fn matches_primitive_directly() {
        let input = vec![0u32, 1, 0xFFFF_FFFF, 0xAAAA_AAAA, 0x12345678];
        assert_eq!(per_word_popcount(&input), primitive_popcount(&input));
    }

    /// Adversarial: half-saturated bitset yields ratio 0.5.
    #[test]
    fn half_saturation_ratio() {
        // 0xAAAA_AAAA has 16 bits set out of 32.
        let r = saturation_ratio(&[0xAAAA_AAAAu32]);
        assert!((r - 0.5).abs() < 1e-9, "expected 0.5, got {r}");
    }

    /// Adversarial: a bitset that's 32 entries wide but only one bit
    /// set has saturation ≈ 1/(32*32).
    #[test]
    fn single_bit_in_large_bitset() {
        let mut input = vec![0u32; 32];
        input[5] = 1;
        let r = saturation_ratio(&input);
        let expected = 1.0 / 1024.0;
        assert!((r - expected).abs() < 1e-9);
    }

    /// Idempotence: per_word_popcount on the same input is
    /// deterministic.
    #[test]
    fn deterministic_summary() {
        let input = vec![0xCAFE_BABEu32, 0x1234_5678];
        let a = per_word_popcount(&input);
        let b = per_word_popcount(&input);
        assert_eq!(a, b);
    }

    #[test]
    fn per_word_popcount_via_dispatches_primitive() {
        let input = vec![0u32, 1, 0xFFFF_FFFF, 0xAAAA_AAAA];
        let out = per_word_popcount_via(&PopcountDispatcher, &input).unwrap();
        assert_eq!(out, vec![0, 1, 32, 16]);
    }

    #[test]
    fn per_word_popcount_via_into_reuses_output() {
        let mut out = Vec::with_capacity(8);
        let ptr = out.as_ptr();
        per_word_popcount_via_into(&PopcountDispatcher, &[0b1011], &mut out).unwrap();
        assert_eq!(out, vec![3]);
        assert_eq!(out.as_ptr(), ptr);
    }

    #[test]
    fn per_word_popcount_via_with_scratch_reuses_dispatch_and_output_storage() {
        let mut scratch = BitsetSummaryGpuScratch::default();
        let mut out = Vec::with_capacity(4);

        per_word_popcount_via_with_scratch_into(
            &PopcountDispatcher,
            &[0b1011, 0xFFFF_FFFF],
            &mut scratch,
            &mut out,
        )
        .unwrap();

        let input_capacities = scratch.inputs.iter().map(Vec::capacity).collect::<Vec<_>>();
        let out_capacity = out.capacity();

        per_word_popcount_via_with_scratch_into(
            &PopcountDispatcher,
            &[0b0101, 0xAAAA_AAAA],
            &mut scratch,
            &mut out,
        )
        .unwrap();

        assert_eq!(
            scratch.inputs.iter().map(Vec::capacity).collect::<Vec<_>>(),
            input_capacities
        );
        assert_eq!(out.capacity(), out_capacity);
        assert_eq!(out, vec![2, 16]);
    }

    #[test]
    fn total_and_ratio_via_match_host_contract() {
        let input = vec![0xFFFF_FFFFu32, 0];
        assert_eq!(total_set_bits_via(&PopcountDispatcher, &input).unwrap(), 32);
        assert!((saturation_ratio_via(&PopcountDispatcher, &input).unwrap() - 0.5).abs() < 1e-9);
    }

    #[test]
    fn production_source_keeps_cpu_bitset_helpers_out_of_via_path() {
        let source = include_str!("bitset_summary.rs");
        let via_section = source
            .split("pub fn per_word_popcount_via")
            .nth(1)
            .expect("Fix: via section should exist")
            .split("#[cfg(test)]\nmod tests")
            .next()
            .expect("Fix: test module marker should exist");

        assert!(!via_section.contains("primitive_popcount"));
        assert!(!via_section.contains("cpu_ref"));
        assert!(!via_section.contains("saturating_add"));
    }
}
