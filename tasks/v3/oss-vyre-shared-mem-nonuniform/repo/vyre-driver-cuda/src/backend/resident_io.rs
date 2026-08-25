//! Host and device copies for CUDA-resident buffers.

use vyre_driver::transfer_accounting::TransferAccountingPolicy;
use vyre_driver::{BackendError, OutputBuffers};

use super::allocations::HostTransferAllocations;
use super::capabilities::cuda_live_free_memory_bytes;
use super::dispatch::CudaBackend;
use super::output_range::CudaOutputReadback;
use super::resident::{CudaResidentBuffer, ResidentViewCache};
use super::resident_readback_fusion::{
    fuse_resident_readback_copies, validate_fused_resident_readbacks, FusedResidentReadbacks,
    ResidentReadbackCopy, ResidentReadbackView,
};
use super::resident_upload_fusion::{
    fuse_resident_upload_copies, push_resident_upload_copy, ResidentUploadCopy,
};
use super::staging_reserve::{
    clear_vec_slots, reserve_smallvec, reserve_vec, reserved_vec, resize_vec_slots,
};
use crate::numeric::CUDA_NUMERIC;
use smallvec::SmallVec;

const CUDA_RESIDENT_BUDGET_NUMERATOR: u64 = 9;
const CUDA_RESIDENT_BUDGET_DENOMINATOR: u64 = 10;
const CUDA_RESIDENT_TRANSFER_ACCOUNTING: TransferAccountingPolicy =
    TransferAccountingPolicy::new("CUDA resident", "split the transfer into bounded chunks");

enum ResidentStreamFailure {
    Completed(BackendError),
    CompletionUnproven(BackendError),
}

impl ResidentStreamFailure {
    fn into_error(self) -> BackendError {
        match self {
            Self::Completed(error) | Self::CompletionUnproven(error) => error,
        }
    }
}

fn cuda_resident_total_budget_bytes(total_memory: u64) -> u64 {
    let budget = (u128::from(total_memory) * u128::from(CUDA_RESIDENT_BUDGET_NUMERATOR))
        / u128::from(CUDA_RESIDENT_BUDGET_DENOMINATOR);
    budget as u64
}

fn cuda_resident_live_budget_bytes(
    total_memory: u64,
    live_free_memory: u64,
    resident_bytes: u64,
) -> u64 {
    let total_budget = cuda_resident_total_budget_bytes(total_memory);
    if resident_bytes >= total_budget {
        return resident_bytes;
    }
    let accounted_available = total_budget - resident_bytes;
    let live_available = cuda_resident_total_budget_bytes(live_free_memory);
    resident_bytes + accounted_available.min(live_available)
}

impl CudaBackend {
    fn with_resident_stream<T>(
        &self,
        operation: impl FnOnce(&crate::stream::CudaStream) -> Result<T, BackendError>,
    ) -> Result<T, BackendError> {
        self.with_resident_stream_classified(operation)
            .map_err(ResidentStreamFailure::into_error)
    }

    fn with_resident_stream_classified<T>(
        &self,
        operation: impl FnOnce(&crate::stream::CudaStream) -> Result<T, BackendError>,
    ) -> Result<T, ResidentStreamFailure> {
        let stream = self
            .launch_resources
            .acquire_stream()
            .map_err(ResidentStreamFailure::Completed)?;
        let result = operation(&stream);
        if result.is_err() {
            match stream.synchronize() {
                Ok(()) => self.telemetry.record_sync_point(),
                Err(error) => {
                    tracing::error!(
                        "Fix: failed to synchronize CUDA resident I/O stream after operation error: {error}. In-flight resident I/O resources will not be recycled."
                    );
                    std::mem::forget(stream);
                    return result.map_err(ResidentStreamFailure::CompletionUnproven);
                }
            }
        }
        self.launch_resources.release_stream(stream);
        result.map_err(ResidentStreamFailure::Completed)
    }
}

fn add_resident_transfer_bytes(
    total: &mut u64,
    bytes: usize,
    label: &str,
) -> Result<(), BackendError> {
    CUDA_RESIDENT_TRANSFER_ACCOUNTING.add_bytes(total, bytes, label)
}

fn add_resident_copy_count(total: &mut usize, label: &str) -> Result<(), BackendError> {
    CUDA_RESIDENT_TRANSFER_ACCOUNTING.add_copy_count(total, label)
}

fn add_resident_copy_slots(
    total: &mut usize,
    slots: usize,
    label: &str,
) -> Result<(), BackendError> {
    CUDA_RESIDENT_TRANSFER_ACCOUNTING.add_copy_slots(total, slots, label)
}

