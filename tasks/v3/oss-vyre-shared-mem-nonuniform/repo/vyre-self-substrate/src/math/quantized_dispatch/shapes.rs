use crate::optimizer::dispatcher::DispatchError;
use vyre_primitives::math::quantized::i4_packed_words;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct PackedI4BatchedMatmulShape {
    pub(super) total_outputs_u32: u32,
    pub(super) output_words: usize,
    pub(super) output_bytes: usize,
    pub(super) top1_output_bytes: usize,
}

pub(super) fn validate_batched_packed_matmul_shape(
    context: &str,
    weights_packed: &[u32],
    activation_batches_packed: &[u32],
    row_scales: &[f32],
    batch_scales: &[f32],
    batch: u32,
    rows: u32,
    cols: u32,
) -> Result<PackedI4BatchedMatmulShape, DispatchError> {
    if batch == 0 || rows == 0 || cols == 0 {
        return Err(DispatchError::BadInputs(format!(
            "Fix: {context} requires batch > 0, rows > 0, and cols > 0, got batch={batch} rows={rows} cols={cols}."
        )));
    }
    let words_per_row = i4_packed_words(cols) as usize;
    let expected_weight_words = (rows as usize).checked_mul(words_per_row).ok_or_else(|| {
        DispatchError::BadInputs(format!(
            "Fix: {context} weight word count overflows usize for rows={rows} cols={cols}."
        ))
    })?;
    if weights_packed.len() != expected_weight_words {
        return Err(DispatchError::BadInputs(format!(
            "Fix: {context} requires weights_packed.len() == rows*i4_packed_words(cols), got len={} expected={expected_weight_words} for rows={rows} cols={cols}.",
            weights_packed.len()
        )));
    }
    let expected_activation_words =
        (batch as usize).checked_mul(words_per_row).ok_or_else(|| {
            DispatchError::BadInputs(format!(
                "Fix: {context} activation word count overflows usize for batch={batch} cols={cols}."
            ))
        })?;
    if activation_batches_packed.len() != expected_activation_words {
        return Err(DispatchError::BadInputs(format!(
            "Fix: {context} requires activation_batches_packed.len() == batch*i4_packed_words(cols), got len={} expected={expected_activation_words} for batch={batch} cols={cols}.",
            activation_batches_packed.len()
        )));
    }
    if row_scales.len() != rows as usize {
        return Err(DispatchError::BadInputs(format!(
            "Fix: {context} requires row_scales.len() == rows, got len={} rows={rows}.",
            row_scales.len()
        )));
    }
    if batch_scales.len() != batch as usize {
        return Err(DispatchError::BadInputs(format!(
            "Fix: {context} requires batch_scales.len() == batch, got len={} batch={batch}.",
            batch_scales.len()
        )));
    }
    let total_outputs_u32 = batch.checked_mul(rows).ok_or_else(|| {
        DispatchError::BadInputs(format!(
            "Fix: {context} output grid overflows u32 for batch={batch} rows={rows}."
        ))
    })?;
    let output_words = (batch as usize).checked_mul(rows as usize).ok_or_else(|| {
        DispatchError::BadInputs(format!(
            "Fix: {context} output word count overflows usize for batch={batch} rows={rows}."
        ))
    })?;
    let output_bytes = output_words
        .checked_mul(std::mem::size_of::<f32>())
        .ok_or_else(|| {
            DispatchError::BadInputs(format!(
                "Fix: {context} output byte count overflows usize for batch={batch} rows={rows}."
            ))
        })?;
    let top1_output_bytes = (batch as usize)
        .checked_mul(std::mem::size_of::<f32>())
        .ok_or_else(|| {
            DispatchError::BadInputs(format!(
                "Fix: {context} top-1 output byte count overflows usize for batch={batch}."
            ))
        })?;

    Ok(PackedI4BatchedMatmulShape {
        total_outputs_u32,
        output_words,
        output_bytes,
        top1_output_bytes,
    })
}

pub(super) fn expect_one_output<'a>(
    context: &str,
    outputs: &'a [Vec<u8>],
) -> Result<&'a [u8], DispatchError> {
    if outputs.len() != 1 {
        return Err(DispatchError::BackendError(format!(
            "Fix: {context} expected exactly one output buffer, got {}.",
            outputs.len()
        )));
    }
    Ok(&outputs[0])
}

pub(super) fn expect_two_outputs<'a>(
    context: &str,
    outputs: &'a [Vec<u8>],
) -> Result<(&'a [u8], &'a [u8]), DispatchError> {
    if outputs.len() != 2 {
        return Err(DispatchError::BackendError(format!(
            "Fix: {context} expected exactly two output buffers, got {}.",
            outputs.len()
        )));
    }
    Ok((&outputs[0], &outputs[1]))
}
