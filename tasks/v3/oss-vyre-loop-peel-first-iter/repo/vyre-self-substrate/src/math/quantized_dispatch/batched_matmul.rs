use super::*;

/// Compute packed signed INT4 batched matrix multiply through the backend.
///
/// `weights_packed` is row-major `[rows][i4_packed_words(cols)]`.
/// `activation_batches_packed` is batch-major `[batch][i4_packed_words(cols)]`.
/// `row_scales` has `rows` f32 values and `batch_scales` has `batch` f32
/// values. The returned vector has `batch * rows` f32 values in batch-major
/// order.
///
/// # Errors
///
/// Returns [`DispatchError`] when dimensions are zero, input shapes are wrong,
/// dispatch fails, or backend readback is malformed.
pub fn i4x8_batched_matmul_f32_scaled_via(
    dispatcher: &impl OptimizerDispatcher,
    weights_packed: &[u32],
    activation_batches_packed: &[u32],
    row_scales: &[f32],
    batch_scales: &[f32],
    batch: u32,
    rows: u32,
    cols: u32,
) -> Result<Vec<f32>, DispatchError> {
    let mut scratch = QuantizedBatchedMatmulGpuScratch::default();
    let mut out = Vec::new();
    i4x8_batched_matmul_f32_scaled_via_with_scratch_into(
        dispatcher,
        weights_packed,
        activation_batches_packed,
        row_scales,
        batch_scales,
        batch,
        rows,
        cols,
        &mut scratch,
        &mut out,
    )?;
    Ok(out)
}

/// Compute packed signed INT4 batched matrix multiply through caller-owned scratch.
///
/// On success, `out` contains exactly `batch * rows` f32 values.
///
/// # Errors
///
/// Returns [`DispatchError`] under the same conditions as
/// [`i4x8_batched_matmul_f32_scaled_via`].
pub fn i4x8_batched_matmul_f32_scaled_via_with_scratch_into(
    dispatcher: &impl OptimizerDispatcher,
    weights_packed: &[u32],
    activation_batches_packed: &[u32],
    row_scales: &[f32],
    batch_scales: &[f32],
    batch: u32,
    rows: u32,
    cols: u32,
    scratch: &mut QuantizedBatchedMatmulGpuScratch,
    out: &mut Vec<f32>,
) -> Result<(), DispatchError> {
    let shape = validate_batched_packed_matmul_shape(
        "i4x8_batched_matmul_f32_scaled_via",
        weights_packed,
        activation_batches_packed,
        row_scales,
        batch_scales,
        batch,
        rows,
        cols,
    )?;

    let QuantizedBatchedMatmulGpuScratch {
        inputs,
        program_cache,
    } = scratch;
    let program = program_cache.get_or_insert_with((batch, rows, cols), || {
        i4x8_batched_matmul_f32_scaled(
            "weights",
            "activations",
            "row_scales",
            "batch_scales",
            "out",
            batch,
            rows,
            cols,
        )
    });
    ensure_input_slots(inputs, 5);
    write_u32_slice_le_bytes(&mut inputs[0], weights_packed);
    write_u32_slice_le_bytes(&mut inputs[1], activation_batches_packed);
    write_f32_slice_le_bytes(&mut inputs[2], row_scales);
    write_f32_slice_le_bytes(&mut inputs[3], batch_scales);
    write_zero_bytes(&mut inputs[4], shape.output_bytes);

    let outputs = dispatcher.dispatch(
        program,
        &inputs[..5],
        Some([ceil_div_u32(shape.total_outputs_u32, 64), 1, 1]),
    )?;
    decode_f32_output_exact(
        expect_one_output("i4x8_batched_matmul_f32_scaled_via", &outputs)?,
        shape.output_words,
        "i4x8_batched_matmul_f32_scaled_via",
        out,
    )
}
