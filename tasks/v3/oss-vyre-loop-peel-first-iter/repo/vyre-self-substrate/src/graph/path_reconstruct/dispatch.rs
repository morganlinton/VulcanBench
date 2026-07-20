use super::{CachedBatchedPathProgram, CachedSinglePathProgram, PathReconstructGpuScratch};
use vyre_primitives::graph::path_reconstruct::{
    plan_batched_path_reconstruct_dispatch, plan_path_reconstruct_dispatch,
    validate_batched_path_reconstruct_readback, validate_path_reconstruct_readback,
    BATCHED_LENS_BUFFER, BATCHED_PATHS_BUFFER, PATH_LEN_BUFFER, PATH_OUT_BUFFER,
};

use crate::graph::dispatch_bridge::{
    dispatch_two_u32_outputs_from_prepared_into, refresh_keyed_dispatch_inputs, DispatchInput,
};
use crate::optimizer::dispatcher::{DispatchError, OptimizerDispatcher};

/// GPU dispatch wrapper around [`reconstruct_path`]. Returns the
/// number of valid entries written to `scratch` (zero-padded to
/// `max_depth`).
///
/// # Errors
///
/// Propagates dispatch failures and rejects malformed readback.
pub fn reconstruct_path_via(
    dispatcher: &dyn OptimizerDispatcher,
    parent: &[u32],
    target: u32,
    max_depth: u32,
    scratch: &mut Vec<u32>,
) -> Result<u32, DispatchError> {
    let mut dispatch_scratch = PathReconstructGpuScratch::default();
    reconstruct_path_via_with_scratch(
        dispatcher,
        parent,
        target,
        max_depth,
        &mut dispatch_scratch,
        scratch,
    )
}

/// GPU dispatch wrapper around the path-reconstruction primitive with caller-owned dispatch scratch.
pub fn reconstruct_path_via_with_scratch(
    dispatcher: &dyn OptimizerDispatcher,
    parent: &[u32],
    target: u32,
    max_depth: u32,
    dispatch_scratch: &mut PathReconstructGpuScratch,
    scratch: &mut Vec<u32>,
) -> Result<u32, DispatchError> {
    let plan = plan_path_reconstruct_dispatch(parent.len(), max_depth)
        .map_err(DispatchError::BadInputs)?;
    let PathReconstructGpuScratch {
        inputs,
        len_out,
        static_input_key,
        single_program_cache,
        ..
    } = dispatch_scratch;
    let target_buf = [target];
    let static_key = plan
        .static_input_key(parent)
        .map_err(DispatchError::BadInputs)?;
    let cached =
        single_program_cache.get_or_insert_with(plan.max_depth, || CachedSinglePathProgram {
            program: plan.program(),
        });
    refresh_keyed_dispatch_inputs(
        inputs,
        static_input_key,
        static_key,
        &[
            DispatchInput::u32_slice(parent),
            DispatchInput::u32_slice(&target_buf),
            DispatchInput::ZeroU32Words {
                words: plan.path_words,
                context: PATH_OUT_BUFFER,
            },
            DispatchInput::ZeroU32Words {
                words: plan.len_words,
                context: PATH_LEN_BUFFER,
            },
        ],
        &[
            (1, DispatchInput::u32_slice(&target_buf)),
            (
                2,
                DispatchInput::ZeroU32Words {
                    words: plan.path_words,
                    context: PATH_OUT_BUFFER,
                },
            ),
            (
                3,
                DispatchInput::ZeroU32Words {
                    words: plan.len_words,
                    context: PATH_LEN_BUFFER,
                },
            ),
        ],
    )?;
    dispatch_two_u32_outputs_from_prepared_into(
        dispatcher,
        &cached.program,
        inputs,
        plan.path_words,
        PATH_OUT_BUFFER,
        scratch,
        plan.len_words,
        PATH_LEN_BUFFER,
        len_out,
        Some(plan.grid),
    )?;
    let len = len_out[0];
    validate_path_reconstruct_readback(&plan, len).map_err(DispatchError::BackendError)?;
    Ok(len)
}

