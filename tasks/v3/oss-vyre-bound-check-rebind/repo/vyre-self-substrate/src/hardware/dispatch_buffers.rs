//! Shared byte-buffer helpers for self-substrate dispatcher calls.
//!
//! Production self-substrate paths build primitive Programs and cross the
//! backend boundary through [`crate::optimizer::dispatcher::OptimizerDispatcher`].
//! Keeping shape checks and little-endian u32 marshalling here prevents every
//! module from growing its own subtly different host-side contract.

use crate::optimizer::dispatcher::DispatchError;

/// Compute `ceil(n / d)` for dispatch-grid sizing.
#[must_use]
pub(crate) fn ceil_div_u32(n: u32, d: u32) -> u32 {
    n.div_ceil(d).max(1)
}

/// Return `n * n` as `usize`, rejecting zero and overflow with an actionable
/// dispatcher error.
pub(crate) fn checked_square_cells(n: u32, context: &str) -> Result<usize, DispatchError> {
    if n == 0 {
        return Err(DispatchError::BadInputs(format!(
            "Fix: {context} requires n > 0."
        )));
    }
    let n_us = n as usize;
    n_us.checked_mul(n_us).ok_or_else(|| {
        DispatchError::BadInputs(format!("Fix: {context} n*n overflows usize for n={n}."))
    })
}

/// Return `left * right` as `usize`, rejecting zeros and overflow with an
/// actionable dispatcher error.
pub(crate) fn checked_product_count(
    left: u32,
    right: u32,
    left_name: &str,
    right_name: &str,
    context: &str,
) -> Result<usize, DispatchError> {
    if left == 0 || right == 0 {
        return Err(DispatchError::BadInputs(format!(
            "Fix: {context} requires {left_name} > 0 and {right_name} > 0, got {left_name}={left}, {right_name}={right}."
        )));
    }
    let left_us = left as usize;
    let right_us = right as usize;
    left_us.checked_mul(right_us).ok_or_else(|| {
        DispatchError::BadInputs(format!(
            "Fix: {context} {left_name}*{right_name} overflows usize for {left_name}={left}, {right_name}={right}."
        ))
    })
}

/// Encode a u32 slice as little-endian bytes for dispatcher input buffers.
///
/// Routes through the canonical `vyre-primitives::wire::pack_u32_slice`
/// LEGO primitive (with `bytemuck::cast_slice` fast path on LE hosts).
/// Dispatcher input-buffer encoding now matches every other GPU upload
/// path's throughput floor instead of running its own scalar `extend`
/// loop.
#[must_use]
pub(crate) fn u32_slice_to_le_bytes(values: &[u32]) -> Vec<u8> {
    vyre_primitives::wire::pack_u32_slice(values)
}

/// Ensure a dispatcher input-vector shell has exactly `count` reusable slots.
///
/// Dispatcher calls consume the whole `Vec<Vec<u8>>`. Leaving stale slots after
/// a scratch object moves from a wider primitive to a narrower primitive silently
/// changes the backend ABI and can force needless uploads. Active slots keep
/// their allocation; inactive slots are dropped instead of being passed on.
pub(crate) fn ensure_input_slots(inputs: &mut Vec<Vec<u8>>, count: usize) {
    if inputs.len() < count {
        inputs.resize_with(count, Vec::new);
    } else if inputs.len() > count {
        inputs.truncate(count);
    }
}

/// Fill a reusable dispatcher byte buffer with zeros without replacing the
/// allocation.
pub(crate) fn write_zero_bytes(out: &mut Vec<u8>, len: usize) {
    if out.len() == len {
        if out.iter().any(|&byte| byte != 0) {
            out.fill(0);
        }
    } else {
        out.clear();
        out.resize(len, 0);
    }
}

/// Return the exact byte count needed for `count` u32 words.
pub(crate) fn u32_word_bytes(count: usize, context: &str) -> Result<usize, DispatchError> {
    count
        .checked_mul(std::mem::size_of::<u32>())
        .ok_or_else(|| {
            DispatchError::BadInputs(format!(
                "Fix: {context} byte count overflows usize for {count} u32 word(s)."
            ))
        })
}

/// Fill a reusable dispatcher byte buffer with `count` zeroed u32 words.
pub(crate) fn write_zero_u32_words(
    out: &mut Vec<u8>,
    count: usize,
    context: &str,
) -> Result<(), DispatchError> {
    let bytes = u32_word_bytes(count, context)?;
    write_zero_bytes(out, bytes);
    Ok(())
}

/// Encode a u32 slice as little-endian bytes into caller-owned dispatcher
/// input storage. Routes through `vyre-primitives::wire::pack_u32_slice_into`
/// so dispatcher writes use the same LE-host `bytemuck::cast_slice` fast
/// path as every other GPU-upload site.
pub(crate) fn write_u32_slice_le_bytes(out: &mut Vec<u8>, values: &[u32]) {
    vyre_primitives::wire::pack_u32_slice_into(values, out);
}

/// Encode an f32 slice as little-endian bytes for dispatcher input buffers.
#[must_use]
#[cfg(test)]
pub(crate) fn f32_slice_to_le_bytes(values: &[f32]) -> Vec<u8> {
    vyre_primitives::wire::pack_f32_slice(values)
}

/// Decode an aligned u32 input buffer for test dispatchers.
#[cfg(test)]
pub(crate) fn decode_u32_input_aligned(
    bytes: &[u8],
    context: &str,
) -> Result<Vec<u32>, DispatchError> {
    if bytes.len() % std::mem::size_of::<u32>() != 0 {
        return Err(DispatchError::BadInputs(format!(
            "Fix: {context} input byte count {} is not divisible by 4.",
            bytes.len()
        )));
    }
    Ok(vyre_primitives::wire::decode_u32_le_bytes_all(bytes))
}

