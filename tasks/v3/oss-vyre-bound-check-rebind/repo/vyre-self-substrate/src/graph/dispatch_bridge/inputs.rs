use crate::dispatch_buffers::{ensure_input_slots, write_u32_slice_le_bytes, write_zero_u32_words};
use crate::optimizer::dispatcher::DispatchError;

/// Stable fingerprint for a u32 dispatch slice.
///
/// Graph wrappers use this to decide whether large static CSR buffers can stay
/// staged in caller-owned input slots across repeated dispatches. The length is
/// stored separately so same-hash/different-width collisions cannot alias
/// dispatch storage shape.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct U32SliceFingerprint {
    words: usize,
    lo: u64,
    hi: u64,
}

/// Fingerprint a u32 slice without allocating or byte-encoding it first.
#[must_use]
pub(crate) fn fingerprint_u32_slice(values: &[u32]) -> U32SliceFingerprint {
    let mut lo = 0x9E37_79B9_7F4A_7C15_u64 ^ values.len() as u64;
    let mut hi = 0xC2B2_AE3D_27D4_EB4F_u64 ^ (values.len() as u64).rotate_left(32);
    for &value in values {
        let word = u64::from(value);
        lo ^= word
            .wrapping_add(0x9E37_79B9_7F4A_7C15)
            .wrapping_add(lo << 6)
            .wrapping_add(lo >> 2);
        lo = lo.rotate_left(27).wrapping_mul(0x94D0_49BB_1331_11EB);
        hi ^= word
            .rotate_left(17)
            .wrapping_add(lo)
            .wrapping_add(hi << 7)
            .wrapping_add(hi >> 3);
        hi = hi.rotate_left(31).wrapping_mul(0xD6E8_FD9D_DA37_3C91);
    }
    U32SliceFingerprint {
        words: values.len(),
        lo,
        hi,
    }
}

/// A graph primitive dispatcher input buffer.
#[derive(Clone, Copy)]
pub(crate) enum DispatchInput<'a> {
    /// Encode the supplied u32 slice as little-endian bytes.
    U32Slice(&'a [u32]),
    /// Encode the supplied u32 slice, or zero-fill `words` when empty.
    U32SliceOrZeroWords {
        /// Values to encode when present.
        values: &'a [u32],
        /// Number of zero u32 words required when `values` is empty.
        words: usize,
        /// Context included in overflow diagnostics.
        context: &'static str,
    },
    /// Provide a zero-filled u32 scratch/output buffer owned by the caller.
    ZeroU32Words {
        /// Number of u32 words required by the primitive-returned layout.
        words: usize,
        /// Context included in overflow diagnostics.
        context: &'static str,
    },
}

impl<'a> DispatchInput<'a> {
    /// Encode the supplied u32 slice as little-endian bytes.
    pub(crate) fn u32_slice(values: &'a [u32]) -> Self {
        Self::U32Slice(values)
    }

    /// Encode the supplied u32 slice, or zero-fill `words` when empty.
    pub(crate) fn u32_slice_or_zero_words(
        values: &'a [u32],
        words: usize,
        context: &'static str,
    ) -> Self {
        Self::U32SliceOrZeroWords {
            values,
            words,
            context,
        }
    }

    /// Provide a zero-filled u32 scratch/output buffer.
    pub(crate) fn zero_u32_words(words: usize, context: &'static str) -> Self {
        Self::ZeroU32Words { words, context }
    }
}

/// Encode a full prepared-input set into caller-owned scratch.
pub(crate) fn prepare_dispatch_inputs(
    scratch_inputs: &mut Vec<Vec<u8>>,
    inputs: &[DispatchInput<'_>],
) -> Result<(), DispatchError> {
    write_dispatch_inputs(scratch_inputs, inputs)
}

/// Refresh a keyed dispatch input set.
///
/// When `next_key` matches the caller-owned `current_key`, only `mutable_inputs`
/// are re-encoded and the remaining slots stay resident in caller-owned
/// scratch. When the key changes or slot shape is not initialized, the complete
/// input set is encoded and the key is updated. Callers must include every
/// static input byte that affects correctness in `next_key`.
pub(crate) fn refresh_keyed_dispatch_inputs<K: Copy + Eq>(
    scratch_inputs: &mut Vec<Vec<u8>>,
    current_key: &mut Option<K>,
    next_key: K,
    all_inputs: &[DispatchInput<'_>],
    mutable_inputs: &[(usize, DispatchInput<'_>)],
) -> Result<(), DispatchError> {
    if *current_key == Some(next_key) && scratch_inputs.len() == all_inputs.len() {
        for &(slot_index, input) in mutable_inputs {
            let Some(slot) = scratch_inputs.get_mut(slot_index) else {
                return Err(DispatchError::BadInputs(format!(
                    "Fix: keyed graph dispatch mutable input slot {slot_index} exceeds prepared input count {}.",
                    scratch_inputs.len()
                )));
            };
            write_dispatch_input(slot, input)?;
        }
        return Ok(());
    }
    prepare_dispatch_inputs(scratch_inputs, all_inputs)?;
    *current_key = Some(next_key);
    Ok(())
}

pub(super) fn write_dispatch_inputs(
    scratch_inputs: &mut Vec<Vec<u8>>,
    inputs: &[DispatchInput<'_>],
) -> Result<(), DispatchError> {
    ensure_input_slots(scratch_inputs, inputs.len());
    for (slot, input) in scratch_inputs.iter_mut().zip(inputs.iter().copied()) {
        write_dispatch_input(slot, input)?;
    }
    Ok(())
}

/// Encode one dispatch input into an existing prepared-input slot.
pub(crate) fn write_dispatch_input(
    slot: &mut Vec<u8>,
    input: DispatchInput<'_>,
) -> Result<(), DispatchError> {
    match input {
        DispatchInput::U32Slice(values) => write_u32_slice_le_bytes(slot, values),
        DispatchInput::U32SliceOrZeroWords {
            values,
            words,
            context,
        } => {
            if values.is_empty() {
                write_zero_u32_words(slot, words, context)?;
            } else {
                write_u32_slice_le_bytes(slot, values);
            }
        }
        DispatchInput::ZeroU32Words { words, context } => {
            write_zero_u32_words(slot, words, context)?;
        }
    }
    Ok(())
}
