use super::inputs::{prepare_dispatch_inputs, DispatchInput};
use crate::dispatch_buffers::decode_u32_output_exact;
use crate::optimizer::dispatcher::{
    DispatchError, OptimizerDispatcher, ResidentDispatchStep, ResidentReadRange,
};
use vyre_foundation::ir::Program;

/// Allocate resident buffers as one logical group and free partial state on failure.
pub(crate) fn alloc_resident_buffers<D: OptimizerDispatcher + ?Sized, const N: usize>(
    dispatcher: &D,
    byte_lengths: [usize; N],
    context: &str,
) -> Result<[u64; N], DispatchError> {
    let handles = dispatcher.alloc_resident_many(&byte_lengths)?;
    handles.try_into().map_err(|handles: Vec<u64>| {
        DispatchError::BackendError(format!(
            "Fix: {context} grouped allocation returned {} handle(s), expected {N}.",
            handles.len()
        ))
    })
}

/// Allocate resident buffers, upload every payload, and free partial state on failure.
pub(crate) fn upload_resident_payloads<D: OptimizerDispatcher + ?Sized, const N: usize>(
    dispatcher: &D,
    payloads: [&[u8]; N],
) -> Result<[u64; N], DispatchError> {
    let mut byte_lengths = [0usize; N];
    for (index, payload) in payloads.iter().enumerate() {
        byte_lengths[index] = payload.len();
    }
    let handles = alloc_resident_buffers(dispatcher, byte_lengths, "resident payload upload")?;

    let empty: &[u8] = &[];
    let mut uploads = [(0u64, empty); N];
    for index in 0..N {
        uploads[index] = (handles[index], payloads[index]);
    }
    if let Err(error) = dispatcher.upload_resident_many(&uploads) {
        let upload_error = error.to_string();
        if let Err(free_error) =
            rollback_resident_handles(dispatcher, &handles, "resident payload upload rollback")
        {
            return Err(DispatchError::BackendError(format!(
                "Fix: resident payload upload failed after allocating {N} buffer(s): {upload_error}; rollback also failed: {free_error}."
            )));
        }
        return Err(error);
    }
    Ok(handles)
}

fn rollback_resident_handles<D: OptimizerDispatcher + ?Sized>(
    dispatcher: &D,
    handles: &[u64],
    context: &str,
) -> Result<(), DispatchError> {
    for (index, &handle) in handles.iter().enumerate() {
        if handle == 0 {
            continue;
        }
        dispatcher.free_resident(handle).map_err(|error| {
            DispatchError::BackendError(format!(
                "Fix: {context} failed to free resident handle {handle} at rollback index {index}: {error}."
            ))
        })?;
    }
    Ok(())
}

/// Prepare dispatch inputs in caller-owned staging and upload them as resident payloads.
pub(crate) fn upload_resident_dispatch_inputs<D: OptimizerDispatcher + ?Sized, const N: usize>(
    dispatcher: &D,
    staging: &mut Vec<Vec<u8>>,
    inputs: [DispatchInput<'_>; N],
) -> Result<[u64; N], DispatchError> {
    prepare_dispatch_inputs(staging, &inputs)?;
    let empty: &[u8] = &[];
    let mut payloads = [empty; N];
    for index in 0..N {
        payloads[index] = staging[index].as_slice();
    }
    upload_resident_payloads(dispatcher, payloads)
}

/// Run one resident dispatch step, read exactly two ranges, and decode u32 outputs.
#[allow(clippy::too_many_arguments)]
pub(crate) fn resident_dispatch_two_u32_outputs_into<D: OptimizerDispatcher + ?Sized>(
    dispatcher: &D,
    uploads: &[(u64, &[u8])],
    program: &Program,
    handle_ids: &[u64],
    grid_override: Option<[u32; 3]>,
    read_ranges: [ResidentReadRange; 2],
    readbacks: &mut Vec<Vec<u8>>,
    first_expected_words: usize,
    first_context: &str,
    first_out: &mut Vec<u32>,
    second_expected_words: usize,
    second_context: &str,
    second_out: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    let steps = [ResidentDispatchStep {
        program,
        handle_ids,
        grid_override,
    }];
    dispatcher.upload_resident_many_sequence_read_ranges_into(
        uploads,
        &steps,
        &read_ranges,
        readbacks,
    )?;
    if readbacks.len() != 2 {
        return Err(DispatchError::BackendError(format!(
            "Fix: {first_context} expected exactly two resident readbacks, got {}.",
            readbacks.len()
        )));
    }
    decode_u32_output_exact(
        &readbacks[0],
        first_expected_words,
        first_context,
        first_out,
    )?;
    decode_u32_output_exact(
        &readbacks[1],
        second_expected_words,
        second_context,
        second_out,
    )
}

/// Run a resident dispatch sequence, read exactly one range, and decode a u32 output.
pub(crate) fn resident_sequence_single_u32_output_into<D: OptimizerDispatcher + ?Sized>(
    dispatcher: &D,
    uploads: &[(u64, &[u8])],
    steps: &[ResidentDispatchStep<'_>],
    read_range: ResidentReadRange,
    readbacks: &mut Vec<Vec<u8>>,
    expected_words: usize,
    context: &str,
    out: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    dispatcher.upload_resident_many_sequence_read_ranges_into(
        uploads,
        steps,
        &[read_range],
        readbacks,
    )?;
    if readbacks.len() != 1 {
        return Err(DispatchError::BackendError(format!(
            "Fix: {context} expected exactly one resident readback, got {}.",
            readbacks.len()
        )));
    }
    decode_u32_output_exact(&readbacks[0], expected_words, context, out)
}
