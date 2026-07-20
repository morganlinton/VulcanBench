use super::*;

/// Compute a packed signed INT4 scaled dot product through the dispatch backend.
///
/// `lhs_packed` and `rhs_packed` store eight signed 4-bit lanes per u32 word.
/// The returned scalar is bit-exact with the primitive CPU oracle for the same
/// lane order and scale factors.
///
/// # Errors
///
/// Returns [`DispatchError`] when lane count is zero, packed input shape is
/// wrong, dispatch fails, or backend readback is malformed.
pub fn i4x8_dot_f32_scaled_via(
    dispatcher: &impl OptimizerDispatcher,
    lhs_packed: &[u32],
    rhs_packed: &[u32],
    lhs_scale: f32,
    rhs_scale: f32,
    lane_count: u32,
) -> Result<f32, DispatchError> {
    let mut scratch = QuantizedDotGpuScratch::default();
    let mut out = Vec::new();
    i4x8_dot_f32_scaled_via_with_scratch_into(
        dispatcher,
        lhs_packed,
        rhs_packed,
        lhs_scale,
        rhs_scale,
        lane_count,
        &mut scratch,
        &mut out,
    )?;
    Ok(out[0])
}

/// Compute a packed signed INT4 scaled dot product through caller-owned scratch.
///
/// On success, `out` contains exactly one f32 scalar.
///
/// # Errors
///
/// Returns [`DispatchError`] under the same conditions as
/// [`i4x8_dot_f32_scaled_via`].
pub fn i4x8_dot_f32_scaled_via_with_scratch_into(
    dispatcher: &impl OptimizerDispatcher,
    lhs_packed: &[u32],
    rhs_packed: &[u32],
    lhs_scale: f32,
    rhs_scale: f32,
    lane_count: u32,
    scratch: &mut QuantizedDotGpuScratch,
    out: &mut Vec<f32>,
) -> Result<(), DispatchError> {
    if lane_count == 0 {
        return Err(DispatchError::BadInputs(
            "Fix: i4x8_dot_f32_scaled_via requires lane_count > 0.".to_string(),
        ));
    }
    let expected_words = i4_packed_words(lane_count) as usize;
    if lhs_packed.len() != expected_words || rhs_packed.len() != expected_words {
        return Err(DispatchError::BadInputs(format!(
            "Fix: i4x8_dot_f32_scaled_via requires lhs/rhs packed lengths == i4_packed_words(lane_count), got lhs={} rhs={} expected={expected_words} for lane_count={lane_count}.",
            lhs_packed.len(),
            rhs_packed.len()
        )));
    }

    let QuantizedDotGpuScratch {
        inputs,
        program_cache,
    } = scratch;
    let program = program_cache.get_or_insert_with(lane_count, || {
        i4x8_dot_f32_scaled("lhs", "rhs", "lhs_scale", "rhs_scale", "out", lane_count)
    });
    ensure_input_slots(inputs, 5);
    write_u32_slice_le_bytes(&mut inputs[0], lhs_packed);
    write_u32_slice_le_bytes(&mut inputs[1], rhs_packed);
    write_f32_slice_le_bytes(&mut inputs[2], &[lhs_scale]);
    write_f32_slice_le_bytes(&mut inputs[3], &[rhs_scale]);
    write_zero_bytes(&mut inputs[4], std::mem::size_of::<f32>());

    let outputs = dispatcher.dispatch(program, &inputs[..5], Some([1, 1, 1]))?;
    if outputs.len() != 1 {
        return Err(DispatchError::BackendError(format!(
            "Fix: i4x8_dot_f32_scaled_via expected exactly one output buffer, got {}.",
            outputs.len()
        )));
    }
    decode_f32_output_exact(&outputs[0], 1, "i4x8_dot_f32_scaled_via", out)
}
