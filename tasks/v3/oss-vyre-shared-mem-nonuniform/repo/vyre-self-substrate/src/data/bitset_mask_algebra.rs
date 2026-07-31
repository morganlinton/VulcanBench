//! Scheduler/cache mask algebra via `vyre-primitives::bitset`.
//!
//! Fusion groups, dirty-region filters, resident-cache reuse masks, and
//! invalidation frontiers are all packed bitsets. This module centralizes the
//! common mask operations so self-substrate users consume the same primitive
//! programs that downstream users consume instead of re-implementing bit twiddles
//! in each optimizer pass.

use crate::dispatch_buffers::{
    ceil_div_u32, decode_u32_output_exact, ensure_input_slots, write_u32_slice_le_bytes,
    write_zero_bytes,
};
use crate::optimizer::dispatcher::{DispatchError, OptimizerDispatcher};
use vyre_primitives::bitset::{
    and::bitset_and, clear_bit::bitset_clear_bit, contains::bitset_contains, equal::bitset_equal,
    not::bitset_not, or::bitset_or, set_bit::bitset_set_bit, subset_of::bitset_subset_of,
    test_bit::bitset_test_bit, xor::bitset_xor,
};

#[cfg(any(test, feature = "cpu-parity"))]
use vyre_primitives::bitset::{
    and::cpu_ref as primitive_and, clear_bit::cpu_ref as primitive_clear_bit,
    contains::cpu_ref as primitive_contains, equal::cpu_ref as primitive_equal,
    not::cpu_ref as primitive_not, or::cpu_ref as primitive_or,
    set_bit::cpu_ref as primitive_set_bit, subset_of::cpu_ref as primitive_subset_of,
    test_bit::cpu_ref as primitive_test_bit, xor::cpu_ref as primitive_xor,
};

/// Caller-owned dispatch scratch for bitset mask algebra.
#[derive(Debug, Default)]
pub struct BitsetMaskAlgebraGpuScratch {
    inputs: Vec<Vec<u8>>,
}

/// Mask operation selector for two-input bitset algebra.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BitsetMaskBinaryOp {
    /// `lhs & rhs`.
    And,
    /// `lhs | rhs`.
    Or,
    /// `lhs ^ rhs`.
    Xor,
}

/// Apply one binary mask operation through the primitive GPU program.
///
/// # Errors
///
/// Returns [`DispatchError`] when input lengths differ, word count exceeds the
/// primitive index space, dispatch fails, or readback is malformed.
pub fn mask_binary_via(
    dispatcher: &dyn OptimizerDispatcher,
    op: BitsetMaskBinaryOp,
    lhs: &[u32],
    rhs: &[u32],
) -> Result<Vec<u32>, DispatchError> {
    let mut out = Vec::new();
    mask_binary_via_into(dispatcher, op, lhs, rhs, &mut out)?;
    Ok(out)
}

/// Apply one binary mask operation into caller-owned output storage.
///
/// # Errors
///
/// Returns [`DispatchError`] when validation, dispatch, or readback fails.
pub fn mask_binary_via_into(
    dispatcher: &dyn OptimizerDispatcher,
    op: BitsetMaskBinaryOp,
    lhs: &[u32],
    rhs: &[u32],
    out: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    let mut scratch = BitsetMaskAlgebraGpuScratch::default();
    mask_binary_via_with_scratch_into(dispatcher, op, lhs, rhs, &mut scratch, out)
}

/// Apply one binary mask operation using caller-owned dispatch scratch.
///
/// # Errors
///
/// Returns [`DispatchError`] when validation, dispatch, or readback fails.
pub fn mask_binary_via_with_scratch_into(
    dispatcher: &dyn OptimizerDispatcher,
    op: BitsetMaskBinaryOp,
    lhs: &[u32],
    rhs: &[u32],
    scratch: &mut BitsetMaskAlgebraGpuScratch,
    out: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    use crate::observability::{bitset_mask_algebra_calls, bump};
    bump(&bitset_mask_algebra_calls);

    if lhs.len() != rhs.len() {
        return Err(DispatchError::BadInputs(format!(
            "Fix: mask_binary_via requires lhs.len() == rhs.len(), got {} and {}.",
            lhs.len(),
            rhs.len()
        )));
    }
    if lhs.is_empty() {
        out.clear();
        return Ok(());
    }
    let words = checked_words(lhs.len(), "mask_binary_via")?;
    let program = match op {
        BitsetMaskBinaryOp::And => bitset_and("lhs", "rhs", "out", words),
        BitsetMaskBinaryOp::Or => bitset_or("lhs", "rhs", "out", words),
        BitsetMaskBinaryOp::Xor => bitset_xor("lhs", "rhs", "out", words),
    };
    ensure_input_slots(&mut scratch.inputs, 3);
    write_u32_slice_le_bytes(&mut scratch.inputs[0], lhs);
    write_u32_slice_le_bytes(&mut scratch.inputs[1], rhs);
    write_zero_bytes(
        &mut scratch.inputs[2],
        lhs.len() * std::mem::size_of::<u32>(),
    );
    let outputs = dispatcher.dispatch(
        &program,
        &scratch.inputs,
        Some([ceil_div_u32(words, 256), 1, 1]),
    )?;
    decode_first_output(&outputs, lhs.len(), "mask_binary_via", out)
}

