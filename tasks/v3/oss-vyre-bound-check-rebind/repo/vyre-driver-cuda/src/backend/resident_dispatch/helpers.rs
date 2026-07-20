use std::ffi::c_void;
use std::sync::Arc;

use rustc_hash::{FxHashMap, FxHashSet};
use smallvec::SmallVec;
use vyre_driver::{BackendError, DispatchConfig};
use vyre_foundation::ir::Program;

use crate::backend::allocations::HostTransferAllocations;
use crate::backend::ordering::{classify_dense_permutation, DensePermutationDefect};
use crate::backend::plan::CudaDispatchPlan;
use crate::backend::resident::CudaResidentBuffer;
use crate::backend::resident_upload_fusion::ResidentUploadCopy;
use crate::backend::staging_reserve::{
    reserve_hash_set, reserve_smallvec, reserve_vec, resize_vec_slots,
};

pub(crate) fn resident_required_handles(
    prepared: &CudaDispatchPlan,
) -> Result<usize, BackendError> {
    prepared
        .bindings
        .bindings
        .len()
        .checked_sub(prepared.bindings.shared_indices.len())
        .ok_or_else(|| BackendError::InvalidProgram {
            fix: format!(
                "Fix: CUDA resident binding plan has {} binding(s) but {} shared binding index(es). Rebuild the dispatch plan before launching.",
                prepared.bindings.bindings.len(),
                prepared.bindings.shared_indices.len()
            ),
        })
}

pub(crate) fn next_resident_handle(
    handles: &[CudaResidentBuffer],
    next_handle: &mut usize,
    context: &'static str,
) -> Result<CudaResidentBuffer, BackendError> {
    let handle_index = *next_handle;
    let Some(handle) = handles.get(handle_index).copied() else {
        return Err(BackendError::InvalidProgram {
            fix: format!(
                "Fix: CUDA {context} ran out of resident buffer handles at descriptor slot {handle_index} after receiving {} handle(s). Validate resident handle count against the binding plan before launch.",
                handles.len()
            ),
        });
    };
    *next_handle = next_handle
        .checked_add(1)
        .ok_or_else(|| BackendError::InvalidProgram {
            fix: format!(
                "Fix: CUDA {context} resident handle cursor overflowed at descriptor slot {handle_index}. Rebuild the resident binding plan before launch.",
            ),
        })?;
    Ok(handle)
}

fn validate_dense_resident_indices<I>(
    indices: I,
    expected_len: usize,
    context: &'static str,
    index_kind: &'static str,
    rebuild_action: &'static str,
) -> Result<(), BackendError>
where
    I: IntoIterator<Item = usize>,
{
    // Callers sort before validating (resident_dispatch::{borrowed,batch,
    // async_dispatch} all `sort_unstable_by_key_if_needed` first); the shared
    // classifier is defined on sorted slot order. Collect into the fallibly
    // reserved staging buffer, then delegate the dense-permutation invariant to
    // the single backend-neutral owner and format the resident-specific message
    // from the classified defect — one algorithm, no per-subsystem fork.
    let iter = indices.into_iter();
    let mut sorted = SmallVec::<[usize; 8]>::new();
    reserve_smallvec(
        &mut sorted,
        iter.size_hint().0,
        "CUDA resident dense index validation",
    )?;
    sorted.extend(iter);
    match classify_dense_permutation(&sorted, expected_len) {
        Ok(()) => Ok(()),
        Err(DensePermutationDefect::Duplicate { index, slot }) => {
            Err(BackendError::InvalidProgram {
                fix: format!(
                    "Fix: CUDA {context} found a duplicate {index_kind} index {index} at sorted {index_kind} slot {slot}; duplicate {index_kind} indexes alias one logical slot onto two descriptors. Rebuild the binding plan with dense unique {index_kind} indexes 0..{expected_len} before {rebuild_action}.",
                ),
            })
        }
        Err(DensePermutationDefect::Sparse { index, slot }) => {
            Err(BackendError::InvalidProgram {
                fix: format!(
                    "Fix: CUDA {context} resolved sparse {index_kind} index {index} at sorted {index_kind} slot {slot}; expected dense {index_kind} indexes 0..{expected_len}. Rebuild the binding plan before {rebuild_action}.",
                ),
            })
        }
        Err(DensePermutationDefect::LengthMismatch { resolved, expected }) => {
            Err(BackendError::InvalidProgram {
                fix: format!(
                    "Fix: CUDA {context} resolved {resolved} {index_kind} index(es); expected {expected}. Rebuild the binding plan before {rebuild_action}.",
                ),
            })
        }
    }
}

pub(crate) fn validate_dense_resident_output_indices<I>(
    output_indices: I,
    expected_len: usize,
    context: &'static str,
) -> Result<(), BackendError>
where
    I: IntoIterator<Item = usize>,
{
    validate_dense_resident_indices(
        output_indices,
        expected_len,
        context,
        "output",
        "resident readback",
    )
}

pub(crate) fn validate_dense_resident_input_indices<I>(
    input_indices: I,
    expected_len: usize,
    context: &'static str,
) -> Result<(), BackendError>
where
    I: IntoIterator<Item = usize>,
{
    validate_dense_resident_indices(
        input_indices,
        expected_len,
        context,
        "input",
        "borrowed fallback launch",
    )
}