fn resident_upload_staging<'a>(
    upload_count: usize,
    copy_label: &'static str,
    view_label: &'static str,
) -> Result<(SmallVec<[ResidentUploadCopy<'a>; 8]>, ResidentViewCache), BackendError> {
    let mut copies = SmallVec::<[ResidentUploadCopy<'a>; 8]>::new();
    reserve_smallvec(&mut copies, upload_count, copy_label)?;
    let mut resident_view_cache = ResidentViewCache::new();
    reserve_smallvec(&mut resident_view_cache, upload_count, view_label)?;
    Ok((copies, resident_view_cache))
}

fn clear_resident_copy_outputs(
    copies: &[ResidentReadbackCopy],
    outputs: &mut OutputBuffers,
) -> Result<(), BackendError> {
    resize_vec_slots(outputs, copies.len(), "readback output")?;
    clear_vec_slots(outputs);
    Ok(())
}

pub(crate) fn reserve_borrowed_resident_readback_outputs(
    views: &[ResidentReadbackView],
    outputs: &mut [&mut Vec<u8>],
) -> Result<(), BackendError> {
    if views.len() != outputs.len() {
        return Err(BackendError::InvalidProgram {
            fix: format!(
                "Fix: CUDA resident readback materialization expected {} caller output slot(s) but received {}. Rebuild the resident readback fusion plan before copying bytes.",
                views.len(),
                outputs.len()
            ),
        });
    }
    for (view, output) in views.iter().zip(outputs.iter_mut()) {
        reserve_vec(
            *output,
            view.byte_len,
            "borrowed resident readback output bytes",
        )?;
    }
    Ok(())
}

fn reserve_resident_readback_outputs(
    views: &[ResidentReadbackView],
    outputs: &mut OutputBuffers,
) -> Result<(), BackendError> {
    let existing_slots_to_copy = outputs.len().min(views.len());
    if outputs.len() < views.len() {
        reserve_vec(outputs, views.len(), "resident readback output slots")?;
    }
    for (view, output) in views
        .iter()
        .take(existing_slots_to_copy)
        .zip(outputs.iter_mut())
    {
        reserve_vec(output, view.byte_len, "resident readback output bytes")?;
    }
    let mut appended_outputs = reserved_vec(
        views.len() - existing_slots_to_copy,
        "resident readback appended output slots",
    )?;
    for view in views.iter().skip(existing_slots_to_copy) {
        appended_outputs.push(reserved_vec(
            view.byte_len,
            "resident readback appended output bytes",
        )?);
    }
    outputs.truncate(views.len());
    outputs.extend(appended_outputs);
    Ok(())
}

fn next_resident_readback_view(
    views: &[ResidentReadbackView],
    view_index: &mut usize,
    total_copy_slots: usize,
) -> Result<ResidentReadbackView, BackendError> {
    let view = views
        .get(*view_index)
        .copied()
        .ok_or_else(|| BackendError::InvalidProgram {
            fix: format!(
                "Fix: CUDA resident fused batched readback ran out of output views while reserving {total_copy_slots} copy slot(s). Rebuild the resident readback fusion plan before collecting outputs."
            ),
        })?;
    *view_index += 1;
    Ok(view)
}

fn reserve_resident_readback_batch_outputs(
    copy_batches: &[SmallVec<[ResidentReadbackCopy; 8]>],
    views: &[ResidentReadbackView],
    outputs: &mut Vec<OutputBuffers>,
) -> Result<(), BackendError> {
    let existing_batches_to_copy = outputs.len().min(copy_batches.len());
    if outputs.len() < copy_batches.len() {
        reserve_vec(outputs, copy_batches.len(), "batched readback output slots")?;
    }

    let mut appended_items_by_batch = reserved_vec(
        existing_batches_to_copy,
        "batched readback appended item groups",
    )?;
    let mut appended_batches = reserved_vec(
        copy_batches.len() - existing_batches_to_copy,
        "batched readback appended output batches",
    )?;
    let total_copy_slots = views.len();
    let mut view_index = 0usize;

    for (copies, batch_outputs) in copy_batches
        .iter()
        .take(existing_batches_to_copy)
        .zip(outputs.iter_mut())
    {
        let existing_items_to_copy = batch_outputs.len().min(copies.len());
        if batch_outputs.len() < copies.len() {
            reserve_vec(batch_outputs, copies.len(), "batched readback item slots")?;
        }
        for output in batch_outputs.iter_mut().take(existing_items_to_copy) {
            let view = next_resident_readback_view(views, &mut view_index, total_copy_slots)?;
            reserve_vec(output, view.byte_len, "batched readback item bytes")?;
        }
        let mut appended_items = reserved_vec(
            copies.len() - existing_items_to_copy,
            "batched readback appended item slots",
        )?;
        for _ in existing_items_to_copy..copies.len() {
            let view = next_resident_readback_view(views, &mut view_index, total_copy_slots)?;
            appended_items.push(reserved_vec(
                view.byte_len,
                "batched readback appended item bytes",
            )?);
        }
        appended_items_by_batch.push(appended_items);
    }

    for copies in copy_batches.iter().skip(existing_batches_to_copy) {
        let mut batch_outputs =
            reserved_vec(copies.len(), "batched readback appended batch item slots")?;
        for _ in copies {
            let view = next_resident_readback_view(views, &mut view_index, total_copy_slots)?;
            batch_outputs.push(reserved_vec(
                view.byte_len,
                "batched readback appended batch item bytes",
            )?);
        }
        appended_batches.push(batch_outputs);
    }

    if view_index != views.len() {
        return Err(BackendError::InvalidProgram {
            fix: format!(
                "Fix: CUDA resident fused batched readback produced {} output view(s) for {view_index} consumed slot(s). Keep resident readback fusion cardinality-preserving before collecting outputs.",
                views.len()
            ),
        });
    }

    outputs.truncate(copy_batches.len());
    for ((copies, batch_outputs), appended_items) in copy_batches
        .iter()
        .take(existing_batches_to_copy)
        .zip(outputs.iter_mut())
        .zip(appended_items_by_batch)
    {
        batch_outputs.truncate(copies.len());
        batch_outputs.extend(appended_items);
    }
    outputs.extend(appended_batches);
    Ok(())
}

impl CudaBackend {
    /// Allocate a CUDA-resident buffer owned by this backend.
    pub fn allocate_resident(&self, byte_len: usize) -> Result<CudaResidentBuffer, BackendError> {
        if byte_len == 0 {
            return Err(BackendError::InvalidProgram {
                fix: "Fix: CUDA resident buffers must have a non-zero byte length.".to_string(),
            });
        }
        self.warmup()?;
        let resident_budget = self.cuda_resident_budget_bytes()?;
        let handle = self.resident_store.allocate(byte_len, resident_budget)?;
        self.telemetry.record_resident_allocation_bytes(
            CUDA_NUMERIC.usize_to_u64(byte_len, "resident allocation byte count")?,
        );
        Ok(handle)
    }

    /// Upload bytes into an existing CUDA-resident buffer.
    pub fn upload_resident(
        &self,
        handle: CudaResidentBuffer,
        bytes: &[u8],
    ) -> Result<(), BackendError> {
        self.upload_resident_many(&[(handle, bytes)])
    }

    /// Upload several full CUDA-resident buffers with one stream synchronization.
    pub fn upload_resident_many(
        &self,
        uploads: &[(CudaResidentBuffer, &[u8])],
    ) -> Result<(), BackendError> {
        if uploads.is_empty() {
            return Ok(());
        }
        let mut uploaded_bytes = 0_u64;
        let (mut copies, mut resident_view_cache) =
            resident_upload_staging(uploads.len(), "upload copy", "resident upload view cache")?;
        for &(handle, bytes) in uploads {
            let buffer = self.resident_store.view_cached(
                handle,
                &mut resident_view_cache,
                "resident upload view cache",
            )?;
            if bytes.len() != buffer.byte_len {
                return Err(BackendError::InvalidProgram {
                    fix: format!(
                        "Fix: CUDA resident upload for handle {} expected {} bytes but received {}.",
                        handle.id,
                        buffer.byte_len,
                        bytes.len()
                    ),
                });
            }
            push_resident_upload_copy(
                &mut copies,
                &mut uploaded_bytes,
                handle.id,
                buffer.ptr,
                bytes,
                "upload",
            )?;
        }
        let (copies, uploaded_bytes) = fuse_resident_upload_copies(copies)?;
        self.copy_resident_uploads(&copies, uploaded_bytes)
    }

    /// Download bytes from an existing CUDA-resident buffer.
    pub fn download_resident(&self, handle: CudaResidentBuffer) -> Result<Vec<u8>, BackendError> {
        let byte_len = self.resident_store.view(handle)?.byte_len;
        let mut bytes = reserved_vec(byte_len, "resident download output bytes")?;
        self.download_resident_into(handle, &mut bytes)?;
        Ok(bytes)
    }

    /// Download several full CUDA-resident buffers with one stream fence.
    pub fn download_resident_many(
        &self,
        handles: &[CudaResidentBuffer],
    ) -> Result<Vec<Vec<u8>>, BackendError> {
        let mut outputs = reserved_vec(handles.len(), "resident output")?;
        self.download_resident_many_into(handles, &mut outputs)?;
        Ok(outputs)
    }

    /// Download several full CUDA-resident buffers into caller-owned output
    /// slots with one stream fence.
    pub fn download_resident_many_into(
        &self,
        handles: &[CudaResidentBuffer],
        outputs: &mut OutputBuffers,
    ) -> Result<(), BackendError> {
        let mut copies = SmallVec::<[ResidentReadbackCopy; 8]>::new();
        reserve_smallvec(&mut copies, handles.len(), "full readback copy")?;
        let mut expected_copy_count = 0usize;
        let mut resident_view_cache = ResidentViewCache::new();
        reserve_smallvec(
            &mut resident_view_cache,
            handles.len(),
            "resident full-readback view cache",
        )?;
        for &handle in handles {
            let buffer = self.resident_store.view_cached(
                handle,
                &mut resident_view_cache,
                "resident full-readback view cache",
            )?;
            copies.push(ResidentReadbackCopy {
                handle_id: handle.id,
                src: if buffer.byte_len == 0 { 0 } else { buffer.ptr },
                byte_len: buffer.byte_len,
            });
            if buffer.byte_len != 0 {
                add_resident_copy_count(&mut expected_copy_count, "full readback")?;
            }
        }
        if expected_copy_count == 0 {
            return clear_resident_copy_outputs(&copies, outputs);
        }
        self.download_resident_fused_copies_many_into(&copies, outputs)
    }

    /// Download bytes from an existing CUDA-resident buffer into caller-owned
    /// storage.
    pub fn download_resident_into(
        &self,
        handle: CudaResidentBuffer,
        bytes: &mut Vec<u8>,
    ) -> Result<(), BackendError> {
        let byte_len = self.resident_store.view(handle)?.byte_len;
        self.download_resident_range_into(handle, 0, byte_len, bytes)
    }

    /// Download a byte range from an existing CUDA-resident buffer.
    pub fn download_resident_range(
        &self,
        handle: CudaResidentBuffer,
        byte_offset: usize,
        byte_len: usize,
    ) -> Result<Vec<u8>, BackendError> {
        let mut bytes = reserved_vec(byte_len, "resident ranged download output bytes")?;
        self.download_resident_range_into(handle, byte_offset, byte_len, &mut bytes)?;
        Ok(bytes)
    }

    /// Download a byte range from an existing CUDA-resident buffer into
    /// caller-owned storage.
    pub fn download_resident_range_into(
        &self,
        handle: CudaResidentBuffer,
        byte_offset: usize,
        byte_len: usize,
        bytes: &mut Vec<u8>,
    ) -> Result<(), BackendError> {
        self.download_resident_ranges_into(&[(handle, byte_offset, byte_len)], &mut [bytes])
    }

    /// Download selected byte ranges from resident buffers into caller-owned
    /// output slots with one stream fence.
    pub fn download_resident_ranges_into(
        &self,
        ranges: &[(CudaResidentBuffer, usize, usize)],
        outputs: &mut [&mut Vec<u8>],
    ) -> Result<(), BackendError> {
        if ranges.len() != outputs.len() {
            return Err(BackendError::InvalidProgram {
                fix: format!(
                    "Fix: CUDA resident ranged batch download expected matching range/output counts but got {} range(s) and {} output(s).",
                    ranges.len(),
                    outputs.len()
                ),
            });
        }
        let mut copies = SmallVec::<[ResidentReadbackCopy; 8]>::new();
        reserve_smallvec(&mut copies, ranges.len(), "ranged readback copy")?;
        let mut expected_copy_count = 0usize;
        let mut resident_view_cache = ResidentViewCache::new();
        reserve_smallvec(
            &mut resident_view_cache,
            ranges.len(),
            "resident ranged-readback view cache",
        )?;
        for &(handle, byte_offset, byte_len) in ranges {
            let buffer = self.resident_store.view_cached(
                handle,
                &mut resident_view_cache,
                "resident ranged-readback view cache",
            )?;
            let end = vyre_driver::accounting::checked_usize_byte_range_end_lazy(
                byte_offset,
                byte_len,
                buffer.byte_len,
                || {
                    BackendError::InvalidProgram {
                    fix: format!(
                        "Fix: CUDA resident ranged batch download for handle {} overflows usize at offset {byte_offset} len {byte_len}.",
                        handle.id
                    ),
                }
                },
                |end| {
                    BackendError::InvalidProgram {
                    fix: format!(
                        "Fix: CUDA resident ranged batch download for handle {} requested bytes [{byte_offset}..{end}) but buffer has {} bytes.",
                        handle.id, buffer.byte_len
                    ),
                }
                },
            )?;
            let src = if byte_len == 0 {
                0
            } else {
                vyre_driver::accounting::checked_add_u64_usize_offset_lazy(
                    buffer.ptr,
                    byte_offset,
                    || {
                        BackendError::InvalidProgram {
                        fix: format!(
                            "Fix: CUDA resident ranged batch download byte offset {byte_offset} does not fit CUdeviceptr arithmetic for handle {}.",
                            handle.id
                        ),
                    }
                    },
                    || {
                        BackendError::InvalidProgram {
                        fix: format!(
                            "Fix: CUDA resident ranged batch download pointer arithmetic overflowed for handle {} at offset {byte_offset}.",
                            handle.id
                        ),
                    }
                    },
                )?
            };
            copies.push(ResidentReadbackCopy {
                handle_id: handle.id,
                src,
                byte_len,
            });
            if byte_len != 0 {
                add_resident_copy_count(&mut expected_copy_count, "ranged readback")?;
            }
        }
        let fused_readbacks = fuse_resident_readback_copies(&copies)?;
        validate_fused_resident_readbacks(
            &fused_readbacks,
            copies.len(),
            "resident ranged batch download",
        )?;
        reserve_borrowed_resident_readback_outputs(&fused_readbacks.views, outputs)?;
        if expected_copy_count == 0 {
            for output in outputs.iter_mut() {
                output.clear();
            }
            return Ok(());
        }
        let (host_transfers, copy_count) =
            self.stage_fused_resident_readbacks_to_host(&fused_readbacks)?;
        for (view, output) in fused_readbacks.views.iter().zip(outputs.iter_mut()) {
            host_transfers.collect_output_range_into(
                view.copy_slot,
                view.byte_offset,
                view.byte_len,
                *output,
            )?;
        }
        self.record_resident_readback_telemetry(
            &fused_readbacks,
            copy_count,
            "resident readback operation count",
        )?;
        Ok(())
    }

    /// Download selected byte ranges from several CUDA-resident buffers with one stream fence.
    pub(crate) fn download_resident_readbacks_many(
        &self,
        handles: &[CudaResidentBuffer],
        readbacks: &[CudaOutputReadback],
    ) -> Result<Vec<Vec<u8>>, BackendError> {
        let mut outputs = reserved_vec(handles.len(), "resident readback output")?;
        self.download_resident_readbacks_many_into(handles, readbacks, &mut outputs)?;
        Ok(outputs)
    }

    /// Download selected byte ranges from several CUDA-resident buffers into
    /// caller-owned output slots with one stream fence.
    pub(crate) fn download_resident_readbacks_many_into(
        &self,
        handles: &[CudaResidentBuffer],
        readbacks: &[CudaOutputReadback],
        outputs: &mut OutputBuffers,
    ) -> Result<(), BackendError> {
        if handles.len() != readbacks.len() {
            return Err(BackendError::InvalidProgram {
                fix: format!(
                    "Fix: CUDA resident readback expected matching handle/range counts but got {} handle(s) and {} range(s).",
                    handles.len(),
                    readbacks.len()
                ),
            });
        }
        let mut copies = SmallVec::<[ResidentReadbackCopy; 8]>::new();
        reserve_smallvec(&mut copies, handles.len(), "readback copy")?;
        let mut resident_view_cache = ResidentViewCache::new();
        reserve_smallvec(
            &mut resident_view_cache,
            handles.len(),
            "resident readback view cache",
        )?;
        for (&handle, readback) in handles.iter().zip(readbacks.iter()) {
            let buffer = self.resident_store.view_cached(
                handle,
                &mut resident_view_cache,
                "resident readback view cache",
            )?;
            let end = vyre_driver::accounting::checked_usize_byte_range_end_lazy(
                readback.device_offset,
                readback.byte_len,
                buffer.byte_len,
                || {
                    BackendError::InvalidProgram {
                    fix: format!(
                        "Fix: CUDA resident readback for handle {} overflows usize at offset {} len {}.",
                        handle.id, readback.device_offset, readback.byte_len
                    ),
                }
                },
                |end| {
                    BackendError::InvalidProgram {
                    fix: format!(
                        "Fix: CUDA resident readback for handle {} requested bytes [{}..{}) but buffer has {} bytes.",
                        handle.id, readback.device_offset, end, buffer.byte_len
                    ),
                }
                },
            )?;
            let src = if readback.byte_len == 0 {
                0
            } else {
                vyre_driver::accounting::checked_add_u64_usize_offset_lazy(
                    buffer.ptr,
                    readback.device_offset,
                    || {
                        BackendError::InvalidProgram {
                        fix: format!(
                            "Fix: CUDA resident readback device offset {} does not fit CUdeviceptr arithmetic for handle {}.",
                            readback.device_offset, handle.id
                        ),
                    }
                    },
                    || {
                        BackendError::InvalidProgram {
                        fix: format!(
                            "Fix: CUDA resident readback pointer arithmetic overflowed for handle {} at offset {}.",
                            handle.id, readback.device_offset
                        ),
                    }
                    },
                )?
            };
            copies.push(ResidentReadbackCopy {
                handle_id: handle.id,
                src,
                byte_len: readback.byte_len,
            });
        }
        self.download_resident_fused_copies_many_into(&copies, outputs)
    }

    fn stage_fused_resident_readbacks_to_host(
        &self,
        fused_readbacks: &FusedResidentReadbacks,
    ) -> Result<(HostTransferAllocations, usize), BackendError> {
        self.warmup()?;
        let mut host_transfers = HostTransferAllocations::with_capacity(
            std::sync::Arc::clone(&self.host_pool),
            fused_readbacks.non_empty_copy_count,
            fused_readbacks.copies.len(),
        )?;
        let copy_count = match self.with_resident_stream_classified(|stream| {
            let mut copy_count = 0usize;
            for copy in &fused_readbacks.copies {
                let dst = host_transfers.push_output(copy.byte_len)?;
                if copy.byte_len != 0 {
                    // SAFETY: FFI to libcuda.so. Source pointer/range was
                    // validated against the resident allocation before staging;
                    // the pinned host destination remains owned until the stream
                    // fence completes.
                    unsafe {
                        super::copy::d2h_async_checked(dst, copy.src, copy.byte_len, stream.raw())?;
                    }
                    copy_count += 1;
                }
            }
            if copy_count != 0 {
                stream.synchronize()?;
                self.telemetry.record_sync_point();
            }
            Ok::<usize, BackendError>(copy_count)
        }) {
            Ok(copy_count) => copy_count,
            Err(ResidentStreamFailure::Completed(error)) => return Err(error),
            Err(ResidentStreamFailure::CompletionUnproven(error)) => {
                std::mem::forget(host_transfers);
                return Err(error);
            }
        };
        Ok((host_transfers, copy_count))
    }

    fn record_resident_readback_telemetry(
        &self,
        fused_readbacks: &FusedResidentReadbacks,
        copy_count: usize,
        operation_count_label: &str,
    ) -> Result<(), BackendError> {
        self.telemetry
            .record_device_to_host_readback(fused_readbacks.bytes);
        self.telemetry.record_device_readback_operations(
            CUDA_NUMERIC.usize_to_u64(copy_count, operation_count_label)?,
        );
        Ok(())
    }

    fn download_resident_fused_copies_many_into(
        &self,
        copies: &[ResidentReadbackCopy],
        outputs: &mut OutputBuffers,
    ) -> Result<(), BackendError> {
        let fused_readbacks = fuse_resident_readback_copies(copies)?;
        validate_fused_resident_readbacks(
            &fused_readbacks,
            copies.len(),
            "resident fused readback",
        )?;
        reserve_resident_readback_outputs(&fused_readbacks.views, outputs)?;
        if fused_readbacks.non_empty_copy_count == 0 {
            clear_vec_slots(outputs);
            return Ok(());
        }
        let (host_transfers, copy_count) =
            self.stage_fused_resident_readbacks_to_host(&fused_readbacks)?;
        for (view, output) in fused_readbacks.views.iter().zip(outputs.iter_mut()) {
            host_transfers.collect_output_range_into(
                view.copy_slot,
                view.byte_offset,
                view.byte_len,
                output,
            )?;
        }
        self.record_resident_readback_telemetry(
            &fused_readbacks,
            copy_count,
            "resident fused readback operation count",
        )?;
        Ok(())
    }

    fn download_resident_fused_copy_batches_many_into(
        &self,
        copy_batches: &[SmallVec<[ResidentReadbackCopy; 8]>],
        total_copy_slots: usize,
        outputs: &mut Vec<OutputBuffers>,
    ) -> Result<(), BackendError> {
        let mut flat_copies = SmallVec::<[ResidentReadbackCopy; 8]>::new();
        reserve_smallvec(
            &mut flat_copies,
            total_copy_slots,
            "flat fused batch readback copy",
        )?;
        for copies in copy_batches {
            flat_copies.extend(copies.iter().copied());
        }

        let fused_readbacks = fuse_resident_readback_copies(&flat_copies)?;
        validate_fused_resident_readbacks(
            &fused_readbacks,
            total_copy_slots,
            "resident fused batched readback",
        )?;
        reserve_resident_readback_batch_outputs(copy_batches, &fused_readbacks.views, outputs)?;
        if fused_readbacks.non_empty_copy_count == 0 {
            for batch_outputs in outputs.iter_mut() {
                clear_vec_slots(batch_outputs);
            }
            return Ok(());
        }

        let (host_transfers, copy_count) =
            self.stage_fused_resident_readbacks_to_host(&fused_readbacks)?;

        let mut fused_views = fused_readbacks.views.iter().copied();
        for (copies, batch_outputs) in copy_batches.iter().zip(outputs.iter_mut()) {
            for output in batch_outputs {
                let view = fused_views.next().ok_or_else(|| BackendError::InvalidProgram {
                    fix: format!(
                        "Fix: CUDA resident fused batched readback ran out of output views while materializing {} batch(es). Rebuild the resident readback fusion plan before collecting outputs.",
                        copy_batches.len()
                    ),
                })?;
                host_transfers.collect_output_range_into(
                    view.copy_slot,
                    view.byte_offset,
                    view.byte_len,
                    output,
                )?;
            }
        }
        if fused_views.next().is_some() {
            return Err(BackendError::InvalidProgram {
                fix: format!(
                    "Fix: CUDA resident fused batched readback produced more output views than {} requested copy slot(s). Keep resident readback fusion cardinality-preserving before collecting outputs.",
                    total_copy_slots
                ),
            });
        }
        self.record_resident_readback_telemetry(
            &fused_readbacks,
            copy_count,
            "resident fused batched readback operation count",
        )?;
        Ok(())
    }

    /// Download selected byte ranges from several resident-output batches into
    /// caller-owned output storage with one stream fence.
    pub(crate) fn download_resident_readback_batches_many_into(
        &self,
        handle_batches: &[SmallVec<[CudaResidentBuffer; 8]>],
        readback_batches: &[SmallVec<[CudaOutputReadback; 8]>],
        outputs: &mut Vec<OutputBuffers>,
    ) -> Result<(), BackendError> {
        if handle_batches.len() != readback_batches.len() {
            return Err(BackendError::InvalidProgram {
                fix: format!(
                    "Fix: CUDA resident batch readback expected matching batch counts but got {} handle batch(es) and {} range batch(es).",
                    handle_batches.len(),
                    readback_batches.len()
                ),
            });
        }
        let mut copy_batches = SmallVec::<[SmallVec<[ResidentReadbackCopy; 8]>; 8]>::new();
        reserve_smallvec(&mut copy_batches, handle_batches.len(), "readback batch")?;
        let mut total_copy_slots = 0usize;
        for (batch_index, (handles, readbacks)) in handle_batches
            .iter()
            .zip(readback_batches.iter())
            .enumerate()
        {
            if handles.len() != readbacks.len() {
                return Err(BackendError::InvalidProgram {
                    fix: format!(
                        "Fix: CUDA resident batch readback item {batch_index} expected matching handle/range counts but got {} handle(s) and {} range(s).",
                        handles.len(),
                        readbacks.len()
                    ),
                });
            }
            let mut copies = SmallVec::<[ResidentReadbackCopy; 8]>::new();
            reserve_smallvec(&mut copies, handles.len(), "batched readback copy")?;
            add_resident_copy_slots(&mut total_copy_slots, handles.len(), "batch readback")?;
            let mut resident_view_cache = ResidentViewCache::new();
            reserve_smallvec(
                &mut resident_view_cache,
                handles.len(),
                "resident batched-readback view cache",
            )?;
            for (&handle, readback) in handles.iter().zip(readbacks.iter()) {
                let buffer = self.resident_store.view_cached(
                    handle,
                    &mut resident_view_cache,
                    "resident batched-readback view cache",
                )?;
                let end = vyre_driver::accounting::checked_usize_byte_range_end_lazy(
                    readback.device_offset,
                    readback.byte_len,
                    buffer.byte_len,
                    || {
                        BackendError::InvalidProgram {
                        fix: format!(
                            "Fix: CUDA resident batch readback for handle {} overflows usize at offset {} len {}.",
                            handle.id, readback.device_offset, readback.byte_len
                        ),
                    }
                    },
                    |end| {
                        BackendError::InvalidProgram {
                        fix: format!(
                            "Fix: CUDA resident batch readback for handle {} requested bytes [{}..{}) but buffer has {} bytes.",
                            handle.id, readback.device_offset, end, buffer.byte_len
                        ),
                    }
                    },
                )?;
                let src = if readback.byte_len == 0 {
                    0
                } else {
                    vyre_driver::accounting::checked_add_u64_usize_offset_lazy(
                        buffer.ptr,
                        readback.device_offset,
                        || {
                            BackendError::InvalidProgram {
                            fix: format!(
                                "Fix: CUDA resident batch readback device offset {} does not fit CUdeviceptr arithmetic for handle {}.",
                                readback.device_offset, handle.id
                            ),
                        }
                        },
                        || {
                            BackendError::InvalidProgram {
                            fix: format!(
                                "Fix: CUDA resident batch readback pointer arithmetic overflowed for handle {} at offset {}.",
                                handle.id, readback.device_offset
                            ),
                        }
                        },
                    )?
                };
                copies.push(ResidentReadbackCopy {
                    handle_id: handle.id,
                    src,
                    byte_len: readback.byte_len,
                });
            }
            copy_batches.push(copies);
        }
        self.download_resident_fused_copy_batches_many_into(
            &copy_batches,
            total_copy_slots,
            outputs,
        )
    }

    /// Free a CUDA-resident buffer handle.
    pub fn free_resident(&self, handle: CudaResidentBuffer) -> Result<(), BackendError> {
        self.resident_store.free(handle)
    }

    /// Upload a partial byte slice into a CUDA-resident buffer at a byte offset.
    pub fn upload_resident_at(
        &self,
        handle: CudaResidentBuffer,
        dst_offset_bytes: usize,
        bytes: &[u8],
    ) -> Result<(), BackendError> {
        self.upload_resident_at_many(&[(handle, dst_offset_bytes, bytes)])
    }

    /// Upload several partial byte slices into CUDA-resident buffers with one stream fence.
    pub fn upload_resident_at_many(
        &self,
        uploads: &[(CudaResidentBuffer, usize, &[u8])],
    ) -> Result<(), BackendError> {
        if uploads.is_empty() {
            return Ok(());
        }
        let mut uploaded_bytes = 0_u64;
        let (mut copies, mut resident_view_cache) = resident_upload_staging(
            uploads.len(),
            "offset upload copy",
            "resident offset-upload view cache",
        )?;
        for &(handle, dst_offset_bytes, bytes) in uploads {
            let buffer = self.resident_store.view_cached(
                handle,
                &mut resident_view_cache,
                "resident offset-upload view cache",
            )?;
            let dst_ptr = checked_resident_dst(
                handle,
                buffer.ptr,
                buffer.byte_len,
                dst_offset_bytes,
                bytes.len(),
            )?;
            push_resident_upload_copy(
                &mut copies,
                &mut uploaded_bytes,
                handle.id,
                dst_ptr,
                bytes,
                "offset upload",
            )?;
        }
        let (copies, uploaded_bytes) = fuse_resident_upload_copies(copies)?;
        self.copy_resident_uploads(&copies, uploaded_bytes)
    }

    fn copy_resident_uploads(
        &self,
        copies: &[ResidentUploadCopy<'_>],
        uploaded_bytes: u64,
    ) -> Result<(), BackendError> {
        if copies.is_empty() {
            return Ok(());
        }
        self.warmup()?;
        let mut host_transfers = HostTransferAllocations::with_capacity(
            std::sync::Arc::clone(&self.host_pool),
            copies.len(),
            0,
        )?;
        match self.with_resident_stream_classified(|stream| {
            for copy in copies {
                let bytes = copy.bytes.as_slice();
                let host_ptr = host_transfers.push_upload(bytes)?;
                // SAFETY: FFI to libcuda.so. Pointer args were validated by the
                // matching alloc / store API; lifetimes are documented in the
                // surrounding function. cuda_check (or matching CUresult guard)
                // propagates non-success codes as BackendError.
                unsafe {
                    super::copy::h2d_async_checked(
                        copy.dst_ptr,
                        host_ptr,
                        bytes.len(),
                        stream.raw(),
                    )?;
                }
            }
            stream.synchronize()
        }) {
            Ok(()) => {}
            Err(ResidentStreamFailure::Completed(error)) => return Err(error),
            Err(ResidentStreamFailure::CompletionUnproven(error)) => {
                std::mem::forget(host_transfers);
                return Err(error);
            }
        }
        self.telemetry.record_sync_point();
        self.telemetry.record_host_to_device_bytes(uploaded_bytes);
        self.telemetry.record_host_upload_operations(
            CUDA_NUMERIC.usize_to_u64(copies.len(), "resident upload operation count")?,
        );
        drop(host_transfers);
        Ok(())
    }

    /// Return the raw CUDA device pointer for a resident buffer.
    pub fn resident_device_ptr(&self, handle: CudaResidentBuffer) -> Result<u64, BackendError> {
        self.with_resident(handle, |buffer| Ok(buffer.ptr))
    }

    /// Bytes currently held by CUDA resident buffers.
    #[must_use]
    pub fn resident_allocated_bytes(&self) -> u64 {
        self.resident_store.allocated_bytes()
    }

    fn cuda_resident_budget_bytes(&self) -> Result<u64, BackendError> {
        Ok(cuda_resident_live_budget_bytes(
            self.caps.total_memory,
            cuda_live_free_memory_bytes()?,
            self.resident_store.allocated_bytes(),
        ))
    }

    /// Pin a pre-allocated host buffer as page-locked for fast async H2D.
    ///
    /// # Safety
    ///
    /// The caller asserts `ptr..ptr+byte_len` is a uniquely owned, mapped
    /// host region that lives at least until [`Self::unpin_host_buffer`] is called.
    pub unsafe fn pin_host_buffer(&self, ptr: u64, byte_len: usize) -> Result<(), BackendError> {
        if byte_len == 0 {
            return Err(BackendError::InvalidProgram {
                fix: "Fix: pin_host_buffer requires a non-zero byte length.".to_string(),
            });
        }
        self.warmup()?;
        // SAFETY: The caller provided the host range lifetime and uniqueness
        // guarantees documented on this unsafe public API.
        unsafe { super::host_memory::register_host_buffer(ptr, byte_len, "cuMemHostRegister_v2") }
    }

    /// Unregister a previously [`Self::pin_host_buffer`]d host region.
    ///
    /// # Safety
    ///
    /// The caller asserts there are no in-flight async copies sourcing from
    /// this region.
    pub unsafe fn unpin_host_buffer(&self, ptr: u64) -> Result<(), BackendError> {
        self.warmup()?;
        // SAFETY: The caller guarantees no in-flight async copies still use
        // this host range, as documented on this unsafe public API.
        unsafe { super::host_memory::unregister_host_buffer(ptr, "cuMemHostUnregister") }
    }

    /// Async H2D copy from a pinned host pointer into a CUDA-resident buffer.
    ///
    /// # Safety
    ///
    /// The caller asserts `src_ptr..src_ptr+byte_count` is page-locked and
    /// remains uniquely borrowed until [`Self::synchronize_uploads`] returns.
    pub unsafe fn upload_resident_async_at(
        &self,
        handle: CudaResidentBuffer,
        dst_offset_bytes: usize,
        src_ptr: u64,
        byte_count: usize,
    ) -> Result<(), BackendError> {
        if byte_count == 0 {
            return Ok(());
        }
        self.with_resident(handle, |buffer| {
            let dst_ptr = checked_resident_dst(handle, buffer.ptr, buffer.byte_len, dst_offset_bytes, byte_count)?;
            let mut pending_stream = self.async_upload_stream.lock().map_err(|_| {
                BackendError::new("CUDA async upload stream lock was poisoned. Fix: recreate the backend before queueing more resident uploads.")
            })?;
            let created_stream = pending_stream.is_none();
            if created_stream {
                *pending_stream = Some(self.launch_resources.acquire_stream()?);
            }
            let stream = pending_stream.as_ref().ok_or_else(|| {
                BackendError::new("CUDA async upload stream allocation failed. Fix: recreate the backend or lower concurrent upload pressure.")
            })?;
            // SAFETY: FFI to libcuda.so. Pointer args were validated by the
            // matching alloc / store API; lifetimes are documented in the
            // surrounding function. cuda_check (or matching CUresult guard)
            // propagates non-success codes as BackendError.
            unsafe {
                let copy_result = super::copy::h2d_async_checked(
                    dst_ptr,
                    src_ptr as *const std::ffi::c_void,
                    byte_count,
                    stream.raw(),
                );
                if let Err(error) = copy_result {
                    if created_stream {
                        if let Some(stream) = pending_stream.take() {
                            self.launch_resources.release_stream(stream);
                        }
                    }
                    return Err(error);
                }
            }
            self.telemetry
                .record_host_to_device_bytes(CUDA_NUMERIC.usize_to_u64(
                    byte_count,
                    "resident byte upload count",
                )?);
            self.telemetry.record_host_upload_operations(1);
            Ok(())
        })
    }

    /// Block until every queued async H2D copy on this backend's upload stream completes.
    pub fn synchronize_uploads(&self) -> Result<(), BackendError> {
        self.warmup()?;
        let stream = self
            .async_upload_stream
            .lock()
            .map_err(|_| {
                BackendError::new("CUDA async upload stream lock was poisoned. Fix: recreate the backend before synchronizing resident uploads.")
            })?
            .take();
        let Some(stream) = stream else {
            return Ok(());
        };
        if let Err(error) = stream.synchronize() {
            tracing::error!(
                "Fix: failed to synchronize CUDA async resident upload stream: {error}. In-flight async resident upload stream will not be recycled."
            );
            std::mem::forget(stream);
            return Err(error);
        }
        self.telemetry.record_sync_point();
        self.launch_resources.release_stream(stream);
        Ok(())
    }
}

#[cfg(test)]

mod resident_budget_tests {
    use super::{cuda_resident_live_budget_bytes, cuda_resident_total_budget_bytes};

    #[test]
    fn resident_budget_caps_new_allocations_against_live_free_vram() {
        assert_eq!(cuda_resident_total_budget_bytes(10_000), 9_000);
        assert_eq!(
            cuda_resident_live_budget_bytes(10_000, 1_000, 0),
            900,
            "Fix: resident allocation budget must respect live free VRAM, not only total board memory."
        );
        assert_eq!(
            cuda_resident_live_budget_bytes(10_000, 8_000, 2_000),
            9_000,
            "Fix: resident allocation budget must preserve already-owned resident bytes while capping only additional allocation headroom."
        );
        assert_eq!(
            cuda_resident_live_budget_bytes(10_000, 0, 2_000),
            2_000,
            "Fix: zero live free VRAM must allow no additional resident allocation beyond already-owned handles."
        );
    }
}

#[cfg(test)]
mod async_upload_tests {
    #[test]
    fn async_uploads_use_backend_stream_not_null_stream() {
        let source = include_str!("resident_io.rs");
        assert!(
            source.contains("async_upload_stream")
                && source.contains("stream.raw()")
                && source.contains("super::copy::h2d_async_checked")
                && source.contains("release_stream(stream)"),
            "Fix: CUDA async resident uploads must retain a backend-owned stream until synchronize_uploads releases it."
        );
        assert!(
            !source.contains(concat!("cuStreamSynchronize", "(std::ptr::null_mut())"))
                && !source.contains(concat!(
                    "cuMemcpyHtoDAsync_v2(\n                        dst_ptr,\n                        src_ptr as *const std::ffi::c_void,\n                        byte_count,\n                        ",
                    "std::ptr::null_mut(),"
                )),
            "Fix: CUDA async resident uploads must not enqueue or synchronize on the null stream; that creates a global device fence."
        );
    }

    #[test]
    fn resident_io_stream_errors_require_completion_before_reuse() {
        let source = include_str!("resident_io.rs");
        assert!(
            source.contains("enum ResidentStreamFailure")
                && source.contains("Completed(BackendError)")
                && source.contains("CompletionUnproven(BackendError)")
                && source.contains("fn with_resident_stream_classified<T>"),
            "Fix: CUDA resident I/O stream cleanup must classify completion-proven and completion-unproven failures."
        );
        let helper = source
            .split("fn with_resident_stream_classified<T>")
            .nth(1)
            .expect("Fix: CUDA resident I/O must use a classified stream lease helper.")
            .split("fn add_resident_transfer_bytes")
            .next()
            .expect("Fix: resident I/O stream helper must precede transfer accounting.");
        assert!(
            helper.contains("let result = operation(&stream);")
                && helper.contains("if result.is_err()")
                && helper.contains("match stream.synchronize()")
                && helper.contains("Ok(()) => self.telemetry.record_sync_point()")
                && helper.contains("In-flight resident I/O resources will not be recycled.")
                && helper.contains("std::mem::forget(stream);")
                && helper.contains("ResidentStreamFailure::CompletionUnproven")
                && helper.contains("result.map_err(ResidentStreamFailure::Completed)"),
            "Fix: CUDA resident I/O stream helper must not return a stream to the pool after an operation error unless completion is proven."
        );
        let error_cleanup_pos = helper
            .find("if result.is_err()")
            .expect("Fix: resident I/O stream helper must handle operation errors.");
        let release_pos = helper
            .find("self.launch_resources.release_stream(stream);")
            .expect("Fix: resident I/O stream helper must release completed streams.");
        assert!(
            error_cleanup_pos < release_pos,
            "Fix: resident I/O stream helper must complete error cleanup before pooled stream release."
        );
    }

    #[test]
    fn resident_io_host_staging_leaks_when_completion_is_unproven() {
        let source = include_str!("resident_io.rs");
        let Some(readback) = source
            .split(concat!("fn stage_fused_resident_", "readbacks_to_host"))
            .nth(1)
        else {
            assert!(
                false,
                "Fix: CUDA fused resident readback staging helper must exist."
            );
            return;
        };
        let Some(readback) = readback
            .split("fn record_resident_readback_telemetry")
            .next()
        else {
            assert!(
                false,
                "Fix: fused resident readback staging must precede readback telemetry."
            );
            return;
        };
        assert!(
            readback.contains("self.with_resident_stream_classified")
                && readback.contains("Err(ResidentStreamFailure::Completed(error)) => return Err(error)")
                && readback.contains("Err(ResidentStreamFailure::CompletionUnproven(error))")
                && readback.contains("std::mem::forget(host_transfers);"),
            "Fix: CUDA resident readback staging must leak pinned host transfers when D2H completion is unproven."
        );

        let Some(upload) = source
            .split("fn copy_resident_uploads")
            .nth(1)
        else {
            assert!(
                false,
                "Fix: CUDA resident upload staging helper must exist."
            );
            return;
        };
        let Some(upload) = upload
            .split("/// Async H2D copy")
            .next()
        else {
            assert!(
                false,
                "Fix: resident upload staging must precede async upload API."
            );
            return;
        };
        assert!(
            upload.contains("self.with_resident_stream_classified")
                && upload.contains("Err(ResidentStreamFailure::Completed(error)) => return Err(error)")
                && upload.contains("Err(ResidentStreamFailure::CompletionUnproven(error))")
                && upload.contains("std::mem::forget(host_transfers);"),
            "Fix: CUDA resident upload staging must leak pinned host transfers when H2D completion is unproven."
        );
    }

    #[test]
    fn synchronize_uploads_releases_stream_only_after_successful_sync() {
        let source = include_str!("resident_io.rs");
        let Some(synchronize_uploads) = source
            .split("pub fn synchronize_uploads(&self)")
            .nth(1)
        else {
            assert!(
                false,
                "Fix: CUDA async upload synchronization entrypoint must exist."
            );
            return;
        };
        let Some(synchronize_uploads) = synchronize_uploads
            .split("}\n}")
            .next()
        else {
            assert!(
                false,
                "Fix: synchronize_uploads must end inside the resident I/O impl."
            );
            return;
        };
        assert!(
            synchronize_uploads.contains("if let Err(error) = stream.synchronize()")
                && synchronize_uploads.contains("In-flight async resident upload stream will not be recycled.")
                && synchronize_uploads.contains("std::mem::forget(stream);")
                && synchronize_uploads.contains("return Err(error);"),
            "Fix: CUDA async resident upload synchronization must leak the stream when completion cannot be proven."
        );
        let Some(sync_pos) = synchronize_uploads.find("stream.synchronize()") else {
            assert!(
                false,
                "Fix: synchronize_uploads must synchronize the upload stream."
            );
            return;
        };
        let Some(telemetry_pos) = synchronize_uploads.find("self.telemetry.record_sync_point();")
        else {
            assert!(
                false,
                "Fix: synchronize_uploads must record sync telemetry after success."
            );
            return;
        };
        let Some(release_pos) =
            synchronize_uploads.find("self.launch_resources.release_stream(stream);")
        else {
            assert!(
                false,
                "Fix: synchronize_uploads must release successfully synchronized streams."
            );
            return;
        };
        assert!(
            sync_pos < telemetry_pos && telemetry_pos < release_pos,
            "Fix: synchronize_uploads must prove stream completion before telemetry or pooled stream release."
        );
    }

    #[test]
    fn fused_resident_readback_dma_is_single_sourced() {
        let source = include_str!("resident_io.rs");
        assert_eq!(
            source
                .matches(concat!("stage_fused_resident_", "readbacks_to_host("))
                .count(),
            4,
            "Fix: ranged, flat, and batched CUDA resident readbacks must share one D2H staging helper instead of drifting across duplicated copy loops."
        );
        assert_eq!(
            source
                .matches(concat!("super::copy::", "d2h_async_checked"))
                .count(),
            1,
            "Fix: CUDA resident D2H FFI must stay behind the single fused-readback staging helper."
        );
    }
}

fn checked_resident_dst(
    handle: CudaResidentBuffer,
    base_ptr: u64,
    buffer_len: usize,
    dst_offset_bytes: usize,
    byte_count: usize,
) -> Result<u64, BackendError> {
    let _end = vyre_driver::accounting::checked_usize_byte_range_end_lazy(
        dst_offset_bytes,
        byte_count,
        buffer_len,
        || {
            BackendError::InvalidProgram {
            fix: format!(
                "Fix: CUDA resident upload at offset {dst_offset_bytes} for handle {} would overflow usize.",
                handle.id
            ),
        }
        },
        |end| {
            BackendError::InvalidProgram {
            fix: format!(
                "Fix: CUDA resident upload for handle {} writes [{dst_offset_bytes}..{end}) but buffer is only {buffer_len} bytes; resize the resident slot or trim the source slice.",
                handle.id
            ),
        }
        },
    )?;
    vyre_driver::accounting::checked_add_u64_usize_offset_lazy(
        base_ptr,
        dst_offset_bytes,
        || {
            BackendError::InvalidProgram {
            fix: format!(
                "Fix: CUDA resident upload offset {dst_offset_bytes} does not fit CUdeviceptr arithmetic for handle {}.",
                handle.id
            ),
        }
        },
        || {
            BackendError::InvalidProgram {
            fix: format!(
                "Fix: CUDA resident upload pointer arithmetic overflowed for handle {} at offset {dst_offset_bytes}.",
                handle.id
            ),
        }
        },
    )
}