/// Compute `lhs & rhs` through the bitset primitive.
///
/// # Errors
///
/// Returns [`DispatchError`] when validation, dispatch, or readback fails.
pub fn mask_and_via(
    dispatcher: &dyn OptimizerDispatcher,
    lhs: &[u32],
    rhs: &[u32],
) -> Result<Vec<u32>, DispatchError> {
    mask_binary_via(dispatcher, BitsetMaskBinaryOp::And, lhs, rhs)
}

/// Compute `lhs | rhs` through the bitset primitive.
///
/// # Errors
///
/// Returns [`DispatchError`] when validation, dispatch, or readback fails.
pub fn mask_or_via(
    dispatcher: &dyn OptimizerDispatcher,
    lhs: &[u32],
    rhs: &[u32],
) -> Result<Vec<u32>, DispatchError> {
    mask_binary_via(dispatcher, BitsetMaskBinaryOp::Or, lhs, rhs)
}

/// Compute `lhs ^ rhs` through the bitset primitive.
///
/// # Errors
///
/// Returns [`DispatchError`] when validation, dispatch, or readback fails.
pub fn mask_xor_via(
    dispatcher: &dyn OptimizerDispatcher,
    lhs: &[u32],
    rhs: &[u32],
) -> Result<Vec<u32>, DispatchError> {
    mask_binary_via(dispatcher, BitsetMaskBinaryOp::Xor, lhs, rhs)
}

/// Compute `!input` through the bitset primitive.
///
/// # Errors
///
/// Returns [`DispatchError`] when word count exceeds the primitive index space,
/// dispatch fails, or readback is malformed.
pub fn mask_not_via(
    dispatcher: &dyn OptimizerDispatcher,
    input: &[u32],
) -> Result<Vec<u32>, DispatchError> {
    let mut scratch = BitsetMaskAlgebraGpuScratch::default();
    let mut out = Vec::new();
    mask_not_via_with_scratch_into(dispatcher, input, &mut scratch, &mut out)?;
    Ok(out)
}

/// Compute `!input` through the bitset primitive using caller-owned scratch.
///
/// # Errors
///
/// Returns [`DispatchError`] when validation, dispatch, or readback fails.
pub fn mask_not_via_with_scratch_into(
    dispatcher: &dyn OptimizerDispatcher,
    input: &[u32],
    scratch: &mut BitsetMaskAlgebraGpuScratch,
    out: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    use crate::observability::{bitset_mask_algebra_calls, bump};
    bump(&bitset_mask_algebra_calls);

    if input.is_empty() {
        out.clear();
        return Ok(());
    }
    let words = checked_words(input.len(), "mask_not_via")?;
    let program = bitset_not("input", "out", words);
    ensure_input_slots(&mut scratch.inputs, 2);
    write_u32_slice_le_bytes(&mut scratch.inputs[0], input);
    write_zero_bytes(
        &mut scratch.inputs[1],
        input.len() * std::mem::size_of::<u32>(),
    );
    let outputs = dispatcher.dispatch(
        &program,
        &scratch.inputs,
        Some([ceil_div_u32(words, 256), 1, 1]),
    )?;
    decode_first_output(&outputs, input.len(), "mask_not_via", out)
}

/// Test exact equality through the bitset primitive.
///
/// # Errors
///
/// Returns [`DispatchError`] when validation, dispatch, or scalar readback fails.
pub fn mask_equal_via(
    dispatcher: &dyn OptimizerDispatcher,
    lhs: &[u32],
    rhs: &[u32],
) -> Result<bool, DispatchError> {
    scalar_binary_predicate_via(dispatcher, "mask_equal_via", lhs, rhs, bitset_equal)
}