/// Convenience wrapper for dispatcher-backed single-target reconstruction.
///
/// Returns the reconstructed path truncated to the actual length. Callers that
/// reconstruct many targets should use [`reconstruct_paths_via`] to avoid
/// launch-per-target amplification.
///
/// # Errors
///
/// Returns [`DispatchError`] when validation or backend execution fails.
pub fn path_to_root_via(
    dispatcher: &dyn OptimizerDispatcher,
    parent: &[u32],
    target: u32,
    max_depth: u32,
) -> Result<Vec<u32>, DispatchError> {
    let mut scratch = Vec::new();
    let len = reconstruct_path_via(dispatcher, parent, target, max_depth, &mut scratch)?;
    scratch.truncate(len as usize);
    Ok(scratch)
}

/// GPU dispatch wrapper around batched parent-walk: reconstructs the
/// path-to-root for every entry in `targets` simultaneously. Returns
/// `(paths, lens)` where `paths` is the concatenation of each
/// target's `max_depth`-padded scratch buffer and `lens[i]` is the
/// valid length for `targets[i]`.
///
/// # Errors
///
/// Propagates path-reconstruction dispatch failures.
pub fn reconstruct_paths_via(
    dispatcher: &dyn OptimizerDispatcher,
    parent: &[u32],
    targets: &[u32],
    max_depth: u32,
) -> Result<(Vec<u32>, Vec<u32>), DispatchError> {
    let mut scratch = PathReconstructGpuScratch::default();
    let mut paths = Vec::new();
    let mut lens = Vec::new();
    reconstruct_paths_via_with_scratch_into(
        dispatcher,
        parent,
        targets,
        max_depth,
        &mut scratch,
        &mut paths,
        &mut lens,
    )?;
    Ok((paths, lens))
}

/// GPU dispatch wrapper around batched parent-walk into caller-owned output storage.
pub fn reconstruct_paths_via_with_scratch_into(
    dispatcher: &dyn OptimizerDispatcher,
    parent: &[u32],
    targets: &[u32],
    max_depth: u32,
    scratch: &mut PathReconstructGpuScratch,
    paths: &mut Vec<u32>,
    lens: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    let plan = plan_batched_path_reconstruct_dispatch(parent.len(), targets.len(), max_depth)
        .map_err(DispatchError::BadInputs)?;
    if plan.layout.target_count == 0 {
        paths.clear();
        lens.clear();
        return Ok(());
    }
    let PathReconstructGpuScratch {
        inputs,
        static_input_key,
        batched_program_cache,
        ..
    } = scratch;
    let static_key = plan
        .static_input_key(parent)
        .map_err(DispatchError::BadInputs)?;
    let cached = batched_program_cache.get_or_insert_with(
        (plan.layout.target_count, plan.max_depth),
        || CachedBatchedPathProgram {
            program: plan.program(),
        },
    );
    refresh_keyed_dispatch_inputs(
        inputs,
        static_input_key,
        static_key,
        &[
            DispatchInput::u32_slice(parent),
            DispatchInput::u32_slice(targets),
            DispatchInput::ZeroU32Words {
                words: plan.path_words,
                context: BATCHED_PATHS_BUFFER,
            },
            DispatchInput::ZeroU32Words {
                words: plan.len_words,
                context: BATCHED_LENS_BUFFER,
            },
        ],
        &[
            (1, DispatchInput::u32_slice(targets)),
            (
                2,
                DispatchInput::ZeroU32Words {
                    words: plan.path_words,
                    context: BATCHED_PATHS_BUFFER,
                },
            ),
            (
                3,
                DispatchInput::ZeroU32Words {
                    words: plan.len_words,
                    context: BATCHED_LENS_BUFFER,
                },
            ),
        ],
    )?;
    dispatch_two_u32_outputs_from_prepared_into(
        dispatcher,
        &cached.program,
        inputs,
        plan.path_words,
        BATCHED_PATHS_BUFFER,
        paths,
        plan.len_words,
        BATCHED_LENS_BUFFER,
        lens,
        Some(plan.grid),
    )?;
    validate_batched_path_reconstruct_readback(&plan, paths.len(), lens.len(), lens)
        .map_err(DispatchError::BackendError)?;
    Ok(())
}
