use super::*;

/// Unpack packed signed INT4 lanes through the dispatch backend.
///
/// `packed_words` stores eight signed 4-bit lanes per u32 word. The returned
/// vector has exactly `lane_count` i32 lanes.
///
/// # Errors
///
/// Returns [`DispatchError`] when lane count is zero, packed input shape is
/// wrong, byte-count arithmetic overflows, dispatch fails, or backend readback
/// is malformed.
pub fn unpack_i4x8_via(
    dispatcher: &impl OptimizerDispatcher,
    packed_words: &[u32],
    lane_count: u32,
) -> Result<Vec<i32>, DispatchError> {
    let mut scratch = QuantizedUnpackGpuScratch::default();
    let mut out = Vec::new();
    unpack_i4x8_via_with_scratch_into(
        dispatcher,
        packed_words,
        lane_count,
        &mut scratch,
        &mut out,
    )?;
    Ok(out)
}

/// Unpack packed signed INT4 lanes through caller-owned scratch and output.
///
/// # Errors
///
/// Returns [`DispatchError`] under the same conditions as [`unpack_i4x8_via`].
pub fn unpack_i4x8_via_with_scratch_into(
    dispatcher: &impl OptimizerDispatcher,
    packed_words: &[u32],
    lane_count: u32,
    scratch: &mut QuantizedUnpackGpuScratch,
    out: &mut Vec<i32>,
) -> Result<(), DispatchError> {
    if lane_count == 0 {
        return Err(DispatchError::BadInputs(
            "Fix: unpack_i4x8_via requires lane_count > 0.".to_string(),
        ));
    }
    let expected_words = i4_packed_words(lane_count) as usize;
    if packed_words.len() != expected_words {
        return Err(DispatchError::BadInputs(format!(
            "Fix: unpack_i4x8_via requires packed_words.len() == i4_packed_words(lane_count), got len={} expected={expected_words} for lane_count={lane_count}.",
            packed_words.len()
        )));
    }
    let lane_words = lane_count as usize;
    let out_bytes = lane_words
        .checked_mul(std::mem::size_of::<i32>())
        .ok_or_else(|| {
            DispatchError::BadInputs(format!(
                "Fix: unpack_i4x8_via output byte count overflows usize for lane_count={lane_count}."
            ))
        })?;

    let QuantizedUnpackGpuScratch {
        inputs,
        program_cache,
    } = scratch;
    let program =
        program_cache.get_or_insert_with(lane_count, || unpack_i4x8("packed", "lanes", lane_count));
    ensure_input_slots(inputs, 2);
    write_u32_slice_le_bytes(&mut inputs[0], packed_words);
    write_zero_bytes(&mut inputs[1], out_bytes);
    let outputs = dispatcher.dispatch(
        program,
        &inputs[..2],
        Some([ceil_div_u32(lane_count, 256), 1, 1]),
    )?;
    if outputs.len() != 1 {
        return Err(DispatchError::BackendError(format!(
            "Fix: unpack_i4x8_via expected exactly one output buffer, got {}.",
            outputs.len()
        )));
    }
    decode_i32_output_exact(&outputs[0], lane_words, "unpack_i4x8_via", out)
}
