use super::*;

/// Compute top-1 scores and row indices for packed signed INT4 batched matmul through the backend.
///
/// `weights_packed` is row-major `[rows][i4_packed_words(cols)]`.
/// `activation_batches_packed` is batch-major `[batch][i4_packed_words(cols)]`.
/// `row_scales` has `rows` f32 values and `batch_scales` has `batch` f32
/// values. The returned scores and indices each have exactly `batch` values.
///
/// # Errors
///
/// Returns [`DispatchError`] when dimensions are zero, input shapes are wrong,
/// dispatch fails, or backend readback is malformed.
pub fn i4x8_batched_matmul_top1_f32_scaled_via(
    dispatcher: &impl OptimizerDispatcher,
    weights_packed: &[u32],
    activation_batches_packed: &[u32],
    row_scales: &[f32],
    batch_scales: &[f32],
    batch: u32,
    rows: u32,
    cols: u32,
) -> Result<(Vec<f32>, Vec<u32>), DispatchError> {
    let mut scratch = QuantizedBatchedMatmulTop1GpuScratch::default();
    let mut scores = Vec::new();
    let mut indices = Vec::new();
    i4x8_batched_matmul_top1_f32_scaled_via_with_scratch_into(
        dispatcher,
        weights_packed,
        activation_batches_packed,
        row_scales,
        batch_scales,
        batch,
        rows,
        cols,
        &mut scratch,
        &mut scores,
        &mut indices,
    )?;
    Ok((scores, indices))
}

/// Compute top-1 scores and row indices for packed signed INT4 batched matmul through caller-owned scratch.
///
/// On success, `scores_out` and `indices_out` each contain exactly `batch`
/// values.
///
/// # Errors
///
/// Returns [`DispatchError`] under the same conditions as
/// [`i4x8_batched_matmul_top1_f32_scaled_via`].
pub fn i4x8_batched_matmul_top1_f32_scaled_via_with_scratch_into(
    dispatcher: &impl OptimizerDispatcher,
    weights_packed: &[u32],
    activation_batches_packed: &[u32],
    row_scales: &[f32],
    batch_scales: &[f32],
    batch: u32,
    rows: u32,
    cols: u32,
    scratch: &mut QuantizedBatchedMatmulTop1GpuScratch,
    scores_out: &mut Vec<f32>,
    indices_out: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    let shape = validate_batched_packed_matmul_shape(
        "i4x8_batched_matmul_top1_f32_scaled_via",
        weights_packed,
        activation_batches_packed,
        row_scales,
        batch_scales,
        batch,
        rows,
        cols,
    )?;

    let QuantizedBatchedMatmulTop1GpuScratch {
        inputs,
        program_cache,
    } = scratch;
    let program = program_cache.get_or_insert_with((batch, rows, cols), || {
        i4x8_batched_matmul_top1_f32_scaled(
            "weights",
            "activations",
            "row_scales",
            "batch_scales",
            "scores",
            batch,
            rows,
            cols,
        )
    });
    ensure_input_slots(inputs, 6);
    write_u32_slice_le_bytes(&mut inputs[0], weights_packed);
    write_u32_slice_le_bytes(&mut inputs[1], activation_batches_packed);
    write_f32_slice_le_bytes(&mut inputs[2], row_scales);
    write_f32_slice_le_bytes(&mut inputs[3], batch_scales);
    write_zero_bytes(&mut inputs[4], shape.top1_output_bytes);
    write_zero_bytes(&mut inputs[5], shape.top1_output_bytes);

    let outputs =
        dispatcher.dispatch(program, &inputs[..6], Some([ceil_div_u32(batch, 64), 1, 1]))?;
    let (scores_bytes, indices_bytes) =
        expect_two_outputs("i4x8_batched_matmul_top1_f32_scaled_via", &outputs)?;
    decode_f32_output_exact(
        scores_bytes,
        batch as usize,
        "i4x8_batched_matmul_top1_f32_scaled_via scores",
        scores_out,
    )?;
    decode_u32_output_exact(
        indices_bytes,
        batch as usize,
        "i4x8_batched_matmul_top1_f32_scaled_via indices",
        indices_out,
    )
}