/// Decode an aligned f32 input buffer for test dispatchers.
#[cfg(test)]
pub(crate) fn decode_f32_input_aligned(
    bytes: &[u8],
    context: &str,
) -> Result<Vec<f32>, DispatchError> {
    if bytes.len() % std::mem::size_of::<f32>() != 0 {
        return Err(DispatchError::BadInputs(format!(
            "Fix: {context} input byte count {} is not divisible by 4.",
            bytes.len()
        )));
    }
    Ok(vyre_primitives::wire::decode_f32_le_bytes_all(bytes))
}

/// Decode a u32 byte buffer for tests and explicit CPU-parity dispatchers that
/// intentionally validate through the same lenient scalar oracle used before centralization.
#[cfg(any(test, feature = "cpu-parity"))]
#[must_use]
pub(crate) fn read_u32s(bytes: &[u8]) -> Vec<u32> {
    vyre_primitives::wire::decode_u32_le_bytes_all(bytes)
}

/// Decode an f32 byte buffer for tests that intentionally validate through the
/// same lenient scalar oracle used before centralization.
#[cfg(test)]
#[must_use]
pub(crate) fn read_f32s(bytes: &[u8]) -> Vec<f32> {
    vyre_primitives::wire::decode_f32_le_bytes_all(bytes)
}

/// Encode an f32 slice as little-endian bytes into caller-owned dispatcher
/// input storage.
pub(crate) fn write_f32_slice_le_bytes(out: &mut Vec<u8>, values: &[f32]) {
    vyre_primitives::wire::pack_f32_slice_into(values, out);
}

/// Decode a dispatcher u32 output buffer with exact byte-count validation.
pub(crate) fn decode_u32_output_exact(
    bytes: &[u8],
    expected_words: usize,
    context: &str,
    out: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    let expected_bytes = expected_words
        .checked_mul(std::mem::size_of::<u32>())
        .ok_or_else(|| {
            DispatchError::BackendError(format!(
                "Fix: {context} output byte count overflowed usize."
            ))
        })?;
    if bytes.len() != expected_bytes {
        return Err(DispatchError::BackendError(format!(
            "Fix: {context} expected {expected_bytes} output bytes, got {}.",
            bytes.len()
        )));
    }

    vyre_primitives::wire::unpack_u32_slice_into(bytes, expected_words, context, out)
        .map_err(DispatchError::BackendError)
}

/// Decode a dispatcher i32 output buffer with exact byte-count validation.
pub(crate) fn decode_i32_output_exact(
    bytes: &[u8],
    expected_words: usize,
    context: &str,
    out: &mut Vec<i32>,
) -> Result<(), DispatchError> {
    let expected_bytes = expected_words
        .checked_mul(std::mem::size_of::<i32>())
        .ok_or_else(|| {
            DispatchError::BackendError(format!(
                "Fix: {context} output byte count overflowed usize."
            ))
        })?;
    if bytes.len() != expected_bytes {
        return Err(DispatchError::BackendError(format!(
            "Fix: {context} expected {expected_bytes} output bytes, got {}.",
            bytes.len()
        )));
    }

    out.clear();
    out.reserve(expected_words);
    out.extend(vyre_primitives::wire::decode_i32_le_bytes_all(bytes));
    Ok(())
}

/// Decode a dispatcher f32 output buffer with exact byte-count validation.
pub(crate) fn decode_f32_output_exact(
    bytes: &[u8],
    expected_words: usize,
    context: &str,
    out: &mut Vec<f32>,
) -> Result<(), DispatchError> {
    let expected_bytes = expected_words
        .checked_mul(std::mem::size_of::<f32>())
        .ok_or_else(|| {
            DispatchError::BackendError(format!(
                "Fix: {context} output byte count overflowed usize."
            ))
        })?;
    if bytes.len() != expected_bytes {
        return Err(DispatchError::BackendError(format!(
            "Fix: {context} expected {expected_bytes} output bytes, got {}.",
            bytes.len()
        )));
    }

    vyre_primitives::wire::unpack_f32_slice_into(bytes, expected_words, context, out)
        .map_err(DispatchError::BackendError)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn u32_word_bytes_rejects_usize_overflow() {
        let overflowing_words = usize::MAX / std::mem::size_of::<u32>() + 1;
        let err = u32_word_bytes(overflowing_words, "dispatch-buffer test")
            .expect_err("overflowing u32 word count must be rejected");
        assert!(
            matches!(err, DispatchError::BadInputs(ref message) if message.contains("dispatch-buffer test")),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn zero_u32_words_preserves_allocation_and_exact_byte_count() {
        let mut bytes = Vec::with_capacity(64);
        let ptr = bytes.as_ptr();
        write_zero_u32_words(&mut bytes, 3, "zero test").expect("Fix: zeroing succeeds");
        assert_eq!(bytes, vec![0; 12]);
        assert_eq!(bytes.as_ptr(), ptr);
    }

    #[test]
    fn zero_bytes_reuses_already_sized_zero_buffer_without_reallocation() {
        let mut bytes = vec![0u8; 32];
        let ptr = bytes.as_ptr();

        write_zero_bytes(&mut bytes, 32);

        assert_eq!(bytes, vec![0; 32]);
        assert_eq!(bytes.as_ptr(), ptr);
    }

    #[test]
    fn zero_bytes_clears_dirty_same_size_buffer_without_reallocation() {
        let mut bytes = vec![0xA5u8; 32];
        let ptr = bytes.as_ptr();

        write_zero_bytes(&mut bytes, 32);

        assert_eq!(bytes, vec![0; 32]);
        assert_eq!(bytes.as_ptr(), ptr);
    }
}