/// Test subset relation through the bitset primitive.
///
/// # Errors
///
/// Returns [`DispatchError`] when validation, dispatch, or scalar readback fails.
pub fn mask_subset_of_via(
    dispatcher: &dyn OptimizerDispatcher,
    lhs: &[u32],
    rhs: &[u32],
) -> Result<bool, DispatchError> {
    scalar_binary_predicate_via(dispatcher, "mask_subset_of_via", lhs, rhs, bitset_subset_of)
}

/// Test whether a bit is present using the index-buffer `contains` primitive.
///
/// # Errors
///
/// Returns [`DispatchError`] when word count exceeds primitive limits, dispatch
/// fails, or scalar readback is malformed.
pub fn mask_contains_via(
    dispatcher: &dyn OptimizerDispatcher,
    input: &[u32],
    bit_idx: u32,
) -> Result<bool, DispatchError> {
    use crate::observability::{bitset_mask_algebra_calls, bump};
    bump(&bitset_mask_algebra_calls);

    let words = checked_words(input.len(), "mask_contains_via")?;
    let program = bitset_contains("input", "index", "out", words);
    let mut scratch = BitsetMaskAlgebraGpuScratch::default();
    ensure_input_slots(&mut scratch.inputs, 3);
    write_u32_slice_le_bytes(&mut scratch.inputs[0], input);
    write_u32_slice_le_bytes(&mut scratch.inputs[1], &[bit_idx]);
    write_zero_bytes(&mut scratch.inputs[2], std::mem::size_of::<u32>());
    let outputs = dispatcher.dispatch(&program, &scratch.inputs, Some([1, 1, 1]))?;
    decode_scalar_bool(&outputs, "mask_contains_via")
}

/// Test a compile-time bit index using the scalar `test_bit` primitive.
///
/// # Errors
///
/// Returns [`DispatchError`] when dispatch fails or scalar readback is malformed.
pub fn mask_test_bit_via(
    dispatcher: &dyn OptimizerDispatcher,
    input: &[u32],
    bit_idx: u32,
) -> Result<bool, DispatchError> {
    use crate::observability::{bitset_mask_algebra_calls, bump};
    bump(&bitset_mask_algebra_calls);

    if (bit_idx / 32) as usize >= input.len() {
        return Ok(false);
    }
    let program = bitset_test_bit("input", bit_idx, "out");
    let mut scratch = BitsetMaskAlgebraGpuScratch::default();
    ensure_input_slots(&mut scratch.inputs, 2);
    write_u32_slice_le_bytes(&mut scratch.inputs[0], input);
    write_zero_bytes(&mut scratch.inputs[1], std::mem::size_of::<u32>());
    let outputs = dispatcher.dispatch(&program, &scratch.inputs, Some([1, 1, 1]))?;
    decode_scalar_bool(&outputs, "mask_test_bit_via")
}

/// Set one bit in a cache/frontier mask through the bitset primitive.
///
/// # Errors
///
/// Returns [`DispatchError`] when word count exceeds primitive limits, dispatch
/// fails, or readback is malformed.
pub fn mask_set_bit_via(
    dispatcher: &dyn OptimizerDispatcher,
    target: &[u32],
    bit_idx: u32,
) -> Result<Vec<u32>, DispatchError> {
    scalar_mutate_bit_via(
        dispatcher,
        "mask_set_bit_via",
        target,
        bit_idx,
        bitset_set_bit,
    )
}

/// Clear one bit in a cache/frontier mask through the bitset primitive.
///
/// # Errors
///
/// Returns [`DispatchError`] when word count exceeds primitive limits, dispatch
/// fails, or readback is malformed.
pub fn mask_clear_bit_via(
    dispatcher: &dyn OptimizerDispatcher,
    target: &[u32],
    bit_idx: u32,
) -> Result<Vec<u32>, DispatchError> {
    scalar_mutate_bit_via(
        dispatcher,
        "mask_clear_bit_via",
        target,
        bit_idx,
        bitset_clear_bit,
    )
}

#[cfg(any(test, feature = "cpu-parity"))]
#[must_use]
pub fn reference_mask_and(lhs: &[u32], rhs: &[u32]) -> Vec<u32> {
    primitive_and(lhs, rhs)
}

#[cfg(any(test, feature = "cpu-parity"))]
#[must_use]
pub fn reference_mask_or(lhs: &[u32], rhs: &[u32]) -> Vec<u32> {
    primitive_or(lhs, rhs)
}