pub(crate) fn stage_resident_fill_payload(
    payload: &mut Vec<u8>,
    value: u8,
    byte_len: usize,
) -> Result<&[u8], BackendError> {
    reserve_vec(payload, byte_len, "resident fallback fill byte")?;
    payload.clear();
    payload.resize(byte_len, value);
    Ok(payload.as_slice())
}

pub(crate) fn enqueue_resident_h2d_copy(
    dst_ptr: u64,
    host_ptr: *const c_void,
    byte_len: usize,
    stream_raw: cudarc::driver::sys::CUstream,
) -> Result<(), BackendError> {
    // SAFETY: The caller owns the stream ordering and guarantees that the
    // pinned host allocation and resident destination remain live until the
    // stream reaches this copy. The shared copy helper validates null pointers
    // for non-empty copies and treats zero-byte copies as no-ops.
    unsafe { crate::backend::copy::h2d_async_checked(dst_ptr, host_ptr, byte_len, stream_raw) }
}

pub(crate) fn enqueue_optional_resident_h2d_copy(
    upload: Option<(u64, *const c_void, usize)>,
    stream_raw: cudarc::driver::sys::CUstream,
) -> Result<(), BackendError> {
    if let Some((dst_ptr, host_ptr, byte_len)) = upload {
        enqueue_resident_h2d_copy(dst_ptr, host_ptr, byte_len, stream_raw)?;
    }
    Ok(())
}

pub(crate) fn enqueue_resident_upload_copies_on_stream(
    copies: &[ResidentUploadCopy<'_>],
    host_transfers: &mut HostTransferAllocations,
    stream_raw: cudarc::driver::sys::CUstream,
) -> Result<(), BackendError> {
    for copy in copies {
        let bytes = copy.bytes.as_slice();
        let host_ptr = host_transfers.push_upload(bytes)?;
        enqueue_resident_h2d_copy(copy.dst_ptr, host_ptr, bytes.len(), stream_raw)?;
    }
    Ok(())
}

pub(crate) fn borrow_resident_sequence_output_slots(
    outputs: &mut Vec<Vec<u8>>,
    slot_count: usize,
) -> Result<SmallVec<[&mut Vec<u8>; 8]>, BackendError> {
    resize_vec_slots(outputs, slot_count, "resident sequence output slots")?;
    let mut borrowed_outputs = SmallVec::<[&mut Vec<u8>; 8]>::new();
    reserve_smallvec(
        &mut borrowed_outputs,
        outputs.len(),
        "resident sequence borrowed output slots",
    )?;
    borrowed_outputs.extend(outputs.iter_mut());
    Ok(borrowed_outputs)
}

pub(crate) fn prepare_resident_sequence_fills(
    fills: &[(CudaResidentBuffer, u8)],
    uploads: &[(CudaResidentBuffer, &[u8])],
) -> Result<SmallVec<[(CudaResidentBuffer, u8); 8]>, BackendError> {
    let mut uploaded_handles = FxHashSet::<CudaResidentBuffer>::default();
    if !uploads.is_empty() {
        reserve_hash_set(
            &mut uploaded_handles,
            uploads.len(),
            "resident sequence upload handle set",
        )?;
        uploaded_handles.extend(uploads.iter().map(|&(handle, _)| handle));
    }

    let mut effective = SmallVec::<[(CudaResidentBuffer, u8); 8]>::new();
    reserve_smallvec(
        &mut effective,
        fills.len(),
        "resident sequence effective fills",
    )?;

    let mut effective_indices = FxHashMap::<CudaResidentBuffer, usize>::default();
    effective_indices
        .try_reserve(fills.len())
        .map_err(|error| BackendError::InvalidProgram {
            fix: format!(
                "Fix: CUDA resident sequence fill index could not reserve {} handle slot(s): {error}.",
                fills.len()
            ),
        })?;

    for &(handle, value) in fills {
        if !uploaded_handles.is_empty() && uploaded_handles.contains(&handle) {
            continue;
        }
        if let Some(&index) = effective_indices.get(&handle) {
            let Some(existing) = effective.get_mut(index) else {
                return Err(BackendError::InvalidProgram {
                    fix: format!(
                        "Fix: CUDA resident sequence fill index for handle {} pointed at stale effective fill slot {index} after {} slot(s) were prepared. Rebuild duplicate-fill coalescing before launching the resident sequence.",
                        handle.id,
                        effective.len()
                    ),
                });
            };
            existing.1 = value;
            continue;
        }
        effective_indices.insert(handle, effective.len());
        effective.push((handle, value));
    }

    Ok(effective)
}

pub(crate) struct PreparedStep<'a> {
    pub(crate) program: &'a Program,
    pub(crate) handles: SmallVec<[CudaResidentBuffer; 8]>,
    pub(crate) config: &'a DispatchConfig,
    pub(crate) ptx_src: Arc<str>,
    pub(crate) module_key: crate::backend::module_cache::ModuleCacheKey,
    pub(crate) prepared: CudaDispatchPlan,
}
