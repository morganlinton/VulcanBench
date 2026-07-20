use super::*;

/// Compute a packed signed INT4 row-scaled matrix-vector product through the backend.
///
/// `weights_packed` is row-major with `i4_packed_words(cols)` u32 words per
/// row. `x` has `cols` f32 values and `row_scales` has `rows` f32 values. The
/// returned vector has exactly `rows` f32 values.
///
/// # Errors
///
/// Returns [`DispatchError`] when dimensions are zero, input shapes are wrong,
/// dispatch fails, or backend readback is malformed.
pub fn i4x8_matvec_f32_scaled_via(
    dispatcher: &impl OptimizerDispatcher,
    weights_packed: &[u32],
    x: &[f32],
    row_scales: &[f32],
    rows: u32,
    cols: u32,
) -> Result<Vec<f32>, DispatchError> {
    let mut scratch = QuantizedMatvecGpuScratch::default();
    let mut out = Vec::new();
    i4x8_matvec_f32_scaled_via_with_scratch_into(
        dispatcher,
        weights_packed,
        x,
        row_scales,
        rows,
        cols,
        &mut scratch,
        &mut out,
    )?;
    Ok(out)
}

/// Compute a packed signed INT4 row-scaled matrix-vector product through caller-owned scratch.
///
/// On success, `out` contains exactly `rows` f32 values.
///
/// # Errors
///
/// Returns [`DispatchError`] under the same conditions as
/// [`i4x8_matvec_f32_scaled_via`].
pub fn i4x8_matvec_f32_scaled_via_with_scratch_into(
    dispatcher: &impl OptimizerDispatcher,
    weights_packed: &[u32],
    x: &[f32],
    row_scales: &[f32],
    rows: u32,
    cols: u32,
    scratch: &mut QuantizedMatvecGpuScratch,
    out: &mut Vec<f32>,
) -> Result<(), DispatchError> {
    if rows == 0 || cols == 0 {
        return Err(DispatchError::BadInputs(format!(
            "Fix: i4x8_matvec_f32_scaled_via requires rows > 0 and cols > 0, got rows={rows} cols={cols}."
        )));
    }
    let words_per_row = i4_packed_words(cols) as usize;
    let expected_weight_words = (rows as usize).checked_mul(words_per_row).ok_or_else(|| {
        DispatchError::BadInputs(format!(
            "Fix: i4x8_matvec_f32_scaled_via weight word count overflows usize for rows={rows} cols={cols}."
        ))
    })?;
    if weights_packed.len() != expected_weight_words {
        return Err(DispatchError::BadInputs(format!(
            "Fix: i4x8_matvec_f32_scaled_via requires weights_packed.len() == rows*i4_packed_words(cols), got len={} expected={expected_weight_words} for rows={rows} cols={cols}.",
            weights_packed.len()
        )));
    }
    if x.len() != cols as usize {
        return Err(DispatchError::BadInputs(format!(
            "Fix: i4x8_matvec_f32_scaled_via requires x.len() == cols, got len={} cols={cols}.",
            x.len()
        )));
    }
    if row_scales.len() != rows as usize {
        return Err(DispatchError::BadInputs(format!(
            "Fix: i4x8_matvec_f32_scaled_via requires row_scales.len() == rows, got len={} rows={rows}.",
            row_scales.len()
        )));
    }
    let out_bytes = (rows as usize)
        .checked_mul(std::mem::size_of::<f32>())
        .ok_or_else(|| {
            DispatchError::BadInputs(format!(
                "Fix: i4x8_matvec_f32_scaled_via output byte count overflows usize for rows={rows}."
            ))
        })?;

    let QuantizedMatvecGpuScratch {
        inputs,
        program_cache,
    } = scratch;
    let program = program_cache.get_or_insert_with((rows, cols), || {
        i4x8_matvec_f32_scaled("weights", "x", "row_scales", "out", rows, cols)
    });
    ensure_input_slots(inputs, 4);
    write_u32_slice_le_bytes(&mut inputs[0], weights_packed);
    write_f32_slice_le_bytes(&mut inputs[1], x);
    write_f32_slice_le_bytes(&mut inputs[2], row_scales);
    write_zero_bytes(&mut inputs[3], out_bytes);

    let outputs = dispatcher.dispatch(program, &inputs[..4], Some([rows, 1, 1]))?;
    if outputs.len() != 1 {
        return Err(DispatchError::BackendError(format!(
            "Fix: i4x8_matvec_f32_scaled_via expected exactly one output buffer, got {}.",
            outputs.len()
        )));
    }
    decode_f32_output_exact(
        &outputs[0],
        rows as usize,
        "i4x8_matvec_f32_scaled_via",
        out,
    )
}