#[cfg(any(test, feature = "cpu-parity"))]
#[must_use]
pub fn reference_mask_xor(lhs: &[u32], rhs: &[u32]) -> Vec<u32> {
    primitive_xor(lhs, rhs)
}

#[cfg(any(test, feature = "cpu-parity"))]
#[must_use]
pub fn reference_mask_not(input: &[u32]) -> Vec<u32> {
    primitive_not(input)
}

#[cfg(any(test, feature = "cpu-parity"))]
#[must_use]
pub fn reference_mask_equal(lhs: &[u32], rhs: &[u32]) -> bool {
    primitive_equal(lhs, rhs) != 0
}

#[cfg(any(test, feature = "cpu-parity"))]
#[must_use]
pub fn reference_mask_subset_of(lhs: &[u32], rhs: &[u32]) -> bool {
    primitive_subset_of(lhs, rhs) != 0
}

#[cfg(any(test, feature = "cpu-parity"))]
#[must_use]
pub fn reference_mask_contains(input: &[u32], bit_idx: u32) -> bool {
    primitive_contains(input, bit_idx) != 0
}

#[cfg(any(test, feature = "cpu-parity"))]
#[must_use]
pub fn reference_mask_test_bit(input: &[u32], bit_idx: u32) -> bool {
    primitive_test_bit(input, bit_idx) != 0
}

#[cfg(any(test, feature = "cpu-parity"))]
#[must_use]
pub fn reference_mask_set_bit(target: &[u32], bit_idx: u32) -> Vec<u32> {
    let mut out = target.to_vec();
    primitive_set_bit(&mut out, bit_idx);
    out
}

#[cfg(any(test, feature = "cpu-parity"))]
#[must_use]
pub fn reference_mask_clear_bit(target: &[u32], bit_idx: u32) -> Vec<u32> {
    let mut out = target.to_vec();
    primitive_clear_bit(&mut out, bit_idx);
    out
}

fn scalar_binary_predicate_via(
    dispatcher: &dyn OptimizerDispatcher,
    context: &'static str,
    lhs: &[u32],
    rhs: &[u32],
    build: fn(&str, &str, &str, u32) -> vyre_foundation::ir::Program,
) -> Result<bool, DispatchError> {
    use crate::observability::{bitset_mask_algebra_calls, bump};
    bump(&bitset_mask_algebra_calls);

    if lhs.len() != rhs.len() {
        return Err(DispatchError::BadInputs(format!(
            "Fix: {context} requires lhs.len() == rhs.len(), got {} and {}.",
            lhs.len(),
            rhs.len()
        )));
    }
    let words = checked_words(lhs.len(), context)?;
    let program = build("lhs", "rhs", "out", words);
    let mut scratch = BitsetMaskAlgebraGpuScratch::default();
    ensure_input_slots(&mut scratch.inputs, 3);
    write_u32_slice_le_bytes(&mut scratch.inputs[0], lhs);
    write_u32_slice_le_bytes(&mut scratch.inputs[1], rhs);
    write_zero_bytes(&mut scratch.inputs[2], std::mem::size_of::<u32>());
    let outputs = dispatcher.dispatch(&program, &scratch.inputs, Some([1, 1, 1]))?;
    decode_scalar_bool(&outputs, context)
}

fn scalar_mutate_bit_via(
    dispatcher: &dyn OptimizerDispatcher,
    context: &'static str,
    target: &[u32],
    bit_idx: u32,
    build: fn(&str, u32, u32) -> vyre_foundation::ir::Program,
) -> Result<Vec<u32>, DispatchError> {
    use crate::observability::{bitset_mask_algebra_calls, bump};
    bump(&bitset_mask_algebra_calls);

    if (bit_idx / 32) as usize >= target.len() {
        return Ok(target.to_vec());
    }
    let words = checked_words(target.len(), context)?;
    let program = build("target", bit_idx, words);
    let mut scratch = BitsetMaskAlgebraGpuScratch::default();
    ensure_input_slots(&mut scratch.inputs, 1);
    write_u32_slice_le_bytes(&mut scratch.inputs[0], target);
    let outputs = dispatcher.dispatch(&program, &scratch.inputs, Some([1, 1, 1]))?;
    let mut out = Vec::new();
    decode_first_output(&outputs, target.len(), context, &mut out)?;
    Ok(out)
}

