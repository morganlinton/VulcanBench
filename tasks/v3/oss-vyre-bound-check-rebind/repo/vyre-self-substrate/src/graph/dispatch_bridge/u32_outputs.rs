use crate::dispatch_buffers::decode_u32_output_exact;
use crate::optimizer::dispatcher::{DispatchError, OptimizerDispatcher};
use vyre_foundation::ir::Program;

/// Dispatch already-prepared inputs and decode exactly one u32 output buffer
/// into `out`.
pub(crate) fn dispatch_single_u32_output_from_prepared_into<D: OptimizerDispatcher + ?Sized>(
    dispatcher: &D,
    program: &Program,
    scratch_inputs: &[Vec<u8>],
    expected_output_words: usize,
    context: &str,
    grid_override: Option<[u32; 3]>,
    out: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    let outputs = dispatcher.dispatch(program, scratch_inputs, grid_override)?;
    if outputs.len() != 1 {
        return Err(DispatchError::BackendError(format!(
            "Fix: {context} expected exactly one u32 output buffer, got {}.",
            outputs.len()
        )));
    }
    decode_u32_output_exact(&outputs[0], expected_output_words, context, out)
}

/// Dispatch already-prepared inputs and decode exactly two u32 output buffers.
#[allow(clippy::too_many_arguments)]
pub(crate) fn dispatch_two_u32_outputs_from_prepared_into<D: OptimizerDispatcher + ?Sized>(
    dispatcher: &D,
    program: &Program,
    scratch_inputs: &[Vec<u8>],
    first_expected_words: usize,
    first_context: &str,
    first_out: &mut Vec<u32>,
    second_expected_words: usize,
    second_context: &str,
    second_out: &mut Vec<u32>,
    grid_override: Option<[u32; 3]>,
) -> Result<(), DispatchError> {
    let outputs = dispatcher.dispatch(program, scratch_inputs, grid_override)?;
    if outputs.len() != 2 {
        return Err(DispatchError::BackendError(format!(
            "Fix: {first_context} expected exactly two u32 output buffers, got {}.",
            outputs.len()
        )));
    }
    decode_u32_output_exact(&outputs[0], first_expected_words, first_context, first_out)?;
    decode_u32_output_exact(
        &outputs[1],
        second_expected_words,
        second_context,
        second_out,
    )
}