fn checked_words(len: usize, context: &'static str) -> Result<u32, DispatchError> {
    u32::try_from(len).map_err(|_| {
        DispatchError::BadInputs(format!(
            "Fix: {context} received {len} words, which exceeds the u32 GPU index space."
        ))
    })
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

fn decode_scalar_bool(outputs: &[Vec<u8>], context: &'static str) -> Result<bool, DispatchError> {
    let mut out = Vec::new();
    decode_first_output(outputs, 1, context, &mut out)?;
    Ok(out[0] != 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch_buffers::u32_slice_to_le_bytes;
    use vyre_foundation::ir::Program;

    struct MaskDispatcher;

    impl OptimizerDispatcher for MaskDispatcher {
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
                .expect("Fix: primitive program should contain region generator");
            match op_id {
                vyre_primitives::bitset::and::OP_ID => {
                    assert_eq!(grid_override, Some([1, 1, 1]));
                    binary(inputs, |a, b| a & b)
                }
                vyre_primitives::bitset::or::OP_ID => {
                    assert_eq!(grid_override, Some([1, 1, 1]));
                    binary(inputs, |a, b| a | b)
                }
                vyre_primitives::bitset::xor::OP_ID => {
                    assert_eq!(grid_override, Some([1, 1, 1]));
                    binary(inputs, |a, b| a ^ b)
                }
                vyre_primitives::bitset::not::OP_ID => {
                    assert_eq!(grid_override, Some([1, 1, 1]));
                    let input = crate::hardware::dispatch_buffers::read_u32s(&inputs[0]);
                    Ok(vec![u32_slice_to_le_bytes(
                        &input.iter().map(|word| !word).collect::<Vec<_>>(),
                    )])
                }
                vyre_primitives::bitset::equal::OP_ID => {
                    assert_eq!(grid_override, Some([1, 1, 1]));
                    let lhs = crate::hardware::dispatch_buffers::read_u32s(&inputs[0]);
                    let rhs = crate::hardware::dispatch_buffers::read_u32s(&inputs[1]);
                    Ok(vec![u32_slice_to_le_bytes(&[u32::from(lhs == rhs)])])
                }
                vyre_primitives::bitset::subset_of::OP_ID => {
                    assert_eq!(grid_override, Some([1, 1, 1]));
                    let lhs = crate::hardware::dispatch_buffers::read_u32s(&inputs[0]);
                    let rhs = crate::hardware::dispatch_buffers::read_u32s(&inputs[1]);
                    let ok = lhs.iter().zip(rhs.iter()).all(|(a, b)| (a & !b) == 0);
                    Ok(vec![u32_slice_to_le_bytes(&[u32::from(ok)])])
                }
                vyre_primitives::bitset::contains::OP_ID => {
                    assert_eq!(grid_override, Some([1, 1, 1]));
                    let input = crate::hardware::dispatch_buffers::read_u32s(&inputs[0]);
                    let index = crate::hardware::dispatch_buffers::read_u32s(&inputs[1])[0];
                    Ok(vec![u32_slice_to_le_bytes(&[primitive_contains(
                        &input, index,
                    )])])
                }
                vyre_primitives::bitset::test_bit::OP_ID => {
                    assert_eq!(grid_override, Some([1, 1, 1]));
                    Ok(vec![u32_slice_to_le_bytes(&[1])])
                }
                vyre_primitives::bitset::set_bit::OP_ID => {
                    assert_eq!(grid_override, Some([1, 1, 1]));
                    let mut target = crate::hardware::dispatch_buffers::read_u32s(&inputs[0]);
                    primitive_set_bit(&mut target, 1);
                    Ok(vec![u32_slice_to_le_bytes(&target)])
                }
                vyre_primitives::bitset::clear_bit::OP_ID => {
                    assert_eq!(grid_override, Some([1, 1, 1]));
                    let mut target = crate::hardware::dispatch_buffers::read_u32s(&inputs[0]);
                    primitive_clear_bit(&mut target, 1);
                    Ok(vec![u32_slice_to_le_bytes(&target)])
                }
                other => panic!("unexpected primitive op id {other}"),
            }
        }
    }

    fn binary(
        inputs: &[Vec<u8>],
        op: impl Fn(u32, u32) -> u32,
    ) -> Result<Vec<Vec<u8>>, DispatchError> {
        let lhs = crate::hardware::dispatch_buffers::read_u32s(&inputs[0]);
        let rhs = crate::hardware::dispatch_buffers::read_u32s(&inputs[1]);
        let out = lhs
            .iter()
            .zip(rhs.iter())
            .map(|(a, b)| op(*a, *b))
            .collect::<Vec<_>>();
        Ok(vec![u32_slice_to_le_bytes(&out)])
    }

    #[test]
    fn reference_mask_algebra_matches_primitives_exactly() {
        let lhs = [0xF0F0u32, 0xAAAA_AAAA];
        let rhs = [0x0FF0u32, 0xFFFF_0000];

        assert_eq!(reference_mask_and(&lhs, &rhs), primitive_and(&lhs, &rhs));
        assert_eq!(reference_mask_or(&lhs, &rhs), primitive_or(&lhs, &rhs));
        assert_eq!(reference_mask_xor(&lhs, &rhs), primitive_xor(&lhs, &rhs));
        assert_eq!(reference_mask_not(&lhs), primitive_not(&lhs));
        assert!(reference_mask_equal(&lhs, &lhs));
        assert!(!reference_mask_equal(&lhs, &rhs));
        assert!(reference_mask_subset_of(&[0b0011], &[0b1111]));
        assert!(reference_mask_contains(&[0b1010], 1));
        assert!(reference_mask_test_bit(&[0b1010], 1));
        assert_eq!(reference_mask_set_bit(&[0], 1), vec![0b10]);
        assert_eq!(reference_mask_clear_bit(&[0b11], 1), vec![0b01]);
    }

    #[test]
    fn binary_dispatch_uses_primitive_programs() {
        let lhs = [0xF0F0u32, 0xAAAA_AAAA];
        let rhs = [0x0FF0u32, 0xFFFF_0000];

        assert_eq!(
            mask_and_via(&MaskDispatcher, &lhs, &rhs).unwrap(),
            reference_mask_and(&lhs, &rhs)
        );
        assert_eq!(
            mask_or_via(&MaskDispatcher, &lhs, &rhs).unwrap(),
            reference_mask_or(&lhs, &rhs)
        );
        assert_eq!(
            mask_xor_via(&MaskDispatcher, &lhs, &rhs).unwrap(),
            reference_mask_xor(&lhs, &rhs)
        );
    }

    #[test]
    fn unary_and_scalar_dispatch_use_primitive_programs() {
        assert_eq!(
            mask_not_via(&MaskDispatcher, &[0x0F0F_F0F0]).unwrap(),
            reference_mask_not(&[0x0F0F_F0F0])
        );
        assert!(mask_equal_via(&MaskDispatcher, &[1, 2], &[1, 2]).unwrap());
        assert!(mask_subset_of_via(&MaskDispatcher, &[0b0011], &[0b1111]).unwrap());
        assert!(mask_contains_via(&MaskDispatcher, &[0b1010], 1).unwrap());
        assert!(mask_test_bit_via(&MaskDispatcher, &[0b1010], 1).unwrap());
        assert_eq!(
            mask_set_bit_via(&MaskDispatcher, &[0], 1).unwrap(),
            vec![0b10]
        );
        assert_eq!(
            mask_clear_bit_via(&MaskDispatcher, &[0b11], 1).unwrap(),
            vec![0b01]
        );
    }

    #[test]
    fn scratch_binary_path_reuses_output_capacity() {
        let mut scratch = BitsetMaskAlgebraGpuScratch::default();
        let mut out = Vec::with_capacity(4);
        mask_binary_via_with_scratch_into(
            &MaskDispatcher,
            BitsetMaskBinaryOp::And,
            &[0xFFFF],
            &[0x00FF],
            &mut scratch,
            &mut out,
        )
        .unwrap();
        let out_capacity = out.capacity();
        let input_capacities = scratch.inputs.iter().map(Vec::capacity).collect::<Vec<_>>();

        mask_binary_via_with_scratch_into(
            &MaskDispatcher,
            BitsetMaskBinaryOp::Or,
            &[0xF000],
            &[0x000F],
            &mut scratch,
            &mut out,
        )
        .unwrap();

        assert_eq!(out.capacity(), out_capacity);
        assert_eq!(
            scratch.inputs.iter().map(Vec::capacity).collect::<Vec<_>>(),
            input_capacities
        );
        assert_eq!(out, vec![0xF00F]);
    }

    #[test]
    fn length_mismatch_is_actionable() {
        let err = mask_and_via(&MaskDispatcher, &[1], &[1, 2]).unwrap_err();
        assert!(err.to_string().contains("Fix: mask_binary_via requires"));
    }
}
